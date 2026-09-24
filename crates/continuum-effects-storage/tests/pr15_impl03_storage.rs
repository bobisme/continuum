//! PR-15 / IMPL-03 (bn-2fk3): the `storage/append-log-v0` profile and its Lab handler.
//!
//! Evidence, by artifact id:
//!
//! | Id | Test | What it shows |
//! |---|---|---|
//! | `pr15-impl03-pos-01` | [`differential_the_lab_handler_equals_an_independent_reference_model_exhaustively`] | over the complete reachable space of six configurations, each with a small step bound that binds, one also under a binding retained-bytes budget, the Lab handler and an independently written reference model accept and refuse the same steps with the same refusals, emit the same events, reach the same states, and list the same enabled choices |
//! | `pr15-impl03-neg-04` | [`differential_is_not_vacuous_every_seeded_model_bug_is_caught`] | twenty-one seeded reference-model bugs, ack-before-sync, lose-synced and three torn-tail variants among them, each caught by the exhaustive differential |
//! | `pr15-impl03-neg-05` | [`differential_key_separates_paths_whose_candidates_differ`] | cr-35ujnx: the pruning key is the model's whole `State`, everything its step and candidate generator read, the step count and retained bytes included; two histories with one key have the same outcomes for every candidate |
//! | `pr15-impl03-neg-06` | [`differential_view_covers_every_field_of_the_lab_handler`] | cr-35ujnx: every field of the Lab handler is in the compared view, so its successors are a function of the key too |
//! | `pr15-impl03-pos-02` | [`determinism_identical_choice_logs_give_byte_identical_journals`] | 2,100 seeded random logs: two runs of one log give identical bytes, and distinct logs give many distinct journals |
//! | `pr15-impl03-pos-03` | [`replay_a_journal_is_its_own_choice_log`] | a journal's choice log replays to the same journal, byte for byte |
//! | `pr15-impl03-pos-04` | [`every_step_boundary_truncation_replays_as_the_journal_prefix`] | cutting a log at any step boundary gives exactly the journal prefix |
//! | `pr15-impl03-pos-05` | [`crash_windows_replay_exactly_keep_every_stable_record_and_fence_that_incarnation`] | a crash with each outcome, alone or with restart and recovery, spliced into every step boundary of seeded logs: the run keeps the journal prefix, replays byte for byte, keeps every stable record intact, fences the crashed incarnation's tickets, and every `Acked` range stays in the log unchanged to the end |
//! | `pr15-impl03-pos-06` | [`golden_ack_before_sync_loses_the_record_and_the_fenced_ack_vouches_for_nothing`] | the M01 shape: a record submitted and synced but not yet stable is lost at a crash, its ack is refused before `Stable` and fenced after the crash; the correct order keeps the record |
//! | `pr15-impl03-pos-07` | [`fault_crash_keeps_stable_records_and_loses_or_tears_the_volatile_suffix`] | the `crash` fault row: every crash outcome of one log state |
//! | `pr15-impl03-pos-08` | [`fault_delay_an_ack_delayed_past_its_incarnation_is_fenced_never_delivered`] | a completion delayed with no bound is delivered while its incarnation runs, and fenced once it crashed |
//! | `pr15-impl03-pos-09` | [`recovery_a_torn_tail_blocks_writes_until_truncated_and_survives_crashes`] | a torn tail reads as `Torn`, a `Submit` or `Sync` over it is a `ProgramFault` (M07), it survives a second crash, and `Truncate` drops it |
//! | `pr15-impl03-neg-01` | [`negative_every_unsupported_step_is_a_typed_unsupported_refusal_and_changes_nothing`] | every unsupported step is `Unsupported(<its row>)`, INV-008 `Unsupported`, and atomic |
//! | `pr15-impl03-neg-02` | [`negative_undeclared_faults_bounds_and_malformed_steps_are_typed`] | undeclared faults, the crash budget, the pending bound, node and ticket state, impossible crash outcomes, unknown nodes |
//! | `pr15-impl03-neg-03` | [`negative_a_perturbed_choice_log_changes_the_journal`] | anti-vacuity: one changed step changes the journal |
//! | `pr15-impl03-bnd-01` | [`boundary_an_overlong_log_is_refused_before_any_step_runs`] | the `MAX_STEPS` charge comes before any work |
//! | `pr15-impl03-bnd-02` | [`boundary_configuration_caps_are_typed_refusals`] | configuration caps |
//! | `pr15-impl03-bnd-03` | [`boundary_a_directly_driven_storage_stops_at_max_steps_and_its_journal_replays`] | `Storage::apply` has the same step bound, so its journals replay |
//! | `pr15-impl03-bnd-04` | [`boundary_the_retained_bytes_budget_is_charged_before_every_push`] | exact boundary, one byte over, submit and sync at pending 1, clone, replay and encode within the budget |
//! | `pr15-impl03-hon-01` | [`honesty_the_profile_covers_every_rfc_0002_and_docs_17_storage_semantic`] | every RFC 0002 storage bullet, every docs/17 §6 claim, fault, exclusion and assumption, and every docs/17 §10 constraint maps to declared rows |
//! | `pr15-impl03-hon-02` | [`honesty_the_profile_is_the_one_the_scenario_contract_and_manifest_cite`] | the scenario, the Intent Contract and the schema example name this profile, with its class, faults and assumption |
//! | `pr15-impl03-hon-03` | [`profile_declares_host_qualification_none`] | T06: the profile claims no host semantics (the compiler lane proves no host effect) |
//! | `pr15-impl03-hon-04` | [`honesty_the_profile_bytes_are_pinned_to_its_version`] | a profile edit without a version bump fails |
//! | `pr15-impl03-hon-05` | [`honesty_the_journal_encoding_is_pinned_byte_for_byte`] | the journal wire form is pinned |
//! | `pr15-impl03-hon-06` | [`honesty_the_process_pack_defers_storage_here_and_this_profile_composes_with_its_crash_and_fence`] | the process profile's `storage` row defers here, its epoch and fence rules are the ones this pack restates, and this profile's composition table answers for `process/crash-restart-v0` |

use std::collections::BTreeSet;

use continuum_effects_storage::lab::JOURNAL_HEADER_BYTES;
use continuum_effects_storage::profile::{
    ASSUMPTIONS, CANCELLATION_CONTRACT, COMPOSITION, HostQualification, IndependenceClaim,
    PROFILE_NAME, PROFILE_NAME_V0, PROFILE_VERSION,
};
use continuum_effects_storage::refusal::{
    Bound, ConfigRefusal, Malformed, NotEnabled, ProgramFault,
};
use continuum_effects_storage::step::{
    MAX_CRASHES_CAP, MAX_PENDING_CAP, MAX_RETAINED_CAP, MAX_STEPS,
};
use continuum_effects_storage::{
    APPEND_LOG_V0, APPEND_LOG_V1, Entry, Epoch, Event, FidelityClass, Journal, NodeId, NodeSet,
    PROFILES, Refusal, RefusalClass, Semantic, Step, Storage, StorageConfig, Support, TicketId,
    Value, run,
};

/// The retained-bytes budget of every configuration that is not testing the budget.
const RETAINED: u64 = MAX_RETAINED_CAP;

// --- an independent reference model ------------------------------------------------------

/// A ticket's state in the reference model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Status {
    Pending,
    Acked,
    Fenced,
}

/// The reference model, written from the profile rows and the module's refusal
/// precedence with different data structures: nodes are a vector of `(up, epoch)`
/// pairs, a log is a vector of `Option<u32>` (`None` is torn), tickets are
/// `(node, epoch, upto, status, flushed)` tuples, and every count is a scan.
///
/// The configuration and the seeded mutant are fixed for one exploration. Everything
/// else a step reads is [`State`], and the transition is a method of `State` that
/// sees nothing but the state, the configuration, the mutant and the step. The events
/// are output only: no transition, refusal or candidate reads them.
#[derive(Debug, Clone)]
struct Model {
    mutant: Mutant,
    config: StorageConfig,
    state: State,
    events: Vec<Event>,
}

/// Everything a model step and the candidate generator read (cr-35ujnx). It is the
/// exploration's pruning key whole, so a counter a transition reads cannot be left
/// out of the key: it would have to be a field here first.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct State {
    nodes: Vec<(bool, u32)>,
    logs: Vec<Vec<Option<u32>>>,
    stable: Vec<u32>,
    tickets: Vec<(u8, u32, u32, Status, bool)>,
    crashes: u32,
    /// `Submit`s taken: the next value and the remaining program budget.
    submitted: usize,
    /// Steps accepted: the journal length the step bound is charged against.
    steps: usize,
    /// The journal's canonical size so far, header included, counted independently:
    /// the retained-bytes budget is charged against it.
    retained: u64,
}

/// Seeded bugs in the reference model. Each is a plausible Lab-handler defect; the
/// differential must see every one of them, or it is not checking anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mutant {
    None,
    /// M01 at the pack: `Ack` is delivered before the flush finished.
    AckBeforeSync,
    /// A crash that loses volatile records also loses the last stable one.
    LoseSynced,
    /// Records a crash kept stay volatile, so a later crash can lose them.
    KeptNotStable,
    /// Torn tail: a crash may tear the last record even when it loses nothing, so a
    /// stable record is torn.
    TornStable,
    /// Torn tail: a torn record reads back as its intact value.
    TornUndetected,
    /// Torn tail (M07 at the pack): a write lands after a torn tail.
    SubmitOverTorn,
    /// A flush finishes after its incarnation crashed.
    PersistAfterCrash,
    /// A completion is judged by liveness only, so an old incarnation's ack reaches the
    /// new one.
    FenceIgnoresEpoch,
    /// A completion is judged by the epoch only, so one arriving while down is
    /// delivered.
    FenceIgnoresDown,
    /// M03: restart keeps the old epoch.
    RestartReusesEpoch,
    /// The sync barrier is taken at `Persist`, not at `Sync`.
    BarrierAtPersist,
    /// `Persist` sets the stable length to its own range, lowering it.
    StableNotMonotone,
    CrashIgnoresBudget,
    SuffixLossUndeclared,
    TornUndeclared,
    BudgetUncharged,
    /// A resolved ticket stays pending, so it can be acked twice.
    AckTwice,
    /// `Delay` resolves the ticket instead of stuttering.
    DelayResolves,
    /// `Truncate` drops an intact record when there is no torn tail.
    TruncateAnyTail,
    /// The node-state check comes before the crash budget.
    PrecedenceSwap,
    /// `Persist` checks the incarnation before the finished flush.
    PersistPrecedenceSwap,
    /// The step bound admits one step too many.
    StepBoundOffByOne,
}

const MUTANTS: [Mutant; 22] = [
    Mutant::AckBeforeSync,
    Mutant::LoseSynced,
    Mutant::KeptNotStable,
    Mutant::TornStable,
    Mutant::TornUndetected,
    Mutant::SubmitOverTorn,
    Mutant::PersistAfterCrash,
    Mutant::FenceIgnoresEpoch,
    Mutant::FenceIgnoresDown,
    Mutant::RestartReusesEpoch,
    Mutant::BarrierAtPersist,
    Mutant::StableNotMonotone,
    Mutant::CrashIgnoresBudget,
    Mutant::SuffixLossUndeclared,
    Mutant::TornUndeclared,
    Mutant::BudgetUncharged,
    Mutant::AckTwice,
    Mutant::DelayResolves,
    Mutant::TruncateAnyTail,
    Mutant::PrecedenceSwap,
    Mutant::PersistPrecedenceSwap,
    Mutant::StepBoundOffByOne,
];

type ModelKey = State;

impl Model {
    fn new(config: StorageConfig, mutant: Mutant) -> Self {
        let n = usize::from(config.nodes());
        Self {
            mutant,
            config,
            state: State {
                nodes: vec![(true, 0); n],
                logs: vec![Vec::new(); n],
                stable: vec![0; n],
                tickets: Vec::new(),
                crashes: 0,
                submitted: 0,
                steps: 0,
                retained: JOURNAL_HEADER_BYTES,
            },
            events: Vec::new(),
        }
    }

    /// The pruning key of the exploration: the whole [`State`].
    fn key(&self) -> ModelKey {
        self.state.clone()
    }

    /// One step: the step bound, the transition on a scratch copy of the state, then
    /// the retained-bytes charge, then commit.
    fn step(&mut self, step: &Step) -> Result<Event, Refusal> {
        let max_steps = self.config.max_steps() as usize
            + usize::from(self.mutant == Mutant::StepBoundOffByOne);
        if self.state.steps >= max_steps {
            return Err(Refusal::BoundReached(Bound::Steps {
                max: self.config.max_steps() as usize,
            }));
        }
        let mut next = self.state.clone();
        let event = next.transition(&self.config, self.mutant, step)?;
        next.steps += 1;
        next.retained += match event {
            Event::Crashed { .. } => 15,
            Event::Restarted { .. } => 6,
            Event::Truncated { .. } => 10,
            _ => 14,
        };
        if matches!(event, Event::Submitted { .. }) {
            next.submitted += 1;
        }
        let max = self.config.max_retained_bytes();
        if next.retained > max && self.mutant != Mutant::BudgetUncharged {
            return Err(Refusal::BoundReached(Bound::Retained { max }));
        }
        self.state = next;
        self.events.push(event);
        Ok(event)
    }
}

impl State {
    fn node(&self, node: NodeId) -> Result<usize, Refusal> {
        if usize::from(node.0) < self.nodes.len() {
            Ok(usize::from(node.0))
        } else {
            Err(Refusal::Malformed(Malformed::UnknownNode(node)))
        }
    }

    fn torn_tail(&self, n: usize) -> bool {
        matches!(self.logs[n].last(), Some(None))
    }

    fn pending_ticket(&self, ticket: TicketId) -> Result<(usize, u8, u32, u32, bool), Refusal> {
        let at = ticket.0 as usize;
        match self.tickets.get(at) {
            Some(&(node, epoch, upto, Status::Pending, flushed)) => {
                Ok((at, node, epoch, upto, flushed))
            }
            _ => Err(Refusal::NotEnabled(NotEnabled::NotPending(ticket))),
        }
    }

    fn live(&self, m: Mutant, node: u8, epoch: u32) -> bool {
        let (up, current) = self.nodes[usize::from(node)];
        match m {
            Mutant::FenceIgnoresEpoch => up,
            Mutant::FenceIgnoresDown => current == epoch,
            _ => up && current == epoch,
        }
    }

    #[allow(clippy::too_many_lines)]
    fn transition(
        &mut self,
        cfg: &StorageConfig,
        m: Mutant,
        step: &Step,
    ) -> Result<Event, Refusal> {
        let event = match *step {
            Step::Corrupt { .. } => return Err(Refusal::Unsupported(Semantic::SectorCorruption)),
            Step::Reorder(_) => return Err(Refusal::Unsupported(Semantic::WriteReordering)),
            Step::FalseFlush(_) => return Err(Refusal::Unsupported(Semantic::FlushDishonesty)),
            Step::Submit { node, value } => {
                let n = self.node(node)?;
                let (up, epoch) = self.nodes[n];
                if !up {
                    return Err(Refusal::NotEnabled(NotEnabled::NodeDown(node)));
                }
                if self.torn_tail(n) && m != Mutant::SubmitOverTorn {
                    return Err(Refusal::ProgramFault(ProgramFault::WriteOverTornTail(node)));
                }
                let index = u32::try_from(self.logs[n].len()).unwrap();
                self.logs[n].push(Some(value.0));
                Event::Submitted {
                    node,
                    epoch: Epoch(epoch),
                    index,
                    value,
                }
            }
            Step::Sync(node) => {
                let n = self.node(node)?;
                let (up, epoch) = self.nodes[n];
                if !up {
                    return Err(Refusal::NotEnabled(NotEnabled::NodeDown(node)));
                }
                if self.torn_tail(n) {
                    return Err(Refusal::ProgramFault(ProgramFault::WriteOverTornTail(node)));
                }
                let pending = self
                    .tickets
                    .iter()
                    .filter(|t| t.3 == Status::Pending)
                    .count();
                let max = cfg.max_pending();
                if pending >= max as usize {
                    return Err(Refusal::BoundReached(Bound::Pending { max }));
                }
                let ticket = TicketId(u32::try_from(self.tickets.len()).unwrap());
                let upto = u32::try_from(self.logs[n].len()).unwrap();
                self.tickets
                    .push((node.0, epoch, upto, Status::Pending, false));
                Event::SyncBegun {
                    ticket,
                    node,
                    epoch: Epoch(epoch),
                    upto,
                }
            }
            Step::Persist(ticket) => {
                let (at, node, epoch, upto, flushed) = self.pending_ticket(ticket)?;
                let (up, current) = self.nodes[usize::from(node)];
                if m == Mutant::PersistPrecedenceSwap && !(up && current == epoch) {
                    return Err(Refusal::NotEnabled(NotEnabled::IncarnationCrashed(ticket)));
                }
                if flushed {
                    return Err(Refusal::NotEnabled(NotEnabled::AlreadyStable(ticket)));
                }
                let n = usize::from(node);
                let (up, current) = self.nodes[n];
                if !(up && current == epoch) && m != Mutant::PersistAfterCrash {
                    return Err(Refusal::NotEnabled(NotEnabled::IncarnationCrashed(ticket)));
                }
                let upto = if m == Mutant::BarrierAtPersist {
                    u32::try_from(self.logs[n].len()).unwrap()
                } else {
                    upto
                };
                self.tickets[at].4 = true;
                self.stable[n] = if m == Mutant::StableNotMonotone {
                    upto
                } else {
                    self.stable[n].max(upto)
                };
                Event::Stable {
                    ticket,
                    node: NodeId(node),
                    epoch: Epoch(epoch),
                    upto,
                }
            }
            Step::Ack(ticket) => {
                let (at, node, epoch, upto, flushed) = self.pending_ticket(ticket)?;
                let live = self.live(m, node, epoch);
                if live && !flushed && m != Mutant::AckBeforeSync {
                    return Err(Refusal::NotEnabled(NotEnabled::NotStable(ticket)));
                }
                if m != Mutant::AckTwice {
                    self.tickets[at].3 = if live { Status::Acked } else { Status::Fenced };
                }
                let (node, epoch) = (NodeId(node), Epoch(epoch));
                if live {
                    Event::Acked {
                        ticket,
                        node,
                        epoch,
                        upto,
                    }
                } else {
                    Event::Fenced {
                        ticket,
                        node,
                        epoch,
                        upto,
                    }
                }
            }
            Step::Delay(ticket) => {
                let (at, node, epoch, upto, _) = self.pending_ticket(ticket)?;
                if m == Mutant::DelayResolves {
                    self.tickets[at].3 = Status::Fenced;
                }
                Event::Delayed {
                    ticket,
                    node: NodeId(node),
                    epoch: Epoch(epoch),
                    upto,
                }
            }
            Step::Crash { node, keep, torn } => {
                let n = self.node(node)?;
                let max = cfg.max_crashes();
                if max == 0 {
                    return Err(Refusal::NotEnabled(NotEnabled::FaultNotDeclared(
                        Semantic::CrashRestart,
                    )));
                }
                let (up, epoch) = self.nodes[n];
                if m == Mutant::PrecedenceSwap && !up {
                    return Err(Refusal::NotEnabled(NotEnabled::NodeDown(node)));
                }
                if self.crashes >= max && m != Mutant::CrashIgnoresBudget {
                    return Err(Refusal::NotEnabled(NotEnabled::CrashBudgetSpent { max }));
                }
                if !up {
                    return Err(Refusal::NotEnabled(NotEnabled::NodeDown(node)));
                }
                let intact = self.logs[n].iter().filter(|e| e.is_some()).count();
                let volatile = u32::try_from(intact).unwrap() - self.stable[n];
                if keep > volatile {
                    return Err(Refusal::Malformed(Malformed::KeepBeyondVolatile {
                        keep,
                        volatile,
                    }));
                }
                if torn && keep == volatile && m != Mutant::TornStable {
                    return Err(Refusal::Malformed(Malformed::NothingToTear));
                }
                if keep < volatile && !cfg.suffix_loss() && m != Mutant::SuffixLossUndeclared {
                    return Err(Refusal::NotEnabled(NotEnabled::FaultNotDeclared(
                        Semantic::VolatileSuffixLoss,
                    )));
                }
                if torn && !cfg.torn() && m != Mutant::TornUndeclared {
                    return Err(Refusal::NotEnabled(NotEnabled::FaultNotDeclared(
                        Semantic::TornWrites,
                    )));
                }
                let old_stable = self.stable[n];
                if !self.torn_tail(n) {
                    let survivors = (old_stable + keep) as usize;
                    let tear_value = self.logs[n].get(survivors).copied().flatten();
                    let mut log: Vec<Option<u32>> = self.logs[n][..survivors].to_vec();
                    if m == Mutant::LoseSynced && keep < volatile && old_stable > 0 {
                        log.remove(old_stable as usize - 1);
                    }
                    if torn {
                        if m == Mutant::TornStable && keep == volatile {
                            if let Some(last) = log.last_mut() {
                                *last = None;
                            }
                        } else if m == Mutant::TornUndetected {
                            log.push(tear_value);
                        } else {
                            log.push(None);
                        }
                    }
                    self.logs[n] = log;
                    if m != Mutant::KeptNotStable {
                        self.stable[n] = old_stable + keep;
                    }
                }
                self.nodes[n].0 = false;
                self.crashes += 1;
                Event::Crashed {
                    node,
                    epoch: Epoch(epoch),
                    keep,
                    torn,
                    lost: volatile - keep,
                }
            }
            Step::Restart(node) => {
                let n = self.node(node)?;
                if !cfg.restart() {
                    return Err(Refusal::NotEnabled(NotEnabled::FaultNotDeclared(
                        Semantic::CrashRestart,
                    )));
                }
                let (up, epoch) = self.nodes[n];
                if up {
                    return Err(Refusal::NotEnabled(NotEnabled::NodeUp(node)));
                }
                let next = if m == Mutant::RestartReusesEpoch {
                    epoch
                } else {
                    epoch + 1
                };
                self.nodes[n] = (true, next);
                Event::Restarted {
                    node,
                    epoch: Epoch(next),
                }
            }
            Step::Truncate(node) => {
                let n = self.node(node)?;
                let (up, epoch) = self.nodes[n];
                if !up {
                    return Err(Refusal::NotEnabled(NotEnabled::NodeDown(node)));
                }
                let droppable =
                    self.torn_tail(n) || (m == Mutant::TruncateAnyTail && !self.logs[n].is_empty());
                if !droppable {
                    return Err(Refusal::NotEnabled(NotEnabled::NoTornTail(node)));
                }
                self.logs[n].pop();
                let index = u32::try_from(self.logs[n].len()).unwrap();
                let len = u32::try_from(self.logs[n].len()).unwrap();
                self.stable[n] = self.stable[n].min(len);
                Event::Truncated {
                    node,
                    epoch: Epoch(epoch),
                    index,
                }
            }
        };
        Ok(event)
    }
}

// --- helpers --------------------------------------------------------------------------

#[allow(clippy::fn_params_excessive_bools)]
fn config(
    nodes: u8,
    crashes: u32,
    restart: bool,
    suffix_loss: bool,
    torn: bool,
    pending: u32,
) -> StorageConfig {
    StorageConfig::new(
        nodes,
        crashes,
        restart,
        suffix_loss,
        torn,
        pending,
        RETAINED,
    )
    .expect("valid config")
}

/// Every fault declared: the replicated-register scenario's storage with a caller's
/// node count, crash budget and pending bound.
fn full(nodes: u8, crashes: u32, pending: u32) -> StorageConfig {
    config(nodes, crashes, true, true, true, pending)
}

fn unsupported_steps() -> Vec<Step> {
    vec![
        Step::Corrupt {
            node: NodeId(0),
            index: 0,
        },
        Step::Reorder(NodeId(0)),
        Step::FalseFlush(TicketId(0)),
    ]
}

/// The differential's program bounds: at most this many `Submit`s (three on one node,
/// so a crash can meet a stable record, a kept one and a torn one at once, two
/// otherwise) and `Sync`s.
const fn submits(nodes: u8) -> usize {
    if nodes == 1 { 3 } else { 2 }
}
const SYNCS: usize = 2;

/// Every step worth trying in a state, in the order `enabled_choices` lists the
/// accepted ones: `Submit` (the next value) and `Sync` at every node and one unknown
/// node, within the program bounds; `Truncate` at every node and one unknown node;
/// `Persist`, `Ack` and `Delay` for every ticket begun and one never begun; every
/// `Crash` outcome at every node and one unknown node, one `keep` past the volatile
/// count included; `Restart` at every node and one unknown node; every unsupported
/// step.
fn candidates(model: &Model) -> Vec<Step> {
    let n = model.config.nodes();
    let mut out = Vec::new();
    let submitted = model.state.submitted;
    for node in 0..=n {
        if submitted < submits(n) {
            out.push(Step::Submit {
                node: NodeId(node),
                value: Value(u32::try_from(submitted).unwrap()),
            });
        }
        if model.state.tickets.len() < SYNCS {
            out.push(Step::Sync(NodeId(node)));
        }
        out.push(Step::Truncate(NodeId(node)));
    }
    for id in 0..=u32::try_from(model.state.tickets.len()).unwrap() {
        out.push(Step::Persist(TicketId(id)));
        out.push(Step::Ack(TicketId(id)));
        out.push(Step::Delay(TicketId(id)));
    }
    for node in 0..=n {
        let len = model
            .state
            .logs
            .get(usize::from(node))
            .map_or(0, |log| u32::try_from(log.len()).unwrap());
        for keep in 0..=len + 1 {
            for torn in [false, true] {
                out.push(Step::Crash {
                    node: NodeId(node),
                    keep,
                    torn,
                });
            }
        }
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

/// A seeded random walk: at each step, an enabled choice or a program step at a random
/// node. Program steps are offered three times, so programs keep writing.
fn random_log(rng: &mut XorShift, cfg: StorageConfig, len: usize) -> Vec<Step> {
    let mut storage = Storage::new(cfg);
    let mut log = Vec::new();
    let n = cfg.nodes();
    let mut attempts = 0;
    while log.len() < len && attempts < len * 50 {
        attempts += 1;
        let mut options = storage.enabled_choices();
        for _ in 0..3 {
            let node = NodeId(u8::try_from(rng.below(n.into())).unwrap());
            options.push(Step::Submit {
                node,
                value: Value(u32::try_from(rng.below(1000)).unwrap()),
            });
            options.push(Step::Sync(node));
            options.push(Step::Truncate(node));
        }
        let pick = options[rng.below(options.len())];
        if storage.apply(&pick).is_ok() {
            log.push(pick);
        }
    }
    log
}

fn state_after(cfg: StorageConfig, steps: &[Step]) -> Storage {
    let mut storage = Storage::new(cfg);
    for step in steps {
        storage.apply(step).unwrap();
    }
    storage
}

// --- positive evidence ----------------------------------------------------------------

/// `pr15-impl03-pos-01`. Differential against the reference model, over the complete
/// reachable space (no depth bound: every configuration has a small step bound that
/// binds, so every path ends, `Delay` stutters included, and the bound itself is
/// explored), under six configurations that between them switch every fault on and
/// off, one of them also under a retained-bytes budget that binds. The pruning key is
/// the model's whole [`State`], everything a step reads (cr-35ujnx).
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
    assert_eq!(kinds.len(), 9, "every event kind was reached: {kinds:?}");
    assert_eq!(
        refusals.len(),
        5,
        "every refusal class was reached: {refusals:?}"
    );
}

const PINNED_STATES: usize = 66_753;

/// `pr15-impl03-neg-04`. Anti-vacuity of the differential: each of the seeded bugs in
/// the reference model makes the exhaustive comparison fail.
#[test]
fn differential_is_not_vacuous_every_seeded_model_bug_is_caught() {
    for mutant in MUTANTS {
        if mutant == Mutant::None {
            continue;
        }
        let caught = differential_configs()
            .into_iter()
            .any(|cfg| explore(cfg, mutant).is_err());
        assert!(caught, "{mutant:?} survived the differential");
    }
}

/// The differential's configurations. Every one has a small step bound, so the whole
/// state space, the step count included, is finite, and the step bound itself is
/// reached and explored (cr-35ujnx) rather than assumed away.
fn differential_configs() -> [StorageConfig; 6] {
    let bounded = |cfg: StorageConfig, steps: u32| cfg.with_max_steps(steps).unwrap();
    [
        // A retained-bytes budget of the header plus 100 bytes: the budget and the step
        // bound, not the program or the crash budget, end most paths.
        bounded(
            StorageConfig::new(2, 1, true, true, true, 2, JOURNAL_HEADER_BYTES + 100).unwrap(),
            STEPS,
        ),
        // Every fault, one node, two crashes: a torn tail meets a second crash.
        bounded(full(1, 2, 2), STEPS),
        // Every fault, two nodes, one crash: one node's crash beside the other's syncs.
        bounded(full(2, 1, 2), STEPS),
        // Crash only: no loss, no tear, no restart.
        bounded(config(2, 1, false, false, false, 1), STEPS),
        // Loss without tears.
        bounded(config(1, 2, true, true, false, 2), STEPS),
        // No crash at all, and a step bound of three that ends every path early.
        bounded(config(3, 0, true, true, true, 2), 3),
    ]
}

/// The step bound of the differential's configurations.
const STEPS: u32 = 8;

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

/// The observable state: up set, epochs, logs, stable lengths, ticket facts, pending
/// tickets, crashes used, retained bytes.
type View = (
    u64,
    Vec<u32>,
    Vec<Vec<Option<u32>>>,
    Vec<u32>,
    Vec<(u8, u32, u32, bool)>,
    Vec<u32>,
    u32,
    usize,
    u64,
);

fn view_of_storage(s: &Storage) -> View {
    let n = s.config().nodes();
    (
        s.up().0,
        (0..n).map(|i| s.epoch(NodeId(i)).unwrap().0).collect(),
        (0..n)
            .map(|i| {
                s.log(NodeId(i))
                    .unwrap()
                    .iter()
                    .map(|e| match e {
                        Entry::Intact(Value(v)) => Some(*v),
                        Entry::Torn => None,
                    })
                    .collect()
            })
            .collect(),
        (0..n).map(|i| s.stable_len(NodeId(i)).unwrap()).collect(),
        (0..u32::try_from(s.ticket_count()).unwrap())
            .map(|t| {
                let (node, epoch, upto, stable) = s.ticket(TicketId(t)).unwrap();
                (node.0, epoch.0, upto, stable)
            })
            .collect(),
        s.pending().map(|t| t.0).collect(),
        s.crashes(),
        s.events().len(),
        s.retained_bytes(),
    )
}

fn view_of_model(model: &Model) -> View {
    let m = &model.state;
    (
        m.nodes
            .iter()
            .enumerate()
            .filter(|(_, (up, _))| *up)
            .fold(0_u64, |acc, (i, _)| acc | (1 << i)),
        m.nodes.iter().map(|(_, e)| *e).collect(),
        m.logs.clone(),
        m.stable.clone(),
        m.tickets
            .iter()
            .map(|(n, e, u, _, f)| (*n, *e, *u, *f))
            .collect(),
        m.tickets
            .iter()
            .enumerate()
            .filter(|(_, t)| t.3 == Status::Pending)
            .map(|(i, _)| u32::try_from(i).unwrap())
            .collect(),
        m.crashes,
        m.steps,
        m.retained,
    )
}

/// Explore the complete reachable space of `cfg` from the initial state, comparing the
/// Lab handler with the reference model on every candidate step of every state. `Err`
/// names the first disagreement.
fn explore(cfg: StorageConfig, mutant: Mutant) -> Result<Stats, String> {
    let mut stats = Stats {
        states: 0,
        transitions: 0,
        kinds: BTreeSet::new(),
        refusals: BTreeSet::new(),
    };
    let mut visited = BTreeSet::new();
    visited.insert(Model::new(cfg, mutant).key());
    let mut stack = vec![(Storage::new(cfg), Model::new(cfg, mutant))];
    while let Some((storage, model)) = stack.pop() {
        stats.states += 1;
        if storage.events() != model.events.as_slice() {
            return Err(format!("journal diverged at {:?}", model.key()));
        }
        let mut model_enabled = Vec::new();
        for step in candidates(&model) {
            let mut storage_next = storage.clone();
            let mut model_next = model.clone();
            let got = storage_next.apply(&step).copied();
            let want = model_next.step(&step);
            if got != want {
                return Err(format!(
                    "{step:?} from {:?}: {got:?} != {want:?}",
                    model.key()
                ));
            }
            if storage.check(&step) != want.map(|_| ()) {
                return Err(format!("check disagrees with apply on {step:?}"));
            }
            match want {
                Err(refusal) => {
                    if storage_next != storage {
                        return Err(format!("refused {step:?} changed the storage"));
                    }
                    stats
                        .refusals
                        .insert(variant(format!("{:?}", refusal.class())));
                }
                Ok(event) => {
                    // Compare the whole observable state after every accepted step, not
                    // only at newly visited states: a divergence that lands on an
                    // already visited model state would otherwise go unseen.
                    if view_of_storage(&storage_next) != view_of_model(&model_next) {
                        return Err(format!("state diverged after {step:?}"));
                    }
                    stats.transitions += 1;
                    stats.kinds.insert(variant(format!("{event:?}")));
                    if !matches!(
                        step,
                        Step::Submit { .. } | Step::Sync(_) | Step::Truncate(_)
                    ) {
                        model_enabled.push(step);
                    }
                    if visited.insert(model_next.key()) {
                        stack.push((storage_next, model_next));
                    }
                }
            }
        }
        if storage.enabled_choices() != model_enabled {
            return Err(format!("enabled choices differ at {:?}", model.key()));
        }
    }
    Ok(stats)
}

/// `pr15-impl03-pos-02`. Identical choice logs give byte-identical journals, over
/// seeded random walks under two configurations; distinct logs are not collapsed.
#[test]
fn determinism_identical_choice_logs_give_byte_identical_journals() {
    let configs = [full(4, 4, 6), config(3, 2, true, false, false, 4)];
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

/// `pr15-impl03-pos-03`. A journal's own choice log replays to the same journal.
#[test]
fn replay_a_journal_is_its_own_choice_log() {
    let cfg = full(4, 4, 6);
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

/// `pr15-impl03-pos-04`. Every step is atomic, so cutting a log at any boundary is a
/// valid run whose journal is exactly the full journal's prefix: prefix replay.
#[test]
fn every_step_boundary_truncation_replays_as_the_journal_prefix() {
    let cfg = full(3, 3, 6);
    let mut rng = XorShift(104_729);
    for _ in 0..50 {
        let log = random_log(&mut rng, cfg, 30);
        let full_journal = run(cfg, &log).unwrap();
        for cut in 0..=log.len() {
            let prefix = run(cfg, &log[..cut]).unwrap();
            assert_eq!(prefix.events(), &full_journal.events()[..cut]);
            assert_eq!(
                prefix.encode()[JOURNAL_HEADER_BYTES as usize..],
                full_journal.encode()
                    [JOURNAL_HEADER_BYTES as usize..prefix.retained_bytes() as usize]
            );
        }
    }
}

/// The records a node's `Acked` events vouch for, checked at the end of a run: for
/// every `Acked { node, upto }`, the first `upto` entries of the node's log at the ack
/// are still the first `upto` entries at the end. Returns how many acks it checked.
fn acks_stay_durable(cfg: StorageConfig, steps: &[Step]) -> usize {
    let mut storage = Storage::new(cfg);
    let mut vouched: Vec<(NodeId, Vec<Entry>)> = Vec::new();
    for step in steps {
        let event = *storage.apply(step).unwrap();
        if let Event::Acked { node, upto, .. } = event {
            let log = storage.log(node).unwrap();
            vouched.push((node, log[..upto as usize].to_vec()));
        }
    }
    for (node, prefix) in &vouched {
        let log = storage.log(*node).unwrap();
        assert!(
            log.len() >= prefix.len() && log[..prefix.len()] == prefix[..],
            "an acknowledged record of {node} was lost or changed"
        );
        assert!(prefix.iter().all(|e| matches!(e, Entry::Intact(_))));
    }
    vouched.len()
}

/// The crash outcomes spliced at a window: lose everything volatile, lose it with the
/// first record torn, and keep everything.
fn crash_outcomes(state: &Storage, node: NodeId) -> Vec<Step> {
    let volatile = if state.is_up(node) {
        let log = state.log(node).unwrap();
        let intact = log.iter().filter(|e| matches!(e, Entry::Intact(_))).count();
        u32::try_from(intact).unwrap() - state.stable_len(node).unwrap()
    } else {
        0
    };
    let mut out = vec![Step::Crash {
        node,
        keep: 0,
        torn: false,
    }];
    if volatile > 0 {
        out.push(Step::Crash {
            node,
            keep: 0,
            torn: true,
        });
        out.push(Step::Crash {
            node,
            keep: volatile,
            torn: false,
        });
    }
    out
}

/// `pr15-impl03-pos-05`. The PR-15 exit's "crash windows replay exactly", at the pack.
/// A crash window is a step boundary. For seeded logs, at every boundary and every
/// node, each crash outcome — lose the volatile suffix, lose it with a torn first
/// record, keep it — is spliced in alone, and again followed by `Restart` and, after a
/// tear, `Truncate`. Whenever the spliced log is a valid run:
///
/// - the journal up to the crash is the original's, event for event;
/// - the whole spliced journal replays from its own choice log, byte for byte, and a run
///   of the same spliced log gives the same bytes again;
/// - right after the crash, every record that was stable before it is still there,
///   intact and unchanged, and the stable length did not shrink;
/// - no ticket the crashed incarnation began is `Acked` after the crash, and each of
///   its completions is `Fenced`;
/// - every `Acked` range of the whole run is still in the log, unchanged, at the end.
///
/// When the spliced log is not a valid run, the refusal is at or after the splice, and
/// never at a step before it.
#[test]
fn crash_windows_replay_exactly_keep_every_stable_record_and_fence_that_incarnation() {
    // The logs are generated under a crash budget of one and run under three, so every
    // window has room for the spliced crash.
    let cfg = full(2, 3, 6);
    let generator = full(2, 1, 6);
    let mut rng = XorShift(0x5eed);
    let (mut windows, mut runs, mut fenced, mut acks) = (0, 0, 0, 0);
    for _ in 0..30 {
        let log = random_log(&mut rng, generator, 24);
        let original = run(cfg, &log).unwrap();
        for at in 0..=log.len() {
            let before = state_after(cfg, &log[..at]);
            for node in 0..cfg.nodes() {
                let node = NodeId(node);
                for crash in crash_outcomes(&before, node) {
                    let torn = matches!(crash, Step::Crash { torn: true, .. });
                    let mut recovered = vec![crash, Step::Restart(node)];
                    if torn {
                        recovered.push(Step::Truncate(node));
                    }
                    for splice in [vec![crash], recovered] {
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
                        assert_eq!(&journal.events()[..at], &original.events()[..at]);
                        assert_eq!(journal.replay().unwrap().encode(), journal.encode());
                        assert_eq!(run(cfg, &spliced).unwrap().encode(), journal.encode());
                        // Stable records survive the crash intact.
                        let stable = before.stable_len(node).unwrap() as usize;
                        let crashed = state_after(cfg, &spliced[..=at]);
                        assert_eq!(
                            crashed.log(node).unwrap()[..stable],
                            before.log(node).unwrap()[..stable],
                            "a stable record changed at the crash"
                        );
                        assert!(crashed.stable_len(node).unwrap() as usize >= stable);
                        // The crashed incarnation's tickets are fenced.
                        let epoch = before.epoch(node).unwrap();
                        let doomed: BTreeSet<TicketId> = before
                            .pending()
                            .filter(|t| {
                                let (n, e, _, _) = before.ticket(*t).unwrap();
                                n == node && e == epoch
                            })
                            .collect();
                        for event in &journal.events()[at + splice.len()..] {
                            match event {
                                Event::Acked { ticket, .. } => {
                                    assert!(!doomed.contains(ticket), "{ticket} crossed a crash");
                                }
                                Event::Fenced { ticket, .. } if doomed.contains(ticket) => {
                                    fenced += 1;
                                }
                                _ => {}
                            }
                        }
                        // Ack means durable, across the crash.
                        acks += acks_stay_durable(cfg, &spliced);
                    }
                }
            }
        }
    }
    // The seeds fix the logs, so the counts are pinned exactly: 6,164 windows, of which
    // 2,262 spliced logs are valid runs, with 375 completions of a crashed incarnation
    // fenced and 2,660 acknowledged ranges checked durable to the end. A change means
    // the evidence changed.
    assert_eq!((windows, runs, fenced, acks), PINNED_WINDOWS);
}

const PINNED_WINDOWS: (usize, usize, usize, usize) = (6_164, 2_262, 375, 2_660);

/// `pr15-impl03-pos-06`. The replicated register's M01 at the pack, under the
/// scenario's configuration. Replica 0 submits and syncs a record, and crashes before
/// the flush finished: its ack is refused while the incarnation runs (`NotStable`),
/// the crash loses the record, and the late completion is `Fenced`, so it vouches for
/// nothing. Replica 1 does the same in the correct order — flush, then ack — and its
/// record survives the crash.
#[test]
fn golden_ack_before_sync_loses_the_record_and_the_fenced_ack_vouches_for_nothing() {
    let cfg = StorageConfig::replicated_register_scenario(3, 8, RETAINED).unwrap();
    let (n0, n1) = (NodeId(0), NodeId(1));
    let (t0, t1) = (TicketId(0), TicketId(1));
    let v = Value(0x0001_0007);
    let mut storage = Storage::new(cfg);
    storage.apply(&Step::Submit { node: n0, value: v }).unwrap();
    storage.apply(&Step::Sync(n0)).unwrap();
    assert_eq!(
        storage.check(&Step::Ack(t0)),
        Err(Refusal::NotEnabled(NotEnabled::NotStable(t0))),
        "an acknowledgement waits for the flush"
    );
    let log = vec![
        Step::Submit { node: n0, value: v },
        Step::Sync(n0),
        Step::Crash {
            node: n0,
            keep: 0,
            torn: false,
        },
        Step::Ack(t0),
        Step::Restart(n0),
        Step::Submit { node: n1, value: v },
        Step::Sync(n1),
        Step::Persist(t1),
        Step::Ack(t1),
        Step::Crash {
            node: n1,
            keep: 0,
            torn: false,
        },
        Step::Restart(n1),
    ];
    let journal = run(cfg, &log).unwrap();
    let e0 = Epoch(0);
    assert_eq!(
        journal.events(),
        &[
            Event::Submitted {
                node: n0,
                epoch: e0,
                index: 0,
                value: v
            },
            Event::SyncBegun {
                ticket: t0,
                node: n0,
                epoch: e0,
                upto: 1
            },
            Event::Crashed {
                node: n0,
                epoch: e0,
                keep: 0,
                torn: false,
                lost: 1
            },
            Event::Fenced {
                ticket: t0,
                node: n0,
                epoch: e0,
                upto: 1
            },
            Event::Restarted {
                node: n0,
                epoch: Epoch(1)
            },
            Event::Submitted {
                node: n1,
                epoch: e0,
                index: 0,
                value: v
            },
            Event::SyncBegun {
                ticket: t1,
                node: n1,
                epoch: e0,
                upto: 1
            },
            Event::Stable {
                ticket: t1,
                node: n1,
                epoch: e0,
                upto: 1
            },
            Event::Acked {
                ticket: t1,
                node: n1,
                epoch: e0,
                upto: 1
            },
            Event::Crashed {
                node: n1,
                epoch: e0,
                keep: 0,
                torn: false,
                lost: 0
            },
            Event::Restarted {
                node: n1,
                epoch: Epoch(1)
            },
        ]
    );
    let end = state_after(cfg, &log);
    assert_eq!(end.log(n0).unwrap(), &[], "the unsynced record is lost");
    assert_eq!(
        end.log(n1).unwrap(),
        &[Entry::Intact(v)],
        "the acked record survives"
    );
    assert_eq!(acks_stay_durable(cfg, &log), 1);
    assert_eq!(
        end.check(&Step::Crash {
            node: NodeId(2),
            keep: 0,
            torn: false
        }),
        Err(Refusal::NotEnabled(NotEnabled::CrashBudgetSpent { max: 2 }))
    );
}

/// `pr15-impl03-pos-07`. Fault coverage, `crash`: from one log state with two stable
/// and three volatile records, every crash outcome keeps the two stable records intact,
/// keeps the first `keep` volatile ones, loses the rest, and tears the first lost one if
/// asked. Every survivor is stable afterwards. The enabled crash outcomes are exactly
/// these seven.
#[test]
fn fault_crash_keeps_stable_records_and_loses_or_tears_the_volatile_suffix() {
    let cfg = full(1, 4, 4);
    let n = NodeId(0);
    let mut s = Storage::new(cfg);
    let values: Vec<Value> = (10..15).map(Value).collect();
    for v in &values[..2] {
        s.apply(&Step::Submit { node: n, value: *v }).unwrap();
    }
    s.apply(&Step::Sync(n)).unwrap();
    s.apply(&Step::Persist(TicketId(0))).unwrap();
    for v in &values[2..] {
        s.apply(&Step::Submit { node: n, value: *v }).unwrap();
    }
    assert_eq!(s.stable_len(n), Some(2));
    let crashes: Vec<Step> = s
        .enabled_choices()
        .into_iter()
        .filter(|step| matches!(step, Step::Crash { .. }))
        .collect();
    assert_eq!(crashes.len(), 7);
    for keep in 0..=3_u32 {
        for torn in [false, true] {
            let mut c = s.clone();
            let step = Step::Crash {
                node: n,
                keep,
                torn,
            };
            if torn && keep == 3 {
                assert_eq!(
                    c.apply(&step),
                    Err(Refusal::Malformed(Malformed::NothingToTear))
                );
                continue;
            }
            assert!(crashes.contains(&step));
            assert_eq!(
                c.apply(&step).unwrap(),
                &Event::Crashed {
                    node: n,
                    epoch: Epoch(0),
                    keep,
                    torn,
                    lost: 3 - keep
                }
            );
            let mut want: Vec<Entry> = values[..2 + keep as usize]
                .iter()
                .map(|v| Entry::Intact(*v))
                .collect();
            if torn {
                want.push(Entry::Torn);
            }
            assert_eq!(c.log(n).unwrap(), want.as_slice());
            assert_eq!(c.stable_len(n), Some(2 + keep));
            assert!(!c.is_up(n));
        }
    }
    assert_eq!(
        s.check(&Step::Crash {
            node: n,
            keep: 4,
            torn: false
        }),
        Err(Refusal::Malformed(Malformed::KeepBeyondVolatile {
            keep: 4,
            volatile: 3
        }))
    );
}

/// `pr15-impl03-pos-08`. Fault coverage, `delay`: docs/17 §10's "delayed completion
/// from old epoch is rejected". `Delay` is the adversary's explicit, journalled stutter:
/// it holds a ticket back one step and changes nothing else. Held for any number of
/// steps while its incarnation runs, the completion is still delivered once the flush
/// finished. Held across a crash and a restart, it is fenced, and its flush can no
/// longer finish.
#[test]
fn fault_delay_an_ack_delayed_past_its_incarnation_is_fenced_never_delivered() {
    let cfg = full(1, 1, 4);
    let n = NodeId(0);
    let mut s = Storage::new(cfg);
    s.apply(&Step::Submit {
        node: n,
        value: Value(1),
    })
    .unwrap();
    s.apply(&Step::Sync(n)).unwrap();
    let delayed = |ticket: u32, upto: u32| Event::Delayed {
        ticket: TicketId(ticket),
        node: n,
        epoch: Epoch(0),
        upto,
    };
    for _ in 0..500 {
        let before = (
            s.pending().collect::<Vec<_>>(),
            s.log(n).unwrap().to_vec(),
            s.stable_len(n),
            s.ticket(TicketId(0)),
        );
        assert_eq!(s.apply(&Step::Delay(TicketId(0))).unwrap(), &delayed(0, 1));
        assert_eq!(
            (
                s.pending().collect::<Vec<_>>(),
                s.log(n).unwrap().to_vec(),
                s.stable_len(n),
                s.ticket(TicketId(0)),
            ),
            before,
            "a delay changed the state"
        );
    }
    s.apply(&Step::Persist(TicketId(0))).unwrap();
    for _ in 0..500 {
        assert_eq!(s.apply(&Step::Delay(TicketId(0))).unwrap(), &delayed(0, 1));
    }
    assert!(matches!(
        s.apply(&Step::Ack(TicketId(0))).unwrap(),
        Event::Acked { .. }
    ));
    assert_eq!(
        s.check(&Step::Delay(TicketId(0))),
        Err(Refusal::NotEnabled(NotEnabled::NotPending(TicketId(0))))
    );
    // The same delay across a crash and a restart: fenced.
    s.apply(&Step::Submit {
        node: n,
        value: Value(2),
    })
    .unwrap();
    s.apply(&Step::Sync(n)).unwrap();
    s.apply(&Step::Delay(TicketId(1))).unwrap();
    s.apply(&Step::Crash {
        node: n,
        keep: 0,
        torn: false,
    })
    .unwrap();
    assert_eq!(s.apply(&Step::Delay(TicketId(1))).unwrap(), &delayed(1, 2));
    s.apply(&Step::Restart(n)).unwrap();
    for _ in 0..500 {
        assert_eq!(s.apply(&Step::Delay(TicketId(1))).unwrap(), &delayed(1, 2));
    }
    assert_eq!(
        s.check(&Step::Persist(TicketId(1))),
        Err(Refusal::NotEnabled(NotEnabled::IncarnationCrashed(
            TicketId(1)
        )))
    );
    assert_eq!(
        s.apply(&Step::Ack(TicketId(1))).unwrap(),
        &Event::Fenced {
            ticket: TicketId(1),
            node: n,
            epoch: Epoch(0),
            upto: 2
        }
    );
    assert_eq!(s.log(n).unwrap(), &[Entry::Intact(Value(1))]);
}

/// `pr15-impl03-pos-09`. Recovery: a torn tail reads as `Torn`, never as the record it
/// was. While it is there the node can neither submit nor sync; it survives a second
/// crash, which has nothing left to lose or tear; `Truncate` drops it, and writes work
/// again. M03 at the pack: every restart's epoch is one more than the last.
#[test]
fn recovery_a_torn_tail_blocks_writes_until_truncated_and_survives_crashes() {
    let cfg = full(1, 3, 4);
    let n = NodeId(0);
    let (a, b) = (Value(1), Value(2));
    let mut s = Storage::new(cfg);
    s.apply(&Step::Submit { node: n, value: a }).unwrap();
    s.apply(&Step::Sync(n)).unwrap();
    s.apply(&Step::Persist(TicketId(0))).unwrap();
    s.apply(&Step::Submit { node: n, value: b }).unwrap();
    s.apply(&Step::Crash {
        node: n,
        keep: 0,
        torn: true,
    })
    .unwrap();
    assert_eq!(s.log(n).unwrap(), &[Entry::Intact(a), Entry::Torn]);
    assert_eq!(
        s.apply(&Step::Restart(n)).unwrap(),
        &Event::Restarted {
            node: n,
            epoch: Epoch(1)
        }
    );
    for step in [Step::Submit { node: n, value: b }, Step::Sync(n)] {
        assert_eq!(
            s.check(&step),
            Err(Refusal::ProgramFault(ProgramFault::WriteOverTornTail(n)))
        );
        let refusal = s.check(&step).unwrap_err();
        assert_eq!(refusal.class(), RefusalClass::ProgramFault);
        assert_eq!(
            refusal.inconclusive_reason(),
            None,
            "a program fault is a verdict"
        );
    }
    assert_eq!(
        s.check(&Step::Crash {
            node: n,
            keep: 0,
            torn: true
        }),
        Err(Refusal::Malformed(Malformed::NothingToTear))
    );
    s.apply(&Step::Crash {
        node: n,
        keep: 0,
        torn: false,
    })
    .unwrap();
    assert_eq!(s.log(n).unwrap(), &[Entry::Intact(a), Entry::Torn]);
    assert_eq!(
        s.check(&Step::Truncate(n)),
        Err(Refusal::NotEnabled(NotEnabled::NodeDown(n)))
    );
    s.apply(&Step::Restart(n)).unwrap();
    assert_eq!(s.epoch(n), Some(Epoch(2)));
    assert_eq!(
        s.apply(&Step::Truncate(n)).unwrap(),
        &Event::Truncated {
            node: n,
            epoch: Epoch(2),
            index: 1
        }
    );
    assert_eq!(s.log(n).unwrap(), &[Entry::Intact(a)]);
    assert_eq!(
        s.check(&Step::Truncate(n)),
        Err(Refusal::NotEnabled(NotEnabled::NoTornTail(n)))
    );
    s.apply(&Step::Submit { node: n, value: b }).unwrap();
    assert_eq!(s.log(n).unwrap(), &[Entry::Intact(a), Entry::Intact(b)]);
    assert_eq!(s.stable_len(n), Some(1));
}

// --- negative evidence ----------------------------------------------------------------

/// `pr15-impl03-neg-01`. Every step the profile does not model is refused as
/// `Unsupported` of exactly the row the step names, before any other check, that row is
/// declared unsupported, the INV-008 reading is `Unsupported`, and the state does not
/// change.
#[test]
fn negative_every_unsupported_step_is_a_typed_unsupported_refusal_and_changes_nothing() {
    let cfg = full(3, 2, 4);
    let mut s = Storage::new(cfg);
    s.apply(&Step::Submit {
        node: NodeId(0),
        value: Value(1),
    })
    .unwrap();
    s.apply(&Step::Sync(NodeId(0))).unwrap();
    s.apply(&Step::Crash {
        node: NodeId(1),
        keep: 0,
        torn: false,
    })
    .unwrap();
    let mut probes = unsupported_steps();
    // Unsupported comes before every other check, even for a node that cannot exist, a
    // node that is down, or a ticket never begun.
    probes.extend([
        Step::Corrupt {
            node: NodeId(200),
            index: 9,
        },
        Step::Reorder(NodeId(1)),
        Step::FalseFlush(TicketId(99)),
    ]);
    for step in probes {
        let before = s.clone();
        let refusal = s.apply(&step).expect_err("unsupported");
        let Refusal::Unsupported(semantic) = refusal else {
            panic!("{step:?} gave {refusal:?}");
        };
        assert_eq!(Some(semantic), step.unsupported_semantic());
        assert_eq!(semantic.support(), Support::Unsupported, "{semantic}");
        assert_eq!(refusal.class(), RefusalClass::Unsupported);
        assert_eq!(refusal.inconclusive_reason(), Some("Unsupported"));
        assert!(refusal.to_string().contains(semantic.statement()));
        assert_eq!(s, before, "{step:?} changed the storage");
    }
    let covered: BTreeSet<Semantic> = unsupported_steps()
        .iter()
        .filter_map(Step::unsupported_semantic)
        .collect();
    assert_eq!(covered.len(), 3);
    // The three unsupported rows with no step: nothing a caller can send asks for them.
    let stepless: BTreeSet<Semantic> = Semantic::ALL
        .into_iter()
        .filter(|s| s.support() == Support::Unsupported && !covered.contains(s))
        .collect();
    assert_eq!(
        stepless,
        [
            Semantic::DirectoryDurability,
            Semantic::DeviceProfile,
            Semantic::UndetectedTear
        ]
        .into_iter()
        .collect()
    );
}

/// `pr15-impl03-neg-02`. Undeclared faults, the crash budget, the pending bound, node,
/// tail and ticket state, impossible crash outcomes and unknown nodes each give their
/// own typed refusal, and change nothing.
#[test]
#[allow(clippy::too_many_lines)]
fn negative_undeclared_faults_bounds_and_malformed_steps_are_typed() {
    let (n0, n1) = (NodeId(0), NodeId(1));
    let no_faults = config(3, 0, false, false, false, 2);
    let mut s = Storage::new(no_faults);
    s.apply(&Step::Submit {
        node: n0,
        value: Value(1),
    })
    .unwrap();
    s.apply(&Step::Sync(n0)).unwrap();
    s.apply(&Step::Sync(n1)).unwrap();
    s.apply(&Step::Persist(TicketId(1))).unwrap();
    s.apply(&Step::Ack(TicketId(1))).unwrap();
    s.apply(&Step::Sync(n1)).unwrap();
    let cases = [
        (
            Step::Crash {
                node: n0,
                keep: 1,
                torn: false,
            },
            Refusal::NotEnabled(NotEnabled::FaultNotDeclared(Semantic::CrashRestart)),
        ),
        (
            Step::Restart(n0),
            Refusal::NotEnabled(NotEnabled::FaultNotDeclared(Semantic::CrashRestart)),
        ),
        (
            Step::Sync(n0),
            Refusal::BoundReached(Bound::Pending { max: 2 }),
        ),
        (
            Step::Ack(TicketId(0)),
            Refusal::NotEnabled(NotEnabled::NotStable(TicketId(0))),
        ),
        (
            Step::Ack(TicketId(1)),
            Refusal::NotEnabled(NotEnabled::NotPending(TicketId(1))),
        ),
        (
            Step::Persist(TicketId(1)),
            Refusal::NotEnabled(NotEnabled::NotPending(TicketId(1))),
        ),
        (
            Step::Persist(TicketId(9)),
            Refusal::NotEnabled(NotEnabled::NotPending(TicketId(9))),
        ),
        (
            Step::Delay(TicketId(9)),
            Refusal::NotEnabled(NotEnabled::NotPending(TicketId(9))),
        ),
        (
            Step::Truncate(n0),
            Refusal::NotEnabled(NotEnabled::NoTornTail(n0)),
        ),
        (
            Step::Submit {
                node: NodeId(3),
                value: Value(0),
            },
            Refusal::Malformed(Malformed::UnknownNode(NodeId(3))),
        ),
        (
            Step::Sync(NodeId(200)),
            Refusal::Malformed(Malformed::UnknownNode(NodeId(200))),
        ),
        (
            Step::Truncate(NodeId(64)),
            Refusal::Malformed(Malformed::UnknownNode(NodeId(64))),
        ),
        (
            Step::Restart(NodeId(3)),
            Refusal::Malformed(Malformed::UnknownNode(NodeId(3))),
        ),
    ];
    for (step, want) in cases {
        let before = s.clone();
        assert_eq!(s.apply(&step), Err(want), "{step:?}");
        assert_eq!(s, before);
    }
    s.apply(&Step::Persist(TicketId(2))).unwrap();
    assert_eq!(
        s.check(&Step::Persist(TicketId(2))),
        Err(Refusal::NotEnabled(NotEnabled::AlreadyStable(TicketId(2))))
    );
    // Crash outcomes: neither loss nor tear declared.
    let crash = |node, keep, torn| Step::Crash { node, keep, torn };
    let strict = config(2, 1, true, false, false, 4);
    let mut s = Storage::new(strict);
    for v in 0..2 {
        s.apply(&Step::Submit {
            node: n0,
            value: Value(v),
        })
        .unwrap();
    }
    for (step, want) in [
        (
            crash(n0, 3, false),
            Refusal::Malformed(Malformed::KeepBeyondVolatile {
                keep: 3,
                volatile: 2,
            }),
        ),
        (
            crash(n0, 2, true),
            Refusal::Malformed(Malformed::NothingToTear),
        ),
        (
            crash(n0, 1, true),
            Refusal::NotEnabled(NotEnabled::FaultNotDeclared(Semantic::VolatileSuffixLoss)),
        ),
        (
            crash(n0, 0, false),
            Refusal::NotEnabled(NotEnabled::FaultNotDeclared(Semantic::VolatileSuffixLoss)),
        ),
        (
            crash(NodeId(2), 0, false),
            Refusal::Malformed(Malformed::UnknownNode(NodeId(2))),
        ),
    ] {
        assert_eq!(s.check(&step), Err(want), "{step:?}");
    }
    assert_eq!(
        s.check(&Step::Restart(n0)),
        Err(Refusal::NotEnabled(NotEnabled::NodeUp(n0)))
    );
    s.apply(&crash(n0, 2, false)).unwrap();
    // The budget is checked before the node state.
    assert_eq!(
        s.check(&crash(n0, 0, false)),
        Err(Refusal::NotEnabled(NotEnabled::CrashBudgetSpent { max: 1 }))
    );
    assert_eq!(
        s.check(&crash(n1, 0, false)),
        Err(Refusal::NotEnabled(NotEnabled::CrashBudgetSpent { max: 1 }))
    );
    for step in [
        Step::Submit {
            node: n0,
            value: Value(9),
        },
        Step::Sync(n0),
        Step::Truncate(n0),
    ] {
        assert_eq!(
            s.check(&step),
            Err(Refusal::NotEnabled(NotEnabled::NodeDown(n0)))
        );
    }
    // Loss declared, tears not.
    let no_tears = config(1, 1, true, true, false, 4);
    let mut s = Storage::new(no_tears);
    s.apply(&Step::Submit {
        node: n0,
        value: Value(1),
    })
    .unwrap();
    assert_eq!(
        s.check(&crash(n0, 0, true)),
        Err(Refusal::NotEnabled(NotEnabled::FaultNotDeclared(
            Semantic::TornWrites
        )))
    );
    s.apply(&crash(n0, 0, false)).unwrap();
    assert_eq!(s.log(n0).unwrap(), &[]);
    assert_eq!(
        Refusal::BoundReached(Bound::Pending { max: 2 }).inconclusive_reason(),
        Some("ResourceExhausted")
    );
    assert_eq!(
        Refusal::NotEnabled(NotEnabled::NodeDown(n0)).inconclusive_reason(),
        None
    );
    assert_eq!(
        Refusal::Malformed(Malformed::NothingToTear).class(),
        RefusalClass::Malformed
    );
    // A refused step inside a log names its index, and nothing after it runs.
    assert_eq!(
        run(
            strict,
            &[
                Step::Sync(n0),
                Step::Ack(TicketId(0)),
                Step::Submit {
                    node: n1,
                    value: Value(0)
                }
            ]
        )
        .unwrap_err()
        .index,
        1
    );
}

/// `pr15-impl03-neg-03`. Anti-vacuity: changing one step of a valid log changes the
/// journal, or makes the log invalid, and never leaves the journal as it was.
#[test]
fn negative_a_perturbed_choice_log_changes_the_journal() {
    let cfg = full(4, 4, 8);
    let mut rng = XorShift(7);
    let mut changed = 0;
    for _ in 0..200 {
        let log = random_log(&mut rng, cfg, 20);
        let journal = run(cfg, &log).unwrap();
        let at = rng.below(log.len());
        let mut perturbed = log.clone();
        perturbed[at] = match log[at] {
            Step::Submit { node, value } => Step::Submit {
                node,
                value: Value(value.0 + 1),
            },
            Step::Sync(NodeId(n)) => Step::Sync(NodeId((n + 1) % 4)),
            Step::Persist(t) | Step::Delay(t) => Step::Ack(t),
            Step::Ack(t) => Step::Persist(t),
            Step::Crash { node, keep, torn } => Step::Crash {
                node,
                keep: keep + 1,
                torn,
            },
            Step::Restart(n) => Step::Crash {
                node: n,
                keep: 0,
                torn: false,
            },
            Step::Truncate(n) => Step::Restart(n),
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

/// `pr15-impl03-bnd-01`. The choice-log length is charged before any step runs.
#[test]
fn boundary_an_overlong_log_is_refused_before_any_step_runs() {
    let cfg = config(2, 0, false, false, false, 1);
    // Every step would be refused as `NotPending` at index 0 if it ran.
    let log = vec![Step::Ack(TicketId(0)); MAX_STEPS + 1];
    let refusal = run(cfg, &log).unwrap_err();
    assert_eq!(refusal.index, MAX_STEPS + 1);
    assert_eq!(
        refusal.refusal,
        Refusal::BoundReached(Bound::Steps { max: MAX_STEPS })
    );
}

/// `pr15-impl03-bnd-02`. Configuration caps.
#[test]
fn boundary_configuration_caps_are_typed_refusals() {
    let new = |nodes, crashes, pending| {
        StorageConfig::new(nodes, crashes, true, true, true, pending, RETAINED)
    };
    assert_eq!(new(0, 1, 1), Err(ConfigRefusal::NodesOutOfRange(0)));
    assert_eq!(new(65, 1, 1), Err(ConfigRefusal::NodesOutOfRange(65)));
    assert_eq!(
        new(2, MAX_CRASHES_CAP + 1, 1),
        Err(ConfigRefusal::CrashesOutOfRange(MAX_CRASHES_CAP + 1))
    );
    assert_eq!(new(2, 1, 0), Err(ConfigRefusal::PendingOutOfRange(0)));
    // The step bound: `1..=MAX_STEPS`, and a small one binds exactly.
    assert_eq!(
        full(1, 1, 1).with_max_steps(0),
        Err(ConfigRefusal::StepsOutOfRange(0))
    );
    let over = u32::try_from(MAX_STEPS).unwrap() + 1;
    assert_eq!(
        full(1, 1, 1).with_max_steps(over),
        Err(ConfigRefusal::StepsOutOfRange(over))
    );
    let three = full(1, 1, 1).with_max_steps(3).unwrap();
    let log = vec![
        Step::Submit {
            node: NodeId(0),
            value: Value(1)
        };
        4
    ];
    assert_eq!(
        run(three, &log).unwrap_err(),
        continuum_effects_storage::RunRefusal {
            index: 4,
            refusal: Refusal::BoundReached(Bound::Steps { max: 3 })
        }
    );
    let mut s = Storage::new(three);
    for step in &log[..3] {
        s.apply(step).unwrap();
    }
    assert_eq!(
        s.apply(&log[3]),
        Err(Refusal::BoundReached(Bound::Steps { max: 3 }))
    );
    assert_eq!(s.into_journal().replay().unwrap().events().len(), 3);
    // A tear needs a lost record, so torn records without suffix loss are refused.
    assert_eq!(
        StorageConfig::new(1, 1, true, false, true, 1, RETAINED),
        Err(ConfigRefusal::TornWithoutLoss)
    );
    assert_eq!(
        new(2, 1, MAX_PENDING_CAP + 1),
        Err(ConfigRefusal::PendingOutOfRange(MAX_PENDING_CAP + 1))
    );
    // 64 nodes is the largest set a `NodeSet` holds, and node 63 works.
    let wide = new(64, MAX_CRASHES_CAP, 1).unwrap();
    let mut s = Storage::new(wide);
    assert_eq!(s.up(), NodeSet(u64::MAX));
    let n = NodeId(63);
    s.apply(&Step::Submit {
        node: n,
        value: Value(u32::MAX),
    })
    .unwrap();
    s.apply(&Step::Crash {
        node: n,
        keep: 1,
        torn: false,
    })
    .unwrap();
    assert_eq!(s.up(), NodeSet(u64::MAX >> 1));
    assert_eq!(
        s.apply(&Step::Restart(n)).unwrap(),
        &Event::Restarted {
            node: n,
            epoch: Epoch(1)
        }
    );
    assert_eq!(s.log(n).unwrap(), &[Entry::Intact(Value(u32::MAX))]);
    assert_eq!(s.epoch(NodeId(64)), None);
    assert_eq!(s.log(NodeId(64)), None);
    assert_eq!(s.stable_len(NodeId(64)), None);
    assert!(!s.is_up(NodeId(64)));
}

/// `pr15-impl03-bnd-03`. Storage driven step by step, without `run`, stops at the
/// same `MAX_STEPS` bound, so every journal it produces replays.
#[test]
fn boundary_a_directly_driven_storage_stops_at_max_steps_and_its_journal_replays() {
    let cfg = config(1, 0, false, false, false, 1);
    let mut s = Storage::new(cfg);
    let n = NodeId(0);
    for step in 0..MAX_STEPS {
        s.apply(&Step::Submit {
            node: n,
            value: Value(u32::try_from(step).unwrap()),
        })
        .unwrap();
    }
    assert_eq!(
        s.apply(&Step::Sync(n)),
        Err(Refusal::BoundReached(Bound::Steps { max: MAX_STEPS }))
    );
    let journal = s.into_journal();
    assert_eq!(journal.events().len(), MAX_STEPS);
    assert_eq!(journal.replay().unwrap(), journal);
}

/// `pr15-impl03-bnd-04`. The retained-bytes budget bounds the whole journal, is
/// charged before anything is pushed, and holds for clone, replay and encode.
#[test]
fn boundary_the_retained_bytes_budget_is_charged_before_every_push() {
    let n = NodeId(0);
    let submit = Step::Submit {
        node: n,
        value: Value(5),
    };
    let one_submit = JOURNAL_HEADER_BYTES + 14;
    let budget = |max| StorageConfig::new(1, 1, true, true, true, 4, max).unwrap();
    // Exact boundary: one `Submit` fits exactly.
    let mut s = Storage::new(budget(one_submit));
    s.apply(&submit).unwrap();
    assert_eq!(s.retained_bytes(), one_submit);
    assert_eq!(s.encode().len() as u64, one_submit);
    for step in [
        Step::Sync(n),
        submit,
        Step::Crash {
            node: n,
            keep: 0,
            torn: true,
        },
    ] {
        let before = s.clone();
        assert_eq!(
            s.apply(&step),
            Err(Refusal::BoundReached(Bound::Retained { max: one_submit }))
        );
        assert_eq!(s, before, "a refused charge changed the storage");
    }
    // One byte short: the same step is refused before anything is retained.
    let mut s = Storage::new(budget(one_submit - 1));
    assert_eq!(
        s.apply(&submit),
        Err(Refusal::BoundReached(Bound::Retained {
            max: one_submit - 1
        }))
    );
    assert_eq!(s.retained_bytes(), JOURNAL_HEADER_BYTES);
    assert!(s.events().is_empty());
    assert_eq!(s.log(n).unwrap(), &[]);
    // A crash (15 bytes) is charged by its own size: it fits in one byte more than a
    // submit.
    let mut s = Storage::new(budget(JOURNAL_HEADER_BYTES + 15));
    s.apply(&Step::Crash {
        node: n,
        keep: 0,
        torn: false,
    })
    .unwrap();
    // Submit, sync, persist and ack at pending 1: the pending bound never binds, the
    // budget does, after exactly as many rounds as it pays for.
    let rounds = 10_u64;
    let max = JOURNAL_HEADER_BYTES + rounds * 56;
    let cfg = StorageConfig::new(1, 0, false, false, false, 1, max).unwrap();
    let mut s = Storage::new(cfg);
    let mut done = 0;
    loop {
        let t = TicketId(u32::try_from(s.ticket_count()).unwrap());
        if let Err(refusal) = s.apply(&submit) {
            assert_eq!(refusal, Refusal::BoundReached(Bound::Retained { max }));
            break;
        }
        s.apply(&Step::Sync(n)).unwrap();
        s.apply(&Step::Persist(t)).unwrap();
        s.apply(&Step::Ack(t)).unwrap();
        done += 1;
    }
    assert_eq!(done, rounds);
    assert_eq!(s.retained_bytes(), max);
    let journal = s.into_journal();
    let copy = journal.clone();
    assert_eq!(copy.retained_bytes(), max);
    let replayed = journal.replay().unwrap();
    assert_eq!(replayed, journal);
    assert_eq!(replayed.encode().len() as u64, max);
    assert_eq!(journal.encode(), replayed.encode());
    // Configuration bounds on the budget itself.
    assert_eq!(
        StorageConfig::new(1, 0, false, false, false, 1, JOURNAL_HEADER_BYTES - 1),
        Err(ConfigRefusal::RetainedOutOfRange(JOURNAL_HEADER_BYTES - 1))
    );
    assert_eq!(
        StorageConfig::new(1, 0, false, false, false, 1, MAX_RETAINED_CAP + 1),
        Err(ConfigRefusal::RetainedOutOfRange(MAX_RETAINED_CAP + 1))
    );
    // The header alone is a valid, empty journal.
    let bare = StorageConfig::new(1, 0, false, false, false, 1, JOURNAL_HEADER_BYTES).unwrap();
    assert_eq!(
        Storage::new(bare).encode().len() as u64,
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
const MANIFEST: &str =
    include_str!("../../../notes/plan/schemas/examples/storage-pack.manifest.json");
const PROCESS_PROFILE: &str = include_str!("../../continuum-effects-process/src/profile.rs");

/// The text between a heading and the next heading of any level.
fn section<'t>(text: &'t str, heading: &str) -> &'t str {
    let start = text.find(heading).expect("heading present") + heading.len();
    let rest = &text[start..];
    let end = rest.find("\n#").unwrap_or(rest.len());
    &rest[..end]
}

fn bullets(text: &str) -> Vec<&str> {
    text.lines()
        .filter_map(|line| line.trim_start().strip_prefix("- "))
        .map(|line| line.trim_end_matches([';', '.']))
        .collect()
}

/// `pr15-impl03-hon-01`. Every storage semantic RFC 0002 "Storage" names, every claim,
/// modelled fault, exclusion and assumption of docs/17 §6's durable-storage example,
/// and every constraint of docs/17 §10 maps to declared rows; every row is declared
/// once with a statement. A new bullet in any of these lists fails this test until the
/// profile states it.
#[test]
fn honesty_the_profile_covers_every_rfc_0002_and_docs_17_storage_semantic() {
    use Semantic as S;
    let rfc: &[(&str, &[Semantic])] = &[
        ("volatile process memory", &[S::VolatileProcessMemory]),
        ("page cache / submitted writes", &[S::SubmittedWrites]),
        ("durable stable storage", &[S::StableStorage]),
        ("atomicity granularity", &[S::AtomicityGranularity]),
        (
            "ordering and barriers",
            &[S::OrderingAndBarriers, S::WriteReordering],
        ),
        ("torn writes", &[S::TornWrites]),
        ("sector/block corruption", &[S::SectorCorruption]),
        (
            "rename/link/directory durability",
            &[S::DirectoryDurability],
        ),
        (
            "crash and restart",
            &[S::CrashRestart, S::VolatileSuffixLoss],
        ),
        ("recovery procedure", &[S::RecoveryProcedure]),
        ("device and filesystem profile", &[S::DeviceProfile]),
    ];
    assert_eq!(
        bullets(section(RFC_0002, "### Storage")),
        rfc.iter().map(|(bullet, _)| *bullet).collect::<Vec<_>>(),
        "RFC 0002's storage list changed; state the new semantics in the profile"
    );
    // docs/17 §6: the example profile's claims, modelled faults, exclusions and
    // assumptions. The example models write reordering; this profile does not, and says
    // so in its row.
    let docs17_example: &[(&str, &[Semantic])] = &[
        (
            "write becomes visible to process after successful syscall",
            &[S::SubmittedWrites],
        ),
        (
            "sync completion is the modeled durability boundary",
            &[S::StableStorage, S::SyncAcknowledgement],
        ),
        ("process crash", &[S::CrashRestart]),
        ("loss of unsynced writes", &[S::VolatileSuffixLoss]),
        ("write reordering before sync", &[S::WriteReordering]),
        ("device firmware lies about flush", &[S::FlushDishonesty]),
        ("latent sector corruption", &[S::SectorCorruption]),
        ("kernel/filesystem bugs", &[S::DeviceProfile]),
        (
            "power-loss torn sector below declared atomic unit",
            &[S::AtomicityGranularity, S::UndetectedTear],
        ),
        ("mount options: [...]", &[S::DeviceProfile]),
        ("device cache configuration: [...]", &[S::DeviceProfile]),
    ];
    assert_eq!(
        bullets(section(
            DOCS_17,
            "## 6. Fidelity profile example: durable storage"
        )),
        docs17_example
            .iter()
            .map(|(bullet, _)| *bullet)
            .collect::<Vec<_>>(),
        "docs/17 §6's durable-storage example changed"
    );
    // docs/17 §10's constraints: the storage ones map to rows; the partition and clock
    // ones are the network and time packs', and cancellation versus crash is the
    // process pack's (this pack's cancellation contract says what a cancellation after
    // Submitted leaves).
    let docs17_faults: &[(&str, &[Semantic])] = &[
        (
            "crash clears volatile state",
            &[S::VolatileProcessMemory, S::VolatileSuffixLoss],
        ),
        ("recovery increments epoch", &[S::CrashRestart]),
        (
            "delayed completion from old epoch is rejected or explicitly modeled",
            &[S::SyncAcknowledgement],
        ),
        ("partition affects matching routes", &[]),
        (
            "clock jump changes wall mapping, not monotonic logical time",
            &[],
        ),
        ("cancellation is not process crash", &[]),
    ];
    assert_eq!(
        bullets(section(DOCS_17, "## 10. Fault algebra")),
        docs17_faults
            .iter()
            .map(|(bullet, _)| *bullet)
            .collect::<Vec<_>>(),
        "docs/17 §10's constraint list changed"
    );
    // docs/17 §9's typical storage dependence: the profile claims no independence, so
    // every listed dependence holds trivially.
    assert_eq!(
        bullets(section(DOCS_17, "Typical storage dependence:")).len(),
        4
    );
    assert_eq!(APPEND_LOG_V0.independence, IndependenceClaim::AllDependent);
    let mut mapped: BTreeSet<Semantic> = BTreeSet::new();
    for (_, rows) in rfc.iter().chain(docs17_example).chain(docs17_faults) {
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
            S::VolatileProcessMemory,
            S::SubmittedWrites,
            S::StableStorage,
            S::AtomicityGranularity,
            S::OrderingAndBarriers,
            S::VolatileSuffixLoss,
            S::TornWrites,
            S::CrashRestart,
            S::RecoveryProcedure,
            S::SyncAcknowledgement,
            S::ExplorationBounds,
        ]
    );
    assert!(S::WriteReordering.statement().contains("prefix"));
    assert_eq!(
        ASSUMPTIONS.map(|(id, _)| id),
        [
            "sync-promotes-a-prefix",
            "durable-log-is-a-prefix",
            "honest-flush",
            "detected-tear",
            "durable-truncation",
            "no-eventual-flush",
            "no-medium-corruption",
            "no-device-profile",
            "durable-log-entry",
        ]
    );
    assert_eq!(CANCELLATION_CONTRACT.len(), 8);
}

/// The quoted strings of the JSON array that follows `key` in `text`.
fn json_strings<'t>(text: &'t str, key: &str) -> Vec<&'t str> {
    let at = text.find(key).unwrap_or_else(|| panic!("{key} present")) + key.len();
    let rest = &text[at..];
    let body = &rest[rest.find('[').unwrap() + 1..rest.find(']').unwrap()];
    body.split('"').skip(1).step_by(2).collect()
}

/// `pr15-impl03-hon-02`. The replicated-register scenario, its Intent Contract and the
/// schema example `storage-pack.manifest.json` all cite this profile by its exact name.
/// The scenario's storage flags are the scenario helper's, the manifest's class,
/// assumption and three faults are this profile's, and the contract's storage
/// assumption is this profile's `durable-log-is-a-prefix`.
#[test]
fn honesty_the_profile_is_the_one_the_scenario_contract_and_manifest_cite() {
    // The scenario.
    let storage = section(SCENARIO, "[storage]");
    let value = |key: &str| {
        storage
            .lines()
            .find_map(|line| line.strip_prefix(key))
            .unwrap_or_else(|| panic!("{key}"))
            .trim()
    };
    // The scenario, the contract and the manifest name the frozen `-v0`, which the
    // pack still declares, byte for byte.
    assert_eq!(value("profile = "), format!("\"{PROFILE_NAME_V0}\""));
    let cfg = StorageConfig::replicated_register_scenario(3, 8, RETAINED).unwrap();
    assert_eq!(
        value("allow_volatile_suffix_loss = "),
        cfg.suffix_loss().to_string()
    );
    assert_eq!(value("allow_torn_last_record = "), cfg.torn().to_string());
    let max_crashes = section(SCENARIO, "[faults]")
        .lines()
        .find_map(|line| line.strip_prefix("max_crashes = "))
        .expect("max_crashes");
    assert_eq!(max_crashes, cfg.max_crashes().to_string());
    assert!(cfg.restart());
    // The Intent Contract.
    assert!(json_strings(CONTRACT, "\"profiles\"").contains(&PROFILE_NAME_V0));
    assert!(json_strings(CONTRACT, "\"trusted\"").contains(&PROFILE_NAME_V0));
    assert!(CONTRACT.contains("durable_log_is_a_prefix_of_appended"));
    assert!(CONTRACT.contains("\"id\":\"DurableLogIsAPrefix\""));
    assert!(ASSUMPTIONS[1].1.contains("DurableLogIsAPrefix"));
    // The manifest example.
    assert!(MANIFEST.contains(&format!("\"id\": \"{PROFILE_NAME_V0}\"")));
    assert!(MANIFEST.contains(&format!("\"class\": \"{}\"", APPEND_LOG_V0.class.as_str())));
    for assumption in json_strings(MANIFEST, "\"assumptions\"") {
        assert!(
            ASSUMPTIONS
                .iter()
                .any(|(_, text)| text.contains(assumption)),
            "the profile does not state the manifest's assumption {assumption:?}"
        );
    }
    let faults = json_strings(MANIFEST, "\"faults\"");
    assert_eq!(
        faults,
        ["crash", "volatile-suffix-loss", "torn-last-record"]
    );
    assert_eq!(Semantic::VolatileSuffixLoss.token(), faults[1]);
    assert_eq!(Semantic::TornWrites.token(), faults[2]);
    for phase in ["Submitted", "Stable", "Acknowledged"] {
        assert!(json_strings(MANIFEST, "\"effect_phases\"").contains(&phase));
    }
}

/// `pr15-impl03-hon-03`. T06: the declared profile claims no host semantics. This is
/// the test GOV-4-13's not-applicable form names as the source of truth for the value
/// (`tools/governance/reviews/bn-2fk3.toml`). The absence of host effects themselves is
/// the compiler's to prove: the crate is `#![no_std]`, and `tests/pr15_no_std_lane.rs`
/// shows that proof is live.
#[test]
fn profile_declares_host_qualification_none() {
    // Both declared profiles, the frozen `-v0` and the current `-v1`, claim no host.
    assert_eq!(APPEND_LOG_V1.host, HostQualification::None);
    for profile in PROFILES {
        assert_eq!(profile.host, HostQualification::None, "{}", profile.name);
        assert_ne!(profile.class, FidelityClass::PlatformQualified);
    }
    assert_eq!(APPEND_LOG_V1.class, FidelityClass::AdversarialEnvelope);
    assert_ne!(APPEND_LOG_V1.class, FidelityClass::PlatformQualified);
    assert_eq!(APPEND_LOG_V1.independence, IndependenceClaim::AllDependent);
}

/// `pr15-impl03-hon-04`. The profile's canonical bytes are pinned together with its
/// version (docs/17 §12). Editing a row, a statement, an assumption, the composition
/// table or the cancellation contract changes the fingerprint and fails this test. The
/// rule is to bump `PROFILE_VERSION` and re-pin both; this test catches an unnoticed
/// edit, but a deliberate re-pin without a bump is caught only by review.
#[test]
fn honesty_the_profile_bytes_are_pinned_to_its_version() {
    let bytes = APPEND_LOG_V1.canonical_bytes();
    let fingerprint = bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    });
    assert_eq!(
        (PROFILE_VERSION.to_string(), fingerprint),
        ("1.0.0".to_owned(), PINNED_FINGERPRINT),
        "storage/append-log-v1 changed: a content change needs a new profile name (RFC 0002 correction 1)"
    );
    let journal: Journal = run(
        config(1, 0, false, false, false, 1),
        &[Step::Sync(NodeId(0))],
    )
    .unwrap();
    let name = PROFILE_NAME.as_bytes();
    assert!(journal.encode().windows(name.len()).any(|w| w == name));
}

const PINNED_FINGERPRINT: u64 = 11_729_929_152_830_614_249;

/// `pr15-impl03-hon-05`. The journal's wire form is pinned byte for byte, so a change
/// to tags, field order, widths or endianness fails here rather than passing every
/// self-consistency test. The log reaches every event kind.
#[test]
fn honesty_the_journal_encoding_is_pinned_byte_for_byte() {
    let cfg = full(2, 2, 4);
    let (n0, n1) = (NodeId(0), NodeId(1));
    let log = [
        Step::Submit {
            node: n0,
            value: Value(0xa1),
        },
        Step::Sync(n0),
        Step::Persist(TicketId(0)),
        Step::Delay(TicketId(0)),
        Step::Ack(TicketId(0)),
        Step::Submit {
            node: n0,
            value: Value(0xb2),
        },
        Step::Sync(n1),
        Step::Crash {
            node: n0,
            keep: 0,
            torn: true,
        },
        Step::Restart(n0),
        Step::Truncate(n0),
        Step::Crash {
            node: n1,
            keep: 0,
            torn: false,
        },
        Step::Ack(TicketId(1)),
    ];
    let journal = run(cfg, &log).unwrap();
    let kinds: BTreeSet<String> = journal
        .events()
        .iter()
        .map(|e| variant(format!("{e:?}")))
        .collect();
    assert_eq!(kinds.len(), 9);
    let hex: String = journal
        .encode()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(hex, PINNED_JOURNAL_HEX, "the journal wire form changed");
}

const PINNED_JOURNAL_HEX: &str = "636f6e74696e75756d2d73746f726167652d6a6f75726e616c000000001573746f726167652f617070656e642d6c6f672d76310001000000000200000002010101000000040010000000000000100000000000000c01000000000000000000000000a1020000000000000000000000000103000000000000000000000000010600000000000000000000000001040000000000000000000000000101000000000000000001000000b20200000001010000000000000000070000000000000000000100000001080000000001090000000001000000010701000000000000000000000000000500000001010000000000000000";

/// `pr15-impl03-hon-06`. The composition with the process pack, read from its source
/// rather than linked (a dev-dependency would be a new dependency edge). Its `storage`
/// composition row defers what stable storage keeps to this pack; its epoch rule (one
/// more than the last) and its fence (a completion after the incarnation crashed is
/// `Fenced`) are the rules this pack restates; and this profile's composition table
/// answers for `process/crash-restart-v0` by its exact name. The epoch and fence rules
/// are also exercised here, on this pack's own handler.
#[test]
fn honesty_the_process_pack_defers_storage_here_and_this_profile_composes_with_its_crash_and_fence()
{
    let flat: String = PROCESS_PROFILE
        .split_whitespace()
        .map(|word| word.trim_matches('\\'))
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    for promise in [
        "what stable storage keeps across a crash is the storage pack's to state",
        "whose epoch is one more than the last",
        "is Fenced: journalled and discarded, never delivered",
    ] {
        assert!(
            flat.contains(promise),
            "the process profile no longer says: {promise}"
        );
    }
    assert!(
        PROCESS_PROFILE.contains("pub const PROFILE_NAME_V0: &str = \"process/crash-restart-v0\";")
    );
    assert_eq!(
        COMPOSITION.map(|(pack, _)| pack),
        [
            "process/crash-restart-v0",
            "network/adversarial-v0",
            "runtime"
        ]
    );
    let (_, statement) = COMPOSITION[0];
    for claim in [
        "at the same step boundary",
        "the same crash budget and restart switch",
        "one more at each restart",
        "by node and epoch",
        "keeps every stable record intact",
    ] {
        assert!(statement.contains(claim), "{claim}");
    }
    assert!(COMPOSITION[1].1.contains("ack-before-sync"));
    // The rules, on this pack's handler, in the process pack's lockstep: crash and
    // restart the same node twice; the epochs are 1 and 2, and a ticket of epoch 0
    // is fenced whether the node is down or up again.
    let cfg = full(1, 2, 4);
    let n = NodeId(0);
    let mut s = Storage::new(cfg);
    s.apply(&Step::Sync(n)).unwrap();
    s.apply(&Step::Sync(n)).unwrap();
    s.apply(&Step::Persist(TicketId(1))).unwrap();
    let crash = Step::Crash {
        node: n,
        keep: 0,
        torn: false,
    };
    s.apply(&crash).unwrap();
    assert!(matches!(
        s.apply(&Step::Ack(TicketId(0))).unwrap(),
        Event::Fenced { .. }
    ));
    s.apply(&Step::Restart(n)).unwrap();
    assert!(matches!(
        s.apply(&Step::Ack(TicketId(1))).unwrap(),
        Event::Fenced { .. }
    ));
    s.apply(&crash).unwrap();
    s.apply(&Step::Restart(n)).unwrap();
    assert_eq!(s.epoch(n), Some(Epoch(2)));
}

/// `pr15-impl03-neg-05`. The regression for cr-35ujnx: the exploration's pruning key
/// must hold everything the candidate generator reads. `Submit; Crash(keep 0);
/// Restart` and `Crash; Restart` leave the same logs, epochs, tickets and crash count,
/// but a different number of submits, so a different next value and a different
/// remaining submit budget. Their keys must differ, and so do their candidates.
#[test]
fn differential_key_separates_paths_whose_candidates_differ() {
    let cfg = full(1, 2, 2);
    let n = NodeId(0);
    let crash = Step::Crash {
        node: n,
        keep: 0,
        torn: false,
    };
    let walk = |steps: &[Step]| {
        let mut model = Model::new(cfg, Mutant::None);
        for step in steps {
            model.step(step).unwrap();
        }
        model
    };
    let wrote = walk(&[
        Step::Submit {
            node: n,
            value: Value(0),
        },
        crash,
        Step::Restart(n),
    ]);
    let idle = walk(&[crash, Step::Restart(n)]);
    // The state the old key held is the same on both paths.
    assert_eq!(
        (
            &wrote.state.nodes,
            &wrote.state.logs,
            &wrote.state.stable,
            &wrote.state.tickets,
            wrote.state.crashes
        ),
        (
            &idle.state.nodes,
            &idle.state.logs,
            &idle.state.stable,
            &idle.state.tickets,
            idle.state.crashes
        )
    );
    assert_ne!(candidates(&wrote), candidates(&idle));
    assert_ne!(
        wrote.key(),
        idle.key(),
        "two paths with different successors share a key"
    );
    // And generally: over the whole reachable space of every configuration, two
    // states with one key have the same candidates, and every candidate has the same
    // outcome, event and successor key from both. A transition, refusal or candidate
    // that read anything the key omits would break this.
    for cfg in differential_configs() {
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

/// `pr15-impl03-neg-06`. The Lab handler's whole state is in the differential's view,
/// which is compared with the model's state after every accepted step, so the Lab's
/// successors are a function of the pruning key as well (cr-35ujnx). This test pins
/// the Lab's fields: a new field fails it until the view covers it. `events` is covered
/// by its length in the view and by the journal comparison at every visited state.
#[test]
fn differential_view_covers_every_field_of_the_lab_handler() {
    let debug = format!("{:?}", Storage::new(full(1, 1, 1)));
    let fields: Vec<&str> = debug
        .split([' ', '{', ',', '('])
        .filter_map(|word| word.strip_suffix(':'))
        .filter(|word| word.chars().all(|c| c.is_ascii_lowercase() || c == '_'))
        .collect();
    let top: Vec<&str> = fields
        .into_iter()
        .filter(|f| {
            [
                "config", "up", "epochs", "logs", "stable", "tickets", "pending", "crashes",
                "events", "retained",
            ]
            .contains(f)
        })
        .collect();
    assert_eq!(
        top,
        [
            "config", "up", "epochs", "logs", "stable", "tickets", "pending", "crashes", "events",
            "retained"
        ],
        "the Lab handler's fields changed; cover the new one in the differential's view"
    );
    let known = [
        "config",
        "up",
        "epochs",
        "logs",
        "stable",
        "tickets",
        "pending",
        "crashes",
        "events",
        "retained",
        "nodes",
        "max_crashes",
        "restart",
        "suffix_loss",
        "torn",
        "max_pending",
        "max_steps",
        "max_retained_bytes",
    ];
    for field in debug
        .split([' ', '{', ',', '('])
        .filter_map(|word| word.strip_suffix(':'))
        .filter(|word| word.chars().all(|c| c.is_ascii_lowercase() || c == '_'))
    {
        assert!(
            known.contains(&field),
            "unknown field {field} in the Lab handler's state"
        );
    }
}
