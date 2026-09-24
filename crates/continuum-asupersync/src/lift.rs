//! The lift: a journal read back into `continuum_task::region`, and checked there.
//!
//! # Conformance, not design
//!
//! The region calculus (bn-2gk) is the specification. The lift replays each event as
//! the operation it reports, against a fresh `RegionTree`, and stops at the first event
//! the specification does not admit. It adds exactly three checks the calculus cannot
//! make by itself, because only a journal carries the facts they compare, and a
//! fourth the calculus leaves open:
//!
//! 1. a region the journal says was opened gets the **same ordinal** from the model;
//! 2. a task the journal says was spawned gets the same ordinal;
//! 3. the tasks a drain **reports** cancelled are exactly the tasks the model's drain
//!    moved to a terminal state;
//! 4. a region finalizes only after a drain report covered every region of its
//!    subtree ([`Nonconformance::FinalizeWithoutDrain`], bn-1i050): the calculus admits
//!    `finalize` on any requested region whose workers are terminal, and a journal
//!    that lost its drain report would otherwise lift.
//!
//! A fail-stop crash (bn-20d8u; RFC 0026 correction 58) adds three more checks the
//! calculus, which has no crash step, cannot make: the stopped set is the crashed
//! subtree's live tasks ([`Nonconformance::CrashOutcome`]), a crash's fences come right
//! after it and name only what it owes ([`Nonconformance::FenceOwed`],
//! [`Nonconformance::FenceUnowed`]), and a region crashes once, while it is open or
//! closing ([`Nonconformance::CrashedRegionState`]). In the calculus each stopped worker
//! fails with the reason `crashed`; what it held stays owed there.
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
use std::collections::BTreeSet;

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
    /// A reserve / commit / abort event breaks its reservation's phase machine or the
    /// calculus's publication steps (PR-14-IMPL-02; the reasons are the family's own).
    Effect(family::effect::EffectFault),
    /// An obligation-ledger event breaks the ledger's linear rules, or a region closes
    /// unbalanced (PR-14-IMPL-04; the reasons are the family's own).
    Obligation(family::obligation::LedgerFault),
    /// A virtual time event breaks the parallel clock and timer model (PR-14-IMPL-05;
    /// the reasons are the family's own).
    Time(family::time::TimeFault),
    /// A channel event breaks the parallel channel model (PR-14-IMPL-06; the reasons are
    /// the family's own).
    Channel(family::channel::ChannelFault),
    /// A region's cancellation was requested again: the region is already draining
    /// under cancellation (its own request or an ancestor's), or its subtree was already
    /// drained. The calculus treats a repeat as idempotent, but a run requests a
    /// region's cancellation once and never after its drain, and each repeat costs the
    /// lift a walk of the subtree (cr-3pu5cu round 6, pre-review pass).
    RepeatedCancel {
        /// The region.
        region: u32,
    },
    /// A region finalized while a region of its subtree had no drain report. The
    /// calculus admits `finalize` on any requested region whose workers are terminal;
    /// the journal's teardown is `request → drain → finalize` (research/09 `cancel .
    /// drain . finalize`), and the binding always reports the drain, so a finalize
    /// without one is a lost observation (bn-1i050).
    FinalizeWithoutDrain {
        /// The finalized region.
        region: u32,
        /// The first region of its subtree with no drain report.
        undrained: u32,
    },
    /// A fail-stop crash reported a different set of stopped tasks than the model's:
    /// every task of the crashed subtree that has not ended (bn-20d8u).
    CrashOutcome {
        /// The crashed region.
        region: u32,
        /// Tasks the journal says the crash stopped.
        reported: Vec<u32>,
        /// The subtree's live tasks in the model.
        model: Vec<u32>,
    },
    /// A crash of a region that is neither open nor closing normally, or that an
    /// earlier crash already covered (bn-20d8u).
    CrashedRegionState {
        /// The region.
        region: u32,
        /// The model's token for its state, or `crashed`.
        state: &'static str,
    },
    /// An event came while the last crash still owed the journal a fence: each
    /// reservation, obligation and timer its stopped tasks held is fenced right after
    /// the crash, before anything else (bn-20d8u).
    FenceOwed {
        /// The family of the first owed fence.
        family: Family,
        /// Its ordinal.
        ordinal: u32,
    },
    /// A fence of something the last crash does not owe: not held by a task it
    /// stopped, already resolved, or already fenced (bn-20d8u).
    FenceUnowed {
        /// The fence's family.
        family: Family,
        /// Its ordinal.
        ordinal: u32,
    },
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
            Self::Effect(fault) => write!(f, "reserve/commit/abort: {fault}"),
            Self::Obligation(fault) => write!(f, "obligation ledger: {fault}"),
            Self::Time(fault) => write!(f, "virtual time: {fault}"),
            Self::Channel(fault) => write!(f, "channel: {fault}"),
            Self::RepeatedCancel { region } => write!(
                f,
                "r{region}'s cancellation is requested again, after it was cancelled or drained"
            ),
            Self::FinalizeWithoutDrain { region, undrained } => write!(
                f,
                "r{region} finalized but r{undrained} in its subtree has no drain report"
            ),
            Self::CrashOutcome {
                region,
                reported,
                model,
            } => write!(
                f,
                "the crash of r{region} reported stopped {reported:?} but the model's live tasks are {model:?}"
            ),
            Self::CrashedRegionState { region, state } => {
                write!(f, "r{region} crashes while it is {state}")
            }
            Self::FenceOwed { family, ordinal } => write!(
                f,
                "an event comes while the last crash still owes the {family} fence of #{ordinal}"
            ),
            Self::FenceUnowed { family, ordinal } => {
                write!(f, "a {family} fence of #{ordinal}, which no crash owes")
            }
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
    /// The journal ends inside a protocol this family completes within one substrate
    /// operation (a cancellation not yet absorbed by its drain or its task's end, a
    /// drain not yet finalized): a truncated journal, never a conforming one
    /// (cr-3pu5cu).
    Incomplete(Family),
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
    /// Regions a `RegionDrained` event covered (the drained region's non-finalized
    /// subtree), for the drain-before-finalize check.
    pub(crate) drained: BTreeSet<u32>,
    /// The task the previous event spawned, when the previous event was a spawn: a
    /// budget deadline is declared by the event right after its task's spawn.
    pub(crate) spawned_just_before: Option<u32>,
    /// Regions a crash covered: each crashed region's subtree, as it was at the crash.
    pub(crate) crashed_regions: BTreeSet<u32>,
    /// What the last crash still owes the journal: its fences.
    pub(crate) fences: family::lifecycle::FencesDue,
    pub(crate) effect: family::effect::LiftState,
    pub(crate) cancellation: family::cancellation::LiftState,
    pub(crate) obligation: family::obligation::LiftState,
    pub(crate) time: family::time::LiftState,
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
        /// [`InconclusiveReason::Unsupported`] for a family with no lift, or
        /// [`InconclusiveReason::InsufficientTelemetry`] for a journal that ends inside a
        /// protocol ([`LiftStop::Incomplete`]).
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
/// After the last event, every family that makes a whole-journal claim checks it: the
/// lifecycle family ([`family::lifecycle`]), the reserve/commit/abort family
/// ([`family::effect`]), the cancellation family ([`family::cancellation`]), the
/// obligations family ([`family::obligation`]), the virtual time family
/// ([`family::time`]) and the channel family ([`family::channel`]). Every one of them
/// runs, whatever an earlier one found: a violation, from any family, is reported at
/// the last event's sequence number and outranks an incomplete or an unsupported family
/// an earlier check found, which would otherwise mask it (cr-19ec8g). With no
/// violation, the first incomplete or unsupported family found, in that same order,
/// is reported.
#[must_use]
pub fn lift(journal: &Journal) -> LiftVerdict {
    let mut cx = LiftContext::default();
    // Which families the journal carries is decided before the first event, not when a
    // family's first event arrives: a check that turns on a family's presence (a
    // cleanup step before its task's phases, a deadline request with no declared
    // deadline, a send with no permit) must not be skipped because the family's first
    // event comes later in the same journal (cr-3pu5cu, pre-review adversarial pass).
    for event in journal.events() {
        match event.body().family() {
            Family::Effect => family::effect::mark_present(&mut cx),
            Family::Cancellation => family::cancellation::mark_present(&mut cx),
            Family::Obligation => family::obligation::mark_present(&mut cx),
            Family::Time => family::time::mark_present(&mut cx),
            _ => {}
        }
    }
    for event in journal.events() {
        cx.seq = event.seq();
        match event.body().lift(&mut cx) {
            Ok(()) => {
                cx.spawned_just_before = match event.body() {
                    crate::family::EventBody::Lifecycle(
                        crate::family::lifecycle::LifecycleEvent::TaskSpawned { task, .. },
                    ) => Some(task.0),
                    _ => None,
                };
            }
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
            Err(LiftStop::Incomplete(family)) => {
                return LiftVerdict::Inconclusive {
                    seq: event.seq(),
                    family,
                    reason: InconclusiveReason::InsufficientTelemetry,
                };
            }
        }
    }
    // Every family's whole-journal check runs, whatever an earlier one found: chaining
    // these with `?` or `and_then` would let one family's incomplete short-circuit the
    // rest, silently hiding a genuine violation a later family's check would have
    // reported. A violation, from any family, always outranks an incomplete or an
    // unsupported family found by an earlier one (cr-19ec8g).
    let mut deferred = None;
    for stop in [
        family::lifecycle::finish(&cx),
        family::effect::finish(&cx),
        family::cancellation::finish(&cx),
        family::obligation::finish(&cx),
        family::time::finish(&cx),
        family::channel::finish(&cx),
    ] {
        match stop {
            Ok(()) => {}
            Err(LiftStop::Violation(reason)) => {
                let seq = journal.events().last().map_or(0, |event| event.seq());
                return LiftVerdict::Violates { seq, reason };
            }
            Err(other) => {
                deferred.get_or_insert(other);
            }
        }
    }
    if let Some(stop) = deferred {
        let seq = journal.events().last().map_or(0, |event| event.seq());
        return match stop {
            LiftStop::Violation(_) => unreachable!("violations return above"),
            LiftStop::Unsupported(family) => LiftVerdict::Inconclusive {
                seq,
                family,
                reason: InconclusiveReason::Unsupported,
            },
            LiftStop::Incomplete(family) => LiftVerdict::Inconclusive {
                seq,
                family,
                reason: InconclusiveReason::InsufficientTelemetry,
            },
        };
    }
    LiftVerdict::Conforms(Lifted {
        tree: cx.tree,
        finalizations: cx.finalizations,
    })
}
