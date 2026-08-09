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

// --- C7: what an enforced `OutputPolicy` ceiling is actually worth --------------------------

/// What a stated byte ceiling on `task.status` costs and saves, measured over the matrix.
///
/// The row `DX10_BYTE_LEDGER.md` §3 C7 projected at **+27,376 B over the matrix, 1,114 B per
/// solved task, "behavior-only, no bump"**, re-measured against the mechanism that was
/// actually admissible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CeilingProjection {
    /// `task.status` answers the matrix produced.
    pub polls: u32,
    /// Their payloads, summed, untrimmed.
    pub payload_bytes: u64,
    /// Their result frames, summed, untrimmed.
    pub frame_bytes: u64,
    /// The best interface-byte saving any stated ceiling reaches, summed over the polls.
    ///
    /// **Per-poll best**, which no real client could achieve: the optimum is taken
    /// independently for each answer, as if the caller had known that answer's sizes before
    /// asking. It is therefore an *upper bound* on what the mechanism is worth, and it is
    /// the bound this measurement exists to report.
    pub best_saving: i64,
    /// The saving at the tightest conforming ceiling — the summary a client that reads only
    /// `task`, `status`, `cost` and `continuation` would ask for.
    pub floor_saving: i64,
    /// What declaring the ceiling costs on the request frame, summed over the polls.
    pub declaration_bytes: u64,
    /// INV-007 trim records the best case produced.
    pub trims: u32,
}

impl CeilingProjection {
    /// The best saving per solved task, over the matrix's 24 solved tasks.
    #[must_use]
    pub const fn best_per_solved(&self) -> i64 {
        self.best_saving / 24
    }
}

/// What `"output_policy":{"max_bytes":<n>}` adds to a request frame, measured.
///
/// Measured rather than counted by hand: one request envelope is encoded with the member and
/// without it, and the difference is the member's cost. Canonical JSON's key order and
/// separators make that difference independent of the rest of the envelope, so one
/// measurement is the cost on every frame that carries the same `n`.
#[must_use]
pub fn ceiling_declaration_bytes(max_bytes: u64) -> u64 {
    use continuumd::protocol::envelope::{OutputPolicy, RequestEnvelope};
    use continuumd::protocol::scalar::{
        ActorId, ByteCount, CapabilityHandle, Opaque, OperationName, ProtocolVersion, RequestId,
    };

    let bare = RequestEnvelope {
        protocol_version: ProtocolVersion::new(3, 4),
        request_id: RequestId::new("req_1").expect("a request id"),
        idempotency_key: Optional::Absent,
        actor: ActorId::new("agent:reader").expect("an actor"),
        capability: CapabilityHandle::new("cap_reader").expect("a capability"),
        operation: OperationName::new("task.status").expect("a name"),
        snapshot: Nullable::Null,
        intent: Nullable::Null,
        arguments: Opaque::from_bytes(b"{}".to_vec()),
        budget: Optional::Absent,
        output_policy: Optional::Absent,
        trace: Optional::Absent,
        page: Optional::Absent,
    };
    let mut bounded = bare.clone();
    bounded.output_policy = Optional::Present(OutputPolicy {
        max_bytes: Optional::Present(ByteCount::new(max_bytes)),
        max_tokens: Optional::Absent,
        max_nodes: Optional::Absent,
        audience: Optional::Absent,
    });
    let before = to_bytes(&bare).map_or(0, |bytes| bytes.len() as u64);
    let after = to_bytes(&bounded).map_or(0, |bytes| bytes.len() as u64);
    after.saturating_sub(before)
}

/// Re-measure C7 against the mechanism `continuumd::daemon::output` actually delivers.
///
/// # Method
///
/// The recorded answers are the daemon's own (`shell_sweep` in recording mode), and the
/// transform is the **landed** one: [`continuumd::daemon::output::fit_task_record`], called
/// here exactly as `task.status` calls it. Nothing is modelled. For each poll the four
/// answers the mechanism can produce — the whole record, and the record with each prefix of
/// the declared elision order taken — are built, the envelope is rebuilt around each with the
/// INV-007 records the mechanism emits, the whole frame is re-encoded, and the request-side
/// cost of stating the ceiling is charged against the result-side saving.
///
/// # What it finds, and why the number matters
///
/// C7 was commissioned as "behavior-only, no bump" worth 1,114 B per solved task. The saving
/// it projected comes from eliding `operation`, `snapshot`, `intent`, `epochs`,
/// `priority_class` and `budget` from the answer — and `TaskRecord` declares five of those
/// `required` and two `nullable`, so eliding them changes a presence marker, which
/// `rule versioning.breaking_change` makes a **major** change. Not behavior-only, and not
/// eligible for the bundled 3.6 minor either.
///
/// What the declaration *does* permit a ceiling to take is the contents of two `required`
/// lists and the nine `optional` members of `Budget`. That is what this function measures,
/// and it comes out **negative**: a conforming INV-007 omission carrying `recoverable_by`
/// costs more than the members the declaration permits eliding are worth. Populating the
/// retrieval half — the thing C7 existed to do first — is what makes the trade lose.
///
/// # Errors
///
/// [`SurfaceError`] as [`shell_sweep`].
pub fn ceiling_projection() -> Result<CeilingProjection, SurfaceError> {
    use continuumd::codec::operations::{decode_payload, encode_payload};
    use continuumd::daemon::family::Payload;
    use continuumd::daemon::output::{self, Ceiling, TASK_RECORD_ELISION_ORDER};
    use continuumd::protocol::vocabulary::Encoding;

    let cells = shell_sweep(Renderer::Standard, Disciplines::ALL, true)?;
    let mut projection = CeilingProjection {
        polls: 0,
        payload_bytes: 0,
        frame_bytes: 0,
        best_saving: 0,
        floor_saving: 0,
        declaration_bytes: 0,
        trims: 0,
    };

    for cell in &cells {
        for recorded in &cell.recorded {
            if recorded.operation != "task.status" {
                continue;
            }
            let Nullable::Value(carried) = &recorded.envelope.payload else {
                continue;
            };
            let Ok(Payload::TaskStatus(record)) = decode_payload("task.status", carried) else {
                continue;
            };
            projection.polls += 1;
            projection.payload_bytes += carried.as_bytes().len() as u64;
            let whole = to_bytes(&recorded.envelope).map_or(0, |bytes| bytes.len() as u64);
            projection.frame_bytes += whole;

            // The ceilings worth trying are exactly the sizes of the four answers the
            // mechanism can produce: asking for less than an answer's size is asking for the
            // next one down, and asking for more is asking for the same one.
            let mut best: Option<(i64, u32)> = None;
            let mut floor_saving = 0;
            let mut floor_declaration = 0;
            for depth in 0..=TASK_RECORD_ELISION_ORDER.len() {
                let mut candidate = record.clone();
                for subject in TASK_RECORD_ELISION_ORDER.iter().take(depth) {
                    take(&mut candidate, subject);
                }
                let Ok(ceiling) = output::measure(&candidate, Encoding::CanonicalJson) else {
                    continue;
                };
                let Ok(fitted) = output::fit_task_record(
                    record.clone(),
                    Ceiling::of_bytes(ceiling),
                    Encoding::CanonicalJson,
                ) else {
                    continue;
                };
                let Ok(Some(payload)) = encode_payload(&Payload::TaskStatus(fitted.record)) else {
                    continue;
                };
                let mut envelope = recorded.envelope.clone();
                envelope.payload = Nullable::Value(payload);
                envelope.omissions.extend(fitted.omissions.iter().cloned());
                let after = to_bytes(&envelope).map_or(whole, |bytes| bytes.len() as u64);
                // The declaration is charged at every depth, including the one that trims
                // nothing: a caller that states a ceiling pays for the member whatever the
                // ceiling turns out to admit. The alternative — stating none — is the zero
                // this whole projection is compared against.
                let declaration = ceiling_declaration_bytes(ceiling);
                let saving = whole as i64 - after as i64 - declaration as i64;
                if depth == TASK_RECORD_ELISION_ORDER.len() {
                    floor_saving = saving;
                    floor_declaration = declaration;
                }
                if best.is_none_or(|(current, _)| saving > current) {
                    best = Some((saving, u32::try_from(fitted.omissions.len()).unwrap_or(0)));
                }
            }
            let (saving, trims) = best.unwrap_or((0, 0));
            projection.best_saving += saving;
            projection.floor_saving += floor_saving;
            projection.declaration_bytes += floor_declaration;
            projection.trims += trims;
        }
    }
    Ok(projection)
}

/// The elision the daemon's declared order names, applied here to build a candidate ceiling.
///
/// A second spelling of `continuumd::daemon::output::elide`, which is private to the daemon
/// because nothing outside it decides what a ceiling may take. This one only *chooses a
/// ceiling to ask for*; the answer is always built by the daemon's own function, so the two
/// cannot drift into disagreeing about the wire.
fn take(record: &mut continuumd::protocol::task::TaskRecord, subject: &str) {
    use continuumd::protocol::envelope::Budget;

    match subject {
        "task.milestones" => record.milestones = Vec::new(),
        "task.committed_evidence" => record.committed_evidence = Vec::new(),
        "task.budget" => {
            record.budget = Budget {
                wall_ms: Optional::Absent,
                cpu_ms: Optional::Absent,
                memory_bytes: Optional::Absent,
                states: Optional::Absent,
                solver_ms: Optional::Absent,
                proof_ms: Optional::Absent,
                tokens: Optional::Absent,
                candidates: Optional::Absent,
                bytes: Optional::Absent,
            };
        }
        _ => {}
    }
}

// --- C5: what pinning the epoch set at the handshake is actually worth, and when ------------

/// What the three `required` envelope members C5 would suppress cost, measured over the matrix.
///
/// The row `DX10_BYTE_LEDGER.md` §3 C5 projected at **27,864 B + 5,160 B of key skeleton,
/// 1,344 B per solved task, "declaration-moving, bundled 3.6"**, re-measured — both for the
/// size of the prize and, the part the projection did not check, for *which protocol version
/// can collect it*.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EpochProjection {
    /// Answers the matrix produced.
    pub answers: u32,
    /// Their result frames, summed, as landed.
    pub frame_bytes: u64,
    /// Distinct `epochs` encodings across those answers.
    pub distinct_epoch_sets: u32,
    /// Answers whose `epochs` is not the set `ServerWelcome` pinned.
    ///
    /// The premise of the whole candidate: absence can only mean "the set the welcome pinned"
    /// if the welcome pinned the set the answers carry. Zero here is the premise holding.
    pub answers_unpinned_by_the_welcome: u32,
    /// Bytes that removing `epochs` from every result frame would save.
    pub epochs_absent_bytes: u64,
    /// Bytes that removing `cost` from every result frame would save.
    pub cost_absent_bytes: u64,
    /// Bytes that removing `next_operations` from every result frame would save.
    pub next_operations_absent_bytes: u64,
    /// Answers whose `cost` carried no dimension at all.
    pub empty_cost: u32,
    /// Answers whose `next_operations` offered nothing.
    pub empty_next_operations: u32,
    /// What reducing `epochs` to its protocol member and six explicit nulls would save.
    ///
    /// The figure the landed instrument credits. It moves no presence marker — every key still
    /// ships — and it is nonetheless **unavailable**, because six of those nulls would be the
    /// daemon denying it can pin an epoch it can pin, which `rule envelope.epochs_named`
    /// spends its second sentence forbidding.
    pub protocol_only_bytes: u64,
    /// What C5 is worth at 3.5, moving no presence marker and stating nothing false.
    ///
    /// Measured, not asserted: for each of the three members, the smallest value the
    /// declaration admits *and the daemon can truthfully emit*, against what it emits today.
    pub available_without_a_bump: u64,
}

impl EpochProjection {
    /// The full-suppression saving per solved task, over the matrix's 24 solved tasks.
    #[must_use]
    pub const fn per_solved(&self) -> u64 {
        (self.epochs_absent_bytes + self.cost_absent_bytes + self.next_operations_absent_bytes) / 24
    }
}

/// Re-measure C5 against the declaration the wire actually has.
///
/// # Method
///
/// The recorded answers are the daemon's own (`shell_sweep` in recording mode), re-encoded
/// with `continuumd`'s codec — the ledger's own method. The suppression itself cannot be
/// performed by building a value, because `ResultEnvelope` declares all three members
/// `required` and [`protocol_struct!`](continuumd::protocol_struct) gives a `required` field a
/// plain Rust type with no absent state: the codec's `required` arm writes the key
/// unconditionally. So the counterfactual is built by **surgery on the canonical JSON**, by
/// [`without_member`], exactly as C7's was. That is not a workaround; it is the finding, and
/// [`EpochProjection::available_without_a_bump`] is what remains once it is excluded.
///
/// # Errors
///
/// [`SurfaceError`] as [`shell_sweep`].
pub fn epoch_projection() -> Result<EpochProjection, SurfaceError> {
    let cells = shell_sweep(Renderer::Standard, Disciplines::ALL, true)?;
    let welcome = crate::rig::epochs();
    let mut sets: BTreeMap<Vec<u8>, u32> = BTreeMap::new();
    let mut projection = EpochProjection {
        answers: 0,
        frame_bytes: 0,
        distinct_epoch_sets: 0,
        answers_unpinned_by_the_welcome: 0,
        epochs_absent_bytes: 0,
        cost_absent_bytes: 0,
        next_operations_absent_bytes: 0,
        empty_cost: 0,
        empty_next_operations: 0,
        protocol_only_bytes: 0,
        available_without_a_bump: 0,
    };

    for cell in &cells {
        for recorded in &cell.recorded {
            let Ok(frame) = to_bytes(&recorded.envelope) else {
                continue;
            };
            let Ok(text) = String::from_utf8(frame) else {
                continue;
            };
            projection.answers += 1;
            projection.frame_bytes += text.len() as u64;

            if recorded.envelope.epochs != welcome {
                projection.answers_unpinned_by_the_welcome += 1;
            }
            if let Ok(encoded) = to_bytes(&recorded.envelope.epochs) {
                *sets.entry(encoded).or_insert(0) += 1;
            }

            for (member, into) in [
                ("epochs", &mut projection.epochs_absent_bytes),
                ("cost", &mut projection.cost_absent_bytes),
                (
                    "next_operations",
                    &mut projection.next_operations_absent_bytes,
                ),
            ] {
                if let Some(stripped) = without_member(&text, member) {
                    *into += text.len().saturating_sub(stripped.len()) as u64;
                }
            }

            if recorded.envelope.cost == EMPTY_COST {
                projection.empty_cost += 1;
            }
            if recorded.envelope.next_operations.is_empty() {
                projection.empty_next_operations += 1;
            }

            // The value-only counterfactual: every key still on the wire, every epoch the
            // daemon can pin denied. Priced here so the credit the landed instrument takes has
            // a measured size beside the reason it cannot be collected.
            let mut denied = recorded.envelope.clone();
            denied.epochs = EpochSet {
                protocol: recorded.envelope.epochs.protocol,
                semantic: Nullable::Null,
                intent: Nullable::Null,
                evidence: Nullable::Null,
                proof: Nullable::Null,
                corpus: Nullable::Null,
                engine: Nullable::Null,
            };
            if let Ok(smaller) = to_bytes(&denied) {
                projection.protocol_only_bytes += text.len().saturating_sub(smaller.len()) as u64;
            }

            projection.available_without_a_bump += truthful_saving(&recorded.envelope, &text);
        }
    }
    projection.distinct_epoch_sets = u32::try_from(sets.len()).unwrap_or(u32::MAX);
    Ok(projection)
}

/// The bytes a **3.5** daemon can still take out of one answer's `epochs`, `cost` and
/// `next_operations` — moving no presence marker, and stating nothing that is not so.
///
/// Each member is replaced by the smallest value its declaration admits, and the replacement
/// is kept only where it changes no statement the answer makes. That is a real per-answer
/// test, not a constant: an answer that shipped a pinnable epoch as a redundant value, or an
/// offer list padded with an offer it did not mean, would show up here as bytes. What it
/// finds on this matrix is reported by [`EpochProjection::available_without_a_bump`].
///
/// Why each replacement is conditional:
///
/// - `epochs` — a nullable member of `EpochSet` is not a spelling choice. `rule
///   envelope.epochs_named` reads "an epoch the result cannot pin reads null", so writing
///   null over an epoch the daemon *can* pin states something false about the daemon, and
///   RFC 0027's Safety section prohibits exactly that trade ("hiding uncertainty to save
///   tokens"). The replacement is therefore admissible only where the member is already null.
/// - `cost` — its nine dimensions are `optional`, so an unspent dimension is already absent.
///   Emptying a `Cost` that reported a spend would delete the spend, not compress it.
/// - `next_operations` — an offer not made is already an empty list.
fn truthful_saving(envelope: &ResultEnvelope, today: &str) -> u64 {
    let mut smallest = envelope.clone();
    let pinned = &envelope.epochs;
    if pinned.semantic.is_null()
        && pinned.intent.is_null()
        && pinned.evidence.is_null()
        && pinned.proof.is_null()
        && pinned.corpus.is_null()
        && pinned.engine.is_null()
    {
        smallest.epochs = EpochSet {
            protocol: pinned.protocol,
            semantic: Nullable::Null,
            intent: Nullable::Null,
            evidence: Nullable::Null,
            proof: Nullable::Null,
            corpus: Nullable::Null,
            engine: Nullable::Null,
        };
    }
    if envelope.cost == EMPTY_COST {
        smallest.cost = EMPTY_COST;
    }
    if envelope.next_operations.is_empty() {
        smallest.next_operations = Vec::new();
    }
    to_bytes(&smallest).map_or(0, |bytes| today.len().saturating_sub(bytes.len()) as u64)
}

/// A `Cost` carrying no dimension: the value the matrix's answers actually hold.
const EMPTY_COST: Cost = Cost {
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

/// One canonical-JSON object with a top-level member removed, or `None` if it has no such
/// member.
///
/// # Why surgery and not a value
///
/// `ResultEnvelope.epochs`, `.cost` and `.next_operations` are `required`, so the generated
/// Rust types are `EpochSet`, `Cost` and `Vec<NextOperation>` — none of which has an absent
/// state — and the generated codec's `required` arm writes the key whatever the value is.
/// A conforming daemon *cannot* emit these frames, and this function is how a measurement
/// prices something the type system is right to refuse to build.
///
/// The scan is a small canonical-JSON walker rather than a regex: a value may contain the
/// member's own name inside a string, and a brace inside a string is not a brace. It removes
/// the member and exactly one separator, which is what absence costs.
#[must_use]
pub fn without_member(object: &str, member: &str) -> Option<String> {
    let bytes = object.as_bytes();
    let key = format!("\"{member}\":");
    let mut at = 1; // past the opening brace
    while at < bytes.len() {
        if bytes[at] == b'}' {
            return None;
        }
        let start = at;
        if !object[at..].starts_with('"') {
            return None;
        }
        let key_end = string_end(bytes, at)?;
        let matched = object[at..].starts_with(&key);
        at = key_end;
        if bytes.get(at) != Some(&b':') {
            return None;
        }
        at += 1;
        at = value_end(bytes, at)?;
        if matched {
            // Take the separator that binds this member to a neighbour: the following comma
            // when there is one, otherwise the preceding one.
            let (from, to) = if bytes.get(at) == Some(&b',') {
                (start, at + 1)
            } else if start > 1 {
                (start - 1, at)
            } else {
                (start, at)
            };
            let mut out = String::with_capacity(object.len());
            out.push_str(&object[..from]);
            out.push_str(&object[to..]);
            return Some(out);
        }
        match bytes.get(at) {
            Some(b',') => at += 1,
            Some(b'}') => return None,
            _ => return None,
        }
    }
    None
}

/// The index one past a JSON string starting at `at`.
fn string_end(bytes: &[u8], at: usize) -> Option<usize> {
    let mut index = at + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'"' => return Some(index + 1),
            _ => index += 1,
        }
    }
    None
}

/// The index one past the JSON value starting at `at`.
fn value_end(bytes: &[u8], at: usize) -> Option<usize> {
    let mut index = at;
    let mut depth = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => index = string_end(bytes, index)?,
            b'{' | b'[' => {
                depth += 1;
                index += 1;
            }
            b'}' | b']' => {
                if depth == 0 {
                    return Some(index);
                }
                depth -= 1;
                index += 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            b',' if depth == 0 => return Some(index),
            _ => index += 1,
        }
    }
    None
}
