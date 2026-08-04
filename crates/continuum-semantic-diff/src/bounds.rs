//! `bounds` field classification: RFC 0031's `bounds` rule (PR-12 / IMPL-04).
//!
//! # What this module answers
//!
//! Given two [`Bounds`] tuples — an Intent Contract's `bounds` before and after a
//! proposed revision — classify the change into RFC 0031's closed relation vocabulary:
//!
//! > ### `bounds`
//! >
//! > The tuple is `(values, nodes, faults, depth)` over the schema's types: `nodes` and
//! > `faults` are integers; `values` and `depth` are integers or `null`. `null` denotes
//! > *unbounded* and is the top of that component's order, so `n → null` contributes an
//! > increase and `null → n` contributes a decrease. An absent `values` or `depth` key
//! > MUST be read as `null`. An absent `nodes` or `faults` key has no null form and no
//! > schema default; a change in the declaredness of either MUST classify `bounds` as
//! > `unknown` and fail closed. A decrease in any component with no increase elsewhere
//! > is `contracted`; an increase in any component with no decrease elsewhere is
//! > `expanded`; mixed movement is `incomparable`.
//! >
//! > — `notes/plan/rfcs/0031-semantic-and-intent-diff.md`, "`bounds`"
//!
//! which restates, for this field, the "Classification lattice" section's summary —
//!
//! > **Bounds:** componentwise partial order on `(values, nodes, faults, depth)`; a
//! > decrease in any component with no increase elsewhere is `contracted`; mixed
//! > changes are `incomparable`. `no-decrease` blocks `contracted` and blocks
//! > `incomparable` pending review.
//! >
//! > — RFC 0031, "Classification lattice"
//!
//! — and the field's own naming correction: "Plan §5.3's 'bound increase/decrease' […]
//! [is] informal. Normative: bounds classify `expanded`/`contracted`" (RFC 0031
//! correction 2). There is no `shrunk`, `strengthened`, or `weakened` token for this
//! field; the closed relation set spells the two directions `expanded` and `contracted`
//! and nothing else.
//!
//! # Why this is a named attack, not an edge case
//!
//! > | Gaming move (plan §5.1) | Field | Relation | Blocking verb (plan §5.4) |
//! > |---|---|---|---|
//! > | reduce node count from five to three | `bounds` | `contracted` | `no-decrease` |
//! >
//! > — RFC 0031, "Completeness guarantee"
//!
//! Shrinking a bound is exactly RFC 0037's phrase for it — "shrinking a bound is a
//! WEAKENING per RFC 0031's classification" (`continuum_intent::bounds`'s own module
//! doc, "The comparison RFC 0031 needs") — because a smaller exploration envelope lets
//! less-explored behavior pass unverified while the claim's text does not move at all.
//! `bounds` is one of RFC 0031's fifteen protected fields and INV-001/INV-011's
//! "weakening-is-privileged" posture applies to it exactly as it applies to a coarsened
//! observer or a weakened property.
//!
//! # One unit, not a keyed set
//!
//! Unlike `observers`, `properties`, or `assumptions`, `bounds` has no per-item unit
//! key. RFC 0031's field table gives its "Unit of classification" as "the tuple
//! `(values, nodes, faults, depth)`" — one unit, the whole field — and
//! `semantic-diff.schema.json` states the same fact structurally:
//!
//! > The eleven fields listed here are exactly those whose RFC 0031 unit-of-
//! > classification table declares a per-unit key; the four omitted fields (`bounds`,
//! > `trust_boundaries`, `completion_policy`, `assurance`) are classified as a whole and
//! > have no sub-unit to name.
//! >
//! > — `notes/plan/schemas/semantic-diff.schema.json`, `unit` member `$comment`
//!
//! [`classify_bounds`] therefore returns exactly one [`BoundsChange`], never zero and
//! never many — there is no added/removed membership case for this field, because the
//! contract's `bounds` key is REQUIRED (RFC 0037's field table: `bounds` is `object (MAY
//! be empty)`, required `yes`), so both sides always carry a tuple to compare.
//!
//! # The declaredness rule fails closed unconditionally
//!
//! `nodes` and `faults` have no `null` spelling and no schema default; leaving either
//! key out is a declaration that it is *undeclared*, not a declaration that it is zero
//! or unbounded. RFC 0037 states the same rule from the contract-shape side, confirming
//! it is not particular to the diff engine:
//!
//! > In `bounds`, `null` denotes **unbounded** and is the top of that component's
//! > order. An absent `values` or `depth` key MUST be read as `null`. `nodes` and
//! > `faults` have no null form and no default: a change in the declaredness of either
//! > MUST classify `bounds` as `unknown` and fail closed.
//! >
//! > — `notes/plan/rfcs/0037-intent-contract.md`, "Absence, null, and defaults"
//!
//! (RFC 0037 correction 15 extends the same "absence reads as `null`" equivalence to
//! `scope.abstraction_level`, citing `bounds.values`/`bounds.depth` as the
//! already-settled precedent — this module relies on that settled reading, not on
//! anything the correction itself changed.)
//!
//! A declaredness change on `nodes` or `faults` classifies `unknown` **even when every
//! other component grew** — RFC 0031 "does not let an increase elsewhere excuse a
//! declaredness change" (`continuum_intent::bounds`, `Bounds::relation_to`'s own
//! comment). This module does not recompute that rule: [`continuum_intent::bounds::
//! Bounds::relation_to`] already implements it exactly (see
//! `continuum_intent::bounds::tests::a_declaredness_change_is_unknown_even_when_every_
//! other_component_grows`, which pins it there), and [`classify_bounds`] is a total,
//! checked projection of that result onto [`Relation`] — the ONE classification
//! vocabulary this crate closes over (`change_policy::Relation`; see this crate's
//! `observers` module for the identical discipline). No second relation enum is
//! introduced here.
//!
//! # Why the mixed case is `incomparable`, and nothing else
//!
//! Both RFC 0031 quotes above state the componentwise rule without exception: growth in
//! one component and shrinkage in another — whether or not they are the same
//! component — classifies `incomparable`, never `expanded` and never `contracted`.
//! `Incomparable` is non-affirmative, so `PolicyTable::verdict`'s P2 makes it contribute
//! at least `review` on every field under every verb, including `unlocked`
//! (`incomparable_blocks_even_under_the_least_restrictive_verb`, below) — the same
//! safety margin `bn-1sdp`'s `observers` module documents for its own mixed case.
//!
//! # What never arises here, and why that is not evasion
//!
//! [`PolicyField::Bounds`]'s admissible-relation set is `{unchanged, expanded,
//! contracted, incomparable, unknown}` plus `unsupported` — the last one admissible
//! only through the fail-closed rule's blanket grant ("the non-affirmative three are
//! admissible on every row", RFC 0031 correction 13), not through the field table's own
//! row (RFC 0031's field-by-field table lists `unchanged`, `expanded`, `contracted`,
//! `incomparable`, `unknown` for `bounds` — five tokens, `unsupported` absent). Neither
//! `unsupported` nor a bare boolean "changed" is ever returned by [`classify_bounds`].
//! `unsupported` is RFC 0031's answer when a unit's *fragment* is not declared in
//! `scope.fragments` ("Fragment attribution"; "Removing a fragment... those units then
//! classify `unsupported`") — a rule stated for expressions, which carry a `fragment`
//! member (`notes/plan/schemas/intent-contract.schema.json`'s `property_expression`
//! `$defs` entry). `bounds` carries no `fragment` member at all — confirmed in this
//! module's tests against the live schema — so there is no fragment for a `bounds`
//! comparison to fall outside of, exactly the reasoning `bn-1sdp`'s `observers` module
//! gives for the same absence. A total classifier that never emits `unsupported` is the
//! honest answer for a field with nothing to be unsupported *by*, not a shortcut around
//! the fail-closed rule.
//!
//! # Scope: PR-12 / IMPL-04 only
//!
//! PR 12 (`notes/START_HERE_IMPLEMENTATION.md`) lists six classification bullets:
//! exact equality, property AST edit, assumption add/remove, **bound change**, observer
//! event change, and fault/fairness/assurance change. This bone (`bn-ycn6`) delivers
//! the fourth, and only the fourth. The other five are separate open or landed bones —
//! `bn-b8ru` (IMPL-01, exact equality), `bn-8mlg` (IMPL-02, property AST edit),
//! `bn-7vg7` (IMPL-03, assumption add/remove), `bn-1sdp` (IMPL-05, observer event
//! change, landed as [`crate::observers`]), `bn-3vxp` (IMPL-06, fault/fairness/assurance
//! change) — and land as their own reviewable units, per this crate's PR-1 scaffold
//! note ("the types and behavior land in the PR named above", now specialized per
//! bullet). Also out of scope, honestly: the wire `intent_changes[]` JSON shape
//! (`evidence`, `fragment` as a *serialized* member, `protected`), the whole-artifact
//! assembly (`schema_id`, `before_snapshot`, `policy.decision`), the `no-decrease` verb
//! enforcement (that is `continuum_intent::change_policy::PolicyTable::verdict`'s job,
//! already landed — this module only produces the record `verdict` consumes), and the
//! field-level P1 completeness a `verdict` call needs even when the field did not
//! change (a field-level "at least one `unchanged` record" the caller supplies) — all of
//! this belongs to whatever later bone assembles a full `diff_*` artifact across all
//! fifteen fields, not to one field's classifier.

use continuum_intent::bounds::{Bounds, BoundsRelation};
use continuum_intent::change_policy::{
    ChangePolicyError, ClassificationRecord, PolicyField, Relation,
};

/// The classified `bounds` field: the [`Relation`] RFC 0031 assigns to one `(values,
/// nodes, faults, depth)` comparison.
///
/// No unit locator — unlike [`crate::observers::ObserverChange`], `bounds` has no
/// per-item key to carry (see the module doc, "One unit, not a keyed set"), so this
/// type is a thin, checked wrapper over [`Relation`] and nothing more.
///
/// The relation is always one [`PolicyField::Bounds`] admits — pinned by this module's
/// tests, not merely asserted — because the only relations [`classify_bounds`] can ever
/// produce are the five [`to_relation`] maps [`BoundsRelation`]'s five variants onto,
/// all five of which are on the bounds row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundsChange {
    relation: Relation,
}

impl BoundsChange {
    fn new(relation: Relation) -> Self {
        debug_assert!(
            PolicyField::Bounds.admits_relation(relation),
            "a bounds classification produced {relation}, which the bounds row does not admit"
        );
        Self { relation }
    }

    /// The RFC 0031 relation this comparison classified to.
    #[must_use]
    pub const fn relation(&self) -> Relation {
        self.relation
    }

    /// This change as a field-level [`ClassificationRecord`], for
    /// [`continuum_intent::change_policy::PolicyTable::verdict`].
    ///
    /// # Errors
    ///
    /// [`ChangePolicyError::InadmissibleRelation`] — never in practice, since every
    /// relation this module produces is on the bounds row (see the struct doc and
    /// `every_relation_this_module_can_produce_is_admissible_on_the_bounds_field`), but
    /// [`ClassificationRecord::new`] is itself fallible and a `debug_assert!` is not a
    /// proof, so the `Result` is threaded rather than unwrapped.
    pub fn to_classification_record(&self) -> Result<ClassificationRecord, ChangePolicyError> {
        ClassificationRecord::new(PolicyField::Bounds, self.relation)
    }
}

/// Classify `bounds` between `before` and `after`, per RFC 0031's "bounds" rule.
///
/// Total and single-valued (see the module doc, "One unit, not a keyed set"): both
/// sides always carry a `bounds` tuple, so this always returns exactly one
/// [`BoundsChange`] — including [`Relation::Unchanged`]. RFC 0031 permits a wire
/// artifact to omit `unchanged` records ("Field-by-field classification rules":
/// "Records whose relation is `unchanged` MAY be omitted"), but that is an emission
/// choice for whatever assembles the wire `intent_changes[]` array, not a property of
/// the classifier — matching [`crate::observers::classify_observers`]'s identical
/// choice for the same reason.
///
/// The componentwise comparison itself — the order over `(values, nodes, faults,
/// depth)`, the `null`-is-unbounded rule, and the declaredness fail-closed rule — is
/// [`Bounds::relation_to`]'s, already landed in `continuum-intent` and already tested
/// there against RFC 0031's and RFC 0037's text (see `continuum_intent::bounds`'s
/// module doc). This function does not recompute that logic — duplicating it here would
/// be a second implementation of one RFC rule, exactly the hazard the "ONE vocabulary"
/// discipline exists to avoid — it only projects [`BoundsRelation`], `continuum-intent`'s
/// own typed answer, onto [`Relation`], the wire vocabulary every diff field is
/// classified into.
#[must_use]
pub fn classify_bounds(before: &Bounds, after: &Bounds) -> BoundsChange {
    BoundsChange::new(to_relation(before.relation_to(after)))
}

/// The projection from [`BoundsRelation`] — `continuum-intent`'s field-local typed
/// order — onto [`Relation`] — `change_policy`'s closed sixteen-token wire vocabulary.
///
/// A total, 1:1 map on names: [`BoundsRelation`]'s five variants are named after, and
/// restricted to, exactly the five [`Relation`] tokens RFC 0031's field table assigns
/// to `bounds` (RFC 0031 correction 2 fixes the two directional names as `expanded`/
/// `contracted`, never `strengthened`/`weakened`/`shrunk`). Nothing is invented on
/// either side of the map; a match with no wildcard arm is what makes that a checked
/// fact of the code rather than an assertion in this comment.
const fn to_relation(relation: BoundsRelation) -> Relation {
    match relation {
        BoundsRelation::Unchanged => Relation::Unchanged,
        BoundsRelation::Expanded => Relation::Expanded,
        BoundsRelation::Contracted => Relation::Contracted,
        BoundsRelation::Incomparable => Relation::Incomparable,
        BoundsRelation::Unknown => Relation::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use continuum_intent::bounds::{DeclaredBound, ExplorationBound};
    use continuum_intent::canonical_json::Json;

    use super::*;

    /// The dossier's validated Intent Contract example, included at compile time — the
    /// same fixture `continuum-intent`'s own PR-4 exit and INV-012 evidence use, and
    /// the same one `crate::observers`'s tests read — used here to confirm the live
    /// schema still carries no `fragment` member on `bounds` (the module doc's "What
    /// never arises here" claim).
    const SCHEMA: &str = include_str!("../../../notes/plan/schemas/intent-contract.schema.json");

    fn bounds(
        values: ExplorationBound,
        nodes: DeclaredBound,
        faults: DeclaredBound,
        depth: ExplorationBound,
    ) -> Bounds {
        Bounds::new(values, nodes, faults, depth).expect("test bounds are within the minimums")
    }

    // --- the four componentwise outcomes plus declaredness-unknown ---------------------------

    #[test]
    fn equal_bounds_classify_unchanged() {
        let a = bounds(
            ExplorationBound::Bounded(2),
            DeclaredBound::Declared(3),
            DeclaredBound::Declared(1),
            ExplorationBound::Unbounded,
        );
        let b = bounds(
            ExplorationBound::Bounded(2),
            DeclaredBound::Declared(3),
            DeclaredBound::Declared(1),
            ExplorationBound::Unbounded,
        );
        let change = classify_bounds(&a, &b);
        assert_eq!(change.relation(), Relation::Unchanged);
    }

    #[test]
    fn growing_one_component_and_holding_the_rest_classifies_expanded() {
        // RFC 0031: "an increase in any component with no decrease elsewhere is
        // `expanded`" — the other three may be equal.
        let before = bounds(
            ExplorationBound::Bounded(2),
            DeclaredBound::Declared(3),
            DeclaredBound::Undeclared,
            ExplorationBound::Unbounded,
        );
        let after = bounds(
            ExplorationBound::Bounded(2),
            DeclaredBound::Declared(4),
            DeclaredBound::Undeclared,
            ExplorationBound::Unbounded,
        );
        assert_eq!(
            classify_bounds(&before, &after).relation(),
            Relation::Expanded
        );
    }

    #[test]
    fn reducing_node_count_from_five_to_three_classifies_contracted() {
        // RFC 0031's own gaming-move example, verbatim: "reduce node count from five
        // to three" classifies `bounds` `contracted`, blocked by `no-decrease`.
        let before = bounds(
            ExplorationBound::Unbounded,
            DeclaredBound::Declared(5),
            DeclaredBound::Undeclared,
            ExplorationBound::Unbounded,
        );
        let after = bounds(
            ExplorationBound::Unbounded,
            DeclaredBound::Declared(3),
            DeclaredBound::Undeclared,
            ExplorationBound::Unbounded,
        );
        assert_eq!(
            classify_bounds(&before, &after).relation(),
            Relation::Contracted
        );
        // And the reverse direction is the safe one, `expanded`.
        assert_eq!(
            classify_bounds(&after, &before).relation(),
            Relation::Expanded
        );
    }

    #[test]
    fn going_unbounded_is_an_expansion_and_the_reverse_is_a_contraction() {
        let bounded = bounds(
            ExplorationBound::Bounded(4),
            DeclaredBound::Undeclared,
            DeclaredBound::Undeclared,
            ExplorationBound::Unbounded,
        );
        let unbounded = bounds(
            ExplorationBound::Unbounded,
            DeclaredBound::Undeclared,
            DeclaredBound::Undeclared,
            ExplorationBound::Unbounded,
        );
        assert_eq!(
            classify_bounds(&bounded, &unbounded).relation(),
            Relation::Expanded
        );
        assert_eq!(
            classify_bounds(&unbounded, &bounded).relation(),
            Relation::Contracted
        );
    }

    #[test]
    fn one_component_growing_while_another_shrinks_is_incomparable() {
        let before = bounds(
            ExplorationBound::Bounded(2),
            DeclaredBound::Declared(5),
            DeclaredBound::Undeclared,
            ExplorationBound::Unbounded,
        );
        // `values` grows, `nodes` shrinks: no single direction.
        let after = bounds(
            ExplorationBound::Bounded(3),
            DeclaredBound::Declared(4),
            DeclaredBound::Undeclared,
            ExplorationBound::Unbounded,
        );
        assert_eq!(
            classify_bounds(&before, &after).relation(),
            Relation::Incomparable
        );
    }

    #[test]
    fn a_declaredness_change_on_faults_is_unknown_even_when_every_other_component_grows() {
        // The fail-closed case the module doc quotes: no increase elsewhere may
        // excuse a declaredness change on `nodes`/`faults`.
        let before = bounds(
            ExplorationBound::Bounded(2),
            DeclaredBound::Undeclared,
            DeclaredBound::Declared(1),
            ExplorationBound::Bounded(10),
        );
        let after = bounds(
            ExplorationBound::Unbounded,
            DeclaredBound::Undeclared,
            DeclaredBound::Undeclared,
            ExplorationBound::Unbounded,
        );
        assert_eq!(
            classify_bounds(&before, &after).relation(),
            Relation::Unknown
        );
    }

    #[test]
    fn a_declaredness_change_on_nodes_is_also_unknown() {
        let before = bounds(
            ExplorationBound::Unbounded,
            DeclaredBound::Declared(5),
            DeclaredBound::Undeclared,
            ExplorationBound::Unbounded,
        );
        let after = bounds(
            ExplorationBound::Unbounded,
            DeclaredBound::Undeclared,
            DeclaredBound::Undeclared,
            ExplorationBound::Unbounded,
        );
        assert_eq!(
            classify_bounds(&before, &after).relation(),
            Relation::Unknown
        );
    }

    // --- boundary --------------------------------------------------------------------------

    #[test]
    fn two_fully_empty_bounds_objects_classify_unchanged() {
        // `bounds` MAY be the empty object — every component reads as
        // unbounded/undeclared — which is itself a declaration, not an absence.
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

    // --- the field-classification contract --------------------------------------------------

    #[test]
    fn every_relation_this_module_can_produce_is_admissible_on_the_bounds_field() {
        for relation in [
            Relation::Unchanged,
            Relation::Expanded,
            Relation::Contracted,
            Relation::Incomparable,
            Relation::Unknown,
        ] {
            assert!(
                PolicyField::Bounds.admits_relation(relation),
                "{relation} must be admissible on `bounds`"
            );
        }
    }

    #[test]
    fn to_classification_record_never_fails_for_a_relation_this_module_produced() {
        let before = bounds(
            ExplorationBound::Unbounded,
            DeclaredBound::Declared(5),
            DeclaredBound::Undeclared,
            ExplorationBound::Unbounded,
        );
        let after = bounds(
            ExplorationBound::Unbounded,
            DeclaredBound::Declared(3),
            DeclaredBound::Undeclared,
            ExplorationBound::Unbounded,
        );
        let change = classify_bounds(&before, &after);
        let record = change
            .to_classification_record()
            .expect("this module's own relations are always admissible on `bounds`");
        assert_eq!(record.field(), PolicyField::Bounds);
        assert_eq!(record.relation(), change.relation());
    }

    #[test]
    fn the_live_schema_carries_no_fragment_member_on_bounds() {
        // Backs the module doc's "What never arises here" claim that `unsupported`
        // cannot arise from fragment narrowing for this field: there is no fragment
        // to narrow.
        let schema = Json::parse(SCHEMA.as_bytes()).expect("the schema itself is canonical JSON");
        let bounds_properties = schema
            .as_object()
            .expect("schema root is an object")
            .get("properties")
            .and_then(Json::as_object)
            .expect("schema declares properties")
            .get("bounds")
            .and_then(Json::as_object)
            .expect("schema declares bounds")
            .get("properties")
            .and_then(Json::as_object)
            .expect("bounds is an object schema with properties");
        assert!(
            !bounds_properties.contains_key("fragment"),
            "bounds gained a `fragment` member; this module's \"never unsupported\" \
             reasoning depends on its absence and must be revisited"
        );
        for expected in ["depth", "faults", "nodes", "values"] {
            assert!(
                bounds_properties.contains_key(expected),
                "bounds is missing its documented component {expected:?}"
            );
        }
    }
}
