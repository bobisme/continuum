//! Nodes, tickets, the run configuration, the choice-log step, and the journal event.

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
/// any step, and [`crate::Process::apply`] refuses a step past it.
pub const MAX_STEPS: usize = 1 << 20;

/// A node address, `0..nodes`. One node is one process slot: at most one incarnation
/// of it runs at a time.
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

/// An incarnation epoch. A node's first incarnation is epoch 0, and each `Restart`
/// starts one whose epoch is one more than the last. An epoch is never reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Epoch(pub u32);

impl fmt::Display for Epoch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

/// A ticket: the ordinal of the `Begin` that opened it, from 0. A ticket stands for one
/// operation an incarnation started and whose completion it waits for: a storage
/// write, a timer, a reply. The ordinal is the idempotence key: a ticket resolves at
/// most once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TicketId(pub u32);

impl fmt::Display for TicketId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "t{}", self.0)
    }
}

/// A validated run configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProcessConfig {
    nodes: u8,
    max_crashes: u32,
    restart: bool,
    max_pending: u32,
    max_retained_bytes: u64,
}

impl ProcessConfig {
    /// A configuration, validated against the caps.
    ///
    /// `max_crashes` is the crash budget: how many `Crash` steps one run may take. It
    /// is part of the declared fault envelope, like the scenario's `max_crashes`, so a
    /// crash past it is not enabled; `0` declares no crash at all. `restart` declares
    /// the `recovery` fault class: without it a crashed node stays down.
    ///
    /// `max_retained_bytes` bounds the journal's whole canonical encoding, header
    /// included ([`crate::lab::Journal::encode`]). Every event has a fixed size, so it
    /// also bounds the event count. Memory is not the encoding: the ticket table and the
    /// pending set add a per-event constant, so memory is O(budget) plus
    /// O([`MAX_STEPS`]).
    ///
    /// # Errors
    ///
    /// [`ConfigRefusal`] when `nodes` is outside `1..=MAX_NODES`, `max_crashes` above
    /// [`MAX_CRASHES_CAP`], `max_pending` outside `1..=MAX_PENDING_CAP`, or
    /// `max_retained_bytes` outside `JOURNAL_HEADER_BYTES..=MAX_RETAINED_CAP`.
    pub const fn new(
        nodes: u8,
        max_crashes: u32,
        restart: bool,
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
        if max_retained_bytes < crate::lab::JOURNAL_HEADER_BYTES
            || max_retained_bytes > MAX_RETAINED_CAP
        {
            return Err(ConfigRefusal::RetainedOutOfRange(max_retained_bytes));
        }
        Ok(Self {
            nodes,
            max_crashes,
            restart,
            max_pending,
            max_retained_bytes,
        })
    }

    /// `replicated_register.scenario.toml`'s `[faults] max_crashes = 2`, with restart
    /// declared (the replicated-register Intent Contract enables `crash` and
    /// `recovery`). The scenario fixes three replicas but not the client count, and sets
    /// no pending bound or retained-bytes budget, so the caller supplies the node count
    /// and both bounds.
    ///
    /// # Errors
    ///
    /// As [`Self::new`].
    pub const fn replicated_register_scenario(
        nodes: u8,
        max_pending: u32,
        max_retained_bytes: u64,
    ) -> Result<Self, ConfigRefusal> {
        Self::new(nodes, 2, true, max_pending, max_retained_bytes)
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

    /// The pending-ticket bound.
    #[must_use]
    pub const fn max_pending(&self) -> u32 {
        self.max_pending
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
/// `Begin` is the program's step. `Complete` is the scheduler's choice of when an
/// operation's completion arrives. `Delay`, `Crash` and `Restart` are the adversary's
/// faults
/// (the Intent Contract's `crash_schedule` site). The last three variants ask for RFC
/// 0002 process semantics this profile does **not** model; they exist so that a
/// caller who asks for them receives a typed [`crate::Refusal::Unsupported`] instead
/// of a silent approximation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Step {
    /// The node's current incarnation starts an operation and waits for its
    /// completion: open a ticket bound to that incarnation.
    Begin(NodeId),
    /// The ticket's completion arrives. It is delivered if the incarnation that began
    /// it is still running, and fenced otherwise.
    Complete(TicketId),
    /// Hold the ticket's completion back for one more step: an explicit, journalled
    /// stutter with no duration, the adversary's `delay`.
    Delay(TicketId),
    /// Fail-stop crash of the node's current incarnation.
    Crash(NodeId),
    /// Start a new incarnation of a crashed node, with a fresh epoch.
    Restart(NodeId),
    /// Unsupported: cancel the node's incarnation gracefully, running its cleanup.
    CancelGracefully(NodeId),
    /// Unsupported: a task of the node's incarnation panics and unwinds.
    Panic(NodeId),
    /// Unsupported: every node of the set loses power at once.
    PowerLoss(NodeSet),
}

/// Who makes a step's choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Chooser {
    /// The program under test.
    Program,
    /// The scheduler: when a completion arrives.
    Scheduler,
    /// The fault adversary.
    Adversary,
}

impl Step {
    /// Who chooses this step.
    #[must_use]
    pub const fn chooser(&self) -> Chooser {
        match self {
            Self::Begin(_) => Chooser::Program,
            Self::Complete(_) => Chooser::Scheduler,
            Self::Delay(_)
            | Self::Crash(_)
            | Self::Restart(_)
            | Self::CancelGracefully(_)
            | Self::Panic(_)
            | Self::PowerLoss(_) => Chooser::Adversary,
        }
    }

    /// The unsupported semantic this step asks for, if the step is one the profile
    /// does not model at all.
    #[must_use]
    pub const fn unsupported_semantic(&self) -> Option<Semantic> {
        match self {
            Self::CancelGracefully(_) => Some(Semantic::GracefulCancellation),
            Self::Panic(_) => Some(Semantic::Panic),
            Self::PowerLoss(_) => Some(Semantic::PowerLoss),
            Self::Begin(_)
            | Self::Complete(_)
            | Self::Delay(_)
            | Self::Crash(_)
            | Self::Restart(_) => None,
        }
    }
}

/// One journalled event. Each accepted step yields exactly one event, and
/// [`Event::step`] gives the step back, so a journal is its own choice log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Event {
    /// A ticket opened, bound to the node's running incarnation.
    Begun {
        /// The ticket.
        ticket: TicketId,
        /// The node.
        node: NodeId,
        /// The incarnation that began it.
        epoch: Epoch,
    },
    /// The ticket's completion reached the incarnation that began it.
    Completed {
        /// The ticket.
        ticket: TicketId,
        /// The node.
        node: NodeId,
        /// The incarnation that began it, still running.
        epoch: Epoch,
    },
    /// The ticket's completion arrived after the incarnation that began it crashed. It
    /// is journalled and discarded: no incarnation receives it.
    Fenced {
        /// The ticket.
        ticket: TicketId,
        /// The node.
        node: NodeId,
        /// The incarnation that began it, no longer running.
        epoch: Epoch,
    },
    /// The adversary held the ticket's completion back for a step.
    Delayed {
        /// The ticket.
        ticket: TicketId,
        /// The node.
        node: NodeId,
        /// The incarnation that began it.
        epoch: Epoch,
    },
    /// The node's incarnation stopped.
    Crashed {
        /// The node.
        node: NodeId,
        /// The incarnation that stopped.
        epoch: Epoch,
    },
    /// A new incarnation of the node started.
    Restarted {
        /// The node.
        node: NodeId,
        /// The new incarnation's epoch.
        epoch: Epoch,
    },
}

impl Event {
    /// The step that produced this event.
    #[must_use]
    pub const fn step(&self) -> Step {
        match self {
            Self::Begun { node, .. } => Step::Begin(*node),
            Self::Completed { ticket, .. } | Self::Fenced { ticket, .. } => Step::Complete(*ticket),
            Self::Delayed { ticket, .. } => Step::Delay(*ticket),
            Self::Crashed { node, .. } => Step::Crash(*node),
            Self::Restarted { node, .. } => Step::Restart(*node),
        }
    }
}
