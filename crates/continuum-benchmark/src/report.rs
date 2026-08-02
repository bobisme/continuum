//! The result artifact: typed, canonically rendered, byte-stable.
//!
//! # Why an artifact and not a printout
//!
//! INV-003 — schemas decide, prose does not — applies to this crate's own output as much as
//! to the daemon's. A [`Report`] is a value; [`Report::render`] is a total function from that
//! value to bytes; and the reproduction evidence compares two renderings byte for byte. A
//! report that were assembled by printing as it went could not be compared, and a difference
//! between two runs would be invisible until somebody read both.
//!
//! # The rendering rules
//!
//! - **Deterministic order.** Rows are sorted by `(task, seed, policy, arm)` — all four are
//!   closed, ordered sets — so no map's iteration order reaches the bytes.
//! - **No floats.** Rates are integer per-mille and ratios are rendered as `a/b`. A float's
//!   decimal rendering is a platform question, and this artifact is compared across runs.
//! - **No clock, no path, no address.** Nothing in a report identifies the machine that
//!   produced it, which is what lets two machines produce the same bytes.
//! - **Every section is derived** from [`Report`]'s own fields. There is no free text.
//!
//! # What the report refuses to say
//!
//! It does not say whether G0-DX-10 passed. The ratified margins in plan §24.5 are computed
//! and printed — that is what [`Margins`] is — but the *decision* they inform is the exit
//! bone's and the falsification bone's, and a harness that graded its own experiment would be
//! the self-certification INV-004 forbids in the small. The `limits` section says so in the
//! artifact itself, so a reader who has only the artifact still knows what it is not.

use std::collections::BTreeMap;

use continuumd::protocol::spec::ProtocolEnum;

use crate::policy::PolicyKind;
use crate::run::{APPROX_TOKEN_RULE, ArmRun, BYTES_PER_APPROX_TOKEN, OperationCost};
use crate::separation::{self, Verdict};
use crate::surface::Arm;
use crate::task::{BenchmarkTask, SUBSET};

/// The ratified plan §24.5 margins, as numbers.
///
/// > native ACI must beat the disciplined-shell baseline by at least 10 percentage points of
/// > absolute task success, at least 30% fewer interface bytes per solved task …, and at
/// > least a 50% relative reduction in invalid-action rate
/// >
/// > — `notes/plan/plan.md` §24.5, quote-id `aci-benchmark-margins`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Thresholds {
    /// Percentage points of absolute task success the native arm must win by.
    pub success_points: i64,
    /// Percent fewer interface bytes per solved task the native arm must spend.
    pub byte_saving_percent: i64,
    /// Percent relative reduction in invalid-action rate the native arm must achieve.
    pub invalid_reduction_percent: i64,
    /// Seeds each metric must be paired over.
    pub seeds: usize,
}

/// The ratified thresholds.
pub const RATIFIED: Thresholds = Thresholds {
    success_points: 10,
    byte_saving_percent: 30,
    invalid_reduction_percent: 50,
    seeds: 3,
};

/// One arm's totals over the whole matrix.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ArmTotals {
    /// Cells run.
    pub runs: u32,
    /// Cells solved.
    pub solved: u32,
    /// Operations attempted.
    pub attempted: u32,
    /// Operations admitted.
    pub admitted: u32,
    /// Attempts that were not admitted.
    pub invalid: u32,
    /// Invalid attempts that spent no interface bytes.
    pub zero_cost_invalid: u32,
    /// Interface bytes spent on cells that were solved.
    pub bytes_on_solved: u64,
    /// Interface bytes spent on every cell.
    pub bytes_total: u64,
    /// Cells that fired at least one fault.
    pub faulted: u32,
    /// Faulted cells that reached the frozen answer anyway.
    pub recovered: u32,
}

impl ArmTotals {
    /// Task success, in per-mille.
    #[must_use]
    pub const fn success_permille(&self) -> i64 {
        if self.runs == 0 {
            return 0;
        }
        (self.solved as i64 * 1000) / self.runs as i64
    }

    /// Invalid-action rate, in per-mille of attempts.
    #[must_use]
    pub const fn invalid_permille(&self) -> i64 {
        if self.attempted == 0 {
            return 0;
        }
        (self.invalid as i64 * 1000) / self.attempted as i64
    }

    /// Interface bytes per solved task, or [`None`] when nothing was solved.
    #[must_use]
    pub const fn bytes_per_solved(&self) -> Option<u64> {
        if self.solved == 0 {
            return None;
        }
        Some(self.bytes_on_solved / self.solved as u64)
    }

    /// Recovery rate over faulted cells, in per-mille.
    #[must_use]
    pub const fn recovery_permille(&self) -> i64 {
        if self.faulted == 0 {
            return 0;
        }
        (self.recovered as i64 * 1000) / self.faulted as i64
    }
}

/// How the two arms compared, against the ratified thresholds.
///
/// Every field is a computed number. None of them is a verdict: see this module's
/// documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Margins {
    /// Native success minus shell success, in per-mille points.
    pub success_points_permille: i64,
    /// Percent fewer bytes per solved task the native arm spent, or [`None`] when either arm
    /// solved nothing.
    pub byte_saving_percent: Option<i64>,
    /// Percent relative reduction in invalid-action rate, or [`None`] when the shell arm's
    /// rate is zero and a relative reduction is undefined.
    pub invalid_reduction_percent: Option<i64>,
    /// Whether each metric was paired over at least [`Thresholds::seeds`] seeds.
    pub seeds_paired: usize,
    /// Whether the success margin clears its threshold.
    pub success_clears: bool,
    /// Whether the byte margin clears its threshold.
    pub bytes_clear: bool,
    /// Whether the invalid-action margin clears its threshold.
    pub invalid_clears: bool,
}

/// The result artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The §19.4 separation verdict over the subset the runs were computed on.
    pub separation: Verdict,
    /// Whether the result may be accepted at all.
    pub accepted: bool,
    /// Every run, sorted.
    pub runs: Vec<ArmRun>,
    /// Per-arm totals.
    pub totals: BTreeMap<Arm, ArmTotals>,
    /// The computed margins.
    pub margins: Margins,
    /// Calls and interface bytes per wire operation, per arm.
    pub per_operation: BTreeMap<(Arm, &'static str), OperationCost>,
}

impl Report {
    /// Assemble a report from both arms' runs.
    ///
    /// The §19.4 check runs *here*, over [`SUBSET`], and its verdict decides
    /// [`Report::accepted`]. A caller cannot assemble an accepted report over a leaked
    /// subset: the gate is inside the constructor rather than beside it.
    #[must_use]
    pub fn new(runs: Vec<ArmRun>) -> Self {
        Self::over(SUBSET, runs)
    }

    /// Assemble a report over an explicitly named subset.
    ///
    /// Used by the negative evidence, which hands in a deliberately leaked assignment and
    /// asserts the report refuses to be accepted.
    #[must_use]
    pub fn over(subset: &[BenchmarkTask], mut runs: Vec<ArmRun>) -> Self {
        runs.sort_by(|left, right| {
            (left.task, left.seed, left.policy, left.arm).cmp(&(
                right.task,
                right.seed,
                right.policy,
                right.arm,
            ))
        });
        let mut totals: BTreeMap<Arm, ArmTotals> = BTreeMap::new();
        for arm in Arm::ALL {
            totals.insert(arm, ArmTotals::default());
        }
        let mut seeds: BTreeMap<u16, ()> = BTreeMap::new();
        for run in &runs {
            seeds.insert(run.seed, ());
            let entry = totals.entry(run.arm).or_default();
            entry.runs += 1;
            entry.attempted += run.attempted;
            entry.admitted += run.admitted;
            entry.invalid += run.invalid;
            entry.zero_cost_invalid += run.zero_cost_invalid;
            entry.bytes_total += run.bytes;
            if run.solved {
                entry.solved += 1;
                entry.bytes_on_solved += run.bytes;
            }
            if !run.faults.is_empty() {
                entry.faulted += 1;
                if run.recovered {
                    entry.recovered += 1;
                }
            }
        }

        let mut per_operation: BTreeMap<(Arm, &'static str), OperationCost> = BTreeMap::new();
        for run in &runs {
            for (operation, cost) in &run.per_operation {
                let entry = per_operation.entry((run.arm, operation)).or_default();
                entry.calls += cost.calls;
                entry.bytes += cost.bytes;
            }
        }

        let native = totals.get(&Arm::Native).copied().unwrap_or_default();
        let shell = totals.get(&Arm::Shell).copied().unwrap_or_default();
        let margins = compare(&native, &shell, seeds.len());
        let separation = separation::check(subset);
        let accepted = separation.passes();
        Self {
            separation,
            accepted,
            runs,
            totals,
            margins,
            per_operation,
        }
    }

    /// The canonical rendering.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("continuum-benchmark aci-report v1\n");
        out.push_str("# G0-DX-10 instrument. This artifact reports; it does not decide.\n");
        out.push('\n');

        out.push_str("[separation]\n");
        match &self.separation {
            Verdict::Separated {
                tasks,
                families,
                source_hashes,
            } => {
                out.push_str(&format!(
                    "verdict: separated\ntasks: {tasks}\nfamilies: {families}\n\
                     source_hashes: {source_hashes}\n"
                ));
            }
            Verdict::Leaked(leaks) => {
                out.push_str("verdict: leaked\n");
                for leak in leaks {
                    out.push_str(&format!("leak: {}\n", leak.render()));
                }
            }
        }
        out.push_str(&format!("accepted: {}\n\n", self.accepted));

        out.push_str("[runs]\n");
        out.push_str(
            "task seed policy arm attempted admitted invalid zero_cost_invalid bytes \
             approx_tokens solved recovered faults\n",
        );
        for run in &self.runs {
            let faults = if run.faults.is_empty() {
                "-".to_owned()
            } else {
                run.faults
                    .iter()
                    .map(|fault| fault.token())
                    .collect::<Vec<_>>()
                    .join(",")
            };
            out.push_str(&format!(
                "{} {} {} {} {} {} {} {} {} {} {} {} {}\n",
                run.task,
                run.seed,
                run.policy.token(),
                run.arm.token(),
                run.attempted,
                run.admitted,
                run.invalid,
                run.zero_cost_invalid,
                run.bytes,
                run.approx_tokens,
                run.solved,
                run.recovered,
                faults,
            ));
        }
        out.push('\n');

        out.push_str("[totals]\n");
        for arm in Arm::ALL {
            let totals = self.totals.get(&arm).copied().unwrap_or_default();
            out.push_str(&format!(
                "{}: runs={} solved={}/{} success_permille={} attempted={} admitted={} \
                 invalid={} zero_cost_invalid={} invalid_permille={} bytes_total={} \
                 bytes_per_solved={} faulted={} recovered={} recovery_permille={}\n",
                arm.token(),
                totals.runs,
                totals.solved,
                totals.runs,
                totals.success_permille(),
                totals.attempted,
                totals.admitted,
                totals.invalid,
                totals.zero_cost_invalid,
                totals.invalid_permille(),
                totals.bytes_total,
                totals
                    .bytes_per_solved()
                    .map_or_else(|| "none".to_owned(), |value| value.to_string()),
                totals.faulted,
                totals.recovered,
                totals.recovery_permille(),
            ));
        }
        out.push('\n');

        out.push_str("[margins]\n");
        out.push_str(&format!(
            "ratified: success_points={} byte_saving_percent={} invalid_reduction_percent={} \
             seeds={}\n",
            RATIFIED.success_points,
            RATIFIED.byte_saving_percent,
            RATIFIED.invalid_reduction_percent,
            RATIFIED.seeds,
        ));
        out.push_str(&format!(
            "measured: success_points_permille={} byte_saving_percent={} \
             invalid_reduction_percent={} seeds_paired={}\n",
            self.margins.success_points_permille,
            self.margins
                .byte_saving_percent
                .map_or_else(|| "undefined".to_owned(), |value| value.to_string()),
            self.margins
                .invalid_reduction_percent
                .map_or_else(|| "undefined".to_owned(), |value| value.to_string()),
            self.margins.seeds_paired,
        ));
        out.push_str(&format!(
            "clears: success={} bytes={} invalid={}\n\n",
            self.margins.success_clears, self.margins.bytes_clear, self.margins.invalid_clears,
        ));

        out.push_str("[operations]\n");
        out.push_str("# where the interface bytes go. A redesign is a valid exit for this\n");
        out.push_str("# lane, and a redesign needs to know which operation is expensive.\n");
        out.push_str("arm operation calls bytes bytes_per_call\n");
        for ((arm, operation), cost) in &self.per_operation {
            let per_call = if cost.calls == 0 {
                0
            } else {
                cost.bytes / u64::from(cost.calls)
            };
            out.push_str(&format!(
                "{} {operation} {} {} {per_call}\n",
                arm.token(),
                cost.calls,
                cost.bytes,
            ));
        }
        out.push('\n');

        out.push_str("[counting]\n");
        out.push_str(&format!(
            "approx_token_rule: {APPROX_TOKEN_RULE}\nbytes_per_approx_token: \
             {BYTES_PER_APPROX_TOKEN}\ngraded_denominator: interface bytes per solved task \
             (RFC 0027)\n"
        ));
        out.push_str(
            "native_bytes: request frame + result frame\n\
             shell_bytes: command line + rendered output (the CLI's own frames are not \
             charged)\n\n",
        );

        out.push_str("[limits]\n");
        out.push_str(
            "agent: scripted deterministic policies, not a live model; this instrument \
             cannot answer whether a real agent would *discover* better use of either \
             surface.\n\
             baseline: a text projection of this daemon, not PR 13's CLI, which does not \
             exist yet.\n\
             subset: four tasks over two semantic families; full leakage validation is \
             DX-15 at G9.\n\
             verdict: none. The DX-10 decision is bn-762i's and bn-2c0a's.\n",
        );
        out
    }

    /// The frozen answers this report was graded against, as a stable line per task.
    ///
    /// Rendered separately from [`Report::render`] because it is a property of the *subset*
    /// rather than of a run, and a reader checking whether a number is the dossier's wants it
    /// without the matrix in the way.
    #[must_use]
    pub fn render_expectations() -> String {
        let mut out = String::from("[expected]\n");
        for task in SUBSET {
            out.push_str(&format!(
                "{} family={} partition={} port={} target={}:{} ceiling={} states={} \
                 verdict={} transitions={} witness_depth={}\n",
                task.id,
                task.family.token(),
                task.partition.token(),
                task.source.port(),
                task.target_kind.as_wire(),
                task.target_id,
                task.ceiling,
                task.expected.states,
                task.expected.verdict.as_wire(),
                task.expected.transitions,
                task.expected.witness_depth,
            ));
        }
        out
    }
}

/// Compute the margins from two arms' totals.
fn compare(native: &ArmTotals, shell: &ArmTotals, seeds: usize) -> Margins {
    let success_points_permille = native.success_permille() - shell.success_permille();
    let byte_saving_percent = match (native.bytes_per_solved(), shell.bytes_per_solved()) {
        (Some(native_bytes), Some(shell_bytes)) if shell_bytes > 0 => {
            Some(((shell_bytes as i64 - native_bytes as i64) * 100) / shell_bytes as i64)
        }
        _ => None,
    };
    let shell_invalid = shell.invalid_permille();
    let invalid_reduction_percent = if shell_invalid > 0 {
        Some(((shell_invalid - native.invalid_permille()) * 100) / shell_invalid)
    } else {
        None
    };
    Margins {
        success_points_permille,
        byte_saving_percent,
        invalid_reduction_percent,
        seeds_paired: seeds,
        success_clears: success_points_permille >= RATIFIED.success_points * 10,
        bytes_clear: byte_saving_percent.is_some_and(|value| value >= RATIFIED.byte_saving_percent),
        invalid_clears: invalid_reduction_percent
            .is_some_and(|value| value >= RATIFIED.invalid_reduction_percent),
    }
}

/// A stable ordering for the policy kind, used by the report's row sort.
impl PolicyKind {
    /// The sort key.
    #[must_use]
    pub const fn order(self) -> u8 {
        match self {
            Self::Faithful => 0,
            Self::Fallible => 1,
        }
    }
}
