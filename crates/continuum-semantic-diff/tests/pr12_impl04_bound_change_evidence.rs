//! PR-12 / IMPL-04 exit evidence — bound change classification (`bn-ycn6`).
//!
//! # What this file is evidence for
//!
//! `src/bounds.rs` classifies `bounds` per RFC 0031's "bounds" rule. This file is the
//! dedicated adversarial and clause-mapped evidence the bone brief asks for, on top of
//! that module's own unit tests (which stay the source of truth for the five
//! componentwise/declaredness outcomes and are not re-derived here):
//!
//! > a real-corpus case […]; an adversarial sweep (a disguised shrink — e.g.
//! > simultaneous grow+shrink across components — never classifies as an affirmative
//! > relation or silent equality); declaredness-change → unknown → fails closed;
//! > anti-vacuity mutants (both directions asserted so no check is vacuous).
//!
//! # Clause → test map
//!
//! | RFC 0031 / plan clause | Test |
//! |---|---|
//! | "reduce node count from five to three" (plan §5.1 gaming-move row) | [`positive_the_named_attack_on_a_real_corpus_fixture_classifies_contracted`] |
//! | `bounds` → `contracted` → blocked by `no-decrease` (RFC 0031 "Completeness guarantee") | [`the_named_attack_is_blocked_not_allowed_under_the_fixtures_own_policy`] |
//! | "mixed movement is `incomparable`" (RFC 0031 "bounds") | [`adversarial_a_simultaneous_grow_and_shrink_never_classifies_as_expanded_contracted_or_unchanged`] |
//! | "a change in the declaredness of either MUST classify `bounds` as `unknown` and fail closed" | [`declaredness_change_classifies_unknown_even_with_no_other_movement`], [`declaredness_change_fails_closed_to_review_under_the_fixtures_own_verb`] |
//! | P2 (non-affirmative relations block under every verb, even `unlocked`) | [`unknown_still_contributes_review_even_under_the_least_restrictive_verb`] |
//! | P2's `locked` clause ("contributes `block` when the field's verb is `locked`") | [`declaredness_change_blocks_outright_under_a_locked_verb`] |
//! | anti-vacuity: the positive assertions are not vacuously true | [`negative_mutant_blind_to_declaredness_would_wrongly_pass_a_fail_closed_case_as_a_direction`], [`negative_mutant_by_total_magnitude_would_wrongly_pass_the_mixed_move_as_a_clean_direction`] |
//! | schema fact this module's "never `unsupported`" reasoning depends on | `src/bounds.rs`'s own `the_live_schema_carries_no_fragment_member_on_bounds` (not re-tested here) |
//!
//! # House rules, inherited from the PR-4/INV-012 evidence precedent (`bn-1sdp`)
//!
//! - `src/` is untouched by this file, and no existing test anywhere is edited.
//! - [`FIXTURE`] is a real, already-reviewed corpus document
//!   (`crates/continuum-intent/tests/fixtures/replicated-register-contract.json`,
//!   already used by `continuum-intent`'s own PR-4 exit and INV-012 evidence, and by
//!   `continuum-semantic-diff`'s `pr12_impl05_observer_event_change_evidence`), read at
//!   compile time via `include_str!`. Every mutation of it below is one bounded, linear
//!   `.replacen` on that fixed ~3 KiB string — never a loop of doublings, never nested
//!   growth.
//! - Positive, negative, and boundary evidence are each present and separately named,
//!   per this bone's brief.

use continuum_intent::bounds::{Bounds, DeclaredBound, ExplorationBound};
use continuum_intent::canonical_json::Json;
use continuum_intent::change_policy::{
    AcceptancePath, ClassificationRecord, PolicyDecision, PolicyField, PolicyReviewers,
    PolicyTable, PolicyVerb, Relation,
};
use continuum_semantic_diff::bounds::classify_bounds;

/// The real corpus fixture: the replicated-register Intent Contract, also used by
/// `continuum-intent`'s own PR-4 exit and INV-012 evidence, and by this crate's PR-12 /
/// IMPL-05 evidence. Its `bounds` is `{"depth":null,"faults":2,"nodes":3,"values":2}`
/// and its `policy.bounds` is `"no-decrease"`.
const FIXTURE: &str =
    include_str!("../../continuum-intent/tests/fixtures/replicated-register-contract.json");

fn bounds(
    values: ExplorationBound,
    nodes: DeclaredBound,
    faults: DeclaredBound,
    depth: ExplorationBound,
) -> Bounds {
    Bounds::new(values, nodes, faults, depth).expect("test bounds are within the minimums")
}

/// Decode the fixture's `bounds` object.
fn fixture_bounds(document: &str) -> Bounds {
    let parsed = Json::parse(document.as_bytes()).expect("the fixture is canonical JSON");
    let bounds_json = parsed
        .as_object()
        .expect("the fixture is an object")
        .get("bounds")
        .expect("the fixture declares bounds");
    Bounds::from_json(bounds_json).expect("the fixture's bounds decode")
}

/// Decode the fixture's `policy` table — its own governance choice for `bounds`.
fn fixture_policy(document: &str) -> PolicyTable {
    let parsed = Json::parse(document.as_bytes()).expect("the fixture is canonical JSON");
    let policy = parsed
        .as_object()
        .expect("the fixture is an object")
        .get("policy")
        .expect("the fixture declares a policy table");
    PolicyTable::from_json(policy).expect("the fixture's policy table decodes")
}

/// Decode the fixture's `policy_reviewers` map — W7 requires a named reviewer for
/// every `review`-verb field, and the fixture names one for each of the four fields
/// that carry it (`bounds` itself is `no-decrease`, not `review`, but `check_reviewers`
/// validates every field in the table regardless of which one changed).
fn fixture_reviewers(document: &str) -> PolicyReviewers {
    let parsed = Json::parse(document.as_bytes()).expect("the fixture is canonical JSON");
    let reviewers = parsed
        .as_object()
        .expect("the fixture is an object")
        .get("policy_reviewers")
        .expect("the fixture declares policy_reviewers");
    PolicyReviewers::from_json(reviewers).expect("the fixture's reviewer map decodes")
}

/// One `unchanged` record per field other than `PolicyField::Bounds` — P1's
/// completeness requirement, filled in honestly because only `bounds` was mutated in
/// every scenario below.
fn unchanged_records_except_bounds() -> Vec<ClassificationRecord> {
    PolicyField::ALL
        .into_iter()
        .filter(|field| *field != PolicyField::Bounds)
        .map(|field| {
            ClassificationRecord::new(field, Relation::Unchanged)
                .expect("unchanged is always admissible")
        })
        .collect()
}

// --- baseline: the fixture's bounds round-trips and is what we think it is -----------------

#[test]
fn the_fixtures_bounds_is_values_2_nodes_3_faults_2_depth_unbounded() {
    let b = fixture_bounds(FIXTURE);
    assert_eq!(b.values(), ExplorationBound::Bounded(2));
    assert_eq!(b.nodes(), DeclaredBound::Declared(3));
    assert_eq!(b.faults(), DeclaredBound::Declared(2));
    assert_eq!(b.depth(), ExplorationBound::Unbounded);
    // Compared against itself, nothing moved.
    assert_eq!(classify_bounds(&b, &b).relation(), Relation::Unchanged);
}

// --- positive: the named attack, on the real fixture, correctly caught ---------------------

#[test]
fn positive_the_named_attack_on_a_real_corpus_fixture_classifies_contracted() {
    // RFC 0031's own example move ("reduce node count from five to three"), applied at
    // the fixture's own starting value: shrink `nodes` alone, nothing else moves.
    let before = fixture_bounds(FIXTURE);
    let shrunk_fixture = FIXTURE.replacen(r#""nodes":3"#, r#""nodes":2"#, 1);
    assert_ne!(
        shrunk_fixture, FIXTURE,
        "the replacement must actually fire"
    );
    let after = fixture_bounds(&shrunk_fixture);

    assert_eq!(
        classify_bounds(&before, &after).relation(),
        Relation::Contracted
    );
    // And the reverse direction is the safe one, `expanded` — the module's positive
    // assertion is not vacuous in either direction.
    assert_eq!(
        classify_bounds(&after, &before).relation(),
        Relation::Expanded
    );
}

#[test]
fn the_named_attack_is_blocked_not_allowed_under_the_fixtures_own_policy() {
    // Close the loop into the policy machinery PR-4 already landed
    // (`continuum_intent::change_policy::PolicyTable::verdict`): the fixture's own
    // `policy.bounds` is `"no-decrease"` (its authors' governance choice, matching RFC
    // 0031's worked example row exactly), so the contraction record must `block`, not
    // merely `review` — `no-decrease` denies `contracted` outright (P3).
    let before = fixture_bounds(FIXTURE);
    let shrunk_fixture = FIXTURE.replacen(r#""nodes":3"#, r#""nodes":2"#, 1);
    let after = fixture_bounds(&shrunk_fixture);
    let policy = fixture_policy(FIXTURE);
    assert_eq!(policy.verb(PolicyField::Bounds), PolicyVerb::NoDecrease);

    let bounds_change = classify_bounds(&before, &after);
    assert_eq!(bounds_change.relation(), Relation::Contracted);

    let mut records = unchanged_records_except_bounds();
    records.push(
        bounds_change
            .to_classification_record()
            .expect("Contracted is admissible on bounds"),
    );

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
            .any(|reason| reason.field() == PolicyField::Bounds
                && reason.relation() == Relation::Contracted),
        "{:?}",
        verdict.reasons()
    );
}

// --- adversarial: a disguised shrink never reads as a clean direction or a no-op -----------

#[test]
fn adversarial_a_simultaneous_grow_and_shrink_never_classifies_as_expanded_contracted_or_unchanged()
{
    // The disguise the bone brief names directly: shrink one component (`nodes`, the
    // exact RFC 0031 gaming target) while growing another (`faults`) in the same
    // comparison, hoping the net effect reads as a harmless — or even beneficial —
    // change. RFC 0031's rule admits no exception for this: mixed movement is
    // `incomparable`, full stop.
    let before = fixture_bounds(FIXTURE);
    let disguised = FIXTURE
        .replacen(r#""nodes":3"#, r#""nodes":2"#, 1)
        .replacen(r#""faults":2"#, r#""faults":3"#, 1);
    assert_ne!(disguised, FIXTURE);
    let after = fixture_bounds(&disguised);

    let relation = classify_bounds(&before, &after).relation();
    assert_eq!(relation, Relation::Incomparable);
    assert_ne!(relation, Relation::Expanded);
    assert_ne!(relation, Relation::Contracted);
    assert_ne!(relation, Relation::Unchanged);
    // Non-affirmative: `PolicyTable::verdict`'s P2 makes this contribute at least
    // `review` on every field under every verb, including `unlocked` — the disguise
    // cannot be waved through merely because some other verb on `bounds` is permissive.
    assert!(!relation.is_affirmative());
}

#[test]
fn adversarial_growing_the_nominal_headline_component_while_shrinking_another_is_still_incomparable()
 {
    // The mirror-image disguise: grow `values` (a plausible "we made verification
    // *more* thorough" cover story) while quietly shrinking `nodes` underneath it.
    let before = fixture_bounds(FIXTURE);
    let disguised = FIXTURE
        .replacen(r#""nodes":3"#, r#""nodes":2"#, 1)
        .replacen(r#""values":2"#, r#""values":5"#, 1);
    assert_ne!(disguised, FIXTURE);
    let after = fixture_bounds(&disguised);

    assert_eq!(
        classify_bounds(&before, &after).relation(),
        Relation::Incomparable
    );
}

// --- declaredness change: unknown, and it fails closed --------------------------------------

#[test]
fn declaredness_change_classifies_unknown_even_with_no_other_movement() {
    // Undeclare `nodes` — the fixture's declared bound goes from `Declared(3)` to
    // `Undeclared` — with nothing else in the tuple moving. RFC 0031: "a change in the
    // declaredness of either MUST classify `bounds` as `unknown` and fail closed."
    let before = fixture_bounds(FIXTURE);
    let undeclared_fixture = FIXTURE.replacen(r#""nodes":3,"#, "", 1);
    assert_ne!(
        undeclared_fixture, FIXTURE,
        "the replacement must actually fire"
    );
    let after = fixture_bounds(&undeclared_fixture);
    assert_eq!(after.nodes(), DeclaredBound::Undeclared);

    assert_eq!(
        classify_bounds(&before, &after).relation(),
        Relation::Unknown
    );
}

#[test]
fn declaredness_change_is_unknown_even_when_every_other_component_grows() {
    // The stronger fail-closed case: every declared/bounded component grows, but
    // `faults` slips from declared to undeclared. No amount of growth elsewhere may
    // excuse it.
    let before = bounds(
        ExplorationBound::Bounded(2),
        DeclaredBound::Declared(3),
        DeclaredBound::Declared(1),
        ExplorationBound::Bounded(10),
    );
    let after = bounds(
        ExplorationBound::Bounded(5),
        DeclaredBound::Declared(9),
        DeclaredBound::Undeclared,
        ExplorationBound::Unbounded,
    );
    assert_eq!(
        classify_bounds(&before, &after).relation(),
        Relation::Unknown
    );
}

#[test]
fn declaredness_change_fails_closed_to_review_under_the_fixtures_own_verb() {
    let before = fixture_bounds(FIXTURE);
    let undeclared_fixture = FIXTURE.replacen(r#""nodes":3,"#, "", 1);
    let after = fixture_bounds(&undeclared_fixture);
    let policy = fixture_policy(FIXTURE);
    assert_eq!(policy.verb(PolicyField::Bounds), PolicyVerb::NoDecrease);

    let bounds_change = classify_bounds(&before, &after);
    assert_eq!(bounds_change.relation(), Relation::Unknown);

    let mut records = unchanged_records_except_bounds();
    records.push(
        bounds_change
            .to_classification_record()
            .expect("Unknown is admissible on bounds"),
    );
    let verdict = policy
        .verdict(
            &records,
            &fixture_reviewers(FIXTURE),
            AcceptancePath::AgentAccept,
        )
        .expect("a complete classification against a well-formed policy table computes");
    // P2: non-affirmative, and the verb is not `locked`, so the record contributes
    // `review` — inconclusive is never `allow`.
    assert_eq!(verdict.decision(), PolicyDecision::Review);
}

#[test]
fn unknown_still_contributes_review_even_under_the_least_restrictive_verb() {
    // P2's unconditional half: a non-affirmative relation contributes at least
    // `review` "on every field under every verb" — including `unlocked`, the verb
    // that would otherwise `allow` any affirmative relation outright (P4).
    let table = PolicyTable::all_unlocked();
    assert_eq!(table.verb(PolicyField::Bounds), PolicyVerb::Unlocked);
    let mut records = unchanged_records_except_bounds();
    records.push(
        ClassificationRecord::new(PolicyField::Bounds, Relation::Unknown)
            .expect("Unknown is admissible on bounds"),
    );
    let verdict = table
        .verdict(
            &records,
            &PolicyReviewers::empty(),
            AcceptancePath::AgentAccept,
        )
        .expect("complete classification against a well-formed table computes");
    assert_eq!(verdict.decision(), PolicyDecision::Review);
}

#[test]
fn declaredness_change_blocks_outright_under_a_locked_verb() {
    // P2's other half: the same non-affirmative record contributes `block`, not merely
    // `review`, when the field's own verb is `locked`.
    let table = PolicyTable::new(PolicyField::ALL.map(|field| {
        if field == PolicyField::Bounds {
            (field, PolicyVerb::Locked)
        } else {
            (field, PolicyVerb::Unlocked)
        }
    }))
    .expect("locked is admissible on bounds");
    let mut records = unchanged_records_except_bounds();
    records.push(
        ClassificationRecord::new(PolicyField::Bounds, Relation::Unknown)
            .expect("Unknown is admissible on bounds"),
    );
    let verdict = table
        .verdict(
            &records,
            &PolicyReviewers::empty(),
            AcceptancePath::AgentAccept,
        )
        .expect("complete classification against a well-formed table computes");
    assert_eq!(verdict.decision(), PolicyDecision::Block);
}

// --- anti-vacuity mutants: the positive assertions are not vacuously true ------------------
//
// Each mutant below is a plausible, *wrong* classifier — the kind of bug this file's
// positive tests exist to catch. Run the same scenarios through it and show it gets the
// wrong answer, in both directions, where `classify_bounds` gets the right one; that is
// the proof this file's assertions are load-bearing rather than tautological.

/// Mutant 1: reads `DeclaredBound::Undeclared` as the value `0` instead of refusing to
/// compare. A classifier with this bug would report a declaredness change as an
/// ordinary numeric movement — the single worst outcome, since it reads a fail-closed
/// "we cannot compare this" case as a confident, allow-eligible direction.
fn mutant_blind_to_declaredness(before: DeclaredBound, after: DeclaredBound) -> Relation {
    fn as_num(bound: DeclaredBound) -> i64 {
        match bound {
            DeclaredBound::Declared(n) => n,
            DeclaredBound::Undeclared => 0,
        }
    }
    match as_num(before).cmp(&as_num(after)) {
        std::cmp::Ordering::Equal => Relation::Unchanged,
        std::cmp::Ordering::Less => Relation::Expanded,
        std::cmp::Ordering::Greater => Relation::Contracted,
    }
}

#[test]
fn negative_mutant_blind_to_declaredness_would_wrongly_pass_a_fail_closed_case_as_a_direction() {
    // Forward: undeclared -> declared(3). The real classifier refuses to compare;
    // the mutant reads "0 -> 3" as growth.
    let real_forward = classify_bounds(
        &bounds(
            ExplorationBound::Bounded(2),
            DeclaredBound::Undeclared,
            DeclaredBound::Declared(1),
            ExplorationBound::Bounded(10),
        ),
        &bounds(
            ExplorationBound::Bounded(2),
            DeclaredBound::Declared(3),
            DeclaredBound::Declared(1),
            ExplorationBound::Bounded(10),
        ),
    )
    .relation();
    assert_eq!(real_forward, Relation::Unknown);
    let mutant_forward =
        mutant_blind_to_declaredness(DeclaredBound::Undeclared, DeclaredBound::Declared(3));
    assert_eq!(
        mutant_forward,
        Relation::Expanded,
        "the mutant must actually get this wrong, or it is not exercising the bug"
    );
    assert_ne!(real_forward, mutant_forward);

    // Reverse: declared(3) -> undeclared. Still `unknown` for real; the mutant now
    // reads "3 -> 0" as shrinkage — a different wrong answer than the forward case,
    // which is exactly why both directions are asserted here rather than one.
    let real_reverse = classify_bounds(
        &bounds(
            ExplorationBound::Bounded(2),
            DeclaredBound::Declared(3),
            DeclaredBound::Declared(1),
            ExplorationBound::Bounded(10),
        ),
        &bounds(
            ExplorationBound::Bounded(2),
            DeclaredBound::Undeclared,
            DeclaredBound::Declared(1),
            ExplorationBound::Bounded(10),
        ),
    )
    .relation();
    assert_eq!(real_reverse, Relation::Unknown);
    let mutant_reverse =
        mutant_blind_to_declaredness(DeclaredBound::Declared(3), DeclaredBound::Undeclared);
    assert_eq!(
        mutant_reverse,
        Relation::Contracted,
        "the mutant must actually get this wrong, or it is not exercising the bug"
    );
    assert_ne!(real_reverse, mutant_reverse);
}

/// Mutant 2: compares only the *total* of the finite/declared components (`Undeclared`
/// and `Unbounded` read as `0`), ignoring which individual component moved which way.
/// A shrink in one component masked by a larger grow in another passes through this
/// mutant as a clean, affirmative direction instead of `incomparable`.
fn mutant_by_total_magnitude(before: &Bounds, after: &Bounds) -> Relation {
    fn total(b: &Bounds) -> i64 {
        let exploration = |bound: ExplorationBound| match bound {
            ExplorationBound::Bounded(n) => n,
            ExplorationBound::Unbounded => 0,
        };
        let declared = |bound: DeclaredBound| match bound {
            DeclaredBound::Declared(n) => n,
            DeclaredBound::Undeclared => 0,
        };
        exploration(b.values())
            + declared(b.nodes())
            + declared(b.faults())
            + exploration(b.depth())
    }
    match total(before).cmp(&total(after)) {
        std::cmp::Ordering::Equal => Relation::Unchanged,
        std::cmp::Ordering::Less => Relation::Expanded,
        std::cmp::Ordering::Greater => Relation::Contracted,
    }
}

#[test]
fn negative_mutant_by_total_magnitude_would_wrongly_pass_the_mixed_move_as_a_clean_direction() {
    // `nodes` shrinks by 1 (3 -> 2), `faults` grows by 2 (2 -> 4): the real classifier
    // sees mixed movement regardless of the net; the mutant nets the totals (5 -> 6)
    // and reports a clean `expanded`.
    let before = bounds(
        ExplorationBound::Bounded(2),
        DeclaredBound::Declared(3),
        DeclaredBound::Declared(2),
        ExplorationBound::Unbounded,
    );
    let after = bounds(
        ExplorationBound::Bounded(2),
        DeclaredBound::Declared(2),
        DeclaredBound::Declared(4),
        ExplorationBound::Unbounded,
    );

    let real_forward = classify_bounds(&before, &after).relation();
    assert_eq!(real_forward, Relation::Incomparable);
    let mutant_forward = mutant_by_total_magnitude(&before, &after);
    assert_eq!(
        mutant_forward,
        Relation::Expanded,
        "the mutant must actually get this wrong, or it is not exercising the bug"
    );
    assert_ne!(real_forward, mutant_forward);

    // Reverse direction: the real classifier still sees mixed movement (still
    // `incomparable`); the mutant's net now runs the other way (6 -> 5) and reports
    // the opposite wrong answer, `contracted` — a different mistake than the forward
    // case, confirming the bug is not a one-off sign error in the test itself.
    let real_reverse = classify_bounds(&after, &before).relation();
    assert_eq!(real_reverse, Relation::Incomparable);
    let mutant_reverse = mutant_by_total_magnitude(&after, &before);
    assert_eq!(
        mutant_reverse,
        Relation::Contracted,
        "the mutant must actually get this wrong, or it is not exercising the bug"
    );
    assert_ne!(real_reverse, mutant_reverse);
}

// --- boundary --------------------------------------------------------------------------------

#[test]
fn boundary_two_fully_unbounded_undeclared_tuples_classify_unchanged() {
    let empty = bounds(
        ExplorationBound::Unbounded,
        DeclaredBound::Undeclared,
        DeclaredBound::Undeclared,
        ExplorationBound::Unbounded,
    );
    let also_empty = bounds(
        ExplorationBound::Unbounded,
        DeclaredBound::Undeclared,
        DeclaredBound::Undeclared,
        ExplorationBound::Unbounded,
    );
    assert_eq!(
        classify_bounds(&empty, &also_empty).relation(),
        Relation::Unchanged
    );
}
