//! The substrate binding: asupersync's lab runtime, driven by a Continuum [`ChoiceLog`],
//! observed into the journal events of the families it binds.
//!
//! # What is bound, and what is not
//!
//! The binding observes four families: [`Family::Lifecycle`] (PR-14-IMPL-01, bn-lf4i),
//! [`Family::Effect`] (PR-14-IMPL-02, bn-gzy1), [`Family::Cancellation`] (PR-14-IMPL-03,
//! bn-bx7i) and [`Family::Obligation`] (PR-14-IMPL-04, bn-6nm8). A run observes the
//! families
//! [`BindingConfig::families`] names. Lifecycle is always among them, because the
//! other families name tasks by the ordinals its spawn events allocate. The default is
//! lifecycle alone, whose journal is the one bn-lf4i pinned, byte for byte.
//!
//! The other two families are still uninstrumented (their files under `src/family/`
//! say so), and for them [`substrate_binding`] answers with the typed absence
//! [`BindingAbsence::FamilyNotBound`], whose INV-008 reading is
//! [`InconclusiveReason::Unsupported`]. A run that asks for one is refused with
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
//! | `ObligationLeak` | [`ObligationEvent::Leaked`] |
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

use core::fmt;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::task::{Context, Poll, Waker};

use asupersync::lab::oracle::ObligationLeakOracle;
use asupersync::lab::oracle::TaskStateKind;
use asupersync::record::{
    ObligationAbortReason, ObligationKind as SubstrateObligationKind, ObligationState,
};
use asupersync::runtime::obligation_mailbox::ObligationToken;
use asupersync::runtime::{JoinError, TaskHandle};
use asupersync::trace::event::{TraceData, TraceEvent, TraceEventKind};
use asupersync::{
    Budget, CancelKind, CancelReason, Cx, LabConfig, LabRuntime, ObligationId,
    RegionId as SubstrateRegion, TaskId as SubstrateTask,
};
use continuum_task::region::worker::Resumability;
use continuum_value::assurance::InconclusiveReason;

use crate::choice::ChoiceLog;
use crate::family::cancellation::{CancelCause, CancellationEvent};
use crate::family::effect::{AbortCause, EffectEvent, ReservationLabel, ReservationOrdinal};
use crate::family::lifecycle::{
    LifecycleEvent, RegionLabel, RegionOrdinal, TaskLabel, TaskOrdinal, TaskSet, TaskStep,
};
use crate::family::obligation::{
    Discharge, ObligationEvent, ObligationKind, ObligationOrdinal, ObligationSet,
};
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
        Family::Lifecycle | Family::Effect | Family::Cancellation | Family::Obligation => {
            SubstrateBinding::Bound
        }
        other => SubstrateBinding::Absent(BindingAbsence::FamilyNotBound(other)),
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
            | Self::UnmappedCancelReason { .. }
            | Self::ReservationDropped { .. } => Some(InconclusiveReason::Unsupported),
            Self::SubstrateProtocolViolation { .. } | Self::SubstrateLedgerDisagrees { .. } => {
                Some(InconclusiveReason::EngineError)
            }
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
            | Self::EffectNotTransferable(_) => None,
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

async fn gated_body(gate: Gate, slot: usize) {
    // The task's own obligations, by program label. Ascending order makes the aborts a
    // cancellation issues deterministic within the task.
    let mut held: BTreeMap<ReservationLabel, ObligationToken> = BTreeMap::new();
    loop {
        let command = GateWait {
            gate: Arc::clone(&gate),
            slot,
        }
        .await;
        // Obligations transferred here since the last poll are this task's now.
        held.extend(core::mem::take(&mut lock(&gate).inbox));
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
            // Returning with obligations held drops them: the substrate records a leak.
            Some(Command::Finish) => break,
            // Cancellation observed: the cleanup is to abort every held obligation.
            None => {
                for (_, token) in core::mem::take(&mut held) {
                    token.abort(ObligationAbortReason::Cancel);
                }
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
    close_order: Vec<RegionOrdinal>,
    phases: BTreeMap<u32, CancelTrack>,
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
            close_order: Vec::new(),
            phases: BTreeMap::new(),
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
        self.flush_closes();
        self.record.append(body);
    }

    const fn observes_effect(&self) -> bool {
        self.config.families.contains(Family::Effect)
    }

    const fn observes_obligation(&self) -> bool {
        self.config.families.contains(Family::Obligation)
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
    fn flush_closes(&mut self) {
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
            } => {
                let region = self.region(*region)?;
                if self.task_labels.contains_key(task) {
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
                }));
                let (id, handle) = self
                    .lab
                    .state
                    .create_task(
                        region,
                        Budget::INFINITE,
                        gated_body(Arc::clone(&gate), slot),
                    )
                    .map_err(|error| refused(format!("{error:?}")))?;
                self.slots.push(Slot {
                    id,
                    handle,
                    gate,
                    resumability: resumability.clone(),
                    ordinal: None,
                });
                self.slot_of.insert(id, slot);
                self.task_labels.insert(*task, slot);
                self.sync()
            }
            SubstrateOp::Begin { task } => {
                let id = self.slots[self.slot(*task)?].id;
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
        if self.observes_cancellation() {
            self.lab
                .check_cancellation_protocol()
                .map_err(|violation| BindingRefusal::SubstrateProtocolViolation {
                    detail: violation.to_string(),
                })?;
        }
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
                self.append(LifecycleEvent::TaskStepped { task, step });
            }
            (TraceEventKind::Complete, TraceData::Task { task, region }) => {
                let slot = self.slot_of(*task)?;
                let region = self.ordinal_of(*region)?;
                let ordinal = self.task_ordinal(slot)?;
                match self.slots[slot].handle.try_join() {
                    Ok(Some(())) => self.append(LifecycleEvent::TaskStepped {
                        task: ordinal,
                        step: TaskStep::Complete,
                    }),
                    Err(JoinError::Cancelled(reason)) => {
                        self.pending_cancelled.push((ordinal, region.0));
                        if self.observes_cancellation() {
                            let cause = cause_of(reason.kind)?;
                            let confirmed =
                                self.lab.oracles.cancellation_protocol.task_state(*task);
                            if confirmed != Some(TaskStateKind::CompletedCancelled) {
                                return Err(BindingRefusal::SubstrateProtocolViolation {
                                    detail: format!(
                                        "t{} completed as cancelled but the oracle has {confirmed:?}",
                                        ordinal.0
                                    ),
                                });
                            }
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
                if self.observes_cancellation() {
                    // Requests start a new batch: closes already pending came first.
                    self.flush_closes();
                    let ordinal = self.task_ordinal(self.slot_of(*task)?)?;
                    let cause = cause_of(reason.kind)?;
                    self.pending_requests.push((ordinal, cause));
                    self.phases.entry(ordinal.0).or_default();
                }
            }
            (TraceEventKind::RegionCloseBegin, _) => {}
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
                    self.append_body(EventBody::Effect(EffectEvent::Reserved {
                        reservation: ReservationOrdinal(reservation),
                        task,
                    }));
                }
                if self.observes_obligation() {
                    self.append_body(EventBody::Obligation(ObligationEvent::Opened {
                        obligation: ObligationOrdinal(ordinal),
                        kind: journal_kind(*kind),
                        holder: task,
                        region,
                    }));
                }
            }
            (TraceEventKind::ObligationCommit, TraceData::Obligation { obligation, .. }) => {
                let held = self.held(*obligation)?;
                self.resolve(*obligation, Standing::Discharged);
                if let Some(reservation) = held.reservation
                    && self.observes_effect()
                {
                    self.append_body(EventBody::Effect(EffectEvent::Committed {
                        reservation: ReservationOrdinal(reservation),
                    }));
                }
                if self.observes_obligation() {
                    self.append_body(EventBody::Obligation(ObligationEvent::Discharged {
                        obligation: ObligationOrdinal(held.obligation),
                        how: Discharge::Committed,
                    }));
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
                            self.append_body(EventBody::Effect(EffectEvent::Aborted {
                                reservation: ReservationOrdinal(reservation),
                                cause: AbortCause::Explicit,
                            }));
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
                        self.append_body(EventBody::Obligation(ObligationEvent::Discharged {
                            obligation: ObligationOrdinal(held.obligation),
                            how: Discharge::Aborted,
                        }));
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
                    self.append_body(EventBody::Obligation(ObligationEvent::Leaked {
                        obligation: ObligationOrdinal(held.obligation),
                    }));
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
            .min()
        {
            return Err(BindingRefusal::UndrainedCancellation { task: *task });
        }
        if self.observes_obligation() {
            self.cross_check_ledger()?;
        }
        let journal = self
            .record
            .into_journal()
            .ok_or(BindingRefusal::JournalFull)?;
        Ok(Witnessed {
            journal,
            substrate_close_order: self.close_order,
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

/// A journal, with the one substrate order the binding canonicalizes, as the substrate
/// produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Witnessed {
    /// The canonical journal, as [`run`] returns it.
    pub journal: Journal,
    /// The regions in the order the substrate traced `RegionCloseComplete` for them.
    /// The lab seed can change this order for sibling regions; the journal's drain
    /// order does not follow it. It is evidence about the substrate, never journal
    /// content.
    pub substrate_close_order: Vec<RegionOrdinal>,
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
        cursors[actor] += 1;
    }
    driver.finish()
}
