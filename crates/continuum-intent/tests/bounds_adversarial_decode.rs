//! Adversarial decode: every malformed `bounds` artifact is a typed rejection
//! (PR-4 / IMPL-04, bn-1tgp).
//!
//! # Why this file exists
//!
//! The same trust-boundary obligation `tests/adversarial_decode.rs` states for
//! `properties` applies to `bounds`: it arrives from an untrusted intent bundle
//! (RFC 0037, INV-016) and must be rejected, with a type, and never
//! best-effort-decoded or panicked on. `bounds` has no recursive AST — it is a
//! flat four-component object — so the attack surface is narrower than a
//! property's, but the same discipline applies: a specific typed error for each
//! attack class, never a bare `is_err()`.
//!
//! # What is exercised here versus in `src/bounds.rs`
//!
//! The unit tests inside `crate::bounds` already cover the componentwise order
//! (`relation_to`), the per-component minimums, and the one-spelling-per-meaning
//! encoding rule directly against the typed API. This file is the artifact-bytes
//! half: truncation, unknown fields, duplicate keys, wrong JSON types, and
//! trailing bytes, plus round-trip and canonical-spelling evidence — the same
//! attack classes `tests/adversarial_decode.rs` runs for `properties`.

use continuum_intent::bounds::{
    Bounds, BoundsDecodeError, BoundsError, DeclaredBound, ExplorationBound,
};
use continuum_intent::canonical_json::JsonError;

/// A well-formed bounds tuple, in canonical artifact form: the schema's own
/// worked example (`notes/plan/schemas/examples/intent-contract.example.json`,
/// `bounds`).
const GOOD: &str = r#"{"depth":20,"faults":1,"nodes":2,"values":2}"#;

fn decode(text: &str) -> Result<Bounds, BoundsDecodeError> {
    Bounds::decode(text.as_bytes())
}

fn error(text: &str) -> BoundsDecodeError {
    decode(text).expect_err("a malformed artifact must not decode")
}

#[test]
fn the_baseline_is_well_formed() {
    let bounds = decode(GOOD).expect("the baseline decodes");
    assert_eq!(bounds.values(), ExplorationBound::Bounded(2));
    assert_eq!(bounds.nodes(), DeclaredBound::Declared(2));
    assert_eq!(bounds.faults(), DeclaredBound::Declared(1));
    assert_eq!(bounds.depth(), ExplorationBound::Bounded(20));
}

// --- round trip and canonical spelling ---------------------------------------------------

#[test]
fn a_wellformed_bounds_tuple_round_trips_through_its_artifact_form() {
    let bounds = decode(GOOD).expect("decodes");
    assert_eq!(bounds.to_artifact_bytes(), GOOD.as_bytes());
    let redecoded = Bounds::decode(&bounds.to_artifact_bytes()).expect("re-decodes");
    assert_eq!(bounds, redecoded);
    assert_eq!(bounds.identity(), redecoded.identity());
}

#[test]
fn reordering_object_keys_is_accepted_and_normalized_to_one_spelling() {
    let reordered = r#"{"values":2,"nodes":2,"depth":20,"faults":1}"#;
    let bounds = decode(reordered).expect("key order is not part of the document");
    assert_eq!(bounds.to_artifact_bytes(), GOOD.as_bytes());
    assert_eq!(bounds, decode(GOOD).expect("baseline"));
    assert_eq!(
        bounds.identity(),
        decode(GOOD).expect("baseline").identity()
    );
}

#[test]
fn the_empty_object_and_every_all_unbounded_undeclared_spelling_share_one_identity() {
    // `bounds` MAY be `{}` — every component reads as unbounded/undeclared, and
    // that is itself a declaration (RFC 0037), not an error.
    let omitted = decode("{}").expect("the empty object decodes");
    let explicit_nulls = decode(r#"{"depth":null,"values":null}"#).expect("decodes");
    assert_eq!(omitted, explicit_nulls);
    assert_eq!(omitted.identity(), explicit_nulls.identity());
    assert_eq!(
        omitted.to_artifact_bytes(),
        br#"{"depth":null,"values":null}"#
    );
}

#[test]
fn declaring_or_undeclaring_nodes_or_faults_each_move_the_identity() {
    let bare = decode("{}").expect("decodes");
    let with_nodes = decode(r#"{"nodes":2}"#).expect("decodes");
    let with_faults = decode(r#"{"faults":1}"#).expect("decodes");
    let with_both = decode(r#"{"faults":1,"nodes":2}"#).expect("decodes");
    assert_ne!(bare.identity(), with_nodes.identity());
    assert_ne!(bare.identity(), with_faults.identity());
    assert_ne!(with_nodes.identity(), with_faults.identity());
    assert_ne!(with_nodes.identity(), with_both.identity());
    assert_ne!(with_faults.identity(), with_both.identity());
}

// --- unknown fields, duplicate keys, and the JSON layer ----------------------------------

#[test]
fn an_unknown_field_is_rejected_because_the_schema_forbids_it() {
    assert_eq!(
        error(r#"{"depth":20,"faults":1,"generous":true,"nodes":2,"values":2}"#),
        BoundsDecodeError::UnknownField {
            field: "bounds",
            key: "generous".to_owned()
        }
    );
    // ID2 excludes `$comment` from the identity preimage, but
    // `additionalProperties: false` leaves it nowhere to sit.
    assert_eq!(
        error(r#"{"$comment":"x","depth":20,"faults":1,"nodes":2,"values":2}"#),
        BoundsDecodeError::UnknownField {
            field: "bounds",
            key: "$comment".to_owned()
        }
    );
}

#[test]
fn a_duplicate_key_is_rejected_rather_than_resolved_by_arrival_order() {
    assert!(matches!(
        error(r#"{"nodes":2,"nodes":3}"#),
        BoundsDecodeError::Json(JsonError::DuplicateKey { .. })
    ));
}

#[test]
fn trailing_bytes_are_rejected() {
    assert_eq!(
        error(&format!("{GOOD}{GOOD}")),
        BoundsDecodeError::Json(JsonError::TrailingBytes { at: GOOD.len() })
    );
    decode(&format!("{GOOD} ")).expect("trailing whitespace alone is insignificant");
    assert!(matches!(
        error(&format!("{GOOD} null")),
        BoundsDecodeError::Json(JsonError::TrailingBytes { .. })
    ));
}

#[test]
fn a_float_is_rejected_rather_than_rounded() {
    assert!(matches!(
        error(r#"{"values":2.5}"#),
        BoundsDecodeError::Json(JsonError::FloatingPoint { .. })
    ));
}

// --- truncation sweep ---------------------------------------------------------------------

#[test]
fn every_truncation_is_a_typed_error_and_never_a_panic() {
    for cut in 1..GOOD.len() {
        let truncated = &GOOD[..cut];
        let error = decode(truncated).expect_err("a truncated artifact must not decode");
        assert!(
            matches!(error, BoundsDecodeError::Json(_)),
            "truncation at {cut} produced {error:?}"
        );
    }
}

// --- shape and type violations -------------------------------------------------------------

#[test]
fn a_wrong_json_type_names_what_was_expected_and_what_arrived() {
    assert_eq!(
        error(r#"["not", "an", "object"]"#),
        BoundsDecodeError::TypeMismatch {
            field: "bounds",
            expected: "object",
            found: "array"
        }
    );
    assert_eq!(
        error(r#"{"values":"two"}"#),
        BoundsDecodeError::TypeMismatch {
            field: "bounds.values",
            expected: "integer or null",
            found: "string"
        }
    );
    assert_eq!(
        error(r#"{"depth":[20]}"#),
        BoundsDecodeError::TypeMismatch {
            field: "bounds.depth",
            expected: "integer or null",
            found: "array"
        }
    );
    // `nodes`/`faults` admit no `null`: there is no meaning it could stand for
    // (module documentation) — a `null` here is a type mismatch, never read as
    // "undeclared".
    assert_eq!(
        error(r#"{"nodes":null}"#),
        BoundsDecodeError::TypeMismatch {
            field: "bounds.nodes",
            expected: "integer",
            found: "null"
        }
    );
    assert_eq!(
        error(r#"{"faults":null}"#),
        BoundsDecodeError::TypeMismatch {
            field: "bounds.faults",
            expected: "integer",
            found: "null"
        }
    );
    assert_eq!(
        error(r#"{"faults":true}"#),
        BoundsDecodeError::TypeMismatch {
            field: "bounds.faults",
            expected: "integer",
            found: "boolean"
        }
    );
}

#[test]
fn each_components_schema_minimum_is_enforced_on_the_way_in() {
    assert_eq!(
        error(r#"{"values":0}"#),
        BoundsDecodeError::Bounds(BoundsError::BelowMinimum {
            field: "bounds.values",
            minimum: 1,
            found: 0
        })
    );
    assert_eq!(
        error(r#"{"nodes":0}"#),
        BoundsDecodeError::Bounds(BoundsError::BelowMinimum {
            field: "bounds.nodes",
            minimum: 1,
            found: 0
        })
    );
    assert_eq!(
        error(r#"{"faults":-1}"#),
        BoundsDecodeError::Bounds(BoundsError::BelowMinimum {
            field: "bounds.faults",
            minimum: 0,
            found: -1
        })
    );
    assert_eq!(
        error(r#"{"depth":-1}"#),
        BoundsDecodeError::Bounds(BoundsError::BelowMinimum {
            field: "bounds.depth",
            minimum: 0,
            found: -1
        })
    );
    // The boundary values themselves are legal.
    assert!(decode(r#"{"depth":0,"faults":0,"nodes":1,"values":1}"#).is_ok());
}

// --- the comparison surface, exercised end to end through decode -------------------------

#[test]
fn decoded_bounds_expose_the_typed_relation_rfc_0031_needs() {
    let five = decode(r#"{"nodes":5}"#).expect("decodes");
    let three = decode(r#"{"nodes":3}"#).expect("decodes");
    // RFC 0031's own gaming-move example.
    assert_eq!(
        five.relation_to(&three),
        continuum_intent::bounds::BoundsRelation::Contracted
    );
    assert_eq!(
        three.relation_to(&five),
        continuum_intent::bounds::BoundsRelation::Expanded
    );
    assert_eq!(
        five.relation_to(&five),
        continuum_intent::bounds::BoundsRelation::Unchanged
    );
    let no_faults = decode("{}").expect("decodes");
    let with_faults = decode(r#"{"faults":0}"#).expect("decodes");
    assert_eq!(
        no_faults.relation_to(&with_faults),
        continuum_intent::bounds::BoundsRelation::Unknown
    );
}

// --- every rejection is reportable ---------------------------------------------------------

#[test]
fn every_rejection_renders_a_message_that_names_its_cause() {
    let cases = [
        r#"{"generous":true}"#,
        r#"{"values":"two"}"#,
        r#"{"nodes":0}"#,
        &format!("{GOOD}{GOOD}"),
    ];
    for case in cases {
        let message = error(case).to_string();
        assert!(!message.is_empty());
        assert!(
            message.len() > 10,
            "the message {message:?} is too thin to act on"
        );
    }
}
