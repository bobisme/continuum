//! The public, typed ledger entry point for adapter obligations (RFC 0026 correction 51,
//! bn-2318t).
//!
//! > Obligations are linear semantic resources created and discharged by events.
//! > Examples include reply obligations, reserved channel capacity, outstanding
//! > durable-write acknowledgements, child-region quiescence, and cancellation
//! > finalization.
//! >
//! > — `notes/plan/rfcs/0001-causal-intermediate-representation.md`, "Obligations"
//!
//! Before this change the calculus's `Ledger::open` and `discharge` were crate-private, so
//! an adapter kept its substrate's obligations in a parallel ledger that
//! `Finalization::is_total` never read. [`RegionTree::open_substrate`],
//! [`RegionTree::discharge_substrate`] and [`RegionTree::transfer_substrate`] put them in
//! the one ledger. These tests hold the entry point to two claims:
//!
//! - **coverage** — an adapter obligation open at finalize makes `is_total` false and is
//!   named in the report; discharged, it balances;
//! - **no forged balance** — every open needs a running or parked holder in an open region
//!   and a fresh identity, every discharge needs a matching open held by the named worker
//!   before it terminates, and every refusal is typed and changes nothing.

use continuum_task::region::obligation::{
    Obligation, ObligationKind, Subject, SubstrateId, SubstrateKind, SubstrateObligation,
    SubstrateOutcome,
};
use continuum_task::region::schedule::{Schedule, Step, StepOutcome};
use continuum_task::region::worker::{
    FailureReason, Resumability, WorkerId, WorkerState, WorkerStep,
};
use continuum_task::region::{DrainCause, RegionFault, RegionId, RegionState, RegionTree};

fn kind(token: &'static str) -> SubstrateKind {
    SubstrateKind::new(token).expect("canonical token")
}

fn permit(id: u64) -> SubstrateObligation {
    SubstrateObligation::new(kind("send-permit"), SubstrateId::at(id))
}

fn lease(id: u64) -> SubstrateObligation {
    SubstrateObligation::new(kind("lease"), SubstrateId::at(id))
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

// --- coverage ------------------------------------------------------------------------

#[test]
fn a_discharged_adapter_obligation_balances_and_an_open_one_is_a_reported_leak() {
    // Discharged: total.
    let mut tree = RegionTree::new();
    let root = tree.root();
    let worker = spawn_running(&mut tree, root);
    tree.open_substrate(worker, permit(0))
        .expect("live holder, open region");
    assert!(tree.ledger().holds(permit(0).obligation()));
    assert_eq!(tree.substrate_holder(SubstrateId::at(0)), Some(worker));
    tree.discharge_substrate(worker, permit(0), SubstrateOutcome::Committed)
        .expect("matching open");
    assert_eq!(tree.substrate_holder(SubstrateId::at(0)), None);
    tree.advance(worker, WorkerStep::Complete).expect("legal");
    tree.close(root).expect("open");
    tree.drain(root).expect("terminal");
    let finalization = tree.finalize(root).expect("drained");
    assert!(finalization.is_total(), "{}", finalization.render());
    assert_eq!(finalization.ledger().opened(), 2);

    // Left open: the worker completes over it, and the teardown reports it.
    let mut tree = RegionTree::new();
    let root = tree.root();
    let worker = spawn_running(&mut tree, root);
    tree.open_substrate(worker, lease(3)).expect("legal");
    tree.advance(worker, WorkerStep::Complete).expect("legal");
    let finalization = tree.teardown(root).expect("torn down");
    assert!(
        !finalization.is_total(),
        "an adapter obligation open at finalize is a leak, and is_total must say so"
    );
    assert!(finalization.orphans().is_empty());
    assert!(finalization.unresolved_publications().is_empty());
    assert_eq!(
        finalization.ledger().outstanding(),
        &[Obligation::new(
            ObligationKind::Substrate(kind("lease")),
            Subject::Substrate(SubstrateId::at(3))
        )]
    );
    assert!(
        finalization.render().contains("substrate(lease):s3"),
        "{}",
        finalization.render()
    );
}

#[test]
fn a_cancelling_teardown_does_not_absorb_an_adapter_obligation() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let child = tree.open_child(root).expect("open");
    let worker = spawn_running(&mut tree, child);
    tree.open_substrate(worker, permit(1)).expect("legal");
    let finalization = tree.teardown(root).expect("torn down");
    assert!(
        !finalization.is_total(),
        "the calculus discharges its own obligations at teardown, never the adapter's"
    );
    assert_eq!(
        finalization.ledger().outstanding(),
        &[permit(1).obligation()]
    );
}

#[test]
fn an_adapter_obligation_discharged_during_cancellation_balances() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let worker = spawn_running(&mut tree, root);
    tree.open_substrate(worker, permit(1)).expect("legal");
    tree.cancel(root).expect("open");
    tree.discharge_substrate(worker, permit(1), SubstrateOutcome::Aborted)
        .expect("a live holder in a cancelling region may still discharge");
    tree.drain(root).expect("cancelled");
    let finalization = tree.finalize(root).expect("drained");
    assert!(finalization.is_total(), "{}", finalization.render());
}

#[test]
fn a_transfer_moves_the_holder_across_regions_and_not_the_ledger() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let left = tree.open_child(root).expect("open");
    let right = tree.open_child(root).expect("open");
    let giver = spawn_running(&mut tree, left);
    let taker = spawn_running(&mut tree, right);
    tree.open_substrate(giver, permit(9)).expect("legal");
    let before = counters(&tree);
    tree.transfer_substrate(giver, permit(9), taker)
        .expect("a live holder hands on to a live worker in an open region");
    assert_eq!(
        counters(&tree),
        before,
        "a transfer is not a discharge and an open"
    );
    assert_eq!(tree.substrate_holder(SubstrateId::at(9)), Some(taker));
    assert_eq!(
        tree.discharge_substrate(giver, permit(9), SubstrateOutcome::Committed),
        Err(RegionFault::SubstrateHolderMismatch {
            obligation: permit(9),
            named: giver,
            holder: taker
        }),
        "the giver no longer holds it"
    );
    tree.discharge_substrate(taker, permit(9), SubstrateOutcome::Committed)
        .expect("the taker holds it");
    for worker in [giver, taker] {
        tree.advance(worker, WorkerStep::Complete).expect("legal");
    }
    tree.close(root).expect("open");
    tree.drain(root).expect("terminal");
    assert!(tree.finalize(root).expect("drained").is_total());
}

// --- no forged balance: the mutants --------------------------------------------------

#[test]
fn mutant_discharge_without_open_is_refused() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let worker = spawn_running(&mut tree, root);
    let before = counters(&tree);
    assert_eq!(
        tree.discharge_substrate(worker, permit(0), SubstrateOutcome::Committed),
        Err(RegionFault::SubstrateNotOpen {
            obligation: permit(0)
        })
    );
    assert_eq!(
        counters(&tree),
        before,
        "a refused discharge moves no counter"
    );

    // A second discharge of the same obligation is a discharge without open.
    tree.open_substrate(worker, permit(0)).expect("legal");
    tree.discharge_substrate(worker, permit(0), SubstrateOutcome::Committed)
        .expect("legal");
    let before = counters(&tree);
    assert_eq!(
        tree.discharge_substrate(worker, permit(0), SubstrateOutcome::Committed),
        Err(RegionFault::SubstrateNotOpen {
            obligation: permit(0)
        })
    );
    assert_eq!(counters(&tree), before);

    // So is a discharge that names the right identity with the wrong kind.
    tree.open_substrate(worker, permit(1)).expect("legal");
    let wrong_kind = SubstrateObligation::new(kind("lease"), SubstrateId::at(1));
    assert_eq!(
        tree.discharge_substrate(worker, wrong_kind, SubstrateOutcome::Committed),
        Err(RegionFault::SubstrateNotOpen {
            obligation: wrong_kind
        })
    );
    assert!(tree.ledger().holds(permit(1).obligation()), "still owed");
}

#[test]
fn mutant_cross_region_discharge_is_refused() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let left = tree.open_child(root).expect("open");
    let right = tree.open_child(root).expect("open");
    let holder = spawn_running(&mut tree, left);
    let stranger = spawn_running(&mut tree, right);
    tree.open_substrate(holder, permit(4)).expect("legal");
    let before = counters(&tree);
    assert_eq!(
        tree.discharge_substrate(stranger, permit(4), SubstrateOutcome::Committed),
        Err(RegionFault::SubstrateHolderMismatch {
            obligation: permit(4),
            named: stranger,
            holder
        })
    );
    assert_eq!(
        tree.transfer_substrate(stranger, permit(4), stranger),
        Err(RegionFault::SubstrateHolderMismatch {
            obligation: permit(4),
            named: stranger,
            holder
        }),
        "a non-holder cannot hand it on either"
    );
    assert_eq!(counters(&tree), before);
    assert_eq!(tree.substrate_holder(SubstrateId::at(4)), Some(holder));
}

#[test]
fn mutant_open_by_a_holder_that_is_not_live_is_refused() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let done = spawn_running(&mut tree, root);
    tree.advance(done, WorkerStep::Complete).expect("legal");
    let failed = spawn_running(&mut tree, root);
    let reason = FailureReason::new("engine-defect").expect("canonical");
    tree.advance(failed, WorkerStep::Fail(reason.clone()))
        .expect("legal");

    let before = counters(&tree);
    for (worker, state) in [
        (done, WorkerState::Completed),
        (failed, WorkerState::Failed(reason)),
    ] {
        assert_eq!(
            tree.open_substrate(worker, permit(u64::from(worker.ordinal()))),
            Err(RegionFault::SubstrateHolderNotLive { worker, state })
        );
    }
    assert_eq!(
        tree.open_substrate(WorkerId::at(99), permit(99)),
        Err(RegionFault::UnknownWorker(WorkerId::at(99)))
    );
    assert_eq!(counters(&tree), before);
}

/// Mutant: a worker that has not begun opens an obligation. It has taken no step, so it
/// holds nothing, and the open is refused with its own typed reason.
#[test]
fn mutant_open_by_a_worker_that_has_not_begun_is_refused() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let created = tree.spawn(root, Resumability::Resumable).expect("open");
    let before = counters(&tree);
    assert_eq!(
        tree.open_substrate(created, permit(0)),
        Err(RegionFault::SubstrateHolderNotBegun { worker: created })
    );
    assert_eq!(counters(&tree), before, "a refused open moves no counter");
    assert_eq!(tree.substrate_holder(SubstrateId::at(0)), None);

    // The same identity opens once the worker has begun: the refusal consumed nothing.
    tree.advance(created, WorkerStep::Begin).expect("legal");
    tree.open_substrate(created, permit(0))
        .expect("a running worker opens");
}

/// A parked worker takes full part: the substrate applies an open after the poll that
/// made it returns, hands obligations between parked tasks, and discharges them before
/// the holder resumes.
#[test]
fn a_parked_worker_opens_hands_on_and_discharges() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let giver = spawn_running(&mut tree, root);
    let taker = spawn_running(&mut tree, root);
    for worker in [giver, taker] {
        tree.advance(worker, WorkerStep::Suspend).expect("legal");
    }
    tree.open_substrate(giver, permit(8))
        .expect("an open applied after the poll returned");
    tree.transfer_substrate(giver, permit(8), taker)
        .expect("a hand-off between parked tasks");
    assert_eq!(tree.substrate_holder(SubstrateId::at(8)), Some(taker));
    tree.discharge_substrate(taker, permit(8), SubstrateOutcome::Committed)
        .expect("a parked holder discharges");
    for worker in [giver, taker] {
        tree.advance(worker, WorkerStep::Resume).expect("legal");
        tree.advance(worker, WorkerStep::Complete).expect("legal");
    }
    tree.close(root).expect("open");
    tree.drain(root).expect("terminal");
    assert!(tree.finalize(root).expect("drained").is_total());
}

/// The real journal shape, replayed as calculus steps:
/// `crates/continuum-asupersync/tests/pr14_impl04_obligations.rs`,
/// `a_cancelled_subtree_settles_its_ledger_in_canonical_order`. Two sibling regions
/// under a cancelled parent; each task opens an obligation while parked; `o1` is handed
/// across regions between two parked tasks; the parent is cancelled; both obligations
/// are discharged inside the cancellation, before either lifecycle is terminal; then
/// each region drains and finalizes in the journal's order. The teardown is total.
#[test]
fn the_pr14_impl04_journal_shape_replays_as_a_total_teardown() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    // 0-4: r1 under r0, r2 and r3 under r1; t0 in r2, t1 in r3.
    let r1 = tree.open_child(root).expect("open");
    let r2 = tree.open_child(r1).expect("open");
    let r3 = tree.open_child(r1).expect("open");
    let t0 = tree.spawn(r2, Resumability::Resumable).expect("open");
    let t1 = tree.spawn(r3, Resumability::Resumable).expect("open");
    let step = |tree: &mut RegionTree, worker, step| {
        tree.advance(worker, step)
            .expect("the journal's step is legal");
    };
    // 5-10.
    step(&mut tree, t0, WorkerStep::Begin);
    step(&mut tree, t0, WorkerStep::Suspend);
    step(&mut tree, t1, WorkerStep::Begin);
    step(&mut tree, t1, WorkerStep::Suspend);
    step(&mut tree, t0, WorkerStep::Resume);
    step(&mut tree, t0, WorkerStep::Suspend);
    // 11: o0 ack opened by t0, parked.
    let o0 = SubstrateObligation::new(kind("ack"), SubstrateId::at(0));
    tree.open_substrate(t0, o0).expect("a parked holder opens");
    // 12-13, 14: o1 io-op opened by t1, parked.
    step(&mut tree, t1, WorkerStep::Resume);
    step(&mut tree, t1, WorkerStep::Suspend);
    let o1 = SubstrateObligation::new(kind("io-op"), SubstrateId::at(1));
    tree.open_substrate(t1, o1).expect("a parked holder opens");
    // 15-16, 17: o1 transferred t1 -> t0 across sibling regions, both parked.
    step(&mut tree, t1, WorkerStep::Resume);
    step(&mut tree, t1, WorkerStep::Suspend);
    tree.transfer_substrate(t1, o1, t0)
        .expect("a hand-off between parked tasks in sibling regions");
    // 18-21: r1 cancelled; both tasks are still parked, not terminal.
    tree.cancel(r1).expect("open");
    assert_eq!(
        tree.state(r2),
        Ok(RegionState::Draining(DrainCause::Cancelled))
    );
    assert_eq!(tree.worker_state(t0), Ok(&WorkerState::Suspended));
    // 22-23: both discharged (aborted) by t0 inside the cancellation.
    tree.discharge_substrate(t0, o0, SubstrateOutcome::Aborted)
        .expect("cleanup inside a cancellation balances");
    tree.discharge_substrate(t0, o1, SubstrateOutcome::Aborted)
        .expect("the transferred obligation is discharged by its new holder");
    // 24-35: r2, then r3, then r1 drain and finalize, each settled with nothing open.
    for region in [r2, r3] {
        tree.drain(region).expect("cancelled");
        let finalization = tree.finalize(region).expect("drained");
        assert!(finalization.orphans().is_empty());
    }
    tree.drain(r1).expect("cancelled");
    let finalization = tree.finalize(r1).expect("drained");
    assert!(finalization.is_total(), "{}", finalization.render());
    assert_eq!(finalization.workers().len(), 2);
    assert_eq!(tree.worker_state(t1), Ok(&WorkerState::Cancelled));
}

#[test]
fn mutant_open_in_a_region_that_stopped_accepting_work_is_refused() {
    for cancel in [false, true] {
        let mut tree = RegionTree::new();
        let root = tree.root();
        let worker = spawn_running(&mut tree, root);
        let state = if cancel {
            tree.cancel(root).expect("open");
            RegionState::Draining(DrainCause::Cancelled)
        } else {
            tree.close(root).expect("open");
            RegionState::Draining(DrainCause::Closed)
        };
        let before = counters(&tree);
        assert_eq!(
            tree.open_substrate(worker, permit(0)),
            Err(RegionFault::SubstrateRegionNotOpen {
                worker,
                region: root,
                state
            })
        );
        assert_eq!(counters(&tree), before);
    }
}

/// Mutant: a hand-off with a party that acknowledged its cancellation. The A7 model's
/// transfer needs both parties acting, and a task that has acknowledged its
/// cancellation only drains and finalizes, so the refusal holds at either end. Before
/// the acknowledgement the task still acts, even inside a cancelled region, so a
/// hand-off then is admitted (RFC 0026 correction 53 relaxes correction 51's
/// region-level rule). The discharge cleanup exception does not extend to transfers.
#[test]
fn mutant_transfer_with_a_party_that_acknowledged_cancellation_is_refused() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let open = tree.open_child(root).expect("open");
    let cancelling = tree.open_child(root).expect("open");
    let giver = spawn_running(&mut tree, open);
    let taker = spawn_running(&mut tree, cancelling);
    tree.open_substrate(giver, permit(2)).expect("legal");
    tree.open_substrate(taker, permit(3)).expect("legal");
    tree.cancel(cancelling).expect("open");

    // Requested, not acknowledged: the taker still acts, in both directions.
    tree.transfer_substrate(giver, permit(2), taker)
        .expect("into a cancelled region, before the acknowledgement");
    tree.transfer_substrate(taker, permit(2), giver)
        .expect("out of a cancelled region, before the acknowledgement");

    tree.advance(taker, WorkerStep::AcknowledgeCancel)
        .expect("the region's cancellation is requested");
    let before = counters(&tree);
    assert_eq!(
        tree.transfer_substrate(giver, permit(2), taker),
        Err(RegionFault::SubstrateHolderCancelling { worker: taker }),
        "into a worker that acknowledged"
    );
    assert_eq!(
        tree.transfer_substrate(taker, permit(3), giver),
        Err(RegionFault::SubstrateHolderCancelling { worker: taker }),
        "out of a worker that acknowledged"
    );
    assert_eq!(counters(&tree), before, "a refused transfer moves nothing");
    assert_eq!(tree.substrate_holder(SubstrateId::at(2)), Some(giver));
    assert_eq!(tree.substrate_holder(SubstrateId::at(3)), Some(taker));

    // The cancelling holder may still discharge: that is cleanup.
    tree.discharge_substrate(taker, permit(3), SubstrateOutcome::Aborted)
        .expect("the discharge keeps the cancellation-cleanup exception");
    tree.discharge_substrate(giver, permit(2), SubstrateOutcome::Committed)
        .expect("legal");
    let finalization = tree.teardown(root).expect("torn down");
    assert!(finalization.is_total(), "{}", finalization.render());
}

/// Mutant: a committed discharge after the holder acknowledged its cancellation. A task
/// in cancellation only drains and finalizes, so its discharges are aborts (cleanup). A
/// commit then is refused with its own typed reason and moves nothing; the abort is
/// admitted and the teardown balances. Before the acknowledgement a commit is admitted,
/// even inside a cancelled region (RFC 0026 correction 53).
#[test]
fn mutant_committed_discharge_after_acknowledgement_is_refused() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let cancelling = tree.open_child(root).expect("open");
    let cleaner = spawn_running(&mut tree, cancelling);
    tree.open_substrate(cleaner, permit(10)).expect("legal");
    tree.open_substrate(cleaner, permit(11)).expect("legal");
    tree.cancel(cancelling).expect("open");

    tree.discharge_substrate(cleaner, permit(11), SubstrateOutcome::Committed)
        .expect("requested, not acknowledged: the holder still commits");
    tree.advance(cleaner, WorkerStep::AcknowledgeCancel)
        .expect("the region's cancellation is requested");
    let before = counters(&tree);
    assert_eq!(
        tree.discharge_substrate(cleaner, permit(10), SubstrateOutcome::Committed),
        Err(RegionFault::SubstrateCommitDuringCancellation {
            obligation: permit(10),
            worker: cleaner
        })
    );
    assert_eq!(counters(&tree), before, "a refused commit moves no counter");
    assert_eq!(tree.substrate_holder(SubstrateId::at(10)), Some(cleaner));
    tree.discharge_substrate(cleaner, permit(10), SubstrateOutcome::Aborted)
        .expect("cleanup inside a cancellation is an abort");
    let finalization = tree.teardown(root).expect("torn down");
    assert!(finalization.is_total(), "{}", finalization.render());
}

/// Mutant: a self-transfer. A hand-off to the holder itself hands nothing, so it is
/// refused with its own typed reason, and nothing moves.
#[test]
fn mutant_self_transfer_is_refused() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let worker = spawn_running(&mut tree, root);
    tree.open_substrate(worker, permit(6)).expect("legal");
    let before = counters(&tree);
    assert_eq!(
        tree.transfer_substrate(worker, permit(6), worker),
        Err(RegionFault::SubstrateSelfTransfer {
            obligation: permit(6),
            worker
        })
    );
    assert_eq!(counters(&tree), before);
    assert_eq!(tree.substrate_holder(SubstrateId::at(6)), Some(worker));
}

/// Mutants on the transfer's two parties: a destination that has not begun, a
/// destination that has terminated, and a source that has terminated. Each is refused
/// with its typed reason and moves nothing.
#[test]
fn mutant_transfer_to_or_from_a_worker_outside_its_lifetime_is_refused() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let giver = spawn_running(&mut tree, root);
    let unbegun = tree.spawn(root, Resumability::Resumable).expect("open");
    let done = spawn_running(&mut tree, root);
    tree.advance(done, WorkerStep::Complete).expect("legal");
    tree.open_substrate(giver, permit(3)).expect("legal");
    let before = counters(&tree);
    assert_eq!(
        tree.transfer_substrate(giver, permit(3), unbegun),
        Err(RegionFault::SubstrateHolderNotBegun { worker: unbegun })
    );
    assert_eq!(
        tree.transfer_substrate(giver, permit(3), done),
        Err(RegionFault::SubstrateHolderNotLive {
            worker: done,
            state: WorkerState::Completed
        })
    );
    assert_eq!(counters(&tree), before);
    assert_eq!(tree.substrate_holder(SubstrateId::at(3)), Some(giver));

    // A terminated source cannot hand on the obligation it leaked.
    let taker = spawn_running(&mut tree, root);
    tree.advance(giver, WorkerStep::Complete).expect("legal");
    assert_eq!(
        tree.transfer_substrate(giver, permit(3), taker),
        Err(RegionFault::SubstrateHolderNotLive {
            worker: giver,
            state: WorkerState::Completed
        })
    );
    assert_eq!(tree.substrate_holder(SubstrateId::at(3)), Some(giver));
}

/// Mutant: a discharge after a cancelling drain has already terminated the holder. The
/// obligation leaked when the drain cancelled the holder; a discharge after that is
/// refused, and the teardown reports the leak.
#[test]
fn mutant_discharge_after_the_cancelling_drain_terminated_the_holder_is_refused() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let worker = spawn_running(&mut tree, root);
    tree.open_substrate(worker, permit(4)).expect("legal");
    tree.cancel(root).expect("open");
    tree.drain(root).expect("cancelled");
    assert_eq!(
        tree.discharge_substrate(worker, permit(4), SubstrateOutcome::Aborted),
        Err(RegionFault::SubstrateHolderNotLive {
            worker,
            state: WorkerState::Cancelled
        })
    );
    let finalization = tree.finalize(root).expect("drained");
    assert!(!finalization.is_total(), "{}", finalization.render());
    assert_eq!(
        finalization.ledger().outstanding(),
        &[permit(4).obligation()]
    );
}

#[test]
fn mutant_reopening_an_identity_is_refused_even_after_discharge() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let worker = spawn_running(&mut tree, root);
    tree.open_substrate(worker, permit(5)).expect("legal");
    assert_eq!(
        tree.open_substrate(worker, permit(5)),
        Err(RegionFault::SubstrateIdentityReused {
            obligation: permit(5)
        })
    );
    tree.discharge_substrate(worker, permit(5), SubstrateOutcome::Committed)
        .expect("legal");
    let before = counters(&tree);
    assert_eq!(
        tree.open_substrate(worker, lease(5)),
        Err(RegionFault::SubstrateIdentityReused {
            obligation: lease(5)
        }),
        "an identity opens once in a tree's life, whatever kind it is reopened as"
    );
    assert_eq!(counters(&tree), before);
}

#[test]
fn mutant_laundering_a_leak_after_the_holder_terminated_is_refused() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let worker = spawn_running(&mut tree, root);
    tree.open_substrate(worker, permit(6)).expect("legal");
    tree.advance(worker, WorkerStep::Complete).expect("legal");
    assert_eq!(
        tree.discharge_substrate(worker, permit(6), SubstrateOutcome::Committed),
        Err(RegionFault::SubstrateHolderNotLive {
            worker,
            state: WorkerState::Completed
        }),
        "the leak happened when the holder terminated; a later discharge must not hide it"
    );
    let finalization = tree.teardown(root).expect("torn down");
    assert!(!finalization.is_total());
}

#[test]
fn a_substrate_kind_token_must_be_canonical() {
    use continuum_task::region::worker::ReasonError;
    assert_eq!(SubstrateKind::new(""), Err(ReasonError::Empty));
    assert_eq!(
        SubstrateKind::new("send permit"),
        Err(ReasonError::NonCanonical {
            index: 4,
            character: ' '
        })
    );
    assert_eq!(kind("ack").as_str(), "ack");
}

// --- schedules ------------------------------------------------------------------------

#[test]
fn substrate_steps_run_render_and_refuse_as_schedule_data() {
    let root = RegionId::at(0);
    let w = WorkerId::at(0);
    let schedule = Schedule::new(vec![
        Step::Spawn {
            region: root,
            resumability: Resumability::Resumable,
        },
        Step::DischargeSubstrate {
            holder: w,
            obligation: permit(0),
            outcome: SubstrateOutcome::Committed,
        },
        Step::advance(w, WorkerStep::Begin),
        Step::OpenSubstrate {
            holder: w,
            obligation: permit(0),
        },
        Step::TransferSubstrate {
            from: w,
            obligation: permit(0),
            to: w,
        },
        Step::DischargeSubstrate {
            holder: w,
            obligation: permit(0),
            outcome: SubstrateOutcome::Committed,
        },
        Step::advance(w, WorkerStep::Complete),
        Step::Close { region: root },
        Step::Drain { region: root },
        Step::Finalize { region: root },
    ]);
    let first = schedule.run(&mut RegionTree::new());
    let second = schedule.run(&mut RegionTree::new());
    assert_eq!(first.render().as_bytes(), second.render().as_bytes());
    let faults = first.faults();
    assert_eq!(faults.len(), 2, "{}", first.render());
    assert_eq!(
        faults[0].1,
        &RegionFault::SubstrateNotOpen {
            obligation: permit(0)
        }
    );
    assert_eq!(
        faults[1].1,
        &RegionFault::SubstrateSelfTransfer {
            obligation: permit(0),
            worker: w
        },
        "a transfer from a worker to itself is refused"
    );
    assert!(
        first
            .records()
            .iter()
            .any(|(_, outcome)| outcome == &Ok(StepOutcome::Substrate(permit(0))))
    );
    assert!(first.finalizations()[0].is_total(), "{}", first.render());
    assert!(
        first
            .render()
            .contains("open-substrate w0 send-permit:s0 -> substrate send-permit:s0"),
        "{}",
        first.render()
    );
}
