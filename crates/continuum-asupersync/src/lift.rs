//! The lift: a journal read back into `continuum_task::region`, and checked there.
//!
//! # Conformance, not design
//!
//! The region calculus (bn-2gk) is the specification. The lift replays each event as
//! the operation it reports, against a fresh `RegionTree`, and stops at the first event
//! the specification does not admit. It adds exactly three checks the calculus cannot
//! make by itself, because only a journal carries the facts they compare:
//!
//! 1. a region the journal says was opened gets the **same ordinal** from the model;
//! 2. a task the journal says was spawned gets the same ordinal;
//! 3. the tasks a drain **reports** cancelled are exactly the tasks the model's drain
//!    moved to a terminal state.
//!
//! Everything else — work enters only an open region, cancellation is subtree-wide,
//! drain is total under cancellation and blocking under close, finalize requires every
//! owned task terminal and discharges the obligation ledger — is the calculus's own
//! `RegionFault` table, reported unchanged. So a journal that lifts to
//! [`LiftVerdict::Conforms`] satisfies the same properties bn-2gk proved over schedules,
//! including the no-orphan post-condition on every `RegionFinalized` it contains.
//!
//! # Three outcomes, never a boolean (INV-008)
//!
//! [`LiftVerdict`] is conforms, violates (at a sequence number, with a typed reason), or
//! inconclusive (an event of a family with no lift yet: unsupported semantics).

use core::fmt;

use continuum_task::region::{Finalization, RegionFault, RegionTree};
use continuum_value::assurance::InconclusiveReason;

use crate::family::{self, Family};
use crate::journal::Journal;

/// Why an event does not conform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Nonconformance {
    /// The region calculus refused the operation the event reports.
    Refused(RegionFault),
    /// The journal names a new region by an ordinal the model did not allocate.
    RegionIdentity {
        /// The journal's ordinal.
        journal: u32,
        /// The model's.
        model: u32,
    },
    /// The journal names a new task by an ordinal the model did not allocate.
    TaskIdentity {
        /// The journal's ordinal.
        journal: u32,
        /// The model's.
        model: u32,
    },
    /// A drain reported a different set of cancelled tasks than the model's drain.
    DrainOutcome {
        /// The drained region.
        region: u32,
        /// Tasks the journal says the drain cancelled.
        reported: Vec<u32>,
        /// Tasks the model's drain terminated.
        model: Vec<u32>,
    },
    /// A cancellation-phase event does not refine the calculus's cancel → drain step
    /// (PR-14-IMPL-03; the reasons are the family's own).
    Cancellation(family::cancellation::CancellationFault),
}

impl fmt::Display for Nonconformance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(fault) => write!(f, "the region calculus refuses it: {fault}"),
            Self::RegionIdentity { journal, model } => {
                write!(
                    f,
                    "the journal opened r{journal} but the model allocated r{model}"
                )
            }
            Self::TaskIdentity { journal, model } => {
                write!(
                    f,
                    "the journal spawned t{journal} but the model allocated w{model}"
                )
            }
            Self::DrainOutcome {
                region,
                reported,
                model,
            } => write!(
                f,
                "draining r{region} reported cancelled {reported:?} but the model cancelled {model:?}"
            ),
            Self::Cancellation(fault) => write!(f, "cancellation phases: {fault}"),
        }
    }
}

/// Why lifting one event stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiftStop {
    /// The event does not conform.
    Violation(Nonconformance),
    /// The event's family has no lift yet.
    Unsupported(Family),
}

impl From<RegionFault> for LiftStop {
    fn from(fault: RegionFault) -> Self {
        Self::Violation(Nonconformance::Refused(fault))
    }
}

/// The lift's shared state: the model tree every family lifts into, and each family's
/// own state. A family that lands adds nothing here.
#[derive(Debug, Default)]
pub struct LiftContext {
    pub(crate) tree: RegionTree,
    pub(crate) seq: u64,
    pub(crate) finalizations: Vec<(u64, Finalization)>,
    #[allow(dead_code)] // filled by PR-14-IMPL-02
    pub(crate) effect: family::effect::LiftState,
    pub(crate) cancellation: family::cancellation::LiftState,
    #[allow(dead_code)] // filled by PR-14-IMPL-04
    pub(crate) obligation: family::obligation::LiftState,
    #[allow(dead_code)] // filled by PR-14-IMPL-05
    pub(crate) time: family::time::LiftState,
    #[allow(dead_code)] // filled by PR-14-IMPL-06
    pub(crate) channel: family::channel::LiftState,
}

/// What a conforming lift produced.
#[derive(Debug)]
pub struct Lifted {
    tree: RegionTree,
    finalizations: Vec<(u64, Finalization)>,
}

impl Lifted {
    /// The model tree after every event.
    #[must_use]
    pub const fn tree(&self) -> &RegionTree {
        &self.tree
    }

    /// Each `RegionFinalized` event's sequence number and the model's report for it.
    #[must_use]
    pub fn finalizations(&self) -> &[(u64, Finalization)] {
        &self.finalizations
    }
}

/// The lift's verdict over a whole journal.
#[derive(Debug)]
pub enum LiftVerdict {
    /// Every event is admitted by the region calculus and every cross-check agrees.
    Conforms(Lifted),
    /// The event at `seq` is not admitted.
    Violates {
        /// The first nonconforming event.
        seq: u64,
        /// Why.
        reason: Nonconformance,
    },
    /// The event at `seq` belongs to a family with no lift yet. Not success.
    Inconclusive {
        /// The first event that could not be lifted.
        seq: u64,
        /// Its family.
        family: Family,
        /// Always [`InconclusiveReason::Unsupported`] today.
        reason: InconclusiveReason,
    },
}

impl LiftVerdict {
    /// The conforming lift, if this is one.
    #[must_use]
    pub const fn conforming(&self) -> Option<&Lifted> {
        match self {
            Self::Conforms(lifted) => Some(lifted),
            _ => None,
        }
    }
}

/// Lift `journal` into a fresh region tree.
///
/// After the last event, the families that make a whole-journal claim check it. Today
/// that is the cancellation family ([`family::cancellation`]): a violation it finds is
/// reported at the last event's sequence number.
#[must_use]
pub fn lift(journal: &Journal) -> LiftVerdict {
    let mut cx = LiftContext::default();
    for event in journal.events() {
        cx.seq = event.seq();
        match event.body().lift(&mut cx) {
            Ok(()) => {}
            Err(LiftStop::Violation(reason)) => {
                return LiftVerdict::Violates {
                    seq: event.seq(),
                    reason,
                };
            }
            Err(LiftStop::Unsupported(family)) => {
                return LiftVerdict::Inconclusive {
                    seq: event.seq(),
                    family,
                    reason: InconclusiveReason::Unsupported,
                };
            }
        }
    }
    if let Err(stop) = family::cancellation::finish(&cx) {
        let seq = journal.events().last().map_or(0, |event| event.seq());
        return match stop {
            LiftStop::Violation(reason) => LiftVerdict::Violates { seq, reason },
            LiftStop::Unsupported(family) => LiftVerdict::Inconclusive {
                seq,
                family,
                reason: InconclusiveReason::Unsupported,
            },
        };
    }
    LiftVerdict::Conforms(Lifted {
        tree: cx.tree,
        finalizations: cx.finalizations,
    })
}
