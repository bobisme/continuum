//! PR 6 / IMPL-02 evidence: `continuumd` on the budget ledger — the omission manifest
//! derived, `task.update_budget`'s five arms live on the wire, and committed partial
//! evidence carrying artifact identity.
//!
//! # What this file is evidence for
//!
//! bn-1gc landed the budget calculus in `continuum-task` with the daemon seam designed and
//! deliberately not wired; bn-23j7s is that wiring. Every claim below is read either off a
//! wire result or off `Daemon::state()`, through `Daemon::dispatch` and nothing else.
//!
//! | Clause | Test |
//! |---|---|
//! | the INV-007 manifest is *derived* from `MeterSet::STATES_ONLY`, never a hand-kept array | `the_omission_manifest_is_derived_from_the_meter_set` |
//! | `TaskRecord.budget` and `Cost` are projections of one ledger, not fields beside it | `the_record_projects_one_ledger_rather_than_two_fields` |
//! | a resumed run charges the difference; spend is the walk's high-water mark | `a_resumed_run_charges_the_difference_rather_than_the_sum` |
//! | `task.update_budget` raised / unchanged / tightened | `the_first_three_arms_of_the_legality_table_are_live_on_the_wire` |
//! | `task.update_budget` **suspend** — B18, parks with committed partial evidence plus a continuation, never truncates | `lowering_below_committed_spend_parks_with_evidence_and_a_continuation` |
//! | `task.update_budget` **unenforced** — a ceiling nothing meters is recorded and named | `a_ceiling_nothing_meters_is_recorded_and_named_on_the_answer` |
//! | committed partial evidence is **named**, not counted, and the names are monotone | `committed_partial_evidence_is_named_and_the_names_only_grow` |
//! | uncommitted partials are absent, never half-visible | `an_uncommitted_partial_is_absent_never_half_visible` |
//! | two daemons name one campaign's publications identically | `two_daemons_name_one_campaigns_publications_identically` |
//! | SD-13: `BudgetExhausted` carries a continuation or a typed `non_resumable_reason` | `budget_exhausted_on_the_wire_is_never_a_silent_dead_end` |
//!
//! # What is claimed about restart, and what is not
//!
//! IMPL-02's own acceptance criterion is "committed partial evidence survives daemon
//! restart; uncommitted partials are absent, never half-visible". This file claims the
//! second conjunct outright and the *identity* half of the first: a publication is named by
//! the content identity of the campaign that produced it, so two daemons that ran the same
//! campaign name it identically (`two_daemons_name_one_campaigns_publications_identically`)
//! and a name is therefore something a store could be asked to return.
//!
//! It does **not** claim durability. Nothing in this workspace writes a task table to disk,
//! and the persistence machinery is bn-3dr's (daemon crash recovery) rather than this
//! bone's; what bn-23j7s owes and delivers is the identity binding and the wire surface it
//! reaches a caller through. A restart test that stood up a second daemon and re-ran the
//! campaign would be testing the identity function, which is what the test above tests
//! honestly and by name.
//!
//! # The harness
//!
//! The fixtures are `daemon_task_regions.rs`'s, duplicated locally rather than imported,
//! because a `tests/*.rs` file is its own crate and nothing here can `use` a sibling one —
//! the same reason `pr8_exit_evidence.rs` gives for duplicating the transport harness. The
//! Die Hard model and contract are `include_str!`'d from the one copy of each in this
//! repository, so no fixture here can drift from the corpus.

use continuum_engine_reference::diehard;
use continuum_intent::canonical_json::Json;
use continuum_intent::contract::IntentContract;
use continuum_task::budget::dimension::{CostDimension, MeterSet};
use continuum_value::epoch::ProtocolWindow;
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::state::{IntentRecord, RegistryStatus};
use continuumd::daemon::task::TaskFamily;
use continuumd::daemon::verification::{VerificationFamily, model_source};
use continuumd::daemon::workspace::WorkspaceFamily;
use continuumd::daemon::{Daemon, OperationOutcome, OperationRequest};
use continuumd::protocol::envelope::{Budget, EpochSet, RequestEnvelope};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, VersionRange, negotiate,
};
use continuumd::protocol::operations::intent::IntentAcceptRequest;
use continuumd::protocol::operations::task::{
    TaskResumeRequest, TaskStatusRequest, TaskUpdateBudgetRequest,
};
use continuumd::protocol::operations::verification::{
    VerificationResultRequest, VerificationStartRequest,
};
use continuumd::protocol::operations::workspace::WorkspaceCreateRequest;
use continuumd::protocol::registry::ENCODINGS;
use continuumd::protocol::scalar::{
    ActorId, ByteCount, CapabilityHandle, Commitment, ContinuationHandle, DurationMs,
    EpochIdentity, IntentHandle, Opaque, OperationName, ProtocolVersion, RequestId, TaskHandle,
    Timestamp, WorkspaceHandle,
};
use continuumd::protocol::shared::{SnapshotComponents, SnapshotEpochs, Target};
use continuumd::protocol::spec::{Nullable, Optional};
use continuumd::protocol::vocabulary::{
    AuthorityLevel, Encoding, ErrorCode, OmissionReason, Portfolio, ResultStatus, TargetKind,
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

// --- driving the operations --------------------------------------------------------------

/// `verification.start` over the fixture's sealed snapshot, under `budget`.
fn start_with(
    fixture: &mut Fixture,
    request: &str,
    key: &str,
    declared: Budget,
    target: Target,
) -> OperationOutcome {
    let snapshot = fixture.snapshot.clone();
    let mut request_envelope = keyed(
        envelope("verification.start", "agent:runner", "cap_runner", request),
        key,
    );
    request_envelope.budget = Optional::Present(declared);
    fixture.daemon.dispatch(&OperationRequest {
        envelope: on(request_envelope, &snapshot),
        arguments: Arguments::VerificationStart(VerificationStartRequest {
            target,
            portfolio: Portfolio::Interactive,
            context_policy: Optional::Absent,
            priority_class: Optional::Absent,
        }),
    })
}

/// `verification.start` under a plain `states` ceiling.
fn start(
    fixture: &mut Fixture,
    request: &str,
    key: &str,
    states: Option<u64>,
    target: Target,
) -> OperationOutcome {
    start_with(fixture, request, key, budget(states), target)
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

/// `task.update_budget` under `declared`.
fn update_budget(
    fixture: &mut Fixture,
    task: &TaskHandle,
    request: &str,
    declared: Budget,
) -> OperationOutcome {
    fixture.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope("task.update_budget", "agent:runner", "cap_runner", request),
            &format!("idem-{request}"),
        ),
        arguments: Arguments::TaskUpdateBudget(TaskUpdateBudgetRequest {
            task: task.clone(),
            budget: declared,
        }),
    })
}

fn update_response(
    outcome: &OperationOutcome,
) -> continuumd::protocol::operations::task::TaskUpdateBudgetResponse {
    match &outcome.payload {
        Payload::TaskUpdateBudget(response) => response.clone(),
        other => panic!("expected a task.update_budget payload, got {other:?}"),
    }
}

/// `task.resume`, carrying `declared` when the caller supplies one.
fn resume(
    fixture: &mut Fixture,
    continuation: &ContinuationHandle,
    request: &str,
    declared: Optional<Budget>,
) -> OperationOutcome {
    let mut request_envelope = keyed(
        envelope("task.resume", "agent:runner", "cap_runner", request),
        &format!("idem-{request}"),
    );
    // `task.resume` is `@task_starting`, so the *envelope* requires a budget whatever the
    // request body says. The body's budget is the one that re-declares the task's ceilings;
    // this one bounds the call, declares nothing, and owes no omission.
    request_envelope.budget = Optional::Present(declared.value().cloned().unwrap_or(budget(None)));
    fixture.daemon.dispatch(&OperationRequest {
        envelope: request_envelope,
        arguments: Arguments::TaskResume(TaskResumeRequest {
            continuation: continuation.clone(),
            budget: declared,
        }),
    })
}

/// The task entry `task` names, for the facts no wire field carries.
fn entry<'a>(fixture: &'a Fixture, task: &TaskHandle) -> &'a continuumd::daemon::task::TaskEntry {
    fixture
        .daemon
        .state()
        .tasks()
        .get(task)
        .expect("the daemon holds the task")
}

/// The omission subjects an answer carries, in the order it carries them.
fn subjects(outcome: &OperationOutcome) -> Vec<String> {
    outcome
        .envelope
        .omissions
        .iter()
        .map(|omission| omission.subject.clone())
        .collect()
}

/// The commitments an answer names, in the order it names them.
fn commitments(outcome: &OperationOutcome) -> Vec<String> {
    outcome
        .envelope
        .artifacts
        .iter()
        .filter_map(|artifact| artifact.commitment.value().map(|c| c.as_str().to_owned()))
        .collect()
}

/// A budget declaring every one of the nine SD-12 dimensions.
fn every_dimension(states: u64) -> Budget {
    Budget {
        wall_ms: Optional::Present(DurationMs::new(5_000)),
        cpu_ms: Optional::Present(DurationMs::new(5_000)),
        memory_bytes: Optional::Present(ByteCount::new(1 << 20)),
        states: Optional::Present(states),
        solver_ms: Optional::Present(DurationMs::new(1)),
        proof_ms: Optional::Present(DurationMs::new(1)),
        tokens: Optional::Present(1_000),
        candidates: Optional::Present(8),
        bytes: Optional::Present(ByteCount::new(4_096)),
    }
}

/// Die Hard's frozen reachable-state count (TV-009).
const FROZEN_STATES: u64 = 16;

/// The state count a bound of four stops the walk at, deterministically.
const PARKED_STATES: u64 = 3;

// --- the tests ---------------------------------------------------------------------------

/// The eight declared ceilings this daemon cannot meter are *derived* from
/// `MeterSet::STATES_ONLY`, in SD-12 declaration order, and `states` — the one it can — is
/// not among them.
///
/// bn-18z wrote the eight by hand in `verification::unenforced`; bn-23j7s deleted the array
/// and asks the ledger. The subjects, the reason token and the order are what that array
/// emitted, which is why this test can assert them literally: the change is in where the
/// list comes from, not in what it says.
#[test]
fn the_omission_manifest_is_derived_from_the_meter_set() {
    // Built before the local binding shadows the constructor: the second daemon is the
    // control, and it declares no ceiling this daemon cannot enforce.
    let mut bare_fixture = fixture();
    let mut fixture = fixture();
    let started = start_with(
        &mut fixture,
        "req_start",
        "idem-start",
        every_dimension(FROZEN_STATES * 4),
        target(TargetKind::AllClaims, "DieHard"),
    );
    let named = subjects(&started);
    for expected in [
        "budget.wall_ms",
        "budget.cpu_ms",
        "budget.memory_bytes",
        "budget.solver_ms",
        "budget.proof_ms",
        "budget.tokens",
        "budget.candidates",
        "budget.bytes",
    ] {
        assert!(
            named.contains(&expected.to_owned()),
            "{expected} was declared and nothing meters it: {named:?}"
        );
    }
    assert!(
        !named.contains(&"budget.states".to_owned()),
        "`states` is the one dimension with an enforcement path"
    );
    assert_eq!(
        named
            .iter()
            .filter(|subject| subject.starts_with("budget."))
            .count(),
        MeterSet::STATES_ONLY.unmetered().len(),
        "one omission per unmetered dimension the caller declared — a count derived from \
         the meter set rather than from a literal eight"
    );
    // Every budget omission is `unsupported`, not `budget`: reason `budget` says a ceiling
    // caused something to be left out, and this says the daemon has no meter at all.
    for omission in &started.envelope.omissions {
        if omission.subject.starts_with("budget.") {
            assert_eq!(omission.reason, OmissionReason::Unsupported);
        }
    }

    // A budget declaring nothing owes nothing: the manifest is a function of what the
    // caller declared and what this daemon meters, not a fixed list.
    let plain = start(
        &mut bare_fixture,
        "req_start",
        "idem-start",
        Some(FROZEN_STATES * 4),
        target(TargetKind::AllClaims, "DieHard"),
    );
    assert!(
        !subjects(&plain)
            .iter()
            .any(|subject| subject.starts_with("budget.")),
        "no ceiling was declared that this daemon cannot enforce"
    );
}

/// `TaskRecord.budget` and `TaskRecord.cost` are two projections of one `BudgetLedger`.
///
/// The budget the caller sent is the budget the record reports, dimension for dimension,
/// and the cost the record reports is the ledger's own recorded spend — not a second read
/// of `campaign.states()`.
#[test]
fn the_record_projects_one_ledger_rather_than_two_fields() {
    let mut fixture = fixture();
    let declared = every_dimension(FROZEN_STATES * 4);
    let started = start_with(
        &mut fixture,
        "req_start",
        "idem-start",
        declared.clone(),
        target(TargetKind::AllClaims, "DieHard"),
    );
    let task = started_task(&started);
    let record = record(&status(&mut fixture, &task, "req_status"));
    assert_eq!(
        record.budget, declared,
        "the ceilings round-trip through the ledger unchanged"
    );
    assert_eq!(record.cost.states, Optional::Present(FROZEN_STATES));
    assert_eq!(
        entry(&fixture, &task)
            .ledger
            .spend()
            .measured(CostDimension::States),
        Some(FROZEN_STATES),
        "the wire number and the ledger's spend are one value"
    );
    assert_eq!(
        entry(&fixture, &task).bounds().states(),
        usize::try_from(FROZEN_STATES * 4).expect("fits"),
        "`bounds` is the ledger's `states` ceiling, projected"
    );
    // Every dimension nothing measures is *absent*, never zero — including the eight the
    // caller declared a ceiling for.
    assert!(record.cost.wall_ms.is_absent());
    assert!(record.cost.bytes.is_absent());
    assert!(record.cost.tokens.is_absent());
    assert!(record.cost.tokenizer_id.is_absent());
}

/// A resumed walk re-explores the parked prefix, so the ledger charges the difference.
///
/// A Die Hard campaign that parks at three states and closes at sixteen cost sixteen, not
/// nineteen. Spend is monotone and equal to the walk's high-water mark at every point a
/// caller can read it.
#[test]
fn a_resumed_run_charges_the_difference_rather_than_the_sum() {
    let mut fixture = fixture();
    let task = started_task(&start(
        &mut fixture,
        "req_start",
        "idem-start",
        Some(4),
        target(TargetKind::AllClaims, "DieHard"),
    ));
    let parked = record(&status(&mut fixture, &task, "req_status_1"));
    assert_eq!(parked.status, TaskStatus::Suspended);
    assert_eq!(parked.cost.states, Optional::Present(PARKED_STATES));
    let continuation = parked
        .continuation
        .value()
        .cloned()
        .expect("a suspended task is resumable by definition");

    let resumed = resume(
        &mut fixture,
        &continuation,
        "req_resume",
        Optional::Present(budget(Some(FROZEN_STATES * 4))),
    );
    assert_eq!(
        resumed.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        resumed.envelope.error
    );
    let closed = record(&status(&mut fixture, &task, "req_status_2"));
    assert_eq!(closed.status, TaskStatus::Completed);
    assert_eq!(
        closed.cost.states,
        Optional::Present(FROZEN_STATES),
        "the resumed walk covered the parked prefix again; it did not cost 3 + 16"
    );
}

/// `raised`, `unchanged` and `tightened` — the three arms that leave the run where it was.
///
/// Each is read off the consequence a caller can see: the recorded ceiling, and the engine
/// bound the next run takes.
#[test]
fn the_first_three_arms_of_the_legality_table_are_live_on_the_wire() {
    let mut fixture = fixture();
    let task = started_task(&start(
        &mut fixture,
        "req_start",
        "idem-start",
        Some(4),
        target(TargetKind::AllClaims, "DieHard"),
    ));
    assert_eq!(entry(&fixture, &task).bounds().states(), 4);

    // raised — "raising a dimension extends the current run", which at this grain means the
    // next `task.resume` runs under the larger bound.
    let raised = update_budget(&mut fixture, &task, "req_raise", budget(Some(64)));
    assert_eq!(
        update_response(&raised).budget.states,
        Optional::Present(64)
    );
    assert_eq!(entry(&fixture, &task).bounds().states(), 64);

    // unchanged — a re-sent update changes nothing (INV-002).
    let again = update_budget(&mut fixture, &task, "req_again", budget(Some(64)));
    assert_eq!(
        update_response(&again).budget,
        update_response(&raised).budget
    );
    assert_eq!(entry(&fixture, &task).bounds().states(), 64);
    assert_eq!(
        record(&status(&mut fixture, &task, "req_status_1"))
            .cost
            .states,
        Optional::Present(PARKED_STATES),
        "a budget update spends nothing"
    );

    // tightened — the ceiling fell and still admits the spend already recorded, so it binds
    // from now on and nothing already spent is invalidated.
    let tightened = update_budget(&mut fixture, &task, "req_tighten", budget(Some(8)));
    assert_eq!(
        update_response(&tightened).budget.states,
        Optional::Present(8)
    );
    assert_eq!(entry(&fixture, &task).bounds().states(), 8);
    assert!(
        entry(&fixture, &task)
            .ledger
            .spend()
            .measured(CostDimension::States)
            .is_some_and(|spent| spent <= 8),
        "a tightening is only a tightening while the ceiling still admits the spend"
    );
    let after = record(&status(&mut fixture, &task, "req_status_2"));
    let milestones: Vec<&str> = after
        .milestones
        .iter()
        .map(|milestone| milestone.name.as_str())
        .collect();
    assert!(
        !milestones.contains(&"task.budget_suspended"),
        "none of these three arms parks the task: {milestones:?}"
    );
}

/// **B18.** Lowering a ceiling below committed spend parks the task with committed partial
/// evidence plus a continuation, and never truncates the campaign.
///
/// This is the arm bn-1gc recorded as uncomputable from the handler, and the bug it was
/// hiding is concrete: before the ledger, the lowered ceiling became the engine bound
/// directly, so the next `task.resume` came back with a *smaller* campaign and a monotone
/// `cost.states` went **down** — the silent truncation INV-009 prohibits. The assertions
/// below are the fix stated as behaviour: the ceiling is recorded, the evidence and the
/// continuation survive, the bound does not fall under recorded spend, and the resumed run
/// is not smaller than the parked one.
#[test]
fn lowering_below_committed_spend_parks_with_evidence_and_a_continuation() {
    let mut fixture = fixture();
    let task = started_task(&start(
        &mut fixture,
        "req_start",
        "idem-start",
        Some(4),
        target(TargetKind::AllClaims, "DieHard"),
    ));
    let parked = record(&status(&mut fixture, &task, "req_status_1"));
    assert_eq!(parked.cost.states, Optional::Present(PARKED_STATES));
    let published = commitments(&status(&mut fixture, &task, "req_status_1b"));
    assert_eq!(published.len(), 1, "one campaign committed");

    // Below the three states already recorded.
    let lowered = update_budget(&mut fixture, &task, "req_lower", budget(Some(2)));
    let response = update_response(&lowered);
    assert_eq!(
        response.status,
        TaskStatus::Suspended,
        "B18 parks; it does not fail and it does not complete"
    );
    assert_eq!(
        response.budget.states,
        Optional::Present(2),
        "the ceiling is recorded: `TaskRecord.budget` reports the budget in force"
    );
    assert!(
        response.continuation.value().is_some(),
        "`continuation` is 'present when lowering the budget suspended the task'"
    );
    assert_eq!(
        commitments(&lowered),
        published,
        "the committed partial evidence the task parks from is unchanged and still named"
    );

    let after = record(&status(&mut fixture, &task, "req_status_2"));
    assert_eq!(after.cost.states, Optional::Present(PARKED_STATES));
    let milestones: Vec<&str> = after
        .milestones
        .iter()
        .map(|milestone| milestone.name.as_str())
        .collect();
    assert!(
        milestones.contains(&"task.budget_updated")
            && milestones.contains(&"task.budget_suspended"),
        "both facts are true: the ceiling was recorded, and it cannot bind this run — \
         {milestones:?}"
    );
    assert_eq!(
        entry(&fixture, &task).bounds().states(),
        usize::try_from(PARKED_STATES).expect("fits"),
        "the withdrawn ceiling is recorded and is not a bound that un-explores the walk"
    );

    // And the truncation that would have followed does not: resuming under the lowered
    // ceiling makes no further progress and loses nothing.
    let continuation = after
        .continuation
        .value()
        .cloned()
        .expect("still resumable");
    let resumed = resume(&mut fixture, &continuation, "req_resume", Optional::Absent);
    assert_eq!(
        resumed.envelope.status,
        ResultStatus::TaskSuspended,
        "the resumed run parks again — a withdrawn ceiling admits no further progress, \
         which is not the same thing as losing what was already explored: {:?}",
        resumed.envelope.error
    );
    let reread = record(&status(&mut fixture, &task, "req_status_3"));
    assert_eq!(
        reread.cost.states,
        Optional::Present(PARKED_STATES),
        "`cost` is monotone: a lowered ceiling admits no further progress and un-explores \
         nothing (INV-009)"
    );
    assert_eq!(
        commitments(&status(&mut fixture, &task, "req_status_4")).len(),
        2,
        "resume MAY add evidence; it MUST NOT replace prior artifacts"
    );
    assert!(
        commitments(&status(&mut fixture, &task, "req_status_5")).starts_with(&published),
        "the first publication's name survives the second"
    );
}

/// **The fifth arm.** A ceiling on a dimension nothing meters is recorded, and the answer
/// says so.
///
/// The arm had no wire surface at all before: `task.update_budget` accepted a `wall_ms`
/// ceiling and reported nothing about it, which is the declared-but-unenforced cell INV-007
/// exists to make impossible to leave silent.
#[test]
fn a_ceiling_nothing_meters_is_recorded_and_named_on_the_answer() {
    let mut fixture = fixture();
    let task = started_task(&start(
        &mut fixture,
        "req_start",
        "idem-start",
        Some(4),
        target(TargetKind::AllClaims, "DieHard"),
    ));
    let updated = update_budget(&mut fixture, &task, "req_update", every_dimension(64));
    let response = update_response(&updated);
    assert_eq!(
        response.budget,
        every_dimension(64),
        "every ceiling is recorded, metered or not"
    );
    let named = subjects(&updated);
    for expected in [
        "budget.wall_ms",
        "budget.cpu_ms",
        "budget.memory_bytes",
        "budget.solver_ms",
        "budget.proof_ms",
        "budget.tokens",
        "budget.candidates",
        "budget.bytes",
    ] {
        assert!(
            named.contains(&expected.to_owned()),
            "{expected} is recorded and unenforceable: {named:?}"
        );
    }
    assert!(!named.contains(&"budget.states".to_owned()));
    // Recording an unenforceable ceiling changes no spend and no bound but the metered one.
    assert_eq!(entry(&fixture, &task).bounds().states(), 64);
    assert_eq!(
        record(&status(&mut fixture, &task, "req_status"))
            .cost
            .states,
        Optional::Present(PARKED_STATES)
    );
}

/// Committed partial evidence is **named**, not counted, and the names only grow.
///
/// The gap bn-1gc recorded: both ledgers count publications, and a count that came back as
/// two says nothing about *which* two. Each publication now carries the content identity of
/// the campaign that produced it, and `ResultEnvelope.artifacts` is where a caller reads
/// them.
#[test]
fn committed_partial_evidence_is_named_and_the_names_only_grow() {
    let mut fixture = fixture();
    let task = started_task(&start(
        &mut fixture,
        "req_start",
        "idem-start",
        Some(4),
        target(TargetKind::AllClaims, "DieHard"),
    ));
    let parked = status(&mut fixture, &task, "req_status_1");
    assert_eq!(parked.envelope.artifacts.len(), 1);
    let reference = &parked.envelope.artifacts[0];
    assert_eq!(
        reference.kind, "task",
        "the plan §4.4 prefix without its underscore"
    );
    assert_eq!(reference.handle.as_str(), task.as_str());
    assert!(
        reference.commitment.value().is_some(),
        "a named publication carries the content identity of what was committed"
    );
    assert!(reference.redacted.is_absent());
    let first = commitments(&parked);

    let continuation = record(&status(&mut fixture, &task, "req_status_2"))
        .continuation
        .value()
        .cloned()
        .expect("parked");
    let resumed = resume(
        &mut fixture,
        &continuation,
        "req_resume",
        Optional::Present(budget(Some(FROZEN_STATES * 4))),
    );
    assert_eq!(resumed.envelope.status, ResultStatus::Ok);

    let after = status(&mut fixture, &task, "req_status_3");
    let both = commitments(&after);
    assert_eq!(both.len(), 2, "resume MAY add evidence (INV-009)");
    assert!(
        both.starts_with(&first),
        "and MUST NOT replace prior artifacts"
    );
    assert_ne!(
        both[0], both[1],
        "a re-derived artifact receives a new identity: the resumed walk went further"
    );
    assert_eq!(
        entry(&fixture, &task).publications(),
        2,
        "the count is a consequence of the named list, not a number kept beside it"
    );
    let sequences: Vec<u32> = entry(&fixture, &task)
        .evidence
        .committed()
        .iter()
        .map(continuumd::daemon::budget::Publication::sequence)
        .collect();
    assert_eq!(sequences, vec![0, 1], "dense ordinals in commit order");
    // Every publication was priced: the checkpoint the budget ledger bound to it names the
    // spend recorded when it committed.
    let priced: Vec<u32> = entry(&fixture, &task)
        .evidence
        .committed()
        .iter()
        .map(|publication| publication.checkpoint().committed())
        .collect();
    assert_eq!(priced, vec![1, 2]);
}

/// Uncommitted partials are absent, never half-visible.
///
/// Three readings of the same claim: a run that failed before it could publish names
/// nothing, no task is left staging at rest, and the committed list and the count agree.
#[test]
fn an_uncommitted_partial_is_absent_never_half_visible() {
    let mut fixture = fixture();
    // A state budget too small to hold the model's own initial states: the campaign never
    // came back, so nothing was staged and nothing is named.
    let failed = started_task(&start(
        &mut fixture,
        "req_start",
        "idem-start",
        Some(0),
        target(TargetKind::AllClaims, "DieHard"),
    ));
    let answer = status(&mut fixture, &failed, "req_status_1");
    assert_eq!(record(&answer).status, TaskStatus::Failed);
    assert!(
        answer.envelope.artifacts.is_empty(),
        "a task that published nothing names nothing"
    );
    assert_eq!(entry(&fixture, &failed).publications(), 0);
    assert!(!entry(&fixture, &failed).evidence.is_staging());

    // And a task that did publish holds no publication in flight at rest: every path out of
    // the run either commits the staged publication or discards it.
    let parked = started_task(&start(
        &mut fixture,
        "req_start_2",
        "idem-start-2",
        Some(4),
        target(TargetKind::AllClaims, "DieHard"),
    ));
    for handle in [&failed, &parked] {
        let held = entry(&fixture, handle);
        assert!(
            !held.evidence.is_staging(),
            "no publication is neither committed nor absent at rest"
        );
        assert_eq!(
            held.publications() as usize,
            held.evidence.committed().len(),
            "the count and the named list are one accounting"
        );
    }
    assert_eq!(entry(&fixture, &parked).publications(), 1);
    assert!(
        fixture.daemon.state().regions().is_total(),
        "{}",
        fixture.daemon.state().regions().render()
    );
}

/// Two daemons that ran one campaign name its publications identically.
///
/// The identity half of "committed partial evidence survives daemon restart": a publication
/// is named by the content identity of the campaign that produced it, so the name is a
/// function of the campaign and of nothing about the process that ran it (INV-005, INV-006).
/// Durability itself is bn-3dr's; see this file's header for exactly what is and is not
/// claimed.
#[test]
fn two_daemons_name_one_campaigns_publications_identically() {
    let run = || {
        let mut fixture = fixture();
        let task = started_task(&start(
            &mut fixture,
            "req_start",
            "idem-start",
            Some(4),
            target(TargetKind::AllClaims, "DieHard"),
        ));
        let names = commitments(&status(&mut fixture, &task, "req_status"));
        let rendered = entry(&fixture, &task).evidence.render();
        (names, rendered)
    };
    let (first, first_render) = run();
    let (second, second_render) = run();
    assert_eq!(first, second);
    assert_eq!(first_render.as_bytes(), second_render.as_bytes());
    assert_eq!(first.len(), 1, "a vacuous agreement proves nothing");
    assert!(!first_render.is_empty());
}

/// **SD-13.** A `BudgetExhausted` on the wire carries a continuation or a typed
/// `non_resumable_reason` — never both absent.
///
/// > Budget exhaustion is never a silent dead end (plan §11.4): a `BudgetExhausted` failure
/// > carries a non-null continuation or a typed `non_resumable_reason`.
/// >
/// > — `notes/plan/schemas/verification-task.schema.json`
///
/// `Error.continuation` and `Error.non_resumable_reason` were hard-coded absent in
/// `result::failure`, so every `BudgetExhausted` this daemon raised was exactly the dead end
/// that clause forbids. bn-1gc listed this as its first acceptance criterion and deferred it
/// here, because the wire join was outside its fence.
#[test]
fn budget_exhausted_on_the_wire_is_never_a_silent_dead_end() {
    let mut fixture = fixture();
    let task = started_task(&start(
        &mut fixture,
        "req_start",
        "idem-start",
        Some(0),
        target(TargetKind::AllClaims, "DieHard"),
    ));
    let record = record(&status(&mut fixture, &task, "req_status"));
    assert_eq!(
        record.failed_reason,
        Optional::Present(ErrorCode::BudgetExhausted)
    );
    let reason = record
        .non_resumable_reason
        .value()
        .cloned()
        .expect("a failed task is never silent (plan §4.5)");
    assert!(record.continuation.is_absent());

    let refused = fixture.daemon.dispatch(&OperationRequest {
        envelope: envelope(
            "verification.result",
            "agent:runner",
            "cap_runner",
            "req_result",
        ),
        arguments: Arguments::VerificationResult(VerificationResultRequest { task }),
    });
    assert_eq!(
        refused.envelope.status,
        ResultStatus::Error,
        "the one result path that is an error rather than a verdict"
    );
    let error = refused
        .envelope
        .error
        .value()
        .cloned()
        .expect("an error status carries one");
    assert_eq!(error.code, ErrorCode::BudgetExhausted);
    assert!(
        error.continuation.is_absent(),
        "initial states are what exploration starts from, so there is no prefix to resume"
    );
    assert_eq!(
        error.non_resumable_reason,
        Optional::Present(reason),
        "one string: the reason a `task.status` reader sees and the reason the failing call \
         carries cannot drift apart"
    );
}
