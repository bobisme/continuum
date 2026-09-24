//! PR-15 / IMPL-04 (bn-1oj6): the declared unsupported cases of `process/crash-restart-v0`.
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
//! | `pr15-impl04-hon-01` | [`honesty_every_unsupported_row_has_exactly_one_consistent_declared_case`] | the table is total over the unsupported rows, in order, and every reliance names a stated assumption, a modelled row, or a composition row |
//! | `pr15-impl04-hon-03` | [`honesty_the_profile_registry_binds_each_name_to_one_fingerprint`] | cr-37bshu: each profile name is bound to one fingerprint; the frozen `-v0` is byte for byte the pre-bn-1oj6 profile |
//! | `pr15-impl04-hon-02` | [`honesty_a_no_operation_case_has_no_step_and_no_configuration_value`] | compiler-level: every step and every configuration value is classified by an exhaustive destructuring match, and none asks for a no-operation case |
//! | `pr15-impl04-pos-01` | [`positive_every_requestable_case_is_refused_exactly_as_declared`] | each requestable case, through `apply`, `check` and `run`, is its declared `Unsupported` refusal, INV-008 `Unsupported`, and changes nothing |
//! | `pr15-impl04-pos-02..04` | `case_*` | per case: the declared refusal (positive), a modelled neighbour that still works (negative), and the edge between them (boundary) |
//! | `pr15-impl04-bnd-01` | [`boundary_at_the_step_bound_a_requestable_case_is_the_step_bound_refusal`] | the declared exception: at the step bound every requestable case is `BoundReached(Steps)`, inconclusive; one step below it, the declared `Unsupported` |
//! | `pr15-impl04-pos-05..06` | `no_operation_*` | per no-operation case: the modelled row it is subsumed by produces the host behaviour's effect |

use continuum_effects_process::profile::{ASSUMPTIONS, COMPOSITION};
use continuum_effects_process::refusal::{Bound, NotEnabled};
use continuum_effects_process::step::{MAX_RETAINED_CAP, MAX_STEPS};
use continuum_effects_process::{
    CRASH_RESTART_V0, CRASH_RESTART_V1, Epoch, Event, NodeId, NodeSet, PROFILES, Process,
    ProcessConfig, Refusal, RefusalClass, Reliance, Request, RunRefusal, Semantic, Step, StepKind,
    Support, TicketId, UNSUPPORTED, UnsupportedCase, run,
};

fn config(crashes: u32, restart: bool) -> ProcessConfig {
    ProcessConfig::new(3, crashes, restart, 8, MAX_RETAINED_CAP).unwrap()
}

/// Every node up, and `t0` begun by n0 and pending.
fn live(cfg: ProcessConfig) -> Process {
    let mut process = Process::new(cfg);
    process.apply(&Step::Begin(NodeId(0))).unwrap();
    process
}

fn case(semantic: Semantic) -> UnsupportedCase {
    semantic
        .unsupported_case()
        .unwrap_or_else(|| panic!("{semantic} has no declared case"))
}

/// Ask `process` for `step`, expect exactly the declared refusal of `semantic`'s case
/// through `check`, `apply` and `run`, and expect nothing to change.
fn assert_refused_as_declared(process: &mut Process, step: &Step, semantic: Semantic) {
    let declared = case(semantic).refusal().expect("a requestable case");
    assert_eq!(declared, Refusal::Unsupported(semantic));
    let before = process.clone();
    assert_eq!(process.check(step), Err(declared), "{step:?}");
    assert_eq!(process.apply(step), Err(declared), "{step:?}");
    assert_eq!(*process, before, "{step:?} changed the process pack");
    assert_eq!(declared.class(), RefusalClass::Unsupported);
    assert_eq!(declared.inconclusive_reason(), Some("Unsupported"));
    assert!(declared.to_string().contains(semantic.statement()));
    let mut log: Vec<Step> = process.events().iter().map(Event::step).collect();
    log.push(*step);
    assert_eq!(
        run(*process.config(), &log),
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
/// assumption, a `Subsumed` row is modelled, an `Owner` is a [`COMPOSITION`] row, and
/// `OutsidePack` has no operation. Every stated assumption backs a case or a modelled
/// row the IMPL-02 tests already hold.
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
    let composition_ids: Vec<&str> = COMPOSITION.iter().map(|(id, _)| *id).collect();
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
            Reliance::Owner(row) => assert!(composition_ids.contains(&row), "{row}"),
            Reliance::OutsidePack => assert_eq!(case.request, Request::NoOperation),
        }
    }
    // Every case's reliance, stated here as well as in the fingerprint, so a changed
    // reliance fails a named assertion.
    assert_eq!(
        UNSUPPORTED.map(|c| (c.semantic, c.reliance)),
        [
            (
                Semantic::GracefulCancellation,
                Reliance::Assumed("node-level-lifecycle")
            ),
            (Semantic::Panic, Reliance::Assumed("node-level-lifecycle")),
            (
                Semantic::PowerLoss,
                Reliance::Assumed("power-loss-as-crashes")
            ),
            (
                Semantic::PartialCleanup,
                Reliance::Subsumed(Semantic::FailStopCrash)
            ),
            (
                Semantic::SupervisorDecisions,
                Reliance::Subsumed(Semantic::RestartNewEpoch)
            ),
        ],
        "a declared reliance changed"
    );
}

/// One step of each kind, well formed against [`live`].
fn probe_of(kind: StepKind) -> Step {
    match kind {
        StepKind::Begin => Step::Begin(NodeId(1)),
        StepKind::Complete => Step::Complete(TicketId(0)),
        StepKind::Delay => Step::Delay(TicketId(0)),
        StepKind::Crash => Step::Crash(NodeId(1)),
        StepKind::Restart => Step::Restart(NodeId(1)),
        StepKind::CancelGracefully => Step::CancelGracefully(NodeId(1)),
        StepKind::Panic => Step::Panic(NodeId(1)),
        StepKind::PowerLoss => Step::PowerLoss(NodeSet(0b011)),
    }
}

/// Which profile row a step asks for, from its variant and every one of its fields.
/// Each arm destructures the variant completely, with no wildcard arm, so a new step,
/// or a new field on one, stops this test compiling until it is classified here.
fn asks_for(step: &Step) -> Semantic {
    match step {
        Step::Begin(NodeId(_)) | Step::Complete(TicketId(_)) | Step::Delay(TicketId(_)) => {
            Semantic::LateCompletion
        }
        Step::Crash(NodeId(_)) => Semantic::FailStopCrash,
        Step::Restart(NodeId(_)) => Semantic::RestartNewEpoch,
        Step::CancelGracefully(NodeId(_)) => Semantic::GracefulCancellation,
        Step::Panic(NodeId(_)) => Semantic::Panic,
        Step::PowerLoss(NodeSet(_)) => Semantic::PowerLoss,
    }
}

/// Which profile row each configuration value sets. [`ProcessConfig::new`] is called
/// with its whole argument list, so a new configuration value stops this test
/// compiling until it is classified here.
fn configuration_rows() -> Vec<Semantic> {
    let (nodes, max_crashes, restart, max_pending, max_retained_bytes) =
        (3_u8, 2_u32, true, 8_u32, MAX_RETAINED_CAP);
    let cfg =
        ProcessConfig::new(nodes, max_crashes, restart, max_pending, max_retained_bytes).unwrap();
    assert_eq!(cfg.max_crashes(), max_crashes);
    vec![
        // nodes: the address space every modelled row acts within.
        Semantic::FailStopCrash,
        // max_crashes, restart.
        Semantic::FailStopCrash,
        Semantic::RestartNewEpoch,
        // max_pending, max_retained_bytes.
        Semantic::ExplorationBounds,
        Semantic::ExplorationBounds,
    ]
}

/// `pr15-impl04-hon-02`. A no-operation case cannot be asked for through the public
/// API. The request surfaces are the step (`Process::check`, `Process::apply`, `run`)
/// and the configuration (`ProcessConfig::new`); [`asks_for`] and
/// [`configuration_rows`] classify every variant, field and argument of both, and the
/// compiler holds them total. No step and no configuration value asks for a
/// no-operation row, and every `Step` case's kind asks for its own row. That the public
/// surface has no other entry point that takes input is the recorded INV-015
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
    let mut no_operation = 0;
    for case in UNSUPPORTED {
        match case.request {
            Request::NoOperation => {
                no_operation += 1;
                assert!(
                    !from_steps.contains(&case.semantic) && !from_config.contains(&case.semantic),
                    "{} is declared no-operation",
                    case.semantic
                );
            }
            Request::Step(kind) => assert_eq!(asks_for(&probe_of(kind)), case.semantic),
        }
    }
    assert_eq!(no_operation, 2);
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
        let mut process = live(config(2, true));
        match case.request {
            Request::Step(kind) => {
                assert_refused_as_declared(&mut process, &probe_of(kind), case.semantic);
                refused += 1;
            }
            Request::NoOperation => {}
        }
    }
    assert_eq!(refused, 3);
}

// --- one test per requestable case ------------------------------------------------------

/// `pr15-impl04-pos-02`. Graceful cancellation is refused; the `node-level-lifecycle`
/// assumption excludes it, so a mapping realizes it only as a crash. The
/// fail-stop crash of the same node is modelled. Boundary: once the node is down, the
/// crash is `NotEnabled` and the cancellation is still `Unsupported` — it is never
/// read as a crash, in either direction.
#[test]
fn case_graceful_cancellation_is_refused_and_a_fail_stop_crash_is_modelled() {
    assert_eq!(
        case(Semantic::GracefulCancellation).reliance,
        Reliance::Assumed("node-level-lifecycle")
    );
    let mut process = live(config(2, true));
    let cancel = Step::CancelGracefully(NodeId(0));
    assert_refused_as_declared(&mut process, &cancel, Semantic::GracefulCancellation);
    assert_eq!(
        process.apply(&Step::Crash(NodeId(0))).copied(),
        Ok(Event::Crashed {
            node: NodeId(0),
            epoch: Epoch(0)
        })
    );
    assert_eq!(
        process.apply(&Step::Crash(NodeId(0))),
        Err(Refusal::NotEnabled(NotEnabled::NodeDown(NodeId(0))))
    );
    assert_refused_as_declared(&mut process, &cancel, Semantic::GracefulCancellation);
}

/// `pr15-impl04-pos-03`. A panic is refused; a crash is modelled. Boundary: with no
/// crash declared, the crash is `NotEnabled` and the panic is still `Unsupported`, so a
/// panic never passes for the crash an undeclared fault would forbid.
#[test]
fn case_panic_is_refused_and_is_not_a_crash() {
    let mut process = live(config(1, false));
    assert_refused_as_declared(&mut process, &Step::Panic(NodeId(0)), Semantic::Panic);
    assert!(process.apply(&Step::Crash(NodeId(0))).is_ok());
    let mut no_crash = live(config(0, false));
    assert_refused_as_declared(&mut no_crash, &Step::Panic(NodeId(0)), Semantic::Panic);
    assert_eq!(
        no_crash.apply(&Step::Crash(NodeId(0))),
        Err(Refusal::NotEnabled(NotEnabled::FaultNotDeclared(
            Semantic::FailStopCrash
        )))
    );
}

/// `pr15-impl04-pos-04`. A power loss is refused; the `power-loss-as-crashes`
/// assumption covers it as consecutive crashes, which are modelled under the budget.
/// Boundary: a power loss of one node is still refused, not rewritten as its crash;
/// and at a budget of one, the second consecutive crash is `NotEnabled` — the power
/// loss of two nodes the assumption puts outside the profile.
#[test]
fn case_power_loss_is_refused_and_consecutive_crashes_are_modelled_under_the_budget() {
    assert_eq!(
        case(Semantic::PowerLoss).reliance,
        Reliance::Assumed("power-loss-as-crashes")
    );
    let mut process = live(config(2, true));
    assert_refused_as_declared(
        &mut process,
        &Step::PowerLoss(NodeSet(0b011)),
        Semantic::PowerLoss,
    );
    assert_refused_as_declared(
        &mut process,
        &Step::PowerLoss(NodeSet(0b001)),
        Semantic::PowerLoss,
    );
    process.apply(&Step::Crash(NodeId(0))).unwrap();
    process.apply(&Step::Crash(NodeId(1))).unwrap();
    assert_eq!(process.up(), NodeSet(0b100));
    // The ticket n0 began is fenced, as a crash fences it.
    assert!(matches!(
        process.apply(&Step::Complete(TicketId(0))).copied(),
        Ok(Event::Fenced { .. })
    ));

    let mut budget_one = live(config(1, true));
    budget_one.apply(&Step::Crash(NodeId(0))).unwrap();
    assert_eq!(
        budget_one.apply(&Step::Crash(NodeId(1))),
        Err(Refusal::NotEnabled(NotEnabled::CrashBudgetSpent { max: 1 }))
    );
}

// --- the no-operation cases -------------------------------------------------------------

/// `pr15-impl04-pos-05`. Partial cleanup has no step; it is subsumed by the fail-stop
/// crash: a cleanup that stops partway is the program's own steps up to a step boundary,
/// then a crash. The crash can land between any two of them, and after it the
/// incarnation's pending operation is fenced, never finished by a cleanup.
#[test]
fn no_operation_partial_cleanup_is_a_crash_at_a_step_boundary() {
    assert_eq!(
        case(Semantic::PartialCleanup).reliance,
        Reliance::Subsumed(Semantic::FailStopCrash)
    );
    for cut in 0..=2 {
        let mut process = Process::new(config(1, false));
        // The program's "cleanup": three operations of n0; the crash lands after `cut`.
        for _ in 0..cut {
            process.apply(&Step::Begin(NodeId(0))).unwrap();
        }
        process.apply(&Step::Crash(NodeId(0))).unwrap();
        assert_eq!(
            process.apply(&Step::Begin(NodeId(0))),
            Err(Refusal::NotEnabled(NotEnabled::NodeDown(NodeId(0))))
        );
        for ticket in 0..cut {
            assert!(matches!(
                process.apply(&Step::Complete(TicketId(ticket))).copied(),
                Ok(Event::Fenced { .. })
            ));
        }
    }
}

/// `pr15-impl04-pos-06`. Supervisor decisions have no step; they are subsumed by the
/// adversary's choice to restart or not. Restarting at once, restarting after other
/// steps (a backoff) and never restarting (giving up) are all runs of the modelled row.
/// Boundary: with restart undeclared, no supervisor restart is in the envelope, and
/// `Restart` is `NotEnabled`.
#[test]
fn no_operation_supervisor_decisions_are_the_adversary_s_restart_choice() {
    assert_eq!(
        case(Semantic::SupervisorDecisions).reliance,
        Reliance::Subsumed(Semantic::RestartNewEpoch)
    );
    let crashed = |restart: bool| {
        let mut process = Process::new(config(1, restart));
        process.apply(&Step::Crash(NodeId(0))).unwrap();
        process
    };
    let mut at_once = crashed(true);
    assert!(at_once.apply(&Step::Restart(NodeId(0))).is_ok());
    let mut backoff = crashed(true);
    backoff.apply(&Step::Begin(NodeId(1))).unwrap();
    backoff.apply(&Step::Complete(TicketId(0))).unwrap();
    assert_eq!(
        backoff.apply(&Step::Restart(NodeId(0))).copied(),
        Ok(Event::Restarted {
            node: NodeId(0),
            epoch: Epoch(1)
        })
    );
    let gave_up = crashed(true);
    assert!(!gave_up.is_up(NodeId(0)));
    let mut undeclared = crashed(false);
    assert_eq!(
        undeclared.apply(&Step::Restart(NodeId(0))),
        Err(Refusal::NotEnabled(NotEnabled::FaultNotDeclared(
            Semantic::RestartNewEpoch
        )))
    );
}

/// `pr15-impl04-bnd-01`. The one exception `Request` declares: at the step bound every
/// step, a requestable unsupported one included, is refused as `BoundReached(Steps)`,
/// inconclusive as `ResourceExhausted`. One step below the bound, each requestable case
/// is still its declared `Unsupported` refusal.
#[test]
fn boundary_at_the_step_bound_a_requestable_case_is_the_step_bound_refusal() {
    let mut process = live(config(2, true));
    while process.events().len() < MAX_STEPS - 1 {
        process.apply(&Step::Delay(TicketId(0))).unwrap();
    }
    let probes: Vec<(Step, Semantic)> = UNSUPPORTED
        .iter()
        .filter_map(|case| match case.request {
            Request::Step(kind) => Some((probe_of(kind), case.semantic)),
            Request::NoOperation => None,
        })
        .collect();
    assert_eq!(probes.len(), 3);
    for (probe, semantic) in &probes {
        assert_eq!(process.check(probe), Err(Refusal::Unsupported(*semantic)));
    }
    process.apply(&Step::Delay(TicketId(0))).unwrap();
    assert_eq!(process.events().len(), MAX_STEPS);
    for (probe, _) in &probes {
        let refusal = process.check(probe).expect_err("at the bound");
        assert_eq!(
            refusal,
            Refusal::BoundReached(Bound::Steps { max: MAX_STEPS }),
            "{probe:?}"
        );
        assert_eq!(refusal.inconclusive_reason(), Some("ResourceExhausted"));
    }
}

/// The profile registry (RFC 0002 correction 1, cr-37bshu): every name this pack has
/// published, with its version and the FNV-1a fingerprint of its canonical bytes. A
/// profile's name identifies its content, so an entry never changes: a content change
/// is a new name and a new entry. The `-v0` fingerprint is the one
/// `tests/pr15_impl02_process.rs` pinned before bn-1oj6, so the frozen profile is proved byte for
/// byte what it was.
const REGISTRY: [(&str, &str, u64); 2] = [
    (
        "process/crash-restart-v0",
        "0.1.0",
        18_224_978_510_953_362_894,
    ),
    ("process/crash-restart-v1", "1.0.0", 366_640_240_911_873_674),
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
    assert_eq!(PROFILES[0], CRASH_RESTART_V0);
    assert_eq!(PROFILES[1], CRASH_RESTART_V1);
    assert_eq!(CRASH_RESTART_V0.unsupported, None);
    assert_eq!(CRASH_RESTART_V1.unsupported, Some(&UNSUPPORTED[..]));
    assert!(
        CRASH_RESTART_V1
            .assumptions
            .starts_with(CRASH_RESTART_V0.assumptions)
    );
    assert!(CRASH_RESTART_V1.assumptions.len() > CRASH_RESTART_V0.assumptions.len());
    // The Lab journal names the newest profile, the last registry entry.
    assert_eq!(PROFILES[PROFILES.len() - 1], CRASH_RESTART_V1);
    // Every profile the source declares is in `PROFILES`, so none escapes the registry.
    let declarations = PROFILE_SOURCE
        .matches(": FidelityProfile = FidelityProfile {")
        .count();
    assert_eq!(declarations, PROFILES.len());
}

/// The profile module's source, for the declaration count above.
const PROFILE_SOURCE: &str = include_str!("../src/profile.rs");
