//! Explicit `Nat` and `Int` bounds from the run configuration (bn-15zfa, RFC 0003
//! correction 4, the schema's optional `bounds`).
//!
//! # What is pinned
//!
//! - **the reader** — `bounds` is read, typed, and refused exactly as the schema and its
//!   document profile state (`cml.config.bound_too_large`, shape errors); the parity
//!   corpus (`tests/configs/parity.json`, checked in `tests/configured.rs`) covers the
//!   same edges against the schema itself;
//! - **refusals** — a configuration constant outside the configuration's own bound is
//!   `cml.config.outside_bound`; under a configuration, an integer state variable with
//!   neither a finite refinement nor a bound is `cml.lower.unbounded_type`, never a
//!   silent default (ADR-0025); without a configuration nothing changes;
//! - **a differential** — a model with bounded `Nat` and `Int` state lowered under a
//!   configuration against the same model built by hand through `ModelBuilder`: one
//!   `Model`, one `Model::identity`, one reference-engine exploration; a different
//!   bound is told apart;
//! - **identity** — the bound is part of the configuration identity and so of the run
//!   identity; a configuration without `bounds` keeps the identity it had before
//!   correction 4.
//!
//! Resource cases are in `tests/resource_bounds.rs`.

use continuum_cml_elab::config::{
    CONFIG_IDENTITY_TAG, ConfigErrorKind, MAX_BOUND_VALUES, RunConfig,
};
use continuum_cml_elab::lower::{ConfigRefusal, ConfigRefusalKind, Configured, lower_configured};
use continuum_cml_elab::{
    Limits, LowerError, LowerErrorKind, NormModel, Unlowerable, elaborate_source, lower,
};
use continuum_engine_reference::bfs::{self, Bounds};
use continuum_model_core::{ActionDecl, BoolExpr, CmpOp, IntExpr, Model, ModelBuilder};

const HEAD: &str = r#""schema_id":"https://continuum.dev/schema/run-config.json","schema_epoch":1"#;

/// A configuration for `model` with the given `bounds` JSON (or none) and constants.
fn doc(model: &str, bounds: Option<&str>, constants: &str) -> String {
    match bounds {
        Some(b) => format!(
            r#"{{{HEAD},"model":"{model}","bounds":{b},"sorts":{{}},"constants":{constants}}}"#
        ),
        None => format!(r#"{{{HEAD},"model":"{model}","sorts":{{}},"constants":{constants}}}"#),
    }
}

fn config(text: &str) -> RunConfig {
    RunConfig::parse(text.as_bytes()).unwrap_or_else(|e| panic!("reads: {e}"))
}

fn reader_error(text: &str) -> ConfigErrorKind {
    RunConfig::parse(text.as_bytes())
        .expect_err("refused by the reader")
        .kind
}

fn elaborate(src: &str) -> NormModel {
    elaborate_source(src).unwrap_or_else(|e| panic!("elaborates: {e}"))
}

fn configured(model: &NormModel, text: &str) -> Result<Configured, LowerError> {
    lower_configured(model, &config(text), Limits::default()).0
}

fn unlowerable(model: &NormModel, text: &str) -> Unlowerable {
    match configured(model, text).expect_err("refused").kind {
        LowerErrorKind::Unlowerable(u) => u,
        other => panic!("not an Unlowerable: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// the reader
// ---------------------------------------------------------------------------

#[test]
fn bounds_are_read_and_absent_bounds_are_none() {
    let c = config(&doc(
        "M",
        Some(r#"{"Nat":{"max":3},"Int":{"min":-2,"max":2}}"#),
        "{}",
    ));
    assert_eq!(c.bounds().nat_max(), Some(3));
    assert_eq!(c.bounds().int_range(), Some((-2, 2)));
    let only_nat = config(&doc("M", Some(r#"{"Nat":{"max":0}}"#), "{}"));
    assert_eq!(only_nat.bounds().nat_max(), Some(0));
    assert_eq!(only_nat.bounds().int_range(), None);
    let none = config(&doc("M", None, "{}"));
    assert_eq!(none.bounds().nat_max(), None);
    assert_eq!(none.bounds().int_range(), None);
}

#[test]
fn the_reader_refuses_bounds_with_typed_errors() {
    let max = MAX_BOUND_VALUES as i64;
    // At the value limit it reads; one value more is `bound_too_large`.
    config(&doc(
        "M",
        Some(&format!(r#"{{"Nat":{{"max":{}}}}}"#, max - 1)),
        "{}",
    ));
    let over =
        RunConfig::parse(doc("M", Some(&format!(r#"{{"Nat":{{"max":{max}}}}}"#)), "{}").as_bytes())
            .expect_err("65536 is one value too many");
    assert_eq!(over.kind, ConfigErrorKind::BoundTooLarge);
    assert_eq!(over.code(), "cml.config.bound_too_large");
    assert_eq!(over.path, "$.bounds.Nat.max");
    let half = max / 2;
    config(&doc(
        "M",
        Some(&format!(
            r#"{{"Int":{{"min":{},"max":{}}}}}"#,
            -half,
            half - 1
        )),
        "{}",
    ));
    assert_eq!(
        reader_error(&doc(
            "M",
            Some(&format!(r#"{{"Int":{{"min":{},"max":{half}}}}}"#, -half)),
            "{}"
        )),
        ConfigErrorKind::BoundTooLarge
    );
    // The whole i64 range: the value count is computed without overflow.
    assert_eq!(
        reader_error(&doc(
            "M",
            Some(&format!(
                r#"{{"Int":{{"min":{},"max":{}}}}}"#,
                i64::MIN,
                i64::MAX
            )),
            "{}"
        )),
        ConfigErrorKind::BoundTooLarge
    );
    // Ranges at the i64 edges read.
    config(&doc(
        "M",
        Some(&format!(
            r#"{{"Int":{{"min":{},"max":{}}}}}"#,
            i64::MAX - 1,
            i64::MAX
        )),
        "{}",
    ));
    for bad in [
        r#"{}"#,
        r#"{"Nat":{"max":-1}}"#,
        r#"{"Nat":{"max":"3"}}"#,
        r#"{"Nat":{"max":true}}"#,
        r#"{"Nat":{}}"#,
        r#"{"Nat":{"max":3,"min":0}}"#,
        r#"{"Nat":3}"#,
        r#"{"Int":{"min":3,"max":2}}"#,
        r#"{"Int":{"max":2}}"#,
        r#"{"Int":{"min":0,"max":2,"step":1}}"#,
        r#"{"Real":{"max":1}}"#,
        r#"{"nat":{"max":1}}"#,
        r#"[]"#,
        r#"null"#,
    ] {
        assert!(
            matches!(
                reader_error(&doc("M", Some(bad), "{}")),
                ConfigErrorKind::Shape(_)
            ),
            "{bad}"
        );
    }
}

// ---------------------------------------------------------------------------
// identity
// ---------------------------------------------------------------------------

/// Metamorphic relation: serialization round trip. Respelling `bounds` (key order,
/// whitespace) keeps the configuration identity; changing its content changes it.
#[test]
fn the_bound_is_part_of_the_configuration_and_run_identity() {
    // A model that does not use `Nat` at all: the lowered model is the same, the run
    // is not.
    let model = elaborate(
        "module M\nstate { x: Int where x in 0..1 }\ninit { x == 0 }\naction A { unchanged x }\n",
    );
    let plain = configured(&model, &doc("M", None, "{}")).expect("lowers");
    let one = configured(&model, &doc("M", Some(r#"{"Nat":{"max":1}}"#), "{}")).expect("lowers");
    let two = configured(&model, &doc("M", Some(r#"{"Nat":{"max":2}}"#), "{}")).expect("lowers");
    assert_eq!(plain.model().identity(), one.model().identity());
    assert_eq!(one.model().identity(), two.model().identity());
    assert_ne!(plain.config_identity(), one.config_identity());
    assert_ne!(one.config_identity(), two.config_identity());
    assert_ne!(plain.run_identity(), one.run_identity());
    assert_ne!(one.run_identity(), two.run_identity());
    // `Nat` and `Int` bounds are told apart.
    let int = config(&doc("M", Some(r#"{"Int":{"min":0,"max":1}}"#), "{}"));
    let nat = config(&doc("M", Some(r#"{"Nat":{"max":1}}"#), "{}"));
    assert_ne!(int.identity(), nat.identity());
    // Spelling does not change it: key order and whitespace inside `bounds`.
    let a = config(&doc(
        "M",
        Some(r#"{"Int":{"min":-1,"max":1},"Nat":{"max":1}}"#),
        "{}",
    ));
    let b = config(&doc(
        "M",
        Some(r#"{ "Nat" : { "max" : 1 }, "Int" : { "max" : 1, "min" : -1 } }"#),
        "{}",
    ));
    assert_eq!(a.identity(), b.identity());
}

/// A configuration without `bounds` has exactly the identity it had before
/// correction 4: the tag and the canonical document with no `bounds` key.
#[test]
fn a_configuration_without_bounds_keeps_its_identity() {
    let c = config(&doc("M", None, r#"{"K":{"int":1}}"#));
    let mut want = CONFIG_IDENTITY_TAG.to_vec();
    want.extend_from_slice(
        br#"{"constants":{"K":{"int":1}},"model":"M","schema_epoch":1,"schema_id":"https://continuum.dev/schema/run-config.json","sorts":{}}"#,
    );
    assert_eq!(c.identity().as_bytes(), want.as_slice());
    let bounded = config(&doc(
        "M",
        Some(r#"{"Nat":{"max":1}}"#),
        r#"{"K":{"int":1}}"#,
    ));
    assert!(
        bounded
            .identity()
            .as_bytes()
            .windows(8)
            .any(|w| w == b"\"bounds\""),
        "a bound is in the identity"
    );
}

// ---------------------------------------------------------------------------
// refusals
// ---------------------------------------------------------------------------

fn refusal(model: &NormModel, text: &str) -> ConfigRefusal {
    match configured(model, text).expect_err("refused").kind {
        LowerErrorKind::Configuration(r) => r,
        other => panic!("not a configuration refusal: {other:?}"),
    }
}

#[test]
fn a_constant_outside_the_configured_bound_is_refused() {
    let model = elaborate(
        "module C\nconst N: Nat\nconst I: Int\nconst S: Set[Nat]\nstate { x: Nat where x <= 3 }\ninit { x == 0 }\naction A { unchanged x }\n",
    );
    let bounds = Some(r#"{"Nat":{"max":3},"Int":{"min":-1,"max":1}}"#);
    let consts = |n: i64, i: i64, s: &str| {
        format!(r#"{{"N":{{"int":{n}}},"I":{{"int":{i}}},"S":{{"set":[{s}]}}}}"#)
    };
    configured(&model, &doc("C", bounds, &consts(3, -1, r#"{"int":0}"#)))
        .expect("inside the bounds");
    let r = refusal(&model, &doc("C", bounds, &consts(4, 0, "")));
    assert_eq!(
        (r.kind, r.name.as_str()),
        (ConfigRefusalKind::OutsideBound, "N")
    );
    assert_eq!(r.kind.code(), "cml.config.outside_bound");
    let r = refusal(&model, &doc("C", bounds, &consts(0, 2, "")));
    assert_eq!(
        (r.kind, r.name.as_str()),
        (ConfigRefusalKind::OutsideBound, "I")
    );
    let r = refusal(&model, &doc("C", bounds, &consts(0, -2, "")));
    assert_eq!(
        (r.kind, r.name.as_str()),
        (ConfigRefusalKind::OutsideBound, "I")
    );
    // Inside a collection too.
    let r = refusal(
        &model,
        &doc("C", bounds, &consts(0, 0, r#"{"int":1},{"int":9}"#)),
    );
    assert_eq!(
        (r.kind, r.name.as_str()),
        (ConfigRefusalKind::OutsideBound, "S")
    );
    // A negative `Nat` is still ill-typed, bound or not.
    let r = refusal(&model, &doc("C", bounds, &consts(-1, 0, "")));
    assert_eq!(r.kind, ConfigRefusalKind::IllTyped);
    // Without a bound for the type, every value is admitted, as before.
    configured(
        &model,
        &doc(
            "C",
            Some(r#"{"Int":{"min":-1,"max":1}}"#),
            &consts(1000, 0, ""),
        ),
    )
    .expect("no Nat bound");
    configured(&model, &doc("C", None, &consts(1000, 5000, r#"{"int":7}"#))).expect("no bounds");
}

#[test]
fn an_unbounded_integer_type_is_refused_typed_under_a_configuration() {
    let nat = elaborate("module U\nstate { x: Nat }\ninit { x == 0 }\naction A { unchanged x }\n");
    // No configuration: unchanged.
    assert_eq!(
        lower(&nat).expect_err("unbounded").kind,
        LowerErrorKind::Unlowerable(Unlowerable::UnboundedDomain)
    );
    // A configuration with no bound for the type: the type is named.
    assert_eq!(
        unlowerable(&nat, &doc("U", None, "{}")),
        Unlowerable::UnboundedType
    );
    assert_eq!(
        Unlowerable::UnboundedType.code(),
        "cml.lower.unbounded_type"
    );
    assert_eq!(
        unlowerable(&nat, &doc("U", Some(r#"{"Int":{"min":0,"max":3}}"#), "{}")),
        Unlowerable::UnboundedType
    );
    // An `Int` bounded on one side only by its refinement, with only a `Nat` bound.
    let int = elaborate(
        "module U\nstate { x: Int where x >= 0 }\ninit { x == 0 }\naction A { unchanged x }\n",
    );
    assert_eq!(
        unlowerable(&int, &doc("U", Some(r#"{"Nat":{"max":3}}"#), "{}")),
        Unlowerable::UnboundedType
    );
    configured(&int, &doc("U", Some(r#"{"Int":{"min":-5,"max":2}}"#), "{}"))
        .expect("the Int bound closes it");
    // A refinement no value of the bound satisfies is empty, never widened.
    let high = elaborate(
        "module U\nstate { x: Nat where x >= 5 }\ninit { true }\naction A { unchanged x }\n",
    );
    assert_eq!(
        unlowerable(&high, &doc("U", Some(r#"{"Nat":{"max":3}}"#), "{}")),
        Unlowerable::EmptyRefinement
    );
}

// ---------------------------------------------------------------------------
// the differential: bounded state against the programmatic model
// ---------------------------------------------------------------------------

const COUNTER: &str = "module Counter
state {
  x: Nat
  y: Int where y <= 1
  z: Nat where z <= 10
}
init { x == 0 && y == -2 && z == 0 }
action IncX {
  require x < 3
  next x = x + 1
  unchanged y, z
}
action IncY {
  require y < 1
  next y = y + 1
  unchanged x, z
}
invariant Small { x <= 3 }
";

const COUNTER_BOUNDS: &str = r#"{"Nat":{"max":3},"Int":{"min":-2,"max":5}}"#;

/// `Counter` built by hand: `x` in `0..=3` (the `Nat` bound), `y` in `-2..=1` (the
/// `Int` bound narrowed by its refinement), `z` in `0..=3` (the refinement `z <= 10`
/// narrowed by the `Nat` bound).
fn programmatic_counter() -> Model {
    ModelBuilder::new()
        .variable("x", 0, 3)
        .variable("y", -2, 1)
        .variable("z", 0, 3)
        .initial_state(&[("x", 0), ("y", -2), ("z", 0)])
        .action(ActionDecl::deterministic(
            "IncX",
            BoolExpr::compare(CmpOp::Lt, IntExpr::var("x"), IntExpr::constant(3)),
            vec![("x", IntExpr::plus(IntExpr::var("x"), IntExpr::constant(1)))],
        ))
        .action(ActionDecl::deterministic(
            "IncY",
            BoolExpr::compare(CmpOp::Lt, IntExpr::var("y"), IntExpr::constant(1)),
            vec![("y", IntExpr::plus(IntExpr::var("y"), IntExpr::constant(1)))],
        ))
        .predicate(
            "Small",
            BoolExpr::compare(CmpOp::Le, IntExpr::var("x"), IntExpr::constant(3)),
        )
        .build()
        .expect("a valid programmatic model")
}

#[test]
fn bounded_state_lowers_to_the_programmatic_model() {
    let model = elaborate(COUNTER);
    let lowered = configured(&model, &doc("Counter", Some(COUNTER_BOUNDS), "{}")).expect("lowers");
    let oracle = programmatic_counter();
    assert_eq!(lowered.model(), &oracle);
    assert_eq!(lowered.model().identity(), oracle.identity());
    let explore = |m: &Model| bfs::explore(m, Bounds::CERTIFIABLE).expect("explores");
    assert_eq!(explore(lowered.model()), explore(&oracle));
    // x in 0..=3 and y in -2..=1 are each reachable independently; z stays 0.
    let closed = explore(&oracle);
    assert_eq!(closed.closed().expect("closes").len(), 4 * 4);
}

/// Anti-vacuity: a different bound is a different programmatic model, which the
/// differential tells apart; and without the bound the same model does not lower.
#[test]
fn a_different_bound_is_told_apart_by_the_differential() {
    let model = elaborate(COUNTER);
    let smaller = configured(
        &model,
        &doc(
            "Counter",
            Some(r#"{"Nat":{"max":2},"Int":{"min":-2,"max":5}}"#),
            "{}",
        ),
    )
    .expect("lowers");
    assert_ne!(smaller.model(), &programmatic_counter());
    assert_ne!(
        smaller.model().identity(),
        programmatic_counter().identity()
    );
    // Under `max: 2`, `IncX` from `x == 2` leaves the domain: reported, never clamped.
    let two = smaller.model().state(&[2, -2, 0]).expect("in the domain");
    assert!(smaller.model().successors(&two).is_err());
    assert_eq!(
        unlowerable(
            &model,
            &doc("Counter", Some(r#"{"Int":{"min":-2,"max":5}}"#), "{}")
        ),
        Unlowerable::UnboundedType
    );
}
