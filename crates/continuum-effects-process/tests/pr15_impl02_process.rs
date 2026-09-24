//! PR-15 / IMPL-02 (bn-3mmf): the `process/crash-restart-v0` profile and its Lab handler.
//!
//! Evidence, by artifact id:
//!
//! | Id | Test | What it shows |
//! |---|---|---|
//! | `pr15-impl02-pos-01` | [`differential_the_lab_handler_equals_an_independent_reference_model_exhaustively`] | over the complete reachable space of four configurations, each under a binding retained-bytes budget, pruned by a key that holds the journal length and retained bytes (cr-35ujnx), the Lab handler and an independently written reference model accept and refuse the same steps with the same refusals, emit the same events, reach the same states, and list the same enabled choices |
//! | `pr15-impl02-neg-04` | [`differential_is_not_vacuous_every_seeded_model_bug_is_caught`] | fourteen seeded reference-model bugs, each caught by the exhaustive differential |
//! | `pr15-impl02-neg-05` | [`differential_key_is_the_whole_input_of_a_step`] | cr-35ujnx: two histories with one pruning key have the same outcome, event and successor key for every candidate, so the exhaustive exploration hides no successor |
//! | `pr15-impl02-pos-02` | [`determinism_identical_choice_logs_give_byte_identical_journals`] | 2,100 seeded random logs: two runs of one log give identical bytes, and distinct logs give many distinct journals |
//! | `pr15-impl02-pos-03` | [`replay_a_journal_is_its_own_choice_log`] | a journal's choice log replays to the same journal, byte for byte |
//! | `pr15-impl02-pos-04` | [`every_step_boundary_truncation_replays_as_the_journal_prefix`] | cutting a log at any step boundary gives exactly the journal prefix |
//! | `pr15-impl02-pos-05` | [`crash_windows_replay_exactly_and_a_crash_in_any_window_fences_that_incarnation`] | a crash, or a crash and restart, spliced into every step boundary of seeded logs: the run keeps the journal prefix, replays byte for byte, and fences exactly the completions of the crashed incarnation |
//! | `pr15-impl02-pos-06` | [`golden_a_register_shaped_log_journals_exactly_these_events`] | replica crashes with a write and a timer in flight: both completions fenced (M08), the new incarnation's completion delivered, the third crash refused |
//! | `pr15-impl02-pos-07` | [`fault_crash_stops_an_incarnation_and_fences_its_pending_completions`] | the `crash` fault row |
//! | `pr15-impl02-pos-08` | [`fault_delay_a_completion_delayed_past_its_incarnation_is_fenced_never_delivered`] | a completion delayed with no bound is delivered while its incarnation runs, and fenced once it crashed |
//! | `pr15-impl02-pos-09` | [`restart_epochs_strictly_increase_and_are_never_reused`] | M03: a restarted node never reuses an epoch |
//! | `pr15-impl02-neg-01` | [`negative_every_unsupported_step_is_a_typed_unsupported_refusal_and_changes_nothing`] | every unsupported step is `Unsupported(<its row>)`, INV-008 `Unsupported`, and atomic |
//! | `pr15-impl02-neg-02` | [`negative_undeclared_faults_bounds_and_malformed_steps_are_typed`] | undeclared faults, the crash budget, the pending bound, down and up nodes, unknown nodes and tickets |
//! | `pr15-impl02-neg-03` | [`negative_a_perturbed_choice_log_changes_the_journal`] | anti-vacuity: one changed step changes the journal |
//! | `pr15-impl02-bnd-01` | [`boundary_an_overlong_log_is_refused_before_any_step_runs`] | the `MAX_STEPS` charge comes before any work |
//! | `pr15-impl02-bnd-02` | [`boundary_configuration_caps_are_typed_refusals`] | configuration caps |
//! | `pr15-impl02-bnd-03` | [`boundary_a_directly_driven_process_stops_at_max_steps_and_its_journal_replays`] | `Process::apply` has the same step bound, so its journals replay |
//! | `pr15-impl02-bnd-04` | [`boundary_the_retained_bytes_budget_is_charged_before_every_push`] | exact boundary, one byte over, begin/complete at pending 1, clone, replay and encode within the budget |
//! | `pr15-impl02-hon-01` | [`honesty_the_profile_covers_every_rfc_0002_and_docs_17_process_semantic`] | every RFC 0002 process-lifecycle bullet and every docs/17 §10 process constraint has one declared row |
//! | `pr15-impl02-hon-02` | [`honesty_the_scenario_helper_matches_the_replicated_register_scenario_and_contract`] | `max_crashes` equals the scenario's, and the contract enables `crash` and `recovery` |
//! | `pr15-impl02-hon-03` | [`profile_declares_host_qualification_none`] | T06: the profile claims no host semantics (the compiler lane proves no host effect) |
//! | `pr15-impl02-hon-04` | [`honesty_the_profile_bytes_are_pinned_to_its_version`] | a profile edit without a version bump fails |
//! | `pr15-impl02-hon-05` | [`honesty_the_journal_encoding_is_pinned_byte_for_byte`] | the journal wire form is pinned |
//! | `pr15-impl02-hon-06` | [`honesty_the_network_pack_defers_crash_semantics_here_and_this_profile_states_them`] | the network profile's crash and epoch rows defer to this pack, and this profile's composition table answers for `network/adversarial-v0` |

use std::collections::BTreeSet;

use continuum_effects_process::lab::JOURNAL_HEADER_BYTES;
use continuum_effects_process::profile::{
    ASSUMPTIONS, CANCELLATION_CONTRACT, COMPOSITION, HostQualification, IndependenceClaim,
    PROFILE_NAME, PROFILE_VERSION,
};
use continuum_effects_process::refusal::{Bound, ConfigRefusal, Malformed, NotEnabled};
use continuum_effects_process::step::{
    MAX_CRASHES_CAP, MAX_PENDING_CAP, MAX_RETAINED_CAP, MAX_STEPS,
};
use continuum_effects_process::{
    CRASH_RESTART_V0, Epoch, Event, FidelityClass, Journal, NodeId, NodeSet, Process,
    ProcessConfig, Refusal, RefusalClass, Semantic, Step, Support, TicketId, run,
};

/// The retained-bytes budget of every configuration that is not testing the budget.
const RETAINED: u64 = MAX_RETAINED_CAP;

// --- an independent reference model ------------------------------------------------------

/// A ticket's state in the reference model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Status {
    Pending,
    Completed,
    Fenced,
}

/// The reference model, written from the profile rows and the module's refusal
/// precedence with different data structures: nodes are a vector of `(up, epoch)`
/// pairs, tickets a vector of `(node, epoch, status)` triples, and every lookup is a
/// scan.
#[derive(Debug, Clone)]
struct Model {
    mutant: Mutant,
    config: ProcessConfig,
    nodes: Vec<(bool, u32)>,
    tickets: Vec<(u8, u32, Status)>,
    crashes: u32,
    events: Vec<Event>,
}

/// Seeded bugs in the reference model. Each is a plausible Lab-handler defect; the
/// differential must see every one of them, or it is not checking anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mutant {
    None,
    /// M03: restart keeps the old epoch.
    RestartReusesEpoch,
    CrashIgnoresBudget,
    CrashWhileDown,
    RestartWhileUp,
    /// A completion is judged by the epoch only, so one arriving while the node is down
    /// is delivered.
    FenceIgnoresDown,
    /// M08: a completion is judged by liveness only, so an old incarnation's completion
    /// reaches the new one.
    FenceIgnoresEpoch,
    /// A crash silently resolves the incarnation's pending tickets, as if it rolled back
    /// their external effects, so no late completion can arrive.
    CrashDropsPending,
    PendingBoundIgnored,
    BeginWhileDown,
    RestartSwitchIgnored,
    BudgetUncharged,
    /// A resolved ticket stays pending, so it can resolve twice.
    CompleteTwice,
    /// `Delay` resolves the ticket instead of stuttering.
    DelayResolves,
    /// The node-state check comes before the crash budget.
    PrecedenceSwap,
}

const MUTANTS: [Mutant; 14] = [
    Mutant::RestartReusesEpoch,
    Mutant::CrashIgnoresBudget,
    Mutant::CrashWhileDown,
    Mutant::RestartWhileUp,
    Mutant::FenceIgnoresDown,
    Mutant::FenceIgnoresEpoch,
    Mutant::CrashDropsPending,
    Mutant::PendingBoundIgnored,
    Mutant::BeginWhileDown,
    Mutant::RestartSwitchIgnored,
    Mutant::BudgetUncharged,
    Mutant::CompleteTwice,
    Mutant::DelayResolves,
    Mutant::PrecedenceSwap,
];

type ModelKey = (Vec<(bool, u32)>, Vec<(u8, u32, Status)>, u32, usize, u64);

impl Model {
    fn new(config: ProcessConfig, mutant: Mutant) -> Self {
        Self {
            mutant,
            config,
            nodes: vec![(true, 0); usize::from(config.nodes())],
            tickets: Vec::new(),
            crashes: 0,
            events: Vec::new(),
        }
    }

    /// The exploration's pruning key: everything a step and the candidate generator
    /// read (cr-35ujnx). That is the node and ticket state and the crashes used, which
    /// fix the next `Begin` too, and the journal length and retained bytes, which the
    /// step bound and the retained-bytes budget are charged against. Only the event
    /// contents are left out, and no step reads them.
    fn key(&self) -> ModelKey {
        (
            self.nodes.clone(),
            self.tickets.clone(),
            self.crashes,
            self.events.len(),
            self.retained(),
        )
    }

    /// The encoded journal size, computed independently: header plus a per-kind table.
    fn retained(&self) -> u64 {
        JOURNAL_HEADER_BYTES
            + self
                .events
                .iter()
                .map(|event| match event {
                    Event::Crashed { .. } | Event::Restarted { .. } => 6,
                    _ => 10,
                })
                .sum::<u64>()
    }

    /// One step: the transition on a scratch copy, then the retained-bytes charge,
    /// then commit.
    fn step(&mut self, step: &Step) -> Result<Event, Refusal> {
        if self.events.len() >= MAX_STEPS {
            return Err(Refusal::BoundReached(Bound::Steps { max: MAX_STEPS }));
        }
        let mut next = self.clone();
        let event = next.transition(step)?;
        let max = self.config.max_retained_bytes();
        if next.retained() > max && self.mutant != Mutant::BudgetUncharged {
            return Err(Refusal::BoundReached(Bound::Retained { max }));
        }
        *self = next;
        Ok(event)
    }

    fn node(&self, node: NodeId) -> Result<usize, Refusal> {
        if usize::from(node.0) < self.nodes.len() {
            Ok(usize::from(node.0))
        } else {
            Err(Refusal::Malformed(Malformed::UnknownNode(node)))
        }
    }

    fn transition(&mut self, step: &Step) -> Result<Event, Refusal> {
        let m = self.mutant;
        let event = match step {
            Step::CancelGracefully(_) => {
                return Err(Refusal::Unsupported(Semantic::GracefulCancellation));
            }
            Step::Panic(_) => return Err(Refusal::Unsupported(Semantic::Panic)),
            Step::PowerLoss(_) => return Err(Refusal::Unsupported(Semantic::PowerLoss)),
            Step::Begin(node) => {
                let n = self.node(*node)?;
                let (up, epoch) = self.nodes[n];
                if !up && m != Mutant::BeginWhileDown {
                    return Err(Refusal::NotEnabled(NotEnabled::NodeDown(*node)));
                }
                let pending = self
                    .tickets
                    .iter()
                    .filter(|(_, _, s)| *s == Status::Pending)
                    .count();
                let max = self.config.max_pending();
                if pending >= max as usize && m != Mutant::PendingBoundIgnored {
                    return Err(Refusal::BoundReached(Bound::Pending { max }));
                }
                let ticket = TicketId(u32::try_from(self.tickets.len()).unwrap());
                self.tickets.push((node.0, epoch, Status::Pending));
                Event::Begun {
                    ticket,
                    node: *node,
                    epoch: Epoch(epoch),
                }
            }
            Step::Delay(ticket) => {
                let at = ticket.0 as usize;
                let Some(&(node, epoch, status)) = self.tickets.get(at) else {
                    return Err(Refusal::NotEnabled(NotEnabled::NotPending(*ticket)));
                };
                if status != Status::Pending {
                    return Err(Refusal::NotEnabled(NotEnabled::NotPending(*ticket)));
                }
                if m == Mutant::DelayResolves {
                    self.tickets[at].2 = Status::Fenced;
                }
                Event::Delayed {
                    ticket: *ticket,
                    node: NodeId(node),
                    epoch: Epoch(epoch),
                }
            }
            Step::Complete(ticket) => {
                let at = ticket.0 as usize;
                let Some(&(node, epoch, status)) = self.tickets.get(at) else {
                    return Err(Refusal::NotEnabled(NotEnabled::NotPending(*ticket)));
                };
                if status != Status::Pending {
                    return Err(Refusal::NotEnabled(NotEnabled::NotPending(*ticket)));
                }
                let (up, current) = self.nodes[usize::from(node)];
                let live = match m {
                    Mutant::FenceIgnoresDown => current == epoch,
                    Mutant::FenceIgnoresEpoch => up,
                    _ => up && current == epoch,
                };
                if m != Mutant::CompleteTwice {
                    self.tickets[at].2 = if live {
                        Status::Completed
                    } else {
                        Status::Fenced
                    };
                }
                let (ticket, node, epoch) = (*ticket, NodeId(node), Epoch(epoch));
                if live {
                    Event::Completed {
                        ticket,
                        node,
                        epoch,
                    }
                } else {
                    Event::Fenced {
                        ticket,
                        node,
                        epoch,
                    }
                }
            }
            Step::Crash(node) => {
                let n = self.node(*node)?;
                let max = self.config.max_crashes();
                if max == 0 {
                    return Err(Refusal::NotEnabled(NotEnabled::FaultNotDeclared(
                        Semantic::FailStopCrash,
                    )));
                }
                let (up, epoch) = self.nodes[n];
                if m == Mutant::PrecedenceSwap && !up {
                    return Err(Refusal::NotEnabled(NotEnabled::NodeDown(*node)));
                }
                if self.crashes >= max && m != Mutant::CrashIgnoresBudget {
                    return Err(Refusal::NotEnabled(NotEnabled::CrashBudgetSpent { max }));
                }
                if !up && m != Mutant::CrashWhileDown {
                    return Err(Refusal::NotEnabled(NotEnabled::NodeDown(*node)));
                }
                self.nodes[n].0 = false;
                self.crashes += 1;
                if m == Mutant::CrashDropsPending {
                    for (tn, te, s) in &mut self.tickets {
                        if *tn == node.0 && *te == epoch && *s == Status::Pending {
                            *s = Status::Fenced;
                        }
                    }
                }
                Event::Crashed {
                    node: *node,
                    epoch: Epoch(epoch),
                }
            }
            Step::Restart(node) => {
                let n = self.node(*node)?;
                if !self.config.restart() && m != Mutant::RestartSwitchIgnored {
                    return Err(Refusal::NotEnabled(NotEnabled::FaultNotDeclared(
                        Semantic::RestartNewEpoch,
                    )));
                }
                let (up, epoch) = self.nodes[n];
                if up && m != Mutant::RestartWhileUp {
                    return Err(Refusal::NotEnabled(NotEnabled::NodeUp(*node)));
                }
                let next = if m == Mutant::RestartReusesEpoch {
                    epoch
                } else {
                    epoch + 1
                };
                self.nodes[n] = (true, next);
                Event::Restarted {
                    node: *node,
                    epoch: Epoch(next),
                }
            }
        };
        self.events.push(event);
        Ok(event)
    }
}

// --- helpers --------------------------------------------------------------------------

fn config(nodes: u8, crashes: u32, restart: bool, pending: u32) -> ProcessConfig {
    ProcessConfig::new(nodes, crashes, restart, pending, RETAINED).expect("valid config")
}

fn unsupported_steps() -> Vec<Step> {
    vec![
        Step::CancelGracefully(NodeId(0)),
        Step::Panic(NodeId(0)),
        Step::PowerLoss(NodeSet(0b11)),
    ]
}

/// The fixed program of the differential: which node begins the next ticket.
const PROGRAM: [u8; 4] = [0, 1, 0, 2];

/// Every step worth trying in a state, in the order `enabled_choices` lists the
/// accepted ones: the next program `Begin` and one at an unknown node; `Complete` for
/// every ticket begun and one never begun; `Crash` and `Restart` at every node and one
/// unknown node; and every unsupported step.
fn candidates(model: &Model) -> Vec<Step> {
    let n = model.config.nodes();
    let mut out = Vec::new();
    if let Some(next) = PROGRAM.get(model.tickets.len()) {
        out.push(Step::Begin(NodeId(*next % n)));
    }
    out.push(Step::Begin(NodeId(n)));
    for id in 0..=u32::try_from(model.tickets.len()).unwrap() {
        out.push(Step::Complete(TicketId(id)));
        out.push(Step::Delay(TicketId(id)));
    }
    for node in 0..=n {
        out.push(Step::Crash(NodeId(node)));
        out.push(Step::Restart(NodeId(node)));
    }
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

/// A seeded random walk: at each step, an enabled choice or a `Begin` at a random node.
/// A `Begin` is offered twice, so programs keep opening tickets.
fn random_log(rng: &mut XorShift, cfg: ProcessConfig, len: usize) -> Vec<Step> {
    let mut process = Process::new(cfg);
    let mut log = Vec::new();
    let n = cfg.nodes();
    let mut attempts = 0;
    while log.len() < len && attempts < len * 50 {
        attempts += 1;
        let mut options = process.enabled_choices();
        for _ in 0..2 {
            options.push(Step::Begin(NodeId(
                u8::try_from(rng.below(n.into())).unwrap(),
            )));
        }
        let pick = options[rng.below(options.len())];
        if process.apply(&pick).is_ok() {
            log.push(pick);
        }
    }
    log
}

// --- positive evidence ----------------------------------------------------------------

/// `pr15-impl02-pos-01`. Differential against the reference model, over the complete
/// reachable space (no depth bound: every configuration has a retained-bytes budget
/// that binds, so every path ends, `Delay` stutters included), under four
/// configurations that between them switch every fault on and off. The pruning key is
/// the whole input of a step, the journal length and retained bytes included
/// (cr-35ujnx).
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
    assert_eq!(states, PINNED_STATES, "the explored space changed");
    assert_eq!(kinds.len(), 6, "every event kind was reached: {kinds:?}");
    assert_eq!(
        refusals.len(),
        4,
        "every refusal class was reached: {refusals:?}"
    );
}

const PINNED_STATES: usize = 21_819;

/// `pr15-impl02-neg-04`. Anti-vacuity of the differential: each of fourteen seeded bugs in
/// the reference model makes the exhaustive comparison fail.
#[test]
fn differential_is_not_vacuous_every_seeded_model_bug_is_caught() {
    for mutant in MUTANTS {
        let caught = differential_configs()
            .into_iter()
            .any(|cfg| explore(cfg, mutant).is_err());
        assert!(caught, "{mutant:?} survived the differential");
    }
}

/// The retained-bytes budget past the header of the differential's unbudgeted-shape
/// configurations: large enough for their programs, small enough to end every path.
const BUDGET: u64 = 140;

fn differential_configs() -> [ProcessConfig; 4] {
    [
        // A retained-bytes budget of the header plus 60 bytes: the budget, not the
        // program or the crash budget, ends most paths.
        ProcessConfig::new(3, 2, true, 2, JOURNAL_HEADER_BYTES + 60).unwrap(),
        // The key holds the journal length and the retained bytes, so every
        // configuration has a budget that binds: `Delay`, a stutter, then ends too.
        ProcessConfig::new(3, 2, true, 3, JOURNAL_HEADER_BYTES + BUDGET).unwrap(),
        ProcessConfig::new(2, 1, false, 2, JOURNAL_HEADER_BYTES + BUDGET).unwrap(),
        ProcessConfig::new(3, 0, true, 4, JOURNAL_HEADER_BYTES + BUDGET).unwrap(),
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

/// The observable state: up set, epochs, ticket facts, pending tickets, crashes used,
/// retained bytes.
type View = (u64, Vec<u32>, Vec<(u8, u32)>, Vec<u32>, u32, usize, u64);

fn view_of_process(p: &Process) -> View {
    let n = p.config().nodes();
    (
        p.up().0,
        (0..n).map(|i| p.epoch(NodeId(i)).unwrap().0).collect(),
        (0..u32::try_from(p.ticket_count()).unwrap())
            .map(|t| {
                let (node, epoch) = p.ticket(TicketId(t)).unwrap();
                (node.0, epoch.0)
            })
            .collect(),
        p.pending().map(|t| t.0).collect(),
        p.crashes(),
        p.events().len(),
        p.retained_bytes(),
    )
}

fn view_of_model(m: &Model) -> View {
    (
        m.nodes
            .iter()
            .enumerate()
            .filter(|(_, (up, _))| *up)
            .fold(0_u64, |acc, (i, _)| acc | (1 << i)),
        m.nodes.iter().map(|(_, e)| *e).collect(),
        m.tickets.iter().map(|(n, e, _)| (*n, *e)).collect(),
        m.tickets
            .iter()
            .enumerate()
            .filter(|(_, (_, _, s))| *s == Status::Pending)
            .map(|(i, _)| u32::try_from(i).unwrap())
            .collect(),
        m.crashes,
        m.events.len(),
        m.retained(),
    )
}

/// Explore the complete reachable space of `cfg` from the initial state, comparing the
/// Lab handler with the reference model on every candidate step of every state. `Err`
/// names the first disagreement.
fn explore(cfg: ProcessConfig, mutant: Mutant) -> Result<Stats, String> {
    let mut stats = Stats {
        states: 0,
        transitions: 0,
        kinds: BTreeSet::new(),
        refusals: BTreeSet::new(),
    };
    let mut visited = BTreeSet::new();
    visited.insert(Model::new(cfg, mutant).key());
    let mut stack = vec![(Process::new(cfg), Model::new(cfg, mutant))];
    while let Some((process, model)) = stack.pop() {
        stats.states += 1;
        if process.events() != model.events.as_slice() {
            return Err(format!("journal diverged at {:?}", model.key()));
        }
        let mut model_enabled = Vec::new();
        for step in candidates(&model) {
            let mut process_next = process.clone();
            let mut model_next = model.clone();
            let got = process_next.apply(&step).copied();
            let want = model_next.step(&step);
            if got != want {
                return Err(format!(
                    "{step:?} from {:?}: {got:?} != {want:?}",
                    model.key()
                ));
            }
            if process.check(&step) != want.map(|_| ()) {
                return Err(format!("check disagrees with apply on {step:?}"));
            }
            match want {
                Err(refusal) => {
                    if process_next != process {
                        return Err(format!("refused {step:?} changed the process"));
                    }
                    stats
                        .refusals
                        .insert(variant(format!("{:?}", refusal.class())));
                }
                Ok(event) => {
                    // Compare the whole observable state after every accepted step, not
                    // only at newly visited states: a divergence that lands on an
                    // already visited model state would otherwise go unseen.
                    if view_of_process(&process_next) != view_of_model(&model_next) {
                        return Err(format!("state diverged after {step:?}"));
                    }
                    stats.transitions += 1;
                    stats.kinds.insert(variant(format!("{event:?}")));
                    if !matches!(step, Step::Begin(_)) {
                        model_enabled.push(step);
                    }
                    if visited.insert(model_next.key()) {
                        stack.push((process_next, model_next));
                    }
                }
            }
        }
        if process.enabled_choices() != model_enabled {
            return Err(format!("enabled choices differ at {:?}", model.key()));
        }
    }
    Ok(stats)
}

/// `pr15-impl02-pos-02`. Identical choice logs give byte-identical journals, over
/// seeded random walks under two configurations; distinct logs are not collapsed.
#[test]
fn determinism_identical_choice_logs_give_byte_identical_journals() {
    let configs = [config(5, 4, true, 6), config(3, 2, false, 4)];
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
        distinct.len() > 1_800,
        "only {} distinct journals",
        distinct.len()
    );
}

/// `pr15-impl02-pos-03`. A journal's own choice log replays to the same journal.
#[test]
fn replay_a_journal_is_its_own_choice_log() {
    let cfg = config(5, 4, true, 6);
    for seed in SEEDS {
        let mut rng = XorShift(seed);
        for _ in 0..100 {
            let len = 1 + rng.below(40);
            let log = random_log(&mut rng, cfg, len);
            let journal = run(cfg, &log).unwrap();
            assert_eq!(journal.choice_log().collect::<Vec<_>>(), log);
            let replayed = journal.replay().unwrap();
            assert_eq!(replayed, journal);
            assert_eq!(replayed.encode(), journal.encode());
        }
    }
}

/// `pr15-impl02-pos-04`. Every step is atomic, so cutting a log at any boundary is a
/// valid run whose journal is exactly the full journal's prefix. This is step-boundary
/// truncation. Graceful cancellation is not modelled by this pack (it is refused
/// `Unsupported`), so cancellation points proper are the runtime's evidence (PR 14).
#[test]
fn every_step_boundary_truncation_replays_as_the_journal_prefix() {
    let cfg = config(4, 3, true, 6);
    let mut rng = XorShift(104_729);
    for _ in 0..50 {
        let log = random_log(&mut rng, cfg, 30);
        let full = run(cfg, &log).unwrap();
        for cut in 0..=log.len() {
            let prefix = run(cfg, &log[..cut]).unwrap();
            assert_eq!(prefix.events(), &full.events()[..cut]);
            assert_eq!(
                prefix.encode()[JOURNAL_HEADER_BYTES as usize..],
                full.encode()[JOURNAL_HEADER_BYTES as usize..prefix.retained_bytes() as usize]
            );
        }
    }
}

/// `pr15-impl02-pos-05`. The PR-15 exit's "crash windows replay exactly", at the pack.
/// A crash window is a step boundary. For seeded logs, a `Crash` of each node, and a
/// `Crash` followed by a `Restart`, is spliced into every boundary. Whenever the spliced
/// log is a valid run:
///
/// - the journal up to the crash is the original's, event for event;
/// - the whole spliced journal replays from its own choice log, byte for byte, and a run
///   of the same spliced log gives the same bytes again;
/// - every completion, after the crash, of a ticket the crashed incarnation began is
///   `Fenced`, and none of them is `Completed`.
///
/// When the spliced log is not a valid run, the refusal is at or after the splice, and
/// never at a step before it.
#[test]
fn crash_windows_replay_exactly_and_a_crash_in_any_window_fences_that_incarnation() {
    // The logs are generated under a crash budget of one and run under three, so every
    // window has room for the spliced crash.
    let cfg = config(3, 3, true, 6);
    let generator = config(3, 1, true, 6);
    let mut rng = XorShift(0x5eed);
    let (mut windows, mut runs, mut fenced) = (0, 0, 0);
    for _ in 0..40 {
        let log = random_log(&mut rng, generator, 24);
        let full = run(cfg, &log).unwrap();
        for at in 0..=log.len() {
            let before = run(cfg, &log[..at]).unwrap();
            for node in 0..cfg.nodes() {
                let node = NodeId(node);
                for splice in [
                    vec![Step::Crash(node)],
                    vec![Step::Crash(node), Step::Restart(node)],
                ] {
                    windows += 1;
                    let spliced: Vec<Step> = log[..at]
                        .iter()
                        .chain(&splice)
                        .chain(&log[at..])
                        .copied()
                        .collect();
                    let journal = match run(cfg, &spliced) {
                        Ok(journal) => journal,
                        Err(refusal) => {
                            assert!(refusal.index >= at, "a splice broke an earlier step");
                            continue;
                        }
                    };
                    runs += 1;
                    assert_eq!(&journal.events()[..at], &full.events()[..at]);
                    assert_eq!(journal.replay().unwrap().encode(), journal.encode());
                    assert_eq!(run(cfg, &spliced).unwrap().encode(), journal.encode());
                    // The crashed incarnation's tickets: begun at `node` before the
                    // splice, in the epoch `node` had at the splice.
                    let mut state = Process::new(cfg);
                    for step in &log[..at] {
                        state.apply(step).unwrap();
                    }
                    let epoch = state.epoch(node).unwrap();
                    let doomed: BTreeSet<TicketId> = before
                        .events()
                        .iter()
                        .filter_map(|event| match event {
                            Event::Begun {
                                ticket,
                                node: n,
                                epoch: e,
                            } if *n == node && *e == epoch => Some(*ticket),
                            _ => None,
                        })
                        .collect();
                    for event in &journal.events()[at + splice.len()..] {
                        match event {
                            Event::Completed { ticket, .. } => {
                                assert!(!doomed.contains(ticket), "{ticket} crossed a crash");
                            }
                            Event::Fenced { ticket, .. } if doomed.contains(ticket) => {
                                fenced += 1;
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    // The seeds fix the logs, so the counts are pinned exactly: 6,000 windows, of
    // which 3,629 spliced logs are valid runs, with 1,551 completions of a crashed
    // incarnation fenced. A change means the evidence changed.
    assert_eq!((windows, runs, fenced), (6_000, 3_629, 1_551));
}

/// `pr15-impl02-pos-06`. A replicated-register-shaped log under the scenario's crash
/// budget: replica 0 crashes with a storage write in flight, replica 1 with a timer
/// armed. Both late completions are fenced (M08: a timer from an old epoch must not
/// fire in the new process), the restarted replica's own write completes, and a third
/// crash is outside the scenario's envelope.
#[test]
fn golden_a_register_shaped_log_journals_exactly_these_events() {
    let cfg = ProcessConfig::replicated_register_scenario(3, 8, RETAINED).unwrap();
    let log = vec![
        Step::Begin(NodeId(0)),
        Step::Begin(NodeId(1)),
        Step::Crash(NodeId(0)),
        Step::Complete(TicketId(0)),
        Step::Restart(NodeId(0)),
        Step::Begin(NodeId(0)),
        Step::Crash(NodeId(1)),
        Step::Restart(NodeId(1)),
        Step::Complete(TicketId(1)),
        Step::Complete(TicketId(2)),
    ];
    let journal = run(cfg, &log).unwrap();
    let (n0, n1) = (NodeId(0), NodeId(1));
    assert_eq!(
        journal.events(),
        &[
            Event::Begun {
                ticket: TicketId(0),
                node: n0,
                epoch: Epoch(0)
            },
            Event::Begun {
                ticket: TicketId(1),
                node: n1,
                epoch: Epoch(0)
            },
            Event::Crashed {
                node: n0,
                epoch: Epoch(0)
            },
            Event::Fenced {
                ticket: TicketId(0),
                node: n0,
                epoch: Epoch(0)
            },
            Event::Restarted {
                node: n0,
                epoch: Epoch(1)
            },
            Event::Begun {
                ticket: TicketId(2),
                node: n0,
                epoch: Epoch(1)
            },
            Event::Crashed {
                node: n1,
                epoch: Epoch(0)
            },
            Event::Restarted {
                node: n1,
                epoch: Epoch(1)
            },
            Event::Fenced {
                ticket: TicketId(1),
                node: n1,
                epoch: Epoch(0)
            },
            Event::Completed {
                ticket: TicketId(2),
                node: n0,
                epoch: Epoch(1)
            },
        ]
    );
    let mut process = Process::new(cfg);
    for step in &log {
        process.apply(step).unwrap();
    }
    assert_eq!(
        process.check(&Step::Crash(NodeId(2))),
        Err(Refusal::NotEnabled(NotEnabled::CrashBudgetSpent { max: 2 }))
    );
}

/// `pr15-impl02-pos-07`. Fault coverage, `crash`: fail-stop. The crashed incarnation
/// begins nothing more and cannot crash twice; its pending tickets are not undone —
/// they stay pending — and each completion that arrives for them is fenced, whether the
/// node is still down or has restarted. Other nodes are untouched.
#[test]
fn fault_crash_stops_an_incarnation_and_fences_its_pending_completions() {
    let cfg = config(2, 2, true, 8);
    let mut p = Process::new(cfg);
    for node in [0, 0, 1] {
        p.apply(&Step::Begin(NodeId(node))).unwrap();
    }
    assert_eq!(
        p.apply(&Step::Crash(NodeId(0))).unwrap(),
        &Event::Crashed {
            node: NodeId(0),
            epoch: Epoch(0)
        }
    );
    assert!(!p.is_up(NodeId(0)));
    assert!(p.is_up(NodeId(1)));
    assert_eq!(
        p.pending().collect::<Vec<_>>(),
        [TicketId(0), TicketId(1), TicketId(2)],
        "a crash undoes no pending operation"
    );
    assert_eq!(
        p.check(&Step::Begin(NodeId(0))),
        Err(Refusal::NotEnabled(NotEnabled::NodeDown(NodeId(0))))
    );
    assert_eq!(
        p.check(&Step::Crash(NodeId(0))),
        Err(Refusal::NotEnabled(NotEnabled::NodeDown(NodeId(0))))
    );
    // While down.
    assert!(matches!(
        p.apply(&Step::Complete(TicketId(0))).unwrap(),
        Event::Fenced { .. }
    ));
    // After a restart.
    p.apply(&Step::Restart(NodeId(0))).unwrap();
    assert!(matches!(
        p.apply(&Step::Complete(TicketId(1))).unwrap(),
        Event::Fenced { .. }
    ));
    // The other node's ticket is delivered.
    assert!(matches!(
        p.apply(&Step::Complete(TicketId(2))).unwrap(),
        Event::Completed { .. }
    ));
    // A fenced ticket is resolved: it cannot be delivered afterwards.
    assert_eq!(
        p.check(&Step::Complete(TicketId(0))),
        Err(Refusal::NotEnabled(NotEnabled::NotPending(TicketId(0))))
    );
}

/// `pr15-impl02-pos-08`. Fault coverage, `delay`: docs/17 §10's "delayed completion
/// from old epoch is rejected". `Delay` is the adversary's explicit, journalled stutter:
/// it holds a pending completion back one step and changes nothing else. Held for any
/// number of steps while its incarnation runs, the completion is still delivered. Held
/// across a crash and a restart, it is fenced. A delay is enabled only on a pending
/// ticket.
#[test]
fn fault_delay_a_completion_delayed_past_its_incarnation_is_fenced_never_delivered() {
    let cfg = config(1, 1, true, 4);
    let mut p = Process::new(cfg);
    p.apply(&Step::Begin(NodeId(0))).unwrap();
    p.apply(&Step::Begin(NodeId(0))).unwrap();
    let delayed = |ticket: u32| Event::Delayed {
        ticket: TicketId(ticket),
        node: NodeId(0),
        epoch: Epoch(0),
    };
    for _ in 0..500 {
        let before = (p.pending().collect::<Vec<_>>(), p.up(), p.epoch(NodeId(0)));
        assert_eq!(p.apply(&Step::Delay(TicketId(0))).unwrap(), &delayed(0));
        assert_eq!(
            (p.pending().collect::<Vec<_>>(), p.up(), p.epoch(NodeId(0))),
            before,
            "a delay changed the state"
        );
    }
    assert!(matches!(
        p.apply(&Step::Complete(TicketId(0))).unwrap(),
        Event::Completed { .. }
    ));
    assert_eq!(
        p.check(&Step::Delay(TicketId(0))),
        Err(Refusal::NotEnabled(NotEnabled::NotPending(TicketId(0))))
    );
    // The same delay across a crash and a restart: fenced.
    p.apply(&Step::Delay(TicketId(1))).unwrap();
    p.apply(&Step::Crash(NodeId(0))).unwrap();
    assert_eq!(p.apply(&Step::Delay(TicketId(1))).unwrap(), &delayed(1));
    p.apply(&Step::Restart(NodeId(0))).unwrap();
    for _ in 0..500 {
        assert_eq!(p.apply(&Step::Delay(TicketId(1))).unwrap(), &delayed(1));
    }
    assert_eq!(
        p.apply(&Step::Complete(TicketId(1))).unwrap(),
        &Event::Fenced {
            ticket: TicketId(1),
            node: NodeId(0),
            epoch: Epoch(0)
        }
    );
}

/// `pr15-impl02-pos-09`. M03: a restart never reuses an epoch. Over every node and
/// every crash the budget allows, each restart's epoch is one more than the last, and
/// no ticket of an old epoch completes into a new one.
#[test]
fn restart_epochs_strictly_increase_and_are_never_reused() {
    let cfg = config(3, 30, true, 64);
    let mut p = Process::new(cfg);
    let mut seen: BTreeSet<(u8, u32)> = (0..3).map(|n| (n, 0)).collect();
    for round in 0..10_u32 {
        for node in 0..3 {
            p.apply(&Step::Begin(NodeId(node))).unwrap();
            p.apply(&Step::Crash(NodeId(node))).unwrap();
            let event = *p.apply(&Step::Restart(NodeId(node))).unwrap();
            assert_eq!(
                event,
                Event::Restarted {
                    node: NodeId(node),
                    epoch: Epoch(round + 1)
                }
            );
            assert!(seen.insert((node, round + 1)), "epoch reused");
        }
    }
    let pending: Vec<TicketId> = p.pending().collect();
    assert_eq!(pending.len(), 30);
    for ticket in pending {
        assert!(matches!(
            p.apply(&Step::Complete(ticket)).unwrap(),
            Event::Fenced { .. }
        ));
    }
    assert_eq!(
        p.check(&Step::Crash(NodeId(0))),
        Err(Refusal::NotEnabled(NotEnabled::CrashBudgetSpent {
            max: 30
        }))
    );
}

// --- negative evidence ----------------------------------------------------------------

/// `pr15-impl02-neg-01`. Every step the profile does not model is refused as
/// `Unsupported` of exactly the row the step names, before any other check, that row is
/// declared unsupported, the INV-008 reading is `Unsupported`, and the state does not
/// change.
#[test]
fn negative_every_unsupported_step_is_a_typed_unsupported_refusal_and_changes_nothing() {
    let cfg = config(3, 2, true, 4);
    let mut p = Process::new(cfg);
    p.apply(&Step::Begin(NodeId(0))).unwrap();
    p.apply(&Step::Crash(NodeId(1))).unwrap();
    let mut probes = unsupported_steps();
    // Unsupported comes before every other check, even for a node that cannot exist or
    // is down.
    probes.extend([
        Step::CancelGracefully(NodeId(200)),
        Step::Panic(NodeId(1)),
        Step::PowerLoss(NodeSet(u64::MAX)),
    ]);
    for step in probes {
        let before = p.clone();
        let refusal = p.apply(&step).expect_err("unsupported");
        let Refusal::Unsupported(semantic) = refusal else {
            panic!("{step:?} gave {refusal:?}");
        };
        assert_eq!(Some(semantic), step.unsupported_semantic());
        assert_eq!(semantic.support(), Support::Unsupported, "{semantic}");
        assert_eq!(refusal.class(), RefusalClass::Unsupported);
        assert_eq!(refusal.inconclusive_reason(), Some("Unsupported"));
        assert!(refusal.to_string().contains(semantic.statement()));
        assert_eq!(p, before, "{step:?} changed the process");
    }
    let covered: BTreeSet<Semantic> = unsupported_steps()
        .iter()
        .filter_map(Step::unsupported_semantic)
        .collect();
    assert_eq!(covered.len(), 3);
    // The two unsupported rows with no step: nothing a caller can send asks for them.
    let stepless: BTreeSet<Semantic> = Semantic::ALL
        .into_iter()
        .filter(|s| s.support() == Support::Unsupported && !covered.contains(s))
        .collect();
    assert_eq!(
        stepless,
        [Semantic::PartialCleanup, Semantic::SupervisorDecisions]
            .into_iter()
            .collect()
    );
}

/// `pr15-impl02-neg-02`. Undeclared faults, the crash budget, the pending bound, node
/// state and malformed input each give their own typed refusal, and change nothing.
#[test]
fn negative_undeclared_faults_bounds_and_malformed_steps_are_typed() {
    let no_faults = config(3, 0, false, 2);
    let mut p = Process::new(no_faults);
    p.apply(&Step::Begin(NodeId(0))).unwrap();
    p.apply(&Step::Begin(NodeId(1))).unwrap();
    p.apply(&Step::Complete(TicketId(1))).unwrap();
    p.apply(&Step::Begin(NodeId(2))).unwrap();
    let cases = [
        (
            Step::Crash(NodeId(0)),
            Refusal::NotEnabled(NotEnabled::FaultNotDeclared(Semantic::FailStopCrash)),
        ),
        (
            Step::Restart(NodeId(0)),
            Refusal::NotEnabled(NotEnabled::FaultNotDeclared(Semantic::RestartNewEpoch)),
        ),
        (
            Step::Begin(NodeId(0)),
            Refusal::BoundReached(Bound::Pending { max: 2 }),
        ),
        (
            Step::Complete(TicketId(1)),
            Refusal::NotEnabled(NotEnabled::NotPending(TicketId(1))),
        ),
        (
            Step::Complete(TicketId(9)),
            Refusal::NotEnabled(NotEnabled::NotPending(TicketId(9))),
        ),
        (
            Step::Begin(NodeId(3)),
            Refusal::Malformed(Malformed::UnknownNode(NodeId(3))),
        ),
        (
            Step::Crash(NodeId(200)),
            Refusal::Malformed(Malformed::UnknownNode(NodeId(200))),
        ),
        (
            Step::Restart(NodeId(64)),
            Refusal::Malformed(Malformed::UnknownNode(NodeId(64))),
        ),
    ];
    for (step, want) in cases {
        let before = p.clone();
        assert_eq!(p.apply(&step), Err(want), "{step:?}");
        assert_eq!(p, before);
    }
    let crashes = config(2, 1, true, 4);
    let mut p = Process::new(crashes);
    assert_eq!(
        p.check(&Step::Restart(NodeId(0))),
        Err(Refusal::NotEnabled(NotEnabled::NodeUp(NodeId(0))))
    );
    p.apply(&Step::Crash(NodeId(0))).unwrap();
    // The budget is checked before the node state.
    assert_eq!(
        p.check(&Step::Crash(NodeId(0))),
        Err(Refusal::NotEnabled(NotEnabled::CrashBudgetSpent { max: 1 }))
    );
    assert_eq!(
        p.check(&Step::Crash(NodeId(1))),
        Err(Refusal::NotEnabled(NotEnabled::CrashBudgetSpent { max: 1 }))
    );
    assert_eq!(
        Refusal::BoundReached(Bound::Pending { max: 2 }).inconclusive_reason(),
        Some("ResourceExhausted")
    );
    assert_eq!(
        Refusal::NotEnabled(NotEnabled::NodeDown(NodeId(0))).inconclusive_reason(),
        None
    );
    assert_eq!(
        Refusal::Malformed(Malformed::UnknownNode(NodeId(9))).class(),
        RefusalClass::Malformed
    );
    // A refused step inside a log names its index, and nothing after it runs.
    assert_eq!(
        run(
            crashes,
            &[
                Step::Crash(NodeId(0)),
                Step::Begin(NodeId(0)),
                Step::Begin(NodeId(1))
            ]
        )
        .unwrap_err()
        .index,
        1
    );
}

/// `pr15-impl02-neg-03`. Anti-vacuity: changing one step of a valid log changes the
/// journal, or makes the log invalid, and never leaves the journal as it was.
#[test]
fn negative_a_perturbed_choice_log_changes_the_journal() {
    let cfg = config(4, 4, true, 8);
    let mut rng = XorShift(7);
    let mut changed = 0;
    for _ in 0..200 {
        let log = random_log(&mut rng, cfg, 20);
        let journal = run(cfg, &log).unwrap();
        let at = rng.below(log.len());
        let mut perturbed = log.clone();
        perturbed[at] = match log[at] {
            Step::Begin(NodeId(n)) => Step::Begin(NodeId((n + 1) % 4)),
            Step::Complete(TicketId(t)) => Step::Complete(TicketId(t + 1)),
            Step::Delay(t) => Step::Complete(t),
            Step::Crash(n) => Step::Restart(n),
            Step::Restart(n) => Step::Crash(n),
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
    assert!(changed > 50, "only {changed} perturbations ran");
}

/// `pr15-impl02-bnd-01`. The choice-log length is charged before any step runs.
#[test]
fn boundary_an_overlong_log_is_refused_before_any_step_runs() {
    let cfg = config(2, 0, false, 1);
    // Every step would be refused as `NotPending` at index 0 if it ran.
    let log = vec![Step::Complete(TicketId(0)); MAX_STEPS + 1];
    let refusal = run(cfg, &log).unwrap_err();
    assert_eq!(refusal.index, MAX_STEPS + 1);
    assert_eq!(
        refusal.refusal,
        Refusal::BoundReached(Bound::Steps { max: MAX_STEPS })
    );
}

/// `pr15-impl02-bnd-02`. Configuration caps.
#[test]
fn boundary_configuration_caps_are_typed_refusals() {
    assert_eq!(
        ProcessConfig::new(0, 1, true, 1, RETAINED),
        Err(ConfigRefusal::NodesOutOfRange(0))
    );
    assert_eq!(
        ProcessConfig::new(65, 1, true, 1, RETAINED),
        Err(ConfigRefusal::NodesOutOfRange(65))
    );
    assert_eq!(
        ProcessConfig::new(2, MAX_CRASHES_CAP + 1, true, 1, RETAINED),
        Err(ConfigRefusal::CrashesOutOfRange(MAX_CRASHES_CAP + 1))
    );
    assert_eq!(
        ProcessConfig::new(2, 1, true, 0, RETAINED),
        Err(ConfigRefusal::PendingOutOfRange(0))
    );
    assert_eq!(
        ProcessConfig::new(2, 1, true, MAX_PENDING_CAP + 1, RETAINED),
        Err(ConfigRefusal::PendingOutOfRange(MAX_PENDING_CAP + 1))
    );
    // 64 nodes is the largest set a `NodeSet` holds, and node 63 works.
    let wide = ProcessConfig::new(64, MAX_CRASHES_CAP, true, 1, RETAINED).unwrap();
    let mut p = Process::new(wide);
    assert_eq!(p.up(), NodeSet(u64::MAX));
    p.apply(&Step::Crash(NodeId(63))).unwrap();
    assert_eq!(p.up(), NodeSet(u64::MAX >> 1));
    assert_eq!(
        p.apply(&Step::Restart(NodeId(63))).unwrap(),
        &Event::Restarted {
            node: NodeId(63),
            epoch: Epoch(1)
        }
    );
    assert_eq!(p.epoch(NodeId(64)), None);
    assert!(!p.is_up(NodeId(64)));
}

/// `pr15-impl02-bnd-03`. A process driven step by step, without `run`, stops at the
/// same `MAX_STEPS` bound, so every journal it produces replays.
#[test]
fn boundary_a_directly_driven_process_stops_at_max_steps_and_its_journal_replays() {
    let cfg = config(1, 0, false, 1);
    let mut p = Process::new(cfg);
    let mut ticket = 0;
    for step in 0..MAX_STEPS {
        if step % 2 == 0 {
            p.apply(&Step::Begin(NodeId(0))).unwrap();
        } else {
            p.apply(&Step::Complete(TicketId(ticket))).unwrap();
            ticket += 1;
        }
    }
    assert_eq!(
        p.apply(&Step::Begin(NodeId(0))),
        Err(Refusal::BoundReached(Bound::Steps { max: MAX_STEPS }))
    );
    let journal = p.into_journal();
    assert_eq!(journal.events().len(), MAX_STEPS);
    assert_eq!(journal.replay().unwrap(), journal);
}

/// `pr15-impl02-bnd-04`. The retained-bytes budget bounds the whole journal, is
/// charged before anything is pushed, and holds for clone, replay and encode.
#[test]
fn boundary_the_retained_bytes_budget_is_charged_before_every_push() {
    let one_begin = JOURNAL_HEADER_BYTES + 10;
    // Exact boundary: one `Begin` fits exactly.
    let exact = ProcessConfig::new(1, 1, true, 4, one_begin).unwrap();
    let mut p = Process::new(exact);
    p.apply(&Step::Begin(NodeId(0))).unwrap();
    assert_eq!(p.retained_bytes(), one_begin);
    assert_eq!(p.encode().len() as u64, one_begin);
    for step in [Step::Complete(TicketId(0)), Step::Crash(NodeId(0))] {
        let before = p.clone();
        assert_eq!(
            p.apply(&step),
            Err(Refusal::BoundReached(Bound::Retained { max: one_begin }))
        );
        assert_eq!(p, before, "a refused charge changed the process");
    }
    // One byte short: the same step is refused before anything is retained.
    let short = ProcessConfig::new(1, 1, true, 4, one_begin - 1).unwrap();
    let mut p = Process::new(short);
    assert_eq!(
        p.apply(&Step::Begin(NodeId(0))),
        Err(Refusal::BoundReached(Bound::Retained {
            max: one_begin - 1
        }))
    );
    assert_eq!(p.retained_bytes(), JOURNAL_HEADER_BYTES);
    assert!(p.events().is_empty());
    assert_eq!(p.ticket_count(), 0);
    // A crash (6 bytes) fits where a begin (10 bytes) does not.
    p.apply(&Step::Crash(NodeId(0))).unwrap();
    // Begin and complete at pending 1: the pending bound never binds, the budget does,
    // after exactly as many rounds as it pays for.
    let rounds = 10_u64;
    let budget = JOURNAL_HEADER_BYTES + rounds * 20;
    let cfg = ProcessConfig::new(1, 0, false, 1, budget).unwrap();
    let mut p = Process::new(cfg);
    let mut done = 0;
    loop {
        let t = TicketId(u32::try_from(p.ticket_count()).unwrap());
        if let Err(refusal) = p.apply(&Step::Begin(NodeId(0))) {
            assert_eq!(
                refusal,
                Refusal::BoundReached(Bound::Retained { max: budget })
            );
            break;
        }
        p.apply(&Step::Complete(t)).unwrap();
        done += 1;
    }
    assert_eq!(done, rounds);
    assert_eq!(p.retained_bytes(), budget);
    let journal = p.into_journal();
    let copy = journal.clone();
    assert_eq!(copy.retained_bytes(), budget);
    let replayed = journal.replay().unwrap();
    assert_eq!(replayed, journal);
    assert_eq!(replayed.encode().len() as u64, budget);
    assert_eq!(journal.encode(), replayed.encode());
    // Configuration bounds on the budget itself.
    assert_eq!(
        ProcessConfig::new(1, 0, false, 1, JOURNAL_HEADER_BYTES - 1),
        Err(ConfigRefusal::RetainedOutOfRange(JOURNAL_HEADER_BYTES - 1))
    );
    assert_eq!(
        ProcessConfig::new(1, 0, false, 1, MAX_RETAINED_CAP + 1),
        Err(ConfigRefusal::RetainedOutOfRange(MAX_RETAINED_CAP + 1))
    );
    // The header alone is a valid, empty journal.
    let bare = ProcessConfig::new(1, 0, false, 1, JOURNAL_HEADER_BYTES).unwrap();
    assert_eq!(
        Process::new(bare).encode().len() as u64,
        JOURNAL_HEADER_BYTES
    );
}

// --- honesty of the profile -----------------------------------------------------------

const RFC_0002: &str =
    include_str!("../../../notes/plan/rfcs/0002-controlled-effects-and-domain-packs.md");
const DOCS_17: &str = include_str!("../../../notes/plan/docs/17_DOMAIN_PACK_CONTRACT.md");
const SCENARIO: &str =
    include_str!("../../../notes/plan/examples/replicated_register.scenario.toml");
const CONTRACT: &str =
    include_str!("../../continuum-intent/tests/fixtures/replicated-register-contract.json");
const NETWORK_PROFILE: &str = include_str!("../../continuum-effects-network/src/profile.rs");

/// The text between a heading and the next heading of any level.
fn section<'t>(text: &'t str, heading: &str) -> &'t str {
    let start = text.find(heading).expect("heading present") + heading.len();
    let rest = &text[start..];
    let end = rest.find("\n#").unwrap_or(rest.len());
    &rest[..end]
}

fn bullets(text: &str) -> Vec<&str> {
    text.lines()
        .filter_map(|line| line.strip_prefix("- "))
        .map(|line| line.trim_end_matches([';', '.']))
        .collect()
}

/// `pr15-impl02-hon-01`. Every process semantic RFC 0002 "Process lifecycle" names, and
/// every process constraint of docs/17 §10, maps to declared rows; every row is declared
/// once with a statement. A new bullet in either list fails this test until the profile
/// states it.
#[test]
fn honesty_the_profile_covers_every_rfc_0002_and_docs_17_process_semantic() {
    use Semantic as S;
    let rfc: &[(&str, &[Semantic])] = &[
        ("graceful cancellation", &[S::GracefulCancellation]),
        ("panic", &[S::Panic]),
        ("fail-stop crash", &[S::FailStopCrash]),
        ("power loss", &[S::PowerLoss]),
        ("restart with a new epoch", &[S::RestartNewEpoch]),
        ("partial cleanup", &[S::PartialCleanup]),
        ("supervisor decisions", &[S::SupervisorDecisions]),
        ("leaked external effects", &[S::LeakedExternalEffects]),
    ];
    assert_eq!(
        bullets(section(RFC_0002, "### Process lifecycle")),
        rfc.iter().map(|(bullet, _)| *bullet).collect::<Vec<_>>(),
        "RFC 0002's process-lifecycle list changed; state the new semantics in the profile"
    );
    // docs/17 §10's constraints: the process ones map to rows; the partition and clock
    // ones are the network and time packs'.
    let docs17: &[(&str, &[Semantic])] = &[
        ("crash clears volatile state", &[S::VolatileStateLoss]),
        ("recovery increments epoch", &[S::RestartNewEpoch]),
        (
            "delayed completion from old epoch is rejected or explicitly modeled",
            &[S::LateCompletion],
        ),
        ("partition affects matching routes", &[]),
        (
            "clock jump changes wall mapping, not monotonic logical time",
            &[],
        ),
        (
            "cancellation is not process crash",
            &[S::GracefulCancellation, S::FailStopCrash],
        ),
    ];
    assert_eq!(
        bullets(section(DOCS_17, "## 10. Fault algebra")),
        docs17.iter().map(|(bullet, _)| *bullet).collect::<Vec<_>>(),
        "docs/17 §10's constraint list changed"
    );
    let mut mapped: BTreeSet<Semantic> = BTreeSet::new();
    for (_, rows) in rfc.iter().chain(docs17) {
        mapped.extend(rows.iter().copied());
    }
    mapped.insert(S::ExplorationBounds);
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
            S::FailStopCrash,
            S::VolatileStateLoss,
            S::RestartNewEpoch,
            S::LateCompletion,
            S::LeakedExternalEffects,
            S::ExplorationBounds,
        ]
    );
    assert_eq!(
        ASSUMPTIONS.map(|(id, _)| id),
        ["fail-stop", "no-eventual-restart", "no-failure-detection"]
    );
    assert_eq!(CANCELLATION_CONTRACT.len(), 8);
}

/// `pr15-impl02-hon-02`. The scenario helper's crash budget is the scenario's own
/// `max_crashes`, and the restart it declares answers the replicated-register Intent
/// Contract, which enables both `crash` and `recovery`.
#[test]
fn honesty_the_scenario_helper_matches_the_replicated_register_scenario_and_contract() {
    let faults = section(SCENARIO, "[faults]");
    let max_crashes = faults
        .lines()
        .find_map(|line| line.strip_prefix("max_crashes = "))
        .expect("max_crashes");
    let cfg = ProcessConfig::replicated_register_scenario(3, 8, RETAINED).unwrap();
    assert_eq!(max_crashes, cfg.max_crashes().to_string());
    assert!(cfg.restart());
    let enabled = &CONTRACT[CONTRACT.find("\"enabled\"").expect("fault_model.enabled")..];
    let enabled = &enabled[..enabled.find(']').expect("the enabled array closes")];
    for class in ["\"crash\"", "\"recovery\""] {
        assert!(
            enabled.contains(class),
            "the contract no longer enables {class}"
        );
    }
}

/// `pr15-impl02-hon-03`. T06: the declared profile claims no host semantics. This is
/// the test GOV-4-13's not-applicable form names as the source of truth for the value
/// (`tools/governance/reviews/bn-3mmf.toml`). The absence of host effects themselves is
/// the compiler's to prove: the crate is `#![no_std]`, and `tests/pr15_no_std_lane.rs`
/// shows that proof is live.
#[test]
fn profile_declares_host_qualification_none() {
    assert_eq!(CRASH_RESTART_V0.host, HostQualification::None);
    assert_eq!(CRASH_RESTART_V0.class, FidelityClass::AdversarialEnvelope);
    assert_ne!(CRASH_RESTART_V0.class, FidelityClass::PlatformQualified);
    assert_eq!(
        CRASH_RESTART_V0.independence,
        IndependenceClaim::AllDependent
    );
}

/// `pr15-impl02-hon-04`. The profile's canonical bytes are pinned together with its
/// version (docs/17 §12). Editing a row, a statement, an assumption, the composition
/// table or the cancellation contract changes the fingerprint and fails this test. The
/// rule is to bump `PROFILE_VERSION` and re-pin both; this test catches an unnoticed
/// edit, but a deliberate re-pin without a bump is caught only by review.
#[test]
fn honesty_the_profile_bytes_are_pinned_to_its_version() {
    let bytes = CRASH_RESTART_V0.canonical_bytes();
    let fingerprint = bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    });
    assert_eq!(
        (PROFILE_VERSION.to_string(), fingerprint),
        ("0.1.0".to_owned(), PINNED_FINGERPRINT),
        "process/crash-restart-v0 changed: bump PROFILE_VERSION, then re-pin"
    );
    let journal: Journal = run(config(1, 0, false, 1), &[Step::Begin(NodeId(0))]).unwrap();
    let name = PROFILE_NAME.as_bytes();
    assert!(journal.encode().windows(name.len()).any(|w| w == name));
}

const PINNED_FINGERPRINT: u64 = 18_224_978_510_953_362_894;

/// `pr15-impl02-hon-05`. The journal's wire form is pinned byte for byte, so a change
/// to tags, field order, widths or endianness fails here rather than passing every
/// self-consistency test.
#[test]
fn honesty_the_journal_encoding_is_pinned_byte_for_byte() {
    let cfg = config(2, 2, true, 4);
    let log = [
        Step::Begin(NodeId(0)),
        Step::Begin(NodeId(1)),
        Step::Complete(TicketId(1)),
        Step::Delay(TicketId(0)),
        Step::Crash(NodeId(0)),
        Step::Restart(NodeId(0)),
        Step::Complete(TicketId(0)),
    ];
    let hex: String = run(cfg, &log)
        .unwrap()
        .encode()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(hex, PINNED_JOURNAL_HEX, "the journal wire form changed");
}

const PINNED_JOURNAL_HEX: &str = "636f6e74696e75756d2d70726f636573732d6a6f75726e616c000000001870726f636573732f63726173682d726573746172742d7630000000010000020000000201000000040000000010000000000000070100000000000000000001000000010100000000020000000101000000000600000000000000000004000000000005000000000103000000000000000000";

/// `pr15-impl02-hon-06`. The coordination with the network pack, read from its source
/// rather than linked (a dev-dependency would be a new dependency edge): its
/// `endpoint-crash` row defers what a crash does to in-flight envelopes to this pack,
/// its `connection-epochs` row says incarnation epochs are this pack's, and this
/// profile's composition table answers for the network profile by its exact name.
#[test]
fn honesty_the_network_pack_defers_crash_semantics_here_and_this_profile_states_them() {
    let flat: String = NETWORK_PROFILE
        .split_whitespace()
        .map(|word| word.trim_matches('\\'))
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    for promise in [
        "what a crash does to envelopes in flight to or from a node is the process pack's to state",
        "the process pack owns incarnation epochs",
    ] {
        assert!(
            flat.contains(promise),
            "the network profile no longer says: {promise}"
        );
    }
    assert!(NETWORK_PROFILE.contains("pub const PROFILE_NAME: &str = \"network/adversarial-v0\";"));
    let (pack, statement) = COMPOSITION[0];
    assert_eq!(pack, "network/adversarial-v0");
    for claim in [
        "stay in flight",
        "a later incarnation",
        "epoch in the payload",
    ] {
        assert!(statement.contains(claim), "{claim}");
    }
    assert_eq!(
        COMPOSITION.map(|(pack, _)| pack),
        ["network/adversarial-v0", "storage", "runtime"]
    );
}

/// `pr15-impl02-neg-05` (cr-35ujnx, found on the storage pack and audited here). The
/// exploration prunes by [`Model::key`], so the key must be the whole input of a step
/// and of the candidate generator. The journal length and the retained bytes are in it,
/// because the step bound and the budget are charged against them. Over the reachable
/// space of every configuration, two histories with one key have the same candidates,
/// and each candidate has the same outcome, event and successor key from both. A step
/// that read anything the key omits would break this.
#[test]
fn differential_key_is_the_whole_input_of_a_step() {
    type Outcomes = Vec<(Step, Result<Event, Refusal>, Option<ModelKey>)>;
    let outcomes = |model: &Model| -> Outcomes {
        candidates(model)
            .into_iter()
            .map(|step| {
                let mut child = model.clone();
                let result = child.step(&step);
                let key = result.is_ok().then(|| child.key());
                (step, result, key)
            })
            .collect()
    };
    for cfg in differential_configs() {
        let mut seen: std::collections::BTreeMap<ModelKey, Outcomes> =
            std::collections::BTreeMap::new();
        let mut collisions = 0;
        let mut stack = vec![Model::new(cfg, Mutant::None)];
        while let Some(model) = stack.pop() {
            let key = model.key();
            let next = outcomes(&model);
            if let Some(before) = seen.get(&key) {
                assert_eq!(before, &next, "a key collision hides a different successor");
                collisions += 1;
                continue;
            }
            for (step, result, _) in &next {
                if result.is_ok() {
                    let mut child = model.clone();
                    child.step(step).unwrap();
                    stack.push(child);
                }
            }
            seen.insert(key, next);
        }
        assert!(
            collisions > 0,
            "no two histories met, so the check checked nothing"
        );
    }
}
