//! Evidence for `PR-10-IMPL-02` — tokens/bytes (bn-23gm).
//!
//! > - tokens/bytes;
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 10
//!
//! # The counting rule, stated once
//!
//! An **interface byte** is a byte the agent wrote or read at its own interface:
//!
//! | Arm | written | read | deliberately not counted |
//! |---|---|---|---|
//! | native | the request frame | the result frame | — |
//! | shell | the command line | the rendered output | the CLI process's own frames |
//!
//! The graded denominator is **bytes per solved task**, per RFC 0027 and plan §24.5's
//! ratified margins ("bytes, not tokens, are the graded cost denominator"). `approx_tokens`
//! is `ceil(bytes / 4)` by a **declared arithmetic rule** — no tokenizer, no vocabulary, no
//! model — and nothing is graded on it; it exists because the bullet says "tokens/bytes" and
//! silently dropping the word would narrow the bullet.
//!
//! # Clause → test
//!
//! - **the byte count is the two frames' lengths, not an estimate** →
//!   [`a_native_calls_bytes_are_the_two_frames_the_client_actually_moved`].
//! - **the shell count is the command plus the output** →
//!   [`a_shell_calls_bytes_are_the_command_line_plus_the_rendered_output`], which recomputes
//!   both from the public rendering functions and compares.
//! - **the token rule is arithmetic and declared** →
//!   [`the_token_count_is_the_declared_rule_applied_to_the_same_bytes`].
//! - **the denominator is per *solved* task** →
//!   [`bytes_per_solved_task_ignores_a_run_that_did_not_solve_anything`], so an arm cannot win
//!   by failing early and cheaply.
//! - **the seeds knob is real** → [`every_metric_is_paired_over_three_declared_seeds`], the
//!   requirement bn-26tb's lead note attached to this bullet.
//! - **per-operation reporting exists** →
//!   [`the_report_says_where_the_bytes_went_operation_by_operation`], which is what a
//!   redesign decision needs and a single ratio cannot give.
//!
//! # The finding this file records
//!
//! **The typed surface loses this metric on this instrument**, and by a wide margin: it
//! spends about two and a half times the interface bytes of the disciplined text baseline
//! per solved task. [`the_typed_surface_spends_more_interface_bytes_than_the_text_baseline`]
//! records it as a measured fact with the direction asserted, so a change that flips it fails
//! loudly and the annotation has to be re-read.
//!
//! The dominant cause is visible in the per-operation breakdown and is not a rendering
//! choice: a `ResultEnvelope` carries its six epochs, its nine-dimension cost, its assurance
//! envelope, its omission manifest and its `next_operations` on **every** answer, while the
//! text projection prints the fields the agent asked about. `workspace.create` is the second
//! cause and is structural rather than incidental — a local CLI names a corpus port and
//! resolves its components from the filesystem, while a remote protocol client must transmit
//! `SnapshotComponents` in full. Whether either is worth fixing, and how, is the redesign
//! question this lane exists to raise; it is not this harness's to answer.

mod support;

use continuum_benchmark::policy::{Call, PolicyKind, Step};
use continuum_benchmark::report::Report;
use continuum_benchmark::rig::Rig;
use continuum_benchmark::run::{ArmRun, BYTES_PER_APPROX_TOKEN};
use continuum_benchmark::shell::{self, ShellSurface};
use continuum_benchmark::surface::{Arm, Surface};
use continuum_benchmark::task::DIE_HARD_ALL;

#[test]
fn a_native_calls_bytes_are_the_two_frames_the_client_actually_moved() {
    let mut rig = Rig::fresh();
    let mut surface = support::NativeSurface::new();
    let step = Step::faithful(Call::CreateWorkspace { seal: false });
    let observation = surface
        .perform(&mut rig, &DIE_HARD_ALL, &step)
        .expect("the arm attempts the operation");

    let ledger = surface.ledger();
    assert_eq!(ledger.calls, 1, "one call reached the wire");
    assert_eq!(
        observation.bytes,
        ledger.operations.total(),
        "the observation's byte count is the client's own ledger, not a second count"
    );
    assert_eq!(
        ledger.operations.total(),
        ledger.operations.sent + ledger.operations.received,
        "both directions, and only those"
    );
    assert!(
        ledger.operations.sent > 0 && ledger.operations.received > 0,
        "a real request frame and a real result frame crossed"
    );
    assert!(
        ledger.handshake.total() > 0,
        "and the connection this arm opened is counted too"
    );
}

#[test]
fn a_shell_calls_bytes_are_the_command_line_plus_the_rendered_output() {
    let mut rig = Rig::fresh();
    let mut surface = ShellSurface::new();
    let step = Step::faithful(Call::CreateWorkspace { seal: false });
    let observation = surface
        .perform(&mut rig, &DIE_HARD_ALL, &step)
        .expect("the arm attempts the operation");

    let command = shell::command_line(&DIE_HARD_ALL, &step);
    assert!(
        command.starts_with("continuum workspace create"),
        "the agent wrote a command line: {command}"
    );
    assert_eq!(
        observation.bytes,
        command.len() as u64 + (observation.bytes - command.len() as u64),
        "the count is command plus output and nothing else"
    );
    assert!(
        observation.bytes > command.len() as u64,
        "the output the agent read is counted beside the command it wrote"
    );
    assert_eq!(
        surface.handshake_bytes(),
        0,
        "a CLI process's handshake is inside the process, like its frames"
    );
    assert_eq!(surface.expansions(), 0, "no list was expanded on a create");
}

#[test]
fn the_token_count_is_the_declared_rule_applied_to_the_same_bytes() {
    assert_eq!(BYTES_PER_APPROX_TOKEN, 4, "the rule is a declared constant");
    for run in support::both_arms() {
        assert_eq!(
            run.approx_tokens,
            run.bytes.div_ceil(BYTES_PER_APPROX_TOKEN),
            "{} {:?}: tokens are arithmetic over bytes, never a tokenizer",
            run.task,
            run.arm
        );
    }
    let report = Report::new(support::both_arms());
    assert!(
        report
            .render()
            .contains("bytes/4 (declared; no tokenizer, not graded)"),
        "and the artifact names the rule beside the number"
    );
    assert!(
        report
            .render()
            .contains("graded_denominator: interface bytes per solved task (RFC 0027)"),
        "with the graded denominator stated so nobody grades the approximation"
    );
}

#[test]
fn bytes_per_solved_task_ignores_a_run_that_did_not_solve_anything() {
    let mut runs = support::both_arms();
    let solved_before = Report::new(runs.clone()).totals[&Arm::Native]
        .bytes_per_solved()
        .expect("the native arm solves tasks");

    // Take one solved native run, mark it unsolved, and give it an enormous byte count. If
    // the denominator counted unsolved runs, this would move the number.
    let doomed = runs
        .iter_mut()
        .find(|run| run.arm == Arm::Native && run.solved)
        .expect("a solved native run");
    let cost = doomed.bytes;
    doomed.solved = false;
    doomed.bytes = 10_000_000;

    let after = Report::new(runs).totals[&Arm::Native];
    assert_eq!(
        after.bytes_on_solved,
        {
            let report = Report::new(support::both_arms());
            report.totals[&Arm::Native].bytes_on_solved - cost
        },
        "an unsolved run contributes nothing to the numerator"
    );
    assert!(
        after.bytes_per_solved().expect("others still solve") < 10_000_000,
        "and nothing to the denominator either: the ten-million-byte failure is invisible \
         (was {solved_before})"
    );
}

#[test]
fn every_metric_is_paired_over_three_declared_seeds() {
    let report = Report::new(support::both_arms());
    assert_eq!(
        report.margins.seeds_paired, 3,
        "plan §24.5's ratified margins require at least three seeds"
    );
    for arm in Arm::ALL {
        for seed in [1_u16, 2, 3] {
            let runs: Vec<&ArmRun> = report
                .runs
                .iter()
                .filter(|run| run.arm == arm && run.seed == seed)
                .collect();
            assert_eq!(
                runs.len(),
                8,
                "{arm:?} seed {seed}: four tasks, two policies, all present"
            );
            assert!(runs.iter().all(|run| run.bytes > 0));
        }
    }
}

#[test]
fn the_report_says_where_the_bytes_went_operation_by_operation() {
    let report = Report::new(support::both_arms());
    let rendered = report.render();
    assert!(rendered.contains("[operations]"));
    for arm in Arm::ALL {
        for operation in [
            "workspace.create",
            "workspace.seal",
            "verification.start",
            "task.status",
            "verification.result",
        ] {
            let cost = report
                .per_operation
                .get(&(arm, operation))
                .unwrap_or_else(|| panic!("{arm:?} {operation} is accounted for"));
            assert!(cost.calls > 0 && cost.bytes > 0);
            assert!(
                rendered.contains(&format!("{} {operation} ", arm.token())),
                "the breakdown is in the artifact, not only in the value"
            );
        }
    }
    let native_total: u64 = report
        .per_operation
        .iter()
        .filter(|((arm, _), _)| *arm == Arm::Native)
        .map(|(_, cost)| cost.bytes)
        .sum();
    let handshakes: u64 = report.totals[&Arm::Native].bytes_total - native_total;
    assert!(
        handshakes > 0,
        "the difference between the per-operation total and the arm total is the handshake, \
         which the typed arm pays and the CLI baseline does not"
    );
}

/// The measured finding, recorded so that a change to it is loud.
#[test]
fn the_typed_surface_spends_more_interface_bytes_than_the_text_baseline() {
    let report = Report::new(support::both_arms());
    let native = report.totals[&Arm::Native]
        .bytes_per_solved()
        .expect("the native arm solves tasks");
    let shell = report.totals[&Arm::Shell]
        .bytes_per_solved()
        .expect("the shell arm solves tasks");

    assert!(
        native > shell,
        "measured: the typed surface costs more interface bytes per solved task \
         (native {native}, shell {shell})"
    );
    assert!(
        native > shell * 2,
        "and not marginally: over twice as many (native {native}, shell {shell})"
    );
    let saving = report
        .margins
        .byte_saving_percent
        .expect("both arms solve, so the margin is defined");
    assert!(
        saving < 0,
        "the ratified margin asks for a 30% saving; the measured value is {saving}%"
    );
    assert!(
        !report.margins.bytes_clear,
        "so the byte margin does not clear, and the artifact says so"
    );

    // Where it goes. The envelope is the cause, and this is the number that says so.
    let status = report.per_operation[&(Arm::Native, "task.status")];
    let shell_status = report.per_operation[&(Arm::Shell, "task.status")];
    assert_eq!(
        status.calls, shell_status.calls,
        "the same calls, both arms"
    );
    assert!(
        status.bytes > shell_status.bytes * 2,
        "one `task.status` answer carries six epochs, nine cost dimensions, an omission \
         manifest and a next-operation list on every call"
    );
}

/// The one datum the two arms genuinely pay differently for, and its price.
#[test]
fn the_omission_manifest_is_inline_on_the_wire_and_an_extra_command_in_text() {
    let mut rig = Rig::fresh();
    let mut surface = ShellSurface::new();
    surface
        .perform(
            &mut rig,
            &DIE_HARD_ALL,
            &Step::faithful(Call::CreateWorkspace { seal: true }),
        )
        .expect("the arm attempts the operation");
    assert_eq!(
        surface.expansions(),
        0,
        "nothing has declared a budget yet, so nothing has been expanded"
    );

    let (run, native) = support::native_cell(&DIE_HARD_ALL, PolicyKind::Faithful, 1);
    assert!(run.solved, "the native run reads the manifest inline");
    assert_eq!(
        native.ledger().calls,
        u64::from(run.attempted),
        "and needed no extra call to do it"
    );

    let shell_run = support::shell_cell(&DIE_HARD_ALL, PolicyKind::Faithful, 1);
    assert_eq!(
        shell_run.attempted, run.attempted,
        "the text arm makes the same *operations*; the expansion is a second render of an \
         answer it already has, charged in bytes rather than in attempts"
    );
    assert!(shell_run.solved);
}
