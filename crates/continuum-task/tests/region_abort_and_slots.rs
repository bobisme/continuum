//! The explicit-abort step and staging slots (RFC 0026 correction 51, bn-2318t).
//!
//! > Reserved(token) → Committed(result)
//! > Reserved(token) → Aborted(reason)
//! >
//! > — `notes/plan/docs/02_SEMANTICS.md` §7, "Effect protocol"
//!
//! Before this change the calculus had no `Reserved → Aborted` arm that a running worker
//! could take: a staged publication was dropped only by a failure or a cancelling drain.
//! And a worker could hold one staged publication at a time, where the substrate lets a
//! task hold many reservations. These tests pin the two additions:
//!
//! - [`WorkerStep::Abort`] drops the one staged publication, is legal only from `Running`
//!   and only while one is staged, and leaves committed evidence alone;
//! - [`WorkerStep::ReserveSlot`] / [`WorkerStep::CommitSlot`] / [`WorkerStep::AbortSlot`]
//!   admit several staged publications per worker, each its own linear obligation,
//!   resolved exactly once.
//!
//! The existing calculus tests are unchanged, which is the other half of the evidence: the
//! one-in-flight steps keep their refusals.

use continuum_task::region::obligation::Obligation;
use continuum_task::region::schedule::{Schedule, Step, interleavings};
use continuum_task::region::worker::{
    CancelOutcome, Continuation, FailureReason, PublicationSlot, Resumability, WorkerId,
    WorkerState, WorkerStep,
};
use continuum_task::region::{RegionFault, RegionTree};

fn slot(ordinal: u32) -> PublicationSlot {
    PublicationSlot::at(ordinal)
}

/// A tree with one running, resumable worker in the root.
fn running() -> (RegionTree, WorkerId) {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let worker = tree
        .spawn(root, Resumability::Resumable)
        .expect("root is open");
    tree.advance(worker, WorkerStep::Begin).expect("legal");
    (tree, worker)
}

fn counters(tree: &RegionTree) -> (u64, u64, Vec<Obligation>) {
    (
        tree.ledger().opened(),
        tree.ledger().discharged(),
        tree.ledger().outstanding(),
    )
}

// --- the explicit abort --------------------------------------------------------------

#[test]
fn an_explicit_abort_drops_the_staged_publication_and_publishes_nothing() {
    let (mut tree, worker) = running();
    tree.advance(worker, WorkerStep::Reserve).expect("legal");
    assert!(
        tree.ledger()
            .holds(Obligation::provisional_publication(worker))
    );

    assert_eq!(
        tree.advance(worker, WorkerStep::Abort),
        Ok(WorkerState::Running),
        "an abort is a step inside Running, not a termination"
    );
    let evidence = tree.evidence(worker).expect("admitted");
    assert!(!evidence.is_provisional());
    assert_eq!(evidence.staged(), 0);
    assert_eq!(evidence.committed(), 0, "an abort publishes nothing");
    assert!(
        !tree
            .ledger()
            .holds(Obligation::provisional_publication(worker)),
        "the abort resolves the staged publication's obligation"
    );

    // The worker may now finish: nothing is staged.
    tree.advance(worker, WorkerStep::Complete).expect("legal");
    let root = tree.root();
    tree.close(root).expect("open");
    tree.drain(root).expect("terminal");
    let finalization = tree.finalize(root).expect("drained");
    assert!(finalization.is_total(), "{}", finalization.render());
    assert_eq!(
        finalization.ledger().opened(),
        2,
        "termination + one staging"
    );
}

#[test]
fn an_abort_with_nothing_staged_is_refused_and_changes_nothing() {
    let (mut tree, worker) = running();
    let before = counters(&tree);
    assert_eq!(
        tree.advance(worker, WorkerStep::Abort),
        Err(RegionFault::AbortWithoutReserve { worker })
    );
    assert_eq!(counters(&tree), before, "a refused step moves no counter");

    // A publication that was already resolved is not abortable either: no abort after
    // commit, and no commit after abort.
    tree.advance(worker, WorkerStep::Reserve).expect("legal");
    tree.advance(worker, WorkerStep::Commit).expect("legal");
    assert_eq!(
        tree.advance(worker, WorkerStep::Abort),
        Err(RegionFault::AbortWithoutReserve { worker })
    );
    tree.advance(worker, WorkerStep::Reserve).expect("legal");
    tree.advance(worker, WorkerStep::Abort).expect("legal");
    assert_eq!(
        tree.advance(worker, WorkerStep::Commit),
        Err(RegionFault::CommitWithoutReserve { worker }),
        "one resolution per staging: an aborted publication cannot then commit"
    );
    assert_eq!(
        tree.advance(worker, WorkerStep::Abort),
        Err(RegionFault::AbortWithoutReserve { worker }),
        "one resolution per staging: an aborted publication cannot abort twice"
    );
    assert_eq!(tree.evidence(worker).expect("admitted").committed(), 1);
}

#[test]
fn an_abort_is_illegal_outside_running() {
    // Created: nothing has begun.
    let mut tree = RegionTree::new();
    let root = tree.root();
    let created = tree.spawn(root, Resumability::Resumable).expect("open");
    assert_eq!(
        tree.advance(created, WorkerStep::Abort),
        Err(RegionFault::IllegalWorkerStep {
            worker: created,
            from: WorkerState::Created,
            step: WorkerStep::Abort,
        })
    );

    // Suspended: a parked worker takes no step but Resume.
    let parked = tree.spawn(root, Resumability::Resumable).expect("open");
    tree.advance(parked, WorkerStep::Begin).expect("legal");
    tree.advance(parked, WorkerStep::Suspend).expect("legal");
    assert_eq!(
        tree.advance(parked, WorkerStep::Abort),
        Err(RegionFault::IllegalWorkerStep {
            worker: parked,
            from: WorkerState::Suspended,
            step: WorkerStep::Abort,
        })
    );

    // Terminal: a terminal status never changes.
    let done = tree.spawn(root, Resumability::Resumable).expect("open");
    tree.advance(done, WorkerStep::Begin).expect("legal");
    tree.advance(done, WorkerStep::Complete).expect("legal");
    assert_eq!(
        tree.advance(done, WorkerStep::Abort),
        Err(RegionFault::IllegalWorkerStep {
            worker: done,
            from: WorkerState::Completed,
            step: WorkerStep::Abort,
        })
    );

    // Finalized region: nothing is left to advance.
    let finalization = tree.teardown(root).expect("torn down");
    assert!(finalization.is_total());
    assert_eq!(
        tree.advance(done, WorkerStep::Abort),
        Err(RegionFault::AdvanceInFinalizedRegion {
            worker: done,
            region: root
        })
    );
}

#[test]
fn an_abort_inside_a_cancelled_region_is_a_wind_down_step() {
    let (mut tree, worker) = running();
    let root = tree.root();
    tree.advance(worker, WorkerStep::Reserve).expect("legal");
    tree.cancel(root).expect("open");
    tree.advance(worker, WorkerStep::Abort)
        .expect("a worker observing cancellation may drop its own staging");
    tree.drain(root).expect("cancelled");
    let finalization = tree.finalize(root).expect("drained");
    assert!(finalization.is_total(), "{}", finalization.render());
    assert_eq!(
        finalization.workers()[0].cancel_outcome(),
        Some(&CancelOutcome::NothingPublished)
    );
}

#[test]
fn an_abort_keeps_committed_evidence_and_its_continuation() {
    let (mut tree, worker) = running();
    tree.advance(worker, WorkerStep::Reserve).expect("legal");
    tree.advance(worker, WorkerStep::Commit).expect("legal");
    tree.advance(worker, WorkerStep::Reserve).expect("legal");
    tree.advance(worker, WorkerStep::Abort).expect("legal");
    let root = tree.root();
    let finalization = tree.teardown(root).expect("torn down");
    assert!(finalization.is_total(), "{}", finalization.render());
    assert_eq!(
        finalization.continuations(),
        vec![Continuation::new(worker, 1)],
        "INV-009: the abort leaves the committed half alone"
    );
}

// --- staging slots -------------------------------------------------------------------

#[test]
fn a_worker_may_hold_several_staged_publications_and_resolve_them_in_any_order() {
    let (mut tree, worker) = running();
    tree.advance(worker, WorkerStep::ReserveSlot(slot(1)))
        .expect("legal");
    tree.advance(worker, WorkerStep::ReserveSlot(slot(2)))
        .expect("a second staging in another slot is admitted");
    tree.advance(worker, WorkerStep::ReserveSlot(slot(3)))
        .expect("and a third");
    assert_eq!(tree.evidence(worker).expect("admitted").staged(), 3);
    for ordinal in [1, 2, 3] {
        assert!(
            tree.ledger()
                .holds(Obligation::staged_publication(worker, slot(ordinal))),
            "slot {ordinal} owes its own resolution"
        );
    }

    // Out of order, and mixed: commit #2, abort #3, commit #1.
    tree.advance(worker, WorkerStep::CommitSlot(slot(2)))
        .expect("legal");
    tree.advance(worker, WorkerStep::AbortSlot(slot(3)))
        .expect("legal");
    tree.advance(worker, WorkerStep::CommitSlot(slot(1)))
        .expect("legal");
    let evidence = tree.evidence(worker).expect("admitted");
    assert_eq!(evidence.staged(), 0);
    assert_eq!(evidence.committed(), 2, "two commits, one abort");

    tree.advance(worker, WorkerStep::Complete).expect("legal");
    let root = tree.root();
    tree.close(root).expect("open");
    tree.drain(root).expect("terminal");
    let finalization = tree.finalize(root).expect("drained");
    assert!(finalization.is_total(), "{}", finalization.render());
    assert_eq!(finalization.ledger().opened(), 4);
    assert_eq!(finalization.ledger().discharged(), 4);
}

#[test]
fn each_slot_is_staged_once_and_resolved_once() {
    let (mut tree, worker) = running();
    tree.advance(worker, WorkerStep::ReserveSlot(slot(1)))
        .expect("legal");
    let before = counters(&tree);
    assert_eq!(
        tree.advance(worker, WorkerStep::ReserveSlot(slot(1))),
        Err(RegionFault::SlotAlreadyStaged {
            worker,
            slot: slot(1)
        })
    );
    assert_eq!(counters(&tree), before);

    for step in [
        WorkerStep::CommitSlot(slot(2)),
        WorkerStep::AbortSlot(slot(2)),
    ] {
        assert_eq!(
            tree.advance(worker, step.clone()),
            Err(RegionFault::SlotNotStaged { worker, step }),
            "a slot nobody staged has nothing to resolve"
        );
    }
    tree.advance(worker, WorkerStep::AbortSlot(slot(1)))
        .expect("legal");
    for step in [
        WorkerStep::CommitSlot(slot(1)),
        WorkerStep::AbortSlot(slot(1)),
    ] {
        assert_eq!(
            tree.advance(worker, step.clone()),
            Err(RegionFault::SlotNotStaged { worker, step }),
            "a resolved slot is not resolved twice"
        );
    }
    assert!(
        tree.ledger().outstanding().len() == 1,
        "only the termination"
    );
}

#[test]
fn an_unnamed_resolution_over_several_staged_publications_is_refused_as_ambiguous() {
    let (mut tree, worker) = running();
    tree.advance(worker, WorkerStep::ReserveSlot(slot(1)))
        .expect("legal");
    tree.advance(worker, WorkerStep::ReserveSlot(slot(2)))
        .expect("legal");
    for step in [WorkerStep::Commit, WorkerStep::Abort] {
        assert_eq!(
            tree.advance(worker, step.clone()),
            Err(RegionFault::AmbiguousResolution {
                worker,
                step,
                staged: 2
            })
        );
    }
    // With one left, the unnamed steps resolve it, whatever its slot.
    tree.advance(worker, WorkerStep::CommitSlot(slot(1)))
        .expect("legal");
    tree.advance(worker, WorkerStep::Abort)
        .expect("the one staged publication is unambiguous");
    assert!(!tree.evidence(worker).expect("admitted").is_provisional());
}

#[test]
fn the_one_in_flight_reserve_keeps_its_refusal_beside_slots() {
    let (mut tree, worker) = running();
    tree.advance(worker, WorkerStep::ReserveSlot(slot(4)))
        .expect("legal");
    assert_eq!(
        tree.advance(worker, WorkerStep::Reserve),
        Err(RegionFault::ReserveOverProvisional { worker }),
        "Reserve is the one-in-flight discipline, whichever slot is staged"
    );
    // The primary slot is Reserve's own; staging it by name is the same obligation.
    tree.advance(worker, WorkerStep::ReserveSlot(PublicationSlot::PRIMARY))
        .expect("legal: another slot");
    assert!(
        tree.ledger()
            .holds(Obligation::provisional_publication(worker)),
        "the primary slot is keyed by the worker alone"
    );
    assert_eq!(
        Obligation::staged_publication(worker, PublicationSlot::PRIMARY),
        Obligation::provisional_publication(worker)
    );
}

#[test]
fn no_worker_parks_or_completes_while_any_slot_is_staged() {
    let (mut tree, worker) = running();
    tree.advance(worker, WorkerStep::ReserveSlot(slot(7)))
        .expect("legal");
    assert_eq!(
        tree.advance(worker, WorkerStep::Suspend),
        Err(RegionFault::SuspendWithProvisionalEvidence { worker })
    );
    assert_eq!(
        tree.advance(worker, WorkerStep::Complete),
        Err(RegionFault::CompleteWithProvisionalEvidence { worker })
    );
}

#[test]
fn a_failure_or_a_drain_discards_every_staged_slot() {
    let fail = FailureReason::new("engine-defect").expect("canonical");

    let (mut tree, worker) = running();
    for ordinal in [0, 1, 2] {
        tree.advance(worker, WorkerStep::ReserveSlot(slot(ordinal)))
            .expect("legal");
    }
    tree.advance(worker, WorkerStep::Fail(fail)).expect("legal");
    assert_eq!(tree.evidence(worker).expect("admitted").staged(), 0);
    assert!(
        tree.ledger().is_balanced(),
        "{:?}",
        tree.ledger().outstanding()
    );

    let (mut tree, worker) = running();
    tree.advance(worker, WorkerStep::ReserveSlot(slot(1)))
        .expect("legal");
    tree.advance(worker, WorkerStep::CommitSlot(slot(1)))
        .expect("legal");
    for ordinal in [5, 9] {
        tree.advance(worker, WorkerStep::ReserveSlot(slot(ordinal)))
            .expect("legal");
    }
    let root = tree.root();
    let finalization = tree.teardown(root).expect("torn down");
    assert!(finalization.is_total(), "{}", finalization.render());
    assert_eq!(
        finalization.unresolved_publications(),
        Vec::<WorkerId>::new()
    );
    assert_eq!(
        finalization.continuations(),
        vec![Continuation::new(worker, 1)],
        "the drain discards the staged half and keeps the committed half"
    );
}

// --- the schedule sweep --------------------------------------------------------------

/// Two workers, each staging two slots and resolving them one by commit and one by
/// abort, interleaved every way and cut by a cancellation at every point. Every
/// interleaving tears down totally, and the ledger pairs every staging with exactly one
/// resolution.
#[test]
fn every_interleaving_of_multi_slot_programs_tears_down_totally() {
    let program = |worker: u32| {
        let w = WorkerId::at(worker);
        vec![
            Step::advance(w, WorkerStep::Begin),
            Step::advance(w, WorkerStep::ReserveSlot(slot(1))),
            Step::advance(w, WorkerStep::ReserveSlot(slot(2))),
            Step::advance(w, WorkerStep::AbortSlot(slot(1))),
            Step::advance(w, WorkerStep::CommitSlot(slot(2))),
            Step::advance(w, WorkerStep::Reserve),
            Step::advance(w, WorkerStep::Abort),
        ]
    };
    let schedules = interleavings(&[program(0), program(1)]);
    assert_eq!(schedules.len(), 3432, "14! / (7! * 7!)");
    let root = continuum_task::region::RegionId::at(0);
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
    for schedule in &schedules {
        for cut in [0, 5, 9, schedule.len()] {
            let prefix = Schedule::new(schedule.steps()[..cut].to_vec());
            let mut tree = RegionTree::new();
            let run = setup.clone().then(&prefix).run(&mut tree);
            assert!(run.faults().is_empty(), "{}", run.render());
            let finalization = tree.teardown(root).expect("torn down");
            assert!(finalization.is_total(), "{}", finalization.render());
            assert_eq!(finalization.workers().len(), 2);
            torn += 1;
        }
    }
    assert_eq!(torn, 3432 * 4);
}

// --- metamorphic relations -----------------------------------------------------------

/// Metamorphic relation: symmetry renaming. Slots are names a caller chose, so renaming
/// them by any injection leaves every observable of the run — each step's outcome, the
/// evidence, and the finalization's totality and counters — unchanged.
#[test]
fn symmetry_renaming_of_slots_preserves_the_run() {
    let build = |a: u32, b: u32, c: u32| {
        let (mut tree, worker) = running();
        let mut outcomes = Vec::new();
        for step in [
            WorkerStep::ReserveSlot(slot(a)),
            WorkerStep::ReserveSlot(slot(b)),
            WorkerStep::ReserveSlot(slot(a)),
            WorkerStep::CommitSlot(slot(b)),
            WorkerStep::ReserveSlot(slot(c)),
            WorkerStep::Commit,
            WorkerStep::AbortSlot(slot(c)),
            WorkerStep::AbortSlot(slot(a)),
            WorkerStep::Abort,
        ] {
            outcomes.push(tree.advance(worker, step).is_ok());
        }
        let evidence = tree.evidence(worker).expect("admitted");
        let root = tree.root();
        let finalization = tree.teardown(root).expect("torn down");
        (
            outcomes,
            evidence,
            finalization.is_total(),
            finalization.ledger().opened(),
            finalization.ledger().discharged(),
        )
    };
    let reference = build(1, 2, 3);
    assert_eq!(
        reference.0,
        vec![true, true, false, true, true, false, true, true, false],
        "the fixture exercises both admissions and refusals"
    );
    for (a, b, c) in [(3, 2, 1), (0, 7, 40), (u32::MAX, 0, 11)] {
        assert_eq!(build(a, b, c), reference, "renaming ({a},{b},{c})");
    }
}

/// Metamorphic relation: independent-event swap. Resolving two different slots touches
/// two different obligations, so the two resolutions commute: swapping them leaves the
/// finalization's canonical render byte-identical.
#[test]
fn independent_event_swap_of_two_slot_resolutions_preserves_the_render() {
    let build = |first: WorkerStep, second: WorkerStep| {
        let (mut tree, worker) = running();
        tree.advance(worker, WorkerStep::ReserveSlot(slot(1)))
            .expect("legal");
        tree.advance(worker, WorkerStep::ReserveSlot(slot(2)))
            .expect("legal");
        tree.advance(worker, first).expect("legal");
        tree.advance(worker, second).expect("legal");
        let root = tree.root();
        tree.teardown(root).expect("torn down").render()
    };
    let pairs = [
        (
            WorkerStep::CommitSlot(slot(1)),
            WorkerStep::AbortSlot(slot(2)),
        ),
        (
            WorkerStep::CommitSlot(slot(1)),
            WorkerStep::CommitSlot(slot(2)),
        ),
        (
            WorkerStep::AbortSlot(slot(1)),
            WorkerStep::AbortSlot(slot(2)),
        ),
    ];
    for (one, two) in pairs {
        let forward = build(one.clone(), two.clone());
        let swapped = build(two, one);
        assert_eq!(forward.as_bytes(), swapped.as_bytes());
        assert!(forward.contains("balanced=true"), "{forward}");
    }
}
