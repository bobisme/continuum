//! The Lab handler: deterministic append logs whose every nondeterministic choice is an
//! explicit [`Step`], and whose every accepted step is one journalled [`Event`].
//!
//! # State
//!
//! Each configured node has one append log and one process slot. The slot is up or
//! down, and it has the epoch of its current incarnation, or of its last one while it
//! is down; all nodes start up at epoch 0 with an empty log. A log is a list of
//! entries, each [`Entry::Intact`] or [`Entry::Torn`], and a torn entry can only be the
//! last. The node's stable length says how many of the first entries are stable; they
//! are all intact. The intact entries past it are the volatile suffix. Every sync
//! ticket ever begun is recorded with its node, the epoch that began it, the log length
//! it covers, and whether its flush finished, forever, so a completion can always be
//! judged against the incarnation it belongs to. The pending set holds the tickets not
//! yet resolved.
//!
//! # Determinism (INV-005)
//!
//! Nothing here reads a clock, an entropy source, the environment, or a hash seed: every
//! collection is a `BTreeSet`, a `Vec` or a fixed array, and every transition is a
//! function of the state and the step. So one configuration and one choice log give one
//! journal, byte for byte ([`Journal::encode`]). Crash windows are exactly the
//! boundaries between steps: no step has a partial effect, and a refused step has none.
//!
//! # Refusal precedence
//!
//! A step is checked in this order, and the first failure is the refusal:
//!
//! 0. a journal already as long as the configuration's step bound (at most
//!    [`MAX_STEPS`]): `BoundReached`, so every journal this handler produces replays
//!    under [`run`];
//! 1. a step the profile does not model at all: `Unsupported`;
//! 2. `Submit`: an unknown node (`Malformed`); a down node (`NotEnabled`); a torn tail
//!    (`ProgramFault`); an exhausted position space (`BoundReached`);
//! 3. `Sync`: an unknown node (`Malformed`); a down node (`NotEnabled`); a torn tail
//!    (`ProgramFault`); a full pending bound, then an exhausted ticket ordinal space
//!    (`BoundReached`);
//! 4. `Persist`: a ticket that is not pending, then one whose flush already finished,
//!    then one whose incarnation crashed (`NotEnabled`);
//! 5. `Ack`: a ticket that is not pending (`NotEnabled`). A pending ticket whose
//!    incarnation crashed is accepted as `Fenced`. One whose incarnation still runs is
//!    `NotEnabled` until its flush finished, and `Acked` after. `Delay`: a ticket that is
//!    not pending (`NotEnabled`); a pending one is accepted and changes nothing;
//! 6. `Crash`: an unknown node (`Malformed`); no crash declared, then the budget spent,
//!    then a node already down (`NotEnabled`); a `keep` above the volatile count, then a
//!    torn crash that loses nothing (`Malformed`); a loss with suffix loss undeclared,
//!    then a tear with torn records undeclared (`NotEnabled`);
//! 7. `Restart`: an unknown node (`Malformed`); no restart declared, then a node that
//!    is up (`NotEnabled`); an exhausted epoch space (`BoundReached`);
//! 8. `Truncate`: an unknown node (`Malformed`); a down node, then a log with no torn
//!    tail (`NotEnabled`);
//! 9. after every check above passes, the retained-bytes charge: the event's encoded
//!    size would take the journal past the configuration's budget (`BoundReached`).
//!    The charge is made before anything is pushed.
//!
//! The position, ticket-ordinal and epoch-space refusals cannot fire under the
//! configuration caps: a run has at most [`MAX_STEPS`] steps, far below `u32::MAX`,
//! and a node's epoch is at most its crash count, at most `MAX_CRASHES_CAP`. They are
//! kept so that an arithmetic limit is a typed refusal, never a wrap, and they have no
//! test.

use alloc::collections::BTreeSet;
use alloc::vec::Vec;

use crate::profile::{APPEND_LOG_V0, Semantic, put_str, u32_len};
use crate::refusal::{Bound, Malformed, NotEnabled, ProgramFault, Refusal, RunRefusal};
use crate::step::{
    Entry, Epoch, Event, MAX_STEPS, NodeId, NodeSet, Step, StorageConfig, TicketId, Value,
};

/// One epoch slot per possible node: [`crate::step::MAX_NODES`].
const SLOTS: usize = 64;

/// The journal encoding's magic.
const JOURNAL_MAGIC: &[u8] = b"continuum-storage-journal\0";

/// The size of the journal encoding's header: magic, profile name and version, the
/// configuration, and the event count. It is the least any journal retains.
#[allow(clippy::cast_possible_truncation, clippy::as_conversions)]
pub const JOURNAL_HEADER_BYTES: u64 =
    (JOURNAL_MAGIC.len() + 4 + crate::profile::PROFILE_NAME.len() + 6 + 24 + 4) as u64;

/// The encoded size of a `Submitted` event: tag, node, epoch, index, value.
const SUBMITTED_BYTES: u64 = 14;

/// The encoded size of a ticket event: tag, ticket, node, epoch, upto.
const TICKET_EVENT_BYTES: u64 = 14;

/// The encoded size of a `Crashed` event: tag, node, epoch, keep, torn, lost.
const CRASHED_BYTES: u64 = 15;

/// The encoded size of a `Restarted` event: tag, node, epoch.
const RESTARTED_BYTES: u64 = 6;

/// The encoded size of a `Truncated` event: tag, node, epoch, index.
const TRUNCATED_BYTES: u64 = 10;

/// One ticket's facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Ticket {
    node: NodeId,
    epoch: Epoch,
    upto: u32,
    stable: bool,
}

/// The Lab append logs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Storage {
    config: StorageConfig,
    up: NodeSet,
    epochs: [Epoch; SLOTS],
    logs: Vec<Vec<Entry>>,
    stable: Vec<u32>,
    tickets: Vec<Ticket>,
    pending: BTreeSet<TicketId>,
    crashes: u32,
    events: Vec<Event>,
    /// The journal's canonical encoding size so far, header included; never above
    /// the configuration's retained-bytes budget.
    retained: u64,
}

impl Storage {
    /// Every node up in its first incarnation, epoch 0, with an empty log; nothing
    /// synced, nothing crashed.
    #[must_use]
    pub fn new(config: StorageConfig) -> Self {
        let nodes = usize::from(config.nodes());
        Self {
            config,
            up: NodeSet::all(config.nodes()),
            epochs: [Epoch(0); SLOTS],
            logs: alloc::vec![Vec::new(); nodes],
            stable: alloc::vec![0; nodes],
            tickets: Vec::new(),
            pending: BTreeSet::new(),
            crashes: 0,
            events: Vec::new(),
            retained: JOURNAL_HEADER_BYTES,
        }
    }

    /// The configuration.
    #[must_use]
    pub const fn config(&self) -> &StorageConfig {
        &self.config
    }

    /// The events journalled so far.
    #[must_use]
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    /// The journal so far, as canonical bytes. Its length is [`Self::retained_bytes`].
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        encode(&self.config, &self.events, self.retained)
    }

    /// The journal's canonical encoding size so far, header included. It never exceeds
    /// the configuration's retained-bytes budget.
    #[must_use]
    pub const fn retained_bytes(&self) -> u64 {
        self.retained
    }

    /// The journal, consuming the state, without copying the history.
    #[must_use]
    pub fn into_journal(self) -> Journal {
        Journal {
            config: self.config,
            events: self.events,
            retained: self.retained,
        }
    }

    /// Whether the node has a running incarnation. An unknown node is not up.
    #[must_use]
    pub const fn is_up(&self, node: NodeId) -> bool {
        node.0 < self.config.nodes() && self.up.contains(node)
    }

    /// The set of nodes that are up.
    #[must_use]
    pub const fn up(&self) -> NodeSet {
        self.up
    }

    /// The epoch of the node's current incarnation, or of its last one while it is
    /// down; `None` for an unknown node.
    #[must_use]
    pub fn epoch(&self, node: NodeId) -> Option<Epoch> {
        if node.0 < self.config.nodes() {
            self.epochs.get(usize::from(node.0)).copied()
        } else {
            None
        }
    }

    /// The node's log as a reader sees it: every entry that exists now, stable or
    /// volatile, intact or torn. `None` for an unknown node.
    #[must_use]
    pub fn log(&self, node: NodeId) -> Option<&[Entry]> {
        self.logs.get(usize::from(node.0)).map(Vec::as_slice)
    }

    /// How many of the node's first log entries are stable. `None` for an unknown node.
    #[must_use]
    pub fn stable_len(&self, node: NodeId) -> Option<u32> {
        self.stable.get(usize::from(node.0)).copied()
    }

    /// How many crashes this run has used.
    #[must_use]
    pub const fn crashes(&self) -> u32 {
        self.crashes
    }

    /// The number of sync tickets begun so far.
    #[must_use]
    pub fn ticket_count(&self) -> usize {
        self.tickets.len()
    }

    /// A begun ticket's node, the epoch that began it, the log length it covers, and
    /// whether its flush finished.
    #[must_use]
    pub fn ticket(&self, ticket: TicketId) -> Option<(NodeId, Epoch, u32, bool)> {
        let facts = self.tickets.get(usize::try_from(ticket.0).ok()?)?;
        Some((facts.node, facts.epoch, facts.upto, facts.stable))
    }

    /// Every pending ticket, in ordinal order.
    pub fn pending(&self) -> impl Iterator<Item = TicketId> + '_ {
        self.pending.iter().copied()
    }

    /// The scheduler's and adversary's enabled choices in this state, in canonical
    /// order: `Persist`, `Ack` and `Delay` for each pending ticket in ordinal order;
    /// then, for each node in order, every enabled `Crash`, by `keep` from 0 up and
    /// untorn before torn, and `Restart` if enabled.
    ///
    /// `Submit`, `Sync` and `Truncate` are the program's, not choices, so they are not
    /// listed. The list holds up to two crash outcomes per volatile record, so it is
    /// bounded by [`MAX_STEPS`]; a node none of whose crash outcomes is enabled is
    /// skipped after one check, because keeping every volatile record is enabled
    /// whenever any outcome is.
    #[must_use]
    pub fn enabled_choices(&self) -> Vec<Step> {
        let mut out = Vec::new();
        for ticket in &self.pending {
            for step in [
                Step::Persist(*ticket),
                Step::Ack(*ticket),
                Step::Delay(*ticket),
            ] {
                if self.check(&step).is_ok() {
                    out.push(step);
                }
            }
        }
        for node in 0..self.config.nodes() {
            let node = NodeId(node);
            let volatile = if self.up.contains(node) {
                self.volatile(node)
            } else {
                0
            };
            let keep_all = Step::Crash {
                node,
                keep: volatile,
                torn: false,
            };
            let any_crash = self.check(&keep_all).is_ok();
            for keep in (0..=volatile).take_while(|_| any_crash) {
                for torn in [false, true] {
                    let step = Step::Crash { node, keep, torn };
                    if self.check(&step).is_ok() {
                        out.push(step);
                    }
                }
            }
            let step = Step::Restart(node);
            if self.check(&step).is_ok() {
                out.push(step);
            }
        }
        out
    }

    /// Whether `step` would be accepted now, and if not, why. Changes nothing.
    ///
    /// # Errors
    ///
    /// The [`Refusal`] that [`Self::apply`] would return.
    pub fn check(&self, step: &Step) -> Result<(), Refusal> {
        self.admit(step).map(|_| ())
    }

    /// Apply one step. On success the step's one event is journalled and returned; on
    /// refusal nothing changes.
    ///
    /// # Errors
    ///
    /// A [`Refusal`], in the precedence the module documentation states.
    pub fn apply(&mut self, step: &Step) -> Result<&Event, Refusal> {
        let event = self.admit(step)?;
        let cost = event_bytes(&event);
        match event {
            Event::Submitted { node, value, .. } => {
                if let Some(log) = self.logs.get_mut(usize::from(node.0)) {
                    log.push(Entry::Intact(value));
                }
            }
            Event::SyncBegun {
                ticket,
                node,
                epoch,
                upto,
            } => {
                self.tickets.push(Ticket {
                    node,
                    epoch,
                    upto,
                    stable: false,
                });
                self.pending.insert(ticket);
            }
            Event::Stable {
                ticket, node, upto, ..
            } => {
                if let Some(facts) = usize::try_from(ticket.0)
                    .ok()
                    .and_then(|at| self.tickets.get_mut(at))
                {
                    facts.stable = true;
                }
                if let Some(stable) = self.stable.get_mut(usize::from(node.0)) {
                    *stable = (*stable).max(upto);
                }
            }
            Event::Acked { ticket, .. } | Event::Fenced { ticket, .. } => {
                self.pending.remove(&ticket);
            }
            Event::Delayed { .. } => {}
            Event::Crashed {
                node, keep, torn, ..
            } => {
                let at = usize::from(node.0);
                let torn_tail = self.torn_tail(node);
                if let (Some(log), Some(stable)) = (self.logs.get_mut(at), self.stable.get_mut(at))
                {
                    // A torn tail can only exist with no volatile record, so `admit`
                    // allowed only keep 0 and no tear: the log is left as it is.
                    if !torn_tail {
                        let survivors = *stable + keep;
                        log.truncate(usize::try_from(survivors).unwrap_or(usize::MAX));
                        if torn {
                            log.push(Entry::Torn);
                        }
                        *stable = survivors;
                    }
                }
                self.up = self.up.without(node);
                self.crashes += 1;
            }
            Event::Restarted { node, epoch } => {
                if let Some(slot) = self.epochs.get_mut(usize::from(node.0)) {
                    *slot = epoch;
                }
                self.up = self.up.with(node);
            }
            Event::Truncated { node, .. } => {
                if let Some(log) = self.logs.get_mut(usize::from(node.0)) {
                    log.pop();
                }
            }
        }
        // `admit` charged this cost against the budget before any state changed; the
        // same number is added here, so `encode().len()` equals `retained`.
        self.retained += cost;
        let index = self.events.len();
        self.events.push(event);
        Ok(&self.events[index])
    }

    fn known(&self, node: NodeId) -> Result<(), Refusal> {
        if node.0 < self.config.nodes() {
            Ok(())
        } else {
            Err(Refusal::Malformed(Malformed::UnknownNode(node)))
        }
    }

    /// The node's current epoch. Only called on a known node.
    fn current(&self, node: NodeId) -> Epoch {
        self.epochs
            .get(usize::from(node.0))
            .copied()
            .unwrap_or(Epoch(0))
    }

    /// The node's log, empty for an unknown node. Only called on a known node.
    fn entries(&self, node: NodeId) -> &[Entry] {
        self.logs
            .get(usize::from(node.0))
            .map_or(&[], Vec::as_slice)
    }

    /// Whether the node's log ends in a torn record.
    fn torn_tail(&self, node: NodeId) -> bool {
        self.entries(node).last() == Some(&Entry::Torn)
    }

    /// The node's volatile record count: intact entries past the stable length. A
    /// torn tail is not counted: it is never volatile, and no record follows it.
    fn volatile(&self, node: NodeId) -> u32 {
        let entries = self.entries(node);
        let intact = entries.len() - usize::from(self.torn_tail(node));
        let intact = u32::try_from(intact).unwrap_or(u32::MAX);
        intact.saturating_sub(self.stable_len(node).unwrap_or(0))
    }

    /// A pending ticket's facts, or `NotPending`.
    fn pending_facts(&self, ticket: TicketId) -> Result<Ticket, Refusal> {
        if !self.pending.contains(&ticket) {
            return Err(Refusal::NotEnabled(NotEnabled::NotPending(ticket)));
        }
        usize::try_from(ticket.0)
            .ok()
            .and_then(|index| self.tickets.get(index))
            .copied()
            .ok_or(Refusal::NotEnabled(NotEnabled::NotPending(ticket)))
    }

    /// Whether the incarnation that began a ticket still runs: the process pack's
    /// fence, by node and epoch.
    fn live(&self, facts: &Ticket) -> bool {
        self.up.contains(facts.node) && self.current(facts.node) == facts.epoch
    }

    /// A running node with an intact tail, as `Submit` and `Sync` need. A write over a
    /// torn tail is the program's fault, not an invalid choice.
    fn writable(&self, node: NodeId) -> Result<(), Refusal> {
        self.known(node)?;
        if !self.up.contains(node) {
            return Err(Refusal::NotEnabled(NotEnabled::NodeDown(node)));
        }
        if self.torn_tail(node) {
            return Err(Refusal::ProgramFault(ProgramFault::WriteOverTornTail(node)));
        }
        Ok(())
    }

    /// Admit a step: every check of [`Self::admit_step`], then the retained-bytes
    /// charge, made before `apply` pushes anything. The result is the event the step
    /// will journal.
    fn admit(&self, step: &Step) -> Result<Event, Refusal> {
        let max_steps = steps_bound(&self.config);
        if self.events.len() >= max_steps {
            return Err(Refusal::BoundReached(Bound::Steps { max: max_steps }));
        }
        let event = self.admit_step(step)?;
        let max = self.config.max_retained_bytes();
        if self
            .retained
            .checked_add(event_bytes(&event))
            .is_none_or(|total| total > max)
        {
            return Err(Refusal::BoundReached(Bound::Retained { max }));
        }
        Ok(event)
    }

    fn admit_step(&self, step: &Step) -> Result<Event, Refusal> {
        match step {
            Step::Submit { node, value } => {
                self.writable(*node)?;
                let index = u32::try_from(self.entries(*node).len())
                    .map_err(|_| Refusal::BoundReached(Bound::Positions))?;
                Ok(Event::Submitted {
                    node: *node,
                    epoch: self.current(*node),
                    index,
                    value: *value,
                })
            }
            Step::Sync(node) => {
                self.writable(*node)?;
                if u32::try_from(self.pending.len())
                    .map_or(true, |pending| pending >= self.config.max_pending())
                {
                    return Err(Refusal::BoundReached(Bound::Pending {
                        max: self.config.max_pending(),
                    }));
                }
                let ticket = u32::try_from(self.tickets.len())
                    .map_err(|_| Refusal::BoundReached(Bound::Tickets))?;
                let upto = u32::try_from(self.entries(*node).len())
                    .map_err(|_| Refusal::BoundReached(Bound::Positions))?;
                Ok(Event::SyncBegun {
                    ticket: TicketId(ticket),
                    node: *node,
                    epoch: self.current(*node),
                    upto,
                })
            }
            Step::Persist(ticket) => {
                let facts = self.pending_facts(*ticket)?;
                if facts.stable {
                    return Err(Refusal::NotEnabled(NotEnabled::AlreadyStable(*ticket)));
                }
                if !self.live(&facts) {
                    return Err(Refusal::NotEnabled(NotEnabled::IncarnationCrashed(*ticket)));
                }
                Ok(Event::Stable {
                    ticket: *ticket,
                    node: facts.node,
                    epoch: facts.epoch,
                    upto: facts.upto,
                })
            }
            Step::Ack(ticket) => {
                let facts = self.pending_facts(*ticket)?;
                let (ticket, node, epoch, upto) = (*ticket, facts.node, facts.epoch, facts.upto);
                if !self.live(&facts) {
                    return Ok(Event::Fenced {
                        ticket,
                        node,
                        epoch,
                        upto,
                    });
                }
                if !facts.stable {
                    return Err(Refusal::NotEnabled(NotEnabled::NotStable(ticket)));
                }
                Ok(Event::Acked {
                    ticket,
                    node,
                    epoch,
                    upto,
                })
            }
            Step::Delay(ticket) => {
                let facts = self.pending_facts(*ticket)?;
                Ok(Event::Delayed {
                    ticket: *ticket,
                    node: facts.node,
                    epoch: facts.epoch,
                    upto: facts.upto,
                })
            }
            Step::Crash { node, keep, torn } => self.admit_crash(*node, *keep, *torn),
            Step::Restart(node) => {
                self.known(*node)?;
                if !self.config.restart() {
                    return Err(Refusal::NotEnabled(NotEnabled::FaultNotDeclared(
                        Semantic::CrashRestart,
                    )));
                }
                if self.up.contains(*node) {
                    return Err(Refusal::NotEnabled(NotEnabled::NodeUp(*node)));
                }
                let next = self
                    .current(*node)
                    .0
                    .checked_add(1)
                    .ok_or(Refusal::BoundReached(Bound::Epochs))?;
                Ok(Event::Restarted {
                    node: *node,
                    epoch: Epoch(next),
                })
            }
            Step::Truncate(node) => {
                self.known(*node)?;
                if !self.up.contains(*node) {
                    return Err(Refusal::NotEnabled(NotEnabled::NodeDown(*node)));
                }
                if !self.torn_tail(*node) {
                    return Err(Refusal::NotEnabled(NotEnabled::NoTornTail(*node)));
                }
                let index = u32::try_from(self.entries(*node).len() - 1)
                    .map_err(|_| Refusal::BoundReached(Bound::Positions))?;
                Ok(Event::Truncated {
                    node: *node,
                    epoch: self.current(*node),
                    index,
                })
            }
            Step::Corrupt { .. } => Err(Refusal::Unsupported(Semantic::SectorCorruption)),
            Step::Reorder(_) => Err(Refusal::Unsupported(Semantic::WriteReordering)),
            Step::FalseFlush(_) => Err(Refusal::Unsupported(Semantic::FlushDishonesty)),
        }
    }

    fn admit_crash(&self, node: NodeId, keep: u32, torn: bool) -> Result<Event, Refusal> {
        self.known(node)?;
        if self.config.max_crashes() == 0 {
            return Err(Refusal::NotEnabled(NotEnabled::FaultNotDeclared(
                Semantic::CrashRestart,
            )));
        }
        if self.crashes >= self.config.max_crashes() {
            return Err(Refusal::NotEnabled(NotEnabled::CrashBudgetSpent {
                max: self.config.max_crashes(),
            }));
        }
        if !self.up.contains(node) {
            return Err(Refusal::NotEnabled(NotEnabled::NodeDown(node)));
        }
        let volatile = self.volatile(node);
        if keep > volatile {
            return Err(Refusal::Malformed(Malformed::KeepBeyondVolatile {
                keep,
                volatile,
            }));
        }
        if torn && keep == volatile {
            return Err(Refusal::Malformed(Malformed::NothingToTear));
        }
        if keep < volatile && !self.config.suffix_loss() {
            return Err(Refusal::NotEnabled(NotEnabled::FaultNotDeclared(
                Semantic::VolatileSuffixLoss,
            )));
        }
        if torn && !self.config.torn() {
            return Err(Refusal::NotEnabled(NotEnabled::FaultNotDeclared(
                Semantic::TornWrites,
            )));
        }
        Ok(Event::Crashed {
            node,
            epoch: self.current(node),
            keep,
            torn,
            lost: volatile - keep,
        })
    }
}

/// The configuration's step bound as a length. It is at most [`MAX_STEPS`], so it
/// always fits.
fn steps_bound(config: &StorageConfig) -> usize {
    usize::try_from(config.max_steps()).unwrap_or(MAX_STEPS)
}

/// The encoded size of an event.
const fn event_bytes(event: &Event) -> u64 {
    match event {
        Event::Submitted { .. } => SUBMITTED_BYTES,
        Event::SyncBegun { .. }
        | Event::Stable { .. }
        | Event::Acked { .. }
        | Event::Fenced { .. }
        | Event::Delayed { .. } => TICKET_EVENT_BYTES,
        Event::Crashed { .. } => CRASHED_BYTES,
        Event::Restarted { .. } => RESTARTED_BYTES,
        Event::Truncated { .. } => TRUNCATED_BYTES,
    }
}

/// Run a whole choice log from the initial state.
///
/// # Errors
///
/// [`RunRefusal`] naming the first refused step. A log longer than the configuration's
/// step bound (at most [`MAX_STEPS`]) is refused before any step runs.
pub fn run(config: StorageConfig, steps: &[Step]) -> Result<Journal, RunRefusal> {
    let max_steps = steps_bound(&config);
    if steps.len() > max_steps {
        return Err(RunRefusal {
            index: steps.len(),
            refusal: Refusal::BoundReached(Bound::Steps { max: max_steps }),
        });
    }
    let mut storage = Storage::new(config);
    for (index, step) in steps.iter().enumerate() {
        storage
            .apply(step)
            .map_err(|refusal| RunRefusal { index, refusal })?;
    }
    Ok(storage.into_journal())
}

/// A run's journal: its configuration and its events, one per accepted step.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Journal {
    config: StorageConfig,
    events: Vec<Event>,
    retained: u64,
}

impl Journal {
    /// The configuration the run used.
    #[must_use]
    pub const fn config(&self) -> &StorageConfig {
        &self.config
    }

    /// The events.
    #[must_use]
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    /// The canonical encoding's size, header included; within the configuration's
    /// retained-bytes budget.
    #[must_use]
    pub const fn retained_bytes(&self) -> u64 {
        self.retained
    }

    /// The choice log this journal records: each event's step, in order. Lazy: each
    /// step is built when it is read.
    pub fn choice_log(&self) -> impl Iterator<Item = Step> + '_ {
        self.events.iter().map(Event::step)
    }

    /// Run [`Self::choice_log`] again under [`Self::config`], one step at a time. For a
    /// journal this crate produced, the result equals `self`: it was produced under the
    /// same step bound and retained-bytes budget.
    ///
    /// # Errors
    ///
    /// [`RunRefusal`] if the recorded log is not a valid run, which a journal this crate
    /// produced never is.
    pub fn replay(&self) -> Result<Self, RunRefusal> {
        let mut storage = Storage::new(self.config);
        for (index, step) in self.choice_log().enumerate() {
            storage
                .apply(&step)
                .map_err(|refusal| RunRefusal { index, refusal })?;
        }
        Ok(storage.into_journal())
    }

    /// The canonical bytes: a magic, the profile name and version, the configuration,
    /// and every event. Integers are big-endian. Written once, into a buffer of exactly
    /// [`Self::retained_bytes`].
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        encode(&self.config, &self.events, self.retained)
    }
}

fn put_ticket(out: &mut Vec<u8>, tag: u8, ticket: TicketId, node: NodeId, epoch: Epoch, upto: u32) {
    out.push(tag);
    out.extend_from_slice(&ticket.0.to_be_bytes());
    out.push(node.0);
    out.extend_from_slice(&epoch.0.to_be_bytes());
    out.extend_from_slice(&upto.to_be_bytes());
}

fn encode(config: &StorageConfig, events: &[Event], retained: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(usize::try_from(retained).unwrap_or(0));
    out.extend_from_slice(JOURNAL_MAGIC);
    put_str(&mut out, APPEND_LOG_V0.name);
    out.extend_from_slice(&APPEND_LOG_V0.version.major.to_be_bytes());
    out.extend_from_slice(&APPEND_LOG_V0.version.minor.to_be_bytes());
    out.extend_from_slice(&APPEND_LOG_V0.version.patch.to_be_bytes());
    out.push(config.nodes());
    out.extend_from_slice(&config.max_crashes().to_be_bytes());
    out.push(u8::from(config.restart()));
    out.push(u8::from(config.suffix_loss()));
    out.push(u8::from(config.torn()));
    out.extend_from_slice(&config.max_pending().to_be_bytes());
    out.extend_from_slice(&config.max_steps().to_be_bytes());
    out.extend_from_slice(&config.max_retained_bytes().to_be_bytes());
    out.extend_from_slice(&u32_len(events.len()).to_be_bytes());
    for event in events {
        match *event {
            Event::Submitted {
                node,
                epoch,
                index,
                value: Value(value),
            } => {
                out.push(1);
                out.push(node.0);
                out.extend_from_slice(&epoch.0.to_be_bytes());
                out.extend_from_slice(&index.to_be_bytes());
                out.extend_from_slice(&value.to_be_bytes());
            }
            Event::SyncBegun {
                ticket,
                node,
                epoch,
                upto,
            } => put_ticket(&mut out, 2, ticket, node, epoch, upto),
            Event::Stable {
                ticket,
                node,
                epoch,
                upto,
            } => put_ticket(&mut out, 3, ticket, node, epoch, upto),
            Event::Acked {
                ticket,
                node,
                epoch,
                upto,
            } => put_ticket(&mut out, 4, ticket, node, epoch, upto),
            Event::Fenced {
                ticket,
                node,
                epoch,
                upto,
            } => put_ticket(&mut out, 5, ticket, node, epoch, upto),
            Event::Delayed {
                ticket,
                node,
                epoch,
                upto,
            } => put_ticket(&mut out, 6, ticket, node, epoch, upto),
            Event::Crashed {
                node,
                epoch,
                keep,
                torn,
                lost,
            } => {
                out.push(7);
                out.push(node.0);
                out.extend_from_slice(&epoch.0.to_be_bytes());
                out.extend_from_slice(&keep.to_be_bytes());
                out.push(u8::from(torn));
                out.extend_from_slice(&lost.to_be_bytes());
            }
            Event::Restarted { node, epoch } => {
                out.push(8);
                out.push(node.0);
                out.extend_from_slice(&epoch.0.to_be_bytes());
            }
            Event::Truncated { node, epoch, index } => {
                out.push(9);
                out.push(node.0);
                out.extend_from_slice(&epoch.0.to_be_bytes());
                out.extend_from_slice(&index.to_be_bytes());
            }
        }
    }
    out
}
