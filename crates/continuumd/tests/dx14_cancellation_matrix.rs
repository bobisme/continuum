//! **G0-DX-14 at the daemon grain.** The four task shapes as dispatch programs, cancelled
//! after every phase, through `Daemon::dispatch` and nothing else.
//!
//! > | G0-DX-14 | Can cancellation of verification work close correctly? | cancel DPOR,
//! > solver, proof, and synthesis tasks at every phase | no leaked obligations; resumable
//! > artifacts either committed or absent |
//! >
//! > — `notes/plan/notes/G0_SPIKE_MATRIX.md`
//!
//! `crates/continuum-task/tests/dx14_cancellation_matrix.rs` is the campaign over the
//! region+budget calculus, where a phase is a beat of a lane's lifecycle and every one of
//! them is reachable. This file is the other half: the same four shapes at the grain a
//! client can actually reach, where a phase is a **dispatch** and the set of reachable ones
//! is smaller and differently shaped. Both halves are needed and neither substitutes for the
//! other — the calculus half can put a cancellation between a `Reserve` and its `Commit`, and
//! the daemon half is the only one where the answer crosses an API.
//!
//! # What a lane is here, and the honest limit
//!
//! This daemon runs one engine — `continuum-engine-reference`'s breadth-first explorer — so a
//! "solver task" and a "proof task" are not separate engines here and this file does not
//! pretend they are. What *is* real, and what the cancellation rule is a function of, is the
//! **residual profile** a task presents when `task.cancel` reaches it: how much it has
//! published ([`TaskEntry::publications`]) and whether it holds a continuation to resume from.
//! `crates/continuumd/src/daemon/task.rs` says so at the point it reads them —
//! `CancelOutcome` is computed from those two facts and from nothing else — so a dispatch
//! program that reaches a profile has reached everything the rule can see.
//!
//! The four lanes are therefore four dispatch programs with four distinct profiles:
//!
//! | Lane | Program | What it stands for |
//! |---|---|---|
//! | `dpor` | start bounded, resume bounded, resume closed | a frontier committed per round: publications 1 → 2 → 3, resumable at every intermediate step |
//! | `solver` | start closed, read | an atomic query answered whole: one publication, nothing to resume |
//! | `proof` | start bounded, re-budget, resume closed | parked between lemmas and re-admitted under a larger ceiling *without* re-running — the one lane whose middle phase changes the budget and not the work |
//! | `synthesis` | start under a ceiling too small to admit anything, read | a campaign that published nothing and has no continuation: RFC 0026's failure with a `non_resumable_reason` beside it |
//!
//! # Three arms, and why the third is not a violation
//!
//! `rule task.cancel_correct` reads "either committed partial evidence plus a valid
//! continuation, or nothing published", and a reader meeting the `committed-non-resumable`
//! arm for the first time should be told plainly why it is not a counterexample:
//!
//! - the arm is the dossier's own third case, not a fallback. RFC 0026 requires
//!   `non_resumable_reason` "when … no continuation exists", and plan §4.1 states it as a
//!   rule: "a task that genuinely cannot be resumed states so with a typed non-resumable
//!   reason (plan §25, SD-13) rather than losing its evidence quietly";
//! - the pass condition's word is **resumable**: "*resumable* artifacts either committed or
//!   absent". A completed campaign's artifact is committed and is not a resumable artifact;
//!   there is no continuation dangling over it, which is what the conjunct forbids;
//! - the shape the row would be a violation of — committed evidence *plus* a continuation
//!   that no longer resolves, or a continuation with nothing behind it — has **no
//!   constructor** in `CancelOutcome`, and this file asserts the wire's nullable
//!   `continuation` is present exactly on the arm that carries one, on every row.
//!
//! # The other honest limit: no cancellation lands mid-publication here
//!
//! `Daemon::dispatch` holds `&mut DaemonState` for a whole call, so when `task.cancel` runs
//! there is no live execution to interrupt: the dispatch that ran the campaign finalized its
//! own region before it returned. What this file therefore proves at this grain is that the
//! *residual* teardown is correct at every phase, and what it cannot prove here is the
//! in-flight case. That case is proved one crate down, exhaustively, over every phase of
//! every lane — `crates/continuum-task/tests/dx14_cancellation_matrix.rs`'s `frontier-staged`,
//! `answer-staged`, `lemma-staged` and `next-candidate-staged` rows are exactly it. Saying
//! which half proves which is the point of splitting them.
//!
//! # Evidence map
//!
//! | Claim | Test |
//! |---|---|
//! | every phase of every lane produces its declared answer, both halves agreeing | [`positive_the_daemon_cancellation_matrix_is_the_declared_table_at_every_phase`] |
//! | cancelling twice is monotone and the second answer says so | [`positive_the_daemon_cancellation_matrix_is_the_declared_table_at_every_phase`] |
//! | a cancelled lane runs nothing further, at any phase | [`positive_no_lane_runs_work_after_it_was_cancelled_at_any_phase`] |
//! | reads and budget updates open no scope | [`positive_only_work_opens_a_scope`] |
//! | a task this daemon does not hold opens no scope at all | [`negative_cancelling_a_task_this_daemon_does_not_hold_opens_no_scope`] |
//! | a dispatch that faulted still finalizes its scope, and its task is cancellable | [`positive_a_run_that_faulted_leaves_a_cancellable_task_and_no_open_scope`] |
//! | four lanes in one daemon leave no orphan work | [`positive_a_four_lane_portfolio_leaves_no_orphan_work_in_one_daemon`] |
//! | the whole matrix renders byte-identically on two fresh daemons | [`positive_the_whole_daemon_matrix_renders_byte_identically_across_two_runs`] |
//!
//! # The harness
//!
//! The fixtures are `daemon_task_regions.rs`'s, duplicated locally rather than imported,
//! because a `tests/*.rs` file is its own crate and nothing here can `use` a sibling one —
//! the same reason `pr8_exit_evidence.rs` gives. The Die Hard model and contract are
//! `include_str!`'d from the one copy of each in this repository, so no fixture here can
//! drift from the corpus.

use continuum_engine_reference::diehard;
use continuum_intent::canonical_json::Json;
use continuum_intent::contract::IntentContract;
use continuum_task::region::RegionState;
use continuum_task::region::worker::{CancelOutcome, WorkerState};
use continuum_value::epoch::ProtocolWindow;
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
use continuumd::protocol::spec::{Nullable, Optional};
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

// --- the four lanes ------------------------------------------------------------------------

/// One dispatch a lane's program makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Beat {
    /// `verification.start` under a state ceiling. Opens one scope.
    Start(u64),
    /// `task.resume` under a ceiling. Opens one scope.
    Resume(u64),
    /// `task.update_budget`: re-admit under a larger bound without re-running. Opens none.
    Rebudget(u64),
    /// `task.status`: a read, which must change nothing. Opens none.
    Read,
}

impl Beat {
    /// Whether this dispatch runs task work, and therefore opens a region.
    fn opens_a_scope(self) -> bool {
        matches!(self, Self::Start(_) | Self::Resume(_))
    }
}

/// One row of the daemon-grain matrix: what `task.cancel` must answer after this many beats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DaemonRow {
    /// What the lane had done when the cancellation arrived.
    phase: &'static str,
    /// The status the task held *before* the cancel.
    found: TaskStatus,
    /// The status the cancel answers with.
    answered: TaskStatus,
    /// The structural verdict the cancel carries.
    outcome: StructuralOutcome,
    /// Whether the wire answer carries a `cont_*` handle.
    resumable: bool,
    /// The arm the cancel's own region scope computed.
    arm: &'static str,
    /// How many publications the task holds. Unchanged by the cancel (INV-009).
    publications: u32,
    /// How many scopes this daemon has opened before the cancel. The cancel adds one.
    opened: u32,
}

/// A task shape as a dispatch program plus one row per phase.
#[derive(Debug)]
struct DaemonLane {
    token: &'static str,
    /// The campaign this lane asks for.
    ///
    /// Distinct per lane, and load-bearing rather than decorative: a `task_*` handle is the
    /// content identity of (snapshot, intent, target, portfolio, priority, budget, epochs),
    /// so two lanes naming one target under one budget would be **one task**, and a portfolio
    /// of four lanes would silently be a portfolio of fewer. All four targets are real
    /// campaigns over the Die Hard model — `all_claims` and `module` are every predicate,
    /// `property` and `claim` name one.
    kind: TargetKind,
    id: &'static str,
    beats: &'static [Beat],
    rows: &'static [DaemonRow],
}

impl DaemonLane {
    fn target(&self) -> Target {
        Target {
            kind: self.kind,
            id: self.id.to_owned(),
        }
    }
}

/// The four shapes G0-DX-14 names, as the residual profiles they present to `task.cancel`.
///
/// See this file's header for what each one stands for and why the profile is the whole of
/// what the rule can see.
const LANES: &[DaemonLane] = &[
    DaemonLane {
        token: "dpor",
        kind: TargetKind::AllClaims,
        id: "DieHard",
        beats: &[Beat::Start(4), Beat::Resume(8), Beat::Resume(64)],
        rows: &[
            DaemonRow {
                phase: "first-frontier-parked",
                found: TaskStatus::Suspended,
                answered: TaskStatus::Cancelled,
                outcome: StructuralOutcome::Cancelled,
                resumable: true,
                arm: "committed-with-continuation",
                publications: 1,
                opened: 1,
            },
            DaemonRow {
                phase: "second-frontier-parked",
                found: TaskStatus::Suspended,
                answered: TaskStatus::Cancelled,
                outcome: StructuralOutcome::Cancelled,
                resumable: true,
                arm: "committed-with-continuation",
                publications: 2,
                opened: 2,
            },
            DaemonRow {
                phase: "exploration-closed",
                found: TaskStatus::Completed,
                answered: TaskStatus::Completed,
                outcome: StructuralOutcome::Unchanged,
                resumable: false,
                arm: "committed-non-resumable",
                publications: 3,
                opened: 3,
            },
        ],
    },
    DaemonLane {
        token: "solver",
        kind: TargetKind::Module,
        id: "DieHard",
        beats: &[Beat::Start(64), Beat::Read],
        rows: &[
            DaemonRow {
                phase: "answer-committed",
                found: TaskStatus::Completed,
                answered: TaskStatus::Completed,
                outcome: StructuralOutcome::Unchanged,
                resumable: false,
                arm: "committed-non-resumable",
                publications: 1,
                opened: 1,
            },
            DaemonRow {
                phase: "answer-read",
                found: TaskStatus::Completed,
                answered: TaskStatus::Completed,
                outcome: StructuralOutcome::Unchanged,
                resumable: false,
                arm: "committed-non-resumable",
                publications: 1,
                opened: 1,
            },
        ],
    },
    DaemonLane {
        token: "proof",
        kind: TargetKind::Property,
        id: "TypeOK",
        beats: &[Beat::Start(4), Beat::Rebudget(64), Beat::Resume(64)],
        rows: &[
            DaemonRow {
                phase: "lemma-parked",
                found: TaskStatus::Suspended,
                answered: TaskStatus::Cancelled,
                outcome: StructuralOutcome::Cancelled,
                resumable: true,
                arm: "committed-with-continuation",
                publications: 1,
                opened: 1,
            },
            DaemonRow {
                phase: "re-budgeted",
                found: TaskStatus::Suspended,
                answered: TaskStatus::Cancelled,
                outcome: StructuralOutcome::Cancelled,
                resumable: true,
                arm: "committed-with-continuation",
                publications: 1,
                opened: 1,
            },
            DaemonRow {
                phase: "checked",
                found: TaskStatus::Completed,
                answered: TaskStatus::Completed,
                outcome: StructuralOutcome::Unchanged,
                resumable: false,
                arm: "committed-non-resumable",
                publications: 2,
                opened: 2,
            },
        ],
    },
    DaemonLane {
        token: "synthesis",
        kind: TargetKind::Claim,
        id: "NotSolved",
        beats: &[Beat::Start(0), Beat::Read],
        rows: &[
            DaemonRow {
                phase: "no-candidate-admissible",
                found: TaskStatus::Failed,
                answered: TaskStatus::Failed,
                outcome: StructuralOutcome::Unchanged,
                resumable: false,
                arm: "nothing-published",
                publications: 0,
                opened: 1,
            },
            DaemonRow {
                phase: "failure-read",
                found: TaskStatus::Failed,
                answered: TaskStatus::Failed,
                outcome: StructuralOutcome::Unchanged,
                resumable: false,
                arm: "nothing-published",
                publications: 0,
                opened: 1,
            },
        ],
    },
];

// --- fixtures ------------------------------------------------------------------------------

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
        client: "continuumd-dx14-matrix-test".to_owned(),
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

// --- the four operations, each parameterised by the request identity -----------------------

/// `verification.start` over the fixture's sealed snapshot.
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

fn resume(
    fixture: &mut Fixture,
    continuation: &ContinuationHandle,
    states: u64,
    request: &str,
) -> OperationOutcome {
    // The idempotency key is derived from the request identity rather than fixed, so a lane
    // may resume twice: `rule idempotency.replay` would answer the second call from the
    // ledger otherwise, and a replayed answer is not the answer this file is asking about.
    fixture.daemon.dispatch(&OperationRequest {
        envelope: budgeted(
            keyed(
                envelope("task.resume", "agent:runner", "cap_runner", request),
                &format!("idem-{request}"),
            ),
            Some(states),
        ),
        arguments: Arguments::TaskResume(TaskResumeRequest {
            continuation: continuation.clone(),
            budget: Optional::Present(budget(Some(states))),
        }),
    })
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
    fixture.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope("task.cancel", "agent:runner", "cap_runner", request),
            &format!("idem-{request}"),
        ),
        arguments: Arguments::TaskCancel(TaskCancelRequest { task: task.clone() }),
    })
}

// --- reading the daemon ---------------------------------------------------------------------

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

fn code(outcome: &OperationOutcome) -> ErrorCode {
    outcome.error_code().expect("an error result")
}

fn structural(outcome: StructuralOutcome) -> Nullable<Verdict> {
    Nullable::Value(Verdict::Structural(StructuralVerdictValue { outcome }))
}

/// The daemon-grain no-orphan property, with the region ledger as the failure message.
fn assert_total(fixture: &Fixture, when: &str) {
    let regions = regions(fixture);
    assert!(
        regions.is_total(),
        "{when}: the daemon's regions are not total\n{}",
        regions.render()
    );
}

/// Run `lane`'s first `phase` beats and return the task they produced.
///
/// The continuation is re-read from the record after every beat rather than remembered: a
/// resumed run that parks again mints a *new* one, and resuming the old handle would be
/// asking a different question.
fn wound_up(fixture: &mut Fixture, lane: &DaemonLane, phase: usize) -> TaskHandle {
    let mut task: Option<TaskHandle> = None;
    for (index, beat) in lane.beats[..phase].iter().enumerate() {
        let request = format!("req_{}_{index}", lane.token);
        match beat {
            Beat::Start(states) => {
                let outcome = start(fixture, &request, *states, lane.target());
                // A bounded run lands on the `task_suspended` lane and a closed one on `ok`;
                // both are successes, and only an `error` envelope is a refusal.
                assert!(
                    outcome.error_code().is_none(),
                    "{}: the start was refused — {:?}",
                    lane.token,
                    outcome.envelope.error
                );
                task = Some(started_task(&outcome));
            }
            Beat::Resume(states) => {
                let handle = task.clone().expect("a resume follows a start");
                let continuation = entry(fixture, &handle)
                    .continuation
                    .clone()
                    .expect("a parked lane holds a continuation");
                let outcome = resume(fixture, &continuation, *states, &request);
                assert!(
                    outcome.error_code().is_none(),
                    "{}: the resume was refused — {:?}",
                    lane.token,
                    outcome.envelope.error
                );
            }
            Beat::Rebudget(states) => {
                let handle = task.clone().expect("a budget update follows a start");
                let outcome = rebudget(fixture, &handle, *states, &request);
                assert_eq!(outcome.envelope.status, ResultStatus::Ok);
                // `task.update_budget` is `@mutation` and NOT `@task_starting`, so however
                // the lane is parked it never lands on the `task_suspended` lane.
            }
            Beat::Read => {
                let handle = task.clone().expect("a read follows a start");
                let outcome = read(fixture, &handle, &request);
                assert_eq!(outcome.envelope.status, ResultStatus::Ok);
            }
        }
    }
    task.expect("every phase of every lane runs at least one beat")
}

/// What one `task.cancel` answered, on both halves.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Answer {
    status: TaskStatus,
    verdict: Nullable<Verdict>,
    continuation: Nullable<ContinuationHandle>,
    arm: Option<&'static str>,
    worker: WorkerState,
    total: bool,
}

/// Cancel through dispatch and read the wire answer and the region the cancel tore down.
fn cancel_answer(fixture: &mut Fixture, task: &TaskHandle, request: &str) -> Answer {
    let before = regions(fixture).finalizations().len();
    let outcome = cancel(fixture, task, request);
    assert_eq!(
        outcome.envelope.status,
        ResultStatus::Ok,
        "the cancel was refused — {:?}",
        outcome.envelope.error
    );
    assert_eq!(
        regions(fixture).finalizations().len(),
        before + 1,
        "a cancel opens exactly one scope and tears it down inside its own dispatch"
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
        .expect("the cancel's scope owns one worker");
    Answer {
        status,
        verdict: outcome.envelope.verdict.clone(),
        continuation,
        arm: worker.cancel_outcome().map(CancelOutcome::token),
        worker: worker.state().clone(),
        total: report.is_total(),
    }
}

// --- the matrix ------------------------------------------------------------------------------

#[test]
fn positive_the_daemon_cancellation_matrix_is_the_declared_table_at_every_phase() {
    let mut rows = 0_usize;
    let mut arms: std::collections::BTreeSet<&'static str> = std::collections::BTreeSet::new();
    for lane in LANES {
        assert_eq!(
            lane.rows.len(),
            lane.beats.len(),
            "{}: one row per dispatch phase",
            lane.token
        );
        for (index, row) in lane.rows.iter().enumerate() {
            let phase = index + 1;
            let context = format!("{}/{}", lane.token, row.phase);
            let mut fixture = fixture();
            let task = wound_up(&mut fixture, lane, phase);

            // What the cancellation is about to find.
            assert_eq!(
                entry(&fixture, &task).status,
                row.found,
                "{context}: the lane did not reach the phase the row names"
            );
            assert_eq!(
                entry(&fixture, &task).publications,
                row.publications,
                "{context}: the lane published a different number of artifacts"
            );
            assert_eq!(
                entry(&fixture, &task).continuation.is_some(),
                row.resumable,
                "{context}: the lane's resumability is not what the row declares"
            );
            assert_eq!(
                regions(&fixture).opened(),
                row.opened,
                "{context}: only work opens a scope, and the row says how much work ran"
            );
            assert_total(&fixture, &format!("{context}: before the cancel"));

            let answer = cancel_answer(&mut fixture, &task, "req_cancel");
            assert_eq!(answer.status, row.answered, "{context}: task status");
            assert_eq!(
                answer.verdict,
                structural(row.outcome),
                "{context}: structural verdict"
            );
            assert_eq!(answer.arm, Some(row.arm), "{context}: the region's arm");
            assert_eq!(
                answer.worker,
                WorkerState::Cancelled,
                "{context}: the cancel's own scope is torn down by a cancellation"
            );
            assert!(
                answer.total,
                "{context}: the cancel's teardown was not total"
            );
            assert_eq!(
                answer.continuation != Nullable::Null,
                row.resumable,
                "{context}: the wire's nullable continuation"
            );

            // `rule task.cancel_correct`, as a property of the row rather than of the two
            // rows somebody remembered: the wire carries a continuation exactly when the
            // region layer put the answer on the arm that has one, and that arm has no
            // constructor without committed evidence.
            assert_eq!(
                answer.continuation != Nullable::Null,
                answer.arm == Some("committed-with-continuation"),
                "{context}: the wire answer and the region's arm are two readings of one fact"
            );
            if answer.arm != Some("nothing-published") {
                assert!(
                    row.publications > 0,
                    "{context}: an arm claiming committed evidence over a task that published \
                     nothing"
                );
            } else {
                assert_eq!(
                    row.publications, 0,
                    "{context}: a task that published something reported nothing-published"
                );
            }

            // INV-009: the cancel published nothing and replaced nothing.
            assert_eq!(
                entry(&fixture, &task).publications,
                row.publications,
                "{context}: the cancel changed what the task had published"
            );
            // The scope the work ran in is finalized, and so is the cancel's own.
            let region = entry(&fixture, &task)
                .region
                .expect("every lane that ran recorded its scope");
            assert_eq!(
                regions(&fixture).tree().state(region),
                Ok(RegionState::Finalized),
                "{context}: the scope the work ran in outlived its dispatch"
            );
            assert_total(&fixture, &format!("{context}: after the cancel"));

            // Cancelling twice is monotone, and the second answer says so.
            let again = cancel_answer(&mut fixture, &task, "req_cancel_2");
            assert_eq!(
                again.status, answer.status,
                "{context}: a terminal status never changes"
            );
            assert_eq!(
                again.verdict,
                structural(StructuralOutcome::Unchanged),
                "{context}: the second cancel changed nothing and says so"
            );
            assert_eq!(again.arm, answer.arm, "{context}: and reaches the same arm");
            assert_total(&fixture, &format!("{context}: after cancelling twice"));

            arms.insert(row.arm);
            rows += 1;
        }
    }
    assert_eq!(rows, 10, "ten dispatch phases across four lanes");
    assert_eq!(
        arms,
        std::collections::BTreeSet::from([
            "committed-non-resumable",
            "committed-with-continuation",
            "nothing-published",
        ]),
        "a matrix that never reached an arm proves nothing about it"
    );
}

#[test]
fn positive_no_lane_runs_work_after_it_was_cancelled_at_any_phase() {
    // The half of "no leaked obligations" a wire client can see: after the cancel, a resume of
    // the continuation the cancel handed back runs nothing. A resume that silently re-ran
    // would be visible here as a second scope even if it produced the same numbers.
    let mut checked = 0_usize;
    for lane in LANES {
        for (index, row) in lane.rows.iter().enumerate() {
            let context = format!("{}/{}", lane.token, row.phase);
            let mut fixture = fixture();
            let task = wound_up(&mut fixture, lane, index + 1);
            let held = entry(&fixture, &task).continuation.clone();

            let answer = cancel_answer(&mut fixture, &task, "req_cancel");
            let opened = regions(&fixture).opened();
            let published = entry(&fixture, &task).publications;

            if let Some(continuation) = held {
                assert_eq!(
                    answer.continuation,
                    Nullable::Value(continuation.clone()),
                    "{context}: the wire continuation is the `cont_*` the task holds"
                );
                let refused = resume(&mut fixture, &continuation, 64, "req_resume");
                assert_eq!(refused.envelope.status, ResultStatus::Ok);
                match &refused.payload {
                    Payload::TaskResume(response) => assert_eq!(
                        response.status,
                        TaskStatus::Cancelled,
                        "{context}: a terminal status never changes"
                    ),
                    other => panic!("expected a task.resume payload, got {other:?}"),
                }
            } else {
                assert_eq!(
                    answer.continuation,
                    Nullable::Null,
                    "{context}: a lane with nothing to resume reports the named null, never an \
                     absent field"
                );
            }
            assert_eq!(
                regions(&fixture).opened(),
                opened,
                "{context}: a refused resume opens no scope, because it runs nothing"
            );
            assert_eq!(
                entry(&fixture, &task).publications,
                published,
                "{context}: and publishes nothing"
            );
            assert_total(&fixture, &format!("{context}: after a refused resume"));
            checked += 1;
        }
    }
    assert_eq!(checked, 10);
}

#[test]
fn positive_only_work_opens_a_scope() {
    // A region is opened for a *unit of work*, so a read and a budget update must open none.
    // Without this, `opened == finalizations().len()` would be a claim about a denominator
    // that grows for reasons unrelated to work.
    for lane in LANES {
        let mut fixture = fixture();
        let _ = wound_up(&mut fixture, lane, lane.beats.len());
        let expected = u32::try_from(
            lane.beats
                .iter()
                .filter(|beat| beat.opens_a_scope())
                .count(),
        )
        .expect("short programs");
        assert_eq!(
            regions(&fixture).opened(),
            expected,
            "{}: one scope per unit of work and no more\n{}",
            lane.token,
            regions(&fixture).render()
        );
        assert_eq!(
            regions(&fixture).finalizations().len(),
            expected as usize,
            "{}: every scope opened was torn down inside its own dispatch",
            lane.token
        );
        assert_total(
            &fixture,
            &format!("{}: after its whole program", lane.token),
        );
    }
}

#[test]
fn positive_a_run_that_faulted_leaves_a_cancellable_task_and_no_open_scope() {
    // The fifth residual profile, and the only one no lane's program reaches: a dispatch that
    // *faults*. `region::scoped` is a bracket over **every** path out of the work, including
    // the [`Fault`] one, so the task the fault strands is `created` — nothing published, no
    // continuation — and cancelling it there is a phase like any other.
    //
    // Without this row the matrix would establish the property only along paths where the
    // operation succeeded, which is the half of an error-handling claim that never fails.
    let mut fixture = fixture();
    let refused = start(
        &mut fixture,
        "req_defect",
        64,
        Target {
            kind: TargetKind::Property,
            id: "NoSuchPredicate".to_owned(),
        },
    );
    assert_eq!(code(&refused), ErrorCode::MalformedRequest);
    assert_eq!(refused.payload, Payload::None, "no partial effect");

    // The scope the faulted run opened was torn down inside the dispatch that opened it.
    assert_eq!(regions(&fixture).opened(), 1);
    assert_eq!(regions(&fixture).finalizations().len(), 1);
    let settled = regions(&fixture)
        .finalizations()
        .last()
        .and_then(|report| report.workers().first())
        .expect("the faulted run's own scope")
        .state()
        .clone();
    assert_eq!(
        settled.status_token(),
        "failed",
        "the region layer carries the daemon's own code rather than a second vocabulary"
    );
    assert_total(&fixture, "after a run that faulted");

    // And the task it stranded is cancellable, on the nothing-published arm.
    let stranded = fixture
        .daemon
        .state()
        .tasks()
        .handles()
        .first()
        .map(|handle| (*handle).clone())
        .expect("the faulted run left its task in the table");
    assert_eq!(entry(&fixture, &stranded).publications, 0);
    assert!(entry(&fixture, &stranded).continuation.is_none());

    let answer = cancel_answer(&mut fixture, &stranded, "req_cancel");
    assert_eq!(answer.status, TaskStatus::Cancelled);
    assert_eq!(answer.verdict, structural(StructuralOutcome::Cancelled));
    assert_eq!(answer.arm, Some("nothing-published"));
    assert_eq!(answer.continuation, Nullable::Null);
    assert_eq!(answer.worker, WorkerState::Cancelled);
    assert!(answer.total);
    assert_eq!(
        regions(&fixture).opened(),
        2,
        "the faulted run's scope and the cancel's own, and nothing else"
    );
    assert_total(&fixture, "after cancelling a task whose run faulted");
}

#[test]
fn negative_cancelling_a_task_this_daemon_does_not_hold_opens_no_scope() {
    // Phase zero of every lane: the cancellation arrives before the task exists. Denial
    // precedes semantic work (RFC 0027 X3), and a scope that is never opened is the strongest
    // form of that.
    let mut fixture = fixture();
    let opened = regions(&fixture).opened();
    assert_eq!(opened, 0, "no work has run yet");
    let unknown = TaskHandle::new("task_nothing").expect("a handle");
    let denied = cancel(&mut fixture, &unknown, "req_cancel");
    assert_eq!(code(&denied), ErrorCode::CapabilityDenied);
    assert_eq!(
        regions(&fixture).opened(),
        opened,
        "a denied cancel names no task, so there is no work to tear down"
    );
    assert!(
        regions(&fixture).finalizations().is_empty(),
        "and nothing was torn down"
    );
    assert_total(&fixture, "after a denied cancel");
}

// --- the four lanes in one daemon ------------------------------------------------------------

/// Run every lane's whole program in one daemon, cancelling each lane at the end.
fn portfolio(fixture: &mut Fixture) -> Vec<TaskHandle> {
    let mut tasks = Vec::new();
    for lane in LANES {
        let task = wound_up(fixture, lane, lane.beats.len());
        let _ = cancel_answer(fixture, &task, &format!("req_cancel_{}", lane.token));
        tasks.push(task);
    }
    // One lane left parked and never resumed or cancelled — the row the rejected design could
    // not have torn down: if a parked continuation were a live worker, this task would hold an
    // open region for the daemon's lifetime.
    let abandoned = started_task(&start(
        fixture,
        "req_abandoned",
        8,
        Target {
            kind: TargetKind::Property,
            id: "NotSolved".to_owned(),
        },
    ));
    tasks.push(abandoned);
    tasks
}

#[test]
fn positive_a_four_lane_portfolio_leaves_no_orphan_work_in_one_daemon() {
    let mut fixture = fixture();
    let tasks = portfolio(&mut fixture);
    let regions = regions(&fixture);
    let tree = regions.tree();

    assert!(
        regions.opened() >= 9,
        "a portfolio over fewer than nine units of work is not a portfolio:\n{}",
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
    // 3. the obligation ledger is balanced, and not vacuously so.
    assert!(tree.ledger().is_balanced());
    assert!(tree.ledger().opened() > 0);
    assert_eq!(tree.ledger().opened(), tree.ledger().discharged());
    // 4. every teardown was itself total, and the wiring was never refused a step.
    assert!(
        regions
            .finalizations()
            .iter()
            .all(|report| report.is_total())
    );
    assert!(
        regions.defects().is_empty(),
        "the region layer refused one of this daemon's own steps: {:?}",
        regions.defects()
    );

    // 5. the property at the task grain, including the lane left parked.
    let mut parked = 0_u32;
    for task in &tasks {
        let entry = entry(&fixture, task);
        let worker = entry
            .worker
            .expect("every lane that ran recorded its worker");
        let state = tree.worker_state(worker).expect("the tree holds it");
        assert!(
            state.is_terminal(),
            "{}: worker {worker} is {state}, which is not terminal",
            task.as_str()
        );
        if entry.status == TaskStatus::Suspended {
            parked += 1;
            assert!(
                entry.continuation.is_some(),
                "a suspended task is resumable by definition"
            );
            assert_eq!(
                tree.state(entry.region.expect("it ran somewhere")),
                Ok(RegionState::Finalized),
                "the scope a still-parked lane parked in is finalized, and the lane is still \
                 resumable: a parked continuation is not a live worker"
            );
        }
    }
    assert!(
        parked > 0,
        "a portfolio with no parked lane never tests the case the resolution is about"
    );
    assert_total(&fixture, "after the four-lane portfolio");
}

// --- determinism -------------------------------------------------------------------------------

/// A canonical rendering of the whole daemon matrix: every lane, every phase, the region
/// ledger the cancel left behind.
fn render_matrix() -> String {
    let mut out = String::new();
    for lane in LANES {
        for (index, row) in lane.rows.iter().enumerate() {
            out.push_str(&format!("== {} {} ==\n", lane.token, row.phase));
            let mut fixture = fixture();
            let task = wound_up(&mut fixture, lane, index + 1);
            let answer = cancel_answer(&mut fixture, &task, "req_cancel");
            out.push_str(&format!(
                "answer status={:?} arm={} continuation={}\n",
                answer.status,
                answer.arm.unwrap_or("-"),
                answer.continuation != Nullable::Null
            ));
            out.push_str(&regions(&fixture).render());
        }
    }
    out
}

#[test]
fn positive_the_whole_daemon_matrix_renders_byte_identically_across_two_runs() {
    // > identical requests against equal states produce equal results
    // >
    // > — `rule ordering.deterministic`
    //
    // Bytes rather than `==` on a collection, because bytes catch an ordering difference that
    // set equality hides.
    let first = render_matrix();
    let second = render_matrix();
    assert_eq!(first.as_bytes(), second.as_bytes());
    assert!(!first.is_empty(), "a vacuous render proves nothing");
    for token in [
        "committed-with-continuation",
        "committed-non-resumable",
        "nothing-published",
        "daemon total: orphans=0 unfinalized=0 defects=0 balanced=true",
    ] {
        assert!(
            first.contains(token),
            "the rendering must carry the facts it is comparing — {token} is missing"
        );
    }
    assert!(
        first.lines().count() > 60,
        "a rendering this short is not the matrix:\n{first}"
    );
}
