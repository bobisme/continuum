//! TEST-3-08 (docs/19 §3, "equivalent guard normalization"), bn-33g98.
//!
//! `metamorphic.rs`'s `equivalent_guard_normalization_preserves_identity` already binds
//! the headline relation `norm.rs` documents: `require a && b`, or two `require`
//! clauses, is one list of guard conjuncts; `x' == x` and `unchanged x` are one frame;
//! `=` and `==` are one equality; the two header styles are one model. This file adds
//! three things the policy evidence (`tools/test-policy/sections/
//! s3_metamorphic_relations_07_10.py`) requires beyond that single positive case:
//!
//! 1. A control that isolates guard-conjunct splitting from the other equivalences,
//!    with more than two conjuncts and two different `&&` associations, so the
//!    relation is shown to hold generally and not only for the one two-conjunct,
//!    two-variable example already committed.
//! 2. A negative control: guards that really differ in meaning (a dropped conjunct, a
//!    changed bound) keep different identities after normalization.
//! 3. A mutant: a hand-built `NormModel`, built from the real elaborator's own output,
//!    with the splitting rule "disabled" — the guard kept as one unsplit `&&`
//!    expression instead of `norm.rs`'s documented list of conjuncts. The identities
//!    then differ, which is exactly what would make
//!    `equivalent_guard_normalization_preserves_identity` fail if `elab.rs`'s
//!    `conjuncts` function (the real implementation of this rule) stopped splitting.
//!
//! Only rules `norm.rs` and RFC 0003 actually document are exercised here. RFC 0003
//! and `elab.rs` have no general commutative rewrite of `&&`/`||` operands, no general
//! double-negation elimination, and no general comparison-flip rewrite (`flip`/
//! `negate` in `elab.rs` exist only inside the recursive-def termination heuristic,
//! `measure_test`, and never touch a guard's normalized form) — so this file does not
//! claim those as preserved relations, per the policy's own "use only the rules the
//! normalizer actually implements" bar. Swapping the order of two conjuncts, for
//! example, is *not* covered: `Action::guard` keeps clauses "in source order"
//! (`norm.rs`), so a real reorder is a real difference here, not an equivalence.

use continuum_cml_elab::norm::{BinOp, Expr, ExprKind};
use continuum_cml_elab::{NormModel, Type, elaborate_source, lower};
use continuum_cml_syntax::Span;

fn elab(src: &str) -> NormModel {
    elaborate_source(src).unwrap_or_else(|e| panic!("elaborates: {e}\n{src}"))
}

const SPAN: Span = Span {
    start: 0,
    end: 0,
    line: 1,
    col: 1,
};

fn bool_expr(kind: ExprKind) -> Expr {
    Expr {
        kind,
        ty: Type::Bool,
        span: SPAN,
    }
}

const LEFT_NESTED: &str = "\
module Nested
state { x: Nat where x <= 5 }
init { x == 0 }
action Step {
  require (x < 5 && x >= 0) && x != 3
  next x = x + 1
}
";

const RIGHT_NESTED: &str = "\
module Nested
state { x: Nat where x <= 5 }
init { x == 0 }
action Step {
  require x < 5 && (x >= 0 && x != 3)
  next x = x + 1
}
";

const THREE_REQUIRES: &str = "\
module Nested
state { x: Nat where x <= 5 }
init { x == 0 }
action Step {
  require x < 5
  require x >= 0
  require x != 3
  next x = x + 1
}
";

/// Relation: "equivalent guard normalization", isolated to conjunction splitting alone
/// (no `unchanged`/`=`/header variation mixed in, unlike `metamorphic.rs`'s example),
/// and with three conjuncts associated two different ways. All three sources must
/// normalize to one form and one identity, both for `NormModel` and for the lowered
/// programmatic `Model`.
#[test]
fn nested_conjunction_splits_the_same_as_three_separate_requires() {
    let a = elab(LEFT_NESTED);
    let b = elab(RIGHT_NESTED);
    let c = elab(THREE_REQUIRES);

    assert_eq!(
        a.identity(),
        b.identity(),
        "left- and right-associated && are one guard"
    );
    assert_eq!(
        a.identity(),
        c.identity(),
        "a nested && chain and three separate requires are one guard"
    );

    assert_eq!(
        lower(&a).expect("lowers").identity(),
        lower(&c).expect("lowers").identity(),
        "the lowered programmatic model agrees: one guard, one identity"
    );
}

/// Expected non-preservation, specific to guards (the sibling module's own
/// `a_change_of_meaning_changes_identity` uses an invariant, not a guard): a dropped
/// conjunct or a changed bound is a real change of meaning, and normalization must not
/// paper over it.
#[test]
fn guards_that_differ_in_meaning_keep_different_identities() {
    let base = THREE_REQUIRES;
    let dropped_conjunct = "\
module Nested
state { x: Nat where x <= 5 }
init { x == 0 }
action Step {
  require x < 5
  require x >= 0
  next x = x + 1
}
";
    let changed_bound = "\
module Nested
state { x: Nat where x <= 5 }
init { x == 0 }
action Step {
  require x < 4
  require x >= 0
  require x != 3
  next x = x + 1
}
";

    let a = elab(base);
    assert_ne!(
        a.identity(),
        elab(dropped_conjunct).identity(),
        "dropping a conjunct changes the guard"
    );
    assert_ne!(
        a.identity(),
        elab(changed_bound).identity(),
        "a different bound changes the guard"
    );
}

/// Mutant: take the real elaborator's output for `THREE_REQUIRES` — already proven
/// equal to `LEFT_NESTED`/`RIGHT_NESTED` above — and simulate disabling the splitting
/// rule by merging its guard list back into one unsplit `&&` expression, the shape
/// `norm.rs` documents `Action::guard` as never holding ("a list of conjuncts", not one
/// conjunction). This is the mutant the policy evidence asks for: it shows that the
/// real rule (`elab.rs`'s `conjuncts`, called from `split_clauses`) is load-bearing —
/// if it stopped splitting `&&`, the elaborated model would stop agreeing with itself
/// on the meaning of "the same guard, spelled two ways", and this equality would fail
/// instead of the one two lines above it.
#[test]
fn disabling_conjunct_splitting_breaks_the_relation() {
    let split = elab(THREE_REQUIRES);
    assert_eq!(
        split.identity(),
        elab(LEFT_NESTED).identity(),
        "sanity: the real, correctly split elaboration still agrees before mutation"
    );

    let mut unsplit = split.clone();
    let action = unsplit
        .actions
        .iter_mut()
        .find(|a| a.name == "Step")
        .expect("Step is the model's only action");
    let mut clauses = std::mem::take(&mut action.guard).into_iter();
    let first = clauses.next().expect("Step has at least one guard clause");
    action.guard = vec![clauses.fold(first, |acc, next| {
        bool_expr(ExprKind::Binary(BinOp::And, Box::new(acc), Box::new(next)))
    })];

    assert_ne!(
        split.identity(),
        unsplit.identity(),
        "an unsplit guard conjunction is not norm.rs's documented normalized form — \
         disabling conjunct splitting must change the identity, or the relation this \
         obligation names would not be testing anything"
    );
}
