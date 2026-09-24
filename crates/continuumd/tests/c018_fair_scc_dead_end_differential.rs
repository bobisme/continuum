//! Differential for the bn-2npu fix: the reference engine's deadlock report against the
//! temporal kernel's fair-SCC exclusion verdict (C018 audit).
//!
//! # The two implementations
//!
//! - **Oracle:** `continuum_engine_reference`. `bfs::explore` walks the model and
//!   `checking::check` under `DeadlockPolicy::Defect` reports every reachable state
//!   with no enabled action. The model's `goal` predicate says which of them are goal
//!   states.
//! - **Subject:** `continuum_kernel_temporal`, reached through the composed checker
//!   `continuum_certificate::continuum_kernel_temporal`. It reads a fair-SCC exclusion
//!   certificate written in this file from the same exploration, and never sees the
//!   model.
//!
//! The two share no code: the kernel depends on nothing, and the certificate bytes are
//! written here from the kernel's documented grammar.
//!
//! # The relation
//!
//! The claim is `eventually goal`. An execution that stops at a non-goal state before
//! any goal visit refutes it: under each RFC 0015 completion policy that applies to a
//! closed relation it never reaches the goal, and every action is disabled at an empty
//! row, so no weak fairness assumption excludes it. A dead end after a goal visit does
//! not refute it (cr-10mg1y).
//!
//! The oracle side builds the same model with every action also guarded by `!goal`, so
//! goal states become terminal, and asks the reference engine for its deadlocks. A
//! deadlock there at a non-goal state is exactly a dead end reached without passing
//! through a goal. The kernel must answer `progress-deadlock` exactly then.
//!
//! `stop-short` passes the goal `c = 1` and then stops at `c = 3`: the claim holds, and
//! the kernel accepts. `branch-dead-end` stops at `(2, 0)` on the branch that never
//! marks: the kernel refuses. Before bn-2npu the kernel accepted both. Between
//! 09d0d28 and cr-10mg1y it refused both.
//!
//! # How independent the two sides are
//!
//! Not fully. The certificate's rows come from the model's successor evaluator, so an
//! empty row matches "no enabled action" by construction. What the test compares is
//! the reference engine's goal-stopped reachability and deadlock judgement against the
//! kernel's own goal-stopped reachability and dead-end rule over the carried bytes.

use continuum_certificate::continuum_kernel_temporal::{Verdict, check_certificate};
use continuum_engine_reference::bfs::{self, Bounds, Exploration};
use continuum_engine_reference::checking::{self, DeadlockPolicy, Obligations};
use continuum_engine_reference::diehard;
use continuum_engine_reference::expr::{BoolExpr, CmpOp, IntExpr};
use continuum_engine_reference::model::{ActionDecl, Model, ModelBuilder};

/// `guard`, and, when `stop` is set, also `!goal`: the goal-stopped variant.
fn guarded(guard: BoolExpr, goal: &BoolExpr, stop: bool) -> BoolExpr {
    if stop {
        BoolExpr::and(guard, BoolExpr::negate(goal.clone()))
    } else {
        guard
    }
}

/// A counter that steps `c` from `0` to `3` and stops, with `goal` at `c == goal`.
fn counter(goal: i64, stop: bool) -> Model {
    let c = || IntExpr::var("c");
    let target = BoolExpr::compare(CmpOp::Eq, c(), IntExpr::constant(goal));
    ModelBuilder::new()
        .variable("c", 0, 3)
        .initial_state(&[("c", 0)])
        .action(ActionDecl::deterministic(
            "step",
            guarded(
                BoolExpr::compare(CmpOp::Lt, c(), IntExpr::constant(3)),
                &target,
                stop,
            ),
            vec![("c", IntExpr::plus(c(), IntExpr::constant(1)))],
        ))
        .predicate("goal", target)
        .build()
        .expect("the counter is a valid model")
}

/// `x` climbs to `2`; at `x == 1, y == 0` a branch may set `y`. Both `x == 2` states
/// are dead ends; `goal` names which of them count.
fn branch(goal: BoolExpr, stop: bool) -> Model {
    let x = || IntExpr::var("x");
    let y = || IntExpr::var("y");
    ModelBuilder::new()
        .variable("x", 0, 2)
        .variable("y", 0, 1)
        .initial_state(&[("x", 0), ("y", 0)])
        .action(ActionDecl::deterministic(
            "climb",
            guarded(
                BoolExpr::compare(CmpOp::Lt, x(), IntExpr::constant(2)),
                &goal,
                stop,
            ),
            vec![("x", IntExpr::plus(x(), IntExpr::constant(1)))],
        ))
        .action(ActionDecl::deterministic(
            "mark",
            guarded(
                BoolExpr::and(
                    BoolExpr::compare(CmpOp::Eq, x(), IntExpr::constant(1)),
                    BoolExpr::compare(CmpOp::Eq, y(), IntExpr::constant(0)),
                ),
                &goal,
                stop,
            ),
            vec![("y", IntExpr::constant(1))],
        ))
        .predicate("goal", goal)
        .build()
        .expect("the branch is a valid model")
}

/// (name, the model, its goal-stopped variant, the goal predicate's index).
fn corpus() -> Vec<(&'static str, Model, Model, usize)> {
    let x = || IntExpr::var("x");
    let y = || IntExpr::var("y");
    let both = || BoolExpr::compare(CmpOp::Eq, x(), IntExpr::constant(2));
    let marked = || BoolExpr::compare(CmpOp::Eq, y(), IntExpr::constant(1));
    let die_hard = diehard::model().expect("the Die Hard transcription is a valid model");
    let not_solved = die_hard
        .predicate_index(diehard::NOT_SOLVED)
        .expect("Die Hard declares NotSolved");
    vec![
        ("reach-the-end", counter(3, false), counter(3, true), 0),
        ("stop-short", counter(1, false), counter(1, true), 0),
        (
            "branch-both-goals",
            branch(both(), false),
            branch(both(), true),
            0,
        ),
        (
            "branch-dead-end",
            branch(marked(), false),
            branch(marked(), true),
            0,
        ),
        // Die Hard has no dead end: every action is always enabled. `NotSolved` stands
        // in for `goal`; the stopped variant is Die Hard itself, since the reference
        // engine judges deadlocks over it and it has none.
        (
            "die-hard",
            die_hard,
            diehard::model().expect("valid"),
            not_solved,
        ),
    ]
}

// --- the certificate, written from the kernel's documented grammar -------------------

struct Bytes(Vec<u8>);

impl Bytes {
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_be_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_be_bytes());
    }
    fn i64(&mut self, v: i64) {
        self.0.extend_from_slice(&v.to_be_bytes());
    }
    fn token(&mut self, s: &str) {
        self.u16(u16::try_from(s.len()).expect("short token"));
        self.0.extend_from_slice(s.as_bytes());
    }
    fn count(&mut self, n: usize) {
        self.u32(u32::try_from(n).expect("small count"));
    }
    fn states<'a>(&mut self, states: impl ExactSizeIterator<Item = &'a [i64]>) {
        self.count(states.len());
        for state in states {
            for value in state {
                self.i64(*value);
            }
        }
    }
}

/// A `CONTTMPC` fair-SCC exclusion certificate for `eventually goal`, no fair actions.
fn certificate(model: &Model, goal_predicate: usize) -> Vec<u8> {
    let Exploration::Complete(reachable) =
        bfs::explore(model, Bounds::CERTIFIABLE).expect("the corpus evaluates everywhere")
    else {
        panic!("the corpus explores completely");
    };
    let states = reachable.states();
    let goal: Vec<&[i64]> = states
        .iter()
        .filter(|s| {
            model
                .evaluate_predicate(goal_predicate, s)
                .expect("evaluates")
        })
        .map(|s| s.as_slice())
        .collect();

    let mut out = Bytes(Vec::new());
    out.0.extend_from_slice(b"CONTTMPC");
    out.u16(1); // wire epoch
    out.u16(2); // fair-scc-exclusion
    for token in [
        "blake3:c018-differential",
        "continuum-semantics-1",
        "blake3:eventually-goal",
        "blake3:c018-scope",
        "blake3:empty-assumptions",
        "c018-differential/0",
    ] {
        out.token(token);
    }
    out.u16(1); // schema epoch
    out.u16(0); // domain packs
    out.u16(u16::try_from(model.variables().len()).expect("few variables"));
    for variable in model.variables() {
        out.token(variable.name().as_str());
        out.i64(variable.domain().lo());
        out.i64(variable.domain().hi());
    }
    out.states(states.iter().map(|s| s.as_slice()));
    out.u16(1); // eventually-state-set
    out.states(goal.iter().copied());
    out.states(model.initial_states().iter().map(|s| s.as_slice()));
    out.u16(u16::try_from(model.actions().len()).expect("few actions"));
    for action in model.actions() {
        out.token(action.name().as_str());
    }
    out.u16(1); // weak fairness
    out.u16(0); // no fair actions
    for state in states {
        let row = model.successors(state).expect("evaluates");
        out.count(row.len());
        for step in row {
            out.u16(u16::try_from(step.action()).expect("few actions"));
            for value in step.target().as_slice() {
                out.i64(*value);
            }
        }
    }
    out.0
}

/// The oracle's answer: does the reference engine, over the goal-stopped model, report
/// a deadlock at a non-goal state?
fn reference_reports_a_non_goal_dead_end(model: &Model, goal_predicate: usize) -> bool {
    let exploration = bfs::explore(model, Bounds::CERTIFIABLE).expect("evaluates");
    let report = checking::check(
        model,
        &exploration,
        &Obligations::new(DeadlockPolicy::Defect),
    )
    .expect("no predicate obligations to resolve");
    report.deadlock().deadlocks().iter().any(|deadlock| {
        !model
            .evaluate_predicate(goal_predicate, deadlock.state())
            .expect("evaluates")
    })
}

fn kernel_reports_a_dead_end(bytes: &[u8]) -> bool {
    match check_certificate(bytes) {
        Verdict::Rejected(rejection) => rejection.reason() == "progress-deadlock",
        Verdict::Verified(_) | Verdict::Unsupported(_) => false,
    }
}

#[test]
fn the_kernel_refuses_a_dead_end_exactly_when_the_reference_engine_reports_one() {
    let mut refused = Vec::new();
    for (name, model, stopped, goal) in corpus() {
        let reference = reference_reports_a_non_goal_dead_end(&stopped, goal);
        let kernel = kernel_reports_a_dead_end(&certificate(&model, goal));
        assert_eq!(
            kernel,
            reference,
            "{name}: kernel {:?}",
            check_certificate(&certificate(&model, goal))
        );
        if kernel {
            refused.push(name);
        }
    }
    // Anti-vacuity: the corpus has both answers.
    assert_eq!(refused, ["branch-dead-end"]);
}

#[test]
fn a_dead_end_after_the_goal_is_accepted_and_the_reference_engine_sees_one_without_the_stop() {
    // `stop-short` really has a dead end: the reference engine over the unstopped model
    // reports it. It lies after the goal, so the kernel accepts. This is the case
    // cr-10mg1y found the kernel refusing.
    let (_, model, _, goal) = corpus()
        .into_iter()
        .find(|(name, ..)| *name == "stop-short")
        .expect("in the corpus");
    assert!(reference_reports_a_non_goal_dead_end(&model, goal));
    assert!(matches!(
        check_certificate(&certificate(&model, goal)),
        Verdict::Verified(_)
    ));
}
