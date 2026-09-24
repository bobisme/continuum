//! Event families: the extension point the sibling PR-14 bullets fill.
//!
//! # The six families
//!
//! START_HERE PR 14 lists six primitive groups to instrument. Each is one family here,
//! with a stable one-byte tag, a stable token, and the requirement id that owns it:
//!
//! | tag | family | token | owner |
//! |---|---|---|---|
//! | 1 | [`Family::Lifecycle`] | `lifecycle` | PR-14-IMPL-01 (this bone, bn-lf4i) |
//! | 2 | [`Family::Effect`] | `reserve-commit-abort` | PR-14-IMPL-02 (bn-gzy1) |
//! | 3 | [`Family::Cancellation`] | `cancellation` | PR-14-IMPL-03 (bn-bx7i) |
//! | 4 | [`Family::Obligation`] | `obligation` | PR-14-IMPL-04 (bn-6nm8) |
//! | 5 | [`Family::Time`] | `virtual-time` | PR-14-IMPL-05 (bn-3m1d) |
//! | 6 | [`Family::Channel`] | `channel` | PR-14-IMPL-06 (bn-3xx9) |
//!
//! The tags are part of the canonical encoding and never move. A seventh family (for
//! example PR 15's fault events) takes tag 7 and a new line in each table below.
//!
//! # How a sibling bullet extends the journal without editing these lines
//!
//! Every family already has its own file under `src/family/`, and every dispatch
//! below already routes to it. A family whose instrumentation has not landed has an
//! **uninhabited** event type and an uninhabited report type, so no event of it can
//! exist, its encoder and lifter are `match *event {}`, and its decoder returns the
//! typed [`DecodeError::FamilyNotInstrumented`]. To land, say, IMPL-02, a bone edits
//! **only `src/family/effect.rs`**:
//!
//! 1. give `EffectEvent` and `EffectReport` their variants, with a tag table;
//! 2. fill `encode`, `decode`, `render`, `lift`, and `record` for them;
//! 3. set `INSTRUMENTED = true`;
//! 4. add its own tests file.
//!
//! The journal header, the event framing, the sequence numbering, the digest, the choice
//! log, the scripted source's scheduling, and the lift's verdict type do not change. A
//! family that needs recorder or lift state keeps it in its own `RecordState` /
//! `LiftState` type, which the shared state already holds one of.
//!
//! [`DecodeError::FamilyNotInstrumented`]: crate::encoding::DecodeError::FamilyNotInstrumented

pub mod cancellation;
pub mod channel;
pub mod effect;
pub mod lifecycle;
pub mod obligation;
pub mod time;

use core::fmt;

use crate::encoding::{DecodeError, Decoder, EncodeError, Encoder};
use crate::lift::{LiftContext, LiftStop};
use crate::source::{RecordContext, RecordRefusal};

/// One of the six PR-14 primitive families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Family {
    /// Task and region lifecycle (PR-14-IMPL-01).
    Lifecycle,
    /// Reserve / commit / abort (PR-14-IMPL-02).
    Effect,
    /// Cancellation phases (PR-14-IMPL-03).
    Cancellation,
    /// Obligations (PR-14-IMPL-04).
    Obligation,
    /// Virtual time (PR-14-IMPL-05).
    Time,
    /// Channel communication (PR-14-IMPL-06).
    Channel,
}

impl Family {
    /// Every family, in tag order.
    pub const ALL: [Self; 6] = [
        Self::Lifecycle,
        Self::Effect,
        Self::Cancellation,
        Self::Obligation,
        Self::Time,
        Self::Channel,
    ];

    /// The family's byte in the canonical encoding.
    #[must_use]
    pub const fn tag(self) -> u8 {
        match self {
            Self::Lifecycle => 1,
            Self::Effect => 2,
            Self::Cancellation => 3,
            Self::Obligation => 4,
            Self::Time => 5,
            Self::Channel => 6,
        }
    }

    /// The family with this tag, if any.
    #[must_use]
    pub const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::Lifecycle),
            2 => Some(Self::Effect),
            3 => Some(Self::Cancellation),
            4 => Some(Self::Obligation),
            5 => Some(Self::Time),
            6 => Some(Self::Channel),
            _ => None,
        }
    }

    /// A stable token for rendering and refusals.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Lifecycle => "lifecycle",
            Self::Effect => "reserve-commit-abort",
            Self::Cancellation => "cancellation",
            Self::Obligation => "obligation",
            Self::Time => "virtual-time",
            Self::Channel => "channel",
        }
    }

    /// The START_HERE requirement id that owns this family's instrumentation.
    #[must_use]
    pub const fn requirement(self) -> &'static str {
        match self {
            Self::Lifecycle => "PR-14-IMPL-01",
            Self::Effect => "PR-14-IMPL-02",
            Self::Cancellation => "PR-14-IMPL-03",
            Self::Obligation => "PR-14-IMPL-04",
            Self::Time => "PR-14-IMPL-05",
            Self::Channel => "PR-14-IMPL-06",
        }
    }

    /// Whether this family's events exist yet. Read from the family's own file, so
    /// landing a family flips this without editing this line.
    #[must_use]
    pub const fn is_instrumented(self) -> bool {
        match self {
            Self::Lifecycle => lifecycle::INSTRUMENTED,
            Self::Effect => effect::INSTRUMENTED,
            Self::Cancellation => cancellation::INSTRUMENTED,
            Self::Obligation => obligation::INSTRUMENTED,
            Self::Time => time::INSTRUMENTED,
            Self::Channel => channel::INSTRUMENTED,
        }
    }
}

impl fmt::Display for Family {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.token())
    }
}

/// The payload of one semantic event: exactly one family's event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventBody {
    /// A task or region lifecycle event.
    Lifecycle(lifecycle::LifecycleEvent),
    /// A reserve / commit / abort event (PR-14-IMPL-02, bn-gzy1).
    Effect(effect::EffectEvent),
    /// A cancellation-phase event (PR-14-IMPL-03, bn-bx7i).
    Cancellation(cancellation::CancellationEvent),
    /// An obligation-ledger event (PR-14-IMPL-04, bn-6nm8).
    Obligation(obligation::ObligationEvent),
    /// A virtual-time event (PR-14-IMPL-05, bn-3m1d).
    Time(time::TimeEvent),
    /// A channel event (PR-14-IMPL-06, bn-3xx9).
    Channel(channel::ChannelEvent),
}

impl EventBody {
    /// The family this event belongs to.
    #[must_use]
    pub const fn family(&self) -> Family {
        match self {
            Self::Lifecycle(_) => Family::Lifecycle,
            Self::Effect(_) => Family::Effect,
            Self::Cancellation(_) => Family::Cancellation,
            Self::Obligation(_) => Family::Obligation,
            Self::Time(_) => Family::Time,
            Self::Channel(_) => Family::Channel,
        }
    }

    pub(crate) fn encode(&self, out: &mut Encoder) -> Result<(), EncodeError> {
        match self {
            Self::Lifecycle(event) => lifecycle::encode(event, out),
            Self::Effect(event) => effect::encode(event, out),
            Self::Cancellation(event) => cancellation::encode(event, out),
            Self::Obligation(event) => obligation::encode(event, out),
            Self::Time(event) => time::encode(event, out),
            Self::Channel(event) => channel::encode(event, out),
        }
    }

    pub(crate) fn decode(
        family: Family,
        input: &mut Decoder<'_>,
        seq: u64,
    ) -> Result<Self, DecodeError> {
        Ok(match family {
            Family::Lifecycle => Self::Lifecycle(lifecycle::decode(input, seq)?),
            Family::Effect => Self::Effect(effect::decode(input, seq)?),
            Family::Cancellation => Self::Cancellation(cancellation::decode(input, seq)?),
            Family::Obligation => Self::Obligation(obligation::decode(input, seq)?),
            Family::Time => Self::Time(time::decode(input, seq)?),
            Family::Channel => Self::Channel(channel::decode(input, seq)?),
        })
    }

    /// A canonical one-line rendering, prefixed with the family token.
    #[must_use]
    pub fn render(&self) -> String {
        let body = match self {
            Self::Lifecycle(event) => lifecycle::render(event),
            Self::Effect(event) => effect::render(event),
            Self::Cancellation(event) => cancellation::render(event),
            Self::Obligation(event) => obligation::render(event),
            Self::Time(event) => time::render(event),
            Self::Channel(event) => channel::render(event),
        };
        format!("{} {body}", self.family())
    }

    /// The event's own token within its family.
    pub(crate) const fn token(&self) -> &'static str {
        match self {
            Self::Lifecycle(event) => event.token(),
            Self::Effect(event) => event.token(),
            Self::Cancellation(event) => event.token(),
            Self::Obligation(event) => event.token(),
            Self::Time(event) => event.token(),
            Self::Channel(event) => event.token(),
        }
    }

    /// The task whose own work this event is, if it is any task's: a `begin`, `resume`,
    /// `suspend`, `complete` or `fail`; a reserve, commit or explicit abort; an
    /// obligation open, committed discharge or hand-off (by the giving holder); a timer
    /// scheduled or fired (the fire is traced in the sleeper's own poll); a send or a
    /// receive. Each is part of a poll of that task, and a bound
    /// task's poll starts with `Cx::checkpoint`. Cleanup steps, receipts, and events
    /// that name no acting task are `None`. One classification for every rule that
    /// turns on "the task acts" (RFC 0026 corrections 53 item 6 and 55).
    pub(crate) fn own_work_of(&self, cx: &LiftContext) -> Option<u32> {
        use lifecycle::{LifecycleEvent, TaskStep};
        match self {
            Self::Lifecycle(LifecycleEvent::TaskStepped {
                task,
                step:
                    TaskStep::Begin
                    | TaskStep::Resume
                    | TaskStep::Suspend
                    | TaskStep::Complete
                    | TaskStep::Fail(_),
            }) => Some(task.0),
            Self::Lifecycle(_) | Self::Cancellation(_) => None,
            Self::Effect(event) => effect::actor_of(cx, event),
            Self::Obligation(event) => obligation::actor_of(cx, event),
            Self::Time(time::TimeEvent::Scheduled { task, .. }) => Some(task.0),
            // A fire is traced by the `Sleep` future inside its task's own poll, which
            // starts with `Cx::checkpoint` (bn-28hup, pre-review pass).
            Self::Time(time::TimeEvent::Fired { timer, .. }) => time::task_of(cx, timer.0),
            Self::Time(_) => None,
            Self::Channel(event) => channel::actor_of(cx, event),
        }
    }

    pub(crate) fn lift(&self, cx: &mut LiftContext) -> Result<(), LiftStop> {
        // A fail-stop crash's fences come right after it, before anything else
        // (bn-20d8u; RFC 0026 correction 58).
        lifecycle::check_fences_first(self, cx)?;
        // A task's own cancellation is one run of that task's events, whatever family
        // an event belongs to (RFC 0026 correction 53 item 6; cr-3pu5cu round 6).
        cancellation::check_own_cancel_admits(self, cx)?;
        // A task's own work is a poll, which meets a passed deadline first (RFC 0026
        // correction 53 item 6).
        time::check_actor_deadline(self, cx)?;
        // A task whose cancellation was requested and not yet acknowledged takes no
        // step: a bound task's next poll observes the request first (RFC 0026
        // correction 55; bn-28hup).
        cancellation::check_before_acknowledgement(self, cx)?;
        match self {
            Self::Lifecycle(event) => lifecycle::lift(event, cx),
            Self::Effect(event) => effect::lift(event, cx),
            Self::Cancellation(event) => cancellation::lift(event, cx),
            Self::Obligation(event) => obligation::lift(event, cx),
            Self::Time(event) => time::lift(event, cx),
            Self::Channel(event) => channel::lift(event, cx),
        }
    }
}

/// What a scripted source reports one primitive did: one family's report, or a named
/// primitive of a family whose instrumentation has not landed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Report {
    /// A task or region lifecycle primitive.
    Lifecycle(lifecycle::LifecycleReport),
    /// A reserve / commit / abort step (PR-14-IMPL-02, bn-gzy1).
    Effect(effect::EffectReport),
    /// A cancellation phase (PR-14-IMPL-03, bn-bx7i).
    Cancellation(cancellation::CancellationReport),
    /// An obligation-ledger step (PR-14-IMPL-04, bn-6nm8).
    Obligation(obligation::ObligationReport),
    /// A virtual-time step (PR-14-IMPL-05, bn-3m1d).
    Time(time::TimeReport),
    /// A channel step (PR-14-IMPL-06, bn-3xx9).
    Channel(channel::ChannelReport),
    /// A primitive the source names but no family instruments yet. Recording it is the
    /// typed refusal [`RecordRefusal::UnsupportedPrimitive`], never a dropped event.
    Uninstrumented {
        /// The family the primitive belongs to.
        family: Family,
        /// The primitive's name, as the source spells it.
        operation: String,
    },
}

impl Report {
    pub(crate) fn record(&self, cx: &mut RecordContext) -> Result<(), RecordRefusal> {
        match self {
            Self::Lifecycle(report) => lifecycle::record(report, cx),
            Self::Effect(report) => effect::record(report, cx),
            Self::Cancellation(report) => cancellation::record(report, cx),
            Self::Obligation(report) => obligation::record(report, cx),
            Self::Time(report) => time::record(report, cx),
            Self::Channel(report) => channel::record(report, cx),
            Self::Uninstrumented { family, operation } => {
                Err(RecordRefusal::UnsupportedPrimitive {
                    family: *family,
                    operation: operation.clone(),
                })
            }
        }
    }
}
