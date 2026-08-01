//! Adversarial decode of the `optimization` field group (PR-4 / IMPL-08).
//!
//! # Why this file exists
//!
//! The group has no closed *token* vocabulary — all three sets carry free strings —
//! so its fail-closed surface is its *key* vocabulary and its set discipline. Two
//! failure modes matter more here than elsewhere:
//!
//! - A fourth key. `additionalProperties: false` holds on the `optimization` object,
//!   and a reader that ignored an unrecognized key would silently drop a category of
//!   obligation the contract meant to carry — the vacuity failure INV-012 exists to
//!   prevent, arriving through the decoder instead of through a repair.
//! - A member landing in the wrong set. `non_vacuity` and `optimization` are governed
//!   by separate verbs, so a decoder that folded the three arrays together would make
//!   `no-removal` on `non_vacuity` unenforceable (RFC 0037, correction 5).
//!
//! Every input is a mutation of a ~120-byte baseline; the truncation sweep is linear
//! in it, and nothing is built by doubling.

use continuum_intent::canonical_json::JsonError;
use continuum_intent::change_policy::PolicyField;
use continuum_intent::optimization::{
    ObjectiveRole, Optimization, OptimizationDecodeError, OptimizationError,
};

/// A well-formed `optimization` block, in canonical artifact form.
const GOOD: &str = concat!(
    r#"{"hard":["progress"],"non_vacuity":["a request is acknowledged"],"#,
    r#""soft":["stable_writes"]}"#,
);

fn error(text: &str) -> OptimizationDecodeError {
    Optimization::decode(text.as_bytes()).expect_err("a malformed artifact must not decode")
}

#[test]
fn the_baseline_is_well_formed_and_round_trips() {
    let group = Optimization::decode(GOOD.as_bytes()).expect("the baseline decodes");
    assert_eq!(group.to_artifact_bytes(), GOOD.as_bytes());
    assert_eq!(group.units().count(), 3);
}

// --- truncation ------------------------------------------------------------------------

#[test]
fn every_truncation_is_a_typed_error_and_never_a_panic() {
    for cut in 1..GOOD.len() {
        // A prefix that closes cleanly is a *smaller group*, not a partial one, so it
        // may decode; it must never panic, and it must never decode into something
        // carrying a member the truncated text does not contain.
        match Optimization::decode(&GOOD.as_bytes()[..cut]) {
            Ok(decoded) => {
                for unit in decoded.units() {
                    assert!(
                        GOOD[..cut].contains(unit.as_str()),
                        "truncation at {cut} invented the member {unit}"
                    );
                }
            }
            Err(error) => assert!(
                matches!(error, OptimizationDecodeError::Json(_)),
                "truncation at {cut} produced {error:?}"
            ),
        }
    }
}

// --- the key vocabulary -------------------------------------------------------------------

#[test]
fn a_fourth_set_is_rejected_rather_than_dropped() {
    assert_eq!(
        error(&GOOD.replace(r#""hard""#, r#""required":["progress"],"hard""#)),
        OptimizationDecodeError::UnknownField {
            field: "optimization",
            key: "required".to_owned()
        }
    );
    // A near miss on the one key INV-012 leans on is the dangerous case: a reader that
    // ignored `nonvacuity` would decode a contract with *no* non-vacuity obligation and
    // report it as well formed.
    assert_eq!(
        error(&GOOD.replace(r#""non_vacuity""#, r#""nonvacuity""#)),
        OptimizationDecodeError::UnknownField {
            field: "optimization",
            key: "nonvacuity".to_owned()
        }
    );
    assert_eq!(
        error(&GOOD.replace(r#""non_vacuity""#, r#""non-vacuity""#)),
        OptimizationDecodeError::UnknownField {
            field: "optimization",
            key: "non-vacuity".to_owned()
        }
    );
    // The policy key is not the contract path: `optimization.non_vacuity` is where the
    // set lives, and it is not a key of the object it lives in.
    assert_eq!(
        error(&GOOD.replace(r#""non_vacuity""#, r#""optimization.non_vacuity""#)),
        OptimizationDecodeError::UnknownField {
            field: "optimization",
            key: "optimization.non_vacuity".to_owned()
        }
    );
    assert_eq!(
        error(&GOOD.replace(r#""hard""#, r#""$comment":"x","hard""#)),
        OptimizationDecodeError::UnknownField {
            field: "optimization",
            key: "$comment".to_owned()
        }
    );
}

#[test]
fn a_duplicate_key_is_rejected_rather_than_resolved_by_arrival_order() {
    // Two `non_vacuity` arrays, one populated and one empty: last-writer-wins would
    // make the presence of the obligation a property of the parser.
    assert!(matches!(
        error(&GOOD.replace(
            r#""non_vacuity":["a request is acknowledged"]"#,
            r#""non_vacuity":["a request is acknowledged"],"non_vacuity":[]"#
        )),
        OptimizationDecodeError::Json(JsonError::DuplicateKey { .. })
    ));
}

#[test]
fn a_repeated_member_is_rejected_rather_than_collapsed() {
    // `uniqueItems: true` on all three arrays.
    assert_eq!(
        error(&GOOD.replace(
            r#""hard":["progress"]"#,
            r#""hard":["progress","progress"]"#
        )),
        OptimizationDecodeError::Optimization(OptimizationError::DuplicateObjective {
            role: ObjectiveRole::Hard,
            objective: "progress".to_owned()
        })
    );
    assert_eq!(
        error(&GOOD.replace(
            r#""non_vacuity":["a request is acknowledged"]"#,
            r#""non_vacuity":["a request is acknowledged","a request is acknowledged"]"#
        )),
        OptimizationDecodeError::Optimization(OptimizationError::DuplicateNonVacuity {
            behavior: "a request is acknowledged".to_owned()
        })
    );
}

#[test]
fn an_empty_string_member_is_rejected() {
    assert_eq!(
        error(&GOOD.replace(r#""hard":["progress"]"#, r#""hard":[""]"#)),
        OptimizationDecodeError::Optimization(OptimizationError::EmptyObjective)
    );
    assert_eq!(
        error(&GOOD.replace(
            r#""non_vacuity":["a request is acknowledged"]"#,
            r#""non_vacuity":[""]"#
        )),
        OptimizationDecodeError::Optimization(OptimizationError::EmptyObjective)
    );
}

#[test]
fn a_wrong_json_type_names_what_was_expected_and_what_arrived() {
    assert_eq!(
        error(&GOOD.replace(r#""hard":["progress"]"#, r#""hard":"progress""#)),
        OptimizationDecodeError::TypeMismatch {
            field: "optimization.hard",
            expected: "array",
            found: "string"
        }
    );
    assert_eq!(
        error(&GOOD.replace(r#""hard":["progress"]"#, r#""hard":[7]"#)),
        OptimizationDecodeError::TypeMismatch {
            field: "optimization.hard",
            expected: "array of strings",
            found: "integer"
        }
    );
    assert_eq!(
        error(&GOOD.replace(
            r#""non_vacuity":["a request is acknowledged"]"#,
            r#""non_vacuity":[{"behavior":"a request is acknowledged"}]"#
        )),
        OptimizationDecodeError::TypeMismatch {
            field: "optimization.non_vacuity",
            expected: "array of strings",
            found: "object"
        }
    );
    assert_eq!(
        error(&GOOD.replace(r#""soft":["stable_writes"]"#, r#""soft":null"#)),
        OptimizationDecodeError::TypeMismatch {
            field: "optimization.soft",
            expected: "array",
            found: "null"
        }
    );
    assert_eq!(
        error("[]"),
        OptimizationDecodeError::TypeMismatch {
            field: "optimization",
            expected: "object",
            found: "array"
        }
    );
}

#[test]
fn trailing_bytes_and_non_utf8_and_floats_are_typed_errors() {
    assert_eq!(
        error(&format!("{GOOD}{GOOD}")),
        OptimizationDecodeError::Json(JsonError::TrailingBytes { at: GOOD.len() })
    );
    Optimization::decode(format!("{GOOD}\n").as_bytes())
        .expect("trailing whitespace alone is insignificant");
    assert!(matches!(
        error(&GOOD.replace(r#""hard":["progress"]"#, r#""hard":[1.5]"#)),
        OptimizationDecodeError::Json(JsonError::FloatingPoint { .. })
    ));
    let mut bytes = GOOD.as_bytes().to_vec();
    let at = GOOD
        .find("progress")
        .expect("the baseline names a constraint");
    bytes[at] = 0xff;
    assert!(matches!(
        Optimization::decode(&bytes).expect_err("invalid UTF-8 must not decode"),
        OptimizationDecodeError::Json(JsonError::NotUtf8 { .. })
    ));
}

#[test]
fn a_member_never_moves_between_sets_across_a_round_trip() {
    // The property the two policy keys depend on: a behavior stays a behavior, and a
    // constraint stays a constraint, through decode and re-encode.
    let group = Optimization::decode(GOOD.as_bytes()).expect("baseline");
    let round_tripped =
        Optimization::decode(&group.to_artifact_bytes()).expect("the encoder's output decodes");
    let before: Vec<(PolicyField, &str)> = group
        .units()
        .map(|unit| (unit.policy_field(), unit.as_str()))
        .collect();
    let after: Vec<(PolicyField, &str)> = round_tripped
        .units()
        .map(|unit| (unit.policy_field(), unit.as_str()))
        .collect();
    assert_eq!(before, after);
    assert_eq!(
        before,
        vec![
            (PolicyField::Optimization, "progress"),
            (PolicyField::Optimization, "stable_writes"),
            (PolicyField::NonVacuity, "a request is acknowledged"),
        ]
    );
}

#[test]
fn a_string_carrying_escapes_survives_with_one_spelling() {
    // The three sets are free strings, so unlike the AST vocabularies they *can* carry
    // an escape. The reader accepts every legal spelling and the writer emits one, so
    // two documents differing only in escape spelling reach one identity.
    let escaped = r#"{"hard":["a b"],"non_vacuity":["x\ty"],"soft":[]}"#;
    let plain = "{\"hard\":[\"a b\"],\"non_vacuity\":[\"x\\ty\"],\"soft\":[]}";
    let one = Optimization::decode(escaped.as_bytes()).expect("legal");
    let other = Optimization::decode(plain.as_bytes()).expect("legal");
    assert_eq!(one, other);
    assert_eq!(one.identity(), other.identity());
    assert_eq!(
        String::from_utf8(one.to_artifact_bytes()).expect("utf-8"),
        plain
    );
}

#[test]
fn every_rejection_renders_a_message_that_names_its_cause() {
    let cases = [
        GOOD.replace(r#""non_vacuity""#, r#""nonvacuity""#),
        GOOD.replace(
            r#""hard":["progress"]"#,
            r#""hard":["progress","progress"]"#,
        ),
        GOOD.replace(r#""hard":["progress"]"#, r#""hard":[""]"#),
        GOOD.replace(r#""hard":["progress"]"#, r#""hard":"progress""#),
        format!("{GOOD}{GOOD}"),
    ];
    for case in cases {
        let message = error(&case).to_string();
        assert!(
            message.len() > 20,
            "the message {message:?} is too thin to act on"
        );
    }
}
