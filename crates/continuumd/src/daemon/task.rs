//! The `task` family — `status`, `cancel`, `resume`, `subscribe`, `update_budget` — and
//! the table its tasks are values in.
//!
//! # A task is a value, and that is the whole design
//!
//! Plan §20 gives `continuumd` no async runtime edge, INV-005 and ADR-0003 put the daemon
//! core inside the deterministic band, and [`Daemon::dispatch`](super::Daemon::dispatch)
//! takes `&mut self`. So there is no thread, no executor, and no interleaving here: a task
//! is an entry in [`TaskTable`], and the only thing that advances one is an explicit
//! dispatch. Three consequences are load-bearing rather than incidental:
//!
//! - **[`TaskStatus::Running`] is representable and never reported.** It is the state a
//!   task is in *during* the dispatch that advances it, and no second dispatch can observe
//!   it, because `dispatch` holds `&mut DaemonState` for the whole call. The statuses a
//!   caller can read are `Suspended`, `Completed`, `Failed`, and `Cancelled`. That is a
//!   property of the borrow, not a simplification of the lifecycle.
//! - **`rule task.status_monotonic` is enforced by construction.** [`TaskEntry::advance`]
//!   is the only writer of `status`, and it refuses every transition out of a terminal
//!   state. Milestones and `committed_evidence` are appended and never removed.
//! - **A bounded campaign that cannot finish parks instead of truncating.** The engine's
//!   `Exploration::Exhausted` arm already carries the explored set, the queue-ordered
//!   frontier and which bound tripped; the frontier *is* the continuation payload, so the
//!   type-level seam RFC 0026's "budget exhaustion carries a continuation" rule needs was
//!   already there, and this module binds a `cont_*` handle to it.
//!
//! What a task's *execution* is, on the other hand, is a region. PR 6 puts every unit of
//! task work inside one, opened and finalized within the dispatch that runs it, and that is
//! what makes "no orphan work" a checked property rather than a consequence of the borrow
//! alone. [`region`](super::region) is that join, and it is where the one design question
//! this rewiring had to answer — whether a parked continuation is a live worker — is
//! answered, argued from RFC 0026, and made visible in [`TaskEntry::region`].
//!
//! # The five operations
//!
//! | Operation | Rule | What it does here |
//! |---|---|---|
//! | `task.status` | `rule task.status_monotonic` | projects a [`TaskEntry`] onto the wire's [`TaskRecord`] |
//! | `task.cancel` | `rule task.cancel_correct` | marks a non-terminal task `Cancelled`, and drives cancel → drain → finalize over the task's residual work, reporting the arm that teardown landed on — a continuation, or `null`, the *named* "nothing published" outcome |
//! | `task.resume` | `rule task.resume` | the admissibility predicate, then one more bounded run under the pinned epochs |
//! | `task.subscribe` | `rule subscription.hints_only` | answers the record the IDL declares as its response body; the events are the recorded transitions |
//! | `task.update_budget` | `rule task.update_budget` | re-admits a parked task under a larger bound, with no identity churn |
//!
//! # Identity, and why it is content-addressed
//!
//! A `task_*` handle is the content identity of what the task *is* — its snapshot, intent,
//! target, portfolio, priority class, budget and epochs — and a `cont_*` handle is the
//! content identity of what a continuation *pins*. Neither is drawn, counted, or timed, so
//! "replaying an idempotent request returns the same task identity" (the PR 5 exit) holds
//! before the idempotency ledger is consulted at all: two equal requests name one task
//! because they name one preimage. The ledger then makes the whole *outcome* identical; the
//! identity agrees without it. It is also exactly what the IDL asks `verification.start`
//! for — "a cached result when one exists for the same snapshot, intent, target, and epochs"
//! is a lookup by that identity, not a cache key invented beside it.
//!
//! # Where the frozen Die Hard facts are, and which of the two gaps is the wire's
//!
//! `TaskRecord.cost.states` carries the reachable-state count — **16** for Die Hard — and
//! that is the one frozen fact RFC 0026's `Cost` declares a dimension for. The other two
//! are held as the engine's own typed values on [`TaskEntry::campaign`] and are reachable
//! through [`Daemon::state`](super::Daemon::state), and bn-i4aem item 8 separated them,
//! because they are not the same kind of gap:
//!
//! - the **96** labelled transitions have no wire field and cannot get one at a minor.
//!   `Cost` and `Budget` carry one nine-dimension list, and SD-12 holds that list
//!   identical across this protocol, `schemas/verification-task.schema.json` (whose
//!   `budget` object sets `additionalProperties: false`), and the plan §8.6 cost ledger.
//!   A tenth dimension therefore moves four artifacts together, one of them rank-1, and is
//!   not something a wire revision decides on its own. Deferred, recorded as RFC 0026 F16;
//! - the depth-**6** shortest witness **does** have a wire home and always did:
//!   `VerificationResult.crashpack`, whose artifact class is
//!   `schemas/crashpack.schema.json`. What is missing is a producer — nothing in this
//!   workspace builds a crashpack — so this is an implementation gap wearing a wire gap's
//!   clothes, and reporting it as the IDL's would have sent a fix to the wrong artifact.
//!
//! Neither is papered over by repurposing a dimension that means something else.

use std::collections::BTreeMap;

use continuum_engine_reference::bfs::{Bound, Bounds, Exploration};
use continuum_engine_reference::checking::{
    CheckOutcome, CheckReport, DeadlockOutcome, Unresolved,
};
use continuum_engine_reference::model::State;
use continuum_task::budget::BudgetLedger;
use continuum_task::region::RegionId;
use continuum_task::region::worker::{WorkerId, WorkerStep};
use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};
use continuum_workspace::publication::{ContentIdentifier, Published, ReferenceStore};
use continuum_workspace::staleness::{LineageError, check_current};

use super::admission::Derived;
use super::budget::{self, Publications};
use super::continuation::{self, ParkState};
use super::family::{Arguments, Call, Effect, Fault, OperationFamily, Payload, ScopeClaim};
use super::output;
use super::region::{self, Scope, TaskRegions};
use super::state::DaemonState;
use super::{Services, verification};
use crate::protocol::envelope::{
    ArtifactRef, Budget, Cost, EpochSet, Omission, StructuralVerdictValue, Verdict,
};
use crate::protocol::operations::task::{
    TaskCancelRequest, TaskCancelResponse, TaskResumeRequest, TaskResumeResponse,
    TaskStatusRequest, TaskSubscribeRequest, TaskSubscribeResponse, TaskUpdateBudgetRequest,
    TaskUpdateBudgetResponse,
};
use crate::protocol::scalar::{
    Commitment, ContinuationHandle, EpochIdentity, EvidenceHandle, IntentHandle, OperationName,
    TaskHandle, Timestamp, WorkspaceHandle,
};
use crate::protocol::shared::Target;
use crate::protocol::spec::{Nullable, Optional};
use crate::protocol::task::{Milestone, TaskEvent, TaskRecord};
use crate::protocol::vocabulary::{
    ErrorCode, OmissionReason, Portfolio, PriorityClass, StructuralOutcome, TaskEventKind,
    TaskStatus,
};

// ---------------------------------------------------------------------------
// what a campaign produced
// ---------------------------------------------------------------------------

/// One bounded run of the reference engine, kept as the engine's own values.
///
/// Held rather than flattened onto the wire because the wire cannot carry it: see this
/// module's documentation on the frozen facts. Everything a wire answer needs is projected
/// out of it by [`TaskEntry::record`] and by the `verification` family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Campaign {
    /// What checking every declared obligation over this exploration established.
    pub report: CheckReport,
    /// Labelled transitions counted across every expanded row. **96** for a closed Die Hard.
    pub transitions: u64,
    /// The deepest layer discovered, or [`None`] for an empty exploration.
    pub max_depth: Option<usize>,
    /// The queue-ordered frontier when a bound tripped; empty when the exploration closed.
    ///
    /// This is the resume point, and the reason the continuation payload needed no
    /// invention: `Partial::frontier` already names "discovered states that were never
    /// expanded", in the order a resumed walk takes them.
    pub frontier: Vec<State>,
    /// Which declared bound stopped the walk, or [`None`] when it closed.
    pub tripped: Option<Bound>,
}

impl Campaign {
    /// Read one exploration and its report into a campaign.
    #[must_use]
    pub fn of(exploration: &Exploration, report: CheckReport) -> Self {
        let reachable = exploration.reachable();
        let partial = exploration.exhausted();
        Self {
            report,
            transitions: reachable.transitions(),
            max_depth: reachable.max_depth(),
            frontier: partial
                .map(|partial| partial.frontier().to_vec())
                .unwrap_or_default(),
            tripped: partial.map(super::verification::tripped),
        }
    }

    /// How many states the answers were computed over. **16** for a closed Die Hard.
    #[must_use]
    pub fn states(&self) -> usize {
        self.report.scope().states()
    }

    /// Whether the exploration closed — docs/03 §3 `EXHAUSTIVE_FINITE`.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.report.scope().is_complete()
    }

    /// The first typed reason a claim in this report was not decided, when there is one.
    ///
    /// INV-008: an inconclusive verdict is never silent, so the wire's
    /// `SemanticVerdictValue.inconclusive_reason` is read off the engine's own
    /// [`Unresolved`] rather than guessed from the scope.
    #[must_use]
    pub fn unresolved(&self) -> Option<&Unresolved> {
        for result in self.report.invariants() {
            if let CheckOutcome::Inconclusive(reason) = result.outcome() {
                return Some(reason);
            }
        }
        match self.report.deadlock() {
            DeadlockOutcome::Inconclusive(reason) => Some(reason),
            DeadlockOutcome::NotJudged { .. }
            | DeadlockOutcome::Free { .. }
            | DeadlockOutcome::Deadlocked { .. } => None,
        }
    }

    /// The shallowest depth at which some declared invariant is violated, when one is.
    ///
    /// Die Hard's frozen third fact reads off this: `NotSolved` is refuted at depth **6**,
    /// the film's six-step solution.
    #[must_use]
    pub fn violation_depth(&self) -> Option<usize> {
        self.report
            .invariants()
            .iter()
            .filter_map(|result| match result.outcome() {
                CheckOutcome::Violated { depth, .. } => Some(*depth),
                CheckOutcome::Holds { .. } | CheckOutcome::Inconclusive(_) => None,
            })
            .min()
    }
}

// ---------------------------------------------------------------------------
// continuations
// ---------------------------------------------------------------------------

/// The compatibility epochs a continuation pins, and the engine identity beside them.
///
/// > A continuation MUST leave `protocol` `Unpinned` in its pinned set: the protocol epoch
/// > is connection-scoped and is consumed by no task.
/// >
/// > — RFC 0026, "The two-predicate obligation"
///
/// The wire's [`EpochSet`] cannot express that — its `protocol` field is `required` — so the
/// pinned set is this struct rather than an `EpochSet`, and the protocol epoch is
/// *structurally* absent from a resume comparison instead of being skipped by a rule
/// somebody has to remember. Five compatibility epochs are pinnable, because those are the
/// five `EpochSet` carries beside `protocol`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinnedEpochs {
    /// The meaning of evaluation (plan §4.6, ADR-0018).
    pub semantic: Nullable<EpochIdentity>,
    /// The Intent Contract vocabulary and its policy tables.
    pub intent: Nullable<EpochIdentity>,
    /// The evidence-graph and receipt schema epoch.
    pub evidence: Nullable<EpochIdentity>,
    /// The Lean toolchain and theorem-package closure (ADR-0035).
    pub proof: Nullable<EpochIdentity>,
    /// The pinned corpus revision and oracle toolchain.
    pub corpus: Nullable<EpochIdentity>,
    /// Engine identity (plan §4.7) — provenance, not a seventh epoch.
    pub engine: Nullable<EpochIdentity>,
}

impl PinnedEpochs {
    /// What a result's [`EpochSet`] pins, with `protocol` deliberately dropped.
    #[must_use]
    pub fn of(epochs: &EpochSet) -> Self {
        Self {
            semantic: epochs.semantic.clone(),
            intent: epochs.intent.clone(),
            evidence: epochs.evidence.clone(),
            proof: epochs.proof.clone(),
            corpus: epochs.corpus.clone(),
            engine: epochs.engine.clone(),
        }
    }

    /// The five compatibility epochs, in `EpochKind::ALL` order minus `protocol`.
    fn compatibility(&self) -> [&Nullable<EpochIdentity>; 5] {
        [
            &self.semantic,
            &self.intent,
            &self.evidence,
            &self.proof,
            &self.corpus,
        ]
    }
}

/// A run refused on the continuation it would mint (`rule capability.instance_scope`, the
/// minted-handle clause; cr-3hcpn4): the capability that presented it, the task, the
/// continuation it started from (none for a `verification.start`), and the bounds it ran
/// under.
///
/// The run is deterministic, so the same run for the same capability would mint the same
/// handle and be refused again. [`TaskTable::refuse_run`] keeps the key and the states the
/// refused run explored — the work charged to that capability — and a repeat is refused
/// before any exploration, so a fresh idempotency key buys nothing. The key is per
/// capability, not per task: a task identity is shared by every caller that names it, and
/// one caller's refusal must not change what another is served.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RefusedRun {
    /// The capability the refused request presented.
    pub capability: crate::protocol::scalar::CapabilityHandle,
    /// The task the run was for.
    pub task: TaskHandle,
    /// The continuation the run started from.
    pub from: Option<ContinuationHandle>,
    /// The bounds it ran under.
    pub bounds: Bounds,
}

/// A `cont_*` continuation: what it pins, and the search state it resumes from.
///
/// Everything RFC 0026's "Continuations and resume admissibility" clause requires a
/// continuation to pin at creation is a field here — the snapshot, the intent where the task
/// is intent-scoped, the committed frontier, the compatibility epochs, and engine identity —
/// and this type has no way to be built without them. A continuation omitting one "is
/// malformed and MUST be rejected at creation, not at resume"; here it is unconstructible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Continuation {
    /// The continuation's own content identity.
    pub handle: ContinuationHandle,
    /// The task it resumes.
    pub task: TaskHandle,
    /// The snapshot the parked run was over.
    pub snapshot: WorkspaceHandle,
    /// The governing intent, by identity only (plan §4.2, INV-001).
    pub intent: Nullable<IntentHandle>,
    /// The epochs and the engine identity the parked run consumed.
    pub pinned: PinnedEpochs,
    /// The bounds that tripped.
    pub bounds: Bounds,
    /// The queue-ordered frontier: discovered states that were never expanded.
    ///
    /// Empty for a continuation restored at startup until `task.resume` reads its durable
    /// frontier back through the model (bn-20142): a state is a value *of a model*, and the
    /// model catalog is provisioned after startup. [`TaskTable::frontier_of`] answers the
    /// frontier as vectors in both cases.
    pub frontier: Vec<State>,
}

// ---------------------------------------------------------------------------
// the table
// ---------------------------------------------------------------------------

/// One task, as the daemon holds it.
///
/// Deliberately not [`Clone`], [`PartialEq`] or [`Eq`] as of bn-23j7s: it holds a
/// [`BudgetLedger`], which is not `Clone` for the reason
/// [`Ledger`](continuum_task::region::obligation::Ledger) is not — "a ledger is one task's
/// single accounting, and a second copy of it is a second answer to what this cost". The
/// three derives were unused; a task is compared through [`Self::record`], which is the
/// wire value and *is* comparable.
#[derive(Debug)]
pub struct TaskEntry {
    /// The task's content identity.
    pub handle: TaskHandle,
    /// The `@task_starting` operation that created it. `rule task.no_generic_start`: there
    /// is no generic `task.start`, so every task names the operation it came from.
    pub operation: OperationName,
    /// Lifecycle state (plan §4.1).
    pub status: TaskStatus,
    /// The snapshot the campaign is over.
    pub snapshot: Nullable<WorkspaceHandle>,
    /// The governing Intent Contract, by identity only.
    pub intent: Nullable<IntentHandle>,
    /// What the campaign is aimed at.
    pub target: Target,
    /// The declared portfolio profile.
    pub portfolio: Portfolio,
    /// The declared priority class.
    pub priority_class: PriorityClass,
    /// This task's budget accounting: what it may spend, what it has spent, and every
    /// checkpoint it has bound committed evidence to.
    ///
    /// The seam `crates/continuum-task/src/budget.rs` designed and bn-23j7s wired. It
    /// replaces two fields that were second readings of it — a `budget: Budget` and the
    /// `bounds: Bounds` derived from that budget — because two fields carrying one fact are
    /// two answers to what a task may spend. Both are still available and are now
    /// *projections*: [`Self::budget`] is the wire spelling of the ledger's ceilings and
    /// [`Self::bounds`] is its `states` ceiling as an engine bound.
    ///
    /// `task.update_budget` writes it through [`budget::update`], which returns
    /// `rule task.update_budget`'s five-arm legality table per dimension; the identity does
    /// not change, because a `task_*` handle's preimage is fixed at creation.
    pub ledger: BudgetLedger,
    /// The epochs the task was created under, copied onto its record.
    pub epochs: EpochSet,
    /// The content identity of the model source the campaign runs against.
    pub model: Commitment,
    /// Semantic milestones reached, in order. Append-only.
    pub milestones: Vec<Milestone>,
    /// Evidence committed so far. Append-only; empty until the evidence graph ships.
    pub committed_evidence: Vec<EvidenceHandle>,
    /// The transitions this task recorded, which is what a subscription delivers.
    pub events: Vec<TaskEvent>,
    /// The continuation, present while `status = suspended` and after a cancel that left one.
    pub continuation: Option<ContinuationHandle>,
    /// REQUIRED when `status = failed` (plan §4.5): a `Failed` task is never silent.
    pub failed_reason: Option<ErrorCode>,
    /// REQUIRED when `failed_reason = BudgetExhausted` and no continuation exists.
    pub non_resumable_reason: Option<String>,
    /// What the last run produced, or [`None`] when nothing ran.
    pub campaign: Option<Campaign>,
    /// The region the task's most recent run happened in, or [`None`] before anything ran.
    ///
    /// The seam bn-2gk designed — "the PR 6 rewiring is a `TaskEntry` gaining a
    /// [`RegionId`] and delegating" — and the field where this bone's design decision is
    /// legible: it is **not** a handle to a live scope, because a resumed task's region is
    /// a *different* one from the region it parked in. A parked continuation is a durable
    /// artifact, not a live worker; [`region`](super::region) states the RFC argument in
    /// full.
    pub region: Option<RegionId>,
    /// The worker that most recent run *was*, or [`None`] before anything ran.
    pub worker: Option<WorkerId>,
    /// What this task has committed — **named**, not counted (bn-23j7s).
    ///
    /// It holds *campaign results*: a run that produced one wrote it onto
    /// [`Self::campaign`], where `verification.result` reads it, and that is what a reader
    /// can observe of this task today. The evidence-graph handles in
    /// [`Self::committed_evidence`] are PR 7's, and a task that commits none says so
    /// through [`Self::omissions`] rather than reporting a zero it did not measure.
    ///
    /// This was a `publications: u32` counter, and the difference is IMPL-02's own
    /// acceptance criterion: "committed partial evidence survives daemon restart;
    /// uncommitted partials are absent, never half-visible" is not a claim a counter can be
    /// held to, because a count that came back as 2 says nothing about *which* two. Each
    /// publication now carries the content identity of what was committed, and
    /// [`Publications`] is a linear typestate in which a staged publication is
    /// unobservable. [`Self::publications`] is the count, derived.
    ///
    /// Monotone: `task.resume` adds to it and nothing subtracts, which is INV-009's
    /// "resuming a task may add evidence … it may not silently replace prior artifacts" at
    /// the list. The invariant `continuation.is_some() ⇒ publications > 0` is what makes
    /// the cancellation table's both-or-neither arms unconstructable wrong, and it holds by
    /// construction: [`verification::advance`](super::verification::advance) is the one
    /// writer of both, and it commits the publication before it mints the continuation.
    pub evidence: Publications,
}

impl TaskEntry {
    /// Whether the task can no longer change (`rule task.status_monotonic`).
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
        )
    }

    /// Move to `status`, recording the transition, unless the task is already terminal.
    ///
    /// Returns whether the move happened. The refusal is the whole of
    /// `rule task.status_monotonic`'s "a terminal status never changes": no other path
    /// writes `status`.
    pub fn advance(&mut self, status: TaskStatus, now: Option<&Timestamp>) -> bool {
        if self.is_terminal() {
            return false;
        }
        self.status = status;
        if let Some(at) = now {
            self.events.push(TaskEvent {
                task: self.handle.clone(),
                at: at.clone(),
                kind: TaskEventKind::StatusChange,
                milestone: Optional::Absent,
                cost: Optional::Absent,
                status: Optional::Present(status),
            });
        }
        true
    }

    /// Append a milestone and the event that reports it.
    ///
    /// A [`Milestone`] REQUIRES a `Timestamp`, and time is not ambient (INV-005, ADR-0003),
    /// so a deployment that supplied no reading records none — and [`TaskEntry::omissions`]
    /// says so with a typed [`Omission`] rather than inventing a clock or emitting a
    /// milestone at a made-up time.
    pub fn reach(&mut self, name: &str, now: Option<&Timestamp>) {
        let Some(at) = now else { return };
        let milestone = Milestone {
            name: name.to_owned(),
            at: at.clone(),
        };
        self.milestones.push(milestone.clone());
        self.events.push(TaskEvent {
            task: self.handle.clone(),
            at: at.clone(),
            kind: TaskEventKind::Milestone,
            milestone: Optional::Present(milestone),
            cost: Optional::Absent,
            status: Optional::Absent,
        });
    }

    /// The budget in force, in the wire's spelling.
    ///
    /// A projection of [`Self::ledger`]'s ceilings rather than a field beside them
    /// (bn-23j7s). `task.update_budget` records a ceiling on every arm of its legality
    /// table, including the one that parks the task, so this reports what the caller most
    /// recently declared — never a bound the caller has withdrawn.
    #[must_use]
    pub fn budget(&self) -> Budget {
        budget::wire_budget(&self.ledger)
    }

    /// The engine bounds this task's budget declares.
    ///
    /// A projection of the ledger's `states` ceiling, floored at recorded spend — see
    /// [`budget::bounds_of`] for why the floor is what keeps a lowered ceiling from
    /// un-exploring a campaign INV-009 forbids truncating.
    #[must_use]
    pub fn bounds(&self) -> Bounds {
        budget::bounds_of(&self.ledger)
    }

    /// How many publications are durable.
    #[must_use]
    pub fn publications(&self) -> u32 {
        self.evidence.count()
    }

    /// What this task spent, in the dimensions the engine measures.
    ///
    /// > A dimension the engine does not measure is absent, never zero.
    /// >
    /// > — `Cost`, IDL §6
    ///
    /// A projection of [`BudgetLedger::spend`] (bn-23j7s), where it was a second reading of
    /// `campaign.states()`. The reference engine counts states and labelled transitions;
    /// RFC 0026's `Cost` declares a dimension for the first and none for the second, and
    /// [`budget::METERS`] says so — `states` is reported and every other dimension is
    /// absent because nothing measures it. Nothing here reads a clock, so `wall_ms` is
    /// absent on every task — absent as a missing *measurement*, which is also what keeps
    /// `rule ordering.deterministic`'s byte-identical repeat true of a wall clock's output.
    ///
    /// The campaign has not stopped being where the number comes from; it has stopped being
    /// where the number is *kept*. A campaign is what gives this daemon's one meter a
    /// reading, so a task that produced none reports absent rather than the ledger's
    /// `Some(0)` starting point.
    #[must_use]
    pub fn cost(&self) -> Cost {
        // A run that committed a publication charged the meter, so the ledger holds a reading.
        // For a live task the two conditions agree: `verification::advance` writes the
        // campaign and commits the publication together. A task restored from its
        // continuation record (bn-20142) has the committed publications and the recorded
        // spend but not the engine's report, and its cost is the spend it recorded.
        budget::cost_of(
            &self.ledger,
            self.campaign.is_some() || self.evidence.count() > 0,
        )
    }

    /// The wire record for this task.
    #[must_use]
    pub fn record(&self) -> TaskRecord {
        TaskRecord {
            task: self.handle.clone(),
            operation: self.operation.clone(),
            status: self.status,
            snapshot: self.snapshot.clone(),
            intent: self.intent.clone(),
            failed_reason: optional(self.failed_reason),
            continuation: optional(self.continuation.clone()),
            non_resumable_reason: optional(self.non_resumable_reason.clone()),
            budget: self.budget(),
            cost: self.cost(),
            epochs: self.epochs.clone(),
            priority_class: self.priority_class,
            milestones: self.milestones.clone(),
            committed_evidence: self.committed_evidence.clone(),
        }
    }

    /// What an answer about this task deliberately leaves out (INV-007).
    #[must_use]
    pub fn omissions(&self) -> Vec<Omission> {
        let mut omissions = Vec::new();
        if self.milestones.is_empty() {
            // No clock reading was supplied, so no milestone could be timed. Named rather
            // than inferred from an empty list.
            omissions.push(unsupported("task.milestones"));
        }
        if self.committed_evidence.is_empty() {
            // The evidence graph is PR 7; a task that commits none says so.
            omissions.push(unsupported("task.committed_evidence"));
        }
        omissions
    }
}

/// Every task this daemon holds, and every continuation that names one.
///
/// Both maps are [`BTreeMap`]s: iteration order is a function of the keys present and of
/// nothing else, which is what `rule ordering.deterministic` needs from the container layer.
///
/// # A terminal task's continuations are kept, not pruned
///
/// Nothing removes from `continuations`, and that is the ratified disposition rather than an
/// omission. bn-3p32's A1 reached a cancelled task's ledger *through* a continuation this
/// table still held, so "prune the continuations of a terminal task" was the other candidate
/// repair; it was not taken, for two reasons stated here because this is where the table is:
///
/// - **Handle validity is a protocol-major property.** RFC 0026 makes a `cont_*` a durable
///   artifact identity, not a live scope — "a continuation is a durable artifact, not a live
///   worker" — and an identity minted under a protocol major stays resolvable for that major.
///   Pruning would turn a resume of a cancelled task's continuation into
///   [`Fault::denied`] — RFC 0027 X2's undistinguished denial — which says *nothing about
///   this daemon* and is byte-identical with "never existed". The caller would lose the one
///   honest answer: the task is terminal, and here is its status.
/// - **The guard belongs at the write, not at the lookup.** `task.resume` refuses every write
///   to a terminal task before it touches the ledger (see its terminal check), so holding
///   the handle confers nothing a terminal task should not grant. Pruning would defend the
///   same property one layer away from the thing it protects, and would leave the ledger
///   write unguarded for any *other* door that resolves a continuation.
///
/// So a cancelled task's continuation still resolves, still names its task, and still
/// answers "this task is `Cancelled`" — and cannot write it. `TaskRecord.continuation` is a
/// separate reading: it reports what the *record* holds, and a cancel that published nothing
/// clears it there without this table forgetting the identity.
#[derive(Debug, Default)]
pub struct TaskTable {
    tasks: BTreeMap<TaskHandle, TaskEntry>,
    continuations: BTreeMap<ContinuationHandle, Continuation>,
    /// Tasks a startup resolution pass found in the store and resolved to `Settled` or
    /// `Failed` (plan §4.5 O2, bn-1z09m).
    ///
    /// Kept apart from `tasks` on purpose. Such a task is not a `TaskEntry`: no durable
    /// record of it holds what a `TaskEntry` needs, and one cannot be built without that
    /// except by inference. A task resolved to `Restored` or `Terminal` is not here: its
    /// continuation record (bn-20142) or terminal record (bn-2g3ei) holds what a `TaskEntry`
    /// needs, so it is in `tasks`.
    resolved: BTreeMap<TaskHandle, super::recovery::ResolvedTask>,
    /// The frontiers of continuations restored at startup, as state vectors, until
    /// `task.resume` reads them back through the model ([`Self::materialize`]).
    pending: BTreeMap<ContinuationHandle, Vec<Vec<i64>>>,
    /// The last durable revision of each continuation record this daemon published or
    /// restored ([`continuation`](super::continuation)), the state count its pins name, and
    /// the revision's store handle tied to its receipt (bn-283p6). A revision is recorded
    /// as durable only with the receipt-tied name of the record that makes it so.
    revisions: BTreeMap<ContinuationHandle, DurableRevision>,
    /// Runs refused on the continuation they would mint, and the states each explored:
    /// the refused work, charged to the capability that presented it ([`RefusedRun`]).
    refused: BTreeMap<RefusedRun, u64>,
}

impl TaskTable {
    /// The empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or replace a task.
    pub fn put(&mut self, entry: TaskEntry) {
        self.tasks.insert(entry.handle.clone(), entry);
    }

    /// Remove a task this dispatch created and must not leave behind — a start refused on
    /// the continuation its run would mint (cr-3hcpn4).
    pub fn remove(&mut self, handle: &TaskHandle) -> Option<TaskEntry> {
        self.tasks.remove(handle)
    }

    /// Record a run refused on the continuation it would mint, with the states it explored.
    pub fn refuse_run(&mut self, run: RefusedRun, explored: u64) {
        self.refused.insert(run, explored);
    }

    /// Whether `run` was refused before, so a repeat is refused without exploring.
    #[must_use]
    pub fn refused(&self, run: &RefusedRun) -> bool {
        self.refused.contains_key(run)
    }

    /// The states explored by refused runs, summed per capability: the refused work each
    /// capability was charged with. No handle but the capability's is in it, and it never
    /// reaches the wire.
    #[must_use]
    pub fn refused_work(&self) -> BTreeMap<crate::protocol::scalar::CapabilityHandle, u64> {
        let mut charged = BTreeMap::new();
        for (run, explored) in &self.refused {
            *charged.entry(run.capability.clone()).or_insert(0) += explored;
        }
        charged
    }

    /// The task `handle` names, or [`None`].
    #[must_use]
    pub fn get(&self, handle: &TaskHandle) -> Option<&TaskEntry> {
        self.tasks.get(handle)
    }

    /// The task `handle` names, mutably.
    pub fn get_mut(&mut self, handle: &TaskHandle) -> Option<&mut TaskEntry> {
        self.tasks.get_mut(handle)
    }

    /// Every task handle, in identity order.
    #[must_use]
    pub fn handles(&self) -> Vec<&TaskHandle> {
        self.tasks.keys().collect()
    }

    /// Bind a continuation.
    pub fn park(&mut self, continuation: Continuation) {
        self.continuations
            .insert(continuation.handle.clone(), continuation);
    }

    /// The continuation `handle` names, or [`None`].
    #[must_use]
    pub fn continuation(&self, handle: &ContinuationHandle) -> Option<&Continuation> {
        self.continuations.get(handle)
    }

    /// Record what the startup resolution pass concluded about one task.
    pub fn resolve(&mut self, resolved: super::recovery::ResolvedTask) {
        self.resolved.insert(resolved.task.clone(), resolved);
    }

    /// The startup resolution of `handle`, or [`None`] when the pass found no record of it.
    #[must_use]
    pub fn resolution(&self, handle: &TaskHandle) -> Option<&super::recovery::ResolvedTask> {
        self.resolved.get(handle)
    }

    /// Remove the startup resolution of `handle`, because a live entry now answers for it.
    ///
    /// The one way a resolution leaves the table, and only for a `Settled` task — a store
    /// written before terminal records existed (bn-2g3ei) — that a fresh
    /// `verification.start` re-runs (see `verification::start`). Returns what was removed.
    pub fn supersede(&mut self, handle: &TaskHandle) -> Option<super::recovery::ResolvedTask> {
        self.resolved.remove(handle)
    }

    /// Every task the startup resolution pass resolved, in identity order.
    pub fn resolved(&self) -> impl Iterator<Item = &super::recovery::ResolvedTask> {
        self.resolved.values()
    }

    /// Load a task and its continuation read back from a continuation record (bn-20142).
    ///
    /// The one door by which a startup restoration enters the live table. The frontier is
    /// held as vectors until [`Self::materialize`] reads it back through the model.
    pub fn restore(&mut self, restored: super::continuation::Restored) {
        let handle = restored.continuation.handle.clone();
        self.revisions.insert(
            handle.clone(),
            DurableRevision {
                revision: restored.revision,
                states: restored.states,
                record: restored.record,
            },
        );
        self.pending.insert(handle, restored.frontier);
        self.park(restored.continuation);
        self.put(restored.entry);
    }

    /// The frontier of the continuation `handle` names, as state vectors, or [`None`] when
    /// this table holds no such continuation.
    #[must_use]
    pub fn frontier_of(&self, handle: &ContinuationHandle) -> Option<Vec<Vec<i64>>> {
        if let Some(pending) = self.pending.get(handle) {
            return Some(pending.clone());
        }
        self.continuations
            .get(handle)
            .map(|continuation| super::continuation::vectors(&continuation.frontier))
    }

    /// Whether the continuation `handle` names still holds its frontier as vectors.
    #[must_use]
    pub fn is_pending(&self, handle: &ContinuationHandle) -> bool {
        self.pending.contains_key(handle)
    }

    /// Read a restored continuation's frontier back through `model`.
    ///
    /// A no-op for a continuation whose frontier is already states. Returns whether the
    /// continuation now holds its frontier as states. `false` means a vector is not a state
    /// of `model`, and nothing changed.
    pub fn materialize(
        &mut self,
        handle: &ContinuationHandle,
        model: &continuum_engine_reference::model::Model,
    ) -> bool {
        let Some(vectors) = self.pending.get(handle) else {
            return self.continuations.contains_key(handle);
        };
        let Some(states) = super::continuation::states_of(model, vectors) else {
            return false;
        };
        let Some(continuation) = self.continuations.get_mut(handle) else {
            return false;
        };
        continuation.frontier = states;
        self.pending.remove(handle);
        true
    }

    /// The revision the next continuation record for `handle` is published at.
    #[must_use]
    pub fn next_revision(&self, handle: &ContinuationHandle) -> u32 {
        self.revisions
            .get(handle)
            .map_or(0, |durable| durable.revision.saturating_add(1))
    }

    /// The state count the pins of `handle` name, once a record of it is durable.
    #[must_use]
    pub fn pinned_states(&self, handle: &ContinuationHandle) -> Option<u64> {
        self.revisions.get(handle).map(|durable| durable.states)
    }

    /// The store handle of the last durable continuation record for `handle`, tied to its
    /// receipt, or [`None`] when no record of it is durable.
    #[must_use]
    pub fn durable_record(
        &self,
        handle: &ContinuationHandle,
    ) -> Option<&Published<ArtifactHandle>> {
        self.revisions.get(handle).map(|durable| &durable.record)
    }

    /// Record that the continuation record for `handle` at `revision` is durable, under the
    /// store handle `record` its receipt names.
    pub fn record_revision(
        &mut self,
        handle: &ContinuationHandle,
        revision: u32,
        states: u64,
        record: Published<ArtifactHandle>,
    ) {
        self.revisions.insert(
            handle.clone(),
            DurableRevision {
                revision,
                states,
                record,
            },
        );
    }
}

/// One durable revision of a continuation record (bn-283p6).
#[derive(Debug, Clone, PartialEq, Eq)]
struct DurableRevision {
    /// The revision the record is at.
    revision: u32,
    /// The state count the continuation's pins name.
    states: u64,
    /// The record's store handle, tied to the receipt the store issued for it.
    record: Published<ArtifactHandle>,
}

// ---------------------------------------------------------------------------
// handle minting
// ---------------------------------------------------------------------------

/// An unambiguous concatenation: every part is length-prefixed, so no pair of inputs can
/// produce another pair's preimage by concatenation.
#[derive(Debug, Default)]
pub struct Preimage(Vec<u8>);

impl Preimage {
    /// The empty preimage.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append one part.
    pub fn push(&mut self, part: &[u8]) {
        self.0.extend_from_slice(&(part.len() as u64).to_be_bytes());
        self.0.extend_from_slice(part);
    }

    /// Append one textual part.
    pub fn text(&mut self, part: &str) {
        self.push(part.as_bytes());
    }

    /// The bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
}

/// Name a `task_*` handle for a preimage.
///
/// # Errors
///
/// [`Fault`] carrying [`ErrorCode::PublicationAborted`] when the identity seam cannot name
/// the record. Nothing is named under a guessed identity.
pub fn task_handle(
    identifier: &dyn ContentIdentifier,
    preimage: &Preimage,
) -> Result<TaskHandle, Fault> {
    let handle = identifier
        .identify(ArtifactClass::Task, preimage.bytes())
        .map_err(|_| identity_fault())?;
    TaskHandle::new(&handle.to_string()).map_err(|_| identity_fault())
}

/// Name a `cont_*` handle for a preimage.
///
/// # Errors
///
/// As [`task_handle`].
pub fn continuation_handle(
    identifier: &dyn ContentIdentifier,
    preimage: &Preimage,
) -> Result<ContinuationHandle, Fault> {
    let handle = identifier
        .identify(ArtifactClass::Continuation, preimage.bytes())
        .map_err(|_| identity_fault())?;
    ContinuationHandle::new(&handle.to_string()).map_err(|_| identity_fault())
}

fn identity_fault() -> Fault {
    Fault::new(
        ErrorCode::PublicationAborted,
        "no content identity could be derived for a task record",
    )
    // Deterministic: the identity is a function of the record, so a retry fails the same way.
    .not_retryable()
}

/// The nine budget dimensions, written into a preimage.
pub fn budget_preimage(preimage: &mut Preimage, budget: &Budget) {
    for dimension in [
        budget.wall_ms.value().map(|value| value.millis()),
        budget.cpu_ms.value().map(|value| value.millis()),
        budget.memory_bytes.value().map(|value| value.bytes()),
        budget.states.value().copied(),
        budget.solver_ms.value().map(|value| value.millis()),
        budget.proof_ms.value().map(|value| value.millis()),
        budget.tokens.value().copied(),
        budget.candidates.value().copied(),
        budget.bytes.value().map(|value| value.bytes()),
    ] {
        match dimension {
            Some(value) => preimage.push(&value.to_be_bytes()),
            None => preimage.text("-"),
        }
    }
}

/// The six identities an [`EpochSet`] pins beside the protocol version, written into a
/// preimage.
pub fn epochs_preimage(preimage: &mut Preimage, epochs: &EpochSet) {
    for identity in [
        epochs.semantic.value(),
        epochs.intent.value(),
        epochs.evidence.value(),
        epochs.proof.value(),
        epochs.corpus.value(),
        epochs.engine.value(),
    ] {
        match identity {
            Some(identity) => preimage.text(identity.as_str()),
            None => preimage.text("-"),
        }
    }
}

// ---------------------------------------------------------------------------
// the family
// ---------------------------------------------------------------------------

/// The `task` namespace's five operations.
#[derive(Debug, Clone, Copy, Default)]
pub struct TaskFamily;

/// Every `(operation, code)` pair this family can answer with.
///
/// A data table rather than a comment, so `tests/daemon_task_operations.rs` can hold every
/// one of them to `rule errors.common` ∪ the operation's `errors` clause instead of trusting
/// that the handlers stayed inside it.
pub const FAULTS: &[(&str, ErrorCode)] = &[
    ("task.status", ErrorCode::CapabilityDenied),
    ("task.cancel", ErrorCode::CapabilityDenied),
    ("task.cancel", ErrorCode::PublicationAborted),
    ("task.resume", ErrorCode::CapabilityDenied),
    ("task.resume", ErrorCode::StaleSnapshot),
    ("task.resume", ErrorCode::ContinuationEpochMismatch),
    ("task.resume", ErrorCode::EpochUnsupported),
    ("task.resume", ErrorCode::PublicationAborted),
    ("task.subscribe", ErrorCode::CapabilityDenied),
    ("task.update_budget", ErrorCode::CapabilityDenied),
    ("task.update_budget", ErrorCode::PublicationAborted),
];

impl OperationFamily for TaskFamily {
    fn namespace(&self) -> &'static str {
        "task"
    }

    fn scope(&self, arguments: &Arguments) -> ScopeClaim {
        // A task operation claims the task class and the one `task_*` or `cont_*` instance
        // it names, which `CapabilityDescriptor.instances` decides (3.7,
        // `rule capability.instance_scope`). The snapshot the task runs over is deliberately
        // *not* claimed here: this function is pure in the arguments and is given no state,
        // so resolving a task handle to the snapshot behind it would be the store lookup X3
        // forbids before admission. The handler re-reads that binding afterwards, where a
        // lookup is allowed. For the same reason `task.resume` claims only the continuation
        // it names and not the task that continuation belongs to: a `task_` instance scope
        // does not cover the task's `cont_` (the rule's derived-artifact clause).
        //
        // `task.cancel` and `task.update_budget` also claim the continuation class: on a
        // parked task each publishes the next revision of its continuation record
        // (bn-20142).
        let task = ArtifactClass::Task.token();
        let claim = |handle: &str| ScopeClaim {
            snapshots: Vec::new(),
            intents: Vec::new(),
            classes: vec![task],
            instances: vec![handle.to_owned()],
        };
        match arguments {
            Arguments::TaskStatus(request) => claim(request.task.as_str()),
            Arguments::TaskSubscribe(request) => claim(request.task.as_str()),
            // The continuation these two write is not named, so its instance is decided in
            // the handler, after the lookup admission may not make ([`continuation_in_scope`]).
            Arguments::TaskCancel(request) => ScopeClaim {
                classes: vec![task, ArtifactClass::Continuation.token()],
                ..claim(request.task.as_str())
            },
            Arguments::TaskUpdateBudget(request) => ScopeClaim {
                classes: vec![task, ArtifactClass::Continuation.token()],
                ..claim(request.task.as_str())
            },
            Arguments::TaskResume(request) => ScopeClaim {
                classes: vec![task, ArtifactClass::Continuation.token()],
                ..claim(request.continuation.as_str())
            },
            _ => ScopeClaim::default(),
        }
    }

    fn handle(
        &self,
        call: &Call<'_>,
        state: &mut DaemonState,
        services: &Services,
        store: &ReferenceStore,
    ) -> Result<Effect, Fault> {
        match call.arguments {
            Arguments::TaskStatus(request) => status(call, request, state, services),
            Arguments::TaskCancel(request) => cancel(call, request, state, services, store),
            Arguments::TaskResume(request) => resume(call, request, state, services, store),
            Arguments::TaskSubscribe(request) => subscribe(call, request, state),
            Arguments::TaskUpdateBudget(request) => {
                update_budget(call, request, state, services, store)
            }
            // Unreachable: the dispatcher checked shape agreement against the registry
            // before routing. A typed refusal rather than an `unreachable!`, because a
            // daemon does not abort on its own invariant.
            _ => Err(Fault::new(
                ErrorCode::MalformedRequest,
                "the request body is not the shape this operation declares",
            )),
        }
    }
}

/// `task.status` — project the entry onto the wire, inside the ceiling the caller stated.
///
/// `errors []`, so the codes available are `rule errors.common`'s five and no more. A task
/// the daemon does not hold is [`Fault::denied`], byte-identical with every other denial:
/// RFC 0027 X2 forbids a distinguishable not-found, and there is no `UnknownTask` code to
/// build one out of.
///
/// # The ceiling, and why it is read here rather than at the envelope
///
/// `OutputPolicy.max_bytes` is "the enforced contract" (RFC 0026) and until bn-6fuu5 no
/// handler enforced it. [`output::fit_task_record`] does, and it is called *here* because
/// what a ceiling may legally take out of an answer is a property of that answer's declared
/// shape: `TaskRecord`'s `required` members are on every conforming answer whatever a caller
/// asks for, and only the operation that knows the shape can say which values may be smaller.
/// [`output`](super::output) states the whole rule — the elision order, the floor, the typed
/// refusal below it, and the two questions it does not answer.
///
/// A caller that states no ceiling is answered exactly as before, without a byte being
/// measured: [`output::Ceiling::of`] reads the envelope and [`output::fit_task_record`]
/// returns early. That is why this change moves no number in the landed benchmark matrix.
///
/// The manifest is *extended*, never replaced. [`TaskEntry::omissions`] says what this
/// deployment could not produce (`unsupported`); the fitting says what this ceiling took
/// (`budget`, with `recoverable_by` naming the task). Two different statements about two
/// different things, and `OmissionReason` is the vocabulary that keeps them apart.
fn status(
    call: &Call<'_>,
    request: &TaskStatusRequest,
    state: &DaemonState,
    services: &Services,
) -> Result<Effect, Fault> {
    let entry = state.tasks().get(&request.task).ok_or_else(Fault::denied)?;
    task_scope(call, entry)?;
    let fitted = output::fit_task_record(
        entry.record(),
        output::Ceiling::of(call.envelope),
        services.negotiated().encoding(),
    )?;
    let mut effect = reported(Payload::TaskStatus(fitted.record), Nullable::Null, entry);
    effect.omissions.extend(fitted.omissions);
    Ok(effect)
}

/// `task.subscribe` — the record now; the events are the recorded transitions.
///
/// > `task.subscribe` streams progress events; they are hints. Committed artifacts and
/// > `task.status` are authoritative. […] A dropped subscription changes nothing (INV-002);
/// > a client MUST be able to recover the same state by re-reading.
/// >
/// > — `rule subscription.hints_only`
///
/// The IDL's response body for this operation is exactly one field — "snapshot of the record
/// at subscription time; events follow" — and that is what this layer answers, because a
/// *stream* is a transport object and this layer emits no wire bytes. The events that follow
/// are [`TaskEntry::events`], recorded as the transitions happened; a transport replays them
/// and, by the rule above, a client that observes none of them recovers the same state from
/// `task.status`.
///
/// So the operation is **served, not refused**. The bn-i4aem finding — that `errors []` and
/// `rule errors.unsupported_surface` contradicted each other for the three operations
/// declaring an empty clause and the twenty-two more that never named the code — does not
/// bite here, and did not before it was paid: that rule is about an operation "registered
/// ahead of its producing subsystem", and the task table is not a missing subsystem.
/// `task.subscribe` returns only common codes, which is what its empty clause permits, and
/// the `unsupported_surface` path is not taken. The contradiction itself is gone as of
/// protocol 3.2 (bn-i4aem item 1).
fn subscribe(
    call: &Call<'_>,
    request: &TaskSubscribeRequest,
    state: &DaemonState,
) -> Result<Effect, Fault> {
    let entry = state.tasks().get(&request.task).ok_or_else(Fault::denied)?;
    task_scope(call, entry)?;
    Ok(reported(
        Payload::TaskSubscribe(TaskSubscribeResponse {
            record: entry.record(),
        }),
        Nullable::Null,
        entry,
    ))
}

/// Every handle a task names beyond itself — its snapshot, its intent, and the continuation
/// it holds — decided against the grant before anything is read about it, written to it, or
/// reported (`rule capability.instance_scope`, the derived-handle clause; cr-3hcpn4).
///
/// Admission saw only the task handle, because finding what the task names is a table
/// lookup X3 keeps out of admission. Every operation that resolves a `task_` calls this
/// first: `task.status` and `task.subscribe` report all three in the record,
/// `task.cancel` and `task.update_budget` report the continuation and, on a parked task,
/// publish its next revision (bn-20142), and `verification.result` and
/// `verification.await` report the continuation. The refusal is the same
/// `CapabilityDenied` as every other denial (X1, X2), for a terminal task as for a live one.
/// A grant that scopes none of those classes and lists no snapshot or intent passes
/// unchanged, which is the 3.6 behaviour.
pub(super) fn task_scope(call: &Call<'_>, entry: &TaskEntry) -> Result<(), Fault> {
    if let Nullable::Value(snapshot) = &entry.snapshot {
        call.derived(Derived::Snapshot(snapshot))?;
    }
    if let Nullable::Value(intent) = &entry.intent {
        call.derived(Derived::Intent(intent))?;
    }
    if let Some(continuation) = &entry.continuation {
        call.derived(Derived::Instance(continuation.as_str()))?;
    }
    Ok(())
}

/// Every handle a continuation names beyond itself, decided against the grant
/// ([`Call::derived`]).
fn continuation_scope(call: &Call<'_>, continuation: &Continuation) -> Result<(), Fault> {
    call.derived(Derived::Instance(continuation.task.as_str()))?;
    call.derived(Derived::Snapshot(&continuation.snapshot))?;
    if let Nullable::Value(intent) = &continuation.intent {
        call.derived(Derived::Intent(intent))?;
    }
    Ok(())
}

/// `task.cancel` — request, drain, finalize, driven through the region that owns the work.
///
/// > `task.cancel` triggers request, drain, finalize. It MUST leave either committed partial
/// > evidence plus a valid continuation, or nothing published (INV-009, plan B19).
/// > Cancellation MUST NOT truncate a publication in progress (INV-017).
/// >
/// > — `rule task.cancel_correct`
///
/// # What is torn down, and why it is a scope of its own
///
/// A dispatch holds `&mut DaemonState` for its whole extent, so when this operation runs
/// there is no *live* execution of the task to interrupt: the dispatch that ran the campaign
/// finalized its own region before it returned (see [`region`](super::region)). What the
/// cancel tears down is the task's **residual** work — the evidence it has committed and the
/// possibility of resuming from it — and it does so in a region of its own, spawned with the
/// two facts the outcome depends on: how much this task has published
/// ([`TaskEntry::publications`]) and whether it holds a continuation to resume from.
///
/// That is not ceremony, and the difference it makes is checkable rather than rhetorical.
/// The three arms of `rule task.cancel_correct` are
/// [`CancelOutcome`](continuum_task::region::worker::CancelOutcome), which has **no
/// constructor** for "committed evidence and no continuation" and none for "a continuation
/// and nothing committed" — the first is the leaked-obligation shape G0-DX-14 exists to
/// catch and the second is a resume pointer into nothing. So this function does not decide
/// which arm it is on; it states two facts, the layer that owns the rule decides, and the
/// daemon supplies the `cont_*` handle for the arm that carries one. The region-layer
/// continuation is bookkeeping and never reaches the wire (see [`region`](super::region)).
///
/// The honest limit is worth stating too: at this grain the drain has no in-flight
/// publication to close, so what this shape prevents is a *reporting* leak rather than a
/// runtime one. The runtime half is prevented by the borrow, and by the region bracket that
/// finalizes the campaign's own scope inside the dispatch that opened it.
///
/// Cancelling a task that is already terminal changes nothing and says so with
/// [`StructuralOutcome::Unchanged`]; `rule task.status_monotonic`'s "a terminal status never
/// changes" is the reason, and [`TaskEntry::advance`] is where it is enforced. Its scope is
/// still opened and torn down, because "what did this task leave behind" is the same
/// question whichever answer the status gives, and asking it in one place is what keeps the
/// rows from drifting.
fn cancel(
    call: &Call<'_>,
    request: &TaskCancelRequest,
    state: &mut DaemonState,
    services: &Services,
    store: &ReferenceStore,
) -> Result<Effect, Fault> {
    let now = services.now().cloned();
    let (residual, terminal) = {
        let entry = state.tasks().get(&request.task).ok_or_else(Fault::denied)?;
        task_scope(call, entry)?;
        (
            Residual {
                publications: entry.publications(),
                resumable: entry.continuation.is_some(),
            },
            entry.is_terminal(),
        )
    };
    // The durable half (bn-20142). A parked task's cancel is published as the next revision
    // of its continuation record *before* the status moves, so a restart restores the task
    // `Cancelled` and never `Suspended` again ("a terminal status never changes"). An abort
    // answers `PublicationAborted` and changes nothing.
    let durable = if terminal {
        None
    } else {
        continuation::current(state.tasks(), &request.task, ParkState::Cancelled).map(
            |mut record| {
                continuation::reach(&mut record, MILESTONE_CANCELLED, now.as_ref());
                record
            },
        )
    };
    let durable = match durable {
        Some(record) => {
            let publisher = super::identity::capability_to_store(&call.envelope.capability)
                .map_err(|_| Fault::denied())?;
            let published =
                continuation::publish(store, &publisher, services.identifier(), &record)?;
            Some((record, published))
        }
        None => None,
    };
    let task = request.task.clone();
    let (moved, settlement) = region::scoped(
        state,
        region::resumability(residual.resumable),
        |state, scope| {
            let entry = state.tasks_mut().get_mut(&task).ok_or_else(Fault::denied)?;
            let moved = entry.advance(TaskStatus::Cancelled, now.as_ref());
            if moved {
                entry.reach(MILESTONE_CANCELLED, now.as_ref());
            }
            residual.admit(state.regions_mut(), scope);
            Ok(moved)
        },
    )?;
    if let Some((record, published)) = durable {
        continuation::settle(state.tasks_mut(), &record, published);
    }

    let entry = state.tasks().get(&request.task).ok_or_else(Fault::denied)?;
    let outcome = if moved {
        StructuralOutcome::Cancelled
    } else {
        StructuralOutcome::Unchanged
    };
    // The arm is the region layer's; the handle is this daemon's. `and` is the join: an arm
    // carrying a continuation reports the `cont_*` the task holds, and every other arm
    // reports `null` — the *named* "cancelled with nothing published" outcome, never an
    // absent field a client has to interpret.
    let continuation = match settlement.continuation().and(entry.continuation.clone()) {
        Some(handle) => Nullable::Value(handle),
        None => Nullable::Null,
    };
    debug_assert_eq!(
        settlement.continuation().is_some(),
        entry.continuation.is_some(),
        "the region's cancellation arm and the task's continuation are two readings of one \
         fact and MUST agree"
    );
    let payload = Payload::TaskCancel(TaskCancelResponse {
        task: entry.handle.clone(),
        status: entry.status,
        continuation,
        committed_evidence: entry.committed_evidence.clone(),
    });
    let omissions = entry.omissions();
    let artifacts = published(entry);
    let handle = entry.handle.clone();
    let mut effect = Effect::new(payload, structural(outcome))
        .observing(handle)
        .with_artifacts(artifacts);
    effect.omissions = omissions;
    Ok(effect)
}

/// What a cancellation finds when it reaches a task: what the task published, and whether
/// any of it can be resumed.
///
/// The two facts [`CancelOutcome`](continuum_task::region::worker::CancelOutcome) is
/// computed from, read off the task table once and written into the scope's worker. Both are
/// *the task's*, not the request's: a caller cannot make a task resumable by asking it to be.
#[derive(Debug, Clone, Copy)]
struct Residual {
    publications: u32,
    resumable: bool,
}

impl Residual {
    /// Re-establish this task's committed evidence inside the scope that is about to be
    /// cancelled.
    ///
    /// Each publication is a `Reserve` followed immediately by its `Commit`, which is the
    /// only order the region layer admits and the only one that is true of what happened:
    /// nothing is staged at rest, so there is no in-flight publication for the cancellation
    /// to truncate (INV-017), and the drain finds committed evidence exactly where the task
    /// table says there is some.
    fn admit(self, regions: &mut TaskRegions, scope: Scope) {
        regions.step(scope, WorkerStep::Begin);
        for _ in 0..self.publications {
            regions.step(scope, WorkerStep::Reserve);
            regions.step(scope, WorkerStep::Commit);
        }
    }
}

/// `task.update_budget` — re-admit a parked task under a larger bound, with no identity
/// churn.
///
/// > Raising a dimension extends the current run. Lowering a dimension below committed spend
/// > triggers suspension-with-continuation (plan B18) […] Silent truncation of a campaign is
/// > prohibited (INV-009).
/// >
/// > — `rule task.update_budget`
///
/// At this grain there is no run to extend *while it runs*: a campaign either closed inside
/// the dispatch that started it or parked. So "raising a dimension extends the current run"
/// is served in two steps a caller can see — this operation records the new budget and
/// leaves the task `Suspended` with its continuation intact, and `task.resume` is what
/// re-runs under it. **The task identity does not change**, which is the PR 5 exit's second
/// clause: the budget is a field of the entry and never re-enters the handle's preimage.
///
/// # The five arms, and the one that could not be computed before
///
/// [`budget::update`] applies the request's ceilings to the task's [`BudgetLedger`] one
/// dimension at a time and returns [`UpdateOutcome`] for each — `rule task.update_budget`'s
/// legality table as nine values in SD-12 declaration order:
///
/// | outcome | what it means here |
/// |---|---|
/// | `Raised` | the ceiling rose or was removed; the next `task.resume` runs under it |
/// | `Unchanged` | a re-sent update changes nothing (INV-002) |
/// | `Tightened` | the ceiling fell and still admits the spend already recorded |
/// | `Suspend` | it fell **below** committed spend: park with committed partial evidence plus a continuation (B18) |
/// | `Unenforced` | nothing meters the dimension, so the ceiling is recorded and stays a typed omission |
///
/// The fourth arm is the one bn-1gc wrote down as unreachable from here: the handler had no
/// committed-spend figure to compare a lowered ceiling against, so a lowering was applied as
/// if it were a tightening, the engine bound shrank below what the parked run had already
/// explored, and the next `task.resume` came back with a *smaller* campaign — the silent
/// truncation INV-009 prohibits, arriving as a monotone `cost.states` going down. The
/// ledger supplies the figure now, so the arm is computed, and what it does is documented
/// where it is enforced: [`budget::bounds_of`] floors the engine's state bound at recorded
/// spend, so the withdrawn ceiling is *recorded and not a bound*. Nothing is un-explored and
/// nothing is published that was not published before.
///
/// A parked task is already `Suspended` with its continuation, which is what makes the arm
/// expressible without a status transition: B18's "transitions to `Suspended` with committed
/// partial evidence plus a valid continuation" is a state this task is already in, and
/// `rule task.status_monotonic` is not disturbed by an operation that re-states it. The
/// response's `continuation` field — "present when lowering the budget suspended the task" —
/// is where the caller reads it, and on this arm it is exactly what the field was declared
/// for.
///
/// A terminal task keeps the budget it ran under and answers
/// [`StructuralOutcome::Unchanged`]: a terminal task's budget is a historical fact, and
/// rewriting it would make its recorded cost unreadable.
///
/// **`task.resume`'s optional budget is held to the same rule by the same reading** — its
/// terminal check sits above its ledger write, so the two operations produce one observable
/// for one ceiling on one terminal task: the ledger untouched, no milestone appended, no
/// event emitted, the record byte-identical. They disagreed until bn-10093; the disagreement
/// is what bn-3p32's A1 and bn-1kp6's attack 18 pinned, and both now guard the agreement.
fn update_budget(
    call: &Call<'_>,
    request: &TaskUpdateBudgetRequest,
    state: &mut DaemonState,
    services: &Services,
    store: &ReferenceStore,
) -> Result<Effect, Fault> {
    let now = services.now().cloned();
    // The durable half (bn-20142), before the ledger changes: the next revision of the
    // parked task's continuation record, carrying the new ceilings and the milestones this
    // update records. An abort answers `PublicationAborted` and changes nothing. A terminal
    // task writes nothing, here as below.
    let durable = {
        let entry = state.tasks().get(&request.task).ok_or_else(Fault::denied)?;
        task_scope(call, entry)?;
        if entry.is_terminal() {
            None
        } else {
            let suspends = budget::would_suspend(&entry.ledger, &request.budget);
            continuation::current(state.tasks(), &request.task, ParkState::Suspended).map(
                |mut record| {
                    record.ceilings = request.budget.clone();
                    continuation::reach(&mut record, MILESTONE_BUDGET_UPDATED, now.as_ref());
                    if suspends {
                        continuation::reach(&mut record, MILESTONE_BUDGET_SUSPENDED, now.as_ref());
                    }
                    record
                },
            )
        }
    };
    let durable = match durable {
        Some(record) => {
            let publisher = super::identity::capability_to_store(&call.envelope.capability)
                .map_err(|_| Fault::denied())?;
            let published =
                continuation::publish(store, &publisher, services.identifier(), &record)?;
            Some((record, published))
        }
        None => None,
    };
    let entry = state
        .tasks_mut()
        .get_mut(&request.task)
        .ok_or_else(Fault::denied)?;
    let outcome = if entry.is_terminal() {
        StructuralOutcome::Unchanged
    } else {
        let projected = budget::would_suspend(&entry.ledger, &request.budget);
        let outcomes = budget::update(&mut entry.ledger, &request.budget);
        debug_assert_eq!(
            projected,
            outcomes
                .iter()
                .any(|outcome| outcome.suspension().is_some()),
            "`budget::would_suspend` restates the ledger's own arm order"
        );
        entry.reach(MILESTONE_BUDGET_UPDATED, now.as_ref());
        if outcomes
            .iter()
            .any(|outcome| outcome.suspension().is_some())
        {
            // B18. The park is a fact about the *run*, not a transition: the task is
            // already `Suspended` and holds the continuation the response reports, and the
            // committed partial evidence it parks from is `entry.evidence`, unchanged and
            // still named. The milestone is what makes the arm legible in
            // `TaskRecord.milestones` rather than only in the response's `continuation`.
            entry.reach(MILESTONE_BUDGET_SUSPENDED, now.as_ref());
            debug_assert!(
                entry.status == TaskStatus::Suspended || entry.evidence.count() == 0,
                "a lowering below committed spend parks a task that has committed something"
            );
        }
        StructuralOutcome::Updated
    };
    if let Some((record, published)) = durable {
        continuation::settle(state.tasks_mut(), &record, published);
    }
    let entry = state.tasks().get(&request.task).ok_or_else(Fault::denied)?;
    let payload = Payload::TaskUpdateBudget(TaskUpdateBudgetResponse {
        task: entry.handle.clone(),
        status: entry.status,
        budget: entry.budget(),
        // "Present when lowering the budget suspended the task." A task that is not terminal
        // here is already suspended-with-continuation, so the field reports the continuation
        // that makes the suspension resumable — never an absent field standing in for one.
        continuation: optional(entry.continuation.clone()),
    });
    // The `Unenforced` arm's wire surface (bn-23j7s). An operation that *accepts* a budget
    // owes INV-007's manifest for it — which is what `verification.start` has always done —
    // and this operation accepting a ceiling nothing meters, then saying nothing, was the
    // arm with no way to be seen. Derived from the ledger, so the two operations report one
    // list computed one way.
    let omissions = [entry.omissions(), budget::omissions_of(&entry.ledger)].concat();
    // `task.update_budget` is `@mutation` and NOT `@task_starting`, so it never lands on
    // the `task_suspended` lane however the task is parked: only a `@task_starting`
    // operation MAY report that status. A task that parked again under the new budget is
    // reported through this operation's own `continuation` response field, and the
    // dispatcher's check on the lane is what keeps the two readings from drifting.
    let artifacts = published(entry);
    let handle = entry.handle.clone();
    let mut effect = Effect::new(payload, structural(outcome))
        .observing(handle)
        .with_artifacts(artifacts);
    effect.omissions = omissions;
    Ok(effect)
}

/// `task.resume` — the admissibility predicate, then one more bounded run.
///
/// # The predicate, and why each answer is the code it is
///
/// RFC 0026's resume decision table, in the order this function checks it:
///
/// | Condition | Error |
/// |---|---|
/// | the continuation is not one this daemon holds | `CapabilityDenied` (X2: no distinguishable not-found) |
/// | the envelope names a snapshot other than the one the continuation pinned | `StaleSnapshot` |
/// | the pinned snapshot is not held at all | `CapabilityDenied` (X2 — see below) |
/// | the pinned snapshot is held but is not sealed, or has been superseded in its lineage | `StaleSnapshot` |
/// | a pinned epoch names a kind the daemon pins no identity for | `EpochUnsupported` |
/// | a pinned epoch disagrees with the daemon's current epoch of that kind (P1) | `ContinuationEpochMismatch` |
/// | the pinned engine identity disagrees with the daemon's (P2) | `ContinuationEpochMismatch` |
/// | the model the continuation names is no longer one this daemon can construct | `UnsupportedSemanticFeature` (3.2; see the comment at the guard) |
///
/// **Every row is a guard in this function, and every guard runs before the first write**
/// (bn-3oocz). The last row used to be raised inside `verification::run_in`, after the
/// request budget reached the ledger and after `verification::advance` opened a region and
/// stamped `TaskEntry::region`/`worker`. The refusal was typed and correct, but it was not
/// zero-trace, so G1-04's "validates … inputs before any reuse" held for twelve classes and
/// not for this one. `gate_g1_04_acceptance.rs` measures it with a total structural
/// fingerprint of the effect surface.
///
/// **The third row is bn-10wdo's disposition of a doc/code disagreement the table itself
/// used to carry**, and the row above is the aligned reading, not a behaviour change: the
/// code has always looked the pinned snapshot up with `state.workspace(..).ok_or_else(
/// Fault::denied)?`, the same "a handle this daemon does not hold is `Fault::denied`, not a
/// not-found" rule `daemon::workspace`'s module documentation states and every family
/// applies to a wholly unheld handle. A prior revision of this table folded "not held" into
/// the same row as "not sealed" and "superseded", both of which *are* `StaleSnapshot` because
/// both presuppose a record the daemon actually has — RFC 0026's own definition, "the named
/// snapshot is not current, or is not sealed where sealing is required", is a statement about
/// a snapshot the daemon can name, not one it cannot find at all. The path is unreachable via
/// wire today (no operation in the registry deletes or evicts a `WorkspaceRecord` once a
/// continuation has pinned it, so a resume can find its pinned snapshot missing only if a
/// future admin surface adds one), so no wire audit exercises either row and this correction
/// is doc-only.
///
/// A continuation whose *task* is terminal is not in that table, because it is not a
/// refusal: it is answered, with the terminal status, and nothing is written. That answer is
/// produced **before** the request's optional budget reaches the ledger, so a terminal task's
/// budget arm is the no-op [`update_budget`] states it is — see the terminal check below.
///
/// **Both epoch predicates are checked, and neither implies the other** (RFC 0026, "The
/// two-predicate obligation"): P1 alone would admit a resume onto a different engine build,
/// which is the case plan §4.7's defect lifecycle exists to catch, and P2 alone a resume
/// across a semantic-epoch advance, which ADR-0018 forbids. **The protocol epoch does not
/// participate** — it is not a field of [`PinnedEpochs`] at all, so that rule is a property
/// of the type rather than a step somebody has to remember.
///
/// The snapshot test is the R3 spike §3's proven behaviour — "rejection of continuation
/// under a different workspace snapshot" — and it reads the *envelope*'s `snapshot`, because
/// `task.resume`'s request body carries a continuation and a budget and nothing else. Naming
/// no snapshot says nothing and is admitted; naming a different one is refused.
///
/// # What "resume" does at a synchronous grain, and why it is not a silent re-run
///
/// `bfs::explore` takes a model and bounds and has no partial-state entry point, so resuming
/// means exploring the same model again under the larger bound. That is *not* the "quietly
/// restart the task under current epochs" RFC 0026 forbids, and the difference is checkable
/// rather than asserted: breadth-first exploration admits states in canonical order, so the
/// parked explored set is a **prefix** of the resumed one and every parked frontier state is
/// discovered again. The run is under the *pinned* epochs, because the predicate above has
/// already refused every case where the daemon's differ. `tests/daemon_task_operations.rs`
/// asserts the prefix property instead of trusting it, and so, on the production path itself,
/// does [`verification::run`](super::verification::run)'s `debug_assert` — `resume` passes
/// this `continuation`'s pinned `frontier` through to it rather than reading it here, which is
/// bn-10wdo's disposition of a DX-03 concern: a `cont_*` handle means "advance this task", not
/// "resume from this point", `Continuation::bounds`/`frontier` are pinned **provenance**
/// (RFC 0030's budget rule), and this daemon's one engine happens to re-derive them rather than
/// consume them as instructions. See RFC 0026, "What a `cont_*` handle means, and what
/// `bounds`/`frontier` are pinned for", for the full disposition and the non-prefix guard it
/// requires of any future engine binding.
fn resume(
    call: &Call<'_>,
    request: &TaskResumeRequest,
    state: &mut DaemonState,
    services: &Services,
    store: &ReferenceStore,
) -> Result<Effect, Fault> {
    // As `verification.start`: the resumed run's campaign record is published under the
    // caller's own capability, so the store decides and audits the durable write.
    let publisher = super::identity::capability_to_store(&call.envelope.capability)
        .map_err(|_| Fault::denied())?;
    let continuation = state
        .tasks()
        .continuation(&request.continuation)
        .ok_or_else(Fault::denied)?
        .clone();
    // The continuation names a task, a snapshot, and perhaps an intent the request did not.
    // Each is decided by the grant before anything is read about it, written to it, or
    // reported: a grant listing `cont_B` and not `task_B` cannot resume, observe, or
    // re-budget `task_B`, terminal or live (cr-3hcpn4). The refusal precedes the staleness
    // checks too, whose recovery offers would otherwise name the task's lineage head.
    continuation_scope(call, &continuation)?;

    if let Nullable::Value(named) = &call.envelope.snapshot {
        if named != &continuation.snapshot {
            return Err(stale());
        }
    }

    let record = state
        .workspace(&continuation.snapshot)
        .ok_or_else(Fault::denied)?;
    if !record.sealed() {
        return Err(stale().with_recovery(super::workspace::reseal_current_head(
            state,
            &record.lineage,
        )));
    }
    let lineage_name = record.lineage.clone();
    let head = record.descriptor.source().identity().clone();
    let lineage = state.lineage(&lineage_name).ok_or_else(Fault::denied)?;
    // Both `LineageError` arms collapse to `stale()` here, deliberately including `Unknown`:
    // `record` above is already a snapshot this daemon fully resolved, so there is no
    // existence-oracle question left, only a currency one — RFC 0026's "An unplaceable lineage
    // identity" disposition (bn-10wdo). See `daemon::workspace`'s module doc for the paired
    // half: the same `Unknown` arm reached validating a caller-declared identity about to be
    // spent on a write answers `CapabilityDenied` there, and both are ratified, not aligned.
    //
    // A `Stale` refusal names the head that superseded the pinned snapshot, as a recovery
    // offer (RFC 0027 H8, bn-27mx7). The continuation itself cannot be re-based — it pins
    // the world it ran against — so the offer is the step a fresh campaign over the head
    // needs, not a resume.
    check_current(lineage, &head).map_err(|error| match error {
        LineageError::Stale(superseded) => stale().with_recovery(super::workspace::reseal_at_head(
            state,
            &lineage_name,
            superseded.current(),
        )),
        LineageError::Unknown(_) => stale(),
    })?;

    admissible_epochs(&continuation.pinned, services.epochs())?;

    let task = continuation.task.clone();

    // A terminal task is not resumed, and — **this test is above the budget write, not below
    // it** — a terminal task is not re-budgeted either. `rule task.status_monotonic` says a
    // terminal status never changes and `rule task.resume` says a resume "MUST NOT replace
    // prior artifacts under the same identity"; a no-op does neither, and returning the
    // terminal status is the honest answer to "resume this". There is no code in this
    // operation's union for "already terminal", and pressing one of the common five into
    // that service would name something false.
    //
    // The order is the whole of bn-10093 (bn-3p32's A1, bn-1kp6's attack 18). With the
    // ledger write above this return, `task.resume` performed the exact rewrite
    // [`update_budget`] refuses for a stated reason — "a terminal task's budget is a
    // historical fact, and rewriting it would make its recorded cost unreadable" — so one
    // ceiling, one cancelled task and one daemon were refused through one operation and
    // accepted through the other, and a `Cancelled` record could gain a
    // `task.budget_suspended` milestone claiming a dead task parked. Two operations that
    // disagree about one rule are two rules; this return is what makes them one.
    //
    // The refusal produces **`update_budget`'s own observable**, not a new one: the ledger
    // is untouched, no milestone and no `TaskEvent` is appended, and the answer is the
    // terminal status. So `cancel` → `resume(continuation, budget)` now leaves the record
    // byte-identical, which is exactly what `cancel` → `resume(continuation, no budget)`
    // always did (bn-3p32's A2, the localising control this repair must not step past).
    //
    // `reported(...)` below answers on the task-observing lane (`status = ok`), never
    // `task_started` — this operation is `@task_starting` and could report either, and this
    // is the one place the choice matters: a terminal task is not being started, so `ok` is
    // the honest lane. `verification::start` faces the identical "identity resolves to an
    // already-terminal task" case (bn-3p32's A12) and, as of protocol 3.4, answers it the
    // same way: its own terminal short-circuit (`daemon::verification::terminal`) mirrors
    // this one, which is F20 paid (bn-3jrtz) — the asymmetry bn-y9f7i's disposition
    // recorded ("What `task_started` means when an identity resolves to a task that will
    // not run again") is closed, and one fact is spelled one way by both operations that
    // can reach it.
    {
        let entry = state.tasks().get(&task).ok_or_else(Fault::denied)?;
        if entry.is_terminal() {
            return Ok(reported(
                Payload::TaskResume(TaskResumeResponse {
                    task: entry.handle.clone(),
                    status: entry.status,
                }),
                Nullable::Null,
                entry,
            ));
        }
    }

    // Model availability: the last row of the decision table, and the last guard before the
    // first write. It is below the terminal answer on purpose: a terminal task is not run,
    // so the model it names is not an input to that answer, and the terminal answer stays
    // what it always was for a task whose model left the catalog.
    //
    // The code is `verification.start`'s code for the same condition, so the two operations
    // agree about one fact. Until protocol 3.2 `task.resume`'s `errors` clause did not admit
    // `UnsupportedSemanticFeature`, and "the model this continuation named is no longer one
    // this daemon can construct" collapsed to the one denial — the workaround bn-18z
    // recorded as wire defect (7). `rule errors.common` admits it for every operation as of
    // 3.2, so the honest answer reaches the wire. The collapse stays for any code outside
    // the union, and the check is registry data, so it tracks the IDL by construction.
    //
    // It is not a new existence oracle. Reaching this line already required holding a
    // continuation this daemon has, and a *successful* resume was always distinguishable
    // from a denial. RFC 0027 X2 is about a caller out of scope for an artifact, and such a
    // caller never reaches a handler.
    //
    // A continuation restored at startup (bn-20142) holds its frontier as vectors, and they
    // are read back into the model's states here, still before the first write: a vector
    // that is not a state of the model the continuation names is the same condition — that
    // model is not one this daemon can construct — and answers the same code.
    {
        let entry = state.tasks().get(&task).ok_or_else(Fault::denied)?;
        let model = state.models().get(&entry.model).cloned();
        let readable =
            model.is_some_and(|model| state.tasks_mut().materialize(&request.continuation, &model));
        if !readable {
            let fault = verification::no_model();
            return Err(if super::errors::admits(call.spec, fault.code) {
                fault
            } else {
                Fault::denied()
            });
        }
    }

    // The same run, refused before for this capability on the continuation it would mint,
    // is refused again before the budget write or any exploration. The run is decided by
    // the continuation it starts from and the bounds the budget write would leave
    // (cr-3hcpn4).
    let refused_key = {
        let entry = state.tasks().get(&task).ok_or_else(Fault::denied)?;
        RefusedRun {
            capability: call.envelope.capability.clone(),
            task: task.clone(),
            from: Some(request.continuation.clone()),
            bounds: budget::bounds_after(&entry.ledger, request.budget.value()),
        }
    };
    if state.tasks().refused(&refused_key) {
        let mut denied = Fault::denied();
        denied.derived_denial = true;
        return Err(denied);
    }
    // What the budget write below changes, so a refused run can put it back: the ceilings
    // and the milestone and event logs `reach` appends to.
    let (prior_budget, prior_milestones, prior_events) = {
        let entry = state.tasks().get(&task).ok_or_else(Fault::denied)?;
        (entry.budget(), entry.milestones.len(), entry.events.len())
    };

    if let Optional::Present(budget) = &request.budget {
        if let Some(entry) = state.tasks_mut().get_mut(&task) {
            // `task.resume` carries an optional budget and RFC 0026 gives it the same
            // legality table `task.update_budget` has, so it goes through the same ledger
            // call rather than through a second write of the two fields the ledger replaced.
            //
            // The outcomes are not read here, and that is not a shortcut: this operation's
            // answer is the *run*, not the update, so the arm a dimension landed on has no
            // field on `TaskResumeResponse` to reach. What the `Suspend` arm does is enforced
            // where it has to be — [`budget::bounds_of`] floors the engine's state bound at
            // recorded spend, so a resume under a withdrawn ceiling makes no further progress
            // and un-explores nothing, rather than quietly re-running the campaign smaller.
            let suspended = budget::update(&mut entry.ledger, budget)
                .iter()
                .any(|outcome| outcome.suspension().is_some());
            if suspended {
                entry.reach(MILESTONE_BUDGET_SUSPENDED, services.now());
            }
        }
    }
    // Read *after* the write, so a raised ceiling extends this resume rather than the next
    // one — the order the budget arm always had, and the reason the terminal test above it
    // is a reordering of two guards rather than of the run itself.
    let bounds = state.tasks().get(&task).ok_or_else(Fault::denied)?.bounds();
    let frontier = state
        .tasks()
        .continuation(&request.continuation)
        .map(|continuation| continuation.frontier.clone())
        .unwrap_or_default();

    // The run's own refusals are the `verification` family's, and `task.resume` declares a
    // narrower `errors` clause than `verification.start` does, so any code outside this
    // operation's union collapses to the one denial. The check is registry data rather than
    // a list maintained here, so it tracks the IDL by construction.
    //
    // Model availability is not one of the run's refusals any more: the guard above decides
    // it before the budget write. `run_in` still looks the model up, because it needs the
    // value, and its `no_model` arm is now unreachable from this operation — see the comment
    // there for why no race can reach it.
    //
    // `continuation.frontier` is passed through unread by anything that decides whether this
    // resume is *admissible* — bn-10wdo's cont_* semantics disposition (RFC 0026,
    // "Continuations and resume admissibility") keeps it that way, `bounds`/`frontier` are
    // pinned provenance, not a resume instruction. It reaches `verification::advance` only so
    // `run` can check, rather than trust, that this run rediscovers it.
    let mut explored = None;
    let mut authorize = |minted: &ContinuationHandle, states: u64| {
        explored = Some(states);
        super::admission::admits_derived(call.grant, Derived::Instance(minted.as_str()))
    };
    let ran = verification::advance(
        &task,
        bounds,
        &frontier,
        state,
        services,
        store,
        &publisher,
        &mut authorize,
    );
    if let Err(fault) = &ran {
        // A run refused on its minted continuation leaves the shared task as it was: the
        // budget write is put back, and the work is charged to the presenting capability.
        if fault.derived_denial {
            if let Some(entry) = state.tasks_mut().get_mut(&task) {
                let _ = budget::update(&mut entry.ledger, &prior_budget);
                entry.milestones.truncate(prior_milestones);
                entry.events.truncate(prior_events);
            }
            state
                .tasks_mut()
                .refuse_run(refused_key, explored.unwrap_or_default());
        }
    }
    ran.map_err(|fault| {
        if super::errors::admits(call.spec, fault.code) {
            fault
        } else {
            Fault::denied()
        }
    })?;
    let entry = state.tasks().get(&task).ok_or_else(Fault::denied)?;
    let mut effect = reported(
        Payload::TaskResume(TaskResumeResponse {
            task: entry.handle.clone(),
            status: entry.status,
        }),
        Nullable::Null,
        entry,
    );
    // `task.resume` is the second operation that accepts a budget, so it owes the same
    // INV-007 manifest `verification.start` and `task.update_budget` do (bn-23j7s).
    effect.omissions = [effect.omissions, budget::omissions_of(&entry.ledger)].concat();
    // A resumed run that parked again lands on the `task_suspended` lane, which is what
    // `@task_starting` licenses this operation to report and what makes the second
    // continuation reachable without a second read. bn-18z recorded the absence of any
    // field for "parked again under a new continuation" as wire defect (8) and covered it
    // by a documented reliance on `task.status`; the envelope had the two fields all
    // along, and `result::success` was hard-coding both away (item 9).
    Ok(match (entry.status, &entry.continuation) {
        (TaskStatus::Suspended, Some(continuation)) => {
            effect.suspended(entry.handle.clone(), continuation.clone())
        }
        _ => effect,
    })
}

/// P1 and P2, composed. See [`resume`] for the decision table this implements.
///
/// # Errors
///
/// [`ErrorCode::EpochUnsupported`] when a pinned epoch names a kind the daemon pins no
/// identity for — "an artifact or continuation declares a schema or semantic epoch unknown
/// or incompatible with this daemon […] typed rejection, never best-effort decoding" — and
/// [`ErrorCode::ContinuationEpochMismatch`] when the two sides pin different identities.
///
/// An epoch the continuation left unpinned constrains nothing, which is
/// `EpochSet::first_mismatch`'s own permissive rule: "a continuation resumes only under its
/// pinned epoch, and one that pinned nothing declared nothing to resume under". What makes
/// that safe is the *creation-time* obligation, which [`Continuation`] discharges by having
/// no constructor that can omit a field.
pub fn admissible_epochs(pinned: &PinnedEpochs, current: &EpochSet) -> Result<(), Fault> {
    let held = PinnedEpochs::of(current);
    for (want, have) in pinned.compatibility().into_iter().zip(held.compatibility()) {
        match (want.value(), have.value()) {
            (None, _) => {}
            (Some(_), None) => {
                return Err(Fault::new(
                    ErrorCode::EpochUnsupported,
                    "the continuation pins an epoch this daemon holds no identity for",
                ));
            }
            (Some(want), Some(have)) if want == have => {}
            (Some(_), Some(_)) => {
                return Err(Fault::new(
                    ErrorCode::ContinuationEpochMismatch,
                    "a pinned compatibility epoch disagrees with this daemon's current one",
                ));
            }
        }
    }
    // P2 is equality on `EpochIdentity`, never an ordering: engine identities are content
    // identities, and there is no "newer engine" relation to accept a resume on.
    match (pinned.engine.value(), held.engine.value()) {
        (None, _) => Ok(()),
        (Some(want), Some(have)) if want == have => Ok(()),
        (Some(_), _) => Err(Fault::new(
            ErrorCode::ContinuationEpochMismatch,
            "the continuation's pinned engine identity is not this daemon's",
        )),
    }
}

/// The milestone a cancel records.
pub const MILESTONE_CANCELLED: &str = "task.cancelled";
/// The milestone a budget update records.
pub const MILESTONE_BUDGET_UPDATED: &str = "task.budget_updated";
/// The milestone a budget update that lowered a ceiling below committed spend records.
///
/// B18's park, named. It is recorded *beside* [`MILESTONE_BUDGET_UPDATED`] rather than
/// instead of it, because both facts are true: the ceiling was recorded, and it cannot bind
/// the current run.
pub const MILESTONE_BUDGET_SUSPENDED: &str = "task.budget_suspended";

fn stale() -> Fault {
    Fault::new(
        ErrorCode::StaleSnapshot,
        "the snapshot the continuation pinned is not the current sealed snapshot",
    )
}

/// One answer about a task, carrying the task's own omission manifest and the publications
/// it has committed.
///
/// The envelope names the task, which is the second clause of the IDL's `task` presence
/// rule — "and on results of task-observing operations". Every operation in this family
/// observes one, so every answer it builds names it.
///
/// It also names what the task *published* (bn-23j7s). `ResultEnvelope.artifacts` is
/// RFC 0026's list of "typed refs (`kind`, `handle`, `commitment?`, `redacted?`), not bare
/// handles", and it is where a result says which artifacts it is about — the same field the
/// `workspace` and `observe` families already answer with, and the field RFC 0026 reasons
/// about when it says "a dedup that is invisible in `artifacts` but visible in reported cost
/// is still an oracle". A task's committed publications belong there; an *uncommitted* one
/// cannot reach it, because [`Publications::artifacts`] reads the committed list and a
/// staged publication is not in it.
fn reported(payload: Payload, verdict: Nullable<Verdict>, entry: &TaskEntry) -> Effect {
    let mut effect = Effect::new(payload, verdict).observing(entry.handle.clone());
    effect.omissions = entry.omissions();
    effect.artifacts = published(entry);
    effect
}

/// The wire references naming a task's committed publications.
///
/// A refusal to name one is recorded as an empty list rather than raised: the identity of a
/// publication this daemon already committed is not something a caller can be at fault for,
/// and `rule envelope.no_prose` leaves no room to explain it on an answer that otherwise
/// succeeded. It is unreachable — a `task_*` handle is a well-formed artifact handle by
/// construction — and the branch is pinned by a unit test rather than left to be believed.
pub(super) fn published(entry: &TaskEntry) -> Vec<ArtifactRef> {
    entry
        .evidence
        .artifacts(&entry.handle)
        .unwrap_or_else(|_| Vec::new())
}

/// A typed "this daemon does not produce that" omission (INV-007).
pub fn unsupported(subject: &str) -> Omission {
    Omission {
        reason: OmissionReason::Unsupported,
        subject: subject.to_owned(),
        recoverable_by: Optional::Absent,
    }
}

fn structural(outcome: StructuralOutcome) -> Nullable<Verdict> {
    Nullable::Value(Verdict::Structural(StructuralVerdictValue { outcome }))
}

fn optional<T>(value: Option<T>) -> Optional<T> {
    match value {
        Some(value) => Optional::Present(value),
        None => Optional::Absent,
    }
}
