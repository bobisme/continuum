//! The promotion-receipt skeleton (PR 22): the field groups PR-22 / IMPL-01, IMPL-02,
//! IMPL-05, IMPL-06 and IMPL-07/08 own, composed from referenced artifacts and verified
//! against them.
//!
//! The normative shape is `notes/plan/schemas/promotion-receipt.schema.json`
//! (`https://continuum.dev/schema/promotion-receipt.json`, epoch 1). The obligations the
//! schema cannot state are RFC 0032, "The promotion receipt" and "Gate profiles".
//!
//! # Clause → mechanism
//!
//! | Clause | Source | Mechanism |
//! |---|---|---|
//! | `intent` is the base intent | RFC 0032 receipt composition table | [`ReceiptSkeleton::compose`] reads it from the transaction's frozen base triple, never from a caller |
//! | the intent is the protected contract's content identity | ADR-0013; RFC 0037 | the [`IntentRegistry`] must resolve the handle to a contract that declares it and stands `accepted`; the skeleton keeps that contract's [`IntentIdentity`] bytes |
//! | `base_snapshot`, `result_snapshot` are the before and after | RFC 0032 receipt composition table; promotion step 7 | before = the transaction's `base_snapshot`; after = its sealed `candidate_snapshot`; each must resolve in [`SealedSnapshots`] and be bound to the base intent |
//! | the profile is the transaction's, frozen at `begin` | RFC 0032 correction 4 | `gate_profile` is copied from the transaction; no parameter can name another |
//! | a gate outside the profile is listed `not_yet_enforced`, never omitted, never `passed` | RFC 0032 "Gate profiles"; plan §21; INV-007 | [`NotYetEnforced`] is built from a profile only, and an [`UnenforcedGate`] renders one status token and has no other |
//! | a gate inside the profile is never `not_yet_enforced` | RFC 0032 "Gate profiles"; the receipt schema has no profile conditional (only `repair-transaction.schema.json` does) | [`verify_skeleton`] refuses [`VerifyRefusal::EnforcedGateListedUnenforced`] |
//! | a client-supplied receipt is checked by reference and accepts no declared status | RFC 0032 "Gate 12 is verified, not asserted"; PR 22 exit | [`verify_skeleton`] resolves the named transaction, re-composes the skeleton from the stores, and compares; no claimed value is copied into the result |
//! | handle possession confers nothing | ADR-0037 | `receipt_id` is not read; a well-formed claim is only a claim |
//! | a receipt's gate status is the transaction's recorded status | RFC 0032 "Status machine"; "Gate 12 is verified, not asserted" | every gate but gate 12 must match the record, else [`VerifyRefusal::GateNotOnRecord`]; no transaction records an in-profile gate `passed` today, so no claim verifies today |
//! | refinement and certificate status come from the evidence the daemon holds for the candidate, never from the claim | RFC 0032 receipt table; RFC 0005 correction 1; INV-008 | [`status`] (PR-22 / IMPL-05): [`RefinementCertificateStatus`] is derived from [`CandidateEvidence`]; a certificate counts only when model-bound to the candidate; a gate 8 or 9 listed `passed` without derived evidence is [`VerifyRefusal::GateWithoutEvidence`], and a claimed `coverage.refinement_coverage` or `coverage.certificate_rebuild` group is [`VerifyRefusal::CoverageNotDerived`] |
//! | `unknowns` lists every unresolved unknown; an absent array is not an empty one | RFC 0032 receipt table, correction 11; INV-007 | [`unknowns`] (PR-22 / IMPL-06): [`Unknowns`] is derived from the record, the status and the daemon's pack cases; a claim must list exactly the derived entries in canonical order |
//! | gate 12 is `pending` on a `ready` transaction; promotion verifies the receipt, then moves gate 12 | RFC 0032 "Gate 12 is the promotion step"; "Promotion" steps 4–7 | [`verify_for_promotion`] requires gate 12 `pending` on the record and returns a [`ReceiptGenerationLicense`], the typed seam PR-20 promote consumes to record the transition; a record already showing gate 12 `passed` is [`VerifyRefusal::ReceiptGenerationAlreadyPassed`]. [`verify_skeleton`] checks a published receipt, whose record shows gate 12 `passed` |
//!
//! # What is not here (typed seams)
//!
//! [`ReceiptSeam`] lists every receipt field this skeleton neither composes nor verifies,
//! with the PR-22 requirement that owns it. [`SkeletonVerification::receipt_verdict`] is
//! therefore always [`ReceiptVerdict::Incomplete`] today: a skeleton that matches is not
//! a verified receipt (INV-008).
//!
//! - This module renders no `passed` entry at all: the whole `gates` array needs the
//!   in-profile outcomes, which arrive with PR-20 `evaluate` and the PR-22 IMPL-03 and
//!   IMPL-04 evidence ([`ReceiptSeam::EnforcedGateOutcomes`]). The evidence behind gates
//!   8 and 9 is IMPL-05's, and it is absent today, so no receipt that lists gate 8
//!   `passed` verifies: every profile enforces gate 8, so no receipt verifies until a
//!   refinement checker ships (PR 17).
//! - The skeleton does not check that the transaction is the lineage head at `ready`;
//!   that is `repair.promote` step 1 (RFC 0032 "Promotion"), which is not served yet.
//! - Daemon wiring: [`IntentRegistry`], [`SealedSnapshots`], [`TransactionStore`] and
//!   [`CandidateEvidence`] are seams the daemon implements over its intent registry, its
//!   sealed workspaces, its transaction store and its evidence graph. No daemon path calls this module yet. Each store MUST answer
//!   `Unknown` for a handle the caller has no standing to read: the refusals below
//!   distinguish "does not exist" from "exists but disagrees", so an unscoped store
//!   would make verification an existence oracle (ADR-0037).
//! - The intent handle is a name the registry minted, not a digest the transaction
//!   pins. The skeleton reports the [`IntentIdentity`] of the contract the registry holds
//!   under that handle; that the registry never refiles a handle under other content is
//!   a registry invariant (RFC 0037 ID3), not something this module can check.
//! - The transaction a receipt names must be the promoted version (RFC 0032 receipt
//!   table). No transaction reaches `promoted` yet, so that check is
//!   [`ReceiptSeam::PromotionRecord`].
//!
//! # Open question raised, not resolved here
//!
//! The receipt schema types refinement coverage `before`, `after` and `delta` as
//! `number`, and the shipped example carries fractions. The one canonical JSON reader
//! (RFC 0037 ID5) admits integers only, so such a receipt is refused
//! [`ClaimRefusal::OutsideId5Vocabulary`] — typed, never `NotJson`. Either the schema
//! narrows those fields or RFC 0032's "canonically encoded" receipt names another
//! encoding; this module does neither.

use core::fmt;
use std::collections::BTreeMap;

pub mod status;
pub mod unknowns;

use continuum_intent::canonical_json::{Json, JsonError};
use continuum_intent::contract::{IntentContract, IntentId, IntentIdentity};
use continuum_value::identity::ContentHasher;

use crate::handle::{RepairId, SnapshotId};
use crate::transaction::{GateName, GateProfile, GateStatus, RepairTransaction, Resolution};

use status::EvidenceDefect;
pub use status::{
    CertificateNode, CertificateRecord, CertificateVerdict, CoverageGroup, EvidenceKind,
    GroupDerivation, RefinementCertificateStatus,
};
pub use unknowns::{PackCase, Unknown, UnknownKind, Unknowns};
use unknowns::{UnknownsDefect, UnknownsMismatch};

/// The receipt schema's artifact-class identity (`schema_id`).
pub const RECEIPT_SCHEMA_ID: &str = "https://continuum.dev/schema/promotion-receipt.json";

/// The largest claimed receipt [`ClaimedReceipt::parse`] reads. It is the daemon's frame
/// bound divided by sixteen: the check runs before the parse, so a claim cannot make the
/// parser allocate in proportion to an unbounded input.
pub const MAX_CLAIMED_RECEIPT_BYTES: usize = 1 << 20;

/// Every top-level property the receipt schema declares, sorted by code point
/// (`additionalProperties: false`).
pub const RECEIPT_PROPERTIES: [&str; 20] = [
    "base_snapshot",
    "checker",
    "cost_ledger",
    "coverage",
    "epochs",
    "gate_profile",
    "gates",
    "intent",
    "intent_diff",
    "policy_decision",
    "receipt_id",
    "redacted_references",
    "repair_transaction",
    "result_snapshot",
    "schema_epoch",
    "schema_id",
    "semantic_diff",
    "semantic_epoch",
    "signature",
    "unknowns",
];

// --- owned fields ------------------------------------------------------------------

/// A receipt field this skeleton composes and verifies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReceiptField {
    /// `repair_transaction`: the anchor every other field is derived from.
    RepairTransaction,
    /// `intent` (PR-22 / IMPL-01).
    Intent,
    /// `base_snapshot` (PR-22 / IMPL-02).
    BaseSnapshot,
    /// `result_snapshot` (PR-22 / IMPL-02).
    ResultSnapshot,
    /// `gate_profile` (PR-22 / IMPL-07).
    GateProfile,
    /// `gates`: the `not_yet_enforced` entries (PR-22 / IMPL-08) and the structure of the
    /// list. The `passed` entries' evidence is [`ReceiptSeam::EnforcedGateOutcomes`],
    /// except gates 8 and 9, whose evidence is IMPL-05's.
    Gates,
    /// `unknowns` (PR-22 / IMPL-06). The `coverage` groups IMPL-05 owns,
    /// `refinement_coverage` and `certificate_rebuild`, are checked too, but `coverage`
    /// is not listed here: its other groups are [`ReceiptSeam::ReplayNeighborhoodMutation`].
    Unknowns,
}

impl ReceiptField {
    /// Every owned field.
    pub const ALL: [Self; 7] = [
        Self::RepairTransaction,
        Self::Intent,
        Self::BaseSnapshot,
        Self::ResultSnapshot,
        Self::GateProfile,
        Self::Gates,
        Self::Unknowns,
    ];

    /// The schema property.
    #[must_use]
    pub const fn property(self) -> &'static str {
        match self {
            Self::RepairTransaction => "repair_transaction",
            Self::Intent => "intent",
            Self::BaseSnapshot => "base_snapshot",
            Self::ResultSnapshot => "result_snapshot",
            Self::GateProfile => "gate_profile",
            Self::Gates => "gates",
            Self::Unknowns => "unknowns",
        }
    }
}

/// A receipt field group this skeleton does not compose or verify, with its owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReceiptSeam {
    /// The evidence behind every in-profile gate listed `passed` other than gates 8 and 9
    /// (PR-20 `evaluate`; the gate evidence of PR-22 IMPL-03 and IMPL-04).
    EnforcedGateOutcomes,
    /// `semantic_diff`, `intent_diff` (PR-22 / IMPL-03).
    SemanticDiff,
    /// `coverage`: replay, neighborhood, mutation and parity results (PR-22 / IMPL-04).
    ReplayNeighborhoodMutation,
    /// `policy_decision` (PR-22 / IMPL-09).
    PolicyDecision,
    /// `cost_ledger`, `checker`, `semantic_epoch`, `epochs`, `redacted_references`
    /// (PR 22 exit, at `repair.promote`).
    LedgerCheckerEpochs,
    /// `schema_id`, `schema_epoch`, `receipt_id`, `signature` (PR 22 exit: the receipt
    /// service's identity and signature, plan §18.6).
    EnvelopeAndSignature,
    /// That the named transaction version is `promoted` and its `receipt` is this
    /// receipt (RFC 0032: `repair_transaction` is "the promoted version"; PR 22 exit, at
    /// `repair.promote`).
    PromotionRecord,
}

impl ReceiptSeam {
    /// Every seam.
    pub const ALL: [Self; 7] = [
        Self::EnforcedGateOutcomes,
        Self::SemanticDiff,
        Self::ReplayNeighborhoodMutation,
        Self::PolicyDecision,
        Self::LedgerCheckerEpochs,
        Self::EnvelopeAndSignature,
        Self::PromotionRecord,
    ];

    /// The requirement that owns this seam.
    #[must_use]
    pub const fn owner(self) -> &'static str {
        match self {
            Self::EnforcedGateOutcomes => "PR-20 evaluate; PR-22-IMPL-03..04",
            Self::SemanticDiff => "PR-22-IMPL-03",
            Self::ReplayNeighborhoodMutation => "PR-22-IMPL-04",
            Self::PolicyDecision => "PR-22-IMPL-09",
            Self::LedgerCheckerEpochs | Self::EnvelopeAndSignature | Self::PromotionRecord => {
                "PR-22-EXIT"
            }
        }
    }

    /// The schema properties the seam covers. The `passed` entries of `gates` belong to
    /// [`Self::EnforcedGateOutcomes`], and `coverage`'s groups other than the two IMPL-05
    /// owns to [`Self::ReplayNeighborhoodMutation`].
    #[must_use]
    pub const fn properties(self) -> &'static [&'static str] {
        match self {
            Self::EnforcedGateOutcomes => &["gates"],
            Self::SemanticDiff => &["intent_diff", "semantic_diff"],
            Self::ReplayNeighborhoodMutation => &["coverage"],
            Self::PolicyDecision => &["policy_decision"],
            Self::LedgerCheckerEpochs => &[
                "checker",
                "cost_ledger",
                "epochs",
                "redacted_references",
                "semantic_epoch",
            ],
            Self::EnvelopeAndSignature => &["receipt_id", "schema_epoch", "schema_id", "signature"],
            Self::PromotionRecord => &["repair_transaction"],
        }
    }
}

// --- gate_profile and NotYetEnforced -----------------------------------------------

/// One gate outside the active profile. It renders as `not_yet_enforced` and has no
/// other status: the type is the refusal to render it `passed`.
///
/// Only [`NotYetEnforced::of`] builds one, and it carries the profile it is outside of,
/// so an in-profile gate never becomes one and an entry cannot be moved to another
/// profile's list unnoticed:
///
/// ```compile_fail
/// use continuum_repair::receipt::UnenforcedGate;
/// use continuum_repair::transaction::{GateName, GateProfile};
/// let forged = UnenforcedGate { gate: GateName::BaseReplay, profile: GateProfile::PhaseB };
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnenforcedGate {
    gate: GateName,
    profile: GateProfile,
}

impl UnenforcedGate {
    /// The schema's status token for this entry, and the only one it has.
    pub const STATUS: &'static str = "not_yet_enforced";

    /// The gate.
    #[must_use]
    pub const fn gate(self) -> GateName {
        self.gate
    }

    /// The profile the gate is outside of.
    #[must_use]
    pub const fn profile(self) -> GateProfile {
        self.profile
    }

    /// The `gates[]` entry: `{"gate": …, "status": "not_yet_enforced"}`.
    #[must_use]
    pub fn entry_json(self) -> Json {
        Json::Object(BTreeMap::from([
            (
                "gate".to_owned(),
                Json::String(self.gate.token().to_owned()),
            ),
            ("status".to_owned(), Json::String(Self::STATUS.to_owned())),
        ]))
    }
}

/// The `NotYetEnforced` list (plan §21; RFC 0032 correction 5 spells the token
/// `not_yet_enforced`): every gate outside `profile`, in gate-number order.
///
/// It is derived from the profile and from nothing else, which reuses
/// [`GateProfile::enforces`] rather than restating the profile table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotYetEnforced {
    profile: GateProfile,
    gates: Vec<UnenforcedGate>,
}

impl NotYetEnforced {
    /// The list for `profile`.
    #[must_use]
    pub fn of(profile: GateProfile) -> Self {
        Self {
            profile,
            gates: GateName::ALL
                .into_iter()
                .filter(|gate| !profile.enforces(*gate))
                .map(|gate| UnenforcedGate { gate, profile })
                .collect(),
        }
    }

    /// The profile the list was derived from.
    #[must_use]
    pub const fn profile(&self) -> GateProfile {
        self.profile
    }

    /// The unenforced gates, in gate-number order.
    #[must_use]
    pub fn gates(&self) -> &[UnenforcedGate] {
        &self.gates
    }

    /// Whether `gate` is outside the profile.
    #[must_use]
    pub const fn contains(&self, gate: GateName) -> bool {
        !self.profile.enforces(gate)
    }

    /// The `gates[]` entries this list contributes, as a JSON array in gate-number order.
    #[must_use]
    pub fn entries_json(&self) -> Json {
        Json::Array(self.gates.iter().map(|gate| gate.entry_json()).collect())
    }
}

// --- the stores (daemon seams) -----------------------------------------------------

/// The standing of a registered Intent Contract (the intent registry record's `status`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IntentStanding {
    /// A proposal: it never had INV-001 protection.
    Proposed,
    /// Accepted, and the lineage head: protected now.
    Accepted,
    /// Accepted once, and since superseded by a successor contract.
    Superseded,
}

/// What the intent registry holds under a handle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredIntent {
    contract: IntentContract,
    standing: IntentStanding,
}

impl RegisteredIntent {
    /// A registry record: the contract and its standing.
    #[must_use]
    pub const fn new(contract: IntentContract, standing: IntentStanding) -> Self {
        Self { contract, standing }
    }
}

/// The daemon's intent registry (RFC 0037). A contract another deployment registered is
/// `Unknown` here.
pub trait IntentRegistry {
    /// The record under `intent`.
    fn registered(&self, intent: &IntentId) -> Resolution<RegisteredIntent>;
}

/// The daemon's sealed snapshots and their intent bindings.
///
/// `Found` only for a snapshot the daemon sealed; the value is the Intent Contract the
/// snapshot is bound to (plan §4.2). An unsealed or foreign snapshot is `Unknown`.
pub trait SealedSnapshots {
    /// The intent `snapshot` is bound to.
    fn sealed_binding(&self, snapshot: &SnapshotId) -> Resolution<IntentId>;
}

/// The daemon's transaction store, keyed by `rt_` version handle.
pub trait TransactionStore<H: ContentHasher> {
    /// The transaction version named `repair`.
    fn transaction(&self, repair: &RepairId) -> Resolution<RepairTransaction<H>>;
}

/// The evidence the daemon holds for a transaction's candidate snapshot (PR-22 IMPL-05
/// and IMPL-06). The daemon implements it over its evidence graph.
///
/// Obligations on the implementation, which this crate cannot check:
///
/// - The answer is the evidence as of the named transaction version's promotion. At
///   promotion that is the current graph. For a published receipt it is the view the
///   promotion recorded, so a later `evidence.verify` does not change a published
///   receipt's derivation (the graph is append-only; the view is
///   [`ReceiptSeam::PromotionRecord`]'s concern).
/// - [`CertificateVerdict::ModelBound`] is answered only for a certificate node that
///   `evidence.verify` settled `validated` through the model-binding path (bn-3hk4v:
///   wire epoch 2, finite closure, carried model byte-equal to the model the daemon holds
///   for the one sealed snapshot the node names), with `snapshot` that sealed snapshot.
///   A status a client attached or declared is never one: a node no daemon verification
///   settled is [`CertificateVerdict::Unchecked`].
/// - Pack cases are the declared unsupported cases of the effect packs the candidate's
///   verification used.
/// - `Unknown` for a transaction the caller has no standing to read (ADR-0037).
pub trait CandidateEvidence {
    /// Every certificate node the daemon holds for `candidate`, with its verdict.
    fn certificates(
        &self,
        repair: &RepairId,
        candidate: &SnapshotId,
    ) -> Resolution<Vec<CertificateRecord>>;

    /// The declared unsupported cases of the effect packs `candidate` was verified under.
    fn unsupported_pack_cases(
        &self,
        repair: &RepairId,
        candidate: &SnapshotId,
    ) -> Resolution<Vec<PackCase>>;
}

/// Which store did not answer. A store that did not answer is not a store that said no
/// (INV-008).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Store {
    /// [`IntentRegistry`].
    IntentRegistry,
    /// [`SealedSnapshots`].
    Snapshots,
    /// [`TransactionStore`].
    Transactions,
    /// [`CandidateEvidence`].
    Evidence,
}

/// Which of the two receipt snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SnapshotRole {
    /// `base_snapshot`: the transaction's base.
    Before,
    /// `result_snapshot`: the transaction's sealed candidate.
    After,
}

// --- composition -------------------------------------------------------------------

/// Why no skeleton was composed. No variant carries caller text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComposeRefusal {
    /// The transaction is at `draft`: there is no sealed candidate to be the after.
    NoCandidate,
    /// The base intent does not resolve in the registry.
    IntentUnregistered,
    /// The base intent is a proposal and never had protection.
    IntentNotProtected,
    /// The base intent was superseded before promotion. The repair targets a contract
    /// that is no longer the protected head; it must be re-based.
    IntentSuperseded,
    /// The registry returned a contract that declares a different `intent_id` from the
    /// handle it was filed under. A store defect, refused rather than trusted.
    RegistryInconsistent,
    /// A snapshot does not resolve among the sealed snapshots.
    SnapshotUnresolved(SnapshotRole),
    /// A snapshot is bound to an intent other than the transaction's base intent.
    SnapshotBoundElsewhere(SnapshotRole),
    /// A store did not answer.
    StoreUnavailable(Store),
    /// The evidence store holds no record for the candidate the caller may read.
    EvidenceUnknown,
    /// An evidence answer holds more entries than its bound; nothing was sorted.
    EvidenceOverBound(EvidenceKind),
    /// An evidence answer lists one entry twice. A store defect, refused rather than
    /// trusted.
    EvidenceDuplicate(EvidenceKind),
    /// The transaction records a gate status its profile does not admit: an in-profile
    /// gate `not_yet_enforced`, or a gate outside the profile with any other status.
    GateRecordInconsistent(GateName),
}

impl From<EvidenceDefect> for ComposeRefusal {
    fn from(defect: EvidenceDefect) -> Self {
        match defect {
            EvidenceDefect::OverBound(kind) => Self::EvidenceOverBound(kind),
            EvidenceDefect::Duplicate(kind) => Self::EvidenceDuplicate(kind),
        }
    }
}

impl From<UnknownsDefect> for ComposeRefusal {
    fn from(defect: UnknownsDefect) -> Self {
        match defect {
            UnknownsDefect::GateRecord(gate) => Self::GateRecordInconsistent(gate),
            UnknownsDefect::Evidence(defect) => defect.into(),
        }
    }
}

impl fmt::Display for ComposeRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoCandidate => f.write_str("the transaction has no sealed candidate snapshot"),
            Self::IntentUnregistered => f.write_str("the base intent is not registered"),
            Self::IntentNotProtected => f.write_str("the base intent was never accepted"),
            Self::IntentSuperseded => f.write_str("the base intent has been superseded"),
            Self::RegistryInconsistent => {
                f.write_str("the registry's contract does not declare its own handle")
            }
            Self::SnapshotUnresolved(role) => write!(f, "the {role:?} snapshot is not sealed here"),
            Self::SnapshotBoundElsewhere(role) => {
                write!(f, "the {role:?} snapshot is bound to another intent")
            }
            Self::StoreUnavailable(store) => write!(f, "the {store:?} store did not answer"),
            Self::EvidenceUnknown => f.write_str("no evidence record resolves for the candidate"),
            Self::EvidenceOverBound(kind) => {
                write!(f, "the {kind:?} evidence answer exceeds its bound")
            }
            Self::EvidenceDuplicate(kind) => {
                write!(f, "the {kind:?} evidence answer lists an entry twice")
            }
            Self::GateRecordInconsistent(gate) => write!(
                f,
                "`{}`'s recorded status contradicts the transaction's profile",
                gate.token()
            ),
        }
    }
}

impl std::error::Error for ComposeRefusal {}

/// When a superseded base intent is admissible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Standing {
    /// At composition (promotion time): only the protected head.
    Current,
    /// At verification of a published receipt: it stays verifiable after a later
    /// intent revision (RFC 0032: published receipts remain verifiable indefinitely).
    Historical,
}

/// The intent identity field (PR-22 / IMPL-01): the handle and the ADR-0013 content
/// identity of the contract it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiptIntent {
    handle: IntentId,
    identity: IntentIdentity,
}

impl ReceiptIntent {
    /// `intent`.
    #[must_use]
    pub const fn handle(&self) -> &IntentId {
        &self.handle
    }

    /// The contract's content identity: its canonical preimage bytes (ADR-0013).
    #[must_use]
    pub const fn identity(&self) -> &IntentIdentity {
        &self.identity
    }
}

/// The before/after snapshots (PR-22 / IMPL-02).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiptSnapshots {
    before: SnapshotId,
    after: SnapshotId,
}

impl ReceiptSnapshots {
    /// `base_snapshot`.
    #[must_use]
    pub const fn before(&self) -> &SnapshotId {
        &self.before
    }

    /// `result_snapshot`.
    #[must_use]
    pub const fn after(&self) -> &SnapshotId {
        &self.after
    }
}

/// The receipt fields PR-22 IMPL-01, IMPL-02, IMPL-05, IMPL-06, IMPL-07 and IMPL-08 own,
/// derived from one transaction version and the stores. Built only by [`Self::compose`]
/// and inside [`verify_skeleton`]; no constructor takes a field value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiptSkeleton {
    repair_transaction: RepairId,
    intent: ReceiptIntent,
    snapshots: ReceiptSnapshots,
    gate_profile: GateProfile,
    not_yet_enforced: NotYetEnforced,
    status: RefinementCertificateStatus,
    unknowns: Unknowns,
}

impl ReceiptSkeleton {
    /// Compose the skeleton for `transaction` at promotion time.
    ///
    /// Every value is read from the transaction or resolved in a store. The intent must
    /// stand `accepted`.
    ///
    /// # Errors
    ///
    /// [`ComposeRefusal`], typed; a refusal composes nothing.
    pub fn compose<H: ContentHasher>(
        transaction: &RepairTransaction<H>,
        registry: &impl IntentRegistry,
        snapshots: &impl SealedSnapshots,
        evidence: &impl CandidateEvidence,
    ) -> Result<Self, ComposeRefusal> {
        derive(
            transaction,
            registry,
            snapshots,
            evidence,
            Standing::Current,
        )
    }

    /// `repair_transaction`.
    #[must_use]
    pub const fn repair_transaction(&self) -> &RepairId {
        &self.repair_transaction
    }

    /// `intent`, with its content identity.
    #[must_use]
    pub const fn intent(&self) -> &ReceiptIntent {
        &self.intent
    }

    /// `base_snapshot` and `result_snapshot`.
    #[must_use]
    pub const fn snapshots(&self) -> &ReceiptSnapshots {
        &self.snapshots
    }

    /// `gate_profile`.
    #[must_use]
    pub const fn gate_profile(&self) -> GateProfile {
        self.gate_profile
    }

    /// The `not_yet_enforced` entries of `gates`.
    #[must_use]
    pub const fn not_yet_enforced(&self) -> &NotYetEnforced {
        &self.not_yet_enforced
    }

    /// The refinement and certificate status (PR-22 / IMPL-05).
    #[must_use]
    pub const fn status(&self) -> &RefinementCertificateStatus {
        &self.status
    }

    /// `unknowns` (PR-22 / IMPL-06).
    #[must_use]
    pub const fn unknowns(&self) -> &Unknowns {
        &self.unknowns
    }

    fn anchor(&self) -> (String, Json) {
        (
            ReceiptField::RepairTransaction.property().to_owned(),
            Json::String(self.repair_transaction.as_str().to_owned()),
        )
    }

    /// IMPL-01's fragment: `intent` and the `repair_transaction` it was derived from.
    #[must_use]
    pub fn intent_fields(&self) -> Json {
        Json::Object(BTreeMap::from([
            self.anchor(),
            (
                ReceiptField::Intent.property().to_owned(),
                Json::String(self.intent.handle.as_str().to_owned()),
            ),
        ]))
    }

    /// IMPL-02's fragment: `base_snapshot`, `result_snapshot`, and `repair_transaction`.
    #[must_use]
    pub fn snapshot_fields(&self) -> Json {
        Json::Object(BTreeMap::from([
            self.anchor(),
            (
                ReceiptField::BaseSnapshot.property().to_owned(),
                Json::String(self.snapshots.before.as_str().to_owned()),
            ),
            (
                ReceiptField::ResultSnapshot.property().to_owned(),
                Json::String(self.snapshots.after.as_str().to_owned()),
            ),
        ]))
    }

    /// IMPL-07's fragment: `gate_profile` and `repair_transaction`. The `gates` property
    /// is not rendered: the schema requires all twelve entries, and the in-profile ones
    /// are [`ReceiptSeam::EnforcedGateOutcomes`]. The unenforced entries are
    /// [`NotYetEnforced::entries_json`].
    #[must_use]
    pub fn profile_fields(&self) -> Json {
        Json::Object(BTreeMap::from([
            self.anchor(),
            (
                ReceiptField::GateProfile.property().to_owned(),
                Json::String(self.gate_profile.token().to_owned()),
            ),
        ]))
    }

    /// IMPL-05's fragment: the `unknowns` entries the refinement and certificate status
    /// contributes, and `repair_transaction`. The status renders nothing in `coverage`
    /// ([`status`]).
    #[must_use]
    pub fn status_fields(&self) -> Json {
        Json::Object(BTreeMap::from([
            self.anchor(),
            (
                ReceiptField::Unknowns.property().to_owned(),
                self.unknowns.status_entries_json(),
            ),
        ]))
    }

    /// IMPL-06's fragment: `unknowns` and `repair_transaction`.
    #[must_use]
    pub fn unknown_fields(&self) -> Json {
        Json::Object(BTreeMap::from([
            self.anchor(),
            (
                ReceiptField::Unknowns.property().to_owned(),
                self.unknowns.entries_json(),
            ),
        ]))
    }

    /// Every field this skeleton renders, as one receipt fragment.
    #[must_use]
    pub fn fields_json(&self) -> Json {
        let mut fields = BTreeMap::new();
        for fragment in [
            self.intent_fields(),
            self.snapshot_fields(),
            self.profile_fields(),
            self.unknown_fields(),
        ] {
            if let Json::Object(part) = fragment {
                fields.extend(part);
            }
        }
        Json::Object(fields)
    }
}

fn resolve<T>(
    answer: Resolution<T>,
    unknown: ComposeRefusal,
    store: Store,
) -> Result<T, ComposeRefusal> {
    match answer {
        Resolution::Found(value) => Ok(value),
        Resolution::Unknown => Err(unknown),
        Resolution::Unavailable => Err(ComposeRefusal::StoreUnavailable(store)),
    }
}

fn bound_snapshot(
    snapshots: &impl SealedSnapshots,
    snapshot: &SnapshotId,
    role: SnapshotRole,
    intent: &IntentId,
) -> Result<(), ComposeRefusal> {
    let binding = resolve(
        snapshots.sealed_binding(snapshot),
        ComposeRefusal::SnapshotUnresolved(role),
        Store::Snapshots,
    )?;
    if &binding == intent {
        Ok(())
    } else {
        Err(ComposeRefusal::SnapshotBoundElsewhere(role))
    }
}

fn resolve_evidence<T>(answer: Resolution<T>) -> Result<T, ComposeRefusal> {
    resolve(answer, ComposeRefusal::EvidenceUnknown, Store::Evidence)
}

fn derive<H: ContentHasher>(
    transaction: &RepairTransaction<H>,
    registry: &impl IntentRegistry,
    snapshots: &impl SealedSnapshots,
    evidence: &impl CandidateEvidence,
    standing: Standing,
) -> Result<ReceiptSkeleton, ComposeRefusal> {
    let after = transaction
        .candidate_snapshot()
        .ok_or(ComposeRefusal::NoCandidate)?
        .clone();
    let before = transaction.base_snapshot().clone();
    let handle = transaction.base_intent().clone();

    let record = resolve(
        registry.registered(&handle),
        ComposeRefusal::IntentUnregistered,
        Store::IntentRegistry,
    )?;
    if record.contract.intent_id() != &handle {
        return Err(ComposeRefusal::RegistryInconsistent);
    }
    match (record.standing, standing) {
        (IntentStanding::Accepted, _) | (IntentStanding::Superseded, Standing::Historical) => {}
        (IntentStanding::Superseded, Standing::Current) => {
            return Err(ComposeRefusal::IntentSuperseded);
        }
        (IntentStanding::Proposed, _) => return Err(ComposeRefusal::IntentNotProtected),
    }

    bound_snapshot(snapshots, &before, SnapshotRole::Before, &handle)?;
    bound_snapshot(snapshots, &after, SnapshotRole::After, &handle)?;

    let gate_profile = transaction.gate_profile();
    let repair = transaction.repair_id();
    let status = RefinementCertificateStatus::derive(
        resolve_evidence(evidence.certificates(repair, &after))?,
        &after,
    )?;
    let unknowns = Unknowns::derive(
        gate_profile,
        transaction.gates().map(|gate| (gate.name(), gate.status())),
        &status,
        resolve_evidence(evidence.unsupported_pack_cases(repair, &after))?,
    )?;
    Ok(ReceiptSkeleton {
        repair_transaction: transaction.repair_id().clone(),
        intent: ReceiptIntent {
            handle,
            identity: record.contract.identity().clone(),
        },
        snapshots: ReceiptSnapshots { before, after },
        gate_profile,
        not_yet_enforced: NotYetEnforced::of(gate_profile),
        status,
        unknowns,
    })
}

// --- claimed receipts --------------------------------------------------------------

/// A claimed gate status. The schema admits two on a receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ClaimedGateStatus {
    /// `passed`.
    Passed,
    /// `not_yet_enforced`.
    NotYetEnforced,
}

/// Why a claimed receipt could not be read. No variant echoes the claim's text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimRefusal {
    /// The bytes exceed [`MAX_CLAIMED_RECEIPT_BYTES`]; nothing was parsed.
    TooLarge {
        /// The bound.
        limit: usize,
    },
    /// The bytes are not one JSON document.
    NotJson,
    /// The bytes are JSON, but outside the ID5 vocabulary the one canonical reader admits:
    /// a fraction, an exponent, or an integer outside `i64`. The receipt schema types three
    /// coverage fields `number`, so a schema-valid receipt can land here (open question in
    /// the module docs). Typed so it is never mistaken for garbage (INV-008).
    OutsideId5Vocabulary,
    /// The document is not an object.
    NotAnObject,
    /// A top-level key the schema does not declare.
    UndeclaredField,
    /// An owned field is absent.
    Missing(ReceiptField),
    /// An owned field has the wrong type or does not match its schema pattern or enum.
    Malformed(ReceiptField),
    /// `gates` does not hold exactly twelve entries.
    GateCount,
    /// A gate is listed twice.
    DuplicateGate(GateName),
    /// `coverage` is not an object, or names a group the schema does not declare.
    MalformedCoverage,
}

impl fmt::Display for ClaimRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { limit } => write!(f, "the claimed receipt exceeds {limit} bytes"),
            Self::NotJson => f.write_str("the claimed receipt is not a JSON document"),
            Self::OutsideId5Vocabulary => f.write_str(
                "the claimed receipt carries a number outside the ID5 integer vocabulary",
            ),
            Self::NotAnObject => f.write_str("the claimed receipt is not an object"),
            Self::UndeclaredField => f.write_str("the claimed receipt has an undeclared field"),
            Self::Missing(field) => write!(f, "`{}` is missing", field.property()),
            Self::Malformed(field) => write!(f, "`{}` is malformed", field.property()),
            Self::GateCount => f.write_str("`gates` does not list exactly twelve gates"),
            Self::DuplicateGate(gate) => write!(f, "`{}` is listed twice", gate.token()),
            Self::MalformedCoverage => f.write_str("`coverage` is malformed"),
        }
    }
}

impl std::error::Error for ClaimRefusal {}

/// The owned fields of a receipt someone else produced. Everything here is a claim;
/// [`verify_skeleton`] reads it and copies none of it into its result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimedReceipt {
    repair_transaction: RepairId,
    intent: IntentId,
    base_snapshot: SnapshotId,
    result_snapshot: SnapshotId,
    gate_profile: GateProfile,
    gates: [ClaimedGateStatus; 12],
    unknowns: Vec<String>,
    coverage_groups: Vec<CoverageGroup>,
}

/// Every group the schema's closed `coverage` object declares, sorted by code point.
const COVERAGE_PROPERTIES: [&str; 7] = [
    "certificate_rebuild",
    "defect_mutants",
    "incremental_parity",
    "neighborhood",
    "property_mutation",
    "refinement_coverage",
    "replay",
];

/// The IMPL-05 groups a claimed `coverage` carries. Other groups are IMPL-04's and are
/// not read beyond their names.
fn claimed_coverage(value: Option<&Json>) -> Result<Vec<CoverageGroup>, ClaimRefusal> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let groups = value.as_object().ok_or(ClaimRefusal::MalformedCoverage)?;
    if groups
        .keys()
        .any(|key| COVERAGE_PROPERTIES.binary_search(&key.as_str()).is_err())
    {
        return Err(ClaimRefusal::MalformedCoverage);
    }
    Ok(CoverageGroup::ALL
        .into_iter()
        .filter(|group| groups.contains_key(group.property()))
        .collect())
}

fn claimed_unknowns(value: &Json) -> Result<Vec<String>, ClaimRefusal> {
    let malformed = ClaimRefusal::Malformed(ReceiptField::Unknowns);
    value
        .as_array()
        .ok_or_else(|| malformed.clone())?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_owned)
                .ok_or_else(|| malformed.clone())
        })
        .collect()
}

const fn gate_index(gate: GateName) -> usize {
    match gate {
        GateName::BaseReplay => 0,
        GateName::PatchApplication => 1,
        GateName::IntentIntegrity => 2,
        GateName::ExactRegression => 3,
        GateName::Neighborhood => 4,
        GateName::PropertyMutation => 5,
        GateName::DefectMutants => 6,
        GateName::RefinementCoverage => 7,
        GateName::CertificateRebuild => 8,
        GateName::IncrementalParity => 9,
        GateName::CodeAndSecurity => 10,
        GateName::ReceiptGeneration => 11,
    }
}

fn gate_from_token(token: &str) -> Option<GateName> {
    Some(match token {
        "base_replay" => GateName::BaseReplay,
        "patch_application" => GateName::PatchApplication,
        "intent_integrity" => GateName::IntentIntegrity,
        "exact_regression" => GateName::ExactRegression,
        "neighborhood" => GateName::Neighborhood,
        "property_mutation" => GateName::PropertyMutation,
        "defect_mutants" => GateName::DefectMutants,
        "refinement_coverage" => GateName::RefinementCoverage,
        "certificate_rebuild" => GateName::CertificateRebuild,
        "incremental_parity" => GateName::IncrementalParity,
        "code_and_security" => GateName::CodeAndSecurity,
        "receipt_generation" => GateName::ReceiptGeneration,
        _ => return None,
    })
}

fn profile_from_token(token: &str) -> Option<GateProfile> {
    Some(match token {
        "phase-b" => GateProfile::PhaseB,
        "phase-c" => GateProfile::PhaseC,
        "phase-d" => GateProfile::PhaseD,
        "default" => GateProfile::Default,
        _ => return None,
    })
}

fn claimed_status_from_token(token: &str) -> Option<ClaimedGateStatus> {
    match token {
        "passed" => Some(ClaimedGateStatus::Passed),
        UnenforcedGate::STATUS => Some(ClaimedGateStatus::NotYetEnforced),
        _ => None,
    }
}

fn string_field(
    fields: &BTreeMap<String, Json>,
    field: ReceiptField,
) -> Result<&str, ClaimRefusal> {
    fields
        .get(field.property())
        .ok_or(ClaimRefusal::Missing(field))?
        .as_str()
        .ok_or(ClaimRefusal::Malformed(field))
}

fn claimed_gates(value: &Json) -> Result<[ClaimedGateStatus; 12], ClaimRefusal> {
    let malformed = ClaimRefusal::Malformed(ReceiptField::Gates);
    let items = value.as_array().ok_or_else(|| malformed.clone())?;
    // Refused before any entry is read, so the loop below is bounded by twelve.
    if items.len() != GateName::ALL.len() {
        return Err(ClaimRefusal::GateCount);
    }
    let mut seen: [Option<ClaimedGateStatus>; 12] = [None; 12];
    for item in items {
        let entry = item.as_object().ok_or_else(|| malformed.clone())?;
        if entry.len() != 2 {
            return Err(malformed);
        }
        let gate = entry
            .get("gate")
            .and_then(Json::as_str)
            .and_then(gate_from_token)
            .ok_or_else(|| malformed.clone())?;
        let status = entry
            .get("status")
            .and_then(Json::as_str)
            .and_then(claimed_status_from_token)
            .ok_or_else(|| malformed.clone())?;
        let slot = &mut seen[gate_index(gate)];
        if slot.is_some() {
            return Err(ClaimRefusal::DuplicateGate(gate));
        }
        *slot = Some(status);
    }
    // Twelve entries and no duplicate cover all twelve gates.
    let mut gates = [ClaimedGateStatus::NotYetEnforced; 12];
    for (target, status) in gates.iter_mut().zip(seen) {
        *target = status.ok_or_else(|| malformed.clone())?;
    }
    Ok(gates)
}

impl ClaimedReceipt {
    /// Read a claimed receipt's owned fields from bytes.
    ///
    /// # Errors
    ///
    /// [`ClaimRefusal::TooLarge`] before any parsing when `bytes` exceeds
    /// [`MAX_CLAIMED_RECEIPT_BYTES`]; otherwise as [`Self::from_json`].
    pub fn parse(bytes: &[u8]) -> Result<Self, ClaimRefusal> {
        if bytes.len() > MAX_CLAIMED_RECEIPT_BYTES {
            return Err(ClaimRefusal::TooLarge {
                limit: MAX_CLAIMED_RECEIPT_BYTES,
            });
        }
        let json = Json::parse(bytes).map_err(|error| match error {
            JsonError::FloatingPoint { .. } | JsonError::IntegerOutOfRange { .. } => {
                ClaimRefusal::OutsideId5Vocabulary
            }
            _ => ClaimRefusal::NotJson,
        })?;
        Self::from_json(&json)
    }

    /// Read a claimed receipt's owned fields from a parsed document.
    ///
    /// Strict where the schema is: an undeclared top-level key, a handle off its pattern,
    /// a token off its enum, a gate list that is not twelve unique `{gate, status}`
    /// entries, a status other than `passed` and `not_yet_enforced`, an `unknowns` that is
    /// absent or not an array of strings, or a `coverage` that is not an object of
    /// declared groups is refused. Other fields are not read, and of `coverage` only the
    /// presence of the two IMPL-05 groups.
    ///
    /// # Errors
    ///
    /// [`ClaimRefusal`].
    pub fn from_json(json: &Json) -> Result<Self, ClaimRefusal> {
        let fields = json.as_object().ok_or(ClaimRefusal::NotAnObject)?;
        if fields
            .keys()
            .any(|key| RECEIPT_PROPERTIES.binary_search(&key.as_str()).is_err())
        {
            return Err(ClaimRefusal::UndeclaredField);
        }
        let malformed = ClaimRefusal::Malformed;
        let repair_transaction =
            RepairId::new(string_field(fields, ReceiptField::RepairTransaction)?)
                .map_err(|_| malformed(ReceiptField::RepairTransaction))?;
        let intent = IntentId::new(string_field(fields, ReceiptField::Intent)?)
            .map_err(|_| malformed(ReceiptField::Intent))?;
        let base_snapshot = SnapshotId::new(string_field(fields, ReceiptField::BaseSnapshot)?)
            .map_err(|_| malformed(ReceiptField::BaseSnapshot))?;
        let result_snapshot = SnapshotId::new(string_field(fields, ReceiptField::ResultSnapshot)?)
            .map_err(|_| malformed(ReceiptField::ResultSnapshot))?;
        let gate_profile = profile_from_token(string_field(fields, ReceiptField::GateProfile)?)
            .ok_or(malformed(ReceiptField::GateProfile))?;
        let gates = claimed_gates(
            fields
                .get(ReceiptField::Gates.property())
                .ok_or(ClaimRefusal::Missing(ReceiptField::Gates))?,
        )?;
        // RFC 0032 correction 11: an absent `unknowns` is not an empty one.
        let unknowns = claimed_unknowns(
            fields
                .get(ReceiptField::Unknowns.property())
                .ok_or(ClaimRefusal::Missing(ReceiptField::Unknowns))?,
        )?;
        let coverage_groups = claimed_coverage(fields.get("coverage"))?;
        Ok(Self {
            repair_transaction,
            intent,
            base_snapshot,
            result_snapshot,
            gate_profile,
            gates,
            unknowns,
            coverage_groups,
        })
    }

    /// The claimed `repair_transaction`.
    #[must_use]
    pub const fn repair_transaction(&self) -> &RepairId {
        &self.repair_transaction
    }

    /// The claimed status of `gate`.
    #[must_use]
    pub const fn gate(&self, gate: GateName) -> ClaimedGateStatus {
        self.gates[gate_index(gate)]
    }
}

// --- verification ------------------------------------------------------------------

/// Why a claimed receipt's owned fields failed verification. No variant echoes a claimed
/// value; the ones that name a gate or a profile name a closed token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyRefusal {
    /// The claim could not be read.
    Claim(ClaimRefusal),
    /// The named transaction does not resolve, or the caller has no standing to read it.
    TransactionUnknown,
    /// A store did not answer. Not a verdict on the claim (INV-008).
    StoreUnavailable(Store),
    /// The transaction store returned a version under a handle that is not its own.
    TransactionStoreInconsistent,
    /// The skeleton could not be re-derived from the stores.
    Compose(ComposeRefusal),
    /// The claimed `intent` is not the transaction's base intent.
    IntentMismatch,
    /// A claimed snapshot is not the transaction's own.
    SnapshotMismatch(SnapshotRole),
    /// The claimed `gate_profile` is not the one the transaction was opened under. Only
    /// the claimed profile is named: the refusal does not disclose the transaction's.
    ProfileMismatch {
        /// The claimed profile.
        claimed: GateProfile,
    },
    /// A gate outside the profile is claimed `passed`: an unenforced gate rendered as
    /// passed, which RFC 0032 forbids.
    UnenforcedGateClaimedPassed(GateName),
    /// A gate inside the profile is listed `not_yet_enforced`.
    EnforcedGateListedUnenforced(GateName),
    /// The claimed status of a gate is not the status the transaction records. A claim
    /// listing an in-profile gate `passed` on a transaction that records it `pending`
    /// is refuted by the referenced artifact, not merely unverified.
    GateNotOnRecord(GateName),
    /// At promotion, the record already shows gate 12 `passed`: the receipt would be
    /// certified before it was verified (RFC 0032: gate 12 is `pending` on a `ready`
    /// transaction and moves only after verification).
    ReceiptGenerationAlreadyPassed,
    /// The claim carries a `coverage` group the daemon's evidence does not derive: a
    /// forged refinement or certificate-rebuild measurement.
    CoverageNotDerived(CoverageGroup),
    /// A derived unknown is missing from the claim's `unknowns`.
    UnknownOmitted(UnknownKind),
    /// The claim's `unknowns` lists an entry the derivation does not produce.
    UnknownNotDerived,
    /// The claim's `unknowns` lists one entry twice.
    UnknownDuplicated,
    /// The claim's `unknowns` holds the derived entries in another order.
    UnknownsNotCanonical,
    /// A gate is listed `passed`, on the claim and on the record, but the evidence it
    /// rests on is absent (gate 8: no refinement coverage; gate 9: no rebuild coverage).
    /// An uncomputable result is never a passed gate (RFC 0032 correction 7).
    GateWithoutEvidence(GateName),
}

impl fmt::Display for VerifyRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Claim(refusal) => write!(f, "{refusal}"),
            Self::TransactionUnknown => {
                f.write_str("the named repair transaction does not resolve")
            }
            Self::TransactionStoreInconsistent => {
                f.write_str("the transaction store answered with another version")
            }
            Self::Compose(refusal) => write!(f, "{refusal}"),
            Self::IntentMismatch => f.write_str("`intent` is not the transaction's base intent"),
            Self::SnapshotMismatch(role) => {
                write!(f, "the {role:?} snapshot is not the transaction's own")
            }
            Self::StoreUnavailable(store) => write!(f, "the {store:?} store did not answer"),
            Self::ProfileMismatch { claimed } => write!(
                f,
                "`gate_profile` claims {}, which is not the transaction's profile",
                claimed.token()
            ),
            Self::UnenforcedGateClaimedPassed(gate) => write!(
                f,
                "`{}` is outside the profile and is claimed passed",
                gate.token()
            ),
            Self::EnforcedGateListedUnenforced(gate) => write!(
                f,
                "`{}` is inside the profile and is listed not_yet_enforced",
                gate.token()
            ),
            Self::GateNotOnRecord(gate) => write!(
                f,
                "`{}`'s claimed status is not the transaction's recorded status",
                gate.token()
            ),
            Self::ReceiptGenerationAlreadyPassed => {
                f.write_str("receipt_generation is already passed before the receipt was verified")
            }
            Self::CoverageNotDerived(group) => {
                write!(f, "`{group}` is claimed, and no evidence derives it")
            }
            Self::UnknownOmitted(kind) => {
                write!(f, "`unknowns` omits a derived `{}` entry", kind.prefix())
            }
            Self::UnknownNotDerived => f.write_str("`unknowns` lists an entry nothing derives"),
            Self::UnknownDuplicated => f.write_str("`unknowns` lists an entry twice"),
            Self::UnknownsNotCanonical => {
                f.write_str("`unknowns` is not in the derived canonical order")
            }
            Self::GateWithoutEvidence(gate) => write!(
                f,
                "`{}` is listed passed, and the evidence it rests on is absent",
                gate.token()
            ),
        }
    }
}

impl std::error::Error for VerifyRefusal {}

impl From<ClaimRefusal> for VerifyRefusal {
    fn from(refusal: ClaimRefusal) -> Self {
        Self::Claim(refusal)
    }
}

impl From<ComposeRefusal> for VerifyRefusal {
    fn from(refusal: ComposeRefusal) -> Self {
        Self::Compose(refusal)
    }
}

impl From<UnknownsMismatch> for VerifyRefusal {
    fn from(mismatch: UnknownsMismatch) -> Self {
        match mismatch {
            UnknownsMismatch::Duplicated => Self::UnknownDuplicated,
            UnknownsMismatch::Omitted(kind) => Self::UnknownOmitted(kind),
            UnknownsMismatch::NotDerived => Self::UnknownNotDerived,
            UnknownsMismatch::NotCanonical => Self::UnknownsNotCanonical,
        }
    }
}

/// The verdict on a whole receipt. There is no `Verified` member yet: the skeleton
/// verifies its own fields only, and the rest are [`ReceiptSeam`]s (INV-008).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiptVerdict {
    /// The owned fields match the referenced artifacts; these seams are unverified.
    Incomplete {
        /// What no check has covered.
        unverified: &'static [ReceiptSeam],
    },
}

/// The result of [`verify_skeleton`]: the skeleton re-derived from the stores, which the
/// claim's owned fields matched. It holds no claimed value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkeletonVerification {
    derived: ReceiptSkeleton,
}

impl SkeletonVerification {
    /// The skeleton derived from the referenced artifacts.
    #[must_use]
    pub const fn derived(&self) -> &ReceiptSkeleton {
        &self.derived
    }

    /// The in-profile gates, which the claim and the transaction's record both list
    /// `passed` and whose evidence this skeleton has not checked
    /// ([`ReceiptSeam::EnforcedGateOutcomes`]).
    #[must_use]
    pub fn enforced_gates_unverified(&self) -> Vec<GateName> {
        let profile = self.derived.gate_profile;
        GateName::ALL
            .into_iter()
            .filter(|gate| profile.enforces(*gate))
            .collect()
    }

    /// The verdict on the whole receipt: always [`ReceiptVerdict::Incomplete`] today.
    #[must_use]
    pub const fn receipt_verdict(&self) -> ReceiptVerdict {
        ReceiptVerdict::Incomplete {
            unverified: &ReceiptSeam::ALL,
        }
    }
}

/// Which moment a receipt is verified at. RFC 0032 fixes the order: "Gate 12 is the
/// promotion step, not a precondition of it. `receipt_generation` is `pending` on a
/// `ready` transaction … Promotion composes the receipt, verifies it, flips gate 12 to
/// `passed`, and only then records `promoted`." So the record a receipt is checked
/// against differs in exactly one gate between the two stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum VerificationStage {
    /// `repair.promote` step 5, before gate 12 moves. The record must show gate 12
    /// `pending`; a record that already shows it `passed` has certified a receipt nobody
    /// verified, and is refused [`VerifyRefusal::ReceiptGenerationAlreadyPassed`]. The
    /// base intent must still be the protected head.
    Promotion,
    /// A published receipt, checked later (`evidence.verify`). The record must show gate
    /// 12 `passed`, and a superseded base intent stays admissible (RFC 0032: published
    /// receipts remain verifiable). That the version is `promoted` and binds this receipt
    /// is [`ReceiptSeam::PromotionRecord`].
    Published,
}

fn resolve_transaction<H: ContentHasher>(
    claim: &ClaimedReceipt,
    transactions: &impl TransactionStore<H>,
) -> Result<RepairTransaction<H>, VerifyRefusal> {
    let transaction = match transactions.transaction(&claim.repair_transaction) {
        Resolution::Found(transaction) => transaction,
        Resolution::Unknown => return Err(VerifyRefusal::TransactionUnknown),
        Resolution::Unavailable => {
            return Err(VerifyRefusal::StoreUnavailable(Store::Transactions));
        }
    };
    if transaction.repair_id() != &claim.repair_transaction {
        return Err(VerifyRefusal::TransactionStoreInconsistent);
    }
    Ok(transaction)
}

/// The shared check: re-derive, compare the owned fields, then compare every gate with
/// the record. Gate 12's expected record depends on `stage`.
/// The refusal for a claimed IMPL-05 coverage group. The `match` is exhaustive, so a
/// derived member added later must say how its values are compared.
const fn claimed_group_refusal(
    status: &RefinementCertificateStatus,
    group: CoverageGroup,
) -> Option<VerifyRefusal> {
    match status.coverage(group) {
        GroupDerivation::Absent => Some(VerifyRefusal::CoverageNotDerived(group)),
    }
}

fn verify_at<H: ContentHasher>(
    claim: &ClaimedReceipt,
    stage: VerificationStage,
    transaction: &RepairTransaction<H>,
    registry: &impl IntentRegistry,
    snapshots: &impl SealedSnapshots,
    evidence: &impl CandidateEvidence,
) -> Result<ReceiptSkeleton, VerifyRefusal> {
    let standing = match stage {
        VerificationStage::Promotion => Standing::Current,
        VerificationStage::Published => Standing::Historical,
    };
    let derived = derive(transaction, registry, snapshots, evidence, standing)?;

    if claim.gate_profile != derived.gate_profile {
        return Err(VerifyRefusal::ProfileMismatch {
            claimed: claim.gate_profile,
        });
    }
    if claim.intent != derived.intent.handle {
        return Err(VerifyRefusal::IntentMismatch);
    }
    if claim.base_snapshot != derived.snapshots.before {
        return Err(VerifyRefusal::SnapshotMismatch(SnapshotRole::Before));
    }
    if claim.result_snapshot != derived.snapshots.after {
        return Err(VerifyRefusal::SnapshotMismatch(SnapshotRole::After));
    }
    for gate in GateName::ALL {
        match (derived.not_yet_enforced.contains(gate), claim.gate(gate)) {
            (true, ClaimedGateStatus::Passed) => {
                return Err(VerifyRefusal::UnenforcedGateClaimedPassed(gate));
            }
            (false, ClaimedGateStatus::NotYetEnforced) => {
                return Err(VerifyRefusal::EnforcedGateListedUnenforced(gate));
            }
            (true, ClaimedGateStatus::NotYetEnforced) | (false, ClaimedGateStatus::Passed) => {}
        }
    }
    // The record, not the claim, says what each gate's status is. `gates()` is in
    // gate-number order, the order of `GateName::ALL`. Gate 12 is always enforced, so
    // the claim lists it `passed` — the only receipt token for an enforced gate — and
    // the stage decides what the record must say.
    for (gate, recorded) in GateName::ALL.into_iter().zip(transaction.gates()) {
        if recorded.name() != gate {
            return Err(VerifyRefusal::GateNotOnRecord(gate));
        }
        if gate == GateName::ReceiptGeneration {
            match (stage, recorded.status()) {
                (VerificationStage::Promotion, GateStatus::Pending)
                | (VerificationStage::Published, GateStatus::Passed) => continue,
                (VerificationStage::Promotion, GateStatus::Passed) => {
                    return Err(VerifyRefusal::ReceiptGenerationAlreadyPassed);
                }
                _ => return Err(VerifyRefusal::GateNotOnRecord(gate)),
            }
        }
        let on_record = match claim.gate(gate) {
            ClaimedGateStatus::Passed => GateStatus::Passed,
            ClaimedGateStatus::NotYetEnforced => GateStatus::NotYetEnforced,
        };
        if recorded.status() != on_record {
            return Err(VerifyRefusal::GateNotOnRecord(gate));
        }
    }
    // IMPL-05: a coverage group is present only when the evidence derives it, and never
    // today (`status`).
    if let Some(refusal) = claim
        .coverage_groups
        .iter()
        .find_map(|group| claimed_group_refusal(&derived.status, *group))
    {
        return Err(refusal);
    }
    // IMPL-06: the claimed list is the derived list, entry for entry.
    unknowns::compare(&claim.unknowns, &derived.unknowns)?;
    // IMPL-05: an enforced gate 8 or 9 is listed `passed` (the checks above leave no
    // other status for it), so its evidence must exist.
    for group in CoverageGroup::ALL {
        if derived.gate_profile.enforces(group.gate()) {
            match derived.status.coverage(group) {
                GroupDerivation::Absent => {
                    return Err(VerifyRefusal::GateWithoutEvidence(group.gate()));
                }
            }
        }
    }
    Ok(derived)
}

/// Verify a **published** receipt's owned fields by reference ([`VerificationStage::Published`]).
///
/// The transaction the claim names is resolved in `transactions`; the skeleton is
/// re-derived from it and the stores as [`ReceiptSkeleton::compose`] derives it, except
/// that a base intent superseded after promotion stays admissible; then each owned field
/// and every gate of the claim is compared with the derived value and the record, which
/// must show gate 12 `passed`. A claim is never a source. Promotion itself uses
/// [`verify_for_promotion`], never this.
///
/// # Errors
///
/// [`VerifyRefusal`], for the first check that fails, in this order: the transaction,
/// the re-derivation, `gate_profile`, `intent`, `base_snapshot`, `result_snapshot`,
/// `gates` against the profile, then `gates` against the record, each in gate-number
/// order; then the IMPL-05 `coverage` groups, `unknowns`, and the evidence behind gates
/// 8 and 9. No transaction records an in-profile gate `passed` until PR-20 `evaluate`
/// lands, and no refinement checker computes gate 8's evidence, so every claim is
/// refused today, at the latest [`VerifyRefusal::GateWithoutEvidence`].
pub fn verify_skeleton<H: ContentHasher>(
    claim: &ClaimedReceipt,
    transactions: &impl TransactionStore<H>,
    registry: &impl IntentRegistry,
    snapshots: &impl SealedSnapshots,
    evidence: &impl CandidateEvidence,
) -> Result<SkeletonVerification, VerifyRefusal> {
    let transaction = resolve_transaction(claim, transactions)?;
    let derived = verify_at(
        claim,
        VerificationStage::Published,
        &transaction,
        registry,
        snapshots,
        evidence,
    )?;
    Ok(SkeletonVerification { derived })
}

/// The licence to move gate 12 (`receipt_generation`) from `pending` to `passed` on one
/// transaction version: the evidence that the composed receipt's owned fields were
/// verified by reference against that version while gate 12 was still `pending`.
///
/// Only [`verify_for_promotion`] builds one. Recording the transition is `repair.promote`
/// steps 5–7 (PR-20 promote, not landed): it must check that the licence names the
/// lineage head it is promoting, record gate 12 `passed` in the same atomic step that
/// publishes the receipt, and drop the licence — leaving the transaction `ready` with
/// gate 12 `pending` — on any failure (RFC 0032 "Rollback"). The licence covers the
/// skeleton's fields only; every [`ReceiptSeam`] other than
/// [`ReceiptSeam::PromotionRecord`] must be verified before promotion too.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiptGenerationLicense {
    derived: ReceiptSkeleton,
}

impl ReceiptGenerationLicense {
    /// The transaction version the licence is for.
    #[must_use]
    pub const fn repair_transaction(&self) -> &RepairId {
        &self.derived.repair_transaction
    }

    /// The skeleton re-derived from the referenced artifacts.
    #[must_use]
    pub const fn derived(&self) -> &ReceiptSkeleton {
        &self.derived
    }

    /// The gate this licence moves: always `receipt_generation`.
    #[must_use]
    pub const fn gate(&self) -> GateName {
        GateName::ReceiptGeneration
    }

    /// The seams the licence does not cover.
    #[must_use]
    pub const fn unverified(&self) -> &'static [ReceiptSeam] {
        &ReceiptSeam::ALL
    }
}

/// Verify a **composed** receipt's owned fields at promotion, before gate 12 moves
/// ([`VerificationStage::Promotion`]; RFC 0032 "Promotion" step 5).
///
/// As [`verify_skeleton`], except that the record must show every in-profile gate but
/// gate 12 `passed` and gate 12 `pending`, and the base intent must stand `accepted`. The
/// result licenses the gate-12 transition; it does not perform it.
///
/// # Errors
///
/// [`VerifyRefusal`], in the order of [`verify_skeleton`];
/// [`VerifyRefusal::ReceiptGenerationAlreadyPassed`] when the record shows gate 12
/// `passed` before this verification ran.
pub fn verify_for_promotion<H: ContentHasher>(
    claim: &ClaimedReceipt,
    transactions: &impl TransactionStore<H>,
    registry: &impl IntentRegistry,
    snapshots: &impl SealedSnapshots,
    evidence: &impl CandidateEvidence,
) -> Result<ReceiptGenerationLicense, VerifyRefusal> {
    let transaction = resolve_transaction(claim, transactions)?;
    let derived = verify_at(
        claim,
        VerificationStage::Promotion,
        &transaction,
        registry,
        snapshots,
        evidence,
    )?;
    Ok(ReceiptGenerationLicense { derived })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handle::CrashpackId;
    use crate::hypothesis::{ChangeKind, Hypothesis, Proposal};
    use crate::patch::{DeclaredChange, FileEdit, HashedIdentifier};
    use crate::transaction::FailureBinding;
    use continuum_value::identity::Blake3Hasher;
    use continuum_workspace::snapshot::{Snapshot, WorkspaceContent, WorkspacePath};

    const CONTRACT: &[u8] =
        include_bytes!("../../continuum-intent/tests/fixtures/replicated-register-contract.json");
    const CRASH: &str = "crash_unit_ack_before_sync";

    /// A one-file base; the unit tests' change creates a second file beside it.
    fn base() -> WorkspaceContent {
        let mut content = WorkspaceContent::new();
        content
            .insert(
                WorkspacePath::new("src/lib.rs").unwrap(),
                b"fn f() {}\n".to_vec(),
            )
            .unwrap();
        content
    }

    fn base_id() -> SnapshotId {
        let tree = Snapshot::build(&base(), &HashedIdentifier::<Blake3Hasher>::new()).unwrap();
        SnapshotId::new(&tree.identity().to_string()).unwrap()
    }

    fn contract() -> IntentContract {
        IntentContract::decode(CONTRACT).unwrap()
    }

    /// A certificate the test world holds for the candidate.
    #[derive(Clone)]
    enum Cert {
        /// Model-bound to whatever candidate the seam is asked about.
        BoundToCandidate,
        /// Any other verdict.
        Verdict(CertificateVerdict),
    }

    /// The transactions, and the evidence the daemon holds for every candidate.
    struct World(
        Vec<RepairTransaction<Blake3Hasher>>,
        Vec<(&'static str, Cert)>,
    );

    impl World {
        /// The transactions, with one model-bound certificate on the candidate.
        fn of(transactions: Vec<RepairTransaction<Blake3Hasher>>) -> Self {
            Self(transactions, vec![("ev_bound", Cert::BoundToCandidate)])
        }
    }

    impl CandidateEvidence for World {
        fn certificates(
            &self,
            _: &RepairId,
            candidate: &SnapshotId,
        ) -> Resolution<Vec<CertificateRecord>> {
            Resolution::Found(
                self.1
                    .iter()
                    .map(|(handle, cert)| {
                        let verdict = match cert {
                            Cert::BoundToCandidate => CertificateVerdict::ModelBound {
                                snapshot: candidate.clone(),
                            },
                            Cert::Verdict(verdict) => verdict.clone(),
                        };
                        CertificateRecord::new(CertificateNode::new(handle).unwrap(), verdict)
                    })
                    .collect(),
            )
        }

        fn unsupported_pack_cases(
            &self,
            _: &RepairId,
            _: &SnapshotId,
        ) -> Resolution<Vec<PackCase>> {
            Resolution::Found(vec![
                PackCase::new("storage/append-log-v1", "flush-dishonesty").unwrap(),
            ])
        }
    }

    impl FailureBinding for World {
        fn failure_base(&self, failure: &CrashpackId) -> Resolution<SnapshotId> {
            if failure.as_str() == CRASH {
                Resolution::Found(base_id())
            } else {
                Resolution::Unknown
            }
        }
        fn snapshot_intent(&self, _: &SnapshotId) -> Resolution<IntentId> {
            Resolution::Found(contract().intent_id().clone())
        }
    }

    impl IntentRegistry for World {
        fn registered(&self, intent: &IntentId) -> Resolution<RegisteredIntent> {
            if intent == contract().intent_id() {
                Resolution::Found(RegisteredIntent::new(contract(), IntentStanding::Accepted))
            } else {
                Resolution::Unknown
            }
        }
    }

    impl SealedSnapshots for World {
        /// Every snapshot resolves: these unit tests exercise the gate record, and the
        /// snapshot refusals are the integration suite's.
        fn sealed_binding(&self, _: &SnapshotId) -> Resolution<IntentId> {
            Resolution::Found(contract().intent_id().clone())
        }
    }

    impl TransactionStore<Blake3Hasher> for World {
        fn transaction(&self, repair: &RepairId) -> Resolution<RepairTransaction<Blake3Hasher>> {
            self.0
                .iter()
                .find(|tx| tx.repair_id() == repair)
                .cloned()
                .map_or(Resolution::Unknown, Resolution::Found)
        }
    }

    fn recorded(profile: GateProfile) -> RepairTransaction<Blake3Hasher> {
        let applied = RepairTransaction::<Blake3Hasher>::begin(
            CrashpackId::new(CRASH).unwrap(),
            profile,
            &World::of(Vec::new()),
        )
        .unwrap()
        .apply(
            &Proposal::new(
                Hypothesis::new("unit"),
                vec![
                    DeclaredChange::new(
                        ChangeKind::Rust,
                        [(
                            WorkspacePath::new("src/extra.rs").unwrap(),
                            FileEdit::Create {
                                content: b"fn g() {}\n".to_vec(),
                            },
                        )],
                    )
                    .unwrap(),
                ],
            ),
            &base(),
        )
        .unwrap()
        .into_parts()
        .0;
        applied.with_recorded_gates(GateName::ALL.map(|gate| {
            if profile.enforces(gate) {
                GateStatus::Passed
            } else {
                GateStatus::NotYetEnforced
            }
        }))
    }

    /// The honest claim for `tx` under `world`'s evidence: the derived fields, including
    /// `unknowns`, and every in-profile gate `passed`.
    fn claim_bytes(tx: &RepairTransaction<Blake3Hasher>, world: &World) -> Vec<u8> {
        let mut fields = claim_fields(tx, world);
        fields.remove("coverage");
        Json::Object(fields).to_canonical_bytes()
    }

    fn claim_fields(tx: &RepairTransaction<Blake3Hasher>, world: &World) -> BTreeMap<String, Json> {
        let skeleton = ReceiptSkeleton::compose(tx, world, world, world).unwrap();
        let mut fields = match skeleton.fields_json() {
            Json::Object(fields) => fields,
            _ => unreachable!(),
        };
        let gates = GateName::ALL
            .into_iter()
            .map(|gate| {
                let status = if tx.gate_profile().enforces(gate) {
                    "passed"
                } else {
                    UnenforcedGate::STATUS
                };
                Json::Object(BTreeMap::from([
                    ("gate".to_owned(), Json::String(gate.token().to_owned())),
                    ("status".to_owned(), Json::String(status.to_owned())),
                ]))
            })
            .collect();
        fields.insert("gates".to_owned(), Json::Array(gates));
        fields
    }

    /// The record a `ready` transaction carries: gates 1–11 of the profile `passed`, gate
    /// 12 still `pending`.
    fn ready(profile: GateProfile) -> RepairTransaction<Blake3Hasher> {
        let promoted = recorded(profile);
        promoted.with_recorded_gates(promoted.gates().map(|entry| {
            if entry.name() == GateName::ReceiptGeneration {
                GateStatus::Pending
            } else {
                entry.status()
            }
        }))
    }

    fn check_promotion(
        tx: &RepairTransaction<Blake3Hasher>,
    ) -> Result<ReceiptGenerationLicense, VerifyRefusal> {
        let world = World::of(vec![tx.clone()]);
        let claim = ClaimedReceipt::parse(&claim_bytes(tx, &world)).unwrap();
        verify_for_promotion(&claim, &world, &world, &world, &world)
    }

    fn check_published(
        tx: &RepairTransaction<Blake3Hasher>,
    ) -> Result<SkeletonVerification, VerifyRefusal> {
        let world = World::of(vec![tx.clone()]);
        let claim = ClaimedReceipt::parse(&claim_bytes(tx, &world)).unwrap();
        verify_skeleton(&claim, &world, &world, &world, &world)
    }

    /// Verify `fields` at promotion against `world`, which must hold `tx`.
    fn promote_fields(
        fields: BTreeMap<String, Json>,
        world: &World,
    ) -> Result<ReceiptGenerationLicense, VerifyRefusal> {
        let claim = ClaimedReceipt::parse(&Json::Object(fields).to_canonical_bytes())?;
        verify_for_promotion(&claim, world, world, world, world)
    }

    /// cr-tz8fwi: at promotion the record shows gate 12 `pending`, and every check of the
    /// record passes there. The receipt is then refused only for gate 8's absent evidence
    /// (IMPL-05): no refinement checker computes a coverage delta, so no licence is issued
    /// under any profile until one ships.
    #[test]
    fn promotion_passes_the_record_with_gate_12_pending_and_stops_at_gate_8_evidence() {
        for profile in GateProfile::ALL {
            assert_eq!(
                check_promotion(&ready(profile)),
                Err(VerifyRefusal::GateWithoutEvidence(
                    GateName::RefinementCoverage
                )),
                "{}",
                profile.token()
            );
        }
    }

    /// cr-tz8fwi: a record that shows gate 12 `passed` before verification ran has
    /// certified an unverified receipt; promotion refuses it.
    #[test]
    fn promotion_refuses_a_record_with_gate_12_already_passed() {
        for profile in GateProfile::ALL {
            assert_eq!(
                check_promotion(&recorded(profile)),
                Err(VerifyRefusal::ReceiptGenerationAlreadyPassed)
            );
        }
    }

    /// The whole sequence: at `ready` (gate 12 pending) promotion passes the record; a
    /// receipt is not published while gate 12 is pending; after gate 12 moves (simulated
    /// by the test recorder; PR-20 owns it) the published check passes the record too,
    /// and promotion verification no longer does. Both stop at gate 8's evidence.
    #[test]
    fn the_promotion_sequence_checks_gate_12_at_each_stage() {
        let before = ready(GateProfile::PhaseB);
        assert_eq!(
            check_promotion(&before),
            Err(VerifyRefusal::GateWithoutEvidence(
                GateName::RefinementCoverage
            ))
        );
        assert_eq!(
            check_published(&before),
            Err(VerifyRefusal::GateNotOnRecord(GateName::ReceiptGeneration)),
            "a receipt is not published while gate 12 is pending"
        );
        let after = before.with_recorded_gates(before.gates().map(|entry| {
            if entry.name() == GateName::ReceiptGeneration {
                GateStatus::Passed
            } else {
                entry.status()
            }
        }));
        assert_eq!(
            check_published(&after),
            Err(VerifyRefusal::GateWithoutEvidence(
                GateName::RefinementCoverage
            ))
        );
        assert_eq!(
            check_promotion(&after),
            Err(VerifyRefusal::ReceiptGenerationAlreadyPassed)
        );
    }

    /// Under `phase-d` both gates 8 and 9 are enforced; gate 8 is refused first.
    #[test]
    fn pr22_impl05_gate_evidence_is_checked_in_gate_order() {
        for profile in [GateProfile::PhaseD, GateProfile::Default] {
            assert_eq!(
                check_published(&recorded(profile)),
                Err(VerifyRefusal::GateWithoutEvidence(
                    GateName::RefinementCoverage
                ))
            );
        }
    }

    /// pr22-impl05 forged status: a claim that matches the record and the unknowns, and
    /// adds a refinement or certificate-rebuild coverage group, is refused — the numbers
    /// are a measurement no daemon evidence derives.
    #[test]
    fn pr22_impl05_negative_a_forged_coverage_group_is_refused() {
        let tx = ready(GateProfile::PhaseD);
        let world = World::of(vec![tx.clone()]);
        let refinement = Json::Object(BTreeMap::from([
            ("after".to_owned(), Json::Integer(1)),
            ("before".to_owned(), Json::Integer(1)),
            ("declared_decrease".to_owned(), Json::Bool(false)),
            ("delta".to_owned(), Json::Integer(0)),
        ]));
        let rebuild = Json::Object(BTreeMap::from([
            ("invalidated".to_owned(), Json::Integer(0)),
            ("rebuilt".to_owned(), Json::Integer(0)),
            ("stale".to_owned(), Json::Integer(0)),
        ]));
        for (group, value) in [
            (CoverageGroup::RefinementCoverage, refinement),
            (CoverageGroup::CertificateRebuild, rebuild),
        ] {
            let mut fields = claim_fields(&tx, &world);
            fields.insert(
                "coverage".to_owned(),
                Json::Object(BTreeMap::from([(group.property().to_owned(), value)])),
            );
            assert_eq!(
                promote_fields(fields, &world),
                Err(VerifyRefusal::CoverageNotDerived(group))
            );
        }
        // Another group is IMPL-04's and is not read beyond its name; an undeclared one
        // is refused by the reader.
        let mut other = claim_fields(&tx, &world);
        other.insert(
            "coverage".to_owned(),
            Json::Object(BTreeMap::from([("replay".to_owned(), Json::Null)])),
        );
        assert_eq!(
            promote_fields(other, &world),
            Err(VerifyRefusal::GateWithoutEvidence(
                GateName::RefinementCoverage
            ))
        );
        let mut undeclared = claim_fields(&tx, &world);
        undeclared.insert(
            "coverage".to_owned(),
            Json::Object(BTreeMap::from([("adequacy".to_owned(), Json::Null)])),
        );
        assert_eq!(
            promote_fields(undeclared, &world),
            Err(VerifyRefusal::Claim(ClaimRefusal::MalformedCoverage))
        );
    }

    fn set_unknowns(fields: &mut BTreeMap<String, Json>, entries: Vec<String>) {
        fields.insert(
            "unknowns".to_owned(),
            Json::Array(entries.into_iter().map(Json::String).collect()),
        );
    }

    fn derived_unknowns(tx: &RepairTransaction<Blake3Hasher>, world: &World) -> Vec<String> {
        ReceiptSkeleton::compose(tx, world, world, world)
            .unwrap()
            .unknowns()
            .tokens()
    }

    /// pr22-impl05/06 forged status: a claim that drops the unknown for an unverified
    /// certificate — presenting it as if it counted — is refused, for every reason a
    /// certificate can fail to count. So is one that drops `certificate_none_verified`
    /// when the daemon holds no verified certificate, and one that drops the refinement
    /// or not-yet-enforced entries.
    #[test]
    fn pr22_impl05_negative_a_forged_certificate_status_is_refused() {
        let tx = ready(GateProfile::PhaseB);
        let verdicts = [
            CertificateVerdict::Rejected,
            CertificateVerdict::Unchecked,
            CertificateVerdict::ModelBound {
                snapshot: SnapshotId::new("ws_some_other_snapshot").unwrap(),
            },
            CertificateVerdict::InsufficientEvidence(
                status::InsufficientReason::TrustsModelCorrespondence,
            ),
            CertificateVerdict::InsufficientEvidence(
                status::InsufficientReason::FamilyNotModelBound,
            ),
            CertificateVerdict::InsufficientEvidence(status::InsufficientReason::NoSealedSnapshot),
            CertificateVerdict::InsufficientEvidence(status::InsufficientReason::NoHeldModel),
            CertificateVerdict::InsufficientEvidence(status::InsufficientReason::ContentMissing),
            CertificateVerdict::InsufficientEvidence(status::InsufficientReason::CheckerNotServed),
            CertificateVerdict::InsufficientEvidence(status::InsufficientReason::WireUnsupported),
            CertificateVerdict::KernelInconclusive,
            CertificateVerdict::Redacted,
            CertificateVerdict::Stale,
        ];
        for verdict in verdicts {
            let world = World(
                vec![tx.clone()],
                vec![
                    ("ev_bound", Cert::BoundToCandidate),
                    ("ev_claimed", Cert::Verdict(verdict.clone())),
                ],
            );
            let honest = derived_unknowns(&tx, &world);
            let forged: Vec<String> = honest
                .iter()
                .filter(|entry| !entry.starts_with("certificate_unverified:ev_claimed:"))
                .cloned()
                .collect();
            assert_eq!(forged.len() + 1, honest.len(), "{verdict:?}");
            let mut fields = claim_fields(&tx, &world);
            set_unknowns(&mut fields, forged);
            assert_eq!(
                promote_fields(fields, &world),
                Err(VerifyRefusal::UnknownOmitted(
                    UnknownKind::CertificateUnverified
                )),
                "{verdict:?}"
            );
        }

        // Missing certificate: the daemon holds none, so the receipt must say so.
        let world = World(vec![tx.clone()], Vec::new());
        let honest = derived_unknowns(&tx, &world);
        assert!(honest.contains(&"certificate_none_verified".to_owned()));
        for (dropped, kind) in [
            (
                "certificate_none_verified",
                UnknownKind::NoVerifiedCertificate,
            ),
            (
                "refinement_not_computed:checker_not_deployed",
                UnknownKind::RefinementNotComputed,
            ),
            (
                "gate_not_yet_enforced:certificate_rebuild",
                UnknownKind::GateNotYetEnforced,
            ),
            (
                "unsupported_pack_case:storage/append-log-v1:flush-dishonesty",
                UnknownKind::UnsupportedPackCase,
            ),
        ] {
            let mut fields = claim_fields(&tx, &world);
            set_unknowns(
                &mut fields,
                honest.iter().filter(|e| *e != dropped).cloned().collect(),
            );
            assert_eq!(
                promote_fields(fields, &world),
                Err(VerifyRefusal::UnknownOmitted(kind)),
                "{dropped}"
            );
        }
    }

    /// pr22-impl06 negative: an invented, duplicated or reordered entry, and a missing
    /// `unknowns`, are each refused; the honest list passes to the gate-evidence check.
    #[test]
    fn pr22_impl06_negative_the_claimed_unknowns_must_be_the_derived_list() {
        let tx = ready(GateProfile::PhaseB);
        let world = World::of(vec![tx.clone()]);
        let honest = derived_unknowns(&tx, &world);
        assert!(honest.len() >= 3);

        let mut fields = claim_fields(&tx, &world);
        set_unknowns(&mut fields, honest.clone());
        assert_eq!(
            promote_fields(fields, &world),
            Err(VerifyRefusal::GateWithoutEvidence(
                GateName::RefinementCoverage
            ))
        );

        let mut invented = honest.clone();
        invented.push("gate_pending:base_replay".to_owned());
        let mut duplicated = honest.clone();
        duplicated.push(honest[0].clone());
        let mut reordered = honest.clone();
        reordered.swap(0, 1);
        for (entries, refusal) in [
            (invented, VerifyRefusal::UnknownNotDerived),
            (duplicated, VerifyRefusal::UnknownDuplicated),
            (reordered, VerifyRefusal::UnknownsNotCanonical),
        ] {
            let mut fields = claim_fields(&tx, &world);
            set_unknowns(&mut fields, entries);
            assert_eq!(promote_fields(fields, &world), Err(refusal));
        }

        let mut absent = claim_fields(&tx, &world);
        absent.remove("unknowns");
        assert_eq!(
            promote_fields(absent, &world),
            Err(VerifyRefusal::Claim(ClaimRefusal::Missing(
                ReceiptField::Unknowns
            )))
        );
        let mut not_strings = claim_fields(&tx, &world);
        not_strings.insert("unknowns".to_owned(), Json::Array(vec![Json::Integer(1)]));
        assert_eq!(
            promote_fields(not_strings, &world),
            Err(VerifyRefusal::Claim(ClaimRefusal::Malformed(
                ReceiptField::Unknowns
            )))
        );
    }

    /// A published receipt passes every record check, for every profile, and stops at
    /// gate 8's evidence.
    #[test]
    fn a_published_receipt_passes_its_record_checks_under_every_profile() {
        for profile in GateProfile::ALL {
            assert_eq!(
                check_published(&recorded(profile)),
                Err(VerifyRefusal::GateWithoutEvidence(
                    GateName::RefinementCoverage
                ))
            );
        }
    }

    /// Any of gates 1–11 of the profile still `pending` refuses at both stages.
    #[test]
    fn a_single_gate_off_the_record_refuses() {
        let base = ready(GateProfile::PhaseB);
        for gate in GateName::ALL
            .into_iter()
            .filter(|g| GateProfile::PhaseB.enforces(*g) && *g != GateName::ReceiptGeneration)
        {
            let statuses = base.gates().map(|entry| {
                if entry.name() == gate {
                    GateStatus::Pending
                } else {
                    entry.status()
                }
            });
            let tx = base.with_recorded_gates(statuses);
            assert_eq!(
                check_promotion(&tx),
                Err(VerifyRefusal::GateNotOnRecord(gate))
            );
        }
    }

    #[test]
    fn receipt_properties_are_sorted_and_unique() {
        assert!(RECEIPT_PROPERTIES.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn gate_index_is_the_gate_number_order() {
        for (index, gate) in GateName::ALL.into_iter().enumerate() {
            assert_eq!(gate_index(gate), index);
            assert_eq!(gate_from_token(gate.token()), Some(gate));
        }
        for profile in GateProfile::ALL {
            assert_eq!(profile_from_token(profile.token()), Some(profile));
        }
    }
}
