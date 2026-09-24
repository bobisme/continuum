//! The on-disk keystore for the daemon's signing authority (plan §18.6, docs/09 §10,
//! ADR-0054, bn-1hape, bn-18w74).
//!
//! > the solo-developer default is a local keypair minted on first use and recorded in the
//! > audit log
//! >
//! > — plan §18.6
//!
//! A keystore is one directory (`0700`) holding:
//!
//! | File | Content | Mode |
//! |---|---|---|
//! | [`STATE_FILE`] | [`STATE_MAGIC`], the held key's public key, then the audit log, the own keys with their kinds, the own links, the adopted links, the pre-signed revocations, and the peer keys revoked locally, each section a count and bounded length-prefixed records | `0600` |
//! | `signer-<64 hex>.key` | [`KEY_MAGIC`], the 32-byte seed, the 32-byte public key ([`KEY_FILE_LEN`] bytes), one file per held key, named by its public key | `0600` |
//!
//! [`LocalKeystore::open_or_mint`] mints through `SigningRegistry::mint` — so the mint is an
//! audit record — on first use, and on every later use reads the state and the held key
//! back instead of drawing entropy. [`LocalKeystore::persist`] records every later change
//! the daemon makes (a mint, a rotation, a revocation, an adoption): the daemon calls it
//! through the `SigningCustody` capability this type implements, before the change takes
//! effect, and `continuumd`'s `Builder::launch_signing` builds a daemon from it (bn-18w74).
//!
//! # Crash consistency
//!
//! The state file is the commit point. It is replaced whole: written to
//! [`STATE_TEMP_FILE`] (created new, `0600`, never following a link), synced, renamed over
//! [`STATE_FILE`], and the directory synced. A new key's file is written, and synced,
//! before the state that names it; a retired key's file is removed only after the state
//! that retires it is in place. So a crash leaves either the old state or the new one,
//! each with its held key's file present. The rename is the commit point:
//! [`LocalKeystore::persist`] reports a failure before it as `CustodyWrite::NotRecorded`
//! and one at or after it (the directory sync) as `CustodyWrite::Unconfirmed`, and a
//! [`KeystoreFaults`] seam can fail a write at each [`PersistPhase`] (review cr-33e464).
//! [`PENDING_FILE`] is written, durably, before the rename and removed after a confirmed
//! sync, so an unconfirmed write stays visible across a crash; the seam's crash oracle
//! (`KeystoreFaults::lose_rename`) lets a test choose which directory entry the crash
//! keeps.
//! A key file no state names is removed by
//! [`LocalKeystore::sweep`], which the launcher runs after a restart has validated the
//! state. A crash during the first-use mint leaves a key file and no state, which
//! [`LocalKeystore::open`] reports as [`KeystoreError::Incomplete`]: never re-minted.
//!
//! # The trust boundary (review cr-3l3n47)
//!
//! The daemon signs with whatever key this module returns, so a store another local user
//! could swap would let them choose the signing key. What is checked, and how:
//!
//! - **The store directory** is examined without following links: it must be a real
//!   directory, not a symlink ([`KeystoreError::SymlinkedStore`]), owned by the expected
//!   owner ([`KeystoreError::ForeignOwner`]), and private — no group or other permission
//!   bits at all ([`KeystoreError::UnsafeDirectory`]). An existing directory is never tightened: a
//!   store that fails these checks is refused, not repaired, and is never re-minted over.
//! - **Every component the store path resolves through** is trusted (review cr-2qu5zr, ssh
//!   `StrictModes` style). The path is resolved one component at a time, following each
//!   symlink by hand, and every directory and every symlink passed through — a symlink's
//!   containing directory included, and a symlink reached through another symlink — must
//!   be owned by root or the expected owner ([`KeystoreError::UntrustedAncestor`]); a
//!   directory must not be group- or world-writable unless it is sticky, like `/tmp`
//!   ([`KeystoreError::UnsafeParent`]). Then only root and the owner can change what the
//!   path resolves to, so resolving it again for a later lock, open, or rename reaches the
//!   same directory. As defence in depth the whole check is re-run immediately before each
//!   lock, open, write, rename, and removal, and the directory must still be the one (device
//!   and inode) the handle first validated ([`KeystoreError::StoreMoved`]); a refusal of
//!   that re-check removes nothing. Descriptor-relative (`*at`) calls would bind the
//!   operations to the directory itself, but std offers none without `unsafe` or a new
//!   dependency; with every component trusted, re-resolution is harmless without them.
//!   A component missing when the walk runs ends it early, and a store found beneath it is
//!   trusted only after a second, complete walk. A traversed symlink must also sit in a
//!   directory only root and the owner can write, sticky or not, and have exactly one link
//!   ([`KeystoreError::LinkedAncestor`]), so a hard link to one of the owner's symlinks,
//!   planted where `fs.protected_hardlinks` is off, is refused (review cr-2qu5zr round 2).
//!   The store's own files are opened only with one link ([`KeystoreError::HardLinked`]),
//!   and the store directory is private to its owner, so only root and the owner can name
//!   or plant an entry there. Stated limits: a FUSE mount can report any owner, as for ssh
//!   `StrictModes`; and where `fs.protected_hardlinks` is off, another user who can search
//!   the directory holding one of the path's symlinks can hard-link it elsewhere, raising
//!   its link count so the store is refused (`LinkedAncestor`) until the owner recreates
//!   the symlink — an availability limit, never an acceptance.
//! - **One writer.** Every write holds an exclusive lock on [`LOCK_FILE`]; a daemon
//!   launched from the store holds it for life ([`KeystoreError::Locked`]). A
//!   [`LocalKeystore`] is not `Clone`, and every write through one handle runs under its
//!   writer mutex, which alone owns the lifetime lock, so neither two threads nor two
//!   handles ever write at once. A read-only [`LocalKeystore::open`] from another handle
//!   takes a shared lock and is refused while a holder lives. Each write stages its state
//!   in a fresh `O_EXCL` temporary file (review cr-1dc5ii).
//! - **Each file is opened once, and everything is read from that descriptor.** The path
//!   is examined without following links (a symlink is [`KeystoreError::SymlinkedFile`]),
//!   then opened with `O_NOFOLLOW` where the platform constant is known, then the *opened
//!   descriptor* is examined (`fstat`): same device and inode as the path examined
//!   ([`KeystoreError::Swapped`]), a regular file ([`KeystoreError::NotRegular`]), owned by
//!   the expected owner, and no group or other permission bits
//!   ([`KeystoreError::InsecurePermissions`]), and one link ([`KeystoreError::HardLinked`]).
//!   A file renamed over the path after that
//!   point is never read: `swap_after_open_reads_the_original_descriptor` holds it.
//! - **Every file is created new** (`O_CREAT | O_EXCL`, `O_NOFOLLOW`, mode `0600`), so a
//!   write never follows a planted link and never reuses another file's permissions.
//!
//! The expected owner is the process's effective user, read from `/proc/self` on Linux
//! and Android. Elsewhere the caller names it with [`LocalKeystore::expecting_owner`], and
//! opening without it is [`KeystoreError::Unsupported`].
//! The lifetime lock also records the process that took it, read from `/proc/self` for
//! the same reason (this module names only filesystem facilities, which INV-015's audit of
//! this crate holds it to). So a durable custody launches on Linux and Android only;
//! elsewhere `SigningCustody::load_or_mint` is [`KeystoreError::Unsupported`], and a
//! `/proc` that is not mounted refuses the same way (review cr-1dc5ii).
//!
//! # Bounded reads
//!
//! Each file's length is checked on its descriptor before a byte is read: the key file is
//! exactly [`KEY_FILE_LEN`], and the state file at most [`MAX_STATE_FILE_LEN`], with at
//! most [`MAX_AUDIT_RECORDS`] audit records of at most [`MAX_AUDIT_RECORD_LEN`] bytes, and
//! at most [`MAX_LINKS`] links of at most `MAX_LINK_RECORD_LEN` bytes per section. Every
//! count is checked against its bound before its section is read, and every record is
//! decoded, shape-checked, and applied before the next is read, so no whole-log value is
//! ever materialized and a hostile file cannot amplify into a large allocation
//! (cr-3l3n47). A store is read back fail-closed: a corrupt, truncated, or oversize file
//! is a typed refusal, never repaired and never re-minted over.
//!
//! # Secret handling
//!
//! The seed only ever lives in `Zeroizing` buffers: the captured copy made while minting,
//! the fixed-size write buffer, and the fixed-size read buffer. No error, `Debug` output,
//! or file name carries it. What this cannot promise: copies the compiler makes of a stack
//! value when it moves it, the kernel's page cache, and the blocks of a removed file,
//! which a filesystem may keep until it reuses them.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use continuum_evidence::actor::ActorId;
pub use continuum_evidence::signing::CustodyWrite;
use continuum_evidence::signing::{
    EntropyUnavailable, KeyEntropy, LinkEvent, LocalSigner, MAX_LINK_RECORD_LEN, MintError,
    PUBLIC_KEY_LEN, ReplayError, RestoreError, SEED_LEN, SignedArtifactKind, SignerIdentity,
    SignerLink, SigningAuditRecord, SigningCustody, SigningCustodyState, SigningRegistry,
    Zeroizing,
};
use continuum_value::value::Value;

/// The state file's name inside the keystore directory.
pub const STATE_FILE: &str = "signing-state";

/// The temporary file a new state is written to before it is renamed over [`STATE_FILE`].
pub const STATE_TEMP_FILE: &str = "signing-state.tmp";

/// The lock file: every write to the store holds an exclusive lock on it, and a daemon
/// launched from the store holds it for its whole life, so two processes never write one
/// store (review of bn-18w74, S2).
pub const LOCK_FILE: &str = "signing.lock";

/// The pending marker: written, durably, before a state write's commit point and removed
/// once the write is confirmed. A store that holds it may hold either the old or the new
/// state, and a restart reconciles it (review cr-33e464): the launcher loads whichever
/// state is durable, validates it in full, and only then clears the marker
/// ([`LocalKeystore::sweep`]).
pub const PENDING_FILE: &str = "signing-pending";

/// The state file's first eight bytes: format name and version.
pub const STATE_MAGIC: [u8; 8] = *b"CTMSTA01";

/// A key file's name is this, 64 lowercase hex digits of its public key, and
/// [`KEY_FILE_SUFFIX`].
pub const KEY_FILE_PREFIX: &str = "signer-";

/// See [`KEY_FILE_PREFIX`].
pub const KEY_FILE_SUFFIX: &str = ".key";

/// How a refusal names the held key's file (its name varies with the key).
pub const KEY_FILE_LABEL: &str = "the held key file";

/// The two files of the bn-1hape layout, which kept the audit log apart from the key and
/// so could not be replaced atomically. A store that still holds either is refused
/// ([`KeystoreError::LegacyLayout`]), never read and never minted over.
pub const LEGACY_FILES: [&str; 2] = ["signer.key", "signing-audit.log"];

/// A key file's first eight bytes: format name and version.
pub const KEY_MAGIC: [u8; 8] = *b"CTMKEY01";

/// A key file's exact length.
pub const KEY_FILE_LEN: usize = KEY_MAGIC.len() + SEED_LEN + PUBLIC_KEY_LEN;

/// The most audit records a store may hold: the daemon's own bound (`continuumd`'s
/// `bundle::MAX_AUDIT_RECORDS`; `continuumd/tests/signing_custody.rs` holds the two equal).
pub const MAX_AUDIT_RECORDS: u32 = 4096;

/// The longest encoded audit record. A rotation, the largest record, encodes to under a
/// third of this (`audit_records_fit_the_record_bound`).
pub const MAX_AUDIT_RECORD_LEN: u32 = 1024;

/// The most links a store may hold in each of its three link sections: the daemon's
/// bundle bound (`continuumd`'s `bundle::MAX_BUNDLE_LINKS`, held equal the same way).
pub const MAX_LINKS: u32 = 4096;

/// The longest state file this format can write, checked on the descriptor before a byte
/// is read.
pub const MAX_STATE_FILE_LEN: u64 = {
    let records = MAX_AUDIT_RECORDS as u64;
    let links = MAX_LINKS as u64;
    let link = 4 + MAX_LINK_RECORD_LEN as u64;
    (STATE_MAGIC.len() + PUBLIC_KEY_LEN) as u64
        + 4
        + records * (4 + MAX_AUDIT_RECORD_LEN as u64)
        + 4
        + records * (PUBLIC_KEY_LEN as u64 + 1)
        + 3 * (4 + links * link)
        + 4
        + records * PUBLIC_KEY_LEN as u64
};

/// Why a keystore could not be opened, minted, written, or swept.
///
/// No variant carries key material: the fields are static labels, permission bits, user
/// ids, an ancestor directory's path, and the library's typed replay and restore errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeystoreError {
    /// A filesystem operation failed.
    Io {
        /// What was being done.
        op: &'static str,
        /// The error kind.
        kind: io::ErrorKind,
    },
    /// Part of the store is missing: a key file with no state, or a state whose held key
    /// has no file. The store is not re-minted.
    Incomplete {
        /// The missing file.
        missing: &'static str,
    },
    /// No store exists here, and the caller asked to open, not to mint.
    Absent,
    /// The store holds a file of the bn-1hape layout ([`LEGACY_FILES`]).
    LegacyLayout,
    /// Another process, or another handle in this one, holds the store's lock.
    Locked,
    /// This handle's lifetime lock was taken by another process: the handle is a copy a
    /// `fork` made, and only its owner writes or reads through it.
    InheritedHandle,
    /// A symlink in the store's path has more than one link: another name for it exists,
    /// which someone other than its owner may have created.
    LinkedAncestor {
        /// The symlink.
        path: PathBuf,
        /// Its link count.
        links: u64,
    },
    /// The store path now reaches another directory than the one this handle validated,
    /// or a missing ancestor appeared during the check and the path could not be proved.
    /// A handle refused this way stays refused (it is pinned to the directory it first
    /// validated); a store the owner moved is opened with a new handle.
    StoreMoved,
    /// An ancestor of the store directory belongs to a user other than root and the
    /// expected owner, who could rename the store away or swap another in.
    UntrustedAncestor {
        /// The ancestor.
        path: PathBuf,
        /// Its owner.
        uid: u32,
    },
    /// A store file has another hard link, which would keep its content after removal.
    HardLinked {
        /// The file.
        file: &'static str,
    },
    /// The store directory is a symlink.
    SymlinkedStore,
    /// The store path exists and is not a directory.
    NotADirectory,
    /// The store directory grants group or other permissions.
    UnsafeDirectory {
        /// Its permission bits.
        mode: u32,
    },
    /// An ancestor of the store directory is group- or world-writable and not sticky, or a
    /// directory holding a symlink of the store's path is group- or world-writable at all.
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
            Self::LegacyLayout => f.write_str(
                "the keystore holds a file of the earlier two-file layout; it is not read",
            ),
            Self::Locked => f.write_str("another handle holds the keystore's lock"),
            Self::LinkedAncestor { path, links } => write!(
                f,
                "{} is a symlink with {links} links; a keystore path follows only single links",
                path.display()
            ),
            Self::StoreMoved => {
                f.write_str("the keystore path no longer reaches the directory validated")
            }
            Self::InheritedHandle => {
                f.write_str("this keystore handle's lock belongs to another process")
            }
            Self::UntrustedAncestor { path, uid } => write!(
                f,
                "{} is owned by uid {uid}, neither root nor the keystore's owner",
                path.display()
            ),
            Self::HardLinked { file } => write!(f, "{file} has another hard link"),
            Self::SymlinkedStore => f.write_str("the keystore directory is a symlink"),
            Self::NotADirectory => f.write_str("the keystore path is not a directory"),
            Self::UnsafeDirectory { mode } => {
                write!(
                    f,
                    "the keystore directory has mode {mode:o}; group and other bits must be clear"
                )
            }
            Self::UnsafeParent { path, mode } => write!(
                f,
                "{} has mode {mode:o}: others can change what the keystore path reaches",
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

/// An opened keystore: the custody state and the held key.
#[derive(Debug)]
pub struct OpenedKeystore {
    state: SigningCustodyState,
    signer: LocalSigner,
    minted: bool,
    pending: bool,
}

impl OpenedKeystore {
    /// The registry, replayed from (or just written to) the state file.
    #[must_use]
    pub const fn registry(&self) -> &SigningRegistry {
        self.state.registry()
    }

    /// The custody state: own keys, links, and pre-signed revocations beside the registry.
    #[must_use]
    pub const fn state(&self) -> &SigningCustodyState {
        &self.state
    }

    /// The held key.
    #[must_use]
    pub const fn signer(&self) -> &LocalSigner {
        &self.signer
    }

    /// Whether the store holds the pending marker: a write passed its commit point without
    /// being confirmed, and this state is whichever one a crash left. The launcher's
    /// validation and sweep reconcile it.
    #[must_use]
    pub const fn pending(&self) -> bool {
        self.pending
    }

    /// Whether this call minted the key (first use) rather than reading it back.
    #[must_use]
    pub const fn minted(&self) -> bool {
        self.minted
    }

    /// The registry and the key, for a caller that installs them without the rest of the
    /// custody state (the daemon then treats the key as its only own key).
    #[must_use]
    pub fn into_parts(self) -> (SigningRegistry, LocalSigner) {
        (self.state.registry().clone(), self.signer)
    }

    /// The custody state and the key, for the launcher's full restore.
    #[must_use]
    pub fn into_custody(self) -> (SigningCustodyState, LocalSigner) {
        (self.state, self.signer)
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

/// The key file name of `identity`: [`KEY_FILE_PREFIX`], its public key in lowercase hex,
/// [`KEY_FILE_SUFFIX`]. The public key is not secret.
#[must_use]
pub fn key_file_name(identity: &SignerIdentity) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut name = String::with_capacity(KEY_FILE_PREFIX.len() + 64 + KEY_FILE_SUFFIX.len());
    name.push_str(KEY_FILE_PREFIX);
    for byte in identity.public_key() {
        name.push(char::from(DIGITS[usize::from(byte >> 4)]));
        name.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    name.push_str(KEY_FILE_SUFFIX);
    name
}

/// The serial of the next temporary state file this process writes.
static TEMP_SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Whether `name` is a temporary state file name this module writes: [`STATE_TEMP_FILE`],
/// alone or followed by `-` and a decimal serial.
fn is_temp_name(name: &str) -> bool {
    name == STATE_TEMP_FILE
        || name
            .strip_prefix(STATE_TEMP_FILE)
            .and_then(|rest| rest.strip_prefix('-'))
            .is_some_and(|serial| !serial.is_empty() && serial.bytes().all(|b| b.is_ascii_digit()))
}

/// Whether `name` is exactly a key file name this module writes.
fn is_key_file_name(name: &str) -> bool {
    name.strip_prefix(KEY_FILE_PREFIX)
        .and_then(|rest| rest.strip_suffix(KEY_FILE_SUFFIX))
        .is_some_and(|hex| {
            hex.len() == 2 * PUBLIC_KEY_LEN
                && hex
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
}

/// The one-byte encoding of a set of kinds. Each kind has a fixed bit, named here rather
/// than taken from an order, so a new kind is a compile error, not a silent shift.
const fn kind_bit(kind: SignedArtifactKind) -> u8 {
    match kind {
        SignedArtifactKind::Receipt => 0b0001,
        SignedArtifactKind::IntentBundle => 0b0010,
        SignedArtifactKind::DomainPack => 0b0100,
        SignedArtifactKind::IntentAcceptance => 0b1000,
    }
}

fn kind_bits(kinds: &BTreeSet<SignedArtifactKind>) -> u8 {
    kinds.iter().fold(0, |bits, kind| bits | kind_bit(*kind))
}

fn kinds_of(bits: u8) -> Result<BTreeSet<SignedArtifactKind>, KeystoreError> {
    let known = SignedArtifactKind::ALL
        .iter()
        .fold(0u8, |all, kind| all | kind_bit(*kind));
    if bits & !known != 0 {
        return Err(KeystoreError::Corrupt("an own key names an unknown kind"));
    }
    Ok(SignedArtifactKind::ALL
        .into_iter()
        .filter(|kind| bits & kind_bit(*kind) != 0)
        .collect())
}

fn put_count(
    out: &mut Vec<u8>,
    count: usize,
    bound: u32,
    what: &'static str,
) -> Result<(), KeystoreError> {
    let count = u32::try_from(count)
        .ok()
        .filter(|count| *count <= bound)
        .ok_or(KeystoreError::Corrupt(what))?;
    out.extend_from_slice(&count.to_be_bytes());
    Ok(())
}

fn put_framed(
    out: &mut Vec<u8>,
    bytes: &[u8],
    bound: u32,
    what: &'static str,
) -> Result<(), KeystoreError> {
    let len = u32::try_from(bytes.len())
        .ok()
        .filter(|len| *len <= bound)
        .ok_or(KeystoreError::Corrupt(what))?;
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(bytes);
    Ok(())
}

/// The state file's bytes (see the module documentation).
///
/// # Errors
///
/// [`KeystoreError::Corrupt`] when the state names no held key or a section exceeds its
/// bound: a state this module could not read back is never written.
pub fn encode_state(state: &SigningCustodyState) -> Result<Vec<u8>, KeystoreError> {
    let held = state
        .held()
        .ok_or(KeystoreError::Corrupt("the state names no held key"))?;
    let mut out = Vec::from(STATE_MAGIC);
    out.extend_from_slice(held.public_key());
    let records = state.registry().audit_log();
    put_count(
        &mut out,
        records.len(),
        MAX_AUDIT_RECORDS,
        "the audit log exceeds its record bound",
    )?;
    for record in records {
        put_framed(
            &mut out,
            &record.to_value().encode(),
            MAX_AUDIT_RECORD_LEN,
            "an audit record exceeds its size bound",
        )?;
    }
    put_count(
        &mut out,
        state.own().len(),
        MAX_AUDIT_RECORDS,
        "the own keys exceed their bound",
    )?;
    for (key, kinds) in state.own() {
        out.extend_from_slice(key.public_key());
        out.push(kind_bits(kinds));
    }
    for links in [state.own_links(), state.adopted_links()] {
        put_count(
            &mut out,
            links.len(),
            MAX_LINKS,
            "a link section exceeds its bound",
        )?;
        for link in links {
            put_framed(
                &mut out,
                &link.encode(),
                MAX_LINK_RECORD_LEN as u32,
                "a link exceeds its size bound",
            )?;
        }
    }
    put_count(
        &mut out,
        state.presigned().len(),
        MAX_LINKS,
        "the pre-signed revocations exceed their bound",
    )?;
    for (key, link) in state.presigned() {
        // The file keys each revocation by the key it revokes, so an entry filed under
        // another key could not be read back as written.
        if link.event()
            != &(LinkEvent::Revoked {
                signer: key.clone(),
            })
        {
            return Err(KeystoreError::Corrupt(
                "a pre-signed revocation is filed under another key",
            ));
        }
        put_framed(
            &mut out,
            &link.encode(),
            MAX_LINK_RECORD_LEN as u32,
            "a link exceeds its size bound",
        )?;
    }
    put_count(
        &mut out,
        state.local_revocations().len(),
        MAX_AUDIT_RECORDS,
        "the local revocations exceed their bound",
    )?;
    for key in state.local_revocations() {
        out.extend_from_slice(key.public_key());
    }
    Ok(out)
}

/// A cursor over a state file's bytes. Every read checks what remains first.
struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    fn take(&mut self, len: usize, what: &'static str) -> Result<&'a [u8], KeystoreError> {
        if len > self.bytes.len() {
            return Err(KeystoreError::Corrupt(what));
        }
        let (head, rest) = self.bytes.split_at(len);
        self.bytes = rest;
        Ok(head)
    }

    fn u32(&mut self, what: &'static str) -> Result<u32, KeystoreError> {
        let mut word = [0u8; 4];
        word.copy_from_slice(self.take(4, what)?);
        Ok(u32::from_be_bytes(word))
    }

    /// A count, refused above `bound` before its section is read.
    fn count(&mut self, bound: u32, what: &'static str) -> Result<u32, KeystoreError> {
        let count = self.u32(what)?;
        if count > bound {
            return Err(KeystoreError::Corrupt(what));
        }
        Ok(count)
    }

    /// One length-prefixed record, refused when empty or above `bound` before it is taken.
    fn framed(&mut self, bound: u32, what: &'static str) -> Result<&'a [u8], KeystoreError> {
        let len = self.u32(what)?;
        if len == 0 || len > bound {
            return Err(KeystoreError::Corrupt(what));
        }
        self.take(len as usize, what)
    }

    fn identity(&mut self, what: &'static str) -> Result<SignerIdentity, KeystoreError> {
        SignerIdentity::from_public_key(self.take(PUBLIC_KEY_LEN, what)?)
            .map_err(|_| KeystoreError::Corrupt(what))
    }

    fn link(&mut self) -> Result<SignerLink, KeystoreError> {
        let bytes = self.framed(
            MAX_LINK_RECORD_LEN as u32,
            "a link is truncated or oversize",
        )?;
        SignerLink::decode(bytes).map_err(|_| KeystoreError::Corrupt("a link is malformed"))
    }

    fn links(&mut self) -> Result<Vec<SignerLink>, KeystoreError> {
        let count = self.count(MAX_LINKS, "a link section exceeds its bound")?;
        (0..count).map(|_| self.link()).collect()
    }
}

/// Read a state file's bytes back, section by section, each bounded before it is read.
/// Only the shape is checked here; `ReceiptSigner::restored` checks the state against the
/// registry and the daemon's invariants.
///
/// # Errors
///
/// [`KeystoreError::Corrupt`] for anything this format does not write — a wrong magic, a
/// count or record over its bound, a truncated section, a malformed key, record, or link,
/// own keys or pre-signed revocations out of order or duplicated, trailing bytes — and
/// [`KeystoreError::Replay`] for an audit log that does not replay.
pub fn decode_state(bytes: &[u8]) -> Result<SigningCustodyState, KeystoreError> {
    if bytes.len() as u64 > MAX_STATE_FILE_LEN {
        return Err(KeystoreError::Corrupt(
            "the state file exceeds its size bound",
        ));
    }
    let mut reader = Reader { bytes };
    if reader.take(STATE_MAGIC.len(), "the state file has no header")? != STATE_MAGIC {
        return Err(KeystoreError::Corrupt("the state file has the wrong magic"));
    }
    let held = reader.identity("the state file names no valid held key")?;
    let count = reader.u32("the state file has no audit record count")?;
    if count > MAX_AUDIT_RECORDS {
        return Err(KeystoreError::Corrupt(
            "the audit log claims more records than its bound",
        ));
    }
    let mut registry = SigningRegistry::new();
    for _ in 0..count {
        let record = reader.framed(
            MAX_AUDIT_RECORD_LEN,
            "an audit record is truncated or oversize",
        )?;
        let value = Value::decode(record)
            .map_err(|_| KeystoreError::Corrupt("an audit record is not a canonical value"))?;
        let parsed = SigningAuditRecord::from_value(&value)
            .map_err(|error| KeystoreError::Replay(ReplayError::Wire(error)))?;
        drop(value);
        registry
            .replay_record(parsed)
            .map_err(KeystoreError::Replay)?;
    }
    let count = reader.count(MAX_AUDIT_RECORDS, "the own keys exceed their bound")?;
    let mut own = BTreeMap::new();
    for _ in 0..count {
        let key = reader.identity("an own key is truncated or invalid")?;
        let bits = reader.take(1, "an own key's kinds are truncated")?[0];
        if own.last_key_value().is_some_and(|(last, _)| last >= &key) {
            return Err(KeystoreError::Corrupt(
                "the own keys are not strictly ordered",
            ));
        }
        own.insert(key, kinds_of(bits)?);
    }
    let own_links = reader.links()?;
    let adopted_links = reader.links()?;
    let count = reader.count(MAX_LINKS, "the pre-signed revocations exceed their bound")?;
    let mut presigned: BTreeMap<SignerIdentity, SignerLink> = BTreeMap::new();
    for _ in 0..count {
        let link = reader.link()?;
        let LinkEvent::Revoked { signer } = link.event() else {
            return Err(KeystoreError::Corrupt(
                "a pre-signed link is not a revocation",
            ));
        };
        if presigned
            .last_key_value()
            .is_some_and(|(last, _)| last >= signer)
        {
            return Err(KeystoreError::Corrupt(
                "the pre-signed revocations are not strictly ordered",
            ));
        }
        presigned.insert(signer.clone(), link);
    }
    let count = reader.count(
        MAX_AUDIT_RECORDS,
        "the local revocations exceed their bound",
    )?;
    let mut local_revocations: BTreeSet<SignerIdentity> = BTreeSet::new();
    for _ in 0..count {
        let key = reader.identity("a local revocation is truncated or invalid")?;
        if local_revocations.last().is_some_and(|last| last >= &key) {
            return Err(KeystoreError::Corrupt(
                "the local revocations are not strictly ordered",
            ));
        }
        local_revocations.insert(key);
    }
    if !reader.bytes.is_empty() {
        return Err(KeystoreError::Corrupt("the state file has trailing bytes"));
    }
    Ok(SigningCustodyState::from_parts(
        registry,
        Some(held),
        own,
        own_links,
        adopted_links,
        presigned,
    )
    .with_local_revocations(local_revocations))
}

/// The points of a state write at which a deployment's fault seam may make it fail
/// (review cr-33e464). The rename is the commit point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistPhase {
    /// After the new state is written to the temporary file, before the rename: the write
    /// is not recorded.
    BeforeRename,
    /// After the rename, before the directory sync: the write is committed, unconfirmed.
    AfterRename,
    /// The directory sync itself fails: the write is committed, unconfirmed.
    DirectorySync,
}

/// A deterministic fault seam for state writes, the keystore's counterpart of the
/// publication store's `StorageFaults`. Consulted at each [`PersistPhase`] of every
/// [`LocalKeystore::persist`]; answering `true` makes the write fail there.
pub trait KeystoreFaults: Send + Sync {
    /// Whether the write fails at `phase`.
    fn fail(&self, phase: PersistPhase) -> bool;

    /// The crash-consistency oracle: after a write fails past its commit point, whether
    /// the crash that follows loses the rename, so a relaunch finds the old directory
    /// entry. `false` keeps the new one. Either is a state a real crash can leave.
    fn lose_rename(&self) -> bool {
        false
    }
}

/// A shared fault seam, named in `Debug` only by its presence.
#[derive(Clone)]
struct Faults(std::sync::Arc<dyn KeystoreFaults>);

impl fmt::Debug for Faults {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Faults")
    }
}

/// The store's lifetime lock as one handle owns it. A `fork` copies the handle, its mutex,
/// and this descriptor — one open file description, so one flock — into the child. The
/// owning process is recorded at construction, outside the mutex ([`LocalKeystore`]'s
/// `owner_process`), so a copy is told it is not the owner before it would lock a mutex
/// the fork may have copied locked (review cr-1dc5ii rounds 2 to 4).
///
/// Dropping it only closes this process's descriptor. A flock lives until the last
/// descriptor of its open file description closes, and nothing here unlocks explicitly,
/// so a child dropping its copy never releases the parent's lock.
#[derive(Debug)]
struct Held {
    /// Kept only for its lock, which it releases when it drops.
    _lock: std::fs::File,
}

/// The process constructing a handle, or 0 when it cannot be read.
fn constructing_process() -> u32 {
    #[cfg(unix)]
    {
        unix::process_id().unwrap_or(0)
    }
    #[cfg(not(unix))]
    {
        0
    }
}

/// A keystore directory.
///
/// One handle is the one writer (review cr-1dc5ii): it is not `Clone`, every write through
/// it runs under one mutex, and the store's lifetime lock, once taken, is owned by that
/// mutex alone.
///
/// # Fork safety: the ordering every path keeps (review cr-1dc5ii rounds 2 to 4)
///
/// The owning process is recorded when the handle is **constructed**, before any lock
/// exists. Every path checks it (`owned_here`) **before** it locks the writer mutex or the
/// store's flock. So whenever a `fork` happens — even while another thread holds the
/// mutex, which the child then inherits locked with no thread to unlock it — the child's
/// copy is refused ([`KeystoreError::InheritedHandle`]) before it can block. A handle
/// whose process could not be read is never treated as owned: it is refused
/// ([`KeystoreError::Unsupported`]).
///
/// | Step | First check | Then locks |
/// |---|---|---|
/// | [`new`](Self::new) | records the process; takes no lock | nothing |
/// | `SigningCustody::load_or_mint` (first lock) | `owned_here` | writer mutex, then the lifetime flock |
/// | [`open_or_mint`](Self::open_or_mint) → mint | `owned_here` (in `open`, then in the write guard) | writer mutex, then a flock for the write |
/// | [`persist`](Self::persist) | `owned_here` | writer mutex (write guard), then a flock unless the lifetime one is held |
/// | [`sweep`](Self::sweep) | `owned_here` | as `persist` |
/// | [`open`](Self::open) | `owned_here` | writer mutex, then a shared flock unless the lifetime one is held |
/// | drop | nothing | nothing: the mutex is dropped unlocked, and the lock file only closes this process's descriptor, which never releases another process's flock |
///
/// One window is outside the handle's control: a child that another thread starts
/// (`Command`) is forked before it execs, and until the exec it holds a copy of every
/// descriptor, a flock just taken included, even though all of them are close-on-exec.
/// A lock this process releases in that window stays held until the child execs, so a
/// concurrent lock attempt meanwhile is answered `Locked`: typed and fail-closed, never a
/// wrong answer (review cr-1dc5ii round 4).
///
/// Share a handle by reference or `Arc`, never by copy:
///
/// ```compile_fail
/// let store = continuum_security::keystore::LocalKeystore::new("store");
/// let copy = store.clone();
/// ```
#[derive(Debug)]
pub struct LocalKeystore {
    faults: Option<Faults>,
    dir: PathBuf,
    owner: Option<u32>,
    /// The one writer: every write through this handle holds this mutex for its whole
    /// length. Once [`SigningCustody::load_or_mint`] took the store's lifetime lock, the
    /// locked descriptor lives here, and only this handle owns it.
    writer: std::sync::Mutex<Option<Held>>,
    /// The process that constructed this handle, or 0 when it could not be read (then the
    /// handle is refused everywhere). Set in [`new`](Self::new), before any lock exists,
    /// and read without any lock: every path checks it before it locks anything. Atomic
    /// only so the hidden test seam `simulate_fork_copy` can flip it.
    owner_process: std::sync::atomic::AtomicU32,
    /// The device and inode of the store directory this handle first validated. Every later
    /// check requires the path to reach the same directory ([`KeystoreError::StoreMoved`]).
    dir_identity: std::sync::OnceLock<(u64, u64)>,
    #[cfg(test)]
    after_open: Option<fn(&Path)>,
}

impl LocalKeystore {
    /// The keystore at `dir`. Nothing is read or created until it is opened.
    #[must_use]
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            faults: None,
            dir: dir.into(),
            owner: None,
            writer: std::sync::Mutex::new(None),
            owner_process: std::sync::atomic::AtomicU32::new(constructing_process()),
            dir_identity: std::sync::OnceLock::new(),
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

    /// A test seam: make this handle behave as the copy a `fork` leaves in a child — its
    /// recorded owner becomes another process, so every later load, write, read, and
    /// sweep through it is refused with [`KeystoreError::InheritedHandle`] before it locks
    /// anything. Calling it again restores the owner. It can only take authority away
    /// while it is set. A real fork needs `unsafe`, which the workspace forbids, so this is
    /// how the fork-copy windows are tested.
    #[doc(hidden)]
    pub fn simulate_fork_copy(&self) {
        let _ = self.owner_process.fetch_update(
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
            |owner| (owner != 0).then_some(owner ^ 1),
        );
    }

    /// Install a deterministic fault seam for state writes (tests and fault campaigns).
    #[must_use]
    pub fn with_faults(mut self, faults: std::sync::Arc<dyn KeystoreFaults>) -> Self {
        self.faults = Some(Faults(faults));
        self
    }

    /// The directory.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Open the store, minting the local key on first use (plan §18.6). The mint, like
    /// every write, runs under the store's lock, and re-reads the store under it.
    ///
    /// First use — no state and no key file — mints through `SigningRegistry::mint` with
    /// `entropy`, records the mint as audit record 0 attributed to `actor`, and writes the
    /// key file, then the state. Every later call reads them back and draws no entropy. A
    /// store that exists in any other state is refused, never minted over.
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

    /// Open an existing store without minting or writing anything.
    ///
    /// # Errors
    ///
    /// [`KeystoreError::Absent`] when there is no state and no key file, otherwise as
    /// [`open_or_mint`](Self::open_or_mint).
    pub fn open(&self) -> Result<OpenedKeystore, KeystoreError> {
        #[cfg(unix)]
        {
            unix::open_shared(self)
        }
        #[cfg(not(unix))]
        {
            Err(KeystoreError::Unsupported)
        }
    }

    /// Record `state` as the store's state, atomically (see the module documentation).
    /// `minted` is the held key's seed when the change made that key: its file is written
    /// first. Otherwise the held key's file must already be in the store, so a state is
    /// never committed naming a key whose secret is not on disk. After the state is in
    /// place, key files no longer held are removed; a failure there leaves them for
    /// [`sweep`](Self::sweep).
    ///
    /// # Errors
    ///
    /// [`CustodyWrite::NotRecorded`] for any failure before the rename — the previous
    /// state is still what a restart loads — and [`CustodyWrite::Unconfirmed`] for a
    /// failure at or after it: the new state is in place and a restart may load it.
    pub fn persist(
        &self,
        state: &SigningCustodyState,
        minted: Option<Zeroizing<[u8; SEED_LEN]>>,
    ) -> Result<(), CustodyWrite<KeystoreError>> {
        #[cfg(unix)]
        {
            unix::persist(self, state, minted)
        }
        #[cfg(not(unix))]
        {
            let _ = (state, minted);
            Err(CustodyWrite::NotRecorded(KeystoreError::Unsupported))
        }
    }

    /// Remove every key file other than `held`'s, and a stale temporary state file: what
    /// a crash between a state change and its clean-up leaves. Returns how many were
    /// removed. Run by the launcher after a restart validated the state.
    ///
    /// # Errors
    ///
    /// [`KeystoreError`] when the directory fails its checks or a removal fails.
    pub fn sweep(&self, held: &SignerIdentity) -> Result<usize, KeystoreError> {
        #[cfg(unix)]
        {
            unix::sweep(self, held)
        }
        #[cfg(not(unix))]
        {
            let _ = held;
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

/// The keystore as the daemon's durable custody (bn-18w74): `continuumd`'s
/// `Builder::launch_signing` loads the authority through it, and the daemon records every
/// later change through it before the change takes effect.
impl SigningCustody for LocalKeystore {
    type Error = KeystoreError;

    fn load_or_mint(
        &mut self,
        actor: &ActorId,
        entropy: &mut dyn KeyEntropy,
    ) -> Result<(SigningCustodyState, LocalSigner), KeystoreError> {
        #[cfg(unix)]
        unix::owned_here(self)?;
        {
            let mut writer = self
                .writer
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if writer.is_none() {
                #[cfg(unix)]
                {
                    *writer = Some(Held {
                        _lock: unix::acquire(self)?,
                    });
                }
                #[cfg(not(unix))]
                {
                    return Err(KeystoreError::Unsupported);
                }
            }
        }
        self.open_or_mint(actor, entropy)
            .map(OpenedKeystore::into_custody)
    }

    fn persist(
        &mut self,
        state: &SigningCustodyState,
        minted: Option<Zeroizing<[u8; SEED_LEN]>>,
    ) -> Result<(), CustodyWrite<KeystoreError>> {
        Self::persist(self, state, minted)
    }

    fn sweep(&mut self, held: &SignerIdentity) -> Result<(), KeystoreError> {
        Self::sweep(self, held).map(|_| ())
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
        KeyEntropy, PUBLIC_KEY_LEN, SEED_LEN, SignerIdentity, SigningCustodyState, SigningRegistry,
        Zeroizing,
    };

    use super::Held;
    use super::{
        Capture, CustodyWrite, KEY_FILE_LABEL, KEY_FILE_LEN, KEY_MAGIC, KeystoreError,
        LEGACY_FILES, LOCK_FILE, LocalKeystore, MAX_STATE_FILE_LEN, OpenedKeystore, PENDING_FILE,
        PersistPhase, STATE_FILE, STATE_TEMP_FILE, TEMP_SERIAL, decode_state, encode_state, io,
        is_key_file_name, is_temp_name, key_file_name,
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

    /// The most symlinks one resolution of the store path follows (as the kernel's own
    /// `MAXSYMLINKS`).
    const MAX_SYMLINK_HOPS: usize = 40;

    /// Whether a component another user could change is trusted: owned by root or the
    /// expected owner, and — for a directory — not group- or world-writable unless it is
    /// sticky (in a sticky directory only an entry's owner, the directory's owner, or root
    /// can rename or remove the entry, and every entry this walk passes through is itself
    /// held to the same owners).
    pub(super) fn trusted_component(
        path: &Path,
        uid: u32,
        mode: u32,
        directory: bool,
        owner: u32,
    ) -> Result<(), KeystoreError> {
        if uid != 0 && uid != owner {
            return Err(KeystoreError::UntrustedAncestor {
                path: path.to_path_buf(),
                uid,
            });
        }
        let mode = mode & 0o7777;
        if directory && mode & GROUP_OR_OTHER_WRITE != 0 && mode & STICKY == 0 {
            return Err(KeystoreError::UnsafeParent {
                path: path.to_path_buf(),
                mode,
            });
        }
        Ok(())
    }

    /// The two rules a traversed symlink meets beyond [`trusted_component`] (review
    /// cr-2qu5zr round 2), both independent of `fs.protected_hardlinks`:
    ///
    /// 1. its containing directory is writable by root and the owner alone — sticky or not,
    ///    because a sticky bit guards renaming and removing an entry, not creating one, and a
    ///    hard link to one of the owner's own symlinks is a new entry that the owner rule
    ///    would otherwise trust;
    /// 2. the symlink has exactly one link: a hard link to a symlink raises its link
    ///    count, so a planted link is always visible.
    fn trusted_symlink(
        holder: &Path,
        link: &Path,
        metadata: &Metadata,
        owner: u32,
    ) -> Result<(), KeystoreError> {
        let holding = fs::symlink_metadata(holder).map_err(io("examining a keystore ancestor"))?;
        let mode = holding.mode() & 0o7777;
        if mode & GROUP_OR_OTHER_WRITE != 0 {
            return Err(KeystoreError::UnsafeParent {
                path: holder.to_path_buf(),
                mode,
            });
        }
        trusted_component(link, metadata.uid(), metadata.mode(), false, owner)?;
        if metadata.nlink() != 1 {
            return Err(KeystoreError::LinkedAncestor {
                path: link.to_path_buf(),
                links: metadata.nlink(),
            });
        }
        Ok(())
    }

    /// Resolve `path`'s parent one component at a time, following each symlink by hand,
    /// and hold every directory and every symlink the resolution passes through to
    /// [`trusted_component`] (review cr-2qu5zr, ssh `StrictModes` style). A symlink's
    /// target is resolved from the directory that holds it, so a symlink's containing
    /// directory is always checked, and a symlink reached through a symlink is checked as
    /// well. Only root and the expected owner can then change what the path resolves to,
    /// so resolving it again later — for a lock, an open, or a rename — reaches the same
    /// directory.
    ///
    /// Returns whether the walk reached the end: `false` when a component does not exist
    /// yet (nothing below it can be checked, and the caller must not trust what a later
    /// lookup finds there without walking again).
    fn trusted_ancestry(path: &Path, owner: u32) -> Result<bool, KeystoreError> {
        use std::collections::VecDeque;
        use std::path::Component;
        let absolute = std::path::absolute(path).map_err(io("resolving the keystore path"))?;
        let Some(parent) = absolute.parent() else {
            return Ok(true);
        };
        let mut pending: VecDeque<std::ffi::OsString> = VecDeque::new();
        for component in parent.components() {
            match component {
                Component::Normal(name) => pending.push_back(name.to_os_string()),
                Component::ParentDir => pending.push_back("..".into()),
                Component::RootDir | Component::CurDir | Component::Prefix(_) => {}
            }
        }
        let mut current = std::path::PathBuf::from("/");
        let root = fs::symlink_metadata(&current).map_err(io("examining a keystore ancestor"))?;
        trusted_component(&current, root.uid(), root.mode(), true, owner)?;
        let mut hops = 0usize;
        while let Some(name) = pending.pop_front() {
            if name == ".." {
                current.pop();
                continue;
            }
            let next = current.join(&name);
            let metadata = match fs::symlink_metadata(&next) {
                Ok(metadata) => metadata,
                // Nothing exists from here down: the first-use mint creates it, owned by
                // the owner and private, and the check after that creation covers it.
                Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
                Err(error) => return Err(io("examining a keystore ancestor")(error)),
            };
            if metadata.file_type().is_symlink() {
                trusted_symlink(&current, &next, &metadata, owner)?;
                hops += 1;
                if hops > MAX_SYMLINK_HOPS {
                    return Err(KeystoreError::Io {
                        op: "resolving the keystore path",
                        kind: ErrorKind::InvalidInput,
                    });
                }
                let target = fs::read_link(&next).map_err(io("reading a keystore ancestor"))?;
                let mut expanded: Vec<std::ffi::OsString> = Vec::new();
                if target.is_absolute() {
                    current = std::path::PathBuf::from("/");
                }
                for component in target.components() {
                    match component {
                        Component::Normal(part) => expanded.push(part.to_os_string()),
                        Component::ParentDir => expanded.push("..".into()),
                        Component::RootDir | Component::CurDir | Component::Prefix(_) => {}
                    }
                }
                for part in expanded.into_iter().rev() {
                    pending.push_front(part);
                }
                continue;
            }
            if !metadata.is_dir() {
                return Err(KeystoreError::NotADirectory);
            }
            trusted_component(&next, metadata.uid(), metadata.mode(), true, owner)?;
            current = next;
        }
        Ok(true)
    }

    /// The directory checks of the module documentation: the store directory itself, then
    /// its whole ancestry ([`trusted_ancestry`]). `None` when it does not exist; otherwise
    /// the directory's device and inode, which later checks compare against.
    fn check_dir(dir: &Path, owner: u32) -> Result<Option<(u64, u64)>, KeystoreError> {
        let complete = trusted_ancestry(dir, owner)?;
        let Some(mut metadata) = lstat(dir)? else {
            return Ok(None);
        };
        if !complete {
            // An ancestor was missing when the walk ran and exists now: something created
            // it in between. Walk again, all the way, before trusting what the path reaches
            // (review cr-2qu5zr adversarial pass), and look again through the path that
            // walk proved.
            if !trusted_ancestry(dir, owner)? {
                return Err(KeystoreError::StoreMoved);
            }
            metadata = lstat(dir)?.ok_or(KeystoreError::StoreMoved)?;
        }
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
        // Private, as the mint creates it: no group or other bits at all. Without search
        // permission no one else can name a file in it — not to read, and not to hard-link
        // a store file elsewhere, which would leave it refused (`HardLinked`) for good
        // (review cr-2qu5zr round 2 adversarial pass).
        if mode & GROUP_OR_OTHER != 0 {
            return Err(KeystoreError::UnsafeDirectory { mode });
        }
        Ok(Some((metadata.dev(), metadata.ino())))
    }

    /// [`check_dir`] for a store handle: the directory must also be the one this handle
    /// first found there. Run before every lock, open, write, rename, and removal, so an
    /// operation never reaches another directory than the one validated (review
    /// cr-2qu5zr). `None` when the store does not exist yet.
    pub(super) fn checked_dir(
        store: &LocalKeystore,
        owner: u32,
    ) -> Result<Option<()>, KeystoreError> {
        let Some(identity) = check_dir(&store.dir, owner)? else {
            return Ok(None);
        };
        let first = *store.dir_identity.get_or_init(|| identity);
        if first != identity {
            return Err(KeystoreError::StoreMoved);
        }
        Ok(Some(()))
    }

    /// [`checked_dir`] for an operation that needs the store to exist.
    fn recheck(store: &LocalKeystore, owner: u32) -> Result<(), KeystoreError> {
        checked_dir(store, owner)?.ok_or(KeystoreError::Absent)?;
        Ok(())
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
        if opened.nlink() != 1 {
            return Err(KeystoreError::HardLinked { file });
        }
        Ok(handle)
    }

    /// Take the store's exclusive lock, creating the directory (`0700`) and the lock file
    /// (`0600`) when absent. The lock file is examined on its descriptor like every other
    /// store file. [`KeystoreError::Locked`] when another handle holds it.
    pub(super) fn acquire(store: &LocalKeystore) -> Result<File, KeystoreError> {
        let owner = owner(store)?;
        if checked_dir(store, owner)?.is_none() {
            DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(&store.dir)
                .map_err(io("creating the keystore directory"))?;
            recheck(store, owner)?;
        }
        let path = store.dir.join(LOCK_FILE);
        if lstat(&path)?.is_none() {
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .custom_flags(O_NOFOLLOW)
                .open(&path)
            {
                Ok(_) => {}
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
                Err(error) => return Err(io("creating the keystore lock")(error)),
            }
        }
        let file = open_bound(&path, LOCK_FILE, owner)?;
        match file.try_lock() {
            Ok(()) => Ok(file),
            Err(std::fs::TryLockError::WouldBlock) => Err(KeystoreError::Locked),
            Err(std::fs::TryLockError::Error(error)) => Err(io("locking the keystore")(error)),
        }
    }

    /// What a write runs under: this handle's writer mutex, and the store's exclusive
    /// lock — the lifetime lock the mutex owns, or one taken for this write alone. Both
    /// are held until the guard drops, so no two writes, through this handle or any
    /// other, overlap (review cr-1dc5ii).
    pub(super) struct WriteGuard<'a> {
        _writer: std::sync::MutexGuard<'a, Option<Held>>,
        _transient: Option<File>,
    }

    /// The id of the calling process, from `/proc/self` (Linux and Android). Elsewhere the
    /// lifetime lock cannot tell an inherited handle from its owner, and is refused.
    pub(super) fn process_id() -> Result<u32, KeystoreError> {
        if cfg!(any(target_os = "linux", target_os = "android")) {
            return fs::read_link("/proc/self")
                .ok()
                .and_then(|link| link.to_str().and_then(|text| text.parse().ok()))
                .ok_or(KeystoreError::Unsupported);
        }
        Err(KeystoreError::Unsupported)
    }

    /// Refuse a handle whose lifetime lock another process took: a copy a `fork` made
    /// shares the lock's open file description and would pass it, so it is refused before
    /// it touches any path.
    ///
    /// Read from outside the mutex and checked before any lock is taken (review cr-1dc5ii
    /// round 3): a child forked while another thread held the writer mutex inherits it
    /// locked, with no thread to unlock it, and would otherwise block forever.
    pub(super) fn owned_here(store: &LocalKeystore) -> Result<(), KeystoreError> {
        let owner = store
            .owner_process
            .load(std::sync::atomic::Ordering::SeqCst);
        if owner == 0 {
            // Never read, so never owned: refused rather than treated as free.
            return Err(KeystoreError::Unsupported);
        }
        if process_id()? != owner {
            return Err(KeystoreError::InheritedHandle);
        }
        Ok(())
    }

    pub(super) fn guard(store: &LocalKeystore) -> Result<WriteGuard<'_>, KeystoreError> {
        owned_here(store)?;
        let writer = store
            .writer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let transient = match writer.as_ref() {
            Some(_) => None,
            None => Some(acquire(store)?),
        };
        Ok(WriteGuard {
            _writer: writer,
            _transient: transient,
        })
    }

    /// A read-only open from outside a write: under this handle's lifetime lock when it
    /// holds one, and otherwise under a shared lock on the store, which is refused
    /// ([`KeystoreError::Locked`]) while any other handle — in this process or another —
    /// holds the store (flock locks belong to the open file description, so a second
    /// handle in one process conflicts like another process). A store no handle has ever
    /// locked has no lock file, and is read as it is: every state it can hold is whole.
    pub(super) fn open_shared(store: &LocalKeystore) -> Result<OpenedKeystore, KeystoreError> {
        owned_here(store)?;
        let writer = store
            .writer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if writer.is_some() {
            return open(store);
        }
        let owner = owner(store)?;
        if checked_dir(store, owner)?.is_none() {
            return Err(KeystoreError::Absent);
        }
        let path = store.dir.join(LOCK_FILE);
        let _shared = match lstat(&path)? {
            None => None,
            Some(_) => {
                let file = open_bound(&path, LOCK_FILE, owner)?;
                match file.try_lock_shared() {
                    Ok(()) => Some(file),
                    Err(std::fs::TryLockError::WouldBlock) => return Err(KeystoreError::Locked),
                    Err(std::fs::TryLockError::Error(error)) => {
                        return Err(io("locking the keystore")(error));
                    }
                }
            }
        };
        open(store)
    }

    pub(super) fn open(store: &LocalKeystore) -> Result<OpenedKeystore, KeystoreError> {
        let owner = owner(store)?;
        if checked_dir(store, owner)?.is_none() {
            return Err(KeystoreError::Absent);
        }
        for legacy in LEGACY_FILES {
            if lstat(&store.dir.join(legacy))?.is_some() {
                return Err(KeystoreError::LegacyLayout);
            }
        }
        let state_path = store.dir.join(STATE_FILE);
        if lstat(&state_path)?.is_none() {
            return if has_key_file(&store.dir)? {
                Err(KeystoreError::Incomplete {
                    missing: STATE_FILE,
                })
            } else {
                Err(KeystoreError::Absent)
            };
        }
        let state = read_state(open_bound(&state_path, STATE_FILE, owner)?)?;
        let held = state
            .held()
            .cloned()
            .ok_or(KeystoreError::Corrupt("the state names no held key"))?;
        recheck(store, owner)?;
        let key = open_bound(&store.dir.join(key_file_name(&held)), KEY_FILE_LABEL, owner)?;
        #[cfg(test)]
        if let Some(hook) = store.after_open {
            hook(&store.dir);
        }

        let (seed, public_key) = read_key(key)?;
        if &public_key != held.public_key() {
            return Err(KeystoreError::Corrupt("the key file is not the held key's"));
        }
        let registry = state.registry();
        let signer = registry.restore(seed).map_err(KeystoreError::Restore)?;
        if signer.identity() != &held {
            return Err(KeystoreError::Corrupt(
                "the stored public key is not the one the stored seed derives",
            ));
        }
        Ok(OpenedKeystore {
            state,
            signer,
            minted: false,
            pending: lstat(&store.dir.join(PENDING_FILE))?.is_some(),
        })
    }

    /// Whether the directory holds any key file.
    fn has_key_file(dir: &Path) -> Result<bool, KeystoreError> {
        for entry in fs::read_dir(dir).map_err(io("listing the keystore"))? {
            let entry = entry.map_err(io("listing the keystore"))?;
            if entry.file_name().to_str().is_some_and(is_key_file_name) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(super) fn mint(
        store: &LocalKeystore,
        actor: &ActorId,
        entropy: &mut dyn KeyEntropy,
    ) -> Result<OpenedKeystore, KeystoreError> {
        let owner = owner(store)?;
        let _guard = guard(store)?;
        // Re-read under the lock: another handle may have minted since `open` looked.
        match open(store) {
            Err(KeystoreError::Absent) => {}
            other => return other,
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
        let identity = signer.identity().clone();
        let state = SigningCustodyState::first_use(registry, identity.clone());
        // Encoded before anything is written: a state this module could not read back is
        // never committed.
        let bytes = encode_state(&state)?;
        write_key(store, owner, &identity, seed)?;
        // First use answers no wire request, so its two failure classes are one refusal:
        // a launch that fails here is retried, and a restart loads whatever was committed.
        replace_state(store, owner, &bytes).map_err(|write| match write {
            CustodyWrite::NotRecorded(error) | CustodyWrite::Unconfirmed(error) => error,
        })?;
        Ok(OpenedKeystore {
            state,
            signer,
            minted: true,
            pending: false,
        })
    }

    pub(super) fn persist(
        store: &LocalKeystore,
        state: &SigningCustodyState,
        minted: Option<Zeroizing<[u8; SEED_LEN]>>,
    ) -> Result<(), CustodyWrite<KeystoreError>> {
        owned_here(store).map_err(CustodyWrite::NotRecorded)?;
        let _guard = prepare(store, state, minted).map_err(CustodyWrite::NotRecorded)?;
        let held = state
            .held()
            .ok_or(CustodyWrite::NotRecorded(KeystoreError::Corrupt(
                "the state names no held key",
            )))?;
        let bytes = encode_state(state).map_err(CustodyWrite::NotRecorded)?;
        let owner = owner(store).map_err(CustodyWrite::NotRecorded)?;
        replace_state(store, owner, &bytes)?;
        // The state that retires a key is in place: its file goes now, or at the next
        // launch's sweep if this fails.
        let _ = sweep_dir(store, owner, held);
        Ok(())
    }

    /// Everything a state write does before its commit point: the checks, the lock, and
    /// the new held key's file. Returns the lock guard the write holds.
    fn prepare<'a>(
        store: &'a LocalKeystore,
        state: &SigningCustodyState,
        minted: Option<Zeroizing<[u8; SEED_LEN]>>,
    ) -> Result<WriteGuard<'a>, KeystoreError> {
        let owner = owner(store)?;
        recheck(store, owner)?;
        let guard = guard(store)?;
        let held = state
            .held()
            .ok_or(KeystoreError::Corrupt("the state names no held key"))?;
        encode_state(state)?;
        match minted {
            Some(seed) => {
                // The seed must derive the key the state holds, or neither is written.
                let mut check = Zeroizing::new([0u8; SEED_LEN]);
                check.copy_from_slice(seed.as_ref());
                let derived = state
                    .registry()
                    .restore(check)
                    .map_err(KeystoreError::Restore)?;
                if derived.identity() != held {
                    return Err(KeystoreError::Corrupt(
                        "the minted seed does not derive the held key",
                    ));
                }
                drop(derived);
                write_key(store, owner, held, seed)?;
            }
            None => {
                // A state is never committed naming a held key whose secret is not in the
                // store, private, and the owner's.
                drop(open_bound(
                    &store.dir.join(key_file_name(held)),
                    KEY_FILE_LABEL,
                    owner,
                )?);
            }
        }
        Ok(guard)
    }

    pub(super) fn sweep(
        store: &LocalKeystore,
        held: &SignerIdentity,
    ) -> Result<usize, KeystoreError> {
        owned_here(store)?;
        let owner = owner(store)?;
        recheck(store, owner)?;
        let _guard = guard(store)?;
        sweep_dir(store, owner, held)
    }

    fn sweep_dir(
        store: &LocalKeystore,
        owner: u32,
        held: &SignerIdentity,
    ) -> Result<usize, KeystoreError> {
        let dir = store.dir.as_path();
        let keep = key_file_name(held);
        let mut removed = 0;
        recheck(store, owner)?;
        for entry in fs::read_dir(dir).map_err(io("listing the keystore"))? {
            let entry = entry.map_err(io("listing the keystore"))?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if (is_key_file_name(name) && name != keep)
                || is_temp_name(name)
                || name == PENDING_FILE
            {
                // `remove_file` unlinks the name; it never follows a link.
                recheck(store, owner)?;
                fs::remove_file(dir.join(name)).map_err(io("removing a retired key file"))?;
                removed += 1;
            }
        }
        if removed > 0 {
            sync_dir(dir)?;
        }
        Ok(removed)
    }

    fn sync_dir(dir: &Path) -> Result<(), KeystoreError> {
        File::open(dir)
            .and_then(|handle| handle.sync_all())
            .map_err(io("syncing the keystore directory"))
    }

    /// Write the key file of `identity` from `seed`, as a new private file.
    fn write_key(
        store: &LocalKeystore,
        owner: u32,
        identity: &SignerIdentity,
        seed: Zeroizing<[u8; SEED_LEN]>,
    ) -> Result<(), KeystoreError> {
        let dir = store.dir.as_path();
        let mut buffer = Zeroizing::new([0u8; KEY_FILE_LEN]);
        buffer[..KEY_MAGIC.len()].copy_from_slice(&KEY_MAGIC);
        buffer[KEY_MAGIC.len()..KEY_MAGIC.len() + SEED_LEN].copy_from_slice(seed.as_ref());
        buffer[KEY_MAGIC.len() + SEED_LEN..].copy_from_slice(identity.public_key());
        drop(seed);
        recheck(store, owner)?;
        write_new_private(&dir.join(key_file_name(identity)), buffer.as_ref())?;
        sync_dir(dir)
    }

    /// Replace the state file whole: a new private temporary file, synced, renamed over
    /// the state, and the directory synced. The rename is the commit point: a failure
    /// before it is [`CustodyWrite::NotRecorded`], one at or after it
    /// [`CustodyWrite::Unconfirmed`] (review cr-33e464).
    fn replace_state(
        store: &LocalKeystore,
        owner: u32,
        bytes: &[u8],
    ) -> Result<(), CustodyWrite<KeystoreError>> {
        let dir = store.dir.as_path();
        let faults = store.faults.as_ref();
        recheck(store, owner).map_err(CustodyWrite::NotRecorded)?;
        let injected = |phase| faults.is_some_and(|faults| faults.0.fail(phase));
        // A fresh temporary name per write, created with `O_EXCL`: defence in depth
        // beside the writer lock, so two writes can never share one temporary file
        // (review cr-1dc5ii).
        let temp = dir.join(format!(
            "{STATE_TEMP_FILE}-{}",
            TEMP_SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let state = dir.join(STATE_FILE);
        // The crash oracle's copy of the entry the rename replaces, taken only when a
        // fault seam is installed (it never runs in production).
        let previous = match faults {
            Some(_) => lstat(&state)
                .and_then(|found| {
                    found
                        .map(|_| fs::read(&state).map_err(io("reading the state file")))
                        .transpose()
                })
                .map_err(CustodyWrite::NotRecorded)?,
            None => None,
        };
        let mut marked_here = false;
        let staged = (|| {
            if lstat(&temp)?.is_some() {
                fs::remove_file(&temp).map_err(io("removing a stale temporary state file"))?;
            }
            write_new_private(&temp, bytes)?;
            // The ambiguity is made durable before the commit point: from the rename on, a
            // restart must reconcile until this write is confirmed.
            marked_here = mark_pending(store, owner)?;
            if injected(PersistPhase::BeforeRename) {
                return Err(KeystoreError::Io {
                    op: "replacing the state file",
                    kind: ErrorKind::Other,
                });
            }
            // The commit point, re-checked right before it: the directory the rename
            // resolves to is still the one validated.
            recheck(store, owner)?;
            fs::rename(&temp, &state).map_err(io("replacing the state file"))
        })();
        if let Err(error) = staged {
            // Nothing was committed by this write. Its own marker goes; one an earlier
            // unconfirmed write left stays, because that ambiguity is still unresolved. A
            // marker left by a failed removal only makes the next restart reconcile.
            // Cleanup only where the directory is still the one validated: a refusal of the
            // recheck itself removes nothing.
            if recheck(store, owner).is_ok() {
                if marked_here {
                    let _ = clear_pending(store, owner);
                }
                let _ = fs::remove_file(&temp);
            }
            return Err(CustodyWrite::NotRecorded(error));
        }
        let unconfirmed =
            if injected(PersistPhase::AfterRename) || injected(PersistPhase::DirectorySync) {
                Err(KeystoreError::Io {
                    op: "syncing the keystore directory",
                    kind: ErrorKind::Other,
                })
            } else {
                sync_dir(dir)
            };
        if let Err(error) = unconfirmed {
            // The crash oracle: a crash after an unconfirmed rename may keep either
            // directory entry. When the seam chooses the old one, put it back, as the
            // relaunch would find it.
            if faults.is_some_and(|faults| faults.0.lose_rename()) && recheck(store, owner).is_ok()
            {
                match &previous {
                    Some(old) => {
                        let _ = fs::remove_file(&temp);
                        if write_new_private(&temp, old).is_ok() {
                            let _ = fs::rename(&temp, &state);
                        }
                    }
                    None => {
                        let _ = fs::remove_file(&state);
                    }
                }
            }
            return Err(CustodyWrite::Unconfirmed(error));
        }
        // Confirmed: the ambiguity is gone. A marker a failed removal leaves only makes the
        // next restart reconcile.
        let _ = clear_pending(store, owner);
        Ok(())
    }

    /// Create the pending marker, durably, if it is not there. Whether this call created it.
    fn mark_pending(store: &LocalKeystore, owner: u32) -> Result<bool, KeystoreError> {
        let dir = store.dir.as_path();
        recheck(store, owner)?;
        if lstat(&dir.join(PENDING_FILE))?.is_some() {
            return Ok(false);
        }
        write_new_private(&dir.join(PENDING_FILE), &[])?;
        sync_dir(dir)?;
        Ok(true)
    }

    /// Remove the pending marker, durably, if it is there.
    fn clear_pending(store: &LocalKeystore, owner: u32) -> Result<(), KeystoreError> {
        let dir = store.dir.as_path();
        recheck(store, owner)?;
        if lstat(&dir.join(PENDING_FILE))?.is_some() {
            fs::remove_file(dir.join(PENDING_FILE)).map_err(io("clearing the pending marker"))?;
            sync_dir(dir)?;
        }
        Ok(())
    }

    /// `create_new` refuses an existing path, a symlink included, so nothing is ever
    /// written over; `O_NOFOLLOW` refuses a link where the platform constant is known.
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

    /// Read the state file through its bound descriptor: its length is checked on the
    /// descriptor first, and at most one byte past the bound is ever read.
    fn read_state(file: File) -> Result<SigningCustodyState, KeystoreError> {
        let len = file
            .metadata()
            .map_err(io("examining the state file"))?
            .len();
        if len > MAX_STATE_FILE_LEN {
            return Err(KeystoreError::Corrupt(
                "the state file exceeds its size bound",
            ));
        }
        let mut bytes = Vec::with_capacity(usize::try_from(len).unwrap_or(0));
        file.take(MAX_STATE_FILE_LEN + 1)
            .read_to_end(&mut bytes)
            .map_err(io("reading the state file"))?;
        decode_state(&bytes)
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
}

#[cfg(all(test, unix))]
mod tests {
    use std::fs::{self, OpenOptions};
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt, symlink};
    use std::path::{Path, PathBuf};

    use continuum_evidence::actor::ActorId;
    use continuum_evidence::signing::{
        EntropyUnavailable, KeyEntropy, SEED_LEN, SigningRegistry, Zeroizing,
    };

    use super::{
        KEY_FILE_LABEL, KeystoreError, LocalKeystore, MAX_AUDIT_RECORD_LEN, STATE_FILE,
        key_file_name, unix,
    };

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

    /// The attacker's store sits beside the victim's; the hook renames its state over the
    /// victim's, and its key file over the victim's key file, after `open` has bound its
    /// descriptors and before it reads the key.
    fn swap_in_attacker(dir: &Path) {
        let attacker = dir.with_file_name("attacker");
        fs::rename(attacker.join(STATE_FILE), dir.join(STATE_FILE)).expect("swap state");
        let victim_key = fs::read_dir(dir)
            .expect("victim")
            .map(|entry| entry.expect("entry").path())
            .find(|path| path.extension().is_some_and(|ext| ext == "key"))
            .expect("the victim's key file");
        let attacker_key = fs::read_dir(&attacker)
            .expect("attacker")
            .map(|entry| entry.expect("entry").path())
            .find(|path| path.extension().is_some_and(|ext| ext == "key"))
            .expect("the attacker's key file");
        fs::rename(attacker_key, victim_key).expect("swap key");
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
        let victim_key = victim_dir.join(key_file_name(victim.signer().identity()));
        let before = fs::read(&victim_key).expect("the victim's key");

        let mut store = LocalKeystore::new(&victim_dir);
        store.after_open = Some(swap_in_attacker);
        let opened = store.open().expect("the bound descriptors still read");
        assert_eq!(
            opened.signer().identity(),
            victim.signer().identity(),
            "a file renamed over the path after open is never read"
        );
        // Anti-vacuity: the swap happened, so the victim's key path now holds other bytes,
        // and a fresh open no longer yields the victim's key.
        assert_ne!(fs::read(&victim_key).expect("swapped"), before);
        assert!(LocalKeystore::new(&victim_dir).open().is_err());
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
            unix::open_bound(&link, KEY_FILE_LABEL, 0).map(|_| ()),
            Err(KeystoreError::SymlinkedFile {
                file: KEY_FILE_LABEL
            })
        );
    }

    /// The flags word of an open descriptor, from `/proc/self/fdinfo`.
    fn descriptor_flags(file: &std::fs::File) -> u32 {
        use std::os::fd::AsRawFd;
        let info =
            fs::read_to_string(format!("/proc/self/fdinfo/{}", file.as_raw_fd())).expect("fdinfo");
        let flags = info
            .lines()
            .find_map(|line| line.strip_prefix("flags:"))
            .expect("a flags line");
        u32::from_str_radix(flags.trim(), 8).expect("octal flags")
    }

    /// Every descriptor this module opens is close-on-exec, so a spawned process inherits
    /// neither the store's lock nor a store file (review cr-1dc5ii round 2): the lock file,
    /// a descriptor-bound read, and a file created as the temporary state is.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn every_store_descriptor_is_close_on_exec() {
        const O_CLOEXEC: u32 = 0o2_000_000;
        let root = scratch("cloexec");
        let dir = root.join("store");
        let store = LocalKeystore::new(&dir);
        store.open_or_mint(&actor(), &mut Fixed(5)).expect("mints");
        let owner = fs::metadata(&dir).expect("dir").uid();
        let lock = unix::acquire(&store).expect("locks");
        assert_ne!(descriptor_flags(&lock) & O_CLOEXEC, 0, "the lock");
        let bound = unix::open_bound(&dir.join(STATE_FILE), STATE_FILE, owner).expect("opens");
        assert_ne!(descriptor_flags(&bound) & O_CLOEXEC, 0, "a bound read");
        let created = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(unix::O_NOFOLLOW)
            .open(dir.join("probe"))
            .expect("creates");
        assert_ne!(descriptor_flags(&created) & O_CLOEXEC, 0, "a created file");
    }

    /// A handle whose lifetime lock another process took — what a `fork` leaves in the
    /// child — is refused before it takes a lock or writes or reads any store file, for a
    /// write and for a read.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn a_handle_owned_by_another_process_is_refused_before_it_touches_a_path() {
        use continuum_evidence::signing::SigningCustody;
        let root = scratch("inherited");
        let dir = root.join("store");
        let mut store = LocalKeystore::new(&dir);
        let (state, _) = store.load_or_mint(&actor(), &mut Fixed(6)).expect("loads");
        let before: Vec<_> = fs::read_dir(&dir)
            .expect("store")
            .map(|entry| entry.expect("entry").file_name())
            .collect();
        // What a fork leaves: the same handle, lock, and descriptor, in another process.
        store
            .owner_process
            .fetch_xor(1, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            store.persist(&state, None),
            Err(super::CustodyWrite::NotRecorded(
                KeystoreError::InheritedHandle
            ))
        );
        assert_eq!(
            store.open().map(|_| ()),
            Err(KeystoreError::InheritedHandle)
        );
        assert_eq!(
            store.sweep(state.held().expect("held")),
            Err(KeystoreError::InheritedHandle)
        );
        let after: Vec<_> = fs::read_dir(&dir)
            .expect("store")
            .map(|entry| entry.expect("entry").file_name())
            .collect();
        assert_eq!(before, after, "no path was touched");
        // Anti-vacuity: the owner itself still writes.
        store
            .owner_process
            .fetch_xor(1, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(store.persist(&state, None), Ok(()));
    }

    /// The trust rule for every component of the store's ancestry (review cr-2qu5zr): a
    /// symlink or directory another user owns is refused — the case of an attacker's link
    /// in a sticky shared directory, which a test cannot create without root — and a
    /// directory others can write is refused unless it is sticky.
    #[test]
    fn an_ancestor_another_user_owns_or_can_write_is_refused() {
        let path = Path::new("/some/ancestor");
        let me = 1000;
        assert_eq!(
            unix::trusted_component(path, 1001, 0o777, false, me),
            Err(KeystoreError::UntrustedAncestor {
                path: path.to_path_buf(),
                uid: 1001
            }),
            "an attacker's symlink"
        );
        assert_eq!(
            unix::trusted_component(path, 1001, 0o755, true, me),
            Err(KeystoreError::UntrustedAncestor {
                path: path.to_path_buf(),
                uid: 1001
            }),
            "an attacker's directory"
        );
        assert_eq!(
            unix::trusted_component(path, me, 0o770, true, me),
            Err(KeystoreError::UnsafeParent {
                path: path.to_path_buf(),
                mode: 0o770
            })
        );
        assert_eq!(
            unix::trusted_component(path, 0, 0o1777, true, me),
            Ok(()),
            "/tmp"
        );
        assert_eq!(
            unix::trusted_component(path, me, 0o777, false, me),
            Ok(()),
            "my link"
        );
        assert_eq!(unix::trusted_component(path, 0, 0o755, true, me), Ok(()));
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
