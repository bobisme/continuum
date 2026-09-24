//! PR-15 / IMPL-01 (bn-3ohe): the `network/adversarial-v0` profile and its Lab handler.
//!
//! Evidence, by artifact id:
//!
//! | Id | Test | What it shows |
//! |---|---|---|
//! | `pr15-impl01-pos-01` | [`differential_the_lab_handler_equals_an_independent_reference_model_exhaustively`] | over the complete reachable space of four configurations (1,110 states, one under a binding retained-bytes budget), the Lab handler and an independently written reference model accept and refuse the same steps with the same refusals, emit the same events, and list the same enabled choices |
//! | `pr15-impl01-pos-02` | [`determinism_identical_choice_logs_give_byte_identical_journals`] | 2,100 seeded random logs: two runs of one log give identical bytes, and distinct logs give many distinct journals |
//! | `pr15-impl01-pos-03` | [`replay_a_journal_is_its_own_choice_log`] | a journal's choice log replays to the same journal, byte for byte |
//! | `pr15-impl01-pos-04` | [`cancellation_points_are_step_boundaries_and_every_prefix_replays_exactly`] | cutting a log at any step boundary gives exactly the journal prefix |
//! | `pr15-impl01-pos-05` | [`golden_a_register_shaped_log_journals_exactly_these_events`] | a replicated-register-shaped log with a duplicated `Commit`, a partition and a loss |
//! | `pr15-impl01-pos-06..08` | `fault_*` | delay, duplicate and partition each reachable and each doing exactly its row |
//! | `pr15-impl01-neg-01` | [`negative_every_unsupported_step_is_a_typed_unsupported_refusal_and_changes_nothing`] | every unsupported step is `Unsupported(<its row>)`, INV-008 `Unsupported`, and atomic |
//! | `pr15-impl01-neg-02` | [`negative_undeclared_faults_bounds_and_malformed_steps_are_typed`] | undeclared faults, the in-flight bound, oversize payloads, FIFO, partitions, malformed nodes and sides |
//! | `pr15-impl01-neg-04` | [`differential_is_not_vacuous_every_seeded_model_bug_is_caught`] | twelve seeded reference-model bugs, each caught by the exhaustive differential |
//! | `pr15-impl01-neg-03` | [`negative_a_perturbed_choice_log_changes_the_journal`] | anti-vacuity: one changed step changes the journal |
//! | `pr15-impl01-bnd-01` | [`boundary_an_overlong_log_is_refused_before_any_step_runs`] | the `MAX_STEPS` charge comes before any work |
//! | `pr15-impl01-bnd-02` | [`boundary_configuration_caps_are_typed_refusals`] | configuration caps |
//! | `pr15-impl01-hon-01` | [`honesty_the_profile_covers_every_rfc_0002_and_docs_17_network_semantic`] | every RFC 0002 / docs/17 §7 network semantic has one declared row |
//! | `pr15-impl01-hon-02` | [`honesty_the_profile_matches_the_replicated_register_scenario`] | name and switches equal `replicated_register.scenario.toml` |
//! | `pr15-impl01-hon-03` | [`profile_declares_host_qualification_none`] | T06: the profile claims no host semantics (the compiler lane proves no host effect) |
//! | `pr15-impl01-bnd-04` | [`boundary_the_retained_bytes_budget_is_charged_before_every_push`] | exact boundary, one byte over, send/deliver at in-flight 1, clone, replay and encode within the budget |
//! | `pr15-impl01-hon-05` | [`honesty_the_journal_encoding_is_pinned_byte_for_byte`] | the journal wire form is pinned |
//! | `pr15-impl01-bnd-03` | [`boundary_a_directly_driven_network_stops_at_max_steps_and_its_journal_replays`] | `Network::apply` has the same step bound, so its journals replay |
//! | `pr15-impl01-hon-04` | [`honesty_the_profile_bytes_are_pinned_to_its_version`] | a profile edit without a version bump fails |

use std::collections::BTreeSet;

use continuum_effects_network::lab::JOURNAL_HEADER_BYTES;
use continuum_effects_network::profile::{
    ASSUMPTIONS, CANCELLATION_CONTRACT, HostQualification, IndependenceClaim, PROFILE_NAME,
    PROFILE_VERSION,
};
use continuum_effects_network::refusal::{Bound, ConfigRefusal, Malformed, NotEnabled};
use continuum_effects_network::step::{
    MAX_IN_FLIGHT_CAP, MAX_PAYLOAD_CAP, MAX_RETAINED_CAP, MAX_STEPS,
};

/// The retained-bytes budget of every configuration that is not testing the budget.
const RETAINED: u64 = MAX_RETAINED_CAP;
use continuum_effects_network::{
    ADVERSARIAL_V0, EnvelopeId, Event, FaultSwitches, FidelityClass, Journal, Network,
    NetworkConfig, NodeId, NodeSet, Payload, Refusal, RefusalClass, Semantic, Step, Support, run,
};

// --- an independent reference model ------------------------------------------------------

/// The reference model, written from the profile rows and the module's refusal
/// precedence with different data structures: the in-flight multiset is a flat list of
/// copies, the partition is a membership vector, and every lookup is a scan.
#[derive(Debug, Clone)]
struct Model {
    mutant: Mutant,
    config: NetworkConfig,
    sent: Vec<(u8, u8, Vec<u8>)>,
    flight: Vec<u32>,
    side: Option<Vec<bool>>,
    used: u32,
    events: Vec<Event>,
}

/// Seeded bugs in the reference model. Each is a plausible Lab-handler defect; the
/// differential must see every one of them, or it is not checking anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mutant {
    None,
    DeliverIgnoresPartition,
    FifoIgnored,
    DuplicateIgnoresBound,
    DropRemovesEveryCopy,
    LossSwitchIgnored,
    DelayConsumesCopy,
    SideNotNormalized,
    OverlapAllowed,
    DuplicationSwitchIgnored,
    BudgetIgnored,
    HealKeepsPartition,
    BudgetUncharged,
}

type ModelKey = (
    Vec<(u8, u8, Vec<u8>)>,
    Vec<u32>,
    Option<Vec<bool>>,
    u32,
    Option<u64>,
);

impl Model {
    fn new(config: NetworkConfig, mutant: Mutant) -> Self {
        Self {
            mutant,
            config,
            sent: Vec::new(),
            flight: Vec::new(),
            side: None,
            used: 0,
            events: Vec::new(),
        }
    }

    fn key(&self) -> ModelKey {
        let mut flight = self.flight.clone();
        flight.sort_unstable();
        // Under a budget that can bind, the bytes retained so far decide the future,
        // so they are part of the state. Under the default budget no explored path gets
        // near it, and leaving it out keeps `Delay`, a stutter, from making the space
        // infinite.
        let retained = (self.config.max_retained_bytes() < RETAINED).then(|| self.retained());
        (
            self.sent.clone(),
            flight,
            self.side.clone(),
            self.used,
            retained,
        )
    }

    fn in_flight(&self, envelope: EnvelopeId) -> bool {
        self.flight.contains(&envelope.0)
    }

    fn remove_one(&mut self, envelope: EnvelopeId) {
        let at = self
            .flight
            .iter()
            .position(|copy| *copy == envelope.0)
            .expect("checked in flight");
        self.flight.remove(at);
    }

    /// The encoded journal size, computed independently: header plus a per-kind table.
    fn retained(&self) -> u64 {
        JOURNAL_HEADER_BYTES
            + self
                .events
                .iter()
                .map(|event| match event {
                    Event::Sent { payload, .. } => 11 + payload.0.len() as u64,
                    Event::Delivered { .. } => 7,
                    Event::Partitioned(_) => 9,
                    Event::Healed => 1,
                    _ => 5,
                })
                .sum::<u64>()
    }

    /// One step: the transition on a scratch copy, then the retained-bytes charge,
    /// then commit.
    fn step(&mut self, step: &Step) -> Result<Event, Refusal> {
        let mut next = self.clone();
        let event = next.transition(step)?;
        let max = self.config.max_retained_bytes();
        if next.retained() > max && self.mutant != Mutant::BudgetUncharged {
            return Err(Refusal::BoundReached(Bound::Retained { max }));
        }
        *self = next;
        Ok(event)
    }

    fn transition(&mut self, step: &Step) -> Result<Event, Refusal> {
        let n = self.config.nodes();
        let faults = self.config.faults();
        let max = self.config.max_in_flight();
        let event = match step {
            Step::OneWayPartition { .. } => {
                return Err(Refusal::Unsupported(Semantic::AsymmetricPartition));
            }
            Step::Corrupt(_) => return Err(Refusal::Unsupported(Semantic::Corruption)),
            Step::Forge { .. } => return Err(Refusal::Unsupported(Semantic::Forgery)),
            Step::ConnectionReset(..) => {
                return Err(Refusal::Unsupported(Semantic::ConnectionReset));
            }
            Step::CrashEndpoint(_) => return Err(Refusal::Unsupported(Semantic::EndpointCrash)),
            Step::Recall(_) => return Err(Refusal::Unsupported(Semantic::RecallInFlight)),
            Step::Send { src, dst, payload } => {
                for node in [src, dst] {
                    if node.0 >= n {
                        return Err(Refusal::Malformed(Malformed::UnknownNode(*node)));
                    }
                }
                if payload.0.len() > self.config.max_payload_bytes() as usize {
                    return Err(Refusal::BoundReached(Bound::Payload {
                        max: self.config.max_payload_bytes(),
                    }));
                }
                if self.flight.len() >= max as usize {
                    return Err(Refusal::BoundReached(Bound::InFlight { max }));
                }
                let id = u32::try_from(self.sent.len()).unwrap();
                self.sent.push((src.0, dst.0, payload.0.clone()));
                self.flight.push(id);
                Event::Sent {
                    envelope: EnvelopeId(id),
                    src: *src,
                    dst: *dst,
                    payload: payload.clone(),
                }
            }
            Step::Deliver(envelope) => {
                if !self.in_flight(*envelope) {
                    return Err(Refusal::NotEnabled(NotEnabled::NotInFlight(*envelope)));
                }
                let (src, dst, _) = self.sent[envelope.0 as usize].clone();
                if let Some(side) = self
                    .side
                    .as_ref()
                    .filter(|_| self.mutant != Mutant::DeliverIgnoresPartition)
                {
                    if side[src as usize] != side[dst as usize] {
                        return Err(Refusal::NotEnabled(NotEnabled::Partitioned(*envelope)));
                    }
                }
                if !faults.reordering && self.mutant != Mutant::FifoIgnored {
                    let ahead = self
                        .flight
                        .iter()
                        .copied()
                        .filter(|copy| {
                            let (s, d, _) = &self.sent[*copy as usize];
                            *s == src && *d == dst
                        })
                        .min()
                        .expect("the envelope itself is on the link");
                    if ahead != envelope.0 {
                        return Err(Refusal::NotEnabled(NotEnabled::WouldReorder {
                            envelope: *envelope,
                            ahead: EnvelopeId(ahead),
                        }));
                    }
                }
                self.remove_one(*envelope);
                Event::Delivered {
                    envelope: *envelope,
                    src: NodeId(src),
                    dst: NodeId(dst),
                }
            }
            Step::Drop(envelope) => {
                if !faults.loss && self.mutant != Mutant::LossSwitchIgnored {
                    return Err(Refusal::NotEnabled(NotEnabled::FaultNotDeclared(
                        Semantic::Loss,
                    )));
                }
                if !self.in_flight(*envelope) {
                    return Err(Refusal::NotEnabled(NotEnabled::NotInFlight(*envelope)));
                }
                if self.mutant == Mutant::DropRemovesEveryCopy {
                    self.flight.retain(|copy| *copy != envelope.0);
                } else {
                    self.remove_one(*envelope);
                }
                Event::Dropped(*envelope)
            }
            Step::Duplicate(envelope) => {
                if !faults.duplication && self.mutant != Mutant::DuplicationSwitchIgnored {
                    return Err(Refusal::NotEnabled(NotEnabled::FaultNotDeclared(
                        Semantic::Duplication,
                    )));
                }
                if !self.in_flight(*envelope) {
                    return Err(Refusal::NotEnabled(NotEnabled::NotInFlight(*envelope)));
                }
                if self.flight.len() >= max as usize && self.mutant != Mutant::DuplicateIgnoresBound
                {
                    return Err(Refusal::BoundReached(Bound::InFlight { max }));
                }
                self.flight.push(envelope.0);
                Event::Duplicated(*envelope)
            }
            Step::Delay(envelope) => {
                if !self.in_flight(*envelope) {
                    return Err(Refusal::NotEnabled(NotEnabled::NotInFlight(*envelope)));
                }
                if self.mutant == Mutant::DelayConsumesCopy {
                    self.remove_one(*envelope);
                }
                Event::Delayed(*envelope)
            }
            Step::Partition(set) => {
                if n < 64 && set.0 >> n != 0 {
                    return Err(Refusal::Malformed(Malformed::SideOutsideNodes(*set)));
                }
                let mut members: Vec<bool> = (0..n).map(|i| (set.0 >> i) & 1 == 1).collect();
                if !members[0] && self.mutant != Mutant::SideNotNormalized {
                    for member in &mut members {
                        *member = !*member;
                    }
                }
                if members.iter().all(|m| *m) || members.iter().all(|m| !*m) {
                    return Err(Refusal::Malformed(Malformed::TrivialSide(*set)));
                }
                let budget = self.config.max_partitions();
                if budget == 0 {
                    return Err(Refusal::NotEnabled(NotEnabled::FaultNotDeclared(
                        Semantic::SymmetricPartition,
                    )));
                }
                if self.used >= budget && self.mutant != Mutant::BudgetIgnored {
                    return Err(Refusal::NotEnabled(NotEnabled::PartitionBudgetSpent {
                        max: budget,
                    }));
                }
                if self.side.is_some() && self.mutant != Mutant::OverlapAllowed {
                    return Err(Refusal::Unsupported(Semantic::OverlappingPartitions));
                }
                let bits = members
                    .iter()
                    .enumerate()
                    .filter(|(_, m)| **m)
                    .fold(0_u64, |acc, (i, _)| acc | (1 << i));
                self.side = Some(members);
                self.used += 1;
                Event::Partitioned(NodeSet(bits))
            }
            Step::Heal => {
                if self.side.is_none() {
                    return Err(Refusal::NotEnabled(NotEnabled::NoPartition));
                }
                if self.mutant != Mutant::HealKeepsPartition {
                    self.side = None;
                }
                Event::Healed
            }
        };
        self.events.push(event.clone());
        Ok(event)
    }
}

// --- helpers --------------------------------------------------------------------------

fn config(nodes: u8, max_in_flight: u32, faults: FaultSwitches, partitions: u32) -> NetworkConfig {
    NetworkConfig::new(nodes, max_in_flight, 4, faults, partitions, RETAINED).expect("valid config")
}

const ALL_FAULTS: FaultSwitches = FaultSwitches {
    loss: true,
    duplication: true,
    reordering: true,
};

fn send(src: u8, dst: u8, payload: &[u8]) -> Step {
    Step::Send {
        src: NodeId(src),
        dst: NodeId(dst),
        payload: Payload(payload.to_vec()),
    }
}

fn unsupported_steps() -> Vec<Step> {
    vec![
        Step::OneWayPartition {
            from: NodeSet(1),
            to: NodeSet(2),
        },
        Step::Corrupt(EnvelopeId(0)),
        Step::Forge {
            src: NodeId(0),
            dst: NodeId(1),
            payload: Payload(b"x".to_vec()),
        },
        Step::ConnectionReset(NodeId(0), NodeId(1)),
        Step::CrashEndpoint(NodeId(0)),
        Step::Recall(EnvelopeId(0)),
    ]
}

/// Every step worth trying in a state: the next program send, every envelope-indexed
/// choice for every sent envelope and one never sent, every partition side including
/// one outside the nodes, heal, and every unsupported step.
fn candidates(model: &Model, program: &[Step]) -> Vec<Step> {
    let mut out = Vec::new();
    if let Some(next) = program.get(model.sent.len()) {
        out.push(next.clone());
    }
    // A send the configuration cannot take: an unknown receiver, and a payload over
    // the byte bound.
    out.push(send(0, model.config.nodes(), b"x"));
    out.push(send(
        0,
        1,
        &vec![0; model.config.max_payload_bytes() as usize + 1],
    ));
    for id in 0..=u32::try_from(model.sent.len()).unwrap() {
        let envelope = EnvelopeId(id);
        out.extend([
            Step::Deliver(envelope),
            Step::Drop(envelope),
            Step::Duplicate(envelope),
            Step::Delay(envelope),
        ]);
    }
    let n = model.config.nodes();
    for bits in 0..=(1_u64 << n) {
        out.push(Step::Partition(NodeSet(bits)));
    }
    out.push(Step::Heal);
    out.extend(unsupported_steps());
    out
}

/// A small xorshift; the seed is explicit, so every walk reproduces from it (INV-005).
struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(n).unwrap()).unwrap()
    }
}

const SEEDS: [u64; 7] = [1, 2, 7, 42, 99, 0x9e37_79b9_7f4a_7c15, u64::MAX];

/// A seeded random walk: at each step, an enabled choice, a send, or a partition.
fn random_log(rng: &mut XorShift, cfg: NetworkConfig, len: usize) -> Vec<Step> {
    let mut net = Network::new(cfg);
    let mut log = Vec::new();
    let n = cfg.nodes();
    while log.len() < len {
        let mut options = net.enabled_choices();
        let src = u8::try_from(rng.below(n.into())).unwrap();
        let dst = u8::try_from(rng.below(n.into())).unwrap();
        let byte = u8::try_from(rng.below(256)).unwrap();
        options.push(send(src, dst, &[byte]));
        options.push(Step::Partition(NodeSet(rng.next() & NodeSet::all(n).0)));
        let pick = options[rng.below(options.len())].clone();
        if net.apply(&pick).is_ok() {
            log.push(pick);
        }
    }
    log
}

// --- positive evidence ----------------------------------------------------------------

/// `pr15-impl01-pos-01`. Differential against the reference model, over the complete
/// reachable space (no depth bound: `Delay` is a stutter, so the visited set closes
/// it), under four configurations that between them switch every fault on and off,
/// one of them under a retained-bytes budget that binds.
#[test]
fn differential_the_lab_handler_equals_an_independent_reference_model_exhaustively() {
    let mut kinds = BTreeSet::new();
    let mut refusals = BTreeSet::new();
    let mut states = 0;
    for cfg in differential_configs() {
        let stats = explore(cfg, Mutant::None).unwrap_or_else(|why| panic!("{why}"));
        assert!(
            stats.states > 20,
            "{} states is too few to mean anything",
            stats.states
        );
        assert!(stats.transitions > stats.states);
        states += stats.states;
        kinds.extend(stats.kinds);
        refusals.extend(stats.refusals);
    }
    // The reachable spaces are fixed by the configurations and the program, so the
    // count is pinned exactly: a change means the explored space changed.
    assert_eq!(states, 1_110, "the explored space changed");
    assert_eq!(kinds.len(), 7, "every event kind was reached: {kinds:?}");
    assert_eq!(
        refusals.len(),
        4,
        "every refusal class was reached: {refusals:?}"
    );
}

/// `pr15-impl01-neg-04`. Anti-vacuity of the differential: each of twelve seeded bugs in
/// the reference model makes the exhaustive comparison fail.
#[test]
fn differential_is_not_vacuous_every_seeded_model_bug_is_caught() {
    let mutants = [
        Mutant::DeliverIgnoresPartition,
        Mutant::FifoIgnored,
        Mutant::DuplicateIgnoresBound,
        Mutant::DropRemovesEveryCopy,
        Mutant::LossSwitchIgnored,
        Mutant::DelayConsumesCopy,
        Mutant::SideNotNormalized,
        Mutant::OverlapAllowed,
        Mutant::DuplicationSwitchIgnored,
        Mutant::BudgetIgnored,
        Mutant::HealKeepsPartition,
        Mutant::BudgetUncharged,
    ];
    for mutant in mutants {
        let caught = differential_configs()
            .into_iter()
            .any(|cfg| explore(cfg, mutant).is_err());
        assert!(caught, "{mutant:?} survived the differential");
    }
}

fn differential_configs() -> [NetworkConfig; 4] {
    [
        // A retained-bytes budget of the header plus 40 bytes: the budget, not the
        // in-flight bound, ends most paths.
        NetworkConfig::new(3, 4, 4, ALL_FAULTS, 1, JOURNAL_HEADER_BYTES + 40).unwrap(),
        config(3, 4, ALL_FAULTS, 1),
        config(
            3,
            4,
            FaultSwitches {
                loss: false,
                duplication: true,
                reordering: false,
            },
            0,
        ),
        config(
            3,
            2,
            FaultSwitches {
                loss: true,
                duplication: false,
                reordering: false,
            },
            2,
        ),
    ]
}

struct Stats {
    states: usize,
    transitions: usize,
    kinds: BTreeSet<String>,
    refusals: BTreeSet<String>,
}

fn variant(debug: String) -> String {
    debug
        .split([' ', '(', '{'])
        .next()
        .unwrap_or_default()
        .to_owned()
}

/// The observable state: envelopes sent, in-flight copies, partition side, budget used.
type View = (usize, Vec<(u32, u32)>, Option<u64>, u32, u64);

fn view_of_network(net: &Network) -> View {
    (
        net.sent_count(),
        net.in_flight().map(|(e, n)| (e.0, n)).collect(),
        net.partition().map(|side| side.0),
        net.partitions_used(),
        net.retained_bytes(),
    )
}

fn view_of_model(model: &Model) -> View {
    let mut counts: std::collections::BTreeMap<u32, u32> = std::collections::BTreeMap::new();
    for copy in &model.flight {
        *counts.entry(*copy).or_default() += 1;
    }
    let side = model.side.as_ref().map(|members| {
        members
            .iter()
            .enumerate()
            .filter(|(_, m)| **m)
            .fold(0_u64, |acc, (i, _)| acc | (1 << i))
    });
    (
        model.sent.len(),
        counts.into_iter().collect(),
        side,
        model.used,
        model.retained(),
    )
}

/// Explore the complete reachable space of `cfg` from the empty network, comparing the
/// Lab handler with the reference model on every candidate step of every state. `Err`
/// names the first disagreement.
fn explore(cfg: NetworkConfig, mutant: Mutant) -> Result<Stats, String> {
    let program = [
        send(0, 1, b"a"),
        send(1, 0, b"b"),
        send(0, 1, b"c"),
        send(2, 1, b"d"),
    ];
    let mut stats = Stats {
        states: 0,
        transitions: 0,
        kinds: BTreeSet::new(),
        refusals: BTreeSet::new(),
    };
    let mut visited = BTreeSet::new();
    visited.insert(Model::new(cfg, mutant).key());
    let mut stack = vec![(Network::new(cfg), Model::new(cfg, mutant))];
    while let Some((net, model)) = stack.pop() {
        stats.states += 1;
        if net.events() != model.events.as_slice()
            || net.in_flight_total() as usize != model.flight.len()
        {
            return Err(format!("state diverged at {:?}", model.key()));
        }
        let mut model_enabled = Vec::new();
        for step in candidates(&model, &program) {
            let mut net_next = net.clone();
            let mut model_next = model.clone();
            let got = net_next.apply(&step).cloned();
            let want = model_next.step(&step);
            if got != want {
                return Err(format!(
                    "{step:?} from {:?}: {got:?} != {want:?}",
                    model.key()
                ));
            }
            if net.check(&step) != want.as_ref().map(|_| ()).map_err(|r| *r) {
                return Err(format!("check disagrees with apply on {step:?}"));
            }
            match want {
                Err(refusal) => {
                    if net_next != net {
                        return Err(format!("refused {step:?} changed the network"));
                    }
                    stats.refusals.insert(variant(format!("{refusal:?}")));
                }
                Ok(event) => {
                    // Compare the whole observable state after every accepted step, not
                    // only at newly visited states: a divergence that lands on an
                    // already visited model state would otherwise go unseen.
                    if view_of_network(&net_next) != view_of_model(&model_next) {
                        return Err(format!("state diverged after {step:?}"));
                    }
                    stats.transitions += 1;
                    stats.kinds.insert(variant(format!("{event:?}")));
                    if !matches!(step, Step::Send { .. } | Step::Partition(_)) {
                        model_enabled.push(step.clone());
                    }
                    if visited.insert(model_next.key()) {
                        stack.push((net_next, model_next));
                    }
                }
            }
        }
        if net.enabled_choices() != model_enabled {
            return Err(format!("enabled choices differ at {:?}", model.key()));
        }
    }
    Ok(stats)
}

/// `pr15-impl01-pos-02`. Identical choice logs give byte-identical journals, over
/// seeded random walks under two configurations; distinct logs are not collapsed.
#[test]
fn determinism_identical_choice_logs_give_byte_identical_journals() {
    let configs = [
        NetworkConfig::new(5, 8, 8, ALL_FAULTS, 2, RETAINED).unwrap(),
        NetworkConfig::new(
            4,
            6,
            8,
            FaultSwitches {
                loss: true,
                duplication: true,
                reordering: false,
            },
            1,
            RETAINED,
        )
        .unwrap(),
    ];
    let mut distinct = BTreeSet::new();
    let mut logs = 0;
    for cfg in configs {
        for seed in SEEDS {
            let mut rng = XorShift(seed);
            for _ in 0..150 {
                let len = 1 + rng.below(40);
                let log = random_log(&mut rng, cfg, len);
                let first = run(cfg, &log).expect("a generated log is valid");
                let second = run(cfg, &log).expect("and still valid");
                assert_eq!(first.encode(), second.encode());
                assert_eq!(first.encode().len() as u64, first.retained_bytes());
                assert_eq!(first.events().len(), log.len());
                distinct.insert(first.encode());
                logs += 1;
            }
        }
    }
    assert_eq!(logs, 2_100);
    assert!(
        distinct.len() > 1_900,
        "only {} distinct journals",
        distinct.len()
    );
}

/// `pr15-impl01-pos-03`. A journal's own choice log replays to the same journal.
#[test]
fn replay_a_journal_is_its_own_choice_log() {
    let cfg = NetworkConfig::new(5, 8, 8, ALL_FAULTS, 2, RETAINED).unwrap();
    for seed in SEEDS {
        let mut rng = XorShift(seed);
        for _ in 0..100 {
            let len = 1 + rng.below(40);
            let log = random_log(&mut rng, cfg, len);
            let journal = run(cfg, &log).unwrap();
            // The journal records a partition by its normalized side (the one that holds
            // node 0), so its log equals the input up to that normalization.
            let normalized: Vec<Step> = log
                .iter()
                .map(|step| match step {
                    Step::Partition(side) if !side.contains(NodeId(0)) => {
                        Step::Partition(NodeSet(NodeSet::all(cfg.nodes()).0 & !side.0))
                    }
                    other => other.clone(),
                })
                .collect();
            assert_eq!(journal.choice_log().collect::<Vec<_>>(), normalized);
            let replayed = journal.replay().unwrap();
            assert_eq!(replayed, journal);
            assert_eq!(replayed.encode(), journal.encode());
        }
    }
}

/// `pr15-impl01-pos-04`. Cancellation lands only between steps. So cutting a log at any
/// boundary is a valid run whose journal is exactly the full journal's prefix, and a
/// crash window or cancellation point in the caller replays exactly.
#[test]
fn cancellation_points_are_step_boundaries_and_every_prefix_replays_exactly() {
    let cfg = NetworkConfig::new(4, 8, 8, ALL_FAULTS, 1, RETAINED).unwrap();
    let mut rng = XorShift(104_729);
    for _ in 0..50 {
        let log = random_log(&mut rng, cfg, 30);
        let full = run(cfg, &log).unwrap();
        for cut in 0..=log.len() {
            let prefix = run(cfg, &log[..cut]).unwrap();
            assert_eq!(prefix.events(), &full.events()[..cut]);
        }
    }
}

/// `pr15-impl01-pos-05`. A replicated-register-shaped log: a proposal, a duplicated
/// `Commit` delivered twice (the pack's side of mutant M04), a lost `Committed`, a
/// partition that blocks a delivery until it heals, and a delay.
#[test]
fn golden_a_register_shaped_log_journals_exactly_these_events() {
    let cfg = NetworkConfig::replicated_register_scenario(4, 64, RETAINED).unwrap();
    let client = 3;
    let log = vec![
        send(client, 0, b"Propose(1,v)"),
        Step::Deliver(EnvelopeId(0)),
        send(0, 1, b"Commit(1,v)"),
        Step::Duplicate(EnvelopeId(1)),
        Step::Deliver(EnvelopeId(1)),
        send(1, 0, b"Committed(1,v)"),
        Step::Drop(EnvelopeId(2)),
        Step::Partition(NodeSet(0b1101)),
        Step::Delay(EnvelopeId(1)),
        Step::Heal,
        Step::Deliver(EnvelopeId(1)),
    ];
    let mut net = Network::new(cfg);
    for step in &log[..8] {
        net.apply(step).unwrap();
    }
    assert_eq!(
        net.check(&Step::Deliver(EnvelopeId(1))),
        Err(Refusal::NotEnabled(NotEnabled::Partitioned(EnvelopeId(1))))
    );
    let journal = run(cfg, &log).unwrap();
    let payload = |bytes: &[u8]| Payload(bytes.to_vec());
    assert_eq!(
        journal.events(),
        &[
            Event::Sent {
                envelope: EnvelopeId(0),
                src: NodeId(3),
                dst: NodeId(0),
                payload: payload(b"Propose(1,v)"),
            },
            Event::Delivered {
                envelope: EnvelopeId(0),
                src: NodeId(3),
                dst: NodeId(0),
            },
            Event::Sent {
                envelope: EnvelopeId(1),
                src: NodeId(0),
                dst: NodeId(1),
                payload: payload(b"Commit(1,v)"),
            },
            Event::Duplicated(EnvelopeId(1)),
            Event::Delivered {
                envelope: EnvelopeId(1),
                src: NodeId(0),
                dst: NodeId(1),
            },
            Event::Sent {
                envelope: EnvelopeId(2),
                src: NodeId(1),
                dst: NodeId(0),
                payload: payload(b"Committed(1,v)"),
            },
            Event::Dropped(EnvelopeId(2)),
            Event::Partitioned(NodeSet(0b1101)),
            Event::Delayed(EnvelopeId(1)),
            Event::Healed,
            Event::Delivered {
                envelope: EnvelopeId(1),
                src: NodeId(0),
                dst: NodeId(1),
            },
        ]
    );
}

/// `pr15-impl01-pos-06`. Fault coverage, `delay`: an explicit stutter. The envelope
/// stays in flight with its copy count, nothing else moves, and it can still be
/// delivered after any number of delays — there is no delay bound.
#[test]
fn fault_delay_holds_an_envelope_with_no_bound_and_no_other_effect() {
    let cfg = NetworkConfig::new(2, 4, 4, ALL_FAULTS, 0, RETAINED).unwrap();
    let mut net = Network::new(cfg);
    net.apply(&send(0, 1, b"m")).unwrap();
    for _ in 0..1_000 {
        let before = net.in_flight().collect::<Vec<_>>();
        assert_eq!(
            net.apply(&Step::Delay(EnvelopeId(0))).unwrap(),
            &Event::Delayed(EnvelopeId(0))
        );
        assert_eq!(net.in_flight().collect::<Vec<_>>(), before);
    }
    assert!(net.apply(&Step::Deliver(EnvelopeId(0))).is_ok());
    assert_eq!(
        net.check(&Step::Delay(EnvelopeId(0))),
        Err(Refusal::NotEnabled(NotEnabled::NotInFlight(EnvelopeId(0))))
    );
}

/// `pr15-impl01-pos-07`. Fault coverage, `duplicate`: each duplicate adds one copy of
/// the same envelope, each copy is delivered once, and the copies share the ordinal —
/// the idempotence key a receiver needs to survive M04.
#[test]
fn fault_duplicate_adds_copies_that_share_the_envelope_ordinal() {
    let cfg = NetworkConfig::new(2, 3, 4, ALL_FAULTS, 0, RETAINED).unwrap();
    let mut net = Network::new(cfg);
    net.apply(&send(0, 1, b"m")).unwrap();
    net.apply(&Step::Duplicate(EnvelopeId(0))).unwrap();
    net.apply(&Step::Duplicate(EnvelopeId(0))).unwrap();
    assert_eq!(net.copies(EnvelopeId(0)), 3);
    assert_eq!(
        net.check(&Step::Duplicate(EnvelopeId(0))),
        Err(Refusal::BoundReached(Bound::InFlight { max: 3 }))
    );
    for _ in 0..3 {
        assert_eq!(
            net.apply(&Step::Deliver(EnvelopeId(0))).unwrap(),
            &Event::Delivered {
                envelope: EnvelopeId(0),
                src: NodeId(0),
                dst: NodeId(1)
            }
        );
    }
    assert_eq!(net.copies(EnvelopeId(0)), 0);
    assert_eq!(net.sent_count(), 1);
}

/// `pr15-impl01-pos-08`. Fault coverage, `partition`: symmetric, both ways, envelopes
/// stay in flight, sides normalize to the side holding node 0, and heal restores
/// delivery; drop and duplicate still act across the cut.
#[test]
fn fault_partition_blocks_both_ways_until_heal() {
    let cfg = NetworkConfig::new(3, 8, 4, ALL_FAULTS, 1, RETAINED).unwrap();
    let mut net = Network::new(cfg);
    net.apply(&send(0, 2, b"a")).unwrap();
    net.apply(&send(2, 0, b"b")).unwrap();
    net.apply(&send(0, 1, b"c")).unwrap();
    // {2} normalizes to {0, 1}.
    assert_eq!(
        net.apply(&Step::Partition(NodeSet(0b100))).unwrap(),
        &Event::Partitioned(NodeSet(0b011))
    );
    for blocked in [EnvelopeId(0), EnvelopeId(1)] {
        assert_eq!(
            net.check(&Step::Deliver(blocked)),
            Err(Refusal::NotEnabled(NotEnabled::Partitioned(blocked)))
        );
        assert!(net.check(&Step::Drop(blocked)).is_ok());
        assert!(net.check(&Step::Duplicate(blocked)).is_ok());
    }
    assert!(net.check(&Step::Deliver(EnvelopeId(2))).is_ok());
    net.apply(&Step::Heal).unwrap();
    assert!(net.apply(&Step::Deliver(EnvelopeId(0))).is_ok());
    assert!(net.apply(&Step::Deliver(EnvelopeId(1))).is_ok());
    assert_eq!(
        net.check(&Step::Partition(NodeSet(0b001))),
        Err(Refusal::NotEnabled(NotEnabled::PartitionBudgetSpent {
            max: 1
        }))
    );
}

// --- negative evidence ----------------------------------------------------------------

/// `pr15-impl01-neg-01`. Every step the profile does not model is refused as
/// `Unsupported` of exactly the row the step names, that row is declared unsupported,
/// the INV-008 reading is `Unsupported`, and the network does not change.
#[test]
fn negative_every_unsupported_step_is_a_typed_unsupported_refusal_and_changes_nothing() {
    let cfg = NetworkConfig::new(3, 8, 4, ALL_FAULTS, 2, RETAINED).unwrap();
    let mut net = Network::new(cfg);
    net.apply(&send(0, 1, b"a")).unwrap();
    net.apply(&Step::Partition(NodeSet(0b001))).unwrap();
    let mut probes = unsupported_steps();
    probes.push(Step::Partition(NodeSet(0b011)));
    for step in probes {
        let before = net.clone();
        let refusal = net.apply(&step).expect_err("unsupported");
        let Refusal::Unsupported(semantic) = refusal else {
            panic!("{step:?} gave {refusal:?}");
        };
        if let Some(named) = step.unsupported_semantic() {
            assert_eq!(semantic, named);
        }
        assert_eq!(semantic.support(), Support::Unsupported, "{semantic}");
        assert_eq!(refusal.class(), RefusalClass::Unsupported);
        assert_eq!(refusal.inconclusive_reason(), Some("Unsupported"));
        assert!(refusal.to_string().contains(semantic.statement()));
        assert_eq!(net, before, "{step:?} changed the network");
    }
    // Framing has no step: an oversize payload is the payload bound, not framing.
    assert_eq!(
        net.apply(&send(0, 1, b"toolong")),
        Err(Refusal::BoundReached(Bound::Payload { max: 4 }))
    );
    let covered: BTreeSet<Semantic> = unsupported_steps()
        .iter()
        .filter_map(Step::unsupported_semantic)
        .collect();
    assert_eq!(covered.len(), 6);
}

/// `pr15-impl01-neg-02`. Undeclared faults, the in-flight bound, FIFO, partitions and
/// malformed input each give their own typed refusal.
#[test]
fn negative_undeclared_faults_bounds_and_malformed_steps_are_typed() {
    let fifo = NetworkConfig::new(
        3,
        2,
        4,
        FaultSwitches {
            loss: false,
            duplication: false,
            reordering: false,
        },
        0,
        RETAINED,
    )
    .unwrap();
    let mut net = Network::new(fifo);
    net.apply(&send(0, 1, b"a")).unwrap();
    net.apply(&send(0, 1, b"b")).unwrap();
    let cases = [
        (
            Step::Drop(EnvelopeId(0)),
            Refusal::NotEnabled(NotEnabled::FaultNotDeclared(Semantic::Loss)),
        ),
        (
            Step::Duplicate(EnvelopeId(0)),
            Refusal::NotEnabled(NotEnabled::FaultNotDeclared(Semantic::Duplication)),
        ),
        (
            Step::Deliver(EnvelopeId(1)),
            Refusal::NotEnabled(NotEnabled::WouldReorder {
                envelope: EnvelopeId(1),
                ahead: EnvelopeId(0),
            }),
        ),
        (
            send(0, 2, b"c"),
            Refusal::BoundReached(Bound::InFlight { max: 2 }),
        ),
        (
            Step::Partition(NodeSet(0b001)),
            Refusal::NotEnabled(NotEnabled::FaultNotDeclared(Semantic::SymmetricPartition)),
        ),
        (Step::Heal, Refusal::NotEnabled(NotEnabled::NoPartition)),
        (
            Step::Deliver(EnvelopeId(9)),
            Refusal::NotEnabled(NotEnabled::NotInFlight(EnvelopeId(9))),
        ),
        (
            send(3, 0, b"c"),
            Refusal::Malformed(Malformed::UnknownNode(NodeId(3))),
        ),
        (
            send(0, 200, b"c"),
            Refusal::Malformed(Malformed::UnknownNode(NodeId(200))),
        ),
        (
            Step::Partition(NodeSet(0b1001)),
            Refusal::Malformed(Malformed::SideOutsideNodes(NodeSet(0b1001))),
        ),
        (
            Step::Partition(NodeSet(0b111)),
            Refusal::Malformed(Malformed::TrivialSide(NodeSet(0b111))),
        ),
        (
            Step::Partition(NodeSet(0)),
            Refusal::Malformed(Malformed::TrivialSide(NodeSet(0))),
        ),
    ];
    for (step, want) in cases {
        let before = net.clone();
        assert_eq!(net.apply(&step), Err(want), "{step:?}");
        assert_eq!(net, before);
    }
    assert_eq!(
        Refusal::BoundReached(Bound::InFlight { max: 2 }).inconclusive_reason(),
        Some("ResourceExhausted")
    );
    assert_eq!(
        Refusal::NotEnabled(NotEnabled::NoPartition).inconclusive_reason(),
        None
    );
    assert_eq!(
        Refusal::Malformed(Malformed::TrivialSide(NodeSet(0))).class(),
        RefusalClass::Malformed
    );
    // FIFO still lets the head of the link go first.
    net.apply(&Step::Deliver(EnvelopeId(0))).unwrap();
    net.apply(&Step::Deliver(EnvelopeId(1))).unwrap();
    // A refused step inside a log names its index, and nothing after it runs.
    assert_eq!(
        run(
            fifo,
            &[send(0, 1, b"a"), Step::Drop(EnvelopeId(0)), Step::Heal]
        )
        .unwrap_err()
        .index,
        1
    );
}

/// `pr15-impl01-neg-03`. Anti-vacuity: changing one step of a valid log changes the
/// journal, or makes the log invalid, and never leaves the journal as it was.
#[test]
fn negative_a_perturbed_choice_log_changes_the_journal() {
    let cfg = NetworkConfig::new(4, 8, 8, ALL_FAULTS, 1, RETAINED).unwrap();
    let mut rng = XorShift(7);
    let mut changed = 0;
    for _ in 0..200 {
        let log = random_log(&mut rng, cfg, 20);
        let journal = run(cfg, &log).unwrap();
        let at = rng.below(log.len());
        let mut perturbed = log.clone();
        perturbed[at] = match &log[at] {
            Step::Send { src, dst, payload } => {
                let mut bytes = payload.0.clone();
                bytes.push(0);
                Step::Send {
                    src: *src,
                    dst: *dst,
                    payload: Payload(bytes),
                }
            }
            Step::Deliver(e) => Step::Drop(*e),
            Step::Drop(e) | Step::Duplicate(e) => Step::Delay(*e),
            Step::Delay(e) => Step::Deliver(*e),
            Step::Partition(_) => Step::Heal,
            Step::Heal => Step::Partition(NodeSet(1)),
            other => panic!("the generator does not emit {other:?}"),
        };
        match run(cfg, &perturbed) {
            Ok(other) => {
                assert_ne!(other.encode(), journal.encode(), "{:?}", log[at]);
                changed += 1;
            }
            Err(refusal) => assert!(refusal.index >= at),
        }
    }
    assert!(changed > 100);
}

/// `pr15-impl01-bnd-01`. The choice-log length is charged before any step runs.
#[test]
fn boundary_an_overlong_log_is_refused_before_any_step_runs() {
    let cfg = NetworkConfig::new(2, 2, 2, ALL_FAULTS, 0, RETAINED).unwrap();
    // Every step would be refused as `NoPartition` at index 0 if it ran.
    let log = vec![Step::Heal; MAX_STEPS + 1];
    let refusal = run(cfg, &log).unwrap_err();
    assert_eq!(refusal.index, MAX_STEPS + 1);
    assert_eq!(
        refusal.refusal,
        Refusal::BoundReached(Bound::Steps { max: MAX_STEPS })
    );
}

/// `pr15-impl01-bnd-02`. Configuration caps.
#[test]
fn boundary_configuration_caps_are_typed_refusals() {
    assert_eq!(
        NetworkConfig::new(0, 1, 1, ALL_FAULTS, 0, RETAINED),
        Err(ConfigRefusal::NodesOutOfRange(0))
    );
    assert_eq!(
        NetworkConfig::new(65, 1, 1, ALL_FAULTS, 0, RETAINED),
        Err(ConfigRefusal::NodesOutOfRange(65))
    );
    assert_eq!(
        NetworkConfig::new(2, 0, 1, ALL_FAULTS, 0, RETAINED),
        Err(ConfigRefusal::InFlightOutOfRange(0))
    );
    assert_eq!(
        NetworkConfig::new(2, MAX_IN_FLIGHT_CAP + 1, 1, ALL_FAULTS, 0, RETAINED),
        Err(ConfigRefusal::InFlightOutOfRange(MAX_IN_FLIGHT_CAP + 1))
    );
    assert_eq!(
        NetworkConfig::new(2, 1, MAX_PAYLOAD_CAP + 1, ALL_FAULTS, 0, RETAINED),
        Err(ConfigRefusal::PayloadOutOfRange(MAX_PAYLOAD_CAP + 1))
    );
    // 64 nodes is the largest set a `NodeSet` holds, and a partition over it works.
    let wide = NetworkConfig::new(64, 1, 0, ALL_FAULTS, 1, RETAINED).unwrap();
    let mut net = Network::new(wide);
    assert_eq!(
        net.apply(&Step::Partition(NodeSet(1 << 63))).unwrap(),
        &Event::Partitioned(NodeSet(!(1 << 63)))
    );
    assert!(net.apply(&send(63, 0, b"")).is_ok());
    assert_eq!(NodeSet::of(&[0, 64]), None);
    assert_eq!(NodeSet::of(&[0, 63]), Some(NodeSet(1 | (1 << 63))));
}

// --- honesty of the profile -----------------------------------------------------------

const RFC_0002: &str =
    include_str!("../../../notes/plan/rfcs/0002-controlled-effects-and-domain-packs.md");
const DOCS_17: &str = include_str!("../../../notes/plan/docs/17_DOMAIN_PACK_CONTRACT.md");
const SCENARIO: &str =
    include_str!("../../../notes/plan/examples/replicated_register.scenario.toml");

/// The text between a heading and the next heading of any level.
fn section<'t>(text: &'t str, heading: &str) -> &'t str {
    let start = text.find(heading).expect("heading present") + heading.len();
    let rest = &text[start..];
    let end = rest.find("\n#").unwrap_or(rest.len());
    &rest[..end]
}

/// `pr15-impl01-hon-01`. Every network semantic RFC 0002 "Network" and docs/17 §7
/// "Choices" name maps to declared rows; every row is declared once with a statement.
/// A new bullet in either document fails this test until the profile states it.
#[test]
fn honesty_the_profile_covers_every_rfc_0002_and_docs_17_network_semantic() {
    use Semantic as S;
    let rfc: &[(&str, &[Semantic])] = &[
        (
            "loss, duplication, reordering",
            &[S::Loss, S::Duplication, S::Reordering],
        ),
        (
            "partition topology",
            &[
                S::SymmetricPartition,
                S::AsymmetricPartition,
                S::OverlappingPartitions,
            ],
        ),
        (
            "bounded/unbounded delay assumptions",
            &[S::BoundedDelay, S::UnboundedDelay],
        ),
        ("connection epochs", &[S::ConnectionEpochs]),
        ("half-open behavior", &[S::HalfOpen]),
        (
            "backpressure and capacity",
            &[S::Backpressure, S::InFlightBound],
        ),
        (
            "corruption/authentication assumptions",
            &[S::Corruption, S::Forgery],
        ),
        (
            "DNS/service-discovery behavior when relevant",
            &[S::ServiceDiscovery],
        ),
    ];
    let network = section(RFC_0002, "### Network");
    let bullets: Vec<&str> = network
        .lines()
        .filter_map(|line| line.strip_prefix("- "))
        .map(|line| line.trim_end_matches([';', '.']))
        .collect();
    assert_eq!(
        bullets,
        rfc.iter().map(|(bullet, _)| *bullet).collect::<Vec<_>>(),
        "RFC 0002's network list changed; state the new semantics in the profile"
    );
    let docs17: &[(&str, &[Semantic])] = &[
        ("deliver", &[S::Reordering]),
        ("delay", &[S::UnboundedDelay]),
        ("drop", &[S::Loss]),
        ("duplicate", &[S::Duplication]),
        (
            "reorder through choice of deliverable envelope",
            &[S::Reordering],
        ),
        ("partition/heal", &[S::SymmetricPartition]),
        ("connection reset", &[S::ConnectionReset]),
        ("endpoint crash", &[S::EndpointCrash]),
    ];
    let choices = section(section(DOCS_17, "## 7. Network pack baseline"), "Choices:");
    // The list runs from the first bullet to the first non-bullet, non-blank line
    // ("Optional profiles:"), so a new choice anywhere in it is seen.
    let listed: Vec<&str> = choices
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map_while(|line| line.strip_prefix("- "))
        .map(|line| line.trim_end_matches([';', '.']))
        .collect();
    assert_eq!(
        listed,
        docs17.iter().map(|(choice, _)| *choice).collect::<Vec<_>>(),
        "docs/17 §7's choice list changed"
    );
    let mut mapped: BTreeSet<Semantic> = BTreeSet::new();
    for (_, rows) in rfc.iter().chain(docs17) {
        mapped.extend(rows.iter().copied());
    }
    mapped.extend([S::Framing, S::RecallInFlight]);
    assert_eq!(mapped, Semantic::ALL.into_iter().collect());
    let tokens: BTreeSet<&str> = Semantic::ALL.iter().map(|s| s.token()).collect();
    assert_eq!(tokens.len(), Semantic::ALL.len());
    for semantic in Semantic::ALL {
        assert!(semantic.statement().len() > 30, "{semantic}");
    }
    let modelled: Vec<Semantic> = Semantic::ALL
        .into_iter()
        .filter(|s| s.support() == Support::Modelled)
        .collect();
    assert_eq!(
        modelled,
        [
            S::Loss,
            S::Duplication,
            S::Reordering,
            S::UnboundedDelay,
            S::SymmetricPartition,
            S::InFlightBound
        ]
    );
    assert_eq!(ASSUMPTIONS[0].0, "network-no-forgery");
    assert_eq!(CANCELLATION_CONTRACT.len(), 8);
}

/// `pr15-impl01-hon-02`. The profile name and the scenario helper are the scenario's
/// own values, read from `replicated_register.scenario.toml`.
#[test]
fn honesty_the_profile_matches_the_replicated_register_scenario() {
    let network = section(SCENARIO, "[network]");
    let faults = section(SCENARIO, "[faults]");
    let value = |table: &str, key: &str| -> String {
        table
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{key} = ")))
            .unwrap_or_else(|| panic!("{key} missing"))
            .trim_matches('"')
            .to_owned()
    };
    assert_eq!(value(network, "profile"), PROFILE_NAME);
    let cfg = NetworkConfig::replicated_register_scenario(4, 64, RETAINED).unwrap();
    assert_eq!(
        value(network, "max_in_flight"),
        cfg.max_in_flight().to_string()
    );
    assert_eq!(value(network, "allow_loss"), cfg.faults().loss.to_string());
    assert_eq!(
        value(network, "allow_duplication"),
        cfg.faults().duplication.to_string()
    );
    assert_eq!(
        value(network, "allow_reordering"),
        cfg.faults().reordering.to_string()
    );
    assert_eq!(
        value(faults, "max_partitions"),
        cfg.max_partitions().to_string()
    );
}

/// `pr15-impl01-hon-03`. T06: the declared profile claims no host semantics. This is
/// the test GOV-4-13's not-applicable form names as the source of truth for the value
/// (`tools/governance/reviews/bn-3ohe.toml`). The absence of host effects themselves is
/// the compiler's to prove: the crate is `#![no_std]`, and `tests/pr15_no_std_lane.rs`
/// shows that proof is live.
#[test]
fn profile_declares_host_qualification_none() {
    assert_eq!(ADVERSARIAL_V0.host, HostQualification::None);
    assert_eq!(ADVERSARIAL_V0.class, FidelityClass::AdversarialEnvelope);
    assert_ne!(ADVERSARIAL_V0.class, FidelityClass::PlatformQualified);
    assert_eq!(ADVERSARIAL_V0.independence, IndependenceClaim::AllDependent);
}

/// `pr15-impl01-hon-04`. The profile's canonical bytes are pinned together with its
/// version (docs/17 §12). Editing a row, a statement, an assumption or the cancellation
/// contract changes the fingerprint and fails this test. The rule is to bump
/// `PROFILE_VERSION` and re-pin both; this test catches an unnoticed edit, but a
/// deliberate re-pin without a bump is caught only by review.
#[test]
fn honesty_the_profile_bytes_are_pinned_to_its_version() {
    let bytes = ADVERSARIAL_V0.canonical_bytes();
    let fingerprint = bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    });
    assert_eq!(PROFILE_VERSION.to_string(), "0.1.0");
    assert_eq!(
        (PROFILE_VERSION.to_string(), fingerprint),
        ("0.1.0".to_owned(), PINNED_FINGERPRINT),
        "network/adversarial-v0 changed: bump PROFILE_VERSION, then re-pin"
    );
    let journal: Journal = run(
        NetworkConfig::new(2, 1, 1, ALL_FAULTS, 0, RETAINED).unwrap(),
        &[send(0, 1, b"z")],
    )
    .unwrap();
    let encoded = journal.encode();
    let name = PROFILE_NAME.as_bytes();
    assert!(encoded.windows(name.len()).any(|w| w == name));
}

/// `pr15-impl01-hon-05`. The journal's wire form is pinned byte for byte, so a change
/// to tags, field order, widths or endianness fails here rather than passing every
/// self-consistency test.
#[test]
fn honesty_the_journal_encoding_is_pinned_byte_for_byte() {
    let cfg = NetworkConfig::new(3, 4, 4, ALL_FAULTS, 1, RETAINED).unwrap();
    let log = [
        send(0, 2, b"ab"),
        Step::Duplicate(EnvelopeId(0)),
        Step::Deliver(EnvelopeId(0)),
        Step::Drop(EnvelopeId(0)),
        send(2, 1, b""),
        Step::Delay(EnvelopeId(1)),
        Step::Partition(NodeSet(0b010)),
        Step::Heal,
    ];
    let hex: String = run(cfg, &log)
        .unwrap()
        .encode()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(hex, PINNED_JOURNAL_HEX, "the journal wire form changed");
}

const PINNED_JOURNAL_HEX: &str = "636f6e74696e75756d2d6e6574776f726b2d6a6f75726e616c00000000166e6574776f726b2f616476657273617269616c2d763000000001000003000000040000000407000000010000000010000000000000080100000000000200000002616204000000000200000000000203000000000100000001020100000000050000000106000000000000000507";

/// `pr15-impl01-bnd-03`. A network driven step by step, without `run`, stops at the
/// same `MAX_STEPS` bound, so every journal it produces replays.
#[test]
fn boundary_a_directly_driven_network_stops_at_max_steps_and_its_journal_replays() {
    let cfg = NetworkConfig::new(2, 1, 1, ALL_FAULTS, 0, RETAINED).unwrap();
    let mut net = Network::new(cfg);
    net.apply(&send(0, 1, b"m")).unwrap();
    for _ in 1..MAX_STEPS {
        net.apply(&Step::Delay(EnvelopeId(0))).unwrap();
    }
    assert_eq!(
        net.apply(&Step::Delay(EnvelopeId(0))),
        Err(Refusal::BoundReached(Bound::Steps { max: MAX_STEPS }))
    );
    let journal = net.into_journal();
    assert_eq!(journal.events().len(), MAX_STEPS);
    assert_eq!(journal.replay().unwrap(), journal);
}

const PINNED_FINGERPRINT: u64 = 6_176_911_479_697_058_609;

/// `pr15-impl01-bnd-04`. The retained-bytes budget bounds the whole journal, is
/// charged before anything is pushed, and holds for clone, replay and encode.
#[test]
fn boundary_the_retained_bytes_budget_is_charged_before_every_push() {
    let one_send = JOURNAL_HEADER_BYTES + 11 + 4;
    // Exact boundary: a 4-byte send fits exactly.
    let exact = NetworkConfig::new(2, 4, 4, ALL_FAULTS, 0, one_send).unwrap();
    let mut net = Network::new(exact);
    net.apply(&send(0, 1, b"abcd")).unwrap();
    assert_eq!(net.retained_bytes(), one_send);
    assert_eq!(net.encode().len() as u64, one_send);
    let before = net.clone();
    assert_eq!(
        net.apply(&Step::Delay(EnvelopeId(0))),
        Err(Refusal::BoundReached(Bound::Retained { max: one_send }))
    );
    assert_eq!(net, before, "a refused charge changed the network");
    assert_eq!(
        Refusal::BoundReached(Bound::Retained { max: one_send }).inconclusive_reason(),
        Some("ResourceExhausted")
    );
    // One byte over: the same send is refused before anything is retained.
    let short = NetworkConfig::new(2, 4, 4, ALL_FAULTS, 0, one_send - 1).unwrap();
    let mut net = Network::new(short);
    assert_eq!(
        net.apply(&send(0, 1, b"abcd")),
        Err(Refusal::BoundReached(Bound::Retained { max: one_send - 1 }))
    );
    assert_eq!(net.retained_bytes(), JOURNAL_HEADER_BYTES);
    assert!(net.events().is_empty());
    // Send and deliver at in-flight 1: the in-flight bound never binds, the budget
    // does, after exactly as many rounds as it pays for.
    let rounds = 10_u64;
    let budget = JOURNAL_HEADER_BYTES + rounds * (11 + 4 + 7);
    let cfg = NetworkConfig::new(2, 1, 4, ALL_FAULTS, 0, budget).unwrap();
    let mut net = Network::new(cfg);
    let mut done = 0;
    loop {
        let id = u32::try_from(net.sent_count()).unwrap();
        if let Err(refusal) = net.apply(&send(0, 1, b"wxyz")) {
            assert_eq!(
                refusal,
                Refusal::BoundReached(Bound::Retained { max: budget })
            );
            break;
        }
        assert_eq!(net.in_flight_total(), 1);
        net.apply(&Step::Deliver(EnvelopeId(id))).unwrap();
        done += 1;
    }
    assert_eq!(done, rounds);
    assert_eq!(net.retained_bytes(), budget);
    // Clone, replay and encode all stay within the budget.
    let journal = net.into_journal();
    let copy = journal.clone();
    assert_eq!(copy.retained_bytes(), budget);
    let replayed = journal.replay().unwrap();
    assert_eq!(replayed, journal);
    assert_eq!(replayed.encode().len() as u64, budget);
    assert_eq!(journal.encode(), replayed.encode());
    // Configuration bounds on the budget itself.
    assert_eq!(
        NetworkConfig::new(2, 1, 1, ALL_FAULTS, 0, JOURNAL_HEADER_BYTES - 1),
        Err(ConfigRefusal::RetainedOutOfRange(JOURNAL_HEADER_BYTES - 1))
    );
    assert_eq!(
        NetworkConfig::new(2, 1, 1, ALL_FAULTS, 0, MAX_RETAINED_CAP + 1),
        Err(ConfigRefusal::RetainedOutOfRange(MAX_RETAINED_CAP + 1))
    );
    // The header alone is a valid, empty journal.
    let bare = NetworkConfig::new(2, 1, 1, ALL_FAULTS, 0, JOURNAL_HEADER_BYTES).unwrap();
    assert_eq!(
        Network::new(bare).encode().len() as u64,
        JOURNAL_HEADER_BYTES
    );
}
