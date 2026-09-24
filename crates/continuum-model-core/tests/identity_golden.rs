//! The `continuum-model/1` identity encoding, pinned byte for byte (bn-3a9sr,
//! cr-1sxoia round 4).
//!
//! `Model::identity` is ADR-0013 content identity: a change to its bytes changes which
//! models are "the same", and the programmatic–CML differential depends on it. This
//! golden was written by the encoder as it stood before its outcome buffering was
//! restructured, over a model that exercises every encoding branch: several
//! variables, a nondeterministic action whose outcomes are declared out of order and
//! with a duplicate, every integer and Boolean expression form, several initial
//! states, and predicates. Regenerate only for a deliberate encoding change (which is
//! a `continuum-model/2`), with `MODEL_IDENTITY_BLESS=1`.
//!
//! bn-1ln12 added the optional fairness section. `every_form` declares no fairness, so
//! its golden did not move; `every_form_fair` pins the section itself, with both
//! strengths, a scope of two actions, and assumptions declared out of order and twice.

use continuum_model_core::{
    ActionDecl, ArithOp, BoolExpr, CmpOp, IntExpr, Model, ModelBuilder, Strength,
};

fn every_form() -> Model {
    every_form_builder().build().expect("a valid model")
}

fn every_form_builder() -> ModelBuilder {
    let x = || IntExpr::var("x");
    let y = || IntExpr::var("y");
    let int = IntExpr::Max(
        Box::new(IntExpr::Min(
            Box::new(IntExpr::Arith(
                ArithOp::Mul,
                Box::new(x()),
                Box::new(IntExpr::constant(2)),
            )),
            Box::new(IntExpr::Arith(
                ArithOp::Sub,
                Box::new(y()),
                Box::new(IntExpr::constant(-1)),
            )),
        )),
        Box::new(IntExpr::Arith(ArithOp::Add, Box::new(x()), Box::new(y()))),
    );
    let guard = BoolExpr::implies(
        BoolExpr::or(
            BoolExpr::negate(BoolExpr::compare(CmpOp::Eq, x(), y())),
            BoolExpr::constant(false),
        ),
        BoolExpr::and(
            BoolExpr::in_range(int.clone(), -3, 9),
            BoolExpr::compare(CmpOp::Ge, int, IntExpr::constant(0)),
        ),
    );
    ModelBuilder::new()
        .variable("x", 0, 3)
        .variable("y", -2, 2)
        .initial_state(&[("x", 0), ("y", 0)])
        .initial_state(&[("x", 1), ("y", -1)])
        .action(ActionDecl::enumerated(
            "Pick",
            guard,
            vec![
                vec![("x", IntExpr::constant(3)), ("y", y())],
                vec![("x", IntExpr::constant(1))],
                vec![("x", IntExpr::constant(3)), ("y", y())],
                vec![("y", IntExpr::constant(-2))],
            ],
        ))
        .action(ActionDecl::deterministic(
            "Step",
            BoolExpr::compare(CmpOp::Lt, x(), IntExpr::constant(3)),
            vec![("x", IntExpr::plus(x(), IntExpr::constant(1)))],
        ))
        .predicate(
            "Bounded",
            BoolExpr::compare(CmpOp::Le, x(), IntExpr::constant(3)),
        )
        .predicate(
            "Even",
            BoolExpr::compare(CmpOp::Ne, y(), IntExpr::constant(1)),
        )
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::new();
    for (i, b) in bytes.iter().enumerate() {
        out.push_str(&format!("{b:02x}"));
        if i % 32 == 31 {
            out.push('\n');
        }
    }
    out.push('\n');
    out
}

#[test]
fn the_model_identity_encoding_is_pinned_byte_for_byte() {
    let model = every_form();
    let got = hex(model.identity().as_bytes());
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/every_form.identity.hex");
    if std::env::var_os("MODEL_IDENTITY_BLESS").is_some() {
        std::fs::write(&path, &got).expect("write golden");
    }
    let want = std::fs::read_to_string(&path).expect("golden present");
    assert_eq!(got, want, "continuum-model/1 bytes changed");
    // The allocation-free bound covers the encoding (the duplicate outcome makes it
    // strictly larger here).
    assert!(model.identity_len_bound() > model.identity().as_bytes().len());
    assert!(model.identity_alloc_bound() > model.identity_len_bound());
}

/// The fairness section, pinned: `every_form` plus strong fairness of `Step`, weak
/// fairness over `{Pick, Step}` (named out of order and with a repeat), and the weak
/// assumption declared a second time. The bytes before the section are exactly
/// `every_form`'s.
#[test]
fn the_fairness_section_is_pinned_byte_for_byte() {
    let model = every_form_builder()
        .fairness(Strength::Strong, ["Step"])
        .fairness(Strength::Weak, ["Step", "Pick", "Step"])
        .fairness(Strength::Weak, ["Pick", "Step"])
        .build()
        .expect("a valid model");
    let plain = every_form().identity();
    let fair = model.identity();
    assert!(fair.as_bytes().starts_with(plain.as_bytes()));
    let got = hex(fair.as_bytes());
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/every_form_fair.identity.hex");
    if std::env::var_os("MODEL_IDENTITY_BLESS").is_some() {
        std::fs::write(&path, &got).expect("write golden");
    }
    let want = std::fs::read_to_string(&path).expect("golden present");
    assert_eq!(got, want, "continuum-model/1 fairness bytes changed");
    // The section by hand: count 2; weak, 2 members, "Pick", "Step"; strong, 1, "Step".
    let mut section: Vec<u8> = Vec::new();
    let count = |out: &mut Vec<u8>, n: u64| out.extend_from_slice(&n.to_be_bytes());
    count(&mut section, 2);
    section.push(0x20);
    count(&mut section, 2);
    for name in ["Pick", "Step"] {
        count(&mut section, name.len() as u64);
        section.extend_from_slice(name.as_bytes());
    }
    section.push(0x21);
    count(&mut section, 1);
    count(&mut section, 4);
    section.extend_from_slice(b"Step");
    assert_eq!(&fair.as_bytes()[plain.as_bytes().len()..], &section[..]);
    assert!(model.identity_len_bound() >= fair.as_bytes().len());
}
