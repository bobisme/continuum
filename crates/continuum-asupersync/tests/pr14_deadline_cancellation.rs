//! Budget-deadline cancellation (bn-36wy3), observed from the real substrate.
//!
//! > **Exit:** identical controlled choice logs produce canonical identical semantic
//! > events.
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 14
//!
//! A task spawned with a finite `Budget` deadline (`SubstrateOp::SpawnWithDeadline`)
//! cancels itself at its next `Cx::checkpoint` once the lab's virtual clock has passed
//! the deadline (asupersync 0.5.0's `CancelKind::Deadline`). The substrate traces no
//! `CancelRequest` and no region cancel for it. The binding journals the task's
//! deadline (virtual time family), its own `cancel-requested` and `cancel` steps
//! (lifecycle family), and its phases with the cause `deadline` (cancellation family),
//! and the lift holds them to the region calculus's single-task cancellation (RFC 0026
//! correction 53).
//!
//! # Evidence map
//!
//! | Claim | Test |
//! |---|---|
//! | identical choice logs → byte-identical journals, every log (sibling regions, deadlines met at a gate wake and inside an advance, region cancels) | [`identical_choice_logs_give_byte_identical_deadline_journals`] |
//! | the lab seed does not reach the journal (7 seeds) | [`the_lab_seed_does_not_reach_the_deadline_journal`] |
//! | substrate journal ≡ an independent scripted account, every log (differential) | [`the_substrate_agrees_byte_for_byte_with_a_scripted_account`] |
//! | dropping a family gives that projection byte for byte: every eleventh log of each set (690 logs), five projections each | [`dropping_a_family_gives_that_projection_byte_for_byte`] |
//! | every log's journal (7,572), with the events of each of the 32 family subsets dropped (242,304 journals), lifts as conforming, and every finalization is total | [`every_deadline_journal_conforms_and_every_finalization_is_total`] |
//! | one pinned run, rendered | [`a_deadline_run_renders_in_canonical_order`] |
//! | mutated journals are rejected, each with its own fault | [`mutated_deadline_journals_are_rejected`] |
//! | deadlines not ahead, commands to an ended task, a deadline racing a region cancel: typed refusals | [`refusals_are_typed`] |
//! | the race is refused under all 32 family projections, with no journal (cr-3pu5cu) | [`the_race_is_refused_under_every_projection`] |
//! | a sample (every sleepers log and every seventh gate log: 1,092 logs) run by the binding under all 32 projections (34,944 runs): each journal lifts as conforming and equals the all-family journal with the other families' events dropped | [`every_journal_the_binding_returns_under_any_projection_conforms`] |
//! | every strict prefix that ends mid-protocol does not conform, every complete one does (cr-3pu5cu) | [`truncated_deadline_journals_do_not_conform_and_complete_prefixes_do`] |

#[path = "support/truncation.rs"]
mod truncation;

use std::collections::{BTreeMap, BTreeSet};

use continuum_asupersync::binding::{
    BindingConfig, BindingRefusal, Program, SubstrateOp, run, run_witnessed,
};
use continuum_asupersync::choice::ChoiceLog;
use continuum_asupersync::family::cancellation::{
    CancelCause, CancellationEvent, CancellationFault, CancellationReport,
};
use continuum_asupersync::family::effect::{
    AbortCause, EffectReport, ReservationLabel, ReservationOrdinal,
};
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

/// The configuration the tests run: every family the corpus uses observed.
fn config(seed: u64) -> BindingConfig {
    BindingConfig::new(seed)
        .observing(Family::Effect)
        .observing(Family::Cancellation)
        .observing(Family::Obligation)
        .observing(Family::Time)
}

const ROOT: RegionLabel = RegionLabel::ROOT;
const R1: RegionLabel = RegionLabel(1);
const R2: RegionLabel = RegionLabel(2);
const R3: RegionLabel = RegionLabel(3);
const T1: TaskLabel = TaskLabel(1);
const T2: TaskLabel = TaskLabel(2);
const T3: TaskLabel = TaskLabel(3);
const T4: TaskLabel = TaskLabel(4);

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

fn spawn_by(region: RegionLabel, task: TaskLabel, deadline: u64) -> SubstrateOp {
    SubstrateOp::SpawnWithDeadline {
        region,
        task,
        resumability: Resumability::Resumable,
        deadline,
    }
}

const fn begin(task: TaskLabel) -> SubstrateOp {
    SubstrateOp::Begin { task }
}

const fn advance(nanos: u64) -> SubstrateOp {
    SubstrateOp::Advance { nanos }
}

const fn sleep(task: TaskLabel, nanos: u64) -> SubstrateOp {
    SubstrateOp::Sleep { task, nanos }
}

/// Deadlines met at a gate wake. A fixed setup opens `r1` with two sibling children:
/// `t1` (deadline 50) in `r2` holds a lease and a reservation, `t2` (deadline 80) in
/// `r3`. Then the clock advances to 45 and 65; `t1` is woken (it ends by its deadline
/// when both advances came first, else it only parks) and `r2` is closed; `t2` is woken
/// and `r3` cancelled (its deadline is never reached, so the region's cancellation ends
/// it); and a root task `t3` runs to completion.
///
/// ```text
/// r1 ─┬─ r2 (t1, deadline 50: lease, reservation)
///     └─ r3 (t2, deadline 80)
/// ```
fn gate() -> (Program, Vec<Program>) {
    let setup = vec![
        open(ROOT, R1),
        open(R1, R2),
        open(R1, R3),
        spawn_by(R2, T1, 50),
        spawn_by(R3, T2, 80),
        begin(T1),
        begin(T2),
        SubstrateOp::Acquire {
            task: T1,
            reservation: ReservationLabel(1),
            kind: ObligationKind::Lease,
        },
        SubstrateOp::Reserve {
            task: T1,
            reservation: ReservationLabel(2),
        },
    ];
    let actors = vec![
        vec![advance(45), advance(20)],
        vec![
            SubstrateOp::Continue { task: T1 },
            SubstrateOp::Close { region: R2 },
        ],
        vec![
            SubstrateOp::Continue { task: T2 },
            SubstrateOp::Cancel { region: R3 },
        ],
        vec![spawn(ROOT, T3), begin(T3), SubstrateOp::Finish { task: T3 }],
    ];
    (setup, actors)
}

/// Deadlines met inside an advance. A fixed setup: `t1` (deadline 30) in `r2` sleeps 40,
/// `t2` (deadline 50) in `r3` holds a lease and sleeps 60, a root task `t4` (deadline
/// 500) sleeps 20, and a root task `t3` has no deadline. Then the clock advances to 45
/// and 65, `r1` is cancelled, and `t3` finishes. Each of `t1` and `t2` ends by its
/// deadline in the advance whose due timer wakes it, or by `r1`'s cancellation when that
/// comes first; `t4`'s timer fires in the first advance, before `t1`'s.
fn sleepers() -> (Program, Vec<Program>) {
    let setup = vec![
        open(ROOT, R1),
        open(R1, R2),
        open(R1, R3),
        spawn_by(R2, T1, 30),
        spawn_by(R3, T2, 50),
        spawn(ROOT, T3),
        spawn_by(ROOT, T4, 500),
        begin(T1),
        begin(T2),
        begin(T3),
        begin(T4),
        SubstrateOp::Acquire {
            task: T2,
            reservation: ReservationLabel(1),
            kind: ObligationKind::Lease,
        },
        sleep(T1, 40),
        sleep(T2, 60),
        sleep(T4, 20),
    ];
    let actors = vec![
        vec![advance(45), advance(20)],
        vec![SubstrateOp::Cancel { region: R1 }],
        vec![SubstrateOp::Finish { task: T3 }],
    ];
    (setup, actors)
}

fn lengths(programs: &[Program]) -> Vec<usize> {
    programs.iter().map(Vec::len).collect()
}

/// The programs (setup as actor 0) and every log that runs the setup first.
fn with_setup((setup, actors): (Program, Vec<Program>)) -> (Vec<Program>, Vec<ChoiceLog>) {
    let prefix = setup.len();
    let logs = ChoiceLog::enumerate(&lengths(&actors))
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

const GATE_LOGS: usize = 7_560; // 9! / (2!·2!·2!·3!)
const SLEEPERS_LOGS: usize = 12; // 4! / (2!·1!·1!)
const ALL_LOGS: usize = GATE_LOGS + SLEEPERS_LOGS;

/// A corpus entry: name, programs, logs.
type Entry = (&'static str, Vec<Program>, Vec<ChoiceLog>);

fn corpus() -> Vec<Entry> {
    let (gate, gate_logs) = with_setup(gate());
    let (sleepers, sleepers_logs) = with_setup(sleepers());
    assert_eq!(gate_logs.len(), GATE_LOGS);
    assert_eq!(sleepers_logs.len(), SLEEPERS_LOGS);
    vec![
        ("gate", gate, gate_logs),
        ("sleepers", sleepers, sleepers_logs),
    ]
}

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

/// Where a journal's deadline cancellations ended their task: inside an advance (the
/// last clock event before the task's `cancel` is an advance, with only that task's
/// cancellation between) or at a gate wake.
fn deadline_ends(journal: &Journal) -> (usize, usize) {
    let b = bodies(journal);
    let (mut in_advance, mut at_gate) = (0, 0);
    for (at, body) in b.iter().enumerate() {
        let EventBody::Lifecycle(LifecycleEvent::TaskStepped {
            task,
            step: TaskStep::CancelRequested,
        }) = body
        else {
            continue;
        };
        let previous = b[..at].iter().rev().find(|e| {
            !matches!(e, EventBody::Lifecycle(LifecycleEvent::TaskStepped { task: t, .. }) if t != task)
                || matches!(e, EventBody::Time(_))
        });
        if matches!(
            previous,
            Some(EventBody::Time(
                TimeEvent::Advanced { .. } | TimeEvent::Fired { .. }
            ))
        ) {
            in_advance += 1;
        } else {
            at_gate += 1;
        }
    }
    (in_advance, at_gate)
}

// --- the exit property ---------------------------------------------------------------

#[test]
fn identical_choice_logs_give_byte_identical_deadline_journals() {
    let mut compared = 0;
    let (mut in_advance, mut at_gate, mut by_region) = (0, 0, 0);
    let mut distinct = BTreeSet::new();
    for (name, programs, logs) in corpus() {
        for log in &logs {
            let first = run(&programs, log, &config(SEED)).expect("every log is a legal run");
            let second = run(&programs, log, &config(SEED)).expect("every log is a legal run");
            assert_eq!(
                first.encode().unwrap(),
                second.encode().unwrap(),
                "{name} log {log}"
            );
            distinct.insert(first.digest().unwrap());
            let (advance, gate) = deadline_ends(&first);
            in_advance += advance;
            at_gate += gate;
            by_region += bodies(&first)
                .iter()
                .filter(|b| {
                    matches!(
                        b,
                        EventBody::Cancellation(CancellationEvent::Cancelled {
                            cause: CancelCause::User | CancelCause::ParentCancelled,
                            ..
                        })
                    )
                })
                .count();
            compared += 1;
        }
    }
    assert_eq!(compared, ALL_LOGS);
    // Non-vacuity: deadlines end tasks both ways, and region cancellations still end
    // others in the same corpus; the logs give many different journals.
    assert!(
        in_advance >= 6,
        "{in_advance} deadline ends inside an advance"
    );
    assert!(at_gate > 300, "{at_gate} deadline ends at a gate wake");
    assert!(by_region > 1_000, "{by_region} region cancellations");
    assert!(distinct.len() > 100, "{} distinct journals", distinct.len());
}

#[test]
fn the_lab_seed_does_not_reach_the_deadline_journal() {
    let mut compared = 0;
    for (name, programs, logs) in corpus() {
        let sample = logs
            .iter()
            .enumerate()
            .filter(|(index, _)| index % 7 == 0 || *index + 1 == logs.len() || name == "sleepers")
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
                compared += 1;
            }
        }
    }
    assert!(compared > 6_000, "the relation ran over {compared} pairs");
}

// --- the differential ----------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Gate {
    Created,
    Running,
    Suspended,
}

/// The test's own account of a run: the region tree, the tasks with their gate phase,
/// deadline and held reservations, the virtual clock, each task's timer, and the
/// obligations. It calls nothing in the adapter or in the substrate. Its rules are the
/// binding's documented observations: a task observes a passed deadline at its next
/// poll, before anything else that poll does; its cleanup aborts every reservation and
/// obligation it holds and drops its timer; and its region stays as it was.
#[derive(Default)]
struct Account {
    regions: BTreeMap<RegionLabel, u32>,
    region_labels: Vec<RegionLabel>,
    parents: Vec<Option<u32>>,
    finalized: Vec<bool>,
    closing: Vec<bool>,
    tasks: BTreeMap<TaskLabel, u32>,
    task_labels: Vec<TaskLabel>,
    task_region: Vec<u32>,
    live: Vec<bool>,
    gate: Vec<Gate>,
    deadline: Vec<Option<u64>>,
    now: u64,
    next_timer: u32,
    asleep: BTreeMap<u32, (u64, u32)>,
    /// Obligations: (holder, region, open, reservation ordinal if a transaction).
    obligations: Vec<(u32, u32, bool, Option<u32>)>,
    next_reservation: u32,
    script: Script,
}

impl Account {
    fn new() -> Self {
        let mut account = Self::default();
        account.regions.insert(ROOT, 0);
        account.region_labels.push(ROOT);
        account.parents.push(None);
        account.finalized.push(false);
        account.closing.push(false);
        account
    }

    fn lc(&mut self, report: LifecycleReport) {
        self.script.push(Report::Lifecycle(report));
    }

    fn time(&mut self, event: TimeEvent) {
        self.script.push(Report::Time(TimeReport(event)));
    }

    fn obligation(&mut self, event: ObligationEvent) {
        self.script
            .push(Report::Obligation(ObligationReport(event)));
    }

    fn step(&mut self, task: u32, step: TaskStep) {
        let task = self.task_labels[task as usize];
        self.lc(LifecycleReport::Step { task, step });
    }

    fn held(&self, task: u32) -> usize {
        self.obligations
            .iter()
            .filter(|(holder, _, open, reservation)| {
                *holder == task && *open && reservation.is_some()
            })
            .count()
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

    fn due(&self, task: u32) -> bool {
        self.deadline[task as usize].is_some_and(|deadline| deadline <= self.now)
    }

    /// A poll of `task`: its deadline when it has passed, otherwise the gate's marks.
    fn poll(&mut self, task: u32, park: bool) {
        if self.due(task) {
            self.deadline_cancel(task);
            return;
        }
        match self.gate[task as usize] {
            Gate::Created => self.step(task, TaskStep::Begin),
            Gate::Suspended => self.step(task, TaskStep::Resume),
            Gate::Running => {}
        }
        self.gate[task as usize] = Gate::Running;
        if park && self.held(task) == 0 {
            self.step(task, TaskStep::Suspend);
            self.gate[task as usize] = Gate::Suspended;
        }
    }

    /// The cleanup a cancellation's acknowledgement starts, in the journal's order: the
    /// dropped timer, the reservations aborted for cancel, then every obligation the task
    /// held, discharged as aborted.
    fn cleanup(&mut self, task: u32) {
        if let Some((_, timer)) = self.asleep.remove(&task) {
            self.time(TimeEvent::Cancelled {
                timer: TimerOrdinal(timer),
                at: VirtualInstant(self.now),
            });
        }
        let held: Vec<usize> = (0..self.obligations.len())
            .filter(|i| self.obligations[*i].0 == task && self.obligations[*i].2)
            .collect();
        for index in &held {
            if let Some(reservation) = self.obligations[*index].3 {
                self.script.push(Report::Effect(EffectReport::Abort {
                    reservation: ReservationOrdinal(reservation),
                    cause: AbortCause::Cancel,
                }));
            }
        }
        for index in held {
            self.obligations[index].2 = false;
            self.obligation(ObligationEvent::Discharged {
                obligation: ObligationOrdinal(u32::try_from(index).unwrap()),
                how: Discharge::Aborted,
            });
        }
    }

    fn deadline_cancel(&mut self, task: u32) {
        self.step(task, TaskStep::CancelRequested);
        let ordinal = TaskOrdinal(task);
        self.script
            .push(Report::Cancellation(CancellationReport::Request {
                task: ordinal,
                cause: CancelCause::Deadline,
            }));
        self.script
            .push(Report::Cancellation(CancellationReport::Acknowledge {
                task: ordinal,
            }));
        self.cleanup(task);
        self.script
            .push(Report::Cancellation(CancellationReport::Complete {
                task: ordinal,
                cause: CancelCause::Deadline,
            }));
        self.step(task, TaskStep::Cancel);
        self.live[task as usize] = false;
        self.close_if_done();
    }

    /// A closing region whose subtree holds no live task closes: drained, finalized and
    /// settled, children first.
    fn close_if_done(&mut self) {
        for region in 0..self.parents.len() {
            let region = u32::try_from(region).unwrap();
            if !self.closing[region as usize] || self.finalized[region as usize] {
                continue;
            }
            let busy = (0..self.live.len())
                .any(|t| self.live[t] && self.in_subtree(self.task_region[t], region));
            if !busy {
                self.teardown(region, &BTreeMap::new());
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn apply(&mut self, op: &SubstrateOp) {
        match op {
            SubstrateOp::OpenRegion { parent, child } => {
                let ordinal = u32::try_from(self.parents.len()).unwrap();
                self.parents.push(Some(self.regions[parent]));
                self.finalized.push(false);
                self.closing.push(false);
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
            }
            | SubstrateOp::SpawnWithDeadline {
                region,
                task,
                resumability,
                ..
            } => {
                let ordinal = u32::try_from(self.live.len()).unwrap();
                self.tasks.insert(*task, ordinal);
                self.task_labels.push(*task);
                self.task_region.push(self.regions[region]);
                self.live.push(true);
                self.gate.push(Gate::Created);
                let deadline = match op {
                    SubstrateOp::SpawnWithDeadline { deadline, .. } => Some(*deadline),
                    _ => None,
                };
                self.deadline.push(deadline);
                self.lc(LifecycleReport::Spawn {
                    region: *region,
                    task: *task,
                    resumability: resumability.clone(),
                });
                if let Some(at) = deadline {
                    self.time(TimeEvent::Deadline {
                        task: TaskOrdinal(ordinal),
                        at: VirtualInstant(at),
                    });
                }
            }
            SubstrateOp::Begin { task } | SubstrateOp::Continue { task } => {
                let task = self.tasks[task];
                self.poll(task, true);
            }
            SubstrateOp::Finish { task } => {
                let task = self.tasks[task];
                if self.due(task) {
                    self.deadline_cancel(task);
                } else {
                    self.poll(task, false);
                    self.step(task, TaskStep::Complete);
                    self.live[task as usize] = false;
                    self.close_if_done();
                }
            }
            SubstrateOp::Acquire { task, kind, .. } => {
                let task = self.tasks[task];
                self.poll(task, true);
                let obligation = u32::try_from(self.obligations.len()).unwrap();
                let region = self.task_region[task as usize];
                self.obligations.push((task, region, true, None));
                self.obligation(ObligationEvent::Opened {
                    obligation: ObligationOrdinal(obligation),
                    kind: *kind,
                    holder: TaskOrdinal(task),
                    region: RegionOrdinal(region),
                });
            }
            SubstrateOp::Reserve { task, .. } => {
                let task = self.tasks[task];
                self.poll(task, false);
                let reservation = self.next_reservation;
                self.next_reservation += 1;
                let obligation = u32::try_from(self.obligations.len()).unwrap();
                let region = self.task_region[task as usize];
                self.obligations
                    .push((task, region, true, Some(reservation)));
                self.script.push(Report::Effect(EffectReport::Reserve {
                    reservation: ReservationOrdinal(reservation),
                    task: TaskOrdinal(task),
                }));
                self.obligation(ObligationEvent::Opened {
                    obligation: ObligationOrdinal(obligation),
                    kind: ObligationKind::Transaction,
                    holder: TaskOrdinal(task),
                    region: RegionOrdinal(region),
                });
            }
            SubstrateOp::Sleep { task, nanos } => {
                let task = self.tasks[task];
                self.poll(task, false);
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
                self.step(task, TaskStep::Suspend);
                self.gate[task as usize] = Gate::Suspended;
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
                    if self.due(task) {
                        // The checkpoint comes before the `Sleep` is polled: the timer
                        // is dropped, not fired.
                        self.deadline_cancel(task);
                    } else {
                        self.asleep.remove(&task);
                        self.time(TimeEvent::Fired {
                            timer: TimerOrdinal(timer),
                            at: VirtualInstant(self.now),
                        });
                        self.poll(task, true);
                    }
                }
            }
            SubstrateOp::Close { region } => {
                self.lc(LifecycleReport::Close { region: *region });
                let ordinal = self.regions[region];
                for member in 0..self.parents.len() {
                    if self.in_subtree(u32::try_from(member).unwrap(), ordinal) {
                        self.closing[member] = true;
                    }
                }
                self.close_if_done();
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
            other => unreachable!("the deadline corpus has no {other:?}"),
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
                self.cleanup(task);
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
                    .filter(|(_, (_, r, open, _))| *r == member && *open)
                    .map(|(i, _)| ObligationOrdinal(u32::try_from(i).unwrap())),
            );
            self.obligation(ObligationEvent::RegionSettled {
                region: RegionOrdinal(member),
                open,
                leaked: ObligationSet::default(),
                fenced: ObligationSet::default(),
            });
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

// --- projections and the lift --------------------------------------------------------

fn without(journal: &Journal, dropped: &[Family]) -> Journal {
    rebuilt(
        bodies(journal)
            .into_iter()
            .filter(|body| !dropped.contains(&body.family()))
            .collect(),
    )
}

/// The family subsets a projection drops, each with the configuration that observes
/// exactly the rest.
fn projections() -> Vec<(Vec<Family>, BindingConfig)> {
    let all = [
        Family::Effect,
        Family::Cancellation,
        Family::Obligation,
        Family::Time,
    ];
    let mut out = Vec::new();
    for dropped in [
        vec![Family::Cancellation],
        vec![Family::Time],
        vec![Family::Effect, Family::Obligation],
        vec![Family::Cancellation, Family::Time],
        all.to_vec(),
    ] {
        let mut config = BindingConfig::new(SEED);
        for family in all {
            if !dropped.contains(&family) {
                config = config.observing(family);
            }
        }
        out.push((dropped, config));
    }
    out
}

/// Every subset of the five families other than lifecycle, as the families a
/// projection drops: 32 subsets, the empty one first.
fn dropped_subsets() -> Vec<Vec<Family>> {
    let others = [
        Family::Effect,
        Family::Cancellation,
        Family::Obligation,
        Family::Time,
        Family::Channel,
    ];
    (0_u32..32)
        .map(|mask| {
            others
                .iter()
                .enumerate()
                .filter(|(bit, _)| mask & (1 << bit) != 0)
                .map(|(_, family)| *family)
                .collect()
        })
        .collect()
}

#[test]
fn dropping_a_family_gives_that_projection_byte_for_byte() {
    let mut compared = 0;
    for (name, programs, logs) in corpus() {
        for log in logs.iter().step_by(11) {
            let full = run(&programs, log, &config(SEED)).unwrap();
            for (dropped, projected) in projections() {
                let observed = run(&programs, log, &projected).unwrap();
                assert_eq!(
                    without(&full, &dropped).encode().unwrap(),
                    observed.encode().unwrap(),
                    "{name} log {log} without {dropped:?}"
                );
                compared += 1;
            }
        }
    }
    assert_eq!(
        compared,
        (GATE_LOGS.div_ceil(11) + SLEEPERS_LOGS.div_ceil(11)) * 5
    );
}

#[test]
fn every_deadline_journal_conforms_and_every_finalization_is_total() {
    let mut journals = 0;
    let mut variants = 0;
    let mut finalizations = 0;
    let mut child_finalizations = 0;
    for (name, programs, logs) in corpus() {
        for log in &logs {
            let journal = run(&programs, log, &config(SEED)).unwrap();
            // The journal under every one of the 32 family subsets, by dropping events
            // (`without`): the full journal is the subset that drops nothing.
            for dropped in dropped_subsets() {
                let variant = without(&journal, &dropped);
                let LiftVerdict::Conforms(lifted) = lift(&variant) else {
                    panic!(
                        "{name} log {log}: {:?}\n{}",
                        lift(&variant),
                        variant.render()
                    );
                };
                for (_, finalization) in lifted.finalizations() {
                    // RFC 0026 correction 53: each finalization is judged by its own
                    // subtree's ledger, so a child region's teardown is total too.
                    assert!(
                        finalization.is_total(),
                        "{name} log {log}: {}",
                        finalization.render()
                    );
                    finalizations += 1;
                    if finalization.region().ordinal() != 0 {
                        child_finalizations += 1;
                    }
                }
                variants += 1;
            }
            journals += 1;
        }
    }
    assert_eq!(journals, ALL_LOGS);
    assert_eq!(variants, ALL_LOGS * 32);
    assert!(
        child_finalizations > 1_000,
        "{child_finalizations} of {finalizations}"
    );
}

/// One run, pinned in full: `t1`'s deadline passes inside the first advance (its due
/// timer woke it), `t0`'s at the wake after the second; both regions then close around
/// them.
#[test]
fn a_deadline_run_renders_in_canonical_order() {
    let (a, b) = (T1, T2);
    let programs = vec![vec![
        open(ROOT, R1),
        spawn_by(R1, a, 50),
        spawn_by(R1, b, 30),
        begin(a),
        begin(b),
        SubstrateOp::Acquire {
            task: a,
            reservation: ReservationLabel(1),
            kind: ObligationKind::Lease,
        },
        SubstrateOp::Reserve {
            task: a,
            reservation: ReservationLabel(2),
        },
        sleep(b, 40),
        advance(45),
        advance(20),
        SubstrateOp::Continue { task: a },
        SubstrateOp::Close { region: R1 },
    ]];
    let journal = run(&programs, &ChoiceLog::new([0; 12]), &config(SEED)).unwrap();
    assert_eq!(
        journal.render(),
        "0 lifecycle region-opened r1 parent=r0
1 lifecycle task-spawned t0 region=r1 resumable
2 virtual-time deadline t0 at=50
3 lifecycle task-spawned t1 region=r1 resumable
4 virtual-time deadline t1 at=30
5 lifecycle task-stepped t0 begin
6 lifecycle task-stepped t0 suspend
7 lifecycle task-stepped t1 begin
8 lifecycle task-stepped t1 suspend
9 lifecycle task-stepped t0 resume
10 lifecycle task-stepped t0 suspend
11 obligation opened o0 lease holder=t0 region=r1
12 lifecycle task-stepped t0 resume
13 reserve-commit-abort reserved e0 task=t0
14 obligation opened o1 transaction holder=t0 region=r1
15 lifecycle task-stepped t1 resume
16 virtual-time scheduled k0 task=t1 at=0 deadline=40
17 lifecycle task-stepped t1 suspend
18 virtual-time advanced 0->45
19 lifecycle task-stepped t1 cancel-requested
20 cancellation requested t1 cause=deadline
21 cancellation acknowledged t1
22 virtual-time cancelled k0 at=45
23 cancellation cancelled t1 cause=deadline
24 lifecycle task-stepped t1 cancel
25 virtual-time advanced 45->65
26 lifecycle task-stepped t0 cancel-requested
27 cancellation requested t0 cause=deadline
28 cancellation acknowledged t0
29 reserve-commit-abort aborted e0 cause=cancel
30 obligation discharged o0 aborted
31 obligation discharged o1 aborted
32 cancellation cancelled t0 cause=deadline
33 lifecycle task-stepped t0 cancel
34 lifecycle region-close-requested r1
35 lifecycle region-drained r1 cancelled=[]
36 lifecycle region-finalized r1
37 obligation region-settled r1 open=[] leaked=[]
"
    );
    let LiftVerdict::Conforms(lifted) = lift(&journal) else {
        panic!("{:?}", lift(&journal));
    };
    assert!(lifted.finalizations().iter().all(|(_, f)| f.is_total()));
}

// --- mutants -------------------------------------------------------------------------

fn position(bodies: &[EventBody], wanted: impl Fn(&EventBody) -> bool) -> usize {
    bodies
        .iter()
        .position(wanted)
        .expect("the base journal has the event")
}

fn violation(journal: &Journal) -> Nonconformance {
    match lift(journal) {
        LiftVerdict::Violates { reason, .. } => reason,
        other => panic!("expected a violation, got {other:?}\n{}", journal.render()),
    }
}

/// A gate-set journal in which `t0` ends by its deadline at a gate wake.
fn base() -> Journal {
    let (name, programs, logs) = corpus().remove(0);
    assert_eq!(name, "gate");
    logs.iter()
        .map(|log| run(&programs, log, &config(SEED)).unwrap())
        .find(|journal| deadline_ends(journal).1 > 0)
        .expect("some log ends t0 by its deadline")
}

const fn requested_alone(e: &EventBody) -> bool {
    matches!(
        e,
        EventBody::Lifecycle(LifecycleEvent::TaskStepped {
            step: TaskStep::CancelRequested,
            ..
        })
    )
}

#[test]
fn mutated_deadline_journals_are_rejected() {
    let original = bodies(&base());
    assert!(matches!(
        lift(&rebuilt(original.clone())),
        LiftVerdict::Conforms(_)
    ));
    let request = position(&original, requested_alone);
    let EventBody::Lifecycle(LifecycleEvent::TaskStepped { task, .. }) = original[request] else {
        unreachable!()
    };

    // 1. The deadline was later than the clock at the request.
    let mut events = original.clone();
    let at = position(
        &events,
        |e| matches!(e, EventBody::Time(TimeEvent::Deadline { task: t, .. }) if *t == task),
    );
    events[at] = EventBody::Time(TimeEvent::Deadline {
        task,
        at: VirtualInstant(1_000),
    });
    assert!(matches!(
        violation(&rebuilt(events)),
        Nonconformance::Time(TimeFault::DeadlineNotPassed { .. })
    ));

    // 2. The task never declared a deadline.
    let mut events = original.clone();
    events.remove(position(
        &events,
        |e| matches!(e, EventBody::Time(TimeEvent::Deadline { task: t, .. }) if *t == task),
    ));
    assert!(matches!(
        violation(&rebuilt(events)),
        Nonconformance::Time(TimeFault::DeadlineUnknown { .. })
    ));

    // 3. The lifecycle request is lost: the phases name a request nobody made.
    let mut events = original.clone();
    events.remove(request);
    assert!(matches!(
        violation(&rebuilt(events)),
        Nonconformance::Cancellation(CancellationFault::NotRequestedAlone { .. })
    ));

    // 4. The request is made twice: inside the task's own cancellation, a second
    // `cancel-requested` is not a step of it (cr-3pu5cu round 6).
    let mut events = original.clone();
    events.insert(request, original[request].clone());
    assert!(matches!(
        violation(&rebuilt(events)),
        Nonconformance::Cancellation(CancellationFault::InterruptedOwnCancel { .. })
    ));

    // 5. The completion names a region's cause.
    let mut events = original.clone();
    let done = position(
        &events,
        |e| matches!(e, EventBody::Cancellation(CancellationEvent::Cancelled { task: t, .. }) if *t == task),
    );
    events[done] = EventBody::Cancellation(CancellationEvent::Cancelled {
        task,
        cause: CancelCause::User,
    });
    assert!(matches!(
        violation(&rebuilt(events)),
        Nonconformance::Cancellation(CancellationFault::CauseMismatch { .. })
    ));

    // 6. The cleanup commits an obligation after the acknowledgement: a committed
    // discharge is not a cleanup step, so it is not a step of the task's own
    // cancellation (cr-3pu5cu round 6; the calculus's
    // `SubstrateCommitDuringCancellation` is the next guard).
    let mut events = original.clone();
    let cleanup = position(&events, |e| {
        matches!(
            e,
            EventBody::Obligation(ObligationEvent::Discharged {
                how: Discharge::Aborted,
                ..
            })
        )
    });
    if let EventBody::Obligation(ObligationEvent::Discharged { obligation, .. }) = events[cleanup] {
        events[cleanup] = EventBody::Obligation(ObligationEvent::Discharged {
            obligation,
            how: Discharge::Committed,
        });
    }
    assert!(matches!(
        violation(&rebuilt(events)),
        Nonconformance::Cancellation(CancellationFault::InterruptedOwnCancel { .. })
    ));

    // 7. The task ends before its own phases finished.
    let mut events = original.clone();
    let end = position(
        &events,
        |e| matches!(e, EventBody::Lifecycle(LifecycleEvent::TaskStepped { task: t, step: TaskStep::Cancel }) if *t == task),
    );
    let moved = events.remove(end);
    events.insert(done, moved);
    assert!(matches!(
        violation(&rebuilt(events)),
        Nonconformance::Cancellation(CancellationFault::OutOfOrder { .. })
    ));

    // 8. The task steps on after it acknowledged: only its own `cancel` is a lifecycle
    // step inside its own cancellation (cr-3pu5cu round 6).
    let mut events = original.clone();
    let ack = position(
        &events,
        |e| matches!(e, EventBody::Cancellation(CancellationEvent::Acknowledged { task: t }) if *t == task),
    );
    events.insert(
        ack + 1,
        EventBody::Lifecycle(LifecycleEvent::TaskStepped {
            task,
            step: TaskStep::Suspend,
        }),
    );
    assert!(matches!(
        violation(&rebuilt(events)),
        Nonconformance::Cancellation(CancellationFault::InterruptedOwnCancel { .. })
    ));
}

// --- refusals ------------------------------------------------------------------------

fn refusal(programs: Vec<Program>) -> BindingRefusal {
    let len = programs.iter().map(Vec::len).sum();
    run(&programs, &ChoiceLog::new(vec![0; len]), &config(SEED)).unwrap_err()
}

#[test]
fn refusals_are_typed() {
    // A deadline not ahead of the clock, at the start and after the clock moved.
    assert_eq!(
        refusal(vec![vec![spawn_by(ROOT, T1, 0)]]),
        BindingRefusal::DeadlineNotAhead {
            deadline: 0,
            now: 0
        }
    );
    let late = refusal(vec![vec![advance(45), spawn_by(ROOT, T1, 40)]]);
    assert_eq!(
        late,
        BindingRefusal::DeadlineNotAhead {
            deadline: 40,
            now: 45
        }
    );
    assert_eq!(
        late.inconclusive_reason(),
        None,
        "a malformed program, not a run"
    );

    // A command to a task its deadline already ended.
    assert_eq!(
        refusal(vec![vec![
            spawn_by(ROOT, T1, 10),
            begin(T1),
            advance(20),
            SubstrateOp::Continue { task: T1 },
            SubstrateOp::Continue { task: T1 },
        ]]),
        BindingRefusal::TaskEnded { task: 0 }
    );

    // A region cancellation that reaches a task whose deadline passed and was not yet
    // observed. asupersync 0.5.0 keeps the more severe reason (`CancelKind::severity`:
    // user 0 < deadline 1 < parent-cancelled 4). For the task's own region's
    // cancellation the completion's reason becomes `deadline` while its request was a
    // region's: two causes for one task, so the run is not bound.
    let own_region = |region: RegionLabel, parent: Option<RegionLabel>| {
        let mut program = vec![open(ROOT, R1)];
        if let Some(parent) = parent {
            program[0] = open(ROOT, parent);
            program.push(open(parent, region));
        }
        program.extend([
            spawn_by(region, T1, 10),
            begin(T1),
            advance(20),
            SubstrateOp::Cancel {
                region: parent.unwrap_or(region),
            },
        ]);
        let len = program.len();
        run(&[program], &ChoiceLog::new(vec![0; len]), &config(SEED))
    };
    let raced = own_region(R1, None).unwrap_err();
    assert_eq!(raced, BindingRefusal::CancelRaced { task: 0 });
    assert_eq!(
        raced.inconclusive_reason(),
        Some(InconclusiveReason::Unsupported)
    );
    // A parent region's cancellation outranks the deadline: an ordinary region
    // cancellation, which lifts as one.
    let parent = own_region(R2, Some(R1)).expect("the parent's reason is kept");
    assert!(
        matches!(lift(&parent), LiftVerdict::Conforms(_)),
        "{}",
        parent.render()
    );
    assert!(bodies(&parent).iter().any(|b| matches!(
        b,
        EventBody::Cancellation(CancellationEvent::Cancelled {
            cause: CancelCause::ParentCancelled,
            ..
        })
    )));
}

// --- projections: substrate facts do not depend on what a run observes (cr-3pu5cu) ---

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

/// The race of a task's own region cancellation with its unobserved deadline is refused
/// under every projection, lifecycle alone included, and never returns a journal. The
/// region's request is substrate state the refusal depends on, so no projection may lose
/// it (cr-3pu5cu: with the cancellation family projected out, the binding once journaled
/// a single-task cancellation after the region's request, which its own lift refuses).
#[test]
fn the_race_is_refused_under_every_projection() {
    let programs = vec![vec![
        open(ROOT, R1),
        spawn_by(R1, T1, 10),
        begin(T1),
        advance(20),
        SubstrateOp::Cancel { region: R1 },
    ]];
    let projections = every_projection();
    assert_eq!(projections.len(), 32);
    for config in &projections {
        assert_eq!(
            run(&programs, &ChoiceLog::new([0; 5]), config),
            Err(BindingRefusal::CancelRaced { task: 0 }),
            "{:?}",
            config.families
        );
    }
}

/// Over a sample of the deadline corpus (every sleepers log and every seventh gate log:
/// 12 + 1,080 = 1,092 logs) and every one of the 32 projections (34,944 runs), each run
/// the binding completes returns a journal its own lift accepts, byte-identical to the
/// all-family run of the same log with the other families' events dropped.
/// `every_deadline_journal_conforms_and_every_finalization_is_total` lifts every log's
/// journal under all 32 dropped subsets; this test ties that filtering to the binding's
/// own projected runs on the sample. The full cross-product (242,304 runs) costs about
/// 160 s of an unoptimised test build, so it is sampled.
#[test]
fn every_journal_the_binding_returns_under_any_projection_conforms() {
    let others = [
        Family::Effect,
        Family::Cancellation,
        Family::Obligation,
        Family::Time,
        Family::Channel,
    ];
    let mut checked = 0_usize;
    for (name, programs, logs) in corpus() {
        let step = if name == "gate" { 7 } else { 1 };
        for log in logs.iter().step_by(step) {
            let projections = every_projection();
            let full = run(&programs, log, &projections[31])
                .unwrap_or_else(|refusal| panic!("{name} log {log}: {refusal}"));
            for config in projections {
                let journal = run(&programs, log, &config)
                    .unwrap_or_else(|refusal| panic!("{name} log {log}: {refusal}"));
                assert!(
                    matches!(lift(&journal), LiftVerdict::Conforms(_)),
                    "{name} log {log} {:?}: {:?}\n{}",
                    config.families,
                    lift(&journal),
                    journal.render()
                );
                let dropped: Vec<Family> = others
                    .into_iter()
                    .filter(|family| !config.families.contains(*family))
                    .collect();
                assert_eq!(
                    without(&full, &dropped).encode().unwrap(),
                    journal.encode().unwrap(),
                    "{name} log {log} without {dropped:?}"
                );
                checked += 1;
            }
        }
    }
    assert_eq!(checked, (GATE_LOGS.div_ceil(7) + SLEEPERS_LOGS) * 32);
    assert_eq!(checked, 34_944);
}

/// Every strict prefix of every deadline-corpus journal (cr-3pu5cu): a prefix that ends
/// inside a protocol (`support/truncation.rs`) never lifts as `Conforms`, and a complete
/// prefix always does.
#[test]
fn truncated_deadline_journals_do_not_conform_and_complete_prefixes_do() {
    let (mut mid, mut complete) = (0_usize, 0_usize);
    for (name, programs, logs) in corpus() {
        for log in &logs {
            let all = bodies(&run(&programs, log, &config(SEED)).unwrap());
            for cut in 0..all.len() {
                let prefix = rebuilt(all[..cut].to_vec());
                let conforms = matches!(lift(&prefix), LiftVerdict::Conforms(_));
                match truncation::mid_protocol(&all[..cut]) {
                    Some(what) => {
                        assert!(!conforms, "{name} log {log} cut {cut} ({what}) conforms");
                        // A cut between a region's finalize and its settle is a missing
                        // report, not a contradiction: it must type
                        // `Inconclusive(InsufficientTelemetry)`, never a violation
                        // (bn-eaxx0, found by the bn-28hup adversarial pass).
                        if what == "a finalize not settled" {
                            assert!(
                                matches!(
                                    lift(&prefix),
                                    LiftVerdict::Inconclusive {
                                        reason: InconclusiveReason::InsufficientTelemetry,
                                        ..
                                    }
                                ),
                                "{name} log {log} cut {cut} ({what}) is not Inconclusive: {:?}",
                                lift(&prefix)
                            );
                        }
                        mid += 1;
                    }
                    None => {
                        assert!(
                            conforms,
                            "{name} log {log} cut {cut}: {:?}\n{}",
                            lift(&prefix),
                            prefix.render()
                        );
                        complete += 1;
                    }
                }
            }
        }
    }
    assert!(
        mid > 10_000 && complete > 10_000,
        "{mid} mid, {complete} complete"
    );
}

/// The A7 witness keeps its own copy of the complete-prefix predicate (it may load the
/// model file only); the copy is the same text as `support/truncation.rs`.
#[test]
fn the_truncation_predicate_has_one_text() {
    let normalize = |text: &str| -> String {
        let start = text
            .find("/// Whether a journal prefix ends inside")
            .expect("the predicate");
        let body = &text[start..];
        let end = body.find("\n}\n").expect("its end") + 3;
        body[..end].replace("pub fn mid_protocol(", "fn mid_protocol(")
    };
    assert_eq!(
        normalize(include_str!("support/truncation.rs")),
        normalize(include_str!("a7_primitive_conformance.rs"))
    );
}
