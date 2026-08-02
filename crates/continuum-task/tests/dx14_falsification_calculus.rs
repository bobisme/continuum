//! **G0-DX-14 falsification, calculus grain.** The one attack the daemon grain structurally
//! cannot make: a *hostile prefix* of region operations injected while a cancellation is
//! half-done, with a publication in flight.
//!
//! > | G0-DX-14 | Can cancellation of verification work close correctly? | cancel DPOR,
//! > solver, proof, and synthesis tasks at every phase | no leaked obligations; resumable
//! > artifacts either committed or absent |
//! >
//! > — `notes/plan/notes/G0_SPIKE_MATRIX.md`
//!
//! `crates/continuumd/tests/dx14_falsification.rs` is the daemon half of this campaign, and
//! it is bounded by what a wire client can do: `Daemon::dispatch` holds `&mut DaemonState` for
//! a whole call, so no hostile sequence there can land *between* a `Reserve` and its `Commit`,
//! and every teardown it can reach is one the daemon itself drove in the right order. This
//! file attacks the layer where neither is true — where the teardown steps are separable
//! values a caller can issue in any order, at any moment, including inside a publication.
//!
//! # What `dx14_cancellation_matrix.rs` establishes, and what is attacked here
//!
//! bn-2zy's constructive matrix drives each lane's *legal* program: 27 phases × 4 placements
//! × 3 teardown arrivals, every run checked for a balanced ledger, both-or-neither on every
//! arm, and agreement with `EvidenceBook::reconcile`. `region_lifecycle.rs` establishes,
//! separately, that each illegal call is refused by name and leaves the tree byte-identical.
//!
//! Neither says what happens when those refusals are **composed**. "Every single illegal call
//! is refused" and "a legal program is total" do not together imply "a program with illegal
//! calls scattered through it is still total": a refusal that left half a state change behind,
//! or that opened an obligation before it refused, would satisfy both existing claims and
//! still leak. That composition is this file's target, and the property it demands is
//! **recovery**: after *any* prefix of hostile operations, the teardown still succeeds, the
//! obligation ledger still balances, and the two independent accountings still agree.
//!
//! # Campaign map — attack → test → result
//!
//! | # | Attack | Test | Result |
//! |---|---|---|---|
//! | C1 | every 3-operation hostile program (216) over a worker mid-publication, then a teardown | [`negative_a_teardown_after_any_hostile_prefix_is_still_total`] | held |
//! | C2 | the same 216, cross-checked against the budget book's independent accounting | [`negative_no_hostile_prefix_makes_the_two_accountings_disagree`] | held |
//! | C3 | the cross-check is load-bearing: a deliberately broken mirror is caught | [`positive_the_budget_cross_check_catches_a_mirror_that_stopped_agreeing`] | held (control) |
//! | C4 | a hostile prefix that *refuses* must leave the tree byte-identical, composed | [`negative_a_refused_operation_leaves_no_residue_however_many_precede_it`] | held |
//! | C5 | cancel a 4-deep chain whose workers sit in four different states, one mid-publication | [`negative_a_deep_chain_cancelled_at_the_root_leaves_no_level_behind`] | held |
//!
//! Nothing in `src/` was patched. Every attack is a sequence of public calls.
//!
//! # OOM hygiene
//!
//! C1 and C2 share one bounded enumeration: `6^3 = 216` programs over a tree with one region
//! and one worker. C5 is a 4-deep chain. No input is built by doubling, and every sweep's size
//! is a written-down constant this file asserts.

use continuum_task::budget::Budget;
use continuum_task::budget::dimension::{CostDimension, MeterSet};
use continuum_task::budget::partial::EvidenceBook;
use continuum_task::region::worker::{Resumability, WorkerId, WorkerState, WorkerStep};
use continuum_task::region::{Finalization, RegionId, RegionTree};

/// The dimension a staged publication holds headroom on, as `budget_partial_evidence.rs` uses
/// it: a payload's true size is known only after it is built, so `bytes` is the dimension
/// where a reservation is a publication in flight.
const BYTES: CostDimension = CostDimension::Bytes;
/// The dimension the work itself is charged to.
const WORK_DIMENSION: CostDimension = CostDimension::States;

const RESERVED: u64 = 10;
const WRITTEN: u64 = 6;
const WORK: u64 = 5;

fn meters() -> MeterSet {
    MeterSet::none().with(WORK_DIMENSION).with(BYTES)
}

/// Bring one worker's budget account into agreement with what the region tree now says.
///
/// The mirror `dx14_cancellation_matrix.rs` uses, kept deliberately identical in behaviour: it
/// **reads** the tree and never writes to it, so it cannot make the two layers agree by moving
/// either one, and it never sees the drain — a cancellation that discards a staged artifact
/// looks to it exactly like any other resolution.
fn mirror(tree: &RegionTree, worker: WorkerId, book: &mut EvidenceBook) {
    let Ok(evidence) = tree.evidence(worker) else {
        return;
    };
    let ledger = book.ledger_mut(worker).expect("admitted at spawn");
    let mut priced = u32::try_from(ledger.checkpoints().len()).expect("small ledgers");

    if evidence.is_provisional() {
        if ledger.reservation(BYTES).is_none() {
            ledger.reserve(BYTES, RESERVED).expect("metered, and free");
        }
        return;
    }
    if ledger.reservation(BYTES).is_some() {
        if evidence.committed() > priced {
            ledger.settle(BYTES, WRITTEN).expect("reserved");
        } else {
            ledger.release(BYTES).expect("reserved");
        }
    }
    while priced < evidence.committed() {
        priced += 1;
        ledger.charge(WORK_DIMENSION, WORK).expect("metered");
        ledger
            .checkpoint(priced)
            .expect("nothing reserved, and the count only grows");
    }
}

/// A canonical rendering of everything the tree holds, for before/after comparison.
///
/// Assembled from the public observers rather than from a debug format, and **including the
/// obligation ledger** — which is the half a residue check has to carry, because a call that
/// refused after opening an obligation would leave every other line unchanged. The same shape
/// `region_lifecycle.rs` uses one call at a time.
fn render(tree: &RegionTree) -> String {
    let mut out = String::new();
    for ordinal in 0..u32::try_from(tree.region_count()).expect("small trees") {
        let region = RegionId::at(ordinal);
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

// --- the hostile alphabet ---------------------------------------------------------------------

/// One operation a hostile caller can issue against a region tree mid-cancellation.
///
/// Six, and the choice is not arbitrary: three are the teardown's own steps out of order, and
/// three are the calls `region_lifecycle.rs` establishes are individually refused. Composing
/// them is the point — a refusal that leaked would need a *second* operation afterwards for
/// the leak to become visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Hostile {
    /// Close the region — legal once, a refusal every time after.
    Close,
    /// Request cancellation — legal until the region is finalized, and idempotent.
    Cancel,
    /// Drain — a refusal before any request, and idempotent after one.
    Drain,
    /// Finalize — a refusal before the drain, and a refusal a second time.
    Finalize,
    /// Spawn a second worker into the region — a refusal once it stopped accepting work.
    Spawn,
    /// Step the worker on — legal or not depending on where the worker is.
    Step,
}

impl Hostile {
    const ALL: [Self; 6] = [
        Self::Close,
        Self::Cancel,
        Self::Drain,
        Self::Finalize,
        Self::Spawn,
        Self::Step,
    ];

    const fn token(self) -> &'static str {
        match self {
            Self::Close => "close",
            Self::Cancel => "cancel",
            Self::Drain => "drain",
            Self::Finalize => "finalize",
            Self::Spawn => "spawn",
            Self::Step => "step",
        }
    }
}

/// Every hostile program of exactly three operations, in a fixed order. `6³ = 216`, built by
/// one linear pass per position.
fn programs() -> Vec<[Hostile; 3]> {
    let mut out = Vec::with_capacity(216);
    for first in Hostile::ALL {
        for second in Hostile::ALL {
            for third in Hostile::ALL {
                out.push([first, second, third]);
            }
        }
    }
    out
}

/// A tree holding one region below the root, one resumable worker inside it that has committed
/// one publication and **staged a second**, and a budget book mirroring it.
///
/// The mid-publication state is the whole point: it is the moment RFC 0026's "cancellation
/// MUST NOT truncate a publication in progress" is about, and it is the moment the daemon
/// grain cannot construct. The book therefore starts with a live `bytes` reservation, which is
/// the budget-side reading of the same in-flight artifact.
fn mid_publication() -> (RegionTree, EvidenceBook, RegionId, WorkerId) {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let region = tree.open_child(root).expect("the root is open");
    let worker = tree
        .spawn(region, Resumability::Resumable)
        .expect("the region is open");

    let mut book = EvidenceBook::new();
    book.admit(worker, Budget::unbounded(), meters())
        .expect("one account per worker");

    for step in [
        WorkerStep::Begin,
        WorkerStep::Reserve,
        WorkerStep::Commit,
        WorkerStep::Reserve,
    ] {
        tree.advance(worker, step).expect("a legal opening");
        mirror(&tree, worker, &mut book);
    }
    assert!(
        tree.evidence(worker).expect("admitted").is_provisional(),
        "the fixture must actually hold a publication in flight"
    );
    assert_eq!(
        book.ledger(worker).expect("admitted").reservations().len(),
        1,
        "and the budget side must be holding its headroom"
    );
    (tree, book, region, worker)
}

/// Apply one hostile operation, mirroring the budget book afterwards. Refusals are recorded,
/// never raised: a refused call is the *expected* case here, and the property under test is
/// what the tree looks like afterwards.
fn apply(
    tree: &mut RegionTree,
    book: &mut EvidenceBook,
    region: RegionId,
    worker: WorkerId,
    operation: Hostile,
) -> Result<(), String> {
    let outcome = match operation {
        Hostile::Close => tree.close(region).map_err(|fault| fault.to_string()),
        Hostile::Cancel => tree.cancel(region).map_err(|fault| fault.to_string()),
        Hostile::Drain => tree.drain(region).map_err(|fault| fault.to_string()),
        Hostile::Finalize => tree
            .finalize(region)
            .map(|_| ())
            .map_err(|fault| fault.to_string()),
        Hostile::Spawn => tree
            .spawn(region, Resumability::Resumable)
            .map(|_| ())
            .map_err(|fault| fault.to_string()),
        Hostile::Step => tree
            .advance(worker, WorkerStep::Commit)
            .map(|_| ())
            .map_err(|fault| fault.to_string()),
    };
    mirror(tree, worker, book);
    outcome
}

/// Finish the teardown of `region`, whatever the hostile prefix left behind.
///
/// A region a hostile prefix already finalized has nothing left to tear down and reports the
/// finalization it produced; every other state is driven to one by `teardown`, whose own
/// documentation makes the totality claim this test is trying to break: "for any region that
/// exists and has not already been finalized, it succeeds, whatever state the subtree's
/// workers are in and whatever the schedule did to get them there".
fn finish(
    tree: &mut RegionTree,
    region: RegionId,
    already: Option<Finalization>,
) -> Result<Finalization, String> {
    match already {
        Some(finalization) => Ok(finalization),
        None => tree.teardown(region).map_err(|fault| fault.to_string()),
    }
}

/// Run one hostile program and return what the teardown left, plus the book that watched it.
fn run(program: &[Hostile; 3]) -> (RegionTree, EvidenceBook, Finalization, WorkerId) {
    let (mut tree, mut book, region, worker) = mid_publication();
    let mut finalized: Option<Finalization> = None;
    for operation in program {
        if *operation == Hostile::Finalize && finalized.is_none() {
            // Capture the report when a hostile `finalize` is the call that succeeds: a
            // `Finalization` is handed back exactly once, and a test that threw it away would
            // have to re-derive the answer it is checking.
            match tree.finalize(region) {
                Ok(report) => {
                    finalized = Some(report);
                    mirror(&tree, worker, &mut book);
                    continue;
                }
                Err(_) => {
                    mirror(&tree, worker, &mut book);
                    continue;
                }
            }
        }
        let _ = apply(&mut tree, &mut book, region, worker, *operation);
    }
    let finalization = finish(&mut tree, region, finalized).unwrap_or_else(|fault| {
        panic!(
            "{}: the teardown itself was refused — {fault}",
            label(program)
        )
    });
    // The mirror runs once more, *after* the teardown, and this is where the budget side's
    // half of "committed or absent, never dangling" is decided. It still only reads the tree:
    // it releases the `bytes` headroom because the tree now says nothing is provisional, and
    // if the drain had left the artifact in flight it would keep holding it and the
    // reconciliation below would name the dangling reservation. Mirroring here is therefore
    // the check, not a way of making the two sides agree.
    mirror(&tree, worker, &mut book);
    (tree, book, finalization, worker)
}

fn label(program: &[Hostile; 3]) -> String {
    program
        .iter()
        .map(|operation| operation.token())
        .collect::<Vec<_>>()
        .join(",")
}

// --- C1 ----------------------------------------------------------------------------------------

/// C1 — a teardown after **any** three-operation hostile prefix is still total.
///
/// The composition attack. Each of the six operations is individually refused-or-accepted
/// correctly (`region_lifecycle.rs`); this asks whether 216 compositions of them can leave a
/// state a teardown cannot close. For every program it holds all four halves of the pass
/// condition at once:
///
/// - every worker the tree admitted is terminal — no orphan, including the worker a hostile
///   `spawn` managed to add;
/// - the obligation ledger is balanced — no cancellation requested and never finalized, which
///   is the leak the row's own experiment names;
/// - `Finalization::is_total` agrees with that, computed independently;
/// - the staged publication is gone: **absent**, never a committed artifact the drain
///   truncated into existence, and never still in flight.
#[test]
fn negative_a_teardown_after_any_hostile_prefix_is_still_total() {
    let programs = programs();
    assert_eq!(programs.len(), 216, "6³ hostile programs");
    let mut committed_seen: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
    for program in &programs {
        let (tree, _, finalization, worker) = run(program);
        let context = label(program);

        assert!(
            finalization.is_total(),
            "{context}: the teardown report is not total\n{}",
            finalization.render()
        );
        assert!(
            tree.ledger().is_balanced(),
            "{context}: an obligation is still outstanding after the teardown"
        );
        for ordinal in 0..tree.worker_count() {
            let admitted = WorkerId::at(u32::try_from(ordinal).expect("small trees"));
            let state = tree.worker_state(admitted).expect("admitted workers exist");
            assert!(
                state.is_terminal(),
                "{context}: {admitted} is {state}, which is not terminal — an orphan"
            );
        }
        let evidence = tree.evidence(worker).expect("admitted");
        assert!(
            !evidence.is_provisional(),
            "{context}: a publication is still in flight after the teardown — neither \
             committed nor absent"
        );
        assert!(
            evidence.committed() >= 1,
            "{context}: the publication committed before the attack was un-published"
        );
        assert!(
            evidence.committed() <= 2,
            "{context}: the drain manufactured a commit out of a staged publication"
        );
        committed_seen.insert(evidence.committed());
    }
    assert_eq!(
        committed_seen,
        [1, 2].into_iter().collect(),
        "the sweep must reach both the discarded-staged and the committed-staged endings, or \
         it is only testing one of them"
    );
}

// --- C2, C3 -------------------------------------------------------------------------------------

/// C2 — no hostile prefix makes the region layer and the budget book disagree.
///
/// Two accountings of one fact, computed from different data: the region tree counts
/// commitments and knows nothing about cost, and `EvidenceBook` prices work and knows nothing
/// about artifacts. `reconcile` is the join, and a disagreement is exactly the shape a leak
/// takes when only one side notices it — a commitment with no price, a price with no
/// commitment, or headroom still held in flight for a publication that no longer exists.
///
/// The constructive matrix checks this over its legal programs. This checks it over the 216
/// hostile ones, and additionally holds the budget side's own resting invariant: **no
/// reservation survives a teardown**, which is the budget-side spelling of "committed or
/// absent, never dangling".
#[test]
fn negative_no_hostile_prefix_makes_the_two_accountings_disagree() {
    for program in &programs() {
        let (_, book, finalization, worker) = run(program);
        let context = label(program);
        let reconciliation = book.reconcile(&finalization);
        assert!(
            reconciliation.is_reconciled(),
            "{context}: the two accountings disagree\n{}",
            reconciliation.render()
        );
        assert!(
            book.ledger(worker)
                .expect("admitted")
                .reservations()
                .is_empty(),
            "{context}: headroom is still held in flight after the teardown"
        );
    }
}

/// C3 — the control that makes C2 mean something: a mirror that stopped agreeing **is**
/// caught.
///
/// A cross-check that cannot fail proves nothing, so this drives the same fixture with the
/// mirror deliberately withheld after the last commit — the budget side never prices the
/// artifact the region layer committed — and requires `reconcile` to name the disagreement.
/// The system under test is untouched; what is mutated is this file's own instrument.
#[test]
fn positive_the_budget_cross_check_catches_a_mirror_that_stopped_agreeing() {
    let (mut tree, book, region, worker) = mid_publication();
    // Commit the staged publication *without* telling the book: the region layer now counts
    // two commitments and the ledger has priced one.
    tree.advance(worker, WorkerStep::Commit)
        .expect("a staged publication may commit");
    let finalization = tree.teardown(region).expect("the region exists");
    let reconciliation = book.reconcile(&finalization);
    assert!(
        !reconciliation.is_reconciled(),
        "a budget book that stopped pricing commitments must be caught:\n{}",
        reconciliation.render()
    );

    // And the same run with the mirror kept up to date does agree, so the failure above is the
    // withheld pricing rather than the fixture.
    let (mut tree, mut book, region, worker) = mid_publication();
    tree.advance(worker, WorkerStep::Commit)
        .expect("a staged publication may commit");
    mirror(&tree, worker, &mut book);
    let finalization = tree.teardown(region).expect("the region exists");
    mirror(&tree, worker, &mut book);
    assert!(
        book.reconcile(&finalization).is_reconciled(),
        "the same run with an honest mirror must reconcile"
    );
}

// --- C4 -----------------------------------------------------------------------------------------

/// C4 — a refused operation leaves no residue, however many refusals precede it.
///
/// `region_lifecycle.rs` establishes this one call at a time: "every refused call leaves the
/// tree byte-identical". The composed version is the one a leak would hide in — a call that
/// refused *after* opening an obligation would still leave the tree's rendering unchanged if
/// the rendering did not include the ledger, and would still pass a one-call test if the next
/// call happened to discharge it.
///
/// So this drives a finalized region — the state in which **every** operation is refused —
/// with all 216 programs, and requires the tree's canonical rendering to be byte-identical to
/// what it was before the program ran, every time.
#[test]
fn negative_a_refused_operation_leaves_no_residue_however_many_precede_it() {
    let mut refusals = 0_usize;
    for program in &programs() {
        let (mut tree, mut book, region, worker) = mid_publication();
        tree.teardown(region).expect("the region exists");
        let before = render(&tree);
        assert!(
            tree.ledger().is_balanced(),
            "the fixture starts from a balanced ledger"
        );

        for operation in program {
            let outcome = apply(&mut tree, &mut book, region, worker, *operation);
            assert!(
                outcome.is_err(),
                "{}: {} was accepted against a finalized region",
                label(program),
                operation.token()
            );
            refusals += 1;
        }
        assert_eq!(
            render(&tree).as_bytes(),
            before.as_bytes(),
            "{}: a refused program left residue in the tree",
            label(program)
        );
        assert!(
            tree.ledger().is_balanced(),
            "{}: a refused program opened an obligation nobody discharged",
            label(program)
        );
    }
    assert_eq!(
        refusals,
        216 * 3,
        "every operation of every program refused"
    );
}

// --- C5 -----------------------------------------------------------------------------------------

/// C5 — a four-deep chain cancelled at the root leaves no level behind.
///
/// The nesting attack: four regions in a chain, each owning one worker, each worker in a
/// different lifecycle state — one never begun, one running, one parked with committed
/// evidence, one **mid-publication** — and the cancellation named at the root, four levels
/// above the artifact in flight. What must hold is that the request reaches every level, the
/// drain resolves the staged half at the deepest one, and every region finalizes with a
/// balanced ledger. A cancellation that stopped at the first level it could satisfy would
/// leave the rest as orphans, which is the leak the row's experiment is named after.
#[test]
fn negative_a_deep_chain_cancelled_at_the_root_leaves_no_level_behind() {
    let mut tree = RegionTree::new();
    let mut book = EvidenceBook::new();
    let mut regions = Vec::new();
    let mut workers = Vec::new();

    let mut region = tree.root();
    for _ in 0..4 {
        region = tree.open_child(region).expect("each region is open");
        regions.push(region);
        let worker = tree
            .spawn(region, Resumability::Resumable)
            .expect("the region is open");
        book.admit(worker, Budget::unbounded(), meters())
            .expect("one account per worker");
        workers.push(worker);
    }

    // Four different lifecycle states, deepest last.
    let programs: [&[WorkerStep]; 4] = [
        &[],
        &[WorkerStep::Begin],
        &[
            WorkerStep::Begin,
            WorkerStep::Reserve,
            WorkerStep::Commit,
            WorkerStep::Suspend,
        ],
        &[WorkerStep::Begin, WorkerStep::Reserve],
    ];
    for (worker, steps) in workers.iter().zip(programs) {
        for step in steps {
            tree.advance(*worker, step.clone())
                .expect("each opening is legal");
            mirror(&tree, *worker, &mut book);
        }
    }
    assert!(
        tree.evidence(workers[3])
            .expect("admitted")
            .is_provisional(),
        "the deepest worker must be mid-publication for the attack to be the one described"
    );

    let root_of_chain = regions[0];
    let finalization = tree.teardown(root_of_chain).expect("the region exists");
    for worker in &workers {
        // As in the 216-program sweep: the mirror reads the post-drain tree, so a staged
        // artifact the drain failed to resolve would leave its headroom held and be named by
        // the reconciliation below.
        mirror(&tree, *worker, &mut book);
    }

    assert!(
        finalization.is_total(),
        "the chain's teardown is not total\n{}",
        finalization.render()
    );
    assert_eq!(
        finalization.workers().len(),
        4,
        "every level's worker must appear in the report, or a level was skipped"
    );
    for worker in &workers {
        let state = tree.worker_state(*worker).expect("admitted");
        assert_eq!(
            state,
            &WorkerState::Cancelled,
            "{worker} was left behind by a cancellation four levels above it"
        );
    }
    assert!(
        !tree
            .evidence(workers[3])
            .expect("admitted")
            .is_provisional(),
        "the deepest worker's staged publication survived the drain"
    );
    assert_eq!(
        tree.evidence(workers[2]).expect("admitted").committed(),
        1,
        "and the parked level's committed evidence was not truncated"
    );
    assert!(
        tree.ledger().is_balanced(),
        "an obligation from one of the four levels is still outstanding"
    );
    assert!(
        book.reconcile(&finalization).is_reconciled(),
        "the budget book disagrees with the chain's teardown:\n{}",
        book.reconcile(&finalization).render()
    );
}
