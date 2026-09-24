//! The Lab handler: deterministic process lifecycles whose every nondeterministic choice
//! is an explicit [`Step`], and whose every accepted step is one journalled [`Event`].
//!
//! # State
//!
//! Each configured node is one process slot. It is up or down, and it has the epoch of
//! its current incarnation, or of its last one while it is down. All nodes start up at
//! epoch 0. Every ticket ever begun is recorded with its node and the epoch that began
//! it, forever, so a completion can always be judged against the incarnation it
//! belongs to. The pending set holds the tickets not yet resolved.
//!
//! # Determinism (INV-005)
//!
//! Nothing here reads a clock, an entropy source, the environment, or a hash seed: every
//! collection is a `BTreeSet`, a `Vec` or a fixed array, and every transition is a
//! function of the state and the step. So one configuration and one choice log give one
//! journal, byte for byte ([`Journal::encode`]). Crash windows and cancellation points
//! are exactly the boundaries between steps: no step has a partial effect, and a
//! refused step has none.
//!
//! # Refusal precedence
//!
//! A step is checked in this order, and the first failure is the refusal:
//!
//! 0. a journal already [`MAX_STEPS`] events long: `BoundReached`, so every journal
//!    this handler produces replays under [`run`];
//! 1. a step the profile does not model at all: `Unsupported`;
//! 2. `Begin`: an unknown node (`Malformed`); a down node (`NotEnabled`); a full
//!    pending bound, then an exhausted ticket ordinal space (`BoundReached`);
//! 3. `Complete`: a ticket that is not pending (`NotEnabled`). A pending ticket is then
//!    accepted: `Completed` if the incarnation that began it still runs, `Fenced`
//!    otherwise. `Delay`: a ticket that is not pending (`NotEnabled`); a pending one is
//!    accepted whether or not its incarnation still runs, and changes nothing;
//! 4. `Crash`: an unknown node (`Malformed`); no crash declared, then the budget spent,
//!    then a node already down (`NotEnabled`);
//! 5. `Restart`: an unknown node (`Malformed`); no restart declared, then a node that
//!    is up (`NotEnabled`); an exhausted epoch space (`BoundReached`);
//! 6. after every check above passes, the retained-bytes charge: the event's encoded
//!    size would take the journal past the configuration's budget (`BoundReached`).
//!    The charge is made before anything is pushed.
//!
//! The ticket-ordinal and epoch-space refusals cannot fire under the configuration
//! caps: a run has at most [`MAX_STEPS`] tickets, far below `u32::MAX`, and a node's
//! epoch is at most its crash count, at most `MAX_CRASHES_CAP`. They are kept so that
//! an arithmetic limit is a typed refusal, never a wrap, and they have no test.

use alloc::collections::BTreeSet;
use alloc::vec::Vec;

use crate::profile::{CRASH_RESTART_V0, Semantic, put_str, u32_len};
use crate::refusal::{Bound, Malformed, NotEnabled, Refusal, RunRefusal};
use crate::step::{Epoch, Event, MAX_STEPS, NodeId, NodeSet, ProcessConfig, Step, TicketId};

/// One epoch slot per possible node: [`crate::step::MAX_NODES`].
const SLOTS: usize = 64;

/// The journal encoding's magic.
const JOURNAL_MAGIC: &[u8] = b"continuum-process-journal\0";

/// The size of the journal encoding's header: magic, profile name and version, the
/// configuration, and the event count. It is the least any journal retains.
#[allow(clippy::cast_possible_truncation, clippy::as_conversions)]
pub const JOURNAL_HEADER_BYTES: u64 =
    (JOURNAL_MAGIC.len() + 4 + crate::profile::PROFILE_NAME.len() + 6 + 18 + 4) as u64;

/// The encoded size of a ticket event: tag, ticket, node, epoch.
const TICKET_EVENT_BYTES: u64 = 10;

/// The encoded size of a lifecycle event: tag, node, epoch.
const LIFECYCLE_EVENT_BYTES: u64 = 6;

/// One ticket's fixed facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Ticket {
    node: NodeId,
    epoch: Epoch,
}

/// The Lab process lifecycles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Process {
    config: ProcessConfig,
    up: NodeSet,
    epochs: [Epoch; SLOTS],
    tickets: Vec<Ticket>,
    pending: BTreeSet<TicketId>,
    crashes: u32,
    events: Vec<Event>,
    /// The journal's canonical encoding size so far, header included; never above
    /// the configuration's retained-bytes budget.
    retained: u64,
}

impl Process {
    /// Every node up in its first incarnation, epoch 0; nothing begun, nothing crashed.
    #[must_use]
    pub const fn new(config: ProcessConfig) -> Self {
        Self {
            config,
            up: NodeSet::all(config.nodes()),
            epochs: [Epoch(0); SLOTS],
            tickets: Vec::new(),
            pending: BTreeSet::new(),
            crashes: 0,
            events: Vec::new(),
            retained: JOURNAL_HEADER_BYTES,
        }
    }

    /// The configuration.
    #[must_use]
    pub const fn config(&self) -> &ProcessConfig {
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

    /// How many crashes this run has used.
    #[must_use]
    pub const fn crashes(&self) -> u32 {
        self.crashes
    }

    /// The number of tickets begun so far.
    #[must_use]
    pub fn ticket_count(&self) -> usize {
        self.tickets.len()
    }

    /// A begun ticket's node and the epoch that began it.
    #[must_use]
    pub fn ticket(&self, ticket: TicketId) -> Option<(NodeId, Epoch)> {
        let facts = self.tickets.get(usize::try_from(ticket.0).ok()?)?;
        Some((facts.node, facts.epoch))
    }

    /// Every pending ticket, in ordinal order.
    pub fn pending(&self) -> impl Iterator<Item = TicketId> + '_ {
        self.pending.iter().copied()
    }

    /// The scheduler's and adversary's enabled choices in this state, in canonical
    /// order: `Complete` and `Delay` for each pending ticket in ordinal order; then, for
    /// each node in order, `Crash` and `Restart` as each is enabled.
    ///
    /// `Begin` is the program's, not a choice, so it is not listed.
    #[must_use]
    pub fn enabled_choices(&self) -> Vec<Step> {
        let mut out = Vec::new();
        for ticket in &self.pending {
            for step in [Step::Complete(*ticket), Step::Delay(*ticket)] {
                if self.check(&step).is_ok() {
                    out.push(step);
                }
            }
        }
        for node in 0..self.config.nodes() {
            for step in [Step::Crash(NodeId(node)), Step::Restart(NodeId(node))] {
                if self.check(&step).is_ok() {
                    out.push(step);
                }
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
            Event::Begun {
                ticket,
                node,
                epoch,
            } => {
                self.tickets.push(Ticket { node, epoch });
                self.pending.insert(ticket);
            }
            Event::Completed { ticket, .. } | Event::Fenced { ticket, .. } => {
                self.pending.remove(&ticket);
            }
            Event::Delayed { .. } => {}
            Event::Crashed { node, .. } => {
                self.up = self.up.without(node);
                self.crashes += 1;
            }
            Event::Restarted { node, epoch } => {
                if let Some(slot) = self.epochs.get_mut(usize::from(node.0)) {
                    *slot = epoch;
                }
                self.up = self.up.with(node);
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

    /// The node's current epoch. Only called on a known node.
    fn current(&self, node: NodeId) -> Epoch {
        self.epochs
            .get(usize::from(node.0))
            .copied()
            .unwrap_or(Epoch(0))
    }

    /// Admit a step: every check of [`Self::admit_step`], then the retained-bytes
    /// charge, made before `apply` pushes anything. The result is the event the step
    /// will journal.
    fn admit(&self, step: &Step) -> Result<Event, Refusal> {
        if self.events.len() >= MAX_STEPS {
            return Err(Refusal::BoundReached(Bound::Steps { max: MAX_STEPS }));
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
            Step::Begin(node) => {
                self.known(*node)?;
                if !self.up.contains(*node) {
                    return Err(Refusal::NotEnabled(NotEnabled::NodeDown(*node)));
                }
                if u32::try_from(self.pending.len())
                    .map_or(true, |pending| pending >= self.config.max_pending())
                {
                    return Err(Refusal::BoundReached(Bound::Pending {
                        max: self.config.max_pending(),
                    }));
                }
                let ticket = u32::try_from(self.tickets.len())
                    .map_err(|_| Refusal::BoundReached(Bound::Tickets))?;
                Ok(Event::Begun {
                    ticket: TicketId(ticket),
                    node: *node,
                    epoch: self.current(*node),
                })
            }
            Step::Delay(ticket) => {
                let facts = self.pending_facts(*ticket)?;
                Ok(Event::Delayed {
                    ticket: *ticket,
                    node: facts.node,
                    epoch: facts.epoch,
                })
            }
            Step::Complete(ticket) => {
                let facts = self.pending_facts(*ticket)?;
                let live = self.up.contains(facts.node) && self.current(facts.node) == facts.epoch;
                Ok(if live {
                    Event::Completed {
                        ticket: *ticket,
                        node: facts.node,
                        epoch: facts.epoch,
                    }
                } else {
                    Event::Fenced {
                        ticket: *ticket,
                        node: facts.node,
                        epoch: facts.epoch,
                    }
                })
            }
            Step::Crash(node) => {
                self.known(*node)?;
                if self.config.max_crashes() == 0 {
                    return Err(Refusal::NotEnabled(NotEnabled::FaultNotDeclared(
                        Semantic::FailStopCrash,
                    )));
                }
                if self.crashes >= self.config.max_crashes() {
                    return Err(Refusal::NotEnabled(NotEnabled::CrashBudgetSpent {
                        max: self.config.max_crashes(),
                    }));
                }
                if !self.up.contains(*node) {
                    return Err(Refusal::NotEnabled(NotEnabled::NodeDown(*node)));
                }
                Ok(Event::Crashed {
                    node: *node,
                    epoch: self.current(*node),
                })
            }
            Step::Restart(node) => {
                self.known(*node)?;
                if !self.config.restart() {
                    return Err(Refusal::NotEnabled(NotEnabled::FaultNotDeclared(
                        Semantic::RestartNewEpoch,
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
            Step::CancelGracefully(_) => Err(Refusal::Unsupported(Semantic::GracefulCancellation)),
            Step::Panic(_) => Err(Refusal::Unsupported(Semantic::Panic)),
            Step::PowerLoss(_) => Err(Refusal::Unsupported(Semantic::PowerLoss)),
        }
    }
}

/// The encoded size of an event.
const fn event_bytes(event: &Event) -> u64 {
    match event {
        Event::Begun { .. }
        | Event::Completed { .. }
        | Event::Fenced { .. }
        | Event::Delayed { .. } => TICKET_EVENT_BYTES,
        Event::Crashed { .. } | Event::Restarted { .. } => LIFECYCLE_EVENT_BYTES,
    }
}

/// Run a whole choice log from the initial state.
///
/// # Errors
///
/// [`RunRefusal`] naming the first refused step. A log longer than [`MAX_STEPS`] is
/// refused before any step runs.
pub fn run(config: ProcessConfig, steps: &[Step]) -> Result<Journal, RunRefusal> {
    if steps.len() > MAX_STEPS {
        return Err(RunRefusal {
            index: steps.len(),
            refusal: Refusal::BoundReached(Bound::Steps { max: MAX_STEPS }),
        });
    }
    let mut process = Process::new(config);
    for (index, step) in steps.iter().enumerate() {
        process
            .apply(step)
            .map_err(|refusal| RunRefusal { index, refusal })?;
    }
    Ok(process.into_journal())
}

/// A run's journal: its configuration and its events, one per accepted step.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Journal {
    config: ProcessConfig,
    events: Vec<Event>,
    retained: u64,
}

impl Journal {
    /// The configuration the run used.
    #[must_use]
    pub const fn config(&self) -> &ProcessConfig {
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
        let mut process = Process::new(self.config);
        for (index, step) in self.choice_log().enumerate() {
            process
                .apply(&step)
                .map_err(|refusal| RunRefusal { index, refusal })?;
        }
        Ok(process.into_journal())
    }

    /// The canonical bytes: a magic, the profile name and version, the configuration,
    /// and every event. Integers are big-endian. Written once, into a buffer of exactly
    /// [`Self::retained_bytes`].
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        encode(&self.config, &self.events, self.retained)
    }
}

fn encode(config: &ProcessConfig, events: &[Event], retained: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(usize::try_from(retained).unwrap_or(0));
    out.extend_from_slice(JOURNAL_MAGIC);
    put_str(&mut out, CRASH_RESTART_V0.name);
    out.extend_from_slice(&CRASH_RESTART_V0.version.major.to_be_bytes());
    out.extend_from_slice(&CRASH_RESTART_V0.version.minor.to_be_bytes());
    out.extend_from_slice(&CRASH_RESTART_V0.version.patch.to_be_bytes());
    out.push(config.nodes());
    out.extend_from_slice(&config.max_crashes().to_be_bytes());
    out.push(u8::from(config.restart()));
    out.extend_from_slice(&config.max_pending().to_be_bytes());
    out.extend_from_slice(&config.max_retained_bytes().to_be_bytes());
    out.extend_from_slice(&u32_len(events.len()).to_be_bytes());
    for event in events {
        let (tag, ticket, node, epoch) = match event {
            Event::Begun {
                ticket,
                node,
                epoch,
            } => (1, Some(*ticket), *node, *epoch),
            Event::Completed {
                ticket,
                node,
                epoch,
            } => (2, Some(*ticket), *node, *epoch),
            Event::Fenced {
                ticket,
                node,
                epoch,
            } => (3, Some(*ticket), *node, *epoch),
            Event::Delayed {
                ticket,
                node,
                epoch,
            } => (6, Some(*ticket), *node, *epoch),
            Event::Crashed { node, epoch } => (4, None, *node, *epoch),
            Event::Restarted { node, epoch } => (5, None, *node, *epoch),
        };
        out.push(tag);
        if let Some(ticket) = ticket {
            out.extend_from_slice(&ticket.0.to_be_bytes());
        }
        out.push(node.0);
        out.extend_from_slice(&epoch.0.to_be_bytes());
    }
    out
}
