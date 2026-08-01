//! Declarations that are not models (PR 8, IMPL-01).
//!
//! Every case here is a model someone could plausibly write by accident, and every
//! one must come back as a typed [`ModelError`] naming what is wrong and where.
//! Nothing may panic, and nothing may be silently repaired: docs/12 §1's
//! "ambiguous behavior is an error, not implementation freedom" means a declaration
//! that could mean two things means neither.
//!
//! The suite is linear in the number of rules, not combinatorial: one case per
//! rejection arm, plus the two size limits.

use continuum_engine_reference::domain::DomainError;
use continuum_engine_reference::expr::{BoolExpr, CmpOp, IntExpr, MAX_EXPR_DEPTH};
use continuum_engine_reference::ident::{IdentError, MAX_IDENT_BYTES};
use continuum_engine_reference::model::{ActionDecl, ModelBuilder, ModelError, Site, Symbol};

/// A model that is valid except for whatever the caller changes.
fn ok() -> ModelBuilder {
    ModelBuilder::new()
        .variable("x", 0, 2)
        .initial_state(&[("x", 0)])
        .action(ActionDecl::deterministic(
            "Nop",
            BoolExpr::Const(true),
            vec![],
        ))
}

fn nop() -> ActionDecl {
    ActionDecl::deterministic("Nop", BoolExpr::Const(true), vec![])
}

// ---------------------------------------------------------------------------
// the three "at least one" rules
// ---------------------------------------------------------------------------

#[test]
fn a_model_with_no_variables_is_rejected() {
    let outcome = ModelBuilder::new().action(nop()).initial_state(&[]).build();
    assert_eq!(outcome.err(), Some(ModelError::NoVariables));
}

/// The wire form requires at least one action
/// (`crates/continuum-kernel-core/src/wire.rs:750`).
#[test]
fn a_model_with_no_actions_is_rejected() {
    let outcome = ModelBuilder::new()
        .variable("x", 0, 1)
        .initial_state(&[("x", 0)])
        .build();
    assert_eq!(outcome.err(), Some(ModelError::NoActions));
}

/// And at least one initial state
/// (`crates/continuum-kernel-core/src/wire.rs:729-732`).
#[test]
fn a_model_with_no_initial_states_is_rejected() {
    let outcome = ModelBuilder::new()
        .variable("x", 0, 1)
        .action(nop())
        .build();
    assert_eq!(outcome.err(), Some(ModelError::NoInitialStates));
}

// ---------------------------------------------------------------------------
// names
// ---------------------------------------------------------------------------

#[test]
fn a_duplicate_variable_name_is_rejected() {
    let outcome = ModelBuilder::new()
        .variable("x", 0, 1)
        .variable("x", 0, 5)
        .initial_state(&[("x", 0)])
        .action(nop())
        .build();
    let Some(ModelError::Duplicate { symbol, name }) = outcome.err() else {
        panic!("two variables named `x` must be rejected");
    };
    assert_eq!(symbol, Symbol::Variable);
    assert_eq!(name.as_str(), "x");
}

#[test]
fn a_duplicate_action_name_is_rejected() {
    let outcome = ok()
        .action(ActionDecl::deterministic(
            "Twin",
            BoolExpr::Const(true),
            vec![],
        ))
        .action(ActionDecl::deterministic(
            "Twin",
            BoolExpr::Const(false),
            vec![],
        ))
        .build();
    let Some(ModelError::Duplicate { symbol, name }) = outcome.err() else {
        panic!("two actions named `Twin` must be rejected");
    };
    assert_eq!(symbol, Symbol::Action);
    assert_eq!(name.as_str(), "Twin");
}

#[test]
fn a_duplicate_predicate_name_is_rejected() {
    let outcome = ok()
        .predicate("P", BoolExpr::Const(true))
        .predicate("P", BoolExpr::Const(false))
        .build();
    let Some(ModelError::Duplicate { symbol, name }) = outcome.err() else {
        panic!("two predicates named `P` must be rejected");
    };
    assert_eq!(symbol, Symbol::Predicate);
    assert_eq!(name.as_str(), "P");
}

#[test]
fn an_empty_name_is_rejected() {
    let outcome = ModelBuilder::new()
        .variable("", 0, 1)
        .initial_state(&[("", 0)])
        .action(nop())
        .build();
    let Some(ModelError::InvalidName {
        symbol,
        spelling,
        source,
    }) = outcome.err()
    else {
        panic!("the empty string is not a name");
    };
    assert_eq!(symbol, Symbol::Variable);
    assert_eq!(spelling, "");
    assert_eq!(source, IdentError::Empty);
}

/// Two spellings of one name would make a state vector's positions ambiguous, so the
/// grammar is the kernel's: printable ASCII only.
#[test]
fn a_non_ascii_name_is_rejected() {
    let outcome = ModelBuilder::new()
        .variable("héight", 0, 1)
        .initial_state(&[("héight", 0)])
        .action(nop())
        .build();
    let Some(ModelError::InvalidName { source, .. }) = outcome.err() else {
        panic!("a non-ASCII name must be rejected");
    };
    assert!(matches!(
        source,
        IdentError::NotPrintableAscii { index: 1, .. }
    ));
}

/// A name the certificate's token decoder could not carry
/// (`MAX_TOKEN_BYTES`, `crates/continuum-kernel-core/src/wire.rs:119`).
#[test]
fn an_over_long_name_is_rejected() {
    let long = "v".repeat(MAX_IDENT_BYTES + 1);
    let outcome = ModelBuilder::new()
        .variable(&long, 0, 1)
        .initial_state(&[(&long, 0)])
        .action(nop())
        .build();
    let Some(ModelError::InvalidName { source, .. }) = outcome.err() else {
        panic!("an over-long name must be rejected");
    };
    assert_eq!(
        source,
        IdentError::TooLong {
            bytes: MAX_IDENT_BYTES + 1,
            max: MAX_IDENT_BYTES
        }
    );
}

// ---------------------------------------------------------------------------
// domains
// ---------------------------------------------------------------------------

/// An empty domain makes the whole state space empty while looking like an ordinary
/// declaration. The kernel refuses `lo > hi` too
/// (`crates/continuum-kernel-core/src/wire.rs:531-533`).
#[test]
fn a_variable_with_an_empty_domain_is_rejected() {
    let outcome = ModelBuilder::new()
        .variable("x", 3, 2)
        .initial_state(&[("x", 3)])
        .action(nop())
        .build();
    let Some(ModelError::InvalidDomain { variable, source }) = outcome.err() else {
        panic!("3..=2 admits no value");
    };
    assert_eq!(variable.as_str(), "x");
    assert_eq!(source, DomainError::Empty { lo: 3, hi: 2 });
}

/// A single-valued domain is not empty and is accepted.
#[test]
fn a_singleton_domain_is_accepted() {
    let model = ModelBuilder::new()
        .variable("x", 7, 7)
        .initial_state(&[("x", 7)])
        .action(nop())
        .build()
        .expect("7..=7 admits exactly one value");
    assert_eq!(model.domain_cardinality(), 1);
}

// ---------------------------------------------------------------------------
// expressions
// ---------------------------------------------------------------------------

#[test]
fn an_undeclared_variable_in_a_guard_is_rejected() {
    let outcome = ok()
        .action(ActionDecl::deterministic(
            "Ghosted",
            BoolExpr::compare(CmpOp::Eq, IntExpr::var("ghost"), IntExpr::constant(0)),
            vec![],
        ))
        .build();
    let Some(ModelError::UnknownVariable { site, name }) = outcome.err() else {
        panic!("a guard over an undeclared variable must be rejected");
    };
    assert_eq!(name.as_str(), "ghost");
    let Site::Guard(action) = site else {
        panic!("the site is the guard");
    };
    assert_eq!(action.as_str(), "Ghosted");
}

#[test]
fn an_undeclared_assignment_target_is_rejected() {
    let outcome = ok()
        .action(ActionDecl::deterministic(
            "Writes",
            BoolExpr::Const(true),
            vec![("ghost", IntExpr::constant(0))],
        ))
        .build();
    let Some(ModelError::UnknownVariable { site, name }) = outcome.err() else {
        panic!("assigning an undeclared variable must be rejected");
    };
    assert_eq!(name.as_str(), "ghost");
    assert!(matches!(site, Site::Update { .. }), "{site:?}");
}

#[test]
fn an_undeclared_variable_in_a_predicate_is_rejected() {
    let outcome = ok()
        .predicate("Bad", BoolExpr::in_range(IntExpr::var("ghost"), 0, 1))
        .build();
    let Some(ModelError::UnknownVariable { site, name }) = outcome.err() else {
        panic!("a predicate over an undeclared variable must be rejected");
    };
    assert_eq!(name.as_str(), "ghost");
    let Site::Predicate(predicate) = site else {
        panic!("the site is the predicate body");
    };
    assert_eq!(predicate.as_str(), "Bad");
}

/// An unspellable name inside an expression is `InvalidName`, not `UnknownVariable`:
/// "no model could declare this" and "this model does not declare it" are different
/// mistakes.
#[test]
fn an_unspellable_name_in_an_expression_is_an_invalid_name() {
    let outcome = ok()
        .predicate("Bad", BoolExpr::in_range(IntExpr::var("has space"), 0, 1))
        .build();
    let Some(ModelError::InvalidName { spelling, .. }) = outcome.err() else {
        panic!("`has space` is not a canonical name");
    };
    assert_eq!(spelling, "has space");
}

/// Two assignments to one variable in one outcome would make the post-state depend on
/// which was applied last, which is precisely the ambiguity that is an error.
#[test]
fn assigning_one_variable_twice_in_one_outcome_is_rejected() {
    let outcome = ok()
        .action(ActionDecl::deterministic(
            "Racy",
            BoolExpr::Const(true),
            vec![("x", IntExpr::constant(0)), ("x", IntExpr::constant(1))],
        ))
        .build();
    let Some(ModelError::DuplicateAssignment { action, variable }) = outcome.err() else {
        panic!("two assignments to `x` must be rejected");
    };
    assert_eq!(action.as_str(), "Racy");
    assert_eq!(variable.as_str(), "x");
}

/// The same variable in *different* outcomes of one action is fine — that is what an
/// enumerated action is.
#[test]
fn assigning_one_variable_in_different_outcomes_is_accepted() {
    ModelBuilder::new()
        .variable("x", 0, 1)
        .initial_state(&[("x", 0)])
        .action(ActionDecl::enumerated(
            "Choose",
            BoolExpr::Const(true),
            vec![
                vec![("x", IntExpr::constant(0))],
                vec![("x", IntExpr::constant(1))],
            ],
        ))
        .build()
        .expect("an enumerated action may assign the same variable in each outcome");
}

#[test]
fn an_action_with_no_outcome_is_rejected() {
    let outcome = ok()
        .action(ActionDecl::enumerated(
            "Hole",
            BoolExpr::Const(true),
            vec![],
        ))
        .build();
    let Some(ModelError::ActionWithoutOutcome { action }) = outcome.err() else {
        panic!("an action with no outcome is not a relation");
    };
    assert_eq!(action.as_str(), "Hole");
}

/// An outcome with no assignments is a legitimate stutter, and is not the same thing
/// as an action with no outcome.
#[test]
fn an_outcome_with_no_assignments_is_a_stutter_and_is_accepted() {
    let model = ok().build().expect("`Nop` assigns nothing");
    let here = model.state(&[1]).expect("well typed");
    let steps = model.successors(&here).expect("evaluates");
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].target().as_slice(), &[1]);
}

#[test]
fn an_over_deep_expression_is_rejected_at_declaration() {
    let mut expr = IntExpr::var("x");
    for _ in 0..MAX_EXPR_DEPTH {
        expr = IntExpr::plus(expr, IntExpr::constant(0));
    }
    let outcome = ok()
        .predicate(
            "Deep",
            BoolExpr::compare(CmpOp::Eq, expr, IntExpr::constant(0)),
        )
        .build();
    let Some(ModelError::ExpressionTooDeep { site, depth, limit }) = outcome.err() else {
        panic!("an over-deep expression must be rejected before it is ever evaluated");
    };
    assert!(matches!(site, Site::Predicate(_)), "{site:?}");
    assert!(depth > limit, "{depth} should exceed {limit}");
    assert_eq!(limit, MAX_EXPR_DEPTH);
}

// ---------------------------------------------------------------------------
// initial states
// ---------------------------------------------------------------------------

/// There is no default value: an initial state that leaves a variable unbound is an
/// initial state nobody wrote down.
#[test]
fn an_incomplete_initial_state_is_rejected() {
    let outcome = ModelBuilder::new()
        .variable("x", 0, 1)
        .variable("y", 0, 1)
        .initial_state(&[("x", 0)])
        .action(nop())
        .build();
    let Some(ModelError::IncompleteInitialState { index, variable }) = outcome.err() else {
        panic!("`y` is unbound");
    };
    assert_eq!(index, 0);
    assert_eq!(variable.as_str(), "y");
}

#[test]
fn an_initial_state_that_binds_a_variable_twice_is_rejected() {
    let outcome = ModelBuilder::new()
        .variable("x", 0, 1)
        .initial_state(&[("x", 0), ("x", 1)])
        .action(nop())
        .build();
    let Some(ModelError::DuplicateInitialBinding { index, variable }) = outcome.err() else {
        panic!("`x` is bound twice");
    };
    assert_eq!(index, 0);
    assert_eq!(variable.as_str(), "x");
}

#[test]
fn an_initial_state_outside_a_domain_is_rejected() {
    let outcome = ModelBuilder::new()
        .variable("x", 0, 1)
        .initial_state(&[("x", 9)])
        .action(nop())
        .build();
    let Some(ModelError::InitialValueOutOfDomain {
        index,
        variable,
        value,
        domain,
    }) = outcome.err()
    else {
        panic!("9 is outside 0..=1");
    };
    assert_eq!(index, 0);
    assert_eq!(variable.as_str(), "x");
    assert_eq!(value, 9);
    assert_eq!((domain.lo(), domain.hi()), (0, 1));
}

#[test]
fn an_initial_state_naming_an_undeclared_variable_is_rejected() {
    let outcome = ModelBuilder::new()
        .variable("x", 0, 1)
        .initial_state(&[("x", 0), ("ghost", 0)])
        .action(nop())
        .build();
    let Some(ModelError::UnknownVariable { site, name }) = outcome.err() else {
        panic!("`ghost` is not declared");
    };
    assert_eq!(name.as_str(), "ghost");
    assert_eq!(site, Site::InitialState(0));
}

/// The wire form forbids a duplicate initial-state vector
/// (`crates/continuum-kernel-core/src/wire.rs:739-747`), so the model layer refuses to
/// declare one.
#[test]
fn a_repeated_initial_state_is_rejected() {
    let outcome = ModelBuilder::new()
        .variable("x", 0, 1)
        .initial_state(&[("x", 1)])
        .initial_state(&[("x", 1)])
        .action(nop())
        .build();
    let Some(ModelError::DuplicateInitialState { state }) = outcome.err() else {
        panic!("the same initial state twice must be rejected");
    };
    assert_eq!(state.as_slice(), &[1]);
}

/// Two initial states that differ are fine, and are stored ascending regardless of the
/// order they were declared in.
#[test]
fn two_distinct_initial_states_are_accepted_and_sorted() {
    let model = ModelBuilder::new()
        .variable("x", 0, 1)
        .initial_state(&[("x", 1)])
        .initial_state(&[("x", 0)])
        .action(nop())
        .build()
        .expect("valid");
    let vectors: Vec<&[i64]> = model
        .initial_states()
        .iter()
        .map(continuum_engine_reference::State::as_slice)
        .collect();
    assert_eq!(vectors, vec![[0].as_slice(), [1].as_slice()]);
}

// ---------------------------------------------------------------------------
// size limits
// ---------------------------------------------------------------------------

/// `MAX_VARIABLES` from the wire form (`crates/continuum-kernel-core/src/wire.rs:130`).
#[test]
fn more_variables_than_a_certificate_can_carry_are_rejected() {
    let limit = continuum_engine_reference::model::MAX_VARIABLES;
    let mut builder = ModelBuilder::new().action(nop());
    let mut bindings: Vec<(String, i64)> = Vec::new();
    for index in 0..=limit {
        let name = format!("v{index:03}");
        builder = builder.variable(&name, 0, 1);
        bindings.push((name, 0));
    }
    let borrowed: Vec<(&str, i64)> = bindings
        .iter()
        .map(|(name, value)| (name.as_str(), *value))
        .collect();
    let outcome = builder.initial_state(&borrowed).build();
    assert_eq!(
        outcome.err(),
        Some(ModelError::TooMany {
            symbol: Symbol::Variable,
            count: limit + 1,
            max: limit,
        })
    );
}

/// Exactly the limit is accepted; the boundary is inclusive on the legal side.
#[test]
fn exactly_the_variable_limit_is_accepted() {
    let limit = continuum_engine_reference::model::MAX_VARIABLES;
    let mut builder = ModelBuilder::new().action(nop());
    let mut bindings: Vec<(String, i64)> = Vec::new();
    for index in 0..limit {
        let name = format!("v{index:03}");
        builder = builder.variable(&name, 0, 1);
        bindings.push((name, 0));
    }
    let borrowed: Vec<(&str, i64)> = bindings
        .iter()
        .map(|(name, value)| (name.as_str(), *value))
        .collect();
    let model = builder
        .initial_state(&borrowed)
        .build()
        .expect("the limit itself is legal");
    assert_eq!(model.arity(), limit);
}
