//! Typed refusals. A refused step leaves the storage state unchanged and journals
//! nothing.
//!
//! INV-008 keeps "unsupported semantics" and "a resource ran out" apart from each other
//! and from an invalid request. [`RefusalClass`] keeps the same three apart, plus
//! malformed input, exactly as the network and process packs do, and adds one class
//! those packs do not need, a program fault:
//!
//! | Class | Meaning | INV-008 reading |
//! |---|---|---|
//! | `Unsupported` | the profile does not model what the step needs | `Unsupported` |
//! | `ResourceExhausted` | an exploration bound was reached | `ResourceExhausted` |
//! | `NotEnabled` | the step is not a behaviour of the declared envelope in this state | none: the choice log is invalid |
//! | `Malformed` | the step names a node or a crash outcome that cannot exist | none: the choice log is invalid |
//! | `ProgramFault` | the program's own step breaks the storage contract | none: a verdict against the program |
//!
//! The first two are inconclusive outcomes a campaign must report as such. `NotEnabled`
//! and `Malformed` are an invalid choice log; they are never a verdict about the
//! program. `ProgramFault` is the opposite: the program, running, asked for something
//! the device contract forbids, and a campaign reports it as a counterexample against
//! the program, with the choice log up to that step as its witness. Its one case is a
//! write over a torn tail: the replicated register's M07, "recovery ignores
//! checksum/truncated-tail state".

use core::fmt;

use crate::profile::Semantic;
use crate::step::{NodeId, TicketId};

/// Why a step was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Refusal {
    /// The profile does not model this semantic.
    Unsupported(Semantic),
    /// The step is not enabled in this state under the declared configuration.
    NotEnabled(NotEnabled),
    /// The step names something that cannot exist in this configuration or state.
    Malformed(Malformed),
    /// An exploration bound was reached.
    BoundReached(Bound),
    /// The program's own step breaks the storage contract: a verdict against the
    /// program, not an invalid choice log.
    ProgramFault(ProgramFault),
}

/// What the program did wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProgramFault {
    /// A running incarnation submitted or synced while its log ends in a torn record:
    /// its recovery ignored the torn tail instead of truncating it (M07).
    WriteOverTornTail(NodeId),
}

/// Why a step is not enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NotEnabled {
    /// The node has no running incarnation: it crashed and has not restarted.
    NodeDown(NodeId),
    /// The node's incarnation is running, so there is nothing to restart.
    NodeUp(NodeId),
    /// The log does not end in a torn record, so there is nothing to truncate.
    NoTornTail(NodeId),
    /// The ticket is not pending: it was never begun, or it already resolved.
    NotPending(TicketId),
    /// The ticket's flush already finished.
    AlreadyStable(TicketId),
    /// The ticket's records are not stable yet, so its incarnation cannot be told they
    /// are: an acknowledgement waits for the flush.
    NotStable(TicketId),
    /// The incarnation that began the ticket crashed. The crash already decided what
    /// of its flush reached the medium, so the flush cannot finish afterwards.
    IncarnationCrashed(TicketId),
    /// The configuration does not declare this fault.
    FaultNotDeclared(Semantic),
    /// Every crash the configuration allows has been used.
    CrashBudgetSpent {
        /// The configured budget.
        max: u32,
    },
}

/// What is malformed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Malformed {
    /// The node is not below the configured node count.
    UnknownNode(NodeId),
    /// A crash keeps more volatile records than the node has.
    KeepBeyondVolatile {
        /// The records the crash asked to keep.
        keep: u32,
        /// The volatile records the node has.
        volatile: u32,
    },
    /// A torn crash with no lost record to tear: it keeps every volatile record.
    NothingToTear,
}

/// Which bound was reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bound {
    /// The pending-ticket bound.
    Pending {
        /// The configured bound.
        max: u32,
    },
    /// The retained-bytes budget: the journal's canonical encoding would exceed it.
    Retained {
        /// The configured budget.
        max: u64,
    },
    /// The ticket ordinal space.
    Tickets,
    /// The log position space of one node.
    Positions,
    /// The epoch space of one node.
    Epochs,
    /// The configuration's journal-length bound, at most [`crate::step::MAX_STEPS`].
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
    /// The program broke the storage contract.
    ProgramFault,
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
            Self::ProgramFault(_) => RefusalClass::ProgramFault,
        }
    }

    /// The INV-008 `inconclusive_reason` spelling (`assurance-result.schema.json`), for
    /// the two classes that are inconclusive outcomes. `None` for an invalid choice
    /// log, which is not an outcome at all, and for a program fault, which is a
    /// conclusive verdict against the program.
    #[must_use]
    pub const fn inconclusive_reason(&self) -> Option<&'static str> {
        match self.class() {
            RefusalClass::Unsupported => Some("Unsupported"),
            RefusalClass::ResourceExhausted => Some("ResourceExhausted"),
            RefusalClass::NotEnabled | RefusalClass::Malformed | RefusalClass::ProgramFault => None,
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
            Self::ProgramFault(fault) => write!(f, "program fault: {fault:?}"),
        }
    }
}

impl core::error::Error for Refusal {}

/// Why a configuration was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConfigRefusal {
    /// The node count is outside `1..=MAX_NODES`.
    NodesOutOfRange(u8),
    /// The crash budget is above `MAX_CRASHES_CAP`.
    CrashesOutOfRange(u32),
    /// The pending bound is outside `1..=MAX_PENDING_CAP`.
    PendingOutOfRange(u32),
    /// The retained-bytes budget is below the journal header or above
    /// `MAX_RETAINED_CAP`.
    RetainedOutOfRange(u64),
    /// The journal-length bound is outside `1..=MAX_STEPS`.
    StepsOutOfRange(u32),
    /// Torn records are declared without volatile-suffix loss. A tear needs a lost
    /// record, so the declared fault could never happen.
    TornWithoutLoss,
}

impl fmt::Display for ConfigRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "storage configuration refused: {self:?}")
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
