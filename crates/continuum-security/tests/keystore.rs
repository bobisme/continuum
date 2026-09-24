//! The on-disk keystore (bn-1hape, bn-18w74, plan §18.6, ADR-0054): round trip, first-use
//! mint, atomic persistence of later changes, permissions, and typed refusal of a corrupt,
//! truncated, oversize, incomplete, legacy, or exposed store.
//!
//! No test in this binary starts a child process. A child `Command` starts is forked
//! before it execs, and in between it holds a copy of every descriptor of this process —
//! among them a flock another test here has just taken and is about to release — so a
//! concurrent test would see that store as `Locked` for a moment. The tests that start
//! children live in `keystore_processes.rs`, and are serialized there.

#![cfg(unix)]

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt, symlink};
use std::path::{Path, PathBuf};

use continuum_evidence::actor::ActorId;
use continuum_evidence::signing::{
    AllowedSigners, EntropyUnavailable, KeyEntropy, SEED_LEN, SignatureVerifier,
    SignedArtifactKind, SigningCustody, SigningCustodyState, SigningRegistry, Zeroizing,
};
use continuum_security::entropy::OsEntropy;
use continuum_security::keystore::{
    CustodyWrite, KEY_FILE_LABEL, KEY_FILE_LEN, KeystoreError, KeystoreFaults, LEGACY_FILES,
    LOCK_FILE, LocalKeystore, MAX_STATE_FILE_LEN, PersistPhase, STATE_FILE, STATE_TEMP_FILE,
    decode_state, encode_state, key_file_name,
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

/// The seed `[n; 32]`: known to the test, so it can look for the seed in every output.
struct Fixed(u8);

impl KeyEntropy for Fixed {
    fn seed(&mut self) -> Result<Zeroizing<[u8; SEED_LEN]>, EntropyUnavailable> {
        Ok(Zeroizing::new([self.0; SEED_LEN]))
    }
}

fn mode(path: &Path) -> u32 {
    fs::metadata(path).expect("metadata").permissions().mode() & 0o7777
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
}

/// Every file in the store but the (empty) lock file, by name, with its bytes.
fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fs::read_dir(dir)
        .expect("store")
        .filter(|entry| {
            entry
                .as_ref()
                .map_or(true, |entry| entry.file_name() != LOCK_FILE)
        })
        .map(|entry| {
            let entry = entry.expect("entry");
            (
                entry.file_name().to_string_lossy().into_owned(),
                fs::read(entry.path()).expect("read"),
            )
        })
        .collect()
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

fn key_path(dir: &Path, store: &LocalKeystore) -> PathBuf {
    let opened = store.open().expect("opens");
    dir.join(key_file_name(opened.signer().identity()))
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
    assert_eq!(
        first.state(),
        &SigningCustodyState::first_use(first.registry().clone(), identity.clone()),
        "first use: the minted key is held, own, allowed every kind, and linked to nothing"
    );

    let second = store.open_or_mint(&actor(), &mut entropy).expect("reopens");
    assert!(!second.minted());
    assert_eq!(second.signer().identity(), &identity);
    assert_eq!(second.state(), first.state());
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
    let (dir, store) = minted("permissions");
    let key = key_path(&dir, &store);
    assert_eq!(mode(&dir), 0o700);
    assert_eq!(mode(&key), 0o600);
    assert_eq!(mode(&dir.join(STATE_FILE)), 0o600);
    assert_eq!(fs::metadata(&key).expect("key").len(), KEY_FILE_LEN as u64);
    assert_eq!(
        snapshot(&dir).len(),
        2,
        "the state and the held key's file, nothing else"
    );
}

#[test]
fn an_exposed_key_or_state_file_is_refused() {
    let (dir, store) = minted("exposed");
    let key = key_path(&dir, &store);
    fs::set_permissions(&key, fs::Permissions::from_mode(0o640)).expect("chmod");
    assert_eq!(
        store.open().map(|_| ()),
        Err(KeystoreError::InsecurePermissions {
            file: KEY_FILE_LABEL,
            mode: 0o640
        })
    );
    fs::set_permissions(&key, fs::Permissions::from_mode(0o600)).expect("chmod");
    fs::set_permissions(dir.join(STATE_FILE), fs::Permissions::from_mode(0o604)).expect("chmod");
    assert_eq!(
        store
            .open_or_mint(&actor(), &mut OsEntropy::new())
            .map(|_| ()),
        Err(KeystoreError::InsecurePermissions {
            file: STATE_FILE,
            mode: 0o604
        }),
        "a world-readable state is refused, never minted over"
    );
}

#[test]
fn a_corrupt_key_file_is_refused_and_never_re_minted() {
    let (dir, store) = minted("corrupt");
    let key = key_path(&dir, &store);
    let original = fs::read(&key).expect("key");

    rewrite(&key, &original[..original.len() - 1]);
    assert!(matches!(
        store.open_or_mint(&actor(), &mut OsEntropy::new()),
        Err(KeystoreError::Corrupt(_))
    ));
    let mut wrong_magic = original.clone();
    wrong_magic[0] ^= 0xff;
    rewrite(&key, &wrong_magic);
    assert!(matches!(store.open(), Err(KeystoreError::Corrupt(_))));
    let mut wrong_public = original.clone();
    let last = wrong_public.len() - 1;
    wrong_public[last] ^= 0x01;
    rewrite(&key, &wrong_public);
    assert!(matches!(store.open(), Err(KeystoreError::Corrupt(_))));
    let mut other_seed = original.clone();
    other_seed[8] ^= 0x01;
    rewrite(&key, &other_seed);
    assert!(matches!(
        store.open(),
        Err(KeystoreError::Restore(_) | KeystoreError::Corrupt(_))
    ));
    rewrite(&key, &original);
    assert!(store.open().is_ok(), "the original bytes open again");
}

/// Every proper prefix of a real state file, the file with one trailing byte, and garbage
/// each fail closed with a typed refusal, and none is ever minted over.
#[test]
fn a_truncated_extended_or_garbage_state_file_fails_closed() {
    let (dir, store) = minted_fixed("truncated", 7);
    let state = dir.join(STATE_FILE);
    let original = fs::read(&state).expect("state");
    let before = snapshot(&dir);
    for len in 0..original.len() {
        rewrite(&state, &original[..len]);
        let refused = store.open_or_mint(&actor(), &mut Fixed(8));
        assert!(
            matches!(
                refused,
                Err(KeystoreError::Corrupt(_) | KeystoreError::Replay(_))
            ),
            "a {len}-byte prefix of {} was not refused as corrupt: {:?}",
            original.len(),
            refused.map(|_| ())
        );
    }
    let mut extended = original.clone();
    extended.push(0);
    rewrite(&state, &extended);
    assert_eq!(
        store.open().map(|_| ()),
        Err(KeystoreError::Corrupt("the state file has trailing bytes"))
    );
    rewrite(&state, b"not a state file");
    assert!(matches!(store.open(), Err(KeystoreError::Corrupt(_))));
    rewrite(&state, &original);
    assert_eq!(
        snapshot(&dir),
        before,
        "no refusal minted, repaired, or removed anything"
    );
    assert!(store.open().is_ok());
}

/// An oversize state file is refused from its descriptor's length, before a byte is read.
/// The file is sparse, so the test allocates nothing of its size.
#[test]
fn an_oversize_state_file_is_refused_before_it_is_read() {
    let (dir, store) = minted("oversize");
    let state = dir.join(STATE_FILE);
    let file = fs::OpenOptions::new()
        .write(true)
        .open(&state)
        .expect("open");
    file.set_len(MAX_STATE_FILE_LEN + 1).expect("grow");
    drop(file);
    assert_eq!(
        store.open().map(|_| ()),
        Err(KeystoreError::Corrupt(
            "the state file exceeds its size bound"
        ))
    );
    assert_eq!(
        decode_state(&vec![
            0u8;
            usize::try_from(MAX_STATE_FILE_LEN).expect("fits") + 1
        ])
        .map(|_| ()),
        Err(KeystoreError::Corrupt(
            "the state file exceeds its size bound"
        ))
    );
}

#[test]
fn a_missing_state_or_key_file_is_refused_and_never_re_minted() {
    let (dir, store) = minted("incomplete");
    let key = key_path(&dir, &store);
    let state = dir.join(STATE_FILE);
    let state_bytes = fs::read(&state).expect("state");

    fs::remove_file(&state).expect("remove");
    assert_eq!(
        store
            .open_or_mint(&actor(), &mut OsEntropy::new())
            .map(|_| ()),
        Err(KeystoreError::Incomplete {
            missing: STATE_FILE
        }),
        "a key file with no state is refused, never silently re-minted"
    );
    rewrite(&state, &state_bytes);
    fs::remove_file(&key).expect("remove");
    assert_eq!(
        store
            .open_or_mint(&actor(), &mut OsEntropy::new())
            .map(|_| ()),
        Err(KeystoreError::Incomplete {
            missing: KEY_FILE_LABEL
        })
    );
    assert_eq!(snapshot(&dir).len(), 1, "nothing was minted");
}

#[test]
fn a_store_of_the_earlier_layout_is_refused_and_never_minted_over() {
    for legacy in LEGACY_FILES {
        let dir = scratch(&format!("legacy-{legacy}"));
        fs::create_dir_all(&dir).expect("dir");
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).expect("chmod");
        rewrite(&dir.join(legacy), b"earlier layout");
        let store = LocalKeystore::new(&dir);
        assert_eq!(
            store
                .open_or_mint(&actor(), &mut OsEntropy::new())
                .map(|_| ()),
            Err(KeystoreError::LegacyLayout)
        );
        assert_eq!(snapshot(&dir).len(), 1, "nothing was minted");
    }
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
    assert!(snapshot(&dir).is_empty(), "nothing was written");
}

// --- bn-18w74: persisting later changes ------------------------------------------------

/// A rotation, persisted with the successor's seed: the state round-trips exactly, the
/// successor's key is the one read back, the retired key's file is gone, and no temporary
/// file is left.
#[test]
fn a_persisted_rotation_round_trips_and_removes_the_retired_key() {
    let (dir, mut store) = minted_fixed("rotation", 21);
    let opened = store.open().expect("opens");
    let retired_file = dir.join(key_file_name(opened.signer().identity()));
    let (state, key) = opened.into_custody();
    let (mut registry, held, mut own, mut own_links, adopted, mut presigned) = state.into_parts();
    let retired = held.expect("held");
    let attested = registry
        .rotate_attested(&key, &actor(), &mut Fixed(22))
        .expect("rotates");
    let successor = attested.successor.identity().clone();
    own.insert(successor.clone(), own[&retired].clone());
    own_links.push(attested.rotation);
    presigned.insert(retired.clone(), attested.revocation);
    let next = SigningCustodyState::from_parts(
        registry,
        Some(successor.clone()),
        own,
        own_links,
        adopted,
        presigned,
    );
    SigningCustody::persist(&mut store, &next, Some(Zeroizing::new([22; SEED_LEN])))
        .expect("persists");

    let reopened = store.open().expect("reopens");
    assert_eq!(reopened.state(), &next, "the state round-trips exactly");
    assert_eq!(reopened.signer().identity(), &successor);
    assert!(
        !retired_file.exists(),
        "the retired key's secret is removed"
    );
    assert!(dir.join(key_file_name(&successor)).exists());
    assert!(!dir.join(STATE_TEMP_FILE).exists());
    assert_eq!(snapshot(&dir).len(), 2);
    assert_eq!(
        decode_state(&encode_state(&next).expect("encodes")).expect("decodes"),
        next
    );
}

/// Metamorphic, serialization round trip: a custody state written by `encode_state` and
/// read by `decode_state` is the same state, and writing it again gives the same bytes —
/// for a first-use state, one with a rotation, a pre-signed revocation, a published
/// revocation, and adopted links, and each of those with its sections empty.
#[test]
fn a_custody_state_survives_the_serialization_round_trip() {
    use continuum_evidence::signing::{LocalSigner, RevocationReason};
    let signed = |seed: u8| -> (SigningRegistry, LocalSigner) {
        let mut registry = SigningRegistry::new();
        let key = registry.mint(&actor(), &mut Fixed(seed)).expect("mint");
        (registry, key)
    };
    let (mut registry, k0) = signed(51);
    let first = SigningCustodyState::first_use(registry.clone(), k0.identity().clone());
    let attested = registry
        .rotate_attested(&k0, &actor(), &mut Fixed(52))
        .expect("rotate");
    let (peer_registry, p2) = signed(54);
    let adopted = vec![peer_registry.attest_revocation(&p2).expect("active")];
    registry
        .record_observed(
            &actor(),
            continuum_evidence::signing::SigningEvent::Minted {
                signer: p2.identity().clone(),
            },
        )
        .expect("learned");
    registry
        .record_observed(
            &actor(),
            continuum_evidence::signing::SigningEvent::Revoked {
                signer: p2.identity().clone(),
                reason: RevocationReason::Compromised,
            },
        )
        .expect("learned");
    let every: std::collections::BTreeSet<SignedArtifactKind> =
        SignedArtifactKind::ALL.into_iter().collect();
    let own = BTreeMap::from([
        (k0.identity().clone(), every.clone()),
        (
            attested.successor.identity().clone(),
            [SignedArtifactKind::DomainPack].into_iter().collect(),
        ),
    ]);
    let rotated = SigningCustodyState::from_parts(
        registry.clone(),
        Some(attested.successor.identity().clone()),
        own.clone(),
        vec![attested.rotation.clone()],
        adopted.clone(),
        BTreeMap::from([(k0.identity().clone(), attested.revocation.clone())]),
    )
    .with_local_revocations([p2.identity().clone()].into_iter().collect());
    let empty_sections = SigningCustodyState::from_parts(
        registry,
        Some(attested.successor.identity().clone()),
        own,
        Vec::new(),
        Vec::new(),
        BTreeMap::new(),
    );
    for state in [first, rotated, empty_sections] {
        let bytes = encode_state(&state).expect("encodes");
        let read = decode_state(&bytes).expect("decodes");
        assert_eq!(read, state, "decode after encode is the identity");
        assert_eq!(
            encode_state(&read).expect("encodes"),
            bytes,
            "and so is encode after decode"
        );
    }
}

/// A persist that cannot be recorded leaves the previous state in place: a seed that does
/// not derive the held key, a held key with no file, and a store that cannot be written.
#[test]
fn a_refused_persist_leaves_the_previous_state() {
    let (dir, mut store) = minted_fixed("refused", 31);
    let before = snapshot(&dir);
    let (state, key) = store.open().expect("opens").into_custody();
    let (mut registry, _, own, own_links, adopted, presigned) = state.clone().into_parts();
    let other = registry.mint(&actor(), &mut Fixed(32)).expect("mint");
    let naming_other = SigningCustodyState::from_parts(
        registry,
        Some(other.identity().clone()),
        own,
        own_links,
        adopted,
        presigned,
    );
    // The wrong seed for the named key.
    assert!(
        SigningCustody::persist(
            &mut store,
            &naming_other,
            Some(Zeroizing::new([33; SEED_LEN]))
        )
        .is_err()
    );
    // No seed for a key whose file is not in the store.
    assert_eq!(
        SigningCustody::persist(&mut store, &naming_other, None),
        Err(CustodyWrite::NotRecorded(KeystoreError::Incomplete {
            missing: KEY_FILE_LABEL
        }))
    );
    assert_eq!(snapshot(&dir), before, "neither refusal wrote anything");

    // A store the owner cannot write (skipped where the process bypasses permissions).
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o500)).expect("chmod");
    let probe = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dir.join("probe"));
    if probe.is_err() {
        assert!(matches!(
            SigningCustody::persist(&mut store, &state, None),
            Err(CustodyWrite::NotRecorded(KeystoreError::Io { .. }))
        ));
    }
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).expect("chmod");
    let _ = fs::remove_file(dir.join("probe"));
    assert_eq!(snapshot(&dir), before);
    assert_eq!(
        store.open().expect("opens").signer().identity(),
        key.identity()
    );
}

/// What a crash between a change and its clean-up leaves — a retired key's file and a
/// temporary state file — is ignored by open and removed by the sweep; the held key's file
/// is kept.
#[test]
fn the_sweep_removes_what_a_crash_leaves_and_keeps_the_held_key() {
    let (dir, mut store) = minted_fixed("sweep", 41);
    let held = store.open().expect("opens").signer().identity().clone();
    let mut other = SigningRegistry::new();
    let stale = other.mint(&actor(), &mut Fixed(42)).expect("mint");
    rewrite(
        &dir.join(key_file_name(stale.identity())),
        &[0u8; KEY_FILE_LEN],
    );
    rewrite(&dir.join(STATE_TEMP_FILE), b"half a state");
    rewrite(&dir.join("unrelated"), b"not the keystore's");
    assert_eq!(
        store.open().expect("still opens").signer().identity(),
        &held
    );
    assert_eq!(store.sweep(&held), Ok(2));
    let names: Vec<String> = snapshot(&dir).into_keys().collect();
    assert_eq!(
        names,
        vec![
            STATE_FILE.to_owned(),
            key_file_name(&held),
            "unrelated".to_owned()
        ]
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
    );
    assert_eq!(SigningCustody::sweep(&mut store, &held), Ok(()));
}

/// No refusal, and no `Debug` output of the store, carries the seed.
#[test]
fn no_error_or_debug_output_carries_key_material() {
    const SEED: u8 = 0xa7;
    let (dir, store) = minted_fixed("no-leak", SEED);
    let key = key_path(&dir, &store);
    let original = fs::read(&key).expect("key");
    let mut outputs = vec![
        format!("{store:?}"),
        format!("{:?}", store.open().expect("opens")),
        format!("{:?}", store.open().expect("opens").state()),
    ];
    let mut damaged = original.clone();
    damaged[8] ^= 0x01;
    rewrite(&key, &damaged);
    let refused = store.open().map(|_| ()).expect_err("refused");
    outputs.push(format!("{refused:?}"));
    outputs.push(refused.to_string());
    rewrite(&key, &original[..original.len() - 1]);
    let refused = store.open().map(|_| ()).expect_err("refused");
    outputs.push(format!("{refused:?}"));
    outputs.push(refused.to_string());
    let seed_hex: String = std::iter::repeat_n(format!("{SEED:02x}"), SEED_LEN).collect();
    let seed_dec = std::iter::repeat_n(SEED.to_string(), 4)
        .collect::<Vec<_>>()
        .join(", ");
    for output in outputs {
        assert!(!output.contains(&seed_hex), "{output}");
        assert!(!output.contains(&seed_hex.to_uppercase()), "{output}");
        assert!(!output.contains(&seed_dec), "{output}");
    }
}

// --- cr-3l3n47: directory trust, symlinks, ownership ------------------------------------

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
    // Refused at the first component the other user does not own: every ancestor the
    // path passes through is held to root and the expected owner first (review
    // cr-2qu5zr), and the store directory after them.
    assert!(matches!(
        foreign.open().map(|_| ()),
        Err(KeystoreError::UntrustedAncestor { uid: found, .. }) if found == uid
    ));
    // The same seam, honestly named, opens the store its owner holds.
    assert!(LocalKeystore::new(&dir).expecting_owner(uid).open().is_ok());
}

#[test]
fn a_symlinked_key_or_state_file_is_refused() {
    let (dir, store) = minted("symlinked-key");
    let key = key_path(&dir, &store);
    let elsewhere = dir.with_file_name("elsewhere.key");
    fs::rename(&key, &elsewhere).expect("move");
    symlink(&elsewhere, &key).expect("symlink");
    assert_eq!(
        store.open().map(|_| ()),
        Err(KeystoreError::SymlinkedFile {
            file: KEY_FILE_LABEL
        })
    );
    fs::remove_file(&key).expect("unlink");
    fs::rename(&elsewhere, &key).expect("restore");

    let state = dir.join(STATE_FILE);
    let elsewhere = dir.with_file_name("elsewhere.state");
    fs::rename(&state, &elsewhere).expect("move");
    symlink(&elsewhere, &state).expect("symlink");
    assert_eq!(
        store
            .open_or_mint(&actor(), &mut OsEntropy::new())
            .map(|_| ()),
        Err(KeystoreError::SymlinkedFile { file: STATE_FILE })
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

// --- review of bn-18w74: the store lock and hard links ------------------------------------

/// A handle that loaded the store as a custody holds its lock for life: a second handle
/// can neither load it nor write it, and can once the first is dropped.
#[test]
fn a_store_held_as_a_custody_is_locked_against_every_other_writer() {
    let (dir, _) = minted_fixed("locked", 61);
    let mut holder = LocalKeystore::new(&dir);
    let (state, _) =
        SigningCustody::load_or_mint(&mut holder, &actor(), &mut Fixed(62)).expect("loads");
    let mut other = LocalKeystore::new(&dir);
    assert_eq!(
        SigningCustody::load_or_mint(&mut other, &actor(), &mut Fixed(63)).map(|_| ()),
        Err(KeystoreError::Locked)
    );
    assert_eq!(
        other.persist(&state, None),
        Err(CustodyWrite::NotRecorded(KeystoreError::Locked))
    );
    assert_eq!(
        other.sweep(state.held().expect("held")),
        Err(KeystoreError::Locked)
    );
    // The holder itself still writes.
    assert_eq!(SigningCustody::persist(&mut holder, &state, None), Ok(()));
    drop(holder);
    assert!(SigningCustody::load_or_mint(&mut other, &actor(), &mut Fixed(63)).is_ok());
}

/// A key file with a second hard link is refused: removing the name would not remove the
/// secret.
#[test]
fn a_hard_linked_key_file_is_refused() {
    let (dir, store) = minted("hard-link");
    let key = key_path(&dir, &store);
    let twin = dir.with_file_name("twin.key");
    fs::hard_link(&key, &twin).expect("link");
    assert_eq!(
        store.open().map(|_| ()),
        Err(KeystoreError::HardLinked {
            file: KEY_FILE_LABEL
        })
    );
    fs::remove_file(&twin).expect("unlink");
    assert!(store.open().is_ok());
}

// --- review cr-33e464: the commit point ------------------------------------------------------

/// Fails the next write once, at one phase; `lose` is the crash oracle's choice.
struct FailOnce {
    phase: PersistPhase,
    lose: bool,
    armed: std::sync::atomic::AtomicBool,
}

impl KeystoreFaults for FailOnce {
    fn fail(&self, phase: PersistPhase) -> bool {
        phase == self.phase && self.armed.swap(false, std::sync::atomic::Ordering::SeqCst)
    }

    fn lose_rename(&self) -> bool {
        self.lose
    }
}

/// A write that fails before the rename is `NotRecorded`, leaves the previous state, and
/// leaves no pending marker. One that fails at or after it is `Unconfirmed` and leaves the
/// marker; a reopen then finds the new state or — when the crash oracle loses the rename —
/// the old one, reports it pending, and the sweep clears the marker.
#[test]
fn a_write_reports_which_side_of_the_rename_it_failed_on() {
    for (phase, lose) in [
        (PersistPhase::BeforeRename, false),
        (PersistPhase::AfterRename, false),
        (PersistPhase::AfterRename, true),
        (PersistPhase::DirectorySync, false),
        (PersistPhase::DirectorySync, true),
    ] {
        let case = format!("{phase:?}, lose={lose}");
        let (dir, _) = minted_fixed(&format!("phase-{phase:?}-{lose}"), 71);
        let mut store = LocalKeystore::new(&dir).with_faults(std::sync::Arc::new(FailOnce {
            phase,
            lose,
            armed: std::sync::atomic::AtomicBool::new(true),
        }));
        let (before, key) =
            SigningCustody::load_or_mint(&mut store, &actor(), &mut Fixed(72)).expect("loads");
        let (mut registry, held, mut own, mut ol, al, mut ps) = before.clone().into_parts();
        let attested = registry
            .rotate_attested(&key, &actor(), &mut Fixed(73))
            .expect("rotates");
        let kinds = own[held.as_ref().expect("held")].clone();
        own.insert(attested.successor.identity().clone(), kinds);
        ol.push(attested.rotation);
        ps.insert(key.identity().clone(), attested.revocation);
        let after = SigningCustodyState::from_parts(
            registry,
            Some(attested.successor.identity().clone()),
            own,
            ol,
            al,
            ps,
        );
        let written =
            SigningCustody::persist(&mut store, &after, Some(Zeroizing::new([73; SEED_LEN])));
        drop(store);
        let pending = dir.join(continuum_security::keystore::PENDING_FILE);
        let reopened = LocalKeystore::new(&dir).open().expect("reopens");
        if phase == PersistPhase::BeforeRename {
            assert!(
                matches!(written, Err(CustodyWrite::NotRecorded(_))),
                "{case}"
            );
            assert_eq!(
                reopened.state(),
                &before,
                "{case}: the previous state loads"
            );
            assert!(!reopened.pending() && !pending.exists(), "{case}");
            continue;
        }
        assert!(
            matches!(written, Err(CustodyWrite::Unconfirmed(_))),
            "{case}"
        );
        let expected = if lose { &before } else { &after };
        assert_eq!(reopened.state(), expected, "{case}: one of the two states");
        assert!(reopened.pending(), "{case}: the ambiguity is durable");
        let held = reopened.signer().identity().clone();
        LocalKeystore::new(&dir).sweep(&held).expect("sweeps");
        assert!(!pending.exists(), "{case}: reconciled");
        assert!(!LocalKeystore::new(&dir).open().expect("reopens").pending());
    }
}

// --- review cr-1dc5ii: one writer ----------------------------------------------------------

/// Many threads writing through one handle — the lifetime lock holder, shared by `Arc`,
/// since a handle cannot be cloned — are serialized: every write succeeds, the store
/// reopens whole, and no temporary file is left. Before the fix, clones sharing the lock
/// skipped the guard and raced on one temporary path.
#[test]
fn concurrent_writes_through_one_handle_are_serialized() {
    let (dir, _) = minted_fixed("serialized", 81);
    let mut holder = LocalKeystore::new(&dir);
    let (state, _) =
        SigningCustody::load_or_mint(&mut holder, &actor(), &mut Fixed(82)).expect("loads");
    let holder = std::sync::Arc::new(holder);
    let state = std::sync::Arc::new(state);
    let threads: Vec<_> = (0..6)
        .map(|_| {
            let (holder, state) = (holder.clone(), state.clone());
            std::thread::spawn(move || {
                (0..8)
                    .map(|_| holder.persist(&state, None))
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    for thread in threads {
        for written in thread.join().expect("a writer thread") {
            assert_eq!(written, Ok(()), "a serialized write never races another");
        }
    }
    assert_eq!(holder.open().expect("reopens").state(), &*state);
    assert!(
        snapshot(&dir)
            .keys()
            .all(|name| !name.starts_with(STATE_TEMP_FILE)),
        "no temporary state file is left"
    );

    // An unheld handle's writes take the lock one at a time as well.
    drop(holder);
    let transient = std::sync::Arc::new(LocalKeystore::new(&dir));
    let threads: Vec<_> = (0..3)
        .map(|_| {
            let (store, state) = (transient.clone(), state.clone());
            std::thread::spawn(move || {
                (0..4)
                    .map(|_| store.persist(&state, None))
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    for thread in threads {
        for written in thread.join().expect("a writer thread") {
            assert_eq!(written, Ok(()));
        }
    }
}

/// A second handle on the same store in this process is refused — to load it, to write
/// it, and to read it — while the first holds it: flock locks belong to the open file
/// description, so a second handle conflicts like a second process. Once the holder is
/// gone, the second handle succeeds.
#[test]
fn a_second_handle_in_process_is_refused_while_one_holds_the_store() {
    let (dir, _) = minted_fixed("second-handle", 91);
    let mut holder = LocalKeystore::new(&dir);
    let (state, _) =
        SigningCustody::load_or_mint(&mut holder, &actor(), &mut Fixed(92)).expect("loads");
    let second = LocalKeystore::new(&dir);
    assert_eq!(
        second.open().map(|_| ()),
        Err(KeystoreError::Locked),
        "no read"
    );
    assert_eq!(
        second.persist(&state, None),
        Err(CustodyWrite::NotRecorded(KeystoreError::Locked)),
        "no write"
    );
    assert!(holder.open().is_ok(), "the holder itself reads");
    drop(holder);
    assert!(second.open().is_ok());
}

/// Parks the first write to reach the commit point, holding the handle's writer mutex, until
/// the test releases it.
struct Park {
    entered: std::sync::Mutex<Option<std::sync::mpsc::Sender<()>>>,
    release: std::sync::Mutex<std::sync::mpsc::Receiver<()>>,
}

impl KeystoreFaults for Park {
    fn fail(&self, phase: PersistPhase) -> bool {
        let entered = if phase == PersistPhase::BeforeRename {
            self.entered.lock().expect("unpoisoned").take()
        } else {
            None
        };
        if let Some(entered) = entered {
            entered.send(()).expect("the test waits");
            self.release
                .lock()
                .expect("unpoisoned")
                .recv()
                .expect("the test releases");
        }
        false
    }
}

/// Where a write is parked, holding the writer mutex, when the fork is simulated.
#[derive(Debug, Clone, Copy)]
enum Window {
    /// Inside the first-use mint (`open_or_mint` on an empty store).
    FirstMint,
    /// Inside a write through a handle that holds the lifetime lock.
    HeldWrite,
    /// Inside a write through a handle that takes a flock for that write alone.
    TransientWrite,
}

/// Review cr-1dc5ii rounds 3 and 4: a child forked while another thread held the writer
/// mutex inherits the mutex locked, with no thread to unlock it. Its copy of the handle
/// must be refused (`InheritedHandle`) promptly, without touching that mutex, at every
/// window where the mutex can be held. For each window, one thread's write is parked
/// holding the mutex. The handle is then marked as a fork copy, and a load, a write, a
/// read, and a sweep from another thread must answer within the timeout; a regression that
/// locked first would block, and the timeout makes that a failure, not a hang. The parked
/// owner's write then completes.
#[test]
fn an_inherited_handle_is_refused_at_every_window_its_mutex_can_be_held() {
    use std::sync::mpsc;
    for (at, window) in [Window::FirstMint, Window::HeldWrite, Window::TransientWrite]
        .into_iter()
        .enumerate()
    {
        let seed = 110 + 4 * u8::try_from(at).expect("small");
        let dir = match window {
            Window::FirstMint => scratch(&format!("window-{window:?}")),
            _ => minted_fixed(&format!("window-{window:?}"), seed).0,
        };
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let mut handle = LocalKeystore::new(&dir).with_faults(std::sync::Arc::new(Park {
            entered: std::sync::Mutex::new(Some(entered_tx)),
            release: std::sync::Mutex::new(release_rx),
        }));
        if matches!(window, Window::HeldWrite) {
            SigningCustody::load_or_mint(&mut handle, &actor(), &mut Fixed(seed + 1))
                .expect("loads");
        }
        // A state to present: the one on disk, or, before the first mint, any whole one.
        let state = match window {
            Window::FirstMint => {
                let mut registry = SigningRegistry::new();
                let key = registry.mint(&actor(), &mut Fixed(seed + 2)).expect("mint");
                SigningCustodyState::first_use(registry, key.identity().clone())
            }
            _ => handle.open().expect("opens").state().clone(),
        };
        let held = state.held().cloned().expect("held");
        let handle = std::sync::Arc::new(handle);
        let owner = {
            let (handle, state) = (std::sync::Arc::clone(&handle), state.clone());
            std::thread::spawn(move || match window {
                Window::FirstMint => handle
                    .open_or_mint(&actor(), &mut Fixed(seed + 3))
                    .map(|_| ())
                    .map_err(CustodyWrite::NotRecorded),
                _ => handle.persist(&state, None),
            })
        };
        entered_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("the owner's write holds the mutex");

        handle.simulate_fork_copy();
        let (answers_tx, answers_rx) = mpsc::channel();
        {
            let (copy, state) = (std::sync::Arc::clone(&handle), state.clone());
            std::thread::spawn(move || {
                let answers = (
                    copy.open_or_mint(&actor(), &mut Fixed(seed + 3))
                        .map(|_| ()),
                    copy.persist(&state, None),
                    copy.open().map(|_| ()),
                    copy.sweep(&held),
                );
                let _ = answers_tx.send(answers);
            });
        }
        let answers = answers_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap_or_else(|_| panic!("{window:?}: refused promptly, never blocked"));
        assert_eq!(
            answers,
            (
                Err(KeystoreError::InheritedHandle),
                Err(CustodyWrite::NotRecorded(KeystoreError::InheritedHandle)),
                Err(KeystoreError::InheritedHandle),
                Err(KeystoreError::InheritedHandle),
            ),
            "{window:?}"
        );

        // Anti-vacuity: the owner, restored and released, completes its write.
        handle.simulate_fork_copy();
        release_tx.send(()).expect("the parked write waits");
        assert_eq!(
            owner.join().expect("the owner's thread"),
            Ok(()),
            "{window:?}"
        );
        assert!(handle.open().is_ok(), "{window:?}");
    }
}

/// The first lock, too: a fork copy's `load_or_mint` is refused before it takes the writer
/// mutex or the lifetime flock, and a handle whose process could not be read is never
/// treated as owned.
#[test]
fn an_inherited_handle_is_refused_its_first_load() {
    let (dir, _) = minted_fixed("first-load", 130);
    let mut handle = LocalKeystore::new(&dir);
    handle.simulate_fork_copy();
    assert_eq!(
        SigningCustody::load_or_mint(&mut handle, &actor(), &mut Fixed(131)).map(|_| ()),
        Err(KeystoreError::InheritedHandle)
    );
    handle.simulate_fork_copy();
    assert!(SigningCustody::load_or_mint(&mut handle, &actor(), &mut Fixed(131)).is_ok());
}

// --- review cr-2qu5zr: symlinks in the store's ancestry ----------------------------------

/// A private directory for a symlink test: `<scratch root>/<name>`, mode `0700`.
fn private_dir(root: &Path, name: &str) -> PathBuf {
    let dir = root.join(name);
    fs::create_dir_all(&dir).expect("dir");
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).expect("private");
    dir
}

/// A symlink whose containing directory another user can write is refused: whoever can
/// write that directory can retarget the link. So is a link whose target passes through
/// such a directory. The same link in a private directory reaches the store. A link in a
/// sticky shared directory is refused too (review cr-2qu5zr round 2, the regression guard
/// for this test's earlier expectation): the sticky bit stops others from replacing an
/// entry, not from creating one, such as a hard link to one of the owner's symlinks.
#[test]
fn a_symlink_in_or_through_a_writable_directory_is_refused() {
    let root = scratch("symlink-trust").with_file_name("");
    let real = private_dir(&root, "real");
    LocalKeystore::new(real.join("store"))
        .open_or_mint(&actor(), &mut Fixed(141))
        .expect("mint");

    let shared = private_dir(&root, "shared");
    fs::set_permissions(&shared, fs::Permissions::from_mode(0o770)).expect("group-writable");
    symlink(&real, shared.join("link")).expect("symlink");
    assert!(matches!(
        LocalKeystore::new(shared.join("link/store"))
            .open()
            .map(|_| ()),
        Err(KeystoreError::UnsafeParent { mode: 0o770, .. })
    ));

    let private = private_dir(&root, "private");
    symlink(&real, private.join("link")).expect("symlink");
    assert!(
        LocalKeystore::new(private.join("link/store"))
            .open()
            .is_ok()
    );
    // Its target, through the writable directory: refused.
    let inner = shared.join("inner");
    fs::create_dir_all(&inner).expect("inner");
    fs::set_permissions(&inner, fs::Permissions::from_mode(0o700)).expect("private");
    symlink(&inner, private.join("through")).expect("symlink");
    LocalKeystore::new(inner.join("store"))
        .open_or_mint(&actor(), &mut Fixed(142))
        .expect_err("the direct path is refused too");
    assert!(matches!(
        LocalKeystore::new(private.join("through/store"))
            .open()
            .map(|_| ()),
        Err(KeystoreError::UnsafeParent { mode: 0o770, .. })
    ));

    let sticky = private_dir(&root, "sticky");
    fs::set_permissions(&sticky, fs::Permissions::from_mode(0o1777)).expect("sticky");
    symlink(&real, sticky.join("link")).expect("symlink");
    assert!(
        matches!(
            LocalKeystore::new(sticky.join("link/store"))
                .open()
                .map(|_| ()),
            Err(KeystoreError::UnsafeParent { mode: 0o1777, .. })
        ),
        "a link in a sticky shared directory is refused"
    );
    // The sticky directory itself, as a plain ancestor with no link in it, stays allowed.
    let beneath = sticky.join("beneath");
    fs::create_dir_all(&beneath).expect("beneath");
    fs::set_permissions(&beneath, fs::Permissions::from_mode(0o700)).expect("private");
    assert!(
        LocalKeystore::new(beneath.join("store"))
            .open_or_mint(&actor(), &mut Fixed(143))
            .is_ok()
    );
}

/// A chain of trusted symlinks — relative and absolute, one through another — reaches the
/// store for every operation.
#[test]
fn a_trusted_symlink_chain_reaches_the_store() {
    let root = scratch("symlink-chain").with_file_name("");
    let real = private_dir(&root, "real");
    let hop = private_dir(&root, "hop");
    symlink(&real, hop.join("absolute")).expect("symlink");
    symlink("absolute", hop.join("relative")).expect("symlink");
    let entry = private_dir(&root, "entry");
    symlink("../hop/relative", entry.join("link")).expect("symlink");
    let mut store = LocalKeystore::new(entry.join("link/store"));
    let (state, _) =
        SigningCustody::load_or_mint(&mut store, &actor(), &mut Fixed(151)).expect("loads");
    assert_eq!(SigningCustody::persist(&mut store, &state, None), Ok(()));
    assert_eq!(store.open().expect("opens").state(), &state);
    assert!(
        real.join("store").join(STATE_FILE).exists(),
        "the chain reached the store"
    );
}

/// Retargeting a symlink after the store was validated cannot redirect a write: the write
/// is parked just before its commit point, the store's ancestor link is switched to
/// another valid store, and the re-check before the rename refuses (`StoreMoved`). The
/// other store is not touched, and every later operation through the handle refuses too.
#[test]
fn a_symlink_swapped_after_validation_cannot_redirect_a_write() {
    use std::sync::mpsc;
    let root = scratch("symlink-swap").with_file_name("");
    let first = private_dir(&root, "first");
    let second = private_dir(&root, "second");
    LocalKeystore::new(second.join("store"))
        .open_or_mint(&actor(), &mut Fixed(161))
        .expect("the other store");
    let other_before = snapshot(&second.join("store"));
    let entry = private_dir(&root, "entry");
    symlink(&first, entry.join("link")).expect("symlink");

    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let park = std::sync::Arc::new(Park {
        entered: std::sync::Mutex::new(None),
        release: std::sync::Mutex::new(release_rx),
    });
    let mut handle = LocalKeystore::new(entry.join("link/store")).with_faults(park.clone());
    let (state, _) =
        SigningCustody::load_or_mint(&mut handle, &actor(), &mut Fixed(162)).expect("loads");
    let first_before = snapshot(&first.join("store"));
    // Arm the park only now, so the first-use mint above ran through.
    *park.entered.lock().expect("unpoisoned") = Some(entered_tx);
    let handle = std::sync::Arc::new(handle);
    let writer = {
        let (handle, state) = (std::sync::Arc::clone(&handle), state.clone());
        std::thread::spawn(move || handle.persist(&state, None))
    };
    entered_rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("the write reached its commit point");
    fs::remove_file(entry.join("link")).expect("unlink");
    symlink(&second, entry.join("link")).expect("retarget");
    release_tx.send(()).expect("release");
    assert_eq!(
        writer.join().expect("the writer"),
        Err(CustodyWrite::NotRecorded(KeystoreError::StoreMoved))
    );
    assert_eq!(
        snapshot(&second.join("store")),
        other_before,
        "the other store is untouched"
    );
    assert_eq!(
        LocalKeystore::new(first.join("store"))
            .open()
            .map(|opened| opened.state().clone()),
        Err(KeystoreError::Locked),
        "the validated store is still held by the handle"
    );
    assert_eq!(handle.open().map(|_| ()), Err(KeystoreError::StoreMoved));
    assert_eq!(
        handle.sweep(state.held().expect("held")),
        Err(KeystoreError::StoreMoved)
    );
    // The validated store kept its state; only the aborted write's temporary file and
    // marker remain there, for the next launch to sweep.
    drop(handle);
    let reopened = LocalKeystore::new(first.join("store"))
        .open()
        .expect("reopens");
    assert_eq!(reopened.state(), &state);
    assert_eq!(
        first_before.get(STATE_FILE),
        snapshot(&first.join("store")).get(STATE_FILE)
    );
}

/// End to end, the case at the heart of review cr-2qu5zr: a symlink the walk passes
/// through is itself examined. Under a root-owned sticky `/tmp`, a link to `/` is refused
/// because its directory is shared (round 2), where a walk that skipped links would have
/// reached the root-owned `/etc` and answered `ForeignOwner`.
#[test]
fn a_symlink_in_the_ancestry_is_itself_held_to_the_owner_rule() {
    let tmp = Path::new("/tmp");
    let Ok(metadata) = fs::metadata(tmp) else {
        return;
    };
    if metadata.uid() != 0 || metadata.permissions().mode() & 0o1000 == 0 {
        // The case needs a root-owned sticky /tmp.
        return;
    }
    let link = tmp.join(format!(
        "continuum-keystore-{}-root-link",
        std::process::id()
    ));
    let _ = fs::remove_file(&link);
    symlink("/", &link).expect("symlink");
    let uid = fs::metadata(std::env::temp_dir()).expect("tmpdir").uid();
    let refused = LocalKeystore::new(link.join("etc"))
        .expecting_owner(uid + 1)
        .open()
        .map(|_| ());
    let _ = fs::remove_file(&link);
    assert!(
        matches!(
            &refused,
            Err(KeystoreError::UnsafeParent { path, mode: 0o1777 }) if path == tmp
        ),
        "{refused:?}"
    );
}

/// A symlink with a second link is refused (review cr-2qu5zr round 2): a hard link to a
/// symlink raises the symlink's link count, so a link planted by anyone — here the owner,
/// who may link its own files whatever `fs.protected_hardlinks` says — is visible. The
/// same symlink with one link reaches the store.
#[test]
fn a_hard_linked_symlink_in_the_path_is_refused() {
    let root = scratch("linked-symlink").with_file_name("");
    let real = private_dir(&root, "real");
    LocalKeystore::new(real.join("store"))
        .open_or_mint(&actor(), &mut Fixed(171))
        .expect("mint");
    let entry = private_dir(&root, "entry");
    symlink(&real, entry.join("link")).expect("symlink");
    assert!(LocalKeystore::new(entry.join("link/store")).open().is_ok());
    // `hard_link` links the symlink itself, never its target.
    fs::hard_link(entry.join("link"), entry.join("second-name")).expect("hard link");
    assert!(
        fs::symlink_metadata(entry.join("second-name"))
            .expect("the second name")
            .file_type()
            .is_symlink()
    );
    assert!(matches!(
        LocalKeystore::new(entry.join("link/store"))
            .open()
            .map(|_| ()),
        Err(KeystoreError::LinkedAncestor { links: 2, .. })
    ));
    fs::remove_file(entry.join("second-name")).expect("unlink");
    assert!(LocalKeystore::new(entry.join("link/store")).open().is_ok());
}

/// The store directory itself belongs to another user while every ancestor is trusted
/// (root-owned `/` and `/etc`): refused as `ForeignOwner`, never opened.
#[test]
fn a_store_directory_another_user_owns_is_refused() {
    let uid = fs::metadata(std::env::temp_dir()).expect("tmpdir").uid();
    if uid == 0 {
        return;
    }
    assert_eq!(
        LocalKeystore::new("/etc")
            .expecting_owner(uid)
            .open()
            .map(|_| ()),
        Err(KeystoreError::ForeignOwner {
            what: "the keystore directory",
            uid: 0,
            expected: uid
        })
    );
}
