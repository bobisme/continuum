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
//! | [`TimeEvent::Deadline`] | the task's `Budget` deadline, as the binding created the task with it (bn-36wy3) |
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
//! 2. a timer is scheduled once, by a running task whose cancellation has not begun and
//!    that sleeps on no other timer, with a deadline after the instant it was
//!    scheduled (bn-1i050). "Not begun" is read from the calculus, not from the
//!    reported phases, so it holds in every projection: a task a request reached and
//!    that has not acknowledged is refused before this family's lift (RFC 0026
//!    correction 55; asupersync 0.5.0's `Sleep` observes the request at its first poll);
//! 3. it fires or is cancelled exactly once; it fires only at or after its deadline;
//!    it is cancelled only while a cancellation is requested for its task (by its
//!    region or its own deadline; bn-36wy3) and the task is live, and after the task's acknowledgement when the journal reports its
//!    cancellation phases (bn-1i050); a sleeping task resumes only after its timer
//!    fired (bn-1i050);
//! 4. no timer is late: before the clock advances, before a new timer is scheduled,
//!    and at the end, no scheduled timer's deadline has passed;
//! 5. no timer outlives its task: once the model terminates a task, its timers have
//!    fired or been cancelled;
//! 6. a task's budget deadline is declared once, by the event right after its spawn, ahead of the
//!    clock; a cancellation with the `deadline` cause needs a declared deadline that
//!    the clock has reached ([`check_deadline_passed`], bn-36wy3). Once the clock has
//!    reached it, none of the task's own work, in any family, comes before that
//!    cancellation ([`check_actor_deadline`], [`TimeFault::DeadlineIgnored`]). A
//!    journal without this family cannot show the clock, so these checks are not made
//!    there.
//!
//! # Identity
//!
//! Timers are named by dense ordinals in the order the journal allocated them. Tasks
//! are the lifecycle family's [`TaskOrdinal`]s.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use continuum_task::region::worker::{CancelPhase, WorkerId, WorkerState};

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
    /// `task` runs under a budget whose deadline is `at`: once the clock reaches it, the
    /// task's next checkpoint observes its cancellation (asupersync's
    /// `CancelKind::Deadline`; bn-36wy3). Journaled right after the task's spawn.
    Deadline {
        /// The task.
        task: TaskOrdinal,
        /// Its deadline.
        at: VirtualInstant,
    },
    /// The timer's task crashed, fail-stop, while it slept on it: the timer is neither
    /// fired nor cancelled, and no later fire of it reaches a task (bn-20d8u). In
    /// asupersync 0.5.0 its wheel entry still comes due and wakes a task that no longer
    /// exists, which polls nothing and traces nothing. Encoding version 3.
    Fenced {
        /// The timer.
        timer: TimerOrdinal,
    },
}

impl TimeEvent {
    const fn tag(&self) -> u8 {
        match self {
            Self::Scheduled { .. } => 1,
            Self::Advanced { .. } => 2,
            Self::Fired { .. } => 3,
            Self::Cancelled { .. } => 4,
            Self::Deadline { .. } => 5,
            Self::Fenced { .. } => 6,
        }
    }

    pub(crate) const fn token(&self) -> &'static str {
        match self {
            Self::Scheduled { .. } => "scheduled",
            Self::Advanced { .. } => "advanced",
            Self::Fired { .. } => "fired",
            Self::Cancelled { .. } => "cancelled",
            Self::Deadline { .. } => "deadline",
            Self::Fenced { .. } => "fenced",
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
        TimeEvent::Fenced { timer } => out.u32(timer.0),
        TimeEvent::Deadline { task, at } => {
            out.u32(task.0);
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
        5 => {
            input.require_version(2, "time event", 5, at_offset)?;
            TimeEvent::Deadline {
                task: TaskOrdinal(input.u32()?),
                at: VirtualInstant(input.u64()?),
            }
        }
        6 => {
            input.require_version(3, "time event", 6, at_offset)?;
            TimeEvent::Fenced {
                timer: TimerOrdinal(input.u32()?),
            }
        }
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
        TimeEvent::Deadline { task, at } => format!("deadline t{} at={}", task.0, at.0),
        TimeEvent::Fenced { timer } => format!("fenced k{}", timer.0),
    }
}

// --- lift ----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Scheduled,
    Fired,
    Cancelled,
    Fenced,
}

impl State {
    const fn token(self) -> &'static str {
        match self {
            Self::Scheduled => "scheduled",
            Self::Fired => "fired",
            Self::Cancelled => "cancelled",
            Self::Fenced => "fenced",
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
    /// A timer was cancelled while no cancellation was requested for its task, by its
    /// region or by its own deadline.
    NotCancelling {
        /// The timer.
        timer: u32,
        /// The model's token for the task's cancellation phase.
        state: &'static str,
    },
    /// A task's budget deadline is not declared by the event right after its task's
    /// spawn (cr-3pu5cu round 8).
    DeadlineNotAtSpawn {
        /// The task.
        task: u32,
    },
    /// A task's budget deadline is declared twice, or after the task began.
    DeadlineMisplaced {
        /// The task.
        task: u32,
        /// The model's status token for the task, or `declared` for a second one.
        state: &'static str,
    },
    /// A task's budget deadline is not ahead of the clock when it is declared.
    TaskDeadlineNotAhead {
        /// The task.
        task: u32,
        /// Its deadline.
        deadline: u64,
        /// The clock's current instant.
        now: u64,
    },
    /// A cancellation with the `deadline` cause names a task with no declared deadline.
    DeadlineUnknown {
        /// The task.
        task: u32,
    },
    /// A cancellation with the `deadline` cause comes before the clock reached the
    /// task's deadline.
    DeadlineNotPassed {
        /// The task.
        task: u32,
        /// Its deadline.
        deadline: u64,
        /// The clock's current instant.
        now: u64,
    },
    /// A task whose deadline the clock has reached is polled on as if it had none:
    /// a step of its own work (any family; [`check_actor_deadline`]) comes after the
    /// deadline. The substrate's poll starts
    /// with `Cx::checkpoint`, which raises the deadline's cancellation first (RFC 0026
    /// correction 53), so the task's next poll after its deadline is its cancellation.
    DeadlineIgnored {
        /// The task.
        task: u32,
        /// Its deadline.
        deadline: u64,
        /// The clock's current instant.
        now: u64,
    },
    /// A task's own region was cancelled after the clock reached the task's deadline and
    /// before the task observed it. The substrate completes such a task with the more
    /// severe `deadline` reason under the region's request, so the binding refuses the
    /// run (`BindingRefusal::CancelRaced`); a journal that shows it is not one the
    /// binding returns (RFC 0026 correction 53 item 6; cr-3pu5cu round 7). A proper
    /// ancestor's cancellation outranks the deadline and is not this fault.
    CancelRaced {
        /// The task.
        task: u32,
        /// Its region, the one cancelled.
        region: u32,
        /// Its deadline.
        deadline: u64,
        /// The clock's current instant.
        now: u64,
    },
    /// A timer scheduled by a task that is not running, or whose cancellation has
    /// begun, or that already sleeps: a task sleeps by calling `sleep_until` while it
    /// runs, one timer at a time (bn-1i050).
    NotRunning {
        /// The timer.
        timer: u32,
        /// The task.
        task: u32,
        /// The model's status token for the task, or its cancellation phase.
        state: &'static str,
    },
    /// A timer was cancelled by a task whose reported cancellation phase is not
    /// `acknowledged`: dropping it is a `Cancelling ─ finalize(resource)*` step (docs/02
    /// §7; bn-1i050).
    OutsideCancelling {
        /// The timer.
        timer: u32,
        /// The task's cancellation phase.
        phase: &'static str,
    },
    /// A sleeping task resumed while its timer was still scheduled (bn-1i050).
    WokenBeforeFire {
        /// The task.
        task: u32,
        /// Its scheduled timer.
        timer: u32,
    },
    /// A task ended (the model terminated it, or the journal reported it completed as
    /// cancelled) while its timer was still scheduled.
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
                write!(
                    f,
                    "k{timer} was cancelled but its task's cancellation is {state}"
                )
            }
            Self::DeadlineNotAtSpawn { task } => {
                write!(
                    f,
                    "t{task}'s deadline is not declared right after its spawn"
                )
            }
            Self::DeadlineMisplaced { task, state } => {
                write!(f, "t{task}'s deadline is declared while it is {state}")
            }
            Self::TaskDeadlineNotAhead {
                task,
                deadline,
                now,
            } => write!(
                f,
                "t{task}'s deadline {deadline} is not ahead of the clock at {now}"
            ),
            Self::DeadlineUnknown { task } => {
                write!(f, "t{task} is cancelled by a deadline it never declared")
            }
            Self::DeadlineNotPassed {
                task,
                deadline,
                now,
            } => write!(
                f,
                "t{task} is cancelled by its deadline {deadline}, but the clock reads {now}"
            ),
            Self::DeadlineIgnored {
                task,
                deadline,
                now,
            } => write!(
                f,
                "t{task} acts at {now}, past its deadline {deadline}, and is not cancelled"
            ),
            Self::CancelRaced {
                task,
                region,
                deadline,
                now,
            } => write!(
                f,
                "r{region} is cancelled at {now}, but its task t{task} passed its deadline {deadline} unobserved"
            ),
            Self::NotRunning { timer, task, state } => {
                write!(f, "k{timer} scheduled by t{task}, which is {state}")
            }
            Self::OutsideCancelling { timer, phase } => write!(
                f,
                "k{timer} was cancelled but its task's cancellation is {phase}"
            ),
            Self::WokenBeforeFire { task, timer } => {
                write!(f, "t{task} resumed while k{timer} was still scheduled")
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
    /// Each task's declared budget deadline.
    deadlines: BTreeMap<u32, u64>,
    /// The tasks with a declared deadline, by the region that owns them, for the race
    /// check at that region's cancellation ([`check_cancel_not_raced`]).
    deadlines_by_region: BTreeMap<u32, std::collections::BTreeSet<u32>>,
    /// The timers each task sleeps on (scheduled), for a crash's fences (bn-20d8u).
    armed: BTreeMap<u32, BTreeSet<u32>>,
    /// Whether the journal carries this family at all.
    present: bool,
}

/// Record `timer`, keeping the per-task index of scheduled timers in step.
fn put(state: &mut LiftState, timer: u32, entry: Timer) {
    if entry.state == State::Scheduled {
        state.armed.entry(entry.task).or_default().insert(timer);
    } else if let Some(set) = state.armed.get_mut(&entry.task) {
        set.remove(&timer);
        if set.is_empty() {
            state.armed.remove(&entry.task);
        }
    }
    state.timers.insert(timer, entry);
}

/// The timers `task` sleeps on, when the journal carries this family: the fences a
/// crash of `task` owes (bn-20d8u).
pub(crate) fn armed_by(cx: &LiftContext, task: u32) -> Vec<u32> {
    if !cx.time.present {
        return Vec::new();
    }
    cx.time
        .armed
        .get(&task)
        .map(|set| set.iter().copied().collect())
        .unwrap_or_default()
}

/// Whether `task` runs under a declared budget deadline, when the journal carries this
/// family: a crash of it has no semantics here (bn-20d8u).
pub(crate) fn has_deadline(cx: &LiftContext, task: u32) -> bool {
    cx.time.present && cx.time.deadlines.contains_key(&task)
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

/// No scheduled timer is past due, when the journal carries this family: checked at a
/// crash, which may not fence an overdue timer away (bn-20d8u).
pub(crate) fn check_none_late(cx: &LiftContext) -> Result<(), LiftStop> {
    none_late(&cx.time)
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

/// The journal carries this family (set before the first event is lifted).
pub(crate) fn mark_present(cx: &mut LiftContext) {
    cx.time.present = true;
}

pub(crate) fn lift(event: &TimeEvent, cx: &mut LiftContext) -> Result<(), LiftStop> {
    cx.time.present = true;
    match event {
        TimeEvent::Deadline { task, at } => {
            // The binding declares a deadline in the event right after its task's spawn
            // (RFC 0026 correction 53 item 6). Both are always carried together: the
            // spawn is lifecycle, which every projection keeps, and dropping a family
            // only removes events, so the two stay adjacent in every projection that
            // carries this family (cr-3pu5cu round 8).
            if cx.spawned_just_before != Some(task.0) {
                return Err(fault(TimeFault::DeadlineNotAtSpawn { task: task.0 }));
            }
            let state = cx.tree.worker_state(WorkerId::at(task.0))?;
            if *state != WorkerState::Created {
                return Err(fault(TimeFault::DeadlineMisplaced {
                    task: task.0,
                    state: state.status_token(),
                }));
            }
            if cx.time.deadlines.contains_key(&task.0) {
                return Err(fault(TimeFault::DeadlineMisplaced {
                    task: task.0,
                    state: "declared",
                }));
            }
            // A deadline is declared at the spawn, before any cancellation of the task.
            let phase = cx.tree.cancel_phase(WorkerId::at(task.0))?;
            if phase != CancelPhase::Active {
                return Err(fault(TimeFault::DeadlineMisplaced {
                    task: task.0,
                    state: phase.token(),
                }));
            }
            if at.0 <= cx.time.now {
                return Err(fault(TimeFault::TaskDeadlineNotAhead {
                    task: task.0,
                    deadline: at.0,
                    now: cx.time.now,
                }));
            }
            cx.time.deadlines.insert(task.0, at.0);
            let owner = cx.tree.owner(WorkerId::at(task.0))?.ordinal();
            cx.time
                .deadlines_by_region
                .entry(owner)
                .or_default()
                .insert(task.0);
        }
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
            let state = cx.tree.worker_state(WorkerId::at(task.0))?;
            let not_running = if *state == WorkerState::Running {
                crate::family::cancellation::phase_of(cx, task.0)
                    .or_else(|| {
                        crate::family::cancellation::is_cancelling(cx, task.0)
                            .then_some("acknowledged")
                    })
                    .or_else(|| armed_for(cx, task.0).map(|_| "asleep"))
            } else {
                Some(state.status_token())
            };
            if let Some(state) = not_running {
                return Err(fault(TimeFault::NotRunning {
                    timer: timer.0,
                    task: task.0,
                    state,
                }));
            }
            put(
                &mut cx.time,
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
                let phase = cx.tree.cancel_phase(WorkerId::at(entry.task))?;
                if phase == CancelPhase::Active {
                    return Err(fault(TimeFault::NotCancelling {
                        timer: timer.0,
                        state: phase.token(),
                    }));
                }
                if let Some(phase) = crate::family::cancellation::outside_cancelling(cx, entry.task)
                {
                    return Err(fault(TimeFault::OutsideCancelling {
                        timer: timer.0,
                        phase,
                    }));
                }
                crate::family::cancellation::imply_acknowledgement(cx, entry.task)?;
                entry.state = State::Cancelled;
            }
            put(&mut cx.time, timer.0, entry);
        }
        // A crash's fence: owed by the crash that stopped the sleeper, and only then
        // (bn-20d8u). A fenced timer is never late: its fire reaches no task.
        TimeEvent::Fenced { timer } => {
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
            if !cx.fences.timers.remove(&timer.0) {
                return Err(LiftStop::Violation(Nonconformance::FenceUnowed {
                    family: crate::family::Family::Time,
                    ordinal: timer.0,
                }));
            }
            entry.state = State::Fenced;
            put(&mut cx.time, timer.0, entry);
        }
    }
    Ok(())
}

/// The task whose cancellation cleanup `event` is: the task that armed the timer a
/// `cancelled` names. `None` for every other event, and for a timer the lift does not
/// know (RFC 0026 correction 53 item 6).
pub(crate) fn cleanup_of(cx: &LiftContext, event: &TimeEvent) -> Option<u32> {
    match event {
        TimeEvent::Cancelled { timer, .. } => cx.time.timers.get(&timer.0).map(|entry| entry.task),
        _ => None,
    }
}

/// A cancellation with the `deadline` cause is admitted for `task`: when the journal
/// carries this family, the task declared a deadline and the clock has reached it. A
/// journal without this family shows no clock, and is not checked (bn-36wy3).
pub(crate) fn check_deadline_passed(cx: &LiftContext, task: u32) -> Result<(), LiftStop> {
    if !cx.time.present {
        return Ok(());
    }
    match cx.time.deadlines.get(&task) {
        None => Err(fault(TimeFault::DeadlineUnknown { task })),
        Some(deadline) if *deadline > cx.time.now => Err(fault(TimeFault::DeadlineNotPassed {
            task,
            deadline: *deadline,
            now: cx.time.now,
        })),
        Some(_) => Ok(()),
    }
}

/// A cancellation request for `region` races a deadline: a live task that `region`
/// itself owns has a declared deadline the clock has reached ([`TimeFault::CancelRaced`]).
/// Tasks of a proper subregion are not checked: the request reaches them as
/// `parent-cancelled`, which outranks the deadline. Each region is cancelled at most
/// once (`RepeatedCancel`), so each task is checked at most once. A journal without
/// this family declares no deadline, so it is not checked: it cannot show the race, and
/// the binding refuses the run itself under every projection.
pub(crate) fn check_cancel_not_raced(cx: &LiftContext, region: u32) -> Result<(), LiftStop> {
    let Some(tasks) = cx.time.deadlines_by_region.get(&region) else {
        return Ok(());
    };
    for task in tasks {
        let Some(&deadline) = cx.time.deadlines.get(task) else {
            continue;
        };
        if deadline > cx.time.now || cx.tree.worker_state(WorkerId::at(*task))?.is_terminal() {
            continue;
        }
        return Err(fault(TimeFault::CancelRaced {
            task: *task,
            region,
            deadline,
            now: cx.time.now,
        }));
    }
    Ok(())
}

/// An event that is a task's own work, in any family, is part of a poll of that task,
/// and the substrate's poll starts with `Cx::checkpoint`, which raises a passed
/// deadline first. So once the clock has reached a task's declared deadline, none of
/// the task's own work comes before its deadline's cancellation: a `begin`, `resume`,
/// `suspend`, `complete` or `fail`; a reserve, commit or explicit abort; an obligation
/// open, committed discharge or hand-off; a send or receive; a timer scheduled
/// ([`TimeFault::DeadlineIgnored`]; cr-3pu5cu round 6, pre-review pass). Cleanup steps
/// (a cancellation's abort, an aborted discharge, a cancelled timer, an abandonment)
/// are the cancellation's own and are not checked here. A journal without this family
/// shows no clock.
pub(crate) fn check_actor_deadline(body: &EventBody, cx: &LiftContext) -> Result<(), LiftStop> {
    if !cx.time.present || cx.time.deadlines.is_empty() {
        return Ok(());
    }
    match body.own_work_of(cx) {
        Some(task) => check_deadline_not_due(cx, task),
        None => Ok(()),
    }
}

/// A step of `task`'s own work is (part of) a poll that is not its deadline's
/// cancellation: refused once the clock has reached the task's declared deadline
/// ([`TimeFault::DeadlineIgnored`]). A journal without this family shows no clock.
pub(crate) fn check_deadline_not_due(cx: &LiftContext, task: u32) -> Result<(), LiftStop> {
    match cx.time.deadlines.get(&task) {
        Some(deadline) if cx.time.present && *deadline <= cx.time.now => {
            Err(fault(TimeFault::DeadlineIgnored {
                task,
                deadline: *deadline,
                now: cx.time.now,
            }))
        }
        _ => Ok(()),
    }
}

/// The task that armed `timer`, if the lift knows the timer.
pub(crate) fn task_of(cx: &LiftContext, timer: u32) -> Option<u32> {
    cx.time.timers.get(&timer).map(|entry| entry.task)
}

/// The first timer `task` sleeps on (scheduled, not fired or cancelled), if any.
pub(crate) fn armed_for(cx: &LiftContext, task: u32) -> Option<u32> {
    cx.time
        .timers
        .iter()
        .find(|(_, entry)| entry.task == task && entry.state == State::Scheduled)
        .map(|(timer, _)| *timer)
}

/// A task may resume only when no timer it sleeps on is still scheduled. A timer
/// already due is [`TimeFault::Late`] (it should have fired first); one not yet due is
/// [`TimeFault::WokenBeforeFire`].
pub(crate) fn check_not_asleep(cx: &LiftContext, task: u32) -> Result<(), LiftStop> {
    let Some(timer) = armed_for(cx, task) else {
        return Ok(());
    };
    let deadline = cx.time.timers.get(&timer).map_or(0, |entry| entry.deadline);
    if deadline <= cx.time.now {
        return Err(fault(TimeFault::Late {
            timer,
            deadline,
            now: cx.time.now,
        }));
    }
    Err(fault(TimeFault::WokenBeforeFire { task, timer }))
}

/// A task that completes as cancelled sleeps on no timer; one still scheduled is
/// [`TimeFault::OutlivesTask`].
pub(crate) fn check_none_armed(cx: &LiftContext, task: u32) -> Result<(), LiftStop> {
    match armed_for(cx, task) {
        Some(timer) => Err(fault(TimeFault::OutlivesTask { timer, task })),
        None => Ok(()),
    }
}

/// The whole-journal check, run after the last event: no timer is late, and no timer
/// outlives the task that sleeps on it.
pub(crate) fn finish(cx: &LiftContext) -> Result<(), LiftStop> {
    // A timer a crash still owes a fence is the truncated crash's, which the lifecycle
    // family's finish reports incomplete; it is neither late nor outliving (bn-20d8u).
    // A crash is refused over a late timer, so an owed timer is never late here.
    let owed = |timer: &u32| cx.fences.timers.contains(timer);
    none_late(&cx.time)?;
    for (timer, entry) in &cx.time.timers {
        if entry.state == State::Scheduled
            && !owed(timer)
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
