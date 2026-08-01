//! Where the daemon's task work runs: one region per unit of work, torn down inside the
//! dispatch that opened it (PR 6 / IMPL-01, IMPL-04).
//!
//! > Search, proof, synthesis, indexing, and debugging tasks run under asupersync regions
//! > and publish only committed artifacts. Cancellation cannot leave false finality or
//! > orphan work.
//! >
//! > — `notes/plan/plan.md` §3
//!
//! [`continuum_task::region`] is the calculus that sentence names — a tree of regions
//! owning workers, RFC 0026's request → drain → finalize, and an obligation ledger that
//! makes "no orphan workers" a checked property. bn-2gk landed it with the daemon seam
//! designed and deliberately not wired; this module is the wiring, and it is small on
//! purpose: the semantics live one crate down, and what happens here is that the daemon's
//! units of work are *put inside* them.
//!
//! # The rule, in one sentence
//!
//! **Every unit of task work runs inside a region that is opened and finalized within one
//! [`Daemon::dispatch`](super::Daemon::dispatch).** [`scoped`] is the only way to open one:
//! [`TaskRegions::open`] and [`TaskRegions::settle`] are private to this module, and
//! `scoped` calls the second on every path out of the first — the value path, the
//! [`Fault`] path, and the path where the work never moved its worker at all. So "no
//! region outlives its dispatch" is a property of what compiles rather than of care taken,
//! and [`TaskRegions::is_total`] is the checkable form of it: after any dispatch sequence,
//! every region this daemon opened is finalized and every worker it admitted is terminal.
//!
//! # A parked continuation is not a live worker
//!
//! This is the one design question the rewiring had to answer, and it had to be answered
//! before either layer could be bent. [`RegionTree::finalize`] requires every owned worker
//! to be **terminal**, not merely quiescent — a parked [`WorkerState::Suspended`] worker
//! blocks it, because "a finalized region that still owns a resumable worker is a
//! continuation pointing into a scope that no longer exists". The daemon parks
//! continuations. If a parked continuation were a live worker, its region could never be
//! finalized while the task stayed suspended, and the totality claim above would be false
//! for every task a client parks and forgets.
//!
//! It is not one, and RFC 0026 says so four times over:
//!
//! - **"Handles remain valid across daemon upgrades within a protocol major."** A
//!   continuation survives a process restart. No live worker does.
//! - **Resume validates before it reuses.** `task.resume` re-checks the pinned snapshot,
//!   P1 over the compatibility epochs, and P2 over engine identity — "the daemon MUST NOT
//!   quietly restart the task under current epochs". A scope that was never left needs no
//!   re-admission; the predicate exists precisely because a continuation is detached state
//!   being admitted again, later, possibly by a different build.
//! - **"Continuations are forked across an epoch advance, never migrated in place. A fork
//!   produces a new continuation with a new identity; the original remains valid for the
//!   epochs it pinned."** That is a value being copied, not a scope being handed over.
//! - **"A continuation MUST leave `protocol` `Unpinned`: the protocol epoch is
//!   connection-scoped and is consumed by no task."** A continuation outlives the
//!   connection that minted it.
//!
//! So the resolution this module implements is: **a parked continuation is a durable wire
//! artifact, and the region-layer [`WorkerState::Suspended`] is only ever transient inside
//! one dispatch.** A campaign that trips its bound parks — the worker takes
//! [`WorkerStep::Suspend`], which is where the region layer's "you may not park
//! mid-publication" guard applies — and then the scope ends, the parked execution is
//! terminated by the scope exit, and what survives is the `cont_*` the daemon minted and
//! filed in its task table. `task.resume` validates that artifact and spawns a **new**
//! worker in a **new** region; [`TaskEntry::region`](super::task::TaskEntry::region) shows
//! the difference, because a resumed task's region is not the one it parked in.
//!
//! The alternative — keeping a region open across dispatches so the parked worker stays
//! alive — was rejected on the same evidence. It would make a task that is never resumed
//! and never cancelled hold an open region for the daemon's lifetime, which is exactly the
//! leak G0-DX-14 asks about ("a cancellation requested and never finalized"), and it would
//! reduce the totality property to a claim about the tasks a client happened to finish.
//!
//! # The region's `Continuation` is not a `cont_*` handle
//!
//! [`continuum_task::region::worker::Continuation`] carries a worker identity and a
//! committed count. RFC 0026's continuation pins the snapshot, the intent, the frontier,
//! five compatibility epochs and engine identity, and that one is
//! [`task::Continuation`](super::task::Continuation) — minted here, in the daemon, from
//! values the region layer cannot see. The region-layer value is **bookkeeping**: it
//! decides which *arm* of the cancellation table an answer is on, and the daemon supplies
//! the handle. A disagreement between the two is a defect, not a fallback, and
//! `tests/daemon_task_regions.rs` asserts the agreement rather than assuming it.
//!
//! # The teardown discipline
//!
//! [`TaskRegions::settle`] closes the region, drains it, and finalizes it — the three
//! operations in RFC 0026's own order. The drain is the interesting step: under
//! [`DrainCause::Closed`](continuum_task::region::DrainCause::Closed) a worker that is
//! still owed a terminal state is a typed
//! [`RegionFault::DrainBlocked`](continuum_task::region::RegionFault::DrainBlocked), and
//! that refusal is what escalates the teardown to a cancellation. So a scope exit is a
//! *close* when the work finished on its own and a *cancel* when it did not, and neither
//! is chosen by a flag someone set: the region layer decides it, from the worker's state.
//!
//! # What is retained, and why it grows
//!
//! The tree keeps every region and worker the daemon ever opened, and the ledger keeps
//! every [`Finalization`]. Both grow with the dispatch count, exactly as
//! [`DaemonState::admissions`](super::state::DaemonState::admissions) does — one audit
//! record per request, retained for the same reason: this daemon's state is an explicit
//! value a test can read, and a teardown nobody can inspect afterwards proves nothing.
//! Bounding the three is one question and it is not this bone's.

use continuum_task::region::worker::{
    CancelOutcome, Continuation as RegionContinuation, FailureReason, NonResumableReason,
    Resumability, WorkerId, WorkerState, WorkerStep,
};
use continuum_task::region::{Finalization, RegionFault, RegionId, RegionState, RegionTree};

use super::family::Fault;
use super::state::DaemonState;
use crate::protocol::vocabulary::ErrorCode;

/// The scope one unit of task work runs in: the region that owns the work, and the worker
/// that *is* the work.
///
/// A pair of dense ordinals and nothing else — no pointer, no lock, no handle to a running
/// thing. It is [`Copy`] because it names a position in the daemon's own region tree, and
/// naming one twice is not a second scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Scope {
    region: RegionId,
    worker: WorkerId,
}

impl Scope {
    /// The region that owns this unit of work.
    #[must_use]
    pub const fn region(self) -> RegionId {
        self.region
    }

    /// The worker this unit of work is.
    #[must_use]
    pub const fn worker(self) -> WorkerId {
        self.worker
    }
}

/// What a scope exit established about the work it owned.
///
/// Read off the [`Finalization`] the teardown produced rather than re-derived: the three
/// facts a caller needs are the terminal state the work reached, the typed
/// [`CancelOutcome`] if a cancellation is what ended it, and whether the teardown was
/// total. The report itself stays in [`TaskRegions::finalizations`], where a test can hold
/// the whole subtree to the same claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settlement {
    region: RegionId,
    worker: WorkerId,
    state: WorkerState,
    outcome: Option<CancelOutcome>,
    total: bool,
}

impl Settlement {
    /// The region that was finalized.
    #[must_use]
    pub const fn region(&self) -> RegionId {
        self.region
    }

    /// The worker the region owned.
    #[must_use]
    pub const fn worker(&self) -> WorkerId {
        self.worker
    }

    /// The state the work ended in. Always terminal: [`RegionTree::finalize`] refuses
    /// otherwise.
    #[must_use]
    pub const fn state(&self) -> &WorkerState {
        &self.state
    }

    /// What the cancellation left behind, when cancellation is what ended the work.
    ///
    /// [`None`] for work that completed or failed on its own — those are not cancellation
    /// outcomes, and reporting one for them would invent a continuation question nobody
    /// asked.
    #[must_use]
    pub const fn outcome(&self) -> Option<&CancelOutcome> {
        self.outcome.as_ref()
    }

    /// The region-layer continuation this teardown minted, if any.
    ///
    /// **Not a `cont_*` handle** — see this module's documentation. It answers "is this
    /// answer on the committed-with-continuation arm", and the daemon supplies the handle.
    #[must_use]
    pub fn continuation(&self) -> Option<RegionContinuation> {
        self.outcome.as_ref().and_then(CancelOutcome::continuation)
    }

    /// Whether the teardown left no orphan, no unresolved publication, and a balanced
    /// ledger.
    #[must_use]
    pub const fn is_total(&self) -> bool {
        self.total
    }
}

/// Every region this daemon has opened for task work, and what each teardown left behind.
///
/// One tree for the daemon's whole life: its root is the process scope and is never torn
/// down, and each unit of work is a child region opened under it. Identities are therefore
/// dense ordinals in open order — `r0` is the root, `r1` the first dispatch's scope — which
/// is what lets a test say "the resumed run is a *different* region" by comparing two
/// numbers (INV-005: nothing here is drawn, hashed, or timed).
#[derive(Debug, Default)]
pub struct TaskRegions {
    tree: RegionTree,
    finalized: Vec<Finalization>,
    opened: u32,
    defects: Vec<RegionFault>,
}

impl TaskRegions {
    /// A daemon holding one open root region and nothing else.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The region tree itself — the instrument.
    ///
    /// Exposed so the no-orphan property can be read off the *tree* rather than off the
    /// reports the tree handed back: G0-DX-14 asks for an instrumented answer, and a check
    /// that only re-reads a summary is checking the summary.
    #[must_use]
    pub const fn tree(&self) -> &RegionTree {
        &self.tree
    }

    /// The root region: the daemon's own scope, parent of every unit of work.
    #[must_use]
    pub const fn root(&self) -> RegionId {
        self.tree.root()
    }

    /// Every teardown so far, in teardown order.
    #[must_use]
    pub fn finalizations(&self) -> &[Finalization] {
        &self.finalized
    }

    /// How many scopes this daemon has opened for task work.
    ///
    /// The denominator of the totality claim: `opened == finalizations().len()` is the half
    /// that says nothing was left open, and a check that does not compare them has only
    /// established that the teardowns which *did* happen were clean.
    #[must_use]
    pub const fn opened(&self) -> u32 {
        self.opened
    }

    /// Every region operation this daemon's own wiring got refused, in order.
    ///
    /// Always empty, and the emptiness is load-bearing: the region layer's transition table
    /// is what checks the daemon's claimed order of operations — a `Suspend` before its
    /// `Commit`, a step against a finalized scope, a teardown out of order — so a
    /// non-empty list means the wiring, not the calculus, is wrong. Nothing here changes a
    /// wire answer: bookkeeping that cannot be recorded is recorded as a defect, never
    /// raised as a fault the caller did not cause.
    #[must_use]
    pub fn defects(&self) -> &[RegionFault] {
        &self.defects
    }

    /// Every worker the daemon ever admitted that is not in a terminal state.
    ///
    /// Computed from the tree's own worker states, not from the finalization reports, so it
    /// is an independent accounting of the same claim. Always empty.
    #[must_use]
    pub fn orphans(&self) -> Vec<WorkerId> {
        (0..self.tree.worker_count())
            .filter_map(|ordinal| {
                let worker = WorkerId::at(u32::try_from(ordinal).unwrap_or(u32::MAX));
                match self.tree.worker_state(worker) {
                    Ok(state) if state.is_terminal() => None,
                    _ => Some(worker),
                }
            })
            .collect()
    }

    /// Every scope this daemon opened and did not finalize.
    ///
    /// The root is excluded by construction: it is the daemon's own scope, it owns no
    /// worker, and finalizing it would be the daemon ending rather than a dispatch ending.
    /// Always empty.
    #[must_use]
    pub fn unfinalized(&self) -> Vec<RegionId> {
        (1..self.tree.region_count())
            .filter_map(|ordinal| {
                let region = RegionId::at(u32::try_from(ordinal).unwrap_or(u32::MAX));
                match self.tree.state(region) {
                    Ok(RegionState::Finalized) => None,
                    _ => Some(region),
                }
            })
            .collect()
    }

    /// Whether every scope this daemon opened was torn down totally.
    ///
    /// Six independently computed conjuncts, and the redundancy is the point — the same
    /// shape [`Finalization::is_total`] uses one layer down, for the same reason: a bug that
    /// fools one accounting still has five to get past.
    ///
    /// 1. no region operation was refused (`defects`);
    /// 2. every scope opened produced a teardown (`opened == finalizations().len()`);
    /// 3. every teardown was itself total ([`Finalization::is_total`]);
    /// 4. no worker in the tree is non-terminal (`orphans`);
    /// 5. no region below the root is unfinalized (`unfinalized`);
    /// 6. the tree's obligation ledger is balanced.
    #[must_use]
    pub fn is_total(&self) -> bool {
        self.defects.is_empty()
            && self.opened as usize == self.finalized.len()
            && self.finalized.iter().all(Finalization::is_total)
            && self.orphans().is_empty()
            && self.unfinalized().is_empty()
            && self.tree.ledger().is_balanced()
    }

    /// One canonical text rendering of every teardown this daemon performed.
    ///
    /// Every line is a fact in a fixed order with no timestamp, address, or iteration order
    /// in it, so two runs of one dispatch sequence render byte-identically — the same
    /// device the region layer's own tests use, lifted to the daemon grain
    /// (`rule ordering.deterministic`).
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = format!(
            "daemon regions: opened={} finalized={}\n",
            self.opened,
            self.finalized.len()
        );
        for finalization in &self.finalized {
            out.push_str(&finalization.render());
        }
        out.push_str(&format!(
            "daemon total: orphans={} unfinalized={} defects={} balanced={}\n",
            self.orphans().len(),
            self.unfinalized().len(),
            self.defects.len(),
            self.tree.ledger().is_balanced()
        ));
        out
    }

    /// Advance this scope's worker by one step, recording a refusal rather than raising it.
    ///
    /// A step is the daemon *saying what it just did* — began, staged, published, parked,
    /// finished — and the region layer decides whether that was legal from the state the
    /// worker is in. A refusal is a defect in this wiring, and it is recorded where the
    /// totality claim is read rather than turned into an answer the caller did not ask
    /// for: the wire result of an operation is not a function of the daemon's own
    /// bookkeeping.
    pub(super) fn step(&mut self, scope: Scope, next: WorkerStep) {
        if let Err(fault) = self.tree.advance(scope.worker, next) {
            self.defects.push(fault);
        }
    }

    /// Fail this scope's worker, naming the wire code as the reason.
    ///
    /// The region layer's [`FailureReason`] is deliberately a carrier rather than a second
    /// vocabulary — RFC 0026's `failed_reason` is an `ErrorCode` and that taxonomy lives in
    /// this crate — so the token written down is the wire code's own spelling. A code whose
    /// token were not a canonical reason cannot fail the daemon: the step is skipped and
    /// the scope exit cancels the worker instead, which is still terminal. The unit test
    /// below pins that the branch is never taken.
    pub(super) fn fail(&mut self, scope: Scope, code: ErrorCode) {
        use crate::protocol::spec::ProtocolEnum;
        if let Ok(reason) = FailureReason::new(code.as_wire()) {
            self.step(scope, WorkerStep::Fail(reason));
        }
    }

    /// Open one scope for one unit of task work.
    ///
    /// Private, and [`scoped`] is the only caller: a region that can be opened without the
    /// call that finalizes it is a region that can be left open.
    fn open(&mut self, resumability: Resumability) -> Option<Scope> {
        let root = self.tree.root();
        let region = match self.tree.open_child(root) {
            Ok(region) => region,
            Err(fault) => {
                self.defects.push(fault);
                return None;
            }
        };
        self.opened += 1;
        match self.tree.spawn(region, resumability) {
            Ok(worker) => Some(Scope { region, worker }),
            Err(fault) => {
                self.defects.push(fault);
                None
            }
        }
    }

    /// Close, drain, and finalize this scope, and report what it left behind.
    ///
    /// Private for the same reason [`Self::open`] is. The escalation is the region layer's
    /// decision and not this module's: a normal close whose drain reports
    /// [`RegionFault::DrainBlocked`] names work that is still owed, and work still owed at
    /// a scope exit is work the scope exit cancels.
    fn settle(&mut self, scope: Scope) -> Settlement {
        match self.finish(scope) {
            Ok(report) => {
                let settlement = Self::read(scope, &report);
                self.finalized.push(report);
                settlement
            }
            Err(fault) => {
                self.defects.push(fault);
                Settlement {
                    region: scope.region,
                    worker: scope.worker,
                    state: self
                        .tree
                        .worker_state(scope.worker)
                        .cloned()
                        .unwrap_or(WorkerState::Created),
                    outcome: None,
                    total: false,
                }
            }
        }
    }

    /// The three operations, in RFC 0026's order, with the one escalation.
    fn finish(&mut self, scope: Scope) -> Result<Finalization, RegionFault> {
        self.tree.close(scope.region)?;
        match self.tree.drain(scope.region) {
            Ok(()) => {}
            Err(RegionFault::DrainBlocked { .. }) => {
                self.tree.cancel(scope.region)?;
                self.tree.drain(scope.region)?;
            }
            Err(other) => return Err(other),
        }
        self.tree.finalize(scope.region)
    }

    /// What one scope's report says about the one worker it owned.
    fn read(scope: Scope, report: &Finalization) -> Settlement {
        let found = report
            .workers()
            .iter()
            .find(|worker| worker.worker() == scope.worker);
        Settlement {
            region: scope.region,
            worker: scope.worker,
            state: found.map_or(WorkerState::Created, |worker| worker.state().clone()),
            outcome: found.and_then(|worker| worker.cancel_outcome().cloned()),
            total: report.is_total(),
        }
    }
}

/// Run one unit of task work inside its own region.
///
/// The bracket: open a scope, run the work, tear the scope down — on **every** path out,
/// including the [`Fault`] one and the one where the work never moved its worker at all.
/// That is the whole of the daemon-grain no-orphan argument, and it is an argument about
/// this function's shape rather than about the callers' discipline, because
/// [`TaskRegions::open`] and [`TaskRegions::settle`] are private to this module.
///
/// The work is handed the state back rather than capturing it, because the region tree
/// lives *inside* [`DaemonState`] and a unit of work that could not reach the task table
/// would not be a unit of this daemon's work.
///
/// # Errors
///
/// The work's own [`Fault`], after the scope has been torn down; or [`Fault::denied`] when
/// no scope could be opened at all. The second is unreachable — the root region is opened
/// by [`TaskRegions::new`] and nothing ever closes it, so `open_child` and `spawn` cannot
/// refuse — and it is a typed refusal rather than a panic because a daemon does not abort
/// on its own invariant. It is recorded in [`TaskRegions::defects`], where
/// [`TaskRegions::is_total`] reads it.
pub(super) fn scoped<T>(
    state: &mut DaemonState,
    resumability: Resumability,
    work: impl FnOnce(&mut DaemonState, Scope) -> Result<T, Fault>,
) -> Result<(T, Settlement), Fault> {
    let Some(scope) = state.regions_mut().open(resumability) else {
        return Err(Fault::denied());
    };
    let produced = work(state, scope);
    let settlement = state.regions_mut().settle(scope);
    produced.map(|value| (value, settlement))
}

/// The token a task with no continuation to resume from carries at the region layer.
///
/// RFC 0026's `non_resumable_reason` is a *wire* field of the task record and is written
/// from the daemon's own fault detail; this is the region-layer carrier, and it names the
/// region-layer fact — this work holds no continuation, so its committed evidence is not
/// resumable. It never reaches a caller.
pub(super) const NO_CONTINUATION: &str = "no-continuation";

/// Whether work carrying `resumable`'s answer can mint a continuation.
///
/// Declared rather than defaulted, because [`RegionTree::spawn`] takes it as a parameter
/// for exactly that reason: a caller that does not state it is making a claim either way.
pub(super) fn resumability(resumable: bool) -> Resumability {
    if resumable {
        Resumability::Resumable
    } else {
        Resumability::NonResumable(
            NonResumableReason::new(NO_CONTINUATION)
                .expect("`no-continuation` is printable ASCII and non-empty"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::spec::ProtocolEnum;

    #[test]
    fn every_error_code_token_is_a_canonical_region_reason() {
        for code in ErrorCode::ALL {
            assert!(
                FailureReason::new(code.as_wire()).is_ok(),
                "{} is not a canonical reason token, so a failure would be recorded as a \
                 cancellation instead",
                code.as_wire()
            );
        }
    }

    #[test]
    fn the_daemon_side_reason_token_is_canonical() {
        assert!(NonResumableReason::new(NO_CONTINUATION).is_ok());
        assert!(matches!(resumability(false), Resumability::NonResumable(_)));
        assert_eq!(resumability(true), Resumability::Resumable);
    }

    #[test]
    fn a_fresh_daemon_holds_one_open_root_and_owes_nothing() {
        let regions = TaskRegions::new();
        assert_eq!(regions.opened(), 0);
        assert!(regions.finalizations().is_empty());
        assert!(regions.orphans().is_empty());
        assert!(regions.unfinalized().is_empty());
        assert!(regions.is_total(), "{}", regions.render());
    }

    #[test]
    fn a_scope_that_is_opened_and_never_stepped_is_still_torn_down() {
        let mut regions = TaskRegions::new();
        let scope = regions.open(resumability(true)).expect("the root is open");
        let settlement = regions.settle(scope);
        assert_eq!(settlement.state(), &WorkerState::Cancelled);
        assert_eq!(
            settlement.outcome(),
            Some(&CancelOutcome::NothingPublished),
            "work that never began published nothing"
        );
        assert!(settlement.is_total());
        assert!(regions.is_total(), "{}", regions.render());
    }

    #[test]
    fn work_that_finished_on_its_own_is_closed_rather_than_cancelled() {
        let mut regions = TaskRegions::new();
        let scope = regions.open(resumability(true)).expect("the root is open");
        regions.step(scope, WorkerStep::Begin);
        regions.step(scope, WorkerStep::Reserve);
        regions.step(scope, WorkerStep::Commit);
        regions.step(scope, WorkerStep::Complete);
        let settlement = regions.settle(scope);
        assert_eq!(settlement.state(), &WorkerState::Completed);
        assert_eq!(
            settlement.outcome(),
            None,
            "a completion is not a cancellation outcome"
        );
        assert!(regions.is_total(), "{}", regions.render());
    }

    #[test]
    fn a_parked_worker_is_terminated_by_the_scope_exit_and_leaves_a_continuation() {
        let mut regions = TaskRegions::new();
        let scope = regions.open(resumability(true)).expect("the root is open");
        for next in [
            WorkerStep::Begin,
            WorkerStep::Reserve,
            WorkerStep::Commit,
            WorkerStep::Suspend,
        ] {
            regions.step(scope, next);
        }
        let settlement = regions.settle(scope);
        assert_eq!(
            settlement.state(),
            &WorkerState::Cancelled,
            "Suspended cannot survive a region teardown; the parked *artifact* is what \
             survives"
        );
        assert_eq!(
            settlement.continuation(),
            Some(RegionContinuation::new(scope.worker(), 1))
        );
        assert!(regions.is_total(), "{}", regions.render());
    }

    #[test]
    fn each_unit_of_work_gets_its_own_region_and_the_ordinals_say_so() {
        let mut regions = TaskRegions::new();
        let first = regions.open(resumability(true)).expect("the root is open");
        regions.settle(first);
        let second = regions.open(resumability(true)).expect("the root is open");
        regions.settle(second);
        assert_ne!(
            first.region(),
            second.region(),
            "a resumed run is a new scope, not the parked one reopened"
        );
        assert_eq!(regions.opened(), 2);
        assert_eq!(regions.finalizations().len(), 2);
        assert!(regions.is_total(), "{}", regions.render());
    }

    #[test]
    fn a_step_the_region_layer_refuses_is_recorded_rather_than_raised() {
        let mut regions = TaskRegions::new();
        let scope = regions.open(resumability(true)).expect("the root is open");
        regions.step(scope, WorkerStep::Begin);
        regions.step(scope, WorkerStep::Reserve);
        // Parking mid-publication: RFC 0026's "cancellation MUST NOT truncate a publication
        // in progress", enforced one layer down.
        regions.step(scope, WorkerStep::Suspend);
        assert_eq!(regions.defects().len(), 1);
        assert!(!regions.is_total(), "a recorded defect falsifies totality");
        let settlement = regions.settle(scope);
        assert_eq!(
            settlement.state(),
            &WorkerState::Cancelled,
            "the teardown is still total over the work itself"
        );
        assert!(settlement.is_total());
    }

    #[test]
    fn the_render_of_one_dispatch_sequence_is_byte_identical_across_two_runs() {
        let build = || {
            let mut regions = TaskRegions::new();
            let first = regions.open(resumability(true)).expect("open");
            regions.step(first, WorkerStep::Begin);
            regions.step(first, WorkerStep::Reserve);
            regions.step(first, WorkerStep::Commit);
            regions.step(first, WorkerStep::Suspend);
            regions.settle(first);
            let second = regions.open(resumability(false)).expect("open");
            regions.step(second, WorkerStep::Begin);
            regions.fail(second, ErrorCode::BudgetExhausted);
            regions.settle(second);
            regions.render()
        };
        let first = build();
        let second = build();
        assert_eq!(first.as_bytes(), second.as_bytes());
        assert!(!first.is_empty(), "a vacuous render proves nothing");
    }
}
