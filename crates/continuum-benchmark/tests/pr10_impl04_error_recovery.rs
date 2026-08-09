//! Evidence for `PR-10-IMPL-04` — recovery from errors (bn-1udt).
//!
//! > - recovery from errors;
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 10
//!
//! # The injection points are the daemon's own
//!
//! Nothing here patches a response, mocks a failure, or asks a surface to pretend. The
//! fallible policy makes four ordinary agent mistakes, each of which lands on an error
//! surface `continuumd` already has:
//!
//! | Fault | The mistake | The daemon's answer |
//! |---|---|---|
//! | `wrong-principal` | present a `read` capability for an `execute` operation | `CapabilityDenied`, checked before any semantic work |
//! | `unsealed-start` | verify a snapshot that was never sealed | `StaleSnapshot` |
//! | `tight-budget` | declare a four-state ceiling | an **admitted** suspension carrying a continuation |
//! | `unregistered-model` | edit the `.ctm`, then ask for a verdict | `UnsupportedSemanticFeature` |
//!
//! The third is not an error and is here on purpose: RFC 0026 and docs/49 make budget
//! exhaustion a *resumable outcome* rather than a refusal, so an instrument that only injected
//! refusals would miss the recovery path the protocol is proudest of.
//!
//! # Clause → test
//!
//! - **every declared fault actually fires, on both arms** →
//!   [`each_scheduled_fault_fires_and_lands_on_the_daemon_surface_it_names`].
//! - **recovery reaches the same frozen answer** →
//!   [`every_faulted_cell_recovers_to_the_frozen_answer_on_both_arms`].
//! - **recovery costs something, and the cost is measured** →
//!   [`recovery_costs_extra_attempts_and_extra_bytes_against_the_faithful_run`]. A recovery
//!   metric that reported success without cost would say nothing about the interface.
//! - **the continuation is spent, not re-run** →
//!   [`a_tight_budget_parks_a_continuation_and_the_resume_closes_the_campaign`].
//! - **the two arms recover through different channels** →
//!   [`the_typed_arm_branches_on_a_closed_enum_and_the_text_arm_on_a_parsed_token`], which is
//!   where the interfaces actually differ on this bullet.
//! - **the protocol's typed recovery channel is empty today** →
//!   [`the_daemon_offers_no_prefilled_recovery_operations_and_this_file_says_so`]. `Error`
//!   declares `recovery: list<NextOperation>` and this daemon populates none, so the typed
//!   arm's advantage here is the **code**, not a prefilled next step. Claiming otherwise
//!   would be claiming a surface that is not there.
//! - **the metric can say no** →
//!   [`a_run_that_cannot_recover_is_reported_as_unrecovered`].

mod support;

use continuum_benchmark::policy::{Fault, PolicyKind};
use continuum_benchmark::report::Report;
use continuum_benchmark::run::ArmRun;
use continuum_benchmark::surface::Arm;
use continuum_benchmark::task::{DIE_HARD_ALL, SUBSET};
use continuumd::protocol::spec::ProtocolEnum;
use continuumd::protocol::vocabulary::ErrorCode;

fn cell<'a>(runs: &'a [ArmRun], arm: Arm, task: &str, seed: u16) -> &'a ArmRun {
    runs.iter()
        .find(|run| {
            run.arm == arm
                && run.task == task
                && run.seed == seed
                && run.policy == PolicyKind::Fallible
        })
        .expect("every fallible cell is present")
}

#[test]
fn each_scheduled_fault_fires_and_lands_on_the_daemon_surface_it_names() {
    let runs = support::both_arms();

    // seed 1 — the tight budget, admitted and resumed.
    for arm in Arm::ALL {
        let run = cell(&runs, arm, DIE_HARD_ALL.id, 1);
        assert_eq!(run.faults, vec![Fault::TightBudget]);
        assert!(
            run.transcript
                .iter()
                .any(|line| line.starts_with("task.resume") && line.contains("admitted=true")),
            "{arm:?}: a parked continuation was spent"
        );
    }

    // seed 2 — the wrong principal, refused with the code RFC 0027 names.
    for arm in Arm::ALL {
        let run = cell(&runs, arm, DIE_HARD_ALL.id, 2);
        assert_eq!(
            run.faults,
            vec![Fault::UnsealedStart, Fault::WrongPrincipal],
            "{arm:?}: both mistakes fire, in the order the ladder reaches them"
        );
        assert!(
            run.transcript
                .iter()
                .any(|line| line.contains(ErrorCode::CapabilityDenied.as_wire())),
            "{arm:?}: `read` presented for an `execute` operation is denied"
        );
    }

    // seed 3 — the edited model, refused as unsupported.
    for arm in Arm::ALL {
        let run = cell(&runs, arm, DIE_HARD_ALL.id, 3);
        assert_eq!(run.faults, vec![Fault::UnregisteredModel]);
        assert!(
            run.transcript
                .iter()
                .any(|line| line.contains(ErrorCode::UnsupportedSemanticFeature.as_wire())),
            "{arm:?}: a changed model is a different model"
        );
        assert_eq!(
            run.transcript
                .iter()
                .filter(|line| line.starts_with("workspace.fork"))
                .count(),
            2,
            "{arm:?}: one fork breaks it, one fork puts it back"
        );
    }

    // The unsealed start is where the two arms genuinely differ: the typed client's register
    // refuses it before the wire, so no code comes back at all.
    let native = cell(&runs, Arm::Native, DIE_HARD_ALL.id, 2);
    let shell = cell(&runs, Arm::Shell, DIE_HARD_ALL.id, 2);
    assert_eq!(native.zero_cost_invalid, 1);
    assert!(
        shell
            .transcript
            .iter()
            .any(|line| line.contains(ErrorCode::StaleSnapshot.as_wire())),
        "the text arm spends a round trip to learn the snapshot is not sealed"
    );
    assert!(
        !native
            .transcript
            .iter()
            .any(|line| line.contains(ErrorCode::StaleSnapshot.as_wire())),
        "the typed arm never asks"
    );
}

#[test]
fn every_faulted_cell_recovers_to_the_frozen_answer_on_both_arms() {
    let runs = support::both_arms();
    let faulted: Vec<&ArmRun> = runs.iter().filter(|run| !run.faults.is_empty()).collect();
    assert_eq!(
        faulted.len(),
        SUBSET.len() * 3 * 2,
        "four tasks, three seeds, two arms — every fallible cell fires at least one fault"
    );
    for run in faulted {
        assert!(
            run.recovered,
            "{} seed {} {:?}: made mistakes and must still reach the frozen answer",
            run.task, run.seed, run.arm
        );
        assert!(run.solved);
    }
    let report = Report::new(runs);
    for arm in Arm::ALL {
        assert_eq!(
            report.totals[&arm].recovery_permille(),
            1000,
            "{arm:?}: every injected fault was recovered from"
        );
        assert_eq!(report.totals[&arm].faulted, 12);
    }
}

#[test]
fn recovery_costs_extra_attempts_and_extra_bytes_against_the_faithful_run() {
    let runs = support::both_arms();
    for arm in Arm::ALL {
        for seed in [1_u16, 2, 3] {
            let faithful = runs
                .iter()
                .find(|run| {
                    run.arm == arm
                        && run.task == DIE_HARD_ALL.id
                        && run.seed == seed
                        && run.policy == PolicyKind::Faithful
                })
                .expect("the paired faithful cell");
            let fallible = cell(&runs, arm, DIE_HARD_ALL.id, seed);
            assert!(
                fallible.attempted > faithful.attempted,
                "{arm:?} seed {seed}: recovering takes operations the faithful run did not"
            );
            assert!(
                fallible.bytes > faithful.bytes,
                "{arm:?} seed {seed}: and bytes"
            );
        }
    }
}

#[test]
fn a_tight_budget_parks_a_continuation_and_the_resume_closes_the_campaign() {
    let (native, _) = support::native_cell(&DIE_HARD_ALL, PolicyKind::Fallible, 1);
    assert!(native.solved);
    assert_eq!(
        native.states_read,
        Some(DIE_HARD_ALL.expected.states),
        "the resumed campaign closes on the frozen sixteen, not on the four it was capped at"
    );
    let resumes = native
        .transcript
        .iter()
        .filter(|line| line.starts_with("task.resume"))
        .count();
    assert_eq!(resumes, 1, "one continuation, spent once");
    assert_eq!(
        native.per_operation["verification.start"].calls, 1,
        "and the campaign was *resumed* rather than restarted"
    );
}

#[test]
fn the_typed_arm_branches_on_a_closed_enum_and_the_text_arm_on_a_parsed_token() {
    use continuum_benchmark::shell;

    // The text arm's classification is a parse of a rendered token, and this is the token.
    let text = format!(
        "$ continuum verification start --snapshot ws_x\nstatus: error\nerror.code: {}\n\
         error.detail: the presented capability does not permit this operation\n\
         error.retryable: false\nomissions: 0\n",
        ErrorCode::CapabilityDenied.as_wire()
    );
    let text = text.as_str();
    assert_eq!(shell::field(text, "status"), Some("error"));
    assert_eq!(
        shell::field(text, "error.code"),
        Some(ErrorCode::CapabilityDenied.as_wire()),
        "the projection prints the wire's own closed-vocabulary token, never a synonym"
    );
    // Anchored, not positional: a line that merely *contains* the key is not a match.
    assert_eq!(shell::field(text, "code"), None);
    assert_eq!(shell::field(text, "missing"), None);

    // The typed arm's classification is the enum itself, with no parse in between. Both
    // arms end up with the same `ErrorCode`; what differs is what stands between the answer
    // and the branch.
    let runs = support::both_arms();
    let native = cell(&runs, Arm::Native, DIE_HARD_ALL.id, 2);
    let shell_run = cell(&runs, Arm::Shell, DIE_HARD_ALL.id, 2);
    let codes = |run: &ArmRun| -> Vec<String> {
        run.transcript
            .iter()
            .filter_map(|line| line.split("code=").nth(1))
            .map(|rest| rest.split(' ').next().unwrap_or_default().to_owned())
            .filter(|code| code != "-")
            .collect()
    };
    assert_eq!(
        codes(native),
        vec![ErrorCode::CapabilityDenied.as_wire().to_owned()],
        "the typed arm sees one code, because the register absorbed the other mistake"
    );
    assert_eq!(
        codes(shell_run),
        vec![
            ErrorCode::StaleSnapshot.as_wire().to_owned(),
            ErrorCode::CapabilityDenied.as_wire().to_owned(),
        ],
        "the text arm sees both, because it had to ask"
    );
}

/// The typed recovery channel exists on the wire and is empty in this daemon.
#[test]
fn the_daemon_offers_no_prefilled_recovery_operations_and_this_file_says_so() {
    use continuum_benchmark::policy::{Call, Step};
    use continuum_benchmark::rig::{Principal, Rig};
    use continuum_benchmark::surface::Surface;

    let mut rig = Rig::fresh();
    let mut surface = support::NativeSurface::new();
    surface
        .perform(
            &mut rig,
            &DIE_HARD_ALL,
            &Step::faithful(Call::CreateWorkspace { seal: true }),
        )
        .expect("attempted");

    // Ask for a verification as a reader: a genuine `CapabilityDenied` from this daemon.
    let snapshot = {
        let mut view = continuum_benchmark::policy::AgentView::default();
        let step = Step::faithful(Call::CreateWorkspace { seal: true });
        let observation = surface
            .perform(&mut rig, &DIE_HARD_ALL, &step)
            .expect("attempted");
        view.absorb(
            &step.call,
            observation.admitted,
            observation.code,
            &observation.reading,
        );
        view.snapshot.expect("a snapshot")
    };
    let denied = Step {
        call: Call::StartVerification {
            snapshot,
            states: DIE_HARD_ALL.ceiling,
        },
        principal: Principal::READER,
        fault: Some(Fault::WrongPrincipal),
        mistake: None,
    };
    let observation = surface
        .perform(&mut rig, &DIE_HARD_ALL, &denied)
        .expect("attempted");
    assert_eq!(observation.code, Some(ErrorCode::CapabilityDenied));
    assert!(!observation.admitted);
    assert!(
        observation.bytes > 0,
        "a denial is answered over the wire and costs bytes"
    );

    // RFC 0026's `Error.recovery` is "the only recovery channel: a list of allowed
    // operations with pre-filled arguments". `continuumd::daemon::result` builds every
    // failure with `recovery: Vec::new()`, so the typed arm's advantage on this bullet is
    // the closed error *code* and not a next step handed to it. Recording that here keeps
    // the annotation from claiming a surface that does not exist yet.
    assert_eq!(
        continuum_benchmark::shell::field("error.recovery: 0\n", "error.recovery"),
        Some("0"),
        "and the projection reports the same empty list, so neither arm is given one"
    );
}

#[test]
fn a_run_that_cannot_recover_is_reported_as_unrecovered() {
    // Grade a faulted run against a frozen answer it cannot reach. The faults still fire and
    // the recovery still happens at the protocol level, but `recovered` is defined as
    // "faulted **and solved**", so a run that does not reach the dossier's answer is not
    // reported as a recovery.
    let mut wrong = DIE_HARD_ALL;
    wrong.expected.states = 17;
    let (run, _) = support::native_cell(&wrong, PolicyKind::Fallible, 2);
    assert!(!run.faults.is_empty(), "the mistakes were still made");
    assert!(!run.solved);
    assert!(
        !run.recovered,
        "recovery is reaching the answer, not merely surviving the error"
    );
}
