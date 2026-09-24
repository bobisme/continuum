//! Nodes, envelopes, the run configuration, the choice-log step, and the journal event.

use alloc::vec::Vec;
use core::fmt;

use crate::profile::Semantic;
use crate::refusal::ConfigRefusal;

/// The largest node count a configuration may declare. A [`NodeSet`] is a 64-bit set.
pub const MAX_NODES: u8 = 64;

/// The largest in-flight bound a configuration may declare.
pub const MAX_IN_FLIGHT_CAP: u32 = 1 << 16;

/// The largest payload bound a configuration may declare, in bytes.
pub const MAX_PAYLOAD_CAP: u32 = 1 << 16;

/// The largest retained-bytes budget a configuration may declare: 256 MiB.
pub const MAX_RETAINED_CAP: u64 = 1 << 28;

/// The largest choice log [`crate::run`] accepts. It checks the length before it runs
/// any step.
pub const MAX_STEPS: usize = 1 << 20;

/// A node address, `0..nodes`.
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
    /// The set of these nodes, or `None` when a node id is 64 or more and so has no
    /// bit (it is refused, never dropped silently).
    #[must_use]
    pub fn of(nodes: &[u8]) -> Option<Self> {
        let mut bits = 0_u64;
        for &node in nodes {
            if node >= MAX_NODES {
                return None;
            }
            bits |= 1_u64 << node;
        }
        Some(Self(bits))
    }

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

    /// Whether every member is below `n`.
    #[must_use]
    pub const fn bits_valid(self, n: u8) -> bool {
        self.0 & !Self::all(n).0 == 0
    }
}

/// An envelope: the ordinal of the `Send` that created it, from 0. Duplicated copies
/// share their envelope's ordinal, which is the idempotence key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EnvelopeId(pub u32);

impl fmt::Display for EnvelopeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "e{}", self.0)
    }
}

/// An opaque payload. The pack never reads it; it only carries it bit-exact.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Payload(pub Vec<u8>);

/// Which faults the configuration declares, `replicated_register.scenario.toml`
/// `[network]`'s `allow_*` switches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FaultSwitches {
    /// `allow_loss`: `Drop` is enabled.
    pub loss: bool,
    /// `allow_duplication`: `Duplicate` is enabled.
    pub duplication: bool,
    /// `allow_reordering`: without it each directed link is FIFO.
    pub reordering: bool,
}

/// A validated run configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NetworkConfig {
    nodes: u8,
    max_in_flight: u32,
    max_payload_bytes: u32,
    faults: FaultSwitches,
    max_partitions: u32,
    max_retained_bytes: u64,
}

impl NetworkConfig {
    /// A configuration, validated against the caps.
    ///
    /// # Errors
    ///
    /// [`ConfigRefusal`] when `nodes` is outside `1..=MAX_NODES`, `max_in_flight`
    /// outside `1..=MAX_IN_FLIGHT_CAP`, `max_payload_bytes` above [`MAX_PAYLOAD_CAP`],
    /// or `max_retained_bytes` outside
    /// `JOURNAL_HEADER_BYTES..=MAX_RETAINED_CAP`.
    ///
    /// `max_retained_bytes` bounds the journal's whole canonical encoding, header
    /// included ([`crate::lab::Journal::encode`]). Every payload the network keeps is
    /// kept once, in its `Sent` event, so payload bytes held are bounded by it, however
    /// many envelopes are delivered in between. Memory is not the encoding: the event
    /// vector and the in-flight maps add a per-event constant, so memory is O(budget)
    /// plus O([`MAX_STEPS`]).
    pub const fn new(
        nodes: u8,
        max_in_flight: u32,
        max_payload_bytes: u32,
        faults: FaultSwitches,
        max_partitions: u32,
        max_retained_bytes: u64,
    ) -> Result<Self, ConfigRefusal> {
        if nodes == 0 || nodes > MAX_NODES {
            return Err(ConfigRefusal::NodesOutOfRange(nodes));
        }
        if max_in_flight == 0 || max_in_flight > MAX_IN_FLIGHT_CAP {
            return Err(ConfigRefusal::InFlightOutOfRange(max_in_flight));
        }
        if max_payload_bytes > MAX_PAYLOAD_CAP {
            return Err(ConfigRefusal::PayloadOutOfRange(max_payload_bytes));
        }
        if max_retained_bytes < crate::lab::JOURNAL_HEADER_BYTES
            || max_retained_bytes > MAX_RETAINED_CAP
        {
            return Err(ConfigRefusal::RetainedOutOfRange(max_retained_bytes));
        }
        Ok(Self {
            nodes,
            max_in_flight,
            max_payload_bytes,
            faults,
            max_partitions,
            max_retained_bytes,
        })
    }

    /// `replicated_register.scenario.toml`'s `[network]` and `[faults]` values:
    /// `max_in_flight = 16`, loss, duplication and reordering allowed, and
    /// `max_partitions = 1`. The scenario fixes three replicas but not the client count,
    /// and sets no payload bound or retained-bytes budget, so the caller supplies the
    /// node count and both bounds.
    ///
    /// # Errors
    ///
    /// As [`Self::new`].
    pub const fn replicated_register_scenario(
        nodes: u8,
        max_payload_bytes: u32,
        max_retained_bytes: u64,
    ) -> Result<Self, ConfigRefusal> {
        Self::new(
            nodes,
            16,
            max_payload_bytes,
            FaultSwitches {
                loss: true,
                duplication: true,
                reordering: true,
            },
            1,
            max_retained_bytes,
        )
    }

    /// The node count.
    #[must_use]
    pub const fn nodes(&self) -> u8 {
        self.nodes
    }

    /// The in-flight bound, counted in copies.
    #[must_use]
    pub const fn max_in_flight(&self) -> u32 {
        self.max_in_flight
    }

    /// The payload bound in bytes.
    #[must_use]
    pub const fn max_payload_bytes(&self) -> u32 {
        self.max_payload_bytes
    }

    /// The declared fault switches.
    #[must_use]
    pub const fn faults(&self) -> FaultSwitches {
        self.faults
    }

    /// The partition budget: how many `Partition` steps one run may take.
    #[must_use]
    pub const fn max_partitions(&self) -> u32 {
        self.max_partitions
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
/// `Send` is the program's step. `Deliver` is the scheduler's choice. `Drop`,
/// `Duplicate`, `Delay`, `Partition` and `Heal` are the adversary's faults. The last
/// six variants ask for RFC 0002 or docs/17 §7 semantics this profile does **not** model; they
/// exist so that a caller who asks for them receives a typed
/// [`crate::Refusal::Unsupported`] instead of a silent approximation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Step {
    /// The program sends `payload` from `src` to `dst`.
    Send {
        /// Sender.
        src: NodeId,
        /// Receiver.
        dst: NodeId,
        /// The payload, carried bit-exact.
        payload: Payload,
    },
    /// Deliver one copy of the envelope to its receiver.
    Deliver(EnvelopeId),
    /// Lose one copy of the envelope.
    Drop(EnvelopeId),
    /// Add one copy of the envelope.
    Duplicate(EnvelopeId),
    /// Hold the envelope in flight for one more step (a journalled stutter).
    Delay(EnvelopeId),
    /// Cut every link between `side` and the other nodes, both ways.
    Partition(NodeSet),
    /// Restore every link.
    Heal,
    /// Unsupported: cut only the links from `from` to `to`.
    OneWayPartition {
        /// Senders whose envelopes would be blocked.
        from: NodeSet,
        /// Receivers they would be blocked towards.
        to: NodeSet,
    },
    /// Unsupported: alter an envelope's payload in flight.
    Corrupt(EnvelopeId),
    /// Unsupported: deliver an envelope nobody sent.
    Forge {
        /// Claimed sender.
        src: NodeId,
        /// Receiver.
        dst: NodeId,
        /// The forged payload.
        payload: Payload,
    },
    /// Unsupported: reset the connection between two nodes.
    ConnectionReset(NodeId, NodeId),
    /// Unsupported: crash an endpoint as a network event.
    CrashEndpoint(NodeId),
    /// Unsupported: withdraw an envelope already in flight.
    Recall(EnvelopeId),
}

/// The kind of a [`Step`], without its arguments: the name a declared
/// [`crate::profile::UnsupportedCase`] uses for the step that asks for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StepKind {
    /// [`Step::Send`].
    Send,
    /// [`Step::Deliver`].
    Deliver,
    /// [`Step::Drop`].
    Drop,
    /// [`Step::Duplicate`].
    Duplicate,
    /// [`Step::Delay`].
    Delay,
    /// [`Step::Partition`].
    Partition,
    /// [`Step::Heal`].
    Heal,
    /// [`Step::OneWayPartition`].
    OneWayPartition,
    /// [`Step::Corrupt`].
    Corrupt,
    /// [`Step::Forge`].
    Forge,
    /// [`Step::ConnectionReset`].
    ConnectionReset,
    /// [`Step::CrashEndpoint`].
    CrashEndpoint,
    /// [`Step::Recall`].
    Recall,
}

impl StepKind {
    /// Every kind, in [`Step`]'s declaration order.
    pub const ALL: [Self; 13] = [
        Self::Send,
        Self::Deliver,
        Self::Drop,
        Self::Duplicate,
        Self::Delay,
        Self::Partition,
        Self::Heal,
        Self::OneWayPartition,
        Self::Corrupt,
        Self::Forge,
        Self::ConnectionReset,
        Self::CrashEndpoint,
        Self::Recall,
    ];

    /// The step's name, as [`Step`] spells its variant.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Send => "Send",
            Self::Deliver => "Deliver",
            Self::Drop => "Drop",
            Self::Duplicate => "Duplicate",
            Self::Delay => "Delay",
            Self::Partition => "Partition",
            Self::Heal => "Heal",
            Self::OneWayPartition => "OneWayPartition",
            Self::Corrupt => "Corrupt",
            Self::Forge => "Forge",
            Self::ConnectionReset => "ConnectionReset",
            Self::CrashEndpoint => "CrashEndpoint",
            Self::Recall => "Recall",
        }
    }
}

/// Who makes a step's choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Chooser {
    /// The program under test.
    Program,
    /// The scheduler (the Intent Contract's `delivery_order` site).
    Scheduler,
    /// The fault adversary.
    Adversary,
}

impl Step {
    /// Who chooses this step.
    #[must_use]
    pub const fn chooser(&self) -> Chooser {
        match self {
            Self::Send { .. } => Chooser::Program,
            Self::Deliver(_) => Chooser::Scheduler,
            Self::Drop(_)
            | Self::Duplicate(_)
            | Self::Delay(_)
            | Self::Partition(_)
            | Self::Heal
            | Self::OneWayPartition { .. }
            | Self::Corrupt(_)
            | Self::Forge { .. }
            | Self::ConnectionReset(..)
            | Self::CrashEndpoint(_)
            | Self::Recall(_) => Chooser::Adversary,
        }
    }

    /// The step's kind.
    #[must_use]
    pub const fn kind(&self) -> StepKind {
        match self {
            Self::Send { .. } => StepKind::Send,
            Self::Deliver(_) => StepKind::Deliver,
            Self::Drop(_) => StepKind::Drop,
            Self::Duplicate(_) => StepKind::Duplicate,
            Self::Delay(_) => StepKind::Delay,
            Self::Partition(_) => StepKind::Partition,
            Self::Heal => StepKind::Heal,
            Self::OneWayPartition { .. } => StepKind::OneWayPartition,
            Self::Corrupt(_) => StepKind::Corrupt,
            Self::Forge { .. } => StepKind::Forge,
            Self::ConnectionReset(..) => StepKind::ConnectionReset,
            Self::CrashEndpoint(_) => StepKind::CrashEndpoint,
            Self::Recall(_) => StepKind::Recall,
        }
    }

    /// The unsupported semantic this step asks for, if the step is one the profile
    /// does not model at all.
    #[must_use]
    pub const fn unsupported_semantic(&self) -> Option<Semantic> {
        match self {
            Self::OneWayPartition { .. } => Some(Semantic::AsymmetricPartition),
            Self::Corrupt(_) => Some(Semantic::Corruption),
            Self::Forge { .. } => Some(Semantic::Forgery),
            Self::ConnectionReset(..) => Some(Semantic::ConnectionReset),
            Self::CrashEndpoint(_) => Some(Semantic::EndpointCrash),
            Self::Recall(_) => Some(Semantic::RecallInFlight),
            Self::Send { .. }
            | Self::Deliver(_)
            | Self::Drop(_)
            | Self::Duplicate(_)
            | Self::Delay(_)
            | Self::Partition(_)
            | Self::Heal => None,
        }
    }
}

/// One journalled event. Each accepted step yields exactly one event, and
/// [`Event::step`] gives the step back, so a journal is its own choice log.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Event {
    /// An envelope entered flight.
    Sent {
        /// Its ordinal.
        envelope: EnvelopeId,
        /// Sender.
        src: NodeId,
        /// Receiver.
        dst: NodeId,
        /// Payload.
        payload: Payload,
    },
    /// One copy reached its receiver.
    Delivered {
        /// The envelope.
        envelope: EnvelopeId,
        /// Sender.
        src: NodeId,
        /// Receiver.
        dst: NodeId,
    },
    /// One copy was lost.
    Dropped(EnvelopeId),
    /// One copy was added.
    Duplicated(EnvelopeId),
    /// The adversary held the envelope for a step.
    Delayed(EnvelopeId),
    /// A partition began. The side is normalized to the one that holds node 0.
    Partitioned(NodeSet),
    /// The partition ended.
    Healed,
}

impl Event {
    /// The step that produced this event.
    #[must_use]
    pub fn step(&self) -> Step {
        match self {
            Self::Sent {
                src, dst, payload, ..
            } => Step::Send {
                src: *src,
                dst: *dst,
                payload: payload.clone(),
            },
            Self::Delivered { envelope, .. } => Step::Deliver(*envelope),
            Self::Dropped(envelope) => Step::Drop(*envelope),
            Self::Duplicated(envelope) => Step::Duplicate(*envelope),
            Self::Delayed(envelope) => Step::Delay(*envelope),
            Self::Partitioned(side) => Step::Partition(*side),
            Self::Healed => Step::Heal,
        }
    }
}
