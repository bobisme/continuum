//! PR-11 / IMPL-01 evidence: the pack's answer header — target, verdict, assurance
//! (RFC 0028, "Required fields, reconciled with plan §6.2", the "target intent and
//! property" and "exact verdict and assurance envelope" rows; RFC 0028, "Verdict and
//! assurance").
//!
//! Linked as a separate crate (`continuum_context::*`, not `crate::*`), the same discipline
//! IMPL-03's own suite states for itself: this is evidence about the client's view of the
//! crate, not about its interior.
//!
//! | Clause | Test |
//! |---|---|
//! | the closed four-member `verdict` vocabulary matches the schema and fails closed on an unrecognized token | `the_verdict_vocabulary_is_closed_and_fails_closed` |
//! | the closed five-member `assurance.class` vocabulary matches the schema and carries RFC 0031's total order | `the_assurance_class_vocabulary_is_closed_and_carries_the_ratified_order` |
//! | a target names a class-checked `in_*` contract and a typed question in one of the RFC's two forms | `a_target_names_a_class_checked_contract_and_a_typed_question` |
//! | an `in_*` identity minted by `continuum-intent` drops straight into the slot | `an_intent_id_drops_straight_into_the_slot` |
//! | inconclusiveness is typed by construction, both on the way in and on the way out (INV-008) | `inconclusiveness_is_typed_by_construction` |
//! | the three field groups carry exactly one artifact handle between them | `the_answer_header_carries_exactly_one_artifact_handle` |
//! | the keys this bullet contributes are the schema's, and no others | `the_bullet_contributes_exactly_its_four_schema_keys` |
//! | two distinct headers never collide into one canonical encoding (anti-vacuity: the encoding is not constant) | `distinct_headers_have_distinct_canonical_encodings` |
//! | a fixed header is byte-identical across independent construction (docs/19 §7 determinism) | `identical_headers_are_byte_identical_across_independent_builds` |
//! | targets sort into one stable, content-derived order (`rule ordering.deterministic`) | `targets_sort_into_one_stable_content_derived_order` |
//! | every emitted fragment round-trips through the shared canonical-JSON reader with no drift | `every_emitted_fragment_round_trips_through_the_canonical_json_reader` |

use core::str::FromStr;

use continuum_context::assurance::Assurance;
use continuum_context::expansion::{Depth, ExpansionQuery, ExpansionQuestion, ExpansionRelation};
use continuum_context::pack::REQUIRED_KEYS;
use continuum_context::target::{Question, QuestionError, Target, TargetError};
use continuum_context::verdict::{Verdict, VerdictError};
use continuum_intent::canonical_json::Json;
use continuum_intent::contract::IntentId;
use continuum_value::assurance::{
    AssuranceDimension, AssuranceEnvelope, AssuranceLevel, DimensionEvidence, InconclusiveReason,
    UnsupportedReason,
};
use continuum_value::value::Name;
use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};

fn intent(identity: &str) -> ArtifactHandle {
    ArtifactHandle::new(ArtifactClass::IntentContract, identity).expect("well-formed in_* handle")
}

fn question(text: &str) -> Question {
    Question::compiled(text).expect("well-formed test question")
}

fn target(identity: &str, text: &str) -> Target {
    Target::new(intent(identity), question(text)).expect("an in_* handle is accepted")
}

fn envelope() -> AssuranceEnvelope {
    AssuranceEnvelope::all_unsupported(
        &UnsupportedReason::new("unattempted").expect("canonical token"),
    )
    .with(
        AssuranceDimension::Bounds,
        DimensionEvidence::produced("continuum-explicit", "faults=1, nodes=2")
            .expect("named producer"),
    )
    .with(
        AssuranceDimension::MemoryModel,
        DimensionEvidence::Unsupported(UnsupportedReason::sequential_consistency_only()),
    )
}

fn assurance() -> Assurance {
    Assurance::new(AssuranceLevel::Bounded, envelope())
}

#[test]
fn the_verdict_vocabulary_is_closed_and_fails_closed() {
    // Transcribed from `notes/plan/schemas/context-pack.schema.json`,
    // `properties.verdict.enum` (schema order); the IDL's `EvaluationVerdict`.
    const SCHEMA_ORDER: [&str; 4] = ["satisfied", "refuted", "deadlock", "inconclusive"];
    assert_eq!(
        Verdict::TOKENS,
        SCHEMA_ORDER,
        "the vocabulary must be exactly the schema's, in order"
    );

    // Every decided token round-trips (anti-vacuity: the parser is not a constant).
    for token in ["satisfied", "refuted", "deadlock"] {
        let verdict = Verdict::from_wire(token, None).expect("a declared token parses");
        assert_eq!(verdict.as_wire_str(), token);
        assert_eq!(verdict.inconclusive_reason(), None);
    }

    // A near-miss of a declared token is refused (fail closed, RFC 0028 "Versioning and
    // revision": "Forward compatibility is achieved by rejecting, never by ignoring").
    // `established` is the *claim-level* vocabulary's token and is deliberately in the
    // list: two closed sets that share a field name must not share members by accident.
    for bad in [
        "Satisfied",
        "SATISFIED",
        "refuted ",
        "deadlocked",
        "",
        "established",
        "verdict",
    ] {
        assert_eq!(
            Verdict::from_wire(bad, None),
            Err(VerdictError::UnknownVerdict),
            "{bad:?} must be refused"
        );
    }
}

#[test]
fn the_assurance_class_vocabulary_is_closed_and_carries_the_ratified_order() {
    // Transcribed from `notes/plan/schemas/context-pack.schema.json`,
    // `properties.assurance.properties.class.enum` (schema order).
    const SCHEMA_ORDER: [&str; 5] = ["observed", "sampled", "bounded", "validated", "proved"];
    let ours: Vec<&str> = AssuranceLevel::ALL
        .iter()
        .map(|level| level.as_str())
        .collect();
    assert_eq!(ours, SCHEMA_ORDER);

    // RFC 0028: "`assurance.class` is an assurance *class*: `observed < sampled < bounded
    // < validated < proved`, RFC 0031's total order." The order is part of what the field
    // means, so it is asserted here and not only the membership.
    let mut sorted = AssuranceLevel::ALL;
    sorted.sort_unstable();
    assert_eq!(sorted, AssuranceLevel::ALL);
    assert!(AssuranceLevel::Observed < AssuranceLevel::Proved);

    // And the class the block reports is the one it was built with — the field a
    // `EvaluationVerdictValue.assurance_class` must equal on the wire (RFC 0028, "Wire
    // surface").
    assert_eq!(assurance().class(), AssuranceLevel::Bounded);
    assert_eq!(
        assurance().to_json().as_object().expect("object")["class"].as_str(),
        Some("bounded")
    );
}

#[test]
fn a_target_names_a_class_checked_contract_and_a_typed_question() {
    let handle = intent("ack_v1");
    let compiled = Target::new(handle.clone(), question("why did AckImpliesDurable fail?"))
        .expect("an in_* handle is accepted");
    assert_eq!(compiled.intent(), &handle);
    assert_eq!(
        compiled.question().as_str(),
        "why did AckImpliesDurable fail?"
    );

    // A handle of the wrong class is refused outright — never silently accepted and never
    // silently dropped, the discipline `ModelActionRef::with_model` set for `model_*`.
    for class in [
        ArtifactClass::WorkspaceSnapshot,
        ArtifactClass::SignedIntentBundle,
        ArtifactClass::ContextPack,
        ArtifactClass::Evidence,
    ] {
        let wrong = ArtifactHandle::new(class, "x1").expect("well-formed");
        assert_eq!(
            Target::new(wrong, question("why?")),
            Err(TargetError::WrongArtifactClass(class))
        );
    }

    // The RFC names exactly two forms of `question`, and the type has exactly two
    // constructors. The expansion form is the canonical triple IMPL-04 already builds,
    // not prose about it.
    let query = ExpansionQuery::new(
        ExpansionRelation::CausalPredecessors,
        Name::new("e_ack").expect("well-formed anchor"),
    );
    let expanded = Question::of_expansion(&ExpansionQuestion::of(&query, Depth::DEFAULT));
    assert_eq!(
        expanded.as_str(),
        r#"{"anchor":"e_ack","depth":1,"relation":"causal_predecessors"}"#
    );

    // A question is identity-bearing, so it has one spelling: empty, untrimmed, and
    // non-printable-ASCII texts are refused, and the refusal names a position rather than
    // echoing the text back (INV-016).
    assert_eq!(Question::compiled(""), Err(QuestionError::Empty));
    assert_eq!(Question::compiled("why? "), Err(QuestionError::Untrimmed));
    assert!(matches!(
        Question::compiled("why\tnow?"),
        Err(QuestionError::NonCanonical { .. })
    ));
}

#[test]
fn an_intent_id_drops_straight_into_the_slot() {
    // Unlike IMPL-03's `model_*`, this class has a producer in this workspace:
    // `IntentContract::mint_intent_id` derives `in_<digest>` from a contract's canonical
    // encoding (RFC 0037 ID1), and `IntentId` is `continuum-intent`'s newtype over the
    // same plan §4.4 grammar. The two spellings agree, so the slot is a real slot and not
    // a promise about a producer that does not exist.
    let minted = IntentId::new("in_9f2c4a").expect("well-formed intent id");
    let handle = ArtifactHandle::from_str(minted.as_str()).expect("the same grammar parses");
    assert_eq!(handle.class(), ArtifactClass::IntentContract);
    assert_eq!(handle.to_string(), minted.as_str());

    let target = Target::new(handle, question("why?")).expect("accepted");
    assert_eq!(
        target.to_json_fields()[0].1,
        Json::String("in_9f2c4a".to_owned())
    );

    // What is declined is resolution, not shape: "`intent` MUST resolve in the daemon's
    // intent registry (RFC 0037)" names a registry this crate has no edge to, so a
    // well-formed handle of the right class is accepted and resolution is `continuumd`'s.
    assert!(Target::new(intent("never_registered_anywhere"), question("why?")).is_ok());
}

#[test]
fn inconclusiveness_is_typed_by_construction() {
    // INV-008 and the schema's own `allOf[0]` ("inconclusive is never untyped"). The
    // pairing is not validated here — it is unrepresentable: `Verdict::Inconclusive`
    // carries its reason, so there is no untyped value to check.
    for reason in InconclusiveReason::ALL {
        let verdict = Verdict::Inconclusive(reason);
        assert_eq!(verdict.as_wire_str(), "inconclusive");
        assert_eq!(verdict.inconclusive_reason(), Some(reason));
        assert_eq!(
            Verdict::from_wire("inconclusive", Some(reason)),
            Ok(verdict),
            "every one of the six reasons round-trips"
        );

        // On the way out, the conditional holds by construction: the reason key appears
        // exactly when the verdict is inconclusive.
        let keys: Vec<String> = verdict
            .to_json_fields()
            .into_iter()
            .map(|(key, _)| key)
            .collect();
        assert_eq!(keys, vec!["verdict", "inconclusive_reason"]);
    }

    // Reading a pack that violates INV-008 is a refusal, not a repair.
    assert_eq!(
        Verdict::from_wire("inconclusive", None),
        Err(VerdictError::UntypedInconclusive)
    );

    // And the narrowing this module states: a reason beside a decided verdict is refused,
    // though the schema's conditional alone would permit it.
    assert_eq!(
        Verdict::from_wire("satisfied", Some(InconclusiveReason::EngineError)),
        Err(VerdictError::ReasonWithoutInconclusive {
            verdict: "satisfied"
        })
    );
    for decided in ["satisfied", "refuted", "deadlock"] {
        let verdict = Verdict::from_wire(decided, None).expect("parses");
        let keys: Vec<String> = verdict
            .to_json_fields()
            .into_iter()
            .map(|(key, _)| key)
            .collect();
        assert_eq!(keys, vec!["verdict"], "a decided verdict names no reason");
    }
}

#[test]
fn the_answer_header_carries_exactly_one_artifact_handle() {
    // Per-key artifact discipline, decided by the schema rather than guessed: `intent` is
    // a `^in_` handle; `verdict` is a bare enum string; `assurance` is an object of
    // `{class, envelope}` with `additionalProperties: false`. So the header names exactly
    // one artifact, and the other two field groups have no field one could occupy.
    let named = fragment(&target("ack_v1", "why?").to_json_fields());
    let handles: Vec<&str> = named
        .as_object()
        .expect("object")
        .values()
        .filter_map(Json::as_str)
        .filter(|value| ArtifactHandle::from_str(value).is_ok())
        .collect();
    assert_eq!(handles, vec!["in_ack_v1"]);

    for fields in [
        Verdict::Inconclusive(InconclusiveReason::AbstractionAmbiguity).to_json_fields(),
        assurance().to_json_fields(),
    ] {
        let mut strings = Vec::new();
        collect_strings(&fragment(&fields), &mut strings);
        assert!(!strings.is_empty(), "the fragment is not empty");
        for value in strings {
            assert!(
                ArtifactHandle::from_str(&value).is_err(),
                "{value:?} must not be a plan §4.4 handle"
            );
        }
    }
}

#[test]
fn the_bullet_contributes_exactly_its_four_schema_keys() {
    let mut keys: Vec<String> = Vec::new();
    keys.extend(
        target("ack_v1", "why?")
            .to_json_fields()
            .into_iter()
            .map(|(key, _)| key),
    );
    keys.extend(
        Verdict::Inconclusive(InconclusiveReason::IncompleteProofSearch)
            .to_json_fields()
            .into_iter()
            .map(|(key, _)| key),
    );
    keys.extend(assurance().to_json_fields().into_iter().map(|(key, _)| key));
    keys.sort();
    assert_eq!(
        keys,
        vec![
            "assurance",
            "inconclusive_reason",
            "intent",
            "question",
            "verdict",
        ]
    );

    // Four of the five are among the schema's seventeen required properties; the fifth,
    // `inconclusive_reason`, is the conditional one INV-008 requires exactly when the
    // verdict is inconclusive, and `pack`'s own `OPTIONAL_INHERITED_KEYS` already treats
    // it that way.
    for key in ["intent", "question", "verdict", "assurance"] {
        assert!(
            REQUIRED_KEYS.contains(&key),
            "{key} is one of the schema's required properties"
        );
    }
    assert!(!REQUIRED_KEYS.contains(&"inconclusive_reason"));
}

#[test]
fn distinct_headers_have_distinct_canonical_encodings() {
    assert_ne!(
        target("ack_v1", "why?").to_canonical_bytes(),
        target("ack_v2", "why?").to_canonical_bytes()
    );
    assert_ne!(
        target("ack_v1", "why?").to_canonical_bytes(),
        target("ack_v1", "why not?").to_canonical_bytes()
    );

    // Every one of the four verdicts, and every one of the six reasons, is a distinct
    // encoding — an inconclusive pack is never indistinguishable from a decided one.
    let mut verdicts = vec![Verdict::Satisfied, Verdict::Refuted, Verdict::Deadlock];
    verdicts.extend(InconclusiveReason::ALL.map(Verdict::Inconclusive));
    for (index, verdict) in verdicts.iter().enumerate() {
        for other in &verdicts[index + 1..] {
            assert_ne!(verdict.to_canonical_bytes(), other.to_canonical_bytes());
        }
    }

    // The class and any dimension both move the assurance block's bytes.
    assert_ne!(
        assurance().to_canonical_bytes(),
        Assurance::new(AssuranceLevel::Proved, envelope()).to_canonical_bytes()
    );
    assert_ne!(
        assurance().to_canonical_bytes(),
        Assurance::new(
            AssuranceLevel::Bounded,
            envelope().with(
                AssuranceDimension::Faults,
                DimensionEvidence::produced("continuum-dpor", "crash faults to bound 1")
                    .expect("named producer"),
            ),
        )
        .to_canonical_bytes()
    );
}

#[test]
fn identical_headers_are_byte_identical_across_independent_builds() {
    // Two independently constructed values, sharing no state — the same discipline
    // docs/19 §7's determinism matrix applies to a compiled artifact.
    assert_eq!(
        target("ack_v1", "why did AckImpliesDurable fail?").to_canonical_bytes(),
        target("ack_v1", "why did AckImpliesDurable fail?").to_canonical_bytes()
    );
    assert_eq!(
        Verdict::Inconclusive(InconclusiveReason::InsufficientTelemetry).to_canonical_bytes(),
        Verdict::Inconclusive(InconclusiveReason::InsufficientTelemetry).to_canonical_bytes()
    );
    assert_eq!(
        assurance().to_canonical_bytes(),
        assurance().to_canonical_bytes()
    );

    // The envelope's nine keys are ID5-sorted by the writer, so the order the dimensions
    // were *set* in cannot reach the bytes.
    let forwards = envelope()
        .with(
            AssuranceDimension::Unknowns,
            DimensionEvidence::produced("e", "s").expect("named"),
        )
        .with(
            AssuranceDimension::Fairness,
            DimensionEvidence::produced("f", "t").expect("named"),
        );
    let backwards = envelope()
        .with(
            AssuranceDimension::Fairness,
            DimensionEvidence::produced("f", "t").expect("named"),
        )
        .with(
            AssuranceDimension::Unknowns,
            DimensionEvidence::produced("e", "s").expect("named"),
        );
    assert_eq!(
        Assurance::new(AssuranceLevel::Bounded, forwards).to_canonical_bytes(),
        Assurance::new(AssuranceLevel::Bounded, backwards).to_canonical_bytes()
    );
}

#[test]
fn targets_sort_into_one_stable_content_derived_order() {
    // RFC 0028, "Pack identity, lineage, and determinism": a list is ordered "by content
    // identity or by a declared sort key". The pack's own list-valued fields are not this
    // bullet's, but a compiler that groups packs by target needs the same property of the
    // values this bullet hands it, and `Target`'s derived order is content-derived: the
    // handle, then the question.
    let mut first = [
        target("ack_v2", "why?"),
        target("ack_v1", "why not?"),
        target("ack_v1", "why?"),
    ];
    let mut second = [
        target("ack_v1", "why?"),
        target("ack_v2", "why?"),
        target("ack_v1", "why not?"),
    ];
    first.sort();
    second.sort();
    let first_bytes: Vec<Vec<u8>> = first.iter().map(Target::to_canonical_bytes).collect();
    let second_bytes: Vec<Vec<u8>> = second.iter().map(Target::to_canonical_bytes).collect();
    assert_eq!(
        first_bytes, second_bytes,
        "one content, one order, regardless of build order"
    );
}

#[test]
fn every_emitted_fragment_round_trips_through_the_canonical_json_reader() {
    let fragments = vec![
        target("ack_v1", "why did AckImpliesDurable fail?").to_canonical_bytes(),
        Verdict::Refuted.to_canonical_bytes(),
        Verdict::Inconclusive(InconclusiveReason::Unsupported).to_canonical_bytes(),
        assurance().to_canonical_bytes(),
    ];

    for bytes in fragments {
        let parsed = Json::parse(&bytes).expect("canonical JSON parses");
        assert!(parsed.as_object().is_some(), "a fragment is an object");
        // Re-writing the parsed document is a fixpoint, matching
        // `continuum_intent::canonical_json`'s own idempotency claim — the fragment is
        // already in the one spelling ID5 admits.
        assert_eq!(parsed.to_canonical_bytes(), bytes);
    }
}

fn fragment(fields: &[(String, Json)]) -> Json {
    Json::object(fields.to_vec()).expect("a fragment's keys are distinct")
}

fn collect_strings(value: &Json, into: &mut Vec<String>) {
    match value {
        Json::String(text) => into.push(text.clone()),
        Json::Object(fields) => {
            for nested in fields.values() {
                collect_strings(nested, into);
            }
        }
        other => panic!("a header fragment carries only strings and objects, not {other:?}"),
    }
}
