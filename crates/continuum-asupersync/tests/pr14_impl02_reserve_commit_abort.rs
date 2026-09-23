//! PR-14-IMPL-02 (bn-gzy1): reserve / commit / abort, observed from the real substrate.
//!
//! > **Exit:** identical controlled choice logs produce canonical identical semantic
//! > events.
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 14
//!
//! The binding (`binding.rs`) drives asupersync 0.5.0's lab runtime under a choice log.
//! Each bound task reserves two-phase effects through its own `Cx` (checked obligation
//! tokens of kind `Transaction`), and commits them, aborts them, or has them aborted by
//! its cancellation. With the reserve/commit/abort family observed, the binding journals
//! each reservation's `reserved → committed | aborted` from the substrate's own
//! `ObligationReserve` / `ObligationCommit` / `ObligationAbort` trace events. These
//! tests close the exit sentence for that family and hold it to the region calculus.
//!
//! # Evidence map
//!
//! | Claim | Test |
//! |---|---|
//! | identical choice logs → byte-identical journals, every log (sibling regions included) | [`identical_choice_logs_give_byte_identical_effect_journals`] |
//! | the lab seed does not reach the journal, though it reorders sibling closes (7 seeds) | [`the_lab_seed_does_not_reach_the_effect_journal`] |
//! | different choice logs → different journals (anti-vacuity of the identity) | [`different_choice_logs_give_different_effect_journals`] |
//! | substrate journal ≡ an independent scripted account, every log (differential) | [`the_substrate_agrees_byte_for_byte_with_a_scripted_account`] |
//! | the family adds events and changes no byte of the other families' journals | [`dropping_a_family_gives_that_projection_byte_for_byte`] |
//! | every journal lifts, every reservation resolves once, no leak at close | [`every_effect_journal_conforms_and_resolves_every_reservation_once`] |
//! | the calculus refuses what it must: a second staged effect; explicit abort is inconclusive | [`the_calculus_judges_real_runs_it_does_not_admit`] |
//! | one pinned run, rendered: aborts sit inside the cancellation phases | [`a_cancelled_holder_aborts_inside_its_cancellation_phases`] |
//! | mutated journals are rejected, each with its own fault (anti-vacuity) | [`mutated_effect_journals_are_rejected`] |
//! | malformed programs, lost effects, dropped reservations, bad bytes: typed refusals | [`refusals_are_typed`] |
//! | no ambient filesystem or environment path in the family | [`the_effect_family_names_no_ambient_path`] |

use std::collections::BTreeMap;

use continuum_asupersync::binding::{
    BindingConfig, BindingRefusal, Program, SubstrateOp, run, run_witnessed,
};
use continuum_asupersync::choice::ChoiceLog;
use continuum_asupersync::encoding::DecodeError;
use continuum_asupersync::family::cancellation::{CancelCause, CancellationReport};
use continuum_asupersync::family::effect::{
    AbortCause, EffectEvent, EffectFault, EffectReport, ReservationLabel, ReservationOrdinal,
};
use continuum_asupersync::family::lifecycle::{
    LifecycleEvent, LifecycleReport, RegionLabel, TaskLabel, TaskOrdinal, TaskStep,
};
use continuum_asupersync::family::{EventBody, Family, Report};
use continuum_asupersync::journal::Journal;
use continuum_asupersync::lift::{LiftVerdict, Nonconformance, lift};
use continuum_asupersync::source::{Script, record};
use continuum_task::region::RegionFault;
use continuum_task::region::worker::Resumability;
use continuum_value::assurance::InconclusiveReason;

const SEED: u64 = 0;
const SEEDS: [u64; 7] = [0, 1, 7, 42, 99, 0x9e37_79b9_7f4a_7c15, u64::MAX];

/// The configuration the tests run: every bound family observed.
fn config(seed: u64) -> BindingConfig {
    BindingConfig::new(seed)
        .observing(Family::Effect)
        .observing(Family::Cancellation)
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

fn reserve(task: TaskLabel, reservation: u32) -> SubstrateOp {
    SubstrateOp::Reserve {
        task,
        reservation: ReservationLabel(reservation),
    }
}

fn commit(reservation: u32) -> SubstrateOp {
    SubstrateOp::Commit {
        reservation: ReservationLabel(reservation),
    }
}

fn begin(task: TaskLabel) -> SubstrateOp {
    SubstrateOp::Begin { task }
}

/// Two actors. A root task that reserves and commits twice. And a cancelled region with
/// two sibling child regions, each with a task that holds a reservation when the
/// cancellation arrives; the second task has committed one reservation first.
///
/// ```text
/// r1 ─┬─ r2 (t: holds e)
///     └─ r3 (t: committed e, holds e)
/// ```
///
/// The substrate's seeded scheduler picks the order in which `r2` and `r3` close and in
/// which the two holders are polled. Every interleaving is a legal run.
fn siblings() -> Vec<Program> {
    let (a, b, c) = (TaskLabel(1), TaskLabel(2), TaskLabel(3));
    let (r1, r2, r3) = (RegionLabel(1), RegionLabel(2), RegionLabel(3));
    vec![
        vec![
            spawn(RegionLabel::ROOT, a),
            begin(a),
            reserve(a, 1),
            commit(1),
            reserve(a, 2),
            commit(2),
        ],
        vec![
            open(RegionLabel::ROOT, r1),
            open(r1, r2),
            open(r1, r3),
            spawn(r2, b),
            spawn(r3, c),
            begin(b),
            begin(c),
            reserve(b, 3),
            reserve(c, 4),
            commit(4),
            reserve(c, 5),
            SubstrateOp::Cancel { region: r1 },
        ],
    ]
}

/// Two actors: a region whose task commits and finishes before a normal close, and a
/// root task that commits.
fn closing() -> Vec<Program> {
    let (a, b) = (TaskLabel(1), TaskLabel(2));
    let r1 = RegionLabel(1);
    vec![
        vec![
            open(RegionLabel::ROOT, r1),
            spawn(r1, a),
            begin(a),
            reserve(a, 1),
            commit(1),
            SubstrateOp::Finish { task: a },
            SubstrateOp::Close { region: r1 },
        ],
        vec![
            spawn(RegionLabel::ROOT, b),
            begin(b),
            reserve(b, 2),
            commit(2),
        ],
    ]
}

fn lengths(programs: &[Program]) -> Vec<usize> {
    programs.iter().map(Vec::len).collect()
}

const SIBLINGS_LOGS: usize = 18_564; // 18! / (6!·12!)
const CLOSING_LOGS: usize = 330; // 11! / (7!·4!)
const ALL_LOGS: usize = SIBLINGS_LOGS + CLOSING_LOGS;

/// Every (program set, log) pair, with the reservations each run makes and how many of
/// them a cancellation aborts.
/// A corpus entry: name, programs, logs, reservations per run, aborted per run.
type Entry = (&'static str, Vec<Program>, Vec<ChoiceLog>, usize, usize);

fn corpus() -> Vec<Entry> {
    let siblings = siblings();
    let closing = closing();
    let siblings_logs = ChoiceLog::enumerate(&lengths(&siblings));
    let closing_logs = ChoiceLog::enumerate(&lengths(&closing));
    assert_eq!(siblings_logs.len(), SIBLINGS_LOGS);
    assert_eq!(closing_logs.len(), CLOSING_LOGS);
    vec![
        ("siblings", siblings, siblings_logs, 5, 2),
        ("closing", closing, closing_logs, 2, 0),
    ]
}

fn effect_events(journal: &Journal) -> Vec<&EffectEvent> {
    journal
        .events()
        .iter()
        .filter_map(|event| match event.body() {
            EventBody::Effect(event) => Some(event),
            _ => None,
        })
        .collect()
}

// --- the exit property ---------------------------------------------------------------

#[test]
fn identical_choice_logs_give_byte_identical_effect_journals() {
    let mut compared = 0;
    for (name, programs, logs, reservations, aborted) in corpus() {
        for log in &logs {
            let first = run(&programs, log, &config(SEED)).expect("every log is a legal run");
            let second = run(&programs, log, &config(SEED)).expect("every log is a legal run");
            assert_eq!(
                first.encode().unwrap(),
                second.encode().unwrap(),
                "{name} log {log}"
            );
            assert_eq!(first.digest().unwrap(), second.digest().unwrap());
            // Non-vacuity: a reserve and one resolution for every reservation.
            let events = effect_events(&first);
            assert_eq!(events.len(), 2 * reservations, "{name} log {log}");
            let aborts = events
                .iter()
                .filter(|e| matches!(e, EffectEvent::Aborted { .. }))
                .count();
            assert_eq!(aborts, aborted, "{name} log {log}:\n{}", first.render());
            compared += 1;
        }
    }
    assert_eq!(compared, ALL_LOGS);
}

/// Metamorphic relation: the lab seed moves the substrate's own close order of the
/// sibling regions and the poll order of the two holders, and the journal does not
/// change.
#[test]
fn the_lab_seed_does_not_reach_the_effect_journal() {
    let mut compared = 0;
    let mut reordered = 0;
    for (name, programs, logs, _, _) in corpus() {
        let sample = logs
            .iter()
            .enumerate()
            .filter(|(index, _)| index % 5 == 0 || *index + 1 == logs.len())
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
                if reference.substrate_close_order != other.substrate_close_order {
                    reordered += 1;
                }
                compared += 1;
            }
        }
    }
    assert!(compared > 20_000, "the relation ran over {compared} pairs");
    // Anti-vacuity: the seed does reorder what the journal canonicalizes.
    assert!(reordered > 0, "no seed moved the substrate's close order");
}

#[test]
fn different_choice_logs_give_different_effect_journals() {
    for (name, programs, logs, _, _) in corpus() {
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

/// The test's own account of a run: labels, ordinals, the region tree, which tasks are
/// live, what each task holds, and the gate's phase. It calls nothing in the adapter or
/// in the substrate, so the scripted journal it produces is a second path to the same
/// bytes.
#[derive(Default)]
struct Account {
    regions: BTreeMap<RegionLabel, u32>,
    region_labels: Vec<RegionLabel>,
    parents: Vec<Option<u32>>,
    finalized: Vec<bool>,
    tasks: BTreeMap<TaskLabel, u32>,
    task_region: Vec<u32>,
    live: Vec<bool>,
    gate: Vec<Gate>,
    held: Vec<Vec<u32>>,
    reservations: BTreeMap<ReservationLabel, (u32, u32)>,
    next_reservation: u32,
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

    fn step(&mut self, task: TaskLabel, step: TaskStep) {
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

    fn label_of(&self, task: u32) -> TaskLabel {
        *self.tasks.iter().find(|(_, t)| **t == task).unwrap().0
    }

    /// Wake a task for a command: `begin` or `resume` as the gate's phase says.
    fn wake(&mut self, task: u32) {
        let label = self.label_of(task);
        match self.gate[task as usize] {
            Gate::Created => self.step(label, TaskStep::Begin),
            Gate::Suspended => self.step(label, TaskStep::Resume),
            Gate::Running => {}
        }
        self.gate[task as usize] = Gate::Running;
    }

    /// Park a task after a command: `suspend` only when it holds nothing.
    fn park(&mut self, task: u32) {
        if self.held[task as usize].is_empty() {
            let label = self.label_of(task);
            self.step(label, TaskStep::Suspend);
            self.gate[task as usize] = Gate::Suspended;
        }
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
                self.task_region.push(self.regions[region]);
                self.live.push(true);
                self.gate.push(Gate::Created);
                self.held.push(Vec::new());
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
            SubstrateOp::Continue { task } => {
                let task = self.tasks[task];
                self.wake(task);
                self.park(task);
            }
            SubstrateOp::Finish { task } => {
                let ordinal = self.tasks[task];
                self.wake(ordinal);
                self.step(*task, TaskStep::Complete);
                self.live[ordinal as usize] = false;
            }
            SubstrateOp::Reserve { task, reservation } => {
                let task = self.tasks[task];
                self.wake(task);
                let ordinal = self.next_reservation;
                self.next_reservation += 1;
                self.reservations.insert(*reservation, (ordinal, task));
                self.held[task as usize].push(ordinal);
                self.script.push(Report::Effect(EffectReport::Reserve {
                    reservation: ReservationOrdinal(ordinal),
                    task: TaskOrdinal(task),
                }));
                self.park(task);
            }
            SubstrateOp::Commit { reservation } | SubstrateOp::Abort { reservation } => {
                let (ordinal, task) = self.reservations[reservation];
                self.wake(task);
                self.held[task as usize].retain(|r| *r != ordinal);
                self.script.push(Report::Effect(match op {
                    SubstrateOp::Commit { .. } => EffectReport::Commit {
                        reservation: ReservationOrdinal(ordinal),
                    },
                    _ => EffectReport::Abort {
                        reservation: ReservationOrdinal(ordinal),
                        cause: AbortCause::Explicit,
                    },
                }));
                self.park(task);
            }
            SubstrateOp::Close { region } => {
                self.lc(LifecycleReport::Close { region: *region });
                let ordinal = self.regions[region];
                self.teardown(ordinal, &BTreeMap::new());
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
        }
    }

    /// Children first, siblings ascending; in each region, each cancelled task's
    /// acknowledgement, aborts and completion, then the drain and finalize.
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
                let mut held = core::mem::take(&mut self.held[task as usize]);
                held.sort_unstable();
                for reservation in held {
                    self.script.push(Report::Effect(EffectReport::Abort {
                        reservation: ReservationOrdinal(reservation),
                        cause: AbortCause::Cancel,
                    }));
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
    for (name, programs, logs, _, _) in corpus() {
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

/// The family is additive: remove a family's events from the full journal and what
/// remains is, byte for byte, the journal of a run that does not observe that family.
/// So the IMPL-01 and IMPL-03 journals are unchanged by this bone.
#[test]
fn dropping_a_family_gives_that_projection_byte_for_byte() {
    for (name, programs, logs, _, _) in corpus() {
        for log in logs.iter().step_by(13) {
            let full = run(&programs, log, &config(SEED)).unwrap();
            let projections = [
                (
                    vec![Family::Effect],
                    BindingConfig::new(SEED).observing(Family::Cancellation),
                ),
                (
                    vec![Family::Cancellation],
                    BindingConfig::new(SEED).observing(Family::Effect),
                ),
                (
                    vec![Family::Effect, Family::Cancellation],
                    BindingConfig::new(SEED),
                ),
            ];
            for (dropped, projection) in projections {
                let expected = run(&programs, log, &projection).unwrap();
                assert_eq!(
                    without(&full, &dropped).encode().unwrap(),
                    expected.encode().unwrap(),
                    "{name} log {log} without {dropped:?}"
                );
            }
        }
    }
}

// --- conformance ---------------------------------------------------------------------

#[test]
fn every_effect_journal_conforms_and_resolves_every_reservation_once() {
    for (name, programs, logs, _, _) in corpus() {
        let expected_finalizations = match name {
            "siblings" => 3,
            _ => 1,
        };
        for log in &logs {
            let journal = run(&programs, log, &config(SEED)).unwrap();
            let journal = Journal::decode(&journal.encode().unwrap()).unwrap();
            let LiftVerdict::Conforms(lifted) = lift(&journal) else {
                panic!("{name} log {log}:\n{}", journal.render());
            };
            assert_eq!(lifted.finalizations().len(), expected_finalizations);
            for (seq, finalization) in lifted.finalizations() {
                assert!(
                    finalization.orphans().is_empty()
                        && finalization.unresolved_publications().is_empty(),
                    "{name} log {log}: finalization at {seq}:\n{}",
                    finalization.render()
                );
            }
            // Each reservation: one reserve, then exactly one resolution.
            let mut resolutions: BTreeMap<u32, usize> = BTreeMap::new();
            for event in effect_events(&journal) {
                let count = resolutions.entry(event.reservation().0).or_default();
                match event {
                    EffectEvent::Reserved { .. } => assert_eq!(*count, 0),
                    _ => *count += 1,
                }
            }
            assert!(resolutions.values().all(|n| *n == 1), "{name} log {log}");
        }
    }
}

/// Real runs the calculus must not admit. A task that holds two reservations at once
/// is a second staged publication, which the calculus refuses. An explicit abort has no
/// step in the calculus, so the lift is inconclusive there, never a pass.
#[test]
fn the_calculus_judges_real_runs_it_does_not_admit() {
    let a = TaskLabel(1);
    let two_held = vec![vec![
        spawn(RegionLabel::ROOT, a),
        begin(a),
        reserve(a, 1),
        reserve(a, 2),
        commit(2),
        commit(1),
    ]];
    let journal = run(&two_held, &ChoiceLog::new([0; 6]), &config(SEED)).unwrap();
    match lift(&journal) {
        LiftVerdict::Violates {
            seq,
            reason: Nonconformance::Refused(RegionFault::ReserveOverProvisional { .. }),
        } => assert!(matches!(
            journal.events()[usize::try_from(seq).unwrap()].body(),
            EventBody::Effect(EffectEvent::Reserved { .. })
        )),
        other => panic!("{other:?}\n{}", journal.render()),
    }

    let explicit = vec![vec![
        spawn(RegionLabel::ROOT, a),
        begin(a),
        reserve(a, 1),
        SubstrateOp::Abort {
            reservation: ReservationLabel(1),
        },
    ]];
    let journal = run(&explicit, &ChoiceLog::new([0; 4]), &config(SEED)).unwrap();
    assert!(journal.events().iter().any(|e| e.body()
        == &EventBody::Effect(EffectEvent::Aborted {
            reservation: ReservationOrdinal(0),
            cause: AbortCause::Explicit,
        })));
    assert!(matches!(
        lift(&journal),
        LiftVerdict::Inconclusive {
            family: Family::Effect,
            reason: InconclusiveReason::Unsupported,
            ..
        }
    ));
}

/// One run, pinned in full. `t0` in `r1` holds `e0` when `r1` is cancelled: the abort
/// is journaled between the task's acknowledgement and its completion. The root task
/// commits `e1`, and its `suspend` follows the commit.
#[test]
fn a_cancelled_holder_aborts_inside_its_cancellation_phases() {
    let (a, b) = (TaskLabel(1), TaskLabel(2));
    let r1 = RegionLabel(1);
    let programs = vec![vec![
        open(RegionLabel::ROOT, r1),
        spawn(r1, a),
        spawn(RegionLabel::ROOT, b),
        begin(a),
        reserve(a, 1),
        begin(b),
        reserve(b, 2),
        commit(2),
        SubstrateOp::Cancel { region: r1 },
    ]];
    let journal = run(&programs, &ChoiceLog::new([0; 9]), &config(SEED)).unwrap();
    assert_eq!(
        journal.render(),
        "0 lifecycle region-opened r1 parent=r0
1 lifecycle task-spawned t0 region=r1 resumable
2 lifecycle task-spawned t1 region=r0 resumable
3 lifecycle task-stepped t0 begin
4 lifecycle task-stepped t0 suspend
5 lifecycle task-stepped t0 resume
6 reserve-commit-abort reserved e0 task=t0
7 lifecycle task-stepped t1 begin
8 lifecycle task-stepped t1 suspend
9 lifecycle task-stepped t1 resume
10 reserve-commit-abort reserved e1 task=t1
11 reserve-commit-abort committed e1
12 lifecycle task-stepped t1 suspend
13 lifecycle region-cancel-requested r1
14 cancellation requested t0 cause=user
15 cancellation acknowledged t0
16 reserve-commit-abort aborted e0 cause=cancel
17 cancellation cancelled t0 cause=user
18 lifecycle region-drained r1 cancelled=[t0]
19 lifecycle region-finalized r1
"
    );
    let bytes = journal.encode().unwrap();
    assert_eq!(Journal::decode(&bytes).unwrap(), journal);
    assert!(matches!(lift(&journal), LiftVerdict::Conforms(_)));
}

// --- anti-vacuity: the lift rejects wrong effect histories ----------------------------

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

fn position(bodies: &[EventBody], wanted: impl Fn(&EffectEvent) -> bool) -> usize {
    bodies
        .iter()
        .position(|body| matches!(body, EventBody::Effect(event) if wanted(event)))
        .expect("the journal has such an event")
}

fn effect_fault(journal: &Journal) -> EffectFault {
    match lift(journal) {
        LiftVerdict::Violates {
            reason: Nonconformance::Effect(fault),
            ..
        } => fault,
        other => panic!(
            "expected an effect fault, got {other:?}\n{}",
            journal.render()
        ),
    }
}

fn is_cancel_abort(event: &EffectEvent) -> bool {
    matches!(
        event,
        EffectEvent::Aborted {
            cause: AbortCause::Cancel,
            ..
        }
    )
}

/// Five mutation operators over real substrate journals, each rejected with its own
/// fault. The unmutated journal conforms, so each rejection is the mutant's doing.
#[test]
fn mutated_effect_journals_are_rejected() {
    let programs = siblings();
    let logs = ChoiceLog::enumerate(&lengths(&programs));
    let mut mutants = 0;
    for log in logs.iter().step_by(37) {
        let journal = run(&programs, log, &config(SEED)).unwrap();
        assert!(matches!(lift(&journal), LiftVerdict::Conforms(_)));
        let original = bodies(&journal);

        // 1. A commit resolved twice.
        let mut events = original.clone();
        let at = position(&events, |e| matches!(e, EffectEvent::Committed { .. }));
        events.insert(at + 1, events[at].clone());
        assert!(
            matches!(
                effect_fault(&rebuilt(events)),
                EffectFault::AlreadyResolved {
                    event: "committed",
                    phase: "committed",
                    ..
                }
            ),
            "log {log}"
        );

        // 2. A commit after an abort.
        let mut events = original.clone();
        let at = position(&events, is_cancel_abort);
        let reservation = match &events[at] {
            EventBody::Effect(event) => event.reservation(),
            _ => unreachable!(),
        };
        events.insert(
            at + 1,
            EventBody::Effect(EffectEvent::Committed { reservation }),
        );
        assert!(
            matches!(
                effect_fault(&rebuilt(events)),
                EffectFault::AlreadyResolved {
                    event: "committed",
                    phase: "aborted",
                    ..
                }
            ),
            "log {log}"
        );

        // 3. A cancellation abort journaled after the drain that discarded it.
        let mut events = original.clone();
        let at = position(&events, is_cancel_abort);
        let abort = events.remove(at);
        let drain = events[at..]
            .iter()
            .position(|b| {
                matches!(
                    b,
                    EventBody::Lifecycle(LifecycleEvent::RegionDrained { .. })
                )
            })
            .unwrap()
            + at;
        events.insert(drain + 1, abort);
        assert!(
            matches!(
                effect_fault(&rebuilt(events)),
                EffectFault::TaskTerminal {
                    state: "cancelled",
                    ..
                }
            ),
            "log {log}"
        );

        // 4. A cancellation abort dropped: the reservation leaks at region close.
        let mut events = original.clone();
        let at = position(&events, is_cancel_abort);
        let dropped = match events.remove(at) {
            EventBody::Effect(event) => event.reservation(),
            _ => unreachable!(),
        };
        assert!(
            matches!(
                effect_fault(&rebuilt(events)),
                EffectFault::LeakedAtClose { reservation, .. } if reservation == dropped.0
            ),
            "log {log}"
        );

        // 5. A reservation renumbered.
        let mut events = original.clone();
        let at = position(&events, |e| matches!(e, EffectEvent::Reserved { .. }));
        if let EventBody::Effect(EffectEvent::Reserved { reservation, .. }) = &mut events[at] {
            reservation.0 += 7;
        }
        assert!(
            matches!(
                effect_fault(&rebuilt(events)),
                EffectFault::ReservationIdentity { .. }
            ),
            "log {log}"
        );
        mutants += 5;
    }
    assert!(mutants > 2000, "{mutants} mutants");
}

// --- refusals ------------------------------------------------------------------------

fn refusal(programs: Vec<Program>) -> BindingRefusal {
    let n: usize = programs.iter().map(Vec::len).sum();
    run(&programs, &ChoiceLog::new(vec![0; n]), &config(SEED)).unwrap_err()
}

#[test]
fn refusals_are_typed() {
    let a = TaskLabel(1);
    let r1 = RegionLabel(1);
    let started = || vec![spawn(RegionLabel::ROOT, a), begin(a)];
    let with = |tail: Vec<SubstrateOp>| {
        let mut program = started();
        program.extend(tail);
        vec![program]
    };

    // Malformed programs: no INV-008 reading.
    let malformed = [
        (with(vec![commit(9)]), BindingRefusal::UnboundReservation(9)),
        (
            with(vec![reserve(a, 1), commit(1), reserve(a, 1)]),
            BindingRefusal::ReservationLabelRebound(1),
        ),
        (
            with(vec![reserve(a, 1), commit(1), commit(1)]),
            BindingRefusal::ReservationResolved(1),
        ),
    ];
    for (programs, expected) in malformed {
        let got = refusal(programs);
        assert_eq!(got, expected);
        assert_eq!(got.inconclusive_reason(), None, "{got}");
    }

    // A reservation its cancellation already aborted cannot be committed.
    let got = refusal(vec![vec![
        open(RegionLabel::ROOT, r1),
        spawn(r1, a),
        begin(a),
        reserve(a, 1),
        SubstrateOp::Cancel { region: r1 },
        commit(1),
    ]]);
    assert_eq!(got, BindingRefusal::ReservationResolved(1));

    // A reserve sent to a task that has not begun: the substrate never polls it, so
    // the trace shows no reservation. Lost telemetry, never a short journal.
    let got = refusal(vec![vec![spawn(RegionLabel::ROOT, a), reserve(a, 1)]]);
    assert_eq!(
        got,
        BindingRefusal::EffectUnobserved {
            operation: "reserve"
        }
    );
    assert_eq!(
        got.inconclusive_reason(),
        Some(InconclusiveReason::InsufficientTelemetry)
    );

    // A holder that returns with its reservation unresolved: the leak at close.
    let got = refusal(with(vec![reserve(a, 1), SubstrateOp::Finish { task: a }]));
    assert_eq!(got, BindingRefusal::ReservationDropped { reservation: 0 });
    assert_eq!(
        got.inconclusive_reason(),
        Some(InconclusiveReason::Unsupported)
    );

    // A reserve into a region whose close has begun: the substrate refuses it.
    let got = refusal(vec![vec![
        open(RegionLabel::ROOT, r1),
        spawn(r1, a),
        begin(a),
        SubstrateOp::Close { region: r1 },
        reserve(a, 1),
    ]]);
    assert!(
        matches!(
            got,
            BindingRefusal::SubstrateRefused {
                operation: "reserve",
                ..
            }
        ),
        "{got}"
    );
    assert_eq!(got.inconclusive_reason(), None);

    // A family the binding does not bind is refused before the substrate runs.
    let programs = siblings();
    let log = ChoiceLog::enumerate(&lengths(&programs)).remove(0);
    let got = run(&programs, &log, &config(SEED).observing(Family::Obligation)).unwrap_err();
    assert_eq!(got, BindingRefusal::FamilyNotBound(Family::Obligation));

    // A trace buffer too small for the run.
    let tiny = BindingConfig {
        trace_capacity: 32,
        ..config(SEED)
    };
    let got = run(&programs, &log, &tiny).unwrap_err();
    assert!(matches!(got, BindingRefusal::TraceOverflow { .. }), "{got}");

    // Bytes: an unknown abort cause and an unknown event tag are refused, not guessed.
    let mut journal = Journal::new();
    journal
        .append(EventBody::Effect(EffectEvent::Aborted {
            reservation: ReservationOrdinal(0),
            cause: AbortCause::Cancel,
        }))
        .unwrap();
    let bytes = journal.encode().unwrap();
    let last = bytes.len() - 1;
    let mut bad_cause = bytes.clone();
    bad_cause[last] = 9;
    assert!(matches!(
        Journal::decode(&bad_cause),
        Err(DecodeError::UnknownTag {
            table: "abort cause",
            tag: 9,
            ..
        })
    ));
    let mut bad_event = bytes;
    bad_event[last - 5] = 9;
    assert!(matches!(
        Journal::decode(&bad_event),
        Err(DecodeError::UnknownTag {
            table: "effect event",
            tag: 9,
            ..
        })
    ));
}

/// INV-005: the family's code names no filesystem, environment, clock or thread path.
/// The binding's own barrier test (`pr14_impl01_binding.rs`) covers `binding.rs`.
#[test]
fn the_effect_family_names_no_ambient_path() {
    let source = include_str!("../src/family/effect.rs");
    let code: String = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for ambient in ["std::fs", "std::env", "std::time", "std::thread", "rand"] {
        assert!(!code.contains(ambient), "effect.rs names {ambient}");
    }
    assert!(code.contains("pub(crate) fn lift"), "the scan sees code");
}
