//! A fail-stop crash in the substrate binding (bn-20d8u path (a); RFC 0026 correction
//! 58): `SubstrateOp::Crash`, its journal events, its lift, and the A7 model's own crash
//! rules.
//!
//! The process pack's profile `process/crash-restart-v0` (bn-3mmf) states the
//! semantics, row `fail-stop-crash`: "no code of that incarnation runs after it, and no
//! finalizer, cancellation handler or cleanup runs at it", and row `late-completion`: an
//! operation the incarnation began stays pending and its completion is fenced. The
//! binding realizes it on asupersync 0.5.0 without a substrate change: it withdraws each
//! stopped task's future from the runtime, unpolled, and ends the task's record through
//! the substrate's own abnormal completion, whose audit leaks what the task held. That
//! leak is the fence. The journal says so: `region-crashed` names the stopped tasks, and
//! each reservation, obligation and timer they held is `fenced`, never aborted, leaked
//! or discharged.
//!
//! # Evidence map
//!
//! | Claim | Test |
//! |---|---|
//! | a crash runs nothing of the stopped tasks: no phase, no abort, no step of theirs after it, where a graceful `Cancel` of the same program runs each | [`a_crash_runs_no_code_of_the_stopped_tasks`] |
//! | a crashed task takes no command (`TaskCrashed`), begun or not | [`a_crashed_task_takes_no_command`] |
//! | a crash the binding gives no fail-stop semantics is a typed refusal, INV-008 `Unsupported`; a closed region is `SubstrateRefused` | [`unsupported_crashes_are_typed_refusals`] |
//! | a stopped sleeper's timer is fenced, and its later fire reaches no task | [`a_fenced_timer_fires_into_no_task`] |
//! | the crash journal is seed-independent | [`the_crash_journal_does_not_depend_on_the_lab_seed`] |
//! | every crash journal of the corpus, under all 32 projections, lifts as `Conforms` and the A7 model accepts it; the model's enabled steps include each step taken | [`every_crash_journal_conforms_and_the_model_accepts_it`] |
//! | curated perturbations of real crash journals: the model rejects each for its own fault, and the lift does not conform | [`perturbed_crash_journals_are_rejected_by_both`] |
//! | every single-event deletion and adjacent swap of the crash corpus: the model and the lift agree | [`deletions_and_swaps_of_crash_journals_get_one_verdict`] |
//! | every prefix: one cut inside a crash (fences owed, or the crashed subtree not finalized) never conforms, and the model agrees | [`a_journal_cut_inside_a_crash_does_not_conform`] |
//! | the scripted source's crash report journals the same stopped set as the binding | [`the_scripted_source_reports_the_same_crash`] |
//! | a crash may not fence a timer already due, or crash a subtree under cancellation (pre-review) | [`a_crash_does_not_launder_a_late_timer_or_a_cancellation`] |
//! | an unsupported crash ends judgement at its event (RFC 0026 correction 61, bn-1id0n): the lift's `Inconclusive(Unsupported)` and the model's `Unsupported` at the crash, whether a genuine violation, no violation, or events whose legality turns on the crash's effect follow; a violation at the crash event outranks it | [`an_unsupported_crash_ends_judgement_at_its_event`] |
//! | every time-observing crash journal of the corpus, with a far deadline declared for a stopped task, is unsupported at the crash in the lift and the model alike; the deletions and swaps of a sample get one verdict and one unsupported position from both | [`an_unsupported_crash_in_a_real_journal_ends_judgement_in_both`] |
//! | the calculus's own crash step (RFC 0026 correction 59, bn-fxxf2): every corpus journal, replayed on a fresh calculus through its public operations, is a legal step sequence and reaches the lift's calculus state; a calculus without the crash step refuses every journal that crashes a parked worker | [`every_lifted_crash_is_a_legal_calculus_step_sequence`] |
//! | on the mutation corpus (the perturbations, deletions and swaps above) the lift conforms to nothing the calculus replay refuses, and the replay refuses each mutant the lift refuses as a calculus fault, at the same event and for the same fault | [`on_the_mutation_corpus_the_calculus_and_the_lift_agree_on_the_calculus_steps`] |

#![allow(clippy::too_many_lines)]

#[path = "support/primitive_conformance_model.rs"]
mod model;

use std::collections::BTreeMap;

use continuum_asupersync::binding::{BindingConfig, BindingRefusal, Program, SubstrateOp, run};
use continuum_asupersync::choice::ChoiceLog;
use continuum_asupersync::family::cancellation::CancellationEvent;
use continuum_asupersync::family::channel::{
    ChannelEvent, ChannelOrdinal, MessageOrdinal, MessageSet,
};
use continuum_asupersync::family::effect::{
    AbortCause, EffectEvent, ReservationLabel, ReservationOrdinal,
};
use continuum_asupersync::family::lifecycle::{
    LifecycleEvent, LifecycleReport, RegionLabel, RegionOrdinal, TaskLabel, TaskOrdinal, TaskSet,
    TaskStep,
};
use continuum_asupersync::family::obligation::{
    ObligationEvent, ObligationKind, ObligationOrdinal, ObligationSet,
};
use continuum_asupersync::family::time::{TimeEvent, TimerOrdinal};
use continuum_asupersync::family::{EventBody, Family, Report};
use continuum_asupersync::journal::Journal;
use continuum_asupersync::lift::{LiftVerdict, lift};
use continuum_asupersync::source::record;
use continuum_task::region::worker::Resumability;
use continuum_value::assurance::InconclusiveReason;

use model::{Alphabet, FamilyTag, Fault, Model, Verdict};

const ROOT: RegionLabel = RegionLabel::ROOT;
const R: [RegionLabel; 4] = [
    RegionLabel(0),
    RegionLabel(1),
    RegionLabel(2),
    RegionLabel(3),
];
const T: [TaskLabel; 6] = [
    TaskLabel(0),
    TaskLabel(1),
    TaskLabel(2),
    TaskLabel(3),
    TaskLabel(4),
    TaskLabel(5),
];

fn open(parent: RegionLabel, child: RegionLabel) -> SubstrateOp {
    SubstrateOp::OpenRegion { parent, child }
}

fn spawn(region: RegionLabel, task: TaskLabel) -> SubstrateOp {
    SubstrateOp::Spawn {
        region,
        task,
        resumability: Resumability::Resumable,
    }
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

const fn acquire(task: TaskLabel, label: u32, kind: ObligationKind) -> SubstrateOp {
    SubstrateOp::Acquire {
        task,
        reservation: ReservationLabel(label),
        kind,
    }
}

const fn crash(region: RegionLabel) -> SubstrateOp {
    SubstrateOp::Crash { region }
}

fn all_families() -> BindingConfig {
    BindingConfig::new(0)
        .observing(Family::Effect)
        .observing(Family::Cancellation)
        .observing(Family::Obligation)
        .observing(Family::Time)
        .observing(Family::Channel)
}

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
            let mut config = BindingConfig::new(0);
            for (bit, family) in others.iter().enumerate() {
                if mask & (1 << bit) != 0 {
                    config = config.observing(*family);
                }
            }
            config
        })
        .collect()
}

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

fn judge(config: &BindingConfig, journal: &Journal) -> Verdict {
    model::judge(&alphabet(config), &journal.encode().unwrap())
}

fn lift_accepts(journal: &Journal) -> bool {
    matches!(lift(journal), LiftVerdict::Conforms(_))
}

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

fn one_actor(ops: Vec<SubstrateOp>) -> (Vec<Program>, ChoiceLog) {
    let log = ChoiceLog::new(vec![0; ops.len()]);
    (vec![ops], log)
}

/// One program, one actor: `r1` holds `t1` (a reservation, begun), `t4` (never begun)
/// and the subregion `r2`, which holds `t2` (a lease, asleep on a timer when `sleep`);
/// `t3` in the root works on. `r1` crashes, the clock moves past `t2`'s deadline, `t3`
/// finishes, and the root closes.
fn crash_program(sleep: bool) -> Program {
    let mut ops = vec![
        open(ROOT, R[1]),
        open(R[1], R[2]),
        spawn(R[1], T[1]),
        spawn(R[2], T[2]),
        spawn(ROOT, T[3]),
        spawn(R[1], T[4]),
        begin(T[1]),
        begin(T[2]),
        begin(T[3]),
        reserve(T[1], 1),
        acquire(T[2], 2, ObligationKind::Lease),
    ];
    if sleep {
        ops.push(SubstrateOp::Sleep {
            task: T[2],
            nanos: 10,
        });
    }
    ops.extend([
        crash(R[1]),
        SubstrateOp::Advance { nanos: 20 },
        reserve(T[3], 3),
        SubstrateOp::Commit {
            reservation: ReservationLabel(3),
        },
        SubstrateOp::Finish { task: T[3] },
        SubstrateOp::Close { region: ROOT },
    ]);
    ops
}

/// The corpus's program sets: a setup (actor 0) and actors whose every interleaving
/// either runs or is refused as a command to a crashed task.
///
/// - `crash-work`: `r1` (`t1` with a reservation, `t4` never begun, subregion `r2` with
///   `t2` asleep holding a lease) crashes while `t1` commits and opens an I/O
///   operation, `t3` in `r3` reserves and commits, and the clock advances past `t2`'s
///   deadline, in every order;
/// - `crash-and-cancel`: `r2` crashes and `r1` is cancelled, in either order: the
///   cancellation's cleanup runs for `t1` and never for the crashed `t2`, and a crash
///   after the cancellation is refused (the region is closed).
fn corpus_sets() -> Vec<(&'static str, Program, Vec<Program>)> {
    let setup = vec![
        open(ROOT, R[1]),
        open(R[1], R[2]),
        open(ROOT, R[3]),
        spawn(R[1], T[1]),
        spawn(R[2], T[2]),
        spawn(R[3], T[3]),
        spawn(R[1], T[4]),
        begin(T[1]),
        begin(T[2]),
        begin(T[3]),
        reserve(T[1], 1),
        acquire(T[2], 2, ObligationKind::Lease),
        SubstrateOp::Sleep {
            task: T[2],
            nanos: 10,
        },
    ];
    let work = vec![
        vec![crash(R[1])],
        vec![
            SubstrateOp::Commit {
                reservation: ReservationLabel(1),
            },
            acquire(T[1], 4, ObligationKind::IoOp),
        ],
        vec![
            reserve(T[3], 5),
            SubstrateOp::Commit {
                reservation: ReservationLabel(5),
            },
        ],
        vec![SubstrateOp::Advance { nanos: 20 }],
    ];
    let cancel_setup = vec![
        open(ROOT, R[1]),
        open(R[1], R[2]),
        spawn(R[1], T[1]),
        spawn(R[2], T[2]),
        begin(T[1]),
        begin(T[2]),
        reserve(T[1], 1),
        acquire(T[2], 2, ObligationKind::Lease),
    ];
    let cancel = vec![
        vec![crash(R[2])],
        vec![SubstrateOp::Cancel { region: R[1] }],
    ];
    vec![
        ("crash-work", setup, work),
        ("crash-and-cancel", cancel_setup, cancel),
    ]
}

/// A corpus entry: a program set, a log, a projection, and its journal.
struct Entry {
    name: &'static str,
    log: ChoiceLog,
    config: BindingConfig,
    journal: Journal,
}

/// Every log of every set, under every projection: the journals, and how many runs were
/// refused and why. Only a command to a crashed task, or a crash of a closed region,
/// may be refused.
fn corpus() -> (Vec<Entry>, BTreeMap<String, usize>) {
    let mut out = Vec::new();
    let mut refused = BTreeMap::new();
    for (name, setup, actors) in corpus_sets() {
        let lengths: Vec<usize> = actors.iter().map(Vec::len).collect();
        let mut programs = vec![setup.clone()];
        programs.extend(actors);
        for tail in ChoiceLog::enumerate(&lengths) {
            let mut choices = vec![0; setup.len()];
            choices.extend(tail.choices().iter().map(|c| c.0));
            let log = ChoiceLog::new(choices);
            for config in every_projection() {
                match run(&programs, &log, &config) {
                    Ok(journal) => out.push(Entry {
                        name,
                        log: log.clone(),
                        config,
                        journal,
                    }),
                    Err(BindingRefusal::TaskCrashed { .. }) => {
                        *refused.entry(format!("{name}: task-crashed")).or_default() += 1;
                    }
                    Err(BindingRefusal::SubstrateRefused {
                        operation: "crash", ..
                    }) => {
                        *refused.entry(format!("{name}: crash refused")).or_default() += 1;
                    }
                    Err(other) => panic!("{name} log {log}: {other}"),
                }
            }
        }
    }
    (out, refused)
}

fn has_crash(journal: &Journal) -> bool {
    journal.events().iter().any(|e| {
        matches!(
            e.body(),
            EventBody::Lifecycle(LifecycleEvent::RegionCrashed { .. })
        )
    })
}

// --- the binding ----------------------------------------------------------------------

#[test]
fn a_crash_runs_no_code_of_the_stopped_tasks() {
    let (programs, log) = one_actor(crash_program(true));
    let journal = run(&programs, &log, &all_families()).unwrap();
    let all = bodies(&journal);
    let at = all
        .iter()
        .position(|b| {
            matches!(
                b,
                EventBody::Lifecycle(LifecycleEvent::RegionCrashed { .. })
            )
        })
        .unwrap();
    // t0 (label t1), t1 (label t2) and t3 (label t4, never begun) stop; t2 (label t3)
    // is in the root and works on.
    assert_eq!(
        all[at],
        EventBody::Lifecycle(LifecycleEvent::RegionCrashed {
            region: RegionOrdinal(1),
            fenced: TaskSet::new([TaskOrdinal(0), TaskOrdinal(1), TaskOrdinal(3)]),
        })
    );
    // Right after it, the fences: the reservation, both obligations, the timer, in the
    // binding's canonical order; then the subtree closes, children first.
    assert_eq!(
        all[at + 1..at + 9],
        [
            EventBody::Effect(EffectEvent::Fenced {
                reservation: ReservationOrdinal(0)
            }),
            EventBody::Obligation(ObligationEvent::Fenced {
                obligation: ObligationOrdinal(0)
            }),
            EventBody::Obligation(ObligationEvent::Fenced {
                obligation: ObligationOrdinal(1)
            }),
            EventBody::Time(TimeEvent::Fenced {
                timer: TimerOrdinal(0)
            }),
            EventBody::Lifecycle(LifecycleEvent::RegionDrained {
                region: RegionOrdinal(2),
                cancelled: TaskSet::default(),
            }),
            EventBody::Lifecycle(LifecycleEvent::RegionFinalized {
                region: RegionOrdinal(2),
            }),
            EventBody::Obligation(ObligationEvent::RegionSettled {
                region: RegionOrdinal(2),
                open: ObligationSet::default(),
                leaked: ObligationSet::default(),
                fenced: ObligationSet::new([ObligationOrdinal(1)]),
            }),
            EventBody::Lifecycle(LifecycleEvent::RegionDrained {
                region: RegionOrdinal(1),
                cancelled: TaskSet::default(),
            }),
        ]
    );
    // Nothing of a stopped task, after the crash or anywhere: no cancellation phase, no
    // abort, no discharge, no leak, no fire, no lifecycle step.
    let stopped = [0_u32, 1, 3];
    for body in &all[at + 1..] {
        let names_stopped = match body {
            EventBody::Lifecycle(LifecycleEvent::TaskStepped { task, .. }) => {
                stopped.contains(&task.0)
            }
            EventBody::Cancellation(_) => true,
            EventBody::Effect(EffectEvent::Aborted { .. } | EffectEvent::Committed { .. }) => {
                matches!(body, EventBody::Effect(e) if e.reservation().0 == 0)
            }
            EventBody::Obligation(
                ObligationEvent::Discharged { obligation, .. }
                | ObligationEvent::Leaked { obligation },
            ) => obligation.0 < 2,
            EventBody::Time(TimeEvent::Fired { .. } | TimeEvent::Cancelled { .. }) => true,
            _ => false,
        };
        assert!(!names_stopped, "{body:?}\n{}", journal.render());
    }
    assert!(lift_accepts(&journal), "{:?}", lift(&journal));
    assert!(judge(&all_families(), &journal).is_accepted());

    // The same program with a graceful `Cancel` in place of the crash runs each task's
    // cancellation: its phases, its cancel aborts, its timer cancelled.
    let graceful: Program = crash_program(true)
        .into_iter()
        .map(|op| match op {
            SubstrateOp::Crash { region } => SubstrateOp::Cancel { region },
            other => other,
        })
        .collect();
    let (programs, log) = one_actor(graceful);
    let journal = run(&programs, &log, &all_families()).unwrap();
    let all = bodies(&journal);
    assert!(all.iter().any(|b| matches!(
        b,
        EventBody::Cancellation(CancellationEvent::Acknowledged { .. })
    )));
    assert!(all.iter().any(|b| matches!(
        b,
        EventBody::Effect(EffectEvent::Aborted {
            cause: AbortCause::Cancel,
            ..
        })
    )));
    assert!(
        all.iter()
            .any(|b| matches!(b, EventBody::Time(TimeEvent::Cancelled { .. })))
    );
    assert!(!has_crash(&journal));
}

#[test]
fn a_crashed_task_takes_no_command() {
    let before = crash_program(false);
    let cut = before
        .iter()
        .position(|op| matches!(op, SubstrateOp::Crash { .. }))
        .unwrap();
    let prefix = &before[..=cut];
    for (what, op, task) in [
        ("continue", SubstrateOp::Continue { task: T[1] }, 0),
        ("finish", SubstrateOp::Finish { task: T[2] }, 1),
        (
            "commit",
            SubstrateOp::Commit {
                reservation: ReservationLabel(1),
            },
            0,
        ),
        (
            "abort",
            SubstrateOp::Abort {
                reservation: ReservationLabel(1),
            },
            0,
        ),
        ("reserve", reserve(T[1], 9), 0),
        (
            "sleep",
            SubstrateOp::Sleep {
                task: T[2],
                nanos: 5,
            },
            1,
        ),
        ("begin a never-begun task", begin(T[4]), 3),
    ] {
        let mut ops = prefix.to_vec();
        ops.push(op);
        let (programs, log) = one_actor(ops);
        assert_eq!(
            run(&programs, &log, &all_families()).unwrap_err(),
            BindingRefusal::TaskCrashed { task },
            "{what}"
        );
        assert_eq!(
            BindingRefusal::TaskCrashed { task }.inconclusive_reason(),
            None
        );
    }
}

#[test]
fn unsupported_crashes_are_typed_refusals() {
    let base = || {
        vec![
            open(ROOT, R[1]),
            spawn(R[1], T[1]),
            spawn(ROOT, T[2]),
            begin(T[1]),
            begin(T[2]),
        ]
    };
    let mut deadline = vec![
        open(ROOT, R[1]),
        SubstrateOp::SpawnWithDeadline {
            region: R[1],
            task: T[1],
            resumability: Resumability::Resumable,
            deadline: 100,
        },
        begin(T[1]),
    ];
    deadline.push(crash(R[1]));
    let mut receiver = base();
    receiver.extend([
        SubstrateOp::OpenChannel {
            channel: continuum_asupersync::family::channel::ChannelLabel(1),
            capacity: 1,
            receiver: T[1],
        },
        crash(R[1]),
    ]);
    let mut blocked = base();
    blocked.extend([
        SubstrateOp::OpenChannel {
            channel: continuum_asupersync::family::channel::ChannelLabel(1),
            capacity: 1,
            receiver: T[2],
        },
        SubstrateOp::Send {
            task: T[1],
            channel: continuum_asupersync::family::channel::ChannelLabel(1),
        },
        SubstrateOp::Send {
            task: T[1],
            channel: continuum_asupersync::family::channel::ChannelLabel(1),
        },
        crash(R[1]),
    ]);
    for (what, ops) in [
        ("deadline", deadline),
        ("receiver", receiver),
        ("blocked send", blocked),
    ] {
        let (programs, log) = one_actor(ops);
        let refusal = run(&programs, &log, &all_families()).unwrap_err();
        assert!(
            matches!(refusal, BindingRefusal::CrashUnsupported { task: 0, .. }),
            "{what}: {refusal}"
        );
        assert_eq!(
            refusal.inconclusive_reason(),
            Some(InconclusiveReason::Unsupported),
            "{what}"
        );
    }
    // A closed region, cancelled or crashed before, does not crash.
    for first in [SubstrateOp::Cancel { region: R[1] }, crash(R[1])] {
        let mut ops = base();
        ops.extend([first, crash(R[1])]);
        let (programs, log) = one_actor(ops);
        assert!(matches!(
            run(&programs, &log, &all_families()).unwrap_err(),
            BindingRefusal::SubstrateRefused {
                operation: "crash",
                ..
            }
        ));
    }
}

#[test]
fn a_fenced_timer_fires_into_no_task() {
    let (programs, log) = one_actor(crash_program(true));
    let journal = run(&programs, &log, &all_families()).unwrap();
    let all = bodies(&journal);
    let fenced = all
        .iter()
        .position(|b| matches!(b, EventBody::Time(TimeEvent::Fenced { .. })))
        .unwrap();
    let advanced = all
        .iter()
        .position(|b| matches!(b, EventBody::Time(TimeEvent::Advanced { .. })))
        .unwrap();
    // The clock passes the fenced timer's deadline (10) and nothing fires or wakes.
    assert!(fenced < advanced);
    assert!(matches!(
        all[advanced],
        EventBody::Time(TimeEvent::Advanced { to, .. }) if to.0 >= 10
    ));
    assert!(
        !all.iter()
            .any(|b| matches!(b, EventBody::Time(TimeEvent::Fired { .. })))
    );
    assert!(lift_accepts(&journal));
}

#[test]
fn the_crash_journal_does_not_depend_on_the_lab_seed() {
    for (programs, log) in [
        one_actor(crash_program(true)),
        one_actor(crash_program(false)),
    ] {
        let base = run(&programs, &log, &all_families()).unwrap();
        for seed in 1..8 {
            let config = BindingConfig::new(seed)
                .observing(Family::Effect)
                .observing(Family::Cancellation)
                .observing(Family::Obligation)
                .observing(Family::Time)
                .observing(Family::Channel);
            assert_eq!(run(&programs, &log, &config).unwrap(), base, "seed {seed}");
        }
    }
}

// --- the lift and the A7 model ---------------------------------------------------------

#[test]
fn every_crash_journal_conforms_and_the_model_accepts_it() {
    let (corpus, refused) = corpus();
    let mut crashed = 0_usize;
    let mut positions = 0_usize;
    for entry in &corpus {
        let journal = &entry.journal;
        assert!(
            lift_accepts(journal),
            "{} log {}: {:?}\n{}",
            entry.name,
            entry.log,
            lift(journal),
            journal.render()
        );
        assert_eq!(
            judge(&entry.config, journal),
            Verdict::Accepted {
                steps: journal.len()
            },
            "{} log {}\n{}",
            entry.name,
            entry.log,
            journal.render()
        );
        crashed += usize::from(has_crash(journal));
        // The converse, on the full projection: the model's generator enables each step
        // the substrate took, and its guard admits each exact step it generates.
        if entry.config == all_families() {
            let mut state = Model::new(alphabet(&entry.config));
            for step in model::read(&journal.encode().unwrap()).unwrap() {
                let enabled = state.enabled();
                assert!(
                    enabled.iter().any(|p| p.admits(&step)),
                    "{} log {}: {step:?} not enabled",
                    entry.name,
                    entry.log
                );
                for pattern in &enabled {
                    if let model::Pattern::Exact(candidate) = pattern {
                        let mut probe = state.clone();
                        assert_eq!(probe.step(candidate), Ok(()), "{candidate:?}");
                    }
                }
                state.step(&step).unwrap();
                positions += 1;
            }
        }
    }
    eprintln!(
        "{} crash-corpus journals, {crashed} with a crash, {positions} model positions, refused {refused:?}",
        corpus.len()
    );
    assert!(crashed > 1_000, "{crashed}");
    assert!(positions > 1_000, "{positions}");
    // Refusals are commands to a crashed task and crashes of a closed region only.
    assert!(refused.keys().any(|k| k.ends_with("task-crashed")));
    assert!(refused.keys().any(|k| k.ends_with("crash refused")));
}

/// A named perturbation of a real crash journal, and the model faults that count as
/// rejecting it for its own reason.
type Perturbation = (
    &'static str,
    fn(&Journal) -> Option<Journal>,
    fn(&Fault) -> bool,
);

fn crash_at(j: &Journal) -> Option<usize> {
    bodies(j).iter().position(|b| {
        matches!(
            b,
            EventBody::Lifecycle(LifecycleEvent::RegionCrashed { .. })
        )
    })
}

fn is_fence(b: &EventBody) -> bool {
    matches!(
        b,
        EventBody::Effect(EffectEvent::Fenced { .. })
            | EventBody::Obligation(ObligationEvent::Fenced { .. })
            | EventBody::Time(TimeEvent::Fenced { .. })
    )
}

fn perturbations() -> Vec<Perturbation> {
    vec![
        (
            "a fence is dropped",
            |j| {
                let mut b = bodies(j);
                let at = b.iter().position(is_fence)?;
                b.remove(at);
                Some(rebuild(b))
            },
            |f| matches!(f, Fault::FenceOwed { .. }),
        ),
        (
            "an event comes before the last fence",
            |j| {
                let mut b = bodies(j);
                let last = b.iter().rposition(is_fence)?;
                let next = b.get(last + 1)?.clone();
                b.remove(last + 1);
                b.insert(last, next);
                Some(rebuild(b))
            },
            |f| matches!(f, Fault::FenceOwed { .. }),
        ),
        (
            "two fences are swapped",
            |j| {
                let mut b = bodies(j);
                let first = b.iter().position(is_fence)?;
                if !b.get(first + 1).is_some_and(is_fence) {
                    return None;
                }
                b.swap(first, first + 1);
                Some(rebuild(b))
            },
            |f| matches!(f, Fault::FenceOwed { .. }),
        ),
        (
            "the last fence is repeated",
            |j| {
                let mut b = bodies(j);
                let at = b.iter().rposition(is_fence)?;
                let fence = b[at].clone();
                b.insert(at + 1, fence);
                Some(rebuild(b))
            },
            |f| matches!(f, Fault::FenceUnowed { .. }),
        ),
        (
            "the stopped set leaves out a live task",
            |j| {
                let mut b = bodies(j);
                let at = crash_at(j)?;
                let EventBody::Lifecycle(LifecycleEvent::RegionCrashed { region, fenced }) =
                    b[at].clone()
                else {
                    return None;
                };
                let fewer: Vec<TaskOrdinal> = fenced.as_slice().iter().skip(1).copied().collect();
                b[at] = EventBody::Lifecycle(LifecycleEvent::RegionCrashed {
                    region,
                    fenced: TaskSet::new(fewer),
                });
                Some(rebuild(b))
            },
            |f| matches!(f, Fault::CrashSet { .. }),
        ),
        (
            "the stopped set names a task outside the subtree",
            |j| {
                let mut b = bodies(j);
                let at = crash_at(j)?;
                let EventBody::Lifecycle(LifecycleEvent::RegionCrashed { region, fenced }) =
                    b[at].clone()
                else {
                    return None;
                };
                let outside = (0..8)
                    .map(TaskOrdinal)
                    .find(|t| !fenced.as_slice().contains(t))?;
                let mut more = fenced.as_slice().to_vec();
                more.push(outside);
                b[at] = EventBody::Lifecycle(LifecycleEvent::RegionCrashed {
                    region,
                    fenced: TaskSet::new(more),
                });
                Some(rebuild(b))
            },
            |f| matches!(f, Fault::CrashSet { .. }),
        ),
        (
            "a crashed region crashes again",
            |j| {
                let mut b = bodies(j);
                let at = crash_at(j)?;
                let EventBody::Lifecycle(LifecycleEvent::RegionCrashed { region, .. }) =
                    b[at].clone()
                else {
                    return None;
                };
                let last = b.iter().rposition(is_fence).unwrap_or(at);
                b.insert(
                    last + 1,
                    EventBody::Lifecycle(LifecycleEvent::RegionCrashed {
                        region,
                        fenced: TaskSet::default(),
                    }),
                );
                Some(rebuild(b))
            },
            |f| matches!(f, Fault::RegionPhase(_)),
        ),
        (
            "a stopped task resumes after the crash",
            |j| {
                let mut b = bodies(j);
                let at = crash_at(j)?;
                let EventBody::Lifecycle(LifecycleEvent::RegionCrashed { fenced, .. }) =
                    b[at].clone()
                else {
                    return None;
                };
                let task = *fenced.as_slice().first()?;
                let last = b.iter().rposition(is_fence).unwrap_or(at);
                b.insert(
                    last + 1,
                    EventBody::Lifecycle(LifecycleEvent::TaskStepped {
                        task,
                        step: TaskStep::Resume,
                    }),
                );
                Some(rebuild(b))
            },
            |f| matches!(f, Fault::TaskPhase(_)),
        ),
        (
            "a fenced reservation is aborted for cancel instead",
            |j| {
                let mut b = bodies(j);
                let at = b
                    .iter()
                    .position(|e| matches!(e, EventBody::Effect(EffectEvent::Fenced { .. })))?;
                let EventBody::Effect(EffectEvent::Fenced { reservation }) = b[at] else {
                    return None;
                };
                b[at] = EventBody::Effect(EffectEvent::Aborted {
                    reservation,
                    cause: AbortCause::Cancel,
                });
                Some(rebuild(b))
            },
            |f| matches!(f, Fault::FenceOwed { .. }),
        ),
        (
            "a fenced obligation is leaked instead",
            |j| {
                let mut b = bodies(j);
                let at = b.iter().position(|e| {
                    matches!(e, EventBody::Obligation(ObligationEvent::Fenced { .. }))
                })?;
                let EventBody::Obligation(ObligationEvent::Fenced { obligation }) = b[at] else {
                    return None;
                };
                b[at] = EventBody::Obligation(ObligationEvent::Leaked { obligation });
                Some(rebuild(b))
            },
            |f| matches!(f, Fault::FenceOwed { .. }),
        ),
        (
            "a fenced obligation is committed after its fence",
            |j| {
                let mut b = bodies(j);
                let last = b.iter().rposition(is_fence)?;
                let obligation = b.iter().find_map(|e| match e {
                    EventBody::Obligation(ObligationEvent::Fenced { obligation }) => {
                        Some(*obligation)
                    }
                    _ => None,
                })?;
                b.insert(
                    last + 1,
                    EventBody::Obligation(ObligationEvent::Discharged {
                        obligation,
                        how: continuum_asupersync::family::obligation::Discharge::Committed,
                    }),
                );
                Some(rebuild(b))
            },
            |f| matches!(f, Fault::ObligationPhase(_)),
        ),
        (
            "a fenced timer fires",
            |j| {
                let mut b = bodies(j);
                let timer = b.iter().find_map(|e| match e {
                    EventBody::Time(TimeEvent::Fenced { timer }) => Some(*timer),
                    _ => None,
                })?;
                let advanced = b
                    .iter()
                    .position(|e| matches!(e, EventBody::Time(TimeEvent::Advanced { .. })))?;
                let EventBody::Time(TimeEvent::Advanced { to, .. }) = b[advanced] else {
                    return None;
                };
                b.insert(
                    advanced + 1,
                    EventBody::Time(TimeEvent::Fired { timer, at: to }),
                );
                Some(rebuild(b))
            },
            |f| matches!(f, Fault::TimerPhase(_)),
        ),
        (
            "a settle hides its fenced obligations",
            |j| {
                let mut b = bodies(j);
                let at = b.iter().position(|e| {
                    matches!(e, EventBody::Obligation(ObligationEvent::RegionSettled { fenced, .. }) if !fenced.is_empty())
                })?;
                let EventBody::Obligation(ObligationEvent::RegionSettled {
                    region,
                    open,
                    leaked,
                    ..
                }) = b[at].clone()
                else {
                    return None;
                };
                b[at] = EventBody::Obligation(ObligationEvent::RegionSettled {
                    region,
                    open,
                    leaked,
                    fenced: ObligationSet::default(),
                });
                Some(rebuild(b))
            },
            |f| matches!(f, Fault::Settle(_)),
        ),
        (
            "a settle reports a fenced obligation as leaked",
            |j| {
                let mut b = bodies(j);
                let at = b.iter().position(|e| {
                    matches!(e, EventBody::Obligation(ObligationEvent::RegionSettled { fenced, .. }) if !fenced.is_empty())
                })?;
                let EventBody::Obligation(ObligationEvent::RegionSettled {
                    region,
                    open,
                    fenced,
                    ..
                }) = b[at].clone()
                else {
                    return None;
                };
                b[at] = EventBody::Obligation(ObligationEvent::RegionSettled {
                    region,
                    open,
                    leaked: fenced,
                    fenced: ObligationSet::default(),
                });
                Some(rebuild(b))
            },
            |f| matches!(f, Fault::Settle(_)),
        ),
        (
            "the crash moves before a stopped task's reserve",
            |j| {
                let mut b = bodies(j);
                let at = crash_at(j)?;
                let EventBody::Lifecycle(LifecycleEvent::RegionCrashed { fenced, .. }) =
                    b[at].clone()
                else {
                    return None;
                };
                let reserve = b[..at].iter().rposition(|e| {
                    matches!(e, EventBody::Effect(EffectEvent::Reserved { task, .. }) if fenced.as_slice().contains(task))
                })?;
                let event = b.remove(at);
                b.insert(reserve, event);
                Some(rebuild(b))
            },
            |_| true,
        ),
    ]
}

#[test]
fn perturbed_crash_journals_are_rejected_by_both() {
    let config = all_families();
    let (corpus, _) = corpus();
    let bases: Vec<&Journal> = corpus
        .iter()
        .filter(|e| e.config == config && has_crash(&e.journal))
        .map(|e| &e.journal)
        .step_by(7)
        .collect();
    assert!(bases.len() > 5, "{}", bases.len());
    let (programs, log) = one_actor(crash_program(true));
    let sleeper = run(&programs, &log, &config).unwrap();
    let mut rejected = BTreeMap::<&str, usize>::new();
    for (name, perturb, expected) in perturbations() {
        for base in bases.iter().copied().chain([&sleeper]) {
            let Some(mutant) = perturb(base) else {
                continue;
            };
            match judge(&config, &mutant) {
                Verdict::Rejected { fault, .. } => {
                    assert!(
                        expected(&fault),
                        "{name}: rejected for {fault:?}\n{}",
                        mutant.render()
                    );
                }
                other => panic!(
                    "{name}: the model does not reject it: {other}\n{}",
                    mutant.render()
                ),
            }
            assert!(
                !lift_accepts(&mutant),
                "{name}: the lift conforms\n{}",
                mutant.render()
            );
            *rejected.entry(name).or_default() += 1;
        }
    }
    eprintln!("{rejected:?}");
    for (name, ..) in perturbations() {
        assert!(
            rejected.get(name).copied().unwrap_or(0) > 0,
            "{name}: never applied"
        );
    }
}

#[test]
fn deletions_and_swaps_of_crash_journals_get_one_verdict() {
    let (corpus, _) = corpus();
    let mut compared = 0_usize;
    let mut rejected_by_both = 0_usize;
    let mut unsupported = 0_usize;
    let mut disagreements = Vec::new();
    for entry in corpus.iter().filter(|e| has_crash(&e.journal)).step_by(11) {
        let b = bodies(&entry.journal);
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
            // An unsupported crash ends judgement in both, at the same event (RFC 0026
            // correction 61).
            let model_unsupported = match verdict {
                Verdict::Unsupported { at, .. } => Some(at as u64),
                _ => None,
            };
            let lift_unsupported = match lift(&variant) {
                LiftVerdict::Inconclusive {
                    seq,
                    reason: InconclusiveReason::Unsupported,
                    ..
                } => Some(seq),
                _ => None,
            };
            if model_unsupported != lift_unsupported {
                disagreements.push(format!(
                    "{} log {} {what}: model unsupported at {model_unsupported:?}, lift at {lift_unsupported:?}",
                    entry.name, entry.log
                ));
            }
            unsupported += usize::from(lift_unsupported.is_some());
            match (by_model, by_lift) {
                (false, false) => rejected_by_both += 1,
                (true, true) => {}
                _ => disagreements.push(format!(
                    "{} log {} {what}: model {:?}, lift {:?}",
                    entry.name,
                    entry.log,
                    judge(&entry.config, &variant),
                    lift(&variant)
                )),
            }
        }
    }
    eprintln!(
        "compared {compared}, rejected by both {rejected_by_both}, unsupported crash {unsupported}"
    );
    assert!(
        disagreements.is_empty(),
        "{} disagreements:\n{}",
        disagreements.len(),
        disagreements
            .iter()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(compared > 5_000, "{compared}");
    // No deletion or swap of a binding journal makes a crash unsupported: the typed
    // agreement above is exercised by the spliced journals of
    // `an_unsupported_crash_in_a_real_journal_ends_judgement_in_both`.
    assert_eq!(unsupported, 0);
    assert!(
        rejected_by_both > compared / 3,
        "{rejected_by_both} of {compared}"
    );
}

#[test]
fn a_journal_cut_inside_a_crash_does_not_conform() {
    let (corpus, _) = corpus();
    let mut inside = 0_usize;
    let mut agreed = 0_usize;
    for entry in corpus.iter().filter(|e| has_crash(&e.journal)).step_by(5) {
        let all = bodies(&entry.journal);
        let at = crash_at(&entry.journal).unwrap();
        let EventBody::Lifecycle(LifecycleEvent::RegionCrashed { region, .. }) = all[at] else {
            unreachable!()
        };
        // The crash is complete once its region's finalize is journaled.
        let done = all
            .iter()
            .position(|b| {
                matches!(b, EventBody::Lifecycle(LifecycleEvent::RegionFinalized { region: r }) if *r == region)
            })
            .unwrap();
        for cut in 0..all.len() {
            let prefix = rebuild(all[..cut].iter().cloned());
            let conforms = lift_accepts(&prefix);
            if cut > at && cut <= done {
                assert!(
                    !conforms,
                    "{} log {} cut {cut} is inside the crash and conforms\n{}",
                    entry.name,
                    entry.log,
                    prefix.render()
                );
                assert!(
                    matches!(
                        lift(&prefix),
                        LiftVerdict::Inconclusive {
                            reason: InconclusiveReason::InsufficientTelemetry,
                            ..
                        }
                    ),
                    "{} cut {cut}: {:?}",
                    entry.name,
                    lift(&prefix)
                );
                inside += 1;
            }
            // The model judges the prefix over the families the full journal's
            // projection observes.
            if judge(&entry.config, &prefix).is_accepted() == conforms {
                agreed += 1;
            } else {
                // A prefix that drops a family's only events reads as a narrower
                // projection to the lift; the model keeps the entry's alphabet. Only a
                // settle-less finalize can part them, and it is typed Inconclusive.
                assert!(
                    matches!(
                        lift(&prefix),
                        LiftVerdict::Inconclusive { .. } | LiftVerdict::Conforms(_)
                    ),
                    "{} cut {cut}",
                    entry.name
                );
            }
        }
    }
    eprintln!("{inside} cuts inside a crash, {agreed} prefixes with one verdict");
    assert!(inside > 100, "{inside}");
}

#[test]
fn the_scripted_source_reports_the_same_crash() {
    let (programs, log) = one_actor(crash_program(false));
    let journal = run(&programs, &log, &BindingConfig::new(0)).unwrap();
    let crashed = bodies(&journal)
        .into_iter()
        .find(|b| {
            matches!(
                b,
                EventBody::Lifecycle(LifecycleEvent::RegionCrashed { .. })
            )
        })
        .unwrap();
    // The recorder's own account of the same program's lifecycle, up to the crash.
    let step = |task, step| Report::Lifecycle(LifecycleReport::Step { task, step });
    let script = vec![
        Report::Lifecycle(LifecycleReport::OpenRegion {
            parent: ROOT,
            child: R[1],
        }),
        Report::Lifecycle(LifecycleReport::OpenRegion {
            parent: R[1],
            child: R[2],
        }),
        Report::Lifecycle(LifecycleReport::Spawn {
            region: R[1],
            task: T[1],
            resumability: Resumability::Resumable,
        }),
        Report::Lifecycle(LifecycleReport::Spawn {
            region: R[2],
            task: T[2],
            resumability: Resumability::Resumable,
        }),
        Report::Lifecycle(LifecycleReport::Spawn {
            region: ROOT,
            task: T[3],
            resumability: Resumability::Resumable,
        }),
        Report::Lifecycle(LifecycleReport::Spawn {
            region: R[1],
            task: T[4],
            resumability: Resumability::Resumable,
        }),
        step(T[1], TaskStep::Begin),
        step(T[2], TaskStep::Begin),
        step(T[3], TaskStep::Begin),
        Report::Lifecycle(LifecycleReport::Crash { region: R[1] }),
    ];
    let recorded = record(
        std::slice::from_ref(&script),
        &ChoiceLog::new(vec![0; script.len()]),
    )
    .unwrap();
    assert_eq!(bodies(&recorded).last(), Some(&crashed));
    // A recorded crash lifts: the stopped tasks fail in the calculus, and nothing is
    // owed, because this journal carries no family that holds anything.
    let lifted = lift(&recorded);
    assert!(
        matches!(lifted, LiftVerdict::Inconclusive { .. }),
        "{lifted:?}: the subtree is not finalized yet"
    );
}

/// Hand-built journals from the pre-review pass (bn-20d8u): a crash may not fence a timer
/// that was already due, and may not crash a subtree whose subregion is draining under
/// cancellation. The lift and the model reject both; without the offending step they
/// conform or are rejected for the old reason.
#[test]
fn a_crash_does_not_launder_a_late_timer_or_a_cancellation() {
    use continuum_asupersync::family::time::VirtualInstant;
    let lc = |e| EventBody::Lifecycle(e);
    let t0 = TaskOrdinal(0);
    let (r0, r1, r2) = (RegionOrdinal(0), RegionOrdinal(1), RegionOrdinal(2));
    let step = |step| lc(LifecycleEvent::TaskStepped { task: t0, step });
    let late = vec![
        lc(LifecycleEvent::RegionOpened {
            region: r1,
            parent: r0,
        }),
        lc(LifecycleEvent::TaskSpawned {
            task: t0,
            region: r1,
            resumability: Resumability::Resumable,
        }),
        step(TaskStep::Begin),
        EventBody::Time(TimeEvent::Scheduled {
            timer: TimerOrdinal(0),
            task: t0,
            at: VirtualInstant(0),
            deadline: VirtualInstant(10),
        }),
        step(TaskStep::Suspend),
        EventBody::Time(TimeEvent::Advanced {
            from: VirtualInstant(0),
            to: VirtualInstant(20),
        }),
        lc(LifecycleEvent::RegionCrashed {
            region: r1,
            fenced: TaskSet::new([t0]),
        }),
        EventBody::Time(TimeEvent::Fenced {
            timer: TimerOrdinal(0),
        }),
        lc(LifecycleEvent::RegionDrained {
            region: r1,
            cancelled: TaskSet::default(),
        }),
        lc(LifecycleEvent::RegionFinalized { region: r1 }),
    ];
    let config = BindingConfig::new(0).observing(Family::Time);
    let journal = rebuild(late.clone());
    assert!(
        matches!(lift(&journal), LiftVerdict::Violates { seq: 6, .. }),
        "{:?}",
        lift(&journal)
    );
    assert!(matches!(
        judge(&config, &journal),
        Verdict::Rejected {
            at: 6,
            fault: Fault::LateTimer(0)
        }
    ));
    // Cut right after the crash: still the violation, never truncation.
    let cut = rebuild(late[..7].iter().cloned());
    assert!(
        matches!(lift(&cut), LiftVerdict::Violates { .. }),
        "{:?}",
        lift(&cut)
    );

    let cancelled = vec![
        lc(LifecycleEvent::RegionOpened {
            region: r1,
            parent: r0,
        }),
        lc(LifecycleEvent::RegionOpened {
            region: r2,
            parent: r1,
        }),
        lc(LifecycleEvent::TaskSpawned {
            task: t0,
            region: r2,
            resumability: Resumability::Resumable,
        }),
        step(TaskStep::Begin),
        step(TaskStep::Complete),
        lc(LifecycleEvent::RegionCancelRequested { region: r2 }),
        lc(LifecycleEvent::RegionCrashed {
            region: r1,
            fenced: TaskSet::default(),
        }),
        lc(LifecycleEvent::RegionDrained {
            region: r2,
            cancelled: TaskSet::default(),
        }),
        lc(LifecycleEvent::RegionFinalized { region: r2 }),
        lc(LifecycleEvent::RegionDrained {
            region: r1,
            cancelled: TaskSet::default(),
        }),
        lc(LifecycleEvent::RegionFinalized { region: r1 }),
    ];
    let journal = rebuild(cancelled.clone());
    assert!(
        matches!(lift(&journal), LiftVerdict::Violates { seq: 6, .. }),
        "{:?}",
        lift(&journal)
    );
    assert!(matches!(
        judge(&BindingConfig::new(0), &journal),
        Verdict::Rejected { at: 6, .. }
    ));
    // Without the crash, the subregion's cancellation drains and finalizes, and r1 is
    // still open: the rest conforms.
    let mut control = cancelled;
    control.remove(6);
    control.truncate(8);
    let journal = rebuild(control);
    assert!(lift_accepts(&journal), "{:?}", lift(&journal));
    assert!(judge(&BindingConfig::new(0), &journal).is_accepted());
}

// --- an unsupported crash ends judgement (RFC 0026 correction 61, bn-1id0n) ---------

/// The lift's verdict is `Inconclusive(Unsupported)` at `seq`, of `family`.
fn unsupported_at(verdict: &LiftVerdict, seq: u64, family: Family) -> bool {
    matches!(
        verdict,
        LiftVerdict::Inconclusive {
            seq: s,
            family: f,
            reason: InconclusiveReason::Unsupported,
        } if *s == seq && *f == family
    )
}

/// `r1` holds `t0`; `t1` in the root. With `deadline`, `t0` runs under a budget deadline,
/// which the crash of `r1` at the returned position has no fail-stop semantics for.
/// Then `r1` drains and finalizes, and `t1` completes.
fn deadline_crash(deadline: bool) -> (Vec<EventBody>, usize) {
    use continuum_asupersync::family::time::VirtualInstant;
    let lc = |e| EventBody::Lifecycle(e);
    let (t0, t1) = (TaskOrdinal(0), TaskOrdinal(1));
    let (r0, r1) = (RegionOrdinal(0), RegionOrdinal(1));
    let mut events = vec![
        lc(LifecycleEvent::RegionOpened {
            region: r1,
            parent: r0,
        }),
        lc(LifecycleEvent::TaskSpawned {
            task: t0,
            region: r1,
            resumability: Resumability::Resumable,
        }),
    ];
    if deadline {
        events.push(EventBody::Time(TimeEvent::Deadline {
            task: t0,
            at: VirtualInstant(10),
        }));
    }
    events.extend([
        lc(LifecycleEvent::TaskStepped {
            task: t0,
            step: TaskStep::Begin,
        }),
        lc(LifecycleEvent::TaskSpawned {
            task: t1,
            region: r0,
            resumability: Resumability::Resumable,
        }),
        lc(LifecycleEvent::TaskStepped {
            task: t1,
            step: TaskStep::Begin,
        }),
    ]);
    let crash = events.len();
    events.extend([
        lc(LifecycleEvent::RegionCrashed {
            region: r1,
            fenced: TaskSet::new([t0]),
        }),
        lc(LifecycleEvent::RegionDrained {
            region: r1,
            cancelled: TaskSet::default(),
        }),
        lc(LifecycleEvent::RegionFinalized { region: r1 }),
        lc(LifecycleEvent::TaskStepped {
            task: t1,
            step: TaskStep::Complete,
        }),
    ]);
    (events, crash)
}

/// RFC 0026 correction 61: a crash the lift gives no fail-stop semantics ends judgement
/// at its own event. The lift reports `Inconclusive(Unsupported)` there, and the A7
/// model `Unsupported` at the same position, whatever follows: (a) a genuine violation
/// that holds under any effect of the crash, (b) no violation, or (c) events whose
/// legality turns on the crash's effect. It is never a conformance and never a later
/// violation. A violation at the crash event itself, or before it, outranks it, and (d)
/// a crash of a task in a cancellation is such a violation, never unsupported.
#[test]
fn an_unsupported_crash_ends_judgement_at_its_event() {
    let time = BindingConfig::new(0).observing(Family::Time);

    // (b) No violation after the crash. Expected: Inconclusive(Unsupported, Time) at the
    // crash, never Conforms. Control: without the deadline the same journal conforms,
    // so the crash's unsupported case alone stops judgement.
    let (clean, crash) = deadline_crash(true);
    let journal = rebuild(clean.clone());
    assert!(
        unsupported_at(&lift(&journal), crash as u64, Family::Time),
        "{:?}",
        lift(&journal)
    );
    assert_eq!(
        judge(&time, &journal),
        Verdict::Unsupported { at: crash, task: 0 }
    );
    let (control, _) = deadline_crash(false);
    let control = rebuild(control);
    assert!(lift_accepts(&control), "{:?}", lift(&control));
    assert!(judge(&time, &control).is_accepted());

    // (a) A genuine later violation: `t1`, outside the crashed subtree, completes twice.
    // It is a violation under any effect of the crash, and still not reported: the
    // verdict is the crash's Inconclusive(Unsupported), never Violates and never
    // Conforms (correction 61 item 4: ending costs precision, not soundness). Control:
    // without the deadline the same step is the violation, at its own event.
    let complete_t1 = EventBody::Lifecycle(LifecycleEvent::TaskStepped {
        task: TaskOrdinal(1),
        step: TaskStep::Complete,
    });
    let mut violating = clean;
    violating.push(complete_t1.clone());
    let journal = rebuild(violating.clone());
    assert!(
        unsupported_at(&lift(&journal), crash as u64, Family::Time),
        "{:?}",
        lift(&journal)
    );
    assert_eq!(
        judge(&time, &journal),
        Verdict::Unsupported { at: crash, task: 0 }
    );
    let (mut control, _) = deadline_crash(false);
    control.push(complete_t1);
    let at = control.len() - 1;
    let control = rebuild(control);
    assert!(
        matches!(lift(&control), LiftVerdict::Violates { seq, .. } if seq == at as u64),
        "{:?}",
        lift(&control)
    );
    assert!(
        matches!(judge(&time, &control), Verdict::Rejected { at: a, .. } if a == at),
        "{:?}",
        judge(&time, &control)
    );

    // A violation at the crash event itself outranks its unsupported case: a stopped set
    // that misses `t0` is CrashOutcome, checked before the deadline.
    let mut misreported = violating;
    misreported[crash] = EventBody::Lifecycle(LifecycleEvent::RegionCrashed {
        region: RegionOrdinal(1),
        fenced: TaskSet::default(),
    });
    let journal = rebuild(misreported);
    assert!(
        matches!(
            lift(&journal),
            LiftVerdict::Violates {
                seq,
                reason: Nonconformance::CrashOutcome { .. },
            } if seq == crash as u64
        ),
        "{:?}",
        lift(&journal)
    );
    assert!(
        matches!(
            judge(&time, &journal),
            Verdict::Rejected { at, fault: Fault::CrashSet { .. } } if at == crash
        ),
        "{:?}",
        judge(&time, &journal)
    );

    // (c) Events whose legality turns on the crash's effect. `t0` in `r1` holds channel
    // 0's receiver, and `t1` in the root sends on it. The binding refuses this crash, so
    // no journal spells its effect. If the receiver goes with the crash, the channel
    // closes: `receiver-gone`, then `t1`'s `send-closed`, is the run, and a send that is
    // queued is not. If it outlives the crash in the withdrawn future (correction 58 item
    // 1), a queued send is the run, and a `send-closed` with no `receiver-gone` is
    // illegal. The lift judges neither: each suffix is Inconclusive(Unsupported,
    // Channel) at the crash, and the model's Unsupported at the same position.
    let chan = BindingConfig::new(0).observing(Family::Channel);
    let lc = |e| EventBody::Lifecycle(e);
    let (t0, t1) = (TaskOrdinal(0), TaskOrdinal(1));
    let (c0, m0) = (ChannelOrdinal(0), MessageOrdinal(0));
    let prefix = vec![
        lc(LifecycleEvent::RegionOpened {
            region: RegionOrdinal(1),
            parent: RegionOrdinal(0),
        }),
        lc(LifecycleEvent::TaskSpawned {
            task: t0,
            region: RegionOrdinal(1),
            resumability: Resumability::Resumable,
        }),
        lc(LifecycleEvent::TaskStepped {
            task: t0,
            step: TaskStep::Begin,
        }),
        lc(LifecycleEvent::TaskSpawned {
            task: t1,
            region: RegionOrdinal(0),
            resumability: Resumability::Resumable,
        }),
        lc(LifecycleEvent::TaskStepped {
            task: t1,
            step: TaskStep::Begin,
        }),
        EventBody::Channel(ChannelEvent::Opened {
            channel: c0,
            capacity: 1,
            receiver: t0,
        }),
    ];
    // The prefix before the crash conforms: nothing before it stops judgement.
    let before = rebuild(prefix.clone());
    assert!(lift_accepts(&before), "{:?}", lift(&before));
    assert!(judge(&chan, &before).is_accepted());
    let crash = prefix.len();
    let teardown = [
        lc(LifecycleEvent::RegionCrashed {
            region: RegionOrdinal(1),
            fenced: TaskSet::new([t0]),
        }),
        lc(LifecycleEvent::RegionDrained {
            region: RegionOrdinal(1),
            cancelled: TaskSet::default(),
        }),
        lc(LifecycleEvent::RegionFinalized {
            region: RegionOrdinal(1),
        }),
    ];
    let receiver_dropped = [
        EventBody::Channel(ChannelEvent::ReceiverGone {
            channel: c0,
            discarded: MessageSet::new([]),
        }),
        EventBody::Channel(ChannelEvent::SendClosed {
            channel: c0,
            message: m0,
            sender: t1,
        }),
    ];
    let receiver_kept = [EventBody::Channel(ChannelEvent::Sent {
        channel: c0,
        message: m0,
        sender: t1,
    })];
    let closed_unannounced = [EventBody::Channel(ChannelEvent::SendClosed {
        channel: c0,
        message: m0,
        sender: t1,
    })];
    for (name, suffix) in [
        ("receiver dropped", &receiver_dropped[..]),
        ("receiver kept", &receiver_kept[..]),
        ("closed, unannounced", &closed_unannounced[..]),
        ("nothing after the teardown", &[][..]),
    ] {
        let journal = rebuild(
            prefix
                .iter()
                .chain(&teardown)
                .chain(suffix)
                .cloned()
                .collect::<Vec<_>>(),
        );
        assert!(
            unsupported_at(&lift(&journal), crash as u64, Family::Channel),
            "{name}: {:?}",
            lift(&journal)
        );
        assert_eq!(
            judge(&chan, &journal),
            Verdict::Unsupported { at: crash, task: 0 },
            "{name}"
        );
    }
    // Control for correction 61 item 3: a receiving task that ends with its receiver
    // still there, and no `receiver-gone`, is a violation under the lift's channel
    // rules. A continuation from a supported crash, which ends `t0` the same way, would
    // report it for the "receiver kept" reading, which is a run.
    let ended = rebuild(
        prefix
            .iter()
            .cloned()
            .chain([lc(LifecycleEvent::TaskStepped {
                task: t0,
                step: TaskStep::Complete,
            })])
            .chain(receiver_kept.iter().cloned())
            .collect::<Vec<_>>(),
    );
    assert!(
        matches!(
            lift(&ended),
            LiftVerdict::Violates {
                seq: 7,
                reason: Nonconformance::Channel(
                    continuum_asupersync::family::channel::ChannelFault::ReceiverOutlivesTask {
                        channel: 0
                    }
                ),
            }
        ),
        "{:?}",
        lift(&ended)
    );
    assert!(
        matches!(
            judge(&chan, &ended),
            Verdict::Rejected {
                at: 6,
                fault: Fault::ChannelOutlivesTask { task: 0 }
            }
        ),
        "{:?}",
        judge(&chan, &ended)
    );

    // (d) The cancellation case of correction 58 item 3 is not reached as unsupported.
    // A crash inside a task's own cancellation interrupts it, and a crash of a region
    // under cancellation finds it cancelling. Both are violations at the crash event,
    // found by checks that run before the unsupported one (correction 61 item 1).
    let own = vec![
        lc(LifecycleEvent::RegionOpened {
            region: RegionOrdinal(1),
            parent: RegionOrdinal(0),
        }),
        lc(LifecycleEvent::TaskSpawned {
            task: t0,
            region: RegionOrdinal(1),
            resumability: Resumability::Resumable,
        }),
        lc(LifecycleEvent::TaskStepped {
            task: t0,
            step: TaskStep::Begin,
        }),
        lc(LifecycleEvent::TaskStepped {
            task: t0,
            step: TaskStep::CancelRequested,
        }),
        lc(LifecycleEvent::RegionCrashed {
            region: RegionOrdinal(1),
            fenced: TaskSet::new([t0]),
        }),
    ];
    // Lifecycle only: with time observed, a task's own request is its deadline's and
    // needs a declared deadline first.
    let lifecycle = BindingConfig::new(0);
    {
        let config = &lifecycle;
        let journal = rebuild(own.clone());
        assert!(
            matches!(
                lift(&journal),
                LiftVerdict::Violates {
                    seq: 4,
                    reason: Nonconformance::Cancellation(
                        continuum_asupersync::family::cancellation::CancellationFault::InterruptedOwnCancel { .. }
                    ),
                }
            ),
            "{:?}",
            lift(&journal)
        );
        assert!(
            matches!(
                judge(config, &journal),
                Verdict::Rejected {
                    at: 4,
                    fault: Fault::OwnCancellationInterrupted(0)
                }
            ),
            "{:?}",
            judge(config, &journal)
        );
    }
    let mut region = own;
    region[3] = lc(LifecycleEvent::RegionCancelRequested {
        region: RegionOrdinal(1),
    });
    let journal = rebuild(region);
    assert!(
        matches!(
            lift(&journal),
            LiftVerdict::Violates {
                seq: 4,
                reason: Nonconformance::CrashedRegionState { region: 1, .. },
            }
        ),
        "{:?}",
        lift(&journal)
    );
    assert!(
        matches!(
            judge(&lifecycle, &journal),
            Verdict::Rejected {
                at: 4,
                fault: Fault::RegionPhase(1)
            }
        ),
        "{:?}",
        judge(&lifecycle, &journal)
    );
}

/// A real crash journal with a far budget deadline declared for one task the crash
/// stops, right after that task's spawn: the same run, except that the crash has no
/// fail-stop semantics. `None` if the crash stops no task.
fn with_deadline_on_a_stopped_task(journal: &Journal) -> Option<(Journal, usize)> {
    use continuum_asupersync::family::time::VirtualInstant;
    let mut b = bodies(journal);
    let at = crash_at(journal)?;
    let EventBody::Lifecycle(LifecycleEvent::RegionCrashed { fenced, .. }) = &b[at] else {
        unreachable!()
    };
    let task = *fenced.as_slice().last()?;
    let spawn = b.iter().position(|e| {
        matches!(e, EventBody::Lifecycle(LifecycleEvent::TaskSpawned { task: t, .. }) if *t == task)
    })?;
    b.insert(
        spawn + 1,
        EventBody::Time(TimeEvent::Deadline {
            task,
            at: VirtualInstant(1_000_000),
        }),
    );
    Some((rebuild(b), at + 1))
}

/// RFC 0026 correction 61 over real journals: each crash journal of the corpus that
/// observes time, with a far deadline declared for one stopped task, is
/// `Inconclusive(Unsupported, Time)` at the crash in the lift and `Unsupported` at the
/// same position in the A7 model; everything after the crash (its fences, drains,
/// finalizes and settles, and the rest of the run) goes unjudged. Every single-event
/// deletion and adjacent swap of a sample of them gets one verdict from both, and one
/// typed unsupported position.
#[test]
fn an_unsupported_crash_in_a_real_journal_ends_judgement_in_both() {
    let (corpus, _) = corpus();
    let mut spliced = 0_usize;
    let mut skipped = 0_usize;
    let mut compared = 0_usize;
    let mut unsupported = 0_usize;
    let mut disagreements = Vec::new();
    for entry in corpus
        .iter()
        .filter(|e| e.config.families.contains(Family::Time) && has_crash(&e.journal))
    {
        let Some((journal, crash)) = with_deadline_on_a_stopped_task(&entry.journal) else {
            skipped += 1;
            continue;
        };
        spliced += 1;
        assert!(
            unsupported_at(&lift(&journal), crash as u64, Family::Time),
            "{} log {}: {:?}",
            entry.name,
            entry.log,
            lift(&journal)
        );
        assert!(
            matches!(judge(&entry.config, &journal), Verdict::Unsupported { at, .. } if at == crash),
            "{} log {}: {:?}",
            entry.name,
            entry.log,
            judge(&entry.config, &journal)
        );
        if spliced % 13 != 0 {
            continue;
        }
        let b = bodies(&journal);
        for i in 0..b.len() {
            let mut variants = vec![{
                let mut deleted = b.clone();
                deleted.remove(i);
                (format!("delete {i}"), deleted)
            }];
            if i + 1 < b.len() && b[i] != b[i + 1] {
                let mut swapped = b.clone();
                swapped.swap(i, i + 1);
                variants.push((format!("swap {i}"), swapped));
            }
            for (what, variant) in variants {
                let variant = rebuild(variant);
                let by_model = judge(&entry.config, &variant);
                let by_lift = lift(&variant);
                let model_unsupported = match by_model {
                    Verdict::Unsupported { at, .. } => Some(at as u64),
                    _ => None,
                };
                let lift_unsupported = match by_lift {
                    LiftVerdict::Inconclusive {
                        seq,
                        reason: InconclusiveReason::Unsupported,
                        ..
                    } => Some(seq),
                    _ => None,
                };
                compared += 1;
                unsupported += usize::from(lift_unsupported.is_some());
                if by_model.is_accepted() != matches!(by_lift, LiftVerdict::Conforms(_))
                    || model_unsupported != lift_unsupported
                {
                    disagreements.push(format!(
                        "{} log {} {what}: model {by_model:?}, lift {by_lift:?}",
                        entry.name, entry.log
                    ));
                }
            }
        }
    }
    eprintln!(
        "{spliced} spliced, {skipped} with no stopped task, {compared} variants compared, {unsupported} unsupported"
    );
    assert!(
        disagreements.is_empty(),
        "{} disagreements:\n{}",
        disagreements.len(),
        disagreements
            .iter()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    // RFC 0026 correction 61's evidence cites these counts.
    assert_eq!(
        (spliced, skipped, compared, unsupported),
        (976, 0, 5_939, 2_378)
    );
}

// --- the calculus crash step against the lift (RFC 0026 correction 59, bn-fxxf2) ------

use continuum_asupersync::family::obligation::Discharge;
use continuum_asupersync::lift::Nonconformance;
use continuum_task::region::obligation::{
    SubstrateId, SubstrateKind, SubstrateObligation, SubstrateOutcome,
};
use continuum_task::region::worker::{
    CancelPhase, FailureReason, PublicationSlot, WorkerId, WorkerState, WorkerStep,
};
use continuum_task::region::{RegionFault, RegionId, RegionState, RegionTree};

/// How the replay states a crash of one stopped worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CrashAs {
    /// The calculus's crash step (correction 59).
    Crash,
    /// A calculus with no crash step: the worker's own `Fail` alone.
    FailOnly,
    /// Correction 58's approximation: `Resume` a parked worker, then `Fail` it.
    ResumeThenFail,
}

/// Why the calculus replay refused a journal.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ReplayFault {
    /// The calculus refused the operation an event names.
    Calculus(RegionFault),
    /// The calculus allocated another ordinal than the event names.
    Identity,
    /// The event names a reservation or an obligation no earlier event opened, so it
    /// names no calculus operation.
    UnknownHandle,
}

impl From<RegionFault> for ReplayFault {
    fn from(fault: RegionFault) -> Self {
        Self::Calculus(fault)
    }
}

/// What a replay accepted: the calculus tree, and the lifecycle state of each worker a
/// crash stopped, just before the crash.
struct Replayed {
    tree: RegionTree,
    stopped_from: Vec<WorkerState>,
    /// Staged publications the crashes discarded, summed over the stopped workers.
    discarded: u32,
}

fn crashed_reason() -> FailureReason {
    FailureReason::new("crashed").unwrap()
}

fn substrate(kind: ObligationKind, obligation: u32) -> SubstrateObligation {
    SubstrateObligation::new(
        SubstrateKind::new(kind.token()).unwrap(),
        SubstrateId::at(u64::from(obligation)),
    )
}

/// The calculus step a lifecycle task step names, written here from the family table
/// (`family::lifecycle`'s header) rather than taken from the lift's own mapping.
fn worker_step(step: &TaskStep) -> WorkerStep {
    match step {
        TaskStep::Begin => WorkerStep::Begin,
        TaskStep::Suspend => WorkerStep::Suspend,
        TaskStep::Resume => WorkerStep::Resume,
        TaskStep::Complete => WorkerStep::Complete,
        TaskStep::Fail(reason) => WorkerStep::Fail(reason.clone()),
        TaskStep::Cancel => WorkerStep::CompleteCancelled,
        TaskStep::CancelRequested => WorkerStep::RequestCancel,
    }
}

/// Replay a journal on a fresh region calculus through its public operations only: the
/// calculus operation each lifecycle, effect, obligation and cancellation-acknowledgement
/// event names, per the family tables (`family::lifecycle`, `family::effect`,
/// `family::obligation`, `family::cancellation`), and none of the lift's own
/// cross-checks (stopped sets, drain sets, fences, settles, timers, channels). A crash
/// is `crash_as` for each stopped worker, then a close of the region when it is still
/// open. This is the calculus's own judgement of a journal: which step sequences it
/// admits.
///
/// Two readings are shared with the lift, because the calculus needs them to name an
/// operation at all: a cleanup abort with the `cancel` cause stands for the
/// acknowledgement when the journal reports no phase (correction 53), and a timer's or
/// channel's cleanup is not replayed (those families take no calculus step but that
/// acknowledgement). A single task's deadline request (cancellation `requested` with
/// the `deadline` cause) is out of scope: the crash corpus has none, and the lifecycle
/// family's `cancel-requested` carries its calculus step.
fn replay(journal: &Journal, crash_as: CrashAs) -> Result<Replayed, (usize, ReplayFault)> {
    let mut tree = RegionTree::new();
    let mut reservations = BTreeMap::<u32, u32>::new();
    let mut obligations = BTreeMap::<u32, (ObligationKind, u32)>::new();
    let mut stopped_from = Vec::new();
    let mut discarded = 0_u32;
    for (at, event) in journal.events().iter().enumerate() {
        let mut step = |tree: &mut RegionTree| -> Result<(), ReplayFault> {
            // A cleanup step stands for the acknowledgement when no phase is journaled.
            let imply_ack = |tree: &mut RegionTree, task: u32| -> Result<(), ReplayFault> {
                let worker = WorkerId::at(task);
                if tree.cancel_phase(worker)? == CancelPhase::Requested {
                    tree.advance(worker, WorkerStep::AcknowledgeCancel)?;
                }
                Ok(())
            };
            match event.body() {
                EventBody::Lifecycle(e) => match e {
                    LifecycleEvent::RegionOpened { region, parent } => {
                        let id = tree.open_child(RegionId::at(parent.0))?;
                        if id.ordinal() != region.0 {
                            return Err(ReplayFault::Identity);
                        }
                    }
                    LifecycleEvent::TaskSpawned {
                        task,
                        region,
                        resumability,
                    } => {
                        let id = tree.spawn(RegionId::at(region.0), resumability.clone())?;
                        if id.ordinal() != task.0 {
                            return Err(ReplayFault::Identity);
                        }
                    }
                    LifecycleEvent::TaskStepped { task, step } => {
                        if *step == TaskStep::Cancel {
                            imply_ack(tree, task.0)?;
                        }
                        tree.advance(WorkerId::at(task.0), worker_step(step))?;
                    }
                    LifecycleEvent::RegionCloseRequested { region } => {
                        tree.close(RegionId::at(region.0))?;
                    }
                    LifecycleEvent::RegionCancelRequested { region } => {
                        tree.cancel(RegionId::at(region.0))?;
                    }
                    LifecycleEvent::RegionDrained { region, .. } => {
                        tree.drain(RegionId::at(region.0))?;
                    }
                    LifecycleEvent::RegionFinalized { region } => {
                        tree.finalize(RegionId::at(region.0))?;
                    }
                    LifecycleEvent::RegionCrashed { region, fenced } => {
                        for task in fenced.as_slice() {
                            let worker = WorkerId::at(task.0);
                            let from = tree.worker_state(worker)?.clone();
                            discarded += tree.evidence(worker)?.staged();
                            match crash_as {
                                CrashAs::Crash => {
                                    tree.advance(worker, WorkerStep::Crash(crashed_reason()))?;
                                }
                                CrashAs::FailOnly => {
                                    tree.advance(worker, WorkerStep::Fail(crashed_reason()))?;
                                }
                                CrashAs::ResumeThenFail => {
                                    if from == WorkerState::Suspended {
                                        tree.advance(worker, WorkerStep::Resume)?;
                                    }
                                    tree.advance(worker, WorkerStep::Fail(crashed_reason()))?;
                                }
                            }
                            stopped_from.push(from);
                        }
                        let id = RegionId::at(region.0);
                        if tree.state(id)? == RegionState::Open {
                            tree.close(id)?;
                        }
                    }
                },
                EventBody::Effect(e) => match e {
                    EffectEvent::Reserved { reservation, task } => {
                        reservations.insert(reservation.0, task.0);
                        tree.advance(
                            WorkerId::at(task.0),
                            WorkerStep::ReserveSlot(PublicationSlot::at(reservation.0)),
                        )?;
                    }
                    EffectEvent::Committed { reservation } => {
                        let holder = reservations
                            .get(&reservation.0)
                            .copied()
                            .ok_or(ReplayFault::UnknownHandle)?;
                        tree.advance(
                            WorkerId::at(holder),
                            WorkerStep::CommitSlot(PublicationSlot::at(reservation.0)),
                        )?;
                    }
                    EffectEvent::Aborted { reservation, cause } => {
                        let holder = reservations
                            .get(&reservation.0)
                            .copied()
                            .ok_or(ReplayFault::UnknownHandle)?;
                        match cause {
                            AbortCause::Explicit => {
                                tree.advance(
                                    WorkerId::at(holder),
                                    WorkerStep::AbortSlot(PublicationSlot::at(reservation.0)),
                                )?;
                            }
                            // A cancellation's abort is the drain's discard.
                            AbortCause::Cancel => imply_ack(tree, holder)?,
                        }
                    }
                    EffectEvent::Fenced { .. } => {}
                },
                EventBody::Obligation(e) => match e {
                    ObligationEvent::Opened {
                        obligation,
                        kind,
                        holder,
                        ..
                    } => {
                        obligations.insert(obligation.0, (*kind, holder.0));
                        tree.open_substrate(
                            WorkerId::at(holder.0),
                            substrate(*kind, obligation.0),
                        )?;
                    }
                    ObligationEvent::Discharged { obligation, how } => {
                        let (kind, holder) = obligations
                            .get(&obligation.0)
                            .copied()
                            .ok_or(ReplayFault::UnknownHandle)?;
                        tree.discharge_substrate(
                            WorkerId::at(holder),
                            substrate(kind, obligation.0),
                            match how {
                                Discharge::Committed => SubstrateOutcome::Committed,
                                Discharge::Aborted => SubstrateOutcome::Aborted,
                            },
                        )?;
                    }
                    ObligationEvent::Transferred {
                        obligation, holder, ..
                    } => {
                        let (kind, from) = obligations
                            .get(&obligation.0)
                            .copied()
                            .ok_or(ReplayFault::UnknownHandle)?;
                        tree.transfer_substrate(
                            WorkerId::at(from),
                            substrate(kind, obligation.0),
                            WorkerId::at(holder.0),
                        )?;
                        obligations.insert(obligation.0, (kind, holder.0));
                    }
                    ObligationEvent::Leaked { .. }
                    | ObligationEvent::Fenced { .. }
                    | ObligationEvent::RegionSettled { .. } => {}
                },
                EventBody::Cancellation(CancellationEvent::Acknowledged { task }) => {
                    tree.advance(WorkerId::at(task.0), WorkerStep::AcknowledgeCancel)?;
                }
                EventBody::Cancellation(_) | EventBody::Time(_) | EventBody::Channel(_) => {}
            }
            Ok(())
        };
        step(&mut tree).map_err(|fault| (at, fault))?;
    }
    Ok(Replayed {
        tree,
        stopped_from,
        discarded,
    })
}

/// The calculus state the replay and the lift reached, compared whole: every worker's
/// state, evidence and cancellation phase, every region's state, and the ledger.
fn same_calculus_state(a: &RegionTree, b: &RegionTree) -> Result<(), String> {
    if a.worker_count() != b.worker_count() || a.region_count() != b.region_count() {
        return Err("sizes".to_owned());
    }
    for w in 0..u32::try_from(a.worker_count()).unwrap() {
        let w = WorkerId::at(w);
        if a.worker_state(w) != b.worker_state(w)
            || a.evidence(w) != b.evidence(w)
            || a.cancel_phase(w) != b.cancel_phase(w)
        {
            return Err(format!(
                "{w}: {:?} {:?} {:?} vs {:?} {:?} {:?}",
                a.worker_state(w),
                a.evidence(w),
                a.cancel_phase(w),
                b.worker_state(w),
                b.evidence(w),
                b.cancel_phase(w)
            ));
        }
    }
    for r in 0..u32::try_from(a.region_count()).unwrap() {
        if a.state(RegionId::at(r)) != b.state(RegionId::at(r)) {
            return Err(format!("r{r}"));
        }
    }
    if a.ledger().outstanding() != b.ledger().outstanding()
        || a.ledger().opened() != b.ledger().opened()
        || a.ledger().discharged() != b.ledger().discharged()
    {
        return Err(format!(
            "ledger {:?} vs {:?}",
            a.ledger().outstanding(),
            b.ledger().outstanding()
        ));
    }
    Ok(())
}

#[test]
fn every_lifted_crash_is_a_legal_calculus_step_sequence() {
    let (corpus, _) = corpus();
    let mut journals = 0_usize;
    let mut stopped = BTreeMap::<&'static str, usize>::new();
    let mut refused_without_crash_step = 0_usize;
    let mut parked_journals = 0_usize;
    let mut discarded = 0_u32;
    for entry in &corpus {
        let journal = &entry.journal;
        let lifted = lift(journal);
        let lifted = lifted.conforming().expect("the corpus conforms");
        let replayed = replay(journal, CrashAs::Crash).unwrap_or_else(|(at, fault)| {
            panic!(
                "{} log {}: the calculus refuses event {at}: {fault:?}\n{}",
                entry.name,
                entry.log,
                journal.render()
            )
        });
        if let Err(diff) = same_calculus_state(&replayed.tree, lifted.tree()) {
            panic!("{} log {}: {diff}", entry.name, entry.log);
        }
        discarded += replayed.discarded;
        for from in &replayed.stopped_from {
            *stopped.entry(from.status_token()).or_default() += 1;
        }
        let parked = replayed.stopped_from.contains(&WorkerState::Suspended);
        parked_journals += usize::from(parked);
        // Without the crash step, the calculus refuses exactly the journals that crash
        // a parked worker; correction 58's injected `Resume` made those legal.
        let fail_only = replay(journal, CrashAs::FailOnly);
        assert_eq!(
            fail_only.is_err(),
            parked,
            "{} log {}",
            entry.name,
            entry.log
        );
        if let Err((_, fault)) = fail_only {
            assert!(matches!(
                fault,
                ReplayFault::Calculus(RegionFault::IllegalWorkerStep { .. })
            ));
            refused_without_crash_step += 1;
        }
        let approximated = replay(journal, CrashAs::ResumeThenFail).unwrap();
        same_calculus_state(&approximated.tree, &replayed.tree).unwrap();
        journals += 1;
    }
    eprintln!(
        "{journals} journals replayed, stopped workers by state at the crash {stopped:?}, \
         {parked_journals} crash a parked worker, {refused_without_crash_step} refused \
         by a calculus without the crash step"
    );
    assert_eq!(journals, corpus.len());
    // Pinned, as the RFC 0026 correction 59 evidence states them.
    assert_eq!(journals, 1_952);
    assert_eq!(
        stopped,
        BTreeMap::from([("created", 1_920), ("suspended", 3_872)])
    );
    assert_eq!(parked_journals, journals);
    // In the corpus every stopped task is parked or never begun, so no crash discards a
    // staged publication there.
    assert_eq!(discarded, 0);
    // `crash_program` crashes `t1` right after its reserve, still running with the
    // publication staged: the crash of a running worker, which discards what it staged.
    let mut running_programs = 0_usize;
    for sleep in [false, true] {
        let (programs, log) = one_actor(crash_program(sleep));
        for config in every_projection() {
            let journal = run(&programs, &log, &config).unwrap();
            let lifted = lift(&journal);
            let lifted = lifted.conforming().expect("the crash program conforms");
            let replayed = replay(&journal, CrashAs::Crash).expect("the calculus admits it");
            same_calculus_state(&replayed.tree, lifted.tree()).unwrap();
            assert!(replayed.stopped_from.contains(&WorkerState::Running));
            let has_effects = config.families.contains(Family::Effect);
            assert_eq!(replayed.discarded > 0, has_effects);
            running_programs += 1;
        }
    }
    assert_eq!(running_programs, 64);
    eprintln!(
        "{running_programs} crash-program journals stop a running worker; \
         {discarded} corpus publications discarded"
    );
    assert_eq!(refused_without_crash_step, journals);
    // The corpus stops parked and never-begun workers only; the crash program below
    // stops a running one.
    assert!(
        stopped.get("suspended").copied().unwrap_or(0) > 1_000,
        "{stopped:?}"
    );
    assert!(
        stopped.get("created").copied().unwrap_or(0) > 1_000,
        "{stopped:?}"
    );
    assert!(refused_without_crash_step > 1_000);
}

#[test]
fn on_the_mutation_corpus_the_calculus_and_the_lift_agree_on_the_calculus_steps() {
    let config = all_families();
    let (corpus, _) = corpus();
    let mut mutants: Vec<(String, Journal)> = Vec::new();
    let bases: Vec<&Journal> = corpus
        .iter()
        .filter(|e| e.config == config && has_crash(&e.journal))
        .map(|e| &e.journal)
        .step_by(7)
        .collect();
    let (programs, log) = one_actor(crash_program(true));
    let sleeper = run(&programs, &log, &config).unwrap();
    for (name, perturb, _) in perturbations() {
        for base in bases.iter().copied().chain([&sleeper]) {
            if let Some(mutant) = perturb(base) {
                mutants.push((name.to_owned(), mutant));
            }
        }
    }
    for entry in corpus.iter().filter(|e| has_crash(&e.journal)).step_by(11) {
        let b = bodies(&entry.journal);
        for i in 0..b.len() {
            let mut deleted = b.clone();
            deleted.remove(i);
            mutants.push((format!("delete {i}"), rebuild(deleted)));
            if i + 1 < b.len() && b[i] != b[i + 1] {
                let mut swapped = b.clone();
                swapped.swap(i, i + 1);
                mutants.push((format!("swap {i}"), rebuild(swapped)));
            }
        }
    }
    let mut calculus_rejects = 0_usize;
    let mut unknown_handle = 0_usize;
    let mut lift_refused_by_calculus = 0_usize;
    let mut lift_rejects_on_its_own_checks = 0_usize;
    let mut lift_conforms = 0_usize;
    let mut failures = Vec::new();
    for (what, mutant) in &mutants {
        let by_calculus = replay(mutant, CrashAs::Crash);
        let by_lift = lift(mutant);
        // The lift accepts nothing the calculus refuses: every lifted step is one.
        if let Err((_, fault)) = &by_calculus {
            calculus_rejects += 1;
            unknown_handle += usize::from(*fault == ReplayFault::UnknownHandle);
            if by_lift.conforming().is_some() {
                failures.push(format!("{what}: the calculus refuses, the lift conforms"));
            }
        }
        match &by_lift {
            // The calculus accepts nothing the lift refuses as a calculus fault.
            // ... at the same event, and for the same fault.
            LiftVerdict::Violates {
                seq,
                reason: Nonconformance::Refused(fault),
            } => {
                lift_refused_by_calculus += 1;
                let same = matches!(
                    &by_calculus,
                    Err((at, ReplayFault::Calculus(f)))
                        if f == fault && mutant.events()[*at].seq() == *seq
                );
                if !same {
                    failures.push(format!(
                        "{what}: the lift's calculus refuses {fault} at {seq}, the replay says {:?}",
                        by_calculus.as_ref().err()
                    ));
                }
            }
            LiftVerdict::Conforms(_) => lift_conforms += 1,
            _ => lift_rejects_on_its_own_checks += 1,
        }
    }
    eprintln!(
        "{} mutants: calculus refuses {calculus_rejects} ({unknown_handle} naming no opened \
         handle); the lift refuses \
         {lift_refused_by_calculus} as a calculus fault and {lift_rejects_on_its_own_checks} \
         on its own cross-checks or as inconclusive, and conforms on {lift_conforms}",
        mutants.len()
    );
    assert!(
        failures.is_empty(),
        "{} disagreements:\n{}",
        failures.len(),
        failures
            .iter()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    // Pinned, as the RFC 0026 correction 59 evidence states them.
    assert_eq!(
        (
            mutants.len(),
            calculus_rejects,
            unknown_handle,
            lift_refused_by_calculus,
            lift_rejects_on_its_own_checks,
            lift_conforms,
        ),
        (13_373, 7_585, 381, 4_984, 4_984, 3_405)
    );
}
