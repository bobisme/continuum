//! PR-14-IMPL-04 (bn-6nm8): the obligation ledger, observed from the real substrate.
//!
//! > **Exit:** identical controlled choice logs produce canonical identical semantic
//! > events.
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 14
//!
//! The binding (`binding.rs`) drives asupersync 0.5.0's lab runtime under a choice log.
//! Bound tasks acquire obligations of several kinds through their own `Cx`, hand them to
//! other tasks (`ObligationToken::try_transfer`), commit them, abort them, leak them by
//! returning while they are open, or have them aborted by their cancellation. With the
//! obligations family observed, the binding journals the substrate's ledger: who holds
//! each obligation in which region, each discharge, transfer and leak, and each region's
//! balance when it closes. These tests close the exit sentence for that family, hold it
//! to the region calculus, and check it against the substrate's own records and
//! obligation-leak oracle (the binding refuses a run where they disagree).
//!
//! # Evidence map
//!
//! | Claim | Test |
//! |---|---|
//! | identical choice logs → byte-identical journals, every log (sibling regions included) | [`identical_choice_logs_give_byte_identical_ledger_journals`] |
//! | the lab seed does not reach the journal, though it reorders sibling closes (7 seeds) | [`the_lab_seed_does_not_reach_the_ledger_journal`] |
//! | different choice logs → different journals (anti-vacuity of the identity) | [`different_choice_logs_give_different_ledger_journals`] |
//! | substrate journal ≡ an independent scripted account, every log (differential) | [`the_substrate_agrees_byte_for_byte_with_a_scripted_account`] |
//! | the family adds events and changes no byte of the other three families' journals | [`dropping_a_family_gives_that_projection_byte_for_byte`] |
//! | balanced runs lift and settle every region; a leak is a violation at its region's close | [`balanced_runs_conform_and_leaks_violate_at_close`] |
//! | one pinned run, rendered: kinds, a cross-region transfer, cancellation discharges | [`a_cancelled_subtree_settles_its_ledger_in_canonical_order`] |
//! | mutated journals are rejected, each with its own fault (anti-vacuity) | [`mutated_ledger_journals_are_rejected`] |
//! | malformed transfers, unobserved leaks, lost steps, bad bytes: typed refusals | [`refusals_are_typed`] |
//! | no ambient filesystem or environment path in the family | [`the_obligation_family_names_no_ambient_path`] |

use std::collections::BTreeMap;

use continuum_asupersync::binding::{
    BindingConfig, BindingRefusal, Program, SubstrateOp, run, run_witnessed,
};
use continuum_asupersync::choice::ChoiceLog;
use continuum_asupersync::encoding::DecodeError;
use continuum_asupersync::family::cancellation::{CancelCause, CancellationReport};
use continuum_asupersync::family::effect::{
    AbortCause, EffectReport, ReservationLabel, ReservationOrdinal,
};
use continuum_asupersync::family::lifecycle::{
    LifecycleEvent, LifecycleReport, RegionLabel, RegionOrdinal, TaskLabel, TaskOrdinal, TaskStep,
};
use continuum_asupersync::family::obligation::{
    Discharge, LedgerFault, ObligationEvent, ObligationKind, ObligationOrdinal, ObligationReport,
    ObligationSet,
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

fn acquire(task: TaskLabel, label: u32, kind: ObligationKind) -> SubstrateOp {
    SubstrateOp::Acquire {
        task,
        reservation: ReservationLabel(label),
        kind,
    }
}

fn reserve(task: TaskLabel, label: u32) -> SubstrateOp {
    SubstrateOp::Reserve {
        task,
        reservation: ReservationLabel(label),
    }
}

fn commit(label: u32) -> SubstrateOp {
    SubstrateOp::Commit {
        reservation: ReservationLabel(label),
    }
}

fn transfer(label: u32, to: TaskLabel) -> SubstrateOp {
    SubstrateOp::Transfer {
        reservation: ReservationLabel(label),
        to,
    }
}

/// Two actors. A root task that holds a lease across a transaction and commits both.
/// And a cancelled region with two sibling child regions:
///
/// ```text
/// r1 ─┬─ r2 (b: holds an ack, and receives c's I/O obligation)
///     └─ r3 (c: acquires an I/O obligation and hands it to b, across regions)
/// ```
///
/// The cancellation aborts what b holds. The substrate's seeded scheduler picks the
/// order in which `r2` and `r3` close. Every interleaving is a legal run.
fn ledger() -> Vec<Program> {
    let (a, b, c) = (TaskLabel(1), TaskLabel(2), TaskLabel(3));
    let (r1, r2, r3) = (RegionLabel(1), RegionLabel(2), RegionLabel(3));
    vec![
        vec![
            spawn(RegionLabel::ROOT, a),
            begin(a),
            acquire(a, 1, ObligationKind::Lease),
            reserve(a, 2),
            commit(2),
            commit(1),
        ],
        vec![
            open(RegionLabel::ROOT, r1),
            open(r1, r2),
            open(r1, r3),
            spawn(r2, b),
            spawn(r3, c),
            begin(b),
            begin(c),
            acquire(b, 3, ObligationKind::Ack),
            acquire(c, 4, ObligationKind::IoOp),
            transfer(4, b),
            SubstrateOp::Cancel { region: r1 },
        ],
    ]
}

/// Two actors. A region whose first task acquires a lease and hands it to the second,
/// which commits it; both finish and the region closes normally. With `leak`, the first
/// task also acquires an ack and returns while it is open. And a root task that commits
/// a send permit.
fn closing(leak: bool) -> Vec<Program> {
    let (a, a2, b) = (TaskLabel(1), TaskLabel(2), TaskLabel(3));
    let r1 = RegionLabel(1);
    let mut first = vec![
        open(RegionLabel::ROOT, r1),
        spawn(r1, a),
        spawn(r1, a2),
        begin(a),
        begin(a2),
        acquire(a, 1, ObligationKind::Lease),
        transfer(1, a2),
        commit(1),
    ];
    if leak {
        first.push(acquire(a, 2, ObligationKind::Ack));
    }
    first.extend([
        SubstrateOp::Finish { task: a },
        SubstrateOp::Finish { task: a2 },
        SubstrateOp::Close { region: r1 },
    ]);
    vec![
        first,
        vec![
            spawn(RegionLabel::ROOT, b),
            begin(b),
            acquire(b, 3, ObligationKind::SendPermit),
            commit(3),
        ],
    ]
}

fn lengths(programs: &[Program]) -> Vec<usize> {
    programs.iter().map(Vec::len).collect()
}

const LEDGER_LOGS: usize = 12_376; // 17! / (6!·11!)
const CLOSING_LOGS: usize = 1_365; // 15! / (11!·4!)
const LEAKING_LOGS: usize = 1_820; // 16! / (12!·4!)

/// A corpus entry: name, programs, logs, obligations per run.
type Entry = (&'static str, Vec<Program>, Vec<ChoiceLog>, usize);

/// The balanced corpora, which every test runs.
fn corpus() -> Vec<Entry> {
    let ledger = ledger();
    let closing = closing(false);
    let ledger_logs = ChoiceLog::enumerate(&lengths(&ledger));
    let closing_logs = ChoiceLog::enumerate(&lengths(&closing));
    assert_eq!(ledger_logs.len(), LEDGER_LOGS);
    assert_eq!(closing_logs.len(), CLOSING_LOGS);
    vec![
        ("ledger", ledger, ledger_logs, 4),
        ("closing", closing, closing_logs, 2),
    ]
}

/// The corpus in which every run leaks.
fn leaking() -> Entry {
    let programs = closing(true);
    let logs = ChoiceLog::enumerate(&lengths(&programs));
    assert_eq!(logs.len(), LEAKING_LOGS);
    ("leaking", programs, logs, 3)
}

fn ledger_events(journal: &Journal) -> Vec<&ObligationEvent> {
    journal
        .events()
        .iter()
        .filter_map(|event| match event.body() {
            EventBody::Obligation(event) => Some(event),
            _ => None,
        })
        .collect()
}

// --- the exit property ---------------------------------------------------------------

#[test]
fn identical_choice_logs_give_byte_identical_ledger_journals() {
    let mut compared = 0;
    let mut all = corpus();
    all.push(leaking());
    for (name, programs, logs, obligations) in all {
        for log in &logs {
            let first = run(&programs, log, &config(SEED)).expect("every log is a legal run");
            let second = run(&programs, log, &config(SEED)).expect("every log is a legal run");
            assert_eq!(
                first.encode().unwrap(),
                second.encode().unwrap(),
                "{name} log {log}"
            );
            assert_eq!(first.digest().unwrap(), second.digest().unwrap());
            // Non-vacuity: every obligation opened, and every one ended.
            let events = ledger_events(&first);
            let opened = events
                .iter()
                .filter(|e| matches!(e, ObligationEvent::Opened { .. }))
                .count();
            let ended = events
                .iter()
                .filter(|e| {
                    matches!(
                        e,
                        ObligationEvent::Discharged { .. } | ObligationEvent::Leaked { .. }
                    )
                })
                .count();
            assert_eq!(
                (opened, ended),
                (obligations, obligations),
                "{name} log {log}"
            );
            compared += 1;
        }
    }
    assert_eq!(compared, LEDGER_LOGS + CLOSING_LOGS + LEAKING_LOGS);
}

#[test]
fn the_lab_seed_does_not_reach_the_ledger_journal() {
    let mut compared = 0;
    let mut reordered = 0;
    let mut all = corpus();
    all.push(leaking());
    for (name, programs, logs, _) in all {
        let sample = logs
            .iter()
            .enumerate()
            .filter(|(index, _)| index % 3 == 0 || *index + 1 == logs.len())
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
    assert!(compared > 25_000, "the relation ran over {compared} pairs");
    assert!(reordered > 0, "no seed moved the substrate's close order");
}

#[test]
fn different_choice_logs_give_different_ledger_journals() {
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Gate {
    Created,
    Running,
    Suspended,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Standing {
    Open,
    Discharged,
    Leaked,
}

#[derive(Clone, Copy)]
struct Obl {
    kind: ObligationKind,
    holder: u32,
    region: u32,
    standing: Standing,
    reservation: Option<u32>,
}

/// The test's own account of a run: labels, ordinals, the region tree, which tasks are
/// live, what each task holds, each obligation's holder and region, and the gate's
/// phase. It calls nothing in the adapter or in the substrate.
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
    held: Vec<Vec<u32>>,
    obligations: Vec<Obl>,
    labels: BTreeMap<ReservationLabel, u32>,
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

    fn ob(&mut self, event: ObligationEvent) {
        self.script
            .push(Report::Obligation(ObligationReport(event)));
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

    fn transactions_held(&self, task: u32) -> usize {
        self.held[task as usize]
            .iter()
            .filter(|o| self.obligations[**o as usize].kind == ObligationKind::Transaction)
            .count()
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
        if self.transactions_held(task) == 0 {
            self.step(task, TaskStep::Suspend);
            self.gate[task as usize] = Gate::Suspended;
        }
    }

    fn acquire(&mut self, task: u32, label: ReservationLabel, kind: ObligationKind) {
        self.wake(task);
        let obligation = u32::try_from(self.obligations.len()).unwrap();
        let reservation = (kind == ObligationKind::Transaction).then(|| {
            self.next_reservation += 1;
            self.next_reservation - 1
        });
        self.obligations.push(Obl {
            kind,
            holder: task,
            region: self.task_region[task as usize],
            standing: Standing::Open,
            reservation,
        });
        self.labels.insert(label, obligation);
        self.held[task as usize].push(obligation);
        // The gate parks in the poll that posts the reservation; the runtime applies the
        // post after the poll.
        self.park(task);
        if let Some(reservation) = reservation {
            self.script.push(Report::Effect(EffectReport::Reserve {
                reservation: ReservationOrdinal(reservation),
                task: TaskOrdinal(task),
            }));
        }
        self.ob(ObligationEvent::Opened {
            obligation: ObligationOrdinal(obligation),
            kind,
            holder: TaskOrdinal(task),
            region: RegionOrdinal(self.task_region[task as usize]),
        });
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
                self.held.push(Vec::new());
                self.lc(LifecycleReport::Spawn {
                    region: *region,
                    task: *task,
                    resumability: resumability.clone(),
                });
            }
            SubstrateOp::Begin { task } | SubstrateOp::Continue { task } => {
                let task = self.tasks[task];
                self.wake(task);
                self.park(task);
            }
            SubstrateOp::Finish { task } => {
                let task = self.tasks[task];
                self.wake(task);
                let mut held = core::mem::take(&mut self.held[task as usize]);
                held.sort_unstable();
                for obligation in held {
                    self.obligations[obligation as usize].standing = Standing::Leaked;
                    self.ob(ObligationEvent::Leaked {
                        obligation: ObligationOrdinal(obligation),
                    });
                }
                self.step(task, TaskStep::Complete);
                self.live[task as usize] = false;
            }
            SubstrateOp::Reserve { task, reservation } => {
                self.acquire(self.tasks[task], *reservation, ObligationKind::Transaction);
            }
            SubstrateOp::Acquire {
                task,
                reservation,
                kind,
            } => self.acquire(self.tasks[task], *reservation, *kind),
            SubstrateOp::Commit { reservation } | SubstrateOp::Abort { reservation } => {
                let obligation = self.labels[reservation];
                let entry = self.obligations[obligation as usize];
                self.wake(entry.holder);
                self.held[entry.holder as usize].retain(|o| *o != obligation);
                self.obligations[obligation as usize].standing = Standing::Discharged;
                let committed = matches!(op, SubstrateOp::Commit { .. });
                if let Some(reservation) = entry.reservation {
                    self.script.push(Report::Effect(if committed {
                        EffectReport::Commit {
                            reservation: ReservationOrdinal(reservation),
                        }
                    } else {
                        EffectReport::Abort {
                            reservation: ReservationOrdinal(reservation),
                            cause: AbortCause::Explicit,
                        }
                    }));
                }
                self.ob(ObligationEvent::Discharged {
                    obligation: ObligationOrdinal(obligation),
                    how: if committed {
                        Discharge::Committed
                    } else {
                        Discharge::Aborted
                    },
                });
                self.park(entry.holder);
            }
            SubstrateOp::Sleep { .. } | SubstrateOp::Advance { .. } => {
                unreachable!("the ledger corpus does not sleep")
            }
            SubstrateOp::Transfer { reservation, to } => {
                let obligation = self.labels[reservation];
                let source = self.obligations[obligation as usize].holder;
                let destination = self.tasks[to];
                self.wake(source);
                self.held[source as usize].retain(|o| *o != obligation);
                self.park(source);
                self.held[destination as usize].push(obligation);
                let region = self.task_region[destination as usize];
                let entry = &mut self.obligations[obligation as usize];
                entry.holder = destination;
                entry.region = region;
                self.ob(ObligationEvent::Transferred {
                    obligation: ObligationOrdinal(obligation),
                    holder: TaskOrdinal(destination),
                    region: RegionOrdinal(region),
                });
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
                for obligation in &held {
                    if let Some(reservation) = self.obligations[*obligation as usize].reservation {
                        self.script.push(Report::Effect(EffectReport::Abort {
                            reservation: ReservationOrdinal(reservation),
                            cause: AbortCause::Cancel,
                        }));
                    }
                }
                for obligation in held {
                    self.obligations[obligation as usize].standing = Standing::Discharged;
                    self.ob(ObligationEvent::Discharged {
                        obligation: ObligationOrdinal(obligation),
                        how: Discharge::Aborted,
                    });
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
            let members = |standing: Standing| {
                ObligationSet::new(
                    self.obligations
                        .iter()
                        .enumerate()
                        .filter(|(_, o)| o.region == member && o.standing == standing)
                        .map(|(i, _)| ObligationOrdinal(u32::try_from(i).unwrap())),
                )
            };
            let (open, leaked) = (members(Standing::Open), members(Standing::Leaked));
            self.ob(ObligationEvent::RegionSettled {
                region: RegionOrdinal(member),
                open,
                leaked,
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
    let mut all = corpus();
    all.push(leaking());
    for (name, programs, logs, _) in all {
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
    assert_eq!(compared, LEDGER_LOGS + CLOSING_LOGS + LEAKING_LOGS);
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
/// the IMPL-01, IMPL-02 and IMPL-03 journals are unchanged by this bone.
#[test]
fn dropping_a_family_gives_that_projection_byte_for_byte() {
    use Family::{Cancellation, Effect, Obligation};
    let subsets: [&[Family]; 7] = [
        &[Obligation],
        &[Effect],
        &[Cancellation],
        &[Effect, Cancellation],
        &[Effect, Obligation],
        &[Cancellation, Obligation],
        &[Effect, Cancellation, Obligation],
    ];
    for (name, programs, logs, _) in corpus() {
        for log in logs.iter().step_by(41) {
            let full = run(&programs, log, &config(SEED)).unwrap();
            for dropped in subsets {
                let mut projection = BindingConfig::new(SEED);
                for family in [Effect, Cancellation, Obligation] {
                    if !dropped.contains(&family) {
                        projection = projection.observing(family);
                    }
                }
                let expected = run(&programs, log, &projection).unwrap();
                assert_eq!(
                    without(&full, dropped).encode().unwrap(),
                    expected.encode().unwrap(),
                    "{name} log {log} without {dropped:?}"
                );
            }
        }
    }
    // A leak is never dropped: without the obligations family the run is refused.
    let (_, programs, logs, _) = leaking();
    let refusal = run(&programs, &logs[0], &BindingConfig::new(SEED)).unwrap_err();
    assert_eq!(
        refusal,
        BindingRefusal::UninstrumentedEvent {
            family: Some(Obligation),
            kind: "ObligationLeak".to_owned()
        }
    );
}

// --- conformance ---------------------------------------------------------------------

#[test]
fn balanced_runs_conform_and_leaks_violate_at_close() {
    for (name, programs, logs, _) in corpus() {
        let finalized = match name {
            "ledger" => 3,
            _ => 1,
        };
        for log in &logs {
            let journal = run(&programs, log, &config(SEED)).unwrap();
            let journal = Journal::decode(&journal.encode().unwrap()).unwrap();
            let LiftVerdict::Conforms(lifted) = lift(&journal) else {
                panic!("{name} log {log}:\n{}", journal.render());
            };
            assert_eq!(lifted.finalizations().len(), finalized);
            for (_, finalization) in lifted.finalizations() {
                assert!(finalization.orphans().is_empty(), "{name} log {log}");
            }
            let settled = ledger_events(&journal)
                .iter()
                .filter(|e| matches!(e, ObligationEvent::RegionSettled { .. }))
                .count();
            assert_eq!(settled, finalized, "{name} log {log}");
        }
    }
    // Every leaking run is refused by the lift at the leaking region's close, naming the
    // leaked ack (the third obligation, o2): the calculus's teardown is total, but the
    // substrate's ledger does not balance.
    let (_, programs, logs, _) = leaking();
    for log in &logs {
        let journal = run(&programs, log, &config(SEED)).unwrap();
        match lift(&journal) {
            LiftVerdict::Violates {
                reason:
                    Nonconformance::Obligation(LedgerFault::UnbalancedAtClose {
                        region: 1,
                        open,
                        leaked,
                    }),
                ..
            } => {
                assert!(open.is_empty(), "log {log}");
                assert_eq!(leaked.len(), 1, "log {log}");
            }
            other => panic!("log {log}: {other:?}\n{}", journal.render()),
        }
    }
}

/// One run, pinned in full: kinds, a transfer across sibling regions, the cancellation's
/// discharges inside the holder's phases, and each region settled after it finalizes.
#[test]
fn a_cancelled_subtree_settles_its_ledger_in_canonical_order() {
    let programs = vec![ledger().remove(1)];
    let journal = run(&programs, &ChoiceLog::new([0; 11]), &config(SEED)).unwrap();
    assert_eq!(
        journal.render(),
        "0 lifecycle region-opened r1 parent=r0
1 lifecycle region-opened r2 parent=r1
2 lifecycle region-opened r3 parent=r1
3 lifecycle task-spawned t0 region=r2 resumable
4 lifecycle task-spawned t1 region=r3 resumable
5 lifecycle task-stepped t0 begin
6 lifecycle task-stepped t0 suspend
7 lifecycle task-stepped t1 begin
8 lifecycle task-stepped t1 suspend
9 lifecycle task-stepped t0 resume
10 lifecycle task-stepped t0 suspend
11 obligation opened o0 ack holder=t0 region=r2
12 lifecycle task-stepped t1 resume
13 lifecycle task-stepped t1 suspend
14 obligation opened o1 io-op holder=t1 region=r3
15 lifecycle task-stepped t1 resume
16 lifecycle task-stepped t1 suspend
17 obligation transferred o1 holder=t0 region=r2
18 lifecycle region-cancel-requested r1
19 cancellation requested t0 cause=parent-cancelled
20 cancellation requested t1 cause=parent-cancelled
21 cancellation acknowledged t0
22 obligation discharged o0 aborted
23 obligation discharged o1 aborted
24 cancellation cancelled t0 cause=parent-cancelled
25 lifecycle region-drained r2 cancelled=[t0]
26 lifecycle region-finalized r2
27 obligation region-settled r2 open=[] leaked=[]
28 cancellation acknowledged t1
29 cancellation cancelled t1 cause=parent-cancelled
30 lifecycle region-drained r3 cancelled=[t1]
31 lifecycle region-finalized r3
32 obligation region-settled r3 open=[] leaked=[]
33 lifecycle region-drained r1 cancelled=[]
34 lifecycle region-finalized r1
35 obligation region-settled r1 open=[] leaked=[]
"
    );
    let bytes = journal.encode().unwrap();
    assert_eq!(Journal::decode(&bytes).unwrap(), journal);
    assert!(matches!(lift(&journal), LiftVerdict::Conforms(_)));
}

// --- anti-vacuity: the lift rejects wrong ledgers -------------------------------------

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

fn position(bodies: &[EventBody], wanted: impl Fn(&ObligationEvent) -> bool) -> usize {
    bodies
        .iter()
        .position(|body| matches!(body, EventBody::Obligation(event) if wanted(event)))
        .expect("the journal has such an event")
}

fn ledger_fault(journal: &Journal) -> LedgerFault {
    match lift(journal) {
        LiftVerdict::Violates {
            reason: Nonconformance::Obligation(fault),
            ..
        } => fault,
        other => panic!(
            "expected a ledger fault, got {other:?}\n{}",
            journal.render()
        ),
    }
}

fn is_cancel_discharge(event: &ObligationEvent) -> bool {
    // In the ledger corpus, only the cancellation aborts.
    matches!(
        event,
        ObligationEvent::Discharged {
            how: Discharge::Aborted,
            ..
        }
    )
}

/// Eight mutation operators over real substrate journals, each rejected with its own
/// fault. The unmutated journal conforms, so each rejection is the mutant's doing.
#[test]
fn mutated_ledger_journals_are_rejected() {
    let programs = ledger();
    let logs = ChoiceLog::enumerate(&lengths(&programs));
    let mut mutants = 0;
    for log in logs.iter().step_by(29) {
        let journal = run(&programs, log, &config(SEED)).unwrap();
        assert!(matches!(lift(&journal), LiftVerdict::Conforms(_)));
        let original = bodies(&journal);
        let fault_of = |events: Vec<EventBody>| ledger_fault(&rebuilt(events));

        // 1. A double resolve.
        let mut events = original.clone();
        let at = position(&events, |e| matches!(e, ObligationEvent::Discharged { .. }));
        events.insert(at + 1, events[at].clone());
        assert!(
            matches!(
                fault_of(events),
                LedgerFault::NotOpen {
                    event: "discharged",
                    state: "discharged",
                    ..
                }
            ),
            "log {log}"
        );

        // 2. A leak: a cancellation discharge becomes a leak, and its region's reported
        //    balance says so too. The region then closes unbalanced.
        let mut events = original.clone();
        let at = position(&events, is_cancel_discharge);
        let EventBody::Obligation(ObligationEvent::Discharged { obligation, .. }) = events[at]
        else {
            unreachable!()
        };
        events[at] = EventBody::Obligation(ObligationEvent::Leaked { obligation });
        let settle = position(
            &events,
            |e| matches!(e, ObligationEvent::RegionSettled { region, .. } if region.0 == 2),
        );
        events[settle] = EventBody::Obligation(ObligationEvent::RegionSettled {
            region: RegionOrdinal(2),
            open: ObligationSet::default(),
            leaked: ObligationSet::new([obligation]),
        });
        assert!(
            matches!(
                fault_of(events),
                LedgerFault::UnbalancedAtClose { region: 2, .. }
            ),
            "log {log}"
        );

        // 3. The same leak without the balance: the reported balance is not the ledger's.
        let mut events = original.clone();
        let at = position(&events, is_cancel_discharge);
        let EventBody::Obligation(ObligationEvent::Discharged { obligation, .. }) = events[at]
        else {
            unreachable!()
        };
        events[at] = EventBody::Obligation(ObligationEvent::Leaked { obligation });
        assert!(
            matches!(
                fault_of(events),
                LedgerFault::BalanceMismatch { region: 2, .. }
            ),
            "log {log}"
        );

        // 4. A cross-region transfer that names the source's region.
        let mut events = original.clone();
        let at = position(&events, |e| {
            matches!(e, ObligationEvent::Transferred { .. })
        });
        if let EventBody::Obligation(ObligationEvent::Transferred { region, .. }) = &mut events[at]
        {
            *region = RegionOrdinal(3);
        }
        assert!(
            matches!(
                fault_of(events),
                LedgerFault::HolderRegionMismatch {
                    journal: 3,
                    model: 2,
                    ..
                }
            ),
            "log {log}"
        );

        // 5. A transfer to the holder the obligation already has.
        let mut events = original.clone();
        let at = position(&events, |e| {
            matches!(e, ObligationEvent::Transferred { .. })
        });
        let EventBody::Obligation(ObligationEvent::Transferred { obligation, .. }) = events[at]
        else {
            unreachable!()
        };
        let opened = position(
            &events,
            |e| matches!(e, ObligationEvent::Opened { obligation: o, .. } if *o == obligation),
        );
        let EventBody::Obligation(ObligationEvent::Opened {
            holder: first,
            region: home,
            ..
        }) = events[opened]
        else {
            unreachable!()
        };
        if let EventBody::Obligation(ObligationEvent::Transferred { holder, region, .. }) =
            &mut events[at]
        {
            *holder = first;
            *region = home;
        }
        assert!(
            matches!(fault_of(events), LedgerFault::SameHolder { .. }),
            "log {log}"
        );

        // 6. A cancellation discharge after the drain that terminated its holder.
        let mut events = original.clone();
        let at = position(&events, is_cancel_discharge);
        let discharge = events.remove(at);
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
        events.insert(drain + 1, discharge);
        assert!(
            matches!(fault_of(events), LedgerFault::HolderTerminal { .. }),
            "log {log}"
        );

        // 7. A region's settlement dropped.
        let mut events = original.clone();
        let at = position(&events, |e| {
            matches!(e, ObligationEvent::RegionSettled { .. })
        });
        let EventBody::Obligation(ObligationEvent::RegionSettled { region, .. }) =
            events.remove(at)
        else {
            unreachable!()
        };
        assert_eq!(
            fault_of(events),
            LedgerFault::Unsettled { region: region.0 },
            "log {log}"
        );

        // 8. An obligation renumbered.
        let mut events = original.clone();
        let at = position(&events, |e| matches!(e, ObligationEvent::Opened { .. }));
        if let EventBody::Obligation(ObligationEvent::Opened { obligation, .. }) = &mut events[at] {
            obligation.0 += 5;
        }
        assert!(
            matches!(fault_of(events), LedgerFault::ObligationIdentity { .. }),
            "log {log}"
        );
        mutants += 8;
    }
    assert!(mutants > 3000, "{mutants} mutants");
}

// --- refusals ------------------------------------------------------------------------

fn refusal(programs: Vec<Program>, config: &BindingConfig) -> BindingRefusal {
    let n: usize = programs.iter().map(Vec::len).sum();
    run(&programs, &ChoiceLog::new(vec![0; n]), config).unwrap_err()
}

#[test]
fn refusals_are_typed() {
    let (a, b) = (TaskLabel(1), TaskLabel(2));
    let r1 = RegionLabel(1);
    let two = || {
        vec![
            spawn(RegionLabel::ROOT, a),
            spawn(RegionLabel::ROOT, b),
            begin(a),
            begin(b),
        ]
    };
    let with = |tail: Vec<SubstrateOp>| {
        let mut program = two();
        program.extend(tail);
        vec![program]
    };

    // Malformed transfers: no INV-008 reading.
    let malformed = [
        (
            with(vec![transfer(9, b)]),
            BindingRefusal::UnboundReservation(9),
        ),
        (
            with(vec![reserve(a, 1), transfer(1, b)]),
            BindingRefusal::EffectNotTransferable(1),
        ),
        (
            with(vec![
                acquire(a, 1, ObligationKind::Lease),
                commit(1),
                transfer(1, b),
            ]),
            BindingRefusal::ReservationResolved(1),
        ),
    ];
    for (programs, expected) in malformed {
        let got = refusal(programs, &config(SEED));
        assert_eq!(got, expected);
        assert_eq!(got.inconclusive_reason(), None, "{got}");
    }

    // A transfer to the holder itself: the substrate refuses it.
    let got = refusal(
        with(vec![acquire(a, 1, ObligationKind::Lease), transfer(1, a)]),
        &config(SEED),
    );
    assert!(
        matches!(
            got,
            BindingRefusal::SubstrateRefused {
                operation: "transfer",
                ..
            }
        ),
        "{got}"
    );

    // An acquire sent to a task that has not begun: lost telemetry, never a short
    // journal.
    let got = refusal(
        vec![vec![
            spawn(RegionLabel::ROOT, a),
            acquire(a, 1, ObligationKind::Lease),
        ]],
        &config(SEED),
    );
    assert_eq!(
        got,
        BindingRefusal::EffectUnobserved {
            operation: "acquire"
        }
    );
    assert_eq!(
        got.inconclusive_reason(),
        Some(InconclusiveReason::InsufficientTelemetry)
    );

    // A leak the configuration cannot report is refused, not dropped.
    let leaking = vec![vec![
        open(RegionLabel::ROOT, r1),
        spawn(r1, a),
        begin(a),
        acquire(a, 1, ObligationKind::Lease),
        SubstrateOp::Finish { task: a },
    ]];
    let got = refusal(leaking.clone(), &BindingConfig::new(SEED));
    assert_eq!(
        got.inconclusive_reason(),
        Some(InconclusiveReason::Unsupported)
    );
    // With the family observed, the same run is a journal the lift then judges.
    let n = leaking[0].len();
    assert!(run(&leaking, &ChoiceLog::new(vec![0; n]), &config(SEED)).is_ok());

    // The INV-008 reading of the cross-check refusal no bound program reaches today.
    assert_eq!(
        BindingRefusal::SubstrateLedgerDisagrees {
            detail: "x".to_owned()
        }
        .inconclusive_reason(),
        Some(InconclusiveReason::EngineError)
    );

    // A family the binding does not bind is refused before the substrate runs.
    let got = refusal(two().into_iter().map(|op| vec![op]).collect(), &{
        config(SEED).observing(Family::Channel)
    });
    assert_eq!(got, BindingRefusal::FamilyNotBound(Family::Channel));

    // Bytes: an unknown kind tag and an unsorted balance set are refused, not guessed.
    let mut journal = Journal::new();
    journal
        .append(EventBody::Obligation(ObligationEvent::Opened {
            obligation: ObligationOrdinal(0),
            kind: ObligationKind::Lease,
            holder: TaskOrdinal(0),
            region: RegionOrdinal(0),
        }))
        .unwrap();
    let mut bad_kind = journal.encode().unwrap();
    let kind_at = bad_kind.len() - 9;
    bad_kind[kind_at] = 9;
    assert!(matches!(
        Journal::decode(&bad_kind),
        Err(DecodeError::UnknownTag {
            table: "obligation kind",
            tag: 9,
            ..
        })
    ));
    let mut journal = Journal::new();
    journal
        .append(EventBody::Obligation(ObligationEvent::RegionSettled {
            region: RegionOrdinal(0),
            open: ObligationSet::default(),
            leaked: ObligationSet::new([ObligationOrdinal(1), ObligationOrdinal(2)]),
        }))
        .unwrap();
    let mut unsorted = journal.encode().unwrap();
    let len = unsorted.len();
    unsorted.swap(len - 1, len - 5);
    assert!(matches!(
        Journal::decode(&unsorted),
        Err(DecodeError::UnsortedSet { .. })
    ));
}

/// INV-005: the family's code names no filesystem, environment, clock or thread path.
/// The binding's own barrier test (`pr14_impl01_binding.rs`) covers `binding.rs`.
#[test]
fn the_obligation_family_names_no_ambient_path() {
    let source = include_str!("../src/family/obligation.rs");
    let code: String = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for ambient in ["std::fs", "std::env", "std::time", "std::thread", "rand"] {
        assert!(!code.contains(ambient), "obligation.rs names {ambient}");
    }
    assert!(code.contains("pub(crate) fn lift"), "the scan sees code");
}
