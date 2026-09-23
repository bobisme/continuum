//! Relational postconditions and relational actions (bn-2ouro, RFC 0003 "Relational
//! actions", docs/11 §4, PORTING_WAVES Wave 0 "primed variables and relational
//! actions").
//!
//! # What is pinned
//!
//! - **the frame rule** — a variable is updated, kept, or relational (primed by a
//!   postcondition); anything else is still an unspecified state change;
//! - **normalization** — a primed read of an updated or kept variable becomes its
//!   post-state value;
//! - **refusals** — a prime on anything but a state variable, a prime outside an
//!   action, and at lowering a relational variable that is not a bounded integer or
//!   whose candidate set is too large, each typed;
//! - **a differential** — the lowered model's successors against an independent
//!   successor computation, written here as Rust predicates over every pre-state and
//!   every post-state of the domain; mutants of the oracle are told apart;
//! - **the reference engine** — a nondeterministic relational action explored by
//!   `continuum-engine-reference` matches a state graph enumerated by hand.

use std::collections::BTreeSet;

use continuum_cml_elab::{
    ElabError, ElabErrorKind, LowerErrorKind, Unlowerable, Unsupported, elaborate_source, lower,
};
use continuum_engine_reference::bfs::{self, Bounds};
use continuum_model_core::Model;

fn fails(src: &str) -> ElabError {
    match elaborate_source(src) {
        Ok(m) => panic!("expected an error, elaborated:\n{}", m.dump()),
        Err(e) => e,
    }
}

/// Two variables, `x` in `0..3` and `y` in `0..3`, both starting at 0, and `actions`.
fn two_vars(actions: &str) -> String {
    format!(
        "module R\nstate {{ x: Nat where x <= 3\n y: Nat where y <= 3 }}\ninit {{ x == 0 && y == 0 }}\n{actions}\n"
    )
}

fn lowered(src: &str) -> Model {
    let m = elaborate_source(src).unwrap_or_else(|e| panic!("elaborates: {e}"));
    lower(&m).unwrap_or_else(|e| panic!("lowers: {e}"))
}

// ---------------------------------------------------------------------------
// the frame rule and normalization
// ---------------------------------------------------------------------------

#[test]
fn a_primed_variable_is_relational_and_an_unmentioned_one_is_still_refused() {
    let m = elaborate_source(&two_vars("action A { x' > x\n unchanged y }")).expect("elaborates");
    let dump = m.dump();
    assert!(dump.contains("(relational x)"), "{dump}");
    assert!(dump.contains("(unchanged y)"), "{dump}");
    assert!(dump.contains("(post (gt (primed x) (state x)))"), "{dump}");

    // `y` is neither updated, kept, nor primed: the frame rule has no default.
    let e = fails(&two_vars("action A { x' > x }"));
    assert_eq!(
        e.kind,
        ElabErrorKind::UnspecifiedStateChange {
            action: "A".to_owned(),
            variable: "y".to_owned()
        }
    );
    // Updated twice is still a conflict, relational or not.
    let e = fails(&two_vars("action A { next x = 1\n next x = 2\n y' > 0 }"));
    assert!(
        matches!(e.kind, ElabErrorKind::ConflictingUpdate { .. }),
        "{e}"
    );
}

/// In a postcondition, `y'` of an updated `y` is its update and of a kept `y` is `y`:
/// only relational variables stay primed.
#[test]
fn primed_reads_of_updated_and_kept_variables_are_their_post_state_values() {
    let m = elaborate_source(&two_vars("action A { next y = x + 1\n x' > y' }")).expect("updated");
    let dump = m.dump();
    assert!(
        dump.contains("(post (gt (primed x) (add (state x) 1)))"),
        "{dump}"
    );
    let m = elaborate_source(&two_vars("action A { unchanged y\n x' > y' }")).expect("kept");
    assert!(
        m.dump().contains("(post (gt (primed x) (state y)))"),
        "{}",
        m.dump()
    );
    // A `next` whose value reads the post-state (through a `let`) is a postcondition.
    let m = elaborate_source(&two_vars(
        "action A {\n let a = x'\n next y = a\n x' <= 1 }",
    ))
    .expect("a let of a primed read");
    let dump = m.dump();
    assert!(dump.contains("(relational y)"), "{dump}");
    assert!(dump.contains("(post (eq (primed y) (primed x)))"), "{dump}");
}

/// A deterministic update written relationally lowers to the same model as its
/// assignment: `x' == x + 1` is an update, and a postcondition over updated variables
/// only joins the guard.
#[test]
fn a_postcondition_without_relational_variables_joins_the_guard() {
    let model = lowered(&two_vars("action A { next x = 1\n unchanged y\n x' > y' }"));
    let names: Vec<String> = model
        .actions()
        .iter()
        .map(|a| a.name().to_string())
        .collect();
    assert_eq!(names, vec!["A".to_owned()]);
}

// ---------------------------------------------------------------------------
// refusals
// ---------------------------------------------------------------------------

#[test]
fn a_prime_on_anything_but_a_state_variable_is_unsupported() {
    for action in [
        "action A { (x + 1)' > x\n unchanged y }",
        "action A { x'' > x\n unchanged y }",
        "action B(n: Nat) { n' > x\n unchanged x, y }",
    ] {
        let e = fails(&two_vars(action));
        assert_eq!(
            e.kind,
            ElabErrorKind::Unsupported(Unsupported::PrimedExpression),
            "{action}"
        );
    }
    let e = fails(&two_vars(
        "action A { unchanged x, y }\ninvariant I { x' == x }",
    ));
    assert_eq!(e.kind, ElabErrorKind::PrimeOutsideAction);
}

fn lower_reason(src: &str) -> Unlowerable {
    let m = elaborate_source(src).unwrap_or_else(|e| panic!("elaborates: {e}"));
    match lower(&m).expect_err("refused").kind {
        LowerErrorKind::Unlowerable(u) => u,
        other => panic!("not a typed reason: {other:?}"),
    }
}

/// A relational variable must be enumerable: a bounded integer with a small enough
/// candidate product.
#[test]
fn a_relational_variable_that_cannot_be_enumerated_is_refused_typed() {
    // unbounded
    assert_eq!(
        lower_reason("module R\nstate { x: Nat }\ninit { x == 0 }\naction A { x' > x }\n"),
        Unlowerable::UnboundedDomain
    );
    // not an integer
    assert_eq!(
        lower_reason("module R\nstate { s: Set[Nat] }\ninit { s == {} }\naction A { 1 in s' }\n"),
        Unlowerable::NonIntegerState
    );
    // bounded, but the candidate product (1000 * 1000) is over the enumeration bound
    assert_eq!(
        lower_reason(
            "module R\nstate { x: Nat where x < 1000\n y: Nat where y < 1000 }\ninit { x == 0 && y == 0 }\naction A { x' + y' == x + y }\n"
        ),
        Unlowerable::SuccessorDomainTooLarge
    );
}

// ---------------------------------------------------------------------------
// the differential: lowered successors against an independent computation
// ---------------------------------------------------------------------------

type Pair = (i64, i64);

/// A transition relation over `(pre, post)`, written independently of the model.
type Relation = fn(Pair, Pair) -> bool;

/// The independent successor computation: every post-state of the domain, filtered by
/// the relation written as a Rust predicate over `(pre, post)`.
fn oracle(rel: Relation, pre: Pair) -> BTreeSet<Pair> {
    let mut out = BTreeSet::new();
    for x in 0..=3 {
        for y in 0..=3 {
            if rel(pre, (x, y)) {
                out.insert((x, y));
            }
        }
    }
    out
}

/// The lowered model's successors of `pre`, from its programmatic transition relation.
fn subject(model: &Model, pre: Pair) -> BTreeSet<Pair> {
    let state = model.state(&[pre.0, pre.1]).expect("in the domain");
    model
        .successors(&state)
        .expect("evaluates")
        .into_iter()
        .map(|s| {
            let v = s.target().as_slice();
            (v[0], v[1])
        })
        .collect()
}

/// Each case: one relational action, and the same relation written independently,
/// frame included.
fn cases() -> Vec<(&'static str, Relation)> {
    vec![
        // both relational: redistribute the sum
        ("action A { x' + y' == x + y }", |(x, y), (a, b)| {
            a + b == x + y
        }),
        // one relational, bounded by the other's post-state
        ("action A { x' >= x && y' == x' }", |(x, _), (a, b)| {
            a >= x && b == a
        }),
        // relational `x`, updated `y` read primed
        (
            "action A { next y = x\n x' != x && x' >= y' }",
            |(x, _), (a, b)| b == x && a != x && a >= b,
        ),
        // relational `x`, kept `y`, a guard, and a disjunctive postcondition
        (
            "action A { require y < 3\n unchanged y\n x' == y || x' == y + 1 }",
            |(_, y), (a, b)| y < 3 && b == y && (a == y || a == y + 1),
        ),
        // a postcondition that no candidate satisfies from some pre-states: disabled there
        ("action A { unchanged y\n x' > x + 2 }", |(x, y), (a, b)| {
            b == y && a > x + 2
        }),
    ]
}

#[test]
fn lowered_successors_match_an_independent_successor_computation() {
    for (action, rel) in cases() {
        let model = lowered(&two_vars(action));
        for x in 0..=3 {
            for y in 0..=3 {
                assert_eq!(
                    subject(&model, (x, y)),
                    oracle(rel, (x, y)),
                    "{action} from ({x}, {y})"
                );
            }
        }
    }
}

/// Anti-vacuity: an oracle off by one comparison, or one that forgets the frame, is
/// told apart by the same comparison.
#[test]
fn mutated_oracles_are_told_apart() {
    let model = lowered(&two_vars("action A { x' >= x && y' == x' }"));
    let mutants: [Relation; 3] = [
        |(x, _), (a, b)| a > x && b == a,
        |(x, _), (a, _)| a >= x,
        |(x, y), (a, b)| a >= x && b == y,
    ];
    for m in mutants {
        let differs =
            (0..=3).any(|x| (0..=3).any(|y| subject(&model, (x, y)) != oracle(m, (x, y))));
        assert!(differs, "a mutant oracle agreed with the lowered model");
    }
}

// ---------------------------------------------------------------------------
// the reference engine against a hand-enumerated state graph
// ---------------------------------------------------------------------------

const JUMP: &str = "module Jump\n\
state { x: Nat where x <= 3 }\n\
init { x == 0 }\n\
action Hop { x' > x && x' <= x + 2 }\n\
action Reset { require x == 3\n next x = 0 }\n";

/// `Hop` moves `x` up by one or two, nondeterministically; `Reset` returns from 3 to 0.
/// By hand: 0→1, 0→2, 1→2, 1→3, 2→3 (Hop), 3→0 (Reset). Every state is reachable;
/// depths 0, 1, 1, 2.
#[test]
fn a_nondeterministic_relational_action_explores_to_the_hand_enumerated_graph() {
    let model = lowered(JUMP);
    let names: Vec<String> = model
        .actions()
        .iter()
        .map(|a| a.name().to_string())
        .collect();
    assert_eq!(
        names,
        ["Hop[x=0]", "Hop[x=1]", "Hop[x=2]", "Hop[x=3]", "Reset"],
        "one action per candidate, in enumeration order"
    );

    let exploration = bfs::explore(&model, Bounds::CERTIFIABLE).expect("explores");
    let closed = exploration.closed().expect("closes");
    let states: Vec<i64> = closed.states().iter().map(|s| s.as_slice()[0]).collect();
    assert_eq!(states, vec![0, 1, 2, 3]);
    assert_eq!(closed.depths(), &[0, 1, 1, 2]);

    let hand: BTreeSet<(i64, &str, i64)> = [
        (0, "Hop", 1),
        (0, "Hop", 2),
        (1, "Hop", 2),
        (1, "Hop", 3),
        (2, "Hop", 3),
        (3, "Reset", 0),
    ]
    .into_iter()
    .collect();
    assert_eq!(closed.transitions(), hand.len() as u64);
    let mut edges: BTreeSet<(i64, String, i64)> = BTreeSet::new();
    for s in closed.states() {
        for step in model.successors(s).expect("evaluates") {
            let action = model.actions()[step.action()].name().to_string();
            let base = action.split('[').next().unwrap_or_default().to_owned();
            edges.insert((s.as_slice()[0], base, step.target().as_slice()[0]));
        }
    }
    let hand: BTreeSet<(i64, String, i64)> = hand
        .into_iter()
        .map(|(a, n, b)| (a, n.to_owned(), b))
        .collect();
    assert_eq!(edges, hand);
}

/// Anti-vacuity: the same model with `<=` weakened to `<` loses the 0→2 and 1→3 hops,
/// and the engine sees it.
#[test]
fn a_mutated_relational_action_explores_differently() {
    let mutated = JUMP.replace("x' <= x + 2", "x' < x + 2");
    assert_ne!(mutated, JUMP);
    let model = lowered(&mutated);
    let exploration = bfs::explore(&model, Bounds::CERTIFIABLE).expect("explores");
    let closed = exploration.closed().expect("closes");
    assert_eq!(closed.transitions(), 4);
    assert_eq!(closed.depths(), &[0, 1, 2, 3]);
}

// ---------------------------------------------------------------------------
// identity
// ---------------------------------------------------------------------------

/// Metamorphic relation "equivalent guard normalization": a postcondition written
/// under `require` or as a bare statement, its statements reordered around an update,
/// and a primed read of a kept variable written unprimed, all normalize to one action
/// and one identity. A different postcondition does not.
#[test]
fn equivalent_guard_normalization_of_a_relational_action_preserves_identity() {
    let id = |action: &str| {
        elaborate_source(&two_vars(action))
            .unwrap_or_else(|e| panic!("{action}: {e}"))
            .identity()
    };
    let base = id("action A { x' > x && x' >= y'\n unchanged y }");
    for same in [
        "action A { require x' > x\n require x' >= y'\n unchanged y }",
        "action A { unchanged y\n x' > x\n x' >= y' }",
        "action A { x' > x && x' >= y\n unchanged y }",
    ] {
        assert_eq!(base, id(same), "{same}");
    }
    assert_ne!(base, id("action A { x' > x && x' > y'\n unchanged y }"));
}

/// Candidate multiplication: two relational variables give the product of their
/// domains, one action each, in the enumeration order RFC 0003 fixes (names ascending,
/// the last variable fastest, values ascending).
#[test]
fn candidates_multiply_in_the_fixed_enumeration_order() {
    let src = "module M\nstate { x: Nat where x <= 2\n y: Nat where y <= 1 }\ninit { x == 0 && y == 0 }\naction A { x' + y' >= 0 }\n";
    let model = lowered(src);
    let names: Vec<String> = model
        .actions()
        .iter()
        .map(|a| a.name().to_string())
        .collect();
    assert_eq!(
        names,
        [
            "A[x=0,y=0]",
            "A[x=0,y=1]",
            "A[x=1,y=0]",
            "A[x=1,y=1]",
            "A[x=2,y=0]",
            "A[x=2,y=1]"
        ]
    );
}
