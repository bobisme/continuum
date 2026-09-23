//! PR-14-IMPL-01 (bn-lf4i): the task/region lifecycle journal, held to its exit property
//! and to the region calculus.
//!
//! > **Exit:** identical controlled choice logs produce canonical identical semantic
//! > events.
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 14
//!
//! # The grain these tests close at
//!
//! The substrate binding is a typed absence (`binding.rs`), so events come from the
//! crate's scripted source: per-actor scripts interleaved by a choice log. Every test
//! here is at that grain. The real-substrate grain stays open until the binding lands,
//! and `the_substrate_binding_is_a_typed_absence` pins that so it cannot be forgotten.
//!
//! # Evidence map
//!
//! | Claim | Test |
//! |---|---|
//! | identical choice logs → byte-identical journals and digests, every log | [`identical_choice_logs_produce_byte_identical_journals`] |
//! | different choice logs → different journals, every pair | [`different_choice_logs_produce_different_journals`] |
//! | alpha-renaming of labels is invisible in the bytes | [`alpha_renaming_of_labels_does_not_reach_the_journal`] |
//! | serialization round trip, and a pinned golden | [`serialization_round_trip_holds_and_the_encoding_is_pinned`] |
//! | the decoder refuses every second spelling | [`the_decoder_refuses_every_second_spelling`] |
//! | lift ≡ region calculus at every interleaving (differential) | [`the_lift_agrees_with_the_region_calculus_at_every_interleaving`] |
//! | the lift can say no (anti-vacuity) | [`the_lift_rejects_nonconforming_journals`] |
//! | a conforming teardown is total (no orphans) | [`a_conforming_teardown_is_total`] |
//! | typed refusal on an unsupported primitive | [`an_unsupported_primitive_is_a_typed_refusal`] |
//! | the extension point is the six families, one instrumented | [`the_extension_point_names_six_families_and_one_is_instrumented`] |
//! | malformed choice logs are typed refusals | [`malformed_choice_logs_are_typed_refusals`] |

use std::collections::BTreeMap;

use continuum_asupersync::binding::{BindingAbsence, SubstrateBinding, substrate_binding};
use continuum_asupersync::choice::ChoiceLog;
use continuum_asupersync::encoding::DecodeError;
use continuum_asupersync::family::lifecycle::{
    LifecycleEvent, LifecycleReport, RegionLabel, RegionOrdinal, TaskLabel, TaskOrdinal, TaskSet,
    TaskStep,
};
use continuum_asupersync::family::{EventBody, Family, Report};
use continuum_asupersync::journal::Journal;
use continuum_asupersync::lift::{LiftVerdict, Nonconformance, lift};
use continuum_asupersync::source::{RecordRefusal, Script, record};
use continuum_task::region::schedule::{Schedule, Step};
use continuum_task::region::worker::{Resumability, WorkerId, WorkerStep};
use continuum_task::region::{RegionFault, RegionId, RegionState, RegionTree};
use continuum_value::assurance::InconclusiveReason;

// --- programs ---------------------------------------------------------------------------

fn lc(report: LifecycleReport) -> Report {
    Report::Lifecycle(report)
}

/// Three actors: a task in the root, a child region with a task, and a teardown of the
/// child. `offset` shifts every label, for the alpha-renaming relation.
fn three_actors(offset: u32) -> Vec<Script> {
    let child = RegionLabel(1 + offset);
    let t_root = TaskLabel(10 + offset);
    let t_child = TaskLabel(20 + offset);
    vec![
        vec![
            lc(LifecycleReport::Spawn {
                region: RegionLabel::ROOT,
                task: t_root,
                resumability: Resumability::Resumable,
            }),
            lc(LifecycleReport::Step {
                task: t_root,
                step: TaskStep::Begin,
            }),
            lc(LifecycleReport::Step {
                task: t_root,
                step: TaskStep::Complete,
            }),
        ],
        vec![
            lc(LifecycleReport::OpenRegion {
                parent: RegionLabel::ROOT,
                child,
            }),
            lc(LifecycleReport::Spawn {
                region: child,
                task: t_child,
                resumability: Resumability::Resumable,
            }),
            lc(LifecycleReport::Step {
                task: t_child,
                step: TaskStep::Begin,
            }),
        ],
        vec![
            lc(LifecycleReport::Cancel { region: child }),
            lc(LifecycleReport::Drain { region: child }),
            lc(LifecycleReport::Finalize { region: child }),
        ],
    ]
}

/// A program whose every interleaving is recordable (no actor names another's label),
/// with a root teardown racing two workers: the space holds conforming and
/// nonconforming runs.
fn racing_teardown() -> Vec<Script> {
    let a = TaskLabel(1);
    let b = TaskLabel(2);
    vec![
        vec![
            lc(LifecycleReport::Spawn {
                region: RegionLabel::ROOT,
                task: a,
                resumability: Resumability::Resumable,
            }),
            lc(LifecycleReport::Step {
                task: a,
                step: TaskStep::Begin,
            }),
            lc(LifecycleReport::Step {
                task: a,
                step: TaskStep::Suspend,
            }),
        ],
        vec![
            lc(LifecycleReport::Spawn {
                region: RegionLabel::ROOT,
                task: b,
                resumability: Resumability::Resumable,
            }),
            lc(LifecycleReport::Step {
                task: b,
                step: TaskStep::Begin,
            }),
            lc(LifecycleReport::Step {
                task: b,
                step: TaskStep::Complete,
            }),
        ],
        vec![
            lc(LifecycleReport::Cancel {
                region: RegionLabel::ROOT,
            }),
            lc(LifecycleReport::Drain {
                region: RegionLabel::ROOT,
            }),
            lc(LifecycleReport::Finalize {
                region: RegionLabel::ROOT,
            }),
        ],
    ]
}

fn lengths(scripts: &[Script]) -> Vec<usize> {
    scripts.iter().map(Vec::len).collect()
}

// --- the exit property ------------------------------------------------------------------

#[test]
fn identical_choice_logs_produce_byte_identical_journals() {
    let logs = ChoiceLog::enumerate(&lengths(&three_actors(0)));
    assert_eq!(logs.len(), 1680, "9! / (3!·3!·3!) interleavings");
    let mut recorded = 0;
    for log in &logs {
        // Two independently built script sets, recorded separately.
        let first = record(&three_actors(0), log);
        let second = record(&three_actors(0), log);
        assert_eq!(first, second, "log {log}");
        if let (Ok(first), Ok(second)) = (first, second) {
            recorded += 1;
            assert_eq!(
                first.encode().unwrap(),
                second.encode().unwrap(),
                "log {log}"
            );
            assert_eq!(
                first.digest().unwrap(),
                second.digest().unwrap(),
                "log {log}"
            );
            assert_eq!(first.render(), second.render(), "log {log}");
        }
    }
    assert!(
        recorded > 100,
        "the determinism check ran over {recorded} journals"
    );
}

#[test]
fn different_choice_logs_produce_different_journals() {
    let scripts = racing_teardown();
    let logs = ChoiceLog::enumerate(&lengths(&scripts));
    let mut seen: BTreeMap<Vec<u8>, ChoiceLog> = BTreeMap::new();
    for log in &logs {
        let journal = record(&scripts, log).expect("every interleaving is recordable");
        let bytes = journal.encode().unwrap();
        if let Some(previous) = seen.insert(bytes, log.clone()) {
            panic!("logs {previous} and {log} produced the same journal");
        }
    }
    assert_eq!(seen.len(), logs.len());
    assert_eq!(seen.len(), 1680);
}

/// Metamorphic relation: alpha-renaming. Labels are program-local names; renaming every
/// one of them changes no byte of the journal, at every interleaving.
#[test]
fn alpha_renaming_of_labels_does_not_reach_the_journal() {
    let logs = ChoiceLog::enumerate(&lengths(&three_actors(0)));
    for log in &logs {
        let base = record(&three_actors(0), log).map(|j| j.encode().unwrap());
        let renamed = record(&three_actors(1000), log).map(|j| j.encode().unwrap());
        match (base, renamed) {
            (Ok(base), Ok(renamed)) => assert_eq!(base, renamed, "log {log}"),
            (Err(_), Err(_)) => {}
            (base, renamed) => panic!("log {log}: {base:?} vs {renamed:?}"),
        }
    }
}

// --- the encoding -----------------------------------------------------------------------

fn tiny_journal() -> Journal {
    let mut journal = Journal::new();
    let events = [
        LifecycleEvent::TaskSpawned {
            task: TaskOrdinal(0),
            region: RegionOrdinal(0),
            resumability: Resumability::Resumable,
        },
        LifecycleEvent::RegionCancelRequested {
            region: RegionOrdinal(0),
        },
        LifecycleEvent::RegionDrained {
            region: RegionOrdinal(0),
            cancelled: TaskSet::new([TaskOrdinal(0)]),
        },
        LifecycleEvent::RegionFinalized {
            region: RegionOrdinal(0),
        },
    ];
    for event in events {
        journal.append(EventBody::Lifecycle(event)).unwrap();
    }
    journal
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Metamorphic relation: serialization round trip.
#[test]
fn serialization_round_trip_holds_and_the_encoding_is_pinned() {
    let journal = tiny_journal();
    let bytes = journal.encode().unwrap();
    let magic = hex(b"continuum/semantic-journal\n");
    let expected = format!(
        "{magic}00000001\
         0000000000000004\
         0000000000000000 01 0000000a 02 00000000 00000000 01\
         0000000000000001 01 00000005 05 00000000\
         0000000000000002 01 0000000d 06 00000000 00000001 00000000\
         0000000000000003 01 00000005 07 00000000"
    )
    .replace(' ', "");
    assert_eq!(hex(&bytes), expected, "the canonical encoding moved");
    assert_eq!(Journal::decode(&bytes).unwrap(), journal);
    assert_eq!(
        journal.render(),
        "0 lifecycle task-spawned t0 region=r0 resumable\n\
         1 lifecycle region-cancel-requested r0\n\
         2 lifecycle region-drained r0 cancelled=[t0]\n\
         3 lifecycle region-finalized r0\n"
    );

    // And over every journal the three-actor program can record.
    for log in ChoiceLog::enumerate(&lengths(&three_actors(0))) {
        if let Ok(journal) = record(&three_actors(0), &log) {
            let bytes = journal.encode().unwrap();
            let decoded = Journal::decode(&bytes).unwrap();
            assert_eq!(decoded, journal, "log {log}");
            assert_eq!(decoded.encode().unwrap(), bytes, "log {log}");
        }
    }
}

#[test]
fn the_decoder_refuses_every_second_spelling() {
    let bytes = tiny_journal().encode().unwrap();
    let header = b"continuum/semantic-journal\n".len();

    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(matches!(
        Journal::decode(&trailing),
        Err(DecodeError::TrailingBytes { .. })
    ));

    assert!(matches!(
        Journal::decode(&bytes[..bytes.len() - 1]),
        Err(DecodeError::Truncated { .. })
    ));

    let mut version = bytes.clone();
    version[header + 3] = 2;
    assert!(matches!(
        Journal::decode(&version),
        Err(DecodeError::BadHeader { .. })
    ));

    // The first event's sequence number sits after magic, version and count.
    let first_seq = header + 4 + 8;
    let mut gap = bytes.clone();
    gap[first_seq + 7] = 1;
    assert_eq!(
        Journal::decode(&gap),
        Err(DecodeError::SequenceGap {
            expected: 0,
            found: 1
        })
    );

    let family_at = first_seq + 8;
    let mut unknown = bytes.clone();
    unknown[family_at] = 99;
    assert!(matches!(
        Journal::decode(&unknown),
        Err(DecodeError::UnknownTag {
            table: "family",
            tag: 99,
            ..
        })
    ));

    // A byte string claiming a family whose instrumentation has not landed.
    let mut uninstrumented = bytes.clone();
    uninstrumented[family_at] = Family::Effect.tag();
    assert_eq!(
        Journal::decode(&uninstrumented),
        Err(DecodeError::FamilyNotInstrumented {
            family: "reserve-commit-abort",
            seq: 0
        })
    );

    // A drained set spelled out of order.
    let mut journal = Journal::new();
    journal
        .append(EventBody::Lifecycle(LifecycleEvent::RegionDrained {
            region: RegionOrdinal(0),
            cancelled: TaskSet::new([TaskOrdinal(1), TaskOrdinal(2)]),
        }))
        .unwrap();
    let mut unsorted = journal.encode().unwrap();
    let len = unsorted.len();
    unsorted.swap(len - 1, len - 5);
    assert!(matches!(
        Journal::decode(&unsorted),
        Err(DecodeError::UnsortedSet { .. })
    ));
}

// --- conformance against continuum_task::region -----------------------------------------

/// The oracle's own reading of `(scripts, log)` as a region-calculus schedule. It binds
/// labels itself and calls nothing in the adapter, so it is a second path to the model.
fn oracle_schedule(scripts: &[Script], log: &ChoiceLog) -> Option<Schedule> {
    let mut cursors = vec![0_usize; scripts.len()];
    let mut regions: BTreeMap<RegionLabel, u32> = BTreeMap::from([(RegionLabel::ROOT, 0)]);
    let mut tasks: BTreeMap<TaskLabel, u32> = BTreeMap::new();
    let mut steps = Vec::new();
    for choice in log.choices() {
        let enabled: Vec<usize> = (0..scripts.len())
            .filter(|a| cursors[*a] < scripts[*a].len())
            .collect();
        let actor = enabled[choice.0 as usize];
        let Report::Lifecycle(report) = &scripts[actor][cursors[actor]] else {
            unreachable!("the programs here are lifecycle-only");
        };
        cursors[actor] += 1;
        let region = |label: &RegionLabel| regions.get(label).map(|o| RegionId::at(*o));
        steps.push(match report {
            LifecycleReport::OpenRegion { parent, child } => {
                let parent = region(parent)?;
                let next = u32::try_from(regions.len()).unwrap();
                regions.insert(*child, next);
                Step::OpenChild { parent }
            }
            LifecycleReport::Spawn {
                region: r,
                task,
                resumability,
            } => {
                let r = region(r)?;
                let next = u32::try_from(tasks.len()).unwrap();
                tasks.insert(*task, next);
                Step::Spawn {
                    region: r,
                    resumability: resumability.clone(),
                }
            }
            LifecycleReport::Step { task, step } => Step::Advance {
                worker: WorkerId::at(*tasks.get(task)?),
                step: step.to_worker_step(),
            },
            LifecycleReport::Close { region: r } => Step::Close { region: region(r)? },
            LifecycleReport::Cancel { region: r } => Step::Cancel { region: region(r)? },
            LifecycleReport::Drain { region: r } => Step::Drain { region: region(r)? },
            LifecycleReport::Finalize { region: r } => Step::Finalize { region: region(r)? },
        });
    }
    Some(Schedule::new(steps))
}

/// Differential: the journal path (record → encode → decode → lift) against the direct
/// path (`Schedule::run` on a fresh `RegionTree`), at every interleaving of two programs.
#[test]
fn the_lift_agrees_with_the_region_calculus_at_every_interleaving() {
    let mut conforming = 0;
    let mut violating = 0;
    let mut unrecordable = 0;
    for scripts in [three_actors(0), racing_teardown()] {
        for log in ChoiceLog::enumerate(&lengths(&scripts)) {
            let recorded = record(&scripts, &log);
            let Some(schedule) = oracle_schedule(&scripts, &log) else {
                assert!(
                    matches!(
                        recorded,
                        Err(RecordRefusal::UnboundRegion(_) | RecordRefusal::UnboundTask(_))
                    ),
                    "log {log}: the oracle could not bind a label, the recorder said {recorded:?}"
                );
                unrecordable += 1;
                continue;
            };
            let journal = recorded.unwrap_or_else(|r| panic!("log {log}: {r}"));
            let journal = Journal::decode(&journal.encode().unwrap()).unwrap();

            let mut direct = RegionTree::new();
            let run = schedule.run(&mut direct);
            let first_fault = run
                .records()
                .iter()
                .position(|(_, outcome)| outcome.is_err());

            match (lift(&journal), first_fault) {
                (LiftVerdict::Conforms(lifted), None) => {
                    conforming += 1;
                    assert_eq!(lifted.tree().worker_count(), direct.worker_count());
                    for w in 0..u32::try_from(direct.worker_count()).unwrap() {
                        assert_eq!(
                            lifted.tree().worker_state(WorkerId::at(w)).unwrap(),
                            direct.worker_state(WorkerId::at(w)).unwrap(),
                            "log {log}, w{w}"
                        );
                    }
                    let lifted: Vec<String> = lifted
                        .finalizations()
                        .iter()
                        .map(|(_, f)| f.render())
                        .collect();
                    let direct: Vec<String> =
                        run.finalizations().iter().map(|f| f.render()).collect();
                    assert_eq!(lifted, direct, "log {log}");
                }
                (LiftVerdict::Violates { seq, reason }, Some(index)) => {
                    violating += 1;
                    assert_eq!(seq, u64::try_from(index).unwrap(), "log {log}");
                    let fault = run.records()[index].1.clone().unwrap_err();
                    assert_eq!(reason, Nonconformance::Refused(fault), "log {log}");
                }
                (verdict, fault) => panic!("log {log}: lift {verdict:?}, direct fault {fault:?}"),
            }
        }
    }
    // Anti-vacuity: the space reaches all three dispositions.
    assert!(conforming > 50, "conforming runs: {conforming}");
    assert!(violating > 50, "violating runs: {violating}");
    assert!(unrecordable > 50, "unrecordable runs: {unrecordable}");
}

fn journal_of(events: Vec<LifecycleEvent>) -> Journal {
    let mut journal = Journal::new();
    for event in events {
        journal.append(EventBody::Lifecycle(event)).unwrap();
    }
    journal
}

#[test]
fn the_lift_rejects_nonconforming_journals() {
    let spawn = |task, region| LifecycleEvent::TaskSpawned {
        task: TaskOrdinal(task),
        region: RegionOrdinal(region),
        resumability: Resumability::Resumable,
    };

    // Work entering a closed region.
    let verdict = lift(&journal_of(vec![
        LifecycleEvent::RegionCloseRequested {
            region: RegionOrdinal(0),
        },
        spawn(0, 0),
    ]));
    assert!(matches!(
        verdict,
        LiftVerdict::Violates {
            seq: 1,
            reason: Nonconformance::Refused(RegionFault::SpawnIntoClosedRegion { .. })
        }
    ));

    // A drain that reports the wrong tasks cancelled.
    let verdict = lift(&journal_of(vec![
        spawn(0, 0),
        spawn(1, 0),
        LifecycleEvent::TaskStepped {
            task: TaskOrdinal(1),
            step: TaskStep::Begin,
        },
        LifecycleEvent::RegionCancelRequested {
            region: RegionOrdinal(0),
        },
        LifecycleEvent::RegionDrained {
            region: RegionOrdinal(0),
            cancelled: TaskSet::new([TaskOrdinal(1)]),
        },
    ]));
    match verdict {
        LiftVerdict::Violates {
            seq: 4,
            reason:
                Nonconformance::DrainOutcome {
                    region: 0,
                    reported,
                    model,
                },
        } => {
            assert_eq!(reported, vec![1]);
            assert_eq!(model, vec![0, 1]);
        }
        other => panic!("{other:?}"),
    }

    // A region or task named by an ordinal the model did not allocate.
    let verdict = lift(&journal_of(vec![LifecycleEvent::RegionOpened {
        region: RegionOrdinal(5),
        parent: RegionOrdinal(0),
    }]));
    assert!(matches!(
        verdict,
        LiftVerdict::Violates {
            seq: 0,
            reason: Nonconformance::RegionIdentity {
                journal: 5,
                model: 1
            }
        }
    ));
    let verdict = lift(&journal_of(vec![spawn(3, 0)]));
    assert!(matches!(
        verdict,
        LiftVerdict::Violates {
            seq: 0,
            reason: Nonconformance::TaskIdentity {
                journal: 3,
                model: 0
            }
        }
    ));

    // Finalizing over a running task: the no-orphan gate.
    let verdict = lift(&journal_of(vec![
        spawn(0, 0),
        LifecycleEvent::TaskStepped {
            task: TaskOrdinal(0),
            step: TaskStep::Begin,
        },
        LifecycleEvent::RegionCloseRequested {
            region: RegionOrdinal(0),
        },
        LifecycleEvent::RegionFinalized {
            region: RegionOrdinal(0),
        },
    ]));
    assert!(matches!(
        verdict,
        LiftVerdict::Violates {
            seq: 3,
            reason: Nonconformance::Refused(RegionFault::FinalizeBeforeTermination { .. })
        }
    ));
}

#[test]
fn a_conforming_teardown_is_total() {
    let scripts = racing_teardown();
    let mut totals = 0;
    for log in ChoiceLog::enumerate(&lengths(&scripts)) {
        let journal = record(&scripts, &log).unwrap();
        if let Some(lifted) = lift(&journal).conforming() {
            let (_, root) = lifted
                .finalizations()
                .last()
                .expect("the teardown actor always finalizes the root");
            assert!(root.is_total(), "log {log}: {}", root.render());
            assert_eq!(
                lifted.tree().state(RegionId::at(0)).unwrap(),
                RegionState::Finalized
            );
            assert!(lifted.tree().ledger().is_balanced(), "log {log}");
            for w in 0..u32::try_from(lifted.tree().worker_count()).unwrap() {
                assert!(
                    lifted
                        .tree()
                        .worker_state(WorkerId::at(w))
                        .unwrap()
                        .is_terminal()
                );
            }
            totals += 1;
        }
    }
    assert!(totals > 0, "some interleaving tears down conformingly");
}

// --- refusals and the extension point ---------------------------------------------------

#[test]
fn an_unsupported_primitive_is_a_typed_refusal() {
    let scripts = vec![vec![
        lc(LifecycleReport::Spawn {
            region: RegionLabel::ROOT,
            task: TaskLabel(1),
            resumability: Resumability::Resumable,
        }),
        Report::Uninstrumented {
            family: Family::Effect,
            operation: "reserve".to_owned(),
        },
    ]];
    let refusal = record(&scripts, &ChoiceLog::new([0, 0])).unwrap_err();
    assert_eq!(
        refusal,
        RecordRefusal::UnsupportedPrimitive {
            family: Family::Effect,
            operation: "reserve".to_owned()
        }
    );
    assert_eq!(
        refusal.inconclusive_reason(),
        Some(InconclusiveReason::Unsupported)
    );
    assert!(refusal.to_string().contains("PR-14-IMPL-02"));
    // A malformed input is not an inconclusive result.
    assert_eq!(RecordRefusal::UnboundTask(1).inconclusive_reason(), None);
}

#[test]
fn the_extension_point_names_six_families_and_one_is_instrumented() {
    let table: Vec<(u8, &str, &str, bool)> = Family::ALL
        .iter()
        .map(|f| (f.tag(), f.token(), f.requirement(), f.is_instrumented()))
        .collect();
    assert_eq!(
        table,
        vec![
            (1, "lifecycle", "PR-14-IMPL-01", true),
            (2, "reserve-commit-abort", "PR-14-IMPL-02", false),
            (3, "cancellation", "PR-14-IMPL-03", false),
            (4, "obligation", "PR-14-IMPL-04", false),
            (5, "virtual-time", "PR-14-IMPL-05", false),
            (6, "channel", "PR-14-IMPL-06", false),
        ]
    );
    for family in Family::ALL {
        assert_eq!(Family::from_tag(family.tag()), Some(family));
    }
    assert_eq!(Family::from_tag(0), None);
    assert_eq!(Family::from_tag(7), None);
    // The lifecycle family's steps are the region calculus's, minus reserve/commit.
    assert_eq!(TaskStep::Begin.to_worker_step(), WorkerStep::Begin);
}

#[test]
fn malformed_choice_logs_are_typed_refusals() {
    let scripts = three_actors(0);
    assert_eq!(
        record(&scripts, &ChoiceLog::new([3])),
        Err(RecordRefusal::ChoiceOutOfRange {
            position: 0,
            choice: 3,
            enabled: 3
        })
    );
    assert_eq!(
        record(&scripts, &ChoiceLog::new([0])),
        Err(RecordRefusal::ChoiceLogExhausted { remaining: 8 })
    );
    let single = vec![vec![lc(LifecycleReport::Close {
        region: RegionLabel::ROOT,
    })]];
    assert_eq!(
        record(&single, &ChoiceLog::new([0, 0])),
        Err(RecordRefusal::ChoiceLogOverrun { position: 1 })
    );
    assert_eq!(record(&[], &ChoiceLog::default()), Ok(Journal::new()));
}

#[test]
fn the_substrate_binding_is_a_typed_absence() {
    let binding = substrate_binding();
    assert_eq!(
        binding,
        SubstrateBinding::Absent(BindingAbsence::DependencyNotDeclared)
    );
    assert_eq!(
        binding.inconclusive_reason(),
        InconclusiveReason::Unsupported
    );
}
