//! **G0-DX-14, the campaign.** Cancel a DPOR, solver, proof and synthesis task at every
//! instrumented phase, and check both conjuncts of the pass condition at every one.
//!
//! > | G0-DX-14 | Can cancellation of verification work close correctly? | cancel DPOR,
//! > solver, proof, and synthesis tasks at every phase | no leaked obligations; resumable
//! > artifacts either committed or absent | integrate deeper with asupersync |
//! >
//! > — `notes/plan/notes/G0_SPIKE_MATRIX.md`
//!
//! > **Exit:** cancellation at every instrumented phase leaves either a valid continuation
//! > or no published partial artifact.
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 6
//!
//! # What a "task shape" is here, stated before it is used
//!
//! Three of the four engines the matrix row names do not exist in this workspace, and this
//! file does not pretend otherwise. `crates/continuum-task/tests/region_cancellation.rs`
//! said so when it fixed the *shape* the campaign would be argued in; this file is the
//! campaign, and the honesty it inherits is worth restating exactly:
//!
//! - a **lane** here is a *lifecycle profile*, not an engine. It is a written-down sequence
//!   of the six things the region calculus can observe about work — begin, stage, publish,
//!   park, wake, finish — together with the SD-12 cost dimension that work is metered on and
//!   the resumability it declares at admission. Nothing here explores a state space, calls a
//!   solver, checks a proof or enumerates a candidate;
//! - what makes the four lanes *different*, and therefore what makes the matrix a matrix
//!   rather than one test run four times, is their **publication and suspension profiles**.
//!   Those are the only two things cancellation-correctness is a function of, which is the
//!   whole reason the row can be closed without the engines: `CancelOutcome` is computed
//!   from committed evidence and declared resumability and from nothing else
//!   (`crates/continuum-task/src/region.rs`, `RegionTree::cancel_outcome`), so a lane that
//!   presents a profile presents everything the rule can see;
//! - the profiles are grounded rather than invented. `docs/25_SEMANTIC_FEATURE_MATRIX.md`
//!   gives the four lanes their dimensions (`states` for explicit/DPOR exploration,
//!   `solver_ms` for the SMT/PDR lane, `proof_ms` for Lean, `candidates` for synthesis), and
//!   RFC 0026 gives resumability its two named cases — a continuation, or a typed
//!   `non_resumable_reason`. What each lane's beats are is stated in [`LANES`], row by row,
//!   with the reason for each.
//!
//! So the claim this file supports is precisely: **for every publication/suspension profile
//! the four lanes present, cancellation arriving at every phase of that profile leaves no
//! leaked obligation and no artifact that is neither committed nor absent.** It is not a
//! claim about DPOR, and when a real DPOR engine lands its lifecycle is held to this table
//! rather than to a new one.
//!
//! # The three independent accountings, per run
//!
//! Every run in this file is checked by [`assert_pass_condition`], which is deliberately
//! more than [`Finalization::is_total`]:
//!
//! 1. **no leaked obligations** — the obligation ledger is balanced *and* it opened
//!    something, read both off the finalization report and off the tree;
//! 2. **committed or absent** — no worker holds a staged publication, and no worker is
//!    non-terminal, re-derived from the tree rather than from the report;
//! 3. **the budget accounting agrees** — [`EvidenceBook::reconcile`]'s third accounting,
//!    computed from checkpoints and headroom rather than from the region tree, so a bug that
//!    fooled the region layer's own two accountings still has this one to get past;
//! 4. **both-or-neither, on every arm** — a committed-evidence arm has committed evidence
//!    and a continuation naming the same frontier; a nothing-published arm has committed
//!    nothing. RFC 0026's rule as a property of every worker of every run, rather than of the
//!    rows somebody remembered.
//!
//! # The sweeps, and what each one adds
//!
//! | Sweep | What varies | Runs |
//! |---|---|---|
//! | [`the_cancellation_matrix_is_the_declared_table_at_every_phase_of_every_lane`] | lane × phase × placement × arrival | 27 × 4 × 3 = 324 |
//! | [`the_request_window_holds_the_pass_condition_while_work_races_the_drain`] | lane × phase × placement, work continuing after the request | 27 × 4 = 108 |
//! | [`no_work_enters_a_lane_after_its_cancellation_was_requested`] | lane × phase | 27 |
//! | [`a_four_lane_portfolio_is_total_at_every_interleaving_and_every_injection`] | interleaving × injection position | 630 × 8 = 5040 |
//!
//! # House rules, inherited unchanged
//!
//! From `tests/region_no_orphan.rs`, which inherited them from the G0-DX-13 campaign:
//!
//! - **assertions are on outcomes, never on interleavings**;
//! - **every sweep carries an anti-vacuity guard** — a loop that ran zero times, a render
//!   that came out empty, or a space every member of which produced one outcome fails the
//!   test rather than passing it;
//! - **the report is never the only witness** — the tree is re-read after every teardown;
//! - **adversarial inputs are built linearly** — every program in this file is a fixed-length
//!   array of beats, and the one enumerated space is a shuffle product of four programs of
//!   2, 2, 2 and 1 beats.
//!
//! # Evidence map
//!
//! | Claim | Test |
//! |---|---|
//! | every phase of every lane produces its declared typed outcome | [`the_cancellation_matrix_is_the_declared_table_at_every_phase_of_every_lane`] |
//! | the outcome is a function of the phase, not of how far the teardown got first | [`the_cancellation_matrix_is_the_declared_table_at_every_phase_of_every_lane`] |
//! | the pass condition survives work that keeps stepping after the request | [`the_request_window_holds_the_pass_condition_while_work_races_the_drain`] |
//! | a cancelled lane admits no further work or sub-scope | [`no_work_enters_a_lane_after_its_cancellation_was_requested`] |
//! | four lanes at once, at every interleaving and every injection point | [`a_four_lane_portfolio_is_total_at_every_interleaving_and_every_injection`] |
//! | a staged artifact really is observed staged, and really is gone afterwards | [`a_staged_artifact_is_observed_in_flight_and_absent_after_the_drain`] |
//! | the ledger conjunct is load-bearing | [`finalizing_a_lane_before_the_drain_is_refused_and_names_the_worker`] |
//! | the budget conjunct is load-bearing | [`headroom_a_cancellation_did_not_release_is_caught_by_the_third_accounting`] |
//! | a lane may not park mid-publication | [`a_lane_cannot_park_while_one_of_its_artifacts_is_in_flight`] |
//! | every arm of the cancellation table is reached by the matrix | [`the_matrix_reaches_every_arm_of_the_cancellation_table`] |
//! | one matrix renders byte-identically on every run | [`the_whole_matrix_renders_byte_identically_across_two_runs`] |

use std::collections::BTreeSet;

use continuum_task::budget::dimension::{CostDimension, MeterSet};
use continuum_task::budget::partial::{Disagreement, EvidenceBook, TerminationCause};
use continuum_task::budget::{Budget, BudgetLedger};
use continuum_task::region::schedule::{Schedule, Step, interleavings};
use continuum_task::region::worker::{
    CancelOutcome, FailureReason, NonResumableReason, Resumability, WorkerId, WorkerState,
    WorkerStep,
};
use continuum_task::region::{Finalization, RegionFault, RegionId, RegionTree, WorkerReport};

// --- the vocabulary the lanes are written in -----------------------------------------------

/// The dimension a publication's headroom is reserved against — RFC 0027's *enforced*
/// context contract, and the one dimension a reservation is genuinely for.
const BYTES: CostDimension = CostDimension::Bytes;

/// Headroom held for one staged artifact, the bytes actually written when it commits, and
/// the lane-dimension spend one publication's worth of work costs.
///
/// Three constants rather than one so that a settle refunds something and a release refunds
/// everything, which is what makes the two paths distinguishable in the ledger.
const RESERVED: u64 = 10;
const WRITTEN: u64 = 6;
const WORK: u64 = 5;

/// Ceilings high enough that no lane in this file exhausts one by accident: exhaustion is a
/// *declared* beat ([`Beat::Break`]), never an artifact of the fixture's arithmetic.
const WORK_CEILING: u64 = 1_000;
const BYTES_CEILING: u64 = 1_000;

/// A ceiling every lane declares and no lane meters, so every partial result this file
/// produces carries the INV-007 omission that says so.
const WALL_MS_CEILING: u64 = 60_000;

/// The reason token a [`Beat::Break`] carries.
///
/// The daemon's `ErrorCode` spelling, because
/// [`FailureReason`] is a carrier for that taxonomy rather than a second vocabulary
/// (`crates/continuum-task/src/region/worker.rs`).
const EXHAUSTED: &str = "BudgetExhausted";

/// One beat of a lane's instrumented lifecycle — the six things the region calculus can
/// observe about work, plus the one way work ends badly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Beat {
    /// The engine started: `Created → Running`.
    Begin,
    /// An artifact was staged. Nothing a reader can observe exists yet, and this is the one
    /// window in which "neither committed nor absent" is a state the work is genuinely in.
    Stage,
    /// The staged artifact became durable and observable, and monotone thereafter (INV-009).
    Publish,
    /// The lane parked with committed partial evidence and a continuation (B18).
    Park,
    /// The lane took its continuation back up.
    Wake,
    /// The lane finished its work.
    Close,
    /// The lane ran out of a declared ceiling.
    Break,
}

impl Beat {
    /// The region-layer step this beat is.
    fn step(self) -> WorkerStep {
        match self {
            Self::Begin => WorkerStep::Begin,
            Self::Stage => WorkerStep::Reserve,
            Self::Publish => WorkerStep::Commit,
            Self::Park => WorkerStep::Suspend,
            Self::Wake => WorkerStep::Resume,
            Self::Close => WorkerStep::Complete,
            Self::Break => WorkerStep::Fail(
                FailureReason::new(EXHAUSTED).expect("the wire code is a canonical token"),
            ),
        }
    }
}

/// The terminal state a phase's teardown must leave the lane's worker in.
///
/// Three members rather than a [`WorkerState`] so the table below reads as a table: a
/// [`WorkerState::Failed`] carries a payload, and spelling it out in twenty-seven rows would
/// bury the one column that varies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ends {
    /// The cancellation terminated it.
    Cancelled,
    /// It had already finished, and RFC 0026's monotonicity leaves that alone.
    Completed,
    /// It had already failed, and keeps its own reason.
    Broken,
}

impl Ends {
    fn state(self) -> WorkerState {
        match self {
            Self::Cancelled => WorkerState::Cancelled,
            Self::Completed => WorkerState::Completed,
            Self::Broken => WorkerState::Failed(
                FailureReason::new(EXHAUSTED).expect("the wire code is a canonical token"),
            ),
        }
    }
}

/// One row of the matrix: what a cancellation arriving at this phase must leave behind.
///
/// Written down rather than derived. A derived expectation is a second implementation of the
/// rule under test, and two implementations that agree prove that they agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PhaseRow {
    /// What the lane was doing when the cancellation reached it.
    phase: &'static str,
    /// The terminal state the worker must be in afterwards.
    ends: Ends,
    /// The [`CancelOutcome`] token, or [`None`] for work that had already terminated on its
    /// own — those are not cancellation outcomes, and reporting one would invent a
    /// continuation question nobody asked.
    arm: Option<&'static str>,
    /// How many publications must still be durable afterwards (INV-009's monotonicity, read
    /// at the far end of a teardown).
    committed: u32,
}

/// One task shape: a publication/suspension profile, the dimension its work is metered on,
/// and the table of what a cancellation at each of its phases must leave.
#[derive(Debug)]
struct Lane {
    /// The engine lane this profile stands for.
    token: &'static str,
    /// The SD-12 dimension this lane's own work is metered on
    /// (`docs/25_SEMANTIC_FEATURE_MATRIX.md`).
    dimension: CostDimension,
    /// Whether the lane declares a continuation at admission. RFC 0026 has both cases and
    /// requires the second to name itself.
    resumable: bool,
    /// The typed reason a non-resumable lane carries — RFC 0026's `non_resumable_reason`.
    non_resumable_reason: &'static str,
    /// The lane's instrumented lifecycle, in order.
    beats: &'static [Beat],
    /// One row per phase. Phase `k` is "the cancellation arrived after the first `k` beats",
    /// so there are `beats.len() + 1` of them and the last is the lane at rest.
    rows: &'static [PhaseRow],
}

impl Lane {
    /// How this lane declares its resumability at [`RegionTree::spawn`].
    fn resumability(&self) -> Resumability {
        if self.resumable {
            Resumability::Resumable
        } else {
            Resumability::NonResumable(
                NonResumableReason::new(self.non_resumable_reason)
                    .expect("the lane reasons are canonical tokens"),
            )
        }
    }

    /// The ceilings a caller declared for this lane: its own dimension, the publication
    /// byte contract, and one ceiling nothing meters.
    fn budget(&self) -> Budget {
        Budget::unbounded()
            .with(self.dimension, WORK_CEILING)
            .with(BYTES, BYTES_CEILING)
            .with(CostDimension::WallMs, WALL_MS_CEILING)
    }

    /// What this lane's accounting can actually measure.
    fn meters(&self) -> MeterSet {
        MeterSet::none().with(self.dimension).with(BYTES)
    }
}

/// The four task shapes G0-DX-14 names, as publication/suspension profiles.
///
/// Read the `beats` of each one as the answer to "what does cancellation have to be correct
/// *across* for this lane", and the `rows` as the answer to "and what does correct mean at
/// each point".
const LANES: &[Lane] = &[
    // A DPOR/explicit-state exploration is metered on `states`
    // (`docs/25_SEMANTIC_FEATURE_MATRIX.md`) and commits a frontier per round, so the shape
    // is *many* publications and a park at the bound: `bfs::Exploration::Exhausted` carries
    // the queue-ordered frontier, which is a resume point by construction, so the lane is
    // resumable.
    Lane {
        token: "dpor",
        dimension: CostDimension::States,
        resumable: true,
        non_resumable_reason: "",
        beats: &[
            Beat::Begin,
            Beat::Stage,
            Beat::Publish,
            Beat::Stage,
            Beat::Publish,
            Beat::Park,
        ],
        rows: &[
            PhaseRow {
                phase: "admitted",
                ends: Ends::Cancelled,
                arm: Some("nothing-published"),
                committed: 0,
            },
            PhaseRow {
                phase: "exploring",
                ends: Ends::Cancelled,
                arm: Some("nothing-published"),
                committed: 0,
            },
            PhaseRow {
                phase: "frontier-staged",
                ends: Ends::Cancelled,
                arm: Some("nothing-published"),
                committed: 0,
            },
            PhaseRow {
                phase: "frontier-committed",
                ends: Ends::Cancelled,
                arm: Some("committed-with-continuation"),
                committed: 1,
            },
            PhaseRow {
                phase: "next-frontier-staged",
                ends: Ends::Cancelled,
                arm: Some("committed-with-continuation"),
                committed: 1,
            },
            PhaseRow {
                phase: "next-frontier-committed",
                ends: Ends::Cancelled,
                arm: Some("committed-with-continuation"),
                committed: 2,
            },
            PhaseRow {
                phase: "parked-at-frontier",
                ends: Ends::Cancelled,
                arm: Some("committed-with-continuation"),
                committed: 2,
            },
        ],
    },
    // An SMT/PDR query is metered on `solver_ms` and is *atomic*: the answer exists or it
    // does not, there is no half of one, and the incremental solver state is not an artifact
    // this daemon can export — so the lane declares itself non-resumable and RFC 0026
    // requires it to name why. Its shape is therefore one long staged publication and a
    // single commit, and the row that matters is `answer-staged`: a query cancelled with an
    // answer in flight must publish *nothing*.
    Lane {
        token: "solver",
        dimension: CostDimension::SolverMs,
        resumable: false,
        non_resumable_reason: "solver-state-not-exportable",
        beats: &[Beat::Begin, Beat::Stage, Beat::Publish, Beat::Close],
        rows: &[
            PhaseRow {
                phase: "admitted",
                ends: Ends::Cancelled,
                arm: Some("nothing-published"),
                committed: 0,
            },
            PhaseRow {
                phase: "querying",
                ends: Ends::Cancelled,
                arm: Some("nothing-published"),
                committed: 0,
            },
            PhaseRow {
                phase: "answer-staged",
                ends: Ends::Cancelled,
                arm: Some("nothing-published"),
                committed: 0,
            },
            PhaseRow {
                phase: "answer-committed",
                ends: Ends::Cancelled,
                arm: Some("committed-non-resumable"),
                committed: 1,
            },
            PhaseRow {
                phase: "answered",
                ends: Ends::Completed,
                arm: None,
                committed: 1,
            },
        ],
    },
    // A proof lane is metered on `proof_ms` and publishes lemma by lemma, parking *between*
    // lemmas rather than at a bound — the only lane whose profile puts a park in the middle
    // and a wake after it, which is what makes `Suspended → Running → Suspended` reachable at
    // all.
    Lane {
        token: "proof",
        dimension: CostDimension::ProofMs,
        resumable: true,
        non_resumable_reason: "",
        beats: &[
            Beat::Begin,
            Beat::Stage,
            Beat::Publish,
            Beat::Park,
            Beat::Wake,
            Beat::Stage,
            Beat::Publish,
            Beat::Close,
        ],
        rows: &[
            PhaseRow {
                phase: "admitted",
                ends: Ends::Cancelled,
                arm: Some("nothing-published"),
                committed: 0,
            },
            PhaseRow {
                phase: "searching",
                ends: Ends::Cancelled,
                arm: Some("nothing-published"),
                committed: 0,
            },
            PhaseRow {
                phase: "lemma-staged",
                ends: Ends::Cancelled,
                arm: Some("nothing-published"),
                committed: 0,
            },
            PhaseRow {
                phase: "lemma-committed",
                ends: Ends::Cancelled,
                arm: Some("committed-with-continuation"),
                committed: 1,
            },
            PhaseRow {
                phase: "parked-between-lemmas",
                ends: Ends::Cancelled,
                arm: Some("committed-with-continuation"),
                committed: 1,
            },
            PhaseRow {
                phase: "resumed",
                ends: Ends::Cancelled,
                arm: Some("committed-with-continuation"),
                committed: 1,
            },
            PhaseRow {
                phase: "next-lemma-staged",
                ends: Ends::Cancelled,
                arm: Some("committed-with-continuation"),
                committed: 1,
            },
            PhaseRow {
                phase: "next-lemma-committed",
                ends: Ends::Cancelled,
                arm: Some("committed-with-continuation"),
                committed: 2,
            },
            PhaseRow {
                phase: "checked",
                ends: Ends::Completed,
                arm: None,
                committed: 2,
            },
        ],
    },
    // A synthesis lane is metered on `candidates`. A rejected candidate is not a publication
    // that was abandoned — it never became one, because `Stage` is the point at which an
    // artifact starts existing — so the churn shows up as spend rather than as staged
    // artifacts, and the shape ends the way a CEGIS loop ends: a ceiling runs out with one
    // candidate durable and the next one still in flight. That last beat is the only place in
    // this file where a *failure* rather than a cancellation resolves a staged artifact.
    Lane {
        token: "synthesis",
        dimension: CostDimension::Candidates,
        resumable: true,
        non_resumable_reason: "",
        beats: &[
            Beat::Begin,
            Beat::Stage,
            Beat::Publish,
            Beat::Stage,
            Beat::Break,
        ],
        rows: &[
            PhaseRow {
                phase: "admitted",
                ends: Ends::Cancelled,
                arm: Some("nothing-published"),
                committed: 0,
            },
            PhaseRow {
                phase: "enumerating",
                ends: Ends::Cancelled,
                arm: Some("nothing-published"),
                committed: 0,
            },
            PhaseRow {
                phase: "candidate-staged",
                ends: Ends::Cancelled,
                arm: Some("nothing-published"),
                committed: 0,
            },
            PhaseRow {
                phase: "candidate-committed",
                ends: Ends::Cancelled,
                arm: Some("committed-with-continuation"),
                committed: 1,
            },
            PhaseRow {
                phase: "next-candidate-staged",
                ends: Ends::Cancelled,
                arm: Some("committed-with-continuation"),
                committed: 1,
            },
            PhaseRow {
                phase: "exhausted",
                ends: Ends::Broken,
                arm: None,
                committed: 1,
            },
        ],
    },
];

// --- where the work sits, and how far the teardown gets ------------------------------------

/// Where the lane's worker sits, and which region the cancellation names.
///
/// Cancellation is subtree-wide, so "which region was named" is a genuinely separate axis
/// from "which phase was the work in": a cancellation that has to cross two region boundaries
/// to reach the work is the case a shallow teardown gets wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Placement {
    /// The worker is in the root, and the root is cancelled.
    Root,
    /// The worker is in a child region, and the *root* is cancelled — one boundary crossed.
    ChildFromRoot,
    /// The worker is in a child region, and the *child* is cancelled; the root is torn down
    /// afterwards.
    ChildDirect,
    /// The worker is three levels down, and the root is cancelled — two boundaries crossed.
    GrandchildFromRoot,
}

impl Placement {
    const ALL: &'static [Self] = &[
        Self::Root,
        Self::ChildFromRoot,
        Self::ChildDirect,
        Self::GrandchildFromRoot,
    ];

    fn token(self) -> &'static str {
        match self {
            Self::Root => "root",
            Self::ChildFromRoot => "child-from-root",
            Self::ChildDirect => "child-direct",
            Self::GrandchildFromRoot => "grandchild-from-root",
        }
    }

    /// How deep the worker's own region sits below the root.
    fn depth(self) -> u32 {
        match self {
            Self::Root => 0,
            Self::ChildFromRoot | Self::ChildDirect => 1,
            Self::GrandchildFromRoot => 2,
        }
    }

    /// Whether the cancellation names the worker's own region rather than the root.
    fn names_home(self) -> bool {
        matches!(self, Self::ChildDirect)
    }
}

/// How far the three-step teardown gets before the run is finished off.
///
/// RFC 0026 names three operations and a daemon has to be able to observe each one, so a
/// cancellation that stopped after the *request* and a cancellation that ran all three are
/// two different moments a client can be in. All three must produce the same row, and that
/// equality is itself an assertion below.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Arrival {
    /// The request only. The drain and the finalize come from the closing teardown.
    Request,
    /// Request and drain: the work is terminal before anything else happens.
    Drain,
    /// All three, in place.
    Teardown,
}

impl Arrival {
    const ALL: &'static [Self] = &[Self::Request, Self::Drain, Self::Teardown];

    fn token(self) -> &'static str {
        match self {
            Self::Request => "request",
            Self::Drain => "drain",
            Self::Teardown => "teardown",
        }
    }
}

// --- one run -------------------------------------------------------------------------------

/// One cancellation, and everything three accountings say about it.
struct Run {
    tree: RegionTree,
    book: EvidenceBook,
    finalization: Finalization,
}

impl Run {
    /// The report for one worker. Every worker the tree admitted is reported, so this cannot
    /// miss one silently.
    fn report(&self, worker: WorkerId) -> &WorkerReport {
        self.finalization
            .workers()
            .iter()
            .find(|report| report.worker() == worker)
            .expect("every admitted worker appears in the report")
    }
}

/// Bring one worker's budget account into agreement with what the region tree now says.
///
/// The mirror `tests/budget_partial_evidence.rs` introduced, per lane: it reads the tree and
/// never writes to it, so it cannot make the two layers agree by moving either one, and it
/// never sees the drain — a cancellation that discards a staged artifact looks to it exactly
/// like any other resolution.
fn mirror(lane: &Lane, tree: &RegionTree, worker: WorkerId, book: &mut EvidenceBook) {
    let evidence = tree.evidence(worker).expect("admitted workers exist");
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
        ledger.charge(lane.dimension, WORK).expect("metered");
        ledger
            .checkpoint(priced)
            .expect("nothing reserved, and the count only grows");
    }
}

/// Open `depth` nested regions below the root and return the innermost one.
fn nest(tree: &mut RegionTree, depth: u32) -> RegionId {
    let mut region = tree.root();
    for _ in 0..depth {
        region = tree.open_child(region).expect("each region is open");
    }
    region
}

/// Admit `lane`'s worker at `placement`, run its first `phase` beats, and mirror the budget
/// accounting after every one.
fn wound_up(
    lane: &Lane,
    phase: usize,
    placement: Placement,
) -> (RegionTree, EvidenceBook, WorkerId) {
    let mut tree = RegionTree::new();
    let home = nest(&mut tree, placement.depth());
    let worker = tree
        .spawn(home, lane.resumability())
        .expect("the home region is open");

    let mut book = EvidenceBook::new();
    book.admit(worker, lane.budget(), lane.meters())
        .expect("one account per worker");
    mirror(lane, &tree, worker, &mut book);

    for beat in &lane.beats[..phase] {
        tree.advance(worker, beat.step())
            .unwrap_or_else(|fault| panic!("{}: beat {beat:?} is not legal — {fault}", lane.token));
        mirror(lane, &tree, worker, &mut book);
    }
    (tree, book, worker)
}

/// The region the cancellation names, for one placement.
fn cancel_target(tree: &RegionTree, placement: Placement, worker: WorkerId) -> RegionId {
    if placement.names_home() {
        tree.owner(worker).expect("the worker was admitted")
    } else {
        tree.root()
    }
}

/// Cancel `lane`'s worker at `phase`, as far as `arrival` goes, then finish the teardown.
///
/// The lane's remaining beats are deliberately *not* run: this sweep is about the moment the
/// cancellation arrives, and [`request_window`] is about the window in which work keeps
/// stepping past it.
fn cancelled_at(lane: &Lane, phase: usize, placement: Placement, arrival: Arrival) -> Run {
    let (mut tree, mut book, worker) = wound_up(lane, phase, placement);
    let root = tree.root();
    let target = cancel_target(&tree, placement, worker);

    let midway = match arrival {
        Arrival::Request => {
            tree.cancel(target).expect("the region is not finalized");
            None
        }
        Arrival::Drain => {
            tree.cancel(target).expect("the region is not finalized");
            tree.drain(target).expect("a cancellation drain is total");
            None
        }
        Arrival::Teardown => Some(tree.teardown(target).expect("the region exists")),
    };
    mirror(lane, &tree, worker, &mut book);

    let finalization = match midway {
        Some(report) if target == root => report,
        _ => tree.teardown(root).expect("the root is not finalized"),
    };
    mirror(lane, &tree, worker, &mut book);
    Run {
        tree,
        book,
        finalization,
    }
}

/// Request cancellation at `phase` and then let the lane keep stepping through the rest of
/// its beats, refusals and all, before the teardown closes it out.
fn request_window(lane: &Lane, phase: usize, placement: Placement) -> Run {
    let (mut tree, mut book, worker) = wound_up(lane, phase, placement);
    let root = tree.root();
    let target = cancel_target(&tree, placement, worker);
    tree.cancel(target).expect("the region is not finalized");

    for beat in &lane.beats[phase..] {
        // A refused beat changes nothing — that is the entire value of a typed refusal — so
        // the run continues past it, exactly as `Schedule::run` does.
        let _ = tree.advance(worker, beat.step());
        mirror(lane, &tree, worker, &mut book);
    }
    let finalization = tree.teardown(root).expect("the root is not finalized");
    mirror(lane, &tree, worker, &mut book);
    Run {
        tree,
        book,
        finalization,
    }
}

// --- the pass condition, checked four ways -------------------------------------------------

/// Both conjuncts of G0-DX-14's pass condition, plus the two cross-checks that make agreement
/// evidence rather than a tautology.
///
/// Called on **every** run in this file. A sweep that checked the interesting conjunct at the
/// interesting row would be evidence about that row.
fn assert_pass_condition(run: &Run, context: &str) {
    // --- conjunct 1: no leaked obligations -------------------------------------------------
    assert!(
        run.finalization.ledger().is_balanced(),
        "{context}: leaked an obligation\n{}",
        run.finalization.render()
    );
    assert!(
        run.tree.ledger().is_balanced(),
        "{context}: the tree's own ledger disagrees with the report it handed back"
    );
    assert_eq!(
        run.tree.ledger().opened(),
        run.tree.ledger().discharged(),
        "{context}: the counters are the ledger's independent accounting and must agree"
    );
    assert!(
        run.finalization.ledger().opened() > 0,
        "{context}: a ledger that never opened anything proves nothing"
    );

    // --- conjunct 2: every artifact committed or absent -------------------------------------
    assert!(
        run.finalization.unresolved_publications().is_empty(),
        "{context}: artifacts left neither committed nor absent: {:?}",
        run.finalization.unresolved_publications()
    );
    assert!(
        run.finalization.orphans().is_empty(),
        "{context}: orphaned workers: {:?}",
        run.finalization.orphans()
    );
    assert!(
        run.finalization.is_total(),
        "{context}: the teardown was not total\n{}",
        run.finalization.render()
    );

    // Re-derived from the tree rather than from the report, because a report that agreed
    // with itself would prove only that.
    let admitted = u32::try_from(run.tree.worker_count()).expect("small trees");
    assert!(
        admitted > 0,
        "{context}: a tree with no workers proves nothing"
    );
    for ordinal in 0..admitted {
        let worker = WorkerId::at(ordinal);
        let state = run
            .tree
            .worker_state(worker)
            .expect("admitted workers exist");
        assert!(
            state.is_terminal(),
            "{context}: {worker} is {state}, which is not terminal"
        );
        assert!(
            !run.tree
                .evidence(worker)
                .expect("admitted workers exist")
                .is_provisional(),
            "{context}: {worker} still holds a staged artifact"
        );
    }
    assert_eq!(
        run.finalization.workers().len(),
        run.tree.worker_count(),
        "{context}: the report and the tree disagree on how many workers there were"
    );

    // --- the third accounting: the budget book ---------------------------------------------
    let reconciliation = run.book.reconcile(&run.finalization);
    assert!(
        reconciliation.is_reconciled(),
        "{context}: the budget accounting and the region layer disagree\n{}",
        reconciliation.render()
    );

    // --- both-or-neither, on every arm ------------------------------------------------------
    for report in run.finalization.workers() {
        let committed = report.evidence().committed();
        match report.cancel_outcome() {
            Some(CancelOutcome::CommittedWithContinuation(continuation)) => {
                assert!(
                    committed > 0,
                    "{context}: {} carries a continuation with nothing committed",
                    report.worker()
                );
                assert_eq!(
                    continuation.committed(),
                    committed,
                    "{context}: {}'s continuation names a frontier the evidence does not",
                    report.worker()
                );
                assert_eq!(
                    continuation.worker(),
                    report.worker(),
                    "{context}: a continuation must name the work it resumes"
                );
            }
            Some(CancelOutcome::CommittedNonResumable(_)) => assert!(
                committed > 0,
                "{context}: {} reports committed evidence it does not hold",
                report.worker()
            ),
            Some(CancelOutcome::NothingPublished) => assert_eq!(
                committed,
                0,
                "{context}: {} published something and says it published nothing",
                report.worker()
            ),
            None => assert!(
                report.state().is_terminal(),
                "{context}: {} has no cancellation outcome and is not terminal",
                report.worker()
            ),
        }
    }
}

// --- sweep 1: the matrix -------------------------------------------------------------------

#[test]
fn the_cancellation_matrix_is_the_declared_table_at_every_phase_of_every_lane() {
    let mut runs = 0_usize;
    let mut rows = 0_usize;
    for lane in LANES {
        assert_eq!(
            lane.rows.len(),
            lane.beats.len() + 1,
            "{}: one row per phase, and one more for the lane at rest",
            lane.token
        );
        for (phase, row) in lane.rows.iter().enumerate() {
            rows += 1;
            // The three arrivals must agree with each other as well as with the table: the
            // outcome is a function of the phase, not of how far the teardown got before the
            // report was read.
            let mut arms: BTreeSet<String> = BTreeSet::new();
            for placement in Placement::ALL {
                for arrival in Arrival::ALL {
                    let context = format!(
                        "{} / {} / {} / {}",
                        lane.token,
                        row.phase,
                        placement.token(),
                        arrival.token()
                    );
                    let run = cancelled_at(lane, phase, *placement, *arrival);
                    assert_pass_condition(&run, &context);

                    let worker = WorkerId::at(0);
                    let report = run.report(worker);
                    assert_eq!(
                        report.state(),
                        &row.ends.state(),
                        "{context}: wrong terminal state\n{}",
                        run.finalization.render()
                    );
                    assert_eq!(
                        report.cancel_outcome().map(CancelOutcome::token),
                        row.arm,
                        "{context}: wrong cancellation arm\n{}",
                        run.finalization.render()
                    );
                    assert_eq!(
                        report.evidence().committed(),
                        row.committed,
                        "{context}: committed evidence is not what the phase left\n{}",
                        run.finalization.render()
                    );

                    // The budget layer's own reading of the same phase.
                    let partial = run.book.partial_evidence(worker, report.state());
                    match row.ends {
                        Ends::Completed => assert!(
                            partial.is_none(),
                            "{context}: a completed lane has evidence, not *partial* evidence"
                        ),
                        Ends::Cancelled | Ends::Broken => {
                            let partial = partial.expect(
                                "a cancelled or failed lane with an account has partial evidence",
                            );
                            assert_eq!(
                                partial.committed(),
                                row.committed,
                                "{context}: the budget layer names a different frontier"
                            );
                            assert_eq!(
                                partial
                                    .omissions()
                                    .iter()
                                    .map(|o| o.subject())
                                    .collect::<Vec<_>>(),
                                vec!["budget.wall_ms".to_owned()],
                                "{context}: the declared, unmetered ceiling travels with the evidence"
                            );
                            if row.ends == Ends::Cancelled {
                                assert_eq!(
                                    partial.cause(),
                                    &TerminationCause::Cancelled,
                                    "{context}: the cause is read off the region layer"
                                );
                            }
                        }
                    }

                    arms.insert(format!(
                        "{}|{}|{}",
                        report.state(),
                        report.cancel_outcome().map_or("-", CancelOutcome::token),
                        report.evidence().committed()
                    ));
                    runs += 1;
                }
            }
            assert_eq!(
                arms.len(),
                1,
                "{}/{}: the outcome depends on how the teardown was driven, not only on the \
                 phase: {arms:?}",
                lane.token,
                row.phase
            );
        }
    }
    assert_eq!(
        rows, 27,
        "the matrix is twenty-seven phases across four lanes"
    );
    assert_eq!(
        runs,
        rows * Placement::ALL.len() * Arrival::ALL.len(),
        "every phase must have been swept at every placement and every arrival"
    );
    assert_eq!(runs, 324);
}

// --- sweep 2: the request window -----------------------------------------------------------

#[test]
fn the_request_window_holds_the_pass_condition_while_work_races_the_drain() {
    // The window an adversarial schedule lives in: the cancellation has been *requested* and
    // the work has not observed it yet, so every remaining beat is still attempted. Some are
    // legal — advancing inside a draining region is deliberate — and the rest are refused.
    // The row is not fixed here, because which beats got in first is exactly what varies; the
    // pass condition is.
    let mut runs = 0_usize;
    let mut committed_beyond_the_row = 0_usize;
    let mut outcomes: BTreeSet<String> = BTreeSet::new();
    for lane in LANES {
        for (phase, row) in lane.rows.iter().enumerate() {
            for placement in Placement::ALL {
                let context = format!(
                    "{} / {} / {} / request-window",
                    lane.token,
                    row.phase,
                    placement.token()
                );
                let run = request_window(lane, phase, *placement);
                assert_pass_condition(&run, &context);

                let report = run.report(WorkerId::at(0));
                assert!(
                    report.evidence().committed() >= row.committed,
                    "{context}: committed evidence is monotone, so it cannot fall below the \
                     phase's own frontier"
                );
                if report.evidence().committed() > row.committed {
                    committed_beyond_the_row += 1;
                }
                outcomes.insert(format!(
                    "{}|{}|{}|{}",
                    lane.token,
                    report.state(),
                    report.cancel_outcome().map_or("-", CancelOutcome::token),
                    report.evidence().committed()
                ));
                runs += 1;
            }
        }
    }
    assert_eq!(runs, 27 * Placement::ALL.len());
    assert_eq!(runs, 108);
    assert!(
        committed_beyond_the_row > 0,
        "no run ever got a beat in after the request, so this sweep tested the same window as \
         the matrix"
    );
    assert!(
        outcomes.len() > 1,
        "every request-window run produced one outcome, so the sweep exercised one behaviour"
    );
}

#[test]
fn no_work_enters_a_lane_after_its_cancellation_was_requested() {
    // The rule that bounds the set a teardown has to account for: after the request, the set
    // can never grow again. Without it, "every worker the subtree ever admitted is terminal"
    // would be a claim about a moving target.
    let mut refusals = 0_usize;
    for lane in LANES {
        for (phase, row) in lane.rows.iter().enumerate() {
            let (mut tree, _, worker) = wound_up(lane, phase, Placement::ChildFromRoot);
            let home = tree.owner(worker).expect("the worker was admitted");
            let root = tree.root();
            tree.cancel(root).expect("the root is not finalized");

            let context = format!("{} / {}", lane.token, row.phase);
            let admitted = tree.worker_count();
            assert!(
                matches!(
                    tree.spawn(home, lane.resumability()),
                    Err(RegionFault::SpawnIntoClosedRegion { .. })
                ),
                "{context}: a cancelled region admitted a new worker"
            );
            assert!(
                matches!(
                    tree.open_child(home),
                    Err(RegionFault::OpenChildInClosedRegion { .. })
                ),
                "{context}: a cancelled region opened a new sub-scope"
            );
            assert_eq!(
                tree.worker_count(),
                admitted,
                "{context}: a refused admission must admit nothing at all"
            );
            refusals += 2;

            // And the refusals did not corrupt the teardown.
            let finalization = tree.teardown(root).expect("the root is not finalized");
            assert!(
                finalization.is_total(),
                "{context}: a refused admission corrupted the teardown\n{}",
                finalization.render()
            );
        }
    }
    assert_eq!(refusals, 27 * 2, "every phase must have been swept");
}

// --- sweep 3: four lanes at once -----------------------------------------------------------

/// The portfolio: one worker per lane, spread across a three-level region tree.
///
/// `dpor` and `solver` sit in the root, `proof` in a child, `synthesis` in a grandchild, so a
/// cancellation from the root has to cross two boundaries to reach all four.
fn portfolio() -> (RegionTree, EvidenceBook, Vec<WorkerId>) {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let child = tree.open_child(root).expect("the root is open");
    let grandchild = tree.open_child(child).expect("the child is open");
    let homes = [root, root, child, grandchild];

    let mut book = EvidenceBook::new();
    let mut workers = Vec::with_capacity(LANES.len());
    for (lane, home) in LANES.iter().zip(homes) {
        let worker = tree
            .spawn(home, lane.resumability())
            .expect("every home region is open");
        book.admit(worker, lane.budget(), lane.meters())
            .expect("one account per worker");
        workers.push(worker);
    }
    (tree, book, workers)
}

/// Mirror every lane's budget account against the tree.
fn mirror_all(tree: &RegionTree, book: &mut EvidenceBook, workers: &[WorkerId]) {
    for (lane, worker) in LANES.iter().zip(workers) {
        mirror(lane, tree, *worker, book);
    }
}

/// The prefix each lane has already run when the interleaved suffixes start.
///
/// Distinct per lane on purpose: four lanes starting from one state would make the shuffle
/// product a sweep over one shape wearing four names.
const PRELUDES: [&[Beat]; 4] = [
    // dpor: one frontier already durable.
    &[Beat::Begin, Beat::Stage, Beat::Publish],
    // solver: the query is running and nothing is staged.
    &[Beat::Begin],
    // proof: one lemma durable and parked between lemmas.
    &[Beat::Begin, Beat::Stage, Beat::Publish, Beat::Park],
    // synthesis: one candidate durable.
    &[Beat::Begin, Beat::Stage, Beat::Publish],
];

/// The suffixes the sweep interleaves — 2, 2, 2 and 1 beats, so the shuffle product has
/// `7! / (2! · 2! · 2! · 1!) = 630` members.
const SUFFIXES: [&[Beat]; 4] = [
    &[Beat::Stage, Beat::Publish],
    &[Beat::Stage, Beat::Publish],
    &[Beat::Wake, Beat::Stage],
    &[Beat::Stage],
];

#[test]
fn a_four_lane_portfolio_is_total_at_every_interleaving_and_every_injection() {
    let programs: Vec<Vec<Step>> = SUFFIXES
        .iter()
        .enumerate()
        .map(|(index, suffix)| {
            let worker = WorkerId::at(u32::try_from(index).expect("four lanes"));
            suffix
                .iter()
                .map(|beat| Step::advance(worker, beat.step()))
                .collect()
        })
        .collect();
    let schedules = interleavings(&programs);
    assert_eq!(
        schedules.len(),
        630,
        "the shuffle product of 2+2+2+1 beats: 7! / (2! * 2! * 2! * 1!)"
    );

    let mut runs = 0_usize;
    let mut arms: BTreeSet<&'static str> = BTreeSet::new();
    let mut renders: BTreeSet<String> = BTreeSet::new();
    for (index, schedule) in schedules.iter().enumerate() {
        for position in 0..=schedule.len() {
            let (mut tree, mut book, workers) = portfolio();
            let root = tree.root();
            for (lane, (prelude, worker)) in LANES.iter().zip(PRELUDES.iter().zip(&workers)) {
                for beat in *prelude {
                    tree.advance(*worker, beat.step())
                        .unwrap_or_else(|fault| panic!("{}: prelude — {fault}", lane.token));
                }
                mirror(lane, &tree, *worker, &mut book);
            }

            // The cancellation *completes* mid-schedule, so every beat after the injection
            // lands on a worker that is already terminal and is refused. Which beats got in
            // first is therefore what distinguishes one interleaving from another.
            let mut steps: Vec<Step> = schedule.steps().to_vec();
            steps.splice(
                position..position,
                [Step::Cancel { region: root }, Step::Drain { region: root }],
            );
            for step in &steps {
                let _ = Schedule::new(vec![step.clone()]).run(&mut tree);
                mirror_all(&tree, &mut book, &workers);
            }

            let finalization = tree.teardown(root).expect("cancelling twice is legal");
            mirror_all(&tree, &mut book, &workers);
            let run = Run {
                tree,
                book,
                finalization,
            };
            let context =
                format!("portfolio schedule {index}, cancellation completed at {position}");
            assert_pass_condition(&run, &context);
            assert_eq!(
                run.finalization.workers().len(),
                LANES.len(),
                "{context}: every lane must be reported"
            );
            assert_eq!(
                run.finalization.regions().len(),
                3,
                "{context}: the whole three-level subtree must be finalized"
            );
            for report in run.finalization.workers() {
                if let Some(outcome) = report.cancel_outcome() {
                    arms.insert(outcome.token());
                }
            }
            renders.insert(run.finalization.render());
            runs += 1;
        }
    }
    assert_eq!(
        runs,
        schedules.len() * 8,
        "every position of every schedule"
    );
    assert_eq!(runs, 5_040);
    assert_eq!(
        arms,
        BTreeSet::from([
            "committed-non-resumable",
            "committed-with-continuation",
            "nothing-published",
        ]),
        "a portfolio sweep that never reached an arm proves nothing about it"
    );
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

// --- the guards are load-bearing -----------------------------------------------------------

#[test]
fn a_staged_artifact_is_observed_in_flight_and_absent_after_the_drain() {
    // The second conjunct is only meaningful if the forbidden state is genuinely reachable.
    // Here it is entered, observed in both accountings, and then observed gone — so "no
    // artifact was left neither committed nor absent" is a statement about a window the run
    // actually passed through.
    for lane in LANES {
        let staged = lane
            .beats
            .iter()
            .position(|beat| *beat == Beat::Stage)
            .expect("every lane stages an artifact");
        let phase = staged + 1;
        let (mut tree, mut book, worker) = wound_up(lane, phase, Placement::Root);

        assert!(
            tree.evidence(worker).expect("admitted").is_provisional(),
            "{}: the fixture did not actually stage anything",
            lane.token
        );
        assert!(
            tree.ledger().holds(
                continuum_task::region::obligation::Obligation::provisional_publication(worker)
            ),
            "{}: an artifact in flight owes a commit or a discard",
            lane.token
        );
        assert!(
            !tree.ledger().is_balanced(),
            "{}: the ledger must show the debt while the artifact is in flight",
            lane.token
        );
        assert_eq!(
            book.ledger(worker).expect("admitted").reservation(BYTES),
            Some(RESERVED),
            "{}: and the budget half must hold its headroom",
            lane.token
        );

        let root = tree.root();
        let finalization = tree.teardown(root).expect("the root is not finalized");
        mirror(lane, &tree, worker, &mut book);
        let run = Run {
            tree,
            book,
            finalization,
        };
        assert_pass_condition(&run, &format!("{} / staged-then-drained", lane.token));
        assert_eq!(
            run.report(worker).evidence().committed(),
            u32::try_from(
                lane.beats[..phase]
                    .iter()
                    .filter(|beat| **beat == Beat::Publish)
                    .count()
            )
            .expect("small programs"),
            "{}: the drain discarded the staged half and kept the committed half",
            lane.token
        );
        assert_eq!(
            run.book
                .ledger(worker)
                .expect("admitted")
                .reservation(BYTES),
            None,
            "{}: the discarded artifact's headroom came back",
            lane.token
        );
    }
}

#[test]
fn finalizing_a_lane_before_the_drain_is_refused_and_names_the_worker() {
    // The mutation this file exists to be sure of: if `finalize` did not check, "no leaked
    // obligations" would be an assertion about a code path rather than a post-condition.
    for lane in LANES {
        // The phase after `Begin`, which every lane has and in which no lane is terminal.
        let (mut tree, _, worker) = wound_up(lane, 1, Placement::Root);
        let root = tree.root();
        tree.cancel(root).expect("not finalized");

        let fault = tree
            .finalize(root)
            .expect_err("finalizing an undrained region must be refused");
        assert_eq!(
            fault,
            RegionFault::FinalizeBeforeTermination {
                region: root,
                worker,
                state: WorkerState::Running,
            },
            "{}: the refusal must name the worker that would have been orphaned",
            lane.token
        );
        assert!(
            !tree.ledger().is_balanced(),
            "{}: and the ledger still shows the debt",
            lane.token
        );

        tree.drain(root).expect("a cancellation drain is total");
        let finalization = tree.finalize(root).expect("drained");
        assert!(
            finalization.is_total(),
            "{}: draining first is what makes it succeed, so the refusal is a pre-condition \
             rather than a prohibition",
            lane.token
        );
    }
}

#[test]
fn headroom_a_cancellation_did_not_release_is_caught_by_the_third_accounting() {
    // The budget conjunct, shown to be able to fail. The region layer resolves its own half
    // — `unresolved_publications` is empty — and the book still catches the headroom, because
    // it is computed from the ledger rather than from the tree.
    for lane in LANES {
        let mut tree = RegionTree::new();
        let root = tree.root();
        let worker = tree
            .spawn(root, lane.resumability())
            .expect("the root is open");
        let mut book = EvidenceBook::new();
        book.admit(worker, lane.budget(), lane.meters())
            .expect("one account per worker");
        // Headroom held and never mirrored back: the failure mode `DanglingReservation`
        // exists to name.
        book.ledger_mut(worker)
            .expect("admitted")
            .reserve(BYTES, RESERVED)
            .expect("metered, and free");

        let finalization = tree.teardown(root).expect("the root is not finalized");
        assert!(
            finalization.unresolved_publications().is_empty(),
            "{}: the region layer resolved its own half",
            lane.token
        );
        assert!(
            finalization.is_total(),
            "{}: and reports totality",
            lane.token
        );
        assert_eq!(
            book.reconcile(&finalization).disagreements(),
            &[Disagreement::DanglingReservation {
                worker,
                dimension: BYTES
            }],
            "{}: the third accounting is independent and catches what the region's cannot",
            lane.token
        );
    }
}

#[test]
fn a_lane_cannot_park_while_one_of_its_artifacts_is_in_flight() {
    // > Cancellation MUST NOT truncate a publication in progress.
    // >
    // > — RFC 0026, "Atomicity of publication"
    //
    // Parking mid-publication would leave an artifact neither committed nor absent for as
    // long as the park lasted, which is the forbidden state at rest rather than in flight.
    // The refusal is what makes the `Park` beat's position in every lane's program a fact
    // rather than a convention.
    for lane in LANES {
        let mut tree = RegionTree::new();
        let root = tree.root();
        let worker = tree
            .spawn(root, Resumability::Resumable)
            .expect("the root is open");
        tree.advance(worker, WorkerStep::Begin).expect("legal");
        tree.advance(worker, WorkerStep::Reserve).expect("legal");
        assert_eq!(
            tree.advance(worker, WorkerStep::Suspend),
            Err(continuum_task::region::RegionFault::SuspendWithProvisionalEvidence { worker }),
            "{}: a lane parked mid-publication",
            lane.token
        );
        // And a non-resumable lane cannot park at all, which is why the solver lane's program
        // has no `Park` beat to place.
        if !lane.resumable {
            let other = tree
                .spawn(root, lane.resumability())
                .expect("the root is open");
            tree.advance(other, WorkerStep::Begin).expect("legal");
            assert_eq!(
                tree.advance(other, WorkerStep::Suspend),
                Err(
                    continuum_task::region::RegionFault::SuspendNonResumableWorker {
                        worker: other
                    }
                ),
                "{}: a lane that declared no continuation parked anyway",
                lane.token
            );
        }
        let finalization = tree.teardown(root).expect("the root is not finalized");
        assert!(
            finalization.is_total(),
            "{}: the refusals must not corrupt the teardown\n{}",
            lane.token,
            finalization.render()
        );
    }
}

#[test]
fn the_matrix_reaches_every_arm_of_the_cancellation_table() {
    // Anti-vacuity for the whole file: a matrix that never reached the
    // committed-with-continuation arm would be a matrix about the empty case.
    let mut arms: BTreeSet<&'static str> = BTreeSet::new();
    let mut terminal: BTreeSet<&'static str> = BTreeSet::new();
    let mut with_evidence = 0_usize;
    for lane in LANES {
        for (phase, row) in lane.rows.iter().enumerate() {
            let run = cancelled_at(lane, phase, Placement::Root, Arrival::Drain);
            let report = run.report(WorkerId::at(0));
            if let Some(outcome) = report.cancel_outcome() {
                arms.insert(outcome.token());
            }
            terminal.insert(report.state().status_token());
            if report.evidence().committed() > 0 {
                with_evidence += 1;
            }
            assert_eq!(row.committed, report.evidence().committed());
        }
    }
    assert_eq!(
        arms,
        BTreeSet::from([
            "committed-non-resumable",
            "committed-with-continuation",
            "nothing-published",
        ]),
        "every arm of `CancelOutcome` must be reached by the matrix"
    );
    assert_eq!(
        terminal,
        BTreeSet::from(["cancelled", "completed", "failed"]),
        "every terminal status a lane can end in must be reached"
    );
    assert!(
        with_evidence >= 8,
        "a matrix in which almost nothing committed proves little about partial evidence: \
         {with_evidence}"
    );
}

// --- determinism ---------------------------------------------------------------------------

/// A canonical rendering of the whole matrix: every lane, every phase, the finalization and
/// the reconciliation, in a fixed order with no timestamp, address or iteration order in it.
fn render_matrix() -> String {
    let mut out = String::new();
    for lane in LANES {
        for (phase, row) in lane.rows.iter().enumerate() {
            out.push_str(&format!("== {} {} ==\n", lane.token, row.phase));
            let run = cancelled_at(lane, phase, Placement::ChildFromRoot, Arrival::Drain);
            out.push_str(&run.finalization.render());
            out.push_str(&run.book.reconcile(&run.finalization).render());
            let ledger: &BudgetLedger = run.book.ledger(WorkerId::at(0)).expect("admitted");
            out.push_str(&ledger.render());
        }
    }
    out
}

#[test]
fn the_whole_matrix_renders_byte_identically_across_two_runs() {
    let first = render_matrix();
    let second = render_matrix();
    assert_eq!(
        first.as_bytes(),
        second.as_bytes(),
        "INV-005: one matrix, one rendering, byte for byte"
    );
    assert!(!first.is_empty(), "a vacuous render proves nothing");
    for token in [
        "committed-with-continuation",
        "committed-non-resumable",
        "nothing-published",
        "unsupported:budget.wall_ms",
    ] {
        assert!(
            first.contains(token),
            "the rendering must carry the facts it is comparing — {token} is missing"
        );
    }
    assert!(
        first.lines().count() > 100,
        "a rendering this short is not the matrix:\n{first}"
    );
}
