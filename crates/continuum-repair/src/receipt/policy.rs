//! The receipt's policy decision (PR-22 / IMPL-09): `policy_decision` is derived from the
//! policy verdict ([`crate::policy`], PR-20 / IMPL-06) computed inside verification from
//! the named transaction version's recorded gates and the classification of the diff the
//! receipt recomputes for that version from the stores ([`super::semantic_diff`],
//! PR-22 / IMPL-03). Nothing is read from the claim, and no verdict or classification is
//! accepted from the caller: the caller supplies only the typed reasons of inconclusive
//! gates ([`GateReasons`]), which cannot change a verdict's kind.
//!
//! # What the RFCs make the field
//!
//! > | policy decision | `policy_decision` | the server-recomputed `allow` |
//! >
//! > — RFC 0032, "The promotion receipt"
//!
//! > 2. re-compute the semantic and intent classification and the `PolicyDecision`
//! >    server-side from current evidence; a cached or client-supplied verdict is never
//! >    trusted;
//! >
//! > — RFC 0032, "Promotion"
//!
//! > **A receipt MUST carry its policy decision, coverage, and unknowns.** […] all three
//! > are required by this RFC
//! >
//! > — RFC 0032, correction 11
//!
//! > There is no `unknown` decision.
//! >
//! > — RFC 0031, "Policy verdict"
//!
//! The schema types `policy_decision` as the closed enum `allow`, `review`, `block` and
//! makes it optional; its `gates` admit only `passed` and `not_yet_enforced`. So a
//! promotable receipt carries `allow` and nothing else. RFC 0032 "Totality" names the
//! wire refusal for the other outcomes, with nothing published: `PolicyGateFailed` for a
//! blocked transaction (a failed gate, or a verdict that is not `allow`), and
//! `InsufficientEvidence` for an inconclusive one, "because a gate that could not decide
//! is not a gate that failed". [`DecisionRefusal`] does not pick the code: the mapping
//! reads [`ReceiptDecision::standing`] of [`DecisionRefusal::recomputed`], never the bare
//! verdict kind (a record that reads eligible on an unclassified diff is
//! [`DecisionStanding::Undecided`]), and it is `repair.promote`'s (bn-23pwm).
//!
//! # Classification → verdict input
//!
//! | Recomputed diff ([`ReceiptDiff`]) | [`ReceiptClassification`] | Effect on the verdict |
//! |---|---|---|
//! | derived, a repair, program layer classified | `Bound(allow)` | none: `allow` overrides nothing |
//! | derived, a privileged intent revision | `Bound(review or block)`: the diff's decision joined with `review`, since a rebinding is a revision even when its verbs allow it (RFC 0037) | [`Verdict::PrivilegedIntentRevisionRequired`] |
//! | derived, a repair, program layer unclassified | `Absent(ProgramLayerUnclassified)` | the verdict is computed on the record alone, and never promotable |
//! | undiffable | `Absent(Undiffable(cause))` | as above. Inside verification this case never reaches the policy step: IMPL-03's check refuses the claim first, `SemanticDiffUndiffable`, because an undiffable diff has no handle to match. It is reachable through [`ReceiptDecision::of_skeleton`] |
//!
//! A store change that changes the classification changes the diff's handle too (the
//! handle covers the program layer and the evidence scope), so a claim built against
//! earlier stores is refused by IMPL-03's handle check before the policy step; the
//! policy step's own contribution is that the classification it reads is always the
//! one just recomputed, and that `Absent` never supports `allow`.
//!
//! # Verdict → field
//!
//! | Verdict | `policy_decision` | Promotable |
//! |---|---|---|
//! | [`Verdict::PromoteEligible`], classification bound | `allow` | yes |
//! | [`Verdict::PromoteEligible`], classification absent | absent: no `allow` rests on a diff nobody classified | no |
//! | [`Verdict::Blocked`] | `block`; the failed gates are named in the verdict (RFC 0032 rule 6) | no |
//! | [`Verdict::PrivilegedIntentRevisionRequired`] | `review` or `block`, the RFC 0031 join ([`Verdict::policy_decision`]) | no: the only way on is RFC 0037's revision procedure under `revise-intent` authority, which re-bases the repair as a new transaction (RFC 0032 "Intent integrity and reclassification") |
//! | [`Verdict::Inconclusive`] | absent (RFC 0031 has no `unknown` decision); each undecided gate is also an `unknowns` entry | no |
//!
//! The schema can express every row honestly: `allow`, `review` and `block` are its
//! enum, and an absent decision claims nothing. RFC 0032 correction 11 requires the
//! field on a receipt; a version that renders none has no receipt, because it is not
//! promotable.
//!
//! # Clause → mechanism
//!
//! | Clause | Source | Mechanism |
//! |---|---|---|
//! | the decision and the classification are recomputed server-side at promotion | RFC 0032 "Promotion" step 2; INV-015 | [`super::verify_for_promotion`] and [`super::verify_skeleton`] recompute the diff from the stores and call [`ReceiptDecision::derive`] on it; [`PolicyVerdict::compute`] runs inside. Neither takes a verdict, a decision or a classification |
//! | a cached or held verdict is never trusted | RFC 0032 "Promotion" step 2 | no public path takes a [`PolicyVerdict`] into a receipt: [`ReceiptDecision::derive`] is crate-private and [`ReceiptDecision::of_skeleton`] recomputes from a skeleton, which only [`super::ReceiptSkeleton::compose`] builds from the stores; a verdict is not a [`GateReasons`] (compile-fail doctest on [`ReceiptDecision`]) |
//! | an unclassified or undiffable diff never supports `allow` | RFC 0032 correction 8; RFC 0031 "Wire surface" | an unclassified one: [`ReceiptClassification::Absent`], so [`ReceiptDecision::field`] is absent, [`ReceiptDecision::check_claim`] refuses, and [`ReceiptDecision::standing`] is [`DecisionStanding::Undecided`]; an undiffable one: IMPL-03's `SemanticDiffUndiffable` first, and `Absent` as well |
//! | the receipt carries the decision | RFC 0032 correction 11 | the claim reader refuses a receipt without `policy_decision` ([`ClaimRefusal::Missing`]) |
//! | a claimed decision the recomputation does not carry is refused | RFC 0032 "Promotion" step 2; PR 22 exit | [`DecisionRefusal::Disagrees`], with the recomputation |
//! | only a classified `allow` is promotable | RFC 0032 receipt table | [`DecisionRefusal::NotPromotable`] for every other agreeing claim |
//! | a refusal says why | INV-008; RFC 0032 rules 6 and 7 | every refusal carries the recomputation: failed gates, each undecided gate with its typed INV-008 reason or the typed absence of one, and the classification or why it is absent |
//!
//! # Not here
//!
//! - A published receipt ([`super::verify_skeleton`]) is checked against the diff
//!   recomputed at verification time from the stores. The daemon's stores must answer
//!   as of the promotion (the [`super::ImpactScope`] obligation of
//!   [`super::ReceiptSeam::PromotionRecord`]); a recorded `policy_verdict` to anchor it
//!   is bn-2vanm's.
//! - `repair.promote` is not served (bn-23pwm), so no production path derives a
//!   receipt's decision yet; its lineage-head check and wire codes are its.

use core::fmt;
use std::collections::BTreeMap;

use continuum_intent::canonical_json::Json;
use continuum_intent::change_policy::PolicyDecision;
use continuum_value::identity::ContentHasher;

use crate::diff::IntentClassification as DiffClassification;
use crate::policy::{
    GateReasons, IntentClassification, PolicyVerdict, Standing, Undecided, Verdict, VerdictRefusal,
};
use crate::transaction::{GateName, RepairTransaction, Resolution};

use super::semantic_diff::{DiffGap, ReceiptDiff};
use super::{ClaimRefusal, ReceiptField, ReceiptSkeleton};

/// The RFC 0031 classification the verdict consults, read from the diff the receipt
/// recomputed for the version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiptClassification {
    /// The diff classifies the candidate: `allow` for a repair whose program-side layer
    /// is classified, `review` or `block` for a privileged intent revision.
    Bound(PolicyDecision),
    /// No classification: the diff is undiffable, or its program-side layer nobody
    /// classified. The verdict is never promotable.
    Absent(DiffGap),
}

impl ReceiptClassification {
    /// The classification of `diff`.
    #[must_use]
    pub fn of(diff: &ReceiptDiff) -> Self {
        match diff {
            ReceiptDiff::Undiffable(cause) => Self::Absent(DiffGap::Undiffable(*cause)),
            ReceiptDiff::Derived(computed) => match computed.classification() {
                DiffClassification::PrivilegedIntentRevision(revision) => {
                    Self::Bound(revision.decision().join(PolicyDecision::Review))
                }
                DiffClassification::Repair => match diff.gap() {
                    None => Self::Bound(PolicyDecision::Allow),
                    Some(gap) => Self::Absent(gap),
                },
            },
        }
    }
}

/// Where a recomputed decision leaves promotion: the typed input to the wire-code
/// mapping (RFC 0032 "Totality").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionStanding {
    /// A classified [`Verdict::PromoteEligible`]: promotion may proceed.
    Promotable,
    /// A failed gate or a protected change ([`Verdict::Blocked`],
    /// [`Verdict::PrivilegedIntentRevisionRequired`]): `PolicyGateFailed`.
    Blocked,
    /// An undecided gate ([`Verdict::Inconclusive`]), or a record that reads eligible on
    /// a diff with no classification: `InsufficientEvidence`, since a classification
    /// that could not be made is not a gate that failed.
    Undecided,
}

/// Why a receipt's policy decision was refused. No variant carries caller text: a claimed
/// decision is a closed token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionRefusal {
    /// No verdict could be computed for the version.
    Verdict(VerdictRefusal),
    /// The diff was computed for another version than the transaction presented.
    DiffOfAnotherVersion,
    /// The transaction has no candidate snapshot. Unreachable through verification,
    /// which refuses a draft first.
    NoCandidate,
    /// The claimed decision is not the recomputed field, including any decision claimed
    /// where the field is absent.
    Disagrees {
        /// The decision claimed.
        claimed: PolicyDecision,
        /// The recomputation, with its reasons.
        recomputed: Box<ReceiptDecision>,
    },
    /// The claim agrees with the recomputation, and it is not a classified
    /// [`Verdict::PromoteEligible`]: no receipt for this version is promotable.
    NotPromotable {
        /// The recomputation, with its reasons.
        recomputed: Box<ReceiptDecision>,
    },
}

impl DecisionRefusal {
    /// The recomputation the refusal carries, when one was made.
    #[must_use]
    pub fn recomputed(&self) -> Option<&ReceiptDecision> {
        match self {
            Self::Verdict(_) | Self::DiffOfAnotherVersion | Self::NoCandidate => None,
            Self::Disagrees { recomputed, .. } | Self::NotPromotable { recomputed } => {
                Some(recomputed)
            }
        }
    }
}

fn write_undecided(f: &mut fmt::Formatter<'_>, undecided: &[Undecided]) -> fmt::Result {
    for (index, entry) in undecided.iter().enumerate() {
        if index > 0 {
            f.write_str(", ")?;
        }
        write!(f, "{} ", entry.gate.token())?;
        match &entry.standing {
            Standing::Pending => f.write_str("pending")?,
            Standing::Inconclusive(Resolution::Found(reason)) => {
                write!(f, "inconclusive ({reason})")?;
            }
            Standing::Inconclusive(Resolution::Unknown) => {
                f.write_str("inconclusive (reason unknown)")?;
            }
            Standing::Inconclusive(Resolution::Unavailable) => {
                f.write_str("inconclusive (reason unavailable)")?;
            }
        }
    }
    Ok(())
}

fn write_failed(f: &mut fmt::Formatter<'_>, failed: &[GateName]) -> fmt::Result {
    for (index, gate) in failed.iter().enumerate() {
        if index > 0 {
            f.write_str(", ")?;
        }
        f.write_str(gate.token())?;
    }
    Ok(())
}

/// Renders the recomputation: the version, the verdict kind, every failed and undecided
/// gate, and the classification. Closed tokens and the stored `rt_` handle only.
impl fmt::Display for ReceiptDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: ", self.verdict.repair_id())?;
        match self.verdict.verdict() {
            Verdict::PromoteEligible if self.promotable() => f.write_str("promote-eligible")?,
            Verdict::PromoteEligible => {
                f.write_str("not eligible: every gate passed on the record, and the classification is absent")?;
            }
            Verdict::Blocked { failed, undecided } => {
                f.write_str("blocked; failed: ")?;
                write_failed(f, failed)?;
                f.write_str("; undecided: ")?;
                write_undecided(f, undecided)?;
            }
            Verdict::PrivilegedIntentRevisionRequired {
                classification,
                failed,
                undecided,
            } => {
                f.write_str("privileged intent revision required")?;
                if let Some(decision) = classification {
                    write!(f, " (classification {decision})")?;
                }
                f.write_str("; failed: ")?;
                write_failed(f, failed)?;
                f.write_str("; undecided: ")?;
                write_undecided(f, undecided)?;
            }
            Verdict::Inconclusive { undecided } => {
                f.write_str("inconclusive; undecided: ")?;
                write_undecided(f, undecided)?;
            }
        }
        match self.classification {
            ReceiptClassification::Bound(decision) => write!(f, "; classification {decision}"),
            ReceiptClassification::Absent(DiffGap::ProgramLayerUnclassified) => {
                f.write_str("; classification absent (program layer unclassified)")
            }
            ReceiptClassification::Absent(DiffGap::Undiffable(cause)) => {
                write!(f, "; classification absent (undiffable: {})", cause.token())
            }
        }
    }
}

impl fmt::Display for DecisionRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Verdict(refusal) => write!(f, "no policy verdict: {refusal}"),
            Self::DiffOfAnotherVersion => {
                f.write_str("the recomputed diff is of another transaction version")
            }
            Self::NoCandidate => f.write_str("the transaction has no candidate snapshot"),
            Self::Disagrees {
                claimed,
                recomputed,
            } => write!(
                f,
                "`policy_decision` claims {claimed}; the recomputed verdict is {recomputed}"
            ),
            Self::NotPromotable { recomputed } => write!(
                f,
                "the receipt is not promotable: the recomputed verdict is {recomputed}"
            ),
        }
    }
}

impl std::error::Error for DecisionRefusal {}

/// The receipt's policy decision: the verdict computed for one transaction version from
/// its recorded gates and the classification of the diff recomputed for it. No public
/// constructor takes a verdict, a decision or a classification.
///
/// A held verdict cannot be offered where only reasons are read:
///
/// ```compile_fail,E0277
/// fn reasons<R: continuum_repair::policy::GateReasons>() {}
/// reasons::<continuum_repair::policy::PolicyVerdict>();
/// ```
///
/// and a decision cannot be built from one:
///
/// ```compile_fail,E0451
/// use continuum_repair::receipt::{ReceiptClassification, ReceiptDecision};
/// fn forge(verdict: continuum_repair::policy::PolicyVerdict) -> ReceiptDecision {
///     ReceiptDecision {
///         verdict,
///         classification: ReceiptClassification::Bound(
///             continuum_intent::change_policy::PolicyDecision::Allow,
///         ),
///     }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiptDecision {
    verdict: PolicyVerdict,
    classification: ReceiptClassification,
}

impl ReceiptDecision {
    /// The decision of `transaction`, whose diff recomputed from the stores is `diff`.
    /// Crate-private: the receipt's verification calls it on the diff it just recomputed.
    pub(crate) fn derive<H: ContentHasher>(
        transaction: &RepairTransaction<H>,
        diff: &ReceiptDiff,
        reasons: &impl GateReasons,
    ) -> Result<Self, DecisionRefusal> {
        // The diff is bound to the version by its `rt_` handle, which is the content
        // identity of the version's record; the classification below is then built from
        // the version's own base, candidate and intent, which the diff was computed from.
        if diff
            .diff()
            .is_some_and(|computed| computed.repair() != transaction.repair_id())
        {
            return Err(DecisionRefusal::DiffOfAnotherVersion);
        }
        let classification = ReceiptClassification::of(diff);
        let bound = match classification {
            ReceiptClassification::Bound(decision) => Some(IntentClassification::new(
                transaction.base_snapshot().clone(),
                transaction
                    .candidate_snapshot()
                    .ok_or(DecisionRefusal::NoCandidate)?
                    .clone(),
                transaction.base_intent().clone(),
                decision,
            )),
            ReceiptClassification::Absent(_) => None,
        };
        let verdict = PolicyVerdict::compute(transaction, reasons, bound.as_ref())
            .map_err(DecisionRefusal::Verdict)?;
        Ok(Self {
            verdict,
            classification,
        })
    }

    /// The decision of the version `skeleton` was composed for, recomputed from its
    /// diff and `transaction`'s recorded gates. For rendering a composed receipt only: a
    /// held skeleton may be stale, so its output is never evidence that a receipt is
    /// promotable. Only the decision a [`super::ReceiptGenerationLicense`] carries,
    /// computed inside [`super::verify_for_promotion`] from the current stores, is.
    ///
    /// # Errors
    ///
    /// [`DecisionRefusal::DiffOfAnotherVersion`] when `transaction` is not the version
    /// the skeleton names; [`DecisionRefusal::Verdict`] when no verdict can be computed.
    pub fn of_skeleton<H: ContentHasher>(
        skeleton: &ReceiptSkeleton,
        transaction: &RepairTransaction<H>,
        reasons: &impl GateReasons,
    ) -> Result<Self, DecisionRefusal> {
        if skeleton.repair_transaction() != transaction.repair_id() {
            return Err(DecisionRefusal::DiffOfAnotherVersion);
        }
        Self::derive(transaction, skeleton.semantic_diff(), reasons)
    }

    /// The verdict, with its reasons.
    #[must_use]
    pub const fn verdict(&self) -> &PolicyVerdict {
        &self.verdict
    }

    /// The classification the verdict consulted, or why there is none.
    #[must_use]
    pub const fn classification(&self) -> ReceiptClassification {
        self.classification
    }

    /// Where this decision leaves promotion, for the wire-code mapping.
    #[must_use]
    pub const fn standing(&self) -> DecisionStanding {
        match self.verdict.verdict() {
            Verdict::PromoteEligible => match self.classification {
                ReceiptClassification::Bound(_) => DecisionStanding::Promotable,
                ReceiptClassification::Absent(_) => DecisionStanding::Undecided,
            },
            Verdict::Blocked { .. } | Verdict::PrivilegedIntentRevisionRequired { .. } => {
                DecisionStanding::Blocked
            }
            Verdict::Inconclusive { .. } => DecisionStanding::Undecided,
        }
    }

    fn promotable(&self) -> bool {
        matches!(self.verdict.verdict(), Verdict::PromoteEligible)
            && matches!(self.classification, ReceiptClassification::Bound(_))
    }

    /// The `policy_decision` value the receipt carries: `allow` exactly for a classified
    /// [`Verdict::PromoteEligible`], none for an unclassified one or for
    /// [`Verdict::Inconclusive`], `block` for [`Verdict::Blocked`], and the RFC 0031 join
    /// for [`Verdict::PrivilegedIntentRevisionRequired`].
    #[must_use]
    pub fn field(&self) -> Option<PolicyDecision> {
        match self.verdict.verdict() {
            Verdict::PromoteEligible if !self.promotable() => None,
            verdict => verdict.policy_decision(),
        }
    }

    /// Hold a claimed `policy_decision` against the recomputation, then require it to be
    /// promotable. A claim always carries a decision: the reader refuses a receipt
    /// without one (RFC 0032 correction 11).
    ///
    /// # Errors
    ///
    /// [`DecisionRefusal::Disagrees`] or [`DecisionRefusal::NotPromotable`], each with
    /// the recomputation.
    pub fn check_claim(&self, claimed: PolicyDecision) -> Result<(), DecisionRefusal> {
        if self.field() != Some(claimed) {
            return Err(DecisionRefusal::Disagrees {
                claimed,
                recomputed: Box::new(self.clone()),
            });
        }
        if self.promotable() {
            Ok(())
        } else {
            // Reached for `block` and `review` only: every other non-promotable case has
            // no field, and a claim always has one.
            Err(DecisionRefusal::NotPromotable {
                recomputed: Box::new(self.clone()),
            })
        }
    }

    /// IMPL-09's fragment: `repair_transaction` and, when the recomputation carries a
    /// decision, `policy_decision`.
    #[must_use]
    pub fn fields_json(&self) -> Json {
        let mut fields = BTreeMap::from([(
            ReceiptField::RepairTransaction.property().to_owned(),
            Json::String(self.verdict.repair_id().as_str().to_owned()),
        )]);
        if let Some(decision) = self.field() {
            fields.insert(
                ReceiptField::PolicyDecision.property().to_owned(),
                Json::String(decision.wire().to_owned()),
            );
        }
        Json::Object(fields)
    }
}

/// Read a claimed `policy_decision`: one of the schema's three tokens.
pub(super) fn claimed(value: &Json) -> Result<PolicyDecision, ClaimRefusal> {
    value
        .as_str()
        .and_then(PolicyDecision::from_wire)
        .ok_or(ClaimRefusal::Malformed(ReceiptField::PolicyDecision))
}

#[cfg(test)]
mod tests {
    //! The eligible path and a recorded gate 3, which only this crate's tests can reach: in
    //! production only an evaluation writes a gate outcome, and only gates 1 and 4 have
    //! one. `with_recorded_gates_evidenced` records a gate list directly, and
    //! `record_outcomes` is the crate-private recorder the evaluation path uses. A
    //! candidate identical to its base is the one whose program-side layer is classified.

    use super::*;
    use crate::diff::{self, DiffScope, ImpactScope, ScopeAnswer};
    use crate::handle::{CrashpackId, EvidenceRef, RepairId, SnapshotId};
    use crate::hypothesis::{ChangeKind, Hypothesis, Proposal};
    use crate::patch::{DeclaredChange, FileEdit, HashedIdentifier};
    use crate::policy::{NoReasons, ReplayRecords};
    use crate::receipt::{
        CandidateEvidence, CertificateRecord, ClaimedReceipt, IntentRegistry, IntentStanding,
        PackCase, ReceiptGenerationLicense, RegisteredIntent, SealedSnapshots, TransactionStore,
        UndiffableCause, UnenforcedGate, VerifyRefusal, verify_for_promotion,
    };
    use crate::replay::{self, RecordedRun, ReplayOutcome, ReplayRun, Replayer};
    use crate::transaction::{FailureBinding, GateProfile, GateStatus};
    use continuum_intent::contract::{IntentContract, IntentId};
    use continuum_semantic_diff::impact::{
        DependencyEdge, DependencyReason, EvidenceId, EvidenceRecord, Independence, ReuseEdgeClass,
    };
    use continuum_value::assurance::InconclusiveReason;
    use continuum_value::identity::Blake3Hasher;
    use continuum_workspace::snapshot::{Snapshot, WorkspaceContent, WorkspacePath};

    type Tx = RepairTransaction<Blake3Hasher>;

    const CONTRACT: &[u8] = include_bytes!(
        "../../../continuum-intent/tests/fixtures/replicated-register-contract.json"
    );
    const BEFORE: &[u8] = b"fn write() {\n    put();\n    ack();\n    sync();\n}\n";
    const AFTER: &[u8] = b"fn write() {\n    put();\n    sync();\n    ack();\n}\n";
    const REPLICA: &str = "src/replica.rs";
    const SCHEDULE: &[u8] = b"r0/i0/Reserve(0)#0\nr0/i0/Submit(0)#0\nr0/i0/Confirm(0)#0\n";
    const JOURNAL: &[u8] = b"journal: ack v0; lose v0; ack v1";
    const FAILURE: &[u8] = b"Agreement(41)";

    fn contract() -> IntentContract {
        IntentContract::decode(CONTRACT).unwrap()
    }

    fn content(replica: &[u8]) -> WorkspaceContent {
        let mut content = WorkspaceContent::new();
        content
            .insert(WorkspacePath::new(REPLICA).unwrap(), replica.to_vec())
            .unwrap();
        content
    }

    fn tree(content: &WorkspaceContent) -> Snapshot {
        Snapshot::build(content, &HashedIdentifier::<Blake3Hasher>::new()).unwrap()
    }

    fn id(content: &WorkspaceContent) -> SnapshotId {
        SnapshotId::new(&tree(content).identity().to_string()).unwrap()
    }

    fn crashpack() -> Vec<u8> {
        RecordedRun::new(
            id(&content(BEFORE)),
            SCHEDULE.to_vec(),
            JOURNAL.to_vec(),
            FAILURE.to_vec(),
        )
        .unwrap()
        .canonical_bytes()
    }

    fn failure() -> CrashpackId {
        CrashpackId::new(&format!(
            "crash_{}",
            <Blake3Hasher as ContentHasher>::hash(&crashpack()).to_token()
        ))
        .unwrap()
    }

    fn scope_record(id: &str) -> EvidenceRecord {
        EvidenceRecord {
            id: EvidenceId::new(id).unwrap(),
            keyed_to_before_intent: true,
            key_change_reuse_witness: false,
            edges: vec![DependencyEdge {
                reason: DependencyReason::ReliesOnAssumptionFairnessBound,
                class: ReuseEdgeClass::Exact,
                independence: Independence::Unknown,
            }],
        }
    }

    /// The daemon's stores: the register contract, every snapshot bound to it, the
    /// transactions, no certificate and no pack case, and the evidence scope `.1`.
    struct World(Vec<Tx>, Vec<EvidenceRecord>);

    impl World {
        fn of(tx: &Tx) -> Self {
            Self(vec![tx.clone()], Vec::new())
        }
    }

    impl FailureBinding for World {
        fn failure_base(&self, _: &CrashpackId) -> Resolution<SnapshotId> {
            Resolution::Found(id(&content(BEFORE)))
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
        fn sealed_binding(&self, _: &SnapshotId) -> Resolution<IntentId> {
            Resolution::Found(contract().intent_id().clone())
        }
    }

    impl TransactionStore<Blake3Hasher> for World {
        fn transaction(&self, repair: &RepairId) -> Resolution<Tx> {
            self.0
                .iter()
                .find(|tx| tx.repair_id() == repair)
                .cloned()
                .map_or(Resolution::Unknown, Resolution::Found)
        }
    }

    impl CandidateEvidence for World {
        fn certificates(&self, _: &RepairId, _: &SnapshotId) -> Resolution<Vec<CertificateRecord>> {
            Resolution::Found(Vec::new())
        }
        fn unsupported_pack_cases(
            &self,
            _: &RepairId,
            _: &SnapshotId,
        ) -> Resolution<Vec<PackCase>> {
            Resolution::Found(Vec::new())
        }
    }

    impl ImpactScope for World {
        fn evidence(&self, _: DiffScope<'_>) -> ScopeAnswer {
            ScopeAnswer::Complete(self.1.clone())
        }
    }

    /// Replace the replica with `after` under `profile`: the ack-after-sync repair for
    /// [`AFTER`], and a candidate identical to its base for [`BEFORE`].
    fn applied_to(profile: GateProfile, after: &[u8]) -> Tx {
        let leaf = tree(&content(BEFORE))
            .node(&WorkspacePath::new(REPLICA).unwrap())
            .and_then(|node| node.as_file())
            .unwrap()
            .identity()
            .to_string();
        let change = DeclaredChange::new(
            ChangeKind::Rust,
            [(
                WorkspacePath::new(REPLICA).unwrap(),
                FileEdit::Replace {
                    before: SnapshotId::new(&leaf).unwrap(),
                    content: after.to_vec(),
                },
            )],
        )
        .unwrap();
        Tx::begin(failure(), profile, &World(Vec::new(), Vec::new()))
            .unwrap()
            .apply(
                &Proposal::new(Hypothesis::new("unit"), vec![change]),
                &content(BEFORE),
            )
            .unwrap()
            .into_parts()
            .0
    }

    fn ready_status(profile: GateProfile, gate: GateName) -> GateStatus {
        if !profile.enforces(gate) {
            GateStatus::NotYetEnforced
        } else if gate == GateName::ReceiptGeneration {
            GateStatus::Pending
        } else {
            GateStatus::Passed
        }
    }

    /// A `ready` record (gates 1–11 passed on evidence, gate 12 pending) with `gate` at
    /// `status`, on the candidate `after`.
    fn ready_but(profile: GateProfile, after: &[u8], gate: Option<(GateName, GateStatus)>) -> Tx {
        applied_to(profile, after).with_recorded_gates_evidenced(GateName::ALL.map(
            |each| match gate {
                Some((named, status)) if named == each => status,
                _ => ready_status(profile, each),
            },
        ))
    }

    fn ready(profile: GateProfile) -> Tx {
        ready_but(profile, BEFORE, None)
    }

    fn skeleton(tx: &Tx, world: &World) -> ReceiptSkeleton {
        ReceiptSkeleton::compose(tx, world, world, world, world).unwrap()
    }

    /// The honest-shaped claim for `tx` under `world`: its derived fields, every
    /// in-profile gate `passed`, and `decision`.
    fn claim(tx: &Tx, world: &World, decision: PolicyDecision) -> ClaimedReceipt {
        let Json::Object(mut fields) = skeleton(tx, world).fields_json() else {
            unreachable!()
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
        fields.insert(
            "policy_decision".to_owned(),
            Json::String(decision.wire().to_owned()),
        );
        ClaimedReceipt::parse(&Json::Object(fields).to_canonical_bytes()).unwrap()
    }

    fn promote_in(
        tx: &Tx,
        world: &World,
        decision: PolicyDecision,
        reasons: &impl GateReasons,
    ) -> Result<ReceiptGenerationLicense, VerifyRefusal> {
        verify_for_promotion(
            &claim(tx, world, decision),
            world,
            world,
            world,
            world,
            world,
            reasons,
        )
    }

    fn promote(
        tx: &Tx,
        decision: PolicyDecision,
    ) -> Result<ReceiptGenerationLicense, VerifyRefusal> {
        promote_in(tx, &World::of(tx), decision, &NoReasons)
    }

    fn decide(tx: &Tx) -> ReceiptDecision {
        let world = World::of(tx);
        ReceiptDecision::of_skeleton(&skeleton(tx, &world), tx, &NoReasons).unwrap()
    }

    /// `pr22-impl09-pos-01`: a `ready` record whose candidate is classified is
    /// promote-eligible under every profile, its receipt carries `allow`, and promotion
    /// verification passes the policy step and every record check, stopping only at
    /// gate 8's absent evidence (IMPL-05). The licence would carry this decision.
    #[test]
    fn pr22_impl09_positive_an_eligible_classified_record_is_allow_and_passes_the_policy_step() {
        for profile in GateProfile::ALL {
            let tx = ready(profile);
            let decision = decide(&tx);
            assert_eq!(decision.verdict().verdict(), &Verdict::PromoteEligible);
            assert_eq!(
                decision.classification(),
                ReceiptClassification::Bound(PolicyDecision::Allow)
            );
            assert_eq!(decision.field(), Some(PolicyDecision::Allow));
            decision.check_claim(PolicyDecision::Allow).unwrap();
            assert_eq!(
                decision.fields_json(),
                Json::Object(BTreeMap::from([
                    (
                        "policy_decision".to_owned(),
                        Json::String("allow".to_owned())
                    ),
                    (
                        "repair_transaction".to_owned(),
                        Json::String(tx.repair_id().as_str().to_owned())
                    ),
                ]))
            );
            assert_eq!(
                promote(&tx, PolicyDecision::Allow),
                Err(VerifyRefusal::GateWithoutEvidence(
                    GateName::RefinementCoverage
                )),
                "{}",
                profile.token()
            );
        }
    }

    /// `pr22-impl09-bnd-01`: every verdict kind under every profile, on the classified
    /// candidate and on the unclassified ack-after-sync candidate, maps to its field, and
    /// only a classified `allow` gets past the policy step. Every other pairing of
    /// recomputation and claimed decision is refused, typed: `Disagrees` when the claim is
    /// not the field, `NotPromotable` when it is.
    #[test]
    fn pr22_impl09_boundary_every_verdict_kind_maps_to_its_field_and_only_allow_promotes() {
        let neighborhood = GateName::Neighborhood;
        for profile in GateProfile::ALL {
            let cases: Vec<(&str, Tx, Option<PolicyDecision>)> = vec![
                ("eligible", ready(profile), Some(PolicyDecision::Allow)),
                (
                    "eligible, unclassified",
                    ready_but(profile, AFTER, None),
                    None,
                ),
                (
                    "blocked",
                    ready_but(profile, BEFORE, Some((neighborhood, GateStatus::Failed))),
                    Some(PolicyDecision::Block),
                ),
                (
                    "blocked, unclassified",
                    ready_but(profile, AFTER, Some((neighborhood, GateStatus::Failed))),
                    Some(PolicyDecision::Block),
                ),
                (
                    "gate 3 failed",
                    ready_but(
                        profile,
                        BEFORE,
                        Some((GateName::IntentIntegrity, GateStatus::Failed)),
                    ),
                    Some(PolicyDecision::Block),
                ),
                (
                    "pending",
                    ready_but(profile, BEFORE, Some((neighborhood, GateStatus::Pending))),
                    None,
                ),
                (
                    "inconclusive",
                    ready_but(
                        profile,
                        BEFORE,
                        Some((neighborhood, GateStatus::Inconclusive)),
                    ),
                    None,
                ),
            ];
            for (case, tx, field) in cases {
                let decision = decide(&tx);
                assert_eq!(decision.field(), field, "{case}");
                let kind = match decision.verdict().verdict() {
                    Verdict::PromoteEligible => "eligible",
                    Verdict::Blocked { .. } => "blocked",
                    Verdict::PrivilegedIntentRevisionRequired { .. } => "gate 3 failed",
                    Verdict::Inconclusive { .. } => "undecided",
                };
                assert!(
                    case.starts_with(kind) || (kind == "undecided" && field.is_none()),
                    "{case}: {kind}"
                );
                if field.is_none() {
                    let Json::Object(fields) = decision.fields_json() else {
                        unreachable!()
                    };
                    assert!(!fields.contains_key("policy_decision"), "{case}");
                }
                for claimed in PolicyDecision::ALL {
                    let checked = decision.check_claim(claimed);
                    let result = promote(&tx, claimed);
                    if case == "eligible" && claimed == PolicyDecision::Allow {
                        checked.unwrap();
                        assert_eq!(
                            result,
                            Err(VerifyRefusal::GateWithoutEvidence(
                                GateName::RefinementCoverage
                            ))
                        );
                        continue;
                    }
                    let refusal = checked.unwrap_err();
                    if Some(claimed) == field {
                        assert!(
                            matches!(refusal, DecisionRefusal::NotPromotable { .. }),
                            "{case} {claimed}"
                        );
                    } else {
                        assert!(
                            matches!(refusal, DecisionRefusal::Disagrees { claimed: c, .. } if c == claimed),
                            "{case} {claimed}"
                        );
                    }
                    assert_eq!(refusal.recomputed(), Some(&decision));
                    assert_eq!(
                        result,
                        Err(VerifyRefusal::PolicyDecision(refusal)),
                        "{case}"
                    );
                }
            }
        }
    }

    /// `pr22-impl09-neg-01`: a verdict held from an earlier recomputation does not carry
    /// over when the stores change. The version is eligible and classified under one
    /// evidence scope; when the store's scope changes so the diff is no longer
    /// classifiable (it names one artifact twice), the classification recomputed at
    /// promotion is absent, the field is absent, and a claimed `allow` is refused; the
    /// earlier claim itself is refused by IMPL-03's handle check. No public path takes
    /// the earlier verdict (the compile-fail doctests on `ReceiptDecision` pin two
    /// routes; the API shape closes the rest).
    #[test]
    fn pr22_impl09_negative_a_changed_store_classification_refuses_an_earlier_eligible_version() {
        let tx = ready(GateProfile::PhaseB);
        let before = World::of(&tx);
        let earlier =
            ReceiptDecision::of_skeleton(&skeleton(&tx, &before), &tx, &NoReasons).unwrap();
        assert_eq!(earlier.field(), Some(PolicyDecision::Allow));
        let held = PolicyVerdict::compute(
            &tx,
            &NoReasons,
            Some(&IntentClassification::new(
                tx.base_snapshot().clone(),
                tx.candidate_snapshot().unwrap().clone(),
                tx.base_intent().clone(),
                PolicyDecision::Allow,
            )),
        )
        .unwrap();
        assert_eq!(held.verdict(), &Verdict::PromoteEligible);

        let after = World(
            vec![tx.clone()],
            vec![scope_record("ev_twice"), scope_record("ev_twice")],
        );
        let now = ReceiptDecision::of_skeleton(&skeleton(&tx, &after), &tx, &NoReasons).unwrap();
        assert_eq!(
            now.classification(),
            ReceiptClassification::Absent(DiffGap::Undiffable(UndiffableCause::ImpactScope))
        );
        // The record alone still reads eligible: only the classification moved.
        assert_eq!(now.verdict().verdict(), &Verdict::PromoteEligible);
        assert_eq!(now.field(), None);
        assert!(matches!(
            now.check_claim(PolicyDecision::Allow),
            Err(DecisionRefusal::Disagrees {
                claimed: PolicyDecision::Allow,
                ..
            })
        ));
        assert!(
            now.check_claim(PolicyDecision::Allow)
                .unwrap_err()
                .to_string()
                .contains("classification absent (undiffable: impact_scope_malformed)")
        );
        assert_eq!(now.standing(), DecisionStanding::Undecided);
        // Promotion under the changed store: the claim built against the earlier stores
        // names the earlier diff, and the diff recomputed now is undiffable, so IMPL-03's
        // check refuses it before the policy step; the policy step's contribution is the
        // recomputed classification above, which never supports `allow`.
        let earlier_claim = claim(&tx, &before, PolicyDecision::Allow);
        assert_eq!(
            verify_for_promotion(
                &earlier_claim,
                &after,
                &after,
                &after,
                &after,
                &after,
                &NoReasons
            )
            .map(|_| ()),
            Err(VerifyRefusal::SemanticDiffUndiffable(
                UndiffableCause::ImpactScope
            ))
        );
    }

    /// `pr22-impl09-neg-02`: a record the verdict cannot read composes no decision (a
    /// gate passed on no evidence), and a skeleton of another version is refused.
    #[test]
    fn pr22_impl09_negative_no_verdict_no_decision() {
        let bare = applied_to(GateProfile::PhaseB, BEFORE)
            .with_recorded_gates(GateName::ALL.map(|gate| ready_status(GateProfile::PhaseB, gate)));
        let world = World::of(&bare);
        assert!(matches!(
            ReceiptDecision::of_skeleton(&skeleton(&bare, &world), &bare, &NoReasons),
            Err(DecisionRefusal::Verdict(
                VerdictRefusal::PassedWithoutEvidence { .. }
            ))
        ));
        let tx = ready(GateProfile::PhaseB);
        let other = ready(GateProfile::PhaseC);
        assert_eq!(
            ReceiptDecision::of_skeleton(&skeleton(&other, &World::of(&other)), &tx, &NoReasons),
            Err(DecisionRefusal::DiffOfAnotherVersion)
        );
    }

    // --- M01 with gate 3 recorded ------------------------------------------------------

    struct Scripted;

    impl Replayer for Scripted {
        fn identity(&self) -> &str {
            "scripted/1"
        }
        fn replay(&self, program: &WorkspaceContent, _: &[u8]) -> ReplayOutcome {
            if id(program) == id(&content(BEFORE)) {
                ReplayOutcome::Ran(ReplayRun::new(
                    JOURNAL.to_vec(),
                    Some(FAILURE.to_vec()),
                    Vec::new(),
                    0,
                ))
            } else {
                // The candidate runs clean but drops recorded steps 2 and 5 and reorders
                // one: not the recording, so `Exact` cannot pass it.
                ReplayOutcome::Ran(ReplayRun::new(
                    b"journal: ack v1".to_vec(),
                    None,
                    vec![2, 5],
                    1,
                ))
            }
        }
    }

    /// Gate 4's reason from its exact-replay records; gate 3's from the diff it cites,
    /// accepted only under the diff's own handle.
    struct Reasons<'a> {
        replay: ReplayRecords<'a, Blake3Hasher>,
        diff: &'a diff::DiffOutcome,
    }

    impl GateReasons for Reasons<'_> {
        fn inconclusive_reason(
            &self,
            gate: GateName,
            evidence: &[EvidenceRef],
        ) -> Resolution<InconclusiveReason> {
            match gate {
                GateName::IntentIntegrity => {
                    let diff::DiffOutcome::Computed(computed) = self.diff else {
                        return Resolution::Unknown;
                    };
                    match (evidence, self.diff.inconclusive_reason()) {
                        ([cited], Some(reason)) if cited.as_str() == computed.handle() => {
                            Resolution::Found(reason)
                        }
                        _ => Resolution::Unknown,
                    }
                }
                _ => self.replay.inconclusive_reason(gate, evidence),
            }
        }
    }

    /// `pr22-impl09-neg-03`: the M01 ack-after-sync transaction, with gate 4 evaluated by
    /// exact replay (inconclusive under `Exact`, `AbstractionAmbiguity`) and gate 3
    /// recorded from its computed diff (inconclusive, `Unsupported`, because the
    /// program-side layer is unclassified), as the evaluation path will record it. Its
    /// receipt renders no decision, promotion refuses every claimed decision, and the
    /// refusal names both gates with their reasons, every pending gate, and the absent
    /// classification.
    #[test]
    fn pr22_impl09_negative_m01_with_gate_3_recorded_is_not_promotable_and_says_why() {
        let world = World(Vec::new(), Vec::new());
        let (evaluated, records) = replay::evaluate(
            &applied_to(GateProfile::PhaseB, AFTER),
            &crashpack(),
            &content(BEFORE),
            &content(AFTER),
            &Scripted,
            None,
        )
        .unwrap()
        .into_parts();
        let outcome = diff::compute(&evaluated, &world, &world, &world);
        assert_eq!(outcome.gate_status(), GateStatus::Inconclusive);
        let diff::DiffOutcome::Computed(computed) = &outcome else {
            panic!("the ack-after-sync diff computes");
        };
        let m01 = evaluated
            .record_outcomes(
                vec![(
                    GateName::IntentIntegrity,
                    outcome.gate_status(),
                    vec![EvidenceRef::new(computed.handle()).unwrap()],
                )],
                evaluated.evaluation_policy().unwrap(),
            )
            .unwrap();
        let reasons = Reasons {
            replay: ReplayRecords::new(&records),
            diff: &outcome,
        };
        let world = World::of(&m01);
        let composed = skeleton(&m01, &world);
        let decision = ReceiptDecision::of_skeleton(&composed, &m01, &reasons).unwrap();
        assert_eq!(
            decision.classification(),
            ReceiptClassification::Absent(DiffGap::ProgramLayerUnclassified)
        );
        let expected: Vec<Undecided> = GateName::ALL
            .into_iter()
            .filter(|gate| {
                GateProfile::PhaseB.enforces(*gate)
                    && !matches!(gate, GateName::BaseReplay | GateName::ReceiptGeneration)
            })
            .map(|gate| Undecided {
                gate,
                standing: match gate {
                    GateName::IntentIntegrity => {
                        Standing::Inconclusive(Resolution::Found(InconclusiveReason::Unsupported))
                    }
                    GateName::ExactRegression => Standing::Inconclusive(Resolution::Found(
                        InconclusiveReason::AbstractionAmbiguity,
                    )),
                    _ => Standing::Pending,
                },
            })
            .collect();
        assert_eq!(
            decision.verdict().verdict(),
            &Verdict::Inconclusive {
                undecided: expected
            }
        );
        assert_eq!(decision.field(), None);
        assert_eq!(decision.standing(), DecisionStanding::Undecided);
        assert_eq!(
            decision.fields_json(),
            Json::Object(BTreeMap::from([(
                "repair_transaction".to_owned(),
                Json::String(m01.repair_id().as_str().to_owned())
            )]))
        );
        for claimed in PolicyDecision::ALL {
            let Err(VerifyRefusal::PolicyDecision(refusal)) =
                promote_in(&m01, &world, claimed, &reasons)
            else {
                panic!("M01 is never promotable");
            };
            assert!(
                matches!(refusal, DecisionRefusal::Disagrees { claimed: c, .. } if c == claimed)
            );
            let text = refusal.to_string();
            for why in [
                "intent_integrity inconclusive (",
                "exact_regression inconclusive (",
                "patch_application pending",
                "classification absent (program layer unclassified)",
            ] {
                assert!(text.contains(why), "{why} in {text}");
            }
        }
        // The same undecided gates are the receipt's gate unknowns.
        let tokens: Vec<String> = match composed.unknowns().entries_json() {
            Json::Array(items) => items
                .into_iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect(),
            _ => unreachable!(),
        };
        let Verdict::Inconclusive { undecided } = decision.verdict().verdict() else {
            unreachable!()
        };
        for entry in undecided {
            assert_eq!(
                tokens
                    .iter()
                    .filter(|token| token.ends_with(&format!(":{}", entry.gate.token())))
                    .count(),
                1,
                "{:?} is one unknowns entry in {tokens:?}",
                entry.gate
            );
        }
    }
}
