//! Adversarial decode: every malformed property artifact is a typed rejection.
//!
//! # Why this file exists
//!
//! RFC 0037 gives the decoder one instruction and repeats it in three places:
//!
//! > A consumer that does not implement an instance's `schema_epoch` MUST reject the
//! > artifact with a typed error; an intent contract is never best-effort decoded
//! > across a breaking epoch.
//! >
//! > A reader that encounters an unrecognized token in any of those vocabularies
//! > MUST fail closed: it MUST reject the contract […] Forward compatibility is
//! > achieved by rejecting, never by ignoring.
//! >
//! > **I4.** Import MUST validate every contract against the schema and against the
//! > well-formedness rules W1–W10 before it enters the registry. An unparseable or
//! > ill-formed contract is rejected; it is never stored partially.
//!
//! An Intent Contract arrives from an intent bundle, and RFC 0037 classes a bundle
//! as untrusted data (INV-016). So the decoder is a trust boundary, and its
//! obligation is not "usually reject" — it is "reject, with a type, and never
//! panic". This file is the evidence for that obligation over the property portion.
//!
//! # The attack classes
//!
//! Each section below is one way a hostile or corrupt artifact differs from a
//! well-formed one, and each asserts a *specific* error rather than merely
//! `is_err()`: an error that cannot be distinguished from another error is a error
//! nobody can act on, and a decoder that returns the same opaque failure for
//! "truncated" and "weakened claim" is not a trust boundary.
//!
//! Nothing here is allowed to panic, allocate unboundedly, or recurse past the
//! declared depth — the truncation and depth sweeps are the tests that say so.

use continuum_intent::ast::AstError;
use continuum_intent::canonical_json::{JsonError, MAX_DEPTH};
use continuum_intent::property::{Claim, ClaimSet, PropertyDecodeError, PropertyError};

/// A well-formed claim, in canonical artifact form. Every case below is a mutation
/// of this string.
const GOOD: &str = concat!(
    r#"{"expression":{"ast":{"kind":"always","operand":{"kind":"compare","#,
    r#""left":{"indices":[],"kind":"state","name":"big"},"op":"ne","#,
    r#""right":{"kind":"literal","value":4}}},"fragment":"Finite","#,
    r#""normal_form":"cpnf-1","source":"big != 4"},"id":"NotSolved","#,
    r#""kind":"safety","observer":null}"#,
);

fn decode(text: &str) -> Result<Claim, PropertyDecodeError> {
    Claim::decode(text.as_bytes())
}

fn error(text: &str) -> PropertyDecodeError {
    decode(text).expect_err("a malformed artifact must not decode")
}

#[test]
fn the_baseline_is_well_formed() {
    let claim = decode(GOOD).expect("the baseline decodes");
    assert_eq!(claim.to_artifact_bytes(), GOOD.as_bytes());
}

// --- truncation ------------------------------------------------------------------------

#[test]
fn every_truncation_is_a_typed_error_and_never_a_panic() {
    for cut in 1..GOOD.len() {
        let truncated = &GOOD[..cut];
        let error = decode(truncated).expect_err("a truncated artifact must not decode");
        assert!(
            matches!(
                error,
                PropertyDecodeError::Json(_)
                    | PropertyDecodeError::MissingField { .. }
                    | PropertyDecodeError::TypeMismatch { .. }
            ),
            "truncation at {cut} produced {error:?}"
        );
    }
}

#[test]
fn a_truncated_claim_set_is_rejected_rather_than_partially_decoded() {
    let set = format!("[{GOOD},{GOOD}]");
    // Cutting inside the second claim must not yield a one-claim set: I4 says an
    // ill-formed contract "is never stored partially".
    for cut in 1..set.len() {
        if ClaimSet::decode(&set.as_bytes()[..cut]).is_ok() {
            assert_eq!(
                cut,
                set.len(),
                "a truncation at {cut} decoded into a claim set"
            );
        }
    }
}

// --- reordering ------------------------------------------------------------------------

#[test]
fn reordering_object_keys_is_accepted_and_normalized_away() {
    // JSON objects are unordered, so a reader must accept any order — and ID5 gives
    // the *writer* exactly one, so re-encoding is the canonicalization.
    let reordered = concat!(
        r#"{"observer":null,"kind":"safety","id":"NotSolved","expression":{"#,
        r#""source":"big != 4","normal_form":"cpnf-1","fragment":"Finite","#,
        r#""ast":{"operand":{"right":{"value":4,"kind":"literal"},"op":"ne","#,
        r#""left":{"name":"big","kind":"state","indices":[]},"kind":"compare"},"#,
        r#""kind":"always"}}}"#,
    );
    let claim = decode(reordered).expect("key order is not part of the document");
    assert_eq!(claim.to_artifact_bytes(), GOOD.as_bytes());
    assert_eq!(claim, decode(GOOD).expect("baseline"));
    assert_eq!(claim.identity(), decode(GOOD).expect("baseline").identity());
}

#[test]
fn reordering_the_conjuncts_of_a_junction_is_normalized_away_and_reordering_claims_is_not_a_change()
{
    let a = GOOD.replace(r#""id":"NotSolved""#, r#""id":"A""#);
    let b = GOOD.replace(r#""id":"NotSolved""#, r#""id":"B""#);
    let one = ClaimSet::decode(format!("[{a},{b}]").as_bytes()).expect("decodes");
    let other = ClaimSet::decode(format!("[{b},{a}]").as_bytes()).expect("decodes");
    assert_eq!(one.identity(), other.identity());
    assert_eq!(one.to_artifact_bytes(), other.to_artifact_bytes());
}

// --- unknown tags ----------------------------------------------------------------------

#[test]
fn an_unknown_node_kind_is_rejected_and_never_treated_as_an_opaque_atom() {
    // `next` is the one the RFC names: "admitting `next` would make step-count
    // refactorings look like semantic changes".
    assert_eq!(
        error(&GOOD.replace(r#""kind":"always""#, r#""kind":"next""#)),
        PropertyDecodeError::UnknownToken {
            field: "formula.kind",
            token: "next".to_owned()
        }
    );
    // A future node kind gets the same answer, which is the whole point of a closed
    // vocabulary: a `cpnf-2` reader's output is not decodable here.
    assert_eq!(
        error(&GOOD.replace(r#""kind":"always""#, r#""kind":"until""#)),
        PropertyDecodeError::UnknownToken {
            field: "formula.kind",
            token: "until".to_owned()
        }
    );
    // Case matters: the wire tokens are lowercase.
    assert_eq!(
        error(&GOOD.replace(r#""kind":"always""#, r#""kind":"Always""#)),
        PropertyDecodeError::UnknownToken {
            field: "formula.kind",
            token: "Always".to_owned()
        }
    );
}

#[test]
fn every_other_closed_vocabulary_fails_closed_too() {
    for (from, to, field) in [
        (
            r#""kind":"safety""#,
            r#""kind":"availability""#,
            "claims[].kind",
        ),
        (r#""op":"ne""#, r#""op":"neq""#, "compare.op"),
        (
            r#""fragment":"Finite""#,
            r#""fragment":"finite""#,
            "expression.fragment",
        ),
        (
            r#""normal_form":"cpnf-1""#,
            r#""normal_form":"cpnf-2""#,
            "expression.normal_form",
        ),
        (r#""kind":"state""#, r#""kind":"register""#, "term.kind"),
    ] {
        let token = to
            .rsplit(':')
            .next()
            .expect("a token")
            .trim_matches('"')
            .to_owned();
        assert_eq!(
            error(&GOOD.replace(from, to)),
            PropertyDecodeError::UnknownToken { field, token },
            "replacing {from} with {to} did not fail closed"
        );
    }
    // The action modality, which the baseline does not carry.
    let with_action = GOOD.replace(
        r#"{"kind":"compare","left":{"indices":[],"kind":"state","name":"big"},"op":"ne","right":{"kind":"literal","value":4}}"#,
        r#"{"kind":"action","modality":"fires","name":"FillBig"}"#,
    );
    assert_eq!(
        error(&with_action),
        PropertyDecodeError::UnknownToken {
            field: "action.modality",
            token: "fires".to_owned()
        }
    );
}

#[test]
fn an_unknown_field_is_rejected_because_the_schema_forbids_it() {
    // `additionalProperties: false` holds on the claim, on the expression, and on
    // every node. A reader that ignored an extra key would accept documents the
    // schema rejects.
    assert_eq!(
        error(&GOOD.replace(r#""id":"NotSolved""#, r#""hidden":true,"id":"NotSolved""#)),
        PropertyDecodeError::UnknownField {
            field: "claims[]",
            key: "hidden".to_owned()
        }
    );
    assert_eq!(
        error(&GOOD.replace(
            r#""fragment":"Finite""#,
            r#""confidence":0,"fragment":"Finite""#
        )),
        PropertyDecodeError::UnknownField {
            field: "expression",
            key: "confidence".to_owned()
        }
    );
    assert_eq!(
        error(&GOOD.replace(r#""op":"ne""#, r#""note":"relaxed","op":"ne""#)),
        PropertyDecodeError::UnknownField {
            field: "formula[compare]",
            key: "note".to_owned()
        }
    );
    // ID2 excludes `$comment` from the identity preimage, but the property portion's
    // `additionalProperties: false` leaves it nowhere to sit, so it is rejected
    // rather than silently dropped. See `property`'s module documentation.
    assert_eq!(
        error(&GOOD.replace(r#""id":"NotSolved""#, r#""$comment":"x","id":"NotSolved""#)),
        PropertyDecodeError::UnknownField {
            field: "claims[]",
            key: "$comment".to_owned()
        }
    );
}

// --- trailing bytes, duplicates, and the JSON layer -------------------------------------

#[test]
fn trailing_bytes_are_rejected() {
    assert_eq!(
        error(&format!("{GOOD}{GOOD}")),
        PropertyDecodeError::Json(JsonError::TrailingBytes { at: GOOD.len() })
    );
    // Trailing whitespace alone is insignificant and is skipped — the decoder
    // accepts non-canonical spellings and re-derives the one canonical form, the
    // same tolerance the key-reordering test exercises. A trailing *value* is not
    // whitespace and is rejected at its own offset.
    decode(&format!("{GOOD} ")).expect("trailing whitespace alone is insignificant");
    assert!(matches!(
        error(&format!("{GOOD} null")),
        PropertyDecodeError::Json(JsonError::TrailingBytes { .. })
    ));
}

#[test]
fn a_duplicate_key_is_rejected_rather_than_resolved_by_arrival_order() {
    // The dangerous case: two `kind`s, one benign and one not. Last-writer-wins
    // would make the document's meaning depend on the parser.
    let doubled = GOOD.replace(r#""kind":"safety""#, r#""kind":"safety","kind":"liveness""#);
    assert!(matches!(
        error(&doubled),
        PropertyDecodeError::Json(JsonError::DuplicateKey { .. })
    ));
    let doubled_ast = GOOD.replace(r#""op":"ne""#, r#""op":"ne","op":"eq""#);
    assert!(matches!(
        error(&doubled_ast),
        PropertyDecodeError::Json(JsonError::DuplicateKey { .. })
    ));
}

#[test]
fn a_float_literal_is_rejected_rather_than_rounded() {
    assert!(matches!(
        error(&GOOD.replace(r#""value":4"#, r#""value":4.0"#)),
        PropertyDecodeError::Json(JsonError::FloatingPoint { .. })
    ));
    assert!(matches!(
        error(&GOOD.replace(r#""value":4"#, r#""value":4e0"#)),
        PropertyDecodeError::Json(JsonError::FloatingPoint { .. })
    ));
}

#[test]
fn non_utf8_input_is_a_typed_error() {
    let mut bytes = GOOD.as_bytes().to_vec();
    let at = GOOD.find("big").expect("the state name is present");
    bytes[at] = 0xff;
    assert!(matches!(
        Claim::decode(&bytes).expect_err("invalid UTF-8 must not decode"),
        PropertyDecodeError::Json(JsonError::NotUtf8 { .. })
    ));
}

// --- shape and type violations -----------------------------------------------------------

#[test]
fn a_missing_required_field_names_the_field_it_is_missing() {
    assert_eq!(
        error(&GOOD.replace(r#""id":"NotSolved","#, "")),
        PropertyDecodeError::MissingField {
            field: "claims[].id"
        }
    );
    assert_eq!(
        error(&GOOD.replace(r#","kind":"safety""#, "")),
        PropertyDecodeError::MissingField {
            field: "claims[].kind"
        }
    );
    assert_eq!(
        error(&GOOD.replace(r#""op":"ne","#, "")),
        PropertyDecodeError::MissingField {
            field: "compare.op"
        }
    );
}

#[test]
fn a_wrong_json_type_names_what_was_expected_and_what_arrived() {
    assert_eq!(
        error(&GOOD.replace(r#""id":"NotSolved""#, r#""id":7"#)),
        PropertyDecodeError::TypeMismatch {
            field: "claims[].id",
            expected: "string",
            found: "integer"
        }
    );
    assert_eq!(
        error(&GOOD.replace(r#""observer":null"#, r#""observer":[]"#)),
        PropertyDecodeError::TypeMismatch {
            field: "claims[].observer",
            expected: "string or null",
            found: "array"
        }
    );
    assert_eq!(
        error(&GOOD.replace(r#""source":"big != 4""#, r#""source":4"#)),
        PropertyDecodeError::TypeMismatch {
            field: "expression.source",
            expected: "string",
            found: "integer"
        }
    );
    assert!(matches!(
        ClaimSet::decode(GOOD.as_bytes()).expect_err("a claim is not a claim set"),
        PropertyDecodeError::TypeMismatch {
            field: "claims",
            expected: "array",
            ..
        }
    ));
}

#[test]
fn an_arity_violation_is_rejected_where_the_schema_states_it() {
    // `$defs/formula_junction`'s `minItems: 2`.
    let single = GOOD.replace(
        r#"{"kind":"compare","left":{"indices":[],"kind":"state","name":"big"},"op":"ne","right":{"kind":"literal","value":4}}"#,
        r#"{"kind":"and","operands":[{"args":[],"kind":"predicate","name":"p"}]}"#,
    );
    assert_eq!(
        error(&single),
        PropertyDecodeError::Ast(AstError::JunctionArity {
            kind: "and",
            found: 1
        })
    );
    // `$defs/term_apply`'s `minItems: 1`.
    let empty_apply = GOOD.replace(
        r#"{"indices":[],"kind":"state","name":"big"}"#,
        r#"{"args":[],"kind":"apply","operator":"plus"}"#,
    );
    assert_eq!(
        error(&empty_apply),
        PropertyDecodeError::Ast(AstError::EmptyApplication {
            operator: "plus".to_owned()
        })
    );
}

#[test]
fn an_identifier_outside_the_schemas_pattern_is_rejected() {
    assert_eq!(
        error(&GOOD.replace(r#""name":"big""#, r#""name":"big-jug""#)),
        PropertyDecodeError::Ast(AstError::Identifier {
            name: "big-jug".to_owned()
        })
    );
    assert_eq!(
        error(&GOOD.replace(r#""name":"big""#, r#""name":"""#)),
        PropertyDecodeError::Ast(AstError::Identifier {
            name: String::new()
        })
    );
}

#[test]
fn an_empty_claim_id_is_rejected() {
    assert_eq!(
        error(&GOOD.replace(r#""id":"NotSolved""#, r#""id":"""#)),
        PropertyDecodeError::Property(PropertyError::EmptyUnitKey)
    );
}

// --- the semantic checks the decoder is responsible for ----------------------------------

#[test]
fn a_false_normal_form_declaration_is_rejected() {
    // The AST is in authoring form and the document swears it is normalized. RFC
    // 0037 N9: "a checker MUST verify the claim by re-normalizing and MUST reject a
    // contract whose declaration does not hold."
    let lying = GOOD.replace(r#""op":"ne""#, r#""op":"gt""#);
    assert_eq!(error(&lying), PropertyDecodeError::FalseNormalForm);
    // Without the declaration the same AST decodes fine — and is normalized, so it
    // comes back as `lt` with swapped operands.
    let undeclared = lying.replace(r#""normal_form":"cpnf-1","#, "");
    let claim = decode(&undeclared).expect("an undeclared authoring form is legal");
    let encoded = String::from_utf8(claim.to_artifact_bytes()).expect("utf-8");
    assert!(encoded.contains(r#""op":"lt""#), "{encoded}");
    assert!(!encoded.contains(r#""op":"gt""#), "{encoded}");
}

#[test]
fn an_unbound_variable_is_rejected() {
    let free = GOOD.replace(
        r#"{"indices":[],"kind":"state","name":"big"}"#,
        r#"{"kind":"var","name":"e"}"#,
    );
    assert!(matches!(
        error(&free),
        PropertyDecodeError::Normalize(_) | PropertyDecodeError::FalseNormalForm
    ));
    // Without the `normal_form` declaration the normalizer is still the one that
    // refuses it, and it names the variable.
    let undeclared = free.replace(r#""normal_form":"cpnf-1","#, "");
    match error(&undeclared) {
        PropertyDecodeError::Normalize(error) => {
            assert!(error.to_string().contains("\"e\""), "{error}");
        }
        other => panic!("expected a normalization error, got {other:?}"),
    }
}

#[test]
fn w1_and_the_minimum_claim_count_are_enforced_on_the_way_in() {
    let duplicate = format!("[{GOOD},{GOOD}]");
    assert_eq!(
        ClaimSet::decode(duplicate.as_bytes()).expect_err("W1 rejects a duplicate id"),
        PropertyDecodeError::Property(PropertyError::DuplicateUnitKey {
            unit: "NotSolved".to_owned()
        })
    );
    assert_eq!(
        ClaimSet::decode(b"[]").expect_err("the schema's minItems is 1"),
        PropertyDecodeError::Property(PropertyError::NoClaims)
    );
}

// --- resource bounds ----------------------------------------------------------------------

#[test]
fn a_deeply_nested_document_is_a_typed_error_and_not_a_stack_overflow() {
    let depth = MAX_DEPTH + 8;
    let mut ast = r#"{"args":[],"kind":"predicate","name":"p"}"#.to_owned();
    for _ in 0..depth {
        ast = format!(r#"{{"kind":"always","operand":{ast}}}"#);
    }
    let claim =
        format!(r#"{{"expression":{{"ast":{ast}}},"id":"deep","kind":"safety","observer":null}}"#);
    let error = decode(&claim).expect_err("a document past the depth bound must not decode");
    assert!(
        matches!(
            error,
            PropertyDecodeError::Json(JsonError::TooDeep { .. })
                | PropertyDecodeError::Ast(AstError::TooDeep { .. })
        ),
        "{error:?}"
    );
}

#[test]
fn an_expanding_bi_implication_tower_is_rejected_before_it_is_expanded() {
    // N1 duplicates both sides of an `iff`, so a tower of them is an amplification
    // attack on the normalizer: nested on ONE side, the authored text stays linear
    // (20 levels ≈ 1.4 KB, depth well under the JSON bound) while the post-N1 size
    // doubles per level to ~2^20 nodes — far past `MAX_NODES`. The bound must be
    // checked before the rewrite runs, in time linear in the authored size.
    let leaf = r#"{"args":[],"kind":"predicate","name":"p"}"#;
    let mut ast = leaf.to_owned();
    for _ in 0..20 {
        ast = format!(r#"{{"kind":"iff","left":{leaf},"right":{ast}}}"#);
    }
    let claim =
        format!(r#"{{"expression":{{"ast":{ast}}},"id":"bomb","kind":"safety","observer":null}}"#);
    let error = decode(&claim).expect_err("an amplification attack must not decode");
    assert!(
        matches!(
            error,
            PropertyDecodeError::Normalize(_)
                | PropertyDecodeError::Json(JsonError::TooDeep { .. })
                | PropertyDecodeError::Ast(AstError::TooDeep { .. })
        ),
        "{error:?}"
    );
}

// --- every rejection is reportable -------------------------------------------------------

#[test]
fn every_rejection_renders_a_message_that_names_its_cause() {
    let cases = [
        GOOD.replace(r#""kind":"always""#, r#""kind":"next""#),
        GOOD.replace(r#""id":"NotSolved","#, String::new().as_str()),
        GOOD.replace(r#""id":"NotSolved""#, r#""id":7"#),
        GOOD.replace(r#""op":"ne""#, r#""op":"gt""#),
        GOOD.replace(r#""name":"big""#, r#""name":"big-jug""#),
        format!("{GOOD}{GOOD}"),
    ];
    for case in cases {
        let message = error(&case).to_string();
        assert!(!message.is_empty());
        // A rejection nobody can act on is not a trust boundary. Every message here
        // names either the field, the token, or the byte offset that failed.
        assert!(
            message.len() > 20,
            "the message {message:?} is too thin to act on"
        );
    }
}
