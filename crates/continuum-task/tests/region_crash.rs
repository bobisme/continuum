//! The fail-stop crash step, `WorkerStep::Crash` (RFC 0026 correction 59; bn-fxxf2).
//!
//! > no code of that incarnation runs after it, and no finalizer, cancellation handler
//! > or cleanup runs at it
//! >
//! > — the process pack's profile `process/crash-restart-v0`, row `fail-stop-crash`
//!
//! Before this step the calculus had no crash, and the PR-14 lift stated one as the
//! worker's own `Fail`, first taking a parked worker off its park with a `Resume` the
//! task never took (correction 58, item 3). The crash step moves a worker from any
//! non-terminal state and any cancellation phase to `Failed`, discards what it staged,
//! and leaves its adapter obligations open with a terminal holder: fenced.

use continuum_task::region::obligation::{
    Obligation, SubstrateId, SubstrateKind, SubstrateObligation, SubstrateOutcome,
};
use continuum_task::region::worker::{
    CancelPhase, FailureReason, PublicationSlot, Resumability, WorkerId, WorkerState, WorkerStep,
};
use continuum_task::region::{RegionFault, RegionId, RegionState, RegionTree};

fn crashed() -> FailureReason {
    FailureReason::new("crashed").expect("canonical")
}

fn crash() -> WorkerStep {
    WorkerStep::Crash(crashed())
}

fn lease(id: u64) -> SubstrateObligation {
    SubstrateObligation::new(
        SubstrateKind::new("lease").expect("canonical"),
        SubstrateId::at(id),
    )
}

fn spawn_running(tree: &mut RegionTree, region: RegionId) -> WorkerId {
    let worker = tree.spawn(region, Resumability::Resumable).expect("open");
    tree.advance(worker, WorkerStep::Begin).expect("legal");
    worker
}

/// A worker in the root, in lifecycle `state` and cancellation `phase`, reached through
/// the calculus's own steps. Each of the nine pairs is reachable: a request and its
/// acknowledgement need no begin.
fn place(tree: &mut RegionTree, state: &str, phase: CancelPhase) -> WorkerId {
    let root = tree.root();
    let worker = tree.spawn(root, Resumability::Resumable).expect("open");
    if state != "created" {
        tree.advance(worker, WorkerStep::Begin).expect("legal");
    }
    if state == "suspended" {
        tree.advance(worker, WorkerStep::Suspend).expect("legal");
    }
    if phase != CancelPhase::Active {
        tree.advance(worker, WorkerStep::RequestCancel)
            .expect("legal");
    }
    if phase == CancelPhase::Acknowledged {
        tree.advance(worker, WorkerStep::AcknowledgeCancel)
            .expect("legal");
    }
    assert_eq!(tree.cancel_phase(worker), Ok(phase));
    worker
}

#[test]
fn a_crash_stops_a_worker_in_every_non_terminal_state_and_cancellation_phase() {
    let mut reached = 0;
    for state in ["created", "running", "suspended"] {
        for phase in [
            CancelPhase::Active,
            CancelPhase::Requested,
            CancelPhase::Acknowledged,
        ] {
            let mut tree = RegionTree::new();
            let worker = place(&mut tree, state, phase);
            assert_eq!(
                tree.advance(worker, crash()),
                Ok(WorkerState::Failed(crashed())),
                "{state}/{phase}"
            );
            assert!(
                !tree.ledger().holds(Obligation::worker_termination(worker)),
                "{state}/{phase}: the crash discharges the termination obligation"
            );
            reached += 1;
        }
    }
    assert_eq!(reached, 9);
}

#[test]
fn fail_still_leaves_only_created_or_running_and_not_a_cancelling_worker() {
    // The crash is a new step, not a widened `Fail`: the worker's own failure keeps its
    // guard, so a parked worker cannot fail and a cancelling one only drains.
    let mut tree = RegionTree::new();
    let parked = place(&mut tree, "suspended", CancelPhase::Active);
    let fail = WorkerStep::Fail(crashed());
    assert!(matches!(
        tree.advance(parked, fail.clone()),
        Err(RegionFault::IllegalWorkerStep { .. })
    ));
    let cancelling = place(&mut tree, "running", CancelPhase::Acknowledged);
    assert!(matches!(
        tree.advance(cancelling, fail),
        Err(RegionFault::WorkerCancelling { .. })
    ));
    // Both crash.
    assert!(tree.advance(parked, crash()).is_ok());
    assert!(tree.advance(cancelling, crash()).is_ok());
}

#[test]
fn a_terminal_worker_does_not_crash() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let done = spawn_running(&mut tree, root);
    tree.advance(done, WorkerStep::Complete).expect("legal");
    let crashed_once = spawn_running(&mut tree, root);
    tree.advance(crashed_once, crash()).expect("legal");
    let cancelled_alone = spawn_running(&mut tree, root);
    tree.advance(cancelled_alone, WorkerStep::RequestCancel)
        .expect("legal");
    tree.advance(cancelled_alone, WorkerStep::AcknowledgeCancel)
        .expect("legal");
    tree.advance(cancelled_alone, WorkerStep::CompleteCancelled)
        .expect("legal");
    let child = tree.open_child(root).expect("open");
    let drained = spawn_running(&mut tree, child);
    tree.cancel(child).expect("open");
    tree.drain(child).expect("cancelling drain");
    assert_eq!(tree.worker_state(drained), Ok(&WorkerState::Cancelled));
    for worker in [done, crashed_once, cancelled_alone, drained] {
        assert!(matches!(
            tree.advance(worker, crash()),
            Err(RegionFault::IllegalWorkerStep { .. })
        ));
    }
}

#[test]
fn a_crashed_worker_takes_no_later_step() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let worker = spawn_running(&mut tree, root);
    tree.advance(worker, WorkerStep::Suspend).expect("legal");
    tree.advance(worker, crash()).expect("legal");
    for step in [
        WorkerStep::Resume,
        WorkerStep::Begin,
        WorkerStep::Reserve,
        WorkerStep::Complete,
        WorkerStep::RequestCancel,
        WorkerStep::AcknowledgeCancel,
        WorkerStep::CompleteCancelled,
    ] {
        assert!(
            tree.advance(worker, step.clone()).is_err(),
            "{step} after a crash"
        );
    }
}

#[test]
fn a_crash_discards_every_staged_publication_and_keeps_committed_evidence() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let worker = spawn_running(&mut tree, root);
    tree.advance(worker, WorkerStep::Reserve).expect("legal");
    tree.advance(worker, WorkerStep::Commit).expect("legal");
    tree.advance(worker, WorkerStep::ReserveSlot(PublicationSlot::at(1)))
        .expect("legal");
    tree.advance(worker, WorkerStep::ReserveSlot(PublicationSlot::at(2)))
        .expect("legal");
    assert_eq!(tree.evidence(worker).map(|e| e.staged()), Ok(2));
    tree.advance(worker, crash()).expect("legal");
    let evidence = tree.evidence(worker).expect("known");
    assert_eq!(evidence.staged(), 0, "nothing staged survives a crash");
    assert_eq!(evidence.committed(), 1, "committed evidence is monotone");
    for slot in [1, 2] {
        assert!(!tree.ledger().holds(Obligation::staged_publication(
            worker,
            PublicationSlot::at(slot)
        )));
    }
}

#[test]
fn a_crashed_holders_obligations_are_fenced_owed_and_out_of_reach() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let child = tree.open_child(root).expect("open");
    let holder = spawn_running(&mut tree, child);
    let peer = spawn_running(&mut tree, root);
    tree.open_substrate(holder, lease(7)).expect("legal");
    tree.advance(holder, WorkerStep::Suspend).expect("legal");
    tree.advance(holder, crash()).expect("legal");

    // Owed: still open in the ledger, still held by the crashed worker.
    assert!(tree.ledger().holds(lease(7).obligation()));
    assert_eq!(tree.substrate_holder(SubstrateId::at(7)), Some(holder));
    // Out of reach: no discharge, of either outcome, and no hand-off.
    for outcome in [SubstrateOutcome::Committed, SubstrateOutcome::Aborted] {
        assert!(matches!(
            tree.discharge_substrate(holder, lease(7), outcome),
            Err(RegionFault::SubstrateHolderNotLive { .. })
        ));
    }
    assert!(matches!(
        tree.transfer_substrate(holder, lease(7), peer),
        Err(RegionFault::SubstrateHolderNotLive { .. })
    ));

    // A finalization over it reports it, and is not total: the calculus does not absorb a
    // fence into a discharge.
    tree.close(child).expect("open");
    tree.drain(child).expect("every worker is terminal");
    let finalization = tree.finalize(child).expect("drained");
    assert!(finalization.orphans().is_empty());
    assert!(finalization.unresolved_publications().is_empty());
    assert!(!finalization.is_total());
    assert_eq!(
        finalization.ledger().outstanding(),
        &[lease(7).obligation()]
    );
    let report = &finalization.workers()[0];
    assert_eq!(report.state(), &WorkerState::Failed(crashed()));
    assert_eq!(
        report.cancel_outcome(),
        None,
        "a crashed worker did not complete as cancelled"
    );
}

#[test]
fn a_crash_inside_a_cancelling_region_leaves_the_drain_nothing_to_cancel() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let child = tree.open_child(root).expect("open");
    let stopped = spawn_running(&mut tree, child);
    let other = spawn_running(&mut tree, child);
    tree.cancel(child).expect("open");
    assert_eq!(
        tree.state(child),
        Ok(RegionState::Draining(
            continuum_task::region::DrainCause::Cancelled
        ))
    );
    tree.advance(stopped, crash()).expect("legal");
    tree.drain(child).expect("cancelling drain");
    assert_eq!(
        tree.worker_state(stopped),
        Ok(&WorkerState::Failed(crashed()))
    );
    assert_eq!(tree.worker_state(other), Ok(&WorkerState::Cancelled));
    let finalization = tree.finalize(child).expect("drained");
    assert!(finalization.is_total(), "{}", finalization.render());
}

#[test]
fn a_crash_in_a_finalized_region_is_refused() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let child = tree.open_child(root).expect("open");
    let worker = tree.spawn(child, Resumability::Resumable).expect("open");
    tree.cancel(child).expect("open");
    tree.drain(child).expect("drain");
    tree.finalize(child).expect("finalize");
    assert!(matches!(
        tree.advance(worker, crash()),
        Err(RegionFault::AdvanceInFinalizedRegion { .. })
    ));
}

#[test]
fn the_crash_step_renders_with_its_reason() {
    assert_eq!(crash().token(), "crash");
    assert_eq!(crash().to_string(), "crash(crashed)");
}

/// Metamorphic relation **independent-event swap** (docs/19 §3): the crashes of two
/// distinct workers are independent events, so swapping their order, or swapping a
/// crash with an independent step of another worker, leaves the same finalization and
/// the same ledger.
#[test]
fn independent_event_swap_of_two_crashes_preserves_the_finalization() {
    let run = |order: &[&str]| {
        let mut tree = RegionTree::new();
        let root = tree.root();
        let child = tree.open_child(root).expect("open");
        let a = spawn_running(&mut tree, child);
        let b = spawn_running(&mut tree, child);
        let c = spawn_running(&mut tree, child);
        tree.advance(a, WorkerStep::Reserve).expect("legal");
        tree.open_substrate(b, lease(1)).expect("legal");
        tree.advance(b, WorkerStep::Suspend).expect("legal");
        for step in order {
            match *step {
                "crash-a" => tree.advance(a, crash()),
                "crash-b" => tree.advance(b, crash()),
                "complete-c" => tree.advance(c, WorkerStep::Complete),
                other => panic!("{other}"),
            }
            .expect("legal in every order");
        }
        tree.close(child).expect("open");
        tree.drain(child).expect("all terminal");
        let finalization = tree.finalize(child).expect("drained");
        (finalization.render(), tree.ledger().summary().to_string())
    };
    let base = run(&["crash-a", "crash-b", "complete-c"]);
    for order in [
        ["crash-b", "crash-a", "complete-c"],
        ["complete-c", "crash-a", "crash-b"],
        ["crash-a", "complete-c", "crash-b"],
        ["complete-c", "crash-b", "crash-a"],
    ] {
        assert_eq!(run(&order), base, "{order:?}");
    }
    assert!(base.0.contains("substrate(lease):s1"), "{}", base.0);
}
