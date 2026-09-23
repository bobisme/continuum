//! Dedicated exit evidence for `INV-009` (`notes/plan/plan.md:339`, the invariant bone
//! `bn-2lq8`).
//!
//! > `INV-009` — Monotonic task evidence
//! >
//! > Resuming a task may add evidence or refine an unknown; it may not silently replace
//! > prior artifacts under the same identity.
//! >
//! > — `notes/plan/plan.md:339-341`
//!
//! The same sentence is normative in four sibling texts, each a different facet:
//! RFC 0026's "Resume is monotone. Resume MAY add evidence; it MUST NOT replace prior
//! artifacts under the same identity (INV-009). The frontier after resume MUST include the
//! frontier before it"; `rule task.update_budget`'s "Silent truncation of a campaign is
//! prohibited (INV-009)"; `rule task.cancel_correct`'s "committed partial evidence plus a
//! valid continuation, or nothing published (INV-009, plan B19)"; and RFC 0026 §"Capability
//! model"'s "Revocation […] MUST NOT retroactively unmake a result the capability lawfully
//! produced (INV-009)".
//!
//! # The prior art this file is built on, not around
//!
//! **bn-23j7s found and fixed a real INV-009 defect.** Before the budget ledger, a
//! `task.update_budget` that lowered a ceiling below committed spend became the engine
//! bound directly: the next `task.resume` re-ran the campaign *smaller*, and a monotone
//! `cost.states` went **down** — silent truncation arriving as a rewrite of recorded
//! results. The fix is [`budget::bounds_of`](continuumd::daemon::budget::bounds_of)
//! flooring the engine's state bound at recorded spend (`limit.max(committed)`,
//! `src/daemon/budget.rs`), so a withdrawn ceiling is *recorded and is not a bound*. This
//! file cites that fix and its guards and verifies both are live
//! ([`the_bn23j7s_floor_is_live_and_its_guards_still_exist`]); it does not re-derive them.
//!
//! # The facet map: monotonicity facet → live enforcement site → owning suite
//!
//! | facet | enforcement site | owning test |
//! |---|---|---|
//! | resume MAY add evidence, MUST NOT replace prior artifacts | [`Publications`](continuumd::daemon::budget::Publications): `committed` is append-only, no removal path compiles against it | `pr6_impl02_budget_evidence.rs::committed_partial_evidence_is_named_and_the_names_only_grow` (`both.starts_with(&first)`) |
//! | a lowered ceiling never silently rewrites recorded results | `budget::bounds_of` floors the bound at recorded spend; `UpdateOutcome::Suspend` is B18, computed in `daemon::task::update_budget`'s five-arm table | wire guard `pr6_impl02_budget_evidence.rs::lowering_below_committed_spend_parks_with_evidence_and_a_continuation`; unit guard `daemon/budget.rs::tests::a_ceiling_below_committed_spend_is_recorded_and_does_not_shrink_the_walk` |
//! | cost dimensions accumulate; spend is monotone, only headroom refunds | `BudgetLedger::charge` (all-or-nothing, irrevocable), `reserve`/`settle`/`release` refund headroom never spend; `charge_states` charges the delta so spend is the walk's high-water mark | `continuum-task/src/budget.rs` unit suite (`a_charge_that_would_pass_the_ceiling_records_nothing`); `pr6_impl02_budget_evidence.rs::a_resumed_run_charges_the_difference_rather_than_the_sum`; `budget_update.rs::the_update_legality_table_holds_in_every_row` ("an update never rewrites recorded spend", every row) |
//! | "committed spend" = recorded, non-refundable spend (the ratified reading) | [`Suspension`](continuum_task::budget::Suspension)'s doc; `BudgetLedger::update` compares the new ceiling against `spend`, not the last checkpoint | `budget_update.rs::lowering_between_the_checkpoint_and_the_live_spend_still_parks` |
//! | committed-evidence count is monotone across checkpoints | `BudgetLedger::checkpoint` refuses `BudgetFault::CheckpointRegressesCommitted` | `continuum-task/src/budget.rs::tests::committed_evidence_is_monotone_across_checkpoints` |
//! | a terminal status never changes; no ledger write below a terminal check | `TaskEntry::advance` (the only `status` writer) refuses every exit from terminal; `task.resume`'s terminal check sits **above** its ledger write (bn-10093, the DX-03 wave's DEFECT 2 / bn-3p32's A1 / bn-1kp6's attack 18); `update_budget` answers `Unchanged` for a terminal task | `dx03_falsification.rs::regression_resume_refuses_the_terminal_budget_write_update_budget_refuses`; `daemon_task_operations.rs::resuming_a_terminal_task_changes_nothing` |
//! | a re-derived artifact receives a *new* identity, never an overwrite | `budget::publication_commitment` derives identity from the campaign's own canonical bytes (sequence included), through the one identity seam | `pr6_impl02_budget_evidence.rs::two_daemons_name_one_campaigns_publications_identically`; unit `one_campaign_names_one_publication_in_any_process` |
//! | an uncommitted partial is absent, never half-visible | `Publications` is a linear typestate: `stage` → `commit`\|`discard`, no third exit; a staged publication is unobservable | `pr6_impl02_budget_evidence.rs::an_uncommitted_partial_is_absent_never_half_visible` |
//! | cancel leaves committed evidence + continuation, or nothing published (B19) | `task.cancel`'s request → drain → finalize over the region layer | bn-1n6r's `pr6_exit_evidence.rs::positive_cancellation_at_every_instrumented_phase_leaves_one_of_the_two_declared_outcomes` and `::positive_a_cancelled_tasks_continuation_is_valid_by_use_on_a_second_fresh_daemon` |
//! | the frontier after resume includes the frontier before it | breadth-first admission makes the parked explored set a prefix of the resumed one; `verification::run` `debug_assert`s it on the production path | `daemon_task_operations.rs::budget_exhaustion_parks_a_continuation_that_update_budget_and_resume_complete` ("the parked frontier is part of the resumed exploration") |
//! | committed partial evidence survives restart; uncommitted partials do not | bn-3dr's durable substrate + recovery reconciliation | `g1_crash_recovery_evidence.rs::a_parked_tasks_committed_publication_survives_the_crash` and `::an_uncommitted_partial_is_absent_after_recovery` |
//! | the region and budget accountings agree about what was committed | `EvidenceBook::reconcile` — a third independently computed accounting required to agree with the other two | `budget_partial_evidence.rs::the_two_accountings_reconcile_at_every_interleaving` |
//!
//! # What this file adds, and why
//!
//! Everything above is landed and tested; four things were genuinely missing, closed below:
//!
//! 1. **The whole-trace restatement.** Every cited suite asserts monotonicity at the
//!    *endpoints* of one arm. Nothing pins THE INV-009 statement — the committed evidence
//!    list and the recorded spend never decrease **at any readable point** — across one
//!    lifecycle that takes *all five* `task.update_budget` arms.
//!    [`positive_every_readable_point_of_a_full_lifecycle_is_monotone`] is that trace:
//!    park → lower-below-spend (`Suspend` + `Unenforced`) → resume under the withdrawn
//!    ceiling → raise (`Raised`) → tighten (`Tightened`) → re-send (`Unchanged`) → resume
//!    to completion → re-budget the *completed* task (the terminal arm no cited guard
//!    exercises for `Completed`; DEFECT 2's guard is the `Cancelled` case).
//! 2. **The removal-verb sweep.** "Append-only" is claimed in doc comments on four
//!    monotone containers (`Publications::committed`, `TaskEntry::milestones`/`events`/
//!    `committed_evidence`, `BudgetLedger::checkpoints`). The sweep reads the three owning
//!    `src` files and refuses any removal verb applied to them, with a mutant proving the
//!    sweep can fail ([`positive_no_monotone_container_has_a_removal_verb`],
//!    [`negative_the_sweeps_are_not_vacuous`]).
//! 3. **Freshness tripwires on every citation** — the bn-34je/bn-1eqt device: each cited
//!    test is pinned by name (and each load-bearing assertion string by spelling) against
//!    the cited file's tracked text, so a renamed or deleted guard breaks this file rather
//!    than silently orphaning the map ([`cited_wire_guards_still_exist`],
//!    [`cited_ledger_guards_still_exist`],
//!    [`the_bn23j7s_floor_is_live_and_its_guards_still_exist`]).
//! 4. **The honest gaps, pinned as typed absences** (bn-n9a1's rule: a facet with no
//!    producer is *recorded as absent*, never guessed at):
//!    - [`boundary_evidence_graph_handles_have_no_task_producer_yet`] —
//!      `TaskEntry::committed_evidence` is declared append-only and carried on the wire,
//!      but nothing writes it: `verification.rs` initializes it empty and no `push` exists
//!      in the daemon. A task reports the typed omission `task.committed_evidence`
//!      (INV-007) instead of a zero it did not measure. The day the evidence-graph task
//!      wiring lands a producer, this goes red and the map must extend to it.
//!    - [`boundary_daemon_grain_monotonicity_is_enforced_for_the_one_metered_dimension`] —
//!      `budget::METERS` is `MeterSet::STATES_ONLY`, so the daemon-grain spend
//!      monotonicity proven here is about `states`; the other eight dimensions' ledger
//!      monotonicity is unit-proven in `budget_dimensions.rs` and their declared ceilings
//!      are typed omissions at this daemon. A widened meter set turns this red and the
//!      daemon-grain evidence must widen with it.
//!
//! # House rules
//!
//! No `src/` file is touched, in this crate or any other, and no existing test is touched.
//! Everything cited is cited, not re-implemented. Every tracked text is `include_str!`'d
//! once into a `const` and never copied or grown at runtime. Nothing here reads a clock,
//! draws entropy, or depends on iteration order (INV-005): the one timestamp is the
//! fixture's own constant.

use continuum_engine_reference::diehard;
use continuum_intent::canonical_json::Json;
use continuum_intent::contract::IntentContract;
use continuum_task::budget::dimension::{CostDimension, MeterSet};
use continuum_value::epoch::ProtocolWindow;
use continuumd::daemon::budget::METERS;
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
use continuumd::protocol::operations::verification::VerificationStartRequest;
use continuumd::protocol::operations::workspace::WorkspaceCreateRequest;
use continuumd::protocol::registry::ENCODINGS;
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, Commitment, ContinuationHandle, DurationMs, EpochIdentity,
    IntentHandle, Opaque, OperationName, ProtocolVersion, RequestId, TaskHandle, Timestamp,
    WorkspaceHandle,
};
use continuumd::protocol::shared::{SnapshotComponents, SnapshotEpochs, Target};
use continuumd::protocol::spec::{Nullable, Optional};
use continuumd::protocol::vocabulary::{
    AuthorityLevel, Encoding, OmissionReason, Portfolio, ResultStatus, TargetKind, TaskStatus,
};

use continuum_workspace::snapshot::WorkspacePath;

// =====================================================================================
// Tracked text this file reads as data — each read once, never grown
// =====================================================================================

/// The module that holds the bn-23j7s fix (`bounds_of`'s floor) and the `Publications`
/// typestate whose committed list the removal-verb sweep covers.
const DAEMON_BUDGET_SRC: &str = include_str!("../src/daemon/budget.rs");
/// The task table: `TaskEntry`'s three append-only lists and `advance`'s terminal refusal.
const DAEMON_TASK_SRC: &str = include_str!("../src/daemon/task.rs");
/// The one initializer of `TaskEntry::committed_evidence` — the no-producer pin reads it.
const DAEMON_VERIFICATION_SRC: &str = include_str!("../src/daemon/verification.rs");
/// The budget calculus: monotone charge, checkpoint regression refusal, the ratified
/// committed-spend reading, and its own unit suite.
const TASK_BUDGET_SRC: &str = include_str!("../../continuum-task/src/budget.rs");

/// bn-23j7s's wire-grain guards (the B18 regression guard among them).
const PR6_IMPL02_SUITE: &str = include_str!("pr6_impl02_budget_evidence.rs");
/// bn-10093's DEFECT 2 regression guard (terminal check above the ledger write).
const DX03_SUITE: &str = include_str!("dx03_falsification.rs");
/// The frontier-prefix assertion and the terminal-resume no-op.
const TASK_OPERATIONS_SUITE: &str = include_str!("daemon_task_operations.rs");
/// bn-1n6r's cancellation both-or-neither, at every instrumented phase.
const PR6_EXIT_SUITE: &str = include_str!("pr6_exit_evidence.rs");
/// bn-3dr's restart survival of committed partial evidence.
const G1_RECOVERY_SUITE: &str = include_str!("g1_crash_recovery_evidence.rs");
/// The update legality table and the ratified committed-spend reading, unit grain.
const BUDGET_UPDATE_SUITE: &str = include_str!("../../continuum-task/tests/budget_update.rs");
/// The `EvidenceBook` three-accounting reconciliation.
const BUDGET_PARTIAL_SUITE: &str =
    include_str!("../../continuum-task/tests/budget_partial_evidence.rs");
/// The per-dimension exhaustion sweep the meter-set boundary cites.
const BUDGET_DIMENSIONS_SUITE: &str =
    include_str!("../../continuum-task/tests/budget_dimensions.rs");

// =====================================================================================
// fixtures — `pr6_impl02_budget_evidence.rs`'s recipe, duplicated locally because a
// `tests/*.rs` file is its own crate and cannot `use` a sibling one's private helpers
// =====================================================================================

const DIE_HARD_MODEL: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm");
const DIE_HARD_CONFIG: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/default.model.toml");
const DIE_HARD_CONTRACT: &str =
    include_str!("../../continuum-intent/tests/fixtures/die-hard-contract.json");
const MODULE_PATH: &str = "DieHard.ctm";

/// Die Hard's frozen reachable-state count (TV-009).
const FROZEN_STATES: u64 = 16;
/// The state count a bound of four stops the walk at, deterministically.
const PARKED_STATES: u64 = 3;

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
        client: "continuumd-inv009-evidence-test".to_owned(),
        actor: who("service:continuumd"),
        capability: cap("cap_root"),
        features: Optional::Absent,
    };
    negotiate(&[version()], ProtocolWindow::new(3), ENCODINGS, &hello).expect("3.1 is served")
}

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

fn acceptance_bytes() -> Opaque {
    let mut fields: std::collections::BTreeMap<String, Json> = std::collections::BTreeMap::new();
    for (key, value) in [
        ("accepted_by", "human:steward"),
        ("capability", "revise-intent"),
        ("signature", "sig-inv009-probe-v1"),
        ("audit_record", "supplied-by-the-caller-and-overwritten"),
        ("timestamp", "2026-08-01T00:00:00.000Z"),
    ] {
        fields.insert(key.to_owned(), Json::String(value.to_owned()));
    }
    Opaque::from_bytes(Json::Object(fields).to_canonical_bytes())
}

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

fn start(fixture: &mut Fixture, request: &str, key: &str, states: u64) -> OperationOutcome {
    let snapshot = fixture.snapshot.clone();
    let mut request_envelope = keyed(
        envelope("verification.start", "agent:runner", "cap_runner", request),
        key,
    );
    request_envelope.budget = Optional::Present(budget(Some(states)));
    fixture.daemon.dispatch(&OperationRequest {
        envelope: on(request_envelope, &snapshot),
        arguments: Arguments::VerificationStart(VerificationStartRequest {
            target: Target {
                kind: TargetKind::AllClaims,
                id: "DieHard".to_owned(),
            },
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
    // request body says (the body's is the one that re-declares the task's ceilings).
    request_envelope.budget = Optional::Present(declared.value().cloned().unwrap_or(budget(None)));
    fixture.daemon.dispatch(&OperationRequest {
        envelope: request_envelope,
        arguments: Arguments::TaskResume(TaskResumeRequest {
            continuation: continuation.clone(),
            budget: declared,
        }),
    })
}

fn entry<'a>(fixture: &'a Fixture, task: &TaskHandle) -> &'a continuumd::daemon::task::TaskEntry {
    fixture
        .daemon
        .state()
        .tasks()
        .get(task)
        .expect("the daemon holds the task")
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

/// One point of the trace: everything INV-009 says may only grow, read off one
/// `task.status` answer.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Reading {
    label: &'static str,
    cost_states: Option<u64>,
    artifacts: Vec<String>,
    milestones: Vec<String>,
}

fn reading(fixture: &mut Fixture, task: &TaskHandle, label: &'static str) -> Reading {
    let outcome = status(fixture, task, &format!("req_read_{label}"));
    let record = record(&outcome);
    Reading {
        label,
        cost_states: record.cost.states.value().copied(),
        artifacts: commitments(&outcome),
        milestones: record
            .milestones
            .iter()
            .map(|milestone| milestone.name.clone())
            .collect(),
    }
}

/// The monotone relation between two adjacent readings: recorded spend never decreases,
/// and each list is a *prefix* of its successor — grown, never reordered, never replaced.
fn assert_monotone(before: &Reading, after: &Reading) {
    assert!(
        after.cost_states >= before.cost_states,
        "cost.states fell between {:?} and {:?}: {:?} -> {:?}",
        before.label,
        after.label,
        before.cost_states,
        after.cost_states
    );
    assert!(
        after.artifacts.starts_with(&before.artifacts),
        "the committed-artifact list between {:?} and {:?} is not prefix-preserving: \
         {:?} -> {:?}",
        before.label,
        after.label,
        before.artifacts,
        after.artifacts
    );
    assert!(
        after.milestones.starts_with(&before.milestones),
        "the milestone list between {:?} and {:?} is not prefix-preserving: {:?} -> {:?}",
        before.label,
        after.label,
        before.milestones,
        after.milestones
    );
}

// =====================================================================================
// 1. the whole-trace restatement: every readable point of one full lifecycle
// =====================================================================================

/// **Positive.** One task, one trace across all five `task.update_budget` arms and both
/// resume shapes, with the three monotone observables — `cost.states`, the named committed
/// artifacts, the milestones — read after *every* dispatch and required to be
/// non-decreasing and prefix-preserving at every adjacent pair, ending with the terminal
/// arm on a `Completed` task (the case none of the cited terminal guards exercises:
/// DEFECT 2's guard re-budgets a `Cancelled` task).
///
/// The endpoints here are each owned by a cited suite (see the module map); what this test
/// alone holds is the *conjunction over the whole trace* — INV-009's own sentence is about
/// every observation, not about chosen pairs.
#[test]
fn positive_every_readable_point_of_a_full_lifecycle_is_monotone() {
    let mut fixture = fixture();
    let task = started_task(&start(&mut fixture, "req_start", "idem-start", 4));
    let mut trace = vec![reading(&mut fixture, &task, "parked")];

    // The park: three states recorded, one publication committed and named.
    assert_eq!(trace[0].cost_states, Some(PARKED_STATES));
    assert_eq!(trace[0].artifacts.len(), 1);

    // Arm `Suspend` (+ `Unenforced` for `wall_ms` in the same call): lower `states` below
    // the three already recorded, and declare a ceiling nothing meters beside it.
    let mut lowered = budget(Some(2));
    lowered.wall_ms = Optional::Present(DurationMs::new(5_000));
    let suspended = update_budget(&mut fixture, &task, "req_lower", lowered);
    let response = update_response(&suspended);
    assert_eq!(response.status, TaskStatus::Suspended, "B18 parks");
    assert_eq!(
        response.budget.states,
        Optional::Present(2),
        "the withdrawn ceiling is recorded"
    );
    assert!(
        response.continuation.value().is_some(),
        "and the park is resumable"
    );
    assert!(
        suspended
            .envelope
            .omissions
            .iter()
            .any(|omission| omission.subject == "budget.wall_ms"
                && omission.reason == OmissionReason::Unsupported),
        "the `Unenforced` arm names its unmetered ceiling (INV-007)"
    );
    assert_eq!(
        entry(&fixture, &task).bounds().states(),
        usize::try_from(PARKED_STATES).expect("fits"),
        "the bn-23j7s floor: the ceiling of 2 is recorded and is not a bound that \
         un-explores the three-state walk"
    );
    trace.push(reading(&mut fixture, &task, "suspended_below_spend"));

    // Resume under the withdrawn ceiling: no further progress, nothing un-explored,
    // evidence *added* (a new name), nothing replaced.
    let continuation = record(&status(&mut fixture, &task, "req_cont_1"))
        .continuation
        .value()
        .cloned()
        .expect("parked");
    let resumed = resume(
        &mut fixture,
        &continuation,
        "req_resume_1",
        Optional::Absent,
    );
    assert_eq!(
        resumed.envelope.status,
        ResultStatus::TaskSuspended,
        "a withdrawn ceiling admits no further progress: {:?}",
        resumed.envelope.error
    );
    trace.push(reading(&mut fixture, &task, "resumed_under_floor"));
    assert_eq!(
        trace[2].cost_states,
        Some(PARKED_STATES),
        "cost is monotone: the resume neither re-ran the campaign smaller nor re-charged \
         the prefix"
    );
    assert_eq!(
        trace[2].artifacts.len(),
        2,
        "resume MAY add evidence (a second named publication)"
    );

    // Arm `Raised`: 2 -> 64.
    let raised = update_budget(&mut fixture, &task, "req_raise", budget(Some(64)));
    assert_eq!(
        update_response(&raised).budget.states,
        Optional::Present(64)
    );
    assert_eq!(entry(&fixture, &task).bounds().states(), 64);
    trace.push(reading(&mut fixture, &task, "raised"));

    // Arm `Tightened`: 64 -> 63, still above both the recorded spend (3) and the frozen
    // closure (16), so it binds from now on and invalidates nothing.
    let tightened = update_budget(&mut fixture, &task, "req_tighten", budget(Some(63)));
    assert_eq!(
        update_response(&tightened).budget.states,
        Optional::Present(63)
    );
    trace.push(reading(&mut fixture, &task, "tightened"));

    // Arm `Unchanged`: 63 re-sent (INV-002).
    let unchanged = update_budget(&mut fixture, &task, "req_resend", budget(Some(63)));
    assert_eq!(
        update_response(&unchanged).budget,
        update_response(&tightened).budget
    );
    trace.push(reading(&mut fixture, &task, "unchanged"));

    // Resume to completion: the walk closes at the frozen sixteen.
    let continuation = record(&status(&mut fixture, &task, "req_cont_2"))
        .continuation
        .value()
        .cloned()
        .expect("still parked");
    let closed = resume(
        &mut fixture,
        &continuation,
        "req_resume_2",
        Optional::Absent,
    );
    assert_eq!(
        closed.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        closed.envelope.error
    );
    trace.push(reading(&mut fixture, &task, "completed"));
    let completed = trace.last().expect("just pushed").clone();
    assert_eq!(completed.cost_states, Some(FROZEN_STATES));
    assert_eq!(
        completed.artifacts.len(),
        3,
        "the closing run's record is the third named publication"
    );

    // The terminal arm, on a `Completed` task: the budget in force and the recorded cost
    // are historical facts now, and a re-budget rewrites neither.
    let terminal = update_budget(&mut fixture, &task, "req_terminal", budget(Some(1)));
    let response = update_response(&terminal);
    assert_eq!(response.status, TaskStatus::Completed);
    assert_eq!(
        response.budget.states,
        Optional::Present(63),
        "a terminal task keeps the budget it ran under — a ceiling of 1 is not recorded \
         over it"
    );
    trace.push(reading(&mut fixture, &task, "terminal_rebudget"));
    assert_eq!(
        trace.last().expect("just pushed").cost_states,
        Some(FROZEN_STATES),
        "recorded results are never rewritten after terminality"
    );
    assert_eq!(
        trace.last().expect("just pushed").milestones,
        completed.milestones,
        "the terminal arm appends no milestone: nothing about a completed task changes"
    );

    // THE INV-009 statement, over the whole trace rather than chosen endpoints.
    for pair in trace.windows(2) {
        assert_monotone(&pair[0], &pair[1]);
    }

    // And the identities are a version list, never an overwrite: three publications,
    // pairwise distinct — including the two three-state runs, whose bytes differ by
    // commit sequence (RFC 0026's "re-derived artifacts receive new identities").
    let names = &trace.last().expect("non-empty").artifacts;
    assert_eq!(names.len(), 3);
    for (index, left) in names.iter().enumerate() {
        for right in &names[index + 1..] {
            assert_ne!(left, right, "two publications never share an identity");
        }
    }
}

// =====================================================================================
// 2. the removal-verb sweep over the declared append-only containers
// =====================================================================================

/// Whether `source` applies a removal verb to the container field `name` — every mutation
/// a `Vec` offers that could shrink, reorder, or replace committed history.
fn declares_a_removal(source: &str, name: &str) -> bool {
    [
        ".pop(",
        ".remove(",
        ".swap_remove(",
        ".truncate(",
        ".drain(",
        ".retain(",
        ".clear(",
        ".sort(",
        ".insert(",
    ]
    .iter()
    .any(|verb| source.contains(&format!("{name}{verb}")))
        || source.contains(&format!("{name} = "))
}

/// **Positive.** The four wire-visible append-only lists (`Publications::committed`,
/// `TaskEntry::milestones`/`events`/`committed_evidence`) and the ledger's checkpoint list
/// admit no removal verb anywhere in their owning `src` files — "append-only" is a fact of
/// the tracked text, not only of the doc comments that claim it. The `staged` slot is
/// deliberately not in this table: it is the *unobservable* half of the typestate, and
/// `staged.take()` is its two legal exits (commit, discard), both cited in the map.
#[test]
fn positive_no_monotone_container_has_a_removal_verb() {
    let table: &[(&str, &str, &str)] = &[
        ("daemon/budget.rs", DAEMON_BUDGET_SRC, "committed"),
        ("daemon/task.rs", DAEMON_TASK_SRC, "milestones"),
        ("daemon/task.rs", DAEMON_TASK_SRC, "events"),
        ("daemon/task.rs", DAEMON_TASK_SRC, "committed_evidence"),
        ("continuum-task/budget.rs", TASK_BUDGET_SRC, "checkpoints"),
    ];
    for (file, source, field) in table {
        // The sweep reads the receiver spelling every real mutation in these files uses.
        let receiver = format!("self.{field}");
        assert!(
            !declares_a_removal(source, &receiver),
            "{file}: `{field}` is declared append-only, and a removal verb on it would be \
             the silent replacement INV-009 forbids"
        );
        assert!(
            source.contains(&receiver),
            "{file}: the sweep must actually cover `{field}` — an absent field is a \
             vacuous pass"
        );
    }
}

// =====================================================================================
// 3. the bn-23j7s fix, freshness-pinned where it lives
// =====================================================================================

/// Whether `source` floors the engine bound at recorded spend — the shape of the
/// bn-23j7s fix in [`bounds_of`](continuumd::daemon::budget::bounds_of).
fn floors_the_bound_at_recorded_spend(source: &str) -> bool {
    source.contains("limit.max(committed)")
}

/// **Positive.** The fix and both of its guards are live:
///
/// - the floor itself, `limit.max(committed)`, in `daemon/budget.rs`'s `bounds_of`;
/// - the in-module unit guard
///   `a_ceiling_below_committed_spend_is_recorded_and_does_not_shrink_the_walk` and the
///   high-water-mark guard `a_charge_records_the_high_water_mark_rather_than_the_sum`;
/// - the wire-grain regression guard
///   `lowering_below_committed_spend_parks_with_evidence_and_a_continuation`, whose doc
///   states the original defect ("a monotone `cost.states` went **down**") and whose
///   assertions are the fix as behaviour.
#[test]
fn the_bn23j7s_floor_is_live_and_its_guards_still_exist() {
    assert!(
        floors_the_bound_at_recorded_spend(DAEMON_BUDGET_SRC),
        "bounds_of no longer floors the engine bound at recorded spend — the bn-23j7s \
         silent-truncation defect is re-openable"
    );
    for guard in [
        "fn a_ceiling_below_committed_spend_is_recorded_and_does_not_shrink_the_walk",
        "fn a_charge_records_the_high_water_mark_rather_than_the_sum",
    ] {
        assert!(
            DAEMON_BUDGET_SRC.contains(guard),
            "daemon/budget.rs's unit guard was renamed or removed: {guard}"
        );
    }
    for (pin, why) in [
        (
            "fn lowering_below_committed_spend_parks_with_evidence_and_a_continuation",
            "the wire-grain B18 regression guard",
        ),
        (
            "a monotone\n/// `cost.states` went **down**",
            "the defect statement the guard exists for",
        ),
        (
            "the withdrawn ceiling is recorded and is not a bound that un-explores the walk",
            "the floor asserted as wire behaviour",
        ),
        (
            "fn a_resumed_run_charges_the_difference_rather_than_the_sum",
            "spend as the walk's high-water mark, wire grain",
        ),
        (
            "fn committed_partial_evidence_is_named_and_the_names_only_grow",
            "prefix-preserving named evidence across resume",
        ),
        (
            "fn an_uncommitted_partial_is_absent_never_half_visible",
            "the staged half of the typestate is unobservable",
        ),
        (
            "fn two_daemons_name_one_campaigns_publications_identically",
            "identity is content-derived, so replacement is detectable",
        ),
    ] {
        assert!(
            PR6_IMPL02_SUITE.contains(pin),
            "pr6_impl02_budget_evidence.rs no longer contains {why}: {pin:?}"
        );
    }
}

// =====================================================================================
// 4. freshness tripwires on every other citation in the module map
// =====================================================================================

/// The wire-grain citations: terminal ordering, frontier inclusion, cancellation
/// both-or-neither, restart survival. Each cited test is pinned by name, load-bearing
/// assertion strings by spelling, so a renamed or deleted guard breaks this map loudly.
#[test]
fn cited_wire_guards_still_exist() {
    for (file, source, pin) in [
        (
            "dx03_falsification.rs",
            DX03_SUITE,
            "fn regression_resume_refuses_the_terminal_budget_write_update_budget_refuses",
        ),
        (
            "dx03_falsification.rs",
            DX03_SUITE,
            "DEFECT 2 (fixed, bn-10093)",
        ),
        (
            "daemon_task_operations.rs",
            TASK_OPERATIONS_SUITE,
            "fn budget_exhaustion_parks_a_continuation_that_update_budget_and_resume_complete",
        ),
        (
            "daemon_task_operations.rs",
            TASK_OPERATIONS_SUITE,
            "the parked frontier is part of the resumed exploration",
        ),
        (
            "daemon_task_operations.rs",
            TASK_OPERATIONS_SUITE,
            "fn resuming_a_terminal_task_changes_nothing",
        ),
        (
            "pr6_exit_evidence.rs",
            PR6_EXIT_SUITE,
            "fn positive_cancellation_at_every_instrumented_phase_leaves_one_of_the_two_declared_outcomes",
        ),
        (
            "pr6_exit_evidence.rs",
            PR6_EXIT_SUITE,
            "fn positive_a_cancelled_tasks_continuation_is_valid_by_use_on_a_second_fresh_daemon",
        ),
        (
            "g1_crash_recovery_evidence.rs",
            G1_RECOVERY_SUITE,
            "fn a_parked_tasks_committed_publication_survives_the_crash",
        ),
        (
            "g1_crash_recovery_evidence.rs",
            G1_RECOVERY_SUITE,
            "fn an_uncommitted_partial_is_absent_after_recovery",
        ),
    ] {
        assert!(
            source.contains(pin),
            "{file} no longer contains the cited guard: {pin:?}"
        );
    }
    // The terminal-check ordering itself, in the source it protects: `resume`'s own
    // comment marks the write site the fix moved below the check.
    assert!(
        DAEMON_TASK_SRC.contains("this test is above the budget write, not below"),
        "task.rs's terminal-check-above-ledger-write disposition comment is gone — if the \
         ordering moved, DEFECT 2 is re-openable and this map is stale"
    );
}

/// The ledger-grain citations: the legality table with its never-rewrites row, the
/// ratified committed-spend reading, the checkpoint monotonicity fault, the reconciliation
/// suite, and the monotone-spend discipline stated where it is enforced.
#[test]
fn cited_ledger_guards_still_exist() {
    for (file, source, pin) in [
        (
            "budget_update.rs",
            BUDGET_UPDATE_SUITE,
            "fn the_update_legality_table_holds_in_every_row",
        ),
        (
            "budget_update.rs",
            BUDGET_UPDATE_SUITE,
            "an update never rewrites recorded spend",
        ),
        (
            "budget_update.rs",
            BUDGET_UPDATE_SUITE,
            "fn lowering_between_the_checkpoint_and_the_live_spend_still_parks",
        ),
        (
            "continuum-task/src/budget.rs",
            TASK_BUDGET_SRC,
            "recorded, non-refundable spend",
        ),
        (
            "continuum-task/src/budget.rs",
            TASK_BUDGET_SRC,
            "CheckpointRegressesCommitted",
        ),
        (
            "continuum-task/src/budget.rs",
            TASK_BUDGET_SRC,
            "fn committed_evidence_is_monotone_across_checkpoints",
        ),
        (
            "continuum-task/src/budget.rs",
            TASK_BUDGET_SRC,
            "# Spend is monotone; only headroom is refundable",
        ),
        (
            "continuum-task/src/budget.rs",
            TASK_BUDGET_SRC,
            "fn a_charge_that_would_pass_the_ceiling_records_nothing",
        ),
        (
            "budget_partial_evidence.rs",
            BUDGET_PARTIAL_SUITE,
            "fn the_two_accountings_reconcile_at_every_interleaving",
        ),
    ] {
        assert!(
            source.contains(pin),
            "{file} no longer contains the cited guard: {pin:?}"
        );
    }
}

// =====================================================================================
// 5. the honest gaps, pinned as typed absences
// =====================================================================================

/// **Boundary.** `TaskEntry::committed_evidence` — the *evidence-graph* half of "may add
/// evidence" — is declared append-only, carried on every `TaskRecord`, and covered by the
/// removal-verb sweep above; but nothing produces one: the daemon's single initializer
/// writes it empty and no `push` exists in the daemon source. The live half of this test
/// drives a campaign to completion and reads the typed absence off the wire — the list
/// empty and `task.committed_evidence` named as an `unsupported` omission (INV-007),
/// never a zero nobody measured. Today the "may add evidence" clause is discharged by the
/// *publications* list (named campaign records); the day the evidence-graph task wiring
/// lands a producer, the textual half goes red and this map must extend to the new list's
/// monotonicity.
#[test]
fn boundary_evidence_graph_handles_have_no_task_producer_yet() {
    // The textual half: one initializer, no producer.
    assert!(
        DAEMON_VERIFICATION_SRC.contains("committed_evidence: Vec::new()"),
        "verification.rs no longer initializes committed_evidence empty — a producer may \
         have landed; re-examine this boundary"
    );
    for (file, source) in [
        ("daemon/task.rs", DAEMON_TASK_SRC),
        ("daemon/verification.rs", DAEMON_VERIFICATION_SRC),
        ("daemon/budget.rs", DAEMON_BUDGET_SRC),
    ] {
        assert!(
            !source.contains("committed_evidence.push"),
            "{file} now appends to committed_evidence — the evidence-graph producer has \
             landed and INV-009's map must cover it"
        );
    }

    // The live half: the absence is typed on the wire, not silent.
    let mut fixture = fixture();
    let task = started_task(&start(&mut fixture, "req_start", "idem-start", 64));
    let outcome = status(&mut fixture, &task, "req_status");
    let observed = record(&outcome);
    assert_eq!(observed.status, TaskStatus::Completed);
    assert!(
        observed.committed_evidence.is_empty(),
        "no producer writes evidence-graph handles yet"
    );
    assert!(
        outcome
            .envelope
            .omissions
            .iter()
            .any(|omission| omission.subject == "task.committed_evidence"
                && omission.reason == OmissionReason::Unsupported),
        "a task that commits none says so with a typed omission, never silently: {:?}",
        outcome.envelope.omissions
    );
}

/// **Boundary.** Daemon-grain spend monotonicity is enforced for exactly the dimensions
/// the daemon meters, and [`METERS`] is `MeterSet::STATES_ONLY` — one dimension. The
/// other eight accumulate monotonically at the ledger grain
/// (`budget_dimensions.rs::every_dimension_exhausts_at_its_own_ceiling_when_it_is_metered`,
/// pinned here), and a ceiling declared on any of them at this daemon is a typed omission
/// rather than a silently-unenforced bound. The pin is equality on the meter set itself:
/// the day a deployment grows a clock or a byte meter, this goes red and the daemon-grain
/// monotonic evidence (the whole-trace test above included) must widen to the new
/// dimension rather than silently claiming it.
#[test]
fn boundary_daemon_grain_monotonicity_is_enforced_for_the_one_metered_dimension() {
    assert_eq!(
        METERS,
        MeterSet::STATES_ONLY,
        "the daemon's meter set widened — extend INV-009's daemon-grain evidence to the \
         newly metered dimension before re-pinning this"
    );
    assert_eq!(
        METERS.metered(),
        vec![CostDimension::States],
        "one meter, the engine's own state count"
    );
    assert!(
        BUDGET_DIMENSIONS_SUITE
            .contains("fn every_dimension_exhausts_at_its_own_ceiling_when_it_is_metered"),
        "budget_dimensions.rs no longer proves per-dimension ledger accumulation — the \
         eight-dimension half of this boundary is uncited"
    );
}

// =====================================================================================
// 6. the sweeps can fail
// =====================================================================================

/// **Negative.** Each textual sweep above detects the mutant it exists for: a removal verb
/// on a committed list, a producer appearing for `committed_evidence`, and a `bounds_of`
/// that takes the ceiling without the floor — the exact pre-bn-23j7s shape.
#[test]
fn negative_the_sweeps_are_not_vacuous() {
    assert!(
        declares_a_removal("self.committed.pop();", "self.committed"),
        "a mutant that pops committed evidence must be detected"
    );
    assert!(
        declares_a_removal("self.milestones.truncate(1);", "self.milestones"),
        "a mutant that truncates milestones must be detected"
    );
    assert!(
        declares_a_removal("self.committed = Vec::new();", "self.committed"),
        "a mutant that replaces the whole list must be detected"
    );
    assert!(
        !declares_a_removal("self.committed.push(publication);", "self.committed"),
        "the one legal verb is not flagged"
    );

    let mutant = "self.committed_evidence.push(handle);";
    assert!(
        mutant.contains("committed_evidence.push"),
        "the no-producer sweep's needle matches the producer it waits for"
    );

    // The pre-fix `bounds_of`: the lowered ceiling becomes the bound directly, with no
    // floor at recorded spend — the mutant that re-opens the silent truncation.
    let mutant = "Bounds::CERTIFIABLE.with_states(usize::try_from(limit).unwrap_or(usize::MAX))";
    assert!(
        !floors_the_bound_at_recorded_spend(mutant),
        "the floor check must refuse the pre-bn-23j7s shape, not pass it vacuously"
    );
}
