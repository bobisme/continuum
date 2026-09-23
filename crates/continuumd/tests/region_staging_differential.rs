//! Differential: the region calculus's staging, abort, adapter-obligation and per-task
//! cancellation rules against an independent transition model explored by
//! `continuum_engine_reference` (RFC 0026 corrections 51 and 53; bn-2318t, cr-1ckhmw,
//! bn-36wy3).
//!
//! The oracle is written from the corrections' text, not from `region.rs`. It models two
//! workers in the root region:
//!
//! - worker A, with its whole lifecycle, three staging slots, and a "has committed" flag;
//! - worker B, with the lifecycle steps a transfer partner needs;
//! - each worker's own cancellation: request, acknowledgement, cancelled completion;
//! - the root region's cancellation, which requests both workers' cancellation;
//! - one adapter obligation, opened by A, discharged by its holder as committed or
//!   aborted, and handed between A and B, including the refused self-transfer.
//!
//! Each step is a guarded action in `continuum_engine_reference`'s programmatic model
//! language. The subject is `continuum_task::region::RegionTree`, driven through its
//! public API only. The test explores both, and asserts two things:
//!
//! - they reach the same abstract states;
//! - in every such state, every step is admitted by one exactly when the other admits
//!   it, and lands in the same state.
//!
//! The two share no code: the oracle never names a calculus type, and the subject's side
//! observes the tree only through `worker_state`, `cancel_phase`, `evidence`,
//! `ledger().holds` and `substrate_holder`. Nine mutated oracle guards are told apart,
//! so agreement is not vacuous. One of them is correction 51's region-level strictness,
//! which correction 53 relaxed.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use continuum_engine_reference::{
    ActionDecl, BoolExpr, Bounds, CmpOp, IntExpr, Model, ModelBuilder, State, explore,
};
use continuum_task::region::obligation::{
    Obligation, SubstrateId, SubstrateKind, SubstrateObligation, SubstrateOutcome,
};
use continuum_task::region::worker::{
    CancelPhase, FailureReason, PublicationSlot, Resumability, WorkerId, WorkerState, WorkerStep,
};
use continuum_task::region::{RegionFault, RegionTree};

/// The abstract state vector, in the oracle's variable order (the model orders its
/// variables by name): B's lifecycle, A's "has committed", the region's cancellation,
/// the obligation's holder (0 = A, 1 = B), A's and B's acknowledgements, A's three
/// slots, A's and B's own cancellation requests, A's lifecycle, and the obligation's
/// phase (0 never opened, 1 open, 2 discharged).
const VARIABLES: [&str; 13] = [
    "b", "c", "cx", "h", "ka", "kb", "p0", "p1", "p2", "ra", "rb", "st", "sub",
];

/// Positions in [`VARIABLES`].
const B: usize = 0;
const CX: usize = 2;
const H: usize = 3;
const KB: usize = 5;
const P0: usize = 6;
const ST: usize = 11;
const SUB: usize = 12;

/// Lifecycle codes: created, running, suspended, completed, failed, cancelled.
const CREATED: i64 = 0;
const RUNNING: i64 = 1;
const SUSPENDED: i64 = 2;
const COMPLETED: i64 = 3;
const FAILED: i64 = 4;
const CANCELLED: i64 = 5;

/// The steps, named once for both sides. `OpenSub` in a state where the obligation was
/// already opened is the identity-reuse mutant; `TransferAA` is the self-transfer.
const ACTIONS: [&str; 36] = [
    "Begin",
    "Reserve",
    "ReserveSlot0",
    "ReserveSlot1",
    "ReserveSlot2",
    "Commit",
    "Abort",
    "CommitSlot0",
    "CommitSlot1",
    "CommitSlot2",
    "AbortSlot0",
    "AbortSlot1",
    "AbortSlot2",
    "Suspend",
    "Resume",
    "Complete",
    "Fail",
    "RequestCancel",
    "AckCancel",
    "CompleteCancelled",
    "BeginB",
    "SuspendB",
    "ResumeB",
    "CompleteB",
    "RequestCancelB",
    "AckCancelB",
    "CompleteCancelledB",
    "Cancel",
    "OpenSub",
    "DischargeCommitA",
    "DischargeAbortA",
    "DischargeCommitB",
    "DischargeAbortB",
    "TransferAB",
    "TransferBA",
    "TransferAA",
];

// --- the oracle: RFC 0026 corrections 51 and 53 as a guarded transition model ----------

fn var(name: &str) -> IntExpr {
    IntExpr::var(name)
}

fn eq(name: &str, value: i64) -> BoolExpr {
    BoolExpr::compare(CmpOp::Eq, var(name), IntExpr::constant(value))
}

fn le(name: &str, value: i64) -> BoolExpr {
    BoolExpr::compare(CmpOp::Le, var(name), IntExpr::constant(value))
}

fn staged() -> IntExpr {
    IntExpr::plus(IntExpr::plus(var("p0"), var("p1")), var("p2"))
}

fn staged_is(value: i64) -> BoolExpr {
    BoolExpr::compare(CmpOp::Eq, staged(), IntExpr::constant(value))
}

fn running() -> BoolExpr {
    eq("st", RUNNING)
}

/// `lifecycle` has begun and not terminated: running or parked. The correction's
/// `WorkerState::holds_substrate`.
fn holds(lifecycle: &str) -> BoolExpr {
    BoolExpr::or(eq(lifecycle, RUNNING), eq(lifecycle, SUSPENDED))
}

fn and3(a: BoolExpr, b: BoolExpr, c: BoolExpr) -> BoolExpr {
    BoolExpr::and(BoolExpr::and(a, b), c)
}

fn set(name: &'static str, value: i64) -> (&'static str, IntExpr) {
    (name, IntExpr::constant(value))
}

/// The guards the mutant controls flip; [`Guards::CORRECT`] is the corrections' text.
#[derive(Clone, Copy)]
#[allow(clippy::struct_excessive_bools)]
struct Guards {
    /// An unnamed abort needs exactly one staged publication.
    abort_needs_staging: bool,
    /// An adapter-obligation open needs a holder that has begun.
    open_needs_begun: bool,
    /// A discharge admits a parked holder (the substrate discharges during
    /// cancellation, before the holder is terminal).
    discharge_admits_parked: bool,
    /// A committed discharge is refused after the holder acknowledges its cancellation.
    commit_refused_after_ack: bool,
    /// A transfer to the holder itself is refused.
    transfer_refuses_self: bool,
    /// A transfer needs both parties acting: neither has acknowledged a cancellation.
    transfer_needs_acting: bool,
    /// Correction 51's strictness, relaxed by correction 53: a transfer is refused
    /// while the region is cancelling, whatever the parties acknowledged.
    region_cancel_blocks_transfer: bool,
    /// An acknowledgement needs a requested cancellation.
    ack_needs_request: bool,
    /// An acknowledged worker takes no ordinary step (it only drains).
    acknowledged_only_drains: bool,
}

impl Guards {
    const CORRECT: Self = Self {
        abort_needs_staging: true,
        open_needs_begun: true,
        discharge_admits_parked: true,
        commit_refused_after_ack: true,
        transfer_refuses_self: true,
        transfer_needs_acting: true,
        region_cancel_blocks_transfer: false,
        ack_needs_request: true,
        acknowledged_only_drains: true,
    };
}

/// One worker's cancellation variables: lifecycle, own request, acknowledgement.
#[derive(Clone, Copy)]
struct Party {
    lifecycle: &'static str,
    requested: &'static str,
    acknowledged: &'static str,
}

const PARTY_A: Party = Party {
    lifecycle: "st",
    requested: "ra",
    acknowledged: "ka",
};
const PARTY_B: Party = Party {
    lifecycle: "b",
    requested: "rb",
    acknowledged: "kb",
};

/// The oracle, with `guards` deciding the flippable guards.
#[allow(clippy::too_many_lines)]
fn oracle(guards: Guards) -> Model {
    let slot = |k: usize| ["p0", "p1", "p2"][k];
    // An ordinary step of a worker is refused once it acknowledged its cancellation.
    let ordinary = |party: Party, guard: BoolExpr| {
        if guards.acknowledged_only_drains {
            BoolExpr::and(guard, eq(party.acknowledged, 0))
        } else {
            guard
        }
    };
    // A transfer party: holds substrate, and (per correction 53) has not acknowledged.
    let party_acts = |party: Party| {
        let mut guard = holds(party.lifecycle);
        if guards.transfer_needs_acting {
            guard = BoolExpr::and(guard, eq(party.acknowledged, 0));
        }
        if guards.region_cancel_blocks_transfer {
            guard = BoolExpr::and(guard, eq("cx", 0));
        }
        guard
    };
    let requested = |party: Party| BoolExpr::or(eq(party.requested, 1), eq("cx", 1));
    let cancellation = |party: Party, request: &str, ack: &str, complete: &str| {
        let mut out = Vec::new();
        // Requested only once: not by its own earlier request and not by its region.
        out.push(ActionDecl::deterministic(
            request,
            and3(
                le(party.lifecycle, SUSPENDED),
                eq(party.acknowledged, 0),
                BoolExpr::and(eq(party.requested, 0), eq("cx", 0)),
            ),
            vec![set(party.requested, 1)],
        ));
        out.push(ActionDecl::deterministic(
            ack,
            and3(
                le(party.lifecycle, SUSPENDED),
                eq(party.acknowledged, 0),
                if guards.ack_needs_request {
                    requested(party)
                } else {
                    BoolExpr::constant(true)
                },
            ),
            vec![set(party.acknowledged, 1)],
        ));
        let mut done = vec![set(party.lifecycle, CANCELLED)];
        if party.lifecycle == "st" {
            done.extend([set("p0", 0), set("p1", 0), set("p2", 0)]);
        }
        out.push(ActionDecl::deterministic(
            complete,
            BoolExpr::and(le(party.lifecycle, SUSPENDED), eq(party.acknowledged, 1)),
            done,
        ));
        out
    };
    let discharge = |name: &str, party: Party, holder: i64, commit: bool| {
        let lifecycle = if guards.discharge_admits_parked {
            holds(party.lifecycle)
        } else {
            eq(party.lifecycle, RUNNING)
        };
        let phase = if commit && guards.commit_refused_after_ack {
            eq(party.acknowledged, 0)
        } else {
            BoolExpr::constant(true)
        };
        ActionDecl::deterministic(
            name,
            and3(
                BoolExpr::and(lifecycle, phase),
                eq("sub", 1),
                eq("h", holder),
            ),
            vec![set("sub", 2), set("h", 0)],
        )
    };

    let mut builder = ModelBuilder::new()
        .variable("b", CREATED, CANCELLED)
        .variable("c", 0, 1)
        .variable("cx", 0, 1)
        .variable("h", 0, 1)
        .variable("ka", 0, 1)
        .variable("kb", 0, 1)
        .variable("p0", 0, 1)
        .variable("p1", 0, 1)
        .variable("p2", 0, 1)
        .variable("ra", 0, 1)
        .variable("rb", 0, 1)
        .variable("st", CREATED, CANCELLED)
        .variable("sub", 0, 2)
        .initial_state(&[
            ("b", CREATED),
            ("c", 0),
            ("cx", 0),
            ("h", 0),
            ("ka", 0),
            ("kb", 0),
            ("p0", 0),
            ("p1", 0),
            ("p2", 0),
            ("ra", 0),
            ("rb", 0),
            ("st", CREATED),
            ("sub", 0),
        ])
        // Created → Running.
        .action(ActionDecl::deterministic(
            "Begin",
            ordinary(PARTY_A, eq("st", CREATED)),
            vec![set("st", RUNNING)],
        ))
        // The one-in-flight reserve stages the primary slot, only when nothing is staged.
        .action(ActionDecl::deterministic(
            "Reserve",
            ordinary(PARTY_A, BoolExpr::and(running(), staged_is(0))),
            vec![set("p0", 1)],
        ));
    for k in 0..3 {
        builder = builder.action(ActionDecl::deterministic(
            ["ReserveSlot0", "ReserveSlot1", "ReserveSlot2"][k],
            ordinary(PARTY_A, BoolExpr::and(running(), eq(slot(k), 0))),
            vec![set(slot(k), 1)],
        ));
    }
    // The unnamed resolutions resolve the sole staged publication, and nothing else.
    let clear = || vec![set("p0", 0), set("p1", 0), set("p2", 0)];
    let mut commit = clear();
    commit.push(set("c", 1));
    builder = builder
        .action(ActionDecl::deterministic(
            "Commit",
            ordinary(PARTY_A, BoolExpr::and(running(), staged_is(1))),
            commit,
        ))
        // An abort drains an effect: admitted after the acknowledgement too.
        .action(ActionDecl::deterministic(
            "Abort",
            if guards.abort_needs_staging {
                BoolExpr::and(running(), staged_is(1))
            } else {
                BoolExpr::and(
                    running(),
                    BoolExpr::compare(CmpOp::Le, staged(), IntExpr::constant(1)),
                )
            },
            clear(),
        ));
    for k in 0..3 {
        builder = builder.action(ActionDecl::deterministic(
            ["CommitSlot0", "CommitSlot1", "CommitSlot2"][k],
            ordinary(PARTY_A, BoolExpr::and(running(), eq(slot(k), 1))),
            vec![set(slot(k), 0), set("c", 1)],
        ));
    }
    for k in 0..3 {
        builder = builder.action(ActionDecl::deterministic(
            ["AbortSlot0", "AbortSlot1", "AbortSlot2"][k],
            BoolExpr::and(running(), eq(slot(k), 1)),
            vec![set(slot(k), 0)],
        ));
    }
    let mut fail = clear();
    fail.push(set("st", FAILED));
    builder = builder
        // Park and finish only with nothing staged.
        .action(ActionDecl::deterministic(
            "Suspend",
            ordinary(PARTY_A, BoolExpr::and(running(), staged_is(0))),
            vec![set("st", SUSPENDED)],
        ))
        .action(ActionDecl::deterministic(
            "Resume",
            ordinary(PARTY_A, eq("st", SUSPENDED)),
            vec![set("st", RUNNING)],
        ))
        .action(ActionDecl::deterministic(
            "Complete",
            ordinary(PARTY_A, BoolExpr::and(running(), staged_is(0))),
            vec![set("st", COMPLETED)],
        ))
        // A failure discards every staged slot.
        .action(ActionDecl::deterministic(
            "Fail",
            ordinary(PARTY_A, le("st", RUNNING)),
            fail,
        ));
    for action in cancellation(PARTY_A, "RequestCancel", "AckCancel", "CompleteCancelled") {
        builder = builder.action(action);
    }
    builder = builder
        // Worker B: the lifecycle a transfer partner needs.
        .action(ActionDecl::deterministic(
            "BeginB",
            ordinary(PARTY_B, eq("b", CREATED)),
            vec![set("b", RUNNING)],
        ))
        .action(ActionDecl::deterministic(
            "SuspendB",
            ordinary(PARTY_B, eq("b", RUNNING)),
            vec![set("b", SUSPENDED)],
        ))
        .action(ActionDecl::deterministic(
            "ResumeB",
            ordinary(PARTY_B, eq("b", SUSPENDED)),
            vec![set("b", RUNNING)],
        ))
        .action(ActionDecl::deterministic(
            "CompleteB",
            ordinary(PARTY_B, eq("b", RUNNING)),
            vec![set("b", COMPLETED)],
        ));
    for action in cancellation(
        PARTY_B,
        "RequestCancelB",
        "AckCancelB",
        "CompleteCancelledB",
    ) {
        builder = builder.action(action);
    }
    let self_transfer = if guards.transfer_refuses_self {
        BoolExpr::constant(false)
    } else {
        and3(eq("sub", 1), eq("h", 0), party_acts(PARTY_A))
    };
    builder
        // The root region's cancellation request; asking twice changes nothing.
        .action(ActionDecl::deterministic(
            "Cancel",
            BoolExpr::constant(true),
            vec![set("cx", 1)],
        ))
        // Adapter obligation s0: opened once by A while it has begun and not terminated,
        // has not acknowledged a cancellation, and its region is open; an identity
        // opened before is never reopened.
        .action(ActionDecl::deterministic(
            "OpenSub",
            and3(
                if guards.open_needs_begun {
                    holds("st")
                } else {
                    le("st", SUSPENDED)
                },
                BoolExpr::and(eq("ka", 0), eq("sub", 0)),
                eq("cx", 0),
            ),
            vec![set("sub", 1), set("h", 0)],
        ))
        // Discharged once, by its holder, which has begun and not terminated. After the
        // holder's acknowledgement only an abort is admitted.
        .action(discharge("DischargeCommitA", PARTY_A, 0, true))
        .action(discharge("DischargeAbortA", PARTY_A, 0, false))
        .action(discharge("DischargeCommitB", PARTY_B, 1, true))
        .action(discharge("DischargeAbortB", PARTY_B, 1, false))
        // Handed on by its acting holder to another acting worker.
        .action(ActionDecl::deterministic(
            "TransferAB",
            and3(
                BoolExpr::and(eq("sub", 1), eq("h", 0)),
                party_acts(PARTY_A),
                party_acts(PARTY_B),
            ),
            vec![set("h", 1)],
        ))
        .action(ActionDecl::deterministic(
            "TransferBA",
            and3(
                BoolExpr::and(eq("sub", 1), eq("h", 1)),
                party_acts(PARTY_B),
                party_acts(PARTY_A),
            ),
            vec![set("h", 0)],
        ))
        // A self-transfer is refused: it would be a hand-off that hands nothing.
        .action(ActionDecl::deterministic(
            "TransferAA",
            self_transfer,
            vec![],
        ))
        .build()
        .expect("the oracle is a well-formed model")
}

// --- the subject: the calculus, observed through its public API -----------------------

fn permit() -> SubstrateObligation {
    SubstrateObligation::new(
        SubstrateKind::new("send-permit").expect("canonical"),
        SubstrateId::at(0),
    )
}

/// The two workers: A is `w0`, B is `w1`, both in the root region.
const A: WorkerId = WorkerId::at(0);
const WB: WorkerId = WorkerId::at(1);

/// Apply one named step to the calculus.
fn apply(tree: &mut RegionTree, action: &str) -> Result<(), RegionFault> {
    let slot = |text: &str| {
        PublicationSlot::at(
            text.chars()
                .last()
                .and_then(|c| c.to_digit(10))
                .expect("slot actions end in a digit"),
        )
    };
    let (worker, step) = match action {
        "Begin" => (A, WorkerStep::Begin),
        "Reserve" => (A, WorkerStep::Reserve),
        "Commit" => (A, WorkerStep::Commit),
        "Abort" => (A, WorkerStep::Abort),
        "Suspend" => (A, WorkerStep::Suspend),
        "Resume" => (A, WorkerStep::Resume),
        "Complete" => (A, WorkerStep::Complete),
        "Fail" => (
            A,
            WorkerStep::Fail(FailureReason::new("engine-defect").expect("canonical")),
        ),
        "RequestCancel" => (A, WorkerStep::RequestCancel),
        "AckCancel" => (A, WorkerStep::AcknowledgeCancel),
        "CompleteCancelled" => (A, WorkerStep::CompleteCancelled),
        "BeginB" => (WB, WorkerStep::Begin),
        "SuspendB" => (WB, WorkerStep::Suspend),
        "ResumeB" => (WB, WorkerStep::Resume),
        "CompleteB" => (WB, WorkerStep::Complete),
        "RequestCancelB" => (WB, WorkerStep::RequestCancel),
        "AckCancelB" => (WB, WorkerStep::AcknowledgeCancel),
        "CompleteCancelledB" => (WB, WorkerStep::CompleteCancelled),
        "Cancel" => {
            let root = tree.root();
            return tree.cancel(root);
        }
        "OpenSub" => return tree.open_substrate(A, permit()),
        "DischargeCommitA" => {
            return tree.discharge_substrate(A, permit(), SubstrateOutcome::Committed);
        }
        "DischargeAbortA" => {
            return tree.discharge_substrate(A, permit(), SubstrateOutcome::Aborted);
        }
        "DischargeCommitB" => {
            return tree.discharge_substrate(WB, permit(), SubstrateOutcome::Committed);
        }
        "DischargeAbortB" => {
            return tree.discharge_substrate(WB, permit(), SubstrateOutcome::Aborted);
        }
        "TransferAB" => return tree.transfer_substrate(A, permit(), WB),
        "TransferBA" => return tree.transfer_substrate(WB, permit(), A),
        "TransferAA" => return tree.transfer_substrate(A, permit(), A),
        other if other.starts_with("ReserveSlot") => (A, WorkerStep::ReserveSlot(slot(other))),
        other if other.starts_with("CommitSlot") => (A, WorkerStep::CommitSlot(slot(other))),
        other if other.starts_with("AbortSlot") => (A, WorkerStep::AbortSlot(slot(other))),
        other => panic!("unknown action {other}"),
    };
    tree.advance(worker, step).map(|_| ())
}

/// The facts the API does not expose as a query, taken from the calculus's own `Ok`
/// answers along a path: whether the obligation was ever opened, whether the root was
/// cancelled, and whether A and B requested their own cancellation (`cancel_phase` says
/// "requested" without saying by whom).
#[derive(Debug, Clone, Copy, Default)]
struct Seen {
    opened: bool,
    cancelled: bool,
    requested_a: bool,
    requested_b: bool,
}

impl Seen {
    fn after(mut self, action: &str) -> Self {
        match action {
            "OpenSub" => self.opened = true,
            "Cancel" => self.cancelled = true,
            "RequestCancel" => self.requested_a = true,
            "RequestCancelB" => self.requested_b = true,
            _ => {}
        }
        self
    }
}

/// Replay a path of steps from a fresh tree.
fn replay(path: &[&'static str]) -> (RegionTree, Seen) {
    let mut tree = RegionTree::new();
    let root = tree.root();
    for _ in 0..2 {
        tree.spawn(root, Resumability::Resumable)
            .expect("root is open");
    }
    let mut seen = Seen::default();
    for action in path {
        apply(&mut tree, action).expect("a replayed path was admitted before");
        seen = seen.after(action);
    }
    (tree, seen)
}

fn lifecycle(tree: &RegionTree, worker: WorkerId) -> i64 {
    match tree.worker_state(worker).expect("admitted") {
        WorkerState::Created => CREATED,
        WorkerState::Running => RUNNING,
        WorkerState::Suspended => SUSPENDED,
        WorkerState::Completed => COMPLETED,
        WorkerState::Failed(_) => FAILED,
        WorkerState::Cancelled => CANCELLED,
    }
}

fn acknowledged(tree: &RegionTree, worker: WorkerId) -> i64 {
    i64::from(tree.cancel_phase(worker).expect("admitted") == CancelPhase::Acknowledged)
}

/// The calculus's abstract state, in the oracle's variable order.
fn abstraction(tree: &RegionTree, seen: Seen) -> Vec<i64> {
    let held = |k: u32| {
        i64::from(
            tree.ledger()
                .holds(Obligation::staged_publication(A, PublicationSlot::at(k))),
        )
    };
    let committed = i64::from(tree.evidence(A).expect("admitted").committed() > 0);
    let holder = tree.substrate_holder(SubstrateId::at(0));
    let sub = match (holder, seen.opened) {
        (Some(_), _) => 1,
        (None, true) => 2,
        (None, false) => 0,
    };
    vec![
        lifecycle(tree, WB),
        committed,
        i64::from(seen.cancelled),
        i64::from(holder == Some(WB)),
        acknowledged(tree, A),
        acknowledged(tree, WB),
        held(0),
        held(1),
        held(2),
        i64::from(seen.requested_a),
        i64::from(seen.requested_b),
        lifecycle(tree, A),
        sub,
    ]
}

/// Every state's full successor row over the steps (`None` for a refused step).
type Row = BTreeMap<&'static str, Option<Vec<i64>>>;

/// Explore the calculus's abstract state space breadth-first.
fn explore_calculus() -> BTreeMap<Vec<i64>, Row> {
    let (tree, seen) = replay(&[]);
    let initial = abstraction(&tree, seen);
    let mut paths: BTreeMap<Vec<i64>, Vec<&'static str>> = BTreeMap::new();
    paths.insert(initial.clone(), Vec::new());
    let mut queue = VecDeque::from([initial]);
    let mut rows = BTreeMap::new();
    while let Some(state) = queue.pop_front() {
        let path = paths[&state].clone();
        let mut row = Row::new();
        for action in ACTIONS {
            let (mut tree, seen) = replay(&path);
            let before = (
                tree.ledger().opened(),
                tree.ledger().discharged(),
                tree.ledger().outstanding(),
                tree.substrate_holder(SubstrateId::at(0)),
            );
            let outcome = apply(&mut tree, action);
            let target = if outcome.is_ok() {
                let target = abstraction(&tree, seen.after(action));
                // Conservation, checked on every admitted step: the ledger owes exactly
                // each non-terminal worker's termination, one entry per staged slot, the
                // open adapter obligation, and the cancellation's finalization.
                let owed = i64::from(target[ST] <= SUSPENDED)
                    + i64::from(target[B] <= SUSPENDED)
                    + target[P0]
                    + target[P0 + 1]
                    + target[P0 + 2]
                    + i64::from(target[SUB] == 1)
                    + target[CX];
                assert_eq!(
                    i64::try_from(tree.ledger().outstanding().len()).expect("small"),
                    owed,
                    "{path:?} + {action}: the ledger does not match the state"
                );
                if !paths.contains_key(&target) {
                    let mut next = path.clone();
                    next.push(action);
                    paths.insert(target.clone(), next);
                    queue.push_back(target.clone());
                }
                Some(target)
            } else {
                let after = (
                    tree.ledger().opened(),
                    tree.ledger().discharged(),
                    tree.ledger().outstanding(),
                    tree.substrate_holder(SubstrateId::at(0)),
                );
                assert_eq!(
                    before, after,
                    "{path:?} + {action}: a refusal changed the ledger or the holder"
                );
                assert_eq!(
                    abstraction(&tree, seen),
                    state,
                    "{path:?} + {action}: a refusal changed the state"
                );
                None
            };
            row.insert(action, target);
        }
        rows.insert(state, row);
    }
    rows
}

/// The oracle's reachable states and successor rows, in the same shape.
fn explore_oracle(model: &Model) -> BTreeMap<Vec<i64>, Row> {
    for (index, name) in VARIABLES.iter().enumerate() {
        assert_eq!(model.variable_index(name), Some(index));
    }
    let exploration =
        explore(model, Bounds::new(1_000_000, 256, 100_000_000)).expect("the oracle explores");
    let reachable = exploration
        .closed()
        .expect("the oracle's space is finite and closed");
    let mut rows = BTreeMap::new();
    for state in reachable.states() {
        let mut row = Row::new();
        for action in ACTIONS {
            let index = model.action_index(action).expect("declared");
            let targets = model
                .action_successors(index, state)
                .expect("the oracle evaluates");
            assert!(targets.len() <= 1, "every oracle action is deterministic");
            row.insert(
                action,
                targets.first().map(|t: &State| t.as_slice().to_vec()),
            );
        }
        rows.insert(state.as_slice().to_vec(), row);
    }
    rows
}

/// The reachable abstract states.
const STATES: usize = 8_020;
/// Admitted transitions over those states (of 8,020 × 36 = 288,720 checked).
const ADMITTED: usize = 58_455;

#[test]
fn the_calculus_and_an_independent_reference_model_agree_on_every_staging_transition() {
    let calculus = explore_calculus();
    let reference = explore_oracle(&oracle(Guards::CORRECT));

    let calculus_states: BTreeSet<_> = calculus.keys().cloned().collect();
    let reference_states: BTreeSet<_> = reference.keys().cloned().collect();
    assert_eq!(
        calculus_states, reference_states,
        "the two implementations reach different abstract states"
    );

    let mut admitted = 0_usize;
    for (state, row) in &calculus {
        assert_eq!(row, &reference[state], "at {state:?}");
        admitted += row.values().filter(|target| target.is_some()).count();
    }
    assert_eq!(
        (calculus_states.len(), admitted),
        (STATES, ADMITTED),
        "the reachable space and the admitted transitions are pinned, so a guard change \
         cannot pass quietly"
    );
    // Multi-staging is reachable: some state holds all three slots at once.
    assert!(
        calculus_states
            .iter()
            .any(|s| s[P0] + s[P0 + 1] + s[P0 + 2] == 3)
    );
    // Correction 53: B holds the obligation inside a cancelled region, requested and not
    // acknowledged, and still acts: it may commit it or hand it back.
    let requested = calculus
        .iter()
        .find(|(s, _)| s[SUB] == 1 && s[H] == 1 && s[CX] == 1 && s[B] == RUNNING && s[KB] == 0)
        .expect("B holds the obligation in a cancelled region, not acknowledged");
    assert!(requested.1["DischargeCommitB"].is_some());
    assert!(requested.1["TransferBA"].is_some());
    // After its acknowledgement B may abort it, and may not commit it or hand it back.
    let acknowledged = calculus
        .iter()
        .find(|(s, _)| s[SUB] == 1 && s[H] == 1 && s[B] == RUNNING && s[KB] == 1)
        .expect("B holds the obligation after acknowledging");
    assert!(acknowledged.1["DischargeAbortB"].is_some());
    assert!(acknowledged.1["DischargeCommitB"].is_none());
    assert!(acknowledged.1["TransferBA"].is_none());
    // The single-task cancellation is reached in an open region.
    assert!(
        calculus_states
            .iter()
            .any(|s| s[ST] == CANCELLED && s[CX] == 0)
    );
}

/// How many states the calculus and the mutant disagree on.
fn disagreements(calculus: &BTreeMap<Vec<i64>, Row>, mutant: &BTreeMap<Vec<i64>, Row>) -> usize {
    let states: BTreeSet<_> = calculus.keys().chain(mutant.keys()).collect();
    states
        .into_iter()
        .filter(|state| calculus.get(*state) != mutant.get(*state))
        .count()
}

#[test]
fn a_mutated_reference_guard_is_told_apart() {
    let calculus = explore_calculus();
    let c = Guards::CORRECT;
    let mutants = [
        (
            "an unnamed abort with nothing staged",
            Guards {
                abort_needs_staging: false,
                ..c
            },
        ),
        (
            "an open by a worker that has not begun",
            Guards {
                open_needs_begun: false,
                ..c
            },
        ),
        (
            "a running-only discharge",
            Guards {
                discharge_admits_parked: false,
                ..c
            },
        ),
        (
            "a committed discharge after the acknowledgement",
            Guards {
                commit_refused_after_ack: false,
                ..c
            },
        ),
        (
            "a self-transfer",
            Guards {
                transfer_refuses_self: false,
                ..c
            },
        ),
        (
            "a transfer by or to a worker that acknowledged",
            Guards {
                transfer_needs_acting: false,
                ..c
            },
        ),
        // Correction 51's strictness, which correction 53 relaxed: the calculus must now
        // admit a transfer in a cancelled region before either party acknowledges.
        (
            "no transfer anywhere in a cancelled region",
            Guards {
                region_cancel_blocks_transfer: true,
                ..c
            },
        ),
        (
            "an acknowledgement nobody requested",
            Guards {
                ack_needs_request: false,
                ..c
            },
        ),
        (
            "an ordinary step after the acknowledgement",
            Guards {
                acknowledged_only_drains: false,
                ..c
            },
        ),
    ];
    for (name, guards) in mutants {
        assert!(
            disagreements(&calculus, &explore_oracle(&oracle(guards))) > 0,
            "the differential cannot tell an oracle that admits {name} from the calculus"
        );
    }
}
