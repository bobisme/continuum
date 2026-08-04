//! The field-level change policy: RFC 0037's closed verb set, its per-field
//! applicability matrix, and the enforcement that turns a classification into a
//! verdict (PR-4 / IMPL-09).
//!
//! # Three closed vocabularies, and which document owns each
//!
//! This module carries the Intent Contract's *enforcement* vocabulary. It is three
//! closed sets and one table relating them:
//!
//! | Vocabulary | Members | Type | Normative home |
//! |---|---|---|---|
//! | policy keys | fifteen | [`PolicyField`] | RFC 0037 "Fields"; the schema's `policy` object and `x-policy-field-map` |
//! | policy verbs | eight | [`PolicyVerb`] | RFC 0037 "Change policy"; the schema's `$defs/policy_verb` |
//! | classification relations | sixteen | [`Relation`] | RFC 0031 "Classification lattice"; `semantic-diff.schema.json`'s `relation` |
//! | verb ↦ field | the W6 matrix | [`PolicyField::admissible_verbs`] | RFC 0037 "Verb applicability"; the schema's five `$defs/policy_verb_*` restrictions |
//!
//! The fifteen policy keys are *also* RFC 0031's fifteen diff `field` members, and
//! the policy name is the diff name:
//!
//! > The fifteen diff `field` members are exactly the fifteen policy keys of the
//! > Intent Contract, related to the contract's field groups by the schema's
//! > normative `x-policy-field-map`. The diff `field` name is the *policy* name, not
//! > the contract path: a change under `claims` is reported as `properties`, a change
//! > under `fault_model` as `faults`, and a change under `optimization.non_vacuity`
//! > as `non_vacuity`.
//! >
//! > — `notes/plan/rfcs/0031-semantic-and-intent-diff.md`
//!
//! [`PolicyField::contract_path`] *is* that map, transcribed from the schema's
//! `x-policy-field-map` object, so a caller never has to guess which of the two names
//! it is holding.
//!
//! # Where the verb → field table comes from
//!
//! RFC 0037 states the applicability matrix as a table of five rows, and
//! `notes/plan/schemas/intent-contract.schema.json` states the same matrix
//! structurally as five `$ref`-plus-`enum` restrictions — `policy_verb_base`,
//! `policy_verb_bounds`, `policy_verb_trust_boundaries`, `policy_verb_assurance`, and
//! `policy_verb_removal` — one per row:
//!
//! > RFC 0037 F4, paid here. One enum applied to all fifteen keys let any verb
//! > validate on any field, so the W6 applicability matrix was prose: `no-decrease` on
//! > `properties` or `no-downgrade` outside `assurance` were schema-valid and had to
//! > be rejected by a checker. The per-field restrictions make an inapplicable verb
//! > unrepresentable, which is what W6 asks for — an inapplicable verb is rejected,
//! > never silently read as `unlocked`.
//! >
//! > — `intent-contract.schema.json`, `$defs/policy` `$comment`
//!
//! The two agree today, and `tests/change_policy_contract.rs` asserts the agreement
//! against both sources rather than trusting it: the five restriction sets are
//! transcribed there from the schema, and every one of the fifteen fields is checked
//! against the RFC's row for it. Where they could ever disagree, INV-003 decides — the
//! schema is the shape authority — and this module implements the schema's reading.
//!
//! # What "protected" means, and what the verb adds
//!
//! All fifteen fields are protected whatever their verb, and the verb is an
//! *additional* block:
//!
//! > **Protection is structural; the verb is additional.** All fifteen appear in
//! > `intent_changes` whatever their verb (RFC 0031: `protected` is `const: true`).
//! > `unlocked` does not mean "an ordinary task may edit this field": it means the
//! > verb contributes no further block to the verdict.
//! >
//! > — RFC 0037, "Change policy"
//!
//! So [`PolicyVerb::Unlocked`] is the bottom of the verb lattice and not an escape
//! from INV-001, and [`PolicyTable::all_unlocked`] is the *least restrictive* table
//! rather than an unprotected one.
//!
//! # Fail closed, in the one direction that matters
//!
//! Two rules point the same way, and both are implemented rather than documented:
//!
//! - An unrecognized verb token is rejected. "A reader that encounters an unrecognized
//!   token in any of those vocabularies MUST fail closed: it MUST reject the contract,
//!   and MUST NOT treat an unknown policy verb as `unlocked`" (RFC 0037).
//!   [`PolicyVerb::from_wire`] returns `None`, and [`PolicyTable::from_json`] turns
//!   that into [`ChangePolicyDecodeError::UnknownVerb`].
//! - A non-affirmative relation — `unknown`, `unsupported`, `incomparable` — never
//!   contributes `allow`, whatever the verb (P2). The fail-closed rule "is not a
//!   property of the directional verbs alone".
//!
//! # What is *not* here
//!
//! - **The classifier.** Producing a [`Relation`] for a field is RFC 0031's job and
//!   PR 12's code. This module consumes relations and produces a verdict; it never
//!   computes a direction. [`PolicyField::classified_relations`] is the per-field
//!   table a record is *checked* against, not a classifier.
//! - **P7's recomputation.** "The verdict MUST be recomputed at `intent.accept` time
//!   against the registry's current state, never reused from the diff produced at
//!   `intent.propose_revision` time." That is a daemon obligation (RFC 0026, PR 5);
//!   this module supplies a pure function of (table, records, reviewers, path), and a
//!   pure function is what makes recomputation cheap and reuse pointless.
//! - **Merging contracts.** [`PolicyTable::join`] is the per-field half of M3(d) and
//!   reports the fields whose join is unrepresentable; assembling the `conflict` node
//!   (M4) belongs to the merge procedure.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use crate::canonical_json::{Json, JsonError};
use crate::identity::canonical_identity;

// --- the fifteen policy keys ---------------------------------------------------------------

/// One of the fifteen protected-field policy keys.
///
/// The declaration order is the wire tokens' Unicode code-point order, which is the
/// key order of the schema's `policy` object and therefore ID5's. Deriving [`Ord`]
/// from it means a `BTreeMap<PolicyField, _>` iterates in exactly the order the
/// canonical encoding needs, so the encoder cannot sort differently from the
/// container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PolicyField {
    /// `abstraction_maps` — the §16 correspondence-map bindings.
    AbstractionMaps,
    /// `assumptions` — the declared environment.
    Assumptions,
    /// `assurance` — the demanded assurance level and checker requirements.
    Assurance,
    /// `bounds` — the verification envelope.
    Bounds,
    /// `completion_policy` — RFC 0015's terminal-state policy.
    CompletionPolicy,
    /// `fairness` — the fairness constraints.
    Fairness,
    /// `faults` — the fault model. Contract path `fault_model`.
    Faults,
    /// `non_vacuity` — INV-012's behaviors. Contract path `optimization.non_vacuity`.
    NonVacuity,
    /// `nondeterminism` — the declared nondeterminism sites.
    Nondeterminism,
    /// `observers` — the observers claims are stated against.
    Observers,
    /// `optimization` — hard constraints and soft objectives.
    Optimization,
    /// `properties` — the claims. Contract path `claims`.
    Properties,
    /// `scope` — components, fragments, and abstraction level.
    Scope,
    /// `security_policy` — classification, redaction, capability requirements.
    SecurityPolicy,
    /// `trust_boundaries` — the trusted and opaque sets.
    TrustBoundaries,
}

impl PolicyField {
    /// Every policy key, in wire code-point order.
    pub const ALL: [Self; 15] = [
        Self::AbstractionMaps,
        Self::Assumptions,
        Self::Assurance,
        Self::Bounds,
        Self::CompletionPolicy,
        Self::Fairness,
        Self::Faults,
        Self::NonVacuity,
        Self::Nondeterminism,
        Self::Observers,
        Self::Optimization,
        Self::Properties,
        Self::Scope,
        Self::SecurityPolicy,
        Self::TrustBoundaries,
    ];

    /// The wire literal: the schema's `policy` key and RFC 0031's diff `field`.
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            Self::AbstractionMaps => "abstraction_maps",
            Self::Assumptions => "assumptions",
            Self::Assurance => "assurance",
            Self::Bounds => "bounds",
            Self::CompletionPolicy => "completion_policy",
            Self::Fairness => "fairness",
            Self::Faults => "faults",
            Self::NonVacuity => "non_vacuity",
            Self::Nondeterminism => "nondeterminism",
            Self::Observers => "observers",
            Self::Optimization => "optimization",
            Self::Properties => "properties",
            Self::Scope => "scope",
            Self::SecurityPolicy => "security_policy",
            Self::TrustBoundaries => "trust_boundaries",
        }
    }

    /// Recover a policy key from its wire literal.
    ///
    /// Returns `None` for a token outside the closed fifteen — including a *contract
    /// path* that is not also a policy key, such as `claims` or `fault_model`. A
    /// policy table naming a field the contract does not have is rejected, never
    /// silently dropped.
    #[must_use]
    pub fn from_wire(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|field| field.wire() == token)
    }

    /// The contract path this policy key governs — the schema's `x-policy-field-map`.
    ///
    /// Three of the fifteen differ from the policy name, and those three are exactly
    /// the ones RFC 0037 correction 4 records: `properties ↦ claims`,
    /// `faults ↦ fault_model`, `non_vacuity ↦ optimization.non_vacuity`.
    #[must_use]
    pub const fn contract_path(self) -> &'static str {
        match self {
            Self::AbstractionMaps => "abstraction_maps",
            Self::Assumptions => "assumptions",
            Self::Assurance => "assurance",
            Self::Bounds => "bounds",
            Self::CompletionPolicy => "completion_policy",
            Self::Fairness => "fairness",
            Self::Faults => "fault_model",
            Self::NonVacuity => "optimization.non_vacuity",
            Self::Nondeterminism => "nondeterminism",
            Self::Observers => "observers",
            Self::Optimization => "optimization",
            Self::Properties => "claims",
            Self::Scope => "scope",
            Self::SecurityPolicy => "security_policy",
            Self::TrustBoundaries => "trust_boundaries",
        }
    }

    /// The verbs W6 admits on this field.
    ///
    /// The five rows of RFC 0037's applicability matrix, which are the schema's five
    /// `$defs/policy_verb_*` restrictions:
    ///
    /// | Policy key | Admissible verbs |
    /// |---|---|
    /// | `properties`, `assumptions`, `observers`, `abstraction_maps`, `scope`, `fairness`, `completion_policy`, `nondeterminism` | `unlocked`, `proposal-only`, `review`, `locked` |
    /// | `bounds` | the four above plus `no-decrease` |
    /// | `trust_boundaries` | the four above plus `no-expansion` |
    /// | `assurance` | the four above plus `no-downgrade` |
    /// | `faults`, `non_vacuity`, `optimization`, `security_policy` | the four above plus `no-removal` |
    ///
    /// The eight base-row fields have gaming directions the closed verb set contains
    /// no verb for — property `weakened`, observer `coarsened`, assumption and
    /// fairness `added` — and are protected through `review` or `locked` instead.
    #[must_use]
    pub const fn admissible_verbs(self) -> &'static [PolicyVerb] {
        const BASE: &[PolicyVerb] = &[
            PolicyVerb::Unlocked,
            PolicyVerb::ProposalOnly,
            PolicyVerb::Review,
            PolicyVerb::Locked,
        ];
        const BOUNDS: &[PolicyVerb] = &[
            PolicyVerb::Unlocked,
            PolicyVerb::ProposalOnly,
            PolicyVerb::Review,
            PolicyVerb::Locked,
            PolicyVerb::NoDecrease,
        ];
        const TRUST: &[PolicyVerb] = &[
            PolicyVerb::Unlocked,
            PolicyVerb::ProposalOnly,
            PolicyVerb::Review,
            PolicyVerb::Locked,
            PolicyVerb::NoExpansion,
        ];
        const ASSURANCE: &[PolicyVerb] = &[
            PolicyVerb::Unlocked,
            PolicyVerb::ProposalOnly,
            PolicyVerb::Review,
            PolicyVerb::Locked,
            PolicyVerb::NoDowngrade,
        ];
        const REMOVAL: &[PolicyVerb] = &[
            PolicyVerb::Unlocked,
            PolicyVerb::ProposalOnly,
            PolicyVerb::Review,
            PolicyVerb::Locked,
            PolicyVerb::NoRemoval,
        ];
        match self {
            Self::Bounds => BOUNDS,
            Self::TrustBoundaries => TRUST,
            Self::Assurance => ASSURANCE,
            Self::Faults | Self::NonVacuity | Self::Optimization | Self::SecurityPolicy => REMOVAL,
            Self::AbstractionMaps
            | Self::Assumptions
            | Self::CompletionPolicy
            | Self::Fairness
            | Self::Nondeterminism
            | Self::Observers
            | Self::Properties
            | Self::Scope => BASE,
        }
    }

    /// The relations RFC 0031's per-field table assigns to this field.
    ///
    /// This is the "Admissible relations" column of RFC 0031's "Field-by-field
    /// classification rules" table — the per-field order a classification is decided
    /// against, transcribed so a record can be checked rather than trusted. It is not
    /// the whole admissible set: see [`PolicyField::admits_relation`].
    #[must_use]
    pub const fn classified_relations(self) -> &'static [Relation] {
        const EXPRESSION: &[Relation] = &[
            Relation::Unchanged,
            Relation::Strengthened,
            Relation::Weakened,
            Relation::Added,
            Relation::Removed,
            Relation::Incomparable,
            Relation::Unsupported,
            Relation::Unknown,
        ];
        const OBSERVERS: &[Relation] = &[
            Relation::Unchanged,
            Relation::Refined,
            Relation::Coarsened,
            Relation::Added,
            Relation::Removed,
            Relation::Incomparable,
            Relation::Unsupported,
            Relation::Unknown,
        ];
        const MAPS: &[Relation] = &[
            Relation::Unchanged,
            Relation::Merged,
            Relation::Split,
            Relation::Added,
            Relation::Removed,
            Relation::Incomparable,
            Relation::Unknown,
        ];
        const SCOPE: &[Relation] = &[
            Relation::Unchanged,
            Relation::Added,
            Relation::Removed,
            Relation::Incomparable,
            Relation::Unknown,
        ];
        const ENVELOPE: &[Relation] = &[
            Relation::Unchanged,
            Relation::Expanded,
            Relation::Contracted,
            Relation::Incomparable,
            Relation::Unknown,
        ];
        const MEMBERSHIP: &[Relation] = &[Relation::Unchanged, Relation::Added, Relation::Removed];
        const COMPLETION: &[Relation] = &[Relation::Unchanged, Relation::Incomparable];
        const NONDETERMINISM: &[Relation] = &[
            Relation::Unchanged,
            Relation::Added,
            Relation::Removed,
            Relation::Incomparable,
        ];
        const ASSURANCE: &[Relation] = &[
            Relation::Unchanged,
            Relation::Upgraded,
            Relation::Downgraded,
            Relation::Added,
            Relation::Removed,
        ];
        const SECURITY: &[Relation] = &[
            Relation::Unchanged,
            Relation::Strengthened,
            Relation::Weakened,
            Relation::Added,
            Relation::Removed,
        ];
        match self {
            Self::Properties | Self::Assumptions | Self::Fairness => EXPRESSION,
            Self::Observers => OBSERVERS,
            Self::AbstractionMaps => MAPS,
            Self::Scope => SCOPE,
            Self::TrustBoundaries | Self::Bounds => ENVELOPE,
            Self::Faults | Self::Optimization | Self::NonVacuity => MEMBERSHIP,
            Self::CompletionPolicy => COMPLETION,
            Self::Nondeterminism => NONDETERMINISM,
            Self::Assurance => ASSURANCE,
            Self::SecurityPolicy => SECURITY,
        }
    }

    /// Whether a relation may be recorded for this field.
    ///
    /// [`classified_relations`](Self::classified_relations) plus the three
    /// non-affirmative relations — with one carve-out for `assurance`. RFC 0031's
    /// per-field table omitted `unknown` from five rows (`faults`, `assurance`,
    /// `optimization`, `non_vacuity`, `security_policy`), which read as forbidding it
    /// on exactly the fields whose set-membership or total order gives a classifier
    /// the least room to discharge an obligation, contradicting its own fail-closed
    /// rule:
    ///
    /// > A classifier that cannot complete MUST NOT emit a partial `intent_changes`
    /// > set with an `allow` decision. Failure to classify is `unknown` on every
    /// > unclassified protected field […] never silence.
    ///
    /// RFC 0031 correction 13 resolves that disagreement and states the one exception
    /// this function must also honor:
    ///
    /// > The per-field admissible-relation table omitted the non-affirmative relations
    /// > from five rows. […] Normative: the non-affirmative three are admissible on
    /// > every row (with `incomparable` excluded from `assurance` alone, per its own
    /// > total-order rule). Direction: this RFC's table is corrected to agree with its
    /// > own "Fail-closed rule" and "Policy verdict" P2, both already normative; found
    /// > by `crates/continuum-intent/src/change_policy.rs`
    /// > (`PolicyField::admits_relation`).
    /// >
    /// > — RFC 0031, "Corrections recorded by this RFC", correction 13
    ///
    /// `assurance` is RFC 0031's one totally ordered field: its three affirmative
    /// relations — `unchanged`, `upgraded`, `downgraded` — already dispose of every
    /// comparable pair, so there is no "both inclusions refuted" case left for
    /// `incomparable` to name. `unknown` stays admissible there for a different case
    /// entirely — the checker-flag declaredness change RFC 0031's "Assurance movement"
    /// and RFC 0037 correction 14 both fail closed to (`AssuranceComparisonError::
    /// CheckerDeclarednessChanged`) — so only `incomparable` is excluded, not the
    /// whole non-affirmative trio.
    ///
    /// This function previously implemented correction 13's general "admissible on
    /// every row" half but not its parenthetical `assurance` exclusion, so it admitted
    /// `Incomparable` on `assurance` regardless — over-admitting exactly the case the
    /// correction carves out. `bn-3vxp` found and pinned that gap
    /// (`crates/continuum-semantic-diff/src/assurance.rs`'s
    /// `admits_relation_now_refuses_incomparable_on_assurance_per_correction_13` test,
    /// née `admits_relation_currently_over_admits_incomparable_on_assurance_a_recorded_continuum_intent_gap`);
    /// this bone (bn-2sngz) closes it.
    #[must_use]
    pub fn admits_relation(self, relation: Relation) -> bool {
        if relation.is_affirmative() {
            return self.classified_relations().contains(&relation);
        }
        // Correction 13's parenthetical: every row admits the non-affirmative three
        // except `assurance`, which excludes `incomparable` alone (see the doc comment
        // above) — its own total-order rule leaves no "both inclusions refuted" case
        // for `incomparable` to name.
        !(matches!(self, Self::Assurance) && matches!(relation, Relation::Incomparable))
    }
}

impl fmt::Display for PolicyField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.wire())
    }
}

// --- the sixteen relations -----------------------------------------------------------------

/// One member of RFC 0031's closed classification lattice.
///
/// The sixteen tokens are `semantic-diff.schema.json`'s `relation` enum, in that
/// order. They live here rather than in a diff module because the verb table is
/// stated *in terms of them*: [`PolicyVerb::denied_relations`] cannot be typed without
/// them, and an untyped denial is a string comparison in the middle of the enforcement
/// path.
///
/// The classifier that *produces* these is RFC 0031's and is not in this crate. What
/// is here is the vocabulary, its fail-closed partition, and the per-field
/// admissibility check ([`PolicyField::admits_relation`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Relation {
    /// The canonical encodings of the classified unit are equal (R2).
    Unchanged,
    /// Fewer behaviors are admitted.
    Strengthened,
    /// More behaviors are admitted.
    Weakened,
    /// A set or envelope grew.
    Expanded,
    /// A set or envelope shrank.
    Contracted,
    /// An observer distinguishes at least as much.
    Refined,
    /// An observer distinguishes less.
    Coarsened,
    /// An abstraction map sends distinct concrete values to one abstract value.
    Merged,
    /// An abstraction map splits one abstract value.
    Split,
    /// An assurance requirement rose.
    Upgraded,
    /// An assurance requirement fell.
    Downgraded,
    /// A classified unit is present only on the after side.
    Added,
    /// A classified unit is present only on the before side.
    Removed,
    /// Both inclusions were refuted.
    Incomparable,
    /// The comparison is outside the declared fragments.
    Unsupported,
    /// No direction was established.
    Unknown,
}

impl Relation {
    /// Every relation, in the schema's enum order.
    pub const ALL: [Self; 16] = [
        Self::Unchanged,
        Self::Strengthened,
        Self::Weakened,
        Self::Expanded,
        Self::Contracted,
        Self::Refined,
        Self::Coarsened,
        Self::Merged,
        Self::Split,
        Self::Upgraded,
        Self::Downgraded,
        Self::Added,
        Self::Removed,
        Self::Incomparable,
        Self::Unsupported,
        Self::Unknown,
    ];

    /// The wire literal. Lowercase, per RFC 0031 correction 9 and RFC 0037
    /// correction 1.
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            Self::Unchanged => "unchanged",
            Self::Strengthened => "strengthened",
            Self::Weakened => "weakened",
            Self::Expanded => "expanded",
            Self::Contracted => "contracted",
            Self::Refined => "refined",
            Self::Coarsened => "coarsened",
            Self::Merged => "merged",
            Self::Split => "split",
            Self::Upgraded => "upgraded",
            Self::Downgraded => "downgraded",
            Self::Added => "added",
            Self::Removed => "removed",
            Self::Incomparable => "incomparable",
            Self::Unsupported => "unsupported",
            Self::Unknown => "unknown",
        }
    }

    /// Recover a relation from its wire literal.
    ///
    /// Returns `None` for an unrecognized token. RFC 0031: "A consumer that reads an
    /// unrecognized `relation` […] MUST fail closed: it MUST treat the record as a
    /// protected change with relation `unknown`", so the caller decides between
    /// rejecting the artifact and recording `unknown`; neither may read it as
    /// `unchanged`.
    #[must_use]
    pub fn from_wire(token: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|relation| relation.wire() == token)
    }

    /// Whether this relation asserts something: equality, or a direction.
    ///
    /// The three non-affirmative relations — `incomparable`, `unsupported`, `unknown`
    /// — assert only that no direction was established, and P2 makes them contribute
    /// at least `review` on every field under every verb.
    #[must_use]
    pub const fn is_affirmative(self) -> bool {
        !matches!(self, Self::Incomparable | Self::Unsupported | Self::Unknown)
    }

    /// Whether this relation is the equality relation.
    #[must_use]
    pub const fn is_unchanged(self) -> bool {
        matches!(self, Self::Unchanged)
    }
}

impl fmt::Display for Relation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.wire())
    }
}

// --- the eight verbs -----------------------------------------------------------------------

/// The authority that must accept a change under a verb.
///
/// > Verbs form a lattice under the product order on (denied relations, acceptance
/// > authority), with authority ordered
/// > `ordinary < proposal < named-reviewer < policy-amendment`.
/// >
/// > — RFC 0037, "Policy joins"
///
/// The declaration order is that order, and [`Ord`] is derived from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AcceptanceAuthority {
    /// Ordinary acceptance — still `revise-intent`, never an ordinary task.
    Ordinary,
    /// An agent may propose; the acceptance path decides.
    Proposal,
    /// A principal named in `policy_reviewers` must approve.
    NamedReviewer,
    /// A prior `intent.lock` amendment is required.
    PolicyAmendment,
}

impl AcceptanceAuthority {
    /// Every authority, weakest first.
    pub const ALL: [Self; 4] = [
        Self::Ordinary,
        Self::Proposal,
        Self::NamedReviewer,
        Self::PolicyAmendment,
    ];

    /// A stable name for diagnostics. Not a wire vocabulary: no schema carries it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ordinary => "ordinary",
            Self::Proposal => "proposal",
            Self::NamedReviewer => "named-reviewer",
            Self::PolicyAmendment => "policy-amendment",
        }
    }
}

impl fmt::Display for AcceptanceAuthority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One of the eight closed policy verbs.
///
/// > | Verb | Meaning | Denied relations | Acceptance authority |
/// > |---|---|---|---|
/// > | `unlocked` | ordinary edits allowed | none | ordinary |
/// > | `proposal-only` | agents may propose; humans accept | none | a human principal MUST perform `intent.accept` |
/// > | `review` | any change requires named-reviewer approval | none | approval by a principal named in `policy_reviewers` |
/// > | `locked` | no change without policy amendment | every non-`unchanged` relation | a prior `intent.lock` amendment |
/// > | `no-decrease` | changes classified as decrease (bounds) are blocked | `contracted` | ordinary |
/// > | `no-removal` | removals (faults, non-vacuity behaviors) are blocked | `removed` | ordinary |
/// > | `no-downgrade` | assurance downgrades are blocked | `downgraded` | ordinary |
/// > | `no-expansion` | growth of the set (opaque boundaries) is blocked | `expanded` | ordinary |
/// >
/// > — RFC 0037, "Change policy"
///
/// The set is closed. A ninth verb — `proof-required`, or a `no-addition` for the
/// fields whose gaming direction is `added` — is an open question in RFC 0037 and
/// requires a revision of it plus a `schema_epoch` advance, which is why
/// [`PolicyVerb::from_wire`] rejects rather than tolerates an unknown token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PolicyVerb {
    /// Ordinary edits allowed; the verb contributes no further block.
    Unlocked,
    /// Agents may propose; a human principal accepts.
    ProposalOnly,
    /// Any change requires approval by a named reviewer.
    Review,
    /// No change without a policy amendment.
    Locked,
    /// Blocks `contracted` — the bounds verb.
    NoDecrease,
    /// Blocks `removed` — the faults, non-vacuity, optimization, and security verb.
    NoRemoval,
    /// Blocks `downgraded` — the assurance verb.
    NoDowngrade,
    /// Blocks `expanded` — the trust-boundaries verb.
    NoExpansion,
}

impl PolicyVerb {
    /// Every verb, in the schema's `$defs/policy_verb` enum order.
    pub const ALL: [Self; 8] = [
        Self::Unlocked,
        Self::ProposalOnly,
        Self::Review,
        Self::Locked,
        Self::NoDecrease,
        Self::NoRemoval,
        Self::NoDowngrade,
        Self::NoExpansion,
    ];

    /// The wire literal.
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            Self::Unlocked => "unlocked",
            Self::ProposalOnly => "proposal-only",
            Self::Review => "review",
            Self::Locked => "locked",
            Self::NoDecrease => "no-decrease",
            Self::NoRemoval => "no-removal",
            Self::NoDowngrade => "no-downgrade",
            Self::NoExpansion => "no-expansion",
        }
    }

    /// Recover a verb from its wire literal.
    ///
    /// Returns `None` for an unrecognized token, and the caller rejects: a reader
    /// "MUST NOT treat an unknown policy verb as `unlocked`" (RFC 0037).
    #[must_use]
    pub fn from_wire(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|verb| verb.wire() == token)
    }

    /// The relations this verb denies.
    ///
    /// `locked` denies every relation except `unchanged` — P4: "`locked` contributes
    /// `block` for any non-`unchanged` relation" — which is what makes it the top of
    /// the lattice and what makes `locked ⊔ v = locked` for every `v`.
    #[must_use]
    pub const fn denied_relations(self) -> &'static [Relation] {
        const NONE: &[Relation] = &[];
        const EVERYTHING_BUT_UNCHANGED: &[Relation] = &[
            Relation::Strengthened,
            Relation::Weakened,
            Relation::Expanded,
            Relation::Contracted,
            Relation::Refined,
            Relation::Coarsened,
            Relation::Merged,
            Relation::Split,
            Relation::Upgraded,
            Relation::Downgraded,
            Relation::Added,
            Relation::Removed,
            Relation::Incomparable,
            Relation::Unsupported,
            Relation::Unknown,
        ];
        const CONTRACTED: &[Relation] = &[Relation::Contracted];
        const REMOVED: &[Relation] = &[Relation::Removed];
        const DOWNGRADED: &[Relation] = &[Relation::Downgraded];
        const EXPANDED: &[Relation] = &[Relation::Expanded];
        match self {
            Self::Unlocked | Self::ProposalOnly | Self::Review => NONE,
            Self::Locked => EVERYTHING_BUT_UNCHANGED,
            Self::NoDecrease => CONTRACTED,
            Self::NoRemoval => REMOVED,
            Self::NoDowngrade => DOWNGRADED,
            Self::NoExpansion => EXPANDED,
        }
    }

    /// Whether this verb denies `relation`.
    #[must_use]
    pub fn denies(self, relation: Relation) -> bool {
        self.denied_relations().contains(&relation)
    }

    /// The authority that must accept a change under this verb.
    #[must_use]
    pub const fn authority(self) -> AcceptanceAuthority {
        match self {
            Self::Unlocked
            | Self::NoDecrease
            | Self::NoRemoval
            | Self::NoDowngrade
            | Self::NoExpansion => AcceptanceAuthority::Ordinary,
            Self::ProposalOnly => AcceptanceAuthority::Proposal,
            Self::Review => AcceptanceAuthority::NamedReviewer,
            Self::Locked => AcceptanceAuthority::PolicyAmendment,
        }
    }

    /// Whether W6 admits this verb on `field`.
    #[must_use]
    pub fn is_admissible_on(self, field: PolicyField) -> bool {
        field.admissible_verbs().contains(&self)
    }

    /// The verb lattice's order: `v₁ ⊑ v₂` iff `denied(v₁) ⊆ denied(v₂)` and
    /// `authority(v₁) ≤ authority(v₂)`.
    #[must_use]
    pub fn is_weaker_or_equal(self, other: Self) -> bool {
        self.authority() <= other.authority()
            && self
                .denied_relations()
                .iter()
                .all(|relation| other.denies(*relation))
    }

    /// The least upper bound of two verbs, when the closed set names it.
    ///
    /// > The closed verb set is **not** closed under join. `no-decrease ⊔ review` is
    /// > `({contracted}, named-reviewer)`, which no closed-set verb names.
    /// >
    /// > A three-way merge (M3d) whose per-field policy join is unrepresentable MUST
    /// > produce a `conflict` node. Rounding up to `locked` is sound but silently
    /// > changes governance, so it MUST NOT be applied automatically; rounding down to
    /// > either input is unsound and MUST NOT be applied at all.
    /// >
    /// > — RFC 0037, "Policy joins"
    ///
    /// So this returns `None` rather than rounding in either direction, and the caller
    /// raises a conflict. The result is always the *least* upper bound, which is why
    /// "A merge MUST NOT produce a policy table strictly more permissive than either
    /// input on any field" holds by construction.
    ///
    /// RFC 0037 illustrates representability with "one side `unlocked` and the other
    /// anything, or two identical verbs". That illustration is narrower than the
    /// definition above it: `locked ⊔ v = locked` for every `v`, because `locked`
    /// denies every non-`unchanged` relation and holds the top authority, and
    /// `review ⊔ proposal-only = review`, because both deny nothing and
    /// `named-reviewer` is the greater authority. Both are computed here. *Raised as a
    /// flag against RFC 0037: the "Policy joins" illustration reads as an enumeration
    /// and is not one.*
    #[must_use]
    pub fn join(self, other: Self) -> Option<Self> {
        let denied: BTreeSet<Relation> = self
            .denied_relations()
            .iter()
            .chain(other.denied_relations())
            .copied()
            .collect();
        let authority = self.authority().max(other.authority());
        Self::ALL.into_iter().find(|verb| {
            verb.authority() == authority
                && verb.denied_relations().len() == denied.len()
                && verb
                    .denied_relations()
                    .iter()
                    .all(|relation| denied.contains(relation))
        })
    }
}

impl fmt::Display for PolicyVerb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.wire())
    }
}

// --- identity ------------------------------------------------------------------------------

canonical_identity! {
    /// The canonical identity of a change-policy artifact: the `policy` table, or the
    /// `policy_reviewers` map.
    ///
    /// As `crate::identity::CanonicalIdentity`, this type *is* the canonical preimage
    /// bytes rather than a digest of them (ADR-0013), so equality is canonical
    /// comparison and [`digest`](Self::digest) can only index. Both artifacts are
    /// inside the `in_*` preimage:
    ///
    /// > **ID2.** […] Nothing else is excluded — in particular the `policy` table,
    /// > `policy_reviewers`, `schema_id`, and `schema_epoch` are **in** the preimage.
    /// >
    /// > — RFC 0037, "Canonical identity"
    ///
    /// which is why ID3 records that `intent.lock` mints a *successor* contract: a
    /// governance edit moves the identity with all fifteen fields classified
    /// `unchanged`.
    ChangePolicyIdentity
}

// --- the policy table ----------------------------------------------------------------------

/// The `policy` object: one verb per protected field, all fifteen required.
///
/// Immutable, like every type in this crate. A governance change is a new table with a
/// new identity and, at the contract level, a new `in_*` (ID3); there is no setter,
/// because a setter is the affordance by which an ordinary operation edits a lock.
///
/// [`PartialEq`] compares the fifteen verbs and never the derived identity.
#[derive(Debug, Clone)]
pub struct PolicyTable {
    verbs: BTreeMap<PolicyField, PolicyVerb>,
    identity: ChangePolicyIdentity,
}

impl PolicyTable {
    /// Build a policy table, enforcing completeness and W6.
    ///
    /// # Errors
    ///
    /// - [`ChangePolicyError::DuplicateField`] — one key given twice.
    /// - [`ChangePolicyError::MissingField`] — the schema requires all fifteen keys.
    /// - [`ChangePolicyError::InapplicableVerb`] — W6: "Every `policy` value MUST be
    ///   well-formed for its field per the applicability matrix […] An inapplicable
    ///   verb is rejected, never silently treated as `unlocked`."
    pub fn new(
        entries: impl IntoIterator<Item = (PolicyField, PolicyVerb)>,
    ) -> Result<Self, ChangePolicyError> {
        let mut verbs: BTreeMap<PolicyField, PolicyVerb> = BTreeMap::new();
        for (field, verb) in entries {
            if !verb.is_admissible_on(field) {
                return Err(ChangePolicyError::InapplicableVerb { field, verb });
            }
            if verbs.insert(field, verb).is_some() {
                return Err(ChangePolicyError::DuplicateField { field });
            }
        }
        for field in PolicyField::ALL {
            if !verbs.contains_key(&field) {
                return Err(ChangePolicyError::MissingField { field });
            }
        }
        let identity = ChangePolicyIdentity::of_bytes(table_json(&verbs).to_canonical_bytes());
        Ok(Self { verbs, identity })
    }

    /// The least restrictive table: `unlocked` on all fifteen fields.
    ///
    /// Not an *unprotected* table. RFC 0037 correction 11: "A reading in which
    /// `unlocked` fields sit outside INV-001 is incorrect. […] all fifteen are
    /// classified, appear in `intent_changes`, and require `revise-intent` to accept;
    /// the verb governs only the additional block."
    #[must_use]
    pub fn all_unlocked() -> Self {
        Self::new(PolicyField::ALL.map(|field| (field, PolicyVerb::Unlocked)))
            .expect("`unlocked` is admissible on every field and all fifteen are named")
    }

    /// The verb governing `field`. Total: every field has one.
    #[must_use]
    pub fn verb(&self, field: PolicyField) -> PolicyVerb {
        *self
            .verbs
            .get(&field)
            .expect("a policy table names all fifteen fields by construction")
    }

    /// Every (field, verb) pair, in wire code-point order.
    pub fn iter(&self) -> impl Iterator<Item = (PolicyField, PolicyVerb)> {
        self.verbs.iter().map(|(field, verb)| (*field, *verb))
    }

    /// The table's canonical identity.
    #[must_use]
    pub const fn identity(&self) -> &ChangePolicyIdentity {
        &self.identity
    }

    /// The artifact-form JSON: the contract's `policy` object.
    #[must_use]
    pub fn artifact_json(&self) -> Json {
        table_json(&self.verbs)
    }

    /// The identity-preimage JSON.
    ///
    /// Identical to [`artifact_json`](Self::artifact_json): ID2's closed exclusion
    /// list has no member inside `policy`, and the table is named there as being *in*
    /// the preimage. The method exists so a contract-level preimage builder can call
    /// one name across all fifteen groups without knowing which of them differ.
    #[must_use]
    pub fn identity_preimage_json(&self) -> Json {
        self.artifact_json()
    }

    /// The artifact-form bytes, under the ID5 canonical-JSON rules.
    #[must_use]
    pub fn to_artifact_bytes(&self) -> Vec<u8> {
        self.artifact_json().to_canonical_bytes()
    }

    /// Decode a policy table from its artifact form.
    ///
    /// # Errors
    ///
    /// [`ChangePolicyDecodeError`], naming the key or the token that failed.
    pub fn decode(bytes: &[u8]) -> Result<Self, ChangePolicyDecodeError> {
        Self::from_json(&Json::parse(bytes)?)
    }

    /// Decode a policy table from an already-parsed JSON object.
    ///
    /// # Errors
    ///
    /// [`ChangePolicyDecodeError`], as [`PolicyTable::decode`].
    pub fn from_json(json: &Json) -> Result<Self, ChangePolicyDecodeError> {
        let fields = json
            .as_object()
            .ok_or_else(|| ChangePolicyDecodeError::TypeMismatch {
                field: "policy",
                expected: "object",
                found: json.type_name(),
            })?;
        let mut entries = Vec::with_capacity(PolicyField::ALL.len());
        for (key, value) in fields {
            // `policy` sets `additionalProperties: false`, so a key outside the
            // fifteen is an unknown *field*: a policy table naming a field the
            // contract does not have is rejected, never ignored.
            let field = PolicyField::from_wire(key).ok_or_else(|| {
                ChangePolicyDecodeError::UnknownField {
                    field: "policy",
                    key: key.clone(),
                }
            })?;
            let token = value
                .as_str()
                .ok_or_else(|| ChangePolicyDecodeError::TypeMismatch {
                    field: "policy.<field>",
                    expected: "string",
                    found: value.type_name(),
                })?;
            let verb = PolicyVerb::from_wire(token).ok_or_else(|| {
                ChangePolicyDecodeError::UnknownVerb {
                    field,
                    token: token.to_owned(),
                }
            })?;
            entries.push((field, verb));
        }
        Ok(Self::new(entries)?)
    }

    /// W7's second half, given the contract's `policy_reviewers`.
    ///
    /// > **W7 — Reviewers are named where required.** Every key of `policy_reviewers`
    /// > MUST be one of the fifteen policy keys, and every field whose verb is
    /// > `review` MUST have a non-empty principal list in `policy_reviewers`. A
    /// > `review` verb naming no reviewer is an unenforceable block.
    /// >
    /// > — RFC 0037
    ///
    /// The first half is structural: [`PolicyReviewers`] cannot hold a key outside the
    /// fifteen, and cannot hold an empty list. This method decides the half that
    /// relates the two objects, which the schema cannot state (RFC 0037 F3).
    ///
    /// # Errors
    ///
    /// [`WellFormednessError::UnenforceableReview`] for the first field, in wire
    /// order, whose `review` verb names no reviewer.
    pub fn check_reviewers(&self, reviewers: &PolicyReviewers) -> Result<(), WellFormednessError> {
        for (field, verb) in self.iter() {
            if verb == PolicyVerb::Review && reviewers.principals(field).is_none() {
                return Err(WellFormednessError::UnenforceableReview { field });
            }
        }
        Ok(())
    }

    /// The per-field policy join of two tables (M3d).
    ///
    /// # Errors
    ///
    /// [`PolicyJoinConflict`] naming every field whose join the closed verb set does
    /// not name. RFC 0037 M4: such a merge "MUST produce a `conflict` node", and
    /// neither rounding direction may be applied automatically.
    pub fn join(&self, other: &Self) -> Result<Self, PolicyJoinConflict> {
        let mut joined = Vec::with_capacity(PolicyField::ALL.len());
        let mut unrepresentable = Vec::new();
        for field in PolicyField::ALL {
            let (mine, theirs) = (self.verb(field), other.verb(field));
            match mine.join(theirs) {
                Some(verb) => joined.push((field, verb)),
                None => unrepresentable.push((field, mine, theirs)),
            }
        }
        if !unrepresentable.is_empty() {
            return Err(PolicyJoinConflict {
                fields: unrepresentable,
            });
        }
        // A join of two W6-admissible verbs is admissible on the same field: it is one
        // of the two inputs, or `locked`, which every field admits. The `Err` arm is
        // therefore unreachable — and it is spelled as a conflict rather than an
        // `unwrap`, because a governance decision must never be reached by a panic.
        Self::new(joined).map_err(|error| PolicyJoinConflict {
            fields: vec![(error.field(), PolicyVerb::Locked, PolicyVerb::Locked)],
        })
    }

    /// The RFC 0037 P1–P6 verdict for a classified revision.
    ///
    /// The computation, verbatim:
    ///
    /// > - **P1.** Classify all fifteen fields (RFC 0031). A partial classification
    /// >   MUST NOT reach a verdict.
    /// > - **P2.** For each record, if the relation is non-affirmative (`unknown`,
    /// >   `unsupported`, `incomparable`), the record contributes at least `review`,
    /// >   and contributes `block` when the field's verb is `locked`. This holds for
    /// >   every field and every verb — the fail-closed rule is not a property of the
    /// >   directional verbs alone.
    /// > - **P3.** Otherwise, if the relation is in the verb's denied set, the record
    /// >   contributes `block`.
    /// > - **P4.** Otherwise the verb's acceptance authority applies: `unlocked`
    /// >   contributes `allow`; `proposal-only` contributes `allow` only on a path
    /// >   where a human principal performs `intent.accept`, and `review` otherwise;
    /// >   `review` contributes `review` for any non-`unchanged` relation and names the
    /// >   reviewers from `policy_reviewers`; `locked` contributes `block` for any
    /// >   non-`unchanged` relation.
    /// > - **P5.** The verdict is the join of the record contributions in the total
    /// >   order `allow < review < block`.
    /// > - **P6.** `policy.reasons` MUST name the field and the relation of every
    /// >   record that forbade `allow`.
    ///
    /// P7 — recomputation at `intent.accept` time — is a daemon obligation and not this
    /// function's, but this function is what makes it cheap: it is pure in its four
    /// arguments, so recomputing costs nothing and caching buys nothing.
    ///
    /// # Errors
    ///
    /// - [`EnforcementError::IncompleteClassification`] — P1. A field with no record is
    ///   not "unchanged by omission": RFC 0031 permits omitting `unchanged` *records*
    ///   on the wire, but a verdict is computed from a complete classification, so the
    ///   caller supplies the `unchanged` records explicitly.
    /// - [`EnforcementError::UnenforceableReview`] — W7. A `review` verb with no named
    ///   principal cannot contribute a reviewable outcome, so the verdict is refused
    ///   rather than computed without it.
    /// - [`EnforcementError::InadmissibleRelation`] — a record carrying a relation the
    ///   field's order cannot produce.
    pub fn verdict(
        &self,
        records: &[ClassificationRecord],
        reviewers: &PolicyReviewers,
        path: AcceptancePath,
    ) -> Result<Verdict, EnforcementError> {
        // P1: a partial classification MUST NOT reach a verdict.
        let covered: BTreeSet<PolicyField> =
            records.iter().map(ClassificationRecord::field).collect();
        for field in PolicyField::ALL {
            if !covered.contains(&field) {
                return Err(EnforcementError::IncompleteClassification { field });
            }
        }
        // W7 before anything else: an unenforceable block is not an `allow`.
        self.check_reviewers(reviewers).map_err(
            |WellFormednessError::UnenforceableReview { field }| {
                EnforcementError::UnenforceableReview { field }
            },
        )?;
        let mut decision = PolicyDecision::Allow;
        let mut reasons = Vec::new();
        for record in records {
            let (field, relation) = (record.field(), record.relation());
            if !field.admits_relation(relation) {
                return Err(EnforcementError::InadmissibleRelation { field, relation });
            }
            let verb = self.verb(field);
            let contribution = if relation.is_affirmative() {
                if verb.denies(relation) {
                    // P3.
                    PolicyDecision::Block
                } else {
                    // P4.
                    match verb {
                        PolicyVerb::Unlocked
                        | PolicyVerb::NoDecrease
                        | PolicyVerb::NoRemoval
                        | PolicyVerb::NoDowngrade
                        | PolicyVerb::NoExpansion => PolicyDecision::Allow,
                        PolicyVerb::ProposalOnly => match path {
                            AcceptancePath::HumanAccept => PolicyDecision::Allow,
                            AcceptancePath::AgentAccept => PolicyDecision::Review,
                        },
                        PolicyVerb::Review => {
                            if relation.is_unchanged() {
                                PolicyDecision::Allow
                            } else {
                                PolicyDecision::Review
                            }
                        }
                        PolicyVerb::Locked => {
                            if relation.is_unchanged() {
                                PolicyDecision::Allow
                            } else {
                                PolicyDecision::Block
                            }
                        }
                    }
                }
            } else if verb == PolicyVerb::Locked {
                // P2.
                PolicyDecision::Block
            } else {
                PolicyDecision::Review
            };
            decision = decision.join(contribution);
            // P6: every record that forbade `allow`, and only those.
            if contribution > PolicyDecision::Allow {
                reasons.push(VerdictReason {
                    field,
                    relation,
                    verb,
                    contribution,
                    reviewers: reviewers.principals(field).cloned().unwrap_or_default(),
                });
            }
        }
        Ok(Verdict { decision, reasons })
    }
}

impl PartialEq for PolicyTable {
    fn eq(&self, other: &Self) -> bool {
        self.verbs == other.verbs
    }
}

impl Eq for PolicyTable {}

fn table_json(verbs: &BTreeMap<PolicyField, PolicyVerb>) -> Json {
    let mut fields = BTreeMap::new();
    for (field, verb) in verbs {
        fields.insert(
            field.wire().to_owned(),
            Json::String(verb.wire().to_owned()),
        );
    }
    Json::Object(fields)
}

/// Why a per-field policy join has no representative in the closed verb set.
///
/// Carries every offending field with the two verbs that were joined, because a merge
/// that reports only the first conflict makes the reviewer re-run the merge to find
/// the second.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyJoinConflict {
    fields: Vec<(PolicyField, PolicyVerb, PolicyVerb)>,
}

impl PolicyJoinConflict {
    /// The conflicting fields, each with the two verbs whose join is unrepresentable.
    #[must_use]
    pub fn fields(&self) -> &[(PolicyField, PolicyVerb, PolicyVerb)] {
        &self.fields
    }
}

impl fmt::Display for PolicyJoinConflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the policy join is unrepresentable in the closed verb set on ")?;
        for (index, (field, mine, theirs)) in self.fields.iter().enumerate() {
            if index > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{field} ({mine} joined with {theirs})")?;
        }
        f.write_str(
            "; rounding up to locked silently changes governance and rounding down is unsound, so \
             the merge is a conflict node (RFC 0037 M4)",
        )
    }
}

impl core::error::Error for PolicyJoinConflict {}

// --- reviewers -----------------------------------------------------------------------------

/// The `policy_reviewers` map: principals whose review satisfies the `review` verb.
///
/// Optional in the contract, so the empty map is a legal value
/// ([`PolicyReviewers::empty`]) and not a missing one. Two of W7's obligations are
/// structural here rather than checked later:
///
/// - a key outside the fifteen policy keys cannot be held — the schema states this as
///   `propertyNames: {enum: […]}`, so an offending key is an unknown *token* rather
///   than an unknown field, and [`PolicyReviewers::from_json`] reports it that way;
/// - an empty principal list cannot be held — the schema's `minItems: 1`, and W7's "A
///   `review` verb naming no reviewer is an unenforceable block".
///
/// The principal lists are stored as sets. The schema declares no `uniqueItems` on
/// them, so `["a","a"]` is schema-legal, and `policy_reviewers` is inside the `in_*`
/// preimage (ID2) — which means a repeated principal would otherwise give one reviewer
/// set two identities. The reader is liberal and the writer has one spelling, exactly
/// as for object key order ([`crate::canonical_json`]), so a repeat is normalized away
/// rather than rejected. *Raised as a flag against `intent-contract.schema.json`:
/// `policy_reviewers`'s principal arrays carry `minItems` but no `uniqueItems`, unlike
/// every other set-valued field in the contract.*
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyReviewers {
    by_field: BTreeMap<PolicyField, BTreeSet<String>>,
}

impl PolicyReviewers {
    /// Build a reviewer map.
    ///
    /// # Errors
    ///
    /// [`ChangePolicyError::EmptyReviewerList`] for a named field with no principals,
    /// and [`ChangePolicyError::DuplicateField`] for a field named twice.
    pub fn new(
        entries: impl IntoIterator<Item = (PolicyField, BTreeSet<String>)>,
    ) -> Result<Self, ChangePolicyError> {
        let mut by_field: BTreeMap<PolicyField, BTreeSet<String>> = BTreeMap::new();
        for (field, principals) in entries {
            if principals.is_empty() {
                return Err(ChangePolicyError::EmptyReviewerList { field });
            }
            if by_field.insert(field, principals).is_some() {
                return Err(ChangePolicyError::DuplicateField { field });
            }
        }
        Ok(Self { by_field })
    }

    /// The empty map — the reading of an absent `policy_reviewers` key.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            by_field: BTreeMap::new(),
        }
    }

    /// The principals named for `field`, or `None` when the field names none.
    ///
    /// `None` and an empty set are one state here, because an empty set cannot be
    /// held: W7 makes a named-but-empty list an unenforceable block, and the schema's
    /// `minItems: 1` rejects it.
    #[must_use]
    pub fn principals(&self, field: PolicyField) -> Option<&BTreeSet<String>> {
        self.by_field.get(&field)
    }

    /// Every (field, principals) pair, in wire code-point order.
    pub fn iter(&self) -> impl Iterator<Item = (PolicyField, &BTreeSet<String>)> {
        self.by_field.iter().map(|(field, names)| (*field, names))
    }

    /// Whether the map names nobody.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_field.is_empty()
    }

    /// How many fields the map names.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_field.len()
    }

    /// The artifact-form JSON: the contract's `policy_reviewers` object.
    #[must_use]
    pub fn artifact_json(&self) -> Json {
        let mut fields = BTreeMap::new();
        for (field, principals) in &self.by_field {
            fields.insert(
                field.wire().to_owned(),
                Json::Array(
                    principals
                        .iter()
                        .map(|name| Json::String(name.clone()))
                        .collect(),
                ),
            );
        }
        Json::Object(fields)
    }

    /// The identity-preimage JSON. Identical to the artifact form (ID2).
    #[must_use]
    pub fn identity_preimage_json(&self) -> Json {
        self.artifact_json()
    }

    /// The artifact-form bytes.
    #[must_use]
    pub fn to_artifact_bytes(&self) -> Vec<u8> {
        self.artifact_json().to_canonical_bytes()
    }

    /// The map's canonical identity.
    #[must_use]
    pub fn identity(&self) -> ChangePolicyIdentity {
        ChangePolicyIdentity::of_bytes(self.identity_preimage_json().to_canonical_bytes())
    }

    /// Decode a reviewer map from its artifact form.
    ///
    /// # Errors
    ///
    /// [`ChangePolicyDecodeError`].
    pub fn decode(bytes: &[u8]) -> Result<Self, ChangePolicyDecodeError> {
        Self::from_json(&Json::parse(bytes)?)
    }

    /// Decode a reviewer map from an already-parsed JSON object.
    ///
    /// # Errors
    ///
    /// [`ChangePolicyDecodeError`].
    pub fn from_json(json: &Json) -> Result<Self, ChangePolicyDecodeError> {
        let fields = json
            .as_object()
            .ok_or_else(|| ChangePolicyDecodeError::TypeMismatch {
                field: "policy_reviewers",
                expected: "object",
                found: json.type_name(),
            })?;
        let mut entries = Vec::new();
        for (key, value) in fields {
            // `propertyNames: {enum: […]}`, not `additionalProperties: false`: the key
            // itself is drawn from a closed vocabulary, so an offending key is an
            // unknown token.
            let field = PolicyField::from_wire(key).ok_or_else(|| {
                ChangePolicyDecodeError::UnknownToken {
                    field: "policy_reviewers",
                    token: key.clone(),
                }
            })?;
            let items = value
                .as_array()
                .ok_or_else(|| ChangePolicyDecodeError::TypeMismatch {
                    field: "policy_reviewers.<field>",
                    expected: "array",
                    found: value.type_name(),
                })?;
            let mut principals = BTreeSet::new();
            for item in items {
                let name = item
                    .as_str()
                    .ok_or_else(|| ChangePolicyDecodeError::TypeMismatch {
                        field: "policy_reviewers.<field>[]",
                        expected: "string",
                        found: item.type_name(),
                    })?;
                principals.insert(name.to_owned());
            }
            entries.push((field, principals));
        }
        Ok(Self::new(entries)?)
    }
}

// --- enforcement ---------------------------------------------------------------------------

/// One classified field change, as the verdict computation consumes it.
///
/// RFC 0031 emits "one record per classified unit that is not `unchanged`", and
/// `intent_changes` "MAY carry several records for one field". This type is one such
/// record reduced to what P2–P4 read: the field and the relation. The per-unit locator
/// the wire cannot carry today is RFC 0031's F1 and is not invented here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassificationRecord {
    field: PolicyField,
    relation: Relation,
}

impl ClassificationRecord {
    /// Record a classified change, checked against the field's order.
    ///
    /// # Errors
    ///
    /// [`ChangePolicyError::InadmissibleRelation`] when RFC 0031's per-field table
    /// does not assign this relation to this field — `downgraded` on `bounds`, say, or
    /// `merged` on `faults`. The non-affirmative three are admissible everywhere except
    /// `incomparable` on `assurance` (RFC 0031 correction 13); see
    /// [`PolicyField::admits_relation`].
    pub fn new(field: PolicyField, relation: Relation) -> Result<Self, ChangePolicyError> {
        if !field.admits_relation(relation) {
            return Err(ChangePolicyError::InadmissibleRelation { field, relation });
        }
        Ok(Self { field, relation })
    }

    /// The field this record classifies.
    #[must_use]
    pub const fn field(&self) -> PolicyField {
        self.field
    }

    /// The relation recorded.
    #[must_use]
    pub const fn relation(&self) -> Relation {
        self.relation
    }
}

/// Which acceptance path a proposed revision is on.
///
/// P4 makes `proposal-only` contribute `allow` "only on a path where a human principal
/// performs `intent.accept`, and `review` otherwise". Whether a human is performing the
/// acceptance is a fact about the caller's principal and capability (RFC 0027), not
/// about the contract, so it is a parameter of the verdict rather than something
/// derived from the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcceptancePath {
    /// A human principal performs `intent.accept`.
    HumanAccept,
    /// An agent performs it, or the accepting principal is not known to be human.
    AgentAccept,
}

/// The three-value policy decision.
///
/// Shared with the IDL's `PolicyDecision` and with `semantic-diff.schema.json`'s
/// `policy.decision`. RFC 0031 correction 1 records that there is no fourth `unknown`
/// value: "a decision value that is neither permissive nor blocking has no defined
/// promotion semantics; inconclusiveness is a property of the relation, and it fails
/// closed into `review` or `block`."
///
/// The declaration order is P5's total order `allow < review < block`, and [`Ord`] is
/// derived from it, so [`PolicyDecision::join`] is `max`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PolicyDecision {
    /// Ordinary promotion may proceed.
    Allow,
    /// The change requires review.
    Review,
    /// The change is blocked.
    Block,
}

impl PolicyDecision {
    /// Every decision, weakest first.
    pub const ALL: [Self; 3] = [Self::Allow, Self::Review, Self::Block];

    /// The wire literal.
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Review => "review",
            Self::Block => "block",
        }
    }

    /// Recover a decision from its wire literal.
    #[must_use]
    pub fn from_wire(token: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|decision| decision.wire() == token)
    }

    /// P5's join: the greater of the two in `allow < review < block`.
    #[must_use]
    pub fn join(self, other: Self) -> Self {
        self.max(other)
    }
}

impl fmt::Display for PolicyDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.wire())
    }
}

/// One record's contribution to a verdict, with everything P6 must name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerdictReason {
    field: PolicyField,
    relation: Relation,
    verb: PolicyVerb,
    contribution: PolicyDecision,
    reviewers: BTreeSet<String>,
}

impl VerdictReason {
    /// The field whose record forbade `allow`.
    #[must_use]
    pub const fn field(&self) -> PolicyField {
        self.field
    }

    /// The relation recorded for it.
    #[must_use]
    pub const fn relation(&self) -> Relation {
        self.relation
    }

    /// The verb that governed the field.
    #[must_use]
    pub const fn verb(&self) -> PolicyVerb {
        self.verb
    }

    /// What this record contributed: `review` or `block`.
    #[must_use]
    pub const fn contribution(&self) -> PolicyDecision {
        self.contribution
    }

    /// The principals whose review satisfies the field's verb, if any are named.
    #[must_use]
    pub const fn reviewers(&self) -> &BTreeSet<String> {
        &self.reviewers
    }
}

impl fmt::Display for VerdictReason {
    /// The `policy.reasons` rendering: field, relation, verb, contribution.
    ///
    /// P6: "Reasons are a rendering of typed records and MUST NOT be the only place a
    /// blocking fact appears (INV-003)." The typed record is [`VerdictReason`]; this is
    /// its rendering, and the accessors above are where the fact actually lives.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} classified {} under {} — {}",
            self.field, self.relation, self.verb, self.contribution
        )?;
        if !self.reviewers.is_empty() {
            f.write_str(" by ")?;
            for (index, name) in self.reviewers.iter().enumerate() {
                if index > 0 {
                    f.write_str(", ")?;
                }
                f.write_str(name)?;
            }
        }
        Ok(())
    }
}

/// A computed policy verdict: the P5 join and the P6 reasons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    decision: PolicyDecision,
    reasons: Vec<VerdictReason>,
}

impl Verdict {
    /// The decision.
    #[must_use]
    pub const fn decision(&self) -> PolicyDecision {
        self.decision
    }

    /// Every record that forbade `allow`, in the order the records were supplied.
    #[must_use]
    pub fn reasons(&self) -> &[VerdictReason] {
        &self.reasons
    }
}

// --- errors --------------------------------------------------------------------------------

/// Why a change policy is not well formed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangePolicyError {
    /// W6: the verb is not admissible on that field.
    InapplicableVerb {
        /// The field.
        field: PolicyField,
        /// The inapplicable verb.
        verb: PolicyVerb,
    },
    /// A policy key was supplied twice.
    DuplicateField {
        /// The repeated key.
        field: PolicyField,
    },
    /// The schema requires all fifteen policy keys and one is absent.
    MissingField {
        /// The absent key.
        field: PolicyField,
    },
    /// W7: a field is named in `policy_reviewers` with no principals.
    EmptyReviewerList {
        /// The field.
        field: PolicyField,
    },
    /// RFC 0031's per-field table does not assign this relation to this field.
    InadmissibleRelation {
        /// The field.
        field: PolicyField,
        /// The relation.
        relation: Relation,
    },
}

impl ChangePolicyError {
    /// The field the error is about. Every variant names one.
    #[must_use]
    pub const fn field(&self) -> PolicyField {
        match self {
            Self::InapplicableVerb { field, .. }
            | Self::DuplicateField { field }
            | Self::MissingField { field }
            | Self::EmptyReviewerList { field }
            | Self::InadmissibleRelation { field, .. } => *field,
        }
    }
}

impl fmt::Display for ChangePolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InapplicableVerb { field, verb } => write!(
                f,
                "the verb `{verb}` is not admissible on the policy field `{field}`; W6's \
                 applicability matrix admits {:?} there, and an inapplicable verb is rejected, \
                 never silently read as unlocked",
                field
                    .admissible_verbs()
                    .iter()
                    .map(|verb| verb.wire())
                    .collect::<Vec<_>>()
            ),
            Self::DuplicateField { field } => {
                write!(f, "the policy key `{field}` is named twice")
            }
            Self::MissingField { field } => write!(
                f,
                "the policy table does not name `{field}`; all fifteen protected fields carry a \
                 verb (intent-contract.schema.json, policy.required)"
            ),
            Self::EmptyReviewerList { field } => write!(
                f,
                "policy_reviewers names `{field}` with no principals; a review verb naming no \
                 reviewer is an unenforceable block (W7)"
            ),
            Self::InadmissibleRelation { field, relation } => write!(
                f,
                "the relation `{relation}` is not one RFC 0031's order for `{field}` produces; \
                 that field's table admits {:?}",
                field
                    .classified_relations()
                    .iter()
                    .map(|relation| relation.wire())
                    .collect::<Vec<_>>()
            ),
        }
    }
}

impl core::error::Error for ChangePolicyError {}

/// Why a byte string is not a change-policy artifact.
///
/// Every variant is a rejection, never a repair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangePolicyDecodeError {
    /// The bytes are not admissible canonical JSON.
    Json(JsonError),
    /// A field carries the wrong JSON type.
    TypeMismatch {
        /// The field's path.
        field: &'static str,
        /// What the schema admits.
        expected: &'static str,
        /// What was found.
        found: &'static str,
    },
    /// The `policy` object carries a key outside the fifteen
    /// (`additionalProperties: false`).
    UnknownField {
        /// The object's path.
        field: &'static str,
        /// The offending key.
        key: String,
    },
    /// A closed key vocabulary carries an unrecognized token — a `policy_reviewers`
    /// key outside the fifteen policy keys (`propertyNames`).
    UnknownToken {
        /// The object's path.
        field: &'static str,
        /// The unrecognized token.
        token: String,
    },
    /// A policy value is not one of the eight verbs.
    ///
    /// The fail-closed case RFC 0037 names outright: an unknown verb "MUST NOT [be
    /// treated] as `unlocked`".
    UnknownVerb {
        /// The field whose verb could not be read.
        field: PolicyField,
        /// The unrecognized token.
        token: String,
    },
    /// The decoded policy is not well formed.
    Policy(ChangePolicyError),
}

impl fmt::Display for ChangePolicyDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => error.fmt(f),
            Self::TypeMismatch {
                field,
                expected,
                found,
            } => write!(f, "`{field}` must be {expected}; found {found}"),
            Self::UnknownField { field, key } => write!(
                f,
                "`{field}` carries the unknown key {key:?}; the fifteen policy keys are the closed \
                 set, and a policy table naming a field the contract does not have is rejected, \
                 never ignored"
            ),
            Self::UnknownToken { field, token } => write!(
                f,
                "`{field}` carries the token {token:?}, which is not one of the fifteen policy \
                 keys (W7)"
            ),
            Self::UnknownVerb { field, token } => write!(
                f,
                "the policy verb {token:?} on `{field}` is not one of the closed eight; an unknown \
                 verb is rejected and MUST NOT be treated as unlocked (RFC 0037)"
            ),
            Self::Policy(error) => error.fmt(f),
        }
    }
}

impl core::error::Error for ChangePolicyDecodeError {}

impl From<JsonError> for ChangePolicyDecodeError {
    fn from(error: JsonError) -> Self {
        Self::Json(error)
    }
}

impl From<ChangePolicyError> for ChangePolicyDecodeError {
    fn from(error: ChangePolicyError) -> Self {
        Self::Policy(error)
    }
}

/// Why a policy table and a reviewer map are not well formed together.
///
/// Separate from [`ChangePolicyError`] because the rule is *relational*: it holds
/// between two contract keys and cannot be decided when either is decoded alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WellFormednessError {
    /// W7: a field's verb is `review` and no principal is named for it.
    UnenforceableReview {
        /// The field.
        field: PolicyField,
    },
}

impl fmt::Display for WellFormednessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnenforceableReview { field } => write!(
                f,
                "the field `{field}` carries the review verb and policy_reviewers names no \
                 principal for it; a review verb naming no reviewer is an unenforceable block (W7)"
            ),
        }
    }
}

impl core::error::Error for WellFormednessError {}

/// Why a verdict could not be computed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnforcementError {
    /// P1: a field carries no classification record.
    IncompleteClassification {
        /// The unclassified field.
        field: PolicyField,
    },
    /// W7: a `review` verb names no principal, so its block is unenforceable.
    UnenforceableReview {
        /// The field.
        field: PolicyField,
    },
    /// A record carries a relation the field's order cannot produce.
    InadmissibleRelation {
        /// The field.
        field: PolicyField,
        /// The relation.
        relation: Relation,
    },
}

impl fmt::Display for EnforcementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IncompleteClassification { field } => write!(
                f,
                "the field `{field}` carries no classification record; a partial classification \
                 MUST NOT reach a verdict (P1), and an omitted field is not an unchanged one"
            ),
            Self::UnenforceableReview { field } => write!(
                f,
                "the field `{field}` carries the review verb and no reviewer is named (W7); the \
                 verdict is refused rather than computed without the block"
            ),
            Self::InadmissibleRelation { field, relation } => write!(
                f,
                "the record classifies `{field}` as `{relation}`, which RFC 0031's order for that \
                 field does not produce"
            ),
        }
    }
}

impl core::error::Error for EnforcementError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_vocabularies_are_closed_and_round_trip_through_their_wire_tokens() {
        for field in PolicyField::ALL {
            assert_eq!(PolicyField::from_wire(field.wire()), Some(field));
        }
        for verb in PolicyVerb::ALL {
            assert_eq!(PolicyVerb::from_wire(verb.wire()), Some(verb));
        }
        for relation in Relation::ALL {
            assert_eq!(Relation::from_wire(relation.wire()), Some(relation));
        }
        assert_eq!(PolicyField::ALL.len(), 15);
        assert_eq!(PolicyVerb::ALL.len(), 8);
        assert_eq!(Relation::ALL.len(), 16);
    }

    #[test]
    fn the_policy_field_declaration_order_is_the_wire_code_point_order() {
        // The `BTreeMap<PolicyField, _>` encoders depend on this: if the derived `Ord`
        // disagreed with the wire order, the canonical encoding's keys would not be
        // sorted and ID5 would be violated by the container rather than by the writer.
        let declared: Vec<&str> = PolicyField::ALL.iter().map(|field| field.wire()).collect();
        let mut sorted = declared.clone();
        sorted.sort_unstable();
        assert_eq!(declared, sorted);
    }

    #[test]
    fn a_contract_path_is_not_always_the_policy_name() {
        assert_eq!(PolicyField::Properties.contract_path(), "claims");
        assert_eq!(PolicyField::Faults.contract_path(), "fault_model");
        assert_eq!(
            PolicyField::NonVacuity.contract_path(),
            "optimization.non_vacuity"
        );
        let renamed = PolicyField::ALL
            .into_iter()
            .filter(|field| field.contract_path() != field.wire())
            .count();
        assert_eq!(renamed, 3);
        // And a contract path is not a policy key, so it cannot be read as one.
        assert_eq!(PolicyField::from_wire("claims"), None);
        assert_eq!(PolicyField::from_wire("fault_model"), None);
    }

    #[test]
    fn locked_is_the_top_of_the_verb_lattice_and_unlocked_the_bottom() {
        for verb in PolicyVerb::ALL {
            assert!(PolicyVerb::Unlocked.is_weaker_or_equal(verb), "{verb}");
            assert!(verb.is_weaker_or_equal(PolicyVerb::Locked), "{verb}");
            assert_eq!(PolicyVerb::Unlocked.join(verb), Some(verb));
            assert_eq!(PolicyVerb::Locked.join(verb), Some(PolicyVerb::Locked));
            assert_eq!(verb.join(verb), Some(verb));
        }
    }

    #[test]
    fn the_join_the_rfc_names_unrepresentable_is_unrepresentable() {
        assert_eq!(PolicyVerb::NoDecrease.join(PolicyVerb::Review), None);
        assert_eq!(PolicyVerb::Review.join(PolicyVerb::NoDecrease), None);
        // The ones the definition represents but the RFC's illustration omits.
        assert_eq!(
            PolicyVerb::Review.join(PolicyVerb::ProposalOnly),
            Some(PolicyVerb::Review)
        );
    }
}
