//! The `workspace` and `intent` operation families, and the dispatch skeleton under them.
//!
//! # What these tests are evidence for
//!
//! Every scenario below runs through [`Daemon::dispatch`] — the same entry point a
//! transport will call — and nothing else. There is no filesystem, no clock, no runtime and
//! no network in this file: the Die Hard workspace arrives as staged content, the Die Hard
//! Intent Contract arrives as a checked-in fixture decoded by `continuum-intent`, a
//! capability's expiry is judged against a supplied reading or not at all, and every
//! identity is a pure function of bytes. That is the claim INV-005 and ADR-0003 make about
//! the daemon core, stated as a property of the test rather than of the prose.
//!
//! # The fixtures, and why they are these fixtures
//!
//! `DieHard.ctm` is `notes/plan/corpus/tla-examples/ports/TV-009/`'s port, and the Intent
//! Contract is `continuum-intent`'s own Die Hard fixture. Both are `include_str!`'d rather
//! than copied: there is one Die Hard model and one Die Hard contract in this repository,
//! and a second copy of either would be a second Die Hard that could drift from the first.
//! The pattern is the workspace's — `continuum-intent`'s suites `include_str!` the dossier's
//! schemas from `notes/plan/` the same way.
//!
//! # The capability tree
//!
//! One connection capability, `cap_root`, with every other capability a registered
//! *delegation* of it. That is not decoration: RFC 0027 permits a request to present a
//! narrower capability than the connection's and defines narrower as "the connection's own
//! capability or a descendant of it in the delegation tree" whose admission set is a subset,
//! so a test suite whose capabilities are unrelated roots would never exercise D6/D7 at all.

use std::collections::BTreeMap;

use continuum_intent::canonical_json::Json;
use continuum_intent::change_policy::{PolicyField, PolicyVerb};
use continuum_intent::contract::IntentContract;
use continuum_value::epoch::ProtocolWindow;
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::state::{IntentRecord, RegistryStatus};
use continuumd::daemon::workspace::WorkspaceFamily;
use continuumd::daemon::{
    Daemon, OperationOutcome, OperationRequest, errors, identity, intent, workspace,
};
use continuumd::protocol::envelope::{Budget, RequestEnvelope, Verdict};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, VersionRange, negotiate,
};
use continuumd::protocol::operations::intent::{
    IntentAcceptRequest, IntentGetRequest, IntentLockRequest, IntentProposeRevisionRequest,
};
use continuumd::protocol::operations::workspace::{
    WorkspaceCreateRequest, WorkspaceDiffRequest, WorkspaceForkRequest, WorkspaceSealRequest,
};
use continuumd::protocol::registry::{self, ENCODINGS};
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, Commitment, EpochIdentity, IntentHandle, Opaque, OperationName,
    ProtocolVersion, RequestId, Timestamp, WorkspaceHandle,
};
use continuumd::protocol::shared::{
    FileOverlay, IntentChangeSet, SnapshotComponents, SnapshotEpochs,
};
use continuumd::protocol::spec::{Nullable, Optional};
use continuumd::protocol::vocabulary::{
    AuthorityLevel, DiffLayer, Encoding, ErrorCode, ResultStatus, StructuralOutcome,
};

use continuum_workspace::snapshot::WorkspacePath;

/// The TV-009 port's model, verbatim.
const DIE_HARD_MODEL: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm");

/// The TV-009 port's default model configuration.
const DIE_HARD_CONFIG: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/default.model.toml");

/// The Die Hard Intent Contract, as `continuum-intent`'s own suites use it.
const DIE_HARD_CONTRACT: &str =
    include_str!("../../continuum-intent/tests/fixtures/die-hard-contract.json");

// --- fixtures ----------------------------------------------------------------------------

fn version() -> ProtocolVersion {
    ProtocolVersion::new(3, 1)
}

fn cap(handle: &str) -> CapabilityHandle {
    CapabilityHandle::new(handle).expect("a well-formed capability handle")
}

fn who(actor: &str) -> ActorId {
    ActorId::new(actor).expect("a well-formed actor identity")
}

fn name(operation: &str) -> OperationName {
    OperationName::new(operation).expect("a well-formed operation name")
}

fn profile(privileged: &[&str], denied: &[&str], sharing: bool) -> CapabilityProfile {
    CapabilityProfile {
        privileged_operations: privileged.iter().map(|entry| name(entry)).collect(),
        denied_operations: denied.iter().map(|entry| name(entry)).collect(),
        data_grants: Vec::new(),
        cross_principal_sharing: sharing,
    }
}

fn grant(
    handle: &str,
    actor: &str,
    level: AuthorityLevel,
    depth: u32,
    profile: Optional<CapabilityProfile>,
) -> CapabilityDescriptor {
    CapabilityDescriptor {
        capability: cap(handle),
        actor: who(actor),
        level,
        snapshots: Vec::new(),
        intents: Vec::new(),
        artifact_classes: Vec::new(),
        expires_at: Nullable::Null,
        delegation_depth: depth,
        profile,
    }
}

/// The five `@privileged` operations, which is what a root capability may delegate.
const PRIVILEGED: &[&str] = &[
    "intent.accept",
    "intent.reject",
    "intent.lock",
    "repair.promote",
    "repair.reject",
];

fn negotiated() -> Negotiated {
    let hello = ClientHello {
        protocol_versions: VersionRange {
            low: version(),
            high: version(),
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuumd-daemon-operations-test".to_owned(),
        actor: who("service:continuumd"),
        capability: cap("cap_root"),
        features: Optional::Absent,
    };
    negotiate(&[version()], ProtocolWindow::new(3), ENCODINGS, &hello).expect("3.1 is served")
}

/// A daemon with the whole capability tree provisioned and both families registered.
fn daemon() -> Daemon {
    let root = Some(cap("cap_root"));
    Daemon::builder(Blake3Identity, negotiated(), cap("cap_root"))
        .capability(
            grant(
                "cap_root",
                "service:continuumd",
                AuthorityLevel::Promote,
                4,
                Optional::Present(profile(PRIVILEGED, &[], true)),
            ),
            None,
        )
        // A builder: `propose` and nothing privileged.
        .capability(
            grant(
                "cap_builder",
                "agent:builder",
                AuthorityLevel::Propose,
                3,
                Optional::Absent,
            ),
            root.clone(),
        )
        // A reader: below every mutating operation in either family.
        .capability(
            grant(
                "cap_reader",
                "agent:reader",
                AuthorityLevel::Read,
                3,
                Optional::Absent,
            ),
            root.clone(),
        )
        // docs/49's "revise intent: proposal only" cell — `revise-intent` with no
        // privileged membership.
        .capability(
            grant(
                "cap_proposer",
                "agent:proposer",
                AuthorityLevel::ReviseIntent,
                3,
                Optional::Present(profile(&[], &[], false)),
            ),
            root.clone(),
        )
        // The steward: `revise-intent` plus the three intent privileges.
        .capability(
            grant(
                "cap_steward",
                "human:steward",
                AuthorityLevel::ReviseIntent,
                3,
                Optional::Present(profile(
                    &["intent.accept", "intent.reject", "intent.lock"],
                    &[],
                    false,
                )),
            ),
            root.clone(),
        )
        // docs/49's Reviewer cell: a level that admits an operation, and a profile that
        // does not.
        .capability(
            grant(
                "cap_restricted",
                "agent:reviewer",
                AuthorityLevel::Propose,
                3,
                Optional::Present(profile(&[], &["workspace.create"], false)),
            ),
            root.clone(),
        )
        // Scoped to a snapshot that will never exist, so every snapshot a test names is
        // out of scope.
        .capability(
            {
                let mut scoped = grant(
                    "cap_scoped",
                    "agent:scoped",
                    AuthorityLevel::Propose,
                    3,
                    Optional::Absent,
                );
                scoped.snapshots = vec![WorkspaceHandle::new("ws_elsewhere").expect("handle")];
                scoped
            },
            root.clone(),
        )
        // Expiring, against a daemon with no clock reading: undecidable, so denied.
        .capability(
            {
                let mut expiring = grant(
                    "cap_expiring",
                    "agent:expiring",
                    AuthorityLevel::Propose,
                    3,
                    Optional::Absent,
                );
                expiring.expires_at =
                    Nullable::Value(Timestamp::new("2026-01-01T00:00:00.000Z").expect("time"));
                expiring
            },
            root.clone(),
        )
        // Registered so it can be revoked; a revoked token must be indistinguishable from
        // one that was never registered.
        .capability(
            grant(
                "cap_revoked",
                "agent:revoked",
                AuthorityLevel::Propose,
                3,
                Optional::Absent,
            ),
            root,
        )
        // A mis-provisioned delegation: a child of the `propose` builder that claims
        // `promote`. RFC 0027 D7 makes a child's admission set a subset of its parent's, so
        // this one is denied on the chain rather than trusted on its own descriptor.
        .capability(
            grant(
                "cap_escalating",
                "agent:escalating",
                AuthorityLevel::Promote,
                2,
                Optional::Absent,
            ),
            Some(cap("cap_builder")),
        )
        .family(WorkspaceFamily)
        .family(IntentFamily)
        .build()
}

/// The staged Die Hard workspace and the intent that governs it.
struct Fixture {
    daemon: Daemon,
    intent: IntentHandle,
    files: Vec<Commitment>,
    configuration: Commitment,
}

fn die_hard_contract() -> IntentContract {
    IntentContract::decode(DIE_HARD_CONTRACT.trim_end().as_bytes()).expect("the fixture decodes")
}

fn intent_handle(contract: &IntentContract) -> IntentHandle {
    let stored = continuum_workspace::publication::ContentIdentifier::identify(
        &Blake3Identity,
        continuum_workspace::artifact_path::ArtifactClass::IntentContract,
        &contract.identity_preimage_bytes(),
    )
    .expect("blake3 names every input");
    identity::intent_to_wire(&stored).expect("an `in_` handle")
}

/// A daemon holding the Die Hard workspace as staged content and its contract as a
/// *proposed* registry record. Acceptance happens through the wire, in the tests that need
/// an accepted one.
fn fixture() -> Fixture {
    let mut daemon = daemon();
    let contract = die_hard_contract();
    let intent = intent_handle(&contract);
    daemon.state_mut().put_intent(
        intent.clone(),
        IntentRecord {
            contract,
            status: RegistryStatus::Proposed,
            supersedes: None,
            superseded_by: None,
            acceptance: None,
        },
    );

    let mut files = Vec::new();
    for (path, content) in [("DieHard.ctm", DIE_HARD_MODEL), ("README.md", "# TV-009\n")] {
        files.push(
            daemon
                .state_mut()
                .stage(
                    &Blake3Identity,
                    WorkspacePath::new(path).expect("a workspace path"),
                    content.as_bytes().to_vec(),
                )
                .expect("staging names its content"),
        );
    }
    let configuration = daemon
        .state_mut()
        .stage(
            &Blake3Identity,
            WorkspacePath::new("default.model.toml").expect("a workspace path"),
            DIE_HARD_CONFIG.as_bytes().to_vec(),
        )
        .expect("staging names its content");

    Fixture {
        daemon,
        intent,
        files,
        configuration,
    }
}

fn envelope(operation: &str, actor: &str, capability: &str, request: &str) -> RequestEnvelope {
    RequestEnvelope {
        protocol_version: version(),
        request_id: RequestId::new(request).expect("a well-formed request id"),
        idempotency_key: Optional::Absent,
        actor: who(actor),
        capability: cap(capability),
        operation: name(operation),
        snapshot: Nullable::Null,
        intent: Nullable::Null,
        // The envelope's `arguments` is `Opaque` and this layer carries the decoded body
        // beside it; see `continuumd::daemon`'s documentation for why there is no codec
        // here to fill it with.
        arguments: Opaque::from_bytes(Vec::new()),
        budget: Optional::Absent,
        output_policy: Optional::Absent,
        trace: Optional::Absent,
        page: Optional::Absent,
    }
}

fn keyed(mut envelope: RequestEnvelope, key: &str) -> RequestEnvelope {
    envelope.idempotency_key = Optional::Present(key.to_owned());
    envelope
}

fn components(fixture: &Fixture) -> SnapshotComponents {
    SnapshotComponents {
        files: fixture.files.clone(),
        cml_modules: Vec::new(),
        rust_extraction: Vec::new(),
        domain_packs: Vec::new(),
        dependencies: Vec::new(),
        epochs: SnapshotEpochs {
            semantic: EpochIdentity::new("semantic-1").expect("epoch"),
            proof: EpochIdentity::new("proof-1").expect("epoch"),
            toolchain: Optional::Absent,
        },
        intent: fixture.intent.clone(),
        correspondence: Vec::new(),
        proof_environment: Vec::new(),
        configuration: vec![fixture.configuration.clone()],
        file_components: Optional::Absent,
    }
}

fn acceptance_bytes() -> Opaque {
    let mut fields: BTreeMap<String, Json> = BTreeMap::new();
    for (key, value) in [
        ("accepted_by", "human:steward"),
        ("capability", "revise-intent"),
        ("signature", "sig-die-hard-v1"),
        ("audit_record", "supplied-by-the-caller-and-overwritten"),
        ("timestamp", "2026-08-01T00:00:00.000Z"),
    ] {
        fields.insert(key.to_owned(), Json::String(value.to_owned()));
    }
    Opaque::from_bytes(Json::Object(fields).to_canonical_bytes())
}

/// Accept the fixture's intent through the wire, with the steward's privileged capability.
fn accept_intent(fixture: &mut Fixture, request: &str, key: &str) -> OperationOutcome {
    fixture.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope("intent.accept", "human:steward", "cap_steward", request),
            key,
        ),
        arguments: Arguments::IntentAccept(IntentAcceptRequest {
            proposal: fixture.intent.clone(),
            acceptance: acceptance_bytes(),
            bundle: Optional::Absent,
        }),
    })
}

/// Create the Die Hard workspace, sealing it on creation.
fn create_workspace(fixture: &mut Fixture, request: &str, key: &str) -> OperationOutcome {
    let arguments = Arguments::WorkspaceCreate(WorkspaceCreateRequest {
        components: components(fixture),
        overlay: Optional::Absent,
        seal: Optional::Present(true),
    });
    fixture.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope("workspace.create", "agent:builder", "cap_builder", request),
            key,
        ),
        arguments,
    })
}

fn created_handle(outcome: &OperationOutcome) -> WorkspaceHandle {
    match &outcome.payload {
        Payload::WorkspaceCreate(response) => response.snapshot.clone(),
        other => panic!("expected a workspace.create payload, got {other:?}"),
    }
}

fn code(outcome: &OperationOutcome) -> ErrorCode {
    outcome.error_code().expect("an error result")
}

// --- the green path ------------------------------------------------------------------

#[test]
fn the_die_hard_workspace_is_created_and_sealed_through_the_operation_layer() {
    let mut fixture = fixture();
    let accepted = accept_intent(&mut fixture, "req_accept", "idem-accept");
    assert_eq!(accepted.envelope.status, ResultStatus::Ok);

    let created = create_workspace(&mut fixture, "req_create", "idem-create");
    assert_eq!(
        created.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        created.envelope.error
    );
    let handle = created_handle(&created);
    assert!(handle.as_str().starts_with("ws_"));
    match &created.payload {
        Payload::WorkspaceCreate(response) => {
            assert!(response.sealed, "the request asked for a sealed snapshot");
            assert!(response.diagnostics.is_empty());
        }
        other => panic!("unexpected payload {other:?}"),
    }
    assert_eq!(
        created.envelope.verdict,
        Nullable::Value(Verdict::Structural(
            continuumd::protocol::envelope::StructuralVerdictValue {
                outcome: StructuralOutcome::Created,
            }
        ))
    );
    assert_eq!(created.envelope.artifacts.len(), 1);
    assert_eq!(created.envelope.artifacts[0].kind, "ws");

    // The seal reached the publication store: every record of the descriptor is published,
    // children before parents, and the store audited the decision.
    assert!(!fixture.daemon.store_audit().is_empty());
    assert!(fixture.daemon.state().workspace(&handle).is_some());
}

#[test]
fn sealing_an_existing_workspace_answers_with_its_root_digest() {
    let mut fixture = fixture();
    accept_intent(&mut fixture, "req_accept", "idem-accept");
    let handle = created_handle(&create_workspace(&mut fixture, "req_create", "idem-create"));

    let sealed = fixture.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope("workspace.seal", "agent:builder", "cap_builder", "req_seal"),
            "idem-seal",
        ),
        arguments: Arguments::WorkspaceSeal(WorkspaceSealRequest {
            snapshot: handle.clone(),
        }),
    });
    assert_eq!(sealed.envelope.status, ResultStatus::Ok);
    match &sealed.payload {
        Payload::WorkspaceSeal(response) => {
            assert_eq!(response.snapshot, handle);
            assert!(response.root_digest.as_str().starts_with("ws_"));
        }
        other => panic!("unexpected payload {other:?}"),
    }
    assert!(
        fixture
            .daemon
            .state()
            .workspace(&handle)
            .expect("held")
            .sealed
    );
}

#[test]
fn a_fork_preserves_the_intent_binding_and_advances_the_lineage() {
    let mut fixture = fixture();
    accept_intent(&mut fixture, "req_accept", "idem-accept");
    let base = created_handle(&create_workspace(&mut fixture, "req_create", "idem-create"));

    let forked = fork(&mut fixture, &base, "req_fork", "idem-fork", "edited\n");
    assert_eq!(forked.envelope.status, ResultStatus::Ok);
    match &forked.payload {
        Payload::WorkspaceFork(response) => {
            assert_eq!(response.intent, fixture.intent);
            assert_ne!(response.snapshot, base);
            assert!(response.pre_diff.is_absent());
        }
        other => panic!("unexpected payload {other:?}"),
    }
}

fn fork(
    fixture: &mut Fixture,
    base: &WorkspaceHandle,
    request: &str,
    key: &str,
    buffer: &str,
) -> OperationOutcome {
    fixture.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope("workspace.fork", "agent:builder", "cap_builder", request),
            key,
        ),
        arguments: Arguments::WorkspaceFork(WorkspaceForkRequest {
            base: base.clone(),
            overlay: Optional::Present(vec![FileOverlay {
                path: "README.md".to_owned(),
                content: buffer.as_bytes().to_vec(),
            }]),
            patches: Optional::Absent,
        }),
    })
}

// --- staleness -----------------------------------------------------------------------

#[test]
fn forking_twice_from_one_base_refuses_the_second_with_a_stale_snapshot() {
    let mut fixture = fixture();
    accept_intent(&mut fixture, "req_accept", "idem-accept");
    let base = created_handle(&create_workspace(&mut fixture, "req_create", "idem-create"));

    assert_eq!(
        fork(&mut fixture, &base, "req_fork_1", "idem-fork-1", "first\n")
            .envelope
            .status,
        ResultStatus::Ok
    );
    let second = fork(&mut fixture, &base, "req_fork_2", "idem-fork-2", "second\n");
    assert_eq!(code(&second), ErrorCode::StaleSnapshot);
    assert!(second.payload == Payload::None, "no partial effect");
}

#[test]
fn sealing_a_superseded_snapshot_is_a_stale_snapshot() {
    let mut fixture = fixture();
    accept_intent(&mut fixture, "req_accept", "idem-accept");
    let base = created_handle(&create_workspace(&mut fixture, "req_create", "idem-create"));
    fork(&mut fixture, &base, "req_fork", "idem-fork", "edited\n");

    let sealed = fixture.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope("workspace.seal", "agent:builder", "cap_builder", "req_seal"),
            "idem-seal",
        ),
        arguments: Arguments::WorkspaceSeal(WorkspaceSealRequest { snapshot: base }),
    });
    assert_eq!(code(&sealed), ErrorCode::StaleSnapshot);
}

#[test]
fn a_snapshot_the_daemon_does_not_hold_is_denied_and_never_reported_missing() {
    let mut fixture = fixture();
    accept_intent(&mut fixture, "req_accept", "idem-accept");
    let held = created_handle(&create_workspace(&mut fixture, "req_create", "idem-create"));

    // Same request identity, same capability, two snapshots: one the daemon holds and one
    // it does not. The capability's scope admits neither, so both answers must be the same
    // bytes (RFC 0027 X2).
    let absent = seal_as(&mut fixture, "cap_scoped", "agent:scoped", &unheld());
    let present = seal_as(&mut fixture, "cap_scoped", "agent:scoped", &held);
    assert_eq!(code(&absent), ErrorCode::CapabilityDenied);
    assert_eq!(absent, present, "a not-found is an existence oracle");
}

fn unheld() -> WorkspaceHandle {
    WorkspaceHandle::new("ws_never_created").expect("handle")
}

fn seal_as(
    fixture: &mut Fixture,
    capability: &str,
    actor: &str,
    snapshot: &WorkspaceHandle,
) -> OperationOutcome {
    fixture.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope("workspace.seal", actor, capability, "req_probe"),
            "idem-probe",
        ),
        arguments: Arguments::WorkspaceSeal(WorkspaceSealRequest {
            snapshot: snapshot.clone(),
        }),
    })
}

// --- admission -----------------------------------------------------------------------

#[test]
fn every_admission_failure_is_one_byte_identical_answer() {
    let mut fixture = fixture();
    fixture
        .daemon
        .state_mut()
        .revoke_capability(&cap("cap_revoked"));

    let create = |fixture: &mut Fixture, actor: &str, capability: &str| {
        let arguments = Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: components(fixture),
            overlay: Optional::Absent,
            seal: Optional::Absent,
        });
        fixture.daemon.dispatch(&OperationRequest {
            envelope: keyed(
                envelope("workspace.create", actor, capability, "req_denied"),
                "idem-denied",
            ),
            arguments,
        })
    };

    let denials = [
        // T4 — never registered.
        create(&mut fixture, "agent:ghost", "cap_absent"),
        // T4 — registered, then revoked.
        create(&mut fixture, "agent:revoked", "cap_revoked"),
        // T4 — expiring, against a daemon with no clock reading.
        create(&mut fixture, "agent:expiring", "cap_expiring"),
        // T4 — the actor does not match the capability's.
        create(&mut fixture, "agent:builder", "cap_reader"),
        // T1 — below the operation's registry minimum.
        create(&mut fixture, "agent:reader", "cap_reader"),
        // T3 — the profile denies the operation whatever the level admits.
        create(&mut fixture, "agent:reviewer", "cap_restricted"),
        // T2 — the capability's snapshot scope does not reach the snapshot the request
        // names. The operation differs from the others, and the answer does not: a result
        // envelope carries no operation, so a denial cannot say which one was attempted.
        fixture.daemon.dispatch(&OperationRequest {
            envelope: keyed(
                envelope("workspace.seal", "agent:scoped", "cap_scoped", "req_denied"),
                "idem-denied",
            ),
            arguments: Arguments::WorkspaceSeal(WorkspaceSealRequest { snapshot: unheld() }),
        }),
    ];
    for denial in &denials {
        assert_eq!(code(denial), ErrorCode::CapabilityDenied);
        assert!(
            denial.envelope.audit.value().is_some(),
            "a denial that cannot cite its audit record fails its case (rule audit.correlation)"
        );
        // Everything but the audit identity is fixed. The audit identity is a function of
        // `request_id` and `actor` and of nothing else, so it differs here exactly because
        // these seven denials name seven principals — which is `rule audit.correlation`
        // holding, not X1 failing. That the identity does not vary with the *outcome* is
        // `the_audit_correlation_does_not_vary_with_the_outcome`, and that two denials for
        // one principal are byte-identical whether or not the artifact exists is
        // `a_snapshot_the_daemon_does_not_hold_is_denied_and_never_reported_missing`.
        assert_eq!(
            without_audit(denial),
            without_audit(&denials[0]),
            "every admission failure is one answer (RFC 0027 X1)"
        );
    }
}

fn without_audit(outcome: &OperationOutcome) -> OperationOutcome {
    let mut stripped = outcome.clone();
    stripped.envelope.audit = Optional::Absent;
    stripped
}

#[test]
fn the_audit_correlation_does_not_vary_with_the_outcome() {
    // > The identity MUST be a function of the request identity alone — `request_id` and
    // > `actor` — and MUST NOT vary with the outcome, the decision, the presented
    // > capability, or whether a named artifact exists.
    // >
    // > — `rule audit.correlation`
    let mut fixture = fixture();
    let accepted = accept_intent(&mut fixture, "req_twice", "idem-accept-1");
    assert_eq!(accepted.envelope.status, ResultStatus::Ok);
    // The same actor and the same request identity, now on a refused call: accepting an
    // already-accepted contract is an intent mutation, not an acceptance.
    let refused = accept_intent(&mut fixture, "req_twice", "idem-accept-2");
    assert_eq!(code(&refused), ErrorCode::IntentMutationDenied);
    assert_eq!(accepted.envelope.audit, refused.envelope.audit);
}

#[test]
fn every_admission_decision_is_recorded_whichever_way_it_went() {
    let mut fixture = fixture();
    accept_intent(&mut fixture, "req_accept", "idem-accept");
    let denied = fixture.daemon.dispatch(&OperationRequest {
        envelope: envelope("intent.get", "agent:ghost", "cap_absent", "req_probe"),
        arguments: Arguments::IntentGet(IntentGetRequest {
            intent: fixture.intent.clone(),
        }),
    });
    assert_eq!(code(&denied), ErrorCode::CapabilityDenied);

    let records = fixture.daemon.state().admissions();
    assert!(records.iter().any(|record| record.admitted));
    let refusal = records
        .iter()
        .find(|record| !record.admitted)
        .expect("the denial is recorded (RFC 0027 P5)");
    assert_eq!(refusal.operation, "intent.get");
    assert_eq!(refusal.actor, "agent:ghost");
    assert_eq!(
        Some(refusal.audit.as_str()),
        denied.envelope.audit.value().map(|id| id.as_str()),
        "the record and the result name each other"
    );
}

#[test]
fn a_capability_handle_never_reaches_a_rendering() {
    // RFC 0027 S5: a `cap_*` "never appears in a trace, an error, a `next_operations`
    // argument, or a rendering". The admission record holds one, and its `Debug` is the
    // rendering a log line would use.
    let mut fixture = fixture();
    let _ = fixture.daemon.dispatch(&OperationRequest {
        envelope: envelope("intent.get", "agent:reader", "cap_reader", "req_probe"),
        arguments: Arguments::IntentGet(IntentGetRequest {
            intent: fixture.intent.clone(),
        }),
    });
    let rendered = format!("{:?}", fixture.daemon.state().admissions());
    assert!(!rendered.contains("cap_reader"), "{rendered}");
    assert!(rendered.contains("redacted"));
}

// --- the annotation obligations ------------------------------------------------------

#[test]
fn a_mutation_without_an_idempotency_key_is_malformed() {
    let mut fixture = fixture();
    let arguments = Arguments::WorkspaceCreate(WorkspaceCreateRequest {
        components: components(&fixture),
        overlay: Optional::Absent,
        seal: Optional::Absent,
    });
    let outcome = fixture.daemon.dispatch(&OperationRequest {
        envelope: envelope(
            "workspace.create",
            "agent:builder",
            "cap_builder",
            "req_no_key",
        ),
        arguments,
    });
    assert_eq!(code(&outcome), ErrorCode::MalformedRequest);
}

#[test]
fn a_readonly_operation_carrying_an_idempotency_key_is_malformed() {
    let mut fixture = fixture();
    let outcome = fixture.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope("intent.get", "agent:reader", "cap_reader", "req_keyed_read"),
            "idem-read",
        ),
        arguments: Arguments::IntentGet(IntentGetRequest {
            intent: fixture.intent.clone(),
        }),
    });
    assert_eq!(code(&outcome), ErrorCode::MalformedRequest);
}

#[test]
fn a_task_starting_operation_without_a_budget_is_malformed() {
    let mut fixture = fixture();
    let request = OperationRequest {
        envelope: envelope("workspace.diff", "agent:builder", "cap_builder", "req_diff"),
        arguments: Arguments::WorkspaceDiff(WorkspaceDiffRequest {
            before: unheld(),
            after: unheld(),
            layers: vec![DiffLayer::Textual],
        }),
    };
    assert_eq!(
        code(&fixture.daemon.dispatch(&request)),
        ErrorCode::MalformedRequest
    );

    // With the budget the annotation requires, the same call reaches the handler. The
    // handler consults both snapshot carriers first (bn-27mx7): handles this daemon does
    // not hold are a denial (X2)...
    let mut envelope = request.envelope.clone();
    envelope.budget = Optional::Present(budget());
    let served = fixture.daemon.dispatch(&OperationRequest {
        envelope: envelope.clone(),
        arguments: request.arguments,
    });
    assert_eq!(code(&served), ErrorCode::CapabilityDenied);

    // ...and two held, sealed snapshots get the typed refusal the unshipped diff lane owes
    // them.
    accept_intent(&mut fixture, "req_accept", "idem-accept");
    let snapshot = created_handle(&create_workspace(&mut fixture, "req_create", "idem-create"));
    let served = fixture.daemon.dispatch(&OperationRequest {
        envelope,
        arguments: Arguments::WorkspaceDiff(WorkspaceDiffRequest {
            before: snapshot.clone(),
            after: snapshot,
            layers: vec![DiffLayer::Textual],
        }),
    });
    assert_eq!(code(&served), ErrorCode::UnsupportedSemanticFeature);
}

fn budget() -> Budget {
    Budget {
        wall_ms: Optional::Absent,
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Absent,
        states: Optional::Absent,
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes: Optional::Absent,
    }
}

// --- idempotency ---------------------------------------------------------------------

#[test]
fn replaying_an_idempotent_request_returns_the_same_identity() {
    let mut fixture = fixture();
    accept_intent(&mut fixture, "req_accept", "idem-accept");
    let first = create_workspace(&mut fixture, "req_create", "idem-create");
    let replay = create_workspace(&mut fixture, "req_create", "idem-create");
    assert_eq!(first, replay);
    assert_eq!(created_handle(&first), created_handle(&replay));
}

#[test]
fn the_same_key_with_a_different_request_is_rejected() {
    let mut fixture = fixture();
    accept_intent(&mut fixture, "req_accept", "idem-accept");
    create_workspace(&mut fixture, "req_create", "idem-create");

    let mut altered = components(&fixture);
    altered.files.truncate(1);
    let outcome = fixture.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope(
                "workspace.create",
                "agent:builder",
                "cap_builder",
                "req_create_2",
            ),
            "idem-create",
        ),
        arguments: Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: altered,
            overlay: Optional::Absent,
            seal: Optional::Present(true),
        }),
    });
    assert_eq!(code(&outcome), ErrorCode::IdempotencyKeyReused);
}

// --- the intent family ---------------------------------------------------------------

#[test]
fn a_snapshot_cannot_be_bound_to_an_unaccepted_intent() {
    let mut fixture = fixture();
    let outcome = create_workspace(&mut fixture, "req_create", "idem-create");
    assert_eq!(code(&outcome), ErrorCode::AcceptanceChainInvalid);
}

#[test]
fn a_reviser_without_the_privilege_cannot_accept_an_intent() {
    let mut fixture = fixture();
    let outcome = fixture.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope(
                "intent.accept",
                "agent:proposer",
                "cap_proposer",
                "req_accept",
            ),
            "idem-accept",
        ),
        arguments: Arguments::IntentAccept(IntentAcceptRequest {
            proposal: fixture.intent.clone(),
            acceptance: acceptance_bytes(),
            bundle: Optional::Absent,
        }),
    });
    assert_eq!(code(&outcome), ErrorCode::CapabilityDenied);
    assert_eq!(
        fixture
            .daemon
            .state()
            .intent(&fixture.intent)
            .expect("held")
            .status,
        RegistryStatus::Proposed,
        "a denied privileged call leaves no partial effect"
    );
}

#[test]
fn an_acceptance_naming_an_intent_bundle_fails_closed() {
    let mut fixture = fixture();
    let outcome = fixture.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope(
                "intent.accept",
                "human:steward",
                "cap_steward",
                "req_accept",
            ),
            "idem-accept",
        ),
        arguments: Arguments::IntentAccept(IntentAcceptRequest {
            proposal: fixture.intent.clone(),
            acceptance: acceptance_bytes(),
            bundle: Optional::Present(
                continuumd::protocol::scalar::IntentBundleHandle::new("inb_imported")
                    .expect("handle"),
            ),
        }),
    });
    assert_eq!(code(&outcome), ErrorCode::AcceptanceChainInvalid);
}

#[test]
fn intent_accept_records_the_audit_correlation_the_result_cites() {
    let mut fixture = fixture();
    let accepted = accept_intent(&mut fixture, "req_accept", "idem-accept");
    let cited = accepted
        .envelope
        .audit
        .value()
        .expect("an @audit_recorded result carries `audit`")
        .as_str()
        .to_owned();

    let record = read_record(&mut fixture, "req_get");
    let Json::Object(fields) = &record else {
        panic!("the registry record is an object")
    };
    assert_eq!(
        fields.get("status"),
        Some(&Json::String("accepted".to_owned()))
    );
    let Some(Json::Object(acceptance)) = fields.get("acceptance") else {
        panic!("an accepted record carries its acceptance")
    };
    assert_eq!(
        acceptance.get("audit_record"),
        Some(&Json::String(cited)),
        "the record names the audit identity the daemon produced, never the caller's"
    );
    assert_eq!(
        acceptance.get("capability"),
        Some(&Json::String("revise-intent".to_owned()))
    );
}

fn read_record(fixture: &mut Fixture, request: &str) -> Json {
    let outcome = fixture.daemon.dispatch(&OperationRequest {
        envelope: envelope("intent.get", "agent:reader", "cap_reader", request),
        arguments: Arguments::IntentGet(IntentGetRequest {
            intent: fixture.intent.clone(),
        }),
    });
    assert_eq!(outcome.envelope.status, ResultStatus::Ok);
    match &outcome.payload {
        Payload::IntentGet(response) => {
            Json::parse(response.record.as_bytes()).expect("a canonical record")
        }
        other => panic!("unexpected payload {other:?}"),
    }
}

#[test]
fn intent_lock_round_trips_the_field_level_change_policy() {
    let mut fixture = fixture();
    accept_intent(&mut fixture, "req_accept", "idem-accept");

    let mut policy = BTreeMap::new();
    policy.insert(
        PolicyField::Fairness.wire().to_owned(),
        PolicyVerb::Locked.wire().to_owned(),
    );
    let locked = fixture.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope("intent.lock", "human:steward", "cap_steward", "req_lock"),
            "idem-lock",
        ),
        arguments: Arguments::IntentLock(IntentLockRequest {
            intent: fixture.intent.clone(),
            policy,
        }),
    });
    assert_eq!(
        locked.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        locked.envelope.error
    );
    let successor = match &locked.payload {
        Payload::IntentLock(response) => {
            assert_eq!(response.policy.len(), PolicyField::ALL.len());
            assert_eq!(
                response
                    .policy
                    .get(PolicyField::Fairness.wire())
                    .map(String::as_str),
                Some(PolicyVerb::Locked.wire()),
            );
            assert_ne!(response.intent, fixture.intent, "ID3 mints a successor");
            response.intent.clone()
        }
        other => panic!("unexpected payload {other:?}"),
    };

    // The round trip: what came back on the wire is what the stored contract decodes to.
    let read = fixture.daemon.dispatch(&OperationRequest {
        envelope: envelope("intent.get", "agent:reader", "cap_reader", "req_get_locked"),
        arguments: Arguments::IntentGet(IntentGetRequest {
            intent: successor.clone(),
        }),
    });
    let Payload::IntentGet(response) = &read.payload else {
        panic!("unexpected payload")
    };
    let contract = IntentContract::decode(response.contract.as_bytes()).expect("decodes");
    assert_eq!(
        contract.policy().verb(PolicyField::Fairness),
        PolicyVerb::Locked
    );
    assert_eq!(contract.intent_id().as_str(), successor.as_str());

    // The predecessor records the supersession edge M1 walks.
    assert_eq!(
        fixture
            .daemon
            .state()
            .intent(&fixture.intent)
            .expect("held")
            .status,
        RegistryStatus::Superseded
    );
}

#[test]
fn an_intent_lock_that_weakens_a_field_is_denied() {
    let mut fixture = fixture();
    accept_intent(&mut fixture, "req_accept", "idem-accept");

    let mut policy = BTreeMap::new();
    policy.insert(
        PolicyField::Properties.wire().to_owned(),
        PolicyVerb::Unlocked.wire().to_owned(),
    );
    let outcome = fixture.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope("intent.lock", "human:steward", "cap_steward", "req_unlock"),
            "idem-unlock",
        ),
        arguments: Arguments::IntentLock(IntentLockRequest {
            intent: fixture.intent.clone(),
            policy,
        }),
    });
    assert_eq!(code(&outcome), ErrorCode::IntentMutationDenied);
    assert_eq!(
        fixture
            .daemon
            .state()
            .intent(&fixture.intent)
            .expect("held")
            .status,
        RegistryStatus::Accepted,
        "a refused lock leaves the registry untouched"
    );
}

#[test]
fn an_intent_lock_naming_a_verb_outside_the_closed_set_is_malformed() {
    let mut fixture = fixture();
    accept_intent(&mut fixture, "req_accept", "idem-accept");
    let mut policy = BTreeMap::new();
    policy.insert(
        PolicyField::Fairness.wire().to_owned(),
        "immutable-forever".to_owned(),
    );
    let outcome = fixture.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope("intent.lock", "human:steward", "cap_steward", "req_lock"),
            "idem-lock",
        ),
        arguments: Arguments::IntentLock(IntentLockRequest {
            intent: fixture.intent.clone(),
            policy,
        }),
    });
    assert_eq!(code(&outcome), ErrorCode::MalformedRequest);
}

#[test]
fn a_revision_touching_a_locked_field_is_refused_before_anything_else() {
    let mut fixture = fixture();
    accept_intent(&mut fixture, "req_accept", "idem-accept");

    // `claims` is the contract path of the `properties` policy key, which the Die Hard
    // contract locks.
    let mut changes: BTreeMap<String, Json> = BTreeMap::new();
    changes.insert("claims".to_owned(), Json::Array(Vec::new()));
    let outcome = propose(
        &mut fixture,
        Json::Object(changes),
        "req_locked",
        "idem-locked",
    );
    assert_eq!(code(&outcome), ErrorCode::IntentMutationDenied);

    // A field the contract does not lock reaches the unshipped classification lane
    // instead, which is a different refusal.
    let mut changes: BTreeMap<String, Json> = BTreeMap::new();
    changes.insert("scope".to_owned(), Json::Array(Vec::new()));
    let outcome = propose(
        &mut fixture,
        Json::Object(changes),
        "req_scope",
        "idem-scope",
    );
    assert_eq!(code(&outcome), ErrorCode::UnsupportedSemanticFeature);
}

fn propose(fixture: &mut Fixture, changes: Json, request: &str, key: &str) -> OperationOutcome {
    fixture.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope(
                "intent.propose_revision",
                "agent:proposer",
                "cap_proposer",
                request,
            ),
            key,
        ),
        arguments: Arguments::IntentProposeRevision(IntentProposeRevisionRequest {
            base: fixture.intent.clone(),
            changes: IntentChangeSet {
                changes: Opaque::from_bytes(changes.to_canonical_bytes()),
                rationale: "a typed rationale for reviewers".to_owned(),
            },
        }),
    })
}

// --- the skeleton itself -------------------------------------------------------------

#[test]
fn a_request_naming_another_protocol_version_is_refused() {
    let mut fixture = fixture();
    let mut envelope = envelope("intent.get", "agent:reader", "cap_reader", "req_version");
    envelope.protocol_version = ProtocolVersion::new(2, 0);
    let outcome = fixture.daemon.dispatch(&OperationRequest {
        envelope,
        arguments: Arguments::IntentGet(IntentGetRequest {
            intent: fixture.intent.clone(),
        }),
    });
    assert_eq!(code(&outcome), ErrorCode::ProtocolVersionUnsupported);
}

#[test]
fn a_body_that_is_not_the_operations_shape_is_malformed() {
    let mut fixture = fixture();
    let outcome = fixture.daemon.dispatch(&OperationRequest {
        // The envelope names `intent.get`; the body is `workspace.seal`'s.
        envelope: envelope("intent.get", "agent:reader", "cap_reader", "req_shape"),
        arguments: Arguments::WorkspaceSeal(WorkspaceSealRequest { snapshot: unheld() }),
    });
    assert_eq!(code(&outcome), ErrorCode::MalformedRequest);
}

#[test]
fn an_operation_whose_family_is_not_registered_is_a_typed_unsupported_feature() {
    // `task.status` is one of the registry's 72 and no family answers for it here.
    let mut daemon = Daemon::builder(Blake3Identity, negotiated(), cap("cap_root"))
        .capability(
            grant(
                "cap_root",
                "service:continuumd",
                AuthorityLevel::Promote,
                4,
                Optional::Present(profile(PRIVILEGED, &[], true)),
            ),
            None,
        )
        .family(IntentFamily)
        .build();
    let outcome = daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope(
                "workspace.seal",
                "service:continuumd",
                "cap_root",
                "req_unserved",
            ),
            "idem-unserved",
        ),
        arguments: Arguments::WorkspaceSeal(WorkspaceSealRequest { snapshot: unheld() }),
    });
    assert_eq!(
        outcome.error_code(),
        Some(ErrorCode::UnsupportedSemanticFeature)
    );
}

#[test]
fn a_successful_result_carries_a_null_envelope_payload_and_a_typed_one_beside_it() {
    let mut fixture = fixture();
    let accepted = accept_intent(&mut fixture, "req_accept", "idem-accept");
    assert!(
        accepted.envelope.payload.is_null(),
        "the envelope's `payload` is `Opaque` and this layer emits no wire bytes"
    );
    assert_eq!(accepted.payload.operation(), Some("intent.accept"));
    assert!(accepted.envelope.next_operations.is_empty());
    assert!(accepted.envelope.artifacts.is_empty());
    // `rule envelope.epochs_named`: every epoch is named, pinned or explicitly null.
    assert_eq!(accepted.envelope.epochs.protocol, version());
    assert!(accepted.envelope.epochs.semantic.is_null());
}

#[test]
fn two_identical_request_sequences_produce_identical_results() {
    let run = || {
        let mut fixture = fixture();
        let accepted = accept_intent(&mut fixture, "req_accept", "idem-accept");
        let created = create_workspace(&mut fixture, "req_create", "idem-create");
        let base = created_handle(&created);
        let forked = fork(&mut fixture, &base, "req_fork", "idem-fork", "edited\n");
        let stale = fork(&mut fixture, &base, "req_fork_2", "idem-fork-2", "edited\n");
        (accepted, created, forked, stale)
    };
    assert_eq!(run(), run(), "dispatch is a function of request and state");
}

#[test]
fn every_fault_a_family_can_raise_is_declared_for_its_operation() {
    for (operation, code) in workspace::FAULTS.iter().chain(intent::FAULTS) {
        let spec = registry::operation(operation).expect("a registered operation");
        assert!(
            errors::admits(spec, *code),
            "{operation} may not answer with {code:?} — `rule errors.common` and its own \
             `errors` clause do not admit it"
        );
    }
}

#[test]
fn the_wire_authority_ladder_agrees_with_the_landed_store_vocabulary() {
    // RFC 0027's "Landed vocabulary" clause binds this RFC and
    // `continuum-workspace/src/publication.rs` together: the five levels, in one order,
    // with the store's `Display` emitting the wire tokens.
    use continuumd::daemon::state::store_level;
    use continuumd::protocol::spec::ProtocolEnum;
    for level in <AuthorityLevel as ProtocolEnum>::ALL {
        assert_eq!(store_level(*level).to_string(), level.as_wire());
    }
    let ladder: Vec<_> = <AuthorityLevel as ProtocolEnum>::ALL
        .iter()
        .map(|level| store_level(*level))
        .collect();
    assert!(
        ladder.windows(2).all(|pair| pair[0] < pair[1]),
        "declaration order is the ladder"
    );
}

#[test]
fn a_workspace_naming_a_component_this_daemon_does_not_bind_is_refused() {
    let mut fixture = fixture();
    accept_intent(&mut fixture, "req_accept", "idem-accept");
    let mut named = components(&fixture);
    named.domain_packs = vec![Commitment::new("pack-1")];
    let outcome = fixture.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope(
                "workspace.create",
                "agent:builder",
                "cap_builder",
                "req_create",
            ),
            "idem-create",
        ),
        arguments: Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: named,
            overlay: Optional::Absent,
            seal: Optional::Absent,
        }),
    });
    assert_eq!(code(&outcome), ErrorCode::UnsupportedSemanticFeature);
}

#[test]
fn a_delegation_that_does_not_narrow_its_parent_is_denied() {
    // `cap_escalating` names `promote` and is a child of a `propose` capability. Its own
    // descriptor admits `workspace.create` at T1; the chain does not, so the request is
    // refused with the same single answer every other admission failure gets.
    let mut fixture = fixture();
    let arguments = Arguments::WorkspaceCreate(WorkspaceCreateRequest {
        components: components(&fixture),
        overlay: Optional::Absent,
        seal: Optional::Absent,
    });
    let outcome = fixture.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope(
                "workspace.create",
                "agent:escalating",
                "cap_escalating",
                "req_escalate",
            ),
            "idem-escalate",
        ),
        arguments,
    });
    assert_eq!(code(&outcome), ErrorCode::CapabilityDenied);
}

#[test]
fn rejecting_a_proposal_takes_it_out_of_the_registry() {
    use continuumd::protocol::operations::intent::IntentRejectRequest;

    let mut fixture = fixture();
    let rejected = fixture.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope(
                "intent.reject",
                "human:steward",
                "cap_steward",
                "req_reject",
            ),
            "idem-reject",
        ),
        arguments: Arguments::IntentReject(IntentRejectRequest {
            proposal: fixture.intent.clone(),
            reason: "the corpus port supersedes it".to_owned(),
        }),
    });
    assert_eq!(rejected.envelope.status, ResultStatus::Ok);
    assert_eq!(
        rejected.envelope.verdict,
        Nullable::Value(Verdict::Structural(
            continuumd::protocol::envelope::StructuralVerdictValue {
                outcome: StructuralOutcome::Rejected,
            }
        ))
    );
    assert!(
        rejected.envelope.audit.value().is_some(),
        "an @audit_recorded operation cites its record"
    );

    // The record is gone, and a read of it is the same denial as a read of anything else
    // the daemon does not hold — never a distinguishable not-found (RFC 0027 X2).
    let read = fixture.daemon.dispatch(&OperationRequest {
        envelope: envelope("intent.get", "agent:reader", "cap_reader", "req_after"),
        arguments: Arguments::IntentGet(IntentGetRequest {
            intent: fixture.intent.clone(),
        }),
    });
    assert_eq!(code(&read), ErrorCode::CapabilityDenied);
}

#[test]
fn the_intent_diff_lane_is_a_typed_refusal() {
    use continuumd::protocol::operations::intent::IntentDiffRequest;

    let mut fixture = fixture();
    let outcome = fixture.daemon.dispatch(&OperationRequest {
        envelope: envelope(
            "intent.diff",
            "agent:reader",
            "cap_reader",
            "req_intent_diff",
        ),
        arguments: Arguments::IntentDiff(IntentDiffRequest {
            before: fixture.intent.clone(),
            after: fixture.intent.clone(),
        }),
    });
    assert_eq!(code(&outcome), ErrorCode::UnsupportedSemanticFeature);
}
