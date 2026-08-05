//! `assumptions` field classification: RFC 0031's `assumptions` rule (PR-12 / IMPL-03).
//!
//! # What this module answers
//!
//! Given two [`AssumptionSet`]s — an Intent Contract's `assumptions[]` before and
//! after a proposed revision — classify every unit into RFC 0031's closed relation
//! vocabulary:
//!
//! > | `assumptions` | `assumptions[]` | one assumption, keyed by `id` | behavior-set
//! > inclusion over CPNF-1 | as `properties` |
//! >
//! > — `notes/plan/rfcs/0031-semantic-and-intent-diff.md`, "Field-by-field
//! > classification rules"
//!
//! > ### `assumptions`
//! >
//! > Keyed by `id`. The relation is computed on `expression` exactly as for claims —
//! > the same order, on the same normal form. Direction and policy valence differ: an
//! > assumption `strengthened` admits fewer environments and makes verification
//! > easier, and is the gaming move plan §5.1 names ("adding a fairness assumption
//! > that schedules away the bug" is its fairness sibling). A change to
//! > `classification` or to `fidelity_profile` with an unchanged expression MUST
//! > classify `incomparable`: RFC 0002's fidelity profiles are explicitly not
//! > ordered, so no direction may be inferred.
//! >
//! > — RFC 0031, "`assumptions`"
//!
//! and the general per-unit membership rule every field inherits from "Field-by-field
//! classification rules": a unit key present on one side only classifies `added` or
//! `removed`.
//!
//! # The direction is the mirror image of `properties` — get this backwards and the
//! module is worse than useless
//!
//! The bone brief is explicit about this and the RFC text backs it exactly: for a
//! *property*, `weakened` is the dangerous direction (a claim now covers more
//! behaviors than it used to, so passing verification proves less). For an
//! *assumption*, the danger inverts. An assumption restricts the environments a claim
//! is checked against; `strengthened` shrinks that set, so the same claim is verified
//! against a smaller, easier world while reading as if nothing changed:
//!
//! > ### Intent attacks
//! >
//! > - weaken property;
//! > - **strengthen assumptions/fairness**;
//! > - shrink bounds;
//! > - remove faults;
//! > - coarsen observer;
//! > - lower assurance.
//! >
//! > — `notes/plan/docs/50_AGENT_EVALUATION_AND_REWARD_HACKING.md`, "Intent attacks"
//!
//! > within declared supported fragments, every property weakening, **assumption
//! > strengthening**, bound decrease, observer coarsening, fault removal, fairness
//! > addition or removal, and assurance downgrade is classified as privileged — the
//! > seven G3 dimensions.
//! >
//! > — `notes/plan/plan.md` §5.3
//!
//! So INV-001/INV-011's "weakening-is-privileged" posture, read at the level of
//! *proof obligations* rather than the bare relation token, applies to
//! `strengthened` here exactly as it applies to `weakened` on `properties`: both are
//! the direction that makes verification quietly easier than the author intended.
//! `assumptions`' own policy verbs (`unlocked`, `proposal-only`, `review`, `locked` —
//! the base row, RFC 0037 W6) contain no directional verb that names `strengthened`
//! specifically, exactly as they contain none for `properties`' `weakened` or
//! `observers`' `coarsened`: RFC 0037 states this is deliberate ("The closed set
//! contains no verb that blocks `added`. Adding an assumption or a fairness
//! constraint is environment-strengthening... so those fields are protected through
//! `review` or `locked`, never through a directional verb"), so the whole field is
//! gated by `review`/`locked` rather than a `no-strengthen` verb the closed set does
//! not have.
//!
//! *Flag, not silently resolved:* RFC 0031's "Completeness guarantee" section states
//! eight gaming moves in a table mapping (field, relation) to the blocking verb, one
//! row per plan §5.1 bullet — `observers`/`coarsened`, `fairness`/`added`+
//! `strengthened`, `bounds`/`contracted`, `faults`/`removed`, `trust_boundaries`/
//! `expanded`, `abstraction_maps`/`merged`, `assurance`/`downgraded`,
//! `properties`/`weakened` — but carries **no row for `assumptions`**, even though
//! "assumption strengthening" is named two paragraphs earlier as one of the seven G3
//! dimensions and docs/50 lists "strengthen assumptions" as its own attack class
//! beside "strengthen fairness". This module treats the omission as an editorial gap
//! in that one table, not a substantive question — the "`assumptions`" section quoted
//! above already states the relation (`strengthened`) and the field
//! (`assumptions`) in prose, and `assumptions`' verb is `review`/`locked` per W6's
//! base row for the same reason `fairness`'s is — and records the gap here and in the
//! bone comment (bn-7vg7) rather than deciding it silently.
//!
//! # No dualize — unlike `fairness[].condition` — derived, not assumed
//!
//! The oracle's answer is consumed **directly**: a formula relation *is* the
//! assumption relation, with no dual step in between. Getting this backwards is
//! exactly the dangerous-direction failure this family exists to prevent (a real
//! in-place strengthening would classify `weakened` and read as the safe
//! direction), so the polarity is derived from the texts and pinned by tests, not
//! assumed:
//!
//! - RFC 0031's `assumptions` section: "The relation is computed on `expression`
//!   exactly as for claims — the same order, on the same normal form." The *order*
//!   is `properties`' behavior-set inclusion, applied to the assumption's own
//!   expression; what "differ[s]" is "Direction and policy valence" — which
//!   direction is dangerous — never the mechanism. An assumption `strengthened`
//!   "admits fewer environments"; a formula `strengthened` admits fewer behaviors.
//!   Same sign, term for term.
//! - `fairness[].condition` is the contrast case, and RFC 0031 marks it
//!   explicitly: "the enabling predicate occupies an **antitone** position. A
//!   fairness constraint applies wherever its condition holds, so weakening the
//!   condition strengthens the constraint" — which is why
//!   [`crate::fairness::formula_relation`]'s answer runs through
//!   [`crate::fairness::dualize`]. An assumption's `expression` occupies no such
//!   position: it is not a guard on when something else applies, it *is* the thing
//!   whose behavior-set the direction is stated over, exactly as a claim's
//!   `expression` is. No antitone marker appears anywhere in the `assumptions`
//!   text.
//! - The antitone reading is anchored, in fairness's case, on a `null` extremum:
//!   "`null` means unconditional and is the weakest condition, hence the
//!   *strongest* constraint" (RFC 0031), with RFC 0037's absence rule ("An absent
//!   `condition` key MUST be read as `null`", the equivalence family correction 15
//!   extends to `scope.abstraction_level` alongside `bounds.values`/`bounds.depth`)
//!   supplying the absent-key case. `assumptions[].expression` is in neither list
//!   and has no null form at all — it is a *required* member of the schema's
//!   `assumptions[]` items (pinned below against the live schema) — so the
//!   weakest-formula/strongest-unit inversion that forces fairness's dual has no
//!   anchor here, and no extremum arm exists in this module's content axis.
//!
//! So this module has no `dualize` function because none is called for, and the
//! disguise sweeps prove the sign: a real in-place strengthening of a corpus
//! assumption classifies `strengthened` (and is reviewed/blocked), never
//! `weakened`, and a dualizing mutant is pinned wrong in both directions.
//!
//! # The oracle, and the license it is consulted under (`bn-2nwpg`)
//!
//! `expression` is a [`PropertyExpression`] — the identical `$defs/property_expression`
//! carrier `claims[].expression` uses (RFC 0037: "`assumptions[].expression` reuses
//! `PropertyExpression`") — and RFC 0031 computes its relation "exactly as for
//! claims". When this module landed (`bn-7vg7`) no Finite-fragment inclusion oracle
//! existed anywhere in this crate or `continuum-intent`, so every unequal
//! canonical encoding failed closed to [`Relation::Unknown`]; `bn-8mlg` (IMPL-02,
//! `properties`) has since landed [`crate::properties::finite_formula_relation`] —
//! a sound, bounded ([`crate::properties::IMPLICATION_STEP_BUDGET`]) implication
//! prover — and made [`crate::properties`] the crate's **one** formula-comparison
//! authority. This module now consumes it through the authority's own license arm,
//! [`crate::properties::expression_relation`], shared verbatim with `properties`'
//! content axis so the license cannot drift between the two consumers:
//!
//! - a direction is affirmed only when **both sides declare the same fragment and
//!   that fragment is `Finite`** (RFC 0037 S3's license — directions "require the
//!   fragment's own decision procedure");
//! - both sides declaring the same non-`Finite` fragment, the two sides declaring
//!   *different* fragments, and both sides leaving `fragment` absent (W3: absence
//!   denotes the *contract's* declared fragment, which a two-[`AssumptionSet`]
//!   classifier is never given — the same `scope.fragments` boundary the
//!   `unsupported` section below records) all fail closed to
//!   [`Relation::Unknown`];
//! - an implication the bounded derivation search cannot establish — including by
//!   budget exhaustion, and including a pair proved in *both* directions (the
//!   properties module doc's correction-16-style collision refusal, inherited here
//!   by delegation) — is [`Relation::Unknown`], never a guess and never
//!   `incomparable` (a failed derivation refutes nothing, so `incomparable` never
//!   flows from formula content anywhere in this crate).
//!
//! The one behavioral upgrade over `bn-7vg7`'s fail-closed landing is therefore
//! exactly the single-axis decidable case: an expression edit with declaredness
//! held fixed, both sides `Finite`, whose inclusion the oracle derives, now
//! classifies an *affirmed* [`Relation::Strengthened`] or [`Relation::Weakened`]
//! with a recomputable witness — the direction consumed straight, per the
//! derivation above. Everything else answers exactly as it did before this bone.
//!
//! # What counts as "the same canonical encoding" — and why `source` and a bare
//! declaredness edit are answered differently
//!
//! R2 claims `unchanged` only on equality of the *canonical encoding of the classified
//! unit*. For `assumptions` that unit's content-bearing part is `expression`, and
//! [`PropertyExpression`]'s own identity discipline (ID2, shared with `claims[]`)
//! already draws the line: the preimage is `ast` plus `fragment` (when declared),
//! and *excludes* the display-only `source` — "the display-only `source` rendering
//! MUST NOT contribute to identity or to any classification" (RFC 0031, "CPNF-1
//! interaction"). [`expression_unchanged`] compares exactly that pair
//! ([`PropertyExpression::ast`], [`PropertyExpression::fragment`]) rather than
//! `PropertyExpression`'s derived [`PartialEq`] (which is *document* equality and
//! does include `source`, per that type's own doc) — so a `source` rewrite with the
//! AST held fixed classifies `unchanged` here exactly as ID7 requires it to.
//!
//! `classification` and `fidelity_profile` are outside that preimage question
//! entirely: they are declaredness flags on the assumption itself, not part of the
//! expression, and RFC 0031 gives their movement its own rule, quoted above, scoped
//! explicitly to "an unchanged expression". [`classify_pair`] implements the scope
//! literally: the declaredness check only fires when [`expression_unchanged`] is
//! `true`. When the expression *also* moved, RFC 0031's `assumptions` section does
//! not state a combined rule for "`expression` and `classification` both moved in the
//! same revision" — the same silence `properties`' analogous `kind`/`observer` bullet
//! has ("a `kind` change with an unchanged expression classifies `incomparable`",
//! stated only for the fixed-expression case). The reading `bn-7vg7` recorded here
//! rather than decided silently — and which this bone (`bn-2nwpg`) preserves
//! unchanged, exactly as `properties` resolved its identical silence: the compound
//! edit classifies [`Relation::Unknown`], and **the oracle is never consulted
//! across a declaredness move**, so a `classification`/`fidelity_profile` flip can
//! never ride a decidable expression edit into an affirmed direction (tested — the
//! mirror of `properties`' "the implication oracle is never consulted across a
//! meaning change"). `Unknown` and `Incomparable` are both non-affirmative and
//! fail closed identically under `PolicyTable::verdict`'s P2, so folding the
//! coincident declaredness edit into the same `Unknown` record costs nothing the
//! fail-closed rule cares about, while inventing a case split RFC 0031 never
//! states would risk being *more* permissive than the text, not less. `bn-181aj`
//! may ratify this resolution as an RFC correction; until it does, the recorded
//! reading stands.
//!
//! # `fragment`-only movement
//!
//! `expression.fragment` is part of the ID2 preimage (unlike `properties`, where the
//! same fact already holds and is pinned by `property.rs`'s own
//! `id7_every_semantic_part_moves_the_identity` test), so a fragment-only change
//! (identical `ast`, different `fragment`, or a declared fragment moving to/from
//! absent) is caught by [`expression_unchanged`] exactly as an `ast` change is —
//! [`Relation::Unknown`], never a guessed `unchanged`. RFC 0031's `assumptions` text
//! does not separately name this case, and it is not a case any live fixture in this
//! corpus exercises; the reading here is the same conservative one the declaredness
//! case above takes, for the same reason.
//!
//! # `unsupported` is out of this module's reach, honestly
//!
//! Unlike `faults[]`, `fairness[]`, and `observers[]` — none of which carry a
//! `fragment` member, which is why those modules' classifiers never emit
//! `unsupported` — `assumptions[].expression.fragment` is a real member of the
//! schema, and "Fragment attribution" states a unit whose fragment is outside the
//! after-contract's declared `scope.fragments` MUST classify `unsupported`. This
//! module cannot decide that: `classify_assumptions` takes two [`AssumptionSet`]s and
//! nothing else, and `scope.fragments` lives in a sibling field group, exactly as
//! `continuum_intent::assumptions::AssumptionSet::check_fragments` already requires
//! that same external `declared: &BTreeSet<Fragment>` parameter to answer the
//! adjacent well-formedness question. Whatever assembles the full `diff_*` artifact
//! across all fifteen fields has that context; a single field's pairwise classifier
//! does not, and manufacturing an opinion here would be guessing with data this
//! function was never given.
//!
//! # Scope: PR-12 / IMPL-03 only
//!
//! PR 12 (`notes/START_HERE_IMPLEMENTATION.md`) lists six classification bullets:
//! exact equality, property AST edit, **assumption add/remove**, bound change,
//! observer event change, and fault/fairness/assurance change. This bone (`bn-7vg7`)
//! delivers the third, and only the third — the whole `assumptions` field
//! classification, per the pattern the other five bullets already set (each bone
//! delivers its named field's *entire* RFC 0031 rule, not only the literal words in
//! its bullet title; `bn-ycn6`'s "bound change" delivers all of `expanded`/
//! `contracted`/`incomparable`/`unknown`, not only "change"). The other five bullets
//! are separate bones — `bn-b8ru` (IMPL-01, exact equality), `bn-8mlg` (IMPL-02,
//! property AST edit), `bn-ycn6` (IMPL-04, bound change), `bn-1sdp` (IMPL-05, observer
//! event change), `bn-3vxp` (IMPL-06, fault/fairness/assurance change) — landing as
//! their own reviewable units. The Finite-fragment inclusion oracle itself is
//! `properties`' (`bn-8mlg`); this module consumes it through the shared license
//! arm (`bn-2nwpg`) and implements none of it. Still out of scope, honestly: the
//! wire `intent_changes[]`/`diff_*` artifact assembly, the impact set, and the
//! field-level P1 completeness a
//! `continuum_intent::change_policy::PolicyTable::verdict` call needs even when
//! zero units changed — all of it belongs to whatever later bone assembles a full
//! `diff_*` artifact across all fifteen fields, not to one field's classifier.

use std::collections::BTreeSet;

use continuum_intent::assumptions::{Assumption, AssumptionSet, UnitKey};
use continuum_intent::change_policy::{
    ChangePolicyError, ClassificationRecord, PolicyField, Relation,
};
use continuum_intent::property::PropertyExpression;

/// One classified `assumptions[]` unit: a [`UnitKey`] and the [`Relation`] RFC 0031
/// assigns it.
///
/// The relation is always one [`PolicyField::Assumptions`] admits — pinned by this
/// module's tests, not merely asserted — because [`classify_assumptions`] only ever
/// produces [`Relation::Unchanged`], [`Relation::Strengthened`],
/// [`Relation::Weakened`], [`Relation::Added`], [`Relation::Removed`],
/// [`Relation::Incomparable`], or [`Relation::Unknown`], all seven of which are on
/// the assumptions row. The directions come only from the `Finite`-licensed oracle
/// arm (`bn-2nwpg`; module doc, "The oracle, and the license it is consulted
/// under"); `unsupported` is admissible on the row too but never emitted here — see
/// "`unsupported` is out of this module's reach, honestly".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssumptionChange {
    unit: UnitKey,
    relation: Relation,
}

impl AssumptionChange {
    fn new(unit: UnitKey, relation: Relation) -> Self {
        debug_assert!(
            PolicyField::Assumptions.admits_relation(relation),
            "an assumption classification produced {relation}, which the assumptions row does not admit"
        );
        Self { unit, relation }
    }

    /// The classified unit: RFC 0031's per-unit locator for `assumptions`, "keyed by
    /// `id`".
    #[must_use]
    pub const fn unit(&self) -> &UnitKey {
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
    /// relation this module produces is on the assumptions row (see the struct doc),
    /// but [`ClassificationRecord::new`] is itself fallible and a `debug_assert!` is
    /// not a proof, so the `Result` is threaded rather than unwrapped.
    pub fn to_classification_record(&self) -> Result<ClassificationRecord, ChangePolicyError> {
        ClassificationRecord::new(PolicyField::Assumptions, self.relation)
    }
}

/// Classify every `assumptions[]` unit between `before` and `after`, per RFC 0031's
/// "`assumptions`" rule.
///
/// Total: one [`AssumptionChange`] for every unit key declared in `before`, in
/// `after`, or both — including [`Relation::Unchanged`] units. RFC 0031 permits a
/// wire artifact to omit `unchanged` records ("Field-by-field classification rules":
/// "Records whose relation is `unchanged` MAY be omitted"), but that is an emission
/// choice for whatever assembles the wire `intent_changes[]` array, not a property of
/// the classifier — matching [`crate::observers::classify_observers`] and
/// [`crate::faults::classify_faults`]'s identical choice for the same reason.
///
/// A unit present on one side only classifies [`Relation::Added`]/[`Relation::Removed`]
/// by key alone — the classifier never matches an `after`-only unit against a
/// `before`-only one by content to "recognize" a rename, exactly as `properties`'
/// text requires ("the classifier MUST NOT match claims by expression to defeat the
/// rename"): a unit key is the whole of a unit's identity for membership purposes.
/// A unit present on both sides is classified by [`classify_pair`].
///
/// Units are visited in [`UnitKey`] order (the union of both sides' unit keys), so
/// the output order is deterministic and does not depend on either input's authored
/// array order, matching RFC 0031's "Determinism": "the diff artifact MUST be
/// byte-identical across platforms and releases".
#[must_use]
pub fn classify_assumptions(
    before: &AssumptionSet,
    after: &AssumptionSet,
) -> Vec<AssumptionChange> {
    let mut units: BTreeSet<UnitKey> = before.iter().map(|a| a.unit().clone()).collect();
    units.extend(after.iter().map(|a| a.unit().clone()));
    units
        .into_iter()
        .map(|unit| {
            let relation = match (before.get(&unit), after.get(&unit)) {
                (None, Some(_)) => Relation::Added,
                (Some(_), None) => Relation::Removed,
                (Some(b), Some(a)) => classify_pair(b, a),
                (None, None) => {
                    unreachable!("`unit` came from the union of both sides' own unit keys")
                }
            };
            AssumptionChange::new(unit, relation)
        })
        .collect()
}

/// Whether two [`PropertyExpression`]s share the same canonical encoding: equal `ast`
/// and equal `fragment`, ignoring the display-only `source` entirely.
///
/// This is deliberately narrower than [`PropertyExpression`]'s derived [`PartialEq`],
/// which is *document* equality and includes `source` (see that type's own doc: "The
/// two carriers differ... `PartialEq` is document equality and includes `source`.
/// Identity is *semantic* and excludes it"). RFC 0031 R2 and "CPNF-1 interaction"
/// both require exactly the semantic reading for a classification decision — "the
/// display-only `source` rendering MUST NOT contribute to... any classification" — so
/// this function reaches for [`PropertyExpression::ast`] and
/// [`PropertyExpression::fragment`] directly rather than the type's `==`.
#[must_use]
fn expression_unchanged(before: &PropertyExpression, after: &PropertyExpression) -> bool {
    before.ast() == after.ast() && before.fragment() == after.fragment()
}

/// RFC 0031's per-unit comparison for one assumption present on both sides.
///
/// The declaredness axis first, then the content axis — the same two-axis shape as
/// [`crate::properties`]' `classify_pair`, with `classification`/`fidelity_profile`
/// standing where `kind`/`observer` stand there:
///
/// - Declaredness fixed, equal canonical encodings ([`expression_unchanged`]):
///   [`Relation::Unchanged`] — R2, and nothing weaker.
/// - Declaredness moved (`classification`, `fidelity_profile`, or both), equal
///   encodings: [`Relation::Incomparable`] — RFC 0031's explicit rule for this
///   field, quoted in the module doc, scoped to exactly this case.
/// - Declaredness moved *and* encodings unequal: [`Relation::Unknown`] — the
///   compound case the RFC does not state, folded into the non-affirmative
///   relation that already governs; the oracle is never consulted across a
///   declaredness move (module doc, and `bn-7vg7`'s recorded resolution).
/// - Declaredness fixed, encodings unequal:
///   [`crate::properties::expression_relation`] — the shared fragment license,
///   then the bounded oracle, its relation consumed directly with no dualize
///   (module doc, "No dualize"; `bn-2nwpg`).
///
/// A `debug_assert_eq!` cross-checks [`expression_unchanged`] against
/// [`PropertyExpression::identity_preimage_json`] equality on every call — the two
/// must always agree, since both are computed from exactly `ast` and `fragment` — the
/// same discipline [`crate::observers::classify_pair`] applies against
/// `Observer::identity`.
#[must_use]
fn classify_pair(before: &Assumption, after: &Assumption) -> Relation {
    let declaredness_moved = before.classification() != after.classification()
        || before.fidelity_profile() != after.fidelity_profile();
    let unchanged = expression_unchanged(before.expression(), after.expression());
    debug_assert_eq!(
        unchanged,
        before.expression().identity_preimage_json() == after.expression().identity_preimage_json(),
        "expression_unchanged and identity_preimage_json equality must agree for unit {}",
        before.unit(),
    );
    match (declaredness_moved, unchanged) {
        (false, true) => Relation::Unchanged,
        (true, true) => Relation::Incomparable,
        (true, false) => Relation::Unknown,
        (false, false) => {
            crate::properties::expression_relation(before.expression(), after.expression())
        }
    }
}

#[cfg(test)]
mod tests {
    use continuum_intent::assumptions::{AssumptionClassification, FidelityProfile};
    use continuum_intent::ast::{Formula, Fragment, Identifier};
    use continuum_intent::canonical_json::Json;

    use super::*;

    /// The dossier's validated Intent Contract example, included at compile time —
    /// used here only to confirm the live schema still carries an `expression`
    /// member with a nested `fragment` on `assumptions[]` (the module doc's
    /// "`unsupported` is out of this module's reach" claim depends on this fragment
    /// existing, in contrast to the sibling fields that have none).
    const SCHEMA: &str = include_str!("../../../notes/plan/schemas/intent-contract.schema.json");

    fn ident(name: &str) -> Identifier {
        Identifier::new(name).expect("a test identifier is well formed")
    }

    fn unit(id: &str) -> UnitKey {
        UnitKey::new(id).expect("a test unit key is non-empty")
    }

    fn predicate(name: &str) -> Formula {
        Formula::predicate(ident(name), Vec::new())
    }

    fn expression(ast: &Formula, fragment: Option<Fragment>) -> PropertyExpression {
        PropertyExpression::normalized(ast, fragment, None).expect("normalizes")
    }

    fn expression_with_source(
        ast: &Formula,
        fragment: Option<Fragment>,
        source: &str,
    ) -> PropertyExpression {
        PropertyExpression::normalized(ast, fragment, Some(source.to_owned())).expect("normalizes")
    }

    fn assumption(
        id: &str,
        ast: &Formula,
        classification: Option<AssumptionClassification>,
        fidelity_profile: Option<FidelityProfile>,
    ) -> Assumption {
        Assumption::new(
            unit(id),
            expression(ast, Some(Fragment::Finite)),
            classification,
            fidelity_profile,
        )
    }

    fn set(assumptions: impl IntoIterator<Item = Assumption>) -> AssumptionSet {
        AssumptionSet::from_assumptions(assumptions).expect("distinct ids in a test fixture")
    }

    fn relation_of(changes: &[AssumptionChange], id: &str) -> Relation {
        changes
            .iter()
            .find(|change| change.unit() == &unit(id))
            .unwrap_or_else(|| panic!("no record for unit {id:?} in {changes:?}"))
            .relation()
    }

    // --- membership: added / removed ----------------------------------------------------------

    #[test]
    fn a_unit_present_only_after_classifies_added() {
        let before = set([]);
        let after = set([assumption("A", &predicate("p"), None, None)]);
        let changes = classify_assumptions(&before, &after);
        assert_eq!(changes.len(), 1);
        assert_eq!(relation_of(&changes, "A"), Relation::Added);
    }

    #[test]
    fn a_unit_present_only_before_classifies_removed() {
        let before = set([assumption("A", &predicate("p"), None, None)]);
        let after = set([]);
        let changes = classify_assumptions(&before, &after);
        assert_eq!(changes.len(), 1);
        assert_eq!(relation_of(&changes, "A"), Relation::Removed);
    }

    #[test]
    fn both_sides_empty_classifies_no_units_at_all() {
        let empty = set([]);
        assert!(classify_assumptions(&empty, &empty).is_empty());
    }

    #[test]
    fn negative_renaming_an_assumption_is_removed_plus_added_never_unchanged() {
        // Parallel to `properties`' own text: "Renaming an id while preserving the
        // expression is removed plus added, not unchanged; the classifier MUST NOT
        // match claims by expression to defeat the rename." This module never
        // attempts a content match across keys, so the disguise is caught for free —
        // pinned here rather than merely assumed.
        let before = set([assumption("A", &predicate("p"), None, None)]);
        let after = set([assumption("A-v2", &predicate("p"), None, None)]);
        let changes = classify_assumptions(&before, &after);
        assert_eq!(changes.len(), 2, "{changes:?}");
        assert_eq!(relation_of(&changes, "A"), Relation::Removed);
        assert_eq!(relation_of(&changes, "A-v2"), Relation::Added);
    }

    // --- unchanged: canonical-encoding equality, and nothing weaker ----------------------------

    #[test]
    fn identical_assumptions_classify_unchanged() {
        let before = set([assumption(
            "A",
            &predicate("p"),
            Some(AssumptionClassification::Storage),
            Some(FidelityProfile::Contractual),
        )]);
        let after = set([assumption(
            "A",
            &predicate("p"),
            Some(AssumptionClassification::Storage),
            Some(FidelityProfile::Contractual),
        )]);
        let changes = classify_assumptions(&before, &after);
        assert_eq!(changes.len(), 1);
        assert_eq!(relation_of(&changes, "A"), Relation::Unchanged);
    }

    #[test]
    fn rewriting_source_alone_still_classifies_unchanged() {
        // R2 / "CPNF-1 interaction": "the display-only `source` rendering MUST NOT
        // contribute to identity or to any classification."
        let before = AssumptionSet::from_assumptions([Assumption::new(
            unit("A"),
            expression_with_source(&predicate("p"), Some(Fragment::Finite), "original wording"),
            None,
            None,
        )])
        .expect("one assumption");
        let after = AssumptionSet::from_assumptions([Assumption::new(
            unit("A"),
            expression_with_source(
                &predicate("p"),
                Some(Fragment::Finite),
                "a completely different rendering of the same fact",
            ),
            None,
            None,
        )])
        .expect("one assumption");
        let changes = classify_assumptions(&before, &after);
        assert_eq!(relation_of(&changes, "A"), Relation::Unchanged);
    }

    // --- declaredness: classification / fidelity_profile, expression held fixed ----------------

    #[test]
    fn a_classification_change_with_an_unchanged_expression_classifies_incomparable() {
        let before = set([assumption("A", &predicate("p"), None, None)]);
        let after = set([assumption(
            "A",
            &predicate("p"),
            Some(AssumptionClassification::Environment),
            None,
        )]);
        let changes = classify_assumptions(&before, &after);
        assert_eq!(relation_of(&changes, "A"), Relation::Incomparable);
    }

    #[test]
    fn a_fidelity_profile_change_with_an_unchanged_expression_classifies_incomparable() {
        // "RFC 0002's fidelity profiles are explicitly not ordered, so no direction
        // may be inferred" — this must never read as `strengthened`/`weakened` even
        // though `ideal -> adversarial-envelope` sounds directional in English.
        let before = set([assumption(
            "A",
            &predicate("p"),
            None,
            Some(FidelityProfile::Ideal),
        )]);
        let after = set([assumption(
            "A",
            &predicate("p"),
            None,
            Some(FidelityProfile::AdversarialEnvelope),
        )]);
        let changes = classify_assumptions(&before, &after);
        assert_eq!(relation_of(&changes, "A"), Relation::Incomparable);
    }

    #[test]
    fn a_declaredness_change_from_undeclared_to_declared_is_incomparable_never_unchanged() {
        // Declaredness itself is part of the meaning (RFC 0037: "absence means
        // *undeclared*, which is distinct from every declared value").
        let before = set([assumption("A", &predicate("p"), None, None)]);
        let after = set([assumption(
            "A",
            &predicate("p"),
            None,
            Some(FidelityProfile::Ideal),
        )]);
        let changes = classify_assumptions(&before, &after);
        assert_eq!(relation_of(&changes, "A"), Relation::Incomparable);
    }

    #[test]
    fn both_classification_and_fidelity_profile_moving_together_is_still_one_incomparable_record() {
        let before = set([assumption("A", &predicate("p"), None, None)]);
        let after = set([assumption(
            "A",
            &predicate("p"),
            Some(AssumptionClassification::Network),
            Some(FidelityProfile::PlatformQualified),
        )]);
        let changes = classify_assumptions(&before, &after);
        assert_eq!(changes.len(), 1, "{changes:?}");
        assert_eq!(relation_of(&changes, "A"), Relation::Incomparable);
    }

    // --- content: the licensed oracle, consumed directly (bn-2nwpg) ----------------------------

    #[test]
    fn two_distinct_expressions_the_oracle_cannot_relate_classify_unknown_not_a_guess() {
        // The oracle is consulted now (bn-2nwpg) but cannot relate two opaque
        // atoms in either direction, and a failed derivation refutes nothing: the
        // honest answer is `unknown`, never a guessed
        // `strengthened`/`weakened`/`incomparable` — S2's safe side, unchanged from
        // bn-7vg7's landing for exactly this pair.
        let before = set([assumption("A", &predicate("p"), None, None)]);
        let after = set([assumption("A", &predicate("q"), None, None)]);
        let changes = classify_assumptions(&before, &after);
        assert_eq!(relation_of(&changes, "A"), Relation::Unknown);
    }

    #[test]
    fn a_decidable_in_place_narrowing_classifies_an_affirmed_strengthened() {
        // The dangerous direction, decided and named: `p` -> `p AND q` admits
        // fewer environments, and the oracle derives behaviors(after) ⊆
        // behaviors(before) by conjunct rules — RFC 0031's "an assumption
        // `strengthened` admits fewer environments and makes verification easier",
        // consumed directly with no dualize (bn-2nwpg; module doc, "No dualize").
        let narrowed = Formula::and(vec![predicate("p"), predicate("q")]).expect("two operands");
        let before = set([assumption("A", &predicate("p"), None, None)]);
        let after = set([assumption("A", &narrowed, None, None)]);
        let changes = classify_assumptions(&before, &after);
        assert_eq!(relation_of(&changes, "A"), Relation::Strengthened);
    }

    #[test]
    fn a_decidable_in_place_widening_classifies_an_affirmed_weakened() {
        // The mirror, so neither directional arm is vacuous: `p` -> `p OR q`
        // admits more environments — formula weakened IS assumption weakened.
        let widened = Formula::or(vec![predicate("p"), predicate("q")]).expect("two operands");
        let before = set([assumption("A", &predicate("p"), None, None)]);
        let after = set([assumption("A", &widened, None, None)]);
        let changes = classify_assumptions(&before, &after);
        assert_eq!(relation_of(&changes, "A"), Relation::Weakened);
    }

    #[test]
    fn polarity_a_sweep_of_dressed_up_strengthenings_never_classifies_weakened_or_silent() {
        // The disguise sweep the bone brief demands prove the polarity rather than
        // assume it: every pair below is a genuine strengthening
        // (behaviors(after) ⊆ behaviors(before)), arriving in CPNF-1 through
        // several different shapes (the `not` costume is rewritten by N2 into
        // conjunct form before the oracle ever sees it — the normalizer, not this
        // module, dissolves that dressing). A dualized consumption — fairness's
        // antitone reading misapplied here — would classify every one `weakened`,
        // the safe-looking token on exactly the dangerous move. None may classify
        // `weakened`, `unchanged`, or fail to appear.
        let p_and_q = Formula::and(vec![predicate("p"), predicate("q")]).expect("two operands");
        let p_or_q = Formula::or(vec![predicate("p"), predicate("q")]).expect("two operands");
        let cases: Vec<(&str, Formula, Formula)> = vec![
            ("conjunct added", predicate("p"), p_and_q.clone()),
            ("disjunct dropped", p_or_q.clone(), predicate("p")),
            (
                "narrowing under always",
                Formula::always(p_or_q.clone()),
                Formula::always(predicate("p")),
            ),
            (
                "narrowing under eventually",
                Formula::eventually(p_or_q),
                Formula::eventually(predicate("p")),
            ),
            (
                "widening under not (antitone congruence)",
                Formula::not(predicate("p")),
                Formula::not(Formula::or(vec![predicate("p"), predicate("q")]).expect("two")),
            ),
        ];
        for (name, before_ast, after_ast) in cases {
            let before = set([assumption("A", &before_ast, None, None)]);
            let after = set([assumption("A", &after_ast, None, None)]);
            let changes = classify_assumptions(&before, &after);
            assert_eq!(
                relation_of(&changes, "A"),
                Relation::Strengthened,
                "disguise {name:?} must classify strengthened — anything else is the \
                 dangerous-direction polarity failure"
            );
        }
    }

    #[test]
    fn a_content_change_alongside_a_declaredness_change_still_classifies_unknown() {
        // Both axes moved: `Unknown` governs regardless of the coincident
        // declaredness edit — bn-7vg7's recorded reading of RFC 0031's silence on
        // the combined case, preserved by bn-2nwpg. Strengthened deliberately by
        // this bone (bn-2nwpg): the expression pair is now one the oracle CAN
        // decide (asserted, so the pin cannot go stale vacuously), proving the
        // oracle is never consulted across a declaredness move — the mirror of
        // properties' "a `kind` flip cannot ride an expression edit into a
        // direction". Never `unchanged`, never an affirmed direction, and never a
        // fabricated `incomparable`.
        let narrowed = Formula::and(vec![predicate("p"), predicate("q")]).expect("two operands");
        assert_eq!(
            crate::properties::finite_formula_relation(&predicate("p"), &narrowed),
            Relation::Strengthened,
            "the oracle must be able to decide this pair, or the pin is vacuous"
        );
        let before = set([assumption("A", &predicate("p"), None, None)]);
        let after = set([assumption(
            "A",
            &narrowed,
            Some(AssumptionClassification::Trust),
            None,
        )]);
        let changes = classify_assumptions(&before, &after);
        assert_eq!(relation_of(&changes, "A"), Relation::Unknown);
    }

    // --- content: the fragment license fails closed (bn-2nwpg) ---------------------------------

    #[test]
    fn a_decidable_shaped_edit_without_the_finite_license_classifies_unknown() {
        // The same `p` -> `p AND q` narrowing the decided test above affirms, in
        // every unlicensed costume: S3 grants directions only under "the
        // fragment's own decision procedure", so each must fail closed to
        // `unknown` — never a direction on an unlicensed pair.
        let narrowed = Formula::and(vec![predicate("p"), predicate("q")]).expect("two operands");
        let unlicensed: Vec<(&str, Option<Fragment>, Option<Fragment>)> = vec![
            (
                "both sides Symbolic",
                Some(Fragment::Symbolic),
                Some(Fragment::Symbolic),
            ),
            (
                "fragment moved Finite -> Symbolic",
                Some(Fragment::Finite),
                Some(Fragment::Symbolic),
            ),
            ("declared on one side only", Some(Fragment::Finite), None),
            ("both sides absent", None, None),
        ];
        for (name, before_fragment, after_fragment) in unlicensed {
            let before = AssumptionSet::from_assumptions([Assumption::new(
                unit("A"),
                expression(&predicate("p"), before_fragment),
                None,
                None,
            )])
            .expect("one assumption");
            let after = AssumptionSet::from_assumptions([Assumption::new(
                unit("A"),
                expression(&narrowed, after_fragment),
                None,
                None,
            )])
            .expect("one assumption");
            let changes = classify_assumptions(&before, &after);
            assert_eq!(
                relation_of(&changes, "A"),
                Relation::Unknown,
                "unlicensed case {name:?} must fail closed"
            );
        }
    }

    #[test]
    fn the_step_budget_fails_closed_through_the_assumptions_path() {
        // Boundedness is observable through this module, not only through the
        // oracle's own surface: the same one-disjunct widening decides `weakened`
        // at 8 atoms and exhausts `IMPLICATION_STEP_BUDGET` into `unknown` at 400
        // — mirroring the properties evidence, through `classify_assumptions`.
        // Both inputs are flat linear vectors of distinct atoms, built once.
        let disjuncts = |n: usize| -> Formula {
            Formula::or((0..n).map(|i| predicate(&format!("p{i:03}"))).collect())
                .expect("two or more operands")
        };
        for (atoms, expected) in [(8, Relation::Weakened), (400, Relation::Unknown)] {
            let before = set([assumption("A", &disjuncts(atoms), None, None)]);
            let after = set([assumption("A", &disjuncts(atoms + 1), None, None)]);
            let changes = classify_assumptions(&before, &after);
            assert_eq!(
                relation_of(&changes, "A"),
                expected,
                "at {atoms} atoms the one-disjunct widening must classify {expected}"
            );
        }
    }

    // --- polarity anti-vacuity: a dualizing consumer is wrong in both directions ---------------

    /// The plausible, *wrong* wiring this bone's brief names as the family's
    /// dangerous-direction failure: consuming the oracle through
    /// `fairness`'s antitone dual (formula `strengthened` ⇒ unit `weakened` and
    /// vice versa) where RFC 0031 says "exactly as for claims — the same order".
    fn mutant_dualizes_the_oracle(before: &Assumption, after: &Assumption) -> Relation {
        match crate::properties::expression_relation(before.expression(), after.expression()) {
            Relation::Strengthened => Relation::Weakened,
            Relation::Weakened => Relation::Strengthened,
            other => other,
        }
    }

    #[test]
    fn negative_mutant_dualizing_the_oracle_is_wrong_in_both_directions() {
        let narrowed = Formula::and(vec![predicate("p"), predicate("q")]).expect("two operands");
        let widened = Formula::or(vec![predicate("p"), predicate("q")]).expect("two operands");

        // Direction one: a real strengthening (the dangerous direction — the
        // dualized reading would sell it as the safe one).
        let before = assumption("A", &predicate("p"), None, None);
        let after = assumption("A", &narrowed, None, None);
        assert_eq!(classify_pair(&before, &after), Relation::Strengthened);
        assert_eq!(
            mutant_dualizes_the_oracle(&before, &after),
            Relation::Weakened,
            "the mutant must actually get this wrong, or it is not exercising the bug"
        );

        // Direction two: a real weakening.
        let after_wide = assumption("A", &widened, None, None);
        assert_eq!(classify_pair(&before, &after_wide), Relation::Weakened);
        assert_eq!(
            mutant_dualizes_the_oracle(&before, &after_wide),
            Relation::Strengthened,
            "the mutant must actually get this wrong, or it is not exercising the bug"
        );
    }

    #[test]
    fn a_fragment_only_change_with_identical_ast_classifies_unknown_not_unchanged() {
        // `fragment` is part of the ID2 preimage exactly as it is for `properties`
        // (module doc, "`fragment`-only movement"): a bare fragment move must not
        // read as a no-op.
        let before = AssumptionSet::from_assumptions([Assumption::new(
            unit("A"),
            expression(&predicate("p"), Some(Fragment::Finite)),
            None,
            None,
        )])
        .expect("one assumption");
        let after = AssumptionSet::from_assumptions([Assumption::new(
            unit("A"),
            expression(&predicate("p"), Some(Fragment::Symbolic)),
            None,
            None,
        )])
        .expect("one assumption");
        let changes = classify_assumptions(&before, &after);
        assert_eq!(relation_of(&changes, "A"), Relation::Unknown);
    }

    // --- determinism ----------------------------------------------------------------------------

    #[test]
    fn output_order_is_unit_key_order_regardless_of_authored_order() {
        let before = set([
            assumption("z", &predicate("p"), None, None),
            assumption("a", &predicate("p"), None, None),
        ]);
        let after = set([
            assumption("a", &predicate("p"), None, None),
            assumption("z", &predicate("p"), None, None),
        ]);
        let changes = classify_assumptions(&before, &after);
        assert_eq!(
            changes
                .iter()
                .map(|c| c.unit().as_str())
                .collect::<Vec<_>>(),
            vec!["a", "z"]
        );
    }

    #[test]
    fn unchanged_units_are_included_by_default_and_filterable() {
        let before = set([
            assumption("a", &predicate("p"), None, None),
            assumption("b", &predicate("p"), None, None),
        ]);
        let after = set([
            assumption("a", &predicate("p"), None, None),
            assumption("b", &predicate("q"), None, None),
        ]);
        let changes = classify_assumptions(&before, &after);
        assert_eq!(changes.len(), 2, "total classification names every unit");
        let non_trivial: Vec<_> = changes
            .iter()
            .filter(|c| c.relation() != Relation::Unchanged)
            .collect();
        assert_eq!(non_trivial.len(), 1);
        assert_eq!(non_trivial[0].unit().as_str(), "b");
    }

    // --- the field-classification contract ------------------------------------------------------

    #[test]
    fn every_relation_this_module_can_produce_is_admissible_on_the_assumptions_field() {
        for relation in [
            Relation::Unchanged,
            Relation::Strengthened,
            Relation::Weakened,
            Relation::Added,
            Relation::Removed,
            Relation::Incomparable,
            Relation::Unknown,
        ] {
            assert!(
                PolicyField::Assumptions.admits_relation(relation),
                "{relation} must be admissible on `assumptions`"
            );
        }
    }

    #[test]
    fn to_classification_record_never_fails_for_a_relation_this_module_produced() {
        let before = set([assumption("A", &predicate("p"), None, None)]);
        let after = set([assumption(
            "A",
            &predicate("p"),
            Some(AssumptionClassification::Environment),
            None,
        )]);
        for change in classify_assumptions(&before, &after) {
            let record = change
                .to_classification_record()
                .expect("this module's own relations are always admissible on `assumptions`");
            assert_eq!(record.field(), PolicyField::Assumptions);
            assert_eq!(record.relation(), change.relation());
        }
    }

    #[test]
    fn the_live_schema_carries_a_fragment_member_nested_under_assumptions_expression() {
        // Backs the module doc's "`unsupported` is out of this module's reach"
        // claim: unlike `faults[]`/`fairness[]`/`observers[]`, this field's items
        // *do* carry a fragment (nested in `expression`), which is exactly why the
        // `unsupported` question is real for this field and is deferred rather than
        // dismissed.
        let schema = Json::parse(SCHEMA.as_bytes()).expect("the schema itself is canonical JSON");
        let assumptions_items = schema
            .as_object()
            .expect("schema root is an object")
            .get("properties")
            .and_then(Json::as_object)
            .expect("schema declares properties")
            .get("assumptions")
            .and_then(Json::as_object)
            .expect("schema declares assumptions")
            .get("items")
            .and_then(Json::as_object)
            .expect("assumptions is an array schema with items");
        let item_properties = assumptions_items
            .get("properties")
            .and_then(Json::as_object)
            .expect("assumptions items declare properties");
        // `assumptions[].expression` is a `$ref` to the shared `$defs/property_expression`
        // (the same carrier `claims[].expression` uses), not an inline object schema, so
        // the fragment member is looked up through the `$defs` entry rather than
        // `expression`'s own (nonexistent) `properties` key.
        assert!(
            item_properties.contains_key("expression"),
            "assumptions items lost their expression member"
        );
        let expression_properties = schema
            .as_object()
            .expect("schema root is an object")
            .get("$defs")
            .and_then(Json::as_object)
            .expect("schema declares $defs")
            .get("property_expression")
            .and_then(Json::as_object)
            .expect("schema declares $defs/property_expression")
            .get("properties")
            .and_then(Json::as_object)
            .expect("$defs/property_expression declares properties");
        assert!(
            expression_properties.contains_key("fragment"),
            "assumptions[].expression lost its `fragment` member; this module's \
             \"unsupported is out of reach\" reasoning depends on its presence and must be \
             revisited"
        );
    }

    // --- anti-vacuity mutants: the positive assertions are not vacuously true ------------------

    /// A plausible, *wrong* classifier: decides a same-key comparison purely by
    /// whether the expression's `ast` matches, never looking at `classification` or
    /// `fidelity_profile` at all.
    fn mutant_blind_to_declaredness(before: &Assumption, after: &Assumption) -> Relation {
        if before.expression().ast() == after.expression().ast() {
            Relation::Unchanged
        } else {
            Relation::Unknown
        }
    }

    #[test]
    fn negative_mutant_blind_to_declaredness_would_wrongly_pass_a_classification_change_as_unchanged()
     {
        let before = assumption("A", &predicate("p"), None, None);
        let after = assumption(
            "A",
            &predicate("p"),
            Some(AssumptionClassification::Trust),
            None,
        );

        let real = classify_pair(&before, &after);
        assert_eq!(real, Relation::Incomparable);

        let mutant_relation = mutant_blind_to_declaredness(&before, &after);
        assert_eq!(
            mutant_relation,
            Relation::Unchanged,
            "the mutant must actually get this wrong, or it is not exercising the bug"
        );
        assert_ne!(real, mutant_relation);
    }

    /// A plausible, *wrong* classifier: matches units by expression content instead
    /// of by `id`, so a rename with an unchanged expression reads as `unchanged`
    /// rather than `removed` + `added` — the exact disguise `properties`' text names.
    fn mutant_matches_by_expression_instead_of_id(
        before: &AssumptionSet,
        after: &AssumptionSet,
    ) -> bool {
        // `Formula` derives neither `Ord` nor `Hash`, so this compares as two small
        // multisets rather than through a `BTreeSet`/`HashSet` — adequate for a test
        // fixture with a handful of units.
        let before_asts: Vec<&Formula> = before.iter().map(|a| a.expression().ast()).collect();
        let after_asts: Vec<&Formula> = after.iter().map(|a| a.expression().ast()).collect();
        before_asts.len() == after_asts.len() && before_asts.iter().all(|b| after_asts.contains(b))
    }

    #[test]
    fn negative_mutant_matching_by_expression_would_wrongly_pass_a_rename_as_unchanged() {
        let before = set([assumption("A", &predicate("p"), None, None)]);
        let after = set([assumption("A-v2", &predicate("p"), None, None)]);

        let real_changes = classify_assumptions(&before, &after);
        assert!(
            real_changes
                .iter()
                .all(|c| c.relation() != Relation::Unchanged),
            "the real classifier never treats a rename as unchanged"
        );

        assert!(
            mutant_matches_by_expression_instead_of_id(&before, &after),
            "the mutant must actually get this wrong (report no change), or it is not \
             exercising the bug"
        );
    }
}
