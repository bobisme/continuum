//! PR-14-IMPL-01 (bn-lf4i): the lifecycle journal observed from the real substrate.
//!
//! > **Exit:** identical controlled choice logs produce canonical identical semantic
//! > events.
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 14
//!
//! These tests close the exit sentence at the grain of real asupersync 0.5.0 events: the
//! lab runtime runs the programs, the choice log decides the interleaving, and the
//! journal is read back from the substrate's own trace (`binding.rs`).
//!
//! # Evidence map
//!
//! | Claim | Test |
//! |---|---|
//! | identical choice logs → byte-identical journals from the substrate, every log | [`identical_choice_logs_give_byte_identical_substrate_journals`] |
//! | the lab seed does not reach the journal (metamorphic) | [`the_lab_seed_does_not_reach_the_journal`] |
//! | different choice logs → different journals (anti-vacuity) | [`different_choice_logs_give_different_substrate_journals`] |
//! | substrate journal ≡ scripted-source journal, every log (differential) | [`the_substrate_agrees_byte_for_byte_with_the_scripted_source`] |
//! | every substrate journal lifts, and no finalization leaves an orphan | [`every_substrate_journal_conforms_to_the_region_calculus`] |
//! | a normal close waits for owned work, as the calculus requires | [`a_normal_close_with_live_work_waits_as_the_calculus_requires`] |
//! | only the lifecycle family is bound; the other five are typed absences | [`only_the_lifecycle_family_is_bound`] |
//! | malformed programs, logs and illegal calls are typed refusals | [`malformed_programs_and_illegal_calls_are_typed_refusals`] |
//! | no ambient crashpack path is reachable from the binding | [`the_binding_cannot_reach_the_ambient_crashpack_writers`] |

use std::collections::BTreeMap;

use continuum_asupersync::binding::{
    BindingAbsence, BindingConfig, BindingRefusal, Program, SubstrateBinding, SubstrateOp, run,
    substrate_binding,
};
use continuum_asupersync::choice::ChoiceLog;
use continuum_asupersync::family::lifecycle::{
    LifecycleEvent, LifecycleReport, RegionLabel, RegionOrdinal, TaskLabel, TaskOrdinal, TaskStep,
};
use continuum_asupersync::family::{EventBody, Family, Report};
use continuum_asupersync::journal::Journal;
use continuum_asupersync::lift::{LiftVerdict, lift};
use continuum_asupersync::source::{Script, record};
use continuum_task::region::worker::Resumability;
use continuum_value::assurance::InconclusiveReason;

const SEED: u64 = 0;

fn config(seed: u64) -> BindingConfig {
    BindingConfig::new(seed)
}

fn spawn(region: RegionLabel, task: TaskLabel) -> SubstrateOp {
    SubstrateOp::Spawn {
        region,
        task,
        resumability: Resumability::Resumable,
    }
}

/// Three actors: a root task that suspends twice and completes, a child region whose
/// task is cancelled while suspended, and a child region cancelled before its task ever
/// ran. Every interleaving is a legal run: no actor names another's label.
fn cancelling() -> Vec<Program> {
    let (a, b, c) = (TaskLabel(1), TaskLabel(2), TaskLabel(3));
    let (r1, r2) = (RegionLabel(1), RegionLabel(2));
    vec![
        vec![
            spawn(RegionLabel::ROOT, a),
            SubstrateOp::Begin { task: a },
            SubstrateOp::Continue { task: a },
            SubstrateOp::Finish { task: a },
        ],
        vec![
            SubstrateOp::OpenRegion {
                parent: RegionLabel::ROOT,
                child: r1,
            },
            spawn(r1, b),
            SubstrateOp::Begin { task: b },
            SubstrateOp::Cancel { region: r1 },
        ],
        vec![
            SubstrateOp::OpenRegion {
                parent: RegionLabel::ROOT,
                child: r2,
            },
            spawn(r2, c),
            SubstrateOp::Cancel { region: r2 },
        ],
    ]
}

/// Two actors: a root task, and a child region whose task completes before a normal
/// close.
fn closing() -> Vec<Program> {
    let (a, b) = (TaskLabel(1), TaskLabel(2));
    let r1 = RegionLabel(1);
    vec![
        vec![
            spawn(RegionLabel::ROOT, a),
            SubstrateOp::Begin { task: a },
            SubstrateOp::Finish { task: a },
        ],
        vec![
            SubstrateOp::OpenRegion {
                parent: RegionLabel::ROOT,
                child: r1,
            },
            spawn(r1, b),
            SubstrateOp::Begin { task: b },
            SubstrateOp::Finish { task: b },
            SubstrateOp::Close { region: r1 },
        ],
    ]
}

fn lengths(programs: &[Program]) -> Vec<usize> {
    programs.iter().map(Vec::len).collect()
}

/// Every (program set, log) pair the exhaustive tests run.
fn corpus() -> Vec<(&'static str, Vec<Program>, Vec<ChoiceLog>)> {
    let cancelling = cancelling();
    let closing = closing();
    let cancelling_logs = ChoiceLog::enumerate(&lengths(&cancelling));
    let closing_logs = ChoiceLog::enumerate(&lengths(&closing));
    assert_eq!(cancelling_logs.len(), 11_550, "11! / (4!·4!·3!)");
    assert_eq!(closing_logs.len(), 56, "8! / (3!·5!)");
    vec![
        ("cancelling", cancelling, cancelling_logs),
        ("closing", closing, closing_logs),
    ]
}

// --- the exit property, at the substrate's grain -------------------------------------

#[test]
fn identical_choice_logs_give_byte_identical_substrate_journals() {
    let mut compared = 0;
    for (name, programs, logs) in corpus() {
        for log in &logs {
            // Two runs, each on a fresh lab runtime.
            let first = run(&programs, log, &config(SEED)).expect("every log is a legal run");
            let second = run(&programs, log, &config(SEED)).expect("every log is a legal run");
            let (first_bytes, second_bytes) = (first.encode().unwrap(), second.encode().unwrap());
            assert_eq!(first_bytes, second_bytes, "{name} log {log}");
            assert_eq!(
                first.digest().unwrap(),
                second.digest().unwrap(),
                "{name} log {log}"
            );
            assert!(!first.is_empty(), "{name} log {log}: an empty journal");
            compared += 1;
        }
    }
    assert_eq!(compared, 11_550 + 56);
}

#[test]
fn the_lab_seed_does_not_reach_the_journal() {
    let seeds = [0, 1, 42, 0x9e37_79b9_7f4a_7c15, u64::MAX];
    let mut compared = 0;
    for (name, programs, logs) in corpus() {
        // Every 37th log, and the first and last, keeps the run time small.
        let sample = logs
            .iter()
            .enumerate()
            .filter(|(index, _)| index % 37 == 0 || *index + 1 == logs.len())
            .map(|(_, log)| log);
        for log in sample {
            let reference = run(&programs, log, &config(seeds[0]))
                .unwrap()
                .encode()
                .unwrap();
            for seed in &seeds[1..] {
                let other = run(&programs, log, &config(*seed))
                    .unwrap()
                    .encode()
                    .unwrap();
                assert_eq!(reference, other, "{name} log {log} seed {seed:#x}");
                compared += 1;
            }
        }
    }
    assert!(compared > 1000, "the relation ran over {compared} pairs");
}

#[test]
fn different_choice_logs_give_different_substrate_journals() {
    for (name, programs, logs) in corpus() {
        let mut seen: BTreeMap<Vec<u8>, ChoiceLog> = BTreeMap::new();
        for log in &logs {
            let bytes = run(&programs, log, &config(SEED))
                .unwrap()
                .encode()
                .unwrap();
            if let Some(previous) = seen.insert(bytes, log.clone()) {
                panic!("{name}: logs {previous} and {log} produced the same journal");
            }
        }
        assert_eq!(seen.len(), logs.len(), "{name}");
    }
}

// --- the differential: substrate against the scripted source -------------------------

/// The scripted reports one substrate operation stands for, when every call succeeds
/// and the calculus's teardown completes: the scripted source's account of the same run.
fn reports(op: &SubstrateOp) -> Vec<Report> {
    let step =
        |task: TaskLabel, step: TaskStep| Report::Lifecycle(LifecycleReport::Step { task, step });
    let teardown = |region: RegionLabel, first: LifecycleReport| {
        vec![
            Report::Lifecycle(first),
            Report::Lifecycle(LifecycleReport::Drain { region }),
            Report::Lifecycle(LifecycleReport::Finalize { region }),
        ]
    };
    match op {
        SubstrateOp::OpenRegion { parent, child } => {
            vec![Report::Lifecycle(LifecycleReport::OpenRegion {
                parent: *parent,
                child: *child,
            })]
        }
        SubstrateOp::Spawn {
            region,
            task,
            resumability,
        } => vec![Report::Lifecycle(LifecycleReport::Spawn {
            region: *region,
            task: *task,
            resumability: resumability.clone(),
        })],
        SubstrateOp::Begin { task } => {
            vec![step(*task, TaskStep::Begin), step(*task, TaskStep::Suspend)]
        }
        SubstrateOp::Continue { task } => {
            vec![
                step(*task, TaskStep::Resume),
                step(*task, TaskStep::Suspend),
            ]
        }
        SubstrateOp::Finish { task } => {
            vec![
                step(*task, TaskStep::Resume),
                step(*task, TaskStep::Complete),
            ]
        }
        SubstrateOp::Close { region } => {
            teardown(*region, LifecycleReport::Close { region: *region })
        }
        SubstrateOp::Cancel { region } => {
            teardown(*region, LifecycleReport::Cancel { region: *region })
        }
    }
}

/// The scripts and the choice log that make the scripted source take the same actor
/// order as `log` takes over `programs`, one operation's reports at a time.
fn scripted(programs: &[Program], log: &ChoiceLog) -> (Vec<Script>, ChoiceLog) {
    let scripts: Vec<Script> = programs
        .iter()
        .map(|program| program.iter().flat_map(reports).collect())
        .collect();
    let mut op_cursor = vec![0_usize; programs.len()];
    let mut report_cursor = vec![0_usize; scripts.len()];
    let mut choices = Vec::new();
    for choice in log.choices() {
        let enabled: Vec<usize> = (0..programs.len())
            .filter(|actor| op_cursor[*actor] < programs[*actor].len())
            .collect();
        let actor = enabled[choice.0 as usize];
        let count = reports(&programs[actor][op_cursor[actor]]).len();
        for _ in 0..count {
            let enabled: Vec<usize> = (0..scripts.len())
                .filter(|a| report_cursor[*a] < scripts[*a].len())
                .collect();
            let index = enabled.iter().position(|a| *a == actor).unwrap();
            choices.push(u32::try_from(index).unwrap());
            report_cursor[actor] += 1;
        }
        op_cursor[actor] += 1;
    }
    (scripts, ChoiceLog::new(choices))
}

#[test]
fn the_substrate_agrees_byte_for_byte_with_the_scripted_source() {
    let mut compared = 0;
    for (name, programs, logs) in corpus() {
        for log in &logs {
            let substrate = run(&programs, log, &config(SEED)).unwrap();
            let (scripts, scripted_log) = scripted(&programs, log);
            let model = record(&scripts, &scripted_log).expect("the translation is recordable");
            assert_eq!(
                substrate.encode().unwrap(),
                model.encode().unwrap(),
                "{name} log {log}\nsubstrate:\n{}scripted:\n{}",
                substrate.render(),
                model.render()
            );
            compared += 1;
        }
    }
    assert_eq!(compared, 11_550 + 56);
}

#[test]
fn every_substrate_journal_conforms_to_the_region_calculus() {
    for (name, programs, logs) in corpus() {
        let expected_finalizations = match name {
            "cancelling" => 2,
            _ => 1,
        };
        for log in &logs {
            let journal = run(&programs, log, &config(SEED)).unwrap();
            let LiftVerdict::Conforms(lifted) = lift(&journal) else {
                panic!("{name} log {log}:\n{}", journal.render());
            };
            assert_eq!(
                lifted.finalizations().len(),
                expected_finalizations,
                "{name} log {log}"
            );
            // These are child-region teardowns while the root still owns work, so the
            // ledger balance (a whole-tree fact) is not expected. The subtree's own two
            // accountings are: no orphan and no unresolved publication.
            for (seq, finalization) in lifted.finalizations() {
                assert!(
                    finalization.orphans().is_empty()
                        && finalization.unresolved_publications().is_empty(),
                    "{name} log {log}: finalization at {seq}:\n{}",
                    finalization.render()
                );
            }
        }
    }
}

/// asupersync's normal close does not cancel owned work: the region waits for it. The
/// region calculus says the same thing (a close drain blocks on non-terminal work). So
/// the journal stops at the close request, and it lifts to a region still draining.
#[test]
fn a_normal_close_with_live_work_waits_as_the_calculus_requires() {
    let (b, r1) = (TaskLabel(2), RegionLabel(1));
    let programs = vec![vec![
        SubstrateOp::OpenRegion {
            parent: RegionLabel::ROOT,
            child: r1,
        },
        spawn(r1, b),
        SubstrateOp::Begin { task: b },
        SubstrateOp::Close { region: r1 },
    ]];
    let journal = run(&programs, &ChoiceLog::new([0, 0, 0, 0]), &config(SEED)).unwrap();
    let bodies: Vec<&EventBody> = journal.events().iter().map(|e| e.body()).collect();
    assert_eq!(
        bodies.last(),
        Some(&&EventBody::Lifecycle(
            LifecycleEvent::RegionCloseRequested {
                region: RegionOrdinal(1)
            }
        )),
        "{}",
        journal.render()
    );
    let LiftVerdict::Conforms(lifted) = lift(&journal) else {
        panic!("{}", journal.render());
    };
    assert!(lifted.finalizations().is_empty());
    // Finishing the task lets the close complete, and the teardown is then total.
    let mut finishing = programs.clone();
    finishing[0].insert(3, SubstrateOp::Finish { task: b });
    let journal = run(&finishing, &ChoiceLog::new([0; 5]), &config(SEED)).unwrap();
    let LiftVerdict::Conforms(lifted) = lift(&journal) else {
        panic!("{}", journal.render());
    };
    assert_eq!(lifted.finalizations().len(), 1, "{}", journal.render());
    // The only work in the tree was in r1, so here the ledger balances too.
    assert!(
        lifted.finalizations()[0].1.is_total(),
        "{}",
        lifted.finalizations()[0].1.render()
    );
    assert!(journal.events().iter().any(|e| e.body()
        == &EventBody::Lifecycle(LifecycleEvent::TaskStepped {
            task: TaskOrdinal(0),
            step: TaskStep::Complete
        })));
}

// --- what is bound, and the refusals -------------------------------------------------

/// Regression guard for the binding landing: before it, every family (and the substrate
/// as a whole) was the typed absence `DependencyNotDeclared`.
#[test]
fn only_the_lifecycle_family_is_bound() {
    for family in Family::ALL {
        let binding = substrate_binding(family);
        if family == Family::Lifecycle {
            assert_eq!(binding, SubstrateBinding::Bound);
            assert_eq!(binding.inconclusive_reason(), None);
        } else {
            assert_eq!(
                binding,
                SubstrateBinding::Absent(BindingAbsence::FamilyNotBound(family))
            );
            assert_eq!(
                binding.inconclusive_reason(),
                Some(InconclusiveReason::Unsupported)
            );
            assert_eq!(
                BindingAbsence::FamilyNotBound(family).token(),
                "family-not-bound"
            );
        }
    }
}

#[test]
fn malformed_programs_and_illegal_calls_are_typed_refusals() {
    let (a, r1) = (TaskLabel(1), RegionLabel(1));
    let cases: Vec<(Vec<Program>, ChoiceLog, BindingRefusal)> = vec![
        (
            vec![vec![SubstrateOp::Begin { task: a }]],
            ChoiceLog::new([0]),
            BindingRefusal::UnboundTask(1),
        ),
        (
            vec![vec![spawn(r1, a)]],
            ChoiceLog::new([0]),
            BindingRefusal::UnboundRegion(1),
        ),
        (
            vec![vec![
                spawn(RegionLabel::ROOT, a),
                spawn(RegionLabel::ROOT, a),
            ]],
            ChoiceLog::new([0, 0]),
            BindingRefusal::TaskLabelRebound(1),
        ),
        (
            vec![vec![
                SubstrateOp::OpenRegion {
                    parent: RegionLabel::ROOT,
                    child: r1,
                },
                SubstrateOp::OpenRegion {
                    parent: RegionLabel::ROOT,
                    child: r1,
                },
            ]],
            ChoiceLog::new([0, 0]),
            BindingRefusal::RegionLabelRebound(1),
        ),
        (
            vec![vec![spawn(RegionLabel::ROOT, a)]],
            ChoiceLog::new([1]),
            BindingRefusal::ChoiceOutOfRange {
                position: 0,
                choice: 1,
                enabled: 1,
            },
        ),
        (
            vec![vec![spawn(RegionLabel::ROOT, a)]],
            ChoiceLog::new([]),
            BindingRefusal::ChoiceLogExhausted { remaining: 1 },
        ),
        (
            vec![vec![spawn(RegionLabel::ROOT, a)]],
            ChoiceLog::new([0, 0]),
            BindingRefusal::ChoiceLogOverrun { position: 1 },
        ),
    ];
    for (programs, log, expected) in cases {
        let refusal = run(&programs, &log, &config(SEED)).unwrap_err();
        assert_eq!(refusal, expected);
        assert_eq!(refusal.inconclusive_reason(), None, "{refusal}");
    }

    // An illegal call: spawning into a region whose close has completed. The substrate
    // refuses it, as the calculus would, and the run is refused with no journal.
    let programs = vec![vec![
        SubstrateOp::OpenRegion {
            parent: RegionLabel::ROOT,
            child: r1,
        },
        SubstrateOp::Close { region: r1 },
        spawn(r1, a),
    ]];
    let refusal = run(&programs, &ChoiceLog::new([0, 0, 0]), &config(SEED)).unwrap_err();
    assert!(
        matches!(
            refusal,
            BindingRefusal::SubstrateRefused {
                operation: "spawn",
                ..
            }
        ),
        "{refusal}"
    );
    assert_eq!(refusal.inconclusive_reason(), None);

    // A trace buffer too small for the run is a telemetry refusal, never a short journal.
    let tiny = BindingConfig {
        trace_capacity: 2,
        ..config(SEED)
    };
    let refusal = run(&closing(), &ChoiceLog::new([0; 8]), &tiny).unwrap_err();
    assert!(
        matches!(refusal, BindingRefusal::TraceOverflow { .. }),
        "{refusal}"
    );
    assert_eq!(
        refusal.inconclusive_reason(),
        Some(InconclusiveReason::InsufficientTelemetry)
    );
}

/// INV-005: no ambient filesystem output reaches a deterministic run. asupersync's only
/// crashpack writers are its lab-test harness and the `write_auto_crashpack*` methods;
/// the binding's code names neither, and the workspace turns the writes off for every
/// process cargo runs.
#[test]
fn the_binding_cannot_reach_the_ambient_crashpack_writers() {
    let source = include_str!("../src/binding.rs");
    let code: String = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for writer in [
        "write_auto_crashpack",
        "run_async_lab_test",
        "run_async_under_lab",
        "lab_test_config_from_env",
        "FileCrashPackWriter",
        "std::fs",
        "std::env",
    ] {
        assert!(!code.contains(writer), "binding.rs names {writer}");
    }
    // The anti-vacuity half: the scan does see code.
    assert!(code.contains("LabRuntime::new"));
    assert_eq!(
        std::env::var("ASUPERSYNC_AUTO_ARTIFACTS").as_deref(),
        Ok("0"),
        ".cargo/config.toml [env] must turn auto-artifacts off"
    );
}

#[test]
fn a_substrate_journal_round_trips_through_its_encoding() {
    let journal = run(&closing(), &ChoiceLog::new([0; 8]), &config(SEED)).unwrap();
    let bytes = journal.encode().unwrap();
    assert_eq!(Journal::decode(&bytes).unwrap(), journal);
}
