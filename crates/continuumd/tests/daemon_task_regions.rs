//! The daemon's task work, inside the regions PR 6 puts it in — through `Daemon::dispatch`
//! and nothing else.
//!
//! # What these tests are evidence for
//!
//! bn-2gk landed the region calculus in `continuum-task` with 61 tests of its own; this file
//! is the other half of PR 6's claim — that the *daemon's* work runs inside it — and it
//! asserts that at the grain a client can reach. Every scenario is a sequence of dispatches
//! against a real `Daemon`, and every claim is read either off the wire result or off
//! `Daemon::state().regions()`, which is the daemon's own region tree rather than a summary
//! of it.
//!
//! Five things are being established, and they are separable:
//!
//! - **cancellation through dispatch, at every phase a task can be in.** The region layer's
//!   cancellation table, mirrored at the daemon grain: what the wire answers, which arm of
//!   `CancelOutcome` the region computed, and the fact that those two are two readings of
//!   one thing. The committed-partial-evidence rows are the point — a parked task carries
//!   committed evidence *and* a valid continuation, a completed one carries committed
//!   evidence and nothing to resume, and neither can be spelled the other way round;
//! - **the suspension resolution.** `Suspended` cannot survive a region teardown, and the
//!   daemon parks continuations. The resolution these tests pin is that **a parked
//!   continuation is not a live worker**: the region a campaign parked in is *finalized*
//!   while the task is still resumable, and `task.resume` mints a new worker in a new
//!   region. `crates/continuumd/src/daemon/region.rs` states the RFC 0026 argument; this
//!   file shows the states;
//! - **no orphan work at the daemon grain.** After a mixed dispatch sequence — campaigns
//!   that closed, campaigns that parked, resumes, cancels at every phase, a run that
//!   faulted, and requests that were denied — every region this daemon opened is finalized,
//!   every worker it admitted is terminal, and its obligation ledger is balanced. Asserted
//!   against the tree, not against the reports the tree handed back;
//! - **determinism.** Two runs of one dispatch sequence render the region ledger
//!   byte-identically, and a valid resume produces a byte-identical task record on two fresh
//!   daemons — R3 spike §3's "resumable continuation", at this layer
//!   (`rule ordering.deterministic`);
//! - **refusals do no work.** A resume the decision table rejects, and a cancel of a task
//!   this daemon does not hold, open no region at all: denial precedes semantic work
//!   (RFC 0027 X3), and a scope that is never opened is the strongest form of that.
//!
//! # The harness
//!
//! The fixtures are `daemon_task_operations.rs`'s, duplicated locally rather than imported,
//! because a `tests/*.rs` file is its own crate and nothing here can `use` a sibling one —
//! the same reason `pr8_exit_evidence.rs` gives for duplicating the transport harness. The
//! Die Hard model and contract are `include_str!`'d from the one copy of each in this
//! repository, so no fixture here can drift from the corpus.

use continuum_engine_reference::diehard;
use continuum_intent::canonical_json::Json;
use continuum_intent::contract::IntentContract;
use continuum_task::region::RegionState;
use continuum_task::region::worker::{CancelOutcome, FailureReason, WorkerState};
use continuum_value::epoch::ProtocolWindow;
use continuumd::codec::operations::encode_payload;
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::region::TaskRegions;
use continuumd::daemon::state::{IntentRecord, RegistryStatus};
use continuumd::daemon::task::TaskFamily;
use continuumd::daemon::verification::{VerificationFamily, model_source};
use continuumd::daemon::workspace::WorkspaceFamily;
use continuumd::daemon::{Daemon, OperationOutcome, OperationRequest};
use continuumd::protocol::envelope::{
    Budget, EpochSet, RequestEnvelope, StructuralVerdictValue, Verdict,
};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, VersionRange, negotiate,
};
use continuumd::protocol::operations::intent::IntentAcceptRequest;
use continuumd::protocol::operations::task::{
    TaskCancelRequest, TaskResumeRequest, TaskStatusRequest, TaskUpdateBudgetRequest,
};
use continuumd::protocol::operations::verification::VerificationStartRequest;
use continuumd::protocol::operations::workspace::WorkspaceCreateRequest;
use continuumd::protocol::registry::ENCODINGS;
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, Commitment, ContinuationHandle, EpochIdentity, IntentHandle, Opaque,
    OperationName, ProtocolVersion, RequestId, TaskHandle, Timestamp, WorkspaceHandle,
};
use continuumd::protocol::shared::{SnapshotComponents, SnapshotEpochs, Target};
use continuumd::protocol::spec::{Nullable, Optional, ProtocolEnum};
use continuumd::protocol::vocabulary::{
    AuthorityLevel, Encoding, ErrorCode, Portfolio, ResultStatus, StructuralOutcome, TargetKind,
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

    Fixture { daemon, snapshot }
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
    // The idempotency key is derived from the request identity rather than fixed, so a test
    // may resume twice in one fixture: `rule idempotency.replay` would answer the second
    // call from the ledger otherwise, and a replayed answer is not the answer this file is
    // asking about.
    let mut request_envelope = budgeted(
        keyed(
            envelope("task.resume", "agent:runner", "cap_runner", request),
            &format!("idem-{request}"),
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
fn cancel(fixture: &mut Fixture, task: &TaskHandle, request: &str, key: &str) -> OperationOutcome {
    fixture.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope("task.cancel", "agent:runner", "cap_runner", request),
            key,
        ),
        arguments: Arguments::TaskCancel(TaskCancelRequest { task: task.clone() }),
    })
}
// --- reading the daemon's regions --------------------------------------------------------

/// The daemon's own region ledger.
fn regions(fixture: &Fixture) -> &TaskRegions {
    fixture.daemon.state().regions()
}

/// The task entry `task` names, for the internal facts no wire field carries.
fn entry<'a>(fixture: &'a Fixture, task: &TaskHandle) -> &'a continuumd::daemon::task::TaskEntry {
    fixture
        .daemon
        .state()
        .tasks()
        .get(task)
        .expect("the daemon holds the task")
}

/// The state the most recent scope's worker ended in.
///
/// Every scope this daemon opens owns exactly one worker — one unit of work — so "the
/// worker" is unambiguous.
fn settled_state(fixture: &Fixture) -> WorkerState {
    regions(fixture)
        .finalizations()
        .last()
        .and_then(|report| report.workers().first())
        .map(|worker| worker.state().clone())
        .expect("at least one scope has been torn down")
}

/// The typed outcome the most recent teardown left behind.
fn settled_outcome(fixture: &Fixture) -> Option<CancelOutcome> {
    regions(fixture)
        .finalizations()
        .last()
        .and_then(|report| report.workers().first())
        .and_then(|worker| worker.cancel_outcome().cloned())
}

/// The daemon-grain no-orphan property, asserted with the ledger as the failure message.
fn assert_total(fixture: &Fixture, when: &str) {
    let regions = regions(fixture);
    assert!(
        regions.is_total(),
        "{when}: the daemon's regions are not total\n{}",
        regions.render()
    );
}

fn structural(outcome: StructuralOutcome) -> Nullable<Verdict> {
    Nullable::Value(Verdict::Structural(StructuralVerdictValue { outcome }))
}

// --- one campaign, one region ------------------------------------------------------------

/// A campaign that closes runs inside a region, and the region is gone before the dispatch
/// that opened it returns.
#[test]
fn positive_a_closed_campaign_runs_in_a_region_finalized_before_the_dispatch_returns() {
    let mut fixture = fixture();
    assert_eq!(regions(&fixture).opened(), 0, "no work has run yet");

    let task = started_task(&start(
        &mut fixture,
        "req_start",
        "idem-start",
        Some(64),
        target(TargetKind::AllClaims, "DieHard"),
    ));

    assert_eq!(regions(&fixture).opened(), 1, "one unit of work, one scope");
    assert_eq!(
        regions(&fixture).finalizations().len(),
        1,
        "the scope is torn down inside the dispatch that opened it"
    );
    assert_eq!(
        settled_state(&fixture),
        WorkerState::Completed,
        "work that finished on its own is closed, not cancelled"
    );
    assert_eq!(
        settled_outcome(&fixture),
        None,
        "a completion is not a cancellation outcome, and reporting one would invent a \
         continuation question nobody asked"
    );

    let entry = entry(&fixture, &task);
    assert_eq!(entry.status, TaskStatus::Completed);
    assert_eq!(entry.publications(), 1, "one campaign committed");
    let region = entry.region.expect("the run recorded the scope it ran in");
    assert_eq!(
        regions(&fixture).tree().state(region),
        Ok(RegionState::Finalized)
    );
    assert_total(&fixture, "after one closed campaign");
}

/// **The suspension resolution.** A parked campaign settles its worker at the scope exit and
/// what survives is the wire continuation.
///
/// `RegionTree::finalize` requires every owned worker to be *terminal*, not merely quiescent,
/// so `Suspended` cannot survive a teardown. This test pins the resolution
/// `crates/continuumd/src/daemon/region.rs` argues from RFC 0026: the parked *execution* is
/// terminated by the scope exit — it reports `cancelled`, carrying the committed evidence and
/// a continuation — while the parked *artifact*, the `cont_*` the daemon minted, outlives the
/// scope in the task table. The region a task parked in is finalized while the task is still
/// resumable, and that is not a contradiction: they are two different objects.
#[test]
fn positive_a_parked_campaign_settles_its_worker_and_the_wire_continuation_survives() {
    let mut fixture = fixture();
    let (task, continuation) = park(&mut fixture);

    assert_eq!(
        settled_state(&fixture),
        WorkerState::Cancelled,
        "a parked worker is quiescent but not terminal, so the scope exit terminates it"
    );
    let outcome = settled_outcome(&fixture).expect("the scope exit cancelled the parked work");
    assert_eq!(outcome.token(), "committed-with-continuation");

    let entry = entry(&fixture, &task);
    assert_eq!(entry.status, TaskStatus::Suspended);
    assert_eq!(
        entry.continuation.as_ref(),
        Some(&continuation),
        "`TaskRecord.continuation` is REQUIRED when `status = suspended`"
    );
    assert_eq!(entry.publications(), 1);

    let region = entry.region.expect("the run recorded the scope it ran in");
    assert_eq!(
        regions(&fixture).tree().state(region),
        Ok(RegionState::Finalized),
        "the scope a campaign parked in is finalized; a parked continuation is not a live \
         worker"
    );

    // The two continuations are different artifacts, and the region layer's is bookkeeping:
    // it names the worker and how much it committed, and it is not a `cont_*` handle.
    let region_continuation = outcome
        .continuation()
        .expect("the committed-with-continuation arm carries one");
    assert_eq!(
        region_continuation.worker(),
        entry.worker.expect("the run recorded its worker")
    );
    assert_eq!(region_continuation.committed(), 1);
    assert!(
        continuation.as_str().starts_with("cont_"),
        "the wire artifact is the daemon's content-addressed handle"
    );
    assert_total(&fixture, "after a parked campaign");
}

/// `task.resume` spawns a **new** worker in a **new** region, and the region the task parked
/// in stays finalized.
///
/// The other half of the resolution above: a resume does not reopen a scope, it opens one.
/// The ordinals say so, and they are counted rather than drawn, so the comparison is a fact
/// about what happened and not about how it was scheduled (INV-005).
#[test]
fn positive_resume_spawns_a_new_worker_in_a_new_region() {
    let mut fixture = fixture();
    let (task, continuation) = park(&mut fixture);
    let parked_region = entry(&fixture, &task)
        .region
        .expect("the parked run recorded its scope");
    let parked_worker = entry(&fixture, &task)
        .worker
        .expect("the parked run recorded its worker");

    let resumed = resume(&mut fixture, &continuation, None, "req_resume");
    assert_eq!(resumed.envelope.status, ResultStatus::Ok);

    let entry = entry(&fixture, &task);
    let region = entry.region.expect("the resumed run recorded its scope");
    let worker = entry.worker.expect("the resumed run recorded its worker");
    assert!(
        region.ordinal() > parked_region.ordinal(),
        "a resumed run is a new scope, not the parked one reopened"
    );
    assert!(
        worker.ordinal() > parked_worker.ordinal(),
        "and a new worker: what was validated and re-admitted is the continuation, not a \
         parked thread"
    );
    assert_eq!(entry.status, TaskStatus::Completed);
    assert_eq!(
        entry.publications(),
        2,
        "resume MAY add evidence; it MUST NOT replace prior artifacts (INV-009)"
    );
    assert_eq!(regions(&fixture).opened(), 2);
    for region in [parked_region, region] {
        assert_eq!(
            regions(&fixture).tree().state(region),
            Ok(RegionState::Finalized)
        );
    }
    assert_total(&fixture, "after a resume");
}

// --- cancellation at every phase, through dispatch ---------------------------------------

/// One row of the cancellation table: what the wire said, and what the region computed.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Row {
    status: TaskStatus,
    verdict: Nullable<Verdict>,
    continuation: Nullable<ContinuationHandle>,
    arm: Option<&'static str>,
    worker: WorkerState,
}

/// Cancel `task` through `Daemon::dispatch` and read both halves of the answer.
fn cancel_row(fixture: &mut Fixture, task: &TaskHandle, request: &str, key: &str) -> Row {
    let before = regions(fixture).finalizations().len();
    let outcome = cancel(fixture, task, request, key);
    assert_eq!(
        regions(fixture).finalizations().len(),
        before + 1,
        "a cancel opens exactly one scope and tears it down"
    );
    let (status, continuation) = match &outcome.payload {
        Payload::TaskCancel(response) => (response.status, response.continuation.clone()),
        other => panic!("expected a task.cancel payload, got {other:?}"),
    };
    let report = regions(fixture)
        .finalizations()
        .last()
        .expect("the cancel's own teardown");
    let worker = report
        .workers()
        .first()
        .expect("the cancel scope owns one worker");
    Row {
        status,
        verdict: outcome.envelope.verdict.clone(),
        continuation,
        arm: worker.cancel_outcome().map(CancelOutcome::token),
        worker: worker.state().clone(),
    }
}

/// **The cancellation table at the daemon grain**, mirroring the region layer's own.
///
/// Every phase a task can be in when `task.cancel` reaches it, including both
/// committed-partial-evidence rows: a parked task carries committed evidence *and* a valid
/// continuation, and a task whose campaign closed carries committed evidence with nothing to
/// resume. `rule task.cancel_correct`'s "either committed partial evidence plus a valid
/// continuation, or nothing published" is the invariant every row is held to, and the last
/// assertion states it directly: the wire's nullable `continuation` is present exactly on the
/// arm the region layer put the answer on.
#[test]
fn positive_cancellation_through_dispatch_at_every_phase_a_task_can_be_in() {
    let mut rows: Vec<(&'static str, Row)> = Vec::new();

    // A task whose run faulted before it published anything: nothing to resume, nothing to
    // discard. `verification.start` answered the fault, and the entry it left behind is
    // reachable through the table it was put in.
    let mut created = fixture();
    let refused = start(
        &mut created,
        "req_start",
        "idem-start",
        Some(64),
        target(TargetKind::Property, "NoSuchPredicate"),
    );
    assert_eq!(code(&refused), ErrorCode::MalformedRequest);
    let stranded = created
        .daemon
        .state()
        .tasks()
        .handles()
        .first()
        .map(|handle| (*handle).clone())
        .expect("the failed run left its task in the table");
    rows.push((
        "created",
        cancel_row(&mut created, &stranded, "req_cancel", "idem-cancel"),
    ));
    assert_total(&created, "after cancelling a task that never published");

    // Parked, with committed partial evidence and a valid continuation.
    let mut parked = fixture();
    let (task, continuation) = park(&mut parked);
    rows.push((
        "suspended",
        cancel_row(&mut parked, &task, "req_cancel", "idem-cancel"),
    ));
    assert_eq!(
        rows.last().expect("just pushed").1.continuation,
        Nullable::Value(continuation),
        "the wire continuation is the `cont_*` the task holds, not a region-layer value"
    );
    // Cancelling twice: monotone, and the second answer says so.
    rows.push((
        "cancelled",
        cancel_row(&mut parked, &task, "req_cancel_2", "idem-cancel-2"),
    ));
    assert_total(&parked, "after cancelling a parked task twice");

    // Completed: committed evidence, and nothing left to resume.
    let mut completed = fixture();
    let done = started_task(&start(
        &mut completed,
        "req_start",
        "idem-start",
        Some(64),
        target(TargetKind::AllClaims, "DieHard"),
    ));
    rows.push((
        "completed",
        cancel_row(&mut completed, &done, "req_cancel", "idem-cancel"),
    ));
    assert_total(&completed, "after cancelling a completed task");

    // Parked and then resumed to completion: two publications, still nothing to resume.
    let mut resumed = fixture();
    let (task, continuation) = park(&mut resumed);
    assert_eq!(
        resume(&mut resumed, &continuation, None, "req_resume")
            .envelope
            .status,
        ResultStatus::Ok
    );
    rows.push((
        "completed-after-resume",
        cancel_row(&mut resumed, &task, "req_cancel", "idem-cancel"),
    ));
    assert_total(&resumed, "after cancelling a resumed task");

    // Failed non-resumably: the state budget could not hold the model's initial states, so
    // there is no prefix of the walk to report and no continuation to mint.
    let mut broken = fixture();
    let failed = started_task(&start(
        &mut broken,
        "req_start",
        "idem-start",
        Some(0),
        target(TargetKind::AllClaims, "DieHard"),
    ));
    rows.push((
        "failed",
        cancel_row(&mut broken, &failed, "req_cancel", "idem-cancel"),
    ));
    assert_total(&broken, "after cancelling a failed task");

    let expected: Vec<(
        &'static str,
        TaskStatus,
        StructuralOutcome,
        bool,
        &'static str,
    )> = vec![
        (
            "created",
            TaskStatus::Cancelled,
            StructuralOutcome::Cancelled,
            false,
            "nothing-published",
        ),
        (
            "suspended",
            TaskStatus::Cancelled,
            StructuralOutcome::Cancelled,
            true,
            "committed-with-continuation",
        ),
        (
            "cancelled",
            TaskStatus::Cancelled,
            StructuralOutcome::Unchanged,
            true,
            "committed-with-continuation",
        ),
        (
            "completed",
            TaskStatus::Completed,
            StructuralOutcome::Unchanged,
            false,
            "committed-non-resumable",
        ),
        (
            "completed-after-resume",
            TaskStatus::Completed,
            StructuralOutcome::Unchanged,
            false,
            "committed-non-resumable",
        ),
        (
            "failed",
            TaskStatus::Failed,
            StructuralOutcome::Unchanged,
            false,
            "nothing-published",
        ),
    ];
    assert_eq!(rows.len(), expected.len());
    for ((phase, row), (name, status, verdict, resumable, arm)) in rows.iter().zip(&expected) {
        assert_eq!(phase, name);
        assert_eq!(row.status, *status, "{phase}: task status");
        assert_eq!(row.verdict, structural(*verdict), "{phase}: verdict");
        assert_eq!(
            row.arm,
            Some(*arm),
            "{phase}: the region's cancellation arm"
        );
        assert_eq!(
            row.worker,
            WorkerState::Cancelled,
            "{phase}: the cancel's own scope is torn down by a cancellation"
        );
        assert_eq!(
            row.continuation != Nullable::Null,
            *resumable,
            "{phase}: the wire's nullable continuation"
        );
        // `rule task.cancel_correct`, as a property of every row rather than of the two rows
        // somebody remembered: the wire carries a continuation exactly when the region layer
        // put the answer on the arm that has one, and that arm has no constructor without
        // committed evidence.
        assert_eq!(
            row.continuation != Nullable::Null,
            row.arm == Some("committed-with-continuation"),
            "{phase}: the wire answer and the region's arm are two readings of one fact"
        );
    }

    // A task this daemon does not hold is a denial — and a denial does no work, so it opens
    // no scope at all (RFC 0027 X3: denial precedes semantic work).
    let opened = regions(&broken).opened();
    let unknown = TaskHandle::new("task_nothing").expect("a handle");
    let denied = cancel(&mut broken, &unknown, "req_cancel_3", "idem-cancel-3");
    assert_eq!(code(&denied), ErrorCode::CapabilityDenied);
    assert_eq!(
        regions(&broken).opened(),
        opened,
        "a denied cancel names no task, so there is no work to tear down"
    );
    assert_total(&broken, "after a denied cancel");
}

// --- resume, refused --------------------------------------------------------------------

/// Resuming after a cancel runs no work, and the refusal is the typed terminal status.
///
/// > Resume never silently re-runs. […] the daemon MUST NOT quietly restart the task under
/// > current epochs.
/// >
/// > — RFC 0026, "Resume decision table"
///
/// `task.resume`'s `errors` clause has no code for "already terminal" and
/// `rule errors.common`'s six name nothing that would be true of it, so the typed answer is
/// the status itself — `cancelled`, monotone, unchanged by the request. What this test adds
/// to the existing status assertion is the *absence of work*: no scope is opened, nothing is
/// published, and the region ledger is untouched. A resume that silently re-ran would be
/// visible here as a second scope even if it produced the same numbers.
#[test]
fn negative_resume_after_a_cancel_runs_no_work_and_reports_the_terminal_status() {
    let mut fixture = fixture();
    let (task, continuation) = park(&mut fixture);
    let cancelled = cancel(&mut fixture, &task, "req_cancel", "idem-cancel");
    assert_eq!(cancelled.envelope.status, ResultStatus::Ok);

    let opened = regions(&fixture).opened();
    let published = entry(&fixture, &task).publications();
    let refused = resume(&mut fixture, &continuation, None, "req_resume");
    assert_eq!(refused.envelope.status, ResultStatus::Ok);
    match &refused.payload {
        Payload::TaskResume(response) => assert_eq!(
            response.status,
            TaskStatus::Cancelled,
            "a terminal status never changes"
        ),
        other => panic!("expected a task.resume payload, got {other:?}"),
    }
    assert_eq!(
        regions(&fixture).opened(),
        opened,
        "a refused resume opens no scope, because it runs nothing"
    );
    assert_eq!(
        entry(&fixture, &task).publications(),
        published,
        "and publishes nothing"
    );
    assert_total(&fixture, "after a refused resume");
}

/// A resume the decision table rejects opens no region either: validation precedes work.
///
/// Both rows are `Daemon::dispatch` answers rather than direct calls to the predicate —
/// `StaleSnapshot` for a continuation whose pinned snapshot is not the one the envelope
/// names, and `CapabilityDenied` for a continuation this daemon does not hold (RFC 0027 X2:
/// no distinguishable not-found).
#[test]
fn negative_a_resume_the_decision_table_rejects_opens_no_region() {
    let mut fixture = fixture();
    let (_, continuation) = park(&mut fixture);
    let opened = regions(&fixture).opened();

    let elsewhere = WorkspaceHandle::new("ws_elsewhere").expect("a handle");
    let stale = resume(&mut fixture, &continuation, Some(&elsewhere), "req_resume");
    assert_eq!(code(&stale), ErrorCode::StaleSnapshot);

    let unheld = ContinuationHandle::new("cont_nothing").expect("a handle");
    let denied = resume(&mut fixture, &unheld, None, "req_resume_2");
    assert_eq!(code(&denied), ErrorCode::CapabilityDenied);

    assert_eq!(
        regions(&fixture).opened(),
        opened,
        "a refused resume never reaches the run, so no scope is opened for it"
    );
    assert_total(&fixture, "after two refused resumes");
}

/// A run that faults still finalizes its region, and the worker it left is `failed`.
///
/// The error path is where an unstructured implementation leaks: the operation returns a
/// typed fault and the execution it started has nowhere to be accounted for. Here the scope
/// is torn down by the same bracket that opened it, on the fault path as on the value path.
#[test]
fn negative_a_run_that_faults_still_finalizes_its_region() {
    let mut fixture = fixture();
    let refused = start(
        &mut fixture,
        "req_start",
        "idem-start",
        Some(64),
        target(TargetKind::Property, "NoSuchPredicate"),
    );
    assert_eq!(code(&refused), ErrorCode::MalformedRequest);
    assert_eq!(refused.payload, Payload::None, "no partial effect");

    assert_eq!(regions(&fixture).opened(), 1);
    assert_eq!(regions(&fixture).finalizations().len(), 1);
    let settled = settled_state(&fixture);
    assert_eq!(settled.status_token(), "failed");
    assert_eq!(
        settled.failure_reason().map(FailureReason::as_str),
        Some(ErrorCode::MalformedRequest.as_wire()),
        "the region layer carries the daemon's own code rather than a second vocabulary"
    );
    assert_total(&fixture, "after a run that faulted");
}

// --- the no-orphan property, instrumented ------------------------------------------------

/// Every dispatch a mixed sequence can make, and the tasks it leaves behind.
fn mixed_sequence(fixture: &mut Fixture) -> Vec<TaskHandle> {
    let mut tasks = Vec::new();

    // A campaign that parks, read, re-budgeted, and resumed to completion.
    let (parked, continuation) = park(fixture);
    let _ = status(fixture, &parked, "req_status_2");
    let _ = fixture.daemon.dispatch(&OperationRequest {
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
            task: parked.clone(),
            budget: budget(Some(64)),
        }),
    });
    let _ = resume(fixture, &continuation, None, "req_resume");
    tasks.push(parked);

    // A campaign that closes, and is then cancelled.
    let closed = started_task(&start(
        fixture,
        "req_start_2",
        "idem-start-2",
        Some(64),
        target(TargetKind::Module, "DieHard"),
    ));
    let _ = cancel(fixture, &closed, "req_cancel", "idem-cancel");
    tasks.push(closed);

    // A campaign whose budget cannot hold the initial states, cancelled afterwards.
    let failed = started_task(&start(
        fixture,
        "req_start_3",
        "idem-start-3",
        Some(0),
        target(TargetKind::AllClaims, "DieHard"),
    ));
    let _ = cancel(fixture, &failed, "req_cancel_2", "idem-cancel-2");
    tasks.push(failed);

    // A run that faults, leaving a task that never published, cancelled afterwards.
    let _ = start(
        fixture,
        "req_start_4",
        "idem-start-4",
        Some(64),
        target(TargetKind::Property, "NoSuchPredicate"),
    );
    let stranded: Vec<TaskHandle> = fixture
        .daemon
        .state()
        .tasks()
        .handles()
        .into_iter()
        .filter(|handle| !tasks.contains(handle))
        .cloned()
        .collect();
    for handle in &stranded {
        let _ = cancel(fixture, handle, "req_cancel_3", "idem-cancel-3");
    }
    tasks.extend(stranded);

    // A campaign that parks and is *left* parked — never resumed, never cancelled. This is
    // the row the rejected design could not have torn down: if a parked continuation were a
    // live worker, this task would hold an open region for the daemon's lifetime and the
    // sweep below would fail.
    let abandoned = started_task(&start(
        fixture,
        "req_start_5",
        "idem-start-5",
        Some(8),
        target(TargetKind::AllClaims, "DieHard"),
    ));
    tasks.push(abandoned);

    // Two requests that answer nothing: a task this daemon does not hold, and a resume of a
    // continuation it does not hold. Neither may open a scope.
    let unknown = TaskHandle::new("task_nothing").expect("a handle");
    let _ = cancel(fixture, &unknown, "req_cancel_4", "idem-cancel-4");
    let _ = resume(
        fixture,
        &ContinuationHandle::new("cont_nothing").expect("a handle"),
        None,
        "req_resume_2",
    );

    tasks
}

/// **The no-orphan proof at the daemon grain.** After a mixed dispatch sequence, no worker
/// survives its task's terminal state and no region survives its dispatch.
///
/// > Cancellation cannot leave false finality or orphan work.
/// >
/// > — `notes/plan/plan.md` §3
///
/// The assertions are made against the *tree* — every worker it ever admitted, every region
/// it ever opened, and its obligation ledger — rather than against the reports the teardowns
/// handed back, because a check that only re-reads a summary is checking the summary. The
/// counts are asserted non-zero for the same reason: a vacuous sweep proves nothing.
#[test]
fn positive_no_worker_survives_its_tasks_terminal_state_across_a_mixed_dispatch_sequence() {
    let mut fixture = fixture();
    let tasks = mixed_sequence(&mut fixture);
    let regions = regions(&fixture);
    let tree = regions.tree();

    assert!(
        regions.opened() >= 8,
        "a sweep over fewer than eight units of work is not a sweep: {}",
        regions.render()
    );
    assert_eq!(
        tree.worker_count(),
        regions.opened() as usize,
        "one unit of work, one worker"
    );
    assert_eq!(
        regions.finalizations().len(),
        regions.opened() as usize,
        "every scope opened was torn down"
    );

    // 1. every worker the tree ever admitted is terminal.
    assert!(
        regions.orphans().is_empty(),
        "orphaned workers: {:?}\n{}",
        regions.orphans(),
        regions.render()
    );
    // 2. every region below the root is finalized; the root is the daemon's own scope.
    assert!(
        regions.unfinalized().is_empty(),
        "regions left open: {:?}",
        regions.unfinalized()
    );
    assert_eq!(tree.state(regions.root()), Ok(RegionState::Open));
    // 3. the obligation ledger is balanced, and it was not vacuously so.
    assert!(tree.ledger().is_balanced());
    assert!(tree.ledger().opened() > 0);
    assert_eq!(tree.ledger().opened(), tree.ledger().discharged());
    // 4. every teardown was itself total, and the wiring was never refused a step.
    assert!(regions.finalizations().iter().all(|f| f.is_total()));
    assert!(
        regions.defects().is_empty(),
        "the region layer refused one of this daemon's own steps: {:?}",
        regions.defects()
    );

    // 5. the property at the task grain: no task — terminal or parked — holds a worker that
    //    is still running. The parked case is the load-bearing one: a suspended task's worker
    //    is terminal *and* the task is still resumable, which is the resolution this bone
    //    implements.
    assert!(!tasks.is_empty());
    let mut suspended = 0_u32;
    for task in &tasks {
        let entry = entry(&fixture, task);
        let worker = entry
            .worker
            .expect("every task that ran recorded its worker");
        let state = tree.worker_state(worker).expect("the tree holds it");
        assert!(
            state.is_terminal(),
            "{}: worker {worker} is {state}, which is not terminal",
            task.as_str()
        );
        if entry.status == TaskStatus::Suspended {
            suspended += 1;
            assert!(
                entry.continuation.is_some(),
                "a suspended task is resumable by definition"
            );
            assert_eq!(
                tree.state(entry.region.expect("it ran somewhere")),
                Ok(RegionState::Finalized),
                "the scope a still-parked task parked in is finalized, and the task is \
                 still resumable: a parked continuation is not a live worker"
            );
        }
    }
    assert!(
        suspended > 0,
        "a sweep with no parked task never tests the case the resolution is about"
    );
    assert_total(&fixture, "after a mixed dispatch sequence");
}

/// Two runs of one dispatch sequence render the region ledger byte-identically.
///
/// > every stateful workflow uses explicit handles […] identical requests against equal
/// > states produce equal results
/// >
/// > — `rule ordering.deterministic`, as `daemon::state` states it
///
/// Bytes rather than `==` on a collection, because bytes catch an ordering difference that
/// set equality hides — the same device the region layer's own determinism test uses, lifted
/// to the daemon grain.
#[test]
fn positive_the_region_ledger_of_one_dispatch_sequence_is_byte_identical_across_two_runs() {
    let render = || {
        let mut fixture = fixture();
        let _ = mixed_sequence(&mut fixture);
        regions(&fixture).render()
    };
    let first = render();
    let second = render();
    assert_eq!(first.as_bytes(), second.as_bytes());
    assert!(
        first.lines().count() > 10,
        "a vacuous render proves nothing:\n{first}"
    );
    assert!(
        first.contains("committed-with-continuation"),
        "the sequence must reach the arm the whole table is about:\n{first}"
    );
}

/// **A valid resume is deterministic** — R3 spike §3's "resumable continuation", at the
/// daemon API and down to the bytes.
///
/// > The in-memory workbench demonstrated: canonical workspace snapshot identities;
/// > idempotent identical task creation; rejection of idempotency-key reuse with different
/// > request; **resumable continuation**; rejection of continuation under a different
/// > workspace snapshot.
/// >
/// > — `notes/plan/spikes/R3_SPIKE_REPORT.md` §3
///
/// Two daemons built the same way, parked the same campaign, and resumed the same
/// continuation. Four things are compared, weakest to strongest: the continuation identity
/// (content-addressed, so it agrees before any ledger is consulted), the whole typed resume
/// result, the canonical **bytes** of the task record the resume produced, and the region
/// ledger the two runs left behind. The third is the one that would catch an ordering
/// difference the second hides, which is why it goes through the codec rather than through
/// `==` on a struct.
#[test]
fn positive_a_valid_resume_is_deterministic_across_two_fresh_daemons() {
    let run = || {
        let mut fixture = fixture();
        let (task, continuation) = park(&mut fixture);
        let resumed = resume(&mut fixture, &continuation, None, "req_resume");
        let record = status(&mut fixture, &task, "req_status_2");
        let bytes = encode_payload(&record.payload)
            .expect("a task record encodes")
            .expect("a task.status answer carries a payload")
            .as_bytes()
            .to_vec();
        (
            continuation,
            resumed,
            bytes,
            regions(&fixture).render(),
            entry(&fixture, &task).publications(),
        )
    };
    let first = run();
    let second = run();

    assert_eq!(
        first.0, second.0,
        "a continuation is the content identity of what it pins"
    );
    assert_eq!(
        first.1, second.1,
        "the resume result is a pure function of the request and the state"
    );
    assert_eq!(
        first.2.as_slice(),
        second.2.as_slice(),
        "the resumed task's canonical record is byte-identical"
    );
    assert_eq!(first.3.as_bytes(), second.3.as_bytes());
    assert_eq!(first.4, 2, "the resume published, and replaced nothing");
    assert!(!first.2.is_empty(), "a vacuous comparison proves nothing");
}
