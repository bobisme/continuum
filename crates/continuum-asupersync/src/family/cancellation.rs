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
//! region the program cancelled, `parent-cancelled` for a task in a region below it,
//! and `deadline` for a task whose own budget deadline passed (bn-36wy3). Those are the
//! three kinds a bound run can produce. The binding refuses any other kind with a
//! typed refusal. It never maps it to one of these three.
//!
//! A deadline cancellation is one task's, not a region's. asupersync 0.5.0 raises it at
//! the task's next `Cx::checkpoint` once the lab clock has passed the deadline, and
//! traces no `CancelRequest` for it: the checkpoint is both the request and the
//! acknowledgement. So the binding journals its `requested … cause=deadline` together
//! with its `acknowledged`, and the task's `cancelled` just before the lifecycle
//! family's `cancel` step, which ends that one task while its region stays as it was.
//!
//! # How the events relate to the region calculus
//!
//! `continuum_task::region` has per-task cancellation phases since RFC 0026 correction
//! 53: `RequestCancel`, `AcknowledgeCancel`, `CompleteCancelled`. A cancelled region's
//! drain still moves every live worker to `Cancelled` in one step. This family refines
//! both, and the lift ([`lift`], [`finish`]) holds it to the calculus:
//!
//! 1. a task is requested with `user` or `parent-cancelled` only while its region is
//!    `Draining(Cancelled)` and the task is not terminal, with the cause the region
//!    tree implies; a `deadline` request is the calculus's own `RequestCancel` (refused
//!    when a region already requested it), and needs a declared deadline the clock has
//!    reached ([`crate::family::time::check_deadline_passed`]);
//!
//!    An acknowledgement is the calculus's `AcknowledgeCancel`, so the calculus knows
//!    from that point that the task only drains;
//! 2. each task walks `requested → acknowledged → cancelled` in that order, exactly
//!    once, with the same cause at both ends, and all of it before the region drain
//!    that terminates it in the model; it completes holding no open obligation, no
//!    scheduled timer, no channel receiver and no blocked send (docs/02 §7
//!    `obligations == ∅ → Cancelled`; each is that family's own fault, bn-1i050);
//! 3. when a journal carries this family at all, every worker a model drain cancelled
//!    has completed all three phases;
//! 4. a task whose own cancellation the lifecycle family requested ends by its own
//!    lifecycle `cancel` and by nothing else: not a region's drain, which leaves the
//!    same `Cancelled` state, and not a `complete` or `fail`
//!    ([`CancellationFault::EndedWithoutOwnCancel`]; RFC 0026 correction 53 item 6).
//!    The lift records that end when the calculus admits it, and never reads it from
//!    the terminal state (cr-3pu5cu);
//! 5. while that own cancellation is open, every event of every family is one of its
//!    own steps: its phases, its cleanup (after its acknowledgement and before its
//!    completion, when the journal reports phases) or its own `cancel`
//!    ([`CancellationFault::InterruptedOwnCancel`], checked before each event's own
//!    family lift; RFC 0026 correction 53 item 6, cr-3pu5cu round 6).
//!
//! # Identity
//!
//! Tasks are named by the lifecycle family's [`TaskOrdinal`]s. The scripted source's
//! [`CancellationReport`]s name them by ordinal too, not by label: label bindings are
//! the lifecycle recorder's own state, and this family does not reach into it.

use std::collections::BTreeMap;
use std::fmt;

use continuum_task::region::worker::{WorkerId, WorkerState, WorkerStep};
use continuum_task::region::{DrainCause, RegionId, RegionState};

use crate::encoding::{DecodeError, Decoder, EncodeError, Encoder};
use crate::family::EventBody;
use crate::family::lifecycle::TaskOrdinal;
use crate::lift::{LiftContext, LiftStop, Nonconformance};
use crate::source::{RecordContext, RecordRefusal};

/// Whether this family's events exist yet.
pub const INSTRUMENTED: bool = true;

/// Why a task was cancelled: the reason's kind, in the three values a bound run
/// produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CancelCause {
    /// The task's own region was cancelled (asupersync's `CancelKind::User`).
    User,
    /// A region above the task's region was cancelled (asupersync's
    /// `CancelKind::ParentCancelled`).
    ParentCancelled,
    /// The task's own budget deadline passed (asupersync's `CancelKind::Deadline`,
    /// raised at `Cx::checkpoint`; bn-36wy3). One task's cancellation, not a region's.
    Deadline,
}

impl CancelCause {
    const fn tag(self) -> u8 {
        match self {
            Self::User => 1,
            Self::ParentCancelled => 2,
            Self::Deadline => 3,
        }
    }

    const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::User),
            2 => Some(Self::ParentCancelled),
            3 => Some(Self::Deadline),
            _ => None,
        }
    }

    /// A stable token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::ParentCancelled => "parent-cancelled",
            Self::Deadline => "deadline",
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

    pub(crate) const fn token(&self) -> &'static str {
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
    let cause = CancelCause::from_tag(tag).ok_or(DecodeError::UnknownTag {
        table: "cancel cause",
        tag,
        at,
    })?;
    if cause == CancelCause::Deadline {
        input.require_version(2, "cancel cause", tag, at)?;
    }
    Ok(cause)
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
    Cancelled(CancelCause),
}

impl Phase {
    const fn token(self) -> &'static str {
        match self {
            Self::Requested(_) => "requested",
            Self::Acknowledged(_) => "acknowledged",
            Self::Cancelled(_) => "cancelled",
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
    /// A deadline's request or a single-task end names a task whose own cancellation
    /// the lifecycle family did not request (`cancel-requested`; bn-36wy3).
    NotRequestedAlone {
        /// The task.
        task: u32,
    },
    /// An event that is not a step of the task's own cancellation came between its
    /// own `cancel-requested` and its own `cancel`: an event of any family that is not
    /// one of the task's cancellation phases, its cleanup (after its acknowledgement
    /// and before its completion, when the journal reports phases) or its own `cancel`.
    /// The binding journals a deadline cancellation within the operation that observed
    /// it, as one run of that task's events (RFC 0026 correction 53 item 6).
    InterruptedOwnCancel {
        /// The task whose own cancellation is open.
        task: u32,
        /// The event's token within its family.
        event: &'static str,
    },
    /// The task's own cancellation was requested (lifecycle `cancel-requested`), and a
    /// step other than its own lifecycle `cancel` ended it: a region's drain, or a
    /// normal completion or failure. RFC 0026 correction 53 item 6 journals a deadline
    /// cancellation as `cancel-requested` … `cancel` within one operation, and item 3
    /// makes that `cancel` the calculus's single-task `CompleteCancelled`. A shared
    /// terminal state does not stand for the missing step (cr-3pu5cu).
    EndedWithoutOwnCancel {
        /// The task.
        task: u32,
        /// The model's status token for the task after the step.
        state: &'static str,
    },
    /// A region's cancellation request reached the task while it was live (the calculus
    /// records it on the worker), the journal carries this family, and it reports no
    /// phase for the task: the task then ended some other way (`complete`, `fail`), or
    /// the journal lost its phases. The binding journals a `requested` phase for every
    /// substrate `CancelRequest` (cr-3pu5cu round 6).
    UnreportedRequest {
        /// The task.
        task: u32,
        /// The step that ended the task, or `end` at the end of the journal.
        event: &'static str,
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
            Self::NotRequestedAlone { task } => write!(
                f,
                "t{task}'s own cancellation was never requested (no lifecycle cancel-requested)"
            ),
            Self::InterruptedOwnCancel { task, event } => write!(
                f,
                "{event} comes inside t{task}'s own cancellation, between its cancel-requested and its cancel"
            ),
            Self::EndedWithoutOwnCancel { task, state } => write!(
                f,
                "t{task}'s own cancellation was requested, but it ended {state} without its own lifecycle cancel"
            ),
            Self::UnreportedRequest { task, event } => write!(
                f,
                "a region's cancellation reached t{task}, but the journal reports no phase for it before {event}"
            ),
            Self::DrainedBeforeCancelled { task, phase } => write!(
                f,
                "the model drain cancelled t{task} while its own cancellation was {phase}"
            ),
        }
    }
}

/// Lift state: each task's phase, whether the journal carries this family at all, the
/// tasks whose own cancellation the lifecycle family requested (bn-36wy3), and of those
/// the tasks whose own lifecycle `cancel` the calculus admitted.
///
/// `ended_alone` is provenance, not a reading of the model: a task is in it only once
/// its own `CompleteCancelled` succeeded. A region's drain also leaves a worker
/// `Cancelled`, so the shared terminal state cannot tell the two ends apart
/// (cr-3pu5cu).
#[derive(Debug, Default)]
pub struct LiftState {
    phases: BTreeMap<u32, Phase>,
    present: bool,
    alone: std::collections::BTreeSet<u32>,
    ended_alone: std::collections::BTreeSet<u32>,
    /// The task whose own cancellation is open: requested and not yet ended.
    open_alone: Option<u32>,
}

/// The journal carries this family (set before the first event is lifted).
pub(crate) fn mark_present(cx: &mut LiftContext) {
    cx.cancellation.present = true;
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

/// The check every phase shares: the task is live in the model.
fn live(cx: &LiftContext, event: &CancellationEvent) -> Result<(), LiftStop> {
    let task = event.task().0;
    let state = cx.tree.worker_state(WorkerId::at(task))?;
    if state.is_terminal() {
        return Err(fault(CancellationFault::TaskTerminal {
            task,
            event: event.token(),
            state: state.status_token(),
        }));
    }
    Ok(())
}

/// A region's cancellation reached the task: its region is draining under
/// cancellation. The rule for every phase of a `user` or `parent-cancelled`
/// cancellation.
fn in_cancelled_region(cx: &LiftContext, event: &CancellationEvent) -> Result<RegionId, LiftStop> {
    let task = event.task().0;
    let worker = WorkerId::at(task);
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
    let worker = WorkerId::at(task);
    live(cx, event)?;
    let before = cx.cancellation.phases.get(&task).copied();
    // A region's cancellation needs the region draining under cancellation at every
    // phase; a deadline's is the task's own and needs none.
    let by_region = match (event, before) {
        (CancellationEvent::Requested { cause, .. }, _) => *cause != CancelCause::Deadline,
        (_, Some(Phase::Requested(cause) | Phase::Acknowledged(cause))) => {
            cause != CancelCause::Deadline
        }
        _ => true,
    };
    let owner = if by_region {
        Some(in_cancelled_region(cx, event)?)
    } else {
        None
    };
    let out_of_order = || {
        fault(CancellationFault::OutOfOrder {
            task,
            event: event.token(),
            phase: before.map_or("active", Phase::token),
        })
    };
    let next = match (event, before) {
        (CancellationEvent::Requested { cause, .. }, None) => {
            match owner {
                Some(owner) => {
                    let expected = implied_cause(cx, owner)?;
                    if *cause != expected {
                        return Err(fault(CancellationFault::CauseMismatch {
                            task,
                            reported: *cause,
                            expected,
                        }));
                    }
                }
                // The task's own deadline: the lifecycle family's `cancel-requested`
                // made the calculus's single-task request just before.
                None => check_requested_alone(cx, task)?,
            }
            Phase::Requested(*cause)
        }
        (CancellationEvent::Acknowledged { .. }, Some(Phase::Requested(cause))) => {
            // From here the calculus knows the task only drains (RFC 0026 correction 53).
            cx.tree.advance(worker, WorkerStep::AcknowledgeCancel)?;
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
            Phase::Cancelled(*cause)
        }
        _ => return Err(out_of_order()),
    };
    cx.cancellation.phases.insert(task, next);
    Ok(())
}

/// The task's cancellation phase token when a cleanup step (a cancellation's effect
/// abort, timer drop, channel abandonment) by this task is outside `Cancelling`
/// (docs/02 §7): the journal carries this family and the task is not `acknowledged`
/// (`active` when it has no phase yet). `None` when the task is acknowledged, or when
/// the journal carries no event of this family (a projection without it).
pub(crate) fn outside_cancelling(cx: &LiftContext, task: u32) -> Option<&'static str> {
    if !cx.cancellation.present {
        return None;
    }
    match cx.cancellation.phases.get(&task) {
        Some(Phase::Acknowledged(_)) => None,
        None => Some("active"),
        Some(phase) => Some(phase.token()),
    }
}

/// Whether the task's reported cancellation has reached `Cancelling`: it acknowledged,
/// or completed as cancelled. Such a task no longer acts on its own account.
/// The calculus's own acknowledgement counts too: a cleanup step stands for it in a
/// journal without this family ([`imply_acknowledgement`]; cr-3pu5cu round 6).
pub(crate) fn is_cancelling(cx: &LiftContext, task: u32) -> bool {
    matches!(
        cx.cancellation.phases.get(&task),
        Some(Phase::Acknowledged(_) | Phase::Cancelled(_))
    ) || cx.tree.cancel_phase(WorkerId::at(task)).ok()
        == Some(continuum_task::region::worker::CancelPhase::Acknowledged)
}

/// A cleanup step by `task` (an abort with the `cancel` cause, a timer's cancellation,
/// an abandoned send or receive) is the task draining, which it does only after it
/// observed its cancellation (docs/02 §7; RFC 0026 correction 53 item 2). When the
/// journal reports phases, the acknowledgement came first and this is a no-op. When it
/// reports none (a projection without this family), the cleanup step stands for the
/// acknowledgement: the calculus takes `AcknowledgeCancel`, so the task's ordinary work
/// after its own cleanup is refused as it is with phases (cr-3pu5cu round 6).
pub(crate) fn imply_acknowledgement(cx: &mut LiftContext, task: u32) -> Result<(), LiftStop> {
    let worker = WorkerId::at(task);
    if cx.tree.cancel_phase(worker)? == continuum_task::region::worker::CancelPhase::Requested {
        cx.tree.advance(worker, WorkerStep::AcknowledgeCancel)?;
    }
    Ok(())
}

/// The task's cancellation phase token, if the journal has reported any.
pub(crate) fn phase_of(cx: &LiftContext, task: u32) -> Option<&'static str> {
    cx.cancellation.phases.get(&task).map(|phase| phase.token())
}

/// The end-of-journal check for single-task cancellations: every task whose own
/// cancellation was requested ended by its own lifecycle `cancel`.
///
/// A task requested alone and still live is a journal truncated inside its
/// cancellation ([`LiftStop::Incomplete`]). A task requested alone that some other step
/// ended is [`CancellationFault::EndedWithoutOwnCancel`]; the lift reports that at the
/// step already, and this is the same rule read once more at the end.
pub(crate) fn finish_alone(cx: &LiftContext) -> Result<(), LiftStop> {
    for task in cx
        .cancellation
        .alone
        .difference(&cx.cancellation.ended_alone)
    {
        check_not_absorbed(cx, *task)?;
    }
    // `ended_alone` is a subset of `alone`: the lifecycle `cancel` needs the task's own
    // request first (`check_requested_alone`).
    if cx.cancellation.alone.len() == cx.cancellation.ended_alone.len() {
        Ok(())
    } else {
        Err(LiftStop::Incomplete(crate::family::Family::Lifecycle))
    }
}

/// A task whose own cancellation was requested may end only by its own lifecycle
/// `cancel` (RFC 0026 correction 53 items 3 and 6). Run after every step that can end
/// `task` some other way: a region's drain, or a lifecycle `complete` or `fail`.
pub(crate) fn check_not_absorbed(cx: &LiftContext, task: u32) -> Result<(), LiftStop> {
    if !cx.cancellation.alone.contains(&task) || cx.cancellation.ended_alone.contains(&task) {
        return Ok(());
    }
    let state = cx.tree.worker_state(WorkerId::at(task))?;
    if state.is_terminal() {
        return Err(fault(CancellationFault::EndedWithoutOwnCancel {
            task,
            state: state.status_token(),
        }));
    }
    Ok(())
}

/// Record that `task`'s own lifecycle `cancel` was admitted: the calculus's
/// `CompleteCancelled` succeeded for it.
pub(crate) fn note_ended_alone(cx: &mut LiftContext, task: u32) {
    cx.cancellation.ended_alone.insert(task);
    cx.cancellation.open_alone = None;
}

/// While a task's own cancellation is open (its lifecycle `cancel-requested` lifted, its
/// own `cancel` not yet), every event of every family must be a step of that
/// cancellation (RFC 0026 correction 53 item 6; [`CancellationFault::InterruptedOwnCancel`]).
///
/// The substrate raises a deadline at one `Cx::checkpoint`, which is both the request and
/// the acknowledgement, and the binding journals the whole cancellation as one
/// contiguous run of that task's events. So the rule admits exactly:
///
/// 1. a cancellation phase of the task (this family orders them: `requested` with the
///    `deadline` cause, `acknowledged`, `cancelled`);
/// 2. a cleanup step of the task: an abort with the `cancel` cause of a reservation it
///    holds, an aborted discharge of an obligation it holds, the cancellation of a timer
///    it armed, the abandonment of its blocked send or blocked receive, or the drop of a
///    receiver it holds. When the journal carries this family, a cleanup step comes
///    after the task's `acknowledged` and before its `cancelled`;
/// 3. the task's own lifecycle `cancel`.
///
/// Every other event is refused: ordinary work of the task (a reserve, commit or
/// explicit abort, an obligation open, committed discharge or transfer, a send or
/// receive, a timer scheduled), any other lifecycle step of the task, any event of
/// another task, and any event that names no task (a region event, a spawn, a clock
/// advance, a deadline declaration, a leak, a region settlement, a senders' close). The
/// per-family rules still judge each admitted event.
pub(crate) fn check_own_cancel_admits(
    body: &crate::family::EventBody,
    cx: &LiftContext,
) -> Result<(), LiftStop> {
    use crate::family::EventBody;
    use crate::family::lifecycle::{LifecycleEvent, TaskStep};
    let Some(task) = cx.cancellation.open_alone else {
        return Ok(());
    };
    let cleanup = match body {
        EventBody::Cancellation(event) => {
            if event.task().0 == task {
                return Ok(());
            }
            None
        }
        EventBody::Lifecycle(LifecycleEvent::TaskStepped {
            task: stepped,
            step: TaskStep::Cancel,
        }) => {
            if stepped.0 == task {
                return Ok(());
            }
            None
        }
        EventBody::Lifecycle(_) => None,
        EventBody::Effect(event) => crate::family::effect::cleanup_of(cx, event),
        EventBody::Obligation(event) => crate::family::obligation::cleanup_of(cx, event),
        EventBody::Time(event) => crate::family::time::cleanup_of(cx, event),
        EventBody::Channel(event) => crate::family::channel::cleanup_of(cx, event),
    };
    if cleanup == Some(task) && outside_cancelling(cx, task).is_none() {
        return Ok(());
    }
    Err(fault(CancellationFault::InterruptedOwnCancel {
        task,
        event: body.token(),
    }))
}

/// A lifecycle `complete` or `fail` of a task whose cancellation the journal reported:
/// once requested, the task has left `Active`, and `complete` and `panic` leave `Active`
/// only (docs/02 §7). Its end is `cancelled`.
///
/// When the journal carries this family, a cancellation that reached the task and has
/// no reported phase is refused too ([`CancellationFault::UnreportedRequest`]): the
/// calculus records a region's request on each live worker, and before its
/// acknowledgement it lets the worker act as before, so the missing `requested` would
/// otherwise go unseen.
pub(crate) fn check_no_phase(
    cx: &LiftContext,
    task: u32,
    event: &'static str,
) -> Result<(), LiftStop> {
    match cx.cancellation.phases.get(&task) {
        Some(phase) => Err(fault(CancellationFault::OutOfOrder {
            task,
            event,
            phase: phase.token(),
        })),
        None if cx.cancellation.present
            && cx.tree.cancel_phase(WorkerId::at(task))?
                != continuum_task::region::worker::CancelPhase::Active =>
        {
            Err(fault(CancellationFault::UnreportedRequest { task, event }))
        }
        None => Ok(()),
    }
}

/// Record that the lifecycle family requested `task`'s own cancellation.
pub(crate) fn note_requested_alone(cx: &mut LiftContext, task: u32) {
    cx.cancellation.alone.insert(task);
    cx.cancellation.open_alone = Some(task);
}

/// A single-task cancellation step needs the task's own lifecycle `cancel-requested`
/// first: a deadline's request and end are one task's, and never a region's.
pub(crate) fn check_requested_alone(cx: &LiftContext, task: u32) -> Result<(), LiftStop> {
    if cx.cancellation.alone.contains(&task) {
        Ok(())
    } else {
        Err(fault(CancellationFault::NotRequestedAlone { task }))
    }
}

/// How a lifecycle `cancel` step may end `task` alone: the journal's own phases, when it
/// reports any for the task. `Ok(true)` when they ended in `cancelled` with the
/// `deadline` cause, `Ok(false)` when the journal carries no event of this family (a
/// projection without it), and a fault otherwise: a single-task end is the deadline's, never a
/// region's, and never before the task's own phases finished (bn-36wy3).
pub(crate) fn single_task_end(cx: &LiftContext, task: u32) -> Result<bool, LiftStop> {
    match cx.cancellation.phases.get(&task).copied() {
        // A journal that carries this family reports every phase of the task's own
        // cancellation before its `cancel`: none is a missing `requested`, not a
        // projection.
        None if cx.cancellation.present => Err(fault(CancellationFault::OutOfOrder {
            task,
            event: "cancel",
            phase: "active",
        })),
        None => Ok(false),
        Some(Phase::Cancelled(CancelCause::Deadline)) => Ok(true),
        Some(Phase::Cancelled(cause)) => Err(fault(CancellationFault::CauseMismatch {
            task,
            reported: cause,
            expected: CancelCause::Deadline,
        })),
        Some(phase) => Err(fault(CancellationFault::OutOfOrder {
            task,
            event: "cancel",
            phase: phase.token(),
        })),
    }
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
    // Every reported phase agrees with the calculus at the end (cr-3pu5cu): a task still
    // `requested` or `acknowledged`, or `cancelled` but not yet terminal in the model
    // (its drain or its own `cancel` step missing), means the journal stops inside the
    // task's cancellation.
    for (task, phase) in &cx.cancellation.phases {
        let state = cx.tree.worker_state(WorkerId::at(*task))?;
        let ended = matches!(phase, Phase::Cancelled(_)) && *state == WorkerState::Cancelled;
        if ended {
            continue;
        }
        // A worker already terminal some other way cannot end its phases later: that is
        // a violation, not a truncation.
        if state.is_terminal() && *state != WorkerState::Cancelled {
            return Err(fault(CancellationFault::OutOfOrder {
                task: *task,
                event: state.status_token(),
                phase: phase.token(),
            }));
        }
        return Err(LiftStop::Incomplete(crate::family::Family::Cancellation));
    }
    let count = u32::try_from(cx.tree.worker_count()).unwrap_or(u32::MAX);
    // Every worker a cancellation reached has a reported phase, whatever state it ended
    // in (cr-3pu5cu round 6). A worker's own request is the lifecycle family's
    // (`finish_alone`); a region's request is recorded on the worker only while it is
    // live, so a worker that ended before it is not reached. A live worker with no phase
    // is a journal that stops inside the cancellation.
    for task in 0..count {
        let worker = WorkerId::at(task);
        if cx.cancellation.phases.contains_key(&task)
            || cx.cancellation.alone.contains(&task)
            || cx.tree.cancel_phase(worker)? == continuum_task::region::worker::CancelPhase::Active
        {
            continue;
        }
        let state = cx.tree.worker_state(worker)?;
        if !state.is_terminal() {
            return Err(LiftStop::Incomplete(crate::family::Family::Cancellation));
        }
        if *state != WorkerState::Cancelled {
            return Err(fault(CancellationFault::UnreportedRequest {
                task,
                event: "end",
            }));
        }
    }
    for task in 0..count {
        if *cx.tree.worker_state(WorkerId::at(task))? != WorkerState::Cancelled {
            continue;
        }
        let phase = cx.cancellation.phases.get(&task).copied();
        if !matches!(phase, Some(Phase::Cancelled(_))) {
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
