//! Typed, source-located elaboration and lowering errors (PR 15a, bn-ybq).
//!
//! Every error kind a well-formed-looking source can reach is pinned here with its
//! stable code and its span, and every unsupported kind is checked to be reported as
//! unsupported, never as ill-formed.

use continuum_cml_elab::{
    ElabError, ElabErrorKind, LowerErrorKind, Unlowerable, Unsupported, elaborate_source, lower,
};

fn fails(src: &str) -> ElabError {
    match elaborate_source(src) {
        Ok(m) => panic!("expected an error, elaborated:\n{}", m.dump()),
        Err(e) => e,
    }
}

/// Wrap declarations into a model with one bounded variable `x`.
fn model(decls: &str) -> String {
    format!("module T\nstate {{ x: Nat where x <= 3 }}\ninit {{ x == 0 }}\n{decls}\n")
}

fn at(e: &ElabError) -> (u32, u32) {
    (e.span.line, e.span.col)
}

#[test]
fn unknown_name_is_located() {
    let e = fails(&model("invariant I { y == 0 }"));
    assert_eq!(e.kind, ElabErrorKind::UnknownName("y".to_owned()));
    assert_eq!(e.code(), "cml.elab.unknown_name");
    assert_eq!(at(&e), (4, 15));
    assert!(!e.is_unsupported());
}

#[test]
fn type_mismatch_is_located() {
    let e = fails(&model("invariant I { x == true }"));
    assert!(matches!(e.kind, ElabErrorKind::TypeMismatch { .. }), "{e}");
    assert_eq!(e.code(), "cml.elab.type_mismatch");
    assert_eq!(at(&e), (4, 20));
}

#[test]
fn an_unspecified_state_change_is_refused() {
    let src = "module T\nstate { x: Nat where x <= 3\n y: Nat where y <= 3 }\ninit { x == 0 && y == 0 }\naction A { next x = 1 }\n";
    let e = fails(src);
    assert_eq!(
        e.kind,
        ElabErrorKind::UnspecifiedStateChange {
            action: "A".to_owned(),
            variable: "y".to_owned()
        }
    );
    assert_eq!(at(&e), (5, 1));
}

#[test]
fn a_conflicting_update_is_refused() {
    let e = fails(&model("action A { next x = 1\n unchanged x }"));
    assert_eq!(
        e.kind,
        ElabErrorKind::ConflictingUpdate {
            action: "A".to_owned(),
            variable: "x".to_owned()
        }
    );
    assert_eq!(at(&e), (5, 12));
}

#[test]
fn duplicate_and_shadowing_names_are_refused() {
    let e = fails(&model("action x { unchanged x }"));
    assert_eq!(e.kind, ElabErrorKind::DuplicateName("x".to_owned()));
    let e = fails(&model("invariant I { forall x in 0..1: true }"));
    assert_eq!(e.kind, ElabErrorKind::DuplicateName("x".to_owned()));
    let e = fails(&model("enum Some { A }"));
    assert_eq!(e.kind, ElabErrorKind::DuplicateName("Some".to_owned()));
}

#[test]
fn primes_and_temporal_forms_are_confined() {
    let e = fails(&model("invariant I { x' == 0 }"));
    assert_eq!(e.kind, ElabErrorKind::PrimeOutsideAction);
    let e = fails(&model("invariant I { always(x == 0) }"));
    assert_eq!(e.kind, ElabErrorKind::TemporalOutsideBehavior);
    let e = fails(&model("action A { require step(A)\n unchanged x }"));
    assert_eq!(e.kind, ElabErrorKind::TemporalOutsideBehavior);
}

/// A domainless binder is admitted when its uses determine its type, and refused
/// — never defaulted — when they do not.
#[test]
fn a_domainless_binder_needs_its_type_determined() {
    let e = fails(&model("invariant I { forall y: true }"));
    assert_eq!(
        e.kind,
        ElabErrorKind::CannotInferType("binder `y`".to_owned())
    );
    assert_eq!(e.code(), "cml.elab.cannot_infer_type");

    let ok = elaborate_source(&model("invariant I { forall y: y + x >= x }"))
        .expect("`y` is an integer by its use");
    assert!(ok.dump().contains("(forall ((y Int))"), "{}", ok.dump());
}

#[test]
fn empty_braces_need_a_set_or_map_type() {
    let e = fails(&model("invariant I { {} == {} }"));
    assert_eq!(e.kind, ElabErrorKind::CannotInferType("`{}`".to_owned()));
}

#[test]
fn unsupported_semantics_are_typed_unsupported() {
    let e = fails(&model(
        "def f(n: Nat): Nat = f(n)\ninvariant I { f(x) == 0 }",
    ));
    assert_eq!(
        e.kind,
        ElabErrorKind::Unsupported(Unsupported::RecursiveDef)
    );
    assert_eq!(e.code(), "cml.elab.unsupported.recursive_def");
    assert!(e.is_unsupported());

    let e = fails(&model("action A { x' > x }"));
    assert_eq!(
        e.kind,
        ElabErrorKind::Unsupported(Unsupported::RelationalPostcondition)
    );
    assert!(e.is_unsupported());
    assert_eq!(at(&e), (4, 12));
}

/// Source is untrusted (INV-016): `let` inlining that would double the tree forty
/// times is refused with a typed resource error instead of exhausting memory.
#[test]
fn inlining_blowup_is_bounded() {
    let mut body = String::from("  let a0 = x\n");
    for i in 1..40 {
        body.push_str(&format!("  let a{i} = a{} + a{}\n", i - 1, i - 1));
    }
    body.push_str("  a39 >= 0\n");
    let e = fails(&model(&format!("invariant I {{\n{body}}}")));
    assert_eq!(e.kind, ElabErrorKind::TooLarge);
    assert_eq!(e.code(), "cml.limit.elaboration_too_large");
}

#[test]
fn a_parse_error_is_carried_with_its_code() {
    let e = fails("module T\nstate { x: Nat }\ninit { x == }\n");
    assert!(matches!(e.kind, ElabErrorKind::Parse(_)));
    assert_eq!(e.code(), "cml.parse.unexpected_token");
}

// ---------------------------------------------------------------------------
// lowering
// ---------------------------------------------------------------------------

fn lower_err(src: &str) -> (Unlowerable, u32) {
    let m = elaborate_source(src).unwrap_or_else(|e| panic!("elaborates: {e}"));
    match lower(&m) {
        Ok(_) => panic!("expected a lowering error"),
        Err(e) => match e.kind {
            LowerErrorKind::Unlowerable(u) => (u, e.span.line),
            other => panic!("expected Unlowerable, got {other:?}"),
        },
    }
}

#[test]
fn lowering_refuses_with_typed_reasons() {
    assert_eq!(
        lower_err("module T\nstate { x: Nat }\ninit { x == 0 }\naction A { unchanged x }\n"),
        (Unlowerable::UnboundedDomain, 2)
    );
    assert_eq!(
        lower_err(&model("action A(n: Nat) { next x = n }")),
        (Unlowerable::ParameterizedAction, 4)
    );
    assert_eq!(
        lower_err(&model("action A { next x = x / 2 }")),
        (Unlowerable::DivisionOrModulo, 4)
    );
    assert_eq!(
        lower_err(&model(
            "action A { unchanged x }\ninvariant I { forall y in 0..1: y <= x }"
        )),
        (Unlowerable::Quantifier, 5)
    );
    assert_eq!(
        lower_err(&model("action A { unchanged x }\nfairness weak A")),
        (Unlowerable::Fairness, 5)
    );
    assert_eq!(
        lower_err(&model(
            "action A { unchanged x }\nbehavior B = always(x == 0)"
        )),
        (Unlowerable::NonStandardBehavior, 5)
    );
    assert_eq!(
        lower_err(
            "module T\nstate { x: Nat where x * x <= 3 }\ninit { x == 0 }\naction A { unchanged x }\n"
        ),
        (Unlowerable::NonIntervalRefinement, 2)
    );
}

#[test]
fn an_unsatisfiable_init_is_the_builders_refusal() {
    let m = elaborate_source(
        "module T\nstate { x: Nat where x <= 3 }\ninit { x == 7 }\naction A { unchanged x }\n",
    )
    .expect("elaborates");
    let e = lower(&m).expect_err("no initial state");
    assert!(matches!(e.kind, LowerErrorKind::Model(_)), "{e}");
    assert_eq!(e.code(), "cml.lower.model_refused");
}

#[test]
fn a_lowered_update_outside_the_refinement_is_detected_not_clamped() {
    let m = elaborate_source(&model("action A { next x = x + 1 }")).expect("elaborates");
    let lowered = lower(&m).expect("lowers");
    // From x == 3, `A` produces 4, outside `x <= 3`: a typed evaluation error.
    let three = lowered.state(&[3]).expect("3 is in the domain");
    assert!(lowered.successors(&three).is_err());
}
