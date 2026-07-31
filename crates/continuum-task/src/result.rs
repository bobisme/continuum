//! The typed machine result of a verification task (the PR-1 exit).
//!
//! # The condition this module discharges
//!
//! > **Exit:** an unsupported empty task returns a valid machine result naming every
//! > epoch and no misleading success flag.
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 1
//!
//! > Every result MUST name all six epochs. An epoch the result cannot pin reads null;
//! > it is never an absent field. An unsupported or empty task still returns a valid
//! > machine result naming every epoch and carries no success flag.
//! >
//! > — `notes/plan/schemas/continuumd-native-protocol.idl`, rule `envelope.epochs_named`
//!
//! [`VerificationTaskResult`] is the smallest type that can carry that claim. It holds
//! three things and nothing else: the six epochs the result is pinned to
//! ([`EpochSet`]), the nine-dimension assurance envelope ([`AssuranceEnvelope`]), and a
//! typed [`TaskOutcome`] whose inconclusive case cannot exist without its INV-008
//! reason.
//!
//! # The empty task
//!
//! An *empty* task declares nothing to verify, so no engine is asked to cover any
//! envelope dimension; at PR 1 there is no engine to ask either — plan §20's
//! `continuum-engine-*` crates are later PRs. Both readings give the same honest
//! answer, and [`VerificationTaskResult::unsupported_empty_task`] constructs it: nine
//! named dimensions each carrying a typed `Unsupported(reason)`, six named epochs, and
//! the outcome `Inconclusive(Unsupported)`.
//!
//! # `Unsupported` is a reason, never a verdict
//!
//! > Unsupported and engine-error outcomes are not verdicts either: they are
//! > inconclusive with `inconclusive_reason` Unsupported/EngineError (INV-008).
//! >
//! > — `notes/plan/schemas/assurance-result.schema.json`, `verdict`
//!
//! The verdict vocabulary is closed at three — `established`, `refuted`,
//! `inconclusive` — and `Unsupported` appears only *inside* the third. [`TaskOutcome`]
//! encodes that by construction: `Unsupported` is reachable only as the payload of
//! [`TaskOutcome::Inconclusive`], and that payload is not optional, so an inconclusive
//! outcome without an INV-008 reason does not compile. This is the pairing
//! `continuum-value`'s assurance module and `continuum-evidence`'s claim-status lattice
//! each left as a seam for the PR-1 exit to join:
//! [`ClaimStatus::schema_obligations`](continuum_evidence::claim_status::ClaimStatus::schema_obligations)
//! states that an `inconclusive` claim owes a typed reason;
//! [`TaskOutcome::claim_status`] names that status and
//! [`VerificationTaskResult::inconclusive_reason`] pays the debt.
//!
//! # No success flag
//!
//! There is no boolean anywhere in this type, no `ok`/`done`/`passed` field, and no
//! variant a caller could read as "the task succeeded". Nothing here is returned as
//! `Result<_, _>` either: [`VerificationTaskResult::unsupported_empty_task`] is
//! infallible and hands back the result itself, so there is no `Ok` to mistake for a
//! verdict. The honest summaries of a task that established nothing are the two lists
//! the deliverables already provide —
//! [`AssuranceEnvelope::unsupported_dimensions`] and [`EpochSet::unpinned_kinds`] — and
//! they are lists of what is missing, deliberately not flags.
//!
//! ```
//! use continuum_task::result::{TaskOutcome, VerificationTaskResult};
//! use continuum_value::assurance::{AssuranceDimension, InconclusiveReason};
//! use continuum_value::epoch::{EpochKind, EpochSet};
//!
//! let result = VerificationTaskResult::unsupported_empty_task(EpochSet::unpinned());
//!
//! // All six epochs are named; the ones nothing pinned read as a named absence.
//! assert_eq!(result.epochs().entries().len(), 6);
//! assert_eq!(result.epochs().unpinned_kinds(), EpochKind::ALL.to_vec());
//!
//! // The outcome is inconclusive *because* it is unsupported — the reason never
//! // becomes the verdict.
//! assert_eq!(
//!     result.outcome(),
//!     TaskOutcome::Inconclusive(InconclusiveReason::Unsupported)
//! );
//! assert_eq!(result.outcome().verdict_token(), "inconclusive");
//!
//! // What the task established is a list of what is missing, not a flag.
//! assert_eq!(
//!     result.assurance().unsupported_dimensions(),
//!     AssuranceDimension::ALL.to_vec()
//! );
//! ```
//!
//! # What this module is not
//!
//! - **Not the wire envelope.** RFC 0026's `ResultEnvelope` also carries `request_id`,
//!   `status`, `error`, `artifacts`, `task`/`continuation`, `omissions`, `warnings`,
//!   `cost` and `next_operations`. Those fields describe a *served request* and belong
//!   to `continuumd` (PR 5), together with the encodings, the negotiated protocol
//!   version, and the golden traces the `conformance.golden_traces` rule requires —
//!   one of which is "an unsupported empty task whose result still names every epoch".
//!   This type is the semantic core such an envelope wraps, so that the daemon, the
//!   evidence graph and any independent checker agree on it without a wire format.
//! - **Not the task service.** The lifecycle `Created → Running → Suspended |
//!   Completed | Failed | Cancelled`, cancellation, drain and finalize, budget
//!   accounting and continuations are this crate's PR 6 deliverable (plan §4.5). A
//!   result type is what that service will return; it is not that service.
//! - **Not storage or publication.** Two-phase publication and artifact handles are
//!   PR 2 and `continuumd`; this type references no artifact.

use continuum_evidence::claim_status::ClaimStatus;
use continuum_value::assurance::{
    AssuranceDimension, AssuranceEnvelope, DimensionEvidence, InconclusiveReason, UnsupportedReason,
};
use continuum_value::epoch::EpochSet;

/// The typed outcome of a verification task.
///
/// The three variants are the closed `verdict` enum of
/// `notes/plan/schemas/assurance-result.schema.json` (`established`, `refuted`,
/// `inconclusive`), with the schema's conditional requirement — "`inconclusive_reason`
/// is required when `verdict` is `inconclusive`" — expressed as a payload rather than
/// as a validation rule, so an unexplained inconclusive outcome is not representable.
///
/// A standalone `SemanticVerdict` enum is deliberately not minted here:
/// `continuum-value`'s assurance module notes that the `established | refuted |
/// inconclusive` verdict "is the lattice's shape, not this module's", and the wire
/// spelling belongs to `continuumd` (PR 5). [`Self::verdict_token`] reports the
/// schema's own spelling so machine output needs no second vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskOutcome {
    /// The claim holds within the envelope the result reports.
    ///
    /// "Verified" alone is prohibited in machine output (plan §3 B11): what this
    /// outcome is worth is read from the [`AssuranceEnvelope`] beside it, never from
    /// the word.
    Established,
    /// The claim is refuted by a valid counterexample or a proof of inconsistency.
    Refuted,
    /// The claim could not be decided, and here is the typed INV-008 reason.
    ///
    /// Budget exhaustion never lands here as a verdict: it is the `BudgetExhausted`
    /// error carrying a continuation (plan §11.4, RFC 0026), which is `continuumd`'s.
    Inconclusive(InconclusiveReason),
}

impl TaskOutcome {
    /// The stable machine token of this outcome's verdict, per
    /// `assurance-result.schema.json`.
    ///
    /// The reason of an inconclusive outcome is *not* part of the token: rendering
    /// `Unsupported` as though it were a verdict is exactly what INV-008 forbids. Read
    /// it from [`Self::inconclusive_reason`], which names it separately.
    #[must_use]
    pub const fn verdict_token(self) -> &'static str {
        match self {
            Self::Established => "established",
            Self::Refuted => "refuted",
            Self::Inconclusive(_) => "inconclusive",
        }
    }

    /// The typed INV-008 reason, or [`None`] when the outcome is a decided verdict.
    #[must_use]
    pub const fn inconclusive_reason(self) -> Option<InconclusiveReason> {
        match self {
            Self::Established | Self::Refuted => None,
            Self::Inconclusive(reason) => Some(reason),
        }
    }

    /// The evidence-graph claim status this outcome determines, when the verdict alone
    /// determines one.
    ///
    /// > | any → refuted | valid counterexample/checker |
    /// > | any → inconclusive | producing service, with a typed INV-008 reason |
    /// >
    /// > — `notes/plan/docs/44_MULTI_AGENT_EVIDENCE_GRAPH.md`, "Status authority"
    ///
    /// [`TaskOutcome::Established`] returns [`None`] on purpose. Which assurance status
    /// an established claim reaches — `observed`, `sampled`, `bounded`, `validated` or
    /// `proved` — is a property of the evidence that established it (RFC 0031's
    /// ladder), not of the word "established", and guessing it here would invent the
    /// promotion the lattice exists to police.
    ///
    /// This answers only *which* status the outcome names. Whether the caller may write
    /// it is authority — INV-004's no-self-certification, INV-015, and docs/44's
    /// right-hand column — and stays with the evidence graph and the daemon.
    #[must_use]
    pub const fn claim_status(self) -> Option<ClaimStatus> {
        match self {
            Self::Established => None,
            Self::Refuted => Some(ClaimStatus::Refuted),
            Self::Inconclusive(_) => Some(ClaimStatus::Inconclusive),
        }
    }
}

/// A verification task's machine result: six named epochs, nine named assurance
/// dimensions, and one typed outcome.
///
/// Every field is required and every constructor path fills all three, so a result
/// cannot report five epochs, eight dimensions, or an outcome without its reason. There
/// is no [`Default`]: a result nobody produced is not a result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationTaskResult {
    outcome: TaskOutcome,
    assurance: AssuranceEnvelope,
    epochs: EpochSet,
}

impl VerificationTaskResult {
    /// The typed reason every dimension of an empty task's envelope carries.
    ///
    /// An empty task declares nothing to verify, so no engine covered any dimension.
    /// The token is canonical (printable ASCII, no spaces) for the same reason
    /// [`UnsupportedReason`] requires it: one reason must have one spelling in machine
    /// output.
    pub const EMPTY_TASK: &'static str = "empty-task";

    /// Assemble a result from its three required parts.
    #[must_use]
    pub const fn new(outcome: TaskOutcome, assurance: AssuranceEnvelope, epochs: EpochSet) -> Self {
        Self {
            outcome,
            assurance,
            epochs,
        }
    }

    /// The result of an unsupported empty task — the PR-1 exit case.
    ///
    /// The caller supplies the epochs because they are the daemon's and the
    /// connection's to pin, not this type's to invent: pass [`EpochSet::unpinned`] when
    /// nothing is pinned, or a set carrying the negotiated
    /// [`ProtocolEpoch`](continuum_value::epoch::ProtocolEpoch) on a served connection.
    /// Either way all six epochs are named, because [`EpochSet`] has no other shape.
    ///
    /// The envelope reports every dimension as `Unsupported`. Eight carry
    /// [`Self::EMPTY_TASK`]; `memory_model` carries the reason the dossier fixes for it
    /// until the ADR-0032 weak-memory lane ships:
    ///
    /// > Until the weak-memory lane ships (ADR-0032), every envelope's memory dimension
    /// > reads `Unsupported(sequential-consistency-only)`.
    /// >
    /// > — `notes/plan/plan.md` §3 B11, restated in §24.5's frontier-lane register
    ///
    /// # Panics
    ///
    /// Never: [`Self::EMPTY_TASK`] is a canonical printable-ASCII token.
    #[must_use]
    pub fn unsupported_empty_task(epochs: EpochSet) -> Self {
        let reason = UnsupportedReason::new(Self::EMPTY_TASK)
            .expect("the empty-task reason is a canonical token");
        let assurance = AssuranceEnvelope::all_unsupported(&reason).with(
            AssuranceDimension::MemoryModel,
            DimensionEvidence::Unsupported(UnsupportedReason::sequential_consistency_only()),
        );
        Self::new(
            TaskOutcome::Inconclusive(InconclusiveReason::Unsupported),
            assurance,
            epochs,
        )
    }

    /// The typed outcome.
    #[must_use]
    pub const fn outcome(&self) -> TaskOutcome {
        self.outcome
    }

    /// The nine-dimension assurance envelope, required on every semantic verdict
    /// (plan §3 B11; RFC 0026 rule `envelope.assurance_required`).
    #[must_use]
    pub const fn assurance(&self) -> &AssuranceEnvelope {
        &self.assurance
    }

    /// The six epochs this result is pinned to. Each is named; the ones it cannot pin
    /// read [`EpochBinding::Unpinned`](continuum_value::epoch::EpochBinding::Unpinned).
    #[must_use]
    pub const fn epochs(&self) -> &EpochSet {
        &self.epochs
    }

    /// The typed INV-008 reason, when the outcome is inconclusive.
    ///
    /// This is the payload that discharges
    /// [`StatusObligations::typed_inconclusive_reason`](continuum_evidence::claim_status::StatusObligations::typed_inconclusive_reason)
    /// for the status [`TaskOutcome::claim_status`] names.
    #[must_use]
    pub const fn inconclusive_reason(&self) -> Option<InconclusiveReason> {
        self.outcome.inconclusive_reason()
    }
}
