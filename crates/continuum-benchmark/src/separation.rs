//! The plan §19.4 family/source-hash separation check.
//!
//! # The rule, and where it comes from
//!
//! > Train/dev/test partitions isolate semantic families and source hashes to reduce
//! > leakage.
//! >
//! > — `notes/plan/plan.md` §19.4
//!
//! > The Phase A benchmark subset used for DX-10 and the G2 Context Pack ablation must
//! > itself pass the §19.4 family/source-hash separation check before either result is
//! > accepted; full leakage validation remains DX-15 at G9.
//! >
//! > — `notes/plan/plan.md` §22 (G0 staging rule); `docs/52_RELEASE_GATES_REV3.md`
//!
//! The second quote is the goal bone's acceptance criterion 2 and is why this module exists
//! *before* a result rather than beside one: [`Report::accepted`](crate::report::Report) is
//! false unless [`check`] returns [`Verdict::Separated`], so a leaked subset produces a
//! report that says so instead of a number somebody might quote.
//!
//! # What is checked
//!
//! Four conditions, and each one is a way the sentence can be false:
//!
//! 1. **Every task is assigned.** A task in no partition is not "held out", it is
//!    unaccounted for.
//! 2. **No semantic family straddles a partition.** This is §19.4's "isolate semantic
//!    families": if `finite-safety` appears in both the development and the held-out suite,
//!    a harness tuned on one has been tuned on the other.
//! 3. **No source hash straddles a partition.** This is §19.4's "and source hashes", and it
//!    is a *different* condition rather than a corollary: two tasks can be different
//!    semantic families over the same `.ctm` bytes — `dh-all` and `dh-type-ok` are exactly
//!    that shape but for the family — and a partition split that separated the families
//!    while sharing the source would leak the model.
//! 4. **Both partitions are non-empty.** A "separated" verdict over a subset where one side
//!    is empty is vacuously true and worth nothing; §19.4's development/held-out split
//!    presumes both exist.
//!
//! # What is deliberately not checked
//!
//! Semantic *similarity* between families, near-duplicate detection, and mutation lineage.
//! Those are DX-15's at G9 ("full leakage validation"), they need the gaming suite and the
//! isolated grader that do not exist yet, and approximating them here would produce a green
//! check that meant less than it looked like.

use std::collections::BTreeMap;

use continuumd::protocol::scalar::Commitment;

use crate::task::{BenchmarkTask, Family, Partition};

/// A way the subset leaks.
///
/// Each variant names the thing that straddles and both partitions it straddles, because a
/// leak report that only said "leaked" would send a reader back to the data to find out
/// what.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Leak {
    /// One semantic family appears in two partitions.
    Family {
        /// The family that straddles.
        family: Family,
        /// A task on one side.
        left: &'static str,
        /// A task on the other.
        right: &'static str,
    },
    /// One source hash appears in two partitions.
    SourceHash {
        /// The `.ctm` content identity that straddles.
        source_hash: String,
        /// A task on one side.
        left: &'static str,
        /// A task on the other.
        right: &'static str,
    },
    /// A partition of the split has no tasks, so "separated" would be vacuous.
    EmptyPartition {
        /// Which side is empty.
        partition: Partition,
    },
}

impl Leak {
    /// The stable one-line rendering a report writes.
    #[must_use]
    pub fn render(&self) -> String {
        match self {
            Self::Family {
                family,
                left,
                right,
            } => format!("family {} straddles: {left} | {right}", family.token()),
            Self::SourceHash {
                source_hash,
                left,
                right,
            } => format!("source-hash {source_hash} straddles: {left} | {right}"),
            Self::EmptyPartition { partition } => {
                format!("partition {} is empty", partition.token())
            }
        }
    }
}

/// What the check answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// The subset passes: families and source hashes are isolated, both sides are populated.
    Separated {
        /// How many tasks were accounted for.
        tasks: usize,
        /// How many distinct semantic families the subset carries.
        families: usize,
        /// How many distinct source hashes the subset carries.
        source_hashes: usize,
    },
    /// The subset leaks, in the ways listed. The list is sorted and complete: a caller sees
    /// every leak rather than the first one.
    Leaked(Vec<Leak>),
}

impl Verdict {
    /// Whether a result computed over this subset may be accepted.
    #[must_use]
    pub const fn passes(&self) -> bool {
        matches!(self, Self::Separated { .. })
    }
}

/// Run the §19.4 check over `subset`.
///
/// The subset is passed in rather than read from [`crate::task::SUBSET`] so the check can be
/// falsified: the negative evidence deliberately hands it a leaked assignment and asserts it
/// is refused. A checker that could only ever see the good input would be untestable, which
/// is the same reason `tools/check_crate_boundaries.py` ships a self-test.
#[must_use]
pub fn check(subset: &[BenchmarkTask]) -> Verdict {
    let mut leaks: Vec<Leak> = Vec::new();

    let mut families: BTreeMap<Family, BTreeMap<Partition, &'static str>> = BTreeMap::new();
    let mut hashes: BTreeMap<String, BTreeMap<Partition, &'static str>> = BTreeMap::new();
    let mut populated: BTreeMap<Partition, usize> = BTreeMap::new();

    for task in subset {
        *populated.entry(task.partition).or_default() += 1;
        families
            .entry(task.family)
            .or_default()
            .entry(task.partition)
            .or_insert(task.id);
        hashes
            .entry(source_hash_token(task))
            .or_default()
            .entry(task.partition)
            .or_insert(task.id);
    }

    for partition in Partition::ALL {
        if populated.get(&partition).copied().unwrap_or_default() == 0 {
            leaks.push(Leak::EmptyPartition { partition });
        }
    }

    for (family, sides) in &families {
        if sides.len() > 1 {
            let mut names = sides.values();
            let left = names.next().copied().unwrap_or_default();
            let right = names.next().copied().unwrap_or_default();
            leaks.push(Leak::Family {
                family: *family,
                left,
                right,
            });
        }
    }

    for (source_hash, sides) in &hashes {
        if sides.len() > 1 {
            let mut names = sides.values();
            let left = names.next().copied().unwrap_or_default();
            let right = names.next().copied().unwrap_or_default();
            leaks.push(Leak::SourceHash {
                source_hash: source_hash.clone(),
                left,
                right,
            });
        }
    }

    if leaks.is_empty() {
        Verdict::Separated {
            tasks: subset.len(),
            families: families.len(),
            source_hashes: hashes.len(),
        }
    } else {
        leaks.sort();
        Verdict::Leaked(leaks)
    }
}

/// The `.ctm` content identity a task's source is keyed by.
///
/// The daemon's own model-registration key (`crate::corpus::Source::source_hash`), rendered
/// as a string so a leak report can name it. Deriving it here rather than storing it on the
/// task is deliberate: a task that carried a *declared* source hash could declare the wrong
/// one, and the check would then be validating a claim instead of the data.
#[must_use]
pub fn source_hash_token(task: &BenchmarkTask) -> String {
    Commitment::as_str(&task.source.source_hash()).to_owned()
}
