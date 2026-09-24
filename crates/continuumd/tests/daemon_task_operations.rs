//! The `verification` and `task` operation families, through the dispatch skeleton.
//!
//! # What these tests are evidence for
//!
//! Every scenario below runs through [`Daemon::dispatch`] — the same entry point a transport
//! will call — and nothing else. There is no filesystem, no clock unless a test supplies one
//! explicitly, no runtime and no network: the Die Hard workspace arrives as staged content,
//! the Die Hard *model* arrives through the out-of-band model catalog, and every identity is
//! a pure function of bytes.
//!
//! Two claims are being made here at once, and they are separable:
//!
//! - **the PR 5 exit** — replaying an idempotent request returns the same task identity, and
//!   a mismatched reuse of a key is rejected. Both are asserted against a task handle that is
//!   the *content identity* of the campaign, so the first holds before the ledger is
//!   consulted;
//! - **the daemon-API half of the PR 8 exit** — "Die Hard returns 16 states and depth-6
//!   solution through the daemon API". The 16 rides `TaskRecord.cost.states` on the wire; the
//!   96 labelled transitions and the depth-6 witness ride the daemon's own task table,
//!   because RFC 0026's `Cost` declares no transition dimension and a witness reaches a client
//!   only as a crashpack this daemon cannot build. Every number is the frozen TV-009 fact,
//!   restated in `notes/plan/spikes/SPIKE_REPORT.md:9-12`,
//!   `notes/plan/docs/27_SPIKE_FINDINGS_REV2.md:24-27`, and
//!   `crates/continuum-kernel-core/src/fixture.rs:11-14`.
//!
//! # The fixtures, and why they are these fixtures
//!
//! `DieHard.ctm` is `notes/plan/corpus/tla-examples/ports/TV-009/`'s port and the Intent
//! Contract is `continuum-intent`'s own Die Hard fixture, both `include_str!`'d rather than
//! copied: there is one Die Hard model and one Die Hard contract in this repository, and a
//! second copy of either would be a second Die Hard that could drift from the first. The
//! model *value* comes from `continuum_engine_reference::diehard::model`, which is that
//! corpus file's line-for-line transcription — so the bytes in the snapshot and the model in
//! the catalog are the same Die Hard by construction and not by coincidence.

use continuum_engine_reference::diehard;
use continuum_intent::canonical_json::Json;
use continuum_intent::contract::IntentContract;
use continuum_value::epoch::ProtocolWindow;
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::state::{Acceptance, IntentRecord, RegistryStatus};
use continuumd::daemon::task::{
    Continuation, PinnedEpochs, TaskFamily, admissible_epochs, task_handle,
};
use continuumd::daemon::verification::{ModelCatalog, VerificationFamily, model_source};
use continuumd::daemon::workspace::WorkspaceFamily;
use continuumd::daemon::{
    Daemon, OperationOutcome, OperationRequest, errors, output, task, verification,
};
use continuumd::protocol::envelope::{
    Budget, EnvelopeDimension, EpochSet, OutputPolicy, RequestEnvelope, Verdict,
};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, VersionRange, negotiate,
};
use continuumd::protocol::operations::intent::IntentAcceptRequest;
use continuumd::protocol::operations::task::{
    TaskCancelRequest, TaskResumeRequest, TaskStatusRequest, TaskSubscribeRequest,
    TaskUpdateBudgetRequest,
};
use continuumd::protocol::operations::verification::{
    VerificationAwaitRequest, VerificationResultRequest, VerificationStartRequest,
};
use continuumd::protocol::operations::workspace::{WorkspaceCreateRequest, WorkspaceForkRequest};
use continuumd::protocol::registry::{self, ENCODINGS};
use continuumd::protocol::scalar::{
    ActorId, ByteCount, CapabilityHandle, Commitment, ContinuationHandle, EpochIdentity,
    IntentHandle, Opaque, OperationName, ProtocolVersion, RequestId, TaskHandle, Timestamp,
    WorkspaceHandle,
};
use continuumd::protocol::shared::{FileOverlay, SnapshotComponents, SnapshotEpochs, Target};
use continuumd::protocol::spec::{Nullable, Optional};
use continuumd::protocol::vocabulary::{
    AssuranceClass, AuthorityLevel, Encoding, ErrorCode, InconclusiveReason, OmissionReason,
    Portfolio, PriorityClass, ResultStatus, SemanticVerdict, StructuralOutcome, TargetKind,
    TaskStatus,
};

use continuum_workspace::snapshot::WorkspacePath;

/// The TV-009 port's model, verbatim — the bytes that go into the snapshot.
const DIE_HARD_MODEL: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm");

/// The TV-009 port's default model configuration.
const DIE_HARD_CONFIG: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/default.model.toml");

/// The Die Hard Intent Contract, as `continuum-intent`'s own suites use it.
const DIE_HARD_CONTRACT: &str =
    include_str!("../../continuum-intent/tests/fixtures/die-hard-contract.json");

/// Where the model lives inside the workspace.
const MODULE_PATH: &str = "DieHard.ctm";

/// The frozen TV-009 facts, named once.
const FROZEN_STATES: u64 = 16;
const FROZEN_TRANSITIONS: u64 = 96;
const FROZEN_WITNESS_DEPTH: usize = 6;

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

fn epoch(token: &str) -> EpochIdentity {
    EpochIdentity::new(token).expect("a well-formed epoch identity")
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
        instances: Optional::Absent,
    }
}

fn profile(privileged: &[&str]) -> CapabilityProfile {
    CapabilityProfile {
        privileged_operations: privileged.iter().map(|entry| name(entry)).collect(),
        denied_operations: Vec::new(),
        data_grants: Vec::new(),
        cross_principal_sharing: true,
    }
}

fn negotiated() -> Negotiated {
    let hello = ClientHello {
        protocol_versions: VersionRange {
            low: version(),
            high: version(),
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuumd-task-operations-test".to_owned(),
        actor: who("service:continuumd"),
        capability: cap("cap_root"),
        features: Optional::Absent,
    };
    negotiate(&[version()], ProtocolWindow::new(3), ENCODINGS, &hello).expect("3.1 is served")
}

/// The epochs this daemon serves. Pinned rather than unpinned, so a continuation has
/// something to pin and the resume predicate has something to disagree with.
fn epochs() -> EpochSet {
    EpochSet {
        protocol: version(),
        semantic: Nullable::Value(epoch("semantic-1")),
        intent: Nullable::Value(epoch("intent-1")),
        evidence: Nullable::Null,
        proof: Nullable::Value(epoch("proof-1")),
        corpus: Nullable::Null,
        engine: Nullable::Value(epoch("engine-reference-1")),
    }
}

fn now() -> Timestamp {
    Timestamp::new("2026-08-01T00:00:00.000Z").expect("a well-formed timestamp")
}

/// A daemon with all four families registered and a capability tree that reaches every
/// authority level the two new families need.
fn daemon() -> Daemon {
    let root = Some(cap("cap_root"));
    Daemon::builder(Blake3Identity, negotiated(), cap("cap_root"))
        .epochs(epochs())
        .now(now())
        .capability(
            grant(
                "cap_root",
                "service:continuumd",
                AuthorityLevel::Promote,
                4,
                Optional::Present(profile(&[
                    "intent.accept",
                    "intent.reject",
                    "intent.lock",
                    "repair.promote",
                    "repair.reject",
                ])),
            ),
            None,
        )
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
        // The runner: `execute`, which is what `verification.*`, `task.cancel`,
        // `task.resume` and `task.update_budget` declare.
        .capability(
            grant(
                "cap_runner",
                "agent:runner",
                AuthorityLevel::Execute,
                3,
                Optional::Absent,
            ),
            root.clone(),
        )
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
        .capability(
            grant(
                "cap_steward",
                "human:steward",
                AuthorityLevel::ReviseIntent,
                3,
                Optional::Present(profile(&["intent.accept", "intent.reject", "intent.lock"])),
            ),
            root.clone(),
        )
        // Scoped to artifact classes that do not include `task`, so every task operation is
        // out of scope at T2 while the level admits it.
        .capability(
            {
                let mut scoped = grant(
                    "cap_elsewhere",
                    "agent:reader",
                    AuthorityLevel::Execute,
                    3,
                    Optional::Absent,
                );
                scoped.artifact_classes = vec!["ws".to_owned()];
                scoped
            },
            root,
        )
        .family(WorkspaceFamily)
        .family(IntentFamily)
        .family(TaskFamily)
        .family(VerificationFamily)
        .build()
}

struct Fixture {
    daemon: Daemon,
    intent: IntentHandle,
    snapshot: WorkspaceHandle,
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
    continuumd::daemon::identity::intent_to_wire(&stored).expect("an `in_` handle")
}

/// The content identity the daemon derives for a snapshot carrying only `DieHard.ctm`.
fn die_hard_source() -> Commitment {
    model_source(&Blake3Identity, [(MODULE_PATH, DIE_HARD_MODEL.as_bytes())])
        .expect("blake3 names the module set")
}

/// A daemon holding the Die Hard workspace sealed, its contract accepted, and the Die Hard
/// model registered in the catalog.
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
    for (path, content) in [(MODULE_PATH, DIE_HARD_MODEL), ("README.md", "# TV-009\n")] {
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

    // The out-of-band model registration this daemon has instead of a CML front end.
    daemon.state_mut().models_mut().register(
        die_hard_source(),
        diehard::model().expect("the port builds"),
    );

    let accepted = daemon.dispatch(&OperationRequest {
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
            proposal: intent.clone(),
            acceptance: acceptance_bytes(),
            bundle: Optional::Absent,
        }),
    });
    assert_eq!(accepted.envelope.status, ResultStatus::Ok);

    let created = daemon.dispatch(&OperationRequest {
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
            components: SnapshotComponents {
                files,
                cml_modules: Vec::new(),
                rust_extraction: Vec::new(),
                domain_packs: Vec::new(),
                dependencies: Vec::new(),
                epochs: SnapshotEpochs {
                    semantic: epoch("semantic-1"),
                    proof: epoch("proof-1"),
                    toolchain: Optional::Absent,
                },
                intent: intent.clone(),
                correspondence: Vec::new(),
                proof_environment: Vec::new(),
                configuration: vec![configuration],
                file_components: Optional::Absent,
            },
            overlay: Optional::Absent,
            seal: Optional::Present(true),
        }),
    });
    assert_eq!(
        created.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        created.envelope.error
    );
    let snapshot = match &created.payload {
        Payload::WorkspaceCreate(response) => response.snapshot.clone(),
        other => panic!("expected a workspace.create payload, got {other:?}"),
    };

    Fixture {
        daemon,
        intent,
        snapshot,
    }
}

fn acceptance_bytes() -> Opaque {
    let mut fields: std::collections::BTreeMap<String, Json> = std::collections::BTreeMap::new();
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

fn budgeted(mut envelope: RequestEnvelope, states: Option<u64>) -> RequestEnvelope {
    envelope.budget = Optional::Present(budget(states));
    envelope
}

fn budget(states: Option<u64>) -> Budget {
    Budget {
        wall_ms: Optional::Absent,
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Absent,
        states: match states {
            Some(states) => Optional::Present(states),
            None => Optional::Absent,
        },
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes: Optional::Absent,
    }
}

fn on(mut envelope: RequestEnvelope, snapshot: &WorkspaceHandle) -> RequestEnvelope {
    envelope.snapshot = Nullable::Value(snapshot.clone());
    envelope
}

fn target(kind: TargetKind, id: &str) -> Target {
    Target {
        kind,
        id: id.to_owned(),
    }
}

/// `verification.start` over the fixture's sealed snapshot.
fn start(
    fixture: &mut Fixture,
    request: &str,
    key: &str,
    states: Option<u64>,
    target: Target,
) -> OperationOutcome {
    let snapshot = fixture.snapshot.clone();
    fixture.daemon.dispatch(&OperationRequest {
        envelope: on(
            budgeted(
                keyed(
                    envelope("verification.start", "agent:runner", "cap_runner", request),
                    key,
                ),
                states,
            ),
            &snapshot,
        ),
        arguments: Arguments::VerificationStart(VerificationStartRequest {
            target,
            portfolio: Portfolio::Interactive,
            context_policy: Optional::Absent,
            priority_class: Optional::Absent,
        }),
    })
}

fn started_task(outcome: &OperationOutcome) -> TaskHandle {
    match &outcome.payload {
        Payload::VerificationStart(response) => response
            .task
            .value()
            .cloned()
            .expect("a fresh start names a task"),
        other => panic!("expected a verification.start payload, got {other:?}"),
    }
}

fn status(fixture: &mut Fixture, task: &TaskHandle, request: &str) -> OperationOutcome {
    fixture.daemon.dispatch(&OperationRequest {
        envelope: envelope("task.status", "agent:reader", "cap_reader", request),
        arguments: Arguments::TaskStatus(TaskStatusRequest { task: task.clone() }),
    })
}

fn record(outcome: &OperationOutcome) -> continuumd::protocol::task::TaskRecord {
    match &outcome.payload {
        Payload::TaskStatus(record) => record.clone(),
        other => panic!("expected a task.status payload, got {other:?}"),
    }
}

fn code(outcome: &OperationOutcome) -> ErrorCode {
    outcome.error_code().expect("an error result")
}

// --- the PR 8 exit, at the operation layer -----------------------------------------------

/// The headline: a Die Hard campaign started through `verification.start` and read back
/// through `task.status` reports the frozen TV-009 facts.
///
/// The wire carries **16** as `TaskRecord.cost.states`; the daemon's own task table carries
/// the **96** labelled transitions and the depth-**6** shortest violation of `NotSolved`,
/// which is the film's six-step solution. Both are the numbers frozen by the Revision 2 spike
/// and restated in four places, and neither is recomputed here — they are read off the
/// campaign the operation ran.
#[test]
fn die_hard_returns_sixteen_states_and_a_depth_six_solution_through_the_daemon_api() {
    let mut fixture = fixture();
    let started = start(
        &mut fixture,
        "req_start",
        "idem-start",
        Some(64),
        target(TargetKind::AllClaims, "DieHard"),
    );
    // `verification.start` is `@task_starting`, and its response names a *task* rather
    // than a cached result, so the envelope reports `task_started` — "a long operation was
    // started; `task` is present". The status and the response body's own discriminator
    // agree by construction, and `the_envelope_status_lane_agrees_with_the_response_body`
    // holds them to it.
    assert_eq!(
        started.envelope.status,
        ResultStatus::TaskStarted,
        "{:?}",
        started.envelope.error
    );
    assert_eq!(
        started.envelope.task.value().map(TaskHandle::as_str),
        Some(started_task(&started).as_str()),
        "the envelope names the task it started"
    );
    let task = started_task(&started);
    assert!(task.as_str().starts_with("task_"));

    let record = record(&status(&mut fixture, &task, "req_status"));
    assert_eq!(record.status, TaskStatus::Completed);
    assert_eq!(record.task, task);
    assert_eq!(record.operation.as_str(), "verification.start");
    assert_eq!(
        record.cost.states,
        Optional::Present(FROZEN_STATES),
        "the frozen reachable-state count, on the wire"
    );
    assert!(
        record.continuation.is_absent(),
        "a closed campaign parks nothing"
    );
    assert!(record.failed_reason.is_absent());
    assert_eq!(record.epochs, epochs());
    assert_eq!(record.priority_class, PriorityClass::Interactive);

    let campaign = fixture
        .daemon
        .state()
        .tasks()
        .get(&task)
        .expect("the daemon holds the task")
        .campaign
        .as_ref()
        .expect("the campaign ran");
    assert!(campaign.is_closed(), "64 states admits Die Hard's 16");
    assert_eq!(campaign.states() as u64, FROZEN_STATES);
    assert_eq!(campaign.transitions, FROZEN_TRANSITIONS);
    assert_eq!(campaign.max_depth, Some(7));
    assert_eq!(
        campaign.violation_depth(),
        Some(FROZEN_WITNESS_DEPTH),
        "NotSolved is refuted by the film's six-step solution"
    );
}

/// The verdict half: `verification.result` carries the CheckReport-shaped answer, with the
/// nine-dimension assurance envelope `rule envelope.assurance_required` demands beside it.
#[test]
fn the_verification_result_carries_the_semantic_verdict_and_its_assurance_envelope() {
    let mut fixture = fixture();
    let task = started_task(&start(
        &mut fixture,
        "req_start",
        "idem-start",
        Some(64),
        target(TargetKind::AllClaims, "DieHard"),
    ));

    let outcome = fixture.daemon.dispatch(&OperationRequest {
        envelope: envelope(
            "verification.result",
            "agent:runner",
            "cap_runner",
            "req_result",
        ),
        arguments: Arguments::VerificationResult(VerificationResultRequest { task: task.clone() }),
    });
    assert_eq!(
        outcome.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        outcome.envelope.error
    );

    // `NotSolved` is intentionally false somewhere, so the fold — in which a refutation
    // dominates — is `refuted` over a closed exploration, which is the `validated` rung.
    match &outcome.envelope.verdict {
        Nullable::Value(Verdict::Semantic(value)) => {
            assert_eq!(value.verdict, SemanticVerdict::Refuted);
            assert!(value.inconclusive_reason.is_absent());
            assert_eq!(value.assurance_class, AssuranceClass::Validated);
        }
        other => panic!("expected a semantic verdict, got {other:?}"),
    }

    let assurance = outcome
        .envelope
        .assurance
        .value()
        .expect("`rule envelope.assurance_required` attaches one to every semantic verdict");
    let dimensions = [
        &assurance.bounds,
        &assurance.faults,
        &assurance.fairness,
        &assurance.values,
        &assurance.schedules,
        &assurance.memory_model,
        &assurance.observer,
        &assurance.proof_status,
        &assurance.unknowns,
    ];
    assert_eq!(dimensions.len(), 9);
    for dimension in dimensions {
        match dimension {
            EnvelopeDimension::Produced(produced) => {
                assert_eq!(produced.engine, verification::ENGINE);
                assert!(!produced.summary.is_empty());
            }
            EnvelopeDimension::Unsupported(unsupported) => {
                assert!(
                    !unsupported.reason.is_empty(),
                    "a typed reason, never a blank"
                );
            }
        }
    }
    match &assurance.bounds {
        EnvelopeDimension::Produced(produced) => assert_eq!(produced.summary, "exhaustive-finite"),
        other => panic!("a closed exploration produces its bounds dimension, got {other:?}"),
    }

    match &outcome.payload {
        Payload::VerificationResult(result) => {
            assert_eq!(result.task, task);
            assert_eq!(
                result.fragments,
                vec![continuumd::protocol::vocabulary::Fragment::Finite]
            );
            assert!(result.continuation.is_absent());
            assert!(result.crashpack.is_absent());
        }
        other => panic!("expected a verification.result payload, got {other:?}"),
    }
    // INV-007: the five fragments the campaign did not cover, and the two artifacts it cannot
    // build, are *named* rather than implied by their absence.
    let subjects: Vec<&str> = outcome
        .envelope
        .omissions
        .iter()
        .map(|omission| omission.subject.as_str())
        .collect();
    for expected in [
        "Symbolic",
        "Temporal",
        "Probabilistic",
        "Theorem",
        "Runtime",
        "crashpack",
    ] {
        assert!(
            subjects.contains(&expected),
            "{expected} is not named in {subjects:?}"
        );
    }
}

/// A single-property target establishes the invariant that does hold, over the same closed
/// exploration — so the verdict is a property of the *target*, not of the model.
#[test]
fn a_single_property_target_establishes_the_invariant_that_holds() {
    let mut fixture = fixture();
    let task = started_task(&start(
        &mut fixture,
        "req_start",
        "idem-start",
        Some(64),
        target(TargetKind::Property, diehard::TYPE_OK),
    ));
    let outcome = fixture.daemon.dispatch(&OperationRequest {
        envelope: envelope(
            "verification.result",
            "agent:runner",
            "cap_runner",
            "req_result",
        ),
        arguments: Arguments::VerificationResult(VerificationResultRequest { task }),
    });
    match &outcome.envelope.verdict {
        Nullable::Value(Verdict::Semantic(value)) => {
            assert_eq!(value.verdict, SemanticVerdict::Established);
            assert_eq!(value.assurance_class, AssuranceClass::Validated);
        }
        other => panic!("expected a semantic verdict, got {other:?}"),
    }
}

/// `verification.await` answers exactly what `verification.result` answers, which is what
/// "waiting never changes a verdict" means at a grain where a task is already terminal or
/// already parked before any operation can look at it.
#[test]
fn awaiting_a_terminal_task_returns_the_same_result_it_already_had() {
    let mut fixture = fixture();
    let task = started_task(&start(
        &mut fixture,
        "req_start",
        "idem-start",
        Some(64),
        target(TargetKind::AllClaims, "DieHard"),
    ));
    let read = fixture.daemon.dispatch(&OperationRequest {
        envelope: envelope(
            "verification.result",
            "agent:runner",
            "cap_runner",
            "req_read",
        ),
        arguments: Arguments::VerificationResult(VerificationResultRequest { task: task.clone() }),
    });
    let waited = fixture.daemon.dispatch(&OperationRequest {
        envelope: budgeted(
            envelope(
                "verification.await",
                "agent:runner",
                "cap_runner",
                "req_read",
            ),
            Some(64),
        ),
        arguments: Arguments::VerificationAwait(VerificationAwaitRequest {
            task,
            timeout_ms: Optional::Absent,
        }),
    });
    assert_eq!(waited.envelope.status, ResultStatus::Ok);
    assert_eq!(waited.envelope.verdict, read.envelope.verdict);
    assert_eq!(waited.envelope.assurance, read.envelope.assurance);
    match (&waited.payload, &read.payload) {
        (Payload::VerificationAwait(one), Payload::VerificationResult(two)) => assert_eq!(one, two),
        other => panic!("unexpected payloads {other:?}"),
    }
}

// --- parking, budget updates, and resume -------------------------------------------------

/// The full B18 loop: a bounded campaign parks with a continuation, `task.update_budget`
/// re-admits it under a larger bound with no identity churn, and `task.resume` closes it.
///
/// The last assertion is the one that makes "resume" honest rather than "restart": the parked
/// frontier is a subset of what the resumed run discovered, so the resumed exploration
/// extends the parked one instead of replacing it (INV-009, `rule task.resume`'s "the
/// frontier after resume MUST include the frontier before it").
#[test]
fn budget_exhaustion_parks_a_continuation_that_update_budget_and_resume_complete() {
    let mut fixture = fixture();
    let started = start(
        &mut fixture,
        "req_start",
        "idem-start",
        Some(4),
        target(TargetKind::AllClaims, "DieHard"),
    );
    let task = started_task(&started);

    let parked = record(&status(&mut fixture, &task, "req_status_1"));
    assert_eq!(parked.status, TaskStatus::Suspended);
    let continuation = parked
        .continuation
        .value()
        .cloned()
        .expect("a suspended task is resumable by definition (plan §11.4)");
    assert!(continuation.as_str().starts_with("cont_"));
    assert_eq!(
        parked.cost.states,
        Optional::Present(3),
        "a state bound of 4 stops the walk before its fourth expansion, deterministically"
    );
    assert!(
        parked
            .cost
            .states
            .value()
            .is_some_and(|states| *states < FROZEN_STATES)
    );

    let frontier = fixture
        .daemon
        .state()
        .tasks()
        .continuation(&continuation)
        .expect("the daemon holds the continuation")
        .frontier
        .clone();
    assert!(
        !frontier.is_empty(),
        "a parked run has somewhere to resume from"
    );

    // Budget exhaustion is never a verdict: it is `inconclusive` with the typed reason, and
    // the result carries the continuation.
    let interim = fixture.daemon.dispatch(&OperationRequest {
        envelope: envelope(
            "verification.result",
            "agent:runner",
            "cap_runner",
            "req_interim",
        ),
        arguments: Arguments::VerificationResult(VerificationResultRequest { task: task.clone() }),
    });
    match &interim.envelope.verdict {
        Nullable::Value(Verdict::Semantic(value)) => {
            assert_eq!(value.verdict, SemanticVerdict::Inconclusive);
            assert_eq!(
                value.inconclusive_reason,
                Optional::Present(InconclusiveReason::ResourceExhausted)
            );
            assert_eq!(value.assurance_class, AssuranceClass::Bounded);
        }
        other => panic!("expected a semantic verdict, got {other:?}"),
    }
    match &interim.payload {
        Payload::VerificationResult(result) => {
            assert_eq!(result.continuation, Optional::Present(continuation.clone()));
        }
        other => panic!("expected a verification.result payload, got {other:?}"),
    }

    let raised = fixture.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope(
                "task.update_budget",
                "agent:runner",
                "cap_runner",
                "req_budget",
            ),
            "idem-budget",
        ),
        arguments: Arguments::TaskUpdateBudget(TaskUpdateBudgetRequest {
            task: task.clone(),
            budget: budget(Some(64)),
        }),
    });
    assert_eq!(raised.envelope.status, ResultStatus::Ok);
    match &raised.payload {
        Payload::TaskUpdateBudget(response) => {
            assert_eq!(response.task, task, "no identity churn");
            assert_eq!(response.status, TaskStatus::Suspended);
            assert_eq!(response.budget.states, Optional::Present(64));
            assert_eq!(
                response.continuation,
                Optional::Present(continuation.clone())
            );
        }
        other => panic!("expected a task.update_budget payload, got {other:?}"),
    }
    assert_eq!(
        raised.envelope.verdict,
        structural(StructuralOutcome::Updated)
    );

    let resumed = fixture.daemon.dispatch(&OperationRequest {
        envelope: budgeted(
            keyed(
                envelope("task.resume", "agent:runner", "cap_runner", "req_resume"),
                "idem-resume",
            ),
            Some(64),
        ),
        arguments: Arguments::TaskResume(TaskResumeRequest {
            continuation,
            budget: Optional::Absent,
        }),
    });
    assert_eq!(
        resumed.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        resumed.envelope.error
    );
    match &resumed.payload {
        Payload::TaskResume(response) => {
            assert_eq!(
                response.task, task,
                "one task identity across the whole loop"
            );
            assert_eq!(response.status, TaskStatus::Completed);
        }
        other => panic!("expected a task.resume payload, got {other:?}"),
    }

    let closed = record(&status(&mut fixture, &task, "req_status_2"));
    assert_eq!(closed.status, TaskStatus::Completed);
    assert_eq!(closed.cost.states, Optional::Present(FROZEN_STATES));
    assert!(closed.continuation.is_absent());

    let campaign = fixture
        .daemon
        .state()
        .tasks()
        .get(&task)
        .expect("held")
        .campaign
        .as_ref()
        .expect("ran");
    assert_eq!(campaign.transitions, FROZEN_TRANSITIONS);
    // Monotone: every state the parked run left on its frontier was discovered by the
    // resumed run, so the resumed exploration extends the parked one.
    let reachable = diehard::model()
        .and_then(|model| {
            continuum_engine_reference::bfs::explore(
                &model,
                continuum_engine_reference::bfs::Bounds::CERTIFIABLE.with_states(64),
            )
            .map_err(|_| continuum_engine_reference::model::ModelError::NoVariables)
        })
        .expect("the closed walk");
    for state in &frontier {
        assert!(
            reachable.reachable().contains(state),
            "the parked frontier is part of the resumed exploration"
        );
    }
}

/// A state budget too small to hold the model's own initial states is a *failed task*, not a
/// refused request: `failed_reason = BudgetExhausted` with the `non_resumable_reason` RFC
/// 0026 requires beside it, and no continuation to pretend otherwise.
#[test]
fn a_budget_that_cannot_hold_the_initial_states_fails_the_task_non_resumably() {
    let mut fixture = fixture();
    let task = started_task(&start(
        &mut fixture,
        "req_start",
        "idem-start",
        Some(0),
        target(TargetKind::AllClaims, "DieHard"),
    ));
    let record = record(&status(&mut fixture, &task, "req_status"));
    assert_eq!(record.status, TaskStatus::Failed);
    assert_eq!(
        record.failed_reason,
        Optional::Present(ErrorCode::BudgetExhausted)
    );
    assert!(
        record.non_resumable_reason.value().is_some(),
        "a failed task is never silent (plan §4.5)"
    );
    assert!(record.continuation.is_absent());

    // And the one result path that must be an error rather than a verdict.
    let result = fixture.daemon.dispatch(&OperationRequest {
        envelope: envelope(
            "verification.result",
            "agent:runner",
            "cap_runner",
            "req_result",
        ),
        arguments: Arguments::VerificationResult(VerificationResultRequest { task }),
    });
    assert_eq!(code(&result), ErrorCode::BudgetExhausted);
}

// --- resume admissibility ----------------------------------------------------------------

fn park(fixture: &mut Fixture) -> (TaskHandle, ContinuationHandle) {
    let task = started_task(&start(
        fixture,
        "req_start",
        "idem-start",
        Some(4),
        target(TargetKind::AllClaims, "DieHard"),
    ));
    let continuation = record(&status(fixture, &task, "req_status"))
        .continuation
        .value()
        .cloned()
        .expect("the bounded run parked");
    (task, continuation)
}

fn resume(
    fixture: &mut Fixture,
    continuation: &ContinuationHandle,
    snapshot: Option<&WorkspaceHandle>,
    request: &str,
) -> OperationOutcome {
    let mut request_envelope = budgeted(
        keyed(
            envelope("task.resume", "agent:runner", "cap_runner", request),
            "idem-resume",
        ),
        Some(64),
    );
    if let Some(snapshot) = snapshot {
        request_envelope = on(request_envelope, snapshot);
    }
    fixture.daemon.dispatch(&OperationRequest {
        envelope: request_envelope,
        arguments: Arguments::TaskResume(TaskResumeRequest {
            continuation: continuation.clone(),
            // `task.resume` carries its own optional budget, which is the second of the two
            // ways RFC 0026 lets a parked task be re-admitted; `task.update_budget` is the
            // first, and the loop test above exercises that one.
            budget: Optional::Present(budget(Some(64))),
        }),
    })
}

/// The R3 spike §3's proven behaviour: a continuation resumed under a *different* workspace
/// snapshot is refused, and the refusal is typed.
#[test]
fn resuming_a_continuation_under_a_different_snapshot_is_a_stale_snapshot() {
    let mut fixture = fixture();
    let (_, continuation) = park(&mut fixture);
    let elsewhere = WorkspaceHandle::new("ws_elsewhere").expect("a handle");
    let refused = resume(&mut fixture, &continuation, Some(&elsewhere), "req_resume");
    assert_eq!(code(&refused), ErrorCode::StaleSnapshot);
    assert_eq!(refused.payload, Payload::None, "no partial effect");
}

/// Naming the snapshot the continuation actually pinned is admitted, so the check above is
/// about *disagreement* and not about the field's presence.
#[test]
fn resuming_a_continuation_under_its_own_snapshot_is_admitted() {
    let mut fixture = fixture();
    let (task, continuation) = park(&mut fixture);
    let snapshot = fixture.snapshot.clone();
    let resumed = resume(&mut fixture, &continuation, Some(&snapshot), "req_resume");
    assert_eq!(resumed.envelope.status, ResultStatus::Ok);
    assert_eq!(
        record(&status(&mut fixture, &task, "req_status_2")).status,
        TaskStatus::Completed
    );
}

/// A continuation whose pinned snapshot has been superseded in its lineage is refused: the
/// lineage advanced under it, so the snapshot it names is no longer current.
#[test]
fn resuming_against_an_advanced_lineage_is_a_stale_snapshot() {
    let mut fixture = fixture();
    let (_, continuation) = park(&mut fixture);
    let base = fixture.snapshot.clone();
    let forked = fixture.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope("workspace.fork", "agent:builder", "cap_builder", "req_fork"),
            "idem-fork",
        ),
        arguments: Arguments::WorkspaceFork(WorkspaceForkRequest {
            base,
            overlay: Optional::Present(vec![FileOverlay {
                path: "README.md".to_owned(),
                content: b"edited\n".to_vec(),
            }]),
            patches: Optional::Absent,
        }),
    });
    assert_eq!(forked.envelope.status, ResultStatus::Ok);

    let refused = resume(&mut fixture, &continuation, None, "req_resume");
    assert_eq!(code(&refused), ErrorCode::StaleSnapshot);
}

/// A continuation pinning a semantic epoch this daemon does not serve is
/// `ContinuationEpochMismatch`; one pinning a *kind* the daemon pins nothing for is
/// `EpochUnsupported`. INV-008 keeps them distinct, and so does the decision table.
#[test]
fn resume_admissibility_separates_an_epoch_disagreement_from_an_unheld_epoch() {
    for (mutate, expected) in [
        (
            (|pinned: &mut PinnedEpochs| {
                pinned.semantic = Nullable::Value(epoch("semantic-2"));
            }) as fn(&mut PinnedEpochs),
            ErrorCode::ContinuationEpochMismatch,
        ),
        (
            |pinned: &mut PinnedEpochs| {
                pinned.corpus = Nullable::Value(epoch("corpus-1"));
            },
            ErrorCode::EpochUnsupported,
        ),
        (
            |pinned: &mut PinnedEpochs| {
                pinned.engine = Nullable::Value(epoch("engine-reference-2"));
            },
            ErrorCode::ContinuationEpochMismatch,
        ),
    ] {
        let mut fixture = fixture();
        let (task, continuation) = park(&mut fixture);
        let mut forged = fixture
            .daemon
            .state()
            .tasks()
            .continuation(&continuation)
            .expect("held")
            .clone();
        mutate(&mut forged.pinned);
        forged.handle = ContinuationHandle::new("cont_forged").expect("a handle");
        let handle = forged.handle.clone();
        fixture.daemon.state_mut().tasks_mut().park(forged);

        let refused = resume(&mut fixture, &handle, None, "req_resume");
        assert_eq!(code(&refused), expected);
        assert_eq!(
            record(&status(&mut fixture, &task, "req_status_2")).status,
            TaskStatus::Suspended,
            "a refused resume never advances the task"
        );
    }
}

/// The predicate itself, over the six identities a continuation can pin. Written against
/// [`admissible_epochs`] directly so every cell of RFC 0026's decision table is exercised,
/// including the permissive one: an epoch the continuation left unpinned constrains nothing.
#[test]
fn the_two_predicate_obligation_is_checked_on_both_halves() {
    let current = epochs();
    let held = PinnedEpochs::of(&current);
    assert!(admissible_epochs(&held, &current).is_ok());

    // Unpinned constrains nothing.
    let mut permissive = held.clone();
    permissive.semantic = Nullable::Null;
    permissive.engine = Nullable::Null;
    assert!(admissible_epochs(&permissive, &current).is_ok());

    // P1 alone is not enough: the compatibility epochs agree and the engine does not.
    let mut engine_only = held.clone();
    engine_only.engine = Nullable::Value(epoch("engine-reference-2"));
    assert_eq!(
        admissible_epochs(&engine_only, &current)
            .expect_err("P2 fails")
            .code,
        ErrorCode::ContinuationEpochMismatch
    );

    // P2 alone is not enough either.
    let mut semantic_only = held.clone();
    semantic_only.semantic = Nullable::Value(epoch("semantic-2"));
    assert_eq!(
        admissible_epochs(&semantic_only, &current)
            .expect_err("P1 fails")
            .code,
        ErrorCode::ContinuationEpochMismatch
    );

    // The protocol epoch is not a field, so it cannot participate: two daemons differing only
    // in protocol version pin the same set.
    let mut other_protocol = current.clone();
    other_protocol.protocol = ProtocolVersion::new(3, 0);
    assert!(admissible_epochs(&held, &other_protocol).is_ok());
}

/// A continuation this daemon does not hold is a denial, not a not-found (RFC 0027 X2).
#[test]
fn resuming_an_unheld_continuation_is_a_denial() {
    let mut fixture = fixture();
    let unknown = ContinuationHandle::new("cont_nothing").expect("a handle");
    let refused = resume(&mut fixture, &unknown, None, "req_resume");
    assert_eq!(code(&refused), ErrorCode::CapabilityDenied);
}

/// Resume of a terminal task is a monotone no-op: the terminal status comes back and nothing
/// is re-run, because `rule task.status_monotonic` says a terminal status never changes.
#[test]
fn resuming_a_terminal_task_changes_nothing() {
    let mut fixture = fixture();
    let (task, continuation) = park(&mut fixture);
    let cancelled = cancel(&mut fixture, &task, "req_cancel", "idem-cancel");
    assert_eq!(cancelled.envelope.status, ResultStatus::Ok);

    let resumed = resume(&mut fixture, &continuation, None, "req_resume");
    assert_eq!(resumed.envelope.status, ResultStatus::Ok);
    match &resumed.payload {
        Payload::TaskResume(response) => assert_eq!(response.status, TaskStatus::Cancelled),
        other => panic!("expected a task.resume payload, got {other:?}"),
    }
    assert_eq!(
        record(&status(&mut fixture, &task, "req_status_2")).status,
        TaskStatus::Cancelled
    );
}

// --- cancellation ------------------------------------------------------------------------

fn cancel(fixture: &mut Fixture, task: &TaskHandle, request: &str, key: &str) -> OperationOutcome {
    fixture.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope("task.cancel", "agent:runner", "cap_runner", request),
            key,
        ),
        arguments: Arguments::TaskCancel(TaskCancelRequest { task: task.clone() }),
    })
}

/// Cancellation at each lifecycle point a caller can reach.
///
/// A suspended task cancels *with* its continuation — "committed partial evidence plus a
/// valid continuation" — and a terminal one is `unchanged`, whichever terminal state it is in.
#[test]
fn cancellation_is_correct_at_every_lifecycle_point_a_caller_can_reach() {
    // Suspended → cancelled, carrying the continuation.
    let mut suspended = fixture();
    let (task, continuation) = park(&mut suspended);
    let cancelled = cancel(&mut suspended, &task, "req_cancel", "idem-cancel");
    assert_eq!(cancelled.envelope.status, ResultStatus::Ok);
    assert_eq!(
        cancelled.envelope.verdict,
        structural(StructuralOutcome::Cancelled)
    );
    match &cancelled.payload {
        Payload::TaskCancel(response) => {
            assert_eq!(response.status, TaskStatus::Cancelled);
            assert_eq!(response.continuation, Nullable::Value(continuation));
            assert!(response.committed_evidence.is_empty());
        }
        other => panic!("expected a task.cancel payload, got {other:?}"),
    }

    // Cancelling twice: the second changes nothing.
    let again = cancel(&mut suspended, &task, "req_cancel_2", "idem-cancel-2");
    assert_eq!(
        again.envelope.verdict,
        structural(StructuralOutcome::Unchanged)
    );
    match &again.payload {
        Payload::TaskCancel(response) => assert_eq!(response.status, TaskStatus::Cancelled),
        other => panic!("expected a task.cancel payload, got {other:?}"),
    }

    // Completed → unchanged, and the terminal status survives.
    let mut completed = fixture();
    let done = started_task(&start(
        &mut completed,
        "req_start",
        "idem-start",
        Some(64),
        target(TargetKind::AllClaims, "DieHard"),
    ));
    let refused = cancel(&mut completed, &done, "req_cancel", "idem-cancel");
    assert_eq!(
        refused.envelope.verdict,
        structural(StructuralOutcome::Unchanged)
    );
    match &refused.payload {
        Payload::TaskCancel(response) => {
            assert_eq!(response.status, TaskStatus::Completed);
            assert_eq!(response.continuation, Nullable::Null, "nothing published");
        }
        other => panic!("expected a task.cancel payload, got {other:?}"),
    }

    // Failed → unchanged.
    let mut broken = fixture();
    let failed = started_task(&start(
        &mut broken,
        "req_start",
        "idem-start",
        Some(0),
        target(TargetKind::AllClaims, "DieHard"),
    ));
    let refused = cancel(&mut broken, &failed, "req_cancel", "idem-cancel");
    assert_eq!(
        refused.envelope.verdict,
        structural(StructuralOutcome::Unchanged)
    );

    // A task this daemon does not hold is a denial, never a not-found.
    let unknown = TaskHandle::new("task_nothing").expect("a handle");
    let denied = cancel(&mut broken, &unknown, "req_cancel_3", "idem-cancel-3");
    assert_eq!(code(&denied), ErrorCode::CapabilityDenied);
}

/// A terminal task's budget is a historical fact and `task.update_budget` does not rewrite it.
#[test]
fn updating_the_budget_of_a_terminal_task_changes_nothing() {
    let mut fixture = fixture();
    let task = started_task(&start(
        &mut fixture,
        "req_start",
        "idem-start",
        Some(64),
        target(TargetKind::AllClaims, "DieHard"),
    ));
    let outcome = fixture.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope(
                "task.update_budget",
                "agent:runner",
                "cap_runner",
                "req_budget",
            ),
            "idem-budget",
        ),
        arguments: Arguments::TaskUpdateBudget(TaskUpdateBudgetRequest {
            task,
            budget: budget(Some(1)),
        }),
    });
    assert_eq!(
        outcome.envelope.verdict,
        structural(StructuralOutcome::Unchanged)
    );
    match &outcome.payload {
        Payload::TaskUpdateBudget(response) => {
            assert_eq!(response.status, TaskStatus::Completed);
            assert_eq!(
                response.budget.states,
                Optional::Present(64),
                "the budget it ran under, not the one that arrived after"
            );
        }
        other => panic!("expected a task.update_budget payload, got {other:?}"),
    }
}

// --- subscription ------------------------------------------------------------------------

/// `task.subscribe` answers the record the IDL declares as its response body, and the events
/// a transport would replay are the transitions the task already recorded.
#[test]
fn subscribing_answers_the_record_and_the_transitions_already_recorded() {
    let mut fixture = fixture();
    let task = started_task(&start(
        &mut fixture,
        "req_start",
        "idem-start",
        Some(64),
        target(TargetKind::AllClaims, "DieHard"),
    ));
    let outcome = fixture.daemon.dispatch(&OperationRequest {
        envelope: envelope("task.subscribe", "agent:reader", "cap_reader", "req_sub"),
        arguments: Arguments::TaskSubscribe(TaskSubscribeRequest { task: task.clone() }),
    });
    assert_eq!(outcome.envelope.status, ResultStatus::Ok);
    let subscribed = match &outcome.payload {
        Payload::TaskSubscribe(response) => response.record.clone(),
        other => panic!("expected a task.subscribe payload, got {other:?}"),
    };
    // `rule subscription.hints_only`: re-reading recovers the same state.
    assert_eq!(
        subscribed,
        record(&status(&mut fixture, &task, "req_status"))
    );

    let entry = fixture.daemon.state().tasks().get(&task).expect("held");
    assert!(
        !entry.events.is_empty(),
        "a deployment that supplied a clock reading records its transitions"
    );
    assert!(
        entry
            .milestones
            .iter()
            .any(|milestone| milestone.name == verification::MILESTONE_CLOSED),
        "the closed exploration is a named milestone"
    );
}

/// With no clock reading, no milestone can be timed — and the result *names* the absence
/// instead of returning an empty list a caller has to interpret (INV-007).
#[test]
fn a_daemon_with_no_clock_records_no_milestone_and_says_so() {
    let mut clockless = clockless_fixture();
    let task = started_task(&start(
        &mut clockless,
        "req_start",
        "idem-start",
        Some(64),
        target(TargetKind::AllClaims, "DieHard"),
    ));
    let record = record(&status(&mut clockless, &task, "req_status"));
    assert!(record.milestones.is_empty());
    let outcome = status(&mut clockless, &task, "req_status_2");
    assert!(
        outcome
            .envelope
            .omissions
            .iter()
            .any(|omission| omission.subject == "task.milestones"),
        "the absence is named, not inferred"
    );
}

// --- idempotency -------------------------------------------------------------------------

/// The PR 5 exit: an identical replay returns the same task identity — and here the *same
/// bytes*, because the ledger returns the recorded outcome verbatim.
#[test]
fn replaying_an_idempotent_verification_start_returns_the_same_task_identity() {
    let mut fixture = fixture();
    let first = start(
        &mut fixture,
        "req_start",
        "idem-start",
        Some(64),
        target(TargetKind::AllClaims, "DieHard"),
    );
    let replay = start(
        &mut fixture,
        "req_start",
        "idem-start",
        Some(64),
        target(TargetKind::AllClaims, "DieHard"),
    );
    assert_eq!(first, replay, "the ledger returns the recorded outcome");
    assert_eq!(started_task(&first), started_task(&replay));
    assert_eq!(
        fixture.daemon.state().tasks().handles().len(),
        1,
        "one campaign, one task"
    );
}

/// Reusing a key for a *different* request is rejected — R3 spike §3's proven behaviour, and
/// the code `rule idempotency.replay` reserves for it.
#[test]
fn reusing_an_idempotency_key_for_a_different_request_is_rejected() {
    let mut fixture = fixture();
    start(
        &mut fixture,
        "req_start",
        "idem-start",
        Some(64),
        target(TargetKind::AllClaims, "DieHard"),
    );
    let mismatched = start(
        &mut fixture,
        "req_start_2",
        "idem-start",
        Some(64),
        target(TargetKind::Property, diehard::TYPE_OK),
    );
    assert_eq!(code(&mismatched), ErrorCode::IdempotencyKeyReused);
    assert_eq!(mismatched.payload, Payload::None, "no partial effect");
}

/// A second start of the *same* campaign under a *different* key is the IDL's cached-result
/// lane: `result` present instead of `task`, no second campaign, no second identity.
#[test]
fn a_second_start_under_a_new_key_returns_the_cached_result() {
    let mut fixture = fixture();
    let first = start(
        &mut fixture,
        "req_start",
        "idem-start",
        Some(64),
        target(TargetKind::AllClaims, "DieHard"),
    );
    let task = started_task(&first);
    let second = start(
        &mut fixture,
        "req_start_2",
        "idem-start-2",
        Some(64),
        target(TargetKind::AllClaims, "DieHard"),
    );
    assert_eq!(second.envelope.status, ResultStatus::Ok);
    match &second.payload {
        Payload::VerificationStart(response) => {
            assert!(
                response.task.is_absent(),
                "a cached result names no new task"
            );
            let result = response.result.value().expect("the cached result");
            assert_eq!(result.task, task, "the same task identity either way");
        }
        other => panic!("expected a verification.start payload, got {other:?}"),
    }
    assert_eq!(fixture.daemon.state().tasks().handles().len(), 1);
}

/// The task identity is a function of the campaign, so it is stable across daemons: two
/// independently built daemons name the same task for the same request.
#[test]
fn the_task_identity_is_a_function_of_the_campaign_and_of_nothing_else() {
    let mut one = fixture();
    let mut two = fixture();
    let first = start(
        &mut one,
        "req_start",
        "idem-start",
        Some(64),
        target(TargetKind::AllClaims, "DieHard"),
    );
    let second = start(
        &mut two,
        "req_start",
        "idem-start",
        Some(64),
        target(TargetKind::AllClaims, "DieHard"),
    );
    assert_eq!(first, second, "`rule ordering.deterministic`");
    assert_eq!(started_task(&first), started_task(&second));

    // A different budget is a different campaign and therefore a different task.
    let mut three = fixture();
    let other = start(
        &mut three,
        "req_start",
        "idem-start",
        Some(63),
        target(TargetKind::AllClaims, "DieHard"),
    );
    assert_ne!(started_task(&first), started_task(&other));
}

// --- refusals ----------------------------------------------------------------------------

/// A snapshot whose modules are not a registered model is the typed refusal
/// `rule errors.unsupported_surface` requires, never a degraded answer.
#[test]
fn a_snapshot_whose_modules_are_not_a_registered_model_is_unsupported() {
    let mut fixture = fixture();
    // Fork the model file itself: a changed model is a different model, and nothing is
    // registered for it.
    let base = fixture.snapshot.clone();
    let forked = fixture.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope("workspace.fork", "agent:builder", "cap_builder", "req_fork"),
            "idem-fork",
        ),
        arguments: Arguments::WorkspaceFork(WorkspaceForkRequest {
            base,
            overlay: Optional::Present(vec![FileOverlay {
                path: MODULE_PATH.to_owned(),
                content: b"// not the corpus port\n".to_vec(),
            }]),
            patches: Optional::Absent,
        }),
    });
    let derived = match &forked.payload {
        Payload::WorkspaceFork(response) => response.snapshot.clone(),
        other => panic!("expected a workspace.fork payload, got {other:?}"),
    };
    // The fork is not sealed, so seal it first — the refusal under test is the model one.
    let sealed = fixture.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope("workspace.seal", "agent:builder", "cap_builder", "req_seal"),
            "idem-seal",
        ),
        arguments: Arguments::WorkspaceSeal(
            continuumd::protocol::operations::workspace::WorkspaceSealRequest {
                snapshot: derived.clone(),
            },
        ),
    });
    assert_eq!(sealed.envelope.status, ResultStatus::Ok);

    fixture.snapshot = derived;
    let refused = start(
        &mut fixture,
        "req_start",
        "idem-start",
        Some(64),
        target(TargetKind::AllClaims, "DieHard"),
    );
    assert_eq!(code(&refused), ErrorCode::UnsupportedSemanticFeature);
}

/// A campaign runs over a *sealed* snapshot: an unsealed one is `StaleSnapshot`, which the
/// code's own definition covers — "not current, **or is not sealed where sealing is
/// required**".
#[test]
fn a_campaign_over_an_unsealed_snapshot_is_a_stale_snapshot() {
    let mut fixture = fixture();
    let base = fixture.snapshot.clone();
    let forked = fixture.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope("workspace.fork", "agent:builder", "cap_builder", "req_fork"),
            "idem-fork",
        ),
        arguments: Arguments::WorkspaceFork(WorkspaceForkRequest {
            base,
            overlay: Optional::Present(vec![FileOverlay {
                path: "README.md".to_owned(),
                content: b"edited\n".to_vec(),
            }]),
            patches: Optional::Absent,
        }),
    });
    fixture.snapshot = match &forked.payload {
        Payload::WorkspaceFork(response) => response.snapshot.clone(),
        other => panic!("expected a workspace.fork payload, got {other:?}"),
    };
    let refused = start(
        &mut fixture,
        "req_start",
        "idem-start",
        Some(64),
        target(TargetKind::AllClaims, "DieHard"),
    );
    assert_eq!(code(&refused), ErrorCode::StaleSnapshot);
}

/// A campaign needs a snapshot to run over, and the envelope's `snapshot` is nullable.
#[test]
fn a_campaign_with_no_snapshot_is_malformed() {
    let mut fixture = fixture();
    let refused = fixture.daemon.dispatch(&OperationRequest {
        envelope: budgeted(
            keyed(
                envelope(
                    "verification.start",
                    "agent:runner",
                    "cap_runner",
                    "req_start",
                ),
                "idem-start",
            ),
            Some(64),
        ),
        arguments: Arguments::VerificationStart(VerificationStartRequest {
            target: target(TargetKind::AllClaims, "DieHard"),
            portfolio: Portfolio::Interactive,
            context_policy: Optional::Absent,
            priority_class: Optional::Absent,
        }),
    });
    assert_eq!(code(&refused), ErrorCode::MalformedRequest);
}

/// A custom portfolio has no definition on the wire, and a target kind that is not a question
/// about finite reachability is refused rather than answered approximately.
#[test]
fn unsupported_portfolios_and_target_kinds_are_typed_refusals() {
    let mut fixture = fixture();
    let snapshot = fixture.snapshot.clone();
    let custom = fixture.daemon.dispatch(&OperationRequest {
        envelope: on(
            budgeted(
                keyed(
                    envelope(
                        "verification.start",
                        "agent:runner",
                        "cap_runner",
                        "req_custom",
                    ),
                    "idem-custom",
                ),
                Some(64),
            ),
            &snapshot,
        ),
        arguments: Arguments::VerificationStart(VerificationStartRequest {
            target: target(TargetKind::AllClaims, "DieHard"),
            portfolio: Portfolio::Custom,
            context_policy: Optional::Absent,
            priority_class: Optional::Absent,
        }),
    });
    assert_eq!(code(&custom), ErrorCode::UnsupportedSemanticFeature);

    let refinement = start(
        &mut fixture,
        "req_refinement",
        "idem-refinement",
        Some(64),
        target(TargetKind::Refinement, "anything"),
    );
    assert_eq!(code(&refinement), ErrorCode::UnsupportedSemanticFeature);

    let unknown_property = start(
        &mut fixture,
        "req_property",
        "idem-property",
        Some(64),
        target(TargetKind::Property, "NoSuchPredicate"),
    );
    assert_eq!(code(&unknown_property), ErrorCode::MalformedRequest);
}

/// RFC 0027 X2: a task that does not exist and a task that exists but is out of scope return
/// **byte-identical** envelopes. The two denials here differ only in which capability was
/// presented, and the audit identity is a function of the request identity alone.
#[test]
fn the_two_denial_paths_are_byte_identical() {
    let mut fixture = fixture();
    let task = started_task(&start(
        &mut fixture,
        "req_start",
        "idem-start",
        Some(64),
        target(TargetKind::AllClaims, "DieHard"),
    ));

    // Held, but the presented capability is scoped to classes that exclude `task`: denied by
    // admission, before the handler runs.
    let out_of_scope = fixture.daemon.dispatch(&OperationRequest {
        envelope: envelope("task.status", "agent:reader", "cap_elsewhere", "req_probe"),
        arguments: Arguments::TaskStatus(TaskStatusRequest { task }),
    });
    // Admitted, but the task does not exist: denied by the handler.
    let absent = fixture.daemon.dispatch(&OperationRequest {
        envelope: envelope("task.status", "agent:reader", "cap_reader", "req_probe"),
        arguments: Arguments::TaskStatus(TaskStatusRequest {
            task: TaskHandle::new("task_nothing").expect("a handle"),
        }),
    });
    assert_eq!(code(&out_of_scope), ErrorCode::CapabilityDenied);
    assert_eq!(code(&absent), ErrorCode::CapabilityDenied);
    assert_eq!(
        out_of_scope, absent,
        "an existence oracle is exactly what X2 forbids"
    );
}

// --- the error union ---------------------------------------------------------------------

/// Every code these two families can raise is inside `rule errors.common` ∪ the operation's
/// own `errors` clause. The tables in the families are data, so this holds them rather than
/// trusting that the handlers stayed inside the union.
#[test]
fn every_fault_these_families_raise_is_inside_the_operations_error_union() {
    for (operation, code) in task::FAULTS.iter().chain(verification::FAULTS) {
        let spec = registry::operation(operation).expect("a registered operation");
        assert!(
            errors::admits(spec, *code),
            "{operation} may not answer {code:?}"
        );
    }
}

/// The families answer for every operation their namespaces declare, so no operation in them
/// falls through to the dispatcher's `unsupported_surface` path.
#[test]
fn the_two_families_answer_for_every_operation_in_their_namespaces() {
    let declared: Vec<&str> = registry::OPERATIONS
        .iter()
        .map(|spec| spec.name)
        .filter(|name| name.starts_with("task.") || name.starts_with("verification."))
        .collect();
    assert_eq!(
        declared.len(),
        8,
        "5 task operations and 3 verification ones"
    );
    let answered = [
        "task.status",
        "task.cancel",
        "task.resume",
        "task.subscribe",
        "task.update_budget",
        "verification.start",
        "verification.result",
        "verification.await",
    ];
    for name in declared {
        assert!(answered.contains(&name), "{name} is not answered");
    }
}

// --- helpers -----------------------------------------------------------------------------

fn structural(outcome: StructuralOutcome) -> Nullable<Verdict> {
    Nullable::Value(Verdict::Structural(
        continuumd::protocol::envelope::StructuralVerdictValue { outcome },
    ))
}

/// The same fixture, on a daemon with no time reading.
fn clockless_fixture() -> Fixture {
    let mut daemon = Daemon::builder(Blake3Identity, negotiated(), cap("cap_root"))
        .epochs(epochs())
        .capability(
            grant(
                "cap_root",
                "service:continuumd",
                AuthorityLevel::Promote,
                4,
                Optional::Present(profile(&["intent.accept", "intent.reject", "intent.lock"])),
            ),
            None,
        )
        .capability(
            grant(
                "cap_builder",
                "agent:builder",
                AuthorityLevel::Propose,
                3,
                Optional::Absent,
            ),
            Some(cap("cap_root")),
        )
        .capability(
            grant(
                "cap_runner",
                "agent:runner",
                AuthorityLevel::Execute,
                3,
                Optional::Absent,
            ),
            Some(cap("cap_root")),
        )
        .capability(
            grant(
                "cap_reader",
                "agent:reader",
                AuthorityLevel::Read,
                3,
                Optional::Absent,
            ),
            Some(cap("cap_root")),
        )
        .capability(
            grant(
                "cap_steward",
                "human:steward",
                AuthorityLevel::ReviseIntent,
                3,
                Optional::Present(profile(&["intent.accept", "intent.reject", "intent.lock"])),
            ),
            Some(cap("cap_root")),
        )
        .family(WorkspaceFamily)
        .family(IntentFamily)
        .family(TaskFamily)
        .family(VerificationFamily)
        .build();

    let contract = die_hard_contract();
    let intent = intent_handle(&contract);
    // Seeded directly, already accepted: a local `intent.accept` now needs a clock (RFC
    // 0037 correction 23 extended, bn-342ek), so this clockless daemon could never reach
    // this state through the wire.
    daemon.state_mut().put_intent(
        intent.clone(),
        IntentRecord {
            contract,
            status: RegistryStatus::Accepted,
            supersedes: None,
            superseded_by: None,
            acceptance: Some(Acceptance {
                accepted_by: "human:steward".to_owned(),
                signature: "unsigned".to_owned(),
                timestamp: "2026-01-01T00:00:00.000Z".to_owned(),
                audit_record: "seeded-accepted".to_owned(),
                chain: Vec::new(),
            }),
        },
    );
    let mut files = Vec::new();
    for (path, content) in [(MODULE_PATH, DIE_HARD_MODEL), ("README.md", "# TV-009\n")] {
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
    daemon.state_mut().models_mut().register(
        die_hard_source(),
        diehard::model().expect("the port builds"),
    );
    let created = daemon.dispatch(&OperationRequest {
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
            components: SnapshotComponents {
                files,
                cml_modules: Vec::new(),
                rust_extraction: Vec::new(),
                domain_packs: Vec::new(),
                dependencies: Vec::new(),
                epochs: SnapshotEpochs {
                    semantic: epoch("semantic-1"),
                    proof: epoch("proof-1"),
                    toolchain: Optional::Absent,
                },
                intent: intent.clone(),
                correspondence: Vec::new(),
                proof_environment: Vec::new(),
                configuration: Vec::new(),
                file_components: Optional::Absent,
            },
            overlay: Optional::Absent,
            seal: Optional::Present(true),
        }),
    });
    assert_eq!(
        created.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        created.envelope.error
    );
    let snapshot = match &created.payload {
        Payload::WorkspaceCreate(response) => response.snapshot.clone(),
        other => panic!("expected a workspace.create payload, got {other:?}"),
    };
    Fixture {
        daemon,
        intent,
        snapshot,
    }
}

/// The model catalog is a plain, deterministic map: registering the same source twice is one
/// entry, and an unregistered source resolves to nothing.
#[test]
fn the_model_catalog_is_keyed_by_source_identity() {
    let mut catalog = ModelCatalog::new();
    assert!(catalog.is_empty());
    let source = die_hard_source();
    catalog.register(source.clone(), diehard::model().expect("the port builds"));
    catalog.register(source.clone(), diehard::model().expect("the port builds"));
    assert_eq!(catalog.len(), 1);
    assert!(catalog.get(&source).is_some());
    assert!(catalog.get(&Commitment::new("model_nothing")).is_none());
    // A different module set is a different identity.
    let other = model_source(&Blake3Identity, [(MODULE_PATH, b"// different".as_slice())])
        .expect("blake3 names it");
    assert_ne!(source, other);
    assert!(
        model_source(&Blake3Identity, []).is_none(),
        "no modules, no model"
    );
}

/// A task handle is a function of its preimage and of nothing else.
#[test]
fn task_identities_are_pure_functions_of_their_preimage() {
    let mut one = task::Preimage::new();
    one.text("verification.start");
    let mut two = task::Preimage::new();
    two.text("verification.start");
    assert_eq!(
        task_handle(&Blake3Identity, &one).expect("named"),
        task_handle(&Blake3Identity, &two).expect("named")
    );
    let mut three = task::Preimage::new();
    three.text("verification.await");
    assert_ne!(
        task_handle(&Blake3Identity, &one).expect("named"),
        task_handle(&Blake3Identity, &three).expect("named")
    );
}

/// A `Continuation` cannot be built without everything RFC 0026 requires it to pin — the
/// claim is structural, and this test is the compiler check made explicit.
#[test]
fn a_continuation_pins_every_field_the_rfc_requires() {
    let continuation = Continuation {
        handle: ContinuationHandle::new("cont_x").expect("a handle"),
        task: TaskHandle::new("task_x").expect("a handle"),
        snapshot: WorkspaceHandle::new("ws_x").expect("a handle"),
        intent: Nullable::Null,
        pinned: PinnedEpochs::of(&epochs()),
        bounds: continuum_engine_reference::bfs::Bounds::CERTIFIABLE,
        frontier: Vec::new(),
    };
    assert_eq!(
        continuation.pinned.semantic,
        Nullable::Value(epoch("semantic-1"))
    );
    assert_eq!(
        continuation.pinned.engine,
        Nullable::Value(epoch("engine-reference-1"))
    );
}

/// The fixture's intent is the one the workspace binds, so the campaign is intent-scoped in
/// the record as well as in the snapshot.
#[test]
fn a_started_task_names_the_intent_its_snapshot_binds() {
    let mut fixture = fixture();
    let intent = fixture.intent.clone();
    let task = started_task(&start(
        &mut fixture,
        "req_start",
        "idem-start",
        Some(64),
        target(TargetKind::AllClaims, "DieHard"),
    ));
    let record = record(&status(&mut fixture, &task, "req_status"));
    assert_eq!(record.intent, Nullable::Value(intent));
    assert_eq!(record.snapshot, Nullable::Value(fixture.snapshot.clone()));
}

// --- `OutputPolicy.max_bytes`, enforced (bn-6fuu5) -----------------------------------------
//
// `max_bytes` is declared "the enforced contract" (RFC 0026) and until this bone no handler
// read it. These are the end-to-end half of `continuumd::daemon::output`'s algebra: the same
// four dispositions, driven through `Daemon::dispatch` with a real campaign's record rather
// than a synthetic one, so the numbers below are the fixture's own measured sizes and not
// constants anybody chose.

/// `task.status` under a stated byte ceiling.
fn bounded_status(
    fixture: &mut Fixture,
    task: &TaskHandle,
    request: &str,
    max_bytes: u64,
) -> OperationOutcome {
    let mut envelope = envelope("task.status", "agent:reader", "cap_reader", request);
    envelope.output_policy = Optional::Present(OutputPolicy {
        max_bytes: Optional::Present(ByteCount::new(max_bytes)),
        max_tokens: Optional::Absent,
        max_nodes: Optional::Absent,
        audience: Optional::Absent,
    });
    fixture.daemon.dispatch(&OperationRequest {
        envelope,
        arguments: Arguments::TaskStatus(TaskStatusRequest { task: task.clone() }),
    })
}

/// The size of a record in the encoding this fixture negotiated.
fn measured(record: &continuumd::protocol::task::TaskRecord) -> u64 {
    output::measure(record, negotiated().encoding()).expect("a measurable record")
}

/// The trim records this answer carries — the ones a *ceiling* produced, as against the ones
/// the deployment's own limits produce.
fn trims(outcome: &OperationOutcome) -> Vec<&continuumd::protocol::envelope::Omission> {
    outcome
        .envelope
        .omissions
        .iter()
        .filter(|omission| omission.reason == OmissionReason::Budget)
        .collect()
}

/// A campaign whose record carries something a ceiling could take.
fn fixture_with_task() -> (Fixture, TaskHandle) {
    let mut fixture = fixture();
    let task = started_task(&start(
        &mut fixture,
        "req_start",
        "idem-start",
        Some(64),
        target(TargetKind::AllClaims, "DieHard"),
    ));
    (fixture, task)
}

/// **The control.** A caller that states no ceiling is answered exactly as it was before this
/// mechanism existed: the whole record, and a manifest carrying only the deployment's own
/// `unsupported` statements.
#[test]
fn a_status_with_no_stated_ceiling_is_the_whole_record_and_carries_no_trim() {
    let (mut fixture, task) = fixture_with_task();
    let outcome = status(&mut fixture, &task, "req_status");
    let record = record(&outcome);

    assert!(
        !record.milestones.is_empty(),
        "the fixture's campaign reached milestones, so there is something to take"
    );
    assert!(trims(&outcome).is_empty(), "nothing was trimmed");
    assert!(
        outcome
            .envelope
            .omissions
            .iter()
            .all(|omission| omission.reason == OmissionReason::Unsupported),
        "and every record on the manifest is a statement about the deployment"
    );
}

/// **At the bound.** `max_bytes` is a maximum, so a ceiling equal to the record's own size
/// serves it whole — and one byte less takes the first member of the declared order and no
/// more, naming the handle that recovers it.
#[test]
fn a_ceiling_at_the_record_size_serves_it_whole_and_one_byte_below_summarizes() {
    let (mut fixture, task) = fixture_with_task();
    let whole = record(&status(&mut fixture, &task, "req_status"));
    let size = measured(&whole);

    let at = bounded_status(&mut fixture, &task, "req_at", size);
    assert_eq!(at.envelope.status, ResultStatus::Ok);
    assert_eq!(
        record(&at),
        whole,
        "the answer at the ceiling is the record"
    );
    assert!(trims(&at).is_empty());

    let below = bounded_status(&mut fixture, &task, "req_below", size - 1);
    assert_eq!(below.envelope.status, ResultStatus::Ok);
    let summary = record(&below);
    assert!(summary.milestones.is_empty(), "the first member goes");
    assert!(
        measured(&summary) < size,
        "and the answer is inside the ceiling it was given"
    );

    // The four members a poll is for are all still there.
    assert_eq!(summary.task, whole.task);
    assert_eq!(summary.status, whole.status);
    assert_eq!(summary.cost, whole.cost);
    assert_eq!(summary.continuation, whole.continuation);
    // And the three the rule forbids a trim from touching.
    assert_eq!(summary.epochs, whole.epochs, "no epoch was dropped");
    assert_eq!(summary.snapshot, whole.snapshot);
    assert_eq!(summary.intent, whole.intent);

    let trims = trims(&below);
    assert_eq!(trims.len(), 1, "one record for the one member taken");
    assert_eq!(trims[0].subject, "task.milestones");
    assert_eq!(trims[0].reason, OmissionReason::Budget);
    assert_eq!(
        trims[0]
            .recoverable_by
            .value()
            .map(|handle| handle.as_str()),
        Some(task.as_str()),
        "INV-007's retrieval half, populated: the same question asked with more room"
    );
}

/// **Over the bound.** Below the floor there is no conforming answer, and the daemon says so
/// with a typed refusal rather than delivering a payload over the ceiling it was handed —
/// which is what "enforced" has to mean if it means anything.
#[test]
fn a_ceiling_below_the_floor_is_a_typed_refusal_and_never_an_answer_over_the_bound() {
    let (mut fixture, task) = fixture_with_task();
    let whole = record(&status(&mut fixture, &task, "req_status"));

    // The floor: every elidable member taken. Measured, not assumed.
    let floor = {
        let below = bounded_status(&mut fixture, &task, "req_floor_probe", 1_u64);
        assert_eq!(below.envelope.status, ResultStatus::Error);
        // A ceiling of one byte refuses, so the floor is read by asking for the smallest
        // ceiling that does not: the record with every elidable member gone.
        let mut candidate = whole.clone();
        candidate.milestones = Vec::new();
        candidate.committed_evidence = Vec::new();
        candidate.budget = Budget {
            wall_ms: Optional::Absent,
            cpu_ms: Optional::Absent,
            memory_bytes: Optional::Absent,
            states: Optional::Absent,
            solver_ms: Optional::Absent,
            proof_ms: Optional::Absent,
            tokens: Optional::Absent,
            candidates: Optional::Absent,
            bytes: Optional::Absent,
        };
        measured(&candidate)
    };

    let at_floor = bounded_status(&mut fixture, &task, "req_at_floor", floor);
    assert_eq!(
        at_floor.envelope.status,
        ResultStatus::Ok,
        "the floor is an answer"
    );
    assert!(measured(&record(&at_floor)) <= floor);

    let refused = bounded_status(&mut fixture, &task, "req_under_floor", floor - 1);
    assert_eq!(refused.envelope.status, ResultStatus::Error);
    assert_eq!(code(&refused), ErrorCode::MalformedRequest);
    let error = refused
        .envelope
        .error
        .value()
        .expect("an error result carries its error");
    assert_eq!(error.detail, output::CEILING_BELOW_FLOOR);
    assert!(!error.retryable, "the same request gets the same answer");
    assert!(
        matches!(refused.payload, Payload::None),
        "and no payload travelled: a refusal is not a smaller answer"
    );
    assert!(
        errors::admits(
            registry::operation("task.status").expect("a registered operation"),
            ErrorCode::MalformedRequest,
        ),
        "and the code is inside the union `errors []` leaves this operation"
    );
}

/// **INV-009.** A summary is a wire economy, never an information reduction: re-reading the
/// same task without the ceiling recovers everything the summary left out, and the manifest
/// said where to look before the caller asked.
#[test]
fn re_reading_without_the_ceiling_yields_a_superset_of_the_summary() {
    let (mut fixture, task) = fixture_with_task();
    let whole = record(&status(&mut fixture, &task, "req_status"));
    let size = measured(&whole);

    let summarized = bounded_status(&mut fixture, &task, "req_summary", size - 1);
    let summary = record(&summarized);
    let route = trims(&summarized)[0]
        .recoverable_by
        .value()
        .expect("the retrieval route is named")
        .as_str()
        .to_owned();
    assert_eq!(route, task.as_str());

    // Follow the route the manifest named: the same handle, read again with more room.
    let expanded = record(&status(&mut fixture, &task, "req_expand"));
    assert_eq!(expanded, whole, "the re-read is the whole record");
    for milestone in &summary.milestones {
        assert!(expanded.milestones.contains(milestone));
    }
    for handle in &summary.committed_evidence {
        assert!(expanded.committed_evidence.contains(handle));
    }
    assert!(
        expanded.milestones.len() > summary.milestones.len(),
        "and strictly more than the summary carried"
    );
}

/// The two omission reasons are two statements, and a ceiling never rewrites the deployment's.
#[test]
fn a_ceiling_records_budget_and_never_relabels_the_deployments_unsupported() {
    let (mut fixture, task) = fixture_with_task();
    let whole = record(&status(&mut fixture, &task, "req_status"));
    let outcome = bounded_status(&mut fixture, &task, "req_bounded", measured(&whole) - 1);

    let unsupported: Vec<&str> = outcome
        .envelope
        .omissions
        .iter()
        .filter(|omission| omission.reason == OmissionReason::Unsupported)
        .map(|omission| omission.subject.as_str())
        .collect();
    assert_eq!(
        unsupported,
        vec!["task.committed_evidence"],
        "the deployment commits no evidence, and a ceiling does not change that"
    );
    assert!(
        unsupported
            .iter()
            .all(|subject| *subject != "task.milestones"),
        "the milestones this campaign *did* reach are trimmed, never reported unproduced"
    );
    assert_eq!(
        trims(&outcome)
            .iter()
            .map(|omission| omission.subject.as_str())
            .collect::<Vec<_>>(),
        vec!["task.milestones"]
    );
    assert!(
        outcome
            .envelope
            .omissions
            .iter()
            .filter(|omission| omission.reason == OmissionReason::Unsupported)
            .all(|omission| omission.recoverable_by.is_absent()),
        "an `unsupported` record still names no route, because there is none to name"
    );
}

// --- replay re-decides the handles a campaign derived (bn-3hk4v, cr-3lrkq3) ---------------

/// Register a same-actor grant for `agent:runner` whose only difference from `cap_runner`
/// is the scope `narrow` applies, so it passes admission and misses one derived handle.
fn register_runner(
    fixture: &mut Fixture,
    handle: &str,
    narrow: impl FnOnce(&mut CapabilityDescriptor),
) {
    let mut descriptor = grant(
        handle,
        "agent:runner",
        AuthorityLevel::Execute,
        3,
        Optional::Absent,
    );
    narrow(&mut descriptor);
    fixture
        .daemon
        .state_mut()
        .register_capability(descriptor, Some(cap("cap_root")))
        .expect("the narrowed grant registers");
}

/// A grant whose intent list names only an intent the fixture's snapshot is not governed
/// by: the campaign's intent is derived from the snapshot, never named by the request.
fn other_intent(descriptor: &mut CapabilityDescriptor) {
    descriptor.intents =
        vec![IntentHandle::new("in_notthefixturesintent").expect("a well-formed intent handle")];
}

fn start_as(
    fixture: &mut Fixture,
    capability: &str,
    request: &str,
    key: &str,
    states: Option<u64>,
) -> OperationOutcome {
    let snapshot = fixture.snapshot.clone();
    fixture.daemon.dispatch(&OperationRequest {
        envelope: on(
            budgeted(
                keyed(
                    envelope("verification.start", "agent:runner", capability, request),
                    key,
                ),
                states,
            ),
            &snapshot,
        ),
        arguments: Arguments::VerificationStart(VerificationStartRequest {
            target: target(TargetKind::AllClaims, "DieHard"),
            portfolio: Portfolio::Interactive,
            context_policy: Optional::Absent,
            priority_class: Optional::Absent,
        }),
    })
}

fn resume_as(
    fixture: &mut Fixture,
    capability: &str,
    continuation: &ContinuationHandle,
    request: &str,
    key: &str,
) -> OperationOutcome {
    fixture.daemon.dispatch(&OperationRequest {
        envelope: budgeted(
            keyed(
                envelope("task.resume", "agent:runner", capability, request),
                key,
            ),
            Some(64),
        ),
        arguments: Arguments::TaskResume(TaskResumeRequest {
            continuation: continuation.clone(),
            budget: Optional::Present(budget(Some(64))),
        }),
    })
}

fn denial_detail(outcome: &OperationOutcome) -> String {
    outcome
        .envelope
        .error
        .value()
        .expect("an error result carries one")
        .detail
        .clone()
}

fn last_denial(fixture: &Fixture) -> (bool, Option<continuumd::daemon::state::Denial>) {
    let record = fixture
        .daemon
        .state()
        .admissions()
        .last()
        .expect("every call is recorded at admission");
    (record.admitted, record.denial)
}

/// The replay denial: admitted, then refused at the ledger, naming nothing.
fn assert_replay_denied(fixture: &Fixture, replayed: &OperationOutcome) {
    assert_eq!(code(replayed), ErrorCode::CapabilityDenied);
    assert_eq!(replayed.payload, Payload::None);
    assert_eq!(
        last_denial(fixture),
        (
            true,
            Some(continuumd::daemon::state::Denial::ReplayAuthority)
        ),
        "the grant passed admission and the replay check refused it"
    );
}

#[test]
fn a_verification_start_replay_under_a_grant_without_its_intent_is_denied() {
    // The campaign's intent is derived from the sealed snapshot. The narrow grant lists
    // another intent only, so a fresh start under it is refused on the derived intent; the
    // recorded answer (a task handle) names nothing the grant refuses, so before cr-3lrkq3
    // the replay returned it.
    let mut fixture = fixture();
    register_runner(&mut fixture, "cap_runner_narrow", other_intent);
    let first = start_as(
        &mut fixture,
        "cap_runner",
        "req_first",
        "idem-replayed",
        Some(64),
    );
    assert!(
        first.envelope.error.is_absent(),
        "{:?}",
        first.envelope.error
    );

    let replayed = start_as(
        &mut fixture,
        "cap_runner_narrow",
        "req_narrow",
        "idem-replayed",
        Some(64),
    );
    assert_replay_denied(&fixture, &replayed);

    let mut fresh_fixture = fixture_for_fresh();
    let fresh = start_as(
        &mut fresh_fixture,
        "cap_runner_narrow",
        "req_fresh",
        "idem-fresh",
        Some(64),
    );
    assert_eq!(code(&fresh), ErrorCode::CapabilityDenied);
    assert_eq!(
        last_denial(&fresh_fixture),
        (true, Some(continuumd::daemon::state::Denial::DerivedHandle))
    );
    assert_eq!(denial_detail(&replayed), denial_detail(&fresh));

    // Anti-vacuity: the full grant replays the recorded answer.
    let again = start_as(
        &mut fixture,
        "cap_runner",
        "req_again",
        "idem-replayed",
        Some(64),
    );
    assert_eq!(again.payload, first.payload);
    assert_eq!(last_denial(&fixture), (true, None));
}

#[test]
fn a_verification_start_replay_under_a_grant_without_the_minted_continuation_is_denied() {
    // A bounded campaign parks and mints a continuation, which the minted-handle check
    // decides through `call.admits`. The narrow grant holds every class except `cont`.
    // The recorded answer names that continuation, so the outcome-name check also refuses
    // this replay, and it did before cr-3lrkq3. This test holds the minted path to the same
    // single denial; the intent tests above are the ones that failed before the fix.
    let no_continuations = |descriptor: &mut CapabilityDescriptor| {
        descriptor.artifact_classes = continuum_workspace::artifact_path::ArtifactClass::ALL
            .into_iter()
            .filter(|class| {
                *class != continuum_workspace::artifact_path::ArtifactClass::Continuation
            })
            .map(|class| class.token().to_owned())
            .collect();
    };
    let mut fixture = fixture();
    register_runner(&mut fixture, "cap_runner_narrow", no_continuations);
    let first = start_as(
        &mut fixture,
        "cap_runner",
        "req_first",
        "idem-parked",
        Some(4),
    );
    assert!(
        first.envelope.error.is_absent(),
        "{:?}",
        first.envelope.error
    );

    let replayed = start_as(
        &mut fixture,
        "cap_runner_narrow",
        "req_narrow",
        "idem-parked",
        Some(4),
    );
    assert_replay_denied(&fixture, &replayed);

    let mut fresh_fixture = fixture_for_fresh();
    register_runner(&mut fresh_fixture, "cap_runner_narrow", no_continuations);
    let fresh = start_as(
        &mut fresh_fixture,
        "cap_runner_narrow",
        "req_fresh",
        "idem-fresh",
        Some(4),
    );
    assert_eq!(code(&fresh), ErrorCode::CapabilityDenied);
    assert_eq!(denial_detail(&replayed), denial_detail(&fresh));

    let again = start_as(
        &mut fixture,
        "cap_runner",
        "req_again",
        "idem-parked",
        Some(4),
    );
    assert_eq!(again.payload, first.payload);
    assert_eq!(last_denial(&fixture), (true, None));
}

#[test]
fn a_task_resume_replay_under_a_grant_without_its_intent_is_denied() {
    // `task.resume` names a continuation; the intent it runs under is derived from it.
    let mut fixture = fixture();
    register_runner(&mut fixture, "cap_runner_narrow", other_intent);
    let (_, continuation) = park(&mut fixture);
    let first = resume_as(
        &mut fixture,
        "cap_runner",
        &continuation,
        "req_first",
        "idem-resumed",
    );
    assert_eq!(
        first.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        first.envelope.error
    );

    let replayed = resume_as(
        &mut fixture,
        "cap_runner_narrow",
        &continuation,
        "req_narrow",
        "idem-resumed",
    );
    assert_replay_denied(&fixture, &replayed);

    let mut fresh_fixture = fixture_for_fresh();
    let (_, fresh_continuation) = park(&mut fresh_fixture);
    let fresh = resume_as(
        &mut fresh_fixture,
        "cap_runner_narrow",
        &fresh_continuation,
        "req_fresh",
        "idem-fresh",
    );
    assert_eq!(code(&fresh), ErrorCode::CapabilityDenied);
    assert_eq!(
        last_denial(&fresh_fixture),
        (true, Some(continuumd::daemon::state::Denial::DerivedHandle))
    );
    assert_eq!(denial_detail(&replayed), denial_detail(&fresh));

    let again = resume_as(
        &mut fixture,
        "cap_runner",
        &continuation,
        "req_again",
        "idem-resumed",
    );
    assert_eq!(again.payload, first.payload);
    assert_eq!(last_denial(&fixture), (true, None));
}

/// A second fixture for the fresh-call comparison, with the intent-narrowed grant
/// registered, so the fresh call does not meet the first fixture's recorded task.
fn fixture_for_fresh() -> Fixture {
    let mut fresh = fixture();
    register_runner(&mut fresh, "cap_runner_narrow", other_intent);
    fresh
}
