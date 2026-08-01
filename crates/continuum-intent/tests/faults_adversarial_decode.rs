//! Adversarial decode: every malformed fault-model artifact is a typed rejection
//! (PR-4 / IMPL-05).
//!
//! The obligation and its sources are stated once, in
//! `tests/adversarial_decode.rs`. This file is the evidence for the `fault_model`
//! portion, structured the same way, and every case asserts a *specific* error rather
//! than merely `is_err()`.
//!
//! `enabled` is a closed six-member vocabulary, and RFC 0037 lists it by name among
//! the vocabularies whose membership cannot move without "a `schema_epoch` advance
//! *and* an explicit revision of this RFC". So the sweep below runs every plausible
//! seventh class through the decoder: a reader that carried one through would be
//! reporting a fault model it does not implement as one it does, which is a *stronger*
//! claim about the environment than the contract makes.
//!
//! Nothing here is allowed to panic, allocate unboundedly, or recurse past the
//! declared depth. Every input is linear in its nesting depth.

use continuum_intent::canonical_json::{JsonError, MAX_DEPTH};
use continuum_intent::faults::{FaultClass, FaultDecodeError, FaultError, FaultModel};

/// A well-formed fault model, in canonical artifact form.
const GOOD: &str = r#"{"enabled":["crash","recovery"],"profiles":["storage-posix-v1"]}"#;

fn error(text: &str) -> FaultDecodeError {
    FaultModel::decode(text.as_bytes()).expect_err("a malformed artifact must not decode")
}

#[test]
fn the_baseline_is_well_formed() {
    let model = FaultModel::decode(GOOD.as_bytes()).expect("the baseline decodes");
    assert_eq!(model.to_artifact_bytes(), GOOD.as_bytes());
}

// --- truncation ------------------------------------------------------------------------

#[test]
fn every_truncation_is_a_typed_error_and_never_a_panic() {
    for cut in 1..GOOD.len() {
        let error = FaultModel::decode(&GOOD.as_bytes()[..cut])
            .expect_err("a truncated artifact must not decode");
        assert!(
            matches!(
                error,
                FaultDecodeError::Json(_)
                    | FaultDecodeError::MissingField { .. }
                    | FaultDecodeError::TypeMismatch { .. }
                    | FaultDecodeError::UnknownToken { .. }
                    | FaultDecodeError::Fault(FaultError::EmptyProfileName)
            ),
            "truncation at {cut} produced {error:?}"
        );
    }
}

#[test]
fn a_truncation_never_yields_a_smaller_fault_model() {
    // The failure that would matter: a cut that lands after `"crash"` and before
    // `"recovery"` must not decode into a one-class model. I4 — "it is never stored
    // partially" — is what forbids it, and the JSON layer is what enforces it.
    for cut in 1..GOOD.len() {
        if let Ok(model) = FaultModel::decode(&GOOD.as_bytes()[..cut]) {
            assert_eq!(cut, GOOD.len(), "a truncation at {cut} decoded");
            assert_eq!(model.enabled().len(), 2);
        }
    }
}

// --- the closed vocabulary ----------------------------------------------------------------

#[test]
fn every_token_outside_the_closed_six_fails_closed() {
    for token in [
        "byzantine",  // a seventh class
        "omission",   // ditto
        "Crash",      // case matters: the wire tokens are lowercase
        "crash ",     // and so does whitespace
        "",           // the empty token
        "partition2", // a versioned member
    ] {
        let mutated = GOOD.replace(r#""crash""#, &format!(r#""{token}""#));
        assert_eq!(
            error(&mutated),
            FaultDecodeError::UnknownToken {
                field: "fault_model.enabled",
                token: token.to_owned()
            },
            "the token {token:?} must fail closed"
        );
        assert_eq!(FaultClass::from_wire(token), None);
    }
}

#[test]
fn a_profile_name_is_not_a_closed_vocabulary_and_is_not_treated_as_one() {
    // `profiles` is open — it names whatever the snapshot's domain packs declare — so
    // an unfamiliar name decodes and is left to W8, which needs the packs. The
    // asymmetry is deliberate and is the reason `check_profiles` exists.
    let model = FaultModel::decode(br#"{"enabled":[],"profiles":["some-pack-nobody-here-knows"]}"#)
        .expect("an unfamiliar profile name is not a decode failure");
    assert_eq!(model.profiles().len(), 1);
}

#[test]
fn an_unknown_field_is_rejected_because_the_schema_forbids_it() {
    for key in ["envelope", "disabled", "$comment", "Enabled"] {
        let mutated = GOOD.replace(r#""enabled""#, &format!(r#""{key}":[],"enabled""#));
        assert_eq!(
            error(&mutated),
            FaultDecodeError::UnknownField {
                field: "fault_model",
                key: key.to_owned()
            }
        );
    }
}

// --- duplicates -----------------------------------------------------------------------

#[test]
fn a_duplicate_json_key_is_rejected_rather_than_resolved_by_arrival_order() {
    // The dangerous case: two `enabled` arrays, one honest and one empty.
    let doubled = GOOD.replace(
        r#""enabled":["crash","recovery"]"#,
        r#""enabled":["crash","recovery"],"enabled":[]"#,
    );
    assert!(matches!(
        error(&doubled),
        FaultDecodeError::Json(JsonError::DuplicateKey { .. })
    ));
}

#[test]
fn a_repeated_member_is_rejected_rather_than_deduplicated() {
    // `uniqueItems: true` on both arrays.
    assert_eq!(
        error(&GOOD.replace(r#"["crash","recovery"]"#, r#"["crash","crash"]"#)),
        FaultDecodeError::Fault(FaultError::DuplicateClass {
            class: FaultClass::Crash
        })
    );
    assert_eq!(
        error(&GOOD.replace(
            r#"["storage-posix-v1"]"#,
            r#"["storage-posix-v1","storage-posix-v1"]"#
        )),
        FaultDecodeError::Fault(FaultError::DuplicateProfile {
            name: "storage-posix-v1".to_owned()
        })
    );
}

// --- trailing bytes and the JSON layer -----------------------------------------------------

#[test]
fn trailing_bytes_are_rejected() {
    assert_eq!(
        error(&format!("{GOOD}{GOOD}")),
        FaultDecodeError::Json(JsonError::TrailingBytes { at: GOOD.len() })
    );
    FaultModel::decode(format!("{GOOD}  ").as_bytes())
        .expect("trailing whitespace is insignificant");
    assert!(matches!(
        error(&format!("{GOOD} 0")),
        FaultDecodeError::Json(JsonError::TrailingBytes { .. })
    ));
}

#[test]
fn non_utf8_input_is_a_typed_error() {
    let mut bytes = GOOD.as_bytes().to_vec();
    let at = GOOD.find("storage").expect("the profile name is present");
    bytes[at] = 0xff;
    assert!(matches!(
        FaultModel::decode(&bytes).expect_err("invalid UTF-8 must not decode"),
        FaultDecodeError::Json(JsonError::NotUtf8 { .. })
    ));
}

// --- shape and type violations ---------------------------------------------------------------

#[test]
fn a_missing_required_field_names_the_field_it_is_missing() {
    assert_eq!(
        error(r#"{"profiles":["storage-posix-v1"]}"#),
        FaultDecodeError::MissingField {
            field: "fault_model.enabled"
        }
    );
    // `profiles` is optional and its absence is the empty set, not an error.
    FaultModel::decode(br#"{"enabled":["crash"]}"#).expect("profiles is optional");
}

#[test]
fn a_wrong_json_type_names_what_was_expected_and_what_arrived() {
    assert_eq!(
        error(&GOOD.replace(r#""enabled":["crash","recovery"]"#, r#""enabled":"crash""#)),
        FaultDecodeError::TypeMismatch {
            field: "fault_model.enabled",
            expected: "array",
            found: "string"
        }
    );
    assert_eq!(
        error(&GOOD.replace(r#"["crash","recovery"]"#, r#"[1]"#)),
        FaultDecodeError::TypeMismatch {
            field: "fault_model.enabled",
            expected: "string",
            found: "integer"
        }
    );
    assert_eq!(
        error(&GOOD.replace(r#""profiles":["storage-posix-v1"]"#, r#""profiles":null"#)),
        FaultDecodeError::TypeMismatch {
            field: "fault_model.profiles",
            expected: "array",
            found: "null"
        }
    );
    assert_eq!(
        error(&GOOD.replace(
            r#"["storage-posix-v1"]"#,
            r#"[{"name":"storage-posix-v1"}]"#
        )),
        FaultDecodeError::TypeMismatch {
            field: "fault_model.profiles",
            expected: "string",
            found: "object"
        }
    );
    assert!(matches!(
        FaultModel::decode(b"[]").expect_err("an array is not a fault model"),
        FaultDecodeError::TypeMismatch {
            field: "fault_model",
            expected: "object",
            ..
        }
    ));
}

#[test]
fn an_empty_profile_name_is_rejected() {
    assert_eq!(
        error(&GOOD.replace(r#""storage-posix-v1""#, r#""""#)),
        FaultDecodeError::Fault(FaultError::EmptyProfileName)
    );
}

// --- resource bounds ---------------------------------------------------------------------------

#[test]
fn a_deeply_nested_document_is_a_typed_error_and_not_a_stack_overflow() {
    // A fault model has no recursive shape, so the only way to nest is to nest the
    // value of one of its two keys. The input is linear in the depth — one bracket
    // pair per level — and the parser rejects it before it recurses past the bound.
    let mut nested = "[]".to_owned();
    for _ in 0..(MAX_DEPTH + 8) {
        nested = format!("[{nested}]");
    }
    let document = format!(r#"{{"enabled":{nested}}}"#);
    assert!(document.len() < 4096, "the input stays kilobyte-scale");
    assert!(matches!(
        error(&document),
        FaultDecodeError::Json(JsonError::TooDeep { .. })
    ));
}

#[test]
fn a_repeated_class_in_a_long_array_is_still_caught() {
    // Linear in size; the duplicate check is over a set, not a quadratic scan.
    let mut members: Vec<String> = FaultClass::ALL
        .iter()
        .map(|class| format!(r#""{}""#, class.wire()))
        .collect();
    members.push(r#""crash""#.to_owned());
    let document = format!(r#"{{"enabled":[{}]}}"#, members.join(","));
    assert_eq!(
        error(&document),
        FaultDecodeError::Fault(FaultError::DuplicateClass {
            class: FaultClass::Crash
        })
    );
}

// --- every rejection is reportable ---------------------------------------------------------------

#[test]
fn every_rejection_renders_a_message_that_names_its_cause() {
    let cases = [
        GOOD.replace(r#""crash""#, r#""byzantine""#),
        GOOD.replace(r#""enabled""#, r#""envelope":[],"enabled""#),
        GOOD.replace(r#"["crash","recovery"]"#, r#"["crash","crash"]"#),
        GOOD.replace(r#""storage-posix-v1""#, r#""""#),
        r#"{"profiles":[]}"#.to_owned(),
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
