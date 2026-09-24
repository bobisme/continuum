//! Typed refusals. A refused step leaves the network unchanged and journals nothing.
//!
//! INV-008 keeps "unsupported semantics" and "a resource ran out" apart from each other
//! and from an invalid request. [`RefusalClass`] keeps the same three apart, plus
//! malformed input:
//!
//! | Class | Meaning | INV-008 reading |
//! |---|---|---|
//! | `Unsupported` | the profile does not model what the step needs | `Unsupported` |
//! | `ResourceExhausted` | an exploration bound was reached | `ResourceExhausted` |
//! | `NotEnabled` | the step is not a behaviour of the declared envelope in this state | none: the choice log is invalid |
//! | `Malformed` | the step names a node or a side that cannot exist | none: the choice log is invalid |
//!
//! The first two are inconclusive outcomes a campaign must report as such. The last
//! two are an invalid choice log; they are never a verdict about the program.

use core::fmt;

use crate::profile::Semantic;
use crate::step::{EnvelopeId, NodeId, NodeSet};

/// Why a step was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Refusal {
    /// The profile does not model this semantic.
    Unsupported(Semantic),
    /// The step is not enabled in this state under the declared configuration.
    NotEnabled(NotEnabled),
    /// The step names something that cannot exist in this configuration.
    Malformed(Malformed),
    /// An exploration bound was reached.
    BoundReached(Bound),
}

/// Why a step is not enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NotEnabled {
    /// No copy of the envelope is in flight (never sent, or every copy consumed).
    NotInFlight(EnvelopeId),
    /// The active partition separates the envelope's sender from its receiver.
    Partitioned(EnvelopeId),
    /// Reordering is not declared, and an older envelope on the same link is still in
    /// flight.
    WouldReorder {
        /// The envelope the step names.
        envelope: EnvelopeId,
        /// The oldest in-flight envelope on its link.
        ahead: EnvelopeId,
    },
    /// The configuration does not declare this fault.
    FaultNotDeclared(Semantic),
    /// `Heal` with no active partition.
    NoPartition,
    /// Every partition the configuration allows has been used.
    PartitionBudgetSpent {
        /// The configured budget.
        max: u32,
    },
}

/// What is malformed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Malformed {
    /// The node is not below the configured node count.
    UnknownNode(NodeId),
    /// A partition side names a node outside the configuration.
    SideOutsideNodes(NodeSet),
    /// A partition side that is empty or holds every node cuts nothing.
    TrivialSide(NodeSet),
}

/// Which bound was reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bound {
    /// The in-flight copy bound.
    InFlight {
        /// The configured bound.
        max: u32,
    },
    /// The payload byte bound.
    Payload {
        /// The configured bound.
        max: u32,
    },
    /// The retained-bytes budget: the journal's canonical encoding would exceed it.
    Retained {
        /// The configured budget.
        max: u64,
    },
    /// The envelope ordinal space.
    Envelopes,
    /// The choice-log and journal length [`crate::MAX_STEPS`].
    Steps {
        /// The bound.
        max: usize,
    },
}

/// The four refusal classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RefusalClass {
    /// Outside the profile.
    Unsupported,
    /// An exploration bound was reached.
    ResourceExhausted,
    /// Not a behaviour of the declared envelope here.
    NotEnabled,
    /// Names something that cannot exist.
    Malformed,
}

impl Refusal {
    /// The refusal's class.
    #[must_use]
    pub const fn class(&self) -> RefusalClass {
        match self {
            Self::Unsupported(_) => RefusalClass::Unsupported,
            Self::BoundReached(_) => RefusalClass::ResourceExhausted,
            Self::NotEnabled(_) => RefusalClass::NotEnabled,
            Self::Malformed(_) => RefusalClass::Malformed,
        }
    }

    /// The INV-008 `inconclusive_reason` spelling (`assurance-result.schema.json`), for
    /// the two classes that are inconclusive outcomes. `None` for an invalid choice
    /// log, which is not an outcome at all.
    #[must_use]
    pub const fn inconclusive_reason(&self) -> Option<&'static str> {
        match self.class() {
            RefusalClass::Unsupported => Some("Unsupported"),
            RefusalClass::ResourceExhausted => Some("ResourceExhausted"),
            RefusalClass::NotEnabled | RefusalClass::Malformed => None,
        }
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(semantic) => {
                write!(f, "unsupported {semantic}: {}", semantic.statement())
            }
            Self::NotEnabled(why) => write!(f, "not enabled: {why:?}"),
            Self::Malformed(why) => write!(f, "malformed: {why:?}"),
            Self::BoundReached(bound) => write!(f, "bound reached: {bound:?}"),
        }
    }
}

impl core::error::Error for Refusal {}

/// Why a configuration was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConfigRefusal {
    /// The node count is outside `1..=MAX_NODES`.
    NodesOutOfRange(u8),
    /// The in-flight bound is outside `1..=MAX_IN_FLIGHT_CAP`.
    InFlightOutOfRange(u32),
    /// The payload bound is above `MAX_PAYLOAD_CAP`.
    PayloadOutOfRange(u32),
    /// The retained-bytes budget is below the journal header or above
    /// `MAX_RETAINED_CAP`.
    RetainedOutOfRange(u64),
}

impl fmt::Display for ConfigRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "network configuration refused: {self:?}")
    }
}

impl core::error::Error for ConfigRefusal {}

/// A refused choice log: the index of the first refused step and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RunRefusal {
    /// The index of the refused step; for a log refused before any step ran,
    /// [`Bound::Steps`], it is the log length.
    pub index: usize,
    /// Why.
    pub refusal: Refusal,
}

impl fmt::Display for RunRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "step {}: {}", self.index, self.refusal)
    }
}

impl core::error::Error for RunRefusal {}
