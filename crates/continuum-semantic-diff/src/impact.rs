//! Impact-set computation: RFC 0031's "Impact set" (PR-12, `bn-7ek41`).
//!
//! # What this module answers
//!
//! Given the set of intent fields a revision changed, whether the program diff
//! touched a correspondence link, and the evidence artifacts in scope — every
//! artifact that names either intent identity or either snapshot — partition the
//! artifacts into RFC 0031's three wire sets:
//!
//! > The diff MUST emit `invalidated` / `reused` / `unknown` evidence sets computed
//! > against the incremental database's dependency classes (RFC 0030). Evidence
//! > whose dependency on a changed field is `unknown` goes to `unknown`, never to
//! > `reused`.
//! >
//! > - The three sets MUST be pairwise disjoint, and their union MUST cover every
//! >   evidence artifact that names either intent identity or either snapshot. An
//! >   artifact in none of the three is a completeness failure, not an implicit
//! >   reuse.
//! > - RFC 0030's tri-state independence rule applies unchanged: `Unknown` is
//! >   dependent. Heuristic independence is permitted only on `Experimental` edges,
//! >   and an `Experimental` reuse MUST NOT support promotion.
//! >
//! > — `notes/plan/rfcs/0031-semantic-and-intent-diff.md`, "Impact set"
//!
//! # The vocabulary is RFC 0030's, restated here because PR 23 has not landed
//!
//! The dependency-reason set ([`DependencyReason`], nine members), the reuse-edge
//! class set ([`ReuseEdgeClass`], four members), and the independence tri-state
//! ([`Independence`]) are RFC 0030's closed sets, spelled token for token from its
//! "Semantic dependency reasons", "Reuse-edge classes", and "Independence and the
//! invalidation cone" sections. `crates/continuum-incremental` — the crate that
//! will own the incremental database — is a PR-23 scaffold with no types yet, so
//! this module carries the RFC 0031-side rendering of those sets: enough to state
//! the impact partition over recorded dependency edges, no more. When PR 23 lands
//! the database, its edges are what a daemon feeds [`compute_impact`]; the closed
//! sets then exist twice and RFC 0030's own rule for that shape applies ("a
//! disagreement is a defect in one of them, resolved by revision, never by
//! divergence").
//!
//! # How a changed field reaches an artifact
//!
//! RFC 0031's mapping table sends each diff field to at most one of RFC 0030's
//! nine typed dependency reasons ([`field_reason`]):
//!
//! > | Diff `field` | RFC 0030 dependency reason |
//! > |---|---|
//! > | `properties` | query-key change: evidence is bound to the old claim identity
//! >   and is not addressable under the new one |
//! > | `assumptions`, `fairness`, `bounds` | `relies-on-assumption-fairness-bound` |
//! > | `observers` | `observes-event-family` |
//! > | `abstraction_maps` | `uses-abstraction-component` |
//! > | `assurance` | `depends-on-lemma-or-checker` |
//! > | `scope`, and the `correspondence` program axis |
//! >   `consumes-encoding-epoch-or-correspondence` |
//! > | `faults`, `completion_policy`, `nondeterminism`, `trust_boundaries`,
//! >   `security_policy`, `optimization`, `non_vacuity` | no named reason; classify
//! >   `Unknown` ⇒ `unknown` |
//!
//! Three trigger shapes fall out, evaluated per artifact in decreasing severity
//! (`invalidated` > `unknown` > `reused`, so the partition is disjoint by
//! construction and total over the input set):
//!
//! 1. **The query-key trigger** (`properties` changed). "A property edit changes
//!    the contract's canonical encoding and therefore the `in_*` identity and RFC
//!    0030's query key. Evidence keyed to the old identity is not stale — it is
//!    unaddressable — so it MUST land in `invalidated` unless a `Validated` edge
//!    carries a checker witness licensing reuse." [`EvidenceRecord`] carries both
//!    facts (`keyed_to_before_intent`, `key_change_reuse_witness`) because neither
//!    is expressible as one of the nine reasons — RFC 0030's own table says so
//!    ("none — a key change").
//! 2. **Reason triggers.** Every other changed field with a named reason implicates
//!    exactly the artifacts holding a dependency edge with that reason, through the
//!    edge's independence tri-state: `Dependent` invalidates; `Unknown` is
//!    dependent and fails to `unknown`; `Independent` licenses reuse only on the
//!    terms of the edge's class (see "Which independence claims license reuse
//!    here").
//! 3. **The no-reason trigger.** A changed field with no named reason yields
//!    independence `Unknown` for *every* in-scope artifact — RFC 0030 calls its own
//!    row "the load-bearing one" — so everything not already invalidated lands in
//!    `unknown`, never in `reused`.
//!
//! A field classified non-affirmatively (`unknown`/`unsupported`/`incomparable`)
//! counts as changed for every trigger above: the diff does not know the field is
//! unchanged, and reuse licensed by ignorance would be exactly the "implicit
//! reuse" the totality rule forbids.
//!
//! # Which independence claims license reuse here
//!
//! The diff artifact is promotion-facing — its `reused` set is what RFC 0032's
//! gates read — so an independence claim reaches `reused` only on terms that
//! support promotion:
//!
//! - `Exact`/`Validated` edges: RFC 0030, "`DefinitelyIndependent` on a
//!   strong-class edge MUST carry a witness" — witnessed independence is reused
//!   ([`ReuseJustification::WitnessedIndependence`]); an unwitnessed independence
//!   claim on a strong class is ill-formed under that rule and fails closed to
//!   `unknown` rather than being believed.
//! - `Conservative` edges: independence established "by the conservative cone" —
//!   a stated dependency-theorem instance with its assumptions enumerated is that
//!   class's own justification, so the reuse is licensed and the justification is
//!   named ([`ReuseJustification::ConservativeTheorem`]), which is what "a
//!   conservatively rebuilt artifact MUST NOT be reported as `reused` without its
//!   `Conservative`-class justification" requires of this output.
//! - `Experimental` edges: heuristic independence is *permitted* there by RFC
//!   0030, and "an `Experimental` reuse MUST NOT support promotion" (RFC 0031) —
//!   so it never reaches this artifact's `reused` set and fails closed to
//!   `unknown` with its cause named. The permission is the cache's, not the
//!   promotion gate's.
//!
//! # Typed dispositions, wire sets
//!
//! "Impact output distinguishes definitely invalidated from conservatively
//! rebuilt": the wire carries three arrays of artifact identities
//! ([`ImpactSet::artifact_json`], the `impact` section of
//! `notes/plan/schemas/semantic-diff.schema.json`), and the typed result carries a
//! [`Disposition`] per artifact whose cause/justification says *why* — a definite
//! dependence, a key change, an unknown independence, a no-reason field, a
//! witnessed reuse, a conservative theorem. INV-003's discipline: the wire arrays
//! are a rendering; the typed dispositions are where the facts live.
//!
//! # What this module does not do
//!
//! It never classifies an intent change (the changed-field set is an *input*,
//! computed by the classifier family and assembled by [`crate::artifact`]); it
//! never consults claim statuses (invalidation is a statement about evidence, not
//! a downgrade of a claim — RFC 0031, "Evidence-status interaction"); and it never
//! reports a cone as minimal (RFC 0030, "Minimality is not claimed").

use std::collections::{BTreeMap, BTreeSet};

use continuum_intent::canonical_json::Json;
use continuum_intent::change_policy::PolicyField;

/// One evidence-artifact identity, validated against the normative schema's
/// item pattern for the three `impact` arrays:
/// `^[a-z][a-z0-9_]*_[A-Za-z0-9_-]+$` (`notes/plan/schemas/semantic-diff.schema.json`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct EvidenceId(String);

impl EvidenceId {
    /// Validate and wrap an evidence-artifact identity.
    ///
    /// # Errors
    ///
    /// [`ImpactError::MalformedEvidenceId`] when the string does not match the
    /// schema's `^[a-z][a-z0-9_]*_[A-Za-z0-9_-]+$` item pattern.
    pub fn new(id: &str) -> Result<Self, ImpactError> {
        let malformed = || ImpactError::MalformedEvidenceId {
            value: id.to_owned(),
        };
        // The first `_` split decides the whole pattern: every character before it
        // must be in the prefix class and every character after it in the suffix
        // class, and no other split can admit a string this one rejects (a later
        // split only moves suffix-class-checked characters into the stricter
        // prefix class).
        let mut chars = id.char_indices();
        match chars.next() {
            Some((_, c)) if c.is_ascii_lowercase() => {}
            _ => return Err(malformed()),
        }
        let Some(underscore) = id.find('_') else {
            return Err(malformed());
        };
        let prefix = &id[1..underscore];
        let suffix = &id[underscore + 1..];
        if !prefix
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        {
            return Err(malformed());
        }
        if suffix.is_empty()
            || !suffix
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(malformed());
        }
        Ok(Self(id.to_owned()))
    }

    /// The identity.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// RFC 0030's closed nine-member dependency-reason set, spelled token for token
/// from its "Semantic dependency reasons" table. See the module doc for why this
/// crate carries the rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DependencyReason {
    /// `reads-type`: the query consumed a declaration's type, not its value.
    ReadsType,
    /// `reads-value`: the query consumed a definition's value or body.
    ReadsValue,
    /// `unfolds-definition`: the query expanded a definition.
    UnfoldsDefinition,
    /// `selects-instance-or-profile`: the query resolved a domain-pack instance or
    /// a fidelity profile (RFC 0002).
    SelectsInstanceOrProfile,
    /// `observes-event-family`: the result depends on which events an observer
    /// publishes.
    ObservesEventFamily,
    /// `relies-on-assumption-fairness-bound`: the result is conditional on an
    /// assumption, a fairness constraint, or a bound.
    ReliesOnAssumptionFairnessBound,
    /// `uses-abstraction-component`: the query consumed an abstraction or
    /// correspondence map component (§16, RFC 0036).
    UsesAbstractionComponent,
    /// `depends-on-lemma-or-checker`: the result rests on a lemma, a checker
    /// identity, or an assurance requirement.
    DependsOnLemmaOrChecker,
    /// `consumes-encoding-epoch-or-correspondence`: the result depends on a solver
    /// encoding, an epoch-scoped encoding, or a model/program correspondence link.
    ConsumesEncodingEpochOrCorrespondence,
}

impl DependencyReason {
    /// Every reason, in RFC 0030's table order.
    pub const ALL: [Self; 9] = [
        Self::ReadsType,
        Self::ReadsValue,
        Self::UnfoldsDefinition,
        Self::SelectsInstanceOrProfile,
        Self::ObservesEventFamily,
        Self::ReliesOnAssumptionFairnessBound,
        Self::UsesAbstractionComponent,
        Self::DependsOnLemmaOrChecker,
        Self::ConsumesEncodingEpochOrCorrespondence,
    ];

    /// The RFC 0030 wire token.
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            Self::ReadsType => "reads-type",
            Self::ReadsValue => "reads-value",
            Self::UnfoldsDefinition => "unfolds-definition",
            Self::SelectsInstanceOrProfile => "selects-instance-or-profile",
            Self::ObservesEventFamily => "observes-event-family",
            Self::ReliesOnAssumptionFairnessBound => "relies-on-assumption-fairness-bound",
            Self::UsesAbstractionComponent => "uses-abstraction-component",
            Self::DependsOnLemmaOrChecker => "depends-on-lemma-or-checker",
            Self::ConsumesEncodingEpochOrCorrespondence => {
                "consumes-encoding-epoch-or-correspondence"
            }
        }
    }
}

/// RFC 0030's closed four-member reuse-edge class set ("Reuse-edge classes").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReuseEdgeClass {
    /// Invalidation is exact.
    Exact,
    /// Justified by a checked witness.
    Validated,
    /// A conservative dependency theorem covers it, assumptions enumerated.
    Conservative,
    /// Unsound-permitted; never supports promotion.
    Experimental,
}

/// RFC 0030's independence tri-state, generalized from RFC 0004's event rule:
/// `DefinitelyIndependent(witness)` / `DefinitelyDependent(reason)` / `Unknown`.
///
/// "`Unknown` is dependent" — the load-bearing member.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Independence {
    /// `DefinitelyDependent`: the edge's dependency on the change is established.
    Dependent,
    /// `DefinitelyIndependent`: independence is claimed; whether it licenses reuse
    /// depends on the edge's class and the witness (module doc, "Which
    /// independence claims license reuse here").
    Independent {
        /// Whether a checked witness accompanies the claim. RFC 0030:
        /// "`DefinitelyIndependent` on a strong-class edge MUST carry a witness.
        /// Heuristic independence licenses only `Experimental` edges."
        witnessed: bool,
    },
    /// Independence is not established in either direction. Dependent (RFC 0030).
    Unknown,
}

/// One recorded dependency edge of an evidence artifact: a typed reason, the
/// reuse-edge class that would license reuse across it, and the independence of
/// the artifact from a change arriving through that reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DependencyEdge {
    /// The RFC 0030 typed reason this edge records.
    pub reason: DependencyReason,
    /// The reuse-edge class of the edge.
    pub class: ReuseEdgeClass,
    /// The artifact's independence from a change arriving through `reason`.
    pub independence: Independence,
}

/// One evidence artifact in the impact set's scope: an artifact that names either
/// intent identity or either snapshot, with the dependency facts RFC 0031's
/// partition reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceRecord {
    /// The artifact's identity.
    pub id: EvidenceId,
    /// Whether the *before* intent identity is among the artifact's canonical
    /// input identities (RFC 0030's query key) — the fact the query-key trigger
    /// reads when `properties` changed.
    pub keyed_to_before_intent: bool,
    /// Whether "a `Validated` edge carries a checker witness licensing reuse"
    /// across the identity change (RFC 0031's one stated exception to key-change
    /// invalidation). Carried as its own fact because a key change is not one of
    /// the nine reasons — RFC 0030's mapping table says "none — a key change".
    pub key_change_reuse_witness: bool,
    /// The artifact's recorded dependency edges.
    pub edges: Vec<DependencyEdge>,
}

/// Why an artifact landed in `invalidated`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidationCause {
    /// The query-key trigger: `properties` changed, the artifact is keyed to the
    /// old intent identity, and no `Validated` witness licenses reuse — the
    /// artifact is unaddressable, not stale.
    KeyChange,
    /// A dependency edge with this reason is `DefinitelyDependent` on a changed
    /// field.
    DependsOn(DependencyReason),
}

/// Why an artifact landed in `unknown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnknownCause {
    /// An edge with this reason has independence `Unknown` from a changed field —
    /// dependent by the tri-state rule, so never `reused`.
    UnknownIndependence(DependencyReason),
    /// A changed field has no named dependency reason, so independence is
    /// `Unknown` for every in-scope artifact (RFC 0030's "load-bearing" row).
    FieldWithoutReason(PolicyField),
    /// The only independence covering this reason rides an `Experimental` edge,
    /// and an `Experimental` reuse must not support promotion.
    ExperimentalIndependence(DependencyReason),
    /// An `Exact`/`Validated` edge claims independence without the witness RFC
    /// 0030 requires of a strong class; the claim is ill-formed and fails closed.
    UnwitnessedIndependence(DependencyReason),
}

/// Why an artifact landed in `reused`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReuseJustification {
    /// No changed field implicates the artifact through any trigger.
    NoDependencyOnAnyChange,
    /// Every implicated edge carries witnessed independence on a strong class.
    WitnessedIndependence,
    /// Every implicated edge is covered by a `Conservative` dependency-theorem
    /// instance — the class's own justification, named as RFC 0031 requires.
    ConservativeTheorem,
    /// A `Validated` edge carries a checker witness licensing reuse across the
    /// `properties` query-key change.
    KeyChangeValidatedWitness,
}

/// One artifact's disposition: which wire set it lands in, and why (typed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    /// The artifact must be re-established; it lands in the wire `invalidated`.
    Invalidated(InvalidationCause),
    /// Dependence could not be resolved; it lands in the wire `unknown`, never in
    /// `reused`.
    Unknown(UnknownCause),
    /// Reuse is licensed; it lands in the wire `reused`.
    Reused(ReuseJustification),
}

impl Disposition {
    /// The severity order the fold uses: `Invalidated` > `Unknown` > `Reused`.
    const fn severity(self) -> u8 {
        match self {
            Self::Invalidated(_) => 2,
            Self::Unknown(_) => 1,
            Self::Reused(_) => 0,
        }
    }

    /// The more severe of two dispositions; on a tie, the first (trigger order is
    /// fixed, so the kept cause is deterministic).
    const fn join(self, other: Self) -> Self {
        if other.severity() > self.severity() {
            other
        } else {
            self
        }
    }
}

/// The computed impact set: one [`Disposition`] per in-scope artifact, pairwise
/// disjoint and total over the input by construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImpactSet {
    dispositions: BTreeMap<EvidenceId, Disposition>,
}

impl ImpactSet {
    /// Every artifact and its disposition, in identity order.
    pub fn iter(&self) -> impl Iterator<Item = (&EvidenceId, Disposition)> {
        self.dispositions
            .iter()
            .map(|(id, disposition)| (id, *disposition))
    }

    /// The artifacts of the wire `invalidated` array, in identity order.
    pub fn invalidated(&self) -> impl Iterator<Item = &EvidenceId> {
        self.in_set(2)
    }

    /// The artifacts of the wire `unknown` array, in identity order.
    pub fn unknown(&self) -> impl Iterator<Item = &EvidenceId> {
        self.in_set(1)
    }

    /// The artifacts of the wire `reused` array, in identity order.
    pub fn reused(&self) -> impl Iterator<Item = &EvidenceId> {
        self.in_set(0)
    }

    fn in_set(&self, severity: u8) -> impl Iterator<Item = &EvidenceId> {
        self.dispositions
            .iter()
            .filter(move |(_, disposition)| disposition.severity() == severity)
            .map(|(id, _)| id)
    }

    /// The wire `impact` section: `{invalidated, reused, unknown}`, each an array
    /// of artifact identities in identity order
    /// (`notes/plan/schemas/semantic-diff.schema.json`).
    #[must_use]
    pub fn artifact_json(&self) -> Json {
        let render = |ids: Box<dyn Iterator<Item = &EvidenceId> + '_>| {
            Json::Array(ids.map(|id| Json::String(id.as_str().to_owned())).collect())
        };
        Json::object([
            (
                "invalidated".to_owned(),
                render(Box::new(self.invalidated())),
            ),
            ("reused".to_owned(), render(Box::new(self.reused()))),
            ("unknown".to_owned(), render(Box::new(self.unknown()))),
        ])
        .unwrap_or(Json::Null)
    }
}

/// The RFC 0031 field → RFC 0030 reason mapping, verbatim from the "Impact set"
/// table. `None` for `properties` (a query-key change, not a reason) and for the
/// seven no-reason fields (independence `Unknown` for everything in scope).
#[must_use]
pub const fn field_reason(field: PolicyField) -> Option<DependencyReason> {
    match field {
        PolicyField::Assumptions | PolicyField::Fairness | PolicyField::Bounds => {
            Some(DependencyReason::ReliesOnAssumptionFairnessBound)
        }
        PolicyField::Observers => Some(DependencyReason::ObservesEventFamily),
        PolicyField::AbstractionMaps => Some(DependencyReason::UsesAbstractionComponent),
        PolicyField::Assurance => Some(DependencyReason::DependsOnLemmaOrChecker),
        PolicyField::Scope => Some(DependencyReason::ConsumesEncodingEpochOrCorrespondence),
        PolicyField::Properties
        | PolicyField::Faults
        | PolicyField::CompletionPolicy
        | PolicyField::Nondeterminism
        | PolicyField::TrustBoundaries
        | PolicyField::SecurityPolicy
        | PolicyField::Optimization
        | PolicyField::NonVacuity => None,
    }
}

/// Whether a changed `field` is one of the seven with no named reason, whose
/// change sends every in-scope artifact's independence to `Unknown`.
const fn field_without_reason(field: PolicyField) -> bool {
    matches!(
        field,
        PolicyField::Faults
            | PolicyField::CompletionPolicy
            | PolicyField::Nondeterminism
            | PolicyField::TrustBoundaries
            | PolicyField::SecurityPolicy
            | PolicyField::Optimization
            | PolicyField::NonVacuity
    )
}

/// Compute the impact set.
///
/// `changed_fields` is every field with at least one record whose relation is not
/// `unchanged` — including the non-affirmative relations, which count as changed
/// (module doc). `correspondence_changed` is whether the program diff carries a
/// record on the `correspondence` axis, which triggers
/// `consumes-encoding-epoch-or-correspondence` exactly as a `scope` change does
/// and places every dependent proof "into `invalidated` or `unknown`" (RFC 0031,
/// "Program semantic diff").
///
/// Trigger evaluation per artifact, in fixed order (key change, then reason
/// triggers in [`PolicyField::ALL`] order with the correspondence axis after
/// `scope`'s reason, then the no-reason fields), folded by severity — so the
/// output is deterministic in the inputs and independent of the order the caller
/// listed `evidence` in.
///
/// # Errors
///
/// [`ImpactError::DuplicateEvidence`] when two records carry one identity: the
/// scope is a set, and folding two edge lists silently would let the second
/// record's edges launder the first's dependence.
pub fn compute_impact(
    changed_fields: &BTreeSet<PolicyField>,
    correspondence_changed: bool,
    evidence: &[EvidenceRecord],
) -> Result<ImpactSet, ImpactError> {
    let triggered_reasons: BTreeSet<DependencyReason> = changed_fields
        .iter()
        .filter_map(|field| field_reason(*field))
        .chain(
            correspondence_changed
                .then_some(DependencyReason::ConsumesEncodingEpochOrCorrespondence),
        )
        .collect();
    let no_reason_field = PolicyField::ALL
        .into_iter()
        .find(|field| changed_fields.contains(field) && field_without_reason(*field));
    let properties_changed = changed_fields.contains(&PolicyField::Properties);

    let mut dispositions: BTreeMap<EvidenceId, Disposition> = BTreeMap::new();
    for record in evidence {
        let mut disposition = Disposition::Reused(ReuseJustification::NoDependencyOnAnyChange);

        // 1. The query-key trigger.
        if properties_changed && record.keyed_to_before_intent {
            disposition = disposition.join(if record.key_change_reuse_witness {
                Disposition::Reused(ReuseJustification::KeyChangeValidatedWitness)
            } else {
                Disposition::Invalidated(InvalidationCause::KeyChange)
            });
        }

        // 2. Reason triggers, through each implicated edge's tri-state.
        for reason in &triggered_reasons {
            for edge in record.edges.iter().filter(|edge| edge.reason == *reason) {
                disposition = disposition.join(edge_disposition(*edge));
            }
        }

        // 3. The no-reason trigger: independence `Unknown` for everything.
        if let Some(field) = no_reason_field {
            disposition = disposition.join(Disposition::Unknown(UnknownCause::FieldWithoutReason(
                field,
            )));
        }

        if dispositions
            .insert(record.id.clone(), disposition)
            .is_some()
        {
            return Err(ImpactError::DuplicateEvidence {
                id: record.id.as_str().to_owned(),
            });
        }
    }
    Ok(ImpactSet { dispositions })
}

/// One implicated edge's contribution, per the module doc's licensing rules.
const fn edge_disposition(edge: DependencyEdge) -> Disposition {
    match edge.independence {
        Independence::Dependent => {
            Disposition::Invalidated(InvalidationCause::DependsOn(edge.reason))
        }
        Independence::Unknown => {
            Disposition::Unknown(UnknownCause::UnknownIndependence(edge.reason))
        }
        Independence::Independent { witnessed } => match edge.class {
            ReuseEdgeClass::Exact | ReuseEdgeClass::Validated => {
                if witnessed {
                    Disposition::Reused(ReuseJustification::WitnessedIndependence)
                } else {
                    Disposition::Unknown(UnknownCause::UnwitnessedIndependence(edge.reason))
                }
            }
            ReuseEdgeClass::Conservative => {
                Disposition::Reused(ReuseJustification::ConservativeTheorem)
            }
            ReuseEdgeClass::Experimental => {
                Disposition::Unknown(UnknownCause::ExperimentalIndependence(edge.reason))
            }
        },
    }
}

/// Why an impact computation was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImpactError {
    /// An evidence identity does not match the schema's item pattern.
    MalformedEvidenceId {
        /// The rejected string.
        value: String,
    },
    /// Two evidence records carry one identity.
    DuplicateEvidence {
        /// The repeated identity.
        id: String,
    },
}

impl core::fmt::Display for ImpactError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MalformedEvidenceId { value } => write!(
                f,
                "evidence identity {value:?} does not match ^[a-z][a-z0-9_]*_[A-Za-z0-9_-]+$"
            ),
            Self::DuplicateEvidence { id } => {
                write!(
                    f,
                    "evidence identity {id:?} appears twice in the impact scope"
                )
            }
        }
    }
}

impl std::error::Error for ImpactError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(id: &str) -> EvidenceId {
        EvidenceId::new(id).expect("test identity is well formed")
    }

    fn record(id: &str, edges: Vec<DependencyEdge>) -> EvidenceRecord {
        EvidenceRecord {
            id: ev(id),
            keyed_to_before_intent: false,
            key_change_reuse_witness: false,
            edges,
        }
    }

    fn edge(
        reason: DependencyReason,
        class: ReuseEdgeClass,
        independence: Independence,
    ) -> DependencyEdge {
        DependencyEdge {
            reason,
            class,
            independence,
        }
    }

    fn wire_sets(set: &ImpactSet) -> (Vec<String>, Vec<String>, Vec<String>) {
        let names = |ids: Box<dyn Iterator<Item = &EvidenceId> + '_>| {
            ids.map(|id| id.as_str().to_owned()).collect()
        };
        (
            names(Box::new(set.invalidated())),
            names(Box::new(set.reused())),
            names(Box::new(set.unknown())),
        )
    }

    /// The three wire sets are pairwise disjoint and their union covers every
    /// input artifact — the RFC's totality rule, asserted as a helper for every
    /// test below.
    fn assert_partition(set: &ImpactSet, expected: &[&str]) {
        let (invalidated, reused, unknown) = wire_sets(set);
        let mut all: Vec<String> = invalidated
            .iter()
            .chain(reused.iter())
            .chain(unknown.iter())
            .cloned()
            .collect();
        all.sort();
        let mut want: Vec<String> = expected.iter().map(|id| (*id).to_owned()).collect();
        want.sort();
        assert_eq!(all, want, "the union covers exactly the input set");
        let unique: BTreeSet<String> = all.iter().cloned().collect();
        assert_eq!(
            unique.len(),
            all.len(),
            "the three sets are pairwise disjoint"
        );
    }

    #[test]
    fn a_dependent_edge_on_a_changed_reason_invalidates() {
        let changed = BTreeSet::from([PolicyField::Bounds]);
        let evidence = [record(
            "ev_bounded_run",
            vec![edge(
                DependencyReason::ReliesOnAssumptionFairnessBound,
                ReuseEdgeClass::Exact,
                Independence::Dependent,
            )],
        )];
        let set = compute_impact(&changed, false, &evidence).expect("computes");
        assert_partition(&set, &["ev_bounded_run"]);
        assert_eq!(
            set.iter().next().map(|(_, d)| d),
            Some(Disposition::Invalidated(InvalidationCause::DependsOn(
                DependencyReason::ReliesOnAssumptionFairnessBound
            )))
        );
    }

    #[test]
    fn unknown_independence_goes_to_unknown_never_reused() {
        let changed = BTreeSet::from([PolicyField::Observers]);
        let evidence = [record(
            "ev_trace",
            vec![edge(
                DependencyReason::ObservesEventFamily,
                ReuseEdgeClass::Exact,
                Independence::Unknown,
            )],
        )];
        let set = compute_impact(&changed, false, &evidence).expect("computes");
        let (invalidated, reused, unknown) = wire_sets(&set);
        assert!(invalidated.is_empty());
        assert!(reused.is_empty());
        assert_eq!(unknown, ["ev_trace"]);
    }

    #[test]
    fn an_unimplicated_artifact_is_reused() {
        let changed = BTreeSet::from([PolicyField::Bounds]);
        let evidence = [record(
            "proof_model1",
            vec![edge(
                DependencyReason::ObservesEventFamily,
                ReuseEdgeClass::Exact,
                Independence::Dependent,
            )],
        )];
        // The artifact's one edge records a different reason than the change
        // triggers, so no trigger implicates it.
        let set = compute_impact(&changed, false, &evidence).expect("computes");
        let (_, reused, _) = wire_sets(&set);
        assert_eq!(reused, ["proof_model1"]);
    }

    #[test]
    fn a_no_reason_field_change_sends_everything_not_invalidated_to_unknown() {
        let changed = BTreeSet::from([PolicyField::Faults]);
        let evidence = [
            record("ev_clean", vec![]),
            record(
                "ev_witnessed",
                vec![edge(
                    DependencyReason::ReliesOnAssumptionFairnessBound,
                    ReuseEdgeClass::Validated,
                    Independence::Independent { witnessed: true },
                )],
            ),
        ];
        let set = compute_impact(&changed, false, &evidence).expect("computes");
        let (invalidated, reused, unknown) = wire_sets(&set);
        assert!(invalidated.is_empty());
        assert!(
            reused.is_empty(),
            "no witness can prove independence from a change no reason expresses"
        );
        assert_eq!(unknown, ["ev_clean", "ev_witnessed"]);
    }

    #[test]
    fn the_key_change_invalidates_unless_a_validated_witness_licenses_reuse() {
        let changed = BTreeSet::from([PolicyField::Properties]);
        let mut keyed = record("ev_old_claim", vec![]);
        keyed.keyed_to_before_intent = true;
        let mut licensed = record("ev_licensed", vec![]);
        licensed.keyed_to_before_intent = true;
        licensed.key_change_reuse_witness = true;
        let set = compute_impact(&changed, false, &[keyed, licensed]).expect("computes");
        let (invalidated, reused, _) = wire_sets(&set);
        assert_eq!(invalidated, ["ev_old_claim"]);
        assert_eq!(reused, ["ev_licensed"]);
    }

    #[test]
    fn experimental_and_unwitnessed_independence_fail_closed_to_unknown() {
        let changed = BTreeSet::from([PolicyField::Assurance]);
        let evidence = [
            record(
                "ev_heuristic",
                vec![edge(
                    DependencyReason::DependsOnLemmaOrChecker,
                    ReuseEdgeClass::Experimental,
                    Independence::Independent { witnessed: false },
                )],
            ),
            record(
                "ev_strong_no_witness",
                vec![edge(
                    DependencyReason::DependsOnLemmaOrChecker,
                    ReuseEdgeClass::Validated,
                    Independence::Independent { witnessed: false },
                )],
            ),
            record(
                "ev_conservative",
                vec![edge(
                    DependencyReason::DependsOnLemmaOrChecker,
                    ReuseEdgeClass::Conservative,
                    Independence::Independent { witnessed: false },
                )],
            ),
        ];
        let set = compute_impact(&changed, false, &evidence).expect("computes");
        let (invalidated, reused, unknown) = wire_sets(&set);
        assert!(invalidated.is_empty());
        assert_eq!(
            reused,
            ["ev_conservative"],
            "the conservative theorem is its own justification"
        );
        assert_eq!(unknown, ["ev_heuristic", "ev_strong_no_witness"]);
    }

    #[test]
    fn the_correspondence_axis_triggers_without_any_intent_change() {
        let changed = BTreeSet::new();
        let evidence = [record(
            "proof_refinement1",
            vec![edge(
                DependencyReason::ConsumesEncodingEpochOrCorrespondence,
                ReuseEdgeClass::Exact,
                Independence::Dependent,
            )],
        )];
        let set = compute_impact(&changed, true, &evidence).expect("computes");
        let (invalidated, _, _) = wire_sets(&set);
        assert_eq!(invalidated, ["proof_refinement1"]);
        // The same artifact with no correspondence change is reused.
        let set = compute_impact(&changed, false, &evidence).expect("computes");
        let (_, reused, _) = wire_sets(&set);
        assert_eq!(reused, ["proof_refinement1"]);
    }

    #[test]
    fn a_duplicate_identity_is_refused() {
        let changed = BTreeSet::new();
        let evidence = [record("ev_twin", vec![]), record("ev_twin", vec![])];
        assert_eq!(
            compute_impact(&changed, false, &evidence),
            Err(ImpactError::DuplicateEvidence {
                id: "ev_twin".to_owned()
            })
        );
    }

    #[test]
    fn evidence_identities_are_validated_against_the_schema_pattern() {
        for good in ["ev_failure1", "proof_model1", "ev_a-B_c", "cert_X"] {
            assert!(EvidenceId::new(good).is_ok(), "{good} matches");
        }
        for bad in ["Ev_x", "ev", "ev_", "_x", "ev-x_y", "9x_y", "eV_x", ""] {
            assert!(EvidenceId::new(bad).is_err(), "{bad} does not match");
        }
    }

    #[test]
    fn severity_folds_keep_the_worst_disposition() {
        // One artifact hit by an invalidating trigger and an unknown trigger
        // lands in `invalidated`, so the sets stay disjoint.
        let changed = BTreeSet::from([PolicyField::Bounds, PolicyField::Faults]);
        let evidence = [record(
            "ev_both",
            vec![edge(
                DependencyReason::ReliesOnAssumptionFairnessBound,
                ReuseEdgeClass::Exact,
                Independence::Dependent,
            )],
        )];
        let set = compute_impact(&changed, false, &evidence).expect("computes");
        let (invalidated, reused, unknown) = wire_sets(&set);
        assert_eq!(invalidated, ["ev_both"]);
        assert!(reused.is_empty());
        assert!(unknown.is_empty());
    }
}
