//! The repair-transaction skeleton, as far as the hypothesis needs it: `begin` opens a
//! `draft` v1 on a frozen base triple, and `apply` records a proposal's hypothesis and
//! changes as a new `applied` version. Canonical bytes and content identity follow
//! ADR-0013; the shape is `notes/plan/schemas/repair-transaction.schema.json`
//! (`https://continuum.dev/schema/repair-transaction.json`, epoch 1).
//!
//! # Clause → mechanism
//!
//! | Clause | Source | Mechanism |
//! |---|---|---|
//! | `begin` takes a crashpack and a gate profile, nothing else | IDL `repair.begin`; RFC 0032 correction 2 | [`RepairTransaction::begin`] resolves the base snapshot from the crashpack and the base intent from that snapshot's binding through [`FailureBinding`] |
//! | a failure that does not resolve opens nothing | RFC 0032 totality: a refusal publishes nothing | [`BeginRefusal`], typed; an unavailable binding is [`BeginRefusal::BindingUnavailable`], never read as "unknown" (INV-008) |
//! | the base triple is frozen | RFC 0032, "The transaction object" | `apply` copies it; no setter exists |
//! | `gate_profile` is declared at `begin` and frozen | RFC 0032 correction 4 | as above |
//! | twelve gates by identity at every status; out-of-profile gates `not_yet_enforced`, in-profile gates never | schema `gates` and profile conditionals | [`GateProfile::enforces`]; every gate starts `pending` or `not_yet_enforced` |
//! | `candidate_snapshot` null exactly at `draft` | RFC 0032 | [`RepairTransaction::status`] derives `draft` from its absence |
//! | `version` is 1 iff `supersedes` is absent, else predecessor + 1 | RFC 0032, "Version arithmetic" | only `begin` and `apply` build a version |
//! | `repair_id` is the content identity of the version, excluded from its own preimage | RFC 0032 | [`RepairTransaction::preimage_json`] omits it; [`RepairIdentity`] is those bytes |
//! | an `intent` change through ordinary repair authority fails `IntentMutationDenied` at `apply` | RFC 0032, "Intent integrity and reclassification"; INV-011 | [`ApplyRefusal::IntentMutationDenied`] |
//! | the hypothesis is never interpolated into a typed field or error text | RFC 0032 correction 12 | it is written to `hypothesis` only; no refusal carries it |
//! | status is derived, never asserted | RFC 0032, "Status machine" | no constructor takes a status |
//!
//! # Canonical encoding
//!
//! One spelling: `continuum_intent::canonical_json` (RFC 0037 ID5) — keys sorted by
//! code point, no whitespace, integers only. Optional fields the skeleton never reaches
//! (`semantic_diff`, `policy_verdict`, `receipt`, `evaluation_policy`, `actor`, …) are
//! omitted, never written as `null`. `changes` and each gate's `evidence` are always
//! written, as `[]` when empty, so an absent list and an empty one cannot share a
//! spelling.

use core::fmt;
use core::marker::PhantomData;
use std::collections::BTreeMap;

use continuum_intent::canonical_json::Json;
use continuum_intent::contract::IntentId;
use continuum_value::identity::ContentHasher;

use crate::handle::{CrashpackId, RepairId, SnapshotId};
use crate::hypothesis::{Change, ChangeKind, Hypothesis, Proposal};

/// The artifact-class identity of the governing schema (`schema_id`).
pub const SCHEMA_ID: &str = "https://continuum.dev/schema/repair-transaction.json";

/// The schema epoch this crate writes (`schema_epoch`).
pub const SCHEMA_EPOCH: i64 = 1;

// --- closed vocabularies ---------------------------------------------------------

/// The twelve gates of plan §8.2, by schema identity (`$defs.gate_name`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GateName {
    /// Gate 1.
    BaseReplay,
    /// Gate 2.
    PatchApplication,
    /// Gate 3.
    IntentIntegrity,
    /// Gate 4.
    ExactRegression,
    /// Gate 5.
    Neighborhood,
    /// Gate 6.
    PropertyMutation,
    /// Gate 7.
    DefectMutants,
    /// Gate 8.
    RefinementCoverage,
    /// Gate 9.
    CertificateRebuild,
    /// Gate 10.
    IncrementalParity,
    /// Gate 11.
    CodeAndSecurity,
    /// Gate 12.
    ReceiptGeneration,
}

impl GateName {
    /// Every gate, in the schema's enum order, which is gate-number order.
    pub const ALL: [Self; 12] = [
        Self::BaseReplay,
        Self::PatchApplication,
        Self::IntentIntegrity,
        Self::ExactRegression,
        Self::Neighborhood,
        Self::PropertyMutation,
        Self::DefectMutants,
        Self::RefinementCoverage,
        Self::CertificateRebuild,
        Self::IncrementalParity,
        Self::CodeAndSecurity,
        Self::ReceiptGeneration,
    ];

    /// The schema token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::BaseReplay => "base_replay",
            Self::PatchApplication => "patch_application",
            Self::IntentIntegrity => "intent_integrity",
            Self::ExactRegression => "exact_regression",
            Self::Neighborhood => "neighborhood",
            Self::PropertyMutation => "property_mutation",
            Self::DefectMutants => "defect_mutants",
            Self::RefinementCoverage => "refinement_coverage",
            Self::CertificateRebuild => "certificate_rebuild",
            Self::IncrementalParity => "incremental_parity",
            Self::CodeAndSecurity => "code_and_security",
            Self::ReceiptGeneration => "receipt_generation",
        }
    }
}

/// The five gate statuses. There is no `not_applicable` (plan §25 SD-11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GateStatus {
    /// The gate's claim holds on the evidence it references.
    Passed,
    /// The gate's claim is refuted.
    Failed,
    /// In the active profile, not yet evaluated.
    Pending,
    /// Evaluated; the evidence cannot decide.
    Inconclusive,
    /// Outside the active profile.
    NotYetEnforced,
}

impl GateStatus {
    /// Every status, in the schema's enum order.
    pub const ALL: [Self; 5] = [
        Self::Passed,
        Self::Failed,
        Self::Pending,
        Self::Inconclusive,
        Self::NotYetEnforced,
    ];

    /// The schema token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Pending => "pending",
            Self::Inconclusive => "inconclusive",
            Self::NotYetEnforced => "not_yet_enforced",
        }
    }
}

/// The phase-staged promotion profile (plan §21), declared at `begin` and frozen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GateProfile {
    /// Gates 1–8, 11, 12.
    PhaseB,
    /// Gates 1–8, 10, 11, 12.
    PhaseC,
    /// All twelve.
    PhaseD,
    /// All twelve.
    Default,
}

impl GateProfile {
    /// Every profile, in the schema's enum order.
    pub const ALL: [Self; 4] = [Self::PhaseB, Self::PhaseC, Self::PhaseD, Self::Default];

    /// The schema token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::PhaseB => "phase-b",
            Self::PhaseC => "phase-c",
            Self::PhaseD => "phase-d",
            Self::Default => "default",
        }
    }

    /// Whether `gate` is inside this profile. A gate outside it is listed
    /// `not_yet_enforced`; a gate inside it never is (the schema's conditionals).
    #[must_use]
    pub const fn enforces(self, gate: GateName) -> bool {
        match self {
            Self::PhaseB => !matches!(
                gate,
                GateName::CertificateRebuild | GateName::IncrementalParity
            ),
            Self::PhaseC => !matches!(gate, GateName::CertificateRebuild),
            Self::PhaseD | Self::Default => true,
        }
    }
}

/// The nine transaction statuses. This skeleton derives only [`Self::Draft`] and
/// [`Self::Applied`]; the others need gate outcomes, a verdict, or a lineage head.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TransactionStatus {
    /// No candidate snapshot yet.
    Draft,
    /// A candidate exists and no in-profile gate has been evaluated.
    Applied,
    /// A gate campaign is running or suspended.
    Evaluating,
    /// Every in-profile gate 1–11 passed and the verdict is `allow`.
    Ready,
    /// An in-profile gate failed, or the verdict is not `allow`.
    Blocked,
    /// An in-profile gate is inconclusive.
    Inconclusive,
    /// Terminal: promoted.
    Promoted,
    /// Terminal: rejected.
    Rejected,
    /// Not the lineage head.
    Superseded,
}

impl TransactionStatus {
    /// Every status, in the schema's enum order.
    pub const ALL: [Self; 9] = [
        Self::Draft,
        Self::Applied,
        Self::Evaluating,
        Self::Ready,
        Self::Blocked,
        Self::Inconclusive,
        Self::Promoted,
        Self::Rejected,
        Self::Superseded,
    ];

    /// The schema token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Applied => "applied",
            Self::Evaluating => "evaluating",
            Self::Ready => "ready",
            Self::Blocked => "blocked",
            Self::Inconclusive => "inconclusive",
            Self::Promoted => "promoted",
            Self::Rejected => "rejected",
            Self::Superseded => "superseded",
        }
    }
}

// --- gates and cost --------------------------------------------------------------

/// One entry of `gates`. Evidence is always empty here: attaching and evaluating are
/// IMPL-05 and IMPL-03.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gate {
    name: GateName,
    status: GateStatus,
}

impl Gate {
    /// The gate.
    #[must_use]
    pub const fn name(&self) -> GateName {
        self.name
    }

    /// Its status.
    #[must_use]
    pub const fn status(&self) -> GateStatus {
        self.status
    }

    fn json(self) -> Json {
        let mut fields = BTreeMap::new();
        fields.insert("evidence".to_owned(), Json::Array(Vec::new()));
        fields.insert(
            "name".to_owned(),
            Json::String(self.name.token().to_owned()),
        );
        fields.insert(
            "status".to_owned(),
            Json::String(self.status.token().to_owned()),
        );
        Json::Object(fields)
    }
}

/// The gate list a version starts from under `profile`: every gate present, in
/// gate-number order, `pending` inside the profile and `not_yet_enforced` outside it.
///
/// `apply` resets to this list too ("gate statuses reset to `pending` or
/// `not_yet_enforced`", RFC 0032 operations table).
#[must_use]
pub fn initial_gates(profile: GateProfile) -> [Gate; 12] {
    GateName::ALL.map(|name| Gate {
        name,
        status: if profile.enforces(name) {
            GateStatus::Pending
        } else {
            GateStatus::NotYetEnforced
        },
    })
}

/// The cumulative cost ledger's five required dimensions (plan §8.6).
///
/// Only the zero ledger exists here: no operation in this skeleton spends. Accruing
/// spend, and the non-decreasing rule across a lineage, arrive with evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CostLedger {
    cpu_ms: i64,
    wall_ms: i64,
    solver_ms: i64,
    memory_bytes: i64,
    tokens: i64,
}

impl CostLedger {
    /// The ledger of a transaction nothing has been spent on.
    #[must_use]
    pub const fn zero() -> Self {
        Self {
            cpu_ms: 0,
            wall_ms: 0,
            solver_ms: 0,
            memory_bytes: 0,
            tokens: 0,
        }
    }

    fn json(self) -> Json {
        Json::Object(BTreeMap::from([
            ("cpu_ms".to_owned(), Json::Integer(self.cpu_ms)),
            ("memory_bytes".to_owned(), Json::Integer(self.memory_bytes)),
            ("solver_ms".to_owned(), Json::Integer(self.solver_ms)),
            ("tokens".to_owned(), Json::Integer(self.tokens)),
            ("wall_ms".to_owned(), Json::Integer(self.wall_ms)),
        ]))
    }
}

// --- the failure binding seam ----------------------------------------------------

/// A lookup's answer. `Unavailable` is not `Unknown`: a store that could not answer
/// has not said the artifact is absent (INV-008), and `begin` refuses the two
/// differently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution<T> {
    /// The artifact resolved.
    Found(T),
    /// The store answered, and it holds no such artifact.
    Unknown,
    /// The store did not answer.
    Unavailable,
}

/// Where `begin` resolves the base triple from (RFC 0032 correction 2).
///
/// The daemon implements this over its crashpack store and its snapshot–intent
/// bindings (RFC 0037). A crashpack that another deployment or principal published is
/// `Unknown` to this binding, so a foreign failure opens nothing.
pub trait FailureBinding {
    /// The snapshot the crashpack was captured on.
    fn failure_base(&self, failure: &CrashpackId) -> Resolution<SnapshotId>;

    /// The intent the snapshot is bound to.
    fn snapshot_intent(&self, snapshot: &SnapshotId) -> Resolution<IntentId>;
}

/// Why `begin` opened nothing. No variant carries caller prose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BeginRefusal {
    /// The crashpack does not resolve: the failure is unknown or foreign.
    UnknownFailure {
        /// The crashpack named.
        failure: CrashpackId,
    },
    /// The crashpack's base snapshot has no intent binding, so there is no protected
    /// intent to repair against.
    UnboundBaseSnapshot {
        /// The crashpack named.
        failure: CrashpackId,
        /// Its base snapshot.
        base_snapshot: SnapshotId,
    },
    /// The binding store did not answer. Typed inconclusiveness, not a verdict on the
    /// crashpack.
    BindingUnavailable {
        /// The crashpack named.
        failure: CrashpackId,
    },
}

impl fmt::Display for BeginRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownFailure { failure } => write!(f, "failure {failure} does not resolve"),
            Self::UnboundBaseSnapshot {
                failure,
                base_snapshot,
            } => write!(
                f,
                "failure {failure}'s base snapshot {base_snapshot} has no intent binding"
            ),
            Self::BindingUnavailable { failure } => {
                write!(f, "the binding store did not answer for failure {failure}")
            }
        }
    }
}

impl std::error::Error for BeginRefusal {}

/// Why `apply` produced no version. No variant carries the hypothesis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyRefusal {
    /// `changes[index]` is of kind `intent`. Weakening a property, removing a fault,
    /// hiding an event — any edit to the Intent Contract — is a privileged intent
    /// revision through RFC 0037, never a repair (INV-001, INV-011). The IDL code is
    /// `IntentMutationDenied`.
    IntentMutationDenied {
        /// The position of the first `intent` change.
        index: usize,
    },
    /// The lineage has reached the largest representable version.
    VersionExhausted,
}

impl fmt::Display for ApplyRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IntentMutationDenied { index } => write!(
                f,
                "IntentMutationDenied: changes[{index}] edits intent; that is an intent revision, not a repair"
            ),
            Self::VersionExhausted => f.write_str("the transaction lineage has no next version"),
        }
    }
}

impl std::error::Error for ApplyRefusal {}

/// The candidate snapshot `apply` records.
///
/// Sealing it from base + changes, and the digest comparison gate 2 makes, are patch
/// identity (PR-20 / IMPL-02). Until then the caller names the snapshot, unchecked:
/// nothing here verifies that it is sealed, that it differs from the base, or that it
/// holds exactly the proposed changes. Gate 2 is where that is decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedCandidate(SnapshotId);

impl SealedCandidate {
    /// A candidate the caller asserts was sealed elsewhere. The name marks the trust
    /// boundary at the call site; IMPL-02 replaces it with a checked seal.
    #[must_use]
    pub const fn unchecked_from(snapshot: SnapshotId) -> Self {
        Self(snapshot)
    }
}

// --- identity --------------------------------------------------------------------

/// A version's content identity: the canonical bytes of its preimage (every field but
/// `repair_id`). Equality is byte equality and consults no hash (ADR-0013).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RepairIdentity(Vec<u8>);

impl fmt::Debug for RepairIdentity {
    /// The length only: the preimage contains the hypothesis, and RFC 0032 keeps that
    /// prose out of any text a `{:?}` of a containing type or refusal could produce.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RepairIdentity(<{} canonical bytes>)", self.0.len())
    }
}

impl RepairIdentity {
    /// The canonical preimage bytes.
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.0
    }

    /// The `rt_*` handle naming this identity under `H`. The digest indexes; it is not
    /// the identity.
    #[must_use]
    pub fn mint<H: ContentHasher>(&self) -> RepairId {
        let handle = format!("rt_{}", H::hash(&self.0).to_token());
        // A lowercase-hex token after `rt_` always matches `^rt_[A-Za-z0-9_-]+$`.
        RepairId::new(&handle).unwrap_or_else(|_| unreachable!("a hex token is a handle suffix"))
    }
}

// --- the transaction -------------------------------------------------------------

/// One immutable transaction version, named under the hasher `H`.
///
/// Built only by [`Self::begin`] and [`Self::apply`], so every value satisfies the
/// clauses in the module table. `H` is a type parameter of the transaction, not of each
/// call, so one lineage names every version, and every `supersedes` pointer, under one
/// algorithm.
///
/// Equality is identity equality: the canonical preimage bytes, never the handle. The
/// preimage of a version after the first embeds its predecessor's `rt_` handle
/// (`supersedes`, a schema field), so from version 2 on the identity's injectivity also
/// rests on `H`'s collision resistance.
pub struct RepairTransaction<H: ContentHasher> {
    hasher: PhantomData<fn() -> H>,
    repair_id: RepairId,
    identity: RepairIdentity,
    version: i64,
    supersedes: Option<RepairId>,
    base_snapshot: SnapshotId,
    base_intent: IntentId,
    failure: CrashpackId,
    hypothesis: Hypothesis,
    changes: Vec<Change>,
    candidate_snapshot: Option<SnapshotId>,
    gate_profile: GateProfile,
    gates: [Gate; 12],
    cost_ledger: CostLedger,
}

impl<H: ContentHasher> Clone for RepairTransaction<H> {
    fn clone(&self) -> Self {
        Self {
            hasher: PhantomData,
            repair_id: self.repair_id.clone(),
            identity: self.identity.clone(),
            version: self.version,
            supersedes: self.supersedes.clone(),
            base_snapshot: self.base_snapshot.clone(),
            base_intent: self.base_intent.clone(),
            failure: self.failure.clone(),
            hypothesis: self.hypothesis.clone(),
            changes: self.changes.clone(),
            candidate_snapshot: self.candidate_snapshot.clone(),
            gate_profile: self.gate_profile,
            gates: self.gates,
            cost_ledger: self.cost_ledger,
        }
    }
}

impl<H: ContentHasher> PartialEq for RepairTransaction<H> {
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity
    }
}

impl<H: ContentHasher> Eq for RepairTransaction<H> {}

impl<H: ContentHasher> fmt::Debug for RepairTransaction<H> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RepairTransaction")
            .field("repair_id", &self.repair_id)
            .field("identity", &self.identity)
            .field("version", &self.version)
            .field("supersedes", &self.supersedes)
            .field("base_snapshot", &self.base_snapshot)
            .field("base_intent", &self.base_intent)
            .field("failure", &self.failure)
            .field("hypothesis", &self.hypothesis)
            .field("changes", &self.changes)
            .field("candidate_snapshot", &self.candidate_snapshot)
            .field("gate_profile", &self.gate_profile)
            .field("gates", &self.gates)
            .field("cost_ledger", &self.cost_ledger)
            .finish()
    }
}

impl<H: ContentHasher> RepairTransaction<H> {
    /// `repair.begin`: open v1 at `draft` on `failure` under `profile`.
    ///
    /// # Errors
    ///
    /// [`BeginRefusal`] when the failure or its base snapshot's intent binding does not
    /// resolve. A refusal builds nothing.
    pub fn begin(
        failure: CrashpackId,
        profile: GateProfile,
        binding: &impl FailureBinding,
    ) -> Result<Self, BeginRefusal> {
        let base_snapshot = match binding.failure_base(&failure) {
            Resolution::Found(snapshot) => snapshot,
            Resolution::Unknown => return Err(BeginRefusal::UnknownFailure { failure }),
            Resolution::Unavailable => return Err(BeginRefusal::BindingUnavailable { failure }),
        };
        let base_intent = match binding.snapshot_intent(&base_snapshot) {
            Resolution::Found(intent) => intent,
            Resolution::Unknown => {
                return Err(BeginRefusal::UnboundBaseSnapshot {
                    failure,
                    base_snapshot,
                });
            }
            Resolution::Unavailable => return Err(BeginRefusal::BindingUnavailable { failure }),
        };
        Ok(Self::seal(Unsealed {
            version: 1,
            supersedes: None,
            base_snapshot,
            base_intent,
            failure,
            hypothesis: Hypothesis::unstated(),
            changes: Vec::new(),
            candidate_snapshot: None,
            gate_profile: profile,
            gates: initial_gates(profile),
            cost_ledger: CostLedger::zero(),
        }))
    }

    /// `repair.apply`: record `proposal`'s hypothesis and changes against `candidate` as
    /// the next version, at `applied`, with every gate reset.
    ///
    /// The base triple, the profile, and the cost ledger carry over unchanged. The
    /// hypothesis is recorded verbatim and read by nothing.
    ///
    /// # Errors
    ///
    /// [`ApplyRefusal::IntentMutationDenied`] when any change is of kind `intent`: no
    /// transaction in this crate is reclassified, so none admits one.
    /// [`ApplyRefusal::VersionExhausted`] when the version cannot advance.
    pub fn apply(
        &self,
        proposal: &Proposal,
        candidate: SealedCandidate,
    ) -> Result<Self, ApplyRefusal> {
        if let Some(index) = proposal
            .changes()
            .iter()
            .position(|change| change.kind() == ChangeKind::Intent)
        {
            return Err(ApplyRefusal::IntentMutationDenied { index });
        }
        let version = self
            .version
            .checked_add(1)
            .ok_or(ApplyRefusal::VersionExhausted)?;
        Ok(Self::seal(Unsealed {
            version,
            supersedes: Some(self.repair_id.clone()),
            base_snapshot: self.base_snapshot.clone(),
            base_intent: self.base_intent.clone(),
            failure: self.failure.clone(),
            hypothesis: proposal.hypothesis().clone(),
            changes: proposal.changes().to_vec(),
            candidate_snapshot: Some(candidate.0),
            gate_profile: self.gate_profile,
            gates: initial_gates(self.gate_profile),
            cost_ledger: self.cost_ledger,
        }))
    }

    fn seal(parts: Unsealed) -> Self {
        let identity = RepairIdentity(preimage(&parts).to_canonical_bytes());
        let repair_id = identity.mint::<H>();
        let Unsealed {
            version,
            supersedes,
            base_snapshot,
            base_intent,
            failure,
            hypothesis,
            changes,
            candidate_snapshot,
            gate_profile,
            gates,
            cost_ledger,
        } = parts;
        Self {
            hasher: PhantomData,
            repair_id,
            identity,
            version,
            supersedes,
            base_snapshot,
            base_intent,
            failure,
            hypothesis,
            changes,
            candidate_snapshot,
            gate_profile,
            gates,
            cost_ledger,
        }
    }

    /// `repair_id`: this version's handle.
    #[must_use]
    pub const fn repair_id(&self) -> &RepairId {
        &self.repair_id
    }

    /// This version's content identity.
    #[must_use]
    pub const fn identity(&self) -> &RepairIdentity {
        &self.identity
    }

    /// `version`.
    #[must_use]
    pub const fn version(&self) -> i64 {
        self.version
    }

    /// `supersedes`: the predecessor, absent exactly at version 1.
    #[must_use]
    pub const fn supersedes(&self) -> Option<&RepairId> {
        self.supersedes.as_ref()
    }

    /// `failure`.
    #[must_use]
    pub const fn failure(&self) -> &CrashpackId {
        &self.failure
    }

    /// `base_snapshot`.
    #[must_use]
    pub const fn base_snapshot(&self) -> &SnapshotId {
        &self.base_snapshot
    }

    /// `base_intent`.
    #[must_use]
    pub const fn base_intent(&self) -> &IntentId {
        &self.base_intent
    }

    /// `hypothesis`.
    #[must_use]
    pub const fn hypothesis(&self) -> &Hypothesis {
        &self.hypothesis
    }

    /// `changes`.
    #[must_use]
    pub fn changes(&self) -> &[Change] {
        &self.changes
    }

    /// `candidate_snapshot`.
    #[must_use]
    pub const fn candidate_snapshot(&self) -> Option<&SnapshotId> {
        self.candidate_snapshot.as_ref()
    }

    /// `gate_profile`.
    #[must_use]
    pub const fn gate_profile(&self) -> GateProfile {
        self.gate_profile
    }

    /// `gates`, in gate-number order.
    #[must_use]
    pub const fn gates(&self) -> &[Gate; 12] {
        &self.gates
    }

    /// `status`, derived (RFC 0032 rules 3 and 5, the two this skeleton reaches).
    #[must_use]
    pub const fn status(&self) -> TransactionStatus {
        if self.candidate_snapshot.is_none() {
            TransactionStatus::Draft
        } else {
            TransactionStatus::Applied
        }
    }

    /// The identity preimage: the artifact minus `repair_id`.
    #[must_use]
    pub fn preimage_json(&self) -> Json {
        preimage(&Unsealed {
            version: self.version,
            supersedes: self.supersedes.clone(),
            base_snapshot: self.base_snapshot.clone(),
            base_intent: self.base_intent.clone(),
            failure: self.failure.clone(),
            hypothesis: self.hypothesis.clone(),
            changes: self.changes.clone(),
            candidate_snapshot: self.candidate_snapshot.clone(),
            gate_profile: self.gate_profile,
            gates: self.gates,
            cost_ledger: self.cost_ledger,
        })
    }

    /// The schema-conformant artifact.
    #[must_use]
    pub fn artifact_json(&self) -> Json {
        let mut json = self.preimage_json();
        if let Json::Object(fields) = &mut json {
            fields.insert(
                "repair_id".to_owned(),
                Json::String(self.repair_id.as_str().to_owned()),
            );
        }
        json
    }

    /// The artifact's canonical bytes.
    #[must_use]
    pub fn to_artifact_bytes(&self) -> Vec<u8> {
        self.artifact_json().to_canonical_bytes()
    }
}

/// The fields of a version before its identity is computed.
struct Unsealed {
    version: i64,
    supersedes: Option<RepairId>,
    base_snapshot: SnapshotId,
    base_intent: IntentId,
    failure: CrashpackId,
    hypothesis: Hypothesis,
    changes: Vec<Change>,
    candidate_snapshot: Option<SnapshotId>,
    gate_profile: GateProfile,
    gates: [Gate; 12],
    cost_ledger: CostLedger,
}

fn string(value: &str) -> Json {
    Json::String(value.to_owned())
}

fn preimage(parts: &Unsealed) -> Json {
    let status = if parts.candidate_snapshot.is_none() {
        TransactionStatus::Draft
    } else {
        TransactionStatus::Applied
    };
    let mut fields = BTreeMap::new();
    fields.insert("schema_id".to_owned(), string(SCHEMA_ID));
    fields.insert("schema_epoch".to_owned(), Json::Integer(SCHEMA_EPOCH));
    fields.insert("version".to_owned(), Json::Integer(parts.version));
    if let Some(predecessor) = &parts.supersedes {
        fields.insert("supersedes".to_owned(), string(predecessor.as_str()));
    }
    fields.insert(
        "base_snapshot".to_owned(),
        string(parts.base_snapshot.as_str()),
    );
    fields.insert("base_intent".to_owned(), string(parts.base_intent.as_str()));
    fields.insert("failure".to_owned(), string(parts.failure.as_str()));
    fields.insert("hypothesis".to_owned(), string(parts.hypothesis.as_prose()));
    fields.insert(
        "changes".to_owned(),
        Json::Array(
            parts
                .changes
                .iter()
                .map(|change| {
                    Json::Object(BTreeMap::from([
                        ("digest".to_owned(), string(change.digest())),
                        ("kind".to_owned(), string(change.kind().token())),
                    ]))
                })
                .collect(),
        ),
    );
    fields.insert(
        "candidate_snapshot".to_owned(),
        parts
            .candidate_snapshot
            .as_ref()
            .map_or(Json::Null, |snapshot| string(snapshot.as_str())),
    );
    fields.insert("status".to_owned(), string(status.token()));
    fields.insert(
        "gate_profile".to_owned(),
        string(parts.gate_profile.token()),
    );
    fields.insert(
        "gates".to_owned(),
        Json::Array(parts.gates.iter().map(|gate| gate.json()).collect()),
    );
    fields.insert("cost_ledger".to_owned(), parts.cost_ledger.json());
    Json::Object(fields)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_profiles_leave_exactly_the_rfc_gates_unenforced() {
        let unenforced = |profile: GateProfile| -> Vec<GateName> {
            GateName::ALL
                .into_iter()
                .filter(|gate| !profile.enforces(*gate))
                .collect()
        };
        assert_eq!(
            unenforced(GateProfile::PhaseB),
            [GateName::CertificateRebuild, GateName::IncrementalParity]
        );
        assert_eq!(
            unenforced(GateProfile::PhaseC),
            [GateName::CertificateRebuild]
        );
        assert!(unenforced(GateProfile::PhaseD).is_empty());
        assert!(unenforced(GateProfile::Default).is_empty());
    }
}
