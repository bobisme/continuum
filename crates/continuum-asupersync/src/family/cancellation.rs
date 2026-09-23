//! The cancellation phases family (PR-14-IMPL-03, bn-bx7i).
//!
//! # The vocabulary, and where it comes from
//!
//! docs/01 §6 maps the substrate's *cancellation* to "phase transitions and reason",
//! and docs/02 §7 writes the phases down:
//!
//! ```text
//! Active ── request(cancel_reason) ──→ Cancelling ── drain* ── finalize* ──→ Cancelled
//! ```
//!
//! asupersync 0.5.0 spells the same machine per task as `CancelRequested → Cancelling →
//! Finalizing → Completed(Cancelled)`. This family's events are the three points of it
//! that the substrate lets a run observe, each for one task:
//!
//! | event | docs/02 §7 | asupersync 0.5.0 |
//! |---|---|---|
//! | [`CancellationEvent::Requested`] | `request(cancel_reason)` | `CancelRequest` trace event: the task is `CancelRequested` |
//! | [`CancellationEvent::Acknowledged`] | `→ Cancelling` | `Cx::checkpoint` returns the cancellation error: the task is `Cancelling` |
//! | [`CancellationEvent::Cancelled`] | `drain*`, `finalize*`, `→ Cancelled` | `Complete` trace event with the join outcome `Cancelled`: cleanup and finalizers ran |
//!
//! The substrate does not trace `Cancelling → Finalizing` by itself. It takes that step
//! and the next in the same poll that returns the task's result, so the completion
//! event is the observation of both. That is why the third event is named for its
//! result, not for one of the two steps.
//!
//! # The reason
//!
//! A request and a completion both carry a [`CancelCause`]: `user` for a task in the
//! region the program cancelled, `parent-cancelled` for a task in a region below it.
//! Those are the two kinds a bound run can produce. The binding refuses any other kind
//! with a typed refusal. It never maps it to one of these two.
//!
//! # How the events relate to the region calculus
//!
//! `continuum_task::region` has no per-task cancellation phases. There, a cancelled
//! region's drain moves every live worker to `Cancelled` in one step. So this family
//! refines that step, and the lift ([`lift`], [`finish`]) holds it to the calculus:
//!
//! 1. a task is requested only while its region is `Draining(Cancelled)` and the task
//!    is not terminal, with the cause the region tree implies;
//! 2. each task walks `requested → acknowledged → cancelled` in that order, exactly
//!    once, with the same cause at both ends, and all of it before the region drain
//!    that terminates it in the model; it completes holding no open obligation, no
//!    scheduled timer, no channel receiver and no blocked send (docs/02 §7
//!    `obligations == ∅ → Cancelled`; each is that family's own fault, bn-1i050);
//! 3. when a journal carries this family at all, every worker a model drain cancelled
//!    has completed all three phases.
//!
//! # Identity
//!
//! Tasks are named by the lifecycle family's [`TaskOrdinal`]s. The scripted source's
//! [`CancellationReport`]s name them by ordinal too, not by label: label bindings are
//! the lifecycle recorder's own state, and this family does not reach into it.

use std::collections::BTreeMap;
use std::fmt;

use continuum_task::region::worker::{WorkerId, WorkerState};
use continuum_task::region::{DrainCause, RegionId, RegionState};

use crate::encoding::{DecodeError, Decoder, EncodeError, Encoder};
use crate::family::EventBody;
use crate::family::lifecycle::TaskOrdinal;
use crate::lift::{LiftContext, LiftStop, Nonconformance};
use crate::source::{RecordContext, RecordRefusal};

/// Whether this family's events exist yet.
pub const INSTRUMENTED: bool = true;

/// Why a task was cancelled: the reason's kind, in the two values a bound run produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CancelCause {
    /// The task's own region was cancelled (asupersync's `CancelKind::User`).
    User,
    /// A region above the task's region was cancelled (asupersync's
    /// `CancelKind::ParentCancelled`).
    ParentCancelled,
}

impl CancelCause {
    const fn tag(self) -> u8 {
        match self {
            Self::User => 1,
            Self::ParentCancelled => 2,
        }
    }

    const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::User),
            2 => Some(Self::ParentCancelled),
            _ => None,
        }
    }

    /// A stable token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::ParentCancelled => "parent-cancelled",
        }
    }
}

impl fmt::Display for CancelCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.token())
    }
}

/// One cancellation-phase event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CancellationEvent {
    /// Cancellation was requested for `task`.
    Requested {
        /// The task.
        task: TaskOrdinal,
        /// Why.
        cause: CancelCause,
    },
    /// `task` observed the request at a checkpoint and started its cleanup.
    Acknowledged {
        /// The task.
        task: TaskOrdinal,
    },
    /// `task` ran its cleanup and finalizers and completed as cancelled.
    Cancelled {
        /// The task.
        task: TaskOrdinal,
        /// The reason its outcome carries.
        cause: CancelCause,
    },
}

impl CancellationEvent {
    const fn tag(&self) -> u8 {
        match self {
            Self::Requested { .. } => 1,
            Self::Acknowledged { .. } => 2,
            Self::Cancelled { .. } => 3,
        }
    }

    const fn token(&self) -> &'static str {
        match self {
            Self::Requested { .. } => "requested",
            Self::Acknowledged { .. } => "acknowledged",
            Self::Cancelled { .. } => "cancelled",
        }
    }

    /// The task the event is about.
    #[must_use]
    pub const fn task(&self) -> TaskOrdinal {
        match self {
            Self::Requested { task, .. }
            | Self::Acknowledged { task }
            | Self::Cancelled { task, .. } => *task,
        }
    }
}

pub(crate) fn encode(event: &CancellationEvent, out: &mut Encoder) -> Result<(), EncodeError> {
    out.tag(event.tag());
    out.u32(event.task().0);
    match event {
        CancellationEvent::Requested { cause, .. } | CancellationEvent::Cancelled { cause, .. } => {
            out.tag(cause.tag());
        }
        CancellationEvent::Acknowledged { .. } => {}
    }
    Ok(())
}

fn cause(input: &mut Decoder<'_>) -> Result<CancelCause, DecodeError> {
    let at = input.offset();
    let tag = input.tag()?;
    CancelCause::from_tag(tag).ok_or(DecodeError::UnknownTag {
        table: "cancel cause",
        tag,
        at,
    })
}

pub(crate) fn decode(input: &mut Decoder<'_>, _seq: u64) -> Result<CancellationEvent, DecodeError> {
    let at = input.offset();
    let tag = input.tag()?;
    Ok(match tag {
        1 => CancellationEvent::Requested {
            task: TaskOrdinal(input.u32()?),
            cause: cause(input)?,
        },
        2 => CancellationEvent::Acknowledged {
            task: TaskOrdinal(input.u32()?),
        },
        3 => CancellationEvent::Cancelled {
            task: TaskOrdinal(input.u32()?),
            cause: cause(input)?,
        },
        other => {
            return Err(DecodeError::UnknownTag {
                table: "cancellation event",
                tag: other,
                at,
            });
        }
    })
}

pub(crate) fn render(event: &CancellationEvent) -> String {
    match event {
        CancellationEvent::Requested { task, cause } => {
            format!("requested t{} cause={cause}", task.0)
        }
        CancellationEvent::Acknowledged { task } => format!("acknowledged t{}", task.0),
        CancellationEvent::Cancelled { task, cause } => {
            format!("cancelled t{} cause={cause}", task.0)
        }
    }
}

// --- lift ----------------------------------------------------------------------------

/// Where one task is in its cancellation, as the journal so far says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Requested(CancelCause),
    Acknowledged(CancelCause),
    Cancelled,
}

impl Phase {
    const fn token(self) -> &'static str {
        match self {
            Self::Requested(_) => "requested",
            Self::Acknowledged(_) => "acknowledged",
            Self::Cancelled => "cancelled",
        }
    }
}

/// Why a cancellation-phase event does not conform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CancellationFault {
    /// The event is not the next phase of its task (`active` when nothing came before).
    OutOfOrder {
        /// The task.
        task: u32,
        /// The event's token.
        event: &'static str,
        /// The task's phase before the event.
        phase: &'static str,
    },
    /// The model says the task is already terminal, so no phase of a cancellation can
    /// happen to it now. A phase after the region's drain is this fault.
    TaskTerminal {
        /// The task.
        task: u32,
        /// The event's token.
        event: &'static str,
        /// The model's status token for the task.
        state: &'static str,
    },
    /// The task's region is not `Draining(Cancelled)` in the model, so nothing has
    /// requested its cancellation.
    RegionNotCancelled {
        /// The task.
        task: u32,
        /// Its region.
        region: u32,
        /// The model's token for the region's state.
        state: &'static str,
    },
    /// The reported cause is not the one the region tree implies, or the completion's
    /// cause differs from the request's.
    CauseMismatch {
        /// The task.
        task: u32,
        /// The cause the event reports.
        reported: CancelCause,
        /// The cause expected.
        expected: CancelCause,
    },
    /// A model drain cancelled the task, but the journal carries cancellation phases and
    /// the task's own did not reach `cancelled`.
    DrainedBeforeCancelled {
        /// The task.
        task: u32,
        /// The task's last phase, `active` when it had none.
        phase: &'static str,
    },
}

impl fmt::Display for CancellationFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfOrder { task, event, phase } => {
                write!(f, "t{task} is {phase}, so {event} is not its next phase")
            }
            Self::TaskTerminal { task, event, state } => {
                write!(
                    f,
                    "t{task} is already {state} in the model, so {event} cannot happen"
                )
            }
            Self::RegionNotCancelled {
                task,
                region,
                state,
            } => write!(
                f,
                "t{task}'s region r{region} is {state}, so no cancellation reached it"
            ),
            Self::CauseMismatch {
                task,
                reported,
                expected,
            } => write!(
                f,
                "t{task} reports cause {reported} but {expected} is expected"
            ),
            Self::DrainedBeforeCancelled { task, phase } => write!(
                f,
                "the model drain cancelled t{task} while its own cancellation was {phase}"
            ),
        }
    }
}

/// Lift state: each task's phase, and whether the journal carries this family at all.
#[derive(Debug, Default)]
pub struct LiftState {
    phases: BTreeMap<u32, Phase>,
    present: bool,
}

fn fault(fault: CancellationFault) -> LiftStop {
    LiftStop::Violation(Nonconformance::Cancellation(fault))
}

/// The cause the model's region tree implies for a task in `owner`: `parent-cancelled`
/// when a proper ancestor of `owner` is draining under cancellation, `user` otherwise.
fn implied_cause(cx: &LiftContext, owner: RegionId) -> Result<CancelCause, LiftStop> {
    let count = u32::try_from(cx.tree.region_count()).unwrap_or(u32::MAX);
    for ordinal in 0..count {
        let region = RegionId::at(ordinal);
        if region == owner || cx.tree.state(region)? != RegionState::Draining(DrainCause::Cancelled)
        {
            continue;
        }
        if cx.tree.subtree(region)?.contains(&owner) {
            return Ok(CancelCause::ParentCancelled);
        }
    }
    Ok(CancelCause::User)
}

/// The checks every phase shares: the task is live in the model and its region is
/// draining under cancellation.
fn live_in_cancelled_region(
    cx: &LiftContext,
    event: &CancellationEvent,
) -> Result<RegionId, LiftStop> {
    let task = event.task().0;
    let worker = WorkerId::at(task);
    let state = cx.tree.worker_state(worker)?;
    if state.is_terminal() {
        return Err(fault(CancellationFault::TaskTerminal {
            task,
            event: event.token(),
            state: state.status_token(),
        }));
    }
    let owner = cx.tree.owner(worker)?;
    let region_state = cx.tree.state(owner)?;
    if region_state != RegionState::Draining(DrainCause::Cancelled) {
        return Err(fault(CancellationFault::RegionNotCancelled {
            task,
            region: owner.ordinal(),
            state: region_state.token(),
        }));
    }
    Ok(owner)
}

pub(crate) fn lift(event: &CancellationEvent, cx: &mut LiftContext) -> Result<(), LiftStop> {
    cx.cancellation.present = true;
    let task = event.task().0;
    let owner = live_in_cancelled_region(cx, event)?;
    let before = cx.cancellation.phases.get(&task).copied();
    let out_of_order = || {
        fault(CancellationFault::OutOfOrder {
            task,
            event: event.token(),
            phase: before.map_or("active", Phase::token),
        })
    };
    let next = match (event, before) {
        (CancellationEvent::Requested { cause, .. }, None) => {
            let expected = implied_cause(cx, owner)?;
            if *cause != expected {
                return Err(fault(CancellationFault::CauseMismatch {
                    task,
                    reported: *cause,
                    expected,
                }));
            }
            Phase::Requested(*cause)
        }
        (CancellationEvent::Acknowledged { .. }, Some(Phase::Requested(cause))) => {
            Phase::Acknowledged(cause)
        }
        (CancellationEvent::Cancelled { cause, .. }, Some(Phase::Acknowledged(requested))) => {
            if *cause != requested {
                return Err(fault(CancellationFault::CauseMismatch {
                    task,
                    reported: *cause,
                    expected: requested,
                }));
            }
            // docs/02 §7: `Cancelling ─ drain(effect)* ─ finalize(resource)* ─
            // obligations == ∅ → Cancelled`. Completing as cancelled ends the task, so
            // an obligation or a timer it still holds is that family's own end-of-task
            // fault, found at the completion rather than at the region's drain
            // (bn-1i050). A staged reservation needs no check here: the effect lift
            // already refuses it at the late abort or at `finish` (`LeakedAtClose`).
            crate::family::obligation::check_none_held(cx, task)?;
            crate::family::time::check_none_armed(cx, task)?;
            crate::family::channel::check_none_held(cx, task)?;
            Phase::Cancelled
        }
        _ => return Err(out_of_order()),
    };
    cx.cancellation.phases.insert(task, next);
    Ok(())
}

/// The task's cancellation phase token when the journal has reported one and it is not
/// `acknowledged`: a cleanup step (a cancellation's effect abort, timer drop) by this
/// task is then outside `Cancelling` (docs/02 §7). `None` when the task is
/// acknowledged, or when the journal reported no phase for it (a journal without this
/// family; [`finish`] holds a drained task to its phases).
pub(crate) fn outside_cancelling(cx: &LiftContext, task: u32) -> Option<&'static str> {
    match cx.cancellation.phases.get(&task) {
        None | Some(Phase::Acknowledged(_)) => None,
        Some(phase) => Some(phase.token()),
    }
}

/// Whether the task's reported cancellation has reached `Cancelling`: it acknowledged,
/// or completed as cancelled. Such a task no longer acts on its own account.
pub(crate) fn is_cancelling(cx: &LiftContext, task: u32) -> bool {
    matches!(
        cx.cancellation.phases.get(&task),
        Some(Phase::Acknowledged(_) | Phase::Cancelled)
    )
}

/// The task's cancellation phase token, if the journal has reported any.
pub(crate) fn phase_of(cx: &LiftContext, task: u32) -> Option<&'static str> {
    cx.cancellation.phases.get(&task).map(|phase| phase.token())
}

/// The whole-journal check, run after the last event: when the journal carries this
/// family, every worker a model drain cancelled completed its own cancellation first.
///
/// A journal with no event of this family makes no claim about phases (it is the
/// lifecycle projection of a run), so it is not checked.
pub(crate) fn finish(cx: &LiftContext) -> Result<(), LiftStop> {
    if !cx.cancellation.present {
        return Ok(());
    }
    let count = u32::try_from(cx.tree.worker_count()).unwrap_or(u32::MAX);
    for task in 0..count {
        if *cx.tree.worker_state(WorkerId::at(task))? != WorkerState::Cancelled {
            continue;
        }
        let phase = cx.cancellation.phases.get(&task).copied();
        if phase != Some(Phase::Cancelled) {
            return Err(fault(CancellationFault::DrainedBeforeCancelled {
                task,
                phase: phase.map_or("active", Phase::token),
            }));
        }
    }
    Ok(())
}

// --- the scripted source -------------------------------------------------------------

/// A cancellation phase, as a scripted source reports it. Tasks are named by journal
/// ordinal (see the module documentation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CancellationReport {
    /// Cancellation was requested for `task`.
    Request {
        /// The task.
        task: TaskOrdinal,
        /// Why.
        cause: CancelCause,
    },
    /// `task` acknowledged it.
    Acknowledge {
        /// The task.
        task: TaskOrdinal,
    },
    /// `task` completed as cancelled.
    Complete {
        /// The task.
        task: TaskOrdinal,
        /// The reason its outcome carries.
        cause: CancelCause,
    },
}

/// Recorder state this family keeps: none. The recorder judges nothing, and the phase
/// order is the lift's to check.
#[derive(Debug, Default)]
pub struct RecordState;

pub(crate) fn record(
    report: &CancellationReport,
    cx: &mut RecordContext,
) -> Result<(), RecordRefusal> {
    let event = match report {
        CancellationReport::Request { task, cause } => CancellationEvent::Requested {
            task: *task,
            cause: *cause,
        },
        CancellationReport::Acknowledge { task } => CancellationEvent::Acknowledged { task: *task },
        CancellationReport::Complete { task, cause } => CancellationEvent::Cancelled {
            task: *task,
            cause: *cause,
        },
    };
    cx.append(EventBody::Cancellation(event));
    Ok(())
}
