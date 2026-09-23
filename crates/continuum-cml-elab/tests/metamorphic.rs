//! Metamorphic relations of elaboration (docs/19 §3), PR 15a, bn-ybq.
//!
//! Each test changes a source in a way that must not change its meaning, and asserts
//! the normalized identity — and, where the model lowers, the programmatic
//! `Model::identity` — is unchanged. The relation each test checks is named in its doc
//! comment with docs/19 §3's own words. One test checks the expected non-preservation:
//! a change of meaning must change the identity.

use continuum_cml_elab::{NormModel, elaborate_source, lower};

fn elab(src: &str) -> NormModel {
    elaborate_source(src).unwrap_or_else(|e| panic!("elaborates: {e}\n{src}"))
}

const DIE_HARD_ISH: &str = "\
model Jugs {
  state {
    big: Nat where big <= 5
    small: Nat where small <= 3
  }
  init Init { big == 0 && small == 0 }
  action FillBig { big' == 5 && small' == small }
  action SmallToBig {
    let next_big = min(big + small, 5)
    big' == next_big
    small' == small - (next_big - big)
  }
  invariant NotSolved { big != 4 }
  invariant Bounds { forall j in {3, 1, 2}: big + j >= j }
}
";

/// Relation: "stable reordering of declarations". Top-level declarations and state
/// fields in another order are the same model.
#[test]
fn stable_reordering_of_declarations_preserves_identity() {
    let reordered = "\
model Jugs {
  invariant Bounds { forall j in {3, 1, 2}: big + j >= j }
  action SmallToBig {
    let next_big = min(big + small, 5)
    small' == small - (next_big - big)
    big' == next_big
  }
  invariant NotSolved { big != 4 }
  state {
    small: Nat where small <= 3
    big: Nat where big <= 5
  }
  action FillBig { small' == small && big' == 5 }
  init Init { big == 0 && small == 0 }
}
";
    let a = elab(DIE_HARD_ISH);
    let b = elab(reordered);
    assert_eq!(a.identity(), b.identity());
    assert_eq!(a.dump(), b.dump());
}

/// Relation: "alpha-renaming". Renaming a `let` and a quantifier binder changes names
/// the normalized identity does not contain.
#[test]
fn alpha_renaming_preserves_identity() {
    let renamed = DIE_HARD_ISH.replace("next_big", "poured").replace(
        "forall j in {3, 1, 2}: big + j >= j",
        "forall k in {3, 1, 2}: big + k >= k",
    );
    let a = elab(DIE_HARD_ISH);
    let b = elab(&renamed);
    assert_eq!(a.identity(), b.identity());
    assert_ne!(
        a.dump(),
        b.dump(),
        "the named dump does show the binder name"
    );
}

/// Relation: "set/map insertion order". A set literal's elements in another order, or
/// repeated, are the same set.
#[test]
fn set_insertion_order_preserves_identity() {
    let permuted = DIE_HARD_ISH.replace("{3, 1, 2}", "{2, 3, 1, 3}");
    assert_eq!(elab(DIE_HARD_ISH).identity(), elab(&permuted).identity());
}

/// Relation: "equivalent guard normalization". `require a && b` and two `require`
/// clauses are one guard; `x' == x` and `unchanged x` are one frame; `=` and `==` are
/// one equality; the two header styles are one model.
#[test]
fn equivalent_guard_normalization_preserves_identity() {
    let one = "\
module M
state { x: Nat where x <= 3
        y: Nat where y <= 3 }
init { x == 0 && y == 0 }
action Step {
  require x < 3 && y < 3
  next x = x + 1
  unchanged y
}
";
    let two = "\
model M {
  state { x: Nat where x <= 3
          y: Nat where y <= 3 }
  init { x = 0
         y = 0 }
  action Step {
    require x < 3
    require y < 3
    x' == x + 1 && y' == y
  }
}
";
    let a = elab(one);
    let b = elab(two);
    assert_eq!(a.identity(), b.identity());
    assert_eq!(
        lower(&a).expect("lowers").identity(),
        lower(&b).expect("lowers").identity()
    );
}

/// Relation: "serialization round trip". The parser's canonical print of a source
/// elaborates to the same identity as the source.
#[test]
fn serialization_round_trip_preserves_identity() {
    for src in [
        DIE_HARD_ISH.to_owned(),
        std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../notes/plan/examples/replicated_register.ctm"
        ))
        .expect("the register is present"),
    ] {
        let file = continuum_cml_syntax::parse(&src).expect("parses");
        let printed = continuum_cml_syntax::print::print_file(&file);
        assert_eq!(elab(&src).identity(), elab(&printed).identity());
    }
}

/// Inlining a non-recursive `def` is the same model as writing its body.
#[test]
fn def_inlining_preserves_identity() {
    let with_def = DIE_HARD_ISH.replace(
        "invariant NotSolved { big != 4 }",
        "def target(): Nat = 4\n  invariant NotSolved { big != target() }",
    );
    assert_eq!(elab(DIE_HARD_ISH).identity(), elab(&with_def).identity());
}

/// Expected non-preservation: a change of meaning changes both identities.
#[test]
fn a_change_of_meaning_changes_identity() {
    // The quantified invariant does not lower; the rest of the model does.
    let base = DIE_HARD_ISH.replace(
        "  invariant Bounds { forall j in {3, 1, 2}: big + j >= j }\n",
        "",
    );
    let changed = base.replace("big != 4", "big != 3");
    let a = elab(&base);
    let b = elab(&changed);
    assert_ne!(a.identity(), b.identity());
    assert_ne!(
        lower(&a).expect("lowers").identity(),
        lower(&b).expect("lowers").identity()
    );
    // The model name is not meaning.
    let renamed = base.replace("model Jugs", "model Buckets");
    assert_eq!(a.identity(), elab(&renamed).identity());
}

/// Relation: "set/map insertion order", with elements that are bound variables of an
/// enclosing quantifier (security review cr-35xovl). Such elements are ordered by their
/// binding depth, not by source order or binder numbering, so permuting them — and
/// alpha-renaming the binders as well — leaves the identity unchanged, while a set that
/// really differs (`{a, a}`) still has another identity.
#[test]
fn set_insertion_order_of_bound_variables_preserves_identity() {
    let with = |set: &str, a: &str, b: &str| {
        elab(&format!(
            "module S\nstate {{ x: Nat where x <= 1 }}\ninit {{ x == 0 }}\naction A {{ unchanged x }}\n\
             invariant I {{ forall {a} in 0..1, {b} in 0..1: {set} subseteq 0..1 }}\n"
        ))
        .identity()
    };
    let ab = with("{a, b}", "a", "b");
    assert_eq!(ab, with("{b, a}", "a", "b"));
    assert_eq!(ab, with("{b, a, b}", "a", "b"));
    assert_eq!(ab, with("{d, c}", "c", "d"), "alpha-renamed and permuted");
    assert_ne!(ab, with("{a, a}", "a", "b"));
    // Map literal keys are ordered the same way.
    let map = |m: &str| {
        elab(&format!(
            "module S\nstate {{ x: Nat where x <= 1 }}\ninit {{ x == 0 }}\naction A {{ unchanged x }}\n\
             invariant I {{ forall a in 0..1, b in 0..1: {m}.get(a) == Some(true) }}\n"
        ))
        .identity()
    };
    assert_eq!(
        map("{a -> true, b -> false}"),
        map("{b -> false, a -> true}")
    );
}
