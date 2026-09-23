//! Schedules: concurrency as written-down data, so an adversarial interleaving is a
//! value a test can enumerate rather than a race it has to provoke.
//!
//! # Why interleaving is data here
//!
//! The G0-DX-13 campaign
//! (`crates/continuum-workspace/tests/dx13_falsification.rs`) attacks a genuinely
//! thread-safe store, so it pins an interleaving by parking one publisher at a phase and
//! letting the test drive the other side — a rendezvous. Its house rules are the durable
//! part, and they carry over unchanged:
//!
//! - **assertions are on outcomes, never on interleavings** — forcing a schedule is not
//!   the same as asserting one occurred;
//! - **determinism is checked as a repeated-round invariant** over a canonical rendering,
//!   never over arrival order;
//! - **a test that can fail because a machine was loaded is not evidence.**
//!
//! What does *not* carry over is the thread. A [`RegionTree`] has none: plan §20 gives
//! this crate no async-runtime edge, INV-005 forbids ambient scheduling, and every
//! operation takes `&mut self`. So the interleaving that the campaign had to *force* is
//! here simply written down, and the consequence is a strictly stronger kind of evidence:
//! [`interleavings`] enumerates the whole schedule space of a set of per-worker programs
//! and a test asserts the property over every member of it, rather than over one point in
//! it that a rendezvous happened to reach.
//!
//! # A run does not stop at the first refusal
//!
//! [`Schedule::run`] executes every step and records each one's [`Result`]. That is
//! deliberate: a refused step changes nothing — which is the entire value of a typed
//! refusal — so continuing is faithful, and an adversarial schedule *wants* to keep
//! going after "spawn into a draining region" is rejected in order to check that the
//! rejection did not corrupt anything. [`Run::faults`] collects the refusals for the
//! tests that are about them.

use core::fmt;

use super::obligation::{SubstrateObligation, SubstrateOutcome};
use super::worker::{Resumability, WorkerId, WorkerStep};
use super::{Finalization, RegionFault, RegionId, RegionState, RegionTree, WorkerState};

/// One move against a [`RegionTree`].
///
/// The ten variants are the tree's ten mutating operations, one for one. Identities
/// are named rather than returned, which works because they are dense ordinals allocated
/// in call order: a schedule that opens two children knows they are `r1` and `r2` before
/// it runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// [`RegionTree::open_child`].
    OpenChild {
        /// The parent to open inside.
        parent: RegionId,
    },
    /// [`RegionTree::spawn`].
    Spawn {
        /// The region to admit the worker into.
        region: RegionId,
        /// Whether the work can be resumed.
        resumability: Resumability,
    },
    /// [`RegionTree::advance`].
    Advance {
        /// The worker to move.
        worker: WorkerId,
        /// The move.
        step: WorkerStep,
    },
    /// [`RegionTree::close`].
    Close {
        /// The region to close.
        region: RegionId,
    },
    /// [`RegionTree::cancel`].
    Cancel {
        /// The region to cancel.
        region: RegionId,
    },
    /// [`RegionTree::drain`].
    Drain {
        /// The region to drain.
        region: RegionId,
    },
    /// [`RegionTree::finalize`].
    Finalize {
        /// The region to finalize.
        region: RegionId,
    },
    /// [`RegionTree::open_substrate`].
    OpenSubstrate {
        /// The worker that takes the obligation.
        holder: WorkerId,
        /// The adapter obligation.
        obligation: SubstrateObligation,
    },
    /// [`RegionTree::discharge_substrate`].
    DischargeSubstrate {
        /// The worker that holds the obligation.
        holder: WorkerId,
        /// The adapter obligation.
        obligation: SubstrateObligation,
        /// How it ended.
        outcome: SubstrateOutcome,
    },
    /// [`RegionTree::transfer_substrate`].
    TransferSubstrate {
        /// The worker that holds the obligation.
        from: WorkerId,
        /// The adapter obligation.
        obligation: SubstrateObligation,
        /// The worker that receives it.
        to: WorkerId,
    },
}

impl Step {
    /// Advance `worker` — the common case, spelled once.
    #[must_use]
    pub const fn advance(worker: WorkerId, step: WorkerStep) -> Self {
        Self::Advance { worker, step }
    }

    /// A canonical one-line rendering.
    #[must_use]
    pub fn render(&self) -> String {
        match self {
            Self::OpenChild { parent } => format!("open-child {parent}"),
            Self::Spawn {
                region,
                resumability,
            } => {
                let resumability = match resumability {
                    Resumability::Resumable => "resumable".to_owned(),
                    Resumability::NonResumable(reason) => format!("non-resumable({reason})"),
                };
                format!("spawn {region} {resumability}")
            }
            Self::Advance { worker, step } => format!("advance {worker} {step}"),
            Self::Close { region } => format!("close {region}"),
            Self::Cancel { region } => format!("cancel {region}"),
            Self::Drain { region } => format!("drain {region}"),
            Self::Finalize { region } => format!("finalize {region}"),
            Self::OpenSubstrate { holder, obligation } => {
                format!("open-substrate {holder} {obligation}")
            }
            Self::DischargeSubstrate {
                holder,
                obligation,
                outcome,
            } => format!("discharge-substrate {holder} {obligation} {outcome}"),
            Self::TransferSubstrate {
                from,
                obligation,
                to,
            } => format!("transfer-substrate {from} {obligation} {to}"),
        }
    }
}

impl fmt::Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render())
    }
}

/// What a step did, when it was not refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepOutcome {
    /// A child region was opened.
    Opened(RegionId),
    /// A worker was admitted.
    Spawned(WorkerId),
    /// A worker moved, and landed here.
    Advanced(WorkerState),
    /// A region's state changed without producing an identity — close, cancel, drain.
    Requested(RegionState),
    /// A subtree was finalized.
    Finalized(Box<Finalization>),
    /// An adapter obligation was opened, discharged, or transferred.
    Substrate(SubstrateObligation),
}

impl StepOutcome {
    /// A canonical one-line rendering.
    ///
    /// A [`Self::Finalized`] outcome renders as its totality summary rather than as the
    /// whole report: a trace is for comparing runs, and the full report is available
    /// from [`Run::finalizations`] for the tests that want it.
    #[must_use]
    pub fn render(&self) -> String {
        match self {
            Self::Opened(region) => format!("opened {region}"),
            Self::Spawned(worker) => format!("spawned {worker}"),
            Self::Advanced(state) => format!("advanced {state}"),
            Self::Requested(state) => format!("requested {state}"),
            Self::Finalized(finalization) => format!(
                "finalized {} workers={} total={}",
                finalization.region(),
                finalization.workers().len(),
                finalization.is_total()
            ),
            Self::Substrate(obligation) => format!("substrate {obligation}"),
        }
    }
}

/// A written-down run: a sequence of [`Step`]s and nothing else.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Schedule(Vec<Step>);

impl Schedule {
    /// A schedule of these steps, in this order.
    #[must_use]
    pub fn new(steps: Vec<Step>) -> Self {
        Self(steps)
    }

    /// The steps.
    #[must_use]
    pub fn steps(&self) -> &[Step] {
        &self.0
    }

    /// How many steps.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether there are no steps.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// This schedule followed by `other`'s steps.
    #[must_use]
    pub fn then(mut self, other: &Self) -> Self {
        self.0.extend(other.0.iter().cloned());
        self
    }

    /// Run every step against `tree`, recording each one's result.
    ///
    /// A refused step changes nothing and the run continues past it — see this module's
    /// header for why. The returned [`Run`] is the evidence: the trace for determinism
    /// comparisons, the faults for legality tests, and the finalizations for the
    /// no-orphan property.
    #[must_use]
    pub fn run(&self, tree: &mut RegionTree) -> Run {
        let mut records = Vec::with_capacity(self.0.len());
        for step in &self.0 {
            let outcome = match step.clone() {
                Step::OpenChild { parent } => tree.open_child(parent).map(StepOutcome::Opened),
                Step::Spawn {
                    region,
                    resumability,
                } => tree.spawn(region, resumability).map(StepOutcome::Spawned),
                Step::Advance { worker, step } => {
                    tree.advance(worker, step).map(StepOutcome::Advanced)
                }
                Step::Close { region } => tree
                    .close(region)
                    .and_then(|()| tree.state(region))
                    .map(StepOutcome::Requested),
                Step::Cancel { region } => tree
                    .cancel(region)
                    .and_then(|()| tree.state(region))
                    .map(StepOutcome::Requested),
                Step::Drain { region } => tree
                    .drain(region)
                    .and_then(|()| tree.state(region))
                    .map(StepOutcome::Requested),
                Step::Finalize { region } => tree
                    .finalize(region)
                    .map(|finalization| StepOutcome::Finalized(Box::new(finalization))),
                Step::OpenSubstrate { holder, obligation } => tree
                    .open_substrate(holder, obligation)
                    .map(|()| StepOutcome::Substrate(obligation)),
                Step::DischargeSubstrate {
                    holder,
                    obligation,
                    outcome,
                } => tree
                    .discharge_substrate(holder, obligation, outcome)
                    .map(|()| StepOutcome::Substrate(obligation)),
                Step::TransferSubstrate {
                    from,
                    obligation,
                    to,
                } => tree
                    .transfer_substrate(from, obligation, to)
                    .map(|()| StepOutcome::Substrate(obligation)),
            };
            records.push((step.clone(), outcome));
        }
        Run { records }
    }

    /// A canonical rendering of the schedule itself, one step per line.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        for step in &self.0 {
            out.push_str(&step.render());
            out.push('\n');
        }
        out
    }
}

/// What running a [`Schedule`] produced: every step paired with what it did or why it was
/// refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    records: Vec<(Step, Result<StepOutcome, RegionFault>)>,
}

impl Run {
    /// Every step and its result, in schedule order.
    pub fn records(&self) -> &[(Step, Result<StepOutcome, RegionFault>)] {
        &self.records
    }

    /// The refusals, paired with the steps that earned them.
    #[must_use]
    pub fn faults(&self) -> Vec<(&Step, &RegionFault)> {
        self.records
            .iter()
            .filter_map(|(step, outcome)| outcome.as_ref().err().map(|fault| (step, fault)))
            .collect()
    }

    /// The finalization reports this run produced, in schedule order.
    #[must_use]
    pub fn finalizations(&self) -> Vec<&Finalization> {
        self.records
            .iter()
            .filter_map(|(_, outcome)| match outcome {
                Ok(StepOutcome::Finalized(finalization)) => Some(finalization.as_ref()),
                _ => None,
            })
            .collect()
    }

    /// A canonical trace: one line per step, `step -> outcome`.
    ///
    /// Byte-comparable across runs, with no timestamp, address or iteration order in it,
    /// which is what makes "two runs of one schedule are byte-identical" a check rather
    /// than a hope (INV-005).
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        for (step, outcome) in &self.records {
            let rendered = match outcome {
                Ok(success) => success.render(),
                Err(fault) => format!("refused {fault}"),
            };
            out.push_str(&format!("{} -> {rendered}\n", step.render()));
        }
        out
    }
}

/// Every interleaving of `programs` that preserves each program's own order.
///
/// The standard shuffle product: at each position any program with steps left may take
/// its next one. For programs of lengths `n₁ … nₖ` there are `(Σnᵢ)! / Πnᵢ!` results —
/// three two-step programs give ninety — so callers keep the inputs small and the
/// enumeration exhaustive. That trade is the right way round for this module: a
/// schedule space small enough to enumerate completely is better evidence than a large
/// one sampled.
///
/// The output order is itself deterministic — programs are offered in index order at
/// every position — so an enumeration is reproducible and its `n`-th member is a stable
/// thing to name in a failure message.
#[must_use]
pub fn interleavings(programs: &[Vec<Step>]) -> Vec<Schedule> {
    let total: usize = programs.iter().map(Vec::len).sum();
    let mut out = Vec::new();
    let mut cursors = vec![0_usize; programs.len()];
    let mut prefix = Vec::with_capacity(total);
    weave(programs, &mut cursors, &mut prefix, total, &mut out);
    out
}

fn weave(
    programs: &[Vec<Step>],
    cursors: &mut [usize],
    prefix: &mut Vec<Step>,
    total: usize,
    out: &mut Vec<Schedule>,
) {
    if prefix.len() == total {
        out.push(Schedule::new(prefix.clone()));
        return;
    }
    for (index, program) in programs.iter().enumerate() {
        let cursor = cursors[index];
        if cursor >= program.len() {
            continue;
        }
        prefix.push(program[cursor].clone());
        cursors[index] = cursor + 1;
        weave(programs, cursors, prefix, total, out);
        cursors[index] = cursor;
        prefix.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::super::DrainCause;
    use super::*;

    fn program(worker: u32) -> Vec<Step> {
        vec![
            Step::advance(WorkerId::at(worker), WorkerStep::Begin),
            Step::advance(WorkerId::at(worker), WorkerStep::Complete),
        ]
    }

    #[test]
    fn the_shuffle_product_of_three_two_step_programs_has_ninety_members() {
        let schedules = interleavings(&[program(0), program(1), program(2)]);
        assert_eq!(schedules.len(), 90, "6! / (2! * 2! * 2!) = 90");
        for schedule in &schedules {
            assert_eq!(schedule.len(), 6);
        }
    }

    #[test]
    fn every_interleaving_preserves_each_programs_own_order() {
        for schedule in interleavings(&[program(0), program(1)]) {
            for worker in [0_u32, 1] {
                let mine: Vec<&Step> = schedule
                    .steps()
                    .iter()
                    .filter(|step| {
                        matches!(step, Step::Advance { worker: w, .. } if w.ordinal() == worker)
                    })
                    .collect();
                assert_eq!(mine, vec![&program(worker)[0], &program(worker)[1]]);
            }
        }
    }

    #[test]
    fn enumerating_the_same_programs_twice_yields_the_same_schedules_in_the_same_order() {
        let first = interleavings(&[program(0), program(1)]);
        let second = interleavings(&[program(0), program(1)]);
        assert_eq!(first, second);
        assert!(!first.is_empty(), "a vacuous enumeration proves nothing");
    }

    #[test]
    fn a_run_continues_past_a_refusal_and_records_it() {
        let mut tree = RegionTree::new();
        let root = tree.root();
        let schedule = Schedule::new(vec![
            Step::Spawn {
                region: root,
                resumability: Resumability::Resumable,
            },
            Step::Cancel { region: root },
            Step::Spawn {
                region: root,
                resumability: Resumability::Resumable,
            },
            Step::Drain { region: root },
            Step::Finalize { region: root },
        ]);
        let run = schedule.run(&mut tree);
        let faults = run.faults();
        assert_eq!(faults.len(), 1, "exactly the spawn into a draining region");
        assert!(matches!(
            faults[0].1,
            RegionFault::SpawnIntoClosedRegion {
                state: RegionState::Draining(DrainCause::Cancelled),
                ..
            }
        ));
        let finalizations = run.finalizations();
        assert_eq!(finalizations.len(), 1);
        assert!(
            finalizations[0].is_total(),
            "the refusal must not have corrupted the teardown"
        );
        assert_eq!(
            tree.worker_count(),
            1,
            "a refused spawn admits no worker at all"
        );
    }

    #[test]
    fn two_runs_of_one_schedule_render_byte_identically() {
        let root = RegionId::at(0);
        let schedule = Schedule::new(vec![
            Step::Spawn {
                region: root,
                resumability: Resumability::Resumable,
            },
            Step::advance(WorkerId::at(0), WorkerStep::Begin),
            Step::advance(WorkerId::at(0), WorkerStep::Reserve),
            Step::Cancel { region: root },
            Step::Drain { region: root },
            Step::Finalize { region: root },
        ]);
        let first = schedule.run(&mut RegionTree::new()).render();
        let second = schedule.run(&mut RegionTree::new()).render();
        assert_eq!(first.as_bytes(), second.as_bytes());
        assert!(!first.is_empty(), "a vacuous trace proves nothing");
    }
}
