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

#![allow(clippy::too_many_lines)]

#[path = "support/primitive_conformance_model.rs"]
mod model;

use std::collections::BTreeMap;

use continuum_asupersync::binding::{BindingConfig, BindingRefusal, Program, SubstrateOp, run};
use continuum_asupersync::choice::ChoiceLog;
use continuum_asupersync::family::cancellation::CancellationEvent;
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
            let by_model = judge(&entry.config, &variant).is_accepted();
            let by_lift = lift_accepts(&variant);
            compared += 1;
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
    eprintln!("compared {compared}, rejected by both {rejected_by_both}");
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
