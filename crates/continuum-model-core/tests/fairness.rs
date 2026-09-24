//! Fairness declarations on the programmatic model (bn-1ln12; docs/02 §8, RFC 0008
//! "Fairness scopes").
//!
//! What is pinned: a scope resolves to action indices and is a set; assumptions are
//! canonical; every malformed declaration is a typed `ModelError`; and a scope is
//! enabled exactly where one of its actions is.

use continuum_model_core::fairness::MAX_FAIRNESS;
use continuum_model_core::model::Symbol;
use continuum_model_core::{
    ActionDecl, BoolExpr, CmpOp, EvaluationError, IntExpr, ModelBuilder, ModelError, Strength,
};

/// `x` counts `0..=2`: `Inc` below 2, `Reset` at 2, `Idle` never.
fn counter() -> ModelBuilder {
    let x = || IntExpr::var("x");
    ModelBuilder::new()
        .variable("x", 0, 2)
        .initial_state(&[("x", 0)])
        .action(ActionDecl::deterministic(
            "Inc",
            BoolExpr::compare(CmpOp::Lt, x(), IntExpr::constant(2)),
            vec![("x", IntExpr::plus(x(), IntExpr::constant(1)))],
        ))
        .action(ActionDecl::deterministic(
            "Reset",
            BoolExpr::compare(CmpOp::Eq, x(), IntExpr::constant(2)),
            vec![("x", IntExpr::constant(0))],
        ))
        .action(ActionDecl::deterministic(
            "Idle",
            BoolExpr::Const(false),
            vec![],
        ))
}

#[test]
fn a_scope_resolves_to_sorted_action_indices() {
    let model = counter()
        .fairness(Strength::Strong, ["Reset", "Idle"])
        .fairness(Strength::Weak, ["Inc"])
        .build()
        .expect("valid");
    // Actions are in name order: Idle 0, Inc 1, Reset 2.
    let got: Vec<(Strength, Vec<usize>)> = model
        .fairness()
        .iter()
        .map(|f| (f.strength(), f.actions().to_vec()))
        .collect();
    assert_eq!(
        got,
        vec![(Strength::Weak, vec![1]), (Strength::Strong, vec![0, 2])]
    );
    assert!(model.fairness()[1].contains(2));
    assert!(!model.fairness()[1].contains(1));
    assert_eq!(Strength::Weak.to_string(), "weak");
    assert_eq!(Strength::Strong.as_str(), "strong");
}

#[test]
fn a_model_without_fairness_declares_none() {
    let model = counter().build().expect("valid");
    assert!(model.fairness().is_empty(), "no hidden default (RFC 0008)");
}

#[test]
fn malformed_fairness_is_a_typed_refusal() {
    let empty: [&str; 0] = [];
    assert_eq!(
        counter().fairness(Strength::Weak, empty).build(),
        Err(ModelError::EmptyFairness { index: 0 })
    );
    let unknown = counter()
        .fairness(Strength::Weak, ["Inc"])
        .fairness(Strength::Strong, ["Inc", "Nope"])
        .build()
        .expect_err("undeclared");
    match unknown {
        ModelError::UnknownAction { fairness, name } => {
            assert_eq!((fairness, name.as_str()), (1, "Nope"));
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        counter().fairness(Strength::Weak, ["not a name"]).build(),
        Err(ModelError::InvalidName {
            symbol: Symbol::Action,
            ..
        })
    ));
    let mut many = counter();
    for _ in 0..=MAX_FAIRNESS {
        many = many.fairness(Strength::Weak, ["Inc"]);
    }
    assert_eq!(
        many.build(),
        Err(ModelError::TooMany {
            symbol: Symbol::Fairness,
            count: MAX_FAIRNESS + 1,
            max: MAX_FAIRNESS,
        })
    );
    // Exactly at the limit is accepted, and the repeats collapse to one assumption.
    let mut at_limit = counter();
    for _ in 0..MAX_FAIRNESS {
        at_limit = at_limit.fairness(Strength::Weak, ["Inc"]);
    }
    assert_eq!(at_limit.build().expect("at the limit").fairness().len(), 1);
}

#[test]
fn a_scope_is_enabled_where_any_of_its_actions_is() {
    let model = counter()
        .fairness(Strength::Weak, ["Inc", "Reset"])
        .fairness(Strength::Weak, ["Idle"])
        .fairness(Strength::Weak, ["Reset"])
        .build()
        .expect("valid");
    // Canonical order: [Idle]=[0], [Inc, Reset]=[1, 2], [Reset]=[2].
    for x in 0..=2 {
        let state = model.state(&[x]).expect("in domain");
        assert!(!model.is_fairness_enabled(0, &state).expect("evaluates"));
        assert!(model.is_fairness_enabled(1, &state).expect("evaluates"));
        assert_eq!(
            model.is_fairness_enabled(2, &state).expect("evaluates"),
            x == 2
        );
    }
    let state = model.state(&[0]).expect("in domain");
    assert_eq!(
        model.is_fairness_enabled(3, &state),
        Err(EvaluationError::UnknownFairness {
            index: 3,
            declared: 3
        })
    );
}
