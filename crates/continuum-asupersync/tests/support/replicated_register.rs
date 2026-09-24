//! The replicated register as a program on asupersync 0.5.0, run through the binding,
//! and its projection onto the operational durable register (PR-16/IMPL-03, bn-131yp).
//!
//! This file is shared by the tests that hold the program to its models. Include it
//! with `#[path = "support/replicated_register.rs"] mod register;`. It has three parts.
//!
//! # 1. The program
//!
//! A [`Plan`] names, for each replica `a b c`, the value it writes at each epoch and a
//! script of [`Act`]s. [`build`] turns a plan into binding [`Program`]s: actor 0 is
//! the setup, which runs first; actors 1..=3 are the replicas; the rest are the
//! coordinators. Each act is one call into asupersync through the binding:
//!
//! | act | substrate call ([`SubstrateOp`]) | storage phase |
//! |---|---|---|
//! | [`Act::Reserve`] | `Reserve`: a `Transaction` obligation through the writer's `Cx` | the write permit for the slot |
//! | [`Act::Submit`] | `Acquire` of an `IoOp` obligation | the bytes enter the volatile log |
//! | [`Act::Sync`] | `Commit` of that `IoOp` | `sync` completes: the bytes are durable |
//! | [`Act::Release`] | `Commit` of the permit | the permit is released after the durable write |
//! | [`Act::Abort`] | `Abort` of the permit | a cancelled writer releases its permit |
//! | [`Act::Confirm`] | `Send` on the coordinator's channel | a stable confirmation |
//! | [`Act::Crash`] | `Cancel` of the replica's region | a crash: in-flight obligations abort, the incarnation ends |
//!
//! Each replica incarnation is a region under the root, with one writer task per
//! epoch. A crash cancels the region; the next incarnation is a region and writers
//! that the setup already opened, spawned, and began. So every task and region ordinal
//! is fixed by the setup, and the journal names every task's role ([`Roles`]).
//!
//! For each epoch and each value some replica writes, a coordinator task owns a
//! bounded channel. It receives confirmations and, after a majority of them (two of
//! three), publishes the acknowledgement as a two-phase effect: `Reserve`, then
//! `Commit` of a `Transaction`. A value that fewer than two replicas write gets a
//! coordinator that receives what it can and never publishes. A coordinator that
//! waits on an empty channel is parked; the binding refuses a command to a parked task
//! (`TaskBlocked`), so a log that commands it is not a run of this program. The log
//! generators ([`admissible_logs`], [`sample_logs`]) produce only admissible logs, and
//! the tests show a log that is not admissible is that typed refusal.
//!
//! Virtual time is not used: no step of the durable register waits for a timer, and a
//! sleeping writer would only add a refusal (`TaskAsleep`) to the admissibility rule.
//!
//! What the choice log controls: the interleaving of the replicas and coordinators,
//! and so every order of reserve, submit, sync, crash, confirmation and ack across
//! replicas. What the plan fixes: which value each replica writes, and where in its
//! own script each crash falls. The binding has no data-dependent control flow, so the
//! program's decisions (write once per slot, retry after a crash, confirm only after
//! sync, ack only after a majority of confirmations) are the scripts' structure.
//!
//! # 2. The projection
//!
//! [`observe`] reads a journal, and only the journal, with the [`Roles`] of its plan,
//! which it first checks against the journal's own spawn events. It keeps one attempt
//! per writer reservation and maps each event to the durable register's slot states:
//!
//! - an attempt whose `IoOp` committed is `Durable(v)`; with the `IoOp` open it is
//!   `Volatile(v)`; with the `IoOp` aborted it is gone; with no `IoOp` and the permit
//!   open it is `Reserved`; otherwise it is gone;
//! - a slot is the one live attempt on it, or `Free`. Two live attempts on one slot
//!   have no durable-register counterpart: the state is unprojectable, a failure;
//! - `acks` holds `(e, v)` once coordinator `(e, v)` commits its acknowledgement.
//!
//! Each event also names the durable-register step it must be: [`Expect`]. A writer's
//! permit reserve is `Reserve`, its `IoOp` open is `Submit`, the `IoOp` commit is
//! `Sync`, the `IoOp` abort is `Lose`; a permit abort before any bytes is `Abort`
//! (explicit) or `Lose` (cancel), and a permit commit before any bytes is `Abort` (a
//! release with no write); a coordinator's commit is `Ack`. Every other event must
//! stutter.
//!
//! The program carries no data. A writer's value is a label of the role table. The
//! projection corroborates it only for writers that confirm: each confirmation must
//! go to the coordinator of the sender's epoch and value. A writer that crashes before
//! it confirms has an uncorroborated value, which the step check covers only in the
//! aggregate: a wrong label there is refuted where an ack quorum depends on it.
//!
//! # 3. The oracle
//!
//! [`check`] holds every observed step to the durable register: a stutter must leave
//! the projected state unchanged, and a named step must be a step of the ported
//! slot protocol [`Spec`] from the projected pre-state, with that label, to the
//! projected post-state. It then holds each step to the abstract register as
//! `pr16_impl02_durable_register.rs::correspondence` does: `acks` read as a map is
//! `chosen`, an `Ack(e, v)` must be an enabled `Choose(e, v)`, and every other step
//! must leave `chosen` unchanged. It also applies that file's `durability_violations`
//! predicate to every step, and requires every projected state to be a reachable state
//! of [`Spec`].
//!
//! Only the step check can fail on a journal of this program. The other layers follow
//! from it: a step of [`Spec`] never loses a durable record, withdraws an ack or
//! promotes bytes without `Sync`; two acks of one epoch need two disjoint majorities of
//! three, so a matched `Ack` is an enabled `Choose`; the post-state of a matched step
//! from a reachable state is reachable; and every event classed as a stutter leaves the
//! projection unchanged by construction. They stay as independent checks of that
//! reasoning, and the tests fire each of them on crafted steps.
//!
//! The section marked "verbatim port" below is copied from
//! `crates/continuum-cml-elab/tests/pr16_impl02_durable_register.rs`, where the
//! differential against the lowered model shows [`Spec`] has exactly the reference
//! engine's reachable states and labelled transitions. The drift test in
//! `pr16_impl03_replicated_register.rs` compares the two texts item by item, and
//! recomputes that file's golden step counts with this port.
//!
//! # Hooks
//!
//! - bn-28oa (IMPL-05 mutants): a mutant is a [`Plan`] whose scripts reorder or drop
//!   acts, or a new [`Act`]. For example, ack-before-sync (M01) is a replica script
//!   whose `Confirm` precedes its `Sync`. [`check`] reports where the first failing
//!   step is, as a typed [`Mismatch`]. The mutants that need process epochs, timers or
//!   checksums need new acts, and the projection rules above say which step each new
//!   event must be.
//! - bn-5fpl (the correct-version exit): [`build`], [`admissible_logs`],
//!   [`sample_logs`], [`observe`] and [`check`] are the whole pipeline; the correct
//!   plans are in `pr16_impl03_replicated_register.rs`.

use std::collections::{BTreeMap, BTreeSet};

use continuum_asupersync::binding::{Program, SubstrateOp};
use continuum_asupersync::choice::ChoiceLog;
use continuum_asupersync::family::EventBody;
use continuum_asupersync::family::channel::{ChannelEvent, ChannelLabel};
use continuum_asupersync::family::effect::{AbortCause, EffectEvent, ReservationLabel};
use continuum_asupersync::family::lifecycle::{LifecycleEvent, RegionLabel, TaskLabel};
use continuum_asupersync::family::obligation::{Discharge, ObligationEvent, ObligationKind};
use continuum_asupersync::journal::Journal;
use continuum_task::region::worker::Resumability;

/// The values, by index, as the durable register's run configuration names them.
pub const VALUES: [&str; 2] = ["v0", "v1"];

// ---------------------------------------------------------------------------
// 1. the program
// ---------------------------------------------------------------------------

/// One step of a replica's script. Epochs are indices `0..plan.epochs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    /// The epoch's writer takes the write permit.
    Reserve(u8),
    /// The writer submits its bytes to the volatile log.
    Submit(u8),
    /// The submitted bytes become durable.
    Sync(u8),
    /// The writer releases its permit.
    Release(u8),
    /// The writer releases its permit without writing.
    Abort(u8),
    /// The writer confirms the durable value to the coordinator of its value.
    Confirm(u8),
    /// The replica crashes: its incarnation's region is cancelled.
    Crash,
}

/// The correct write of one slot: reserve, submit, sync, release, confirm.
pub fn write(epoch: u8) -> Vec<Act> {
    vec![
        Act::Reserve(epoch),
        Act::Submit(epoch),
        Act::Sync(epoch),
        Act::Release(epoch),
        Act::Confirm(epoch),
    ]
}

/// A program: which value each replica writes, and each replica's script.
#[derive(Debug, Clone)]
pub struct Plan {
    /// A stable name.
    pub name: &'static str,
    /// Epochs `0..epochs`, one or two.
    pub epochs: u8,
    /// `values[n][e]`: the index of the value replica `n` writes at epoch `e`.
    pub values: [[u8; 2]; 3],
    /// Each replica's script, `a b c`.
    pub replicas: [Vec<Act>; 3],
}

/// What one task is, by its journal ordinal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    /// The writer of replica `node` for `epoch`, in one incarnation, writing `value`.
    Writer {
        /// Replica index: `a b c`.
        node: u8,
        /// The epoch.
        epoch: u8,
        /// The value index.
        value: u8,
    },
    /// The coordinator of `(epoch, value)`.
    Coordinator {
        /// The epoch.
        epoch: u8,
        /// The value index.
        value: u8,
    },
}

/// Every task's role and owning region ordinal, indexed by task ordinal, as the
/// setup allocates them, and how many regions the setup opens.
///
/// The projection holds the table to the journal: each spawn's region, the region
/// count, each channel's receiver (a coordinator), and each confirmation's sender (a
/// writer of the coordinator's epoch and value). So the value of a writer that
/// confirms is corroborated by the coordinator it confirms to; the value of a writer
/// that never confirms is not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Roles {
    /// `(role, region ordinal)` by task ordinal.
    pub tasks: Vec<(Role, u32)>,
    /// Regions the setup opens under the root.
    pub regions: u32,
}

/// Static facts about one operation, for the admissibility rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Meta {
    /// The coordinator this operation commands, by coordinator index.
    commands: Option<usize>,
    /// The coordinator whose channel this operation sends to.
    feeds: Option<usize>,
    /// Whether it is a receive.
    recv: bool,
}

/// A built plan: the binding programs, the roles, and what the log generators need.
#[derive(Debug, Clone)]
pub struct Built {
    /// Actor 0 is the setup; then replicas `a b c`; then the coordinators.
    pub programs: Vec<Program>,
    /// Task roles by ordinal.
    pub roles: Roles,
    meta: Vec<Vec<Meta>>,
    coordinators: usize,
}

struct Labels {
    next: u32,
}

impl Labels {
    fn take(&mut self) -> u32 {
        self.next += 1;
        self.next
    }
}

/// The coordinators of a plan: every `(epoch, value)` some replica confirms, with how
/// many replicas confirm it.
fn coordinators(plan: &Plan) -> Vec<(u8, u8, usize)> {
    let mut out = Vec::new();
    for e in 0..plan.epochs {
        for v in 0..u8::try_from(VALUES.len()).expect("two values") {
            let writers = (0..3)
                .filter(|n| {
                    plan.values[*n][usize::from(e)] == v
                        && plan.replicas[*n].contains(&Act::Confirm(e))
                })
                .count();
            if writers > 0 {
                out.push((e, v, writers));
            }
        }
    }
    out
}

/// The binding programs of `plan`.
///
/// # Panics
///
/// On a plan that is not well formed: an epoch out of range, or an act on a permit or
/// bytes that the same incarnation has not taken.
pub fn build(plan: &Plan) -> Built {
    assert!((1..=2).contains(&plan.epochs), "one or two epochs");
    assert!(
        plan.values
            .iter()
            .flatten()
            .all(|v| usize::from(*v) < VALUES.len()),
        "every value names one of VALUES"
    );
    let mut labels = Labels { next: 0 };
    let mut setup: Program = Vec::new();
    let mut tasks: Vec<(Role, u32)> = Vec::new();
    let mut region_ordinal = 0_u32;

    // Regions and writers: every incarnation of every replica.
    let incarnations: Vec<usize> = plan
        .replicas
        .iter()
        .map(|s| 1 + s.iter().filter(|a| **a == Act::Crash).count())
        .collect();
    // writer[(n, inc, e)] = task label; region[(n, inc)] = region label
    let mut writer: BTreeMap<(u8, usize, u8), TaskLabel> = BTreeMap::new();
    let mut region: BTreeMap<(u8, usize), RegionLabel> = BTreeMap::new();
    let mut spawns = Vec::new();
    for n in 0..3_u8 {
        for inc in 0..incarnations[usize::from(n)] {
            let r = RegionLabel(labels.take());
            setup.push(SubstrateOp::OpenRegion {
                parent: RegionLabel::ROOT,
                child: r,
            });
            region_ordinal += 1;
            region.insert((n, inc), r);
            for e in 0..plan.epochs {
                let t = TaskLabel(labels.take());
                writer.insert((n, inc, e), t);
                let value = plan.values[usize::from(n)][usize::from(e)];
                tasks.push((
                    Role::Writer {
                        node: n,
                        epoch: e,
                        value,
                    },
                    region_ordinal,
                ));
                spawns.push((r, t));
            }
        }
    }
    let coords = coordinators(plan);
    let coord_region = RegionLabel(labels.take());
    setup.push(SubstrateOp::OpenRegion {
        parent: RegionLabel::ROOT,
        child: coord_region,
    });
    region_ordinal += 1;
    let mut coord_task = Vec::new();
    let mut channel = BTreeMap::new();
    for (i, &(e, v, _)) in coords.iter().enumerate() {
        let t = TaskLabel(labels.take());
        tasks.push((Role::Coordinator { epoch: e, value: v }, region_ordinal));
        spawns.push((coord_region, t));
        coord_task.push(t);
        channel.insert((e, v), (i, ChannelLabel(labels.take())));
    }
    for &(r, t) in &spawns {
        setup.push(SubstrateOp::Spawn {
            region: r,
            task: t,
            resumability: Resumability::Resumable,
        });
    }
    for &(_, t) in &spawns {
        setup.push(SubstrateOp::Begin { task: t });
    }
    for (i, &(e, v, _)) in coords.iter().enumerate() {
        setup.push(SubstrateOp::OpenChannel {
            channel: channel[&(e, v)].1,
            capacity: 4,
            receiver: coord_task[i],
        });
    }

    let none = Meta {
        commands: None,
        feeds: None,
        recv: false,
    };
    let mut programs = vec![setup.clone()];
    let mut meta = vec![vec![none; setup.len()]];

    // Replicas.
    for n in 0..3_u8 {
        let mut inc = 0;
        let mut permit: BTreeMap<u8, ReservationLabel> = BTreeMap::new();
        let mut bytes: BTreeMap<u8, ReservationLabel> = BTreeMap::new();
        let mut ops = Vec::new();
        let mut m = Vec::new();
        for act in &plan.replicas[usize::from(n)] {
            let w = |e: u8| {
                assert!(e < plan.epochs, "epoch in range");
                writer[&(n, inc, e)]
            };
            let (op, feeds) = match *act {
                Act::Reserve(e) => {
                    let p = ReservationLabel(labels.take());
                    permit.insert(e, p);
                    (
                        SubstrateOp::Reserve {
                            task: w(e),
                            reservation: p,
                        },
                        None,
                    )
                }
                Act::Submit(e) => {
                    let b = ReservationLabel(labels.take());
                    bytes.insert(e, b);
                    (
                        SubstrateOp::Acquire {
                            task: w(e),
                            reservation: b,
                            kind: ObligationKind::IoOp,
                        },
                        None,
                    )
                }
                Act::Sync(e) => (
                    SubstrateOp::Commit {
                        reservation: bytes.remove(&e).expect("submitted bytes"),
                    },
                    None,
                ),
                Act::Release(e) => (
                    SubstrateOp::Commit {
                        reservation: permit.remove(&e).expect("a permit"),
                    },
                    None,
                ),
                Act::Abort(e) => (
                    SubstrateOp::Abort {
                        reservation: permit.remove(&e).expect("a permit"),
                    },
                    None,
                ),
                Act::Confirm(e) => {
                    let value = plan.values[usize::from(n)][usize::from(e)];
                    let (i, c) = channel[&(e, value)];
                    (
                        SubstrateOp::Send {
                            task: w(e),
                            channel: c,
                        },
                        Some(i),
                    )
                }
                Act::Crash => {
                    let r = region[&(n, inc)];
                    inc += 1;
                    permit.clear();
                    bytes.clear();
                    (SubstrateOp::Cancel { region: r }, None)
                }
            };
            ops.push(op);
            m.push(Meta {
                commands: None,
                feeds,
                recv: false,
            });
        }
        programs.push(ops);
        meta.push(m);
    }

    // Coordinators: a majority of confirmations, then the acknowledgement.
    for (i, &(e, v, writers)) in coords.iter().enumerate() {
        let (_, c) = channel[&(e, v)];
        let mut ops = Vec::new();
        let mut m = Vec::new();
        let at = Meta {
            commands: Some(i),
            feeds: None,
            recv: false,
        };
        for _ in 0..writers.min(2) {
            ops.push(SubstrateOp::Recv { channel: c });
            m.push(Meta { recv: true, ..at });
        }
        if writers >= 2 {
            let a = ReservationLabel(labels.take());
            ops.push(SubstrateOp::Reserve {
                task: coord_task[i],
                reservation: a,
            });
            ops.push(SubstrateOp::Commit { reservation: a });
            m.push(at);
            m.push(at);
        }
        programs.push(ops);
        meta.push(m);
    }
    Built {
        programs,
        roles: Roles {
            tasks,
            regions: region_ordinal,
        },
        meta,
        coordinators: coords.len(),
    }
}

// ---------------------------------------------------------------------------
// admissible choice logs
// ---------------------------------------------------------------------------

/// The admissibility state: per coordinator, confirmations sent and receives issued.
#[derive(Clone)]
struct Sched {
    cursors: Vec<usize>,
    sent: Vec<usize>,
    received: Vec<usize>,
}

impl Sched {
    fn new(built: &Built) -> Self {
        Self {
            cursors: vec![0; built.programs.len()],
            sent: vec![0; built.coordinators],
            received: vec![0; built.coordinators],
        }
    }

    /// Actors with operations left, in ascending order: what a choice indexes.
    fn enabled(&self, built: &Built) -> Vec<usize> {
        (0..built.programs.len())
            .filter(|a| self.cursors[*a] < built.programs[*a].len())
            .collect()
    }

    /// Whether `actor`'s next operation commands a task that is not parked.
    fn admissible(&self, built: &Built, actor: usize) -> bool {
        let m = built.meta[actor][self.cursors[actor]];
        m.commands.is_none_or(|k| self.received[k] <= self.sent[k])
    }

    fn take(&mut self, built: &Built, actor: usize) {
        let m = built.meta[actor][self.cursors[actor]];
        if let Some(k) = m.feeds {
            self.sent[k] += 1;
        }
        if m.recv {
            let k = m.commands.expect("a receive commands its coordinator");
            self.received[k] += 1;
        }
        self.cursors[actor] += 1;
    }
}

fn setup_prefix(built: &Built) -> (Sched, Vec<u32>) {
    let mut sched = Sched::new(built);
    for _ in 0..built.programs[0].len() {
        sched.take(built, 0);
    }
    (sched, vec![0; built.programs[0].len()])
}

/// Every admissible choice log of `built`, setup first, in lexicographic order, or
/// `None` when there are more than `cap`.
pub fn admissible_logs(built: &Built, cap: usize) -> Option<Vec<ChoiceLog>> {
    fn go(
        built: &Built,
        sched: &Sched,
        prefix: &mut Vec<u32>,
        out: &mut Vec<ChoiceLog>,
        cap: usize,
    ) -> bool {
        let enabled = sched.enabled(built);
        if enabled.is_empty() {
            out.push(ChoiceLog::new(prefix.iter().copied()));
            return out.len() <= cap;
        }
        for (index, &actor) in enabled.iter().enumerate() {
            if !sched.admissible(built, actor) {
                continue;
            }
            let mut next = sched.clone();
            next.take(built, actor);
            prefix.push(u32::try_from(index).expect("few actors"));
            let ok = go(built, &next, prefix, out, cap);
            prefix.pop();
            if !ok {
                return false;
            }
        }
        true
    }
    let (sched, mut prefix) = setup_prefix(built);
    let mut out = Vec::new();
    go(built, &sched, &mut prefix, &mut out, cap).then_some(out)
}

/// SplitMix64: the explicit, seeded source of the sampled schedules (INV-005: the
/// seed is a parameter, recorded with the evidence).
struct SplitMix(u64);

impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(n).expect("small")).expect("small")
    }
}

/// `count` distinct admissible choice logs of `built`, drawn by a uniform random walk
/// over the admissible actors from `seed`, in the order drawn.
///
/// # Panics
///
/// When `count` distinct logs are not found within `64 * count` draws.
pub fn sample_logs(built: &Built, count: usize, seed: u64) -> Vec<ChoiceLog> {
    let mut rng = SplitMix(seed);
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    let mut draws = 0;
    while out.len() < count {
        draws += 1;
        assert!(draws <= 64 * count, "too few distinct logs");
        let (mut sched, mut prefix) = setup_prefix(built);
        loop {
            let enabled = sched.enabled(built);
            if enabled.is_empty() {
                break;
            }
            let options: Vec<usize> = (0..enabled.len())
                .filter(|i| sched.admissible(built, enabled[*i]))
                .collect();
            assert!(
                !options.is_empty(),
                "every actor with operations left is parked: the plan deadlocks"
            );
            let index = options[rng.below(options.len())];
            sched.take(built, enabled[index]);
            prefix.push(u32::try_from(index).expect("few actors"));
        }
        let log = ChoiceLog::new(prefix);
        if seen.insert(log.clone()) {
            out.push(log);
        }
    }
    out
}

/// An inadmissible log of `built`: a greedy admissible prefix that prefers the
/// highest-index actor, so a coordinator parks early, then a command to the parked
/// coordinator, completed in actor order. `None` when the walk meets no parked
/// coordinator.
pub fn an_inadmissible_log(built: &Built) -> Option<ChoiceLog> {
    let (mut sched, mut prefix) = setup_prefix(built);
    loop {
        let enabled = sched.enabled(built);
        if enabled.is_empty() {
            return None;
        }
        if let Some(index) = (0..enabled.len()).find(|i| !sched.admissible(built, enabled[*i])) {
            // Command the parked coordinator now, then finish in actor order.
            let mut rest = sched.clone();
            rest.take(built, enabled[index]);
            prefix.push(u32::try_from(index).expect("few actors"));
            while !rest.enabled(built).is_empty() {
                let first = rest.enabled(built)[0];
                rest.take(built, first);
                prefix.push(0);
            }
            return Some(ChoiceLog::new(prefix));
        }
        let index = (0..enabled.len())
            .find(|i| sched.admissible(built, enabled[*i]))
            .expect("an admissible actor");
        // Prefer a coordinator step, so that one parks before its confirmation.
        let index = (0..enabled.len())
            .rev()
            .find(|i| sched.admissible(built, enabled[*i]))
            .unwrap_or(index);
        sched.take(built, enabled[index]);
        prefix.push(u32::try_from(index).expect("few actors"));
    }
}

// ---------------------------------------------------------------------------
// 2. the projection
// ---------------------------------------------------------------------------

/// The phase of one obligation, as the journal reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Open,
    Committed,
    Aborted,
}

/// One writer attempt at a slot: a permit, and bytes once submitted.
#[derive(Debug, Clone)]
struct Attempt {
    node: u8,
    epoch: u8,
    value: u8,
    permit: Option<Phase>,
    bytes: Option<Phase>,
}

/// What an attempt contributes to its slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Live {
    Reserved,
    Volatile(u8),
    Durable(u8),
}

impl Attempt {
    fn live(&self) -> Option<Live> {
        match (self.bytes, self.permit) {
            (Some(Phase::Committed), _) => Some(Live::Durable(self.value)),
            (Some(Phase::Open), _) => Some(Live::Volatile(self.value)),
            (Some(Phase::Aborted), _) => None,
            (None, Some(Phase::Open)) => Some(Live::Reserved),
            (None, _) => None,
        }
    }
}

/// The durable-register step one event must be.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Expect {
    /// The projected state must not change.
    Stutter,
    /// A step with exactly this label.
    Step(String),
    /// A step whose label starts with this: `Lose` of a reserved slot is enabled under
    /// every value label.
    StepPrefix(String),
}

/// Why a journal could not be projected at all (fail closed: no steps).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectionRefusal {
    /// A task spawn disagrees with the plan's roles, or names a task it has none for.
    RolesDisagree {
        /// The event's sequence number.
        seq: u64,
    },
    /// An event names a task, reservation or obligation the journal never introduced.
    UnknownEntity {
        /// The event's sequence number.
        seq: u64,
    },
    /// The journal spawned fewer tasks, or opened another number of regions, than the
    /// plan has.
    RolesUnspawned,
}

/// One observed event as a step: what it must be, and the projected states around it.
/// `None` is an unprojectable state.
#[derive(Debug, Clone)]
pub struct Observed {
    /// The event's sequence number.
    pub seq: u64,
    /// The event, rendered.
    pub event: String,
    /// What the event must be.
    pub expect: Expect,
    /// The projected state before it.
    pub pre: Option<Raw>,
    /// The projected state after it.
    pub post: Option<Raw>,
}

#[derive(Default)]
struct View {
    attempts: Vec<Attempt>,
    /// Effect reservation ordinal → attempt index, or the ack it stages.
    permits: BTreeMap<u32, Result<usize, (u8, u8)>>,
    /// Obligation ordinal of an `IoOp` → attempt index.
    bytes: BTreeMap<u32, usize>,
    /// Writer task ordinal → its latest attempt.
    latest: BTreeMap<u32, usize>,
    acks: BTreeSet<(u8, u8)>,
}

impl View {
    fn raw(&self) -> Option<Raw> {
        let mut slots: BTreeMap<(u8, u8), Live> = BTreeMap::new();
        for a in &self.attempts {
            if let Some(live) = a.live()
                && slots.insert((a.node, a.epoch), live).is_some()
            {
                return None;
            }
        }
        let mut raw = Raw {
            log: BTreeSet::new(),
            pending: BTreeSet::new(),
            acks: self.acks.clone(),
        };
        for (&(n, e), live) in &slots {
            match *live {
                Live::Reserved => {
                    raw.pending.insert((n, e));
                }
                Live::Volatile(v) => {
                    raw.pending.insert((n, e));
                    raw.log.insert((n, e, v));
                }
                Live::Durable(v) => {
                    raw.log.insert((n, e, v));
                }
            }
        }
        Some(raw)
    }
}

fn at(node: u8, epoch: u8) -> String {
    format!("n={},epoch={epoch}", NODES[usize::from(node)])
}

/// The journal as durable-register steps.
///
/// # Errors
///
/// A [`ProjectionRefusal`] when the journal's spawns disagree with `roles` or an event
/// names an entity the journal never introduced.
pub fn observe(roles: &Roles, journal: &Journal) -> Result<Vec<Observed>, ProjectionRefusal> {
    let mut view = View::default();
    let mut out = Vec::new();
    let mut spawned = 0_usize;
    let mut regions = 0_u32;
    // Channel ordinal → the `(epoch, value)` of the coordinator that receives on it.
    let mut channels: BTreeMap<u32, (u8, u8)> = BTreeMap::new();
    let role_of = |task: u32| roles.tasks.get(task as usize).map(|r| r.0);
    for event in journal.events() {
        let seq = event.seq();
        let pre = view.raw();
        let unknown = ProjectionRefusal::UnknownEntity { seq };
        let expect = match event.body() {
            EventBody::Lifecycle(LifecycleEvent::TaskSpawned { task, region, .. }) => {
                let want = roles
                    .tasks
                    .get(usize::try_from(task.0).unwrap_or(usize::MAX));
                if task.0 as usize != spawned || want.map(|w| w.1) != Some(region.0) {
                    return Err(ProjectionRefusal::RolesDisagree { seq });
                }
                spawned += 1;
                Expect::Stutter
            }
            EventBody::Lifecycle(LifecycleEvent::RegionOpened { .. }) => {
                regions += 1;
                Expect::Stutter
            }
            EventBody::Channel(ChannelEvent::Opened {
                channel, receiver, ..
            }) => {
                let Some(Role::Coordinator { epoch, value }) = role_of(receiver.0) else {
                    return Err(ProjectionRefusal::RolesDisagree { seq });
                };
                channels.insert(channel.0, (epoch, value));
                Expect::Stutter
            }
            EventBody::Channel(ChannelEvent::Sent {
                channel, sender, ..
            }) => {
                let to = channels.get(&channel.0).ok_or_else(|| unknown.clone())?;
                match role_of(sender.0) {
                    Some(Role::Writer { epoch, value, .. }) if (epoch, value) == *to => {}
                    _ => return Err(ProjectionRefusal::RolesDisagree { seq }),
                }
                Expect::Stutter
            }
            EventBody::Effect(EffectEvent::Reserved { reservation, task }) => {
                let (role, _) = *roles
                    .tasks
                    .get(task.0 as usize)
                    .ok_or_else(|| unknown.clone())?;
                match role {
                    Role::Writer { node, epoch, value } => {
                        view.attempts.push(Attempt {
                            node,
                            epoch,
                            value,
                            permit: Some(Phase::Open),
                            bytes: None,
                        });
                        let i = view.attempts.len() - 1;
                        view.permits.insert(reservation.0, Ok(i));
                        view.latest.insert(task.0, i);
                        Expect::Step(format!("Reserve({})", at(node, epoch)))
                    }
                    Role::Coordinator { epoch, value } => {
                        view.permits.insert(reservation.0, Err((epoch, value)));
                        Expect::Stutter
                    }
                }
            }
            EventBody::Effect(EffectEvent::Committed { reservation }) => {
                match *view.permits.get(&reservation.0).ok_or(unknown)? {
                    Ok(i) => {
                        let a = &mut view.attempts[i];
                        a.permit = Some(Phase::Committed);
                        if a.bytes.is_none() {
                            // A permit released with no bytes: the release step.
                            Expect::Step(format!("Abort({})", at(a.node, a.epoch)))
                        } else {
                            Expect::Stutter
                        }
                    }
                    Err((epoch, value)) => {
                        view.acks.insert((epoch, value));
                        Expect::Step(format!(
                            "Ack(epoch={epoch},value={})",
                            VALUES[usize::from(value)]
                        ))
                    }
                }
            }
            EventBody::Effect(EffectEvent::Aborted { reservation, cause }) => {
                match *view.permits.get(&reservation.0).ok_or(unknown)? {
                    Ok(i) => {
                        let a = &mut view.attempts[i];
                        a.permit = Some(Phase::Aborted);
                        match (a.bytes, cause) {
                            (Some(_), _) => Expect::Stutter,
                            (None, AbortCause::Explicit) => {
                                Expect::Step(format!("Abort({})", at(a.node, a.epoch)))
                            }
                            (None, AbortCause::Cancel) => {
                                Expect::StepPrefix(format!("Lose({},", at(a.node, a.epoch)))
                            }
                        }
                    }
                    Err(_) => Expect::Stutter,
                }
            }
            EventBody::Obligation(ObligationEvent::Opened {
                obligation,
                kind: ObligationKind::IoOp,
                holder,
                ..
            }) => {
                let (role, _) = *roles
                    .tasks
                    .get(holder.0 as usize)
                    .ok_or_else(|| unknown.clone())?;
                let Role::Writer { node, epoch, value } = role else {
                    return Err(ProjectionRefusal::RolesDisagree { seq });
                };
                // The bytes of the writer's latest attempt, or of a new attempt with no
                // permit when that attempt already has bytes or there is none.
                let i = match view.latest.get(&holder.0) {
                    Some(&i) if view.attempts[i].bytes.is_none() => i,
                    _ => {
                        view.attempts.push(Attempt {
                            node,
                            epoch,
                            value,
                            permit: None,
                            bytes: None,
                        });
                        view.attempts.len() - 1
                    }
                };
                view.attempts[i].bytes = Some(Phase::Open);
                view.latest.insert(holder.0, i);
                view.bytes.insert(obligation.0, i);
                Expect::Step(format!(
                    "Submit({},value={})",
                    at(node, epoch),
                    VALUES[usize::from(value)]
                ))
            }
            EventBody::Obligation(ObligationEvent::Discharged { obligation, how }) => {
                if let Some(&i) = view.bytes.get(&obligation.0) {
                    let a = &mut view.attempts[i];
                    if *how == Discharge::Committed {
                        a.bytes = Some(Phase::Committed);
                        Expect::Step(format!("Sync({})", at(a.node, a.epoch)))
                    } else {
                        a.bytes = Some(Phase::Aborted);
                        Expect::Step(format!(
                            "Lose({},value={})",
                            at(a.node, a.epoch),
                            VALUES[usize::from(a.value)]
                        ))
                    }
                } else {
                    Expect::Stutter
                }
            }
            _ => Expect::Stutter,
        };
        out.push(Observed {
            seq,
            event: event.render(),
            expect,
            pre,
            post: view.raw(),
        });
    }
    if spawned != roles.tasks.len() || regions != roles.regions {
        return Err(ProjectionRefusal::RolesUnspawned);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// 3. the oracle
// ---------------------------------------------------------------------------

/// Why one observed step does not correspond.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Mismatch {
    /// The projected state before or after the step has no durable-register
    /// counterpart: two live attempts on one slot.
    Unprojectable,
    /// A projected state the durable register does not reach.
    Unreachable,
    /// An event that must stutter changes the projected state.
    NotAStutter,
    /// A named step that is no enabled durable-register step with that label from the
    /// projected pre-state to the projected post-state.
    NotAnEnabledStep,
    /// A durable-register `Ack` that is no enabled abstract `Choose`, or another step
    /// that changes the abstract `chosen`.
    NotAnEnabledChoose,
    /// A step that loses a durable record, withdraws an acknowledgement, or promotes
    /// bytes to durable without `Sync` (IMPL-02's `durability_violations`).
    Durability,
}

/// The result of holding one journal's steps to the durable and abstract registers.
#[derive(Debug, Default, Clone)]
pub struct Correspondence {
    /// Observed steps.
    pub steps: usize,
    /// Steps that leave the projected durable state unchanged.
    pub stutters: usize,
    /// Durable-register steps taken, by label.
    pub durable: BTreeMap<String, usize>,
    /// Durable-register steps taken, by action, with `Lose` split by what it loses:
    /// `Lose/reserved` (a permit) and `Lose/volatile` (submitted bytes).
    pub kinds: BTreeMap<String, usize>,
    /// Abstract `Choose` steps that are real moves, by label.
    pub chooses: BTreeMap<String, usize>,
    /// The projected durable states visited.
    pub states: BTreeSet<Raw>,
    /// Failures: kind, sequence number, event.
    pub failures: Vec<(Mismatch, u64, String)>,
}

impl Correspondence {
    /// Fold another journal's result into this one.
    pub fn absorb(&mut self, other: Self) {
        self.steps += other.steps;
        self.stutters += other.stutters;
        for (k, v) in other.durable {
            *self.durable.entry(k).or_default() += v;
        }
        for (k, v) in other.kinds {
            *self.kinds.entry(k).or_default() += v;
        }
        for (k, v) in other.chooses {
            *self.chooses.entry(k).or_default() += v;
        }
        self.states.extend(other.states);
        self.failures.extend(other.failures);
    }
}

/// `acks` read as a map: the abstract `chosen`, or `None` when an epoch has two values.
fn chosen(raw: &Raw) -> Option<BTreeMap<u8, u8>> {
    let mut out = BTreeMap::new();
    for &(e, v) in &raw.acks {
        if out.insert(e, v).is_some() {
            return None;
        }
    }
    Some(out)
}

/// A projected state as the slot protocol's state, at `epochs` epochs. `None` when a
/// slot holds two values.
fn protocol(raw: &Raw, epochs: u8) -> Option<Protocol> {
    let mut slots = BTreeMap::new();
    for n in 0..3 {
        for e in 0..epochs {
            let values: Vec<u8> = raw
                .log
                .iter()
                .filter(|(m, f, _)| (*m, *f) == (n, e))
                .map(|(_, _, v)| *v)
                .collect();
            let slot = match (values.as_slice(), raw.pending.contains(&(n, e))) {
                ([], false) => Slot::Free,
                ([], true) => Slot::Reserved,
                ([v], true) => Slot::Volatile(*v),
                ([v], false) => Slot::Durable(*v),
                _ => return None,
            };
            slots.insert((n, e), slot);
        }
    }
    Some(Protocol {
        slots,
        acks: raw.acks.clone(),
    })
}

/// The one-step abstract check shared by [`check`] and the drift test: `label` from
/// `pre` to `post`, read through the refinement map. `Ok(Some(choose))` is a real
/// `Choose`, `Ok(None)` a stutter.
pub fn abstract_step(label: &str, pre: &Raw, post: &Raw) -> Result<Option<String>, Mismatch> {
    let a = chosen(pre).ok_or(Mismatch::Unprojectable)?;
    let b = chosen(post);
    if let Some(args) = label.strip_prefix("Ack(") {
        let (e, v) = args
            .strip_suffix(')')
            .and_then(|s| s.split_once(",value="))
            .and_then(|(e, v)| {
                let e: u8 = e.strip_prefix("epoch=")?.parse().ok()?;
                let v = u8::try_from(VALUES.iter().position(|x| *x == v)?).ok()?;
                Some((e, v))
            })
            .ok_or(Mismatch::NotAnEnabledChoose)?;
        // abstract_register.ctm, `Choose`: require chosen.get(epoch) in {None,
        // Some(value)}; next chosen = chosen.put(epoch, value).
        if a.get(&e).is_some_and(|x| *x != v) {
            return Err(Mismatch::NotAnEnabledChoose);
        }
        let mut want = a.clone();
        want.insert(e, v);
        if b.as_ref() != Some(&want) {
            return Err(Mismatch::NotAnEnabledChoose);
        }
        Ok((want != a).then(|| format!("Choose({args}")))
    } else if b.as_ref() == Some(&a) {
        Ok(None)
    } else {
        Err(Mismatch::NotAnEnabledChoose)
    }
}

/// IMPL-02's `durability_violations` predicate, for one step.
pub fn durability_violated(label: &str, pre: &Raw, post: &Raw) -> bool {
    let durable = pre.durable();
    let promoted = pre.pending.iter().any(|&(n, e)| {
        !post.pending.contains(&(n, e)) && post.log.iter().any(|&(m, f, _)| (m, f) == (n, e))
    });
    !durable.is_subset(&post.durable())
        || !pre.acks.is_subset(&post.acks)
        || (promoted && !label.starts_with("Sync("))
}

/// The durable register's reachable states at `epochs` epochs, as projected states.
pub fn reachable(epochs: u8) -> BTreeSet<Raw> {
    Spec {
        epochs,
        values: VALUES.to_vec(),
    }
    .reachable()
    .iter()
    .map(Protocol::raw)
    .collect()
}

/// Hold one journal's observed steps to the durable register at `epochs` epochs, and
/// through it to the abstract register. `reachable` is [`reachable`] at `epochs`.
pub fn check(steps: &[Observed], epochs: u8, reachable: &BTreeSet<Raw>) -> Correspondence {
    let spec = Spec {
        epochs,
        values: VALUES.to_vec(),
    };
    let mut out = Correspondence::default();
    for s in steps {
        out.steps += 1;
        let (Some(pre), Some(post)) = (&s.pre, &s.post) else {
            out.failures
                .push((Mismatch::Unprojectable, s.seq, s.event.clone()));
            continue;
        };
        let mut found = step(&spec, &mut out, &s.expect, pre, post, epochs);
        for state in [pre, post] {
            out.states.insert(state.clone());
            if !reachable.contains(state) && !found.contains(&Mismatch::Unreachable) {
                found.push(Mismatch::Unreachable);
            }
        }
        out.failures
            .extend(found.into_iter().map(|m| (m, s.seq, s.event.clone())));
    }
    out
}

/// One observed step's mismatches, in the order the step, the abstract register and
/// durability are checked; counts the step into `out`.
fn step(
    spec: &Spec,
    out: &mut Correspondence,
    expect: &Expect,
    pre: &Raw,
    post: &Raw,
    epochs: u8,
) -> Vec<Mismatch> {
    let label = match expect {
        Expect::Stutter => {
            if pre == post {
                out.stutters += 1;
                return vec![];
            }
            return vec![Mismatch::NotAStutter];
        }
        Expect::Step(want) | Expect::StepPrefix(want) => {
            let exact = matches!(expect, Expect::Step(_));
            let Some(from) = protocol(pre, epochs) else {
                return vec![Mismatch::Unprojectable];
            };
            let found = spec.step(&from).into_iter().find(|(l, t)| {
                (if exact {
                    l == want
                } else {
                    l.starts_with(want.as_str())
                }) && t.raw() == *post
            });
            let Some((label, _)) = found else {
                return vec![Mismatch::NotAnEnabledStep];
            };
            label
        }
    };
    if pre == post {
        out.stutters += 1;
    }
    *out.durable.entry(label.clone()).or_default() += 1;
    let kind = match label.split('(').next().unwrap_or_default() {
        "Lose" if pre.log == post.log => "Lose/reserved".to_owned(),
        "Lose" => "Lose/volatile".to_owned(),
        other => other.to_owned(),
    };
    *out.kinds.entry(kind).or_default() += 1;
    let mut found = vec![];
    match abstract_step(&label, pre, post) {
        Ok(Some(choose)) => *out.chooses.entry(choose).or_default() += 1,
        Ok(None) => {}
        Err(m) => found.push(m),
    }
    if durability_violated(&label, pre, post) {
        found.push(Mismatch::Durability);
    }
    found
}

/// Every step of the ported slot protocol over its reachable set at `epochs`, held to
/// the abstract register as IMPL-02's `correspondence` holds the engine's steps:
/// `(steps, stutters, chooses, realized, covered, failures)`, plus the durability
/// violations. The drift test compares these with IMPL-02's golden.
pub fn port_correspondence(epochs: u8) -> (usize, usize, usize, usize, usize, usize, usize) {
    let spec = Spec {
        epochs,
        values: VALUES.to_vec(),
    };
    let (mut steps, mut stutters, mut chooses, mut failures, mut durability) = (0, 0, 0, 0, 0);
    let mut realized = BTreeSet::new();
    let mut covered = BTreeSet::new();
    for s in spec.reachable() {
        let pre = s.raw();
        if let Some(c) = chosen(&pre) {
            covered.insert(c);
        }
        for (label, t) in spec.step(&s) {
            steps += 1;
            let post = t.raw();
            match abstract_step(&label, &pre, &post) {
                Ok(Some(c)) => {
                    chooses += 1;
                    realized.insert(c);
                }
                Ok(None) => stutters += 1,
                Err(_) => failures += 1,
            }
            if durability_violated(&label, &pre, &post) {
                durability += 1;
            }
        }
    }
    (
        steps,
        stutters,
        chooses,
        realized.len(),
        covered.len(),
        failures,
        durability,
    )
}

/// A projected state from its sets, by position: `log` holds `(node, epoch, value)`,
/// `pending` holds `(node, epoch)`, `acks` holds `(epoch, value)`. For crafted steps.
pub fn raw_of(log: &[(u8, u8, u8)], pending: &[(u8, u8)], acks: &[(u8, u8)]) -> Raw {
    Raw {
        log: log.iter().copied().collect(),
        pending: pending.iter().copied().collect(),
        acks: acks.iter().copied().collect(),
    }
}

/// A projected state's acknowledgements, `(epoch, value)`.
pub fn acks_of(raw: &Raw) -> &BTreeSet<(u8, u8)> {
    &raw.acks
}

/// Render a projected state as IMPL-02's evidence does: one slot per replica and epoch.
pub fn render_raw(raw: &Raw) -> String {
    let epochs = raw
        .log
        .iter()
        .map(|(_, e, _)| *e)
        .chain(raw.pending.iter().map(|(_, e)| *e))
        .chain(raw.acks.iter().map(|(e, _)| *e))
        .max()
        .map_or(1, |e| e + 1);
    let mut parts = Vec::new();
    if let Some(p) = protocol(raw, epochs) {
        for (&(n, e), slot) in &p.slots {
            let s = match slot {
                Slot::Free => "-".to_owned(),
                Slot::Reserved => "R".to_owned(),
                Slot::Volatile(v) => format!("V{v}"),
                Slot::Durable(v) => format!("D{v}"),
            };
            parts.push(format!("{}{e}:{s}", NODES[usize::from(n)]));
        }
    }
    let acks: Vec<String> = raw
        .acks
        .iter()
        .map(|(e, v)| format!("{e}{}", VALUES[usize::from(*v)]))
        .collect();
    format!("{} acks={{{}}}", parts.join(" "), acks.join(","))
}

// ---------------------------------------------------------------------------
// verbatim port of crates/continuum-cml-elab/tests/pr16_impl02_durable_register.rs
// (drift-checked as a whole against the nine items joined in order; edit only by
// re-copying)
// ---------------------------------------------------------------------------

const NODES: [&str; 3] = ["a", "b", "c"];
const MAJORITY: &[&[&str]] = &[&["a", "b"], &["a", "c"], &["b", "c"]];

/// A concrete state as sets, by position: node, epoch, value.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Raw {
    log: BTreeSet<(u8, u8, u8)>,
    pending: BTreeSet<(u8, u8)>,
    acks: BTreeSet<(u8, u8)>,
}

impl Raw {
    fn durable(&self) -> BTreeSet<(u8, u8, u8)> {
        self.log
            .iter()
            .filter(|(n, e, _)| !self.pending.contains(&(*n, *e)))
            .copied()
            .collect()
    }
}

/// One replica's log slot for one epoch, as `replicated_register.md` and the storage
/// strata of RFC 0007 describe it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Slot {
    Free,
    Reserved,
    Volatile(u8),
    Durable(u8),
}

/// The protocol over three replicas `a b c`, majority quorums, `epochs` epochs, and
/// `values` values, by position.
struct Spec {
    epochs: u8,
    values: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Protocol {
    slots: BTreeMap<(u8, u8), Slot>,
    acks: BTreeSet<(u8, u8)>,
}

impl Protocol {
    fn raw(&self) -> Raw {
        let mut raw = Raw {
            log: BTreeSet::new(),
            pending: BTreeSet::new(),
            acks: self.acks.clone(),
        };
        for (&(n, e), slot) in &self.slots {
            match *slot {
                Slot::Free => {}
                Slot::Reserved => {
                    raw.pending.insert((n, e));
                }
                Slot::Volatile(v) => {
                    raw.pending.insert((n, e));
                    raw.log.insert((n, e, v));
                }
                Slot::Durable(v) => {
                    raw.log.insert((n, e, v));
                }
            }
        }
        raw
    }
}

impl Spec {
    fn init(&self) -> Protocol {
        let mut slots = BTreeMap::new();
        for n in 0..3 {
            for e in 0..self.epochs {
                slots.insert((n, e), Slot::Free);
            }
        }
        Protocol {
            slots,
            acks: BTreeSet::new(),
        }
    }

    fn step(&self, s: &Protocol) -> BTreeSet<(String, Protocol)> {
        let mut out = BTreeSet::new();
        let with = |n: u8, e: u8, slot: Slot| {
            let mut t = s.clone();
            t.slots.insert((n, e), slot);
            t
        };
        for (&(n, e), &slot) in &s.slots {
            let at = format!("n={},epoch={e}", NODES[usize::from(n)]);
            match slot {
                Slot::Free => {
                    out.insert((format!("Reserve({at})"), with(n, e, Slot::Reserved)));
                }
                Slot::Reserved => {
                    out.insert((format!("Abort({at})"), with(n, e, Slot::Free)));
                    for (v, name) in self.values.iter().enumerate() {
                        let v = u8::try_from(v).expect("few values");
                        out.insert((
                            format!("Submit({at},value={name})"),
                            with(n, e, Slot::Volatile(v)),
                        ));
                        // A crash takes the permit: every value names the empty bytes.
                        out.insert((format!("Lose({at},value={name})"), with(n, e, Slot::Free)));
                    }
                }
                Slot::Volatile(v) => {
                    out.insert((format!("Sync({at})"), with(n, e, Slot::Durable(v))));
                    let name = self.values[usize::from(v)];
                    out.insert((format!("Lose({at},value={name})"), with(n, e, Slot::Free)));
                }
                Slot::Durable(_) => {}
            }
        }
        for e in 0..self.epochs {
            for (v, name) in self.values.iter().enumerate() {
                let v = u8::try_from(v).expect("few values");
                let durable = |n: &str| {
                    let i = u8::try_from(NODES.iter().position(|x| *x == n).expect("node"))
                        .expect("three nodes");
                    s.slots[&(i, e)] == Slot::Durable(v)
                };
                if MAJORITY.iter().any(|q| q.iter().all(|n| durable(n))) {
                    let mut t = s.clone();
                    t.acks.insert((e, v));
                    out.insert((format!("Ack(epoch={e},value={name})"), t));
                }
            }
        }
        out
    }

    fn reachable(&self) -> BTreeSet<Protocol> {
        let mut seen = BTreeSet::from([self.init()]);
        let mut work = vec![self.init()];
        while let Some(s) = work.pop() {
            for (_, t) in self.step(&s) {
                if seen.insert(t.clone()) {
                    work.push(t);
                }
            }
        }
        seen
    }
}
