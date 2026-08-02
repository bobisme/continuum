//! Committed partial evidence: what an exhausted, cancelled or suspended task durably
//! published, bound to what it spent getting there (PR-6 / IMPL-02).
//!
//! > On cancellation:
//! >
//! > - child workers receive cancellation;
//! > - provisional streams close;
//! > - **committed partial artifacts are finalized**;
//! > - continuation is emitted if supported;
//! > - task transitions exactly once to terminal/suspended state;
//! > - obligations and resource leases are resolved.
//! >
//! > — `notes/plan/docs/35_CONTINUUMD_WORKBENCH_DAEMON.md`, "Cancellation"
//!
//! # The two halves, and why neither is enough
//!
//! The region layer counts commitments: an
//! [`EvidenceLedger`](crate::region::worker::EvidenceLedger) holds a monotone `committed`
//! count and a provisional flag, and a
//! [`Continuation`](crate::region::worker::Continuation) carries "*which worker* is
//! resumable and *how much* committed evidence it is resumable from". It has no idea what
//! any of it cost.
//!
//! [`BudgetLedger`](super::BudgetLedger) measures cost across the nine SD-12 dimensions.
//! It has no idea what was published.
//!
//! A partial result has to say both, and say them about the same moment. That moment is a
//! [`Checkpoint`] — a commitment count paired with the [`Spend`] recorded when it
//! happened — and [`CommittedPartialEvidence`] is what a caller reads off the pair:
//!
//! - **what it committed** — [`CommittedPartialEvidence::committed`], the region layer's
//!   count;
//! - **what that cost** — [`CommittedPartialEvidence::spend_at_commit`], the spend at the
//!   last checkpoint;
//! - **what it spent in total** — [`CommittedPartialEvidence::total_spend`], the task's
//!   `Cost`;
//! - **what bought nothing durable** — [`CommittedPartialEvidence::uncommitted_spend`], the
//!   difference. This is the number an honest partial result owes and neither layer can
//!   produce alone: work done after the last commit, ended by a cancellation or a ceiling,
//!   which is real spend against no artifact.
//!
//! Attributing the whole `Cost` to the committed artifact would overstate what the
//! artifact is worth; attributing only the checkpoint's spend to the task would understate
//! what the campaign cost. Both numbers are reported, separately, for the same reason
//! `Cost` and the assurance envelope are separate fields: they answer different questions.
//!
//! # The obligation cross-check
//!
//! > While it is open, the artifact is neither committed nor absent — the state G0-DX-14's
//! > second conjunct forbids at rest.
//! >
//! > — [`ObligationKind::ProvisionalPublication`](crate::region::obligation::ObligationKind::ProvisionalPublication)
//!
//! The region layer already discharges that obligation on commit or on discard, and
//! [`Finalization::unresolved_publications`](crate::region::Finalization::unresolved_publications)
//! reports the set it left behind — always empty. This module does **not** open a second
//! ledger over the same obligations: two accountings of one debt are two answers to "what
//! is outstanding", and the obligation module says why that is worse than one.
//!
//! What it does instead is add a *third independently computed* accounting and require it
//! to agree. [`EvidenceBook::reconcile`] walks a finalized subtree and checks, per worker:
//!
//! - the region layer's committed count equals the number of checkpoints the budget ledger
//!   took — every commitment was priced, and nothing was priced that was not committed;
//! - no budget ledger still holds a reservation — the budget-side half of "neither
//!   committed nor absent", read off headroom rather than off the region tree;
//! - no worker that committed evidence is missing an account, and no account names a worker
//!   the teardown did not report.
//!
//! That is the same discipline
//! [`Finalization::is_total`](crate::region::Finalization::is_total) uses — "a conjunction
//! of three *independently computed* facts rather than one flag" — extended by one
//! independent path, so a bug that fooled the region tree's own two accountings still has
//! this one to get past.

use core::fmt;
use std::collections::BTreeMap;

use super::dimension::{CostDimension, DimensionOmission, MeterSet};
use super::{Budget, BudgetLedger, Exhaustion, Spend, Suspension};
use crate::region::Finalization;
use crate::region::worker::{FailureReason, WorkerId, WorkerState};

/// A commitment count paired with the spend recorded when it happened.
///
/// Taken by [`BudgetLedger::checkpoint`](super::BudgetLedger::checkpoint), and the only
/// place the two vocabularies meet. Sequence numbers are dense ordinals in checkpoint
/// order, like [`RegionId`](crate::region::RegionId) and [`WorkerId`]: counted, never drawn
/// or timed, so one run of one program names the same checkpoints on every platform
/// (INV-005).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Checkpoint {
    sequence: u32,
    committed: u32,
    spend: Spend,
}

impl Checkpoint {
    /// The `sequence`-th checkpoint, at `committed` publications and `spend`.
    #[must_use]
    pub const fn new(sequence: u32, committed: u32, spend: Spend) -> Self {
        Self {
            sequence,
            committed,
            spend,
        }
    }

    /// This checkpoint's position in checkpoint order.
    #[must_use]
    pub const fn sequence(&self) -> u32 {
        self.sequence
    }

    /// How many publications the region layer had committed.
    #[must_use]
    pub const fn committed(&self) -> u32 {
        self.committed
    }

    /// What had been spent by then.
    #[must_use]
    pub const fn spend(&self) -> &Spend {
        &self.spend
    }

    /// A canonical one-line rendering.
    #[must_use]
    pub fn render(&self) -> String {
        format!(
            "checkpoint {} committed={} {}",
            self.sequence, self.committed, self.spend
        )
    }
}

impl fmt::Display for Checkpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render())
    }
}

/// Why a task stopped short of finishing.
///
/// Four arms, each one a case the dossier names separately, and none of them a verdict:
/// budget exhaustion, cancellation, preemption-by-budget-lowering and an ordinary failure
/// are four different next moves for a caller, and INV-008's rule that "timeout,
/// unsupported semantics, insufficient telemetry, abstraction ambiguity, and incomplete
/// proof search are distinct outcomes" is the same rule one layer up.
///
/// There is no `Completed` arm on purpose: a task that finished has evidence, not
/// *partial* evidence, and minting a partial-evidence report for it would invite a reader
/// to treat a whole answer as a truncated one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminationCause {
    /// A declared ceiling ran out. This is RFC 0026's `BudgetExhausted`, and the
    /// [`Exhaustion`] names which of the nine dimensions.
    Exhausted(Exhaustion),
    /// A cancellation reached the worker. The region layer's
    /// [`CancelOutcome`](crate::region::worker::CancelOutcome) says whether a continuation
    /// came with it.
    Cancelled,
    /// `task.update_budget` lowered a ceiling below committed spend, so the task parked
    /// (B18).
    Suspended(Suspension),
    /// The work failed for a reason that is not about budget, naming it.
    Failed(FailureReason),
}

impl TerminationCause {
    /// A stable token for canonical rendering.
    #[must_use]
    pub const fn token(&self) -> &'static str {
        match self {
            Self::Exhausted(_) => "exhausted",
            Self::Cancelled => "cancelled",
            Self::Suspended(_) => "suspended",
            Self::Failed(_) => "failed",
        }
    }

    /// The exhaustion, when a ceiling is what stopped the task.
    #[must_use]
    pub const fn exhaustion(&self) -> Option<&Exhaustion> {
        match self {
            Self::Exhausted(exhaustion) => Some(exhaustion),
            _ => None,
        }
    }

    /// A canonical one-line rendering.
    #[must_use]
    pub fn render(&self) -> String {
        match self {
            Self::Exhausted(exhaustion) => format!("exhausted({exhaustion})"),
            Self::Cancelled => "cancelled".to_owned(),
            Self::Suspended(suspension) => format!("suspended({suspension})"),
            Self::Failed(reason) => format!("failed({reason})"),
        }
    }
}

impl fmt::Display for TerminationCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render())
    }
}

/// What a task that did not finish durably published, and what it spent.
///
/// The shape docs/35 calls *committed partial evidence*, with both halves named and
/// neither inferred. Every field is required and the constructor fills all of them from a
/// [`BudgetLedger`](super::BudgetLedger), so a report cannot claim a commitment count
/// without the spend that produced it, or a cost without saying what it bought.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommittedPartialEvidence {
    worker: WorkerId,
    cause: TerminationCause,
    at: Option<Checkpoint>,
    total: Spend,
    omissions: Vec<DimensionOmission>,
}

impl CommittedPartialEvidence {
    /// Bind `worker`'s budget accounting to the reason it stopped.
    ///
    /// The checkpoint taken is the *last* one, because that is the frontier a continuation
    /// resumes from — RFC 0030's budget rule makes "the committed frontier … part of output
    /// identity" for a budget-truncated search, and an earlier checkpoint would name a
    /// frontier the artifact has already moved past.
    #[must_use]
    pub fn bind(worker: WorkerId, cause: TerminationCause, ledger: &BudgetLedger) -> Self {
        Self {
            worker,
            cause,
            at: ledger.last_checkpoint().copied(),
            total: *ledger.spend(),
            omissions: ledger.omissions(),
        }
    }

    /// Which worker this is about.
    #[must_use]
    pub const fn worker(&self) -> WorkerId {
        self.worker
    }

    /// Why it stopped.
    #[must_use]
    pub const fn cause(&self) -> &TerminationCause {
        &self.cause
    }

    /// How many publications are durable.
    ///
    /// Zero is a named outcome, not a missing one: it is the same fact
    /// [`CancelOutcome::NothingPublished`](crate::region::worker::CancelOutcome::NothingPublished)
    /// carries, and [`Self::published_anything`] is how a caller asks.
    #[must_use]
    pub fn committed(&self) -> u32 {
        self.at.as_ref().map_or(0, Checkpoint::committed)
    }

    /// Whether anything a reader can observe was published.
    #[must_use]
    pub fn published_anything(&self) -> bool {
        self.committed() > 0
    }

    /// The checkpoint the durable evidence stands at, when there is any.
    #[must_use]
    pub const fn checkpoint(&self) -> Option<&Checkpoint> {
        self.at.as_ref()
    }

    /// What had been spent when the last publication committed.
    ///
    /// What the durable artifact is worth. A task that committed nothing reports
    /// [`Spend::unmeasured`], which is honest in the exact sense RFC 0026 asks for:
    /// nothing was measured *at a commit*, because there was no commit.
    #[must_use]
    pub fn spend_at_commit(&self) -> Spend {
        self.at
            .as_ref()
            .map_or_else(Spend::unmeasured, |checkpoint| *checkpoint.spend())
    }

    /// What the task spent in total — its `Cost`.
    #[must_use]
    pub const fn total_spend(&self) -> &Spend {
        &self.total
    }

    /// The spend that bought nothing durable: total minus spend-at-commit.
    ///
    /// The number the two layers can only compute together. It is not waste and it is not
    /// a defect — a cancelled search legitimately explores past its last commit — but it is
    /// spend the caller paid for and no artifact accounts for, and a partial result that
    /// did not report it would let the cost of the committed evidence be read as the cost
    /// of the campaign.
    ///
    /// A task that committed nothing reports [`Spend::unmeasured`], not its whole cost: the
    /// difference between a measurement and a non-measurement is not a measurement
    /// ([`Spend::since`]), and [`Self::total_spend`] already says what the campaign cost.
    #[must_use]
    pub fn uncommitted_spend(&self) -> Spend {
        self.total.since(&self.spend_at_commit())
    }

    /// The INV-007 manifest carried over from the ledger: declared ceilings with no meter.
    ///
    /// Carried on the evidence rather than left on the ledger because a partial result
    /// travels: a caller reading "committed 3 artifacts, spent 900 states" has to be able
    /// to see, in the same value, that the wall-clock ceiling it also declared was never
    /// enforced.
    #[must_use]
    pub fn omissions(&self) -> &[DimensionOmission] {
        &self.omissions
    }

    /// A canonical multi-line rendering, for byte-identity comparison.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = format!(
            "partial {} cause={} committed={}\n",
            self.worker,
            self.cause,
            self.committed()
        );
        out.push_str(&format!("at-commit {}\n", self.spend_at_commit()));
        out.push_str(&format!("total {}\n", self.total));
        out.push_str(&format!("uncommitted {}\n", self.uncommitted_spend()));
        out.push_str("omissions:");
        for omission in &self.omissions {
            out.push(' ');
            out.push_str(&omission.render());
        }
        out.push('\n');
        out
    }
}

impl fmt::Display for CommittedPartialEvidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render())
    }
}

/// Every way admitting a worker into an [`EvidenceBook`] can be refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BookFault {
    /// This worker already has an account.
    ///
    /// Refused rather than replaced: a second ledger for one worker is a second answer to
    /// what that worker cost, and the first one may already have priced a commitment.
    AlreadyAdmitted {
        /// The worker the call named.
        worker: WorkerId,
    },
}

impl fmt::Display for BookFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyAdmitted { worker } => {
                write!(f, "{worker} already holds a budget account")
            }
        }
    }
}

impl core::error::Error for BookFault {}

/// One budget account per worker, and the cross-check against a finalized region subtree.
///
/// Keyed by [`WorkerId`] in a [`BTreeMap`], so iteration order is identity order on every
/// platform (INV-005) — the same reason
/// [`Ledger`](crate::region::obligation::Ledger) holds a [`BTreeSet`](std::collections::BTreeSet).
#[derive(Debug, Default)]
pub struct EvidenceBook {
    accounts: BTreeMap<WorkerId, BudgetLedger>,
}

impl EvidenceBook {
    /// A book with no accounts.
    #[must_use]
    pub fn new() -> Self {
        Self {
            accounts: BTreeMap::new(),
        }
    }

    /// Open an account for `worker` under `budget`, measuring what `meters` measures.
    ///
    /// # Errors
    ///
    /// [`BookFault::AlreadyAdmitted`] when the worker already has one.
    pub fn admit(
        &mut self,
        worker: WorkerId,
        budget: Budget,
        meters: MeterSet,
    ) -> Result<(), BookFault> {
        if self.accounts.contains_key(&worker) {
            return Err(BookFault::AlreadyAdmitted { worker });
        }
        self.accounts
            .insert(worker, BudgetLedger::new(budget, meters));
        Ok(())
    }

    /// `worker`'s ledger, if it has one.
    #[must_use]
    pub fn ledger(&self, worker: WorkerId) -> Option<&BudgetLedger> {
        self.accounts.get(&worker)
    }

    /// `worker`'s ledger for charging against.
    pub fn ledger_mut(&mut self, worker: WorkerId) -> Option<&mut BudgetLedger> {
        self.accounts.get_mut(&worker)
    }

    /// The workers with an account, in identity order.
    #[must_use]
    pub fn workers(&self) -> Vec<WorkerId> {
        self.accounts.keys().copied().collect()
    }

    /// The committed partial evidence for one worker of a finalized subtree.
    ///
    /// [`None`] when the worker has no account, or when it *completed*: a completed task's
    /// evidence is whole rather than partial, and reporting it here would invite a reader
    /// to treat a finished campaign as a truncated one.
    ///
    /// The cause is read off the region layer rather than guessed: a cancelled worker is
    /// [`TerminationCause::Cancelled`], and a failed one is
    /// [`TerminationCause::Exhausted`] when its ledger ran out of a ceiling and
    /// [`TerminationCause::Failed`] otherwise — so "why did this stop" is answered by the
    /// two layers together instead of by a token somebody typed twice.
    ///
    /// A worker in a non-terminal state also returns [`None`], and
    /// [`RegionTree::finalize`](crate::region::RegionTree::finalize) makes that
    /// unreachable from a [`Finalization`]: it refuses while any owned worker is
    /// non-terminal.
    #[must_use]
    pub fn partial_evidence(
        &self,
        worker: WorkerId,
        state: &WorkerState,
    ) -> Option<CommittedPartialEvidence> {
        let ledger = self.ledger(worker)?;
        let cause = match state {
            WorkerState::Cancelled => TerminationCause::Cancelled,
            WorkerState::Failed(reason) => ledger.exhaustion().map_or_else(
                || TerminationCause::Failed(reason.clone()),
                |exhaustion| TerminationCause::Exhausted(*exhaustion),
            ),
            WorkerState::Completed
            | WorkerState::Created
            | WorkerState::Running
            | WorkerState::Suspended => return None,
        };
        Some(CommittedPartialEvidence::bind(worker, cause, ledger))
    }

    /// Check this book against a finalized subtree.
    ///
    /// The three questions in this module's header, answered per worker. A reconciliation
    /// that reports no disagreement is the statement that every commitment the region layer
    /// counted has a price, every price names a commitment, and no headroom is still held
    /// in flight — "committed or absent, never dangling", read off the budget side.
    #[must_use]
    pub fn reconcile(&self, finalization: &Finalization) -> Reconciliation {
        let mut entries = Vec::new();
        let mut disagreements = Vec::new();
        let mut seen = Vec::new();
        for report in finalization.workers() {
            let worker = report.worker();
            seen.push(worker);
            let committed = report.evidence().committed();
            let Some(ledger) = self.ledger(worker) else {
                if committed > 0 {
                    disagreements.push(Disagreement::UnaccountedWorker { worker, committed });
                }
                continue;
            };
            let checkpoints = u32::try_from(ledger.checkpoints().len()).unwrap_or(u32::MAX);
            if checkpoints != committed {
                disagreements.push(Disagreement::CommittedCountDisagrees {
                    worker,
                    region: committed,
                    book: checkpoints,
                });
            }
            for dimension in ledger.reservations() {
                disagreements.push(Disagreement::DanglingReservation { worker, dimension });
            }
            entries.push(ReconciledWorker {
                worker,
                committed,
                checkpoints,
                spend: *ledger.spend(),
            });
        }
        for worker in self.workers() {
            if !seen.contains(&worker) {
                disagreements.push(Disagreement::UnknownWorker { worker });
            }
        }
        Reconciliation {
            region: finalization.region().to_string(),
            entries,
            disagreements,
        }
    }
}

/// One worker's reconciled accounting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconciledWorker {
    worker: WorkerId,
    committed: u32,
    checkpoints: u32,
    spend: Spend,
}

impl ReconciledWorker {
    /// Which worker.
    #[must_use]
    pub const fn worker(&self) -> WorkerId {
        self.worker
    }

    /// What the region layer counted.
    #[must_use]
    pub const fn committed(&self) -> u32 {
        self.committed
    }

    /// What the budget ledger priced.
    #[must_use]
    pub const fn checkpoints(&self) -> u32 {
        self.checkpoints
    }

    /// What it spent.
    #[must_use]
    pub const fn spend(&self) -> &Spend {
        &self.spend
    }

    /// A canonical one-line rendering.
    #[must_use]
    pub fn render(&self) -> String {
        format!(
            "{} committed={} checkpoints={} {}",
            self.worker, self.committed, self.checkpoints, self.spend
        )
    }
}

/// A way the budget accounting and the region layer disagree about one worker.
///
/// Every variant is a bug in one of the two layers, never a legal state — which is what
/// makes [`Reconciliation::is_reconciled`] a checkable claim rather than a summary.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Disagreement {
    /// The region layer committed a different number of publications than the budget
    /// ledger priced.
    CommittedCountDisagrees {
        /// The worker.
        worker: WorkerId,
        /// What the region layer counted.
        region: u32,
        /// How many checkpoints the budget ledger took.
        book: u32,
    },
    /// A budget account still holds headroom for a publication in flight, after the region
    /// that owned the worker finalized.
    ///
    /// The budget-side form of
    /// [`Finalization::unresolved_publications`](crate::region::Finalization::unresolved_publications),
    /// computed from headroom rather than from the region tree — so agreement between the
    /// two is evidence and not a tautology.
    DanglingReservation {
        /// The worker.
        worker: WorkerId,
        /// The dimension still holding a reservation.
        dimension: CostDimension,
    },
    /// A worker committed evidence and has no budget account, so its cost is unaccounted
    /// for.
    UnaccountedWorker {
        /// The worker.
        worker: WorkerId,
        /// What it committed.
        committed: u32,
    },
    /// The book holds an account for a worker the teardown did not report.
    UnknownWorker {
        /// The worker.
        worker: WorkerId,
    },
}

impl fmt::Display for Disagreement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CommittedCountDisagrees {
                worker,
                region,
                book,
            } => write!(
                f,
                "{worker}: the region committed {region} publications, the budget priced {book}"
            ),
            Self::DanglingReservation { worker, dimension } => write!(
                f,
                "{worker}: {dimension} headroom is still reserved after finalize"
            ),
            Self::UnaccountedWorker { worker, committed } => write!(
                f,
                "{worker} committed {committed} publications with no budget account"
            ),
            Self::UnknownWorker { worker } => {
                write!(f, "{worker} has a budget account but was not torn down")
            }
        }
    }
}

/// What checking a book against a finalized subtree found.
///
/// A value rather than a log line, for the reason
/// [`Finalization`](crate::region::Finalization) is one: a claim about a teardown has to be
/// checkable by the caller and by a test without either re-deriving it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reconciliation {
    region: String,
    entries: Vec<ReconciledWorker>,
    disagreements: Vec<Disagreement>,
}

impl Reconciliation {
    /// Every worker that had both a report and an account, in identity order.
    #[must_use]
    pub fn entries(&self) -> &[ReconciledWorker] {
        &self.entries
    }

    /// What did not agree. Empty is the claim.
    #[must_use]
    pub fn disagreements(&self) -> &[Disagreement] {
        &self.disagreements
    }

    /// Whether the two accountings agree everywhere.
    #[must_use]
    pub fn is_reconciled(&self) -> bool {
        self.disagreements.is_empty()
    }

    /// A canonical multi-line rendering, for byte-identity comparison.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = format!("reconciled {}\n", self.region);
        for entry in &self.entries {
            out.push_str(&entry.render());
            out.push('\n');
        }
        for disagreement in &self.disagreements {
            out.push_str(&format!("disagreement: {disagreement}\n"));
        }
        out.push_str(&format!(
            "agree: workers={} disagreements={}\n",
            self.entries.len(),
            self.disagreements.len()
        ));
        out
    }
}

impl fmt::Display for Reconciliation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::dimension::CostDimension;
    use crate::region::RegionTree;
    use crate::region::worker::{Resumability, WorkerStep};

    fn ledger() -> BudgetLedger {
        BudgetLedger::new(
            Budget::unbounded()
                .with(CostDimension::States, 100)
                .with(CostDimension::WallMs, 50),
            MeterSet::STATES_ONLY,
        )
    }

    #[test]
    fn partial_evidence_reports_what_was_committed_and_what_bought_nothing() {
        let mut ledger = ledger();
        ledger.charge(CostDimension::States, 30).expect("metered");
        ledger.checkpoint(1).expect("nothing reserved");
        ledger.charge(CostDimension::States, 12).expect("metered");
        let evidence =
            CommittedPartialEvidence::bind(WorkerId::at(0), TerminationCause::Cancelled, &ledger);
        assert_eq!(evidence.committed(), 1);
        assert!(evidence.published_anything());
        assert_eq!(
            evidence.spend_at_commit().measured(CostDimension::States),
            Some(30)
        );
        assert_eq!(
            evidence.total_spend().measured(CostDimension::States),
            Some(42)
        );
        assert_eq!(
            evidence.uncommitted_spend().measured(CostDimension::States),
            Some(12),
            "spend past the last commit bought nothing durable"
        );
    }

    #[test]
    fn a_task_that_committed_nothing_says_so_rather_than_pricing_an_artifact() {
        let mut ledger = ledger();
        ledger.charge(CostDimension::States, 7).expect("metered");
        let evidence =
            CommittedPartialEvidence::bind(WorkerId::at(3), TerminationCause::Cancelled, &ledger);
        assert_eq!(evidence.committed(), 0);
        assert!(!evidence.published_anything());
        assert_eq!(evidence.checkpoint(), None);
        assert_eq!(
            evidence.spend_at_commit().measured(CostDimension::States),
            None,
            "nothing was measured at a commit, because there was no commit"
        );
        assert_eq!(
            evidence.uncommitted_spend().measured(CostDimension::States),
            None
        );
        assert_eq!(
            evidence.total_spend().measured(CostDimension::States),
            Some(7),
            "the campaign still cost what it cost"
        );
    }

    #[test]
    fn partial_evidence_carries_the_unenforced_ceilings_with_it() {
        let evidence =
            CommittedPartialEvidence::bind(WorkerId::at(0), TerminationCause::Cancelled, &ledger());
        let subjects: Vec<String> = evidence.omissions().iter().map(|o| o.subject()).collect();
        assert_eq!(subjects, vec!["budget.wall_ms".to_owned()]);
    }

    #[test]
    fn a_book_and_a_teardown_agree_when_every_commitment_was_priced() {
        let mut tree = RegionTree::new();
        let root = tree.root();
        let worker = tree.spawn(root, Resumability::Resumable).expect("open");
        let mut book = EvidenceBook::new();
        book.admit(
            worker,
            Budget::unbounded().with(CostDimension::States, 64),
            MeterSet::STATES_ONLY,
        )
        .expect("fresh worker");

        tree.advance(worker, WorkerStep::Begin).expect("legal");
        tree.advance(worker, WorkerStep::Reserve).expect("legal");
        tree.advance(worker, WorkerStep::Commit).expect("legal");
        let account = book.ledger_mut(worker).expect("admitted");
        account.charge(CostDimension::States, 20).expect("metered");
        account.checkpoint(1).expect("nothing reserved");

        let finalization = tree.teardown(root).expect("torn down");
        assert!(finalization.is_total());
        let reconciliation = book.reconcile(&finalization);
        assert!(
            reconciliation.is_reconciled(),
            "{}",
            reconciliation.render()
        );
        assert_eq!(reconciliation.entries().len(), 1);
        assert_eq!(reconciliation.entries()[0].committed(), 1);
        assert_eq!(reconciliation.entries()[0].checkpoints(), 1);
    }

    #[test]
    fn a_commitment_nobody_priced_is_a_disagreement() {
        let mut tree = RegionTree::new();
        let root = tree.root();
        let worker = tree.spawn(root, Resumability::Resumable).expect("open");
        let mut book = EvidenceBook::new();
        book.admit(worker, Budget::unbounded(), MeterSet::STATES_ONLY)
            .expect("fresh worker");
        tree.advance(worker, WorkerStep::Begin).expect("legal");
        tree.advance(worker, WorkerStep::Reserve).expect("legal");
        tree.advance(worker, WorkerStep::Commit).expect("legal");
        let finalization = tree.teardown(root).expect("torn down");
        let reconciliation = book.reconcile(&finalization);
        assert_eq!(
            reconciliation.disagreements(),
            &[Disagreement::CommittedCountDisagrees {
                worker,
                region: 1,
                book: 0
            }]
        );
        assert!(!reconciliation.is_reconciled());
    }

    #[test]
    fn headroom_still_held_after_a_teardown_is_a_dangling_reservation() {
        let mut tree = RegionTree::new();
        let root = tree.root();
        let worker = tree.spawn(root, Resumability::Resumable).expect("open");
        let mut book = EvidenceBook::new();
        book.admit(
            worker,
            Budget::unbounded().with(CostDimension::Bytes, 100),
            MeterSet::all(),
        )
        .expect("fresh worker");
        book.ledger_mut(worker)
            .expect("admitted")
            .reserve(CostDimension::Bytes, 10)
            .expect("metered");
        let finalization = tree.teardown(root).expect("torn down");
        assert!(
            finalization.unresolved_publications().is_empty(),
            "the region layer resolved its own half"
        );
        assert_eq!(
            book.reconcile(&finalization).disagreements(),
            &[Disagreement::DanglingReservation {
                worker,
                dimension: CostDimension::Bytes
            }],
            "the budget half is an independent accounting and catches what the region's cannot"
        );
    }

    #[test]
    fn an_account_for_a_worker_nobody_tore_down_is_named() {
        let mut tree = RegionTree::new();
        let root = tree.root();
        let mut book = EvidenceBook::new();
        book.admit(WorkerId::at(7), Budget::unbounded(), MeterSet::none())
            .expect("fresh worker");
        assert_eq!(
            book.admit(WorkerId::at(7), Budget::unbounded(), MeterSet::none()),
            Err(BookFault::AlreadyAdmitted {
                worker: WorkerId::at(7)
            })
        );
        let finalization = tree.teardown(root).expect("torn down");
        assert_eq!(
            book.reconcile(&finalization).disagreements(),
            &[Disagreement::UnknownWorker {
                worker: WorkerId::at(7)
            }]
        );
    }

    #[test]
    fn a_completed_worker_has_evidence_rather_than_partial_evidence() {
        let mut book = EvidenceBook::new();
        book.admit(WorkerId::at(0), Budget::unbounded(), MeterSet::STATES_ONLY)
            .expect("fresh worker");
        assert!(
            book.partial_evidence(WorkerId::at(0), &WorkerState::Completed)
                .is_none()
        );
        assert!(
            book.partial_evidence(WorkerId::at(0), &WorkerState::Cancelled)
                .is_some()
        );
    }

    #[test]
    fn a_failed_worker_whose_ledger_ran_out_reports_the_dimension_not_the_token() {
        let mut book = EvidenceBook::new();
        let worker = WorkerId::at(0);
        book.admit(
            worker,
            Budget::unbounded().with(CostDimension::States, 4),
            MeterSet::STATES_ONLY,
        )
        .expect("fresh worker");
        book.ledger_mut(worker)
            .expect("admitted")
            .charge(CostDimension::States, 5)
            .expect("metered");
        let state = WorkerState::Failed(
            FailureReason::new("budget-exhausted").expect("test token is canonical"),
        );
        let evidence = book
            .partial_evidence(worker, &state)
            .expect("a failed worker with an account has partial evidence");
        assert_eq!(
            evidence.cause().exhaustion().map(Exhaustion::dimension),
            Some(CostDimension::States)
        );
    }
}
