//! Cancellation at every lifecycle phase, and the typed outcome each one leaves —
//! the region-layer half of `PR-6-IMPL-01` and the shape `G0-DX-14` will be argued in.
//!
//! > **`task.cancel` triggers request → drain → finalize.** It MUST leave either
//! > committed partial evidence plus a valid continuation, or nothing published
//! > (INV-009, B19). Its response carries `continuation` as `nullable`, so "cancelled
//! > with nothing published" is a representable, named outcome rather than an inference
//! > from an absent field.
//! >
//! > — `notes/plan/rfcs/0026-continuumd-native-protocol.md`, "Task lifecycle"
//!
//! > - child workers receive cancellation;
//! > - provisional streams close;
//! > - committed partial artifacts are finalized;
//! > - continuation is emitted if supported;
//! > - task transitions exactly once to terminal/suspended state;
//! > - obligations and resource leases are resolved.
//! >
//! > — `notes/plan/docs/35_CONTINUUMD_WORKBENCH_DAEMON.md`, "Cancellation"
//!
//! # Why this file exists
//!
//! `G0-DX-14`'s required experiment is "cancel DPOR, solver, proof, and synthesis tasks
//! at every phase" and its pass condition is "no leaked obligations; resumable artifacts
//! either committed or absent" (`notes/plan/notes/G0_SPIKE_MATRIX.md`). Three of those
//! four engines do not exist in this workspace, so **this file is not that campaign and
//! does not close that row.** What it does is fix the *shape* the campaign will be
//! argued in, over the one substrate that does exist: a table of every phase a
//! cancellation can arrive in, the typed outcome each one produces, and a check of both
//! pass-condition conjuncts at every row.
//!
//! The campaign itself is `tests/dx14_cancellation_matrix.rs` (bn-2zy), which took this
//! shape and ran it per task lane — the four engines as publication/suspension profiles,
//! every phase of each, across placements, teardown arrivals, a request window, and an
//! interleaved four-lane portfolio — with `crates/continuumd/tests/dx14_cancellation_matrix.rs`
//! as its daemon-grain half. This file stays as the single-worker table it always was: it is
//! the readable statement of the rule, and the campaign is the sweep.
//!
//! The table is the artifact. Each row is a `(phase, expected outcome)` pair written as
//! data, so a change to the calculus that silently reclassified one of them fails here
//! with the phase named rather than passing with a different meaning.
//!
//! # Evidence map
//!
//! | Claim | Test |
//! |---|---|
//! | every phase produces its declared typed outcome | [`cancellation_at_every_phase_produces_its_declared_outcome`] |
//! | no phase leaks an obligation | [`cancellation_at_every_phase_leaves_the_ledger_balanced`] |
//! | no phase leaves an artifact neither committed nor absent | [`cancellation_at_every_phase_leaves_every_artifact_committed_or_absent`] |
//! | committed evidence survives cancellation, staged evidence does not | [`cancellation_discards_the_staged_half_and_keeps_the_committed_half`] |
//! | a task already terminal is not re-terminated | [`cancellation_does_not_reopen_a_terminal_status`] |
//! | cancellation reaches every region beneath the one it named | [`cancellation_reaches_the_deepest_worker_in_the_subtree`] |
//! | cancelling twice changes nothing | [`a_second_cancellation_request_changes_nothing`] |
//! | the both-or-neither rule holds at every row | [`no_row_pairs_committed_evidence_with_a_missing_continuation`] |
//!
//! # House rules
//!
//! - **Every row is checked for both conjuncts**, not just the interesting one. A phase
//!   that produced the right outcome while leaking an obligation would be a pass on the
//!   half the reader was looking at.
//! - **The table is enumerated, never sampled.** [`Phase::ALL`] is the list, and a test
//!   that iterated a subset of it would be evidence about a subset.
//! - **No assertion is on a message.** Outcomes are compared as typed values.

use continuum_task::region::worker::{
    CancelOutcome, Continuation, FailureReason, NonResumableReason, Resumability, WorkerId,
    WorkerState, WorkerStep,
};
use continuum_task::region::{Finalization, RegionFault, RegionState, RegionTree};

/// A phase a cancellation can arrive in.
///
/// "Phase" is the word `PR-6`'s exit condition uses — "cancellation at every instrumented
/// phase leaves either a valid continuation or no published partial artifact" — and this
/// enum is the region layer's instrumentation of it: every reachable combination of
/// worker state, staged publication, committed evidence and declared resumability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// The region was already draining when the spawn was attempted.
    BeforeAdmission,
    /// Admitted, never stepped.
    Created,
    /// Stepping, nothing staged and nothing committed.
    Running,
    /// Stepping, one publication staged and nothing committed.
    RunningStaged,
    /// Stepping, one publication committed.
    RunningCommitted,
    /// Stepping, one committed and a second staged.
    RunningCommittedAndStaged,
    /// Parked, nothing committed.
    SuspendedEmpty,
    /// Parked with committed partial evidence — the B18 preemption shape.
    SuspendedCommitted,
    /// Already finished its work.
    Completed,
    /// Already failed.
    Failed,
    /// Already cancelled by an earlier request.
    AlreadyCancelled,
    /// Stepping, declared non-resumable, nothing committed.
    NonResumableEmpty,
    /// Stepping, declared non-resumable, one publication committed.
    NonResumableCommitted,
}

impl Phase {
    /// Every phase. The table is this list; a test that iterates less than all of it is
    /// evidence about less than all of it.
    const ALL: &'static [Self] = &[
        Self::BeforeAdmission,
        Self::Created,
        Self::Running,
        Self::RunningStaged,
        Self::RunningCommitted,
        Self::RunningCommittedAndStaged,
        Self::SuspendedEmpty,
        Self::SuspendedCommitted,
        Self::Completed,
        Self::Failed,
        Self::AlreadyCancelled,
        Self::NonResumableEmpty,
        Self::NonResumableCommitted,
    ];

    /// What the worker's terminal state must be after the teardown.
    ///
    /// Cancellation terminates everything it reaches; a worker that was already terminal
    /// keeps the status it had, because RFC 0026's monotonicity says a terminal status
    /// never changes.
    fn expected_state(self) -> WorkerState {
        match self {
            Self::Completed => WorkerState::Completed,
            Self::Failed => WorkerState::Failed(failure("engine-defect")),
            _ => WorkerState::Cancelled,
        }
    }

    /// What the worker's cancel outcome must be after the teardown.
    ///
    /// [`None`] for a worker that had already reached a terminal state on its own: those
    /// are not cancellation outcomes, and reporting one would invent a continuation
    /// question nobody asked.
    fn expected_outcome(self) -> Option<CancelOutcome> {
        match self {
            // Admission was refused, so there is no worker to report on at all.
            Self::BeforeAdmission => None,
            Self::Created
            | Self::Running
            | Self::RunningStaged
            | Self::SuspendedEmpty
            | Self::NonResumableEmpty => Some(CancelOutcome::NothingPublished),
            Self::RunningCommitted
            | Self::RunningCommittedAndStaged
            | Self::SuspendedCommitted
            | Self::AlreadyCancelled => Some(CancelOutcome::CommittedWithContinuation(
                Continuation::new(WorkerId::at(0), 1),
            )),
            Self::Completed | Self::Failed => None,
            Self::NonResumableCommitted => Some(CancelOutcome::CommittedNonResumable(
                NonResumableReason::new("no-continuation-support").expect("canonical"),
            )),
        }
    }

    /// How many publications the worker must still have committed afterwards.
    ///
    /// INV-009's monotonicity, read at the far end of a teardown: the staged half is
    /// discarded and the committed half is exactly what it was.
    fn expected_committed(self) -> u32 {
        match self {
            Self::RunningCommitted
            | Self::RunningCommittedAndStaged
            | Self::SuspendedCommitted
            | Self::AlreadyCancelled
            | Self::NonResumableCommitted => 1,
            _ => 0,
        }
    }
}

fn failure(token: &str) -> FailureReason {
    FailureReason::new(token).expect("test reason tokens are canonical")
}

fn non_resumable() -> Resumability {
    Resumability::NonResumable(
        NonResumableReason::new("no-continuation-support").expect("canonical"),
    )
}

/// Build a tree whose single root worker sits at `phase`, and return it un-torn-down.
///
/// The worker, when there is one, is always `w0`, which is what lets
/// [`Phase::expected_outcome`] name a continuation without threading identities through
/// the table.
fn at(phase: Phase) -> RegionTree {
    let mut tree = RegionTree::new();
    let root = tree.root();

    if phase == Phase::BeforeAdmission {
        tree.cancel(root).expect("the root is not finalized");
        assert!(
            tree.spawn(root, Resumability::Resumable).is_err(),
            "a draining region must refuse admission"
        );
        return tree;
    }

    let resumability = match phase {
        Phase::NonResumableEmpty | Phase::NonResumableCommitted => non_resumable(),
        _ => Resumability::Resumable,
    };
    let worker = tree.spawn(root, resumability).expect("the root is open");
    if phase == Phase::Created {
        return tree;
    }
    tree.advance(worker, WorkerStep::Begin).expect("legal");

    let commit_first = matches!(
        phase,
        Phase::RunningCommitted
            | Phase::RunningCommittedAndStaged
            | Phase::SuspendedCommitted
            | Phase::AlreadyCancelled
            | Phase::NonResumableCommitted
    );
    if commit_first {
        tree.advance(worker, WorkerStep::Reserve).expect("legal");
        tree.advance(worker, WorkerStep::Commit).expect("legal");
    }
    if matches!(
        phase,
        Phase::RunningStaged | Phase::RunningCommittedAndStaged
    ) {
        tree.advance(worker, WorkerStep::Reserve).expect("legal");
    }
    match phase {
        Phase::SuspendedEmpty | Phase::SuspendedCommitted => {
            tree.advance(worker, WorkerStep::Suspend).expect("legal");
        }
        Phase::Completed => {
            tree.advance(worker, WorkerStep::Complete).expect("legal");
        }
        Phase::Failed => {
            tree.advance(worker, WorkerStep::Fail(failure("engine-defect")))
                .expect("legal");
        }
        Phase::AlreadyCancelled => {
            tree.cancel(root).expect("not finalized");
            tree.drain(root).expect("cancellation drains");
        }
        _ => {}
    }
    tree
}

/// Cancel, drain and finalize the root of a tree built at `phase`.
fn torn_down(phase: Phase) -> (RegionTree, Finalization) {
    let mut tree = at(phase);
    let root = tree.root();
    let finalization = tree
        .teardown(root)
        .expect("teardown is total for a region that exists and is not finalized");
    (tree, finalization)
}

// --- the table -----------------------------------------------------------------------

#[test]
fn cancellation_at_every_phase_produces_its_declared_outcome() {
    let mut rows = 0_usize;
    for phase in Phase::ALL {
        let (_, finalization) = torn_down(*phase);
        let reports = finalization.workers();
        if *phase == Phase::BeforeAdmission {
            assert!(
                reports.is_empty(),
                "{phase:?}: a refused admission must leave no worker to report"
            );
            rows += 1;
            continue;
        }
        assert_eq!(reports.len(), 1, "{phase:?}: the fixture spawns one worker");
        let report = &reports[0];
        assert_eq!(
            report.state(),
            &phase.expected_state(),
            "{phase:?}: wrong terminal state\n{}",
            finalization.render()
        );
        assert_eq!(
            report.cancel_outcome(),
            phase.expected_outcome().as_ref(),
            "{phase:?}: wrong cancel outcome\n{}",
            finalization.render()
        );
        assert_eq!(
            report.evidence().committed(),
            phase.expected_committed(),
            "{phase:?}: committed evidence is not monotone\n{}",
            finalization.render()
        );
        rows += 1;
    }
    assert_eq!(rows, Phase::ALL.len(), "every phase must have been visited");
    assert!(rows >= 13, "a table this small is not the table");
}

#[test]
fn cancellation_at_every_phase_leaves_the_ledger_balanced() {
    for phase in Phase::ALL {
        let (tree, finalization) = torn_down(*phase);
        assert!(
            finalization.ledger().is_balanced(),
            "{phase:?}: leaked an obligation — {}",
            finalization.render()
        );
        assert!(
            tree.ledger().is_balanced(),
            "{phase:?}: the tree's own ledger disagrees with the report"
        );
        assert!(
            finalization.ledger().opened() > 0 || *phase == Phase::BeforeAdmission,
            "{phase:?}: a ledger that never opened anything proves nothing"
        );
    }
}

#[test]
fn cancellation_at_every_phase_leaves_every_artifact_committed_or_absent() {
    for phase in Phase::ALL {
        let (_, finalization) = torn_down(*phase);
        assert!(
            finalization.unresolved_publications().is_empty(),
            "{phase:?}: an artifact was left neither committed nor absent — {}",
            finalization.render()
        );
        assert!(
            finalization.is_total(),
            "{phase:?}: teardown was not total — {}",
            finalization.render()
        );
    }
}

#[test]
fn no_row_pairs_committed_evidence_with_a_missing_continuation() {
    for phase in Phase::ALL {
        let (_, finalization) = torn_down(*phase);
        for report in finalization.workers() {
            let Some(outcome) = report.cancel_outcome() else {
                continue;
            };
            let committed = report.evidence().has_committed_evidence();
            match outcome {
                CancelOutcome::NothingPublished => assert!(
                    !committed,
                    "{phase:?}: reported nothing published while holding committed evidence"
                ),
                CancelOutcome::CommittedWithContinuation(continuation) => {
                    assert!(
                        committed,
                        "{phase:?}: minted a continuation over nothing committed"
                    );
                    assert_eq!(
                        continuation.committed(),
                        report.evidence().committed(),
                        "{phase:?}: the continuation and the ledger disagree on how much was committed"
                    );
                }
                CancelOutcome::CommittedNonResumable(_) => assert!(
                    committed,
                    "{phase:?}: named a non-resumable reason over nothing committed"
                ),
            }
        }
    }
}

// --- the individual claims the table rests on ----------------------------------------

#[test]
fn cancellation_discards_the_staged_half_and_keeps_the_committed_half() {
    let (tree, finalization) = torn_down(Phase::RunningCommittedAndStaged);
    let worker = WorkerId::at(0);
    let evidence = tree.evidence(worker).expect("the fixture spawned it");
    assert_eq!(evidence.committed(), 1, "INV-009: committed is monotone");
    assert!(
        !evidence.is_provisional(),
        "the staged publication must be gone, not truncated"
    );
    assert_eq!(
        finalization.continuations(),
        vec![Continuation::new(worker, 1)],
        "the continuation resumes from the committed count, not the staged one"
    );
}

#[test]
fn cancellation_does_not_reopen_a_terminal_status() {
    for (phase, expected) in [
        (Phase::Completed, WorkerState::Completed),
        (Phase::Failed, WorkerState::Failed(failure("engine-defect"))),
    ] {
        let (tree, finalization) = torn_down(phase);
        assert_eq!(
            tree.worker_state(WorkerId::at(0)),
            Ok(&expected),
            "{phase:?}: cancellation rewrote a terminal status"
        );
        assert_eq!(
            finalization.workers()[0].cancel_outcome(),
            None,
            "{phase:?}: a worker cancellation never reached has no cancellation outcome"
        );
    }
}

#[test]
fn a_second_cancellation_request_changes_nothing() {
    let (tree, first) = torn_down(Phase::AlreadyCancelled);
    // `AlreadyCancelled` was cancelled once inside the fixture and once by the teardown.
    assert_eq!(
        tree.worker_state(WorkerId::at(0)),
        Ok(&WorkerState::Cancelled)
    );
    assert_eq!(
        first.workers()[0].cancel_outcome(),
        Some(&CancelOutcome::CommittedWithContinuation(
            Continuation::new(WorkerId::at(0), 1)
        )),
        "the outcome the first cancellation computed is the one that survives"
    );
    assert!(first.is_total());

    // And a request against a region that is merely draining is idempotent in state.
    let mut tree = at(Phase::Running);
    let root = tree.root();
    tree.cancel(root).expect("not finalized");
    let after_one = tree.state(root);
    tree.cancel(root).expect("requesting twice changes nothing");
    assert_eq!(tree.state(root), after_one);
}

#[test]
fn cancellation_reaches_the_deepest_worker_in_the_subtree() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let child = tree.open_child(root).expect("open");
    let grandchild = tree.open_child(child).expect("open");
    let deep = tree
        .spawn(grandchild, Resumability::Resumable)
        .expect("open");
    tree.advance(deep, WorkerStep::Begin).expect("legal");
    tree.advance(deep, WorkerStep::Reserve).expect("legal");

    let finalization = tree.teardown(root).expect("the root exists");
    assert_eq!(
        finalization.regions().len(),
        3,
        "the teardown must cover the whole subtree"
    );
    assert_eq!(
        finalization.workers()[0].state(),
        &WorkerState::Cancelled,
        "a worker three regions down still receives the cancellation"
    );
    assert!(finalization.is_total(), "{}", finalization.render());
    for region in finalization.regions() {
        assert_eq!(tree.state(*region), Ok(RegionState::Finalized));
    }
}

#[test]
fn a_cancelled_region_admits_no_late_work_and_says_so() {
    let mut tree = at(Phase::Running);
    let root = tree.root();
    tree.cancel(root).expect("not finalized");
    let fault = tree
        .spawn(root, Resumability::Resumable)
        .expect_err("a cancelled region admits nothing");
    assert!(matches!(fault, RegionFault::SpawnIntoClosedRegion { .. }));
    let fault = tree
        .open_child(root)
        .expect_err("a cancelled region opens no child");
    assert!(matches!(fault, RegionFault::OpenChildInClosedRegion { .. }));
    assert_eq!(
        tree.worker_count(),
        1,
        "the set the teardown must account for stopped growing at the request"
    );
}

#[test]
fn a_worker_may_still_take_steps_between_the_request_and_the_drain() {
    // Cancellation is cooperative: the request stops new work entering, and the worker
    // that is mid-flight keeps stepping until the drain reaches it. This is the window
    // every interesting adversarial schedule lives in, so it must be open.
    let mut tree = at(Phase::Running);
    let root = tree.root();
    let worker = WorkerId::at(0);
    tree.cancel(root).expect("not finalized");
    tree.advance(worker, WorkerStep::Reserve)
        .expect("a cancelled worker may still stage");
    tree.advance(worker, WorkerStep::Commit)
        .expect("and may still finish the publication it started");
    let finalization = tree.teardown(root).expect("cancelling twice is fine");
    assert_eq!(
        finalization.workers()[0].cancel_outcome(),
        Some(&CancelOutcome::CommittedWithContinuation(
            Continuation::new(worker, 1)
        )),
        "work committed after the request is committed, not truncated"
    );
    assert!(finalization.is_total());
}
