//! PR-12 follow-up exit evidence — `assumptions` wired to the Finite implication
//! oracle (`bn-2nwpg`).
//!
//! # What this file is evidence for
//!
//! `bn-7vg7` landed `src/assumptions.rs` fail-closed: every unequal expression
//! classified `unknown`, because no Finite-fragment inclusion oracle existed yet.
//! `bn-8mlg` landed that oracle in `src/properties.rs`
//! (`finite_formula_relation` — sound, bounded, fail-closed). This bone wires the
//! assumptions content axis to it through the authority's own shared license arm,
//! consuming the oracle's relation **directly, with no dualize** — the polarity the
//! assumptions module doc derives from RFC 0031 ("exactly as for claims — the same
//! order") against `fairness[].condition`'s explicitly antitone contrast. This file
//! is the dedicated real-corpus evidence: the upgrade is observable (an affirmed
//! direction where `bn-7vg7` pinned `unknown`), in the safe direction (still
//! reviewed under the fixture's own verb, blocked under a `locked` one, never
//! allowed), and at the right polarity (a real in-place strengthening classifies
//! `strengthened`, never the safe-looking `weakened` a dualized wiring would
//! report).
//!
//! # Clause → test map
//!
//! | RFC 0031 / plan clause | Test |
//! |---|---|
//! | "an assumption `strengthened` admits fewer environments and makes verification easier, and is the gaming move plan §5.1 names" — now *affirmed*, not merely suspected | [`positive_dropping_the_real_fixtures_forgery_escape_disjunct_classifies_an_affirmed_strengthened_and_is_reviewed`] |
//! | the same decided strengthening under a `locked` verb blocks | [`positive_the_same_shape_of_in_place_strengthening_under_die_hards_locked_verb_blocks`] |
//! | "`weakened` iff behaviors(before) ⊆ behaviors(after)" — the mirror, so neither directional arm is vacuous | [`positive_restoring_the_dropped_disjunct_classifies_an_affirmed_weakened_and_is_reviewed`] |
//! | "the classifier MUST NOT match claims by expression to defeat the rename" (same order for assumptions) | [`negative_a_strengthening_dressed_as_a_rename_is_removed_plus_added_never_a_direction`] |
//! | the oracle is never consulted across a declaredness move (`bn-7vg7`'s compound-silence resolution, preserved) | [`negative_a_strengthening_riding_a_reclassification_still_classifies_unknown_and_is_reviewed`] |
//! | R2 / "CPNF-1 interaction": `source` MUST NOT contribute — a cosmetic-looking `source` cannot mask the direction | [`negative_a_source_rewrite_riding_the_strengthening_does_not_change_the_affirmed_direction`] |
//! | S2 / fail-closed: an edit the bounded machinery cannot relate is `unknown`, never a guess | [`boundary_an_in_place_edit_the_oracle_cannot_relate_still_classifies_unknown_and_is_reviewed`] |
//! | boundedness: the step budget is a real, observable boundary through this field | [`boundary_the_step_budget_fails_closed_through_the_assumptions_field_and_is_reviewed`] |
//! | S3: directions only under the fragment's own decision procedure | [`negative_mutant_ignoring_the_fragment_license_would_affirm_a_direction_on_an_unlicensed_pair`] |
//! | polarity anti-vacuity, both directions: a dualized consumption is wrong on the real corpus | [`negative_mutant_dualizing_the_oracle_is_wrong_on_the_real_corpus_in_both_directions`] |
//!
//! # House rules, inherited from the PR-12 evidence precedent
//!
//! - `src/` is exercised through its public surface only; this file edits no
//!   existing test.
//! - [`FIXTURE`] is the same real, already-reviewed corpus document every sibling
//!   evidence file reads (`replicated-register-contract.json`) — three real
//!   assumptions, every expression declared `Finite`, and the fixture's own
//!   `policy.assumptions` verb is `review` with a named reviewer. [`DIE_HARD`] is
//!   the campaign's second real fixture, whose `policy.assumptions` verb is
//!   **`locked`** — the block path. Every fixture mutation below is one bounded,
//!   linear `.replacen` on one fixed, small string; the two programmatic
//!   constructions (the budget boundary, the unlicensed pair) are flat linear
//!   vectors of distinct atoms, stated where they happen.
//! - Positive, negative, and boundary evidence are each present and separately
//!   named. All tests are deterministic (INV-005): fixed inputs, fixed budgets, no
//!   clock, no entropy, no map-order dependence.

use continuum_intent::assumptions::{Assumption, AssumptionSet, UnitKey};
use continuum_intent::ast::{Formula, Fragment, Identifier};
use continuum_intent::canonical_json::Json;
use continuum_intent::change_policy::{
    AcceptancePath, ClassificationRecord, PolicyDecision, PolicyField, PolicyReviewers,
    PolicyTable, PolicyVerb, Relation,
};
use continuum_intent::property::PropertyExpression;
use continuum_semantic_diff::assumptions::classify_assumptions;
use continuum_semantic_diff::properties::finite_formula_relation;

/// The real corpus fixture: the replicated-register Intent Contract, also used by
/// the six PR-12 evidence files and the DX-02 falsification campaign.
const FIXTURE: &str =
    include_str!("../../continuum-intent/tests/fixtures/replicated-register-contract.json");

/// The campaign's second real fixture: Die Hard, whose `policy.assumptions` verb is
/// `locked` and whose one assumption (`JugCapacities`) is declared `Finite`.
const DIE_HARD: &str = include_str!("../../continuum-intent/tests/fixtures/die-hard-contract.json");

/// The disjunction inside the fixture's `NetworkNoForgery` assumption —
/// `was_sent OR NOT occurs(Deliver)` ("every delivered message was sent, unless
/// nothing was delivered"). Unique in the document; the tests that mutate around it
/// assert their replacement fires.
const NO_FORGERY_DISJUNCTION: &str = r#"{"kind":"or","operands":[{"args":[],"kind":"predicate","name":"was_sent"},{"kind":"not","operand":{"kind":"action","modality":"occurs","name":"Deliver"}}]}"#;

/// The strengthened replacement: the bare `was_sent` predicate. Dropping the
/// `NOT occurs(Deliver)` escape turns "delivered messages were sent" into "every
/// message was sent, always" — fewer environments admitted, verification easier:
/// plan §5.1/§5.3's assumption-strengthening move in its most literal costume,
/// arriving in valid CPNF-1 through the front door (`always(p)` needs no junction,
/// so no N4 ordering is disturbed).
const NO_FORGERY_STRENGTHENED: &str = r#"{"args":[],"kind":"predicate","name":"was_sent"}"#;

/// Die Hard's `JugCapacities` conjunction head. Inserting a predicate conjunct
/// before it keeps N4's sorted operand order (`{"args"` < `{"kind":"compare"` in
/// N8 byte order), so the mutated document is still valid CPNF-1 and decodes.
const JUG_CONJUNCTION_HEAD: &str =
    r#""operands":[{"kind":"compare","left":{"indices":[],"kind":"state","name":"big"}"#;

/// The extra conjunct: a third constraint on the environment, making the
/// assumption strictly stronger via the oracle's conjunct rules.
const JUG_EXTRA_CONJUNCT: &str = r#"{"args":[],"kind":"predicate","name":"taps_are_available"}"#;

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

fn relation_of(
    changes: &[continuum_semantic_diff::assumptions::AssumptionChange],
    id: &str,
) -> Relation {
    changes
        .iter()
        .find(|c| c.unit().as_str() == id)
        .unwrap_or_else(|| panic!("no record for unit {id:?} in {changes:?}"))
        .relation()
}

/// A complete `PolicyTable::verdict` records slice: `Unchanged` on every field
/// except the ones named in `overrides` — P1 needs one record per field, and every
/// field but `assumptions` is honestly unchanged in these scenarios. Mirrors the
/// identical helper in the impl02/impl03/impl06 evidence files.
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

/// Close an `assumptions` relation into the named fixture's own verdict path,
/// asserting P6 names the record, and return the decision.
fn verdict_of(fixture: &str, relation: Relation) -> PolicyDecision {
    let verdict = fixture_policy(fixture)
        .verdict(
            &records_with_overrides(&[(PolicyField::Assumptions, relation)]),
            &fixture_reviewers(fixture),
            AcceptancePath::AgentAccept,
        )
        .expect("a complete classification against a well-formed policy table computes");
    if relation != Relation::Unchanged {
        assert!(
            verdict
                .reasons()
                .iter()
                .any(|r| r.field() == PolicyField::Assumptions && r.relation() == relation),
            "P6: the contributing record must be named: {:?}",
            verdict.reasons()
        );
    }
    verdict.decision()
}

fn ident(name: &str) -> Identifier {
    Identifier::new(name).expect("a test identifier is well formed")
}

fn predicate(name: &str) -> Formula {
    Formula::predicate(ident(name), Vec::new())
}

fn unit(id: &str) -> UnitKey {
    UnitKey::new(id).expect("a test unit key is non-empty")
}

fn assumption_with_fragment(id: &str, ast: &Formula, fragment: Option<Fragment>) -> Assumption {
    Assumption::new(
        unit(id),
        PropertyExpression::normalized(ast, fragment, None).expect("normalizes"),
        None,
        None,
    )
}

fn set(assumptions: impl IntoIterator<Item = Assumption>) -> AssumptionSet {
    AssumptionSet::from_assumptions(assumptions).expect("distinct ids in a test fixture")
}

// --- baseline: the two fixtures are what the evidence needs them to be ----------------------

#[test]
fn the_fixtures_carry_the_finite_license_and_the_two_verbs_this_evidence_closes_into() {
    // The license: every classified expression below declares `Finite` on both
    // sides, so the oracle arm is genuinely reachable on the real corpus — not a
    // synthetic license granted by the test.
    let assumptions = fixture_assumptions(FIXTURE);
    assert_eq!(assumptions.len(), 3);
    for a in assumptions.iter() {
        assert_eq!(a.expression().fragment(), Some(Fragment::Finite));
    }
    assert_eq!(
        fixture_policy(FIXTURE).verb(PolicyField::Assumptions),
        PolicyVerb::Review
    );

    let die_hard = fixture_assumptions(DIE_HARD);
    assert_eq!(die_hard.len(), 1);
    for a in die_hard.iter() {
        assert_eq!(a.expression().fragment(), Some(Fragment::Finite));
    }
    assert_eq!(
        fixture_policy(DIE_HARD).verb(PolicyField::Assumptions),
        PolicyVerb::Locked
    );
}

// --- positive: the named attack, decided and closed, in both verdict costumes ----------------

#[test]
fn positive_dropping_the_real_fixtures_forgery_escape_disjunct_classifies_an_affirmed_strengthened_and_is_reviewed()
 {
    // THE upgrade this bone delivers, observable on the real corpus: under
    // bn-7vg7's landing this exact edit fell to the fail-closed `unknown`
    // (`review` by P2's floor); the oracle now derives
    // behaviors(after) ⊆ behaviors(before) and the record carries the *affirmed*
    // `strengthened` — the RFC's own name for the gaming move — while the verdict
    // stays in the safe direction (`review` under the fixture's own verb, never
    // `allow`). Direct consumption: this must be `strengthened`, not the
    // `weakened` a dualized (fairness-shaped) wiring would report.
    let before = fixture_assumptions(FIXTURE);
    let mutated = FIXTURE.replacen(NO_FORGERY_DISJUNCTION, NO_FORGERY_STRENGTHENED, 1);
    assert_ne!(mutated, FIXTURE, "the replacement must actually fire");
    let after = fixture_assumptions(&mutated);

    let changes = classify_assumptions(&before, &after);
    let relation = relation_of(&changes, "NetworkNoForgery");
    assert_eq!(relation, Relation::Strengthened);
    assert!(
        relation.is_affirmative(),
        "the direction is decided, not suspected"
    );

    assert_eq!(verdict_of(FIXTURE, relation), PolicyDecision::Review);
}

#[test]
fn positive_the_same_shape_of_in_place_strengthening_under_die_hards_locked_verb_blocks() {
    // The block path: Die Hard's `assumptions` verb is `locked`. Adding a third
    // conjunct to `JugCapacities` admits strictly fewer environments; the oracle
    // derives the inclusion by conjunct rules, and the affirmed `strengthened`
    // closes into P3's `block` — a decided strengthening cannot pass a locked
    // field.
    let before = fixture_assumptions(DIE_HARD);
    let mutated = DIE_HARD.replacen(
        JUG_CONJUNCTION_HEAD,
        &format!(r#""operands":[{JUG_EXTRA_CONJUNCT},{{"kind":"compare","left":{{"indices":[],"kind":"state","name":"big"}}"#),
        1,
    );
    assert_ne!(mutated, DIE_HARD, "the replacement must actually fire");
    let after = fixture_assumptions(&mutated);

    let changes = classify_assumptions(&before, &after);
    let relation = relation_of(&changes, "JugCapacities");
    assert_eq!(relation, Relation::Strengthened);

    assert_eq!(verdict_of(DIE_HARD, relation), PolicyDecision::Block);
}

#[test]
fn positive_restoring_the_dropped_disjunct_classifies_an_affirmed_weakened_and_is_reviewed() {
    // The mirror on the same real expression, so neither directional arm is
    // vacuous and the polarity is pinned from both sides: widening the assumption
    // back out (`always(was_sent)` -> `always(was_sent OR NOT occurs(Deliver))`)
    // admits more environments — formula weakened IS assumption weakened, and the
    // verdict still refuses `allow` (a weakened assumption makes verification
    // *harder*, but the field is protected through `review`, not through a
    // directional verb — AO1).
    let mutated = FIXTURE.replacen(NO_FORGERY_DISJUNCTION, NO_FORGERY_STRENGTHENED, 1);
    assert_ne!(mutated, FIXTURE, "the replacement must actually fire");
    let before = fixture_assumptions(&mutated);
    let after = fixture_assumptions(FIXTURE);

    let changes = classify_assumptions(&before, &after);
    let relation = relation_of(&changes, "NetworkNoForgery");
    assert_eq!(relation, Relation::Weakened);

    assert_eq!(verdict_of(FIXTURE, relation), PolicyDecision::Review);
}

// --- negative: disguised strengthenings never classify weakened, unchanged, or silent -------

#[test]
fn negative_a_strengthening_dressed_as_a_rename_is_removed_plus_added_never_a_direction() {
    // The rename costume over the same strengthening: a direction is a statement
    // about ONE unit's two sides, so a strengthening that also moves the `id` is
    // membership (`removed`+`added`), never an affirmed direction smuggled across
    // keys and never `unchanged`.
    let before = fixture_assumptions(FIXTURE);
    let strengthened = FIXTURE.replacen(NO_FORGERY_DISJUNCTION, NO_FORGERY_STRENGTHENED, 1);
    let mutated = strengthened.replacen(
        r#""id":"NetworkNoForgery""#,
        r#""id":"NetworkNoForgeryV2""#,
        1,
    );
    assert_ne!(mutated, strengthened, "the rename must actually fire");
    let after = fixture_assumptions(&mutated);

    let changes = classify_assumptions(&before, &after);
    assert_eq!(relation_of(&changes, "NetworkNoForgery"), Relation::Removed);
    assert_eq!(relation_of(&changes, "NetworkNoForgeryV2"), Relation::Added);
    assert!(
        changes.iter().all(|c| !matches!(
            c.relation(),
            Relation::Strengthened | Relation::Weakened | Relation::Unchanged
        ) || (c.unit().as_str() != "NetworkNoForgery"
            && c.unit().as_str() != "NetworkNoForgeryV2")),
        "{changes:?}"
    );
}

#[test]
fn negative_a_strengthening_riding_a_reclassification_still_classifies_unknown_and_is_reviewed() {
    // bn-7vg7's compound-silence resolution, preserved by this bone and now
    // non-vacuous: the expression pair is one the oracle CAN decide (asserted),
    // but a coincident `classification` flip means the oracle is never consulted
    // — `unknown`, never a direction riding a declaredness move, mirroring
    // properties' meaning-axis rule.
    let before = fixture_assumptions(FIXTURE);
    let strengthened = FIXTURE.replacen(NO_FORGERY_DISJUNCTION, NO_FORGERY_STRENGTHENED, 1);
    let mutated = strengthened.replacen(
        r#""classification":"network""#,
        r#""classification":"trust""#,
        1,
    );
    assert_ne!(
        mutated, strengthened,
        "the reclassification must actually fire"
    );
    let after = fixture_assumptions(&mutated);

    let network_key = unit("NetworkNoForgery");
    let (b, a) = (
        before.get(&network_key).expect("present before"),
        after.get(&network_key).expect("present after"),
    );
    assert_eq!(
        finite_formula_relation(b.expression().ast(), a.expression().ast()),
        Relation::Strengthened,
        "the oracle must be able to decide this pair, or the pin is vacuous"
    );

    let changes = classify_assumptions(&before, &after);
    let relation = relation_of(&changes, "NetworkNoForgery");
    assert_eq!(relation, Relation::Unknown);
    assert_eq!(verdict_of(FIXTURE, relation), PolicyDecision::Review);
}

#[test]
fn negative_a_source_rewrite_riding_the_strengthening_does_not_change_the_affirmed_direction() {
    // The cosmetic costume: rewrite the display-only `source` to read like a
    // harmless prose cleanup while the AST underneath strengthens. `source` MUST
    // NOT contribute to classification in either direction — the strengthening is
    // still affirmed, and cannot be softened back to `unknown` by dressing.
    let before = fixture_assumptions(FIXTURE);
    let strengthened = FIXTURE.replacen(NO_FORGERY_DISJUNCTION, NO_FORGERY_STRENGTHENED, 1);
    let mutated = strengthened.replacen(
        r#""source":"allow_loss, allow_duplication, allow_reordering""#,
        r#""source":"editorial cleanup only, no semantic change""#,
        1,
    );
    assert_ne!(
        mutated, strengthened,
        "the source rewrite must actually fire"
    );
    let after = fixture_assumptions(&mutated);

    let changes = classify_assumptions(&before, &after);
    assert_eq!(
        relation_of(&changes, "NetworkNoForgery"),
        Relation::Strengthened
    );
}

// --- boundary: undecidable-therefore-closed, and the budget ---------------------------------

#[test]
fn boundary_an_in_place_edit_the_oracle_cannot_relate_still_classifies_unknown_and_is_reviewed() {
    // DX02-A04's own costume (the atom rename `was_sent` ->
    // `was_definitely_sent`): the oracle is consulted now, but two opaque atoms
    // are unrelatable in either direction and a failed derivation refutes nothing
    // — the honest answer is still `unknown`, reviewed, never allowed and never a
    // guessed direction. The campaign file pins the same fact end to end.
    let before = fixture_assumptions(FIXTURE);
    let mutated = FIXTURE.replacen(r#""name":"was_sent""#, r#""name":"was_definitely_sent""#, 1);
    assert_ne!(mutated, FIXTURE, "the replacement must actually fire");
    let after = fixture_assumptions(&mutated);

    let changes = classify_assumptions(&before, &after);
    let relation = relation_of(&changes, "NetworkNoForgery");
    assert_eq!(relation, Relation::Unknown);
    assert_eq!(verdict_of(FIXTURE, relation), PolicyDecision::Review);
}

#[test]
fn boundary_the_step_budget_fails_closed_through_the_assumptions_field_and_is_reviewed() {
    // Boundedness, observable through this field's public surface: the same
    // one-disjunct widening decides `weakened` at 8 atoms and exhausts
    // `IMPLICATION_STEP_BUDGET` into `unknown` at 400 — and the exhausted case
    // still closes into `review`, never `allow`. Flat linear vectors of distinct
    // atoms; no doubling loops.
    let disjuncts = |n: usize| -> Formula {
        Formula::or((0..n).map(|i| predicate(&format!("p{i:03}"))).collect())
            .expect("two or more operands")
    };
    for (atoms, expected) in [(8, Relation::Weakened), (400, Relation::Unknown)] {
        let before = set([assumption_with_fragment(
            "A",
            &disjuncts(atoms),
            Some(Fragment::Finite),
        )]);
        let after = set([assumption_with_fragment(
            "A",
            &disjuncts(atoms + 1),
            Some(Fragment::Finite),
        )]);
        let changes = classify_assumptions(&before, &after);
        assert_eq!(relation_of(&changes, "A"), expected, "at {atoms} atoms");
        assert_eq!(verdict_of(FIXTURE, expected), PolicyDecision::Review);
    }
}

// --- anti-vacuity mutants: license and polarity, both directions -----------------------------

/// A plausible, *wrong* classifier: consults the oracle without holding the S3
/// fragment license — affirming directions "the fragment's own decision procedure"
/// never granted.
fn mutant_ignores_the_fragment_license(before: &Assumption, after: &Assumption) -> Relation {
    finite_formula_relation(before.expression().ast(), after.expression().ast())
}

#[test]
fn negative_mutant_ignoring_the_fragment_license_would_affirm_a_direction_on_an_unlicensed_pair() {
    // A Symbolic-declared pair whose shape the oracle could decide: the real
    // classifier fails closed (`unknown` — no license), the fail-open mutant
    // affirms `strengthened` anyway.
    let narrowed = Formula::and(vec![predicate("p"), predicate("q")]).expect("two operands");
    let before = assumption_with_fragment("A", &predicate("p"), Some(Fragment::Symbolic));
    let after = assumption_with_fragment("A", &narrowed, Some(Fragment::Symbolic));

    let real = classify_assumptions(&set([before.clone()]), &set([after.clone()]));
    assert_eq!(relation_of(&real, "A"), Relation::Unknown);

    assert_eq!(
        mutant_ignores_the_fragment_license(&before, &after),
        Relation::Strengthened,
        "the mutant must actually get this wrong, or it is not exercising the bug"
    );
}

/// The wrong-polarity wiring the bone brief names as the family's
/// dangerous-direction failure: the license held correctly, then the oracle's
/// answer run through fairness's antitone dual — where RFC 0031's `assumptions`
/// rule says "exactly as for claims — the same order".
fn mutant_dualizes_the_oracle(before: &Assumption, after: &Assumption) -> Relation {
    if before.expression().fragment() != Some(Fragment::Finite)
        || after.expression().fragment() != Some(Fragment::Finite)
    {
        return Relation::Unknown;
    }
    match finite_formula_relation(before.expression().ast(), after.expression().ast()) {
        Relation::Strengthened => Relation::Weakened,
        Relation::Weakened => Relation::Strengthened,
        other => other,
    }
}

#[test]
fn negative_mutant_dualizing_the_oracle_is_wrong_on_the_real_corpus_in_both_directions() {
    // Both directions, on the fixture's own expression: the dualized wiring
    // reports the real strengthening as `weakened` (selling the gaming move as
    // the safe direction) and the real weakening as `strengthened`. The real
    // classifier disagrees with it on both — the polarity is proven, not assumed.
    let before = fixture_assumptions(FIXTURE);
    let mutated = FIXTURE.replacen(NO_FORGERY_DISJUNCTION, NO_FORGERY_STRENGTHENED, 1);
    assert_ne!(mutated, FIXTURE, "the replacement must actually fire");
    let after = fixture_assumptions(&mutated);
    let key = unit("NetworkNoForgery");
    let (b, a) = (
        before.get(&key).expect("present before"),
        after.get(&key).expect("present after"),
    );

    // Direction one: the real strengthening.
    let real = relation_of(&classify_assumptions(&before, &after), "NetworkNoForgery");
    assert_eq!(real, Relation::Strengthened);
    assert_eq!(
        mutant_dualizes_the_oracle(b, a),
        Relation::Weakened,
        "the mutant must actually get this wrong, or it is not exercising the bug"
    );

    // Direction two: the real weakening (sides swapped).
    let real_back = relation_of(&classify_assumptions(&after, &before), "NetworkNoForgery");
    assert_eq!(real_back, Relation::Weakened);
    assert_eq!(
        mutant_dualizes_the_oracle(a, b),
        Relation::Strengthened,
        "the mutant must actually get this wrong, or it is not exercising the bug"
    );
}
