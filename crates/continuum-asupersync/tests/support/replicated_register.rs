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
//! | [`Act::Crash`] | `Cancel` of the replica's region | a crash, modelled as graceful region cancellation: the incarnation's tasks run their cancellation cleanup, in-flight obligations abort, the incarnation ends |
//! | [`Act::CrashRepropose`] | as [`Act::Crash`] | a crash as [`Act::Crash`], and the next incarnation is proposed another value for one epoch |
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
//! # What a crash is here, and what it is not (bn-20d8u)
//!
//! A crash is modelled as graceful region cancellation, [`CRASH_SEMANTICS`]. The replica
//! cancels its incarnation's region through the binding's `Cancel`. Each writer task then
//! observes its cancellation at a checkpoint, and the binding's gate runs its cleanup: it
//! aborts every obligation the task holds, for `Cancel`, and the task ends. So the permit
//! and the unsynced bytes of a crashed writer are aborted by the crashed incarnation's
//! own cleanup, and the projection reads those aborts as `Lose`. The incarnation's tasks
//! end and its region finalizes, and a timer that one of its tasks armed is dropped.
//!
//! The process pack's profile `process/crash-restart-v0` (bn-3mmf) calls this graceful
//! cancellation, not a crash. Its fail-stop crash runs nothing more of the incarnation,
//! no finalizer and no cancellation handler, and leaves each operation the incarnation
//! began pending, with a late completion fenced by `(node, epoch)`. [`build`] and
//! [`build_with_shutdown`] realize every crash as graceful region cancellation, and
//! every claim made of their runs, here and in `register_baseline.rs` and
//! `register_mutants.rs`, is a claim about graceful region cancellation.
//!
//! # The fail-stop crash (bn-20d8u)
//!
//! [`build_with_shutdown_in`] with [`CrashMode::FailStop`] builds the same program with
//! each crash act realized as the binding's `Crash` of the incarnation's region instead
//! ([`FAIL_STOP_SEMANTICS`]): the incarnation's tasks stop where they are, run nothing
//! (no cancellation handler, no cleanup), and what they held is fenced: the write permit,
//! the unsynced bytes' `IoOp`. The projection reads the loss from the crash event itself:
//! at `region-crashed`, every attempt of a stopped writer that is still reserved or
//! volatile is lost (one `Lose` step each, by attempt order), and the fences that follow
//! stutter. A durable attempt stays durable. So the refinement map does not rest on any
//! cleanup, and `register_baseline.rs` reads quiescence and conservation with the stopped
//! tasks and the fenced obligations counted apart.
//!
//! Virtual time is not used by [`build`], [`build_with_shutdown`] or
//! [`build_with_shutdown_in`]: no step of the durable register waits for a timer, and a
//! sleeping writer would only add a refusal (`TaskAsleep`) to the admissibility rule. The
//! timer carrier below uses it, on a task that no other actor commands.
//!
//! # The carrier: a process epoch carried in data (bn-2faf1)
//!
//! [`build_carried`] gives each incarnation of a replica a process epoch, one more than the
//! one before it ([`process_epoch`], [`Fence`]), and carries that epoch in data to the next
//! incarnation, at the replica's last crash ([`carrier_sites`], [`CARRIER_SEMANTICS`]):
//!
//! - [`Carrier::Message`]: the crashed incarnation's writer has a submit in flight, not
//!   synced. Its late completion is a message it sends, before the crash, on a mailbox the
//!   next incarnation's writer of the slot receives; the payload names the crashed
//!   incarnation's process epoch, the slot and the value.
//! - [`Carrier::Timer`]: at the same site, the incarnation arms a timer on its node's
//!   supervisor ([`Role::Supervisor`]), a task in its own region outside every
//!   incarnation's, so neither crash semantics touches it. After the crash the clock
//!   passes the deadline, and the supervisor's callback sends the payload on the mailbox.
//! - [`Carrier::Recovery`], the boundary: after a crash between a `Sync` and its `Confirm`,
//!   the supervisor's recovery report names the new incarnation's own process epoch and the
//!   durable record, right after the restart.
//!
//! The receiver acts on a payload, by confirming its slot and value, only when the payload
//! names its own process epoch ([`accepts`]); otherwise it drops it. The binding carries no
//! payload and has no data-dependent control flow, so each payload is a program-side fact
//! bound to its mailbox ([`Roles::mailboxes`]), and the check is decided at build time from
//! that payload and the receiver's epoch: the built program is the check's outcome. Each
//! mailbox carries one message, from one sender, so the payload a receive delivers is fixed
//! by the program, not by the schedule. A mutant changes the fence ([`Fence::reuse_epoch`]
//! for M03, [`Fence::check_timers`] for M08), never the carrier. The receiver is always the
//! last incarnation, which never crashes, because a fail-stop crash of a task that holds a
//! channel's receiver is the binding's typed `CrashUnsupported`. A crash with no submit in
//! flight has no pending completion, and so no message or timer site.
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
//! The program carries no data except a carried build's payloads. A writer's value is a
//! label of the role table. The projection corroborates it only for writers that confirm:
//! each confirmation must go to the coordinator of the sender's epoch and value. A writer
//! that crashes before it confirms has an uncorroborated value, which the step check covers
//! only in the aggregate: a wrong label there is refuted where an ack quorum depends on it.
//!
//! In a carried build (bn-2faf1), each mailbox's channel must be opened, in order after the
//! coordinators', to the writer the roles name, each send on it must come from the
//! payload's node (its writer of the slot for a message, its supervisor otherwise), and a
//! confirmation may also name a slot and value the journal has delivered to the sender in
//! a payload: that confirmation is corroborated by the delivery, not by the sender's label.
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
//! - bn-5fpl (IMPL-04, the correct version) added [`Act::CrashRepropose`], so an
//!   incarnation's value can differ from the one before it ([`incarnation_values`]),
//!   and [`build_with_shutdown`], whose runs end quiescent. The coordinators now count
//!   confirmations, which is the replica count for every plan that confirms each epoch
//!   at most once per replica, as every IMPL-03 plan does. The correct protocol, the
//!   baseline campaign and the four scenario properties are in
//!   `support/register_baseline.rs`; a mutant is run beside that campaign.
//! - bn-28oa (IMPL-05) added [`Built::labels`], each task's program label by ordinal,
//!   and [`with_op`], so a program mutant can retarget or insert operations. The
//!   mutants themselves are in `support/register_mutants.rs`. [`build`] and
//!   [`build_with_shutdown`] build the same programs as before.
//! - bn-2faf1 added the carrier ([`build_carried`]), so M03 and M08 run against a process
//!   epoch carried in data. A plan with no carrier site builds as before, operation for
//!   operation, and the admissibility rule gained a full-channel wait for a confirmation
//!   ([`CONFIRM_CAPACITY`]) that no plan of the correct program reaches. Every earlier
//!   campaign identity in the IMPL-03, IMPL-04 and IMPL-05 goldens is the same as before
//!   bn-2faf1 (checked by diff; the IMPL-03 and IMPL-04 goldens are byte-identical).

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

/// What [`Act::Crash`] and [`Act::CrashRepropose`] are, in the terms of the process
/// pack's profile `process/crash-restart-v0` (bn-3mmf): its `graceful-cancellation`
/// row, not its `fail-stop-crash` row. The evidence renders this line, and
/// `pr16_impl03_replicated_register.rs` holds the program to it.
pub const CRASH_SEMANTICS: &str = "a crash is modelled as graceful region cancellation (Cancel of the \
     incarnation's region, whose tasks run their cancellation cleanup and abort what they hold), \
     process/crash-restart-v0 row graceful-cancellation, which that profile marks Unsupported; \
     a fail-stop crash, which runs no \
     handler and fences pending completions by (node, epoch), is run separately where the evidence says \
     fail-stop: bn-20d8u";

/// What [`Act::Crash`] and [`Act::CrashRepropose`] are in a [`CrashMode::FailStop`] build
/// (bn-20d8u): the profile's `fail-stop-crash` row. The evidence renders this line beside
/// [`CRASH_SEMANTICS`].
pub const FAIL_STOP_SEMANTICS: &str = "a fail-stop crash is the binding's Crash of the incarnation's region: \
     its tasks stop where they are and run nothing, no cancellation handler and no cleanup; the permit and \
     the unsynced IoOp they held are fenced, never aborted, and the projection reads their Lose from the \
     crash event; process/crash-restart-v0 row fail-stop-crash (bn-20d8u)";

/// What the carried builds are ([`build_carried`], bn-2faf1). The evidence renders this
/// line.
pub const CARRIER_SEMANTICS: &str = "a process epoch carried in data: each incarnation of a replica has a process \
     epoch, one more than the one before it; at a replica's last crash with a submit in flight, the late completion \
     of that submit (a message the crashed writer sends) or a timer it armed on the node's supervisor outside the \
     incarnation's region (whose callback sends after the clock passes it) carries the crashed incarnation's process \
     epoch, slot and value to the new incarnation right after the restart; the receiver acts on a payload, by \
     confirming its slot and value, only when the payload names its own process epoch; the recovery carrier sends, \
     after a crash between sync and confirm, a report that names the new incarnation's own epoch, which it confirms \
     from; the binding carries no payload, so the check is decided at build time from the payload and the receiver's \
     epoch, and the projection corroborates a confirmation made from a payload by the journal's delivery of it \
     (bn-2faf1); the payload's process epoch is a program-side fact bound to its mailbox and never appears in the \
     journal, and the supervisor's send is a scripted operation after the Advance, not a substrate callback of the \
     timer's fire; what the journal shows is each payload's receipt and whether the receiver's next step confirms it";

/// How a build realizes a crash act (bn-20d8u).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrashMode {
    /// `Cancel` of the incarnation's region: graceful region cancellation.
    Graceful,
    /// `Crash` of the incarnation's region: the profile's fail-stop crash.
    FailStop,
}

/// A process epoch carried in data (bn-2faf1): what carries it to a restarted replica's new
/// incarnation. See "The carrier" in the module documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Carrier {
    /// The late completion of the crashed incarnation's pending submit: a message its
    /// writer sends to the node's mailbox before the crash, which names that incarnation's
    /// process epoch and the slot and value it submitted. The new incarnation receives it
    /// right after the restart.
    Message,
    /// A timer the crashed incarnation arms on its node's supervisor, outside every
    /// incarnation's region, before the crash, with the same payload. The clock passes
    /// its deadline after the crash, and the supervisor delivers the payload to the new
    /// incarnation.
    Timer,
    /// The boundary: right after a restart that follows a synced and unconfirmed write,
    /// the supervisor's recovery report names the new incarnation's own process epoch and
    /// the durable record. The new incarnation confirms from it, in place of its scripted
    /// confirmation.
    Recovery,
}

/// How a program checks a carried process epoch (bn-2faf1). The correct program is
/// [`CORRECT_FENCE`]; a mutant changes one field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fence {
    /// A restarted incarnation takes the process epoch of the incarnation before it,
    /// instead of one more (M03's defect).
    pub reuse_epoch: bool,
    /// A timer's delivery is held to the epoch check (M08's defect drops it).
    pub check_timers: bool,
}

/// The correct program's fence: every incarnation's process epoch is one more than the one
/// before it, and every carried payload, message or timer, is held to the epoch check.
pub const CORRECT_FENCE: Fence = Fence {
    reuse_epoch: false,
    check_timers: true,
};

/// The process epoch of incarnation `inc` of a replica under `fence`: `inc`, or `0` for
/// every incarnation when a restart reuses the epoch before it.
#[must_use]
pub const fn process_epoch(inc: usize, fence: Fence) -> usize {
    if fence.reuse_epoch { 0 } else { inc }
}

/// The receiver's epoch check: whether an incarnation whose process epoch is `own` acts on
/// a payload that `via` carried and that names process epoch `carried`. The correct check
/// accepts exactly the payloads of its own epoch.
#[must_use]
pub fn accepts(fence: Fence, via: Carrier, own: usize, carried: usize) -> bool {
    (via == Carrier::Timer && !fence.check_timers) || own == carried
}

/// A carried payload: the slot and value a replica's incarnation, by process epoch,
/// reports as written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Payload {
    /// What carries it.
    pub via: Carrier,
    /// Replica index.
    pub node: u8,
    /// The process epoch it names.
    pub process: usize,
    /// The slot's epoch.
    pub epoch: u8,
    /// The value index.
    pub value: u8,
}

/// One mailbox of a carried build: the task that receives on it, by ordinal, the payload
/// of the one message it carries, and whether the receiver acts on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mailbox {
    /// Its channel ordinal: after every coordinator's channel, in site order.
    pub channel: u32,
    /// The one task that sends on it, by ordinal: the crashed incarnation's writer of the
    /// slot for a message, the node's supervisor otherwise.
    pub sender: u32,
    /// The receiving writer's task ordinal.
    pub taker: u32,
    /// The payload.
    pub payload: Payload,
    /// Whether the receiver's epoch check accepts it.
    pub accepted: bool,
}

/// The capacity of each coordinator's channel.
pub const CONFIRM_CAPACITY: u32 = 4;

/// The virtual timer the [`Carrier::Timer`] arms, and how far the clock moves after the
/// crash.
pub const CARRIER_TIMER: (u64, u64) = (10, 20);

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
    /// The replica crashes: its incarnation's region is cancelled. This is graceful
    /// region cancellation, not a fail-stop crash ([`CRASH_SEMANTICS`]).
    Crash,
    /// The replica crashes as [`Act::Crash`] does, and a client proposes `value` for
    /// `epoch` to its next incarnation: that incarnation's writer for `epoch` writes
    /// `value` (PR-16/IMPL-04, bn-5fpl). Every other epoch keeps its value.
    CrashRepropose(u8, u8),
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
    /// Replica `node`'s supervisor, in its own region outside every incarnation's: it
    /// holds the node's timers and its recovery report (bn-2faf1, [`Carrier`]).
    Supervisor {
        /// Replica index.
        node: u8,
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
    /// The carried build's mailboxes, in the order the setup opens them, after every
    /// coordinator's channel; empty for a build with no carrier (bn-2faf1).
    pub mailboxes: Vec<Mailbox>,
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
    /// Whether it waits for every other actor to finish: the shutdown's operations.
    last: bool,
}

/// A built plan: the binding programs, the roles, and what the log generators need.
#[derive(Debug, Clone)]
pub struct Built {
    /// Actor 0 is the setup; then replicas `a b c`; then the coordinators.
    pub programs: Vec<Program>,
    /// Task roles by ordinal.
    pub roles: Roles,
    /// Each task's program label, by journal ordinal, as [`Roles::tasks`] indexes them:
    /// what a program mutant rewrites (PR-16/IMPL-05, bn-28oa).
    pub labels: Vec<TaskLabel>,
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

/// Each incarnation's values, by epoch: `plan.values[n]` for the first incarnation of
/// replica `n`, and the one before it, with a re-proposal applied, for each later one.
pub fn incarnation_values(plan: &Plan, n: usize) -> Vec<[u8; 2]> {
    let mut out = vec![plan.values[n]];
    for act in &plan.replicas[n] {
        let mut next = *out.last().expect("one incarnation");
        match *act {
            Act::Crash => {}
            Act::CrashRepropose(e, v) => next[usize::from(e)] = v,
            _ => continue,
        }
        out.push(next);
    }
    out
}

/// The confirmations of a plan: `(epoch, value)` of each `Confirm`, in script order,
/// replica by replica. The value is the confirming incarnation's.
fn confirmations(plan: &Plan) -> Vec<(u8, u8)> {
    let mut out = Vec::new();
    for n in 0..3 {
        let values = incarnation_values(plan, n);
        let mut inc = 0;
        for act in &plan.replicas[n] {
            match *act {
                Act::Crash | Act::CrashRepropose(..) => inc += 1,
                Act::Confirm(e) => out.push((e, values[inc][usize::from(e)])),
                _ => {}
            }
        }
    }
    out
}

/// One carrier site (bn-2faf1): the crash act of replica `node` it sits at, by script
/// index, its payload, and whether the new incarnation's epoch check accepts it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Site {
    /// Replica index.
    pub node: u8,
    /// The crash act's index in the replica's script.
    pub at: usize,
    /// The crashed incarnation's index; the receiver is the next one.
    pub crashed: usize,
    /// The payload.
    pub payload: Payload,
    /// Whether the receiver acts on it.
    pub accepted: bool,
}

/// The carrier sites of `plan` (bn-2faf1). Each replica has at most one: its last crash,
/// so the receiver is its last incarnation, which never crashes (a fail-stop crash of a
/// task that holds a channel's receiver is the binding's `CrashUnsupported`).
///
/// - [`Carrier::Message`] and [`Carrier::Timer`]: the last crash of a replica whose
///   crashed incarnation has exactly one submit in flight, not synced; the payload names
///   that incarnation's process epoch, the slot and its value. A crash with no submit in
///   flight has no pending completion, and so no site.
/// - [`Carrier::Recovery`]: the last crash of a replica right after a `Sync(e)` and right
///   before the new incarnation's `Confirm(e)`; the payload names the new incarnation's
///   process epoch and the durable record.
#[must_use]
pub fn carrier_sites(plan: &Plan, carrier: Carrier, fence: Fence) -> Vec<Site> {
    let mut out = Vec::new();
    for n in 0..3_u8 {
        let script = &plan.replicas[usize::from(n)];
        let crashes: Vec<usize> = script
            .iter()
            .enumerate()
            .filter(|(_, a)| matches!(a, Act::Crash | Act::CrashRepropose(..)))
            .map(|(i, _)| i)
            .collect();
        let Some(&at) = crashes.last() else {
            continue;
        };
        let crashed = crashes.len() - 1;
        let start = crashes.len().checked_sub(2).map_or(0, |i| crashes[i] + 1);
        let values = incarnation_values(plan, usize::from(n));
        let (process, epoch, value) = match carrier {
            Carrier::Message | Carrier::Timer => {
                let mut pending = BTreeSet::new();
                for act in &script[start..at] {
                    match *act {
                        Act::Submit(e) => {
                            pending.insert(e);
                        }
                        Act::Sync(e) => {
                            pending.remove(&e);
                        }
                        _ => {}
                    }
                }
                let [e] = pending.into_iter().collect::<Vec<_>>()[..] else {
                    continue;
                };
                (
                    process_epoch(crashed, fence),
                    e,
                    values[crashed][usize::from(e)],
                )
            }
            Carrier::Recovery => {
                let Some(Act::Sync(e)) = at.checked_sub(1).map(|i| script[i]) else {
                    continue;
                };
                if script.get(at + 1) != Some(&Act::Confirm(e)) {
                    continue;
                }
                (
                    process_epoch(crashed + 1, fence),
                    e,
                    values[crashed + 1][usize::from(e)],
                )
            }
        };
        let payload = Payload {
            via: carrier,
            node: n,
            process,
            epoch,
            value,
        };
        out.push(Site {
            node: n,
            at,
            crashed,
            payload,
            accepted: accepts(fence, carrier, process_epoch(crashed + 1, fence), process),
        });
    }
    out
}

/// The coordinators of a plan: every `(epoch, value)` some replica confirms, with how
/// many confirmations it gets. With carrier sites (bn-2faf1), an accepted payload is a
/// confirmation of its slot and value, and a recovery site replaces the scripted
/// confirmation that follows its crash.
fn coordinators_with(plan: &Plan, sites: &[Site]) -> Vec<(u8, u8, usize)> {
    let mut confirmed = confirmations(plan);
    for s in sites {
        if s.payload.via == Carrier::Recovery {
            // The scripted `Confirm(e)` right after the crash confirms the new
            // incarnation's value of `e`, which is the payload's value: both read
            // `incarnation_values` of that incarnation, so it is in `confirmed`.
            let replaced = (s.payload.epoch, s.payload.value);
            let i = confirmed
                .iter()
                .position(|c| *c == replaced)
                .expect("the scripted confirmation a recovery site replaces");
            confirmed.remove(i);
        }
        if s.accepted {
            confirmed.push((s.payload.epoch, s.payload.value));
        }
    }
    let mut out = Vec::new();
    for e in 0..plan.epochs {
        for v in 0..u8::try_from(VALUES.len()).expect("two values") {
            let writers = confirmed.iter().filter(|c| **c == (e, v)).count();
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
    build_inner(plan, false, CrashMode::Graceful, &[], false)
}

/// The binding programs of `plan` with the service's shutdown (PR-16/IMPL-04, bn-5fpl):
/// [`build`], plus one last actor whose operations wait until every other actor has
/// finished. It drains each coordinator's channel of the confirmations left after its
/// majority, finishes each coordinator, finishes each writer of each replica's last
/// incarnation, closes those regions and the coordinators' region, and closes the root.
/// A cancelled incarnation's writers and region ended with its crash. So a run of it
/// ends quiescent: every task ended, every region finalized, and nothing queued. The
/// shutdown uses no label that [`build`] does not bind, so the other actors' programs
/// are [`build`]'s, operation for operation.
///
/// # Panics
///
/// As [`build`].
pub fn build_with_shutdown(plan: &Plan) -> Built {
    build_inner(plan, true, CrashMode::Graceful, &[], false)
}

/// [`build_with_shutdown`] with each crash act realized as `mode` says (bn-20d8u). The
/// graceful build is [`build_with_shutdown`], operation for operation.
///
/// # Panics
///
/// As [`build`].
pub fn build_with_shutdown_in(plan: &Plan, mode: CrashMode) -> Built {
    build_inner(plan, true, mode, &[], false)
}

/// [`build_with_shutdown_in`] with `carrier` at every site of [`carrier_sites`], and each
/// payload held to `fence` (bn-2faf1). A plan with no site builds exactly as
/// [`build_with_shutdown_in`] does, operation for operation. At a site of replica `n`,
/// whose last crash ends incarnation `k`:
///
/// - [`Carrier::Message`]: right before the crash, `k`'s writer of the slot sends the
///   payload on the site's mailbox; right after it, the last incarnation's writer of the
///   slot receives it;
/// - [`Carrier::Timer`]: right before the crash, `n`'s supervisor sleeps
///   [`CARRIER_TIMER`]`.0` (the timer, armed outside the incarnation's region); right after
///   it, the clock advances [`CARRIER_TIMER`]`.1`, the supervisor sends the payload on the
///   mailbox (the timer's callback), and the last incarnation's writer receives it;
/// - [`Carrier::Recovery`]: right after the crash, the supervisor sends the recovery
///   report on the mailbox and the writer receives it, and the scripted `Confirm` that
///   follows the crash is dropped.
///
/// Then, when the receiver's epoch check accepts the payload ([`accepts`]), the receiver
/// confirms the payload's slot and value to that coordinator; otherwise it drops it. The
/// binding has no data-dependent control flow, so the check is decided here, from the
/// payload and the receiver's process epoch, and the program is its outcome; the journal
/// carries the payload's delivery, and [`observe`] reads it back ([`Roles::mailboxes`]).
///
/// # Panics
///
/// As [`build`].
pub fn build_carried(plan: &Plan, mode: CrashMode, carrier: Carrier, fence: Fence) -> Built {
    build_inner(
        plan,
        true,
        mode,
        &carrier_sites(plan, carrier, fence),
        false,
    )
}

/// [`build_carried`] with explicit `sites`, for a test that needs a payload
/// [`carrier_sites`] does not make, such as a recovery report naming a stale epoch.
///
/// # Panics
///
/// As [`build`], and on a site that is not at a crash act of its replica.
pub fn build_with_sites(plan: &Plan, mode: CrashMode, sites: &[Site]) -> Built {
    build_inner(plan, true, mode, sites, false)
}

/// [`build_with_shutdown`] that spawns only the owners the scripts use (PR 18's owner
/// reduction, bn-25z9o): an incarnation's region is opened only when the incarnation has
/// an act, crash acts included, and its writer for epoch `e` is spawned only when the
/// incarnation has an act on `e`. So a replica with an empty script has no region and no
/// task, as a node that never starts; the model still has three nodes, and its slots
/// stay free. The shutdown finishes and closes only what was spawned and opened. Every
/// other operation is [`build_with_shutdown`]'s, and a plan whose every incarnation acts
/// on every epoch builds exactly as [`build_with_shutdown`] does.
///
/// # Panics
///
/// As [`build`].
pub fn build_pruned(plan: &Plan) -> Built {
    build_inner(plan, true, CrashMode::Graceful, &[], true)
}

/// `built` without operation `index` of actor `actor`, its admissibility facts kept in
/// step: a program mutant for bn-28oa, such as a shutdown with one `Finish` missing.
///
/// # Panics
///
/// When the actor or the operation does not exist.
#[must_use]
pub fn without_op(built: &Built, actor: usize, index: usize) -> Built {
    let mut out = built.clone();
    out.programs[actor].remove(index);
    out.meta[actor].remove(index);
    out
}

/// `built` with `op` inserted as operation `index` of actor `actor`, with no
/// admissibility facts of its own: a program mutant for bn-28oa. It may command only a
/// task that no coordinator's admissibility depends on (a writer), or none.
///
/// # Panics
///
/// When the actor does not exist, `index` is past its end, or `op` commands a task that
/// is not a writer.
#[must_use]
pub fn with_op(built: &Built, actor: usize, index: usize, op: SubstrateOp) -> Built {
    let task = match &op {
        SubstrateOp::Begin { task }
        | SubstrateOp::Continue { task }
        | SubstrateOp::Finish { task }
        | SubstrateOp::Reserve { task, .. }
        | SubstrateOp::Acquire { task, .. }
        | SubstrateOp::Sleep { task, .. }
        | SubstrateOp::Send { task, .. } => Some(*task),
        _ => None,
    };
    assert!(
        !matches!(op, SubstrateOp::Recv { .. }),
        "a receive commands a coordinator"
    );
    if let Some(t) = task {
        let ordinal = built.labels.iter().position(|l| *l == t);
        assert!(
            ordinal.is_some_and(|i| matches!(built.roles.tasks[i].0, Role::Writer { .. })),
            "with_op commands only a writer"
        );
    }
    let mut out = built.clone();
    out.programs[actor].insert(index, op);
    out.meta[actor].insert(
        index,
        Meta {
            commands: None,
            feeds: None,
            recv: false,
            last: false,
        },
    );
    out
}

fn build_inner(plan: &Plan, shutdown: bool, mode: CrashMode, sites: &[Site], prune: bool) -> Built {
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
    let mut task_labels: Vec<TaskLabel> = Vec::new();

    // Regions and writers: every incarnation of every replica.
    let incarnations: Vec<usize> = plan
        .replicas
        .iter()
        .map(|s| {
            1 + s
                .iter()
                .filter(|a| matches!(a, Act::Crash | Act::CrashRepropose(..)))
                .count()
        })
        .collect();
    // writer[(n, inc, e)] = task label; region[(n, inc)] = region label
    let mut writer: BTreeMap<(u8, usize, u8), TaskLabel> = BTreeMap::new();
    let mut region: BTreeMap<(u8, usize), RegionLabel> = BTreeMap::new();
    let mut spawns = Vec::new();
    // What each incarnation's acts use, for a pruned build: `(n, inc, None)` for any act,
    // `(n, inc, Some(e))` for an act on epoch `e`.
    let mut used: BTreeSet<(u8, usize, Option<u8>)> = BTreeSet::new();
    for n in 0..3_u8 {
        let mut inc = 0;
        for act in &plan.replicas[usize::from(n)] {
            used.insert((n, inc, None));
            match *act {
                Act::Crash | Act::CrashRepropose(..) => inc += 1,
                Act::Reserve(e)
                | Act::Submit(e)
                | Act::Sync(e)
                | Act::Release(e)
                | Act::Abort(e)
                | Act::Confirm(e) => {
                    used.insert((n, inc, Some(e)));
                }
            }
        }
    }
    for n in 0..3_u8 {
        let values = incarnation_values(plan, usize::from(n));
        for (inc, values) in values.iter().enumerate() {
            if prune && !used.contains(&(n, inc, None)) {
                continue;
            }
            let r = RegionLabel(labels.take());
            setup.push(SubstrateOp::OpenRegion {
                parent: RegionLabel::ROOT,
                child: r,
            });
            region_ordinal += 1;
            region.insert((n, inc), r);
            for e in 0..plan.epochs {
                if prune && !used.contains(&(n, inc, Some(e))) {
                    continue;
                }
                let t = TaskLabel(labels.take());
                task_labels.push(t);
                writer.insert((n, inc, e), t);
                let value = values[usize::from(e)];
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
    let coords = coordinators_with(plan, sites);
    let coord_region = RegionLabel(labels.take());
    setup.push(SubstrateOp::OpenRegion {
        parent: RegionLabel::ROOT,
        child: coord_region,
    });
    region_ordinal += 1;
    let coord_ordinal = region_ordinal;
    // The supervisors of the nodes whose site needs one, each in its own region under the
    // root, outside every incarnation (bn-2faf1).
    let mut supervisor: BTreeMap<u8, (RegionLabel, TaskLabel)> = BTreeMap::new();
    let mut supervisor_region: BTreeMap<u8, u32> = BTreeMap::new();
    for site in sites {
        if site.payload.via != Carrier::Message {
            let r = RegionLabel(labels.take());
            setup.push(SubstrateOp::OpenRegion {
                parent: RegionLabel::ROOT,
                child: r,
            });
            region_ordinal += 1;
            supervisor_region.insert(site.node, region_ordinal);
            supervisor.insert(site.node, (r, TaskLabel(labels.take())));
        }
    }
    let mut coord_task = Vec::new();
    let mut channel = BTreeMap::new();
    for (i, &(e, v, _)) in coords.iter().enumerate() {
        let t = TaskLabel(labels.take());
        task_labels.push(t);
        tasks.push((Role::Coordinator { epoch: e, value: v }, coord_ordinal));
        spawns.push((coord_region, t));
        coord_task.push(t);
        channel.insert((e, v), (i, ChannelLabel(labels.take())));
    }
    for (&node, &(r, t)) in &supervisor {
        task_labels.push(t);
        tasks.push((Role::Supervisor { node }, supervisor_region[&node]));
        spawns.push((r, t));
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
            capacity: CONFIRM_CAPACITY,
            receiver: coord_task[i],
        });
    }
    // One mailbox per site, received by the slot's writer in the last incarnation.
    let mut mailbox: BTreeMap<u8, ChannelLabel> = BTreeMap::new();
    let mut mailboxes = Vec::new();
    let ordinal_of = |t: TaskLabel| {
        u32::try_from(task_labels.iter().position(|x| *x == t).expect("a task")).expect("few tasks")
    };
    for (k, site) in sites.iter().enumerate() {
        let c = ChannelLabel(labels.take());
        let taker = writer[&(site.node, site.crashed + 1, site.payload.epoch)];
        let sender = match site.payload.via {
            Carrier::Message => writer[&(site.node, site.crashed, site.payload.epoch)],
            Carrier::Timer | Carrier::Recovery => supervisor[&site.node].1,
        };
        setup.push(SubstrateOp::OpenChannel {
            channel: c,
            capacity: 1,
            receiver: taker,
        });
        mailbox.insert(site.node, c);
        mailboxes.push(Mailbox {
            channel: u32::try_from(coords.len() + k).expect("few channels"),
            sender: ordinal_of(sender),
            taker: ordinal_of(taker),
            payload: site.payload,
            accepted: site.accepted,
        });
    }

    let none = Meta {
        commands: None,
        feeds: None,
        recv: false,
        last: false,
    };
    let mut programs = vec![setup.clone()];
    let mut meta = vec![vec![none; setup.len()]];

    // Replicas.
    for n in 0..3_u8 {
        let values = incarnation_values(plan, usize::from(n));
        let mut inc = 0;
        let mut permit: BTreeMap<u8, ReservationLabel> = BTreeMap::new();
        let mut bytes: BTreeMap<u8, ReservationLabel> = BTreeMap::new();
        let mut ops = Vec::new();
        let mut m = Vec::new();
        let site = sites.iter().find(|s| s.node == n);
        let mut skip = None;
        for (at, act) in plan.replicas[usize::from(n)].iter().enumerate() {
            if skip == Some(at) {
                continue;
            }
            let carried_here = site.filter(|s| s.at == at);
            if let Some(s) = carried_here {
                // Before the crash: the message's send, or the timer's arming.
                match s.payload.via {
                    Carrier::Message => {
                        ops.push(SubstrateOp::Send {
                            task: writer[&(n, inc, s.payload.epoch)],
                            channel: mailbox[&n],
                        });
                        m.push(none);
                    }
                    Carrier::Timer => {
                        ops.push(SubstrateOp::Sleep {
                            task: supervisor[&n].1,
                            nanos: CARRIER_TIMER.0,
                        });
                        m.push(none);
                    }
                    Carrier::Recovery => {}
                }
            }
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
                    let value = values[inc][usize::from(e)];
                    let (i, c) = channel[&(e, value)];
                    (
                        SubstrateOp::Send {
                            task: w(e),
                            channel: c,
                        },
                        Some(i),
                    )
                }
                Act::Crash | Act::CrashRepropose(..) => {
                    let r = region[&(n, inc)];
                    inc += 1;
                    permit.clear();
                    bytes.clear();
                    match mode {
                        CrashMode::Graceful => (SubstrateOp::Cancel { region: r }, None),
                        CrashMode::FailStop => (SubstrateOp::Crash { region: r }, None),
                    }
                }
            };
            ops.push(op);
            m.push(Meta { feeds, ..none });
            if let Some(s) = carried_here {
                // After the crash: the timer comes due and its callback sends; the
                // recovery report is sent; then the new incarnation receives, and acts on
                // the payload only when its epoch check accepts it.
                let e = s.payload.epoch;
                if s.payload.via == Carrier::Timer {
                    ops.push(SubstrateOp::Advance {
                        nanos: CARRIER_TIMER.1,
                    });
                    m.push(none);
                }
                if s.payload.via != Carrier::Message {
                    ops.push(SubstrateOp::Send {
                        task: supervisor[&n].1,
                        channel: mailbox[&n],
                    });
                    m.push(none);
                }
                ops.push(SubstrateOp::Recv {
                    channel: mailbox[&n],
                });
                m.push(none);
                if s.payload.via == Carrier::Recovery {
                    skip = Some(at + 1);
                }
                if s.accepted {
                    let (i, c) = channel[&(e, s.payload.value)];
                    ops.push(SubstrateOp::Send {
                        task: writer[&(n, inc, e)],
                        channel: c,
                    });
                    m.push(Meta {
                        feeds: Some(i),
                        ..none
                    });
                }
            }
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
            ..none
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

    if shutdown {
        let mut ops = Vec::new();
        let mut m = Vec::new();
        let last = Meta { last: true, ..none };
        for (i, &(e, v, writers)) in coords.iter().enumerate() {
            for _ in 0..writers.saturating_sub(2) {
                ops.push(SubstrateOp::Recv {
                    channel: channel[&(e, v)].1,
                });
                m.push(Meta {
                    commands: Some(i),
                    recv: true,
                    ..last
                });
            }
            ops.push(SubstrateOp::Finish {
                task: coord_task[i],
            });
            m.push(Meta {
                commands: Some(i),
                ..last
            });
        }
        let mut live = Vec::new();
        for n in 0..3_u8 {
            let inc = incarnations[usize::from(n)] - 1;
            for e in 0..plan.epochs {
                // Every writer exists unless the build is pruned.
                if let Some(&task) = writer.get(&(n, inc, e)) {
                    ops.push(SubstrateOp::Finish { task });
                    m.push(last);
                }
            }
            if let Some(&r) = region.get(&(n, inc)) {
                live.push(r);
            }
        }
        live.push(coord_region);
        for &(r, t) in supervisor.values() {
            ops.push(SubstrateOp::Finish { task: t });
            m.push(last);
            live.push(r);
        }
        live.push(RegionLabel::ROOT);
        for r in live {
            ops.push(SubstrateOp::Close { region: r });
            m.push(last);
        }
        programs.push(ops);
        meta.push(m);
    }
    Built {
        programs,
        roles: Roles {
            tasks,
            regions: region_ordinal,
            mailboxes,
        },
        labels: task_labels,
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

    /// Whether `actor`'s next operation commands a task that is not parked, and, for
    /// the shutdown, whether every other actor has finished. A confirmation waits while
    /// its coordinator's channel is full ([`CONFIRM_CAPACITY`]): the sender would park
    /// on it. No plan of the correct program sends a coordinator more than three
    /// confirmations, so this never binds there; it can bind only where two accepted
    /// payloads add two confirmations to one coordinator (bn-2faf1).
    fn admissible(&self, built: &Built, actor: usize) -> bool {
        let m = built.meta[actor][self.cursors[actor]];
        m.commands.is_none_or(|k| self.received[k] <= self.sent[k])
            && m.feeds.is_none_or(|k| {
                self.sent[k].saturating_sub(self.received[k]) < CONFIRM_CAPACITY as usize
            })
            && (!m.last
                || (0..built.programs.len())
                    .all(|a| a == actor || self.cursors[a] == built.programs[a].len()))
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

/// The admissible choice log of `built` that follows `rank` (PR 18's scenario reduction,
/// bn-25z9o): the setup first, then at each step the admissible actor whose next
/// operation `(actor, index)` has the lowest rank, the lower actor on a tie. A reduced
/// configuration's run keeps the order its operations had in the original run this way,
/// and every other operation joins where the program first admits it. `None` when every
/// actor with operations left is parked: the program deadlocks under this order.
pub fn guided_log(built: &Built, rank: impl Fn(usize, usize) -> u64) -> Option<ChoiceLog> {
    let (mut sched, mut prefix) = setup_prefix(built);
    loop {
        let enabled = sched.enabled(built);
        if enabled.is_empty() {
            return Some(ChoiceLog::new(prefix));
        }
        let index = (0..enabled.len())
            .filter(|&i| sched.admissible(built, enabled[i]))
            .min_by_key(|&i| (rank(enabled[i], sched.cursors[enabled[i]]), i))?;
        sched.take(built, enabled[index]);
        prefix.push(u32::try_from(index).expect("few actors"));
    }
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

/// The admissible choice log of `built` that follows a schedule preference (PR 21,
/// bn-4ykgg): setup first, then at each step the admissible actor whose next operation
/// has the least `rank(actor, operation index)`, an operation `rank` leaves unranked
/// after every ranked one, ties by actor index. With it, the operations taken after the
/// setup, as `(actor, operation index)`, in order. `Err` with the log taken so far when
/// every actor with operations left is parked: the program deadlocks under that
/// preference.
///
/// # Errors
///
/// The deadlocked prefix.
pub fn guided_run(
    built: &Built,
    rank: impl Fn(usize, usize) -> Option<usize>,
) -> Result<(ChoiceLog, Vec<(usize, usize)>), ChoiceLog> {
    let (mut sched, mut prefix) = setup_prefix(built);
    let mut taken = Vec::new();
    loop {
        let enabled = sched.enabled(built);
        if enabled.is_empty() {
            return Ok((ChoiceLog::new(prefix), taken));
        }
        let Some((index, actor)) = enabled
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, a)| sched.admissible(built, *a))
            .min_by_key(|(_, a)| (rank(*a, sched.cursors[*a]).unwrap_or(usize::MAX), *a))
        else {
            return Err(ChoiceLog::new(prefix));
        };
        taken.push((actor, sched.cursors[actor]));
        sched.take(built, actor);
        prefix.push(u32::try_from(index).expect("few actors"));
    }
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
    /// The writer task that makes it.
    task: u32,
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
    // Channel ordinal → the carried build's mailbox it is, by index (bn-2faf1).
    let mut mailboxes: BTreeMap<u32, usize> = BTreeMap::new();
    // Writer task ordinal → the `(epoch, value)` of each payload the journal delivered to
    // it: what corroborates a confirmation made from a payload.
    let mut delivered: BTreeMap<u32, BTreeSet<(u8, u8)>> = BTreeMap::new();
    let role_of = |task: u32| roles.tasks.get(task as usize).map(|r| r.0);
    for event in journal.events() {
        let seq = event.seq();
        let pre = view.raw();
        let unknown = ProjectionRefusal::UnknownEntity { seq };
        // A fail-stop crash (bn-20d8u): every attempt of a stopped writer that is still
        // reserved or volatile is lost at the crash itself, one `Lose` step each, by
        // attempt order; a durable one stays durable. The fences that follow stutter.
        if let EventBody::Lifecycle(LifecycleEvent::RegionCrashed { fenced, .. }) = event.body() {
            let stopped: BTreeSet<u32> = fenced.as_slice().iter().map(|t| t.0).collect();
            let mut lost_any = false;
            for i in 0..view.attempts.len() {
                let a = &view.attempts[i];
                if !stopped.contains(&a.task) {
                    continue;
                }
                let before = view.raw();
                let live = a.live();
                let a = &mut view.attempts[i];
                let expect = match live {
                    Some(Live::Volatile(v)) => Expect::Step(format!(
                        "Lose({},value={})",
                        at(a.node, a.epoch),
                        VALUES[usize::from(v)]
                    )),
                    Some(Live::Reserved) => {
                        Expect::StepPrefix(format!("Lose({},", at(a.node, a.epoch)))
                    }
                    Some(Live::Durable(_)) | None => {
                        if a.permit == Some(Phase::Open) {
                            a.permit = Some(Phase::Aborted);
                        }
                        continue;
                    }
                };
                if a.bytes == Some(Phase::Open) {
                    a.bytes = Some(Phase::Aborted);
                }
                if a.permit == Some(Phase::Open) {
                    a.permit = Some(Phase::Aborted);
                }
                lost_any = true;
                out.push(Observed {
                    seq,
                    event: event.render(),
                    expect,
                    pre: before,
                    post: view.raw(),
                });
            }
            if !lost_any {
                out.push(Observed {
                    seq,
                    event: event.render(),
                    expect: Expect::Stutter,
                    pre,
                    post: view.raw(),
                });
            }
            continue;
        }
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
                match role_of(receiver.0) {
                    Some(Role::Coordinator { epoch, value }) => {
                        channels.insert(channel.0, (epoch, value));
                    }
                    // A mailbox: the next one the roles name, with this receiver.
                    Some(Role::Writer { .. })
                        if roles.mailboxes.get(mailboxes.len()).is_some_and(|mb| {
                            mb.taker == receiver.0 && mb.channel == channel.0
                        }) =>
                    {
                        mailboxes.insert(channel.0, mailboxes.len());
                    }
                    _ => return Err(ProjectionRefusal::RolesDisagree { seq }),
                }
                Expect::Stutter
            }
            EventBody::Channel(ChannelEvent::Sent {
                channel, sender, ..
            }) => {
                if let Some(&k) = mailboxes.get(&channel.0) {
                    // A payload's send: by the crashed incarnation's writer of its node
                    // and slot (a message), or by its node's supervisor.
                    let p = roles.mailboxes[k].payload;
                    if sender.0 != roles.mailboxes[k].sender {
                        return Err(ProjectionRefusal::RolesDisagree { seq });
                    }
                    match (p.via, role_of(sender.0)) {
                        (Carrier::Message, Some(Role::Writer { node, epoch, .. }))
                            if (node, epoch) == (p.node, p.epoch) => {}
                        (Carrier::Timer | Carrier::Recovery, Some(Role::Supervisor { node }))
                            if node == p.node => {}
                        _ => return Err(ProjectionRefusal::RolesDisagree { seq }),
                    }
                } else {
                    let to = *channels.get(&channel.0).ok_or_else(|| unknown.clone())?;
                    match role_of(sender.0) {
                        Some(Role::Writer { epoch, value, .. }) if (epoch, value) == to => {}
                        // Corroborated by a payload the journal delivered to the sender,
                        // used once.
                        Some(Role::Writer { epoch, .. })
                            if epoch == to.0
                                && delivered.get_mut(&sender.0).is_some_and(|d| d.remove(&to)) => {}
                        _ => return Err(ProjectionRefusal::RolesDisagree { seq }),
                    }
                }
                Expect::Stutter
            }
            EventBody::Channel(ChannelEvent::Received { channel, .. }) => {
                if let Some(&k) = mailboxes.get(&channel.0) {
                    let mb = roles.mailboxes[k];
                    delivered
                        .entry(mb.taker)
                        .or_default()
                        .insert((mb.payload.epoch, mb.payload.value));
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
                            task: task.0,
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
                    Role::Supervisor { .. } => {
                        return Err(ProjectionRefusal::RolesDisagree { seq });
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
                            task: holder.0,
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
    if spawned != roles.tasks.len()
        || regions != roles.regions
        || mailboxes.len() != roles.mailboxes.len()
    {
        return Err(ProjectionRefusal::RolesUnspawned);
    }
    Ok(out)
}

/// What a carried build's journal shows of its payloads (bn-2faf1): for each mailbox, in
/// order, `None` when its message was never received, or `Some(acted)`, where `acted` says
/// whether the receiver's first own step after the receipt (a reserve, an obligation it
/// opens other than a send's permit, or a send) is a send to the coordinator of the payload's slot and value. That is
/// the confirmation made from the payload; a receiver that drops the payload goes on to its
/// own write instead. Read from the journal alone, with the mailboxes of `roles`.
#[must_use]
pub fn carried_in_journal(roles: &Roles, journal: &Journal) -> Vec<Option<bool>> {
    let mut coordinator_of: BTreeMap<u32, (u8, u8)> = BTreeMap::new();
    let events = journal.events();
    for e in events {
        if let EventBody::Channel(ChannelEvent::Opened {
            channel, receiver, ..
        }) = e.body()
            && let Some((Role::Coordinator { epoch, value }, _)) =
                roles.tasks.get(receiver.0 as usize)
        {
            coordinator_of.insert(channel.0, (*epoch, *value));
        }
    }
    roles
        .mailboxes
        .iter()
        .map(|mb| {
            let at = events.iter().position(|e| {
                matches!(e.body(), EventBody::Channel(ChannelEvent::Received { channel, .. }) if channel.0 == mb.channel)
            })?;
            let next = events[at + 1..].iter().find_map(|e| match e.body() {
                EventBody::Effect(EffectEvent::Reserved { task, .. }) if task.0 == mb.taker => {
                    Some(None)
                }
                // A send opens its send permit first: that is the send, not a step
                // of its own.
                EventBody::Obligation(ObligationEvent::Opened { holder, kind, .. })
                    if holder.0 == mb.taker && *kind != ObligationKind::SendPermit =>
                {
                    Some(None)
                }
                EventBody::Channel(ChannelEvent::Sent {
                    channel, sender, ..
                }) if sender.0 == mb.taker => Some(coordinator_of.get(&channel.0).copied()),
                _ => None,
            });
            Some(next.flatten() == Some((mb.payload.epoch, mb.payload.value)))
        })
        .collect()
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

/// A projected state's sets, by position, as [`raw_of`] takes them: `log`, `pending`,
/// `acks`.
#[allow(clippy::type_complexity)]
pub fn parts_of(raw: &Raw) -> (Vec<(u8, u8, u8)>, Vec<(u8, u8)>, Vec<(u8, u8)>) {
    (
        raw.log.iter().copied().collect(),
        raw.pending.iter().copied().collect(),
        raw.acks.iter().copied().collect(),
    )
}

/// A projected state's durable records, `(node, epoch, value)`: the log entries whose
/// slot has no write in flight.
pub fn durable_of(raw: &Raw) -> BTreeSet<(u8, u8, u8)> {
    raw.durable()
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
