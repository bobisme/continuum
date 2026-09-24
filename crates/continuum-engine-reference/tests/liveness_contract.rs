//! The liveness check under the model's own fairness (bn-1ln12; RFC 0008 "Fairness
//! scopes" and "Finite graph", RFC 0015 "Core behavior model").
//!
//! What is pinned, each on a model small enough to read:
//!
//! - no fairness is no fairness: a stuttering `Wait` loop refutes `◇ done`, and weak
//!   fairness of `Complete` excludes it (the Revision 2 fair-lasso spike's pair);
//! - weak and strong differ: an action enabled only every other step is not forced by
//!   weak fairness and is by strong;
//! - a scope is one assumption over a set, not one per member: weak fairness over
//!   `{RecoverA, RecoverB}` lets `b` stay down, and per-member weak fairness does not;
//! - strong fairness shrinks a component and finds the fair cycle inside it;
//! - stuttering is declared, never assumed: under `Stuttering::Everywhere` (CML's
//!   `stutter(state)`) a model without fairness guarantees no progress, and only its
//!   declared fairness excludes stuttering; the terminal policies decide a terminal
//!   state; a bounded exploration is inconclusive, never a pass;
//! - every lasso is checked independently here: its links are successors, its loop
//!   closes, it avoids the goal, and each assumption is satisfied on its loop, judged
//!   from `Model::is_fairness_enabled` and the step labels, not from the engine.

use continuum_engine_reference::bfs::{self, Bounds, Exploration};
use continuum_engine_reference::checking::Unresolved;
use continuum_engine_reference::liveness::{
    Cycle, Goal, Lasso, LivenessError, LivenessOutcome, Stuttering, check_liveness,
};
use continuum_engine_reference::{
    ActionDecl, BoolExpr, CmpOp, IntExpr, Model, ModelBuilder, State, Strength,
};

fn var(name: &str) -> IntExpr {
    IntExpr::var(name)
}

fn eq(name: &str, value: i64) -> BoolExpr {
    BoolExpr::compare(CmpOp::Eq, var(name), IntExpr::constant(value))
}

fn set(name: &str, value: i64) -> (&str, IntExpr) {
    (name, IntExpr::constant(value))
}

fn explore(model: &Model) -> Exploration {
    bfs::explore(model, Bounds::CERTIFIABLE).expect("evaluates")
}

fn check(model: &Model, goal: Goal, stuttering: Stuttering) -> LivenessOutcome {
    check_liveness(model, &explore(model), goal, stuttering).expect("runs")
}

fn predicate(model: &Model, name: &str) -> usize {
    model.predicate_index(name).expect("declared")
}

/// Check a lasso without the engine: every link is a successor, the loop closes, the
/// goal fails where it must, and each assumption holds on the loop.
fn validate(model: &Model, lasso: &Lasso, goal: Goal) {
    let (index, eventually) = match goal {
        Goal::Eventually(i) => (i, true),
        Goal::Recurrence(i) => (i, false),
    };
    let holds = |s: &State| model.evaluate_predicate(index, s).expect("evaluates");
    assert!(model.initial_states().contains(lasso.start()));
    let mut here = lasso.start().clone();
    if eventually {
        assert!(!holds(&here), "the lasso starts where the goal fails");
    }
    for step in lasso.stem() {
        let next = model.successors(&here).expect("evaluates");
        assert!(
            next.iter()
                .any(|s| s.action() == step.action() && s.target() == step.target()),
            "stem link {} is a successor",
            step.name()
        );
        here = step.target().clone();
        if eventually {
            assert!(!holds(&here), "the stem avoids the goal");
        }
    }
    assert_eq!(&here, lasso.loop_state());
    match lasso.cycle() {
        Cycle::Stutter => {
            assert!(!holds(&here), "the stutter state fails the goal");
            for f in 0..model.fairness().len() {
                assert!(
                    !model.is_fairness_enabled(f, &here).expect("evaluates"),
                    "a fair stutter has every scope disabled"
                );
            }
        }
        Cycle::Steps(steps) => {
            assert!(!steps.is_empty());
            let mut visited: Vec<State> = vec![here.clone()];
            let mut taken: Vec<usize> = Vec::new();
            for step in steps {
                let next = model.successors(&here).expect("evaluates");
                assert!(
                    next.iter()
                        .any(|s| s.action() == step.action() && s.target() == step.target()),
                    "loop link {} is a successor",
                    step.name()
                );
                taken.push(step.action());
                here = step.target().clone();
                visited.push(here.clone());
            }
            assert_eq!(&here, lasso.loop_state(), "the loop closes");
            for s in &visited {
                assert!(!holds(s), "the loop avoids the goal");
            }
            for (f, assumption) in model.fairness().iter().enumerate() {
                let is_taken = taken.iter().any(|a| assumption.actions().contains(a));
                let enabled: Vec<bool> = visited
                    .iter()
                    .map(|s| model.is_fairness_enabled(f, s).expect("evaluates"))
                    .collect();
                let ok = match assumption.strength() {
                    Strength::Weak => is_taken || enabled.iter().any(|e| !e),
                    Strength::Strong => is_taken || enabled.iter().all(|e| !e),
                };
                assert!(ok, "assumption #{f} holds on the loop");
            }
        }
    }
}

fn violated(outcome: LivenessOutcome) -> Lasso {
    match outcome {
        LivenessOutcome::Violated(lasso) => *lasso,
        other => panic!("expected a counterexample, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// models
// ---------------------------------------------------------------------------

/// The fair-lasso spike's `FairProgress`: `Wait` loops, `Complete` finishes.
fn progress() -> ModelBuilder {
    ModelBuilder::new()
        .variable("done", 0, 1)
        .initial_state(&[("done", 0)])
        .action(ActionDecl::deterministic(
            "Wait",
            eq("done", 0),
            vec![set("done", 0)],
        ))
        .action(ActionDecl::deterministic(
            "Complete",
            eq("done", 0),
            vec![set("done", 1)],
        ))
        .predicate("Done", eq("done", 1))
}

/// `Flip` toggles `x` forever; `Go` finishes, enabled only at `x == 1`.
fn blinking() -> ModelBuilder {
    ModelBuilder::new()
        .variable("x", 0, 1)
        .variable("done", 0, 1)
        .initial_state(&[("x", 0), ("done", 0)])
        .action(ActionDecl::deterministic(
            "Flip",
            BoolExpr::Const(true),
            vec![(
                "x",
                IntExpr::Arith(
                    continuum_engine_reference::ArithOp::Sub,
                    Box::new(IntExpr::constant(1)),
                    Box::new(var("x")),
                ),
            )],
        ))
        .action(ActionDecl::deterministic(
            "Go",
            BoolExpr::and(eq("x", 1), eq("done", 0)),
            vec![set("done", 1)],
        ))
        .predicate("Done", eq("done", 1))
}

/// Two nodes that crash and recover; `B` is `b == 1`.
fn nodes() -> ModelBuilder {
    let mut builder = ModelBuilder::new()
        .variable("a", 0, 1)
        .variable("b", 0, 1)
        .initial_state(&[("a", 1), ("b", 1)])
        .predicate("B", eq("b", 1));
    for (node, name) in [("a", "A"), ("b", "B")] {
        builder = builder
            .action(ActionDecl::deterministic(
                &format!("Crash{name}"),
                eq(node, 1),
                vec![set(node, 0)],
            ))
            .action(ActionDecl::deterministic(
                &format!("Recover{name}"),
                eq(node, 0),
                vec![set(node, 1)],
            ));
    }
    builder
}

/// `s` moves `0 <-> 1 <-> 2`; `Exit` leaves from `2` to `3`, where `P` holds.
fn diamond() -> ModelBuilder {
    let step = |name: &str, from: i64, to: i64| {
        ActionDecl::deterministic(name, eq("s", from), vec![set("s", to)])
    };
    ModelBuilder::new()
        .variable("s", 0, 3)
        .initial_state(&[("s", 0)])
        .action(step("Up01", 0, 1))
        .action(step("Down10", 1, 0))
        .action(step("Up12", 1, 2))
        .action(step("Down21", 2, 1))
        .action(step("Exit", 2, 3))
        .action(step("Stay", 3, 3))
        .predicate("P", eq("s", 3))
}

// ---------------------------------------------------------------------------
// the tests
// ---------------------------------------------------------------------------

#[test]
fn without_fairness_a_wait_loop_refutes_progress_and_weak_fairness_excludes_it() {
    let unfair = progress().build().expect("valid");
    let goal = Goal::Eventually(predicate(&unfair, "Done"));
    let lasso = violated(check(&unfair, goal, Stuttering::AtTerminal));
    validate(&unfair, &lasso, goal);
    match lasso.cycle() {
        Cycle::Steps(steps) => assert!(steps.iter().all(|s| s.name().as_str() == "Wait")),
        Cycle::Stutter => panic!("the Wait loop is a cycle"),
    }

    let fair = progress()
        .fairness(Strength::Weak, ["Complete"])
        .build()
        .expect("valid");
    let goal = Goal::Eventually(predicate(&fair, "Done"));
    assert_eq!(
        check(&fair, goal, Stuttering::AtTerminal),
        LivenessOutcome::Holds { states: 2 }
    );
    // Fairness of the wrong action does not help.
    let wrong = progress()
        .fairness(Strength::Strong, ["Wait"])
        .build()
        .expect("valid");
    validate(
        &wrong,
        &violated(check(&wrong, goal, Stuttering::AtTerminal)),
        goal,
    );
}

#[test]
fn weak_fairness_does_not_force_an_intermittent_action_and_strong_does() {
    for (strength, holds) in [(Strength::Weak, false), (Strength::Strong, true)] {
        let model = blinking()
            .fairness(strength, ["Go"])
            .build()
            .expect("valid");
        let goal = Goal::Eventually(predicate(&model, "Done"));
        let outcome = check(&model, goal, Stuttering::AtTerminal);
        if holds {
            assert!(
                matches!(outcome, LivenessOutcome::Holds { .. }),
                "{strength}: {outcome:?}"
            );
        } else {
            validate(&model, &violated(outcome), goal);
        }
    }
}

/// One assumption over a set is weaker than one per member: the difference RFC 0008's
/// "attached to action schemas" makes for a schema expanded per parameter.
#[test]
fn a_scope_is_one_assumption_over_its_set() {
    let union = nodes()
        .fairness(Strength::Weak, ["RecoverA", "RecoverB"])
        .build()
        .expect("valid");
    let goal = Goal::Recurrence(predicate(&union, "B"));
    let lasso = violated(check(&union, goal, Stuttering::AtTerminal));
    validate(&union, &lasso, goal);

    let each = nodes()
        .fairness(Strength::Weak, ["RecoverA"])
        .fairness(Strength::Weak, ["RecoverB"])
        .build()
        .expect("valid");
    assert!(matches!(
        check(&each, goal, Stuttering::AtTerminal),
        LivenessOutcome::Holds { states: 4 }
    ));
    assert_ne!(union.identity(), each.identity());
}

#[test]
fn strong_fairness_shrinks_a_component_to_the_fair_cycle_inside_it() {
    let model = diamond()
        .fairness(Strength::Strong, ["Exit"])
        .build()
        .expect("valid");
    let goal = Goal::Eventually(predicate(&model, "P"));
    let lasso = violated(check(&model, goal, Stuttering::AtTerminal));
    validate(&model, &lasso, goal);
    // The loop avoids `s == 2`, where `Exit` is enabled.
    let Cycle::Steps(steps) = lasso.cycle() else {
        panic!("a cycle")
    };
    assert!(steps.iter().all(|s| s.target().as_slice() != [2]));
    // Strong fairness of `Up12` as well forces the exit: the `0 <-> 1` loop enables
    // `Up12` infinitely often, and every loop through `2` enables `Exit` there.
    let forced = diamond()
        .fairness(Strength::Strong, ["Exit"])
        .fairness(Strength::Strong, ["Up12"])
        .build()
        .expect("valid");
    assert!(matches!(
        check(&forced, goal, Stuttering::AtTerminal),
        LivenessOutcome::Holds { .. }
    ));
}

#[test]
fn the_completion_policy_decides_a_terminal_state() {
    // `Halt` ends at `x == 1`, where the goal `x == 2` fails and nothing is enabled.
    let model = ModelBuilder::new()
        .variable("x", 0, 2)
        .initial_state(&[("x", 0)])
        .action(ActionDecl::deterministic(
            "Halt",
            eq("x", 0),
            vec![set("x", 1)],
        ))
        .predicate("Two", eq("x", 2))
        .fairness(Strength::Weak, ["Halt"])
        .build()
        .expect("valid");
    let goal = Goal::Eventually(predicate(&model, "Two"));
    let lasso = violated(check(&model, goal, Stuttering::AtTerminal));
    assert_eq!(lasso.cycle(), &Cycle::Stutter);
    assert_eq!(lasso.loop_state().as_slice(), [1]);
    validate(&model, &lasso, goal);
    assert_eq!(
        check(&model, goal, Stuttering::Never),
        LivenessOutcome::Holds { states: 2 }
    );
}

#[test]
fn a_bounded_exploration_is_inconclusive_and_a_bad_goal_is_refused() {
    let model = progress().build().expect("valid");
    let goal = Goal::Eventually(predicate(&model, "Done"));
    let partial = bfs::explore(&model, Bounds::CERTIFIABLE.with_depth(0)).expect("evaluates");
    assert!(!partial.is_complete());
    assert!(matches!(
        check_liveness(&model, &partial, goal, Stuttering::AtTerminal),
        Ok(LivenessOutcome::Inconclusive(
            Unresolved::ResourceExhausted { .. }
        ))
    ));
    assert_eq!(
        check_liveness(
            &model,
            &explore(&model),
            Goal::Recurrence(9),
            Stuttering::Never
        ),
        Err(LivenessError::UnknownPredicate {
            index: 9,
            declared: 1
        })
    );
}

/// A long cycle is found without recursion: a ring of 20,000 states.
#[test]
fn a_long_ring_is_searched_in_a_small_stack() {
    let n = 20_000;
    let model = ModelBuilder::new()
        .variable("i", 0, n - 1)
        .initial_state(&[("i", 0)])
        .action(ActionDecl::deterministic(
            "Next",
            BoolExpr::compare(CmpOp::Lt, var("i"), IntExpr::constant(n - 1)),
            vec![("i", IntExpr::plus(var("i"), IntExpr::constant(1)))],
        ))
        .action(ActionDecl::deterministic(
            "Wrap",
            eq("i", n - 1),
            vec![set("i", 0)],
        ))
        .predicate("Never", BoolExpr::Const(false))
        .fairness(Strength::Weak, ["Next", "Wrap"])
        .build()
        .expect("valid");
    let goal = Goal::Recurrence(predicate(&model, "Never"));
    let handle = std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(move || {
            let lasso = violated(check(&model, goal, Stuttering::AtTerminal));
            validate(&model, &lasso, goal);
        })
        .expect("spawns");
    handle.join().expect("no stack overflow");
}

/// The semantic oracle states fairness per action and ignores a model's own; a model
/// that declares any is refused rather than silently read without it.
#[test]
fn the_semantic_oracle_refuses_a_model_that_declares_fairness() {
    use continuum_engine_reference::semantic::{System, SystemError, SystemParts};
    let parts = |model: Model| SystemParts {
        model,
        kinds: std::collections::BTreeMap::new(),
        meta: std::collections::BTreeMap::new(),
        conflicts: std::collections::BTreeSet::new(),
        obligations: Vec::new(),
        phases: Vec::new(),
        lineage: Vec::new(),
    };
    let fair = progress()
        .fairness(Strength::Weak, ["Complete"])
        .build()
        .expect("valid");
    assert_eq!(
        System::new(parts(fair)).map(|_| ()),
        Err(SystemError::ModelFairness)
    );
    // Without it, the same parts fail later, on the missing kinds.
    let plain = progress().build().expect("valid");
    assert!(matches!(
        System::new(parts(plain)),
        Err(SystemError::KindMismatch { .. })
    ));
}

/// With stuttering everywhere (CML's standard behaviour) there is no hidden progress
/// assumption: `Inc` may never be taken, so `◇ x == 1` fails at the initial state by
/// stuttering, and weak fairness of `Inc` restores it. The maximal-execution reading
/// (`AtTerminal`) makes `◇` hold without fairness, which is why it is only a named
/// choice, never a default.
#[test]
fn stuttering_everywhere_makes_no_progress_assumption() {
    let inc = || {
        ModelBuilder::new()
            .variable("x", 0, 1)
            .initial_state(&[("x", 0)])
            .action(ActionDecl::deterministic(
                "Inc",
                eq("x", 0),
                vec![set("x", 1)],
            ))
            .predicate("Done", eq("x", 1))
    };
    let unfair = inc().build().expect("valid");
    let goal = Goal::Eventually(predicate(&unfair, "Done"));
    let lasso = violated(check(&unfair, goal, Stuttering::Everywhere));
    assert_eq!(lasso.cycle(), &Cycle::Stutter);
    assert!(lasso.stem().is_empty(), "it stutters at the initial state");
    validate(&unfair, &lasso, goal);
    assert!(matches!(
        check(&unfair, goal, Stuttering::AtTerminal),
        LivenessOutcome::Holds { .. }
    ));
    let fair = inc()
        .fairness(Strength::Weak, ["Inc"])
        .build()
        .expect("valid");
    assert_eq!(
        check(&fair, goal, Stuttering::Everywhere),
        LivenessOutcome::Holds { states: 2 }
    );
    // The spike's pair under stuttering: weak fairness of `Complete` excludes both the
    // `Wait` loop and the stutter; without it, the stutter alone refutes progress.
    let progress_fair = progress()
        .fairness(Strength::Weak, ["Complete"])
        .build()
        .expect("valid");
    let goal = Goal::Eventually(predicate(&progress_fair, "Done"));
    assert!(matches!(
        check(&progress_fair, goal, Stuttering::Everywhere),
        LivenessOutcome::Holds { .. }
    ));
}
