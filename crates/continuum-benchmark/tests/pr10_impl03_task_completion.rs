//! Evidence for `PR-10-IMPL-03` — task completion (bn-3awq).
//!
//! > - task completion;
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 10
//!
//! # What "completed" means here, and what it refuses to mean
//!
//! Not "the script ran to the end". A run is **solved** when the agent read the answer *and
//! the answer was the dossier's*:
//!
//! ```text
//! solved = view.complete()
//!       && states_read  == expected.states
//!       && verdict_read == expected.verdict
//! ```
//!
//! `view.complete()` is the agent-side half — settled, verdict read, state count read, and
//! ceiling-enforcement known — and the equality is graded against `task::Frozen`, whose
//! numbers are the Revision-2 spike's and are repeated in four other places in the dossier.
//! Neither arm can influence them.
//!
//! # Clause → test
//!
//! - **both arms complete both corpus ports** →
//!   [`every_faithful_cell_is_solved_on_both_arms`], which is plan §21's Phase A exit
//!   sentence — "Die Hard **and Dining Philosophers** can be checked through … an agent
//!   client with identical artifacts" — at this harness's grain.
//! - **the artifacts are identical across arms** →
//!   [`the_two_arms_read_the_same_state_count_and_the_same_verdict`].
//! - **the numbers crossed the interface** →
//!   [`the_state_count_each_arm_reports_is_the_one_it_read_through_its_own_surface`].
//! - **what has no wire home is corroborated, not invented** →
//!   [`the_transition_count_and_witness_depth_are_read_one_layer_down_and_said_to_be`]. RFC
//!   0026 F16 gives `Cost` no transition dimension and no producer builds a crashpack, so
//!   those two numbers are read from `Daemon::state` with the boundary named — exactly as
//!   `continuumd`'s own PR-8 exit evidence reads them.
//! - **the grader can say no** →
//!   [`a_run_graded_against_the_wrong_frozen_answer_is_not_solved`] and
//!   [`a_run_that_never_read_the_manifest_is_not_complete`]. A completion criterion that
//!   could not fail would be a tautology.
//! - **the two targets are genuinely different questions** →
//!   [`the_property_target_and_the_all_claims_target_produce_different_verdicts`].

mod support;

use continuum_benchmark::policy::{AgentView, PolicyKind};
use continuum_benchmark::report::Report;
use continuum_benchmark::rig::Rig;
use continuum_benchmark::run::{self, ArmRun};
use continuum_benchmark::surface::{Arm, Reading};
use continuum_benchmark::task::{
    DIE_HARD_ALL, DIE_HARD_TYPE_OK, PHILOSOPHERS_ALL, PHILOSOPHERS_OWNERSHIP, SUBSET,
};
use continuumd::protocol::scalar::TaskHandle;
use continuumd::protocol::vocabulary::SemanticVerdict;

#[test]
fn every_faithful_cell_is_solved_on_both_arms() {
    let runs = support::both_arms();
    for run in runs.iter().filter(|run| run.policy == PolicyKind::Faithful) {
        assert!(
            run.solved,
            "{} seed {} on the {:?} arm did not reach the frozen answer: states={:?} \
             verdict={:?} stuck={:?}",
            run.task, run.seed, run.arm, run.states_read, run.verdict_read, run.stuck
        );
        assert!(run.stuck.is_none());
    }
    let report = Report::new(runs);
    for arm in Arm::ALL {
        assert_eq!(
            report.totals[&arm].success_permille(),
            1000,
            "{arm:?}: every cell in the matrix is solved"
        );
    }
}

#[test]
fn the_two_arms_read_the_same_state_count_and_the_same_verdict() {
    let runs = support::both_arms();
    for task in SUBSET {
        for seed in [1_u16, 2, 3] {
            for kind in PolicyKind::ALL {
                let pick = |arm: Arm| -> &ArmRun {
                    runs.iter()
                        .find(|run| {
                            run.arm == arm
                                && run.task == task.id
                                && run.seed == seed
                                && run.policy == kind
                        })
                        .expect("every cell is present")
                };
                let native = pick(Arm::Native);
                let shell = pick(Arm::Shell);
                assert_eq!(
                    (native.states_read, native.verdict_read),
                    (shell.states_read, shell.verdict_read),
                    "{} seed {seed} {kind:?}: identical artifacts through two surfaces",
                    task.id
                );
                assert_eq!(native.states_read, Some(task.expected.states));
                assert_eq!(native.verdict_read, Some(task.expected.verdict));
            }
        }
    }
}

#[test]
fn the_state_count_each_arm_reports_is_the_one_it_read_through_its_own_surface() {
    // The native arm reads `TaskRecord.cost.states` off a decoded frame; the shell arm reads
    // the `cost.states:` line of a rendering. Neither reads the engine.
    for task in [&DIE_HARD_ALL, &PHILOSOPHERS_ALL] {
        let (native, _) = support::native_cell(task, PolicyKind::Faithful, 1);
        let shell = support::shell_cell(task, PolicyKind::Faithful, 1);
        assert_eq!(
            native.states_read,
            Some(task.expected.states),
            "{}",
            task.id
        );
        assert_eq!(shell.states_read, Some(task.expected.states), "{}", task.id);
        assert!(
            native
                .transcript
                .iter()
                .any(|line| line.starts_with("task.status")),
            "and it crossed the interface on a `task.status` answer"
        );
    }
}

/// The honesty boundary: two of the four frozen numbers have no wire field.
#[test]
fn the_transition_count_and_witness_depth_are_read_one_layer_down_and_said_to_be() {
    use continuum_benchmark::policy::{Call, Step};
    use continuum_benchmark::surface::Surface;

    for task in [&DIE_HARD_ALL, &PHILOSOPHERS_ALL] {
        let mut rig = Rig::fresh();
        let mut surface = continuum_benchmark::shell::ShellSurface::new();
        let mut view = AgentView::default();
        let mut run = |view: &mut AgentView, call: Call, rig: &mut Rig| -> Reading {
            let step = Step::faithful(call);
            let observation = surface
                .perform(rig, task, &step)
                .expect("the arm attempts the operation");
            view.absorb(
                &step.call,
                observation.admitted,
                observation.code,
                &observation.reading,
            );
            observation.reading
        };
        run(&mut view, Call::CreateWorkspace { seal: false }, &mut rig);
        let snapshot = view.snapshot.clone().expect("a snapshot was created");
        run(
            &mut view,
            Call::SealWorkspace {
                snapshot: snapshot.clone(),
            },
            &mut rig,
        );
        run(
            &mut view,
            Call::StartVerification {
                snapshot,
                states: task.ceiling,
            },
            &mut rig,
        );
        let handle: TaskHandle = view.task.clone().expect("a campaign started");

        // Nothing between this line and the previous one crossed a frame. The two numbers
        // below are read from the daemon in the same process, because RFC 0026 F16 gives
        // `Cost` no transition dimension and nothing in this workspace builds a crashpack.
        let campaign = rig
            .daemon()
            .state()
            .tasks()
            .get(&handle)
            .expect("the daemon holds the task")
            .campaign
            .as_ref()
            .expect("the campaign ran");
        assert!(
            campaign.is_closed(),
            "{}: the ceiling admits the model",
            task.id
        );
        assert_eq!(campaign.states() as u64, task.expected.states);
        assert_eq!(
            campaign.transitions, task.expected.transitions,
            "{}: the labelled transition count, one layer down",
            task.id
        );
    }

    // Die Hard's witness depth is an *invariant* violation; Dining Philosophers' is a
    // deadlock, which `violation_depth` deliberately does not report because it reads
    // invariant outcomes only. Reading each from the place that actually holds it is the
    // point of this test.
    let mut rig = Rig::fresh();
    let (run, _) = drive_to_campaign(&mut rig, &DIE_HARD_ALL);
    assert_eq!(
        run,
        Some(DIE_HARD_ALL.expected.witness_depth),
        "the film's six-step solution refutes `NotSolved`"
    );
}

/// Start a campaign over `task` through the shell arm and return the invariant violation
/// depth the daemon recorded, if any.
fn drive_to_campaign(
    rig: &mut Rig,
    task: &continuum_benchmark::task::BenchmarkTask,
) -> (Option<usize>, TaskHandle) {
    use continuum_benchmark::policy::{Call, Step};
    use continuum_benchmark::surface::Surface;

    let mut surface = continuum_benchmark::shell::ShellSurface::new();
    let mut view = AgentView::default();
    let step = Step::faithful(Call::CreateWorkspace { seal: true });
    let observation = surface.perform(rig, task, &step).expect("attempted");
    view.absorb(
        &step.call,
        observation.admitted,
        observation.code,
        &observation.reading,
    );
    let snapshot = view.snapshot.clone().expect("a snapshot was created");
    let step = Step::faithful(Call::StartVerification {
        snapshot,
        states: task.ceiling,
    });
    let observation = surface.perform(rig, task, &step).expect("attempted");
    view.absorb(
        &step.call,
        observation.admitted,
        observation.code,
        &observation.reading,
    );
    let handle = view.task.clone().expect("a campaign started");
    let depth = rig
        .daemon()
        .state()
        .tasks()
        .get(&handle)
        .expect("the daemon holds the task")
        .campaign
        .as_ref()
        .expect("the campaign ran")
        .violation_depth();
    (depth, handle)
}

#[test]
fn a_run_graded_against_the_wrong_frozen_answer_is_not_solved() {
    let mut wrong = DIE_HARD_ALL;
    wrong.expected.states += 1;
    let (run, _) = support::native_cell(&wrong, PolicyKind::Faithful, 1);
    assert!(
        !run.solved,
        "an arm that read 16 states must not be graded as solving a task expecting 17"
    );
    assert_eq!(
        run.states_read,
        Some(DIE_HARD_ALL.expected.states),
        "and the number it actually read is still reported"
    );
    assert!(run.stuck.is_none(), "it finished; it just did not match");

    let mut flipped = DIE_HARD_ALL;
    flipped.expected.verdict = SemanticVerdict::Established;
    let (run, _) = support::native_cell(&flipped, PolicyKind::Faithful, 1);
    assert!(
        !run.solved,
        "a refuted campaign does not satisfy `established`"
    );
}

#[test]
fn a_run_that_never_read_the_manifest_is_not_complete() {
    // Completion includes knowing whether the declared ceiling was metered. A view with
    // every other field filled in is still incomplete without it, which is what makes the
    // INV-007 manifest part of the task rather than a decoration.
    let mut view = AgentView {
        settled: true,
        result_read: true,
        states: Some(16),
        verdict: Some(SemanticVerdict::Refuted),
        ceiling_enforced: None,
        ..AgentView::default()
    };
    assert!(!view.complete());
    view.ceiling_enforced = Some(true);
    assert!(view.complete());
}

#[test]
fn the_property_target_and_the_all_claims_target_produce_different_verdicts() {
    // `dh-type-ok` asks about one invariant that holds, over a model with no deadlock, and
    // is `established`. `dh-all` asks about every claim, and `NotSolved` is refuted at depth
    // six. Same model, same snapshot, same explored set — different obligations.
    assert_eq!(
        DIE_HARD_TYPE_OK.expected.verdict,
        SemanticVerdict::Established
    );
    assert_eq!(DIE_HARD_ALL.expected.verdict, SemanticVerdict::Refuted);
    for task in [&DIE_HARD_TYPE_OK, &DIE_HARD_ALL] {
        let (run, _) = support::native_cell(task, PolicyKind::Faithful, 1);
        assert_eq!(run.verdict_read, Some(task.expected.verdict), "{}", task.id);
        assert!(run.solved);
    }

    // Dining Philosophers is the case where the two agree *and the reason is worth stating*:
    // `ForkOwnership` is established over all 573 states, and the campaign is refuted anyway,
    // because `verification.start` maps every target kind onto `DeadlockPolicy::Defect` and a
    // refutation dominates the fold.
    assert_eq!(
        PHILOSOPHERS_OWNERSHIP.expected.verdict,
        SemanticVerdict::Refuted
    );
    let (run, _) = support::native_cell(&PHILOSOPHERS_OWNERSHIP, PolicyKind::Faithful, 1);
    assert!(run.solved);
    assert_eq!(run.verdict_read, Some(SemanticVerdict::Refuted));
    assert!(run::subset_separated().passes());
}
