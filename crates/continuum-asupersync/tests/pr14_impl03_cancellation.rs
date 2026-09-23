//! PR-14-IMPL-03 (bn-bx7i): the cancellation phases, observed from the real substrate.
//!
//! > **Exit:** identical controlled choice logs produce canonical identical semantic
//! > events.
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 14
//!
//! The binding (`binding.rs`) drives asupersync 0.5.0's lab runtime under a choice log
//! and, with the cancellation family observed, journals each cancelled task's
//! `requested → acknowledged → cancelled` phases from the substrate's own trace. These
//! tests close the exit sentence for that family and hold its journal to the region
//! calculus.
//!
//! # Evidence map
//!
//! | Claim | Test |
//! |---|---|
//! | identical choice logs → byte-identical journals, every log | [`identical_choice_logs_give_byte_identical_cancellation_journals`] |
//! | the lab seed does not reach the journal, though it reorders the substrate's polls and sibling closes (7 seeds, incl. 0, 1, 7, 99, u64::MAX) | [`the_lab_seed_does_not_reach_the_cancellation_journal`] |
//! | different choice logs → different journals (anti-vacuity of the identity) | [`different_choice_logs_give_different_cancellation_journals`] |
//! | substrate journal ≡ an independent scripted account, every log (differential) | [`the_substrate_agrees_byte_for_byte_with_a_scripted_account`] |
//! | the cancellation family adds events and changes no lifecycle byte | [`dropping_the_cancellation_events_gives_the_lifecycle_journal`] |
//! | every journal lifts into the region calculus, phases included | [`every_cancellation_journal_conforms_to_the_region_calculus`] |
//! | one pinned run, rendered: the canonical order and the causes | [`a_nested_cancellation_journals_its_phases_in_canonical_order`] |
//! | mutated journals are rejected, each with its own fault (anti-vacuity) | [`mutated_phase_journals_are_rejected`] |
//! | unbound families, missing lifecycle, lost telemetry, bad bytes: typed refusals | [`refusals_are_typed`] |
//! | no ambient filesystem or environment path in the family | [`the_cancellation_family_names_no_ambient_path`] |

use std::collections::BTreeMap;

use continuum_asupersync::binding::{
    BindingConfig, BindingRefusal, Families, Program, SubstrateOp, run,
};
use continuum_asupersync::choice::ChoiceLog;
use continuum_asupersync::encoding::DecodeError;
use continuum_asupersync::family::cancellation::{
    CancelCause, CancellationEvent, CancellationFault, CancellationReport,
};
use continuum_asupersync::family::lifecycle::{
    LifecycleEvent, LifecycleReport, RegionLabel, TaskLabel, TaskOrdinal, TaskStep,
};
use continuum_asupersync::family::{EventBody, Family, Report};
use continuum_asupersync::journal::Journal;
use continuum_asupersync::lift::{LiftVerdict, Nonconformance, lift};
use continuum_asupersync::source::{Script, record};
use continuum_task::region::worker::Resumability;
use continuum_value::assurance::InconclusiveReason;

const SEED: u64 = 0;

/// The configuration the tests run: lifecycle and cancellation observed.
fn config(seed: u64) -> BindingConfig {
    BindingConfig::new(seed).observing(Family::Cancellation)
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

/// Three actors. A root task, spawned and never cancelled. A nested pair `r1 ⊃ r2`,
/// cancelled at `r1`, with a never-begun task in `r1` (cause `user`) and a suspended
/// task in `r2` (cause `parent-cancelled`). A region `r3` with one suspended and one
/// never-begun task, cancelled directly, so two tasks are cancelled in one operation
/// and the substrate's seeded scheduler picks their poll order.
///
/// Sibling regions under a cancelled region are [`branching`]'s. Every interleaving is a
/// legal run.
fn cascading() -> Vec<Program> {
    let (a, b, c, d, e) = (
        TaskLabel(1),
        TaskLabel(2),
        TaskLabel(3),
        TaskLabel(4),
        TaskLabel(5),
    );
    let (r1, r2, r3) = (RegionLabel(1), RegionLabel(2), RegionLabel(3));
    vec![
        vec![spawn(RegionLabel::ROOT, a)],
        vec![
            open(RegionLabel::ROOT, r1),
            open(r1, r2),
            spawn(r1, b),
            spawn(r2, c),
            SubstrateOp::Begin { task: c },
            SubstrateOp::Cancel { region: r1 },
        ],
        vec![
            open(RegionLabel::ROOT, r3),
            spawn(r3, d),
            spawn(r3, e),
            SubstrateOp::Begin { task: d },
            SubstrateOp::Cancel { region: r3 },
        ],
    ]
}

/// Three actors. A root task, never cancelled. A cancelled region with two child regions,
/// one of which has a child of its own, and a task in every region:
///
/// ```text
/// r1 ─┬─ r2 ── r4
///     └─ r3
/// ```
///
/// The substrate's seeded scheduler picks the order in which `r2`'s and `r3`'s closes
/// complete. And a third region cancelled on its own, interleaved with the rest.
fn branching() -> Vec<Program> {
    let (a, b, c, d, e, f) = (
        TaskLabel(1),
        TaskLabel(2),
        TaskLabel(3),
        TaskLabel(4),
        TaskLabel(5),
        TaskLabel(6),
    );
    let (r1, r2, r3, r4, r5) = (
        RegionLabel(1),
        RegionLabel(2),
        RegionLabel(3),
        RegionLabel(4),
        RegionLabel(5),
    );
    vec![
        vec![spawn(RegionLabel::ROOT, a)],
        vec![
            open(RegionLabel::ROOT, r1),
            open(r1, r2),
            open(r1, r3),
            open(r2, r4),
            spawn(r1, b),
            spawn(r2, c),
            spawn(r3, d),
            spawn(r4, e),
            SubstrateOp::Begin { task: c },
            SubstrateOp::Begin { task: e },
            SubstrateOp::Cancel { region: r1 },
        ],
        vec![
            open(RegionLabel::ROOT, r5),
            spawn(r5, f),
            SubstrateOp::Cancel { region: r5 },
        ],
    ]
}

/// Two actors: a root task, and a region whose normal close waits for live work until a
/// cancellation upgrades it.
fn close_then_cancel() -> Vec<Program> {
    let (a, b) = (TaskLabel(1), TaskLabel(2));
    let r1 = RegionLabel(1);
    vec![
        vec![spawn(RegionLabel::ROOT, a), SubstrateOp::Begin { task: a }],
        vec![
            open(RegionLabel::ROOT, r1),
            spawn(r1, b),
            SubstrateOp::Begin { task: b },
            SubstrateOp::Close { region: r1 },
            SubstrateOp::Cancel { region: r1 },
        ],
    ]
}

fn lengths(programs: &[Program]) -> Vec<usize> {
    programs.iter().map(Vec::len).collect()
}

const CASCADING_LOGS: usize = 5_544; // 12! / (1!·6!·5!)
const CLOSE_THEN_CANCEL_LOGS: usize = 21; // 7! / (2!·5!)
const BRANCHING_LOGS: usize = 5_460; // 15! / (1!·11!·3!)
const ALL_LOGS: usize = CASCADING_LOGS + CLOSE_THEN_CANCEL_LOGS + BRANCHING_LOGS;

/// Every (program set, log) pair the exhaustive tests run, with the number of tasks
/// each run cancels.
fn corpus() -> Vec<(&'static str, Vec<Program>, Vec<ChoiceLog>, usize)> {
    let cascading = cascading();
    let close_then_cancel = close_then_cancel();
    let branching = branching();
    let cascading_logs = ChoiceLog::enumerate(&lengths(&cascading));
    let close_logs = ChoiceLog::enumerate(&lengths(&close_then_cancel));
    let branching_logs = ChoiceLog::enumerate(&lengths(&branching));
    assert_eq!(cascading_logs.len(), CASCADING_LOGS);
    assert_eq!(close_logs.len(), CLOSE_THEN_CANCEL_LOGS);
    assert_eq!(branching_logs.len(), BRANCHING_LOGS);
    vec![
        ("cascading", cascading, cascading_logs, 4),
        ("close-then-cancel", close_then_cancel, close_logs, 1),
        ("branching", branching, branching_logs, 5),
    ]
}

fn cancellation_events(journal: &Journal) -> Vec<&CancellationEvent> {
    journal
        .events()
        .iter()
        .filter_map(|event| match event.body() {
            EventBody::Cancellation(event) => Some(event),
            _ => None,
        })
        .collect()
}

// --- the exit property ---------------------------------------------------------------

#[test]
fn identical_choice_logs_give_byte_identical_cancellation_journals() {
    let mut compared = 0;
    for (name, programs, logs, cancelled) in corpus() {
        for log in &logs {
            let first = run(&programs, log, &config(SEED)).expect("every log is a legal run");
            let second = run(&programs, log, &config(SEED)).expect("every log is a legal run");
            assert_eq!(
                first.encode().unwrap(),
                second.encode().unwrap(),
                "{name} log {log}"
            );
            assert_eq!(first.digest().unwrap(), second.digest().unwrap());
            // Non-vacuity: three phases for every cancelled task, in every run.
            assert_eq!(
                cancellation_events(&first).len(),
                3 * cancelled,
                "{name} log {log}:\n{}",
                first.render()
            );
            compared += 1;
        }
    }
    assert_eq!(compared, ALL_LOGS);
}

/// Metamorphic relation: the lab seed changes the order in which the substrate polls
/// concurrently cancelled tasks (seed 7 reverses `r3`'s two tasks against seed 0 in
/// asupersync 0.5.0) and the order in which sibling regions finish closing, and the
/// journal does not change.
#[test]
fn the_lab_seed_does_not_reach_the_cancellation_journal() {
    let seeds = [0, 1, 7, 42, 99, 0x9e37_79b9_7f4a_7c15, u64::MAX];
    let mut compared = 0;
    for (name, programs, logs, _) in corpus() {
        let sample = logs
            .iter()
            .enumerate()
            .filter(|(index, _)| index % 7 == 0 || *index + 1 == logs.len())
            .map(|(_, log)| log);
        for log in sample {
            let reference = run(&programs, log, &config(seeds[0]))
                .unwrap()
                .encode()
                .unwrap();
            for seed in &seeds[1..] {
                let other = run(&programs, log, &config(*seed))
                    .unwrap()
                    .encode()
                    .unwrap();
                assert_eq!(reference, other, "{name} log {log} seed {seed:#x}");
                compared += 1;
            }
        }
    }
    assert!(compared > 9000, "the relation ran over {compared} pairs");
}

#[test]
fn different_choice_logs_give_different_cancellation_journals() {
    for (name, programs, logs, _) in corpus() {
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

/// The test's own account of a run: labels, ordinals, the region tree, and which tasks
/// are live. It calls nothing in the adapter or in the substrate, so the scripted
/// journal it produces is a second path to the same bytes.
#[derive(Default)]
struct Account {
    regions: BTreeMap<RegionLabel, u32>,
    region_labels: Vec<RegionLabel>,
    parents: Vec<Option<u32>>,
    finalized: Vec<bool>,
    tasks: BTreeMap<TaskLabel, u32>,
    task_region: Vec<u32>,
    live: Vec<bool>,
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

    fn cn(&mut self, report: CancellationReport) {
        self.script.push(Report::Cancellation(report));
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

    fn live_in(&self, root: u32) -> Vec<u32> {
        (0..self.live.len())
            .map(|t| u32::try_from(t).unwrap())
            .filter(|t| {
                self.live[*t as usize] && self.in_subtree(self.task_region[*t as usize], root)
            })
            .collect()
    }

    /// Children before parents, then ascending: the order in which a chain of nested
    /// regions finishes closing.
    fn post_order(&self, root: u32) -> Vec<u32> {
        let mut children: Vec<u32> = (0..self.parents.len())
            .map(|r| u32::try_from(r).unwrap())
            .filter(|r| self.parents[*r as usize] == Some(root) && !self.finalized[*r as usize])
            .collect();
        children.sort_unstable();
        let mut out = Vec::new();
        for child in children {
            out.extend(self.post_order(child));
        }
        out.push(root);
        out
    }

    fn teardown(&mut self, region: u32, causes: &BTreeMap<u32, CancelCause>) {
        for member in self.post_order(region) {
            for task in 0..self.live.len() {
                let task = u32::try_from(task).unwrap();
                if self.live[task as usize] && self.task_region[task as usize] == member {
                    let Some(cause) = causes.get(&task).copied() else {
                        continue;
                    };
                    self.cn(CancellationReport::Acknowledge {
                        task: TaskOrdinal(task),
                    });
                    self.cn(CancellationReport::Complete {
                        task: TaskOrdinal(task),
                        cause,
                    });
                    self.live[task as usize] = false;
                }
            }
            let label = self.region_labels[member as usize];
            self.lc(LifecycleReport::Drain { region: label });
            self.lc(LifecycleReport::Finalize { region: label });
            self.finalized[member as usize] = true;
        }
    }

    fn apply(&mut self, op: &SubstrateOp) {
        let step = |task: TaskLabel, step: TaskStep| LifecycleReport::Step { task, step };
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
                self.lc(LifecycleReport::Spawn {
                    region: *region,
                    task: *task,
                    resumability: resumability.clone(),
                });
            }
            SubstrateOp::Begin { task } => {
                self.lc(step(*task, TaskStep::Begin));
                self.lc(step(*task, TaskStep::Suspend));
            }
            SubstrateOp::Continue { task } => {
                self.lc(step(*task, TaskStep::Resume));
                self.lc(step(*task, TaskStep::Suspend));
            }
            SubstrateOp::Finish { task } => {
                self.lc(step(*task, TaskStep::Resume));
                self.lc(step(*task, TaskStep::Complete));
                self.live[self.tasks[task] as usize] = false;
            }
            SubstrateOp::Close { region } => {
                self.lc(LifecycleReport::Close { region: *region });
                let ordinal = self.regions[region];
                if self.live_in(ordinal).is_empty() {
                    self.teardown(ordinal, &BTreeMap::new());
                }
            }
            SubstrateOp::Cancel { region } => {
                self.lc(LifecycleReport::Cancel { region: *region });
                let ordinal = self.regions[region];
                let mut causes = BTreeMap::new();
                for task in self.live_in(ordinal) {
                    let cause = if self.task_region[task as usize] == ordinal {
                        CancelCause::User
                    } else {
                        CancelCause::ParentCancelled
                    };
                    causes.insert(task, cause);
                    self.cn(CancellationReport::Request {
                        task: TaskOrdinal(task),
                        cause,
                    });
                }
                self.teardown(ordinal, &causes);
            }
        }
    }
}

/// The account's script for `log` over `programs`: one actor, so its log is all zeros.
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
    for (name, programs, logs, _) in corpus() {
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

/// The cancellation family is additive: remove its events from a run's journal and what
/// remains is, byte for byte, the lifecycle journal bn-lf4i pinned for the same run.
#[test]
fn dropping_the_cancellation_events_gives_the_lifecycle_journal() {
    for (name, programs, logs, _) in corpus() {
        for log in logs.iter().step_by(7) {
            let full = run(&programs, log, &config(SEED)).unwrap();
            let lifecycle = run(&programs, log, &BindingConfig::new(SEED)).unwrap();
            let mut projected = Journal::new();
            for event in full.events() {
                if event.family() == Family::Lifecycle {
                    projected.append(event.body().clone()).unwrap();
                }
            }
            assert_eq!(
                projected.encode().unwrap(),
                lifecycle.encode().unwrap(),
                "{name} log {log}"
            );
            assert!(cancellation_events(&lifecycle).is_empty());
        }
    }
}

// --- conformance ---------------------------------------------------------------------

#[test]
fn every_cancellation_journal_conforms_to_the_region_calculus() {
    for (name, programs, logs, _) in corpus() {
        let expected_finalizations = match name {
            "cascading" => 3,
            "branching" => 5,
            _ => 1,
        };
        for log in &logs {
            let journal = run(&programs, log, &config(SEED)).unwrap();
            let journal = Journal::decode(&journal.encode().unwrap()).unwrap();
            let LiftVerdict::Conforms(lifted) = lift(&journal) else {
                panic!("{name} log {log}:\n{}", journal.render());
            };
            assert_eq!(
                lifted.finalizations().len(),
                expected_finalizations,
                "{name} log {log}"
            );
            for (seq, finalization) in lifted.finalizations() {
                assert!(
                    finalization.orphans().is_empty()
                        && finalization.unresolved_publications().is_empty(),
                    "{name} log {log}: finalization at {seq}:\n{}",
                    finalization.render()
                );
            }
        }
    }
}

/// One run, pinned in full. `r1 ⊃ r2` is cancelled at `r1`: `t0` (in `r1`, never begun)
/// has cause `user` and `t1` (in `r2`, suspended) has `parent-cancelled`. The requests
/// are journaled together after the region's cancel request. Each drain is preceded by
/// the acknowledgement and completion of exactly the tasks it absorbs, children first.
#[test]
fn a_nested_cancellation_journals_its_phases_in_canonical_order() {
    let (b, c) = (TaskLabel(2), TaskLabel(3));
    let (r1, r2) = (RegionLabel(1), RegionLabel(2));
    let programs = vec![vec![
        open(RegionLabel::ROOT, r1),
        open(r1, r2),
        spawn(r1, b),
        spawn(r2, c),
        SubstrateOp::Begin { task: c },
        SubstrateOp::Cancel { region: r1 },
    ]];
    let journal = run(&programs, &ChoiceLog::new([0; 6]), &config(SEED)).unwrap();
    assert_eq!(
        journal.render(),
        "0 lifecycle region-opened r1 parent=r0
1 lifecycle region-opened r2 parent=r1
2 lifecycle task-spawned t0 region=r1 resumable
3 lifecycle task-spawned t1 region=r2 resumable
4 lifecycle task-stepped t1 begin
5 lifecycle task-stepped t1 suspend
6 lifecycle region-cancel-requested r1
7 cancellation requested t0 cause=user
8 cancellation requested t1 cause=parent-cancelled
9 cancellation acknowledged t1
10 cancellation cancelled t1 cause=parent-cancelled
11 lifecycle region-drained r2 cancelled=[t1]
12 lifecycle region-finalized r2
13 cancellation acknowledged t0
14 cancellation cancelled t0 cause=user
15 lifecycle region-drained r1 cancelled=[t0]
16 lifecycle region-finalized r1
"
    );
    let bytes = journal.encode().unwrap();
    assert_eq!(Journal::decode(&bytes).unwrap(), journal);
    let LiftVerdict::Conforms(lifted) = lift(&journal) else {
        panic!("{}", journal.render());
    };
    // Only r1's subtree held work, so the whole ledger balances.
    let (_, last) = lifted.finalizations().last().unwrap();
    assert!(last.is_total(), "{}", last.render());
}

// --- anti-vacuity: the lift rejects wrong phase histories -----------------------------

fn rebuilt(bodies: Vec<EventBody>) -> Journal {
    let mut journal = Journal::new();
    for body in bodies {
        journal.append(body).unwrap();
    }
    journal
}

fn bodies(journal: &Journal) -> Vec<EventBody> {
    journal.events().iter().map(|e| e.body().clone()).collect()
}

fn position(bodies: &[EventBody], wanted: impl Fn(&CancellationEvent) -> bool) -> usize {
    bodies
        .iter()
        .position(|body| matches!(body, EventBody::Cancellation(event) if wanted(event)))
        .expect("the journal has such an event")
}

fn cancellation_fault(journal: &Journal) -> CancellationFault {
    match lift(journal) {
        LiftVerdict::Violates {
            reason: Nonconformance::Cancellation(fault),
            ..
        } => fault,
        other => panic!(
            "expected a cancellation fault, got {other:?}\n{}",
            journal.render()
        ),
    }
}

/// Four mutation operators over real substrate journals. Each one yields a journal the
/// lift must reject, and each is rejected with its own fault. The unmutated journal
/// conforms, so the rejection is the mutant's doing.
#[test]
fn mutated_phase_journals_are_rejected() {
    let programs = cascading();
    let logs = ChoiceLog::enumerate(&lengths(&programs));
    let mut mutants = 0;
    for log in logs.iter().step_by(17) {
        let journal = run(&programs, log, &config(SEED)).unwrap();
        assert!(matches!(lift(&journal), LiftVerdict::Conforms(_)));
        let original = bodies(&journal);

        // 1. A task completes without acknowledging.
        let mut events = original.clone();
        events.remove(position(&events, |e| {
            matches!(e, CancellationEvent::Acknowledged { .. })
        }));
        assert!(
            matches!(
                cancellation_fault(&rebuilt(events)),
                CancellationFault::OutOfOrder {
                    event: "cancelled",
                    phase: "requested",
                    ..
                }
            ),
            "log {log}"
        );

        // 2. A completion journaled after the drain that terminated the task.
        let mut events = original.clone();
        let at = position(&events, |e| {
            matches!(e, CancellationEvent::Cancelled { .. })
        });
        let completion = events.remove(at);
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
        events.insert(drain + 1, completion);
        assert!(
            matches!(
                cancellation_fault(&rebuilt(events)),
                CancellationFault::TaskTerminal {
                    event: "cancelled",
                    state: "cancelled",
                    ..
                }
            ),
            "log {log}"
        );

        // 3. A request with the wrong cause.
        let mut events = original.clone();
        let at = position(&events, |e| {
            matches!(e, CancellationEvent::Requested { .. })
        });
        if let EventBody::Cancellation(CancellationEvent::Requested { cause, .. }) = &mut events[at]
        {
            *cause = match cause {
                CancelCause::User => CancelCause::ParentCancelled,
                CancelCause::ParentCancelled => CancelCause::User,
            };
        }
        assert!(
            matches!(
                cancellation_fault(&rebuilt(events)),
                CancellationFault::CauseMismatch { .. }
            ),
            "log {log}"
        );

        // 4. Every phase of one task dropped: its drain overtakes its cancellation.
        let mut events = original.clone();
        let EventBody::Cancellation(first) = &events[position(&events, |_| true)] else {
            unreachable!()
        };
        let victim = first.task();
        events.retain(|b| !matches!(b, EventBody::Cancellation(e) if e.task() == victim));
        assert_eq!(
            cancellation_fault(&rebuilt(events)),
            CancellationFault::DrainedBeforeCancelled {
                task: victim.0,
                phase: "active"
            },
            "log {log}"
        );
        mutants += 4;
    }
    assert!(mutants > 1200, "{mutants} mutants");
}

// --- refusals ------------------------------------------------------------------------

#[test]
fn refusals_are_typed() {
    let programs = cascading();
    let log = ChoiceLog::enumerate(&lengths(&programs)).remove(0);

    // A family the binding does not bind is refused before the substrate runs.
    let refusal = run(&programs, &log, &config(SEED).observing(Family::Obligation)).unwrap_err();
    assert_eq!(refusal, BindingRefusal::FamilyNotBound(Family::Obligation));
    assert_eq!(
        refusal.inconclusive_reason(),
        Some(InconclusiveReason::Unsupported)
    );
    assert!(refusal.to_string().contains("PR-14-IMPL-04"), "{refusal}");

    // Cancellation without lifecycle is a malformed configuration.
    let bare = BindingConfig {
        families: Families::NONE.with(Family::Cancellation),
        ..config(SEED)
    };
    let refusal = run(&programs, &log, &bare).unwrap_err();
    assert_eq!(refusal, BindingRefusal::LifecycleNotObserved);
    assert_eq!(refusal.inconclusive_reason(), None);

    // A trace buffer too small for the run: lost telemetry, never a short journal.
    let tiny = BindingConfig {
        trace_capacity: 24,
        ..config(SEED)
    };
    let refusal = run(&programs, &log, &tiny).unwrap_err();
    assert!(
        matches!(refusal, BindingRefusal::TraceOverflow { .. }),
        "{refusal}"
    );
    assert_eq!(
        refusal.inconclusive_reason(),
        Some(InconclusiveReason::InsufficientTelemetry)
    );

    // The INV-008 reading of the refusals no bound program reaches today.
    let readings = [
        (
            BindingRefusal::UnmappedCancelReason {
                kind: "Timeout".to_owned(),
            },
            Some(InconclusiveReason::Unsupported),
        ),
        (
            BindingRefusal::SubstrateProtocolViolation {
                detail: "skipped state".to_owned(),
            },
            Some(InconclusiveReason::EngineError),
        ),
        (
            BindingRefusal::CancellationUnfinished { task: 0 },
            Some(InconclusiveReason::InsufficientTelemetry),
        ),
    ];
    for (refusal, reading) in readings {
        assert_eq!(refusal.inconclusive_reason(), reading, "{refusal}");
    }

    // Bytes: an unknown cause tag and an unknown event tag are refused, not guessed.
    let mut journal = Journal::new();
    journal
        .append(EventBody::Cancellation(CancellationEvent::Requested {
            task: TaskOrdinal(0),
            cause: CancelCause::User,
        }))
        .unwrap();
    let bytes = journal.encode().unwrap();
    let last = bytes.len() - 1;
    let mut bad_cause = bytes.clone();
    bad_cause[last] = 9;
    assert!(matches!(
        Journal::decode(&bad_cause),
        Err(DecodeError::UnknownTag {
            table: "cancel cause",
            tag: 9,
            ..
        })
    ));
    let mut bad_event = bytes;
    bad_event[last - 5] = 9;
    assert!(matches!(
        Journal::decode(&bad_event),
        Err(DecodeError::UnknownTag {
            table: "cancellation event",
            tag: 9,
            ..
        })
    ));
}

/// INV-005: the family's code names no filesystem or environment path. The binding's own
/// barrier test (`pr14_impl01_binding.rs`) covers `binding.rs`, which this bone extends.
#[test]
fn the_cancellation_family_names_no_ambient_path() {
    let source = include_str!("../src/family/cancellation.rs");
    let code: String = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for ambient in ["std::fs", "std::env", "std::time", "std::thread", "rand"] {
        assert!(!code.contains(ambient), "cancellation.rs names {ambient}");
    }
    assert!(code.contains("pub(crate) fn lift"), "the scan sees code");
}
