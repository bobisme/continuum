//! The channel communication family (PR-14-IMPL-06, bn-3xx9).
//!
//! # The vocabulary, and where it comes from
//!
//! asupersync 0.5.0's bounded `channel::mpsc` is a two-phase channel: a sender reserves a
//! slot (`Sender::reserve_checked`, which registers a `SendPermit` obligation through the
//! sender's `Cx`) and then sends into it (`SendPermit::send`, which commits the permit).
//! A full channel makes the reserve wait (backpressure); an empty one makes a receive
//! wait. The channel itself writes nothing into the substrate's trace; only the
//! permit's obligation steps reach it. So the binding observes a channel two ways:
//!
//! | event | source |
//! |---|---|
//! | [`ChannelEvent::Opened`] | the binding's `mpsc::channel(capacity)` call, whose receiver it hands to one task |
//! | [`ChannelEvent::Sent`] | the substrate's trace: the sender's `SendPermit` obligation commits |
//! | [`ChannelEvent::SendBlocked`] | the channel gate: the reserve returned `Pending` (the channel was full) |
//! | [`ChannelEvent::SendClosed`] | the channel gate: the reserve returned the channel's closed error |
//! | [`ChannelEvent::SendAbandoned`] | the channel gate: the blocked sender's cancellation dropped its reserve |
//! | [`ChannelEvent::Received`] | the channel gate: `Receiver::recv` returned a value, which is the message ordinal |
//! | [`ChannelEvent::RecvBlocked`] | the channel gate: the receive returned `Pending` (the channel was empty) |
//! | [`ChannelEvent::RecvClosed`] | the channel gate: the receive returned `Disconnected` |
//! | [`ChannelEvent::RecvAbandoned`] | the channel gate: the blocked receiver's cancellation dropped its receive |
//! | [`ChannelEvent::SendersClosed`] | the binding dropped the last sender it keeps |
//! | [`ChannelEvent::ReceiverGone`] | the channel gate: the receiving task ended and dropped its receiver, with what was still queued |
//!
//! The channel gate's marks are written into the substrate's trace through `Cx::trace`
//! at the moment the substrate's call returns, in sequence with the substrate's own
//! events, as the lifecycle gate's marks are.
//!
//! # Message identity
//!
//! A message is a dense ordinal the binding allocates when a send is commanded, and the
//! ordinal is the payload itself. A received message is named by the value the
//! substrate delivered, never by a program label: labels never reach the journal
//! (bn-iey9f).
//!
//! # How the events relate to the region calculus
//!
//! `continuum_task::region` has no channels. The lift keeps a parallel, checked model of
//! each channel (`lift`, `finish`) and ties it to the calculus's tasks:
//!
//! 1. a channel opens once, with a positive capacity and a live receiving task;
//! 2. FIFO: a message is sent once, into a channel whose receiver is still there and
//!    that has room, and is received only when it is the oldest message queued. None
//!    is delivered twice; none disappears except when the receiver goes, which declares
//!    the queued messages it drops;
//! 3. backpressure: a send blocks only on a full channel, a receive only on an empty one
//!    that can still receive; a send fails as closed only after the receiver is gone,
//!    and a receive returns closed only on an empty channel with no sender left;
//! 4. an abandoned send or receive is one that was blocked, abandoned by a cancellation
//!    of a live task whose region drains under cancellation;
//! 5. every channel whose receiving task the model terminated has seen its receiver go;
//! 6. (bn-1i050) a fresh send is the next dense message ordinal, through a kept
//!    sender, by a sender with no other send waiting; with the obligation family, each
//!    send consumes one committed `SendPermit` of its sender; receives and a normal
//!    receiver drop are by a running receiver whose cancellation has not begun; an
//!    abandonment, or a receiver drop in cancellation, follows the task's
//!    acknowledgement; and a task completes as cancelled holding no receiver and no
//!    blocked send.
//!
//! # Identity
//!
//! Channels and messages are named by dense ordinals in journal allocation order. Tasks
//! are the lifecycle family's [`TaskOrdinal`]s.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use continuum_task::region::worker::{CancelPhase, WorkerId, WorkerState};

use crate::encoding::{DecodeError, Decoder, EncodeError, Encoder};
use crate::family::EventBody;
use crate::family::lifecycle::TaskOrdinal;
use crate::lift::{LiftContext, LiftStop, Nonconformance};
use crate::source::{RecordContext, RecordRefusal};

/// Whether this family's events exist yet.
pub const INSTRUMENTED: bool = true;

/// A channel, by its position in the journal's allocation order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChannelOrdinal(pub u32);

/// A program's local name for a channel. It never reaches the journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChannelLabel(pub u32);

/// A message, by the ordinal its send allocated. It is the payload the channel carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MessageOrdinal(pub u64);

/// A set of messages, held strictly ascending so it has one spelling.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MessageSet(Vec<MessageOrdinal>);

impl MessageSet {
    /// The set of these messages, in canonical order.
    #[must_use]
    pub fn new(members: impl IntoIterator<Item = MessageOrdinal>) -> Self {
        let mut members: Vec<MessageOrdinal> = members.into_iter().collect();
        members.sort_unstable();
        members.dedup();
        Self(members)
    }

    /// The members, ascending.
    #[must_use]
    pub fn as_slice(&self) -> &[MessageOrdinal] {
        &self.0
    }
}

/// One channel event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelEvent {
    /// A bounded channel opened, its receiver held by `receiver`.
    Opened {
        /// The channel.
        channel: ChannelOrdinal,
        /// Its capacity.
        capacity: u32,
        /// The task that receives from it.
        receiver: TaskOrdinal,
    },
    /// `sender` sent `message`: the permit committed and the message is queued.
    Sent {
        /// The channel.
        channel: ChannelOrdinal,
        /// The message.
        message: MessageOrdinal,
        /// The sending task.
        sender: TaskOrdinal,
    },
    /// `sender`'s send of `message` waits for room.
    SendBlocked {
        /// The channel.
        channel: ChannelOrdinal,
        /// The message.
        message: MessageOrdinal,
        /// The sending task.
        sender: TaskOrdinal,
    },
    /// `sender`'s send of `message` failed: the receiver is gone.
    SendClosed {
        /// The channel.
        channel: ChannelOrdinal,
        /// The message.
        message: MessageOrdinal,
        /// The sending task.
        sender: TaskOrdinal,
    },
    /// `sender`'s blocked send of `message` was abandoned by its cancellation.
    SendAbandoned {
        /// The channel.
        channel: ChannelOrdinal,
        /// The message.
        message: MessageOrdinal,
        /// The sending task.
        sender: TaskOrdinal,
    },
    /// The receiver took `message`.
    Received {
        /// The channel.
        channel: ChannelOrdinal,
        /// The message.
        message: MessageOrdinal,
    },
    /// The receive waits for a message.
    RecvBlocked {
        /// The channel.
        channel: ChannelOrdinal,
    },
    /// The receive returned closed: the channel is empty and no sender is left.
    RecvClosed {
        /// The channel.
        channel: ChannelOrdinal,
    },
    /// The blocked receive was abandoned by the receiver's cancellation.
    RecvAbandoned {
        /// The channel.
        channel: ChannelOrdinal,
    },
    /// The last sender the binding keeps was dropped.
    SendersClosed {
        /// The channel.
        channel: ChannelOrdinal,
    },
    /// The receiving task ended and dropped its receiver, and with it these queued
    /// messages: a declared drop.
    ReceiverGone {
        /// The channel.
        channel: ChannelOrdinal,
        /// The messages still queued.
        discarded: MessageSet,
    },
}

impl ChannelEvent {
    const fn tag(&self) -> u8 {
        match self {
            Self::Opened { .. } => 1,
            Self::Sent { .. } => 2,
            Self::SendBlocked { .. } => 3,
            Self::SendClosed { .. } => 4,
            Self::SendAbandoned { .. } => 5,
            Self::Received { .. } => 6,
            Self::RecvBlocked { .. } => 7,
            Self::RecvClosed { .. } => 8,
            Self::RecvAbandoned { .. } => 9,
            Self::SendersClosed { .. } => 10,
            Self::ReceiverGone { .. } => 11,
        }
    }

    pub(crate) const fn token(&self) -> &'static str {
        match self {
            Self::Opened { .. } => "opened",
            Self::Sent { .. } => "sent",
            Self::SendBlocked { .. } => "send-blocked",
            Self::SendClosed { .. } => "send-closed",
            Self::SendAbandoned { .. } => "send-abandoned",
            Self::Received { .. } => "received",
            Self::RecvBlocked { .. } => "recv-blocked",
            Self::RecvClosed { .. } => "recv-closed",
            Self::RecvAbandoned { .. } => "recv-abandoned",
            Self::SendersClosed { .. } => "senders-closed",
            Self::ReceiverGone { .. } => "receiver-gone",
        }
    }

    /// The channel the event is about.
    #[must_use]
    pub const fn channel(&self) -> ChannelOrdinal {
        match self {
            Self::Opened { channel, .. }
            | Self::Sent { channel, .. }
            | Self::SendBlocked { channel, .. }
            | Self::SendClosed { channel, .. }
            | Self::SendAbandoned { channel, .. }
            | Self::Received { channel, .. }
            | Self::RecvBlocked { channel }
            | Self::RecvClosed { channel }
            | Self::RecvAbandoned { channel }
            | Self::SendersClosed { channel }
            | Self::ReceiverGone { channel, .. } => *channel,
        }
    }
}

pub(crate) fn encode(event: &ChannelEvent, out: &mut Encoder) -> Result<(), EncodeError> {
    out.tag(event.tag());
    out.u32(event.channel().0);
    match event {
        ChannelEvent::Opened {
            capacity, receiver, ..
        } => {
            out.u32(*capacity);
            out.u32(receiver.0);
        }
        ChannelEvent::Sent {
            message, sender, ..
        }
        | ChannelEvent::SendBlocked {
            message, sender, ..
        }
        | ChannelEvent::SendClosed {
            message, sender, ..
        }
        | ChannelEvent::SendAbandoned {
            message, sender, ..
        } => {
            out.u64(message.0);
            out.u32(sender.0);
        }
        ChannelEvent::Received { message, .. } => out.u64(message.0),
        ChannelEvent::RecvBlocked { .. }
        | ChannelEvent::RecvClosed { .. }
        | ChannelEvent::RecvAbandoned { .. }
        | ChannelEvent::SendersClosed { .. } => {}
        ChannelEvent::ReceiverGone { discarded, .. } => {
            let count =
                u32::try_from(discarded.0.len()).map_err(|_| EncodeError::FieldTooLong {
                    len: discarded.0.len(),
                })?;
            out.u32(count);
            for message in &discarded.0 {
                out.u64(message.0);
            }
        }
    }
    Ok(())
}

pub(crate) fn decode(input: &mut Decoder<'_>, _seq: u64) -> Result<ChannelEvent, DecodeError> {
    let at = input.offset();
    let tag = input.tag()?;
    if !(1..=11).contains(&tag) {
        return Err(DecodeError::UnknownTag {
            table: "channel event",
            tag,
            at,
        });
    }
    let channel = ChannelOrdinal(input.u32()?);
    let send = |input: &mut Decoder<'_>| -> Result<(MessageOrdinal, TaskOrdinal), DecodeError> {
        Ok((MessageOrdinal(input.u64()?), TaskOrdinal(input.u32()?)))
    };
    Ok(match tag {
        1 => ChannelEvent::Opened {
            channel,
            capacity: input.u32()?,
            receiver: TaskOrdinal(input.u32()?),
        },
        2 => {
            let (message, sender) = send(input)?;
            ChannelEvent::Sent {
                channel,
                message,
                sender,
            }
        }
        3 => {
            let (message, sender) = send(input)?;
            ChannelEvent::SendBlocked {
                channel,
                message,
                sender,
            }
        }
        4 => {
            let (message, sender) = send(input)?;
            ChannelEvent::SendClosed {
                channel,
                message,
                sender,
            }
        }
        5 => {
            let (message, sender) = send(input)?;
            ChannelEvent::SendAbandoned {
                channel,
                message,
                sender,
            }
        }
        6 => ChannelEvent::Received {
            channel,
            message: MessageOrdinal(input.u64()?),
        },
        7 => ChannelEvent::RecvBlocked { channel },
        8 => ChannelEvent::RecvClosed { channel },
        9 => ChannelEvent::RecvAbandoned { channel },
        10 => ChannelEvent::SendersClosed { channel },
        _ => {
            let set_at = input.offset();
            let count = input.u32()?;
            let mut members: Vec<MessageOrdinal> = Vec::new();
            for _ in 0..count {
                let member = MessageOrdinal(input.u64()?);
                if members.last().is_some_and(|last| *last >= member) {
                    return Err(DecodeError::UnsortedSet { at: set_at });
                }
                members.push(member);
            }
            ChannelEvent::ReceiverGone {
                channel,
                discarded: MessageSet(members),
            }
        }
    })
}

pub(crate) fn render(event: &ChannelEvent) -> String {
    let c = event.channel().0;
    match event {
        ChannelEvent::Opened {
            capacity, receiver, ..
        } => format!("opened c{c} capacity={capacity} receiver=t{}", receiver.0),
        ChannelEvent::Sent {
            message, sender, ..
        }
        | ChannelEvent::SendBlocked {
            message, sender, ..
        }
        | ChannelEvent::SendClosed {
            message, sender, ..
        }
        | ChannelEvent::SendAbandoned {
            message, sender, ..
        } => format!("{} c{c} m{} sender=t{}", event.token(), message.0, sender.0),
        ChannelEvent::Received { message, .. } => format!("received c{c} m{}", message.0),
        ChannelEvent::ReceiverGone { discarded, .. } => {
            let members: Vec<String> = discarded.0.iter().map(|m| format!("m{}", m.0)).collect();
            format!("receiver-gone c{c} discarded=[{}]", members.join(","))
        }
        other => format!("{} c{c}", other.token()),
    }
}

// --- lift ----------------------------------------------------------------------------

#[derive(Debug)]
struct Model {
    capacity: usize,
    receiver: u32,
    queue: VecDeque<u64>,
    receiver_present: bool,
    senders_closed: bool,
    blocked_sends: BTreeMap<u64, u32>,
    receive_blocked: bool,
}

/// Why a channel event does not conform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelFault {
    /// A new channel is not named by the next ordinal, or has no capacity.
    BadOpen {
        /// The channel.
        channel: u32,
    },
    /// The event names a channel no earlier event opened.
    UnknownChannel {
        /// The channel.
        channel: u32,
    },
    /// A message was sent, blocked or failed twice, or its send was already settled.
    MessageReused {
        /// The channel.
        channel: u32,
        /// The message.
        message: u64,
    },
    /// A message was sent into a full channel.
    Overflow {
        /// The channel.
        channel: u32,
        /// The message.
        message: u64,
    },
    /// A message was received that is not the oldest queued one (reordered, duplicated
    /// or never sent).
    NotFifo {
        /// The channel.
        channel: u32,
        /// The message received.
        received: u64,
        /// The oldest queued message, if any.
        expected: Option<u64>,
    },
    /// The event's precondition on the channel's state does not hold.
    StateMismatch {
        /// The channel.
        channel: u32,
        /// The event's token.
        event: &'static str,
    },
    /// The task the event names is terminal in the model.
    TaskTerminal {
        /// The channel.
        channel: u32,
        /// The event's token.
        event: &'static str,
    },
    /// An abandonment by a task no cancellation reached: its region is not draining
    /// under cancellation and it requested none of its own (a deadline; bn-36wy3).
    NotCancelling {
        /// The channel.
        channel: u32,
        /// The event's token.
        event: &'static str,
    },
    /// The receiver went with a different set of queued messages than the model's.
    DropMismatch {
        /// The channel.
        channel: u32,
        /// The messages the event declares.
        declared: Vec<u64>,
        /// The messages still queued in the model.
        queued: Vec<u64>,
    },
    /// A channel's receiving task ended (the model terminated it, or it completed as
    /// cancelled), and its receiver never went.
    ReceiverOutlivesTask {
        /// The channel.
        channel: u32,
    },
    /// A fresh message is not named by the next message ordinal (the binding allocates
    /// them densely, one per commanded send), or its sender could not send it: no kept
    /// sender remains, or the sender already waits on another send (bn-1i050).
    MessageIdentity {
        /// The channel.
        channel: u32,
        /// The message.
        message: u64,
        /// The next message ordinal.
        expected: u64,
    },
    /// A send, receive or receiver drop by a task that is not running its own call: it
    /// is parked, or its cancellation has begun (bn-1i050).
    NotActing {
        /// The channel.
        channel: u32,
        /// The event's token.
        event: &'static str,
    },
    /// An abandonment by a task whose reported cancellation phase is not
    /// `acknowledged`: the drop is a `Cancelling` cleanup step (docs/02 §7; bn-1i050).
    OutsideCancelling {
        /// The channel.
        channel: u32,
        /// The event's token.
        event: &'static str,
        /// The task's cancellation phase.
        phase: &'static str,
    },
    /// A send with no committed `SendPermit` of its sender: the send is the permit's
    /// commit (docs/02 §7 two-phase effect; bn-1i050).
    NoSendPermit {
        /// The channel.
        channel: u32,
        /// The message.
        message: u64,
    },
    /// A task ended while one of its sends was still blocked (bn-1i050).
    SendOutlivesTask {
        /// The channel.
        channel: u32,
        /// The message.
        message: u64,
    },
}

impl fmt::Display for ChannelFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadOpen { channel } => write!(f, "c{channel} is not a well-formed new channel"),
            Self::UnknownChannel { channel } => write!(f, "c{channel} was never opened"),
            Self::MessageReused { channel, message } => {
                write!(f, "m{message} on c{channel} was already sent or settled")
            }
            Self::Overflow { channel, message } => {
                write!(f, "m{message} was sent into full channel c{channel}")
            }
            Self::NotFifo {
                channel,
                received,
                expected,
            } => write!(
                f,
                "c{channel} delivered m{received} but the oldest queued is {expected:?}"
            ),
            Self::StateMismatch { channel, event } => {
                write!(f, "c{channel}'s state does not admit {event}")
            }
            Self::TaskTerminal { channel, event } => {
                write!(f, "{event} on c{channel} names a terminal task")
            }
            Self::NotCancelling { channel, event } => write!(
                f,
                "{event} on c{channel} but no cancellation reached its task"
            ),
            Self::DropMismatch {
                channel,
                declared,
                queued,
            } => write!(
                f,
                "c{channel}'s receiver dropped {declared:?} but {queued:?} were queued"
            ),
            Self::ReceiverOutlivesTask { channel } => {
                write!(
                    f,
                    "c{channel}'s receiving task ended but its receiver never went"
                )
            }
            Self::MessageIdentity {
                channel,
                message,
                expected,
            } => write!(
                f,
                "m{message} on c{channel} is not a fresh send (the next message is m{expected})"
            ),
            Self::NotActing { channel, event } => {
                write!(
                    f,
                    "{event} on c{channel} by a task that is not running its call"
                )
            }
            Self::OutsideCancelling {
                channel,
                event,
                phase,
            } => write!(
                f,
                "{event} on c{channel} but the task's cancellation is {phase}"
            ),
            Self::NoSendPermit { channel, message } => write!(
                f,
                "m{message} on c{channel} was sent with no committed send permit"
            ),
            Self::SendOutlivesTask { channel, message } => {
                write!(
                    f,
                    "a task ended with its send of m{message} on c{channel} blocked"
                )
            }
        }
    }
}

/// Lift state: each channel's model, and every message ever sent or settled.
#[derive(Debug, Default)]
pub struct LiftState {
    channels: BTreeMap<u32, Model>,
    settled: BTreeSet<u64>,
    next_message: u64,
}

fn fault(fault: ChannelFault) -> LiftStop {
    LiftStop::Violation(Nonconformance::Channel(fault))
}

fn live(cx: &LiftContext, task: u32, channel: u32, event: &ChannelEvent) -> Result<(), LiftStop> {
    if cx.tree.worker_state(WorkerId::at(task))?.is_terminal() {
        return Err(fault(ChannelFault::TaskTerminal {
            channel,
            event: event.token(),
        }));
    }
    Ok(())
}

fn cancelling(
    cx: &LiftContext,
    task: u32,
    channel: u32,
    event: &ChannelEvent,
) -> Result<(), LiftStop> {
    live(cx, task, channel, event)?;
    if cx.tree.cancel_phase(WorkerId::at(task))? == CancelPhase::Active {
        return Err(fault(ChannelFault::NotCancelling {
            channel,
            event: event.token(),
        }));
    }
    Ok(())
}

/// The task acts on its own account: live, and its reported cancellation has not
/// reached `Cancelling`. With `running`, it is also running its call (bn-1i050).
fn acting(
    cx: &LiftContext,
    task: u32,
    running: bool,
    channel: u32,
    event: &ChannelEvent,
) -> Result<(), LiftStop> {
    let state = cx.tree.worker_state(WorkerId::at(task))?;
    if state.is_terminal()
        || (running && *state != WorkerState::Running)
        || crate::family::cancellation::is_cancelling(cx, task)
    {
        return Err(fault(ChannelFault::NotActing {
            channel,
            event: event.token(),
        }));
    }
    Ok(())
}

/// A cancellation cleanup step by `task` comes after its acknowledgement, when the
/// journal reports its phases (docs/02 §7; bn-1i050).
fn after_acknowledgement(
    cx: &LiftContext,
    task: u32,
    channel: u32,
    event: &ChannelEvent,
) -> Result<(), LiftStop> {
    match crate::family::cancellation::outside_cancelling(cx, task) {
        Some(phase) => Err(fault(ChannelFault::OutsideCancelling {
            channel,
            event: event.token(),
            phase,
        })),
        None => Ok(()),
    }
}

/// The task whose own work `event` is: the sender of a send, or the receiver of a
/// receive. `None` for a channel's open, an abandonment or a receiver's drop (cleanup),
/// a senders' close, and a channel the lift does not know.
pub(crate) fn actor_of(cx: &LiftContext, event: &ChannelEvent) -> Option<u32> {
    match event {
        ChannelEvent::Sent { sender, .. }
        | ChannelEvent::SendBlocked { sender, .. }
        | ChannelEvent::SendClosed { sender, .. } => Some(sender.0),
        ChannelEvent::Received { channel, .. }
        | ChannelEvent::RecvBlocked { channel }
        | ChannelEvent::RecvClosed { channel } => cx
            .channel
            .channels
            .get(&channel.0)
            .map(|model| model.receiver),
        _ => None,
    }
}

/// The task whose cancellation cleanup `event` is: the sender of an abandoned send, or
/// the receiver of the channel an abandoned receive or a receiver's drop names. `None`
/// for every other event, and for a channel the lift does not know (RFC 0026
/// correction 53 item 6).
pub(crate) fn cleanup_of(cx: &LiftContext, event: &ChannelEvent) -> Option<u32> {
    match event {
        ChannelEvent::SendAbandoned { sender, .. } => Some(sender.0),
        ChannelEvent::RecvAbandoned { channel } | ChannelEvent::ReceiverGone { channel, .. } => cx
            .channel
            .channels
            .get(&channel.0)
            .map(|model| model.receiver),
        _ => None,
    }
}

/// The task that holds `channel`'s receiver, if the lift knows the channel.
pub(crate) fn receiver_of(cx: &LiftContext, channel: u32) -> Option<u32> {
    cx.channel
        .channels
        .get(&channel)
        .map(|model| model.receiver)
}

/// A task that completes as cancelled holds no channel's receiver and no blocked
/// send.
pub(crate) fn check_none_held(cx: &LiftContext, task: u32) -> Result<(), LiftStop> {
    for (channel, model) in &cx.channel.channels {
        if model.receiver_present && model.receiver == task {
            return Err(fault(ChannelFault::ReceiverOutlivesTask {
                channel: *channel,
            }));
        }
        if let Some((message, _)) = model.blocked_sends.iter().find(|(_, s)| **s == task) {
            return Err(fault(ChannelFault::SendOutlivesTask {
                channel: *channel,
                message: *message,
            }));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
pub(crate) fn lift(event: &ChannelEvent, cx: &mut LiftContext) -> Result<(), LiftStop> {
    let channel = event.channel().0;
    if let ChannelEvent::Opened {
        capacity, receiver, ..
    } = event
    {
        let expected = u32::try_from(cx.channel.channels.len()).unwrap_or(u32::MAX);
        if channel != expected || *capacity == 0 {
            return Err(fault(ChannelFault::BadOpen { channel }));
        }
        live(cx, receiver.0, channel, event)?;
        // The receiver is handed to a task that acts on its own account: not one in
        // cancellation (a cancellation fact; cr-3pu5cu round 7, self-sweep, as the A7
        // model states it).
        acting(cx, receiver.0, false, channel, event)?;
        cx.channel.channels.insert(
            channel,
            Model {
                capacity: *capacity as usize,
                receiver: receiver.0,
                queue: VecDeque::new(),
                receiver_present: true,
                senders_closed: false,
                blocked_sends: BTreeMap::new(),
                receive_blocked: false,
            },
        );
        return Ok(());
    }
    let mismatch = || {
        fault(ChannelFault::StateMismatch {
            channel,
            event: event.token(),
        })
    };
    let Some(model) = cx.channel.channels.get(&channel) else {
        return Err(fault(ChannelFault::UnknownChannel { channel }));
    };
    let receiver = model.receiver;
    match event {
        ChannelEvent::Opened { .. } => unreachable!("handled above"),
        ChannelEvent::Sent {
            message, sender, ..
        }
        | ChannelEvent::SendBlocked {
            message, sender, ..
        }
        | ChannelEvent::SendClosed {
            message, sender, ..
        } => {
            live(cx, sender.0, channel, event)?;
            let was_blocked = model.blocked_sends.get(&message.0) == Some(&sender.0);
            let senders_closed = model.senders_closed;
            let sender_busy = cx.channel.channels.values().any(|m| {
                m.blocked_sends
                    .iter()
                    .any(|(k, s)| *s == sender.0 && *k != message.0)
            });
            if cx.channel.settled.contains(&message.0)
                || (!was_blocked && model.blocked_sends.contains_key(&message.0))
            {
                return Err(fault(ChannelFault::MessageReused {
                    channel,
                    message: message.0,
                }));
            }
            let model = cx.channel.channels.get_mut(&channel).ok_or_else(mismatch)?;
            match event {
                ChannelEvent::Sent { .. } => {
                    if !model.receiver_present {
                        return Err(mismatch());
                    }
                    if model.queue.len() >= model.capacity {
                        return Err(fault(ChannelFault::Overflow {
                            channel,
                            message: message.0,
                        }));
                    }
                    model.queue.push_back(message.0);
                    model.blocked_sends.remove(&message.0);
                    cx.channel.settled.insert(message.0);
                }
                ChannelEvent::SendBlocked { .. } => {
                    if was_blocked || !model.receiver_present || model.queue.len() < model.capacity
                    {
                        return Err(mismatch());
                    }
                    model.blocked_sends.insert(message.0, sender.0);
                }
                _ => {
                    if model.receiver_present {
                        return Err(mismatch());
                    }
                    model.blocked_sends.remove(&message.0);
                    cx.channel.settled.insert(message.0);
                }
            }
            // bn-1i050: a fresh send is the next dense message, through a clone of a
            // kept sender, by a sender with no other send waiting; the sender acts on
            // its own account; and a send is its permit's commit.
            if !was_blocked {
                let expected = cx.channel.next_message;
                if message.0 != expected || senders_closed || sender_busy {
                    return Err(fault(ChannelFault::MessageIdentity {
                        channel,
                        message: message.0,
                        expected,
                    }));
                }
                cx.channel.next_message += 1;
            }
            acting(cx, sender.0, false, channel, event)?;
            if matches!(event, ChannelEvent::Sent { .. })
                && !crate::family::obligation::take_send_permit(cx, sender.0)
            {
                return Err(fault(ChannelFault::NoSendPermit {
                    channel,
                    message: message.0,
                }));
            }
        }
        ChannelEvent::SendAbandoned {
            message, sender, ..
        } => {
            cancelling(cx, sender.0, channel, event)?;
            let model = cx.channel.channels.get_mut(&channel).ok_or_else(mismatch)?;
            if model.blocked_sends.remove(&message.0) != Some(sender.0) {
                return Err(mismatch());
            }
            cx.channel.settled.insert(message.0);
            after_acknowledgement(cx, sender.0, channel, event)?;
            crate::family::cancellation::imply_acknowledgement(cx, sender.0)?;
        }
        ChannelEvent::Received { message, .. } => {
            live(cx, receiver, channel, event)?;
            let model = cx.channel.channels.get_mut(&channel).ok_or_else(mismatch)?;
            if !model.receiver_present {
                return Err(mismatch());
            }
            let expected = model.queue.front().copied();
            if expected != Some(message.0) {
                return Err(fault(ChannelFault::NotFifo {
                    channel,
                    received: message.0,
                    expected,
                }));
            }
            model.queue.pop_front();
            model.receive_blocked = false;
            acting(cx, receiver, true, channel, event)?;
        }
        ChannelEvent::RecvBlocked { .. } => {
            live(cx, receiver, channel, event)?;
            let model = cx.channel.channels.get_mut(&channel).ok_or_else(mismatch)?;
            let can_receive = !model.senders_closed || !model.blocked_sends.is_empty();
            if model.receive_blocked || !model.queue.is_empty() || !can_receive {
                return Err(mismatch());
            }
            model.receive_blocked = true;
            acting(cx, receiver, true, channel, event)?;
        }
        ChannelEvent::RecvClosed { .. } => {
            live(cx, receiver, channel, event)?;
            let model = cx.channel.channels.get_mut(&channel).ok_or_else(mismatch)?;
            if !model.queue.is_empty() || !model.senders_closed || !model.blocked_sends.is_empty() {
                return Err(mismatch());
            }
            model.receive_blocked = false;
            acting(cx, receiver, true, channel, event)?;
        }
        ChannelEvent::RecvAbandoned { .. } => {
            cancelling(cx, receiver, channel, event)?;
            let model = cx.channel.channels.get_mut(&channel).ok_or_else(mismatch)?;
            if !model.receive_blocked {
                return Err(mismatch());
            }
            model.receive_blocked = false;
            after_acknowledgement(cx, receiver, channel, event)?;
            crate::family::cancellation::imply_acknowledgement(cx, receiver)?;
        }
        ChannelEvent::SendersClosed { .. } => {
            let model = cx.channel.channels.get_mut(&channel).ok_or_else(mismatch)?;
            if model.senders_closed {
                return Err(mismatch());
            }
            model.senders_closed = true;
        }
        ChannelEvent::ReceiverGone { discarded, .. } => {
            live(cx, receiver, channel, event)?;
            let model = cx.channel.channels.get_mut(&channel).ok_or_else(mismatch)?;
            if !model.receiver_present || model.receive_blocked {
                return Err(mismatch());
            }
            let mut queued: Vec<u64> = model.queue.iter().copied().collect();
            queued.sort_unstable();
            let declared: Vec<u64> = discarded.0.iter().map(|m| m.0).collect();
            if declared != queued {
                return Err(fault(ChannelFault::DropMismatch {
                    channel,
                    declared,
                    queued,
                }));
            }
            model.queue.clear();
            model.receiver_present = false;
            // The receiver goes when its task ends: in its own last call (running and
            // acting), or in its cancellation's cleanup, after the acknowledgement.
            if acting(cx, receiver, true, channel, event).is_err() {
                cancelling(cx, receiver, channel, event).map_err(|_| {
                    fault(ChannelFault::NotActing {
                        channel,
                        event: event.token(),
                    })
                })?;
                after_acknowledgement(cx, receiver, channel, event)?;
            }
        }
    }
    Ok(())
}

/// The whole-journal check, run after the last event: a channel whose receiving task
/// the model terminated has seen its receiver go.
pub(crate) fn finish(cx: &LiftContext) -> Result<(), LiftStop> {
    for (channel, model) in &cx.channel.channels {
        if model.receiver_present
            && cx
                .tree
                .worker_state(WorkerId::at(model.receiver))?
                .is_terminal()
        {
            return Err(fault(ChannelFault::ReceiverOutlivesTask {
                channel: *channel,
            }));
        }
        for (message, sender) in &model.blocked_sends {
            if cx.tree.worker_state(WorkerId::at(*sender))?.is_terminal() {
                return Err(fault(ChannelFault::SendOutlivesTask {
                    channel: *channel,
                    message: *message,
                }));
            }
        }
    }
    Ok(())
}

// --- the scripted source -------------------------------------------------------------

/// A channel step, as a scripted source reports it: the event itself, named by journal
/// ordinals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelReport(pub ChannelEvent);

/// Recorder state this family keeps: none. The recorder judges nothing.
#[derive(Debug, Default)]
pub struct RecordState;

pub(crate) fn record(report: &ChannelReport, cx: &mut RecordContext) -> Result<(), RecordRefusal> {
    cx.append(EventBody::Channel(report.0.clone()));
    Ok(())
}
