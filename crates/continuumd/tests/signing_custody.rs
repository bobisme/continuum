//! The signing authority across a restart (bn-18w74, ADR-0054 "Revision: custody across
//! restart", plan §18.6).
//!
//! Every daemon here is built by the deployment launcher, `Builder::launch_signing`, over
//! the real on-disk keystore (`continuum_security::keystore::LocalKeystore`), and driven
//! through `Daemon::dispatch`. A restart is a second launch over the same directory.
//!
//! | Property | Tests |
//! |---|---|
//! | a restart reloads everything: registry, held key, own keys, own and adopted links, pre-signed revocations | `a_restart_keeps_rotations_revocations_and_adopted_links` |
//! | own keys stay apart from adopted peer keys across a restart | `own_and_adopted_keys_stay_apart_across_a_restart` |
//! | an install without custody state owns only its held key (the ADR-0054 residual) | `an_install_without_custody_state_owns_only_the_held_key` |
//! | a corrupt, truncated, or oversize store fails closed at launch and is never re-minted | `a_corrupt_truncated_or_oversize_store_fails_closed_at_launch` |
//! | a state that decodes but does not hold is refused before a key is used | `a_state_that_does_not_hold_is_refused_before_any_key_is_used`, `a_restored_state_without_room_to_recover_is_refused` |
//! | an exposed or symlinked file is refused at launch | `an_exposed_or_symlinked_store_is_refused_at_launch` |
//! | a refused write changes nothing and stops signing until a restart | `a_custody_that_refuses_a_write_changes_nothing_and_stops_signing` |
//! | what a crash leaves is swept only after validation | `the_launch_sweeps_a_retired_key_left_by_a_crash` |
//! | no key material in any error or `Debug` output | `no_launch_error_or_debug_output_carries_key_material` |
//! | review of bn-18w74: a state the daemon writes restarts; one writer; no custody over another identity; rotation leftovers checked | `a_replacement_minted_near_the_bound_survives_a_restart`, `a_second_launch_on_a_held_store_is_refused`, `a_signer_installed_over_a_launched_custody_writes_nothing`, `a_state_missing_what_a_rotation_leaves_is_refused` |
//! | differential against the library | `the_restored_authority_matches_the_library_running_the_same_operations` |
//! | review cr-33e464: every answer agrees with every state a crash can leave (oracle); `OutcomeUnknown` quarantines until a relaunch reconciles; no pre-3.9 client receives it; a compromised own key keeps its link | `every_answer_agrees_with_every_state_a_crash_can_leave`, `a_custody_daemon_refuses_signing_writes_below_3_9`, `a_compromised_own_key_without_its_revocation_link_is_refused` |

#![cfg(unix)]

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt, symlink};
use std::path::{Path, PathBuf};

use continuum_evidence::actor::ActorId as SigningActor;
use continuum_evidence::signing::{
    AllowedSigners, EntropyUnavailable, KeyEntropy, LocalSigner, SEED_LEN, Signature,
    SignedArtifactKind as Kind, SignerIdentity, SignerLink, SignerStanding, SigningCustody,
    SigningCustodyState, SigningEvent, SigningRegistry, Zeroizing,
};
use continuum_security::keystore::{
    CustodyWrite, KEY_FILE_LEN, KeystoreError, KeystoreFaults, LocalKeystore, MAX_AUDIT_RECORDS,
    MAX_LINKS, PersistPhase, STATE_FILE, STATE_TEMP_FILE, key_file_name,
};
use continuum_value::epoch::ProtocolWindow;
use continuumd::daemon::bundle::{
    BundleBody, MAX_AUDIT_RECORDS as DAEMON_RECORDS, MAX_BUNDLE_LINKS, encode_signed,
    signed_bytes_identity,
};
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::signing::{CustodyRefusal, SigningFamily};
use continuumd::daemon::{
    Builder, Daemon, InstallRefusal, LaunchRefusal, OperationOutcome, OperationRequest,
    ReceiptSigner,
};
use continuumd::protocol::envelope::RequestEnvelope;
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, VersionRange, negotiate,
};
use continuumd::protocol::operations::intent::IntentImportBundleRequest;
use continuumd::protocol::operations::signing::{
    SigningMintRequest, SigningRegistryRequest, SigningRevokeRequest, SigningRotateRequest,
    SigningSignPackRequest, SigningVerifyRequest,
};
use continuumd::protocol::registry::{self, ENCODINGS};
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, Opaque, OperationName, ProtocolVersion, RequestId, SignerHandle,
    Timestamp,
};
use continuumd::protocol::spec::{Annotation, Nullable, Optional};
use continuumd::protocol::vocabulary::{
    AuthorityLevel, Encoding, ErrorCode, RevocationReason, SignatureOutcome, SignedArtifactKind,
};

const STEWARD: (&str, &str) = ("human:steward", "cap_steward");
const READER: (&str, &str) = ("agent:reader", "cap_reader");
const PRIVILEGES: [&str; 5] = [
    "intent.import_bundle",
    "signing.mint",
    "signing.rotate",
    "signing.revoke",
    "signing.sign_pack",
];
const BUNDLE_KINDS: [Kind; 2] = [Kind::IntentBundle, Kind::IntentAcceptance];

// --- fixtures ------------------------------------------------------------------------------

/// Deterministic entropy: seed `[n + 1; 32]`, then `[n + 2; 32]`, and so on.
struct Seeds(u8);

impl KeyEntropy for Seeds {
    fn seed(&mut self) -> Result<Zeroizing<[u8; SEED_LEN]>, EntropyUnavailable> {
        self.0 = self.0.wrapping_add(1);
        Ok(Zeroizing::new([self.0; SEED_LEN]))
    }
}

struct OneSeed([u8; SEED_LEN]);

impl KeyEntropy for OneSeed {
    fn seed(&mut self) -> Result<Zeroizing<[u8; SEED_LEN]>, EntropyUnavailable> {
        Ok(Zeroizing::new(self.0))
    }
}

fn cap(handle: &str) -> CapabilityHandle {
    CapabilityHandle::new(handle).expect("capability")
}

fn who(actor: &str) -> ActorId {
    ActorId::new(actor).expect("actor")
}

fn signing_actor() -> SigningActor {
    SigningActor::new("human:steward").expect("actor")
}

fn grant(
    principal: (&str, &str),
    level: AuthorityLevel,
    privileged: &[&str],
) -> CapabilityDescriptor {
    CapabilityDescriptor {
        capability: cap(principal.1),
        actor: who(principal.0),
        level,
        snapshots: Vec::new(),
        intents: Vec::new(),
        artifact_classes: Vec::new(),
        expires_at: Nullable::Null,
        delegation_depth: 3,
        profile: Optional::Present(CapabilityProfile {
            privileged_operations: privileged
                .iter()
                .map(|op| OperationName::new(op).expect("operation"))
                .collect(),
            denied_operations: Vec::new(),
            data_grants: Vec::new(),
            cross_principal_sharing: false,
        }),
        instances: Optional::Absent,
    }
}

fn negotiated() -> Negotiated {
    let version = ProtocolVersion::new(3, 9);
    let hello = ClientHello {
        protocol_versions: VersionRange {
            low: version,
            high: version,
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuumd-signing-custody".to_owned(),
        actor: who("service:continuumd"),
        capability: cap("cap_root"),
        features: Optional::Absent,
    };
    negotiate(&[version], ProtocolWindow::new(3), ENCODINGS, &hello).expect("served")
}

fn builder(pins: &AllowedSigners) -> Builder {
    let root = Some(cap("cap_root"));
    Daemon::builder(Blake3Identity, negotiated(), cap("cap_root"))
        .now(Timestamp::new("2026-09-24T00:00:00.000Z").expect("timestamp"))
        .capability(
            CapabilityDescriptor {
                delegation_depth: 5,
                ..grant(
                    ("service:continuumd", "cap_root"),
                    AuthorityLevel::Promote,
                    &PRIVILEGES,
                )
            },
            None,
        )
        .capability(
            grant(STEWARD, AuthorityLevel::ReviseIntent, &PRIVILEGES),
            root.clone(),
        )
        .capability(grant(READER, AuthorityLevel::Read, &[]), root)
        .family(SigningFamily)
        .family(IntentFamily)
        .allowed_signers(pins)
}

/// A private scratch directory, unique per test; the keystore goes in `store` under it.
fn scratch(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/tmp")
        .join(format!("signing-custody-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("scratch");
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("private");
    root.join("store")
}

struct World {
    daemon: Daemon,
    next: u32,
    version_override: Option<ProtocolVersion>,
}

/// Launch a daemon over the keystore at `dir`, drawing keys from `Seeds(start)`.
fn launch(
    dir: &Path,
    start: u8,
    pins: &AllowedSigners,
) -> Result<World, LaunchRefusal<KeystoreError>> {
    builder(pins)
        .launch_signing(
            LocalKeystore::new(dir),
            &signing_actor(),
            Box::new(Seeds(start)),
        )
        .map(|builder| World {
            daemon: builder.build(),
            next: 0,
            version_override: None,
        })
}

impl World {
    fn call(&mut self, principal: (&str, &str), arguments: Arguments) -> OperationOutcome {
        self.call_keyed(principal, arguments, None)
    }

    /// [`call`](Self::call) with a chosen idempotency key, so a request can be replayed.
    fn call_keyed(
        &mut self,
        principal: (&str, &str),
        arguments: Arguments,
        key: Option<&str>,
    ) -> OperationOutcome {
        self.next += 1;
        let operation = arguments.operation();
        let spec = registry::operation(operation).expect("a declared operation");
        let envelope = RequestEnvelope {
            protocol_version: self.version_override.unwrap_or(ProtocolVersion::new(3, 9)),
            request_id: RequestId::new(&format!("req_{}", self.next)).expect("request id"),
            idempotency_key: if spec.has(Annotation::Mutation) {
                Optional::Present(key.map_or_else(|| format!("k{}", self.next), str::to_owned))
            } else {
                Optional::Absent
            },
            actor: who(principal.0),
            capability: cap(principal.1),
            operation: OperationName::new(operation).expect("operation"),
            snapshot: Nullable::Null,
            intent: Nullable::Null,
            arguments: Opaque::from_bytes(Vec::new()),
            budget: Optional::Absent,
            output_policy: Optional::Absent,
            trace: Optional::Absent,
            page: Optional::Absent,
        };
        self.daemon.dispatch(&OperationRequest {
            envelope,
            arguments,
        })
    }

    fn held(&self) -> SignerIdentity {
        self.daemon
            .state()
            .signing()
            .held()
            .cloned()
            .expect("a held key")
    }

    fn snapshot(&self) -> SigningCustodyState {
        self.daemon.state().signing().custody_snapshot()
    }

    fn standing(&self, signer: &SignerIdentity) -> Option<SignerStanding> {
        self.daemon
            .state()
            .signing()
            .registry()
            .standing(signer)
            .cloned()
    }

    fn rotate(&mut self) -> Result<SignerIdentity, ErrorCode> {
        let signer = handle(&self.held());
        let outcome = self.call(
            STEWARD,
            Arguments::SigningRotate(SigningRotateRequest { signer }),
        );
        match outcome.payload {
            Payload::SigningRotate(rotated) => {
                Ok(SignerIdentity::from_public_key(&rotated.public_key).expect("a key"))
            }
            _ => Err(outcome.error_code().expect("a refusal")),
        }
    }

    fn revoke(&mut self, signer: &SignerIdentity) -> Option<ErrorCode> {
        self.call(
            STEWARD,
            Arguments::SigningRevoke(SigningRevokeRequest {
                signer: handle(signer),
                reason: RevocationReason::Compromised,
            }),
        )
        .error_code()
    }

    fn sign_pack(&mut self, pack: &[u8]) -> Result<Vec<u8>, ErrorCode> {
        let outcome = self.call(
            STEWARD,
            Arguments::SigningSignPack(SigningSignPackRequest {
                pack: pack.to_vec(),
            }),
        );
        match outcome.payload {
            Payload::SigningSignPack(signed) => Ok(signed.signature),
            _ => Err(outcome.error_code().expect("a refusal")),
        }
    }

    fn verify_pack(&mut self, pack: &[u8], signature: Vec<u8>) -> SignatureOutcome {
        let outcome = self.call(
            READER,
            Arguments::SigningVerify(SigningVerifyRequest {
                kind: SignedArtifactKind::DomainPack,
                artifact: pack.to_vec(),
                signature: Optional::Present(signature),
            }),
        );
        let Payload::SigningVerify(verified) = outcome.payload else {
            panic!("verify failed: {:?}", outcome.error_code());
        };
        verified.outcome
    }

    fn import(&mut self, content: Vec<u8>) -> (SignatureOutcome, u32) {
        let outcome = self.call(
            STEWARD,
            Arguments::IntentImportBundle(IntentImportBundleRequest { content }),
        );
        let Payload::IntentImportBundle(imported) = outcome.payload else {
            panic!("import failed: {:?}", outcome.error_code());
        };
        (imported.outcome, imported.adopted)
    }
}

fn handle(identity: &SignerIdentity) -> SignerHandle {
    SignerHandle::new(identity.handle().as_str()).expect("a signer name")
}

/// A key from one fixed seed byte, and a registry that knows it.
fn keyed(seed: u8) -> (SigningRegistry, LocalSigner) {
    let mut registry = SigningRegistry::new();
    let key = registry
        .mint(&signing_actor(), &mut OneSeed([seed; SEED_LEN]))
        .expect("mint");
    (registry, key)
}

/// A registry where each of `keys` is active.
fn knowing(keys: &[&LocalSigner]) -> SigningRegistry {
    let mut registry = SigningRegistry::new();
    for key in keys {
        registry
            .record_observed(
                &signing_actor(),
                SigningEvent::Minted {
                    signer: key.identity().clone(),
                },
            )
            .expect("a new signer");
    }
    registry
}

/// A bundle with no contracts, carrying `links`, signed by `key` (active in `registry`).
fn linked_bundle(
    registry: &SigningRegistry,
    key: &LocalSigner,
    allowed: AllowedSigners,
    links: Vec<SignerLink>,
) -> Vec<u8> {
    let body = BundleBody {
        allowed,
        contracts: Vec::new(),
        links,
    };
    let body_bytes = body.encode();
    let signature = registry
        .sign(key, Kind::IntentBundle, &signed_bytes_identity(&body_bytes))
        .expect("sign");
    encode_signed(&body_bytes, &signature.encode())
}

/// A peer that rotates P1 → P2 and revokes P1 as compromised, attested by the keys; the
/// bundle is signed by P2. Returns the bundle, the pins it needs, and the two keys.
fn peer_bundle(first: u8) -> (Vec<u8>, AllowedSigners, LocalSigner, LocalSigner) {
    let (_, p1) = keyed(first);
    let (_, p2) = keyed(first + 1);
    let pins = AllowedSigners::new()
        .allow(p1.identity().clone(), BUNDLE_KINDS)
        .allow(p2.identity().clone(), BUNDLE_KINDS);
    let rotation = knowing(&[&p1, &p2])
        .attest_rotation(&p1, &p2)
        .expect("both active");
    let revocation = knowing(&[&p1]).attest_revocation(&p1).expect("active");
    let content = linked_bundle(
        &knowing(&[&p2]),
        &p2,
        pins.clone(),
        vec![rotation, revocation],
    );
    (content, pins, p1, p2)
}

/// Every file of the store but the (empty) lock file, by name, with its bytes.
fn files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fs::read_dir(dir)
        .expect("store")
        .filter(|entry| {
            entry.as_ref().map_or(true, |entry| {
                entry.file_name() != continuum_security::keystore::LOCK_FILE
            })
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

// --- restart -------------------------------------------------------------------------------

#[test]
fn a_restart_keeps_rotations_revocations_and_adopted_links() {
    let dir = scratch("round-trip");
    let (bundle, pins, p1, p2) = peer_bundle(150);
    let mut first = launch(&dir, 10, &pins).expect("first use");
    let k0 = first.held();
    let old_pack = b"a pack signed by the first key".as_slice();
    let old_signature = first.sign_pack(old_pack).expect("signs");
    let k1 = first.rotate().expect("rotates");
    let mid_pack = b"a pack signed by the second key".as_slice();
    let mid_signature = first.sign_pack(mid_pack).expect("signs");
    assert_eq!(
        first.revoke(&k0),
        None,
        "the retired key's pre-signed revocation"
    );
    let k2 = first.rotate().expect("rotates again");
    let (outcome, adopted) = first.import(bundle);
    assert_eq!(outcome, SignatureOutcome::Verified);
    assert!(
        adopted > 0,
        "the peer's rotation and revocation are adopted"
    );
    let before = first.snapshot();
    assert_eq!(
        before.own_links().len(),
        3,
        "two rotations and one revocation"
    );
    assert_eq!(before.adopted_links().len(), 2);
    assert_eq!(
        before.presigned().keys().collect::<Vec<_>>(),
        vec![&k1],
        "k0's pre-signed revocation was published; k1's is kept"
    );
    drop(first);

    let mut second = launch(&dir, 60, &pins).expect("a restart");
    assert_eq!(
        second.snapshot(),
        before,
        "everything the authority held survives"
    );
    assert_eq!(second.held(), k2);
    assert!(matches!(
        second.standing(&k0),
        Some(SignerStanding::Revoked { .. })
    ));
    assert!(matches!(
        second.standing(p1.identity()),
        Some(SignerStanding::Revoked { .. })
    ));
    assert_eq!(second.standing(p2.identity()), Some(SignerStanding::Active));
    // Signatures made before the restart keep their standing: revoked, rotated, active.
    assert_eq!(
        second.verify_pack(old_pack, old_signature),
        SignatureOutcome::SignerRevoked
    );
    assert_eq!(
        second.verify_pack(mid_pack, mid_signature),
        SignatureOutcome::Verified
    );
    let pack = b"a pack signed after the restart".as_slice();
    let signature = second.sign_pack(pack).expect("the held key signs");
    assert_eq!(
        second.verify_pack(pack, signature),
        SignatureOutcome::Verified
    );
    // k1's revocation was pre-signed before k1 was wiped, and survives the restart: revoking
    // k1 now publishes it.
    assert_eq!(second.revoke(&k1), None);
    let after = second.snapshot();
    assert!(after.presigned().is_empty());
    assert!(after.own_links().iter().any(|link| link.event()
        == &continuum_evidence::signing::LinkEvent::Revoked { signer: k1.clone() }));
    // Only the held key's secret is on disk.
    let names: Vec<String> = files(&dir).into_keys().collect();
    assert_eq!(names, vec![key_file_name(&k2), STATE_FILE.to_owned()]);
    drop(second);
    assert_eq!(launch(&dir, 90, &pins).expect("again").snapshot(), after);
}

#[test]
fn own_and_adopted_keys_stay_apart_across_a_restart() {
    let dir = scratch("own-adopted");
    let (bundle, pins, _, p2) = peer_bundle(170);
    let mut first = launch(&dir, 20, &pins).expect("first use");
    let k0 = first.held();
    first.rotate().expect("rotates");
    assert_eq!(first.import(bundle).0, SignatureOutcome::Verified);
    let own: Vec<SignerIdentity> = first.snapshot().own().keys().cloned().collect();
    assert!(own.contains(&k0));
    assert!(
        !own.contains(p2.identity()),
        "an adopted peer key is never own"
    );
    drop(first);

    let second = launch(&dir, 80, &pins).expect("a restart");
    assert_eq!(
        second.snapshot().own().keys().cloned().collect::<Vec<_>>(),
        own
    );
    // A thief holding the retired own key k0 (seed 21) attests its revocation: skipped,
    // because k0 is still known as this daemon's own after the restart.
    let (_, stolen) = keyed(21);
    assert_eq!(stolen.identity(), &k0, "the same seed makes the same key");
    let (_, p3) = keyed(172);
    let thief_pins = pins.clone().allow(p3.identity().clone(), BUNDLE_KINDS);
    let thief = linked_bundle(
        &knowing(&[&p3]),
        &p3,
        AllowedSigners::new().allow(p3.identity().clone(), BUNDLE_KINDS),
        vec![
            knowing(&[&stolen])
                .attest_revocation(&stolen)
                .expect("active"),
        ],
    );
    drop(second);
    let mut third = launch(&dir, 100, &thief_pins).expect("a restart");
    let (outcome, _) = third.import(thief);
    assert_eq!(outcome, SignatureOutcome::Verified);
    assert!(
        matches!(third.standing(&k0), Some(SignerStanding::Rotated { .. })),
        "a link about an own key is never adopted, before or after a restart"
    );
    // The adopted peer key p2 is still a peer: its own attested revocation is adopted.
    let p2_revocation = linked_bundle(
        &knowing(&[&p3]),
        &p3,
        AllowedSigners::new().allow(p3.identity().clone(), BUNDLE_KINDS),
        vec![knowing(&[&p2]).attest_revocation(&p2).expect("active")],
    );
    assert_eq!(third.import(p2_revocation), (SignatureOutcome::Verified, 1));
    assert!(matches!(
        third.standing(p2.identity()),
        Some(SignerStanding::Revoked { .. })
    ));
}

/// The ADR-0054 residual, closed: a registry installed without custody state makes its
/// held key the only own key. Before bn-18w74 every signer of the registry was own, so a
/// peer key the registry knew could never be revoked by its own attestation.
#[test]
fn an_install_without_custody_state_owns_only_the_held_key() {
    let mut registry = SigningRegistry::new();
    let held = registry
        .mint(&signing_actor(), &mut OneSeed([190; SEED_LEN]))
        .expect("mint");
    let peer = registry
        .mint(&signing_actor(), &mut OneSeed([191; SEED_LEN]))
        .expect("mint");
    let (_, exporter) = keyed(192);
    let pins = AllowedSigners::new().allow(exporter.identity().clone(), BUNDLE_KINDS);
    let signer = ReceiptSigner::new(registry, held).expect("installable");
    let mut world = World {
        daemon: builder(&pins).receipt_signer(signer).build(),
        next: 0,
        version_override: None,
    };
    let own: Vec<SignerIdentity> = world.snapshot().own().keys().cloned().collect();
    assert_eq!(own, vec![world.held()]);
    let content = linked_bundle(
        &knowing(&[&exporter]),
        &exporter,
        pins,
        vec![knowing(&[&peer]).attest_revocation(&peer).expect("active")],
    );
    let (outcome, adopted) = world.import(content);
    assert_eq!(outcome, SignatureOutcome::Verified);
    assert_eq!(
        adopted, 2,
        "the exporter's mint and the peer's own revocation"
    );
    assert!(matches!(
        world.standing(peer.identity()),
        Some(SignerStanding::Revoked { .. })
    ));
}

// --- fail closed ---------------------------------------------------------------------------

#[test]
fn a_corrupt_truncated_or_oversize_store_fails_closed_at_launch() {
    let dir = scratch("corrupt");
    let pins = AllowedSigners::new();
    let mut world = launch(&dir, 30, &pins).expect("first use");
    world.rotate().expect("rotates");
    drop(world);
    let state = dir.join(STATE_FILE);
    let original = fs::read(&state).expect("state");
    let before = files(&dir);
    let refused = |dir: &Path| match launch(dir, 40, &AllowedSigners::new()) {
        Ok(_) => panic!("a damaged store launched"),
        Err(refusal) => refusal,
    };
    // Truncated at every length.
    for len in 0..original.len() {
        rewrite(&state, &original[..len]);
        assert!(
            matches!(
                refused(&dir),
                LaunchRefusal::Custody(KeystoreError::Corrupt(_) | KeystoreError::Replay(_))
            ),
            "a {len}-byte prefix launched or was not refused as corrupt"
        );
    }
    // One byte of the held key's name flipped: no longer a key, or not the key on disk.
    let mut flipped = original.clone();
    flipped[20] ^= 0x20;
    rewrite(&state, &flipped);
    assert!(matches!(refused(&dir), LaunchRefusal::Custody(_)));
    // Oversize, sparse.
    rewrite(&state, &original);
    let file = fs::OpenOptions::new()
        .write(true)
        .open(&state)
        .expect("open");
    file.set_len(continuum_security::keystore::MAX_STATE_FILE_LEN + 1)
        .expect("grow");
    drop(file);
    assert_eq!(
        refused(&dir),
        LaunchRefusal::Custody(KeystoreError::Corrupt(
            "the state file exceeds its size bound"
        ))
    );
    // A corrupt key file.
    rewrite(&state, &original);
    let key = before
        .keys()
        .find(|name| name.ends_with(".key"))
        .expect("a key file")
        .clone();
    rewrite(&dir.join(&key), &[0u8; KEY_FILE_LEN]);
    assert!(matches!(
        refused(&dir),
        LaunchRefusal::Custody(KeystoreError::Corrupt(_))
    ));
    rewrite(&dir.join(&key), &before[&key]);
    assert_eq!(
        files(&dir),
        before,
        "no refusal minted, repaired, or swept anything"
    );
    assert!(launch(&dir, 40, &AllowedSigners::new()).is_ok());
}

/// Persist `state` through the keystore as it is, unchecked, so a launch reads it back.
fn plant(dir: &Path, state: &SigningCustodyState) {
    SigningCustody::persist(&mut LocalKeystore::new(dir), state, None).expect("planted");
}

#[test]
fn a_state_that_does_not_hold_is_refused_before_any_key_is_used() {
    let dir = scratch("does-not-hold");
    let (bundle, pins, p1, p2) = peer_bundle(200);
    let mut world = launch(&dir, 50, &pins).expect("first use");
    world.rotate().expect("rotates");
    assert_eq!(world.import(bundle).0, SignatureOutcome::Verified);
    let good = world.snapshot();
    drop(world);
    let held = good.held().cloned().expect("held");
    let retired = good
        .presigned()
        .keys()
        .next()
        .cloned()
        .expect("a retired key");
    let parts = || good.clone().into_parts();
    let refused = |state: &SigningCustodyState| {
        plant(&dir, state);
        let before = files(&dir);
        let refusal = match launch(&dir, 60, &pins) {
            Ok(_) => panic!("a state that does not hold launched"),
            Err(refusal) => refusal,
        };
        assert_eq!(files(&dir), before, "a refused launch sweeps nothing");
        refusal
    };
    let install = |refusal| LaunchRefusal::Install(InstallRefusal::Custody(refusal));

    // The held key is not own.
    let (registry, h, mut own, ol, al, ps) = parts();
    own.remove(&held);
    let state = SigningCustodyState::from_parts(registry, h, own, ol, al, ps);
    assert_eq!(refused(&state), install(CustodyRefusal::OwnKeyUnknown));

    // An adopted peer key claimed as own: p1 is revoked as compromised, so it is refused
    // first for lacking an own revocation link, before its adopted links are examined.
    let (registry, h, mut own, ol, al, ps) = parts();
    own.insert(p1.identity().clone(), [Kind::Receipt].into_iter().collect());
    let state = SigningCustodyState::from_parts(registry, h, own, ol, al, ps);
    assert_eq!(refused(&state), install(CustodyRefusal::OwnLink));
    // And the adopted peer p2: the rotation p1 -> p2 would then join a peer and an own
    // key, which no daemon records.
    let (registry, h, mut own, ol, al, ps) = parts();
    own.insert(p2.identity().clone(), [Kind::Receipt].into_iter().collect());
    let state = SigningCustodyState::from_parts(registry, h, own, ol, al, ps);
    assert_eq!(refused(&state), install(CustodyRefusal::OwnLink));

    // An own link the audit log does not record.
    let (registry, h, own, mut ol, al, ps) = parts();
    let (_, x) = keyed(210);
    ol.push(knowing(&[&x]).attest_revocation(&x).expect("active"));
    let state = SigningCustodyState::from_parts(registry, h, own, ol, al, ps);
    assert_eq!(refused(&state), install(CustodyRefusal::OwnLink));

    // An own link duplicated.
    let (registry, h, own, mut ol, al, ps) = parts();
    ol.push(ol[0].clone());
    let state = SigningCustodyState::from_parts(registry, h, own, ol, al, ps);
    assert_eq!(refused(&state), install(CustodyRefusal::OwnLink));

    // An adopted link with one signature byte changed.
    let (registry, h, own, ol, mut al, ps) = parts();
    let mut signatures: Vec<Signature> = al[0].signatures().to_vec();
    let mut bytes = *signatures[0].as_bytes();
    bytes[0] ^= 0x01;
    signatures[0] = Signature::from_bytes(&bytes).expect("64 bytes");
    al[0] = SignerLink::from_parts(al[0].event().clone(), signatures);
    let state = SigningCustodyState::from_parts(registry, h, own, ol, al, ps);
    assert_eq!(refused(&state), install(CustodyRefusal::AdoptedLink));

    // A pre-signed revocation for the held key (seed 52): the held key is never retired.
    let (registry, h, own, ol, al, mut ps) = parts();
    let (_, held_key) = keyed(52);
    assert_eq!(
        held_key.identity(),
        &held,
        "the same seed makes the same key"
    );
    ps.insert(
        held.clone(),
        knowing(&[&held_key])
            .attest_revocation(&held_key)
            .expect("active"),
    );
    let state = SigningCustodyState::from_parts(registry, h, own, ol, al, ps);
    assert_eq!(refused(&state), install(CustodyRefusal::Presigned));

    // A state naming a held key whose secret is not in the store is never committed.
    let (registry, _, own, ol, al, ps) = parts();
    let state = SigningCustodyState::from_parts(registry, Some(retired.clone()), own, ol, al, ps);
    let before = files(&dir);
    assert!(SigningCustody::persist(&mut LocalKeystore::new(&dir), &state, None).is_err());
    assert_eq!(files(&dir), before);

    plant(&dir, &good);
    assert!(
        launch(&dir, 60, &pins).is_ok(),
        "the good state still launches"
    );
}

/// The reserve, re-checked at restart: a restored registry whose held key is active with
/// no record free could not even be revoked.
#[test]
fn a_restored_state_without_room_to_recover_is_refused() {
    let dir = scratch("no-room");
    let world = launch(&dir, 70, &AllowedSigners::new()).expect("first use");
    let (mut registry, held, own, ol, al, ps) = world.snapshot().into_parts();
    drop(world);
    let mut seed = [0u8; SEED_LEN];
    seed[31] = 0x5a;
    let mut n: u32 = 0;
    while registry.audit_log().len() < DAEMON_RECORDS {
        n += 1;
        seed[..4].copy_from_slice(&n.to_le_bytes());
        registry
            .mint(&signing_actor(), &mut OneSeed(seed))
            .expect("mint");
    }
    let state = SigningCustodyState::from_parts(registry, held, own, ol, al, ps);
    plant(&dir, &state);
    assert_eq!(
        launch(&dir, 71, &AllowedSigners::new()).err(),
        Some(LaunchRefusal::Install(InstallRefusal::NoRoomToRecover))
    );
}

#[test]
fn an_exposed_or_symlinked_store_is_refused_at_launch() {
    let dir = scratch("exposed");
    let pins = AllowedSigners::new();
    let world = launch(&dir, 110, &pins).expect("first use");
    let key = dir.join(key_file_name(&world.held()));
    drop(world);
    fs::set_permissions(&key, fs::Permissions::from_mode(0o644)).expect("chmod");
    assert!(matches!(
        launch(&dir, 111, &pins).err(),
        Some(LaunchRefusal::Custody(KeystoreError::InsecurePermissions {
            mode: 0o644,
            ..
        }))
    ));
    fs::set_permissions(&key, fs::Permissions::from_mode(0o600)).expect("chmod");
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o750)).expect("chmod");
    assert!(
        matches!(
            launch(&dir, 111, &pins).err(),
            Some(LaunchRefusal::Custody(KeystoreError::UnsafeDirectory {
                mode: 0o750
            }))
        ),
        "a store directory others can search is refused: they could name its files"
    );
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o770)).expect("chmod");
    assert!(matches!(
        launch(&dir, 111, &pins).err(),
        Some(LaunchRefusal::Custody(
            KeystoreError::UnsafeDirectory { .. }
        ))
    ));
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).expect("chmod");
    let state = dir.join(STATE_FILE);
    let moved = dir.with_file_name("moved-state");
    fs::rename(&state, &moved).expect("move");
    symlink(&moved, &state).expect("symlink");
    assert!(matches!(
        launch(&dir, 111, &pins).err(),
        Some(LaunchRefusal::Custody(KeystoreError::SymlinkedFile { .. }))
    ));
}

/// A custody that refuses every write after the first `allowed`.
struct Refusing {
    inner: LocalKeystore,
    allowed: usize,
}

impl SigningCustody for Refusing {
    type Error = KeystoreError;

    fn load_or_mint(
        &mut self,
        actor: &SigningActor,
        entropy: &mut dyn KeyEntropy,
    ) -> Result<(SigningCustodyState, LocalSigner), KeystoreError> {
        self.inner.load_or_mint(actor, entropy)
    }

    fn persist(
        &mut self,
        state: &SigningCustodyState,
        minted: Option<Zeroizing<[u8; SEED_LEN]>>,
    ) -> Result<(), CustodyWrite<KeystoreError>> {
        if self.allowed == 0 {
            return Err(CustodyWrite::NotRecorded(KeystoreError::Io {
                op: "writing a keystore file",
                kind: std::io::ErrorKind::StorageFull,
            }));
        }
        self.allowed -= 1;
        SigningCustody::persist(&mut self.inner, state, minted)
    }

    fn sweep(&mut self, held: &SignerIdentity) -> Result<(), KeystoreError> {
        SigningCustody::sweep(&mut self.inner, held)
    }
}

#[test]
fn a_custody_that_refuses_a_write_changes_nothing_and_stops_signing() {
    let dir = scratch("refusing");
    let (bundle, pins, _, _) = peer_bundle(220);
    let mut world = World {
        daemon: builder(&pins)
            .launch_signing(
                Refusing {
                    inner: LocalKeystore::new(&dir),
                    allowed: 1,
                },
                &signing_actor(),
                Box::new(Seeds(120)),
            )
            .expect("first use")
            .build(),
        next: 0,
        version_override: None,
    };
    let k0 = world.held();
    let k1 = world.rotate().expect("the one allowed write");
    let recorded = world.snapshot();
    let on_disk = files(&dir);

    // The next write is refused: nothing changes, in memory or on disk.
    assert_eq!(world.rotate(), Err(ErrorCode::PublicationAborted));
    assert_eq!(world.snapshot(), recorded);
    assert_eq!(world.held(), k1);
    assert_eq!(files(&dir), on_disk);
    assert!(world.daemon.state().signing().custody_failed());
    // And every later signing write and signature is refused until a restart.
    assert_eq!(world.revoke(&k0), Some(ErrorCode::PublicationAborted));
    assert_eq!(world.sign_pack(b"pack"), Err(ErrorCode::PolicyGateFailed));
    let imported = world.call(
        STEWARD,
        Arguments::IntentImportBundle(IntentImportBundleRequest {
            content: bundle.clone(),
        }),
    );
    assert_eq!(imported.error_code(), Some(ErrorCode::PublicationAborted));
    let minted = world.call(
        STEWARD,
        Arguments::SigningMint(SigningMintRequest {
            kinds: vec![SignedArtifactKind::Receipt],
        }),
    );
    assert!(minted.error_code().is_some());
    assert_eq!(world.snapshot(), recorded);
    drop(world);

    // A restart reloads exactly what was recorded, and signs again.
    let mut restarted = launch(&dir, 140, &pins).expect("a restart");
    assert_eq!(restarted.snapshot(), recorded);
    let signature = restarted.sign_pack(b"pack").expect("signs");
    assert_eq!(
        restarted.verify_pack(b"pack", signature),
        SignatureOutcome::Verified
    );
    assert_eq!(restarted.import(bundle).0, SignatureOutcome::Verified);
}

#[test]
fn the_launch_sweeps_a_retired_key_left_by_a_crash() {
    let dir = scratch("sweep");
    let pins = AllowedSigners::new();
    let mut world = launch(&dir, 130, &pins).expect("first use");
    let retired = world.held();
    let retired_file = fs::read(dir.join(key_file_name(&retired))).expect("key");
    world.rotate().expect("rotates");
    let held = world.held();
    drop(world);
    // What a crash between the state rename and the clean-up leaves.
    rewrite(&dir.join(key_file_name(&retired)), &retired_file);
    rewrite(&dir.join(STATE_TEMP_FILE), b"half a state");
    let world = launch(&dir, 131, &pins).expect("a restart");
    assert_eq!(world.held(), held);
    let names: Vec<String> = files(&dir).into_keys().collect();
    assert_eq!(names, vec![key_file_name(&held), STATE_FILE.to_owned()]);
}

#[test]
fn no_launch_error_or_debug_output_carries_key_material() {
    const START: u8 = 0xb0;
    let dir = scratch("no-leak");
    let pins = AllowedSigners::new();
    let mut world = launch(&dir, START, &pins).expect("first use");
    world.rotate().expect("rotates");
    let mut outputs = vec![
        format!("{:?}", world.daemon.state().signing()),
        format!("{:?}", world.snapshot()),
    ];
    let key = dir.join(key_file_name(&world.held()));
    drop(world);
    let original = fs::read(&key).expect("key");
    let mut damaged = original.clone();
    damaged[8] ^= 0x01;
    rewrite(&key, &damaged);
    let refusal = launch(&dir, START, &pins).err().expect("refused");
    outputs.push(format!("{refusal:?}"));
    outputs.push(refusal.to_string());
    rewrite(&key, &original);
    // Both keys this test minted: seeds [START + 1; 32] and [START + 2; 32].
    for seed in [START + 1, START + 2] {
        let hex: String = std::iter::repeat_n(format!("{seed:02x}"), SEED_LEN).collect();
        let dec = std::iter::repeat_n(seed.to_string(), 4)
            .collect::<Vec<_>>()
            .join(", ");
        for output in &outputs {
            assert!(!output.contains(&hex), "{output}");
            assert!(!output.contains(&hex.to_uppercase()), "{output}");
            assert!(!output.contains(&dec), "{output}");
        }
    }
    assert!(launch(&dir, START, &pins).is_ok());
}

#[test]
fn the_keystore_bounds_are_the_daemons() {
    assert_eq!(MAX_AUDIT_RECORDS as usize, DAEMON_RECORDS);
    assert_eq!(MAX_LINKS as usize, MAX_BUNDLE_LINKS);
}

/// Differential: the daemon's custody state after a first-use mint, a rotation, and the
/// retired key's compromise revocation equals what the signing library produces running
/// the same operations with the same seeds and principal (oracle: `continuum_evidence`,
/// subject: `continuumd` through the keystore launcher and a restart).
#[test]
fn the_restored_authority_matches_the_library_running_the_same_operations() {
    let dir = scratch("differential");
    let pins = AllowedSigners::new();
    let mut world = launch(&dir, 230, &pins).expect("first use");
    let k0 = world.held();
    world.rotate().expect("rotates");
    assert_eq!(world.revoke(&k0), None);
    drop(world);
    let subject = launch(&dir, 240, &pins).expect("a restart").snapshot();

    let mut registry = continuum_evidence::signing::SigningRegistry::new();
    let first = registry
        .mint(&signing_actor(), &mut OneSeed([231; SEED_LEN]))
        .expect("mint");
    let attested = registry
        .rotate_attested(&first, &signing_actor(), &mut OneSeed([232; SEED_LEN]))
        .expect("rotate");
    registry
        .revoke(
            first.identity(),
            &signing_actor(),
            continuum_evidence::signing::RevocationReason::Compromised,
        )
        .expect("revoke");
    let every: std::collections::BTreeSet<Kind> = Kind::ALL.into_iter().collect();
    let oracle = SigningCustodyState::from_parts(
        registry,
        Some(attested.successor.identity().clone()),
        BTreeMap::from([
            (first.identity().clone(), every.clone()),
            (attested.successor.identity().clone(), every),
        ]),
        vec![attested.rotation, attested.revocation],
        Vec::new(),
        BTreeMap::new(),
    );
    assert_eq!(subject, oracle);
}

/// Review of bn-18w74, B1: a state the running daemon writes always restarts. Near the
/// bound, revoking the held key and minting its replacement leaves the new key active with
/// fewer than `RECOVERY_RECORDS` free — revocable, not replaceable — and a restart accepts
/// exactly that.
#[test]
fn a_replacement_minted_near_the_bound_survives_a_restart() {
    let dir = scratch("near-bound");
    let pins = AllowedSigners::new();
    let world = launch(&dir, 72, &pins).expect("first use");
    let (mut registry, held, own, ol, al, ps) = world.snapshot().into_parts();
    drop(world);
    let mut seed = [0u8; SEED_LEN];
    seed[31] = 0x6b;
    let mut n: u32 = 0;
    while registry.audit_log().len() < DAEMON_RECORDS - 4 {
        n += 1;
        seed[..4].copy_from_slice(&n.to_le_bytes());
        registry
            .mint(&signing_actor(), &mut OneSeed(seed))
            .expect("mint");
    }
    plant(
        &dir,
        &SigningCustodyState::from_parts(registry, held, own, ol, al, ps),
    );
    let mut world = launch(&dir, 73, &pins).expect("three records free: installable");
    let old = world.held();
    assert_eq!(world.revoke(&old), None);
    let minted = world.call(
        STEWARD,
        Arguments::SigningMint(SigningMintRequest {
            kinds: vec![SignedArtifactKind::DomainPack],
        }),
    );
    assert_eq!(minted.error_code(), None);
    let written = world.snapshot();
    assert_eq!(written.registry().audit_log().len(), DAEMON_RECORDS - 2);
    drop(world);
    let mut restarted = launch(&dir, 90, &pins).expect("a state the daemon wrote restarts");
    assert_eq!(restarted.snapshot(), written);
    let new = restarted.held();
    assert_eq!(
        restarted.revoke(&new),
        None,
        "and its active key can still be revoked"
    );
}

/// Review of bn-18w74, S1: installing another identity after the launcher attached the
/// custody makes the authority refuse every write, so it never records that identity
/// over the store.
#[test]
fn a_signer_installed_over_a_launched_custody_writes_nothing() {
    let dir = scratch("reinstall");
    let pins = AllowedSigners::new();
    drop(launch(&dir, 150, &pins).expect("first use"));
    let before = files(&dir);
    let (registry, key) = keyed(160);
    let mut world = World {
        daemon: builder(&pins)
            .launch_signing(
                LocalKeystore::new(&dir),
                &signing_actor(),
                Box::new(Seeds(151)),
            )
            .expect("launches")
            .receipt_signer(ReceiptSigner::new(registry, key).expect("installable"))
            .build(),
        next: 0,
        version_override: None,
    };
    assert!(world.daemon.state().signing().custody_failed());
    assert_eq!(world.rotate(), Err(ErrorCode::PublicationAborted));
    assert_eq!(world.sign_pack(b"pack"), Err(ErrorCode::PolicyGateFailed));
    drop(world);
    assert_eq!(files(&dir), before, "the store is untouched");
}

/// Review of bn-18w74: two daemons never hold one store.
#[test]
fn a_second_launch_on_a_held_store_is_refused() {
    let dir = scratch("second-launch");
    let pins = AllowedSigners::new();
    let first = launch(&dir, 170, &pins).expect("first use");
    assert_eq!(
        launch(&dir, 171, &pins).err(),
        Some(LaunchRefusal::Custody(KeystoreError::Locked))
    );
    drop(first);
    assert!(launch(&dir, 171, &pins).is_ok());
}

/// Review of bn-18w74, S4: a state no daemon writes — a held key retired by rotation, or
/// a retired own key without its rotation link or its pre-signed revocation — is refused.
#[test]
fn a_state_missing_what_a_rotation_leaves_is_refused() {
    let dir = scratch("rotation-gaps");
    let pins = AllowedSigners::new();
    let mut world = launch(&dir, 180, &pins).expect("first use");
    let retired = world.held();
    world.rotate().expect("rotates");
    let good = world.snapshot();
    drop(world);
    let install = |refusal| LaunchRefusal::Install(InstallRefusal::Custody(refusal));
    let (registry, h, own, ol, al, mut ps) = good.clone().into_parts();
    ps.remove(&retired);
    plant(
        &dir,
        &SigningCustodyState::from_parts(registry, h, own, ol, al, ps),
    );
    assert_eq!(
        launch(&dir, 181, &pins).err(),
        Some(install(CustodyRefusal::OwnLink))
    );
    let (registry, h, own, _, al, ps) = good.clone().into_parts();
    plant(
        &dir,
        &SigningCustodyState::from_parts(registry, h, own, Vec::new(), al, ps),
    );
    assert_eq!(
        launch(&dir, 181, &pins).err(),
        Some(install(CustodyRefusal::OwnLink))
    );
    plant(&dir, &good);
    assert!(launch(&dir, 181, &pins).is_ok());
}

// --- review cr-33e464: the commit point, end to end --------------------------------------

/// Fails the next state write once, at one phase, and — the crash-consistency oracle —
/// says whether the crash after an unconfirmed write keeps the new directory entry or the
/// old one.
struct FailOnce {
    phase: Option<PersistPhase>,
    lose: bool,
    armed: std::sync::atomic::AtomicBool,
}

impl KeystoreFaults for FailOnce {
    fn fail(&self, phase: PersistPhase) -> bool {
        self.phase == Some(phase) && self.armed.swap(false, std::sync::atomic::Ordering::SeqCst)
    }

    fn lose_rename(&self) -> bool {
        self.lose
    }
}

#[derive(Debug, Clone, Copy)]
enum Op {
    Mint,
    Rotate,
    Revoke,
    Adopt,
}

/// Launch over `dir` with a keystore whose next write fails at `phase` (none: no failure)
/// and whose crash oracle keeps (`lose == false`) or loses the rename.
fn launch_faulty(
    dir: &Path,
    start: u8,
    pins: &AllowedSigners,
    phase: Option<PersistPhase>,
    lose: bool,
) -> World {
    let store = LocalKeystore::new(dir).with_faults(std::sync::Arc::new(FailOnce {
        phase,
        lose,
        armed: std::sync::atomic::AtomicBool::new(true),
    }));
    World {
        daemon: builder(pins)
            .launch_signing(store, &signing_actor(), Box::new(Seeds(start)))
            .expect("launches")
            .build(),
        next: 0,
        version_override: None,
    }
}

fn perform(world: &mut World, op: Op, bundle: &[u8]) -> Option<ErrorCode> {
    let held = world.held();
    match op {
        Op::Mint => world
            .call(
                STEWARD,
                Arguments::SigningMint(SigningMintRequest {
                    kinds: vec![SignedArtifactKind::Receipt, SignedArtifactKind::DomainPack],
                }),
            )
            .error_code(),
        Op::Rotate => world.rotate().err(),
        Op::Revoke => world.revoke(&held),
        Op::Adopt => world
            .call(
                STEWARD,
                Arguments::IntentImportBundle(IntentImportBundleRequest {
                    content: bundle.to_vec(),
                }),
            )
            .error_code(),
    }
}

/// Review cr-33e464 round 2, with the crash-consistency oracle. For mint, rotate, revoke
/// (a compromise revocation of the held key) and adoption:
/// - success: the relaunch loads the new state, even with an oracle that loses every
///   unconfirmed rename — a success is only ever a confirmed record, so it can never later
///   reload the old state;
/// - a failure before the rename: `PublicationAborted`, nothing changed, live or
///   relaunched;
/// - a failure after the rename or at the directory sync: `OutcomeUnknown`, never success.
///   The authority is quarantined — no signature, no signing write — and the relaunch lands
///   in exactly the new state or exactly the old one, as the oracle chose, is validated in
///   full, signs again, and leaves no pending marker behind.
#[test]
fn every_answer_agrees_with_every_state_a_crash_can_leave() {
    let unconfirmed = [
        (Some(PersistPhase::AfterRename), false),
        (Some(PersistPhase::AfterRename), true),
        (Some(PersistPhase::DirectorySync), false),
        (Some(PersistPhase::DirectorySync), true),
    ];
    for op in [Op::Mint, Op::Rotate, Op::Revoke, Op::Adopt] {
        let cases = [(None, true), (Some(PersistPhase::BeforeRename), true)]
            .into_iter()
            .chain(unconfirmed);
        for (phase, lose) in cases {
            let case = format!("{op:?} at {phase:?}, lose={lose}");
            let dir = scratch(&format!("oracle-{op:?}-{phase:?}-{lose}"));
            let (bundle, pins, _, _) = peer_bundle(240);
            let mut setup = launch(&dir, 20, &pins).expect("first use");
            if matches!(op, Op::Mint) {
                let held = setup.held();
                assert_eq!(setup.revoke(&held), None, "a mint replaces a revoked key");
            }
            drop(setup);

            let mut world = launch_faulty(&dir, 60, &pins, phase, lose);
            let before = world.snapshot();
            let answer = perform(&mut world, op, &bundle);
            let live = world.snapshot();
            let quarantined = world.daemon.state().signing().custody_failed();
            // Probed only after a failure: after a success, a rotation would change the
            // state the relaunch is compared with.
            let refused_after =
                phase.map(|_| (world.sign_pack(b"pack").err(), world.rotate().err()));
            drop(world);
            let mut relaunched = launch(&dir, 100, &pins).expect("relaunches, validated");
            let reloaded = relaunched.snapshot();
            assert!(
                !dir.join(continuum_security::keystore::PENDING_FILE)
                    .exists(),
                "{case}: the relaunch reconciled and cleared the marker"
            );
            match phase {
                None => {
                    assert_eq!(answer, None, "{case}");
                    assert_ne!(live, before, "{case}");
                    assert_eq!(
                        reloaded, live,
                        "{case}: a success never reloads the old state"
                    );
                }
                Some(PersistPhase::BeforeRename) => {
                    assert_eq!(answer, Some(ErrorCode::PublicationAborted), "{case}");
                    assert_eq!(live, before, "{case}");
                    assert_eq!(reloaded, before, "{case}");
                    assert!(quarantined, "{case}");
                }
                Some(_) => {
                    assert_eq!(answer, Some(ErrorCode::OutcomeUnknown), "{case}");
                    assert!(quarantined, "{case}: quarantined");
                    assert_eq!(
                        refused_after,
                        Some((
                            Some(ErrorCode::PolicyGateFailed),
                            Some(ErrorCode::PublicationAborted)
                        )),
                        "{case}: no signature and no signing write until the relaunch"
                    );
                    let expected = if lose { &before } else { &live };
                    assert_ne!(live, before, "{case}");
                    assert_eq!(&reloaded, expected, "{case}: one of the two states, whole");
                }
            }
            // Whatever it reloaded, the relaunched authority is whole and signs.
            if reloaded.held().is_some_and(|held| {
                reloaded.registry().standing(held) == Some(&SignerStanding::Active)
            }) {
                let signature = relaunched.sign_pack(b"after").expect("signs");
                assert_eq!(
                    relaunched.verify_pack(b"after", signature),
                    SignatureOutcome::Verified,
                    "{case}"
                );
            }
        }
    }
}

/// `rule signing.custody`: a daemon with durable custody never lets a pre-3.9 client
/// receive a code its version does not define. The four signing writes are refused on a
/// 3.8 connection with `UnsupportedSemanticFeature`, before anything changes.
#[test]
fn a_custody_daemon_refuses_signing_writes_below_3_9() {
    let dir = scratch("below-3-9");
    let (bundle, pins, _, _) = peer_bundle(245);
    drop(launch(&dir, 190, &pins).expect("first use"));
    let before = files(&dir);
    let version = ProtocolVersion::new(3, 8);
    let hello = ClientHello {
        protocol_versions: VersionRange {
            low: version,
            high: version,
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuumd-signing-custody".to_owned(),
        actor: who("service:continuumd"),
        capability: cap("cap_root"),
        features: Optional::Absent,
    };
    let at_3_8 = negotiate(
        &[ProtocolVersion::new(3, 9), version],
        ProtocolWindow::new(3),
        ENCODINGS,
        &hello,
    )
    .expect("3.8 is served");
    assert_eq!(at_3_8.protocol_version(), version);
    let root = Some(cap("cap_root"));
    let mut world = World {
        daemon: Daemon::builder(Blake3Identity, at_3_8, cap("cap_root"))
            .now(Timestamp::new("2026-09-24T00:00:00.000Z").expect("timestamp"))
            .capability(
                CapabilityDescriptor {
                    delegation_depth: 5,
                    ..grant(
                        ("service:continuumd", "cap_root"),
                        AuthorityLevel::Promote,
                        &PRIVILEGES,
                    )
                },
                None,
            )
            .capability(
                grant(STEWARD, AuthorityLevel::ReviseIntent, &PRIVILEGES),
                root,
            )
            .family(SigningFamily)
            .family(IntentFamily)
            .allowed_signers(&pins)
            .launch_signing(
                LocalKeystore::new(&dir),
                &signing_actor(),
                Box::new(Seeds(191)),
            )
            .expect("launches")
            .build(),
        next: 0,
        version_override: Some(version),
    };
    let snapshot = world.snapshot();
    for op in [Op::Mint, Op::Rotate, Op::Revoke, Op::Adopt] {
        assert_eq!(
            perform(&mut world, op, &bundle),
            Some(ErrorCode::UnsupportedSemanticFeature),
            "{op:?}"
        );
    }
    assert_eq!(world.snapshot(), snapshot);
    assert!(!world.daemon.state().signing().custody_failed());
    // Signing itself is unaffected.
    assert!(world.sign_pack(b"pack").is_ok());
    drop(world);
    assert_eq!(files(&dir), before, "nothing was written");
}

/// Review cr-33e464: an own key revoked as compromised must keep its attested revocation
/// link, or export could not relay it and peers would keep trusting the key. An own key
/// other than the held one may not be active.
#[test]
fn a_compromised_own_key_without_its_revocation_link_is_refused() {
    let dir = scratch("compromised-link");
    let pins = AllowedSigners::new();
    let mut world = launch(&dir, 40, &pins).expect("first use");
    let held = world.held();
    assert_eq!(world.revoke(&held), None);
    let good = world.snapshot();
    drop(world);
    let install = |refusal| LaunchRefusal::Install(InstallRefusal::Custody(refusal));

    let (registry, h, own, ol, al, ps) = good.clone().into_parts();
    let without: Vec<SignerLink> = ol
        .into_iter()
        .filter(|link| {
            link.event()
                != &continuum_evidence::signing::LinkEvent::Revoked {
                    signer: held.clone(),
                }
        })
        .collect();
    plant(
        &dir,
        &SigningCustodyState::from_parts(registry, h, own, without, al, ps),
    );
    assert_eq!(
        launch(&dir, 41, &pins).err(),
        Some(install(CustodyRefusal::OwnLink))
    );

    let (mut registry, h, mut own, ol, al, ps) = good.clone().into_parts();
    let extra = registry
        .mint(&signing_actor(), &mut OneSeed([44; SEED_LEN]))
        .expect("mint");
    own.insert(
        extra.identity().clone(),
        [Kind::Receipt].into_iter().collect(),
    );
    plant(
        &dir,
        &SigningCustodyState::from_parts(registry, h, own, ol, al, ps),
    );
    assert_eq!(
        launch(&dir, 41, &pins).err(),
        Some(install(CustodyRefusal::OwnStanding))
    );

    plant(&dir, &good);
    assert!(launch(&dir, 41, &pins).is_ok());
}

// --- review cr-33e464 round 4: completeness of links and entries against the log ----------

/// Edits the own links, the adopted links, and the local revocations of a state.
type Edit<'a> = dyn Fn(&mut Vec<SignerLink>, &mut Vec<SignerLink>, &mut std::collections::BTreeSet<SignerIdentity>)
    + 'a;

/// A valid state after every kind of record export or a restart depends on: an own
/// rotation, the retired own key's compromise revocation (published from its pre-signed
/// link), an adopted peer rotation and revocation, a peer key revoked locally as
/// compromised, and one revoked locally as lost. It relaunches whole; removing any one
/// link or entry, or adding an entry no record accounts for, is refused.
#[test]
fn every_record_keeps_its_link_or_entry_across_a_restart() {
    let dir = scratch("completeness");
    let (bundle, pins, p1, p2) = peer_bundle(250);
    let (_, q1) = keyed(252);
    let (_, q2) = keyed(253);
    let (_, exporter) = keyed(254);
    let pins = pins
        .allow(q1.identity().clone(), BUNDLE_KINDS)
        .allow(q2.identity().clone(), BUNDLE_KINDS)
        .allow(exporter.identity().clone(), BUNDLE_KINDS);
    let mut world = launch(&dir, 110, &pins).expect("first use");
    let k0 = world.held();
    world.rotate().expect("rotates");
    assert_eq!(
        world.revoke(&k0),
        None,
        "the pre-signed revocation is published"
    );
    assert_eq!(world.import(bundle).0, SignatureOutcome::Verified);
    // Two more peers, introduced by the exporter's bundle, then revoked locally.
    let introduce = linked_bundle(
        &knowing(&[&exporter]),
        &exporter,
        AllowedSigners::new()
            .allow(exporter.identity().clone(), BUNDLE_KINDS)
            .allow(q1.identity().clone(), BUNDLE_KINDS)
            .allow(q2.identity().clone(), BUNDLE_KINDS),
        vec![
            knowing(&[&q1, &q2])
                .attest_rotation(&q1, &q2)
                .expect("active"),
        ],
    );
    assert_eq!(world.import(introduce).0, SignatureOutcome::Verified);
    assert_eq!(
        world.revoke(q2.identity()),
        None,
        "a local compromise revocation of a peer"
    );
    let lost = world.call(
        STEWARD,
        Arguments::SigningRevoke(SigningRevokeRequest {
            signer: handle(exporter.identity()),
            reason: RevocationReason::KeyLost,
        }),
    );
    assert_eq!(lost.error_code(), None, "a local loss revocation of a peer");
    let good = world.snapshot();
    assert_eq!(
        good.local_revocations().iter().collect::<Vec<_>>().len(),
        2,
        "q2 and the exporter"
    );
    drop(world);
    assert_eq!(
        launch(&dir, 111, &pins)
            .expect("the whole state relaunches")
            .snapshot(),
        good
    );

    let install = |refusal| LaunchRefusal::Install(InstallRefusal::Custody(refusal));
    let relaunch = |state: &SigningCustodyState| {
        plant(&dir, state);
        launch(&dir, 112, &pins).err()
    };
    let rebuild = |edit: &Edit<'_>| {
        let local = good.local_revocations().clone();
        let (registry, h, own, mut ol, mut al, ps) = good.clone().into_parts();
        let mut local = local;
        edit(&mut ol, &mut al, &mut local);
        SigningCustodyState::from_parts(registry, h, own, ol, al, ps).with_local_revocations(local)
    };
    let without = |links: &mut Vec<SignerLink>, event: continuum_evidence::signing::LinkEvent| {
        let before = links.len();
        links.retain(|link| link.event() != &event);
        assert_eq!(links.len(), before - 1, "the piece was present");
    };
    use continuum_evidence::signing::LinkEvent;

    // Codex's case: the adopted peer rotation's only relay link.
    let state = rebuild(&|_, al, _| {
        without(
            al,
            LinkEvent::Rotated {
                from: p1.identity().clone(),
                to: p2.identity().clone(),
            },
        );
    });
    assert_eq!(relaunch(&state), Some(install(CustodyRefusal::AdoptedLink)));
    // The adopted peer revocation's link.
    let state = rebuild(&|_, al, _| {
        without(
            al,
            LinkEvent::Revoked {
                signer: p1.identity().clone(),
            },
        );
    });
    assert_eq!(relaunch(&state), Some(install(CustodyRefusal::AdoptedLink)));
    // The adopted rotation that introduced q1 and q2.
    let state = rebuild(&|_, al, _| {
        without(
            al,
            LinkEvent::Rotated {
                from: q1.identity().clone(),
                to: q2.identity().clone(),
            },
        );
    });
    assert_eq!(relaunch(&state), Some(install(CustodyRefusal::AdoptedLink)));
    // The own rotation's link, and the own compromise revocation's link.
    let k1 = good.held().cloned().expect("held");
    let state = rebuild(&|ol, _, _| {
        without(
            ol,
            LinkEvent::Rotated {
                from: k0.clone(),
                to: k1.clone(),
            },
        );
    });
    assert_eq!(relaunch(&state), Some(install(CustodyRefusal::OwnLink)));
    let state = rebuild(&|ol, _, _| {
        without(ol, LinkEvent::Revoked { signer: k0.clone() });
    });
    assert_eq!(relaunch(&state), Some(install(CustodyRefusal::OwnLink)));
    // Each local revocation entry: the compromise one, then the loss one.
    let state = rebuild(&|_, _, local| {
        assert!(local.remove(q2.identity()));
    });
    assert_eq!(relaunch(&state), Some(install(CustodyRefusal::AdoptedLink)));
    let state = rebuild(&|_, _, local| {
        assert!(local.remove(exporter.identity()));
    });
    assert_eq!(
        relaunch(&state),
        Some(install(CustodyRefusal::LocalRevocation))
    );
    // The reverse: an entry no record accounts for. A local entry for a peer revoked by
    // an adopted link (it would then be both), for a peer still active, and for an own key.
    let state = rebuild(&|_, _, local| {
        local.insert(p1.identity().clone());
    });
    assert_eq!(relaunch(&state), Some(install(CustodyRefusal::AdoptedLink)));
    let state = rebuild(&|_, _, local| {
        local.insert(p2.identity().clone());
    });
    assert_eq!(
        relaunch(&state),
        Some(install(CustodyRefusal::LocalRevocation))
    );
    let state = rebuild(&|_, _, local| {
        local.insert(k0.clone());
    });
    assert_eq!(relaunch(&state), Some(install(CustodyRefusal::OwnLink)));
    // A link whose record the log does not hold (its reverse), for each side.
    let (_, x) = keyed(255);
    let stray = knowing(&[&x]).attest_revocation(&x).expect("active");
    let state = rebuild(&|_, al, _| al.push(stray.clone()));
    assert_eq!(relaunch(&state), Some(install(CustodyRefusal::AdoptedLink)));
    let state = rebuild(&|ol, _, _| ol.push(stray.clone()));
    assert_eq!(relaunch(&state), Some(install(CustodyRefusal::OwnLink)));

    plant(&dir, &good);
    assert!(
        launch(&dir, 113, &pins).is_ok(),
        "the whole state still relaunches"
    );
}

// --- review cr-1dc5ii round 2: unreconciled standing and cross-version replay -------------

/// A 3.8-negotiated daemon with the same principals and families, and no custody.
fn world_at_3_8(pins: &AllowedSigners) -> World {
    let version = ProtocolVersion::new(3, 8);
    let hello = ClientHello {
        protocol_versions: VersionRange {
            low: version,
            high: version,
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuumd-signing-custody".to_owned(),
        actor: who("service:continuumd"),
        capability: cap("cap_root"),
        features: Optional::Absent,
    };
    let at_3_8 = negotiate(
        &[ProtocolVersion::new(3, 9), version],
        ProtocolWindow::new(3),
        ENCODINGS,
        &hello,
    )
    .expect("3.8 is served");
    let root = Some(cap("cap_root"));
    World {
        daemon: Daemon::builder(Blake3Identity, at_3_8, cap("cap_root"))
            .now(Timestamp::new("2026-09-24T00:00:00.000Z").expect("timestamp"))
            .capability(
                CapabilityDescriptor {
                    delegation_depth: 5,
                    ..grant(
                        ("service:continuumd", "cap_root"),
                        AuthorityLevel::Promote,
                        &PRIVILEGES,
                    )
                },
                None,
            )
            .capability(
                grant(STEWARD, AuthorityLevel::ReviseIntent, &PRIVILEGES),
                root.clone(),
            )
            .capability(grant(READER, AuthorityLevel::Read, &[]), root)
            .family(SigningFamily)
            .family(IntentFamily)
            .allowed_signers(pins)
            .build(),
        next: 0,
        version_override: Some(version),
    }
}

/// Review cr-1dc5ii round 2, HIGH: while custody is unreconciled, no trust-deciding read
/// uses the live standing. An adoption answered `OutcomeUnknown` leaves the peer's facts in
/// memory, and then:
/// - a retry of the same bundle under a fresh key (zero facts left to apply) is refused,
///   and nothing is held or imported;
/// - `signing.verify` of a signature by the adopted peer is `standing-stale`, where the
///   same daemon before the fault answered `verified`;
/// - `signing.registry` refuses, typed.
///
/// A relaunch into either state (the crash oracle's two choices) serves the reconciled
/// standing again.
#[test]
fn unreconciled_standing_decides_no_trust() {
    for lose in [false, true] {
        let dir = scratch(&format!("unreconciled-{lose}"));
        let (bundle, pins, _, p2) = peer_bundle(236);
        let artifact = b"a body the peer signs".as_slice();
        let peer_signature = knowing(&[&p2])
            .sign(&p2, Kind::IntentBundle, &signed_bytes_identity(artifact))
            .expect("sign")
            .encode();
        let verify = |world: &mut World| -> SignatureOutcome {
            let outcome = world.call(
                READER,
                Arguments::SigningVerify(SigningVerifyRequest {
                    kind: SignedArtifactKind::IntentBundle,
                    artifact: artifact.to_vec(),
                    signature: Optional::Present(peer_signature.clone()),
                }),
            );
            let Payload::SigningVerify(verified) = outcome.payload else {
                panic!("verify failed: {:?}", outcome.error_code());
            };
            verified.outcome
        };
        drop(launch(&dir, 200, &pins).expect("first use"));
        let mut world = launch_faulty(&dir, 201, &pins, Some(PersistPhase::DirectorySync), lose);
        let answer = perform(&mut world, Op::Adopt, &bundle);
        assert_eq!(answer, Some(ErrorCode::OutcomeUnknown));
        assert!(world.daemon.state().signing().custody_unreconciled());
        // A fresh-key retry: nothing is left to apply, and still nothing is trusted — it is
        // refused before it is decoded, and nothing is held or imported.
        let held_before = world.snapshot();
        let retry = world.call(
            STEWARD,
            Arguments::IntentImportBundle(IntentImportBundleRequest {
                content: bundle.clone(),
            }),
        );
        assert_eq!(
            retry.error_code(),
            Some(ErrorCode::UnsupportedSemanticFeature)
        );
        assert_eq!(world.snapshot(), held_before);
        assert_eq!(verify(&mut world), SignatureOutcome::StandingStale);
        let registry = world.call(
            READER,
            Arguments::SigningRegistry(SigningRegistryRequest {}),
        );
        assert_eq!(
            registry.error_code(),
            Some(ErrorCode::UnsupportedSemanticFeature)
        );
        drop(world);
        // The relaunch reconciles, and the durable standing decides again.
        let mut relaunched = launch(&dir, 202, &pins).expect("relaunches");
        let expected = if lose {
            SignatureOutcome::StandingUnknown
        } else {
            SignatureOutcome::Verified
        };
        assert_eq!(verify(&mut relaunched), expected, "lose={lose}");
    }
}

/// Review cr-1dc5ii round 2, MEDIUM: a 3.9 `OutcomeUnknown` recorded in the replay ledger
/// and replayed on a 3.8 connection — state moved onto a newly negotiated daemon, the same
/// actor, key, and request — is refused as `UnsupportedSemanticFeature`, with nothing run
/// and nothing changed. A 3.8 client never receives a code its version does not define.
#[test]
fn a_recorded_outcome_unknown_is_never_replayed_to_a_3_8_client() {
    let dir = scratch("replay-3-8");
    let pins = AllowedSigners::new();
    drop(launch(&dir, 210, &pins).expect("first use"));
    let mut at_3_9 = launch_faulty(&dir, 211, &pins, Some(PersistPhase::DirectorySync), false);
    let request = || {
        Arguments::SigningRotate(SigningRotateRequest {
            signer: handle(&at_3_9_held()),
        })
    };
    fn at_3_9_held() -> SignerIdentity {
        // The first-use key of `Seeds(210)`.
        keyed(211).1.identity().clone()
    }
    let first = at_3_9.call_keyed(STEWARD, request(), Some("replayed"));
    assert_eq!(first.error_code(), Some(ErrorCode::OutcomeUnknown));
    // The same answer replays on 3.9.
    let again = at_3_9.call_keyed(STEWARD, request(), Some("replayed"));
    assert_eq!(again.error_code(), Some(ErrorCode::OutcomeUnknown));
    let snapshot = at_3_9.snapshot();

    let mut at_3_8 = world_at_3_8(&pins);
    std::mem::swap(at_3_8.daemon.state_mut(), at_3_9.daemon.state_mut());
    let replayed = at_3_8.call_keyed(STEWARD, request(), Some("replayed"));
    assert_eq!(
        replayed.error_code(),
        Some(ErrorCode::UnsupportedSemanticFeature),
        "a 3.8 client is never sent OutcomeUnknown"
    );
    assert_eq!(at_3_8.snapshot(), snapshot, "nothing ran");
}
