//! Regions: the deterministic task-lifecycle calculus PR 6 puts under the daemon's work
//! (plan §4.1, §4.5; PR-6 / IMPL-01, IMPL-05, IMPL-06).
//!
//! > It runs its own work as cancel-correct asupersync regions. A task has explicit
//! > lifecycle:
//! >
//! > ```text
//! > Created → Running → Suspended | Completed | Failed | Cancelled
//! >                        │
//! >                        └── continuation + committed partial evidence
//! > ```
//! >
//! > — `notes/plan/plan.md` §4.1
//!
//! > Search, proof, synthesis, indexing, and debugging tasks run under asupersync
//! > regions and publish only committed artifacts. Cancellation cannot leave false
//! > finality or orphan work.
//! >
//! > — `notes/plan/plan.md` §3
//!
//! # What this module is, and what it is not
//!
//! It is the **region semantics**, in typed deterministic code: a tree of regions that
//! own workers and child regions, a lifecycle in which work can only enter an open
//! region, a request → drain → finalize teardown, and an obligation ledger that makes
//! "no orphan workers" a checked property rather than a convention.
//!
//! It is **not** an asupersync integration, and does not pretend to be one. The dossier
//! splits those two things and this module sits on the near side of the split:
//!
//! - plan §21's **Phase A** deliverable is "task/continuation lifecycle"; the
//!   "asupersync semantic adapter (one adapter crate …)" is a **Phase B** deliverable,
//!   landing in `continuum-asupersync` (PR 14), whose own module documentation scopes it
//!   to "instrumentation of narrow asupersync primitives … emitted as a canonical
//!   semantic journal";
//! - plan §20 gives `continuum-task` the responsibility this file implements — "task
//!   lifecycle, committed partial evidence, budget accounting, suspension/continuation,
//!   drain and finalize behavior, and the guarantee of no orphan workers" — and gives
//!   the workspace no async-runtime edge at all;
//! - ADR-0001 requires exactly this separation to be possible: "Continuum remains
//!   capable of model-only execution without asupersync. The normative abstract
//!   semantics are owned by Continuum, so the runtime cannot silently redefine model
//!   behavior."
//!
//! So the substrate slots *behind* this seam later: when the adapter lands, its journal
//! of region and task events is lifted into these states, and the properties below are
//! what the adapter is then held to. Writing them first is what makes that a
//! conformance question instead of a design question.
//!
//! # Determinism: concurrency is data
//!
//! There is no thread here, no clock, no executor, no interior mutability, and no
//! ambient anything — a region is a value and a run is a
//! [`Schedule`](schedule::Schedule) (INV-005: "controlled code accesses scheduling,
//! time, entropy, I/O, faults, and cancellation through explicit capabilities"; INV-002:
//! "every stateful workflow uses explicit handles"). Interleaving is modelled the way
//! the G0-DX-13 campaign modelled it — one schedule pins one interleaving, and the
//! campaign's house rule that *assertions are on outcomes, never on interleavings*
//! carries over unchanged — except that here the interleaving is a written-down value
//! rather than a rendezvous between real threads, so the tests enumerate the schedule
//! space exhaustively instead of forcing one point in it.
//!
//! # The lifecycle, in one place
//!
//! ```text
//!                spawn / open_child           (refused after this line)
//!                        │
//!   Open ──── close ─────┼──── cancel ───→ Draining(Cancelled)
//!     │                  ↓                          │
//!     └──────────→ Draining(Closed)                 │
//!                        │                          │
//!                      drain ──────────────────── drain
//!                        │  (requires terminal)     │  (forces terminal)
//!                        ↓                          ↓
//!                     finalize ─────────────────→ Finalized
//! ```
//!
//! Four rules carry the weight, and each is enforced by construction rather than by
//! discipline:
//!
//! 1. **Work enters only through an open region.** [`RegionTree::spawn`] and
//!    [`RegionTree::open_child`] refuse anything else with a typed fault. This is what
//!    bounds the set a teardown has to account for: after the request phase, that set
//!    can never grow again.
//! 2. **Cancellation is subtree-wide and monotone.** [`RegionTree::cancel`] marks the
//!    region *and every non-finalized descendant*, and upgrades a descendant that was
//!    merely closed. There is no path by which a cancelled parent finalizes over a
//!    still-open child.
//! 3. **Drain is total for cancellation and blocking for a normal close.** Under
//!    [`DrainCause::Cancelled`] every non-terminal worker is driven to
//!    [`WorkerState::Cancelled`]; under [`DrainCause::Closed`] a non-terminal worker is
//!    a typed [`RegionFault::DrainBlocked`] naming it, which is what "wait for owned
//!    work" becomes when waiting is not a thing a deterministic model can do.
//! 4. **Finalize requires *terminal*, not merely quiescent.** A parked
//!    [`WorkerState::Suspended`] worker is quiescent — nothing is running — but its
//!    status can still change, and a finalized region that still owns a resumable worker
//!    is a continuation pointing into a scope that no longer exists. The stronger gate
//!    is what makes the post-condition sayable in one sentence: **every worker ever
//!    spawned into the subtree is in a terminal state, and the obligation ledger is
//!    empty.**
//!
//! # The no-orphan argument
//!
//! [`Finalization::is_total`] is the machine-checked form of it, and the argument it
//! encodes is short enough to read:
//!
//! - the only constructor of a worker is [`RegionTree::spawn`], which opens exactly one
//!   [`ObligationKind::WorkerTermination`](obligation::ObligationKind::WorkerTermination)
//!   obligation and refuses any region that is not [`RegionState::Open`];
//! - the only writer of [`WorkerState`] is one private transition function, which
//!   discharges that obligation on — and only on — the transition into a terminal state;
//! - [`RegionTree::finalize`] refuses while any owned worker is non-terminal, and
//!   discharges the child-region and cancellation obligations as it goes;
//! - therefore, when [`RegionTree::finalize`] returns, the obligations opened by every
//!   spawn, every child region and every cancellation request in the subtree have been
//!   discharged, and [`Ledger::is_balanced`](obligation::Ledger::is_balanced) says so
//!   with two independent accountings — the outstanding *set* and the opened/discharged
//!   *counters*.
//!
//! Two later additions keep the argument intact (RFC 0026 correction 51, bn-2318t):
//!
//! - **explicit abort and staging slots** — a running worker may drop a staged
//!   publication itself ([`WorkerStep::Abort`]), and may hold several staged at once, one
//!   per [`PublicationSlot`](worker::PublicationSlot). Each slot opens its own
//!   `provisional-publication` obligation and is resolved exactly once, by a commit, an
//!   abort, or the discard a drain or failure performs, so the unresolved-publication
//!   conjunct still reads "nothing staged" and the ledger still pairs every stage with one
//!   resolution;
//! - **adapter obligations** — [`RegionTree::open_substrate`] is the one public way into
//!   the ledger. It needs a holder that has begun and not terminated, in an open region,
//!   and a fresh identity; [`RegionTree::discharge_substrate`] needs a matching open held
//!   by the named worker, which may be parked or in a draining region.
//!   Nothing in the calculus discharges an adapter obligation on the adapter's behalf, so
//!   one left open at finalize makes [`Finalization::is_total`] false: a leak is reported,
//!   never absorbed.
//!
//! RFC 0026 correction 53 (bn-36wy3) adds the per-task half of docs/02 §7's lifecycle,
//! and keeps the argument too:
//!
//! - **single-task cancellation** — [`WorkerStep::RequestCancel`] requests one worker's
//!   cancellation (a deadline), [`WorkerStep::AcknowledgeCancel`] is the worker observing
//!   a request (its own or its region's), and [`WorkerStep::CompleteCancelled`] ends one
//!   acknowledged worker through the same per-worker cancellation a drain uses. It
//!   discards what is staged, records the cancel outcome, and discharges the termination
//!   obligation, so the orphan and unresolved-publication conjuncts are unchanged. An
//!   acknowledged worker only drains;
//! - **the acknowledgement decides** — the adapter-obligation rules turn at the worker's
//!   own acknowledgement ([`RegionTree::cancel_phase`]), not at its region's request;
//! - **a subtree's ledger** — [`Finalization::ledger`] sums only the finalized
//!   subtree's own obligations, so a child's teardown is judged by what it owes.
//!
//! RFC 0026 correction 59 (bn-fxxf2) adds the fail-stop crash, and keeps the argument:
//!
//! - **a crash is one step** — [`WorkerStep::Crash`] moves a worker from any non-terminal
//!   state, parked included, and from any cancellation phase, to
//!   [`WorkerState::Failed`], and runs none of its code. It discards what is staged, as a
//!   failure does, so the unresolved-publication conjunct is unchanged, and it discharges
//!   the termination obligation through the one private transition function;
//! - **what a crashed worker held is fenced, not absorbed** — its adapter obligations stay
//!   open in the ledger with a terminal holder, so no discharge or hand-off reaches them,
//!   and a finalization over them is not total. The calculus reports them; it does not
//!   pretend they were met.
//!
//! The crash step's instrumented half is `crates/continuum-task/tests/region_crash.rs`,
//! with `continuumd`'s `tests/region_staging_differential.rs` and
//! `continuum-asupersync`'s `tests/fail_stop_crash.rs`; the schedule sweeps below do not
//! inject it.
//!
//! `crates/continuum-task/tests/region_no_orphan.rs` is the instrumented half: it
//! enumerates schedule spaces, tears each one down, and asserts the post-condition over
//! every worker the tree ever admitted.
//!
//! # Where the daemon plugs in (the seam, not the rewiring)
//!
//! `continuumd`'s `TaskTable` (bn-18z) is the synchronous-deterministic task table PR 5
//! landed: `TaskEntry::advance` is its single status writer, it refuses every transition
//! out of a terminal state, and its `TaskStatus::Running` "is representable and never
//! reported" because `Daemon::dispatch` holds `&mut DaemonState` for a whole call. Three
//! things make this module the layer that goes underneath it, and none of them requires
//! an edge in either direction:
//!
//! - the vocabularies already agree — [`WorkerState::status_token`] emits RFC 0026's six
//!   `TaskStatus` spellings, so a `TaskEntry` and a worker name the same state;
//! - the shapes already agree — `TaskEntry::advance` is one writer refusing terminal
//!   transitions, which is exactly this module's private transition function;
//! - what this module adds is the part a table cannot have: ownership, subtree
//!   cancellation, and a teardown with a post-condition.
//!
//! So the PR 6 rewiring is a `TaskEntry` gaining a [`RegionId`] and delegating, with
//! `task.cancel` calling [`RegionTree::cancel`] then [`RegionTree::drain`] then
//! [`RegionTree::finalize`] — the three operations RFC 0026 already names in that order
//! — and reading its nullable `continuation` off the resulting
//! [`CancelOutcome`](worker::CancelOutcome). That rewiring is deliberately **not** done
//! here: this bone owns the semantics, and moving the daemon's task table onto them is
//! its own change with its own golden traces.
//!
//! # What is deferred, and named
//!
//! - **Budget accounting (PR-6 / IMPL-02, IMPL-03, IMPL-04).** A worker here commits and
//!   discards evidence; it does not spend a nine-dimension budget, and this module mints
//!   no `Cost`. Committed partial evidence, budget accounting and suspension/continuation
//!   are separate PR-6 deliverables and are not claimed by this file.
//! - **Real continuations.** [`Continuation`](worker::Continuation) is a region-layer
//!   fact, not a `cont_*` handle; RFC 0026's pinning obligation (snapshot, intent, six
//!   epochs, engine identity) is `continuumd`'s and is not forged here.
//! - **The publication store.** The two-phase reserve/commit shape mirrors
//!   `continuum_workspace::publication`'s `StagedPublication` → `CommittedContent` →
//!   receipt typestate and its `abandon` path, and is cited throughout, but this crate
//!   declares no `continuum-workspace` edge (plan §20 states none, and `src/lib.rs`
//!   records the omission as deliberate). Binding a worker's evidence ledger to a real
//!   `ReferenceStore` is the daemon's join.
//! - **G0-DX-14.** This module is one input to that campaign, not the campaign. The
//!   matrix row requires cancelling *DPOR, solver, proof and synthesis* tasks at every
//!   phase; three of those four engines do not exist yet, so the campaign
//!   (`crates/continuum-task/tests/dx14_cancellation_matrix.rs`, bn-2zy) runs the four as
//!   publication/suspension **profiles** rather than engines — which is enough because
//!   [`RegionTree::cancel_outcome`] is a function of committed evidence and declared
//!   resumability and of nothing else. That file, its daemon-grain half
//!   (`crates/continuumd/tests/dx14_cancellation_matrix.rs`), and the sweeps in
//!   `tests/region_no_orphan.rs` and `tests/budget_partial_evidence.rs` are what the row's
//!   Evidence column now names. Nothing in this module changed to make it pass.

pub mod obligation;
pub mod schedule;
pub mod worker;

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use obligation::{
    Ledger, LedgerSummary, Obligation, SubstrateId, SubstrateObligation, SubstrateOutcome,
};
use worker::{
    CancelOutcome, CancelPhase, Continuation, EvidenceLedger, PublicationSlot, Resumability,
    WorkerId, WorkerState, WorkerStep,
};

/// A region's identity inside one [`RegionTree`].
///
/// Dense ordinals assigned in open order, root first. Like [`WorkerId`], they are
/// counted and never drawn or timed, so one schedule names one set of identities on
/// every run and on every platform (INV-005).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegionId(u32);

impl RegionId {
    /// The identity at `ordinal` in open order. The root region is ordinal 0.
    #[must_use]
    pub const fn at(ordinal: u32) -> Self {
        Self(ordinal)
    }

    /// This region's position in open order.
    #[must_use]
    pub const fn ordinal(self) -> u32 {
        self.0
    }
}

impl fmt::Display for RegionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "r{}", self.0)
    }
}

/// Why a region stopped accepting work.
///
/// The distinction is the whole difference between the two teardown disciplines, and it
/// is carried inside [`RegionState::Draining`] rather than beside it so that a draining
/// region without a reason for draining is not representable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DrainCause {
    /// Closed normally: no new work may enter, and the work already inside must reach a
    /// terminal state on its own before the region can finalize.
    Closed,
    /// A cancellation reached it: the work inside is terminated by the drain.
    Cancelled,
}

impl DrainCause {
    /// A stable token for canonical rendering.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Closed => "closed",
            Self::Cancelled => "cancelled",
        }
    }
}

impl fmt::Display for DrainCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.token())
    }
}

/// Where a region is in its own lifecycle.
///
/// Three states, and the transitions between them are one-way: `Open → Draining →
/// Finalized`. A region never reopens, which is what lets the teardown argument quantify
/// over a set that stopped growing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RegionState {
    /// Accepting work: [`RegionTree::spawn`] and [`RegionTree::open_child`] succeed.
    Open,
    /// No longer accepting work, not yet finalized. Workers already inside may still be
    /// advanced — a cancelled worker observing cancellation is a *step*, not a state.
    Draining(DrainCause),
    /// Torn down. Every worker the subtree owned is terminal and every obligation it
    /// opened is discharged.
    Finalized,
}

impl RegionState {
    /// A stable token for canonical rendering.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Draining(DrainCause::Closed) => "draining(closed)",
            Self::Draining(DrainCause::Cancelled) => "draining(cancelled)",
            Self::Finalized => "finalized",
        }
    }

    /// Whether work may still enter.
    #[must_use]
    pub const fn accepts_work(self) -> bool {
        matches!(self, Self::Open)
    }

    /// The reason it is draining, if it is.
    #[must_use]
    pub const fn drain_cause(self) -> Option<DrainCause> {
        match self {
            Self::Draining(cause) => Some(cause),
            _ => None,
        }
    }
}

impl fmt::Display for RegionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.token())
    }
}

/// Every way a region operation can be refused, named.
///
/// INV-008 at this layer: an illegal transition is a distinct typed value, never a
/// panic, never a silently-ignored call, and never a bare `false`. The variants are the
/// legality table of this module made total — if a step is not in the table, the refusal
/// says which step, which subject, and what state the subject was actually in, so
/// "why was this refused" is answerable without re-deriving it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegionFault {
    /// No region has this identity in this tree.
    UnknownRegion(RegionId),
    /// No worker has this identity in this tree.
    UnknownWorker(WorkerId),
    /// Work may only be spawned into an open region — the rule that bounds the set a
    /// teardown must account for.
    SpawnIntoClosedRegion {
        /// The region the spawn named.
        region: RegionId,
        /// What state it was actually in.
        state: RegionState,
    },
    /// A child region may only be opened inside an open parent, for the same reason.
    OpenChildInClosedRegion {
        /// The parent the call named.
        region: RegionId,
        /// What state it was actually in.
        state: RegionState,
    },
    /// Only an open region can be closed. Closing a draining one would be a second
    /// request against a region that already has one; closing a finalized one is a
    /// request against something that no longer exists.
    CloseNonOpenRegion {
        /// The region the call named.
        region: RegionId,
        /// What state it was actually in.
        state: RegionState,
    },
    /// A finalized region cannot be cancelled: its work is already terminal, and RFC
    /// 0026's monotonicity forbids reopening the question.
    CancelFinalizedRegion {
        /// The region the call named.
        region: RegionId,
    },
    /// Drain runs between the request and the finalize. An open region has had no
    /// request.
    DrainBeforeRequest {
        /// The region the call named.
        region: RegionId,
    },
    /// A finalized region has nothing left to drain.
    DrainFinalizedRegion {
        /// The region the call named.
        region: RegionId,
    },
    /// A normally-closed region still owns a non-terminal worker.
    ///
    /// This is what "drain waits for owned work" becomes in a model with no waiting: the
    /// refusal names the worker the schedule still has to advance. It is not reachable
    /// under [`DrainCause::Cancelled`], where the drain terminates the work itself.
    DrainBlocked {
        /// The region being drained.
        region: RegionId,
        /// The worker still owing a terminal state.
        worker: WorkerId,
        /// What state that worker was in.
        state: WorkerState,
    },
    /// Finalize follows drain, which follows a request. This region has had no request.
    FinalizeBeforeDrain {
        /// The region the call named.
        region: RegionId,
        /// What state it was actually in.
        state: RegionState,
    },
    /// The region is already finalized. Finalizing is idempotent in effect but not in
    /// report: a second call has no [`Finalization`] to hand back that the first did not
    /// already hand back, so it says so rather than inventing one.
    AlreadyFinalized {
        /// The region the call named.
        region: RegionId,
    },
    /// A worker in the subtree is not terminal, so the no-orphan post-condition would be
    /// false the moment [`RegionTree::finalize`] returned.
    FinalizeBeforeTermination {
        /// The region owning the worker.
        region: RegionId,
        /// The worker still owing a terminal state.
        worker: WorkerId,
        /// What state that worker was in.
        state: WorkerState,
    },
    /// The worker's region is finalized, so there is nothing left to advance.
    AdvanceInFinalizedRegion {
        /// The worker the call named.
        worker: WorkerId,
        /// Its region.
        region: RegionId,
    },
    /// The step is not legal from the state the worker is in.
    ///
    /// This is the catch-all row of the transition table, and it carries both halves so
    /// the reader never has to guess which one was wrong.
    IllegalWorkerStep {
        /// The worker the call named.
        worker: WorkerId,
        /// The state it was in.
        from: WorkerState,
        /// The step that was refused.
        step: WorkerStep,
    },
    /// A publication is already staged. At most one is in flight, which is the store's
    /// own linear typestate.
    ///
    /// [`WorkerStep::Reserve`] keeps that one-in-flight discipline;
    /// [`WorkerStep::ReserveSlot`] is the step that stages a second publication (RFC 0026
    /// correction 51).
    ReserveOverProvisional {
        /// The worker the call named.
        worker: WorkerId,
    },
    /// Nothing is staged, so there is nothing to commit.
    CommitWithoutReserve {
        /// The worker the call named.
        worker: WorkerId,
    },
    /// A worker may not park mid-publication.
    ///
    /// > Cancellation MUST NOT truncate a publication in progress.
    /// >
    /// > — RFC 0026, "Atomicity of publication"
    ///
    /// Parking with a staged publication would leave an artifact that is neither
    /// committed nor absent for as long as the park lasts, which is the state G0-DX-14's
    /// second conjunct forbids at rest.
    SuspendWithProvisionalEvidence {
        /// The worker the call named.
        worker: WorkerId,
    },
    /// A worker may not complete mid-publication, for the same reason.
    CompleteWithProvisionalEvidence {
        /// The worker the call named.
        worker: WorkerId,
    },
    /// [`WorkerStep::RequestCancel`] named a worker whose cancellation is already
    /// requested, by its own request or by its region's.
    CancelAlreadyRequested {
        /// The worker the call named.
        worker: WorkerId,
    },
    /// [`WorkerStep::AcknowledgeCancel`] named a worker whose cancellation nobody
    /// requested.
    CancelNotRequested {
        /// The worker the call named.
        worker: WorkerId,
    },
    /// [`WorkerStep::AcknowledgeCancel`] named a worker that already acknowledged.
    CancelAlreadyAcknowledged {
        /// The worker the call named.
        worker: WorkerId,
    },
    /// [`WorkerStep::CompleteCancelled`] named a worker that has not acknowledged its
    /// cancellation.
    CancelNotAcknowledged {
        /// The worker the call named.
        worker: WorkerId,
    },
    /// The worker acknowledged its cancellation, so it only drains: this step is not an
    /// abort or its cancelled completion (docs/02 §7, RFC 0026 correction 53).
    WorkerCancelling {
        /// The worker the call named.
        worker: WorkerId,
        /// The step that was refused.
        step: WorkerStep,
    },
    /// A worker declared [`Resumability::NonResumable`] cannot suspend.
    ///
    /// > `TaskRecord.continuation` is REQUIRED when `status = suspended` — a suspended
    /// > task is resumable by definition.
    /// >
    /// > — RFC 0026, "Task lifecycle"
    SuspendNonResumableWorker {
        /// The worker the call named.
        worker: WorkerId,
    },
    /// Nothing is staged, so there is nothing to abort (RFC 0026 correction 51).
    AbortWithoutReserve {
        /// The worker the call named.
        worker: WorkerId,
    },
    /// [`WorkerStep::Commit`] or [`WorkerStep::Abort`] named no slot while more than one
    /// publication is staged, so it cannot say which one it resolves. The slot-addressed
    /// steps can.
    AmbiguousResolution {
        /// The worker the call named.
        worker: WorkerId,
        /// The step that was refused.
        step: WorkerStep,
        /// How many publications are staged.
        staged: u32,
    },
    /// [`WorkerStep::ReserveSlot`] named a slot that already holds a staged publication.
    /// Each slot is one linear resource, staged once and resolved once.
    SlotAlreadyStaged {
        /// The worker the call named.
        worker: WorkerId,
        /// The slot it named.
        slot: PublicationSlot,
    },
    /// [`WorkerStep::CommitSlot`] or [`WorkerStep::AbortSlot`] named a slot that holds no
    /// staged publication.
    SlotNotStaged {
        /// The worker the call named.
        worker: WorkerId,
        /// The step that was refused.
        step: WorkerStep,
    },
    /// A worker in an adapter-obligation operation has terminated, so it may not open,
    /// discharge, hand on, or receive one. A terminal holder's open obligation is a leak,
    /// and a leak is not laundered by a later discharge.
    SubstrateHolderNotLive {
        /// The worker the call named.
        worker: WorkerId,
        /// What state it was in.
        state: WorkerState,
    },
    /// A worker that has not begun takes no part in an adapter obligation — it may not
    /// open or receive one — because it has taken no step and holds nothing (RFC 0026
    /// correction 51).
    SubstrateHolderNotBegun {
        /// The worker the call named.
        worker: WorkerId,
    },
    /// The region of a worker in an adapter-obligation operation does not admit it. An
    /// open needs an [`RegionState::Open`] region — the rule that bounds what a teardown
    /// must account for. A discharge or a transfer needs only a region that is not
    /// finalized (defence in depth, unreachable while the party is not terminal).
    SubstrateRegionNotOpen {
        /// The worker the call named.
        worker: WorkerId,
        /// Its region.
        region: RegionId,
        /// What state that region was in.
        state: RegionState,
    },
    /// A worker that acknowledged its cancellation may not open, hand on or receive an
    /// adapter obligation: it no longer acts on its own account (RFC 0026 correction 53).
    SubstrateHolderCancelling {
        /// The worker the call named.
        worker: WorkerId,
    },
    /// A committed discharge by a holder that acknowledged its cancellation. A task in
    /// cancellation only drains and finalizes, so its discharges must be
    /// [`SubstrateOutcome::Aborted`] cleanup (RFC 0026 corrections 51 and 53).
    SubstrateCommitDuringCancellation {
        /// The obligation the call named.
        obligation: SubstrateObligation,
        /// The holder that tried to commit it.
        worker: WorkerId,
    },
    /// A transfer named the obligation's holder as its receiver. A hand-off to the holder
    /// itself hands nothing, so it is refused rather than admitted as a no-op.
    SubstrateSelfTransfer {
        /// The obligation the call named.
        obligation: SubstrateObligation,
        /// The worker named as both giver and receiver.
        worker: WorkerId,
    },
    /// This adapter obligation identity was opened before in this tree. An identity opens
    /// once, so a discharged obligation cannot be reopened to move the counters twice.
    SubstrateIdentityReused {
        /// The obligation the call named.
        obligation: SubstrateObligation,
    },
    /// No open adapter obligation has this identity and kind: it was never opened, it is
    /// already discharged, or the call named the wrong kind.
    SubstrateNotOpen {
        /// The obligation the call named.
        obligation: SubstrateObligation,
    },
    /// The worker the call named is not the obligation's holder — including a holder in
    /// another region. Only the holder discharges or hands on what it holds.
    SubstrateHolderMismatch {
        /// The obligation the call named.
        obligation: SubstrateObligation,
        /// The worker the call named.
        named: WorkerId,
        /// The worker that holds it.
        holder: WorkerId,
    },
}

impl fmt::Display for RegionFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownRegion(region) => write!(f, "no region {region} in this tree"),
            Self::UnknownWorker(worker) => write!(f, "no worker {worker} in this tree"),
            Self::SpawnIntoClosedRegion { region, state } => {
                write!(f, "cannot spawn into {region}: it is {state}, not open")
            }
            Self::OpenChildInClosedRegion { region, state } => {
                write!(
                    f,
                    "cannot open a child of {region}: it is {state}, not open"
                )
            }
            Self::CloseNonOpenRegion { region, state } => {
                write!(f, "cannot close {region}: it is {state}, not open")
            }
            Self::CancelFinalizedRegion { region } => {
                write!(f, "cannot cancel {region}: it is already finalized")
            }
            Self::DrainBeforeRequest { region } => {
                write!(f, "cannot drain {region}: it is open, so nothing requested")
            }
            Self::DrainFinalizedRegion { region } => {
                write!(f, "cannot drain {region}: it is already finalized")
            }
            Self::DrainBlocked {
                region,
                worker,
                state,
            } => write!(
                f,
                "{region} is closed but {worker} is {state}, so the drain is not done"
            ),
            Self::FinalizeBeforeDrain { region, state } => {
                write!(f, "cannot finalize {region}: it is {state}, not draining")
            }
            Self::AlreadyFinalized { region } => write!(f, "{region} is already finalized"),
            Self::FinalizeBeforeTermination {
                region,
                worker,
                state,
            } => write!(
                f,
                "cannot finalize {region}: {worker} is {state}, which is not terminal"
            ),
            Self::AdvanceInFinalizedRegion { worker, region } => {
                write!(
                    f,
                    "cannot advance {worker}: its region {region} is finalized"
                )
            }
            Self::IllegalWorkerStep { worker, from, step } => {
                write!(f, "{worker} is {from}, so the step {step} is not legal")
            }
            Self::ReserveOverProvisional { worker } => {
                write!(f, "{worker} already holds a staged publication")
            }
            Self::CommitWithoutReserve { worker } => {
                write!(f, "{worker} has nothing staged to commit")
            }
            Self::SuspendWithProvisionalEvidence { worker } => {
                write!(f, "{worker} cannot park while a publication is staged")
            }
            Self::CompleteWithProvisionalEvidence { worker } => {
                write!(f, "{worker} cannot complete while a publication is staged")
            }
            Self::SuspendNonResumableWorker { worker } => {
                write!(f, "{worker} was declared non-resumable, so it cannot park")
            }
            Self::CancelAlreadyRequested { worker } => {
                write!(f, "{worker}'s cancellation is already requested")
            }
            Self::CancelNotRequested { worker } => {
                write!(f, "nothing requested {worker}'s cancellation")
            }
            Self::CancelAlreadyAcknowledged { worker } => {
                write!(f, "{worker} already acknowledged its cancellation")
            }
            Self::CancelNotAcknowledged { worker } => write!(
                f,
                "{worker} has not acknowledged its cancellation, so it cannot complete as cancelled"
            ),
            Self::WorkerCancelling { worker, step } => write!(
                f,
                "{worker} acknowledged its cancellation, so it may only drain, not {step}"
            ),
            Self::AbortWithoutReserve { worker } => {
                write!(f, "{worker} has nothing staged to abort")
            }
            Self::AmbiguousResolution {
                worker,
                step,
                staged,
            } => write!(
                f,
                "{worker} holds {staged} staged publications, so {step} must name a slot"
            ),
            Self::SlotAlreadyStaged { worker, slot } => {
                write!(f, "{worker} already holds a publication staged in {slot}")
            }
            Self::SlotNotStaged { worker, step } => {
                write!(f, "{worker} has nothing staged in the slot {step} names")
            }
            Self::SubstrateHolderNotLive { worker, state } => write!(
                f,
                "{worker} is {state}, so it cannot take part in an obligation"
            ),
            Self::SubstrateHolderNotBegun { worker } => {
                write!(f, "{worker} has not begun, so it cannot take an obligation")
            }
            Self::SubstrateRegionNotOpen {
                worker,
                region,
                state,
            } => write!(
                f,
                "{worker} is in {region}, which is {state}, so it cannot take an obligation"
            ),
            Self::SubstrateCommitDuringCancellation { obligation, worker } => write!(
                f,
                "{worker} acknowledged its cancellation, so it may abort {obligation} but not commit it"
            ),
            Self::SubstrateHolderCancelling { worker } => write!(
                f,
                "{worker} acknowledged its cancellation, so it cannot take or hand on an obligation"
            ),
            Self::SubstrateSelfTransfer { obligation, worker } => {
                write!(f, "{worker} cannot hand {obligation} to itself")
            }
            Self::SubstrateIdentityReused { obligation } => {
                write!(f, "{obligation} names an identity this tree already opened")
            }
            Self::SubstrateNotOpen { obligation } => {
                write!(f, "{obligation} is not open, so it cannot be discharged")
            }
            Self::SubstrateHolderMismatch {
                obligation,
                named,
                holder,
            } => write!(f, "{obligation} is held by {holder}, not {named}"),
        }
    }
}

impl core::error::Error for RegionFault {}

/// One region: what owns it, what it owns, and where it is in its lifecycle.
#[derive(Debug)]
struct RegionNode {
    parent: Option<RegionId>,
    children: Vec<RegionId>,
    workers: Vec<WorkerId>,
    state: RegionState,
}

/// One worker: which region owns it, and everything the lifecycle says about it.
#[derive(Debug)]
struct WorkerRecord {
    region: RegionId,
    state: WorkerState,
    evidence: EvidenceLedger,
    /// The slots holding a staged publication. Its size is `evidence.staged()`.
    slots: BTreeSet<PublicationSlot>,
    resumability: Resumability,
    cancel_outcome: Option<CancelOutcome>,
    /// This worker's own cancellation request ([`WorkerStep::RequestCancel`]).
    cancel_requested: bool,
    /// A region's cancellation reached this worker while it was live. Recorded on the
    /// worker when the region is cancelled, so the phase it reached survives the
    /// region's finalization (cr-3pu5cu).
    region_requested: bool,
    /// The worker acknowledged its cancellation ([`WorkerStep::AcknowledgeCancel`]).
    cancel_acknowledged: bool,
}

/// A tree of regions owning workers and child regions — the value the whole calculus is
/// stated over.
///
/// Deliberately not [`Clone`]: the tree carries the one obligation ledger that the
/// no-orphan claim is read off, and a second copy of it is a second answer to "what is
/// outstanding". Deliberately without interior mutability: every operation takes
/// `&mut self`, so a run is a sequence of moves a caller wrote down rather than something
/// that happened.
///
/// Regions and workers are held in arenas indexed by their dense ordinals. The arena is
/// how the tree is *stored*; ownership is the parent edge and the worker list, and those
/// are what every rule below is stated over.
#[derive(Debug)]
pub struct RegionTree {
    regions: Vec<RegionNode>,
    workers: Vec<WorkerRecord>,
    ledger: Ledger,
    /// Every adapter obligation identity ever opened, with its kind and current holder,
    /// and whether it is still open. Kept forever so an identity opens once.
    substrate: BTreeMap<SubstrateId, SubstrateRecord>,
    /// Each worker's adapter obligations, open or discharged, by their current or last
    /// holder: the index a subtree's ledger is built from.
    held_by: BTreeMap<WorkerId, BTreeSet<SubstrateId>>,
}

/// What the tree knows about one adapter obligation.
#[derive(Debug, Clone, Copy)]
struct SubstrateRecord {
    obligation: SubstrateObligation,
    holder: WorkerId,
    open: bool,
}

impl Default for RegionTree {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionTree {
    /// A tree holding one open root region and nothing else.
    #[must_use]
    pub fn new() -> Self {
        Self {
            regions: vec![RegionNode {
                parent: None,
                children: Vec::new(),
                workers: Vec::new(),
                state: RegionState::Open,
            }],
            workers: Vec::new(),
            ledger: Ledger::new(),
            substrate: BTreeMap::new(),
            held_by: BTreeMap::new(),
        }
    }

    /// The root region, which has no parent and is opened by [`Self::new`].
    #[must_use]
    pub const fn root(&self) -> RegionId {
        RegionId::at(0)
    }

    /// How many regions this tree has ever opened, root included.
    #[must_use]
    pub fn region_count(&self) -> usize {
        self.regions.len()
    }

    /// How many workers this tree has ever admitted.
    ///
    /// The denominator of the no-orphan property: [`Finalization::is_total`] is a claim
    /// about *this* many workers, and a test that does not check it against a non-zero
    /// count has proved nothing.
    #[must_use]
    pub fn worker_count(&self) -> usize {
        self.workers.len()
    }

    /// The obligation ledger.
    #[must_use]
    pub const fn ledger(&self) -> &Ledger {
        &self.ledger
    }

    // --- observation -------------------------------------------------------------------

    /// Where `region` is in its lifecycle.
    ///
    /// # Errors
    ///
    /// [`RegionFault::UnknownRegion`] when no such region exists.
    pub fn state(&self, region: RegionId) -> Result<RegionState, RegionFault> {
        self.node(region).map(|node| node.state)
    }

    /// Where `worker` is in the RFC 0026 lifecycle.
    ///
    /// # Errors
    ///
    /// [`RegionFault::UnknownWorker`] when no such worker exists.
    pub fn worker_state(&self, worker: WorkerId) -> Result<&WorkerState, RegionFault> {
        self.record(worker).map(|record| &record.state)
    }

    /// Where `worker` is in its own cancellation (RFC 0026 correction 53).
    ///
    /// [`CancelPhase::Acknowledged`] after its [`WorkerStep::AcknowledgeCancel`];
    /// otherwise [`CancelPhase::Requested`] when it requested its own cancellation or a
    /// region's cancellation reached it while it was live; otherwise
    /// [`CancelPhase::Active`]. Both requests are recorded on the worker, so a worker that
    /// terminated, and a worker whose region finalized, keep the phase they reached: a
    /// cancelled worker is never `Active`.
    ///
    /// # Errors
    ///
    /// [`RegionFault::UnknownWorker`] when no such worker exists.
    pub fn cancel_phase(&self, worker: WorkerId) -> Result<CancelPhase, RegionFault> {
        let record = self.record(worker)?;
        if record.cancel_acknowledged {
            return Ok(CancelPhase::Acknowledged);
        }
        Ok(if record.cancel_requested || record.region_requested {
            CancelPhase::Requested
        } else {
            CancelPhase::Active
        })
    }

    /// What `worker` has published and what it still holds provisionally.
    ///
    /// # Errors
    ///
    /// [`RegionFault::UnknownWorker`] when no such worker exists.
    pub fn evidence(&self, worker: WorkerId) -> Result<EvidenceLedger, RegionFault> {
        self.record(worker).map(|record| record.evidence)
    }

    /// The region that owns `worker`.
    ///
    /// # Errors
    ///
    /// [`RegionFault::UnknownWorker`] when no such worker exists.
    pub fn owner(&self, worker: WorkerId) -> Result<RegionId, RegionFault> {
        self.record(worker).map(|record| record.region)
    }

    /// `region` and every region beneath it, in ascending identity order.
    ///
    /// Ascending order is parent-before-child, because a child's ordinal is always
    /// allocated after its parent's. Teardown walks it backwards for exactly that reason.
    ///
    /// # Errors
    ///
    /// [`RegionFault::UnknownRegion`] when no such region exists.
    pub fn subtree(&self, region: RegionId) -> Result<Vec<RegionId>, RegionFault> {
        self.node(region)?;
        let mut found = Vec::new();
        let mut pending = vec![region];
        while let Some(current) = pending.pop() {
            found.push(current);
            if let Ok(node) = self.node(current) {
                pending.extend(node.children.iter().copied());
            }
        }
        found.sort_unstable();
        Ok(found)
    }

    /// The workers in `region`'s subtree that are not yet terminal, in ascending order.
    ///
    /// The set a drain has to resolve. Empty is the pre-condition of
    /// [`Self::finalize`].
    ///
    /// # Errors
    ///
    /// [`RegionFault::UnknownRegion`] when no such region exists.
    pub fn outstanding_workers(&self, region: RegionId) -> Result<Vec<WorkerId>, RegionFault> {
        let mut outstanding = Vec::new();
        for owner in self.subtree(region)? {
            for worker in &self.node(owner)?.workers {
                if !self.record(*worker)?.state.is_terminal() {
                    outstanding.push(*worker);
                }
            }
        }
        outstanding.sort_unstable();
        Ok(outstanding)
    }

    // --- structure ---------------------------------------------------------------------

    /// Open a child region inside `parent`.
    ///
    /// The child owes its parent a
    /// [`ChildRegionQuiescence`](obligation::ObligationKind::ChildRegionQuiescence)
    /// obligation from this moment — RFC 0001 names that obligation outright — and the
    /// parent cannot finalize while it is outstanding.
    ///
    /// # Errors
    ///
    /// [`RegionFault::UnknownRegion`] when `parent` does not exist, or
    /// [`RegionFault::OpenChildInClosedRegion`] when it is not [`RegionState::Open`].
    pub fn open_child(&mut self, parent: RegionId) -> Result<RegionId, RegionFault> {
        let state = self.node(parent)?.state;
        if !state.accepts_work() {
            return Err(RegionFault::OpenChildInClosedRegion {
                region: parent,
                state,
            });
        }
        let child = RegionId::at(u32::try_from(self.regions.len()).unwrap_or(u32::MAX));
        self.regions.push(RegionNode {
            parent: Some(parent),
            children: Vec::new(),
            workers: Vec::new(),
            state: RegionState::Open,
        });
        self.node_mut(parent)?.children.push(child);
        self.ledger.open(Obligation::child_region_quiescence(child));
        Ok(child)
    }

    /// Admit a worker into `region`, declaring whether its work can be resumed.
    ///
    /// This is the only constructor of a worker, and it opens exactly one
    /// [`WorkerTermination`](obligation::ObligationKind::WorkerTermination) obligation.
    /// Both halves of that sentence are load-bearing: the first is why the set a teardown
    /// accounts for is knowable, and the second is why "accounted for" is checkable.
    ///
    /// `resumability` is a parameter rather than a default because INV-005 makes
    /// explicitness the rule, and because a caller that does not state it is making a
    /// claim either way.
    ///
    /// # Errors
    ///
    /// [`RegionFault::UnknownRegion`] when `region` does not exist, or
    /// [`RegionFault::SpawnIntoClosedRegion`] when it is not [`RegionState::Open`].
    pub fn spawn(
        &mut self,
        region: RegionId,
        resumability: Resumability,
    ) -> Result<WorkerId, RegionFault> {
        let state = self.node(region)?.state;
        if !state.accepts_work() {
            return Err(RegionFault::SpawnIntoClosedRegion { region, state });
        }
        let worker = WorkerId::at(u32::try_from(self.workers.len()).unwrap_or(u32::MAX));
        self.workers.push(WorkerRecord {
            region,
            state: WorkerState::Created,
            evidence: EvidenceLedger::empty(),
            slots: BTreeSet::new(),
            resumability,
            cancel_outcome: None,
            cancel_requested: false,
            region_requested: false,
            cancel_acknowledged: false,
        });
        self.node_mut(region)?.workers.push(worker);
        self.ledger.open(Obligation::worker_termination(worker));
        Ok(worker)
    }

    // --- worker transitions ------------------------------------------------------------

    /// Advance `worker` by one step, returning the state it landed in.
    ///
    /// The single writer of [`WorkerState`], and therefore the single place the
    /// termination obligation is discharged. That is not a style preference: the
    /// no-orphan argument in this module's header is only as good as the claim that no
    /// other code can move a worker, and one private transition function is how that
    /// claim is made true rather than asserted.
    ///
    /// Advancing inside a *draining* region is legal and deliberate — a worker that
    /// observes cancellation and winds itself down is taking steps — but advancing inside
    /// a finalized one is refused, because the region that owned it is gone.
    ///
    /// # Errors
    ///
    /// Any of [`RegionFault::UnknownWorker`], [`RegionFault::AdvanceInFinalizedRegion`],
    /// [`RegionFault::IllegalWorkerStep`], [`RegionFault::ReserveOverProvisional`],
    /// [`RegionFault::CommitWithoutReserve`],
    /// [`RegionFault::SuspendWithProvisionalEvidence`],
    /// [`RegionFault::CompleteWithProvisionalEvidence`],
    /// [`RegionFault::SuspendNonResumableWorker`], [`RegionFault::AbortWithoutReserve`],
    /// [`RegionFault::AmbiguousResolution`], [`RegionFault::SlotAlreadyStaged`] or
    /// [`RegionFault::SlotNotStaged`].
    pub fn advance(
        &mut self,
        worker: WorkerId,
        step: WorkerStep,
    ) -> Result<WorkerState, RegionFault> {
        let region = self.record(worker)?.region;
        if self.node(region)?.state == RegionState::Finalized {
            return Err(RegionFault::AdvanceInFinalizedRegion { worker, region });
        }
        let from = self.record(worker)?.state.clone();
        let phase = self.cancel_phase(worker)?;
        // docs/02 §7: `Cancelling` only drains and finalizes. After its acknowledgement a
        // worker may abort what it staged and complete as cancelled, and nothing else.
        // A fail-stop crash is not a step the worker takes, so it is not a step of its
        // cleanup either: it may stop a cancelling worker too (correction 59).
        if phase == CancelPhase::Acknowledged
            && !from.is_terminal()
            && !matches!(
                step,
                WorkerStep::Abort
                    | WorkerStep::AbortSlot(_)
                    | WorkerStep::RequestCancel
                    | WorkerStep::AcknowledgeCancel
                    | WorkerStep::CompleteCancelled
                    | WorkerStep::Crash(_)
            )
        {
            return Err(RegionFault::WorkerCancelling { worker, step });
        }
        let landed = match (&from, &step) {
            (
                WorkerState::Created | WorkerState::Running | WorkerState::Suspended,
                WorkerStep::RequestCancel,
            ) => {
                if phase != CancelPhase::Active {
                    return Err(RegionFault::CancelAlreadyRequested { worker });
                }
                self.record_mut(worker)?.cancel_requested = true;
                from.clone()
            }
            (
                WorkerState::Created | WorkerState::Running | WorkerState::Suspended,
                WorkerStep::AcknowledgeCancel,
            ) => {
                match phase {
                    CancelPhase::Active => return Err(RegionFault::CancelNotRequested { worker }),
                    CancelPhase::Acknowledged => {
                        return Err(RegionFault::CancelAlreadyAcknowledged { worker });
                    }
                    CancelPhase::Requested => {}
                }
                self.record_mut(worker)?.cancel_acknowledged = true;
                from.clone()
            }
            (
                WorkerState::Created | WorkerState::Running | WorkerState::Suspended,
                WorkerStep::CompleteCancelled,
            ) => {
                if phase != CancelPhase::Acknowledged {
                    return Err(RegionFault::CancelNotAcknowledged { worker });
                }
                // The single-task arm of the cancelling drain, through the one place a
                // worker is cancelled.
                self.cancel_worker(worker)?;
                return Ok(WorkerState::Cancelled);
            }
            (WorkerState::Created, WorkerStep::Begin) => WorkerState::Running,
            (WorkerState::Running, WorkerStep::Reserve) => {
                if self.record(worker)?.evidence.is_provisional() {
                    return Err(RegionFault::ReserveOverProvisional { worker });
                }
                self.stage(worker, PublicationSlot::PRIMARY)?;
                WorkerState::Running
            }
            (WorkerState::Running, WorkerStep::ReserveSlot(slot)) => {
                if self.record(worker)?.slots.contains(slot) {
                    return Err(RegionFault::SlotAlreadyStaged {
                        worker,
                        slot: *slot,
                    });
                }
                self.stage(worker, *slot)?;
                WorkerState::Running
            }
            (WorkerState::Running, WorkerStep::Commit | WorkerStep::Abort) => {
                let commit = step == WorkerStep::Commit;
                let slots = &self.record(worker)?.slots;
                let staged = u32::try_from(slots.len()).unwrap_or(u32::MAX);
                let sole = match slots.first().copied() {
                    None if commit => return Err(RegionFault::CommitWithoutReserve { worker }),
                    None => return Err(RegionFault::AbortWithoutReserve { worker }),
                    Some(slot) if staged == 1 => slot,
                    Some(_) => {
                        return Err(RegionFault::AmbiguousResolution {
                            worker,
                            step,
                            staged,
                        });
                    }
                };
                self.resolve(worker, sole, commit)?;
                WorkerState::Running
            }
            (WorkerState::Running, WorkerStep::CommitSlot(slot) | WorkerStep::AbortSlot(slot)) => {
                if !self.record(worker)?.slots.contains(slot) {
                    return Err(RegionFault::SlotNotStaged { worker, step });
                }
                self.resolve(worker, *slot, matches!(step, WorkerStep::CommitSlot(_)))?;
                WorkerState::Running
            }
            (WorkerState::Running, WorkerStep::Suspend) => {
                if self.record(worker)?.evidence.is_provisional() {
                    return Err(RegionFault::SuspendWithProvisionalEvidence { worker });
                }
                if self.record(worker)?.resumability != Resumability::Resumable {
                    return Err(RegionFault::SuspendNonResumableWorker { worker });
                }
                WorkerState::Suspended
            }
            (WorkerState::Suspended, WorkerStep::Resume) => WorkerState::Running,
            (WorkerState::Running, WorkerStep::Complete) => {
                if self.record(worker)?.evidence.is_provisional() {
                    return Err(RegionFault::CompleteWithProvisionalEvidence { worker });
                }
                WorkerState::Completed
            }
            (WorkerState::Created | WorkerState::Running, WorkerStep::Fail(reason)) => {
                self.discard_provisional(worker)?;
                WorkerState::Failed(reason.clone())
            }
            // A fail-stop crash (RFC 0026 correction 59): from any non-terminal state and
            // any cancellation phase. It discards what is staged and leaves the worker's
            // adapter obligations open, owed by a terminal holder that can never
            // discharge or hand them on: fenced.
            (
                WorkerState::Created | WorkerState::Running | WorkerState::Suspended,
                WorkerStep::Crash(reason),
            ) => {
                self.discard_provisional(worker)?;
                WorkerState::Failed(reason.clone())
            }
            _ => return Err(RegionFault::IllegalWorkerStep { worker, from, step }),
        };
        self.settle(worker, landed.clone())?;
        Ok(landed)
    }

    // --- adapter obligations -----------------------------------------------------------

    /// Open an adapter obligation held by `holder` — the public entry point into this
    /// tree's [`Ledger`] (RFC 0026 correction 51).
    ///
    /// An adapter lifting a substrate whose tasks hold obligations of their own (send
    /// permits, acks, leases, …) opens each one here, so the ledger that
    /// [`Finalization::is_total`] reads covers them, and a region that finalizes while one
    /// is open reports it as owed. The entry point cannot forge balance:
    ///
    /// - `holder` must exist and have begun and not terminated — [`WorkerState::Running`]
    ///   or [`WorkerState::Suspended`] — and its region must be [`RegionState::Open`], the
    ///   same rule [`Self::spawn`] enforces, so an obligation cannot enter a scope whose
    ///   teardown set has already stopped growing. A [`WorkerState::Created`] worker has
    ///   not begun and holds nothing, so its open is refused as
    ///   [`RegionFault::SubstrateHolderNotBegun`]. A parked worker may open, because the
    ///   substrate applies an open after the poll that made it returns;
    /// - an identity opens once in a tree's life, so a discharged obligation cannot be
    ///   reopened to move the counters again;
    /// - the only way out is [`Self::discharge_substrate`], which needs a matching open.
    ///
    /// # Errors
    ///
    /// [`RegionFault::UnknownWorker`], [`RegionFault::SubstrateHolderNotBegun`],
    /// [`RegionFault::SubstrateHolderNotLive`], [`RegionFault::SubstrateHolderCancelling`],
    /// [`RegionFault::SubstrateRegionNotOpen`] or [`RegionFault::SubstrateIdentityReused`].
    pub fn open_substrate(
        &mut self,
        holder: WorkerId,
        obligation: SubstrateObligation,
    ) -> Result<(), RegionFault> {
        self.check_opener(holder)?;
        if self.substrate.contains_key(&obligation.id()) {
            return Err(RegionFault::SubstrateIdentityReused { obligation });
        }
        self.substrate.insert(
            obligation.id(),
            SubstrateRecord {
                obligation,
                holder,
                open: true,
            },
        );
        self.held_by
            .entry(holder)
            .or_default()
            .insert(obligation.id());
        self.ledger.open(obligation.obligation());
        Ok(())
    }

    /// Discharge an adapter obligation that `holder` holds.
    ///
    /// Needs a matching open — the same identity *and* kind, still open — and `holder` must
    /// be the worker holding it, which is what refuses a discharge from another region.
    /// The holder must [hold substrate](WorkerState::holds_substrate) — running or
    /// parked — and its region may be open or draining: the substrate discharges during a
    /// cancellation, before the holder's lifecycle is terminal, and that cleanup must
    /// balance. Once the holder is terminal an open obligation is a leak, and a discharge
    /// after that would launder it.
    ///
    /// `outcome` says how the obligation ended. Until the holder acknowledges a
    /// cancellation both outcomes are admitted, whatever its region's state. After the
    /// acknowledgement only [`SubstrateOutcome::Aborted`] is: a task in cancellation only
    /// drains and finalizes (docs/02 §7), so its discharges are cleanup, and a commit is
    /// refused as [`RegionFault::SubstrateCommitDuringCancellation`]. The rule is per
    /// task, at the task's own acknowledgement, as the A7 conformance model has it (RFC
    /// 0026 correction 53, which relaxes correction 51's region-level rule).
    ///
    /// # Errors
    ///
    /// [`RegionFault::UnknownWorker`], [`RegionFault::SubstrateNotOpen`],
    /// [`RegionFault::SubstrateHolderMismatch`], [`RegionFault::SubstrateHolderNotLive`],
    /// [`RegionFault::SubstrateCommitDuringCancellation`] or
    /// [`RegionFault::SubstrateRegionNotOpen`] (a finalized region, defence in depth).
    pub fn discharge_substrate(
        &mut self,
        holder: WorkerId,
        obligation: SubstrateObligation,
        outcome: SubstrateOutcome,
    ) -> Result<(), RegionFault> {
        self.check_holder(holder, obligation)?;
        if outcome == SubstrateOutcome::Committed
            && self.cancel_phase(holder)? == CancelPhase::Acknowledged
        {
            return Err(RegionFault::SubstrateCommitDuringCancellation {
                obligation,
                worker: holder,
            });
        }
        if let Some(record) = self.substrate.get_mut(&obligation.id()) {
            record.open = false;
        }
        self.ledger.discharge(obligation.obligation());
        Ok(())
    }

    /// Hand an open adapter obligation from `from` to another worker `to`, which may be in
    /// another region.
    ///
    /// The obligation keeps its identity, so the ledger does not move: a transfer is not a
    /// discharge and an open. `from` must hold the obligation, and both parties must be
    /// *acting*: each [holds substrate](WorkerState::holds_substrate) — running or
    /// parked — and neither has acknowledged a cancellation. This is the A7 conformance
    /// model's rule: ownership transfer is a causal act by an acting holder to an acting
    /// receiver, and a task in cancellation only drains and finalizes. So, unlike a
    /// discharge, a transfer has no cancellation-cleanup exception. A worker whose
    /// cancellation is requested but not acknowledged still acts, and so does a worker in
    /// a normally closed region (RFC 0026 correction 53). Neither party's region may be
    /// finalized. `from == to` is refused: a hand-off to the holder itself hands nothing.
    ///
    /// # Errors
    ///
    /// [`RegionFault::UnknownWorker`], [`RegionFault::SubstrateNotOpen`] or
    /// [`RegionFault::SubstrateHolderMismatch`] for `from`;
    /// [`RegionFault::SubstrateSelfTransfer`]; and, for either party,
    /// [`RegionFault::SubstrateHolderNotBegun`], [`RegionFault::SubstrateHolderNotLive`],
    /// [`RegionFault::SubstrateHolderCancelling`] or [`RegionFault::SubstrateRegionNotOpen`]
    /// (a finalized region, defence in depth).
    pub fn transfer_substrate(
        &mut self,
        from: WorkerId,
        obligation: SubstrateObligation,
        to: WorkerId,
    ) -> Result<(), RegionFault> {
        self.check_holder(from, obligation)?;
        if from == to {
            return Err(RegionFault::SubstrateSelfTransfer {
                obligation,
                worker: from,
            });
        }
        self.check_acting(from)?;
        self.check_acting(to)?;
        if let Some(record) = self.substrate.get_mut(&obligation.id()) {
            record.holder = to;
        }
        if let Some(held) = self.held_by.get_mut(&from) {
            held.remove(&obligation.id());
        }
        self.held_by.entry(to).or_default().insert(obligation.id());
        Ok(())
    }

    /// The worker holding the adapter obligation with identity `id`, while it is open.
    #[must_use]
    pub fn substrate_holder(&self, id: SubstrateId) -> Option<WorkerId> {
        self.substrate
            .get(&id)
            .filter(|record| record.open)
            .map(|record| record.holder)
    }

    // --- request, drain, finalize ------------------------------------------------------

    /// Close `region` normally: no new work may enter it or any region beneath it.
    ///
    /// The request half of a *non-cancelling* teardown. It propagates to the subtree —
    /// a scope that has stopped accepting work cannot have a child that has not — but it
    /// never downgrades a descendant that a cancellation already reached, because
    /// cancellation is the stronger request and RFC 0026's monotonicity forbids walking
    /// it back.
    ///
    /// # Errors
    ///
    /// [`RegionFault::UnknownRegion`], or [`RegionFault::CloseNonOpenRegion`] when
    /// `region` is not [`RegionState::Open`].
    pub fn close(&mut self, region: RegionId) -> Result<(), RegionFault> {
        let state = self.node(region)?.state;
        if !state.accepts_work() {
            return Err(RegionFault::CloseNonOpenRegion { region, state });
        }
        for member in self.subtree(region)? {
            if self.node(member)?.state == RegionState::Open {
                self.node_mut(member)?.state = RegionState::Draining(DrainCause::Closed);
            }
        }
        Ok(())
    }

    /// Request cancellation of `region` and everything beneath it.
    ///
    /// > **`task.cancel` triggers request → drain → finalize.**
    /// >
    /// > — RFC 0026, "Task lifecycle"
    ///
    /// This is the *request*, and only the request: it stops new work entering the
    /// subtree, upgrades any descendant that was merely closed, and opens the
    /// [`CancellationFinalization`](obligation::ObligationKind::CancellationFinalization)
    /// obligation — RFC 0001's own named example — which stays outstanding until
    /// [`Self::finalize`] discharges it. A cancellation requested and never finalized is
    /// precisely the leak G0-DX-14 asks about, and it is visible in the ledger from this
    /// call onward.
    ///
    /// Requesting twice changes nothing, which is INV-002's "a dropped connection or
    /// restarted client does not change meaning" at this layer.
    ///
    /// # Errors
    ///
    /// [`RegionFault::UnknownRegion`], or [`RegionFault::CancelFinalizedRegion`] when the
    /// region has already been torn down.
    pub fn cancel(&mut self, region: RegionId) -> Result<(), RegionFault> {
        if self.node(region)?.state == RegionState::Finalized {
            return Err(RegionFault::CancelFinalizedRegion { region });
        }
        for member in self.subtree(region)? {
            if self.node(member)?.state != RegionState::Finalized {
                self.node_mut(member)?.state = RegionState::Draining(DrainCause::Cancelled);
                // The request reaches each live worker, and stays on it: its phase must
                // not be read back from a region state that finalize later replaces.
                // Setting a flag already set changes nothing, so a repeated cancel is
                // idempotent.
                for worker in self.node(member)?.workers.clone() {
                    let record = self.record_mut(worker)?;
                    if !record.state.is_terminal() {
                        record.region_requested = true;
                    }
                }
            }
        }
        self.ledger
            .open(Obligation::cancellation_finalization(region));
        Ok(())
    }

    /// Drain `region`'s subtree: drive every worker it owns to a terminal state.
    ///
    /// > - child workers receive cancellation;
    /// > - provisional streams close;
    /// > - committed partial artifacts are finalized;
    /// > - continuation is emitted if supported;
    /// > - task transitions exactly once to terminal/suspended state;
    /// > - obligations and resource leases are resolved.
    /// >
    /// > — `notes/plan/docs/35_CONTINUUMD_WORKBENCH_DAEMON.md`, "Cancellation"
    ///
    /// Under [`DrainCause::Cancelled`] all six happen here, in that order, per worker in
    /// ascending identity order: the staged publication is discarded so nothing
    /// half-published survives, the committed half is left alone because INV-009 makes it
    /// monotone, the [`CancelOutcome`] is computed from what is left, the worker
    /// transitions exactly once into [`WorkerState::Cancelled`], and its obligation is
    /// discharged.
    ///
    /// Under [`DrainCause::Closed`] nothing is forced: a worker that has not reached a
    /// terminal state is reported as [`RegionFault::DrainBlocked`], naming it. That is
    /// the honest translation of "wait for owned work" into a calculus with no waiting —
    /// the caller's schedule has more steps to take.
    ///
    /// Draining is idempotent: a subtree that is already terminal drains to itself.
    ///
    /// # Errors
    ///
    /// [`RegionFault::UnknownRegion`], [`RegionFault::DrainBeforeRequest`] when no
    /// request has been made, [`RegionFault::DrainFinalizedRegion`], or
    /// [`RegionFault::DrainBlocked`].
    pub fn drain(&mut self, region: RegionId) -> Result<(), RegionFault> {
        match self.node(region)?.state {
            RegionState::Open => return Err(RegionFault::DrainBeforeRequest { region }),
            RegionState::Finalized => return Err(RegionFault::DrainFinalizedRegion { region }),
            RegionState::Draining(_) => {}
        }
        for member in self.subtree(region)? {
            let Some(cause) = self.node(member)?.state.drain_cause() else {
                continue;
            };
            for worker in self.node(member)?.workers.clone() {
                let state = self.record(worker)?.state.clone();
                if state.is_terminal() {
                    continue;
                }
                if cause == DrainCause::Closed {
                    return Err(RegionFault::DrainBlocked {
                        region: member,
                        worker,
                        state,
                    });
                }
                self.cancel_worker(worker)?;
            }
        }
        Ok(())
    }

    /// Finalize `region`'s subtree, children first, and report what it left behind.
    ///
    /// The post-condition is the point of the whole module: when this returns [`Ok`],
    /// every worker ever spawned into the subtree is in a terminal state and every
    /// obligation the subtree opened has been discharged. [`Finalization::is_total`] is
    /// that sentence as a checkable value, carried by the report rather than re-derived
    /// by the caller.
    ///
    /// Children finalize before parents — the subtree is walked in descending identity
    /// order, and a child's ordinal is always allocated after its parent's — so a
    /// parent's
    /// [`ChildRegionQuiescence`](obligation::ObligationKind::ChildRegionQuiescence)
    /// obligation is discharged by an event that has already happened rather than by one
    /// that is assumed.
    ///
    /// # Errors
    ///
    /// [`RegionFault::UnknownRegion`], [`RegionFault::AlreadyFinalized`],
    /// [`RegionFault::FinalizeBeforeDrain`] when the region has had no request, or
    /// [`RegionFault::FinalizeBeforeTermination`] naming the first worker in the subtree
    /// that is not terminal.
    pub fn finalize(&mut self, region: RegionId) -> Result<Finalization, RegionFault> {
        match self.node(region)?.state {
            RegionState::Finalized => return Err(RegionFault::AlreadyFinalized { region }),
            state @ RegionState::Open => {
                return Err(RegionFault::FinalizeBeforeDrain { region, state });
            }
            RegionState::Draining(_) => {}
        }
        let members = self.subtree(region)?;
        for member in &members {
            for worker in self.node(*member)?.workers.clone() {
                let state = self.record(worker)?.state.clone();
                if !state.is_terminal() {
                    return Err(RegionFault::FinalizeBeforeTermination {
                        region: *member,
                        worker,
                        state,
                    });
                }
            }
        }
        for member in members.iter().rev() {
            if self.node(*member)?.state == RegionState::Finalized {
                continue;
            }
            self.node_mut(*member)?.state = RegionState::Finalized;
            if self.node(*member)?.parent.is_some() {
                self.ledger
                    .discharge(Obligation::child_region_quiescence(*member));
            }
            self.ledger
                .discharge(Obligation::cancellation_finalization(*member));
        }
        self.report(region, &members)
    }

    /// Cancel, drain and finalize `region` in one call — the total teardown.
    ///
    /// The three-step protocol exists because RFC 0026 names three steps and a daemon has
    /// to be able to observe each one. When nothing needs to be observed in between, this
    /// is the whole of it, and it is *total* in the sense that matters: for any region
    /// that exists and has not already been finalized, it succeeds, whatever state the
    /// subtree's workers are in and whatever the schedule did to get them there. The only
    /// two faults it can return are [`RegionFault::UnknownRegion`] and
    /// [`RegionFault::CancelFinalizedRegion`], and both are statements about the *region
    /// argument*, never about the work inside.
    ///
    /// # Errors
    ///
    /// [`RegionFault::UnknownRegion`] or [`RegionFault::CancelFinalizedRegion`].
    pub fn teardown(&mut self, region: RegionId) -> Result<Finalization, RegionFault> {
        self.cancel(region)?;
        self.drain(region)?;
        self.finalize(region)
    }

    // --- internals ---------------------------------------------------------------------

    /// The subtree's own obligations (RFC 0026 correction 53): those about its regions
    /// and its workers, and the adapter obligations its workers hold now (or held last,
    /// once discharged). Each is named by key, so a finalization costs the size of its
    /// subtree, not the size of the ledger's history. The same set, stated as a filter
    /// over the whole history, is [`Self::in_subtree_obligation`].
    fn subtree_obligations(&self, members: &[RegionId]) -> Result<Vec<Obligation>, RegionFault> {
        let mut out = Vec::new();
        for member in members {
            out.push(Obligation::child_region_quiescence(*member));
            out.push(Obligation::cancellation_finalization(*member));
            for worker in &self.node(*member)?.workers {
                out.push(Obligation::worker_termination(*worker));
                out.push(Obligation::provisional_publication(*worker));
                out.extend(self.ledger.publications_of(*worker));
                for id in self.held_by.get(worker).into_iter().flatten() {
                    if let Some(record) = self.substrate.get(id) {
                        out.push(record.obligation.obligation());
                    }
                }
            }
        }
        Ok(out)
    }

    /// Whether `obligation` is one of the subtree's own, as a predicate: the reference
    /// statement of [`Self::subtree_obligations`].
    #[cfg(test)]
    fn in_subtree_obligation(&self, members: &[RegionId], obligation: &Obligation) -> bool {
        let in_subtree = |region: RegionId| members.contains(&region);
        let worker_in = |worker: WorkerId| {
            self.record(worker)
                .is_ok_and(|record| in_subtree(record.region))
        };
        match obligation.subject() {
            obligation::Subject::Region(region) => in_subtree(region),
            obligation::Subject::Worker(worker) | obligation::Subject::Publication(worker, _) => {
                worker_in(worker)
            }
            obligation::Subject::Substrate(id) => self
                .substrate
                .get(&id)
                .is_some_and(|record| worker_in(record.holder)),
        }
    }

    /// `worker` exists and may take part in an adapter-obligation operation: it
    /// [holds substrate](WorkerState::holds_substrate) — it has begun and has not
    /// terminated — and its region is not finalized. Returns that region's state.
    ///
    /// The region check is defence in depth: [`Self::finalize`] refuses while any worker
    /// in the subtree is not terminal, so a worker that holds substrate is never in a
    /// finalized region.
    fn check_substrate_party(&self, worker: WorkerId) -> Result<RegionState, RegionFault> {
        let record = self.record(worker)?;
        if !record.state.holds_substrate() {
            return Err(if record.state == WorkerState::Created {
                RegionFault::SubstrateHolderNotBegun { worker }
            } else {
                RegionFault::SubstrateHolderNotLive {
                    worker,
                    state: record.state.clone(),
                }
            });
        }
        let region = record.region;
        let state = self.node(region)?.state;
        if state == RegionState::Finalized {
            return Err(RegionFault::SubstrateRegionNotOpen {
                worker,
                region,
                state,
            });
        }
        Ok(state)
    }

    /// A substrate party that is *acting*: it has not acknowledged a cancellation. The
    /// rule for both parties of a transfer, and for an opener. Returns its region state.
    fn check_acting(&self, worker: WorkerId) -> Result<RegionState, RegionFault> {
        let state = self.check_substrate_party(worker)?;
        if self.cancel_phase(worker)? == CancelPhase::Acknowledged {
            return Err(RegionFault::SubstrateHolderCancelling { worker });
        }
        Ok(state)
    }

    /// The rule for opening an adapter obligation: an acting substrate party whose
    /// region is [`RegionState::Open`].
    fn check_opener(&self, worker: WorkerId) -> Result<(), RegionFault> {
        let state = self.check_acting(worker)?;
        if !state.accepts_work() {
            return Err(RegionFault::SubstrateRegionNotOpen {
                worker,
                region: self.record(worker)?.region,
                state,
            });
        }
        Ok(())
    }

    /// `obligation` is open with this kind, `named` holds it, and `named` is a substrate
    /// party — the rule for discharging an adapter obligation, and the holder half of a
    /// transfer. Returns the holder's region state. The region may be open or draining: a cancellation drain is where
    /// cleanup discharges happen. A transfer then adds [`Self::check_acting`].
    fn check_holder(
        &self,
        named: WorkerId,
        obligation: SubstrateObligation,
    ) -> Result<RegionState, RegionFault> {
        self.record(named)?;
        let held = self
            .substrate
            .get(&obligation.id())
            .filter(|held| held.open && held.obligation == obligation)
            .ok_or(RegionFault::SubstrateNotOpen { obligation })?;
        if held.holder != named {
            return Err(RegionFault::SubstrateHolderMismatch {
                obligation,
                named,
                holder: held.holder,
            });
        }
        self.check_substrate_party(named)
    }

    fn node(&self, region: RegionId) -> Result<&RegionNode, RegionFault> {
        self.regions
            .get(region.ordinal() as usize)
            .ok_or(RegionFault::UnknownRegion(region))
    }

    fn node_mut(&mut self, region: RegionId) -> Result<&mut RegionNode, RegionFault> {
        self.regions
            .get_mut(region.ordinal() as usize)
            .ok_or(RegionFault::UnknownRegion(region))
    }

    fn record(&self, worker: WorkerId) -> Result<&WorkerRecord, RegionFault> {
        self.workers
            .get(worker.ordinal() as usize)
            .ok_or(RegionFault::UnknownWorker(worker))
    }

    fn record_mut(&mut self, worker: WorkerId) -> Result<&mut WorkerRecord, RegionFault> {
        self.workers
            .get_mut(worker.ordinal() as usize)
            .ok_or(RegionFault::UnknownWorker(worker))
    }

    /// Drop any staged publication, discharging its obligation.
    ///
    /// The store-side counterpart is `StagedPublication::abandon`: "no index entry, no
    /// receipt, nothing a reader can observe". Committed publications are untouched,
    /// because INV-009 makes them monotone.
    fn discard_provisional(&mut self, worker: WorkerId) -> Result<(), RegionFault> {
        let record = self.record_mut(worker)?;
        let slots = core::mem::take(&mut record.slots);
        record.evidence.discard();
        for slot in slots {
            self.ledger
                .discharge(Obligation::staged_publication(worker, slot));
        }
        Ok(())
    }

    /// Stage a publication in `slot`, opening its obligation. The caller has checked the
    /// step's guard.
    fn stage(&mut self, worker: WorkerId, slot: PublicationSlot) -> Result<(), RegionFault> {
        let record = self.record_mut(worker)?;
        record.slots.insert(slot);
        record.evidence.reserve();
        self.ledger
            .open(Obligation::staged_publication(worker, slot));
        Ok(())
    }

    /// Resolve the publication staged in `slot` — committed or dropped — discharging its
    /// obligation. The caller has checked that `slot` is staged.
    fn resolve(
        &mut self,
        worker: WorkerId,
        slot: PublicationSlot,
        commit: bool,
    ) -> Result<(), RegionFault> {
        let record = self.record_mut(worker)?;
        record.slots.remove(&slot);
        if commit {
            record.evidence.commit();
        } else {
            record.evidence.abort();
        }
        self.ledger
            .discharge(Obligation::staged_publication(worker, slot));
        Ok(())
    }

    /// Terminate one non-terminal worker by cancellation: discard what it staged, record
    /// what the cancellation leaves behind, and settle it into
    /// [`WorkerState::Cancelled`].
    ///
    /// The per-worker body of a cancelling drain, and the one place a worker is cancelled.
    /// A single-task cancellation step (bn-36wy3's deadline cancellation) is this function
    /// behind its own legality guard, not a second copy of it.
    fn cancel_worker(&mut self, worker: WorkerId) -> Result<(), RegionFault> {
        self.discard_provisional(worker)?;
        let outcome = self.cancel_outcome(worker)?;
        self.record_mut(worker)?.cancel_outcome = Some(outcome);
        self.settle(worker, WorkerState::Cancelled)
    }

    /// What a cancellation leaves behind for one worker — RFC 0026's both-or-neither.
    fn cancel_outcome(&self, worker: WorkerId) -> Result<CancelOutcome, RegionFault> {
        let record = self.record(worker)?;
        if !record.evidence.has_committed_evidence() {
            return Ok(CancelOutcome::NothingPublished);
        }
        Ok(match &record.resumability {
            Resumability::Resumable => CancelOutcome::CommittedWithContinuation(Continuation::new(
                worker,
                record.evidence.committed(),
            )),
            Resumability::NonResumable(reason) => {
                CancelOutcome::CommittedNonResumable(reason.clone())
            }
        })
    }

    /// Write a worker's new state, discharging its termination obligation on — and only
    /// on — the transition into a terminal state.
    fn settle(&mut self, worker: WorkerId, landed: WorkerState) -> Result<(), RegionFault> {
        // A cancelled worker's history includes a request: `Cancelled ⇒ phase ≠ Active`.
        debug_assert!(
            landed != WorkerState::Cancelled
                || self
                    .cancel_phase(worker)
                    .is_ok_and(|phase| phase != CancelPhase::Active),
            "{worker} settles as cancelled with no cancellation requested"
        );
        let terminal = landed.is_terminal();
        self.record_mut(worker)?.state = landed;
        if terminal {
            self.ledger
                .discharge(Obligation::worker_termination(worker));
        }
        Ok(())
    }

    fn report(&self, region: RegionId, members: &[RegionId]) -> Result<Finalization, RegionFault> {
        let mut workers = Vec::new();
        for member in members {
            for worker in &self.node(*member)?.workers {
                let record = self.record(*worker)?;
                workers.push(WorkerReport {
                    worker: *worker,
                    region: record.region,
                    state: record.state.clone(),
                    evidence: record.evidence,
                    cancel_outcome: record.cancel_outcome.clone(),
                });
            }
        }
        workers.sort_by_key(|report| report.worker);
        let ledger = self.ledger.summary_of(self.subtree_obligations(members)?);
        Ok(Finalization {
            region,
            regions: members.to_vec(),
            workers,
            ledger,
            tree_ledger: self.ledger.summary(),
        })
    }
}

/// What one worker was, at the moment its region finalized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerReport {
    worker: WorkerId,
    region: RegionId,
    state: WorkerState,
    evidence: EvidenceLedger,
    cancel_outcome: Option<CancelOutcome>,
}

impl WorkerReport {
    /// Which worker.
    #[must_use]
    pub const fn worker(&self) -> WorkerId {
        self.worker
    }

    /// Which region owned it.
    #[must_use]
    pub const fn region(&self) -> RegionId {
        self.region
    }

    /// The state it ended in. Always terminal, by [`RegionTree::finalize`]'s
    /// pre-condition.
    #[must_use]
    pub const fn state(&self) -> &WorkerState {
        &self.state
    }

    /// What it published and what it still held.
    #[must_use]
    pub const fn evidence(&self) -> EvidenceLedger {
        self.evidence
    }

    /// What cancellation left behind, when cancellation is what ended it.
    ///
    /// [`None`] for a worker that completed or failed on its own: those are not
    /// cancellation outcomes, and reporting one for them would invent a continuation
    /// question nobody asked.
    #[must_use]
    pub const fn cancel_outcome(&self) -> Option<&CancelOutcome> {
        self.cancel_outcome.as_ref()
    }

    /// The continuation this worker's cancellation minted, if any.
    #[must_use]
    pub fn continuation(&self) -> Option<Continuation> {
        self.cancel_outcome
            .as_ref()
            .and_then(CancelOutcome::continuation)
    }

    /// One canonical line for [`Finalization::render`].
    #[must_use]
    pub fn render(&self) -> String {
        let outcome = self
            .cancel_outcome
            .as_ref()
            .map_or_else(|| "-".to_owned(), ToString::to_string);
        format!(
            "{} region={} state={} committed={} provisional={} outcome={}",
            self.worker,
            self.region,
            self.state,
            self.evidence.committed(),
            self.evidence.is_provisional(),
            outcome
        )
    }
}

/// The report [`RegionTree::finalize`] hands back: what the subtree was, and the evidence
/// that nothing was left running.
///
/// A value rather than a log line, because the no-orphan claim has to be checkable by
/// the caller and by a test without either of them re-deriving it from the tree. The
/// three questions it answers are [`Self::orphans`] (which workers were left
/// non-terminal — always none), [`Self::unresolved_publications`] (which artifacts were
/// left neither committed nor absent — always none), and
/// [`Self::ledger`] (what was still owed — always nothing).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finalization {
    region: RegionId,
    regions: Vec<RegionId>,
    workers: Vec<WorkerReport>,
    ledger: LedgerSummary,
    tree_ledger: LedgerSummary,
}

impl Finalization {
    /// The region that was finalized.
    #[must_use]
    pub const fn region(&self) -> RegionId {
        self.region
    }

    /// Every region in the finalized subtree, in ascending identity order.
    #[must_use]
    pub fn regions(&self) -> &[RegionId] {
        &self.regions
    }

    /// Every worker the subtree owned, in ascending identity order.
    #[must_use]
    pub fn workers(&self) -> &[WorkerReport] {
        &self.workers
    }

    /// What the finalized subtree owed when it finalized: its own obligations only.
    ///
    /// Scoped to the subtree (RFC 0026 correction 53): the obligations about its regions
    /// and its workers, and the adapter obligations its workers held at that moment.
    /// Work elsewhere in the tree that still owes something does not make this subtree's
    /// teardown partial, and an obligation this subtree still owes is never hidden by
    /// balance elsewhere. The counters are summed over the same obligations. The whole
    /// tree's ledger is [`Self::tree_ledger`].
    #[must_use]
    pub const fn ledger(&self) -> &LedgerSummary {
        &self.ledger
    }

    /// What the whole tree's obligation ledger held when the subtree finalized. For a
    /// root finalization it equals [`Self::ledger`].
    #[must_use]
    pub const fn tree_ledger(&self) -> &LedgerSummary {
        &self.tree_ledger
    }

    /// The workers left in a non-terminal state — the orphans.
    ///
    /// Always empty, because [`RegionTree::finalize`] refuses otherwise. It is computed
    /// and reported anyway: a post-condition that is only enforced at the entry point is
    /// a post-condition nobody downstream can check, and G0-DX-14 asks for an
    /// instrumented answer rather than an assurance.
    #[must_use]
    pub fn orphans(&self) -> Vec<WorkerId> {
        self.workers
            .iter()
            .filter(|report| !report.state.is_terminal())
            .map(WorkerReport::worker)
            .collect()
    }

    /// The workers still holding a staged publication — artifacts that are neither
    /// committed nor absent.
    ///
    /// The second conjunct of G0-DX-14's pass condition, at this layer. Always empty.
    #[must_use]
    pub fn unresolved_publications(&self) -> Vec<WorkerId> {
        self.workers
            .iter()
            .filter(|report| report.evidence.is_provisional())
            .map(WorkerReport::worker)
            .collect()
    }

    /// The continuations this teardown minted, in worker order.
    ///
    /// Each one belongs to a worker that held committed partial evidence when
    /// cancellation reached it — RFC 0026's "committed partial evidence plus a valid
    /// continuation" arm. A worker that published nothing has none, and that is the other
    /// arm rather than a missing field.
    #[must_use]
    pub fn continuations(&self) -> Vec<Continuation> {
        self.workers
            .iter()
            .filter_map(WorkerReport::continuation)
            .collect()
    }

    /// Whether the teardown was total: no orphan, no unresolved publication, and a
    /// balanced ledger for the finalized subtree ([`Self::ledger`]).
    ///
    /// This is the property the module exists to make true. It is deliberately a
    /// conjunction of three *independently computed* facts rather than one flag: the
    /// orphan set is read off worker states, the unresolved set off evidence ledgers, and
    /// the balance off the obligation ledger, so a bug that fooled one accounting still
    /// has two to get past.
    #[must_use]
    pub fn is_total(&self) -> bool {
        self.orphans().is_empty()
            && self.unresolved_publications().is_empty()
            && self.ledger.is_balanced()
    }

    /// One canonical text rendering, for byte-identity comparison.
    ///
    /// Every line is a fact in a fixed order with no timestamp, address or iteration
    /// order in it, so two runs of one schedule render byte-identically and two different
    /// schedules render differently exactly when they differ (INV-005). This is the same
    /// device `continuum-engine-reference`'s contract tests use — comparing rendered
    /// bytes rather than `==` on a collection, because bytes catch an ordering difference
    /// that set equality hides.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = format!("finalized {}\n", self.region);
        out.push_str("regions:");
        for region in &self.regions {
            out.push(' ');
            out.push_str(&region.to_string());
        }
        out.push('\n');
        for report in &self.workers {
            out.push_str(&report.render());
            out.push('\n');
        }
        out.push_str(&format!("ledger: {}\n", self.ledger));
        out.push_str(&format!(
            "total: orphans={} unresolved={} balanced={}\n",
            self.orphans().len(),
            self.unresolved_publications().len(),
            self.ledger.is_balanced()
        ));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::worker::{FailureReason, NonResumableReason};
    use super::*;

    fn resumable() -> Resumability {
        Resumability::Resumable
    }

    fn non_resumable(token: &str) -> Resumability {
        Resumability::NonResumable(NonResumableReason::new(token).expect("test token is canonical"))
    }

    fn failure(token: &str) -> WorkerStep {
        WorkerStep::Fail(FailureReason::new(token).expect("test token is canonical"))
    }

    /// The keyed subtree ledger equals the same ledger stated as a filter over the whole
    /// history, for every region of a tree with nested regions, staging slots, and
    /// adapter obligations opened, transferred across regions, and discharged.
    #[test]
    fn the_keyed_subtree_ledger_is_the_filtered_one() {
        use obligation::{SubstrateKind, SubstrateObligation, SubstrateOutcome};
        let kind = SubstrateKind::new("lease").expect("canonical");
        let lease = |id| SubstrateObligation::new(kind, SubstrateId::at(id));
        let mut tree = RegionTree::new();
        let root = tree.root();
        let left = tree.open_child(root).expect("open");
        let right = tree.open_child(root).expect("open");
        let deep = tree.open_child(left).expect("open");
        let mut workers = Vec::new();
        for region in [root, left, right, deep, deep] {
            let worker = tree.spawn(region, Resumability::Resumable).expect("open");
            tree.advance(worker, WorkerStep::Begin).expect("legal");
            workers.push(worker);
        }
        tree.advance(workers[3], WorkerStep::ReserveSlot(PublicationSlot::at(2)))
            .expect("legal");
        tree.advance(workers[3], WorkerStep::CommitSlot(PublicationSlot::at(2)))
            .expect("legal");
        tree.advance(workers[4], WorkerStep::Reserve)
            .expect("legal");
        tree.open_substrate(workers[1], lease(0)).expect("legal");
        tree.open_substrate(workers[3], lease(1)).expect("legal");
        tree.transfer_substrate(workers[3], lease(1), workers[2])
            .expect("legal");
        tree.open_substrate(workers[2], lease(2)).expect("legal");
        tree.discharge_substrate(workers[2], lease(2), SubstrateOutcome::Committed)
            .expect("legal");
        tree.cancel(left).expect("open");
        for region in [root, left, right, deep] {
            let members = tree.subtree(region).expect("known");
            let keyed = tree
                .ledger
                .summary_of(tree.subtree_obligations(&members).expect("known"));
            let filtered = tree
                .ledger
                .summary_where(|o| tree.in_subtree_obligation(&members, o));
            assert_eq!(keyed, filtered, "{region}");
        }
        assert!(!tree.ledger().is_balanced(), "the fixture owes something");
    }

    #[test]
    fn a_fresh_tree_holds_one_open_root_and_owes_nothing() {
        let tree = RegionTree::new();
        assert_eq!(tree.state(tree.root()), Ok(RegionState::Open));
        assert_eq!(tree.worker_count(), 0);
        assert!(tree.ledger().is_balanced());
    }

    #[test]
    fn spawning_opens_exactly_one_termination_obligation() {
        let mut tree = RegionTree::new();
        let root = tree.root();
        let worker = tree.spawn(root, resumable()).expect("root is open");
        assert_eq!(tree.ledger().opened(), 1);
        assert!(
            tree.ledger()
                .holds(obligation::Obligation::worker_termination(worker))
        );
        assert!(!tree.ledger().is_balanced());
    }

    #[test]
    fn work_cannot_enter_a_region_that_has_stopped_accepting_it() {
        let mut tree = RegionTree::new();
        let root = tree.root();
        tree.close(root).expect("root is open");
        assert_eq!(
            tree.spawn(root, resumable()),
            Err(RegionFault::SpawnIntoClosedRegion {
                region: root,
                state: RegionState::Draining(DrainCause::Closed),
            })
        );
        assert_eq!(
            tree.open_child(root),
            Err(RegionFault::OpenChildInClosedRegion {
                region: root,
                state: RegionState::Draining(DrainCause::Closed),
            })
        );
    }

    #[test]
    fn cancellation_reaches_every_region_beneath_the_one_it_named() {
        let mut tree = RegionTree::new();
        let root = tree.root();
        let child = tree.open_child(root).expect("root is open");
        let grandchild = tree.open_child(child).expect("child is open");
        tree.cancel(root).expect("root is not finalized");
        for region in [root, child, grandchild] {
            assert_eq!(
                tree.state(region),
                Ok(RegionState::Draining(DrainCause::Cancelled)),
                "{region} did not receive the cancellation"
            );
        }
    }

    #[test]
    fn cancellation_upgrades_a_descendant_that_was_only_closed() {
        let mut tree = RegionTree::new();
        let root = tree.root();
        let child = tree.open_child(root).expect("root is open");
        tree.close(child).expect("child is open");
        assert_eq!(
            tree.state(child),
            Ok(RegionState::Draining(DrainCause::Closed))
        );
        tree.cancel(root).expect("root is not finalized");
        assert_eq!(
            tree.state(child),
            Ok(RegionState::Draining(DrainCause::Cancelled)),
            "cancellation is the stronger request and must not be downgraded"
        );
    }

    #[test]
    fn a_normal_close_names_the_worker_the_drain_is_still_waiting_on() {
        let mut tree = RegionTree::new();
        let root = tree.root();
        let worker = tree.spawn(root, resumable()).expect("root is open");
        tree.advance(worker, WorkerStep::Begin).expect("legal");
        tree.close(root).expect("root is open");
        assert_eq!(
            tree.drain(root),
            Err(RegionFault::DrainBlocked {
                region: root,
                worker,
                state: WorkerState::Running,
            })
        );
        tree.advance(worker, WorkerStep::Complete).expect("legal");
        tree.drain(root).expect("the worker is terminal now");
        let finalization = tree.finalize(root).expect("drained");
        assert!(finalization.is_total());
    }

    #[test]
    fn finalize_refuses_while_a_worker_is_parked_because_suspended_is_not_terminal() {
        let mut tree = RegionTree::new();
        let root = tree.root();
        let worker = tree.spawn(root, resumable()).expect("root is open");
        tree.advance(worker, WorkerStep::Begin).expect("legal");
        tree.advance(worker, WorkerStep::Suspend).expect("legal");
        tree.close(root).expect("root is open");
        assert_eq!(
            tree.drain(root),
            Err(RegionFault::DrainBlocked {
                region: root,
                worker,
                state: WorkerState::Suspended,
            }),
            "a parked worker is quiescent but not terminal, and finalize needs terminal"
        );
    }

    #[test]
    fn teardown_is_total_over_a_subtree_of_workers_in_every_reachable_state() {
        let mut tree = RegionTree::new();
        let root = tree.root();
        let child = tree.open_child(root).expect("root is open");

        let created = tree.spawn(root, resumable()).expect("open");
        let running = tree.spawn(root, resumable()).expect("open");
        let parked = tree.spawn(child, resumable()).expect("open");
        let done = tree.spawn(child, resumable()).expect("open");
        let broken = tree
            .spawn(child, non_resumable("engine-defect"))
            .expect("open");

        tree.advance(running, WorkerStep::Begin).expect("legal");
        tree.advance(running, WorkerStep::Reserve).expect("legal");
        tree.advance(parked, WorkerStep::Begin).expect("legal");
        tree.advance(parked, WorkerStep::Reserve).expect("legal");
        tree.advance(parked, WorkerStep::Commit).expect("legal");
        tree.advance(parked, WorkerStep::Suspend).expect("legal");
        tree.advance(done, WorkerStep::Begin).expect("legal");
        tree.advance(done, WorkerStep::Complete).expect("legal");
        tree.advance(broken, WorkerStep::Begin).expect("legal");
        tree.advance(broken, WorkerStep::Reserve).expect("legal");
        tree.advance(broken, WorkerStep::Commit).expect("legal");

        let finalization = tree
            .teardown(root)
            .expect("root exists and is not finalized");
        assert!(finalization.is_total(), "{}", finalization.render());
        assert_eq!(finalization.workers().len(), 5);
        assert_eq!(tree.worker_count(), 5);

        let by_worker = |id: WorkerId| {
            finalization
                .workers()
                .iter()
                .find(|report| report.worker() == id)
                .expect("every spawned worker is reported")
        };
        assert_eq!(by_worker(created).state(), &WorkerState::Cancelled);
        assert_eq!(
            by_worker(created).cancel_outcome(),
            Some(&CancelOutcome::NothingPublished)
        );
        assert_eq!(
            by_worker(running).cancel_outcome(),
            Some(&CancelOutcome::NothingPublished),
            "a staged publication is discarded, so nothing was published"
        );
        assert_eq!(
            by_worker(parked).cancel_outcome(),
            Some(&CancelOutcome::CommittedWithContinuation(
                Continuation::new(parked, 1)
            ))
        );
        assert_eq!(
            by_worker(done).state(),
            &WorkerState::Completed,
            "a worker that already terminated is not re-terminated"
        );
        assert_eq!(by_worker(done).cancel_outcome(), None);
        assert_eq!(
            by_worker(broken).cancel_outcome(),
            Some(&CancelOutcome::CommittedNonResumable(
                NonResumableReason::new("engine-defect").expect("canonical")
            ))
        );
    }

    #[test]
    fn a_failed_worker_discards_its_staged_publication_and_keeps_its_committed_one() {
        let mut tree = RegionTree::new();
        let root = tree.root();
        let worker = tree.spawn(root, resumable()).expect("open");
        tree.advance(worker, WorkerStep::Begin).expect("legal");
        tree.advance(worker, WorkerStep::Reserve).expect("legal");
        tree.advance(worker, WorkerStep::Commit).expect("legal");
        tree.advance(worker, WorkerStep::Reserve).expect("legal");
        tree.advance(worker, failure("budget-exhausted"))
            .expect("legal");
        let evidence = tree.evidence(worker).expect("known worker");
        assert_eq!(evidence.committed(), 1, "INV-009: committed is monotone");
        assert!(!evidence.is_provisional());
        assert!(tree.ledger().is_balanced());
    }

    #[test]
    fn advancing_a_worker_whose_region_is_finalized_is_refused() {
        let mut tree = RegionTree::new();
        let root = tree.root();
        let worker = tree.spawn(root, resumable()).expect("open");
        tree.teardown(root).expect("torn down");
        assert_eq!(
            tree.advance(worker, WorkerStep::Begin),
            Err(RegionFault::AdvanceInFinalizedRegion {
                worker,
                region: root
            })
        );
    }

    #[test]
    fn finalizing_twice_says_so_rather_than_inventing_a_second_report() {
        let mut tree = RegionTree::new();
        let root = tree.root();
        tree.teardown(root).expect("torn down");
        assert_eq!(
            tree.finalize(root),
            Err(RegionFault::AlreadyFinalized { region: root })
        );
        assert_eq!(
            tree.cancel(root),
            Err(RegionFault::CancelFinalizedRegion { region: root })
        );
    }

    #[test]
    fn the_render_of_one_teardown_is_byte_identical_across_two_runs() {
        let build = || {
            let mut tree = RegionTree::new();
            let root = tree.root();
            let child = tree.open_child(root).expect("open");
            let one = tree.spawn(root, resumable()).expect("open");
            let two = tree.spawn(child, resumable()).expect("open");
            tree.advance(one, WorkerStep::Begin).expect("legal");
            tree.advance(two, WorkerStep::Begin).expect("legal");
            tree.advance(two, WorkerStep::Reserve).expect("legal");
            tree.advance(two, WorkerStep::Commit).expect("legal");
            tree.teardown(root).expect("torn down").render()
        };
        let first = build();
        let second = build();
        assert_eq!(first.as_bytes(), second.as_bytes());
        assert!(!first.is_empty(), "a vacuous render proves nothing");
    }
}
