//! `properties` field classification: RFC 0031's `properties` rule (PR-12 / IMPL-02).
//!
//! # What this module answers
//!
//! Given two [`ClaimSet`]s — an Intent Contract's `claims[]` before and after a
//! proposed revision — classify every unit into RFC 0031's closed relation
//! vocabulary:
//!
//! > | `properties` | `claims[]` | one claim, keyed by `id` | behavior-set inclusion
//! > over CPNF-1 | `unchanged`, `strengthened`, `weakened`, `added`, `removed`,
//! > `incomparable`, `unsupported`, `unknown` |
//! >
//! > — `notes/plan/rfcs/0031-semantic-and-intent-diff.md`, "Field-by-field
//! > classification rules"
//!
//! > ### `properties` (contract `claims`)
//! >
//! > Claims are keyed by `id`. A claim present on one side only classifies `added`
//! > or `removed`. A claim present on both sides is classified by comparing
//! > `expression` under CPNF-1: equal N8 encodings classify `unchanged` (RFC 0037
//! > S1); otherwise the direction is `strengthened` iff behaviors(after) ⊆
//! > behaviors(before), `weakened` iff behaviors(before) ⊆ behaviors(after),
//! > `incomparable` iff both inclusions are refuted, and `unknown` otherwise. A
//! > change to `kind` or to the bound `observer` with an unchanged expression is a
//! > change of what the claim means and MUST classify `incomparable`, never
//! > `unchanged`. Renaming an `id` while preserving the expression is `removed` plus
//! > `added`, not `unchanged`; the classifier MUST NOT match claims by expression to
//! > defeat the rename.
//! >
//! > — RFC 0031, "`properties` (contract `claims`)"
//!
//! # The dangerous direction is `weakened`
//!
//! > | Gaming move (plan §5.1) | Field | Relation | Blocking verb (plan §5.4) |
//! > |---|---|---|---|
//! > | exclude the failing state with a constraint | `properties` | `weakened` |
//! > `review` |
//! >
//! > — RFC 0031, "Completeness guarantee"
//!
//! A weakened claim covers more behaviors, so passing verification proves less than
//! the author intended — docs/50 lists "weaken property" first among the intent
//! attacks, and plan §5.3's completeness guarantee opens with "every property
//! weakening". This is the mirror image of [`crate::assumptions`]' `strengthened`
//! (that module's doc states the inversion); here nothing is dualized and nothing is
//! antitone — a formula weakened *is* the claim weakened.
//!
//! # One unit, three axes
//!
//! A `claims[]` unit carries three things a revision can move, and RFC 0031 gives
//! each its own rule:
//!
//! - **Membership** (`id`): a unit on one side only is [`Relation::Added`] /
//!   [`Relation::Removed`], by key alone — never matched by expression to defeat a
//!   rename.
//! - **Meaning** (`kind`, `observer`): moved with the expression held fixed, the
//!   unit is [`Relation::Incomparable`] — "a change of what the claim means", the
//!   RFC's own words, not a refutation of two inclusions. Moved *together with* the
//!   expression, the unit is [`Relation::Unknown`]: the RFC scopes its
//!   `incomparable` rule to "with an unchanged expression" and states no combined
//!   rule, the same silence [`crate::assumptions`]' `classification`/
//!   `fidelity_profile` bullet has, resolved the same way that module resolved it —
//!   fold the compound edit into the non-affirmative relation that already governs,
//!   never into a case split the text does not state. One consequence is
//!   deliberate and tested: **the implication oracle is never consulted across a
//!   meaning change**, so a `kind` flip can never ride an expression edit into an
//!   affirmative direction — a direction in this field's order is a statement about
//!   one kind of claim, and no order is defined between a `safety` claim and the
//!   `liveness` claim that replaced it.
//! - **Content** (`expression`): equal canonical encodings (`ast` plus `fragment`,
//!   the ID2 preimage — the display-only `source` never contributes, per R2 and
//!   "CPNF-1 interaction") classify [`Relation::Unchanged`]; unequal encodings go to
//!   the bounded implication oracle below, under the fragment license below.
//!
//! # The bounded, sound implication oracle
//!
//! START_HERE's own boundary for this PR: "Add solver-based implication only where
//! sound and bounded." Both words are load-bearing and both are structural here:
//!
//! - **Sound.** [`finite_formula_relation`] affirms a direction only when a
//!   syntactic entailment derivation proves the inclusion *valid in every
//!   interpretation* — which entails RFC 0031's "exact language inclusion over the
//!   bounded universe" for the `Finite` fragment a fortiori, and is therefore a
//!   discharge of S2's Finite implication obligation for exactly the cases it
//!   proves. Every inference rule is individually meaning-preserving (the list
//!   below), so a proved direction is a true direction; the derivation itself is the
//!   checkable witness, re-derivable by any implementation of the same closed rule
//!   set from the two ASTs alone.
//! - **Bounded.** The prover spends one unit of [`IMPLICATION_STEP_BUDGET`] per
//!   inference step and gives up — *not established*, never a guess — when the
//!   budget is exhausted. There is no iterative deepening, no saturation loop, and
//!   no external solver process; the search is a single structural recursion whose
//!   step count is capped by a compile-time constant, so an adversarial formula pair
//!   costs a bounded number of steps and then a `review`, never a hang.
//!
//! The closed rule set, each with its soundness argument:
//!
//! | Rule | Sound because |
//! |---|---|
//! | `P ⇒ P` (structural equality) | equal structure ⇒ equal N8 encoding ⇒ equal behaviors (S1) |
//! | `false ⇒ Q`, `P ⇒ true` | the boolean constants are the lattice extremes |
//! | `P ⇒ Q₁ ∧ … ∧ Qₙ` iff `P ⇒ Qᵢ` for all `i` | intersection is the greatest lower bound (invertible) |
//! | `P₁ ∨ … ∨ Pₙ ⇒ Q` iff `Pᵢ ⇒ Q` for all `i` | union is the least upper bound (invertible) |
//! | `P₁ ∧ … ∧ Pₙ ⇒ Q` if `Pᵢ ⇒ Q` for some `i` | a conjunction entails each conjunct |
//! | `P ⇒ Q₁ ∨ … ∨ Qₙ` if `P ⇒ Qᵢ` for some `i` | each disjunct entails the disjunction |
//! | `¬A ⇒ ¬B` if `B ⇒ A` | complement is antitone |
//! | `always A ⇒ always B`, `eventually A ⇒ eventually B`, if `A ⇒ B` | both operators are monotone in their operand |
//! | `∀x∈D. A ⇒ ∀x∈D. B` and `∃x∈D. A ⇒ ∃x∈D. B`, if `A ⇒ B`, *same binder* | both quantifiers are monotone in their body over a fixed domain; N5's de-Bruijn-level naming makes "same binder" a structural equality |
//!
//! Declined rules, each declined for a reason and none silently:
//!
//! - **Literal-integer comparison arithmetic** (`le(t, 3) ⇒ le(t, 5)`): `lt`/`le`
//!   relate terms of a model-declared sort, and no sort table exists —
//!   `continuum_intent::ast`'s own module doc records that N6's `not`-absorption
//!   over `lt`/`le` is already declined for exactly this reason ("only where the
//!   compared sort is declared totally ordered"). Affirming transitivity over an
//!   undeclared order would be the unsound half of the same assumption. Tested: a
//!   pure bound relaxation classifies `unknown`.
//! - **`always A ⇒ A`, `always A ⇒ eventually A`**: both depend on trace-model
//!   facts (non-emptiness, the state/trace level crossing) the dossier has not fixed
//!   for this classifier to cite.
//! - **Quantifier instantiation / `∀ ⇒ ∃`**: unsound over an empty domain, and
//!   domains are model constants this crate cannot inspect.
//! - **Distribution, tautology recognition, general propositional completeness**:
//!   valid but unlisted; S2 says an unrecognized equivalence costs a review, not an
//!   invariant, and every extension of the rule set is a classifier-version change
//!   under RFC 0030's query key.
//!
//! Two consequences of the rule set are stated rather than discovered:
//!
//! - **`incomparable` is never produced by the oracle.** A failed derivation search
//!   refutes nothing — the atoms it treats as opaque may be related in ways the
//!   rules cannot see — and RFC 0031 admits `incomparable` for an expression
//!   comparison only when *both inclusions are refuted*. This module has no
//!   refutation mechanism, so expression content alone never reaches
//!   `incomparable`; only the meaning axis (`kind`/`observer`) does, where the RFC
//!   assigns it directly. The same fact holds in [`crate::fairness`] and is stated
//!   there.
//! - **A pair proved in both directions classifies `unknown`.** Both of the RFC's
//!   `iff` conditions hold for a proven equivalence whose encodings differ, and a
//!   record carries one relation: choosing either direction would invent a
//!   preference the classification lattice does not define — the same
//!   one-record-one-direction reasoning RFC 0031 correction 16 applied to the
//!   fairness `kind`+`condition` collapse — and R2 forbids `unchanged` off
//!   encoding equality. So the arm fails closed, and the collision is flagged in
//!   the bone comment (bn-8mlg) as an RFC 0031 text gap (no `equivalent` token, two
//!   satisfied `iff`s) rather than resolved silently. No CPNF-1 pair is known to
//!   reach it — N3/N4 sort and dedup the junction shapes that would — so through
//!   [`classify_properties`] the arm is a soundness guard; it is reachable, and
//!   tested, through [`finite_formula_relation`]'s public raw-formula surface.
//!
//! # The fragment license
//!
//! S3: "The *directional* relations `strengthened` and `weakened` MUST NOT be
//! [taken from CPNF-1]: they require the fragment's own decision procedure", and
//! RFC 0031's open question keeps `Symbolic`, `Temporal`, and `Probabilistic` at
//! "`unchanged` and `unsupported`/`unknown` but no direction" until it closes. So a
//! direction is affirmed only when **both sides declare the same fragment and that
//! fragment is `Finite`** — the one fragment whose obligation a universally valid
//! entailment discharges under S2's own words. Everything else fails closed to
//! [`Relation::Unknown`] on unequal encodings:
//!
//! - both sides declare the same non-`Finite` fragment: that fragment's decision
//!   procedure is a discharged SMT/Lean obligation (RFC 0031's assurance table),
//!   which no in-process derivation is;
//! - the two sides declare *different* fragments (including declared on one side
//!   only): the unit changed which decision procedure it answers to, and no single
//!   fragment's order covers the pair — the encodings already differ, so
//!   `unchanged` is out, and no direction may be guessed;
//! - both sides leave `fragment` absent: W3 makes absence denote the intent's
//!   declared fragment, which is a statement about the *contract* — and a
//!   two-[`ClaimSet`] classifier is never given `scope.fragments`, exactly the gap
//!   [`crate::assumptions`]' module doc records for `unsupported`. Guessing
//!   `Finite` would affirm directions under a license nobody granted.
//!
//! # Out of this module's reach, honestly
//!
//! - **`unsupported`.** "Fragment attribution" needs the after-contract's
//!   `scope.fragments`, which this classifier is not given — the identical boundary
//!   [`crate::assumptions`] records, for the identical reason. Whatever assembles
//!   the full `diff_*` artifact holds that context.
//! - **The `requested_assurance` clamp.** RFC 0031's admissibility table makes
//!   expression-level directions inadmissible below `bounded` ("every
//!   expression-level direction classifies `unknown`" at `observed`/`sampled`).
//!   The directions this module affirms are witnessed by universally valid
//!   entailment and so are admissible at `bounded` and above; the clamp for a diff
//!   requested at a weaker level belongs to the artifact assembler that receives
//!   `requested_assurance`, which no family classifier models — the same boundary
//!   as `unsupported`, recorded rather than guessed at.
//! - **Witness attachment.** The wire record shape carries an `evidence` member
//!   (RFC 0031 F1's `{field, relation, protected, fragment, evidence}`);
//!   [`PropertyChange`] carries none, like every sibling, because the wire artifact
//!   is not one field-classifier's fence. The derivation is deterministic in the
//!   two ASTs, so the witness is recomputable wherever the record is assembled.
//!
//! # One formula authority, and who consumes it
//!
//! [`formula_relation`] (structural: equality ⇒ `unchanged`, anything else ⇒
//! `unknown`) and [`finite_formula_relation`] (the structural core plus the bounded
//! oracle, licensed for `Finite`) are the crate's **one** formula-comparison
//! authority, both closed over `continuum_intent::change_policy::Relation` — no
//! second vocabulary, no second order. RFC 0031 defines the order once, in this
//! field's section, and points `assumptions` ("exactly as for claims") and
//! `fairness[].condition` ("computed over CPNF-1 exactly as for claims") back at
//! it, which is why the authority lives here. [`crate::fairness`]'s same-key
//! `condition` comparison delegates its declared-pair arm to [`formula_relation`]
//! (behavior unchanged: fairness items declare no fragment, so no direction is
//! licensed there yet — its module doc records the upgrade path);
//! [`crate::assumptions`] consumes the authority through the shared license arm
//! [`expression_relation`] (`bn-2nwpg`, the follow-up this module's bone comment
//! recorded): its expressions carry the `fragment` a license needs, so its
//! single-axis content edits reach [`finite_formula_relation`] under the identical
//! `Finite` license — the oracle's relation consumed *directly*, with no dualize,
//! per RFC 0031's "exactly as for claims — the same order" (that module's doc
//! derives the polarity).
//!
//! # Scope: PR-12 / IMPL-02 only
//!
//! See [`crate::assumptions`]' module doc, "Scope: PR-12 / IMPL-03 only" — the same
//! boundary, for this bone (`bn-8mlg`): the whole `properties` field classification
//! and the formula authority above, and nothing else. The wire
//! `intent_changes[]`/`diff_*` assembly, the impact set, the P1 completeness a
//! verdict needs, and the rewiring of `assumptions`/`fairness` onto the oracle are
//! all later bones'.

use std::collections::BTreeSet;

use continuum_intent::ast::{Formula, Fragment};
use continuum_intent::change_policy::{
    ChangePolicyError, ClassificationRecord, PolicyField, Relation,
};
use continuum_intent::property::{Claim, ClaimSet, PropertyExpression, UnitKey};

/// How many inference steps one implication derivation may spend.
///
/// The bound that makes the oracle *bounded* in START_HERE's sense: every
/// recursive entailment call spends one step, exhaustion is "not established" —
/// which fails closed to [`Relation::Unknown`] at the relation level — and each of
/// the two directions of one comparison gets its own budget, so neither direction
/// can starve the other and the answer cannot depend on which is asked first.
///
/// The value is a compile-time constant, not a tunable: raising it changes which
/// pairs classify with a direction, which is a classifier-version change under RFC
/// 0030's query key and RFC 0031's "Determinism", never a deployment knob. It is
/// generous for every contract-shaped formula in the corpus (the fixture's largest
/// claim costs under a hundred steps per direction) and small enough that the
/// exhaustion path is a tested, observable behavior rather than a theoretical one.
pub const IMPLICATION_STEP_BUDGET: usize = 4096;

/// One classified `claims[]` unit: a [`UnitKey`] and the [`Relation`] RFC 0031
/// assigns it.
///
/// The relation is always one [`PolicyField::Properties`] admits — pinned by this
/// module's tests — because [`classify_properties`] only ever produces
/// [`Relation::Unchanged`], [`Relation::Strengthened`], [`Relation::Weakened`],
/// [`Relation::Added`], [`Relation::Removed`], [`Relation::Incomparable`], or
/// [`Relation::Unknown`], all seven of which are on the properties row.
/// `unsupported` is admissible there too but never emitted here — see the module
/// doc, "Out of this module's reach, honestly".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyChange {
    unit: UnitKey,
    relation: Relation,
}

impl PropertyChange {
    fn new(unit: UnitKey, relation: Relation) -> Self {
        debug_assert!(
            PolicyField::Properties.admits_relation(relation),
            "a property classification produced {relation}, which the properties row does not admit"
        );
        Self { unit, relation }
    }

    /// The classified unit: RFC 0031's per-unit locator for `properties`, "keyed by
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
    /// relation this module produces is on the properties row (see the struct doc),
    /// but [`ClassificationRecord::new`] is itself fallible and a `debug_assert!`
    /// is not a proof, so the `Result` is threaded rather than unwrapped.
    pub fn to_classification_record(&self) -> Result<ClassificationRecord, ChangePolicyError> {
        ClassificationRecord::new(PolicyField::Properties, self.relation)
    }
}

/// Classify every `claims[]` unit between `before` and `after`, per RFC 0031's
/// "`properties`" rule.
///
/// Total: one [`PropertyChange`] for every unit key declared in `before`, in
/// `after`, or both — including [`Relation::Unchanged`] units. RFC 0031 permits a
/// wire artifact to omit `unchanged` records; that is an emission choice for
/// whatever assembles `intent_changes[]`, not a property of the classifier —
/// matching every sibling's identical choice for the same reason.
///
/// A unit present on one side only classifies [`Relation::Added`] /
/// [`Relation::Removed`] by key alone — "the classifier MUST NOT match claims by
/// expression to defeat the rename". A unit present on both sides is classified by
/// [`classify_pair`].
///
/// Units are visited in [`UnitKey`] order (the union of both sides' unit keys), so
/// the output order is deterministic and independent of either input's authored
/// array order (RFC 0031, "Determinism").
#[must_use]
pub fn classify_properties(before: &ClaimSet, after: &ClaimSet) -> Vec<PropertyChange> {
    let mut units: BTreeSet<UnitKey> = before.iter().map(|c| c.unit().clone()).collect();
    units.extend(after.iter().map(|c| c.unit().clone()));
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
            PropertyChange::new(unit, relation)
        })
        .collect()
}

/// Whether two [`PropertyExpression`]s share the same canonical encoding: equal
/// `ast` and equal `fragment`, ignoring the display-only `source` entirely.
///
/// Deliberately narrower than [`PropertyExpression`]'s derived [`PartialEq`], which
/// is *document* equality and includes `source`; R2 and "CPNF-1 interaction" both
/// require the semantic reading for a classification decision. Identical to
/// `crate::assumptions`' private helper of the same name, over the same two fields,
/// for the same shared carrier type.
#[must_use]
fn expression_unchanged(before: &PropertyExpression, after: &PropertyExpression) -> bool {
    before.ast() == after.ast() && before.fragment() == after.fragment()
}

/// RFC 0031's per-unit comparison for one claim present on both sides.
///
/// The meaning axis first, then the content axis (module doc, "One unit, three
/// axes"):
///
/// - meaning fixed, expression's canonical encoding equal: [`Relation::Unchanged`]
///   — R2, and nothing weaker.
/// - meaning moved (`kind` or `observer`), expression equal:
///   [`Relation::Incomparable`] — the RFC's verbatim rule.
/// - meaning moved *and* expression moved: [`Relation::Unknown`] — the compound
///   case the RFC does not state, folded into the non-affirmative relation that
///   already governs; the oracle is never consulted across a meaning change.
/// - meaning fixed, expression moved: [`expression_relation`] — the fragment
///   license, then the bounded oracle.
///
/// A `debug_assert_eq!` cross-checks [`expression_unchanged`] against
/// [`PropertyExpression::identity_preimage_json`] equality on every call, the same
/// discipline `crate::assumptions::classify_pair` applies.
#[must_use]
fn classify_pair(before: &Claim, after: &Claim) -> Relation {
    let meaning_moved = before.class() != after.class() || before.observer() != after.observer();
    let unchanged = expression_unchanged(before.expression(), after.expression());
    debug_assert_eq!(
        unchanged,
        before.expression().identity_preimage_json() == after.expression().identity_preimage_json(),
        "expression_unchanged and identity_preimage_json equality must agree for unit {}",
        before.unit(),
    );
    match (meaning_moved, unchanged) {
        (false, true) => Relation::Unchanged,
        (true, true) => Relation::Incomparable,
        (true, false) => Relation::Unknown,
        (false, false) => expression_relation(before.expression(), after.expression()),
    }
}

/// The content-axis relation for two expressions whose canonical encodings differ:
/// the fragment license (module doc, "The fragment license"), then
/// [`finite_formula_relation`].
///
/// Reached only from a content-moved, single-axis arm — [`classify_pair`]'s
/// meaning-fixed arm here, and [`crate::assumptions`]' declaredness-fixed arm
/// (`bn-2nwpg`), which shares this function so the fragment license is written
/// once and cannot drift between the two consumers ("exactly as for claims — the
/// same order" is RFC 0031's own pointer back at this field's rule). The
/// `Unchanged` arm of the oracle is unreachable from here (equal fragments plus
/// unequal encodings entail unequal ASTs); it exists on the authority function
/// because that function is also a public surface.
#[must_use]
pub(crate) fn expression_relation(
    before: &PropertyExpression,
    after: &PropertyExpression,
) -> Relation {
    if before.fragment() != after.fragment() {
        // The unit changed which decision procedure it answers to; no single
        // fragment's order covers the pair.
        return Relation::Unknown;
    }
    match before.fragment() {
        Some(Fragment::Finite) => finite_formula_relation(before.ast(), after.ast()),
        // A non-`Finite` fragment's obligation is a discharged SMT/Lean obligation,
        // which no in-process derivation is; an absent fragment denotes the
        // contract's declared fragment, which this classifier is never given.
        _ => Relation::Unknown,
    }
}

/// The structural formula relation: the crate's one formula-comparison authority at
/// its license-free core.
///
/// Equal structure — which entails equal N8 encoding and therefore equal behaviors
/// (S1) — is [`Relation::Unchanged`]; anything else is [`Relation::Unknown`],
/// because unequal encodings carry no direction by themselves (S2, R3) and no
/// direction may be affirmed without a fragment license. This is the comparison
/// [`crate::fairness`] delegates its declared-condition pair to: sound on any
/// input, normalized or not, since structural equality implies semantic equality
/// regardless of normal form — on non-CPNF input it merely under-approximates
/// equality, which is the safe side.
#[must_use]
pub fn formula_relation(before: &Formula, after: &Formula) -> Relation {
    if before == after {
        Relation::Unchanged
    } else {
        Relation::Unknown
    }
}

/// The `Finite`-fragment formula relation: the structural core plus the bounded,
/// sound implication oracle (module doc, "The bounded, sound implication oracle").
///
/// - Equal structure: [`Relation::Unchanged`] — R2's only admissible basis.
/// - `behaviors(after) ⊆ behaviors(before)` derived, and not the converse:
///   [`Relation::Strengthened`].
/// - `behaviors(before) ⊆ behaviors(after)` derived, and not the converse:
///   [`Relation::Weakened`].
/// - Both derived (a proven equivalence under unequal encodings): fails closed to
///   [`Relation::Unknown`] — one record carries one direction, and R2 forbids
///   `unchanged` here; see the module doc for the correction-16-style reasoning
///   and the flag.
/// - Neither derived — including by budget exhaustion: [`Relation::Unknown`],
///   never a guess and never `incomparable` (a failed derivation refutes nothing).
///
/// Callers hold the `Finite` license before asking for a direction;
/// [`classify_properties`] does so via the declared `fragment` on both sides.
/// Inputs are expected in CPNF-1 (every [`Claim`] carries one by construction);
/// every rule is meaning-preserving on arbitrary well-formed formulas, so a
/// non-normal input degrades completeness, never soundness.
///
/// The two directions are derived under two independent budgets, in a fixed order,
/// so the answer is deterministic and neither direction's search can starve the
/// other.
#[must_use]
pub fn finite_formula_relation(before: &Formula, after: &Formula) -> Relation {
    if before == after {
        return Relation::Unchanged;
    }
    let mut strengthened_budget = IMPLICATION_STEP_BUDGET;
    let strengthened = entails(after, before, &mut strengthened_budget);
    let mut weakened_budget = IMPLICATION_STEP_BUDGET;
    let weakened = entails(before, after, &mut weakened_budget);
    match (strengthened, weakened) {
        // A proven equivalence with unequal encodings: both of RFC 0031's `iff`
        // conditions hold and a record carries one relation, so neither direction
        // may be preferred and R2 forbids `unchanged` — fail closed (module doc).
        (true, true) => Relation::Unknown,
        (true, false) => Relation::Strengthened,
        (false, true) => Relation::Weakened,
        (false, false) => Relation::Unknown,
    }
}

/// The derivation search: `true` iff the closed rule set proves that `premise`
/// entails `conclusion` in every interpretation, within the remaining budget.
///
/// `false` means *not established* — by rule exhaustion or by budget exhaustion —
/// and is never a refutation. Each call spends one step of `fuel` before doing
/// anything, so the total step count of one derivation search is bounded by the
/// budget the caller supplies, and recursion depth is bounded by the same number.
///
/// The invertible rules (conclusion-conjunction, premise-disjunction) run before
/// the non-invertible choices (premise-conjunct projection, conclusion-disjunct
/// selection), so no proof is lost to trying a committing rule first; the
/// congruences close the search. The order is fixed and the operand walks are
/// left-to-right, so the search — and its fuel consumption — is deterministic.
fn entails(premise: &Formula, conclusion: &Formula, fuel: &mut usize) -> bool {
    if *fuel == 0 {
        return false;
    }
    *fuel -= 1;
    if premise == conclusion {
        return true;
    }
    // The boolean extremes: `false` entails everything, everything entails `true`.
    if matches!(premise, Formula::Boolean { value: false })
        || matches!(conclusion, Formula::Boolean { value: true })
    {
        return true;
    }
    // Invertible: a conjunction is entailed iff every conjunct is.
    if let Formula::And { operands } = conclusion {
        return operands.iter().all(|q| entails(premise, q, fuel));
    }
    // Invertible: a disjunction entails iff every disjunct does.
    if let Formula::Or { operands } = premise {
        return operands.iter().all(|p| entails(p, conclusion, fuel));
    }
    // Choice: a conjunction entails whatever any conjunct entails.
    if let Formula::And { operands } = premise
        && operands.iter().any(|p| entails(p, conclusion, fuel))
    {
        return true;
    }
    // Choice: a disjunction is entailed by whatever entails any disjunct.
    if let Formula::Or { operands } = conclusion
        && operands.iter().any(|q| entails(premise, q, fuel))
    {
        return true;
    }
    match (premise, conclusion) {
        // Complement is antitone.
        (Formula::Not { operand: a }, Formula::Not { operand: b }) => entails(b, a, fuel),
        // `always` and `eventually` are monotone in their operand.
        (Formula::Always { operand: a }, Formula::Always { operand: b })
        | (Formula::Eventually { operand: a }, Formula::Eventually { operand: b }) => {
            entails(a, b, fuel)
        }
        // Quantifiers are monotone in their body over a fixed binder. N5 names
        // binders by de Bruijn level, so "the same binder" is structural equality
        // of variable and domain, and body-to-body references stay aligned.
        (
            Formula::Forall {
                binder: before_binder,
                body: a,
            },
            Formula::Forall {
                binder: after_binder,
                body: b,
            },
        )
        | (
            Formula::Exists {
                binder: before_binder,
                body: a,
            },
            Formula::Exists {
                binder: after_binder,
                body: b,
            },
        ) if before_binder == after_binder => entails(a, b, fuel),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use continuum_intent::ast::{Binder, ComparisonOperator, Identifier, Literal, Term};
    use continuum_intent::canonical_json::Json;
    use continuum_intent::property::ClaimKind;

    use super::*;

    /// The dossier's validated Intent Contract schema, included at compile time —
    /// used here to pin that `claims[]` items really do carry the three axes this
    /// module classifies (`kind`, `observer`, and `expression` with a nested
    /// `fragment`), so the meaning rule and the fragment license are grounded in
    /// the live schema rather than in this module's reading of it.
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

    fn claim(id: &str, ast: &Formula) -> Claim {
        Claim::new(
            unit(id),
            ClaimKind::Safety,
            expression(ast, Some(Fragment::Finite)),
            None,
        )
    }

    fn set(claims: impl IntoIterator<Item = Claim>) -> ClaimSet {
        ClaimSet::from_claims(claims).expect("distinct unit keys in a test fixture")
    }

    fn relation_of(changes: &[PropertyChange], id: &str) -> Relation {
        changes
            .iter()
            .find(|change| change.unit().as_str() == id)
            .unwrap_or_else(|| panic!("no record for unit {id:?} in {changes:?}"))
            .relation()
    }

    fn and(operands: Vec<Formula>) -> Formula {
        Formula::and(operands).expect("two or more operands")
    }

    fn or(operands: Vec<Formula>) -> Formula {
        Formula::or(operands).expect("two or more operands")
    }

    // --- membership -------------------------------------------------------------------------

    #[test]
    fn a_claim_present_only_after_classifies_added() {
        let before = set([claim("C-base", &predicate("p"))]);
        let after = set([
            claim("C-base", &predicate("p")),
            claim("C-new", &predicate("q")),
        ]);
        let changes = classify_properties(&before, &after);
        assert_eq!(changes.len(), 2);
        assert_eq!(relation_of(&changes, "C-new"), Relation::Added);
        assert_eq!(relation_of(&changes, "C-base"), Relation::Unchanged);
    }

    #[test]
    fn a_claim_present_only_before_classifies_removed() {
        let before = set([
            claim("C-base", &predicate("p")),
            claim("C-old", &predicate("q")),
        ]);
        let after = set([claim("C-base", &predicate("p"))]);
        let changes = classify_properties(&before, &after);
        assert_eq!(relation_of(&changes, "C-old"), Relation::Removed);
    }

    #[test]
    fn a_rename_with_a_preserved_expression_is_removed_plus_added_never_unchanged() {
        // "Renaming an `id` while preserving the expression is `removed` plus
        // `added`, not `unchanged`; the classifier MUST NOT match claims by
        // expression to defeat the rename."
        let before = set([claim("C-1", &predicate("p"))]);
        let after = set([claim("C-2", &predicate("p"))]);
        let changes = classify_properties(&before, &after);
        assert_eq!(changes.len(), 2, "{changes:?}");
        assert_eq!(relation_of(&changes, "C-1"), Relation::Removed);
        assert_eq!(relation_of(&changes, "C-2"), Relation::Added);
    }

    // --- R2: equality, and nothing weaker ---------------------------------------------------

    #[test]
    fn identical_sets_classify_every_unit_unchanged() {
        let claims = set([claim("C-1", &predicate("p")), claim("C-2", &predicate("q"))]);
        let changes = classify_properties(&claims, &claims);
        assert_eq!(changes.len(), 2);
        assert!(changes.iter().all(|c| c.relation() == Relation::Unchanged));
    }

    #[test]
    fn a_source_only_rewrite_classifies_unchanged() {
        // "the display-only `source` rendering MUST NOT contribute to identity or
        // to any classification."
        let before = set([Claim::new(
            unit("C-1"),
            ClaimKind::Safety,
            PropertyExpression::normalized(
                &predicate("p"),
                Some(Fragment::Finite),
                Some("p holds".to_owned()),
            )
            .expect("normalizes"),
            None,
        )]);
        let after = set([Claim::new(
            unit("C-1"),
            ClaimKind::Safety,
            PropertyExpression::normalized(
                &predicate("p"),
                Some(Fragment::Finite),
                Some("an entirely different English rendering".to_owned()),
            )
            .expect("normalizes"),
            None,
        )]);
        let changes = classify_properties(&before, &after);
        assert_eq!(relation_of(&changes, "C-1"), Relation::Unchanged);
    }

    #[test]
    fn a_cpnf_rewrite_of_the_same_property_classifies_unchanged() {
        // S1's other half: the N1–N7 rewrites are not changes. The same
        // implication, authored as `implies` and as its expanded contrapositive
        // spelling, normalizes to one AST and classifies `unchanged`.
        let authored = Formula::always(Formula::implies(predicate("p"), predicate("q")));
        let rewritten = Formula::always(Formula::not(and(vec![
            predicate("p"),
            Formula::not(predicate("q")),
        ])));
        let before = set([claim("C-1", &authored)]);
        let after = set([claim("C-1", &rewritten)]);
        let changes = classify_properties(&before, &after);
        assert_eq!(relation_of(&changes, "C-1"), Relation::Unchanged);
    }

    // --- the meaning axis: `kind` and `observer` --------------------------------------------

    #[test]
    fn a_kind_change_with_an_unchanged_expression_classifies_incomparable() {
        let before = set([claim("C-1", &predicate("p"))]);
        let after = set([Claim::new(
            unit("C-1"),
            ClaimKind::Liveness,
            expression(&predicate("p"), Some(Fragment::Finite)),
            None,
        )]);
        let changes = classify_properties(&before, &after);
        assert_eq!(relation_of(&changes, "C-1"), Relation::Incomparable);
    }

    #[test]
    fn an_observer_rebinding_with_an_unchanged_expression_classifies_incomparable() {
        let before = set([claim("C-1", &predicate("p"))]);
        let after = set([Claim::new(
            unit("C-1"),
            ClaimKind::Safety,
            expression(&predicate("p"), Some(Fragment::Finite)),
            Some("O-ledger".to_owned()),
        )]);
        let changes = classify_properties(&before, &after);
        assert_eq!(relation_of(&changes, "C-1"), Relation::Incomparable);
    }

    #[test]
    fn a_meaning_change_never_lets_the_oracle_affirm_a_direction() {
        // The compound case: the expression edit alone is a strengthening the
        // oracle decides (adding a conjunct), but `kind` moved in the same
        // revision, so the unit is `unknown` — never `strengthened`. A direction
        // in this field's order is a statement about one kind of claim.
        let strengthened_only = set([claim("C-1", &and(vec![predicate("p"), predicate("q")]))]);
        let before = set([claim("C-1", &predicate("p"))]);
        assert_eq!(
            relation_of(&classify_properties(&before, &strengthened_only), "C-1"),
            Relation::Strengthened,
            "the expression edit alone must be decidable, or this test is vacuous"
        );

        let compound = set([Claim::new(
            unit("C-1"),
            ClaimKind::Liveness,
            expression(
                &and(vec![predicate("p"), predicate("q")]),
                Some(Fragment::Finite),
            ),
            None,
        )]);
        assert_eq!(
            relation_of(&classify_properties(&before, &compound), "C-1"),
            Relation::Unknown
        );
    }

    // --- the content axis: decided directions in `Finite` -----------------------------------

    #[test]
    fn adding_a_conjunct_classifies_strengthened() {
        let before = set([claim("C-1", &predicate("p"))]);
        let after = set([claim("C-1", &and(vec![predicate("p"), predicate("q")]))]);
        let changes = classify_properties(&before, &after);
        assert_eq!(relation_of(&changes, "C-1"), Relation::Strengthened);
    }

    #[test]
    fn dropping_a_conjunct_classifies_weakened() {
        let before = set([claim("C-1", &and(vec![predicate("p"), predicate("q")]))]);
        let after = set([claim("C-1", &predicate("p"))]);
        let changes = classify_properties(&before, &after);
        assert_eq!(relation_of(&changes, "C-1"), Relation::Weakened);
    }

    #[test]
    fn adding_a_disjunct_classifies_weakened() {
        // The gaming shape: "exclude the failing state with a constraint" — the
        // claim now also holds wherever the new disjunct does.
        let before = set([claim("C-1", &or(vec![predicate("p"), predicate("q")]))]);
        let after = set([claim(
            "C-1",
            &or(vec![predicate("p"), predicate("q"), predicate("excused")]),
        )]);
        let changes = classify_properties(&before, &after);
        assert_eq!(relation_of(&changes, "C-1"), Relation::Weakened);
    }

    #[test]
    fn dropping_a_disjunct_classifies_strengthened() {
        let before = set([claim(
            "C-1",
            &or(vec![predicate("p"), predicate("q"), predicate("r")]),
        )]);
        let after = set([claim("C-1", &or(vec![predicate("p"), predicate("q")]))]);
        let changes = classify_properties(&before, &after);
        assert_eq!(relation_of(&changes, "C-1"), Relation::Strengthened);
    }

    #[test]
    fn a_direction_is_derived_through_temporal_and_quantifier_congruence() {
        // The monotone congruences at work: a conjunct dropped *inside*
        // `always(forall …)` is still a weakening.
        let binder = || {
            Binder::new(
                ident("e"),
                Term::Constant {
                    name: ident("Events"),
                },
            )
        };
        let event_predicate =
            |name: &str| Formula::predicate(ident(name), vec![Term::Var { name: ident("e") }]);
        let before = set([claim(
            "C-1",
            &Formula::always(Formula::forall(
                binder(),
                and(vec![event_predicate("logged"), event_predicate("durable")]),
            )),
        )]);
        let after = set([claim(
            "C-1",
            &Formula::always(Formula::forall(binder(), event_predicate("logged"))),
        )]);
        let changes = classify_properties(&before, &after);
        assert_eq!(relation_of(&changes, "C-1"), Relation::Weakened);
    }

    // --- the content axis: fail-closed boundaries -------------------------------------------

    #[test]
    fn unrelated_atoms_classify_unknown_never_a_guess() {
        let before = set([claim("C-1", &predicate("p"))]);
        let after = set([claim("C-1", &predicate("q"))]);
        let changes = classify_properties(&before, &after);
        assert_eq!(relation_of(&changes, "C-1"), Relation::Unknown);
    }

    #[test]
    fn literal_bound_relaxation_classifies_unknown_because_no_sort_table_exists() {
        // `le(x, 3)` → `le(x, 5)` is a weakening over the integers, and the module
        // declines to know it: `lt`/`le` relate a model-declared sort with no
        // declared order (the same reason `continuum_intent::ast` declines N6's
        // `not`-absorption there). The honest answer is `unknown`.
        let bound = |limit: i64| {
            Formula::compare(
                ComparisonOperator::Le,
                Term::State {
                    name: ident("depth"),
                    indices: Vec::new(),
                },
                Term::Literal {
                    value: Literal::Integer(limit),
                },
            )
        };
        let before = set([claim("C-1", &bound(3))]);
        let after = set([claim("C-1", &bound(5))]);
        let changes = classify_properties(&before, &after);
        assert_eq!(relation_of(&changes, "C-1"), Relation::Unknown);
    }

    #[test]
    fn the_oracle_never_emits_incomparable_and_the_meaning_axis_is_its_only_source() {
        // A failed derivation search refutes nothing, so expression content never
        // reaches `incomparable`; every non-affirmative content answer is
        // `unknown`.
        for (before_ast, after_ast) in [
            (predicate("p"), predicate("q")),
            (
                and(vec![predicate("p"), predicate("q")]),
                or(vec![predicate("r"), predicate("s")]),
            ),
        ] {
            assert_eq!(
                finite_formula_relation(&before_ast, &after_ast),
                Relation::Unknown
            );
        }
    }

    // --- the fragment license ---------------------------------------------------------------

    #[test]
    fn the_same_decidable_edit_is_directed_in_finite_and_unknown_in_temporal() {
        // Anti-vacuity pairing: the identical AST edit, decided under the `Finite`
        // license and failed closed under `Temporal`, so the license check is
        // demonstrably the thing deciding.
        let before_ast = and(vec![predicate("p"), predicate("q")]);
        let after_ast = predicate("p");
        let finite = classify_properties(
            &set([claim("C-1", &before_ast)]),
            &set([claim("C-1", &after_ast)]),
        );
        assert_eq!(relation_of(&finite, "C-1"), Relation::Weakened);

        let temporal_claim = |ast: &Formula| {
            Claim::new(
                unit("C-1"),
                ClaimKind::Safety,
                expression(ast, Some(Fragment::Temporal)),
                None,
            )
        };
        let temporal = classify_properties(
            &set([temporal_claim(&before_ast)]),
            &set([temporal_claim(&after_ast)]),
        );
        assert_eq!(relation_of(&temporal, "C-1"), Relation::Unknown);
    }

    #[test]
    fn a_fragment_move_with_an_identical_ast_is_never_unchanged() {
        let before = set([claim("C-1", &predicate("p"))]);
        let after = set([Claim::new(
            unit("C-1"),
            ClaimKind::Safety,
            expression(&predicate("p"), Some(Fragment::Temporal)),
            None,
        )]);
        let changes = classify_properties(&before, &after);
        assert_eq!(relation_of(&changes, "C-1"), Relation::Unknown);
    }

    #[test]
    fn an_undeclared_fragment_licenses_no_direction() {
        // W3 makes absence denote the contract's declared fragment, which this
        // classifier is never given — so even a decidable-shaped edit fails
        // closed.
        let unfragmented =
            |ast: &Formula| Claim::new(unit("C-1"), ClaimKind::Safety, expression(ast, None), None);
        let changes = classify_properties(
            &set([unfragmented(&and(vec![predicate("p"), predicate("q")]))]),
            &set([unfragmented(&predicate("p"))]),
        );
        assert_eq!(relation_of(&changes, "C-1"), Relation::Unknown);
    }

    // --- the both-directions collision arm ---------------------------------------------------

    #[test]
    fn a_pair_proved_in_both_directions_fails_closed_to_unknown() {
        // Reachable only through the raw-formula surface: CPNF-1's N4 sorts
        // junction operands, so no `ClaimSet` carries this pair. The two orderings
        // are structurally unequal, each direction is derivable, and the answer is
        // the refusal the module doc records — never a preferred direction and
        // never `unchanged` off encoding equality.
        let one = and(vec![predicate("p"), predicate("q")]);
        let other = and(vec![predicate("q"), predicate("p")]);
        assert_ne!(one, other);
        assert_eq!(finite_formula_relation(&one, &other), Relation::Unknown);
        assert_eq!(finite_formula_relation(&other, &one), Relation::Unknown);
    }

    // --- boundedness ------------------------------------------------------------------------

    #[test]
    fn the_step_budget_is_the_boundary_between_decided_and_fail_closed() {
        // One shape, two sizes: a disjunction extended by one disjunct. Small, the
        // derivation completes well inside the budget and the relation is
        // `weakened`; large, the identical derivation would need more steps than
        // the budget grants, and the relation fails closed to `unknown`. The
        // construction is linear — a flat vector of distinct atoms, no doubling.
        let disjunction = |n: usize| or((0..n).map(|i| predicate(&format!("p{i:04}"))).collect());
        let small_before = disjunction(8);
        let small_after = or((0..8)
            .map(|i| predicate(&format!("p{i:04}")))
            .chain([predicate("extra")])
            .collect());
        assert_eq!(
            finite_formula_relation(&small_before, &small_after),
            Relation::Weakened
        );

        let large_before = disjunction(400);
        let large_after = or((0..400)
            .map(|i| predicate(&format!("p{i:04}")))
            .chain([predicate("extra")])
            .collect());
        assert_eq!(
            finite_formula_relation(&large_before, &large_after),
            Relation::Unknown
        );
    }

    // --- the structural authority ------------------------------------------------------------

    #[test]
    fn the_structural_relation_is_equality_or_unknown_and_nothing_else() {
        assert_eq!(
            formula_relation(&predicate("p"), &predicate("p")),
            Relation::Unchanged
        );
        // Even a pair the oracle decides: the structural authority carries no
        // license and affirms nothing.
        assert_eq!(
            formula_relation(&predicate("p"), &or(vec![predicate("p"), predicate("q")])),
            Relation::Unknown
        );
    }

    // --- the field-classification contract ---------------------------------------------------

    #[test]
    fn every_relation_this_module_can_produce_is_admissible_on_the_properties_field() {
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
                PolicyField::Properties.admits_relation(relation),
                "{relation} must be admissible on `properties`"
            );
        }
    }

    #[test]
    fn to_classification_record_never_fails_for_a_relation_this_module_produced() {
        let before = set([
            claim("C-directed", &and(vec![predicate("p"), predicate("q")])),
            claim("C-opaque", &predicate("x")),
            claim("C-gone", &predicate("z")),
        ]);
        let after = set([
            claim("C-directed", &predicate("p")),
            claim("C-opaque", &predicate("y")),
            claim("C-new", &predicate("w")),
        ]);
        for change in classify_properties(&before, &after) {
            let record = change
                .to_classification_record()
                .expect("this module's own relations are always admissible on `properties`");
            assert_eq!(record.field(), PolicyField::Properties);
            assert_eq!(record.relation(), change.relation());
        }
    }

    // --- determinism (INV-005) ---------------------------------------------------------------

    #[test]
    fn two_independent_classifications_of_one_revision_are_identical() {
        let before = || {
            set([
                claim("C-a", &and(vec![predicate("p"), predicate("q")])),
                claim("C-b", &predicate("x")),
            ])
        };
        let after = || set([claim("C-a", &predicate("p")), claim("C-c", &predicate("y"))]);
        assert_eq!(
            classify_properties(&before(), &after()),
            classify_properties(&before(), &after())
        );
    }

    // --- the live schema still carries the three axes ---------------------------------------

    #[test]
    fn the_live_schema_carries_kind_observer_and_a_nested_fragment_on_claims_items() {
        let schema = Json::parse(SCHEMA.as_bytes()).expect("the schema itself is canonical JSON");
        let claims_items = schema
            .as_object()
            .expect("schema root is an object")
            .get("properties")
            .and_then(Json::as_object)
            .expect("schema declares properties")
            .get("claims")
            .and_then(Json::as_object)
            .expect("schema declares claims")
            .get("items")
            .and_then(Json::as_object)
            .expect("claims is an array schema with items");
        let item_properties = claims_items
            .get("properties")
            .and_then(Json::as_object)
            .expect("claims items declare properties");
        for axis in ["kind", "observer", "expression"] {
            assert!(
                item_properties.contains_key(axis),
                "claims[] lost its `{axis}` member; this module's meaning/content split \
                 must be revisited"
            );
        }
        let expression = schema
            .as_object()
            .and_then(|root| root.get("$defs"))
            .and_then(Json::as_object)
            .and_then(|defs| defs.get("property_expression"))
            .and_then(Json::as_object)
            .and_then(|def| def.get("properties"))
            .and_then(Json::as_object)
            .expect("$defs/property_expression declares properties");
        assert!(
            expression.contains_key("fragment"),
            "property_expression lost its `fragment` member; the fragment license \
             must be revisited"
        );
    }

    // --- anti-vacuity mutant: presence is not content ---------------------------------------

    /// A plausible, *wrong* classifier: decides a same-key comparison purely by
    /// whether the key is present on both sides, never looking at the expression.
    fn mutant_blind_to_expression_content(before: &ClaimSet, after: &ClaimSet) -> Relation {
        let key = unit("C-1");
        match (before.get(&key).is_some(), after.get(&key).is_some()) {
            (true, true) => Relation::Unchanged,
            _ => Relation::Unknown,
        }
    }

    #[test]
    fn negative_mutant_blind_to_expression_content_would_wrongly_pass_a_weakening_as_unchanged() {
        let before = set([claim("C-1", &and(vec![predicate("p"), predicate("q")]))]);
        let after = set([claim("C-1", &predicate("p"))]);

        let real = relation_of(&classify_properties(&before, &after), "C-1");
        assert_eq!(real, Relation::Weakened);

        let mutant_relation = mutant_blind_to_expression_content(&before, &after);
        assert_eq!(
            mutant_relation,
            Relation::Unchanged,
            "the mutant must actually get this wrong, or it is not exercising the bug"
        );
        assert_ne!(real, mutant_relation);
    }
}
