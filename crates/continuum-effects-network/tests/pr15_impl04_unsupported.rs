//! PR-15 / IMPL-04 (bn-1oj6): the declared unsupported cases of `network/adversarial-v0`.
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
//! | `pr15-impl04-hon-01` | [`honesty_every_unsupported_row_has_exactly_one_consistent_declared_case`] | the table is total over the unsupported rows, in order, and every reliance names a stated assumption, a modelled row, or the process profile |
//! | `pr15-impl04-hon-03` | [`honesty_the_profile_registry_binds_each_name_to_one_fingerprint`] | cr-37bshu: each profile name is bound to one fingerprint; the frozen `-v0` is byte for byte the pre-bn-1oj6 profile |
//! | `pr15-impl04-hon-02` | [`honesty_a_no_operation_case_has_no_step_and_no_configuration_value`] | compiler-level: every step and every configuration value is classified by an exhaustive destructuring match, and none asks for a no-operation case |
//! | `pr15-impl04-pos-01` | [`positive_every_requestable_case_is_refused_exactly_as_declared`] | each requestable case, through `apply`, `check` and `run`, is its declared `Unsupported` refusal, INV-008 `Unsupported`, and changes nothing |
//! | `pr15-impl04-pos-02..08` | `case_*` | per case: the declared refusal (positive), a modelled neighbour that still works (negative), and the edge between them (boundary) |
//! | `pr15-impl04-neg-01` | [`negative_stale_resolution_satisfies_no_forgery_so_only_fixed_addressing_excludes_it`] | cr-37bshu: a send to a stale address is delivered faithfully and satisfies `network-no-forgery`, so service discovery rests on its own `fixed-addressing` assumption |
//! | `pr15-impl04-bnd-01` | [`boundary_at_the_step_bound_a_requestable_case_is_the_step_bound_refusal`] | the declared exception: at the step bound every requestable case is `BoundReached(Steps)`, inconclusive; one step below it, the declared `Unsupported` |

use continuum_effects_network::profile::{ASSUMPTIONS, PROFILE_NAME};
use continuum_effects_network::refusal::{Bound, NotEnabled};
use continuum_effects_network::step::{MAX_RETAINED_CAP, MAX_STEPS};
use continuum_effects_network::{
    ADVERSARIAL_V0, ADVERSARIAL_V1, EnvelopeId, Event, FaultSwitches, Network, NetworkConfig,
    NodeId, NodeSet, PROFILES, Payload, Refusal, RefusalClass, Reliance, Request, RunRefusal,
    Semantic, Step, StepKind, Support, UNSUPPORTED, UnsupportedCase, When, run,
};

/// The process pack's profile source. The two crates share no dependency, so the
/// sibling profile's name is read from its declaration.
const PROCESS_PROFILE: &str = include_str!("../../continuum-effects-process/src/profile.rs");

const ALL_FAULTS: FaultSwitches = FaultSwitches {
    loss: true,
    duplication: true,
    reordering: true,
};

fn config(partitions: u32, faults: FaultSwitches) -> NetworkConfig {
    NetworkConfig::new(3, 8, 8, faults, partitions, MAX_RETAINED_CAP).unwrap()
}

fn send(src: u8, dst: u8, payload: &[u8]) -> Step {
    Step::Send {
        src: NodeId(src),
        dst: NodeId(dst),
        payload: Payload(payload.to_vec()),
    }
}

/// The prefix of every probe: `e0` from n0 to n1 and `e1` from n1 to n0, both in flight.
fn prefix() -> Vec<Step> {
    vec![send(0, 1, b"a"), send(1, 0, b"b")]
}

fn live(cfg: NetworkConfig) -> Network {
    let mut net = Network::new(cfg);
    for step in prefix() {
        net.apply(&step).unwrap();
    }
    net
}

fn case(semantic: Semantic) -> UnsupportedCase {
    semantic
        .unsupported_case()
        .unwrap_or_else(|| panic!("{semantic} has no declared case"))
}

/// Ask `net` for `step`, expect exactly the declared refusal of `semantic`'s case
/// through `check`, `apply` and `run`, and expect nothing to change.
fn assert_refused_as_declared(net: &mut Network, step: &Step, semantic: Semantic) {
    let declared = case(semantic).refusal().expect("a requestable case");
    assert_eq!(declared, Refusal::Unsupported(semantic));
    let before = net.clone();
    assert_eq!(net.check(step), Err(declared), "{step:?}");
    assert_eq!(net.apply(step), Err(declared), "{step:?}");
    assert_eq!(*net, before, "{step:?} changed the network");
    assert_eq!(declared.class(), RefusalClass::Unsupported);
    assert_eq!(declared.inconclusive_reason(), Some("Unsupported"));
    assert!(declared.to_string().contains(semantic.statement()));
    // The same log through `run`: the refusal is at the probe's index, never a journal.
    let mut log: Vec<Step> = net.events().iter().map(Event::step).collect();
    log.push(step.clone());
    assert_eq!(
        run(*net.config(), &log),
        Err(RunRefusal {
            index: log.len() - 1,
            refusal: declared
        })
    );
}

// --- the declaration --------------------------------------------------------------------

/// `pr15-impl04-hon-01`. The declared cases are exactly the unsupported rows, one each,
/// in [`Semantic::ALL`] order, and each is internally consistent: a `Step` request names
/// the kind whose [`Step::unsupported_semantic`] is the row, `StepWhen` names a modelled
/// step, an `Assumed` id is a stated assumption, a `Subsumed` row is modelled, the
/// `Owner` is the process profile's own name, and `OutsidePack` has no operation.
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
    let owner_decl = "pub const PROFILE_NAME_V0: &str = \"process/crash-restart-v0\";";
    assert!(PROCESS_PROFILE.contains(owner_decl));
    for case in UNSUPPORTED {
        assert!(case.host_behaviour.len() > 30, "{}", case.semantic);
        match case.request {
            Request::Step(kind) => {
                let probe = probe_of(kind);
                assert_eq!(probe.kind(), kind);
                assert_eq!(probe.unsupported_semantic(), Some(case.semantic));
            }
            Request::StepWhen { step, when } => {
                assert_eq!(probe_of(step).unsupported_semantic(), None);
                assert_eq!(step, StepKind::Partition);
                assert_eq!(when, When::PartitionActiveWithBudget);
                assert_eq!(when.token(), "partition-active-with-budget");
            }
            Request::NoOperation => assert_eq!(case.refusal(), None),
        }
        match case.reliance {
            Reliance::Assumed(id) => assert!(assumption_ids.contains(&id), "{id}"),
            Reliance::Subsumed(row) => assert_eq!(row.support(), Support::Modelled),
            Reliance::Owner(profile) => {
                // The composition tables every profile shares name `-v0` siblings, whose
                // rows `-v1` keeps, so the owner is named the same way.
                assert_eq!(profile, "process/crash-restart-v0");
                assert_ne!(profile, PROFILE_NAME);
            }
            Reliance::OutsidePack => assert_eq!(case.request, Request::NoOperation),
        }
    }
    // Every case's reliance, stated here as well as in the fingerprint, so a changed
    // reliance fails a named assertion.
    let expected: [(Semantic, Reliance); 13] = [
        (
            Semantic::BoundedDelay,
            Reliance::Subsumed(Semantic::UnboundedDelay),
        ),
        (
            Semantic::AsymmetricPartition,
            Reliance::Subsumed(Semantic::UnboundedDelay),
        ),
        (
            Semantic::OverlappingPartitions,
            Reliance::Subsumed(Semantic::UnboundedDelay),
        ),
        (
            Semantic::ConnectionEpochs,
            Reliance::Assumed("connectionless-send"),
        ),
        (Semantic::HalfOpen, Reliance::Assumed("connectionless-send")),
        (
            Semantic::Backpressure,
            Reliance::Assumed("connectionless-send"),
        ),
        (
            Semantic::Corruption,
            Reliance::Assumed("network-no-forgery"),
        ),
        (Semantic::Forgery, Reliance::Assumed("network-no-forgery")),
        (
            Semantic::ServiceDiscovery,
            Reliance::Assumed("fixed-addressing"),
        ),
        (Semantic::Framing, Reliance::Assumed("atomic-payload")),
        (
            Semantic::ConnectionReset,
            Reliance::Assumed("connectionless-send"),
        ),
        (
            Semantic::EndpointCrash,
            Reliance::Owner("process/crash-restart-v0"),
        ),
        (
            Semantic::RecallInFlight,
            Reliance::Subsumed(Semantic::UnboundedDelay),
        ),
    ];
    assert_eq!(
        UNSUPPORTED.map(|c| (c.semantic, c.reliance)),
        expected,
        "a declared reliance changed"
    );
}

/// One step of each kind, well formed against [`live`].
fn probe_of(kind: StepKind) -> Step {
    match kind {
        StepKind::Send => send(0, 2, b"c"),
        StepKind::Deliver => Step::Deliver(EnvelopeId(0)),
        StepKind::Drop => Step::Drop(EnvelopeId(0)),
        StepKind::Duplicate => Step::Duplicate(EnvelopeId(0)),
        StepKind::Delay => Step::Delay(EnvelopeId(0)),
        StepKind::Partition => Step::Partition(NodeSet(0b001)),
        StepKind::Heal => Step::Heal,
        StepKind::OneWayPartition => Step::OneWayPartition {
            from: NodeSet(0b001),
            to: NodeSet(0b110),
        },
        StepKind::Corrupt => Step::Corrupt(EnvelopeId(0)),
        StepKind::Forge => Step::Forge {
            src: NodeId(0),
            dst: NodeId(1),
            payload: Payload(b"f".to_vec()),
        },
        StepKind::ConnectionReset => Step::ConnectionReset(NodeId(0), NodeId(1)),
        StepKind::CrashEndpoint => Step::CrashEndpoint(NodeId(1)),
        StepKind::Recall => Step::Recall(EnvelopeId(0)),
    }
}

/// Which profile row a step asks for, from its variant and every one of its fields.
/// Each arm destructures the variant completely, with no `..` and no wildcard arm, so a
/// new step, or a new field on one (a delay duration, a connection id), stops this
/// test compiling until it is classified here. `None` is the program's `Send`, the
/// pack's one operation. `Partition` is the modelled row; the state in which it asks
/// for overlapping partitions is the declared `StepWhen`.
fn asks_for(step: &Step) -> Option<Semantic> {
    match step {
        Step::Send {
            src: NodeId(_),
            dst: NodeId(_),
            payload: Payload(_),
        } => None,
        Step::Deliver(EnvelopeId(_)) => Some(Semantic::Reordering),
        Step::Drop(EnvelopeId(_)) => Some(Semantic::Loss),
        Step::Duplicate(EnvelopeId(_)) => Some(Semantic::Duplication),
        Step::Delay(EnvelopeId(_)) => Some(Semantic::UnboundedDelay),
        Step::Partition(NodeSet(_)) | Step::Heal => Some(Semantic::SymmetricPartition),
        Step::OneWayPartition {
            from: NodeSet(_),
            to: NodeSet(_),
        } => Some(Semantic::AsymmetricPartition),
        Step::Corrupt(EnvelopeId(_)) => Some(Semantic::Corruption),
        Step::Forge {
            src: NodeId(_),
            dst: NodeId(_),
            payload: Payload(_),
        } => Some(Semantic::Forgery),
        Step::ConnectionReset(NodeId(_), NodeId(_)) => Some(Semantic::ConnectionReset),
        Step::CrashEndpoint(NodeId(_)) => Some(Semantic::EndpointCrash),
        Step::Recall(EnvelopeId(_)) => Some(Semantic::RecallInFlight),
    }
}

/// Which profile row each configuration value sets. [`NetworkConfig::new`] is called
/// with its whole argument list and [`FaultSwitches`] is destructured completely, so a
/// new configuration value stops this test compiling until it is classified here.
fn configuration_rows() -> Vec<Semantic> {
    let FaultSwitches {
        loss,
        duplication,
        reordering,
    } = ALL_FAULTS;
    let cfg = NetworkConfig::new(
        3,
        8,
        8,
        FaultSwitches {
            loss,
            duplication,
            reordering,
        },
        1,
        MAX_RETAINED_CAP,
    )
    .unwrap();
    assert_eq!(cfg.faults(), ALL_FAULTS);
    vec![
        // nodes: the address space every modelled row delivers within.
        Semantic::Reordering,
        // max_in_flight, max_payload_bytes, max_retained_bytes.
        Semantic::InFlightBound,
        Semantic::InFlightBound,
        Semantic::InFlightBound,
        // loss, duplication, reordering.
        Semantic::Loss,
        Semantic::Duplication,
        Semantic::Reordering,
        // max_partitions.
        Semantic::SymmetricPartition,
    ]
}

/// `pr15-impl04-hon-02`. A no-operation case cannot be asked for through the public
/// API. The request surfaces are the step (`Network::check`, `Network::apply`, `run`)
/// and the configuration (`NetworkConfig::new`); [`asks_for`] and
/// [`configuration_rows`] classify every variant, field and argument of both, and the
/// compiler holds them total. No step and no configuration value asks for a
/// no-operation row, every `Step` case's kind asks for its own row, and the one
/// `StepWhen` kind asks for a modelled row outside its state. That the public surface
/// has no other entry point that takes input is the recorded INV-015 inventory's claim
/// (`crates/continuumd/tests/inv015_agent_least_authority_evidence.rs`), which is
/// lexical.
#[test]
fn honesty_a_no_operation_case_has_no_step_and_no_configuration_value() {
    let from_steps: Vec<Semantic> = StepKind::ALL
        .into_iter()
        .filter_map(|kind| asks_for(&probe_of(kind)))
        .collect();
    let from_config = configuration_rows();
    for row in &from_config {
        assert_eq!(row.support(), Support::Modelled, "{row}");
    }
    let mut no_operation = 0;
    for case in UNSUPPORTED {
        let reached = from_steps.contains(&case.semantic) || from_config.contains(&case.semantic);
        match case.request {
            Request::NoOperation => {
                no_operation += 1;
                assert!(!reached, "{} is declared no-operation", case.semantic);
            }
            Request::Step(kind) => {
                assert_eq!(asks_for(&probe_of(kind)), Some(case.semantic));
            }
            Request::StepWhen { step, .. } => {
                let row = asks_for(&probe_of(step)).expect("a modelled row");
                assert_eq!(row.support(), Support::Modelled);
            }
        }
    }
    assert_eq!(no_operation, 6);
    // Every kind, and only these, is in `StepKind::ALL`, and `Step::kind` agrees.
    for kind in StepKind::ALL {
        assert_eq!(probe_of(kind).kind(), kind);
    }
}

/// The probe that asks `net` (a [`live`] network with a partition budget of two) for
/// the requestable case of `semantic`, after any setup the case's state needs. The
/// match is over every row, so a new row stops this test compiling until it is
/// classified.
fn probe_for(semantic: Semantic, net: &mut Network) -> Option<Step> {
    match semantic {
        Semantic::AsymmetricPartition
        | Semantic::Corruption
        | Semantic::Forgery
        | Semantic::ConnectionReset
        | Semantic::EndpointCrash
        | Semantic::RecallInFlight => {
            let Request::Step(kind) = case(semantic).request else {
                panic!("{semantic} is not a plain step case");
            };
            Some(probe_of(kind))
        }
        Semantic::OverlappingPartitions => {
            net.apply(&Step::Partition(NodeSet(0b001))).unwrap();
            Some(Step::Partition(NodeSet(0b010)))
        }
        Semantic::BoundedDelay
        | Semantic::ConnectionEpochs
        | Semantic::HalfOpen
        | Semantic::Backpressure
        | Semantic::ServiceDiscovery
        | Semantic::Framing
        | Semantic::Loss
        | Semantic::Duplication
        | Semantic::Reordering
        | Semantic::UnboundedDelay
        | Semantic::SymmetricPartition
        | Semantic::InFlightBound => None,
    }
}

/// `pr15-impl04-pos-01`. Every case with a request, asked for through the public API,
/// is refused with exactly its declared refusal, and the network does not change.
#[test]
fn positive_every_requestable_case_is_refused_exactly_as_declared() {
    let mut refused = 0;
    for case in UNSUPPORTED {
        let mut net = live(config(2, ALL_FAULTS));
        match (case.request, probe_for(case.semantic, &mut net)) {
            (Request::NoOperation, None) => {}
            (Request::Step(_) | Request::StepWhen { .. }, Some(probe)) => {
                assert_refused_as_declared(&mut net, &probe, case.semantic);
                refused += 1;
            }
            (request, probe) => panic!("{}: {request:?} but probe {probe:?}", case.semantic),
        }
    }
    assert_eq!(refused, 7);
}

// --- one test per requestable case ------------------------------------------------------

/// `pr15-impl04-pos-02`. A one-way cut is refused; the symmetric cut with the same
/// side is modelled. Boundary: a one-way cut whose two sets are exactly the two sides of
/// a symmetric partition is still one-way, and still refused.
#[test]
fn case_asymmetric_partition_one_way_is_refused_symmetric_is_modelled() {
    let mut net = live(config(1, ALL_FAULTS));
    let one_way = Step::OneWayPartition {
        from: NodeSet(0b001),
        to: NodeSet(0b010),
    };
    assert_refused_as_declared(&mut net, &one_way, Semantic::AsymmetricPartition);
    let complementary = Step::OneWayPartition {
        from: NodeSet(0b001),
        to: NodeSet(0b110),
    };
    assert_refused_as_declared(&mut net, &complementary, Semantic::AsymmetricPartition);
    assert_eq!(
        net.apply(&Step::Partition(NodeSet(0b001))).cloned(),
        Ok(Event::Partitioned(NodeSet(0b001)))
    );
}

/// `pr15-impl04-pos-03`. A second partition while one is active is refused as
/// overlapping; after `Heal` the same step is modelled. Boundary: with the scenario's
/// budget of one partition, the second is not a behaviour of the declared envelope at
/// all, so it is `NotEnabled` by the budget, which is checked first; a repeat of the
/// active side is still overlapping.
#[test]
fn case_overlapping_partitions_are_refused_while_active_and_modelled_after_heal() {
    let mut net = live(config(2, ALL_FAULTS));
    net.apply(&Step::Partition(NodeSet(0b001))).unwrap();
    assert_refused_as_declared(
        &mut net,
        &Step::Partition(NodeSet(0b010)),
        Semantic::OverlappingPartitions,
    );
    assert_refused_as_declared(
        &mut net,
        &Step::Partition(NodeSet(0b001)),
        Semantic::OverlappingPartitions,
    );
    net.apply(&Step::Heal).unwrap();
    assert_eq!(
        net.apply(&Step::Partition(NodeSet(0b010))).cloned(),
        Ok(Event::Partitioned(NodeSet(0b101)))
    );

    let mut budget_one = live(config(1, ALL_FAULTS));
    budget_one.apply(&Step::Partition(NodeSet(0b001))).unwrap();
    assert_eq!(
        budget_one.apply(&Step::Partition(NodeSet(0b010))),
        Err(Refusal::NotEnabled(NotEnabled::PartitionBudgetSpent {
            max: 1
        }))
    );
}

/// `pr15-impl04-pos-04`. Corrupting an envelope is refused; delivering it is modelled
/// and bit-exact. Boundary: duplication, the modelled change to an envelope in flight,
/// is accepted and keeps the payload, and `Corrupt` stays refused for an envelope that
/// is no longer in flight, where no modelled step applies.
#[test]
fn case_corruption_is_refused_and_delivery_is_bit_exact() {
    let mut net = live(config(1, ALL_FAULTS));
    assert_refused_as_declared(
        &mut net,
        &Step::Corrupt(EnvelopeId(0)),
        Semantic::Corruption,
    );
    net.apply(&Step::Duplicate(EnvelopeId(0))).unwrap();
    assert_eq!(
        net.envelope(EnvelopeId(0)).map(|(_, _, p)| p.clone()),
        Some(Payload(b"a".to_vec()))
    );
    net.apply(&Step::Deliver(EnvelopeId(0))).unwrap();
    net.apply(&Step::Deliver(EnvelopeId(0))).unwrap();
    assert_eq!(net.copies(EnvelopeId(0)), 0);
    assert_refused_as_declared(
        &mut net,
        &Step::Corrupt(EnvelopeId(0)),
        Semantic::Corruption,
    );
    assert_eq!(
        net.apply(&Step::Deliver(EnvelopeId(0))),
        Err(Refusal::NotEnabled(NotEnabled::NotInFlight(EnvelopeId(0))))
    );
}

/// `pr15-impl04-pos-05`. Forging an envelope is refused; the program's own `Send` with
/// the same sender, receiver and payload is modelled. Boundary: a forged copy of an
/// envelope already sent is still forgery, while the adversary's modelled way to
/// repeat it, `Duplicate`, is accepted.
#[test]
fn case_forgery_is_refused_and_the_same_send_is_modelled() {
    let mut net = live(config(1, ALL_FAULTS));
    let forged = Step::Forge {
        src: NodeId(0),
        dst: NodeId(2),
        payload: Payload(b"z".to_vec()),
    };
    assert_refused_as_declared(&mut net, &forged, Semantic::Forgery);
    let replayed = Step::Forge {
        src: NodeId(0),
        dst: NodeId(1),
        payload: Payload(b"a".to_vec()),
    };
    assert_refused_as_declared(&mut net, &replayed, Semantic::Forgery);
    net.apply(&Step::Duplicate(EnvelopeId(0))).unwrap();
    assert!(matches!(
        net.apply(&send(0, 2, b"z")).cloned(),
        Ok(Event::Sent {
            envelope: EnvelopeId(2),
            ..
        })
    ));
}

/// `pr15-impl04-pos-06`. A connection reset is refused; losing the envelopes on the
/// link is modelled. Boundary: the refusal does not depend on the configuration — with
/// loss undeclared, `Drop` is `NotEnabled` while the reset is still `Unsupported`.
#[test]
fn case_connection_reset_is_refused_and_loss_is_modelled() {
    let mut net = live(config(1, ALL_FAULTS));
    let reset = Step::ConnectionReset(NodeId(0), NodeId(1));
    assert_refused_as_declared(&mut net, &reset, Semantic::ConnectionReset);
    assert_eq!(
        net.apply(&Step::Drop(EnvelopeId(0))).cloned(),
        Ok(Event::Dropped(EnvelopeId(0)))
    );
    let no_loss = FaultSwitches {
        loss: false,
        ..ALL_FAULTS
    };
    let mut strict = live(config(1, no_loss));
    assert_refused_as_declared(&mut strict, &reset, Semantic::ConnectionReset);
    assert_eq!(
        strict.apply(&Step::Drop(EnvelopeId(0))),
        Err(Refusal::NotEnabled(NotEnabled::FaultNotDeclared(
            Semantic::Loss
        )))
    );
}

/// `pr15-impl04-pos-07`. An endpoint crash is refused as a network event; the process
/// profile owns it. Delivery to that node is modelled. Boundary: the refusal holds for
/// a node the configuration does not have, so it never passes as a malformed step.
#[test]
fn case_endpoint_crash_is_refused_and_delivery_to_the_node_is_modelled() {
    assert_eq!(
        case(Semantic::EndpointCrash).reliance,
        Reliance::Owner("process/crash-restart-v0")
    );
    let mut net = live(config(1, ALL_FAULTS));
    assert_refused_as_declared(
        &mut net,
        &Step::CrashEndpoint(NodeId(1)),
        Semantic::EndpointCrash,
    );
    assert_refused_as_declared(
        &mut net,
        &Step::CrashEndpoint(NodeId(9)),
        Semantic::EndpointCrash,
    );
    assert!(matches!(
        net.apply(&Step::Deliver(EnvelopeId(0))).cloned(),
        Ok(Event::Delivered { dst: NodeId(1), .. })
    ));
}

/// `pr15-impl04-pos-08`. Recalling an envelope in flight is refused; holding it with
/// `Delay` is modelled, which is the unbounded-delay row the case is subsumed by.
/// Boundary: once the envelope is delivered, `Delay` is `NotEnabled` (nothing in flight)
/// and `Recall` is still `Unsupported`.
#[test]
fn case_recall_is_refused_and_delay_is_modelled() {
    assert_eq!(
        case(Semantic::RecallInFlight).reliance,
        Reliance::Subsumed(Semantic::UnboundedDelay)
    );
    let mut net = live(config(1, ALL_FAULTS));
    assert_refused_as_declared(
        &mut net,
        &Step::Recall(EnvelopeId(0)),
        Semantic::RecallInFlight,
    );
    assert_eq!(
        net.apply(&Step::Delay(EnvelopeId(0))).cloned(),
        Ok(Event::Delayed(EnvelopeId(0)))
    );
    net.apply(&Step::Deliver(EnvelopeId(0))).unwrap();
    assert_eq!(
        net.apply(&Step::Delay(EnvelopeId(0))),
        Err(Refusal::NotEnabled(NotEnabled::NotInFlight(EnvelopeId(0))))
    );
    assert_refused_as_declared(
        &mut net,
        &Step::Recall(EnvelopeId(0)),
        Semantic::RecallInFlight,
    );
}

/// `pr15-impl04-bnd-01`. The one exception `Request` declares: at the step bound every
/// step, a requestable unsupported one included, is refused as `BoundReached(Steps)`,
/// inconclusive as `ResourceExhausted`, before the step is looked at. One step below
/// the bound, each requestable case is still its declared `Unsupported` refusal.
#[test]
fn boundary_at_the_step_bound_a_requestable_case_is_the_step_bound_refusal() {
    let mut net = live(config(2, ALL_FAULTS));
    net.apply(&Step::Partition(NodeSet(0b001))).unwrap();
    while net.events().len() < MAX_STEPS - 1 {
        net.apply(&Step::Delay(EnvelopeId(0))).unwrap();
    }
    let probes: Vec<(Step, Semantic)> = UNSUPPORTED
        .iter()
        .filter_map(|case| match case.request {
            Request::Step(kind) => Some((probe_of(kind), case.semantic)),
            Request::StepWhen { step, .. } => {
                assert_eq!(step, StepKind::Partition);
                Some((Step::Partition(NodeSet(0b010)), case.semantic))
            }
            Request::NoOperation => None,
        })
        .collect();
    assert_eq!(probes.len(), 7);
    for (probe, semantic) in &probes {
        assert_eq!(
            net.check(probe),
            Err(Refusal::Unsupported(*semantic)),
            "{probe:?} below the bound"
        );
    }
    net.apply(&Step::Delay(EnvelopeId(0))).unwrap();
    assert_eq!(net.events().len(), MAX_STEPS);
    for (probe, _) in &probes {
        let refusal = net.check(probe).expect_err("at the bound");
        assert_eq!(
            refusal,
            Refusal::BoundReached(Bound::Steps { max: MAX_STEPS }),
            "{probe:?}"
        );
        assert_eq!(refusal.inconclusive_reason(), Some("ResourceExhausted"));
    }
}

fn assumption(id: &str) -> &'static str {
    ASSUMPTIONS
        .iter()
        .find(|(name, _)| *name == id)
        .map(|(_, text)| *text)
        .unwrap_or_else(|| panic!("no assumption {id}"))
}

/// `pr15-impl04-neg-01` (cr-37bshu). A stale resolution, modelled as the program sending
/// to the node a stale table names, n2, when it means n1: the pack delivers it to n2
/// faithfully, and the delivered envelope is one that was sent, from its sender to its
/// receiver, with its payload unchanged. So `network-no-forgery` holds of this run and
/// cannot be what excludes a stale address. The case rests on `fixed-addressing`,
/// whose statement names a stale or wrong address, and no-forgery's does not.
#[test]
fn negative_stale_resolution_satisfies_no_forgery_so_only_fixed_addressing_excludes_it() {
    // The program means n1; its stale table names n2.
    let stale = NodeId(2);
    let mut net = Network::new(config(1, ALL_FAULTS));
    net.apply(&Step::Send {
        src: NodeId(0),
        dst: stale,
        payload: Payload(b"for n1".to_vec()),
    })
    .unwrap();
    let delivered = net.apply(&Step::Deliver(EnvelopeId(0))).cloned().unwrap();
    assert_eq!(
        delivered,
        Event::Delivered {
            envelope: EnvelopeId(0),
            src: NodeId(0),
            dst: stale,
        }
    );
    // Sent to that receiver, payload unchanged: no-forgery's statement is satisfied.
    assert_eq!(
        net.envelope(EnvelopeId(0)),
        Some((NodeId(0), stale, &Payload(b"for n1".to_vec())))
    );
    // The machine-checked claim is the reliance pin below; assumptions are prose, and
    // this run is the witness that no-forgery's statement holds of a stale send.
    assert!(assumption("fixed-addressing").contains("late, stale, wrong"));
    assert_eq!(
        case(Semantic::ServiceDiscovery).reliance,
        Reliance::Assumed("fixed-addressing")
    );
}

/// The profile registry (RFC 0002 correction 1, cr-37bshu): every name this pack has
/// published, with its version and the FNV-1a fingerprint of its canonical bytes. A
/// profile's name identifies its content, so an entry never changes: a content change
/// is a new name and a new entry. The `-v0` fingerprint is the one
/// `tests/pr15_impl01_network.rs` pinned before bn-1oj6, so the frozen profile is proved byte for
/// byte what it was.
const REGISTRY: [(&str, &str, u64); 2] = [
    ("network/adversarial-v0", "0.1.0", 6_176_911_479_697_058_609),
    (
        "network/adversarial-v1",
        "1.0.0",
        13_799_255_423_715_173_705,
    ),
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
    assert_eq!(PROFILES[0], ADVERSARIAL_V0);
    assert_eq!(PROFILES[1], ADVERSARIAL_V1);
    assert_eq!(ADVERSARIAL_V0.unsupported, None);
    assert_eq!(ADVERSARIAL_V1.unsupported, Some(&UNSUPPORTED[..]));
    assert!(
        ADVERSARIAL_V1
            .assumptions
            .starts_with(ADVERSARIAL_V0.assumptions)
    );
    assert!(ADVERSARIAL_V1.assumptions.len() > ADVERSARIAL_V0.assumptions.len());
    // The Lab journal names the newest profile, the last registry entry.
    assert_eq!(PROFILES[PROFILES.len() - 1], ADVERSARIAL_V1);
    // Every profile the source declares is in `PROFILES`, so none escapes the registry.
    let declarations = PROFILE_SOURCE
        .matches(": FidelityProfile = FidelityProfile {")
        .count();
    assert_eq!(declarations, PROFILES.len());
}

/// The profile module's source, for the declaration count above.
const PROFILE_SOURCE: &str = include_str!("../src/profile.rs");
