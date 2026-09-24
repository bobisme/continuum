//! The chain differential: a left-deep operator chain through the CML front end against
//! the same chain built programmatically (bn-1nmq).
//!
//! bn-1nmq changed how the parser bounds an operator chain: it now bounds the depth of
//! the tree the chain builds, where it bounded the level each operand was entered at.
//! What must not change is what a chain *means*. So:
//!
//! - **subject**: a CML model whose update, guard, and invariant are left-deep chains
//!   of `+` and `-` as long as the lowering admits, parsed by `continuum-cml-syntax` and
//!   elaborated and lowered by `continuum_cml_elab`;
//! - **oracle**: the programmatic twin, built with `continuum_model_core::ModelBuilder`
//!   by folding the same operands to the left, and checked by
//!   `continuum_engine_reference`.
//!
//! The chains are subtraction-heavy, so a right-associated parse (`x - (1 - 1)`) would
//! compute different values and be told apart by the engine.

use continuum_cml_elab::{elaborate_source, lower};
use continuum_engine_reference::bfs::{self, Bounds, Exploration};
use continuum_engine_reference::checking::{self, DeadlockPolicy, Obligations};
use continuum_engine_reference::model::Model;
use continuum_model_core::{ActionDecl, BoolExpr, CmpOp, IntExpr, ModelBuilder};

/// Operands after the first in each chain. The lowering admits expressions up to
/// `continuum_model_core::expr::MAX_EXPR_DEPTH` (32) deep, so each chain has at most
/// `CHAIN + 2` operands under one comparison.
const CHAIN: usize = 26;

fn source() -> String {
    // `x + 26 - 1 - 1 - … - 1` (24 ones): x + 2, left-associated.
    let update = format!("x + {CHAIN}{}", " - 1".repeat(CHAIN - 2));
    // `x - 1 - 1 - … - 1 + 26` (25 ones): x + 1.
    let guard = format!("x{} + {CHAIN} <= 6", " - 1".repeat(CHAIN - 1));
    // `x - 1 - … - 1 + 19` (26 ones) `<= 0`: x <= 7.
    let invariant = format!("x{} + 19 <= 0", " - 1".repeat(CHAIN));
    format!(
        "module Chain\nstate {{ x: Int where x >= 0 && x <= 9 }}\ninit {{ x == 0 }}\n\
         action Up {{\n  require {guard}\n  next x = {update}\n}}\n\
         action Reset {{ next x = 0 }}\n\
         invariant Small {{ {invariant} }}\n"
    )
}

/// `first op o1 op o2 …`, folded to the left, as the parser must build it.
fn fold(first: IntExpr, rest: &[(bool, i64)]) -> IntExpr {
    rest.iter().fold(first, |acc, &(add, k)| {
        if add {
            IntExpr::plus(acc, IntExpr::constant(k))
        } else {
            IntExpr::minus(acc, IntExpr::constant(k))
        }
    })
}

fn programmatic() -> Model {
    let x = || IntExpr::var("x");
    let chain = i64::try_from(CHAIN).expect("small");
    let mut update = vec![(true, chain)];
    update.extend(std::iter::repeat_n((false, 1), CHAIN - 2));
    let mut guard: Vec<(bool, i64)> = std::iter::repeat_n((false, 1), CHAIN - 1).collect();
    guard.push((true, chain));
    let mut invariant: Vec<(bool, i64)> = std::iter::repeat_n((false, 1), CHAIN).collect();
    invariant.push((true, 19));
    let le = |left: IntExpr, right: i64| BoolExpr::Compare {
        op: CmpOp::Le,
        left,
        right: IntExpr::constant(right),
    };
    ModelBuilder::new()
        .variable("x", 0, 9)
        .initial_state(&[("x", 0)])
        .action(ActionDecl::deterministic(
            "Up",
            le(fold(x(), &guard), 6),
            vec![("x", fold(x(), &update))],
        ))
        .action(ActionDecl::deterministic(
            "Reset",
            BoolExpr::Const(true),
            vec![("x", IntExpr::constant(0))],
        ))
        .predicate("Small", le(fold(x(), &invariant), 0))
        .build()
        .expect("the programmatic twin is valid")
}

fn elaborated(src: &str) -> Model {
    let norm = elaborate_source(src).unwrap_or_else(|e| panic!("elaborates: {e}\n{src}"));
    lower(&norm).unwrap_or_else(|e| panic!("lowers: {e}\n{src}"))
}

fn explore(model: &Model) -> Exploration {
    bfs::explore(model, Bounds::CERTIFIABLE).expect("the chain model evaluates everywhere")
}

fn report(model: &Model) -> checking::CheckReport {
    let obligations = Obligations::every_predicate(model, DeadlockPolicy::Defect);
    checking::check(model, &explore(model), &obligations).expect("the predicate is declared")
}

/// A left-deep chain at the lowering's depth bound means the same through CML as through
/// the programmatic API: the reference engine explores the same states and transitions
/// and reports the same verdicts, and the two models are one model.
#[test]
fn a_left_deep_chain_lowers_to_the_programmatic_fold() {
    let subject = elaborated(&source());
    let oracle = programmatic();
    assert_eq!(explore(&subject), explore(&oracle), "exploration");
    assert_eq!(report(&subject), report(&oracle), "check report");
    let closed_states = explore(&oracle).closed().expect("closes").len();
    // x climbs 0, 2, 4, 6 (the guard x + 1 <= 6 stops it there), and Reset returns to 0.
    assert_eq!(closed_states, 4, "the chain arithmetic is exercised");
    assert_eq!(subject, oracle, "the two front ends built equal models");
    assert_eq!(subject.identity(), oracle.identity());
}

/// The differential is not vacuous: a right-associated reading of the same chain is a
/// different model, which the engine tells apart.
#[test]
fn a_right_associated_chain_is_told_apart() {
    let src = source();
    // Group the guard's ones to the right: x - (1 - 1 - … - 1) + 26 is x + 49, so `Up`
    // never fires, where the left fold x - 25 + 26 lets it fire from 0, 2, and 4.
    let tail = " - 1".repeat(CHAIN - 1);
    let grouped = src.replacen(
        &format!("x{tail} + {CHAIN} <= 6"),
        &format!("x - (1{}) + {CHAIN} <= 6", " - 1".repeat(CHAIN - 2)),
        1,
    );
    assert_ne!(grouped, src, "the regrouping applies");
    let subject = elaborated(&grouped);
    let oracle = programmatic();
    assert_ne!(subject.identity(), oracle.identity());
    assert_ne!(explore(&subject), explore(&oracle));
}
