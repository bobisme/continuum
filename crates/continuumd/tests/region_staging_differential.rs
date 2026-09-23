//! Differential: the region calculus's staging, abort and adapter-obligation rules against
//! an independent transition model explored by `continuum_engine_reference` (RFC 0026
//! correction 51, bn-2318t, cr-1ckhmw).
//!
//! The oracle is written from the correction's text, not from `region.rs`. It models two
//! workers in the root region:
//!
//! - worker A, with its whole lifecycle, three staging slots, and a "has committed" flag;
//! - worker B, with the lifecycle steps a transfer partner needs;
//! - the root region's cancellation;
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
//! observes the tree only through `worker_state`, `evidence`, `ledger().holds` and
//! `substrate_holder`. Six mutated oracle guards are told apart, so agreement is not
//! vacuous.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use continuum_engine_reference::{
    ActionDecl, BoolExpr, Bounds, CmpOp, IntExpr, Model, ModelBuilder, State, explore,
};
use continuum_task::region::obligation::{
    Obligation, SubstrateId, SubstrateKind, SubstrateObligation, SubstrateOutcome,
};
use continuum_task::region::worker::{
    FailureReason, PublicationSlot, Resumability, WorkerId, WorkerState, WorkerStep,
};
use continuum_task::region::{RegionFault, RegionTree};

/// The abstract state vector, in the oracle's variable order (the model orders its
/// variables by name): B's lifecycle, A's "has committed", the region's cancellation,
/// the obligation's holder (0 = A, 1 = B), A's three slots, A's lifecycle, and the
/// obligation's phase (0 never opened, 1 open, 2 discharged).
const VARIABLES: [&str; 9] = ["b", "c", "cx", "h", "p0", "p1", "p2", "st", "sub"];

/// Positions in [`VARIABLES`].
const B: usize = 0;
const CX: usize = 2;
const H: usize = 3;
const P0: usize = 4;
const ST: usize = 7;
const SUB: usize = 8;

/// Lifecycle codes: created, running, suspended, completed, failed.
const CREATED: i64 = 0;
const RUNNING: i64 = 1;
const SUSPENDED: i64 = 2;
const COMPLETED: i64 = 3;
const FAILED: i64 = 4;

/// The steps, named once for both sides. `OpenSub` in a state where the obligation was
/// already opened is the identity-reuse mutant; `TransferAA` is the self-transfer.
const ACTIONS: [&str; 30] = [
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
    "BeginB",
    "SuspendB",
    "ResumeB",
    "CompleteB",
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

// --- the oracle: RFC 0026 correction 51 as a guarded transition model ------------------

fn var(name: &str) -> IntExpr {
    IntExpr::var(name)
}

fn eq(name: &str, value: i64) -> BoolExpr {
    BoolExpr::compare(CmpOp::Eq, var(name), IntExpr::constant(value))
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

/// Created or running — may fail.
fn live() -> BoolExpr {
    BoolExpr::compare(CmpOp::Le, var("st"), IntExpr::constant(RUNNING))
}

/// `lifecycle` has begun and not terminated: running or parked. The correction's
/// `WorkerState::holds_substrate`.
fn holds(lifecycle: &str) -> BoolExpr {
    BoolExpr::or(eq(lifecycle, RUNNING), eq(lifecycle, SUSPENDED))
}

/// `lifecycle` holds substrate and its region is not cancelling: the A7 model's
/// "acting", which a transfer needs of both parties.
fn acting(lifecycle: &str) -> BoolExpr {
    BoolExpr::and(holds(lifecycle), eq("cx", 0))
}

fn and3(a: BoolExpr, b: BoolExpr, c: BoolExpr) -> BoolExpr {
    BoolExpr::and(BoolExpr::and(a, b), c)
}

fn set(name: &'static str, value: i64) -> (&'static str, IntExpr) {
    (name, IntExpr::constant(value))
}

/// The guards the mutant controls flip; [`Guards::CORRECT`] is the correction's text.
#[derive(Clone, Copy)]
struct Guards {
    /// An unnamed abort needs exactly one staged publication.
    abort_needs_staging: bool,
    /// An adapter-obligation open needs a holder that has begun.
    open_needs_begun: bool,
    /// A discharge admits a parked holder (the substrate discharges during
    /// cancellation, before the holder is terminal).
    discharge_admits_parked: bool,
    /// A committed discharge is refused in a cancelling region; an abort is cleanup.
    commit_refused_in_cancel: bool,
    /// A transfer to the holder itself is refused.
    transfer_refuses_self: bool,
    /// A transfer needs both parties acting: not in a cancelling region.
    transfer_needs_acting: bool,
}

impl Guards {
    const CORRECT: Self = Self {
        abort_needs_staging: true,
        open_needs_begun: true,
        discharge_admits_parked: true,
        commit_refused_in_cancel: true,
        transfer_refuses_self: true,
        transfer_needs_acting: true,
    };
}

/// The oracle, with `guards` deciding the flippable guards.
#[allow(clippy::too_many_lines)]
fn oracle(guards: Guards) -> Model {
    let Guards {
        abort_needs_staging,
        open_needs_begun,
        discharge_admits_parked,
        commit_refused_in_cancel,
        transfer_refuses_self,
        transfer_needs_acting,
    } = guards;
    let slot = |k: usize| ["p0", "p1", "p2"][k];
    let party = |lifecycle: &str| {
        if transfer_needs_acting {
            acting(lifecycle)
        } else {
            holds(lifecycle)
        }
    };
    let mut builder = ModelBuilder::new()
        .variable("b", CREATED, COMPLETED)
        .variable("c", 0, 1)
        .variable("cx", 0, 1)
        .variable("h", 0, 1)
        .variable("p0", 0, 1)
        .variable("p1", 0, 1)
        .variable("p2", 0, 1)
        .variable("st", CREATED, FAILED)
        .variable("sub", 0, 2)
        .initial_state(&[
            ("b", CREATED),
            ("c", 0),
            ("cx", 0),
            ("h", 0),
            ("p0", 0),
            ("p1", 0),
            ("p2", 0),
            ("st", CREATED),
            ("sub", 0),
        ])
        // Created → Running.
        .action(ActionDecl::deterministic(
            "Begin",
            eq("st", CREATED),
            vec![set("st", RUNNING)],
        ))
        // The one-in-flight reserve stages the primary slot, only when nothing is staged.
        .action(ActionDecl::deterministic(
            "Reserve",
            BoolExpr::and(running(), staged_is(0)),
            vec![set("p0", 1)],
        ));
    for k in 0..3 {
        builder = builder.action(ActionDecl::deterministic(
            ["ReserveSlot0", "ReserveSlot1", "ReserveSlot2"][k],
            BoolExpr::and(running(), eq(slot(k), 0)),
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
            BoolExpr::and(running(), staged_is(1)),
            commit,
        ))
        .action(ActionDecl::deterministic(
            "Abort",
            if abort_needs_staging {
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
            BoolExpr::and(running(), eq(slot(k), 1)),
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
    let discharge = |name: &str, lifecycle: &str, holder: i64, commit: bool| {
        let party = if discharge_admits_parked {
            holds(lifecycle)
        } else {
            eq(lifecycle, RUNNING)
        };
        let region = if commit && commit_refused_in_cancel {
            eq("cx", 0)
        } else {
            BoolExpr::constant(true)
        };
        ActionDecl::deterministic(
            name,
            and3(BoolExpr::and(party, region), eq("sub", 1), eq("h", holder)),
            vec![set("sub", 2), set("h", 0)],
        )
    };
    let self_transfer = if transfer_refuses_self {
        BoolExpr::constant(false)
    } else {
        and3(eq("sub", 1), eq("h", 0), party("st"))
    };
    builder
        // Park and finish only with nothing staged.
        .action(ActionDecl::deterministic(
            "Suspend",
            BoolExpr::and(running(), staged_is(0)),
            vec![set("st", SUSPENDED)],
        ))
        .action(ActionDecl::deterministic(
            "Resume",
            eq("st", SUSPENDED),
            vec![set("st", RUNNING)],
        ))
        .action(ActionDecl::deterministic(
            "Complete",
            BoolExpr::and(running(), staged_is(0)),
            vec![set("st", COMPLETED)],
        ))
        // A failure discards every staged slot.
        .action(ActionDecl::deterministic("Fail", live(), fail))
        // Worker B: the lifecycle a transfer partner needs.
        .action(ActionDecl::deterministic(
            "BeginB",
            eq("b", CREATED),
            vec![set("b", RUNNING)],
        ))
        .action(ActionDecl::deterministic(
            "SuspendB",
            eq("b", RUNNING),
            vec![set("b", SUSPENDED)],
        ))
        .action(ActionDecl::deterministic(
            "ResumeB",
            eq("b", SUSPENDED),
            vec![set("b", RUNNING)],
        ))
        .action(ActionDecl::deterministic(
            "CompleteB",
            eq("b", RUNNING),
            vec![set("b", COMPLETED)],
        ))
        // The root region's cancellation request; asking twice changes nothing.
        .action(ActionDecl::deterministic(
            "Cancel",
            BoolExpr::constant(true),
            vec![set("cx", 1)],
        ))
        // Adapter obligation s0: opened once by A while it has begun and not terminated,
        // in an open region; an identity opened before is never reopened.
        .action(ActionDecl::deterministic(
            "OpenSub",
            and3(
                if open_needs_begun {
                    holds("st")
                } else {
                    BoolExpr::compare(CmpOp::Le, var("st"), IntExpr::constant(SUSPENDED))
                },
                eq("sub", 0),
                eq("cx", 0),
            ),
            vec![set("sub", 1), set("h", 0)],
        ))
        // Discharged once, by its holder, which has begun and not terminated. The region
        // may be cancelling — that is where cleanup discharges happen — but there only an
        // abort is admitted.
        .action(discharge("DischargeCommitA", "st", 0, true))
        .action(discharge("DischargeAbortA", "st", 0, false))
        .action(discharge("DischargeCommitB", "b", 1, true))
        .action(discharge("DischargeAbortB", "b", 1, false))
        // Handed on by its acting holder to another acting worker.
        .action(ActionDecl::deterministic(
            "TransferAB",
            and3(
                BoolExpr::and(eq("sub", 1), eq("h", 0)),
                party("st"),
                party("b"),
            ),
            vec![set("h", 1)],
        ))
        .action(ActionDecl::deterministic(
            "TransferBA",
            and3(
                BoolExpr::and(eq("sub", 1), eq("h", 1)),
                party("b"),
                party("st"),
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
        "BeginB" => (WB, WorkerStep::Begin),
        "SuspendB" => (WB, WorkerStep::Suspend),
        "ResumeB" => (WB, WorkerStep::Resume),
        "CompleteB" => (WB, WorkerStep::Complete),
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

/// Replay a path of steps from a fresh tree. Returns the tree, whether the adapter
/// obligation was ever opened, and whether the root was cancelled — the two facts the
/// API does not expose as a query, taken from the calculus's own `Ok` answers.
fn replay(path: &[&'static str]) -> (RegionTree, bool, bool) {
    let mut tree = RegionTree::new();
    let root = tree.root();
    for _ in 0..2 {
        tree.spawn(root, Resumability::Resumable)
            .expect("root is open");
    }
    let mut opened = false;
    let mut cancelled = false;
    for action in path {
        apply(&mut tree, action).expect("a replayed path was admitted before");
        opened |= *action == "OpenSub";
        cancelled |= *action == "Cancel";
    }
    (tree, opened, cancelled)
}

fn lifecycle(tree: &RegionTree, worker: WorkerId) -> i64 {
    match tree.worker_state(worker).expect("admitted") {
        WorkerState::Created => CREATED,
        WorkerState::Running => RUNNING,
        WorkerState::Suspended => SUSPENDED,
        WorkerState::Completed => COMPLETED,
        WorkerState::Failed(_) => FAILED,
        WorkerState::Cancelled => panic!("no drain is driven here"),
    }
}

/// The calculus's abstract state, in the oracle's variable order.
fn abstraction(tree: &RegionTree, opened: bool, cancelled: bool) -> Vec<i64> {
    let held = |k: u32| {
        i64::from(
            tree.ledger()
                .holds(Obligation::staged_publication(A, PublicationSlot::at(k))),
        )
    };
    let committed = i64::from(tree.evidence(A).expect("admitted").committed() > 0);
    let holder = tree.substrate_holder(SubstrateId::at(0));
    let sub = match (holder, opened) {
        (Some(_), _) => 1,
        (None, true) => 2,
        (None, false) => 0,
    };
    vec![
        lifecycle(tree, WB),
        committed,
        i64::from(cancelled),
        i64::from(holder == Some(WB)),
        held(0),
        held(1),
        held(2),
        lifecycle(tree, A),
        sub,
    ]
}

/// Every state's full successor row over the steps (`None` for a refused step).
type Row = BTreeMap<&'static str, Option<Vec<i64>>>;

/// Explore the calculus's abstract state space breadth-first.
fn explore_calculus() -> BTreeMap<Vec<i64>, Row> {
    let (tree, opened, cancelled) = replay(&[]);
    let initial = abstraction(&tree, opened, cancelled);
    let mut paths: BTreeMap<Vec<i64>, Vec<&'static str>> = BTreeMap::new();
    paths.insert(initial.clone(), Vec::new());
    let mut queue = VecDeque::from([initial]);
    let mut rows = BTreeMap::new();
    while let Some(state) = queue.pop_front() {
        let path = paths[&state].clone();
        let mut row = Row::new();
        for action in ACTIONS {
            let (mut tree, opened, cancelled) = replay(&path);
            let before = (
                tree.ledger().opened(),
                tree.ledger().discharged(),
                tree.ledger().outstanding(),
                tree.substrate_holder(SubstrateId::at(0)),
            );
            let outcome = apply(&mut tree, action);
            let target = if outcome.is_ok() {
                let target = abstraction(
                    &tree,
                    opened || action == "OpenSub",
                    cancelled || action == "Cancel",
                );
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
                    abstraction(&tree, opened, cancelled),
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
        explore(model, Bounds::new(100_000, 128, 10_000_000)).expect("the oracle explores");
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
const STATES: usize = 668;
/// Admitted transitions over those states (of 668 × 30 = 20,040 checked).
const ADMITTED: usize = 5_084;

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
    // The transfer and outcome cases are reached: B holds the obligation inside a
    // cancelled region, where it may abort it but may not commit it or hand it back.
    let cancelled_with_b = calculus
        .iter()
        .find(|(s, _)| s[SUB] == 1 && s[H] == 1 && s[CX] == 1 && s[B] == RUNNING)
        .expect("B holds the obligation in a cancelled region");
    assert!(cancelled_with_b.1["DischargeAbortB"].is_some());
    assert!(cancelled_with_b.1["DischargeCommitB"].is_none());
    assert!(cancelled_with_b.1["TransferBA"].is_none());
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
    let mutants = [
        (
            "an unnamed abort with nothing staged",
            Guards {
                abort_needs_staging: false,
                ..Guards::CORRECT
            },
        ),
        (
            "an open by a worker that has not begun",
            Guards {
                open_needs_begun: false,
                ..Guards::CORRECT
            },
        ),
        // The rule cr-1ckhmw found too strict for cleanup during cancellation.
        (
            "a running-only discharge",
            Guards {
                discharge_admits_parked: false,
                ..Guards::CORRECT
            },
        ),
        // The rules cr-1ckhmw found too permissive against the A7 model.
        (
            "a committed discharge during cancellation",
            Guards {
                commit_refused_in_cancel: false,
                ..Guards::CORRECT
            },
        ),
        (
            "a self-transfer",
            Guards {
                transfer_refuses_self: false,
                ..Guards::CORRECT
            },
        ),
        (
            "a transfer by or to a party in a cancelling region",
            Guards {
                transfer_needs_acting: false,
                ..Guards::CORRECT
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
