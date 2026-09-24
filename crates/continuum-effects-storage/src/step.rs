//! Nodes, records, sync tickets, the run configuration, the choice-log step, and the
//! journal event.

use core::fmt;

use crate::profile::Semantic;
use crate::refusal::ConfigRefusal;

/// The largest node count a configuration may declare. A [`NodeSet`] is a 64-bit set.
pub const MAX_NODES: u8 = 64;

/// The largest crash budget a configuration may declare.
pub const MAX_CRASHES_CAP: u32 = 1 << 16;

/// The largest pending-ticket bound a configuration may declare.
pub const MAX_PENDING_CAP: u32 = 1 << 16;

/// The largest retained-bytes budget a configuration may declare: 256 MiB.
pub const MAX_RETAINED_CAP: u64 = 1 << 28;

/// The largest choice log [`crate::run`] accepts. It checks the length before it runs
/// any step, and [`crate::Storage::apply`] refuses a step past it.
pub const MAX_STEPS: usize = 1 << 20;

/// [`MAX_STEPS`] as the configuration stores it.
#[allow(clippy::cast_possible_truncation, clippy::as_conversions)]
const MAX_STEPS_U32: u32 = MAX_STEPS as u32;

/// A node address, `0..nodes`. One node has one append log, and at most one
/// incarnation of the node runs at a time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(pub u8);

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "n{}", self.0)
    }
}

/// A set of nodes, bit `i` for node `i`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeSet(pub u64);

impl NodeSet {
    /// Whether `node` is in the set.
    #[must_use]
    pub const fn contains(self, node: NodeId) -> bool {
        node.0 < MAX_NODES && (self.0 >> node.0) & 1 == 1
    }

    /// The set of all `n` nodes of a configuration.
    #[must_use]
    pub const fn all(n: u8) -> Self {
        if n >= MAX_NODES {
            Self(u64::MAX)
        } else {
            Self((1_u64 << n) - 1)
        }
    }

    /// `self` with `node` added. A node of 64 or more has no bit and is ignored; the
    /// callers here only pass known nodes.
    #[must_use]
    pub const fn with(self, node: NodeId) -> Self {
        if node.0 < MAX_NODES {
            Self(self.0 | (1_u64 << node.0))
        } else {
            self
        }
    }

    /// `self` with `node` removed.
    #[must_use]
    pub const fn without(self, node: NodeId) -> Self {
        if node.0 < MAX_NODES {
            Self(self.0 & !(1_u64 << node.0))
        } else {
            self
        }
    }
}

/// An incarnation epoch, by the process pack's rule (`process/crash-restart-v0`,
/// `restart-new-epoch`): a node's first incarnation is epoch 0, and each `Restart`
/// starts one whose epoch is one more than the last. An epoch is never reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Epoch(pub u32);

impl fmt::Display for Epoch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

/// A record's value: an opaque, fixed-width word. The program encodes what it stores in
/// it (for the register, an epoch and a value). The record is the atomic unit: it is
/// intact or torn as a whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Value(pub u32);

/// One log position as a reader sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Entry {
    /// A whole record: its checksum holds.
    Intact(Value),
    /// A record that a crash left part written: its checksum fails. A reader never sees
    /// it as some other intact value.
    Torn,
}

/// A sync ticket: the ordinal of the `Sync` that opened it, from 0. The ordinal is the
/// idempotence key: a ticket resolves, `Acked` or `Fenced`, at most once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TicketId(pub u32);

impl fmt::Display for TicketId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "s{}", self.0)
    }
}

/// A validated run configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StorageConfig {
    nodes: u8,
    max_crashes: u32,
    restart: bool,
    suffix_loss: bool,
    torn: bool,
    max_pending: u32,
    max_steps: u32,
    max_retained_bytes: u64,
}

impl StorageConfig {
    /// A configuration, validated against the caps.
    ///
    /// `max_crashes` is the crash budget and `restart` declares recovery, exactly as in
    /// the process pack's configuration; a composed run gives both packs the same two
    /// values. `suffix_loss` declares the `volatile-suffix-loss` fault: without it a
    /// crash keeps every submitted record. `torn` declares the `torn-last-record` fault;
    /// a tear needs a lost record, so it needs `suffix_loss`.
    /// `max_pending` bounds the unresolved sync tickets.
    ///
    /// `max_retained_bytes` bounds the journal's whole canonical encoding, header
    /// included ([`crate::lab::Journal::encode`]). Every event has a fixed size, so it
    /// also bounds the event count, and so the log lengths and the ticket table: memory
    /// is O(budget) plus O([`MAX_STEPS`]).
    ///
    /// # Errors
    ///
    /// [`ConfigRefusal`] when `nodes` is outside `1..=MAX_NODES`, `max_crashes` above
    /// [`MAX_CRASHES_CAP`], `max_pending` outside `1..=MAX_PENDING_CAP`, or
    /// `max_retained_bytes` outside `JOURNAL_HEADER_BYTES..=MAX_RETAINED_CAP`, or
    /// `torn` without `suffix_loss`.
    #[allow(clippy::fn_params_excessive_bools)]
    pub const fn new(
        nodes: u8,
        max_crashes: u32,
        restart: bool,
        suffix_loss: bool,
        torn: bool,
        max_pending: u32,
        max_retained_bytes: u64,
    ) -> Result<Self, ConfigRefusal> {
        if nodes == 0 || nodes > MAX_NODES {
            return Err(ConfigRefusal::NodesOutOfRange(nodes));
        }
        if max_crashes > MAX_CRASHES_CAP {
            return Err(ConfigRefusal::CrashesOutOfRange(max_crashes));
        }
        if max_pending == 0 || max_pending > MAX_PENDING_CAP {
            return Err(ConfigRefusal::PendingOutOfRange(max_pending));
        }
        if torn && !suffix_loss {
            return Err(ConfigRefusal::TornWithoutLoss);
        }
        if max_retained_bytes < crate::lab::JOURNAL_HEADER_BYTES
            || max_retained_bytes > MAX_RETAINED_CAP
        {
            return Err(ConfigRefusal::RetainedOutOfRange(max_retained_bytes));
        }
        Ok(Self {
            nodes,
            max_crashes,
            restart,
            suffix_loss,
            torn,
            max_pending,
            max_steps: MAX_STEPS_U32,
            max_retained_bytes,
        })
    }

    /// The same configuration with a smaller journal-length bound: at most `max_steps`
    /// accepted steps, in `1..=MAX_STEPS`. [`Self::new`] sets [`MAX_STEPS`]. A small
    /// bound makes the step bound reachable in an exhaustive exploration, so its
    /// boundary is explored rather than assumed.
    ///
    /// # Errors
    ///
    /// [`ConfigRefusal::StepsOutOfRange`] outside `1..=MAX_STEPS`.
    pub const fn with_max_steps(self, max_steps: u32) -> Result<Self, ConfigRefusal> {
        if max_steps == 0 || max_steps > MAX_STEPS_U32 {
            return Err(ConfigRefusal::StepsOutOfRange(max_steps));
        }
        Ok(Self { max_steps, ..self })
    }

    /// `replicated_register.scenario.toml`: `[faults] max_crashes = 2`, restart
    /// declared (the Intent Contract enables `crash` and `recovery`), and `[storage]
    /// allow_volatile_suffix_loss = true` and `allow_torn_last_record = true`. The
    /// scenario fixes no pending bound or retained-bytes budget, so the caller supplies
    /// the node count and both bounds.
    ///
    /// # Errors
    ///
    /// As [`Self::new`].
    pub const fn replicated_register_scenario(
        nodes: u8,
        max_pending: u32,
        max_retained_bytes: u64,
    ) -> Result<Self, ConfigRefusal> {
        Self::new(nodes, 2, true, true, true, max_pending, max_retained_bytes)
    }

    /// The node count.
    #[must_use]
    pub const fn nodes(&self) -> u8 {
        self.nodes
    }

    /// The crash budget.
    #[must_use]
    pub const fn max_crashes(&self) -> u32 {
        self.max_crashes
    }

    /// Whether `Restart` is declared.
    #[must_use]
    pub const fn restart(&self) -> bool {
        self.restart
    }

    /// Whether the `volatile-suffix-loss` fault is declared.
    #[must_use]
    pub const fn suffix_loss(&self) -> bool {
        self.suffix_loss
    }

    /// Whether the `torn-last-record` fault is declared.
    #[must_use]
    pub const fn torn(&self) -> bool {
        self.torn
    }

    /// The pending-ticket bound.
    #[must_use]
    pub const fn max_pending(&self) -> u32 {
        self.max_pending
    }

    /// The journal-length bound: the most steps a run accepts.
    #[must_use]
    pub const fn max_steps(&self) -> u32 {
        self.max_steps
    }

    /// The retained-bytes budget: the largest canonical journal encoding a run may
    /// reach, header included.
    #[must_use]
    pub const fn max_retained_bytes(&self) -> u64 {
        self.max_retained_bytes
    }
}

/// One choice-log entry.
///
/// `Submit`, `Sync` and `Truncate` are the program's steps. `Persist` and `Ack` are the
/// scheduler's choice of when the device finishes a flush and when its completion
/// arrives. `Delay`, `Crash` and `Restart` are the adversary's faults (the Intent
/// Contract's `crash_schedule` site). The last three variants ask for storage
/// semantics this profile does **not** model; they exist so that a caller who asks for
/// them receives a typed [`crate::Refusal::Unsupported`] instead of a silent
/// approximation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Step {
    /// The node's running incarnation appends one record to its log. The record is
    /// submitted: visible to the incarnation at once, and volatile.
    Submit {
        /// The node.
        node: NodeId,
        /// The record.
        value: Value,
    },
    /// The node's running incarnation asks for every record it has submitted so far to
    /// become stable, and waits for the completion: open a sync ticket over the log's
    /// current length, bound to that incarnation.
    Sync(NodeId),
    /// The device finishes the ticket's flush: every record the ticket covers is now
    /// stable.
    Persist(TicketId),
    /// The ticket's completion arrives. It is delivered if the incarnation that began
    /// it still runs, and fenced otherwise.
    Ack(TicketId),
    /// Hold the ticket back for one more step: an explicit, journalled stutter with no
    /// duration, the adversary's `delay`.
    Delay(TicketId),
    /// Fail-stop crash of the node's incarnation. Every stable record survives intact.
    /// Of the volatile suffix, the first `keep` records survive; the rest are lost. If
    /// `torn`, the first lost record survives torn instead.
    Crash {
        /// The node.
        node: NodeId,
        /// How many volatile records reached the medium, in submit order.
        keep: u32,
        /// Whether the first record past them survives torn.
        torn: bool,
    },
    /// Start a new incarnation of a crashed node, with a fresh epoch.
    Restart(NodeId),
    /// Recovery drops the log's torn tail.
    Truncate(NodeId),
    /// Unsupported: a stored record's bits change on the medium.
    Corrupt {
        /// The node.
        node: NodeId,
        /// The log position.
        index: u32,
    },
    /// Unsupported: a crash keeps a later volatile record and loses an earlier one.
    Reorder(NodeId),
    /// Unsupported: the device reports a flush it did not perform.
    FalseFlush(TicketId),
}

/// Who makes a step's choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Chooser {
    /// The program under test.
    Program,
    /// The scheduler: when a flush finishes and when its completion arrives.
    Scheduler,
    /// The fault adversary.
    Adversary,
}

impl Step {
    /// Who chooses this step.
    #[must_use]
    pub const fn chooser(&self) -> Chooser {
        match self {
            Self::Submit { .. } | Self::Sync(_) | Self::Truncate(_) => Chooser::Program,
            Self::Persist(_) | Self::Ack(_) => Chooser::Scheduler,
            Self::Delay(_)
            | Self::Crash { .. }
            | Self::Restart(_)
            | Self::Corrupt { .. }
            | Self::Reorder(_)
            | Self::FalseFlush(_) => Chooser::Adversary,
        }
    }

    /// The unsupported semantic this step asks for, if the step is one the profile
    /// does not model at all.
    #[must_use]
    pub const fn unsupported_semantic(&self) -> Option<Semantic> {
        match self {
            Self::Corrupt { .. } => Some(Semantic::SectorCorruption),
            Self::Reorder(_) => Some(Semantic::WriteReordering),
            Self::FalseFlush(_) => Some(Semantic::FlushDishonesty),
            Self::Submit { .. }
            | Self::Sync(_)
            | Self::Persist(_)
            | Self::Ack(_)
            | Self::Delay(_)
            | Self::Crash { .. }
            | Self::Restart(_)
            | Self::Truncate(_) => None,
        }
    }
}

/// One journalled event. Each accepted step yields exactly one event, and
/// [`Event::step`] gives the step back, so a journal is its own choice log.
///
/// A ticket event carries the ticket, its node, the epoch of the incarnation that
/// began it, and `upto`: the log length its sync covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Event {
    /// A record appended at `index`, volatile.
    Submitted {
        /// The node.
        node: NodeId,
        /// The incarnation that submitted it.
        epoch: Epoch,
        /// Its log position.
        index: u32,
        /// The record.
        value: Value,
    },
    /// A sync ticket opened over the first `upto` records.
    SyncBegun {
        /// The ticket.
        ticket: TicketId,
        /// The node.
        node: NodeId,
        /// The incarnation that began it.
        epoch: Epoch,
        /// The log length the sync covers.
        upto: u32,
    },
    /// The flush finished: the first `upto` records are stable.
    Stable {
        /// The ticket.
        ticket: TicketId,
        /// The node.
        node: NodeId,
        /// The incarnation that began it, still running.
        epoch: Epoch,
        /// The log length now stable at least.
        upto: u32,
    },
    /// The completion reached the incarnation that began it: its records are stable.
    Acked {
        /// The ticket.
        ticket: TicketId,
        /// The node.
        node: NodeId,
        /// The incarnation that began it, still running.
        epoch: Epoch,
        /// The log length the acknowledgement vouches for.
        upto: u32,
    },
    /// The completion arrived after the incarnation that began it crashed. It is
    /// journalled and discarded: no incarnation receives it, and it vouches for
    /// nothing.
    Fenced {
        /// The ticket.
        ticket: TicketId,
        /// The node.
        node: NodeId,
        /// The incarnation that began it, no longer running.
        epoch: Epoch,
        /// The log length its sync covered.
        upto: u32,
    },
    /// The adversary held the ticket back for a step.
    Delayed {
        /// The ticket.
        ticket: TicketId,
        /// The node.
        node: NodeId,
        /// The incarnation that began it.
        epoch: Epoch,
        /// The log length its sync covers.
        upto: u32,
    },
    /// The node's incarnation stopped; its log kept the stable prefix and `keep`
    /// volatile records and lost `lost` records. If `torn`, this crash tore the first
    /// lost record, which is now the log's last entry. A torn tail from an earlier
    /// crash survives unchanged, with `torn` false and nothing lost.
    Crashed {
        /// The node.
        node: NodeId,
        /// The incarnation that stopped.
        epoch: Epoch,
        /// Volatile records that survived intact.
        keep: u32,
        /// Whether this crash tore the first record past them.
        torn: bool,
        /// Volatile records lost; a torn record counts as lost.
        lost: u32,
    },
    /// A new incarnation of the node started.
    Restarted {
        /// The node.
        node: NodeId,
        /// The new incarnation's epoch.
        epoch: Epoch,
    },
    /// Recovery dropped the torn record at `index`.
    Truncated {
        /// The node.
        node: NodeId,
        /// The running incarnation.
        epoch: Epoch,
        /// The torn record's position.
        index: u32,
    },
}

impl Event {
    /// The step that produced this event.
    #[must_use]
    pub const fn step(&self) -> Step {
        match self {
            Self::Submitted { node, value, .. } => Step::Submit {
                node: *node,
                value: *value,
            },
            Self::SyncBegun { node, .. } => Step::Sync(*node),
            Self::Stable { ticket, .. } => Step::Persist(*ticket),
            Self::Acked { ticket, .. } | Self::Fenced { ticket, .. } => Step::Ack(*ticket),
            Self::Delayed { ticket, .. } => Step::Delay(*ticket),
            Self::Crashed {
                node, keep, torn, ..
            } => Step::Crash {
                node: *node,
                keep: *keep,
                torn: *torn,
            },
            Self::Restarted { node, .. } => Step::Restart(*node),
            Self::Truncated { node, .. } => Step::Truncate(*node),
        }
    }
}
