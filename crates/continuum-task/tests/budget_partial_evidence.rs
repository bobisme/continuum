//! Committed partial evidence: the budget accounting bound to the region layer's
//! commitments, swept over an exhaustively enumerated schedule space.
//!
//! > `task.cancel` triggers request → drain → finalize. It MUST leave either committed
//! > partial evidence plus a valid continuation, or nothing published (INV-009, B19).
//! >
//! > — `notes/plan/rfcs/0026-continuumd-native-protocol.md`, "Task lifecycle"
//!
//! The region layer already discharges that rule — `tests/region_cancellation.rs` is its
//! evidence. What this file adds is the second half a *partial result* owes: not only
//! *whether* something was published, but what it cost and what the run spent past it.
//!
//! # The mirror
//!
//! [`mirror`] is the daemon shaped as a function. After every step of a schedule it brings
//! each worker's [`BudgetLedger`] into agreement with what the region tree now says about
//! that worker:
//!
//! | the region layer says | the budget accounting does |
//! |---|---|
//! | a publication is staged | reserve its headroom ([`WorkerStep::Reserve`]) |
//! | the staged publication is gone and the committed count grew | settle the reservation, charge the work, and checkpoint at the new count |
//! | the staged publication is gone and the count did not grow | release the reservation — the artifact was discarded, so its bytes were never written |
//!
//! It is written as a mirror rather than as a co-driven pair of programs because that is
//! the shape of the real integration: `continuumd` owns a task table and would learn what
//! happened from the region layer. The interesting consequence is that **the mirror never
//! sees the drain**: it only ever observes the tree *after* a step, so a cancellation that
//! discards a staged publication looks to it exactly like any other resolution, and the
//! release path is reached without anyone telling it a cancellation occurred.
//!
//! # House rules, inherited unchanged
//!
//! From `tests/region_no_orphan.rs`, which inherited them from the G0-DX-13 campaign:
//! assertions are on outcomes and never on interleavings; every sweep carries an
//! anti-vacuity guard; and determinism is a repeated-round invariant over a canonical
//! rendering.
//!
//! # Evidence map
//!
//! | Claim | Test |
//! |---|---|
//! | the two accountings reconcile at every interleaving, with a cancellation at every position | [`the_two_accountings_reconcile_at_every_interleaving`] |
//! | a partial result names what it committed *and* what it spent past it | [`partial_evidence_names_the_committed_frontier_and_the_spend_beyond_it`] |
//! | the continuation and the checkpoint name the same frontier | [`the_continuation_and_the_checkpoint_agree_on_the_committed_frontier`] |
//! | a discarded publication refunds headroom and commits nothing | [`a_discarded_publication_refunds_its_headroom_and_prices_nothing`] |
//! | exhaustion produces committed partial evidence naming its dimension | [`an_exhausted_campaign_reports_the_dimension_that_stopped_it`] |
//! | one schedule reconciles byte-identically on every run | [`two_runs_of_one_schedule_reconcile_byte_identically`] |

use continuum_task::budget::dimension::{CostDimension, MeterSet};
use continuum_task::budget::partial::{EvidenceBook, TerminationCause};
use continuum_task::budget::{Budget, Exhaustion};
use continuum_task::region::schedule::{Schedule, Step, interleavings};
use continuum_task::region::worker::{
    CancelOutcome, NonResumableReason, Resumability, WorkerId, WorkerStep,
};
use continuum_task::region::{Finalization, RegionId, RegionTree};

/// The dimension standing in for a publication's size — RFC 0027's *enforced* context
/// contract, and the one dimension a reservation is genuinely for.
const BYTES: CostDimension = CostDimension::Bytes;
/// The dimension standing in for the work itself, metered by the engine's state bound.
const STATES: CostDimension = CostDimension::States;

const RESERVED: u64 = 10;
const WRITTEN: u64 = 6;
const EXPLORED: u64 = 5;

fn meters() -> MeterSet {
    MeterSet::none().with(BYTES).with(STATES)
}

fn budget() -> Budget {
    Budget::unbounded()
        .with(STATES, 1_000)
        .with(BYTES, 1_000)
        // Declared and unmetered on purpose: every partial result this file produces
        // carries the INV-007 omission that says so.
        .with(CostDimension::WallMs, 60_000)
}

/// The three worker programs the sweep interleaves — the shapes
/// `tests/region_no_orphan.rs` chose, for the same reason: one worker that publishes, one
/// that stages and never resolves it on its own, and one that finishes without publishing.
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
            Step::advance(WorkerId::at(2), WorkerStep::Complete),
        ],
    ]
}

/// A tree with a child region, three workers, and a budget account for each.
fn fixture() -> (RegionTree, RegionId, EvidenceBook) {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let child = tree.open_child(root).expect("the root is open");
    tree.spawn(root, Resumability::Resumable).expect("open");
    tree.spawn(
        root,
        Resumability::NonResumable(NonResumableReason::new("no-continuation").expect("canonical")),
    )
    .expect("open");
    tree.spawn(child, Resumability::Resumable).expect("open");

    let mut book = EvidenceBook::new();
    for ordinal in 0..3 {
        book.admit(WorkerId::at(ordinal), budget(), meters())
            .expect("each worker is admitted once");
    }
    (tree, root, book)
}

/// Bring every budget account into agreement with what the region tree now says.
///
/// See this file's header for the table. It reads the tree and never writes to it, so it
/// cannot make the two layers agree by moving either one.
fn mirror(tree: &RegionTree, book: &mut EvidenceBook) {
    for ordinal in 0..u32::try_from(tree.worker_count()).expect("small trees") {
        let worker = WorkerId::at(ordinal);
        let evidence = tree.evidence(worker).expect("admitted workers exist");
        let ledger = book.ledger_mut(worker).expect("admitted at spawn");
        let priced = u32::try_from(ledger.checkpoints().len()).expect("small ledgers");

        if evidence.is_provisional() {
            if ledger.reservation(BYTES).is_none() {
                ledger.reserve(BYTES, RESERVED).expect("metered, and free");
            }
            continue;
        }
        if ledger.reservation(BYTES).is_some() {
            if evidence.committed() > priced {
                ledger.settle(BYTES, WRITTEN).expect("reserved");
            } else {
                ledger.release(BYTES).expect("reserved");
            }
        }
        if evidence.committed() > priced {
            ledger.charge(STATES, EXPLORED).expect("metered");
            ledger
                .checkpoint(evidence.committed())
                .expect("nothing reserved, and the count only grows");
        }
    }
}

/// Run `schedule` against a fresh fixture, mirroring the budget accounting at every step,
/// and tear the root down.
fn run(schedule: &Schedule) -> (RegionTree, Finalization, EvidenceBook) {
    let (mut tree, root, mut book) = fixture();
    mirror(&tree, &mut book);
    for step in schedule.steps() {
        let _ = Schedule::new(vec![step.clone()]).run(&mut tree);
        mirror(&tree, &mut book);
    }
    let finalization = tree.teardown(root).expect("the root exists");
    mirror(&tree, &mut book);
    (tree, finalization, book)
}

// --- the sweep -------------------------------------------------------------------------

#[test]
fn the_two_accountings_reconcile_at_every_interleaving() {
    let schedules = interleavings(&programs());
    assert_eq!(
        schedules.len(),
        210,
        "the shuffle product of 3+2+2 steps: 7! / (3! * 2! * 2!)"
    );

    let mut runs = 0_usize;
    let mut renderings: Vec<String> = Vec::new();
    let mut with_committed_evidence = 0_usize;

    for (index, schedule) in schedules.iter().enumerate() {
        for position in 0..=schedule.len() {
            // The cancellation *completes* mid-schedule — request and drain both — so
            // every step after the injection lands on a worker that is already terminal
            // and is refused. Which steps got in first is therefore what distinguishes one
            // interleaving from another, and a worker's publication is committed, staged
            // and discarded, or never begun depending on where the injection fell.
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
            let context = format!("schedule {index}, cancellation completed at {position}");
            let (_, finalization, book) = run(&Schedule::new(steps));

            assert!(
                finalization.is_total(),
                "{context}: teardown was not total\n{}",
                finalization.render()
            );

            let reconciliation = book.reconcile(&finalization);
            assert!(
                reconciliation.is_reconciled(),
                "{context}: the budget accounting and the region layer disagree\n{}",
                reconciliation.render()
            );
            assert_eq!(
                reconciliation.entries().len(),
                finalization.workers().len(),
                "{context}: every torn-down worker must have been priced"
            );

            for report in finalization.workers() {
                let Some(evidence) = book.partial_evidence(report.worker(), report.state()) else {
                    continue;
                };
                assert_eq!(
                    evidence.committed(),
                    report.evidence().committed(),
                    "{context}: {} names a different committed count than the region layer",
                    report.worker()
                );
                assert!(
                    evidence.total_spend().measured(STATES).unwrap_or(0)
                        >= evidence.spend_at_commit().measured(STATES).unwrap_or(0),
                    "{context}: spend is monotone, so the total cannot be below a checkpoint"
                );
                assert_eq!(
                    evidence
                        .omissions()
                        .iter()
                        .map(|omission| omission.subject())
                        .collect::<Vec<_>>(),
                    vec!["budget.wall_ms".to_owned()],
                    "{context}: the declared, unmetered ceiling travels with the evidence"
                );
                if evidence.published_anything() {
                    with_committed_evidence += 1;
                }
            }

            renderings.push(reconciliation.render());
            runs += 1;
        }
    }

    assert_eq!(
        runs,
        schedules.len() * 8,
        "every position of every schedule must have been swept"
    );
    assert!(
        with_committed_evidence > 0,
        "a sweep in which nothing ever committed proves nothing about partial evidence"
    );
    renderings.sort();
    renderings.dedup();
    assert!(
        renderings.len() > 1,
        "every schedule reconciled to the same thing, so the sweep exercised one behaviour"
    );
}

// --- the binding -----------------------------------------------------------------------

#[test]
fn partial_evidence_names_the_committed_frontier_and_the_spend_beyond_it() {
    // Commit once, explore further, then cancel: the spend past the last commit bought
    // nothing durable, and a partial result has to say so.
    let (mut tree, root, mut book) = fixture();
    let worker = WorkerId::at(0);
    for step in [WorkerStep::Begin, WorkerStep::Reserve, WorkerStep::Commit] {
        tree.advance(worker, step).expect("legal");
        mirror(&tree, &mut book);
    }
    book.ledger_mut(worker)
        .expect("admitted")
        .charge(STATES, 40)
        .expect("metered");

    let finalization = tree.teardown(root).expect("the root exists");
    let report = finalization
        .workers()
        .iter()
        .find(|report| report.worker() == worker)
        .expect("every worker is reported");
    let evidence = book
        .partial_evidence(worker, report.state())
        .expect("a cancelled worker has partial evidence");

    assert_eq!(evidence.committed(), 1);
    assert_eq!(evidence.cause(), &TerminationCause::Cancelled);
    assert_eq!(evidence.spend_at_commit().measured(STATES), Some(EXPLORED));
    assert_eq!(evidence.spend_at_commit().measured(BYTES), Some(WRITTEN));
    assert_eq!(evidence.total_spend().measured(STATES), Some(EXPLORED + 40));
    assert_eq!(
        evidence.uncommitted_spend().measured(STATES),
        Some(40),
        "work done after the last commit is real spend against no artifact"
    );
    assert_eq!(
        evidence.uncommitted_spend().measured(BYTES),
        Some(0),
        "nothing was written after the commit"
    );
    assert_eq!(
        evidence.total_spend().measured(CostDimension::WallMs),
        None,
        "a dimension nothing measures is absent, never zero"
    );
}

#[test]
fn the_continuation_and_the_checkpoint_agree_on_the_committed_frontier() {
    let (mut tree, root, mut book) = fixture();
    let worker = WorkerId::at(0);
    for step in [
        WorkerStep::Begin,
        WorkerStep::Reserve,
        WorkerStep::Commit,
        WorkerStep::Reserve,
        WorkerStep::Commit,
    ] {
        tree.advance(worker, step).expect("legal");
        mirror(&tree, &mut book);
    }
    let finalization = tree.teardown(root).expect("the root exists");
    let report = finalization
        .workers()
        .iter()
        .find(|report| report.worker() == worker)
        .expect("every worker is reported");

    let continuation = match report.cancel_outcome() {
        Some(CancelOutcome::CommittedWithContinuation(continuation)) => *continuation,
        other => {
            panic!("a resumable worker with committed evidence owes a continuation: {other:?}")
        }
    };
    let evidence = book
        .partial_evidence(worker, report.state())
        .expect("a cancelled worker has partial evidence");
    assert_eq!(
        continuation.committed(),
        evidence.committed(),
        "the region layer's continuation and the budget layer's checkpoint must name one \
         frontier, or a resume would start from a place nobody priced"
    );
    assert_eq!(evidence.committed(), 2);
    assert_eq!(
        evidence
            .checkpoint()
            .map(|checkpoint| checkpoint.sequence()),
        Some(1),
        "the last checkpoint is the frontier a continuation resumes from"
    );
}

#[test]
fn a_discarded_publication_refunds_its_headroom_and_prices_nothing() {
    let (mut tree, root, mut book) = fixture();
    let worker = WorkerId::at(1);
    for step in [WorkerStep::Begin, WorkerStep::Reserve] {
        tree.advance(worker, step).expect("legal");
        mirror(&tree, &mut book);
    }
    assert_eq!(
        book.ledger(worker).expect("admitted").reservation(BYTES),
        Some(RESERVED),
        "a staged publication holds its headroom"
    );

    let finalization = tree.teardown(root).expect("the root exists");
    mirror(&tree, &mut book);

    let report = finalization
        .workers()
        .iter()
        .find(|report| report.worker() == worker)
        .expect("every worker is reported");
    assert_eq!(
        report.cancel_outcome(),
        Some(&CancelOutcome::NothingPublished)
    );
    let ledger = book.ledger(worker).expect("admitted");
    assert_eq!(ledger.reservation(BYTES), None, "the headroom came back");
    assert_eq!(
        ledger.spend().measured(BYTES),
        Some(0),
        "a discarded publication wrote no bytes, so none were spent"
    );
    assert!(
        ledger.checkpoints().is_empty(),
        "nothing was committed to price"
    );
    assert!(
        book.reconcile(&finalization).is_reconciled(),
        "committed-or-discarded, never dangling — read off headroom rather than off the tree"
    );
}

#[test]
fn an_exhausted_campaign_reports_the_dimension_that_stopped_it() {
    let (mut tree, root, mut book) = fixture();
    let worker = WorkerId::at(0);
    for step in [WorkerStep::Begin, WorkerStep::Reserve, WorkerStep::Commit] {
        tree.advance(worker, step).expect("legal");
        mirror(&tree, &mut book);
    }
    let ledger = book.ledger_mut(worker).expect("admitted");
    // Tighten to just above the committed spend, then ask for more than is left.
    ledger.charge(STATES, 990).expect("metered");
    let outcome = ledger.charge(STATES, 10).expect("metered");
    let exhaustion = *outcome
        .exhaustion()
        .expect("995 + 10 is past the declared 1000");
    assert_eq!(exhaustion.dimension(), STATES);

    // The region layer's own vocabulary for "this run stopped": a typed failure.
    let failure = continuum_task::region::worker::FailureReason::new("budget-exhausted")
        .expect("canonical token");
    tree.advance(worker, WorkerStep::Fail(failure))
        .expect("legal");
    let finalization = tree.teardown(root).expect("the root exists");
    let report = finalization
        .workers()
        .iter()
        .find(|report| report.worker() == worker)
        .expect("every worker is reported");

    let evidence = book
        .partial_evidence(worker, report.state())
        .expect("a failed worker with an account has partial evidence");
    assert_eq!(
        evidence.cause().exhaustion().map(Exhaustion::dimension),
        Some(STATES),
        "the cause is the dimension that ran out, not the token somebody typed"
    );
    assert_eq!(
        evidence
            .cause()
            .exhaustion()
            .map(Exhaustion::error_code_token),
        Some("BudgetExhausted")
    );
    assert_eq!(
        evidence.committed(),
        1,
        "an exhausted campaign keeps what it committed (INV-009)"
    );
    assert_eq!(
        evidence.uncommitted_spend().measured(STATES),
        Some(990),
        "the spend that produced no further artifact is reported separately"
    );
    assert!(book.reconcile(&finalization).is_reconciled());
}

// --- determinism -----------------------------------------------------------------------

#[test]
fn two_runs_of_one_schedule_reconcile_byte_identically() {
    let schedule = Schedule::new(vec![
        Step::advance(WorkerId::at(0), WorkerStep::Begin),
        Step::advance(WorkerId::at(0), WorkerStep::Reserve),
        Step::advance(WorkerId::at(1), WorkerStep::Begin),
        Step::advance(WorkerId::at(0), WorkerStep::Commit),
        Step::advance(WorkerId::at(1), WorkerStep::Reserve),
        Step::Cancel {
            region: RegionId::at(0),
        },
    ]);
    let render = || {
        let (_, finalization, book) = run(&schedule);
        book.reconcile(&finalization).render()
    };
    let first = render();
    assert_eq!(first.as_bytes(), render().as_bytes());
    assert!(!first.is_empty(), "a vacuous render proves nothing");
    assert!(
        first.contains("committed=1 checkpoints=1"),
        "the rendering has to carry the numbers it is comparing:\n{first}"
    );
}

#[test]
fn two_ledgers_built_the_same_way_render_identically() {
    let build = || {
        let (_, _, book) = run(&Schedule::new(vec![
            Step::advance(WorkerId::at(0), WorkerStep::Begin),
            Step::advance(WorkerId::at(0), WorkerStep::Reserve),
            Step::advance(WorkerId::at(0), WorkerStep::Commit),
        ]));
        book.ledger(WorkerId::at(0))
            .expect("admitted")
            .render()
            .clone()
    };
    let first = build();
    assert_eq!(first.as_bytes(), build().as_bytes());
    assert!(first.contains("checkpoint 0 committed=1"), "{first}");
    assert!(
        first.contains("unsupported:budget.wall_ms"),
        "the unenforced ceiling is in the canonical rendering too:\n{first}"
    );
}
