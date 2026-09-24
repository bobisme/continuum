//! The keystore tests that start child processes (bn-1hape, bn-18w74, review cr-1dc5ii).
//!
//! They live apart from `keystore.rs`, and are serialized here, because a child `Command`
//! starts is forked before it execs: in between it holds a copy of every descriptor of the
//! parent, among them a flock a concurrent test has just taken and is about to release, so
//! that test would see its own store as `Locked` for a moment. That window is real in any
//! process that spawns children while it uses a keystore (a typed, fail-closed `Locked`,
//! never a wrong answer); here it would only make the suite flaky.

#![cfg(unix)]

use std::fs;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use continuum_evidence::actor::ActorId;
use continuum_evidence::signing::{
    EntropyUnavailable, KeyEntropy, SEED_LEN, SigningCustody, Zeroizing,
};
use continuum_security::entropy::OsEntropy;
use continuum_security::keystore::{
    KeystoreError, LocalKeystore, MAX_AUDIT_RECORD_LEN, MAX_AUDIT_RECORDS, STATE_FILE, STATE_MAGIC,
};
use continuum_value::value::Value;

/// One test of this binary at a time: each starts a child process.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// A fresh directory under the build's temporary root, unique per test and process.
fn scratch(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "continuum-keystore-processes-{}-{name}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("scratch root");
    root.join("store")
}

fn actor() -> ActorId {
    ActorId::new("human:solo-dev").expect("actor")
}

/// The seed `[n; 32]`.
struct Fixed(u8);

impl KeyEntropy for Fixed {
    fn seed(&mut self) -> Result<Zeroizing<[u8; SEED_LEN]>, EntropyUnavailable> {
        Ok(Zeroizing::new([self.0; SEED_LEN]))
    }
}

/// Replace `path`'s bytes, keeping it private.
fn rewrite(path: &Path, bytes: &[u8]) {
    let _ = fs::remove_file(path);
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .expect("create");
    fs::write(path, bytes).expect("rewrite");
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).expect("private");
}

fn minted(name: &str) -> (PathBuf, LocalKeystore) {
    let dir = scratch(name);
    let store = LocalKeystore::new(&dir);
    store
        .open_or_mint(&actor(), &mut OsEntropy::new())
        .expect("mint");
    (dir, store)
}

fn minted_fixed(name: &str, seed: u8) -> (PathBuf, LocalKeystore) {
    let dir = scratch(name);
    let store = LocalKeystore::new(&dir);
    store
        .open_or_mint(&actor(), &mut Fixed(seed))
        .expect("mint");
    (dir, store)
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

/// A state file with the real header (magic and held key) and `audit` as its audit section.
fn hostile_state(dir: &Path, header: &[u8], audit: &[u8]) {
    let mut bytes = header.to_vec();
    bytes.extend_from_slice(audit);
    rewrite(&dir.join(STATE_FILE), &bytes);
}

fn amplification_body() {
    let (dir, store) = minted("amplification");
    let original = fs::read(dir.join(STATE_FILE)).expect("state");
    let header = &original[..STATE_MAGIC.len() + 32];
    // 1. The largest audit section the format admits: every record at the size bound, each
    //    an amplifying sequence of nulls. Refused at the first record's shape, with bounded
    //    memory.
    let record = amplifying_record();
    let mut log = MAX_AUDIT_RECORDS.to_be_bytes().to_vec();
    for _ in 0..MAX_AUDIT_RECORDS {
        log.extend_from_slice(&(record.len() as u32).to_be_bytes());
        log.extend_from_slice(&record);
    }
    hostile_state(&dir, header, &log);
    assert!(matches!(store.open(), Err(KeystoreError::Replay(_))));

    // 2. A section claiming four billion records, and a record claiming 4 GiB: refused
    //    before any allocation.
    hostile_state(&dir, header, &u32::MAX.to_be_bytes());
    assert!(matches!(store.open(), Err(KeystoreError::Corrupt(_))));
    let mut huge = 1u32.to_be_bytes().to_vec();
    huge.extend_from_slice(&u32::MAX.to_be_bytes());
    hostile_state(&dir, header, &huge);
    assert!(matches!(store.open(), Err(KeystoreError::Corrupt(_))));

    // 3. 16 MiB that would decode as one sequence of nulls: the format has no whole-log
    //    value to decode, and the file is over its size bound.
    // Anti-vacuity: materializing that value would itself exceed the child's limit.
    assert!(std::mem::size_of::<Value>() * 16 * 1024 * 1024 > MEMORY_LIMIT_KIB * 1024);
    let mut whole = vec![0u8; 16 * 1024 * 1024];
    whole[..record.len()].copy_from_slice(&record);
    hostile_state(&dir, header, &whole);
    assert!(matches!(store.open(), Err(KeystoreError::Corrupt(_))));
}

#[test]
fn a_hostile_audit_log_is_refused_within_a_memory_limit() {
    let _serial = serial();
    under_memory_limit(
        "a_hostile_audit_log_is_refused_within_a_memory_limit",
        amplification_body,
    );
}

const SPAWN_CHILD: &str = "CONTINUUM_KEYSTORE_SPAWN_CHILD_STORE";

/// A real child process started while this process holds the store (review cr-1dc5ii
/// round 2): it inherits no keystore descriptor, so it holds neither the lock nor any
/// store file, and its own attempt to take the store is refused as `Locked`, like any
/// other process. Run as the child under `SPAWN_CHILD`.
#[cfg(target_os = "linux")]
#[test]
fn a_spawned_process_inherits_nothing_and_is_refused_the_store() {
    let _serial = serial();
    if let Some(store) = std::env::var_os(SPAWN_CHILD) {
        let dir = fs::canonicalize(PathBuf::from(store)).expect("the store");
        for entry in fs::read_dir("/proc/self/fd").expect("fd table") {
            let target = fs::read_link(entry.expect("fd").path()).unwrap_or_default();
            assert!(
                !target.starts_with(&dir),
                "the child inherited a store descriptor: {}",
                target.display()
            );
        }
        let mut other = LocalKeystore::new(&dir);
        assert_eq!(
            SigningCustody::load_or_mint(&mut other, &actor(), &mut Fixed(96)).map(|_| ()),
            Err(KeystoreError::Locked)
        );
        // Proof the child ran these checks: the parent requires this file.
        fs::write(dir.with_file_name("child-ran"), b"ok").expect("sentinel");
        return;
    }
    let (dir, _) = minted_fixed("spawn", 95);
    let dir = fs::canonicalize(&dir).expect("the store");
    let sentinel = dir.with_file_name("child-ran");
    let _ = fs::remove_file(&sentinel);
    let mut holder = LocalKeystore::new(&dir);
    SigningCustody::load_or_mint(&mut holder, &actor(), &mut Fixed(97)).expect("loads");
    let status = std::process::Command::new(std::env::current_exe().expect("the test binary"))
        .args([
            "--exact",
            "a_spawned_process_inherits_nothing_and_is_refused_the_store",
            "--test-threads",
            "1",
        ])
        .env(SPAWN_CHILD, &dir)
        .status()
        .expect("spawn the child");
    assert!(
        status.success(),
        "the child inherited the store or was not refused"
    );
    assert!(
        sentinel.exists(),
        "the child ran its checks (not an empty test filter)"
    );
    drop(holder);
}
