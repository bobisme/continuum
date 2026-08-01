//! What deterministic exploration promises, on models built to test one promise each
//! (PR 8, IMPL-02).
//!
//! Die Hard is in `tests/bfs_diehard.rs`; it is the corpus model and its numbers are
//! frozen elsewhere. The models here are the opposite kind: each is the smallest
//! declaration that exercises one rule, and each is *linear* in the rule it tests —
//! a counter chain, a self-loop, a fork, a runaway increment. Nothing here is
//! combinatorial, and nothing here needs to be, because the rules are independent.
//!
//! The suite covers: the complete case, the three bounds and their exact partial
//! results, the two error arms, deadlock surfacing, and determinism.

use continuum_engine_reference::bfs::{self, Bound, Bounds, Exploration, ExplorationError};
use continuum_engine_reference::expr::{ArithOp, BoolExpr, CmpOp, EvalError, IntExpr};
use continuum_engine_reference::ident::Ident;
use continuum_engine_reference::model::{ActionDecl, EvaluationError, Model, ModelBuilder, State};

// ---------------------------------------------------------------------------
// the models
// ---------------------------------------------------------------------------

/// A chain: `n` counts from `0` to `limit`, one step at a time, and then stops.
///
/// `limit + 1` states, depth `limit`, `limit` transitions, and the last state is a
/// deadlock — the guard is what makes it one, and it is the only thing that does.
fn counter(limit: i64) -> Model {
    ModelBuilder::new()
        .variable("n", 0, limit)
        .initial_state(&[("n", 0)])
        .action(ActionDecl::deterministic(
            "Step",
            BoolExpr::compare(CmpOp::Lt, IntExpr::var("n"), IntExpr::constant(limit)),
            vec![("n", IntExpr::plus(IntExpr::var("n"), IntExpr::constant(1)))],
        ))
        .build()
        .expect("the counter is a valid model")
}

/// The same chain with the guard removed, so at `n == limit` the update leaves the
/// declared domain. docs/16 PO-MOD-003 failing at a genuinely reachable state.
fn runaway(limit: i64) -> Model {
    ModelBuilder::new()
        .variable("n", 0, limit)
        .initial_state(&[("n", 0)])
        .action(ActionDecl::deterministic(
            "Bump",
            BoolExpr::Const(true),
            vec![("n", IntExpr::plus(IntExpr::var("n"), IntExpr::constant(1)))],
        ))
        .build()
        .expect("the runaway counter is a valid model")
}

/// One state, one always-enabled action that assigns nothing, so the frame rule makes
/// its only transition a self-loop.
fn self_loop() -> Model {
    ModelBuilder::new()
        .variable("x", 0, 1)
        .initial_state(&[("x", 0)])
        .action(ActionDecl::deterministic(
            "Stay",
            BoolExpr::Const(true),
            vec![],
        ))
        .build()
        .expect("the self-loop is a valid model")
}

/// Two initial states, each a self-loop, so depth 0 has two members.
fn two_starts() -> Model {
    ModelBuilder::new()
        .variable("x", 0, 1)
        .initial_state(&[("x", 0)])
        .initial_state(&[("x", 1)])
        .action(ActionDecl::deterministic(
            "Stay",
            BoolExpr::Const(true),
            vec![],
        ))
        .build()
        .expect("the two-start model is a valid model")
}

/// One action with two outcomes: firing it at `x == 0` reaches both `1` and `2`, which
/// is enumerated nondeterminism rather than two actions.
fn fork() -> Model {
    ModelBuilder::new()
        .variable("x", 0, 2)
        .initial_state(&[("x", 0)])
        .action(ActionDecl::enumerated(
            "Pick",
            BoolExpr::compare(CmpOp::Eq, IntExpr::var("x"), IntExpr::constant(0)),
            vec![
                vec![("x", IntExpr::constant(1))],
                vec![("x", IntExpr::constant(2))],
            ],
        ))
        .build()
        .expect("the fork is a valid model")
}

/// A model whose *guard* — not its update — cannot be evaluated at a reachable state.
///
/// `Overflows` is enabled at `x == 0` and overflows at `x == 1`, which
/// [`EvaluationError`] reports as a bare `Expression(Overflow)` naming no action. It
/// is the case that proves the exploration error's own action attribution is doing
/// work the model layer's error cannot do.
fn overflowing_guard() -> Model {
    ModelBuilder::new()
        .variable("x", 0, 1)
        .initial_state(&[("x", 0)])
        .action(ActionDecl::deterministic(
            "Advance",
            BoolExpr::compare(CmpOp::Lt, IntExpr::var("x"), IntExpr::constant(1)),
            vec![("x", IntExpr::constant(1))],
        ))
        .action(ActionDecl::deterministic(
            "Overflows",
            BoolExpr::compare(
                CmpOp::Gt,
                IntExpr::plus(IntExpr::var("x"), IntExpr::constant(i64::MAX)),
                IntExpr::constant(0),
            ),
            vec![],
        ))
        .build()
        .expect("the overflowing guard is a valid model")
}

fn vectors(states: &[State]) -> Vec<Vec<i64>> {
    states.iter().map(|s| s.as_slice().to_vec()).collect()
}

fn explore(model: &Model, bounds: Bounds) -> Exploration {
    bfs::explore(model, bounds).expect("this model evaluates everywhere")
}

// ---------------------------------------------------------------------------
// the complete case
// ---------------------------------------------------------------------------

#[test]
fn the_counter_chain_is_explored_layer_by_layer() {
    let model = counter(4);
    let exploration = explore(&model, Bounds::CERTIFIABLE);
    assert!(exploration.is_complete());
    let reachable = exploration.closed().expect("the chain closes");
    assert_eq!(vectors(reachable.states()), vec![[0], [1], [2], [3], [4]]);
    assert_eq!(reachable.depths(), [0, 1, 2, 3, 4]);
    assert_eq!(reachable.max_depth(), Some(4));
    assert_eq!(reachable.expanded(), 5);
    assert_eq!(
        reachable.transitions(),
        4,
        "the last state has no successor"
    );
    assert!(!reachable.is_empty());
    for depth in 0..=4 {
        assert_eq!(reachable.layer(depth).len(), 1, "a chain is one per layer");
    }
}

/// A self-loop is a transition. The set is closed at one state, and the exploration
/// terminates because the state is already visited, not because the edge is dropped.
#[test]
fn a_self_loop_is_one_state_and_one_transition() {
    let model = self_loop();
    let exploration = explore(&model, Bounds::CERTIFIABLE);
    let reachable = exploration.closed().expect("a self-loop closes");
    assert_eq!(vectors(reachable.states()), vec![[0]]);
    assert_eq!(reachable.depths(), [0]);
    assert_eq!(reachable.transitions(), 1);
    assert_eq!(reachable.expanded(), 1);
}

/// Every initial state is at depth 0, and they are enumerated in canonical order.
#[test]
fn every_initial_state_is_at_depth_zero() {
    let model = two_starts();
    let exploration = explore(&model, Bounds::CERTIFIABLE);
    let reachable = exploration.closed().expect("both self-loops close");
    assert_eq!(vectors(reachable.states()), vec![[0], [1]]);
    assert_eq!(reachable.depths(), [0, 0]);
    assert_eq!(reachable.layer(0).len(), 2);
    assert_eq!(reachable.transitions(), 2);
}

/// One action, two outcomes, two successors — and both land in the same layer.
#[test]
fn an_enumerated_action_puts_both_outcomes_in_one_layer() {
    let model = fork();
    let exploration = explore(&model, Bounds::CERTIFIABLE);
    let reachable = exploration.closed().expect("the fork closes");
    assert_eq!(vectors(reachable.states()), vec![[0], [1], [2]]);
    assert_eq!(reachable.depths(), [0, 1, 1]);
    assert_eq!(
        vectors(&reachable.layer(1).into_iter().cloned().collect::<Vec<_>>()),
        vec![[1], [2]]
    );
    assert_eq!(reachable.transitions(), 2);
    assert_eq!(reachable.expanded(), 3, "the two leaves are expanded too");
}

/// A state whose successor row is empty appears in the reachable set like any other,
/// expanded, contributing no transitions.
///
/// Surfaced, not judged: whether an empty row is a deadlock *defect* or a legitimate
/// terminal state is checking policy and belongs to the invariant/deadlock bone
/// (IMPL-03). This module reports the row and says nothing about it.
#[test]
fn a_state_with_no_successors_is_explored_and_not_judged() {
    let model = counter(4);
    let exploration = explore(&model, Bounds::CERTIFIABLE);
    let reachable = exploration.closed().expect("the chain closes");
    let last = model.state(&[4]).expect("4 is a counter state");
    assert!(reachable.contains(&last));
    assert_eq!(reachable.depth_of(&last), Some(4));
    assert!(
        model.successors(&last).expect("evaluates").is_empty(),
        "the guard disables the only action"
    );
    // It was expanded — the row was computed and found empty — so `expanded` counts it.
    assert_eq!(reachable.expanded(), reachable.len());
}

#[test]
fn a_state_outside_the_reachable_set_has_no_depth() {
    let model = counter(4);
    let exploration = explore(&model, Bounds::CERTIFIABLE.with_states(3));
    let reachable = exploration.reachable();
    let beyond = model.state(&[4]).expect("4 is a counter state");
    assert_eq!(reachable.depth_of(&beyond), None);
    assert!(!reachable.contains(&beyond));
}

// ---------------------------------------------------------------------------
// the three bounds
// ---------------------------------------------------------------------------

/// Each bound stops the same chain at the same place, and each names itself.
///
/// The three partial results are identical apart from [`Partial::tripped`], which is
/// the point: the caller learns *which* budget to raise, not merely that one ran out
/// (INV-008).
#[test]
fn each_bound_stops_the_chain_and_names_itself() {
    let model = counter(4);
    for (bounds, expected) in [
        (Bounds::CERTIFIABLE.with_states(3), Bound::States),
        (Bounds::CERTIFIABLE.with_depth(2), Bound::Depth),
        (Bounds::CERTIFIABLE.with_transitions(2), Bound::Transitions),
    ] {
        let exploration = explore(&model, bounds);
        assert!(!exploration.is_complete(), "{bounds:?}");
        assert!(exploration.closed().is_none(), "{bounds:?}");
        let partial = exploration.exhausted().expect("a bound tripped");
        assert_eq!(partial.tripped(), expected, "{bounds:?}");
        assert_eq!(partial.bounds(), bounds);
        assert_eq!(vectors(partial.explored().states()), vec![[0], [1], [2]]);
        assert_eq!(partial.explored().depths(), [0, 1, 2]);
        assert_eq!(partial.explored().expanded(), 2);
        assert_eq!(partial.explored().transitions(), 2);
        assert_eq!(
            vectors(partial.frontier()),
            vec![[2]],
            "the expansion that tripped the bound went back on the queue"
        );
    }
}

/// No bound is ever exceeded, and none is silently clipped to either: the counts a
/// partial result reports are the counts actually spent.
#[test]
fn a_partial_result_never_exceeds_its_bounds() {
    let model = counter(20);
    for bounds in [
        Bounds::CERTIFIABLE.with_states(7),
        Bounds::CERTIFIABLE.with_depth(3),
        Bounds::CERTIFIABLE.with_transitions(11),
        Bounds::new(5, 9, 100),
        Bounds::new(100, 9, 100),
    ] {
        let exploration = explore(&model, bounds);
        let reachable = exploration.reachable();
        assert!(reachable.len() <= bounds.states(), "{bounds:?}");
        assert!(
            reachable.max_depth().unwrap_or(0) <= bounds.depth(),
            "{bounds:?}"
        );
        assert!(
            reachable.transitions() <= bounds.transitions(),
            "{bounds:?}"
        );
    }
}

/// A depth bound of `0` admits the initial states and refuses every successor of a new
/// state. It is a representable request, not a degenerate one.
#[test]
fn a_depth_bound_of_zero_explores_only_the_initial_layer() {
    let model = counter(4);
    let exploration = explore(&model, Bounds::CERTIFIABLE.with_depth(0));
    let partial = exploration.exhausted().expect("depth 0 trips at once");
    assert_eq!(partial.tripped(), Bound::Depth);
    assert_eq!(vectors(partial.explored().states()), vec![[0]]);
    assert_eq!(partial.explored().expanded(), 0);
    assert_eq!(partial.explored().transitions(), 0);
    assert_eq!(vectors(partial.frontier()), vec![[0]]);
}

/// But a depth bound of `0` on a model that discovers nothing new *completes*.
///
/// An expansion that adds no state cannot deepen the walk, so the depth check is not
/// consulted — reporting `Exhausted` here would name a bound that was never reached.
#[test]
fn a_depth_bound_of_zero_still_completes_a_self_loop() {
    let model = self_loop();
    let exploration = explore(&model, Bounds::CERTIFIABLE.with_depth(0));
    let reachable = exploration
        .closed()
        .expect("nothing new is ever discovered");
    assert_eq!(reachable.transitions(), 1);
    assert_eq!(reachable.max_depth(), Some(0));
}

/// A state bound equal to the reachable set exactly completes; one less does not.
#[test]
fn the_state_bound_is_exact() {
    let model = counter(4);
    assert!(explore(&model, Bounds::CERTIFIABLE.with_states(5)).is_complete());
    let short = explore(&model, Bounds::CERTIFIABLE.with_states(4));
    assert_eq!(
        short.exhausted().map(|partial| partial.tripped()),
        Some(Bound::States)
    );
}

/// A transition bound equal to the edge count exactly completes; one less does not.
#[test]
fn the_transition_bound_is_exact() {
    let model = counter(4);
    assert!(explore(&model, Bounds::CERTIFIABLE.with_transitions(4)).is_complete());
    let short = explore(&model, Bounds::CERTIFIABLE.with_transitions(3));
    assert_eq!(
        short.exhausted().map(|partial| partial.tripped()),
        Some(Bound::Transitions)
    );
}

/// When two bounds would trip on one expansion, the reported one is fixed by the
/// checking order — transitions, then depth, then states — so the answer is a function
/// of the model and the bounds alone.
#[test]
fn a_simultaneous_trip_reports_one_bound_deterministically() {
    let model = counter(4);
    // Expanding `n == 2` would be the third transition, would reach depth 3, and would
    // be the fourth state. All three bounds refuse it.
    let bounds = Bounds::new(3, 2, 2);
    let exploration = explore(&model, bounds);
    let partial = exploration.exhausted().expect("all three bounds trip");
    assert_eq!(partial.tripped(), Bound::Transitions);
    // And with the transition budget raised, the next one in order answers.
    let bounds = bounds.with_transitions(99);
    let exploration = explore(&model, bounds);
    let partial = exploration.exhausted().expect("two bounds still trip");
    assert_eq!(partial.tripped(), Bound::Depth);
    let bounds = bounds.with_depth(99);
    let exploration = explore(&model, bounds);
    let partial = exploration
        .exhausted()
        .expect("the state bound still trips");
    assert_eq!(partial.tripped(), Bound::States);
}

/// The frontier is exactly the discovered-but-unexpanded states: every one of them is
/// in the explored set, none of them has been expanded, and together with the expanded
/// states they account for the whole set.
#[test]
fn the_frontier_is_exactly_the_unexpanded_states() {
    let model = counter(20);
    for bounds in [
        Bounds::CERTIFIABLE.with_states(7),
        Bounds::CERTIFIABLE.with_depth(3),
        Bounds::CERTIFIABLE.with_transitions(11),
    ] {
        let exploration = explore(&model, bounds);
        let partial = exploration.exhausted().expect("a bound tripped");
        for waiting in partial.frontier() {
            assert!(partial.explored().contains(waiting), "{waiting} {bounds:?}");
        }
        let expanded = partial.explored().expanded();
        assert_eq!(
            partial.explored().len().saturating_sub(expanded),
            partial.frontier().len(),
            "{bounds:?}"
        );
        assert!(!partial.frontier().is_empty(), "{bounds:?}");
    }
}

// ---------------------------------------------------------------------------
// the two error arms
// ---------------------------------------------------------------------------

/// A model whose update leaves its declared domain mid-walk surfaces the typed error,
/// naming the state, its depth, and the action — never a panic, never a smaller answer
/// that looks like a complete one.
#[test]
fn an_update_that_leaves_the_domain_names_the_state_and_the_action() {
    let model = runaway(3);
    let outcome = bfs::explore(&model, Bounds::CERTIFIABLE);
    let Err(ExplorationError::Evaluation {
        state,
        depth,
        action,
        source,
    }) = outcome
    else {
        panic!("an unguarded increment must fail at the top of its domain: {outcome:?}");
    };
    assert_eq!(state.as_slice(), [3], "the walk reached 3 and then failed");
    assert_eq!(depth, 3, "three steps in");
    assert_eq!(action.as_ref().map(Ident::as_str), Some("Bump"));
    let EvaluationError::UpdateOutOfDomain {
        action: named,
        variable,
        value,
        domain,
    } = *source
    else {
        panic!("PO-MOD-003 failing is `UpdateOutOfDomain`");
    };
    assert_eq!(named.as_str(), "Bump");
    assert_eq!(variable.as_str(), "n");
    assert_eq!(value, 4);
    assert_eq!((domain.lo(), domain.hi()), (0, 3));
}

/// The same failure is *recoverable* by bounding the walk short of it, which is why
/// the error does not need to carry a partial reachable set: the well-defined part of
/// the model is re-derivable exactly, as an ordinary partial result.
#[test]
fn bounding_short_of_the_defect_returns_a_partial_result_instead() {
    let model = runaway(3);
    let exploration = explore(&model, Bounds::CERTIFIABLE.with_depth(2));
    let partial = exploration
        .exhausted()
        .expect("the depth bound tripped first");
    assert_eq!(partial.tripped(), Bound::Depth);
    assert_eq!(vectors(partial.explored().states()), vec![[0], [1], [2]]);
    assert_eq!(partial.explored().depths(), [0, 1, 2]);
    assert_eq!(vectors(partial.frontier()), vec![[2]]);
    // One layer further and the defect is reached again, so the bound is what avoided
    // it and not luck.
    assert!(bfs::explore(&model, Bounds::CERTIFIABLE.with_depth(3)).is_err());
}

/// A guard that cannot be evaluated is attributed to its action even though
/// [`EvaluationError::Expression`] names no action itself.
#[test]
fn a_failing_guard_is_attributed_to_its_action() {
    let model = overflowing_guard();
    let outcome = bfs::explore(&model, Bounds::CERTIFIABLE);
    let Err(ExplorationError::Evaluation {
        state,
        depth,
        action,
        source,
    }) = outcome
    else {
        panic!("the guard overflows at x == 1: {outcome:?}");
    };
    assert_eq!(state.as_slice(), [1]);
    assert_eq!(depth, 1);
    assert_eq!(
        action.as_ref().map(Ident::as_str),
        Some("Overflows"),
        "the model layer's error names no action; the exploration's does"
    );
    assert_eq!(
        *source,
        EvaluationError::Expression(EvalError::Overflow {
            operator: ArithOp::Add,
            left: 1,
            right: i64::MAX,
        })
    );
}

/// A state bound that cannot hold the model's own initial states is not a partial
/// result — there is no prefix of the walk to report.
#[test]
fn a_state_bound_below_the_initial_states_is_an_error() {
    let model = two_starts();
    let outcome = bfs::explore(&model, Bounds::CERTIFIABLE.with_states(1));
    assert_eq!(
        outcome.err(),
        Some(ExplorationError::InitialStatesExceedBound {
            initial: 2,
            limit: 1,
        })
    );
    // Exactly enough is enough, and it trips on the first expansion instead.
    let exploration = explore(&model, Bounds::CERTIFIABLE.with_states(2));
    assert!(exploration.is_complete(), "both starts only self-loop");
}

/// Both error arms print what they are about, and the evaluation arm chains to the
/// model layer's error rather than swallowing it.
#[test]
fn the_errors_are_reportable() {
    use core::error::Error;

    let evaluation =
        bfs::explore(&runaway(3), Bounds::CERTIFIABLE).expect_err("the runaway counter fails");
    let rendered = evaluation.to_string();
    assert!(rendered.contains("Bump"), "{rendered}");
    assert!(rendered.contains("(3)"), "{rendered}");
    assert!(rendered.contains("depth 3"), "{rendered}");
    assert!(evaluation.source().is_some(), "the model error is chained");

    let refused = bfs::explore(&two_starts(), Bounds::CERTIFIABLE.with_states(1))
        .expect_err("one slot cannot hold two starts");
    let rendered = refused.to_string();
    assert!(
        rendered.contains('2') && rendered.contains('1'),
        "{rendered}"
    );
    assert!(refused.source().is_none());
}

// ---------------------------------------------------------------------------
// determinism, bounds, and the vocabulary
// ---------------------------------------------------------------------------

/// Repeat runs of complete, bounded, and failing explorations all agree exactly.
#[test]
fn repeat_runs_agree_on_every_outcome_shape() {
    for bounds in [
        Bounds::CERTIFIABLE,
        Bounds::CERTIFIABLE.with_states(7),
        Bounds::CERTIFIABLE.with_depth(3),
        Bounds::CERTIFIABLE.with_transitions(11),
    ] {
        let run = || bfs::explore(&counter(20), bounds);
        let first = run();
        let second = run();
        assert_eq!(first, second, "{bounds:?}");
        assert_eq!(
            format!("{first:?}").as_bytes(),
            format!("{second:?}").as_bytes(),
            "{bounds:?}"
        );
    }
    let failing = || bfs::explore(&runaway(6), Bounds::CERTIFIABLE);
    assert_eq!(failing(), failing());
}

/// The bounds a caller declares are the bounds the result reports back.
#[test]
fn bounds_report_what_they_were_given() {
    let bounds = Bounds::new(11, 22, 33);
    assert_eq!(bounds.states(), 11);
    assert_eq!(bounds.depth(), 22);
    assert_eq!(bounds.transitions(), 33);
    assert_eq!(bounds.with_states(1).states(), 1);
    assert_eq!(bounds.with_depth(2).depth(), 2);
    assert_eq!(bounds.with_transitions(3).transitions(), 3);
    assert_eq!(
        Bounds::CERTIFIABLE,
        Bounds::new(bfs::MAX_STATES, bfs::MAX_DEPTH, bfs::MAX_TRANSITIONS)
    );
}

/// The default bounds are the certificate wire form's, cited to it.
#[test]
fn the_default_bounds_are_the_wire_forms() {
    // `crates/continuum-kernel-core/src/wire.rs:131, 139`.
    assert_eq!(bfs::MAX_STATES, 1 << 20);
    assert_eq!(bfs::MAX_TRANSITIONS, 1 << 22);
    // No certificate carries a depth, so this one is the engine's own, set to the
    // ceiling the state bound already implies.
    assert_eq!(bfs::MAX_DEPTH, bfs::MAX_STATES);
}

#[test]
fn each_bound_prints_its_own_name() {
    assert_eq!(Bound::States.to_string(), "state");
    assert_eq!(Bound::Depth.to_string(), "depth");
    assert_eq!(Bound::Transitions.to_string(), "transition");
}
