//! Single-task cancellation, the per-task acknowledgement, and the subtree-scoped
//! finalization ledger (RFC 0026 correction 53, bn-36wy3).
//!
//! > ```text
//! > Active ─ request(cancel_reason) → Cancelling ─ drain* ─ finalize* ─ obligations == ∅ → Cancelled
//! > ```
//! >
//! > — `notes/plan/docs/02_SEMANTICS.md` §7, "Lifecycle"
//!
//! Before this change a worker could be cancelled only by its region's drain, and the
//! calculus decided "cancelling" per region, at the region's cancel request. Now:
//!
//! - [`WorkerStep::RequestCancel`] requests one worker's cancellation (a deadline, say);
//! - [`WorkerStep::AcknowledgeCancel`] is the worker observing a request, its own or its
//!   region's; after it the worker only drains;
//! - [`WorkerStep::CompleteCancelled`] ends one acknowledged worker as cancelled,
//!   through the same per-worker cancellation a drain uses;
//! - the adapter-obligation rules turn at the worker's own acknowledgement, not at the
//!   region's request;
//! - [`Finalization::ledger`] is the finalized subtree's own ledger.

use continuum_task::region::obligation::{
    Obligation, SubstrateId, SubstrateKind, SubstrateObligation, SubstrateOutcome,
};
use continuum_task::region::schedule::{Schedule, Step, interleavings};
use continuum_task::region::worker::{
    CancelOutcome, CancelPhase, Continuation, FailureReason, PublicationSlot, Resumability,
    WorkerId, WorkerState, WorkerStep,
};
use continuum_task::region::{RegionFault, RegionId, RegionTree};

fn permit(id: u64) -> SubstrateObligation {
    SubstrateObligation::new(
        SubstrateKind::new("send-permit").expect("canonical"),
        SubstrateId::at(id),
    )
}

fn spawn_running(tree: &mut RegionTree, region: RegionId) -> WorkerId {
    let worker = tree.spawn(region, Resumability::Resumable).expect("open");
    tree.advance(worker, WorkerStep::Begin).expect("legal");
    worker
}

fn counters(tree: &RegionTree) -> (u64, u64, Vec<Obligation>) {
    (
        tree.ledger().opened(),
        tree.ledger().discharged(),
        tree.ledger().outstanding(),
    )
}

// --- the single-task cancellation -----------------------------------------------------

#[test]
fn a_worker_is_cancelled_alone_while_its_region_stays_open() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let worker = spawn_running(&mut tree, root);
    let bystander = spawn_running(&mut tree, root);
    tree.advance(worker, WorkerStep::Reserve).expect("legal");
    tree.advance(worker, WorkerStep::Commit).expect("legal");
    assert_eq!(tree.cancel_phase(worker), Ok(CancelPhase::Active));

    assert_eq!(
        tree.advance(worker, WorkerStep::RequestCancel),
        Ok(WorkerState::Running),
        "a request changes no state"
    );
    assert_eq!(tree.cancel_phase(worker), Ok(CancelPhase::Requested));
    assert_eq!(
        tree.cancel_phase(bystander),
        Ok(CancelPhase::Active),
        "one worker's request reaches no other"
    );
    tree.advance(worker, WorkerStep::AcknowledgeCancel)
        .expect("requested");
    assert_eq!(tree.cancel_phase(worker), Ok(CancelPhase::Acknowledged));
    assert_eq!(
        tree.advance(worker, WorkerStep::CompleteCancelled),
        Ok(WorkerState::Cancelled)
    );
    assert_eq!(
        tree.state(root),
        Ok(continuum_task::region::RegionState::Open)
    );

    // The region carries on, and closes normally around the cancelled worker.
    tree.advance(bystander, WorkerStep::Complete)
        .expect("legal");
    tree.close(root).expect("open");
    tree.drain(root).expect("every worker is terminal");
    let finalization = tree.finalize(root).expect("drained");
    assert!(finalization.is_total(), "{}", finalization.render());
    let report = finalization
        .workers()
        .iter()
        .find(|report| report.worker() == worker)
        .expect("reported");
    assert_eq!(report.state(), &WorkerState::Cancelled);
    assert_eq!(
        report.cancel_outcome(),
        Some(&CancelOutcome::CommittedWithContinuation(
            Continuation::new(worker, 1)
        )),
        "the single-task cancellation leaves RFC 0026's both-or-neither outcome"
    );
}

#[test]
fn the_single_task_completion_discards_what_is_still_staged() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let worker = spawn_running(&mut tree, root);
    tree.advance(worker, WorkerStep::ReserveSlot(PublicationSlot::at(4)))
        .expect("legal");
    tree.advance(worker, WorkerStep::RequestCancel)
        .expect("legal");
    tree.advance(worker, WorkerStep::AcknowledgeCancel)
        .expect("legal");
    tree.advance(worker, WorkerStep::CompleteCancelled)
        .expect("legal");
    let evidence = tree.evidence(worker).expect("admitted");
    assert!(!evidence.is_provisional());
    assert_eq!(evidence.committed(), 0);
    let finalization = tree.teardown(root).expect("torn down");
    assert!(finalization.is_total(), "{}", finalization.render());
    assert_eq!(
        finalization.workers()[0].cancel_outcome(),
        Some(&CancelOutcome::NothingPublished)
    );
}

#[test]
fn the_three_steps_are_refused_out_of_order_and_change_nothing() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let worker = spawn_running(&mut tree, root);
    let before = counters(&tree);
    assert_eq!(
        tree.advance(worker, WorkerStep::AcknowledgeCancel),
        Err(RegionFault::CancelNotRequested { worker })
    );
    assert_eq!(
        tree.advance(worker, WorkerStep::CompleteCancelled),
        Err(RegionFault::CancelNotAcknowledged { worker })
    );
    tree.advance(worker, WorkerStep::RequestCancel)
        .expect("legal");
    assert_eq!(
        tree.advance(worker, WorkerStep::RequestCancel),
        Err(RegionFault::CancelAlreadyRequested { worker })
    );
    assert_eq!(
        tree.advance(worker, WorkerStep::CompleteCancelled),
        Err(RegionFault::CancelNotAcknowledged { worker })
    );
    tree.advance(worker, WorkerStep::AcknowledgeCancel)
        .expect("legal");
    assert_eq!(
        tree.advance(worker, WorkerStep::AcknowledgeCancel),
        Err(RegionFault::CancelAlreadyAcknowledged { worker })
    );
    assert_eq!(
        counters(&tree),
        before,
        "refusals and requests move no counter"
    );
    tree.advance(worker, WorkerStep::CompleteCancelled)
        .expect("legal");
    for step in [
        WorkerStep::RequestCancel,
        WorkerStep::AcknowledgeCancel,
        WorkerStep::CompleteCancelled,
    ] {
        assert_eq!(
            tree.advance(worker, step.clone()),
            Err(RegionFault::IllegalWorkerStep {
                worker,
                from: WorkerState::Cancelled,
                step,
            }),
            "a terminal status never changes"
        );
    }
}

#[test]
fn a_region_cancellation_is_a_request_the_worker_acknowledges_itself() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let child = tree.open_child(root).expect("open");
    let worker = spawn_running(&mut tree, child);
    tree.cancel(root).expect("open");
    assert_eq!(
        tree.cancel_phase(worker),
        Ok(CancelPhase::Requested),
        "the region's request reaches every worker in its subtree"
    );
    assert_eq!(
        tree.advance(worker, WorkerStep::RequestCancel),
        Err(RegionFault::CancelAlreadyRequested { worker })
    );
    tree.advance(worker, WorkerStep::AcknowledgeCancel)
        .expect("the region requested it");
    let finalization = tree.teardown(root).expect("torn down");
    assert!(finalization.is_total(), "{}", finalization.render());
    assert_eq!(finalization.workers()[0].state(), &WorkerState::Cancelled);
}

#[test]
fn a_parked_or_unbegun_worker_acknowledges_at_its_next_poll() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let parked = spawn_running(&mut tree, root);
    tree.advance(parked, WorkerStep::Suspend).expect("legal");
    let unbegun = tree.spawn(root, Resumability::Resumable).expect("open");
    for worker in [parked, unbegun] {
        tree.advance(worker, WorkerStep::RequestCancel)
            .expect("legal");
        tree.advance(worker, WorkerStep::AcknowledgeCancel)
            .expect("the substrate's checkpoint comes before any lifecycle mark");
        assert_eq!(
            tree.advance(worker, WorkerStep::CompleteCancelled),
            Ok(WorkerState::Cancelled)
        );
    }
}

#[test]
fn mutant_an_acknowledged_worker_takes_an_ordinary_step() {
    let reason = FailureReason::new("engine-defect").expect("canonical");
    let mut tree = RegionTree::new();
    let root = tree.root();
    let worker = spawn_running(&mut tree, root);
    tree.advance(worker, WorkerStep::ReserveSlot(PublicationSlot::at(1)))
        .expect("legal");
    tree.advance(worker, WorkerStep::RequestCancel)
        .expect("legal");
    // Requested, not acknowledged: the worker still acts.
    tree.advance(worker, WorkerStep::ReserveSlot(PublicationSlot::at(2)))
        .expect("before the acknowledgement");
    tree.advance(worker, WorkerStep::CommitSlot(PublicationSlot::at(2)))
        .expect("before the acknowledgement");
    tree.advance(worker, WorkerStep::AcknowledgeCancel)
        .expect("legal");
    let before = counters(&tree);
    for step in [
        WorkerStep::Suspend,
        WorkerStep::Complete,
        WorkerStep::Fail(reason),
        WorkerStep::ReserveSlot(PublicationSlot::at(3)),
        WorkerStep::CommitSlot(PublicationSlot::at(1)),
        WorkerStep::Reserve,
    ] {
        assert_eq!(
            tree.advance(worker, step.clone()),
            Err(RegionFault::WorkerCancelling { worker, step }),
            "docs/02 §7: `Cancelling` only drains and finalizes"
        );
    }
    assert_eq!(counters(&tree), before);
    // Draining is admitted: the abort of what it staged, then the completion.
    tree.advance(worker, WorkerStep::AbortSlot(PublicationSlot::at(1)))
        .expect("an abort is the cleanup");
    tree.advance(worker, WorkerStep::CompleteCancelled)
        .expect("legal");
    assert_eq!(
        tree.evidence(worker).expect("admitted").committed(),
        1,
        "INV-009: the commit before the acknowledgement stands"
    );
}

// --- the phase a worker reached survives its region's finalization (cr-3pu5cu) ------

/// Every worker the tree ever admitted, with its phase.
fn phases(tree: &RegionTree) -> Vec<(WorkerId, CancelPhase)> {
    (0..u32::try_from(tree.worker_count()).expect("small"))
        .map(|ordinal| {
            let worker = WorkerId::at(ordinal);
            (worker, tree.cancel_phase(worker).expect("admitted"))
        })
        .collect()
}

/// `Cancelled ⇒ phase ≠ Active`: a cancelled worker's history includes a request.
fn assert_no_impossible_history(tree: &RegionTree) {
    for (worker, phase) in phases(tree) {
        if tree.worker_state(worker) == Ok(&WorkerState::Cancelled) {
            assert_ne!(
                phase,
                CancelPhase::Active,
                "{worker} is cancelled and active"
            );
        }
    }
}

/// Region-only cancellation, for a root subtree, a child subtree and an ancestor's
/// cancel, over workers that are created, running, parked, acknowledged, and already
/// complete: after drain and finalize every worker reports the phase it had before, and
/// a repeated cancel changes nothing.
#[test]
fn a_region_cancellation_s_request_survives_finalization() {
    for target in ["root", "child", "ancestor"] {
        let mut tree = RegionTree::new();
        let root = tree.root();
        let child = tree.open_child(root).expect("open");
        let grandchild = tree.open_child(child).expect("open");
        let created = tree
            .spawn(grandchild, Resumability::Resumable)
            .expect("open");
        let running = spawn_running(&mut tree, child);
        let parked = spawn_running(&mut tree, grandchild);
        tree.advance(parked, WorkerStep::Suspend).expect("legal");
        let acknowledging = spawn_running(&mut tree, child);
        let done = spawn_running(&mut tree, child);
        tree.advance(done, WorkerStep::Complete).expect("legal");
        let bystander = spawn_running(&mut tree, root);

        // `ancestor` cancels the root and finalizes the child subtree alone.
        let (cancelled, finalized) = match target {
            "root" => (root, root),
            "child" => (child, child),
            _ => (root, child),
        };
        tree.cancel(cancelled).expect("not finalized");
        tree.advance(acknowledging, WorkerStep::AcknowledgeCancel)
            .expect("its region requested it");
        let before = phases(&tree);
        for worker in [created, running, parked] {
            assert_eq!(
                tree.cancel_phase(worker),
                Ok(CancelPhase::Requested),
                "{target}"
            );
        }
        assert_eq!(
            tree.cancel_phase(done),
            Ok(CancelPhase::Active),
            "terminal before"
        );
        let opened = tree.ledger().opened();
        tree.cancel(cancelled).expect("a repeated request");
        assert_eq!(
            phases(&tree),
            before,
            "{target}: a repeated cancel is idempotent"
        );
        assert_eq!(
            tree.ledger().opened(),
            opened,
            "{target}: and opens nothing"
        );

        tree.drain(finalized).expect("cancelled");
        let finalization = tree.finalize(finalized).expect("drained");
        assert!(finalization.is_total(), "{}", finalization.render());
        assert_eq!(
            phases(&tree),
            before,
            "{target}: the phase each worker reached survives finalization"
        );
        for worker in [created, running, parked, acknowledging] {
            assert_eq!(tree.worker_state(worker), Ok(&WorkerState::Cancelled));
        }
        assert_no_impossible_history(&tree);
        if target == "child" {
            assert_eq!(
                tree.cancel_phase(bystander),
                Ok(CancelPhase::Active),
                "a sibling's cancel reaches no worker outside its subtree"
            );
        }
        let _ = tree.teardown(root);
        assert_no_impossible_history(&tree);
    }
}

// --- adapter obligations turn at the acknowledgement ----------------------------------

#[test]
fn a_requested_worker_still_acts_for_the_substrate_until_it_acknowledges() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let a = spawn_running(&mut tree, root);
    let b = spawn_running(&mut tree, root);
    tree.advance(a, WorkerStep::RequestCancel).expect("legal");
    // Requested in an open region: open, transfer and commit as before.
    tree.open_substrate(a, permit(0)).expect("still acting");
    tree.transfer_substrate(a, permit(0), b)
        .expect("still acting");
    tree.transfer_substrate(b, permit(0), a)
        .expect("to a worker still acting");
    tree.open_substrate(a, permit(1)).expect("still acting");
    tree.discharge_substrate(a, permit(1), SubstrateOutcome::Committed)
        .expect("still acting");

    tree.advance(a, WorkerStep::AcknowledgeCancel)
        .expect("legal");
    let before = counters(&tree);
    assert_eq!(
        tree.open_substrate(a, permit(2)),
        Err(RegionFault::SubstrateHolderCancelling { worker: a })
    );
    assert_eq!(
        tree.transfer_substrate(a, permit(0), b),
        Err(RegionFault::SubstrateHolderCancelling { worker: a })
    );
    assert_eq!(
        tree.discharge_substrate(a, permit(0), SubstrateOutcome::Committed),
        Err(RegionFault::SubstrateCommitDuringCancellation {
            obligation: permit(0),
            worker: a
        })
    );
    assert_eq!(counters(&tree), before, "every refusal moves nothing");
    tree.discharge_substrate(a, permit(0), SubstrateOutcome::Aborted)
        .expect("the cleanup abort");
    tree.advance(a, WorkerStep::CompleteCancelled)
        .expect("legal");
    tree.advance(b, WorkerStep::Complete).expect("legal");
    tree.close(root).expect("open");
    tree.drain(root).expect("terminal");
    let finalization = tree.finalize(root).expect("drained");
    assert!(finalization.is_total(), "{}", finalization.render());
}

#[test]
fn a_single_task_cancellation_that_leaks_an_obligation_is_reported() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let worker = spawn_running(&mut tree, root);
    tree.open_substrate(worker, permit(7)).expect("legal");
    tree.advance(worker, WorkerStep::RequestCancel)
        .expect("legal");
    tree.advance(worker, WorkerStep::AcknowledgeCancel)
        .expect("legal");
    tree.advance(worker, WorkerStep::CompleteCancelled)
        .expect("the calculus reports a leak, it does not absorb it");
    assert_eq!(
        tree.discharge_substrate(worker, permit(7), SubstrateOutcome::Aborted),
        Err(RegionFault::SubstrateHolderNotLive {
            worker,
            state: WorkerState::Cancelled
        }),
        "a leak is not laundered after the holder ended"
    );
    let finalization = tree.teardown(root).expect("torn down");
    assert!(!finalization.is_total());
    assert_eq!(
        finalization.ledger().outstanding(),
        &[permit(7).obligation()]
    );
}

// --- the subtree-scoped ledger --------------------------------------------------------

#[test]
fn a_child_finalizes_totally_while_work_elsewhere_still_owes() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let left = tree.open_child(root).expect("open");
    let right = tree.open_child(root).expect("open");
    let busy = spawn_running(&mut tree, right);
    tree.open_substrate(busy, permit(3)).expect("legal");
    let done = spawn_running(&mut tree, left);
    tree.advance(done, WorkerStep::Complete).expect("legal");
    tree.cancel(root).expect("open");
    tree.drain(left).expect("terminal");
    let finalization = tree.finalize(left).expect("drained");
    assert!(
        finalization.is_total(),
        "the left subtree owes nothing, whatever the right one owes: {}",
        finalization.render()
    );
    assert!(finalization.ledger().outstanding().is_empty());
    assert!(
        !finalization.tree_ledger().is_balanced(),
        "the whole tree still owes: the busy worker, its obligation, the root's cancellation"
    );
    assert!(
        finalization
            .tree_ledger()
            .outstanding()
            .contains(&permit(3).obligation())
    );
}

#[test]
fn a_child_that_owes_an_obligation_is_not_total() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let child = tree.open_child(root).expect("open");
    let holder = spawn_running(&mut tree, child);
    tree.open_substrate(holder, permit(5)).expect("legal");
    tree.advance(holder, WorkerStep::Complete).expect("legal");
    tree.close(child).expect("open");
    tree.drain(child).expect("terminal");
    let finalization = tree.finalize(child).expect("drained");
    assert!(!finalization.is_total(), "{}", finalization.render());
    assert_eq!(
        finalization.ledger().outstanding(),
        &[permit(5).obligation()]
    );
}

#[test]
fn an_obligation_transferred_out_is_the_receivers_subtrees_to_settle() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let left = tree.open_child(root).expect("open");
    let right = tree.open_child(root).expect("open");
    let giver = spawn_running(&mut tree, left);
    let taker = spawn_running(&mut tree, right);
    tree.open_substrate(giver, permit(8)).expect("legal");
    tree.transfer_substrate(giver, permit(8), taker)
        .expect("legal");
    tree.advance(giver, WorkerStep::Complete).expect("legal");
    tree.close(left).expect("open");
    tree.drain(left).expect("terminal");
    let left_report = tree.finalize(left).expect("drained");
    assert!(
        left_report.is_total(),
        "the giver handed the obligation on: {}",
        left_report.render()
    );
    tree.advance(taker, WorkerStep::Complete).expect("legal");
    tree.close(right).expect("open");
    tree.drain(right).expect("terminal");
    let right_report = tree.finalize(right).expect("drained");
    assert!(
        !right_report.is_total(),
        "the receiver ended holding it: {}",
        right_report.render()
    );
}

#[test]
fn a_root_finalization_ledger_is_the_whole_tree_ledger() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let child = tree.open_child(root).expect("open");
    let one = spawn_running(&mut tree, child);
    let two = spawn_running(&mut tree, root);
    tree.open_substrate(one, permit(0)).expect("legal");
    tree.advance(two, WorkerStep::Reserve).expect("legal");
    let finalization = tree.teardown(root).expect("torn down");
    assert_eq!(finalization.ledger(), finalization.tree_ledger());
    assert!(!finalization.is_total(), "the obligation leaked");
}

// --- schedules ------------------------------------------------------------------------

/// Two workers, each requesting, acknowledging and completing its own cancellation
/// around a staged publication and a commit, interleaved every way and cut at every
/// point by a teardown. Every interleaving tears down totally, and every schedule's own
/// steps are admitted.
#[test]
fn every_interleaving_of_single_task_cancellations_tears_down_totally() {
    let program = |worker: u32| {
        let w = WorkerId::at(worker);
        vec![
            Step::advance(w, WorkerStep::Begin),
            Step::advance(w, WorkerStep::ReserveSlot(PublicationSlot::at(1))),
            Step::advance(w, WorkerStep::RequestCancel),
            Step::advance(w, WorkerStep::CommitSlot(PublicationSlot::at(1))),
            Step::advance(w, WorkerStep::ReserveSlot(PublicationSlot::at(2))),
            Step::advance(w, WorkerStep::AcknowledgeCancel),
            Step::advance(w, WorkerStep::AbortSlot(PublicationSlot::at(2))),
            Step::advance(w, WorkerStep::CompleteCancelled),
        ]
    };
    let schedules = interleavings(&[program(0), program(1)]);
    assert_eq!(schedules.len(), 12_870, "16! / (8! * 8!)");
    let root = RegionId::at(0);
    let setup = Schedule::new(vec![
        Step::Spawn {
            region: root,
            resumability: Resumability::Resumable,
        },
        Step::Spawn {
            region: root,
            resumability: Resumability::Resumable,
        },
    ]);
    let mut torn = 0_usize;
    for schedule in schedules.iter().step_by(3) {
        for cut in [4, 9, schedule.len()] {
            let prefix = Schedule::new(schedule.steps()[..cut].to_vec());
            let mut tree = RegionTree::new();
            let run = setup.clone().then(&prefix).run(&mut tree);
            assert!(run.faults().is_empty(), "{}", run.render());
            let finalization = tree.teardown(root).expect("torn down");
            assert!(finalization.is_total(), "{}", finalization.render());
            assert_no_impossible_history(&tree);
            if cut == schedule.len() {
                for report in finalization.workers() {
                    assert_eq!(
                        report.cancel_outcome(),
                        Some(&CancelOutcome::CommittedWithContinuation(
                            Continuation::new(report.worker(), 1)
                        ))
                    );
                }
            }
            torn += 1;
        }
    }
    assert_eq!(torn, 4_290 * 3);
}

/// Metamorphic relation: independent-event swap. Two different workers' cancellation
/// steps touch disjoint state, so swapping two adjacent ones leaves the teardown's
/// canonical render byte-identical.
#[test]
fn independent_event_swap_of_two_workers_cancellation_steps_preserves_the_render() {
    let build = |order: &[(u32, WorkerStep)]| {
        let mut tree = RegionTree::new();
        let root = tree.root();
        let a = spawn_running(&mut tree, root);
        let b = spawn_running(&mut tree, root);
        assert_eq!((a, b), (WorkerId::at(0), WorkerId::at(1)));
        for (worker, step) in order {
            tree.advance(WorkerId::at(*worker), step.clone())
                .expect("legal");
        }
        tree.teardown(root).expect("torn down").render()
    };
    let steps = [
        (0, WorkerStep::RequestCancel),
        (1, WorkerStep::RequestCancel),
        (0, WorkerStep::AcknowledgeCancel),
        (1, WorkerStep::AcknowledgeCancel),
        (0, WorkerStep::CompleteCancelled),
    ];
    let reference = build(&steps);
    for i in 0..steps.len() - 1 {
        if steps[i].0 == steps[i + 1].0 {
            continue;
        }
        let mut swapped = steps.clone();
        swapped.swap(i, i + 1);
        assert_eq!(build(&swapped).as_bytes(), reference.as_bytes(), "swap {i}");
    }
    assert!(reference.contains("balanced=true"), "{reference}");
}
