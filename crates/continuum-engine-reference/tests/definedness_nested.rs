//! Nested definedness guards (bn-24a5c, Codex cr-pt5h3a).
//!
//! The programmatic model API accepts a guard of a guard: `I#defined#defined` says where
//! the reads of `I#defined` are defined. A scanner that reads only `I#defined` reports
//! `Holds` for `I = true`, `I#defined = true`, `I#defined#defined = false`, and an
//! emitter certifies it. Every predicate of a chain is an obligation of its base, so
//! each path here must report the undefined read. These tests use only APIs that
//! predate the fix, so they compile, and fail, on the revision before it (0a5c152a).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use continuum_engine_reference::bfs::{self, Bounds};
use continuum_engine_reference::certificate::{
    self, ClaimEnvelope, ClosedSet, EmissionError, PRODUCER,
};
use continuum_engine_reference::checking::{
    self, CheckOutcome, DeadlockOutcome, DeadlockPolicy, Obligations, Verdict,
};
use continuum_engine_reference::liveness::{Goal, LivenessOutcome, Stuttering, check_liveness};
use continuum_engine_reference::witness::{self, NoWitness, Target};
use continuum_engine_reference::{
    ActionDecl, BoolExpr, CmpOp, IntExpr, Model, ModelBuilder, Unresolved,
};

fn x_is(value: i64) -> BoolExpr {
    BoolExpr::compare(CmpOp::Eq, IntExpr::var("x"), IntExpr::constant(value))
}

/// `x` steps 0 → 1 and stops. Extra predicates are added by `with`.
fn model(with: &[(&str, BoolExpr)]) -> Model {
    let mut builder = ModelBuilder::new()
        .variable("x", 0, 1)
        .action(ActionDecl::deterministic(
            "Step",
            x_is(0),
            vec![("x", IntExpr::constant(1))],
        ))
        .initial_state(&[("x", 0)]);
    for (name, body) in with {
        builder = builder.predicate(name, body.clone());
    }
    builder.build().expect("builds")
}

fn envelope() -> ClaimEnvelope<'static> {
    ClaimEnvelope {
        model_digest: "blake3:nested-model",
        semantic_epoch: "continuum-semantics-1",
        property_digest: "blake3:nested-property",
        scope_digest: "blake3:nested-scope",
        assumptions_digest: "blake3:empty-assumptions",
        producer: PRODUCER,
        domain_pack_digests: &[],
    }
}

/// Codex's case: `I = true`, `I#defined = true`, `I#defined#defined` false at `x = 1`.
/// Checking, liveness, witnesses and the invariant emitter all see the undefined read.
#[test]
fn a_false_guard_of_a_guard_is_an_undefined_read_in_the_invariant() {
    let model = model(&[
        ("I", BoolExpr::constant(true)),
        ("I#defined", BoolExpr::constant(true)),
        ("I#defined#defined", x_is(0)),
    ]);
    let exploration = bfs::explore(&model, Bounds::CERTIFIABLE).expect("explores");
    let i = model.predicate_index("I").unwrap();
    let guard = model.predicate_index("I#defined").unwrap();

    let report = checking::check(
        &model,
        &exploration,
        &Obligations::new(DeadlockPolicy::Allowed)
            .invariant(i)
            .invariant(guard),
    )
    .expect("checks");
    for index in [i, guard] {
        let CheckOutcome::Undefined(u) = report.invariant(index).unwrap().outcome() else {
            panic!("{report}");
        };
        assert_eq!(u.subject(), "I");
        assert_eq!(u.depth(), 1);
        assert_eq!(u.guard_name().as_str(), "I#defined#defined");
    }
    assert_eq!(report.verdict(), Verdict::Inconclusive);

    let live =
        check_liveness(&model, &exploration, Goal::Eventually(i), Stuttering::Never).expect("runs");
    assert!(matches!(live, LivenessOutcome::Undefined(_)), "{live:?}");

    // `I` holds at x = 0 and has no value at x = 1: the only `Holds` endpoint is the
    // initial state, and no state is a `Fails` endpoint.
    let holds = witness::shortest(&model, &exploration, &Target::Holds(i)).expect("found");
    assert_eq!(holds.len(), 0);
    let deepest = witness::shortest(&model, &exploration, &Target::Fails(guard));
    assert!(
        matches!(deepest, Err(NoWitness::Unreached { .. })),
        "{deepest:?}"
    );

    let closed = ClosedSet::of(&exploration).expect("closed");
    for name in ["I", "I#defined"] {
        let emitted = certificate::emit_invariant_closure(&model, closed, &envelope(), name);
        let Err(EmissionError::Undefined { predicate, .. }) = emitted else {
            panic!("{name}: {emitted:?}");
        };
        assert_eq!(predicate, "I#defined#defined");
    }
}

/// An action chain: `Step#defined` holds, `Step#defined#defined` is false at x = 1. The
/// undefined action read dominates every obligation and the deadlock question, and
/// the finite-closure emitter refuses.
#[test]
fn a_false_guard_of_an_action_guard_is_an_undefined_action_read() {
    let model = model(&[
        ("Ok", BoolExpr::constant(true)),
        ("Step#defined", BoolExpr::constant(true)),
        ("Step#defined#defined", x_is(0)),
    ]);
    let exploration = bfs::explore(&model, Bounds::CERTIFIABLE).expect("explores");
    let ok = model.predicate_index("Ok").unwrap();
    let report = checking::check(
        &model,
        &exploration,
        &Obligations::new(DeadlockPolicy::Defect).invariant(ok),
    )
    .expect("checks");
    let CheckOutcome::Undefined(u) = report.invariant(ok).unwrap().outcome() else {
        panic!("{report}");
    };
    assert_eq!(u.subject(), "Step");
    assert!(matches!(report.deadlock(), DeadlockOutcome::Undefined(_)));

    let closed = ClosedSet::of(&exploration).expect("closed");
    let emitted = certificate::emit_finite_closure(&model, closed, &envelope());
    let Err(EmissionError::Undefined { predicate, .. }) = emitted else {
        panic!("{emitted:?}");
    };
    assert_eq!(predicate, "Step#defined#defined");
}

/// A chain with a gap (`I#defined#defined` with no `I#defined`) is not a well-formed
/// guard of `I`, so it is read as an action's (fail closed): it invalidates every
/// obligation, the deadlock question, and the finite closure.
#[test]
fn a_chain_with_a_gap_fails_closed_as_an_action_read() {
    let model = model(&[
        ("I", BoolExpr::constant(true)),
        ("J", BoolExpr::constant(true)),
        ("I#defined#defined", x_is(0)),
    ]);
    let exploration = bfs::explore(&model, Bounds::CERTIFIABLE).expect("explores");
    let i = model.predicate_index("I").unwrap();
    let j = model.predicate_index("J").unwrap();
    let report = checking::check(
        &model,
        &exploration,
        &Obligations::new(DeadlockPolicy::Allowed)
            .invariant(i)
            .invariant(j),
    )
    .expect("checks");
    for index in [i, j] {
        let CheckOutcome::Undefined(u) = report.invariant(index).unwrap().outcome() else {
            panic!("{report}");
        };
        assert_eq!(u.subject(), "I");
    }
    assert!(matches!(report.deadlock(), DeadlockOutcome::Undefined(_)));
    let closed = ClosedSet::of(&exploration).expect("closed");
    let emitted = certificate::emit_finite_closure(&model, closed, &envelope());
    assert!(
        matches!(emitted, Err(EmissionError::Undefined { .. })),
        "{emitted:?}"
    );
}

/// A chain one of whose intermediate names is an action (`I#defined` is an action,
/// and `I#defined#defined` a predicate) is read as an action's, as it was before the
/// chain was followed: it invalidates `J` and the deadlock question. An action schema
/// `I#defined(k)` does the same.
#[test]
fn a_chain_through_an_action_name_fails_closed() {
    for action in ["I#defined", "I#defined(k=0)"] {
        let model = ModelBuilder::new()
            .variable("x", 0, 1)
            .action(ActionDecl::deterministic(
                action,
                x_is(0),
                vec![("x", IntExpr::constant(1))],
            ))
            .predicate("I", BoolExpr::constant(true))
            .predicate("J", BoolExpr::constant(true))
            .predicate("I#defined#defined", x_is(0))
            .initial_state(&[("x", 0)])
            .build()
            .expect("builds");
        let exploration = bfs::explore(&model, Bounds::CERTIFIABLE).expect("explores");
        let j = model.predicate_index("J").unwrap();
        let report = checking::check(
            &model,
            &exploration,
            &Obligations::new(DeadlockPolicy::Allowed).invariant(j),
        )
        .expect("checks");
        assert!(
            matches!(
                report.invariant(j).unwrap().outcome(),
                CheckOutcome::Undefined(_)
            ),
            "{action}: {report}"
        );
        assert!(
            matches!(report.deadlock(), DeadlockOutcome::Undefined(_)),
            "{action}"
        );
    }
}

/// A witness to an action guard's own value respects that guard's deeper guards:
/// `Step#defined` is true everywhere, but where `Step#defined#defined` is false it has
/// no value, so the only `Holds` endpoint is the initial state.
#[test]
fn a_witness_to_an_action_guard_respects_its_deeper_guard() {
    let model = model(&[
        ("Step#defined", BoolExpr::constant(true)),
        ("Step#defined#defined", x_is(0)),
    ]);
    let exploration = bfs::explore(&model, Bounds::CERTIFIABLE).expect("explores");
    let guard = model.predicate_index("Step#defined").unwrap();
    let found = witness::shortest(&model, &exploration, &Target::Holds(guard)).expect("found");
    assert_eq!(found.len(), 0);
    assert_eq!(found.target().as_slice(), &[0]);
    let deeper = model.predicate_index("Step#defined#defined").unwrap();
    // `Holds(Step#defined#defined)` has no deeper guard: its endpoint is x = 0 too, and
    // `Fails` of it is x = 1, one step away.
    let fails = witness::shortest(&model, &exploration, &Target::Fails(deeper)).expect("found");
    assert_eq!(fails.len(), 1);
}

/// A nested guard that cannot be evaluated is an engine error, reported before the
/// shallower guard is read: never a verdict.
#[test]
fn a_nested_guard_that_fails_to_evaluate_is_an_engine_error() {
    let overflow = BoolExpr::compare(
        CmpOp::Eq,
        IntExpr::times(IntExpr::constant(i64::MAX), IntExpr::constant(2)),
        IntExpr::constant(0),
    );
    let model = model(&[
        ("I", BoolExpr::constant(true)),
        ("I#defined", BoolExpr::constant(true)),
        ("I#defined#defined", overflow),
    ]);
    let exploration = bfs::explore(&model, Bounds::CERTIFIABLE).expect("explores");
    let i = model.predicate_index("I").unwrap();
    let report = checking::check(
        &model,
        &exploration,
        &Obligations::new(DeadlockPolicy::Allowed).invariant(i),
    )
    .expect("checks");
    assert!(
        matches!(
            report.invariant(i).unwrap().outcome(),
            CheckOutcome::Inconclusive(Unresolved::EngineError { .. })
        ),
        "{report}"
    );
    let closed = ClosedSet::of(&exploration).expect("closed");
    let emitted = certificate::emit_invariant_closure(&model, closed, &envelope(), "I");
    assert!(
        matches!(emitted, Err(EmissionError::DefinednessEvaluation { .. })),
        "{emitted:?}"
    );
}

/// cr-pt5h3a round 2: an action sharing the FULL name of a chain member — at depth 1
/// (`I#defined`), at depth 2 (`I#defined#defined`), or as a schema of instances — makes
/// the chain an action's (fail closed). The false member then invalidates `J`, the
/// deadlock question and liveness, and both emitters refuse.
#[test]
fn an_action_sharing_a_chain_members_full_name_fails_closed() {
    for (action, members) in [
        ("I#defined", &[("I#defined", false)][..]),
        ("I#defined(k=0)", &[("I#defined", false)][..]),
        (
            "I#defined#defined",
            &[("I#defined", true), ("I#defined#defined", false)][..],
        ),
        (
            "I#defined#defined(k=0)",
            &[("I#defined", true), ("I#defined#defined", false)][..],
        ),
    ] {
        let mut builder = ModelBuilder::new()
            .variable("x", 0, 1)
            .action(ActionDecl::deterministic(
                "Step",
                x_is(0),
                vec![("x", IntExpr::constant(1))],
            ))
            // Never enabled: it only shares a name with a chain member.
            .action(ActionDecl::deterministic(
                action,
                BoolExpr::constant(false),
                vec![],
            ))
            .predicate("I", BoolExpr::constant(true))
            .predicate("J", BoolExpr::constant(true))
            .initial_state(&[("x", 0)]);
        for (name, always) in members {
            let body = if *always {
                BoolExpr::constant(true)
            } else {
                x_is(0)
            };
            builder = builder.predicate(name, body);
        }
        let model = builder.build().expect("builds");
        let exploration = bfs::explore(&model, Bounds::CERTIFIABLE).expect("explores");
        let j = model.predicate_index("J").unwrap();
        let report = checking::check(
            &model,
            &exploration,
            &Obligations::new(DeadlockPolicy::Defect).invariant(j),
        )
        .expect("checks");
        let CheckOutcome::Undefined(u) = report.invariant(j).unwrap().outcome() else {
            panic!("{action}: {report}");
        };
        assert_eq!(u.subject(), "I", "{action}");
        assert!(
            matches!(report.deadlock(), DeadlockOutcome::Undefined(_)),
            "{action}"
        );
        let live = check_liveness(&model, &exploration, Goal::Eventually(j), Stuttering::Never)
            .expect("runs");
        assert!(
            matches!(live, LivenessOutcome::Undefined(_)),
            "{action}: {live:?}"
        );
        let closed = ClosedSet::of(&exploration).expect("closed");
        let finite = certificate::emit_finite_closure(&model, closed, &envelope());
        assert!(
            matches!(finite, Err(EmissionError::Undefined { .. })),
            "{action}: {finite:?}"
        );
        let inv = certificate::emit_invariant_closure(&model, closed, &envelope(), "J");
        assert!(
            matches!(inv, Err(EmissionError::Undefined { .. })),
            "{action}: {inv:?}"
        );
        // A witness to `I`'s value ends only where its chain holds: at x = 0.
        let i = model.predicate_index("I").unwrap();
        let found = witness::shortest(&model, &exploration, &Target::Holds(i)).expect("found");
        assert_eq!(found.len(), 0, "{action}");
        assert!(
            matches!(
                witness::shortest(&model, &exploration, &Target::Fails(i)),
                Err(NoWitness::Unreached { .. })
            ),
            "{action}"
        );
    }
}
