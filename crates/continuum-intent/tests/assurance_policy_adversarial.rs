//! Adversarial decode of the `assurance` field group (PR-4 / IMPL-07).
//!
//! # Why this file exists
//!
//! `assurance` is the field that says how strong the evidence must be, so a decoder
//! that is generous here is generous about proof. RFC 0037 gives it one instruction:
//!
//! > A reader that encounters an unrecognized token in any of those vocabularies MUST
//! > fail closed: it MUST reject the contract […] Forward compatibility is achieved by
//! > rejecting, never by ignoring.
//!
//! The specific hazard this group has, which the sibling groups do not, is that it
//! carries *two* closed vocabularies whose members look alike and which share the token
//! `sampled`. A reader that resolved a token against whichever of the two happened to
//! match would let `minimum: "example"` — an evidence class — become an assurance
//! demand. Both directions are tested below.
//!
//! Every input is a mutation of a ~110-byte baseline; the truncation sweep is linear
//! in it, and nothing is built by doubling.

use continuum_intent::assurance_policy::{
    AssuranceDecodeError, AssurancePolicy, AssurancePolicyError,
};
use continuum_intent::canonical_json::JsonError;
use continuum_value::assurance::{AssuranceLevel, EvidenceClass};

/// A well-formed assurance block, in canonical artifact form.
const GOOD: &str = concat!(
    r#"{"accepted_evidence_classes":["finite-exact","inductive"],"clean_recompute":true,"#,
    r#""independent_checker":true,"minimum":"bounded"}"#,
);

fn error(text: &str) -> AssuranceDecodeError {
    AssurancePolicy::decode(text.as_bytes()).expect_err("a malformed artifact must not decode")
}

#[test]
fn the_baseline_is_well_formed_and_round_trips() {
    let group = AssurancePolicy::decode(GOOD.as_bytes()).expect("the baseline decodes");
    assert_eq!(group.to_artifact_bytes(), GOOD.as_bytes());
    assert_eq!(group.minimum(), AssuranceLevel::Bounded);
    assert!(group.accepts(EvidenceClass::FiniteExact));
    assert!(group.accepts(EvidenceClass::Inductive));
    assert!(!group.accepts(EvidenceClass::Sampled));
}

// --- truncation ------------------------------------------------------------------------

#[test]
fn every_truncation_is_a_typed_error_and_never_a_panic() {
    for cut in 1..GOOD.len() {
        let error = AssurancePolicy::decode(&GOOD.as_bytes()[..cut])
            .expect_err("a truncated artifact must not decode");
        assert!(
            matches!(
                error,
                AssuranceDecodeError::Json(_) | AssuranceDecodeError::MissingField { .. }
            ),
            "truncation at {cut} produced {error:?}"
        );
    }
}

// --- the two closed vocabularies -------------------------------------------------------

#[test]
fn an_unknown_assurance_level_is_rejected_and_never_rounded_to_a_neighbour() {
    for token in ["exhaustive", "Bounded", "BOUNDED", "bounded ", "proven", ""] {
        assert_eq!(
            error(&GOOD.replace(r#""minimum":"bounded""#, &format!(r#""minimum":"{token}""#))),
            AssuranceDecodeError::UnknownToken {
                field: "assurance.minimum",
                token: token.to_owned()
            },
            "the token {token:?} did not fail closed"
        );
    }
}

#[test]
fn an_unknown_evidence_class_is_rejected() {
    for token in ["fuzzing", "Inductive", "dpor_complete", ""] {
        assert_eq!(
            error(&GOOD.replace(r#""inductive""#, &format!(r#""{token}""#))),
            AssuranceDecodeError::UnknownToken {
                field: "assurance.accepted_evidence_classes[]",
                token: token.to_owned()
            },
            "the token {token:?} did not fail closed"
        );
    }
}

#[test]
fn neither_vocabulary_can_borrow_the_others_tokens() {
    // A level is what the intent demands; an evidence class is what an artifact is.
    // `example` is a class and not a level.
    assert_eq!(
        error(&GOOD.replace(r#""minimum":"bounded""#, r#""minimum":"example""#)),
        AssuranceDecodeError::UnknownToken {
            field: "assurance.minimum",
            token: "example".to_owned()
        }
    );
    assert_eq!(
        error(&GOOD.replace(r#""minimum":"bounded""#, r#""minimum":"certificate""#)),
        AssuranceDecodeError::UnknownToken {
            field: "assurance.minimum",
            token: "certificate".to_owned()
        }
    );
    // And `proved` is a level and not a class.
    assert_eq!(
        error(&GOOD.replace(r#""inductive""#, r#""proved""#)),
        AssuranceDecodeError::UnknownToken {
            field: "assurance.accepted_evidence_classes[]",
            token: "proved".to_owned()
        }
    );
    // The one shared token resolves in both, and means two different things: as a
    // level it is the second rung, as a class it is a randomized campaign.
    let both = GOOD
        .replace(r#""minimum":"bounded""#, r#""minimum":"sampled""#)
        .replace(r#""inductive""#, r#""sampled""#);
    let group = AssurancePolicy::decode(both.as_bytes()).expect("both readings are legal");
    assert_eq!(group.minimum(), AssuranceLevel::Sampled);
    assert!(group.accepts(EvidenceClass::Sampled));
    // Accepting the `sampled` *class* says nothing about the demanded *level*: the
    // group cannot rank the two, and does not try.
    assert!(!group.meets_minimum(AssuranceLevel::Observed));
}

// --- structural violations -------------------------------------------------------------

#[test]
fn the_required_minimum_is_required() {
    assert_eq!(
        error(&GOOD.replace(r#","minimum":"bounded""#, "")),
        AssuranceDecodeError::MissingField {
            field: "assurance.minimum"
        }
    );
    assert_eq!(
        error("{}"),
        AssuranceDecodeError::MissingField {
            field: "assurance.minimum"
        }
    );
}

#[test]
fn an_unknown_field_is_rejected_because_the_schema_forbids_it() {
    // `additionalProperties: false` on the `assurance` object. The dangerous case is a
    // key that *looks* like policy — a reader that ignored it would silently drop a
    // requirement the contract meant to state.
    assert_eq!(
        error(&GOOD.replace(
            r#""minimum":"bounded""#,
            r#""maximum":"proved","minimum":"bounded""#
        )),
        AssuranceDecodeError::UnknownField {
            field: "assurance",
            key: "maximum".to_owned()
        }
    );
    assert_eq!(
        error(&GOOD.replace(
            r#""minimum":"bounded""#,
            r#""independent_reviewer":true,"minimum":"bounded""#
        )),
        AssuranceDecodeError::UnknownField {
            field: "assurance",
            key: "independent_reviewer".to_owned()
        }
    );
    assert_eq!(
        error(&GOOD.replace(
            r#""minimum":"bounded""#,
            r#""$comment":"x","minimum":"bounded""#
        )),
        AssuranceDecodeError::UnknownField {
            field: "assurance",
            key: "$comment".to_owned()
        }
    );
}

#[test]
fn a_duplicate_key_is_rejected_rather_than_resolved_by_arrival_order() {
    // Two minimums, one strong and one weak: last-writer-wins would make the demanded
    // assurance a property of the parser.
    assert!(matches!(
        error(&GOOD.replace(
            r#""minimum":"bounded""#,
            r#""minimum":"proved","minimum":"observed""#
        )),
        AssuranceDecodeError::Json(JsonError::DuplicateKey { .. })
    ));
    assert!(matches!(
        error(&GOOD.replace(
            r#""clean_recompute":true"#,
            r#""clean_recompute":true,"clean_recompute":false"#
        )),
        AssuranceDecodeError::Json(JsonError::DuplicateKey { .. })
    ));
}

#[test]
fn a_repeated_evidence_class_is_rejected_rather_than_collapsed() {
    // `uniqueItems: true`. Collapsing the repeat would accept a document the schema
    // rejects; the array is a set and says so.
    assert_eq!(
        error(&GOOD.replace(r#""inductive""#, r#""inductive","inductive""#)),
        AssuranceDecodeError::Policy(AssurancePolicyError::DuplicateEvidenceClass {
            class: "inductive"
        })
    );
}

#[test]
fn a_wrong_json_type_names_what_was_expected_and_what_arrived() {
    assert_eq!(
        error(&GOOD.replace(r#""minimum":"bounded""#, r#""minimum":3"#)),
        AssuranceDecodeError::TypeMismatch {
            field: "assurance.minimum",
            expected: "string",
            found: "integer"
        }
    );
    assert_eq!(
        error(&GOOD.replace(
            r#""independent_checker":true"#,
            r#""independent_checker":"yes""#
        )),
        AssuranceDecodeError::TypeMismatch {
            field: "assurance.independent_checker",
            expected: "boolean",
            found: "string"
        }
    );
    assert_eq!(
        error(&GOOD.replace(r#""clean_recompute":true"#, r#""clean_recompute":1"#)),
        AssuranceDecodeError::TypeMismatch {
            field: "assurance.clean_recompute",
            expected: "boolean",
            found: "integer"
        }
    );
    assert_eq!(
        error(&GOOD.replace(
            r#""accepted_evidence_classes":["finite-exact","inductive"]"#,
            r#""accepted_evidence_classes":"inductive""#
        )),
        AssuranceDecodeError::TypeMismatch {
            field: "assurance.accepted_evidence_classes",
            expected: "array",
            found: "string"
        }
    );
    assert_eq!(
        error(&GOOD.replace(r#""inductive""#, "7")),
        AssuranceDecodeError::TypeMismatch {
            field: "assurance.accepted_evidence_classes[]",
            expected: "string",
            found: "integer"
        }
    );
    assert_eq!(
        error("[]"),
        AssuranceDecodeError::TypeMismatch {
            field: "assurance",
            expected: "object",
            found: "array"
        }
    );
    // `null` is not "undeclared": the schema gives the two flags no null form, so a
    // null is a type error rather than a second spelling of absence.
    assert_eq!(
        error(&GOOD.replace(
            r#""independent_checker":true"#,
            r#""independent_checker":null"#
        )),
        AssuranceDecodeError::TypeMismatch {
            field: "assurance.independent_checker",
            expected: "boolean",
            found: "null"
        }
    );
}

#[test]
fn trailing_bytes_and_non_utf8_and_floats_are_typed_errors() {
    assert_eq!(
        error(&format!("{GOOD}{GOOD}")),
        AssuranceDecodeError::Json(JsonError::TrailingBytes { at: GOOD.len() })
    );
    AssurancePolicy::decode(format!("{GOOD} ").as_bytes())
        .expect("trailing whitespace alone is insignificant");
    assert!(matches!(
        error(&GOOD.replace(r#""clean_recompute":true"#, r#""clean_recompute":1.0"#)),
        AssuranceDecodeError::Json(JsonError::FloatingPoint { .. })
    ));
    let mut bytes = GOOD.as_bytes().to_vec();
    let at = GOOD.find("bounded").expect("the baseline names a level");
    bytes[at] = 0xff;
    assert!(matches!(
        AssurancePolicy::decode(&bytes).expect_err("invalid UTF-8 must not decode"),
        AssuranceDecodeError::Json(JsonError::NotUtf8 { .. })
    ));
}

#[test]
fn reordering_keys_and_members_is_accepted_and_normalized_away() {
    let reordered = concat!(
        r#"{"minimum":"bounded","independent_checker":true,"clean_recompute":true,"#,
        r#""accepted_evidence_classes":["inductive","finite-exact"]}"#,
    );
    let group = AssurancePolicy::decode(reordered.as_bytes()).expect("JSON objects are unordered");
    assert_eq!(group.to_artifact_bytes(), GOOD.as_bytes());
    assert_eq!(
        group.identity(),
        AssurancePolicy::decode(GOOD.as_bytes())
            .expect("baseline")
            .identity()
    );
}

#[test]
fn every_rejection_renders_a_message_that_names_its_cause() {
    let cases = [
        GOOD.replace(r#""minimum":"bounded""#, r#""minimum":"exhaustive""#),
        GOOD.replace(r#","minimum":"bounded""#, ""),
        GOOD.replace(r#""minimum":"bounded""#, r#""minimum":3"#),
        GOOD.replace(r#""inductive""#, r#""inductive","inductive""#),
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
