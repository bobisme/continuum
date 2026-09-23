//! Dedicated exit evidence for `PR-6-EXIT` (`notes/plan/notes/PLAN_REQUIREMENTS.json`, id
//! `PR-6-EXIT`; `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 6's Exit line).
//!
//! > **Exit:** cancellation at every instrumented phase leaves either a valid continuation
//! > or no published partial artifact.
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 6
//!
//! This file asserts that sentence clause by clause, as directly as it reads, through
//! [`Daemon::dispatch`] and nothing else — the way `pr8_exit_evidence.rs` did for PR 8's exit
//! and `pr6_impl02_budget_evidence.rs` did for PR 6's second bullet. It is the **readable**
//! statement of the exit: one sample per equivalence class, every assertion mechanical, and
//! the exhaustive half cited rather than re-run.
//!
//! # Why this file samples and does not sweep
//!
//! The exhaustive campaigns already exist and are green, and re-running them here would buy
//! nothing but minutes:
//!
//! - `crates/continuum-task/tests/dx14_cancellation_matrix.rs` (bn-2zy) — **5,472 runs**: 324
//!   matrix runs over 27 instrumented calculus phases × 4 placements × 3 teardown arrivals, a
//!   108-run request window in which work keeps stepping past the request, and a four-lane
//!   portfolio over all 630 interleavings of its suffixes with the cancellation completed at
//!   each of 8 positions (5,040), beside a 27-row check that a cancelled scope admits no
//!   further worker or sub-scope;
//! - `crates/continuum-task/tests/region_no_orphan.rs` (bn-2gk) — 3,570 teardowns over 210
//!   interleavings of three worker programs;
//! - `crates/continuum-task/tests/budget_partial_evidence.rs` (bn-1gc) — 1,680 runs, each
//!   asserting `EvidenceBook::reconcile`'s independent budget-side accounting;
//! - `crates/continuumd/tests/dx14_cancellation_matrix.rs` (bn-2zy) — the same four shapes as
//!   dispatch programs: the 10 lane phases, the faulted dispatch and the unheld task;
//! - `crates/continuumd/tests/dx14_falsification.rs` +
//!   `crates/continuum-task/tests/dx14_falsification_calculus.rs` (bn-3p32) — 18 adversarial
//!   attacks against the two conjuncts;
//! - `crates/continuumd/tests/dx03_falsification.rs` (bn-1kp6) — 27 tests, 22 of them attacks,
//!   over idempotence, stale-snapshot rejection, deterministic resume and handle reuse.
//!
//! Those are the exhaustive half. This file is the other half: the sentence itself, held over
//! the **wire-reachable instrumented phase set** — every phase a client can actually drive
//! through `Daemon::dispatch` — with one run per phase and the disjunction computed from the
//! wire answer by a function that has a **`None` branch a hostile input reaches**
//! ([`exit_side`]), so the classification is a claim rather than a tautology.
//!
//! # The sentence, decomposed
//!
//! | Clause | Test |
//! |---|---|
//! | **"cancellation at every instrumented phase"** — all twelve wire-reachable phases: the four lanes' ten dispatch phases, the faulted dispatch, and phase zero (the task this daemon does not hold) | [`positive_cancellation_at_every_instrumented_phase_leaves_one_of_the_two_declared_outcomes`] |
//! | **"leaves either a valid continuation"** — validity proven by **use**: the `cont_*` a cancel handed back resumes to Die Hard's frozen 16 states on a second fresh daemon, and the handle is byte-identical there | [`positive_a_cancelled_tasks_continuation_is_valid_by_use_on_a_second_fresh_daemon`] |
//! | **"or no published partial artifact"** — the two dispositions that reach the right-hand side, with `ResultEnvelope.artifacts` (bn-23j7s's *named* publications) checked against the region layer's own committed count, and against the task table's | [`positive_a_cancel_with_no_continuation_leaves_no_published_partial_artifact`] |
//! | the arm that has **no constructor** — "committed evidence and no continuation" and "a continuation and nothing published" are both unreachable, asserted at the wire over every phase | [`positive_a_cancel_with_no_continuation_leaves_no_published_partial_artifact`] |
//! | **closure** — post-cancel the record is closed, and the exit disjunction is stable under the hostile suffix that falsified it | [`regression_the_cancelled_record_is_closed_and_the_exit_side_does_not_move`] |
//! | **anti-vacuity** — the phases exercise more than one behaviour class, and the classifier rejects the shape the sentence forbids | [`positive_the_exit_phases_exercise_more_than_one_behaviour_class`] |
//!
//! # How the disjunction is read, and why the third arm is the right-hand side
//!
//! `rule task.cancel_correct` has three arms, and a reader meeting `committed-non-resumable`
//! for the first time should be told plainly why it satisfies the exit sentence rather than
//! refuting it. The sentence's word is **partial**:
//!
//! - `committed-with-continuation` — committed partial evidence *plus* a `cont_*`. The
//!   left-hand side, and the continuation is proved valid by use below rather than by being
//!   well-formed.
//! - `nothing-published` — no artifact a reader can observe. The right-hand side, trivially.
//! - `committed-non-resumable` — the campaign **closed**. What it published is a whole
//!   answer, not a partial one; there is no continuation dangling over it, which is what the
//!   conjunct forbids, and RFC 0026 names this case separately and requires it to carry a
//!   typed reason rather than degrade to silence. The right-hand side, and the reason is
//!   asserted rather than assumed.
//!
//! The shape that would be a falsification — **a published partial artifact with nothing to
//! resume it from**: artifacts named, no continuation, and a campaign that did not close — is
//! exactly the `None` branch of [`exit_side`]. It has no constructor in
//! [`CancelOutcome`](continuum_task::region::worker::CancelOutcome) either, and
//! [`positive_the_exit_phases_exercise_more_than_one_behaviour_class`] feeds that shape to the
//! classifier, built out of a *real* artifact list, to prove the branch is reachable by an
//! input and therefore load-bearing.
//!
//! # What this file does not claim
//!
//! - **Durability.** Nothing in this workspace writes a task table to disk. The persistence
//!   machinery is bn-3dr's (daemon crash recovery). What is claimed here is the identity and
//!   the wire surface: two fresh daemons that ran one campaign name its continuation and its
//!   publications identically, so a name is something a store could be asked to return —
//!   which `pr6_impl02_budget_evidence.rs`'s
//!   `two_daemons_name_one_campaigns_publications_identically` already asserts for the
//!   publications and this file asserts for the continuation, by using it.
//! - **Cancellation landing mid-publication, at the wire.** [`Daemon::dispatch`] holds
//!   `&mut DaemonState` for a whole call, so when `task.cancel` runs there is no live
//!   execution to interrupt: the dispatch that ran the campaign finalized its own region
//!   before it returned. The in-flight case is proved one crate down, exhaustively, by
//!   `crates/continuum-task/tests/dx14_cancellation_matrix.rs`'s `frontier-staged`,
//!   `answer-staged`, `lemma-staged` and `next-candidate-staged` rows. A wire-grain
//!   mid-publication cancellation opens with the asupersync adapter in Phase B (PR 14).
//! - **Concurrency.** One thread, one request at a time — `Daemon::dispatch` takes
//!   `&mut self`, which is `continuumd`'s own documented grain.
//!
//! # The harness
//!
//! The fixtures are `dx14_cancellation_matrix.rs`'s, duplicated locally rather than imported,
//! because a `tests/*.rs` file is its own crate and nothing here can `use` a sibling one — the
//! same reason `pr8_exit_evidence.rs` gives. The Die Hard model and contract are
//! `include_str!`'d from the one copy of each in this repository, so no fixture here can drift
//! from the corpus. No `src/` file is touched and no existing test is edited, weakened or
//! moved.
//!
//! [`Daemon::dispatch`]: continuumd::daemon::Daemon::dispatch

use continuum_engine_reference::diehard;
use continuum_intent::canonical_json::Json;
use continuum_intent::contract::IntentContract;
use continuum_task::region::RegionState;
use continuum_task::region::worker::{CancelOutcome, WorkerState};
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
use continuumd::protocol::envelope::{ArtifactRef, Budget, EpochSet, RequestEnvelope};
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

/// The frozen TV-009 reachable-state count (`SPIKE_REPORT.md:9-12`).
const FROZEN_STATES: u64 = 16;

// --- the exit sentence, as a computed value --------------------------------------------------

/// Which side of the exit sentence's disjunction one cancellation landed on.
///
/// The sentence is a disjunction, so the honest reading of "it held" is *which* disjunct held
/// — recorded per phase, so a phase that quietly stopped reaching its side is a failure rather
/// than a silent re-classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ExitSide {
    /// **"a valid continuation"** — the wire answered a `cont_*`, and committed partial
    /// evidence sits behind it.
    ValidContinuation,
    /// **"no published partial artifact"**, with nothing published at all: no artifact a
    /// reader can observe.
    NothingPublished,
    /// **"no published partial artifact"**, the campaign having closed: what it published is
    /// a whole answer rather than a partial one, and nothing dangles over it.
    ClosedAndComplete,
}

impl ExitSide {
    /// A stable token, for canonical rendering and for failure messages.
    const fn token(self) -> &'static str {
        match self {
            Self::ValidContinuation => "valid-continuation",
            Self::NothingPublished => "nothing-published",
            Self::ClosedAndComplete => "closed-and-complete",
        }
    }
}

/// The exit sentence as a total function of what a client can read off one `task.cancel`
/// answer: the task's answered status, the nullable `continuation`, and
/// `ResultEnvelope.artifacts`.
///
/// [`None`] is the **falsification**: artifacts named, no continuation to resume them from,
/// and a campaign that did not close — a published *partial* artifact stranded with nothing
/// pointing at it. That is the leaked-obligation shape G0-DX-14 exists to catch; it has no
/// constructor in [`CancelOutcome`], and the branch here exists so that the disjunction this
/// file asserts is a claim about the daemon rather than a function that cannot say no.
/// [`positive_the_exit_phases_exercise_more_than_one_behaviour_class`] reaches it with a real
/// artifact list.
///
/// Nothing about the *arm* the region layer chose is consulted: this reads the wire alone, and
/// the tests below then check the two readings agree.
fn exit_side(
    status: TaskStatus,
    continuation: &Nullable<ContinuationHandle>,
    artifacts: &[ArtifactRef],
) -> Option<ExitSide> {
    match (continuation, artifacts.is_empty(), status) {
        (Nullable::Value(_), _, _) => Some(ExitSide::ValidContinuation),
        (Nullable::Null, true, _) => Some(ExitSide::NothingPublished),
        (Nullable::Null, false, TaskStatus::Completed) => Some(ExitSide::ClosedAndComplete),
        (Nullable::Null, false, _) => None,
    }
}

// --- the wire-reachable instrumented phase set -------------------------------------------------

/// One dispatch a lane's program makes.
///
/// `dx14_cancellation_matrix.rs`'s own `Beat`, kept because the phase set this file samples is
/// *that* file's: an exit test that invented its own phases would be evidence for a different
/// sentence.
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

/// One instrumented phase: what the cancellation finds, and which side of the exit sentence
/// it must land on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PhaseRow {
    /// What the lane had done when the cancellation arrived.
    phase: &'static str,
    /// The status the cancel answers with.
    answered: TaskStatus,
    /// The arm the cancel's own region scope computed.
    arm: &'static str,
    /// How many publications the task holds. Unchanged by the cancel (INV-009).
    publications: u32,
    /// The disjunct the exit sentence lands on here.
    side: ExitSide,
}

/// A task shape as a dispatch program plus one row per phase.
#[derive(Debug)]
struct Lane {
    token: &'static str,
    kind: TargetKind,
    id: &'static str,
    beats: &'static [Beat],
    rows: &'static [PhaseRow],
}

impl Lane {
    fn target(&self) -> Target {
        Target {
            kind: self.kind,
            id: self.id.to_owned(),
        }
    }
}

/// The four G0-DX-14 shapes as the residual profiles they present to `task.cancel`, and the
/// exit disjunct each phase must reach.
///
/// The table is `dx14_cancellation_matrix.rs`'s, projected onto the columns the exit sentence
/// is about. Ten phases; the eleventh (a dispatch that faulted) and the twelfth (a task this
/// daemon does not hold) reach no lane program and are driven directly.
const LANES: &[Lane] = &[
    Lane {
        token: "dpor",
        kind: TargetKind::AllClaims,
        id: "DieHard",
        beats: &[Beat::Start(4), Beat::Resume(8), Beat::Resume(64)],
        rows: &[
            PhaseRow {
                phase: "first-frontier-parked",
                answered: TaskStatus::Cancelled,
                arm: "committed-with-continuation",
                publications: 1,
                side: ExitSide::ValidContinuation,
            },
            PhaseRow {
                phase: "second-frontier-parked",
                answered: TaskStatus::Cancelled,
                arm: "committed-with-continuation",
                publications: 2,
                side: ExitSide::ValidContinuation,
            },
            PhaseRow {
                phase: "exploration-closed",
                answered: TaskStatus::Completed,
                arm: "committed-non-resumable",
                publications: 3,
                side: ExitSide::ClosedAndComplete,
            },
        ],
    },
    Lane {
        token: "solver",
        kind: TargetKind::Module,
        id: "DieHard",
        beats: &[Beat::Start(64), Beat::Read],
        rows: &[
            PhaseRow {
                phase: "answer-committed",
                answered: TaskStatus::Completed,
                arm: "committed-non-resumable",
                publications: 1,
                side: ExitSide::ClosedAndComplete,
            },
            PhaseRow {
                phase: "answer-read",
                answered: TaskStatus::Completed,
                arm: "committed-non-resumable",
                publications: 1,
                side: ExitSide::ClosedAndComplete,
            },
        ],
    },
    Lane {
        token: "proof",
        kind: TargetKind::Property,
        id: "TypeOK",
        beats: &[Beat::Start(4), Beat::Rebudget(64), Beat::Resume(64)],
        rows: &[
            PhaseRow {
                phase: "lemma-parked",
                answered: TaskStatus::Cancelled,
                arm: "committed-with-continuation",
                publications: 1,
                side: ExitSide::ValidContinuation,
            },
            PhaseRow {
                phase: "re-budgeted",
                answered: TaskStatus::Cancelled,
                arm: "committed-with-continuation",
                publications: 1,
                side: ExitSide::ValidContinuation,
            },
            PhaseRow {
                phase: "checked",
                answered: TaskStatus::Completed,
                arm: "committed-non-resumable",
                publications: 2,
                side: ExitSide::ClosedAndComplete,
            },
        ],
    },
    Lane {
        token: "synthesis",
        kind: TargetKind::Claim,
        id: "NotSolved",
        beats: &[Beat::Start(0), Beat::Read],
        rows: &[
            PhaseRow {
                phase: "no-candidate-admissible",
                answered: TaskStatus::Failed,
                arm: "nothing-published",
                publications: 0,
                side: ExitSide::NothingPublished,
            },
            PhaseRow {
                phase: "failure-read",
                answered: TaskStatus::Failed,
                arm: "nothing-published",
                publications: 0,
                side: ExitSide::NothingPublished,
            },
        ],
    },
];

/// The eleventh phase: a dispatch that *faulted*, reached by no lane program.
const FAULTED_ROW: PhaseRow = PhaseRow {
    phase: "run-faulted",
    answered: TaskStatus::Cancelled,
    arm: "nothing-published",
    publications: 0,
    side: ExitSide::NothingPublished,
};

/// How many phases carry a task, and are therefore cancellable at all.
const TASK_BEARING_PHASES: usize = 11;

// --- fixtures ----------------------------------------------------------------------------------
//
// Duplicated from `dx14_cancellation_matrix.rs` rather than imported: a `tests/*.rs` file is
// its own crate and nothing here can `use` a sibling one.

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
        client: "continuumd-pr6-exit-evidence".to_owned(),
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

fn budgeted(mut envelope: RequestEnvelope, states: Option<u64>) -> RequestEnvelope {
    envelope.budget = Optional::Present(budget(states));
    envelope
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

/// `task.resume`, with the request's optional budget under the caller's control: the arm
/// bn-10093's fix is about is the one that carries one.
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
            budget: Optional::Present(budget(states)),
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
    fixture.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope("task.cancel", "agent:runner", "cap_runner", request),
            &format!("idem-{request}"),
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
/// exactly as `transport::Server::answer` splices them into one frame — the device
/// `inv002_no_hidden_state_evidence.rs` and `dx14_falsification.rs` both already use.
fn wire_bytes(outcome: &OperationOutcome) -> Vec<u8> {
    let mut bytes = to_bytes(&outcome.envelope).expect("a result envelope encodes");
    if let Some(payload) = encode_payload(&outcome.payload).expect("a payload encodes") {
        bytes.extend_from_slice(payload.as_bytes());
    }
    bytes
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

/// Everything one `task.cancel` left behind, on both halves of the seam.
#[derive(Debug, Clone)]
struct Left {
    /// The status the wire answered with.
    status: TaskStatus,
    /// The wire's nullable `continuation`.
    continuation: Nullable<ContinuationHandle>,
    /// `ResultEnvelope.artifacts` — bn-23j7s's *named* publications.
    artifacts: Vec<ArtifactRef>,
    /// The arm the region layer's `CancelOutcome` chose.
    arm: &'static str,
    /// The typed `NonResumableReason`, on the arm that carries one.
    reason: Option<String>,
    /// The region layer's own count of what this task committed.
    region_committed: u32,
    /// The task table's count of what this task committed.
    table_committed: u32,
    /// Whether anything was staged and unresolved when the cancel ran (INV-017).
    staging: bool,
    /// The cancel's own teardown was total.
    total: bool,
    /// The worker state the cancel's scope ended in.
    worker: WorkerState,
}

/// Cancel through dispatch, and read what it left on both halves.
fn cancel_and_read(fixture: &mut Fixture, task: &TaskHandle, request: &str) -> Left {
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
    let total = report.is_total();
    let worker_report = report
        .workers()
        .first()
        .expect("the cancel's scope owns one worker");
    let worker = worker_report.state().clone();
    let region_committed = worker_report.evidence().committed();
    let provisional = worker_report.evidence().is_provisional();
    let (arm, reason) = match worker_report
        .cancel_outcome()
        .expect("a cancellation ends its worker with a cancellation outcome")
    {
        outcome @ CancelOutcome::CommittedWithContinuation(_) => (outcome.token(), None),
        outcome @ CancelOutcome::CommittedNonResumable(why) => {
            (outcome.token(), Some(why.as_str().to_owned()))
        }
        outcome @ CancelOutcome::NothingPublished => (outcome.token(), None),
    };
    assert!(
        !provisional,
        "the cancel's own worker left a publication in flight, which INV-017 forbids"
    );
    let table = entry(fixture, task);
    Left {
        status,
        continuation,
        artifacts: outcome.envelope.artifacts.clone(),
        arm,
        reason,
        region_committed,
        table_committed: table.publications(),
        staging: table.evidence.is_staging(),
        total,
        worker,
    }
}

/// Run `lane`'s first `phase` beats and return the task they produced.
///
/// The continuation is re-read from the record after every beat rather than remembered: a
/// resumed run that parks again mints a *new* one, and resuming the old handle would be asking
/// a different question.
fn wound_up(fixture: &mut Fixture, lane: &Lane, phase: usize) -> TaskHandle {
    let mut task: Option<TaskHandle> = None;
    for (index, beat) in lane.beats[..phase].iter().enumerate() {
        let request = format!("req_{}_{index}", lane.token);
        match beat {
            Beat::Start(states) => {
                let outcome = start(fixture, &request, *states, lane.target());
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

/// The eleventh phase, built directly: a `verification.start` whose target names no predicate
/// faults, and `region::scoped` finalizes its scope on the fault path too, stranding a
/// `created` task with nothing published.
fn faulted(fixture: &mut Fixture) -> TaskHandle {
    let refused = start(
        fixture,
        "req_faulted",
        64,
        Target {
            kind: TargetKind::Property,
            id: "NoSuchPredicate".to_owned(),
        },
    );
    assert_eq!(
        refused.error_code(),
        Some(ErrorCode::MalformedRequest),
        "the eleventh phase needs a dispatch that faulted"
    );
    assert_eq!(refused.payload, Payload::None, "no partial effect");
    fixture
        .daemon
        .state()
        .tasks()
        .handles()
        .first()
        .map(|handle| (*handle).clone())
        .expect("the faulted run left its task in the table")
}

/// Every task-bearing instrumented phase, as `(context, fixture, task)` — the sweep the exit
/// tests share, so no test can silently sample a different phase set than another.
fn every_task_bearing_phase() -> Vec<(String, Fixture, TaskHandle, PhaseRow)> {
    let mut phases = Vec::new();
    for lane in LANES {
        assert_eq!(
            lane.rows.len(),
            lane.beats.len(),
            "{}: one row per dispatch phase",
            lane.token
        );
        for (index, row) in lane.rows.iter().enumerate() {
            let mut fixture = fixture();
            let task = wound_up(&mut fixture, lane, index + 1);
            phases.push((format!("{}/{}", lane.token, row.phase), fixture, task, *row));
        }
    }
    let mut fixture = fixture();
    let task = faulted(&mut fixture);
    phases.push((
        format!("faulted/{}", FAULTED_ROW.phase),
        fixture,
        task,
        FAULTED_ROW,
    ));
    assert_eq!(
        phases.len(),
        TASK_BEARING_PHASES,
        "the wire-reachable instrumented phase set is the one `dx14_cancellation_matrix.rs` \
         drives: ten lane phases plus the faulted dispatch"
    );
    phases
}

// --- clause 1: "cancellation at every instrumented phase" --------------------------------------

/// **The exit sentence, at every wire-reachable instrumented phase.**
///
/// Twelve phases: the four lanes' ten dispatch phases, the faulted dispatch, and phase zero —
/// a cancellation that arrives before the task exists. Each is driven once, through
/// [`Daemon::dispatch`] and nothing else, and each must land on one of the sentence's two
/// disjuncts as computed by [`exit_side`] from the wire answer alone.
///
/// The exhaustive half of "every phase" is cited, not re-run: the calculus-grain sweep
/// (`crates/continuum-task/tests/dx14_cancellation_matrix.rs`, 5,472 runs over 27 instrumented
/// phases including the four mid-publication ones no wire cancel can reach) and the
/// daemon-grain matrix (`crates/continuumd/tests/dx14_cancellation_matrix.rs`) between them
/// establish that this table *is* the reachable set and that the outcome is the declared one
/// at every position. What this test adds is the sentence itself, said once per equivalence
/// class, with the disjunction as a value rather than as a reading.
///
/// Three independently computed counts of what the task published are compared on every phase
/// — `ResultEnvelope.artifacts` (the wire), `TaskEntry::publications` (the task table), and
/// the cancel scope's own `EvidenceLedger::committed` (the region layer) — because a bug that
/// fooled one accounting still has two to get past.
///
/// [`Daemon::dispatch`]: continuumd::daemon::Daemon::dispatch
#[test]
fn positive_cancellation_at_every_instrumented_phase_leaves_one_of_the_two_declared_outcomes() {
    let mut sides: std::collections::BTreeSet<&'static str> = std::collections::BTreeSet::new();
    let mut driven = 0_usize;

    for (context, mut fixture, task, row) in every_task_bearing_phase() {
        assert_eq!(
            entry(&fixture, &task).publications(),
            row.publications,
            "{context}: the lane published a different number of artifacts than the row declares"
        );
        assert_total(&fixture, &format!("{context}: before the cancel"));

        let left = cancel_and_read(&mut fixture, &task, "req_cancel");

        // --- the sentence ------------------------------------------------------------------
        let side =
            exit_side(left.status, &left.continuation, &left.artifacts).unwrap_or_else(|| {
                panic!(
                    "{context}: FALSIFICATION — the cancel left a published partial artifact with \
                 no continuation to resume it from: status={:?} artifacts={} arm={}",
                    left.status,
                    left.artifacts.len(),
                    left.arm
                )
            });
        assert_eq!(
            side, row.side,
            "{context}: the cancel landed on a different disjunct than the row declares"
        );

        // --- the two halves of the seam agree ------------------------------------------------
        assert_eq!(left.status, row.answered, "{context}: the answered status");
        assert_eq!(left.arm, row.arm, "{context}: the region layer's arm");
        assert_eq!(
            left.continuation != Nullable::Null,
            left.arm == "committed-with-continuation",
            "{context}: the wire's nullable continuation and the region's arm are two readings \
             of one fact"
        );
        assert_eq!(
            side == ExitSide::ValidContinuation,
            left.arm == "committed-with-continuation",
            "{context}: the left-hand disjunct is the arm that carries a continuation"
        );

        // --- committed or absent, never dangling, counted three ways -------------------------
        assert_eq!(
            u32::try_from(left.artifacts.len()).expect("short publication lists"),
            row.publications,
            "{context}: the wire named a different number of artifacts than the task published"
        );
        assert_eq!(
            left.table_committed, row.publications,
            "{context}: the cancel changed what the task had published (INV-009)"
        );
        assert_eq!(
            left.region_committed, row.publications,
            "{context}: the region layer and the task table disagree about the committed count"
        );
        assert!(
            !left.staging,
            "{context}: a publication was staged and unresolved — uncommitted partials must be \
             absent, never half-visible"
        );
        for artifact in &left.artifacts {
            assert_eq!(artifact.kind, "task", "{context}: the artifact class");
            assert_eq!(
                artifact.handle.as_str(),
                task.as_str(),
                "{context}: every named publication belongs to this task"
            );
            assert!(
                !artifact.commitment.is_absent(),
                "{context}: a publication is *named* by a content identity (bn-23j7s), never \
                 counted"
            );
        }

        // --- and the teardown left nothing behind --------------------------------------------
        assert_eq!(
            left.worker,
            WorkerState::Cancelled,
            "{context}: the cancel's own scope is torn down by a cancellation"
        );
        assert!(left.total, "{context}: the cancel's teardown was not total");
        let region = entry(&fixture, &task)
            .region
            .expect("every phase that ran work recorded its scope");
        assert_eq!(
            regions(&fixture).tree().state(region),
            Ok(RegionState::Finalized),
            "{context}: the scope the work ran in outlived its dispatch"
        );
        assert_total(&fixture, &format!("{context}: after the cancel"));

        sides.insert(side.token());
        driven += 1;
    }

    assert_eq!(
        driven, TASK_BEARING_PHASES,
        "eleven task-bearing instrumented phases"
    );

    // --- phase zero: the cancellation arrives before the task exists -----------------------
    //
    // The twelfth phase, and the only one where "what did the cancel leave" is answered by
    // *nothing having happened*: denial precedes semantic work (RFC 0027 X3), so no scope is
    // opened, no artifact is named and the sentence is satisfied on its right-hand side
    // vacuously — which is worth saying out loud rather than leaving as an unswept corner.
    let mut fixture = fixture();
    let unknown = TaskHandle::new("task_nothing").expect("a handle");
    let denied = cancel(&mut fixture, &unknown, "req_cancel_zero");
    assert_eq!(
        denied.error_code(),
        Some(ErrorCode::CapabilityDenied),
        "phase zero: a task this daemon does not hold is denied"
    );
    assert!(
        denied.envelope.artifacts.is_empty(),
        "phase zero: a denied cancel names no artifact"
    );
    assert_eq!(
        regions(&fixture).opened(),
        0,
        "phase zero: a denied cancel names no task, so there is no work to tear down"
    );
    assert!(
        regions(&fixture).finalizations().is_empty(),
        "phase zero: and nothing was torn down"
    );
    assert_total(&fixture, "phase zero: after a denied cancel");

    assert_eq!(
        sides,
        std::collections::BTreeSet::from([
            "closed-and-complete",
            "nothing-published",
            "valid-continuation",
        ]),
        "a sweep that never reached a disjunct proves nothing about it"
    );
}

// --- clause 2: "leaves either a valid continuation" --------------------------------------------

/// **"a valid continuation" — proved by use, on a second fresh daemon.**
///
/// A handle that decodes is not a valid continuation; a handle that *resumes* is. This test
/// spends the continuation rather than inspecting it.
///
/// The device is forced by bn-10093's fix and stated rather than worked around: after the
/// cancel, daemon A's task is **terminal**, so `task.resume` there is a documented no-op —
/// which is exactly what
/// `dx14_falsification.rs`'s `regression_a_cancelled_task_is_not_writable_through_task_resume`
/// requires, and what this file's own closure test re-asserts. So validity is demonstrated
/// where the task is *not* closed, and the bridge between the two daemons is content identity:
///
/// 1. daemon **A** parks a campaign, is cancelled, and answers a `cont_*` on the
///    `committed-with-continuation` arm;
/// 2. daemon **B** — built from scratch, sharing nothing with A — runs the identical
///    `verification.start` and mints a continuation whose handle is **byte-identical** to A's
///    (no counter, no clock, no address: INV-005, INV-006);
/// 3. **A's handle**, not B's, is then resumed on B — and closes the campaign to Die Hard's
///    frozen 16 states, adding a publication rather than replacing one (INV-009).
///
/// So what A handed back is a resume pointer into real, re-derivable state. The refusal
/// `dx14_falsification.rs`'s `negative_a_cancelled_daemons_continuation_does_not_resolve_on_a_fresh_daemon`
/// records — A's handle denied on a fresh B that has not run the campaign — is the other side
/// of the same coin, and is why step 2 is required rather than decorative: the handle is
/// valid, and the *state behind it* is what a daemon must hold.
///
/// Step 3 is then run again on a third fresh daemon and the two resume answers are compared
/// byte for byte, so "resumes successfully" is a deterministic fact rather than one run's luck.
#[test]
fn positive_a_cancelled_tasks_continuation_is_valid_by_use_on_a_second_fresh_daemon() {
    // --- daemon A: park, cancel, and take the continuation it hands back --------------------
    let mut alpha = fixture();
    let started = start(&mut alpha, "req_alpha", 4, LANES[0].target());
    assert!(
        started.error_code().is_none(),
        "the parked fixture start was refused — {:?}",
        started.envelope.error
    );
    let task = started_task(&started);
    assert_eq!(entry(&alpha, &task).status, TaskStatus::Suspended);
    assert_eq!(entry(&alpha, &task).publications(), 1);

    let left = cancel_and_read(&mut alpha, &task, "req_alpha_cancel");
    assert_eq!(left.arm, "committed-with-continuation");
    assert_eq!(
        exit_side(left.status, &left.continuation, &left.artifacts),
        Some(ExitSide::ValidContinuation),
    );
    let handed_back = match &left.continuation {
        Nullable::Value(handle) => handle.clone(),
        Nullable::Null => panic!("the committed-with-continuation arm carries a `cont_*`"),
    };
    assert_eq!(
        left.artifacts.len(),
        1,
        "committed partial evidence sits behind the continuation, and is named"
    );
    assert_total(&alpha, "daemon A after the cancel");

    // The handle *resolves* on A too — a terminal no-op rather than a denial, which is
    // bn-10093's fix and is asserted below in its own test.
    let on_a = resume(&mut alpha, &handed_back, 64, "req_alpha_resume");
    assert_eq!(
        on_a.envelope.status,
        ResultStatus::Ok,
        "the cancelled task's continuation still names something this daemon holds — {:?}",
        on_a.envelope.error
    );

    // --- daemon B: the same campaign, from scratch ------------------------------------------
    let mut beta = fixture();
    let mirrored = start(&mut beta, "req_alpha", 4, LANES[0].target());
    assert!(mirrored.error_code().is_none());
    let mirror_task = started_task(&mirrored);
    assert_eq!(
        mirror_task, task,
        "the task identity is content-addressed and must agree across daemons"
    );
    let mirror_continuation = entry(&beta, &mirror_task)
        .continuation
        .clone()
        .expect("a bounded run parks with a continuation");
    assert_eq!(
        mirror_continuation, handed_back,
        "the continuation A handed back and the one B minted are one name: no counter, no \
         clock, no address (INV-005, INV-006)"
    );

    // --- validity, by use: A's handle, spent on B --------------------------------------------
    let resumed = resume(&mut beta, &handed_back, 64, "req_beta_resume");
    assert_eq!(
        resumed.envelope.status,
        ResultStatus::Ok,
        "A's continuation did not resume — {:?}",
        resumed.envelope.error
    );
    let after = record(&read(&mut beta, &mirror_task, "req_beta_read"));
    assert_eq!(
        after.status,
        TaskStatus::Completed,
        "the resumed campaign closed"
    );
    assert_eq!(
        after.cost.states.value().copied(),
        Some(FROZEN_STATES),
        "and closed on Die Hard's frozen reachable-state count, which is what makes this a \
         resume of the parked campaign rather than any run at all"
    );
    assert!(
        after.continuation.is_absent(),
        "a closed campaign parks nothing"
    );
    assert_eq!(
        entry(&beta, &mirror_task).publications(),
        2,
        "the resume *added* a publication (INV-009) rather than replacing the parked one"
    );
    assert_total(&beta, "daemon B after spending A's continuation");

    // --- and it is deterministic: a third fresh daemon, byte for byte ------------------------
    let mut gamma = fixture();
    let gamma_start = start(&mut gamma, "req_alpha", 4, LANES[0].target());
    assert!(gamma_start.error_code().is_none());
    let gamma_task = started_task(&gamma_start);
    let gamma_resume = resume(&mut gamma, &handed_back, 64, "req_beta_resume");
    assert_eq!(
        wire_bytes(&gamma_resume),
        wire_bytes(&resumed),
        "two fresh daemons spending the same continuation answer byte-identically"
    );
    assert_eq!(
        wire_bytes(&read(&mut gamma, &gamma_task, "req_beta_read")),
        wire_bytes(&read(&mut beta, &mirror_task, "req_beta_read")),
        "and the record they leave is byte-identical too"
    );
    assert_total(&gamma, "daemon C after the same resume");
}

// --- clause 3: "or no published partial artifact" ----------------------------------------------

/// **"no published partial artifact" — the right-hand side, in detail, plus the arm that has
/// no constructor.**
///
/// Two dispositions reach the right-hand side, and they are different claims:
///
/// - **nothing published at all.** The synthesis lane — a ceiling too small to admit anything
///   — and the faulted dispatch. `ResultEnvelope.artifacts` is empty, all three counts agree
///   at zero, nothing is staged, and RFC 0026's SD-13 clause is honoured: a `BudgetExhausted`
///   failure with no continuation carries a typed `non_resumable_reason` rather than being a
///   silent dead end.
/// - **published, and complete.** The solver lane — an atomic query answered whole. The
///   campaign *closed*, so what it published is not a partial artifact; the publication is
///   named by a content identity, the counts agree, and the region layer's arm carries the
///   typed reason RFC 0026 requires of committed evidence that genuinely cannot be resumed.
///
/// The second half of this test is the **no-constructor** claim, asserted at the wire over
/// every task-bearing phase rather than argued from the type:
///
/// - a continuation is never answered over a task that published nothing — a resume pointer
///   into nothing;
/// - committed evidence with no continuation is never *silent* — it always carries the arm's
///   typed reason, so "committed evidence and no continuation" full stop is unreachable.
///
/// `CancelOutcome` has no constructor for either shape (`continuum-task`'s
/// `region::worker`), and `dx14_falsification.rs`'s
/// `negative_no_wire_sequence_reaches_a_dangling_continuation_or_a_pointerless_resume` attacks
/// the same claim over 8 residual profiles. This is the readable statement of it.
#[test]
fn positive_a_cancel_with_no_continuation_leaves_no_published_partial_artifact() {
    // --- nothing published: the synthesis lane -----------------------------------------------
    let mut nothing = fixture();
    let task = wound_up(&mut nothing, &LANES[3], 1);
    let synthesis = cancel_and_read(&mut nothing, &task, "req_synth_cancel");
    assert_eq!(synthesis.arm, "nothing-published");
    assert_eq!(
        exit_side(
            synthesis.status,
            &synthesis.continuation,
            &synthesis.artifacts
        ),
        Some(ExitSide::NothingPublished),
    );
    assert!(
        synthesis.artifacts.is_empty(),
        "a campaign that admitted nothing names no artifact"
    );
    assert_eq!(synthesis.table_committed, 0);
    assert_eq!(synthesis.region_committed, 0);
    assert!(!synthesis.staging);
    assert_eq!(synthesis.continuation, Nullable::Null);

    let failed = record(&read(&mut nothing, &task, "req_synth_read"));
    assert_eq!(failed.status, TaskStatus::Failed);
    assert_eq!(
        failed.failed_reason.value().copied(),
        Some(ErrorCode::BudgetExhausted)
    );
    assert!(
        failed.continuation.is_absent(),
        "nothing was published, so there is nothing to resume from"
    );
    assert!(
        !failed.non_resumable_reason.is_absent(),
        "SD-13: `failed_reason = BudgetExhausted` with no continuation REQUIRES a typed \
         `non_resumable_reason` — a dead end that says why it is one"
    );
    assert_total(&nothing, "after cancelling a lane that published nothing");

    // --- nothing published: the faulted dispatch ---------------------------------------------
    let mut stranded_fixture = fixture();
    let stranded = faulted(&mut stranded_fixture);
    let fault = cancel_and_read(&mut stranded_fixture, &stranded, "req_fault_cancel");
    assert_eq!(fault.arm, "nothing-published");
    assert_eq!(
        exit_side(fault.status, &fault.continuation, &fault.artifacts),
        Some(ExitSide::NothingPublished),
    );
    assert!(fault.artifacts.is_empty());
    assert_eq!(fault.table_committed, 0);
    assert_eq!(fault.region_committed, 0);
    assert!(!fault.staging);
    assert_total(
        &stranded_fixture,
        "after cancelling a task whose run faulted",
    );

    // --- published, and complete: the solver lane --------------------------------------------
    let mut closed = fixture();
    let task = wound_up(&mut closed, &LANES[1], 1);
    let solver = cancel_and_read(&mut closed, &task, "req_solver_cancel");
    assert_eq!(solver.arm, "committed-non-resumable");
    assert_eq!(
        exit_side(solver.status, &solver.continuation, &solver.artifacts),
        Some(ExitSide::ClosedAndComplete),
    );
    assert_eq!(
        solver.status,
        TaskStatus::Completed,
        "the campaign closed, which is what makes its artifact a whole answer rather than a \
         partial one"
    );
    assert_eq!(solver.continuation, Nullable::Null);
    assert_eq!(
        solver.artifacts.len(),
        1,
        "an atomic query answered whole publishes once"
    );
    assert_eq!(solver.table_committed, 1);
    assert_eq!(solver.region_committed, 1);
    assert!(!solver.staging);
    assert!(
        !solver.artifacts[0].commitment.is_absent(),
        "the publication is named by a content identity"
    );
    assert_eq!(
        solver.reason.as_deref(),
        Some("no-continuation"),
        "committed evidence that genuinely cannot be resumed carries RFC 0026's typed reason \
         rather than degrading to silence"
    );
    assert_total(&closed, "after cancelling a closed campaign");

    // --- the arm with no constructor, at the wire, over every phase --------------------------
    let mut checked = 0_usize;
    for (context, mut phase, task, row) in every_task_bearing_phase() {
        let left = cancel_and_read(&mut phase, &task, "req_cancel");
        let has_continuation = left.continuation != Nullable::Null;
        let published = !left.artifacts.is_empty();

        assert!(
            !(has_continuation && !published),
            "{context}: a continuation over a task that published nothing is a resume pointer \
             into nothing, and has no constructor"
        );
        if published && !has_continuation {
            assert_eq!(
                left.arm, "committed-non-resumable",
                "{context}: committed evidence with no continuation is a *named* outcome"
            );
            assert!(
                left.reason.is_some_and(|reason| !reason.is_empty()),
                "{context}: and it carries a typed reason — 'committed evidence and no \
                 continuation' full stop has no constructor"
            );
            assert_eq!(
                left.status,
                TaskStatus::Completed,
                "{context}: and the campaign it belongs to closed, so the artifact is not partial"
            );
        }
        assert_eq!(
            row.side == ExitSide::ValidContinuation,
            has_continuation,
            "{context}: the row and the wire disagree about which disjunct held"
        );
        checked += 1;
    }
    assert_eq!(checked, TASK_BEARING_PHASES);
}

// --- the falsified-and-fixed closure property --------------------------------------------------

/// **REGRESSION GUARD (was bn-3p32's A1 FALSIFICATION, found independently again by bn-1kp6's
/// attack 18) — a cancelled record is closed, and the exit disjunct does not move under the
/// suffix that broke it.**
///
/// # The cycle this test is the end of
///
/// bn-2zy's constructive matrix established the table at every phase with no change to the
/// system under test. Two adversarial campaigns then attacked it with `src/` frozen — bn-3p32
/// (18 attacks against G0-DX-14's two conjuncts) and bn-1kp6 (22 attacks against G0-DX-03's
/// three) — and **both, independently, found the same defect**: `task.resume` applied the
/// request's optional budget to the task's ledger *before* it checked terminality, so
///
/// ```text
/// verification.start (parks)  →  task.cancel  →  task.resume(continuation, budget)
/// ```
///
/// rewrote a `Cancelled` task's ceilings, and with the new ceiling below recorded spend the
/// B18 arm fired and appended a `task.budget_suspended` milestone — and its `TaskEvent` — to a
/// **closed** record. bn-10093 moved the terminal check above the ledger write, so the budget
/// arm now produces `task.update_budget`'s own observable for a terminal task: nothing
/// written, no milestone, no event.
///
/// # What this test adds to the two regression guards that already exist
///
/// `dx14_falsification.rs`'s `regression_a_cancelled_task_is_not_writable_through_task_resume`
/// and `dx03_falsification.rs`'s own reproduction both hold the *record* byte-identical. This
/// one holds the **exit sentence** across the same suffix: the disjunct the cancellation landed
/// on is recomputed after the hostile resume and must be the same value, because a closure
/// defect that moved a task from "valid continuation" to "published partial artifact with
/// nothing pointing at it" is precisely the failure PR 6's exit is about, and asserting the
/// record is unchanged is a proxy for that rather than the thing itself.
#[test]
fn regression_the_cancelled_record_is_closed_and_the_exit_side_does_not_move() {
    let mut fixture = fixture();
    let started = start(&mut fixture, "req_closure", 4, LANES[0].target());
    assert!(started.error_code().is_none());
    let task = started_task(&started);
    let continuation = entry(&fixture, &task)
        .continuation
        .clone()
        .expect("a bounded run parks with a continuation");

    let left = cancel_and_read(&mut fixture, &task, "req_closure_cancel");
    let before_side = exit_side(left.status, &left.continuation, &left.artifacts)
        .expect("the cancel landed on a disjunct");
    assert_eq!(before_side, ExitSide::ValidContinuation);
    assert_total(&fixture, "after the cancel");

    let before_bytes = wire_bytes(&read(&mut fixture, &task, "req_closure_read"));
    let before = record(&read(&mut fixture, &task, "req_closure_read"));
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

    // The attack, unchanged: resume the dead task's continuation carrying a budget below its
    // recorded spend — the one request that used to reach the ledger.
    let attacked = resume(&mut fixture, &continuation, 1, "req_closure_resume");
    assert_eq!(
        attacked.envelope.status,
        ResultStatus::Ok,
        "the resume is still answered rather than refused — {:?}",
        attacked.envelope.error
    );

    let after_bytes = wire_bytes(&read(&mut fixture, &task, "req_closure_read"));
    let after = record(&read(&mut fixture, &task, "req_closure_read"));
    assert_eq!(
        after_bytes, before_bytes,
        "a cancelled task's record must be byte-identical across a resume carrying a budget"
    );
    assert_eq!(
        after.status,
        TaskStatus::Cancelled,
        "the status is monotone"
    );
    assert_eq!(
        after.budget.states.value().copied(),
        Some(ceiling),
        "a cancelled task keeps the budget it ran under"
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

    // --- the exit sentence, recomputed after the attack ---------------------------------------
    let again = cancel_and_read(&mut fixture, &task, "req_closure_cancel_2");
    let after_side = exit_side(again.status, &again.continuation, &again.artifacts)
        .expect("the second cancel landed on a disjunct");
    assert_eq!(
        after_side, before_side,
        "the disjunct the cancellation landed on moved under a post-cancel resume — the exit \
         sentence must survive the suffix that falsified record closure"
    );
    assert_eq!(
        again.table_committed, left.table_committed,
        "and nothing was published or un-published in between"
    );
    assert_eq!(again.artifacts, left.artifacts, "nor renamed");
    assert_total(&fixture, "after the hostile resume of a cancelled task");
}

// --- anti-vacuity -------------------------------------------------------------------------------

/// **The exit phases exercise more than one behaviour class, and the classifier can say no.**
///
/// Two independent guards against a green suite that proves nothing:
///
/// 1. **the phases differ.** A canonical rendering of each phase's outcome — the disjunct, the
///    region layer's arm, the answered status, the publication count and whether a continuation
///    was answered — is collected across all eleven task-bearing phases, and the set of
///    *distinct* renderings must have more than one member. In fact all three arms, all three
///    disjuncts and both sides of "did it publish anything" must appear, which is the shape
///    `region_no_orphan.rs`'s own anti-vacuity check uses one crate down.
/// 2. **[`exit_side`] is not a tautology.** The function's [`None`] branch is fed the shape the
///    exit sentence forbids — artifacts named, no continuation, a campaign that did not close —
///    built from a **real** `ResultEnvelope.artifacts` list taken off a real cancellation, so
///    the input is one a daemon could in principle produce rather than a hand-drawn value. If
///    the classifier accepted it, every assertion in this file would be vacuous.
#[test]
fn positive_the_exit_phases_exercise_more_than_one_behaviour_class() {
    let mut renderings: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut arms: std::collections::BTreeSet<&'static str> = std::collections::BTreeSet::new();
    let mut sides: std::collections::BTreeSet<&'static str> = std::collections::BTreeSet::new();
    let mut published: std::collections::BTreeSet<bool> = std::collections::BTreeSet::new();
    // A genuine artifact list, kept for the falsification probe below.
    let mut real_artifacts: Vec<ArtifactRef> = Vec::new();

    for (context, mut fixture, task, _) in every_task_bearing_phase() {
        let left = cancel_and_read(&mut fixture, &task, "req_cancel");
        let side = exit_side(left.status, &left.continuation, &left.artifacts)
            .unwrap_or_else(|| panic!("{context}: the cancel landed on neither disjunct"));
        renderings.insert(format!(
            "side={} arm={} status={:?} published={} continuation={}",
            side.token(),
            left.arm,
            left.status,
            left.table_committed,
            left.continuation != Nullable::Null,
        ));
        arms.insert(left.arm);
        sides.insert(side.token());
        published.insert(!left.artifacts.is_empty());
        if real_artifacts.is_empty() {
            real_artifacts = left.artifacts.clone();
        }
    }

    assert!(
        renderings.len() > 1,
        "a sweep whose phases all behave identically proves nothing about any of them: {renderings:?}"
    );
    assert_eq!(
        arms,
        std::collections::BTreeSet::from([
            "committed-non-resumable",
            "committed-with-continuation",
            "nothing-published",
        ]),
        "all three arms of `rule task.cancel_correct` are reached"
    );
    assert_eq!(
        sides,
        std::collections::BTreeSet::from([
            "closed-and-complete",
            "nothing-published",
            "valid-continuation",
        ]),
        "all three dispositions of the exit disjunction are reached"
    );
    assert_eq!(
        published,
        std::collections::BTreeSet::from([false, true]),
        "phases that published something and phases that published nothing are both present"
    );

    // --- the classifier can say no ------------------------------------------------------------
    assert!(
        !real_artifacts.is_empty(),
        "the probe needs a genuine artifact list off a real cancellation"
    );
    assert_eq!(
        exit_side(TaskStatus::Suspended, &Nullable::Null, &real_artifacts),
        None,
        "a published partial artifact with no continuation to resume it from must be rejected: \
         if `exit_side` accepted it, every assertion in this file would be vacuous"
    );
    assert_eq!(
        exit_side(TaskStatus::Cancelled, &Nullable::Null, &real_artifacts),
        None,
        "and the same shape on a cancelled task, which is the wire sequence the sentence is \
         literally about"
    );
    // The controls that localise the rejection to the shape rather than to the artifact list.
    assert_eq!(
        exit_side(TaskStatus::Completed, &Nullable::Null, &real_artifacts),
        Some(ExitSide::ClosedAndComplete),
        "the same artifacts over a campaign that closed are a whole answer, not a partial one"
    );
    assert_eq!(
        exit_side(TaskStatus::Suspended, &Nullable::Null, &[]),
        Some(ExitSide::NothingPublished),
        "and the same status with nothing published is the right-hand disjunct"
    );
}
