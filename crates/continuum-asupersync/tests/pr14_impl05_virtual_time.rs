//! PR-14-IMPL-05 (bn-3m1d): virtual time, observed from the real substrate.
//!
//! > **Exit:** identical controlled choice logs produce canonical identical semantic
//! > events.
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 14
//!
//! The binding (`binding.rs`) drives asupersync 0.5.0's lab runtime under a choice log.
//! Bound tasks sleep on their `Cx`'s virtual timer driver (`time::sleep_until`), and the
//! choice log advances the lab's virtual clock (`LabRuntime::advance_time`). With the
//! virtual time family observed, the binding journals timer scheduling, clock advances,
//! timer fires and timer cancellations from the substrate's own trace and clock. Time is
//! a capability here: nothing reads the host clock, and timers that fire in one advance
//! are journaled in canonical order, so the lab seed does not reach the journal.
//!
//! # Evidence map
//!
//! | Claim | Test |
//! |---|---|
//! | identical choice logs → byte-identical journals, every log (sibling regions and timer ties) | [`identical_choice_logs_give_byte_identical_time_journals`] |
//! | the lab seed does not reach the journal, though it reorders ties and sibling closes (7 seeds) | [`the_lab_seed_does_not_reach_the_time_journal`] |
//! | different choice logs → different journals (anti-vacuity of the identity) | [`different_choice_logs_give_different_time_journals`] |
//! | substrate journal ≡ an independent scripted account, every log (differential) | [`the_substrate_agrees_byte_for_byte_with_a_scripted_account`] |
//! | the family adds events and changes no byte of the other four families' journals | [`dropping_a_family_gives_that_projection_byte_for_byte`] |
//! | every journal lifts, with the parallel clock and timer model | [`every_time_journal_conforms`] |
//! | one pinned run, rendered: a tie fired by deadline then ordinal | [`a_timer_tie_fires_in_canonical_order`] |
//! | mutated journals are rejected, each with its own fault (anti-vacuity) | [`mutated_time_journals_are_rejected`] |
//! | zero durations, commands to sleepers, lost timers, bad bytes: typed refusals | [`refusals_are_typed`] |
//! | no host clock in the family or the binding | [`no_host_clock_is_named`] |

use std::collections::BTreeMap;

use continuum_asupersync::binding::{
    BindingConfig, BindingRefusal, Program, SubstrateOp, run, run_witnessed,
};
use continuum_asupersync::choice::ChoiceLog;
use continuum_asupersync::encoding::DecodeError;
use continuum_asupersync::family::cancellation::{CancelCause, CancellationReport};
use continuum_asupersync::family::effect::ReservationLabel;
use continuum_asupersync::family::lifecycle::{
    LifecycleEvent, LifecycleReport, RegionLabel, RegionOrdinal, TaskLabel, TaskOrdinal, TaskStep,
};
use continuum_asupersync::family::obligation::{
    Discharge, ObligationEvent, ObligationKind, ObligationOrdinal, ObligationReport, ObligationSet,
};
use continuum_asupersync::family::time::{
    TimeEvent, TimeFault, TimeReport, TimerOrdinal, VirtualInstant,
};
use continuum_asupersync::family::{EventBody, Family, Report};
use continuum_asupersync::journal::Journal;
use continuum_asupersync::lift::{LiftVerdict, Nonconformance, lift};
use continuum_asupersync::source::{Script, record};
use continuum_task::region::worker::Resumability;
use continuum_value::assurance::InconclusiveReason;

const SEED: u64 = 0;
const SEEDS: [u64; 7] = [0, 1, 7, 42, 99, 0x9e37_79b9_7f4a_7c15, u64::MAX];

/// The configuration the tests run: every bound family observed.
fn config(seed: u64) -> BindingConfig {
    BindingConfig::new(seed)
        .observing(Family::Effect)
        .observing(Family::Cancellation)
        .observing(Family::Obligation)
        .observing(Family::Time)
}

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

fn begin(task: TaskLabel) -> SubstrateOp {
    SubstrateOp::Begin { task }
}

fn sleep(task: TaskLabel, nanos: u64) -> SubstrateOp {
    SubstrateOp::Sleep { task, nanos }
}

fn advance(nanos: u64) -> SubstrateOp {
    SubstrateOp::Advance { nanos }
}

/// Three actors. A root task that sleeps. A cancelled region with two sibling child
/// regions, each with a task that sleeps the same duration (a tie whenever both sleep at
/// the same instant), so the cancellation cancels whatever timer has not fired. And the
/// clock, which advances twice. Every interleaving is a legal run.
///
/// ```text
/// r1 ─┬─ r2 (b sleeps 100)
///     └─ r3 (c sleeps 100)
/// ```
fn clocked() -> Vec<Program> {
    let (a, b, c) = (TaskLabel(1), TaskLabel(2), TaskLabel(3));
    let (r1, r2, r3) = (RegionLabel(1), RegionLabel(2), RegionLabel(3));
    vec![
        vec![spawn(RegionLabel::ROOT, a), begin(a), sleep(a, 100)],
        vec![
            open(RegionLabel::ROOT, r1),
            open(r1, r2),
            open(r1, r3),
            spawn(r2, b),
            spawn(r3, c),
            begin(b),
            begin(c),
            sleep(b, 100),
            sleep(c, 100),
            SubstrateOp::Cancel { region: r1 },
        ],
        vec![advance(60), advance(60)],
    ]
}

/// Three actors. Two root tasks that sleep the same duration, the first holding a lease
/// across its sleep, and one advance that fires both when both slept first.
fn ties() -> Vec<Program> {
    let (a, b) = (TaskLabel(1), TaskLabel(2));
    vec![
        vec![
            spawn(RegionLabel::ROOT, a),
            begin(a),
            SubstrateOp::Acquire {
                task: a,
                reservation: ReservationLabel(1),
                kind: ObligationKind::Lease,
            },
            sleep(a, 40),
        ],
        vec![spawn(RegionLabel::ROOT, b), begin(b), sleep(b, 40)],
        vec![advance(40)],
    ]
}

fn lengths(programs: &[Program]) -> Vec<usize> {
    programs.iter().map(Vec::len).collect()
}

const CLOCKED_LOGS: usize = 30_030; // 15! / (3!·10!·2!)
const TIES_LOGS: usize = 280; // 8! / (4!·3!·1!)
const ALL_LOGS: usize = CLOCKED_LOGS + TIES_LOGS;

/// A corpus entry: name, programs, logs.
type Entry = (&'static str, Vec<Program>, Vec<ChoiceLog>);

fn corpus() -> Vec<Entry> {
    let clocked = clocked();
    let ties = ties();
    let clocked_logs = ChoiceLog::enumerate(&lengths(&clocked));
    let ties_logs = ChoiceLog::enumerate(&lengths(&ties));
    assert_eq!(clocked_logs.len(), CLOCKED_LOGS);
    assert_eq!(ties_logs.len(), TIES_LOGS);
    vec![
        ("clocked", clocked, clocked_logs),
        ("ties", ties, ties_logs),
    ]
}

fn time_events(journal: &Journal) -> Vec<&TimeEvent> {
    journal
        .events()
        .iter()
        .filter_map(|event| match event.body() {
            EventBody::Time(event) => Some(event),
            _ => None,
        })
        .collect()
}

// --- the exit property ---------------------------------------------------------------

#[test]
fn identical_choice_logs_give_byte_identical_time_journals() {
    let mut compared = 0;
    let mut ties_fired_together = 0;
    for (name, programs, logs) in corpus() {
        for log in &logs {
            let first = run(&programs, log, &config(SEED)).expect("every log is a legal run");
            let second = run(&programs, log, &config(SEED)).expect("every log is a legal run");
            assert_eq!(
                first.encode().unwrap(),
                second.encode().unwrap(),
                "{name} log {log}"
            );
            assert_eq!(first.digest().unwrap(), second.digest().unwrap());
            // Non-vacuity: every task sleeps once, and the clock moves every time.
            let events = time_events(&first);
            let scheduled = events
                .iter()
                .filter(|e| matches!(e, TimeEvent::Scheduled { .. }))
                .count();
            let advanced = events
                .iter()
                .filter(|e| matches!(e, TimeEvent::Advanced { .. }))
                .count();
            let expected = if name == "clocked" { (3, 2) } else { (2, 1) };
            assert_eq!((scheduled, advanced), expected, "{name} log {log}");
            let fired_at: Vec<u64> = events
                .iter()
                .filter_map(|e| match e {
                    TimeEvent::Fired { at, .. } => Some(at.0),
                    _ => None,
                })
                .collect();
            if fired_at.windows(2).any(|w| w[0] == w[1]) {
                ties_fired_together += 1;
            }
            compared += 1;
        }
    }
    assert_eq!(compared, ALL_LOGS);
    assert!(
        ties_fired_together > 500,
        "{ties_fired_together} runs fired a tie"
    );
}

#[test]
fn the_lab_seed_does_not_reach_the_time_journal() {
    let mut compared = 0;
    let mut fires_reordered = 0;
    let mut closes_reordered = 0;
    for (name, programs, logs) in corpus() {
        let sample = logs
            .iter()
            .enumerate()
            .filter(|(index, _)| index % 7 == 0 || *index + 1 == logs.len())
            .map(|(_, log)| log);
        for log in sample {
            let reference = run_witnessed(&programs, log, &config(SEEDS[0])).unwrap();
            for seed in &SEEDS[1..] {
                let other = run_witnessed(&programs, log, &config(*seed)).unwrap();
                assert_eq!(
                    reference.journal.encode().unwrap(),
                    other.journal.encode().unwrap(),
                    "{name} log {log} seed {seed:#x}"
                );
                if reference.substrate_fire_order != other.substrate_fire_order {
                    fires_reordered += 1;
                }
                if reference.substrate_close_order != other.substrate_close_order {
                    closes_reordered += 1;
                }
                compared += 1;
            }
        }
    }
    assert!(compared > 25_000, "the relation ran over {compared} pairs");
    // Anti-vacuity: the seed does move what the journal canonicalizes.
    assert!(
        fires_reordered > 0,
        "no seed moved the substrate's fire order"
    );
    assert!(
        closes_reordered > 0,
        "no seed moved the substrate's close order"
    );
}

#[test]
fn different_choice_logs_give_different_time_journals() {
    for (name, programs, logs) in corpus() {
        let mut seen: BTreeMap<Vec<u8>, ChoiceLog> = BTreeMap::new();
        for log in &logs {
            let bytes = run(&programs, log, &config(SEED))
                .unwrap()
                .encode()
                .unwrap();
            if let Some(previous) = seen.insert(bytes, log.clone()) {
                panic!("{name}: logs {previous} and {log} produced the same journal");
            }
        }
        assert_eq!(seen.len(), logs.len(), "{name}");
    }
}

// --- the differential: substrate against an independent scripted account ------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Gate {
    Created,
    Running,
    Suspended,
}

/// The test's own account of a run: the region tree, the tasks and their gate phase,
/// the virtual clock, each task's timer, and each task's open obligations. It calls
/// nothing in the adapter or in the substrate.
#[derive(Default)]
struct Account {
    regions: BTreeMap<RegionLabel, u32>,
    region_labels: Vec<RegionLabel>,
    parents: Vec<Option<u32>>,
    finalized: Vec<bool>,
    tasks: BTreeMap<TaskLabel, u32>,
    task_labels: Vec<TaskLabel>,
    task_region: Vec<u32>,
    live: Vec<bool>,
    gate: Vec<Gate>,
    now: u64,
    next_timer: u32,
    asleep: BTreeMap<u32, (u64, u32)>,
    obligations: Vec<(u32, u32, bool)>,
    script: Script,
}

impl Account {
    fn new() -> Self {
        let mut account = Self::default();
        account.regions.insert(RegionLabel::ROOT, 0);
        account.region_labels.push(RegionLabel::ROOT);
        account.parents.push(None);
        account.finalized.push(false);
        account
    }

    fn lc(&mut self, report: LifecycleReport) {
        self.script.push(Report::Lifecycle(report));
    }

    fn time(&mut self, event: TimeEvent) {
        self.script.push(Report::Time(TimeReport(event)));
    }

    fn step(&mut self, task: u32, step: TaskStep) {
        let task = self.task_labels[task as usize];
        self.lc(LifecycleReport::Step { task, step });
    }

    fn in_subtree(&self, member: u32, root: u32) -> bool {
        let mut at = Some(member);
        while let Some(region) = at {
            if region == root {
                return true;
            }
            at = self.parents[region as usize];
        }
        false
    }

    fn post_order(&self, root: u32) -> Vec<u32> {
        let mut out = Vec::new();
        for child in 0..self.parents.len() {
            let child = u32::try_from(child).unwrap();
            if self.parents[child as usize] == Some(root) && !self.finalized[child as usize] {
                out.extend(self.post_order(child));
            }
        }
        out.push(root);
        out
    }

    fn wake(&mut self, task: u32) {
        match self.gate[task as usize] {
            Gate::Created => self.step(task, TaskStep::Begin),
            Gate::Suspended => self.step(task, TaskStep::Resume),
            Gate::Running => {}
        }
        self.gate[task as usize] = Gate::Running;
    }

    fn park(&mut self, task: u32) {
        self.step(task, TaskStep::Suspend);
        self.gate[task as usize] = Gate::Suspended;
    }

    fn apply(&mut self, op: &SubstrateOp) {
        match op {
            SubstrateOp::OpenRegion { parent, child } => {
                let ordinal = u32::try_from(self.parents.len()).unwrap();
                self.parents.push(Some(self.regions[parent]));
                self.finalized.push(false);
                self.regions.insert(*child, ordinal);
                self.region_labels.push(*child);
                self.lc(LifecycleReport::OpenRegion {
                    parent: *parent,
                    child: *child,
                });
            }
            SubstrateOp::Spawn {
                region,
                task,
                resumability,
            } => {
                self.tasks
                    .insert(*task, u32::try_from(self.live.len()).unwrap());
                self.task_labels.push(*task);
                self.task_region.push(self.regions[region]);
                self.live.push(true);
                self.gate.push(Gate::Created);
                self.lc(LifecycleReport::Spawn {
                    region: *region,
                    task: *task,
                    resumability: resumability.clone(),
                });
            }
            SubstrateOp::Begin { task } => {
                let task = self.tasks[task];
                self.wake(task);
                self.park(task);
            }
            SubstrateOp::Acquire { task, kind, .. } => {
                let task = self.tasks[task];
                self.wake(task);
                self.park(task);
                let obligation = u32::try_from(self.obligations.len()).unwrap();
                let region = self.task_region[task as usize];
                self.obligations.push((task, region, true));
                self.script.push(Report::Obligation(ObligationReport(
                    ObligationEvent::Opened {
                        obligation: ObligationOrdinal(obligation),
                        kind: *kind,
                        holder: TaskOrdinal(task),
                        region: RegionOrdinal(region),
                    },
                )));
            }
            SubstrateOp::Sleep { task, nanos } => {
                let task = self.tasks[task];
                self.wake(task);
                let timer = self.next_timer;
                self.next_timer += 1;
                let deadline = self.now + nanos;
                self.time(TimeEvent::Scheduled {
                    timer: TimerOrdinal(timer),
                    task: TaskOrdinal(task),
                    at: VirtualInstant(self.now),
                    deadline: VirtualInstant(deadline),
                });
                self.asleep.insert(task, (deadline, timer));
                self.park(task);
            }
            SubstrateOp::Advance { nanos } => {
                let from = self.now;
                self.now += nanos;
                self.time(TimeEvent::Advanced {
                    from: VirtualInstant(from),
                    to: VirtualInstant(self.now),
                });
                let mut due: Vec<(u64, u32, u32)> = self
                    .asleep
                    .iter()
                    .filter(|(_, (deadline, _))| *deadline <= self.now)
                    .map(|(task, (deadline, timer))| (*deadline, *timer, *task))
                    .collect();
                due.sort_unstable();
                for (_, timer, task) in due {
                    self.asleep.remove(&task);
                    self.time(TimeEvent::Fired {
                        timer: TimerOrdinal(timer),
                        at: VirtualInstant(self.now),
                    });
                    self.wake(task);
                    self.park(task);
                }
            }
            SubstrateOp::Cancel { region } => {
                self.lc(LifecycleReport::Cancel { region: *region });
                let ordinal = self.regions[region];
                let mut causes = BTreeMap::new();
                for task in 0..self.live.len() {
                    let task = u32::try_from(task).unwrap();
                    if !self.live[task as usize]
                        || !self.in_subtree(self.task_region[task as usize], ordinal)
                    {
                        continue;
                    }
                    let cause = if self.task_region[task as usize] == ordinal {
                        CancelCause::User
                    } else {
                        CancelCause::ParentCancelled
                    };
                    causes.insert(task, cause);
                    self.script
                        .push(Report::Cancellation(CancellationReport::Request {
                            task: TaskOrdinal(task),
                            cause,
                        }));
                }
                self.teardown(ordinal, &causes);
            }
            other => unreachable!("the time corpus has no {other:?}"),
        }
    }

    fn teardown(&mut self, region: u32, causes: &BTreeMap<u32, CancelCause>) {
        for member in self.post_order(region) {
            for task in 0..self.live.len() {
                let task = u32::try_from(task).unwrap();
                if self.task_region[task as usize] != member || !self.live[task as usize] {
                    continue;
                }
                let Some(cause) = causes.get(&task).copied() else {
                    continue;
                };
                self.script
                    .push(Report::Cancellation(CancellationReport::Acknowledge {
                        task: TaskOrdinal(task),
                    }));
                if let Some((_, timer)) = self.asleep.remove(&task) {
                    self.time(TimeEvent::Cancelled {
                        timer: TimerOrdinal(timer),
                        at: VirtualInstant(self.now),
                    });
                }
                for index in 0..self.obligations.len() {
                    let (holder, _, open) = self.obligations[index];
                    if holder == task && open {
                        self.obligations[index].2 = false;
                        self.script.push(Report::Obligation(ObligationReport(
                            ObligationEvent::Discharged {
                                obligation: ObligationOrdinal(u32::try_from(index).unwrap()),
                                how: Discharge::Aborted,
                            },
                        )));
                    }
                }
                self.script
                    .push(Report::Cancellation(CancellationReport::Complete {
                        task: TaskOrdinal(task),
                        cause,
                    }));
                self.live[task as usize] = false;
            }
            let label = self.region_labels[member as usize];
            self.lc(LifecycleReport::Drain { region: label });
            self.lc(LifecycleReport::Finalize { region: label });
            self.finalized[member as usize] = true;
            let open = ObligationSet::new(
                self.obligations
                    .iter()
                    .enumerate()
                    .filter(|(_, (_, r, open))| *r == member && *open)
                    .map(|(i, _)| ObligationOrdinal(u32::try_from(i).unwrap())),
            );
            self.script.push(Report::Obligation(ObligationReport(
                ObligationEvent::RegionSettled {
                    region: RegionOrdinal(member),
                    open,
                    leaked: ObligationSet::default(),
                    fenced: ObligationSet::default(),
                },
            )));
        }
    }
}

fn scripted(programs: &[Program], log: &ChoiceLog) -> (Vec<Script>, ChoiceLog) {
    let mut account = Account::new();
    let mut cursors = vec![0_usize; programs.len()];
    for choice in log.choices() {
        let enabled: Vec<usize> = (0..programs.len())
            .filter(|actor| cursors[*actor] < programs[*actor].len())
            .collect();
        let actor = enabled[choice.0 as usize];
        account.apply(&programs[actor][cursors[actor]]);
        cursors[actor] += 1;
    }
    let len = account.script.len();
    (vec![account.script], ChoiceLog::new(vec![0; len]))
}

#[test]
fn the_substrate_agrees_byte_for_byte_with_a_scripted_account() {
    let mut compared = 0;
    for (name, programs, logs) in corpus() {
        for log in &logs {
            let substrate = run(&programs, log, &config(SEED)).unwrap();
            let (scripts, scripted_log) = scripted(&programs, log);
            let model = record(&scripts, &scripted_log).expect("the account is recordable");
            assert_eq!(
                substrate.encode().unwrap(),
                model.encode().unwrap(),
                "{name} log {log}\nsubstrate:\n{}scripted:\n{}",
                substrate.render(),
                model.render()
            );
            compared += 1;
        }
    }
    assert_eq!(compared, ALL_LOGS);
}

fn without(journal: &Journal, dropped: &[Family]) -> Journal {
    let mut projected = Journal::new();
    for event in journal.events() {
        if !dropped.contains(&event.family()) {
            projected.append(event.body().clone()).unwrap();
        }
    }
    projected
}

/// The family is additive: remove any bound families' events from the full journal and
/// what remains is, byte for byte, the journal of a run that does not observe them. So
/// the IMPL-01 to IMPL-04 journals are unchanged by this bone.
#[test]
fn dropping_a_family_gives_that_projection_byte_for_byte() {
    let optional = [
        Family::Effect,
        Family::Cancellation,
        Family::Obligation,
        Family::Time,
    ];
    let mut compared = 0;
    for (name, programs, logs) in corpus() {
        for log in logs.iter().step_by(97) {
            let full = run(&programs, log, &config(SEED)).unwrap();
            for mask in 1_u8..16 {
                let dropped: Vec<Family> = optional
                    .iter()
                    .enumerate()
                    .filter(|(bit, _)| mask & (1 << bit) != 0)
                    .map(|(_, family)| *family)
                    .collect();
                let mut projection = BindingConfig::new(SEED);
                for family in optional {
                    if !dropped.contains(&family) {
                        projection = projection.observing(family);
                    }
                }
                let expected = run(&programs, log, &projection).unwrap();
                assert_eq!(
                    without(&full, &dropped).encode().unwrap(),
                    expected.encode().unwrap(),
                    "{name} log {log} without {dropped:?}"
                );
                compared += 1;
            }
        }
    }
    assert!(compared > 4000, "{compared} projections");
}

// --- conformance ---------------------------------------------------------------------

#[test]
fn every_time_journal_conforms() {
    for (name, programs, logs) in corpus() {
        for log in &logs {
            let journal = run(&programs, log, &config(SEED)).unwrap();
            let journal = Journal::decode(&journal.encode().unwrap()).unwrap();
            let LiftVerdict::Conforms(lifted) = lift(&journal) else {
                panic!("{name} log {log}:\n{}", journal.render());
            };
            for (_, finalization) in lifted.finalizations() {
                assert!(finalization.orphans().is_empty(), "{name} log {log}");
            }
            // Each timer ends at most once, and never before it is due.
            let mut deadlines = BTreeMap::new();
            for event in time_events(&journal) {
                match event {
                    TimeEvent::Scheduled {
                        timer, deadline, ..
                    } => {
                        assert!(deadlines.insert(timer.0, deadline.0).is_none());
                    }
                    TimeEvent::Fired { timer, at } => {
                        assert!(
                            deadlines.remove(&timer.0).unwrap() <= at.0,
                            "{name} log {log}"
                        );
                    }
                    TimeEvent::Cancelled { timer, .. } => {
                        assert!(deadlines.remove(&timer.0).is_some());
                    }
                    TimeEvent::Fenced { timer } => {
                        assert!(deadlines.remove(&timer.0).is_some());
                    }
                    TimeEvent::Advanced { .. } | TimeEvent::Deadline { .. } => {}
                }
            }
        }
    }
}

/// One run, pinned in full: two tasks sleep 40 at instant 0, the first holding a lease,
/// and one advance fires both. They fire in deadline-then-ordinal order, each followed by
/// its task's resume and suspend, whatever order the substrate polled them in.
#[test]
fn a_timer_tie_fires_in_canonical_order() {
    let log = ChoiceLog::new([0, 0, 0, 0, 0, 0, 0, 0]);
    let journal = run(&ties(), &log, &config(SEED)).unwrap();
    assert_eq!(
        journal.render(),
        "0 lifecycle task-spawned t0 region=r0 resumable
1 lifecycle task-stepped t0 begin
2 lifecycle task-stepped t0 suspend
3 lifecycle task-stepped t0 resume
4 lifecycle task-stepped t0 suspend
5 obligation opened o0 lease holder=t0 region=r0
6 lifecycle task-stepped t0 resume
7 virtual-time scheduled k0 task=t0 at=0 deadline=40
8 lifecycle task-stepped t0 suspend
9 lifecycle task-spawned t1 region=r0 resumable
10 lifecycle task-stepped t1 begin
11 lifecycle task-stepped t1 suspend
12 lifecycle task-stepped t1 resume
13 virtual-time scheduled k1 task=t1 at=0 deadline=40
14 lifecycle task-stepped t1 suspend
15 virtual-time advanced 0->40
16 virtual-time fired k0 at=40
17 lifecycle task-stepped t0 resume
18 lifecycle task-stepped t0 suspend
19 virtual-time fired k1 at=40
20 lifecycle task-stepped t1 resume
21 lifecycle task-stepped t1 suspend
"
    );
    let bytes = journal.encode().unwrap();
    assert_eq!(Journal::decode(&bytes).unwrap(), journal);
    assert!(matches!(lift(&journal), LiftVerdict::Conforms(_)));
    // The substrate fired the tie in the same journal order under every seed only
    // because the binding orders it; its own order is its scheduler's.
    for seed in SEEDS {
        let witnessed = run_witnessed(&ties(), &log, &config(seed)).unwrap();
        assert_eq!(witnessed.journal, journal, "seed {seed:#x}");
    }
}

// --- anti-vacuity: the lift rejects wrong clocks ---------------------------------------

fn bodies(journal: &Journal) -> Vec<EventBody> {
    journal.events().iter().map(|e| e.body().clone()).collect()
}

fn rebuilt(bodies: Vec<EventBody>) -> Journal {
    let mut journal = Journal::new();
    for body in bodies {
        journal.append(body).unwrap();
    }
    journal
}

fn position(bodies: &[EventBody], wanted: impl Fn(&TimeEvent) -> bool) -> Option<usize> {
    bodies
        .iter()
        .position(|body| matches!(body, EventBody::Time(event) if wanted(event)))
}

fn time_fault(journal: &Journal) -> TimeFault {
    match lift(journal) {
        LiftVerdict::Violates {
            reason: Nonconformance::Time(fault),
            ..
        } => fault,
        other => panic!("expected a time fault, got {other:?}\n{}", journal.render()),
    }
}

/// Seven mutation operators over real substrate journals, each rejected with its own
/// fault. The unmutated journal conforms, so each rejection is the mutant's doing.
#[test]
fn mutated_time_journals_are_rejected() {
    let programs = clocked();
    let logs = ChoiceLog::enumerate(&lengths(&programs));
    let mut mutants = 0;
    for log in logs.iter().step_by(23) {
        let journal = run(&programs, log, &config(SEED)).unwrap();
        assert!(matches!(lift(&journal), LiftVerdict::Conforms(_)));
        let original = bodies(&journal);
        let fault_of = |events: Vec<EventBody>| time_fault(&rebuilt(events));

        // 1. A clock that does not move forward.
        let mut events = original.clone();
        let at = position(&events, |e| matches!(e, TimeEvent::Advanced { .. })).unwrap();
        if let EventBody::Time(TimeEvent::Advanced { from, to }) = &mut events[at] {
            *to = *from;
        }
        assert!(
            matches!(fault_of(events), TimeFault::NotForward { .. }),
            "log {log}"
        );
        mutants += 1;

        // 2. A timer renumbered.
        let mut events = original.clone();
        let at = position(&events, |e| matches!(e, TimeEvent::Scheduled { .. })).unwrap();
        if let EventBody::Time(TimeEvent::Scheduled { timer, .. }) = &mut events[at] {
            timer.0 += 9;
        }
        assert!(
            matches!(fault_of(events), TimeFault::TimerIdentity { .. }),
            "log {log}"
        );
        mutants += 1;

        // 3. A scheduled event off the clock.
        let mut events = original.clone();
        let at = position(&events, |e| matches!(e, TimeEvent::Scheduled { .. })).unwrap();
        if let EventBody::Time(TimeEvent::Scheduled { at, deadline, .. }) = &mut events[at] {
            at.0 += 1;
            deadline.0 += 1;
        }
        assert!(
            matches!(
                fault_of(events),
                TimeFault::OffClock {
                    event: "scheduled",
                    ..
                }
            ),
            "log {log}"
        );
        mutants += 1;

        if let Some(fired) = position(&original, |e| matches!(e, TimeEvent::Fired { .. })) {
            let EventBody::Time(TimeEvent::Fired { timer, .. }) = original[fired] else {
                unreachable!()
            };
            let scheduled = position(
                &original,
                |e| matches!(e, TimeEvent::Scheduled { timer: t, .. } if *t == timer),
            )
            .unwrap();

            // 4. A timer that fires before its deadline.
            let mut events = original.clone();
            if let EventBody::Time(TimeEvent::Scheduled { deadline, .. }) = &mut events[scheduled] {
                deadline.0 += 1_000;
            }
            assert!(
                matches!(fault_of(events), TimeFault::Early { .. }),
                "log {log}"
            );

            // 5. A timer that fires twice.
            let mut events = original.clone();
            events.insert(fired + 1, events[fired].clone());
            assert!(
                matches!(
                    fault_of(events),
                    TimeFault::AlreadyEnded {
                        event: "fired",
                        state: "fired",
                        ..
                    }
                ),
                "log {log}"
            );

            // 6. A fire dropped: the timer is late.
            let mut events = original.clone();
            events.remove(fired);
            assert!(
                matches!(fault_of(events), TimeFault::Late { .. }),
                "log {log}"
            );
            mutants += 3;
        }

        // 7. A timer cancelled before its region's cancellation: no cancellation reached
        //    its task, whose phase is `active` (bn-36wy3 keys the check to the task's
        //    own cancellation phase, which a region's cancellation or a deadline sets).
        if let Some(cancelled) = position(&original, |e| matches!(e, TimeEvent::Cancelled { .. })) {
            let mut events = original.clone();
            let moved = events.remove(cancelled);
            let request = events
                .iter()
                .position(|b| {
                    matches!(
                        b,
                        EventBody::Lifecycle(LifecycleEvent::RegionCancelRequested { .. })
                    )
                })
                .unwrap();
            events.insert(request, moved);
            assert!(
                matches!(
                    fault_of(events),
                    TimeFault::NotCancelling {
                        state: "active",
                        ..
                    }
                ),
                "log {log}"
            );
            mutants += 1;
        }
    }
    assert!(mutants > 5000, "{mutants} mutants");
}

// --- refusals ------------------------------------------------------------------------

fn refusal(programs: Vec<Program>, config: &BindingConfig) -> BindingRefusal {
    let n: usize = programs.iter().map(Vec::len).sum();
    run(&programs, &ChoiceLog::new(vec![0; n]), config).unwrap_err()
}

#[test]
fn refusals_are_typed() {
    let a = TaskLabel(1);
    let started = || vec![spawn(RegionLabel::ROOT, a), begin(a)];
    let with = |tail: Vec<SubstrateOp>| {
        let mut program = started();
        program.extend(tail);
        vec![program]
    };

    // Malformed programs: no INV-008 reading.
    let malformed = [
        (
            with(vec![sleep(a, 0)]),
            BindingRefusal::ZeroDuration { operation: "sleep" },
        ),
        (
            with(vec![advance(0)]),
            BindingRefusal::ZeroDuration {
                operation: "advance",
            },
        ),
        (
            with(vec![sleep(a, 10), SubstrateOp::Continue { task: a }]),
            BindingRefusal::TaskAsleep { task: 0 },
        ),
        (
            with(vec![sleep(a, 10), SubstrateOp::Finish { task: a }]),
            BindingRefusal::TaskAsleep { task: 0 },
        ),
    ];
    for (programs, expected) in malformed {
        let got = refusal(programs, &config(SEED));
        assert_eq!(got, expected);
        assert_eq!(got.inconclusive_reason(), None, "{got}");
    }
    // After the timer fires, the same task takes commands again.
    let woken = with(vec![
        sleep(a, 10),
        advance(10),
        SubstrateOp::Finish { task: a },
    ]);
    let n = woken[0].len();
    assert!(run(&woken, &ChoiceLog::new(vec![0; n]), &config(SEED)).is_ok());

    // Regression guard (bn-iey9f): a sleep sent to a task that has not begun used to
    // wait unseen in its gate, so the step was lost telemetry. The binding now refuses
    // the command before it is sent: the task is not polling its gate.
    let got = refusal(
        vec![vec![spawn(RegionLabel::ROOT, a), sleep(a, 10)]],
        &config(SEED),
    );
    assert_eq!(got, BindingRefusal::TaskNotBegun { task: 0 });
    assert_eq!(got.inconclusive_reason(), None);

    // The INV-008 reading of the batch-order refusal no bound program reaches today.
    assert_eq!(
        BindingRefusal::UnorderedDuringAdvance {
            kind: "x".to_owned()
        }
        .inconclusive_reason(),
        Some(InconclusiveReason::Unsupported)
    );

    // Every family is bound since PR-14-IMPL-06 (bn-3xx9): observing the channel family
    // too is accepted.
    let started_only = with(vec![]);
    let n = started_only[0].len();
    assert!(
        run(
            &started_only,
            &ChoiceLog::new(vec![0; n]),
            &config(SEED).observing(Family::Channel)
        )
        .is_ok()
    );

    // Bytes: an unknown event tag is refused, not guessed.
    let mut journal = Journal::new();
    journal
        .append(EventBody::Time(TimeEvent::Fired {
            timer: TimerOrdinal(0),
            at: VirtualInstant(5),
        }))
        .unwrap();
    let mut bad = journal.encode().unwrap();
    let tag_at = bad.len() - 13;
    bad[tag_at] = 9;
    assert!(matches!(
        Journal::decode(&bad),
        Err(DecodeError::UnknownTag {
            table: "time event",
            tag: 9,
            ..
        })
    ));
}

/// INV-005: time reaches the binding only as the lab's virtual clock. Neither the family
/// nor the binding names a host clock: no `SystemTime`, no `Instant::now`, no wall-clock
/// helper of asupersync's, and no `std::time` import in the family.
#[test]
fn no_host_clock_is_named() {
    let code = |source: &str| -> String {
        source
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let family = code(include_str!("../src/family/time.rs"));
    let binding = code(include_str!("../src/binding.rs"));
    for (name, source) in [("time.rs", &family), ("binding.rs", &binding)] {
        for host in [
            "SystemTime",
            "Instant::now",
            "std::time::Instant",
            "wall_now",
            "wall_clock",
            "UNIX_EPOCH",
            "chrono",
        ] {
            assert!(!source.contains(host), "{name} names {host}");
        }
    }
    for ambient in ["std::time", "std::fs", "std::env", "std::thread"] {
        assert!(!family.contains(ambient), "time.rs names {ambient}");
    }
    // The scan sees code: the binding does read the virtual clock.
    assert!(binding.contains("self.lab.now()"));
    assert!(binding.contains("sleep_until"));
}
