//! Evidence for `PR-10-IMPL-05` — deterministic reproduction (bn-26tb).
//!
//! > - deterministic reproduction.
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 10
//!
//! # The device: two fresh runs, nothing shared
//!
//! Every claim here is made by running a scenario **twice against two independently built
//! rigs** and comparing bytes. Nothing is shared between the two runs but the script: two
//! daemons, two capability tables, two staged corpora, two connections, two clients. So an
//! agreement cannot be explained by a cache, a counter that survived, a map's internal
//! layout, or an address — the failure modes a same-daemon replay would miss.
//!
//! That is a stronger device than replaying one request against one already-built daemon,
//! and it is the same one `continuumd`'s PR-8 exit evidence uses for the same reason.
//!
//! # Where the determinism comes from
//!
//! INV-005, held at four places, each of which this file checks rather than assumes:
//!
//! - **no clock** — `rig::NOW` is a constant and the daemon is built with it;
//! - **no entropy** — request identifiers and idempotency keys are minted from a monotone
//!   call counter (typed arm) or from a digest of the command line (text arm);
//! - **no ambient schedule** — a "seed" selects a *declared* [`Schedule`], not a generator;
//! - **no float, no map order, no path** in the artifact — `report::Report::render` is
//!   documented to that effect and [`the_rendered_report_carries_nothing_machine_specific`]
//!   holds it to it.
//!
//! # Clause → test
//!
//! - **the same scenario reproduces byte for byte** →
//!   [`two_fresh_runs_of_one_cell_produce_identical_transcripts_and_metrics`].
//! - **on both arms** → same test, over both.
//! - **the whole artifact reproduces** →
//!   [`two_full_sweeps_render_byte_identical_reports`].
//! - **the comparison can fail** →
//!   [`two_different_scenarios_do_not_render_the_same_report`] and
//!   [`a_single_changed_byte_in_a_transcript_is_visible`]. A byte comparison that could not
//!   distinguish two different runs would prove nothing about either.
//! - **the daemon itself is deterministic under this rig** →
//!   [`two_fresh_rigs_negotiate_byte_identical_handshake_frames`].
//! - **seeds are declared, not drawn** → [`the_three_seeds_are_a_closed_declared_set`].

mod support;

use continuum_benchmark::policy::{PolicyKind, SCHEDULES, Schedule};
use continuum_benchmark::report::Report;
use continuum_benchmark::rig::Rig;
use continuum_benchmark::run::ArmRun;
use continuum_benchmark::task::{DIE_HARD_ALL, PHILOSOPHERS_ALL, SUBSET};

/// Everything about a run that is not its transcript, as one comparable value.
fn metrics(run: &ArmRun) -> (u32, u32, u32, u32, u64, u64, bool, bool) {
    (
        run.attempted,
        run.admitted,
        run.invalid,
        run.zero_cost_invalid,
        run.bytes,
        run.approx_tokens,
        run.solved,
        run.recovered,
    )
}

#[test]
fn two_fresh_runs_of_one_cell_produce_identical_transcripts_and_metrics() {
    for task in [&DIE_HARD_ALL, &PHILOSOPHERS_ALL] {
        for kind in PolicyKind::ALL {
            for seed in [1_u16, 2, 3] {
                let (first, _) = support::native_cell(task, kind, seed);
                let (second, _) = support::native_cell(task, kind, seed);
                assert_eq!(
                    first.transcript, second.transcript,
                    "native {} {kind:?} seed {seed}: two fresh daemons, one transcript",
                    task.id
                );
                assert_eq!(metrics(&first), metrics(&second));
                assert_eq!(first.per_operation, second.per_operation);

                let first = support::shell_cell(task, kind, seed);
                let second = support::shell_cell(task, kind, seed);
                assert_eq!(
                    first.transcript, second.transcript,
                    "shell {} {kind:?} seed {seed}: two fresh daemons, one transcript",
                    task.id
                );
                assert_eq!(metrics(&first), metrics(&second));
                assert_eq!(first.per_operation, second.per_operation);
            }
        }
    }
}

#[test]
fn two_full_sweeps_render_byte_identical_reports() {
    let first = Report::new(support::both_arms()).render();
    let second = Report::new(support::both_arms()).render();
    assert_eq!(
        first.as_bytes(),
        second.as_bytes(),
        "the whole artifact is a function of the script"
    );
    assert_eq!(
        Report::render_expectations(),
        Report::render_expectations(),
        "and so is the frozen-answer table it is graded against"
    );
    assert!(first.contains("accepted: true"));
}

#[test]
fn two_different_scenarios_do_not_render_the_same_report() {
    // Anti-vacuity. If the rendering collapsed differences, every comparison above would be
    // trivially true.
    let full = Report::new(support::both_arms()).render();

    let mut trimmed = support::both_arms();
    trimmed.retain(|run| run.seed != 3);
    let partial = Report::new(trimmed).render();
    assert_ne!(full, partial, "dropping a seed must change the artifact");

    let faithful_only: Vec<ArmRun> = support::both_arms()
        .into_iter()
        .filter(|run| run.policy == PolicyKind::Faithful)
        .collect();
    assert_ne!(
        full,
        Report::new(faithful_only).render(),
        "dropping the fallible policy must change the artifact"
    );
}

#[test]
fn a_single_changed_byte_in_a_transcript_is_visible() {
    let (first, _) = support::native_cell(&DIE_HARD_ALL, PolicyKind::Faithful, 1);
    let mut second = first.clone();
    let line = second
        .transcript
        .first_mut()
        .expect("the run attempted something");
    line.push('.');
    assert_ne!(
        first.transcript, second.transcript,
        "the comparison this file rests on is byte-exact"
    );
}

#[test]
fn two_fresh_rigs_negotiate_byte_identical_handshake_frames() {
    let first = Rig::fresh();
    let second = Rig::fresh();
    assert_eq!(
        first.hello_frame(),
        second.hello_frame(),
        "one hello, whoever builds it"
    );
    assert_eq!(
        first.welcome_frame(),
        second.welcome_frame(),
        "and one welcome — no clock, no entropy, no host in the bytes"
    );
    assert_eq!(
        first.intent(),
        second.intent(),
        "one governing intent identity"
    );
    for task in SUBSET {
        assert_eq!(
            first.components(task.source),
            second.components(task.source),
            "{}: the staged corpus is content-addressed, so two rigs name it identically",
            task.id
        );
    }
}

#[test]
fn the_three_seeds_are_a_closed_declared_set() {
    assert_eq!(
        SCHEDULES.len(),
        3,
        "plan §24.5 asks for at least three seeds"
    );
    let seeds: Vec<u16> = SCHEDULES.iter().map(|schedule| schedule.seed).collect();
    assert_eq!(
        seeds,
        vec![1, 2, 3],
        "declared, in order, with no generator"
    );
    // Each schedule differs from the others in something that changes a run, or it would be
    // a duplicate wearing a different number.
    for (index, left) in SCHEDULES.iter().enumerate() {
        for right in &SCHEDULES[index + 1..] {
            assert_ne!(
                shape(left),
                shape(right),
                "seeds must differ in what they do"
            );
        }
    }
}

fn shape(schedule: &Schedule) -> (bool, u32, Vec<&'static str>) {
    (
        schedule.result_first,
        schedule.extra_polls,
        schedule.faults.iter().map(|fault| fault.token()).collect(),
    )
}

#[test]
fn the_rendered_report_carries_nothing_machine_specific() {
    let rendered = Report::new(support::both_arms()).render();
    for line in rendered.lines() {
        for token in line.split_whitespace() {
            assert!(
                !token.starts_with('/') && !token.contains('\\'),
                "no filesystem path reaches the artifact: {line}"
            );
        }
    }
    assert!(
        !rendered.contains(".rs") && !rendered.contains("target/"),
        "and no source or build location either"
    );
    assert!(!rendered.contains("0x"), "no address reaches the artifact");
    assert!(
        !rendered.contains('.') || !rendered.lines().any(is_float_line),
        "no float reaches the artifact"
    );
    assert!(
        rendered.lines().all(str::is_ascii),
        "the artifact is ASCII, so no locale reaches it"
    );
}

/// Whether a line contains something that renders like a float.
fn is_float_line(line: &str) -> bool {
    line.split_whitespace().any(|word| {
        let mut parts = word.split('.');
        matches!(
            (parts.next(), parts.next(), parts.next()),
            (Some(left), Some(right), None)
                if !left.is_empty()
                    && !right.is_empty()
                    && left.bytes().all(|byte| byte.is_ascii_digit())
                    && right.bytes().all(|byte| byte.is_ascii_digit())
        )
    })
}
