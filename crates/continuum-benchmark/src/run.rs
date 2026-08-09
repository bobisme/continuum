//! The runner, and the five metric definitions it computes.
//!
//! # One loop, both arms
//!
//! [`drive`] is the only place a run happens. It builds a fresh [`Rig`], asks the [`Policy`]
//! for a step, hands the step to a [`Surface`], folds what came back into the
//! [`AgentView`], and repeats. Attempt counting, admission classification, byte accounting,
//! completion, and the transcript are all computed *here*, from the [`Observation`] the
//! surface returned — never inside an arm. An arm decides which method to call and what it
//! could read; it does not decide how it is scored.
//!
//! # The five metrics, defined exactly
//!
//! ## IMPL-01 — valid operation rate
//!
//! ```text
//! valid_operation_rate = admitted / attempted
//! ```
//!
//! An **attempt** is one operation the policy decided to make. An **admission** is a result
//! envelope whose `status` is not `error` — `ok`, `task_started` and `task_suspended` are all
//! admissions, because a suspended task is a budget outcome carrying a continuation and not a
//! rejected request (docs/49, RFC 0026).
//!
//! Two counting rules are worth stating because they are where a benchmark of this shape
//! usually cheats:
//!
//! - **A locally refused operation counts as an attempt and as an invalid one.** The typed
//!   client can refuse an operation whose preconditions do not hold before writing a frame
//!   (research/25's "low-cost local feedback"). That saves *bytes*, not *validity*, and
//!   [`ArmRun::zero_cost_invalid`] is reported separately so the saving is visible without
//!   being laundered into the rate.
//! - **Both arms run the same policy**, so the *number of mistakes* is fixed by construction.
//!   What the arms can differ on is whether an interface turns a correct decision into an
//!   invalid operation — by handing back an answer the agent cannot use.
//!
//! ## IMPL-02 — tokens/bytes
//!
//! ```text
//! interface_bytes = handshake_bytes + sum over attempts of (written + read)
//! approx_tokens   = ceil(interface_bytes / BYTES_PER_APPROX_TOKEN)
//! cost            = interface_bytes per *solved* task
//! ```
//!
//! **Bytes are the graded denominator, not tokens.** RFC 0027 rejects token-denominated
//! enforcement as model-relative and plan §24.5's ratified ACI margins repeat it: "at least
//! 30% fewer interface bytes per solved task (bytes, not tokens, are the graded cost
//! denominator per RFC 0027)". [`ArmRun::approx_tokens`] is reported because START_HERE's
//! bullet says "tokens/bytes" and dropping the word would be quietly narrowing the bullet; it
//! is computed by a **declared arithmetic rule** over the same bytes, with no tokenizer, no
//! vocabulary, and no model. It is an affine restatement of the byte count and nothing is
//! graded on it. RFC 0026's `Cost.tokenizer_id` — "REQUIRED whenever `tokens` is reported" —
//! is why this crate reports the rule's name beside the number and never puts it on a wire.
//!
//! What counts as an interface byte differs by arm because the interfaces differ, and the
//! difference is deliberately in the baseline's favour:
//!
//! | Arm | Written | Read | Not counted |
//! |---|---|---|---|
//! | native | the request frame | the result frame | — |
//! | shell | the command line | the rendered output | the CLI's own frames |
//!
//! A real `continuum` process exchanges protocol frames the agent never sees, so charging
//! the shell arm for them would be charging it for bytes that are not its interface. The
//! consequence is that the native arm pays for its whole envelope and the shell arm pays for
//! a summary of it, which is the honest boundary and a handicap this instrument accepts
//! rather than hides.
//!
//! ## IMPL-03 — task completion
//!
//! A run is **solved** when the agent read the frozen answer *and it was the right one*:
//!
//! ```text
//! solved = view.complete() && states == expected.states && verdict == expected.verdict
//! ```
//!
//! [`AgentView::complete`] is the agent-side half — settled, verdict read, states read,
//! ceiling-enforcement known — and the equality against [`Frozen`](crate::task::Frozen) is
//! the grader's. An arm that finished its script without recovering the two numbers is not
//! solved, and an arm that recovered numbers that disagree with the dossier is not solved
//! either.
//!
//! ## IMPL-04 — recovery from errors
//!
//! ```text
//! recovered = faults_fired > 0 && solved
//! ```
//!
//! The fallible policy makes the schedule's mistakes ([`Fault`]), each one against an error
//! surface the daemon already has. Recovery is measured as reaching the same frozen answer
//! anyway, and its *cost* is the difference in attempts and bytes against the faithful run of
//! the same `(task, seed, arm)` — which is why both policies run over the same matrix.
//!
//! ## IMPL-05 — deterministic reproduction
//!
//! Two runs of the same `(task, seed, policy, arm)`, each against a **freshly built** rig,
//! must produce a byte-identical transcript and identical metrics. Nothing is shared between
//! the two runs but the script, so a daemon-instance-specific fact — a map's iteration order,
//! an address, a counter that survived — cannot hide in the agreement.
//!
//! [`Fault`]: crate::policy::Fault

use std::collections::BTreeMap;

use crate::policy::{AgentView, Decision, Fault, Policy, PolicyKind, SCHEDULES, Step, Stuck};
use crate::rig::Rig;
use crate::separation;
use crate::shell::{ShellSurface, transcript_line};
use crate::surface::{Arm, Surface, SurfaceError};
use crate::task::{BenchmarkTask, SUBSET};

/// The declared bytes-per-approximate-token rule.
///
/// Four, the ratio the field uses for English-and-JSON text, declared as a constant so the
/// number this crate reports is reproducible without a tokenizer. It is a *rule*, not a
/// measurement: see this module's IMPL-02 section for why nothing is graded on it.
pub const BYTES_PER_APPROX_TOKEN: u64 = 4;

/// The name of that rule, reported beside every token count.
pub const APPROX_TOKEN_RULE: &str = "bytes/4 (declared; no tokenizer, not graded)";

/// One arm's run of one `(task, seed, policy)` cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArmRun {
    /// Which arm.
    pub arm: Arm,
    /// Which task.
    pub task: &'static str,
    /// Which seed's schedule.
    pub seed: u16,
    /// Faithful or fallible.
    pub policy: PolicyKind,
    /// Operations attempted.
    pub attempted: u32,
    /// Operations the daemon admitted.
    pub admitted: u32,
    /// Attempts that were not admitted.
    pub invalid: u32,
    /// Invalid attempts that spent no interface bytes, because the typed surface refused
    /// them before the wire. A subset of `invalid`, reported beside it and never subtracted
    /// from it.
    pub zero_cost_invalid: u32,
    /// Interface bytes, including the handshake.
    pub bytes: u64,
    /// The declared-rule token approximation of `bytes`.
    pub approx_tokens: u64,
    /// Whether the run reached the frozen answer.
    pub solved: bool,
    /// Why the run stopped, when it did not finish its script.
    pub stuck: Option<Stuck>,
    /// The faults the policy fired.
    pub faults: Vec<Fault>,
    /// The fallible-policy family mistakes the policy decided on, in order.
    ///
    /// Identical on both arms by construction — the policy is shared and the mistakes are
    /// decided above the surface seam — so a difference between the arms is a difference in
    /// what the surface could *express*, never in what the agent tried to do. Empty on every
    /// landed run.
    pub mistakes: Vec<crate::families::MistakeClass>,
    /// Mistakes this surface had no channel for, and therefore never attempted.
    ///
    /// Counted separately from `invalid` and never folded into it: see
    /// [`crate::surface::Observation::unrepresentable`]. Zero on every landed run.
    pub prevented: u32,
    /// Whether a run that made mistakes reached the frozen answer anyway.
    pub recovered: bool,
    /// The reachable-state count the arm read, when it read one.
    pub states_read: Option<u64>,
    /// The verdict the arm read, when it read one.
    pub verdict_read: Option<continuumd::protocol::vocabulary::SemanticVerdict>,
    /// The canonical transcript: one line per attempt.
    pub transcript: Vec<String>,
    /// Calls and interface bytes, per wire operation.
    ///
    /// The diagnostic half of IMPL-02. A single "bytes per solved task" number says which
    /// arm is cheaper; this says *where* the difference is, which is what a redesign decision
    /// needs — and a redesign is an explicitly valid exit for this lane (the goal bone's
    /// criterion 1). Keyed by the wire operation name, so the breakdown is comparable across
    /// arms without either arm naming its own categories.
    pub per_operation: BTreeMap<&'static str, OperationCost>,
}

/// Calls and interface bytes spent on one wire operation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OperationCost {
    /// How many times the operation was attempted.
    pub calls: u32,
    /// Interface bytes those attempts spent.
    pub bytes: u64,
}

impl ArmRun {
    /// The valid operation rate, in per-mille.
    ///
    /// Integer per-mille rather than a float, because the report is compared byte for byte
    /// and a float's rendering is a portability question nobody should have to answer to read
    /// a benchmark. Zero attempts reads zero rather than dividing.
    #[must_use]
    pub const fn valid_rate_permille(&self) -> u64 {
        if self.attempted == 0 {
            return 0;
        }
        (self.admitted as u64 * 1000) / self.attempted as u64
    }
}

/// Drive one `(task, seed, policy)` cell on one arm.
///
/// # Errors
///
/// [`SurfaceError`] when an arm could not attempt an operation at all, which means this crate
/// is wrong rather than that an arm lost.
pub fn drive(
    surface: &mut dyn Surface,
    rig: &mut Rig,
    task: &BenchmarkTask,
    policy: Policy,
) -> Result<ArmRun, SurfaceError> {
    let mut view = AgentView::default();
    let mut run = ArmRun {
        arm: surface.arm(),
        task: task.id,
        seed: policy.schedule.seed,
        policy: policy.kind,
        attempted: 0,
        admitted: 0,
        invalid: 0,
        zero_cost_invalid: 0,
        bytes: surface.handshake_bytes(),
        approx_tokens: 0,
        solved: false,
        stuck: None,
        faults: Vec::new(),
        mistakes: Vec::new(),
        prevented: 0,
        recovered: false,
        states_read: None,
        verdict_read: None,
        transcript: Vec::new(),
        per_operation: BTreeMap::new(),
    };

    loop {
        match policy.decide(task, &view) {
            Decision::Done => break,
            Decision::Stuck(reason) => {
                run.stuck = Some(reason);
                run.transcript.push(format!("stuck {reason:?}"));
                break;
            }
            Decision::Call(step) => {
                let step: Step = *step;
                if let Some(fault) = step.fault {
                    view.fired.push(fault);
                    run.faults.push(fault);
                }
                // A family mistake is recorded when it is *decided*, not when it lands, and
                // on both arms alike. That is what makes the injection arm-blind: the typed
                // arm that has no channel for it still knows the agent tried, so it does not
                // decide the same mistake again on the next iteration.
                if let Some(mistake) = step.mistake {
                    view.mistakes.push(mistake.class());
                    run.mistakes.push(mistake.class());
                }
                let observation = surface.perform(rig, task, &step)?;
                // No channel, no composition, no refusal: nothing was attempted, so nothing
                // is counted. The trace still records the intent, which is what lets an
                // independent grader recompute both arms' counts from the transcript alone.
                if observation.unrepresentable {
                    run.prevented += 1;
                    run.transcript.push(observation.line.clone());
                    continue;
                }
                run.attempted += 1;
                run.bytes += observation.bytes;
                if observation.admitted {
                    run.admitted += 1;
                } else {
                    run.invalid += 1;
                    if observation.local_refusal {
                        run.zero_cost_invalid += 1;
                    }
                }
                let entry = run.per_operation.entry(step.call.operation()).or_default();
                entry.calls += 1;
                entry.bytes += observation.bytes;
                run.transcript.push(observation.line.clone());
                view.absorb(
                    &step.call,
                    observation.admitted,
                    observation.code,
                    &observation.reading,
                );
            }
        }
    }

    // The handshake is charged at the end rather than the beginning because a client opens
    // its connection lazily, on its first call: reading the ledger before that would read a
    // zero. The shell arm reports zero here by construction — a CLI process's handshake is
    // inside the process, like the frames it exchanges afterwards — which means the typed
    // arm pays for one negotiation and the baseline pays for none of the many a real
    // per-command CLI would perform. That is the same deliberate handicap `crate::shell`
    // states, applied once more.
    run.bytes += surface.handshake_bytes();
    run.approx_tokens = run.bytes.div_ceil(BYTES_PER_APPROX_TOKEN);
    run.states_read = view.states;
    run.verdict_read = view.verdict;
    run.solved = view.complete()
        && view.states == Some(task.expected.states)
        && view.verdict == Some(task.expected.verdict);
    run.recovered = !run.faults.is_empty() && run.solved;
    Ok(run)
}

/// Drive one arm over the whole matrix: every task, every seed, both policies.
///
/// A **fresh rig per cell**. Sharing one daemon across cells would make a later cell's
/// measurement a function of an earlier cell's idempotency table and task ledger, and the
/// reproduction check would then be asserting that the *sequence* is deterministic rather than
/// that the run is.
///
/// # Errors
///
/// [`SurfaceError`] as [`drive`].
pub fn sweep<S: Surface>(mut build: impl FnMut() -> S) -> Result<Vec<ArmRun>, SurfaceError> {
    let mut runs = Vec::new();
    for task in SUBSET {
        for schedule in SCHEDULES {
            for kind in PolicyKind::ALL {
                let mut rig = Rig::fresh();
                let mut surface = build();
                runs.push(drive(
                    &mut surface,
                    &mut rig,
                    task,
                    Policy::new(kind, schedule.seed),
                )?);
            }
        }
    }
    Ok(runs)
}

/// Drive the shell baseline over the whole matrix.
///
/// # Errors
///
/// [`SurfaceError`] as [`drive`].
pub fn sweep_shell() -> Result<Vec<ArmRun>, SurfaceError> {
    sweep(ShellSurface::new)
}

/// Whether the Phase A subset may be reported on at all.
///
/// The G0 staging rule: "the Phase A benchmark subset used for DX-10 … must itself pass the
/// §19.4 family/source-hash separation check before either result is accepted" (plan §22).
/// This is that gate, and [`crate::report::Report::new`] refuses to mark a report accepted
/// without it.
#[must_use]
pub fn subset_separated() -> separation::Verdict {
    separation::check(SUBSET)
}

/// One attempt's transcript line, for an arm that builds its own observation.
///
/// Re-exported from [`crate::shell`] so both arms render the line with one function: a
/// transcript whose two arms formatted differently could not be compared, and comparing them
/// is what the reproduction check does.
#[must_use]
pub fn line(
    step: &Step,
    admitted: bool,
    code: Option<continuumd::protocol::vocabulary::ErrorCode>,
    bytes: u64,
) -> String {
    transcript_line(step, admitted, code, bytes)
}
