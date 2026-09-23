//! The on-disk keystore for the solo-developer local signing key (plan §18.6, docs/09 §10,
//! ADR-0054, bn-1hape).
//!
//! > the solo-developer default is a local keypair minted on first use and recorded in the
//! > audit log
//! >
//! > — plan §18.6
//!
//! A keystore is one directory holding two files:
//!
//! | File | Content | Mode |
//! |---|---|---|
//! | [`KEY_FILE`] | [`KEY_MAGIC`], the 32-byte seed, the 32-byte public key ([`KEY_FILE_LEN`] bytes) | `0600` |
//! | [`AUDIT_FILE`] | [`AUDIT_MAGIC`], a record count, then each audit record as a length-prefixed canonical value | `0600` |
//!
//! The directory is `0700`. [`LocalKeystore::open_or_mint`] mints through
//! `SigningRegistry::mint` — so the mint is an audit record — on first use, and on every
//! later use re-reads the key and replays the audit log instead of drawing entropy.
//!
//! # The trust boundary (review cr-3l3n47)
//!
//! The daemon signs with whatever key this module returns, so a store another local user
//! could swap would let them choose the signing key. What is checked, and how:
//!
//! - **The store directory** is examined without following links: it must be a real
//!   directory, not a symlink ([`KeystoreError::SymlinkedStore`]), owned by the expected
//!   owner ([`KeystoreError::ForeignOwner`]), and not group- or world-writable
//!   ([`KeystoreError::UnsafeDirectory`]). An existing directory is never tightened: a
//!   store that fails these checks is refused, not repaired, and is never re-minted over.
//! - **Every ancestor** of the directory must not be group- or world-writable unless it is
//!   sticky, like `/tmp` ([`KeystoreError::UnsafeParent`]). Ancestors are otherwise trusted:
//!   their owners are not checked, and a symlink among them is followed. That is the
//!   boundary: whoever controls a parent of the store controls the store.
//! - **Each file is opened once, and everything is read from that descriptor.** The path
//!   is examined without following links (a symlink is [`KeystoreError::SymlinkedFile`]),
//!   then opened with `O_NOFOLLOW` where the platform constant is known, then the *opened
//!   descriptor* is examined (`fstat`): same device and inode as the path examined
//!   ([`KeystoreError::Swapped`]), a regular file ([`KeystoreError::NotRegular`]), owned by
//!   the expected owner, and no group or other permission bits
//!   ([`KeystoreError::InsecurePermissions`]). A file renamed over the path after that
//!   point is never read: `swap_after_open_reads_the_original_descriptor` holds it.
//!
//! The expected owner is the process's effective user, read from `/proc/self` on Linux
//! and Android. Elsewhere the caller names it with [`LocalKeystore::expecting_owner`], and
//! opening without it is [`KeystoreError::Unsupported`].
//!
//! # Bounded reads
//!
//! The key file's length is checked on the descriptor before a byte is read. The audit
//! log is replayed one record at a time: at most [`MAX_AUDIT_RECORDS`] records of at most
//! [`MAX_AUDIT_RECORD_LEN`] bytes each, each decoded, shape-checked, and applied before the
//! next is read, so no whole-log value is ever materialized and a hostile file cannot
//! amplify into a large allocation (cr-3l3n47).
//!
//! # Secret handling
//!
//! The seed only ever lives in `Zeroizing` buffers: the captured copy made while minting,
//! the fixed-size write buffer, and the fixed-size read buffer. What this cannot promise:
//! copies the compiler makes of a stack value when it moves it, and the kernel's page
//! cache of the file itself.
//!
//! Persisting later rotations and revocations belongs with the daemon operations that
//! perform them (bn-3glnv); this module persists the first-use mint.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use continuum_evidence::actor::ActorId;
use continuum_evidence::signing::{
    EntropyUnavailable, KeyEntropy, LocalSigner, MintError, PUBLIC_KEY_LEN, ReplayError,
    RestoreError, SEED_LEN, SigningRegistry, Zeroizing,
};

/// The key file's name inside the keystore directory.
pub const KEY_FILE: &str = "signer.key";

/// The audit log's name inside the keystore directory.
pub const AUDIT_FILE: &str = "signing-audit.log";

/// The key file's first eight bytes: format name and version.
pub const KEY_MAGIC: [u8; 8] = *b"CTMKEY01";

/// The audit log's first eight bytes: format name and version.
pub const AUDIT_MAGIC: [u8; 8] = *b"CTMAUD01";

/// The key file's exact length.
pub const KEY_FILE_LEN: usize = KEY_MAGIC.len() + SEED_LEN + PUBLIC_KEY_LEN;

/// The most audit records a store may hold.
pub const MAX_AUDIT_RECORDS: u32 = 4096;

/// The longest encoded audit record. A rotation, the largest record, encodes to under a
/// third of this (`audit_records_fit_the_record_bound`).
pub const MAX_AUDIT_RECORD_LEN: u32 = 1024;

/// Why a keystore could not be opened or minted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeystoreError {
    /// A filesystem operation failed.
    Io {
        /// What was being done.
        op: &'static str,
        /// The error kind.
        kind: io::ErrorKind,
    },
    /// Exactly one of the two files exists. The store is not re-minted.
    Incomplete {
        /// The missing file.
        missing: &'static str,
    },
    /// Neither file exists, and the caller asked to open, not to mint.
    Absent,
    /// The store directory is a symlink.
    SymlinkedStore,
    /// The store path exists and is not a directory.
    NotADirectory,
    /// The store directory is group- or world-writable.
    UnsafeDirectory {
        /// Its permission bits.
        mode: u32,
    },
    /// An ancestor of the store directory is group- or world-writable and not sticky.
    UnsafeParent {
        /// The ancestor.
        path: PathBuf,
        /// Its permission bits.
        mode: u32,
    },
    /// The store directory or a file in it is not owned by the expected owner.
    ForeignOwner {
        /// The directory or file.
        what: &'static str,
        /// Its owner.
        uid: u32,
        /// The expected owner.
        expected: u32,
    },
    /// A store file is a symlink.
    SymlinkedFile {
        /// The file.
        file: &'static str,
    },
    /// A store file is not a regular file.
    NotRegular {
        /// The file.
        file: &'static str,
    },
    /// The file opened is not the file examined: it was replaced in between.
    Swapped {
        /// The file.
        file: &'static str,
    },
    /// A file is group- or world-accessible.
    InsecurePermissions {
        /// The offending file.
        file: &'static str,
        /// Its permission bits.
        mode: u32,
    },
    /// A file's content is not what this format writes.
    Corrupt(&'static str),
    /// The audit log does not replay.
    Replay(ReplayError),
    /// The stored seed is not a key the audit log minted.
    Restore(RestoreError),
    /// Minting failed.
    Mint(MintError),
    /// This platform has no owner or permission model this module can enforce, or the
    /// process owner is not known and was not named.
    Unsupported,
}

impl fmt::Display for KeystoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { op, kind } => write!(f, "keystore I/O failed while {op}: {kind}"),
            Self::Incomplete { missing } => {
                write!(f, "keystore is incomplete: {missing} is missing")
            }
            Self::Absent => f.write_str("no keystore exists at this path"),
            Self::SymlinkedStore => f.write_str("the keystore directory is a symlink"),
            Self::NotADirectory => f.write_str("the keystore path is not a directory"),
            Self::UnsafeDirectory { mode } => {
                write!(
                    f,
                    "the keystore directory has mode {mode:o}; it must not be group- or world-writable"
                )
            }
            Self::UnsafeParent { path, mode } => write!(
                f,
                "{} has mode {mode:o}: a writable, non-sticky ancestor controls the keystore",
                path.display()
            ),
            Self::ForeignOwner {
                what,
                uid,
                expected,
            } => write!(f, "{what} is owned by uid {uid}, not {expected}"),
            Self::SymlinkedFile { file } => write!(f, "{file} is a symlink"),
            Self::NotRegular { file } => write!(f, "{file} is not a regular file"),
            Self::Swapped { file } => write!(f, "{file} was replaced while it was being opened"),
            Self::InsecurePermissions { file, mode } => {
                write!(
                    f,
                    "{file} has mode {mode:o}; group and other bits must be clear"
                )
            }
            Self::Corrupt(what) => write!(f, "keystore is corrupt: {what}"),
            Self::Replay(error) => write!(f, "the audit log does not replay: {error:?}"),
            Self::Restore(error) => write!(f, "the stored key is not restorable: {error:?}"),
            Self::Mint(error) => write!(f, "minting failed: {error:?}"),
            Self::Unsupported => {
                f.write_str("keystore ownership and permissions are unsupported here")
            }
        }
    }
}

impl std::error::Error for KeystoreError {}

fn io(op: &'static str) -> impl FnOnce(io::Error) -> KeystoreError {
    move |error| KeystoreError::Io {
        op,
        kind: error.kind(),
    }
}

/// An opened keystore: the replayed registry and the local signer.
#[derive(Debug)]
pub struct OpenedKeystore {
    registry: SigningRegistry,
    signer: LocalSigner,
    minted: bool,
}

impl OpenedKeystore {
    /// The registry, replayed from (or just written to) the audit log.
    #[must_use]
    pub const fn registry(&self) -> &SigningRegistry {
        &self.registry
    }

    /// The local signer.
    #[must_use]
    pub const fn signer(&self) -> &LocalSigner {
        &self.signer
    }

    /// Whether this call minted the key (first use) rather than reading it back.
    #[must_use]
    pub const fn minted(&self) -> bool {
        self.minted
    }

    /// Both halves, for a deployment that hands them to the daemon's receipt signer.
    #[must_use]
    pub fn into_parts(self) -> (SigningRegistry, LocalSigner) {
        (self.registry, self.signer)
    }
}

/// Wraps the caller's entropy capability during a first-use mint and keeps a zeroizing copy
/// of the seed it supplied, so the seed can be persisted. It is the caller's capability that
/// is consulted; this adds none of its own.
struct Capture<'a> {
    inner: &'a mut dyn KeyEntropy,
    seed: Option<Zeroizing<[u8; SEED_LEN]>>,
}

impl KeyEntropy for Capture<'_> {
    fn seed(&mut self) -> Result<Zeroizing<[u8; SEED_LEN]>, EntropyUnavailable> {
        let seed = self.inner.seed()?;
        let mut kept = Zeroizing::new([0u8; SEED_LEN]);
        kept.copy_from_slice(seed.as_ref());
        self.seed = Some(kept);
        Ok(seed)
    }
}

/// The audit log's on-disk bytes: [`AUDIT_MAGIC`], the record count, and each record as a
/// big-endian `u32` length followed by its canonical encoding.
///
/// # Errors
///
/// [`KeystoreError::Corrupt`] when the log holds more than [`MAX_AUDIT_RECORDS`] records or
/// a record encodes longer than [`MAX_AUDIT_RECORD_LEN`]: a log this module could not read
/// back is never written.
pub fn encode_audit_log(registry: &SigningRegistry) -> Result<Vec<u8>, KeystoreError> {
    let records = registry.audit_log();
    let count = u32::try_from(records.len())
        .ok()
        .filter(|count| *count <= MAX_AUDIT_RECORDS)
        .ok_or(KeystoreError::Corrupt(
            "the audit log exceeds its record bound",
        ))?;
    let mut out = Vec::from(AUDIT_MAGIC);
    out.extend_from_slice(&count.to_be_bytes());
    for record in records {
        let bytes = record.to_value().encode();
        let len = u32::try_from(bytes.len())
            .ok()
            .filter(|len| *len <= MAX_AUDIT_RECORD_LEN)
            .ok_or(KeystoreError::Corrupt(
                "an audit record exceeds its size bound",
            ))?;
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(&bytes);
    }
    Ok(out)
}

/// A keystore directory.
#[derive(Debug, Clone)]
pub struct LocalKeystore {
    dir: PathBuf,
    owner: Option<u32>,
    #[cfg(test)]
    after_open: Option<fn(&Path)>,
}

impl LocalKeystore {
    /// The keystore at `dir`. Nothing is read or created until it is opened.
    #[must_use]
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            owner: None,
            #[cfg(test)]
            after_open: None,
        }
    }

    /// Name the user the store must belong to, instead of the process's effective user.
    ///
    /// Required where the process owner cannot be read with std alone (outside Linux and
    /// Android). Naming a different user makes every check stricter for the process's own
    /// store, never looser for another user's: the files must still be private.
    #[must_use]
    pub const fn expecting_owner(mut self, uid: u32) -> Self {
        self.owner = Some(uid);
        self
    }

    /// The directory.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Open the store, minting the local key on first use (plan §18.6).
    ///
    /// First use — neither file exists — mints through `SigningRegistry::mint` with
    /// `entropy`, records the mint as audit record 0 attributed to `actor`, and writes both
    /// files. Every later call reads them back and draws no entropy. A store that exists in
    /// any other state is refused, never minted over.
    ///
    /// # Errors
    ///
    /// [`KeystoreError`], typed; see the module documentation.
    pub fn open_or_mint(
        &self,
        actor: &ActorId,
        entropy: &mut dyn KeyEntropy,
    ) -> Result<OpenedKeystore, KeystoreError> {
        match self.open() {
            Err(KeystoreError::Absent) => self.mint(actor, entropy),
            other => other,
        }
    }

    /// Open an existing store without minting.
    ///
    /// # Errors
    ///
    /// [`KeystoreError::Absent`] when neither file exists, otherwise as
    /// [`open_or_mint`](Self::open_or_mint).
    pub fn open(&self) -> Result<OpenedKeystore, KeystoreError> {
        #[cfg(unix)]
        {
            unix::open(self)
        }
        #[cfg(not(unix))]
        {
            Err(KeystoreError::Unsupported)
        }
    }

    fn mint(
        &self,
        actor: &ActorId,
        entropy: &mut dyn KeyEntropy,
    ) -> Result<OpenedKeystore, KeystoreError> {
        #[cfg(unix)]
        {
            unix::mint(self, actor, entropy)
        }
        #[cfg(not(unix))]
        {
            let _ = (actor, entropy);
            Err(KeystoreError::Unsupported)
        }
    }
}

#[cfg(unix)]
mod unix {
    use std::fs::{self, DirBuilder, File, Metadata, OpenOptions};
    use std::io::{ErrorKind, Read, Write};
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
    use std::path::Path;

    use continuum_evidence::actor::ActorId;
    use continuum_evidence::signing::{
        KeyEntropy, PUBLIC_KEY_LEN, ReplayError, SEED_LEN, SigningAuditRecord, SigningRegistry,
        Zeroizing,
    };
    use continuum_value::value::Value;

    use super::{
        AUDIT_FILE, AUDIT_MAGIC, Capture, KEY_FILE, KEY_FILE_LEN, KEY_MAGIC, KeystoreError,
        LocalKeystore, MAX_AUDIT_RECORD_LEN, MAX_AUDIT_RECORDS, OpenedKeystore, encode_audit_log,
        io,
    };

    /// `O_NOFOLLOW` where its value is known without a C binding. Where it is not (0),
    /// the descriptor check still refuses a followed link: the opened inode would not be
    /// the one the path named.
    #[cfg(all(
        any(target_os = "linux", target_os = "android"),
        any(
            target_arch = "aarch64",
            target_arch = "arm",
            target_arch = "powerpc",
            target_arch = "powerpc64"
        )
    ))]
    pub(super) const O_NOFOLLOW: i32 = 0o100_000;
    #[cfg(all(
        any(target_os = "linux", target_os = "android"),
        not(any(
            target_arch = "aarch64",
            target_arch = "arm",
            target_arch = "powerpc",
            target_arch = "powerpc64"
        ))
    ))]
    pub(super) const O_NOFOLLOW: i32 = 0o400_000;
    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))]
    pub(super) const O_NOFOLLOW: i32 = 0x0100;
    #[cfg(not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )))]
    pub(super) const O_NOFOLLOW: i32 = 0;

    const GROUP_OR_OTHER: u32 = 0o077;
    const GROUP_OR_OTHER_WRITE: u32 = 0o022;
    const STICKY: u32 = 0o1000;

    /// The user the store must belong to.
    fn owner(store: &LocalKeystore) -> Result<u32, KeystoreError> {
        if let Some(uid) = store.owner {
            return Ok(uid);
        }
        if cfg!(any(target_os = "linux", target_os = "android")) {
            // `/proc/self` is owned by the process's effective user.
            return fs::metadata("/proc/self")
                .map(|metadata| metadata.uid())
                .map_err(|_| KeystoreError::Unsupported);
        }
        Err(KeystoreError::Unsupported)
    }

    fn lstat(path: &Path) -> Result<Option<Metadata>, KeystoreError> {
        match fs::symlink_metadata(path) {
            Ok(metadata) => Ok(Some(metadata)),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(io("examining the keystore")(error)),
        }
    }

    /// The directory checks of the module documentation. `None` when it does not exist.
    fn check_dir(dir: &Path, owner: u32) -> Result<Option<()>, KeystoreError> {
        let Some(metadata) = lstat(dir)? else {
            return Ok(None);
        };
        if metadata.file_type().is_symlink() {
            return Err(KeystoreError::SymlinkedStore);
        }
        if !metadata.is_dir() {
            return Err(KeystoreError::NotADirectory);
        }
        if metadata.uid() != owner {
            return Err(KeystoreError::ForeignOwner {
                what: "the keystore directory",
                uid: metadata.uid(),
                expected: owner,
            });
        }
        let mode = metadata.mode() & 0o7777;
        if mode & GROUP_OR_OTHER_WRITE != 0 {
            return Err(KeystoreError::UnsafeDirectory { mode });
        }
        let absolute = std::path::absolute(dir).map_err(io("resolving the keystore path"))?;
        for ancestor in absolute.ancestors().skip(1) {
            if ancestor.as_os_str().is_empty() {
                continue;
            }
            let metadata = fs::metadata(ancestor).map_err(io("examining a keystore ancestor"))?;
            let mode = metadata.mode() & 0o7777;
            if mode & GROUP_OR_OTHER_WRITE != 0 && mode & STICKY == 0 {
                return Err(KeystoreError::UnsafeParent {
                    path: ancestor.to_path_buf(),
                    mode,
                });
            }
        }
        Ok(Some(()))
    }

    /// Open `path` once and bind every later check and read to that descriptor.
    pub(super) fn open_bound(
        path: &Path,
        file: &'static str,
        owner: u32,
    ) -> Result<File, KeystoreError> {
        let examined = lstat(path)?.ok_or(KeystoreError::Incomplete { missing: file })?;
        if examined.file_type().is_symlink() {
            return Err(KeystoreError::SymlinkedFile { file });
        }
        if !examined.is_file() {
            return Err(KeystoreError::NotRegular { file });
        }
        let handle = OpenOptions::new()
            .read(true)
            .custom_flags(O_NOFOLLOW)
            .open(path)
            .map_err(|error| match lstat(path) {
                Ok(Some(now)) if now.file_type().is_symlink() => {
                    KeystoreError::SymlinkedFile { file }
                }
                _ => io("opening a keystore file")(error),
            })?;
        let opened = handle.metadata().map_err(io("examining a keystore file"))?;
        if (opened.dev(), opened.ino()) != (examined.dev(), examined.ino()) {
            return Err(KeystoreError::Swapped { file });
        }
        if !opened.is_file() {
            return Err(KeystoreError::NotRegular { file });
        }
        if opened.uid() != owner {
            return Err(KeystoreError::ForeignOwner {
                what: file,
                uid: opened.uid(),
                expected: owner,
            });
        }
        let mode = opened.mode() & 0o7777;
        if mode & GROUP_OR_OTHER != 0 {
            return Err(KeystoreError::InsecurePermissions { file, mode });
        }
        Ok(handle)
    }

    pub(super) fn open(store: &LocalKeystore) -> Result<OpenedKeystore, KeystoreError> {
        let owner = owner(store)?;
        if check_dir(&store.dir, owner)?.is_none() {
            return Err(KeystoreError::Absent);
        }
        let key_path = store.dir.join(KEY_FILE);
        let audit_path = store.dir.join(AUDIT_FILE);
        match (lstat(&key_path)?.is_some(), lstat(&audit_path)?.is_some()) {
            (false, false) => return Err(KeystoreError::Absent),
            (true, false) => {
                return Err(KeystoreError::Incomplete {
                    missing: AUDIT_FILE,
                });
            }
            (false, true) => return Err(KeystoreError::Incomplete { missing: KEY_FILE }),
            (true, true) => {}
        }
        let key = open_bound(&key_path, KEY_FILE, owner)?;
        let audit = open_bound(&audit_path, AUDIT_FILE, owner)?;
        #[cfg(test)]
        if let Some(hook) = store.after_open {
            hook(&store.dir);
        }

        let registry = read_audit(audit)?;
        let (seed, public_key) = read_key(key)?;
        let signer = registry.restore(seed).map_err(KeystoreError::Restore)?;
        if signer.identity().public_key() != &public_key {
            return Err(KeystoreError::Corrupt(
                "the stored public key is not the one the stored seed derives",
            ));
        }
        Ok(OpenedKeystore {
            registry,
            signer,
            minted: false,
        })
    }

    pub(super) fn mint(
        store: &LocalKeystore,
        actor: &ActorId,
        entropy: &mut dyn KeyEntropy,
    ) -> Result<OpenedKeystore, KeystoreError> {
        let owner = owner(store)?;
        if check_dir(&store.dir, owner)?.is_none() {
            DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(&store.dir)
                .map_err(io("creating the keystore directory"))?;
            check_dir(&store.dir, owner)?.ok_or(KeystoreError::Absent)?;
        }
        let mut registry = SigningRegistry::new();
        let mut capture = Capture {
            inner: entropy,
            seed: None,
        };
        let signer = registry
            .mint(actor, &mut capture)
            .map_err(KeystoreError::Mint)?;
        let seed = capture
            .seed
            .take()
            .ok_or(KeystoreError::Corrupt("minting drew no seed"))?;

        let mut buffer = Zeroizing::new([0u8; KEY_FILE_LEN]);
        buffer[..KEY_MAGIC.len()].copy_from_slice(&KEY_MAGIC);
        buffer[KEY_MAGIC.len()..KEY_MAGIC.len() + SEED_LEN].copy_from_slice(seed.as_ref());
        buffer[KEY_MAGIC.len() + SEED_LEN..].copy_from_slice(signer.identity().public_key());
        drop(seed);

        // The audit log is written first and the key last: a crash in between leaves an
        // audit log with no key, which `open` reports as `Incomplete` rather than a key no
        // audit record accounts for. `create_new` refuses an existing path, a symlink
        // included, so nothing is ever written over.
        let audit = encode_audit_log(&registry)?;
        write_new_private(&store.dir.join(AUDIT_FILE), &audit)?;
        write_new_private(&store.dir.join(KEY_FILE), buffer.as_ref())?;
        File::open(&store.dir)
            .and_then(|handle| handle.sync_all())
            .map_err(io("syncing the keystore directory"))?;
        Ok(OpenedKeystore {
            registry,
            signer,
            minted: true,
        })
    }

    fn write_new_private(path: &Path, bytes: &[u8]) -> Result<(), KeystoreError> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(O_NOFOLLOW)
            .open(path)
            .map_err(io("creating a keystore file"))?;
        file.write_all(bytes)
            .map_err(io("writing a keystore file"))?;
        file.sync_all().map_err(io("syncing a keystore file"))
    }

    fn read_key(
        mut file: File,
    ) -> Result<(Zeroizing<[u8; SEED_LEN]>, [u8; PUBLIC_KEY_LEN]), KeystoreError> {
        let len = file.metadata().map_err(io("examining the key file"))?.len();
        if len != KEY_FILE_LEN as u64 {
            return Err(KeystoreError::Corrupt("the key file has the wrong length"));
        }
        let mut buffer = Zeroizing::new([0u8; KEY_FILE_LEN]);
        file.read_exact(buffer.as_mut())
            .map_err(io("reading the key file"))?;
        if buffer[..KEY_MAGIC.len()] != KEY_MAGIC {
            return Err(KeystoreError::Corrupt("the key file has the wrong magic"));
        }
        let mut seed = Zeroizing::new([0u8; SEED_LEN]);
        seed.copy_from_slice(&buffer[KEY_MAGIC.len()..KEY_MAGIC.len() + SEED_LEN]);
        let mut public_key = [0u8; PUBLIC_KEY_LEN];
        public_key.copy_from_slice(&buffer[KEY_MAGIC.len() + SEED_LEN..]);
        Ok((seed, public_key))
    }

    fn read_u32(file: &mut File, what: &'static str) -> Result<u32, KeystoreError> {
        let mut bytes = [0u8; 4];
        file.read_exact(&mut bytes).map_err(|error| {
            if error.kind() == ErrorKind::UnexpectedEof {
                KeystoreError::Corrupt(what)
            } else {
                io("reading the audit log")(error)
            }
        })?;
        Ok(u32::from_be_bytes(bytes))
    }

    /// Replay the audit log one bounded record at a time.
    pub(super) fn read_audit(mut file: File) -> Result<SigningRegistry, KeystoreError> {
        let mut magic = [0u8; AUDIT_MAGIC.len()];
        file.read_exact(&mut magic)
            .map_err(|_| KeystoreError::Corrupt("the audit log has no header"))?;
        if magic != AUDIT_MAGIC {
            return Err(KeystoreError::Corrupt("the audit log has the wrong magic"));
        }
        let count = read_u32(&mut file, "the audit log has no record count")?;
        if count > MAX_AUDIT_RECORDS {
            return Err(KeystoreError::Corrupt(
                "the audit log claims more records than its bound",
            ));
        }
        let mut registry = SigningRegistry::new();
        let mut record = Vec::new();
        for _ in 0..count {
            let len = read_u32(&mut file, "an audit record is truncated")?;
            if len == 0 || len > MAX_AUDIT_RECORD_LEN {
                return Err(KeystoreError::Corrupt(
                    "an audit record exceeds its size bound",
                ));
            }
            record.clear();
            record.resize(len as usize, 0);
            file.read_exact(&mut record)
                .map_err(|_| KeystoreError::Corrupt("an audit record is truncated"))?;
            let value = Value::decode(&record)
                .map_err(|_| KeystoreError::Corrupt("an audit record is not a canonical value"))?;
            let parsed = SigningAuditRecord::from_value(&value)
                .map_err(|error| KeystoreError::Replay(ReplayError::Wire(error)))?;
            drop(value);
            registry
                .replay_record(parsed)
                .map_err(KeystoreError::Replay)?;
        }
        let mut trailing = [0u8; 1];
        if file
            .read(&mut trailing)
            .map_err(io("reading the audit log"))?
            != 0
        {
            return Err(KeystoreError::Corrupt("the audit log has trailing bytes"));
        }
        Ok(registry)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::fs::{self, OpenOptions};
    use std::os::unix::fs::{OpenOptionsExt, symlink};
    use std::path::{Path, PathBuf};

    use continuum_evidence::actor::ActorId;
    use continuum_evidence::signing::{
        EntropyUnavailable, KeyEntropy, SEED_LEN, SigningRegistry, Zeroizing,
    };

    use super::{AUDIT_FILE, KEY_FILE, KeystoreError, LocalKeystore, MAX_AUDIT_RECORD_LEN, unix};

    struct Fixed(u8);

    impl KeyEntropy for Fixed {
        fn seed(&mut self) -> Result<Zeroizing<[u8; SEED_LEN]>, EntropyUnavailable> {
            Ok(Zeroizing::new([self.0; SEED_LEN]))
        }
    }

    fn actor() -> ActorId {
        ActorId::new("human:solo-dev").expect("actor")
    }

    fn scratch(name: &str) -> PathBuf {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/tmp")
            .join(format!("keystore-unit-{name}"));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("scratch");
        root
    }

    /// The attacker's store sits beside the victim's; the hook renames its files over the
    /// victim's after `open` has bound its descriptors and before it reads them.
    fn swap_in_attacker(dir: &Path) {
        let attacker = dir.with_file_name("attacker");
        fs::rename(attacker.join(KEY_FILE), dir.join(KEY_FILE)).expect("swap key");
        fs::rename(attacker.join(AUDIT_FILE), dir.join(AUDIT_FILE)).expect("swap audit");
    }

    #[test]
    fn swap_after_open_reads_the_original_descriptor() {
        let root = scratch("swap");
        let victim_dir = root.join("victim");
        let victim = LocalKeystore::new(&victim_dir)
            .open_or_mint(&actor(), &mut Fixed(1))
            .expect("victim mints");
        let attacker = LocalKeystore::new(root.join("attacker"))
            .open_or_mint(&actor(), &mut Fixed(2))
            .expect("attacker mints");
        assert_ne!(victim.signer().identity(), attacker.signer().identity());

        let mut store = LocalKeystore::new(&victim_dir);
        store.after_open = Some(swap_in_attacker);
        let opened = store.open().expect("the bound descriptors still read");
        assert_eq!(
            opened.signer().identity(),
            victim.signer().identity(),
            "a file renamed over the path after open is never read"
        );
        // Anti-vacuity: the swap happened, so a fresh open now sees the attacker's key.
        let after = LocalKeystore::new(&victim_dir).open().expect("reopens");
        assert_eq!(after.signer().identity(), attacker.signer().identity());
    }

    #[test]
    fn o_nofollow_refuses_a_symlink_at_open() {
        let root = scratch("nofollow");
        let target = root.join("target");
        fs::write(&target, b"x").expect("target");
        let link = root.join("link");
        symlink(&target, &link).expect("symlink");
        let refused = OpenOptions::new()
            .read(true)
            .custom_flags(unix::O_NOFOLLOW)
            .open(&link);
        if unix::O_NOFOLLOW != 0 {
            assert!(refused.is_err(), "O_NOFOLLOW followed a symlink");
        }
        assert_eq!(
            unix::open_bound(&link, KEY_FILE, 0).map(|_| ()),
            Err(KeystoreError::SymlinkedFile { file: KEY_FILE })
        );
    }

    #[test]
    fn audit_records_fit_the_record_bound() {
        let mut registry = SigningRegistry::new();
        let first = registry.mint(&actor(), &mut Fixed(3)).expect("mint");
        registry
            .rotate(&first, &actor(), &mut Fixed(4))
            .expect("rotate");
        for record in registry.audit_log() {
            let len = record.to_value().encode().len();
            assert!(len * 3 <= MAX_AUDIT_RECORD_LEN as usize, "{len}");
        }
    }
}
