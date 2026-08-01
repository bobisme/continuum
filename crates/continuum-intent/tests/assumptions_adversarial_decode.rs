//! Adversarial decode: every malformed assumption artifact is a typed rejection
//! (PR-4 / IMPL-02, bn-136f).
//!
//! # Why this file exists
//!
//! This is the `assumptions` field group's half of the obligation
//! `tests/adversarial_decode.rs` states for `properties`:
//!
//! > A reader that encounters an unrecognized token in any of those vocabularies
//! > MUST fail closed: it MUST reject the contract […] Forward compatibility is
//! > achieved by rejecting, never by ignoring.
//! >
//! > — RFC 0037, "Versioning and revision"
//!
//! An assumption arrives from the same untrusted bundle a claim does (INV-016),
//! so the same trust-boundary obligation applies: reject, with a type, and never
//! panic. Each test below asserts a *specific* error, not merely `is_err()`.
//!
//! # A note on the amplification tests
//!
//! `assumptions[].expression` reuses [`continuum_intent::property::PropertyExpression`]'s
//! CPNF-1 normalization, so it inherits the same amplification hazard a bare
//! `iff`/`leads_to` tower creates on the way to N1 expansion. Every input below
//! that probes `MAX_DEPTH`/`MAX_NODES` is built by nesting on ONE side only —
//! never by `format!` embedding the accumulator twice — so the authored text
//! stays linear (kilobyte-scale) even though the post-rewrite tree would not be,
//! per this bone's amplification-safety rule. A prior test in this crate family
//! OOM-killed the host by doubling an accumulator string per level; nothing here
//! repeats that mistake.

use continuum_intent::assumptions::{
    Assumption, AssumptionClassification, AssumptionDecodeError, AssumptionError, AssumptionSet,
    FidelityProfile,
};
use continuum_intent::ast::AstError;
use continuum_intent::canonical_json::{JsonError, MAX_DEPTH};

/// A well-formed assumption, in canonical artifact form: the CPNF-1 normal form
/// of the schema's own worked example
/// (`notes/plan/schemas/examples/intent-contract.example.json`, `assumptions[0]`,
/// `always (SyncCompleted implies stable_recovery)`). Every case below is a
/// mutation of this string.
///
/// N1 eliminates `implies(p, q)` into `or(not p, q)`; N4 then orders the two
/// operands by their own N8 encodings, and `{"args":[],"kind":"predicate",…}`
/// sorts before `{"kind":"not",…}` (`a` < `k`), so the predicate comes first.
const GOOD: &str = concat!(
    r#"{"classification":"storage","expression":{"ast":{"kind":"always","operand":{"#,
    r#""kind":"or","operands":[{"args":[],"kind":"predicate","name":"stable_recovery"},"#,
    r#"{"kind":"not","operand":{"kind":"action","modality":"occurs","name":"SyncCompleted"}}]}},"#,
    r#""fragment":"Finite","normal_form":"cpnf-1","#,
    r#""source":"always (SyncCompleted implies stable_recovery)"},"#,
    r#""fidelity_profile":"contractual","id":"A-storage"}"#,
);

fn decode(text: &str) -> Result<Assumption, AssumptionDecodeError> {
    Assumption::decode(text.as_bytes())
}

fn error(text: &str) -> AssumptionDecodeError {
    decode(text).expect_err("a malformed artifact must not decode")
}

#[test]
fn the_baseline_is_well_formed() {
    let assumption = decode(GOOD).expect("the baseline decodes");
    assert_eq!(
        assumption.classification(),
        Some(AssumptionClassification::Storage)
    );
    assert_eq!(
        assumption.fidelity_profile(),
        Some(FidelityProfile::Contractual)
    );
}

// --- round trip and canonical spelling ---------------------------------------------------

#[test]
fn a_wellformed_assumption_round_trips_through_its_artifact_form() {
    let assumption = decode(GOOD).expect("decodes");
    let reencoded = assumption.to_artifact_bytes();
    let redecoded = Assumption::decode(&reencoded).expect("re-decodes");
    assert_eq!(assumption, redecoded);
    assert_eq!(reencoded, assumption.to_artifact_bytes());
}

#[test]
fn reordering_object_keys_is_accepted_and_normalized_to_one_spelling() {
    // Object keys are reordered at every level; the `operands` ARRAY order is
    // left untouched, because array order is significant JSON and — unlike key
    // order — a declared `normal_form: "cpnf-1"` requires the decoded AST to
    // already be in N4's order, not merely equivalent to it after re-sorting.
    let reordered = concat!(
        r#"{"id":"A-storage","fidelity_profile":"contractual","classification":"storage","#,
        r#""expression":{"source":"always (SyncCompleted implies stable_recovery)","#,
        r#""normal_form":"cpnf-1","fragment":"Finite","ast":{"operand":{"operands":["#,
        r#"{"kind":"predicate","args":[],"name":"stable_recovery"},{"operand":{"#,
        r#""modality":"occurs","kind":"action","name":"SyncCompleted"},"kind":"not"}],"#,
        r#""kind":"or"},"kind":"always"}}}"#,
    );
    let assumption = decode(reordered).expect("key order is not part of the document");
    assert_eq!(assumption.to_artifact_bytes(), GOOD.as_bytes());
    assert_eq!(assumption, decode(GOOD).expect("baseline"));
    assert_eq!(
        assumption.identity(),
        decode(GOOD).expect("baseline").identity()
    );
}

#[test]
fn reordering_assumptions_in_a_set_does_not_change_the_sets_identity() {
    let a = GOOD.replace(r#""id":"A-storage""#, r#""id":"A""#);
    let b = GOOD.replace(r#""id":"A-storage""#, r#""id":"B""#);
    let one = AssumptionSet::decode(format!("[{a},{b}]").as_bytes()).expect("decodes");
    let other = AssumptionSet::decode(format!("[{b},{a}]").as_bytes()).expect("decodes");
    assert_eq!(one.identity(), other.identity());
    assert_eq!(one.to_artifact_bytes(), other.to_artifact_bytes());
}

#[test]
fn declaring_or_undeclaring_classification_and_fidelity_profile_each_move_the_identity() {
    let bare = GOOD.replace(r#""classification":"storage","#, "");
    let bare = bare.replace(r#","fidelity_profile":"contractual""#, "");
    let with_classification_only = GOOD.replace(r#","fidelity_profile":"contractual""#, "");
    let baseline = decode(GOOD).expect("baseline");
    let bare = decode(&bare).expect("an assumption with neither declared decodes");
    let partial = decode(&with_classification_only).expect("decodes");
    // Three distinct declaredness states, three distinct identities: RFC 0037
    // "a change from absent to present, or the reverse, is a declaredness change
    // and classifies incomparable […] never unchanged."
    assert_ne!(baseline.identity(), bare.identity());
    assert_ne!(baseline.identity(), partial.identity());
    assert_ne!(bare.identity(), partial.identity());
}

// --- unknown tags in every closed vocabulary this group touches -------------------------

#[test]
fn an_unknown_classification_is_rejected() {
    assert_eq!(
        error(&GOOD.replace(
            r#""classification":"storage""#,
            r#""classification":"disk""#
        )),
        AssumptionDecodeError::UnknownToken {
            field: "assumptions[].classification",
            token: "disk".to_owned()
        }
    );
    // Case matters: the wire tokens are lowercase.
    assert_eq!(
        error(&GOOD.replace(
            r#""classification":"storage""#,
            r#""classification":"Storage""#
        )),
        AssumptionDecodeError::UnknownToken {
            field: "assumptions[].classification",
            token: "Storage".to_owned()
        }
    );
}

#[test]
fn an_unknown_fidelity_profile_is_rejected() {
    assert_eq!(
        error(&GOOD.replace(
            r#""fidelity_profile":"contractual""#,
            r#""fidelity_profile":"best-effort""#
        )),
        AssumptionDecodeError::UnknownToken {
            field: "assumptions[].fidelity_profile",
            token: "best-effort".to_owned()
        }
    );
}

#[test]
fn the_property_ast_vocabularies_fail_closed_too() {
    // The reused property-AST decoder must reject exactly what `property.rs`'s
    // rejects: an unrecognized formula kind, never treated as an opaque atom.
    assert_eq!(
        error(&GOOD.replace(r#""kind":"always""#, r#""kind":"next""#)),
        AssumptionDecodeError::UnknownToken {
            field: "formula.kind",
            token: "next".to_owned()
        }
    );
    assert_eq!(
        error(&GOOD.replace(r#""fragment":"Finite""#, r#""fragment":"finite""#)),
        AssumptionDecodeError::UnknownToken {
            field: "expression.fragment",
            token: "finite".to_owned()
        }
    );
    assert_eq!(
        error(&GOOD.replace(r#""normal_form":"cpnf-1""#, r#""normal_form":"cpnf-2""#)),
        AssumptionDecodeError::UnknownToken {
            field: "expression.normal_form",
            token: "cpnf-2".to_owned()
        }
    );
    assert_eq!(
        error(&GOOD.replace(r#""modality":"occurs""#, r#""modality":"fires""#)),
        AssumptionDecodeError::UnknownToken {
            field: "action.modality",
            token: "fires".to_owned()
        }
    );
}

#[test]
fn an_unknown_field_is_rejected_at_every_level() {
    assert_eq!(
        error(&GOOD.replace(r#""id":"A-storage""#, r#""hidden":true,"id":"A-storage""#)),
        AssumptionDecodeError::UnknownField {
            field: "assumptions[]",
            key: "hidden".to_owned()
        }
    );
    assert_eq!(
        error(&GOOD.replace(
            r#""fragment":"Finite""#,
            r#""confidence":0,"fragment":"Finite""#
        )),
        AssumptionDecodeError::UnknownField {
            field: "expression",
            key: "confidence".to_owned()
        }
    );
    // ID2 excludes `$comment`, but `additionalProperties: false` leaves it
    // nowhere to sit — rejected, not silently dropped.
    assert_eq!(
        error(&GOOD.replace(r#""id":"A-storage""#, r#""$comment":"x","id":"A-storage""#)),
        AssumptionDecodeError::UnknownField {
            field: "assumptions[]",
            key: "$comment".to_owned()
        }
    );
}

// --- duplicate keys, trailing bytes, and the JSON layer ---------------------------------

#[test]
fn a_duplicate_key_is_rejected_rather_than_resolved_by_arrival_order() {
    let doubled = GOOD.replace(
        r#""classification":"storage""#,
        r#""classification":"storage","classification":"network""#,
    );
    assert!(matches!(
        error(&doubled),
        AssumptionDecodeError::Json(JsonError::DuplicateKey { .. })
    ));
}

#[test]
fn trailing_bytes_are_rejected() {
    assert_eq!(
        error(&format!("{GOOD}{GOOD}")),
        AssumptionDecodeError::Json(JsonError::TrailingBytes { at: GOOD.len() })
    );
    decode(&format!("{GOOD} ")).expect("trailing whitespace alone is insignificant");
    assert!(matches!(
        error(&format!("{GOOD} null")),
        AssumptionDecodeError::Json(JsonError::TrailingBytes { .. })
    ));
}

#[test]
fn a_float_literal_is_rejected_rather_than_rounded() {
    let with_literal = GOOD.replace(
        r#"{"args":[],"kind":"predicate","name":"stable_recovery"}"#,
        r#"{"kind":"compare","op":"eq","left":{"kind":"literal","value":4},"right":{"kind":"literal","value":4}}"#,
    );
    assert!(matches!(
        error(&with_literal.replace(r#""value":4"#, r#""value":4.5"#)),
        AssumptionDecodeError::Json(JsonError::FloatingPoint { .. })
    ));
}

// --- truncation sweep ---------------------------------------------------------------------

#[test]
fn every_truncation_is_a_typed_error_and_never_a_panic() {
    for cut in 1..GOOD.len() {
        let truncated = &GOOD[..cut];
        let error = decode(truncated).expect_err("a truncated artifact must not decode");
        assert!(
            matches!(
                error,
                AssumptionDecodeError::Json(_)
                    | AssumptionDecodeError::MissingField { .. }
                    | AssumptionDecodeError::TypeMismatch { .. }
            ),
            "truncation at {cut} produced {error:?}"
        );
    }
}

#[test]
fn a_truncated_assumption_set_is_rejected_rather_than_partially_decoded() {
    let set = format!("[{GOOD},{GOOD}]");
    for cut in 1..set.len() {
        if AssumptionSet::decode(&set.as_bytes()[..cut]).is_ok() {
            assert_eq!(
                cut,
                set.len(),
                "a truncation at {cut} decoded into an assumption set"
            );
        }
    }
}

// --- shape and type violations -----------------------------------------------------------

#[test]
fn a_missing_required_field_names_the_field_it_is_missing() {
    assert_eq!(
        error(&GOOD.replace(r#","id":"A-storage""#, "")),
        AssumptionDecodeError::MissingField {
            field: "assumptions[].id"
        }
    );
    assert_eq!(
        error(r#"{"id":"A-storage"}"#),
        AssumptionDecodeError::MissingField {
            field: "assumptions[].expression"
        }
    );
}

#[test]
fn a_wrong_json_type_names_what_was_expected_and_what_arrived() {
    assert_eq!(
        error(&GOOD.replace(r#""id":"A-storage""#, r#""id":7"#)),
        AssumptionDecodeError::TypeMismatch {
            field: "assumptions[].id",
            expected: "string",
            found: "integer"
        }
    );
    assert_eq!(
        error(&GOOD.replace(r#""classification":"storage""#, r#""classification":7"#)),
        AssumptionDecodeError::TypeMismatch {
            field: "assumptions[].classification",
            expected: "string",
            found: "integer"
        }
    );
    assert_eq!(
        error(&GOOD.replace(
            r#""fidelity_profile":"contractual""#,
            r#""fidelity_profile":["contractual"]"#
        )),
        AssumptionDecodeError::TypeMismatch {
            field: "assumptions[].fidelity_profile",
            expected: "string",
            found: "array"
        }
    );
    // Neither optional field admits `null`: the schema types both as plain
    // string enums, and there is no meaning `null` could stand for.
    assert_eq!(
        error(&GOOD.replace(r#""classification":"storage""#, r#""classification":null"#)),
        AssumptionDecodeError::TypeMismatch {
            field: "assumptions[].classification",
            expected: "string",
            found: "null"
        }
    );
    assert!(matches!(
        AssumptionSet::decode(GOOD.as_bytes()),
        Err(AssumptionDecodeError::TypeMismatch {
            field: "assumptions",
            expected: "array",
            ..
        })
    ));
}

#[test]
fn an_empty_assumption_id_is_rejected() {
    assert_eq!(
        error(&GOOD.replace(r#""id":"A-storage""#, r#""id":"""#)),
        AssumptionDecodeError::Assumption(AssumptionError::EmptyUnitKey)
    );
}

#[test]
fn w1_rejects_a_duplicate_id_on_the_way_in() {
    let duplicate = format!("[{GOOD},{GOOD}]");
    assert_eq!(
        AssumptionSet::decode(duplicate.as_bytes()),
        Err(AssumptionDecodeError::Assumption(
            AssumptionError::DuplicateUnitKey {
                unit: "A-storage".to_owned()
            }
        ))
    );
    // Unlike claims, the empty array is legal: no `minItems` on `assumptions`.
    assert!(AssumptionSet::decode(b"[]").is_ok());
}

// --- the semantic check this decoder is responsible for ----------------------------------

#[test]
fn a_false_normal_form_declaration_is_rejected() {
    // Force a genuine authoring-form AST under a declared normal form: a `gt`
    // comparison re-normalizes to `lt` with swapped operands (N6), so declaring
    // `cpnf-1` over the authored `gt` shape is a false declaration.
    let with_gt = GOOD.replace(
        r#"{"args":[],"kind":"predicate","name":"stable_recovery"}"#,
        r#"{"kind":"compare","op":"gt","left":{"kind":"literal","value":1},"right":{"kind":"literal","value":0}}"#,
    );
    assert_eq!(error(&with_gt), AssumptionDecodeError::FalseNormalForm);
    let undeclared = with_gt.replace(r#""normal_form":"cpnf-1","#, "");
    let assumption = decode(&undeclared).expect("an undeclared authoring form is legal");
    let encoded = String::from_utf8(assumption.to_artifact_bytes()).expect("utf-8");
    assert!(encoded.contains(r#""op":"lt""#), "{encoded}");
    assert!(!encoded.contains(r#""op":"gt""#), "{encoded}");
}

// --- resource bounds, linear in the authored size (never doubled) ------------------------

#[test]
fn a_deeply_nested_expression_is_a_typed_error_and_not_a_stack_overflow() {
    // Nested on one side only: `depth` levels of `always`, authored text linear
    // in `depth` (a few bytes per level), never doubled.
    let depth = MAX_DEPTH + 8;
    let mut ast = r#"{"args":[],"kind":"predicate","name":"p"}"#.to_owned();
    for _ in 0..depth {
        ast = format!(r#"{{"kind":"always","operand":{ast}}}"#);
    }
    let assumption = format!(r#"{{"expression":{{"ast":{ast}}},"id":"deep"}}"#);
    let error = decode(&assumption).expect_err("a document past the depth bound must not decode");
    assert!(
        matches!(
            error,
            AssumptionDecodeError::Json(JsonError::TooDeep { .. })
                | AssumptionDecodeError::Ast(AstError::TooDeep { .. })
        ),
        "{error:?}"
    );
}

#[test]
fn an_expanding_bi_implication_tower_is_rejected_before_it_is_expanded() {
    // The same amplification shape `property.rs`'s own test uses: N1 duplicates
    // both sides of an `iff`, so a tower of them nested on ONE side keeps the
    // *authored* text linear (about 1.4 KB at 20 levels) while the post-N1 size
    // would double per level — the bound is checked before the rewrite runs.
    let leaf = r#"{"args":[],"kind":"predicate","name":"p"}"#;
    let mut ast = leaf.to_owned();
    for _ in 0..20 {
        ast = format!(r#"{{"kind":"iff","left":{leaf},"right":{ast}}}"#);
    }
    assert!(
        ast.len() < 4096,
        "authored size must stay kilobyte-scale, was {}",
        ast.len()
    );
    let assumption = format!(r#"{{"expression":{{"ast":{ast}}},"id":"bomb"}}"#);
    let error = decode(&assumption).expect_err("an amplification attack must not decode");
    assert!(
        matches!(
            error,
            AssumptionDecodeError::Normalize(_)
                | AssumptionDecodeError::Json(JsonError::TooDeep { .. })
                | AssumptionDecodeError::Ast(AstError::TooDeep { .. })
        ),
        "{error:?}"
    );
}

// --- every rejection is reportable ---------------------------------------------------------

#[test]
fn every_rejection_renders_a_message_that_names_its_cause() {
    let cases = [
        GOOD.replace(
            r#""classification":"storage""#,
            r#""classification":"disk""#,
        ),
        GOOD.replace(r#","id":"A-storage""#, String::new().as_str()),
        GOOD.replace(r#""id":"A-storage""#, r#""id":7"#),
        format!("{GOOD}{GOOD}"),
    ];
    for case in cases {
        let message = error(&case).to_string();
        assert!(!message.is_empty());
        assert!(
            message.len() > 15,
            "the message {message:?} is too thin to act on"
        );
    }
}
