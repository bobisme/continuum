//! `faults` field classification: RFC 0031's `faults` rule (PR-12 / IMPL-06).
//!
//! # What this module answers
//!
//! Given two [`FaultModel`]s — an Intent Contract's `fault_model` before and after a
//! proposed revision — classify every member of `enabled` or `profiles` into RFC
//! 0031's closed relation vocabulary:
//!
//! > | `fault_model` | `{enabled, profiles?}` | `enabled` | `enabled` ⊆ {`crash`,
//! > `recovery`, `partition`, `loss`, `duplication`, `delay`}; `profiles` are
//! > domain-pack profile names (RFC 0002) |
//! >
//! > — `notes/plan/rfcs/0037-intent-contract.md`, "Field types and enums"
//!
//! > ### `faults` (contract `fault_model`)
//! >
//! > `enabled` is a closed set of six classes (`crash`, `recovery`, `partition`,
//! > `loss`, `duplication`, `delay`) and `profiles` is a set of pack profile names.
//! > Both are classified by membership: one record per class or profile added or
//! > removed. `removed` is the "removing crash-after-submit" attack and is what
//! > `no-removal` blocks. Plan §5.3's "fault envelope expansion/contraction" is the
//! > informal alias for a set of `added`/`removed` records.
//! >
//! > — `notes/plan/rfcs/0031-semantic-and-intent-diff.md`, "`faults`"
//!
//! So a fault-model change is **pure set membership** on two closed-ish vocabularies:
//! no componentwise movement (unlike `bounds` or `observers`), no antitone dual
//! (unlike `fairness`'s `condition`), and no content to compare once a unit's
//! presence agrees on both sides — a fault class or a profile name is an opaque
//! token, and RFC 0031 gives this field exactly three admissible directional-ish
//! outcomes: `unchanged`, `added`, `removed`.
//!
//! # Why this is a named attack, not an edge case
//!
//! > | Gaming move (plan §5.1) | Field | Relation | Blocking verb (plan §5.4) |
//! > |---|---|---|---|
//! > | remove crash-after-submit | `faults` | `removed` | `no-removal` |
//! >
//! > — RFC 0031, "Completeness guarantee"
//!
//! "Remove faults" is on `notes/plan/docs/50_AGENT_EVALUATION_AND_REWARD_HACKING.md`'s
//! intent-attack list. Removing a fault class does not touch a claim's text at all —
//! the property still reads the same — it just shrinks the adversary the claim was
//! ever checked against, which is exactly the kind of quiet weakening
//! INV-001/INV-011's "weakening-is-privileged" posture exists to catch.
//!
//! # One unit, no sub-content — the whole story is membership
//!
//! [`continuum_intent::faults::FaultModel::units`] is the surface this module
//! compares: it enumerates every [`FaultUnit`] (a [`FaultClass`](continuum_intent::faults::FaultClass)
//! or a [`ProfileName`](continuum_intent::faults::ProfileName)), classes first, each
//! set in token order. A unit is either present or absent on a side; there is no
//! third state and nothing inside a unit that could itself move, so
//! [`classify_faults`] never needs anything beyond `FaultModel::contains` — unlike
//! [`crate::observers::classify_pair`] or this bone's `fairness::formula_relation`,
//! there is no componentwise sub-comparison to get wrong here.
//!
//! # AO4: a `no-removal` lock over an empty model is dormant, not ill-formed — and
//! that is not this module's concern either way
//!
//! > ... the identical dormant shape the corpus's own die-hard contract carries
//! > twice, on `faults` over an empty `fault_model.enabled` and on `optimization`
//! > over empty `hard`/`soft`.
//! >
//! > — RFC 0037 correction 17 (AO4)
//!
//! `tests/fixtures/die-hard-contract.json` (`continuum-intent`) declares
//! `fault_model: {enabled: [], profiles: []}` under `policy.faults = "no-removal"`.
//! RFC 0037 correction 17 settles that this is a legitimate, dormant policy state —
//! "a lock over nothing" that "has nothing to guard yet" — not a well-formedness
//! defect. [`classify_faults`] needs no special case to agree: comparing two empty
//! `FaultModel`s visits the empty union of units and returns an empty `Vec`, which is
//! the same answer this classifier gives for any two models with identical content.
//! **This module classifies *revisions*, not the emptiness of either side** — it has
//! no opinion on whether a fault model "ought" to be non-empty, and manufacturing one
//! would be answering a question (is this policy dormant, or a mistake?) that belongs
//! to whoever authors the contract, not to a classifier that only compares two of
//! them.
//!
//! # What never arises here, and why that is not evasion
//!
//! [`PolicyField::Faults`]'s admissible-relation row lists `unknown`, `unsupported`,
//! and `incomparable` as always admissible (the fail-closed rule, RFC 0031 correction
//! 13), but [`classify_faults`] never returns any of the three. Set membership over
//! `enabled`/`profiles` is decidable outright by `FaultModel::contains` — no
//! `requested_assurance` budget, no solver, nothing to be `unknown` about — and
//! `fault_model` carries no `fragment` member in
//! `notes/plan/schemas/intent-contract.schema.json` (confirmed in this module's tests
//! against the live schema), so no unit has a fragment to fall outside of, and
//! `incomparable` has no componentwise axis to disagree along in the first place.
//! Exactly the reasoning `bn-1sdp`'s `observers` module gives for its own field.
//!
//! # Scope: PR-12 / IMPL-06 only
//!
//! PR 12 (`notes/START_HERE_IMPLEMENTATION.md`) lists six classification bullets:
//! exact equality, property AST edit, assumption add/remove, bound change, observer
//! event change, and **fault/fairness/assurance change**. This bone (`bn-3vxp`)
//! delivers the sixth, alongside [`crate::fairness`] and [`crate::assurance`]. The
//! other four are separate bones — `bn-b8ru` (IMPL-01, exact equality), `bn-8mlg`
//! (IMPL-02, property AST edit), `bn-7vg7` (IMPL-03, assumption add/remove), `bn-ycn6`
//! (IMPL-04, bound change) — landing as their own reviewable units. Also out of
//! scope, honestly: the wire `intent_changes[]` JSON shape, the whole-artifact
//! assembly, and the field-level P1 completeness a
//! `continuum_intent::change_policy::PolicyTable::verdict` call needs even when zero
//! units changed — all of it belongs to whatever later bone assembles a full `diff_*`
//! artifact across all fifteen fields, not to one field's classifier.

use std::collections::BTreeSet;

use continuum_intent::change_policy::{
    ChangePolicyError, ClassificationRecord, PolicyField, Relation,
};
use continuum_intent::faults::{FaultModel, FaultUnit};

/// One classified `fault_model` unit: a [`FaultUnit`] (a class of `enabled` or a
/// member of `profiles`) and the [`Relation`] RFC 0031 assigns it.
///
/// The relation is always one [`PolicyField::Faults`] admits — pinned by this
/// module's tests, not merely asserted — because [`classify_faults`] only ever
/// produces [`Relation::Unchanged`], [`Relation::Added`], or [`Relation::Removed`],
/// all three of which are on the faults row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaultChange {
    unit: FaultUnit,
    relation: Relation,
}

impl FaultChange {
    fn new(unit: FaultUnit, relation: Relation) -> Self {
        debug_assert!(
            PolicyField::Faults.admits_relation(relation),
            "a fault classification produced {relation}, which the faults row does not admit"
        );
        Self { unit, relation }
    }

    /// The classified unit: RFC 0031's per-unit locator for `faults`, "the member of
    /// `enabled` or `profiles`".
    #[must_use]
    pub const fn unit(&self) -> &FaultUnit {
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
    /// # Errors
    ///
    /// [`ChangePolicyError::InadmissibleRelation`] — never in practice, since every
    /// relation this module produces is on the faults row (see the struct doc), but
    /// [`ClassificationRecord::new`] is itself fallible and a `debug_assert!` is not
    /// a proof, so the `Result` is threaded rather than unwrapped.
    pub fn to_classification_record(&self) -> Result<ClassificationRecord, ChangePolicyError> {
        ClassificationRecord::new(PolicyField::Faults, self.relation)
    }
}

/// Classify every `fault_model` unit between `before` and `after`, per RFC 0031's
/// "`faults`" rule.
///
/// Total: one [`FaultChange`] for every unit named in `before`, in `after`, or both —
/// including [`Relation::Unchanged`] units. RFC 0031 permits a wire artifact to omit
/// `unchanged` records ("Field-by-field classification rules": "Records whose
/// relation is `unchanged` MAY be omitted"), but that is an emission choice for
/// whatever assembles the wire `intent_changes[]` array, not a property of the
/// classifier — matching [`crate::observers::classify_observers`]'s identical choice.
///
/// Units are visited in [`FaultUnit`] order (classes before profiles, each set in
/// token order — `FaultModel::units`'s own order), so the output order is
/// deterministic and does not depend on either input's authored array order,
/// matching RFC 0031's "Determinism": "the diff artifact MUST be byte-identical
/// across platforms and releases".
#[must_use]
pub fn classify_faults(before: &FaultModel, after: &FaultModel) -> Vec<FaultChange> {
    let mut units: BTreeSet<FaultUnit> = before.units().collect();
    units.extend(after.units());
    units
        .into_iter()
        .map(|unit| {
            let relation = match (before.contains(&unit), after.contains(&unit)) {
                (true, true) => Relation::Unchanged,
                (false, true) => Relation::Added,
                (true, false) => Relation::Removed,
                (false, false) => {
                    unreachable!("`unit` came from the union of both sides' own `units()`")
                }
            };
            FaultChange::new(unit, relation)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use continuum_intent::canonical_json::Json;
    use continuum_intent::faults::{FaultClass, ProfileName};

    use super::*;

    /// The dossier's validated Intent Contract example, included at compile time —
    /// used here only to confirm the live schema still carries no `fragment` member
    /// on `fault_model` (the module doc's "What never arises here" claim).
    const SCHEMA: &str = include_str!("../../../notes/plan/schemas/intent-contract.schema.json");

    fn profile(name: &str) -> ProfileName {
        ProfileName::new(name).expect("a test profile name is non-empty")
    }

    fn model(classes: &[FaultClass], profiles: &[&str]) -> FaultModel {
        FaultModel::new(
            classes.iter().copied(),
            profiles.iter().map(|name| profile(name)),
        )
        .expect("test fault models are well formed")
    }

    fn relation_of(changes: &[FaultChange], unit: &FaultUnit) -> Relation {
        changes
            .iter()
            .find(|change| change.unit() == unit)
            .unwrap_or_else(|| panic!("no record for unit {unit:?} in {changes:?}"))
            .relation()
    }

    // --- membership: the whole story ----------------------------------------------------------

    #[test]
    fn identical_models_classify_every_unit_unchanged() {
        let a = model(&[FaultClass::Crash, FaultClass::Recovery], &["p"]);
        let b = model(&[FaultClass::Crash, FaultClass::Recovery], &["p"]);
        let changes = classify_faults(&a, &b);
        assert_eq!(changes.len(), 3);
        assert!(changes.iter().all(|c| c.relation() == Relation::Unchanged));
    }

    #[test]
    fn a_class_present_only_after_classifies_added() {
        let before = model(&[], &[]);
        let after = model(&[FaultClass::Crash], &[]);
        let changes = classify_faults(&before, &after);
        assert_eq!(changes.len(), 1);
        assert_eq!(
            relation_of(&changes, &FaultUnit::Class(FaultClass::Crash)),
            Relation::Added
        );
    }

    #[test]
    fn a_class_present_only_before_classifies_removed() {
        // RFC 0031's own gaming-move example, in miniature: `crash` disappears.
        let before = model(&[FaultClass::Crash], &[]);
        let after = model(&[], &[]);
        let changes = classify_faults(&before, &after);
        assert_eq!(changes.len(), 1);
        assert_eq!(
            relation_of(&changes, &FaultUnit::Class(FaultClass::Crash)),
            Relation::Removed
        );
    }

    #[test]
    fn a_profile_present_only_after_classifies_added() {
        let before = model(&[], &[]);
        let after = model(&[], &["storage/append-log-v0"]);
        let changes = classify_faults(&before, &after);
        assert_eq!(changes.len(), 1);
        assert_eq!(
            relation_of(
                &changes,
                &FaultUnit::Profile(profile("storage/append-log-v0"))
            ),
            Relation::Added
        );
    }

    #[test]
    fn a_profile_present_only_before_classifies_removed() {
        let before = model(&[], &["storage/append-log-v0"]);
        let after = model(&[], &[]);
        let changes = classify_faults(&before, &after);
        assert_eq!(changes.len(), 1);
        assert_eq!(
            relation_of(
                &changes,
                &FaultUnit::Profile(profile("storage/append-log-v0"))
            ),
            Relation::Removed
        );
    }

    #[test]
    fn negative_swapping_one_class_for_another_is_never_silent() {
        // "Remove crash, add recovery" nets the same *count* but is a genuine
        // weakening (crash-after-submit is no longer modeled at all) disguised as a
        // lateral move. Both records must appear; neither may cancel the other out.
        let before = model(&[FaultClass::Crash], &[]);
        let after = model(&[FaultClass::Recovery], &[]);
        let changes = classify_faults(&before, &after);
        assert_eq!(changes.len(), 2, "{changes:?}");
        assert_eq!(
            relation_of(&changes, &FaultUnit::Class(FaultClass::Crash)),
            Relation::Removed
        );
        assert_eq!(
            relation_of(&changes, &FaultUnit::Class(FaultClass::Recovery)),
            Relation::Added
        );
    }

    // --- AO4: emptiness is not this module's concern -------------------------------------------

    #[test]
    fn two_empty_models_classify_no_units_at_all() {
        // The die-hard fixture's shape: `no-removal` over an empty `enabled` has
        // nothing to guard, and this classifier agrees by construction — it visits
        // the empty union and returns nothing, not a judgment about emptiness.
        let empty = model(&[], &[]);
        assert!(classify_faults(&empty, &empty).is_empty());
    }

    // --- determinism -----------------------------------------------------------------------

    #[test]
    fn output_order_is_class_then_profile_in_token_order_regardless_of_authored_order() {
        let before = model(&[FaultClass::Recovery, FaultClass::Crash], &["zzz", "aaa"]);
        let after = model(&[FaultClass::Recovery, FaultClass::Crash], &["zzz", "aaa"]);
        let changes = classify_faults(&before, &after);
        let locators: Vec<&str> = changes.iter().map(|c| c.unit().locator()).collect();
        assert_eq!(locators, vec!["crash", "recovery", "aaa", "zzz"]);
    }

    // --- the field-classification contract --------------------------------------------------

    #[test]
    fn every_relation_this_module_can_produce_is_admissible_on_the_faults_field() {
        for relation in [Relation::Unchanged, Relation::Added, Relation::Removed] {
            assert!(
                PolicyField::Faults.admits_relation(relation),
                "{relation} must be admissible on `faults`"
            );
        }
    }

    #[test]
    fn to_classification_record_never_fails_for_a_relation_this_module_produced() {
        let before = model(&[FaultClass::Crash], &[]);
        let after = model(&[], &[]);
        for change in classify_faults(&before, &after) {
            let record = change
                .to_classification_record()
                .expect("this module's own relations are always admissible on `faults`");
            assert_eq!(record.field(), PolicyField::Faults);
            assert_eq!(record.relation(), change.relation());
        }
    }

    #[test]
    fn the_live_schema_carries_no_fragment_member_on_fault_model() {
        let schema = Json::parse(SCHEMA.as_bytes()).expect("the schema itself is canonical JSON");
        let fault_model_properties = schema
            .as_object()
            .expect("schema root is an object")
            .get("properties")
            .and_then(Json::as_object)
            .expect("schema declares properties")
            .get("fault_model")
            .and_then(Json::as_object)
            .expect("schema declares fault_model")
            .get("properties")
            .and_then(Json::as_object)
            .expect("fault_model is an object schema with properties");
        assert!(
            !fault_model_properties.contains_key("fragment"),
            "fault_model gained a `fragment` member; this module's \"never unsupported\" \
             reasoning depends on its absence and must be revisited"
        );
        for expected in ["enabled", "profiles"] {
            assert!(
                fault_model_properties.contains_key(expected),
                "fault_model is missing its documented component {expected:?}"
            );
        }
    }

    // --- anti-vacuity mutant: cardinality is not membership -------------------------------------

    /// A plausible, *wrong* classifier: compares only the total unit count on each
    /// side, so a same-size swap reads as `unchanged`. Shows the real classifier's
    /// positive assertions above are load-bearing, not vacuously true.
    fn mutant_by_cardinality(before: &FaultModel, after: &FaultModel) -> bool {
        let before_count = before.enabled().len() + before.profiles().len();
        let after_count = after.enabled().len() + after.profiles().len();
        before_count == after_count
    }

    #[test]
    fn negative_mutant_by_cardinality_would_wrongly_pass_the_swap_as_unchanged() {
        let before = model(&[FaultClass::Crash], &[]);
        let after = model(&[FaultClass::Recovery], &[]);

        let real_changes = classify_faults(&before, &after);
        assert!(
            real_changes
                .iter()
                .any(|c| c.relation() != Relation::Unchanged),
            "the real classifier sees `crash` removed and `recovery` added"
        );

        assert!(
            mutant_by_cardinality(&before, &after),
            "the mutant must actually get this wrong (report no change), or it is not \
             exercising the bug"
        );
    }
}
