//! Evidence for `PR-10-IMPL-01` — valid operation rate (bn-134i).
//!
//! > - valid operation rate;
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 10
//!
//! # What the metric is, in one line
//!
//! `admitted / attempted`, per arm, per `(task, seed, policy)` cell, where an *attempt* is
//! one operation the shared policy decided to make and an *admission* is a result envelope
//! whose `status` is not `error`. The definition and its two anti-cheating rules are in
//! `continuum_benchmark::run`'s IMPL-01 section; this file holds them to it.
//!
//! # Clause → test
//!
//! - **the rate is computed, per arm, over the whole matrix** →
//!   [`every_cell_reports_a_valid_operation_rate_on_both_arms`].
//! - **a faithful agent's operations are all admitted** →
//!   [`a_faithful_policy_attains_a_perfect_rate_on_both_arms`]. This is the control: if a
//!   correct script produced invalid operations, the rate would be measuring the harness.
//! - **the metric can say no** →
//!   [`the_rate_falls_below_perfect_exactly_where_a_mistake_was_made`]. A metric that could
//!   only report 1000‰ would be an assertion that the daemon exists, not a measurement.
//! - **a locally refused operation is still an invalid attempt** →
//!   [`the_typed_surfaces_local_refusal_is_counted_as_an_invalid_attempt_not_hidden`]. This
//!   is the anti-laundering rule, and it is the one place a benchmark of this shape would
//!   most easily flatter the arm it is promoting.
//! - **the same local refusal costs nothing** →
//!   [`a_locally_refused_operation_spends_no_interface_bytes`], which is the *actual*
//!   research/25 claim ("invalid operations become low-cost local feedback") stated as
//!   bytes rather than as validity.
//! - **the arms are paired by construction** →
//!   [`both_arms_attempt_the_identical_operation_sequence`]: the same policy drives both, so
//!   a difference in the rate cannot come from one arm having been given an easier script.
//! - **admission follows the wire, not the reader** →
//!   [`a_suspended_task_is_an_admission_and_not_an_invalid_operation`]: `task_suspended` is a
//!   budget outcome carrying a continuation (docs/49, RFC 0026), and counting it as a failure
//!   would make the tight-budget fault look like a refusal on both arms.
//!
//! # The finding this file records
//!
//! On this instrument the two arms' valid-operation rates are **equal**, cell for cell.
//! That is not a null result to be explained away: it is what a *disciplined* baseline
//! against a well-formed text projection produces, and
//! [`the_two_arms_valid_operation_rates_are_equal_on_this_instrument`] records it as a
//! measured fact so that a change which moves it fails loudly. What the typed surface
//! changes here is the **cost** of a mistake, not its frequency — see the byte evidence.

mod support;

use continuum_benchmark::policy::{Fault, Policy, PolicyKind};
use continuum_benchmark::report::Report;
use continuum_benchmark::rig::Rig;
use continuum_benchmark::run::{self, ArmRun};
use continuum_benchmark::surface::Arm;
use continuum_benchmark::task::{DIE_HARD_ALL, SUBSET};
use continuumd::protocol::spec::ProtocolEnum;
use continuumd::protocol::vocabulary::ErrorCode;

fn matrix() -> Vec<ArmRun> {
    support::both_arms()
}

fn cell<'a>(runs: &'a [ArmRun], arm: Arm, task: &str, seed: u16, kind: PolicyKind) -> &'a ArmRun {
    runs.iter()
        .find(|run| run.arm == arm && run.task == task && run.seed == seed && run.policy == kind)
        .expect("the matrix contains every cell")
}

#[test]
fn every_cell_reports_a_valid_operation_rate_on_both_arms() {
    let runs = matrix();
    assert_eq!(
        runs.len(),
        SUBSET.len() * 3 * 2 * 2,
        "four tasks, three seeds, two policies, two arms"
    );
    for run in &runs {
        assert!(
            run.attempted > 0,
            "{} {} attempted nothing",
            run.task,
            run.seed
        );
        assert_eq!(
            run.admitted + run.invalid,
            run.attempted,
            "every attempt is admitted or invalid, and never both"
        );
        assert_eq!(
            run.valid_rate_permille(),
            (u64::from(run.admitted) * 1000) / u64::from(run.attempted),
            "the reported rate is the definition, not a second calculation"
        );
        assert!(
            run.zero_cost_invalid <= run.invalid,
            "a zero-cost refusal is a kind of invalid attempt, never a separate population"
        );
    }
}

#[test]
fn a_faithful_policy_attains_a_perfect_rate_on_both_arms() {
    let runs = matrix();
    for run in runs.iter().filter(|run| run.policy == PolicyKind::Faithful) {
        assert_eq!(
            run.valid_rate_permille(),
            1000,
            "{} {} {:?}: a correct script is admitted end to end",
            run.task,
            run.seed,
            run.arm
        );
        assert_eq!(run.invalid, 0);
    }
}

#[test]
fn the_rate_falls_below_perfect_exactly_where_a_mistake_was_made() {
    let runs = matrix();
    for arm in Arm::ALL {
        // Seed 2 fires `wrong-principal` and `unsealed-start`; both are refusals.
        let faulted = cell(&runs, arm, DIE_HARD_ALL.id, 2, PolicyKind::Fallible);
        assert!(
            faulted.valid_rate_permille() < 1000,
            "{arm:?}: two deliberate mistakes must show up in the rate"
        );
        assert_eq!(faulted.invalid, 2, "{arm:?}: exactly the two mistakes");

        // Seed 1 fires `tight-budget`, which the daemon *admits* — it is a budget outcome,
        // not a rejected request — so the rate stays perfect and the recovery shows up in
        // attempts and bytes instead.
        let budgeted = cell(&runs, arm, DIE_HARD_ALL.id, 1, PolicyKind::Fallible);
        assert_eq!(budgeted.faults, vec![Fault::TightBudget]);
        assert_eq!(
            budgeted.valid_rate_permille(),
            1000,
            "{arm:?}: a suspension is an admission"
        );
    }
}

#[test]
fn the_typed_surfaces_local_refusal_is_counted_as_an_invalid_attempt_not_hidden() {
    let runs = matrix();
    let native = cell(&runs, Arm::Native, DIE_HARD_ALL.id, 2, PolicyKind::Fallible);
    let shell = cell(&runs, Arm::Shell, DIE_HARD_ALL.id, 2, PolicyKind::Fallible);

    assert_eq!(
        native.zero_cost_invalid, 1,
        "the register refuses `verification.start` over an unsealed snapshot before the wire"
    );
    assert_eq!(
        shell.zero_cost_invalid, 0,
        "a text surface has no register to refuse with"
    );
    assert_eq!(
        native.invalid, shell.invalid,
        "and the locally refused operation is still counted: the two arms made the same two \
         mistakes and both arms report two invalid attempts"
    );
    assert_eq!(
        native.attempted, shell.attempted,
        "a locally refused operation is an attempt on both arms"
    );
}

#[test]
fn a_locally_refused_operation_spends_no_interface_bytes() {
    // Drive the fallible seed-2 script one step at a time and catch the refusal in flight,
    // because the run-level totals cannot show that *this* attempt was the free one.
    use continuum_benchmark::policy::{AgentView, Decision, Step};
    use continuum_benchmark::surface::Surface;

    let mut rig = Rig::fresh();
    let mut surface = support::NativeSurface::new();
    let policy = Policy::new(PolicyKind::Fallible, 2);
    let mut view = AgentView::default();
    let mut zero_cost = 0;

    for _ in 0..12 {
        match policy.decide(&DIE_HARD_ALL, &view) {
            Decision::Call(step) => {
                let step: Step = *step;
                if let Some(fault) = step.fault {
                    view.fired.push(fault);
                }
                let observation = surface
                    .perform(&mut rig, &DIE_HARD_ALL, &step)
                    .expect("the arm attempts every operation");
                if observation.local_refusal {
                    zero_cost += 1;
                    assert_eq!(
                        observation.bytes, 0,
                        "a refusal that never reached the wire spends nothing"
                    );
                    assert!(!observation.admitted, "and it is not an admission");
                    assert_eq!(
                        observation.code, None,
                        "no wire error code is minted for a call the wire never saw"
                    );
                }
                view.absorb(
                    &step.call,
                    observation.admitted,
                    observation.code,
                    &observation.reading,
                );
            }
            Decision::Done => break,
            Decision::Stuck(reason) => panic!("the script must finish: {reason:?}"),
        }
    }
    assert_eq!(
        zero_cost, 1,
        "exactly the unsealed start was refused locally"
    );
}

#[test]
fn both_arms_attempt_the_identical_operation_sequence() {
    let runs = matrix();
    for task in SUBSET {
        for seed in [1_u16, 2, 3] {
            for kind in PolicyKind::ALL {
                let native = cell(&runs, Arm::Native, task.id, seed, kind);
                let shell = cell(&runs, Arm::Shell, task.id, seed, kind);
                assert_eq!(
                    native.attempted, shell.attempted,
                    "{} seed {seed} {kind:?}: one policy, two arms",
                    task.id
                );
                assert_eq!(native.faults, shell.faults);
                let operations = |run: &ArmRun| -> Vec<String> {
                    run.transcript
                        .iter()
                        .filter_map(|line| line.split(' ').next().map(str::to_owned))
                        .collect()
                };
                assert_eq!(
                    operations(native),
                    operations(shell),
                    "{} seed {seed} {kind:?}: the same operations, in the same order",
                    task.id
                );
            }
        }
    }
}

#[test]
fn a_suspended_task_is_an_admission_and_not_an_invalid_operation() {
    let runs = matrix();
    for arm in Arm::ALL {
        let run = cell(&runs, arm, DIE_HARD_ALL.id, 1, PolicyKind::Fallible);
        assert!(
            run.transcript.iter().any(
                |line| line.starts_with("verification.start") && line.contains("admitted=true")
            ),
            "{arm:?}: the start under a four-state ceiling is admitted"
        );
        assert!(
            run.transcript
                .iter()
                .any(|line| line.starts_with("task.resume") && line.contains("admitted=true")),
            "{arm:?}: and the continuation it parked is spendable"
        );
        assert!(
            !run.transcript
                .iter()
                .any(|line| line.contains(ErrorCode::BudgetExhausted.as_wire())),
            "{arm:?}: budget exhaustion is never a refusal here"
        );
    }
}

/// The measured finding, recorded so that a change to it is loud.
#[test]
fn the_two_arms_valid_operation_rates_are_equal_on_this_instrument() {
    let report = Report::new(matrix());
    let native = report.totals[&Arm::Native];
    let shell = report.totals[&Arm::Shell];

    assert_eq!(
        native.invalid_permille(),
        shell.invalid_permille(),
        "measured: a disciplined text baseline against a well-formed projection makes the \
         same number of invalid operations as the typed surface"
    );
    assert_eq!(
        report.margins.invalid_reduction_percent,
        Some(0),
        "so the ratified 50% relative reduction is not met on this instrument"
    );
    assert!(
        !report.margins.invalid_clears,
        "and the harness says so rather than rounding it away"
    );
    assert!(
        native.zero_cost_invalid > 0 && shell.zero_cost_invalid == 0,
        "what the typed surface does change is that some of those mistakes cost nothing"
    );
    // Anti-vacuity for the whole file: the matrix contains mistakes at all.
    assert!(native.invalid > 0 && shell.invalid > 0);
    assert!(run::subset_separated().passes());
}
