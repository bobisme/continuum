//! The virtual time family (PR-14-IMPL-05, bn-3m1d).
//!
//! # The vocabulary, and where it comes from
//!
//! docs/01 §6 maps the substrate's *virtual time* to "time constraints", and INV-005
//! makes time a capability: controlled code reads it only through an explicit handle,
//! never from the host. asupersync 0.5.0's lab runtime has exactly that: a virtual clock
//! that moves only when the driver advances it, and a timer driver on it that each
//! task's `Cx` carries. A task sleeps with `time::sleep_until` on that clock, and the
//! `Sleep` future writes its timer's life into the substrate's trace. This family's
//! events are that clock and those timers:
//!
//! | event | asupersync 0.5.0 |
//! |---|---|
//! | [`TimeEvent::Scheduled`] | `TimerScheduled` trace event, with the virtual instant and the deadline |
//! | [`TimeEvent::Advanced`] | `LabRuntime::advance_time`, read back from the lab's virtual clock |
//! | [`TimeEvent::Fired`] | `TimerFired` trace event, at the virtual instant it fired |
//! | [`TimeEvent::Cancelled`] | `TimerCancelled` trace event: the sleeping task's cancellation dropped its timer |
//!
//! Instants are nanoseconds on the lab's virtual clock, which starts at zero. No event
//! reads the host clock.
//!
//! # How the events relate to the region calculus
//!
//! `continuum_task::region` has no notion of time. So the lift keeps a parallel,
//! checked model of the clock and the timers beside it (`lift`, `finish`), and ties it
//! to the calculus where the two meet — the task that owns a timer:
//!
//! 1. the clock starts at zero and is monotone: an advance starts at the current
//!    instant and moves forward, and every scheduled, fired or cancelled event happens
//!    at the current instant;
//! 2. a timer is scheduled once, by a live task, with a deadline after the instant it
//!    was scheduled;
//! 3. it fires or is cancelled exactly once; it fires only at or after its deadline;
//!    it is cancelled only while its task's region drains under cancellation and the
//!    task is live;
//! 4. no timer is late: before the clock advances, before a new timer is scheduled,
//!    and at the end, no scheduled timer's deadline has passed;
//! 5. no timer outlives its task: once the model terminates a task, its timers have
//!    fired or been cancelled.
//!
//! # Identity
//!
//! Timers are named by dense ordinals in the order the journal allocated them. Tasks
//! are the lifecycle family's [`TaskOrdinal`]s.

use std::collections::BTreeMap;
use std::fmt;

use continuum_task::region::worker::WorkerId;
use continuum_task::region::{DrainCause, RegionState};

use crate::encoding::{DecodeError, Decoder, EncodeError, Encoder};
use crate::family::EventBody;
use crate::family::lifecycle::TaskOrdinal;
use crate::lift::{LiftContext, LiftStop, Nonconformance};
use crate::source::{RecordContext, RecordRefusal};

/// Whether this family's events exist yet.
pub const INSTRUMENTED: bool = true;

/// A timer, by its position in the journal's allocation order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimerOrdinal(pub u32);

/// An instant on the lab's virtual clock, in nanoseconds from its start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VirtualInstant(pub u64);

/// One virtual time event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimeEvent {
    /// `task` scheduled a timer at `at` for `deadline`.
    Scheduled {
        /// The new timer.
        timer: TimerOrdinal,
        /// The task that sleeps on it.
        task: TaskOrdinal,
        /// When it was scheduled.
        at: VirtualInstant,
        /// When it is due.
        deadline: VirtualInstant,
    },
    /// The virtual clock moved from `from` to `to`.
    Advanced {
        /// The instant before.
        from: VirtualInstant,
        /// The instant after.
        to: VirtualInstant,
    },
    /// The timer fired at `at`.
    Fired {
        /// The timer.
        timer: TimerOrdinal,
        /// When.
        at: VirtualInstant,
    },
    /// The timer was cancelled at `at`.
    Cancelled {
        /// The timer.
        timer: TimerOrdinal,
        /// When.
        at: VirtualInstant,
    },
}

impl TimeEvent {
    const fn tag(&self) -> u8 {
        match self {
            Self::Scheduled { .. } => 1,
            Self::Advanced { .. } => 2,
            Self::Fired { .. } => 3,
            Self::Cancelled { .. } => 4,
        }
    }

    const fn token(&self) -> &'static str {
        match self {
            Self::Scheduled { .. } => "scheduled",
            Self::Advanced { .. } => "advanced",
            Self::Fired { .. } => "fired",
            Self::Cancelled { .. } => "cancelled",
        }
    }
}

pub(crate) fn encode(event: &TimeEvent, out: &mut Encoder) -> Result<(), EncodeError> {
    out.tag(event.tag());
    match event {
        TimeEvent::Scheduled {
            timer,
            task,
            at,
            deadline,
        } => {
            out.u32(timer.0);
            out.u32(task.0);
            out.u64(at.0);
            out.u64(deadline.0);
        }
        TimeEvent::Advanced { from, to } => {
            out.u64(from.0);
            out.u64(to.0);
        }
        TimeEvent::Fired { timer, at } | TimeEvent::Cancelled { timer, at } => {
            out.u32(timer.0);
            out.u64(at.0);
        }
    }
    Ok(())
}

pub(crate) fn decode(input: &mut Decoder<'_>, _seq: u64) -> Result<TimeEvent, DecodeError> {
    let at_offset = input.offset();
    let tag = input.tag()?;
    Ok(match tag {
        1 => TimeEvent::Scheduled {
            timer: TimerOrdinal(input.u32()?),
            task: TaskOrdinal(input.u32()?),
            at: VirtualInstant(input.u64()?),
            deadline: VirtualInstant(input.u64()?),
        },
        2 => TimeEvent::Advanced {
            from: VirtualInstant(input.u64()?),
            to: VirtualInstant(input.u64()?),
        },
        3 => TimeEvent::Fired {
            timer: TimerOrdinal(input.u32()?),
            at: VirtualInstant(input.u64()?),
        },
        4 => TimeEvent::Cancelled {
            timer: TimerOrdinal(input.u32()?),
            at: VirtualInstant(input.u64()?),
        },
        other => {
            return Err(DecodeError::UnknownTag {
                table: "time event",
                tag: other,
                at: at_offset,
            });
        }
    })
}

pub(crate) fn render(event: &TimeEvent) -> String {
    match event {
        TimeEvent::Scheduled {
            timer,
            task,
            at,
            deadline,
        } => format!(
            "scheduled k{} task=t{} at={} deadline={}",
            timer.0, task.0, at.0, deadline.0
        ),
        TimeEvent::Advanced { from, to } => format!("advanced {}->{}", from.0, to.0),
        TimeEvent::Fired { timer, at } => format!("fired k{} at={}", timer.0, at.0),
        TimeEvent::Cancelled { timer, at } => format!("cancelled k{} at={}", timer.0, at.0),
    }
}

// --- lift ----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Scheduled,
    Fired,
    Cancelled,
}

impl State {
    const fn token(self) -> &'static str {
        match self {
            Self::Scheduled => "scheduled",
            Self::Fired => "fired",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Timer {
    task: u32,
    deadline: u64,
    state: State,
}

/// Why a virtual time event does not conform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimeFault {
    /// A new timer is not named by the next ordinal.
    TimerIdentity {
        /// The journal's ordinal.
        journal: u32,
        /// The next ordinal.
        expected: u32,
    },
    /// The event names a timer no earlier event scheduled.
    UnknownTimer {
        /// The timer.
        timer: u32,
    },
    /// The event happens at an instant other than the clock's current one.
    OffClock {
        /// The event's token.
        event: &'static str,
        /// The instant it names.
        at: u64,
        /// The clock's current instant.
        now: u64,
    },
    /// The clock did not move forward.
    NotForward {
        /// The instant before.
        from: u64,
        /// The instant after.
        to: u64,
    },
    /// A timer was scheduled with a deadline that is not after its instant.
    DeadlineNotAhead {
        /// The timer.
        timer: u32,
        /// Its deadline.
        deadline: u64,
    },
    /// The timer already fired or was cancelled.
    AlreadyEnded {
        /// The timer.
        timer: u32,
        /// The event's token.
        event: &'static str,
        /// How it ended.
        state: &'static str,
    },
    /// A timer fired before its deadline.
    Early {
        /// The timer.
        timer: u32,
        /// Its deadline.
        deadline: u64,
        /// When it fired.
        at: u64,
    },
    /// A timer's deadline passed and it did not fire before the next clock or timer
    /// step, or before the end.
    Late {
        /// The timer.
        timer: u32,
        /// Its deadline.
        deadline: u64,
        /// The clock's current instant.
        now: u64,
    },
    /// The timer's task is terminal in the model.
    TaskTerminal {
        /// The timer.
        timer: u32,
        /// The event's token.
        event: &'static str,
    },
    /// A timer was cancelled while its task's region is not draining under
    /// cancellation.
    NotCancelling {
        /// The timer.
        timer: u32,
        /// The model's token for the region's state.
        state: &'static str,
    },
    /// The model terminated a task whose timer was still scheduled.
    OutlivesTask {
        /// The timer.
        timer: u32,
        /// Its task.
        task: u32,
    },
}

impl fmt::Display for TimeFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TimerIdentity { journal, expected } => write!(
                f,
                "the journal scheduled k{journal} but the next timer is k{expected}"
            ),
            Self::UnknownTimer { timer } => write!(f, "k{timer} was never scheduled"),
            Self::OffClock { event, at, now } => {
                write!(f, "{event} at {at} but the clock reads {now}")
            }
            Self::NotForward { from, to } => write!(f, "the clock moved from {from} to {to}"),
            Self::DeadlineNotAhead { timer, deadline } => {
                write!(
                    f,
                    "k{timer}'s deadline {deadline} is not ahead of the clock"
                )
            }
            Self::AlreadyEnded {
                timer,
                event,
                state,
            } => write!(f, "k{timer} is already {state}, so it cannot be {event}"),
            Self::Early {
                timer,
                deadline,
                at,
            } => write!(f, "k{timer} fired at {at}, before its deadline {deadline}"),
            Self::Late {
                timer,
                deadline,
                now,
            } => write!(
                f,
                "k{timer} is due at {deadline} but had not fired by {now}"
            ),
            Self::TaskTerminal { timer, event } => {
                write!(f, "k{timer}'s task is terminal, so it cannot be {event}")
            }
            Self::NotCancelling { timer, state } => {
                write!(f, "k{timer} was cancelled but its task's region is {state}")
            }
            Self::OutlivesTask { timer, task } => {
                write!(f, "t{task} ended with k{timer} still scheduled")
            }
        }
    }
}

/// Lift state: the model clock, which starts at zero as the lab's does, and each timer.
#[derive(Debug, Default)]
pub struct LiftState {
    now: u64,
    timers: BTreeMap<u32, Timer>,
}

fn fault(fault: TimeFault) -> LiftStop {
    LiftStop::Violation(Nonconformance::Time(fault))
}

/// The event happens at the clock's current instant.
fn on_clock(state: &LiftState, event: &TimeEvent, at: u64) -> Result<(), LiftStop> {
    if state.now == at {
        Ok(())
    } else {
        Err(fault(TimeFault::OffClock {
            event: event.token(),
            at,
            now: state.now,
        }))
    }
}

/// No scheduled timer is past due.
fn none_late(state: &LiftState) -> Result<(), LiftStop> {
    let now = state.now;
    for (timer, entry) in &state.timers {
        if entry.state == State::Scheduled && entry.deadline <= now {
            return Err(fault(TimeFault::Late {
                timer: *timer,
                deadline: entry.deadline,
                now,
            }));
        }
    }
    Ok(())
}

fn live_task(cx: &LiftContext, timer: u32, task: u32, event: &TimeEvent) -> Result<(), LiftStop> {
    if cx.tree.worker_state(WorkerId::at(task))?.is_terminal() {
        return Err(fault(TimeFault::TaskTerminal {
            timer,
            event: event.token(),
        }));
    }
    Ok(())
}

pub(crate) fn lift(event: &TimeEvent, cx: &mut LiftContext) -> Result<(), LiftStop> {
    match event {
        TimeEvent::Scheduled {
            timer,
            task,
            at,
            deadline,
        } => {
            let expected = u32::try_from(cx.time.timers.len()).unwrap_or(u32::MAX);
            if timer.0 != expected {
                return Err(fault(TimeFault::TimerIdentity {
                    journal: timer.0,
                    expected,
                }));
            }
            on_clock(&cx.time, event, at.0)?;
            none_late(&cx.time)?;
            live_task(cx, timer.0, task.0, event)?;
            if deadline.0 <= at.0 {
                return Err(fault(TimeFault::DeadlineNotAhead {
                    timer: timer.0,
                    deadline: deadline.0,
                }));
            }
            cx.time.timers.insert(
                timer.0,
                Timer {
                    task: task.0,
                    deadline: deadline.0,
                    state: State::Scheduled,
                },
            );
        }
        TimeEvent::Advanced { from, to } => {
            on_clock(&cx.time, event, from.0)?;
            none_late(&cx.time)?;
            if to.0 <= from.0 {
                return Err(fault(TimeFault::NotForward {
                    from: from.0,
                    to: to.0,
                }));
            }
            cx.time.now = to.0;
        }
        TimeEvent::Fired { timer, at } | TimeEvent::Cancelled { timer, at } => {
            on_clock(&cx.time, event, at.0)?;
            let Some(mut entry) = cx.time.timers.get(&timer.0).copied() else {
                return Err(fault(TimeFault::UnknownTimer { timer: timer.0 }));
            };
            if entry.state != State::Scheduled {
                return Err(fault(TimeFault::AlreadyEnded {
                    timer: timer.0,
                    event: event.token(),
                    state: entry.state.token(),
                }));
            }
            live_task(cx, timer.0, entry.task, event)?;
            if let TimeEvent::Fired { .. } = event {
                if at.0 < entry.deadline {
                    return Err(fault(TimeFault::Early {
                        timer: timer.0,
                        deadline: entry.deadline,
                        at: at.0,
                    }));
                }
                entry.state = State::Fired;
            } else {
                let owner = cx.tree.owner(WorkerId::at(entry.task))?;
                let state = cx.tree.state(owner)?;
                if state != RegionState::Draining(DrainCause::Cancelled) {
                    return Err(fault(TimeFault::NotCancelling {
                        timer: timer.0,
                        state: state.token(),
                    }));
                }
                entry.state = State::Cancelled;
            }
            cx.time.timers.insert(timer.0, entry);
        }
    }
    Ok(())
}

/// The whole-journal check, run after the last event: no timer is late, and no timer
/// outlives the task that sleeps on it.
pub(crate) fn finish(cx: &LiftContext) -> Result<(), LiftStop> {
    none_late(&cx.time)?;
    for (timer, entry) in &cx.time.timers {
        if entry.state == State::Scheduled
            && cx
                .tree
                .worker_state(WorkerId::at(entry.task))?
                .is_terminal()
        {
            return Err(fault(TimeFault::OutlivesTask {
                timer: *timer,
                task: entry.task,
            }));
        }
    }
    Ok(())
}

// --- the scripted source -------------------------------------------------------------

/// A virtual time step, as a scripted source reports it: the event itself, named by
/// journal ordinals and virtual instants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeReport(pub TimeEvent);

/// Recorder state this family keeps: none. The recorder judges nothing.
#[derive(Debug, Default)]
pub struct RecordState;

pub(crate) fn record(report: &TimeReport, cx: &mut RecordContext) -> Result<(), RecordRefusal> {
    cx.append(EventBody::Time(report.0.clone()));
    Ok(())
}
