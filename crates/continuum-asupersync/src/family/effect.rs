//! The reserve / commit / abort family (PR-14-IMPL-02, bn-gzy1).
//!
//! # The vocabulary, and where it comes from
//!
//! docs/01 §6 maps the substrate's *reserve/commit* to "effect phase", and docs/02 §7
//! writes the phase machine down:
//!
//! ```text
//! Idle → Reserved(token)
//! Reserved(token) → Committed(result)
//! Reserved(token) → Aborted(reason)
//! ```
//!
//! asupersync 0.5.0's two-phase primitive is the checked obligation token: a task
//! reserves it through its own `Cx` (`Cx::try_register_obligation_checked`), then
//! commits it or aborts it for a reason. The runtime applies each step and traces it
//! as `ObligationReserve`, `ObligationCommit` or `ObligationAbort`. This family's events
//! are those steps, one reservation at a time:
//!
//! | event | docs/02 §7 | asupersync 0.5.0 trace |
//! |---|---|---|
//! | [`EffectEvent::Reserved`] | `Idle → Reserved(token)` | `ObligationReserve` |
//! | [`EffectEvent::Committed`] | `Reserved → Committed` | `ObligationCommit` |
//! | [`EffectEvent::Aborted`] | `Reserved → Aborted(reason)` | `ObligationAbort`, reason `cancel` or `explicit` |
//!
//! The obligation *ledger* view of the same records — leaks, ownership transfer, the
//! linear resource delta — is the obligations family's (PR-14-IMPL-04). This family
//! is the effect phase only.
//!
//! # How the events relate to the region calculus
//!
//! `continuum_task::region` already has the effect phase for a worker's publication:
//! `WorkerStep::Reserve` stages one, `WorkerStep::Commit` commits it, and a cancelled
//! region's drain discards whatever is still staged. So the lift (`lift`, `finish`)
//! replays the events as those steps and holds each reservation to its own machine:
//!
//! 1. a reservation is `Reserved` once, by the task that then holds it, and the
//!    calculus must admit the `Reserve` step (the task is running, and it holds no other
//!    staged publication);
//! 2. it resolves exactly once: a second resolution, a commit after an abort or an
//!    abort after a commit is a fault;
//! 3. a commit is the calculus's `Commit` step;
//! 4. an abort for `cancel` happens only while the task's region drains under
//!    cancellation, the task is live, and the model still holds its staged publication,
//!    all before the drain that discards it; when the journal reports the task's
//!    cancellation phases, it comes after the task's acknowledgement, the step into
//!    `Cancelling` (bn-1i050);
//! 5. no reservation is left unresolved once the model has terminated its task: that is
//!    a leak at region close.
//!
//! An abort for `explicit` has no step in the calculus: a running worker cannot drop a
//! staged publication there, only fail or be drained. The lift reports it as
//! inconclusive (unsupported semantics), after it checks the reservation's own phase.
//!
//! # Identity
//!
//! Reservations are named by dense ordinals in the order the journal allocated them.
//! Tasks are the lifecycle family's [`TaskOrdinal`]s.

use std::collections::BTreeMap;
use std::fmt;

use continuum_task::region::worker::{WorkerId, WorkerStep};
use continuum_task::region::{DrainCause, RegionState};

use crate::encoding::{DecodeError, Decoder, EncodeError, Encoder};
use crate::family::lifecycle::TaskOrdinal;
use crate::family::{EventBody, Family};
use crate::lift::{LiftContext, LiftStop, Nonconformance};
use crate::source::{RecordContext, RecordRefusal};

/// Whether this family's events exist yet.
pub const INSTRUMENTED: bool = true;

/// A reservation, by its position in the journal's allocation order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReservationOrdinal(pub u32);

/// A program's local name for a reservation. It never reaches the journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReservationLabel(pub u32);

/// Why a reservation was aborted, in the two values the family represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AbortCause {
    /// The holding task's cancellation aborted it (asupersync's
    /// `ObligationAbortReason::Cancel`).
    Cancel,
    /// The holding task aborted it on purpose (`ObligationAbortReason::Explicit`).
    Explicit,
}

impl AbortCause {
    const fn tag(self) -> u8 {
        match self {
            Self::Cancel => 1,
            Self::Explicit => 2,
        }
    }

    const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::Cancel),
            2 => Some(Self::Explicit),
            _ => None,
        }
    }

    /// A stable token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Cancel => "cancel",
            Self::Explicit => "explicit",
        }
    }
}

impl fmt::Display for AbortCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.token())
    }
}

/// One reserve / commit / abort event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectEvent {
    /// `task` reserved a new reservation.
    Reserved {
        /// The new reservation.
        reservation: ReservationOrdinal,
        /// The task that holds it.
        task: TaskOrdinal,
    },
    /// The reservation was committed.
    Committed {
        /// The reservation.
        reservation: ReservationOrdinal,
    },
    /// The reservation was aborted.
    Aborted {
        /// The reservation.
        reservation: ReservationOrdinal,
        /// Why.
        cause: AbortCause,
    },
}

impl EffectEvent {
    const fn tag(&self) -> u8 {
        match self {
            Self::Reserved { .. } => 1,
            Self::Committed { .. } => 2,
            Self::Aborted { .. } => 3,
        }
    }

    const fn token(&self) -> &'static str {
        match self {
            Self::Reserved { .. } => "reserved",
            Self::Committed { .. } => "committed",
            Self::Aborted { .. } => "aborted",
        }
    }

    /// The reservation the event is about.
    #[must_use]
    pub const fn reservation(&self) -> ReservationOrdinal {
        match self {
            Self::Reserved { reservation, .. }
            | Self::Committed { reservation }
            | Self::Aborted { reservation, .. } => *reservation,
        }
    }
}

pub(crate) fn encode(event: &EffectEvent, out: &mut Encoder) -> Result<(), EncodeError> {
    out.tag(event.tag());
    out.u32(event.reservation().0);
    match event {
        EffectEvent::Reserved { task, .. } => out.u32(task.0),
        EffectEvent::Committed { .. } => {}
        EffectEvent::Aborted { cause, .. } => out.tag(cause.tag()),
    }
    Ok(())
}

pub(crate) fn decode(input: &mut Decoder<'_>, _seq: u64) -> Result<EffectEvent, DecodeError> {
    let at = input.offset();
    let tag = input.tag()?;
    Ok(match tag {
        1 => EffectEvent::Reserved {
            reservation: ReservationOrdinal(input.u32()?),
            task: TaskOrdinal(input.u32()?),
        },
        2 => EffectEvent::Committed {
            reservation: ReservationOrdinal(input.u32()?),
        },
        3 => {
            let reservation = ReservationOrdinal(input.u32()?);
            let cause_at = input.offset();
            let cause_tag = input.tag()?;
            let cause = AbortCause::from_tag(cause_tag).ok_or(DecodeError::UnknownTag {
                table: "abort cause",
                tag: cause_tag,
                at: cause_at,
            })?;
            EffectEvent::Aborted { reservation, cause }
        }
        other => {
            return Err(DecodeError::UnknownTag {
                table: "effect event",
                tag: other,
                at,
            });
        }
    })
}

pub(crate) fn render(event: &EffectEvent) -> String {
    match event {
        EffectEvent::Reserved { reservation, task } => {
            format!("reserved e{} task=t{}", reservation.0, task.0)
        }
        EffectEvent::Committed { reservation } => format!("committed e{}", reservation.0),
        EffectEvent::Aborted { reservation, cause } => {
            format!("aborted e{} cause={cause}", reservation.0)
        }
    }
}

// --- lift ----------------------------------------------------------------------------

/// Where one reservation is, as the journal so far says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Reserved,
    Committed,
    Aborted,
}

impl Phase {
    const fn token(self) -> &'static str {
        match self {
            Self::Reserved => "reserved",
            Self::Committed => "committed",
            Self::Aborted => "aborted",
        }
    }
}

/// Why a reserve / commit / abort event does not conform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectFault {
    /// A new reservation is not named by the next ordinal.
    ReservationIdentity {
        /// The journal's ordinal.
        journal: u32,
        /// The next ordinal.
        expected: u32,
    },
    /// The event names a reservation no earlier event reserved.
    UnknownReservation {
        /// The reservation.
        reservation: u32,
    },
    /// The reservation already resolved, so it cannot resolve again.
    AlreadyResolved {
        /// The reservation.
        reservation: u32,
        /// The event's token.
        event: &'static str,
        /// How it resolved.
        phase: &'static str,
    },
    /// A cancellation abort, but the holding task is already terminal in the model: the
    /// abort comes after the drain that discarded the staged publication.
    TaskTerminal {
        /// The reservation.
        reservation: u32,
        /// The model's status token for the task.
        state: &'static str,
    },
    /// A cancellation abort, but the holding task's region is not draining under
    /// cancellation in the model.
    RegionNotCancelled {
        /// The reservation.
        reservation: u32,
        /// The task's region.
        region: u32,
        /// The model's token for the region's state.
        state: &'static str,
    },
    /// A cancellation abort, but the model holds no staged publication for the task.
    NotStaged {
        /// The reservation.
        reservation: u32,
    },
    /// A cancellation abort by a task whose reported cancellation phase is not
    /// `acknowledged`: the abort is a `Cancelling ─ drain(effect)*` step, and the
    /// acknowledgement is the step into `Cancelling` (docs/02 §7; bn-1i050).
    OutsideCancelling {
        /// The reservation.
        reservation: u32,
        /// The holding task.
        task: u32,
        /// The task's cancellation phase.
        phase: &'static str,
    },
    /// The model terminated the holding task while the reservation was unresolved.
    LeakedAtClose {
        /// The reservation.
        reservation: u32,
        /// The holding task.
        task: u32,
    },
}

impl fmt::Display for EffectFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReservationIdentity { journal, expected } => write!(
                f,
                "the journal reserved e{journal} but the next reservation is e{expected}"
            ),
            Self::UnknownReservation { reservation } => {
                write!(f, "e{reservation} was never reserved")
            }
            Self::AlreadyResolved {
                reservation,
                event,
                phase,
            } => write!(
                f,
                "e{reservation} is already {phase}, so it cannot be {event}"
            ),
            Self::TaskTerminal { reservation, state } => write!(
                f,
                "e{reservation}'s task is already {state} in the model, so it cannot abort now"
            ),
            Self::RegionNotCancelled {
                reservation,
                region,
                state,
            } => write!(
                f,
                "e{reservation} aborts for cancel but its region r{region} is {state}"
            ),
            Self::NotStaged { reservation } => write!(
                f,
                "e{reservation} aborts for cancel but the model holds no staged publication"
            ),
            Self::OutsideCancelling {
                reservation,
                task,
                phase,
            } => write!(
                f,
                "e{reservation} aborts for cancel but t{task}'s cancellation is {phase}"
            ),
            Self::LeakedAtClose { reservation, task } => write!(
                f,
                "t{task} was terminated with e{reservation} still reserved: a leak at region close"
            ),
        }
    }
}

/// Lift state: each reservation's holder and phase.
#[derive(Debug, Default)]
pub struct LiftState {
    reservations: BTreeMap<u32, (u32, Phase)>,
}

fn fault(fault: EffectFault) -> LiftStop {
    LiftStop::Violation(Nonconformance::Effect(fault))
}

pub(crate) fn lift(event: &EffectEvent, cx: &mut LiftContext) -> Result<(), LiftStop> {
    let reservation = event.reservation().0;
    if let EffectEvent::Reserved { task, .. } = event {
        let expected = u32::try_from(cx.effect.reservations.len()).unwrap_or(u32::MAX);
        if reservation != expected {
            return Err(fault(EffectFault::ReservationIdentity {
                journal: reservation,
                expected,
            }));
        }
        cx.tree.advance(WorkerId::at(task.0), WorkerStep::Reserve)?;
        cx.effect
            .reservations
            .insert(reservation, (task.0, Phase::Reserved));
        return Ok(());
    }
    let Some((task, phase)) = cx.effect.reservations.get(&reservation).copied() else {
        return Err(fault(EffectFault::UnknownReservation { reservation }));
    };
    if phase != Phase::Reserved {
        return Err(fault(EffectFault::AlreadyResolved {
            reservation,
            event: event.token(),
            phase: phase.token(),
        }));
    }
    let worker = WorkerId::at(task);
    let next = match event {
        EffectEvent::Committed { .. } => {
            cx.tree.advance(worker, WorkerStep::Commit)?;
            Phase::Committed
        }
        EffectEvent::Aborted {
            cause: AbortCause::Cancel,
            ..
        } => {
            let state = cx.tree.worker_state(worker)?;
            if state.is_terminal() {
                return Err(fault(EffectFault::TaskTerminal {
                    reservation,
                    state: state.status_token(),
                }));
            }
            let owner = cx.tree.owner(worker)?;
            let region_state = cx.tree.state(owner)?;
            if region_state != RegionState::Draining(DrainCause::Cancelled) {
                return Err(fault(EffectFault::RegionNotCancelled {
                    reservation,
                    region: owner.ordinal(),
                    state: region_state.token(),
                }));
            }
            if !cx.tree.evidence(worker)?.is_provisional() {
                return Err(fault(EffectFault::NotStaged { reservation }));
            }
            if let Some(phase) = crate::family::cancellation::outside_cancelling(cx, task) {
                return Err(fault(EffectFault::OutsideCancelling {
                    reservation,
                    task,
                    phase,
                }));
            }
            Phase::Aborted
        }
        EffectEvent::Aborted {
            cause: AbortCause::Explicit,
            ..
        } => return Err(LiftStop::Unsupported(Family::Effect)),
        EffectEvent::Reserved { .. } => unreachable!("handled above"),
    };
    cx.effect.reservations.insert(reservation, (task, next));
    Ok(())
}

/// The whole-journal check, run after the last event: no reservation is still reserved
/// while the model has terminated its holder.
pub(crate) fn finish(cx: &LiftContext) -> Result<(), LiftStop> {
    for (reservation, (task, phase)) in &cx.effect.reservations {
        if *phase == Phase::Reserved && cx.tree.worker_state(WorkerId::at(*task))?.is_terminal() {
            return Err(fault(EffectFault::LeakedAtClose {
                reservation: *reservation,
                task: *task,
            }));
        }
    }
    Ok(())
}

// --- the scripted source -------------------------------------------------------------

/// A reserve / commit / abort step, as a scripted source reports it. Reservations and
/// tasks are named by journal ordinal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectReport {
    /// `task` reserved `reservation`.
    Reserve {
        /// The reservation.
        reservation: ReservationOrdinal,
        /// The holding task.
        task: TaskOrdinal,
    },
    /// `reservation` was committed.
    Commit {
        /// The reservation.
        reservation: ReservationOrdinal,
    },
    /// `reservation` was aborted.
    Abort {
        /// The reservation.
        reservation: ReservationOrdinal,
        /// Why.
        cause: AbortCause,
    },
}

/// Recorder state this family keeps: none. The recorder judges nothing, and the phase
/// order is the lift's to check.
#[derive(Debug, Default)]
pub struct RecordState;

pub(crate) fn record(report: &EffectReport, cx: &mut RecordContext) -> Result<(), RecordRefusal> {
    let event = match report {
        EffectReport::Reserve { reservation, task } => EffectEvent::Reserved {
            reservation: *reservation,
            task: *task,
        },
        EffectReport::Commit { reservation } => EffectEvent::Committed {
            reservation: *reservation,
        },
        EffectReport::Abort { reservation, cause } => EffectEvent::Aborted {
            reservation: *reservation,
            cause: *cause,
        },
    };
    cx.append(EventBody::Effect(event));
    Ok(())
}
