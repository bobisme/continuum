//! C023 arrow A7 (bn-ujpz0): the asupersync adapter's journal, checked against an
//! executable primitive conformance model written apart from it.
//!
//! > asupersync adapter ↔ executable primitive conformance models
//! >
//! > — RFC 0013, "Cross-path matrix". "Each arrow has generated random tests, curated
//! > edge cases, and mutation tests."
//!
//! > The adapter cannot be the sole refinement checker for itself.
//! >
//! > — docs/01 §6
//!
//! # The two paths
//!
//! - **Program side.** [`run`] drives asupersync 0.5.0's lab runtime under a choice log
//!   and observes the journal from the substrate's own trace (`src/binding.rs`). Its
//!   own check is [`lift`], into `continuum_task::region` and the families' parallel
//!   models.
//! - **Model side.** `support/primitive_conformance_model.rs`: a transition system of
//!   regions, tasks, cancellation phases, effects, obligations, virtual time and
//!   bounded channels (bn-1i050), written from docs/02 §7, docs/02 §5, docs/01 §6 and
//!   plan §4.1. It imports `std` only and
//!   reads the journal from its canonical **bytes** with its own reader, so it shares
//!   neither the adapter's types, its decoder, its lift, nor its scripted source.
//!   `tools/check_triptych_independence.py` (rule `a7-model-independent`) holds that.
//!
//! This file is the only place the two meet: it runs the substrate, encodes the journal,
//! and hands the bytes to the model.
//!
//! # Evidence map
//!
//! | Claim | Test |
//! |---|---|
//! | every substrate journal in the corpus is a trace the model accepts, and the lift agrees | [`every_substrate_journal_is_a_trace_the_conformance_model_accepts`] |
//! | converse: at every position of a sample, the model's enabled steps include what the substrate did, and the generator and the guard agree | [`the_models_enabled_steps_include_what_the_substrate_did`] |
//! | a run that leaks an obligation is rejected by the model at its region's close, and by the lift | [`a_leaking_run_is_rejected_by_the_model_and_by_the_lift`] |
//! | curated perturbations of real journals are rejected by the model, each for its own fault, and by the lift | [`perturbed_journals_are_rejected_by_the_conformance_model`] |
//! | every single-event deletion and adjacent swap of a sample: the model and the lift give the same verdict, in both directions | [`deletions_and_swaps_get_the_same_verdict_from_the_model_and_the_lift`] |
//! | the model reads bytes: truncations, trailing bytes and unknown family tags are malformed | [`the_model_reads_the_canonical_bytes_and_types_what_it_cannot_judge`] |
//! | every strict prefix of every corpus journal: mid-protocol ones do not conform, complete ones do, and the model agrees (cr-3pu5cu) | [`truncated_journals_do_not_conform_and_complete_prefixes_do`] |
//! | a task's own cancellation whose `cancel` is replaced by a region's drain, under all 32 projections and from created, running and suspended: the lift and the model reject it for that rule (cr-3pu5cu) | [`a_region_drain_does_not_end_a_task_s_own_cancellation`] |
//! | the same with `cancel` replaced by `resume`, `complete` (or `complete` alone), under all 32 projections | [`a_normal_end_does_not_end_a_task_s_own_cancellation`] |
//! | inside a task's own cancellation, 23 forged events of every family (the task's own work, another task's work, events that name no task) are rejected under every projection that carries their family, at the request, after the acknowledgement and before the task's `cancel`: 974 journals, the lift's `InterruptedOwnCancel` and the model's `OwnCancellationInterrupted`; the base journal, cleanup included, conforms under all 32 (cr-3pu5cu round 6) | [`every_family_is_held_to_a_task_s_own_cancellation`] |
//! | a region request that reaches a task needs its phases when the journal carries the family: `t0` unphased and ended by `complete` or `fail` (created or running, own or ancestor region) is `UnreportedRequest` in the lift and the model; the same without the family, and `t0` ended before the request, conform (6 violations, 18 controls; cr-3pu5cu round 6) | [`a_region_request_needs_phases_for_every_task_it_reaches`] |
//! | an own-region cancel after a due deadline (created, running, suspended) is `CancelRaced` in the lift and the model, as the binding refuses it under all 32 projections; the own region before the deadline, an ancestor after it, and every journal without the time family conform (3 violations, 15 controls; cr-3pu5cu round 7) | [`an_own_region_cancel_after_a_due_deadline_is_the_race`] |
//! | rules that read another family's fact: a leak by a parked holder, a leak whose holder never ends, a channel handed to a task in cancellation (cr-3pu5cu round 7 self-sweep) | [`cross_family_facts_are_read_by_the_lift`] |
//! | a deadline declared anywhere but right after its task's spawn is `DeadlineNotAtSpawn` in the lift and the model; the adjacent declaration and no deadline conform (cr-3pu5cu round 8) | [`a_deadline_is_declared_right_after_its_spawn`] |
//! | encoding versions 1 and 2 are two grammars: both readers read both, and refuse a version-2 tag under a version-1 label for each table that grew (cr-3pu5cu round 8) | [`the_two_encoding_versions_are_two_grammars`] |
//! | 13 hand-built journals from the round-6 pre-review pass (cleanup without the cancellation family then ordinary work, own work past a deadline, a repeated region cancel): the lift reports a violation and the model rejects each | [`round_six_pre_review_journals_are_rejected`] |
//! | a journal cut between a region's finalize and its settle is `Inconclusive(InsufficientTelemetry)`, never `Violates(Unsettled)`; restoring the settle conforms (bn-eaxx0, found by the bn-28hup adversarial pass) | [`a_journal_cut_between_finalize_and_settle_is_inconclusive`] |
//! | a region finalized over an obligation its holder ended still holding, never leaked, never settled, and with unrelated legitimate work after it, is `Violates(UnbalancedAtClose)`, not `Inconclusive`, closing the hole a first, unconditional fix reopened (bn-eaxx0's own pre-review adversarial pass) | [`a_finalized_region_with_an_open_obligation_and_no_settle_is_unbalanced_not_inconclusive`] |
//! | two finalized-unsettled regions, one clean and one genuinely unbalanced, siblings or nested, in both scan orders: always `Violates(UnbalancedAtClose)`, never masked by the clean one's incomplete found first; both clean is still `Inconclusive` (cr-19ec8g) | [`sibling_regions_one_clean_one_unbalanced_missing_settlement_is_unbalanced_at_close`], [`nested_regions_one_clean_one_unbalanced_missing_settlement_is_unbalanced_at_close`], [`two_regions_both_clean_missing_settlement_is_inconclusive`] |

#[path = "support/primitive_conformance_model.rs"]
mod model;

use std::collections::{BTreeMap, BTreeSet};

use continuum_asupersync::binding::{BindingConfig, Program, SubstrateOp, run};
use continuum_asupersync::choice::ChoiceLog;
use continuum_asupersync::family::cancellation::{CancelCause, CancellationEvent};
use continuum_asupersync::family::channel::{
    ChannelEvent, ChannelLabel, MessageOrdinal, MessageSet,
};
use continuum_asupersync::family::effect::{AbortCause, EffectEvent, ReservationLabel};
use continuum_asupersync::family::lifecycle::{
    LifecycleEvent, RegionLabel, RegionOrdinal, TaskLabel, TaskOrdinal, TaskSet, TaskStep,
};
use continuum_asupersync::family::obligation::{
    Discharge, ObligationEvent, ObligationKind, ObligationOrdinal, ObligationSet,
};
use continuum_asupersync::family::time::{TimeEvent, VirtualInstant};
use continuum_asupersync::family::{EventBody, Family};
use continuum_asupersync::journal::Journal;
use continuum_asupersync::lift::{LiftVerdict, Nonconformance, lift};
use continuum_task::region::RegionFault;
use continuum_task::region::worker::Resumability;

use model::{Alphabet, FamilyTag, Fault, Model, Verdict, WireFault};

const SEED: u64 = 0;

// --- the corpus ----------------------------------------------------------------------

fn spawn(region: RegionLabel, task: TaskLabel) -> SubstrateOp {
    SubstrateOp::Spawn {
        region,
        task,
        resumability: Resumability::Resumable,
    }
}

fn open(parent: RegionLabel, child: RegionLabel) -> SubstrateOp {
    SubstrateOp::OpenRegion { parent, child }
}

const fn begin(task: TaskLabel) -> SubstrateOp {
    SubstrateOp::Begin { task }
}

const fn reserve(task: TaskLabel, label: u32) -> SubstrateOp {
    SubstrateOp::Reserve {
        task,
        reservation: ReservationLabel(label),
    }
}

const fn commit(label: u32) -> SubstrateOp {
    SubstrateOp::Commit {
        reservation: ReservationLabel(label),
    }
}

const fn acquire(task: TaskLabel, label: u32, kind: ObligationKind) -> SubstrateOp {
    SubstrateOp::Acquire {
        task,
        reservation: ReservationLabel(label),
        kind,
    }
}

const ROOT: RegionLabel = RegionLabel::ROOT;
const T: [TaskLabel; 7] = [
    TaskLabel(0),
    TaskLabel(1),
    TaskLabel(2),
    TaskLabel(3),
    TaskLabel(4),
    TaskLabel(5),
    TaskLabel(6),
];
const R: [RegionLabel; 6] = [
    RegionLabel(0),
    RegionLabel(1),
    RegionLabel(2),
    RegionLabel(3),
    RegionLabel(4),
    RegionLabel(5),
];

/// Lifecycle: a root task that parks twice and completes, a region whose running task
/// is cancelled, and a region cancelled before its task ever ran.
fn lifecycle() -> Vec<Program> {
    vec![
        vec![
            spawn(ROOT, T[1]),
            begin(T[1]),
            SubstrateOp::Continue { task: T[1] },
            SubstrateOp::Finish { task: T[1] },
        ],
        vec![
            open(ROOT, R[1]),
            spawn(R[1], T[2]),
            begin(T[2]),
            SubstrateOp::Cancel { region: R[1] },
        ],
        vec![
            open(ROOT, R[2]),
            spawn(R[2], T[3]),
            SubstrateOp::Cancel { region: R[2] },
        ],
    ]
}

/// Cancellation: a nested pair cancelled at the outer region (`user` and
/// `parent-cancelled` causes, a never-begun task), a region whose close is upgraded to a
/// cancellation, and a sibling pair under one cancelled region.
fn cascade() -> Vec<Program> {
    vec![
        vec![
            open(ROOT, R[1]),
            open(R[1], R[2]),
            open(R[1], R[3]),
            spawn(R[1], T[1]),
            spawn(R[2], T[2]),
            spawn(R[3], T[3]),
            begin(T[2]),
            SubstrateOp::Cancel { region: R[1] },
        ],
        vec![
            open(ROOT, R[4]),
            spawn(R[4], T[4]),
            begin(T[4]),
            SubstrateOp::Close { region: R[4] },
            SubstrateOp::Cancel { region: R[4] },
        ],
    ]
}

/// Effects: a root task that commits two reservations in turn; a cancelled pair of
/// sibling regions whose tasks hold reservations when the cancellation arrives; and a
/// root task that aborts one on purpose.
fn effects() -> Vec<Program> {
    vec![
        vec![
            spawn(ROOT, T[1]),
            begin(T[1]),
            reserve(T[1], 1),
            commit(1),
            reserve(T[1], 2),
            commit(2),
        ],
        vec![
            open(ROOT, R[1]),
            open(R[1], R[2]),
            open(R[1], R[3]),
            spawn(R[2], T[2]),
            spawn(R[3], T[3]),
            begin(T[2]),
            begin(T[3]),
            reserve(T[2], 3),
            reserve(T[3], 4),
            commit(4),
            reserve(T[3], 5),
            SubstrateOp::Cancel { region: R[1] },
        ],
    ]
}

/// Obligations: a root task holding a lease across a transaction; a cancelled subtree
/// in which one task hands its I/O obligation to a task in a sibling region; a region
/// whose tasks hand a lease over and close normally.
fn ledger() -> Vec<Program> {
    vec![
        vec![
            spawn(ROOT, T[1]),
            begin(T[1]),
            acquire(T[1], 1, ObligationKind::Lease),
            reserve(T[1], 2),
            commit(2),
            commit(1),
        ],
        vec![
            open(ROOT, R[1]),
            open(R[1], R[2]),
            open(R[1], R[3]),
            spawn(R[2], T[2]),
            spawn(R[3], T[3]),
            begin(T[2]),
            begin(T[3]),
            acquire(T[2], 3, ObligationKind::Ack),
            acquire(T[3], 4, ObligationKind::IoOp),
            SubstrateOp::Transfer {
                reservation: ReservationLabel(4),
                to: T[2],
            },
            SubstrateOp::Cancel { region: R[1] },
        ],
    ]
}

/// A region whose first task hands a lease to the second, which commits it; both
/// finish and the region closes. With `leak`, the first task also returns holding an
/// ack.
fn handoff(leak: bool) -> Vec<Program> {
    let mut first = vec![
        open(ROOT, R[1]),
        spawn(R[1], T[1]),
        spawn(R[1], T[2]),
        begin(T[1]),
        begin(T[2]),
        acquire(T[1], 1, ObligationKind::Lease),
        SubstrateOp::Transfer {
            reservation: ReservationLabel(1),
            to: T[2],
        },
        commit(1),
    ];
    if leak {
        first.push(acquire(T[1], 2, ObligationKind::Ack));
    }
    first.extend([
        SubstrateOp::Finish { task: T[1] },
        SubstrateOp::Finish { task: T[2] },
        SubstrateOp::Close { region: R[1] },
    ]);
    vec![
        first,
        vec![
            spawn(ROOT, T[3]),
            begin(T[3]),
            acquire(T[3], 3, ObligationKind::SendPermit),
            commit(3),
        ],
    ]
}

/// Virtual time: a root task that sleeps; a cancelled subtree of two sleeping tasks
/// (their timers tie); a clock that advances twice.
fn clocked() -> Vec<Program> {
    vec![
        vec![
            spawn(ROOT, T[1]),
            begin(T[1]),
            SubstrateOp::Sleep {
                task: T[1],
                nanos: 100,
            },
        ],
        vec![
            open(ROOT, R[1]),
            open(R[1], R[2]),
            open(R[1], R[3]),
            spawn(R[2], T[2]),
            spawn(R[3], T[3]),
            begin(T[2]),
            begin(T[3]),
            SubstrateOp::Sleep {
                task: T[2],
                nanos: 100,
            },
            SubstrateOp::Sleep {
                task: T[3],
                nanos: 70,
            },
            SubstrateOp::Cancel { region: R[1] },
        ],
        vec![
            SubstrateOp::Advance { nanos: 60 },
            SubstrateOp::Advance { nanos: 60 },
        ],
    ]
}

/// One task that holds two reservations, aborts the first on purpose and commits the
/// second, then reserves again and commits.
fn explicit() -> Vec<Program> {
    vec![
        vec![
            spawn(ROOT, T[1]),
            begin(T[1]),
            reserve(T[1], 1),
            reserve(T[1], 2),
            SubstrateOp::Abort {
                reservation: ReservationLabel(1),
            },
            commit(2),
            reserve(T[1], 3),
            commit(3),
        ],
        vec![
            spawn(ROOT, T[2]),
            begin(T[2]),
            SubstrateOp::Finish { task: T[2] },
        ],
    ]
}

const fn send(task: TaskLabel, channel: ChannelLabel) -> SubstrateOp {
    SubstrateOp::Send { task, channel }
}

const fn recv(channel: ChannelLabel) -> SubstrateOp {
    SubstrateOp::Recv { channel }
}

const C1: ChannelLabel = ChannelLabel(1);
const C2: ChannelLabel = ChannelLabel(2);

/// Channels under cancellation. Two root producers race into `c1` (capacity 1), whose
/// receiver is in `r1`; a producer in `r1` and one at the root race into `c2`
/// (capacity 1), whose root receiver receives once. `r1`'s actor sends, receives once,
/// then cancels `r1`: the cancellation can find its receiver blocked (an abandoned
/// receive) or holding a message (dropped with the receiver), and its producer blocked
/// on `c2` (an abandoned send). Setup first, then the actors.
fn channels_cancel() -> (Program, Vec<Program>) {
    let mut setup = vec![open(ROOT, R[1])];
    for (region, task) in [
        (ROOT, T[1]),
        (ROOT, T[2]),
        (R[1], T[3]),
        (R[1], T[4]),
        (ROOT, T[5]),
        (ROOT, T[6]),
    ] {
        setup.push(spawn(region, task));
    }
    for task in &T[1..=6] {
        setup.push(begin(*task));
    }
    setup.push(SubstrateOp::OpenChannel {
        channel: C1,
        capacity: 1,
        receiver: T[3],
    });
    setup.push(SubstrateOp::OpenChannel {
        channel: C2,
        capacity: 1,
        receiver: T[5],
    });
    let actors = vec![
        vec![send(T[1], C1)],
        vec![send(T[2], C1)],
        vec![
            send(T[4], C2),
            recv(C1),
            SubstrateOp::Cancel { region: R[1] },
        ],
        vec![send(T[6], C2)],
        vec![recv(C2)],
    ];
    (setup, actors)
}

/// Channels that close. Three root producers race into `c1` (capacity 2), whose
/// receiver's task finishes without receiving: it goes with what is queued, and a
/// blocked producer then fails as closed. On `c2` the kept sender is dropped while its
/// receiver receives once: closed, before or after the receive blocks.
fn channels_close() -> (Program, Vec<Program>) {
    let mut setup = Vec::new();
    for task in &T[1..=5] {
        setup.push(spawn(ROOT, *task));
        setup.push(begin(*task));
    }
    setup.push(SubstrateOp::OpenChannel {
        channel: C1,
        capacity: 2,
        receiver: T[4],
    });
    setup.push(SubstrateOp::OpenChannel {
        channel: C2,
        capacity: 1,
        receiver: T[5],
    });
    let actors = vec![
        vec![send(T[1], C1)],
        vec![send(T[2], C1)],
        vec![send(T[3], C1)],
        vec![SubstrateOp::Finish { task: T[4] }],
        vec![SubstrateOp::CloseSenders { channel: C2 }],
        vec![recv(C2)],
    ];
    (setup, actors)
}

/// Budget deadlines (bn-36wy3). A fixed setup spawns four tasks: `t1` (deadline 50)
/// in `r2` holding a lease and a reservation, `t2` (deadline 30) in `r3` holding an
/// io-op and asleep for 40, a root task `t3` with no deadline, and a root task `t4`
/// (deadline 500) asleep for 20. Then the clock advances to 45 and 65, `t1` is woken
/// and its region closed, `r3` is cancelled, and `t3` finishes, in every order. So `t2`
/// ends by its deadline inside an advance (its due timer woke it) or by its region's
/// cancellation before the clock reaches 30; `t1` ends by its deadline at the wake after
/// the second advance, or just parks when woken earlier; `t4`'s timer fires before its
/// deadline.
fn deadlines() -> (Program, Vec<Program>) {
    let deadline = |region: RegionLabel, task: TaskLabel, at: u64| SubstrateOp::SpawnWithDeadline {
        region,
        task,
        resumability: Resumability::Resumable,
        deadline: at,
    };
    let setup = vec![
        open(ROOT, R[1]),
        open(R[1], R[2]),
        open(R[1], R[3]),
        deadline(R[2], T[1], 50),
        deadline(R[3], T[2], 30),
        spawn(ROOT, T[3]),
        deadline(ROOT, T[4], 500),
        begin(T[1]),
        begin(T[2]),
        begin(T[3]),
        begin(T[4]),
        acquire(T[1], 1, ObligationKind::Lease),
        reserve(T[1], 2),
        acquire(T[2], 3, ObligationKind::IoOp),
        SubstrateOp::Sleep {
            task: T[2],
            nanos: 40,
        },
        SubstrateOp::Sleep {
            task: T[4],
            nanos: 20,
        },
    ];
    let actors = vec![
        vec![
            SubstrateOp::Advance { nanos: 45 },
            SubstrateOp::Advance { nanos: 20 },
        ],
        vec![
            SubstrateOp::Continue { task: T[1] },
            SubstrateOp::Close { region: R[2] },
        ],
        vec![SubstrateOp::Cancel { region: R[3] }],
        vec![SubstrateOp::Finish { task: T[3] }],
    ];
    (setup, actors)
}

/// The programs (setup as actor 0) and a sample of logs that run the setup first.
fn sample_with_setup(
    setup: Program,
    actors: Vec<Program>,
    rng: &mut Splitmix,
) -> (Vec<Program>, Vec<ChoiceLog>) {
    let prefix = setup.len();
    let logs = sample(&actors, rng)
        .into_iter()
        .map(|log| {
            let mut choices = vec![0; prefix];
            choices.extend(log.choices().iter().map(|c| c.0));
            ChoiceLog::new(choices)
        })
        .collect();
    let mut programs = vec![setup];
    programs.extend(actors);
    (programs, logs)
}

fn lengths(programs: &[Program]) -> Vec<usize> {
    programs.iter().map(Vec::len).collect()
}

/// The explicit, seeded pseudo-random source of the random half of the corpus
/// (splitmix64). Its seed is a constant: no ambient entropy (INV-005).
struct Splitmix(u64);

impl Splitmix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}

/// A random complete log: at each step a uniformly chosen enabled actor.
fn random_log(lengths: &[usize], rng: &mut Splitmix) -> ChoiceLog {
    let mut left = lengths.to_vec();
    let mut choices = Vec::new();
    loop {
        let enabled: Vec<usize> = (0..left.len()).filter(|a| left[*a] > 0).collect();
        if enabled.is_empty() {
            return ChoiceLog::new(choices);
        }
        let len = u64::try_from(enabled.len()).unwrap();
        let index = usize::try_from(rng.next() % len).unwrap();
        left[enabled[index]] -= 1;
        choices.push(u32::try_from(index).unwrap());
    }
}

/// How many logs of each program set the corpus takes: a deterministic stride through
/// the exhaustive enumeration (curated: first and last included), then as many random
/// logs.
const STRIDE_SAMPLE: usize = 500;
const RANDOM_SAMPLE: usize = 500;

fn sample(programs: &[Program], rng: &mut Splitmix) -> Vec<ChoiceLog> {
    let all = ChoiceLog::enumerate(&lengths(programs));
    let mut out: Vec<ChoiceLog> = if all.len() <= STRIDE_SAMPLE {
        all
    } else {
        let step = all.len().div_ceil(STRIDE_SAMPLE);
        let mut picked: Vec<ChoiceLog> = all.iter().step_by(step).cloned().collect();
        picked.push(all[all.len() - 1].clone());
        for _ in 0..RANDOM_SAMPLE {
            picked.push(random_log(&lengths(programs), rng));
        }
        picked
    };
    out.sort();
    out.dedup();
    out
}

fn all_families() -> BindingConfig {
    BindingConfig::new(SEED)
        .observing(Family::Effect)
        .observing(Family::Cancellation)
        .observing(Family::Obligation)
        .observing(Family::Time)
        .observing(Family::Channel)
}

/// The model's alphabet for a binding configuration: the same families, named by the
/// model's own tags.
fn alphabet(config: &BindingConfig) -> Alphabet {
    let pairs = [
        (Family::Effect, FamilyTag::Effect),
        (Family::Cancellation, FamilyTag::Cancellation),
        (Family::Obligation, FamilyTag::Obligation),
        (Family::Time, FamilyTag::Time),
        (Family::Channel, FamilyTag::Channel),
    ];
    Alphabet::new(
        pairs
            .into_iter()
            .filter(|(family, _)| config.families.contains(*family))
            .map(|(_, tag)| tag),
    )
}

/// A corpus entry: name, programs, logs, configuration.
struct Entry {
    name: &'static str,
    programs: Vec<Program>,
    logs: Vec<ChoiceLog>,
    config: BindingConfig,
}

/// Every conforming (program set, log, configuration) the tests run.
fn corpus() -> Vec<Entry> {
    let mut rng = Splitmix(0xa7a7_a7a7_0c02_3a70);
    let mut out = Vec::new();
    // The channel sets run a fixed setup first (actor 0), then interleave the rest.
    for (name, (setup, actors)) in [
        ("channels-cancel", channels_cancel()),
        ("channels-close", channels_close()),
    ] {
        let (programs, logs) = sample_with_setup(setup, actors, &mut rng);
        out.push(Entry {
            name,
            programs,
            logs,
            config: all_families(),
        });
    }
    let sets: [(&'static str, Vec<Program>); 6] = [
        ("lifecycle", lifecycle()),
        ("cascade", cascade()),
        ("effects", effects()),
        ("ledger", ledger()),
        ("handoff", handoff(false)),
        ("clocked", clocked()),
    ];
    for (name, programs) in sets {
        let logs = sample(&programs, &mut rng);
        out.push(Entry {
            name,
            programs,
            logs,
            config: all_families(),
        });
    }
    // Budget deadlines (bn-36wy3), observed with every family, and without the
    // cancellation family (so the single-task cancellation shows only as lifecycle
    // steps around its cleanup). Appended last, so the entries above keep their logs.
    let deadline_entries: [(&'static str, BindingConfig); 2] = [
        ("deadlines", all_families()),
        (
            "deadlines/without-cancellation",
            BindingConfig::new(SEED)
                .observing(Family::Effect)
                .observing(Family::Obligation)
                .observing(Family::Time),
        ),
    ];
    // The alphabet is a parameter: the same lifecycle programs, observed as lifecycle
    // alone and as lifecycle with cancellation phases.
    for (name, config) in [
        ("lifecycle/lifecycle-only", BindingConfig::new(SEED)),
        (
            "cascade/with-cancellation",
            BindingConfig::new(SEED).observing(Family::Cancellation),
        ),
    ] {
        let programs = if name.starts_with("lifecycle") {
            lifecycle()
        } else {
            cascade()
        };
        let logs = sample(&programs, &mut rng);
        out.push(Entry {
            name,
            programs,
            logs,
            config,
        });
    }
    for (name, config) in deadline_entries {
        let (setup, actors) = deadlines();
        let (programs, logs) = sample_with_setup(setup, actors, &mut rng);
        out.push(Entry {
            name,
            programs,
            logs,
            config,
        });
    }
    out
}

fn journal_of(entry: &Entry, log: &ChoiceLog) -> Journal {
    run(&entry.programs, log, &entry.config)
        .unwrap_or_else(|refusal| panic!("{} log {log}: {refusal}", entry.name))
}

fn judge(config: &BindingConfig, journal: &Journal) -> Verdict {
    model::judge(&alphabet(config), &journal.encode().unwrap())
}

fn steps_of(journal: &Journal) -> Vec<model::Step> {
    model::read(&journal.encode().unwrap()).unwrap()
}

// --- acceptance ----------------------------------------------------------------------

#[test]
fn every_substrate_journal_is_a_trace_the_conformance_model_accepts() {
    let mut runs = 0_usize;
    let mut per_family: BTreeMap<&'static str, usize> = BTreeMap::new();
    for entry in corpus() {
        for log in &entry.logs {
            let journal = journal_of(&entry, log);
            let verdict = judge(&entry.config, &journal);
            assert_eq!(
                verdict,
                Verdict::Accepted {
                    steps: journal.len()
                },
                "{} log {log}: the model rejects a substrate journal\n{}",
                entry.name,
                journal.render()
            );
            assert!(
                matches!(lift(&journal), LiftVerdict::Conforms(_)),
                "{} log {log}: the lift and the model disagree",
                entry.name
            );
            for event in journal.events() {
                *per_family.entry(event.family().token()).or_default() += 1;
            }
            runs += 1;
        }
    }
    // Non-vacuity: the corpus is large and every bound family's events were judged.
    eprintln!("{runs} runs, events per family {per_family:?}");
    // The corpus is deterministic (explicit seeds, seed-independent journals), and
    // docs/18's C023 record cites these counts.
    assert_eq!(runs, 7_873, "docs/18 C023 cites this count");
    for family in [
        "lifecycle",
        "reserve-commit-abort",
        "cancellation",
        "obligation",
        "virtual-time",
    ] {
        assert!(
            per_family.get(family).copied().unwrap_or(0) > 100,
            "{family}: {per_family:?}"
        );
    }
}

#[test]
fn explicit_aborts_and_several_held_reservations_are_accepted_by_the_model_and_the_lift() {
    // docs/02 §7 has `Reserved → Aborted(reason)` for any reason, and RFC 0026
    // correction 51 gives the calculus the holder's own abort (`AbortSlot`) and one
    // staged publication per slot. Before bn-j1a50 the lift read an explicit abort as
    // unsupported semantics; now both paths accept it, with two reservations held.
    let programs = explicit();
    for log in ChoiceLog::enumerate(&lengths(&programs)) {
        let journal = run(&programs, &log, &all_families()).unwrap();
        assert!(judge(&all_families(), &journal).is_accepted(), "log {log}");
        assert!(
            matches!(lift(&journal), LiftVerdict::Conforms(_)),
            "log {log}: {:?}\n{}",
            lift(&journal),
            journal.render()
        );
    }
}

// --- the converse --------------------------------------------------------------------

#[test]
fn the_models_enabled_steps_include_what_the_substrate_did() {
    let mut positions = 0_usize;
    let mut exact_checked = 0_usize;
    for entry in corpus() {
        // A sample: every seventh log of each entry.
        for log in entry.logs.iter().step_by(10) {
            let journal = journal_of(&entry, log);
            let mut state = Model::new(alphabet(&entry.config));
            for (at, step) in steps_of(&journal).iter().enumerate() {
                let enabled = state.enabled();
                assert!(
                    enabled.iter().any(|pattern| pattern.admits(step)),
                    "{} log {log} at {at}: the substrate took {step:?}, which the model does \
                     not enable; enabled: {enabled:?}",
                    entry.name
                );
                // The two formulations of the relation agree here: every step the
                // generator enables exactly, the guard admits.
                for pattern in &enabled {
                    if let model::Pattern::Exact(candidate) = pattern {
                        let mut probe = state.clone();
                        assert_eq!(
                            probe.step(candidate),
                            Ok(()),
                            "{} log {log} at {at}: generated {candidate:?} but the guard refuses",
                            entry.name
                        );
                        exact_checked += 1;
                    }
                }
                state.step(step).unwrap();
                positions += 1;
            }
            assert_eq!(state.finish(), Ok(()));
        }
    }
    eprintln!("{positions} positions, {exact_checked} generated steps checked by the guard");
    assert_eq!(positions, 31_650, "docs/18 C023 cites this count");
    assert!(exact_checked > positions, "{exact_checked}");
}

// --- rejection -----------------------------------------------------------------------

/// The deadline entries are not vacuous (bn-36wy3): among their journals, deadline
/// cancellations end tasks both inside an advance (a due timer woke the task) and at a
/// gate wake, and a region's cancellation ends the same task in other logs. The model
/// accepts every one, with and without the cancellation family, and the lift agrees
/// (both are checked for every journal by the corpus test above).
#[test]
fn deadline_cancellations_are_in_the_corpus_and_accepted() {
    let mut by_deadline = BTreeMap::<&str, usize>::new();
    let mut by_region = 0_usize;
    for entry in corpus()
        .into_iter()
        .filter(|entry| entry.name.starts_with("deadlines"))
    {
        for log in &entry.logs {
            let journal = journal_of(&entry, log);
            assert!(judge(&entry.config, &journal).is_accepted());
            let ended = |task: u32| {
                bodies(&journal).iter().any(|b| {
                    matches!(
                        b,
                        EventBody::Lifecycle(LifecycleEvent::TaskStepped {
                            task: TaskOrdinal(t),
                            step: TaskStep::Cancel,
                        }) if *t == task
                    )
                })
            };
            // t0 (label t1) at a gate wake; t1 (label t2) inside an advance.
            if ended(0) {
                *by_deadline.entry("gate wake").or_default() += 1;
            }
            if ended(1) {
                *by_deadline.entry("advance").or_default() += 1;
            } else {
                by_region += 1;
            }
        }
    }
    assert!(
        by_deadline.get("gate wake").copied().unwrap_or(0) > 20
            && by_deadline.get("advance").copied().unwrap_or(0) > 20,
        "{by_deadline:?}"
    );
    assert!(by_region > 20, "{by_region}");
}

#[test]
fn a_leaking_run_is_rejected_by_the_model_and_by_the_lift() {
    let programs = handoff(true);
    let mut rng = Splitmix(7);
    let logs = sample(&programs, &mut rng);
    assert!(logs.len() > 100);
    for log in &logs {
        let journal = run(&programs, log, &all_families()).unwrap();
        let verdict = judge(&all_families(), &journal);
        assert!(
            matches!(
                verdict,
                Verdict::Rejected {
                    fault: Fault::CloseWithObligations(1),
                    ..
                }
            ),
            "log {log}: {verdict}\n{}",
            journal.render()
        );
        assert!(
            matches!(lift(&journal), LiftVerdict::Violates { .. }),
            "log {log}"
        );
    }
}

/// Rebuild a journal from event bodies (sequence numbers are the positions).
fn rebuild(bodies: impl IntoIterator<Item = EventBody>) -> Journal {
    let mut journal = Journal::new();
    for body in bodies {
        journal.append(body).unwrap();
    }
    journal
}

fn bodies(journal: &Journal) -> Vec<EventBody> {
    journal.events().iter().map(|e| e.body().clone()).collect()
}

/// Where the first event `test` picks sits, if the journal has one.
fn position(journal: &Journal, test: impl Fn(&EventBody) -> bool) -> Option<usize> {
    journal.events().iter().position(|e| test(e.body()))
}

/// A named perturbation of a real journal (`None` when the journal has no event of the
/// kind it perturbs) and the faults that count as rejecting it for its own reason.
type Mutant = (
    &'static str,
    fn(&Journal) -> Option<Journal>,
    fn(&Fault) -> bool,
);

/// The task of the journal's first single-task `cancel-requested`, if any.
fn first_requested_alone(j: &Journal) -> Option<TaskOrdinal> {
    bodies(j).into_iter().find_map(|b| match b {
        EventBody::Lifecycle(LifecycleEvent::TaskStepped {
            task,
            step: TaskStep::CancelRequested,
        }) => Some(task),
        _ => None,
    })
}

/// The first cancellation phase `requested` a region's cancellation reported, and where
/// it sits: the task is then requested and has not acknowledged.
fn first_region_request(j: &Journal) -> Option<(usize, TaskOrdinal)> {
    bodies(j)
        .into_iter()
        .enumerate()
        .find_map(|(at, b)| match b {
            EventBody::Cancellation(CancellationEvent::Requested { task, cause })
                if cause != CancelCause::Deadline =>
            {
                Some((at, task))
            }
            _ => None,
        })
}

/// `event` inserted right after the first region-requested phase, for its task.
fn insert_after_region_request(
    j: &Journal,
    event: impl Fn(&[EventBody], TaskOrdinal) -> EventBody,
) -> Option<Journal> {
    let (at, task) = first_region_request(j)?;
    let mut b = bodies(j);
    let forged = event(&b[..=at], task);
    b.insert(at + 1, forged);
    Some(rebuild(b))
}

#[allow(clippy::too_many_lines)]
fn mutants() -> Vec<Mutant> {
    vec![
        // RFC 0026 correction 55 (bn-28hup): a requested task's next step is its
        // acknowledgement. Three of its steps, each placed between its `requested` and
        // its `acknowledged`: its own lifecycle step, its own work in another family,
        // and a receipt.
        (
            "a requested task parks before it acknowledges",
            |j| {
                insert_after_region_request(j, |_, task| {
                    EventBody::Lifecycle(LifecycleEvent::TaskStepped {
                        task,
                        step: TaskStep::Suspend,
                    })
                })
            },
            |f| matches!(f, Fault::Unobserved(_)),
        ),
        (
            "a requested task arms a timer before it acknowledges",
            |j| {
                insert_after_region_request(j, |prefix, task| {
                    let timers = prefix
                        .iter()
                        .filter(|e| matches!(e, EventBody::Time(TimeEvent::Scheduled { .. })))
                        .count();
                    let now = prefix
                        .iter()
                        .rev()
                        .find_map(|e| match e {
                            EventBody::Time(TimeEvent::Advanced { to, .. }) => Some(to.0),
                            _ => None,
                        })
                        .unwrap_or(0);
                    EventBody::Time(TimeEvent::Scheduled {
                        timer: continuum_asupersync::family::time::TimerOrdinal(
                            u32::try_from(timers).unwrap(),
                        ),
                        task,
                        at: VirtualInstant(now),
                        deadline: VirtualInstant(now + 1),
                    })
                })
            },
            |f| matches!(f, Fault::Unobserved(_)),
        ),
        (
            "a requested task is handed an obligation before it acknowledges",
            |j| {
                let (_, task) = first_region_request(j)?;
                // An obligation held, at that point, by a task no request reached.
                let (at, _) = first_region_request(j)?;
                let b = bodies(j);
                let region_of = |t: TaskOrdinal| {
                    b.iter().find_map(|e| match e {
                        EventBody::Lifecycle(LifecycleEvent::TaskSpawned {
                            task, region, ..
                        }) if *task == t => Some(*region),
                        _ => None,
                    })
                };
                let obligation = b[..at].iter().find_map(|e| match e {
                    EventBody::Obligation(ObligationEvent::Opened {
                        obligation, holder, ..
                    }) if *holder != task => Some(*obligation),
                    _ => None,
                })?;
                let region = region_of(task)?;
                insert_after_region_request(j, |_, task| {
                    EventBody::Obligation(ObligationEvent::Transferred {
                        obligation,
                        holder: task,
                        region,
                    })
                })
            },
            // The giver may be a requested task too, whose `requested` comes later.
            |f| matches!(f, Fault::Unobserved(_) | Fault::UnreportedRequest(_)),
        ),
        (
            "a deadline declared one event after its task's spawn",
            |j| {
                let mut b = bodies(j);
                let at = position(j, |e| {
                    matches!(e, EventBody::Time(TimeEvent::Deadline { .. }))
                })?;
                if at + 1 >= b.len() {
                    return None;
                }
                b.swap(at, at + 1);
                Some(rebuild(b))
            },
            // The event after the declaration now comes first: the declaration is not
            // right after the spawn (cr-3pu5cu round 8), or that event is refused first.
            |f| {
                matches!(
                    f,
                    Fault::DeadlineNotAtSpawn(_) | Fault::TaskPhase(_) | Fault::Identity { .. }
                )
            },
        ),
        (
            "a deadline cancellation before the clock reached the deadline",
            |j| {
                let task = first_requested_alone(j)?;
                let mut b = bodies(j);
                let at = position(
                    j,
                    |e| matches!(e, EventBody::Time(TimeEvent::Deadline { task: t, .. }) if *t == task),
                )?;
                b[at] = EventBody::Time(TimeEvent::Deadline {
                    task,
                    at: VirtualInstant(1_000_000),
                });
                Some(rebuild(b))
            },
            |f| matches!(f, Fault::Deadline(_)),
        ),
        (
            "a deadline's cancellation without the task's own request",
            |j| {
                let task = first_requested_alone(j)?;
                let mut b = bodies(j);
                b.remove(position(j, |e| {
                    matches!(e, EventBody::Lifecycle(LifecycleEvent::TaskStepped {
                        task: t,
                        step: TaskStep::CancelRequested,
                    }) if *t == task)
                })?);
                Some(rebuild(b))
            },
            |f| matches!(f, Fault::TaskPhase(_) | Fault::CancelPhase(_)),
        ),
        (
            "a cancellation completes without its acknowledgement",
            |j| {
                let mut b = bodies(j);
                b.remove(position(j, |e| {
                    matches!(
                        e,
                        EventBody::Cancellation(CancellationEvent::Acknowledged { .. })
                    )
                })?);
                Some(rebuild(b))
            },
            // The next step of the task's cleanup is refused: its completion, or the
            // effect abort, aborted discharge or receiver's drop that precedes it
            // (the last two by RFC 0026 correction 55's observation rule). Inside a
            // task's own (deadline) cancellation, a cleanup step before the
            // acknowledgement is not a step of that cancellation.
            |f| {
                matches!(
                    f,
                    Fault::CancelPhase(_)
                        | Fault::TaskPhase(_)
                        | Fault::Unobserved(_)
                        | Fault::OwnCancellationInterrupted(_)
                )
            },
        ),
        (
            "a request carries the other cause",
            |j| {
                let mut b = bodies(j);
                // A region's request: user and parent-cancelled are the two causes the
                // region tree implies (a deadline's request is the task's own).
                let at = position(j, |e| {
                    matches!(
                        e,
                        EventBody::Cancellation(CancellationEvent::Requested { cause, .. })
                            if *cause != CancelCause::Deadline
                    )
                })?;
                if let EventBody::Cancellation(CancellationEvent::Requested { task, cause }) = b[at]
                {
                    let other = match cause {
                        CancelCause::User => CancelCause::ParentCancelled,
                        CancelCause::ParentCancelled | CancelCause::Deadline => CancelCause::User,
                    };
                    b[at] = EventBody::Cancellation(CancellationEvent::Requested {
                        task,
                        cause: other,
                    });
                }
                Some(rebuild(b))
            },
            |f| matches!(f, Fault::Cause(_)),
        ),
        (
            "a drain reports one cancelled task fewer",
            |j| {
                let mut b = bodies(j);
                let at = position(j, |e| {
                    matches!(e, EventBody::Lifecycle(LifecycleEvent::RegionDrained { cancelled, .. })
                        if !cancelled.as_slice().is_empty())
                })?;
                if let EventBody::Lifecycle(LifecycleEvent::RegionDrained { region, cancelled }) =
                    &b[at]
                {
                    let fewer = TaskSet::new(cancelled.as_slice()[1..].iter().copied());
                    b[at] = EventBody::Lifecycle(LifecycleEvent::RegionDrained {
                        region: *region,
                        cancelled: fewer,
                    });
                }
                Some(rebuild(b))
            },
            |f| matches!(f, Fault::DrainSet { .. }),
        ),
        (
            "a region finalizes before it drains",
            |j| {
                let mut b = bodies(j);
                let at = position(j, |e| {
                    matches!(
                        e,
                        EventBody::Lifecycle(LifecycleEvent::RegionDrained { .. })
                    )
                })?;
                b.swap(at, at + 1);
                Some(rebuild(b))
            },
            |f| matches!(f, Fault::RegionPhase(_)),
        ),
        (
            "a committed reservation commits again",
            |j| {
                let mut b = bodies(j);
                let at = position(j, |e| {
                    matches!(e, EventBody::Effect(EffectEvent::Committed { .. }))
                })?;
                b.insert(at + 1, b[at].clone());
                Some(rebuild(b))
            },
            |f| matches!(f, Fault::EffectPhase(_)),
        ),
        (
            "a cancel abort precedes the holder's acknowledgement",
            |j| {
                let mut b = bodies(j);
                let at = position(j, |e| {
                    matches!(
                        e,
                        EventBody::Effect(EffectEvent::Aborted {
                            cause: AbortCause::Cancel,
                            ..
                        })
                    )
                })?;
                let EventBody::Effect(EffectEvent::Aborted { reservation, .. }) = b[at] else {
                    return None;
                };
                let holder = b.iter().find_map(|e| match e {
                    EventBody::Effect(EffectEvent::Reserved {
                        reservation: r,
                        task,
                    }) if *r == reservation => Some(*task),
                    _ => None,
                })?;
                let ack = position(
                    j,
                    |e| matches!(e, EventBody::Cancellation(CancellationEvent::Acknowledged { task }) if *task == holder),
                )?;
                let aborted = b.remove(at);
                b.insert(ack, aborted);
                Some(rebuild(b))
            },
            // Inside a task's own (deadline) cancellation, a cleanup step before the
            // acknowledgement is not a step of that cancellation (cr-3pu5cu round 6).
            |f| {
                matches!(
                    f,
                    Fault::TaskPhase(_) | Fault::OwnCancellationInterrupted(_)
                )
            },
        ),
        (
            "a timer fires before the clock reaches its deadline",
            |j| {
                let mut b = bodies(j);
                let at = position(j, |e| matches!(e, EventBody::Time(TimeEvent::Fired { .. })))?;
                let advanced = (0..at)
                    .rev()
                    .find(|i| matches!(b[*i], EventBody::Time(TimeEvent::Advanced { .. })))?;
                let fired = b.remove(at);
                b.insert(advanced, fired);
                Some(rebuild(b))
            },
            |f| matches!(f, Fault::OffClock { .. } | Fault::TimerPhase(_)),
        ),
        (
            "the clock runs backwards",
            |j| {
                let mut b = bodies(j);
                let at = position(j, |e| {
                    matches!(e, EventBody::Time(TimeEvent::Advanced { .. }))
                })?;
                if let EventBody::Time(TimeEvent::Advanced { from, to }) = b[at] {
                    b[at] = EventBody::Time(TimeEvent::Advanced { from: to, to: from });
                }
                Some(rebuild(b))
            },
            |f| matches!(f, Fault::OffClock { .. } | Fault::NotAhead),
        ),
        (
            "a timer is lost: its task wakes with the timer still armed",
            |j| {
                let mut b = bodies(j);
                b.remove(position(j, |e| {
                    matches!(e, EventBody::Time(TimeEvent::Fired { .. }))
                })?);
                Some(rebuild(b))
            },
            |f| matches!(f, Fault::TimerOutlivesSleep { .. } | Fault::LateTimer(_)),
        ),
        (
            "a transfer names a region other than the new holder's",
            |j| {
                let mut b = bodies(j);
                let at = position(j, |e| {
                    matches!(
                        e,
                        EventBody::Obligation(ObligationEvent::Transferred { .. })
                    )
                })?;
                if let EventBody::Obligation(ObligationEvent::Transferred {
                    obligation,
                    holder,
                    ..
                }) = b[at]
                {
                    b[at] = EventBody::Obligation(ObligationEvent::Transferred {
                        obligation,
                        holder,
                        region: RegionOrdinal(0),
                    });
                }
                Some(rebuild(b))
            },
            |f| matches!(f, Fault::ObligationRegion(_)),
        ),
        (
            "a cancelled task's obligation abort is lost",
            |j| {
                let mut b = bodies(j);
                b.remove(position(j, |e| {
                    matches!(
                        e,
                        EventBody::Obligation(ObligationEvent::Discharged {
                            how: Discharge::Aborted,
                            ..
                        })
                    )
                })?);
                Some(rebuild(b))
            },
            |f| {
                matches!(
                    f,
                    Fault::HeldObligation { .. }
                        | Fault::Settle(_)
                        | Fault::CloseWithObligations(_)
                )
            },
        ),
        (
            "a region settles with an obligation reported open",
            |j| {
                let mut b = bodies(j);
                let at = position(j, |e| {
                    matches!(
                        e,
                        EventBody::Obligation(ObligationEvent::RegionSettled { .. })
                    )
                })?;
                if let EventBody::Obligation(ObligationEvent::RegionSettled {
                    region,
                    leaked,
                    ..
                }) = &b[at]
                {
                    b[at] = EventBody::Obligation(ObligationEvent::RegionSettled {
                        region: *region,
                        open: ObligationSet::new([ObligationOrdinal(0)]),
                        leaked: leaked.clone(),
                        fenced: ObligationSet::default(),
                    });
                }
                Some(rebuild(b))
            },
            |f| matches!(f, Fault::Settle(_)),
        ),
        (
            "a region is opened under a skipped ordinal",
            |j| {
                let mut b = bodies(j);
                let at = position(j, |e| {
                    matches!(e, EventBody::Lifecycle(LifecycleEvent::RegionOpened { .. }))
                })?;
                if let EventBody::Lifecycle(LifecycleEvent::RegionOpened { region, parent }) = b[at]
                {
                    b[at] = EventBody::Lifecycle(LifecycleEvent::RegionOpened {
                        region: RegionOrdinal(region.0 + 1),
                        parent,
                    });
                }
                Some(rebuild(b))
            },
            |f| matches!(f, Fault::Identity { .. }),
        ),
        (
            "a cancelling task completes normally",
            |j| {
                let mut b = bodies(j);
                let at = position(j, |e| {
                    matches!(
                        e,
                        EventBody::Cancellation(CancellationEvent::Acknowledged { .. })
                    )
                })?;
                if let EventBody::Cancellation(CancellationEvent::Acknowledged { task }) = b[at] {
                    b.insert(
                        at + 1,
                        EventBody::Lifecycle(LifecycleEvent::TaskStepped {
                            task,
                            step: TaskStep::Complete,
                        }),
                    );
                }
                Some(rebuild(b))
            },
            // Inside a task's own (deadline) cancellation, only its own `→ Cancelled`
            // is a lifecycle step (cr-3pu5cu round 6).
            |f| {
                matches!(
                    f,
                    Fault::TaskPhase(_)
                        | Fault::CancelPhase(_)
                        | Fault::OwnCancellationInterrupted(_)
                )
            },
        ),
        (
            "work enters a region after its cancellation",
            |j| {
                let mut b = bodies(j);
                let at = position(j, |e| {
                    matches!(
                        e,
                        EventBody::Lifecycle(LifecycleEvent::RegionCancelRequested { .. })
                    )
                })?;
                if let EventBody::Lifecycle(LifecycleEvent::RegionCancelRequested { region }) =
                    b[at]
                {
                    let next = j
                        .events()
                        .iter()
                        .filter(|e| {
                            matches!(
                                e.body(),
                                EventBody::Lifecycle(LifecycleEvent::TaskSpawned { .. })
                            )
                        })
                        .count();
                    b.insert(
                        at + 1,
                        EventBody::Lifecycle(LifecycleEvent::TaskSpawned {
                            task: TaskOrdinal(u32::try_from(next).unwrap()),
                            region,
                            resumability: Resumability::Resumable,
                        }),
                    );
                }
                Some(rebuild(b))
            },
            |f| matches!(f, Fault::RegionNotOpen(_) | Fault::Identity { .. }),
        ),
        (
            "a sleeping timer is scheduled in the past",
            |j| {
                let mut b = bodies(j);
                let at = position(j, |e| {
                    matches!(e, EventBody::Time(TimeEvent::Scheduled { .. }))
                })?;
                if let EventBody::Time(TimeEvent::Scheduled {
                    timer,
                    task,
                    at: when,
                    ..
                }) = b[at]
                {
                    b[at] = EventBody::Time(TimeEvent::Scheduled {
                        timer,
                        task,
                        at: when,
                        deadline: VirtualInstant(when.0),
                    });
                }
                Some(rebuild(b))
            },
            |f| matches!(f, Fault::NotAhead),
        ),
        (
            "a receiver takes a message that is not the oldest queued",
            |j| {
                let mut b = bodies(j);
                let at = position(j, |e| {
                    matches!(e, EventBody::Channel(ChannelEvent::Received { .. }))
                })?;
                if let EventBody::Channel(ChannelEvent::Received { channel, message }) = b[at] {
                    b[at] = EventBody::Channel(ChannelEvent::Received {
                        channel,
                        message: MessageOrdinal(message.0 + 100),
                    });
                }
                Some(rebuild(b))
            },
            |f| matches!(f, Fault::Channel { .. }),
        ),
        (
            "a receiver goes declaring one queued message fewer",
            |j| {
                let mut b = bodies(j);
                let at = position(j, |e| {
                    matches!(e, EventBody::Channel(ChannelEvent::ReceiverGone { discarded, .. })
                        if !discarded.as_slice().is_empty())
                })?;
                if let EventBody::Channel(ChannelEvent::ReceiverGone { channel, discarded }) =
                    &b[at]
                {
                    let fewer = MessageSet::new(discarded.as_slice()[1..].iter().copied());
                    b[at] = EventBody::Channel(ChannelEvent::ReceiverGone {
                        channel: *channel,
                        discarded: fewer,
                    });
                }
                Some(rebuild(b))
            },
            |f| matches!(f, Fault::Channel { .. }),
        ),
        (
            "a message is sent without its committed send permit",
            |j| {
                let mut b = bodies(j);
                let sent = position(j, |e| {
                    matches!(e, EventBody::Channel(ChannelEvent::Sent { .. }))
                })?;
                let commit = (0..sent).rev().find(|i| {
                    matches!(
                        b[*i],
                        EventBody::Obligation(ObligationEvent::Discharged {
                            how: Discharge::Committed,
                            ..
                        })
                    )
                })?;
                b.remove(commit);
                Some(rebuild(b))
            },
            |f| matches!(f, Fault::NoSendPermit { .. }),
        ),
        (
            "a blocked send is reported sent into the full channel",
            |j| {
                let mut b = bodies(j);
                let at = position(j, |e| {
                    matches!(e, EventBody::Channel(ChannelEvent::SendBlocked { .. }))
                })?;
                if let EventBody::Channel(ChannelEvent::SendBlocked {
                    channel,
                    message,
                    sender,
                }) = b[at]
                {
                    b[at] = EventBody::Channel(ChannelEvent::Sent {
                        channel,
                        message,
                        sender,
                    });
                }
                Some(rebuild(b))
            },
            |f| matches!(f, Fault::Channel { .. } | Fault::NoSendPermit { .. }),
        ),
        (
            "a blocked receive is abandoned before the receiver's acknowledgement",
            |j| {
                let mut b = bodies(j);
                let at = position(j, |e| {
                    matches!(e, EventBody::Channel(ChannelEvent::RecvAbandoned { .. }))
                })?;
                let EventBody::Channel(ChannelEvent::RecvAbandoned { channel }) = b[at] else {
                    return None;
                };
                let receiver = b.iter().find_map(|e| match e {
                    EventBody::Channel(ChannelEvent::Opened {
                        channel: c,
                        receiver,
                        ..
                    }) if *c == channel => Some(*receiver),
                    _ => None,
                })?;
                let ack = position(
                    j,
                    |e| matches!(e, EventBody::Cancellation(CancellationEvent::Acknowledged { task }) if *task == receiver),
                )?;
                let abandoned = b.remove(at);
                b.insert(ack, abandoned);
                Some(rebuild(b))
            },
            |f| matches!(f, Fault::TaskPhase(_)),
        ),
    ]
}

#[test]
fn perturbed_journals_are_rejected_by_the_conformance_model() {
    let config = all_families();
    // Real journals with every family in them: one per program set that has the
    // events a mutant perturbs.
    let mut bases: Vec<Journal> = [effects(), ledger(), clocked(), cascade()]
        .iter()
        .map(|programs| {
            let log = ChoiceLog::enumerate(&lengths(programs)).remove(0);
            run(programs, &log, &config).unwrap()
        })
        .collect();
    // Channel and deadline journals: every tenth sampled log of each set.
    let mut rng = Splitmix(0xc4a7);
    for (setup, actors) in [channels_cancel(), channels_close(), deadlines()] {
        let (programs, logs) = sample_with_setup(setup, actors, &mut rng);
        for log in logs.iter().step_by(10) {
            bases.push(run(&programs, log, &config).unwrap());
        }
    }
    let mut rejected = 0;
    let mut admitted_by_lift = Vec::new();
    for (name, mutate, expected) in mutants() {
        let mut applied = 0;
        for base in &bases {
            assert!(judge(&config, base).is_accepted());
            let Some(mutant) = mutate(base) else {
                continue; // this base has no event of the kind the mutant perturbs
            };
            applied += 1;
            match judge(&config, &mutant) {
                Verdict::Rejected { fault, .. } => {
                    assert!(expected(&fault), "{name}: rejected, but for {fault:?}");
                    rejected += 1;
                    if lift_accepts(&mutant) {
                        admitted_by_lift.push(name);
                    }
                }
                other => panic!(
                    "{name}: the model does not reject the mutant: {other}\n{}",
                    mutant.render()
                ),
            }
        }
        assert!(
            applied > 0,
            "{name}: no base journal has the event it perturbs"
        );
    }
    assert_eq!(mutants().len(), 27, "docs/18 C023 cites this count");
    assert!(rejected >= mutants().len());
    // Since bn-1i050 the lift rejects every mutant the model rejects.
    assert!(admitted_by_lift.is_empty(), "{admitted_by_lift:?}");
}

/// The lift's verdict, as the model's two outcomes: conforms, or not.
fn lift_accepts(journal: &Journal) -> bool {
    matches!(lift(journal), LiftVerdict::Conforms(_))
}

/// A disagreement's class: the model's fault and the kind of step it refused.
fn class_of(config: &BindingConfig, variant: &Journal, verdict: &Verdict) -> String {
    let Verdict::Rejected { at, fault } = verdict else {
        return format!("model {verdict}");
    };
    let steps = steps_of(variant);
    let step = steps.get(*at).map_or_else(
        || "end of trace".to_owned(),
        |s| {
            format!("{s:?}")
                .split([' ', '{'])
                .next()
                .unwrap_or("")
                .to_owned()
        },
    );
    let fault = format!("{fault:?}");
    let fault = fault.split([' ', '{', '(']).next().unwrap_or("");
    let _ = config;
    format!("{fault} at {step}")
}

/// Before bn-1i050 the two paths parted on seven classes, each a perturbation the
/// model refused and the lift admitted: a finalize with no drain report; a
/// cancellation's effect abort or timer drop before the task's acknowledgement; a task
/// completing as cancelled while it held an obligation or an armed timer; a parked task
/// arming a timer; a sleeping task resuming before its timer fired. The lift now refuses
/// each (`Nonconformance::FinalizeWithoutDrain`, `EffectFault::OutsideCancelling`,
/// `TimeFault::{OutsideCancelling, NotRunning, WokenBeforeFire, Late, OutlivesTask}`,
/// `LedgerFault::HolderTerminal`), so the sweep requires agreement in both directions.
///
/// One known divergence remained after bn-j1a50, in the other direction, and this pin
/// held it: RFC 0026 correction 51's "known strictness". The calculus decided
/// "cancelling" per region, at the cancel request, and the model per task, at the
/// task's acknowledgement, so 33 perturbations (a transfer or a committed discharge
/// between a cancel request and the holder's acknowledgement) were admitted by the model
/// and refused by the calculus. RFC 0026 correction 53 (bn-36wy3) gave the calculus the
/// per-task acknowledgement and relaxed the rule to the model's, and the count is zero.
/// The pin is now the regression guard that it stays there: a window test
/// ([`in_strictness_window`]) that matched anything again would fail it.
const KNOWN_STRICTNESS: usize = 0;

/// Whether the lift's refusal at `seq` is correction 51's known strictness: a
/// substrate transfer or committed discharge refused because the region is cancelling,
/// at a point where a cancel request precedes it and neither party has acknowledged.
fn in_strictness_window(variant: &Journal, seq: u64, reason: &Nonconformance) -> bool {
    if !matches!(
        reason,
        Nonconformance::Refused(
            RegionFault::SubstrateRegionNotOpen { .. }
                | RegionFault::SubstrateCommitDuringCancellation { .. }
        )
    ) {
        return false;
    }
    let at = usize::try_from(seq).unwrap();
    let prefix = &bodies(variant)[..at];
    let (obligation, target) = match variant.events()[at].body() {
        EventBody::Obligation(ObligationEvent::Transferred {
            obligation, holder, ..
        }) => (*obligation, Some(*holder)),
        EventBody::Obligation(ObligationEvent::Discharged {
            obligation,
            how: Discharge::Committed,
        }) => (*obligation, None),
        _ => return false,
    };
    let holder = prefix.iter().rev().find_map(|e| match e {
        EventBody::Obligation(
            ObligationEvent::Opened {
                obligation: o,
                holder,
                ..
            }
            | ObligationEvent::Transferred {
                obligation: o,
                holder,
                ..
            },
        ) if *o == obligation => Some(*holder),
        _ => None,
    });
    let requested = prefix.iter().any(|e| {
        matches!(
            e,
            EventBody::Lifecycle(LifecycleEvent::RegionCancelRequested { .. })
        )
    });
    let acknowledged = |task: TaskOrdinal| {
        prefix.iter().any(|e| {
            matches!(e, EventBody::Cancellation(CancellationEvent::Acknowledged { task: t }) if *t == task)
        })
    };
    requested && holder.is_some_and(|h| !acknowledged(h)) && target.is_none_or(|t| !acknowledged(t))
}

#[test]
fn deletions_and_swaps_get_the_same_verdict_from_the_model_and_the_lift() {
    let mut known_strictness = 0_usize;
    let mut compared = 0_usize;
    let mut rejected_by_both = 0_usize;
    let mut disagreements: BTreeMap<String, usize> = BTreeMap::new();
    let mut examples = Vec::new();
    for entry in corpus() {
        for log in entry.logs.iter().step_by(25) {
            let journal = journal_of(&entry, log);
            let b = bodies(&journal);
            let mut variants = Vec::new();
            for i in 0..b.len() {
                let mut deleted = b.clone();
                deleted.remove(i);
                variants.push((format!("delete {i}"), rebuild(deleted)));
                if i + 1 < b.len() && b[i] != b[i + 1] {
                    let mut swapped = b.clone();
                    swapped.swap(i, i + 1);
                    variants.push((format!("swap {i}"), rebuild(swapped)));
                }
            }
            for (what, variant) in variants {
                let verdict = judge(&entry.config, &variant);
                let by_model = verdict.is_accepted();
                let by_lift = lift_accepts(&variant);
                compared += 1;
                match (by_model, by_lift) {
                    (false, false) => rejected_by_both += 1,
                    (true, true) => {}
                    // The lift admits what the model refuses: a gap in the lift.
                    (false, true) => {
                        let class = class_of(&entry.config, &variant, &verdict);
                        if examples.len() < 20 {
                            examples.push(format!("{} log {log} {what}: {class}", entry.name));
                        }
                        *disagreements
                            .entry(format!("lift admits: {class}"))
                            .or_default() += 1;
                    }
                    // The model admits what the lift refuses: a hole in the model.
                    (true, false) => {
                        if let LiftVerdict::Violates { seq, reason } = lift(&variant) {
                            if in_strictness_window(&variant, seq, &reason) {
                                known_strictness += 1;
                                continue;
                            }
                        }
                        let reason = match lift(&variant) {
                            LiftVerdict::Violates { reason, .. } => format!("{reason:?}"),
                            other => format!("{other:?}"),
                        };
                        let reason = reason
                            .split([' ', '{', '('])
                            .take(2)
                            .collect::<Vec<_>>()
                            .join(" ");
                        if examples.len() < 20 {
                            examples.push(format!(
                                "{} log {log} {what}: model accepts, lift {reason}",
                                entry.name
                            ));
                        }
                        *disagreements
                            .entry(format!("model admits: {reason}"))
                            .or_default() += 1;
                    }
                }
            }
        }
    }
    eprintln!(
        "compared {compared}, rejected by both {rejected_by_both}, disagreements {disagreements:?}"
    );
    assert_eq!(compared, 25_277, "docs/18 C023 cites this count");
    assert_eq!(
        known_strictness, KNOWN_STRICTNESS,
        "RFC 0026 correction 51's known strictness, relaxed by correction 53 (bn-36wy3); docs/18 C023 cites it"
    );
    assert!(
        disagreements.is_empty(),
        "{disagreements:?}\n{}",
        examples.join("\n")
    );
    assert!(
        rejected_by_both > compared / 2,
        "{rejected_by_both} of {compared}"
    );
}

// --- the wire ------------------------------------------------------------------------

#[test]
fn the_model_reads_the_canonical_bytes_and_types_what_it_cannot_judge() {
    let config = all_families();
    let programs = ledger();
    let log = ChoiceLog::enumerate(&lengths(&programs)).remove(0);
    let journal = run(&programs, &log, &config).unwrap();
    let bytes = journal.encode().unwrap();
    assert_eq!(steps_of(&journal).len(), journal.len());

    // Every proper prefix is malformed, never a shorter accepted trace.
    for cut in 0..bytes.len() {
        assert!(
            matches!(
                model::judge(&alphabet(&config), &bytes[..cut]),
                Verdict::Malformed(_)
            ),
            "prefix {cut}"
        );
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(matches!(
        model::judge(&alphabet(&config), &trailing),
        Verdict::Malformed(WireFault::Trailing { .. })
    ));
    // The first event's family byte sits after the header and its sequence number.
    let family_at = b"continuum/semantic-journal\n".len() + 4 + 8 + 8;
    let mut unknown = bytes.clone();
    unknown[family_at] = 9;
    assert!(matches!(
        model::judge(&alphabet(&config), &unknown),
        Verdict::Malformed(WireFault::Tag {
            table: "family",
            tag: 9,
            ..
        })
    ));
    // Tag 7 names no family: malformed, as the adapter's decoder reads it too.
    let mut seventh = bytes.clone();
    seventh[family_at] = 7;
    assert!(matches!(
        model::judge(&alphabet(&config), &seventh),
        Verdict::Malformed(WireFault::Tag { tag: 7, .. })
    ));
    // A journal observed with fewer families than the model is told is a family the
    // alphabet does not observe, not a silent pass.
    let lifecycle_only = Alphabet::new([]);
    assert!(matches!(
        model::judge(&lifecycle_only, &bytes),
        Verdict::Rejected {
            fault: Fault::OutsideAlphabet,
            ..
        }
    ));
}

// --- truncation (cr-3pu5cu) ----------------------------------------------------------

// A copy of `support/truncation.rs::mid_protocol`: the rule `a7-model-independent`
// lets this witness load the model file only. `pr14_deadline_cancellation.rs` checks
// that the two copies are the same text.
/// Whether a journal prefix ends inside a protocol the substrate completes within one
/// operation. This is the test's own definition of a *genuinely complete* prefix, stated
/// from the event list alone, not from the lift: a prefix is complete when it ends
/// outside every one of these, and mid-protocol otherwise.
///
/// - a task's cancellation phase not yet absorbed by the drain that lists it or by the
///   task's own lifecycle `cancel`;
/// - a task's own `cancel-requested` without its `cancel`;
/// - a region's cancel request whose region has not been drained;
/// - a drained region not finalized;
/// - with the obligations family in the prefix, a finalized region not settled;
/// - a timer whose deadline the clock has passed, neither fired nor cancelled.
///
/// A parked task, a held reservation, an open obligation, an armed timer that is not
/// due, a blocked channel operation, and a normally closing region that waits for owned
/// work are states a run can end in: prefixes ending there are complete.
///
/// Completeness is relative to the families a prefix carries. A journal has no alphabet
/// header, so a prefix with no event of a family reads as that family's projection,
/// which is complete without it: before its first obligation event, a finalized region
/// owes no settle.
fn mid_protocol(prefix: &[EventBody]) -> Option<&'static str> {
    let obligations_observed = prefix
        .iter()
        .any(|body| matches!(body, EventBody::Obligation(_)));
    let mut phased = BTreeSet::new();
    let mut absorbed = BTreeSet::new();
    let mut alone = BTreeSet::new();
    let mut ended_alone = BTreeSet::new();
    let mut cancel_requested = BTreeSet::new();
    let mut drained = BTreeSet::new();
    let mut finalized = BTreeSet::new();
    let mut settled = BTreeSet::new();
    let mut timers: BTreeMap<u32, u64> = BTreeMap::new();
    let mut now = 0;
    for body in prefix {
        match body {
            EventBody::Cancellation(event) => {
                phased.insert(event.task());
            }
            EventBody::Lifecycle(LifecycleEvent::TaskStepped { task, step }) => match step {
                TaskStep::CancelRequested => {
                    alone.insert(*task);
                }
                TaskStep::Cancel => {
                    ended_alone.insert(*task);
                    absorbed.insert(*task);
                }
                _ => {}
            },
            EventBody::Lifecycle(LifecycleEvent::RegionCancelRequested { region }) => {
                cancel_requested.insert(*region);
            }
            EventBody::Lifecycle(LifecycleEvent::RegionDrained { region, cancelled }) => {
                drained.insert(*region);
                absorbed.extend(cancelled.as_slice().iter().copied());
            }
            EventBody::Lifecycle(LifecycleEvent::RegionFinalized { region }) => {
                finalized.insert(*region);
            }
            EventBody::Obligation(ObligationEvent::RegionSettled { region, .. }) => {
                settled.insert(*region);
            }
            EventBody::Time(TimeEvent::Scheduled {
                timer, deadline, ..
            }) => {
                timers.insert(timer.0, deadline.0);
            }
            EventBody::Time(
                TimeEvent::Fired { timer, .. } | TimeEvent::Cancelled { timer, .. },
            ) => {
                timers.remove(&timer.0);
            }
            EventBody::Time(TimeEvent::Advanced { to, .. }) => now = to.0,
            _ => {}
        }
    }
    if phased.iter().any(|task| !absorbed.contains(task)) {
        return Some("a cancellation not absorbed");
    }
    if alone.iter().any(|task| !ended_alone.contains(task)) {
        return Some("a single-task cancellation not ended");
    }
    if cancel_requested
        .iter()
        .any(|region| !drained.contains(region))
    {
        return Some("a region's cancellation not drained");
    }
    if drained.iter().any(|region| !finalized.contains(region)) {
        return Some("a drain not finalized");
    }
    if obligations_observed && finalized.iter().any(|region| !settled.contains(region)) {
        return Some("a finalize not settled");
    }
    if timers.values().any(|deadline| *deadline <= now) {
        return Some("a due timer not fired");
    }
    None
}

/// The model's verdict on `journal` over the families `bodies` carry.
fn judge_carried(bodies: &[EventBody], journal: &Journal) -> Verdict {
    let mut config = BindingConfig::new(SEED);
    for family in [
        Family::Effect,
        Family::Cancellation,
        Family::Obligation,
        Family::Time,
        Family::Channel,
    ] {
        if bodies.iter().any(|body| body.family() == family) {
            config = config.observing(family);
        }
    }
    judge(&config, journal)
}

/// Every strict prefix of every corpus journal: a mid-protocol prefix never lifts as
/// `Conforms` (a truncated journal is typed, `Inconclusive(InsufficientTelemetry)` or a
/// violation), and a complete prefix always does. The model, judging each prefix over
/// the families the prefix carries, agrees with the lift on every one.
#[test]
fn truncated_journals_do_not_conform_and_complete_prefixes_do() {
    let mut mid = BTreeMap::<&str, usize>::new();
    let mut complete = 0_usize;
    for entry in corpus() {
        for log in &entry.logs {
            let journal = journal_of(&entry, log);
            let all = bodies(&journal);
            for cut in 0..all.len() {
                let prefix = rebuild(all[..cut].iter().cloned());
                let conforms = lift_accepts(&prefix);
                match mid_protocol(&all[..cut]) {
                    Some(what) => {
                        assert!(
                            !conforms,
                            "{} log {log} cut {cut} ({what}) lifts as Conforms\n{}",
                            entry.name,
                            prefix.render()
                        );
                        // A cut between a region's finalize and its settle is a missing
                        // report, not a contradiction: it must type
                        // `Inconclusive(InsufficientTelemetry)`, never a violation
                        // (bn-eaxx0, found by the bn-28hup adversarial pass).
                        if what == "a finalize not settled" {
                            assert!(
                                matches!(
                                    lift(&prefix),
                                    LiftVerdict::Inconclusive {
                                        reason:
                                            continuum_value::assurance::InconclusiveReason::InsufficientTelemetry,
                                        ..
                                    }
                                ),
                                "{} log {log} cut {cut} ({what}) is not Inconclusive: {:?}\n{}",
                                entry.name,
                                lift(&prefix),
                                prefix.render()
                            );
                        }
                        *mid.entry(what).or_default() += 1;
                    }
                    None => {
                        assert!(
                            conforms,
                            "{} log {log} cut {cut}: a complete prefix does not conform: {:?}\n{}",
                            entry.name,
                            lift(&prefix),
                            prefix.render()
                        );
                        complete += 1;
                    }
                }
                assert_eq!(
                    judge_carried(&all[..cut], &prefix).is_accepted(),
                    conforms,
                    "{} log {log} cut {cut}: the model and the lift disagree",
                    entry.name
                );
            }
        }
    }
    eprintln!("mid-protocol prefixes {mid:?}, complete {complete}");
    // Non-vacuity: each kind of truncation occurs, and so do complete prefixes.
    for what in [
        "a cancellation not absorbed",
        "a single-task cancellation not ended",
        "a drain not finalized",
        "a finalize not settled",
    ] {
        assert!(mid.get(what).copied().unwrap_or(0) > 0, "{what}: {mid:?}");
    }
    assert!(complete > 10_000, "{complete}");
}

/// Anti-vacuity: the truncation the review found. A deadline journal cut just before
/// its task's lifecycle `cancel` is `Inconclusive(InsufficientTelemetry)`, and putting
/// that one event back makes the same journal conform.
#[test]
fn a_deadline_journal_cut_before_its_cancel_is_inconclusive() {
    let entry = corpus()
        .into_iter()
        .find(|entry| entry.name == "deadlines")
        .expect("the deadline entry");
    let (journal, end) = entry
        .logs
        .iter()
        .find_map(|log| {
            let journal = journal_of(&entry, log);
            let end = position(&journal, |b| {
                matches!(
                    b,
                    EventBody::Lifecycle(LifecycleEvent::TaskStepped {
                        step: TaskStep::Cancel,
                        ..
                    })
                )
            })?;
            Some((journal, end))
        })
        .expect("a deadline cancellation in the corpus");
    let all = bodies(&journal);
    let truncated = rebuild(all[..end].iter().cloned());
    assert!(
        matches!(
            lift(&truncated),
            LiftVerdict::Inconclusive {
                reason: continuum_value::assurance::InconclusiveReason::InsufficientTelemetry,
                ..
            }
        ),
        "{:?}",
        lift(&truncated)
    );
    assert!(!judge(&entry.config, &truncated).is_accepted());
    let restored = rebuild(all[..=end].iter().cloned());
    assert!(lift_accepts(&restored), "{:?}", lift(&restored));
}

/// Anti-vacuity: a journal cut between a region's finalize and its settle is
/// `Inconclusive(InsufficientTelemetry)`, never `Violates(Unsettled)` (bn-eaxx0, found
/// by the bn-28hup adversarial pass). The old lift rejected this cut with a violation:
/// this test fails against that behaviour. Putting the settle back makes the same
/// journal conform.
#[test]
fn a_journal_cut_between_finalize_and_settle_is_inconclusive() {
    let (all, end) = corpus()
        .into_iter()
        .find_map(|entry| {
            entry.logs.iter().find_map(|log| {
                let all = bodies(&journal_of(&entry, log));
                // The cut right before a settle: exactly "a finalize not settled" rules
                // out a log whose cut also lands inside some other unresolved protocol
                // (a sibling task's cancellation, say), which would report that instead.
                let cut = (0..all.len()).find(|cut| {
                    matches!(
                        all.get(*cut),
                        Some(EventBody::Obligation(ObligationEvent::RegionSettled { .. }))
                    ) && mid_protocol(&all[..*cut]) == Some("a finalize not settled")
                })?;
                Some((all, cut))
            })
        })
        .expect("a clean finalize-not-settled cut somewhere in the corpus");
    let truncated = rebuild(all[..end].iter().cloned());
    assert_eq!(
        mid_protocol(&all[..end]),
        Some("a finalize not settled"),
        "{}",
        truncated.render()
    );
    assert!(
        matches!(
            lift(&truncated),
            LiftVerdict::Inconclusive {
                reason: continuum_value::assurance::InconclusiveReason::InsufficientTelemetry,
                ..
            }
        ),
        "{:?}",
        lift(&truncated)
    );
    assert!(!judge_carried(&all[..end], &truncated).is_accepted());
    let restored = rebuild(all[..=end].iter().cloned());
    assert!(lift_accepts(&restored), "{:?}", lift(&restored));
}

/// Regression, bn-eaxx0's own adversarial pass: a region finalized over an obligation
/// its holder ended still holding, never leaked and never settled, is a violation
/// (`LedgerFault::UnbalancedAtClose`), never merely absent telemetry, even when the
/// journal is not truncated and unrelated legitimate work follows. `finish` must read
/// this family's own ledger for the region, not only whether a settle report arrived,
/// or the fix for the finalize/settle truncation case would have reopened this hole:
/// before it, the old unconditional `Violates(Unsettled)` caught this by accident.
#[test]
fn a_finalized_region_with_an_open_obligation_and_no_settle_is_unbalanced_not_inconclusive() {
    use continuum_asupersync::family::obligation::LedgerFault;
    let r = RegionOrdinal;
    let t = TaskOrdinal;
    let open_region = |region, parent| {
        EventBody::Lifecycle(LifecycleEvent::RegionOpened {
            region: r(region),
            parent: r(parent),
        })
    };
    let spawn = |task, region| {
        EventBody::Lifecycle(LifecycleEvent::TaskSpawned {
            task: t(task),
            region: r(region),
            resumability: Resumability::Resumable,
        })
    };
    let step = |task, step| {
        EventBody::Lifecycle(LifecycleEvent::TaskStepped {
            task: t(task),
            step,
        })
    };
    let close =
        |region| EventBody::Lifecycle(LifecycleEvent::RegionCloseRequested { region: r(region) });
    let drained = |region| {
        EventBody::Lifecycle(LifecycleEvent::RegionDrained {
            region: r(region),
            cancelled: TaskSet::new([]),
        })
    };
    let finalized =
        |region| EventBody::Lifecycle(LifecycleEvent::RegionFinalized { region: r(region) });
    let ob_open = |o, holder, region| {
        EventBody::Obligation(ObligationEvent::Opened {
            obligation: ObligationOrdinal(o),
            kind: ObligationKind::Lease,
            holder: t(holder),
            region: r(region),
        })
    };

    let forged = rebuild([
        open_region(1, 0),
        spawn(0, 1),
        step(0, TaskStep::Begin),
        ob_open(0, 0, 1),
        // t0 completes without discharging or leaking o0: the calculus's own `Complete`
        // does not check held obligations (only `Cancelled` does, docs/02 §7), so this
        // is admitted.
        step(0, TaskStep::Complete),
        close(1),
        drained(1),
        finalized(1),
        // No `RegionSettled` for r1 ever comes. The journal is not truncated:
        // legitimate, unrelated work for another task follows.
        spawn(1, 0),
        step(1, TaskStep::Begin),
        step(1, TaskStep::Complete),
    ]);
    match lift(&forged) {
        LiftVerdict::Violates {
            reason:
                Nonconformance::Obligation(LedgerFault::UnbalancedAtClose {
                    region: 1,
                    open,
                    leaked,
                }),
            ..
        } => {
            assert_eq!(open, vec![0]);
            assert!(leaked.is_empty());
        }
        other => panic!("{other:?}\n{}", forged.render()),
    }
}

/// A two-region forged journal (cr-19ec8g): `r1` and `r2`, siblings under root or `r2`
/// nested inside `r1`, each with one task that opens one obligation. `dirty` names
/// which region (by ordinal, `1` or `2`) leaves its obligation open when its task
/// completes; the other discharges cleanly. `None` leaves both clean. Neither region
/// ever gets a `RegionSettled`. `r2` finalizes first (so a nested `r1` may finalize
/// once its child already has), then `r1`; legitimate, unrelated work for a third task
/// follows, so the journal is not truncated.
fn two_region_no_settle_events(nested: bool, dirty: Option<u32>) -> Vec<EventBody> {
    let r = RegionOrdinal;
    let t = TaskOrdinal;
    let open_region = |region, parent| {
        EventBody::Lifecycle(LifecycleEvent::RegionOpened {
            region: r(region),
            parent: r(parent),
        })
    };
    let spawn = |task, region| {
        EventBody::Lifecycle(LifecycleEvent::TaskSpawned {
            task: t(task),
            region: r(region),
            resumability: Resumability::Resumable,
        })
    };
    let step = |task, step| {
        EventBody::Lifecycle(LifecycleEvent::TaskStepped {
            task: t(task),
            step,
        })
    };
    let close =
        |region| EventBody::Lifecycle(LifecycleEvent::RegionCloseRequested { region: r(region) });
    let drained = |region| {
        EventBody::Lifecycle(LifecycleEvent::RegionDrained {
            region: r(region),
            cancelled: TaskSet::new([]),
        })
    };
    let finalized =
        |region| EventBody::Lifecycle(LifecycleEvent::RegionFinalized { region: r(region) });
    let ob_open = |o, holder, region| {
        EventBody::Obligation(ObligationEvent::Opened {
            obligation: ObligationOrdinal(o),
            kind: ObligationKind::Lease,
            holder: t(holder),
            region: r(region),
        })
    };
    let ob_dis = |o| {
        EventBody::Obligation(ObligationEvent::Discharged {
            obligation: ObligationOrdinal(o),
            how: Discharge::Committed,
        })
    };

    let parent_of_r2 = if nested { 1 } else { 0 };
    let mut events = vec![
        open_region(1, 0),
        open_region(2, parent_of_r2),
        spawn(0, 1),
        step(0, TaskStep::Begin),
        ob_open(0, 0, 1),
    ];
    if dirty != Some(1) {
        events.push(ob_dis(0));
    }
    events.push(step(0, TaskStep::Complete));
    events.extend([spawn(1, 2), step(1, TaskStep::Begin), ob_open(1, 1, 2)]);
    if dirty != Some(2) {
        events.push(ob_dis(1));
    }
    events.push(step(1, TaskStep::Complete));
    // r2 finalizes first: a nested r1 may finalize once its child already has.
    events.extend([close(2), drained(2), finalized(2)]);
    events.extend([close(1), drained(1), finalized(1)]);
    // Legitimate, unrelated work follows: the journal is not truncated.
    events.extend([
        spawn(2, 0),
        step(2, TaskStep::Begin),
        step(2, TaskStep::Complete),
    ]);
    events
}

/// Regression, cr-19ec8g: sibling regions, one finalized-unsettled with a clean balance
/// and one finalized-unsettled with an open obligation, in both region orders. The
/// clean one's incomplete must never mask the other's violation, whichever region
/// (lower or higher ordinal) `finish` scans first: it must scan every finalized region,
/// not stop at the first unsettled one.
#[test]
fn sibling_regions_one_clean_one_unbalanced_missing_settlement_is_unbalanced_at_close() {
    use continuum_asupersync::family::obligation::LedgerFault;
    for dirty in [1_u32, 2] {
        let events = two_region_no_settle_events(false, Some(dirty));
        let forged = rebuild(events.iter().cloned());
        match lift(&forged) {
            LiftVerdict::Violates {
                reason:
                    Nonconformance::Obligation(LedgerFault::UnbalancedAtClose {
                        region,
                        open,
                        leaked,
                    }),
                ..
            } => {
                assert_eq!(region, dirty, "dirty={dirty}\n{}", forged.render());
                assert_eq!(open.len(), 1, "dirty={dirty}");
                assert!(leaked.is_empty(), "dirty={dirty}");
            }
            other => panic!("dirty={dirty}: {other:?}\n{}", forged.render()),
        }
        assert!(
            !judge_carried(&events, &forged).is_accepted(),
            "dirty={dirty}: the model accepts it"
        );
    }
}

/// The same, `r2` nested inside `r1` instead of a sibling of it.
#[test]
fn nested_regions_one_clean_one_unbalanced_missing_settlement_is_unbalanced_at_close() {
    use continuum_asupersync::family::obligation::LedgerFault;
    for dirty in [1_u32, 2] {
        let events = two_region_no_settle_events(true, Some(dirty));
        let forged = rebuild(events.iter().cloned());
        match lift(&forged) {
            LiftVerdict::Violates {
                reason:
                    Nonconformance::Obligation(LedgerFault::UnbalancedAtClose {
                        region,
                        open,
                        leaked,
                    }),
                ..
            } => {
                assert_eq!(region, dirty, "dirty={dirty}\n{}", forged.render());
                assert_eq!(open.len(), 1, "dirty={dirty}");
                assert!(leaked.is_empty(), "dirty={dirty}");
            }
            other => panic!("dirty={dirty}: {other:?}\n{}", forged.render()),
        }
        assert!(
            !judge_carried(&events, &forged).is_accepted(),
            "dirty={dirty}: the model accepts it"
        );
    }
}

/// Control, cr-19ec8g: two finalized-unsettled regions, both with a clean balance, over
/// both topologies, is `Inconclusive`, never a violation — the full scan for a
/// violation must not manufacture one where the ledger is clean everywhere, and the
/// model agrees.
#[test]
fn two_regions_both_clean_missing_settlement_is_inconclusive() {
    for nested in [false, true] {
        let events = two_region_no_settle_events(nested, None);
        let forged = rebuild(events.iter().cloned());
        assert!(
            matches!(
                lift(&forged),
                LiftVerdict::Inconclusive {
                    reason: continuum_value::assurance::InconclusiveReason::InsufficientTelemetry,
                    ..
                }
            ),
            "nested={nested}: {:?}\n{}",
            lift(&forged),
            forged.render()
        );
        assert!(
            !judge_carried(&events, &forged).is_accepted(),
            "nested={nested}: the model accepts it"
        );
    }
}

// --- a task's own cancellation ends by its own `cancel` (cr-3pu5cu, th-1shbkn) -------

/// Every family projection: lifecycle, with each subset of the other five families.
fn every_projection() -> Vec<BindingConfig> {
    let others = [
        Family::Effect,
        Family::Cancellation,
        Family::Obligation,
        Family::Time,
        Family::Channel,
    ];
    (0_u32..32)
        .map(|mask| {
            let mut config = BindingConfig::new(SEED);
            for (bit, family) in others.iter().enumerate() {
                if mask & (1 << bit) != 0 {
                    config = config.observing(*family);
                }
            }
            config
        })
        .collect()
}

/// Runs in which `t0` ends by its own deadline and a region's cancellation then drains
/// its region: from `Running` with its own region cancelled, from `Created` with an
/// ancestor region cancelled, and from `Suspended` (a due timer wakes it) with its own
/// region cancelled.
fn own_then_region() -> Vec<(&'static str, Program)> {
    let deadline = |region: RegionLabel, task: TaskLabel| SubstrateOp::SpawnWithDeadline {
        region,
        task,
        resumability: Resumability::Resumable,
        deadline: 10,
    };
    vec![
        (
            "running, own region",
            vec![
                open(ROOT, R[1]),
                deadline(R[1], T[1]),
                begin(T[1]),
                SubstrateOp::Advance { nanos: 20 },
                SubstrateOp::Continue { task: T[1] },
                SubstrateOp::Cancel { region: R[1] },
            ],
        ),
        (
            "created, ancestor region",
            vec![
                open(ROOT, R[1]),
                open(R[1], R[2]),
                deadline(R[2], T[1]),
                SubstrateOp::Advance { nanos: 20 },
                begin(T[1]),
                SubstrateOp::Cancel { region: R[1] },
            ],
        ),
        (
            "suspended, own region",
            vec![
                open(ROOT, R[1]),
                deadline(R[1], T[1]),
                begin(T[1]),
                SubstrateOp::Sleep {
                    task: T[1],
                    nanos: 40,
                },
                SubstrateOp::Advance { nanos: 50 },
                SubstrateOp::Cancel { region: R[1] },
            ],
        ),
    ]
}

fn is_own_cancel(body: &EventBody) -> bool {
    matches!(
        body,
        EventBody::Lifecycle(LifecycleEvent::TaskStepped {
            task: TaskOrdinal(0),
            step: TaskStep::Cancel,
        })
    )
}

/// The splice the review found: a task's own `cancel-requested` (and, with the
/// cancellation family, its `requested`/`acknowledged`/`cancelled` with the `deadline`
/// cause) whose own lifecycle `cancel` is removed, and a later region cancel, drain and
/// finalize that list the task. The drain leaves the worker `Cancelled` as the own step
/// would, so a check that reads the shared terminal state accepts it. RFC 0026
/// correction 53 items 3 and 6 end a task's own cancellation with its own `cancel`,
/// within the operation that observed it, so under every family projection the lift
/// reports `InterruptedOwnCancel` at the region's cancel request and the model
/// `OwnCancellationInterrupted`. The unspliced journal conforms in both.
#[test]
fn a_region_drain_does_not_end_a_task_s_own_cancellation() {
    let mut spliced = 0_usize;
    for (name, program) in own_then_region() {
        let log = ChoiceLog::new(vec![0; program.len()]);
        let programs = vec![program];
        for config in every_projection() {
            let journal = run(&programs, &log, &config)
                .unwrap_or_else(|refusal| panic!("{name} {:?}: {refusal}", config.families));
            assert!(
                lift_accepts(&journal) && judge(&config, &journal).is_accepted(),
                "{name} {:?}: {:?} / {}\n{}",
                config.families,
                lift(&journal),
                judge(&config, &journal),
                journal.render()
            );
            let mut b = bodies(&journal);
            let end = b.iter().position(is_own_cancel).expect("t0 ends alone");
            b.remove(end);
            let drain = b
                .iter()
                .position(|e| {
                    matches!(
                        e,
                        EventBody::Lifecycle(LifecycleEvent::RegionDrained { .. })
                    )
                })
                .expect("the region's drain");
            let EventBody::Lifecycle(LifecycleEvent::RegionDrained { region, cancelled }) =
                &b[drain]
            else {
                unreachable!()
            };
            assert!(cancelled.as_slice().is_empty(), "{name}: {cancelled:?}");
            b[drain] = EventBody::Lifecycle(LifecycleEvent::RegionDrained {
                region: *region,
                cancelled: TaskSet::new([TaskOrdinal(0)]),
            });
            let variant = rebuild(b);
            assert!(
                matches!(
                    lift(&variant),
                    LiftVerdict::Violates {
                        reason: Nonconformance::Cancellation(
                            continuum_asupersync::family::cancellation::CancellationFault::InterruptedOwnCancel {
                                task: 0,
                                ..
                            }
                        ),
                        ..
                    }
                ),
                "{name} {:?}: {:?}\n{}",
                config.families,
                lift(&variant),
                variant.render()
            );
            let verdict = judge(&config, &variant);
            assert!(
                matches!(
                    verdict,
                    Verdict::Rejected {
                        fault: Fault::OwnCancellationInterrupted(0),
                        ..
                    }
                ),
                "{name} {:?}: {verdict}",
                config.families
            );
            spliced += 1;
        }
    }
    assert_eq!(spliced, 3 * 32);
}

/// The same rule for a normal end: the task's own lifecycle `cancel` replaced by
/// `resume`, `complete`. Only the task's own `cancel` is a lifecycle step inside its own
/// cancellation, so under every projection the lift reports `InterruptedOwnCancel` at
/// the `resume` and the model `OwnCancellationInterrupted` (cr-3pu5cu round 6). With the
/// `resume` dropped, the `complete` alone is refused the same way.
#[test]
fn a_normal_end_does_not_end_a_task_s_own_cancellation() {
    let (name, program) = own_then_region().remove(0);
    let log = ChoiceLog::new(vec![0; program.len()]);
    let programs = vec![program];
    let mut checked = 0_usize;
    for config in every_projection() {
        let journal = run(&programs, &log, &config).unwrap();
        for with_resume in [true, false] {
            let mut b = bodies(&journal);
            let end = b.iter().position(is_own_cancel).expect("t0 ends alone");
            b[end] = EventBody::Lifecycle(LifecycleEvent::TaskStepped {
                task: TaskOrdinal(0),
                step: TaskStep::Complete,
            });
            if with_resume {
                b.insert(
                    end,
                    EventBody::Lifecycle(LifecycleEvent::TaskStepped {
                        task: TaskOrdinal(0),
                        step: TaskStep::Resume,
                    }),
                );
            }
            let variant = rebuild(b);
            let lifted = lift(&variant);
            let verdict = judge(&config, &variant);
            assert!(
                matches!(
                    lifted,
                    LiftVerdict::Violates {
                        reason: Nonconformance::Cancellation(
                            continuum_asupersync::family::cancellation::CancellationFault::InterruptedOwnCancel {
                                task: 0,
                                ..
                            }
                        ),
                        ..
                    }
                ),
                "{name} {:?} resume={with_resume}: {lifted:?}",
                config.families
            );
            assert!(
                matches!(
                    verdict,
                    Verdict::Rejected {
                        fault: Fault::OwnCancellationInterrupted(0),
                        ..
                    }
                ),
                "{name} {:?} resume={with_resume}: {verdict}",
                config.families
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 2 * 32);
}

// --- family presence is the journal's, not the first event's (pre-review pass) ------

/// Hand-built journals from the pre-review adversarial pass. Each once lifted as
/// `Conforms` (or, for the complete-after-phase one, `Inconclusive`), while the model
/// rejected it: a check that turned on a family's presence was skipped because that
/// family's first event came later, a `complete` followed a reported phase, a task was
/// polled past its deadline as if it had none, or a region event came inside a task's
/// own cancellation. Now the lift reports each as the named violation, and the model,
/// judging over the families the journal carries, rejects each.
#[test]
fn late_family_events_and_interleavings_do_not_hide_a_violation() {
    use continuum_asupersync::family::cancellation::CancellationFault;
    use continuum_asupersync::family::channel::ChannelOrdinal;
    use continuum_asupersync::family::effect::ReservationOrdinal;
    use continuum_asupersync::family::time::{TimeFault, TimerOrdinal};

    let t = TaskOrdinal;
    let r = RegionOrdinal;
    let open = |region: u32, parent: u32| {
        EventBody::Lifecycle(LifecycleEvent::RegionOpened {
            region: r(region),
            parent: r(parent),
        })
    };
    let spawn = |task: u32, region: u32| {
        EventBody::Lifecycle(LifecycleEvent::TaskSpawned {
            task: t(task),
            region: r(region),
            resumability: Resumability::Resumable,
        })
    };
    let step = |task: u32, step: TaskStep| {
        EventBody::Lifecycle(LifecycleEvent::TaskStepped {
            task: t(task),
            step,
        })
    };
    let cancel_region = |region: u32| {
        EventBody::Lifecycle(LifecycleEvent::RegionCancelRequested { region: r(region) })
    };
    let drained = |region: u32, cancelled: &[u32]| {
        EventBody::Lifecycle(LifecycleEvent::RegionDrained {
            region: r(region),
            cancelled: TaskSet::new(cancelled.iter().map(|c| t(*c))),
        })
    };
    let finalized =
        |region: u32| EventBody::Lifecycle(LifecycleEvent::RegionFinalized { region: r(region) });
    let requested = |task: u32, cause: CancelCause| {
        EventBody::Cancellation(CancellationEvent::Requested {
            task: t(task),
            cause,
        })
    };
    let ack =
        |task: u32| EventBody::Cancellation(CancellationEvent::Acknowledged { task: t(task) });
    let cancelled = |task: u32, cause: CancelCause| {
        EventBody::Cancellation(CancellationEvent::Cancelled {
            task: t(task),
            cause,
        })
    };
    let deadline = |task: u32, at: u64| {
        EventBody::Time(TimeEvent::Deadline {
            task: t(task),
            at: VirtualInstant(at),
        })
    };
    let advanced = |from: u64, to: u64| {
        EventBody::Time(TimeEvent::Advanced {
            from: VirtualInstant(from),
            to: VirtualInstant(to),
        })
    };
    let reserved = |task: u32| {
        EventBody::Effect(EffectEvent::Reserved {
            reservation: ReservationOrdinal(0),
            task: t(task),
        })
    };
    let cancel_abort = EventBody::Effect(EffectEvent::Aborted {
        reservation: ReservationOrdinal(0),
        cause: AbortCause::Cancel,
    });
    let own_phases = |task: u32| {
        vec![
            requested(task, CancelCause::Deadline),
            ack(task),
            cancelled(task, CancelCause::Deadline),
        ]
    };

    type Expect = fn(&Nonconformance) -> bool;
    let mut cases: Vec<(&str, Vec<EventBody>, Expect)> = Vec::new();

    // S7: a region's cancel abort before the task's cancellation phases.
    cases.push((
        "region cleanup before phases",
        vec![
            open(1, 0),
            spawn(0, 1),
            step(0, TaskStep::Begin),
            reserved(0),
            cancel_region(1),
            cancel_abort.clone(),
            requested(0, CancelCause::User),
            ack(0),
            cancelled(0, CancelCause::User),
            drained(1, &[0]),
            finalized(1),
        ],
        |n| matches!(n, Nonconformance::Effect(_)),
    ));
    // S3: a deadline's cancel abort before the task's phases.
    let mut s3 = vec![
        spawn(0, 0),
        deadline(0, 10),
        step(0, TaskStep::Begin),
        reserved(0),
        advanced(0, 20),
        step(0, TaskStep::CancelRequested),
        cancel_abort.clone(),
    ];
    s3.extend(own_phases(0));
    s3.push(step(0, TaskStep::Cancel));
    // Inside the task's own cancellation, a cleanup step before its acknowledgement is
    // not a step of that cancellation (cr-3pu5cu round 6).
    cases.push(("deadline cleanup before phases", s3, |n| {
        matches!(
            n,
            Nonconformance::Cancellation(CancellationFault::InterruptedOwnCancel { .. })
        )
    }));
    // S2: a deadline's timer drop before the task's phases.
    let mut s2 = vec![
        spawn(0, 0),
        deadline(0, 10),
        step(0, TaskStep::Begin),
        EventBody::Time(TimeEvent::Scheduled {
            timer: TimerOrdinal(0),
            task: t(0),
            at: VirtualInstant(0),
            deadline: VirtualInstant(40),
        }),
        step(0, TaskStep::Suspend),
        advanced(0, 20),
        step(0, TaskStep::CancelRequested),
        EventBody::Time(TimeEvent::Cancelled {
            timer: TimerOrdinal(0),
            at: VirtualInstant(20),
        }),
    ];
    s2.extend(own_phases(0));
    s2.push(step(0, TaskStep::Cancel));
    cases.push(("deadline timer drop before phases", s2, |n| {
        matches!(
            n,
            Nonconformance::Cancellation(CancellationFault::InterruptedOwnCancel { .. })
        )
    }));
    // S4a: a deadline request, then the clock's first event: no deadline was declared.
    let mut s4a = vec![
        spawn(0, 0),
        step(0, TaskStep::Begin),
        step(0, TaskStep::CancelRequested),
    ];
    s4a.extend(own_phases(0));
    s4a.push(step(0, TaskStep::Cancel));
    s4a.push(advanced(0, 10));
    cases.push(("deadline request before the clock", s4a, |n| {
        matches!(n, Nonconformance::Time(TimeFault::DeadlineUnknown { .. }))
    }));
    // S4b: the deadline declared after the request.
    let mut s4b = vec![
        spawn(0, 0),
        step(0, TaskStep::CancelRequested),
        deadline(0, 100),
    ];
    s4b.extend(own_phases(0));
    s4b.push(step(0, TaskStep::Cancel));
    cases.push(("deadline declared after the request", s4b, |n| {
        matches!(n, Nonconformance::Time(TimeFault::DeadlineUnknown { .. }))
    }));
    // S8: a send before the obligation family's first event, with no send permit.
    cases.push((
        "send before the ledger's first event",
        vec![
            spawn(0, 0),
            spawn(1, 0),
            step(0, TaskStep::Begin),
            step(1, TaskStep::Begin),
            EventBody::Channel(ChannelEvent::Opened {
                channel: ChannelOrdinal(0),
                capacity: 1,
                receiver: t(0),
            }),
            EventBody::Channel(ChannelEvent::Sent {
                channel: ChannelOrdinal(0),
                message: MessageOrdinal(0),
                sender: t(1),
            }),
            EventBody::Channel(ChannelEvent::Received {
                channel: ChannelOrdinal(0),
                message: MessageOrdinal(0),
            }),
            EventBody::Obligation(ObligationEvent::Opened {
                obligation: ObligationOrdinal(0),
                kind: ObligationKind::Lease,
                holder: t(1),
                region: r(0),
            }),
            EventBody::Obligation(ObligationEvent::Discharged {
                obligation: ObligationOrdinal(0),
                how: Discharge::Committed,
            }),
        ],
        |n| matches!(n, Nonconformance::Channel(_)),
    ));
    // S6: a requested task completes normally.
    cases.push((
        "complete after a reported phase",
        vec![
            open(1, 0),
            spawn(0, 1),
            step(0, TaskStep::Begin),
            cancel_region(1),
            requested(0, CancelCause::User),
            step(0, TaskStep::Complete),
            drained(1, &[]),
            finalized(1),
        ],
        // Before RFC 0026 correction 55 the lifecycle family's `OutOfOrder`; the
        // observation rule now refuses it first, as it does every step of a requested
        // task that has not acknowledged.
        |n| {
            matches!(
                n,
                Nonconformance::Cancellation(CancellationFault::BeforeAcknowledgement {
                    task: 0,
                    event: "complete"
                })
            )
        },
    ));
    // S10: a task polled past its deadline as if it had none.
    cases.push((
        "resume past the deadline",
        vec![
            spawn(0, 0),
            deadline(0, 50),
            step(0, TaskStep::Begin),
            step(0, TaskStep::Suspend),
            advanced(0, 100),
            step(0, TaskStep::Resume),
            step(0, TaskStep::Complete),
        ],
        |n| matches!(n, Nonconformance::Time(TimeFault::DeadlineIgnored { .. })),
    ));
    // S11: a parent region's cancellation inside a task's own cancellation.
    let mut s11 = vec![
        open(1, 0),
        open(2, 1),
        spawn(0, 2),
        deadline(0, 10),
        step(0, TaskStep::Begin),
        advanced(0, 20),
        step(0, TaskStep::CancelRequested),
        cancel_region(1),
    ];
    s11.extend(own_phases(0));
    s11.extend([step(0, TaskStep::Cancel), drained(1, &[]), finalized(1)]);
    cases.push(("region cancel inside an own cancellation", s11, |n| {
        matches!(
            n,
            Nonconformance::Cancellation(CancellationFault::InterruptedOwnCancel { .. })
        )
    }));

    // S12: a task's own `cancel` with no phase of its own, in a journal that reports
    // phases (another task's): not a projection without the family, so its missing
    // `requested` is refused at the `cancel` (cr-3pu5cu round 6).
    cases.push((
        "own cancel with no phase in a journal with phases",
        vec![
            spawn(0, 0),
            deadline(0, 10),
            step(0, TaskStep::Begin),
            advanced(0, 20),
            step(0, TaskStep::CancelRequested),
            step(0, TaskStep::Cancel),
            open(1, 0),
            spawn(1, 1),
            step(1, TaskStep::Begin),
            cancel_region(1),
            requested(1, CancelCause::User),
            ack(1),
            cancelled(1, CancelCause::User),
            drained(1, &[1]),
            finalized(1),
        ],
        |n| {
            matches!(
                n,
                Nonconformance::Cancellation(CancellationFault::OutOfOrder {
                    event: "cancel",
                    phase: "active",
                    ..
                })
            )
        },
    ));

    for (name, events, expected) in cases {
        let journal = rebuild(events.iter().cloned());
        match lift(&journal) {
            LiftVerdict::Violates { reason, .. } => {
                assert!(
                    expected(&reason),
                    "{name}: {reason:?}\n{}",
                    journal.render()
                );
            }
            other => panic!("{name}: {other:?}\n{}", journal.render()),
        }
        let verdict = judge_carried(&events, &journal);
        assert!(!verdict.is_accepted(), "{name}: the model accepts it");
    }
}

// --- every family is held to a task's own cancellation (cr-3pu5cu round 6) -----------

/// A deadline run whose cancelled task `t0` holds a lease `o0` and a reservation `e0`,
/// next to a root task `t1` that holds a lease, a reservation, and a channel with one
/// queued message. `t0`'s deadline (50) passes in the advance, and `t0` observes it at
/// its next poll (`Continue`).
fn own_cancel_base() -> Vec<Program> {
    vec![vec![
        open(ROOT, R[1]),
        SubstrateOp::SpawnWithDeadline {
            region: R[1],
            task: T[1],
            resumability: Resumability::Resumable,
            deadline: 50,
        },
        spawn(ROOT, T[2]),
        begin(T[1]),
        begin(T[2]),
        acquire(T[1], 1, ObligationKind::Lease),
        reserve(T[1], 2),
        acquire(T[2], 3, ObligationKind::Lease),
        reserve(T[2], 4),
        SubstrateOp::OpenChannel {
            channel: ChannelLabel(1),
            capacity: 2,
            receiver: T[2],
        },
        SubstrateOp::Send {
            task: T[2],
            channel: ChannelLabel(1),
        },
        SubstrateOp::Advance { nanos: 60 },
        SubstrateOp::Continue { task: T[1] },
    ]]
}

/// One forged event and where it goes: the events it replaces (removed first), and
/// the event itself.
struct OwnCancelAttack {
    name: &'static str,
    removes: fn(&EventBody) -> bool,
    event: EventBody,
}

#[allow(clippy::too_many_lines)]
fn own_cancel_attacks() -> Vec<OwnCancelAttack> {
    let t = TaskOrdinal;
    let r = RegionOrdinal;
    let e = continuum_asupersync::family::effect::ReservationOrdinal;
    let o = ObligationOrdinal;
    let c = continuum_asupersync::family::channel::ChannelOrdinal;
    let k = continuum_asupersync::family::time::TimerOrdinal;
    let nothing = |_: &EventBody| false;
    let abort_e0 = |b: &EventBody| {
        matches!(
            b,
            EventBody::Effect(EffectEvent::Aborted { reservation, .. }) if reservation.0 == 0
        )
    };
    let discharge_o0 = |b: &EventBody| {
        matches!(
            b,
            EventBody::Obligation(ObligationEvent::Discharged { obligation, .. }) if obligation.0 == 0
        )
    };
    let attack = |name, removes: fn(&EventBody) -> bool, event| OwnCancelAttack {
        name,
        removes,
        event,
    };
    vec![
        // The cancelled task's own ordinary work.
        attack(
            "t0 commits e0 (its cancel abort removed)",
            abort_e0,
            EventBody::Effect(EffectEvent::Committed { reservation: e(0) }),
        ),
        attack(
            "t0 reserves e2",
            nothing,
            EventBody::Effect(EffectEvent::Reserved {
                reservation: e(2),
                task: t(0),
            }),
        ),
        attack(
            "t0 aborts e0 explicitly (its cancel abort removed)",
            abort_e0,
            EventBody::Effect(EffectEvent::Aborted {
                reservation: e(0),
                cause: AbortCause::Explicit,
            }),
        ),
        attack(
            "t0 discharges o0 committed (its aborted discharge removed)",
            discharge_o0,
            EventBody::Obligation(ObligationEvent::Discharged {
                obligation: o(0),
                how: Discharge::Committed,
            }),
        ),
        attack(
            "t0 opens o5",
            nothing,
            EventBody::Obligation(ObligationEvent::Opened {
                obligation: o(5),
                kind: ObligationKind::Lease,
                holder: t(0),
                region: r(1),
            }),
        ),
        attack(
            "t0 hands o0 to t1 (its aborted discharge removed)",
            discharge_o0,
            EventBody::Obligation(ObligationEvent::Transferred {
                obligation: o(0),
                holder: t(1),
                region: r(0),
            }),
        ),
        attack(
            "t0 schedules a timer",
            nothing,
            EventBody::Time(TimeEvent::Scheduled {
                timer: k(0),
                task: t(0),
                at: VirtualInstant(60),
                deadline: VirtualInstant(70),
            }),
        ),
        attack(
            "t0 sends on c0",
            nothing,
            EventBody::Channel(ChannelEvent::Sent {
                channel: c(0),
                message: MessageOrdinal(1),
                sender: t(0),
            }),
        ),
        attack(
            "t0 suspends",
            nothing,
            EventBody::Lifecycle(LifecycleEvent::TaskStepped {
                task: t(0),
                step: TaskStep::Suspend,
            }),
        ),
        // Another task's work.
        attack(
            "t1 commits e1",
            nothing,
            EventBody::Effect(EffectEvent::Committed { reservation: e(1) }),
        ),
        attack(
            "t1 aborts e1 with the cancel cause",
            nothing,
            EventBody::Effect(EffectEvent::Aborted {
                reservation: e(1),
                cause: AbortCause::Cancel,
            }),
        ),
        attack(
            "t1 discharges o2 committed",
            nothing,
            EventBody::Obligation(ObligationEvent::Discharged {
                obligation: o(2),
                how: Discharge::Committed,
            }),
        ),
        attack(
            "t1 discharges o2 aborted",
            nothing,
            EventBody::Obligation(ObligationEvent::Discharged {
                obligation: o(2),
                how: Discharge::Aborted,
            }),
        ),
        attack(
            "t1 opens o5",
            nothing,
            EventBody::Obligation(ObligationEvent::Opened {
                obligation: o(5),
                kind: ObligationKind::Lease,
                holder: t(1),
                region: r(0),
            }),
        ),
        attack(
            "t1 hands o2 to t0",
            nothing,
            EventBody::Obligation(ObligationEvent::Transferred {
                obligation: o(2),
                holder: t(0),
                region: r(1),
            }),
        ),
        attack(
            "t1 schedules a timer",
            nothing,
            EventBody::Time(TimeEvent::Scheduled {
                timer: k(0),
                task: t(1),
                at: VirtualInstant(60),
                deadline: VirtualInstant(70),
            }),
        ),
        attack(
            "the clock advances",
            nothing,
            EventBody::Time(TimeEvent::Advanced {
                from: VirtualInstant(60),
                to: VirtualInstant(70),
            }),
        ),
        attack(
            "t1 receives m0",
            nothing,
            EventBody::Channel(ChannelEvent::Received {
                channel: c(0),
                message: MessageOrdinal(0),
            }),
        ),
        attack(
            "t1 sends on c0",
            nothing,
            EventBody::Channel(ChannelEvent::Sent {
                channel: c(0),
                message: MessageOrdinal(1),
                sender: t(1),
            }),
        ),
        attack(
            "c0's senders close",
            nothing,
            EventBody::Channel(ChannelEvent::SendersClosed { channel: c(0) }),
        ),
        attack(
            "t1 suspends",
            nothing,
            EventBody::Lifecycle(LifecycleEvent::TaskStepped {
                task: t(1),
                step: TaskStep::Suspend,
            }),
        ),
        attack(
            "t2 is spawned",
            nothing,
            EventBody::Lifecycle(LifecycleEvent::TaskSpawned {
                task: t(2),
                region: r(0),
                resumability: Resumability::Resumable,
            }),
        ),
        attack(
            "t1's cancellation is requested",
            nothing,
            EventBody::Cancellation(CancellationEvent::Requested {
                task: t(1),
                cause: CancelCause::User,
            }),
        ),
    ]
}

/// Codex cr-3pu5cu round 6: a task's own (deadline) cancellation is one run of that
/// task's events in every family, not only in the lifecycle family. For every one of
/// the 32 projections, a real deadline journal conforms and the model accepts it (its
/// cleanup after the acknowledgement included), and each forged event of the family
/// set above, placed right after the task's `cancel-requested`, right after its
/// `acknowledged` (when the projection reports phases) and right before its own
/// `cancel`, is `InterruptedOwnCancel` in the lift and `OwnCancellationInterrupted` in
/// the independent model. A forged event of a family the projection does not carry is
/// not a journal of that projection, so it is not placed. Measured when this test was
/// written: with the lift's rule turned off for every family but the lifecycle, 412 of
/// the 974 placed journals lifted as `Conforms` (14 of the 23 attacks, under at least
/// one projection each), so this rule is what refuses them.
#[test]
fn every_family_is_held_to_a_task_s_own_cancellation() {
    use continuum_asupersync::family::cancellation::CancellationFault;
    let programs = own_cancel_base();
    let log = ChoiceLog::new(vec![0; programs[0].len()]);
    let is_request = |b: &EventBody| {
        matches!(
            b,
            EventBody::Lifecycle(LifecycleEvent::TaskStepped {
                task: TaskOrdinal(0),
                step: TaskStep::CancelRequested,
            })
        )
    };
    let is_ack = |b: &EventBody| {
        matches!(
            b,
            EventBody::Cancellation(CancellationEvent::Acknowledged {
                task: TaskOrdinal(0)
            })
        )
    };
    let mut placed = 0_usize;
    let mut families = BTreeSet::new();
    for config in every_projection() {
        let journal = run(&programs, &log, &config).unwrap();
        assert!(
            lift_accepts(&journal) && judge(&config, &journal).is_accepted(),
            "{:?}: {:?} / {}\n{}",
            config.families,
            lift(&journal),
            judge(&config, &journal),
            journal.render()
        );
        for attack in own_cancel_attacks() {
            if !config.families.contains(attack.event.family()) {
                continue;
            }
            let mut base = bodies(&journal);
            // An attack that replaces a cleanup step goes where that step was, or
            // earlier: a later place leaves the step missing before it, which is a
            // fault of its own.
            let replaced = base.iter().position(|b| (attack.removes)(b));
            base.retain(|b| !(attack.removes)(b));
            let request = base.iter().position(is_request).expect("t0's request");
            let end = base.iter().position(is_own_cancel).expect("t0's cancel");
            let mut points = match replaced {
                Some(at) => vec![request + 1, at],
                None => vec![request + 1, end],
            };
            if let Some(ack) = base.iter().position(is_ack) {
                if replaced.is_none_or(|at| ack < at) {
                    points.push(ack + 1);
                }
            }
            points.sort_unstable();
            points.dedup();
            for at in points {
                let mut b = base.clone();
                b.insert(at, attack.event.clone());
                let variant = rebuild(b);
                let lifted = lift(&variant);
                assert!(
                    matches!(
                        lifted,
                        LiftVerdict::Violates {
                            reason: Nonconformance::Cancellation(
                                CancellationFault::InterruptedOwnCancel { task: 0, .. }
                            ),
                            ..
                        }
                    ),
                    "{} at {at} {:?}: {lifted:?}\n{}",
                    attack.name,
                    config.families,
                    variant.render()
                );
                let verdict = judge(&config, &variant);
                assert!(
                    matches!(
                        verdict,
                        Verdict::Rejected {
                            fault: Fault::OwnCancellationInterrupted(0),
                            ..
                        }
                    ),
                    "{} at {at} {:?}: {verdict}",
                    attack.name,
                    config.families
                );
                families.insert(attack.event.family().to_string());
                placed += 1;
            }
        }
    }
    assert_eq!(families.len(), 6, "{families:?}");
    assert_eq!(placed, 974);
}

/// Journals from the round-6 pre-review adversarial pass (cr-3pu5cu). Each once lifted
/// as `Conforms` and the model accepted most of them:
///
/// - without the cancellation family, a task's own cleanup step (a cancellation's
///   abort, a timer's cancellation, an abandoned send or receive) did not stop its
///   later work (B, B2, C, I, P1, P2, P3). Cleanup now stands for the acknowledgement in
///   the lift (`cancellation::imply_acknowledgement`) and in the model (`cleaned_up`);
/// - a task whose deadline the clock had reached still did its own work in other
///   families, or completed or failed (E, E2, E3). A task's own work in any family now
///   needs its deadline not reached (`time::check_actor_deadline`, the model's
///   `actor`);
/// - a region's cancellation was requested again (R1, R2, R3). The lift refuses the
///   repeat (`RepeatedCancel`), as the model does, so a forged run of repeats costs
///   nothing per repeat.
///
/// Now the lift reports each as a violation, and the model, judging over the families
/// the journal carries, rejects each.
#[test]
#[allow(clippy::too_many_lines)]
fn round_six_pre_review_journals_are_rejected() {
    use continuum_asupersync::family::channel::ChannelOrdinal;
    use continuum_asupersync::family::effect::ReservationOrdinal;
    use continuum_asupersync::family::time::TimerOrdinal;
    use continuum_task::region::worker::FailureReason;
    let t = TaskOrdinal;
    let r = RegionOrdinal;
    let open = |region, parent| {
        EventBody::Lifecycle(LifecycleEvent::RegionOpened {
            region: r(region),
            parent: r(parent),
        })
    };
    let spawn = |task, region| {
        EventBody::Lifecycle(LifecycleEvent::TaskSpawned {
            task: t(task),
            region: r(region),
            resumability: Resumability::Resumable,
        })
    };
    let step = |task, step| {
        EventBody::Lifecycle(LifecycleEvent::TaskStepped {
            task: t(task),
            step,
        })
    };
    let cancel_region =
        |region| EventBody::Lifecycle(LifecycleEvent::RegionCancelRequested { region: r(region) });
    let drained = |region, cancelled: &[u32]| {
        EventBody::Lifecycle(LifecycleEvent::RegionDrained {
            region: r(region),
            cancelled: TaskSet::new(cancelled.iter().map(|x| t(*x))),
        })
    };
    let finalized =
        |region| EventBody::Lifecycle(LifecycleEvent::RegionFinalized { region: r(region) });
    let reserved = |e, task| {
        EventBody::Effect(EffectEvent::Reserved {
            reservation: ReservationOrdinal(e),
            task: t(task),
        })
    };
    let committed = |e| {
        EventBody::Effect(EffectEvent::Committed {
            reservation: ReservationOrdinal(e),
        })
    };
    let cancel_abort = |e| {
        EventBody::Effect(EffectEvent::Aborted {
            reservation: ReservationOrdinal(e),
            cause: AbortCause::Cancel,
        })
    };
    let ob_open = |o, holder, region| {
        EventBody::Obligation(ObligationEvent::Opened {
            obligation: ObligationOrdinal(o),
            kind: ObligationKind::Lease,
            holder: t(holder),
            region: r(region),
        })
    };
    let ob_dis = |o, how| {
        EventBody::Obligation(ObligationEvent::Discharged {
            obligation: ObligationOrdinal(o),
            how,
        })
    };
    let ob_xfer = |o, holder, region| {
        EventBody::Obligation(ObligationEvent::Transferred {
            obligation: ObligationOrdinal(o),
            holder: t(holder),
            region: r(region),
        })
    };
    let settled = |region| {
        EventBody::Obligation(ObligationEvent::RegionSettled {
            region: r(region),
            open: ObligationSet::new([]),
            leaked: ObligationSet::new([]),
            fenced: ObligationSet::new([]),
        })
    };
    let deadline = |task, at| {
        EventBody::Time(TimeEvent::Deadline {
            task: t(task),
            at: VirtualInstant(at),
        })
    };
    let advanced = |from, to| {
        EventBody::Time(TimeEvent::Advanced {
            from: VirtualInstant(from),
            to: VirtualInstant(to),
        })
    };
    let sched = |k, task, at, dl| {
        EventBody::Time(TimeEvent::Scheduled {
            timer: TimerOrdinal(k),
            task: t(task),
            at: VirtualInstant(at),
            deadline: VirtualInstant(dl),
        })
    };
    let timer_cancelled = |k, at| {
        EventBody::Time(TimeEvent::Cancelled {
            timer: TimerOrdinal(k),
            at: VirtualInstant(at),
        })
    };
    let fired = |k, at| {
        EventBody::Time(TimeEvent::Fired {
            timer: TimerOrdinal(k),
            at: VirtualInstant(at),
        })
    };
    let c = ChannelOrdinal;
    let copen = |ch, capacity, receiver| {
        EventBody::Channel(ChannelEvent::Opened {
            channel: c(ch),
            capacity,
            receiver: t(receiver),
        })
    };
    let sent = |ch, m, sender| {
        EventBody::Channel(ChannelEvent::Sent {
            channel: c(ch),
            message: MessageOrdinal(m),
            sender: t(sender),
        })
    };
    let send_blocked = |ch, m, sender| {
        EventBody::Channel(ChannelEvent::SendBlocked {
            channel: c(ch),
            message: MessageOrdinal(m),
            sender: t(sender),
        })
    };
    let send_abandoned = |ch, m, sender| {
        EventBody::Channel(ChannelEvent::SendAbandoned {
            channel: c(ch),
            message: MessageOrdinal(m),
            sender: t(sender),
        })
    };
    let received = |ch, m| {
        EventBody::Channel(ChannelEvent::Received {
            channel: c(ch),
            message: MessageOrdinal(m),
        })
    };
    let recv_blocked = |ch| EventBody::Channel(ChannelEvent::RecvBlocked { channel: c(ch) });
    let recv_abandoned = |ch| EventBody::Channel(ChannelEvent::RecvAbandoned { channel: c(ch) });
    let gone = |ch| {
        EventBody::Channel(ChannelEvent::ReceiverGone {
            channel: c(ch),
            discarded: MessageSet::new([]),
        })
    };
    use TaskStep::{Begin, Complete};

    let cases: Vec<(&str, Vec<EventBody>)> = vec![
        (
            "B cancel abort, then commit",
            vec![
                open(1, 0),
                spawn(0, 1),
                step(0, Begin),
                reserved(0, 0),
                reserved(1, 0),
                cancel_region(1),
                cancel_abort(0),
                committed(1),
                drained(1, &[0]),
                finalized(1),
            ],
        ),
        (
            "B2 cancel abort, then reserve and commit",
            vec![
                open(1, 0),
                spawn(0, 1),
                step(0, Begin),
                reserved(0, 0),
                cancel_region(1),
                cancel_abort(0),
                reserved(1, 0),
                committed(1),
                drained(1, &[0]),
                finalized(1),
            ],
        ),
        (
            "C send abandoned, then a fresh send",
            vec![
                open(1, 0),
                spawn(0, 1),
                spawn(1, 0),
                step(0, Begin),
                step(1, Begin),
                copen(0, 1, 1),
                sent(0, 0, 0),
                send_blocked(0, 1, 0),
                cancel_region(1),
                send_abandoned(0, 1, 0),
                received(0, 0),
                sent(0, 2, 0),
                drained(1, &[0]),
                finalized(1),
            ],
        ),
        (
            "I timer cancelled, then a new timer",
            vec![
                open(1, 0),
                spawn(0, 1),
                step(0, Begin),
                sched(0, 0, 0, 10),
                cancel_region(1),
                timer_cancelled(0, 0),
                sched(1, 0, 0, 5),
                advanced(0, 5),
                fired(1, 5),
                drained(1, &[0]),
                finalized(1),
            ],
        ),
        (
            "P1 cancel abort, then a committed discharge",
            vec![
                open(1, 0),
                spawn(0, 1),
                step(0, Begin),
                reserved(0, 0),
                ob_open(0, 0, 1),
                cancel_region(1),
                cancel_abort(0),
                ob_dis(0, Discharge::Committed),
                drained(1, &[0]),
                finalized(1),
                settled(1),
            ],
        ),
        (
            "P2 timer cancelled, then a hand-off",
            vec![
                open(1, 0),
                spawn(0, 1),
                spawn(1, 0),
                step(0, Begin),
                step(1, Begin),
                ob_open(0, 0, 1),
                sched(0, 0, 0, 10),
                cancel_region(1),
                timer_cancelled(0, 0),
                ob_xfer(0, 1, 0),
                drained(1, &[0]),
                finalized(1),
                settled(1),
                ob_dis(0, Discharge::Committed),
            ],
        ),
        (
            "P3 receive abandoned, then a committed discharge",
            vec![
                open(1, 0),
                spawn(0, 1),
                step(0, Begin),
                ob_open(0, 0, 1),
                copen(0, 2, 0),
                recv_blocked(0),
                cancel_region(1),
                recv_abandoned(0),
                ob_dis(0, Discharge::Committed),
                gone(0),
                drained(1, &[0]),
                finalized(1),
                settled(1),
            ],
        ),
        (
            "E complete past the deadline",
            vec![
                spawn(0, 0),
                deadline(0, 10),
                step(0, Begin),
                advanced(0, 20),
                step(0, Complete),
            ],
        ),
        (
            "E2 fail from created past the deadline",
            vec![
                spawn(0, 0),
                deadline(0, 10),
                advanced(0, 20),
                step(0, TaskStep::Fail(FailureReason::new("boom").unwrap())),
            ],
        ),
        (
            "E3 own work in every family past the deadline",
            vec![
                spawn(0, 0),
                deadline(0, 10),
                spawn(1, 0),
                step(0, Begin),
                step(1, Begin),
                copen(0, 2, 1),
                advanced(0, 20),
                reserved(0, 0),
                committed(0),
                ob_open(0, 0, 0),
                ob_dis(0, Discharge::Committed),
                step(0, Complete),
            ],
        ),
        (
            "R1 a region cancelled twice",
            vec![
                open(1, 0),
                spawn(0, 1),
                step(0, Begin),
                cancel_region(1),
                cancel_region(1),
                drained(1, &[0]),
                finalized(1),
            ],
        ),
        (
            "R2 a child cancelled after its parent",
            vec![
                open(1, 0),
                open(2, 1),
                spawn(0, 2),
                step(0, Begin),
                cancel_region(1),
                cancel_region(2),
                drained(1, &[0]),
                finalized(1),
            ],
        ),
        (
            "R3 a region cancelled after its drain",
            vec![
                open(1, 0),
                spawn(0, 1),
                step(0, Begin),
                cancel_region(1),
                drained(1, &[0]),
                cancel_region(1),
                finalized(1),
            ],
        ),
    ];
    assert_eq!(cases.len(), 13);
    for (name, events) in cases {
        let journal = rebuild(events.iter().cloned());
        assert!(
            matches!(lift(&journal), LiftVerdict::Violates { .. }),
            "{name}: {:?}\n{}",
            lift(&journal),
            journal.render()
        );
        let verdict = judge_carried(&events, &journal);
        assert!(!verdict.is_accepted(), "{name}: the model accepts it");
    }
}

/// Codex cr-3pu5cu round 6 (th-2f07u7): every live task a region's cancellation reaches
/// has phases when the journal carries the cancellation family. A two-task journal: one
/// region request reaches `t0` and `t1`; `t1` is fully phased and drained, `t0` has no
/// phase and ends by `complete` or `fail`, from created or running, under its own
/// region's request or an ancestor's. Each is `UnreportedRequest` in the lift and in
/// the model. Without the cancellation family the same journals are refused too, since
/// RFC 0026 correction 55 (bn-28hup): `t0` ends before it observed the request, which
/// is `BeforeAcknowledgement` in the lift and `Unobserved` in the model under every
/// projection. Controls: `t0` ending before the request conforms in both, with the
/// cancellation family and without it.
#[test]
fn a_region_request_needs_phases_for_every_task_it_reaches() {
    use continuum_asupersync::family::cancellation::CancellationFault;
    use continuum_task::region::worker::FailureReason;
    let t = TaskOrdinal;
    let r = RegionOrdinal;
    let lc = EventBody::Lifecycle;
    let open = |region, parent| {
        lc(LifecycleEvent::RegionOpened {
            region: r(region),
            parent: r(parent),
        })
    };
    let spawn = |task, region| {
        lc(LifecycleEvent::TaskSpawned {
            task: t(task),
            region: r(region),
            resumability: Resumability::Resumable,
        })
    };
    let step = |task, step| {
        lc(LifecycleEvent::TaskStepped {
            task: t(task),
            step,
        })
    };
    let phase = |event| EventBody::Cancellation(event);
    let fail = || TaskStep::Fail(FailureReason::new("boom").unwrap());
    let mut violations = 0;
    let mut controls = 0;
    // `ancestor`: t0 and t1 live in r2 under r1, and r1 is cancelled; else in r1.
    for ancestor in [false, true] {
        for running in [false, true] {
            for end in [TaskStep::Complete, fail()] {
                if !running && end == TaskStep::Complete {
                    continue; // `complete` needs a running task
                }
                for before in [false, true] {
                    let (home, cancelled) = if ancestor { (2, 1) } else { (1, 1) };
                    let cause = if ancestor {
                        CancelCause::ParentCancelled
                    } else {
                        CancelCause::User
                    };
                    let mut b = vec![open(1, 0)];
                    if ancestor {
                        b.push(open(2, 1));
                    }
                    b.extend([spawn(0, home), spawn(1, home), step(1, TaskStep::Begin)]);
                    if running {
                        b.push(step(0, TaskStep::Begin));
                    }
                    if before {
                        b.push(step(0, end.clone()));
                    }
                    b.push(lc(LifecycleEvent::RegionCancelRequested {
                        region: r(cancelled),
                    }));
                    if !before {
                        b.push(step(0, end.clone()));
                    }
                    b.extend([
                        phase(CancellationEvent::Requested { task: t(1), cause }),
                        phase(CancellationEvent::Acknowledged { task: t(1) }),
                        phase(CancellationEvent::Cancelled { task: t(1), cause }),
                        lc(LifecycleEvent::RegionDrained {
                            region: r(cancelled),
                            cancelled: TaskSet::new([t(1)]),
                        }),
                        lc(LifecycleEvent::RegionFinalized {
                            region: r(cancelled),
                        }),
                    ]);
                    let without: Vec<EventBody> = b
                        .iter()
                        .filter(|e| e.family() != Family::Cancellation)
                        .cloned()
                        .collect();
                    let name = format!(
                        "ancestor={ancestor} running={running} end={end:?} before={before}"
                    );
                    let journal = rebuild(b.iter().cloned());
                    let control = rebuild(without.iter().cloned());
                    if before {
                        assert!(
                            lift_accepts(&control)
                                && judge_carried(&without, &control).is_accepted(),
                            "{name} without phases: {:?} / {}\n{}",
                            lift(&control),
                            judge_carried(&without, &control),
                            control.render()
                        );
                        controls += 1;
                    } else {
                        assert!(
                            matches!(
                                lift(&control),
                                LiftVerdict::Violates {
                                    reason: Nonconformance::Cancellation(
                                        CancellationFault::BeforeAcknowledgement { task: 0, .. }
                                    ),
                                    ..
                                }
                            ),
                            "{name} without phases: {:?}\n{}",
                            lift(&control),
                            control.render()
                        );
                        assert!(
                            matches!(
                                judge_carried(&without, &control),
                                Verdict::Rejected {
                                    fault: Fault::Unobserved(0),
                                    ..
                                }
                            ),
                            "{name} without phases: {}",
                            judge_carried(&without, &control)
                        );
                        violations += 1;
                    }
                    if before {
                        assert!(
                            lift_accepts(&journal) && judge_carried(&b, &journal).is_accepted(),
                            "{name}: {:?} / {}\n{}",
                            lift(&journal),
                            judge_carried(&b, &journal),
                            journal.render()
                        );
                        controls += 1;
                        continue;
                    }
                    assert!(
                        matches!(
                            lift(&journal),
                            LiftVerdict::Violates {
                                reason: Nonconformance::Cancellation(
                                    CancellationFault::UnreportedRequest { task: 0, .. }
                                ),
                                ..
                            }
                        ),
                        "{name}: {:?}\n{}",
                        lift(&journal),
                        journal.render()
                    );
                    assert!(
                        matches!(
                            judge_carried(&b, &journal),
                            Verdict::Rejected {
                                fault: Fault::UnreportedRequest(0),
                                ..
                            }
                        ),
                        "{name}: {}",
                        judge_carried(&b, &journal)
                    );
                    violations += 1;
                }
            }
        }
    }
    assert_eq!((violations, controls), (12, 12));
}

/// Codex cr-3pu5cu round 7 (th-18a2iz): a task's own region cancelled after the clock
/// reached the task's deadline, before the task observed it, is the race the binding
/// refuses as `CancelRaced` under every projection. A hand-built journal of it, with
/// the task created, running or suspended, is `TimeFault::CancelRaced` in the lift and
/// `Fault::CancelRaced` in the model. Controls conform in both: the own region
/// cancelled before the deadline, a proper ancestor cancelled after it
/// (`parent-cancelled` outranks the deadline), and every journal with the time family
/// dropped, which declares no deadline and so cannot show the race.
#[test]
fn an_own_region_cancel_after_a_due_deadline_is_the_race() {
    use continuum_asupersync::family::time::TimeFault;
    let t = TaskOrdinal;
    let r = RegionOrdinal;
    let lc = EventBody::Lifecycle;
    let step = |step| lc(LifecycleEvent::TaskStepped { task: t(0), step });
    let mut raced = 0;
    let mut controls = 0;
    for start in ["created", "running", "suspended"] {
        for (ancestor, due) in [(false, true), (false, false), (true, true)] {
            let home = if ancestor { 2 } else { 1 };
            let cause = if ancestor {
                CancelCause::ParentCancelled
            } else {
                CancelCause::User
            };
            let mut b = vec![lc(LifecycleEvent::RegionOpened {
                region: r(1),
                parent: r(0),
            })];
            if ancestor {
                b.push(lc(LifecycleEvent::RegionOpened {
                    region: r(2),
                    parent: r(1),
                }));
            }
            b.push(lc(LifecycleEvent::TaskSpawned {
                task: t(0),
                region: r(home),
                resumability: Resumability::Resumable,
            }));
            b.push(EventBody::Time(TimeEvent::Deadline {
                task: t(0),
                at: VirtualInstant(10),
            }));
            match start {
                "running" => b.push(step(TaskStep::Begin)),
                "suspended" => b.extend([step(TaskStep::Begin), step(TaskStep::Suspend)]),
                _ => {}
            }
            b.push(EventBody::Time(TimeEvent::Advanced {
                from: VirtualInstant(0),
                to: VirtualInstant(if due { 20 } else { 5 }),
            }));
            b.extend([
                lc(LifecycleEvent::RegionCancelRequested { region: r(1) }),
                EventBody::Cancellation(CancellationEvent::Requested { task: t(0), cause }),
                EventBody::Cancellation(CancellationEvent::Acknowledged { task: t(0) }),
                EventBody::Cancellation(CancellationEvent::Cancelled { task: t(0), cause }),
                lc(LifecycleEvent::RegionDrained {
                    region: r(1),
                    cancelled: TaskSet::new([t(0)]),
                }),
                lc(LifecycleEvent::RegionFinalized { region: r(1) }),
            ]);
            let name = format!("{start} ancestor={ancestor} due={due}");
            let journal = rebuild(b.iter().cloned());
            let without: Vec<EventBody> = b
                .iter()
                .filter(|e| e.family() != Family::Time)
                .cloned()
                .collect();
            let control = rebuild(without.iter().cloned());
            assert!(
                lift_accepts(&control) && judge_carried(&without, &control).is_accepted(),
                "{name} without time: {:?} / {}\n{}",
                lift(&control),
                judge_carried(&without, &control),
                control.render()
            );
            controls += 1;
            if ancestor || !due {
                assert!(
                    lift_accepts(&journal) && judge_carried(&b, &journal).is_accepted(),
                    "{name}: {:?} / {}\n{}",
                    lift(&journal),
                    judge_carried(&b, &journal),
                    journal.render()
                );
                controls += 1;
                continue;
            }
            assert!(
                matches!(
                    lift(&journal),
                    LiftVerdict::Violates {
                        reason: Nonconformance::Time(TimeFault::CancelRaced {
                            task: 0,
                            region: 1,
                            ..
                        }),
                        ..
                    }
                ),
                "{name}: {:?}\n{}",
                lift(&journal),
                journal.render()
            );
            assert!(
                matches!(
                    judge_carried(&b, &journal),
                    Verdict::Rejected {
                        fault: Fault::CancelRaced(0),
                        ..
                    }
                ),
                "{name}: {}",
                judge_carried(&b, &journal)
            );
            raced += 1;
        }
    }
    assert_eq!((raced, controls), (3, 15));
    // The binding refuses the same execution under every projection, lifecycle alone
    // included, so no projection returns a journal the lift would have to judge.
    let programs = vec![vec![
        open(ROOT, R[1]),
        SubstrateOp::SpawnWithDeadline {
            region: R[1],
            task: T[1],
            resumability: Resumability::Resumable,
            deadline: 10,
        },
        begin(T[1]),
        SubstrateOp::Advance { nanos: 20 },
        SubstrateOp::Cancel { region: R[1] },
    ]];
    for config in every_projection() {
        assert_eq!(
            run(&programs, &ChoiceLog::new([0; 5]), &config),
            Err(continuum_asupersync::binding::BindingRefusal::CancelRaced { task: 0 }),
            "{:?}",
            config.families
        );
    }
}

/// Self-sweep for cr-3pu5cu round 7: rules whose verdict turns on another family's fact.
/// A leak is its holder's end (lifecycle), so a leak by a parked holder is a violation
/// and a leak whose holder never ends is a truncation; a channel's receiver is handed
/// to a task not in cancellation. Each once lifted as `Conforms` while the model
/// rejected it. Now neither accepts.
#[test]
fn cross_family_facts_are_read_by_the_lift() {
    use continuum_asupersync::family::channel::ChannelOrdinal;
    use continuum_asupersync::family::obligation::LedgerFault;
    let t = TaskOrdinal;
    let r = RegionOrdinal;
    let lc = EventBody::Lifecycle;
    let step = |task, step| {
        lc(LifecycleEvent::TaskStepped {
            task: t(task),
            step,
        })
    };
    let spawn = |task, region| {
        lc(LifecycleEvent::TaskSpawned {
            task: t(task),
            region: r(region),
            resumability: Resumability::Resumable,
        })
    };
    let opened = EventBody::Obligation(ObligationEvent::Opened {
        obligation: ObligationOrdinal(0),
        kind: ObligationKind::Lease,
        holder: t(0),
        region: r(0),
    });
    let leaked = EventBody::Obligation(ObligationEvent::Leaked {
        obligation: ObligationOrdinal(0),
    });
    // A leak whose holder never ends: a truncation.
    let unended = vec![
        spawn(0, 0),
        step(0, TaskStep::Begin),
        opened.clone(),
        leaked.clone(),
    ];
    let journal = rebuild(unended.iter().cloned());
    assert!(
        matches!(
            lift(&journal),
            LiftVerdict::Inconclusive {
                reason: continuum_value::assurance::InconclusiveReason::InsufficientTelemetry,
                ..
            }
        ),
        "{:?}",
        lift(&journal)
    );
    assert!(!judge_carried(&unended, &journal).is_accepted());
    // A leak by a parked holder.
    let parked = vec![
        spawn(0, 0),
        step(0, TaskStep::Begin),
        opened,
        step(0, TaskStep::Suspend),
        leaked.clone(),
    ];
    let journal = rebuild(parked.iter().cloned());
    assert!(
        matches!(
            lift(&journal),
            LiftVerdict::Violates {
                reason: Nonconformance::Obligation(LedgerFault::LeakByLiveHolder { .. }),
                ..
            }
        ),
        "{:?}",
        lift(&journal)
    );
    assert!(!judge_carried(&parked, &journal).is_accepted());
    // A channel whose receiver is a task in cancellation.
    let in_cancel = vec![
        lc(LifecycleEvent::RegionOpened {
            region: r(1),
            parent: r(0),
        }),
        spawn(0, 1),
        step(0, TaskStep::Begin),
        lc(LifecycleEvent::RegionCancelRequested { region: r(1) }),
        EventBody::Cancellation(CancellationEvent::Requested {
            task: t(0),
            cause: CancelCause::User,
        }),
        EventBody::Cancellation(CancellationEvent::Acknowledged { task: t(0) }),
        EventBody::Channel(ChannelEvent::Opened {
            channel: ChannelOrdinal(0),
            capacity: 1,
            receiver: t(0),
        }),
    ];
    let journal = rebuild(in_cancel.iter().cloned());
    assert!(
        matches!(
            lift(&journal),
            LiftVerdict::Violates {
                reason: Nonconformance::Channel(
                    continuum_asupersync::family::channel::ChannelFault::NotActing { .. }
                ),
                ..
            }
        ),
        "{:?}",
        lift(&journal)
    );
    assert!(!judge_carried(&in_cancel, &journal).is_accepted());
}

/// Codex cr-3pu5cu round 8 (1): a budget deadline is declared by the event right after
/// its task's spawn, as the binding writes it. The spawn is lifecycle, which every
/// projection keeps, and a projection only drops events, so the two stay adjacent in
/// every projection that carries the time family: the rule is decidable wherever the
/// declaration is visible, and a projection without time has no declaration to judge.
/// A declaration after a clock advance, after another task's spawn, or after the
/// task's own step is `DeadlineNotAtSpawn` in the lift and in the model. The
/// adjacent declaration and a task with no deadline conform in both.
#[test]
fn a_deadline_is_declared_right_after_its_spawn() {
    use continuum_asupersync::family::time::TimeFault;
    let t = TaskOrdinal;
    let lc = EventBody::Lifecycle;
    let spawn = |task| {
        lc(LifecycleEvent::TaskSpawned {
            task: t(task),
            region: RegionOrdinal(0),
            resumability: Resumability::Resumable,
        })
    };
    let deadline = EventBody::Time(TimeEvent::Deadline {
        task: t(0),
        at: VirtualInstant(10),
    });
    let begin = lc(LifecycleEvent::TaskStepped {
        task: t(0),
        step: TaskStep::Begin,
    });
    let advanced = EventBody::Time(TimeEvent::Advanced {
        from: VirtualInstant(0),
        to: VirtualInstant(5),
    });
    let late: Vec<(&str, Vec<EventBody>)> = vec![
        (
            "after a clock advance",
            vec![spawn(0), advanced.clone(), deadline.clone(), begin.clone()],
        ),
        (
            "after another task's spawn",
            vec![spawn(0), spawn(1), deadline.clone(), begin.clone()],
        ),
        (
            "after its own first step",
            vec![spawn(0), begin.clone(), deadline.clone()],
        ),
    ];
    for (name, events) in late {
        let journal = rebuild(events.iter().cloned());
        assert!(
            matches!(
                lift(&journal),
                LiftVerdict::Violates {
                    reason: Nonconformance::Time(TimeFault::DeadlineNotAtSpawn { task: 0 }),
                    ..
                }
            ),
            "{name}: {:?}",
            lift(&journal)
        );
        assert!(
            matches!(
                judge_carried(&events, &journal),
                Verdict::Rejected {
                    fault: Fault::DeadlineNotAtSpawn(0),
                    ..
                }
            ),
            "{name}: {}",
            judge_carried(&events, &journal)
        );
    }
    for (name, events) in [
        (
            "adjacent",
            vec![spawn(0), deadline.clone(), begin.clone(), advanced.clone()],
        ),
        ("no deadline", vec![spawn(0), advanced, begin]),
    ] {
        let journal = rebuild(events.iter().cloned());
        assert!(
            lift_accepts(&journal) && judge_carried(&events, &journal).is_accepted(),
            "{name}: {:?} / {}",
            lift(&journal),
            judge_carried(&events, &journal)
        );
    }
}

/// The canonical bytes of `journal` under the version-2 grammar: the version word 2,
/// and each settle without the fenced set that version 3 added (bn-20d8u). Only for a
/// journal with no version-3 tag and no fenced obligation.
fn as_version_2(journal: &Journal) -> Vec<u8> {
    let magic = b"continuum/semantic-journal\n";
    let mut out = magic.to_vec();
    out.extend_from_slice(&2_u32.to_be_bytes());
    out.extend_from_slice(&(journal.len() as u64).to_be_bytes());
    for event in journal.events() {
        let one = rebuild([event.body().clone()]).encode().unwrap();
        // MAGIC, version, count, seq, family tag, length.
        let at = magic.len() + 4 + 8 + 8;
        let family = one[at];
        let mut payload = one[at + 1 + 4..].to_vec();
        if let EventBody::Obligation(ObligationEvent::RegionSettled { fenced, .. }) = event.body() {
            assert!(fenced.is_empty(), "a version-2 journal has no fence");
            let cut = payload.len() - 4;
            assert_eq!(payload[cut..], [0, 0, 0, 0]);
            payload.truncate(cut);
        }
        out.extend_from_slice(&event.seq().to_be_bytes());
        out.push(family);
        out.extend_from_slice(&u32::try_from(payload.len()).unwrap().to_be_bytes());
        out.extend_from_slice(&payload);
    }
    out
}

/// Encoding version 3 (bn-20d8u) adds lifecycle event 8 (`region-crashed`), effect
/// event 4, obligation event 6 and time event 6 (each `fenced`), and a third set on
/// obligation event 5 (`region-settled`). Both readers (the crate's decoder and the A7
/// model's own) read versions 2 and 3, each under its own grammar, and no longer read
/// version 1 (ADR-0018: two at most). A real crash journal decodes as version 3, lifts
/// as conforming and is accepted by the model; the same bytes labelled version 2 are
/// refused by both, a new tag as a tag outside that version's grammar, one table at a
/// time. A version-2-grammar journal (settles with two sets) decodes to the same journal
/// as its version-3 encoding and gets the same verdict from both. The bn-36wy3 tables
/// (lifecycle steps 6-7, cancel cause 3, time event 5) are version-2 tags and read under
/// either label.
#[test]
fn the_two_encoding_versions_are_two_grammars() {
    use continuum_asupersync::encoding::DecodeError;
    let header = b"continuum/semantic-journal\n".len();
    let relabel = |bytes: &[u8], version: u8| {
        let mut out = bytes.to_vec();
        assert_eq!(&out[header..header + 4], &[0, 0, 0, 3]);
        out[header + 3] = version;
        out
    };
    let config = all_families();
    let programs = vec![vec![
        open(ROOT, R[1]),
        spawn(R[1], T[1]),
        begin(T[1]),
        reserve(T[1], 1),
        acquire(T[1], 2, ObligationKind::Lease),
        SubstrateOp::Sleep {
            task: T[1],
            nanos: 10,
        },
        SubstrateOp::Crash { region: R[1] },
    ]];
    let log = ChoiceLog::new(vec![0; programs[0].len()]);
    let journal = run(&programs, &log, &config).unwrap();
    let bytes = journal.encode().unwrap();
    assert_eq!(Journal::decode(&bytes).unwrap(), journal);
    assert!(lift_accepts(&journal) && judge(&config, &journal).is_accepted());
    assert!(matches!(
        Journal::decode(&relabel(&bytes, 2)),
        Err(DecodeError::TagNotInVersion { version: 2, .. })
    ));
    assert!(matches!(
        model::read(&relabel(&bytes, 2)),
        Err(WireFault::NotInVersion { .. })
    ));
    for unread in [0_u8, 1, 4] {
        assert!(matches!(
            Journal::decode(&relabel(&bytes, unread)),
            Err(DecodeError::UnsupportedVersion { .. })
        ));
        assert_eq!(
            model::read(&relabel(&bytes, unread)),
            Err(WireFault::Header)
        );
    }
    // Each table that grew, alone.
    let spawn = EventBody::Lifecycle(LifecycleEvent::TaskSpawned {
        task: TaskOrdinal(0),
        region: RegionOrdinal(0),
        resumability: Resumability::Resumable,
    });
    for (table, event) in [
        (
            "lifecycle event",
            EventBody::Lifecycle(LifecycleEvent::RegionCrashed {
                region: RegionOrdinal(0),
                fenced: TaskSet::new([TaskOrdinal(0)]),
            }),
        ),
        (
            "effect event",
            EventBody::Effect(EffectEvent::Fenced {
                reservation: continuum_asupersync::family::effect::ReservationOrdinal(0),
            }),
        ),
        (
            "obligation event",
            EventBody::Obligation(ObligationEvent::Fenced {
                obligation: ObligationOrdinal(0),
            }),
        ),
        (
            "time event",
            EventBody::Time(TimeEvent::Fenced {
                timer: continuum_asupersync::family::time::TimerOrdinal(0),
            }),
        ),
    ] {
        let bytes = rebuild([spawn.clone(), event]).encode().unwrap();
        assert!(Journal::decode(&bytes).is_ok(), "{table}");
        assert!(model::read(&bytes).is_ok(), "{table}");
        assert!(
            matches!(
                Journal::decode(&relabel(&bytes, 2)),
                Err(DecodeError::TagNotInVersion { table: found, version: 2, .. }) if found == table
            ),
            "{table}: {:?}",
            Journal::decode(&relabel(&bytes, 2))
        );
        assert!(
            matches!(
                model::read(&relabel(&bytes, 2)),
                Err(WireFault::NotInVersion { .. })
            ),
            "{table}"
        );
    }
    // A version-3 settle labelled version 2 has a set too many for its payload.
    let settle = rebuild([EventBody::Obligation(ObligationEvent::RegionSettled {
        region: RegionOrdinal(0),
        open: ObligationSet::default(),
        leaked: ObligationSet::default(),
        fenced: ObligationSet::default(),
    })])
    .encode()
    .unwrap();
    assert!(matches!(
        Journal::decode(&relabel(&settle, 2)),
        Err(DecodeError::PayloadLength { .. })
    ));
    assert!(matches!(
        model::read(&relabel(&settle, 2)),
        Err(WireFault::PayloadLength { .. })
    ));
    // Journals of the version-2 grammar (settles included, and a deadline journal with
    // the bn-36wy3 tags) read as the same journal and get the same verdicts.
    let deadline = own_cancel_base();
    for (programs, config) in [
        (ledger(), all_families()),
        (deadline.clone(), every_projection().remove(31)),
    ] {
        let log = ChoiceLog::enumerate(&lengths(&programs)).remove(0);
        let journal = run(&programs, &log, &config).unwrap();
        let old = as_version_2(&journal);
        assert_eq!(Journal::decode(&old).unwrap(), journal);
        assert_eq!(
            model::read(&old).unwrap(),
            model::read(&journal.encode().unwrap()).unwrap()
        );
        assert_eq!(
            model::judge(&alphabet(&config), &old),
            judge(&config, &journal)
        );
        assert!(lift_accepts(&Journal::decode(&old).unwrap()));
    }
}

// --- a requested task acknowledges before it does anything (bn-28hup) ---------------

/// The window runs: `t0` in `r1` holds a lease `o0` and the receiver of `c0`, where one
/// message waits; `t1` in the root holds a lease and the receiver of `c1`. Each case
/// adds `pre` ops, then `x`, one or two ops of `t0` (or one that hands `t0`
/// something), then cancels `r1`.
fn window_base() -> Vec<SubstrateOp> {
    vec![
        open(ROOT, R[1]),
        spawn(R[1], T[1]),
        spawn(ROOT, T[2]),
        begin(T[1]),
        begin(T[2]),
        acquire(T[1], 1, ObligationKind::Lease),
        acquire(T[2], 3, ObligationKind::Lease),
        SubstrateOp::OpenChannel {
            channel: C1,
            capacity: 2,
            receiver: T[1],
        },
        SubstrateOp::OpenChannel {
            channel: C2,
            capacity: 2,
            receiver: T[2],
        },
        send(T[2], C1),
    ]
}

/// One window case: its name, the ops before it, and the ops whose events move.
type WindowCase = (&'static str, Vec<SubstrateOp>, Vec<SubstrateOp>);

fn window_cases() -> Vec<WindowCase> {
    let label = ReservationLabel;
    vec![
        ("t0 reserves", vec![], vec![reserve(T[1], 10)]),
        ("t0 commits", vec![reserve(T[1], 10)], vec![commit(10)]),
        (
            "t0 aborts on purpose",
            vec![reserve(T[1], 10)],
            vec![SubstrateOp::Abort {
                reservation: label(10),
            }],
        ),
        (
            "t0 opens an obligation",
            vec![],
            vec![acquire(T[1], 11, ObligationKind::Lease)],
        ),
        ("t0 discharges its lease committed", vec![], vec![commit(1)]),
        (
            "t0 discharges its lease aborted",
            vec![],
            vec![SubstrateOp::Abort {
                reservation: label(1),
            }],
        ),
        (
            "t0 hands its lease to t1",
            vec![],
            vec![SubstrateOp::Transfer {
                reservation: label(1),
                to: T[2],
            }],
        ),
        (
            "t1 hands its lease to t0",
            vec![],
            vec![SubstrateOp::Transfer {
                reservation: label(3),
                to: T[1],
            }],
        ),
        (
            "t0 sleeps",
            vec![],
            vec![SubstrateOp::Sleep {
                task: T[1],
                nanos: 10,
            }],
        ),
        (
            "t0's timer fires",
            vec![SubstrateOp::Sleep {
                task: T[1],
                nanos: 10,
            }],
            vec![SubstrateOp::Advance { nanos: 10 }],
        ),
        ("t0 receives", vec![], vec![recv(C1)]),
        ("t0 waits to receive", vec![recv(C1)], vec![recv(C1)]),
        ("t0 sends", vec![], vec![send(T[1], C2)]),
        (
            "a channel is opened to t0",
            vec![],
            vec![SubstrateOp::OpenChannel {
                channel: ChannelLabel(3),
                capacity: 1,
                receiver: T[1],
            }],
        ),
        (
            "t0 resumes and parks",
            vec![],
            vec![SubstrateOp::Continue { task: T[1] }],
        ),
        (
            "t0 completes",
            vec![commit(1)],
            vec![SubstrateOp::Finish { task: T[1] }],
        ),
    ]
}

/// An event of a window case's moved block that is `t0`'s own work, or hands `t0`
/// something: refused in `t0`'s window under every projection (RFC 0026 correction 55
/// items 1-2). Written here from the correction and the cases, not from the lift's
/// classification: in [`window_cases`] every reservation, every discharge and every
/// hand-off a block carries is `t0`'s or goes to `t0`, and `c0` is `t0`'s channel.
fn acts_or_receives(body: &EventBody) -> bool {
    let t0 = TaskOrdinal(0);
    match body {
        EventBody::Lifecycle(LifecycleEvent::TaskStepped { task, step }) => {
            *task == t0
                && matches!(
                    step,
                    TaskStep::Begin
                        | TaskStep::Resume
                        | TaskStep::Suspend
                        | TaskStep::Complete
                        | TaskStep::Fail(_)
                )
        }
        EventBody::Effect(EffectEvent::Reserved { task, .. }) => *task == t0,
        EventBody::Effect(EffectEvent::Aborted { cause, .. }) => *cause == AbortCause::Explicit,
        EventBody::Effect(EffectEvent::Committed { .. })
        | EventBody::Time(TimeEvent::Fired { .. })
        | EventBody::Obligation(
            ObligationEvent::Transferred { .. }
            | ObligationEvent::Discharged {
                how: Discharge::Committed,
                ..
            },
        ) => true,
        EventBody::Time(TimeEvent::Scheduled { task, .. })
        | EventBody::Obligation(ObligationEvent::Opened { holder: task, .. })
        | EventBody::Channel(
            ChannelEvent::Opened { receiver: task, .. }
            | ChannelEvent::Sent { sender: task, .. }
            | ChannelEvent::SendBlocked { sender: task, .. }
            | ChannelEvent::SendClosed { sender: task, .. },
        ) => *task == t0,
        EventBody::Channel(
            ChannelEvent::Received { channel, .. }
            | ChannelEvent::RecvBlocked { channel }
            | ChannelEvent::RecvClosed { channel },
        ) => channel.0 == 0,
        _ => false,
    }
}

/// An event that is `t0`'s cleanup or end, which only the cancellation family can
/// place against the acknowledgement (RFC 0026 correction 55 item 3).
fn cleans_up_or_ends(body: &EventBody) -> bool {
    matches!(
        body,
        EventBody::Obligation(
            ObligationEvent::Discharged {
                how: Discharge::Aborted,
                ..
            } | ObligationEvent::Leaked { .. }
        ) | EventBody::Channel(ChannelEvent::ReceiverGone { .. })
    )
}

/// RFC 0026 correction 55 (bn-28hup): between a region's cancel request reaching a task
/// and the task's acknowledgement, the task takes no step, in every family and every
/// projection. Sixteen real runs, each a `t0` step (or a hand-off to `t0`) just before
/// `r1`'s cancellation, under all 32 projections: the real journal is accepted by the
/// lift and the model (the step outside the window). The same journal with the step's
/// events moved into `t0`'s window — right after the region's request, right after
/// `t0`'s `requested`, and right before its `acknowledged` — is refused by both, at the
/// same event, the lift as `BeforeAcknowledgement` (or `UnreportedRequest` before the
/// `requested` phase) and the model as `Unobserved` (or `UnreportedRequest`). A moved
/// block that is only another task's steps (`t1`'s poll that hands `t0` its lease,
/// under a projection without the obligation family) is accepted by both: `t1` may act
/// while `t0` has not acknowledged; so is a clock advance, which is not `t0`'s, before
/// `t0`'s timer fires (the fire is refused: the `Sleep` future traces it in `t0`'s own
/// poll). [`every_family_is_held_to_the_acknowledgement`] places single forged events of
/// every family. Before this rule, the lift admitted a
/// moved reserve, commit, abort, open, discharge, hand-off, send, receive, receipt and
/// lifecycle step, and refused a moved timer only when the projection reported
/// phases: measured once when this test was written (not re-run by it), with the lift's
/// rule turned off, 432 of the 480 blocks placed right after the region's request
/// lifted as `Conforms`.
#[test]
#[allow(clippy::too_many_lines)]
fn a_requested_task_acknowledges_before_it_does_anything() {
    use continuum_asupersync::family::cancellation::CancellationFault;
    let is_region_request = |b: &EventBody| {
        matches!(
            b,
            EventBody::Lifecycle(LifecycleEvent::RegionCancelRequested { .. })
        )
    };
    let is_requested = |b: &EventBody| {
        matches!(
            b,
            EventBody::Cancellation(CancellationEvent::Requested {
                task: TaskOrdinal(0),
                ..
            })
        )
    };
    let is_ack = |b: &EventBody| {
        matches!(
            b,
            EventBody::Cancellation(CancellationEvent::Acknowledged {
                task: TaskOrdinal(0)
            })
        )
    };
    let mut controls = 0_usize;
    let mut refused = 0_usize;
    let mut admitted = 0_usize;
    let mut refused_families = BTreeSet::new();
    let mut cases_refused = BTreeSet::new();
    for (name, pre, x) in window_cases() {
        let mut before: Vec<SubstrateOp> = window_base();
        before.extend(pre.iter().cloned());
        let mut whole = before.clone();
        whole.extend(x.iter().cloned());
        whole.push(SubstrateOp::Cancel { region: R[1] });
        for config in every_projection() {
            let prefix = run(
                &[before.clone()],
                &ChoiceLog::new(vec![0; before.len()]),
                &config,
            )
            .unwrap_or_else(|r| panic!("{name} {:?}: {r}", config.families));
            let journal = run(
                &[whole.clone()],
                &ChoiceLog::new(vec![0; whole.len()]),
                &config,
            )
            .unwrap_or_else(|r| panic!("{name} {:?}: {r}", config.families));
            // Outside the window: the real journal.
            assert!(
                lift_accepts(&journal) && judge(&config, &journal).is_accepted(),
                "{name} {:?}: {:?} / {}\n{}",
                config.families,
                lift(&journal),
                judge(&config, &journal),
                journal.render()
            );
            controls += 1;
            let all = bodies(&journal);
            let from = prefix.events().len();
            let request = all
                .iter()
                .position(is_region_request)
                .expect("r1's request");
            assert_eq!(bodies(&prefix), all[..from], "{name}: the prefix is shared");
            let block: Vec<EventBody> = all[from..request].to_vec();
            if block.is_empty() {
                continue; // this projection carries none of the step's events
            }
            // What the journal carries, which is what the lift reads (a run whose `t0`
            // ended before the request carries no phase even when observed).
            let phases = all.iter().any(|b| b.family() == Family::Cancellation);
            let mut rest = all.clone();
            rest.drain(from..request);
            let request = rest.iter().position(is_region_request).unwrap();
            let mut points = vec![request + 1];
            if let Some(at) = rest.iter().position(is_requested) {
                points.push(at + 1);
            }
            if let Some(at) = rest.iter().position(is_ack) {
                points.push(at);
            }
            points.sort_unstable();
            points.dedup();
            let refusable = block.iter().any(acts_or_receives)
                || (phases && block.iter().any(cleans_up_or_ends));
            for at in points {
                let mut b = rest.clone();
                b.splice(at..at, block.iter().cloned());
                let variant = rebuild(b.iter().cloned());
                let lifted = lift(&variant);
                let verdict = judge_carried(&b, &variant);
                let place = format!("{name} at {at} {:?}", config.families);
                if !refusable {
                    assert!(
                        matches!(lifted, LiftVerdict::Conforms(_)) && verdict.is_accepted(),
                        "{place}: {lifted:?} / {verdict}\n{}",
                        variant.render()
                    );
                    admitted += 1;
                    continue;
                }
                let (seq, unreported) = match lifted {
                    LiftVerdict::Violates {
                        seq,
                        reason:
                            Nonconformance::Cancellation(CancellationFault::BeforeAcknowledgement {
                                task: 0,
                                ..
                            }),
                    } => (seq, false),
                    LiftVerdict::Violates {
                        seq,
                        reason:
                            Nonconformance::Cancellation(CancellationFault::UnreportedRequest {
                                task: 0,
                                ..
                            }),
                    } => (seq, true),
                    other => panic!("{place}: {other:?}\n{}", variant.render()),
                };
                let seq = usize::try_from(seq).unwrap();
                assert!(
                    (at..at + block.len()).contains(&seq),
                    "{place}: refused at {seq}\n{}",
                    variant.render()
                );
                let expected = if unreported {
                    Fault::UnreportedRequest(0)
                } else {
                    Fault::Unobserved(0)
                };
                assert_eq!(
                    verdict,
                    Verdict::Rejected {
                        at: seq,
                        fault: expected
                    },
                    "{place}\n{}",
                    variant.render()
                );
                // `UnreportedRequest` exactly where the phases are carried and `t0`'s
                // `requested` has not come yet.
                assert_eq!(
                    unreported,
                    phases && !b[..seq].iter().any(is_requested),
                    "{place}"
                );
                refused_families.insert(b[seq].family().to_string());
                cases_refused.insert(name);
                refused += 1;
            }
        }
    }
    eprintln!(
        "controls {controls}, refused {refused}, admitted {admitted}, families {refused_families:?}"
    );
    assert_eq!(refused_families.len(), 5, "{refused_families:?}");
    assert_eq!(
        cases_refused.len(),
        window_cases().len(),
        "{cases_refused:?}"
    );
    assert_eq!((controls, refused, admitted), (512, 704, 24));
}

/// One forged event for [`every_family_is_held_to_the_acknowledgement`], built from the
/// base journal: its name, whether only a journal that reports phases can refuse it
/// (a cleanup or an end), and the event.
type WindowAttack = (&'static str, bool, EventBody);

#[allow(clippy::too_many_lines)]
fn window_attacks(base: &[EventBody]) -> Vec<WindowAttack> {
    let t = TaskOrdinal;
    let o = ObligationOrdinal;
    let c = continuum_asupersync::family::channel::ChannelOrdinal;
    let e = continuum_asupersync::family::effect::ReservationOrdinal;
    let lease_of = |holder: u32| {
        base.iter().find_map(|b| match b {
            EventBody::Obligation(ObligationEvent::Opened {
                obligation,
                kind: ObligationKind::Lease,
                holder: h,
                ..
            }) if h.0 == holder => Some(*obligation),
            _ => None,
        })
    };
    let opened = |b: &EventBody| matches!(b, EventBody::Obligation(ObligationEvent::Opened { .. }));
    let o_next = o(u32::try_from(base.iter().filter(|b| opened(b)).count()).unwrap());
    let e_next = e(u32::try_from(
        base.iter()
            .filter(|b| matches!(b, EventBody::Effect(EffectEvent::Reserved { .. })))
            .count(),
    )
    .unwrap());
    let (o0, o1) = (lease_of(0).unwrap_or(o(0)), lease_of(1).unwrap_or(o(1)));
    let fail = continuum_task::region::worker::FailureReason::new("boom").unwrap();
    let lc = |step| EventBody::Lifecycle(LifecycleEvent::TaskStepped { task: t(0), step });
    vec![
        ("t0 begins", false, lc(TaskStep::Begin)),
        ("t0 resumes", false, lc(TaskStep::Resume)),
        ("t0 completes", false, lc(TaskStep::Complete)),
        ("t0 fails", false, lc(TaskStep::Fail(fail))),
        (
            "t0 reserves",
            false,
            EventBody::Effect(EffectEvent::Reserved {
                reservation: e_next,
                task: t(0),
            }),
        ),
        (
            "t0 commits e0",
            false,
            EventBody::Effect(EffectEvent::Committed { reservation: e(0) }),
        ),
        (
            "t0 aborts e0 on purpose",
            false,
            EventBody::Effect(EffectEvent::Aborted {
                reservation: e(0),
                cause: AbortCause::Explicit,
            }),
        ),
        (
            "t0 opens a lease",
            false,
            EventBody::Obligation(ObligationEvent::Opened {
                obligation: o_next,
                kind: ObligationKind::Lease,
                holder: t(0),
                region: RegionOrdinal(1),
            }),
        ),
        (
            "t0 discharges its lease committed",
            false,
            EventBody::Obligation(ObligationEvent::Discharged {
                obligation: o0,
                how: Discharge::Committed,
            }),
        ),
        (
            "t0 hands its lease to t1",
            false,
            EventBody::Obligation(ObligationEvent::Transferred {
                obligation: o0,
                holder: t(1),
                region: RegionOrdinal(0),
            }),
        ),
        (
            "t1 hands its lease to t0",
            false,
            EventBody::Obligation(ObligationEvent::Transferred {
                obligation: o1,
                holder: t(0),
                region: RegionOrdinal(1),
            }),
        ),
        (
            "t0 arms a timer",
            false,
            EventBody::Time(TimeEvent::Scheduled {
                timer: continuum_asupersync::family::time::TimerOrdinal(0),
                task: t(0),
                at: VirtualInstant(0),
                deadline: VirtualInstant(10),
            }),
        ),
        (
            "t0 sends on c1",
            false,
            EventBody::Channel(ChannelEvent::Sent {
                channel: c(1),
                message: MessageOrdinal(1),
                sender: t(0),
            }),
        ),
        (
            "t0's send waits on c1",
            false,
            EventBody::Channel(ChannelEvent::SendBlocked {
                channel: c(1),
                message: MessageOrdinal(1),
                sender: t(0),
            }),
        ),
        (
            "t0's send finds c1 closed",
            false,
            EventBody::Channel(ChannelEvent::SendClosed {
                channel: c(1),
                message: MessageOrdinal(1),
                sender: t(0),
            }),
        ),
        (
            "t0's receive finds c0 closed",
            false,
            EventBody::Channel(ChannelEvent::RecvClosed { channel: c(0) }),
        ),
        (
            "t0 receives m0",
            false,
            EventBody::Channel(ChannelEvent::Received {
                channel: c(0),
                message: MessageOrdinal(0),
            }),
        ),
        (
            "t0 is handed a new channel's receiver",
            false,
            EventBody::Channel(ChannelEvent::Opened {
                channel: c(2),
                capacity: 1,
                receiver: t(0),
            }),
        ),
        (
            "t0 discharges its lease aborted",
            true,
            EventBody::Obligation(ObligationEvent::Discharged {
                obligation: o0,
                how: Discharge::Aborted,
            }),
        ),
        (
            "t0 leaks its lease",
            true,
            EventBody::Obligation(ObligationEvent::Leaked { obligation: o0 }),
        ),
        (
            "t0's receiver goes",
            true,
            EventBody::Channel(ChannelEvent::ReceiverGone {
                channel: c(0),
                discarded: MessageSet::new([MessageOrdinal(0)]),
            }),
        ),
    ]
}

/// RFC 0026 correction 55 (bn-28hup), one forged event at a time: a real run in which
/// `t0` (in `r1`) holds a reservation `e0`, a lease and a channel's receiver with one
/// message queued, next to `t1` in the root, and `r1` is cancelled. For each of the 32
/// projections, each forged event of a family the projection carries is placed in
/// `t0`'s window (right after the region's request, right after `t0`'s `requested`,
/// right before its `acknowledged`): its own work in the five families other than the
/// cancellation family (whose phases are the window), a receipt of an
/// obligation or of a channel's receiver, and, where the journal reports phases, its
/// cleanup or end. Each is refused by the lift (`BeforeAcknowledgement`, or
/// `UnreportedRequest` before the `requested` phase) and by the model (`Unobserved` or
/// `UnreportedRequest`), at the forged event, 576 placements. The real journal is
/// accepted by both. Measured once for the first 17 attacks, with the lift's rule
/// turned off: 36 of their 456 placements lifted as `Conforms` (a hand-off out of `t0`
/// and a send by `t0`), and the rest were refused only later, by a rule that is not
/// about the window, so the fault and its position are asserted.
#[test]
fn every_family_is_held_to_the_acknowledgement() {
    use continuum_asupersync::family::cancellation::CancellationFault;
    let mut program = window_base();
    program.push(reserve(T[1], 10));
    program.push(SubstrateOp::Cancel { region: R[1] });
    let log = ChoiceLog::new(vec![0; program.len()]);
    let mut placed = 0_usize;
    let mut families = BTreeSet::new();
    for config in every_projection() {
        let journal = run(&[program.clone()], &log, &config).unwrap();
        assert!(
            lift_accepts(&journal) && judge(&config, &journal).is_accepted(),
            "{:?}: {:?} / {}
{}",
            config.families,
            lift(&journal),
            judge(&config, &journal),
            journal.render()
        );
        let base = bodies(&journal);
        let phases = base.iter().any(|b| b.family() == Family::Cancellation);
        let request = base
            .iter()
            .position(|b| {
                matches!(
                    b,
                    EventBody::Lifecycle(LifecycleEvent::RegionCancelRequested { .. })
                )
            })
            .unwrap();
        let mut points = vec![request + 1];
        for (i, b) in base.iter().enumerate() {
            match b {
                EventBody::Cancellation(CancellationEvent::Requested {
                    task: TaskOrdinal(0),
                    ..
                }) => points.push(i + 1),
                EventBody::Cancellation(CancellationEvent::Acknowledged {
                    task: TaskOrdinal(0),
                }) => points.push(i),
                _ => {}
            }
        }
        points.sort_unstable();
        points.dedup();
        for (name, needs_phases, event) in window_attacks(&base) {
            if !config.families.contains(event.family()) || (needs_phases && !phases) {
                continue;
            }
            for &at in &points {
                let mut b = base.clone();
                b.insert(at, event.clone());
                let variant = rebuild(b.iter().cloned());
                let place = format!("{name} at {at} {:?}", config.families);
                let unreported = phases
                    && !b[..at].iter().any(|e| {
                        matches!(
                            e,
                            EventBody::Cancellation(CancellationEvent::Requested {
                                task: TaskOrdinal(0),
                                ..
                            })
                        )
                    });
                let seq = u64::try_from(at).unwrap();
                match lift(&variant) {
                    LiftVerdict::Violates {
                        seq: s,
                        reason:
                            Nonconformance::Cancellation(CancellationFault::BeforeAcknowledgement {
                                task: 0,
                                ..
                            }),
                    } if s == seq && !unreported => {}
                    LiftVerdict::Violates {
                        seq: s,
                        reason:
                            Nonconformance::Cancellation(CancellationFault::UnreportedRequest {
                                task: 0,
                                ..
                            }),
                    } if s == seq && unreported => {}
                    other => panic!("{place}: {other:?}\n{}", variant.render()),
                }
                let fault = if unreported {
                    Fault::UnreportedRequest(0)
                } else {
                    Fault::Unobserved(0)
                };
                assert_eq!(
                    judge(&config, &variant),
                    Verdict::Rejected { at, fault },
                    "{place}"
                );
                families.insert(event.family().to_string());
                placed += 1;
            }
        }
    }
    // Every family but the cancellation family, whose phases are the window itself.
    assert_eq!(families.len(), 5, "{families:?}");
    assert_eq!(placed, 576);
}
