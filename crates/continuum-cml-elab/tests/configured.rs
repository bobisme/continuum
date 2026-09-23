//! Lowering under a run configuration (bn-3a9sr, RFC 0003 "Run configuration
//! (normative)", `notes/plan/schemas/run-config.schema.json`).
//!
//! # What is pinned
//!
//! - **the schema example** — `schemas/examples/replicated-register.run-config.json`
//!   is read by the Rust reader, names the schema file that governs it, and binds the
//!   replicated register, which then lowers as far as its (non-integer) state allows;
//! - **refusals** — the reader's (JSON, header, shape, duplicates, sort size, name
//!   length) and the binding check's (other model, missing or extra sort or constant,
//!   ill-typed values, a function-typed constant), each typed;
//! - **a differential** — the `Ring` model lowered under its configuration against
//!   the same model built by hand through `ModelBuilder` with the same values: one
//!   `Model::identity`, and one reference-engine exploration;
//! - **identity** — two configurations give two run identities, including when the
//!   lowered models are equal; equal models under equal configurations (spelled
//!   differently) give one.
//!
//! Resource cases are in `tests/resource_bounds.rs`.

use std::path::PathBuf;

use continuum_cml_elab::config::{ConfigErrorKind, RunConfig, RunIdentity};
use continuum_cml_elab::lower::{ConfigRefusalKind, Configured, lower_configured};
use continuum_cml_elab::{
    Limits, LowerError, LowerErrorKind, NormModel, Unlowerable, elaborate_source, lower,
};
use continuum_engine_reference::bfs::{self, Bounds};
use continuum_model_core::{ActionDecl, BoolExpr, CmpOp, IntExpr, Model, ModelBuilder};

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: &PathBuf) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn dossier(rel: &str) -> PathBuf {
    manifest().join("../../notes/plan").join(rel)
}

fn ring_model() -> NormModel {
    elaborate_source(&read(&manifest().join("tests/configs/Ring.ctm"))).expect("Ring elaborates")
}

fn ring_config_text() -> String {
    read(&manifest().join("tests/configs/ring.run-config.json"))
}

fn config(text: &str) -> RunConfig {
    RunConfig::parse(text.as_bytes()).unwrap_or_else(|e| panic!("reads: {e}"))
}

fn configured(model: &NormModel, text: &str) -> Result<Configured, LowerError> {
    lower_configured(model, &config(text), Limits::default()).0
}

/// A configuration document for `model` with the given `sorts` and `constants` JSON.
fn doc(model: &str, sorts: &str, constants: &str) -> String {
    format!(
        r#"{{"schema_id":"https://continuum.dev/schema/run-config.json","schema_epoch":1,"model":"{model}","sorts":{sorts},"constants":{constants}}}"#
    )
}

const RING_SORTS: &str = r#"{"Node":{"elements":["a","b","c"]}}"#;

fn ring_constants(start: &str, limit: i64, open: bool) -> String {
    format!(
        r#"{{"Start":{{"elem":{{"sort":"Node","name":"{start}"}}}},"Limit":{{"int":{limit}}},"Open":{{"bool":{open}}}}}"#
    )
}

// ---------------------------------------------------------------------------
// the schema example and the replicated register
// ---------------------------------------------------------------------------

/// The schema's example is read by the Rust reader, and the `schema_id` it carries
/// resolves (schemas/README.md's rule) to a schema file whose own `$id` and
/// `schema_epoch` agree. The JSON Schema validation of the same file is the dossier
/// validator's (`SCHEMA_PAIRS`).
#[test]
fn the_schema_example_reads_and_names_its_schema() {
    let text = read(&dossier(
        "schemas/examples/replicated-register.run-config.json",
    ));
    let c = config(&text);
    assert_eq!(c.model(), "ReplicatedRegister");
    assert_eq!(c.sorts().len(), 2);
    let schema = read(&dossier("schemas/run-config.schema.json"));
    assert!(schema.contains(r#""$id": "https://continuum.dev/schema/v1/run-config.json""#));
    assert!(schema.contains(r#""schema_epoch": 1"#));
    assert!(text.contains(r#""schema_id": "https://continuum.dev/schema/run-config.json""#));
}

/// The register's sorts and constants bind (no configuration refusal). Lowering stops
/// at its fairness line (bn-1ln12); without it the register lowers under the flat
/// layout (bn-23hzh; `tests/register.rs` checks the result against a simulation).
#[test]
fn the_replicated_register_binds_and_lowers_except_for_its_fairness() {
    let src = read(&dossier("examples/replicated_register.ctm"));
    let text = read(&dossier(
        "schemas/examples/replicated-register.run-config.json",
    ));
    let model = elaborate_source(&src).expect("elaborates");
    let err = configured(&model, &text).expect_err("fairness");
    assert_eq!(err.kind, LowerErrorKind::Unlowerable(Unlowerable::Fairness));

    let without = elaborate_source(&src.replace("fairness weak Recover", "")).expect("elaborates");
    let lowered = configured(&without, &text).expect("map-valued state lowers (bn-23hzh)");
    assert_eq!(lowered.model().variables().len(), 14);
    assert_eq!(lowered.model().actions().len(), 50);

    // The bindings are checked before anything else: dropping `Quorum` from the
    // configuration is refused ahead of the fairness line.
    let short = text.replace(r#""Quorum""#, r#""Unused""#);
    let err = configured(&model, &short).expect_err("Quorum is missing");
    assert_eq!(
        err.kind,
        LowerErrorKind::Configuration(continuum_cml_elab::lower::ConfigRefusal {
            kind: ConfigRefusalKind::MissingConstant,
            name: "Quorum".to_owned()
        })
    );
}

// ---------------------------------------------------------------------------
// refusals
// ---------------------------------------------------------------------------

fn refusal(model: &NormModel, text: &str) -> (ConfigRefusalKind, String) {
    match configured(model, text).expect_err("refused").kind {
        LowerErrorKind::Configuration(r) => (r.kind, r.name),
        other => panic!("not a configuration refusal: {other:?}"),
    }
}

#[test]
fn bindings_that_do_not_fit_the_model_are_refused_typed() {
    let ring = ring_model();
    let k = |kind: ConfigRefusalKind, name: &str| (kind, name.to_owned());
    let cases = [
        (
            doc("Other", RING_SORTS, &ring_constants("a", 2, true)),
            k(ConfigRefusalKind::OtherModel, "Other"),
        ),
        (
            doc("Ring", "{}", &ring_constants("a", 2, true)),
            k(ConfigRefusalKind::MissingSort, "Node"),
        ),
        (
            doc(
                "Ring",
                r#"{"Node":{"elements":["a"]},"Extra":{"elements":["x"]}}"#,
                &ring_constants("a", 2, true),
            ),
            k(ConfigRefusalKind::ExtraSort, "Extra"),
        ),
        (
            doc(
                "Ring",
                RING_SORTS,
                r#"{"Start":{"elem":{"sort":"Node","name":"a"}},"Limit":{"int":2}}"#,
            ),
            k(ConfigRefusalKind::MissingConstant, "Open"),
        ),
        (
            doc(
                "Ring",
                RING_SORTS,
                &ring_constants("a", 2, true).replace(r#""Open""#, r#""Extra":{"int":1},"Open""#),
            ),
            k(ConfigRefusalKind::ExtraConstant, "Extra"),
        ),
        // an element that is not in its sort
        (
            doc("Ring", RING_SORTS, &ring_constants("z", 2, true)),
            k(ConfigRefusalKind::IllTyped, "Start"),
        ),
        // a negative Nat
        (
            doc("Ring", RING_SORTS, &ring_constants("a", -1, true)),
            k(ConfigRefusalKind::IllTyped, "Limit"),
        ),
        // a Boolean where an integer is declared
        (
            doc(
                "Ring",
                RING_SORTS,
                &ring_constants("a", 2, true).replace(r#"{"int":2}"#, r#"{"bool":true}"#),
            ),
            k(ConfigRefusalKind::IllTyped, "Limit"),
        ),
        // an element of another sort
        (
            doc(
                "Ring",
                RING_SORTS,
                &ring_constants("a", 2, true).replace(r#""sort":"Node""#, r#""sort":"Other""#),
            ),
            k(ConfigRefusalKind::IllTyped, "Start"),
        ),
    ];
    for (text, expected) in cases {
        assert_eq!(refusal(&ring, &text), expected, "{text}");
    }
}

#[test]
fn structured_values_are_typed_and_function_constants_unbindable() {
    let model = elaborate_source(
        "module S\ntype P\nenum Phase { Idle, Busy }\nconst T: (Nat, Bool)\nconst R: {a: Nat, b: P}\nconst M: Map[P, Phase]\nconst O: Option[Seq[Nat]]\nstate { x: Nat where x <= 1 }\ninit { x == 0 }\naction A { unchanged x }\n",
    )
    .expect("elaborates");
    let sorts = r#"{"P":{"elements":["p","q"]}}"#;
    let good = r#"{"T":{"tuple":[{"int":1},{"bool":false}]},"R":{"record":{"a":{"int":0},"b":{"elem":{"sort":"P","name":"q"}}}},"M":{"map":[[{"elem":{"sort":"P","name":"p"}},{"variant":{"enum":"Phase","name":"Busy"}}]]},"O":{"some":{"seq":[{"int":1},{"int":1}]}}}"#;
    configured(&model, &doc("S", sorts, good)).expect("every structured value fits");
    let mut typed = 0;
    for (bad, name) in [
        (good.replace(r#"{"bool":false}"#, r#"{"int":0}"#), "T"),
        (good.replace(r#""a":{"int":0},"#, ""), "R"),
        (good.replace(r#""name":"Busy""#, r#""name":"Done""#), "M"),
        (
            good.replace(r#"{"some":"#, r#"{"some":{"none":null},"x":"#),
            "O",
        ),
    ] {
        let Ok(c) = RunConfig::parse(doc("S", sorts, &bad).as_bytes()) else {
            // A malformed value is the reader's refusal; that is also typed.
            continue;
        };
        let err = lower_configured(&model, &c, Limits::default())
            .0
            .expect_err("ill-typed");
        match err.kind {
            LowerErrorKind::Configuration(r) => {
                assert_eq!(
                    (r.kind, r.name.as_str()),
                    (ConfigRefusalKind::IllTyped, name)
                );
                typed += 1;
            }
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(typed, 3, "three of the four mutants reach the type check");

    let f = elaborate_source(
        "module F\nconst G: Nat -> Nat\nstate { x: Nat where x <= 1 }\ninit { x == 0 }\naction A { unchanged x }\n",
    )
    .expect("elaborates");
    let (kind, name) = refusal(&f, &doc("F", "{}", r#"{"G":{"int":0}}"#));
    assert_eq!(
        (kind, name.as_str()),
        (ConfigRefusalKind::UnbindableType, "G")
    );
}

fn reader_error(text: &str) -> ConfigErrorKind {
    RunConfig::parse(text.as_bytes())
        .expect_err("refused by the reader")
        .kind
}

#[test]
fn documents_the_schema_does_not_admit_are_refused_by_the_reader() {
    let ok = doc("Ring", RING_SORTS, &ring_constants("a", 2, true));
    RunConfig::parse(ok.as_bytes()).expect("the base document reads");
    assert!(matches!(reader_error("{"), ConfigErrorKind::Json(_)));
    assert!(matches!(
        reader_error(&ok.replace(r#""int":2"#, r#""int":2.5"#)),
        ConfigErrorKind::Json(_)
    ));
    assert!(matches!(
        reader_error(&ok.replace(r#""model":"Ring""#, r#""model":"Ring","model":"Ring""#)),
        ConfigErrorKind::Json(_)
    ));
    assert_eq!(
        reader_error(&ok.replace(r#""schema_epoch":1"#, r#""schema_epoch":2"#)),
        ConfigErrorKind::Header
    );
    assert!(matches!(
        reader_error(&ok.replace(r#""model":"Ring""#, r#""model":"Ring","faults":{}"#)),
        ConfigErrorKind::Shape(_)
    ));
    assert!(matches!(
        reader_error(&ok.replace(r#"{"int":2}"#, r#"{"int":2,"bool":true}"#)),
        ConfigErrorKind::Shape(_)
    ));
    assert!(matches!(
        reader_error(&ok.replace(r#"{"int":2}"#, r#"{"float":2}"#)),
        ConfigErrorKind::Shape(_)
    ));
    assert_eq!(
        reader_error(&ok.replace(r#"["a","b","c"]"#, r#"["a","b","a"]"#)),
        ConfigErrorKind::Duplicate
    );
    assert!(matches!(
        reader_error(&ok.replace(r#"["a","b","c"]"#, r#"["a b"]"#)),
        ConfigErrorKind::Shape(_)
    ));
    let long = "n".repeat(129);
    assert!(matches!(
        reader_error(&ok.replace(r#"["a","b","c"]"#, &format!(r#"["{long}"]"#))),
        ConfigErrorKind::Shape(_)
    ));
    // Duplicates by value, whatever the spelling: record field order does not matter.
    let set = r#"{"set":[{"record":{"a":{"int":1},"b":{"int":2}}},{"record":{"b":{"int":2},"a":{"int":1}}}]}"#;
    assert_eq!(
        reader_error(&ok.replace(r#"{"int":2}"#, set)),
        ConfigErrorKind::Duplicate
    );
    let map = r#"{"map":[[{"int":1},{"int":2}],[{"int":1},{"int":3}]]}"#;
    assert_eq!(
        reader_error(&ok.replace(r#"{"int":2}"#, map)),
        ConfigErrorKind::Duplicate
    );
}

/// Without a configuration nothing changes: the constants are still refused as
/// needing one.
#[test]
fn unconfigured_lowering_still_refuses_constants() {
    let err = lower(&ring_model()).expect_err("constants need a configuration");
    assert!(
        matches!(
            err.kind,
            LowerErrorKind::Unlowerable(Unlowerable::NonIntegerState | Unlowerable::Constant)
        ),
        "{err}"
    );
}

// ---------------------------------------------------------------------------
// the differential: configured CML against the programmatic model
// ---------------------------------------------------------------------------

/// The Ring model built by hand with `Node = {a, b, c}` (0, 1, 2), `Start = b` (1),
/// `Limit = 2`, and `Open = true`, in the shape the lowering documents: one action
/// per candidate of the relational `leader`, guard `Open && round < Limit && c !=
/// leader` (the guard clauses, then the postcondition, conjoined left to right).
fn programmatic_ring() -> Model {
    let mut b = ModelBuilder::new()
        .variable("leader", 0, 2)
        .variable("round", 0, 3)
        .initial_state(&[("leader", 1), ("round", 0)])
        .predicate(
            "Bounded",
            BoolExpr::compare(CmpOp::Le, IntExpr::var("round"), IntExpr::constant(2)),
        );
    for c in 0..=2 {
        let guard = BoolExpr::and(
            BoolExpr::and(
                BoolExpr::constant(true),
                BoolExpr::compare(CmpOp::Lt, IntExpr::var("round"), IntExpr::constant(2)),
            ),
            BoolExpr::compare(CmpOp::Ne, IntExpr::constant(c), IntExpr::var("leader")),
        );
        b = b.action(ActionDecl::deterministic(
            &format!("Pass[leader={c}]"),
            guard,
            vec![
                (
                    "round",
                    IntExpr::plus(IntExpr::var("round"), IntExpr::constant(1)),
                ),
                ("leader", IntExpr::constant(c)),
            ],
        ));
    }
    b.build().expect("a valid programmatic model")
}

#[test]
fn the_configured_ring_is_the_programmatic_ring() {
    let lowered = configured(&ring_model(), &ring_config_text()).expect("lowers");
    let oracle = programmatic_ring();
    assert_eq!(lowered.model(), &oracle);
    assert_eq!(lowered.model().identity(), oracle.identity());
    let explore = |m: &Model| bfs::explore(m, Bounds::CERTIFIABLE).expect("explores");
    assert_eq!(explore(lowered.model()), explore(&oracle));
    // 1 initial state; round 0 -> 1 -> 2 with the leader changing each time.
    let closed = explore(&oracle);
    let closed = closed.closed().expect("closes");
    assert_eq!(closed.len(), 1 + 2 + 3);
}

/// Anti-vacuity: the same model under `Start = a` is a different programmatic model.
#[test]
fn a_different_binding_is_told_apart_by_the_differential() {
    let other = doc("Ring", RING_SORTS, &ring_constants("a", 2, true));
    let lowered = configured(&ring_model(), &other).expect("lowers");
    assert_ne!(lowered.model().identity(), programmatic_ring().identity());
}

// ---------------------------------------------------------------------------
// identity
// ---------------------------------------------------------------------------

#[test]
fn two_configurations_always_give_two_run_identities() {
    let ring = ring_model();
    let ring_config = config(&ring_config_text());
    let base = configured(&ring, &ring_config_text()).expect("lowers");
    // Renamed elements: the same lowered model, a different configuration.
    let renamed = doc(
        "Ring",
        r#"{"Node":{"elements":["x","y","z"]}}"#,
        &ring_constants("y", 2, true),
    );
    let renamed = configured(&ring, &renamed).expect("lowers");
    assert_eq!(base.model().identity(), renamed.model().identity());
    assert_ne!(base.config_identity(), renamed.config_identity());
    assert_ne!(base.run_identity(), renamed.run_identity());
    // A different limit: a different model and a different run.
    let limit = configured(
        &ring,
        &doc("Ring", RING_SORTS, &ring_constants("b", 1, true)),
    )
    .expect("lowers");
    assert_ne!(base.model().identity(), limit.model().identity());
    assert_ne!(base.run_identity(), limit.run_identity());
    // The run identity is the typed pair, not either half.
    assert_eq!(
        base.run_identity(),
        &RunIdentity::of(base.model().identity(), ring_config.shared_identity())
    );
}

/// Metamorphic relation "serialization round trip": key order, whitespace, and set
/// member order do not change a configuration's identity, and equal models under equal
/// configurations have one run identity.
#[test]
fn equal_models_under_equal_configurations_share_a_run_identity() {
    let ring = ring_model();
    let a = configured(&ring, &ring_config_text()).expect("lowers");
    let respelled = r#"{ "constants": { "Open": {"bool": true}, "Limit": {"int": 2},
        "Start": {"elem": {"name": "b", "sort": "Node"}} },
      "sorts": {"Node": {"elements": ["a", "b", "c"]}},
      "model": "Ring", "schema_epoch": 1,
      "schema_id": "https://continuum.dev/schema/run-config.json" }"#;
    let b = configured(&ring, respelled).expect("lowers");
    assert_eq!(a.config_identity(), b.config_identity());
    assert_eq!(a.run_identity(), b.run_identity());

    let e = |n: &str| format!(r#"{{"elem":{{"sort":"Node","name":"{n}"}}}}"#);
    let with_nodes = |members: &[&str], elements: &str| {
        let set: Vec<String> = members.iter().map(|n| e(n)).collect();
        doc(
            "ReplicatedRegister",
            &format!(r#"{{"Node":{{"elements":{elements}}}}}"#),
            &format!(r#"{{"Nodes":{{"set":[{}]}}}}"#, set.join(",")),
        )
    };
    let one = config(&with_nodes(&["a", "b", "c"], r#"["a","b","c"]"#));
    let two = config(&with_nodes(&["c", "a", "b"], r#"["a","b","c"]"#));
    assert_eq!(one.identity(), two.identity(), "set order is not identity");
    // But the order of a sort's elements is (it is their integer encoding).
    let three = config(&with_nodes(&["a", "b", "c"], r#"["b","a","c"]"#));
    assert_ne!(one.identity(), three.identity());
}

// ---------------------------------------------------------------------------
// the reader against the schema (cr-1sxoia round 4)
// ---------------------------------------------------------------------------

/// The differential between the reader and the schema: every case of
/// `tests/configs/parity.json` — the committed fixtures, and generated edge names and
/// shapes for every field the schema constrains — carries the verdict of a Draft
/// 2020-12 validation of `run-config.schema.json` under its document profile
/// (`notes/plan/tools/run_config_parity.py`; the dossier gate regenerates it and fails
/// if it is stale). The reader must accept exactly the accepted cases and refuse the
/// rest.
#[test]
fn the_reader_agrees_with_the_schema_on_every_parity_case() {
    use continuum_intent::canonical_json::Json;
    let text = read(&manifest().join("tests/configs/parity.json"));
    let corpus = Json::parse(text.as_bytes()).expect("the corpus is strict JSON");
    let cases = corpus
        .as_object()
        .and_then(|o| o.get("cases"))
        .and_then(Json::as_array)
        .expect("cases");
    assert!(cases.len() > 150, "the corpus is not vacuous");
    let (mut accepted, mut refused) = (0, 0);
    let mut disagreements = Vec::new();
    for case in cases {
        let o = case.as_object().expect("a case");
        let name = o.get("name").and_then(Json::as_str).expect("name");
        let document = o.get("document").and_then(Json::as_str).expect("document");
        let accept = o.get("accept").and_then(Json::as_bool).expect("accept");
        let got = RunConfig::parse(document.as_bytes());
        if got.is_ok() != accept {
            disagreements.push(format!("{name}: schema {accept}, reader {got:?}"));
        }
        if accept {
            accepted += 1;
        } else {
            refused += 1;
        }
    }
    assert!(
        disagreements.is_empty(),
        "reader/schema disagreements:\n{}",
        disagreements.join("\n")
    );
    assert!(
        accepted > 50 && refused > 50,
        "{accepted} accepted, {refused} refused"
    );
}
