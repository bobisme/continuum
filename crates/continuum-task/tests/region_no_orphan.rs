//! The instrumented no-orphan proof for `PR-6-IMPL-06` ("no orphan workers"), over an
//! exhaustively enumerated schedule space.
//!
//! > Region teardown provably leaves no orphan workers (instrumented test).
//! >
//! > — bone `bn-2gk`, acceptance criterion
//!
//! > Search, proof, synthesis, indexing, and debugging tasks run under asupersync
//! > regions and publish only committed artifacts. Cancellation cannot leave false
//! > finality or orphan work.
//! >
//! > — `notes/plan/plan.md` §3
//!
//! # What "instrumented" means here
//!
//! Three independent accountings have to agree, and each is computed from a different
//! part of the tree so that a bug fooling one still has two to get past:
//!
//! 1. **the worker states** — every worker the tree ever admitted is terminal
//!    ([`WorkerState::is_terminal`]), read back from the tree itself rather than from the
//!    report the teardown handed out;
//! 2. **the evidence ledgers** — no worker still holds a staged publication, so every
//!    artifact is committed or absent;
//! 3. **the obligation ledger** — the outstanding set is empty *and* the opened and
//!    discharged counters agree (RFC 0001's obligation conservation, at the region
//!    layer).
//!
//! [`Finalization::is_total`] is the conjunction. Every test below asserts it *and*
//! re-derives conjunct 1 from the tree, because a report that agreed with itself would
//! prove only that.
//!
//! # Why the schedule space is enumerated rather than sampled
//!
//! The G0-DX-13 campaign had to *force* an interleaving, because its subject is a
//! genuinely thread-safe store; its house rules — assertions on outcomes and never on
//! interleavings, determinism as a repeated-round invariant, and "a test that can fail
//! because a machine was loaded is not evidence" — are inherited here unchanged. What is
//! not inherited is the thread: a [`RegionTree`] has none, so an interleaving is a value.
//! [`interleavings`] therefore enumerates the whole shuffle product of a set of
//! per-worker programs, and the sweep below runs the teardown against **every** member of
//! it, with a cancellation injected at **every** position. That is a stronger statement
//! than any number of forced rendezvous: not "the property held when we managed to
//! interleave it this way", but "the property held at every interleaving there is".
//!
//! # Evidence map
//!
//! | Claim | Test |
//! |---|---|
//! | teardown is total at every interleaving | [`teardown_is_total_at_every_interleaving_of_three_workers`] |
//! | teardown is total with a cancellation *request* at every position | [`teardown_is_total_with_a_cancellation_request_injected_at_every_position`] |
//! | teardown is total when the drain *completes* at every position, and the space is genuinely varied | [`teardown_is_total_when_the_drain_completes_at_every_position`] |
//! | the property holds across a region tree, not just one region | [`teardown_is_total_across_a_three_level_region_tree`] |
//! | every worker ever admitted is accounted for, by count | [`every_worker_the_tree_ever_admitted_appears_in_the_report`] |
//! | the gate is load-bearing, not vacuous | [`finalize_refuses_when_the_drain_is_skipped`] |
//! | one schedule renders byte-identically on every run | [`two_runs_of_one_schedule_render_byte_identically`] |
//! | a refused step renders identically too | [`the_trace_of_a_refused_step_is_the_same_on_every_run`] |
//!
//! # House rules
//!
//! - **Assertions are on outcomes, never on interleavings.** No test asserts that a
//!   particular schedule ran; they assert what must hold after every schedule.
//! - **Every sweep carries an anti-vacuity guard.** A loop that ran zero times, or a
//!   render that came out empty, fails the test rather than passing it.
//! - **The report is never the only witness.** The tree is re-read after every teardown.

use continuum_task::region::schedule::{Schedule, Step, interleavings};
use continuum_task::region::worker::{
    FailureReason, NonResumableReason, Resumability, WorkerId, WorkerState, WorkerStep,
};
use continuum_task::region::{Finalization, RegionId, RegionTree};

fn failure(token: &str) -> WorkerStep {
    WorkerStep::Fail(FailureReason::new(token).expect("test tokens are canonical"))
}

fn non_resumable() -> Resumability {
    Resumability::NonResumable(NonResumableReason::new("no-continuation").expect("canonical"))
}

/// The three worker programs the sweep interleaves.
///
/// Chosen so that the space contains every shape the cancellation table cares about: one
/// worker that publishes and completes, one that stages and never resolves it on its own,
/// and one that parks with committed evidence behind it. A cancellation arriving anywhere
/// in the shuffle therefore lands on at least one worker in each of those shapes.
fn programs() -> Vec<Vec<Step>> {
    vec![
        vec![
            Step::advance(WorkerId::at(0), WorkerStep::Begin),
            Step::advance(WorkerId::at(0), WorkerStep::Reserve),
            Step::advance(WorkerId::at(0), WorkerStep::Commit),
        ],
        vec![
            Step::advance(WorkerId::at(1), WorkerStep::Begin),
            Step::advance(WorkerId::at(1), WorkerStep::Reserve),
        ],
        vec![
            Step::advance(WorkerId::at(2), WorkerStep::Begin),
            Step::advance(WorkerId::at(2), WorkerStep::Suspend),
        ],
    ]
}

/// A tree with a child region and the three workers the programs above drive.
///
/// `w0` and `w1` sit in the root, `w2` in the child, so every sweep exercises a
/// cancellation that has to cross a region boundary rather than only a worker list.
fn fixture() -> (RegionTree, RegionId) {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let child = tree.open_child(root).expect("the root is open");
    tree.spawn(root, Resumability::Resumable).expect("open");
    tree.spawn(root, non_resumable()).expect("open");
    tree.spawn(child, Resumability::Resumable).expect("open");
    (tree, root)
}

/// Assert the three independent accountings, and re-derive the first from the tree.
///
/// `context` names the schedule so a failure says which one, and the finalization's own
/// rendering is attached because a totality failure is unreadable without it.
fn assert_no_orphans(tree: &RegionTree, finalization: &Finalization, context: &str) {
    assert!(
        finalization.is_total(),
        "{context}: teardown was not total\n{}",
        finalization.render()
    );
    assert!(
        finalization.orphans().is_empty(),
        "{context}: orphans {:?}",
        finalization.orphans()
    );
    assert!(
        finalization.unresolved_publications().is_empty(),
        "{context}: unresolved publications {:?}",
        finalization.unresolved_publications()
    );
    assert!(
        finalization.ledger().is_balanced(),
        "{context}: ledger {}",
        finalization.ledger()
    );

    // Conjunct 1 again, from the tree rather than from the report.
    let admitted = u32::try_from(tree.worker_count()).expect("small trees");
    assert!(
        admitted > 0,
        "{context}: a tree with no workers proves nothing"
    );
    for ordinal in 0..admitted {
        let worker = WorkerId::at(ordinal);
        let state = tree.worker_state(worker).expect("admitted workers exist");
        assert!(
            state.is_terminal(),
            "{context}: {worker} is {state}, which is not terminal"
        );
        assert!(
            !tree
                .evidence(worker)
                .expect("admitted workers exist")
                .is_provisional(),
            "{context}: {worker} still holds a staged publication"
        );
    }
    assert_eq!(
        finalization.workers().len(),
        tree.worker_count(),
        "{context}: the report and the tree disagree on how many workers there were"
    );
}

// --- the sweeps ----------------------------------------------------------------------

#[test]
fn teardown_is_total_at_every_interleaving_of_three_workers() {
    let schedules = interleavings(&programs());
    assert_eq!(
        schedules.len(),
        210,
        "the shuffle product of 3+2+2 steps: 7! / (3! * 2! * 2!)"
    );
    for (index, schedule) in schedules.iter().enumerate() {
        let (mut tree, root) = fixture();
        let _ = schedule.run(&mut tree);
        let finalization = tree.teardown(root).expect("the root exists");
        assert_no_orphans(&tree, &finalization, &format!("schedule {index}"));
    }
    assert!(!schedules.is_empty(), "a vacuous sweep proves nothing");
}

#[test]
fn teardown_is_total_with_a_cancellation_request_injected_at_every_position() {
    // The *request* window: cancellation is asked for, and the workers keep stepping
    // until a drain reaches them. Every step after the injection is still legal, which is
    // what makes this the window an adversarial schedule lives in.
    let schedules = interleavings(&programs());
    let mut runs = 0_usize;
    for (index, schedule) in schedules.iter().enumerate() {
        for position in 0..=schedule.len() {
            let mut steps: Vec<Step> = schedule.steps().to_vec();
            steps.insert(
                position,
                Step::Cancel {
                    region: RegionId::at(0),
                },
            );

            let (mut tree, root) = fixture();
            let _ = Schedule::new(steps).run(&mut tree);
            let finalization = tree.teardown(root).expect("cancelling twice is legal");
            assert_no_orphans(
                &tree,
                &finalization,
                &format!("schedule {index}, cancel at {position}"),
            );
            runs += 1;
        }
    }
    assert_eq!(
        runs,
        schedules.len() * 8,
        "every position of every schedule must have been swept"
    );
    assert!(runs > 0, "a vacuous sweep proves nothing");
}

#[test]
fn teardown_is_total_when_the_drain_completes_at_every_position() {
    // The harder sweep: the cancellation *finishes* mid-schedule, so every step after the
    // injection lands on a worker that is already terminal and is refused. Which steps got
    // in first is now what distinguishes one interleaving from another, so this sweep is
    // also the anti-vacuity guard for the two above — it collects the distinct outcomes
    // and requires there to be more than one.
    let schedules = interleavings(&programs());
    let mut renders = std::collections::BTreeSet::new();
    let mut runs = 0_usize;
    for (index, schedule) in schedules.iter().enumerate() {
        for position in 0..=schedule.len() {
            let mut steps: Vec<Step> = schedule.steps().to_vec();
            steps.splice(
                position..position,
                [
                    Step::Cancel {
                        region: RegionId::at(0),
                    },
                    Step::Drain {
                        region: RegionId::at(0),
                    },
                ],
            );

            let (mut tree, root) = fixture();
            let _ = Schedule::new(steps).run(&mut tree);
            let finalization = tree.teardown(root).expect("cancelling twice is legal");
            assert_no_orphans(
                &tree,
                &finalization,
                &format!("schedule {index}, drain completed at {position}"),
            );
            renders.insert(finalization.render());
            runs += 1;
        }
    }
    assert_eq!(runs, schedules.len() * 8);
    assert!(
        renders.len() > 1,
        "every interleaving produced the same outcome, so the sweep tested one behaviour"
    );
    for render in &renders {
        assert!(
            render.contains("total: orphans=0 unresolved=0 balanced=true"),
            "a distinct outcome must still be a total one:\n{render}"
        );
    }
}

#[test]
fn teardown_is_total_across_a_three_level_region_tree() {
    // A cancellation that has to reach three levels down, with a worker at each level and
    // a failure in the middle, so the sweep is not only about depth.
    let mut tree = RegionTree::new();
    let root = tree.root();
    let child = tree.open_child(root).expect("open");
    let grandchild = tree.open_child(child).expect("open");

    let top = tree.spawn(root, Resumability::Resumable).expect("open");
    let middle = tree.spawn(child, Resumability::Resumable).expect("open");
    let bottom = tree.spawn(grandchild, non_resumable()).expect("open");

    tree.advance(top, WorkerStep::Begin).expect("legal");
    tree.advance(top, WorkerStep::Reserve).expect("legal");
    tree.advance(middle, WorkerStep::Begin).expect("legal");
    tree.advance(middle, failure("engine-defect"))
        .expect("legal");
    tree.advance(bottom, WorkerStep::Begin).expect("legal");
    tree.advance(bottom, WorkerStep::Reserve).expect("legal");
    tree.advance(bottom, WorkerStep::Commit).expect("legal");

    let finalization = tree.teardown(root).expect("the root exists");
    assert_no_orphans(&tree, &finalization, "three-level tree");
    assert_eq!(finalization.regions().len(), 3);
    assert_eq!(
        tree.worker_state(middle),
        Ok(&WorkerState::Failed(
            FailureReason::new("engine-defect").expect("canonical")
        )),
        "a worker that already failed keeps its own reason"
    );
    assert_eq!(
        finalization.continuations().len(),
        0,
        "w0 published nothing and w2 is non-resumable, so no continuation is minted"
    );
}

#[test]
fn every_worker_the_tree_ever_admitted_appears_in_the_report() {
    let (mut tree, root) = fixture();
    // Drive one worker to completion and leave the others mid-flight, so the report has
    // to cover workers in three different shapes.
    tree.advance(WorkerId::at(0), WorkerStep::Begin)
        .expect("legal");
    tree.advance(WorkerId::at(0), WorkerStep::Complete)
        .expect("legal");
    tree.advance(WorkerId::at(1), WorkerStep::Begin)
        .expect("legal");

    let admitted = tree.worker_count();
    let finalization = tree.teardown(root).expect("the root exists");
    assert_eq!(finalization.workers().len(), admitted);
    let reported: Vec<WorkerId> = finalization
        .workers()
        .iter()
        .map(continuum_task::region::WorkerReport::worker)
        .collect();
    assert_eq!(
        reported,
        vec![WorkerId::at(0), WorkerId::at(1), WorkerId::at(2)],
        "the report is ordered by identity, with no gap and no duplicate"
    );
    assert_no_orphans(&tree, &finalization, "mixed shapes");
}

// --- the gate is load-bearing --------------------------------------------------------

#[test]
fn finalize_refuses_when_the_drain_is_skipped() {
    // The mutation this file exists to be sure of: if `finalize` did not check, the
    // no-orphan property would be an assertion about a code path rather than a
    // post-condition. Skipping the drain must be refused, and the refusal must name the
    // worker that would have been orphaned.
    let (mut tree, root) = fixture();
    tree.advance(WorkerId::at(0), WorkerStep::Begin)
        .expect("legal");
    tree.cancel(root).expect("not finalized");

    let fault = tree
        .finalize(root)
        .expect_err("finalizing an undrained region must be refused");
    assert_eq!(
        fault.to_string(),
        "cannot finalize r0: w0 is running, which is not terminal"
    );
    assert!(
        !tree.ledger().is_balanced(),
        "and the ledger still shows the debt"
    );

    // Draining first is what makes it succeed, so the refusal is a pre-condition rather
    // than a prohibition.
    tree.drain(root).expect("cancellation drains");
    let finalization = tree.finalize(root).expect("drained");
    assert_no_orphans(&tree, &finalization, "after a proper drain");
}

// --- determinism ---------------------------------------------------------------------

#[test]
fn two_runs_of_one_schedule_render_byte_identically() {
    let schedules = interleavings(&programs());
    // One schedule from the middle of the space, so the check is not about the trivial
    // sequential one at either end.
    let schedule = &schedules[schedules.len() / 2];

    let render = |schedule: &Schedule| {
        let (mut tree, root) = fixture();
        let trace = schedule.run(&mut tree).render();
        let finalization = tree.teardown(root).expect("the root exists");
        format!("{trace}{}", finalization.render())
    };

    let first = render(schedule);
    let second = render(schedule);
    assert_eq!(
        first.as_bytes(),
        second.as_bytes(),
        "INV-005: one schedule, one rendering, byte for byte"
    );
    assert!(!first.is_empty(), "a vacuous render proves nothing");
}

#[test]
fn the_trace_of_a_refused_step_is_the_same_on_every_run() {
    // Determinism has to cover the refusal path too: a schedule whose later steps are
    // rejected because the drain already terminated their workers must render those
    // rejections identically every time, or a golden trace of a cancellation is not a
    // thing the daemon can pin later.
    let schedule = Schedule::new(vec![
        Step::advance(WorkerId::at(0), WorkerStep::Begin),
        Step::advance(WorkerId::at(0), WorkerStep::Reserve),
        Step::Cancel {
            region: RegionId::at(0),
        },
        Step::Drain {
            region: RegionId::at(0),
        },
        Step::advance(WorkerId::at(0), WorkerStep::Commit),
        Step::advance(WorkerId::at(1), WorkerStep::Begin),
        Step::Finalize {
            region: RegionId::at(0),
        },
    ]);
    let render = || {
        let (mut tree, _) = fixture();
        schedule.run(&mut tree).render()
    };
    let first = render();
    assert_eq!(first.as_bytes(), render().as_bytes());
    assert!(
        first.contains("refused w0 is cancelled, so the step commit is not legal"),
        "the refusal must be in the trace verbatim:\n{first}"
    );
    assert!(
        first.contains("total=true"),
        "and the finalization must still be total:\n{first}"
    );
}
