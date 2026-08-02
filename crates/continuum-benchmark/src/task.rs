//! The Phase A benchmark subset: four tasks, two semantic families, two partitions.
//!
//! # What a task is
//!
//! A [`BenchmarkTask`] is a corpus port, a verification target over it, a declared state
//! ceiling, and the **frozen** answer the dossier already records for that pair. Nothing
//! here is generated, sampled, or discovered at run time: a task's expected outcome is a
//! number written in `notes/plan/spikes/` and repeated in four other places, so "did the arm
//! complete the task" is decidable against an authority neither arm can influence.
//!
//! # Which families, and why exactly these
//!
//! Plan §19.2 lists fifteen task families. Two of them can be driven end to end against
//! Phase A machinery — "diagnose finite safety failure" and the deadlock half of "diagnose
//! liveness/fairness failure" — and those are the two here, one per corpus port. The other
//! thirteen need subsystems that have not shipped (CML front end, debugger, repair, Forge,
//! proof service), and inventing a task for one of them would have produced a benchmark
//! entry no arm could attempt.
//!
//! Plan §21's Phase A exit names the two ports directly: "Die Hard and Dining Philosophers
//! can be checked through native API, CLI, and an agent client with identical artifacts".
//!
//! # Two tasks per port, and the distinction the second one carries
//!
//! `*-all` targets `all_claims`: every declared predicate, under the strictest deadlock
//! policy. `*-property` targets one named predicate. The pair is not redundancy — it is the
//! observability question research/25 asks ("underpowered: semantically distinct states are
//! observation-equivalent but require different actions"), because the two targets produce
//! *different obligations* over the *same* explored state set, and an interface that could
//! not report which claim was decided would collapse them.
//!
//! One consequence is worth stating rather than leaving to be discovered:
//! [`PHILOSOPHERS_OWNERSHIP`] expects **refuted**, even though `ForkOwnership` is
//! established on all 573 states. `verification.start` maps every target kind onto
//! `DeadlockPolicy::Defect` (`continuumd::daemon::verification::obligations`), and a
//! campaign's verdict is a fold in which "a refutation dominates" — so a model with a
//! reachable deadlock is refuted whichever invariant the caller asked about. That is the
//! daemon's semantics and this task records it rather than working around it; the invariant's
//! own outcome is corroborated one layer down in the evidence tests.

use continuumd::protocol::vocabulary::{SemanticVerdict, TargetKind};

use crate::corpus::Source;
use crate::philosophers;

/// A plan §19.2 semantic family.
///
/// Two members, because two are what Phase A machinery can drive. The enum is not "the
/// families §19.2 lists" — it is the families this subset *contains*, which is what a
/// separation check partitions over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Family {
    /// "diagnose finite safety failure" — a reachable state refutes a declared invariant.
    FiniteSafety,
    /// "diagnose liveness/fairness failure", deadlock half — a reachable state has no
    /// enabled action.
    Deadlock,
}

impl Family {
    /// Every family, in report order.
    pub const ALL: [Self; 2] = [Self::FiniteSafety, Self::Deadlock];

    /// The stable token this family is written as in a report.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::FiniteSafety => "finite-safety",
            Self::Deadlock => "deadlock",
        }
    }
}

/// Which §19.4 partition a task belongs to.
///
/// Plan §19.4 splits the corpus into a development suite "disclosed to implementers" and a
/// held-out suite "excluded from all development, tuning, and regression use". This subset
/// carries both, and [`crate::separation`] is the check that they do not leak into each
/// other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Partition {
    /// Disclosed: used to develop and regress the harness itself.
    Development,
    /// Held out: graded, never tuned against.
    HeldOut,
}

impl Partition {
    /// Every partition, in report order.
    pub const ALL: [Self; 2] = [Self::Development, Self::HeldOut];

    /// The stable token this partition is written as in a report.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::HeldOut => "held-out",
        }
    }
}

/// The frozen answer a task is graded against.
///
/// `states` and `verdict` cross the wire; `transitions` and `witness_depth` do not, and are
/// carried so a test can corroborate them one layer down. `continuumd`'s own PR-8 exit
/// evidence documents why: `Cost` has nine dimensions and none of them is a transition
/// count (RFC 0026 F16), and no producer in this workspace builds the crashpack a witness
/// would ride in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frozen {
    /// Reachable states — `Cost.states` on the wire.
    pub states: u64,
    /// The campaign's folded verdict.
    pub verdict: SemanticVerdict,
    /// Labelled transitions. No wire field; corroborated from `Daemon::state`.
    pub transitions: u64,
    /// Depth of the shortest witness. No wire field; corroborated from `Daemon::state`.
    pub witness_depth: usize,
}

/// One benchmark task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BenchmarkTask {
    /// The stable identifier a report names this task by.
    pub id: &'static str,
    /// Which semantic family it belongs to.
    pub family: Family,
    /// Which §19.4 partition it belongs to.
    pub partition: Partition,
    /// Which corpus port it runs over.
    pub source: Source,
    /// What `verification.start` aims at.
    pub target_kind: TargetKind,
    /// The target's identifier: a port name for `all_claims`, a predicate name otherwise.
    pub target_id: &'static str,
    /// The state ceiling a faithful policy declares.
    ///
    /// Comfortably above the frozen state count in every case, because a task is about the
    /// *interface* and a task that ran out of budget by design would be measuring the
    /// budget. Budget exhaustion is a **fault**, injected on purpose by
    /// [`crate::policy::Fault::TightBudget`], and it is scheduled rather than baked in.
    pub ceiling: u64,
    /// The dossier's frozen answer.
    pub expected: Frozen,
}

/// `dh-all` — Die Hard, every claim.
pub const DIE_HARD_ALL: BenchmarkTask = BenchmarkTask {
    id: "dh-all",
    family: Family::FiniteSafety,
    partition: Partition::Development,
    source: Source::DieHard,
    target_kind: TargetKind::AllClaims,
    target_id: "DieHard",
    ceiling: 64,
    expected: Frozen {
        states: 16,
        verdict: SemanticVerdict::Refuted,
        transitions: 96,
        witness_depth: 6,
    },
};

/// `dh-type-ok` — Die Hard, the `TypeOK` invariant alone, which holds.
pub const DIE_HARD_TYPE_OK: BenchmarkTask = BenchmarkTask {
    id: "dh-type-ok",
    family: Family::FiniteSafety,
    partition: Partition::Development,
    source: Source::DieHard,
    target_kind: TargetKind::Property,
    target_id: "TypeOK",
    ceiling: 64,
    expected: Frozen {
        states: 16,
        verdict: SemanticVerdict::Established,
        transitions: 96,
        // No witness: nothing was refuted. Zero is the honest reading of "the shortest
        // counterexample has no steps because there is no counterexample", and the evidence
        // tests assert the *absence* rather than a depth.
        witness_depth: 0,
    },
};

/// `dp-all` — Dining Philosophers, every claim, under the strictest deadlock policy.
pub const PHILOSOPHERS_ALL: BenchmarkTask = BenchmarkTask {
    id: "dp-all",
    family: Family::Deadlock,
    partition: Partition::HeldOut,
    source: Source::Philosophers,
    target_kind: TargetKind::AllClaims,
    target_id: "DiningPhilosophers",
    ceiling: 1_024,
    expected: Frozen {
        states: philosophers::FROZEN_STATES,
        verdict: SemanticVerdict::Refuted,
        transitions: philosophers::FROZEN_TRANSITIONS,
        witness_depth: philosophers::FROZEN_DEADLOCK_DEPTH,
    },
};

/// `dp-ownership` — Dining Philosophers, the `ForkOwnership` invariant alone.
///
/// Expects **refuted**: the invariant holds, and the deadlock the target did not ask about
/// refutes the campaign anyway. See this module's documentation.
pub const PHILOSOPHERS_OWNERSHIP: BenchmarkTask = BenchmarkTask {
    id: "dp-ownership",
    family: Family::Deadlock,
    partition: Partition::HeldOut,
    source: Source::Philosophers,
    target_kind: TargetKind::Property,
    target_id: philosophers::FORK_OWNERSHIP,
    ceiling: 1_024,
    expected: Frozen {
        states: philosophers::FROZEN_STATES,
        verdict: SemanticVerdict::Refuted,
        transitions: philosophers::FROZEN_TRANSITIONS,
        witness_depth: philosophers::FROZEN_DEADLOCK_DEPTH,
    },
};

/// The Phase A benchmark subset, in report order.
pub const SUBSET: &[BenchmarkTask] = &[
    DIE_HARD_ALL,
    DIE_HARD_TYPE_OK,
    PHILOSOPHERS_ALL,
    PHILOSOPHERS_OWNERSHIP,
];
