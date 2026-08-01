//! The programmatic transition model's contract (PR 8, IMPL-01).
//!
//! Every assertion here is about the *model layer* — declaration, canonical order,
//! and the evaluation primitives. Nothing in this file explores; the Die Hard facts
//! live in `diehard_evidence.rs` and the declaration-time rejections in
//! `adversarial_model.rs`.

use continuum_engine_reference::expr::{ArithOp, BoolExpr, CmpOp, IntExpr, MAX_EXPR_DEPTH};
use continuum_engine_reference::model::{ActionDecl, EvaluationError, Model, ModelBuilder, State};
use continuum_engine_reference::{EvalError, Step};

// ---------------------------------------------------------------------------
// fixtures
// ---------------------------------------------------------------------------

/// A two-variable model with a genuinely guarded action, an enumerated action, and an
/// action that can deadlock the whole model.
///
/// - `n in 0..=3`, `flag in 0..=1`
/// - `Step` is enabled while `n < 3` and increments `n`
/// - `Pick` chooses `flag' = 0` or `flag' = 1`, and is enabled only while `flag == 0`
/// - `Wrap` is enabled only at `n == 3` and resets `n` to `0`
fn guarded() -> Model {
    ModelBuilder::new()
        .variable("n", 0, 3)
        .variable("flag", 0, 1)
        .initial_state(&[("n", 0), ("flag", 0)])
        .action(ActionDecl::deterministic(
            "Step",
            BoolExpr::compare(CmpOp::Lt, IntExpr::var("n"), IntExpr::constant(3)),
            vec![("n", IntExpr::plus(IntExpr::var("n"), IntExpr::constant(1)))],
        ))
        .action(ActionDecl::enumerated(
            "Pick",
            BoolExpr::compare(CmpOp::Eq, IntExpr::var("flag"), IntExpr::constant(0)),
            vec![
                vec![("flag", IntExpr::constant(0))],
                vec![("flag", IntExpr::constant(1))],
            ],
        ))
        .action(ActionDecl::deterministic(
            "Wrap",
            BoolExpr::compare(CmpOp::Eq, IntExpr::var("n"), IntExpr::constant(3)),
            vec![("n", IntExpr::constant(0))],
        ))
        .predicate("InBounds", BoolExpr::in_range(IntExpr::var("n"), 0, 3))
        .predicate(
            "AtTop",
            BoolExpr::compare(CmpOp::Eq, IntExpr::var("n"), IntExpr::constant(3)),
        )
        .build()
        .expect("the guarded fixture is a valid model")
}

fn state(model: &Model, vector: &[i64]) -> State {
    model.state(vector).expect("fixture state is well typed")
}

fn targets(model: &Model, from: &State, action: &str) -> Vec<Vec<i64>> {
    let index = model
        .action_index(action)
        .unwrap_or_else(|| panic!("action `{action}` is declared"));
    model
        .action_successors(index, from)
        .expect("evaluation succeeds")
        .iter()
        .map(|target| target.as_slice().to_vec())
        .collect()
}

// ---------------------------------------------------------------------------
// canonical order
// ---------------------------------------------------------------------------

/// Variables are ordered by name in byte order, which is the order
/// `wire::StateDomain` requires (`crates/continuum-kernel-core/src/wire.rs:496-497`),
/// not the order they were declared in.
#[test]
fn variables_are_ordered_by_name_not_by_declaration() {
    let model = guarded();
    let names: Vec<&str> = model
        .variables()
        .iter()
        .map(|variable| variable.name().as_str())
        .collect();
    // Declared "n" first, then "flag"; canonical order is "flag" < "n".
    assert_eq!(names, vec!["flag", "n"]);
    assert_eq!(model.variable_index("flag"), Some(0));
    assert_eq!(model.variable_index("n"), Some(1));
    assert_eq!(model.variable_index("absent"), None);
}

/// The strongest form of the previous test: declaration order is not part of a
/// model's identity. Two builders that declare the same things in different orders
/// produce equal models, so a certificate emitted from either is the same bytes.
#[test]
fn declaration_order_is_not_part_of_the_model() {
    let forward = ModelBuilder::new()
        .variable("a", 0, 1)
        .variable("b", 0, 1)
        .initial_state(&[("a", 0), ("b", 0)])
        .action(ActionDecl::deterministic(
            "Left",
            BoolExpr::Const(true),
            vec![("a", IntExpr::constant(1))],
        ))
        .action(ActionDecl::deterministic(
            "Right",
            BoolExpr::Const(true),
            vec![("b", IntExpr::constant(1))],
        ))
        .predicate("P", BoolExpr::Const(true))
        .predicate("Q", BoolExpr::Const(false))
        .build()
        .expect("valid");
    let reversed = ModelBuilder::new()
        .variable("b", 0, 1)
        .variable("a", 0, 1)
        .initial_state(&[("b", 0), ("a", 0)])
        .action(ActionDecl::deterministic(
            "Right",
            BoolExpr::Const(true),
            vec![("b", IntExpr::constant(1))],
        ))
        .action(ActionDecl::deterministic(
            "Left",
            BoolExpr::Const(true),
            vec![("a", IntExpr::constant(1))],
        ))
        .predicate("Q", BoolExpr::Const(false))
        .predicate("P", BoolExpr::Const(true))
        .build()
        .expect("valid");
    assert_eq!(forward, reversed);
}

/// Action indices are positions in ascending-name order, which is the certificate's
/// action table (`crates/continuum-kernel-core/src/wire.rs:750-763`).
#[test]
fn actions_are_indexed_by_ascending_name() {
    let model = guarded();
    let names: Vec<&str> = model
        .actions()
        .iter()
        .map(|action| action.name().as_str())
        .collect();
    assert_eq!(names, vec!["Pick", "Step", "Wrap"]);
    assert_eq!(model.action_index("Pick"), Some(0));
    assert_eq!(model.action_index("Wrap"), Some(2));
    assert_eq!(model.action_index("Nope"), None);
    for pair in names.windows(2) {
        assert!(pair[0].as_bytes() < pair[1].as_bytes(), "{pair:?}");
    }
}

/// Predicates are ordered too, so `predicates()` is a stable list a checking policy
/// can index into.
#[test]
fn predicates_are_ordered_by_name() {
    let model = guarded();
    let names: Vec<&str> = model
        .predicates()
        .iter()
        .map(|predicate| predicate.name().as_str())
        .collect();
    assert_eq!(names, vec!["AtTop", "InBounds"]);
    assert_eq!(model.predicate_index("InBounds"), Some(1));
    assert_eq!(model.predicate_index("Missing"), None);
}

/// Initial states are strictly ascending state vectors
/// (`crates/continuum-kernel-core/src/wire.rs:705`).
#[test]
fn initial_states_are_strictly_ascending() {
    let model = ModelBuilder::new()
        .variable("x", 0, 2)
        .initial_state(&[("x", 2)])
        .initial_state(&[("x", 0)])
        .initial_state(&[("x", 1)])
        .action(ActionDecl::deterministic(
            "Nop",
            BoolExpr::Const(true),
            vec![],
        ))
        .build()
        .expect("valid");
    let vectors: Vec<&[i64]> = model.initial_states().iter().map(State::as_slice).collect();
    assert_eq!(
        vectors,
        vec![[0].as_slice(), [1].as_slice(), [2].as_slice()]
    );
}

// ---------------------------------------------------------------------------
// states as canonical vectors
// ---------------------------------------------------------------------------

#[test]
fn a_state_is_a_vector_in_canonical_variable_order() {
    let model = guarded();
    // Positions are (flag, n) — canonical order, not declaration order.
    let s = state(&model, &[1, 2]);
    assert_eq!(s.as_slice(), &[1, 2]);
    assert_eq!(s.arity(), 2);
    assert_eq!(model.binding(&s, "flag"), Some(1));
    assert_eq!(model.binding(&s, "n"), Some(2));
    assert_eq!(model.binding(&s, "nope"), None);
}

#[test]
fn a_vector_of_the_wrong_length_is_a_typed_error() {
    let model = guarded();
    assert_eq!(
        model.state(&[0]),
        Err(EvaluationError::StateArity {
            expected: 2,
            found: 1
        })
    );
    assert!(!model.admits(&[0, 0, 0]));
}

/// The declared domain is inclusive at both ends, and a value one step outside it is
/// a typed error rather than a clamp.
#[test]
fn domain_edges_are_inclusive_and_one_step_outside_is_an_error() {
    let model = guarded();
    assert!(model.admits(&[0, 0]));
    assert!(model.admits(&[1, 3]));
    let Err(EvaluationError::StateOutOfDomain {
        variable,
        value,
        domain,
    }) = model.state(&[0, 4])
    else {
        panic!("n = 4 is outside 0..=3");
    };
    assert_eq!(variable.as_str(), "n");
    assert_eq!(value, 4);
    assert_eq!((domain.lo(), domain.hi()), (0, 3));

    let Err(EvaluationError::StateOutOfDomain { variable, .. }) = model.state(&[-1, 0]) else {
        panic!("flag = -1 is outside 0..=1");
    };
    assert_eq!(variable.as_str(), "flag");
}

#[test]
fn domain_cardinality_is_the_product_of_the_variable_ranges() {
    let model = guarded();
    // flag: 2 values, n: 4 values.
    assert_eq!(model.domain_cardinality(), 8);
}

// ---------------------------------------------------------------------------
// enabledness and successors
// ---------------------------------------------------------------------------

#[test]
fn a_guard_decides_enabledness() {
    let model = guarded();
    let step = model.action_index("Step").expect("declared");
    let wrap = model.action_index("Wrap").expect("declared");
    let low = state(&model, &[0, 0]);
    let top = state(&model, &[0, 3]);
    assert_eq!(model.is_enabled(step, &low), Ok(true));
    assert_eq!(model.is_enabled(wrap, &low), Ok(false));
    assert_eq!(model.is_enabled(step, &top), Ok(false));
    assert_eq!(model.is_enabled(wrap, &top), Ok(true));
}

#[test]
fn a_disabled_action_contributes_no_successors() {
    let model = guarded();
    assert_eq!(
        targets(&model, &state(&model, &[0, 3]), "Step"),
        Vec::<Vec<i64>>::new()
    );
    assert_eq!(
        targets(&model, &state(&model, &[0, 0]), "Wrap"),
        Vec::<Vec<i64>>::new()
    );
}

/// An enumerated action yields one successor per *distinct* outcome, ascending.
#[test]
fn an_enumerated_action_yields_its_outcomes_in_ascending_order() {
    let model = guarded();
    assert_eq!(
        targets(&model, &state(&model, &[0, 0]), "Pick"),
        vec![vec![0, 0], vec![1, 0]]
    );
}

/// Two outcomes that land on the same state are one successor: the wire form forbids
/// a row from carrying the same `(action, target)` twice
/// (`crates/continuum-kernel-core/src/wire.rs:786-794`).
#[test]
fn outcomes_that_agree_collapse_to_one_successor() {
    let model = ModelBuilder::new()
        .variable("x", 0, 1)
        .initial_state(&[("x", 0)])
        .action(ActionDecl::enumerated(
            "Twice",
            BoolExpr::Const(true),
            vec![
                vec![("x", IntExpr::constant(1))],
                vec![("x", IntExpr::constant(1))],
            ],
        ))
        .build()
        .expect("valid");
    assert_eq!(
        targets(&model, &state(&model, &[0]), "Twice"),
        vec![vec![1]]
    );
}

/// A variable no outcome assigns is unchanged — the frame rule, stated once in
/// `Outcome`'s documentation and asserted here.
#[test]
fn unassigned_variables_are_unchanged() {
    let model = guarded();
    // `Step` assigns only `n`; `flag` keeps its value.
    assert_eq!(
        targets(&model, &state(&model, &[1, 1]), "Step"),
        vec![vec![1, 2]]
    );
}

/// Every right-hand side reads the pre-state, so a two-variable swap is a swap and
/// not an aliasing accident.
#[test]
fn assignments_within_one_outcome_are_simultaneous() {
    let model = ModelBuilder::new()
        .variable("a", 0, 9)
        .variable("b", 0, 9)
        .initial_state(&[("a", 1), ("b", 2)])
        .action(ActionDecl::deterministic(
            "Swap",
            BoolExpr::Const(true),
            vec![("a", IntExpr::var("b")), ("b", IntExpr::var("a"))],
        ))
        .build()
        .expect("valid");
    assert_eq!(
        targets(&model, &state(&model, &[1, 2]), "Swap"),
        vec![vec![2, 1]]
    );
}

/// `successors` is strictly ascending by `(action index, target vector)` — the exact
/// order one certificate row must be written in.
#[test]
fn successors_are_strictly_ascending_by_action_then_target() {
    let model = guarded();
    let steps = model
        .successors(&state(&model, &[0, 0]))
        .expect("evaluation succeeds");
    let keys: Vec<(usize, Vec<i64>)> = steps
        .iter()
        .map(|step| (step.action(), step.target().as_slice().to_vec()))
        .collect();
    // Pick(0) -> (0,0) and (1,0); Step(1) -> (0,1); Wrap(2) disabled.
    assert_eq!(
        keys,
        vec![(0, vec![0, 0]), (0, vec![1, 0]), (1, vec![0, 1]),]
    );
    for pair in keys.windows(2) {
        assert!(pair[0] < pair[1], "{pair:?} is not strictly ascending");
    }
}

/// A state where nothing is enabled has no successors, and that is the only way to
/// get an empty list — docs/03 §6.1's "disabled witness" at row level.
#[test]
fn a_deadlock_is_an_empty_successor_list() {
    let model = ModelBuilder::new()
        .variable("x", 0, 1)
        .initial_state(&[("x", 0)])
        .action(ActionDecl::deterministic(
            "Once",
            BoolExpr::compare(CmpOp::Eq, IntExpr::var("x"), IntExpr::constant(0)),
            vec![("x", IntExpr::constant(1))],
        ))
        .build()
        .expect("valid");
    let live = model.successors(&state(&model, &[0])).expect("evaluates");
    assert_eq!(live.len(), 1);
    let dead = model.successors(&state(&model, &[1])).expect("evaluates");
    assert!(dead.is_empty());
}

// ---------------------------------------------------------------------------
// typed failures
// ---------------------------------------------------------------------------

/// An enabled action that computes a value outside the target's domain is docs/16
/// PO-MOD-003 failing. It is reported, not clamped and not panicked.
#[test]
fn an_update_that_leaves_the_domain_is_a_typed_error() {
    let model = ModelBuilder::new()
        .variable("x", 0, 2)
        .initial_state(&[("x", 0)])
        .action(ActionDecl::deterministic(
            "Overshoot",
            BoolExpr::Const(true),
            vec![("x", IntExpr::plus(IntExpr::var("x"), IntExpr::constant(2)))],
        ))
        .build()
        .expect("valid");
    // From 0 the update lands on 2, which is in domain.
    assert_eq!(
        targets(&model, &state(&model, &[0]), "Overshoot"),
        vec![vec![2]]
    );
    // From 2 it lands on 4, which is not.
    let outcome = model.action_successors(0, &state(&model, &[2]));
    let Err(EvaluationError::UpdateOutOfDomain {
        action,
        variable,
        value,
        domain,
    }) = outcome
    else {
        panic!("expected an out-of-domain update, got {outcome:?}");
    };
    assert_eq!(action.as_str(), "Overshoot");
    assert_eq!(variable.as_str(), "x");
    assert_eq!(value, 4);
    assert_eq!((domain.lo(), domain.hi()), (0, 2));
}

/// Arithmetic that `i64` cannot represent is a value, not a panic — and not a
/// silently wrapped result, which is what `release` would otherwise produce.
#[test]
fn arithmetic_overflow_is_a_typed_error() {
    let model = ModelBuilder::new()
        .variable("x", i64::MAX - 1, i64::MAX)
        .initial_state(&[("x", i64::MAX)])
        .action(ActionDecl::deterministic(
            "Bump",
            BoolExpr::Const(true),
            vec![("x", IntExpr::plus(IntExpr::var("x"), IntExpr::constant(1)))],
        ))
        .build()
        .expect("valid");
    let outcome = model.action_successors(0, &state(&model, &[i64::MAX]));
    assert_eq!(
        outcome,
        Err(EvaluationError::Expression(EvalError::Overflow {
            operator: ArithOp::Add,
            left: i64::MAX,
            right: 1,
        }))
    );
}

#[test]
fn an_out_of_range_action_or_predicate_index_is_a_typed_error() {
    let model = guarded();
    let s = state(&model, &[0, 0]);
    assert_eq!(
        model.is_enabled(9, &s),
        Err(EvaluationError::UnknownAction {
            index: 9,
            declared: 3
        })
    );
    assert_eq!(
        model.evaluate_predicate(7, &s),
        Err(EvaluationError::UnknownPredicate {
            index: 7,
            declared: 2
        })
    );
}

/// An expression deeper than `MAX_EXPR_DEPTH` is refused by evaluation as well as by
/// the builder, so a hand-built expression cannot overflow the stack.
#[test]
fn evaluation_refuses_an_over_deep_expression() {
    let mut expr = IntExpr::constant(0);
    for _ in 0..MAX_EXPR_DEPTH {
        expr = IntExpr::plus(expr, IntExpr::constant(1));
    }
    assert!(expr.depth() > MAX_EXPR_DEPTH);
    let model = guarded();
    let here = state(&model, &[0, 0]);
    let environment =
        continuum_engine_reference::expr::Environment::new(model.variables(), here.as_slice());
    assert_eq!(
        expr.evaluate(&environment),
        Err(EvalError::TooDeep {
            limit: MAX_EXPR_DEPTH
        })
    );
}

/// An environment that binds nothing reports the name rather than guessing a value.
#[test]
fn an_unbound_variable_is_a_typed_error() {
    let environment = continuum_engine_reference::expr::Environment::new(&[], &[]);
    assert_eq!(
        IntExpr::var("ghost").evaluate(&environment),
        Err(EvalError::Unbound {
            name: "ghost".to_owned()
        })
    );
}

// ---------------------------------------------------------------------------
// predicates
// ---------------------------------------------------------------------------

/// A predicate is a named boolean evaluation and nothing else: this layer evaluates
/// `AtTop`, and says nothing about whether it is an invariant or a goal.
#[test]
fn predicates_evaluate_over_states() {
    let model = guarded();
    let at_top = model.predicate_index("AtTop").expect("declared");
    let in_bounds = model.predicate_index("InBounds").expect("declared");
    assert_eq!(
        model.evaluate_predicate(at_top, &state(&model, &[0, 3])),
        Ok(true)
    );
    assert_eq!(
        model.evaluate_predicate(at_top, &state(&model, &[0, 2])),
        Ok(false)
    );
    for n in 0..=3 {
        assert_eq!(
            model.evaluate_predicate(in_bounds, &state(&model, &[0, n])),
            Ok(true)
        );
    }
}

/// The connectives mean what they say, including the non-short-circuiting choice: an
/// overflow under a `false` conjunct is still reported.
#[test]
fn boolean_connectives_evaluate_both_operands() {
    let model = ModelBuilder::new()
        .variable("x", i64::MAX - 1, i64::MAX)
        .initial_state(&[("x", i64::MAX)])
        .action(ActionDecl::deterministic(
            "Nop",
            BoolExpr::Const(true),
            vec![],
        ))
        .predicate(
            "Guarded",
            BoolExpr::and(
                BoolExpr::Const(false),
                BoolExpr::compare(
                    CmpOp::Gt,
                    IntExpr::plus(IntExpr::var("x"), IntExpr::constant(1)),
                    IntExpr::constant(0),
                ),
            ),
        )
        .build()
        .expect("valid");
    let outcome = model.evaluate_predicate(0, &state(&model, &[i64::MAX]));
    assert!(
        matches!(
            outcome,
            Err(EvaluationError::Expression(EvalError::Overflow { .. }))
        ),
        "a short-circuiting evaluator would have answered Ok(false); got {outcome:?}"
    );
}

#[test]
fn implication_and_negation_have_their_usual_meaning() {
    let model = ModelBuilder::new()
        .variable("x", 0, 1)
        .initial_state(&[("x", 0)])
        .action(ActionDecl::deterministic(
            "Nop",
            BoolExpr::Const(true),
            vec![],
        ))
        .predicate(
            "Implies",
            BoolExpr::implies(
                BoolExpr::compare(CmpOp::Eq, IntExpr::var("x"), IntExpr::constant(1)),
                BoolExpr::Const(false),
            ),
        )
        .predicate("Negate", BoolExpr::negate(BoolExpr::Const(false)))
        .build()
        .expect("valid");
    let implies = model.predicate_index("Implies").expect("declared");
    let negate = model.predicate_index("Negate").expect("declared");
    assert_eq!(
        model.evaluate_predicate(implies, &state(&model, &[0])),
        Ok(true)
    );
    assert_eq!(
        model.evaluate_predicate(implies, &state(&model, &[1])),
        Ok(false)
    );
    assert_eq!(
        model.evaluate_predicate(negate, &state(&model, &[0])),
        Ok(true)
    );
}

/// `min`, `max`, and `*` are the rest of the arithmetic surface.
#[test]
fn min_max_and_multiplication_evaluate() {
    let model = ModelBuilder::new()
        .variable("x", 0, 4)
        .initial_state(&[("x", 0)])
        .action(ActionDecl::deterministic(
            "Clamp",
            BoolExpr::Const(true),
            vec![(
                "x",
                IntExpr::max(
                    IntExpr::constant(1),
                    IntExpr::min(
                        IntExpr::times(IntExpr::var("x"), IntExpr::constant(2)),
                        IntExpr::constant(4),
                    ),
                ),
            )],
        ))
        .build()
        .expect("valid");
    assert_eq!(
        targets(&model, &state(&model, &[0]), "Clamp"),
        vec![vec![1]]
    );
    assert_eq!(
        targets(&model, &state(&model, &[1]), "Clamp"),
        vec![vec![2]]
    );
    assert_eq!(
        targets(&model, &state(&model, &[3]), "Clamp"),
        vec![vec![4]]
    );
    assert_eq!(
        targets(&model, &state(&model, &[4]), "Clamp"),
        vec![vec![4]]
    );
}

// ---------------------------------------------------------------------------
// determinism
// ---------------------------------------------------------------------------

/// INV-005 at this layer: two enumerations of one model over one state are
/// byte-identical, not merely equivalent. Rendering through `Debug` and comparing the
/// bytes is deliberate — it catches an ordering difference that `==` on a set would
/// hide.
#[test]
fn two_enumerations_of_the_same_model_are_byte_identical() {
    let first = guarded();
    let second = guarded();
    let mut left = String::new();
    let mut right = String::new();
    for n in 0..=3 {
        for flag in 0..=1 {
            let a = state(&first, &[flag, n]);
            let b = state(&second, &[flag, n]);
            left.push_str(&format!("{:?}\n", first.successors(&a).expect("evaluates")));
            right.push_str(&format!(
                "{:?}\n",
                second.successors(&b).expect("evaluates")
            ));
        }
    }
    assert_eq!(left.as_bytes(), right.as_bytes());
    assert!(!left.is_empty());
}

/// The same, one level down: repeating a single call gives the same `Vec<Step>` in the
/// same order every time.
#[test]
fn repeating_one_enumeration_gives_the_same_sequence() {
    let model = guarded();
    let s = state(&model, &[0, 1]);
    let first: Vec<Step> = model.successors(&s).expect("evaluates");
    for _ in 0..8 {
        assert_eq!(model.successors(&s).expect("evaluates"), first);
    }
}
