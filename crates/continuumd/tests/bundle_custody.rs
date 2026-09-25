//! Held intent bundles and import records across a restart (bn-3snfi, ADR-0054, plan
//! §4.2.1, TEST-9-07).
//!
//! bn-18w74 made the signing authority durable; this makes the bundles an import verified,
//! and the record of which contracts each import entered, durable with it: one custody
//! write per import, in the keystore's one state file, checked at launch before anything
//! is used or swept. Every test drives the real operations through `Daemon::dispatch` and
//! launches from the real `LocalKeystore`.
//!
//! | Property | Tests |
//! |---|---|
//! | restart round trip: import, restart, accept | `an_imported_bundle_survives_a_restart_and_its_proposal_accepts` |
//! | a restart is exactly the state after the import | `a_reimport_after_a_restart_changes_nothing_and_writes_nothing` |
//! | crash oracle over every write point | `every_import_answer_agrees_with_every_state_a_crash_can_leave` |
//! | corruption and tamper refused at launch, nothing swept | `a_tampered_held_bundle_or_import_record_is_refused_at_launch` |
//! | what is and is not recorded | `an_exported_bundle_is_recorded_only_once_it_is_imported`, `a_rejected_import_re_enters_at_proposed_after_a_restart_and_needs_its_bundle` |
//! | replay and version rules still hold | `a_replayed_import_is_answered_from_the_ledger_and_writes_nothing`, `an_import_below_3_9_is_refused_before_anything_is_recorded` |
//! | differential: daemon, store reader, and library agree | `the_restored_bundles_match_the_store_and_the_library` |

#![cfg(unix)]

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use continuum_evidence::actor::ActorId as SigningActor;
use continuum_evidence::signing::{
    AllowedSigners, EntropyUnavailable, KeyEntropy, SEED_LEN, SignedArtifactKind as Kind,
    SignerIdentity, Zeroizing,
};
use continuum_intent::canonical_json::Json as ContractJson;
use continuum_intent::contract::IntentContract;
use continuum_security::keystore::{
    KeystoreError, KeystoreFaults, LocalKeystore, PENDING_FILE, PersistPhase, STATE_FILE,
    decode_state, encode_state,
};
use continuum_value::epoch::ProtocolWindow;
use continuum_workspace::artifact_path::ArtifactClass;
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::signing::{CustodyRefusal, SigningFamily};
use continuumd::daemon::state::{IntentRecord, RegistryStatus};
use continuumd::daemon::{
    Daemon, InstallRefusal, LaunchRefusal, OperationOutcome, OperationRequest,
};
use continuumd::protocol::envelope::RequestEnvelope;
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, VersionRange, negotiate,
};
use continuumd::protocol::operations::intent::{
    IntentAcceptRequest, IntentExportBundleRequest, IntentImportBundleRequest, IntentRejectRequest,
};
use continuumd::protocol::operations::signing::SigningMintRequest;
use continuumd::protocol::registry::{self, ENCODINGS};
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, IntentBundleHandle, IntentHandle, Opaque, OperationName,
    ProtocolVersion, RequestId, Timestamp,
};
use continuumd::protocol::spec::{Annotation, Nullable, Optional};
use continuumd::protocol::vocabulary::{
    AuthorityLevel, Encoding, ErrorCode, SignatureOutcome, SignedArtifactKind, StructuralOutcome,
};

const DIE_HARD_CONTRACT: &str =
    include_str!("../../continuum-intent/tests/fixtures/die-hard-contract.json");

const STEWARD: (&str, &str) = ("human:steward", "cap_steward");
const PRIVILEGES: [&str; 7] = [
    "intent.accept",
    "intent.reject",
    "intent.export_bundle",
    "intent.import_bundle",
    "signing.mint",
    "signing.rotate",
    "signing.revoke",
];
const NOW: &str = "2026-09-24T00:00:00.000Z";
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

/// Counts every state write (each reaches `BeforeRename` once), and fails the `nth` one
/// (1-based) at `phase`; the crash oracle keeps (`lose == false`) or loses its rename.
struct Faults {
    nth: usize,
    phase: Option<PersistPhase>,
    lose: bool,
    writes: AtomicUsize,
}

impl Faults {
    fn new(nth: usize, phase: Option<PersistPhase>, lose: bool) -> Arc<Self> {
        Arc::new(Self {
            nth,
            phase,
            lose,
            writes: AtomicUsize::new(0),
        })
    }

    fn none() -> Arc<Self> {
        Self::new(usize::MAX, None, false)
    }

    fn writes(&self) -> usize {
        self.writes.load(Ordering::SeqCst)
    }
}

impl KeystoreFaults for Faults {
    fn fail(&self, phase: PersistPhase) -> bool {
        let current = if phase == PersistPhase::BeforeRename {
            self.writes.fetch_add(1, Ordering::SeqCst) + 1
        } else {
            self.writes.load(Ordering::SeqCst)
        };
        current == self.nth && self.phase == Some(phase)
    }

    fn lose_rename(&self) -> bool {
        self.lose
    }
}

fn cap(handle: &str) -> CapabilityHandle {
    CapabilityHandle::new(handle).expect("capability")
}

fn who(actor: &str) -> ActorId {
    ActorId::new(actor).expect("actor")
}

fn grant(principal: (&str, &str), level: AuthorityLevel) -> CapabilityDescriptor {
    CapabilityDescriptor {
        capability: cap(principal.1),
        actor: who(principal.0),
        level,
        snapshots: Vec::new(),
        intents: Vec::new(),
        artifact_classes: Vec::new(),
        expires_at: Nullable::Null,
        delegation_depth: 5,
        profile: Optional::Present(CapabilityProfile {
            privileged_operations: PRIVILEGES
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

fn negotiated(version: ProtocolVersion) -> Negotiated {
    let hello = ClientHello {
        protocol_versions: VersionRange {
            low: version,
            high: version,
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuumd-bundle-custody".to_owned(),
        actor: who("service:continuumd"),
        capability: cap("cap_root"),
        features: Optional::Absent,
    };
    negotiate(&[version], ProtocolWindow::new(3), ENCODINGS, &hello).expect("served")
}

fn builder(version: ProtocolVersion, pins: &AllowedSigners) -> continuumd::daemon::Builder {
    Daemon::builder(Blake3Identity, negotiated(version), cap("cap_root"))
        .now(Timestamp::new(NOW).expect("timestamp"))
        .capability(
            grant(("service:continuumd", "cap_root"), AuthorityLevel::Promote),
            None,
        )
        .capability(
            CapabilityDescriptor {
                delegation_depth: 3,
                ..grant(STEWARD, AuthorityLevel::ReviseIntent)
            },
            Some(cap("cap_root")),
        )
        .family(SigningFamily)
        .family(IntentFamily)
        .allowed_signers(pins)
}

struct World {
    daemon: Daemon,
    version: ProtocolVersion,
    next: u32,
}

/// A private scratch directory, unique per test; the keystore goes in `store` under it.
fn scratch(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/tmp")
        .join(format!("bundle-custody-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("scratch");
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("private");
    root.join("store")
}

/// Launch an importer over the keystore at `dir`, pinning `pins`, at protocol 3.9.
fn launch_with(
    dir: &Path,
    pins: &AllowedSigners,
    faults: Arc<Faults>,
) -> Result<World, LaunchRefusal<KeystoreError>> {
    launch_at(dir, pins, faults, ProtocolVersion::new(3, 9))
}

/// [`launch_with`] on a connection negotiated at `version`.
fn launch_at(
    dir: &Path,
    pins: &AllowedSigners,
    faults: Arc<Faults>,
    version: ProtocolVersion,
) -> Result<World, LaunchRefusal<KeystoreError>> {
    builder(version, pins)
        .launch_signing(
            LocalKeystore::new(dir).with_faults(faults),
            &SigningActor::new("human:steward").expect("actor"),
            Box::new(Seeds(100)),
        )
        .map(|builder| World {
            daemon: builder.build(),
            version,
            next: 0,
        })
}

fn launch(dir: &Path, pins: &AllowedSigners) -> World {
    launch_with(dir, pins, Faults::none()).expect("launches, validated")
}

/// An exporter with no custody: its own key signs bundles and acceptances.
fn exporter() -> (World, SignerIdentity) {
    let version = ProtocolVersion::new(3, 8);
    let mut world = World {
        daemon: builder(version, &AllowedSigners::new())
            .key_entropy(Box::new(Seeds(10)))
            .build(),
        version,
        next: 0,
    };
    let outcome = world.call(
        STEWARD,
        Arguments::SigningMint(SigningMintRequest {
            kinds: vec![
                SignedArtifactKind::IntentBundle,
                SignedArtifactKind::IntentAcceptance,
            ],
        }),
    );
    let Payload::SigningMint(minted) = outcome.payload else {
        panic!("mint failed: {:?}", outcome.error_code());
    };
    let identity = SignerIdentity::from_public_key(&minted.public_key).expect("a key");
    (world, identity)
}

/// An exporter holding one accepted contract, and the bundle it exports: its handle, its
/// bytes, the proposal it carries, and the pins an importer needs.
fn exported(variant: Option<u32>) -> (IntentBundleHandle, Vec<u8>, IntentHandle, AllowedSigners) {
    let (mut exporter, identity) = exporter();
    let proposal = exporter.put_accepted(&contract(variant));
    let (bundle, content) = exporter.export(vec![proposal.clone()]);
    let pins = AllowedSigners::new().allow(identity, BUNDLE_KINDS);
    (bundle, content, proposal, pins)
}

impl World {
    fn call(&mut self, principal: (&str, &str), arguments: Arguments) -> OperationOutcome {
        self.next += 1;
        let key = format!("k{}", self.next);
        self.call_keyed(principal, arguments, &key)
    }

    fn call_keyed(
        &mut self,
        principal: (&str, &str),
        arguments: Arguments,
        key: &str,
    ) -> OperationOutcome {
        let operation = arguments.operation();
        let spec = registry::operation(operation).expect("a declared operation");
        self.next += 1;
        let envelope = RequestEnvelope {
            protocol_version: self.version,
            request_id: RequestId::new(&format!("req_{}", self.next)).expect("request id"),
            idempotency_key: if spec.has(Annotation::Mutation) {
                Optional::Present(key.to_owned())
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

    fn put_accepted(&mut self, contract: &IntentContract) -> IntentHandle {
        let handle = intent_handle(contract);
        self.daemon.state_mut().put_intent(
            handle.clone(),
            IntentRecord {
                contract: contract.clone(),
                status: RegistryStatus::Proposed,
                supersedes: None,
                superseded_by: None,
                acceptance: None,
            },
        );
        let accepted = self.accept(&handle, None);
        assert_eq!(accepted.error_code(), None, "a local acceptance");
        handle
    }

    fn export(&mut self, intents: Vec<IntentHandle>) -> (IntentBundleHandle, Vec<u8>) {
        let outcome = self.call(
            STEWARD,
            Arguments::IntentExportBundle(IntentExportBundleRequest { intents }),
        );
        let Payload::IntentExportBundle(exported) = outcome.payload else {
            panic!("export failed: {:?}", outcome.envelope.error);
        };
        (exported.bundle, exported.content)
    }

    fn import_keyed(&mut self, content: &[u8], key: &str) -> OperationOutcome {
        self.call_keyed(
            STEWARD,
            Arguments::IntentImportBundle(IntentImportBundleRequest {
                content: content.to_vec(),
            }),
            key,
        )
    }

    fn import(&mut self, content: &[u8]) -> OperationOutcome {
        self.call(
            STEWARD,
            Arguments::IntentImportBundle(IntentImportBundleRequest {
                content: content.to_vec(),
            }),
        )
    }

    /// `intent.accept`. With a bundle, present the acceptance the held bundle's record
    /// carries for `proposal`; without one, a local acceptance by the admitted steward now.
    fn accept(
        &mut self,
        proposal: &IntentHandle,
        bundle: Option<&IntentBundleHandle>,
    ) -> OperationOutcome {
        let (by, signature) = match bundle {
            Some(bundle) => (
                STEWARD.0.to_owned(),
                self.daemon
                    .state()
                    .signing()
                    .bundle(bundle)
                    .and_then(|held| held.body().contract(proposal))
                    .and_then(|entry| record_acceptance(&entry.record))
                    .map_or_else(|| "not-held".to_owned(), |(_, signature)| signature),
            ),
            None => (STEWARD.0.to_owned(), "caller-attestation".to_owned()),
        };
        let at = match bundle {
            Some(bundle) => self
                .daemon
                .state()
                .signing()
                .bundle(bundle)
                .and_then(|held| held.body().contract(proposal))
                .and_then(|entry| record_acceptance(&entry.record))
                .map_or_else(|| NOW.to_owned(), |(at, _)| at),
            None => NOW.to_owned(),
        };
        self.call(
            STEWARD,
            Arguments::IntentAccept(IntentAcceptRequest {
                proposal: proposal.clone(),
                acceptance: acceptance(&by, &signature, &at),
                bundle: bundle.map_or(Optional::Absent, |bundle| Optional::Present(bundle.clone())),
            }),
        )
    }

    fn reject(&mut self, proposal: &IntentHandle) -> Option<ErrorCode> {
        self.call(
            STEWARD,
            Arguments::IntentReject(IntentRejectRequest {
                proposal: proposal.clone(),
                reason: "not wanted here".to_owned(),
            }),
        )
        .error_code()
    }

    fn status(&self, proposal: &IntentHandle) -> Option<RegistryStatus> {
        self.daemon
            .state()
            .intent(proposal)
            .map(|record| record.status)
    }

    fn holds(&self, bundle: &IntentBundleHandle) -> bool {
        self.daemon.state().signing().bundle(bundle).is_some()
    }
}

/// The acceptance `timestamp` and `signature` a registry record's JSON carries.
fn record_acceptance(record: &[u8]) -> Option<(String, String)> {
    let Ok(ContractJson::Object(fields)) = ContractJson::parse(record) else {
        return None;
    };
    let Some(ContractJson::Object(acceptance)) = fields.get("acceptance") else {
        return None;
    };
    match (acceptance.get("timestamp"), acceptance.get("signature")) {
        (Some(ContractJson::String(at)), Some(ContractJson::String(signature))) => {
            Some((at.clone(), signature.clone()))
        }
        _ => None,
    }
}

fn acceptance(by: &str, signature: &str, at: &str) -> Opaque {
    let mut fields = BTreeMap::new();
    for (key, value) in [
        ("accepted_by", by),
        ("capability", "revise-intent"),
        ("signature", signature),
        ("timestamp", at),
    ] {
        fields.insert(key.to_owned(), ContractJson::String(value.to_owned()));
    }
    Opaque::from_bytes(ContractJson::Object(fields).to_canonical_bytes())
}

/// The Die Hard contract, stamped with the `in_` its preimage names (W9); a variant changes
/// a bound in the preimage, so each variant is its own identity.
fn contract(variant: Option<u32>) -> IntentContract {
    let text = DIE_HARD_CONTRACT.trim_end();
    let text = match variant {
        Some(faults) => text.replace("\"faults\":0", &format!("\"faults\":{faults}")),
        None => text.to_owned(),
    };
    let unstamped = IntentContract::decode(text.as_bytes()).expect("the fixture decodes");
    let handle = intent_handle(&unstamped);
    let stamped = text.replacen(
        "\"intent_id\":\"in_die_hard_v1\"",
        &format!("\"intent_id\":\"{}\"", handle.as_str()),
        1,
    );
    IntentContract::decode(stamped.as_bytes()).expect("the stamped fixture decodes")
}

fn intent_handle(contract: &IntentContract) -> IntentHandle {
    let stored = continuum_workspace::publication::ContentIdentifier::identify(
        &Blake3Identity,
        ArtifactClass::IntentContract,
        &contract.identity_preimage_bytes(),
    )
    .expect("blake3 names every input");
    continuumd::daemon::identity::intent_to_wire(&stored).expect("an in_ handle")
}

fn import_outcome(outcome: &OperationOutcome) -> (SignatureOutcome, u32, usize) {
    let Payload::IntentImportBundle(imported) = &outcome.payload else {
        panic!("import failed: {:?}", outcome.envelope.error);
    };
    (imported.outcome, imported.adopted, imported.imported.len())
}

/// Every file of the store but the (empty) lock file, by name, with its bytes.
fn files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fs::read_dir(dir)
        .expect("store")
        .map(|entry| entry.expect("entry"))
        .filter(|entry| entry.file_name() != continuum_security::keystore::LOCK_FILE)
        .map(|entry| {
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

// --- the round trip --------------------------------------------------------------------------

/// Import, restart, accept: the bundle held before the restart is held after it, its
/// contract is back in the registry at `proposed` as an import, and the plan §4.2.1 CI
/// acceptance through it succeeds with no second import. A local acceptance of the
/// imported proposal is still refused: it is accepted only through a verified bundle.
#[test]
fn an_imported_bundle_survives_a_restart_and_its_proposal_accepts() {
    let dir = scratch("round-trip");
    let (bundle, content, proposal, pins) = exported(None);
    let mut first = launch(&dir, &pins);
    let (outcome, _, entered) = import_outcome(&first.import(&content));
    assert_eq!((outcome, entered), (SignatureOutcome::Verified, 1));
    let before = first.daemon.state().signing().custody_snapshot();
    assert_eq!(
        before.held_bundles().len(),
        1,
        "the custody records the bundle"
    );
    assert_eq!(
        before.imports().get(proposal.as_str()).map(String::as_str),
        Some(bundle.as_str())
    );
    drop(first);

    let mut second = launch(&dir, &pins);
    assert_eq!(
        second.daemon.state().signing().custody_snapshot(),
        before,
        "the restart restores the custody state whole"
    );
    assert!(
        second.holds(&bundle),
        "the bundle is held without a re-import"
    );
    assert_eq!(second.status(&proposal), Some(RegistryStatus::Proposed));
    assert_eq!(
        second.daemon.state().signing().imported_from(&proposal),
        Some(&bundle)
    );
    assert_eq!(
        second.accept(&proposal, None).error_code(),
        Some(ErrorCode::AcceptanceChainInvalid),
        "an imported proposal is never accepted locally, before or after a restart"
    );
    let accepted = second.accept(&proposal, Some(&bundle));
    assert_eq!(accepted.error_code(), None, "{:?}", accepted.envelope.error);
    assert_eq!(second.status(&proposal), Some(RegistryStatus::Accepted));
}

/// Metamorphic, snapshot restore versus root replay: a restart lands exactly where the
/// import left the daemon. Importing the same bytes again (replaying the root operation)
/// finds the bundle held, the contract entered, and the facts adopted by the restore, so
/// it changes nothing and writes nothing to the custody.
#[test]
fn a_reimport_after_a_restart_changes_nothing_and_writes_nothing() {
    let dir = scratch("reimport");
    let (_, content, _, pins) = exported(Some(3));
    let mut first = launch(&dir, &pins);
    let (_, adopted, entered) = import_outcome(&first.import(&content));
    assert!(
        adopted > 0 && entered == 1,
        "anti-vacuity: the import did change things"
    );
    drop(first);

    let faults = Faults::none();
    let mut second = launch_with(&dir, &pins, Arc::clone(&faults)).expect("relaunches");
    let written = faults.writes();
    let again = second.import(&content);
    assert_eq!(import_outcome(&again), (SignatureOutcome::Verified, 0, 0));
    let Some(verdict) = again.envelope.verdict.value() else {
        panic!("an import answers a verdict");
    };
    assert!(
        format!("{verdict:?}").contains(&format!("{:?}", StructuralOutcome::Unchanged)),
        "{verdict:?}"
    );
    assert_eq!(faults.writes(), written, "nothing new to record");
}

// --- the crash oracle ------------------------------------------------------------------------

/// What a relaunch shows of one import: whether the bundle is held, whether its contract is
/// in the registry, whether its signer's standing was adopted, and whether the proposal
/// accepts through it.
fn observe(
    dir: &Path,
    pins: &AllowedSigners,
    bundle: &IntentBundleHandle,
    proposal: &IntentHandle,
    signer: &SignerIdentity,
) -> (bool, bool, bool, bool) {
    let mut world = launch(dir, pins);
    assert!(!dir.join(PENDING_FILE).exists(), "the relaunch reconciled");
    let held = world.holds(bundle);
    let entered = world.status(proposal) == Some(RegistryStatus::Proposed);
    let adopted = world
        .daemon
        .state()
        .signing()
        .registry()
        .standing(signer)
        .is_some();
    let accepts = entered && world.accept(proposal, Some(bundle)).error_code().is_none();
    (held, entered, adopted, accepts)
}

/// The import's one custody write, failed at every phase, with the crash oracle keeping or
/// losing an unconfirmed rename:
/// - success: the relaunch holds the bundle, the contract, and the adopted standing, and
///   accepts through the bundle — even when the oracle would lose an unconfirmed rename;
/// - before the rename: `PublicationAborted`, nothing held, entered, or adopted, live or
///   relaunched;
/// - after the rename or at the directory sync: `OutcomeUnknown`, never success; the live
///   daemon refuses the bundle acceptance (quarantined), and the relaunch holds all of the
///   import or none of it, as the oracle chose — never a bundle without its contract or
///   its facts.
#[test]
fn every_import_answer_agrees_with_every_state_a_crash_can_leave() {
    let (bundle, content, proposal, pins) = exported(Some(5));
    let signer = pins.iter().next().expect("one pin").0.clone();
    let cases = [
        (None, true),
        (Some(PersistPhase::BeforeRename), false),
        (Some(PersistPhase::AfterRename), false),
        (Some(PersistPhase::AfterRename), true),
        (Some(PersistPhase::DirectorySync), false),
        (Some(PersistPhase::DirectorySync), true),
    ];
    for (phase, lose) in cases {
        let case = format!("{phase:?}, lose={lose}");
        let dir = scratch(&format!("oracle-{phase:?}-{lose}"));
        // Write 1 is the first-use mint; write 2 is the import.
        let mut world = launch_with(&dir, &pins, Faults::new(2, phase, lose)).expect("first use");
        let answer = world.import(&content).error_code();
        let live_held = world.holds(&bundle);
        let live_accept = world.accept(&proposal, Some(&bundle)).error_code();
        drop(world);
        let seen = observe(&dir, &pins, &bundle, &proposal, &signer);
        match phase {
            None => {
                assert_eq!(answer, None, "{case}");
                assert_eq!(live_accept, None, "{case}");
                assert_eq!(seen, (true, true, true, true), "{case}");
            }
            Some(PersistPhase::BeforeRename) => {
                assert_eq!(answer, Some(ErrorCode::PublicationAborted), "{case}");
                assert!(!live_held, "{case}: nothing held after a refused write");
                assert_eq!(seen, (false, false, false, false), "{case}");
            }
            Some(_) => {
                assert_eq!(answer, Some(ErrorCode::OutcomeUnknown), "{case}");
                assert_eq!(
                    live_accept,
                    Some(ErrorCode::AcceptanceChainInvalid),
                    "{case}: quarantined until the relaunch"
                );
                let expected = if lose {
                    (false, false, false, false)
                } else {
                    (true, true, true, true)
                };
                assert_eq!(seen, expected, "{case}: one whole state");
            }
        }
    }
}

// --- corruption and tamper -------------------------------------------------------------------

/// Rewrite the state file with `edit` applied to its decoded state.
fn tamper(
    dir: &Path,
    edit: impl FnOnce(
        continuum_evidence::signing::SigningCustodyState,
    ) -> continuum_evidence::signing::SigningCustodyState,
) {
    let path = dir.join(STATE_FILE);
    let state = decode_state(&fs::read(&path).expect("state")).expect("decodes");
    rewrite(&path, &encode_state(&edit(state)).expect("encodes"));
}

/// Every way a stored bundle or import record can disagree with itself is refused at
/// launch, typed, before any key is used or anything is swept, and restoring the original
/// bytes launches again:
/// - a byte of a held bundle changed (it is no longer the identity it is recorded under);
/// - a bundle recorded under another well-formed handle;
/// - a bundle that is not a bundle at all;
/// - a well-formed bundle whose signature was replaced;
/// - an import record naming a contract its bundle does not export;
/// - an import record under a handle that is not an `in_`;
/// - a held bundle removed while an import record still names it is refused by the store's
///   own reader (the record's bundle index dangles), and the store is left as it was.
#[test]
fn a_tampered_held_bundle_or_import_record_is_refused_at_launch() {
    let dir = scratch("tamper");
    let (bundle, content, proposal, pins) = exported(Some(7));
    let (_, other_content, _, _) = exported(Some(8));
    let mut first = launch(&dir, &pins);
    assert_eq!(import_outcome(&first.import(&content)).2, 1);
    drop(first);
    let original = files(&dir);
    let refused = |dir: &Path| match launch_with(dir, &pins, Faults::none()) {
        Err(LaunchRefusal::Install(InstallRefusal::Custody(refusal))) => Some(refusal),
        Err(other) => panic!("another refusal: {other:?}"),
        Ok(_) => None,
    };
    let key = bundle.as_str().to_owned();

    let flip = |state: continuum_evidence::signing::SigningCustodyState| {
        let mut bundles = state.held_bundles().clone();
        let imports = state.imports().clone();
        let mut bytes = bundles[&key].to_vec();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        bundles.insert(key.clone(), Arc::from(bytes));
        state.with_held_bundles(bundles, imports)
    };
    let renamed = |state: continuum_evidence::signing::SigningCustodyState| {
        let mut bundles = state.held_bundles().clone();
        let bytes = bundles.remove(&key).expect("held");
        let other = format!("inb_{}", "0".repeat(64));
        let imports = state
            .imports()
            .keys()
            .map(|intent| (intent.clone(), other.clone()))
            .collect();
        bundles.insert(other, bytes);
        state.with_held_bundles(bundles, imports)
    };
    let garbage = |state: continuum_evidence::signing::SigningCustodyState| {
        let mut bundles = state.held_bundles().clone();
        let imports = state.imports().clone();
        bundles.insert(key.clone(), Arc::from(b"not a bundle".as_slice()));
        state.with_held_bundles(bundles, imports)
    };
    let resigned = |state: continuum_evidence::signing::SigningCustodyState| {
        // The other bundle's signature, under this bundle's body: well framed, not
        // authentic for this body.
        let mut bundles = state.held_bundles().clone();
        let imports = state.imports().clone();
        let mine = continuumd::daemon::bundle::decode_signed(&bundles[&key]).expect("decodes");
        let theirs = continuumd::daemon::bundle::decode_signed(&other_content).expect("decodes");
        let forged =
            continuumd::daemon::bundle::encode_signed(mine.body_bytes(), theirs.signature());
        bundles.insert(key.clone(), Arc::from(forged));
        state.with_held_bundles(bundles, imports)
    };
    let not_exported = |state: continuum_evidence::signing::SigningCustodyState| {
        let bundles = state.held_bundles().clone();
        let mut imports = state.imports().clone();
        imports.insert(
            intent_handle(&contract(Some(9))).as_str().to_owned(),
            key.clone(),
        );
        state.with_held_bundles(bundles, imports)
    };
    let not_an_intent = |state: continuum_evidence::signing::SigningCustodyState| {
        let bundles = state.held_bundles().clone();
        let mut imports = state.imports().clone();
        imports.insert("inb_not_an_intent".to_owned(), key.clone());
        state.with_held_bundles(bundles, imports)
    };
    type Edit<'a> = Box<
        dyn Fn(
                continuum_evidence::signing::SigningCustodyState,
            ) -> continuum_evidence::signing::SigningCustodyState
            + 'a,
    >;
    let edits: Vec<(&str, Edit<'_>, CustodyRefusal)> = vec![
        ("a flipped byte", Box::new(flip), CustodyRefusal::HeldBundle),
        (
            "another handle",
            Box::new(renamed),
            CustodyRefusal::HeldBundle,
        ),
        ("garbage", Box::new(garbage), CustodyRefusal::HeldBundle),
        (
            "a foreign signature",
            Box::new(resigned),
            CustodyRefusal::HeldBundle,
        ),
        (
            "a contract not exported",
            Box::new(not_exported),
            CustodyRefusal::ImportRecord,
        ),
        (
            "a record that is not an in_",
            Box::new(not_an_intent),
            CustodyRefusal::ImportRecord,
        ),
    ];
    for (what, edit, expected) in edits {
        tamper(&dir, edit);
        let tampered = files(&dir);
        assert_eq!(refused(&dir), Some(expected), "{what}");
        assert_eq!(
            files(&dir),
            tampered,
            "{what}: nothing was swept or repaired"
        );
        rewrite(&dir.join(STATE_FILE), &original[STATE_FILE]);
    }

    // The store's reader refuses a record whose bundle is gone.
    let mut bytes = original[STATE_FILE].clone();
    let at = bytes
        .windows(content.len())
        .position(|window| window == content.as_slice())
        .expect("the bundle is in the state file");
    // Claim the bundle count is zero: the bundle's bytes are then read as the import
    // section, which does not hold.
    let count_at = at - 4 - 4 - bundle.as_str().len() - 4;
    bytes[count_at..count_at + 4].copy_from_slice(&0u32.to_be_bytes());
    rewrite(&dir.join(STATE_FILE), &bytes);
    assert!(matches!(
        launch_with(&dir, &pins, Faults::none()),
        Err(LaunchRefusal::Custody(KeystoreError::Corrupt(_)))
    ));
    rewrite(&dir.join(STATE_FILE), &original[STATE_FILE]);

    // Anti-vacuity: the original store launches and still accepts through the bundle.
    let mut restored = launch(&dir, &pins);
    assert_eq!(restored.accept(&proposal, Some(&bundle)).error_code(), None);
}

// --- what is and is not recorded -------------------------------------------------------------

/// A bundle this daemon exported is held in memory only: `intent.export_bundle` has no
/// `OutcomeUnknown` to answer an unconfirmed write with, so it records nothing. Importing
/// the same bytes records it, and it then survives a restart.
#[test]
fn an_exported_bundle_is_recorded_only_once_it_is_imported() {
    let dir = scratch("exported");
    let mut world = launch(&dir, &AllowedSigners::new());
    let proposal = world.put_accepted(&contract(Some(11)));
    let (bundle, content) = world.export(vec![proposal.clone()]);
    assert!(world.holds(&bundle));
    assert!(
        !world.daemon.state().signing().records_bundle(&bundle),
        "an export records nothing"
    );
    drop(world);
    let mut world = launch(&dir, &AllowedSigners::new());
    assert!(
        !world.holds(&bundle),
        "an unrecorded export does not survive"
    );
    let (outcome, _, _) = import_outcome(&world.import(&content));
    assert_eq!(
        outcome,
        SignatureOutcome::Verified,
        "its own bundle verifies"
    );
    assert!(world.daemon.state().signing().records_bundle(&bundle));
    drop(world);
    assert!(launch(&dir, &AllowedSigners::new()).holds(&bundle));
}

/// `intent.reject` records nothing (it has no custody failure to answer), and the registry
/// itself is not persisted: a rejected import leaves the registry now, and a restart
/// re-enters it at `proposed` from its import record — exactly what importing its bundle
/// again would do. It is still accepted only through its bundle.
#[test]
fn a_rejected_import_re_enters_at_proposed_after_a_restart_and_needs_its_bundle() {
    let dir = scratch("rejected");
    let (bundle, content, proposal, pins) = exported(Some(13));
    let mut world = launch(&dir, &pins);
    assert_eq!(import_outcome(&world.import(&content)).2, 1);
    assert_eq!(world.reject(&proposal), None);
    assert_eq!(
        world.status(&proposal),
        None,
        "rejected: out of the registry"
    );
    assert!(
        world
            .daemon
            .state()
            .signing()
            .imported_from(&proposal)
            .is_none()
    );
    assert!(
        world
            .daemon
            .state()
            .signing()
            .import_records()
            .contains_key(&proposal),
        "the custody's import record stays"
    );
    drop(world);
    let mut world = launch(&dir, &pins);
    assert_eq!(world.status(&proposal), Some(RegistryStatus::Proposed));
    assert_eq!(
        world.accept(&proposal, None).error_code(),
        Some(ErrorCode::AcceptanceChainInvalid)
    );
    assert_eq!(world.accept(&proposal, Some(&bundle)).error_code(), None);
}

// --- replay and version ----------------------------------------------------------------------

/// A replayed import — the same idempotency key — is answered from the ledger: it runs
/// nothing again and writes nothing to the custody.
#[test]
fn a_replayed_import_is_answered_from_the_ledger_and_writes_nothing() {
    let dir = scratch("replay");
    let (bundle, content, _, pins) = exported(Some(15));
    let faults = Faults::none();
    let mut world = launch_with(&dir, &pins, Arc::clone(&faults)).expect("first use");
    let first = world.import_keyed(&content, "the-import");
    assert_eq!(first.error_code(), None);
    let written = faults.writes();
    let replayed = world.import_keyed(&content, "the-import");
    assert_eq!(
        import_outcome(&replayed),
        import_outcome(&first),
        "the recorded answer"
    );
    assert_eq!(faults.writes(), written, "a replay writes nothing");
    assert!(world.holds(&bundle));
}

/// `rule signing.custody`: on a 3.8 connection a daemon with durable custody refuses an
/// import with `UnsupportedSemanticFeature` before anything is held, entered, or written.
#[test]
fn an_import_below_3_9_is_refused_before_anything_is_recorded() {
    let dir = scratch("below-3-9");
    let (bundle, content, proposal, pins) = exported(Some(17));
    drop(launch(&dir, &pins));
    let before = files(&dir);
    let mut world = launch_at(&dir, &pins, Faults::none(), ProtocolVersion::new(3, 8))
        .expect("launches at 3.8");
    assert_eq!(
        world.import(&content).error_code(),
        Some(ErrorCode::UnsupportedSemanticFeature)
    );
    assert!(!world.holds(&bundle));
    assert_eq!(world.status(&proposal), None);
    drop(world);
    assert_eq!(files(&dir), before, "nothing was written");
}

// --- differential ----------------------------------------------------------------------------

/// Differential (oracle: `continuum_security`'s state-file reader, and `continuum_evidence`'s
/// signature check; subject: `continuumd` restored through the launcher). After imports of
/// two bundles and a restart, the custody state the daemon restored is exactly what the
/// keystore's reader decodes from the file on disk, which is exactly what the daemon held
/// before the restart; every bundle the daemon holds again is byte-identical to the one on
/// disk and authenticated by the library over its body.
#[test]
fn the_restored_bundles_match_the_store_and_the_library() {
    let dir = scratch("differential");
    let (first_bundle, first_content, _, first_pins) = exported(Some(21));
    let (second_bundle, second_content, _, second_pins) = exported(Some(22));
    let pins = first_pins
        .iter()
        .chain(second_pins.iter())
        .fold(AllowedSigners::new(), |set, (signer, kinds)| {
            set.allow(signer.clone(), kinds.iter().copied())
        });
    let mut world = launch(&dir, &pins);
    for content in [&first_content, &second_content] {
        assert_eq!(
            import_outcome(&world.import(content)).0,
            SignatureOutcome::Verified
        );
    }
    let live = world.daemon.state().signing().custody_snapshot();
    drop(world);

    let world = launch(&dir, &pins);
    let restored = world.daemon.state().signing().custody_snapshot();
    let on_disk = decode_state(&fs::read(dir.join(STATE_FILE)).expect("state")).expect("decodes");
    assert_eq!(
        restored, on_disk,
        "the daemon restored what the store holds"
    );
    assert_eq!(restored, live, "and what it held before the restart");
    assert_eq!(restored.held_bundles().len(), 2);
    for (handle, content) in [
        (&first_bundle, &first_content),
        (&second_bundle, &second_content),
    ] {
        let held = world
            .daemon
            .state()
            .signing()
            .bundle(handle)
            .expect("held again");
        assert_eq!(held.content().as_ref(), content.as_slice());
        assert_eq!(
            &*on_disk.held_bundles()[handle.as_str()],
            content.as_slice()
        );
        let signature = continuum_evidence::signing::ArtifactSignature::decode(held.signature())
            .expect("a signature record");
        assert!(signature.authenticates(
            Kind::IntentBundle,
            &continuumd::daemon::bundle::signed_bytes_identity(held.body_bytes())
        ));
    }
}
