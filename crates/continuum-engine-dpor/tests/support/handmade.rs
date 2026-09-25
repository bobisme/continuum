//! Adversarial models for C005: each is built to break one naive shortcut in a
//! reducer (RFC 0004: "adversarial programs constructed to break naïve conflict
//! relations"). Shared by `tests/c005_differential.rs` and `src/mutation.rs`.
//!
//! | Id | Breaks |
//! |---|---|
//! | `guard-read` | a conflict relation that ignores a guard or right-hand side reading a variable another label writes |
//! | `enabling` | a stubborn closure that does not add a necessary enabling set for a disabled member |
//! | `ignoring` | a reduction with no cycle proviso: an invisible loop postpones a visible step forever |
//! | `wakeup` | a sleep set that does not wake a label when a dependent label fires |
//! | `fault` | a reduction that prunes the only interleaving on which an update leaves its domain |
//! | `philosophers` | deadlock preservation: three dining philosophers, one circular wait |
//! | `independent` | nothing: three fully independent counters, where the reduction is largest |
//! | `hidden-undefined` | a reduction that leaves definedness predicates' reads invisible: the undefined read is reached only on the interleaving it would postpone |
//! | `undefined-invariant` | an invariant guarded by a nested definedness chain (`I#defined#defined`) |

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    dead_code
)]

use continuum_model_core::expr::{BoolExpr, CmpOp, IntExpr};
use continuum_model_core::model::{ActionDecl, Model, ModelBuilder};

fn var(name: &str) -> IntExpr {
    IntExpr::var(name)
}

fn int(value: i64) -> IntExpr {
    IntExpr::constant(value)
}

fn eq(name: &str, value: i64) -> BoolExpr {
    BoolExpr::compare(CmpOp::Eq, var(name), int(value))
}

/// `a` writes `x`; `b` is guarded by `x == 0` and sets the visible `y`. Only the
/// interleaving `b` first reaches `y == 1`.
fn guard_read() -> Model {
    ModelBuilder::new()
        .variable("x", 0, 1)
        .variable("y", 0, 1)
        .variable("pa", 0, 1)
        .variable("pb", 0, 1)
        .action(ActionDecl::deterministic(
            "A",
            eq("pa", 0),
            vec![("pa", int(1)), ("x", int(1))],
        ))
        .action(ActionDecl::deterministic(
            "B",
            BoolExpr::and(eq("pb", 0), eq("x", 0)),
            vec![("pb", int(1)), ("y", int(1))],
        ))
        .predicate("YStaysZero", eq("y", 0))
        .initial_state(&[("x", 0), ("y", 0), ("pa", 0), ("pb", 0)])
        .build()
        .unwrap()
}

/// `a` writes `x`; `b` copies `x` into the visible `y` but only once `c` has raised
/// `flag`. `b` is disabled at the start, so a closure from `a` must add `b`'s enabler
/// `c`, or the run that fires `c`, then `b`, then `a` (reaching `y == 0`) is lost.
fn enabling() -> Model {
    ModelBuilder::new()
        .variable("x", 0, 1)
        .variable("y", 0, 2)
        .variable("flag", 0, 1)
        .variable("pa", 0, 1)
        .variable("pb", 0, 1)
        .action(ActionDecl::deterministic(
            "A",
            eq("pa", 0),
            vec![("pa", int(1)), ("x", int(1))],
        ))
        .action(ActionDecl::deterministic(
            "B",
            BoolExpr::and(eq("pb", 0), eq("flag", 1)),
            vec![("pb", int(1)), ("y", var("x"))],
        ))
        .action(ActionDecl::deterministic(
            "C",
            eq("flag", 0),
            vec![("flag", int(1))],
        ))
        .predicate("YNeverZero", BoolExpr::compare(CmpOp::Ne, var("y"), int(0)))
        .initial_state(&[("x", 0), ("y", 2), ("flag", 0), ("pa", 0), ("pb", 0)])
        .build()
        .unwrap()
}

/// An invisible two-step loop in one process and one visible step in another. With
/// no proviso, a reducer can circle the loop forever and never fire the visible step.
fn ignoring() -> Model {
    ModelBuilder::new()
        .variable("loop", 0, 1)
        .variable("y", 0, 1)
        .action(ActionDecl::deterministic(
            "L0",
            eq("loop", 0),
            vec![("loop", int(1))],
        ))
        .action(ActionDecl::deterministic(
            "L1",
            eq("loop", 1),
            vec![("loop", int(0))],
        ))
        .action(ActionDecl::deterministic(
            "V",
            eq("y", 0),
            vec![("y", int(1))],
        ))
        .predicate("YStaysZero", eq("y", 0))
        .initial_state(&[("loop", 0), ("y", 0)])
        .build()
        .unwrap()
}

/// `a` copies `y` into the visible `w`; `b` sets `y`. They are dependent, so after
/// exploring `a` first, `a` must wake again once `b` fires: only `b` then `a` reaches
/// `w == 1`.
fn wakeup() -> Model {
    ModelBuilder::new()
        .variable("w", 0, 1)
        .variable("y", 0, 1)
        .variable("pa", 0, 1)
        .variable("pb", 0, 1)
        .action(ActionDecl::deterministic(
            "A",
            eq("pa", 0),
            vec![("pa", int(1)), ("w", var("y"))],
        ))
        .action(ActionDecl::deterministic(
            "B",
            eq("pb", 0),
            vec![("pb", int(1)), ("y", int(1))],
        ))
        .predicate("WStaysZero", eq("w", 0))
        .initial_state(&[("w", 0), ("y", 0), ("pa", 0), ("pb", 0)])
        .build()
        .unwrap()
}

/// `b` adds 2 to `x` into `z` (domain 0..=2): out of domain only after `a` raised
/// `x`. The fault is reachable on one interleaving; the invariant never fails.
fn fault() -> Model {
    ModelBuilder::new()
        .variable("x", 0, 1)
        .variable("z", 0, 2)
        .variable("pa", 0, 1)
        .variable("pb", 0, 1)
        .variable("q", 0, 1)
        .action(ActionDecl::deterministic(
            "A",
            eq("pa", 0),
            vec![("pa", int(1)), ("x", int(1))],
        ))
        .action(ActionDecl::deterministic(
            "B",
            eq("pb", 0),
            vec![("pb", int(1)), ("z", IntExpr::plus(var("x"), int(2)))],
        ))
        .action(ActionDecl::deterministic(
            "Q",
            eq("q", 0),
            vec![("q", int(1))],
        ))
        .predicate("QBounded", BoolExpr::compare(CmpOp::Le, var("q"), int(1)))
        .initial_state(&[("x", 0), ("z", 0), ("pa", 0), ("pb", 0), ("q", 0)])
        .build()
        .unwrap()
}

/// Three dining philosophers: each takes its left fork, then its right, then puts
/// both down. The circular wait (every philosopher holding its left fork) is the one
/// deadlock; a reduction must preserve it.
fn philosophers() -> Model {
    let mut builder = ModelBuilder::new();
    let n = 3;
    for i in 0..n {
        builder = builder
            .variable(&format!("ph{i}"), 0, 2)
            .variable(&format!("f{i}"), 0, 1);
    }
    for i in 0..n {
        let me = format!("ph{i}");
        let left = format!("f{i}");
        let right = format!("f{}", (i + 1) % n);
        builder = builder
            .action(ActionDecl::deterministic(
                &format!("TakeLeft{i}"),
                BoolExpr::and(eq(&me, 0), eq(&left, 0)),
                vec![(me.as_str(), int(1)), (left.as_str(), int(1))],
            ))
            .action(ActionDecl::deterministic(
                &format!("TakeRight{i}"),
                BoolExpr::and(eq(&me, 1), eq(&right, 0)),
                vec![(me.as_str(), int(2)), (right.as_str(), int(1))],
            ))
            .action(ActionDecl::deterministic(
                &format!("PutDown{i}"),
                eq(&me, 2),
                vec![
                    (me.as_str(), int(0)),
                    (left.as_str(), int(0)),
                    (right.as_str(), int(0)),
                ],
            ));
    }
    builder = builder.predicate(
        "NotAllEating",
        BoolExpr::negate(BoolExpr::and(
            eq("ph0", 2),
            BoolExpr::and(eq("ph1", 2), eq("ph2", 2)),
        )),
    );
    let initial: Vec<(String, i64)> = (0..n)
        .flat_map(|i| [(format!("ph{i}"), 0), (format!("f{i}"), 0)])
        .collect();
    let bindings: Vec<(&str, i64)> = initial.iter().map(|(n, v)| (n.as_str(), *v)).collect();
    builder.initial_state(&bindings).build().unwrap()
}

/// Three independent counters `c0..c2` in `0..=3` and one visible flag raised by an
/// independent step. The unreduced space is `4^3 · 2`; persistent sets explore one
/// interleaving of the invisible counters.
fn independent() -> Model {
    let mut builder = ModelBuilder::new();
    for i in 0..3 {
        let name = format!("c{i}");
        builder = builder
            .variable(&name, 0, 3)
            .action(ActionDecl::deterministic(
                &format!("Inc{i}"),
                BoolExpr::compare(CmpOp::Lt, var(&name), int(3)),
                vec![(name.as_str(), IntExpr::plus(var(&name), int(1)))],
            ));
    }
    builder
        .variable("done", 0, 1)
        .action(ActionDecl::deterministic(
            "Finish",
            eq("done", 0),
            vec![("done", int(1))],
        ))
        .predicate("DoneIsBinary", BoolExpr::in_range(var("done"), 0, 1))
        .initial_state(&[("c0", 0), ("c1", 0), ("c2", 0), ("done", 0)])
        .build()
        .unwrap()
}

/// `A` and `B` set independent flags; `B#defined` (a definedness predicate of the
/// action `B`, RFC 0003) is false once `B` has fired while `A` has not. The invariant
/// reads only `y`, so without the definedness reads in the visible set a persistent
/// set may explore `A` first everywhere and never meet the undefined read.
fn hidden_undefined() -> Model {
    ModelBuilder::new()
        .variable("a", 0, 1)
        .variable("b", 0, 1)
        .variable("y", 0, 1)
        .action(ActionDecl::deterministic(
            "A",
            eq("a", 0),
            vec![("a", int(1))],
        ))
        .action(ActionDecl::deterministic(
            "B",
            eq("b", 0),
            vec![("b", int(1))],
        ))
        .action(ActionDecl::deterministic(
            "V",
            eq("y", 0),
            vec![("y", int(1))],
        ))
        // Named to sort first, so the corpus's narrow scope (the first predicate alone)
        // asks for it and not for `B#defined`.
        .predicate("AllBinary", BoolExpr::in_range(var("y"), 0, 1))
        .predicate(
            "B#defined",
            BoolExpr::negate(BoolExpr::and(eq("b", 1), eq("a", 0))),
        )
        .initial_state(&[("a", 0), ("b", 0), ("y", 0)])
        .build()
        .unwrap()
}

/// An invariant `I` over `x`, whose reads are guarded by `I#defined` (`x` stays below
/// 2) and that guard's own reads by `I#defined#defined` (`z` stays 0): a nested
/// chain, read deepest first, reached only after independent steps.
fn undefined_invariant() -> Model {
    ModelBuilder::new()
        .variable("x", 0, 2)
        .variable("z", 0, 1)
        .variable("w", 0, 1)
        .action(ActionDecl::deterministic(
            "Inc",
            BoolExpr::compare(CmpOp::Lt, var("x"), int(2)),
            vec![("x", IntExpr::plus(var("x"), int(1)))],
        ))
        .action(ActionDecl::deterministic(
            "Z",
            eq("z", 0),
            vec![("z", int(1))],
        ))
        .action(ActionDecl::deterministic(
            "W",
            eq("w", 0),
            vec![("w", int(1))],
        ))
        .predicate("I", BoolExpr::compare(CmpOp::Le, var("x"), int(2)))
        .predicate("I#defined", BoolExpr::compare(CmpOp::Lt, var("x"), int(2)))
        .predicate("I#defined#defined", eq("z", 0))
        .initial_state(&[("x", 0), ("z", 0), ("w", 0)])
        .build()
        .unwrap()
}

/// Every adversarial model, with its stable id.
pub fn all() -> Vec<(&'static str, Model)> {
    vec![
        ("guard-read", guard_read()),
        ("enabling", enabling()),
        ("ignoring", ignoring()),
        ("wakeup", wakeup()),
        ("fault", fault()),
        ("philosophers", philosophers()),
        ("independent", independent()),
        ("hidden-undefined", hidden_undefined()),
        ("undefined-invariant", undefined_invariant()),
    ]
}
