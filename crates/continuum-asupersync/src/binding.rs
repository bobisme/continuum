//! The substrate binding: asupersync's lab runtime, driven by a Continuum [`ChoiceLog`],
//! observed into the journal events of the families it binds.
//!
//! # What is bound, and what is not
//!
//! The binding observes all six families: [`Family::Lifecycle`] (PR-14-IMPL-01,
//! bn-lf4i), [`Family::Effect`] (PR-14-IMPL-02, bn-gzy1), [`Family::Cancellation`]
//! (PR-14-IMPL-03, bn-bx7i), [`Family::Obligation`] (PR-14-IMPL-04, bn-6nm8),
//! [`Family::Time`] (PR-14-IMPL-05, bn-3m1d) and [`Family::Channel`] (PR-14-IMPL-06,
//! bn-3xx9). A run observes the families
//! [`BindingConfig::families`] names. Lifecycle is always among them, because the
//! other families name tasks by the ordinals its spawn events allocate. The default is
//! lifecycle alone, whose journal is the one bn-lf4i pinned, byte for byte.
//!
//! A family a later PR adds starts unbound: [`substrate_binding`] answers it with the
//! typed absence [`BindingAbsence::FamilyNotBound`], whose INV-008 reading is
//! [`InconclusiveReason::Unsupported`], and a run that asks for it is refused with
//! [`BindingRefusal::FamilyNotBound`]. A caller never gets an empty journal that looks
//! like a run in which nothing happened.
//!
//! # Who decides the schedule
//!
//! A run is a set of actor [`Program`]s and a [`ChoiceLog`], exactly as for the scripted
//! source ([`crate::source::record`]): at each step the enabled actors are those with
//! operations left, in ascending actor index, and a choice indexes into that list. Each
//! chosen [`SubstrateOp`] is one call into the substrate's public API, followed by
//! `LabRuntime::run_until_idle`. Every operation makes at most one task runnable, except
//! a cancellation, whose drain set is recorded as a set. So the substrate's own seeded
//! scheduler has no choice left to make that reaches the journal: the choice log is the
//! scheduling authority (INV-005), and the lab seed is an explicit, journal-invisible
//! parameter ([`BindingConfig::lab_seed`]). The tests hold that as a metamorphic
//! relation over several seeds.
//!
//! # Where events come from
//!
//! Events are observations of the substrate, not restatements of the program. The
//! binding reads the lab runtime's own trace buffer after every operation, in the
//! buffer's sequence order, and maps each trace event it recognizes to one lifecycle
//! event:
//!
//! | substrate trace event | journal event |
//! |---|---|
//! | `RegionCreated` with a parent | [`LifecycleEvent::RegionOpened`] |
//! | `RegionCreated` of the root | none: `r0` is implicit |
//! | `Spawn` | [`LifecycleEvent::TaskSpawned`] (resumability as the program declared it: the substrate has no such notion) |
//! | gate trace `begin` / `suspend` / `resume` | [`LifecycleEvent::TaskStepped`] |
//! | `Complete`, join result `Ok` | [`LifecycleEvent::TaskStepped`] `complete` |
//! | `Complete`, join result `Cancelled` | the task joins the next drain set of an enclosing region |
//! | `RegionCancelled`, first in its subtree | [`LifecycleEvent::RegionCancelRequested`] |
//! | `RegionCloseComplete` | [`LifecycleEvent::RegionDrained`] then [`LifecycleEvent::RegionFinalized`], in canonical order within a batch (below) |
//!
//! [`LifecycleEvent::RegionCloseRequested`] is the one exception. The substrate traces no
//! event for a normal close request, so the binding records it when `begin_close` on the
//! region record returns `true`. That return value is the substrate's acknowledgement.
//!
//! The *gate* is the instrumented primitive for task suspension. Every bound task's body
//! is a loop over one `GateWait` future. The substrate polls it. The gate writes a
//! `begin`, `suspend` or `resume` mark into the substrate's trace through `Cx::trace`
//! when, and only when, the poll changes the task's lifecycle phase. So those marks are
//! ordered with the substrate's own events in one sequence. The gate also observes
//! cancellation through `Cx::checkpoint`, as asupersync's cooperative cancellation
//! requires.
//!
//! `RegionCloseBegin` is recognized and has no event of its own: it is the start of a
//! close that the request event already reports. A trace event of a kind the binding
//! does not map is a typed refusal ([`BindingRefusal::UninstrumentedEvent`]), never a
//! dropped event.
//!
//! # Reserve / commit / abort
//!
//! A bound task's effects are asupersync's own two-phase primitive: a checked
//! obligation token of kind `Transaction`, reserved through the task's `Cx`
//! (`Cx::try_register_obligation_checked`) and committed or aborted through the token.
//! [`SubstrateOp::Reserve`], [`SubstrateOp::Commit`] and [`SubstrateOp::Abort`] are
//! gate commands; the task itself makes the calls. When [`Family::Effect`] is
//! observed:
//!
//! | substrate trace event | journal event |
//! |---|---|
//! | `ObligationReserve` | [`EffectEvent::Reserved`] |
//! | `ObligationCommit` | [`EffectEvent::Committed`] |
//! | `ObligationAbort`, reason `Explicit` | [`EffectEvent::Aborted`] `explicit` |
//! | `ObligationAbort`, reason `Cancel` | [`EffectEvent::Aborted`] `cancel`, placed canonically (below) |
//! | `ObligationAbort`, reason `Error`, or `ObligationLeak`, of a reservation of this run | [`BindingRefusal::ReservationDropped`] |
//!
//! A task that holds a reservation is mid-effect, not parked, so its pending polls
//! write no `suspend` mark until it resolves everything it holds. The runtime applies a
//! resolution after the poll that posts it, so the gate yields once, unmarked, before
//! it may write `suspend`: the mark then follows the resolution in the trace, as the
//! calculus requires (a worker may not park mid-publication). A task that observes its
//! cancellation aborts every reservation it holds, for `Cancel`, before it returns.
//! After each effect operation the binding confirms from the trace that the substrate
//! took the step it asked for ([`BindingRefusal::EffectUnobserved`] otherwise).
//!
//! # Channels
//!
//! [`SubstrateOp::OpenChannel`] opens a bounded `channel::mpsc` channel and hands its
//! receiver to one task; the binding keeps one sender. [`SubstrateOp::Send`] makes a
//! task send one new message through a clone of it (`Sender::reserve_checked`, then
//! `SendPermit::send`); [`SubstrateOp::Recv`] makes the receiving task receive once
//! (`Receiver::recv`); [`SubstrateOp::CloseSenders`] drops the kept sender. A message is
//! a dense ordinal the binding allocates when the send is commanded, and it is the
//! payload itself, so a received message is named by what the substrate delivered.
//! When [`Family::Channel`] is observed:
//!
//! | substrate observation | journal event |
//! |---|---|
//! | the binding's `mpsc::channel` call | [`ChannelEvent::Opened`] |
//! | the send's `SendPermit` obligation commits (`ObligationCommit` trace event) | [`ChannelEvent::Sent`] |
//! | channel gate: the reserve returned `Pending` / the channel's closed error | [`ChannelEvent::SendBlocked`] / [`ChannelEvent::SendClosed`] |
//! | channel gate: `recv` returned a value / `Pending` / `Disconnected` | [`ChannelEvent::Received`] / [`ChannelEvent::RecvBlocked`] / [`ChannelEvent::RecvClosed`] |
//! | channel gate: a cancellation dropped a blocked send or receive | [`ChannelEvent::SendAbandoned`] / [`ChannelEvent::RecvAbandoned`], placed canonically after the task's acknowledgement |
//! | the binding dropped its kept sender | [`ChannelEvent::SendersClosed`] |
//! | channel gate: the receiving task ended, with what was still queued | [`ChannelEvent::ReceiverGone`] |
//!
//! asupersync 0.5.0's channels write nothing into the trace themselves, so the channel
//! gate writes its marks through `Cx::trace` when the substrate's call returns, in
//! sequence with the substrate's own events. The binding also keeps its own account of
//! each queue from the trace, and a delivered message or a dropped queue that disagrees
//! with it is [`BindingRefusal::SubstrateChannelDisagrees`]. A task that waits on a
//! channel is parked; an operation that commands it is [`BindingRefusal::TaskBlocked`].
//!
//! # Virtual time
//!
//! Time reaches a bound run only as the lab's virtual clock (INV-005).
//! [`SubstrateOp::Sleep`] makes a task sleep on its `Cx`'s virtual timer driver
//! (`time::sleep_until`; a `Cx` without that driver is refused, because a `Sleep` would
//! otherwise fall back to a wall-clock thread). [`SubstrateOp::Advance`] moves the clock
//! with `LabRuntime::advance_time`, lets every due timer fire, and runs the woken tasks.
//! When [`Family::Time`] is observed:
//!
//! | substrate observation | journal event |
//! |---|---|
//! | `TimerScheduled` trace event (written by the `Sleep` future) | [`TimeEvent::Scheduled`] |
//! | `LabRuntime::now` before and after `advance_time` | [`TimeEvent::Advanced`] |
//! | `TimerFired` trace event | [`TimeEvent::Fired`] |
//! | `TimerCancelled` trace event (the sleeping task's cancellation dropped its timer) | [`TimeEvent::Cancelled`], placed canonically after the task's acknowledgement |
//!
//! A sleeping task is parked: `suspend` when its timer is armed, `resume` when it fires.
//! An operation that commands a sleeping task is [`BindingRefusal::TaskAsleep`].
//!
//! # The obligation ledger
//!
//! [`SubstrateOp::Acquire`] reserves an obligation of any kind the substrate has, and
//! [`SubstrateOp::Transfer`] hands one to another task through the substrate's own
//! `ObligationToken::try_transfer`. When [`Family::Obligation`] is observed, every
//! obligation's ledger steps are journaled, whatever the kind:
//!
//! | substrate observation | journal event |
//! |---|---|
//! | `ObligationReserve` | [`ObligationEvent::Opened`], with kind, holder and region |
//! | `ObligationCommit` / `ObligationAbort` | [`ObligationEvent::Discharged`]; a cancellation's aborts are placed canonically, after the task's effect aborts |
//! | the runtime's `obligation_handoff_v1` trace message | [`ObligationEvent::Transferred`], holder and region read from the substrate's obligation record |
//! | `ObligationLeak` | [`ObligationEvent::Leaked`]; one completion's leaks by ascending obligation ordinal |
//! | `RegionCloseComplete` | [`ObligationEvent::RegionSettled`] after the region's finalize, with the region's open and leaked obligations |
//!
//! A leak is never dropped: without this family observed, it is a typed refusal. At
//! the end of a run the binding holds the ledger it journaled to the substrate's own:
//! every obligation record the runtime keeps, and its obligation-leak oracle rebuilt
//! from the runtime state (`ObligationLeakOracle::snapshot_from_state`). A disagreement
//! is [`BindingRefusal::SubstrateLedgerDisagrees`].
//!
//! # The cancellation phases
//!
//! When [`Family::Cancellation`] is observed, the task-level phases of a cancellation
//! become events of their own ([`CancellationEvent`]). When it is not, they are
//! summarized by the lifecycle drain's cancelled set, as before.
//!
//! | substrate observation | journal event |
//! |---|---|
//! | `CancelRequest` trace event, with its reason's kind | [`CancellationEvent::Requested`] |
//! | gate trace `ack`: `Cx::checkpoint` returned the cancellation error | [`CancellationEvent::Acknowledged`] |
//! | `Complete`, join result `Cancelled`, with its reason's kind | [`CancellationEvent::Cancelled`] |
//!
//! # Budget deadlines (bn-36wy3)
//!
//! [`SubstrateOp::SpawnWithDeadline`] creates a task with a finite `Budget` deadline
//! on the lab's virtual clock. Once the clock reaches it, the task's next
//! `Cx::checkpoint` raises `CancelKind::Deadline`: the task observes its own
//! cancellation, cleans up, and completes as cancelled, and its region stays as it was.
//! asupersync 0.5.0 traces no `CancelRequest` for it, so the checkpoint is both the
//! request and the acknowledgement, and the binding journals the whole single-task
//! cancellation at the task's completion:
//!
//! | substrate observation | journal event |
//! |---|---|
//! | the `Budget` the binding created the task with | [`TimeEvent::Deadline`], after the task's spawn |
//! | `Complete`, join result `Cancelled` with reason kind `Deadline` | lifecycle `cancel-requested`; [`CancellationEvent::Requested`] `deadline`; [`CancellationEvent::Acknowledged`]; the cleanup (timer dropped, cancel aborts, aborted discharges); [`CancellationEvent::Cancelled`] `deadline`; lifecycle `cancel` |
//!
//! Without the cancellation family the lifecycle steps and the cleanup remain. Each
//! event goes where the task's other events go: an advance's per-task batch (a task a
//! due timer woke, whose deadline had also passed, drops the timer instead of firing it,
//! and is still ordered by that timer's deadline), the woken tasks' batch, or the
//! journal. A task's own region cancelled after its deadline passed but before its next
//! poll completes with the more severe `deadline` reason under a region's request:
//! [`BindingRefusal::CancelRaced`], not a journal. A parent region's cancellation
//! outranks the deadline and is an ordinary region cancellation.
//!
//! asupersync 0.5.0 declares a `CancelAck` trace kind but never pushes it. Its
//! acknowledgement is the checkpoint: a checkpoint that returns the cancellation error
//! is what moves the task from `CancelRequested` to `Cancelling`. So the gate writes an
//! `ack` mark into the substrate's trace at that return, in sequence with the
//! substrate's own events, exactly as it writes `begin`, `suspend` and `resume`.
//! `Cancelling → Finalizing → Completed(Cancelled)` then happens inside the poll that
//! returns the task's result, and the `Complete` event observes it. After each
//! operation the binding asks the substrate's own cancellation-protocol oracle
//! (`LabRuntime::check_cancellation_protocol`) and confirms each cancelled completion
//! against the oracle's task state. A disagreement is
//! [`BindingRefusal::SubstrateProtocolViolation`], not an event.
//!
//! # Canonical order
//!
//! The choice log fixes which operation runs, but inside one operation the substrate's
//! seeded scheduler picks the order in which concurrently cancelled tasks are polled,
//! and so the order in which sibling regions finish closing. That order is not
//! controlled, so it must not reach the journal (INV-005). The binding therefore
//! journals a canonical linearization of the substrate's partial order:
//!
//! - an advance's woken tasks are journaled one task at a time, by the deadline of the
//!   timer that woke each, then its ordinal: each task's fire and steps in their own
//!   order, the tasks canonically. A tie at one virtual instant is the common case, and
//!   the substrate's scheduler polls tied tasks in an order its seed picks. Whatever
//!   else the substrate traces during an advance has no canonical place, and is
//!   [`BindingRefusal::UnorderedDuringAdvance`]. [`run_witnessed`] returns the
//!   substrate's raw fire order as evidence;
//! - the tasks an operation wakes (a receiver a send fed, a sender a receive freed, the
//!   senders a gone receiver releases) are journaled after the operation, one task at a
//!   time, by ascending task ordinal. Several tasks woken together run in an order the
//!   substrate's scheduler picks, and are independent only while none of them sends,
//!   receives or moves an obligation: several woken tasks that do are
//!   [`BindingRefusal::UnorderedWake`]. [`run_witnessed`] returns the raw wake order;
//! - one completion's leaks are journaled together, by obligation ordinal. The
//!   substrate traces them in the order the task drops its tokens, which follows the
//!   program's labels, and labels must not reach the journal;
//! - consecutive `RegionCloseComplete` events (with the cancelled completions and
//!   acknowledgements between them, which journal nothing by themselves) form one
//!   batch, ended by the next event that journals anything or by the operation's end.
//!   A batch's drains and finalizes are journaled in post-order over the region tree,
//!   siblings by ascending region ordinal. The substrate only orders a child's close
//!   before its parent's, so this is one of its admitted orders, and the lift still
//!   sees every child drained before its parent. [`run_witnessed`] returns the
//!   substrate's raw close order beside the journal, as evidence that the two differ;
//! - the requests of one operation, which the substrate issues before any poll, are
//!   journaled together, by ascending task ordinal, right after the region cancel
//!   request;
//! - each task's acknowledgement and completion are journaled immediately before the
//!   lifecycle drain that absorbs the task, by ascending task ordinal, with the
//!   reservations its cancellation aborted between them, by ascending reservation
//!   ordinal (or, without the cancellation family, alone before the drain).
//!
//! Per task, `requested → acknowledged → cancelled` keeps its substrate order, and all
//! three precede the drain, as they do in the trace. Only the relative order of
//! independent tasks and independent sibling regions inside one operation is fixed
//! canonically.
//!
//! Refusals name one instance. [`BindingRefusal::UndrainedCancellation`] names the
//! smallest task ordinal. When one operation produces several distinct faults at once
//! (for example two cancelled completions the substrate's oracle does not confirm),
//! the refusal reports the first in trace order, which the seed can move. No journal
//! is returned in that case, so no journal content depends on it.
//!
//! # No ambient output
//!
//! The binding builds its `LabRuntime` from an explicit `LabConfig` and never calls the
//! lab-test harness (`run_async_lab_test_with_config`, `run_async_under_lab*`) or the
//! `write_auto_crashpack*` methods. Those are the only paths in asupersync 0.5.0 that
//! write crashpacks to `./target/test-artifacts` (`src/lab/runtime.rs:5667-5686`). A test
//! pins the absence of those names from this file. The workspace's `.cargo/config.toml`
//! also sets `ASUPERSYNC_AUTO_ARTIFACTS=0` for every process cargo runs, as a second
//! barrier. The oracles' panic switches are turned off, so a substrate finding is a
//! typed refusal and not an abort.
//!
//! # Refusals
//!
//! A run that cannot be observed completely is a typed [`BindingRefusal`] and no
//! journal (INV-008). [`BindingRefusal::inconclusive_reason`] gives the INV-008 reading
//! of each refusal that has one.
//!
//! A command reaches a task only through its gate, and the gate is polled only while the
//! task runs. So a command to a task that has not begun ([`BindingRefusal::TaskNotBegun`]),
//! has ended ([`BindingRefusal::TaskEnded`]) or sleeps on a timer
//! ([`BindingRefusal::TaskAsleep`]) is refused before it is sent: it would otherwise wait
//! unseen, and the journal would lose the operation.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::task::{Context, Poll, Waker};

use asupersync::channel::mpsc::{
    self, CheckedSendError, Receiver as MpscReceiver, RecvError, Sender as MpscSender,
};
use asupersync::lab::oracle::ObligationLeakOracle;
use asupersync::lab::oracle::TaskStateKind;
use asupersync::record::{
    ObligationAbortReason, ObligationKind as SubstrateObligationKind, ObligationState,
};
use asupersync::runtime::obligation_mailbox::ObligationToken;
use asupersync::runtime::{JoinError, TaskHandle};
use asupersync::time::{Sleep, sleep_until};
use asupersync::trace::event::{TraceData, TraceEvent, TraceEventKind};
use asupersync::{
    Budget, CancelKind, CancelReason, Cx, LabConfig, LabRuntime, ObligationId,
    RegionId as SubstrateRegion, TaskId as SubstrateTask, Time,
};
use continuum_task::region::worker::Resumability;
use continuum_value::assurance::InconclusiveReason;

use crate::choice::ChoiceLog;
use crate::family::cancellation::{CancelCause, CancellationEvent};
use crate::family::channel::{
    ChannelEvent, ChannelLabel, ChannelOrdinal, MessageOrdinal, MessageSet,
};
use crate::family::effect::{AbortCause, EffectEvent, ReservationLabel, ReservationOrdinal};
use crate::family::lifecycle::{
    LifecycleEvent, RegionLabel, RegionOrdinal, TaskLabel, TaskOrdinal, TaskSet, TaskStep,
};
use crate::family::obligation::{
    Discharge, ObligationEvent, ObligationKind, ObligationOrdinal, ObligationSet,
};
use crate::family::time::{TimeEvent, TimerOrdinal, VirtualInstant};
use crate::family::{EventBody, Family};
use crate::journal::Journal;
use crate::source::RecordContext;

// --- which families are bound ------------------------------------------------------

/// Why the substrate binding does not drive a family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingAbsence {
    /// The family's instrumentation is not bound to the substrate yet.
    FamilyNotBound(Family),
}

impl BindingAbsence {
    /// A stable token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::FamilyNotBound(_) => "family-not-bound",
        }
    }
}

impl fmt::Display for BindingAbsence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FamilyNotBound(family) => write!(f, "{} ({family})", self.token()),
        }
    }
}

/// Whether events of a family can come from the real substrate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubstrateBinding {
    /// They can: [`run`] drives the substrate and observes this family's events.
    Bound,
    /// They cannot, for this reason.
    Absent(BindingAbsence),
}

impl SubstrateBinding {
    /// The INV-008 reason a request for this family's substrate events is
    /// inconclusive, or `None` when the family is bound.
    #[must_use]
    pub const fn inconclusive_reason(self) -> Option<InconclusiveReason> {
        match self {
            Self::Bound => None,
            Self::Absent(_) => Some(InconclusiveReason::Unsupported),
        }
    }
}

/// The binding this build has for `family`.
#[must_use]
pub const fn substrate_binding(family: Family) -> SubstrateBinding {
    match family {
        Family::Lifecycle
        | Family::Effect
        | Family::Cancellation
        | Family::Obligation
        | Family::Time
        | Family::Channel => SubstrateBinding::Bound,
    }
}

// --- programs ----------------------------------------------------------------------

/// One call into the substrate, by the program's labels.
///
/// Labels are program-local names, bound by [`SubstrateOp::OpenRegion`] and
/// [`SubstrateOp::Spawn`]. They never reach the journal. [`RegionLabel::ROOT`] is bound
/// to the substrate's root region before the first operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubstrateOp {
    /// `RuntimeState::create_child_region(parent)`, bound to `child`.
    OpenRegion {
        /// The parent.
        parent: RegionLabel,
        /// The label the new region is bound to.
        child: RegionLabel,
    },
    /// `RuntimeState::create_task(region, gated body)`, bound to `task`. The task is
    /// not scheduled until [`SubstrateOp::Begin`].
    Spawn {
        /// The owning region.
        region: RegionLabel,
        /// The label the new task is bound to.
        task: TaskLabel,
        /// Declared resumability. The substrate has no such notion, so the journal
        /// carries the declaration.
        resumability: Resumability,
    },
    /// As [`SubstrateOp::Spawn`], with a finite `Budget` deadline at the virtual instant
    /// `deadline` (`Budget::INFINITE.with_deadline`). Once the lab clock reaches it, the
    /// task's next `Cx::checkpoint` observes its cancellation (`CancelKind::Deadline`),
    /// and the task cleans up and completes as cancelled on its own, while its region
    /// stays as it was (bn-36wy3). A deadline not ahead of the clock is refused, and so
    /// is a task whose `Cx` carries no virtual timer driver, because its checkpoint would
    /// then read the host clock (INV-005).
    SpawnWithDeadline {
        /// The owning region.
        region: RegionLabel,
        /// The label the new task is bound to.
        task: TaskLabel,
        /// Declared resumability.
        resumability: Resumability,
        /// The deadline, in virtual nanoseconds from the clock's start.
        deadline: u64,
    },
    /// Schedule `task` for its first poll. Its body parks on its gate.
    Begin {
        /// The task.
        task: TaskLabel,
    },
    /// Wake `task` with a command to park again.
    Continue {
        /// The task.
        task: TaskLabel,
    },
    /// Wake `task` with a command to return.
    Finish {
        /// The task.
        task: TaskLabel,
    },
    /// `RegionRecord::begin_close(None)`, then `RuntimeState::advance_region_state`.
    Close {
        /// The region.
        region: RegionLabel,
    },
    /// `RuntimeState::cancel_request`, with its effects dispatched as the lab runtime's
    /// own helpers dispatch them.
    Cancel {
        /// The region.
        region: RegionLabel,
    },
    /// Wake `task` with a command to reserve a two-phase effect through its own `Cx`
    /// (`Cx::try_register_obligation_checked`), bound to `reservation`. The task then
    /// waits, mid-effect, for the reservation's resolution.
    Reserve {
        /// The task.
        task: TaskLabel,
        /// The label the new reservation is bound to.
        reservation: ReservationLabel,
    },
    /// Wake the holding task with a command to commit `reservation`.
    Commit {
        /// The reservation.
        reservation: ReservationLabel,
    },
    /// Wake the holding task with a command to abort `reservation` on purpose.
    Abort {
        /// The reservation.
        reservation: ReservationLabel,
    },
    /// As [`SubstrateOp::Reserve`], for an obligation of any `kind`. Only a
    /// `Transaction` is a reservation of the reserve/commit/abort family; every kind is
    /// the obligations family's. [`SubstrateOp::Commit`] and [`SubstrateOp::Abort`]
    /// resolve it by label.
    Acquire {
        /// The task.
        task: TaskLabel,
        /// The label the new obligation is bound to.
        reservation: ReservationLabel,
        /// Its kind.
        kind: ObligationKind,
    },
    /// Wake `task` with a command to sleep `nanos` nanoseconds on its `Cx`'s virtual
    /// timer driver (`time::sleep_until`). The task parks until an
    /// [`SubstrateOp::Advance`] moves the clock past the deadline, or until its
    /// cancellation drops the timer.
    Sleep {
        /// The task.
        task: TaskLabel,
        /// How long, in virtual nanoseconds. Zero is refused.
        nanos: u64,
    },
    /// `mpsc::channel(capacity)`: open a bounded channel, bound to `channel`, and hand its
    /// receiver to `receiver`. The binding keeps one sender.
    OpenChannel {
        /// The label the channel is bound to.
        channel: ChannelLabel,
        /// Its capacity. Zero is refused.
        capacity: u32,
        /// The task that holds the receiver.
        receiver: TaskLabel,
    },
    /// Wake `task` with a command to send one new message on `channel` through a clone
    /// of the binding's sender (`Sender::reserve_checked`, then `SendPermit::send`). A
    /// full channel makes it wait.
    Send {
        /// The sending task.
        task: TaskLabel,
        /// The channel.
        channel: ChannelLabel,
    },
    /// Wake the receiving task with a command to receive once on `channel`
    /// (`Receiver::recv`). An empty channel makes it wait.
    Recv {
        /// The channel.
        channel: ChannelLabel,
    },
    /// Drop the sender the binding keeps for `channel`.
    CloseSenders {
        /// The channel.
        channel: ChannelLabel,
    },
    /// `LabRuntime::advance_time`: move the virtual clock forward `nanos` nanoseconds,
    /// then let every timer that came due fire and its task run.
    Advance {
        /// How far, in virtual nanoseconds. Zero is refused.
        nanos: u64,
    },
    /// Wake the holding task with a command to hand `reservation` to `to`
    /// (`ObligationToken::try_transfer`, with `to`'s own `Cx`). A `Transaction` cannot
    /// be transferred: the region calculus has one staged publication per worker.
    Transfer {
        /// The obligation.
        reservation: ReservationLabel,
        /// The new holder.
        to: TaskLabel,
    },
}

impl SubstrateOp {
    const fn name(&self) -> &'static str {
        match self {
            Self::OpenRegion { .. } => "open-region",
            Self::Spawn { .. } => "spawn",
            Self::SpawnWithDeadline { .. } => "spawn-with-deadline",
            Self::Begin { .. } => "begin",
            Self::Continue { .. } => "continue",
            Self::Finish { .. } => "finish",
            Self::Close { .. } => "close",
            Self::Cancel { .. } => "cancel",
            Self::Reserve { .. } => "reserve",
            Self::Commit { .. } => "commit",
            Self::Abort { .. } => "abort",
            Self::Acquire { .. } => "acquire",
            Self::Transfer { .. } => "transfer",
            Self::Sleep { .. } => "sleep",
            Self::Advance { .. } => "advance",
            Self::OpenChannel { .. } => "open-channel",
            Self::Send { .. } => "send",
            Self::Recv { .. } => "recv",
            Self::CloseSenders { .. } => "close-senders",
        }
    }
}

/// One actor's operations, in program order.
pub type Program = Vec<SubstrateOp>;

/// A set of families, as the families a run observes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Families(u8);

impl Families {
    /// No family.
    pub const NONE: Self = Self(0);
    /// The lifecycle family alone: the default, and bn-lf4i's journal.
    pub const LIFECYCLE: Self = Self::NONE.with(Family::Lifecycle);

    const fn bit(family: Family) -> u8 {
        1 << family.tag()
    }

    /// This set with `family` added.
    #[must_use]
    pub const fn with(self, family: Family) -> Self {
        Self(self.0 | Self::bit(family))
    }

    /// Whether `family` is in the set.
    #[must_use]
    pub const fn contains(self, family: Family) -> bool {
        self.0 & Self::bit(family) != 0
    }
}

/// The explicit parameters of a run. Nothing else configures the substrate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BindingConfig {
    /// The lab runtime's seed. The journal must not depend on it.
    pub lab_seed: u64,
    /// The substrate trace buffer's capacity. A run that overflows it is refused.
    pub trace_capacity: usize,
    /// The lab runtime's step limit. A run that reaches it is refused.
    pub max_steps: u64,
    /// The families the run observes. Lifecycle must be one of them.
    pub families: Families,
}

impl BindingConfig {
    /// A configuration with this lab seed and the default bounds.
    #[must_use]
    pub const fn new(lab_seed: u64) -> Self {
        Self {
            lab_seed,
            trace_capacity: 1 << 16,
            max_steps: 1 << 20,
            families: Families::LIFECYCLE,
        }
    }

    /// This configuration, observing `family` too.
    #[must_use]
    pub const fn observing(mut self, family: Family) -> Self {
        self.families = self.families.with(family);
        self
    }
}

// --- refusals ----------------------------------------------------------------------

/// Why a run could not be observed into a journal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingRefusal {
    /// An operation names a region label no earlier operation bound.
    UnboundRegion(u32),
    /// An operation names a task label no earlier operation bound.
    UnboundTask(u32),
    /// An operation binds a region label that is already bound.
    RegionLabelRebound(u32),
    /// An operation binds a task label that is already bound.
    TaskLabelRebound(u32),
    /// A choice indexes past the enabled actors.
    ChoiceOutOfRange {
        /// Position in the log.
        position: usize,
        /// The choice.
        choice: u32,
        /// How many actors were enabled.
        enabled: usize,
    },
    /// The log ended while operations remained.
    ChoiceLogExhausted {
        /// Operations not yet applied.
        remaining: usize,
    },
    /// Choices remained after every program finished.
    ChoiceLogOverrun {
        /// Position of the first unused choice.
        position: usize,
    },
    /// The journal could not be extended.
    JournalFull,
    /// The substrate refused an operation.
    SubstrateRefused {
        /// The operation.
        operation: &'static str,
        /// The substrate's own description.
        detail: String,
    },
    /// The lab runtime reached its step limit with work still runnable.
    StepLimit,
    /// The substrate trace buffer overflowed, so events were lost.
    TraceOverflow {
        /// Events pushed.
        pushed: u64,
        /// Buffer capacity.
        capacity: usize,
    },
    /// A trace event arrived after a later-sequenced one was already observed.
    TraceOutOfOrder {
        /// Its sequence number.
        seq: u64,
    },
    /// A trace event names a region or task this run did not create.
    UnknownSubstrateEntity {
        /// What the event named.
        what: &'static str,
    },
    /// A gate could not write its mark, because no `Cx` was current at the poll.
    GateUntraced,
    /// A completed task's outcome was not published.
    TaskOutcomeUnobserved {
        /// The task's journal ordinal.
        task: u32,
    },
    /// A task panicked. Task failure is not bound yet.
    TaskPanicked {
        /// The task's journal ordinal.
        task: u32,
    },
    /// A task was cancelled but no enclosing region's close completed.
    UndrainedCancellation {
        /// The task's journal ordinal.
        task: u32,
    },
    /// The substrate traced an event of a kind the binding does not map.
    UninstrumentedEvent {
        /// The family the kind belongs to, when it belongs to one.
        family: Option<Family>,
        /// The substrate's name for the kind.
        kind: String,
    },
    /// The configuration asks to observe a family the binding does not bind.
    FamilyNotBound(Family),
    /// The configuration does not observe the lifecycle family, which every other
    /// family's task ordinals come from.
    LifecycleNotObserved,
    /// A cancellation carries a reason kind the cancellation family has no cause for.
    UnmappedCancelReason {
        /// The substrate's name for the kind.
        kind: String,
    },
    /// The substrate's own cancellation-protocol oracle found a violation, or does not
    /// confirm a cancelled completion the trace reports.
    SubstrateProtocolViolation {
        /// The oracle's description.
        detail: String,
    },
    /// A task's cancellation was requested or acknowledged, but no drain absorbed its
    /// completion before the run ended.
    CancellationUnfinished {
        /// The task's journal ordinal.
        task: u32,
    },
    /// An operation names a reservation label no earlier operation bound.
    UnboundReservation(u32),
    /// An operation binds a reservation label that is already bound.
    ReservationLabelRebound(u32),
    /// An operation resolves a reservation that already resolved.
    ReservationResolved(u32),
    /// An effect operation completed, but the substrate's trace does not show the step
    /// it asked for.
    EffectUnobserved {
        /// The operation.
        operation: &'static str,
    },
    /// A reservation's holder dropped it unresolved (it returned while still holding
    /// it), and the substrate recorded a leak or aborted it for error. The family has no
    /// event for that: it is the leak the lift forbids at region close.
    ReservationDropped {
        /// The reservation's journal ordinal.
        reservation: u32,
    },
    /// A transfer names a `Transaction`: the effect family's reservation cannot move.
    EffectNotTransferable(u32),
    /// The substrate's own obligation records or its obligation-leak oracle disagree with
    /// the ledger the trace produced.
    SubstrateLedgerDisagrees {
        /// What disagrees.
        detail: String,
    },
    /// A sleep or an advance of zero nanoseconds: it would not move the clock or park
    /// the task.
    ZeroDuration {
        /// The operation.
        operation: &'static str,
    },
    /// An operation commands a task that has not begun. The command would wait unseen
    /// until the task's first poll.
    TaskNotBegun {
        /// The task's journal ordinal.
        task: u32,
    },
    /// An operation commands a task that has ended. Nothing would ever take the command.
    TaskEnded {
        /// The task's journal ordinal.
        task: u32,
    },
    /// An operation begins a task that has already begun.
    TaskAlreadyBegun {
        /// The task's journal ordinal.
        task: u32,
    },
    /// An operation commands a task that is asleep on a timer. The command would wait
    /// unseen until the timer fires.
    TaskAsleep {
        /// The task's journal ordinal.
        task: u32,
    },
    /// An operation commands a task that waits on a channel. The command would wait
    /// unseen until the channel frees it.
    TaskBlocked {
        /// The task's journal ordinal.
        task: u32,
    },
    /// An operation names a channel label no earlier operation bound.
    UnboundChannel(u32),
    /// An operation binds a channel label that is already bound.
    ChannelLabelRebound(u32),
    /// A channel of capacity zero.
    ZeroCapacity(u32),
    /// A send on a channel whose kept sender the program already dropped.
    SendersAlreadyClosed(u32),
    /// One operation woke more than one other task, or a cancellation woke a task it did
    /// not cancel: their steps would interleave in an order the substrate's scheduler
    /// picks.
    UnorderedWake {
        /// The task that woke.
        task: u32,
    },
    /// The substrate delivered a message, or dropped a queue, other than the one the
    /// trace's own sends imply.
    SubstrateChannelDisagrees {
        /// What disagrees.
        detail: String,
    },
    /// During an advance, the substrate traced an event that is not a timer fire or a
    /// woken task's step, so the batch has no canonical order.
    UnorderedDuringAdvance {
        /// The substrate's name for the event kind.
        kind: String,
    },
    /// A task's deadline is not ahead of the virtual clock when it is spawned.
    DeadlineNotAhead {
        /// The deadline.
        deadline: u64,
        /// The clock's instant.
        now: u64,
    },
    /// A task's deadline cancellation and a region's cancellation both reached it, so
    /// its completion's cause and its journaled request disagree. The cancellation
    /// family journals one cause per task, so the run is not bound (bn-36wy3).
    CancelRaced {
        /// The task's journal ordinal.
        task: u32,
    },
}

impl BindingRefusal {
    /// The INV-008 reason this refusal is an instance of, when it is one.
    ///
    /// Malformed programs and choice logs have none: they do not describe a run. Neither
    /// does [`Self::SubstrateRefused`]: the substrate answered an illegal call (a spawn
    /// into a closed region, say) with a refusal, so the program does not describe a
    /// run of this substrate either.
    #[must_use]
    pub const fn inconclusive_reason(&self) -> Option<InconclusiveReason> {
        match self {
            Self::UninstrumentedEvent { .. }
            | Self::TaskPanicked { .. }
            | Self::FamilyNotBound(_)
            | Self::UnorderedDuringAdvance { .. }
            | Self::UnorderedWake { .. }
            | Self::UnmappedCancelReason { .. }
            | Self::CancelRaced { .. }
            | Self::ReservationDropped { .. } => Some(InconclusiveReason::Unsupported),
            Self::SubstrateProtocolViolation { .. }
            | Self::SubstrateLedgerDisagrees { .. }
            | Self::SubstrateChannelDisagrees { .. } => Some(InconclusiveReason::EngineError),
            Self::StepLimit => Some(InconclusiveReason::ResourceExhausted),
            Self::TraceOverflow { .. }
            | Self::TraceOutOfOrder { .. }
            | Self::UnknownSubstrateEntity { .. }
            | Self::GateUntraced
            | Self::TaskOutcomeUnobserved { .. }
            | Self::UndrainedCancellation { .. }
            | Self::CancellationUnfinished { .. }
            | Self::EffectUnobserved { .. } => Some(InconclusiveReason::InsufficientTelemetry),
            Self::SubstrateRefused { .. }
            | Self::UnboundRegion(_)
            | Self::UnboundTask(_)
            | Self::RegionLabelRebound(_)
            | Self::TaskLabelRebound(_)
            | Self::ChoiceOutOfRange { .. }
            | Self::ChoiceLogExhausted { .. }
            | Self::ChoiceLogOverrun { .. }
            | Self::JournalFull
            | Self::LifecycleNotObserved
            | Self::UnboundReservation(_)
            | Self::ReservationLabelRebound(_)
            | Self::ReservationResolved(_)
            | Self::EffectNotTransferable(_)
            | Self::ZeroDuration { .. }
            | Self::TaskNotBegun { .. }
            | Self::TaskEnded { .. }
            | Self::TaskAlreadyBegun { .. }
            | Self::TaskAsleep { .. }
            | Self::TaskBlocked { .. }
            | Self::UnboundChannel(_)
            | Self::ChannelLabelRebound(_)
            | Self::ZeroCapacity(_)
            | Self::DeadlineNotAhead { .. }
            | Self::SendersAlreadyClosed(_) => None,
        }
    }
}

impl fmt::Display for BindingRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnboundRegion(label) => write!(f, "region label {label} is not bound"),
            Self::UnboundTask(label) => write!(f, "task label {label} is not bound"),
            Self::RegionLabelRebound(label) => write!(f, "region label {label} is already bound"),
            Self::TaskLabelRebound(label) => write!(f, "task label {label} is already bound"),
            Self::ChoiceOutOfRange {
                position,
                choice,
                enabled,
            } => write!(
                f,
                "choice {choice} at position {position} exceeds the {enabled} enabled actors"
            ),
            Self::ChoiceLogExhausted { remaining } => write!(
                f,
                "the choice log ended with {remaining} operations unapplied"
            ),
            Self::ChoiceLogOverrun { position } => write!(
                f,
                "choice {position} was not used: every program had finished"
            ),
            Self::JournalFull => f.write_str("the journal cannot hold another event"),
            Self::SubstrateRefused { operation, detail } => {
                write!(f, "the substrate refused {operation}: {detail}")
            }
            Self::StepLimit => f.write_str("the lab runtime reached its step limit"),
            Self::TraceOverflow { pushed, capacity } => write!(
                f,
                "the substrate trace overflowed: {pushed} events into a buffer of {capacity}"
            ),
            Self::TraceOutOfOrder { seq } => {
                write!(f, "trace event {seq} arrived after a later one")
            }
            Self::UnknownSubstrateEntity { what } => {
                write!(f, "the substrate traced a {what} this run did not create")
            }
            Self::GateUntraced => f.write_str("a gate was polled with no current Cx"),
            Self::TaskOutcomeUnobserved { task } => {
                write!(f, "task t{task} completed but published no outcome")
            }
            Self::TaskPanicked { task } => {
                write!(f, "task t{task} panicked; task failure is not bound")
            }
            Self::UndrainedCancellation { task } => write!(
                f,
                "task t{task} was cancelled but no enclosing region's close completed"
            ),
            Self::UninstrumentedEvent { family, kind } => match family {
                Some(family) => write!(
                    f,
                    "the substrate traced {kind}, a {family} event, which is not bound ({})",
                    family.requirement()
                ),
                None => write!(f, "the substrate traced {kind}, which no family maps"),
            },
            Self::FamilyNotBound(family) => write!(
                f,
                "the {family} family is not bound to the substrate ({})",
                family.requirement()
            ),
            Self::LifecycleNotObserved => f.write_str(
                "a run must observe the lifecycle family, which allocates task ordinals",
            ),
            Self::UnmappedCancelReason { kind } => {
                write!(
                    f,
                    "the cancellation family has no cause for reason kind {kind}"
                )
            }
            Self::SubstrateProtocolViolation { detail } => {
                write!(f, "the substrate's cancellation oracle disagrees: {detail}")
            }
            Self::CancellationUnfinished { task } => write!(
                f,
                "task t{task}'s cancellation began but no drain absorbed its completion"
            ),
            Self::UnboundReservation(label) => {
                write!(f, "reservation label {label} is not bound")
            }
            Self::ReservationLabelRebound(label) => {
                write!(f, "reservation label {label} is already bound")
            }
            Self::ReservationResolved(label) => {
                write!(f, "reservation label {label} has already resolved")
            }
            Self::EffectUnobserved { operation } => write!(
                f,
                "the substrate's trace does not show the step {operation} asked for"
            ),
            Self::ReservationDropped { reservation } => write!(
                f,
                "e{reservation} was dropped unresolved and the substrate aborted it for error"
            ),
            Self::EffectNotTransferable(label) => write!(
                f,
                "reservation label {label} is a transaction, which cannot be transferred"
            ),
            Self::SubstrateLedgerDisagrees { detail } => {
                write!(f, "the substrate's obligation ledger disagrees: {detail}")
            }
            Self::ZeroDuration { operation } => {
                write!(f, "{operation} of zero virtual nanoseconds")
            }
            Self::TaskNotBegun { task } => {
                write!(f, "task t{task} has not begun and cannot take a command")
            }
            Self::TaskEnded { task } => {
                write!(f, "task t{task} has ended and cannot take a command")
            }
            Self::TaskAlreadyBegun { task } => write!(f, "task t{task} has already begun"),
            Self::TaskAsleep { task } => {
                write!(
                    f,
                    "task t{task} is asleep on a timer and cannot take a command"
                )
            }
            Self::TaskBlocked { task } => {
                write!(
                    f,
                    "task t{task} waits on a channel and cannot take a command"
                )
            }
            Self::UnboundChannel(label) => write!(f, "channel label {label} is not bound"),
            Self::ChannelLabelRebound(label) => {
                write!(f, "channel label {label} is already bound")
            }
            Self::ZeroCapacity(label) => write!(f, "channel label {label} has capacity zero"),
            Self::SendersAlreadyClosed(label) => {
                write!(f, "channel label {label}'s kept sender was already dropped")
            }
            Self::UnorderedWake { task } => write!(
                f,
                "task t{task} woke in an operation that already woke another, or that did not cancel it"
            ),
            Self::SubstrateChannelDisagrees { detail } => {
                write!(
                    f,
                    "the substrate's channel disagrees with the trace: {detail}"
                )
            }
            Self::UnorderedDuringAdvance { kind } => write!(
                f,
                "the substrate traced {kind} during an advance, which has no canonical place"
            ),
            Self::DeadlineNotAhead { deadline, now } => write!(
                f,
                "a deadline of {deadline} is not ahead of the virtual clock at {now}"
            ),
            Self::CancelRaced { task } => write!(
                f,
                "t{task}'s deadline and a region's cancellation both reached it"
            ),
        }
    }
}

impl core::error::Error for BindingRefusal {}

// --- the gate ----------------------------------------------------------------------

/// The prefix of every gate mark in the substrate's trace.
const GATE_MARK: &str = "continuum.lifecycle-gate/1";

/// The prefix of the runtime's own obligation-handoff trace message (asupersync 0.5.0,
/// `trace::event::ObligationHandoff`).
const HANDOFF_MARK: &str = "obligation_handoff_v";

/// The prefix of every channel-gate mark in the substrate's trace.
const CHANNEL_MARK: &str = "continuum.channel-gate/1";

/// The prefix of the gate's cancellation acknowledgement mark.
const CANCEL_MARK: &str = "continuum.cancellation-gate/1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GatePhase {
    Created,
    Running,
    Suspended,
}

#[derive(Debug, Clone)]
enum Command {
    Park,
    Finish,
    Reserve(ReservationLabel, SubstrateObligationKind),
    Commit(ReservationLabel),
    Abort(ReservationLabel),
    /// Hand the obligation to the task whose `Cx` and gate these are.
    Transfer(ReservationLabel, Box<Cx>, Gate),
    /// Sleep this many nanoseconds on the `Cx`'s virtual timer driver.
    Sleep(u64),
    /// Send `message` on `channel` through this sender clone.
    Send {
        channel: u32,
        message: u64,
        sender: MpscSender<u64>,
    },
    /// Receive once on `channel`, whose receiver this task holds.
    Recv(u32),
}

#[derive(Debug)]
struct GateState {
    phase: GatePhase,
    commands: VecDeque<Command>,
    waker: Option<Waker>,
    untraced: bool,
    /// How many reservations the task holds. A task that holds one is mid-effect, not
    /// parked: its pending polls write no `suspend` mark.
    held: usize,
    /// The substrate's refusal of the last effect command, if it refused.
    refused: Option<String>,
    /// A resolution was posted in this poll. The runtime applies it after the poll
    /// returns, so the next pending poll yields once, unmarked, and only the poll after
    /// that may write `suspend`: the mark then follows the resolution in the trace.
    settling: bool,
    /// Obligations transferred to this task, waiting for its body to take them.
    inbox: Vec<(ReservationLabel, ObligationToken)>,
    /// Channel receivers handed to this task, waiting for its body to take them.
    receiver_inbox: Vec<(u32, MpscReceiver<u64>)>,
}

type Gate = Arc<Mutex<GateState>>;

fn lock(gate: &Gate) -> MutexGuard<'_, GateState> {
    gate.lock().unwrap_or_else(PoisonError::into_inner)
}

/// One wait on a task's gate. It resolves to the next command, or to `None` when the
/// substrate has requested cancellation.
struct GateWait {
    gate: Gate,
    slot: usize,
}

impl Future for GateWait {
    type Output = Option<Command>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let cx = Cx::current();
        if let Some(cx) = &cx
            && cx.checkpoint().is_err()
        {
            // The substrate's acknowledgement: this return is what moves the task
            // from `CancelRequested` to `Cancelling`.
            cx.trace(&format!("{CANCEL_MARK} ack {}", self.slot));
            return Poll::Ready(None);
        }
        let mut marks: Vec<&'static str> = Vec::new();
        let mut state = lock(&self.gate);
        if state.phase == GatePhase::Created {
            state.phase = GatePhase::Running;
            marks.push("begin");
        }
        let outcome = if let Some(command) = state.commands.pop_front() {
            if state.phase == GatePhase::Suspended {
                state.phase = GatePhase::Running;
                marks.push("resume");
            }
            Poll::Ready(Some(command))
        } else if state.settling {
            state.settling = false;
            context.waker().wake_by_ref();
            Poll::Pending
        } else {
            state.waker = Some(context.waker().clone());
            if state.phase == GatePhase::Running && state.held == 0 {
                state.phase = GatePhase::Suspended;
                marks.push("suspend");
            }
            Poll::Pending
        };
        if !marks.is_empty() {
            match &cx {
                Some(cx) => {
                    for mark in marks {
                        cx.trace(&format!("{GATE_MARK} {mark} {}", self.slot));
                    }
                }
                None => state.untraced = true,
            }
        }
        drop(state);
        outcome
    }
}

/// The kind every [`SubstrateOp::Reserve`] takes: a two-phase effect that commits or
/// rolls back. Only this kind is the reserve/commit/abort family's.
const RESERVATION_KIND: SubstrateObligationKind = SubstrateObligationKind::Transaction;

/// The substrate kind for a journal kind.
const fn substrate_kind(kind: ObligationKind) -> SubstrateObligationKind {
    match kind {
        ObligationKind::SendPermit => SubstrateObligationKind::SendPermit,
        ObligationKind::Ack => SubstrateObligationKind::Ack,
        ObligationKind::Lease => SubstrateObligationKind::Lease,
        ObligationKind::IoOp => SubstrateObligationKind::IoOp,
        ObligationKind::SemaphorePermit => SubstrateObligationKind::SemaphorePermit,
        ObligationKind::Transaction => SubstrateObligationKind::Transaction,
    }
}

/// The journal kind for a substrate kind.
const fn journal_kind(kind: SubstrateObligationKind) -> ObligationKind {
    match kind {
        SubstrateObligationKind::SendPermit => ObligationKind::SendPermit,
        SubstrateObligationKind::Ack => ObligationKind::Ack,
        SubstrateObligationKind::Lease => ObligationKind::Lease,
        SubstrateObligationKind::IoOp => ObligationKind::IoOp,
        SubstrateObligationKind::SemaphorePermit => ObligationKind::SemaphorePermit,
        SubstrateObligationKind::Transaction => ObligationKind::Transaction,
    }
}

/// One channel operation (a reserve or a receive). It resolves to the operation's
/// result, or to `None` when the substrate has requested cancellation (the ack mark
/// written). The first `Pending` writes `blocked`, the channel-gate mark that the
/// operation waits; the task is then parked, as a sleeping one is.
struct ChannelWait<F> {
    gate: Gate,
    slot: usize,
    inner: Pin<Box<F>>,
    blocked: String,
    marked: bool,
}

impl<F: Future> Future for ChannelWait<F> {
    type Output = Option<F::Output>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let cx = Cx::current();
        if let Some(cx) = &cx
            && cx.checkpoint().is_err()
        {
            cx.trace(&format!("{CANCEL_MARK} ack {}", self.slot));
            return Poll::Ready(None);
        }
        let outcome = self.inner.as_mut().poll(context);
        let blocked = matches!(outcome, Poll::Pending) && !self.marked;
        if blocked {
            self.marked = true;
        }
        let mut state = lock(&self.gate);
        let mut marks: Vec<String> = Vec::new();
        if blocked {
            marks.push(self.blocked.clone());
        }
        match outcome {
            Poll::Ready(_) if state.phase == GatePhase::Suspended => {
                state.phase = GatePhase::Running;
                marks.push(format!("{GATE_MARK} resume {}", self.slot));
            }
            Poll::Pending if state.phase == GatePhase::Running && state.held == 0 => {
                state.phase = GatePhase::Suspended;
                marks.push(format!("{GATE_MARK} suspend {}", self.slot));
            }
            _ => {}
        }
        match &cx {
            Some(cx) => {
                for mark in marks {
                    cx.trace(&mark);
                }
            }
            None if !marks.is_empty() => state.untraced = true,
            None => {}
        }
        drop(state);
        outcome.map(Some)
    }
}

/// Write a channel-gate mark through the current `Cx`.
fn channel_mark(gate: &Gate, mark: &str) {
    match Cx::current() {
        Some(cx) => cx.trace(&format!("{CHANNEL_MARK} {mark}")),
        None => lock(gate).untraced = true,
    }
}

/// A task's end drops its receivers: each one's mark names what was still queued.
fn drop_receivers(gate: &Gate, slot: usize, receivers: &mut BTreeMap<u32, MpscReceiver<u64>>) {
    for (channel, receiver) in core::mem::take(receivers) {
        channel_mark(
            gate,
            &format!("receiver-gone {slot} {channel} {}", receiver.len()),
        );
        drop(receiver);
    }
}

/// A cancelled task's cleanup: abort every held obligation, drop every receiver.
fn abandon_all(
    gate: &Gate,
    slot: usize,
    held: &mut BTreeMap<ReservationLabel, ObligationToken>,
    receivers: &mut BTreeMap<u32, MpscReceiver<u64>>,
) {
    for (_, token) in core::mem::take(held) {
        token.abort(ObligationAbortReason::Cancel);
    }
    drop_receivers(gate, slot, receivers);
}

/// One sleep on the task's virtual timer. It resolves to `true` when the timer fires,
/// or to `false` when the substrate has requested cancellation (the ack mark written).
///
/// A sleeping task is parked: the first pending poll writes `suspend`, and the poll
/// that sees the timer fire writes `resume`, after the `Sleep` future has traced the
/// fire. A task that holds a reservation stays mid-effect and writes neither.
struct SleepWait {
    gate: Gate,
    slot: usize,
    sleep: Pin<Box<Sleep>>,
}

impl Future for SleepWait {
    type Output = bool;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let cx = Cx::current();
        if let Some(cx) = &cx
            && cx.checkpoint().is_err()
        {
            cx.trace(&format!("{CANCEL_MARK} ack {}", self.slot));
            return Poll::Ready(false);
        }
        let outcome = self.sleep.as_mut().poll(context).map(|()| true);
        let mut state = lock(&self.gate);
        let mark = match outcome {
            Poll::Ready(_) if state.phase == GatePhase::Suspended => {
                state.phase = GatePhase::Running;
                Some("resume")
            }
            Poll::Pending if state.phase == GatePhase::Running && state.held == 0 => {
                state.phase = GatePhase::Suspended;
                Some("suspend")
            }
            _ => None,
        };
        if let Some(mark) = mark {
            match &cx {
                Some(cx) => cx.trace(&format!("{GATE_MARK} {mark} {}", self.slot)),
                None => state.untraced = true,
            }
        }
        drop(state);
        outcome
    }
}

async fn gated_body(gate: Gate, slot: usize) {
    // The task's own obligations, by program label. Ascending order makes the aborts a
    // cancellation issues deterministic within the task.
    let mut held: BTreeMap<ReservationLabel, ObligationToken> = BTreeMap::new();
    let mut receivers: BTreeMap<u32, MpscReceiver<u64>> = BTreeMap::new();
    loop {
        let command = GateWait {
            gate: Arc::clone(&gate),
            slot,
        }
        .await;
        // Obligations transferred here since the last poll are this task's now.
        held.extend(core::mem::take(&mut lock(&gate).inbox));
        receivers.extend(core::mem::take(&mut lock(&gate).receiver_inbox));
        let settling = matches!(command, Some(Command::Commit(_) | Command::Abort(_)));
        let refused = match command {
            Some(Command::Park) => None,
            Some(Command::Reserve(label, kind)) => match Cx::current() {
                Some(cx) => match cx.try_register_obligation_checked(kind, cx.task_id()) {
                    Ok(Some(token)) => {
                        held.insert(label, token);
                        None
                    }
                    Ok(None) => Some("the task's Cx carries no obligation runtime".to_owned()),
                    Err(error) => Some(error.to_string()),
                },
                None => {
                    lock(&gate).untraced = true;
                    None
                }
            },
            Some(Command::Commit(label)) => match held.remove(&label) {
                Some(token) => (!token.commit()).then(|| "commit was not accepted".to_owned()),
                None => Some("no held reservation has this label".to_owned()),
            },
            Some(Command::Abort(label)) => match held.remove(&label) {
                Some(token) => (!token.abort(ObligationAbortReason::Explicit))
                    .then(|| "abort was not accepted".to_owned()),
                None => Some("no held reservation has this label".to_owned()),
            },
            Some(Command::Transfer(label, destination, inbox)) => match held.remove(&label) {
                Some(token) => match token.try_transfer(&*destination) {
                    Ok(next) => {
                        lock(&inbox).inbox.push((label, next));
                        None
                    }
                    Err(failure) => {
                        let (reason, token) = failure.into_parts();
                        held.insert(label, token);
                        Some(reason.to_string())
                    }
                },
                None => Some("no held reservation has this label".to_owned()),
            },
            Some(Command::Sleep(nanos)) => {
                match Cx::current().and_then(|cx| cx.timer_driver()) {
                    // Without the runtime's virtual driver a `Sleep` would fall back to
                    // a wall-clock thread (INV-005), so the gate refuses to sleep.
                    None => Some("the task's Cx carries no virtual timer driver".to_owned()),
                    Some(driver) => {
                        let deadline = driver.now().saturating_add_nanos(nanos);
                        let woke = SleepWait {
                            gate: Arc::clone(&gate),
                            slot,
                            sleep: Box::pin(sleep_until(deadline)),
                        }
                        .await;
                        if !woke {
                            // Cancelled while asleep: the timer is gone, and the cleanup
                            // is to abort every held obligation and drop every receiver.
                            abandon_all(&gate, slot, &mut held, &mut receivers);
                            break;
                        }
                        None
                    }
                }
            }
            Some(Command::Send {
                channel,
                message,
                sender,
            }) => match Cx::current() {
                None => {
                    lock(&gate).untraced = true;
                    None
                }
                Some(cx) => {
                    let outcome = ChannelWait {
                        gate: Arc::clone(&gate),
                        slot,
                        inner: Box::pin(sender.reserve_checked(&cx)),
                        blocked: format!("{CHANNEL_MARK} send-blocked {slot} {channel} {message}"),
                        marked: false,
                    }
                    .await;
                    match outcome {
                        None => {
                            channel_mark(
                                &gate,
                                &format!("send-abandoned {slot} {channel} {message}"),
                            );
                            abandon_all(&gate, slot, &mut held, &mut receivers);
                            break;
                        }
                        // The permit's commit is the substrate's own trace of the send.
                        Some(Ok(permit)) => permit
                            .send(message)
                            .is_err()
                            .then(|| "the permit refused the message".to_owned()),
                        Some(Err(CheckedSendError::Channel(_))) => {
                            channel_mark(&gate, &format!("send-closed {slot} {channel} {message}"));
                            None
                        }
                        Some(Err(other)) => Some(format!("{other:?}")),
                    }
                }
            },
            Some(Command::Recv(channel)) => match (Cx::current(), receivers.get_mut(&channel)) {
                (None, _) => {
                    lock(&gate).untraced = true;
                    None
                }
                (_, None) => Some("this task holds no receiver of the channel".to_owned()),
                (Some(cx), Some(receiver)) => {
                    let outcome = ChannelWait {
                        gate: Arc::clone(&gate),
                        slot,
                        inner: Box::pin(receiver.recv(&cx)),
                        blocked: format!("{CHANNEL_MARK} recv-blocked {slot} {channel}"),
                        marked: false,
                    }
                    .await;
                    match outcome {
                        None => {
                            channel_mark(&gate, &format!("recv-abandoned {slot} {channel}"));
                            abandon_all(&gate, slot, &mut held, &mut receivers);
                            break;
                        }
                        Some(Ok(message)) => {
                            channel_mark(&gate, &format!("recv {slot} {channel} {message}"));
                            None
                        }
                        Some(Err(RecvError::Disconnected)) => {
                            channel_mark(&gate, &format!("recv-closed {slot} {channel}"));
                            None
                        }
                        Some(Err(other)) => Some(format!("{other:?}")),
                    }
                }
            },
            // Returning drops every receiver (marked) and every obligation held (the
            // substrate records a leak).
            Some(Command::Finish) => {
                drop_receivers(&gate, slot, &mut receivers);
                break;
            }
            // Cancellation observed: the cleanup is to abort every held obligation and
            // drop every receiver.
            None => {
                abandon_all(&gate, slot, &mut held, &mut receivers);
                break;
            }
        };
        let mut state = lock(&gate);
        state.settling = settling;
        // Only a held reservation (the effect kind) keeps the task mid-effect.
        state.held = held
            .values()
            .filter(|token| token.kind() == RESERVATION_KIND)
            .count();
        if refused.is_some() {
            state.refused = refused;
        }
    }
}

// --- the driver --------------------------------------------------------------------

struct Slot {
    id: SubstrateTask,
    handle: TaskHandle<()>,
    gate: Gate,
    resumability: Resumability,
    ordinal: Option<TaskOrdinal>,
    /// The task's budget deadline, in virtual nanoseconds, when it has one.
    deadline: Option<u64>,
}

/// What the trace has shown of one task's cancellation, before its drain journals it.
#[derive(Debug, Default)]
struct CancelTrack {
    acknowledged: bool,
    completed: Option<CancelCause>,
}

/// The cancellation family's cause for a substrate reason kind.
fn cause_of(kind: CancelKind) -> Result<CancelCause, BindingRefusal> {
    match kind {
        CancelKind::User => Ok(CancelCause::User),
        CancelKind::ParentCancelled => Ok(CancelCause::ParentCancelled),
        CancelKind::Deadline => Ok(CancelCause::Deadline),
        other => Err(BindingRefusal::UnmappedCancelReason {
            kind: format!("{other:?}"),
        }),
    }
}

/// A program's obligation label: its obligation ordinal, and whether it resolved.
#[derive(Debug, Clone, Copy)]
struct LabelState {
    obligation: u32,
    resolved: bool,
}

/// Where a traced obligation is in the substrate's ledger, as the trace shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Standing {
    Open,
    Discharged,
    Leaked,
}

/// A substrate obligation the run reserved, as the trace shows it.
#[derive(Debug, Clone, Copy)]
struct Held {
    /// The obligations family's ordinal.
    obligation: u32,
    /// The reserve/commit/abort family's ordinal, for a `Transaction`.
    reservation: Option<u32>,
    kind: SubstrateObligationKind,
    slot: usize,
    task: TaskOrdinal,
    region: u32,
    standing: Standing,
}

/// A channel the run opened, and the binding's account of it from the trace.
struct ChannelTrack {
    receiver_slot: usize,
    sender: Option<MpscSender<u64>>,
    queue: VecDeque<u64>,
}

/// A timer the trace scheduled.
#[derive(Debug, Clone, Copy)]
struct TimerTrack {
    ordinal: u32,
    slot: usize,
    task: TaskOrdinal,
    deadline: u64,
}

/// What one advance's run produced, per woken task, before it is journaled in canonical
/// order: by the fired timer's deadline, then its ordinal.
#[derive(Debug, Default)]
struct Advance {
    per_task: BTreeMap<u32, Vec<EventBody>>,
    keys: BTreeMap<u32, (u64, u32)>,
    unordered: Option<String>,
}

struct Driver {
    lab: LabRuntime,
    config: BindingConfig,
    region_labels: BTreeMap<RegionLabel, SubstrateRegion>,
    task_labels: BTreeMap<TaskLabel, usize>,
    slots: Vec<Slot>,
    slot_of: BTreeMap<SubstrateTask, usize>,
    root: SubstrateRegion,
    region_ordinals: BTreeMap<SubstrateRegion, RegionOrdinal>,
    parents: Vec<Option<u32>>,
    cancelled: BTreeSet<u32>,
    pending_cancelled: Vec<(TaskOrdinal, u32)>,
    pending_requests: Vec<(TaskOrdinal, CancelCause)>,
    pending_closes: Vec<RegionOrdinal>,
    reservation_labels: BTreeMap<ReservationLabel, LabelState>,
    reservations: BTreeMap<ObligationId, Held>,
    reservation_owner_label: BTreeMap<u32, ReservationLabel>,
    next_reservation: u32,
    next_obligation: u32,
    op_reserved: Vec<(u32, usize)>,
    op_resolved: Vec<u32>,
    op_transferred: Vec<u32>,
    pending_transfer: Option<(ObligationId, usize)>,
    pending_aborts: BTreeMap<u32, BTreeSet<u32>>,
    pending_discharges: BTreeMap<u32, BTreeSet<u32>>,
    timers: BTreeMap<u64, TimerTrack>,
    next_timer: u32,
    sleeping: BTreeMap<usize, u32>,
    sleep_slot: Option<usize>,
    op_scheduled: Vec<(u32, usize)>,
    advancing: Option<Advance>,
    pending_timer_cancels: BTreeMap<u32, BTreeSet<(u32, u64)>>,
    fire_order: Vec<TimerOrdinal>,
    pending_leaks: BTreeSet<u32>,
    channels: Vec<ChannelTrack>,
    channel_labels: BTreeMap<ChannelLabel, u32>,
    next_message: u64,
    pending_sends: BTreeMap<usize, (u32, u64)>,
    op_channel: Vec<&'static str>,
    acked: BTreeSet<u32>,
    pending_channel: BTreeMap<u32, Vec<EventBody>>,
    op_subject: Option<u32>,
    op_woken: BTreeSet<u32>,
    in_cancel: bool,
    channel_blocked: BTreeSet<usize>,
    wake_batch: BTreeMap<u32, Vec<EventBody>>,
    wake_order: Vec<TaskOrdinal>,
    leak_order: Vec<ObligationOrdinal>,
    begun: BTreeSet<usize>,
    ended: BTreeSet<usize>,
    close_order: Vec<RegionOrdinal>,
    phases: BTreeMap<u32, CancelTrack>,
    /// Tasks a region's cancellation requested (`CancelRequest` trace events), whatever
    /// the run observes: substrate state a refusal ([`BindingRefusal::CancelRaced`])
    /// depends on, so no projection may lose it (cr-3pu5cu).
    region_requested: BTreeSet<u32>,
    next_task: u32,
    observed: BTreeSet<u64>,
    last_seq: Option<u64>,
    record: RecordContext,
}

fn ordinal(count: usize) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
}

impl Driver {
    fn new(config: BindingConfig) -> Result<Self, BindingRefusal> {
        let mut lab_config = LabConfig::new(config.lab_seed)
            .panic_on_leak(false)
            .panic_on_futurelock(false)
            .trace_capacity(config.trace_capacity)
            .max_steps(config.max_steps);
        lab_config.panic_on_cancellation_violation = false;
        let mut lab = LabRuntime::new(lab_config);
        let root = lab.state.create_root_region(Budget::INFINITE);
        let mut region_labels = BTreeMap::new();
        region_labels.insert(RegionLabel::ROOT, root);
        let mut driver = Self {
            lab,
            config,
            region_labels,
            task_labels: BTreeMap::new(),
            slots: Vec::new(),
            slot_of: BTreeMap::new(),
            root,
            region_ordinals: BTreeMap::new(),
            parents: Vec::new(),
            cancelled: BTreeSet::new(),
            pending_cancelled: Vec::new(),
            pending_requests: Vec::new(),
            pending_closes: Vec::new(),
            reservation_labels: BTreeMap::new(),
            reservations: BTreeMap::new(),
            reservation_owner_label: BTreeMap::new(),
            next_reservation: 0,
            next_obligation: 0,
            op_reserved: Vec::new(),
            op_resolved: Vec::new(),
            op_transferred: Vec::new(),
            pending_transfer: None,
            pending_aborts: BTreeMap::new(),
            pending_discharges: BTreeMap::new(),
            timers: BTreeMap::new(),
            next_timer: 0,
            sleeping: BTreeMap::new(),
            sleep_slot: None,
            op_scheduled: Vec::new(),
            advancing: None,
            pending_timer_cancels: BTreeMap::new(),
            fire_order: Vec::new(),
            pending_leaks: BTreeSet::new(),
            channels: Vec::new(),
            channel_labels: BTreeMap::new(),
            next_message: 0,
            pending_sends: BTreeMap::new(),
            op_channel: Vec::new(),
            acked: BTreeSet::new(),
            pending_channel: BTreeMap::new(),
            op_subject: None,
            op_woken: BTreeSet::new(),
            in_cancel: false,
            channel_blocked: BTreeSet::new(),
            wake_batch: BTreeMap::new(),
            wake_order: Vec::new(),
            leak_order: Vec::new(),
            begun: BTreeSet::new(),
            ended: BTreeSet::new(),
            close_order: Vec::new(),
            phases: BTreeMap::new(),
            region_requested: BTreeSet::new(),
            next_task: 0,
            observed: BTreeSet::new(),
            last_seq: None,
            record: RecordContext::default(),
        };
        driver.sync()?;
        if driver.region_ordinals.get(&root) != Some(&RegionOrdinal(0)) {
            return Err(BindingRefusal::UnknownSubstrateEntity {
                what: "root region",
            });
        }
        Ok(driver)
    }

    fn region(&self, label: RegionLabel) -> Result<SubstrateRegion, BindingRefusal> {
        self.region_labels
            .get(&label)
            .copied()
            .ok_or(BindingRefusal::UnboundRegion(label.0))
    }

    fn slot(&self, label: TaskLabel) -> Result<usize, BindingRefusal> {
        self.task_labels
            .get(&label)
            .copied()
            .ok_or(BindingRefusal::UnboundTask(label.0))
    }

    /// Append a lifecycle event. A pending batch of region closes is journaled first,
    /// because every close in it came before this event in the trace.
    fn append(&mut self, event: LifecycleEvent) {
        self.append_body(EventBody::Lifecycle(event));
    }

    fn append_body(&mut self, body: EventBody) {
        self.flush_leaks();
        if let Some(batch) = &mut self.advancing {
            // Nothing but timer fires and woken tasks' steps belongs in an advance.
            batch
                .unordered
                .get_or_insert_with(|| format!("{:?}", body.family()));
            return;
        }
        self.flush_closes();
        self.record.append(body);
    }

    const fn observes_effect(&self) -> bool {
        self.config.families.contains(Family::Effect)
    }

    const fn observes_obligation(&self) -> bool {
        self.config.families.contains(Family::Obligation)
    }

    const fn observes_time(&self) -> bool {
        self.config.families.contains(Family::Time)
    }

    const fn observes_channel(&self) -> bool {
        self.config.families.contains(Family::Channel)
    }

    fn channel_of(&self, label: ChannelLabel) -> Result<u32, BindingRefusal> {
        self.channel_labels
            .get(&label)
            .copied()
            .ok_or(BindingRefusal::UnboundChannel(label.0))
    }

    /// A task stepped in the current operation: note it when the operation woke it (it is
    /// neither the task the operation commands nor one a cancellation cancels). Woken
    /// tasks are ordered by `flush_wakes`; an advance orders its own batch.
    fn note_task_mark(&mut self, task: u32) -> Result<(), BindingRefusal> {
        if self.advancing.is_some() {
            return Ok(());
        }
        let woken = if self.in_cancel {
            !self.acked.contains(&task)
        } else {
            self.op_subject != Some(task)
        };
        if woken && self.op_woken.insert(task) {
            self.wake_order.push(TaskOrdinal(task));
        }
        Ok(())
    }

    /// Journal a channel event of `task`: with its cancellation's phases when the task
    /// is being cancelled, otherwise in trace order.
    fn channel_event(&mut self, task: TaskOrdinal, event: ChannelEvent) {
        if !self.observes_channel() {
            return;
        }
        let body = EventBody::Channel(event);
        if self.acked.contains(&task.0) {
            self.pending_channel.entry(task.0).or_default().push(body);
        } else {
            self.append_task_event(task, body);
        }
    }

    /// A `SendPermit` committed: the send it belongs to is a message on its channel.
    fn observe_sent(&mut self, slot: usize, task: TaskOrdinal) -> Result<(), BindingRefusal> {
        self.note_task_mark(task.0)?;
        let (channel, message) =
            self.pending_sends
                .remove(&slot)
                .ok_or(BindingRefusal::UnknownSubstrateEntity {
                    what: "send permit",
                })?;
        self.channel_blocked.remove(&slot);
        self.channels[channel as usize].queue.push_back(message);
        self.op_channel.push("send");
        self.channel_event(
            task,
            ChannelEvent::Sent {
                channel: ChannelOrdinal(channel),
                message: MessageOrdinal(message),
                sender: task,
            },
        );
        Ok(())
    }

    /// A channel-gate mark: `<kind> <slot> <channel> [<message or length>]`.
    fn observe_channel(&mut self, rest: &str) -> Result<(), BindingRefusal> {
        let unknown = BindingRefusal::UnknownSubstrateEntity {
            what: "channel mark",
        };
        let mut parts = rest.split_whitespace();
        let kind = parts.next().ok_or(unknown.clone())?;
        let mut number = || -> Result<u64, BindingRefusal> {
            parts
                .next()
                .and_then(|part| part.parse::<u64>().ok())
                .ok_or(unknown.clone())
        };
        let slot = usize::try_from(number()?).map_err(|_| unknown.clone())?;
        let channel = u32::try_from(number()?).map_err(|_| unknown.clone())?;
        let value = number().ok();
        if slot >= self.slots.len() || channel as usize >= self.channels.len() {
            return Err(unknown);
        }
        let task = self.task_ordinal(slot)?;
        self.note_task_mark(task.0)?;
        let c = ChannelOrdinal(channel);
        let pending = |driver: &Self, message: Option<u64>| -> Result<u64, BindingRefusal> {
            let message = message.ok_or(unknown.clone())?;
            if driver.pending_sends.get(&slot) == Some(&(channel, message)) {
                Ok(message)
            } else {
                Err(unknown.clone())
            }
        };
        let event = match kind {
            "send-blocked" => {
                let message = pending(self, value)?;
                self.channel_blocked.insert(slot);
                self.op_channel.push("send");
                ChannelEvent::SendBlocked {
                    channel: c,
                    message: MessageOrdinal(message),
                    sender: task,
                }
            }
            "send-closed" | "send-abandoned" => {
                let message = pending(self, value)?;
                self.pending_sends.remove(&slot);
                self.channel_blocked.remove(&slot);
                self.op_channel.push("send");
                let message = MessageOrdinal(message);
                if kind == "send-closed" {
                    ChannelEvent::SendClosed {
                        channel: c,
                        message,
                        sender: task,
                    }
                } else {
                    ChannelEvent::SendAbandoned {
                        channel: c,
                        message,
                        sender: task,
                    }
                }
            }
            "recv" => {
                let message = value.ok_or(unknown)?;
                let queue = &mut self.channels[channel as usize].queue;
                if queue.front() != Some(&message) {
                    return Err(BindingRefusal::SubstrateChannelDisagrees {
                        detail: format!(
                            "c{channel} delivered m{message}, the oldest sent is {:?}",
                            queue.front()
                        ),
                    });
                }
                queue.pop_front();
                self.channel_blocked.remove(&slot);
                self.op_channel.push("recv");
                ChannelEvent::Received {
                    channel: c,
                    message: MessageOrdinal(message),
                }
            }
            "recv-blocked" => {
                self.channel_blocked.insert(slot);
                self.op_channel.push("recv");
                ChannelEvent::RecvBlocked { channel: c }
            }
            "recv-closed" => {
                self.channel_blocked.remove(&slot);
                self.op_channel.push("recv");
                ChannelEvent::RecvClosed { channel: c }
            }
            "recv-abandoned" => {
                self.channel_blocked.remove(&slot);
                ChannelEvent::RecvAbandoned { channel: c }
            }
            "receiver-gone" => {
                let length = value.ok_or(unknown)?;
                let queue = core::mem::take(&mut self.channels[channel as usize].queue);
                if queue.len() as u64 != length {
                    return Err(BindingRefusal::SubstrateChannelDisagrees {
                        detail: format!(
                            "c{channel}'s receiver dropped {length} messages, the trace queued {}",
                            queue.len()
                        ),
                    });
                }
                ChannelEvent::ReceiverGone {
                    channel: c,
                    discarded: MessageSet::new(queue.into_iter().map(MessageOrdinal)),
                }
            }
            _ => return Err(unknown),
        };
        self.channel_event(task, event);
        Ok(())
    }

    /// Advance the virtual clock, let the due timers fire, and journal what the woken
    /// tasks did in canonical order.
    fn advance(&mut self, nanos: u64) -> Result<(), BindingRefusal> {
        let from = self.lab.now().as_nanos();
        self.lab.advance_time(nanos);
        let to = self.lab.now().as_nanos();
        // The timers this advance makes due, by task: the key of a task its due timer
        // woke. A task whose deadline has also passed observes its cancellation at the
        // checkpoint before its `Sleep` is polled, so no fire is traced for it: the
        // timer is dropped instead (bn-36wy3). It was still that timer that woke it.
        let due: BTreeMap<u32, (u64, u32)> = self
            .timers
            .values()
            .filter(|track| {
                track.deadline <= to && self.sleeping.get(&track.slot) == Some(&track.ordinal)
            })
            .map(|track| (track.task.0, (track.deadline, track.ordinal)))
            .collect();
        if self.observes_time() {
            self.append_body(EventBody::Time(TimeEvent::Advanced {
                from: VirtualInstant(from),
                to: VirtualInstant(to),
            }));
        }
        self.advancing = Some(Advance::default());
        if let Some(timers) = self.lab.state.timer_driver_handle() {
            let _woken = timers.process_timers();
        }
        let result = self.run();
        let batch = self.advancing.take().unwrap_or_default();
        result?;
        if let Some(kind) = batch.unordered {
            return Err(BindingRefusal::UnorderedDuringAdvance { kind });
        }
        let mut order: Vec<(u64, u32, u32)> = Vec::new();
        for task in batch.per_task.keys() {
            let (deadline, timer) = batch
                .keys
                .get(task)
                .copied()
                .or_else(|| due.get(task).copied())
                .ok_or(BindingRefusal::UnorderedDuringAdvance {
                    kind: "a step of a task no timer woke".to_owned(),
                })?;
            order.push((deadline, timer, *task));
        }
        order.sort_unstable();
        let mut per_task = batch.per_task;
        for (_, _, task) in order {
            for body in per_task.remove(&task).unwrap_or_default() {
                self.record.append(body);
            }
        }
        Ok(())
    }

    /// Journal an event of a woken task: into the advance's batch when one is open.
    fn append_task_event(&mut self, task: TaskOrdinal, body: EventBody) {
        if let Some(batch) = &mut self.advancing {
            batch.per_task.entry(task.0).or_default().push(body);
        } else if (self.in_cancel && !self.acked.contains(&task.0))
            || (!self.in_cancel && self.op_subject != Some(task.0))
        {
            // A task the operation woke: journaled after the operation, with the other
            // woken tasks, in canonical order (flush_wakes).
            self.wake_batch.entry(task.0).or_default().push(body);
        } else {
            self.append_body(body);
        }
    }

    /// Journal what the operation's woken tasks did, one task at a time, by ascending
    /// task ordinal, after everything the operation itself did.
    ///
    /// A task wakes only after the operation's own task has parked or ended, so its
    /// steps follow the operation's. Several tasks woken together (a receiver that goes
    /// wakes every sender blocked on its channel) run in an order the substrate's
    /// scheduler picks. They are independent only while none of them sends or receives
    /// a message or moves an obligation, so several woken tasks that do are refused. A
    /// cancellation's woken tasks (senders a cancelled receiver's drop woke) follow the
    /// cancellation's whole batch under the same condition.
    fn flush_wakes(&mut self) -> Result<(), BindingRefusal> {
        let batch = core::mem::take(&mut self.wake_batch);
        // Tasks a cancellation woke outside what it cancels follow its whole batch, so
        // they must be as independent of it as several woken tasks are of each other.
        if batch.len() > 1 || (self.in_cancel && !batch.is_empty()) {
            for (task, bodies) in &batch {
                let independent = bodies.iter().all(|body| {
                    matches!(
                        body,
                        EventBody::Lifecycle(LifecycleEvent::TaskStepped { .. })
                            | EventBody::Channel(
                                ChannelEvent::SendClosed { .. } | ChannelEvent::RecvClosed { .. }
                            )
                    )
                });
                if !independent {
                    return Err(BindingRefusal::UnorderedWake { task: *task });
                }
            }
        }
        self.flush_leaks();
        self.flush_closes();
        for (_, bodies) in batch {
            for body in bodies {
                self.record.append(body);
            }
        }
        Ok(())
    }

    /// Send an effect command, then confirm from the trace that the substrate took the
    /// step. The gate's own report of a substrate refusal comes first.
    fn effect_command(
        &mut self,
        slot: usize,
        command: Command,
        operation: &'static str,
    ) -> Result<(), BindingRefusal> {
        self.op_reserved.clear();
        self.op_resolved.clear();
        self.op_transferred.clear();
        self.command_slot(slot, command)?;
        if let Some(detail) = lock(&self.slots[slot].gate).refused.take() {
            return Err(BindingRefusal::SubstrateRefused { operation, detail });
        }
        Ok(())
    }

    /// Every region, children before parents and siblings by ascending ordinal: the
    /// canonical order for closes the substrate completes in one batch.
    fn post_order(&self) -> Vec<u32> {
        let mut children: Vec<Vec<u32>> = vec![Vec::new(); self.parents.len()];
        for (region, parent) in self.parents.iter().enumerate() {
            if let Some(parent) = parent {
                children[*parent as usize].push(ordinal(region));
            }
        }
        let mut out = Vec::with_capacity(self.parents.len());
        // Iterative post-order from the root; `children` is ascending by construction.
        let mut stack: Vec<(u32, usize)> = vec![(0, 0)];
        while let Some((region, next)) = stack.pop() {
            if let Some(child) = children[region as usize].get(next) {
                stack.push((region, next + 1));
                stack.push((*child, 0));
            } else {
                out.push(region);
            }
        }
        out
    }

    /// Journal a batch of region closes in canonical order: each close's cancellation
    /// phases, then its drain and finalize.
    ///
    /// The substrate completes a cancelled subtree's closes in an order its seeded
    /// scheduler picks, constrained only by children before parents. Post-order with
    /// siblings by ascending ordinal is one linearization of that partial order, so the
    /// lift still sees every child drained before its parent.
    /// Journal a batch of leaks, by ascending obligation ordinal.
    ///
    /// The substrate detects every obligation a task still holds when it completes, and
    /// traces the leaks in its own order. They are one completion's facts, so the batch
    /// ends at the next event that journals anything.
    fn flush_leaks(&mut self) {
        for obligation in core::mem::take(&mut self.pending_leaks) {
            self.record
                .append(EventBody::Obligation(ObligationEvent::Leaked {
                    obligation: ObligationOrdinal(obligation),
                }));
        }
    }

    fn flush_closes(&mut self) {
        self.flush_leaks();
        if self.pending_closes.is_empty() {
            return;
        }
        let batch: BTreeSet<u32> = core::mem::take(&mut self.pending_closes)
            .into_iter()
            .map(|region| region.0)
            .collect();
        for region in self.post_order() {
            if !batch.contains(&region) {
                continue;
            }
            let (drained, kept): (Vec<_>, Vec<_>) = self
                .pending_cancelled
                .iter()
                .partition(|(_, owner)| self.in_subtree(*owner, region));
            self.pending_cancelled = kept;
            let cancelled = TaskSet::new(drained.into_iter().map(|(task, _)| task));
            self.journal_phases(&cancelled);
            let region = RegionOrdinal(region);
            self.record
                .append(EventBody::Lifecycle(LifecycleEvent::RegionDrained {
                    region,
                    cancelled,
                }));
            self.record
                .append(EventBody::Lifecycle(LifecycleEvent::RegionFinalized {
                    region,
                }));
            if self.observes_obligation() {
                let settled = self.settled(region.0);
                self.record.append(EventBody::Obligation(settled));
            }
        }
    }

    fn command(&mut self, label: TaskLabel, command: Command) -> Result<(), BindingRefusal> {
        let slot = self.slot(label)?;
        self.command_slot(slot, command)
    }

    fn command_slot(&mut self, slot: usize, command: Command) -> Result<(), BindingRefusal> {
        // A command to a task that is not polling its gate would wait unseen: before
        // `begin`, after the task ended, or while it sleeps.
        if !self.begun.contains(&slot) {
            return Err(BindingRefusal::TaskNotBegun {
                task: self.task_ordinal(slot)?.0,
            });
        }
        if self.ended.contains(&slot) {
            return Err(BindingRefusal::TaskEnded {
                task: self.task_ordinal(slot)?.0,
            });
        }
        if self.sleeping.contains_key(&slot) {
            return Err(BindingRefusal::TaskAsleep {
                task: self.task_ordinal(slot)?.0,
            });
        }
        if self.channel_blocked.contains(&slot) {
            return Err(BindingRefusal::TaskBlocked {
                task: self.task_ordinal(slot)?.0,
            });
        }
        self.op_subject = Some(self.task_ordinal(slot)?.0);
        let waker = {
            let mut state = lock(&self.slots[slot].gate);
            state.commands.push_back(command);
            state.waker.take()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
        self.run()
    }

    fn apply(&mut self, op: &SubstrateOp) -> Result<(), BindingRefusal> {
        self.op_subject = None;
        self.op_woken.clear();
        self.in_cancel = false;
        self.op_channel.clear();
        let refused = |detail: String| BindingRefusal::SubstrateRefused {
            operation: op.name(),
            detail,
        };
        match op {
            SubstrateOp::OpenRegion { parent, child } => {
                let parent = self.region(*parent)?;
                if self.region_labels.contains_key(child) {
                    return Err(BindingRefusal::RegionLabelRebound(child.0));
                }
                let region = self
                    .lab
                    .state
                    .create_child_region(parent, Budget::INFINITE)
                    .map_err(|error| refused(format!("{error:?}")))?;
                self.region_labels.insert(*child, region);
                self.sync()
            }
            SubstrateOp::Spawn {
                region,
                task,
                resumability,
            } => self.spawn(*region, *task, resumability, None),
            SubstrateOp::SpawnWithDeadline {
                region,
                task,
                resumability,
                deadline,
            } => {
                let now = self.lab.now().as_nanos();
                if *deadline <= now {
                    return Err(BindingRefusal::DeadlineNotAhead {
                        deadline: *deadline,
                        now,
                    });
                }
                self.spawn(*region, *task, resumability, Some(*deadline))
            }
            SubstrateOp::Begin { task } => {
                let slot = self.slot(*task)?;
                if !self.begun.insert(slot) {
                    return Err(BindingRefusal::TaskAlreadyBegun {
                        task: self.task_ordinal(slot)?.0,
                    });
                }
                self.op_subject = Some(self.task_ordinal(slot)?.0);
                let id = self.slots[slot].id;
                self.lab
                    .scheduler
                    .lock()
                    .schedule(id, Budget::INFINITE.priority);
                self.run()
            }
            SubstrateOp::Continue { task } => self.command(*task, Command::Park),
            SubstrateOp::Finish { task } => self.command(*task, Command::Finish),
            SubstrateOp::Close { region } => {
                let id = self.region(*region)?;
                let ordinal = self.ordinal_of(id)?;
                let began = self
                    .lab
                    .state
                    .region(id)
                    .ok_or_else(|| refused("the region record is gone".to_owned()))?
                    .begin_close(None);
                if !began {
                    return Err(refused("begin_close returned false".to_owned()));
                }
                self.append(LifecycleEvent::RegionCloseRequested { region: ordinal });
                self.lab.state.advance_region_state(id);
                self.run()
            }
            SubstrateOp::Cancel { region } => {
                self.in_cancel = true;
                let id = self.region(*region)?;
                if self.lab.state.region(id).is_none() {
                    return Err(refused("the region record is gone".to_owned()));
                }
                let effects = self.lab.state.cancel_request(
                    id,
                    &CancelReason::user("continuum lifecycle binding"),
                    None,
                );
                let (targets, wakes) = effects.into_parts();
                {
                    let mut scheduler = self.lab.scheduler.lock();
                    for (task, priority) in targets {
                        scheduler.schedule_cancel(task, priority);
                    }
                }
                wakes.dispatch();
                self.run()
            }
            SubstrateOp::Reserve { task, reservation } => {
                self.acquire(*task, *reservation, RESERVATION_KIND)
            }
            SubstrateOp::Acquire {
                task,
                reservation,
                kind,
            } => self.acquire(*task, *reservation, substrate_kind(*kind)),
            SubstrateOp::Commit { reservation } | SubstrateOp::Abort { reservation } => {
                let (held, _) = self.bound(*reservation)?;
                let (command, operation) = match op {
                    SubstrateOp::Commit { .. } => (Command::Commit(*reservation), "commit"),
                    _ => (Command::Abort(*reservation), "abort"),
                };
                self.effect_command(held.slot, command, operation)?;
                let resolved = self
                    .reservation_labels
                    .get(reservation)
                    .is_some_and(|state| state.resolved);
                if self.op_resolved.len() == 1 && resolved {
                    Ok(())
                } else {
                    Err(BindingRefusal::EffectUnobserved { operation })
                }
            }
            SubstrateOp::OpenChannel {
                channel,
                capacity,
                receiver,
            } => {
                if self.channel_labels.contains_key(channel) {
                    return Err(BindingRefusal::ChannelLabelRebound(channel.0));
                }
                if *capacity == 0 {
                    return Err(BindingRefusal::ZeroCapacity(channel.0));
                }
                let slot = self.slot(*receiver)?;
                let task = self.task_ordinal(slot)?;
                if self.ended.contains(&slot) {
                    return Err(BindingRefusal::TaskEnded { task: task.0 });
                }
                let (sender, receiving) = mpsc::channel::<u64>(*capacity as usize);
                let ordinal = ordinal(self.channels.len());
                lock(&self.slots[slot].gate)
                    .receiver_inbox
                    .push((ordinal, receiving));
                self.channels.push(ChannelTrack {
                    receiver_slot: slot,
                    sender: Some(sender),
                    queue: VecDeque::new(),
                });
                self.channel_labels.insert(*channel, ordinal);
                if self.observes_channel() {
                    self.append_body(EventBody::Channel(ChannelEvent::Opened {
                        channel: ChannelOrdinal(ordinal),
                        capacity: *capacity,
                        receiver: task,
                    }));
                }
                Ok(())
            }
            SubstrateOp::Send { task, channel } => {
                let c = self.channel_of(*channel)?;
                let sender = self.channels[c as usize]
                    .sender
                    .clone()
                    .ok_or(BindingRefusal::SendersAlreadyClosed(channel.0))?;
                let slot = self.slot(*task)?;
                let message = self.next_message;
                self.next_message = self.next_message.saturating_add(1);
                self.pending_sends.insert(slot, (c, message));
                self.command_slot(
                    slot,
                    Command::Send {
                        channel: c,
                        message,
                        sender,
                    },
                )?;
                if let Some(detail) = lock(&self.slots[slot].gate).refused.take() {
                    return Err(BindingRefusal::SubstrateRefused {
                        operation: "send",
                        detail,
                    });
                }
                if self.op_channel.contains(&"send") {
                    Ok(())
                } else {
                    Err(BindingRefusal::EffectUnobserved { operation: "send" })
                }
            }
            SubstrateOp::Recv { channel } => {
                let c = self.channel_of(*channel)?;
                let slot = self.channels[c as usize].receiver_slot;
                self.command_slot(slot, Command::Recv(c))?;
                if let Some(detail) = lock(&self.slots[slot].gate).refused.take() {
                    return Err(BindingRefusal::SubstrateRefused {
                        operation: "recv",
                        detail,
                    });
                }
                if self.op_channel.contains(&"recv") {
                    Ok(())
                } else {
                    Err(BindingRefusal::EffectUnobserved { operation: "recv" })
                }
            }
            SubstrateOp::CloseSenders { channel } => {
                let c = self.channel_of(*channel)?;
                let sender = self.channels[c as usize]
                    .sender
                    .take()
                    .ok_or(BindingRefusal::SendersAlreadyClosed(channel.0))?;
                if self.observes_channel() {
                    self.append_body(EventBody::Channel(ChannelEvent::SendersClosed {
                        channel: ChannelOrdinal(c),
                    }));
                }
                // Dropping the last sender wakes a waiting receiver, which then runs.
                drop(sender);
                self.run()
            }
            SubstrateOp::Sleep { task, nanos } => {
                if *nanos == 0 {
                    return Err(BindingRefusal::ZeroDuration { operation: "sleep" });
                }
                let slot = self.slot(*task)?;
                self.op_scheduled.clear();
                self.sleep_slot = Some(slot);
                let result = self.command_slot(slot, Command::Sleep(*nanos));
                self.sleep_slot = None;
                result?;
                if let Some(detail) = lock(&self.slots[slot].gate).refused.take() {
                    return Err(BindingRefusal::SubstrateRefused {
                        operation: "sleep",
                        detail,
                    });
                }
                match self.op_scheduled.as_slice() {
                    [(_, holder)] if *holder == slot => Ok(()),
                    _ => Err(BindingRefusal::EffectUnobserved { operation: "sleep" }),
                }
            }
            SubstrateOp::Advance { nanos } => {
                if *nanos == 0 {
                    return Err(BindingRefusal::ZeroDuration {
                        operation: "advance",
                    });
                }
                self.advance(*nanos)
            }
            SubstrateOp::Transfer { reservation, to } => {
                let (held, id) = self.bound(*reservation)?;
                if held.kind == RESERVATION_KIND {
                    return Err(BindingRefusal::EffectNotTransferable(reservation.0));
                }
                let destination = self.slot(*to)?;
                let cx = self
                    .lab
                    .state
                    .task(self.slots[destination].id)
                    .and_then(|record| record.cx.clone())
                    .ok_or_else(|| refused("the destination task has no Cx".to_owned()))?;
                let inbox = Arc::clone(&self.slots[destination].gate);
                self.pending_transfer = Some((id, destination));
                let result = self.effect_command(
                    held.slot,
                    Command::Transfer(*reservation, Box::new(cx), inbox),
                    "transfer",
                );
                self.pending_transfer = None;
                result?;
                if self.op_transferred.len() == 1 {
                    Ok(())
                } else {
                    Err(BindingRefusal::EffectUnobserved {
                        operation: "transfer",
                    })
                }
            }
        }
    }

    /// Create a gated task in `region`, bound to `task`, with a finite budget deadline
    /// when `deadline` names one.
    fn spawn(
        &mut self,
        region: RegionLabel,
        task: TaskLabel,
        resumability: &Resumability,
        deadline: Option<u64>,
    ) -> Result<(), BindingRefusal> {
        let operation = if deadline.is_some() {
            "spawn-with-deadline"
        } else {
            "spawn"
        };
        let refused = |detail: String| BindingRefusal::SubstrateRefused { operation, detail };
        let region = self.region(region)?;
        if self.task_labels.contains_key(&task) {
            return Err(BindingRefusal::TaskLabelRebound(task.0));
        }
        let slot = self.slots.len();
        let gate: Gate = Arc::new(Mutex::new(GateState {
            phase: GatePhase::Created,
            commands: VecDeque::new(),
            waker: None,
            untraced: false,
            held: 0,
            refused: None,
            settling: false,
            inbox: Vec::new(),
            receiver_inbox: Vec::new(),
        }));
        let budget = deadline.map_or(Budget::INFINITE, |at| {
            Budget::INFINITE.with_deadline(Time::from_nanos(at))
        });
        let (id, handle) = self
            .lab
            .state
            .create_task(region, budget, gated_body(Arc::clone(&gate), slot))
            .map_err(|error| refused(format!("{error:?}")))?;
        if deadline.is_some() {
            // A checkpoint reads the task's own timer driver, and falls back to the host
            // clock without one: a deadline is bound only on the lab's virtual clock.
            let virtual_clock = self
                .lab
                .state
                .task(id)
                .and_then(|record| record.cx.clone())
                .is_some_and(|cx| cx.timer_driver().is_some());
            if !virtual_clock {
                return Err(refused(
                    "the task's Cx carries no virtual timer driver".to_owned(),
                ));
            }
        }
        self.slots.push(Slot {
            id,
            handle,
            gate,
            resumability: resumability.clone(),
            ordinal: None,
            deadline,
        });
        self.slot_of.insert(id, slot);
        self.task_labels.insert(task, slot);
        self.sync()
    }

    /// Acquire an obligation of `kind` for `task`, bound to `label`.
    fn acquire(
        &mut self,
        task: TaskLabel,
        label: ReservationLabel,
        kind: SubstrateObligationKind,
    ) -> Result<(), BindingRefusal> {
        let slot = self.slot(task)?;
        if self.reservation_labels.contains_key(&label) {
            return Err(BindingRefusal::ReservationLabelRebound(label.0));
        }
        let operation = if kind == RESERVATION_KIND {
            "reserve"
        } else {
            "acquire"
        };
        self.effect_command(slot, Command::Reserve(label, kind), operation)?;
        match self.op_reserved.as_slice() {
            [(obligation, holder)] if *holder == slot => {
                self.reservation_owner_label.insert(*obligation, label);
                self.reservation_labels.insert(
                    label,
                    LabelState {
                        obligation: *obligation,
                        resolved: false,
                    },
                );
                Ok(())
            }
            _ => Err(BindingRefusal::EffectUnobserved { operation }),
        }
    }

    /// The obligation a label is bound to, when it is bound and still unresolved.
    fn bound(&self, label: ReservationLabel) -> Result<(Held, ObligationId), BindingRefusal> {
        let state = *self
            .reservation_labels
            .get(&label)
            .ok_or(BindingRefusal::UnboundReservation(label.0))?;
        if state.resolved {
            return Err(BindingRefusal::ReservationResolved(label.0));
        }
        self.reservations
            .iter()
            .find(|(_, held)| held.obligation == state.obligation)
            .map(|(id, held)| (*held, *id))
            .ok_or(BindingRefusal::UnknownSubstrateEntity { what: "obligation" })
    }

    /// The reservation a traced obligation is, as the run allocated it.
    fn held(&self, obligation: ObligationId) -> Result<Held, BindingRefusal> {
        self.reservations
            .get(&obligation)
            .copied()
            .ok_or(BindingRefusal::UnknownSubstrateEntity { what: "obligation" })
    }

    /// Record that an obligation resolved, for the label bookkeeping and the ledger.
    fn resolve(&mut self, id: ObligationId, standing: Standing) {
        let Some(held) = self.reservations.get_mut(&id) else {
            return;
        };
        held.standing = standing;
        let ordinal = held.obligation;
        self.op_resolved.push(ordinal);
        if let Some(label) = self.reservation_owner_label.get(&ordinal)
            && let Some(state) = self.reservation_labels.get_mut(label)
        {
            state.resolved = true;
        }
    }

    fn run(&mut self) -> Result<(), BindingRefusal> {
        self.lab.run_until_idle();
        if !self.lab.scheduler.lock().is_empty() {
            return Err(BindingRefusal::StepLimit);
        }
        self.sync()?;
        // The substrate's own protocol oracle, whatever the run observes: a projection
        // must not turn its finding into a journal (cr-3pu5cu).
        self.lab
            .check_cancellation_protocol()
            .map_err(|violation| BindingRefusal::SubstrateProtocolViolation {
                detail: violation.to_string(),
            })?;
        Ok(())
    }

    const fn observes_cancellation(&self) -> bool {
        self.config.families.contains(Family::Cancellation)
    }

    /// Journal the requests one operation issued, by ascending task ordinal.
    fn flush_requests(&mut self) {
        if self.pending_requests.is_empty() {
            return;
        }
        let mut requests = core::mem::take(&mut self.pending_requests);
        requests.sort_unstable();
        for (task, cause) in requests {
            self.record
                .append(EventBody::Cancellation(CancellationEvent::Requested {
                    task,
                    cause,
                }));
        }
    }

    fn ordinal_of(&self, region: SubstrateRegion) -> Result<RegionOrdinal, BindingRefusal> {
        self.region_ordinals
            .get(&region)
            .copied()
            .ok_or(BindingRefusal::UnknownSubstrateEntity { what: "region" })
    }

    fn slot_of(&self, task: SubstrateTask) -> Result<usize, BindingRefusal> {
        self.slot_of
            .get(&task)
            .copied()
            .ok_or(BindingRefusal::UnknownSubstrateEntity { what: "task" })
    }

    fn task_ordinal(&self, slot: usize) -> Result<TaskOrdinal, BindingRefusal> {
        self.slots[slot]
            .ordinal
            .ok_or(BindingRefusal::UnknownSubstrateEntity {
                what: "task before its spawn",
            })
    }

    /// Whether `region` or one of its ancestors is already cancel-requested.
    fn under_cancel(&self, region: u32) -> bool {
        let mut at = Some(region);
        while let Some(current) = at {
            if self.cancelled.contains(&current) {
                return true;
            }
            at = self.parents.get(current as usize).copied().flatten();
        }
        false
    }

    fn in_subtree(&self, member: u32, root: u32) -> bool {
        let mut at = Some(member);
        while let Some(current) = at {
            if current == root {
                return true;
            }
            at = self.parents.get(current as usize).copied().flatten();
        }
        false
    }

    /// Observe every trace event not yet observed, in sequence order.
    fn sync(&mut self) -> Result<(), BindingRefusal> {
        let trace = self.lab.trace();
        let pushed = trace.total_pushed();
        if pushed > u64::try_from(self.config.trace_capacity).unwrap_or(u64::MAX) {
            return Err(BindingRefusal::TraceOverflow {
                pushed,
                capacity: self.config.trace_capacity,
            });
        }
        for event in trace.snapshot() {
            if self.observed.contains(&event.seq) {
                continue;
            }
            if self.last_seq.is_some_and(|last| event.seq < last) {
                return Err(BindingRefusal::TraceOutOfOrder { seq: event.seq });
            }
            self.observed.insert(event.seq);
            self.last_seq = Some(event.seq);
            self.observe(&event)?;
            if self.record.is_full() {
                return Err(BindingRefusal::JournalFull);
            }
        }
        self.flush_requests();
        self.flush_leaks();
        self.flush_closes();
        if self.record.is_full() {
            return Err(BindingRefusal::JournalFull);
        }
        if self.slots.iter().any(|slot| lock(&slot.gate).untraced) {
            return Err(BindingRefusal::GateUntraced);
        }
        Ok(())
    }

    fn observe(&mut self, event: &TraceEvent) -> Result<(), BindingRefusal> {
        // An operation's requests are issued together, before any poll: the first
        // event of another kind ends them.
        if !matches!(
            event.kind,
            TraceEventKind::RegionCancelled
                | TraceEventKind::RegionCloseBegin
                | TraceEventKind::CancelRequest
        ) {
            self.flush_requests();
        }
        match (&event.kind, &event.data) {
            (TraceEventKind::RegionCreated, TraceData::Region { region, parent }) => match parent {
                None if *region == self.root && self.region_ordinals.is_empty() => {
                    self.region_ordinals.insert(*region, RegionOrdinal(0));
                    self.parents.push(None);
                }
                None => {
                    return Err(BindingRefusal::UnknownSubstrateEntity {
                        what: "second root region",
                    });
                }
                Some(parent) => {
                    let parent = self.ordinal_of(*parent)?;
                    let new = RegionOrdinal(ordinal(self.parents.len()));
                    self.region_ordinals.insert(*region, new);
                    self.parents.push(Some(parent.0));
                    self.append(LifecycleEvent::RegionOpened {
                        region: new,
                        parent,
                    });
                }
            },
            (TraceEventKind::Spawn, TraceData::Task { task, region }) => {
                let slot = self.slot_of(*task)?;
                let region = self.ordinal_of(*region)?;
                let task = TaskOrdinal(self.next_task);
                self.next_task = self.next_task.saturating_add(1);
                self.slots[slot].ordinal = Some(task);
                self.append(LifecycleEvent::TaskSpawned {
                    task,
                    region,
                    resumability: self.slots[slot].resumability.clone(),
                });
                if let Some(deadline) = self.slots[slot].deadline
                    && self.observes_time()
                {
                    self.append_body(EventBody::Time(TimeEvent::Deadline {
                        task,
                        at: VirtualInstant(deadline),
                    }));
                }
            }
            (TraceEventKind::UserTrace, TraceData::Message(message)) => {
                // Only the gate writes user traces in a bound run. Any other message is
                // an event this binding cannot place, so it is refused, not dropped.
                if let Some(rest) = message.strip_prefix(CANCEL_MARK) {
                    return self.observe_ack(rest);
                }
                if message.starts_with(HANDOFF_MARK) {
                    return self.observe_handoff();
                }
                if let Some(rest) = message.strip_prefix(CHANNEL_MARK) {
                    return self.observe_channel(rest);
                }
                let Some(rest) = message.strip_prefix(GATE_MARK) else {
                    return Err(BindingRefusal::UninstrumentedEvent {
                        family: None,
                        kind: "UserTrace".to_owned(),
                    });
                };
                let mut parts = rest.split_whitespace();
                let step = match parts.next() {
                    Some("begin") => TaskStep::Begin,
                    Some("suspend") => TaskStep::Suspend,
                    Some("resume") => TaskStep::Resume,
                    _ => {
                        return Err(BindingRefusal::UnknownSubstrateEntity { what: "gate mark" });
                    }
                };
                let slot = parts
                    .next()
                    .and_then(|slot| slot.parse::<usize>().ok())
                    .filter(|slot| *slot < self.slots.len())
                    .ok_or(BindingRefusal::UnknownSubstrateEntity { what: "gate" })?;
                let task = self.task_ordinal(slot)?;
                self.note_task_mark(task.0)?;
                self.append_task_event(
                    task,
                    EventBody::Lifecycle(LifecycleEvent::TaskStepped { task, step }),
                );
            }
            (TraceEventKind::Complete, TraceData::Task { task, region }) => {
                let slot = self.slot_of(*task)?;
                self.ended.insert(slot);
                let region = self.ordinal_of(*region)?;
                let ordinal = self.task_ordinal(slot)?;
                match self.slots[slot].handle.try_join() {
                    Ok(Some(())) => self.append(LifecycleEvent::TaskStepped {
                        task: ordinal,
                        step: TaskStep::Complete,
                    }),
                    Err(JoinError::Cancelled(reason)) if reason.kind == CancelKind::Deadline => {
                        self.single_cancel(*task, ordinal)?;
                    }
                    Err(JoinError::Cancelled(reason)) => {
                        self.pending_cancelled.push((ordinal, region.0));
                        // The cause must map and the substrate's oracle must confirm the
                        // completion whatever the run observes; only the phase record
                        // is the cancellation family's (cr-3pu5cu).
                        let cause = cause_of(reason.kind)?;
                        self.confirm_cancelled(*task, ordinal)?;
                        if self.observes_cancellation() {
                            self.phases.entry(ordinal.0).or_default().completed = Some(cause);
                        }
                    }
                    Err(JoinError::Panicked(_)) => {
                        return Err(BindingRefusal::TaskPanicked { task: ordinal.0 });
                    }
                    Ok(None) | Err(JoinError::PolledAfterCompletion) => {
                        return Err(BindingRefusal::TaskOutcomeUnobserved { task: ordinal.0 });
                    }
                }
            }
            (TraceEventKind::RegionCancelled, TraceData::RegionCancel { region, .. }) => {
                let region = self.ordinal_of(*region)?;
                if !self.under_cancel(region.0) {
                    self.cancelled.insert(region.0);
                    self.append(LifecycleEvent::RegionCancelRequested { region });
                }
            }
            (TraceEventKind::RegionCloseComplete, TraceData::Region { region, .. }) => {
                // Buffered: the batch is journaled in canonical order (flush_closes).
                let region = self.ordinal_of(*region)?;
                self.close_order.push(region);
                self.pending_closes.push(region);
            }
            (
                TraceEventKind::CancelRequest,
                TraceData::Cancel {
                    task,
                    region: _,
                    reason,
                },
            ) => {
                // Substrate facts first, whatever the run observes: the request's cause
                // must map, and the task's region request decides a later refusal.
                let ordinal = self.task_ordinal(self.slot_of(*task)?)?;
                let cause = cause_of(reason.kind)?;
                self.region_requested.insert(ordinal.0);
                if self.observes_cancellation() {
                    // Requests start a new batch: closes already pending came first.
                    self.flush_closes();
                    self.pending_requests.push((ordinal, cause));
                    self.phases.entry(ordinal.0).or_default();
                }
            }
            (TraceEventKind::RegionCloseBegin, _) => {}
            (
                TraceEventKind::TimerScheduled,
                TraceData::Timer {
                    timer_id,
                    deadline: Some(deadline),
                },
            ) => {
                let slot = self
                    .sleep_slot
                    .ok_or(BindingRefusal::UnknownSubstrateEntity { what: "timer" })?;
                let task = self.task_ordinal(slot)?;
                let ordinal = self.next_timer;
                self.next_timer = self.next_timer.saturating_add(1);
                self.timers.insert(
                    *timer_id,
                    TimerTrack {
                        ordinal,
                        slot,
                        task,
                        deadline: deadline.as_nanos(),
                    },
                );
                self.sleeping.insert(slot, ordinal);
                self.op_scheduled.push((ordinal, slot));
                if self.observes_time() {
                    self.append_body(EventBody::Time(TimeEvent::Scheduled {
                        timer: TimerOrdinal(ordinal),
                        task,
                        at: VirtualInstant(event.time.as_nanos()),
                        deadline: VirtualInstant(deadline.as_nanos()),
                    }));
                }
            }
            (TraceEventKind::TimerFired, TraceData::Timer { timer_id, .. }) => {
                let track = self.timer(*timer_id)?;
                self.sleeping.remove(&track.slot);
                self.fire_order.push(TimerOrdinal(track.ordinal));
                let Some(batch) = &mut self.advancing else {
                    return Err(BindingRefusal::UnknownSubstrateEntity {
                        what: "timer fire outside an advance",
                    });
                };
                batch
                    .keys
                    .insert(track.task.0, (track.deadline, track.ordinal));
                if self.observes_time() {
                    self.append_task_event(
                        track.task,
                        EventBody::Time(TimeEvent::Fired {
                            timer: TimerOrdinal(track.ordinal),
                            at: VirtualInstant(event.time.as_nanos()),
                        }),
                    );
                }
            }
            (TraceEventKind::TimerCancelled, TraceData::Timer { timer_id, .. }) => {
                let track = self.timer(*timer_id)?;
                self.sleeping.remove(&track.slot);
                if self.observes_time() {
                    // Buffered: journaled with the task's drain (journal_phases).
                    self.pending_timer_cancels
                        .entry(track.task.0)
                        .or_default()
                        .insert((track.ordinal, event.time.as_nanos()));
                }
            }
            (
                TraceEventKind::ObligationReserve,
                TraceData::Obligation {
                    obligation,
                    task,
                    region,
                    kind,
                    ..
                },
            ) => {
                let slot = self.slot_of(*task)?;
                let task = self.task_ordinal(slot)?;
                let region = self.ordinal_of(*region)?;
                let ordinal = self.next_obligation;
                self.next_obligation = self.next_obligation.saturating_add(1);
                let reservation = (*kind == RESERVATION_KIND).then(|| {
                    let reservation = self.next_reservation;
                    self.next_reservation = self.next_reservation.saturating_add(1);
                    reservation
                });
                self.reservations.insert(
                    *obligation,
                    Held {
                        obligation: ordinal,
                        reservation,
                        kind: *kind,
                        slot,
                        task,
                        region: region.0,
                        standing: Standing::Open,
                    },
                );
                self.op_reserved.push((ordinal, slot));
                if let Some(reservation) = reservation
                    && self.observes_effect()
                {
                    self.append_task_event(
                        task,
                        EventBody::Effect(EffectEvent::Reserved {
                            reservation: ReservationOrdinal(reservation),
                            task,
                        }),
                    );
                }
                if self.observes_obligation() {
                    self.append_task_event(
                        task,
                        EventBody::Obligation(ObligationEvent::Opened {
                            obligation: ObligationOrdinal(ordinal),
                            kind: journal_kind(*kind),
                            holder: task,
                            region,
                        }),
                    );
                }
            }
            (TraceEventKind::ObligationCommit, TraceData::Obligation { obligation, .. }) => {
                let held = self.held(*obligation)?;
                self.resolve(*obligation, Standing::Discharged);
                if let Some(reservation) = held.reservation
                    && self.observes_effect()
                {
                    self.append_task_event(
                        held.task,
                        EventBody::Effect(EffectEvent::Committed {
                            reservation: ReservationOrdinal(reservation),
                        }),
                    );
                }
                if self.observes_obligation() {
                    self.append_task_event(
                        held.task,
                        EventBody::Obligation(ObligationEvent::Discharged {
                            obligation: ObligationOrdinal(held.obligation),
                            how: Discharge::Committed,
                        }),
                    );
                }
                // A channel send's permit. (A `SendPermit` a program acquires directly is
                // only an obligation.)
                if held.kind == SubstrateObligationKind::SendPermit
                    && self.pending_sends.contains_key(&held.slot)
                {
                    self.observe_sent(held.slot, held.task)?;
                }
            }
            (
                TraceEventKind::ObligationAbort,
                TraceData::Obligation {
                    obligation,
                    abort_reason,
                    ..
                },
            ) => {
                let held = self.held(*obligation)?;
                self.resolve(*obligation, Standing::Discharged);
                if let Some(reservation) = held.reservation
                    && self.observes_effect()
                {
                    match abort_reason {
                        // Buffered: journaled with the task's drain (journal_phases).
                        Some(ObligationAbortReason::Cancel) => {
                            self.pending_aborts
                                .entry(held.task.0)
                                .or_default()
                                .insert(reservation);
                        }
                        Some(ObligationAbortReason::Explicit) => {
                            self.append_task_event(
                                held.task,
                                EventBody::Effect(EffectEvent::Aborted {
                                    reservation: ReservationOrdinal(reservation),
                                    cause: AbortCause::Explicit,
                                }),
                            );
                        }
                        Some(ObligationAbortReason::Error) | None => {
                            return Err(BindingRefusal::ReservationDropped { reservation });
                        }
                    }
                }
                if self.observes_obligation() {
                    if *abort_reason == Some(ObligationAbortReason::Cancel) {
                        // Buffered like the effect abort, for the same reason.
                        self.pending_discharges
                            .entry(held.task.0)
                            .or_default()
                            .insert(held.obligation);
                    } else {
                        self.append_task_event(
                            held.task,
                            EventBody::Obligation(ObligationEvent::Discharged {
                                obligation: ObligationOrdinal(held.obligation),
                                how: Discharge::Aborted,
                            }),
                        );
                    }
                }
            }
            (TraceEventKind::ObligationLeak, TraceData::Obligation { obligation, .. })
                if self.reservations.contains_key(obligation) =>
            {
                let held = self.held(*obligation)?;
                self.resolve(*obligation, Standing::Leaked);
                if let Some(reservation) = held.reservation
                    && self.observes_effect()
                {
                    return Err(BindingRefusal::ReservationDropped { reservation });
                }
                if self.observes_obligation() {
                    // Buffered: the leaks of one completion are journaled together, by
                    // obligation ordinal (flush_leaks).
                    self.leak_order.push(ObligationOrdinal(held.obligation));
                    self.pending_leaks.insert(held.obligation);
                } else {
                    // A leak is never dropped silently: only the obligations family can
                    // report it.
                    return Err(BindingRefusal::UninstrumentedEvent {
                        family: Some(Family::Obligation),
                        kind: "ObligationLeak".to_owned(),
                    });
                }
            }
            (kind, _) => {
                return Err(BindingRefusal::UninstrumentedEvent {
                    family: family_of(*kind),
                    kind: format!("{kind:?}"),
                });
            }
        }
        Ok(())
    }

    /// The timer a traced timer id is, as the run scheduled it.
    fn timer(&self, id: u64) -> Result<TimerTrack, BindingRefusal> {
        self.timers
            .get(&id)
            .copied()
            .ok_or(BindingRefusal::UnknownSubstrateEntity { what: "timer" })
    }

    /// The runtime's handoff message. Its fields are the runtime's own serialization, so
    /// the binding reads the handoff from the substrate's obligation record instead: it
    /// must be the transfer this operation asked for, and the record must now name the
    /// destination as holder.
    fn observe_handoff(&mut self) -> Result<(), BindingRefusal> {
        let (id, destination) = self
            .pending_transfer
            .ok_or(BindingRefusal::UnknownSubstrateEntity { what: "handoff" })?;
        let record = self
            .lab
            .state
            .obligation(id)
            .ok_or(BindingRefusal::UnknownSubstrateEntity { what: "obligation" })?;
        if record.holder != self.slots[destination].id {
            return Err(BindingRefusal::SubstrateLedgerDisagrees {
                detail: "the handoff's record does not name the destination".to_owned(),
            });
        }
        let region = self.ordinal_of(record.region)?;
        let task = self.task_ordinal(destination)?;
        let held = self
            .reservations
            .get_mut(&id)
            .ok_or(BindingRefusal::UnknownSubstrateEntity { what: "obligation" })?;
        held.slot = destination;
        held.task = task;
        held.region = region.0;
        let obligation = held.obligation;
        self.op_transferred.push(obligation);
        if self.observes_obligation() {
            self.append_body(EventBody::Obligation(ObligationEvent::Transferred {
                obligation: ObligationOrdinal(obligation),
                holder: task,
                region,
            }));
        }
        Ok(())
    }

    /// A region's balance at close, from the ledger the trace produced.
    fn settled(&self, region: u32) -> ObligationEvent {
        let members = |standing: Standing| {
            ObligationSet::new(
                self.reservations
                    .values()
                    .filter(|held| held.region == region && held.standing == standing)
                    .map(|held| ObligationOrdinal(held.obligation)),
            )
        };
        ObligationEvent::RegionSettled {
            region: RegionOrdinal(region),
            open: members(Standing::Open),
            leaked: members(Standing::Leaked),
        }
    }

    /// Hold the trace's ledger to the substrate's own: each obligation record it still
    /// keeps, and its obligation-leak oracle rebuilt from the runtime state.
    fn cross_check_ledger(&self) -> Result<(), BindingRefusal> {
        let disagree = |detail: String| BindingRefusal::SubstrateLedgerDisagrees { detail };
        let mut regions: BTreeMap<u32, SubstrateRegion> = BTreeMap::new();
        for (substrate, ordinal) in &self.region_ordinals {
            regions.insert(ordinal.0, *substrate);
        }
        for (id, held) in &self.reservations {
            let Some(record) = self.lab.state.obligation(*id) else {
                continue;
            };
            let standing = match record.state {
                ObligationState::Reserved => Standing::Open,
                ObligationState::Committed | ObligationState::Aborted => Standing::Discharged,
                ObligationState::Leaked => Standing::Leaked,
            };
            if standing != held.standing
                || record.kind != held.kind
                || record.holder != self.slots[held.slot].id
                || regions.get(&held.region) != Some(&record.region)
            {
                return Err(disagree(format!(
                    "o{} is {:?} held by {:?} in {:?}, but the trace says {:?} in r{}",
                    held.obligation,
                    record.state,
                    record.holder,
                    record.region,
                    held.standing,
                    held.region
                )));
            }
        }
        let now = self.lab.now();
        let mut oracle = ObligationLeakOracle::new();
        oracle.snapshot_from_state(&self.lab.state, now);
        // The oracle flags a closed region holding an obligation that did not succeed.
        let mut unbalanced: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
        for held in self.reservations.values() {
            let closed = regions.get(&held.region).is_some_and(|region| {
                self.lab
                    .state
                    .region(*region)
                    .is_none_or(|record| record.state().is_terminal())
            });
            if held.standing == Standing::Leaked || (held.standing == Standing::Open && closed) {
                unbalanced
                    .entry(held.region)
                    .or_default()
                    .insert(held.obligation);
            }
        }
        match (oracle.check(now), unbalanced.is_empty()) {
            (Ok(()), true) => Ok(()),
            (Ok(()), false) => Err(disagree(format!(
                "the trace shows unbalanced regions {unbalanced:?}, the oracle none"
            ))),
            (Err(violation), _) => {
                let region = self.ordinal_of(violation.region)?.0;
                let mut flagged = BTreeSet::new();
                for leak in &violation.leaked {
                    flagged.insert(self.held(leak.obligation)?.obligation);
                }
                if unbalanced.get(&region) == Some(&flagged) {
                    Ok(())
                } else {
                    Err(disagree(format!(
                        "the oracle flags r{region} with {flagged:?}, the trace {unbalanced:?}"
                    )))
                }
            }
        }
    }

    /// The gate's acknowledgement mark: `ack <slot>`.
    fn observe_ack(&mut self, rest: &str) -> Result<(), BindingRefusal> {
        let mut parts = rest.split_whitespace();
        if parts.next() != Some("ack") {
            return Err(BindingRefusal::UnknownSubstrateEntity { what: "gate mark" });
        }
        let slot = parts
            .next()
            .and_then(|slot| slot.parse::<usize>().ok())
            .filter(|slot| *slot < self.slots.len())
            .ok_or(BindingRefusal::UnknownSubstrateEntity { what: "gate" })?;
        let task = self.task_ordinal(slot)?;
        self.acked.insert(task.0);
        self.note_task_mark(task.0)?;
        if self.observes_cancellation() {
            self.phases.entry(task.0).or_default().acknowledged = true;
        }
        Ok(())
    }

    /// Journal the acknowledgement and completion of each task a drain absorbs, by
    /// ascending task ordinal.
    ///
    /// With the reserve/commit/abort family observed, the reservations a task's
    /// cancellation aborted come between its acknowledgement and its completion, by
    /// ascending reservation ordinal: they are the cleanup of the `Cancelling` phase.
    fn journal_phases(&mut self, drained: &TaskSet) {
        for task in drained.as_slice() {
            let track = self.phases.remove(&task.0).unwrap_or_default();
            if track.acknowledged {
                self.record
                    .append(EventBody::Cancellation(CancellationEvent::Acknowledged {
                        task: *task,
                    }));
            }
            for (timer, at) in self
                .pending_timer_cancels
                .remove(&task.0)
                .unwrap_or_default()
            {
                self.record.append(EventBody::Time(TimeEvent::Cancelled {
                    timer: TimerOrdinal(timer),
                    at: VirtualInstant(at),
                }));
            }
            for body in self.pending_channel.remove(&task.0).unwrap_or_default() {
                self.record.append(body);
            }
            self.acked.remove(&task.0);
            for reservation in self.pending_aborts.remove(&task.0).unwrap_or_default() {
                self.record.append(EventBody::Effect(EffectEvent::Aborted {
                    reservation: ReservationOrdinal(reservation),
                    cause: AbortCause::Cancel,
                }));
            }
            for obligation in self.pending_discharges.remove(&task.0).unwrap_or_default() {
                self.record
                    .append(EventBody::Obligation(ObligationEvent::Discharged {
                        obligation: ObligationOrdinal(obligation),
                        how: Discharge::Aborted,
                    }));
            }
            if let Some(cause) = track.completed {
                self.record
                    .append(EventBody::Cancellation(CancellationEvent::Cancelled {
                        task: *task,
                        cause,
                    }));
            }
        }
    }

    /// Journal a task that completed as cancelled by its own deadline (bn-36wy3).
    ///
    /// No region drain absorbs it: its region stays as it was. So its whole cancellation
    /// is journaled at its completion, in the order a drain journals a task's phases:
    /// the lifecycle `cancel-requested` step and the cancellation family's request (the
    /// substrate traces none: the checkpoint that raised the deadline is both the
    /// request and the acknowledgement), the acknowledgement, the cleanup (a dropped
    /// timer, abandoned channel operations, cancel aborts, aborted discharges), the
    /// completion, and then the lifecycle `cancel` step that ends the task. Each
    /// event goes where the task's own events go: an advance's batch, the woken tasks'
    /// batch, or the journal.
    fn single_cancel(
        &mut self,
        id: SubstrateTask,
        task: TaskOrdinal,
    ) -> Result<(), BindingRefusal> {
        let track = self.phases.remove(&task.0).unwrap_or_default();
        if self.region_requested.contains(&task.0) {
            return Err(BindingRefusal::CancelRaced { task: task.0 });
        }
        self.confirm_cancelled(id, task)?;
        let mut bodies = vec![EventBody::Lifecycle(LifecycleEvent::TaskStepped {
            task,
            step: TaskStep::CancelRequested,
        })];
        if self.observes_cancellation() {
            bodies.push(EventBody::Cancellation(CancellationEvent::Requested {
                task,
                cause: CancelCause::Deadline,
            }));
            if track.acknowledged {
                bodies.push(EventBody::Cancellation(CancellationEvent::Acknowledged {
                    task,
                }));
            }
        }
        for (timer, at) in self
            .pending_timer_cancels
            .remove(&task.0)
            .unwrap_or_default()
        {
            bodies.push(EventBody::Time(TimeEvent::Cancelled {
                timer: TimerOrdinal(timer),
                at: VirtualInstant(at),
            }));
        }
        bodies.extend(self.pending_channel.remove(&task.0).unwrap_or_default());
        self.acked.remove(&task.0);
        for reservation in self.pending_aborts.remove(&task.0).unwrap_or_default() {
            bodies.push(EventBody::Effect(EffectEvent::Aborted {
                reservation: ReservationOrdinal(reservation),
                cause: AbortCause::Cancel,
            }));
        }
        for obligation in self.pending_discharges.remove(&task.0).unwrap_or_default() {
            bodies.push(EventBody::Obligation(ObligationEvent::Discharged {
                obligation: ObligationOrdinal(obligation),
                how: Discharge::Aborted,
            }));
        }
        if self.observes_cancellation() {
            bodies.push(EventBody::Cancellation(CancellationEvent::Cancelled {
                task,
                cause: CancelCause::Deadline,
            }));
        }
        bodies.push(EventBody::Lifecycle(LifecycleEvent::TaskStepped {
            task,
            step: TaskStep::Cancel,
        }));
        for body in bodies {
            self.append_task_event(task, body);
        }
        Ok(())
    }

    /// The substrate's cancellation oracle confirms that `id` completed as cancelled.
    fn confirm_cancelled(
        &self,
        id: SubstrateTask,
        task: TaskOrdinal,
    ) -> Result<(), BindingRefusal> {
        let confirmed = self.lab.oracles.cancellation_protocol.task_state(id);
        if confirmed == Some(TaskStateKind::CompletedCancelled) {
            Ok(())
        } else {
            Err(BindingRefusal::SubstrateProtocolViolation {
                detail: format!(
                    "t{} completed as cancelled but the oracle has {confirmed:?}",
                    task.0
                ),
            })
        }
    }

    fn finish(mut self) -> Result<Witnessed, BindingRefusal> {
        self.sync()?;
        // The smallest ordinal, not the first in trace order, which the seed can move.
        if let Some((task, _)) = self.pending_cancelled.iter().min() {
            return Err(BindingRefusal::UndrainedCancellation { task: task.0 });
        }
        if let Some(task) = self.phases.keys().next() {
            return Err(BindingRefusal::CancellationUnfinished { task: *task });
        }
        if let Some(task) = self
            .pending_aborts
            .keys()
            .chain(self.pending_discharges.keys())
            .chain(self.pending_timer_cancels.keys())
            .chain(self.pending_channel.keys())
            .min()
        {
            return Err(BindingRefusal::UndrainedCancellation { task: *task });
        }
        // The substrate's own ledger against the run's, whatever the run observes: the
        // standing it checks is kept for every obligation (cr-3pu5cu).
        self.cross_check_ledger()?;
        let journal = self
            .record
            .into_journal()
            .ok_or(BindingRefusal::JournalFull)?;
        Ok(Witnessed {
            journal,
            substrate_close_order: self.close_order,
            substrate_fire_order: self.fire_order,
            substrate_leak_order: self.leak_order,
            substrate_wake_order: self.wake_order,
        })
    }
}

/// The PR-14 family a substrate trace kind belongs to, for a refusal's reading.
const fn family_of(kind: TraceEventKind) -> Option<Family> {
    match kind {
        TraceEventKind::ObligationReserve
        | TraceEventKind::ObligationCommit
        | TraceEventKind::ObligationAbort => Some(Family::Effect),
        TraceEventKind::ObligationLeak => Some(Family::Obligation),
        TraceEventKind::TimeAdvance
        | TraceEventKind::TimerScheduled
        | TraceEventKind::TimerFired
        | TraceEventKind::TimerCancelled => Some(Family::Time),
        TraceEventKind::Spawn
        | TraceEventKind::Complete
        | TraceEventKind::RegionCreated
        | TraceEventKind::RegionCancelled
        | TraceEventKind::RegionCloseBegin
        | TraceEventKind::RegionCloseComplete => Some(Family::Lifecycle),
        TraceEventKind::CancelRequest | TraceEventKind::CancelAck => Some(Family::Cancellation),
        _ => None,
    }
}

/// Run `programs` on the substrate, interleaved by `log`, and observe the journal of the
/// families `config` names.
///
/// # Errors
///
/// A [`BindingRefusal`] naming the first operation, choice, or observation that could
/// not be completed. No partial journal is returned.
pub fn run(
    programs: &[Program],
    log: &ChoiceLog,
    config: &BindingConfig,
) -> Result<Journal, BindingRefusal> {
    run_witnessed(programs, log, config).map(|witnessed| witnessed.journal)
}

/// A journal, with the substrate orders the binding canonicalizes, as the substrate
/// produced them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Witnessed {
    /// The canonical journal, as [`run`] returns it.
    pub journal: Journal,
    /// The regions in the order the substrate traced `RegionCloseComplete` for them.
    /// The lab seed can change this order for sibling regions; the journal's drain
    /// order does not follow it. It is evidence about the substrate, never journal
    /// content.
    pub substrate_close_order: Vec<RegionOrdinal>,
    /// The timers in the order the substrate traced `TimerFired` for them. The lab seed
    /// can change this order for timers that come due in one advance; the journal
    /// orders them by deadline, then ordinal.
    pub substrate_fire_order: Vec<TimerOrdinal>,
    /// The leaked obligations in the order the substrate traced `ObligationLeak` for
    /// them (observed only with the obligations family). The journal orders one
    /// completion's leaks by obligation ordinal.
    pub substrate_leak_order: Vec<ObligationOrdinal>,
    /// The tasks operations woke (besides the one each commands, or a cancellation's
    /// own), in the order the substrate first stepped them. The lab seed can change it
    /// when one operation wakes several; the journal orders them by task ordinal.
    pub substrate_wake_order: Vec<TaskOrdinal>,
}

/// As [`run`], and also return the substrate's own close order.
///
/// # Errors
///
/// As for [`run`].
pub fn run_witnessed(
    programs: &[Program],
    log: &ChoiceLog,
    config: &BindingConfig,
) -> Result<Witnessed, BindingRefusal> {
    for family in Family::ALL {
        if config.families.contains(family) && substrate_binding(family) != SubstrateBinding::Bound
        {
            return Err(BindingRefusal::FamilyNotBound(family));
        }
    }
    if !config.families.contains(Family::Lifecycle) {
        return Err(BindingRefusal::LifecycleNotObserved);
    }
    let mut driver = Driver::new(*config)?;
    let mut cursors = vec![0_usize; programs.len()];
    let mut choices = log.choices().iter().enumerate();
    loop {
        let enabled: Vec<usize> = (0..programs.len())
            .filter(|actor| cursors[*actor] < programs[*actor].len())
            .collect();
        let Some((position, choice)) = choices.next() else {
            if enabled.is_empty() {
                break;
            }
            let remaining = enabled
                .iter()
                .map(|actor| programs[*actor].len() - cursors[*actor])
                .sum();
            return Err(BindingRefusal::ChoiceLogExhausted { remaining });
        };
        if enabled.is_empty() {
            return Err(BindingRefusal::ChoiceLogOverrun { position });
        }
        let actor = *enabled
            .get(choice.0 as usize)
            .ok_or(BindingRefusal::ChoiceOutOfRange {
                position,
                choice: choice.0,
                enabled: enabled.len(),
            })?;
        driver.apply(&programs[actor][cursors[actor]])?;
        driver.flush_wakes()?;
        cursors[actor] += 1;
    }
    driver.finish()
}
