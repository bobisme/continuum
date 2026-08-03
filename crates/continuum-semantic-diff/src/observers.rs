//! Observer-group classification: RFC 0031's `observers` rule (PR-12 / IMPL-05).
//!
//! # What this module answers
//!
//! Given two [`ObserverSet`]s — an Intent Contract's `observers[]` before and after a
//! proposed revision — classify every unit into RFC 0031's closed relation vocabulary:
//!
//! > ### `observers`
//! >
//! > Keyed by `id`. Compare the four sets `events`, `state_projection`,
//! > `knowledge_projection`, `security_projection` componentwise. All four supersets,
//! > at least one strict, is `refined`; all four subsets, at least one strict, is
//! > `coarsened`; equal on all four is `unchanged`; any mixed movement is
//! > `incomparable` — the componentwise rule admits no exception for this case.
//! > Dropping an event family or a projection element, with no other component growing
//! > in the same comparison, is the "hide observer events" attack (plan §19.5) and
//! > classifies `coarsened` under the rule just stated. If another component grows in
//! > the same comparison, the movement is mixed: it classifies `incomparable`, not
//! > `coarsened`, and still blocks ordinary promotion under the fail-closed rule,
//! > exactly as every other non-affirmative relation does.
//! >
//! > — `notes/plan/rfcs/0031-semantic-and-intent-diff.md`, "`observers`" (correction 15,
//! > bn-13nlo)
//!
//! and the general per-unit membership rule that section inherits from "Field-by-field
//! classification rules": a unit key present on one side only classifies `added` or
//! `removed`.
//!
//! # Why observer coarsening is a named attack, not an edge case
//!
//! > ### Intent attacks
//! >
//! > - weaken property;
//! > - strengthen assumptions/fairness;
//! > - shrink bounds;
//! > - remove faults;
//! > - **coarsen observer**;
//! > - lower assurance.
//! >
//! > — `notes/plan/docs/50_AGENT_EVALUATION_AND_REWARD_HACKING.md`, "Intent attacks"
//!
//! and RFC 0031's own worked example of the move:
//!
//! > | Gaming move (plan §5.1) | Field | Relation | Blocking verb (plan §5.4) |
//! > |---|---|---|---|
//! > | change `Agreement` to a weaker observer | `observers` | `coarsened` | `review` |
//! >
//! > — RFC 0031, "Completeness guarantee"
//!
//! An observer is what a claim is verified *against*: it is the projection of a
//! system's behavior a property is stated over. Coarsening it — dropping an event
//! family, or a state/knowledge/security projection element — does not touch the
//! claim's text at all, so a property-only diff would see nothing. It is the
//! quietest way to make a claim's verified statement weaker than its authored one,
//! which is exactly what INV-001 ("ordinary tasks may not mutate intent") and
//! INV-011 ("a repair transaction cannot be promoted if it changes protected intent
//! unless explicitly reclassified") exist to stop: an observer is intent (RFC 0037
//! makes `observers` one of the fifteen protected policy keys), coarsening it is a
//! weakening, and INV-001/INV-011's "weakening-is-privileged" posture applies to it
//! exactly as it applies to a weakened property. This module is the mechanism that
//! makes the weakening visible: it is the reason "hide observer events" cannot be
//! spelled as a change with no `intent_changes` record.
//!
//! # `unit`, not just `field`
//!
//! [`continuum_intent::change_policy::ClassificationRecord`] is `{field, relation}`
//! only — deliberately, per its own module doc: "The per-unit locator the wire cannot
//! carry today is RFC 0031's F1 and is not invented here." [`ObserverChange`] is not
//! that record; it is the finer-grained fact this module actually computes — RFC
//! 0031's per-*unit* classification — and it carries the [`ObserverId`] the record
//! shape cannot. [`ObserverChange::to_classification_record`] is the lossy projection
//! from one to the other, for a caller feeding
//! [`continuum_intent::change_policy::PolicyTable::verdict`], which only needs
//! `{field, relation}` per record and accepts several records for one field.
//!
//! # Why the mixed case is `incomparable`, not `coarsened`
//!
//! RFC 0031's `observers` rule (quoted above) now states the componentwise rule
//! without exception: a comparison with growth in one component and shrinkage in
//! another classifies `incomparable`, never `coarsened`. This module implements that
//! reading exactly (`classify_pair`, below), with no special case that forces
//! `coarsened` over `incomparable` when the two could disagree. The RFC text used to
//! read as self-contradictory here — a literal "MUST classify `coarsened` even when
//! other components grow" clause conflicting with the componentwise rule one sentence
//! earlier — and was corrected to the text quoted above (RFC 0031 correction 15,
//! bn-13nlo) to match the reading this module already implemented (bn-1sdp). The
//! reading is also the *safer* one: [`Relation::Incomparable`] is non-affirmative, and
//! `PolicyTable::verdict`'s P2 makes every non-affirmative relation contribute at
//! least `review` **on every field under every verb, including `unlocked`**; a bare
//! [`Relation::Coarsened`] only blocks under a verb whose denied set names it, which
//! the closed verb set does not do for `observers`.
//!
//! # What never arises here, and why that is not evasion
//!
//! [`PolicyField::Observers`]'s admissible-relation row lists `unknown` and
//! `unsupported` (always admissible, per the fail-closed rule applied to every field).
//! Neither is ever returned by [`classify_observers`]. `unknown` is RFC 0031's answer
//! when a direction cannot be decided at the requested assurance — solver-dependent,
//! per the "expression relations" table — and `observers` is compared by plain finite
//! set inclusion, which is decidable outright; there is no `requested_assurance` this
//! comparison could be too weak for. `unsupported` is RFC 0031's answer when a unit's
//! *fragment* is not declared in `scope.fragments` ("Fragment attribution"; "Removing
//! a fragment... those units then classify `unsupported`"); `observers[]` carries no
//! `fragment` member in `notes/plan/schemas/intent-contract.schema.json` — confirmed
//! in this module's tests against the live schema — so no observer unit has a
//! fragment to fall outside of. A total, always-decidable classifier is the honest
//! answer for this field, not a shortcut around the fail-closed rule: the rule exists
//! to stop a classifier from *guessing* past what it can decide, and nothing here
//! guesses.
//!
//! # Scope: PR-12 / IMPL-05 only
//!
//! PR 12 (`notes/START_HERE_IMPLEMENTATION.md`) lists six classification bullets:
//! exact equality, property AST edit, assumption add/remove, bound change, **observer
//! event change**, and fault/fairness/assurance change. This bone (`bn-1sdp`) delivers
//! the fifth, and only the fifth. The other five are separate open bones —
//! `bn-b8ru` (IMPL-01, exact equality), `bn-8mlg` (IMPL-02, property AST edit),
//! `bn-7vg7` (IMPL-03, assumption add/remove), `bn-ycn6` (IMPL-04, bound change),
//! `bn-3vxp` (IMPL-06, fault/fairness/assurance change) — and land as their own
//! reviewable units, per this crate's PR-1 scaffold note ("the types and behavior land
//! in the PR named above", now specialized per bullet). Also out of scope, honestly:
//! the wire `intent_changes[]` JSON shape (`evidence`, `fragment` as a *serialized*
//! member, `protected`), the whole-artifact assembly (`schema_id`, `before_snapshot`,
//! `policy.decision`), and the field-level P1 completeness a
//! [`continuum_intent::change_policy::PolicyTable::verdict`] call needs even when
//! zero units changed (a field-level "at least one `unchanged` record" the caller
//! supplies, distinct from this module's per-*unit* output) — all three belong to
//! whatever later bone assembles a full `diff_*` artifact across all fifteen fields,
//! not to one field's classifier.

use std::collections::BTreeSet;

use continuum_intent::change_policy::{
    ChangePolicyError, ClassificationRecord, PolicyField, Relation,
};
use continuum_intent::observers::{Observer, ObserverId, ObserverSet};

/// One classified `observers[]` unit: an [`ObserverId`] and the [`Relation`] RFC 0031
/// assigns it.
///
/// The relation is always one [`PolicyField::Observers`] admits — pinned by this
/// module's tests, not merely asserted — because every relation this module produces
/// comes from [`classify_pair`] or the added/removed membership check in
/// [`classify_observers`], and both are closed over
/// `{Unchanged, Refined, Coarsened, Added, Removed, Incomparable}`, all six of which
/// are in the observers row (`unknown` and `unsupported` are admissible on that row
/// too, per the fail-closed rule, but this module never emits them — see the module
/// doc, "What never arises here").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObserverChange {
    unit: ObserverId,
    relation: Relation,
}

impl ObserverChange {
    fn new(unit: ObserverId, relation: Relation) -> Self {
        debug_assert!(
            PolicyField::Observers.admits_relation(relation),
            "an observer classification produced {relation}, which the observers row does not admit"
        );
        Self { unit, relation }
    }

    /// The classified unit: RFC 0031's per-unit locator for `observers`, "keyed by
    /// `id`".
    #[must_use]
    pub const fn unit(&self) -> &ObserverId {
        &self.unit
    }

    /// The RFC 0031 relation this unit classified to.
    #[must_use]
    pub const fn relation(&self) -> Relation {
        self.relation
    }

    /// This change as a field-level [`ClassificationRecord`], for
    /// [`continuum_intent::change_policy::PolicyTable::verdict`].
    ///
    /// The record drops the unit locator — `ClassificationRecord` is `{field,
    /// relation}` only (RFC 0031 F1) — and keeps the relation. `verdict` accepts
    /// several records naming the same field, so calling this once per
    /// non-`unchanged` [`ObserverChange`] and folding the results into one records
    /// slice is a sound way to feed several observer-unit weakenings into one
    /// verdict computation.
    ///
    /// # Errors
    ///
    /// [`ChangePolicyError::InadmissibleRelation`] — never in practice, since every
    /// relation this module produces is on the observers row (see the struct doc and
    /// `every_relation_this_module_can_produce_is_admissible_on_the_observers_field`),
    /// but [`ClassificationRecord::new`] is itself fallible and a `debug_assert!` is
    /// not a proof, so the `Result` is threaded rather than unwrapped.
    pub fn to_classification_record(&self) -> Result<ClassificationRecord, ChangePolicyError> {
        ClassificationRecord::new(PolicyField::Observers, self.relation)
    }
}

/// Classify every `observers[]` unit between `before` and `after`, per RFC 0031's
/// "observers" rule.
///
/// Total: one [`ObserverChange`] for every unit key declared in `before`, in `after`,
/// or both — including [`Relation::Unchanged`] units. RFC 0031 permits a wire artifact
/// to omit `unchanged` records ("Field-by-field classification rules": "Records whose
/// relation is `unchanged` MAY be omitted"), but that is an emission choice for
/// whatever assembles the wire `intent_changes[]` array, not a property of the
/// classifier; a caller that wants the omitting projection filters this function's
/// result by [`Relation::is_unchanged`].
///
/// The three relations RFC 0031 gives for membership and componentwise movement:
///
/// - a unit present only in `after` classifies [`Relation::Added`];
/// - a unit present only in `before` classifies [`Relation::Removed`];
/// - a unit present in both is classified by [`classify_pair`].
///
/// Units are visited in [`ObserverId`] order (the union of both sides' `unit_keys()`,
/// which are already `BTreeSet`s), so the output order is deterministic and does not
/// depend on either input's authored order — matching RFC 0031's "Determinism": "the
/// diff artifact MUST be byte-identical across platforms and releases".
#[must_use]
pub fn classify_observers(before: &ObserverSet, after: &ObserverSet) -> Vec<ObserverChange> {
    let mut keys: BTreeSet<ObserverId> = before.unit_keys().into_iter().cloned().collect();
    keys.extend(after.unit_keys().into_iter().cloned());
    keys.into_iter()
        .map(|unit| {
            let relation = match (before.get(&unit), after.get(&unit)) {
                (None, Some(_)) => Relation::Added,
                (Some(_), None) => Relation::Removed,
                (Some(b), Some(a)) => classify_pair(b, a),
                (None, None) => {
                    unreachable!("`unit` came from the union of both sides' own unit_keys()")
                }
            };
            ObserverChange::new(unit, relation)
        })
        .collect()
}

/// RFC 0031's componentwise comparison for one observer present on both sides.
///
/// Compares [`Observer::components`] — the four sets in [`ProjectionKind::ALL`] order
/// — pairwise. A component "grew" if `after`'s set has an element `before`'s does not
/// (not a superset test alone: two same-size sets with different elements, `{A,B}` vs
/// `{A,C}`, both grew *and* shrank in the same component, which is exactly the
/// "swap one event for another" disguise a cardinality-only check would miss — see
/// this module's `negative_a_same_size_event_swap_is_incomparable_not_unchanged`
/// test). A component "shrank" symmetrically. The four outcomes:
///
/// - nothing grew and nothing shrank, anywhere: [`Relation::Unchanged`] — equality of
///   all four sets, which (R2, RFC 0031) is exactly equality of the observer's
///   canonical encoding, since `observers[]` has no field outside the four sets plus
///   `id`, and `id` is fixed across this comparison. `debug_assert_eq!` below pins
///   this equivalence on every call rather than trusting it.
/// - something grew, nothing shrank, anywhere: [`Relation::Refined`] — "all four
///   supersets, at least one strict".
/// - something shrank, nothing grew, anywhere: [`Relation::Coarsened`] — "all four
///   subsets, at least one strict". This is the "hide observer events" attack's
///   direct classification.
/// - both happened — somewhere grew, somewhere (possibly the same component, possibly
///   a different one) shrank: [`Relation::Incomparable`] — "any mixed movement".
#[must_use]
fn classify_pair(before: &Observer, after: &Observer) -> Relation {
    let mut grew = false;
    let mut shrank = false;
    for (kind, before_set) in before.components() {
        let after_set = after.projection(kind);
        if !before_set.is_superset(after_set) {
            grew = true;
        }
        if !after_set.is_superset(before_set) {
            shrank = true;
        }
    }
    let relation = match (grew, shrank) {
        (false, false) => Relation::Unchanged,
        (true, false) => Relation::Refined,
        (false, true) => Relation::Coarsened,
        (true, true) => Relation::Incomparable,
    };
    debug_assert_eq!(
        relation == Relation::Unchanged,
        before.identity().canonical_bytes() == after.identity().canonical_bytes(),
        "R2: componentwise set equality and canonical-encoding equality must agree, for {} and {}",
        before.locator(),
        after.locator(),
    );
    relation
}

#[cfg(test)]
mod tests {
    use continuum_intent::canonical_json::Json;
    use continuum_intent::observers::ProjectionKind;

    use super::*;

    /// The dossier's validated Intent Contract example, included at compile time — the
    /// same fixture `continuum-intent`'s `observers_group.rs` reads, used here to
    /// confirm the live schema still carries no `fragment` member on `observers[]`
    /// (the module doc's "What never arises here" claim).
    const SCHEMA: &str = include_str!("../../../notes/plan/schemas/intent-contract.schema.json");

    fn id(text: &str) -> ObserverId {
        ObserverId::new(text).expect("a test observer id is non-empty")
    }

    fn observer(name: &str, events: &[&str], state: &[&str]) -> Observer {
        Observer::new(
            id(name),
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

    fn relation_of(changes: &[ObserverChange], unit: &str) -> Relation {
        changes
            .iter()
            .find(|change| change.unit().as_str() == unit)
            .unwrap_or_else(|| panic!("no record for unit {unit:?} in {changes:?}"))
            .relation()
    }

    // --- the four componentwise outcomes ---------------------------------------------------

    #[test]
    fn equal_observers_classify_unchanged() {
        let before = set([observer("client", &["A"], &["s"])]);
        let after = set([observer("client", &["A"], &["s"])]);
        let changes = classify_observers(&before, &after);
        assert_eq!(changes.len(), 1);
        assert_eq!(relation_of(&changes, "client"), Relation::Unchanged);
    }

    #[test]
    fn strictly_growing_every_component_classifies_refined() {
        let before = set([observer("client", &["A"], &["s"])]);
        let after = set([observer("client", &["A", "B"], &["s", "t"])]);
        let changes = classify_observers(&before, &after);
        assert_eq!(relation_of(&changes, "client"), Relation::Refined);
    }

    #[test]
    fn growing_one_component_and_holding_the_rest_still_classifies_refined() {
        // "All four supersets, at least one strict" — the other three may be equal.
        let before = set([observer("client", &["A"], &["s"])]);
        let after = set([observer("client", &["A", "B"], &["s"])]);
        let changes = classify_observers(&before, &after);
        assert_eq!(relation_of(&changes, "client"), Relation::Refined);
    }

    #[test]
    fn dropping_an_event_family_alone_classifies_coarsened() {
        // The named attack, isolated: nothing else moves.
        let before = set([observer("client", &["A", "B"], &["s"])]);
        let after = set([observer("client", &["A"], &["s"])]);
        let changes = classify_observers(&before, &after);
        assert_eq!(relation_of(&changes, "client"), Relation::Coarsened);
    }

    #[test]
    fn dropping_a_projection_element_alone_is_as_visible_as_dropping_an_event() {
        let before = set([observer("client", &["A"], &["s", "t"])]);
        let after = set([observer("client", &["A"], &["s"])]);
        let changes = classify_observers(&before, &after);
        assert_eq!(relation_of(&changes, "client"), Relation::Coarsened);
    }

    #[test]
    fn shrinking_every_component_classifies_coarsened() {
        let before = set([observer("client", &["A", "B"], &["s", "t"])]);
        let after = set([observer("client", &["A"], &["s"])]);
        let changes = classify_observers(&before, &after);
        assert_eq!(relation_of(&changes, "client"), Relation::Coarsened);
    }

    #[test]
    fn one_component_shrinking_while_another_grows_is_incomparable() {
        // The genuinely mixed case: `events` loses B, `state_projection` gains t.
        let before = set([observer("client", &["A", "B"], &["s"])]);
        let after = set([observer("client", &["A"], &["s", "t"])]);
        let changes = classify_observers(&before, &after);
        assert_eq!(relation_of(&changes, "client"), Relation::Incomparable);
    }

    #[test]
    fn negative_a_same_size_event_swap_is_incomparable_not_unchanged() {
        // Same cardinality on every component, different membership: a classifier
        // that compared set *sizes* rather than set *membership* would call this
        // unchanged. This is the "swap the audited event for a decoy of the same
        // weight" disguise; it must not read as benign.
        let before = set([observer("client", &["SecurityAudit"], &[])]);
        let after = set([observer("client", &["Heartbeat"], &[])]);
        let changes = classify_observers(&before, &after);
        let relation = relation_of(&changes, "client");
        assert_eq!(relation, Relation::Incomparable);
        // Non-affirmative: `PolicyTable::verdict`'s P2 makes this contribute at least
        // `review` on every field under every verb, including `unlocked` — the swap
        // cannot be waved through merely because the intent's `observers` verb is
        // permissive.
        assert!(!relation.is_affirmative());
    }

    // --- membership: added / removed --------------------------------------------------------

    #[test]
    fn a_unit_present_only_after_classifies_added() {
        let before = set([]);
        let after = set([observer("client", &["A"], &[])]);
        let changes = classify_observers(&before, &after);
        assert_eq!(relation_of(&changes, "client"), Relation::Added);
    }

    #[test]
    fn a_unit_present_only_before_classifies_removed() {
        let before = set([observer("client", &["A"], &[])]);
        let after = set([]);
        let changes = classify_observers(&before, &after);
        assert_eq!(relation_of(&changes, "client"), Relation::Removed);
    }

    #[test]
    fn negative_renaming_an_observer_is_removed_plus_added_never_unchanged() {
        // The rename disguise (parallel to `properties`' "renaming an id while
        // preserving the expression is removed plus added, not unchanged; the
        // classifier MUST NOT match claims by expression to defeat the rename"):
        // identical projections under a new id must not read as one unchanged unit.
        let before = set([observer("Agreement", &["Commit", "Abort"], &[])]);
        let after = set([observer("AgreementV2", &["Commit", "Abort"], &[])]);
        let changes = classify_observers(&before, &after);
        assert_eq!(changes.len(), 2, "{changes:?}");
        assert_eq!(relation_of(&changes, "Agreement"), Relation::Removed);
        assert_eq!(relation_of(&changes, "AgreementV2"), Relation::Added);
    }

    #[test]
    fn coarsening_and_then_renaming_the_same_observer_still_removes_the_old_unit() {
        // The compound disguise the bone brief names directly: coarsen a heavily
        // observed unit, then rename it, hoping a lenient classifier matches by
        // content and calls the whole thing a harmless rename. It must not: the old,
        // fully-observing id disappears (`removed`), full stop.
        let before = set([observer("Agreement", &["Commit", "Abort", "Timeout"], &[])]);
        let after = set([observer("AgreementV2", &["Commit"], &[])]);
        let changes = classify_observers(&before, &after);
        assert_eq!(changes.len(), 2, "{changes:?}");
        assert_eq!(relation_of(&changes, "Agreement"), Relation::Removed);
        assert_eq!(relation_of(&changes, "AgreementV2"), Relation::Added);
    }

    // --- totality and determinism -----------------------------------------------------------

    #[test]
    fn both_sides_empty_classifies_no_units_at_all() {
        let empty = set([]);
        assert!(classify_observers(&empty, &empty).is_empty());
    }

    #[test]
    fn unchanged_units_are_included_by_default_and_filterable() {
        let before = set([observer("a", &["X"], &[]), observer("b", &["Y"], &[])]);
        let after = set([observer("a", &["X"], &[]), observer("b", &["Y", "Z"], &[])]);
        let changes = classify_observers(&before, &after);
        assert_eq!(changes.len(), 2, "total classification names every unit");
        let non_trivial: Vec<_> = changes
            .iter()
            .filter(|c| c.relation() != Relation::Unchanged)
            .collect();
        assert_eq!(non_trivial.len(), 1);
        assert_eq!(non_trivial[0].unit().as_str(), "b");
    }

    #[test]
    fn output_order_is_unit_key_order_regardless_of_authored_order() {
        let before = set([observer("z", &["A"], &[]), observer("a", &["A"], &[])]);
        let after = set([observer("a", &["A"], &[]), observer("z", &["A"], &[])]);
        let changes = classify_observers(&before, &after);
        assert_eq!(
            changes
                .iter()
                .map(|c| c.unit().as_str())
                .collect::<Vec<_>>(),
            vec!["a", "z"]
        );
    }

    // --- the field-classification contract --------------------------------------------------

    #[test]
    fn every_relation_this_module_can_produce_is_admissible_on_the_observers_field() {
        for relation in [
            Relation::Unchanged,
            Relation::Refined,
            Relation::Coarsened,
            Relation::Added,
            Relation::Removed,
            Relation::Incomparable,
        ] {
            assert!(
                PolicyField::Observers.admits_relation(relation),
                "{relation} must be admissible on `observers`"
            );
        }
    }

    #[test]
    fn to_classification_record_never_fails_for_a_relation_this_module_produced() {
        let before = set([observer("client", &["A", "B"], &[])]);
        let after = set([observer("client", &["A"], &[])]);
        for change in classify_observers(&before, &after) {
            let record = change
                .to_classification_record()
                .expect("this module's own relations are always admissible on `observers`");
            assert_eq!(record.field(), PolicyField::Observers);
            assert_eq!(record.relation(), change.relation());
        }
    }

    #[test]
    fn the_live_schema_carries_no_fragment_member_on_observers_items() {
        // Backs the module doc's "What never arises here" claim that `unsupported`
        // cannot arise from fragment narrowing for this field: there is no fragment
        // to narrow.
        let schema = Json::parse(SCHEMA.as_bytes()).expect("the schema itself is canonical JSON");
        let observers_items = schema
            .as_object()
            .expect("schema root is an object")
            .get("properties")
            .and_then(Json::as_object)
            .expect("schema declares properties")
            .get("observers")
            .and_then(Json::as_object)
            .expect("schema declares observers")
            .get("items")
            .and_then(Json::as_object)
            .expect("observers is an array schema with items");
        let item_properties = observers_items
            .get("properties")
            .and_then(Json::as_object)
            .expect("observers items declare properties");
        assert!(
            !item_properties.contains_key("fragment"),
            "observers[] gained a `fragment` member; this module's \"never unsupported\" \
             reasoning depends on its absence and must be revisited"
        );
    }
}
