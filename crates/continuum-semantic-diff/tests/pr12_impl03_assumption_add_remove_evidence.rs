//! PR-12 / IMPL-03 exit evidence — assumption add/remove classification (`bn-7vg7`).
//!
//! # What this file is evidence for
//!
//! `src/assumptions.rs` classifies `assumptions[]` per RFC 0031's "`assumptions`"
//! rule. This file is the dedicated adversarial and clause-mapped evidence the bone
//! brief asks for, on top of that module's own unit tests (which stay the source of
//! truth for the per-unit mechanics and are not re-derived here), matching
//! `tests/pr12_impl05_observer_event_change_evidence.rs` and
//! `tests/pr12_impl06_fault_fairness_assurance_evidence.rs`'s shape.
//!
//! # Clause → test map
//!
//! | RFC 0031 / plan clause | Test |
//! |---|---|
//! | "an assumption `strengthened` admits fewer environments and makes verification easier, and is the gaming move plan §5.1 names" | [`positive_narrowing_a_real_corpus_assumptions_expression_is_never_silent_and_is_reviewed`] |
//! | docs/50 "strengthen assumptions" / plan §5.3 "assumption strengthening" (one of the seven G3 dimensions) | same test above |
//! | "one assumption, keyed by `id`" — membership | [`positive_removing_a_real_corpus_assumption_outright_classifies_removed_and_is_reviewed`], [`positive_adding_a_new_assumption_classifies_added_and_is_reviewed`] |
//! | "A change to `classification` or to `fidelity_profile` with an unchanged expression MUST classify `incomparable`" | [`positive_reclassifying_a_real_corpus_assumption_with_its_expression_held_fixed_classifies_incomparable_and_is_reviewed`] |
//! | "the classifier MUST NOT match claims by expression to defeat the rename" (properties' text, same order for assumptions) | [`negative_renaming_a_real_corpus_assumption_while_preserving_its_expression_is_removed_plus_added`] |
//! | R2 / "CPNF-1 interaction": `source` MUST NOT contribute to classification | [`negative_rewriting_a_real_corpus_assumptions_source_alone_still_classifies_unchanged`] |
//! | S2 / fail-closed rule: an unequal expression the oracle cannot relate is `unknown`, never a guessed direction (clause re-scoped by `bn-2nwpg`, which wired the oracle in after this file landed — every sweep member is an atom rename the oracle cannot decide, so the pinned expectations hold unchanged) | [`pin_no_content_edit_across_a_sweep_of_real_fixture_mutations_ever_emits_strengthened_or_weakened`] |
//! | anti-vacuity: the positive tests are not vacuously true | [`negative_mutant_blind_to_declaredness_would_wrongly_pass_a_reclassification_as_unchanged_on_the_real_fixture`], [`negative_mutant_matching_by_expression_instead_of_id_would_wrongly_pass_the_real_fixtures_rename_as_unchanged`] |
//!
//! # House rules, inherited from the PR-12 evidence precedent
//!
//! - `src/` is untouched by this file, and no existing test anywhere is edited.
//! - [`FIXTURE`] is the same real, already-reviewed corpus document
//!   `pr12_impl05_observer_event_change_evidence.rs` and
//!   `pr12_impl06_fault_fairness_assurance_evidence.rs` read
//!   (`crates/continuum-intent/tests/fixtures/replicated-register-contract.json`) —
//!   three real assumptions (`DurableLogIsAPrefix`, `NetworkNoForgery`,
//!   `QuorumIntersection`), a declared `classification` on every one, and a declared
//!   `fidelity_profile` on one (`NetworkNoForgery`, `adversarial-envelope`). Every
//!   mutation below is one bounded, linear `.replacen` on that one fixed, small
//!   string — never a loop of doublings, never nested growth.
//! - Positive, negative, and boundary evidence are each present and separately named.

use continuum_intent::assumptions::AssumptionSet;
use continuum_intent::canonical_json::Json;
use continuum_intent::change_policy::{
    AcceptancePath, ClassificationRecord, PolicyDecision, PolicyField, PolicyReviewers,
    PolicyTable, PolicyVerb, Relation,
};
use continuum_semantic_diff::assumptions::classify_assumptions;

/// The real corpus fixture: the replicated-register Intent Contract, also used by
/// `crate::observers`', `crate::bounds`', and `crate::faults`'/`fairness`'/`assurance`'
/// own evidence, and by `continuum-intent`'s PR-4 exit and INV-012 evidence.
const FIXTURE: &str =
    include_str!("../../continuum-intent/tests/fixtures/replicated-register-contract.json");

fn document(text: &str) -> Json {
    Json::parse(text.as_bytes()).expect("the fixture is canonical JSON")
}

fn fixture_field<'a>(parsed: &'a Json, field: &str) -> &'a Json {
    parsed
        .as_object()
        .expect("the fixture is an object")
        .get(field)
        .unwrap_or_else(|| panic!("the fixture declares {field:?}"))
}

fn fixture_assumptions(text: &str) -> AssumptionSet {
    let parsed = document(text);
    AssumptionSet::from_json(fixture_field(&parsed, "assumptions"))
        .expect("the fixture's assumptions decode")
}

fn fixture_policy(text: &str) -> PolicyTable {
    let parsed = document(text);
    PolicyTable::from_json(fixture_field(&parsed, "policy"))
        .expect("the fixture's policy table decodes")
}

fn fixture_reviewers(text: &str) -> PolicyReviewers {
    let parsed = document(text);
    PolicyReviewers::from_json(fixture_field(&parsed, "policy_reviewers"))
        .expect("the fixture's reviewer map decodes")
}

/// A complete `PolicyTable::verdict` records slice: `Unchanged` on every field except
/// the ones named in `overrides`, which take the given record instead. Mirrors
/// `pr12_impl06_fault_fairness_assurance_evidence.rs`'s identical helper: P1 needs one
/// record per field, and every field but the one(s) under test is honestly unchanged
/// in these scenarios (only `assumptions` was mutated per test).
fn records_with_overrides(overrides: &[(PolicyField, Relation)]) -> Vec<ClassificationRecord> {
    PolicyField::ALL
        .into_iter()
        .map(|field| {
            let relation = overrides
                .iter()
                .find(|(f, _)| *f == field)
                .map_or(Relation::Unchanged, |(_, r)| *r);
            ClassificationRecord::new(field, relation)
                .expect("every relation supplied here is admissible on its field")
        })
        .collect()
}

// --- baseline: the fixture's content is what we think it is ---------------------------------

#[test]
fn the_fixtures_three_assumptions_are_what_the_module_doc_says_they_are() {
    let assumptions = fixture_assumptions(FIXTURE);
    assert_eq!(assumptions.len(), 3);
    let changes = classify_assumptions(&assumptions, &assumptions);
    assert_eq!(changes.len(), 3);
    assert!(changes.iter().all(|c| c.relation() == Relation::Unchanged));
}

#[test]
fn the_fixtures_own_policy_is_review_with_a_named_reviewer() {
    // AO1's own text: "the closed set contains no verb that blocks `added`... those
    // fields are protected through `review` or `locked`, never through a directional
    // verb" — the fixture's authors made exactly that choice for `assumptions`.
    let policy = fixture_policy(FIXTURE);
    assert_eq!(policy.verb(PolicyField::Assumptions), PolicyVerb::Review);
    let reviewers = fixture_reviewers(FIXTURE);
    assert!(
        reviewers
            .principals(PolicyField::Assumptions)
            .is_some_and(|names| !names.is_empty())
    );
}

// --- positive: the named attack, on the real fixture, correctly caught and reviewed ---------

#[test]
fn positive_narrowing_a_real_corpus_assumptions_expression_is_never_silent_and_is_reviewed() {
    // The gaming move plan §5.1/§5.3 and docs/50 name: an assumption edited toward
    // "admits fewer environments", making verification easier while the claim it
    // guards reads unchanged. Applied to the fixture's `NetworkNoForgery` assumption
    // (`always(was_sent OR NOT occurs(Deliver))`): renaming the predicate it hinges
    // on is enough to move the canonical encoding, and — since the atoms are opaque
    // to the oracle bn-2nwpg wired in after this file landed, exactly as they were
    // to the oracle-less classifier it pinned — the honest, fail-closed answer is
    // `unknown`, never a guessed `unchanged` that would let the edit pass
    // silently. (A *decidable* in-place strengthening now classifies an affirmed
    // `strengthened`: `tests/pr12_assumption_oracle_wiring_evidence.rs`.)
    let before = fixture_assumptions(FIXTURE);
    let mutated = FIXTURE.replacen(r#""name":"was_sent""#, r#""name":"was_definitely_sent""#, 1);
    assert_ne!(mutated, FIXTURE, "the replacement must actually fire");
    let after = fixture_assumptions(&mutated);

    let changes = classify_assumptions(&before, &after);
    let network_change = changes
        .iter()
        .find(|c| c.unit().as_str() == "NetworkNoForgery")
        .expect("NetworkNoForgery is classified");
    assert_eq!(network_change.relation(), Relation::Unknown);
    assert!(!network_change.relation().is_affirmative());

    let policy = fixture_policy(FIXTURE);
    assert_eq!(policy.verb(PolicyField::Assumptions), PolicyVerb::Review);
    let records = records_with_overrides(&[(PolicyField::Assumptions, Relation::Unknown)]);
    let verdict = policy
        .verdict(
            &records,
            &fixture_reviewers(FIXTURE),
            AcceptancePath::AgentAccept,
        )
        .expect("a complete classification against a well-formed policy table computes");
    // P2: a non-affirmative relation contributes at least `review` on every field
    // under every verb; `review` here is both P2's floor and the fixture's own verb.
    assert_eq!(verdict.decision(), PolicyDecision::Review);
    assert!(
        verdict
            .reasons()
            .iter()
            .any(|r| r.field() == PolicyField::Assumptions && r.relation() == Relation::Unknown),
        "{:?}",
        verdict.reasons()
    );
}

#[test]
fn positive_removing_a_real_corpus_assumption_outright_classifies_removed_and_is_reviewed() {
    let before = fixture_assumptions(FIXTURE);
    let after_json = document(FIXTURE);
    let assumptions_array = fixture_field(&after_json, "assumptions")
        .as_array()
        .expect("assumptions is an array")
        .iter()
        .filter(|entry| {
            entry
                .as_object()
                .and_then(|obj| obj.get("id"))
                .and_then(Json::as_str)
                != Some("QuorumIntersection")
        })
        .cloned()
        .collect::<Vec<_>>();
    let after = AssumptionSet::from_json(&Json::Array(assumptions_array))
        .expect("the pruned assumptions array decodes");
    assert_eq!(after.len(), 2);

    let changes = classify_assumptions(&before, &after);
    let removed = changes
        .iter()
        .find(|c| c.unit().as_str() == "QuorumIntersection")
        .expect("QuorumIntersection is classified");
    assert_eq!(removed.relation(), Relation::Removed);

    let policy = fixture_policy(FIXTURE);
    let records = records_with_overrides(&[(PolicyField::Assumptions, Relation::Removed)]);
    let verdict = policy
        .verdict(
            &records,
            &fixture_reviewers(FIXTURE),
            AcceptancePath::AgentAccept,
        )
        .expect("a complete classification against a well-formed policy table computes");
    assert_eq!(verdict.decision(), PolicyDecision::Review);
}

#[test]
fn positive_adding_a_new_assumption_classifies_added_and_is_reviewed() {
    let before = fixture_assumptions(FIXTURE);
    // Append a fourth, well-formed assumption entry right before the closing `]` of
    // the `assumptions` array — a single bounded insertion, not a rebuild.
    let extra = r#",{"classification":"trust","expression":{"ast":{"args":[],"kind":"predicate","name":"operator_is_honest"},"fragment":"Finite","normal_form":"cpnf-1","source":"operator_is_honest"},"id":"OperatorHonesty"}]"#;
    let mutated = FIXTURE.replacen(r#"],"assurance""#, &format!("{extra},\"assurance\""), 1);
    assert_ne!(mutated, FIXTURE, "the replacement must actually fire");
    let after = fixture_assumptions(&mutated);
    assert_eq!(after.len(), 4);

    let changes = classify_assumptions(&before, &after);
    let added = changes
        .iter()
        .find(|c| c.unit().as_str() == "OperatorHonesty")
        .expect("OperatorHonesty is classified");
    assert_eq!(added.relation(), Relation::Added);

    let policy = fixture_policy(FIXTURE);
    let records = records_with_overrides(&[(PolicyField::Assumptions, Relation::Added)]);
    let verdict = policy
        .verdict(
            &records,
            &fixture_reviewers(FIXTURE),
            AcceptancePath::AgentAccept,
        )
        .expect("a complete classification against a well-formed policy table computes");
    assert_eq!(verdict.decision(), PolicyDecision::Review);
}

#[test]
fn positive_reclassifying_a_real_corpus_assumption_with_its_expression_held_fixed_classifies_incomparable_and_is_reviewed()
 {
    // RFC 0031's declaredness rule, exercised on the real fixture: `DurableLogIsAPrefix`
    // is declared `"classification":"storage"`; move it to `"network"` with the
    // expression untouched.
    let before = fixture_assumptions(FIXTURE);
    let mutated = FIXTURE.replacen(
        r#""classification":"storage""#,
        r#""classification":"network""#,
        1,
    );
    assert_ne!(mutated, FIXTURE, "the replacement must actually fire");
    let after = fixture_assumptions(&mutated);

    let changes = classify_assumptions(&before, &after);
    let reclassified = changes
        .iter()
        .find(|c| c.unit().as_str() == "DurableLogIsAPrefix")
        .expect("DurableLogIsAPrefix is classified");
    assert_eq!(reclassified.relation(), Relation::Incomparable);

    let policy = fixture_policy(FIXTURE);
    let records = records_with_overrides(&[(PolicyField::Assumptions, Relation::Incomparable)]);
    let verdict = policy
        .verdict(
            &records,
            &fixture_reviewers(FIXTURE),
            AcceptancePath::AgentAccept,
        )
        .expect("a complete classification against a well-formed policy table computes");
    assert_eq!(verdict.decision(), PolicyDecision::Review);
}

// --- negative: disguises are never silent -----------------------------------------------------

#[test]
fn negative_renaming_a_real_corpus_assumption_while_preserving_its_expression_is_removed_plus_added()
 {
    let before = fixture_assumptions(FIXTURE);
    let mutated = FIXTURE.replacen(
        r#""id":"QuorumIntersection""#,
        r#""id":"QuorumIntersectionV2""#,
        1,
    );
    assert_ne!(mutated, FIXTURE, "the replacement must actually fire");
    let after = fixture_assumptions(&mutated);

    let changes = classify_assumptions(&before, &after);
    assert_eq!(changes.len(), 4, "{changes:?}");
    let old = changes
        .iter()
        .find(|c| c.unit().as_str() == "QuorumIntersection")
        .expect("the old id is classified");
    let new = changes
        .iter()
        .find(|c| c.unit().as_str() == "QuorumIntersectionV2")
        .expect("the new id is classified");
    assert_eq!(old.relation(), Relation::Removed);
    assert_eq!(new.relation(), Relation::Added);
    assert!(
        changes.iter().all(|c| c.relation() != Relation::Unchanged
            || c.unit().as_str() != "QuorumIntersection"
                && c.unit().as_str() != "QuorumIntersectionV2"),
        "the rename must never collapse into a single unchanged record"
    );
}

#[test]
fn negative_rewriting_a_real_corpus_assumptions_source_alone_still_classifies_unchanged() {
    // R2 / "CPNF-1 interaction": `source` is display-only and MUST NOT contribute to
    // classification, so this must stay `unchanged` — the mirror-image check to the
    // rename test above, confirming the classifier is neither too strict nor too
    // lenient.
    let before = fixture_assumptions(FIXTURE);
    let mutated = FIXTURE.replacen(
        r#""source":"allow_volatile_suffix_loss, allow_torn_last_record""#,
        r#""source":"a completely different English rendering of the same fact""#,
        1,
    );
    assert_ne!(mutated, FIXTURE, "the replacement must actually fire");
    let after = fixture_assumptions(&mutated);

    let changes = classify_assumptions(&before, &after);
    let unchanged = changes
        .iter()
        .find(|c| c.unit().as_str() == "DurableLogIsAPrefix")
        .expect("DurableLogIsAPrefix is classified");
    assert_eq!(unchanged.relation(), Relation::Unchanged);
}

// --- pin: no content edit ever emits a guessed direction ---------------------------------------

#[test]
fn pin_no_content_edit_across_a_sweep_of_real_fixture_mutations_ever_emits_strengthened_or_weakened()
 {
    // S2 / the fail-closed rule: "unequal encodings ⇒ no direction" unless an
    // obligation is discharged. Every mutation in this sweep is an atom rename,
    // which the Finite oracle (wired in by bn-2nwpg after this file landed) cannot
    // relate in either direction — so every one of these real-fixture content
    // edits, including ones that read, in English, as obviously narrowing or
    // obviously widening, must still land on `unknown`, never on a guessed
    // `strengthened`/`weakened`. The pinned expectations are unchanged; only this
    // comment's reason moved from "no oracle exists" to "the oracle proves
    // nothing here, and a failed derivation refutes nothing".
    let before = fixture_assumptions(FIXTURE);
    let mutations: [(&str, &str); 3] = [
        (r#""name":"was_sent""#, r#""name":"was_definitely_sent""#),
        (
            r#""name":"durable_log_is_a_prefix_of_appended""#,
            r#""name":"durable_log_is_a_strict_prefix_of_appended""#,
        ),
        (r#""name":"Nodes""#, r#""name":"Servers""#),
    ];
    for (pattern, replacement) in mutations {
        let mutated = FIXTURE.replacen(pattern, replacement, 1);
        assert_ne!(mutated, FIXTURE, "{pattern:?} must actually fire");
        let after = fixture_assumptions(&mutated);
        let changes = classify_assumptions(&before, &after);
        assert!(
            changes
                .iter()
                .all(|c| !matches!(c.relation(), Relation::Strengthened | Relation::Weakened)),
            "mutation {pattern:?} -> {replacement:?} produced a guessed direction: {changes:?}"
        );
    }
}

// --- anti-vacuity mutants, applied to the real fixture's own attack cases --------------------

/// A plausible, *wrong* classifier: decides a same-key comparison purely by whether
/// the expression's canonical bytes match, never looking at `classification` or
/// `fidelity_profile` at all — misses the reclassification attack entirely.
fn mutant_blind_to_declaredness(before: &AssumptionSet, after: &AssumptionSet, id: &str) -> bool {
    let key = continuum_intent::assumptions::UnitKey::new(id).expect("test id is non-empty");
    let (Some(b), Some(a)) = (before.get(&key), after.get(&key)) else {
        return false;
    };
    b.expression().ast() == a.expression().ast()
}

#[test]
fn negative_mutant_blind_to_declaredness_would_wrongly_pass_a_reclassification_as_unchanged_on_the_real_fixture()
 {
    let before = fixture_assumptions(FIXTURE);
    let mutated = FIXTURE.replacen(
        r#""classification":"storage""#,
        r#""classification":"network""#,
        1,
    );
    assert_ne!(mutated, FIXTURE);
    let after = fixture_assumptions(&mutated);

    let real = classify_assumptions(&before, &after)
        .into_iter()
        .find(|c| c.unit().as_str() == "DurableLogIsAPrefix")
        .expect("classified")
        .relation();
    assert_eq!(real, Relation::Incomparable);

    assert!(
        mutant_blind_to_declaredness(&before, &after, "DurableLogIsAPrefix"),
        "the mutant must actually get this wrong (report no change), or it is not \
         exercising the bug"
    );
}

/// A plausible, *wrong* classifier: matches units by expression content instead of by
/// `id`, so a clean rename with a preserved expression reads as `unchanged` rather
/// than `removed` + `added` — the exact disguise the properties/observers/fairness
/// text all name independently for their own fields.
fn mutant_matches_by_expression_instead_of_id(
    before: &AssumptionSet,
    after: &AssumptionSet,
) -> bool {
    let before_asts: Vec<_> = before.iter().map(|a| a.expression().ast()).collect();
    let after_asts: Vec<_> = after.iter().map(|a| a.expression().ast()).collect();
    before_asts.len() == after_asts.len() && before_asts.iter().all(|b| after_asts.contains(b))
}

#[test]
fn negative_mutant_matching_by_expression_instead_of_id_would_wrongly_pass_the_real_fixtures_rename_as_unchanged()
 {
    let before = fixture_assumptions(FIXTURE);
    let mutated = FIXTURE.replacen(
        r#""id":"QuorumIntersection""#,
        r#""id":"QuorumIntersectionV2""#,
        1,
    );
    assert_ne!(mutated, FIXTURE);
    let after = fixture_assumptions(&mutated);

    let real_changes = classify_assumptions(&before, &after);
    assert!(
        real_changes
            .iter()
            .all(|c| c.relation() != Relation::Unchanged
                || (c.unit().as_str() != "QuorumIntersection"
                    && c.unit().as_str() != "QuorumIntersectionV2")),
        "the real classifier never treats the rename as unchanged"
    );

    assert!(
        mutant_matches_by_expression_instead_of_id(&before, &after),
        "the mutant must actually get this wrong (report no change), or it is not \
         exercising the bug"
    );
}
