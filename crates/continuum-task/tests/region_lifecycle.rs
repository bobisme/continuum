//! Lifecycle legality for `PR-6-IMPL-01` ("task lifecycle") and `PR-6-IMPL-05`
//! ("drain/finalize behavior") — `notes/plan/notes/PLAN_REQUIREMENTS.json`;
//! `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 6.
//!
//! > Use asupersync regions for daemon work: task lifecycle; … drain/finalize behavior;
//! > no orphan workers.
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 6
//!
//! > `TaskStatus` (`created`, `running`, `suspended`, `completed`, `failed`,
//! > `cancelled`) is the task-lifecycle vocabulary. … **`task.status` is monotonic.**
//! > Reported milestones and `committed_evidence` only grow, and a terminal status never
//! > changes.
//! >
//! > — `notes/plan/rfcs/0026-continuumd-native-protocol.md`, "Task lifecycle"
//!
//! # Why this file exists
//!
//! The unit tests in [`continuum_task::region`] were written by the same hand as the
//! implementation and check that the legal paths work. This file checks the complement:
//! that **every illegal transition is a distinct typed refusal**, that the refusal names
//! its subject and the state that subject was actually in, and that a refused call
//! changes nothing. INV-008's typed-inconclusiveness discipline is stated for verdicts,
//! but its reason is the same one here — "timeout, unsupported semantics, insufficient
//! telemetry, abstraction ambiguity, and incomplete proof search are distinct outcomes" —
//! and a lifecycle that collapses "you cannot spawn there" into "you cannot suspend that"
//! is a lifecycle nobody can debug.
//!
//! # Evidence map
//!
//! | Claim | Test |
//! |---|---|
//! | the worker transition table is total, and its off-table cells are typed | [`the_worker_transition_table_refuses_every_illegal_cell_by_name`] |
//! | a terminal status never changes (RFC 0026 monotonicity) | [`no_step_moves_a_worker_out_of_a_terminal_state`] |
//! | work enters only through an open region | [`every_region_state_that_is_not_open_refuses_new_work`] |
//! | the region state machine is one-way | [`the_region_lifecycle_admits_no_path_backwards`] |
//! | drain is between the request and the finalize | [`drain_and_finalize_refuse_out_of_order_and_name_the_state`] |
//! | a normal close blocks on its own outstanding work, naming it | [`a_closed_region_names_each_worker_the_drain_is_still_waiting_on`] |
//! | publication is atomic with respect to the lifecycle | [`a_worker_may_not_park_or_complete_while_a_publication_is_staged`] |
//! | a refusal changes nothing | [`every_refused_call_leaves_the_tree_byte_identical`] |
//!
//! # House rules
//!
//! - **Every assertion is on a typed value, never on a message.** `Display` is checked
//!   in exactly one test and only for the shape a human reads; the rest compare
//!   [`RegionFault`] variants.
//! - **A refusal is checked for its effect as well as its type.** A typed error that
//!   corrupted the tree on the way out would pass a variant assertion and fail the
//!   property, so [`every_refused_call_leaves_the_tree_byte_identical`] renders the tree
//!   before and after.
//! - **No test asserts an interleaving.** Schedules here are fixtures, not claims.

use continuum_task::region::obligation::{Obligation, ObligationKind};
use continuum_task::region::worker::{
    FailureReason, NonResumableReason, Resumability, WorkerId, WorkerState, WorkerStep,
};
use continuum_task::region::{DrainCause, RegionFault, RegionState, RegionTree};

// --- fixtures ------------------------------------------------------------------------

/// A canonical reason token, for the tests that need a `Failed` worker.
fn failure(token: &str) -> FailureReason {
    FailureReason::new(token).expect("test reason tokens are canonical")
}

/// The step that fails a worker with `token`.
fn fail(token: &str) -> WorkerStep {
    WorkerStep::Fail(failure(token))
}

/// Resumable work — the ordinary declaration.
fn resumable() -> Resumability {
    Resumability::Resumable
}

/// Work declared non-resumable, with the typed reason RFC 0026 requires.
fn non_resumable(token: &str) -> Resumability {
    Resumability::NonResumable(NonResumableReason::new(token).expect("canonical token"))
}

/// Every step, so a transition-table test cannot quietly omit one.
fn every_step() -> Vec<WorkerStep> {
    vec![
        WorkerStep::Begin,
        WorkerStep::Reserve,
        WorkerStep::Commit,
        WorkerStep::Suspend,
        WorkerStep::Resume,
        WorkerStep::Complete,
        fail("engine-defect"),
    ]
}

/// A tree whose single root worker has been driven to `state`, with `committed`
/// publications behind it and, when `staged`, one more in flight.
fn worker_in(state: &WorkerState, committed: u32, staged: bool) -> (RegionTree, WorkerId) {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let worker = tree.spawn(root, resumable()).expect("the root is open");
    if !matches!(state, WorkerState::Created) {
        tree.advance(worker, WorkerStep::Begin).expect("legal");
    }
    for _ in 0..committed {
        tree.advance(worker, WorkerStep::Reserve).expect("legal");
        tree.advance(worker, WorkerStep::Commit).expect("legal");
    }
    if staged {
        tree.advance(worker, WorkerStep::Reserve).expect("legal");
    }
    match state {
        WorkerState::Created | WorkerState::Running => {}
        WorkerState::Suspended => {
            tree.advance(worker, WorkerStep::Suspend).expect("legal");
        }
        WorkerState::Completed => {
            tree.advance(worker, WorkerStep::Complete).expect("legal");
        }
        WorkerState::Failed(reason) => {
            tree.advance(worker, WorkerStep::Fail(reason.clone()))
                .expect("legal");
        }
        WorkerState::Cancelled => {
            tree.cancel(root).expect("not finalized");
            tree.drain(root).expect("cancellation drains");
        }
    }
    (tree, worker)
}

/// A canonical rendering of everything the tree holds, for before/after comparison.
///
/// Deliberately assembled from the public observers rather than from a debug format:
/// what must be unchanged by a refusal is the tree's *meaning*, and reading it through
/// the same surface a caller reads it through is what makes the check meaningful.
fn render(tree: &RegionTree) -> String {
    let mut out = String::new();
    for ordinal in 0..u32::try_from(tree.region_count()).expect("small trees") {
        let region = continuum_task::region::RegionId::at(ordinal);
        let state = tree.state(region).expect("enumerated region exists");
        out.push_str(&format!("{region} {state}\n"));
    }
    for ordinal in 0..u32::try_from(tree.worker_count()).expect("small trees") {
        let worker = WorkerId::at(ordinal);
        let state = tree.worker_state(worker).expect("enumerated worker exists");
        let evidence = tree.evidence(worker).expect("enumerated worker exists");
        let owner = tree.owner(worker).expect("enumerated worker exists");
        out.push_str(&format!(
            "{worker} owner={owner} state={state} committed={} provisional={}\n",
            evidence.committed(),
            evidence.is_provisional()
        ));
    }
    out.push_str(&format!(
        "ledger opened={} discharged={} outstanding={:?}\n",
        tree.ledger().opened(),
        tree.ledger().discharged(),
        tree.ledger()
            .outstanding()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    ));
    out
}

// --- the worker transition table -----------------------------------------------------

#[test]
fn the_worker_transition_table_refuses_every_illegal_cell_by_name() {
    // The legal cells, as data. Anything not named here must be refused, and the whole
    // point of writing it this way is that adding a step to `WorkerStep` without adding
    // a row makes this test fail rather than silently skip it.
    let legal: &[(&str, &str)] = &[
        ("created", "begin"),
        ("created", "fail"),
        ("running", "reserve"),
        ("running", "commit"),
        ("running", "suspend"),
        ("running", "complete"),
        ("running", "fail"),
        ("suspended", "resume"),
    ];

    let states = [
        (WorkerState::Created, 0_u32, false),
        (WorkerState::Running, 0, false),
        // `commit` is legal from `running` only with something staged; the staged
        // fixture is what makes that cell reachable rather than a false negative.
        (WorkerState::Running, 0, true),
        (WorkerState::Suspended, 1, false),
        (WorkerState::Completed, 0, false),
        (WorkerState::Failed(failure("engine-defect")), 0, false),
        (WorkerState::Cancelled, 0, false),
    ];

    let mut refused = 0_usize;
    let mut allowed = 0_usize;
    for (state, committed, staged) in &states {
        for step in every_step() {
            let (mut tree, worker) = worker_in(state, *committed, *staged);
            let cell = (state.status_token(), step.token());
            let outcome = tree.advance(worker, step.clone());
            if legal.contains(&cell) {
                // A legal cell may still be refused by an evidence pre-condition — that
                // is what the `staged` dimension exists to separate — but it may never be
                // refused as an illegal *step*.
                if let Err(fault) = &outcome {
                    assert!(
                        !matches!(fault, RegionFault::IllegalWorkerStep { .. }),
                        "{cell:?} is a legal cell but was refused as an illegal step: {fault}"
                    );
                }
                allowed += 1;
            } else {
                let fault = outcome.expect_err(&format!("{cell:?} must be refused"));
                assert!(
                    matches!(
                        fault,
                        RegionFault::IllegalWorkerStep {
                            worker: reported,
                            ref from,
                            ..
                        } if reported == worker && from.status_token() == state.status_token()
                    ),
                    "{cell:?} was refused, but not as a typed illegal step naming its state: {fault}"
                );
                refused += 1;
            }
        }
    }
    assert_eq!(
        allowed + refused,
        states.len() * every_step().len(),
        "every cell of the table must have been visited"
    );
    assert!(refused > 0 && allowed > 0, "a vacuous table proves nothing");
}

#[test]
fn no_step_moves_a_worker_out_of_a_terminal_state() {
    let terminal = [
        WorkerState::Completed,
        WorkerState::Failed(failure("budget-exhausted")),
        WorkerState::Cancelled,
    ];
    for state in &terminal {
        for step in every_step() {
            let (mut tree, worker) = worker_in(state, 0, false);
            let before = tree
                .worker_state(worker)
                .expect("the fixture spawned it")
                .clone();
            assert_eq!(&before, state, "the fixture did not reach {state}");
            let fault = tree
                .advance(worker, step.clone())
                .expect_err("a terminal status never changes");
            assert!(matches!(fault, RegionFault::IllegalWorkerStep { .. }));
            assert_eq!(
                tree.worker_state(worker).expect("still known"),
                state,
                "the refusal moved a terminal worker"
            );
        }
    }
}

#[test]
fn a_worker_may_not_park_or_complete_while_a_publication_is_staged() {
    let (mut tree, worker) = worker_in(&WorkerState::Running, 1, true);
    assert_eq!(
        tree.advance(worker, WorkerStep::Suspend),
        Err(RegionFault::SuspendWithProvisionalEvidence { worker })
    );
    assert_eq!(
        tree.advance(worker, WorkerStep::Complete),
        Err(RegionFault::CompleteWithProvisionalEvidence { worker })
    );
    // Resolving the publication unblocks both, which is what makes the refusal a
    // pre-condition rather than a prohibition.
    tree.advance(worker, WorkerStep::Commit).expect("legal");
    tree.advance(worker, WorkerStep::Suspend).expect("legal");
}

#[test]
fn the_publication_pair_is_refused_in_both_wrong_orders() {
    let (mut tree, worker) = worker_in(&WorkerState::Running, 0, false);
    assert_eq!(
        tree.advance(worker, WorkerStep::Commit),
        Err(RegionFault::CommitWithoutReserve { worker }),
        "there is nothing staged to commit"
    );
    tree.advance(worker, WorkerStep::Reserve).expect("legal");
    assert_eq!(
        tree.advance(worker, WorkerStep::Reserve),
        Err(RegionFault::ReserveOverProvisional { worker }),
        "at most one publication is in flight, as the store's typestate has it"
    );
    assert!(
        tree.ledger()
            .holds(Obligation::provisional_publication(worker)),
        "the staged publication is owed a resolution the whole time"
    );
}

#[test]
fn work_declared_non_resumable_cannot_park() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let worker = tree
        .spawn(root, non_resumable("no-continuation-support"))
        .expect("the root is open");
    tree.advance(worker, WorkerStep::Begin).expect("legal");
    assert_eq!(
        tree.advance(worker, WorkerStep::Suspend),
        Err(RegionFault::SuspendNonResumableWorker { worker }),
        "RFC 0026: a suspended task is resumable by definition"
    );
}

// --- the region state machine --------------------------------------------------------

#[test]
fn every_region_state_that_is_not_open_refuses_new_work() {
    let closed = |request: fn(&mut RegionTree)| {
        let mut tree = RegionTree::new();
        request(&mut tree);
        tree
    };
    let cases: Vec<(RegionState, RegionTree)> = vec![
        (
            RegionState::Draining(DrainCause::Closed),
            closed(|tree| {
                let root = tree.root();
                tree.close(root).expect("the root is open");
            }),
        ),
        (
            RegionState::Draining(DrainCause::Cancelled),
            closed(|tree| {
                let root = tree.root();
                tree.cancel(root).expect("the root is not finalized");
            }),
        ),
        (
            RegionState::Finalized,
            closed(|tree| {
                let root = tree.root();
                tree.teardown(root).expect("the root exists");
            }),
        ),
    ];
    for (expected, mut tree) in cases {
        let root = tree.root();
        assert_eq!(tree.state(root), Ok(expected));
        assert_eq!(
            tree.spawn(root, resumable()),
            Err(RegionFault::SpawnIntoClosedRegion {
                region: root,
                state: expected
            })
        );
        assert_eq!(
            tree.open_child(root),
            Err(RegionFault::OpenChildInClosedRegion {
                region: root,
                state: expected
            })
        );
        assert_eq!(
            tree.worker_count(),
            0,
            "a refused spawn must admit no worker"
        );
        assert_eq!(
            tree.region_count(),
            1,
            "a refused open_child must open no region"
        );
    }
}

#[test]
fn the_region_lifecycle_admits_no_path_backwards() {
    let mut tree = RegionTree::new();
    let root = tree.root();

    tree.close(root).expect("open → draining(closed)");
    assert_eq!(
        tree.close(root),
        Err(RegionFault::CloseNonOpenRegion {
            region: root,
            state: RegionState::Draining(DrainCause::Closed)
        }),
        "a second request against a region that already has one"
    );

    tree.cancel(root).expect("closed → cancelled is an upgrade");
    assert_eq!(
        tree.state(root),
        Ok(RegionState::Draining(DrainCause::Cancelled))
    );
    tree.cancel(root).expect("requesting twice changes nothing");

    tree.drain(root).expect("cancellation drains");
    tree.finalize(root).expect("drained");
    assert_eq!(
        tree.state(root),
        Ok(RegionState::Finalized),
        "and there it stays"
    );
    assert_eq!(
        tree.cancel(root),
        Err(RegionFault::CancelFinalizedRegion { region: root })
    );
    assert_eq!(
        tree.close(root),
        Err(RegionFault::CloseNonOpenRegion {
            region: root,
            state: RegionState::Finalized
        })
    );
    assert_eq!(
        tree.drain(root),
        Err(RegionFault::DrainFinalizedRegion { region: root })
    );
    assert_eq!(
        tree.finalize(root),
        Err(RegionFault::AlreadyFinalized { region: root })
    );
}

#[test]
fn drain_and_finalize_refuse_out_of_order_and_name_the_state() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    assert_eq!(
        tree.drain(root),
        Err(RegionFault::DrainBeforeRequest { region: root }),
        "an open region has had no request to drain"
    );
    assert_eq!(
        tree.finalize(root),
        Err(RegionFault::FinalizeBeforeDrain {
            region: root,
            state: RegionState::Open
        }),
        "finalize follows drain, which follows a request"
    );
    assert_eq!(tree.state(root), Ok(RegionState::Open), "nothing moved");
}

#[test]
fn a_closed_region_names_each_worker_the_drain_is_still_waiting_on() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let child = tree.open_child(root).expect("the root is open");
    let first = tree.spawn(root, resumable()).expect("open");
    let second = tree.spawn(child, resumable()).expect("open");
    tree.advance(first, WorkerStep::Begin).expect("legal");
    tree.advance(second, WorkerStep::Begin).expect("legal");
    tree.close(root).expect("the root is open");

    assert_eq!(
        tree.drain(root),
        Err(RegionFault::DrainBlocked {
            region: root,
            worker: first,
            state: WorkerState::Running
        }),
        "the drain names the lowest outstanding worker first"
    );
    tree.advance(first, WorkerStep::Complete).expect("legal");
    assert_eq!(
        tree.drain(root),
        Err(RegionFault::DrainBlocked {
            region: child,
            worker: second,
            state: WorkerState::Running
        }),
        "and then the next one, in the child region"
    );
    assert_eq!(
        tree.outstanding_workers(root),
        Ok(vec![second]),
        "the observer and the refusal agree on who is left"
    );
    tree.advance(second, WorkerStep::Complete).expect("legal");
    assert_eq!(tree.outstanding_workers(root), Ok(Vec::new()));
    tree.drain(root).expect("nothing is outstanding now");
    let finalization = tree.finalize(root).expect("drained");
    assert!(finalization.is_total(), "{}", finalization.render());
}

#[test]
fn a_parent_cannot_finalize_while_it_owes_a_child_region_quiescence() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let child = tree.open_child(root).expect("the root is open");
    assert!(
        tree.ledger()
            .holds(Obligation::child_region_quiescence(child)),
        "RFC 0001 names child-region quiescence as an obligation; it must be open"
    );
    let worker = tree.spawn(child, resumable()).expect("open");
    tree.advance(worker, WorkerStep::Begin).expect("legal");
    tree.close(root).expect("the root is open");
    assert_eq!(
        tree.drain(root),
        Err(RegionFault::DrainBlocked {
            region: child,
            worker,
            state: WorkerState::Running
        }),
        "a parent's drain reaches into the child it owns"
    );
    assert!(
        !tree.ledger().is_balanced(),
        "the debt is still on the books"
    );
    tree.advance(worker, WorkerStep::Complete).expect("legal");
    tree.drain(root).expect("clear now");
    let finalization = tree.finalize(root).expect("drained");
    assert!(finalization.ledger().is_balanced());
    assert_eq!(
        finalization.regions(),
        &[root, child],
        "the report covers the whole subtree, parent-before-child in identity order"
    );
}

#[test]
fn cancelling_a_child_leaves_its_parent_open() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let child = tree.open_child(root).expect("the root is open");
    let sibling = tree.open_child(root).expect("the root is open");
    tree.cancel(child).expect("not finalized");
    assert_eq!(tree.state(root), Ok(RegionState::Open));
    assert_eq!(tree.state(sibling), Ok(RegionState::Open));
    assert_eq!(
        tree.state(child),
        Ok(RegionState::Draining(DrainCause::Cancelled))
    );
    // The root is still open, so new work may still enter it and its other child.
    tree.spawn(root, resumable()).expect("the root is open");
    tree.spawn(sibling, resumable())
        .expect("the sibling is open");
    assert_eq!(tree.worker_count(), 2);
}

#[test]
fn an_unknown_region_or_worker_is_named_rather_than_panicked_over() {
    let mut tree = RegionTree::new();
    let ghost_region = continuum_task::region::RegionId::at(7);
    let ghost_worker = WorkerId::at(9);
    assert_eq!(
        tree.state(ghost_region),
        Err(RegionFault::UnknownRegion(ghost_region))
    );
    assert_eq!(
        tree.spawn(ghost_region, resumable()),
        Err(RegionFault::UnknownRegion(ghost_region))
    );
    assert_eq!(
        tree.advance(ghost_worker, WorkerStep::Begin),
        Err(RegionFault::UnknownWorker(ghost_worker))
    );
    assert_eq!(
        tree.evidence(ghost_worker),
        Err(RegionFault::UnknownWorker(ghost_worker))
    );
}

// --- refusals are inert --------------------------------------------------------------

#[test]
fn every_refused_call_leaves_the_tree_byte_identical() {
    // A tree with something of every kind in it, so a refusal has something to damage.
    let mut tree = RegionTree::new();
    let root = tree.root();
    let child = tree.open_child(root).expect("open");
    let running = tree.spawn(root, resumable()).expect("open");
    let staged = tree.spawn(child, resumable()).expect("open");
    let done = tree.spawn(child, resumable()).expect("open");
    tree.advance(running, WorkerStep::Begin).expect("legal");
    tree.advance(staged, WorkerStep::Begin).expect("legal");
    tree.advance(staged, WorkerStep::Reserve).expect("legal");
    tree.advance(done, WorkerStep::Begin).expect("legal");
    tree.advance(done, WorkerStep::Complete).expect("legal");

    let before = render(&tree);
    let ghost = continuum_task::region::RegionId::at(42);

    // Each of these must be refused, and none of them may change anything.
    assert!(tree.advance(done, WorkerStep::Begin).is_err());
    assert!(tree.advance(staged, WorkerStep::Suspend).is_err());
    assert!(tree.advance(staged, WorkerStep::Reserve).is_err());
    assert!(tree.advance(running, WorkerStep::Commit).is_err());
    assert!(tree.advance(running, WorkerStep::Resume).is_err());
    assert!(tree.spawn(ghost, resumable()).is_err());
    assert!(tree.open_child(ghost).is_err());
    assert!(tree.drain(root).is_err());
    assert!(tree.finalize(root).is_err());
    assert!(tree.advance(WorkerId::at(99), WorkerStep::Begin).is_err());

    let after = render(&tree);
    assert_eq!(
        before.as_bytes(),
        after.as_bytes(),
        "a typed refusal that changed the tree is not a refusal"
    );
    assert!(!before.is_empty(), "a vacuous comparison proves nothing");
}

#[test]
fn a_refusal_reads_as_a_sentence_naming_its_subject_and_its_state() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let worker = tree.spawn(root, resumable()).expect("open");
    let fault = tree
        .advance(worker, WorkerStep::Commit)
        .expect_err("a created worker cannot commit");
    assert_eq!(
        fault.to_string(),
        "w0 is created, so the step commit is not legal"
    );

    tree.close(root).expect("open");
    let fault = tree
        .spawn(root, resumable())
        .expect_err("a closed region admits nothing");
    assert_eq!(
        fault.to_string(),
        "cannot spawn into r0: it is draining(closed), not open"
    );
}

#[test]
fn the_four_obligation_kinds_all_appear_and_all_clear() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let child = tree.open_child(root).expect("open");
    let worker = tree.spawn(child, resumable()).expect("open");
    tree.advance(worker, WorkerStep::Begin).expect("legal");
    tree.advance(worker, WorkerStep::Reserve).expect("legal");
    tree.cancel(root).expect("not finalized");

    let outstanding: Vec<ObligationKind> = tree
        .ledger()
        .outstanding()
        .iter()
        .map(Obligation::kind)
        .collect();
    assert!(outstanding.contains(&ObligationKind::WorkerTermination));
    assert!(outstanding.contains(&ObligationKind::ChildRegionQuiescence));
    assert!(outstanding.contains(&ObligationKind::CancellationFinalization));
    assert!(outstanding.contains(&ObligationKind::ProvisionalPublication));

    tree.drain(root).expect("cancellation drains");
    let finalization = tree.finalize(root).expect("drained");
    assert!(
        finalization.ledger().outstanding().is_empty(),
        "all four kinds must clear: {}",
        finalization.render()
    );
    assert_eq!(
        finalization.ledger().opened(),
        finalization.ledger().discharged()
    );
    assert!(
        finalization.ledger().opened() >= 4,
        "a vacuous ledger proves nothing"
    );
}
