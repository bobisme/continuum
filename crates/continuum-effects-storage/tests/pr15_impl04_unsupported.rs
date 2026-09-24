//! PR-15 / IMPL-04 (bn-1oj6): the declared unsupported cases of `storage/append-log-v0`.
//!
//! Every [`Support::Unsupported`] row of the profile has one [`UnsupportedCase`] in
//! [`UNSUPPORTED`]: the host behaviour it leaves out, the [`Request`] by which a caller
//! could ask for it, and the [`Reliance`] that says what a verdict gives a program that
//! depends on it. These tests hold the declaration to the Lab handler.
//!
//! Evidence, by artifact id:
//!
//! | Id | Test | What it shows |
//! |---|---|---|
//! | `pr15-impl04-hon-01` | [`honesty_every_unsupported_row_has_exactly_one_consistent_declared_case`] | the table is total over the unsupported rows, in order, and every reliance names a stated assumption or has no operation |
//! | `pr15-impl04-hon-03` | [`honesty_the_profile_registry_binds_each_name_to_one_fingerprint`] | cr-37bshu: each profile name is bound to one fingerprint; the frozen `-v0` is byte for byte the pre-bn-1oj6 profile |
//! | `pr15-impl04-hon-02` | [`honesty_a_no_operation_case_has_no_step_and_no_configuration_value`] | compiler-level: every step, every configuration value, every entry a reader sees and every refusal is classified by an exhaustive destructuring match, and none asks for or reports a no-operation case |
//! | `pr15-impl04-pos-01` | [`positive_every_requestable_case_is_refused_exactly_as_declared`] | each requestable case, through `apply`, `check` and `run`, is its declared `Unsupported` refusal, INV-008 `Unsupported`, and changes nothing |
//! | `pr15-impl04-pos-02..04` | `case_*` | per case: the declared refusal (positive), a modelled neighbour that still works (negative), and the edge between them (boundary) |
//! | `pr15-impl04-bnd-01` | [`boundary_at_the_step_bound_a_requestable_case_is_the_step_bound_refusal`] | the declared exception: at the configured step bound every requestable case is `BoundReached(Steps)`, inconclusive; one step below it, the declared `Unsupported` |
//! | `pr15-impl04-pos-05` | [`no_operation_a_tear_always_reads_as_torn`] | the `detected-tear` assumption as behaviour: every tear a crash can leave reads `Torn`, never another intact value |

use continuum_effects_storage::profile::ASSUMPTIONS;
use continuum_effects_storage::refusal::{Bound, Malformed, NotEnabled, ProgramFault};
use continuum_effects_storage::step::MAX_RETAINED_CAP;
use continuum_effects_storage::{
    APPEND_LOG_V0, APPEND_LOG_V1, Entry, Event, NodeId, PROFILES, Refusal, RefusalClass, Reliance,
    Request, RunRefusal, Semantic, Step, StepKind, Storage, StorageConfig, Support, TicketId,
    UNSUPPORTED, UnsupportedCase, Value, run,
};

fn config(crashes: u32) -> StorageConfig {
    StorageConfig::new(2, crashes, true, true, true, 8, MAX_RETAINED_CAP).unwrap()
}

fn submit(node: u8, value: u32) -> Step {
    Step::Submit {
        node: NodeId(node),
        value: Value(value),
    }
}

/// n0's log: record 0 stable (synced, persisted, acked as `s0`), records 1 and 2
/// volatile, and `s1` open over all three and not yet persisted.
fn live(cfg: StorageConfig) -> Storage {
    let mut storage = Storage::new(cfg);
    for step in [
        submit(0, 10),
        Step::Sync(NodeId(0)),
        Step::Persist(TicketId(0)),
        Step::Ack(TicketId(0)),
        submit(0, 11),
        submit(0, 12),
        Step::Sync(NodeId(0)),
    ] {
        storage.apply(&step).unwrap();
    }
    assert_eq!(storage.stable_len(NodeId(0)), Some(1));
    storage
}

fn case(semantic: Semantic) -> UnsupportedCase {
    semantic
        .unsupported_case()
        .unwrap_or_else(|| panic!("{semantic} has no declared case"))
}

/// Ask `storage` for `step`, expect exactly the declared refusal of `semantic`'s case
/// through `check`, `apply` and `run`, and expect nothing to change.
fn assert_refused_as_declared(storage: &mut Storage, step: &Step, semantic: Semantic) {
    let declared = case(semantic).refusal().expect("a requestable case");
    assert_eq!(declared, Refusal::Unsupported(semantic));
    let before = storage.clone();
    assert_eq!(storage.check(step), Err(declared), "{step:?}");
    assert_eq!(storage.apply(step), Err(declared), "{step:?}");
    assert_eq!(*storage, before, "{step:?} changed the storage pack");
    assert_eq!(declared.class(), RefusalClass::Unsupported);
    assert_eq!(declared.inconclusive_reason(), Some("Unsupported"));
    assert!(declared.to_string().contains(semantic.statement()));
    let mut log: Vec<Step> = storage.events().iter().map(Event::step).collect();
    log.push(*step);
    assert_eq!(
        run(*storage.config(), &log),
        Err(RunRefusal {
            index: log.len() - 1,
            refusal: declared
        })
    );
}

// --- the declaration --------------------------------------------------------------------

/// `pr15-impl04-hon-01`. The declared cases are exactly the unsupported rows, one each,
/// in [`Semantic::ALL`] order, and each is internally consistent: a `Step` request names
/// the kind whose [`Step::unsupported_semantic`] is the row, an `Assumed` id is a stated
/// assumption, a `Subsumed` row is modelled, and `OutsidePack` has no operation. The
/// two assumptions IMPL-04 added each back a case.
#[test]
fn honesty_every_unsupported_row_has_exactly_one_consistent_declared_case() {
    let unsupported: Vec<Semantic> = Semantic::ALL
        .into_iter()
        .filter(|s| s.support() == Support::Unsupported)
        .collect();
    let declared: Vec<Semantic> = UNSUPPORTED.iter().map(|c| c.semantic).collect();
    assert_eq!(declared, unsupported);
    for semantic in Semantic::ALL {
        assert_eq!(
            semantic.unsupported_case().is_some(),
            semantic.support() == Support::Unsupported,
            "{semantic}"
        );
    }
    let assumption_ids: Vec<&str> = ASSUMPTIONS.iter().map(|(id, _)| *id).collect();
    for case in UNSUPPORTED {
        assert!(case.host_behaviour.len() > 30, "{}", case.semantic);
        match case.request {
            Request::Step(kind) => {
                let probe = probe_of(kind);
                assert_eq!(probe.kind(), kind);
                assert_eq!(probe.unsupported_semantic(), Some(case.semantic));
            }
            Request::NoOperation => assert_eq!(case.refusal(), None),
        }
        match case.reliance {
            Reliance::Assumed(id) => assert!(assumption_ids.contains(&id), "{id}"),
            Reliance::Subsumed(row) => assert_eq!(row.support(), Support::Modelled),
            Reliance::Owner(row) => panic!("no storage case is owned elsewhere, got {row}"),
            Reliance::OutsidePack => assert_eq!(case.request, Request::NoOperation),
        }
    }
    // Every case's reliance, stated here as well as in the fingerprint, so a changed
    // reliance fails a named assertion.
    assert_eq!(
        UNSUPPORTED.map(|c| (c.semantic, c.reliance)),
        [
            (
                Semantic::SectorCorruption,
                Reliance::Assumed("no-medium-corruption")
            ),
            (
                Semantic::WriteReordering,
                Reliance::Assumed("durable-log-is-a-prefix")
            ),
            (Semantic::FlushDishonesty, Reliance::Assumed("honest-flush")),
            (
                Semantic::DirectoryDurability,
                Reliance::Assumed("durable-log-entry")
            ),
            (
                Semantic::DeviceProfile,
                Reliance::Assumed("no-device-profile")
            ),
            (Semantic::UndetectedTear, Reliance::Assumed("detected-tear")),
        ],
        "a declared reliance changed"
    );
}

/// One step of each kind, well formed against [`live`].
fn probe_of(kind: StepKind) -> Step {
    match kind {
        StepKind::Submit => submit(1, 20),
        StepKind::Sync => Step::Sync(NodeId(1)),
        StepKind::Persist => Step::Persist(TicketId(1)),
        StepKind::Ack => Step::Ack(TicketId(1)),
        StepKind::Delay => Step::Delay(TicketId(1)),
        StepKind::Crash => Step::Crash {
            node: NodeId(0),
            keep: 1,
            torn: false,
        },
        StepKind::Restart => Step::Restart(NodeId(1)),
        StepKind::Truncate => Step::Truncate(NodeId(0)),
        StepKind::Corrupt => Step::Corrupt {
            node: NodeId(0),
            index: 0,
        },
        StepKind::Reorder => Step::Reorder(NodeId(0)),
        StepKind::FalseFlush => Step::FalseFlush(TicketId(1)),
    }
}

/// Which profile row a step asks for, from its variant and every one of its fields.
/// Each arm destructures the variant completely, with no `..` and no wildcard arm, so a
/// new step, or a new field on one (a file name, a gap in a crash), stops this test
/// compiling until it is classified here. `Crash` carries only `keep` and `torn`, so
/// what it leaves is a prefix with at most a torn last record: no gap is expressible.
fn asks_for(step: &Step) -> Semantic {
    match step {
        Step::Submit {
            node: NodeId(_),
            value: Value(_),
        } => Semantic::SubmittedWrites,
        Step::Sync(NodeId(_)) => Semantic::OrderingAndBarriers,
        Step::Persist(TicketId(_)) => Semantic::StableStorage,
        Step::Ack(TicketId(_)) | Step::Delay(TicketId(_)) => Semantic::SyncAcknowledgement,
        Step::Crash {
            node: NodeId(_),
            keep: _,
            torn: _,
        } => Semantic::VolatileSuffixLoss,
        Step::Restart(NodeId(_)) => Semantic::CrashRestart,
        Step::Truncate(NodeId(_)) => Semantic::RecoveryProcedure,
        Step::Corrupt {
            node: NodeId(_),
            index: _,
        } => Semantic::SectorCorruption,
        Step::Reorder(NodeId(_)) => Semantic::WriteReordering,
        Step::FalseFlush(TicketId(_)) => Semantic::FlushDishonesty,
    }
}

/// Which profile row each configuration value sets. [`StorageConfig::new`] and
/// [`StorageConfig::with_max_steps`] are called with their whole argument lists, so a
/// new configuration value stops this test compiling until it is classified here.
fn configuration_rows() -> Vec<Semantic> {
    let (nodes, max_crashes, restart, suffix_loss, torn, max_pending, max_retained_bytes) =
        (2_u8, 2_u32, true, true, true, 8_u32, MAX_RETAINED_CAP);
    let cfg = StorageConfig::new(
        nodes,
        max_crashes,
        restart,
        suffix_loss,
        torn,
        max_pending,
        max_retained_bytes,
    )
    .unwrap()
    .with_max_steps(64)
    .unwrap();
    assert_eq!(cfg.max_steps(), 64);
    vec![
        // nodes: one log per node.
        Semantic::SubmittedWrites,
        // max_crashes, restart, suffix_loss, torn.
        Semantic::CrashRestart,
        Semantic::CrashRestart,
        Semantic::VolatileSuffixLoss,
        Semantic::TornWrites,
        // max_pending, max_retained_bytes, max_steps.
        Semantic::ExplorationBounds,
        Semantic::ExplorationBounds,
        Semantic::ExplorationBounds,
    ]
}

/// What a reader sees at a log position. [`Entry`] is matched exhaustively: a record is
/// intact with its own value or `Torn`, and there is no third reading, such as an intact
/// record with another value.
fn reading(entry: Entry) -> Semantic {
    match entry {
        Entry::Intact(Value(_)) => Semantic::AtomicityGranularity,
        Entry::Torn => Semantic::TornWrites,
    }
}

/// What a refusal reports. [`Refusal`] and every enum inside it are matched
/// exhaustively, with no wildcard arm: the only program-visible failure of a write is
/// the modelled write over a torn tail, no refusal reports a failed write or flush of
/// the host (the `no-device-profile` assumption), and a new refusal variant stops this
/// test compiling until it is classified here.
fn reported(refusal: &Refusal) -> Option<Semantic> {
    match refusal {
        Refusal::Unsupported(semantic) => Some(*semantic),
        Refusal::NotEnabled(
            NotEnabled::NodeDown(NodeId(_))
            | NotEnabled::NodeUp(NodeId(_))
            | NotEnabled::NoTornTail(NodeId(_))
            | NotEnabled::NotPending(TicketId(_))
            | NotEnabled::AlreadyStable(TicketId(_))
            | NotEnabled::NotStable(TicketId(_))
            | NotEnabled::IncarnationCrashed(TicketId(_))
            | NotEnabled::FaultNotDeclared(_)
            | NotEnabled::CrashBudgetSpent { max: _ },
        )
        | Refusal::Malformed(
            Malformed::UnknownNode(NodeId(_))
            | Malformed::KeepBeyondVolatile {
                keep: _,
                volatile: _,
            }
            | Malformed::NothingToTear,
        )
        | Refusal::BoundReached(
            Bound::Pending { max: _ }
            | Bound::Retained { max: _ }
            | Bound::Tickets
            | Bound::Positions
            | Bound::Epochs
            | Bound::Steps { max: _ },
        ) => None,
        Refusal::ProgramFault(ProgramFault::WriteOverTornTail(NodeId(_))) => {
            Some(Semantic::RecoveryProcedure)
        }
    }
}

/// `pr15-impl04-hon-02`. A no-operation case cannot be asked for through the public
/// API. The request surfaces are the step (`Storage::check`, `Storage::apply`, `run`)
/// and the configuration (`StorageConfig::new`, `with_max_steps`); [`asks_for`] and
/// [`configuration_rows`] classify every variant, field and argument of both, and the
/// compiler holds them total. No step and no configuration value asks for a
/// no-operation row. [`reading`] and [`reported`] hold the two outputs total the same
/// way: no reading is an undetected tear, and no refusal is a device error. That the
/// public surface has no other entry point that takes input is the recorded INV-015
/// inventory's claim, which is lexical.
#[test]
fn honesty_a_no_operation_case_has_no_step_and_no_configuration_value() {
    let from_steps: Vec<Semantic> = StepKind::ALL
        .into_iter()
        .map(|kind| asks_for(&probe_of(kind)))
        .collect();
    let from_config = configuration_rows();
    for row in &from_config {
        assert_eq!(row.support(), Support::Modelled, "{row}");
    }
    let readings = [reading(Entry::Intact(Value(1))), reading(Entry::Torn)];
    let reports: Vec<Semantic> = [
        Refusal::ProgramFault(ProgramFault::WriteOverTornTail(NodeId(0))),
        Refusal::NotEnabled(NotEnabled::NodeDown(NodeId(0))),
    ]
    .iter()
    .filter_map(reported)
    .collect();
    let mut no_operation = 0;
    for case in UNSUPPORTED {
        match case.request {
            Request::NoOperation => {
                no_operation += 1;
                for surface in [&from_steps[..], &from_config, &readings, &reports] {
                    assert!(
                        !surface.contains(&case.semantic),
                        "{} is declared no-operation",
                        case.semantic
                    );
                }
            }
            Request::Step(kind) => assert_eq!(asks_for(&probe_of(kind)), case.semantic),
        }
    }
    assert_eq!(no_operation, 3);
    for kind in StepKind::ALL {
        assert_eq!(probe_of(kind).kind(), kind);
    }
}

/// `pr15-impl04-pos-01`. Every case with a request, asked for through the public API,
/// is refused with exactly its declared refusal, and the pack does not change.
#[test]
fn positive_every_requestable_case_is_refused_exactly_as_declared() {
    let mut refused = 0;
    for case in UNSUPPORTED {
        let mut storage = live(config(2));
        match case.request {
            Request::Step(kind) => {
                assert_refused_as_declared(&mut storage, &probe_of(kind), case.semantic);
                refused += 1;
            }
            Request::NoOperation => {}
        }
    }
    assert_eq!(refused, 3);
}

// --- one test per requestable case ------------------------------------------------------

/// `pr15-impl04-pos-02`. Corrupting a stored record is refused. The modelled damage, a
/// crash that tears the first lost volatile record, is accepted and reads `Torn`.
/// Boundary: the stable record next to it is never corrupted — `Corrupt` of it is
/// refused, and after the crash it reads intact with its own value.
#[test]
fn case_sector_corruption_is_refused_and_only_a_crash_tears_a_volatile_record() {
    let mut storage = live(config(2));
    for index in [0, 1, 2, 9] {
        assert_refused_as_declared(
            &mut storage,
            &Step::Corrupt {
                node: NodeId(0),
                index,
            },
            Semantic::SectorCorruption,
        );
    }
    let crash = Step::Crash {
        node: NodeId(0),
        keep: 0,
        torn: true,
    };
    assert!(matches!(
        storage.apply(&crash).copied(),
        Ok(Event::Crashed {
            keep: 0,
            torn: true,
            lost: 2,
            ..
        })
    ));
    assert_eq!(
        storage.log(NodeId(0)),
        Some(&[Entry::Intact(Value(10)), Entry::Torn][..])
    );
    assert_refused_as_declared(
        &mut storage,
        &Step::Corrupt {
            node: NodeId(0),
            index: 0,
        },
        Semantic::SectorCorruption,
    );
}

/// `pr15-impl04-pos-03`. Reordering writes is refused. What a crash keeps is a prefix
/// of the submitted log, for every `keep`, which the `durable-log-is-a-prefix`
/// assumption states. Boundary: `keep` equal to the volatile count loses nothing, and
/// one more is `Malformed`, so no crash can reach past the prefix; `Reorder` of an
/// empty log is still refused.
#[test]
fn case_write_reordering_is_refused_and_a_crash_keeps_a_prefix() {
    assert_eq!(
        case(Semantic::WriteReordering).reliance,
        Reliance::Assumed("durable-log-is-a-prefix")
    );
    let mut storage = live(config(2));
    assert_refused_as_declared(
        &mut storage,
        &Step::Reorder(NodeId(0)),
        Semantic::WriteReordering,
    );
    assert_refused_as_declared(
        &mut storage,
        &Step::Reorder(NodeId(1)),
        Semantic::WriteReordering,
    );
    let submitted = [Value(10), Value(11), Value(12)];
    for keep in 0..=2 {
        let mut crashed = storage.clone();
        crashed
            .apply(&Step::Crash {
                node: NodeId(0),
                keep,
                torn: false,
            })
            .unwrap();
        let log: Vec<Entry> = crashed.log(NodeId(0)).unwrap().to_vec();
        let prefix: Vec<Entry> = submitted[..=keep as usize]
            .iter()
            .copied()
            .map(Entry::Intact)
            .collect();
        assert_eq!(log, prefix, "keep {keep}");
    }
    assert!(
        storage
            .check(&Step::Crash {
                node: NodeId(0),
                keep: 3,
                torn: false,
            })
            .is_err()
    );
}

/// `pr15-impl04-pos-04`. A dishonest flush is refused. The honest flush, `Persist`,
/// is modelled, and only after it does `Ack` vouch for the records. Boundary: `Ack`
/// before `Persist` is `NotEnabled`, and `FalseFlush` is refused both before and after
/// the honest flush.
#[test]
fn case_flush_dishonesty_is_refused_and_an_ack_vouches_only_after_persist() {
    let mut storage = live(config(2));
    let false_flush = Step::FalseFlush(TicketId(1));
    assert_refused_as_declared(&mut storage, &false_flush, Semantic::FlushDishonesty);
    assert_eq!(
        storage.apply(&Step::Ack(TicketId(1))),
        Err(Refusal::NotEnabled(NotEnabled::NotStable(TicketId(1))))
    );
    assert!(matches!(
        storage.apply(&Step::Persist(TicketId(1))).copied(),
        Ok(Event::Stable { upto: 3, .. })
    ));
    assert_eq!(storage.stable_len(NodeId(0)), Some(3));
    assert_refused_as_declared(&mut storage, &false_flush, Semantic::FlushDishonesty);
    assert!(matches!(
        storage.apply(&Step::Ack(TicketId(1))).copied(),
        Ok(Event::Acked { upto: 3, .. })
    ));
}

// --- the no-operation cases -------------------------------------------------------------

/// `pr15-impl04-pos-05`. An undetected tear has no step; the `detected-tear`
/// assumption excludes it. As behaviour: every tear a crash can leave, at every
/// `keep`, reads `Torn`, and every other surviving record reads intact with the value
/// submitted at its position — never a different intact value.
#[test]
fn no_operation_a_tear_always_reads_as_torn() {
    assert_eq!(
        case(Semantic::UndetectedTear).reliance,
        Reliance::Assumed("detected-tear")
    );
    let storage = live(config(2));
    let submitted = [Value(10), Value(11), Value(12)];
    for keep in 0..2_u32 {
        let mut crashed = storage.clone();
        crashed
            .apply(&Step::Crash {
                node: NodeId(0),
                keep,
                torn: true,
            })
            .unwrap();
        let log = crashed.log(NodeId(0)).unwrap();
        let (last, intact) = log.split_last().unwrap();
        assert_eq!(*last, Entry::Torn, "keep {keep}");
        for (position, entry) in intact.iter().enumerate() {
            assert_eq!(*entry, Entry::Intact(submitted[position]));
        }
    }
    // The tear stays a tear, and writing over it is the program's fault, not a device's.
    let mut crashed = storage;
    crashed
        .apply(&Step::Crash {
            node: NodeId(0),
            keep: 0,
            torn: true,
        })
        .unwrap();
    crashed.apply(&Step::Restart(NodeId(0))).unwrap();
    assert_eq!(
        crashed.apply(&submit(0, 13)),
        Err(Refusal::ProgramFault(ProgramFault::WriteOverTornTail(
            NodeId(0)
        )))
    );
}

/// `pr15-impl04-bnd-01`. The one exception `Request` declares: at the configured step
/// bound every step, a requestable unsupported one included, is refused as
/// `BoundReached(Steps)`, inconclusive as `ResourceExhausted`. One step below the bound,
/// each requestable case is still its declared `Unsupported` refusal.
#[test]
fn boundary_at_the_step_bound_a_requestable_case_is_the_step_bound_refusal() {
    let bounded = config(2).with_max_steps(8).unwrap();
    let mut storage = live(bounded);
    assert_eq!(storage.events().len(), 7);
    let probes: Vec<(Step, Semantic)> = UNSUPPORTED
        .iter()
        .filter_map(|case| match case.request {
            Request::Step(kind) => Some((probe_of(kind), case.semantic)),
            Request::NoOperation => None,
        })
        .collect();
    assert_eq!(probes.len(), 3);
    for (probe, semantic) in &probes {
        assert_eq!(storage.check(probe), Err(Refusal::Unsupported(*semantic)));
    }
    storage.apply(&Step::Delay(TicketId(1))).unwrap();
    assert_eq!(storage.events().len(), 8);
    for (probe, _) in &probes {
        let refusal = storage.check(probe).expect_err("at the bound");
        assert_eq!(
            refusal,
            Refusal::BoundReached(Bound::Steps { max: 8 }),
            "{probe:?}"
        );
        assert_eq!(refusal.inconclusive_reason(), Some("ResourceExhausted"));
    }
}

/// The profile registry (RFC 0002 correction 1, cr-37bshu): every name this pack has
/// published, with its version and the FNV-1a fingerprint of its canonical bytes. A
/// profile's name identifies its content, so an entry never changes: a content change
/// is a new name and a new entry. The `-v0` fingerprint is the one
/// `tests/pr15_impl03_storage.rs` pinned before bn-1oj6, so the frozen profile is proved byte for
/// byte what it was.
const REGISTRY: [(&str, &str, u64); 2] = [
    ("storage/append-log-v0", "0.1.0", 2_455_172_717_111_530_958),
    ("storage/append-log-v1", "1.0.0", 11_729_929_152_830_614_249),
];

fn fingerprint(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// `pr15-impl04-hon-03` (cr-37bshu). Each declared profile is exactly its registry
/// entry — name, version and fingerprint — so changing a profile's content under an
/// existing name fails this test; names and fingerprints are unique; `-v0` declares no
/// unsupported cases and the assumptions it always stated, and `-v1` extends them.
#[test]
fn honesty_the_profile_registry_binds_each_name_to_one_fingerprint() {
    let declared: Vec<(&str, String, u64)> = PROFILES
        .iter()
        .map(|p| {
            (
                p.name,
                p.version.to_string(),
                fingerprint(&p.canonical_bytes()),
            )
        })
        .collect();
    let registry: Vec<(&str, String, u64)> = REGISTRY
        .iter()
        .map(|(name, version, fp)| (*name, (*version).to_owned(), *fp))
        .collect();
    assert_eq!(
        declared, registry,
        "a profile changed under a published name"
    );
    let names: std::collections::BTreeSet<&str> = REGISTRY.iter().map(|e| e.0).collect();
    let prints: std::collections::BTreeSet<u64> = REGISTRY.iter().map(|e| e.2).collect();
    assert_eq!(
        (names.len(), prints.len()),
        (REGISTRY.len(), REGISTRY.len())
    );
    assert_eq!(PROFILES[0], APPEND_LOG_V0);
    assert_eq!(PROFILES[1], APPEND_LOG_V1);
    assert_eq!(APPEND_LOG_V0.unsupported, None);
    assert_eq!(APPEND_LOG_V1.unsupported, Some(&UNSUPPORTED[..]));
    assert!(
        APPEND_LOG_V1
            .assumptions
            .starts_with(APPEND_LOG_V0.assumptions)
    );
    assert!(APPEND_LOG_V1.assumptions.len() > APPEND_LOG_V0.assumptions.len());
    // The Lab journal names the newest profile, the last registry entry.
    assert_eq!(PROFILES[PROFILES.len() - 1], APPEND_LOG_V1);
    // Every profile the source declares is in `PROFILES`, so none escapes the registry.
    let declarations = PROFILE_SOURCE
        .matches(": FidelityProfile = FidelityProfile {")
        .count();
    assert_eq!(declarations, PROFILES.len());
}

/// The profile module's source, for the declaration count above.
const PROFILE_SOURCE: &str = include_str!("../src/profile.rs");
