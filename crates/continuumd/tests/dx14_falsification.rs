//! **G0-DX-14 falsification.** An adversarial campaign against the row bn-2zy flipped to
//! `Evidence (reference implementation)`, run through `Daemon::dispatch` and nothing else.
//!
//! > | G0-DX-14 | Can cancellation of verification work close correctly? | cancel DPOR,
//! > solver, proof, and synthesis tasks at every phase | no leaked obligations; resumable
//! > artifacts either committed or absent |
//! >
//! > — `notes/plan/notes/G0_SPIKE_MATRIX.md`
//!
//! `dx14_cancellation_matrix.rs` (both halves) is the *constructive* sweep: it walks each
//! lane's own program, cancels after every phase, and holds the answer to a declared table.
//! This file is the attack on what that sweep takes for granted. Its house rule is the one
//! bn-21dd's DX-13 campaign set: **the system under test is frozen.** Nothing in `src/` is
//! patched to make an attack pass or fail; every attack is a sequence a client can issue.
//!
//! # What the constructive sweep takes for granted, and what is attacked here
//!
//! | Assumption | Attack |
//! |---|---|
//! | a cancelled task is *closed* — nothing writes it afterwards | resume its continuation with a budget (A1, A2) |
//! | the two readings of "is there a continuation" always agree | drive every reachable residual profile and compare them on the wire (A3) |
//! | `CancelOutcome` has no constructor for "committed evidence, no continuation" | try to reach that **observable** state through any wire sequence (A4) |
//! | a cancel is idempotent | compare the second and third cancels byte-for-byte (A5) |
//! | B18 is a park, not a truncation | lower below committed spend, then cancel; lower, resume, lower again (A6, A7) |
//! | the four lanes' declared resumability is honest | try to make a non-resumable lane hold a continuation (A8) |
//! | a continuation is a durable artifact, not a live scope | replay one on a fresh daemon, and after its task was cancelled *and* re-resumed (A9, A10) |
//! | teardown is total whatever order the operations arrive in | sweep every ordering of a five-operation hostile suffix (A11) |
//! | a cancel closes the identity too | re-issue the identical `verification.start` afterwards (A12) |
//! | the whole thing is deterministic | render the attack programme on two fresh daemons (A13) |
//!
//! # Campaign map — attack → test → result
//!
//! | # | Attack | Test | Result |
//! |---|---|---|---|
//! | A1 | cancel, then `task.resume` the dead task's continuation **carrying a budget** | [`regression_a_cancelled_task_is_not_writable_through_task_resume`] | **FALSIFIED (closure, not the ledger)**, then **FIXED** by bn-10093 — now a regression guard |
//! | A2 | the same, with no budget on the request | [`negative_a_resume_without_a_budget_leaves_a_cancelled_task_untouched`] | held — the write is the budget arm's, and only that arm's |
//! | A3 | every reachable residual profile: the region arm and the task's continuation, compared on the wire | [`negative_the_two_readings_of_resumability_agree_on_every_reachable_profile`] | held |
//! | A4 | reach "committed evidence and no continuation" as an *observable* wire state | [`negative_no_wire_sequence_reaches_a_dangling_continuation_or_a_pointerless_resume`] | held |
//! | A5 | cancel twice, three times; byte-identity of the repeats | [`negative_a_repeated_cancel_is_byte_identical_and_opens_no_new_obligation`] | held |
//! | A6 | B18: lower the ceiling below committed spend, **then** cancel | [`negative_a_cancel_after_a_ceiling_lowered_below_spend_still_settles_both_or_neither`] | held |
//! | A7 | B18: lower, resume, lower again — three times | [`negative_repeated_lowering_and_resuming_never_un_explores_or_un_publishes`] | held |
//! | A8 | make a lane the daemon declared non-resumable hold a continuation | [`negative_a_non_resumable_lane_never_acquires_a_continuation_by_any_route`] | held |
//! | A9 | resume a cancelled task's continuation on a **fresh** daemon | [`negative_a_cancelled_daemons_continuation_does_not_resolve_on_a_fresh_daemon`] | held |
//! | A10 | replay a stale continuation after its task was cancelled *and* re-resumed | [`negative_a_stale_continuation_replayed_after_cancellation_runs_nothing`] | held |
//! | A11 | every ordering of a five-operation hostile suffix (120 programmes) | [`negative_every_ordering_of_a_hostile_operation_suffix_leaves_a_total_daemon`] | held |
//! | A12 | re-issue the identical `verification.start` after the cancel | [`negative_a_cancelled_identity_is_reported_as_cancelled_and_never_re_run`] | held, with a concern recorded in the test |
//! | A13 | the whole attack programme on two fresh daemons | [`negative_the_attack_programme_renders_byte_identically_on_two_fresh_daemons`] | held |
//!
//! # The one that landed, and the fix that closed it
//!
//! [`regression_a_cancelled_task_is_not_writable_through_task_resume`] ran as A1's
//! FALSIFICATION when bn-3p32 wrote it. In one sentence: `task.update_budget` refuses to
//! rewrite a terminal task's ledger *and says why* — "a terminal task's budget is a
//! historical fact, and rewriting it would make its recorded cost unreadable" — while
//! `task.resume` applied the request's budget to the ledger **before** it checked
//! terminality, so the sequence `cancel` → `resume(continuation, budget)` rewrote the
//! ceilings of a task that was already `Cancelled` and appended `task.budget_suspended` to
//! its milestone list.
//!
//! What broke and what did not, stated as precisely as the campaign could state it:
//!
//! - the pass condition's two named conjuncts **held** under all thirteen attacks. No
//!   obligation leaks — `TaskRegions::is_total` is asserted after every beat of every attack,
//!   including all 120 orderings of A11 — and no artifact is left neither committed nor
//!   absent;
//! - what broke is the **closure** the row's own question presupposes: "can cancellation of
//!   verification work close *correctly*". A cancelled task was still writable, so its record
//!   was not final, and the specific invariant broken was one the code states and enforces in
//!   one operation and did not enforce in the other. A cancelled task ended up reporting a
//!   budget it never ran under, a cost above that budget, and a milestone saying it parked.
//!
//! **bn-10093 fixed it**, in `daemon::task::resume`, by moving the terminal check above the
//! ledger write: the budget arm of a resume now produces `task.update_budget`'s own
//! observable for a terminal task — nothing written, the terminal status answered, the record
//! byte-identical. A1's test is unchanged as a *sequence* and flipped as an *assertion*: it
//! now asserts the record is byte-identical across the resume, which is A2's shape, so the
//! two tests state one behaviour. bn-1kp6's attack 18 pins the same fix from the DX-03 side
//! (`crates/continuumd/tests/dx03_falsification.rs`), and the disposition on the continuation
//! table — a terminal task's continuations are **kept**, guarded by the refusal rather than
//! pruned — is recorded in `daemon::task`'s `TaskTable` documentation, where the table is.
//!
//! So this was a defect in what a cancellation leaves *readable*, not in what it leaves
//! *outstanding*, and the row records the falsification-and-fix cycle rather than a bare
//! pass.
//!
//! # OOM hygiene
//!
//! Every sweep is bounded and linear in written-down data: A11 is `5! = 120` programmes over
//! a four-state model, A3 is 8 profiles, A7 is 3 rounds. No input is built by doubling.

use continuum_engine_reference::diehard;
use continuum_intent::canonical_json::Json;
use continuum_intent::contract::IntentContract;
use continuum_value::epoch::ProtocolWindow;
use continuum_workspace::snapshot::WorkspacePath;
use continuumd::codec::operations::encode_payload;
use continuumd::codec::to_bytes;
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::region::TaskRegions;
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
use continuumd::protocol::spec::{Nullable, Optional};
use continuumd::protocol::task::TaskRecord;
use continuumd::protocol::vocabulary::{
    AuthorityLevel, Encoding, ErrorCode, Portfolio, ResultStatus, TargetKind, TaskStatus,
};

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

// --- fixtures ---------------------------------------------------------------------------------
//
// Duplicated from `dx14_cancellation_matrix.rs` rather than imported: a `tests/*.rs` file is
// its own crate and nothing here can `use` a sibling one. The Die Hard model and contract are
// `include_str!`'d from the one copy of each in this repository, so no fixture here can drift
// from the corpus.

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
        client: "continuumd-dx14-falsification".to_owned(),
        actor: who("service:continuumd"),
        capability: cap("cap_root"),
        features: Optional::Absent,
    };
    negotiate(&[version()], ProtocolWindow::new(3), ENCODINGS, &hello).expect("3.1 is served")
}

/// The epochs this daemon serves. Pinned, so a continuation has something to pin.
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

fn die_hard_source() -> Commitment {
    model_source(&Blake3Identity, [(MODULE_PATH, DIE_HARD_MODEL.as_bytes())])
        .expect("blake3 names the module set")
}

/// A daemon holding the Die Hard workspace sealed, its contract accepted, and the model
/// registered in the catalog.
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

// --- the operations, each parameterised by the request identity --------------------------------

fn start(fixture: &mut Fixture, request: &str, states: u64, target: Target) -> OperationOutcome {
    let snapshot = fixture.snapshot.clone();
    fixture.daemon.dispatch(&OperationRequest {
        envelope: on(
            budgeted(
                keyed(
                    envelope("verification.start", "agent:runner", "cap_runner", request),
                    &format!("idem-{request}"),
                ),
                Some(states),
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

/// `task.resume`, with the request's `budget` field under the caller's control.
///
/// The constructive sweep always sends one; A1 and A2 turn it on and off, because that is
/// exactly the difference between the arm that writes a terminal task and the arm that does
/// not.
fn resume_with(
    fixture: &mut Fixture,
    continuation: &ContinuationHandle,
    states: Option<u64>,
    request: &str,
) -> OperationOutcome {
    fixture.daemon.dispatch(&OperationRequest {
        envelope: budgeted(
            keyed(
                envelope("task.resume", "agent:runner", "cap_runner", request),
                &format!("idem-{request}"),
            ),
            states,
        ),
        arguments: Arguments::TaskResume(TaskResumeRequest {
            continuation: continuation.clone(),
            budget: match states {
                Some(states) => Optional::Present(budget(Some(states))),
                None => Optional::Absent,
            },
        }),
    })
}

fn resume(
    fixture: &mut Fixture,
    continuation: &ContinuationHandle,
    states: u64,
    request: &str,
) -> OperationOutcome {
    resume_with(fixture, continuation, Some(states), request)
}

fn rebudget(
    fixture: &mut Fixture,
    task: &TaskHandle,
    states: u64,
    request: &str,
) -> OperationOutcome {
    fixture.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope("task.update_budget", "agent:runner", "cap_runner", request),
            &format!("idem-{request}"),
        ),
        arguments: Arguments::TaskUpdateBudget(TaskUpdateBudgetRequest {
            task: task.clone(),
            budget: budget(Some(states)),
        }),
    })
}

fn read(fixture: &mut Fixture, task: &TaskHandle, request: &str) -> OperationOutcome {
    fixture.daemon.dispatch(&OperationRequest {
        envelope: envelope("task.status", "agent:reader", "cap_reader", request),
        arguments: Arguments::TaskStatus(TaskStatusRequest { task: task.clone() }),
    })
}

fn cancel(fixture: &mut Fixture, task: &TaskHandle, request: &str) -> OperationOutcome {
    cancel_keyed(fixture, task, request, &format!("idem-{request}"))
}

/// `task.cancel` with the request identity and the idempotency key controlled separately.
///
/// A5 needs both: the *same* `request_id`, because a result envelope echoes it and two
/// answers that differ only there are not the comparison the attack is making, and *distinct*
/// idempotency keys, because `rule idempotency.replay` would otherwise answer the second call
/// from the ledger and a replayed answer is byte-identical for an uninteresting reason.
fn cancel_keyed(
    fixture: &mut Fixture,
    task: &TaskHandle,
    request: &str,
    key: &str,
) -> OperationOutcome {
    fixture.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope("task.cancel", "agent:runner", "cap_runner", request),
            key,
        ),
        arguments: Arguments::TaskCancel(TaskCancelRequest { task: task.clone() }),
    })
}

// --- reading the daemon ------------------------------------------------------------------------

fn regions(fixture: &Fixture) -> &TaskRegions {
    fixture.daemon.state().regions()
}

fn entry<'a>(fixture: &'a Fixture, task: &TaskHandle) -> &'a continuumd::daemon::task::TaskEntry {
    fixture
        .daemon
        .state()
        .tasks()
        .get(task)
        .expect("the daemon holds the task")
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

fn record(outcome: &OperationOutcome) -> TaskRecord {
    match &outcome.payload {
        Payload::TaskStatus(record) => record.clone(),
        other => panic!("expected a task.status payload, got {other:?}"),
    }
}

/// The wire bytes of one result: the envelope followed by the typed payload's own encoding,
/// exactly as `transport::Server::answer` splices them into one frame. Two calls compare
/// byte-identical here if and only if they would have produced the identical frame on the
/// real wire (the device `inv002_no_hidden_state_evidence.rs` uses).
fn wire_bytes(outcome: &OperationOutcome) -> Vec<u8> {
    let mut bytes = to_bytes(&outcome.envelope).expect("a result envelope encodes");
    if let Some(payload) = encode_payload(&outcome.payload).expect("a payload encodes") {
        bytes.extend_from_slice(payload.as_bytes());
    }
    bytes
}

/// The daemon-grain no-orphan property, with the region ledger as the failure message.
///
/// Six independently computed conjuncts live behind `is_total`; this is asserted after every
/// beat of every attack, because an attack that broke the teardown and left the wire answer
/// intact is exactly the one a wire-only check would miss.
fn assert_total(fixture: &Fixture, when: &str) {
    let regions = regions(fixture);
    assert!(
        regions.is_total(),
        "{when}: the daemon's regions are not total\n{}",
        regions.render()
    );
}

/// The `continuation` the wire answered a cancel with.
fn cancelled_continuation(outcome: &OperationOutcome) -> Nullable<ContinuationHandle> {
    match &outcome.payload {
        Payload::TaskCancel(response) => response.continuation.clone(),
        other => panic!("expected a task.cancel payload, got {other:?}"),
    }
}

/// Whether an `optional` wire field carries a value.
///
/// `Optional` names the absent case, which is the presence distinction RFC 0026 asks the wire
/// to keep; this is the affirmative reading of it, so an assertion reads as the claim it is
/// making rather than as a double negative.
fn present<T>(field: &Optional<T>) -> bool {
    !field.is_absent()
}

/// `all_claims` over Die Hard — the campaign every attack that needs a parked task uses.
fn all_claims() -> Target {
    Target {
        kind: TargetKind::AllClaims,
        id: "DieHard".to_owned(),
    }
}

/// A task parked with committed partial evidence plus a valid continuation.
///
/// The residual profile every "close it, then attack the corpse" sequence starts from:
/// `publications = 1`, `continuation = Some`, `status = suspended`.
fn parked(fixture: &mut Fixture, request: &str) -> (TaskHandle, ContinuationHandle) {
    let outcome = start(fixture, request, 4, all_claims());
    assert!(
        outcome.error_code().is_none(),
        "the parked fixture start was refused — {:?}",
        outcome.envelope.error
    );
    let task = started_task(&outcome);
    let continuation = entry(fixture, &task)
        .continuation
        .clone()
        .expect("a bounded run parks with a continuation");
    assert_eq!(entry(fixture, &task).status, TaskStatus::Suspended);
    assert_eq!(entry(fixture, &task).publications(), 1);
    (task, continuation)
}

// --- A1: the attack that landed ------------------------------------------------------------

/// **REGRESSION GUARD (was A1's FALSIFICATION) — a cancelled task is not writable through
/// `task.resume`.**
///
/// # What must hold
///
/// A cancelled task is closed. `rule task.status_monotonic` makes its status permanent, and
/// `task.update_budget` extends that to the ledger *explicitly*, refusing a terminal task and
/// stating the reason at the refusal:
///
/// > A terminal task keeps the budget it ran under and answers
/// > [`StructuralOutcome::Unchanged`]: a terminal task's budget is a historical fact, and
/// > rewriting it would make its recorded cost unreadable.
/// >
/// > — `crates/continuumd/src/daemon/task.rs`, `update_budget`
///
/// So after `task.cancel`, no operation may change what the task's record says it was
/// allowed to spend, and no operation may append a milestone claiming the dead task did
/// something.
///
/// # What this test found, and what fixed it
///
/// As bn-3p32 ran it, this was a FALSIFICATION: `task.resume` applied the request's optional
/// budget to the task's `BudgetLedger` **before** it checked terminality — the ledger write
/// was at the top of the function and the `entry.is_terminal()` early return below it — so
/// the sequence
///
/// ```text
/// verification.start (parks)  →  task.cancel  →  task.resume(continuation, budget)
/// ```
///
/// reached that write on a task whose status was already `Cancelled`: `TaskRecord.budget`
/// changed after the task closed, and with the new ceiling below recorded spend the B18 arm
/// fired and appended `task.budget_suspended` — and a `TaskEvent` for it — to a **cancelled**
/// task's record.
///
/// bn-10093 moved the terminal check above the ledger write, so the budget arm produces
/// `task.update_budget`'s own observable for a terminal task: nothing is written and the
/// terminal status is the answer. The two operations no longer disagree about one rule.
///
/// # What the assertions are now
///
/// The same wire sequence, and the record compared **byte for byte** across it — A2's shape,
/// which is the point: with the budget arm refused, a resume carrying a budget and a resume
/// carrying none are one behaviour. The ceiling, the milestone list, the recorded spend, the
/// publication count and the continuation are each asserted individually as well, so a
/// regression says *which* of them moved rather than only that the bytes differ.
///
/// What was never broken is still checked here, because a repair that bought closure by
/// leaking an obligation would be a worse defect than the one it fixed: the resume returns
/// before `verification::advance`, so it opens no region, runs no work, publishes nothing and
/// mints no continuation, and `TaskRegions::is_total` holds throughout.
#[test]
fn regression_a_cancelled_task_is_not_writable_through_task_resume() {
    let mut fixture = fixture();
    let (task, continuation) = parked(&mut fixture, "req_a1");

    let cancelled = cancel(&mut fixture, &task, "req_a1_cancel");
    assert_eq!(cancelled.envelope.status, ResultStatus::Ok);
    assert_total(&fixture, "after the cancel");

    let before_bytes = wire_bytes(&read(&mut fixture, &task, "req_a1_read"));
    let before = record(&read(&mut fixture, &task, "req_a1_read"));
    assert_eq!(before.status, TaskStatus::Cancelled);
    let spent = *before
        .cost
        .states
        .value()
        .expect("the campaign recorded a state count");
    let ceiling = *before
        .budget
        .states
        .value()
        .expect("the task ran under a declared state ceiling");
    assert!(
        spent > 1,
        "the attack needs a ceiling strictly below recorded spend to reach the B18 arm"
    );

    // The attack, unchanged: resume the dead task's continuation, carrying a budget below
    // its spend — the one request that used to reach the ledger.
    let resumed = resume(&mut fixture, &continuation, 1, "req_a1_resume");
    assert_eq!(
        resumed.envelope.status,
        ResultStatus::Ok,
        "the resume itself is still answered rather than refused — {:?}",
        resumed.envelope.error
    );

    let after_bytes = wire_bytes(&read(&mut fixture, &task, "req_a1_read"));
    let after = record(&read(&mut fixture, &task, "req_a1_read"));

    // --- the guard ---------------------------------------------------------------------
    assert_eq!(
        after_bytes, before_bytes,
        "a cancelled task's record must be byte-identical across a resume carrying a budget \
         — the same thing A2 asserts for a resume carrying none"
    );
    assert_eq!(
        after.status,
        TaskStatus::Cancelled,
        "the status is monotone"
    );
    assert_eq!(
        after.budget.states.value().copied(),
        Some(ceiling),
        "a cancelled task keeps the budget it ran under: the rewrite task.update_budget \
         refuses is refused here too, for the same stated reason"
    );
    assert_eq!(
        after.milestones.len(),
        before.milestones.len(),
        "a cancelled task gains no milestone after it closed"
    );
    assert!(
        !after
            .milestones
            .iter()
            .any(|milestone| milestone.name == "task.budget_suspended"),
        "B18's arm does not fire on a task that can never resume"
    );
    assert!(
        after.budget.states.value().copied() >= Some(spent),
        "the record never reports a task that spent more than it was allowed to"
    );

    // --- what was never broken, still checked ------------------------------------------
    assert_eq!(
        after.cost.states.value().copied(),
        Some(spent),
        "recorded spend is untouched: nothing was un-explored"
    );
    assert_eq!(
        entry(&fixture, &task).publications(),
        1,
        "no publication was added or removed"
    );
    assert_eq!(
        after.continuation, before.continuation,
        "no new continuation was minted for a dead task"
    );
    assert_total(&fixture, "after the resume of a cancelled task");
    assert_eq!(
        regions(&fixture).opened(),
        2,
        "the resume of a terminal task opens no scope: one for the start, one for the cancel"
    );
}

/// A2 — the same sequence with **no** budget on the resume request leaves the cancelled task
/// byte-for-byte as the cancel left it.
///
/// The control that localises A1: the write is the budget arm's and only that arm's, so the
/// finding is "`task.resume` applies a budget before checking terminality" rather than
/// "`task.resume` touches terminal tasks". A repair therefore has one line to move, and this
/// test is what would catch a repair that moved it too far.
#[test]
fn negative_a_resume_without_a_budget_leaves_a_cancelled_task_untouched() {
    let mut fixture = fixture();
    let (task, continuation) = parked(&mut fixture, "req_a2");
    cancel(&mut fixture, &task, "req_a2_cancel");

    let before = wire_bytes(&read(&mut fixture, &task, "req_a2_read"));
    let resumed = resume_with(&mut fixture, &continuation, None, "req_a2_resume");
    assert_eq!(resumed.envelope.status, ResultStatus::Ok);
    let after = wire_bytes(&read(&mut fixture, &task, "req_a2_read"));

    assert_eq!(
        before, after,
        "a budgetless resume of a cancelled task must change nothing a caller can read"
    );
    assert_total(&fixture, "after a budgetless resume of a cancelled task");
}

// --- A3: the two readings of resumability ---------------------------------------------------

/// One reachable residual profile, and the sequence that reaches it.
struct Profile {
    token: &'static str,
    /// The beats before the cancel. Each is a `(states, kind)` pair the driver interprets.
    build: fn(&mut Fixture, &str) -> TaskHandle,
}

fn profile_fresh_park(fixture: &mut Fixture, request: &str) -> TaskHandle {
    parked(fixture, request).0
}

fn profile_twice_parked(fixture: &mut Fixture, request: &str) -> TaskHandle {
    let (task, continuation) = parked(fixture, request);
    resume(fixture, &continuation, 8, &format!("{request}_r1"));
    task
}

fn profile_closed(fixture: &mut Fixture, request: &str) -> TaskHandle {
    let outcome = start(fixture, request, 64, all_claims());
    started_task(&outcome)
}

fn profile_parked_then_closed(fixture: &mut Fixture, request: &str) -> TaskHandle {
    let (task, continuation) = parked(fixture, request);
    resume(fixture, &continuation, 64, &format!("{request}_r1"));
    task
}

fn profile_failed(fixture: &mut Fixture, request: &str) -> TaskHandle {
    let outcome = start(fixture, request, 0, all_claims());
    started_task(&outcome)
}

fn profile_rebudgeted_park(fixture: &mut Fixture, request: &str) -> TaskHandle {
    let (task, _) = parked(fixture, request);
    rebudget(fixture, &task, 64, &format!("{request}_b1"));
    task
}

fn profile_lowered_park(fixture: &mut Fixture, request: &str) -> TaskHandle {
    let (task, _) = parked(fixture, request);
    rebudget(fixture, &task, 1, &format!("{request}_b1"));
    task
}

fn profile_read_only(fixture: &mut Fixture, request: &str) -> TaskHandle {
    let (task, _) = parked(fixture, request);
    read(fixture, &task, &format!("{request}_read"));
    task
}

const PROFILES: &[Profile] = &[
    Profile {
        token: "fresh-park",
        build: profile_fresh_park,
    },
    Profile {
        token: "twice-parked",
        build: profile_twice_parked,
    },
    Profile {
        token: "closed",
        build: profile_closed,
    },
    Profile {
        token: "parked-then-closed",
        build: profile_parked_then_closed,
    },
    Profile {
        token: "failed",
        build: profile_failed,
    },
    Profile {
        token: "rebudgeted-park",
        build: profile_rebudgeted_park,
    },
    Profile {
        token: "lowered-park",
        build: profile_lowered_park,
    },
    Profile {
        token: "read-only",
        build: profile_read_only,
    },
];

/// A3 — the region layer's cancellation arm and the task's own continuation are two readings
/// of one fact, and they agree on every residual profile a client can reach.
///
/// `task.cancel` carries a `debug_assert_eq!` saying exactly that. A `debug_assert` is not
/// evidence: it is compiled out of a release build and it fires only on the profiles a test
/// happens to reach. This drives eight distinct profiles and holds the agreement as a real
/// assertion — and holds the *wire* answer to it too, which is what a client sees.
#[test]
fn negative_the_two_readings_of_resumability_agree_on_every_reachable_profile() {
    let mut checked = 0_usize;
    let mut arms: std::collections::BTreeSet<bool> = std::collections::BTreeSet::new();
    for profile in PROFILES {
        let mut fixture = fixture();
        let task = (profile.build)(&mut fixture, &format!("req_a3_{}", profile.token));
        assert_total(&fixture, profile.token);

        let held = entry(&fixture, &task).continuation.is_some();
        let publications = entry(&fixture, &task).publications();
        // The claim `continuation.is_some() ⇒ publications > 0`, which `Publications::count`
        // says holds by construction. If it ever failed, the region layer would answer
        // `nothing-published` while the task held a resume pointer, and the wire's nullable
        // `continuation` would contradict `task.status`.
        assert!(
            !held || publications > 0,
            "{}: a task holds a continuation with nothing committed behind it",
            profile.token
        );

        let outcome = cancel(&mut fixture, &task, "req_a3_cancel");
        assert_eq!(outcome.envelope.status, ResultStatus::Ok);
        let wire = cancelled_continuation(&outcome);
        let report = regions(&fixture)
            .finalizations()
            .last()
            .expect("the cancel's own teardown");
        let worker = report
            .workers()
            .first()
            .expect("the cancel's scope owns one worker");
        let arm_carries_one = worker
            .cancel_outcome()
            .is_some_and(|outcome| outcome.continuation().is_some());

        assert_eq!(
            arm_carries_one,
            wire != Nullable::Null,
            "{}: the region arm and the wire's nullable continuation disagree",
            profile.token
        );
        assert_eq!(
            arm_carries_one, held,
            "{}: the region arm and the task's own continuation disagree",
            profile.token
        );
        assert_total(&fixture, profile.token);
        arms.insert(arm_carries_one);
        checked += 1;
    }
    assert_eq!(checked, 8, "eight profiles were driven");
    assert_eq!(
        arms.len(),
        2,
        "a sweep that reached only one arm has not tested the agreement"
    );
}

// --- A4: the constructor claim, on the wire --------------------------------------------------

/// A4 — no wire sequence reaches "committed evidence and no continuation" as an *observable*
/// dangling pair, and none reaches a continuation with nothing behind it.
///
/// `CancelOutcome` has no constructor for either shape. That is a claim about a type; this is
/// the claim about the **protocol**, which is the one G0-DX-14's pass condition is stated
/// over. For every reachable profile it holds the wire's own answer to both halves:
///
/// - a cancel that answered with a `cont_*` names at least one committed artifact in
///   `ResultEnvelope.artifacts` — never a resume pointer into nothing;
/// - a cancel that named no artifact answered `continuation: null` — never a dangling pointer
///   over evidence nobody can resume from.
///
/// The third arm — committed evidence, honestly non-resumable — is the dossier's own case and
/// is checked to be *named* rather than silent: `task.status` reports it with the continuation
/// field null and the artifacts present, which is a client-visible distinction from the
/// nothing-published arm.
#[test]
fn negative_no_wire_sequence_reaches_a_dangling_continuation_or_a_pointerless_resume() {
    let mut with_pointer = 0_usize;
    let mut with_evidence_only = 0_usize;
    let mut empty = 0_usize;
    for profile in PROFILES {
        let mut fixture = fixture();
        let task = (profile.build)(&mut fixture, &format!("req_a4_{}", profile.token));
        let outcome = cancel(&mut fixture, &task, "req_a4_cancel");
        let wire = cancelled_continuation(&outcome);
        let artifacts = outcome.envelope.artifacts.len();

        match (wire != Nullable::Null, artifacts > 0) {
            (true, true) => with_pointer += 1,
            (false, true) => with_evidence_only += 1,
            (false, false) => empty += 1,
            (true, false) => panic!(
                "{}: the cancel answered with a continuation and named no committed artifact \
                 — a resume pointer into nothing",
                profile.token
            ),
        }

        // The same pair, re-read through `task.status` rather than off the cancel's own
        // answer: a leak visible on one operation and not the other is still a leak.
        let after = record(&read(&mut fixture, &task, "req_a4_read"));
        assert_eq!(
            present(&after.continuation),
            wire != Nullable::Null,
            "{}: task.cancel and task.status disagree about the continuation",
            profile.token
        );
        assert_total(&fixture, profile.token);
    }
    assert!(
        with_pointer > 0 && with_evidence_only > 0 && empty > 0,
        "all three arms must be reached for the sweep to mean anything: \
         with-pointer={with_pointer} evidence-only={with_evidence_only} empty={empty}"
    );
}

// --- A5: idempotence of cancel ----------------------------------------------------------------

/// A5 — the second and third `task.cancel` are byte-identical to each other, add no
/// obligation the daemon does not discharge, and change nothing a caller can read.
///
/// The constructive sweep checks that a repeat answers `unchanged`. This checks the stronger
/// thing: the *bytes*. A response that drifted in any field — a milestone, a cost, an
/// artifact list, an omission — would be a cancel that is still doing work, and only a
/// byte comparison catches drift in a field nobody thought to assert.
///
/// The **first** repeat is deliberately compared against the second rather than against the
/// original: the original moves the status and records `task.cancelled`, so it is a different
/// answer by design, and `StructuralOutcome::Unchanged` is how the wire says so.
#[test]
fn negative_a_repeated_cancel_is_byte_identical_and_opens_no_new_obligation() {
    let mut fixture = fixture();
    let (task, _) = parked(&mut fixture, "req_a5");

    // One request identity across all three, so the comparison is about the *answer*; three
    // distinct idempotency keys, so nothing is served from the replay ledger.
    let first = cancel_keyed(&mut fixture, &task, "req_a5", "idem-a5-1");
    assert_eq!(first.envelope.status, ResultStatus::Ok);
    let opened_after_first = regions(&fixture).opened();

    let second = cancel_keyed(&mut fixture, &task, "req_a5", "idem-a5-2");
    let second_bytes = wire_bytes(&second);
    let third = cancel_keyed(&mut fixture, &task, "req_a5", "idem-a5-3");
    let third_bytes = wire_bytes(&third);

    assert_ne!(
        wire_bytes(&first),
        second_bytes,
        "the first cancel moves the status and must not be byte-equal to a repeat"
    );
    assert_eq!(
        second_bytes, third_bytes,
        "a repeated cancel must be byte-identical, field for field"
    );
    assert!(
        !second_bytes.is_empty(),
        "a vacuous comparison proves nothing"
    );
    assert_eq!(
        regions(&fixture).opened(),
        opened_after_first + 2,
        "each cancel opens exactly one scope, whatever it answers"
    );
    assert_eq!(
        regions(&fixture).finalizations().len(),
        regions(&fixture).opened() as usize,
        "and tears every one of them down inside its own dispatch"
    );
    assert_total(&fixture, "after three cancels");
}

// --- A6, A7: the B18 path, adversarially ------------------------------------------------------

/// A6 — lowering the ceiling below committed spend and *then* cancelling still settles
/// both-or-neither.
///
/// B18's suspension is the arm the daemon could not compute before bn-1gc, and it is the one
/// that leaves a task holding a ceiling it has already exceeded. The attack is that a cancel
/// arriving on top of that state has to decide resumability from a ledger that is internally
/// inconsistent — the ceiling says 1, the spend says more — and could read the withdrawn
/// ceiling as "nothing to resume".
#[test]
fn negative_a_cancel_after_a_ceiling_lowered_below_spend_still_settles_both_or_neither() {
    let mut fixture = fixture();
    let (task, _) = parked(&mut fixture, "req_a6");

    let lowered = rebudget(&mut fixture, &task, 1, "req_a6_low");
    assert_eq!(lowered.envelope.status, ResultStatus::Ok);
    let after_lowering = record(&read(&mut fixture, &task, "req_a6_read"));
    assert_eq!(
        after_lowering.status,
        TaskStatus::Suspended,
        "B18 parks; it does not terminate"
    );
    assert!(
        present(&after_lowering.continuation),
        "a suspended task is resumable by definition, and the withdrawn ceiling does not \
         change that"
    );
    assert!(
        after_lowering.budget.states.value().copied() < after_lowering.cost.states.value().copied(),
        "the fixture must actually be in the below-spend state the attack is about"
    );

    let outcome = cancel(&mut fixture, &task, "req_a6_cancel");
    assert_eq!(outcome.envelope.status, ResultStatus::Ok);
    assert_ne!(
        cancelled_continuation(&outcome),
        Nullable::Null,
        "committed evidence plus a valid continuation: the withdrawn ceiling must not \
         downgrade the arm to nothing-published"
    );
    assert!(
        !outcome.envelope.artifacts.is_empty(),
        "and the committed half must be named on the same answer"
    );
    assert_eq!(
        entry(&fixture, &task).publications(),
        1,
        "the lowering published nothing and un-published nothing"
    );
    assert_total(&fixture, "after a cancel over a withdrawn ceiling");
}

/// A7 — lower, resume, lower again, three rounds: nothing is un-explored and nothing is
/// un-published.
///
/// INV-009's prohibition is on *silent truncation*, and the shape that would violate it is a
/// monotone quantity going down: recorded spend shrinking because a ceiling was withdrawn, or
/// a publication disappearing because a run was re-priced. This drives the withdrawal three
/// times, resuming under the withdrawn ceiling each round, and holds both quantities monotone
/// — then cancels and holds the teardown total.
#[test]
fn negative_repeated_lowering_and_resuming_never_un_explores_or_un_publishes() {
    let mut fixture = fixture();
    let (task, _) = parked(&mut fixture, "req_a7");

    let mut spend = record(&read(&mut fixture, &task, "req_a7_read"))
        .cost
        .states
        .value()
        .copied()
        .expect("the campaign recorded a state count");
    let mut publications = entry(&fixture, &task).publications();

    for round in 0..3_u32 {
        rebudget(&mut fixture, &task, 1, &format!("req_a7_low_{round}"));
        let continuation = entry(&fixture, &task)
            .continuation
            .clone()
            .expect("a parked task holds a continuation to resume");
        let resumed = resume_with(
            &mut fixture,
            &continuation,
            None,
            &format!("req_a7_resume_{round}"),
        );
        // A resume that parks again lands on the `task_suspended` lane, which is a success:
        // only an `error` envelope is a refusal.
        assert!(
            resumed.error_code().is_none(),
            "round {round}: the resume under a withdrawn ceiling was refused — {:?}",
            resumed.envelope.error
        );

        let now = record(&read(&mut fixture, &task, "req_a7_read"));
        let next_spend = now
            .cost
            .states
            .value()
            .copied()
            .expect("a task that ran reports a state count");
        let next_publications = entry(&fixture, &task).publications();
        assert!(
            next_spend >= spend,
            "round {round}: recorded spend went down from {spend} to {next_spend} — a campaign \
             was un-explored"
        );
        assert!(
            next_publications >= publications,
            "round {round}: a publication disappeared ({publications} → {next_publications})"
        );
        assert!(
            present(&now.continuation),
            "round {round}: the task parked again and must still name a continuation"
        );
        spend = next_spend;
        publications = next_publications;
        assert_total(&fixture, "mid-lowering");
    }

    let outcome = cancel(&mut fixture, &task, "req_a7_cancel");
    assert_ne!(
        cancelled_continuation(&outcome),
        Nullable::Null,
        "the task is still resumable at the cancel, so the arm carries a continuation"
    );
    assert!(
        !outcome.envelope.artifacts.is_empty(),
        "and names the evidence it is resumable from"
    );
    assert_total(&fixture, "after three lowering rounds and a cancel");
}

// --- A8: declared resumability ------------------------------------------------------------

/// A8 — a lane the daemon reports as non-resumable never acquires a continuation, by any
/// route.
///
/// The `synthesis` lane of the constructive matrix is a campaign whose state ceiling cannot
/// hold the model's own initial states: `failed_reason = BudgetExhausted` with the
/// `non_resumable_reason` RFC 0026 requires beside it, and no continuation. The attack is to
/// try to give it one afterwards — raise the ceiling, cancel it, read it, raise it again —
/// and to check the wire never reports a resume pointer over a lane that declared it has
/// none.
#[test]
fn negative_a_non_resumable_lane_never_acquires_a_continuation_by_any_route() {
    let mut fixture = fixture();
    let outcome = start(&mut fixture, "req_a8", 0, all_claims());
    let task = started_task(&outcome);

    let failed = record(&read(&mut fixture, &task, "req_a8_read"));
    assert_eq!(failed.status, TaskStatus::Failed);
    assert_eq!(
        failed.failed_reason.value().copied(),
        Some(ErrorCode::BudgetExhausted)
    );
    assert!(
        present(&failed.non_resumable_reason),
        "RFC 0026 requires the typed reason when a BudgetExhausted failure carries no \
         continuation"
    );
    assert!(!present(&failed.continuation));

    // Every operation that could plausibly hand it one.
    rebudget(&mut fixture, &task, 64, "req_a8_raise");
    assert!(!present(
        &record(&read(&mut fixture, &task, "req_a8_r1")).continuation
    ));
    cancel(&mut fixture, &task, "req_a8_cancel");
    assert!(!present(
        &record(&read(&mut fixture, &task, "req_a8_r2")).continuation
    ));
    rebudget(&mut fixture, &task, 128, "req_a8_raise_again");
    let last = record(&read(&mut fixture, &task, "req_a8_r3"));
    assert!(
        !present(&last.continuation),
        "a lane that declared itself non-resumable acquired a continuation"
    );
    assert_eq!(
        last.status,
        TaskStatus::Failed,
        "and its terminal status is still the one it failed with"
    );
    assert_eq!(entry(&fixture, &task).publications(), 0);
    assert_total(&fixture, "after every route to a continuation was tried");
}

// --- A9, A10: continuation validity ---------------------------------------------------------

/// A9 — a continuation minted by one daemon does not resolve on a fresh one, and the refusal
/// is the single denial RFC 0027 X2 requires rather than a distinguishable not-found.
///
/// The two-fresh-daemons device, turned adversarial: daemon A parks a campaign and cancels it,
/// then daemon B — built identically, holding the identical sealed snapshot and the identical
/// registered model — is asked to resume A's `cont_*`. B has run nothing, so it holds no
/// continuation, and the honest answer is a denial. The second half is the control that makes
/// the first mean something: once B runs the same campaign it mints the **same** handle, so
/// the refusal above was about held state and not about a handle B could never name.
#[test]
fn negative_a_cancelled_daemons_continuation_does_not_resolve_on_a_fresh_daemon() {
    let mut first = fixture();
    let (task, continuation) = parked(&mut first, "req_a9");
    cancel(&mut first, &task, "req_a9_cancel");
    assert_total(&first, "daemon A after the cancel");

    let mut second = fixture();
    let refused = resume(&mut second, &continuation, 64, "req_a9_cross");
    assert_eq!(
        refused.error_code(),
        Some(ErrorCode::CapabilityDenied),
        "a continuation this daemon does not hold is the one denial, never a not-found"
    );
    assert!(
        second.daemon.state().tasks().handles().is_empty(),
        "the refused resume created no task"
    );
    assert_total(&second, "daemon B after the refused cross-daemon resume");

    // The control: B running the same campaign names the same continuation, so the refusal
    // above was about state B did not hold rather than about an unnameable handle.
    let (mirror_task, mirror_continuation) = parked(&mut second, "req_a9");
    assert_eq!(
        mirror_task, task,
        "the task identity is content-addressed and must agree across daemons"
    );
    assert_eq!(
        mirror_continuation, continuation,
        "so must the continuation's"
    );
    assert_total(&second, "daemon B after mirroring the campaign");
}

/// A10 — a stale continuation replayed after its task was cancelled *and* re-resumed runs
/// nothing and publishes nothing.
///
/// The nastiest ordering available at this grain: park (mint `C`), resume to completion,
/// cancel, then replay `C` — a handle that is still in the continuation table, that names a
/// task which has since closed twice over. Nothing prunes the table, so the handle resolves;
/// what must not happen is a run.
#[test]
fn negative_a_stale_continuation_replayed_after_cancellation_runs_nothing() {
    let mut fixture = fixture();
    let (task, stale) = parked(&mut fixture, "req_a10");
    resume(&mut fixture, &stale, 64, "req_a10_finish");
    assert_eq!(entry(&fixture, &task).status, TaskStatus::Completed);
    let publications = entry(&fixture, &task).publications();

    cancel(&mut fixture, &task, "req_a10_cancel");
    let opened = regions(&fixture).opened();

    for round in 0..3_u32 {
        let replayed = resume_with(
            &mut fixture,
            &stale,
            None,
            &format!("req_a10_replay_{round}"),
        );
        assert_eq!(
            replayed.envelope.status,
            ResultStatus::Ok,
            "round {round}: the replay is answered rather than faulted"
        );
        assert_eq!(
            entry(&fixture, &task).status,
            TaskStatus::Completed,
            "round {round}: a terminal status never changes"
        );
        assert_eq!(
            entry(&fixture, &task).publications(),
            publications,
            "round {round}: a replayed stale continuation published something"
        );
        assert_eq!(
            regions(&fixture).opened(),
            opened,
            "round {round}: a replayed stale continuation opened a scope, so it ran work"
        );
        assert_total(&fixture, "after a stale replay");
    }
}

// --- A11: every ordering of a hostile suffix --------------------------------------------------

/// One operation in the hostile suffix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Hostile {
    /// `task.cancel`.
    Cancel,
    /// `task.resume` under a ceiling that closes the campaign.
    ResumeWide,
    /// `task.resume` under a ceiling below recorded spend.
    ResumeNarrow,
    /// `task.update_budget` lowering the ceiling below recorded spend.
    LowerBudget,
    /// `task.update_budget` raising it.
    RaiseBudget,
}

impl Hostile {
    const ALL: [Self; 5] = [
        Self::Cancel,
        Self::ResumeWide,
        Self::ResumeNarrow,
        Self::LowerBudget,
        Self::RaiseBudget,
    ];

    const fn token(self) -> &'static str {
        match self {
            Self::Cancel => "cancel",
            Self::ResumeWide => "resume-wide",
            Self::ResumeNarrow => "resume-narrow",
            Self::LowerBudget => "lower",
            Self::RaiseBudget => "raise",
        }
    }
}

/// Every ordering of `Hostile::ALL`, in a fixed order. `5! = 120`, built by insertion so the
/// construction is linear in the output and nothing is doubled.
fn orderings() -> Vec<Vec<Hostile>> {
    let mut permutations: Vec<Vec<Hostile>> = vec![Vec::new()];
    for operation in Hostile::ALL {
        let mut next = Vec::with_capacity(permutations.len() * (permutations[0].len() + 1));
        for permutation in &permutations {
            for position in 0..=permutation.len() {
                let mut candidate = permutation.clone();
                candidate.insert(position, operation);
                next.push(candidate);
            }
        }
        permutations = next;
    }
    permutations.sort_unstable();
    permutations.dedup();
    permutations
}

/// Run one hostile ordering against a task parked with committed partial evidence, and return
/// the canonical rendering of what the daemon looks like afterwards.
fn run_ordering(program: &[Hostile]) -> String {
    let mut fixture = fixture();
    let (task, first_continuation) = parked(&mut fixture, "req_a11");
    for (index, operation) in program.iter().enumerate() {
        let request = format!("req_a11_{index}");
        match operation {
            Hostile::Cancel => {
                let outcome = cancel(&mut fixture, &task, &request);
                assert_eq!(
                    outcome.envelope.status,
                    ResultStatus::Ok,
                    "{}: a cancel is always answered",
                    operation.token()
                );
                // The both-or-neither, on every step of every ordering.
                let carried = cancelled_continuation(&outcome) != Nullable::Null;
                assert_eq!(
                    carried,
                    !outcome.envelope.artifacts.is_empty()
                        && entry(&fixture, &task).continuation.is_some(),
                    "{}: the cancel's two halves disagree",
                    operation.token()
                );
            }
            Hostile::ResumeWide | Hostile::ResumeNarrow => {
                // The continuation the task holds *now* when it holds one, and the first one
                // it ever minted otherwise — which is the stale-handle case, on purpose.
                let continuation = entry(&fixture, &task)
                    .continuation
                    .clone()
                    .unwrap_or_else(|| first_continuation.clone());
                let states = if *operation == Hostile::ResumeWide {
                    64
                } else {
                    1
                };
                let outcome = resume(&mut fixture, &continuation, states, &request);
                assert!(
                    outcome.error_code().is_none(),
                    "{}: the resume was refused — {:?}",
                    operation.token(),
                    outcome.envelope.error
                );
            }
            Hostile::LowerBudget => {
                let outcome = rebudget(&mut fixture, &task, 1, &request);
                assert_eq!(outcome.envelope.status, ResultStatus::Ok);
            }
            Hostile::RaiseBudget => {
                let outcome = rebudget(&mut fixture, &task, 64, &request);
                assert_eq!(outcome.envelope.status, ResultStatus::Ok);
            }
        }
        assert_total(
            &fixture,
            &format!("after {} in the ordering", operation.token()),
        );
        // Nothing is ever staged at rest: a publication in flight when a dispatch returns is
        // the "neither committed nor absent" state the pass condition forbids.
        assert!(
            !entry(&fixture, &task).evidence.is_staging(),
            "{}: a publication was left staged across a dispatch boundary",
            operation.token()
        );
    }

    let final_record = record(&read(&mut fixture, &task, "req_a11_read"));
    format!(
        "program={}\nstatus={:?} publications={} continuation={}\n{}",
        program
            .iter()
            .map(|operation| operation.token())
            .collect::<Vec<_>>()
            .join(","),
        final_record.status,
        entry(&fixture, &task).publications(),
        present(&final_record.continuation),
        regions(&fixture).render()
    )
}

/// A11 — all 120 orderings of a five-operation hostile suffix leave a total daemon, with no
/// publication staged across any dispatch boundary.
///
/// The constructive sweep drives each lane's own *intended* program. This drives every
/// ordering of the five operations a hostile client has, including the ones that make no
/// sense — resume after cancel, cancel between two budget updates, lower a ceiling on a dead
/// task — and holds the same property after every single step of every single one: the region
/// tree is total, and nothing is staged at rest.
///
/// 120 programmes × up to 5 dispatches over a 16-state model. Bounded and linear.
#[test]
fn negative_every_ordering_of_a_hostile_operation_suffix_leaves_a_total_daemon() {
    let programs = orderings();
    assert_eq!(programs.len(), 120, "5! orderings, deduplicated");
    let mut endings: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for program in &programs {
        let rendering = run_ordering(program);
        assert!(
            rendering.contains("daemon total: orphans=0 unfinalized=0 defects=0 balanced=true"),
            "an ordering left the daemon's own accounting unbalanced:\n{rendering}"
        );
        endings.insert(
            rendering
                .lines()
                .nth(1)
                .expect("the rendering carries a status line")
                .to_owned(),
        );
    }
    assert!(
        endings.len() > 1,
        "every ordering reached the same end state, so the sweep is not exercising the \
         orderings: {endings:?}"
    );
}

// --- A12: the identity after a cancel ---------------------------------------------------------

/// A12 — re-issuing the identical `verification.start` after a cancel returns the cancelled
/// task and runs nothing.
///
/// A `task_*` handle is the content identity of (snapshot, intent, target, portfolio,
/// priority, budget, epochs), so the "same" campaign after a cancel is the *same task*, and
/// `verification.start`'s cached-result lane only fires for a `Completed` one. What a caller
/// gets is therefore the cancelled task back.
///
/// **Recorded as a concern rather than a violation.** No obligation leaks and nothing is
/// half-published — the envelope names the task and `task.status` reports `cancelled`
/// authoritatively, which is what `rule subscription.hints_only` makes the load-bearing
/// reading. But the envelope's lane says `task_started` for a task that will never run again,
/// and a client that cancels a campaign cannot re-ask the identical question of this daemon:
/// it must vary the budget or the target to get a new identity. That is a real property of
/// the design and it is pinned here so a change to it is a change to a test.
#[test]
fn negative_a_cancelled_identity_is_reported_as_cancelled_and_never_re_run() {
    let mut fixture = fixture();
    let (task, _) = parked(&mut fixture, "req_a12");
    cancel(&mut fixture, &task, "req_a12_cancel");
    let opened = regions(&fixture).opened();
    let publications = entry(&fixture, &task).publications();

    // The identical request, under a fresh request identity so nothing is replayed from the
    // idempotency ledger.
    let again = start(&mut fixture, "req_a12_again", 4, all_claims());
    assert!(
        again.error_code().is_none(),
        "the re-start was refused — {:?}",
        again.envelope.error
    );
    assert_eq!(
        started_task(&again),
        task,
        "the identity is content-addressed, so the same question names the same task"
    );
    assert_eq!(
        regions(&fixture).opened(),
        opened,
        "no campaign ran: a cancelled task is not re-entered"
    );
    assert_eq!(
        entry(&fixture, &task).publications(),
        publications,
        "and nothing was published under the closed identity"
    );
    assert_eq!(
        record(&read(&mut fixture, &task, "req_a12_read")).status,
        TaskStatus::Cancelled,
        "task.status is authoritative and still says cancelled"
    );
    assert_total(&fixture, "after re-starting a cancelled identity");
}

// --- A13: determinism -------------------------------------------------------------------------

/// The canonical rendering of the whole attack programme: every profile, cancelled, plus the
/// A1 sequence and the four extreme orderings, with the region ledger after each.
fn render_campaign() -> String {
    let mut out = String::new();
    for profile in PROFILES {
        let mut fixture = fixture();
        let task = (profile.build)(&mut fixture, &format!("req_a13_{}", profile.token));
        let outcome = cancel(&mut fixture, &task, "req_a13_cancel");
        out.push_str(&format!(
            "== {} == continuation={} artifacts={}\n",
            profile.token,
            cancelled_continuation(&outcome) != Nullable::Null,
            outcome.envelope.artifacts.len()
        ));
        out.push_str(&regions(&fixture).render());
    }
    for program in [
        [
            Hostile::Cancel,
            Hostile::ResumeNarrow,
            Hostile::LowerBudget,
            Hostile::RaiseBudget,
            Hostile::ResumeWide,
        ],
        [
            Hostile::ResumeWide,
            Hostile::LowerBudget,
            Hostile::Cancel,
            Hostile::ResumeNarrow,
            Hostile::RaiseBudget,
        ],
        [
            Hostile::LowerBudget,
            Hostile::ResumeNarrow,
            Hostile::RaiseBudget,
            Hostile::ResumeWide,
            Hostile::Cancel,
        ],
    ] {
        out.push_str(&run_ordering(&program));
    }
    out
}

/// A13 — the whole attack programme renders byte-identically on two fresh daemons.
///
/// > identical requests against equal states produce equal results
/// >
/// > — `rule ordering.deterministic`
///
/// Bytes rather than `==` on a collection, because bytes catch an ordering difference that set
/// equality hides. The programme includes the sequence that falsified A1, so the *violation*
/// is deterministic too — which is what makes it a defect to repair rather than a flake to
/// chase.
#[test]
fn negative_the_attack_programme_renders_byte_identically_on_two_fresh_daemons() {
    let first = render_campaign();
    let second = render_campaign();
    assert_eq!(first.as_bytes(), second.as_bytes());
    assert!(!first.is_empty(), "a vacuous render proves nothing");
    for token in [
        "daemon total: orphans=0 unfinalized=0 defects=0 balanced=true",
        "== fresh-park == continuation=true",
        "== failed == continuation=false",
    ] {
        assert!(
            first.contains(token),
            "the rendering must carry the facts it compares — {token} is missing"
        );
    }
    assert!(
        first.lines().count() > 60,
        "a rendering this short is not the campaign:\n{first}"
    );
}
