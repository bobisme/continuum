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
//!   time). Where the adapter's family documentation fixes an observation the normative
//!   text leaves open (which phase of a cancellation the substrate lets a run see), the
//!   rule says so.
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
/// The encoding version this reader accepts.
const VERSION: u32 = 1;

/// The five families the substrate binding observes, with their wire tags
/// (`src/family.rs`, "The six families"). Tag 6 (channel) has no binding and no rules
/// here, so a journal that carries it is [`Verdict::Unsupported`].
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
}

impl FamilyTag {
    const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::Lifecycle),
            2 => Some(Self::Effect),
            3 => Some(Self::Cancellation),
            4 => Some(Self::Obligation),
            5 => Some(Self::Time),
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
    /// A normal close requested on a region's subtree.
    Close { region: u32 },
    /// Cancellation requested on a region's subtree.
    Cancel { region: u32 },
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
    },
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
            | Self::Close { .. }
            | Self::Cancel { .. }
            | Self::Drain { .. }
            | Self::Finalize { .. } => FamilyTag::Lifecycle,
            Self::CancelRequested { .. }
            | Self::CancelAcknowledged { .. }
            | Self::CancelCompleted { .. } => FamilyTag::Cancellation,
            Self::Reserve { .. } | Self::Commit { .. } | Self::Abort { .. } => FamilyTag::Effect,
            Self::Opened { .. }
            | Self::Discharged { .. }
            | Self::Transferred { .. }
            | Self::Leaked { .. }
            | Self::Settled { .. } => FamilyTag::Obligation,
            Self::Scheduled { .. }
            | Self::Advanced { .. }
            | Self::Fired { .. }
            | Self::TimerCancelled { .. } => FamilyTag::Time,
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

/// One decoded event: a step of a covered family, or an event of a family the model
/// does not cover.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decoded {
    /// A step.
    Step(Step),
    /// An event of a family with no rules here, by wire tag.
    Uncovered { tag: u8 },
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
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
    fn cause(&mut self) -> Result<Cause, WireFault> {
        Ok(match self.tag("cancel cause", 2)? {
            1 => Cause::User,
            _ => Cause::ParentCancelled,
        })
    }
}

/// Read a canonical journal: `MAGIC u32(version) u64(count) event*`, each event
/// `u64(seq) u8(family) u32(len) payload[len]`, big-endian, nothing after the last.
///
/// # Errors
///
/// The first [`WireFault`].
pub fn read(bytes: &[u8]) -> Result<Vec<Decoded>, WireFault> {
    let mut input = Reader { bytes, at: 0 };
    if input.take(MAGIC.len())? != MAGIC || input.u32()? != VERSION {
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
                };
                let step = payload_step(tag, &mut inner)?;
                if inner.at != payload.len() {
                    return Err(WireFault::PayloadLength { event: expected });
                }
                Decoded::Step(step)
            }
            None if family == 6 => Decoded::Uncovered { tag: family },
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
        // src/family/lifecycle.rs: tags 1..=7.
        FamilyTag::Lifecycle => match r.tag("lifecycle event", 7)? {
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
                match r.tag("task step", 5)? {
                    1 => Step::Begin { task },
                    2 => Step::Suspend { task },
                    3 => Step::Resume { task },
                    4 => Step::Complete { task },
                    _ => Step::Fail {
                        task,
                        reason: r.token()?,
                    },
                }
            }
            4 => Step::Close { region: r.u32()? },
            5 => Step::Cancel { region: r.u32()? },
            6 => Step::Drain {
                region: r.u32()?,
                cancelled: r.set()?,
            },
            _ => Step::Finalize { region: r.u32()? },
        },
        // src/family/effect.rs: tags 1..=3, abort reasons 1..=2.
        FamilyTag::Effect => {
            let tag = r.tag("effect event", 3)?;
            let reservation = r.u32()?;
            match tag {
                1 => Step::Reserve {
                    reservation,
                    task: r.u32()?,
                },
                2 => Step::Commit { reservation },
                _ => Step::Abort {
                    reservation,
                    reason: match r.tag("abort reason", 2)? {
                        1 => AbortReason::Cancel,
                        _ => AbortReason::Explicit,
                    },
                },
            }
        }
        // src/family/cancellation.rs: tags 1..=3, causes 1..=2.
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
        // src/family/obligation.rs: tags 1..=5, kinds 1..=6, discharge 1..=2.
        FamilyTag::Obligation => match r.tag("obligation event", 5)? {
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
            _ => Step::Settled {
                region: r.u32()?,
                open: r.set()?,
                leaked: r.set()?,
            },
        },
        // src/family/time.rs: tags 1..=4.
        FamilyTag::Time => match r.tag("time event", 4)? {
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
            _ => Step::TimerCancelled {
                timer: r.u32()?,
                at: r.u64()?,
            },
        },
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
}

impl TaskPhase {
    const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
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
}

/// docs/02 §7 effect protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EffectPhase {
    Reserved,
    Committed,
    Aborted,
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
}

#[derive(Debug, Clone)]
struct Obligation {
    holder: u32,
    region: u32,
    phase: ObligationPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TimerPhase {
    Armed,
    Fired,
    Cancelled,
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
    /// A second reservation while one is staged.
    SecondReservation(u32),
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
    /// End of trace: a finalized region never settled.
    Unsettled(u32),
    /// End of trace: an obligation leaked by a holder that never ended.
    UnendedLeak(u32),
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
    /// The event at `at` belongs to a family this model has no rules for.
    Unsupported { at: usize, tag: u8 },
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
            }],
            tasks: Vec::new(),
            reservations: Vec::new(),
            obligations: Vec::new(),
            timers: Vec::new(),
            now: 0,
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
            matches!(
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
            self.regions
                .get(task.region as usize)
                .is_some_and(|g| g.phase == RegionPhase::Cancelling)
        }
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
    fn balance(&self, r: u32) -> (Vec<u32>, Vec<u32>) {
        let pick = |phase| {
            (0..ord(self.obligations.len()))
                .filter(|o| {
                    let ob = &self.obligations[*o as usize];
                    ob.region == r && ob.phase == phase
                })
                .collect()
        };
        (pick(ObligationPhase::Open), pick(ObligationPhase::Leaked))
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
                if t.cancel != CancelPhase::None {
                    return Err(Fault::CancelPhase(*task));
                }
                if self.staged(*task) {
                    return Err(Fault::HalfEffect { task: *task });
                }
                if self.armed(*task) {
                    return Err(Fault::TimerOutlivesSleep { task: *task });
                }
            }
            Step::Close { region } => {
                if self.region(*region)?.phase != RegionPhase::Open {
                    return Err(Fault::RegionPhase(*region));
                }
            }
            // A cancel request may upgrade a close; it may not revisit a cancelled or
            // finalized region.
            Step::Cancel { region } => {
                let g = self.region(*region)?;
                if !matches!(g.phase, RegionPhase::Open | RegionPhase::Closing) || g.drained {
                    return Err(Fault::RegionPhase(*region));
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
            }
            // docs/02 §7 effect protocol `Idle → Reserved(token)`, by a running task
            // that is not cancelling, one staged effect at a time.
            Step::Reserve { reservation, task } => {
                fresh(self.reservations.len(), *reservation)?;
                let t = self.task(*task)?;
                if t.phase != TaskPhase::Running {
                    return Err(Fault::TaskPhase(*task));
                }
                if t.cancel != CancelPhase::None {
                    return Err(Fault::CancelPhase(*task));
                }
                if self.staged(*task) {
                    return Err(Fault::SecondReservation(*task));
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
            Step::Settled {
                region,
                open,
                leaked,
            } => {
                let g = self.region(*region)?;
                if g.phase != RegionPhase::Finalized || g.settled {
                    return Err(Fault::Settle(*region));
                }
                let (model_open, model_leaked) = self.balance(*region);
                if &model_open != open || &model_leaked != leaked {
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
                if t.phase != TaskPhase::Running || t.cancel != CancelPhase::None {
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
            }),
            Step::Spawn { region, .. } => self.tasks.push(Task {
                region: *region,
                phase: TaskPhase::Created,
                cancel: CancelPhase::None,
            }),
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
            Step::Abort { reservation, .. } => {
                self.reservations[*reservation as usize].phase = EffectPhase::Aborted;
            }
            Step::Opened { holder, region, .. } => self.obligations.push(Obligation {
                holder: *holder,
                region: *region,
                phase: ObligationPhase::Open,
            }),
            Step::Discharged { obligation, .. } => {
                self.obligations[*obligation as usize].phase = ObligationPhase::Discharged;
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
                self.timers[*timer as usize].phase = TimerPhase::Cancelled;
            }
        }
    }

    /// The end conditions of an accepted trace.
    ///
    /// # Errors
    ///
    /// The first [`Fault`] among: a requested cancellation that never drained, a
    /// terminated task with a reserved effect or an armed timer, an overdue timer, a
    /// leak whose holder never ended, and a finalized region that never settled its
    /// obligations.
    pub fn finish(&self) -> Result<(), Fault> {
        for t in 0..ord(self.tasks.len()) {
            let task = &self.tasks[t as usize];
            if !task.phase.is_terminal() && task.cancel != CancelPhase::None {
                return Err(Fault::UndrainedCancellation(t));
            }
            if task.phase.is_terminal() && self.staged(t) {
                return Err(Fault::HalfEffect { task: t });
            }
            if task.phase.is_terminal() && self.armed(t) {
                return Err(Fault::TimerOutlivesSleep { task: t });
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
            if matches!(g.phase, RegionPhase::Open | RegionPhase::Closing) && !g.drained {
                out.push(Pattern::Exact(Step::Cancel { region: r }));
            }
            if matches!(g.phase, RegionPhase::Closing | RegionPhase::Cancelling) && !g.drained {
                if let Ok(cancelled) = self.drain_outcome(r) {
                    let ready = cancelled.iter().all(|t| {
                        (!has(FamilyTag::Cancellation)
                            || matches!(self.tasks[*t as usize].cancel, CancelPhase::Done(_)))
                            && !self.staged(*t)
                            && !self.armed(*t)
                            && !self.holds(*t)
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
            let quiet = !self.staged(t) && !self.armed(t);
            match task.phase {
                TaskPhase::Created => {
                    if active(t) {
                        out.push(Pattern::Exact(Step::Begin { task: t }));
                    }
                    if task.cancel == CancelPhase::None && quiet {
                        out.push(Pattern::Fail { task: t });
                    }
                }
                TaskPhase::Running => {
                    if active(t) && !self.staged(t) {
                        out.push(Pattern::Exact(Step::Suspend { task: t }));
                    }
                    if task.cancel == CancelPhase::None && quiet {
                        out.push(Pattern::Exact(Step::Complete { task: t }));
                        out.push(Pattern::Fail { task: t });
                    }
                }
                TaskPhase::Suspended if active(t) && !self.armed(t) => {
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
                        if !self.staged(t) && !self.holds(t) && !self.armed(t) {
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
                if task.phase == TaskPhase::Running
                    && task.cancel == CancelPhase::None
                    && !self.staged(t)
                {
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
                    let (o, l) = self.balance(r);
                    if o.is_empty() && l.is_empty() {
                        out.push(Pattern::Exact(Step::Settled {
                            region: r,
                            open: o,
                            leaked: l,
                        }));
                    }
                }
            }
        }

        // Time.
        if has(FamilyTag::Time) {
            let late = self.late().is_some();
            if !late {
                out.push(Pattern::Advanced { from: self.now });
                let next = ord(self.timers.len());
                for t in 0..tasks {
                    let task = &self.tasks[t as usize];
                    if task.phase == TaskPhase::Running
                        && task.cancel == CancelPhase::None
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
        out
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
    let decoded = match read(bytes) {
        Ok(decoded) => decoded,
        Err(fault) => return Verdict::Malformed(fault),
    };
    let mut steps = Vec::with_capacity(decoded.len());
    for (at, event) in decoded.into_iter().enumerate() {
        match event {
            Decoded::Step(step) => steps.push(step),
            Decoded::Uncovered { tag } => return Verdict::Unsupported { at, tag },
        }
    }
    judge_steps(alphabet, &steps)
}
