//! PR-12 / IMPL-06 exit evidence — fault/fairness/assurance change classification
//! (`bn-3vxp`).
//!
//! # What this file is evidence for
//!
//! `src/faults.rs`, `src/fairness.rs`, and `src/assurance.rs` classify `faults`,
//! `fairness`, and `assurance` per RFC 0031's rules for those three fields. This file
//! is the dedicated adversarial and clause-mapped evidence the bone brief asks for,
//! on top of those modules' own unit tests (which stay the source of truth for the
//! componentwise/membership mechanics and are not re-derived here), matching
//! `tests/pr12_impl05_observer_event_change_evidence.rs`'s shape.
//!
//! # Clause → test map
//!
//! | RFC 0031 / plan clause | Test |
//! |---|---|
//! | "`removed` is the 'removing crash-after-submit' attack and is what `no-removal` blocks" | [`positive_removing_a_fault_class_on_a_real_corpus_fixture_classifies_removed_and_blocks`] |
//! | plan §5.1 "remove crash-after-submit" → `faults` → `removed` → `no-removal` | same test above |
//! | RFC 0031's `kind` movement, "weak → strong... classifies strengthened" | [`positive_strengthening_the_real_fixtures_fairness_constraint_classifies_strengthened_and_is_reviewed`] |
//! | plan §5.1 "add a fairness assumption that schedules away the bug" → `fairness` → `added`/`strengthened` → `review` | same test above |
//! | RFC 0031 gaming-move row: "lower assurance from exhaustive to sampled" → `assurance` → `downgraded` → `no-downgrade` | [`positive_lowering_the_real_fixtures_assurance_minimum_classifies_downgraded_and_blocks`] |
//! | AO4 (RFC 0037 correction 17): `no-removal` over an empty `fault_model` is dormant | [`the_die_hard_fixtures_empty_fault_model_under_no_removal_is_dormant_not_blocking`] |
//! | "swap one fault class for another is never silent" | [`negative_swapping_one_fault_class_for_another_is_never_silent`] |
//! | "renaming... is removed plus added, not unchanged" (fairness analogue) | [`negative_renaming_a_fairness_action_is_removed_plus_added_never_unchanged`] |
//! | RFC 0031 "Assurance movement": declaredness change classifies `unknown`, fails closed | [`assurance_declaredness_change_on_the_real_fixture_classifies_unknown_and_reviews_not_allows`] |
//! | "`incomparable` MUST NOT be emitted for `assurance`" | [`pin_assurance_never_emits_incomparable_across_a_sweep_of_real_fixture_mutations`] |
//! | anti-vacuity: the positive tests are not vacuously true | [`negative_mutant_blind_to_fault_membership_would_wrongly_pass_the_removal_as_unchanged`], [`negative_mutant_ignoring_assurance_checkers_would_wrongly_pass_the_drop_as_unchanged`], [`negative_mutant_flagging_every_assurance_move_as_downgraded_would_wrongly_block_a_benign_upgrade`] |
//!
//! # House rules, inherited from the PR-4/INV-012 evidence precedent
//!
//! - `src/` is untouched by this file, and no existing test anywhere is edited.
//! - [`FIXTURE`] is the same real, already-reviewed corpus document
//!   `pr12_impl05_observer_event_change_evidence.rs` and `crate::bounds`'s own tests
//!   read (`crates/continuum-intent/tests/fixtures/replicated-register-contract.json`)
//!   — the richest available for `fault_model`/`fairness`/`assurance` content among
//!   this crate's fixtures (six enabled fault classes, two profiles, one fairness
//!   constraint, and a full assurance requirement). [`DORMANT_FIXTURE`] is
//!   `die-hard-contract.json`, whose `fault_model.enabled` is empty under a
//!   `no-removal` verb — the AO4 shape the bone brief names directly. Every mutation
//!   below is one bounded, linear `.replacen` on one of these two fixed, small
//!   strings — never a loop of doublings, never nested growth.
//! - Positive, negative, and boundary evidence are each present and separately named.

use continuum_intent::assurance_policy::AssurancePolicy;
use continuum_intent::canonical_json::Json;
use continuum_intent::change_policy::{
    AcceptancePath, ClassificationRecord, PolicyDecision, PolicyField, PolicyReviewers,
    PolicyTable, PolicyVerb, Relation,
};
use continuum_intent::fairness::{
    ActionName, FairnessConstraint, FairnessKey, FairnessKind, FairnessSet,
};
use continuum_intent::faults::{FaultClass, FaultModel};
use continuum_semantic_diff::assurance::classify_assurance;
use continuum_semantic_diff::fairness::{FairnessUnit, classify_fairness};
use continuum_semantic_diff::faults::classify_faults;

/// The real corpus fixture: the replicated-register Intent Contract, also used by
/// `crate::observers`' and `crate::bounds`' own evidence, and by `continuum-intent`'s
/// PR-4 exit and INV-012 evidence.
const FIXTURE: &str =
    include_str!("../../continuum-intent/tests/fixtures/replicated-register-contract.json");

/// The die-hard fixture: an empty `fault_model.enabled` under `policy.faults =
/// "no-removal"` — AO4's "dormant, not ill-formed" shape (RFC 0037 correction 17).
const DORMANT_FIXTURE: &str =
    include_str!("../../continuum-intent/tests/fixtures/die-hard-contract.json");

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

fn fixture_fault_model(text: &str) -> FaultModel {
    let parsed = document(text);
    FaultModel::from_json(fixture_field(&parsed, "fault_model"))
        .expect("the fixture's fault_model decodes")
}

fn fixture_fairness(text: &str) -> FairnessSet {
    let parsed = document(text);
    FairnessSet::from_json(fixture_field(&parsed, "fairness"))
        .expect("the fixture's fairness decodes")
}

fn fixture_assurance(text: &str) -> AssurancePolicy {
    let parsed = document(text);
    AssurancePolicy::from_json(fixture_field(&parsed, "assurance"))
        .expect("the fixture's assurance decodes")
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
/// `pr12_impl05_observer_event_change_evidence.rs`'s identical helper inline: P1
/// needs one record per field, and every field but the one(s) under test is honestly
/// unchanged in these scenarios (only one field was mutated per test).
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
fn the_fixtures_fault_model_has_six_classes_and_two_profiles() {
    let model = fixture_fault_model(FIXTURE);
    assert_eq!(model.enabled().len(), 6);
    assert_eq!(model.profiles().len(), 2);
    let changes = classify_faults(&model, &model);
    assert_eq!(changes.len(), 8);
    assert!(changes.iter().all(|c| c.relation() == Relation::Unchanged));
}

#[test]
fn the_fixtures_one_fairness_constraint_is_weak_recover_unconditional() {
    let set = fixture_fairness(FIXTURE);
    assert_eq!(set.len(), 1);
    let recover = set
        .get(&FairnessKey::new(
            FairnessKind::Weak,
            ActionName::new("Recover").unwrap(),
        ))
        .expect("the fixture names a weak:Recover constraint");
    assert!(recover.is_unconditional());
    let changes = classify_fairness(&set, &set);
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].relation(), Relation::Unchanged);
}

#[test]
fn the_fixtures_assurance_is_validated_with_both_checkers_on() {
    let policy = fixture_assurance(FIXTURE);
    assert_eq!(
        policy.minimum(),
        continuum_intent::assurance_policy::level_from_wire("validated")
            .expect("\"validated\" is one of the five assurance levels")
    );
    assert_eq!(policy.independent_checker(), Some(true));
    assert_eq!(policy.clean_recompute(), Some(true));
    assert_eq!(
        classify_assurance(&policy, &policy).relation(),
        Relation::Unchanged
    );
}

// --- positive: the named attacks, on the real fixture, correctly caught and blocked ---------

#[test]
fn positive_removing_a_fault_class_on_a_real_corpus_fixture_classifies_removed_and_blocks() {
    // RFC 0031's own gaming-move example, applied to the fixture's `crash` class
    // exactly as "removing crash-after-submit" names it.
    let before = fixture_fault_model(FIXTURE);
    let mutated = FIXTURE.replacen(r#""crash","#, "", 1);
    assert_ne!(mutated, FIXTURE, "the replacement must actually fire");
    let after = fixture_fault_model(&mutated);

    let changes = classify_faults(&before, &after);
    let crash_change = changes
        .iter()
        .find(|c| c.unit().locator() == "crash")
        .expect("crash is classified");
    assert_eq!(crash_change.relation(), Relation::Removed);

    let policy = fixture_policy(FIXTURE);
    assert_eq!(policy.verb(PolicyField::Faults), PolicyVerb::NoRemoval);
    let records = records_with_overrides(&[(PolicyField::Faults, Relation::Removed)]);
    let verdict = policy
        .verdict(
            &records,
            &fixture_reviewers(FIXTURE),
            AcceptancePath::AgentAccept,
        )
        .expect("a complete classification against a well-formed policy table computes");
    assert_eq!(verdict.decision(), PolicyDecision::Block);
    assert!(
        verdict
            .reasons()
            .iter()
            .any(|r| r.field() == PolicyField::Faults && r.relation() == Relation::Removed),
        "{:?}",
        verdict.reasons()
    );
}

#[test]
fn positive_strengthening_the_real_fixtures_fairness_constraint_classifies_strengthened_and_is_reviewed()
 {
    // The fixture's one constraint, `weak:Recover`, strengthened to `strong:Recover`
    // with its (unconditional) condition held fixed — RFC 0031's `kind` movement,
    // collapsed to the single-record encoding the RFC fixes.
    let before = fixture_fairness(FIXTURE);
    let mutated = FIXTURE.replacen(r#""kind":"weak""#, r#""kind":"strong""#, 1);
    assert_ne!(mutated, FIXTURE, "the replacement must actually fire");
    let after = fixture_fairness(&mutated);

    let changes = classify_fairness(&before, &after);
    assert_eq!(changes.len(), 1, "{changes:?}");
    assert_eq!(changes[0].relation(), Relation::Strengthened);
    assert!(matches!(
        changes[0].unit(),
        FairnessUnit::KindChange {
            before: FairnessKind::Weak,
            after: FairnessKind::Strong,
            ..
        }
    ));

    let policy = fixture_policy(FIXTURE);
    assert_eq!(policy.verb(PolicyField::Fairness), PolicyVerb::Review);
    let records = records_with_overrides(&[(PolicyField::Fairness, Relation::Strengthened)]);
    let verdict = policy
        .verdict(
            &records,
            &fixture_reviewers(FIXTURE),
            AcceptancePath::AgentAccept,
        )
        .expect("a complete classification against a well-formed policy table computes");
    assert_eq!(verdict.decision(), PolicyDecision::Review);
    assert!(
        verdict
            .reasons()
            .iter()
            .any(|r| r.field() == PolicyField::Fairness && r.relation() == Relation::Strengthened),
        "{:?}",
        verdict.reasons()
    );
    // W7: the fixture names a reviewer for `fairness`.
    assert!(
        verdict
            .reasons()
            .iter()
            .any(|r| r.field() == PolicyField::Fairness && !r.reviewers().is_empty()),
        "{:?}",
        verdict.reasons()
    );
}

#[test]
fn positive_lowering_the_real_fixtures_assurance_minimum_classifies_downgraded_and_blocks() {
    // RFC 0031's own gaming-move example, verbatim in spirit: "lower assurance from
    // exhaustive to sampled" — here, `validated` down to `bounded`.
    let before = fixture_assurance(FIXTURE);
    let mutated = FIXTURE.replacen(r#""minimum":"validated""#, r#""minimum":"bounded""#, 1);
    assert_ne!(mutated, FIXTURE, "the replacement must actually fire");
    let after = fixture_assurance(&mutated);

    assert_eq!(
        classify_assurance(&before, &after).relation(),
        Relation::Downgraded
    );

    let policy = fixture_policy(FIXTURE);
    assert_eq!(policy.verb(PolicyField::Assurance), PolicyVerb::NoDowngrade);
    let records = records_with_overrides(&[(PolicyField::Assurance, Relation::Downgraded)]);
    let verdict = policy
        .verdict(
            &records,
            &fixture_reviewers(FIXTURE),
            AcceptancePath::AgentAccept,
        )
        .expect("a complete classification against a well-formed policy table computes");
    assert_eq!(verdict.decision(), PolicyDecision::Block);
    assert!(
        verdict
            .reasons()
            .iter()
            .any(|r| r.field() == PolicyField::Assurance && r.relation() == Relation::Downgraded),
        "{:?}",
        verdict.reasons()
    );
}

// --- AO4: dormant, not ill-formed, not blocking ----------------------------------------------

#[test]
fn the_die_hard_fixtures_empty_fault_model_under_no_removal_is_dormant_not_blocking() {
    let policy = fixture_policy(DORMANT_FIXTURE);
    assert_eq!(policy.verb(PolicyField::Faults), PolicyVerb::NoRemoval);
    let model = fixture_fault_model(DORMANT_FIXTURE);
    assert!(model.enabled().is_empty());

    // Comparing the empty model against itself: no units, nothing to classify.
    assert!(classify_faults(&model, &model).is_empty());

    // A complete, all-`unchanged` classification against a `no-removal` lock over an
    // empty set must `allow` — the lock has nothing to guard yet (RFC 0037
    // correction 17, AO4), and this classifier's total silence on an unmutated,
    // empty field is the honest input to that verdict, not a special case.
    let records = records_with_overrides(&[]);
    let verdict = policy
        .verdict(
            &records,
            &fixture_reviewers(DORMANT_FIXTURE),
            AcceptancePath::AgentAccept,
        )
        .expect("a complete classification against a well-formed policy table computes");
    assert_eq!(verdict.decision(), PolicyDecision::Allow);
}

// --- negative: disguises are never silent -----------------------------------------------------

#[test]
fn negative_swapping_one_fault_class_for_another_is_never_silent() {
    let before = FaultModel::new([FaultClass::Crash], []).expect("well formed");
    let after = FaultModel::new([FaultClass::Recovery], []).expect("well formed");
    let changes = classify_faults(&before, &after);
    assert_eq!(changes.len(), 2, "{changes:?}");
    assert!(
        changes
            .iter()
            .any(|c| c.unit().locator() == "crash" && c.relation() == Relation::Removed)
    );
    assert!(
        changes
            .iter()
            .any(|c| c.unit().locator() == "recovery" && c.relation() == Relation::Added)
    );
    // Non-affirmative-or-affirmative-weakening either way: neither record is
    // `unchanged`, so `PolicyTable::verdict`'s P4 cannot wave this through as a no-op
    // under any verb that isn't itself `unlocked`.
    assert!(changes.iter().all(|c| c.relation() != Relation::Unchanged));
}

#[test]
fn negative_renaming_a_fairness_action_is_removed_plus_added_never_unchanged() {
    // Parallel to `observers`' and `properties`' rename disguise: an identical
    // condition under a new action key must not read as one unchanged unit.
    let before_key = FairnessKey::new(FairnessKind::Weak, ActionName::new("Recover").unwrap());
    let after_key = FairnessKey::new(FairnessKind::Weak, ActionName::new("RecoverV2").unwrap());
    let before =
        FairnessSet::from_constraints([FairnessConstraint::new(before_key.clone(), None).unwrap()])
            .unwrap();
    let after =
        FairnessSet::from_constraints([FairnessConstraint::new(after_key.clone(), None).unwrap()])
            .unwrap();
    let changes = classify_fairness(&before, &after);
    assert_eq!(changes.len(), 2, "{changes:?}");
    assert!(changes.iter().any(|c| matches!(
        c.unit(),
        FairnessUnit::Key(k) if *k == before_key
    ) && c.relation() == Relation::Removed));
    assert!(changes.iter().any(|c| matches!(
        c.unit(),
        FairnessUnit::Key(k) if *k == after_key
    ) && c.relation() == Relation::Added));
}

// --- assurance: declaredness fails closed, even on the real fixture --------------------------

#[test]
fn assurance_declaredness_change_on_the_real_fixture_classifies_unknown_and_reviews_not_allows() {
    let before = fixture_assurance(FIXTURE);
    let mutated = FIXTURE.replacen(r#""independent_checker":true,"#, "", 1);
    assert_ne!(mutated, FIXTURE, "the replacement must actually fire");
    let after = fixture_assurance(&mutated);
    assert_eq!(before.independent_checker(), Some(true));
    assert_eq!(after.independent_checker(), None);

    assert_eq!(
        classify_assurance(&before, &after).relation(),
        Relation::Unknown
    );

    let policy = fixture_policy(FIXTURE);
    assert_eq!(policy.verb(PolicyField::Assurance), PolicyVerb::NoDowngrade);
    let records = records_with_overrides(&[(PolicyField::Assurance, Relation::Unknown)]);
    let verdict = policy
        .verdict(
            &records,
            &fixture_reviewers(FIXTURE),
            AcceptancePath::AgentAccept,
        )
        .expect("a complete classification against a well-formed policy table computes");
    // P2: non-affirmative contributes at least `review`, and `block` only when the
    // field's verb is `locked` — `no-downgrade` is not `locked`, so this is `review`,
    // not `allow` and not `block`.
    assert_eq!(verdict.decision(), PolicyDecision::Review);
}

// --- pin: `incomparable` is never emitted for `assurance`, even under real-corpus mutation ----

#[test]
fn pin_assurance_never_emits_incomparable_across_a_sweep_of_real_fixture_mutations() {
    let before = fixture_assurance(FIXTURE);
    let mutations: [&str; 5] = [
        r#""minimum":"validated""#,
        r#""minimum":"validated""#,
        r#""independent_checker":true,"#,
        r#""clean_recompute":true,"#,
        r#""minimum":"validated""#,
    ];
    let replacements: [&str; 5] = [
        r#""minimum":"observed""#,
        r#""minimum":"proved""#,
        "",
        "",
        r#""minimum":"sampled""#,
    ];
    for (pattern, replacement) in mutations.iter().zip(replacements.iter()) {
        let mutated = FIXTURE.replacen(pattern, replacement, 1);
        assert_ne!(mutated, FIXTURE);
        let after = fixture_assurance(&mutated);
        assert_ne!(
            classify_assurance(&before, &after).relation(),
            Relation::Incomparable,
            "assurance must never emit incomparable, mutation {pattern:?} -> {replacement:?}"
        );
    }
}

// --- anti-vacuity mutants, applied to the real fixture's own attack cases --------------------

/// A plausible, *wrong* faults classifier: reports a change only when the total
/// enabled-class count differs, never checking *which* classes are present.
fn mutant_blind_to_fault_membership(before: &FaultModel, after: &FaultModel) -> bool {
    before.enabled().len() == after.enabled().len()
}

#[test]
fn negative_mutant_blind_to_fault_membership_would_wrongly_pass_the_removal_as_unchanged() {
    // The real fixture's crash-removal attack, again added a decoy so the mutant's
    // cardinality check is fooled: remove `crash`, but the fixture already has six
    // classes, so instead exercise the mutant directly against a same-size scenario
    // it is blind to.
    let before = FaultModel::new([FaultClass::Crash, FaultClass::Recovery], []).unwrap();
    let after = FaultModel::new([FaultClass::Recovery, FaultClass::Delay], []).unwrap();

    let real_changes = classify_faults(&before, &after);
    assert!(
        real_changes
            .iter()
            .any(|c| c.unit().locator() == "crash" && c.relation() == Relation::Removed),
        "the real classifier sees `crash` removed even though the total count held"
    );

    assert!(
        mutant_blind_to_fault_membership(&before, &after),
        "the mutant must actually get this wrong (report no change), or it is not exercising \
         the bug"
    );
}

/// A plausible, *wrong* assurance classifier: decides purely by `minimum`, ignoring
/// both checker flags — misses the checker-dropping attack (over-lenient direction).
fn mutant_ignoring_assurance_checkers(before: &AssurancePolicy, after: &AssurancePolicy) -> bool {
    before.minimum() == after.minimum()
}

#[test]
fn negative_mutant_ignoring_assurance_checkers_would_wrongly_pass_the_drop_as_unchanged() {
    let before = fixture_assurance(FIXTURE);
    let mutated = FIXTURE.replacen(
        r#""independent_checker":true"#,
        r#""independent_checker":false"#,
        1,
    );
    assert_ne!(mutated, FIXTURE);
    let after = fixture_assurance(&mutated);

    assert_eq!(
        classify_assurance(&before, &after).relation(),
        Relation::Downgraded
    );
    assert!(
        mutant_ignoring_assurance_checkers(&before, &after),
        "the mutant must actually get this wrong (report no change), or it is not exercising \
         the bug"
    );
}

/// A plausible, *wrong* assurance classifier in the opposite direction: flags *any*
/// inequality as a downgrade, ignoring the requirement's own order — would falsely
/// block a benign upgrade (over-strict direction).
fn mutant_flags_every_move_as_downgraded(
    before: &AssurancePolicy,
    after: &AssurancePolicy,
) -> bool {
    before.minimum() != after.minimum()
        || before.independent_checker() != after.independent_checker()
        || before.clean_recompute() != after.clean_recompute()
}

#[test]
fn negative_mutant_flagging_every_assurance_move_as_downgraded_would_wrongly_block_a_benign_upgrade()
 {
    let before = fixture_assurance(FIXTURE);
    let mutated = FIXTURE.replacen(r#""minimum":"validated""#, r#""minimum":"proved""#, 1);
    assert_ne!(mutated, FIXTURE);
    let after = fixture_assurance(&mutated);

    assert_eq!(
        classify_assurance(&before, &after).relation(),
        Relation::Upgraded,
        "raising `minimum` alone, with both checkers held, is a genuine, benign upgrade"
    );
    assert!(
        mutant_flags_every_move_as_downgraded(&before, &after),
        "the mutant must actually get this wrong (a false alarm on a benign change), or it is \
         not exercising the bug"
    );
}
