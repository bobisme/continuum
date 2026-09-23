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

#[path = "support/primitive_conformance_model.rs"]
mod model;

use std::collections::BTreeMap;

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
    assert_eq!(runs, 7_513, "docs/18 C023 cites this count");
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
    assert_eq!(positions, 29_761, "docs/18 C023 cites this count");
    assert!(exact_checked > positions, "{exact_checked}");
}

// --- rejection -----------------------------------------------------------------------

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

fn mutants() -> Vec<Mutant> {
    vec![
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
            // effect abort that precedes it.
            |f| matches!(f, Fault::CancelPhase(_) | Fault::TaskPhase(_)),
        ),
        (
            "a request carries the other cause",
            |j| {
                let mut b = bodies(j);
                let at = position(j, |e| {
                    matches!(
                        e,
                        EventBody::Cancellation(CancellationEvent::Requested { .. })
                    )
                })?;
                if let EventBody::Cancellation(CancellationEvent::Requested { task, cause }) = b[at]
                {
                    let other = match cause {
                        CancelCause::User => CancelCause::ParentCancelled,
                        CancelCause::ParentCancelled => CancelCause::User,
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
            |f| matches!(f, Fault::TaskPhase(_)),
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
            |f| matches!(f, Fault::TaskPhase(_) | Fault::CancelPhase(_)),
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
    // Channel journals: every tenth sampled log of each channel set.
    let mut rng = Splitmix(0xc4a7);
    for (setup, actors) in [channels_cancel(), channels_close()] {
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
    assert_eq!(mutants().len(), 21, "docs/18 C023 cites this count");
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
/// One known divergence remains, in the other direction, and it is pinned here:
/// RFC 0026 correction 51's "known strictness, owned by bn-36wy3". The calculus decides
/// "cancelling" per region, at the cancel request, and the model per task, at the
/// task's acknowledgement. So an obligation transfer, or a committed discharge, placed
/// between a cancel request and the holder's acknowledgement is admitted by the model
/// and refused by the calculus (`SubstrateRegionNotOpen`,
/// `SubstrateCommitDuringCancellation`). No real journal falls in that window; only
/// perturbations do. The lift errs toward nonconformance, never toward a false
/// `is_total`. When bn-36wy3 relaxes the calculus the count drops to zero, and this pin
/// becomes the regression guard that it stays there.
const KNOWN_STRICTNESS: usize = 33;

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
    assert_eq!(compared, 23_587, "docs/18 C023 cites this count");
    assert_eq!(
        known_strictness, KNOWN_STRICTNESS,
        "RFC 0026 correction 51's known strictness (bn-36wy3); docs/18 C023 cites it"
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
