//! PR-12 / IMPL-02 exit evidence — property AST edit classification (`bn-8mlg`).
//!
//! # What this file is evidence for
//!
//! `src/properties.rs` classifies `claims[]` per RFC 0031's "`properties`" rule,
//! including the bounded, sound implication oracle START_HERE licenses ("Add
//! solver-based implication only where sound and bounded"). This file is the
//! dedicated adversarial and clause-mapped evidence on top of that module's own
//! unit tests (which stay the source of truth for the per-unit mechanics and are
//! not re-derived here), matching the shape of the five sibling evidence files.
//!
//! # Clause → test map
//!
//! | RFC 0031 / plan clause | Test |
//! |---|---|
//! | "exclude the failing state with a constraint" — `properties`/`weakened`, the plan §5.1 gaming move | [`positive_gutting_the_real_fixtures_agreement_claim_with_a_complementary_disjunct_classifies_weakened_and_blocks`] |
//! | "`weakened` iff behaviors(before) ⊆ behaviors(after)" with a derived witness | same test above |
//! | "`strengthened` iff behaviors(after) ⊆ behaviors(before)" with a derived witness | [`positive_dropping_the_real_fixtures_agreement_disjunct_classifies_strengthened_and_blocks`] |
//! | "one claim, keyed by `id`" — membership | [`negative_renaming_a_real_corpus_claim_while_weakening_it_is_removed_plus_added_never_unchanged`] |
//! | "A change to `kind` or to the bound `observer` with an unchanged expression … MUST classify `incomparable`" | [`positive_flipping_a_real_corpus_claims_kind_classifies_incomparable_and_blocks`], [`positive_unbinding_a_real_corpus_claims_observer_classifies_incomparable_and_blocks`] |
//! | the meaning axis dominates: no direction across a `kind` flip | [`negative_a_kind_flip_riding_a_decidable_expression_edit_never_classifies_a_direction`] |
//! | S2 / fail-closed: an implication the bounded machinery cannot establish is `unknown`, never a guess | [`boundary_an_edit_the_oracle_cannot_relate_classifies_unknown_and_blocks`] |
//! | boundedness: the step budget is a real, observable boundary | [`boundary_the_step_budget_fails_closed_on_a_derivation_too_large_for_it`] |
//! | N9 / "MUST reject a contract whose declaration does not hold" | [`negative_a_double_negation_disguise_cannot_even_board_the_artifact`] |
//! | S1: the CPNF-1 rewrites cannot disguise or manufacture a change | [`negative_a_double_negation_dressing_dissolves_and_the_weakening_underneath_classifies_weakened`] |
//! | R2: a vacuous-conjunct game never reaches `unchanged` | [`negative_a_vacuous_conjunct_game_never_classifies_unchanged`] |
//! | R2 / "CPNF-1 interaction": `source` MUST NOT contribute to classification | [`negative_rewriting_a_real_corpus_claims_source_alone_still_classifies_unchanged`] |
//! | anti-vacuity: the positive tests are not vacuously true, in both directions | [`negative_mutant_blind_to_expression_content_would_wrongly_pass_the_weakening_as_unchanged`], [`negative_mutant_comparing_document_equality_would_wrongly_flag_a_source_rewrite_as_changed`] |
//!
//! # House rules, inherited from the PR-12 evidence precedent
//!
//! - `src/` is untouched by this file, and no existing test anywhere is edited.
//! - [`FIXTURE`] is the same real, already-reviewed corpus document every sibling
//!   evidence file reads
//!   (`crates/continuum-intent/tests/fixtures/replicated-register-contract.json`) —
//!   three real claims (`Agreement`, `RuntimeToAbstract`, `StableWitness`), every
//!   one stated in the `Finite` fragment, one (`RuntimeToAbstract`) bound to the
//!   `client` observer, and the fixture's own `policy.properties` verb is
//!   **`locked`** — so closing a classification into `PolicyTable::verdict` under
//!   the fixture's own verb is the load-bearing `block` path, not a softball.
//!   Every fixture mutation below is one bounded, linear `.replacen` on that one
//!   fixed, small string — never a loop of doublings, never nested growth. The one
//!   programmatic construction (the budget boundary) is a flat linear vector of
//!   distinct atoms, stated where it happens.
//! - Positive, negative, and boundary evidence are each present and separately
//!   named. All tests are deterministic (INV-005): fixed inputs, fixed budgets, no
//!   clock, no entropy, no map-order dependence.

use continuum_intent::ast::{Formula, Fragment, Identifier};
use continuum_intent::canonical_json::Json;
use continuum_intent::change_policy::{
    AcceptancePath, ClassificationRecord, PolicyDecision, PolicyField, PolicyReviewers,
    PolicyTable, PolicyVerb, Relation,
};
use continuum_intent::property::{Claim, ClaimKind, ClaimSet, PropertyExpression, UnitKey};
use continuum_semantic_diff::properties::{classify_properties, finite_formula_relation};

/// The real corpus fixture: the replicated-register Intent Contract, also used by
/// the other five PR-12 evidence files and by `continuum-intent`'s PR-4 exit and
/// INV-012 evidence.
const FIXTURE: &str =
    include_str!("../../continuum-intent/tests/fixtures/replicated-register-contract.json");

/// The one place the fixture spells `v1 == v2` — the disjunct that makes
/// `Agreement` an agreement property at all. Unique in the document, which the
/// tests that mutate around it assert by requiring their replacement to fire.
const AGREEMENT_EQ_DISJUNCT: &str = r#"{"kind":"compare","left":{"kind":"var","name":"v1"},"op":"eq","right":{"kind":"var","name":"v2"}}"#;

/// The complementary disjunct: `v1 != v2`. Appended after
/// [`AGREEMENT_EQ_DISJUNCT`] it keeps N4's sorted operand order (`"op":"eq"` <
/// `"op":"ne"` under the shared prefix), so the mutated document is still valid
/// CPNF-1 and decodes — the attack arrives through the front door.
const AGREEMENT_NE_DISJUNCT: &str = r#"{"kind":"compare","left":{"kind":"var","name":"v1"},"op":"ne","right":{"kind":"var","name":"v2"}}"#;

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

fn fixture_claims(text: &str) -> ClaimSet {
    let parsed = document(text);
    ClaimSet::from_json(fixture_field(&parsed, "claims")).expect("the fixture's claims decode")
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
    changes: &[continuum_semantic_diff::properties::PropertyChange],
    id: &str,
) -> Relation {
    changes
        .iter()
        .find(|c| c.unit().as_str() == id)
        .unwrap_or_else(|| panic!("no record for unit {id:?} in {changes:?}"))
        .relation()
}

/// A complete `PolicyTable::verdict` records slice: `Unchanged` on every field
/// except the ones named in `overrides`. Mirrors the identical helper in the
/// impl03/impl06 evidence files: P1 needs one record per field, and every field but
/// the one under test is honestly unchanged in these scenarios (only `claims` was
/// mutated per test).
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

/// Close a `properties` relation into the fixture's own verdict path and return the
/// decision. The fixture's `properties` verb is `locked`, so this is the P2/P3
/// block path for everything but `unchanged`.
fn fixture_verdict(relation: Relation) -> PolicyDecision {
    let verdict = fixture_policy(FIXTURE)
        .verdict(
            &records_with_overrides(&[(PolicyField::Properties, relation)]),
            &fixture_reviewers(FIXTURE),
            AcceptancePath::AgentAccept,
        )
        .expect("a complete classification against a well-formed policy table computes");
    if relation != Relation::Unchanged {
        assert!(
            verdict
                .reasons()
                .iter()
                .any(|r| r.field() == PolicyField::Properties && r.relation() == relation),
            "P6: the blocking record must be named: {:?}",
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

// --- baseline: the fixture's content is what we think it is ---------------------------------

#[test]
fn the_fixtures_three_claims_are_what_the_module_doc_says_they_are() {
    let claims = fixture_claims(FIXTURE);
    assert_eq!(claims.len(), 3);
    for claim in claims.iter() {
        assert_eq!(claim.expression().fragment(), Some(Fragment::Finite));
    }
    let changes = classify_properties(&claims, &claims);
    assert_eq!(changes.len(), 3);
    assert!(changes.iter().all(|c| c.relation() == Relation::Unchanged));
}

#[test]
fn the_fixtures_own_properties_verb_is_locked() {
    // The fixture's authors locked `properties` outright — the strongest of the
    // base-row verbs — so every non-`unchanged` classification below must close to
    // `block`, and an `unchanged` one to `allow`.
    let policy = fixture_policy(FIXTURE);
    assert_eq!(policy.verb(PolicyField::Properties), PolicyVerb::Locked);
    assert_eq!(fixture_verdict(Relation::Unchanged), PolicyDecision::Allow);
}

// --- positive: the named attack, on the real fixture, caught with a derived witness ---------

#[test]
fn positive_gutting_the_real_fixtures_agreement_claim_with_a_complementary_disjunct_classifies_weakened_and_blocks()
 {
    // Plan §5.1's `properties` gaming move, in its most literal costume:
    // `Agreement` says "…or v1 == v2"; the revision appends "…or v1 != v2", which
    // exempts every remaining state and guts the claim while every original
    // disjunct survives verbatim. The oracle derives behaviors(before) ⊆
    // behaviors(after) — every before-disjunct entails the after-disjunction — and
    // no derivation exists the other way, so the classification is an *affirmed*
    // `weakened` with a recomputable witness, not a shrugged `unknown`.
    let before = fixture_claims(FIXTURE);
    let mutated = FIXTURE.replacen(
        AGREEMENT_EQ_DISJUNCT,
        &format!("{AGREEMENT_EQ_DISJUNCT},{AGREEMENT_NE_DISJUNCT}"),
        1,
    );
    assert_ne!(mutated, FIXTURE, "the replacement must actually fire");
    let after = fixture_claims(&mutated);

    let changes = classify_properties(&before, &after);
    assert_eq!(relation_of(&changes, "Agreement"), Relation::Weakened);

    // Closed into the fixture's own verdict: `locked` denies `weakened` (P3).
    assert_eq!(fixture_verdict(Relation::Weakened), PolicyDecision::Block);

    // And under the gaming table's own verb for this move — `review` — the same
    // relation is reviewed, never allowed: the classification, not the verb,
    // is what this module owes, and it holds up under both governance shapes.
    let review_table = PolicyTable::new(PolicyField::ALL.map(|field| {
        if field == PolicyField::Properties {
            (field, PolicyVerb::Review)
        } else {
            (field, PolicyVerb::Unlocked)
        }
    }))
    .expect("review is admissible on properties");
    let reviewers = PolicyReviewers::new([(
        PolicyField::Properties,
        std::collections::BTreeSet::from(["verification-lead".to_owned()]),
    )])
    .expect("a named, non-empty reviewer list");
    let verdict = review_table
        .verdict(
            &records_with_overrides(&[(PolicyField::Properties, Relation::Weakened)]),
            &reviewers,
            AcceptancePath::AgentAccept,
        )
        .expect("computes");
    assert_eq!(verdict.decision(), PolicyDecision::Review);
}

#[test]
fn positive_dropping_the_real_fixtures_agreement_disjunct_classifies_strengthened_and_blocks() {
    // The oracle's other direction, so neither directional arm is vacuous: drop
    // the `v1 == v2` disjunct and `Agreement` claims strictly more. Every
    // remaining after-disjunct entails the before-disjunction and the dropped
    // disjunct blocks the converse, so behaviors(after) ⊆ behaviors(before):
    // `strengthened` — and still `block` under `locked`, which denies every
    // non-`unchanged` relation.
    let before = fixture_claims(FIXTURE);
    let mutated = FIXTURE.replacen(&format!(",{AGREEMENT_EQ_DISJUNCT}"), "", 1);
    assert_ne!(mutated, FIXTURE, "the replacement must actually fire");
    let after = fixture_claims(&mutated);

    let changes = classify_properties(&before, &after);
    assert_eq!(relation_of(&changes, "Agreement"), Relation::Strengthened);
    assert_eq!(
        fixture_verdict(Relation::Strengthened),
        PolicyDecision::Block
    );
}

// --- positive: the meaning axis, on the real fixture ----------------------------------------

#[test]
fn positive_flipping_a_real_corpus_claims_kind_classifies_incomparable_and_blocks() {
    // "A change to `kind` … with an unchanged expression is a change of what the
    // claim means and MUST classify `incomparable`, never `unchanged`." The first
    // `"kind":"safety"` in the document is `Agreement`'s.
    let before = fixture_claims(FIXTURE);
    let mutated = FIXTURE.replacen(r#""kind":"safety""#, r#""kind":"security""#, 1);
    assert_ne!(mutated, FIXTURE, "the replacement must actually fire");
    let after = fixture_claims(&mutated);

    let changes = classify_properties(&before, &after);
    assert_eq!(relation_of(&changes, "Agreement"), Relation::Incomparable);
    assert_eq!(
        fixture_verdict(Relation::Incomparable),
        PolicyDecision::Block
    );
}

#[test]
fn positive_unbinding_a_real_corpus_claims_observer_classifies_incomparable_and_blocks() {
    // The other meaning axis: `RuntimeToAbstract` is the fixture's one
    // observer-bound claim (`"observer":"client"`); unbinding it changes what the
    // claim means with its expression untouched.
    let before = fixture_claims(FIXTURE);
    let mutated = FIXTURE.replacen(r#""observer":"client""#, r#""observer":null"#, 1);
    assert_ne!(mutated, FIXTURE, "the replacement must actually fire");
    let after = fixture_claims(&mutated);

    let changes = classify_properties(&before, &after);
    assert_eq!(
        relation_of(&changes, "RuntimeToAbstract"),
        Relation::Incomparable
    );
    assert_eq!(
        fixture_verdict(Relation::Incomparable),
        PolicyDecision::Block
    );
}

// --- negative: disguises are never silent ---------------------------------------------------

#[test]
fn negative_renaming_a_real_corpus_claim_while_weakening_it_is_removed_plus_added_never_unchanged()
{
    // Rename + weaken in one revision: the classifier keys by `id` and MUST NOT
    // match by expression, so the pair is `removed` + `added` — two records, both
    // non-`unchanged`, either of which blocks under `locked`.
    let before = fixture_claims(FIXTURE);
    let mutated = FIXTURE
        .replacen(
            AGREEMENT_EQ_DISJUNCT,
            &format!("{AGREEMENT_EQ_DISJUNCT},{AGREEMENT_NE_DISJUNCT}"),
            1,
        )
        .replacen(r#""id":"Agreement""#, r#""id":"AgreementPrime""#, 1);
    assert_ne!(mutated, FIXTURE, "the replacements must actually fire");
    let after = fixture_claims(&mutated);

    let changes = classify_properties(&before, &after);
    assert_eq!(changes.len(), 4, "{changes:?}");
    assert_eq!(relation_of(&changes, "Agreement"), Relation::Removed);
    assert_eq!(relation_of(&changes, "AgreementPrime"), Relation::Added);
    assert_eq!(fixture_verdict(Relation::Removed), PolicyDecision::Block);
    assert_eq!(fixture_verdict(Relation::Added), PolicyDecision::Block);
}

#[test]
fn negative_a_kind_flip_riding_a_decidable_expression_edit_never_classifies_a_direction() {
    // The compound meaning+content case: dropping the disjunct alone classifies
    // `strengthened` (proved by the positive test above), and an attacker pairs it
    // with a `kind` flip hoping the affirmative direction survives. It must not:
    // the oracle is never consulted across a meaning change, and the unit fails
    // closed to `unknown` — which still blocks.
    let before = fixture_claims(FIXTURE);
    let mutated = FIXTURE
        .replacen(&format!(",{AGREEMENT_EQ_DISJUNCT}"), "", 1)
        .replacen(r#""kind":"safety""#, r#""kind":"security""#, 1);
    assert_ne!(mutated, FIXTURE, "the replacements must actually fire");
    let after = fixture_claims(&mutated);

    let changes = classify_properties(&before, &after);
    assert_eq!(relation_of(&changes, "Agreement"), Relation::Unknown);
    assert_eq!(fixture_verdict(Relation::Unknown), PolicyDecision::Block);
}

#[test]
fn negative_a_double_negation_disguise_cannot_even_board_the_artifact() {
    // N9's artifact-layer half: the fixture declares `normal_form: "cpnf-1"`, and a
    // double negation is not in CPNF-1, so the dressed document is rejected at
    // decode — "a checker MUST verify the claim by re-normalizing and MUST reject a
    // contract whose declaration does not hold". The disguise never reaches the
    // classifier at all.
    let stutters = r#"{"args":[],"kind":"predicate","name":"stutters"}"#;
    let dressed = FIXTURE.replacen(
        stutters,
        r#"{"kind":"not","operand":{"kind":"not","operand":{"args":[],"kind":"predicate","name":"stutters"}}}"#,
        1,
    );
    assert_ne!(dressed, FIXTURE, "the replacement must actually fire");
    let parsed = document(&dressed);
    assert!(
        ClaimSet::from_json(fixture_field(&parsed, "claims")).is_err(),
        "a denormalized AST under a cpnf-1 declaration must be rejected, never repaired"
    );
}

#[test]
fn negative_a_double_negation_dressing_dissolves_and_the_weakening_underneath_classifies_weakened()
{
    // N9's classifier-layer half: authored through the API (the only door that
    // accepts an authoring form), `not(not(weakened))` normalizes to the weakening
    // it dresses, so the classification is exactly `weakened` — the disguise
    // contributes nothing in either direction.
    let base =
        Formula::or(vec![predicate("holds"), predicate("held_previously")]).expect("two operands");
    let weakened = Formula::or(vec![
        predicate("holds"),
        predicate("held_previously"),
        predicate("excused"),
    ])
    .expect("three operands");
    let claim = |ast: &Formula| {
        Claim::new(
            UnitKey::new("C-guard").expect("non-empty"),
            ClaimKind::Safety,
            PropertyExpression::normalized(ast, Some(Fragment::Finite), None).expect("normalizes"),
            None,
        )
    };
    let before = ClaimSet::from_claims([claim(&base)]).expect("one claim");
    let dressed = Formula::not(Formula::not(weakened));
    let after = ClaimSet::from_claims([claim(&dressed)]).expect("one claim");

    let changes = classify_properties(&before, &after);
    assert_eq!(relation_of(&changes, "C-guard"), Relation::Weakened);
}

#[test]
fn negative_a_vacuous_conjunct_game_never_classifies_unchanged() {
    // The tautology CPNF-1 deliberately does not recognize: `and(base, or(p, not
    // p))` denotes the same behaviors as `base`, but R2 licenses `unchanged` on
    // encoding equality alone, and the encodings differ. Alone, the game earns
    // `strengthened` — a true (non-strict) inclusion the oracle derives, blocked
    // under `locked` like everything else. Paired with a weakening it earns
    // `unknown` — the two edits pull the derivation in both directions and neither
    // completes. What it can never earn is silence.
    let base =
        Formula::or(vec![predicate("holds"), predicate("held_previously")]).expect("two operands");
    let taut =
        Formula::or(vec![predicate("p"), Formula::not(predicate("p"))]).expect("two operands");
    let claim = |id: &str, ast: &Formula| {
        Claim::new(
            UnitKey::new(id).expect("non-empty"),
            ClaimKind::Safety,
            PropertyExpression::normalized(ast, Some(Fragment::Finite), None).expect("normalizes"),
            None,
        )
    };
    let before = ClaimSet::from_claims([claim("C-guard", &base)]).expect("one claim");

    let vacuous_only = Formula::and(vec![base.clone(), taut.clone()]).expect("two operands");
    let after = ClaimSet::from_claims([claim("C-guard", &vacuous_only)]).expect("one claim");
    let relation = relation_of(&classify_properties(&before, &after), "C-guard");
    assert_eq!(relation, Relation::Strengthened);
    assert_ne!(fixture_verdict(relation), PolicyDecision::Allow);

    let weakened = Formula::or(vec![
        predicate("holds"),
        predicate("held_previously"),
        predicate("excused"),
    ])
    .expect("three operands");
    let vacuous_and_weakened = Formula::and(vec![weakened, taut]).expect("two operands");
    let after =
        ClaimSet::from_claims([claim("C-guard", &vacuous_and_weakened)]).expect("one claim");
    let relation = relation_of(&classify_properties(&before, &after), "C-guard");
    assert_eq!(relation, Relation::Unknown);
    assert!(
        !matches!(relation, Relation::Unchanged | Relation::Strengthened),
        "a weakening under a vacuous conjunct must never read as benign"
    );
    assert_eq!(fixture_verdict(relation), PolicyDecision::Block);
}

#[test]
fn negative_rewriting_a_real_corpus_claims_source_alone_still_classifies_unchanged() {
    // The mirror-image check: `source` is display-only and MUST NOT contribute to
    // classification, so the classifier is neither too strict nor too lenient.
    let before = fixture_claims(FIXTURE);
    let mutated = FIXTURE.replacen(
        r#""source":"forall epoch, v1, v2: chosen.get(epoch) == Some(v1) && chosen.get(epoch) == Some(v2) => v1 == v2""#,
        r#""source":"a completely different English rendering of the same fact""#,
        1,
    );
    assert_ne!(mutated, FIXTURE, "the replacement must actually fire");
    let after = fixture_claims(&mutated);

    let changes = classify_properties(&before, &after);
    assert_eq!(relation_of(&changes, "Agreement"), Relation::Unchanged);
    assert_eq!(fixture_verdict(Relation::Unchanged), PolicyDecision::Allow);
}

// --- boundary: the soundness line, from both sides ------------------------------------------

#[test]
fn boundary_an_edit_the_oracle_cannot_relate_classifies_unknown_and_blocks() {
    // The undecidable-therefore-closed case beside the decided ones above: rename
    // the predicate `RuntimeToAbstract` hinges on. English says the claims are
    // related; no derivation rule does, and S2 makes `unknown` the honest answer —
    // never a guessed direction. Closed into the fixture's verdict it blocks, so
    // failing to decide is never failing open.
    let before = fixture_claims(FIXTURE);
    let mutated = FIXTURE.replacen(r#""name":"stutters""#, r#""name":"stutters_quietly""#, 1);
    assert_ne!(mutated, FIXTURE, "the replacement must actually fire");
    let after = fixture_claims(&mutated);

    let changes = classify_properties(&before, &after);
    assert_eq!(
        relation_of(&changes, "RuntimeToAbstract"),
        Relation::Unknown
    );
    assert!(
        changes
            .iter()
            .all(|c| !matches!(c.relation(), Relation::Strengthened | Relation::Weakened)),
        "no direction may be guessed: {changes:?}"
    );
    assert_eq!(fixture_verdict(Relation::Unknown), PolicyDecision::Block);
}

#[test]
fn boundary_the_step_budget_fails_closed_on_a_derivation_too_large_for_it() {
    // Boundedness made observable: one shape, two sizes. A disjunction extended by
    // one disjunct is decidably `weakened` at 8 disjuncts and would be at 400 too —
    // but the 400-atom derivation needs more steps than `IMPLICATION_STEP_BUDGET`
    // grants, so it fails closed to `unknown` instead of searching unboundedly.
    // The construction is a flat, linear vector of distinct atoms — no doubling,
    // no nesting — and both runs are deterministic in the fixed budget.
    let disjunction = |n: usize, extra: bool| {
        let mut operands: Vec<Formula> = (0..n).map(|i| predicate(&format!("p{i:04}"))).collect();
        if extra {
            operands.push(predicate("extra"));
        }
        Formula::or(operands).expect("two or more operands")
    };
    assert_eq!(
        finite_formula_relation(&disjunction(8, false), &disjunction(8, true)),
        Relation::Weakened
    );
    assert_eq!(
        finite_formula_relation(&disjunction(400, false), &disjunction(400, true)),
        Relation::Unknown
    );
    assert_eq!(fixture_verdict(Relation::Unknown), PolicyDecision::Block);
}

// --- anti-vacuity mutants, in both directions ------------------------------------------------

/// A plausible, *wrong* classifier: decides a same-key comparison purely by whether
/// the key is present on both sides, never looking at the expression — misses the
/// weakening entirely.
fn mutant_blind_to_expression_content(before: &ClaimSet, after: &ClaimSet, id: &str) -> Relation {
    let key = UnitKey::new(id).expect("test id is non-empty");
    match (before.get(&key).is_some(), after.get(&key).is_some()) {
        (true, true) => Relation::Unchanged,
        _ => Relation::Unknown,
    }
}

#[test]
fn negative_mutant_blind_to_expression_content_would_wrongly_pass_the_weakening_as_unchanged() {
    let before = fixture_claims(FIXTURE);
    let mutated = FIXTURE.replacen(
        AGREEMENT_EQ_DISJUNCT,
        &format!("{AGREEMENT_EQ_DISJUNCT},{AGREEMENT_NE_DISJUNCT}"),
        1,
    );
    assert_ne!(mutated, FIXTURE);
    let after = fixture_claims(&mutated);

    let real = relation_of(&classify_properties(&before, &after), "Agreement");
    assert_eq!(real, Relation::Weakened);

    let mutant_relation = mutant_blind_to_expression_content(&before, &after, "Agreement");
    assert_eq!(
        mutant_relation,
        Relation::Unchanged,
        "the mutant must actually get this wrong, or it is not exercising the bug"
    );
    assert_ne!(real, mutant_relation);
}

/// A plausible, *wrong* classifier in the other direction: compares the whole
/// expression document — `PropertyExpression`'s derived `PartialEq`, which includes
/// the display-only `source` — so a harmless prose rewrite reads as a change.
fn mutant_compares_document_equality(before: &ClaimSet, after: &ClaimSet, id: &str) -> Relation {
    let key = UnitKey::new(id).expect("test id is non-empty");
    let (Some(b), Some(a)) = (before.get(&key), after.get(&key)) else {
        return Relation::Unknown;
    };
    if b.expression() == a.expression() {
        Relation::Unchanged
    } else {
        Relation::Unknown
    }
}

#[test]
fn negative_mutant_comparing_document_equality_would_wrongly_flag_a_source_rewrite_as_changed() {
    let before = fixture_claims(FIXTURE);
    let mutated = FIXTURE.replacen(
        r#""source":"forall epoch, v1, v2: chosen.get(epoch) == Some(v1) && chosen.get(epoch) == Some(v2) => v1 == v2""#,
        r#""source":"a completely different English rendering of the same fact""#,
        1,
    );
    assert_ne!(mutated, FIXTURE);
    let after = fixture_claims(&mutated);

    let real = relation_of(&classify_properties(&before, &after), "Agreement");
    assert_eq!(real, Relation::Unchanged);

    let mutant_relation = mutant_compares_document_equality(&before, &after, "Agreement");
    assert_eq!(
        mutant_relation,
        Relation::Unknown,
        "the mutant must actually get this wrong (flag a non-change), or it is not \
         exercising the bug"
    );
    assert_ne!(real, mutant_relation);
}
