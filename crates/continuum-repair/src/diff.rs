//! The semantic and protected-intent diff of a repair transaction (PR-20 / IMPL-04).
//!
//! A repair transaction carries one RFC 0031 `diff_*` artifact between its base and its
//! candidate: the semantic diff, with the protected-intent diff inside it as the
//! `intent_changes` set (RFC 0032: `semantic_diff` and `intent_diff` name one artifact).
//! This module computes that artifact from the stored artifacts the transaction names,
//! classifies the transaction as a repair or as a privileged intent revision, and refuses
//! a claimed diff that is not the computed one.
//!
//! # Clause → mechanism
//!
//! | Clause | Source | Mechanism |
//! |---|---|---|
//! | the diff compares the base and the candidate, and resolves both intents by reference | RFC 0031 "Inputs"; RFC 0037 "a snapshot carries its intent by reference" | [`compute`] reads `base_snapshot`, `candidate_snapshot` and `base_intent` from the transaction, the candidate's binding from [`SealedSnapshots`], and both contracts from [`IntentRegistry`]. No caller names a snapshot, an intent, a contract, or a diff |
//! | both snapshots are sealed | RFC 0031 "Inputs" | a snapshot [`SealedSnapshots`] does not resolve is [`Undiffable::SnapshotUnresolved`] |
//! | one diff artifact shape, one classification authority | RFC 0031 "Wire surface"; RFC 0032 correction 10 | [`continuum_semantic_diff::artifact::assemble`], called unchanged. This module adds no relation and re-derives none |
//! | the classifier cannot complete: a typed error, never silence | RFC 0031 "Fail-closed rule"; INV-008 | [`DiffOutcome::Inconclusive`] with an [`Undiffable`] reason. It is never read as "no change" |
//! | weakening a property, strengthening an assumption, shrinking a bound, hiding an event, removing a fault or downgrading assurance is a privileged intent revision, never a repair | RFC 0032 "Intent integrity and reclassification"; INV-001, INV-011 | [`IntentClassification::PrivilegedIntentRevision`], with each record typed as a [`Weakening`] |
//! | rebinding a snapshot lineage to a different intent is privileged | RFC 0037 "Storage and custody" | a candidate bound to an intent other than `base_intent` is a revision even when every field classifies `unchanged` ([`IntentRevision::rebinds`]) |
//! | gate 3 passes on the recomputed verdict | RFC 0032 gate 3, correction 8 | [`DiffOutcome::gate_status`]: `passed` only for a repair, whose decision is `allow`, and only when the program-side layer is classified |
//! | an unrequested layer is not evidence of absence | RFC 0031 "Wire surface" | the program-side layer is [`ProgramLayer::Unclassified`] unless the candidate equals the base; then no impact entry is `reused` and gate 3 is `inconclusive` |
//! | a client-supplied diff is never trusted | RFC 0031 "Wire surface"; RFC 0032 | [`verify_claimed_diff`] recomputes and compares canonical bytes; [`verify_claimed_handle`] compares the handle |
//! | the diff's identity is its content | ADR-0013 | [`TransactionDiff::handle`] is [`mint_handle`]: `diff_` plus the digest of a versioned program-layer tag, a NUL, and the artifact's canonical bytes without `diff_id` |
//! | a persisted artifact cannot keep a state its bytes do not carry | cr-3psesg th-1k7ib7 | the layer is bound into the handle and so into the bytes; [`gate_status_of_cached`] is the only path from stored bytes to a gate status, and it recomputes from the stores and refuses a layer mismatch |
//!
//! # Inputs this module fixes, and why
//!
//! - **Acceptance path.** [`AcceptancePath::AgentAccept`]. A repair runs under ordinary
//!   repair authority, and RFC 0037's `AgentAccept` is also the path for a principal
//!   "not known to be human", so it is the reading that cannot open a human-only `allow`.
//! - **Requested assurance.** The base contract's own `assurance.minimum`, raised to
//!   `validated`, RFC 0032's evidence floor for gate 3. The caller does not choose it.
//!   The assembler's clamp only narrows a direction to `unknown`, and `unknown` fails
//!   closed, so no level can turn a weakening into `allow`.
//! - **Gate 3 is a conjunction.** RFC 0032's claim is "classifies no protected change"
//!   and its rule is "passes iff the recomputed decision is `allow`". Both must hold,
//!   and the candidate must stay bound to `base_intent`. A candidate that keeps the
//!   base binding compares the base contract with itself, so RFC 0031's `unchanged`
//!   shortcut always fires for it: in practice a repair is exactly a candidate that
//!   keeps the binding under an `accepted` base intent.
//! - **Base standing.** The base intent must stand `accepted`. A proposal was never
//!   protected, and a superseded contract no longer governs (RFC 0037 P7), so both are
//!   typed inconclusives and never a pass. One exception, crate-private: the receipt
//!   re-derives a **published** receipt's diff against a base superseded after
//!   promotion (`BaseStanding::Historical`, RFC 0032: published receipts remain
//!   verifiable). It reaches that path only after checking that the record shows gate
//!   12 `passed`, so no new decision rests on a superseded base.
//! - **Program-side changes: not classified, and fail closed.** No program-diff
//!   classifier for RFC 0031's seven axes exists in this workspace, so `semantic_changes`
//!   is empty. RFC 0031: "A layer that was not requested MUST leave its artifact section
//!   empty and MUST NOT be reported as evidence of absence". The diff is assembled with
//!   [`ProgramLayer::Unclassified`] (the typed fact is
//!   `DiffArtifact::program_layer`), with one exception: a candidate identical to its
//!   base has no program change, so its layer is classified. Under `Unclassified`:
//!   - no scoped evidence is `reused`: each entry is `invalidated` or `unknown`
//!     (`UnknownCause::ProgramLayerUnclassified`), that is, revalidated;
//!   - a repair's gate 3 is `inconclusive` (`Unsupported`), never `passed`, because an
//!     `allow` must not rest on program changes nobody classified. So **gate 3 cannot
//!     pass today** for any candidate that changes the workspace. The owner is the
//!     missing program-side classifier.
//!
//!   The intent verdict itself is unchanged: RFC 0031 keeps program-side changes out
//!   of the intent policy. The artifact's wire form has no field for "not classified"
//!   (`semantic-diff.schema.json` types `semantic_changes` items by a closed seven-axis
//!   `kind`, and every object is closed), so on the wire the empty section and the
//!   all-`unknown` impact are the only trace; the impact arrays hold bare artifact
//!   ids with no cause token, so they cannot name the cause either. The layer is
//!   therefore bound into the handle ([`mint_handle`]), which is the artifact's
//!   `diff_id`, so two layers never share bytes. A typed wire marker needs a schema
//!   change, raised and not made here.
//! - **Impact scope.** The evidence that names either intent or either snapshot comes
//!   from the daemon's evidence store through [`ImpactScope`]. A store that does not
//!   answer is [`Undiffable::StoreUnavailable`], never an empty scope.
//!
//! # What is not here
//!
//! - Writing `semantic_diff` into a transaction version, and gate 3 as a recorded gate
//!   status. Both belong to the evaluation path that records a version
//!   (`repair.evaluate`); this module gives it [`TransactionDiff::handle`] and
//!   [`DiffOutcome::gate_status`].
//! - The evidence-status floor of RFC 0032 (gate 3's supporting evidence at
//!   `validated` or `proved`, by an independent checker): the recording path's, which
//!   writes the gate's evidence.
//! - A bound on the evidence scope before assembly. The scope comes from the daemon's
//!   own store, not from a caller, and the artifact is assembled twice (once to derive
//!   its identity), because `DiffArtifact` has no way to re-stamp its handle.
//! - Publication of the `diff_*` artifact, and daemon wiring of `intent.diff` and
//!   `intent.propose_revision` (the daemon's).
//! - A weakening carried by a `model`, `proof` or `correspondence` change while the
//!   candidate stays bound to the base intent. The protected fields do not move, so the
//!   intent diff is empty; what such a change does to the meaning of a property is the
//!   program-side layer's and gates 5 to 8's.

use core::fmt;

use continuum_intent::assurance_policy::AssuranceLevel;
use continuum_intent::canonical_json::Json;
use continuum_intent::change_policy::{AcceptancePath, PolicyDecision, PolicyField, Relation};
use continuum_intent::contract::{IntentContract, IntentId};
use continuum_semantic_diff::artifact::{
    self, AssembleError, DiffArtifact, DiffId, DiffRequest, IntentChangeRecord,
};
use continuum_semantic_diff::impact::{EvidenceRecord, ProgramLayer};
use continuum_value::assurance::InconclusiveReason;
use continuum_value::identity::ContentHasher;

use crate::handle::{RepairId, SnapshotId};
use crate::receipt::{IntentRegistry, IntentStanding, SealedSnapshots, SnapshotRole};
use crate::transaction::{GateStatus, RepairTransaction, Resolution};

// --- the evidence-scope seam -----------------------------------------------------

/// The four identities whose evidence the impact set must cover (RFC 0031, "Impact
/// set": "every evidence artifact that names either intent identity or either
/// snapshot"), and the transaction version the scope is taken as of.
///
/// The impact set is part of the artifact, and so of its handle. Evidence recorded
/// after a version (gates 4 to 11 record evidence on the candidate) would move both, so
/// the store MUST answer the scope as of `repair`: the evidence recorded before that
/// version was published. Then the diff is a function of the version, and a promote-time
/// recomputation over the same version reproduces the recorded handle.
#[derive(Debug, Clone, Copy)]
pub struct DiffScope<'a> {
    /// The transaction version the scope is taken as of.
    pub repair: &'a RepairId,
    /// The base snapshot.
    pub before_snapshot: &'a SnapshotId,
    /// The candidate snapshot.
    pub after_snapshot: &'a SnapshotId,
    /// The base intent.
    pub before_intent: &'a IntentId,
    /// The intent the candidate is bound to.
    pub after_intent: &'a IntentId,
}

/// The evidence store's answer for a [`DiffScope`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeAnswer {
    /// Every evidence artifact in the scope, with its recorded dependency facts. An
    /// empty list means the store holds no such artifact.
    Complete(Vec<EvidenceRecord>),
    /// The store did not answer. Not an empty scope (INV-008).
    Unavailable,
}

/// The daemon's evidence store, as the impact set reads it.
pub trait ImpactScope {
    /// The evidence that names any identity in `scope`.
    fn evidence(&self, scope: DiffScope<'_>) -> ScopeAnswer;
}

// --- undiffable ------------------------------------------------------------------

/// Which store did not answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiffStore {
    /// [`SealedSnapshots`].
    Snapshots,
    /// [`IntentRegistry`].
    IntentRegistry,
    /// [`ImpactScope`].
    Evidence,
}

/// Why no diff was computed. A typed inconclusive, never "no change" (INV-008). No
/// variant carries caller text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Undiffable {
    /// The transaction is at `draft`: there is no candidate to compare.
    NoCandidate,
    /// A snapshot is not among the sealed snapshots. RFC 0031: the diff "MUST NOT
    /// classify against a mutable input" (`StaleSnapshot` on the wire).
    SnapshotUnresolved(SnapshotRole),
    /// The base snapshot is bound to an intent other than the transaction's
    /// `base_intent`. The frozen base triple and the store disagree.
    BaseBoundElsewhere,
    /// An intent does not resolve in the registry.
    IntentUnregistered(SnapshotRole),
    /// The base intent is a proposal: it never had INV-001 protection, so there is no
    /// protected intent to classify against.
    BaseNotProtected,
    /// The base intent was superseded: a verdict against it is stale (RFC 0037 P7), and
    /// the repair must be re-based on the current head.
    BaseSuperseded,
    /// The registry returned a contract that declares a different `intent_id` from the
    /// handle it was filed under. A store defect, refused rather than trusted.
    RegistryInconsistent(SnapshotRole),
    /// A store did not answer.
    StoreUnavailable(DiffStore),
    /// The assembled artifact did not render as an object carrying `diff_id`, so no
    /// content identity can be minted for it.
    Unrenderable,
    /// The classifier refused the artifact: an incomplete classification (P1), a
    /// `review` verb with no reviewer (W7), an inadmissible relation, or a malformed
    /// evidence scope. RFC 0031: no partial emission.
    Unclassifiable(AssembleError),
}

impl Undiffable {
    /// The INV-008 reason gate 3 records, or `None` at `draft`, where the gate is not
    /// yet evaluable and stays `pending`.
    ///
    /// A candidate-side input that is missing, or a store that is silent, is
    /// `InsufficientTelemetry`: the stores cannot supply what the classification reads.
    /// A base-side input that no longer resolves (`begin` resolved it), a store that
    /// contradicts itself, or a classifier that cannot complete is `EngineError`: a
    /// defect of the verifier, not a fact about the repair. A base intent without
    /// current protection is `Unsupported`: there is no protected intent to classify
    /// against.
    #[must_use]
    pub const fn inconclusive_reason(&self) -> Option<InconclusiveReason> {
        match self {
            Self::NoCandidate => None,
            Self::SnapshotUnresolved(SnapshotRole::After)
            | Self::IntentUnregistered(SnapshotRole::After)
            | Self::StoreUnavailable(_) => Some(InconclusiveReason::InsufficientTelemetry),
            Self::BaseNotProtected | Self::BaseSuperseded => Some(InconclusiveReason::Unsupported),
            Self::SnapshotUnresolved(SnapshotRole::Before)
            | Self::IntentUnregistered(SnapshotRole::Before)
            | Self::BaseBoundElsewhere
            | Self::RegistryInconsistent(_)
            | Self::Unrenderable
            | Self::Unclassifiable(_) => Some(InconclusiveReason::EngineError),
        }
    }
}

impl fmt::Display for Undiffable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoCandidate => f.write_str("the transaction has no candidate to diff"),
            Self::SnapshotUnresolved(role) => write!(f, "the {role:?} snapshot is not sealed"),
            Self::BaseBoundElsewhere => {
                f.write_str("the base snapshot is bound to an intent other than base_intent")
            }
            Self::IntentUnregistered(role) => {
                write!(f, "the {role:?} intent does not resolve in the registry")
            }
            Self::BaseNotProtected => f.write_str("the base intent is a proposal"),
            Self::BaseSuperseded => f.write_str("the base intent was superseded"),
            Self::RegistryInconsistent(role) => write!(
                f,
                "the registry's {role:?} contract declares a different intent_id"
            ),
            Self::StoreUnavailable(store) => write!(f, "the {store:?} store did not answer"),
            Self::Unrenderable => f.write_str("the diff artifact did not render"),
            Self::Unclassifiable(error) => write!(f, "the diff could not be classified: {error}"),
        }
    }
}

impl std::error::Error for Undiffable {}

// --- classification --------------------------------------------------------------

/// The intent-weakening kinds a record can carry. The first six are the sentence of
/// INV-001 and RFC 0032; the next three are the other gaming moves of RFC 0031's
/// completeness table that an affirmative relation names; the last is RFC 0031
/// correction 20's loosening of the accepted evidence classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Weakening {
    /// `properties` `weakened` or `removed`.
    WeakenedProperty,
    /// `assumptions` `strengthened` or `added`.
    StrengthenedAssumption,
    /// `bounds` `contracted`.
    ShrunkBound,
    /// `observers` `coarsened` or `removed`.
    HiddenEvent,
    /// `faults` `removed`.
    RemovedFault,
    /// `assurance` `downgraded`.
    DowngradedAssurance,
    /// `fairness` `added`, `removed`, `strengthened` or `weakened`: a fairness
    /// constraint that schedules the bug away, or one that is dropped.
    FairnessChanged,
    /// `trust_boundaries` `expanded`: an effect marked opaque.
    OpaqueEscape,
    /// `abstraction_maps` `merged`.
    AbstractionMerged,
    /// `assurance` `added`: a newly accepted evidence class loosens what satisfies
    /// the intent (RFC 0031 correction 20).
    LoosenedEvidenceClasses,
}

impl Weakening {
    /// The weakening `relation` on `field` is, if any.
    #[must_use]
    pub const fn of(field: PolicyField, relation: Relation) -> Option<Self> {
        match (field, relation) {
            (PolicyField::Properties, Relation::Weakened | Relation::Removed) => {
                Some(Self::WeakenedProperty)
            }
            (PolicyField::Assumptions, Relation::Strengthened | Relation::Added) => {
                Some(Self::StrengthenedAssumption)
            }
            (PolicyField::Bounds, Relation::Contracted) => Some(Self::ShrunkBound),
            (PolicyField::Observers, Relation::Coarsened | Relation::Removed) => {
                Some(Self::HiddenEvent)
            }
            (PolicyField::Faults, Relation::Removed) => Some(Self::RemovedFault),
            (PolicyField::Assurance, Relation::Downgraded) => Some(Self::DowngradedAssurance),
            (
                PolicyField::Fairness,
                Relation::Added | Relation::Removed | Relation::Strengthened | Relation::Weakened,
            ) => Some(Self::FairnessChanged),
            (PolicyField::TrustBoundaries, Relation::Expanded) => Some(Self::OpaqueEscape),
            (PolicyField::AbstractionMaps, Relation::Merged) => Some(Self::AbstractionMerged),
            (PolicyField::Assurance, Relation::Added) => Some(Self::LoosenedEvidenceClasses),
            _ => None,
        }
    }
}

/// What one protected change is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChangeClass {
    /// An intent weakening.
    Weakening(Weakening),
    /// `unknown`, `unsupported` or `incomparable`: the direction is not decided, and
    /// RFC 0031 treats it exactly as a confirmed protected change.
    Undecided,
    /// Any other affirmative change to a protected field. Still an intent change.
    Other,
}

/// One `intent_changes[]` record, typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtectedChange {
    field: PolicyField,
    relation: Relation,
    unit: Option<String>,
    class: ChangeClass,
}

impl ProtectedChange {
    fn of(record: &IntentChangeRecord) -> Self {
        let (field, relation) = (record.field(), record.relation());
        let class = match relation {
            Relation::Unknown | Relation::Unsupported | Relation::Incomparable => {
                ChangeClass::Undecided
            }
            _ => Weakening::of(field, relation).map_or(ChangeClass::Other, ChangeClass::Weakening),
        };
        Self {
            field,
            relation,
            unit: record.unit().map(str::to_owned),
            class,
        }
    }

    /// The protected field.
    #[must_use]
    pub const fn field(&self) -> PolicyField {
        self.field
    }

    /// The relation the classifier recorded.
    #[must_use]
    pub const fn relation(&self) -> Relation {
        self.relation
    }

    /// The per-unit locator, where the field has one.
    #[must_use]
    pub fn unit(&self) -> Option<&str> {
        self.unit.as_deref()
    }

    /// What the change is.
    #[must_use]
    pub const fn class(&self) -> ChangeClass {
        self.class
    }
}

/// A privileged intent revision: what a repair transaction is when its candidate
/// changes protected intent. It is blocked as a repair and may only proceed through
/// RFC 0037's revision procedure, under `revise-intent` authority, as a new transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentRevision {
    rebinds: bool,
    changes: Vec<ProtectedChange>,
    decision: PolicyDecision,
}

impl IntentRevision {
    /// Whether the candidate is bound to an intent other than `base_intent`.
    #[must_use]
    pub const fn rebinds(&self) -> bool {
        self.rebinds
    }

    /// Every protected change, in the artifact's wire order.
    #[must_use]
    pub fn changes(&self) -> &[ProtectedChange] {
        &self.changes
    }

    /// The weakenings among the changes, each once, in [`Weakening`] order.
    #[must_use]
    pub fn weakenings(&self) -> Vec<Weakening> {
        let mut out: Vec<Weakening> = self
            .changes
            .iter()
            .filter_map(|change| match change.class {
                ChangeClass::Weakening(kind) => Some(kind),
                ChangeClass::Undecided | ChangeClass::Other => None,
            })
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// The artifact's `policy.decision` under the base contract's governance.
    #[must_use]
    pub const fn decision(&self) -> PolicyDecision {
        self.decision
    }
}

/// What a transaction is, on its computed diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntentClassification {
    /// The candidate is bound to `base_intent`, no protected field changed, and the
    /// recomputed decision is `allow`. Intent-preserving only: gate 3 passes on it only
    /// when the program-side layer is also classified ([`DiffOutcome::gate_status`]).
    Repair,
    /// Anything else.
    PrivilegedIntentRevision(IntentRevision),
}

// --- the computed diff -----------------------------------------------------------

/// The diff of one transaction version, computed from the stores.
///
/// Only [`compute`] builds one, so the artifact is always the one the stores determine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionDiff {
    repair: RepairId,
    handle: DiffId,
    artifact: DiffArtifact,
    classification: IntentClassification,
}

impl TransactionDiff {
    /// The transaction version the diff was computed for.
    #[must_use]
    pub const fn repair(&self) -> &RepairId {
        &self.repair
    }

    /// The `diff_*` handle: the value of the version's `semantic_diff` field, and of a
    /// receipt's `semantic_diff` and `intent_diff`.
    #[must_use]
    pub fn handle(&self) -> &str {
        self.handle.as_str()
    }

    /// The RFC 0031 artifact.
    #[must_use]
    pub const fn artifact(&self) -> &DiffArtifact {
        &self.artifact
    }

    /// The artifact's canonical bytes.
    #[must_use]
    pub fn to_artifact_bytes(&self) -> Vec<u8> {
        self.artifact.to_artifact_bytes()
    }

    /// Repair or privileged intent revision.
    #[must_use]
    pub const fn classification(&self) -> &IntentClassification {
        &self.classification
    }

    /// Gate 3 on this diff: `failed` for a privileged intent revision; for a repair,
    /// `passed` only when the program-side layer is classified, else `inconclusive`.
    #[must_use]
    pub const fn gate_status(&self) -> GateStatus {
        match self.classification {
            IntentClassification::PrivilegedIntentRevision(_) => GateStatus::Failed,
            IntentClassification::Repair => match self.artifact.program_layer() {
                ProgramLayer::Classified => GateStatus::Passed,
                ProgramLayer::Unclassified => GateStatus::Inconclusive,
            },
        }
    }

    /// The INV-008 reason gate 3 records when [`Self::gate_status`] is `inconclusive`:
    /// a repair over an unclassified program change is `Unsupported`.
    #[must_use]
    pub const fn inconclusive_reason(&self) -> Option<InconclusiveReason> {
        match (&self.classification, self.artifact.program_layer()) {
            (IntentClassification::Repair, ProgramLayer::Unclassified) => {
                Some(InconclusiveReason::Unsupported)
            }
            _ => None,
        }
    }
}

/// What [`compute`] found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffOutcome {
    /// The diff and its classification.
    Computed(Box<TransactionDiff>),
    /// No diff: a typed inconclusive.
    Inconclusive(Undiffable),
}

impl DiffOutcome {
    /// Gate 3, `intent_integrity`, on this outcome:
    ///
    /// - `failed` for a privileged intent revision;
    /// - for a repair, `passed` only when the program-side layer is classified, and
    ///   `inconclusive` when it is not, because an `allow` must not rest on program
    ///   changes nobody classified;
    /// - `inconclusive` when no diff could be computed, and `pending` at `draft`.
    ///
    /// No program-side classifier exists yet, so a repair whose candidate differs from
    /// its base is `inconclusive` today: gate 3 passes only for a candidate that is
    /// byte-identical to its base.
    #[must_use]
    pub const fn gate_status(&self) -> GateStatus {
        match self {
            Self::Computed(diff) => diff.gate_status(),
            Self::Inconclusive(Undiffable::NoCandidate) => GateStatus::Pending,
            Self::Inconclusive(_) => GateStatus::Inconclusive,
        }
    }

    /// The INV-008 reason gate 3 records when [`Self::gate_status`] is
    /// `inconclusive`, and `None` otherwise. A repair over an unclassified program
    /// change is `Unsupported`: the program-side semantics are outside what any
    /// classifier here decides.
    #[must_use]
    pub const fn inconclusive_reason(&self) -> Option<InconclusiveReason> {
        match self {
            Self::Computed(diff) => diff.inconclusive_reason(),
            Self::Inconclusive(reason) => reason.inconclusive_reason(),
        }
    }
}

/// Compute the diff of `transaction` between its base and its candidate.
///
/// Every input is resolved from the stores through the transaction's own fields. The
/// diff's identity is minted under `H`, the transaction's hasher.
pub fn compute<H: ContentHasher>(
    transaction: &RepairTransaction<H>,
    snapshots: &impl SealedSnapshots,
    registry: &impl IntentRegistry,
    evidence: &impl ImpactScope,
) -> DiffOutcome {
    match compute_inner(transaction, snapshots, registry, evidence) {
        Ok(diff) => DiffOutcome::Computed(Box::new(diff)),
        Err(reason) => DiffOutcome::Inconclusive(reason),
    }
}

/// Everything the diff reads, resolved from the stores.
struct Resolved {
    repair: RepairId,
    base: SnapshotId,
    candidate: SnapshotId,
    base_intent: IntentId,
    after_intent: IntentId,
    before: IntentContract,
    after: IntentContract,
    records: Vec<EvidenceRecord>,
    rebinds: bool,
    program: ProgramLayer,
}

impl Resolved {
    fn scope(&self) -> DiffScope<'_> {
        DiffScope {
            repair: &self.repair,
            before_snapshot: &self.base,
            after_snapshot: &self.candidate,
            before_intent: &self.base_intent,
            after_intent: &self.after_intent,
        }
    }

    /// The diff under `program`, minted under `H`.
    fn build<H: ContentHasher>(
        &self,
        program: ProgramLayer,
    ) -> Result<TransactionDiff, Undiffable> {
        let (handle, artifact) = assemble_minted::<H>(
            self.scope(),
            &self.before,
            &self.after,
            self.records.clone(),
            program,
        )?;
        let classification = classify(&artifact, self.rebinds);
        Ok(TransactionDiff {
            repair: self.repair.clone(),
            handle,
            artifact,
            classification,
        })
    }
}

fn compute_inner<H: ContentHasher>(
    transaction: &RepairTransaction<H>,
    snapshots: &impl SealedSnapshots,
    registry: &impl IntentRegistry,
    evidence: &impl ImpactScope,
) -> Result<TransactionDiff, Undiffable> {
    let resolved = resolve(
        transaction,
        snapshots,
        registry,
        evidence,
        BaseStanding::Current,
    )?;
    resolved.build::<H>(resolved.program)
}

/// Which standings of the base intent a diff may be computed against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BaseStanding {
    /// Only the protected head: every new decision ([`compute`], promotion).
    Current,
    /// Also a base intent superseded after promotion: re-deriving the diff a published
    /// receipt was decided against (RFC 0032: published receipts remain verifiable).
    /// The artifact does not carry the standing, so the bytes are the ones promotion
    /// computed.
    Historical,
}

fn resolve<H: ContentHasher>(
    transaction: &RepairTransaction<H>,
    snapshots: &impl SealedSnapshots,
    registry: &impl IntentRegistry,
    evidence: &impl ImpactScope,
    admit: BaseStanding,
) -> Result<Resolved, Undiffable> {
    let candidate = transaction
        .candidate_snapshot()
        .ok_or(Undiffable::NoCandidate)?;
    let base = transaction.base_snapshot();
    let base_intent = transaction.base_intent();

    if &binding(snapshots, base, SnapshotRole::Before)? != base_intent {
        return Err(Undiffable::BaseBoundElsewhere);
    }
    let after_intent = binding(snapshots, candidate, SnapshotRole::After)?;

    let (before, standing) = contract(registry, base_intent, SnapshotRole::Before)?;
    match (standing, admit) {
        (IntentStanding::Accepted, _) | (IntentStanding::Superseded, BaseStanding::Historical) => {}
        (IntentStanding::Proposed, _) => return Err(Undiffable::BaseNotProtected),
        (IntentStanding::Superseded, BaseStanding::Current) => {
            return Err(Undiffable::BaseSuperseded);
        }
    }
    let rebinds = &after_intent != base_intent;
    let after = if rebinds {
        contract(registry, &after_intent, SnapshotRole::After)?.0
    } else {
        before.clone()
    };

    let scope = DiffScope {
        repair: transaction.repair_id(),
        before_snapshot: base,
        after_snapshot: candidate,
        before_intent: base_intent,
        after_intent: &after_intent,
    };
    let records = match evidence.evidence(scope) {
        ScopeAnswer::Complete(records) => records,
        ScopeAnswer::Unavailable => return Err(Undiffable::StoreUnavailable(DiffStore::Evidence)),
    };

    // The program-side layer is classified only in the one case that needs no
    // classifier: a candidate identical to its base has no program change.
    let program = if candidate == base {
        ProgramLayer::Classified
    } else {
        ProgramLayer::Unclassified
    };
    Ok(Resolved {
        repair: transaction.repair_id().clone(),
        base: base.clone(),
        candidate: candidate.clone(),
        base_intent: base_intent.clone(),
        after_intent,
        before,
        after,
        records,
        rebinds,
        program,
    })
}

fn binding(
    snapshots: &impl SealedSnapshots,
    snapshot: &SnapshotId,
    role: SnapshotRole,
) -> Result<IntentId, Undiffable> {
    match snapshots.sealed_binding(snapshot) {
        Resolution::Found(intent) => Ok(intent),
        Resolution::Unknown => Err(Undiffable::SnapshotUnresolved(role)),
        Resolution::Unavailable => Err(Undiffable::StoreUnavailable(DiffStore::Snapshots)),
    }
}

fn contract(
    registry: &impl IntentRegistry,
    intent: &IntentId,
    role: SnapshotRole,
) -> Result<(IntentContract, IntentStanding), Undiffable> {
    match registry.registered(intent) {
        Resolution::Found(record) => {
            if record.contract().intent_id() == intent {
                Ok((record.contract().clone(), record.standing()))
            } else {
                Err(Undiffable::RegistryInconsistent(role))
            }
        }
        Resolution::Unknown => Err(Undiffable::IntentUnregistered(role)),
        Resolution::Unavailable => Err(Undiffable::StoreUnavailable(DiffStore::IntentRegistry)),
    }
}

/// The domain-separation tag of a diff handle's preimage under `program`. Versioned,
/// so a later change to the layer vocabulary mints new identities rather than reusing
/// these.
#[must_use]
pub const fn layer_tag(program: ProgramLayer) -> &'static str {
    match program {
        ProgramLayer::Classified => "semantic-diff/program-layer/classified/v1",
        ProgramLayer::Unclassified => "semantic-diff/program-layer/unclassified/v1",
    }
}

/// The `diff_*` handle of an artifact whose canonical bytes without `diff_id` are
/// `preimage`, assembled under `program`: `diff_` plus the digest of
/// `layer_tag(program) || 0x00 || preimage`.
///
/// The layer is part of the identity because the schema has no field for it: two
/// artifacts with equal wire bytes but different program layers have different
/// handles, and since the handle is the artifact's `diff_id`, different bytes too. The
/// tags contain no NUL byte and form a fixed set, so the tagged preimage is injective
/// in `(program, preimage)`.
#[must_use]
pub fn mint_handle<H: ContentHasher>(program: ProgramLayer, preimage: &[u8]) -> String {
    let tag = layer_tag(program).as_bytes();
    let mut tagged = Vec::with_capacity(tag.len() + 1 + preimage.len());
    tagged.extend_from_slice(tag);
    tagged.push(0);
    tagged.extend_from_slice(preimage);
    format!("diff_{}", H::hash(&tagged).to_token())
}

/// The placeholder the identity preimage is assembled under; it never reaches a
/// returned artifact, because the `diff_id` key is dropped from the preimage.
const UNMINTED: &str = "diff_unminted";

/// Assemble the artifact twice: once to derive its content identity from the canonical
/// bytes without `diff_id`, and once under the minted handle. `assemble` is a pure
/// function of its request (RFC 0031 determinism), so the two agree on every other byte.
fn assemble_minted<H: ContentHasher>(
    scope: DiffScope<'_>,
    before: &IntentContract,
    after: &IntentContract,
    evidence: Vec<EvidenceRecord>,
    program: ProgramLayer,
) -> Result<(DiffId, DiffArtifact), Undiffable> {
    let mut request = DiffRequest {
        diff_id: DiffId::new(UNMINTED).map_err(Undiffable::Unclassifiable)?,
        before_snapshot: artifact::SnapshotId::new(scope.before_snapshot.as_str())
            .map_err(Undiffable::Unclassifiable)?,
        after_snapshot: artifact::SnapshotId::new(scope.after_snapshot.as_str())
            .map_err(Undiffable::Unclassifiable)?,
        before_intent: scope.before_intent.clone(),
        after_intent: scope.after_intent.clone(),
        before,
        after,
        requested_assurance: before.assurance().minimum().max(AssuranceLevel::Validated),
        path: AcceptancePath::AgentAccept,
        semantic_changes: Vec::new(),
        evidence,
    };
    let draft = artifact::assemble_under(&request, program).map_err(Undiffable::Unclassifiable)?;
    // The artifact renders as an object with a `diff_id` key, or the preimage is not
    // the artifact's and no handle is minted.
    let Json::Object(mut fields) = draft.artifact_json() else {
        return Err(Undiffable::Unrenderable);
    };
    if fields.remove("diff_id").is_none() {
        return Err(Undiffable::Unrenderable);
    }
    let minted = DiffId::new(&mint_handle::<H>(
        program,
        &Json::Object(fields).to_canonical_bytes(),
    ))
    .map_err(Undiffable::Unclassifiable)?;
    request.diff_id = minted.clone();
    let artifact =
        artifact::assemble_under(&request, program).map_err(Undiffable::Unclassifiable)?;
    Ok((minted, artifact))
}

fn classify(artifact: &DiffArtifact, rebinds: bool) -> IntentClassification {
    let changes: Vec<ProtectedChange> = artifact
        .intent_changes()
        .iter()
        .map(ProtectedChange::of)
        .collect();
    let decision = artifact.decision();
    if !rebinds && changes.is_empty() && decision == PolicyDecision::Allow {
        IntentClassification::Repair
    } else {
        IntentClassification::PrivilegedIntentRevision(IntentRevision {
            rebinds,
            changes,
            decision,
        })
    }
}

// --- claimed diffs ---------------------------------------------------------------

/// Why a claimed diff was refused. No variant carries the claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffClaimRefusal {
    /// The claim's bytes are not the computed artifact's canonical bytes.
    Disagrees,
    /// The claimed handle is not the computed artifact's handle.
    HandleMismatch,
    /// The claim is the diff of these same inputs under the other program layer: a
    /// cached or forged artifact whose layer is not the one the stores determine
    /// today. A `classified` claim for a transaction the stores leave unclassified is
    /// this refusal, never a pass.
    LayerMismatch {
        /// The layer the stores determine.
        computed: ProgramLayer,
    },
    /// No diff could be computed, so no claim can be checked against one.
    Undiffable(Undiffable),
}

impl fmt::Display for DiffClaimRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disagrees => f.write_str("the claimed diff is not the computed diff"),
            Self::HandleMismatch => f.write_str("the claimed diff handle is not the computed one"),
            Self::LayerMismatch { computed } => write!(
                f,
                "the claimed diff was minted under the other program layer; the stores determine {computed:?}"
            ),
            Self::Undiffable(reason) => write!(f, "the claim cannot be checked: {reason}"),
        }
    }
}

impl std::error::Error for DiffClaimRefusal {}

/// Recompute the diff of `transaction` and accept `claimed` only if it is exactly the
/// computed artifact's canonical bytes. The claim is compared, never parsed, so it
/// contributes nothing but equality. The program layer is derived from the stores,
/// never from the claim, and is bound into the bytes through the handle.
///
/// # Errors
///
/// [`DiffClaimRefusal::LayerMismatch`] when the claim is the diff of these inputs
/// under the other program layer, [`DiffClaimRefusal::Disagrees`] for any other bytes,
/// and [`DiffClaimRefusal::Undiffable`] when no diff can be computed.
pub fn verify_claimed_diff<H: ContentHasher>(
    transaction: &RepairTransaction<H>,
    claimed: &[u8],
    snapshots: &impl SealedSnapshots,
    registry: &impl IntentRegistry,
    evidence: &impl ImpactScope,
) -> Result<TransactionDiff, DiffClaimRefusal> {
    verify(transaction, snapshots, registry, evidence, |diff| {
        diff.to_artifact_bytes() == claimed
    })
    .map_err(|refusal| match refusal {
        DiffClaimRefusal::HandleMismatch => DiffClaimRefusal::Disagrees,
        other => other,
    })
}

/// Recompute the diff of `transaction` and accept `claimed` only if it is the computed
/// artifact's handle, which binds the program layer the stores determine.
///
/// # Errors
///
/// [`DiffClaimRefusal::LayerMismatch`] when `claimed` is the handle of these inputs
/// under the other program layer, [`DiffClaimRefusal::HandleMismatch`] for any other
/// handle, and [`DiffClaimRefusal::Undiffable`] when no diff can be computed.
pub fn verify_claimed_handle<H: ContentHasher>(
    transaction: &RepairTransaction<H>,
    claimed: &str,
    snapshots: &impl SealedSnapshots,
    registry: &impl IntentRegistry,
    evidence: &impl ImpactScope,
) -> Result<TransactionDiff, DiffClaimRefusal> {
    verify(transaction, snapshots, registry, evidence, |diff| {
        diff.handle() == claimed
    })
}

/// Gate 3 for a cached or persisted diff artifact.
///
/// This is the only way from stored bytes to a gate status. Nothing is read from the
/// bytes: the diff is recomputed from the stores, and the cached bytes are accepted
/// only if they are exactly the recomputed artifact, whose handle binds the program
/// layer. A cached artifact from before a layer change, or one that claims a layer
/// the stores do not determine, is refused, so a persisted `classified` artifact can
/// never yield gate 3 `passed` for a transaction the stores leave unclassified. An
/// artifact whose layer cannot be established this way yields no gate status at all.
///
/// # Errors
///
/// As [`verify_claimed_diff`].
pub fn gate_status_of_cached<H: ContentHasher>(
    transaction: &RepairTransaction<H>,
    cached: &[u8],
    snapshots: &impl SealedSnapshots,
    registry: &impl IntentRegistry,
    evidence: &impl ImpactScope,
) -> Result<GateStatus, DiffClaimRefusal> {
    let diff = verify_claimed_diff(transaction, cached, snapshots, registry, evidence)?;
    Ok(DiffOutcome::Computed(Box::new(diff)).gate_status())
}

/// Recompute under the layer the stores determine; on a mismatch, recompute under the
/// other layer only to name the refusal.
fn verify<H: ContentHasher>(
    transaction: &RepairTransaction<H>,
    snapshots: &impl SealedSnapshots,
    registry: &impl IntentRegistry,
    evidence: &impl ImpactScope,
    matches: impl Fn(&TransactionDiff) -> bool,
) -> Result<TransactionDiff, DiffClaimRefusal> {
    let (diff, basis) = compute_with_basis(
        transaction,
        snapshots,
        registry,
        evidence,
        BaseStanding::Current,
    )
    .map_err(DiffClaimRefusal::Undiffable)?;
    basis.check::<H>(&diff, matches)?;
    Ok(diff)
}

/// The inputs one diff was computed from, kept so that a claim that does not match it
/// can be named without reading the stores again.
pub(crate) struct DiffBasis(Resolved);

impl DiffBasis {
    /// The content identity of the base contract the diff read.
    pub(crate) const fn before_identity(&self) -> &continuum_intent::contract::IntentIdentity {
        self.0.before.identity()
    }

    /// The intent the candidate was bound to when the diff read it.
    pub(crate) const fn after_intent(&self) -> &IntentId {
        &self.0.after_intent
    }

    /// `Ok` when `matches` accepts `diff`. Otherwise the refusal: `LayerMismatch` when
    /// it accepts the diff of these same inputs under the other program layer, else
    /// `HandleMismatch`.
    pub(crate) fn check<H: ContentHasher>(
        &self,
        diff: &TransactionDiff,
        matches: impl Fn(&TransactionDiff) -> bool,
    ) -> Result<(), DiffClaimRefusal> {
        if matches(diff) {
            return Ok(());
        }
        let resolved = &self.0;
        let other = match resolved.program {
            ProgramLayer::Classified => ProgramLayer::Unclassified,
            ProgramLayer::Unclassified => ProgramLayer::Classified,
        };
        match resolved.build::<H>(other) {
            Ok(alternate) if matches(&alternate) => Err(DiffClaimRefusal::LayerMismatch {
                computed: resolved.program,
            }),
            _ => Err(DiffClaimRefusal::HandleMismatch),
        }
    }
}

/// [`compute`] for the receipt (PR-22 / IMPL-03): the diff of `transaction` under the
/// base standing `admit`, with the basis it was computed from.
pub(crate) fn compute_with_basis<H: ContentHasher>(
    transaction: &RepairTransaction<H>,
    snapshots: &impl SealedSnapshots,
    registry: &impl IntentRegistry,
    evidence: &impl ImpactScope,
    admit: BaseStanding,
) -> Result<(TransactionDiff, DiffBasis), Undiffable> {
    let resolved = resolve(transaction, snapshots, registry, evidence, admit)?;
    let diff = resolved.build::<H>(resolved.program)?;
    Ok((diff, DiffBasis(resolved)))
}

#[cfg(test)]
mod tests {
    use continuum_intent::contract::IntentContract;

    use super::*;

    const CONTRACT: &str =
        include_str!("../../continuum-intent/tests/fixtures/replicated-register-contract.json");

    fn artifact_between(before: &IntentContract, after: &IntentContract) -> DiffArtifact {
        artifact::assemble(&DiffRequest {
            diff_id: DiffId::new("diff_unit").expect("a diff_ handle"),
            before_snapshot: artifact::SnapshotId::new("ws_before").expect("ws_"),
            after_snapshot: artifact::SnapshotId::new("ws_after").expect("ws_"),
            before_intent: before.intent_id().clone(),
            after_intent: after.intent_id().clone(),
            before,
            after,
            requested_assurance: AssuranceLevel::Validated,
            path: AcceptancePath::AgentAccept,
            semantic_changes: Vec::new(),
            evidence: Vec::new(),
        })
        .expect("assembles")
    }

    /// `classify` decides on the artifact as well as on the binding: an artifact with a
    /// protected change is a revision even when the binding did not move.
    #[test]
    fn classify_needs_no_change_and_allow_besides_the_binding() {
        let base = IntentContract::decode(CONTRACT.trim_end().as_bytes()).expect("decodes");
        let shrunk = IntentContract::decode(
            CONTRACT
                .trim_end()
                .replacen(r#""nodes":3"#, r#""nodes":2"#, 1)
                .as_bytes(),
        )
        .expect("decodes");
        let expanded = IntentContract::decode(
            CONTRACT
                .trim_end()
                .replacen(r#""nodes":3"#, r#""nodes":4"#, 1)
                .as_bytes(),
        )
        .expect("decodes");

        assert_eq!(
            classify(&artifact_between(&base, &base), false),
            IntentClassification::Repair
        );
        // Non-empty and `block`.
        let IntentClassification::PrivilegedIntentRevision(revision) =
            classify(&artifact_between(&base, &shrunk), false)
        else {
            panic!("a shrunk bound is not a repair");
        };
        assert!(!revision.rebinds());
        assert_eq!(revision.weakenings(), [Weakening::ShrunkBound]);
        // Non-empty and `allow`: still a change to protected intent.
        let IntentClassification::PrivilegedIntentRevision(revision) =
            classify(&artifact_between(&base, &expanded), false)
        else {
            panic!("an expanded bound is not a repair");
        };
        assert_eq!(revision.decision(), PolicyDecision::Allow);
        assert!(revision.weakenings().is_empty());
        // Empty and `allow`, but rebound.
        assert!(matches!(
            classify(&artifact_between(&base, &base), true),
            IntentClassification::PrivilegedIntentRevision(_)
        ));
    }
}
