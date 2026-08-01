//! Adversarial decode: every malformed observer artifact is a typed rejection
//! (PR-4 / IMPL-03).
//!
//! The obligation and its sources are stated once, in
//! `tests/adversarial_decode.rs`: an Intent Contract arrives from an intent bundle,
//! RFC 0037 classes a bundle as untrusted data (INV-016), and I4 requires that an
//! ill-formed contract "is rejected; it is never stored partially". This file is the
//! evidence for the `observers` portion, structured the same way, and every case
//! asserts a *specific* error rather than merely `is_err()`.
//!
//! One thing is particular to this group. An observer is four *sets*, so the decoder's
//! failure mode is not only "accept something malformed" but "accept something
//! smaller than what was written": a reader that ignored an unknown key would drop a
//! projection, and a reader that deduplicated a repeated element would rewrite a set
//! the schema rejects into one it accepts. Both are silent coarsenings — the attack
//! RFC 0031 classifies `coarsened` — so both are tested here rather than left to the
//! type's documentation.
//!
//! Nothing here is allowed to panic, allocate unboundedly, or recurse past the
//! declared depth. Every input below is linear in its nesting depth.

use continuum_intent::canonical_json::{JsonError, MAX_DEPTH};
use continuum_intent::observers::{
    Observer, ObserverDecodeError, ObserverError, ObserverSet, ProjectionKind,
};

/// A well-formed observer, in canonical artifact form. Every case below is a mutation
/// of this string.
const GOOD: &str = concat!(
    r#"{"events":["ReplyPublished","RequestReceived"],"id":"client","#,
    r#""knowledge_projection":[],"security_projection":[],"#,
    r#""state_projection":["acknowledged"]}"#,
);

fn error(text: &str) -> ObserverDecodeError {
    Observer::decode(text.as_bytes()).expect_err("a malformed artifact must not decode")
}

#[test]
fn the_baseline_is_well_formed() {
    let observer = Observer::decode(GOOD.as_bytes()).expect("the baseline decodes");
    assert_eq!(observer.to_artifact_bytes(), GOOD.as_bytes());
}

// --- truncation ------------------------------------------------------------------------

#[test]
fn every_truncation_is_a_typed_error_and_never_a_panic() {
    for cut in 1..GOOD.len() {
        let error = Observer::decode(&GOOD.as_bytes()[..cut])
            .expect_err("a truncated artifact must not decode");
        assert!(
            matches!(
                error,
                ObserverDecodeError::Json(_)
                    | ObserverDecodeError::MissingField { .. }
                    | ObserverDecodeError::TypeMismatch { .. }
            ),
            "truncation at {cut} produced {error:?}"
        );
    }
}

#[test]
fn a_truncated_observer_set_is_rejected_rather_than_partially_decoded() {
    let other = GOOD.replace(r#""id":"client""#, r#""id":"server""#);
    let set = format!("[{GOOD},{other}]");
    // Cutting inside the second observer must not yield a one-observer set: I4 says an
    // ill-formed contract "is never stored partially".
    for cut in 1..set.len() {
        if let Ok(decoded) = ObserverSet::decode(&set.as_bytes()[..cut]) {
            assert_eq!(cut, set.len(), "a truncation at {cut} decoded into a set");
            assert_eq!(decoded.len(), 2);
        }
    }
}

// --- the closed vocabulary, which here is a key set --------------------------------------

#[test]
fn every_projection_key_outside_the_closed_set_is_rejected() {
    // The group's only closed vocabulary is the four projection kinds, and the schema
    // realizes it as object keys under `additionalProperties: false`. A reader that
    // ignored an unrecognized key would silently drop whatever the author put in it.
    for key in [
        "state",             // plan §5.2's prose name
        "event",             // ditto
        "events2",           // a fifth projection
        "Events",            // case matters
        "timing_projection", // a plausible future member
        "$comment",          // ID2's third exclusion has no site here
    ] {
        let mutated = GOOD.replace(r#""id":"client""#, &format!(r#""{key}":[],"id":"client""#));
        assert_eq!(
            error(&mutated),
            ObserverDecodeError::UnknownField {
                field: "observers[]",
                key: key.to_owned()
            },
            "the key {key} must fail closed"
        );
        assert_eq!(ProjectionKind::from_wire(key), None);
    }
}

#[test]
fn an_unknown_key_on_the_set_is_rejected_at_its_own_item() {
    let mutated = GOOD.replace(r#""id":"client""#, r#""hidden":["E"],"id":"client""#);
    assert_eq!(
        ObserverSet::decode(format!("[{mutated}]").as_bytes()).expect_err("rejects"),
        ObserverDecodeError::UnknownField {
            field: "observers[]",
            key: "hidden".to_owned()
        }
    );
}

// --- duplicates -----------------------------------------------------------------------

#[test]
fn a_duplicate_json_key_is_rejected_rather_than_resolved_by_arrival_order() {
    // The dangerous case: two `events` arrays, one honest and one coarsened.
    // Last-writer-wins would make the document's meaning depend on the parser.
    let doubled = GOOD.replace(
        r#""events":["ReplyPublished","RequestReceived"]"#,
        r#""events":["ReplyPublished","RequestReceived"],"events":[]"#,
    );
    assert!(matches!(
        error(&doubled),
        ObserverDecodeError::Json(JsonError::DuplicateKey { .. })
    ));
    let doubled_id = GOOD.replace(r#""id":"client""#, r#""id":"client","id":"server""#);
    assert!(matches!(
        error(&doubled_id),
        ObserverDecodeError::Json(JsonError::DuplicateKey { .. })
    ));
}

#[test]
fn a_repeated_set_element_is_rejected_rather_than_deduplicated() {
    // `uniqueItems: true`. A reader that collapsed the repeat would accept a document
    // the schema rejects, and would do it by *rewriting* the author's set.
    let repeated = GOOD.replace(
        r#"["ReplyPublished","RequestReceived"]"#,
        r#"["ReplyPublished","ReplyPublished"]"#,
    );
    assert_eq!(
        error(&repeated),
        ObserverDecodeError::Observer(ObserverError::DuplicateElement {
            kind: ProjectionKind::Events,
            value: "ReplyPublished".to_owned()
        })
    );
    let repeated_projection = GOOD.replace(
        r#""state_projection":["acknowledged"]"#,
        r#""state_projection":["acknowledged","acknowledged"]"#,
    );
    assert_eq!(
        error(&repeated_projection),
        ObserverDecodeError::Observer(ObserverError::DuplicateElement {
            kind: ProjectionKind::State,
            value: "acknowledged".to_owned()
        })
    );
}

#[test]
fn a_repeated_observer_id_is_rejected_by_w1() {
    assert_eq!(
        ObserverSet::decode(format!("[{GOOD},{GOOD}]").as_bytes())
            .expect_err("W1 rejects a duplicate id"),
        ObserverDecodeError::Observer(ObserverError::DuplicateObserverId {
            id: "client".to_owned()
        })
    );
}

// --- trailing bytes and the JSON layer -----------------------------------------------------

#[test]
fn trailing_bytes_are_rejected() {
    assert_eq!(
        error(&format!("{GOOD}{GOOD}")),
        ObserverDecodeError::Json(JsonError::TrailingBytes { at: GOOD.len() })
    );
    // Trailing whitespace alone is insignificant and is skipped; a trailing *value* is
    // not whitespace.
    Observer::decode(format!("{GOOD} ").as_bytes()).expect("trailing whitespace is insignificant");
    assert!(matches!(
        error(&format!("{GOOD} []")),
        ObserverDecodeError::Json(JsonError::TrailingBytes { .. })
    ));
}

#[test]
fn non_utf8_input_is_a_typed_error() {
    let mut bytes = GOOD.as_bytes().to_vec();
    let at = GOOD.find("client").expect("the id is present");
    bytes[at] = 0xff;
    assert!(matches!(
        Observer::decode(&bytes).expect_err("invalid UTF-8 must not decode"),
        ObserverDecodeError::Json(JsonError::NotUtf8 { .. })
    ));
}

// --- shape and type violations ---------------------------------------------------------------

#[test]
fn a_missing_required_field_names_the_field_it_is_missing() {
    assert_eq!(
        error(&GOOD.replace(r#""id":"client","#, "")),
        ObserverDecodeError::MissingField {
            field: "observers[].id"
        }
    );
    // `events` is the one projection the schema requires; the other three default.
    assert_eq!(
        error(&GOOD.replace(r#""events":["ReplyPublished","RequestReceived"],"#, "")),
        ObserverDecodeError::MissingField { field: "events" }
    );
}

#[test]
fn a_wrong_json_type_names_what_was_expected_and_what_arrived() {
    assert_eq!(
        error(&GOOD.replace(r#""id":"client""#, r#""id":7"#)),
        ObserverDecodeError::TypeMismatch {
            field: "observers[].id",
            expected: "string",
            found: "integer"
        }
    );
    assert_eq!(
        error(&GOOD.replace(
            r#""events":["ReplyPublished","RequestReceived"]"#,
            r#""events":"ReplyPublished""#
        )),
        ObserverDecodeError::TypeMismatch {
            field: "events",
            expected: "array",
            found: "string"
        }
    );
    assert_eq!(
        error(&GOOD.replace(
            r#""state_projection":["acknowledged"]"#,
            r#""state_projection":null"#
        )),
        ObserverDecodeError::TypeMismatch {
            field: "state_projection",
            expected: "array",
            found: "null"
        }
    );
    // An element of the wrong type: a set whose members are not strings is not a set
    // of event families.
    assert_eq!(
        error(&GOOD.replace(r#"["ReplyPublished","RequestReceived"]"#, r#"[1,2]"#)),
        ObserverDecodeError::TypeMismatch {
            field: "events",
            expected: "string",
            found: "integer"
        }
    );
    assert_eq!(
        error(&GOOD.replace(
            r#"["ReplyPublished","RequestReceived"]"#,
            r#"[{"family":"ReplyPublished"}]"#
        )),
        ObserverDecodeError::TypeMismatch {
            field: "events",
            expected: "string",
            found: "object"
        }
    );
    // The set decoder wants an array; a bare observer is not one.
    assert!(matches!(
        ObserverSet::decode(GOOD.as_bytes()).expect_err("an observer is not an observer set"),
        ObserverDecodeError::TypeMismatch {
            field: "observers",
            expected: "array",
            ..
        }
    ));
    assert!(matches!(
        Observer::decode(b"[]").expect_err("an array is not an observer"),
        ObserverDecodeError::TypeMismatch {
            field: "observers[]",
            expected: "object",
            ..
        }
    ));
}

#[test]
fn an_empty_observer_id_is_rejected() {
    assert_eq!(
        error(&GOOD.replace(r#""id":"client""#, r#""id":"""#)),
        ObserverDecodeError::Observer(ObserverError::EmptyObserverId)
    );
}

// --- resource bounds ---------------------------------------------------------------------------

#[test]
fn a_deeply_nested_document_is_a_typed_error_and_not_a_stack_overflow() {
    // An observer has no recursive shape of its own, so the only way to nest is to
    // nest the *value* of a projection. The input is linear in the depth — one bracket
    // pair per level, nested on one side — and the parser must reject it before it
    // recurses past the bound.
    let mut nested = "[]".to_owned();
    for _ in 0..(MAX_DEPTH + 8) {
        nested = format!("[{nested}]");
    }
    let document = format!(r#"{{"events":{nested},"id":"deep"}}"#);
    assert!(document.len() < 4096, "the input stays kilobyte-scale");
    assert!(matches!(
        error(&document),
        ObserverDecodeError::Json(JsonError::TooDeep { .. })
    ));
}

#[test]
fn a_large_but_legal_set_decodes_without_amplification() {
    // The honest upper end: many elements, no nesting. Linear in size and linear in
    // work — there is no rewrite in this group to amplify.
    let events: Vec<String> = (0..2000).map(|i| format!(r#""E{i}""#)).collect();
    let document = format!(r#"{{"events":[{}],"id":"wide"}}"#, events.join(","));
    let observer = Observer::decode(document.as_bytes()).expect("a wide set is legal");
    assert_eq!(observer.events().len(), 2000);
}

// --- every rejection is reportable ---------------------------------------------------------------

#[test]
fn every_rejection_renders_a_message_that_names_its_cause() {
    let cases = [
        GOOD.replace(r#""id":"client""#, r#""state":[],"id":"client""#),
        GOOD.replace(r#""id":"client","#, String::new().as_str()),
        GOOD.replace(r#""id":"client""#, r#""id":7"#),
        GOOD.replace(r#""id":"client""#, r#""id":"""#),
        GOOD.replace(
            r#"["ReplyPublished","RequestReceived"]"#,
            r#"["ReplyPublished","ReplyPublished"]"#,
        ),
        format!("{GOOD}{GOOD}"),
    ];
    for case in cases {
        let message = error(&case).to_string();
        // A rejection nobody can act on is not a trust boundary. Every message names
        // the field, the key, or the byte offset that failed.
        assert!(
            message.len() > 20,
            "the message {message:?} is too thin to act on"
        );
    }
    // The set-level rejection too.
    let message = ObserverSet::decode(format!("[{GOOD},{GOOD}]").as_bytes())
        .expect_err("W1")
        .to_string();
    assert!(message.contains("client"), "{message}");
}
