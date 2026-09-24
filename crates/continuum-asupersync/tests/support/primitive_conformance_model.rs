//! An executable conformance model of the asupersync primitive semantics, written for
//! C023 arrow A7 (RFC 0013 "Cross-path matrix": "asupersync adapter ↔ executable
//! primitive conformance models"; bn-ujpz0).
//!
//! # Why this file exists, and what it must not touch
//!
//! The adapter already holds its journal to a model: `lift` replays the journal into
//! `continuum_task::region`, and each family's `lift`/`finish` keeps a checked parallel
//! model beside it. That is the adapter checking itself. docs/01 §6 says "the adapter
//! cannot be the sole refinement checker for itself", and RFC 0013 says the planes "may
//! not all call the same evaluator to justify agreement". So this model is a second,
//! separately written account of the same primitives, and it shares nothing with the
//! adapter but the wire format:
//!
//! - it imports `std` only. It names no adapter type, no `asupersync` type and no
//!   `continuum_task` type. `tools/check_triptych_independence.py` holds that as the rule
//!   `a7-model-independent`;
//! - it reads a journal from its **canonical bytes**, with its own reader written from
//!   the format comment in `src/journal.rs` and the payload tables in `src/family/*.rs`.
//!   A wire format is a shared declarative artifact, which RFC 0013 permits ("versioned
//!   algebraic data schemas", "generated serialization fixtures"). The adapter's own
//!   decoder is not used;
//! - its rules come from the normative text, cited at each rule: docs/02 §7 (the
//!   cancellation calculus: lifecycle, effect protocol, core invariants), plan §4.1
//!   (the task lifecycle), docs/01 §6 (the adapter mapping), and docs/02 §5 (virtual
//!   time). The channel rules (bn-1i050) are those of a bounded multi-producer,
//!   single-consumer FIFO channel whose send is a docs/02 §7 two-phase effect (a send
//!   permit is reserved, then committed as the send), written from that semantics and
//!   the family's wire table, not from the adapter's channel lift. The fail-stop crash
//!   rules (bn-20d8u) are those of the process pack's profile
//!   `process/crash-restart-v0`, rows `fail-stop-crash` and `late-completion`, written
//!   from the profile's statements and RFC 0026 correction 58, not from the adapter's
//!   crash lift: a crash stops its incarnation's live tasks where they are, runs
//!   nothing of them, and fences what they held. Where the adapter's family
//!   documentation fixes an observation the normative text leaves open (which phase of
//!   a cancellation the substrate lets a run see), the rule says so.
//!
//! # The model
//!
//! A state is a region tree, tasks with a lifecycle phase and a cancellation phase,
//! reservations, obligations, timers and a virtual clock. The model is parameterized by
//! an [`Alphabet`], the set of families a run observes, as a model is parameterized by
//! its configuration: a run that does not observe the cancellation family cannot be held
//! to cancellation phases it never reports.
//!
//! It has two formulations of one transition relation, written separately:
//!
//! - [`Model::step`], a guard per step: is this step enabled here, and if so, take it;
//! - [`Model::enabled`], a generator: every step enabled here, built constructively from
//!   the state (free time coordinates and free tokens as [`Pattern`]s).
//!
//! A trace is accepted when every step is enabled in turn and the end state meets the
//! end conditions ([`Model::finish`]). The verdict is typed ([`Verdict`]): accepted,
//! rejected at a position for a named [`Fault`], malformed bytes, or a family the model
//! does not cover (INV-008: never a bare boolean).

use std::collections::BTreeSet;
use std::fmt;

// --- the wire alphabet ---------------------------------------------------------------

/// The fixed header of a canonical journal (`src/journal.rs`, "The encoding").
const MAGIC: &[u8] = b"continuum/semantic-journal\n";
/// The encoding versions this reader accepts: 2, and 3, which adds lifecycle event 8
/// (`region-crashed`), effect event 4, obligation event 6 and time event 6 (each
/// `fenced`), and a third set on obligation event 5 (`src/journal.rs`, bn-20d8u). Under
/// version 2 those tags are [`WireFault::NotInVersion`] and a settle has no fenced set.
/// Version 1 is no longer read (two versions at most, ADR-0018).
const VERSIONS: [u32; 2] = [2, 3];

/// The six families the substrate binding observes, with their wire tags
/// (`src/family.rs`, "The six families").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FamilyTag {
    /// Tag 1.
    Lifecycle,
    /// Tag 2.
    Effect,
    /// Tag 3.
    Cancellation,
    /// Tag 4.
    Obligation,
    /// Tag 5.
    Time,
    /// Tag 6.
    Channel,
}

impl FamilyTag {
    const fn from_tag(tag: u8) -> Option<Self> {
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
}

/// The families a run observes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alphabet(BTreeSet<FamilyTag>);

impl Alphabet {
    /// These families. Lifecycle is always among them: every other family names tasks
    /// and regions by the ordinals lifecycle events allocate.
    #[must_use]
    pub fn new(families: impl IntoIterator<Item = FamilyTag>) -> Self {
        let mut set: BTreeSet<FamilyTag> = families.into_iter().collect();
        set.insert(FamilyTag::Lifecycle);
        Self(set)
    }

    fn has(&self, family: FamilyTag) -> bool {
        self.0.contains(&family)
    }
}

/// Why a task's cancellation was requested (docs/02 §7 `request(cancel_reason)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Cause {
    /// The task's own region was the one cancelled.
    User,
    /// A region above the task's region was cancelled.
    ParentCancelled,
    /// The task's own budget deadline passed (docs/02 §7 `request(cancel_reason)` for
    /// one task, raised by docs/02 §5's virtual clock; bn-36wy3).
    Deadline,
}

/// Why a reservation was aborted (docs/02 §7 `Aborted(reason)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AbortReason {
    /// The holder's cancellation aborted it.
    Cancel,
    /// The holder aborted it on purpose.
    Explicit,
}

/// One primitive step, in the model's own vocabulary. Ordinals are the journal's dense
/// allocation orders; the root region is `0` and implicit.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Step {
    /// A child region opened.
    OpenRegion { region: u32, parent: u32 },
    /// A task admitted into a region; `None` for resumable, else the declared reason.
    Spawn {
        task: u32,
        region: u32,
        non_resumable: Option<String>,
    },
    /// `Created → Running`.
    Begin { task: u32 },
    /// `Running → Suspended`.
    Suspend { task: u32 },
    /// `Suspended → Running`.
    Resume { task: u32 },
    /// `Running → Completed`.
    Complete { task: u32 },
    /// `Created | Running → Failed`.
    Fail { task: u32, reason: String },
    /// One task's own cancellation requested, outside any region's (its deadline).
    TaskCancelRequested { task: u32 },
    /// One task ended as cancelled on its own, its region unchanged.
    TaskCancelled { task: u32 },
    /// A normal close requested on a region's subtree.
    Close { region: u32 },
    /// Cancellation requested on a region's subtree.
    Cancel { region: u32 },
    /// A fail-stop crash of a region's subtree; these live tasks stopped (bn-20d8u).
    Crash { region: u32, stopped: Vec<u32> },
    /// A region's subtree drained; these tasks were cancelled by it.
    Drain { region: u32, cancelled: Vec<u32> },
    /// A region's subtree finalized.
    Finalize { region: u32 },
    /// A task's cancellation requested.
    CancelRequested { task: u32, cause: Cause },
    /// A task observed its cancellation at a checkpoint.
    CancelAcknowledged { task: u32 },
    /// A task finished its cleanup and completed as cancelled.
    CancelCompleted { task: u32, cause: Cause },
    /// `Idle → Reserved(token)`.
    Reserve { reservation: u32, task: u32 },
    /// `Reserved → Committed`.
    Commit { reservation: u32 },
    /// `Reserved → Aborted(reason)`.
    Abort {
        reservation: u32,
        reason: AbortReason,
    },
    /// `Reserved`, and its holder crashed: fenced (bn-20d8u).
    EffectFenced { reservation: u32 },
    /// An obligation opened, of wire kind `kind`, held by `holder` in `region`.
    Opened {
        obligation: u32,
        kind: u8,
        holder: u32,
        region: u32,
    },
    /// An obligation discharged (`committed` or aborted).
    Discharged { obligation: u32, committed: bool },
    /// An obligation moved to a new holder and region.
    Transferred {
        obligation: u32,
        holder: u32,
        region: u32,
    },
    /// An obligation leaked: its holder ended with it open.
    Leaked { obligation: u32 },
    /// A region's obligation balance at close.
    Settled {
        region: u32,
        open: Vec<u32>,
        leaked: Vec<u32>,
        fenced: Vec<u32>,
    },
    /// An open obligation whose holder crashed: fenced (bn-20d8u).
    ObligationFenced { obligation: u32 },
    /// A timer armed by a task at `at` for `deadline`.
    Scheduled {
        timer: u32,
        task: u32,
        at: u64,
        deadline: u64,
    },
    /// The virtual clock moved.
    Advanced { from: u64, to: u64 },
    /// A timer fired.
    Fired { timer: u32, at: u64 },
    /// A timer was cancelled.
    TimerCancelled { timer: u32, at: u64 },
    /// A task runs under a budget deadline at `deadline`.
    DeadlineSet { task: u32, deadline: u64 },
    /// An armed timer whose task crashed: fenced (bn-20d8u).
    TimerFenced { timer: u32 },
    /// A bounded channel opened, its receiver held by `receiver`.
    ChannelOpened {
        channel: u32,
        capacity: u32,
        receiver: u32,
    },
    /// A message was sent (its permit committed) and queued.
    Sent {
        channel: u32,
        message: u64,
        sender: u32,
    },
    /// A send waits for room.
    SendBlocked {
        channel: u32,
        message: u64,
        sender: u32,
    },
    /// A send failed: the receiver is gone.
    SendClosed {
        channel: u32,
        message: u64,
        sender: u32,
    },
    /// A blocked send was dropped by its sender's cancellation.
    SendAbandoned {
        channel: u32,
        message: u64,
        sender: u32,
    },
    /// The receiver took a message.
    Received { channel: u32, message: u64 },
    /// A receive waits for a message.
    RecvBlocked { channel: u32 },
    /// A receive returned closed.
    RecvClosed { channel: u32 },
    /// A blocked receive was dropped by the receiver's cancellation.
    RecvAbandoned { channel: u32 },
    /// The last kept sender was dropped.
    SendersClosed { channel: u32 },
    /// The receiver went, dropping these queued messages.
    ReceiverGone { channel: u32, discarded: Vec<u64> },
}

impl Step {
    const fn family(&self) -> FamilyTag {
        match self {
            Self::OpenRegion { .. }
            | Self::Spawn { .. }
            | Self::Begin { .. }
            | Self::Suspend { .. }
            | Self::Resume { .. }
            | Self::Complete { .. }
            | Self::Fail { .. }
            | Self::TaskCancelRequested { .. }
            | Self::TaskCancelled { .. }
            | Self::Close { .. }
            | Self::Cancel { .. }
            | Self::Crash { .. }
            | Self::Drain { .. }
            | Self::Finalize { .. } => FamilyTag::Lifecycle,
            Self::CancelRequested { .. }
            | Self::CancelAcknowledged { .. }
            | Self::CancelCompleted { .. } => FamilyTag::Cancellation,
            Self::Reserve { .. }
            | Self::Commit { .. }
            | Self::Abort { .. }
            | Self::EffectFenced { .. } => FamilyTag::Effect,
            Self::Opened { .. }
            | Self::Discharged { .. }
            | Self::Transferred { .. }
            | Self::Leaked { .. }
            | Self::Settled { .. }
            | Self::ObligationFenced { .. } => FamilyTag::Obligation,
            Self::Scheduled { .. }
            | Self::Advanced { .. }
            | Self::Fired { .. }
            | Self::TimerCancelled { .. }
            | Self::DeadlineSet { .. }
            | Self::TimerFenced { .. } => FamilyTag::Time,
            Self::ChannelOpened { .. }
            | Self::Sent { .. }
            | Self::SendBlocked { .. }
            | Self::SendClosed { .. }
            | Self::SendAbandoned { .. }
            | Self::Received { .. }
            | Self::RecvBlocked { .. }
            | Self::RecvClosed { .. }
            | Self::RecvAbandoned { .. }
            | Self::SendersClosed { .. }
            | Self::ReceiverGone { .. } => FamilyTag::Channel,
        }
    }
}

/// Why bytes are not a canonical journal, as this reader sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireFault {
    /// The bytes end inside a field.
    Truncated { at: usize },
    /// The header or version is not the one this reader knows.
    Header,
    /// An event's sequence number is not its position.
    Sequence { expected: u64, found: u64 },
    /// A tag is in its table only from a later version than the journal declares.
    NotInVersion {
        table: &'static str,
        tag: u8,
        at: usize,
    },
    /// A tag is outside its table.
    Tag {
        table: &'static str,
        tag: u8,
        at: usize,
    },
    /// A token is not UTF-8, or is empty.
    Token { at: usize },
    /// A set is not strictly ascending.
    Unsorted { at: usize },
    /// A payload's length disagrees with its content.
    PayloadLength { event: u64 },
    /// Bytes follow the last event.
    Trailing { at: usize },
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
    version: u32,
}

impl<'a> Reader<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8], WireFault> {
        let end = self
            .at
            .checked_add(len)
            .filter(|end| *end <= self.bytes.len())
            .ok_or(WireFault::Truncated { at: self.at })?;
        let out = &self.bytes[self.at..end];
        self.at = end;
        Ok(out)
    }
    fn u8(&mut self) -> Result<u8, WireFault> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, WireFault> {
        let mut b = [0_u8; 4];
        b.copy_from_slice(self.take(4)?);
        Ok(u32::from_be_bytes(b))
    }
    fn u64(&mut self) -> Result<u64, WireFault> {
        let mut b = [0_u8; 8];
        b.copy_from_slice(self.take(8)?);
        Ok(u64::from_be_bytes(b))
    }
    fn field(&mut self) -> Result<&'a [u8], WireFault> {
        let len = self.u32()? as usize;
        self.take(len)
    }
    fn token(&mut self) -> Result<String, WireFault> {
        let at = self.at;
        let field = self.field()?;
        match std::str::from_utf8(field) {
            Ok(text) if !text.is_empty() => Ok(text.to_owned()),
            _ => Err(WireFault::Token { at }),
        }
    }
    fn set64(&mut self) -> Result<Vec<u64>, WireFault> {
        let at = self.at;
        let count = self.u32()?;
        let mut out: Vec<u64> = Vec::new();
        for _ in 0..count {
            let member = self.u64()?;
            if out.last().is_some_and(|last| *last >= member) {
                return Err(WireFault::Unsorted { at });
            }
            out.push(member);
        }
        Ok(out)
    }
    fn set(&mut self) -> Result<Vec<u32>, WireFault> {
        let at = self.at;
        let count = self.u32()?;
        let mut out: Vec<u32> = Vec::new();
        for _ in 0..count {
            let member = self.u32()?;
            if out.last().is_some_and(|last| *last >= member) {
                return Err(WireFault::Unsorted { at });
            }
            out.push(member);
        }
        Ok(out)
    }
    fn tag(&mut self, table: &'static str, max: u8) -> Result<u8, WireFault> {
        let at = self.at;
        let tag = self.u8()?;
        if tag == 0 || tag > max {
            return Err(WireFault::Tag { table, tag, at });
        }
        Ok(tag)
    }
    /// A tag of a table that grew in version 3: `max` is its version-3 bound, and a
    /// tag above `v2_max` needs version 3.
    fn tag_since(&mut self, table: &'static str, v2_max: u8, max: u8) -> Result<u8, WireFault> {
        let at = self.at;
        let tag = self.tag(table, max)?;
        if tag > v2_max && self.version < 3 {
            return Err(WireFault::NotInVersion { table, tag, at });
        }
        Ok(tag)
    }
    fn cause(&mut self) -> Result<Cause, WireFault> {
        Ok(match self.tag("cancel cause", 3)? {
            1 => Cause::User,
            2 => Cause::ParentCancelled,
            _ => Cause::Deadline,
        })
    }
}

/// Read a canonical journal: `MAGIC u32(version) u64(count) event*`, each event
/// `u64(seq) u8(family) u32(len) payload[len]`, big-endian, nothing after the last.
///
/// # Errors
///
/// The first [`WireFault`].
pub fn read(bytes: &[u8]) -> Result<Vec<Step>, WireFault> {
    let mut input = Reader {
        bytes,
        at: 0,
        version: 0,
    };
    if input.take(MAGIC.len())? != MAGIC {
        return Err(WireFault::Header);
    }
    let version = input.u32()?;
    if !VERSIONS.contains(&version) {
        return Err(WireFault::Header);
    }
    let count = input.u64()?;
    let mut out = Vec::new();
    for expected in 0..count {
        let found = input.u64()?;
        if found != expected {
            return Err(WireFault::Sequence { expected, found });
        }
        let family_at = input.at;
        let family = input.u8()?;
        let payload = input.field()?;
        let decoded = match FamilyTag::from_tag(family) {
            Some(tag) => {
                let mut inner = Reader {
                    bytes: payload,
                    at: 0,
                    version,
                };
                let step = payload_step(tag, &mut inner)?;
                if inner.at != payload.len() {
                    return Err(WireFault::PayloadLength { event: expected });
                }
                step
            }
            None => {
                return Err(WireFault::Tag {
                    table: "family",
                    tag: family,
                    at: family_at,
                });
            }
        };
        out.push(decoded);
    }
    if input.at != bytes.len() {
        return Err(WireFault::Trailing { at: input.at });
    }
    Ok(out)
}

fn payload_step(family: FamilyTag, r: &mut Reader<'_>) -> Result<Step, WireFault> {
    Ok(match family {
        // src/family/lifecycle.rs: tags 1..=8; 8 from version 3.
        FamilyTag::Lifecycle => match r.tag_since("lifecycle event", 7, 8)? {
            1 => Step::OpenRegion {
                region: r.u32()?,
                parent: r.u32()?,
            },
            2 => {
                let task = r.u32()?;
                let region = r.u32()?;
                let non_resumable = match r.tag("resumability", 2)? {
                    1 => None,
                    _ => Some(r.token()?),
                };
                Step::Spawn {
                    task,
                    region,
                    non_resumable,
                }
            }
            3 => {
                let task = r.u32()?;
                match r.tag("task step", 7)? {
                    1 => Step::Begin { task },
                    2 => Step::Suspend { task },
                    3 => Step::Resume { task },
                    4 => Step::Complete { task },
                    5 => Step::Fail {
                        task,
                        reason: r.token()?,
                    },
                    6 => Step::TaskCancelled { task },
                    _ => Step::TaskCancelRequested { task },
                }
            }
            4 => Step::Close { region: r.u32()? },
            5 => Step::Cancel { region: r.u32()? },
            6 => Step::Drain {
                region: r.u32()?,
                cancelled: r.set()?,
            },
            7 => Step::Finalize { region: r.u32()? },
            _ => Step::Crash {
                region: r.u32()?,
                stopped: r.set()?,
            },
        },
        // src/family/effect.rs: tags 1..=4 (4 from version 3), abort reasons 1..=2.
        FamilyTag::Effect => {
            let tag = r.tag_since("effect event", 3, 4)?;
            let reservation = r.u32()?;
            match tag {
                1 => Step::Reserve {
                    reservation,
                    task: r.u32()?,
                },
                2 => Step::Commit { reservation },
                4 => Step::EffectFenced { reservation },
                _ => Step::Abort {
                    reservation,
                    reason: match r.tag("abort reason", 2)? {
                        1 => AbortReason::Cancel,
                        _ => AbortReason::Explicit,
                    },
                },
            }
        }
        // src/family/cancellation.rs: tags 1..=3, causes 1..=3.
        FamilyTag::Cancellation => {
            let tag = r.tag("cancellation event", 3)?;
            let task = r.u32()?;
            match tag {
                1 => Step::CancelRequested {
                    task,
                    cause: r.cause()?,
                },
                2 => Step::CancelAcknowledged { task },
                _ => Step::CancelCompleted {
                    task,
                    cause: r.cause()?,
                },
            }
        }
        // src/family/obligation.rs: tags 1..=6 (6 from version 3), kinds 1..=6,
        // discharge 1..=2; a version-3 settle has a third set, the fenced obligations.
        FamilyTag::Obligation => match r.tag_since("obligation event", 5, 6)? {
            1 => Step::Opened {
                obligation: r.u32()?,
                kind: r.tag("obligation kind", 6)?,
                holder: r.u32()?,
                region: r.u32()?,
            },
            2 => Step::Discharged {
                obligation: r.u32()?,
                committed: r.tag("discharge", 2)? == 1,
            },
            3 => Step::Transferred {
                obligation: r.u32()?,
                holder: r.u32()?,
                region: r.u32()?,
            },
            4 => Step::Leaked {
                obligation: r.u32()?,
            },
            5 => Step::Settled {
                region: r.u32()?,
                open: r.set()?,
                leaked: r.set()?,
                fenced: if r.version >= 3 { r.set()? } else { Vec::new() },
            },
            _ => Step::ObligationFenced {
                obligation: r.u32()?,
            },
        },
        // src/family/time.rs: tags 1..=6 (6 from version 3).
        FamilyTag::Time => match r.tag_since("time event", 5, 6)? {
            1 => Step::Scheduled {
                timer: r.u32()?,
                task: r.u32()?,
                at: r.u64()?,
                deadline: r.u64()?,
            },
            2 => Step::Advanced {
                from: r.u64()?,
                to: r.u64()?,
            },
            3 => Step::Fired {
                timer: r.u32()?,
                at: r.u64()?,
            },
            4 => Step::TimerCancelled {
                timer: r.u32()?,
                at: r.u64()?,
            },
            5 => Step::DeadlineSet {
                task: r.u32()?,
                deadline: r.u64()?,
            },
            _ => Step::TimerFenced { timer: r.u32()? },
        },
        // src/family/channel.rs: tags 1..=11, each `u8 tag, u32 channel, fields`.
        FamilyTag::Channel => {
            let tag = r.tag("channel event", 11)?;
            let channel = r.u32()?;
            match tag {
                1 => Step::ChannelOpened {
                    channel,
                    capacity: r.u32()?,
                    receiver: r.u32()?,
                },
                2..=5 => {
                    let message = r.u64()?;
                    let sender = r.u32()?;
                    match tag {
                        2 => Step::Sent {
                            channel,
                            message,
                            sender,
                        },
                        3 => Step::SendBlocked {
                            channel,
                            message,
                            sender,
                        },
                        4 => Step::SendClosed {
                            channel,
                            message,
                            sender,
                        },
                        _ => Step::SendAbandoned {
                            channel,
                            message,
                            sender,
                        },
                    }
                }
                6 => Step::Received {
                    channel,
                    message: r.u64()?,
                },
                7 => Step::RecvBlocked { channel },
                8 => Step::RecvClosed { channel },
                9 => Step::RecvAbandoned { channel },
                10 => Step::SendersClosed { channel },
                _ => Step::ReceiverGone {
                    channel,
                    discarded: r.set64()?,
                },
            }
        }
    })
}

// --- the state -----------------------------------------------------------------------

/// A region's lifecycle (plan §4.1, docs/02 §7 "region close implies no descendant tasks
/// or obligations"): open, then a close or cancel request, then drained, then finalized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegionPhase {
    Open,
    Closing,
    Cancelling,
    Finalized,
}

#[derive(Debug, Clone)]
struct Region {
    parent: Option<u32>,
    phase: RegionPhase,
    /// The region whose cancel request put this one in `Cancelling`.
    origin: Option<u32>,
    drained: bool,
    settled: bool,
    /// A fail-stop crash covered it (bn-20d8u).
    crashed: bool,
}

/// A task's lifecycle (plan §4.1: `Created → Running → Suspended | Completed | Failed |
/// Cancelled`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskPhase {
    Created,
    Running,
    Suspended,
    Completed,
    Failed,
    Cancelled,
    /// Stopped by a fail-stop crash: nothing of it runs again (`process/crash-restart-v0`
    /// row `fail-stop-crash`; bn-20d8u).
    Crashed,
}

impl TaskPhase {
    const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Cancelled | Self::Crashed
        )
    }
}

/// A task's cancellation (docs/02 §7 `Active → request → Cancelling → … → Cancelled`),
/// at the three points the substrate lets a run observe (`src/family/cancellation.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CancelPhase {
    None,
    Requested(Cause),
    Acknowledged(Cause),
    Done(Cause),
}

#[derive(Debug, Clone)]
struct Task {
    region: u32,
    phase: TaskPhase,
    cancel: CancelPhase,
    /// Its budget deadline, when it runs under one (bn-36wy3).
    deadline: Option<u64>,
    /// Its own cancellation was requested, outside any region's (bn-36wy3).
    requested_alone: bool,
    /// Its own single-task `→ Cancelled` ([`Step::TaskCancelled`]) was taken. Kept apart
    /// from `phase`, because a region's drain also ends a task as `Cancelled`
    /// (cr-3pu5cu).
    ended_alone: bool,
    /// It took a cleanup step (a cancel abort, a timer cancelled, a send or receive
    /// abandoned): it drains, so it has observed its cancellation even when the
    /// alphabet shows no acknowledgement (docs/02 §7; cr-3pu5cu round 6).
    cleaned_up: bool,
    /// A region's cancellation reached it while it was live (cr-3pu5cu round 6).
    region_requested: bool,
}

/// docs/02 §7 effect protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EffectPhase {
    Reserved,
    Committed,
    Aborted,
    /// Its holder crashed with it reserved: neither committed nor aborted, ever.
    Fenced,
}

#[derive(Debug, Clone)]
struct Reservation {
    task: u32,
    phase: EffectPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObligationPhase {
    Open,
    Discharged,
    Leaked,
    /// Its holder crashed with it open: owed, never discharged (row `late-completion`).
    Fenced,
}

#[derive(Debug, Clone)]
struct Obligation {
    kind: u8,
    holder: u32,
    region: u32,
    phase: ObligationPhase,
}

/// The obligation kind a channel send's permit is (`src/family/obligation.rs` wire
/// table: `send-permit`).
const SEND_PERMIT: u8 = 1;

/// A bounded channel: its capacity, its receiver and whether it is still there, the
/// FIFO queue, whether a kept sender remains, and whether a receive is blocked.
#[derive(Debug, Clone)]
struct Channel {
    capacity: u32,
    receiver: u32,
    present: bool,
    queue: std::collections::VecDeque<u64>,
    senders_open: bool,
    recv_blocked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TimerPhase {
    Armed,
    Fired,
    Cancelled,
    /// Its task crashed while it slept on it: its fire reaches no task.
    Fenced,
}

#[derive(Debug, Clone)]
struct Timer {
    task: u32,
    deadline: u64,
    phase: TimerPhase,
}

/// Why a step is not enabled, or why a trace does not end well.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fault {
    /// A family the alphabet does not observe.
    OutsideAlphabet,
    /// A fresh entity not named by the next ordinal.
    Identity { expected: u32, found: u32 },
    /// A region's cancellation reached the task, the alphabet observes phases, and the
    /// task ended (or the trace ended) with no phase of its cancellation.
    UnreportedRequest(u32),
    /// A task's own region is cancelled after the clock reached the task's deadline and
    /// before the task observed it.
    CancelRaced(u32),
    /// A budget deadline not declared right after its task's spawn.
    DeadlineNotAtSpawn(u32),
    /// A step of a task whose cancellation was requested and not yet observed: its own
    /// work, a receipt, or (when the alphabet observes phases) its cleanup or its end.
    /// asupersync 0.5.0 lets a task observe a request only at `Cx::checkpoint`, which
    /// every poll of a bound task calls first, so a requested task's next step is its
    /// observation (docs/02 §7 `request(cancel_reason) → Cancelling`: there is no
    /// working state between; RFC 0026 correction 55, bn-28hup).
    Unobserved(u32),
    /// A step names an entity that does not exist.
    Unknown(&'static str, u32),
    /// Work entering a region that does not accept it.
    RegionNotOpen(u32),
    /// A request, drain or finalize on a region in the wrong phase.
    RegionPhase(u32),
    /// A task step from the wrong lifecycle phase.
    TaskPhase(u32),
    /// A task step from the wrong cancellation phase.
    CancelPhase(u32),
    /// A cancellation cause other than the one the region tree implies.
    Cause(u32),
    /// A deadline cancellation before the clock reached the task's deadline, or of a
    /// task with none (bn-36wy3).
    Deadline(u32),
    /// A normal close drained with live owned work.
    DrainBlocked { region: u32, task: u32 },
    /// A drain reported a cancelled set other than the model's.
    DrainSet { region: u32, model: Vec<u32> },
    /// A drain absorbed a task whose cancellation phases did not complete.
    UnphasedCancellation(u32),
    /// A finalize with undrained regions or live tasks in the subtree.
    FinalizeEarly(u32),
    /// A task parks, ends or is drained holding a reserved effect (a visible
    /// half-effect).
    HalfEffect { task: u32 },
    /// A reservation step from the wrong phase.
    EffectPhase(u32),
    /// An obligation step from the wrong phase or by the wrong holder.
    ObligationPhase(u32),
    /// An obligation whose region is not its holder's.
    ObligationRegion(u32),
    /// A task ends cancelled still holding an obligation (docs/02 §7 `obligations == ∅
    /// → Cancelled`).
    HeldObligation { task: u32 },
    /// A settle before finalize, twice, or with a balance other than the model's.
    Settle(u32),
    /// Region close with an open or leaked obligation (docs/02 §7 core invariant).
    CloseWithObligations(u32),
    /// A time event at an instant other than now.
    OffClock { now: u64, at: u64 },
    /// The clock did not move forward, or a deadline is not ahead.
    NotAhead,
    /// A timer step from the wrong phase.
    TimerPhase(u32),
    /// A timer past its deadline and not fired.
    LateTimer(u32),
    /// A task that sleeps is woken, ends, or is drained with its timer armed.
    TimerOutlivesSleep { task: u32 },
    /// End of trace: a cancellation requested and not drained.
    UndrainedCancellation(u32),
    /// A lifecycle step that is not a step of the task comes between a task's own
    /// `cancel-requested` and its own single-task `→ Cancelled`: RFC 0026 correction 53 item 6 journals a task's own
    /// cancellation, from its request to its `cancel`, within the one substrate
    /// operation that observed it, so no region, spawn or other task's lifecycle step
    /// comes between (cr-3pu5cu).
    OwnCancellationInterrupted(u32),
    /// A task whose own cancellation was requested ends some other way than its own
    /// single-task `→ Cancelled`: a region's drain, a completion or a failure
    /// (docs/02 §7; RFC 0026 correction 53 items 3 and 6; cr-3pu5cu).
    OwnCancellationUnended(u32),
    /// End of trace: a finalized region never settled.
    Unsettled(u32),
    /// End of trace: a region still cancelling and not drained, or drained and not
    /// finalized (docs/02 §7 `Cancelling ─ drain* ─ finalize*`; cr-3pu5cu).
    UnfinishedRegion(u32),
    /// End of trace: an obligation leaked by a holder that never ended.
    UnendedLeak(u32),
    /// A channel step its channel's state does not admit; `rule` names the channel
    /// rule it breaks.
    Channel { channel: u32, rule: &'static str },
    /// A send with no committed send permit of its sender (docs/02 §7: the send is the
    /// permit's commit).
    NoSendPermit { task: u32 },
    /// A task ends, or is drained, while it still holds a channel's receiver, a blocked
    /// receive or a blocked send.
    ChannelOutlivesTask { task: u32 },
    /// A crash stopped a set of tasks other than its subtree's live ones (bn-20d8u).
    CrashSet { region: u32, model: Vec<u32> },
    /// A crash of a task with no fail-stop semantics here: in a cancellation, under a
    /// budget deadline, or holding a channel (bn-20d8u).
    CrashUnsupported(u32),
    /// A step other than an owed fence while a crash still owes one: what a crashed
    /// task held is fenced right after its crash (bn-20d8u).
    FenceOwed { family: FamilyTag, ordinal: u32 },
    /// A fence that no crash owes (bn-20d8u).
    FenceUnowed { family: FamilyTag, ordinal: u32 },
}

/// A step the generator enables, with free coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pattern {
    /// Exactly this step.
    Exact(Step),
    /// A spawn with any resumability.
    Spawn { task: u32, region: u32 },
    /// A failure with any reason token.
    Fail { task: u32 },
    /// An obligation of any kind.
    Opened {
        obligation: u32,
        holder: u32,
        region: u32,
    },
    /// A timer with any deadline after `at`.
    Scheduled { timer: u32, task: u32, at: u64 },
    /// An advance to any instant after `from`.
    Advanced { from: u64 },
    /// A channel of any positive capacity.
    ChannelOpened { channel: u32, receiver: u32 },
    /// A budget deadline for a new task, at any instant after `after`.
    DeadlineSet { task: u32, after: u64 },
}

impl Pattern {
    /// Whether `step` is an instance of this pattern.
    #[must_use]
    pub fn admits(&self, step: &Step) -> bool {
        match (self, step) {
            (Self::Exact(exact), step) => exact == step,
            (
                Self::Spawn { task, region },
                Step::Spawn {
                    task: t, region: g, ..
                },
            ) => task == t && region == g,
            (Self::Fail { task }, Step::Fail { task: t, .. }) => task == t,
            (
                Self::Opened {
                    obligation,
                    holder,
                    region,
                },
                Step::Opened {
                    obligation: o,
                    holder: h,
                    region: g,
                    ..
                },
            ) => obligation == o && holder == h && region == g,
            (
                Self::Scheduled { timer, task, at },
                Step::Scheduled {
                    timer: k,
                    task: t,
                    at: a,
                    deadline,
                },
            ) => timer == k && task == t && at == a && deadline > a,
            (Self::Advanced { from }, Step::Advanced { from: f, to }) => from == f && to > f,
            (Self::DeadlineSet { task, after }, Step::DeadlineSet { task: t, deadline }) => {
                task == t && deadline > after
            }
            (
                Self::ChannelOpened { channel, receiver },
                Step::ChannelOpened {
                    channel: c,
                    capacity,
                    receiver: r,
                },
            ) => channel == c && receiver == r && *capacity > 0,
            _ => false,
        }
    }
}

/// The model's verdict on a trace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Every step was enabled in turn and the end conditions hold.
    Accepted { steps: usize },
    /// The step at `at` is not enabled (or, with `at == len`, the end conditions fail).
    Rejected { at: usize, fault: Fault },
    /// The bytes are not a canonical journal.
    Malformed(WireFault),
}

impl Verdict {
    /// Whether the verdict is an acceptance.
    #[must_use]
    pub const fn is_accepted(&self) -> bool {
        matches!(self, Self::Accepted { .. })
    }
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

/// The conformance model's state.
#[derive(Debug, Clone)]
pub struct Model {
    alphabet: Alphabet,
    regions: Vec<Region>,
    tasks: Vec<Task>,
    reservations: Vec<Reservation>,
    obligations: Vec<Obligation>,
    timers: Vec<Timer>,
    now: u64,
    channels: Vec<Channel>,
    /// The task the previous step spawned, when it was a spawn.
    last_spawn: Option<u32>,
    /// Blocked sends: message → (channel, sender).
    blocked: std::collections::BTreeMap<u64, (u32, u32)>,
    next_message: u64,
    /// Send permits a task committed and has not yet sent with.
    permits: std::collections::BTreeMap<u32, u32>,
    /// The fences the last crash still owes, by family and ordinal (bn-20d8u).
    owed: BTreeSet<(FamilyTag, u32)>,
}

fn ord(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
}

impl Model {
    /// The initial state: the root region `0`, open; nothing else; the clock at zero.
    #[must_use]
    pub fn new(alphabet: Alphabet) -> Self {
        Self {
            alphabet,
            regions: vec![Region {
                parent: None,
                phase: RegionPhase::Open,
                origin: None,
                drained: false,
                settled: false,
                crashed: false,
            }],
            tasks: Vec::new(),
            reservations: Vec::new(),
            obligations: Vec::new(),
            timers: Vec::new(),
            now: 0,
            channels: Vec::new(),
            last_spawn: None,
            blocked: std::collections::BTreeMap::new(),
            next_message: 0,
            permits: std::collections::BTreeMap::new(),
            owed: BTreeSet::new(),
        }
    }

    // --- queries -------------------------------------------------------------------

    fn region(&self, r: u32) -> Result<&Region, Fault> {
        self.regions
            .get(r as usize)
            .ok_or(Fault::Unknown("region", r))
    }
    fn task(&self, t: u32) -> Result<&Task, Fault> {
        self.tasks.get(t as usize).ok_or(Fault::Unknown("task", t))
    }
    fn in_subtree(&self, member: u32, root: u32) -> bool {
        let mut at = Some(member);
        while let Some(r) = at {
            if r == root {
                return true;
            }
            at = self.regions.get(r as usize).and_then(|g| g.parent);
        }
        false
    }
    fn subtree(&self, root: u32) -> Vec<u32> {
        (0..ord(self.regions.len()))
            .filter(|r| self.in_subtree(*r, root))
            .collect()
    }
    /// A task that can act on its own account: not terminal, and not cancelling.
    fn acting(&self, t: u32) -> bool {
        self.tasks
            .get(t as usize)
            .is_some_and(|task| !task.phase.is_terminal())
            && !self.cancelling(t)
    }
    fn staged(&self, t: u32) -> bool {
        self.alphabet.has(FamilyTag::Effect)
            && self
                .reservations
                .iter()
                .any(|e| e.task == t && e.phase == EffectPhase::Reserved)
    }
    fn armed(&self, t: u32) -> bool {
        self.alphabet.has(FamilyTag::Time)
            && self
                .timers
                .iter()
                .any(|k| k.task == t && k.phase == TimerPhase::Armed)
    }
    fn holds(&self, t: u32) -> bool {
        self.alphabet.has(FamilyTag::Obligation)
            && self
                .obligations
                .iter()
                .any(|o| o.holder == t && o.phase == ObligationPhase::Open)
    }
    fn channel(&self, c: u32) -> Result<&Channel, Fault> {
        self.channels
            .get(c as usize)
            .ok_or(Fault::Unknown("channel", c))
    }
    /// Whether `t` still holds a channel's receiver or a blocked send: it may not end
    /// while it does.
    fn holds_channel(&self, t: u32) -> bool {
        self.alphabet.has(FamilyTag::Channel)
            && (self.channels.iter().any(|c| c.present && c.receiver == t)
                || self.blocked.values().any(|(_, s)| *s == t))
    }
    /// A send needs a committed send permit of its sender (docs/02 §7 two-phase
    /// effect; the send is the commit) when the ledger is observed.
    fn permit_ok(&self, t: u32) -> bool {
        !self.alphabet.has(FamilyTag::Obligation) || self.permits.get(&t).copied().unwrap_or(0) > 0
    }
    /// Whether anything can still send on `c`: a kept sender, or a blocked sender.
    fn can_still_receive(&self, c: u32) -> bool {
        self.channels
            .get(c as usize)
            .is_some_and(|ch| ch.senders_open)
            || self.blocked.values().any(|(cc, _)| *cc == c)
    }
    /// A task with no blocked send of its own.
    fn sender_free(&self, t: u32) -> bool {
        !self.blocked.values().any(|(_, s)| *s == t)
    }
    /// The receiver runs its own receive: running and acting.
    fn receiving(&self, t: u32) -> bool {
        self.acting(t)
            && self
                .tasks
                .get(t as usize)
                .is_some_and(|x| x.phase == TaskPhase::Running)
    }
    /// A fresh send of `m` by `s` on `c`: the next message, while a kept sender remains
    /// to clone, by a sender with no other send pending.
    fn fresh_send(&self, c: u32, m: u64, s: u32) -> bool {
        m == self.next_message
            && self
                .channels
                .get(c as usize)
                .is_some_and(|ch| ch.senders_open)
            && self.sender_free(s)
    }

    fn late(&self) -> Option<u32> {
        (0..ord(self.timers.len())).find(|k| {
            let timer = &self.timers[*k as usize];
            timer.phase == TimerPhase::Armed && timer.deadline <= self.now
        })
    }
    /// docs/02 §7: `request(cancel_reason)`. The reason is `user` for a task in the
    /// region the request named, `parent-cancelled` below it.
    fn implied_cause(&self, t: u32) -> Option<Cause> {
        let task = self.tasks.get(t as usize)?;
        let origin = self.regions.get(task.region as usize)?.origin?;
        Some(if origin == task.region {
            Cause::User
        } else {
            Cause::ParentCancelled
        })
    }
    /// Whether a task has entered cancellation far enough that it can no longer act
    /// on its own (docs/02 §7: `Cancelling` only drains and finalizes).
    fn cancelling(&self, t: u32) -> bool {
        self.tasks.get(t as usize).is_some_and(|task| {
            task.cleaned_up
                || matches!(
                    task.cancel,
                    CancelPhase::Acknowledged(_) | CancelPhase::Done(_)
                )
        })
    }
    /// A cancellation-driven step by `t` is admitted: with the cancellation family, the
    /// task acknowledged its cancellation; without it, the task's region is cancelling.
    fn may_clean_up(&self, t: u32) -> bool {
        let Some(task) = self.tasks.get(t as usize) else {
            return false;
        };
        if task.phase.is_terminal() {
            return false;
        }
        if self.alphabet.has(FamilyTag::Cancellation) {
            matches!(task.cancel, CancelPhase::Acknowledged(_))
        } else {
            task.requested_alone
                || self
                    .regions
                    .get(task.region as usize)
                    .is_some_and(|g| g.phase == RegionPhase::Cancelling)
        }
    }
    /// A single-task cancellation of `t` is due: it has a budget deadline the clock has
    /// reached, when the run observes the clock (bn-36wy3).
    /// The task whose own cancellation is open: requested and not yet ended by its own
    /// `→ Cancelled`.
    fn open_alone(&self) -> Option<u32> {
        (0..ord(self.tasks.len())).find(|t| {
            let task = &self.tasks[*t as usize];
            task.requested_alone && !task.ended_alone
        })
    }
    /// Whether `step` may come while `t`'s own cancellation is open (RFC 0026 correction
    /// 53 item 6). The deadline's checkpoint is both its request and its acknowledgement,
    /// so nothing but that cancellation's own steps comes between `t`'s request and its
    /// own `→ Cancelled`: `t`'s cancellation phases, `t`'s cleanup once it may clean up
    /// (a cancel abort of its reservation, an aborted discharge of its obligation, its
    /// timer cancelled, its blocked send or receive abandoned, its receiver gone), and
    /// its own `→ Cancelled`. Everything else is refused, whoever's it is.
    fn inside_own_cancel(&self, t: u32, step: &Step) -> bool {
        let cleanup = match step {
            Step::CancelRequested { task, .. }
            | Step::CancelAcknowledged { task }
            | Step::CancelCompleted { task, .. }
            | Step::TaskCancelled { task } => return *task == t,
            Step::Abort {
                reservation,
                reason: AbortReason::Cancel,
            } => self.reservations.get(*reservation as usize).map(|e| e.task),
            Step::Discharged {
                obligation,
                committed: false,
            } => self.obligations.get(*obligation as usize).map(|o| o.holder),
            Step::TimerCancelled { timer, .. } => self.timers.get(*timer as usize).map(|k| k.task),
            Step::SendAbandoned { sender, .. } => Some(*sender),
            Step::RecvAbandoned { channel } | Step::ReceiverGone { channel, .. } => {
                self.channels.get(*channel as usize).map(|c| c.receiver)
            }
            _ => None,
        };
        cleanup == Some(t) && self.may_clean_up(t)
    }
    /// A live task `r` itself owns whose budget deadline the clock has reached: a
    /// cancellation of `r` now races it, and asupersync would complete it with the more
    /// severe `deadline` reason under the region's request, which no run journals (RFC
    /// 0026 correction 53 item 6; cr-3pu5cu round 7). A task of a proper subregion gets
    /// `parent-cancelled`, which outranks the deadline.
    fn raced_by_cancel(&self, r: u32) -> Option<u32> {
        (0..ord(self.tasks.len())).find(|t| {
            let task = &self.tasks[*t as usize];
            task.region == r && !task.phase.is_terminal() && self.deadline_reached(*t)
        })
    }
    /// The task whose own work `step` is, if it is any task's: a lifecycle step of a
    /// poll, a reserve, commit or explicit abort, an obligation open, committed discharge
    /// or hand-off (by its holder), a timer armed or fired, a send or a receive. Cleanup steps and
    /// steps that name no acting task are `None`.
    fn actor(&self, step: &Step) -> Option<u32> {
        match step {
            Step::Begin { task }
            | Step::Resume { task }
            | Step::Suspend { task }
            | Step::Complete { task }
            | Step::Fail { task, .. }
            | Step::Reserve { task, .. }
            | Step::Scheduled { task, .. } => Some(*task),
            // The sleeper's own poll traces its timer's fire.
            Step::Fired { timer, .. } => self.timers.get(*timer as usize).map(|k| k.task),
            Step::Commit { reservation }
            | Step::Abort {
                reservation,
                reason: AbortReason::Explicit,
            } => self.reservations.get(*reservation as usize).map(|e| e.task),
            Step::Opened { holder, .. } => Some(*holder),
            Step::Discharged {
                obligation,
                committed: true,
            }
            | Step::Transferred { obligation, .. } => {
                self.obligations.get(*obligation as usize).map(|o| o.holder)
            }
            Step::Sent { sender, .. }
            | Step::SendBlocked { sender, .. }
            | Step::SendClosed { sender, .. } => Some(*sender),
            Step::Received { channel, .. }
            | Step::RecvBlocked { channel }
            | Step::RecvClosed { channel } => {
                self.channels.get(*channel as usize).map(|c| c.receiver)
            }
            _ => None,
        }
    }
    /// docs/02 §7 `Active ─ request(cancel_reason) → Cancelling`: a request takes the
    /// task out of `Active` at once, and the substrate lets the task observe it at its
    /// next checkpoint, which starts its next poll. So a task whose cancellation was
    /// requested (by its region, or by its own deadline) and not yet observed (no
    /// acknowledgement, no cleanup step, not terminal) is waiting to observe it, and
    /// takes no step in the meantime (RFC 0026 correction 55, bn-28hup).
    fn unobserved(&self, t: u32) -> bool {
        self.tasks.get(t as usize).is_some_and(|task| {
            !task.phase.is_terminal()
                && !self.cancelling(t)
                && (task.region_requested
                    || task.requested_alone
                    || matches!(task.cancel, CancelPhase::Requested(_)))
        })
    }
    /// The task that `step` hands something to: the new holder of a hand-off, or the
    /// receiver of a new channel.
    fn recipient(step: &Step) -> Option<u32> {
        match step {
            Step::Transferred { holder, .. } => Some(*holder),
            Step::ChannelOpened { receiver, .. } => Some(*receiver),
            _ => None,
        }
    }
    /// The task whose cleanup or end `step` is, among the steps no other rule of the
    /// model holds to the observation: an aborted discharge or a leak (its holder), a
    /// receiver's drop (its task).
    fn ending(&self, step: &Step) -> Option<u32> {
        match step {
            Step::Discharged {
                obligation,
                committed: false,
            }
            | Step::Leaked { obligation } => {
                self.obligations.get(*obligation as usize).map(|o| o.holder)
            }
            Step::ReceiverGone { channel, .. } => {
                self.channels.get(*channel as usize).map(|c| c.receiver)
            }
            _ => None,
        }
    }
    /// Whether `step` is a step of a task that has not observed its requested
    /// cancellation ([`Self::unobserved`]): its own work or a receipt under every
    /// alphabet, and its cleanup or end when the alphabet observes phases.
    fn before_observation(&self, step: &Step) -> Option<u32> {
        let ending = if self.alphabet.has(FamilyTag::Cancellation) {
            self.ending(step)
        } else {
            None
        };
        [self.actor(step), Self::recipient(step), ending]
            .into_iter()
            .flatten()
            .find(|t| self.unobserved(*t))
    }
    /// The run observes the clock, and it has reached `t`'s budget deadline: a poll of
    /// `t` now is its deadline's cancellation, never a `begin` or `resume` (asupersync
    /// 0.5.0 raises `CancelKind::Deadline` at the poll's `Cx::checkpoint`; RFC 0026
    /// correction 53 item 6).
    fn deadline_reached(&self, t: u32) -> bool {
        self.alphabet.has(FamilyTag::Time)
            && self
                .tasks
                .get(t as usize)
                .and_then(|task| task.deadline)
                .is_some_and(|deadline| deadline <= self.now)
    }
    fn deadline_due(&self, t: u32) -> bool {
        !self.alphabet.has(FamilyTag::Time)
            || self
                .tasks
                .get(t as usize)
                .and_then(|task| task.deadline)
                .is_some_and(|deadline| deadline <= self.now)
    }
    /// The tasks a drain of `r` cancels, or the task that blocks it.
    fn drain_outcome(&self, r: u32) -> Result<Vec<u32>, Fault> {
        let mut out = Vec::new();
        for t in 0..ord(self.tasks.len()) {
            let task = &self.tasks[t as usize];
            if task.phase.is_terminal() || !self.in_subtree(task.region, r) {
                continue;
            }
            match self.regions[task.region as usize].phase {
                RegionPhase::Cancelling => out.push(t),
                _ => return Err(Fault::DrainBlocked { region: r, task: t }),
            }
        }
        Ok(out)
    }
    fn balance(&self, r: u32) -> (Vec<u32>, Vec<u32>, Vec<u32>) {
        let pick = |phase| {
            (0..ord(self.obligations.len()))
                .filter(|o| {
                    let ob = &self.obligations[*o as usize];
                    ob.region == r && ob.phase == phase
                })
                .collect()
        };
        (
            pick(ObligationPhase::Open),
            pick(ObligationPhase::Leaked),
            pick(ObligationPhase::Fenced),
        )
    }
    /// A region of `r`'s subtree under cancellation, or crashed and not yet finalized: an
    /// incarnation that is already being torn down does not crash.
    fn crash_blocked(&self, r: u32) -> Option<u32> {
        self.subtree(r).into_iter().find(|s| {
            let g = &self.regions[*s as usize];
            g.phase == RegionPhase::Cancelling || (g.crashed && g.phase != RegionPhase::Finalized)
        })
    }
    /// The live tasks of `r`'s subtree: what a crash of `r` stops.
    fn crash_set(&self, r: u32) -> Vec<u32> {
        (0..ord(self.tasks.len()))
            .filter(|t| {
                let task = &self.tasks[*t as usize];
                !task.phase.is_terminal() && self.in_subtree(task.region, r)
            })
            .collect()
    }
    /// A live task a crash may stop (bn-20d8u): not in a cancellation (its own, its
    /// region's, requested or observed), not under a budget deadline, and holding no
    /// channel. The process profile's crash lands at a step boundary of a task that acts
    /// on its own; the model gives the others no crash semantics.
    fn crashable(&self, t: u32) -> bool {
        self.tasks.get(t as usize).is_some_and(|task| {
            task.cancel == CancelPhase::None
                && !task.requested_alone
                && !task.region_requested
                && !task.cleaned_up
                && task.deadline.is_none()
                && !self.holds_channel(t)
        })
    }
    /// What a crash of `stopped` owes (`process/crash-restart-v0` rows `fail-stop-crash`
    /// and `late-completion`): every effect it had reserved, every obligation it held
    /// open and every timer it slept on, each of a family the alphabet observes.
    fn fences_of(&self, stopped: &[u32]) -> BTreeSet<(FamilyTag, u32)> {
        let mut out = BTreeSet::new();
        let stopped: BTreeSet<u32> = stopped.iter().copied().collect();
        if self.alphabet.has(FamilyTag::Effect) {
            for (e, res) in self.reservations.iter().enumerate() {
                if res.phase == EffectPhase::Reserved && stopped.contains(&res.task) {
                    out.insert((FamilyTag::Effect, ord(e)));
                }
            }
        }
        if self.alphabet.has(FamilyTag::Obligation) {
            for (o, ob) in self.obligations.iter().enumerate() {
                if ob.phase == ObligationPhase::Open && stopped.contains(&ob.holder) {
                    out.insert((FamilyTag::Obligation, ord(o)));
                }
            }
        }
        if self.alphabet.has(FamilyTag::Time) {
            for (k, timer) in self.timers.iter().enumerate() {
                if timer.phase == TimerPhase::Armed && stopped.contains(&timer.task) {
                    out.insert((FamilyTag::Time, ord(k)));
                }
            }
        }
        out
    }
    /// The fence `step` is, if it is one.
    const fn fence(step: &Step) -> Option<(FamilyTag, u32)> {
        match step {
            Step::EffectFenced { reservation } => Some((FamilyTag::Effect, *reservation)),
            Step::ObligationFenced { obligation } => Some((FamilyTag::Obligation, *obligation)),
            Step::TimerFenced { timer } => Some((FamilyTag::Time, *timer)),
            _ => None,
        }
    }

    // --- the guard formulation -----------------------------------------------------

    /// Take `step` if it is enabled.
    ///
    /// # Errors
    ///
    /// The [`Fault`] that disables it. The state is unchanged on error.
    pub fn step(&mut self, step: &Step) -> Result<(), Fault> {
        if !self.alphabet.has(step.family()) {
            return Err(Fault::OutsideAlphabet);
        }
        self.guard(step)?;
        self.apply(step);
        self.last_spawn = match step {
            Step::Spawn { task, .. } => Some(*task),
            _ => None,
        };
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn guard(&self, step: &Step) -> Result<(), Fault> {
        let fresh = |expected: usize, found: u32| {
            if ord(expected) == found {
                Ok(())
            } else {
                Err(Fault::Identity {
                    expected: ord(expected),
                    found,
                })
            }
        };
        // `process/crash-restart-v0`: the crash is one step, and what its tasks held is
        // fenced with it, before anything else happens (RFC 0026 correction 58).
        // Exactly the next owed fence, in the binding's order (effect, obligation, time;
        // each by ordinal), so a crash has one spelling.
        if let Some(&(family, ordinal)) = self.owed.first() {
            if Self::fence(step) != Some((family, ordinal)) {
                return Err(Fault::FenceOwed { family, ordinal });
            }
        }
        if let Some(t) = self.open_alone() {
            if !self.inside_own_cancel(t, step) {
                return Err(Fault::OwnCancellationInterrupted(t));
            }
        }
        // A task's own work is part of a poll, and a poll meets a passed deadline at its
        // first checkpoint (docs/02 §5; cr-3pu5cu round 6).
        if let Some(t) = self.actor(step) {
            if self.deadline_reached(t) {
                return Err(Fault::Deadline(t));
            }
        }
        // A requested task observes its cancellation before it does anything else.
        if let Some(t) = self.before_observation(step) {
            let task = &self.tasks[t as usize];
            let unreported = self.alphabet.has(FamilyTag::Cancellation)
                && task.cancel == CancelPhase::None
                && !task.requested_alone;
            return Err(if unreported {
                Fault::UnreportedRequest(t)
            } else {
                Fault::Unobserved(t)
            });
        }
        match step {
            // plan §4.1 / docs/02 §7: work enters only an open region.
            Step::OpenRegion { region, parent } => {
                fresh(self.regions.len(), *region)?;
                if self.region(*parent)?.phase != RegionPhase::Open {
                    return Err(Fault::RegionNotOpen(*parent));
                }
            }
            Step::Spawn { task, region, .. } => {
                fresh(self.tasks.len(), *task)?;
                if self.region(*region)?.phase != RegionPhase::Open {
                    return Err(Fault::RegionNotOpen(*region));
                }
            }
            // plan §4.1: Created → Running. docs/02 §7: a task still `Active` (at most
            // requested, not yet cancelling) may be polled.
            Step::Begin { task } => {
                let t = self.task(*task)?;
                if t.phase != TaskPhase::Created {
                    return Err(Fault::TaskPhase(*task));
                }
                if self.cancelling(*task) {
                    return Err(Fault::CancelPhase(*task));
                }
                if self.deadline_reached(*task) {
                    return Err(Fault::Deadline(*task));
                }
            }
            // A task may not park mid-publication: docs/02 §7 "cancellation does not
            // create a visible half-effect" needs a parked task to hold no reserved
            // effect.
            Step::Suspend { task } => {
                let t = self.task(*task)?;
                if t.phase != TaskPhase::Running {
                    return Err(Fault::TaskPhase(*task));
                }
                if self.cancelling(*task) {
                    return Err(Fault::CancelPhase(*task));
                }
                if self.staged(*task) {
                    return Err(Fault::HalfEffect { task: *task });
                }
            }
            // A sleeping task is woken by its timer (docs/02 §5, virtual time).
            Step::Resume { task } => {
                let t = self.task(*task)?;
                if t.phase != TaskPhase::Suspended {
                    return Err(Fault::TaskPhase(*task));
                }
                if self.cancelling(*task) {
                    return Err(Fault::CancelPhase(*task));
                }
                if self.deadline_reached(*task) {
                    return Err(Fault::Deadline(*task));
                }
                if self.armed(*task) {
                    return Err(Fault::TimerOutlivesSleep { task: *task });
                }
            }
            // docs/02 §7: `Active ─ complete(value) → Completed`, only while active.
            Step::Complete { task } | Step::Fail { task, .. } => {
                let t = self.task(*task)?;
                let from_ok = match step {
                    Step::Complete { .. } => t.phase == TaskPhase::Running,
                    _ => matches!(t.phase, TaskPhase::Created | TaskPhase::Running),
                };
                if !from_ok {
                    return Err(Fault::TaskPhase(*task));
                }
                if t.cancel != CancelPhase::None || t.cleaned_up {
                    return Err(Fault::CancelPhase(*task));
                }
                // With phases observed, a cancellation that reached the task is
                // reported before the task ends (cr-3pu5cu round 6).
                if self.alphabet.has(FamilyTag::Cancellation) && t.region_requested {
                    return Err(Fault::UnreportedRequest(*task));
                }
                // A poll past the deadline is its cancellation (docs/02 §5).
                if self.deadline_reached(*task) {
                    return Err(Fault::Deadline(*task));
                }
                // docs/02 §7: `complete` and `panic` leave `Active` only, and a task's
                // own request has left it. RFC 0026 correction 53 item 6: the journal
                // of a task's own cancellation ends with its own lifecycle `cancel`.
                if t.requested_alone {
                    return Err(Fault::OwnCancellationUnended(*task));
                }
                if self.staged(*task) {
                    return Err(Fault::HalfEffect { task: *task });
                }
                if self.armed(*task) {
                    return Err(Fault::TimerOutlivesSleep { task: *task });
                }
                if self.holds_channel(*task) {
                    return Err(Fault::ChannelOutlivesTask { task: *task });
                }
            }
            Step::Close { region } => {
                if self.region(*region)?.phase != RegionPhase::Open {
                    return Err(Fault::RegionPhase(*region));
                }
            }
            // `process/crash-restart-v0` row `fail-stop-crash`: the node's running
            // incarnation stops at a step boundary, all of its live tasks at once, and
            // nothing of it runs after. An incarnation is a region that still runs: open,
            // or closing while its tasks finish; a region under cancellation or torn
            // down is not one, and a crashed one does not crash again.
            Step::Crash { region, stopped } => {
                let g = self.region(*region)?;
                if !matches!(g.phase, RegionPhase::Open | RegionPhase::Closing)
                    || g.drained
                    || g.crashed
                {
                    return Err(Fault::RegionPhase(*region));
                }
                if let Some(r) = self.crash_blocked(*region) {
                    return Err(Fault::RegionPhase(r));
                }
                // A due timer fires before anything else happens: a crash may not fence it.
                if let Some(k) = self.late() {
                    return Err(Fault::LateTimer(k));
                }
                let model = self.crash_set(*region);
                if &model != stopped {
                    return Err(Fault::CrashSet {
                        region: *region,
                        model,
                    });
                }
                if let Some(t) = model.iter().find(|t| !self.crashable(**t)) {
                    return Err(Fault::CrashUnsupported(*t));
                }
            }
            // Row `late-completion`: a fence is owed by the crash that stopped its holder.
            Step::EffectFenced { .. }
            | Step::ObligationFenced { .. }
            | Step::TimerFenced { .. } => {
                let fence = Self::fence(step).unwrap_or((FamilyTag::Lifecycle, 0));
                if !self.owed.contains(&fence) {
                    return Err(Fault::FenceUnowed {
                        family: fence.0,
                        ordinal: fence.1,
                    });
                }
            }
            // A cancel request may upgrade a close; it may not revisit a cancelled or
            // finalized region.
            Step::Cancel { region } => {
                let g = self.region(*region)?;
                if !matches!(g.phase, RegionPhase::Open | RegionPhase::Closing) || g.drained {
                    return Err(Fault::RegionPhase(*region));
                }
                if let Some(t) = self.raced_by_cancel(*region) {
                    return Err(Fault::CancelRaced(t));
                }
            }
            // docs/02 §7 lifecycle: `Cancelling ─ drain* ─ finalize* ─ obligations == ∅
            // → Cancelled`. A normal close waits for owned work; a cancelled subtree's
            // drain moves every live task to Cancelled, and only after the task's own
            // phases, effects, timers and obligations are done.
            Step::Drain { region, cancelled } => {
                let g = self.region(*region)?;
                if !matches!(g.phase, RegionPhase::Closing | RegionPhase::Cancelling) || g.drained {
                    return Err(Fault::RegionPhase(*region));
                }
                let model = self.drain_outcome(*region)?;
                if &model != cancelled {
                    return Err(Fault::DrainSet {
                        region: *region,
                        model,
                    });
                }
                for t in &model {
                    // RFC 0026 correction 53 items 3 and 6: a task's own cancellation
                    // ends with its own single-task `→ Cancelled`, never with a
                    // region's drain, whatever its phases say.
                    if self.tasks[*t as usize].requested_alone {
                        return Err(Fault::OwnCancellationUnended(*t));
                    }
                    if self.alphabet.has(FamilyTag::Cancellation)
                        && !matches!(self.tasks[*t as usize].cancel, CancelPhase::Done(_))
                    {
                        return Err(Fault::UnphasedCancellation(*t));
                    }
                    if self.staged(*t) {
                        return Err(Fault::HalfEffect { task: *t });
                    }
                    if self.armed(*t) {
                        return Err(Fault::TimerOutlivesSleep { task: *t });
                    }
                    if self.holds(*t) {
                        return Err(Fault::HeldObligation { task: *t });
                    }
                    if self.holds_channel(*t) {
                        return Err(Fault::ChannelOutlivesTask { task: *t });
                    }
                }
            }
            // docs/02 §7 "region close implies no descendant tasks": every region in
            // the subtree drained, every task in it terminal.
            Step::Finalize { region } => {
                let g = self.region(*region)?;
                if !g.drained || g.phase == RegionPhase::Finalized {
                    return Err(Fault::RegionPhase(*region));
                }
                for r in self.subtree(*region) {
                    let sub = &self.regions[r as usize];
                    if !sub.drained && sub.phase != RegionPhase::Finalized {
                        return Err(Fault::FinalizeEarly(*region));
                    }
                }
                if self
                    .tasks
                    .iter()
                    .any(|t| !t.phase.is_terminal() && self.in_subtree(t.region, *region))
                {
                    return Err(Fault::FinalizeEarly(*region));
                }
            }
            // docs/02 §7 `Active ─ request(cancel_reason) → Cancelling`: requested only
            // for a live task whose region is being cancelled, with the reason the tree
            // implies.
            // A deadline's request is the task's own: it follows the task's lifecycle
            // `cancel-requested`, whatever its region's state.
            Step::CancelRequested {
                task,
                cause: Cause::Deadline,
            } => {
                let t = self.task(*task)?;
                if t.phase.is_terminal() || !t.requested_alone {
                    return Err(Fault::TaskPhase(*task));
                }
                if t.cancel != CancelPhase::None {
                    return Err(Fault::CancelPhase(*task));
                }
            }
            Step::CancelRequested { task, cause } => {
                let t = self.task(*task)?;
                if t.phase.is_terminal()
                    || self.regions[t.region as usize].phase != RegionPhase::Cancelling
                {
                    return Err(Fault::TaskPhase(*task));
                }
                if t.cancel != CancelPhase::None {
                    return Err(Fault::CancelPhase(*task));
                }
                if self.implied_cause(*task) != Some(*cause) {
                    return Err(Fault::Cause(*task));
                }
            }
            // docs/02 §7 `Active ─ request(cancel_reason)` for one task: requested once,
            // while it is live and no region's cancellation reached it, once the clock
            // reached its deadline (docs/02 §5; bn-36wy3).
            Step::TaskCancelRequested { task } => {
                let t = self.task(*task)?;
                if t.phase.is_terminal()
                    || self.regions[t.region as usize].phase == RegionPhase::Cancelling
                {
                    return Err(Fault::TaskPhase(*task));
                }
                if t.requested_alone || t.cancel != CancelPhase::None {
                    return Err(Fault::CancelPhase(*task));
                }
                if !self.deadline_due(*task) {
                    return Err(Fault::Deadline(*task));
                }
            }
            // The single task's `→ Cancelled`: after its own phases when the run observes
            // them, otherwise with the cleanup they would show done (bn-36wy3).
            Step::TaskCancelled { task } => {
                let t = self.task(*task)?;
                if t.phase.is_terminal() || !t.requested_alone {
                    return Err(Fault::TaskPhase(*task));
                }
                if self.alphabet.has(FamilyTag::Cancellation) {
                    match t.cancel {
                        CancelPhase::Done(Cause::Deadline) => {}
                        CancelPhase::Done(_) => return Err(Fault::Cause(*task)),
                        _ => return Err(Fault::CancelPhase(*task)),
                    }
                } else {
                    if self.staged(*task) {
                        return Err(Fault::HalfEffect { task: *task });
                    }
                    if self.holds(*task) {
                        return Err(Fault::HeldObligation { task: *task });
                    }
                    if self.armed(*task) {
                        return Err(Fault::TimerOutlivesSleep { task: *task });
                    }
                    if self.holds_channel(*task) {
                        return Err(Fault::ChannelOutlivesTask { task: *task });
                    }
                }
            }
            // docs/02 §5: a budget deadline is declared with the task, ahead of the clock.
            Step::DeadlineSet { task, deadline } => {
                let t = self.task(*task)?;
                // docs/02 §5: the budget is the spawn's, so its deadline is declared by
                // the step right after the task's spawn (cr-3pu5cu round 8).
                if self.last_spawn != Some(*task) {
                    return Err(Fault::DeadlineNotAtSpawn(*task));
                }
                if t.phase != TaskPhase::Created || t.deadline.is_some() {
                    return Err(Fault::TaskPhase(*task));
                }
                if *deadline <= self.now {
                    return Err(Fault::NotAhead);
                }
            }
            Step::CancelAcknowledged { task } => {
                let t = self.task(*task)?;
                if t.phase.is_terminal() || !matches!(t.cancel, CancelPhase::Requested(_)) {
                    return Err(Fault::CancelPhase(*task));
                }
            }
            // `obligations == ∅ → Cancelled`, and no effect left reserved.
            Step::CancelCompleted { task, cause } => {
                let t = self.task(*task)?;
                match t.cancel {
                    CancelPhase::Acknowledged(c) if c == *cause && !t.phase.is_terminal() => {}
                    CancelPhase::Acknowledged(_) => return Err(Fault::Cause(*task)),
                    _ => return Err(Fault::CancelPhase(*task)),
                }
                if self.staged(*task) {
                    return Err(Fault::HalfEffect { task: *task });
                }
                if self.holds(*task) {
                    return Err(Fault::HeldObligation { task: *task });
                }
                if self.armed(*task) {
                    return Err(Fault::TimerOutlivesSleep { task: *task });
                }
                if self.holds_channel(*task) {
                    return Err(Fault::ChannelOutlivesTask { task: *task });
                }
            }
            // docs/02 §7 effect protocol `Idle → Reserved(token)`, by a running task
            // that is not cancelling. A task may hold several reserved effects at once,
            // each resolved exactly once (RFC 0026 correction 51 item 2, bn-j1a50).
            Step::Reserve { reservation, task } => {
                fresh(self.reservations.len(), *reservation)?;
                let t = self.task(*task)?;
                if t.phase != TaskPhase::Running {
                    return Err(Fault::TaskPhase(*task));
                }
                if t.cancel != CancelPhase::None {
                    return Err(Fault::CancelPhase(*task));
                }
            }
            // `Reserved → Committed(result)`, by the running holder, before it starts
            // cancelling ("committed effects cannot be silently rolled back").
            Step::Commit { reservation } => {
                let e = self.reservation(*reservation)?;
                if e.phase != EffectPhase::Reserved {
                    return Err(Fault::EffectPhase(*reservation));
                }
                let t = self.task(e.task)?;
                if t.phase != TaskPhase::Running || self.cancelling(e.task) {
                    return Err(Fault::TaskPhase(e.task));
                }
            }
            // `Reserved → Aborted(reason)`: a cancel abort is part of `Cancelling ─
            // drain(effect)*`; an explicit abort is the running holder's.
            Step::Abort {
                reservation,
                reason,
            } => {
                let e = self.reservation(*reservation)?;
                if e.phase != EffectPhase::Reserved {
                    return Err(Fault::EffectPhase(*reservation));
                }
                let ok = match reason {
                    AbortReason::Cancel => self.may_clean_up(e.task),
                    AbortReason::Explicit => {
                        self.task(e.task)?.phase == TaskPhase::Running && !self.cancelling(e.task)
                    }
                };
                if !ok {
                    return Err(Fault::TaskPhase(e.task));
                }
            }
            // docs/01 §6 obligation = linear resource delta; docs/02 §7 "every
            // obligation has one owner": opened by an acting holder, in the holder's
            // region, while that region accepts work. The ledger is the substrate's
            // (every kind), and the substrate applies a registration after the poll
            // that made it, so the holder may already be parked again: the rule is
            // "acting", not "running".
            Step::Opened {
                obligation,
                holder,
                region,
                ..
            } => {
                fresh(self.obligations.len(), *obligation)?;
                let t = self.task(*holder)?;
                if !self.acting(*holder) {
                    return Err(Fault::TaskPhase(*holder));
                }
                if t.region != *region {
                    return Err(Fault::ObligationRegion(*obligation));
                }
                if self.region(*region)?.phase != RegionPhase::Open {
                    return Err(Fault::RegionNotOpen(*region));
                }
            }
            // "… or is discharged": once, while open, by its holder (a commit by an
            // acting holder; an abort by an acting or a cleaning-up holder).
            Step::Discharged {
                obligation,
                committed,
            } => {
                let o = self.obligation(*obligation)?;
                if o.phase != ObligationPhase::Open {
                    return Err(Fault::ObligationPhase(*obligation));
                }
                let ok = self.acting(o.holder) || (!*committed && self.may_clean_up(o.holder));
                if !ok {
                    return Err(Fault::TaskPhase(o.holder));
                }
            }
            // "ownership transfer is causal": the acting holder hands an open
            // obligation to another acting task, which then owns it in its own region.
            Step::Transferred {
                obligation,
                holder,
                region,
            } => {
                let o = self.obligation(*obligation)?;
                if o.phase != ObligationPhase::Open {
                    return Err(Fault::ObligationPhase(*obligation));
                }
                if !self.acting(o.holder) {
                    return Err(Fault::TaskPhase(o.holder));
                }
                let to = self.task(*holder)?;
                if *holder == o.holder || !self.acting(*holder) {
                    return Err(Fault::ObligationPhase(*obligation));
                }
                if to.region != *region {
                    return Err(Fault::ObligationRegion(*obligation));
                }
                if self.region(*region)?.phase == RegionPhase::Finalized {
                    return Err(Fault::RegionNotOpen(*region));
                }
            }
            // A leak is the holder ending with the obligation open. The substrate
            // records it inside the poll that returns the holder's result, so the
            // holder is running (in its last poll) or already terminal; `finish` holds
            // the running case to its end.
            Step::Leaked { obligation } => {
                let o = self.obligation(*obligation)?;
                if o.phase != ObligationPhase::Open {
                    return Err(Fault::ObligationPhase(*obligation));
                }
                let t = self.task(o.holder)?;
                if !t.phase.is_terminal() && t.phase != TaskPhase::Running {
                    return Err(Fault::TaskPhase(o.holder));
                }
            }
            // "region close implies no descendant … obligations": the balance reported
            // at close is the ledger's own, and it must be empty.
            // A fenced obligation is the crash's, reported apart; it is not the region's
            // own close failing (bn-20d8u).
            Step::Settled {
                region,
                open,
                leaked,
                fenced,
            } => {
                let g = self.region(*region)?;
                if g.phase != RegionPhase::Finalized || g.settled {
                    return Err(Fault::Settle(*region));
                }
                let (model_open, model_leaked, model_fenced) = self.balance(*region);
                if &model_open != open || &model_leaked != leaked || &model_fenced != fenced {
                    return Err(Fault::Settle(*region));
                }
                if !open.is_empty() || !leaked.is_empty() {
                    return Err(Fault::CloseWithObligations(*region));
                }
            }
            // docs/02 §5 virtual execution time, controlled by Lab. A running task arms
            // one timer at the current instant for a deadline ahead of it; no armed
            // timer may be overdue.
            Step::Scheduled {
                timer,
                task,
                at,
                deadline,
            } => {
                fresh(self.timers.len(), *timer)?;
                if *at != self.now {
                    return Err(Fault::OffClock {
                        now: self.now,
                        at: *at,
                    });
                }
                if deadline <= at {
                    return Err(Fault::NotAhead);
                }
                let t = self.task(*task)?;
                if t.phase != TaskPhase::Running || t.cancel != CancelPhase::None || t.cleaned_up {
                    return Err(Fault::TaskPhase(*task));
                }
                if self.armed(*task) {
                    return Err(Fault::TimerPhase(*timer));
                }
                if let Some(k) = self.late() {
                    return Err(Fault::LateTimer(k));
                }
            }
            // The clock is monotone, and moves only when no armed timer is overdue.
            Step::Advanced { from, to } => {
                if *from != self.now {
                    return Err(Fault::OffClock {
                        now: self.now,
                        at: *from,
                    });
                }
                if to <= from {
                    return Err(Fault::NotAhead);
                }
                if let Some(k) = self.late() {
                    return Err(Fault::LateTimer(k));
                }
            }
            // A timer fires at the current instant, at or after its deadline.
            Step::Fired { timer, at } => {
                if *at != self.now {
                    return Err(Fault::OffClock {
                        now: self.now,
                        at: *at,
                    });
                }
                let k = self.timer(*timer)?;
                if k.phase != TimerPhase::Armed || k.deadline > self.now {
                    return Err(Fault::TimerPhase(*timer));
                }
            }
            // A timer is cancelled only by its task's cancellation.
            Step::TimerCancelled { timer, at } => {
                if *at != self.now {
                    return Err(Fault::OffClock {
                        now: self.now,
                        at: *at,
                    });
                }
                let k = self.timer(*timer)?;
                if k.phase != TimerPhase::Armed {
                    return Err(Fault::TimerPhase(*timer));
                }
                if !self.may_clean_up(k.task) {
                    return Err(Fault::TaskPhase(k.task));
                }
            }
            _ => self.channel_guard(step)?,
        }
        Ok(())
    }

    /// The channel rules: a bounded multi-producer, single-consumer FIFO channel whose
    /// send is a two-phase effect (docs/02 §7), owned by tasks (docs/01 §6: a task is
    /// "task identity and program order", a region the lifecycle resource).
    #[allow(clippy::too_many_lines)]
    fn channel_guard(&self, step: &Step) -> Result<(), Fault> {
        let rule = |channel: u32, rule: &'static str| Err(Fault::Channel { channel, rule });
        match step {
            // A channel opens once, with room for at least one message, its receiver
            // handed to an acting task.
            Step::ChannelOpened {
                channel,
                capacity,
                receiver,
            } => {
                if *channel != ord(self.channels.len()) {
                    return Err(Fault::Identity {
                        expected: ord(self.channels.len()),
                        found: *channel,
                    });
                }
                if *capacity == 0 {
                    return rule(*channel, "capacity");
                }
                self.task(*receiver)?;
                if !self.acting(*receiver) {
                    return Err(Fault::TaskPhase(*receiver));
                }
            }
            // FIFO and bounded: a message enters a channel whose receiver is there and
            // that has room, once. The send is the commit of the sender's permit.
            Step::Sent {
                channel,
                message,
                sender,
            } => {
                let ch = self.channel(*channel)?;
                self.task(*sender)?;
                if !ch.present {
                    return rule(*channel, "send after the receiver went");
                }
                if ch.queue.len() >= ch.capacity as usize {
                    return rule(*channel, "send into a full channel");
                }
                let unblocks = self.blocked.get(message) == Some(&(*channel, *sender));
                if !unblocks && !self.fresh_send(*channel, *message, *sender) {
                    return rule(*channel, "message identity");
                }
                if !self.acting(*sender) {
                    return Err(Fault::TaskPhase(*sender));
                }
                if !self.permit_ok(*sender) {
                    return Err(Fault::NoSendPermit { task: *sender });
                }
            }
            // Backpressure: a send waits only on a full channel.
            Step::SendBlocked {
                channel,
                message,
                sender,
            } => {
                let ch = self.channel(*channel)?;
                self.task(*sender)?;
                if !ch.present || ch.queue.len() < ch.capacity as usize {
                    return rule(*channel, "send blocked without a full channel");
                }
                if !self.fresh_send(*channel, *message, *sender) {
                    return rule(*channel, "message identity");
                }
                if !self.acting(*sender) {
                    return Err(Fault::TaskPhase(*sender));
                }
            }
            // A send fails as closed only once the receiver is gone.
            Step::SendClosed {
                channel,
                message,
                sender,
            } => {
                let ch = self.channel(*channel)?;
                self.task(*sender)?;
                if ch.present {
                    return rule(*channel, "send closed while the receiver is there");
                }
                let unblocks = self.blocked.get(message) == Some(&(*channel, *sender));
                if !unblocks && !self.fresh_send(*channel, *message, *sender) {
                    return rule(*channel, "message identity");
                }
                if !self.acting(*sender) {
                    return Err(Fault::TaskPhase(*sender));
                }
            }
            // A blocked send is dropped only by its sender's cancellation cleanup.
            Step::SendAbandoned {
                channel,
                message,
                sender,
            } => {
                self.channel(*channel)?;
                if self.blocked.get(message) != Some(&(*channel, *sender)) {
                    return rule(*channel, "abandoned send was not blocked");
                }
                if !self.may_clean_up(*sender) {
                    return Err(Fault::TaskPhase(*sender));
                }
            }
            // FIFO: the receiver takes the oldest queued message.
            Step::Received { channel, message } => {
                let ch = self.channel(*channel)?;
                if !ch.present || ch.queue.front() != Some(message) {
                    return rule(*channel, "received out of FIFO order");
                }
                if !self.receiving(ch.receiver) {
                    return Err(Fault::TaskPhase(ch.receiver));
                }
            }
            // A receive waits only on an empty channel something can still send on.
            Step::RecvBlocked { channel } => {
                let ch = self.channel(*channel)?;
                if !ch.present
                    || !ch.queue.is_empty()
                    || ch.recv_blocked
                    || !self.can_still_receive(*channel)
                {
                    return rule(*channel, "receive blocked on a channel that can deliver");
                }
                if !self.receiving(ch.receiver) {
                    return Err(Fault::TaskPhase(ch.receiver));
                }
            }
            // A receive returns closed only on an empty channel nothing can send on.
            Step::RecvClosed { channel } => {
                let ch = self.channel(*channel)?;
                if !ch.present || !ch.queue.is_empty() || self.can_still_receive(*channel) {
                    return rule(*channel, "receive closed on a channel that can deliver");
                }
                if !self.receiving(ch.receiver) {
                    return Err(Fault::TaskPhase(ch.receiver));
                }
            }
            // A blocked receive is dropped only by the receiver's cancellation cleanup.
            Step::RecvAbandoned { channel } => {
                let ch = self.channel(*channel)?;
                if !ch.recv_blocked {
                    return rule(*channel, "abandoned receive was not blocked");
                }
                if !self.may_clean_up(ch.receiver) {
                    return Err(Fault::TaskPhase(ch.receiver));
                }
            }
            Step::SendersClosed { channel } => {
                if !self.channel(*channel)?.senders_open {
                    return rule(*channel, "senders closed twice");
                }
            }
            // The receiver goes when its task ends (normally, or in its cancellation's
            // cleanup), and declares exactly the messages still queued.
            Step::ReceiverGone { channel, discarded } => {
                let ch = self.channel(*channel)?;
                if !ch.present || ch.recv_blocked {
                    return rule(*channel, "receiver gone twice or while receiving");
                }
                let mut queued: Vec<u64> = ch.queue.iter().copied().collect();
                queued.sort_unstable();
                if &queued != discarded {
                    return rule(*channel, "discarded set is not the queue");
                }
                if !self.receiving(ch.receiver) && !self.may_clean_up(ch.receiver) {
                    return Err(Fault::TaskPhase(ch.receiver));
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn reservation(&self, e: u32) -> Result<&Reservation, Fault> {
        self.reservations
            .get(e as usize)
            .ok_or(Fault::Unknown("reservation", e))
    }
    fn obligation(&self, o: u32) -> Result<&Obligation, Fault> {
        self.obligations
            .get(o as usize)
            .ok_or(Fault::Unknown("obligation", o))
    }
    fn timer(&self, k: u32) -> Result<&Timer, Fault> {
        self.timers
            .get(k as usize)
            .ok_or(Fault::Unknown("timer", k))
    }

    fn set_task(&mut self, t: u32, phase: TaskPhase) {
        self.tasks[t as usize].phase = phase;
    }

    fn apply(&mut self, step: &Step) {
        match step {
            Step::OpenRegion { parent, .. } => self.regions.push(Region {
                parent: Some(*parent),
                phase: RegionPhase::Open,
                origin: None,
                drained: false,
                settled: false,
                crashed: false,
            }),
            Step::Crash { region, stopped } => {
                for t in stopped {
                    self.set_task(*t, TaskPhase::Crashed);
                }
                for r in self.subtree(*region) {
                    let g = &mut self.regions[r as usize];
                    g.crashed = true;
                    if g.phase == RegionPhase::Open {
                        g.phase = RegionPhase::Closing;
                    }
                }
                self.owed = self.fences_of(stopped);
            }
            Step::EffectFenced { reservation } => {
                self.reservations[*reservation as usize].phase = EffectPhase::Fenced;
                self.owed.remove(&(FamilyTag::Effect, *reservation));
            }
            Step::ObligationFenced { obligation } => {
                self.obligations[*obligation as usize].phase = ObligationPhase::Fenced;
                self.owed.remove(&(FamilyTag::Obligation, *obligation));
            }
            Step::TimerFenced { timer } => {
                self.timers[*timer as usize].phase = TimerPhase::Fenced;
                self.owed.remove(&(FamilyTag::Time, *timer));
            }
            Step::Spawn { region, .. } => self.tasks.push(Task {
                region: *region,
                phase: TaskPhase::Created,
                cancel: CancelPhase::None,
                deadline: None,
                requested_alone: false,
                ended_alone: false,
                cleaned_up: false,
                region_requested: false,
            }),
            Step::DeadlineSet { task, deadline } => {
                self.tasks[*task as usize].deadline = Some(*deadline);
            }
            Step::TaskCancelRequested { task } => {
                self.tasks[*task as usize].requested_alone = true;
            }
            Step::TaskCancelled { task } => {
                self.set_task(*task, TaskPhase::Cancelled);
                self.tasks[*task as usize].ended_alone = true;
            }
            Step::Begin { task } | Step::Resume { task } => {
                self.set_task(*task, TaskPhase::Running)
            }
            Step::Suspend { task } => self.set_task(*task, TaskPhase::Suspended),
            Step::Complete { task } => self.set_task(*task, TaskPhase::Completed),
            Step::Fail { task, .. } => self.set_task(*task, TaskPhase::Failed),
            Step::Close { region } => {
                for r in self.subtree(*region) {
                    let g = &mut self.regions[r as usize];
                    if g.phase == RegionPhase::Open {
                        g.phase = RegionPhase::Closing;
                    }
                }
            }
            Step::Cancel { region } => {
                for r in self.subtree(*region) {
                    let g = &mut self.regions[r as usize];
                    if matches!(g.phase, RegionPhase::Open | RegionPhase::Closing) {
                        g.phase = RegionPhase::Cancelling;
                        g.origin = Some(*region);
                        for task in &mut self.tasks {
                            if task.region == r && !task.phase.is_terminal() {
                                task.region_requested = true;
                            }
                        }
                    }
                }
            }
            Step::Drain { region, cancelled } => {
                for t in cancelled {
                    self.set_task(*t, TaskPhase::Cancelled);
                }
                for r in self.subtree(*region) {
                    let g = &mut self.regions[r as usize];
                    if g.phase != RegionPhase::Finalized {
                        g.drained = true;
                    }
                }
            }
            Step::Finalize { region } => {
                for r in self.subtree(*region) {
                    self.regions[r as usize].phase = RegionPhase::Finalized;
                }
            }
            Step::CancelRequested { task, cause } => {
                self.tasks[*task as usize].cancel = CancelPhase::Requested(*cause);
            }
            Step::CancelAcknowledged { task } => {
                if let CancelPhase::Requested(c) = self.tasks[*task as usize].cancel {
                    self.tasks[*task as usize].cancel = CancelPhase::Acknowledged(c);
                }
            }
            Step::CancelCompleted { task, cause } => {
                self.tasks[*task as usize].cancel = CancelPhase::Done(*cause);
            }
            Step::Reserve { task, .. } => self.reservations.push(Reservation {
                task: *task,
                phase: EffectPhase::Reserved,
            }),
            Step::Commit { reservation } => {
                self.reservations[*reservation as usize].phase = EffectPhase::Committed;
            }
            Step::Abort {
                reservation,
                reason,
            } => {
                let e = &mut self.reservations[*reservation as usize];
                e.phase = EffectPhase::Aborted;
                if *reason == AbortReason::Cancel {
                    let t = e.task;
                    self.tasks[t as usize].cleaned_up = true;
                }
            }
            Step::Opened {
                kind,
                holder,
                region,
                ..
            } => self.obligations.push(Obligation {
                kind: *kind,
                holder: *holder,
                region: *region,
                phase: ObligationPhase::Open,
            }),
            Step::Discharged {
                obligation,
                committed,
            } => {
                let ob = &mut self.obligations[*obligation as usize];
                ob.phase = ObligationPhase::Discharged;
                if *committed && ob.kind == SEND_PERMIT {
                    *self.permits.entry(ob.holder).or_insert(0) += 1;
                }
            }
            Step::Transferred {
                obligation,
                holder,
                region,
            } => {
                let o = &mut self.obligations[*obligation as usize];
                o.holder = *holder;
                o.region = *region;
            }
            Step::Leaked { obligation } => {
                self.obligations[*obligation as usize].phase = ObligationPhase::Leaked;
            }
            Step::Settled { region, .. } => self.regions[*region as usize].settled = true,
            Step::Scheduled { task, deadline, .. } => self.timers.push(Timer {
                task: *task,
                deadline: *deadline,
                phase: TimerPhase::Armed,
            }),
            Step::Advanced { to, .. } => self.now = *to,
            Step::Fired { timer, .. } => self.timers[*timer as usize].phase = TimerPhase::Fired,
            Step::TimerCancelled { timer, .. } => {
                let k = &mut self.timers[*timer as usize];
                k.phase = TimerPhase::Cancelled;
                let t = k.task;
                self.tasks[t as usize].cleaned_up = true;
            }
            Step::ChannelOpened {
                capacity, receiver, ..
            } => self.channels.push(Channel {
                capacity: *capacity,
                receiver: *receiver,
                present: true,
                queue: std::collections::VecDeque::new(),
                senders_open: true,
                recv_blocked: false,
            }),
            Step::Sent {
                channel,
                message,
                sender,
            } => {
                if self.blocked.remove(message).is_none() {
                    self.next_message += 1;
                }
                self.channels[*channel as usize].queue.push_back(*message);
                if let Some(count) = self.permits.get_mut(sender) {
                    *count = count.saturating_sub(1);
                }
            }
            Step::SendBlocked {
                channel,
                message,
                sender,
            } => {
                self.next_message += 1;
                self.blocked.insert(*message, (*channel, *sender));
            }
            Step::SendClosed { message, .. } => {
                if self.blocked.remove(message).is_none() {
                    self.next_message += 1;
                }
            }
            Step::SendAbandoned {
                message, sender, ..
            } => {
                self.blocked.remove(message);
                self.tasks[*sender as usize].cleaned_up = true;
            }
            Step::Received { channel, .. } => {
                let ch = &mut self.channels[*channel as usize];
                ch.queue.pop_front();
                ch.recv_blocked = false;
            }
            Step::RecvBlocked { channel } => self.channels[*channel as usize].recv_blocked = true,
            Step::RecvClosed { channel } => {
                self.channels[*channel as usize].recv_blocked = false;
            }
            Step::RecvAbandoned { channel } => {
                let ch = &mut self.channels[*channel as usize];
                ch.recv_blocked = false;
                let t = ch.receiver;
                self.tasks[t as usize].cleaned_up = true;
            }
            Step::SendersClosed { channel } => {
                self.channels[*channel as usize].senders_open = false;
            }
            Step::ReceiverGone { channel, .. } => {
                let ch = &mut self.channels[*channel as usize];
                ch.present = false;
                ch.queue.clear();
            }
        }
    }

    /// The end conditions of an accepted trace.
    ///
    /// # Errors
    ///
    /// The first [`Fault`] among: a requested cancellation that never drained, a
    /// terminated task with a reserved effect, an armed timer or a channel it still
    /// holds, an overdue timer, a
    /// leak whose holder never ended, and a finalized region that never settled its
    /// obligations.
    pub fn finish(&self) -> Result<(), Fault> {
        // A crash's fences come within the step that crashed it.
        if let Some(&(family, ordinal)) = self.owed.first() {
            return Err(Fault::FenceOwed { family, ordinal });
        }
        for t in 0..ord(self.tasks.len()) {
            let task = &self.tasks[t as usize];
            if !task.phase.is_terminal()
                && (task.cancel != CancelPhase::None || task.requested_alone)
            {
                return Err(Fault::UndrainedCancellation(t));
            }
            // Provenance, not the shared terminal phase: a task requested alone ended
            // by its own `→ Cancelled` (cr-3pu5cu).
            if task.requested_alone && !task.ended_alone {
                return Err(Fault::OwnCancellationUnended(t));
            }
            if self.alphabet.has(FamilyTag::Cancellation)
                && task.region_requested
                && task.cancel == CancelPhase::None
            {
                return Err(Fault::UnreportedRequest(t));
            }
            if task.phase.is_terminal() && self.staged(t) {
                return Err(Fault::HalfEffect { task: t });
            }
            if task.phase.is_terminal() && self.armed(t) {
                return Err(Fault::TimerOutlivesSleep { task: t });
            }
            if task.phase.is_terminal() && self.holds_channel(t) {
                return Err(Fault::ChannelOutlivesTask { task: t });
            }
        }
        // A cancellation drains, and a drain finalizes, within the substrate operation
        // that started it: a trace that stops between them is unfinished.
        for r in 0..ord(self.regions.len()) {
            let g = &self.regions[r as usize];
            if (g.phase == RegionPhase::Cancelling && !g.drained)
                || (g.drained && g.phase != RegionPhase::Finalized)
                || (g.crashed && g.phase != RegionPhase::Finalized)
            {
                return Err(Fault::UnfinishedRegion(r));
            }
        }
        if self.alphabet.has(FamilyTag::Time) {
            if let Some(k) = self.late() {
                return Err(Fault::LateTimer(k));
            }
        }
        if self.alphabet.has(FamilyTag::Obligation) {
            for o in 0..ord(self.obligations.len()) {
                let ob = &self.obligations[o as usize];
                if ob.phase == ObligationPhase::Leaked
                    && !self.tasks[ob.holder as usize].phase.is_terminal()
                {
                    return Err(Fault::UnendedLeak(o));
                }
            }
            for r in 0..ord(self.regions.len()) {
                let g = &self.regions[r as usize];
                if g.phase == RegionPhase::Finalized && !g.settled {
                    return Err(Fault::Unsettled(r));
                }
            }
        }
        Ok(())
    }

    // --- the generator formulation -------------------------------------------------

    /// Every step enabled in this state, built from the state rather than checked
    /// against a candidate.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn enabled(&self) -> Vec<Pattern> {
        let mut out = Vec::new();
        // A crash's owed fences, and nothing else (bn-20d8u).
        if let Some(&(family, ordinal)) = self.owed.first() {
            {
                out.push(Pattern::Exact(match family {
                    FamilyTag::Effect => Step::EffectFenced {
                        reservation: ordinal,
                    },
                    FamilyTag::Obligation => Step::ObligationFenced {
                        obligation: ordinal,
                    },
                    _ => Step::TimerFenced { timer: ordinal },
                }));
            }
            return out;
        }
        let has = |f| self.alphabet.has(f);
        let regions = ord(self.regions.len());
        let tasks = ord(self.tasks.len());
        let open = |r: u32| self.regions[r as usize].phase == RegionPhase::Open;
        let active = |t: u32| !self.cancelling(t);

        // Lifecycle.
        for r in 0..regions {
            let g = &self.regions[r as usize];
            if open(r) {
                out.push(Pattern::Exact(Step::OpenRegion {
                    region: regions,
                    parent: r,
                }));
                out.push(Pattern::Spawn {
                    task: tasks,
                    region: r,
                });
                out.push(Pattern::Exact(Step::Close { region: r }));
            }
            if matches!(g.phase, RegionPhase::Open | RegionPhase::Closing)
                && !g.drained
                && self.raced_by_cancel(r).is_none()
            {
                out.push(Pattern::Exact(Step::Cancel { region: r }));
            }
            if matches!(g.phase, RegionPhase::Open | RegionPhase::Closing)
                && !g.drained
                && !g.crashed
                && self.crash_blocked(r).is_none()
                && self.late().is_none()
            {
                let stopped = self.crash_set(r);
                if stopped.iter().all(|t| self.crashable(*t)) {
                    out.push(Pattern::Exact(Step::Crash { region: r, stopped }));
                }
            }
            if matches!(g.phase, RegionPhase::Closing | RegionPhase::Cancelling) && !g.drained {
                if let Ok(cancelled) = self.drain_outcome(r) {
                    let ready = cancelled.iter().all(|t| {
                        !self.tasks[*t as usize].requested_alone
                            && (!has(FamilyTag::Cancellation)
                                || matches!(self.tasks[*t as usize].cancel, CancelPhase::Done(_)))
                            && !self.staged(*t)
                            && !self.armed(*t)
                            && !self.holds(*t)
                            && !self.holds_channel(*t)
                    });
                    if ready {
                        out.push(Pattern::Exact(Step::Drain {
                            region: r,
                            cancelled,
                        }));
                    }
                }
            }
            if g.drained && g.phase != RegionPhase::Finalized {
                let subtree = self.subtree(r);
                let regions_done = subtree.iter().all(|s| {
                    let sub = &self.regions[*s as usize];
                    sub.drained || sub.phase == RegionPhase::Finalized
                });
                let tasks_done = self
                    .tasks
                    .iter()
                    .all(|t| t.phase.is_terminal() || !subtree.contains(&t.region));
                if regions_done && tasks_done {
                    out.push(Pattern::Exact(Step::Finalize { region: r }));
                }
            }
        }
        for t in 0..tasks {
            let task = &self.tasks[t as usize];
            let quiet = !self.staged(t) && !self.armed(t) && !self.holds_channel(t);
            // One task's own cancellation (bn-36wy3): requested once its deadline is due
            // and no region's cancellation reached it; ended after its own phases, or
            // with its cleanup done when the run does not observe them.
            if !task.phase.is_terminal() {
                if !task.requested_alone
                    && task.cancel == CancelPhase::None
                    && self.regions[task.region as usize].phase != RegionPhase::Cancelling
                    && self.deadline_due(t)
                {
                    out.push(Pattern::Exact(Step::TaskCancelRequested { task: t }));
                }
                let ended = if has(FamilyTag::Cancellation) {
                    task.cancel == CancelPhase::Done(Cause::Deadline)
                } else {
                    quiet && !self.holds(t)
                };
                if task.requested_alone && ended {
                    out.push(Pattern::Exact(Step::TaskCancelled { task: t }));
                }
            }
            match task.phase {
                TaskPhase::Created => {
                    if active(t) && !self.deadline_reached(t) {
                        out.push(Pattern::Exact(Step::Begin { task: t }));
                    }
                    if task.cancel == CancelPhase::None
                        && !task.requested_alone
                        && !task.cleaned_up
                        && !(has(FamilyTag::Cancellation) && task.region_requested)
                        && !self.deadline_reached(t)
                        && quiet
                    {
                        out.push(Pattern::Fail { task: t });
                    }
                }
                TaskPhase::Running => {
                    if active(t) && !self.staged(t) {
                        out.push(Pattern::Exact(Step::Suspend { task: t }));
                    }
                    if task.cancel == CancelPhase::None
                        && !task.requested_alone
                        && !task.cleaned_up
                        && !(has(FamilyTag::Cancellation) && task.region_requested)
                        && !self.deadline_reached(t)
                        && quiet
                    {
                        out.push(Pattern::Exact(Step::Complete { task: t }));
                        out.push(Pattern::Fail { task: t });
                    }
                }
                TaskPhase::Suspended
                    if active(t) && !self.armed(t) && !self.deadline_reached(t) =>
                {
                    out.push(Pattern::Exact(Step::Resume { task: t }));
                }
                _ => {}
            }
        }

        // Cancellation.
        if has(FamilyTag::Cancellation) {
            for t in 0..tasks {
                let task = &self.tasks[t as usize];
                if task.phase.is_terminal() {
                    continue;
                }
                match task.cancel {
                    CancelPhase::None if task.requested_alone => {
                        out.push(Pattern::Exact(Step::CancelRequested {
                            task: t,
                            cause: Cause::Deadline,
                        }));
                    }
                    CancelPhase::None => {
                        if let Some(cause) = self.implied_cause(t) {
                            if self.regions[task.region as usize].phase == RegionPhase::Cancelling {
                                out.push(Pattern::Exact(Step::CancelRequested { task: t, cause }));
                            }
                        }
                    }
                    CancelPhase::Requested(_) => {
                        out.push(Pattern::Exact(Step::CancelAcknowledged { task: t }));
                    }
                    CancelPhase::Acknowledged(cause) => {
                        if !self.staged(t)
                            && !self.holds(t)
                            && !self.armed(t)
                            && !self.holds_channel(t)
                        {
                            out.push(Pattern::Exact(Step::CancelCompleted { task: t, cause }));
                        }
                    }
                    CancelPhase::Done(_) => {}
                }
            }
        }

        // Effect.
        if has(FamilyTag::Effect) {
            let next = ord(self.reservations.len());
            for t in 0..tasks {
                let task = &self.tasks[t as usize];
                if task.phase == TaskPhase::Running && task.cancel == CancelPhase::None {
                    out.push(Pattern::Exact(Step::Reserve {
                        reservation: next,
                        task: t,
                    }));
                }
            }
            for e in 0..next {
                let res = &self.reservations[e as usize];
                if res.phase != EffectPhase::Reserved {
                    continue;
                }
                let running = self.tasks[res.task as usize].phase == TaskPhase::Running;
                if running && active(res.task) {
                    out.push(Pattern::Exact(Step::Commit { reservation: e }));
                    out.push(Pattern::Exact(Step::Abort {
                        reservation: e,
                        reason: AbortReason::Explicit,
                    }));
                }
                if self.may_clean_up(res.task) {
                    out.push(Pattern::Exact(Step::Abort {
                        reservation: e,
                        reason: AbortReason::Cancel,
                    }));
                }
            }
        }

        // Obligation.
        if has(FamilyTag::Obligation) {
            let next = ord(self.obligations.len());
            for t in 0..tasks {
                let task = &self.tasks[t as usize];
                if self.acting(t) && open(task.region) {
                    out.push(Pattern::Opened {
                        obligation: next,
                        holder: t,
                        region: task.region,
                    });
                }
            }
            for o in 0..next {
                let ob = &self.obligations[o as usize];
                let holder = &self.tasks[ob.holder as usize];
                match ob.phase {
                    ObligationPhase::Open if holder.phase.is_terminal() => {
                        out.push(Pattern::Exact(Step::Leaked { obligation: o }));
                    }
                    ObligationPhase::Open => {
                        if holder.phase == TaskPhase::Running {
                            out.push(Pattern::Exact(Step::Leaked { obligation: o }));
                        }
                        let acting = self.acting(ob.holder);
                        if acting {
                            out.push(Pattern::Exact(Step::Discharged {
                                obligation: o,
                                committed: true,
                            }));
                        }
                        if acting || self.may_clean_up(ob.holder) {
                            out.push(Pattern::Exact(Step::Discharged {
                                obligation: o,
                                committed: false,
                            }));
                        }
                        if acting {
                            for to in 0..tasks {
                                let target = &self.tasks[to as usize];
                                if to != ob.holder
                                    && self.acting(to)
                                    && self.regions[target.region as usize].phase
                                        != RegionPhase::Finalized
                                {
                                    out.push(Pattern::Exact(Step::Transferred {
                                        obligation: o,
                                        holder: to,
                                        region: target.region,
                                    }));
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            for r in 0..regions {
                let g = &self.regions[r as usize];
                if g.phase == RegionPhase::Finalized && !g.settled {
                    let (o, l, f) = self.balance(r);
                    if o.is_empty() && l.is_empty() {
                        out.push(Pattern::Exact(Step::Settled {
                            region: r,
                            open: o,
                            leaked: l,
                            fenced: f,
                        }));
                    }
                }
            }
        }

        // Time.
        if has(FamilyTag::Time) {
            if let Some(t) = self.last_spawn {
                let task = &self.tasks[t as usize];
                if task.phase == TaskPhase::Created && task.deadline.is_none() {
                    out.push(Pattern::DeadlineSet {
                        task: t,
                        after: self.now,
                    });
                }
            }
            let late = self.late().is_some();
            if !late {
                out.push(Pattern::Advanced { from: self.now });
                let next = ord(self.timers.len());
                for t in 0..tasks {
                    let task = &self.tasks[t as usize];
                    if task.phase == TaskPhase::Running
                        && task.cancel == CancelPhase::None
                        && !task.cleaned_up
                        && !self.armed(t)
                    {
                        out.push(Pattern::Scheduled {
                            timer: next,
                            task: t,
                            at: self.now,
                        });
                    }
                }
            }
            for k in 0..ord(self.timers.len()) {
                let timer = &self.timers[k as usize];
                if timer.phase != TimerPhase::Armed {
                    continue;
                }
                if timer.deadline <= self.now {
                    out.push(Pattern::Exact(Step::Fired {
                        timer: k,
                        at: self.now,
                    }));
                }
                if self.may_clean_up(timer.task) {
                    out.push(Pattern::Exact(Step::TimerCancelled {
                        timer: k,
                        at: self.now,
                    }));
                }
            }
        }

        // Channel.
        if has(FamilyTag::Channel) {
            self.enabled_channel(&mut out);
        }
        // No task's own work once the clock has reached its deadline.
        out.retain(|pattern| {
            let actor = match pattern {
                Pattern::Exact(step) => self.actor(step),
                Pattern::Fail { task } | Pattern::Scheduled { task, .. } => Some(*task),
                Pattern::Opened { holder, .. } => Some(*holder),
                _ => None,
            };
            actor.is_none_or(|t| !self.deadline_reached(t))
        });
        // No step of a task that has not observed its requested cancellation.
        out.retain(|pattern| {
            let task = match pattern {
                Pattern::Exact(step) => return self.before_observation(step).is_none(),
                Pattern::Fail { task } | Pattern::Scheduled { task, .. } => *task,
                Pattern::Opened { holder, .. } => *holder,
                Pattern::ChannelOpened { receiver, .. } => *receiver,
                _ => return true,
            };
            !self.unobserved(task)
        });
        // While a task's own cancellation is open, only that cancellation's own steps
        // are enabled (RFC 0026 correction 53 item 6; cr-3pu5cu). No free pattern is one.
        if let Some(t) = self.open_alone() {
            out.retain(|pattern| match pattern {
                Pattern::Exact(step) => self.inside_own_cancel(t, step),
                _ => false,
            });
        }
        out
    }

    fn enabled_channel(&self, out: &mut Vec<Pattern>) {
        let tasks = ord(self.tasks.len());
        let next = self.next_message;
        for t in 0..tasks {
            if self.acting(t) {
                out.push(Pattern::ChannelOpened {
                    channel: ord(self.channels.len()),
                    receiver: t,
                });
            }
        }
        for c in 0..ord(self.channels.len()) {
            let ch = &self.channels[c as usize];
            let room = ch.queue.len() < ch.capacity as usize;
            let fresh_senders: Vec<u32> = (0..tasks)
                .filter(|s| ch.senders_open && self.acting(*s) && self.sender_free(*s))
                .collect();
            let blocked: Vec<(u64, u32)> = self
                .blocked
                .iter()
                .filter(|(_, (cc, _))| *cc == c)
                .map(|(m, (_, s))| (*m, *s))
                .collect();
            if ch.present {
                for s in &fresh_senders {
                    if room && self.permit_ok(*s) {
                        out.push(Pattern::Exact(Step::Sent {
                            channel: c,
                            message: next,
                            sender: *s,
                        }));
                    }
                    if !room {
                        out.push(Pattern::Exact(Step::SendBlocked {
                            channel: c,
                            message: next,
                            sender: *s,
                        }));
                    }
                }
                for (m, s) in &blocked {
                    if room && self.acting(*s) && self.permit_ok(*s) {
                        out.push(Pattern::Exact(Step::Sent {
                            channel: c,
                            message: *m,
                            sender: *s,
                        }));
                    }
                }
                if self.receiving(ch.receiver) {
                    if let Some(m) = ch.queue.front() {
                        out.push(Pattern::Exact(Step::Received {
                            channel: c,
                            message: *m,
                        }));
                    } else if self.can_still_receive(c) {
                        if !ch.recv_blocked {
                            out.push(Pattern::Exact(Step::RecvBlocked { channel: c }));
                        }
                    } else {
                        out.push(Pattern::Exact(Step::RecvClosed { channel: c }));
                    }
                }
                if !ch.recv_blocked
                    && (self.receiving(ch.receiver) || self.may_clean_up(ch.receiver))
                {
                    let mut discarded: Vec<u64> = ch.queue.iter().copied().collect();
                    discarded.sort_unstable();
                    out.push(Pattern::Exact(Step::ReceiverGone {
                        channel: c,
                        discarded,
                    }));
                }
            } else {
                for s in &fresh_senders {
                    out.push(Pattern::Exact(Step::SendClosed {
                        channel: c,
                        message: next,
                        sender: *s,
                    }));
                }
                for (m, s) in &blocked {
                    if self.acting(*s) {
                        out.push(Pattern::Exact(Step::SendClosed {
                            channel: c,
                            message: *m,
                            sender: *s,
                        }));
                    }
                }
            }
            for (m, s) in &blocked {
                if self.may_clean_up(*s) {
                    out.push(Pattern::Exact(Step::SendAbandoned {
                        channel: c,
                        message: *m,
                        sender: *s,
                    }));
                }
            }
            if ch.recv_blocked && self.may_clean_up(ch.receiver) {
                out.push(Pattern::Exact(Step::RecvAbandoned { channel: c }));
            }
            if ch.senders_open {
                out.push(Pattern::Exact(Step::SendersClosed { channel: c }));
            }
        }
    }
}

/// Run the model over a trace of steps.
#[must_use]
pub fn judge_steps(alphabet: &Alphabet, steps: &[Step]) -> Verdict {
    let mut model = Model::new(alphabet.clone());
    for (at, step) in steps.iter().enumerate() {
        if let Err(fault) = model.step(step) {
            return Verdict::Rejected { at, fault };
        }
    }
    match model.finish() {
        Ok(()) => Verdict::Accepted { steps: steps.len() },
        Err(fault) => Verdict::Rejected {
            at: steps.len(),
            fault,
        },
    }
}

/// Read canonical journal bytes and run the model over them.
#[must_use]
pub fn judge(alphabet: &Alphabet, bytes: &[u8]) -> Verdict {
    match read(bytes) {
        Ok(steps) => judge_steps(alphabet, &steps),
        Err(fault) => Verdict::Malformed(fault),
    }
}
