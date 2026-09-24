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
//! | the lab seed does not reach the journal (metamorphic), sibling regions included | [`the_lab_seed_does_not_reach_the_journal`] |
//! | the seed moves the substrate's sibling close order; the journal does not follow it (mutant) | [`sibling_close_order_is_the_substrates_and_the_journal_does_not_follow_it`] |
//! | different choice logs → different journals (anti-vacuity) | [`different_choice_logs_give_different_substrate_journals`] |
//! | substrate journal ≡ scripted-source journal, every log (differential) | [`the_substrate_agrees_byte_for_byte_with_the_scripted_source`] |
//! | every substrate journal lifts, and no finalization leaves an orphan | [`every_substrate_journal_conforms_to_the_region_calculus`] |
//! | a normal close waits for owned work, as the calculus requires | [`a_normal_close_with_live_work_waits_as_the_calculus_requires`] |
//! | every family is bound; the typed absence remains for a later family | [`every_family_is_bound`] |
//! | malformed programs, logs and illegal calls are typed refusals | [`malformed_programs_and_illegal_calls_are_typed_refusals`] |
//! | no ambient crashpack path is reachable from the binding | [`the_binding_cannot_reach_the_ambient_crashpack_writers`] |

use std::collections::BTreeMap;

use continuum_asupersync::binding::{
    BindingAbsence, BindingConfig, BindingRefusal, Program, SubstrateBinding, SubstrateOp, run,
    run_witnessed, substrate_binding,
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

/// Two actors: a root task that runs to completion, and a cancelled region with two
/// child regions, one of which has a child of its own:
///
/// ```text
/// r1 ─┬─ r2 ── r4
///     └─ r3
/// ```
///
/// with a task in each of `r2`, `r3` and `r4`. The substrate's seeded scheduler picks
/// the order in which `r2`'s and `r3`'s closes complete. Every interleaving is a legal
/// run: the second actor opens every region it cancels.
fn branching() -> Vec<Program> {
    let (a, b, c, d) = (TaskLabel(1), TaskLabel(2), TaskLabel(3), TaskLabel(4));
    let (r1, r2, r3, r4) = (
        RegionLabel(1),
        RegionLabel(2),
        RegionLabel(3),
        RegionLabel(4),
    );
    let open = |parent, child| SubstrateOp::OpenRegion { parent, child };
    vec![
        vec![
            spawn(RegionLabel::ROOT, a),
            SubstrateOp::Begin { task: a },
            SubstrateOp::Finish { task: a },
        ],
        vec![
            open(RegionLabel::ROOT, r1),
            open(r1, r2),
            open(r1, r3),
            open(r2, r4),
            spawn(r2, b),
            spawn(r3, c),
            spawn(r4, d),
            SubstrateOp::Begin { task: b },
            SubstrateOp::Cancel { region: r1 },
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
    let branching = branching();
    let cancelling_logs = ChoiceLog::enumerate(&lengths(&cancelling));
    let closing_logs = ChoiceLog::enumerate(&lengths(&closing));
    let branching_logs = ChoiceLog::enumerate(&lengths(&branching));
    assert_eq!(cancelling_logs.len(), 11_550, "11! / (4!·4!·3!)");
    assert_eq!(closing_logs.len(), 56, "8! / (3!·5!)");
    assert_eq!(branching_logs.len(), BRANCHING_LOGS, "12! / (3!·9!)");
    vec![
        ("cancelling", cancelling, cancelling_logs),
        ("closing", closing, closing_logs),
        ("branching", branching, branching_logs),
    ]
}

const BRANCHING_LOGS: usize = 220;
const ALL_LOGS: usize = 11_550 + 56 + BRANCHING_LOGS;

/// The seeds every seed-invariance test runs. 7 and 99 reverse the close order of
/// sibling regions against 0 and 1 in asupersync 0.5.0.
const SEEDS: [u64; 7] = [0, 1, 7, 42, 99, 0x9e37_79b9_7f4a_7c15, u64::MAX];

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
    assert_eq!(compared, ALL_LOGS);
}

#[test]
fn the_lab_seed_does_not_reach_the_journal() {
    let seeds = SEEDS;
    let mut compared = 0;
    for (name, programs, logs) in corpus() {
        // Every 37th log, and the first and last, keeps the run time small. The
        // branching corpus, where sibling order is the substrate's, runs in full.
        let sample = logs
            .iter()
            .enumerate()
            .filter(|(index, _)| name == "branching" || index % 37 == 0 || *index + 1 == logs.len())
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
    assert!(compared > 3000, "the relation ran over {compared} pairs");
}

/// The journal a binding that followed the substrate's raw close order would write:
/// the drain/finalize pairs of the cancelled subtree, reordered as `raw` lists them.
fn raw_order_mutant(journal: &Journal, raw: &[RegionOrdinal]) -> Journal {
    let bodies: Vec<EventBody> = journal.events().iter().map(|e| e.body().clone()).collect();
    let first = bodies
        .iter()
        .position(|b| {
            matches!(
                b,
                EventBody::Lifecycle(LifecycleEvent::RegionDrained { .. })
            )
        })
        .unwrap();
    let pair = |region: RegionOrdinal| {
        let at = bodies
            .iter()
            .position(|b| {
                matches!(b, EventBody::Lifecycle(LifecycleEvent::RegionDrained { region: r, .. }) if *r == region)
            })
            .unwrap();
        bodies[at..at + 2].to_vec()
    };
    let mut mutated = bodies[..first].to_vec();
    for region in raw {
        mutated.extend(pair(*region));
    }
    mutated.extend_from_slice(&bodies[first + 2 * raw.len()..]);
    let mut out = Journal::new();
    for body in mutated {
        out.append(body).unwrap();
    }
    out
}

/// The mutant the canonical close order kills. In the branching corpus the substrate's
/// own close order for the sibling regions `r2` and `r3` changes with the lab seed, so
/// a binding that journaled drains in raw trace order would leak the seed: its journals
/// for seeds 0 and 7 differ. The binding's journal is the same for every seed.
#[test]
fn sibling_close_order_is_the_substrates_and_the_journal_does_not_follow_it() {
    let programs = branching();
    let canonical = vec![
        RegionOrdinal(4),
        RegionOrdinal(2),
        RegionOrdinal(3),
        RegionOrdinal(1),
    ];
    let mut leaks = 0;
    for log in ChoiceLog::enumerate(&lengths(&programs)) {
        let mut journals = BTreeMap::new();
        let mut mutants = BTreeMap::new();
        for seed in SEEDS {
            let witnessed = run_witnessed(&programs, &log, &config(seed)).unwrap();
            let raw = witnessed.substrate_close_order.clone();
            let mut sorted = raw.clone();
            sorted.sort_unstable();
            assert_eq!(sorted, {
                let mut c = canonical.clone();
                c.sort_unstable();
                c
            });
            let drains: Vec<RegionOrdinal> = witnessed
                .journal
                .events()
                .iter()
                .filter_map(|e| match e.body() {
                    EventBody::Lifecycle(LifecycleEvent::RegionDrained { region, .. }) => {
                        Some(*region)
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(drains, canonical, "log {log} seed {seed}");
            let mutant = raw_order_mutant(&witnessed.journal, &raw);
            if raw == canonical {
                assert_eq!(mutant, witnessed.journal, "log {log} seed {seed}");
            }
            mutants.insert(seed, mutant.encode().unwrap());
            journals.insert(seed, witnessed.journal.encode().unwrap());
        }
        assert!(journals.values().all(|j| j == &journals[&0]), "log {log}");
        if mutants[&0] != mutants[&7] {
            leaks += 1;
        }
    }
    // Anti-vacuity: the raw-order mutant leaks the seed on real runs.
    assert!(
        leaks > 0,
        "the raw-order mutant never differed between seeds 0 and 7"
    );
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

/// The regions under `root` that `program` opened before `index`, children before
/// parents and siblings in open order: the canonical close order of a cancelled
/// subtree. Every region a corpus actor cancels is opened by that actor, so open order
/// within the program is ordinal order.
fn post_order(program: &Program, index: usize, root: RegionLabel) -> Vec<RegionLabel> {
    let mut out = Vec::new();
    for op in &program[..index] {
        if let SubstrateOp::OpenRegion { parent, child } = op
            && *parent == root
        {
            out.extend(post_order(program, index, *child));
        }
    }
    out.push(root);
    out
}

/// The scripted reports one substrate operation stands for, when every call succeeds
/// and the calculus's teardown completes: the scripted source's account of the same run.
fn reports(program: &Program, index: usize) -> Vec<Report> {
    let op = &program[index];
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
            let mut out = vec![Report::Lifecycle(LifecycleReport::Cancel {
                region: *region,
            })];
            for member in post_order(program, index, *region) {
                out.push(Report::Lifecycle(LifecycleReport::Drain { region: member }));
                out.push(Report::Lifecycle(LifecycleReport::Finalize {
                    region: member,
                }));
            }
            out
        }
        SubstrateOp::Reserve { .. }
        | SubstrateOp::Commit { .. }
        | SubstrateOp::Abort { .. }
        | SubstrateOp::Acquire { .. }
        | SubstrateOp::Transfer { .. }
        | SubstrateOp::Sleep { .. }
        | SubstrateOp::Advance { .. }
        | SubstrateOp::OpenChannel { .. }
        | SubstrateOp::Send { .. }
        | SubstrateOp::Recv { .. }
        | SubstrateOp::CloseSenders { .. }
        | SubstrateOp::Crash { .. }
        | SubstrateOp::SpawnWithDeadline { .. } => {
            unreachable!("the lifecycle corpus has no effect operations")
        }
    }
}

/// The scripts and the choice log that make the scripted source take the same actor
/// order as `log` takes over `programs`, one operation's reports at a time.
fn scripted(programs: &[Program], log: &ChoiceLog) -> (Vec<Script>, ChoiceLog) {
    let scripts: Vec<Script> = programs
        .iter()
        .map(|program| {
            (0..program.len())
                .flat_map(|index| reports(program, index))
                .collect()
        })
        .collect();
    let mut op_cursor = vec![0_usize; programs.len()];
    let mut report_cursor = vec![0_usize; scripts.len()];
    let mut choices = Vec::new();
    for choice in log.choices() {
        let enabled: Vec<usize> = (0..programs.len())
            .filter(|actor| op_cursor[*actor] < programs[*actor].len())
            .collect();
        let actor = enabled[choice.0 as usize];
        let count = reports(&programs[actor], op_cursor[actor]).len();
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
    assert_eq!(compared, ALL_LOGS);
}

#[test]
fn every_substrate_journal_conforms_to_the_region_calculus() {
    for (name, programs, logs) in corpus() {
        let expected_finalizations = match name {
            "cancelling" => 2,
            "branching" => 4,
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
/// as a whole) was the typed absence `DependencyNotDeclared`. PR-14-IMPL-03 (bn-bx7i)
/// bound the cancellation family too, and PR-14-IMPL-02 (bn-gzy1) the reserve/commit/abort
/// family, PR-14-IMPL-04 (bn-6nm8) the obligations family, and PR-14-IMPL-05 (bn-3m1d)
/// the virtual time family, and PR-14-IMPL-06 (bn-3xx9) the channel family: every family
/// is bound. The typed absence stays for a family a later PR adds.
#[test]
fn every_family_is_bound() {
    for family in Family::ALL {
        let binding = substrate_binding(family);
        assert_eq!(binding, SubstrateBinding::Bound);
        assert_eq!(binding.inconclusive_reason(), None);
        let absent = SubstrateBinding::Absent(BindingAbsence::FamilyNotBound(family));
        assert_eq!(
            absent.inconclusive_reason(),
            Some(InconclusiveReason::Unsupported)
        );
        assert_eq!(
            BindingAbsence::FamilyNotBound(family).token(),
            "family-not-bound"
        );
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
        // Regression guards (bn-iey9f): a command to a task that is not polling its gate
        // used to wait unseen, and the journal lost the operation. Now it is refused.
        (
            vec![vec![
                spawn(RegionLabel::ROOT, a),
                SubstrateOp::Continue { task: a },
            ]],
            ChoiceLog::new([0, 0]),
            BindingRefusal::TaskNotBegun { task: 0 },
        ),
        (
            vec![vec![
                spawn(RegionLabel::ROOT, a),
                SubstrateOp::Begin { task: a },
                SubstrateOp::Finish { task: a },
                SubstrateOp::Continue { task: a },
            ]],
            ChoiceLog::new([0, 0, 0, 0]),
            BindingRefusal::TaskEnded { task: 0 },
        ),
        (
            vec![vec![
                spawn(RegionLabel::ROOT, a),
                SubstrateOp::Begin { task: a },
                SubstrateOp::Begin { task: a },
            ]],
            ChoiceLog::new([0, 0, 0]),
            BindingRefusal::TaskAlreadyBegun { task: 0 },
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
