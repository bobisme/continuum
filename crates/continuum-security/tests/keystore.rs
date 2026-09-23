//! The on-disk keystore (bn-1hape, plan §18.6, ADR-0054): round trip, first-use mint,
//! permissions, and typed refusal of a corrupt, incomplete, or exposed store.

#![cfg(unix)]

use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
use std::path::PathBuf;

use continuum_evidence::actor::ActorId;
use continuum_evidence::signing::{
    AllowedSigners, EntropyUnavailable, KeyEntropy, SEED_LEN, SignatureVerifier,
    SignedArtifactKind, SigningRegistry, Zeroizing,
};
use continuum_security::entropy::OsEntropy;
use continuum_security::keystore::{
    AUDIT_FILE, AUDIT_MAGIC, KEY_FILE, KEY_FILE_LEN, KeystoreError, LocalKeystore,
    MAX_AUDIT_RECORD_LEN, MAX_AUDIT_RECORDS, encode_audit_log,
};
use continuum_value::identity::ContentIdentity;
use continuum_value::value::Value;

/// A fresh directory under the build's temporary root, unique per test.
fn scratch(name: &str) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("continuum-keystore-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("scratch root");
    root.join("store")
}

fn actor() -> ActorId {
    ActorId::new("human:solo-dev").expect("actor")
}

/// Counts draws, so a test can see that a reopened store draws no entropy.
struct Counting<E> {
    inner: E,
    draws: usize,
}

impl<E: KeyEntropy> KeyEntropy for Counting<E> {
    fn seed(&mut self) -> Result<Zeroizing<[u8; SEED_LEN]>, EntropyUnavailable> {
        self.draws += 1;
        self.inner.seed()
    }
}

fn mode(path: &std::path::Path) -> u32 {
    fs::metadata(path).expect("metadata").permissions().mode() & 0o7777
}

#[test]
fn first_use_mints_once_with_os_entropy_and_reopening_restores_the_same_key() {
    let dir = scratch("round-trip");
    let store = LocalKeystore::new(&dir);
    let mut entropy = Counting {
        inner: OsEntropy::new(),
        draws: 0,
    };
    let first = store
        .open_or_mint(&actor(), &mut entropy)
        .expect("first use mints");
    assert!(first.minted());
    assert_eq!(first.registry().audit_log().len(), 1, "the mint is audited");
    let identity = first.signer().identity().clone();

    let second = store.open_or_mint(&actor(), &mut entropy).expect("reopens");
    assert!(!second.minted());
    assert_eq!(second.signer().identity(), &identity);
    assert_eq!(second.registry(), first.registry());
    assert_eq!(entropy.draws, 1, "a reopened store draws no entropy");

    // The restored key signs, and its signature verifies against the replayed registry.
    let artifact = ContentIdentity::of(&Value::text("keystore round trip"));
    let signature = second
        .registry()
        .sign(second.signer(), SignedArtifactKind::Receipt, &artifact)
        .expect("the restored key is active");
    let allowed = AllowedSigners::new().allow(identity, [SignedArtifactKind::Receipt]);
    let head = second.registry().head();
    let verifier = SignatureVerifier::new(&allowed, second.registry(), &head);
    assert!(
        verifier
            .verify(SignedArtifactKind::Receipt, &artifact, Some(&signature))
            .verified()
            .is_some()
    );
}

#[test]
fn the_store_is_private_to_its_owner() {
    let dir = scratch("permissions");
    LocalKeystore::new(&dir)
        .open_or_mint(&actor(), &mut OsEntropy::new())
        .expect("mint");
    assert_eq!(mode(&dir), 0o700);
    assert_eq!(mode(&dir.join(KEY_FILE)), 0o600);
    assert_eq!(mode(&dir.join(AUDIT_FILE)), 0o600);
    assert_eq!(
        fs::metadata(dir.join(KEY_FILE)).expect("key").len(),
        KEY_FILE_LEN as u64
    );
}

#[test]
fn an_exposed_key_file_is_refused() {
    let dir = scratch("exposed");
    let store = LocalKeystore::new(&dir);
    store
        .open_or_mint(&actor(), &mut OsEntropy::new())
        .expect("mint");
    fs::set_permissions(dir.join(KEY_FILE), fs::Permissions::from_mode(0o640)).expect("chmod");
    assert_eq!(
        store.open().map(|_| ()),
        Err(KeystoreError::InsecurePermissions {
            file: KEY_FILE,
            mode: 0o640
        })
    );
}

#[test]
fn a_corrupt_key_file_is_refused_and_never_re_minted() {
    let dir = scratch("corrupt");
    let store = LocalKeystore::new(&dir);
    store
        .open_or_mint(&actor(), &mut OsEntropy::new())
        .expect("mint");
    let key = dir.join(KEY_FILE);
    let original = fs::read(&key).expect("key");

    let rewrite = |bytes: &[u8]| {
        fs::set_permissions(&key, fs::Permissions::from_mode(0o600)).expect("chmod");
        fs::write(&key, bytes).expect("rewrite");
    };
    rewrite(&original[..original.len() - 1]);
    assert!(matches!(
        store.open_or_mint(&actor(), &mut OsEntropy::new()),
        Err(KeystoreError::Corrupt(_))
    ));
    let mut wrong_magic = original.clone();
    wrong_magic[0] ^= 0xff;
    rewrite(&wrong_magic);
    assert!(matches!(store.open(), Err(KeystoreError::Corrupt(_))));
    let mut wrong_public = original.clone();
    let last = wrong_public.len() - 1;
    wrong_public[last] ^= 0x01;
    rewrite(&wrong_public);
    assert!(matches!(store.open(), Err(KeystoreError::Corrupt(_))));
    let mut other_seed = original.clone();
    other_seed[8] ^= 0x01;
    rewrite(&other_seed);
    assert!(matches!(
        store.open(),
        Err(KeystoreError::Restore(_) | KeystoreError::Corrupt(_))
    ));
}

#[test]
fn a_missing_or_corrupt_audit_log_is_refused() {
    let dir = scratch("audit");
    let store = LocalKeystore::new(&dir);
    store
        .open_or_mint(&actor(), &mut OsEntropy::new())
        .expect("mint");
    let audit = dir.join(AUDIT_FILE);
    let original = fs::read(&audit).expect("audit");

    fs::write(&audit, b"not a canonical value").expect("rewrite");
    assert!(matches!(store.open(), Err(KeystoreError::Corrupt(_))));
    fs::write(
        &audit,
        encode_audit_log(&SigningRegistry::new()).expect("an empty log encodes"),
    )
    .expect("rewrite");
    assert!(matches!(store.open(), Err(KeystoreError::Restore(_))));
    fs::write(&audit, &original).expect("restore");
    assert!(store.open().is_ok());

    fs::remove_file(&audit).expect("remove");
    assert_eq!(
        store
            .open_or_mint(&actor(), &mut OsEntropy::new())
            .map(|_| ()),
        Err(KeystoreError::Incomplete {
            missing: AUDIT_FILE
        }),
        "an incomplete store is refused, never silently re-minted"
    );
}

#[test]
fn an_absent_store_opens_as_absent_and_a_failed_source_mints_nothing() {
    struct Dead;
    impl KeyEntropy for Dead {
        fn seed(&mut self) -> Result<Zeroizing<[u8; SEED_LEN]>, EntropyUnavailable> {
            Err(EntropyUnavailable)
        }
    }
    let dir = scratch("absent");
    let store = LocalKeystore::new(&dir);
    assert_eq!(store.open().map(|_| ()), Err(KeystoreError::Absent));
    assert!(matches!(
        store.open_or_mint(&actor(), &mut Dead),
        Err(KeystoreError::Mint(_))
    ));
    assert!(!dir.join(KEY_FILE).exists());
    assert!(!dir.join(AUDIT_FILE).exists());
}

// --- cr-3l3n47: directory trust, symlinks, ownership ------------------------------------

fn minted(name: &str) -> (PathBuf, LocalKeystore) {
    let dir = scratch(name);
    let store = LocalKeystore::new(&dir);
    store
        .open_or_mint(&actor(), &mut OsEntropy::new())
        .expect("mint");
    (dir, store)
}

#[test]
fn a_group_or_world_writable_store_directory_is_refused_and_not_re_minted() {
    let (dir, store) = minted("writable-dir");
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o770)).expect("chmod");
    assert_eq!(
        store.open().map(|_| ()),
        Err(KeystoreError::UnsafeDirectory { mode: 0o770 })
    );
    assert_eq!(
        store
            .open_or_mint(&actor(), &mut OsEntropy::new())
            .map(|_| ()),
        Err(KeystoreError::UnsafeDirectory { mode: 0o770 }),
        "a writable store is refused, never repaired or re-minted"
    );
    assert_eq!(
        mode(&dir),
        0o770,
        "an existing directory is never tightened"
    );
}

#[test]
fn a_store_owned_by_another_user_is_refused() {
    let (dir, _) = minted("foreign");
    let uid = fs::metadata(&dir).expect("metadata").uid();
    let foreign = LocalKeystore::new(&dir).expecting_owner(uid + 1);
    assert_eq!(
        foreign.open().map(|_| ()),
        Err(KeystoreError::ForeignOwner {
            what: "the keystore directory",
            uid,
            expected: uid + 1
        })
    );
    // The same seam, honestly named, opens the store its owner holds.
    assert!(LocalKeystore::new(&dir).expecting_owner(uid).open().is_ok());
}

#[test]
fn a_symlinked_key_file_is_refused() {
    let (dir, store) = minted("symlinked-key");
    let elsewhere = dir.with_file_name("elsewhere.key");
    fs::rename(dir.join(KEY_FILE), &elsewhere).expect("move");
    symlink(&elsewhere, dir.join(KEY_FILE)).expect("symlink");
    assert_eq!(
        store.open().map(|_| ()),
        Err(KeystoreError::SymlinkedFile { file: KEY_FILE })
    );
}

#[test]
fn a_symlinked_store_directory_is_refused() {
    let (dir, _) = minted("symlinked-dir");
    let link = dir.with_file_name("link-to-store");
    let _ = fs::remove_file(&link);
    symlink(&dir, &link).expect("symlink");
    assert_eq!(
        LocalKeystore::new(&link).open().map(|_| ()),
        Err(KeystoreError::SymlinkedStore)
    );
    assert_eq!(
        LocalKeystore::new(&link)
            .open_or_mint(&actor(), &mut OsEntropy::new())
            .map(|_| ()),
        Err(KeystoreError::SymlinkedStore)
    );
}

#[test]
fn a_world_writable_non_sticky_ancestor_is_refused() {
    let root = scratch("writable-parent");
    let parent = root.join("shared");
    fs::create_dir_all(&parent).expect("parent");
    let store = LocalKeystore::new(parent.join("store"));
    store
        .open_or_mint(&actor(), &mut OsEntropy::new())
        .expect("mint");
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o777)).expect("chmod");
    assert!(matches!(
        store.open(),
        Err(KeystoreError::UnsafeParent { mode: 0o777, .. })
    ));
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o1777)).expect("sticky");
    assert!(
        store.open().is_ok(),
        "a sticky shared ancestor, like /tmp, is allowed"
    );
}

// --- cr-3l3n47: bounded replay ----------------------------------------------------------

const MEMORY_CHILD: &str = "CONTINUUM_KEYSTORE_MEMORY_CHILD";

/// Address-space limit for the child: far below what a 16 MiB log of one-byte values
/// would take to materialize (hundreds of MiB), well above what bounded replay and the
/// test harness use.
const MEMORY_LIMIT_KIB: usize = 256 * 1024;

/// Run `body` in a child process under `ulimit -v`, the device
/// `continuum-cml-elab/tests/resource_bounds.rs` uses, so an unbounded allocation fails
/// that process rather than the host.
fn under_memory_limit(test: &str, body: fn()) {
    if std::env::var_os(MEMORY_CHILD).is_some() || !cfg!(target_os = "linux") {
        body();
        return;
    }
    let exe = std::env::current_exe().expect("the test binary");
    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!(
            "ulimit -v {MEMORY_LIMIT_KIB} && exec \"$0\" --exact {test} --test-threads 1"
        ))
        .arg(exe)
        .env(MEMORY_CHILD, "1")
        .status()
        .expect("spawn the child");
    assert!(
        status.success(),
        "{test} failed or exceeded {MEMORY_LIMIT_KIB} KiB of address space"
    );
}

/// One canonical record-sized value that amplifies as far as a record can: a sequence of
/// nulls filling [`MAX_AUDIT_RECORD_LEN`].
fn amplifying_record() -> Vec<u8> {
    let mut n = 1;
    loop {
        let next = Value::seq(std::iter::repeat_n(Value::Null, n + 1))
            .expect("seq")
            .encode();
        if next.len() > MAX_AUDIT_RECORD_LEN as usize {
            return Value::seq(std::iter::repeat_n(Value::Null, n))
                .expect("seq")
                .encode();
        }
        n += 1;
    }
}

fn hostile_log(dir: &std::path::Path, bytes: &[u8]) {
    let audit = dir.join(AUDIT_FILE);
    fs::write(&audit, bytes).expect("write the hostile log");
    fs::set_permissions(&audit, fs::Permissions::from_mode(0o600)).expect("chmod");
}

fn amplification_body() {
    let (dir, store) = minted("amplification");
    // 1. The largest file the format admits: every record at the size bound, each an
    //    amplifying sequence of nulls. Refused at the first record's shape, with bounded
    //    memory.
    let record = amplifying_record();
    let mut log = Vec::from(AUDIT_MAGIC);
    log.extend_from_slice(&MAX_AUDIT_RECORDS.to_be_bytes());
    for _ in 0..MAX_AUDIT_RECORDS {
        log.extend_from_slice(&(record.len() as u32).to_be_bytes());
        log.extend_from_slice(&record);
    }
    hostile_log(&dir, &log);
    assert!(matches!(store.open(), Err(KeystoreError::Replay(_))));

    // 2. A header claiming four billion records, and a record claiming 4 GiB: refused
    //    before any allocation.
    let mut claims = Vec::from(AUDIT_MAGIC);
    claims.extend_from_slice(&u32::MAX.to_be_bytes());
    hostile_log(&dir, &claims);
    assert!(matches!(store.open(), Err(KeystoreError::Corrupt(_))));
    let mut huge = Vec::from(AUDIT_MAGIC);
    huge.extend_from_slice(&1u32.to_be_bytes());
    huge.extend_from_slice(&u32::MAX.to_be_bytes());
    hostile_log(&dir, &huge);
    assert!(matches!(store.open(), Err(KeystoreError::Corrupt(_))));

    // 3. The pre-cr-3l3n47 attack: 16 MiB that would decode as one sequence of nulls. The
    //    format has no whole-log value to decode; it is refused at the header.
    // Anti-vacuity: materializing that value would itself exceed the child's limit.
    assert!(std::mem::size_of::<Value>() * 16 * 1024 * 1024 > MEMORY_LIMIT_KIB * 1024);
    let mut whole = vec![0u8; 16 * 1024 * 1024];
    whole[..record.len()].copy_from_slice(&record);
    hostile_log(&dir, &whole);
    assert!(matches!(store.open(), Err(KeystoreError::Corrupt(_))));
}

#[test]
fn a_hostile_audit_log_is_refused_within_a_memory_limit() {
    under_memory_limit(
        "a_hostile_audit_log_is_refused_within_a_memory_limit",
        amplification_body,
    );
}
