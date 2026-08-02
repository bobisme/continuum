//! Measurement variants: the accounting rules bn-2c0a's falsification campaign runs
//! **beside** the landed ones, never instead of them.
//!
//! # What this module is for
//!
//! `crate::run` computes the five landed metrics under one accounting rule, fixed before
//! any number was taken. That rule embeds choices, and a choice is not a fact. This module
//! makes each choice a *parameter* and re-runs the arithmetic, so the question "is the
//! measured result an artefact of the harness?" is answered with a second number rather
//! than with a paragraph.
//!
//! Two directions, because a falsification campaign that only attacked one would be
//! advocacy with tests:
//!
//! - **pro-shell-bias** — the landed rule gives the baseline a deliberate handicap
//!   advantage (`crate::shell`, "the baseline is not charged for the CLI's own protocol
//!   traffic"). [`Accounting`] prices it.
//! - **pro-native-bias** — the landed rule gives the typed arm a free local refusal, a
//!   *disciplined* baseline that never misreads, and a completion criterion containing the
//!   one datum the baseline pays extra for. [`Accounting::price_local_refusals`],
//!   [`crate::shell::Disciplines`] and [`ceiling_free_completion`] price those.
//!
//! # Nothing here changes a landed number
//!
//! Every landed constructor keeps its landed behaviour: [`ShellSurface::new`] is
//! [`Disciplines::ALL`] and [`Renderer::Standard`], and [`crate::shell::HiddenCost`] enters
//! no [`Observation::bytes`](crate::surface::Observation). A variant is a *second* row in
//! the sensitivity table, and the falsification report prints the landed row beside it with
//! what each over- and under-counts named.

use std::collections::BTreeMap;

use continuumd::codec::to_bytes;
use continuumd::protocol::envelope::{Cost, EpochSet, ResultEnvelope};
use continuumd::protocol::spec::{Nullable, Optional};

use crate::policy::{Policy, PolicyKind, SCHEDULES};
use crate::report::{ArmTotals, RATIFIED};
use crate::rig::Rig;
use crate::run::{self, ArmRun};
use crate::shell::{Disciplines, HiddenCost, Renderer, ShellSurface};
use crate::surface::{Arm, SurfaceError};
use crate::task::{BenchmarkTask, SUBSET};

// --- driving the baseline under a named configuration ---------------------------------

/// One cell of a variant sweep: the run, and what the baseline spent underneath it.
#[derive(Debug, Clone)]
pub struct Cell {
    /// The run, scored by `crate::run` exactly as a landed run is.
    pub run: ArmRun,
    /// What the CLI spent that the agent was not charged for.
    pub hidden: HiddenCost,
    /// The answers the CLI process received, when the sweep recorded them.
    pub recorded: Vec<crate::shell::Recorded>,
}

/// Drive the baseline over the whole matrix under a named renderer and discipline set.
///
/// The matrix, the policies, the seeds and the fresh-rig-per-cell rule are `crate::run`'s;
/// only the baseline's configuration varies. `record` keeps every answer the CLI process
/// received, which the envelope decomposition needs and nothing else does.
///
/// # Errors
///
/// [`SurfaceError`] as [`crate::run::drive`].
pub fn shell_sweep(
    renderer: Renderer,
    disciplines: Disciplines,
    record: bool,
) -> Result<Vec<Cell>, SurfaceError> {
    let mut cells = Vec::new();
    for task in SUBSET {
        for schedule in SCHEDULES {
            for kind in PolicyKind::ALL {
                let mut rig = Rig::fresh();
                let mut surface = ShellSurface::configured(renderer, disciplines);
                if record {
                    surface = surface.recording();
                }
                let run = run::drive(
                    &mut surface,
                    &mut rig,
                    task,
                    Policy::new(kind, schedule.seed),
                )?;
                cells.push(Cell {
                    run,
                    hidden: surface.hidden(),
                    recorded: surface.recorded().to_vec(),
                });
            }
        }
    }
    Ok(cells)
}

/// The landed baseline, with its hidden ledger read off.
///
/// # Errors
///
/// [`SurfaceError`] as [`shell_sweep`].
pub fn metered_shell_sweep() -> Result<Vec<Cell>, SurfaceError> {
    shell_sweep(Renderer::Standard, Disciplines::ALL, false)
}

/// The interface bytes one connection handshake costs.
///
/// The hello frame plus the welcome frame, read off a fresh rig — the same two frames the
/// typed client is charged for once per run, and the same two a `continuum` process would
/// exchange on every invocation.
#[must_use]
pub fn handshake_bytes() -> u64 {
    let rig = Rig::fresh();
    rig.hello_frame().len() as u64 + rig.welcome_frame().len() as u64
}

// --- the accounting rules --------------------------------------------------------------

/// One named accounting rule.
///
/// Each flag is one choice the landed rule made, restated as a parameter. All four false is
/// the landed rule, and [`LANDED`] is that value under its own name so the sensitivity
/// table's first row is the artifact bn-134i published rather than a re-derivation of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Accounting {
    /// The stable token the report writes this rule as.
    pub token: &'static str,
    /// Charge the baseline for the CLI process's own request and result frames.
    pub charge_cli_frames: bool,
    /// Charge the baseline one connection handshake per process invocation.
    pub charge_invocation_handshakes: bool,
    /// Charge the expansion command a real process invocation and round trip, instead of
    /// serving it from an envelope the harness is already holding.
    pub charge_expansion_round_trip: bool,
    /// Charge the typed arm for composing an operation its register refused locally.
    pub price_local_refusals: bool,
}

/// The landed rule: the artifact bn-134i published.
pub const LANDED: Accounting = Accounting {
    token: "landed",
    charge_cli_frames: false,
    charge_invocation_handshakes: false,
    charge_expansion_round_trip: false,
    price_local_refusals: false,
};

/// The baseline charged for the CLI's own protocol traffic.
pub const CLI_FRAMES: Accounting = Accounting {
    token: "cli-frames",
    charge_cli_frames: true,
    ..LANDED
};

/// The baseline charged for its frames and for one handshake per invocation.
pub const CLI_FRAMES_AND_HANDSHAKES: Accounting = Accounting {
    token: "cli-frames+handshakes",
    charge_cli_frames: true,
    charge_invocation_handshakes: true,
    ..LANDED
};

/// Fully symmetric: every byte and every negotiation either arm's stack performs.
pub const SYMMETRIC: Accounting = Accounting {
    token: "symmetric",
    charge_cli_frames: true,
    charge_invocation_handshakes: true,
    charge_expansion_round_trip: true,
    ..LANDED
};

/// The landed rule, with the typed arm's locally refused calls priced at composition.
pub const COMPOSED_REFUSALS_PRICED: Accounting = Accounting {
    token: "composed-refusals-priced",
    price_local_refusals: true,
    ..LANDED
};

/// Symmetric on both sides at once: the baseline's hidden traffic and the typed arm's
/// hidden composition, both charged.
pub const SYMMETRIC_BOTH_WAYS: Accounting = Accounting {
    token: "symmetric-both-ways",
    charge_cli_frames: true,
    charge_invocation_handshakes: true,
    charge_expansion_round_trip: true,
    price_local_refusals: true,
};

/// Every accounting rule, in report order.
pub const ACCOUNTING_RULES: &[Accounting] = &[
    LANDED,
    CLI_FRAMES,
    CLI_FRAMES_AND_HANDSHAKES,
    SYMMETRIC,
    COMPOSED_REFUSALS_PRICED,
    SYMMETRIC_BOTH_WAYS,
];

impl Accounting {
    /// The baseline's interface bytes for one cell under this rule.
    #[must_use]
    pub fn shell_bytes(self, cell: &Cell, handshake: u64) -> u64 {
        let invocations = u64::from(cell.hidden.invocations);
        let mean_frame_pair = cell
            .hidden
            .frame_bytes()
            .checked_div(invocations)
            .unwrap_or_default();
        let expansions = u64::from(cell.hidden.expansions);
        let mut frames = cell.hidden.frame_bytes();
        let mut processes = invocations;
        if self.charge_expansion_round_trip {
            frames += expansions * mean_frame_pair;
            processes += expansions;
        }
        let mut bytes = cell.run.bytes;
        if self.charge_cli_frames {
            bytes += frames;
        }
        if self.charge_invocation_handshakes {
            bytes += processes * handshake;
        }
        bytes
    }

    /// The typed arm's interface bytes for one run under this rule.
    ///
    /// `composition` is what one locally refused call cost the agent to *compose* — the
    /// request frame it would have written, measured rather than assumed. The landed rule
    /// charges zero for it, which is true of the wire and false of the agent.
    #[must_use]
    pub fn native_bytes(self, run: &ArmRun, composition: u64) -> u64 {
        if self.price_local_refusals {
            run.bytes + u64::from(run.zero_cost_invalid) * composition
        } else {
            run.bytes
        }
    }
}

// --- the sensitivity table --------------------------------------------------------------

/// One row of the sensitivity table: both arms' totals under one named rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sensitivity {
    /// What the row varies.
    pub rule: String,
    /// Cells the typed arm solved.
    pub native_solved: u32,
    /// Cells the baseline solved.
    pub shell_solved: u32,
    /// Cells run, per arm.
    pub runs: u32,
    /// Native success minus shell success, in per-mille points.
    pub success_points_permille: i64,
    /// Typed-arm interface bytes per solved task.
    pub native_bytes_per_solved: Option<u64>,
    /// Baseline interface bytes per solved task.
    pub shell_bytes_per_solved: Option<u64>,
    /// Percent fewer bytes per solved task the typed arm spent.
    pub byte_saving_percent: Option<i64>,
    /// Typed-arm invalid-action rate, per mille of attempts.
    pub native_invalid_permille: i64,
    /// Baseline invalid-action rate, per mille of attempts.
    pub shell_invalid_permille: i64,
    /// Percent relative reduction in invalid-action rate.
    pub invalid_reduction_percent: Option<i64>,
    /// Whether each ratified margin clears under this row.
    pub clears: (bool, bool, bool),
}

impl Sensitivity {
    /// Whether all three ratified margins clear under this row.
    #[must_use]
    pub const fn clears_all(&self) -> bool {
        self.clears.0 && self.clears.1 && self.clears.2
    }

    /// The row, as one stable line.
    #[must_use]
    pub fn render(&self) -> String {
        format!(
            "{} native_solved={}/{} shell_solved={}/{} success_points_permille={} \
             native_bytes_per_solved={} shell_bytes_per_solved={} byte_saving_percent={} \
             native_invalid_permille={} shell_invalid_permille={} \
             invalid_reduction_percent={} clears=success:{},bytes:{},invalid:{}",
            self.rule,
            self.native_solved,
            self.runs,
            self.shell_solved,
            self.runs,
            self.success_points_permille,
            render_option(self.native_bytes_per_solved),
            render_option(self.shell_bytes_per_solved),
            render_option(self.byte_saving_percent),
            self.native_invalid_permille,
            self.shell_invalid_permille,
            render_option(self.invalid_reduction_percent),
            self.clears.0,
            self.clears.1,
            self.clears.2,
        )
    }
}

fn render_option<T: core::fmt::Display>(value: Option<T>) -> String {
    value.map_or_else(|| "undefined".to_owned(), |value| value.to_string())
}

/// Fold one arm's runs into totals, with each run's bytes supplied by the caller.
///
/// The byte total is a parameter rather than `run.bytes` so a variant can re-price a run
/// without rebuilding it. Everything else — solved, attempted, invalid — is the run's own,
/// scored by `crate::run` and untouched here.
fn totals_with(runs: &[ArmRun], bytes: impl Fn(usize, &ArmRun) -> u64) -> ArmTotals {
    let mut totals = ArmTotals::default();
    for (index, run) in runs.iter().enumerate() {
        let cost = bytes(index, run);
        totals.runs += 1;
        totals.attempted += run.attempted;
        totals.admitted += run.admitted;
        totals.invalid += run.invalid;
        totals.zero_cost_invalid += run.zero_cost_invalid;
        totals.bytes_total += cost;
        if run.solved {
            totals.solved += 1;
            totals.bytes_on_solved += cost;
        }
        if !run.faults.is_empty() {
            totals.faulted += 1;
            if run.recovered {
                totals.recovered += 1;
            }
        }
    }
    totals
}

/// Compare two arms' totals against the ratified margins.
///
/// The same arithmetic `crate::report::compare` performs, restated over totals a variant
/// built. It is restated rather than shared because `report::compare` is private to the
/// artifact it serves, and a falsification module that reached into it could not be read as
/// an independent check.
#[must_use]
pub fn row(rule: impl Into<String>, native: &ArmTotals, shell: &ArmTotals) -> Sensitivity {
    let success_points_permille = native.success_permille() - shell.success_permille();
    let native_bytes_per_solved = native.bytes_per_solved();
    let shell_bytes_per_solved = shell.bytes_per_solved();
    let byte_saving_percent = match (native_bytes_per_solved, shell_bytes_per_solved) {
        (Some(native_bytes), Some(shell_bytes)) if shell_bytes > 0 => {
            Some(((shell_bytes as i64 - native_bytes as i64) * 100) / shell_bytes as i64)
        }
        _ => None,
    };
    let shell_invalid = shell.invalid_permille();
    let native_invalid = native.invalid_permille();
    let invalid_reduction_percent =
        (shell_invalid > 0).then(|| ((shell_invalid - native_invalid) * 100) / shell_invalid);
    Sensitivity {
        rule: rule.into(),
        native_solved: native.solved,
        shell_solved: shell.solved,
        runs: native.runs.max(shell.runs),
        success_points_permille,
        native_bytes_per_solved,
        shell_bytes_per_solved,
        byte_saving_percent,
        native_invalid_permille: native_invalid,
        shell_invalid_permille: shell_invalid,
        invalid_reduction_percent,
        clears: (
            success_points_permille >= RATIFIED.success_points * 10,
            byte_saving_percent.is_some_and(|value| value >= RATIFIED.byte_saving_percent),
            invalid_reduction_percent
                .is_some_and(|value| value >= RATIFIED.invalid_reduction_percent),
        ),
    }
}

/// The sensitivity row for one accounting rule over one pair of sweeps.
#[must_use]
pub fn under(
    rule: Accounting,
    native: &[ArmRun],
    shell: &[Cell],
    composition: u64,
    handshake: u64,
) -> Sensitivity {
    let native_totals = totals_with(native, |_, run| rule.native_bytes(run, composition));
    let shell_runs: Vec<ArmRun> = shell.iter().map(|cell| cell.run.clone()).collect();
    let shell_totals = totals_with(&shell_runs, |index, _| {
        rule.shell_bytes(&shell[index], handshake)
    });
    row(rule.token, &native_totals, &shell_totals)
}

/// The sensitivity row for a pair of sweeps under the landed accounting.
#[must_use]
pub fn plain(rule: impl Into<String>, native: &[ArmRun], shell: &[ArmRun]) -> Sensitivity {
    row(
        rule,
        &totals_with(native, |_, run| run.bytes),
        &totals_with(shell, |_, run| run.bytes),
    )
}

// --- envelope separability ---------------------------------------------------------------

/// What one field of the result envelope costs, and whether a redesign could amortize it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldCost {
    /// The envelope field.
    pub field: &'static str,
    /// One of [`CONFORMANCE_VARYING`], [`CONFORMANCE_CONSTANT`], [`AMORTIZABLE`].
    pub class: &'static str,
    /// Bytes it costs across every answer the matrix produced.
    pub bytes: u64,
}

/// Required by RFC 0026 and different on every answer. Not reducible without changing what
/// the protocol guarantees, which is a redesign of the *guarantee*, not of the encoding.
pub const CONFORMANCE_VARYING: &str = "conformance-varying";

/// Required by RFC 0026 and **identical on every answer of one connection**. A redesign
/// could pin it at the handshake and answer with it by reference, dropping no guarantee.
/// This is the redesign-actionable half of the conformance bytes.
pub const CONFORMANCE_CONSTANT: &str = "conformance-constant";

/// Not required to travel on every answer at all: a redesign could send it on demand.
pub const AMORTIZABLE: &str = "amortizable";

/// Where the typed arm's result bytes go, field by field.
///
/// Measured, not modelled: each field is reduced to the smallest value its declaration
/// admits, the envelope is re-encoded with `continuumd`'s own codec, and the difference is
/// the field's cost. The sum of the reductions is not the total, because canonical JSON's
/// separators mean the reductions are not independent; [`Decomposition::floor_bytes`] is
/// the envelope with every reduction applied at once, which is the number a redesign is
/// bounded below by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decomposition {
    /// Answers decomposed.
    pub answers: u32,
    /// Result-frame bytes, summed.
    pub result_bytes: u64,
    /// Request-frame bytes, summed.
    pub request_bytes: u64,
    /// Per-field costs, in report order.
    pub fields: Vec<FieldCost>,
    /// Result bytes with every amortizable and every repeated-conformance field reduced at
    /// once — the floor a redesign of the *envelope* could reach without touching payloads.
    pub floor_bytes: u64,
    /// Request-argument bytes spent on `workspace.create`, which is where
    /// `SnapshotComponents` travels.
    pub snapshot_component_bytes: u64,
    /// How many `workspace.create` requests those bytes are spread over.
    pub snapshot_component_calls: u32,
    /// Calls and request-frame bytes, per wire operation.
    ///
    /// The request half of the breakdown `crate::report` prints for the whole exchange, and
    /// the measurement [`Accounting::price_local_refusals`] needs: what an operation costs
    /// to *compose* is the frame a client writes for it.
    pub request_by_operation: BTreeMap<&'static str, (u32, u64)>,
}

impl Decomposition {
    /// Bytes in fields of one class.
    #[must_use]
    pub fn class_bytes(&self, class: &str) -> u64 {
        self.fields
            .iter()
            .filter(|cost| cost.class == class)
            .map(|cost| cost.bytes)
            .sum()
    }

    /// Every byte a redesign could remove **without dropping a protocol guarantee**: the
    /// fields that need not travel at all, the fields that are constant across a
    /// connection, and `SnapshotComponents` on the request.
    #[must_use]
    pub fn redesign_reducible(&self) -> u64 {
        self.class_bytes(AMORTIZABLE)
            + self.class_bytes(CONFORMANCE_CONSTANT)
            + self.snapshot_component_bytes
    }

    /// Every byte a redesign could remove if it were also willing to change what the
    /// envelope guarantees: the whole envelope reduced to its floor, plus
    /// `SnapshotComponents`. The lower bound on any redesign of this encoding, and the
    /// number that says whether the byte loss is an envelope-overhead artefact.
    #[must_use]
    pub fn maximal_reducible(&self) -> u64 {
        self.result_bytes.saturating_sub(self.floor_bytes) + self.snapshot_component_bytes
    }

    /// The mean request frame one operation costs to compose.
    #[must_use]
    pub fn compose_price(&self, operation: &str) -> u64 {
        self.request_by_operation
            .get(operation)
            .filter(|(calls, _)| *calls > 0)
            .map_or(0, |(calls, bytes)| bytes / u64::from(*calls))
    }
}

/// The minimal legal value of each reducible field, applied one at a time.
fn reduce(envelope: &ResultEnvelope, field: &str) -> ResultEnvelope {
    let mut reduced = envelope.clone();
    match field {
        "epochs" => {
            reduced.epochs = EpochSet {
                protocol: envelope.epochs.protocol,
                semantic: Nullable::Null,
                intent: Nullable::Null,
                evidence: Nullable::Null,
                proof: Nullable::Null,
                corpus: Nullable::Null,
                engine: Nullable::Null,
            };
        }
        "next_operations" => reduced.next_operations = Vec::new(),
        "assurance" => reduced.assurance = Optional::Absent,
        "cost" => {
            reduced.cost = Cost {
                wall_ms: Optional::Absent,
                cpu_ms: Optional::Absent,
                memory_bytes: Optional::Absent,
                states: Optional::Absent,
                solver_ms: Optional::Absent,
                proof_ms: Optional::Absent,
                tokens: Optional::Absent,
                candidates: Optional::Absent,
                bytes: Optional::Absent,
                tokenizer_id: Optional::Absent,
            };
        }
        "omissions" => reduced.omissions = Vec::new(),
        "warnings" => reduced.warnings = Vec::new(),
        "artifacts" => reduced.artifacts = Vec::new(),
        _ => {}
    }
    reduced
}

/// Every field this decomposition reduces, with its classification.
///
/// The classification is what makes the number actionable rather than merely large.
/// `epochs` is the only field RFC 0026 requires on every answer that is also *identical* on
/// every answer of one connection — all six are pinned at the handshake and never move
/// during a run — so it is the one conformance cost a redesign can amortize without
/// dropping a guarantee. `next_operations` need not travel at all: RFC 0026 declares the
/// list required, not non-empty, and this daemon populates none. Everything else varies per
/// answer and is what the answer *is*.
pub const DECOMPOSED_FIELDS: &[(&str, &str)] = &[
    ("artifacts", CONFORMANCE_VARYING),
    ("assurance", CONFORMANCE_VARYING),
    ("cost", CONFORMANCE_VARYING),
    ("epochs", CONFORMANCE_CONSTANT),
    ("next_operations", AMORTIZABLE),
    ("omissions", CONFORMANCE_VARYING),
    ("warnings", CONFORMANCE_VARYING),
];

/// Decompose every answer the matrix produced.
///
/// The answers are the daemon's own, collected on the arm that can hold them: the CLI
/// process receives the identical `ResultEnvelope` the typed client receives, because both
/// arms drive one daemon over one wire with one set of arguments. The two frames differ in
/// the length of the `request_id` string and in nothing else, and
/// `the_decomposed_answer_re_encodes_to_the_frame_that_carried_it` holds the re-encoding to
/// the frame it came from.
///
/// # Errors
///
/// [`SurfaceError`] as [`shell_sweep`].
pub fn decompose() -> Result<Decomposition, SurfaceError> {
    let cells = shell_sweep(Renderer::Standard, Disciplines::ALL, true)?;
    let mut answers = 0;
    let mut result_bytes = 0;
    let mut request_bytes = 0;
    let mut per_field: BTreeMap<&'static str, u64> = BTreeMap::new();
    let mut floor_bytes = 0;
    let mut snapshot_component_bytes = 0;
    let mut snapshot_component_calls = 0;
    let mut request_by_operation: BTreeMap<&'static str, (u32, u64)> = BTreeMap::new();

    for cell in &cells {
        for recorded in &cell.recorded {
            answers += 1;
            request_bytes += recorded.sent;
            let entry = request_by_operation.entry(recorded.operation).or_default();
            entry.0 += 1;
            entry.1 += recorded.sent;
            let whole =
                to_bytes(&recorded.envelope).map_or(recorded.received, |bytes| bytes.len() as u64);
            result_bytes += whole;
            let mut floor = recorded.envelope.clone();
            for (field, _) in DECOMPOSED_FIELDS {
                let reduced = reduce(&recorded.envelope, field);
                let smaller = to_bytes(&reduced).map_or(whole, |bytes| bytes.len() as u64);
                *per_field.entry(field).or_default() += whole.saturating_sub(smaller);
                floor = reduce(&floor, field);
            }
            floor_bytes += to_bytes(&floor).map_or(whole, |bytes| bytes.len() as u64);
            if recorded.operation == "workspace.create" {
                snapshot_component_calls += 1;
                snapshot_component_bytes += recorded.argument_bytes;
            }
        }
    }

    let fields = DECOMPOSED_FIELDS
        .iter()
        .map(|(field, class)| FieldCost {
            field,
            class,
            bytes: per_field.get(field).copied().unwrap_or_default(),
        })
        .collect();

    Ok(Decomposition {
        answers,
        result_bytes,
        request_bytes,
        fields,
        floor_bytes,
        snapshot_component_bytes,
        snapshot_component_calls,
        request_by_operation,
    })
}

// --- the mistake class only one surface can express ---------------------------------------

/// Every `--flag` a command line writes: one place a misnamed argument can land.
///
/// The typed client's count is structurally zero — an argument's name is a struct field,
/// resolved by the compiler, and there is no call through which a caller can offer a
/// different one. That asymmetry has no representation in the landed instrument, because
/// the landed instrument's load-bearing fairness rule is that both arms attempt the
/// identical operation sequence, and a mistake only one surface can express is not in any
/// sequence both can attempt.
#[must_use]
pub fn flag_names(command: &str) -> Vec<&str> {
    command
        .split_whitespace()
        .filter(|token| token.starts_with("--"))
        .collect()
}

/// What a CLI prints when an argument name is not one it knows.
///
/// Derived from the command rather than invented: the unknown flag, the nearest known one,
/// and the operation's own usage line. A typed client has no analogue of any of it.
#[must_use]
pub fn unknown_argument_usage(command: &str, offered: &str, meant: &str) -> String {
    let usage = command
        .split_whitespace()
        .take(3)
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "error: unexpected argument '{offered}' found\n\n  tip: a similar argument exists: \
         '{meant}'\n\nUsage: {usage} {meant} <VALUE>\n\nFor more information, try '--help'.\n"
    )
}

/// What one misnamed argument costs the baseline: the command it wrote, plus the usage it
/// read back.
///
/// No daemon sees it — a CLI refuses an unknown flag in its own argument parser, exactly as
/// the typed client's register refuses an operation whose preconditions do not hold. The
/// two local refusals are the fair comparison, and the difference between them is that one
/// arm *has* the mistake available to make.
#[must_use]
pub fn misnamed_argument_cost(command: &str) -> u64 {
    let Some(meant) = flag_names(command).first().copied() else {
        return 0;
    };
    let offered = format!("--{}", &meant[3..]);
    let mistaken = command.replacen(meant, &offered, 1);
    mistaken.len() as u64 + unknown_argument_usage(command, &offered, meant).len() as u64
}

/// The same runs, with `per_run` shell-only invalid attempts added to each.
///
/// Every added attempt is invalid, spends `unit_bytes`, and reaches no daemon — a CLI
/// refusing an argument name it does not know, which is the local refusal that answers the
/// typed client's own. Nothing else about a run changes: it solved what it solved.
#[must_use]
pub fn with_shell_only_mistakes(runs: &[ArmRun], per_run: u32, unit_bytes: u64) -> Vec<ArmRun> {
    runs.iter()
        .map(|run| {
            let mut run = run.clone();
            run.attempted += per_run;
            run.invalid += per_run;
            run.bytes += u64::from(per_run) * unit_bytes;
            run
        })
        .collect()
}

/// The smallest number of shell-only invalid attempts *per run* at which the ratified 50%
/// relative reduction in invalid-action rate clears.
///
/// Reported in tenths, because the answer is below one and rounding it to an integer would
/// turn "about six mistakes in ten runs" into either "none" or "one whole mistake every
/// run". The typed arm is unchanged; the baseline gains `k/10` attempts per run, all of
/// them invalid, which is what a mistake class only it can express does to the metric.
#[must_use]
pub fn invalid_reduction_breakeven_tenths(native: &ArmTotals, shell: &ArmTotals) -> Option<u32> {
    for tenths in 0..=1000 {
        let extra = (u64::from(shell.runs) * u64::from(tenths)) / 10;
        let attempted = u64::from(shell.attempted) + extra;
        let invalid = u64::from(shell.invalid) + extra;
        if attempted == 0 {
            continue;
        }
        let shell_permille = (invalid * 1000 / attempted) as i64;
        if shell_permille == 0 {
            continue;
        }
        let reduction = ((shell_permille - native.invalid_permille()) * 100) / shell_permille;
        if reduction >= RATIFIED.invalid_reduction_percent {
            return Some(tenths);
        }
    }
    None
}

// --- the completion criterion --------------------------------------------------------------

/// Re-score a run with the omission-manifest clause removed from completion.
///
/// `AgentView::complete` requires `ceiling_enforced`, which is the one datum the typed arm
/// receives inline and the baseline pays an expansion command for. That clause is the
/// instrument's single deliberate concession to the typed arm, and the honest way to price
/// it is to ask what the numbers are without it — which is what this does, by re-reading
/// the run's own recorded readings rather than by re-running anything.
///
/// A run is `ceiling-free solved` when it read the frozen state count and the frozen
/// verdict, whatever it knew about metering. Nothing else about the run changes.
#[must_use]
pub fn ceiling_free_completion(run: &ArmRun, task: &BenchmarkTask) -> bool {
    run.states_read == Some(task.expected.states) && run.verdict_read == Some(task.expected.verdict)
}

// --- the task set ----------------------------------------------------------------------------

/// Every leave-one-out subset of [`SUBSET`], each named by the task it drops.
#[must_use]
pub fn leave_one_out() -> Vec<(&'static str, Vec<BenchmarkTask>)> {
    SUBSET
        .iter()
        .map(|dropped| {
            (
                dropped.id,
                SUBSET
                    .iter()
                    .filter(|task| task.id != dropped.id)
                    .copied()
                    .collect(),
            )
        })
        .collect()
}

/// Keep only the runs whose task is in `subset`.
#[must_use]
pub fn restrict(runs: &[ArmRun], subset: &[BenchmarkTask]) -> Vec<ArmRun> {
    runs.iter()
        .filter(|run| subset.iter().any(|task| task.id == run.task))
        .cloned()
        .collect()
}

/// Split a mixed-arm run list into the two arms, preserving order.
#[must_use]
pub fn split_arms(runs: &[ArmRun]) -> (Vec<ArmRun>, Vec<ArmRun>) {
    let native = runs
        .iter()
        .filter(|run| run.arm == Arm::Native)
        .cloned()
        .collect();
    let shell = runs
        .iter()
        .filter(|run| run.arm == Arm::Shell)
        .cloned()
        .collect();
    (native, shell)
}
