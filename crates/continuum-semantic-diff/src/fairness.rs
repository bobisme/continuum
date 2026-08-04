//! `fairness` field classification: RFC 0031's `fairness` rule (PR-12 / IMPL-06).
//!
//! # What this module answers
//!
//! Given two [`FairnessSet`]s — an Intent Contract's `fairness[]` before and after a
//! proposed revision — classify every unit into RFC 0031's closed relation
//! vocabulary:
//!
//! > | `fairness[]` | `{kind, action, condition?}` | `kind`, `action` | `kind` ∈
//! > {`weak`, `strong`}; `condition` is a temporal-operator-free property AST or
//! > `null`; fairness strengthening is the canonical gaming vector (docs/50) |
//! >
//! > — `notes/plan/rfcs/0037-intent-contract.md`, "Field types and enums"
//!
//! > ### `fairness`
//! >
//! > Constraints are keyed by (`kind`, `action`); the schema gives fairness items no
//! > `id`. Three movements are classified:
//! >
//! > - Membership: a constraint present on one side only is `added` or `removed`.
//! >   Adding fairness is environment-strengthening and is the canonical gaming
//! >   vector (docs/50).
//! > - `kind`: `weak → strong` on the same action classifies `strengthened`, because
//! >   strong fairness implies weak fairness; `strong → weak` classifies `weakened`.
//! >   (A `kind` change is equivalently expressible as one `removed` plus one `added`
//! >   record; a classifier MUST choose one encoding and MUST NOT emit both.)
//! > - `condition`: the enabling predicate occupies an **antitone** position. A
//! >   fairness constraint applies wherever its condition holds, so weakening the
//! >   condition strengthens the constraint. Where `condition` is a state formula,
//! >   the relation on the *constraint* is the dual of the relation on the *formula*:
//! >   formula `weakened` ⇒ constraint `strengthened`, and formula `strengthened` ⇒
//! >   constraint `weakened`. `null` means unconditional and is the weakest
//! >   condition, hence the strongest constraint: `C → null` classifies
//! >   `strengthened`. Formula relations are computed over CPNF-1 exactly as for
//! >   claims, and `incomparable`/`unknown` pass through the duality unchanged.
//! >
//! > — `notes/plan/rfcs/0031-semantic-and-intent-diff.md`, "`fairness`"
//!
//! and RFC 0037's absence rule for the third movement's extremum:
//!
//! > `fairness[].condition: null` means unconditional, which is the weakest condition
//! > and therefore the strongest constraint (RFC 0031's antitone rule). An absent
//! > `condition` key MUST be read as `null`.
//! >
//! > — `notes/plan/rfcs/0037-intent-contract.md`, "Absence, null, and defaults"
//!
//! (RFC 0037 correction 15 confirms this equivalence was already settled text, citing
//! it beside `bounds.values`/`bounds.depth` as precedent for extending the same
//! "absence reads as `null`" reading to `scope.abstraction_level` — this module
//! relies on the settled reading, not on anything correction 15 itself changed.)
//!
//! # Why this is a named attack, not an edge case
//!
//! > | Gaming move (plan §5.1) | Field | Relation | Blocking verb (plan §5.4) |
//! > |---|---|---|---|
//! > | add a fairness assumption that schedules away the bug | `fairness` | `added` /
//! > `strengthened` | `review` |
//! >
//! > — RFC 0031, "Completeness guarantee"
//!
//! A stronger fairness constraint does not touch a claim's text; it deletes the
//! *behaviors* in which the property could fail, so the same claim passes over a
//! smaller world. `fairness` is one of RFC 0031's fifteen protected fields, and
//! INV-001/INV-011's "weakening-is-privileged" posture applies to environment
//! strengthening exactly as it applies to a weakened property, dualized: here the
//! *dangerous* direction is the constraint growing stronger (`added`, `kind`
//! `strengthened`, `condition` narrowing toward unconditional), not shrinking.
//!
//! # Three movements, three mechanisms
//!
//! - **Membership** — [`classify_fairness`]'s base pass: the union of both sides'
//!   `unit_keys()` (the (`kind`, `action`) pair, W1's uniqueness key), with a unit on
//!   one side only classifying [`Relation::Added`]/[`Relation::Removed`].
//! - **`condition`** — [`formula_relation`] plus [`dualize`], applied to every key
//!   present on *both* sides (the key, and therefore `kind`, is fixed in this case,
//!   so the only thing that can have moved is the condition).
//! - **`kind`** — [`classify_fairness`]'s second pass, over the leftover
//!   before-only/after-only units: when one side has exactly `weak:A` and the other
//!   has exactly `strong:A` for the *same* action `A`, with an **identical**
//!   condition on both sides, the pair collapses into one
//!   [`FairnessUnit::KindChange`] record rather than a `removed`+`added` pair — the
//!   single-record encoding RFC 0031's "Open questions" section fixes: "Whether a
//!   fairness `kind` change should be encoded as one `strengthened`/`weakened`
//!   record or as a `removed`+`added` pair; this RFC fixes the single-record
//!   encoding and the alternative is left open for the acceptance corpus to
//!   evaluate."
//!
//! # A recorded tension in RFC 0031's text (for the lead, not resolved here)
//!
//! The field-by-field table gives `fairness`'s unit of classification as "one
//! constraint, keyed by (`kind`, `action`)" — `kind` is *part* of the unit key under
//! that table's own definition, so `weak:A` and `strong:A` are two distinct units.
//! The `kind` bullet then describes `weak → strong` "on the same action" as if it
//! were one edit to one constraint, which requires matching *across* two different
//! keys by `action` alone — a second, narrower keying scheme laid over the first,
//! stated in prose with no rule for when a same-action removed+added pair should
//! *not* be read as a kind change (a condition edit at the same time; an action that
//! legitimately carries both kinds and drops one). This module resolves the gap
//! conservatively: the collapse fires only when the two candidate units' conditions
//! are structurally identical (see [`classify_fairness`]'s second pass, and
//! `positive_strengthening_the_real_fixtures_fairness_constraint_classifies_strengthened_and_is_reviewed`
//! in the evidence test for the case it *does* catch); a same-action swap whose
//! condition also differs is left as the RFC's own explicitly-sanctioned alternate
//! encoding, `removed`+`added` (see
//! `negative_a_same_action_swap_with_a_different_condition_is_not_collapsed_into_a_kind_change`,
//! below), rather than this module inventing an untested rule for combining two axes
//! of movement the RFC's `kind` bullet never actually considers together. Recorded
//! for the lead; not a claim that RFC 0031's text is wrong, only that it is silent
//! here and this module had to choose.
//!
//! # The oracle this module does not have
//!
//! `condition` is a property AST (a bare, temporal-operator-free formula — RFC
//! 0037's "the two carriers differ in shape... `fairness[].condition` is a **bare**
//! temporal-operator-free formula"), and RFC 0031 computes its relation "over CPNF-1
//! exactly as for claims": Finite-fragment behavior-set inclusion, with a checked
//! witness, per the "Properties (per fragment)" classification-lattice bullet. That
//! decision procedure is not implemented anywhere in `continuum-intent` as of this
//! bone — `continuum_intent::property`'s own module doc says plainly "Classification
//! is not here. Whether a change is `weakened` or `unknown` is RFC 0031's question
//! (PR 12)", and no Finite-fragment inclusion oracle exists yet in this crate or that
//! one (it is `properties`' bone, `bn-8mlg`, IMPL-02, still open at the time of this
//! writing). [`formula_relation`] is honest about the gap rather than guessing past
//! it: two *unequal*, *both-declared* conditions classify [`Relation::Unknown`] —
//! never a guessed direction, and never [`Relation::Incomparable`] either, since this
//! module never discharges a refutation of *both* inclusions (the only way RFC 0031
//! lets `incomparable` be earned for an expression-shaped comparison; see "Properties
//! (per fragment)" and "Fail-closed rule"). Only the two structurally decidable
//! moves — equality, and the `null` extremum RFC 0031 states outright ("`C → null`
//! classifies `strengthened`") — are computed without an oracle. This is recorded
//! again in the bone comment as a real, current gap for whoever lands the Finite
//! oracle next, not invented or duplicated here.
//!
//! # Why `assurance`'s `incomparable` exclusion has no analogue here
//!
//! `fairness`'s admissible-relation row *does* include `incomparable` (unlike
//! `assurance`'s, which excludes it by its own total-order rule). This module simply
//! never happens to construct one, for the reason above: the only way to earn
//! `incomparable` on an expression-shaped field is a discharged double refutation,
//! and this module has no discharge mechanism to call. A future oracle landing in
//! `continuum-intent` could make [`formula_relation`] return `Incomparable` for a
//! provably-neither-side-implies-the-other pair without changing this module's
//! `dualize` step at all — "`incomparable`/`unknown` pass through the duality
//! unchanged" is already implemented that way below.
//!
//! # Scope: PR-12 / IMPL-06 only
//!
//! See [`crate::faults`]'s module doc, "Scope: PR-12 / IMPL-06 only" — the same
//! boundary applies here and to [`crate::assurance`], all three landing together
//! under `bn-3vxp`.

use std::collections::{BTreeMap, BTreeSet};

use continuum_intent::ast::Formula;
use continuum_intent::change_policy::{
    ChangePolicyError, ClassificationRecord, PolicyField, Relation,
};
use continuum_intent::fairness::{
    ActionName, FairnessConstraint, FairnessKey, FairnessKind, FairnessSet,
};

/// The unit a [`FairnessChange`] names.
///
/// Either one ordinary (`kind`, `action`) key — RFC 0031's stated unit of
/// classification, used for membership records and for a same-key `condition`
/// movement — or the collapsed `kind`-movement record the module doc describes,
/// which names both kinds and the one action they share because no single
/// `FairnessKey` could.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FairnessUnit {
    /// An ordinary per-key record: membership, or a same-key `condition` movement.
    Key(FairnessKey),
    /// The collapsed `kind` movement: `before` and `after` are always the two
    /// different [`FairnessKind`] values, on the one `action` both candidate units
    /// shared.
    KindChange {
        /// The shared action family.
        action: ActionName,
        /// The kind before the revision.
        before: FairnessKind,
        /// The kind after the revision.
        after: FairnessKind,
    },
}

/// One classified `fairness[]` unit: a [`FairnessUnit`] and the [`Relation`] RFC 0031
/// assigns it.
///
/// The relation is always one [`PolicyField::Fairness`] admits — pinned by this
/// module's tests — because every relation [`classify_fairness`] can produce
/// (`unchanged`, `strengthened`, `weakened`, `added`, `removed`, `unknown`) is on the
/// fairness row. `incomparable` is on that row too but is never produced; see the
/// module doc, "Why `assurance`'s `incomparable` exclusion has no analogue here".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FairnessChange {
    unit: FairnessUnit,
    relation: Relation,
}

impl FairnessChange {
    fn new(unit: FairnessUnit, relation: Relation) -> Self {
        debug_assert!(
            PolicyField::Fairness.admits_relation(relation),
            "a fairness classification produced {relation}, which the fairness row does not admit"
        );
        Self { unit, relation }
    }

    /// The classified unit.
    #[must_use]
    pub const fn unit(&self) -> &FairnessUnit {
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
    /// relation this module produces is on the fairness row (see the struct doc),
    /// but [`ClassificationRecord::new`] is itself fallible and a `debug_assert!` is
    /// not a proof, so the `Result` is threaded rather than unwrapped.
    pub fn to_classification_record(&self) -> Result<ClassificationRecord, ChangePolicyError> {
        ClassificationRecord::new(PolicyField::Fairness, self.relation)
    }
}

/// Classify every `fairness[]` unit between `before` and `after`, per RFC 0031's
/// "`fairness`" rule.
///
/// Two passes:
///
/// 1. Every key present on both sides classifies by [`formula_relation`] plus
///    [`dualize`] on its `condition` (`kind` is fixed for a same-key comparison, so
///    this is the whole story for that unit). Every key present on one side only is
///    provisionally `added`/`removed`.
/// 2. The provisional before-only and after-only sets are scanned for the `kind`
///    movement: for each `action` with *exactly* one before-only unit and *exactly*
///    one after-only unit (the only shape W1 allows, since a duplicate `(kind,
///    action)` pair is rejected at construction), if their conditions are
///    structurally identical the pair collapses into one
///    [`FairnessUnit::KindChange`] record; otherwise both are left as plain
///    `removed`+`added` records (see the module doc's recorded tension for why this
///    module declines to guess a combined direction when both axes moved at once).
///
/// RFC 0031 permits a wire artifact to omit `unchanged` records; this function does
/// not, matching [`crate::observers::classify_observers`] and
/// [`crate::faults::classify_faults`]'s identical choice for the same reason: an
/// emission choice belongs to whatever assembles `intent_changes[]`, not to the
/// classifier.
#[must_use]
pub fn classify_fairness(before: &FairnessSet, after: &FairnessSet) -> Vec<FairnessChange> {
    let mut changes = Vec::new();
    let mut before_only: BTreeMap<ActionName, (FairnessKind, &FairnessConstraint)> =
        BTreeMap::new();
    let mut after_only: BTreeMap<ActionName, (FairnessKind, &FairnessConstraint)> = BTreeMap::new();

    let mut keys: BTreeSet<FairnessKey> = before.unit_keys().into_iter().cloned().collect();
    keys.extend(after.unit_keys().into_iter().cloned());

    for key in keys {
        match (before.get(&key), after.get(&key)) {
            (Some(b), Some(a)) => {
                let relation = dualize(formula_relation(b.condition(), a.condition()));
                changes.push(FairnessChange::new(FairnessUnit::Key(key), relation));
            }
            (Some(b), None) => {
                before_only.insert(b.action().clone(), (b.kind(), b));
            }
            (None, Some(a)) => {
                after_only.insert(a.action().clone(), (a.kind(), a));
            }
            (None, None) => {
                unreachable!("`key` came from the union of both sides' own `unit_keys()`")
            }
        }
    }

    // Pass 2: the `kind` movement, over whatever membership left provisional.
    let swap_candidates: Vec<ActionName> = before_only
        .keys()
        .filter(|action| after_only.contains_key(*action))
        .cloned()
        .collect();
    for action in swap_candidates {
        let (before_kind, before_constraint) =
            before_only.remove(&action).expect("just confirmed present");
        let (after_kind, after_constraint) =
            after_only.remove(&action).expect("just confirmed present");
        debug_assert_ne!(
            before_kind, after_kind,
            "the same (kind, action) key cannot be simultaneously before-only and after-only"
        );
        if before_constraint.condition() == after_constraint.condition() {
            let relation = match (before_kind, after_kind) {
                (FairnessKind::Weak, FairnessKind::Strong) => Relation::Strengthened,
                (FairnessKind::Strong, FairnessKind::Weak) => Relation::Weakened,
                (FairnessKind::Weak, FairnessKind::Weak)
                | (FairnessKind::Strong, FairnessKind::Strong) => {
                    unreachable!("the debug_assert_ne! above rules out equal kinds")
                }
            };
            changes.push(FairnessChange::new(
                FairnessUnit::KindChange {
                    action,
                    before: before_kind,
                    after: after_kind,
                },
                relation,
            ));
        } else {
            // Both axes moved at once: the RFC's `kind` bullet does not say what this
            // classifies (see the module doc's recorded tension), so this module
            // declines to guess and falls back to the RFC's own explicitly-sanctioned
            // alternate encoding.
            before_only.insert(action.clone(), (before_kind, before_constraint));
            after_only.insert(action, (after_kind, after_constraint));
        }
    }

    for (_, constraint) in before_only.into_values() {
        changes.push(FairnessChange::new(
            FairnessUnit::Key(constraint.key().clone()),
            Relation::Removed,
        ));
    }
    for (_, constraint) in after_only.into_values() {
        changes.push(FairnessChange::new(
            FairnessUnit::Key(constraint.key().clone()),
            Relation::Added,
        ));
    }

    changes
}

/// The relation on the `condition` *formula itself*, before [`dualize`] turns it into
/// the relation on the constraint.
///
/// `null` (`None`) is RFC 0031's stated bottom of the condition order ("the weakest
/// condition"), so:
///
/// - both unconditional, or both declared and structurally identical: the formula is
///   unchanged.
/// - a declared condition moving to `null`: the formula moved to the bottom, which is
///   a formula *weakening* (RFC 0031's `Relation::Weakened` — "More behaviors are
///   admitted" — matches "unconditional" admitting every state).
/// - `null` moving to a declared condition: the formula moved away from the bottom,
///   a formula *strengthening*.
/// - two declared, structurally unequal conditions: undecidable without the Finite
///   oracle this module does not have (see the module doc); the honest answer per
///   RFC 0031's S2 ("otherwise `unknown` (undecidable/unattempted)") is
///   [`Relation::Unknown`], never a guessed direction.
///
/// [`dualize`] turns this into the relation RFC 0031 actually wants recorded, which
/// is the relation on the *constraint*.
fn formula_relation(before: Option<&Formula>, after: Option<&Formula>) -> Relation {
    match (before, after) {
        (None, None) => Relation::Unchanged,
        (Some(before), Some(after)) if before == after => Relation::Unchanged,
        (Some(_), None) => Relation::Weakened,
        (None, Some(_)) => Relation::Strengthened,
        (Some(_), Some(_)) => Relation::Unknown,
    }
}

/// RFC 0031's antitone duality: "the relation on the constraint is the dual of the
/// relation on the formula: formula `weakened` ⇒ constraint `strengthened`, and
/// formula `strengthened` ⇒ constraint `weakened`... `incomparable`/`unknown` pass
/// through the duality unchanged."
fn dualize(formula_relation: Relation) -> Relation {
    match formula_relation {
        Relation::Strengthened => Relation::Weakened,
        Relation::Weakened => Relation::Strengthened,
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use continuum_intent::ast::Identifier;
    use continuum_intent::canonical_json::Json;

    use super::*;

    /// The dossier's validated Intent Contract example, included at compile time —
    /// used here only to confirm the live schema still carries no `fragment` member
    /// on a `fairness[]` item (the module doc's "no analogue" reasoning depends on
    /// there being no fragment budget to be `unsupported` about either).
    const SCHEMA: &str = include_str!("../../../notes/plan/schemas/intent-contract.schema.json");

    fn action(name: &str) -> ActionName {
        ActionName::new(name).expect("a test action name is non-empty")
    }

    fn key(kind: FairnessKind, name: &str) -> FairnessKey {
        FairnessKey::new(kind, action(name))
    }

    fn predicate(name: &str) -> Formula {
        Formula::predicate(
            Identifier::new(name).expect("a test identifier is well formed"),
            Vec::new(),
        )
    }

    fn constraint(
        kind: FairnessKind,
        name: &str,
        condition: Option<&Formula>,
    ) -> FairnessConstraint {
        FairnessConstraint::new(key(kind, name), condition).expect("test constraint is well formed")
    }

    fn set(constraints: impl IntoIterator<Item = FairnessConstraint>) -> FairnessSet {
        FairnessSet::from_constraints(constraints).expect("distinct keys in a test fixture")
    }

    fn relation_of(changes: &[FairnessChange], unit: &FairnessUnit) -> Relation {
        changes
            .iter()
            .find(|change| change.unit() == unit)
            .unwrap_or_else(|| panic!("no record for unit {unit:?} in {changes:?}"))
            .relation()
    }

    // --- membership --------------------------------------------------------------------------

    #[test]
    fn a_constraint_present_only_after_classifies_added() {
        let before = set([]);
        let after = set([constraint(FairnessKind::Weak, "A", None)]);
        let changes = classify_fairness(&before, &after);
        assert_eq!(changes.len(), 1);
        assert_eq!(
            relation_of(&changes, &FairnessUnit::Key(key(FairnessKind::Weak, "A"))),
            Relation::Added
        );
    }

    #[test]
    fn a_constraint_present_only_before_classifies_removed() {
        let before = set([constraint(FairnessKind::Weak, "A", None)]);
        let after = set([]);
        let changes = classify_fairness(&before, &after);
        assert_eq!(changes.len(), 1);
        assert_eq!(
            relation_of(&changes, &FairnessUnit::Key(key(FairnessKind::Weak, "A"))),
            Relation::Removed
        );
    }

    #[test]
    fn identical_sets_classify_every_unit_unchanged() {
        let before = set([constraint(FairnessKind::Weak, "A", Some(&predicate("p")))]);
        let after = set([constraint(FairnessKind::Weak, "A", Some(&predicate("p")))]);
        let changes = classify_fairness(&before, &after);
        assert_eq!(changes.len(), 1);
        assert_eq!(
            relation_of(&changes, &FairnessUnit::Key(key(FairnessKind::Weak, "A"))),
            Relation::Unchanged
        );
    }

    // --- `condition`, antitone ------------------------------------------------------------------

    #[test]
    fn condition_moving_to_null_strengthens_the_constraint() {
        // RFC 0031, verbatim: "`C → null` classifies `strengthened`."
        let before = set([constraint(FairnessKind::Weak, "A", Some(&predicate("p")))]);
        let after = set([constraint(FairnessKind::Weak, "A", None)]);
        let changes = classify_fairness(&before, &after);
        assert_eq!(
            relation_of(&changes, &FairnessUnit::Key(key(FairnessKind::Weak, "A"))),
            Relation::Strengthened
        );
    }

    #[test]
    fn condition_moving_away_from_null_weakens_the_constraint() {
        // The dual: `null → C` classifies `weakened`.
        let before = set([constraint(FairnessKind::Weak, "A", None)]);
        let after = set([constraint(FairnessKind::Weak, "A", Some(&predicate("p")))]);
        let changes = classify_fairness(&before, &after);
        assert_eq!(
            relation_of(&changes, &FairnessUnit::Key(key(FairnessKind::Weak, "A"))),
            Relation::Weakened
        );
    }

    #[test]
    fn absence_and_null_are_the_same_condition_and_classify_unchanged() {
        // RFC 0037: "An absent `condition` key MUST be read as `null`" — decoded
        // through `FairnessConstraint::from_json`, so this module never even sees a
        // distinction to fail closed on (see the module doc).
        let before =
            FairnessConstraint::decode(br#"{"action":"A","kind":"weak"}"#).expect("decodes");
        let after = constraint(FairnessKind::Weak, "A", None);
        let changes = classify_fairness(&set([before]), &set([after]));
        assert_eq!(
            relation_of(&changes, &FairnessUnit::Key(key(FairnessKind::Weak, "A"))),
            Relation::Unchanged
        );
    }

    #[test]
    fn two_distinct_declared_conditions_classify_unknown_not_a_guess() {
        // No Finite oracle exists yet (see the module doc): the honest answer is
        // `unknown`, never a guessed `strengthened`/`weakened`/`incomparable`.
        let before = set([constraint(FairnessKind::Weak, "A", Some(&predicate("p")))]);
        let after = set([constraint(FairnessKind::Weak, "A", Some(&predicate("q")))]);
        let changes = classify_fairness(&before, &after);
        assert_eq!(
            relation_of(&changes, &FairnessUnit::Key(key(FairnessKind::Weak, "A"))),
            Relation::Unknown
        );
    }

    // --- `kind` -----------------------------------------------------------------------------

    #[test]
    fn weak_to_strong_on_the_same_action_with_the_same_condition_classifies_strengthened() {
        let before = set([constraint(FairnessKind::Weak, "A", Some(&predicate("p")))]);
        let after = set([constraint(FairnessKind::Strong, "A", Some(&predicate("p")))]);
        let changes = classify_fairness(&before, &after);
        assert_eq!(changes.len(), 1, "{changes:?}");
        assert_eq!(
            relation_of(
                &changes,
                &FairnessUnit::KindChange {
                    action: action("A"),
                    before: FairnessKind::Weak,
                    after: FairnessKind::Strong,
                }
            ),
            Relation::Strengthened
        );
    }

    #[test]
    fn strong_to_weak_on_the_same_action_classifies_weakened() {
        let before = set([constraint(FairnessKind::Strong, "A", None)]);
        let after = set([constraint(FairnessKind::Weak, "A", None)]);
        let changes = classify_fairness(&before, &after);
        assert_eq!(changes.len(), 1, "{changes:?}");
        assert_eq!(
            relation_of(
                &changes,
                &FairnessUnit::KindChange {
                    action: action("A"),
                    before: FairnessKind::Strong,
                    after: FairnessKind::Weak,
                }
            ),
            Relation::Weakened
        );
    }

    #[test]
    fn the_collapsed_encoding_never_also_emits_a_removed_plus_added_pair() {
        // "A classifier MUST choose one encoding and MUST NOT emit both."
        let before = set([constraint(FairnessKind::Weak, "A", None)]);
        let after = set([constraint(FairnessKind::Strong, "A", None)]);
        let changes = classify_fairness(&before, &after);
        assert_eq!(changes.len(), 1, "{changes:?}");
        assert!(
            changes
                .iter()
                .all(|c| !matches!(c.relation(), Relation::Added | Relation::Removed))
        );
    }

    #[test]
    fn negative_a_same_action_swap_with_a_different_condition_is_not_collapsed_into_a_kind_change()
    {
        // The recorded tension: both axes moved at once, so this module declines to
        // guess a combined direction and falls back to the RFC's own sanctioned
        // alternate encoding.
        let before = set([constraint(FairnessKind::Weak, "A", Some(&predicate("p")))]);
        let after = set([constraint(FairnessKind::Strong, "A", Some(&predicate("q")))]);
        let changes = classify_fairness(&before, &after);
        assert_eq!(changes.len(), 2, "{changes:?}");
        assert_eq!(
            relation_of(&changes, &FairnessUnit::Key(key(FairnessKind::Weak, "A"))),
            Relation::Removed
        );
        assert_eq!(
            relation_of(&changes, &FairnessUnit::Key(key(FairnessKind::Strong, "A"))),
            Relation::Added
        );
    }

    #[test]
    fn a_kind_flip_does_not_interfere_with_an_unrelated_actions_membership() {
        let before = set([
            constraint(FairnessKind::Weak, "A", None),
            constraint(FairnessKind::Strong, "B", None),
        ]);
        let after = set([
            constraint(FairnessKind::Strong, "A", None),
            constraint(FairnessKind::Strong, "B", None),
        ]);
        let changes = classify_fairness(&before, &after);
        assert_eq!(changes.len(), 2, "{changes:?}");
        assert_eq!(
            relation_of(
                &changes,
                &FairnessUnit::KindChange {
                    action: action("A"),
                    before: FairnessKind::Weak,
                    after: FairnessKind::Strong,
                }
            ),
            Relation::Strengthened
        );
        assert_eq!(
            relation_of(&changes, &FairnessUnit::Key(key(FairnessKind::Strong, "B"))),
            Relation::Unchanged
        );
    }

    // --- boundary and determinism -------------------------------------------------------------

    #[test]
    fn two_empty_sets_classify_no_units_at_all() {
        let empty = set([]);
        assert!(classify_fairness(&empty, &empty).is_empty());
    }

    // --- the field-classification contract --------------------------------------------------

    #[test]
    fn every_relation_this_module_can_produce_is_admissible_on_the_fairness_field() {
        for relation in [
            Relation::Unchanged,
            Relation::Strengthened,
            Relation::Weakened,
            Relation::Added,
            Relation::Removed,
            Relation::Unknown,
        ] {
            assert!(
                PolicyField::Fairness.admits_relation(relation),
                "{relation} must be admissible on `fairness`"
            );
        }
    }

    #[test]
    fn to_classification_record_never_fails_for_a_relation_this_module_produced() {
        let before = set([constraint(FairnessKind::Weak, "A", Some(&predicate("p")))]);
        let after = set([constraint(FairnessKind::Weak, "A", Some(&predicate("q")))]);
        for change in classify_fairness(&before, &after) {
            let record = change
                .to_classification_record()
                .expect("this module's own relations are always admissible on `fairness`");
            assert_eq!(record.field(), PolicyField::Fairness);
            assert_eq!(record.relation(), change.relation());
        }
    }

    #[test]
    fn the_live_schema_carries_no_fragment_member_on_fairness_items() {
        let schema = Json::parse(SCHEMA.as_bytes()).expect("the schema itself is canonical JSON");
        let fairness_items = schema
            .as_object()
            .expect("schema root is an object")
            .get("properties")
            .and_then(Json::as_object)
            .expect("schema declares properties")
            .get("fairness")
            .and_then(Json::as_object)
            .expect("schema declares fairness")
            .get("items")
            .and_then(Json::as_object)
            .expect("fairness is an array schema with items");
        let item_properties = fairness_items
            .get("properties")
            .and_then(Json::as_object)
            .expect("fairness items declare properties");
        assert!(
            !item_properties.contains_key("fragment"),
            "fairness[] gained a `fragment` member; this module's reasoning about \
             `unsupported` depends on its absence and must be revisited"
        );
    }

    // --- anti-vacuity mutant: presence is not content -------------------------------------------

    /// A plausible, *wrong* classifier: decides a same-key comparison purely by
    /// whether the key is present on both sides, never looking at `condition`.
    fn mutant_blind_to_condition_content(before: &FairnessSet, after: &FairnessSet) -> Relation {
        let k = key(FairnessKind::Weak, "A");
        match (before.get(&k).is_some(), after.get(&k).is_some()) {
            (true, true) => Relation::Unchanged,
            _ => Relation::Unknown,
        }
    }

    #[test]
    fn negative_mutant_blind_to_condition_content_would_wrongly_pass_a_real_edit_as_unchanged() {
        let before = set([constraint(FairnessKind::Weak, "A", Some(&predicate("p")))]);
        let after = set([constraint(FairnessKind::Weak, "A", Some(&predicate("q")))]);

        let real = relation_of(
            &classify_fairness(&before, &after),
            &FairnessUnit::Key(key(FairnessKind::Weak, "A")),
        );
        assert_eq!(real, Relation::Unknown);

        let mutant_relation = mutant_blind_to_condition_content(&before, &after);
        assert_eq!(
            mutant_relation,
            Relation::Unchanged,
            "the mutant must actually get this wrong, or it is not exercising the bug"
        );
        assert_ne!(real, mutant_relation);
    }
}
