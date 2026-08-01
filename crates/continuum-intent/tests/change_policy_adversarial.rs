//! Adversarial decode of the `policy` table and the `policy_reviewers` map
//! (PR-4 / IMPL-09).
//!
//! # Why this file exists
//!
//! An Intent Contract arrives from an intent bundle, and RFC 0037 classes a bundle as
//! untrusted data (INV-016). The policy table is the part of that document which says
//! *what may be changed*, so a decoder that is loose here is loose about governance:
//!
//! > A reader that encounters an unrecognized token in any of those vocabularies MUST
//! > fail closed: it MUST reject the contract, and MUST NOT treat an unknown policy
//! > verb as `unlocked` […] Forward compatibility is achieved by rejecting, never by
//! > ignoring.
//! >
//! > — RFC 0037, "Versioning and revision"
//!
//! Every case below asserts a *specific* error rather than `is_err()`: a decoder that
//! returns one opaque failure for "truncated" and "unknown verb" is not a trust
//! boundary, and a rejection nobody can locate is a rejection nobody can act on.
//!
//! # Input sizes
//!
//! Every input here is one mutation of a ~500-byte baseline, and the truncation sweeps
//! are linear in it. Nothing is built by doubling.

use continuum_intent::canonical_json::JsonError;
use continuum_intent::change_policy::{
    ChangePolicyDecodeError, ChangePolicyError, PolicyField, PolicyReviewers, PolicyTable,
    PolicyVerb,
};

/// A well-formed policy table, in canonical artifact form: plan §5.4's example.
const GOOD: &str = concat!(
    r#"{"abstraction_maps":"review","assumptions":"review","assurance":"no-downgrade","#,
    r#""bounds":"no-decrease","completion_policy":"review","fairness":"review","#,
    r#""faults":"no-removal","non_vacuity":"no-removal","nondeterminism":"review","#,
    r#""observers":"review","optimization":"review","properties":"locked","scope":"review","#,
    r#""security_policy":"review","trust_boundaries":"no-expansion"}"#,
);

/// A well-formed reviewer map.
const REVIEWERS: &str =
    r#"{"assumptions":["did:continuum:alice"],"fairness":["did:continuum:bob"]}"#;

fn error(text: &str) -> ChangePolicyDecodeError {
    PolicyTable::decode(text.as_bytes()).expect_err("a malformed policy table must not decode")
}

fn reviewer_error(text: &str) -> ChangePolicyDecodeError {
    PolicyReviewers::decode(text.as_bytes()).expect_err("a malformed reviewer map must not decode")
}

#[test]
fn the_baselines_are_well_formed_and_round_trip() {
    let table = PolicyTable::decode(GOOD.as_bytes()).expect("the baseline decodes");
    assert_eq!(table.to_artifact_bytes(), GOOD.as_bytes());
    let reviewers = PolicyReviewers::decode(REVIEWERS.as_bytes()).expect("the baseline decodes");
    assert_eq!(reviewers.to_artifact_bytes(), REVIEWERS.as_bytes());
}

// --- truncation ------------------------------------------------------------------------

#[test]
fn every_truncation_of_a_policy_table_is_a_typed_error_and_never_a_panic() {
    for cut in 1..GOOD.len() {
        let error = PolicyTable::decode(&GOOD.as_bytes()[..cut])
            .expect_err("a truncated table must not decode");
        assert!(
            matches!(
                error,
                ChangePolicyDecodeError::Json(_)
                    | ChangePolicyDecodeError::Policy(ChangePolicyError::MissingField { .. })
            ),
            "truncation at {cut} produced {error:?}"
        );
    }
}

#[test]
fn every_truncation_of_a_reviewer_map_is_a_typed_error() {
    for cut in 1..REVIEWERS.len() {
        // A prefix that happens to close cleanly is a *shorter map*, not a partial
        // one, so it may decode; what it must never do is panic or silently invent a
        // field. Only the empty-object prefix and the full text can decode.
        match PolicyReviewers::decode(&REVIEWERS.as_bytes()[..cut]) {
            Ok(decoded) => assert!(
                decoded.is_empty(),
                "truncation at {cut} decoded a non-empty reviewer map"
            ),
            Err(error) => assert!(
                matches!(error, ChangePolicyDecodeError::Json(_)),
                "truncation at {cut} produced {error:?}"
            ),
        }
    }
}

// --- unknown tokens in every closed vocabulary ---------------------------------------------

#[test]
fn an_unknown_policy_verb_is_rejected_and_never_read_as_unlocked() {
    // The case RFC 0037 names outright. A `proof-required` verb is a *candidate* future
    // addition; until the RFC is revised and the schema epoch advances, a contract
    // carrying it is not decodable here, and it certainly does not decode as unlocked.
    assert_eq!(
        error(&GOOD.replace(
            r#""properties":"locked""#,
            r#""properties":"proof-required""#
        )),
        ChangePolicyDecodeError::UnknownVerb {
            field: PolicyField::Properties,
            token: "proof-required".to_owned()
        }
    );
    // A `no-addition` verb — the other open question — gets the same answer.
    assert_eq!(
        error(&GOOD.replace(r#""fairness":"review""#, r#""fairness":"no-addition""#)),
        ChangePolicyDecodeError::UnknownVerb {
            field: PolicyField::Fairness,
            token: "no-addition".to_owned()
        }
    );
    // Case matters: the wire tokens are lowercase.
    assert_eq!(
        error(&GOOD.replace(r#""properties":"locked""#, r#""properties":"Locked""#)),
        ChangePolicyDecodeError::UnknownVerb {
            field: PolicyField::Properties,
            token: "Locked".to_owned()
        }
    );
    // So does spelling: `unlock` is not `unlocked`, and the near miss is the dangerous
    // one — a reader that fell back to `unlocked` would have unlocked the field.
    assert_eq!(
        error(&GOOD.replace(r#""properties":"locked""#, r#""properties":"unlock""#)),
        ChangePolicyDecodeError::UnknownVerb {
            field: PolicyField::Properties,
            token: "unlock".to_owned()
        }
    );
    // The empty string is not a verb either.
    assert_eq!(
        error(&GOOD.replace(r#""scope":"review""#, r#""scope":"""#)),
        ChangePolicyDecodeError::UnknownVerb {
            field: PolicyField::Scope,
            token: String::new()
        }
    );
}

#[test]
fn a_policy_table_naming_a_field_the_contract_does_not_have_is_a_typed_rejection() {
    // The contract *path* is not the policy key: `claims` is what `properties`
    // governs, and a table keyed by the path is a table this reader does not accept.
    assert_eq!(
        error(&GOOD.replace(r#""properties":"locked""#, r#""claims":"locked""#)),
        ChangePolicyDecodeError::UnknownField {
            field: "policy",
            key: "claims".to_owned()
        }
    );
    assert_eq!(
        error(&GOOD.replace(r#""faults":"no-removal""#, r#""fault_model":"no-removal""#)),
        ChangePolicyDecodeError::UnknownField {
            field: "policy",
            key: "fault_model".to_owned()
        }
    );
    // A sixteenth field is the same answer — a policy over something the contract does
    // not carry cannot be enforced, so it is rejected rather than ignored.
    assert_eq!(
        error(&GOOD.replace(
            r#""scope":"review""#,
            r#""performance":"locked","scope":"review""#
        )),
        ChangePolicyDecodeError::UnknownField {
            field: "policy",
            key: "performance".to_owned()
        }
    );
    // Including `$comment`, which ID2 excludes from the identity preimage but which
    // `additionalProperties: false` leaves nowhere to sit inside `policy`.
    assert_eq!(
        error(&GOOD.replace(r#""scope":"review""#, r#""$comment":"x","scope":"review""#)),
        ChangePolicyDecodeError::UnknownField {
            field: "policy",
            key: "$comment".to_owned()
        }
    );
}

#[test]
fn a_reviewer_map_naming_a_field_the_contract_does_not_have_is_a_typed_rejection() {
    // `policy_reviewers` states its key vocabulary as `propertyNames`, so the offending
    // key is an unknown *token* rather than an unknown field. The distinction is the
    // schema's, and the error keeps it.
    assert_eq!(
        reviewer_error(r#"{"claims":["did:continuum:alice"]}"#),
        ChangePolicyDecodeError::UnknownToken {
            field: "policy_reviewers",
            token: "claims".to_owned()
        }
    );
    assert_eq!(
        reviewer_error(r#"{"reviewers":["did:continuum:alice"]}"#),
        ChangePolicyDecodeError::UnknownToken {
            field: "policy_reviewers",
            token: "reviewers".to_owned()
        }
    );
}

#[test]
fn an_inapplicable_verb_is_rejected_at_decode_time_and_not_only_at_construction() {
    // W6 through the decoder: `no-decrease` on `properties` is schema-invalid under the
    // per-field restriction, and this reader agrees.
    assert_eq!(
        error(&GOOD.replace(r#""properties":"locked""#, r#""properties":"no-decrease""#)),
        ChangePolicyDecodeError::Policy(ChangePolicyError::InapplicableVerb {
            field: PolicyField::Properties,
            verb: PolicyVerb::NoDecrease
        })
    );
    assert_eq!(
        error(&GOOD.replace(r#""bounds":"no-decrease""#, r#""bounds":"no-downgrade""#)),
        ChangePolicyDecodeError::Policy(ChangePolicyError::InapplicableVerb {
            field: PolicyField::Bounds,
            verb: PolicyVerb::NoDowngrade
        })
    );
    assert_eq!(
        error(&GOOD.replace(
            r#""trust_boundaries":"no-expansion""#,
            r#""trust_boundaries":"no-decrease""#
        )),
        ChangePolicyDecodeError::Policy(ChangePolicyError::InapplicableVerb {
            field: PolicyField::TrustBoundaries,
            verb: PolicyVerb::NoDecrease
        })
    );
}

// --- structural violations -------------------------------------------------------------

#[test]
fn a_missing_policy_key_is_rejected_because_all_fifteen_are_required() {
    assert_eq!(
        error(&GOOD.replace(r#""assurance":"no-downgrade","#, "")),
        ChangePolicyDecodeError::Policy(ChangePolicyError::MissingField {
            field: PolicyField::Assurance
        })
    );
    assert_eq!(
        error("{}"),
        ChangePolicyDecodeError::Policy(ChangePolicyError::MissingField {
            field: PolicyField::AbstractionMaps
        })
    );
}

#[test]
fn a_duplicate_key_is_rejected_rather_than_resolved_by_arrival_order() {
    // The dangerous case: one benign verb and one not. Last-writer-wins would make the
    // governance of a field depend on the parser.
    assert!(matches!(
        error(&GOOD.replace(
            r#""properties":"locked""#,
            r#""properties":"locked","properties":"unlocked""#
        )),
        ChangePolicyDecodeError::Json(JsonError::DuplicateKey { .. })
    ));
    assert!(matches!(
        reviewer_error(r#"{"fairness":["a"],"fairness":["b"]}"#),
        ChangePolicyDecodeError::Json(JsonError::DuplicateKey { .. })
    ));
}

#[test]
fn trailing_bytes_are_rejected() {
    assert_eq!(
        error(&format!("{GOOD}{GOOD}")),
        ChangePolicyDecodeError::Json(JsonError::TrailingBytes { at: GOOD.len() })
    );
    // Trailing whitespace alone is insignificant and is skipped; a trailing *value* is
    // a second document and is not.
    PolicyTable::decode(format!("{GOOD}  ").as_bytes()).expect("trailing whitespace is skipped");
    assert!(matches!(
        error(&format!("{GOOD} null")),
        ChangePolicyDecodeError::Json(JsonError::TrailingBytes { .. })
    ));
    assert!(matches!(
        reviewer_error(&format!("{REVIEWERS}{REVIEWERS}")),
        ChangePolicyDecodeError::Json(JsonError::TrailingBytes { .. })
    ));
}

#[test]
fn a_wrong_json_type_names_what_was_expected_and_what_arrived() {
    assert_eq!(
        error(&GOOD.replace(r#""scope":"review""#, r#""scope":7"#)),
        ChangePolicyDecodeError::TypeMismatch {
            field: "policy.<field>",
            expected: "string",
            found: "integer"
        }
    );
    assert_eq!(
        error(&GOOD.replace(r#""scope":"review""#, r#""scope":["review"]"#)),
        ChangePolicyDecodeError::TypeMismatch {
            field: "policy.<field>",
            expected: "string",
            found: "array"
        }
    );
    assert_eq!(
        error(&GOOD.replace(r#""scope":"review""#, r#""scope":null"#)),
        ChangePolicyDecodeError::TypeMismatch {
            field: "policy.<field>",
            expected: "string",
            found: "null"
        }
    );
    assert_eq!(
        error("[]"),
        ChangePolicyDecodeError::TypeMismatch {
            field: "policy",
            expected: "object",
            found: "array"
        }
    );
    assert_eq!(
        reviewer_error(r#"{"fairness":"did:continuum:bob"}"#),
        ChangePolicyDecodeError::TypeMismatch {
            field: "policy_reviewers.<field>",
            expected: "array",
            found: "string"
        }
    );
    assert_eq!(
        reviewer_error(r#"{"fairness":[7]}"#),
        ChangePolicyDecodeError::TypeMismatch {
            field: "policy_reviewers.<field>[]",
            expected: "string",
            found: "integer"
        }
    );
}

#[test]
fn an_empty_reviewer_list_is_rejected_as_an_unenforceable_block() {
    assert_eq!(
        reviewer_error(r#"{"fairness":[]}"#),
        ChangePolicyDecodeError::Policy(ChangePolicyError::EmptyReviewerList {
            field: PolicyField::Fairness
        })
    );
}

#[test]
fn a_float_and_non_utf8_input_are_typed_errors() {
    assert!(matches!(
        error(&GOOD.replace(r#""scope":"review""#, r#""scope":1.5"#)),
        ChangePolicyDecodeError::Json(JsonError::FloatingPoint { .. })
    ));
    let mut bytes = GOOD.as_bytes().to_vec();
    let at = GOOD
        .find("review")
        .expect("the baseline carries a review verb");
    bytes[at] = 0xff;
    assert!(matches!(
        PolicyTable::decode(&bytes).expect_err("invalid UTF-8 must not decode"),
        ChangePolicyDecodeError::Json(JsonError::NotUtf8 { .. })
    ));
}

#[test]
fn key_order_and_escape_spelling_do_not_change_the_decoded_table() {
    // The reader is liberal about spelling and the writer has exactly one, so a
    // re-encoded document is the canonical one — which is what makes the identity a
    // function of meaning rather than of formatting (ID7).
    // `\\u0073` is `s`: a legal spelling of the key `scope`, and not the canonical one.
    let escaped = GOOD.replace(r#""scope":"review""#, r#""\u0073cope":"review""#);
    assert_ne!(escaped, GOOD);
    let table = PolicyTable::decode(escaped.as_bytes()).expect("an escaped key is the same key");
    assert_eq!(table.to_artifact_bytes(), GOOD.as_bytes());
    let spaced = format!("  {}  ", GOOD.replace(',', " , "));
    assert_ne!(spaced, GOOD);
    assert_eq!(
        PolicyTable::decode(spaced.as_bytes())
            .expect("insignificant whitespace")
            .to_artifact_bytes(),
        GOOD.as_bytes()
    );
    // And a table whose keys arrive in reverse order is the same table.
    let mut pairs: Vec<String> = PolicyTable::decode(GOOD.as_bytes())
        .expect("baseline")
        .iter()
        .map(|(field, verb)| format!(r#""{}":"{}""#, field.wire(), verb.wire()))
        .collect();
    pairs.reverse();
    let reversed = format!("{{{}}}", pairs.join(","));
    assert_ne!(reversed, GOOD);
    assert_eq!(
        PolicyTable::decode(reversed.as_bytes())
            .expect("JSON objects are unordered")
            .to_artifact_bytes(),
        GOOD.as_bytes()
    );
}

#[test]
fn every_rejection_renders_a_message_that_names_its_cause() {
    let cases = [
        GOOD.replace(
            r#""properties":"locked""#,
            r#""properties":"proof-required""#,
        ),
        GOOD.replace(r#""properties":"locked""#, r#""claims":"locked""#),
        GOOD.replace(r#""properties":"locked""#, r#""properties":"no-decrease""#),
        GOOD.replace(r#""assurance":"no-downgrade","#, ""),
        GOOD.replace(r#""scope":"review""#, r#""scope":7"#),
        format!("{GOOD}{GOOD}"),
    ];
    for case in cases {
        let message = error(&case).to_string();
        assert!(
            message.len() > 20,
            "the message {message:?} is too thin to act on"
        );
    }
    // And the well-formedness errors render too, including the W6 matrix they cite.
    let inapplicable = ChangePolicyError::InapplicableVerb {
        field: PolicyField::Properties,
        verb: PolicyVerb::NoDecrease,
    }
    .to_string();
    assert!(inapplicable.contains("no-decrease"), "{inapplicable}");
    assert!(inapplicable.contains("properties"), "{inapplicable}");
    assert!(inapplicable.contains("unlocked"), "{inapplicable}");
}
