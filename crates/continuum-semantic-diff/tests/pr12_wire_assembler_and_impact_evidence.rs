//! PR-12 / `bn-7ek41` evidence: the wire `intent_changes[]`/`diff_*` artifact
//! assembler and the impact-set computation — the two halves that close semantic
//! diff v0 over the landed classifier family.
//!
//! # What this file proves, and against what
//!
//! | Claim | Test |
//! |---|---|
//! | A genuine corpus revision assembles end-to-end: pair → full artifact → schema-valid canonical bytes → `PolicyTable::verdict`, byte-pinned | [`the_corpus_attack_assembles_schema_valid_bytes_and_matches_the_golden`] |
//! | Correction 14's exact wire shape: equal identities ⇒ a literally empty `intent_changes` array, `allow`, no reasons | [`the_unchanged_shortcut_produces_correction_14s_exact_wire_shape`] |
//! | The `requested_assurance` clamp: expression directions are `unknown` below `bounded` and the direction at `bounded`+ — both directions asserted, so neither arm is vacuous | [`the_clamp_reports_unknown_below_bounded_and_the_direction_at_bounded_and_above`], [`the_clamp_reaches_assumption_directions_too`] |
//! | Encoding-decidable directions do not consume the budget: a clean fairness `kind` swap survives at `observed` | [`a_decidable_fairness_direction_survives_below_bounded`] |
//! | The assembler-owned `unsupported` override: narrowing `scope.fragments` forces every unit stated in the removed fragment to `unsupported`, never an allowed direction | [`narrowing_scope_fragments_forces_unsupported_on_every_finite_unit`] |
//! | A rename disguise arrives on the wire as `removed`+`added` with both unit locators, never `unchanged` | [`a_rename_disguise_arrives_as_removed_plus_added_with_units`] |
//! | An *allowed* revision still invalidates its dependent evidence — the impact set is not the verdict | [`an_allowed_revision_still_invalidates_dependent_evidence`] |
//! | A no-reason field change blankets the impact scope to `unknown`: no witness can prove independence from a change no reason expresses | [`a_no_reason_field_change_blankets_the_impact_scope_to_unknown`] |
//! | Determinism (INV-005): byte-identical across runs and under input permutation | [`the_artifact_is_byte_deterministic_under_input_permutation`] |
//! | The schema-conformance checker itself catches violations (anti-vacuity, both directions) | [`the_schema_conformance_checker_is_not_vacuous`] |
//! | Every scenario's artifact validates against the live schema bytes | [`every_scenario_artifact_validates_against_the_live_schema`] |
//! | `policy.decision` equals the typed verdict on every scenario — the RFC 0031 wire-surface MUST | [`policy_decision_always_equals_the_typed_verdict`] |
//!
//! # The schema decides
//!
//! The conformance checker below is **derived from the live schema bytes**
//! (`notes/plan/schemas/semantic-diff.schema.json`, read at compile time), not
//! from a restatement: the required-key lists, the closed enums, the `protected`
//! const, the eleven-field `unit` requirement, the identity patterns, and the
//! fail-closed `allOf` are all navigated out of the schema document and then
//! enforced against the artifact. A schema edit that moves any of them fails
//! these tests loudly instead of silently diverging; an *unrecognized* pattern
//! string is an error, never a skip.
//!
//! # Golden discipline
//!
//! Deterministic (INV-005): fixed inputs, no clock, no entropy. The corpus
//! scenario's canonical bytes are pinned at `tests/golden/pr12_wire_artifact.json`;
//! regeneration is explicit and always red (`PR12_ARTIFACT_BLESS=1` rewrites the
//! golden and then panics, so a blessing run cannot pass and the diff is reviewed
//! as a contract change).

use std::collections::BTreeSet;

use continuum_intent::assurance_policy::{AssuranceLevel, level_from_wire};
use continuum_intent::canonical_json::Json;
use continuum_intent::change_policy::{AcceptancePath, PolicyDecision, PolicyField, Relation};
use continuum_intent::contract::{IntentContract, IntentId};
use continuum_semantic_diff::artifact::{
    AssembleError, DiffArtifact, DiffId, DiffRequest, SemanticAxis, SemanticChange, SnapshotId,
    assemble,
};
use continuum_semantic_diff::impact::{
    DependencyEdge, DependencyReason, EvidenceId, EvidenceRecord, Independence, ReuseEdgeClass,
};

// --- the governing texts, read at compile time ----------------------------------------------

const FIXTURE: &str =
    include_str!("../../continuum-intent/tests/fixtures/replicated-register-contract.json");

const SCHEMA: &str = include_str!("../../../notes/plan/schemas/semantic-diff.schema.json");

const GOLDEN: &str = include_str!("golden/pr12_wire_artifact.json");

// --- fixture substrings the mutations pivot on (the DX-02 campaign's own spellings) --------

/// The one place the fixture spells `v1 == v2` — the disjunct that makes
/// `Agreement` an agreement property at all.
const AGREEMENT_EQ_DISJUNCT: &str = r#"{"kind":"compare","left":{"kind":"var","name":"v1"},"op":"eq","right":{"kind":"var","name":"v2"}}"#;

/// The complementary disjunct `v1 != v2`; appended after
/// [`AGREEMENT_EQ_DISJUNCT`] it keeps CPNF-1's sorted operand order, so the
/// gutted document is still canonical and arrives through the front door.
const AGREEMENT_NE_DISJUNCT: &str = r#"{"kind":"compare","left":{"kind":"var","name":"v1"},"op":"ne","right":{"kind":"var","name":"v2"}}"#;

/// `NetworkNoForgery`'s disjunction (`was_sent ∨ ¬Deliver-occurs`) — dropping the
/// forgery-escape disjunct to the bare predicate is the in-place assumption
/// strengthening the oracle decides (`bn-2nwpg`'s own mutation).
const NO_FORGERY_DISJUNCTION: &str = r#"{"kind":"or","operands":[{"args":[],"kind":"predicate","name":"was_sent"},{"kind":"not","operand":{"kind":"action","modality":"occurs","name":"Deliver"}}]}"#;

/// The strengthened replacement: the bare `was_sent` predicate.
const NO_FORGERY_STRENGTHENED: &str = r#"{"args":[],"kind":"predicate","name":"was_sent"}"#;

// --- helpers --------------------------------------------------------------------------------

fn decode(document: &str) -> IntentContract {
    IntentContract::decode(document.trim_end().as_bytes()).expect("the document decodes")
}

fn replace_once(document: &str, needle: &str, replacement: &str) -> String {
    assert!(
        document.contains(needle),
        "the fixture carries the text this mutation edits: {needle}"
    );
    document.replacen(needle, replacement, 1)
}

fn level(token: &str) -> AssuranceLevel {
    level_from_wire(token).expect("a closed-set level token")
}

fn request<'a>(
    before: &'a IntentContract,
    after: &'a IntentContract,
    requested: AssuranceLevel,
) -> DiffRequest<'a> {
    DiffRequest {
        diff_id: DiffId::new("diff_pr12_evidence").expect("well formed"),
        before_snapshot: SnapshotId::new("ws_before1").expect("well formed"),
        after_snapshot: SnapshotId::new("ws_after1").expect("well formed"),
        before_intent: IntentId::new("in_replicated_register_v1").expect("well formed"),
        after_intent: IntentId::new("in_replicated_register_v2").expect("well formed"),
        before,
        after,
        requested_assurance: requested,
        path: AcceptancePath::AgentAccept,
        semantic_changes: Vec::new(),
        evidence: Vec::new(),
    }
}

fn ev(id: &str) -> EvidenceId {
    EvidenceId::new(id).expect("well formed")
}

fn wire_relation(artifact: &DiffArtifact, field: PolicyField, unit: Option<&str>) -> Relation {
    artifact
        .intent_changes()
        .iter()
        .find(|record| record.field() == field && record.unit() == unit)
        .unwrap_or_else(|| panic!("a wire record for {field:?}/{unit:?} exists"))
        .relation()
}

fn impact_names(ids: Box<dyn Iterator<Item = &EvidenceId> + '_>) -> Vec<String> {
    ids.map(|id| id.as_str().to_owned()).collect()
}

/// The corpus scenario the golden pins: the plan §5.1 property attack (the
/// complementary `v1 != v2` disjunct on `Agreement`) riding beside the §5.1
/// bounds attack (`nodes` 3 → 2), with an evidence scope exercising all three
/// impact sets and one advisory program-side record.
fn corpus_scenario() -> (IntentContract, IntentContract) {
    let before = decode(FIXTURE);
    let weakened = replace_once(
        FIXTURE,
        AGREEMENT_EQ_DISJUNCT,
        &format!("{AGREEMENT_EQ_DISJUNCT},{AGREEMENT_NE_DISJUNCT}"),
    );
    let after = decode(&replace_once(&weakened, "\"nodes\":3", "\"nodes\":2"));
    (before, after)
}

fn corpus_evidence() -> Vec<EvidenceRecord> {
    vec![
        // Keyed to the old intent identity with no reuse witness: the query-key
        // trigger invalidates (unaddressable, not stale).
        EvidenceRecord {
            id: ev("ev_old_agreement_run"),
            keyed_to_before_intent: true,
            key_change_reuse_witness: false,
            edges: Vec::new(),
        },
        // Keyed, but a Validated edge carries a checker witness licensing reuse.
        EvidenceRecord {
            id: ev("proof_reusable_model"),
            keyed_to_before_intent: true,
            key_change_reuse_witness: true,
            edges: Vec::new(),
        },
        // Conditional on a bound (the contracted `nodes`), independence unknown:
        // dependent by the tri-state rule, lands in `unknown`.
        EvidenceRecord {
            id: ev("ev_bound_conditional"),
            keyed_to_before_intent: false,
            key_change_reuse_witness: false,
            edges: vec![DependencyEdge {
                reason: DependencyReason::ReliesOnAssumptionFairnessBound,
                class: ReuseEdgeClass::Exact,
                independence: Independence::Unknown,
            }],
        },
        // Names a snapshot only, no dependency on any changed field: reused.
        EvidenceRecord {
            id: ev("ev_unrelated_trace"),
            keyed_to_before_intent: false,
            key_change_reuse_witness: false,
            edges: Vec::new(),
        },
    ]
}

fn corpus_semantic_changes() -> Vec<SemanticChange> {
    vec![SemanticChange {
        kind: SemanticAxis::DurabilityOrdering,
        summary: "ReplyPublished now follows SyncCompleted".to_owned(),
        fragment: None,
        locations: BTreeSet::from(["src/lib.rs:42".to_owned()]),
    }]
}

fn corpus_artifact() -> DiffArtifact {
    let (before, after) = corpus_scenario();
    let mut request = request(&before, &after, level("bounded"));
    request.evidence = corpus_evidence();
    request.semantic_changes = corpus_semantic_changes();
    assemble(&request).expect("the corpus scenario assembles")
}

// --- the schema-driven conformance checker --------------------------------------------------

fn get<'a>(node: &'a Json, key: &str) -> Option<&'a Json> {
    node.as_object().and_then(|fields| fields.get(key))
}

fn expect<'a>(node: &'a Json, key: &str) -> &'a Json {
    get(node, key).unwrap_or_else(|| panic!("the schema carries `{key}` where navigated"))
}

fn enum_tokens(node: &Json) -> Vec<String> {
    expect(node, "enum")
        .as_array()
        .expect("an enum is an array")
        .iter()
        .map(|token| token.as_str().expect("enum tokens are strings").to_owned())
        .collect()
}

fn string_items(node: &Json) -> Vec<String> {
    node.as_array()
        .expect("an array")
        .iter()
        .map(|item| item.as_str().expect("string items").to_owned())
        .collect()
}

/// Interpret exactly the pattern shapes the live schema states; an unrecognized
/// pattern is an error, so schema drift fails loudly instead of skipping.
fn matches_pattern(pattern: &str, value: &str) -> Result<bool, String> {
    let handle_class = |suffix: &str| {
        !suffix.is_empty()
            && suffix
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    };
    match pattern {
        "^diff_[A-Za-z0-9_-]+$" => Ok(value.strip_prefix("diff_").is_some_and(handle_class)),
        "^ws_[A-Za-z0-9_-]+$" => Ok(value.strip_prefix("ws_").is_some_and(handle_class)),
        "^in_[A-Za-z0-9_-]+$" => Ok(value.strip_prefix("in_").is_some_and(handle_class)),
        "^[a-z][a-z0-9_]*_[A-Za-z0-9_-]+$" => Ok(EvidenceId::new(value).is_ok()),
        _ => Err(format!("unrecognized schema pattern {pattern:?}")),
    }
}

/// Enforce the live schema's load-bearing constraints against an artifact
/// document. Not a general JSON-Schema evaluator: it implements exactly what
/// `semantic-diff.schema.json` states, reading every list, const, enum, and
/// pattern out of the schema document itself.
fn check_schema_conformance(artifact: &Json, schema: &Json) -> Result<(), String> {
    let fields = artifact
        .as_object()
        .ok_or_else(|| "the artifact is an object".to_owned())?;
    let properties = expect(schema, "properties");

    // Required keys, and `additionalProperties: false`.
    for key in string_items(expect(schema, "required")) {
        if !fields.contains_key(&key) {
            return Err(format!("required key `{key}` is absent"));
        }
    }
    let declared = properties.as_object().expect("properties is an object");
    for key in fields.keys() {
        if !declared.contains_key(key) {
            return Err(format!(
                "undeclared key `{key}` (additionalProperties: false)"
            ));
        }
    }

    // Header consts.
    let schema_id = expect(expect(properties, "schema_id"), "const")
        .as_str()
        .expect("a const string");
    if get(artifact, "schema_id").and_then(Json::as_str) != Some(schema_id) {
        return Err("schema_id does not match the schema's const".to_owned());
    }
    let epoch = expect(expect(properties, "schema_epoch"), "const");
    if get(artifact, "schema_epoch") != Some(epoch) {
        return Err("schema_epoch does not match the schema's const".to_owned());
    }

    // Identity patterns.
    for key in [
        "diff_id",
        "before_snapshot",
        "after_snapshot",
        "before_intent",
        "after_intent",
    ] {
        let pattern = expect(expect(properties, key), "pattern")
            .as_str()
            .expect("a pattern string");
        let value = get(artifact, key)
            .and_then(Json::as_str)
            .ok_or_else(|| format!("`{key}` is a string"))?;
        if !matches_pattern(pattern, value)? {
            return Err(format!("`{key}` value {value:?} fails pattern {pattern:?}"));
        }
    }

    // `requested_assurance`, when present, is in the closed enum.
    if let Some(requested) = get(artifact, "requested_assurance") {
        let token = requested
            .as_str()
            .ok_or_else(|| "requested_assurance is a string".to_owned())?;
        if !enum_tokens(expect(properties, "requested_assurance")).contains(&token.to_owned()) {
            return Err(format!("requested_assurance {token:?} outside the enum"));
        }
    }

    // `classifier`, when present, carries its two required members.
    if let Some(classifier) = get(artifact, "classifier") {
        let spec = expect(properties, "classifier");
        for key in string_items(expect(spec, "required")) {
            if get(classifier, &key).and_then(Json::as_str).is_none() {
                return Err(format!("classifier.{key} is a required string"));
            }
        }
    }

    // `intent_changes` records.
    let items = expect(expect(properties, "intent_changes"), "items");
    let record_properties = expect(items, "properties");
    let field_enum = enum_tokens(expect(record_properties, "field"));
    let relation_enum = enum_tokens(expect(record_properties, "relation"));
    let fragment_enum = enum_tokens(expect(record_properties, "fragment"));
    let unit_rule = expect(items, "allOf").as_array().expect("an allOf array")[0].clone();
    let unit_required_fields = enum_tokens(expect(
        expect(expect(&unit_rule, "if"), "properties"),
        "field",
    ));
    let evidence_pattern = expect(expect(record_properties, "evidence"), "anyOf")
        .as_array()
        .expect("an anyOf array")[0]
        .clone();
    let evidence_pattern = expect(&evidence_pattern, "pattern")
        .as_str()
        .expect("a pattern string");
    let records = get(artifact, "intent_changes")
        .and_then(Json::as_array)
        .ok_or_else(|| "intent_changes is an array".to_owned())?;
    for record in records {
        let record_fields = record
            .as_object()
            .ok_or_else(|| "an intent_changes record is an object".to_owned())?;
        for key in record_fields.keys() {
            if !record_properties
                .as_object()
                .expect("an object")
                .contains_key(key)
            {
                return Err(format!("undeclared record key `{key}`"));
            }
        }
        for key in string_items(expect(items, "required")) {
            if !record_fields.contains_key(&key) {
                return Err(format!("record key `{key}` is required"));
            }
        }
        if get(record, "protected") != Some(&Json::Bool(true)) {
            return Err("protected is const true".to_owned());
        }
        let field = get(record, "field")
            .and_then(Json::as_str)
            .ok_or_else(|| "field is a string".to_owned())?;
        if !field_enum.contains(&field.to_owned()) {
            return Err(format!("field {field:?} outside the enum"));
        }
        let relation = get(record, "relation")
            .and_then(Json::as_str)
            .ok_or_else(|| "relation is a string".to_owned())?;
        if !relation_enum.contains(&relation.to_owned()) {
            return Err(format!("relation {relation:?} outside the enum"));
        }
        if let Some(fragment) = get(record, "fragment") {
            let token = fragment
                .as_str()
                .ok_or_else(|| "fragment is a string".to_owned())?;
            if !fragment_enum.contains(&token.to_owned()) {
                return Err(format!("fragment {token:?} outside the enum"));
            }
        }
        if unit_required_fields.contains(&field.to_owned()) {
            let unit = get(record, "unit")
                .and_then(Json::as_str)
                .ok_or_else(|| format!("field {field:?} requires a unit locator"))?;
            if unit.is_empty() {
                return Err("unit has minLength 1".to_owned());
            }
        }
        match get(record, "evidence") {
            None | Some(Json::Null) => {}
            Some(Json::String(value)) => {
                if !matches_pattern(evidence_pattern, value)? {
                    return Err(format!("evidence {value:?} fails its pattern"));
                }
            }
            Some(_) => return Err("evidence is a string or null".to_owned()),
        }
    }

    // `semantic_changes` records.
    let semantic_items = expect(expect(properties, "semantic_changes"), "items");
    let semantic_properties = expect(semantic_items, "properties");
    let axis_enum = enum_tokens(expect(semantic_properties, "kind"));
    let semantic_records = get(artifact, "semantic_changes")
        .and_then(Json::as_array)
        .ok_or_else(|| "semantic_changes is an array".to_owned())?;
    for record in semantic_records {
        for key in string_items(expect(semantic_items, "required")) {
            if get(record, &key).is_none() {
                return Err(format!("semantic_changes key `{key}` is required"));
            }
        }
        let kind = get(record, "kind")
            .and_then(Json::as_str)
            .ok_or_else(|| "kind is a string".to_owned())?;
        if !axis_enum.contains(&kind.to_owned()) {
            return Err(format!("semantic axis {kind:?} outside the enum"));
        }
    }

    // `impact`: the three required arrays, every item on the pattern.
    let impact_spec = expect(properties, "impact");
    let impact = get(artifact, "impact").ok_or_else(|| "impact is required".to_owned())?;
    for key in string_items(expect(impact_spec, "required")) {
        let pattern = expect(
            expect(expect(expect(impact_spec, "properties"), &key), "items"),
            "pattern",
        )
        .as_str()
        .expect("a pattern string");
        let entries = get(impact, &key)
            .and_then(Json::as_array)
            .ok_or_else(|| format!("impact.{key} is an array"))?;
        for entry in entries {
            let value = entry
                .as_str()
                .ok_or_else(|| format!("impact.{key} items are strings"))?;
            if !matches_pattern(pattern, value)? {
                return Err(format!("impact.{key} item {value:?} fails its pattern"));
            }
        }
    }

    // `policy`: closed decision, unique string reasons.
    let policy_spec = expect(properties, "policy");
    let policy = get(artifact, "policy").ok_or_else(|| "policy is required".to_owned())?;
    let decision_enum = enum_tokens(expect(expect(policy_spec, "properties"), "decision"));
    let decision = get(policy, "decision")
        .and_then(Json::as_str)
        .ok_or_else(|| "policy.decision is a string".to_owned())?;
    if !decision_enum.contains(&decision.to_owned()) {
        return Err(format!("policy.decision {decision:?} outside the enum"));
    }
    let reasons = get(policy, "reasons")
        .and_then(Json::as_array)
        .ok_or_else(|| "policy.reasons is an array".to_owned())?;
    let mut seen = BTreeSet::new();
    for reason in reasons {
        let text = reason
            .as_str()
            .ok_or_else(|| "reasons are strings".to_owned())?;
        if !seen.insert(text.to_owned()) {
            return Err(format!("duplicate reason {text:?} (uniqueItems)"));
        }
    }

    // The fail-closed allOf: a protected non-affirmative record forces the
    // decision to block or review.
    let fail_closed = expect(schema, "allOf").as_array().expect("an allOf array")[0].clone();
    let non_affirmative = enum_tokens(expect(
        expect(
            expect(
                expect(
                    expect(expect(&fail_closed, "if"), "properties"),
                    "intent_changes",
                ),
                "contains",
            ),
            "properties",
        ),
        "relation",
    ));
    let blocking_decisions = enum_tokens(expect(
        expect(
            expect(expect(expect(&fail_closed, "then"), "properties"), "policy"),
            "properties",
        ),
        "decision",
    ));
    let triggered = records.iter().any(|record| {
        get(record, "protected") == Some(&Json::Bool(true))
            && get(record, "relation")
                .and_then(Json::as_str)
                .is_some_and(|relation| non_affirmative.contains(&relation.to_owned()))
    });
    if triggered && !blocking_decisions.contains(&decision.to_owned()) {
        return Err(format!(
            "a protected non-affirmative record demands a decision in {blocking_decisions:?}, \
             found {decision:?}"
        ));
    }
    Ok(())
}

fn schema() -> Json {
    Json::parse(SCHEMA.as_bytes()).expect("the live schema parses under the canonical reader")
}

// --- the corpus scenario, end to end --------------------------------------------------------

#[test]
fn the_corpus_attack_assembles_schema_valid_bytes_and_matches_the_golden() {
    let artifact = corpus_artifact();

    // The wire records: the weakened Agreement with its per-unit locator and
    // Finite attribution (F1 paid), the contracted bounds with no unit (a
    // whole-field record), and nothing else — every unchanged unit omitted.
    assert_eq!(artifact.intent_changes().len(), 2);
    assert_eq!(
        wire_relation(&artifact, PolicyField::Properties, Some("Agreement")),
        Relation::Weakened
    );
    assert_eq!(
        wire_relation(&artifact, PolicyField::Bounds, None),
        Relation::Contracted
    );
    let agreement = artifact
        .intent_changes()
        .iter()
        .find(|record| record.unit() == Some("Agreement"))
        .expect("the Agreement record is on the wire");
    assert_eq!(agreement.fragment().map(|f| f.wire()), Some("Finite"));

    // The verdict closes into the before contract's own governance: `locked`
    // properties block the weakening, `no-decrease` blocks the contraction.
    assert_eq!(artifact.decision(), PolicyDecision::Block);
    let reasons: Vec<String> = artifact
        .verdict()
        .reasons()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(
        reasons
            .iter()
            .any(|reason| reason.contains("properties") && reason.contains("weakened")),
        "P6 names the weakening: {reasons:?}"
    );
    assert!(
        reasons
            .iter()
            .any(|reason| reason.contains("bounds") && reason.contains("contracted")),
        "P6 names the contraction: {reasons:?}"
    );

    // The impact partition: the keyed run is unaddressable, the witnessed proof
    // and the unrelated trace are reused, the bound-conditional run is unknown.
    assert_eq!(
        impact_names(Box::new(artifact.impact().invalidated())),
        ["ev_old_agreement_run"]
    );
    assert_eq!(
        impact_names(Box::new(artifact.impact().reused())),
        ["ev_unrelated_trace", "proof_reusable_model"]
    );
    assert_eq!(
        impact_names(Box::new(artifact.impact().unknown())),
        ["ev_bound_conditional"]
    );

    // Schema-valid canonical bytes, byte-pinned.
    let json = artifact.artifact_json();
    check_schema_conformance(&json, &schema()).expect("the artifact validates");
    let actual = String::from_utf8(artifact.to_artifact_bytes()).expect("UTF-8 by construction");
    if std::env::var_os("PR12_ARTIFACT_BLESS").is_some() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/golden/pr12_wire_artifact.json"
        );
        std::fs::write(path, &actual).expect("the golden is writable");
        panic!(
            "PR12_ARTIFACT_BLESS rewrote {path}; rerun without the variable and review the diff \
             as a contract change"
        );
    }
    assert_eq!(
        actual,
        GOLDEN.trim_end(),
        "the corpus artifact is byte-stable against the retained golden"
    );
}

// --- correction 14: the shortcut's exact wire shape -----------------------------------------

#[test]
fn the_unchanged_shortcut_produces_correction_14s_exact_wire_shape() {
    let before = decode(FIXTURE);
    let after = decode(FIXTURE);
    let mut shortcut_request = request(&before, &after, level("bounded"));
    shortcut_request.evidence = vec![EvidenceRecord {
        id: ev("ev_still_good"),
        keyed_to_before_intent: true,
        key_change_reuse_witness: false,
        edges: Vec::new(),
    }];
    let artifact = assemble(&shortcut_request).expect("the shortcut assembles");

    // The MUST, at wire grain: a literally empty array in the canonical bytes.
    assert!(artifact.intent_changes().is_empty());
    let bytes = String::from_utf8(artifact.to_artifact_bytes()).expect("UTF-8");
    assert!(
        bytes.contains(r#""intent_changes":[]"#),
        "correction 14's exact wire shape: {bytes}"
    );
    // The verdict is still P1-complete — allow with no reasons, computed from
    // fifteen explicit unchanged records, not asserted from silence.
    assert_eq!(artifact.decision(), PolicyDecision::Allow);
    assert!(artifact.verdict().reasons().is_empty());
    assert!(bytes.contains(r#""policy":{"decision":"allow","reasons":[]}"#));
    // No field changed, so nothing implicates the evidence: reused, not dropped.
    assert_eq!(
        impact_names(Box::new(artifact.impact().reused())),
        ["ev_still_good"]
    );
    check_schema_conformance(&artifact.artifact_json(), &schema()).expect("validates");
}

// --- the requested_assurance clamp ----------------------------------------------------------

#[test]
fn the_clamp_reports_unknown_below_bounded_and_the_direction_at_bounded_and_above() {
    let before = decode(FIXTURE);
    let after = decode(&replace_once(
        FIXTURE,
        AGREEMENT_EQ_DISJUNCT,
        &format!("{AGREEMENT_EQ_DISJUNCT},{AGREEMENT_NE_DISJUNCT}"),
    ));
    for (token, expected) in [
        ("observed", Relation::Unknown),
        ("sampled", Relation::Unknown),
        ("bounded", Relation::Weakened),
        ("validated", Relation::Weakened),
        ("proved", Relation::Weakened),
    ] {
        let artifact = assemble(&request(&before, &after, level(token))).expect("assembles");
        assert_eq!(
            wire_relation(&artifact, PolicyField::Properties, Some("Agreement")),
            expected,
            "at requested {token}"
        );
        // Fail-closed either way under the fixture's own `locked` verb: the
        // clamp trades an affirmed direction for `unknown`, never for silence
        // and never for `allow`.
        assert_eq!(artifact.decision(), PolicyDecision::Block);
        check_schema_conformance(&artifact.artifact_json(), &schema()).expect("validates");
    }
}

#[test]
fn the_clamp_reaches_assumption_directions_too() {
    let before = decode(FIXTURE);
    let after = decode(&replace_once(
        FIXTURE,
        NO_FORGERY_DISJUNCTION,
        NO_FORGERY_STRENGTHENED,
    ));
    let at_bounded = assemble(&request(&before, &after, level("bounded"))).expect("assembles");
    assert_eq!(
        wire_relation(
            &at_bounded,
            PolicyField::Assumptions,
            Some("NetworkNoForgery")
        ),
        Relation::Strengthened,
        "the oracle's affirmed direction stands at bounded"
    );
    let at_observed = assemble(&request(&before, &after, level("observed"))).expect("assembles");
    assert_eq!(
        wire_relation(
            &at_observed,
            PolicyField::Assumptions,
            Some("NetworkNoForgery")
        ),
        Relation::Unknown,
        "below bounded the same pair reports unknown, never the direction"
    );
    // Both close into review under the fixture's own `review` verb — the clamp
    // never opens an allow.
    assert_eq!(at_bounded.decision(), PolicyDecision::Review);
    assert_eq!(at_observed.decision(), PolicyDecision::Review);
}

#[test]
fn a_decidable_fairness_direction_survives_below_bounded() {
    let before = decode(FIXTURE);
    // The clean kind swap on the fixture's one constraint (`weak:Recover`,
    // unconditional): weak → strong is definitional, decidable by canonical
    // encoding alone, and consumes no directional-evidence budget.
    let after = decode(&replace_once(
        FIXTURE,
        r#""kind":"weak""#,
        r#""kind":"strong""#,
    ));
    let artifact = assemble(&request(&before, &after, level("observed"))).expect("assembles");
    assert_eq!(
        wire_relation(&artifact, PolicyField::Fairness, Some("strong:Recover")),
        Relation::Strengthened,
        "an encoding-decidable direction is admissible at every level"
    );
    assert_eq!(artifact.decision(), PolicyDecision::Review);
    check_schema_conformance(&artifact.artifact_json(), &schema()).expect("validates");
}

// --- the unsupported override ---------------------------------------------------------------

#[test]
fn narrowing_scope_fragments_forces_unsupported_on_every_finite_unit() {
    let before = decode(FIXTURE);
    let after = decode(&replace_once(
        FIXTURE,
        r#""fragments":["Finite","Temporal"]"#,
        r#""fragments":["Temporal"]"#,
    ));
    let artifact = assemble(&request(&before, &after, level("bounded"))).expect("assembles");

    // Every claim and assumption in the fixture declares Finite; all classify
    // unsupported, each with its unit and the out-of-scope fragment attributed.
    for (field, unit) in [
        (PolicyField::Properties, "Agreement"),
        (PolicyField::Properties, "RuntimeToAbstract"),
        (PolicyField::Properties, "StableWitness"),
        (PolicyField::Assumptions, "DurableLogIsAPrefix"),
        (PolicyField::Assumptions, "NetworkNoForgery"),
        (PolicyField::Assumptions, "QuorumIntersection"),
    ] {
        assert_eq!(
            wire_relation(&artifact, field, Some(unit)),
            Relation::Unsupported,
            "{unit} is stated in the removed fragment"
        );
    }
    // The scope change itself is on the wire at its sub-field grain, fail-closed.
    assert_eq!(
        wire_relation(&artifact, PolicyField::Scope, Some("fragments")),
        Relation::Unknown
    );
    // Narrowing converted the situation into a fail-closed block (locked
    // properties, P2), never into an allowed one.
    assert_eq!(artifact.decision(), PolicyDecision::Block);
    check_schema_conformance(&artifact.artifact_json(), &schema()).expect("validates");

    // Anti-vacuity: with the fragment still declared, the same units carry no
    // unsupported record anywhere.
    let (_, weakened_after) = corpus_scenario();
    let baseline = assemble(&request(&before, &weakened_after, level("bounded")))
        .expect("the baseline assembles");
    assert!(
        baseline
            .intent_changes()
            .iter()
            .all(|record| record.relation() != Relation::Unsupported),
        "unsupported arises from scope, not from the classifiers"
    );
}

// --- disguises ------------------------------------------------------------------------------

#[test]
fn a_rename_disguise_arrives_as_removed_plus_added_with_units() {
    let before = decode(FIXTURE);
    let after = decode(&replace_once(
        FIXTURE,
        r#""id":"StableWitness""#,
        r#""id":"StableWitnessPrime""#,
    ));
    let artifact = assemble(&request(&before, &after, level("bounded"))).expect("assembles");
    assert_eq!(
        wire_relation(&artifact, PolicyField::Properties, Some("StableWitness")),
        Relation::Removed
    );
    assert_eq!(
        wire_relation(
            &artifact,
            PolicyField::Properties,
            Some("StableWitnessPrime")
        ),
        Relation::Added
    );
    assert_eq!(
        artifact.intent_changes().len(),
        2,
        "two records, two units — never one unchanged pair"
    );
    assert_eq!(artifact.decision(), PolicyDecision::Block);
}

// --- the impact set through the full path ---------------------------------------------------

#[test]
fn an_allowed_revision_still_invalidates_dependent_evidence() {
    let before = decode(FIXTURE);
    // Raise the assurance minimum: `upgraded`, which the fixture's `no-downgrade`
    // verb does not deny — an allow. (This test once removed an accepted evidence
    // class instead; RFC 0031 correction 20, bn-36luu, routes a membership change to
    // `review` under `no-downgrade`, so that edit is no longer an allowed revision.
    // `evidence_class_membership_review.rs` pins it.)
    let after = decode(&replace_once(
        FIXTURE,
        r#""minimum":"validated""#,
        r#""minimum":"proved""#,
    ));
    let mut allowed_request = request(&before, &after, level("bounded"));
    allowed_request.evidence = vec![
        EvidenceRecord {
            id: ev("proof_lemma_dependent"),
            keyed_to_before_intent: false,
            key_change_reuse_witness: false,
            edges: vec![DependencyEdge {
                reason: DependencyReason::DependsOnLemmaOrChecker,
                class: ReuseEdgeClass::Exact,
                independence: Independence::Dependent,
            }],
        },
        EvidenceRecord {
            id: ev("ev_untouched"),
            keyed_to_before_intent: false,
            key_change_reuse_witness: false,
            edges: Vec::new(),
        },
    ];
    let artifact = assemble(&allowed_request).expect("assembles");

    // The requirement-triple record carries no unit.
    assert_eq!(
        wire_relation(&artifact, PolicyField::Assurance, None),
        Relation::Upgraded
    );
    assert_eq!(artifact.decision(), PolicyDecision::Allow);
    // The impact set is not the verdict: the allowed change still invalidates
    // what definitely depended on the assurance requirement.
    assert_eq!(
        impact_names(Box::new(artifact.impact().invalidated())),
        ["proof_lemma_dependent"]
    );
    assert_eq!(
        impact_names(Box::new(artifact.impact().reused())),
        ["ev_untouched"]
    );
    check_schema_conformance(&artifact.artifact_json(), &schema()).expect("validates");
}

#[test]
fn a_no_reason_field_change_blankets_the_impact_scope_to_unknown() {
    let before = decode(FIXTURE);
    // The plan §5.1 faults attack: remove `crash`.
    let after = decode(&replace_once(
        FIXTURE,
        r#""enabled":["crash","#,
        r#""enabled":["#,
    ));
    let mut blanket_request = request(&before, &after, level("bounded"));
    blanket_request.evidence = vec![EvidenceRecord {
        id: ev("ev_witnessed_independent"),
        keyed_to_before_intent: false,
        key_change_reuse_witness: false,
        edges: vec![DependencyEdge {
            reason: DependencyReason::ReliesOnAssumptionFairnessBound,
            class: ReuseEdgeClass::Validated,
            independence: Independence::Independent { witnessed: true },
        }],
    }];
    let artifact = assemble(&blanket_request).expect("assembles");
    assert_eq!(
        wire_relation(&artifact, PolicyField::Faults, Some("crash")),
        Relation::Removed
    );
    assert_eq!(artifact.decision(), PolicyDecision::Block, "no-removal");
    // `faults` has no dependency reason, so independence is Unknown for every
    // in-scope artifact — even a witnessed independence on another reason.
    assert_eq!(
        impact_names(Box::new(artifact.impact().unknown())),
        ["ev_witnessed_independent"]
    );
    assert!(impact_names(Box::new(artifact.impact().reused())).is_empty());
}

// --- determinism ----------------------------------------------------------------------------

#[test]
fn the_artifact_is_byte_deterministic_under_input_permutation() {
    let (before, after) = corpus_scenario();
    let mut first = request(&before, &after, level("bounded"));
    first.evidence = corpus_evidence();
    first.semantic_changes = corpus_semantic_changes();
    let mut second = request(&before, &after, level("bounded"));
    second.evidence = corpus_evidence().into_iter().rev().collect();
    second.semantic_changes = corpus_semantic_changes().into_iter().rev().collect();
    let first = assemble(&first).expect("assembles");
    let second = assemble(&second).expect("assembles");
    assert_eq!(
        first.to_artifact_bytes(),
        second.to_artifact_bytes(),
        "INV-005: byte-identical for the same inputs, whatever order they arrived in"
    );
}

// --- anti-vacuity of the conformance checker ------------------------------------------------

#[test]
fn the_schema_conformance_checker_is_not_vacuous() {
    let schema = schema();
    let artifact = corpus_artifact();
    let good = artifact.artifact_json();
    check_schema_conformance(&good, &schema).expect("the untouched artifact validates");

    let doctor = |json: &Json, edit: &dyn Fn(&mut std::collections::BTreeMap<String, Json>)| {
        let Json::Object(mut fields) = json.clone() else {
            panic!("the artifact is an object")
        };
        edit(&mut fields);
        Json::Object(fields)
    };
    // Doctor the `properties` record specifically: it is the one whose field
    // requires a `unit`, so the dropped-locator probe below cannot land on a
    // whole-field record and pass vacuously.
    let doctor_record =
        |json: &Json, edit: &dyn Fn(&mut std::collections::BTreeMap<String, Json>)| {
            doctor(json, &|fields| {
                let Some(Json::Array(records)) = fields.get_mut("intent_changes") else {
                    panic!("intent_changes is an array")
                };
                let record = records
                    .iter_mut()
                    .find_map(|record| match record {
                        Json::Object(fields)
                            if fields.get("field")
                                == Some(&Json::String("properties".to_owned())) =>
                        {
                            Some(fields)
                        }
                        _ => None,
                    })
                    .expect("the corpus artifact carries a properties record");
                edit(record);
            })
        };

    // An unprotected record.
    let unprotected = doctor_record(&good, &|record| {
        record.insert("protected".to_owned(), Json::Bool(false));
    });
    assert!(check_schema_conformance(&unprotected, &schema).is_err());

    // A per-unit field with its locator dropped.
    let unlocated = doctor_record(&good, &|record| {
        record.remove("unit");
    });
    assert!(check_schema_conformance(&unlocated, &schema).is_err());

    // A relation outside the closed set.
    let invented = doctor_record(&good, &|record| {
        record.insert("relation".to_owned(), Json::String("equivalent".to_owned()));
    });
    assert!(check_schema_conformance(&invented, &schema).is_err());

    // The fail-closed allOf: an unknown record beside an allow decision.
    let fail_open = doctor(&good, &|fields| {
        let Some(Json::Array(records)) = fields.get_mut("intent_changes") else {
            panic!("intent_changes is an array")
        };
        let Some(Json::Object(record)) = records.first_mut() else {
            panic!("records exist")
        };
        record.insert("relation".to_owned(), Json::String("unknown".to_owned()));
        let Some(Json::Object(policy)) = fields.get_mut("policy") else {
            panic!("policy is an object")
        };
        policy.insert("decision".to_owned(), Json::String("allow".to_owned()));
    });
    assert!(check_schema_conformance(&fail_open, &schema).is_err());

    // A required section removed.
    let gutted = doctor(&good, &|fields| {
        fields.remove("impact");
    });
    assert!(check_schema_conformance(&gutted, &schema).is_err());

    // A malformed identity.
    let mis_keyed = doctor(&good, &|fields| {
        fields.insert("diff_id".to_owned(), Json::String("not_a_diff".to_owned()));
    });
    assert!(check_schema_conformance(&mis_keyed, &schema).is_err());
}

// --- the whole scenario battery under the schema and the verdict-equality MUST --------------

fn scenario_artifacts() -> Vec<(&'static str, DiffArtifact)> {
    let base = decode(FIXTURE);
    let mutations: Vec<(&'static str, String)> = vec![
        (
            "weaken-property",
            replace_once(
                FIXTURE,
                AGREEMENT_EQ_DISJUNCT,
                &format!("{AGREEMENT_EQ_DISJUNCT},{AGREEMENT_NE_DISJUNCT}"),
            ),
        ),
        (
            "strengthen-assumption",
            replace_once(FIXTURE, NO_FORGERY_DISJUNCTION, NO_FORGERY_STRENGTHENED),
        ),
        (
            "reduce-bounds",
            replace_once(FIXTURE, "\"nodes\":3", "\"nodes\":2"),
        ),
        (
            "swap-fairness-kind",
            replace_once(FIXTURE, r#""kind":"weak""#, r#""kind":"strong""#),
        ),
        (
            "remove-fault",
            replace_once(FIXTURE, r#""enabled":["crash","#, r#""enabled":["#),
        ),
        (
            "narrow-scope",
            replace_once(
                FIXTURE,
                r#""fragments":["Finite","Temporal"]"#,
                r#""fragments":["Temporal"]"#,
            ),
        ),
        (
            "rebind-abstraction-map",
            replace_once(
                FIXTURE,
                r#""map_runtime_to_abstract_v1""#,
                r#""map_runtime_to_abstract_v2""#,
            ),
        ),
        (
            "rename-claim",
            replace_once(
                FIXTURE,
                r#""id":"StableWitness""#,
                r#""id":"StableWitnessPrime""#,
            ),
        ),
        ("unchanged", FIXTURE.to_owned()),
    ];
    mutations
        .into_iter()
        .map(|(name, document)| {
            let after = decode(&document);
            let artifact = assemble(&request(&base, &after, level("bounded"))).expect("assembles");
            (name, artifact)
        })
        .collect()
}

#[test]
fn every_scenario_artifact_validates_against_the_live_schema() {
    let schema = schema();
    for (name, artifact) in scenario_artifacts() {
        check_schema_conformance(&artifact.artifact_json(), &schema)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        // Round-trip: the canonical bytes parse back to the same document.
        let bytes = artifact.to_artifact_bytes();
        assert_eq!(
            Json::parse(&bytes).expect("the bytes parse"),
            artifact.artifact_json(),
            "{name}: one byte spelling"
        );
    }
}

#[test]
fn policy_decision_always_equals_the_typed_verdict() {
    for (name, artifact) in scenario_artifacts() {
        let json = artifact.artifact_json();
        let decision = json
            .as_object()
            .and_then(|fields| fields.get("policy"))
            .and_then(Json::as_object)
            .and_then(|policy| policy.get("decision"))
            .and_then(Json::as_str)
            .expect("the wire decision is a string");
        assert_eq!(
            decision,
            artifact.verdict().decision().wire(),
            "{name}: PolicyVerdictValue.decision MUST equal the artifact's policy.decision"
        );
    }
}

// --- refused inputs -------------------------------------------------------------------------

#[test]
fn a_duplicated_evidence_identity_refuses_the_whole_artifact() {
    let (before, after) = corpus_scenario();
    let mut bad = request(&before, &after, level("bounded"));
    bad.evidence = vec![
        EvidenceRecord {
            id: ev("ev_twin"),
            keyed_to_before_intent: false,
            key_change_reuse_witness: false,
            edges: Vec::new(),
        },
        EvidenceRecord {
            id: ev("ev_twin"),
            keyed_to_before_intent: true,
            key_change_reuse_witness: true,
            edges: Vec::new(),
        },
    ];
    assert!(
        matches!(assemble(&bad), Err(AssembleError::Impact(_))),
        "a laundered duplicate is a refusal, not a merge"
    );
}
