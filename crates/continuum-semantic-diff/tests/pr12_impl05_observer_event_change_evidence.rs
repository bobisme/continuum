//! PR-12 / IMPL-05 exit evidence — observer event change classification (`bn-1sdp`).
//!
//! # What this file is evidence for
//!
//! `src/observers.rs` classifies `observers[]` per RFC 0031's "observers" rule. This
//! file is the dedicated adversarial and clause-mapped evidence the bone brief asks
//! for, on top of that module's own unit tests (which stay the source of truth for
//! the four componentwise outcomes and are not re-derived here):
//!
//! > Evidence tests with module-doc clause→test map; anti-vacuity mutants; the
//! > removed-event-is-never-benign property deserves its own adversarial test (try to
//! > spell the hiding as something innocent; the type system or classifier must
//! > refuse).
//!
//! # Clause → test map
//!
//! | RFC 0031 / plan clause | Test |
//! |---|---|
//! | "Dropping an event family... is the 'hide observer events' attack (plan §19.5)" | [`positive_the_named_attack_on_a_real_corpus_fixture_classifies_coarsened`] |
//! | plan §5.1 gaming-move row: `observers` → `coarsened` → blocked by `review` | [`the_named_attack_is_reviewed_not_allowed_under_the_fixtures_own_policy`] |
//! | docs/50 "coarsen observer" intent attack | same two tests above; the fixture's own `policy.observers = "review"` is the governance response docs/50 names |
//! | "any mixed movement is `incomparable`" | [`negative_a_same_size_event_swap_is_never_read_as_a_clean_rename_or_no_op`] |
//! | "a unit key present on one side only classifies `added` or `removed`" | [`negative_deleting_the_real_fixtures_observer_outright_is_removed_not_silence`] |
//! | INV-001 / INV-011 (weakening is privileged, never spellable as benign) | [`property_no_attempted_disguise_of_a_dropped_element_ever_reads_as_unchanged_refined_or_added`] |
//! | fail-closed rule, P2 (non-affirmative relations block under every verb, even `unlocked`) | [`incomparable_blocks_even_under_the_least_restrictive_verb`] |
//! | anti-vacuity: the positive tests are not vacuously true | [`negative_mutant_blind_to_shrinkage_would_wrongly_pass_the_drop_as_a_refinement`], [`negative_mutant_by_total_cardinality_would_wrongly_pass_the_swap_as_unchanged`] |
//! | schema fact this module's "never `unsupported`" reasoning depends on | `src/observers.rs`'s own `the_live_schema_carries_no_fragment_member_on_observers_items` (not re-tested here) |
//!
//! # House rules, inherited from the PR-4/INV-012 evidence precedent
//!
//! - `src/` is untouched by this file, and no existing test anywhere is edited.
//! - [`FIXTURE`] is a real, already-reviewed corpus document
//!   (`crates/continuum-intent/tests/fixtures/replicated-register-contract.json`,
//!   already used by `tests/pr4_exit_evidence.rs` and
//!   `tests/inv012_nonvacuity_evidence.rs`), read at compile time via `include_str!`.
//!   Every mutation of it below is one bounded, linear `.replace`/`.replacen` on that
//!   fixed ~3 KiB string — never a loop of doublings, never nested growth.
//! - Positive, negative, and boundary evidence are each present and separately named,
//!   per this bone's brief.

use std::collections::BTreeSet;

use continuum_intent::canonical_json::Json;
use continuum_intent::change_policy::{
    ClassificationRecord, PolicyDecision, PolicyField, PolicyReviewers, PolicyTable, PolicyVerb,
    Relation,
};
use continuum_intent::observers::{Observer, ObserverId, ObserverSet, ProjectionKind};
use continuum_semantic_diff::observers::classify_observers;

/// The real corpus fixture: the replicated-register Intent Contract, also used by
/// `continuum-intent`'s own PR-4 exit and INV-012 evidence.
const FIXTURE: &str =
    include_str!("../../continuum-intent/tests/fixtures/replicated-register-contract.json");

/// A well-formed, minimal observer, for the synthetic disguise sweep.
fn observer(name: &str, events: &[&str], state: &[&str]) -> Observer {
    Observer::new(
        ObserverId::new(name).expect("a test observer id is non-empty"),
        [
            (
                ProjectionKind::Events,
                events.iter().map(|e| (*e).to_owned()).collect(),
            ),
            (
                ProjectionKind::State,
                state.iter().map(|s| (*s).to_owned()).collect(),
            ),
        ],
    )
    .expect("a test observer is well formed")
}

fn set(observers: impl IntoIterator<Item = Observer>) -> ObserverSet {
    ObserverSet::from_observers(observers).expect("distinct ids in a test fixture")
}

fn relation_of(
    changes: &[continuum_semantic_diff::observers::ObserverChange],
    unit: &str,
) -> Relation {
    changes
        .iter()
        .find(|change| change.unit().as_str() == unit)
        .unwrap_or_else(|| panic!("no record for unit {unit:?} in {changes:?}"))
        .relation()
}

/// Decode the fixture's `observers` array.
fn fixture_observers(document: &str) -> ObserverSet {
    let parsed = Json::parse(document.as_bytes()).expect("the fixture is canonical JSON");
    let observers = parsed
        .as_object()
        .expect("the fixture is an object")
        .get("observers")
        .expect("the fixture declares observers");
    ObserverSet::from_json(observers).expect("the fixture's observers decode")
}

/// Decode the fixture's `policy` table — its own governance choice for `observers`.
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
/// every `review`-verb field, and the fixture names one for `observers`.
fn fixture_reviewers(document: &str) -> PolicyReviewers {
    let parsed = Json::parse(document.as_bytes()).expect("the fixture is canonical JSON");
    let reviewers = parsed
        .as_object()
        .expect("the fixture is an object")
        .get("policy_reviewers")
        .expect("the fixture declares policy_reviewers");
    PolicyReviewers::from_json(reviewers).expect("the fixture's reviewer map decodes")
}

// --- baseline: the fixture's one observer round-trips and is what we think it is ------------

#[test]
fn the_fixtures_one_observer_is_client_with_committed_and_chosen() {
    let observers = fixture_observers(FIXTURE);
    assert_eq!(observers.len(), 1);
    let client = observers
        .get(&ObserverId::new("client").unwrap())
        .expect("the fixture names a `client` observer");
    assert_eq!(client.events(), &BTreeSet::from(["Committed".to_owned()]));
    assert_eq!(
        client.projection(ProjectionKind::State),
        &BTreeSet::from(["chosen".to_owned()])
    );
    // Compared against itself, nothing moved.
    let changes = classify_observers(&observers, &observers);
    assert_eq!(changes.len(), 1);
    assert_eq!(relation_of(&changes, "client"), Relation::Unchanged);
}

// --- positive: the named attack, on the real fixture, correctly caught ----------------------

#[test]
fn positive_the_named_attack_on_a_real_corpus_fixture_classifies_coarsened() {
    // RFC 0031's own example move, applied to `client` rather than the illustrative
    // `Agreement`: drop the one event family the fixture declares. This is plan
    // §19.5's "hide observer events" and docs/50's "coarsen observer", enacted on a
    // real, already-reviewed contract rather than a synthetic stand-in.
    let before = fixture_observers(FIXTURE);
    let coarsened_fixture = FIXTURE.replacen(r#""events":["Committed"]"#, r#""events":[]"#, 1);
    assert_ne!(
        coarsened_fixture, FIXTURE,
        "the replacement must actually fire"
    );
    let after = fixture_observers(&coarsened_fixture);

    let changes = classify_observers(&before, &after);
    assert_eq!(changes.len(), 1);
    assert_eq!(relation_of(&changes, "client"), Relation::Coarsened);
}

#[test]
fn the_named_attack_is_reviewed_not_allowed_under_the_fixtures_own_policy() {
    // Close the loop into the policy machinery PR-4 already landed
    // (`continuum_intent::change_policy::PolicyTable::verdict`): the fixture's own
    // `policy.observers` is `"review"` (its authors' governance choice, matching RFC
    // 0031's worked example row exactly), so the coarsening record must not `allow`.
    let before = fixture_observers(FIXTURE);
    let coarsened_fixture = FIXTURE.replacen(r#""events":["Committed"]"#, r#""events":[]"#, 1);
    let after = fixture_observers(&coarsened_fixture);
    let policy = fixture_policy(FIXTURE);
    assert_eq!(policy.verb(PolicyField::Observers), PolicyVerb::Review);

    let observer_change = classify_observers(&before, &after)
        .into_iter()
        .find(|c| c.unit().as_str() == "client")
        .expect("client is classified");
    assert_eq!(observer_change.relation(), Relation::Coarsened);

    // P1 needs one record per field; every other field is genuinely unchanged in this
    // scenario (only `observers` was mutated), so `Unchanged` is the honest record —
    // this is the field-level completeness the module doc says the classifier itself
    // does not supply, filled in here by the test, not invented as a real diff would
    // invent it.
    let mut records: Vec<ClassificationRecord> = PolicyField::ALL
        .into_iter()
        .filter(|field| *field != PolicyField::Observers)
        .map(|field| {
            ClassificationRecord::new(field, Relation::Unchanged)
                .expect("unchanged is always admissible")
        })
        .collect();
    records.push(
        observer_change
            .to_classification_record()
            .expect("Coarsened is admissible on observers"),
    );

    let verdict = policy
        .verdict(
            &records,
            &fixture_reviewers(FIXTURE),
            continuum_intent::change_policy::AcceptancePath::AgentAccept,
        )
        .expect("a complete classification against a well-formed policy table computes");
    assert_eq!(verdict.decision(), PolicyDecision::Review);
    assert!(
        verdict
            .reasons()
            .iter()
            .any(|reason| reason.field() == PolicyField::Observers
                && reason.relation() == Relation::Coarsened),
        "{:?}",
        verdict.reasons()
    );
}

// --- negative: membership, not silence -------------------------------------------------------

#[test]
fn negative_deleting_the_real_fixtures_observer_outright_is_removed_not_silence() {
    let before = fixture_observers(FIXTURE);
    let after = set([]);
    let changes = classify_observers(&before, &after);
    assert_eq!(changes.len(), 1);
    assert_eq!(relation_of(&changes, "client"), Relation::Removed);
}

// --- negative: mixed movement is never a clean rename or a no-op ----------------------------

#[test]
fn negative_a_same_size_event_swap_is_never_read_as_a_clean_rename_or_no_op() {
    let before = set([observer("client", &["SecurityAudit"], &["chosen"])]);
    let after = set([observer("client", &["Heartbeat"], &["chosen"])]);
    let changes = classify_observers(&before, &after);
    let relation = relation_of(&changes, "client");
    assert_ne!(relation, Relation::Unchanged);
    assert_ne!(relation, Relation::Refined);
    assert_eq!(relation, Relation::Incomparable);
}

#[test]
fn incomparable_blocks_even_under_the_least_restrictive_verb() {
    // P2's unconditional half: a non-affirmative relation contributes at least
    // `review` "on every field under every verb" — including `unlocked`, the verb
    // that would otherwise `allow` any affirmative relation outright (P4).
    let table = PolicyTable::all_unlocked();
    assert_eq!(table.verb(PolicyField::Observers), PolicyVerb::Unlocked);
    let mut records: Vec<ClassificationRecord> = PolicyField::ALL
        .into_iter()
        .filter(|field| *field != PolicyField::Observers)
        .map(|field| ClassificationRecord::new(field, Relation::Unchanged).expect("admissible"))
        .collect();
    records.push(
        ClassificationRecord::new(PolicyField::Observers, Relation::Incomparable)
            .expect("Incomparable is admissible on observers"),
    );
    let verdict = table
        .verdict(
            &records,
            &PolicyReviewers::empty(),
            continuum_intent::change_policy::AcceptancePath::AgentAccept,
        )
        .expect("complete classification against a well-formed table computes");
    assert_eq!(verdict.decision(), PolicyDecision::Review);
}

// --- property: no disguise of a dropped element ever reads as benign ------------------------

/// Every attempted "innocent" disguise of an event-family or projection-element drop,
/// checked against one property: the resulting per-unit relation set for the touched
/// id(s) never contains [`Relation::Unchanged`], [`Relation::Refined`], or a bare
/// [`Relation::Added`] standing in for the dropped unit — RFC 0031's `unit` was
/// present in `before`, so it must appear as `coarsened`, `incomparable`, or
/// `removed`, and never simply vanish from the output or reappear as a clean
/// `added` under the same key with no matching `removed`.
#[test]
fn property_no_attempted_disguise_of_a_dropped_element_ever_reads_as_unchanged_refined_or_added() {
    let base = observer("Agreement", &["Commit", "Abort", "Timeout"], &["s1", "s2"]);
    let before = set([base.clone()]);

    // Every disguise below drops at least one element `base` declared. None may
    // classify `unchanged`, `refined`, or `added`-with-no-`removed` for the id that
    // held it.
    let disguises: Vec<(&str, ObserverSet)> = vec![
        (
            "drop one event, add nothing",
            set([observer("Agreement", &["Commit", "Abort"], &["s1", "s2"])]),
        ),
        (
            "drop one event, add a decoy event of equal weight",
            set([observer("Agreement", &["Commit", "Decoy"], &["s1", "s2"])]),
        ),
        (
            "drop one event, grow an unrelated projection",
            set([observer(
                "Agreement",
                &["Commit", "Abort"],
                &["s1", "s2", "s3"],
            )]),
        ),
        (
            "drop one state element, grow events",
            set([observer(
                "Agreement",
                &["Commit", "Abort", "Timeout", "Extra"],
                &["s1"],
            )]),
        ),
        (
            "drop everything from one component, hold the other",
            set([observer("Agreement", &[], &["s1", "s2"])]),
        ),
        (
            "rename while preserving every remaining element",
            set([observer(
                "AgreementRenamed",
                &["Commit", "Abort", "Timeout"],
                &["s1", "s2"],
            )]),
        ),
        ("delete the unit outright", set([])),
    ];

    for (name, after) in disguises {
        let changes = classify_observers(&before, &after);
        // The old unit key `Agreement` must appear unless the disguise itself moved
        // it under a new key (the rename and delete cases); when it does appear, it
        // must never be `unchanged` or `refined`.
        if let Some(old) = changes.iter().find(|c| c.unit().as_str() == "Agreement") {
            assert!(
                matches!(
                    old.relation(),
                    Relation::Coarsened | Relation::Incomparable | Relation::Removed
                ),
                "disguise {name:?} let `Agreement` classify {:?}",
                old.relation()
            );
        } else {
            panic!(
                "disguise {name:?} produced no record at all for `Agreement`; a dropped unit must never vanish from a total classification"
            );
        }
        // And nothing in the whole record set claims `Agreement`'s content survived
        // unscathed under a fresh `added` key with no corresponding `removed` for the
        // old one — i.e. a rename can never present as pure growth.
        let any_bare_addition_of_full_content = changes.iter().any(|c| {
            c.relation() == Relation::Added
                && c.unit().as_str() != "Agreement"
                && !changes
                    .iter()
                    .any(|other| other.unit().as_str() == "Agreement")
        });
        assert!(
            !any_bare_addition_of_full_content,
            "disguise {name:?} let the old unit disappear without a matching record"
        );
    }
}

// --- boundary ---------------------------------------------------------------------------------

#[test]
fn boundary_two_empty_sets_classify_nothing() {
    let empty = set([]);
    assert!(classify_observers(&empty, &empty).is_empty());
}

#[test]
fn boundary_an_observer_with_every_projection_empty_compares_as_unchanged_against_itself() {
    let o = observer("silent", &[], &[]);
    let before = set([o.clone()]);
    let after = set([o]);
    let changes = classify_observers(&before, &after);
    assert_eq!(relation_of(&changes, "silent"), Relation::Unchanged);
}

// --- anti-vacuity mutants: the positive assertions are not vacuously true -------------------
//
// Each mutant below is a plausible, *wrong* classifier — the kind of bug this file's
// positive tests exist to catch. Run the same fixture through it and show it gets the
// wrong answer where `classify_observers` gets the right one; that is the proof this
// file's assertions are load-bearing rather than tautological.

/// Mutant 1: ignores shrinkage entirely and looks only at whether the `events`
/// component's *cardinality* went up. A classifier with this bug would report a
/// dropped-and-replaced event family as a *refinement* — the single worst outcome,
/// since it reads a weakening as its opposite.
fn mutant_blind_to_shrinkage(before: &Observer, after: &Observer) -> Relation {
    if after.events().len() >= before.events().len() {
        Relation::Refined
    } else {
        Relation::Coarsened
    }
}

#[test]
fn negative_mutant_blind_to_shrinkage_would_wrongly_pass_the_drop_as_a_refinement() {
    // The real attack shape: drop the audited event, add two unrelated decoys — net
    // cardinality grows, but `Committed` is gone.
    let before = observer("client", &["Committed"], &[]);
    let after = observer("client", &["Heartbeat", "Ping"], &[]);

    let real = relation_of(
        &classify_observers(&set([before.clone()]), &set([after.clone()])),
        "client",
    );
    assert_eq!(
        real,
        Relation::Incomparable,
        "the real classifier sees `Committed` dropped and two decoys added: a mixed movement"
    );

    let mutant_relation = mutant_blind_to_shrinkage(&before, &after);
    assert_eq!(
        mutant_relation,
        Relation::Refined,
        "the mutant must actually get this wrong, or it is not exercising the bug"
    );
    assert_ne!(
        real, mutant_relation,
        "the real classifier must disagree with the mutant on the attack case"
    );
}

/// Mutant 2: compares total element counts summed across all four components,
/// ignoring *which* elements they are. A same-size swap — drop the audited event,
/// add a decoy of equal weight — passes through this mutant as `unchanged`.
fn mutant_by_total_cardinality(before: &Observer, after: &Observer) -> Relation {
    let before_total: usize = before.components().iter().map(|(_, s)| s.len()).sum();
    let after_total: usize = after.components().iter().map(|(_, s)| s.len()).sum();
    if before_total == after_total {
        Relation::Unchanged
    } else if after_total > before_total {
        Relation::Refined
    } else {
        Relation::Coarsened
    }
}

#[test]
fn negative_mutant_by_total_cardinality_would_wrongly_pass_the_swap_as_unchanged() {
    let before = observer("client", &["SecurityAudit"], &[]);
    let after = observer("client", &["Heartbeat"], &[]);

    let real = relation_of(
        &classify_observers(&set([before.clone()]), &set([after.clone()])),
        "client",
    );
    assert_eq!(real, Relation::Incomparable);

    let mutant_relation = mutant_by_total_cardinality(&before, &after);
    assert_eq!(
        mutant_relation,
        Relation::Unchanged,
        "the mutant must actually get this wrong, or it is not exercising the bug"
    );
    assert_ne!(
        real, mutant_relation,
        "the real classifier must disagree with the mutant on the swap"
    );
}
