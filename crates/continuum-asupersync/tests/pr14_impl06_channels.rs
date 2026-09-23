//! PR-14-IMPL-06 (bn-3xx9): channel communication, observed from the real substrate.
//!
//! > **Exit:** identical controlled choice logs produce canonical identical semantic
//! > events.
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 14
//!
//! The binding (`binding.rs`) drives asupersync 0.5.0's lab runtime under a choice log.
//! It opens bounded `channel::mpsc` channels and hands each receiver to one task; bound
//! tasks send through clones of the binding's sender (`reserve_checked`, then
//! `SendPermit::send`) and receive with `Receiver::recv`. With the channel family
//! observed, the binding journals each send from the substrate's own trace (the
//! `SendPermit` commit) and each receive, block, close and drop from the channel gate's
//! marks, with message identity the payload the substrate delivered.
//!
//! # Evidence map
//!
//! | Claim | Test |
//! |---|---|
//! | identical choice logs → byte-identical journals, every log (sibling regions, three producers, backpressure) | [`identical_choice_logs_give_byte_identical_channel_journals`] |
//! | the lab seed does not reach the journal, though it reorders woken senders and sibling closes (7 seeds) | [`the_lab_seed_does_not_reach_the_channel_journal`] |
//! | program labels do not reach the journal | [`renaming_labels_does_not_change_the_journal`] |
//! | different choice logs → different journals (anti-vacuity of the identity) | [`different_choice_logs_give_different_channel_journals`] |
//! | substrate journal ≡ an independent scripted account, every log (differential) | [`the_substrate_agrees_byte_for_byte_with_a_scripted_account`] |
//! | the family adds events and changes no byte of the other five families' journals | [`dropping_a_family_gives_that_projection_byte_for_byte`] |
//! | every journal lifts into the parallel channel model | [`every_channel_journal_conforms`] |
//! | one pinned run, rendered: a cancelled blocked receiver drops its queue | [`a_cancelled_blocked_receiver_drops_its_queue`] |
//! | mutated journals are rejected, each with its own fault (anti-vacuity) | [`mutated_channel_journals_are_rejected`] |
//! | malformed channel programs, commands to blocked tasks, bad bytes: typed refusals | [`refusals_are_typed`] |

use std::collections::{BTreeMap, VecDeque};

use continuum_asupersync::binding::{
    BindingConfig, BindingRefusal, Program, SubstrateOp, run, run_witnessed,
};
use continuum_asupersync::choice::ChoiceLog;
use continuum_asupersync::encoding::DecodeError;
use continuum_asupersync::family::cancellation::{CancelCause, CancellationReport};
use continuum_asupersync::family::channel::{
    ChannelEvent, ChannelFault, ChannelLabel, ChannelOrdinal, ChannelReport, MessageOrdinal,
    MessageSet,
};
use continuum_asupersync::family::lifecycle::{
    LifecycleEvent, LifecycleReport, RegionLabel, RegionOrdinal, TaskLabel, TaskOrdinal, TaskStep,
};
use continuum_asupersync::family::obligation::{
    Discharge, ObligationEvent, ObligationKind, ObligationOrdinal, ObligationReport, ObligationSet,
};
use continuum_asupersync::family::{EventBody, Family, Report};
use continuum_asupersync::journal::Journal;
use continuum_asupersync::lift::{LiftVerdict, Nonconformance, lift};
use continuum_asupersync::source::{Script, record};
use continuum_task::region::worker::Resumability;
use continuum_value::assurance::InconclusiveReason;

const SEED: u64 = 0;
const SEEDS: [u64; 7] = [0, 1, 7, 42, 99, 0x9e37_79b9_7f4a_7c15, u64::MAX];

/// The configuration the tests run: every family observed.
fn config(seed: u64) -> BindingConfig {
    BindingConfig::new(seed)
        .observing(Family::Effect)
        .observing(Family::Cancellation)
        .observing(Family::Obligation)
        .observing(Family::Time)
        .observing(Family::Channel)
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

fn send(task: TaskLabel, channel: ChannelLabel) -> SubstrateOp {
    SubstrateOp::Send { task, channel }
}

fn recv(channel: ChannelLabel) -> SubstrateOp {
    SubstrateOp::Recv { channel }
}

/// The labels a corpus spells its names with. Renaming them must not change a journal.
#[derive(Clone, Copy)]
struct Names {
    tasks: [u32; 6],
    regions: [u32; 3],
    channels: [u32; 2],
}

const NAMES: Names = Names {
    tasks: [1, 2, 3, 4, 5, 6],
    regions: [1, 2, 3],
    channels: [1, 2],
};

const RENAMED: Names = Names {
    tasks: [60, 50, 40, 30, 20, 10],
    regions: [9, 8, 7],
    channels: [77, 66],
};

/// The `cancelling` corpus: a setup every run starts with, in a fixed order, and six
/// actors whose operations the choice log interleaves.
///
/// ```text
/// root: p1, p3 (producers on c1), p4, p5 (producers on c2), rx2 (receives c2)
/// r1 ─┬─ r2: rx (receives c1, capacity 1)
///     └─ r3: p2 (producer on c2, capacity 2)
/// ```
///
/// On c1, two root producers race one message each into a channel of capacity 1, so one
/// can block until the receiver takes a message (backpressure). The receiver receives
/// once, then r1 is cancelled: it can find rx blocked (an abandoned receive) or holding
/// a queued message (dropped with its receiver), and the drop wakes every sender still
/// blocked on c1, which then fails as closed. On c2 three producers race into capacity 2
/// and rx2 receives once; the cancellation can find p2 blocked on c2 (an abandoned
/// send).
fn cancelling(names: Names) -> (Program, Vec<Program>) {
    let t = |i: usize| TaskLabel(names.tasks[i]);
    let r = |i: usize| RegionLabel(names.regions[i]);
    let c = |i: usize| ChannelLabel(names.channels[i]);
    let (p1, p2, p3, rx, p4, rx2) = (t(0), t(1), t(2), t(3), t(4), t(5));
    let p5 = TaskLabel(names.tasks[0] + 1000);
    let setup = vec![
        open(RegionLabel::ROOT, r(0)),
        open(r(0), r(1)),
        open(r(0), r(2)),
        spawn(RegionLabel::ROOT, p1),
        spawn(r(2), p2),
        spawn(RegionLabel::ROOT, p3),
        spawn(r(1), rx),
        spawn(RegionLabel::ROOT, p4),
        spawn(RegionLabel::ROOT, rx2),
        spawn(RegionLabel::ROOT, p5),
        begin(p1),
        begin(p2),
        begin(p3),
        begin(rx),
        begin(p4),
        begin(rx2),
        begin(p5),
        SubstrateOp::OpenChannel {
            channel: c(0),
            capacity: 1,
            receiver: rx,
        },
        SubstrateOp::OpenChannel {
            channel: c(1),
            capacity: 2,
            receiver: rx2,
        },
    ];
    let actors = vec![
        vec![send(p1, c(0))],
        vec![send(p3, c(0))],
        vec![
            send(p2, c(1)),
            recv(c(0)),
            SubstrateOp::Cancel { region: r(0) },
        ],
        vec![send(p4, c(1))],
        vec![send(p5, c(1))],
        vec![recv(c(1))],
    ];
    (setup, actors)
}

/// The `closing` corpus. Three root producers race one message each into c1 (capacity
/// 1), and its receiver's task finishes: the receiver goes, dropping what is queued and
/// waking every sender still blocked, which fail as closed together — the substrate's
/// scheduler picks their order, and the journal orders them by task. On c2, the
/// binding's sender is dropped while its receiver receives once: closed, before or after
/// the receive blocks.
fn closing(names: Names) -> (Program, Vec<Program>) {
    let t = |i: usize| TaskLabel(names.tasks[i]);
    let c = |i: usize| ChannelLabel(names.channels[i]);
    let (q1, q2, q3, rxq, rxe) = (t(0), t(1), t(2), t(3), t(4));
    let mut setup = Vec::new();
    for task in [q1, q2, q3, rxq, rxe] {
        setup.push(spawn(RegionLabel::ROOT, task));
        setup.push(begin(task));
    }
    setup.push(SubstrateOp::OpenChannel {
        channel: c(0),
        capacity: 1,
        receiver: rxq,
    });
    setup.push(SubstrateOp::OpenChannel {
        channel: c(1),
        capacity: 1,
        receiver: rxe,
    });
    let actors = vec![
        vec![send(q1, c(0))],
        vec![send(q2, c(0))],
        vec![send(q3, c(0))],
        vec![SubstrateOp::Finish { task: rxq }],
        vec![SubstrateOp::CloseSenders { channel: c(1) }],
        vec![recv(c(1))],
    ];
    (setup, actors)
}

const CANCELLING_LOGS: usize = 6_720; // 8! / 3!
const CLOSING_LOGS: usize = 720; // 6!
const ALL_LOGS: usize = CANCELLING_LOGS + CLOSING_LOGS;

/// A corpus entry: name, programs (the setup as actor 0), logs. A log runs the setup
/// first, then its own choices, which index the actors that remain enabled.
type Entry = (&'static str, Vec<Program>, Vec<ChoiceLog>);

fn entry(name: &'static str, (setup, actors): (Program, Vec<Program>)) -> Entry {
    let lengths: Vec<usize> = actors.iter().map(Vec::len).collect();
    let logs: Vec<ChoiceLog> = ChoiceLog::enumerate(&lengths)
        .into_iter()
        .map(|log| {
            let mut choices = vec![0; setup.len()];
            choices.extend(log.choices().iter().map(|c| c.0));
            ChoiceLog::new(choices)
        })
        .collect();
    let mut programs = vec![setup];
    programs.extend(actors);
    (name, programs, logs)
}

fn corpus(names: Names) -> Vec<Entry> {
    let all = vec![
        entry("cancelling", cancelling(names)),
        entry("closing", closing(names)),
    ];
    assert_eq!(all[0].2.len(), CANCELLING_LOGS);
    assert_eq!(all[1].2.len(), CLOSING_LOGS);
    all
}

fn channel_events(journal: &Journal) -> Vec<&ChannelEvent> {
    journal
        .events()
        .iter()
        .filter_map(|event| match event.body() {
            EventBody::Channel(event) => Some(event),
            _ => None,
        })
        .collect()
}

// --- the exit property ---------------------------------------------------------------

#[test]
fn identical_choice_logs_give_byte_identical_channel_journals() {
    let mut blocked = 0;
    let mut abandoned = 0;
    let mut dropped = 0;
    let mut closed_together = 0;
    let mut compared = 0;
    for (name, programs, logs) in corpus(NAMES) {
        for log in &logs {
            let first = run(&programs, log, &config(SEED)).expect("every log is a legal run");
            let second = run(&programs, log, &config(SEED)).expect("every log is a legal run");
            assert_eq!(
                first.encode().unwrap(),
                second.encode().unwrap(),
                "{name} log {log}"
            );
            assert_eq!(first.digest().unwrap(), second.digest().unwrap());
            let events = channel_events(&first);
            let count =
                |wanted: fn(&ChannelEvent) -> bool| events.iter().filter(|e| wanted(e)).count();
            if count(|e| matches!(e, ChannelEvent::SendBlocked { .. })) > 0 {
                blocked += 1;
            }
            if count(|e| matches!(e, ChannelEvent::RecvAbandoned { .. })) > 0 {
                abandoned += 1;
            }
            if events.iter().any(|e| {
                matches!(e, ChannelEvent::ReceiverGone { discarded, .. } if !discarded.as_slice().is_empty())
            }) {
                dropped += 1;
            }
            if count(|e| matches!(e, ChannelEvent::SendClosed { .. })) > 1 {
                closed_together += 1;
            }
            compared += 1;
        }
    }
    assert_eq!(compared, ALL_LOGS);
    // Non-vacuity: backpressure, a cancelled blocked receiver, a declared drop, and
    // several senders failing as closed at once all happen.
    assert!(blocked > 1_000, "{blocked} runs blocked a sender");
    assert!(
        abandoned > 100,
        "{abandoned} runs abandoned a blocked receive"
    );
    assert!(dropped > 100, "{dropped} runs dropped queued messages");
    assert!(
        closed_together > 50,
        "{closed_together} runs closed senders together"
    );
}

#[test]
fn the_lab_seed_does_not_reach_the_channel_journal() {
    let mut compared = 0;
    let mut wakes_reordered = 0;
    let mut closes_reordered = 0;
    for (name, programs, logs) in corpus(NAMES) {
        for log in logs.iter().step_by(2) {
            let reference = run_witnessed(&programs, log, &config(SEEDS[0])).unwrap();
            for seed in &SEEDS[1..] {
                let other = run_witnessed(&programs, log, &config(*seed)).unwrap();
                assert_eq!(
                    reference.journal.encode().unwrap(),
                    other.journal.encode().unwrap(),
                    "{name} log {log} seed {seed:#x}"
                );
                if reference.substrate_wake_order != other.substrate_wake_order {
                    wakes_reordered += 1;
                }
                if reference.substrate_close_order != other.substrate_close_order {
                    closes_reordered += 1;
                }
                compared += 1;
            }
        }
    }
    assert!(compared > 20_000, "the relation ran over {compared} pairs");
    // Anti-vacuity: the seed does reorder what the journal canonicalizes.
    assert!(
        wakes_reordered > 0,
        "no seed moved the substrate's wake order"
    );
    assert!(
        closes_reordered > 0,
        "no seed moved the substrate's close order"
    );
}

/// Labels are program-local names: renaming every task, region and channel label
/// changes no byte of any journal (the lesson of bn-iey9f).
#[test]
fn renaming_labels_does_not_change_the_journal() {
    let mut compared = 0;
    for ((name, programs, logs), (_, renamed, _)) in corpus(NAMES).into_iter().zip(corpus(RENAMED))
    {
        for log in logs.iter().step_by(3) {
            assert_eq!(
                run(&programs, log, &config(SEED)).unwrap(),
                run(&renamed, log, &config(SEED)).unwrap(),
                "{name} log {log}"
            );
            compared += 1;
        }
    }
    assert!(compared > 2_000, "{compared} renamings");
}

#[test]
fn different_choice_logs_give_different_channel_journals() {
    for (name, programs, logs) in corpus(NAMES) {
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

struct Chan {
    capacity: usize,
    receiver: u32,
    queue: VecDeque<u64>,
    receiver_present: bool,
    sender_kept: bool,
    blocked_senders: VecDeque<(u32, u64)>,
    receive_blocked: bool,
}

/// The test's own account of a run: the region tree, the tasks and their gates, each
/// channel's queue, blocked senders and receiver, and the obligations the sends make. It
/// calls nothing in the adapter or in the substrate.
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
    channels: BTreeMap<ChannelLabel, u32>,
    chans: Vec<Chan>,
    next_message: u64,
    next_obligation: u32,
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

    fn ch(&mut self, event: ChannelEvent) {
        self.script.push(Report::Channel(ChannelReport(event)));
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

    /// A send that got its slot: the permit opens and commits, then the message queues.
    fn sent(&mut self, c: u32, message: u64, sender: u32) {
        let obligation = self.next_obligation;
        self.next_obligation += 1;
        let region = self.task_region[sender as usize];
        self.script.push(Report::Obligation(ObligationReport(
            ObligationEvent::Opened {
                obligation: ObligationOrdinal(obligation),
                kind: ObligationKind::SendPermit,
                holder: TaskOrdinal(sender),
                region: RegionOrdinal(region),
            },
        )));
        self.script.push(Report::Obligation(ObligationReport(
            ObligationEvent::Discharged {
                obligation: ObligationOrdinal(obligation),
                how: Discharge::Committed,
            },
        )));
        self.ch(ChannelEvent::Sent {
            channel: ChannelOrdinal(c),
            message: MessageOrdinal(message),
            sender: TaskOrdinal(sender),
        });
        self.chans[c as usize].queue.push_back(message);
    }

    /// The blocked receiver of `c`, woken: it takes a message or finds the channel
    /// closed, then parks on its gate.
    fn wake_receiver(&mut self, c: u32) {
        let chan = &self.chans[c as usize];
        if !chan.receive_blocked {
            return;
        }
        let receiver = chan.receiver;
        if let Some(message) = chan.queue.front().copied() {
            self.chans[c as usize].queue.pop_front();
            self.chans[c as usize].receive_blocked = false;
            self.wake(receiver);
            self.ch(ChannelEvent::Received {
                channel: ChannelOrdinal(c),
                message: MessageOrdinal(message),
            });
            self.park(receiver);
        } else if !chan.sender_kept && chan.blocked_senders.is_empty() {
            self.chans[c as usize].receive_blocked = false;
            self.wake(receiver);
            self.ch(ChannelEvent::RecvClosed {
                channel: ChannelOrdinal(c),
            });
            self.park(receiver);
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
            SubstrateOp::OpenChannel {
                channel,
                capacity,
                receiver,
            } => {
                let c = u32::try_from(self.chans.len()).unwrap();
                self.channels.insert(*channel, c);
                let receiver = self.tasks[receiver];
                self.chans.push(Chan {
                    capacity: *capacity as usize,
                    receiver,
                    queue: VecDeque::new(),
                    receiver_present: true,
                    sender_kept: true,
                    blocked_senders: VecDeque::new(),
                    receive_blocked: false,
                });
                self.ch(ChannelEvent::Opened {
                    channel: ChannelOrdinal(c),
                    capacity: *capacity,
                    receiver: TaskOrdinal(receiver),
                });
            }
            SubstrateOp::Send { task, channel } => {
                let c = self.channels[channel];
                let sender = self.tasks[task];
                let message = self.next_message;
                self.next_message += 1;
                self.wake(sender);
                let chan = &self.chans[c as usize];
                if !chan.receiver_present {
                    self.ch(ChannelEvent::SendClosed {
                        channel: ChannelOrdinal(c),
                        message: MessageOrdinal(message),
                        sender: TaskOrdinal(sender),
                    });
                    self.park(sender);
                } else if chan.queue.len() < chan.capacity {
                    self.park(sender);
                    self.sent(c, message, sender);
                    self.wake_receiver(c);
                } else {
                    self.ch(ChannelEvent::SendBlocked {
                        channel: ChannelOrdinal(c),
                        message: MessageOrdinal(message),
                        sender: TaskOrdinal(sender),
                    });
                    self.park(sender);
                    self.chans[c as usize]
                        .blocked_senders
                        .push_back((sender, message));
                }
            }
            SubstrateOp::Recv { channel } => {
                let c = self.channels[channel];
                let receiver = self.chans[c as usize].receiver;
                self.wake(receiver);
                let chan = &self.chans[c as usize];
                if let Some(message) = chan.queue.front().copied() {
                    self.chans[c as usize].queue.pop_front();
                    self.ch(ChannelEvent::Received {
                        channel: ChannelOrdinal(c),
                        message: MessageOrdinal(message),
                    });
                    self.park(receiver);
                    // The freed slot goes to the oldest blocked sender.
                    if let Some((sender, message)) =
                        self.chans[c as usize].blocked_senders.pop_front()
                    {
                        self.wake(sender);
                        self.park(sender);
                        self.sent(c, message, sender);
                    }
                } else if !chan.sender_kept && chan.blocked_senders.is_empty() {
                    self.ch(ChannelEvent::RecvClosed {
                        channel: ChannelOrdinal(c),
                    });
                    self.park(receiver);
                } else {
                    self.ch(ChannelEvent::RecvBlocked {
                        channel: ChannelOrdinal(c),
                    });
                    self.park(receiver);
                    self.chans[c as usize].receive_blocked = true;
                }
            }
            SubstrateOp::CloseSenders { channel } => {
                let c = self.channels[channel];
                self.chans[c as usize].sender_kept = false;
                self.ch(ChannelEvent::SendersClosed {
                    channel: ChannelOrdinal(c),
                });
                self.wake_receiver(c);
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
                let mut woken: Vec<(u32, u32, u64)> = Vec::new();
                self.teardown(ordinal, &causes, &mut woken);
                self.wake_closed(woken);
            }
            SubstrateOp::Finish { task } => {
                let task = self.tasks[task];
                self.wake(task);
                let mut woken: Vec<(u32, u32, u64)> = Vec::new();
                self.receiver_goes(task, &mut woken);
                self.step(task, TaskStep::Complete);
                self.live[task as usize] = false;
                self.wake_closed(woken);
            }
            other => unreachable!("the channel corpus has no {other:?}"),
        }
    }

    /// `task` ends: each receiver it holds goes with its queue, and the senders blocked
    /// on it are woken.
    fn receiver_goes(&mut self, task: u32, woken: &mut Vec<(u32, u32, u64)>) {
        for c in 0..self.chans.len() {
            let c32 = u32::try_from(c).unwrap();
            if self.chans[c].receiver != task || !self.chans[c].receiver_present {
                continue;
            }
            let discarded = MessageSet::new(
                core::mem::take(&mut self.chans[c].queue)
                    .into_iter()
                    .map(MessageOrdinal),
            );
            self.ch(ChannelEvent::ReceiverGone {
                channel: ChannelOrdinal(c32),
                discarded,
            });
            self.chans[c].receiver_present = false;
            for (sender, message) in core::mem::take(&mut self.chans[c].blocked_senders) {
                woken.push((sender, c32, message));
            }
        }
    }

    /// Senders a gone receiver woke, after the operation: by task ordinal.
    fn wake_closed(&mut self, mut woken: Vec<(u32, u32, u64)>) {
        woken.sort_unstable();
        for (sender, c, message) in woken {
            self.wake(sender);
            self.ch(ChannelEvent::SendClosed {
                channel: ChannelOrdinal(c),
                message: MessageOrdinal(message),
                sender: TaskOrdinal(sender),
            });
            self.park(sender);
        }
    }

    fn teardown(
        &mut self,
        region: u32,
        causes: &BTreeMap<u32, CancelCause>,
        woken: &mut Vec<(u32, u32, u64)>,
    ) {
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
                for c in 0..self.chans.len() {
                    let c32 = u32::try_from(c).unwrap();
                    let position = self.chans[c]
                        .blocked_senders
                        .iter()
                        .position(|(sender, _)| *sender == task);
                    if let Some(position) = position {
                        let (_, message) = self.chans[c].blocked_senders.remove(position).unwrap();
                        self.ch(ChannelEvent::SendAbandoned {
                            channel: ChannelOrdinal(c32),
                            message: MessageOrdinal(message),
                            sender: TaskOrdinal(task),
                        });
                    }
                }
                for c in 0..self.chans.len() {
                    let c32 = u32::try_from(c).unwrap();
                    if self.chans[c].receiver == task
                        && self.chans[c].receiver_present
                        && self.chans[c].receive_blocked
                    {
                        self.chans[c].receive_blocked = false;
                        self.ch(ChannelEvent::RecvAbandoned {
                            channel: ChannelOrdinal(c32),
                        });
                    }
                }
                self.receiver_goes(task, woken);
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
            self.script.push(Report::Obligation(ObligationReport(
                ObligationEvent::RegionSettled {
                    region: RegionOrdinal(member),
                    open: ObligationSet::default(),
                    leaked: ObligationSet::default(),
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
    for (name, programs, logs) in corpus(NAMES) {
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

/// The family is additive: remove any of the five optional families' events from the
/// full journal and what remains is, byte for byte, the journal of a run that does not
/// observe them. So the IMPL-01 to IMPL-05 journals are unchanged by this bone.
#[test]
fn dropping_a_family_gives_that_projection_byte_for_byte() {
    let optional = [
        Family::Effect,
        Family::Cancellation,
        Family::Obligation,
        Family::Time,
        Family::Channel,
    ];
    let mut compared = 0;
    for (_, programs, logs) in corpus(NAMES) {
        for log in logs.iter().step_by(53) {
            let full = run(&programs, log, &config(SEED)).unwrap();
            for mask in 1_u8..32 {
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
                    "log {log} without {dropped:?}"
                );
                compared += 1;
            }
        }
    }
    assert!(compared > 4_000, "{compared} projections");
}

// --- conformance ---------------------------------------------------------------------

#[test]
fn every_channel_journal_conforms() {
    for (_, programs, logs) in corpus(NAMES) {
        for log in &logs {
            let journal = run(&programs, log, &config(SEED)).unwrap();
            let journal = Journal::decode(&journal.encode().unwrap()).unwrap();
            let LiftVerdict::Conforms(lifted) = lift(&journal) else {
                panic!("log {log}:\n{}", journal.render());
            };
            for (_, finalization) in lifted.finalizations() {
                assert!(finalization.orphans().is_empty(), "log {log}");
            }
            // No message is delivered twice.
            let mut received = BTreeMap::new();
            for event in channel_events(&journal) {
                if let ChannelEvent::Received { message, .. } = event {
                    assert!(received.insert(message.0, ()).is_none(), "log {log}");
                }
            }
        }
    }
}

/// One run, pinned: p2 sends on c2; the receiver of c1 blocks; p1's send wakes it and
/// is received; p3's send queues; the cancellation of r1 makes the receiver go and drop
/// p3's message. Then on c2, p4 fills the channel, p5 blocks, and rx2's receive frees the
/// slot p5's send takes.
#[test]
fn a_cancelled_blocked_receiver_drops_its_queue() {
    let (_, programs, _) = entry("cancelling", cancelling(NAMES));
    let (setup, _) = cancelling(NAMES);
    let mut choices = vec![0; setup.len()];
    choices.extend([2, 2, 0, 0, 0, 0, 0, 0]);
    let log = ChoiceLog::new(choices);
    let journal = run(&programs, &log, &config(SEED)).unwrap();
    let rendered: Vec<String> = channel_events(&journal)
        .iter()
        .map(|e| format!("{e:?}"))
        .collect();
    assert_eq!(
        rendered,
        vec![
            "Opened { channel: ChannelOrdinal(0), capacity: 1, receiver: TaskOrdinal(3) }",
            "Opened { channel: ChannelOrdinal(1), capacity: 2, receiver: TaskOrdinal(5) }",
            "Sent { channel: ChannelOrdinal(1), message: MessageOrdinal(0), sender: TaskOrdinal(1) }",
            "RecvBlocked { channel: ChannelOrdinal(0) }",
            "Sent { channel: ChannelOrdinal(0), message: MessageOrdinal(1), sender: TaskOrdinal(0) }",
            "Received { channel: ChannelOrdinal(0), message: MessageOrdinal(1) }",
            "Sent { channel: ChannelOrdinal(0), message: MessageOrdinal(2), sender: TaskOrdinal(2) }",
            "ReceiverGone { channel: ChannelOrdinal(0), discarded: MessageSet([MessageOrdinal(2)]) }",
            "Sent { channel: ChannelOrdinal(1), message: MessageOrdinal(3), sender: TaskOrdinal(4) }",
            "SendBlocked { channel: ChannelOrdinal(1), message: MessageOrdinal(4), sender: TaskOrdinal(6) }",
            "Received { channel: ChannelOrdinal(1), message: MessageOrdinal(0) }",
            "Sent { channel: ChannelOrdinal(1), message: MessageOrdinal(4), sender: TaskOrdinal(6) }",
        ],
        "{}",
        journal.render()
    );
    assert!(matches!(lift(&journal), LiftVerdict::Conforms(_)));
}

// --- anti-vacuity: the lift rejects wrong channel histories ---------------------------

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

fn position(bodies: &[EventBody], wanted: impl Fn(&ChannelEvent) -> bool) -> Option<usize> {
    bodies
        .iter()
        .position(|body| matches!(body, EventBody::Channel(event) if wanted(event)))
}

fn channel_fault(journal: &Journal) -> ChannelFault {
    match lift(journal) {
        LiftVerdict::Violates {
            reason: Nonconformance::Channel(fault),
            ..
        } => fault,
        other => panic!(
            "expected a channel fault, got {other:?}\n{}",
            journal.render()
        ),
    }
}

/// Seven mutation operators over real substrate journals, each rejected with its own
/// fault. The unmutated journal conforms, so each rejection is the mutant's doing.
#[test]
fn mutated_channel_journals_are_rejected() {
    let mut counts = [0_usize; 7];
    for (_, programs, logs) in corpus(NAMES) {
        for log in logs.iter().step_by(5) {
            let journal = run(&programs, log, &config(SEED)).unwrap();
            assert!(matches!(lift(&journal), LiftVerdict::Conforms(_)));
            let original = bodies(&journal);
            let fault_of = |events: Vec<EventBody>| channel_fault(&rebuilt(events));

            // 1. A message delivered twice.
            if let Some(at) = position(&original, |e| matches!(e, ChannelEvent::Received { .. })) {
                let mut events = original.clone();
                events.insert(at + 1, events[at].clone());
                assert!(
                    matches!(fault_of(events), ChannelFault::NotFifo { .. }),
                    "log {log}"
                );
                counts[0] += 1;
            }

            // 2. A message received that was never sent.
            if let Some(at) = position(&original, |e| matches!(e, ChannelEvent::Received { .. })) {
                let mut events = original.clone();
                if let EventBody::Channel(ChannelEvent::Received { message, .. }) = &mut events[at]
                {
                    message.0 += 100;
                }
                assert!(
                    matches!(fault_of(events), ChannelFault::NotFifo { .. }),
                    "log {log}"
                );
                counts[1] += 1;
            }

            // 3. A send into a full channel: a blocked send delivered at once.
            if let Some(blocked) =
                position(&original, |e| matches!(e, ChannelEvent::SendBlocked { .. }))
            {
                let mut events = original.clone();
                if let EventBody::Channel(ChannelEvent::SendBlocked {
                    channel,
                    message,
                    sender,
                }) = events[blocked].clone()
                {
                    events[blocked] = EventBody::Channel(ChannelEvent::Sent {
                        channel,
                        message,
                        sender,
                    });
                }
                assert!(
                    matches!(fault_of(events), ChannelFault::Overflow { .. }),
                    "log {log}"
                );
                counts[2] += 1;
            }

            // 4. A receiver that goes with a different queue than the model's.
            if let Some(gone) = position(&original, |e| {
                matches!(e, ChannelEvent::ReceiverGone { .. })
            }) {
                let mut events = original.clone();
                if let EventBody::Channel(ChannelEvent::ReceiverGone { discarded, .. }) =
                    &mut events[gone]
                {
                    let mut members = discarded.as_slice().to_vec();
                    members.push(MessageOrdinal(999));
                    *discarded = MessageSet::new(members);
                }
                assert!(
                    matches!(fault_of(events), ChannelFault::DropMismatch { .. }),
                    "log {log}"
                );
                counts[3] += 1;

                // 5. The receiver never goes: it outlives its task.
                let mut events = original.clone();
                events.remove(gone);
                let fault = fault_of(events);
                assert!(
                    matches!(
                        fault,
                        ChannelFault::ReceiverOutlivesTask { .. }
                            | ChannelFault::StateMismatch { .. }
                    ),
                    "log {log}: {fault:?}"
                );
                counts[4] += 1;
            }

            // 6. A receive that blocks on a channel holding a message.
            if let Some(sent) = position(
                &original,
                |e| matches!(e, ChannelEvent::Sent { channel, .. } if channel.0 == 1),
            ) {
                let mut events = original.clone();
                events.insert(
                    sent + 1,
                    EventBody::Channel(ChannelEvent::RecvBlocked {
                        channel: ChannelOrdinal(1),
                    }),
                );
                assert!(
                    matches!(
                        fault_of(events),
                        ChannelFault::StateMismatch {
                            event: "recv-blocked",
                            ..
                        }
                    ),
                    "log {log}"
                );
                counts[5] += 1;
            }

            // 7. A blocked receive abandoned before any cancellation reached it.
            if let Some(abandoned) = position(&original, |e| {
                matches!(e, ChannelEvent::RecvAbandoned { .. })
            }) {
                let mut events = original.clone();
                let moved = events.remove(abandoned);
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
                    matches!(fault_of(events), ChannelFault::NotCancelling { .. }),
                    "log {log}"
                );
                counts[6] += 1;
            }
        }
    }
    for (operator, count) in counts.iter().enumerate() {
        assert!(*count > 20, "operator {} ran {count} times", operator + 1);
    }
}

// --- refusals ------------------------------------------------------------------------

fn refusal(programs: Vec<Program>) -> BindingRefusal {
    let n: usize = programs.iter().map(Vec::len).sum();
    run(&programs, &ChoiceLog::new(vec![0; n]), &config(SEED)).unwrap_err()
}

#[test]
fn refusals_are_typed() {
    let (a, b) = (TaskLabel(1), TaskLabel(2));
    let (c, d) = (ChannelLabel(1), ChannelLabel(2));
    let started = || {
        vec![
            spawn(RegionLabel::ROOT, a),
            spawn(RegionLabel::ROOT, b),
            begin(a),
            begin(b),
        ]
    };
    let with = |tail: Vec<SubstrateOp>| {
        let mut program = started();
        program.extend(tail);
        vec![program]
    };
    let opened = |capacity| SubstrateOp::OpenChannel {
        channel: c,
        capacity,
        receiver: b,
    };

    // Malformed programs: no INV-008 reading.
    let malformed = [
        (with(vec![send(a, d)]), BindingRefusal::UnboundChannel(2)),
        (
            with(vec![opened(1), opened(1)]),
            BindingRefusal::ChannelLabelRebound(1),
        ),
        (with(vec![opened(0)]), BindingRefusal::ZeroCapacity(1)),
        (
            with(vec![
                opened(1),
                SubstrateOp::CloseSenders { channel: c },
                send(a, c),
            ]),
            BindingRefusal::SendersAlreadyClosed(1),
        ),
        // A sender blocked on a full channel cannot take another command.
        (
            with(vec![opened(1), send(a, c), send(a, c), send(a, c)]),
            BindingRefusal::TaskBlocked { task: 0 },
        ),
        // A receiver blocked on an empty channel cannot either.
        (
            with(vec![opened(1), recv(c), SubstrateOp::Finish { task: b }]),
            BindingRefusal::TaskBlocked { task: 1 },
        ),
    ];
    for (programs, expected) in malformed {
        let got = refusal(programs);
        assert_eq!(got, expected);
        assert_eq!(got.inconclusive_reason(), None, "{got}");
    }

    // The INV-008 readings of the refusals no bound corpus reaches.
    assert_eq!(
        BindingRefusal::UnorderedWake { task: 0 }.inconclusive_reason(),
        Some(InconclusiveReason::Unsupported)
    );
    assert_eq!(
        BindingRefusal::SubstrateChannelDisagrees {
            detail: "x".to_owned()
        }
        .inconclusive_reason(),
        Some(InconclusiveReason::EngineError)
    );

    // Bytes: an unknown event tag and an unsorted drop set are refused, not guessed.
    let mut journal = Journal::new();
    journal
        .append(EventBody::Channel(ChannelEvent::RecvClosed {
            channel: ChannelOrdinal(0),
        }))
        .unwrap();
    let mut bad = journal.encode().unwrap();
    let tag_at = bad.len() - 5;
    bad[tag_at] = 99;
    assert!(matches!(
        Journal::decode(&bad),
        Err(DecodeError::UnknownTag {
            table: "channel event",
            tag: 99,
            ..
        })
    ));
    let mut journal = Journal::new();
    journal
        .append(EventBody::Channel(ChannelEvent::ReceiverGone {
            channel: ChannelOrdinal(0),
            discarded: MessageSet::new([MessageOrdinal(1), MessageOrdinal(2)]),
        }))
        .unwrap();
    let mut unsorted = journal.encode().unwrap();
    let len = unsorted.len();
    for i in 0..8 {
        unsorted.swap(len - 16 + i, len - 8 + i);
    }
    assert!(matches!(
        Journal::decode(&unsorted),
        Err(DecodeError::UnsortedSet { .. })
    ));
}
