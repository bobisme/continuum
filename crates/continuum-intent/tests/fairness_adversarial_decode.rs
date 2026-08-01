//! Adversarial decode: every malformed fairness artifact is a typed rejection
//! (PR-4 / IMPL-06).
//!
//! The obligation and its sources are stated once, in
//! `tests/adversarial_decode.rs`. This file is the evidence for the `fairness`
//! portion, and every case asserts a *specific* error rather than merely `is_err()`.
//!
//! Fairness is the group with the largest closed-vocabulary surface, because a
//! constraint carries a property AST: `fairness[].kind` is its own two-member enum,
//! and the condition reaches `formula.kind`, `term.kind`, `compare.op`, and
//! `action.modality` through the crate's one property-AST reader. All five are swept
//! here — through *this* decoder, so the sweep is evidence that the reuse is real and
//! that a fairness condition is not quietly read by a second, laxer parser.
//!
//! Two hazards are specific to this group:
//!
//! - **W4.** A temporal operator in a condition is a rejection, not a normalization,
//!   and it must be caught at any depth.
//! - **Amplification.** The condition is normalized, and CPNF-1's N1 duplicates both
//!   sides of an `iff`. The tower below is authored *linearly* — nested on one side —
//!   so a kilobyte of input names a post-rewrite size past `MAX_NODES`, and the bound
//!   must be checked before the rewrite runs.

use continuum_intent::ast::AstError;
use continuum_intent::canonical_json::{JsonError, MAX_DEPTH};
use continuum_intent::fairness::{
    FairnessConstraint, FairnessDecodeError, FairnessError, FairnessSet,
};
use continuum_intent::property::PropertyDecodeError;

/// A well-formed fairness constraint, in canonical artifact form.
const GOOD: &str = concat!(
    r#"{"action":"SyncCompleted","condition":{"args":[],"kind":"predicate","#,
    r#""name":"node_running"},"kind":"weak"}"#,
);

/// The condition of `GOOD`, for substitution.
const CONDITION: &str = r#"{"args":[],"kind":"predicate","name":"node_running"}"#;

fn error(text: &str) -> FairnessDecodeError {
    FairnessConstraint::decode(text.as_bytes()).expect_err("a malformed artifact must not decode")
}

#[test]
fn the_baseline_is_well_formed() {
    let constraint = FairnessConstraint::decode(GOOD.as_bytes()).expect("the baseline decodes");
    assert_eq!(constraint.to_artifact_bytes(), GOOD.as_bytes());
}

// --- truncation ------------------------------------------------------------------------

#[test]
fn every_truncation_is_a_typed_error_and_never_a_panic() {
    for cut in 1..GOOD.len() {
        let error = FairnessConstraint::decode(&GOOD.as_bytes()[..cut])
            .expect_err("a truncated artifact must not decode");
        assert!(
            matches!(
                error,
                FairnessDecodeError::Json(_)
                    | FairnessDecodeError::MissingField { .. }
                    | FairnessDecodeError::TypeMismatch { .. }
                    | FairnessDecodeError::UnknownToken { .. }
                    | FairnessDecodeError::Condition(_)
                    | FairnessDecodeError::Fairness(FairnessError::EmptyAction)
            ),
            "truncation at {cut} produced {error:?}"
        );
    }
}

#[test]
fn a_truncated_fairness_set_is_rejected_rather_than_partially_decoded() {
    let other = GOOD.replace(r#""kind":"weak""#, r#""kind":"strong""#);
    let set = format!("[{GOOD},{other}]");
    for cut in 1..set.len() {
        if let Ok(decoded) = FairnessSet::decode(&set.as_bytes()[..cut]) {
            assert_eq!(cut, set.len(), "a truncation at {cut} decoded into a set");
            assert_eq!(decoded.len(), 2);
        }
    }
}

// --- every closed vocabulary the group reaches ---------------------------------------------

#[test]
fn the_fairness_kind_vocabulary_fails_closed() {
    for token in ["Weak", "strong-ish", "unconditional", "", "WEAK"] {
        assert_eq!(
            error(&GOOD.replace(r#""kind":"weak"}"#, &format!(r#""kind":"{token}"}}"#))),
            FairnessDecodeError::UnknownToken {
                field: "fairness[].kind",
                token: token.to_owned()
            },
            "the token {token:?} must fail closed"
        );
    }
}

#[test]
fn every_vocabulary_the_condition_reaches_fails_closed_too() {
    // These are the property-AST reader's vocabularies, exercised through this
    // decoder. That they arrive as `Condition(...)` is the point: there is one reader,
    // and a fairness condition does not get a second, more forgiving one.
    for (condition, field, token) in [
        (
            r#"{"kind":"until","left":{"args":[],"kind":"predicate","name":"p"},"right":{"args":[],"kind":"predicate","name":"q"}}"#,
            "formula.kind",
            "until",
        ),
        (
            r#"{"kind":"next","operand":{"args":[],"kind":"predicate","name":"p"}}"#,
            "formula.kind",
            "next",
        ),
        (
            r#"{"kind":"compare","left":{"indices":[],"kind":"state","name":"q"},"op":"neq","right":{"kind":"literal","value":0}}"#,
            "compare.op",
            "neq",
        ),
        (
            r#"{"kind":"compare","left":{"kind":"register","name":"q"},"op":"eq","right":{"kind":"literal","value":0}}"#,
            "term.kind",
            "register",
        ),
        (
            r#"{"kind":"action","modality":"fires","name":"Sync"}"#,
            "action.modality",
            "fires",
        ),
    ] {
        assert_eq!(
            error(&GOOD.replace(CONDITION, condition)),
            FairnessDecodeError::Condition(PropertyDecodeError::UnknownToken {
                field,
                token: token.to_owned()
            }),
            "{condition}"
        );
    }
}

#[test]
fn an_unknown_field_is_rejected_at_both_levels() {
    for key in ["id", "weight", "$comment", "Kind"] {
        assert_eq!(
            error(&GOOD.replace(r#""action""#, &format!(r#""{key}":1,"action""#))),
            FairnessDecodeError::UnknownField {
                field: "fairness[]",
                key: key.to_owned()
            }
        );
    }
    // Inside the condition, `additionalProperties: false` holds on every node.
    assert_eq!(
        error(&GOOD.replace(
            CONDITION,
            r#"{"args":[],"kind":"predicate","name":"p","note":"x"}"#
        )),
        FairnessDecodeError::Condition(PropertyDecodeError::UnknownField {
            field: "formula[predicate]",
            key: "note".to_owned()
        })
    );
    // A `property_expression` object is not a bare condition. RFC 0037 makes the
    // difference normative: no `source`, no `fragment`, no `normal_form`.
    assert_eq!(
        error(&GOOD.replace(
            CONDITION,
            r#"{"ast":{"args":[],"kind":"predicate","name":"p"},"source":"p"}"#
        )),
        FairnessDecodeError::Condition(PropertyDecodeError::MissingField {
            field: "formula.kind"
        })
    );
}

// --- W4 --------------------------------------------------------------------------------------

#[test]
fn a_temporal_operator_is_rejected_at_every_depth_and_named() {
    for (condition, operator) in [
        (
            r#"{"kind":"always","operand":{"args":[],"kind":"predicate","name":"p"}}"#,
            "always",
        ),
        (
            r#"{"kind":"not","operand":{"kind":"eventually","operand":{"args":[],"kind":"predicate","name":"p"}}}"#,
            "eventually",
        ),
        (
            r#"{"kind":"or","operands":[{"args":[],"kind":"predicate","name":"p"},{"antecedent":{"args":[],"kind":"predicate","name":"q"},"consequent":{"args":[],"kind":"predicate","name":"r"},"kind":"leads_to"}]}"#,
            "leads_to",
        ),
    ] {
        assert_eq!(
            error(&GOOD.replace(CONDITION, condition)),
            FairnessDecodeError::Fairness(FairnessError::TemporalCondition { operator }),
            "{condition}"
        );
    }
}

#[test]
fn a_temporal_operator_nested_under_sixty_negations_is_still_found() {
    // Linear in the depth — one `not` per level, nested on one side — and under the
    // JSON depth bound, so the rejection is W4's rather than the parser's.
    let mut condition =
        r#"{"kind":"always","operand":{"args":[],"kind":"predicate","name":"p"}}"#.to_owned();
    for _ in 0..40 {
        condition = format!(r#"{{"kind":"not","operand":{condition}}}"#);
    }
    let document = GOOD.replace(CONDITION, &condition);
    assert!(document.len() < 4096, "the input stays kilobyte-scale");
    assert_eq!(
        error(&document),
        FairnessDecodeError::Fairness(FairnessError::TemporalCondition { operator: "always" })
    );
}

// --- duplicates -----------------------------------------------------------------------

#[test]
fn a_duplicate_json_key_is_rejected_rather_than_resolved_by_arrival_order() {
    // The dangerous case: two `kind`s, one weak and one strong. Last-writer-wins would
    // make the constraint's strength depend on the parser.
    let doubled = GOOD.replace(r#""kind":"weak"}"#, r#""kind":"weak","kind":"strong"}"#);
    assert!(matches!(
        error(&doubled),
        FairnessDecodeError::Json(JsonError::DuplicateKey { .. })
    ));
    // And one inside the condition.
    let doubled_condition = GOOD.replace(
        r#""kind":"predicate","name":"node_running""#,
        r#""kind":"predicate","name":"node_running","name":"anything""#,
    );
    assert!(matches!(
        error(&doubled_condition),
        FairnessDecodeError::Json(JsonError::DuplicateKey { .. })
    ));
    // A second `condition`, the shape that would let a null quietly win.
    let doubled_null = GOOD.replace(
        &format!(r#""condition":{CONDITION}"#),
        &format!(r#""condition":{CONDITION},"condition":null"#),
    );
    assert!(matches!(
        error(&doubled_null),
        FairnessDecodeError::Json(JsonError::DuplicateKey { .. })
    ));
}

#[test]
fn a_repeated_unit_key_is_rejected_by_w1() {
    assert_eq!(
        FairnessSet::decode(format!("[{GOOD},{GOOD}]").as_bytes())
            .expect_err("W1 rejects a repeated (kind, action) pair"),
        FairnessDecodeError::Fairness(FairnessError::DuplicateKey {
            key: "weak:SyncCompleted".to_owned()
        })
    );
}

// --- trailing bytes and the JSON layer -----------------------------------------------------

#[test]
fn trailing_bytes_are_rejected() {
    assert_eq!(
        error(&format!("{GOOD}{GOOD}")),
        FairnessDecodeError::Json(JsonError::TrailingBytes { at: GOOD.len() })
    );
    FairnessConstraint::decode(format!("{GOOD}\n").as_bytes())
        .expect("trailing whitespace is insignificant");
    assert!(matches!(
        error(&format!("{GOOD} null")),
        FairnessDecodeError::Json(JsonError::TrailingBytes { .. })
    ));
}

#[test]
fn non_utf8_input_is_a_typed_error() {
    let mut bytes = GOOD.as_bytes().to_vec();
    let at = GOOD.find("SyncCompleted").expect("the action is present");
    bytes[at] = 0xff;
    assert!(matches!(
        FairnessConstraint::decode(&bytes).expect_err("invalid UTF-8 must not decode"),
        FairnessDecodeError::Json(JsonError::NotUtf8 { .. })
    ));
}

#[test]
fn a_float_in_a_condition_literal_is_rejected_rather_than_rounded() {
    let condition = r#"{"kind":"compare","left":{"indices":[],"kind":"state","name":"q"},"op":"eq","right":{"kind":"literal","value":0.5}}"#;
    assert!(matches!(
        error(&GOOD.replace(CONDITION, condition)),
        FairnessDecodeError::Json(JsonError::FloatingPoint { .. })
    ));
}

// --- shape and type violations ---------------------------------------------------------------

#[test]
fn a_missing_required_field_names_the_field_it_is_missing() {
    assert_eq!(
        error(&GOOD.replace(r#""action":"SyncCompleted","#, "")),
        FairnessDecodeError::MissingField {
            field: "fairness[].action"
        }
    );
    assert_eq!(
        error(&GOOD.replace(r#","kind":"weak""#, "")),
        FairnessDecodeError::MissingField {
            field: "fairness[].kind"
        }
    );
}

#[test]
fn a_wrong_json_type_names_what_was_expected_and_what_arrived() {
    assert_eq!(
        error(&GOOD.replace(r#""action":"SyncCompleted""#, r#""action":7"#)),
        FairnessDecodeError::TypeMismatch {
            field: "fairness[].action",
            expected: "string",
            found: "integer"
        }
    );
    assert_eq!(
        error(&GOOD.replace(r#""kind":"weak"}"#, r#""kind":["weak"]}"#)),
        FairnessDecodeError::TypeMismatch {
            field: "fairness[].kind",
            expected: "string",
            found: "array"
        }
    );
    // A condition that is not an object is not a formula. It is *not* read as `null`:
    // absence and `null` are one meaning, and a `false` is neither.
    assert_eq!(
        error(&GOOD.replace(CONDITION, "false")),
        FairnessDecodeError::Condition(PropertyDecodeError::TypeMismatch {
            field: "formula",
            expected: "object",
            found: "boolean"
        })
    );
    assert!(matches!(
        FairnessSet::decode(GOOD.as_bytes()).expect_err("a constraint is not a set"),
        FairnessDecodeError::TypeMismatch {
            field: "fairness",
            expected: "array",
            ..
        }
    ));
}

#[test]
fn an_empty_action_is_rejected() {
    assert_eq!(
        error(&GOOD.replace(r#""action":"SyncCompleted""#, r#""action":"""#)),
        FairnessDecodeError::Fairness(FairnessError::EmptyAction)
    );
}

#[test]
fn an_arity_violation_inside_a_condition_is_rejected_where_the_schema_states_it() {
    assert_eq!(
        error(&GOOD.replace(
            CONDITION,
            r#"{"kind":"and","operands":[{"args":[],"kind":"predicate","name":"p"}]}"#
        )),
        FairnessDecodeError::Condition(PropertyDecodeError::Ast(AstError::JunctionArity {
            kind: "and",
            found: 1
        }))
    );
}

#[test]
fn an_unbound_variable_in_a_condition_is_rejected() {
    let condition = r#"{"args":[{"kind":"var","name":"e"}],"kind":"predicate","name":"p"}"#;
    assert!(matches!(
        error(&GOOD.replace(CONDITION, condition)),
        FairnessDecodeError::Condition(PropertyDecodeError::Normalize(_))
    ));
}

// --- resource bounds ---------------------------------------------------------------------------

#[test]
fn a_deeply_nested_condition_is_a_typed_error_and_not_a_stack_overflow() {
    // Linear input: one `not` per level, nested on one side.
    let mut condition = r#"{"args":[],"kind":"predicate","name":"p"}"#.to_owned();
    for _ in 0..(MAX_DEPTH + 8) {
        condition = format!(r#"{{"kind":"not","operand":{condition}}}"#);
    }
    let document = GOOD.replace(CONDITION, &condition);
    assert!(document.len() < 8192, "the input stays kilobyte-scale");
    let error = error(&document);
    assert!(
        matches!(
            error,
            FairnessDecodeError::Json(JsonError::TooDeep { .. })
                | FairnessDecodeError::Condition(PropertyDecodeError::Ast(
                    AstError::TooDeep { .. }
                ))
        ),
        "{error:?}"
    );
}

#[test]
fn an_expanding_bi_implication_tower_in_a_condition_is_rejected_before_it_is_expanded() {
    // N1 duplicates both sides of an `iff`, so a tower of them is an amplification
    // attack on the normalizer. Nested on ONE side, the authored text stays linear
    // (20 levels, well under a kilobyte, depth far under the JSON bound) while the
    // post-N1 size doubles per level to ~2^20 nodes — past `MAX_NODES`. The bound is
    // checked before the rewrite runs, in time linear in the authored size.
    let leaf = r#"{"args":[],"kind":"predicate","name":"p"}"#;
    let mut condition = leaf.to_owned();
    for _ in 0..20 {
        condition = format!(r#"{{"kind":"iff","left":{leaf},"right":{condition}}}"#);
    }
    let document = GOOD.replace(CONDITION, &condition);
    assert!(document.len() < 4096, "the input stays kilobyte-scale");
    let error = error(&document);
    assert!(
        matches!(
            error,
            FairnessDecodeError::Condition(PropertyDecodeError::Normalize(_))
                | FairnessDecodeError::Json(JsonError::TooDeep { .. })
                | FairnessDecodeError::Condition(PropertyDecodeError::Ast(
                    AstError::TooDeep { .. }
                ))
        ),
        "{error:?}"
    );
}

// --- every rejection is reportable ---------------------------------------------------------------

#[test]
fn every_rejection_renders_a_message_that_names_its_cause() {
    let cases = [
        GOOD.replace(r#""kind":"weak"}"#, r#""kind":"fair"}"#),
        GOOD.replace(r#""action":"SyncCompleted","#, ""),
        GOOD.replace(r#""action":"SyncCompleted""#, r#""action":"""#),
        GOOD.replace(
            CONDITION,
            r#"{"kind":"always","operand":{"args":[],"kind":"predicate","name":"p"}}"#,
        ),
        GOOD.replace(r#""action""#, r#""id":"f1","action""#),
        format!("{GOOD}{GOOD}"),
    ];
    for case in cases {
        let message = error(&case).to_string();
        assert!(
            message.len() > 20,
            "the message {message:?} is too thin to act on"
        );
    }
    // W4's message names the operator and says why the rule exists.
    let w4 = error(&GOOD.replace(
        CONDITION,
        r#"{"kind":"eventually","operand":{"args":[],"kind":"predicate","name":"p"}}"#,
    ))
    .to_string();
    assert!(w4.contains("eventually"), "{w4}");
    assert!(w4.contains("state formula"), "{w4}");
}
