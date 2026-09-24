//! The Lab handler: a deterministic network whose every nondeterministic choice is an
//! explicit [`Step`], and whose every accepted step is one journalled [`Event`].
//!
//! # State (docs/17 §7)
//!
//! Endpoints are the configured node addresses. Every sent envelope is recorded with
//! its sender, receiver and payload, forever, so a delivery can only ever be of a sent
//! envelope (the `network-no-forgery` assumption is structural, not checked after the
//! fact). The in-flight multiset maps each envelope to its copy count, and a per-link
//! index keeps each directed link's in-flight envelopes in send order for the FIFO
//! check. At most one partition is active.
//!
//! # Determinism (INV-005)
//!
//! Nothing here reads a clock, an entropy source, the environment, or a hash seed: every
//! collection is a `BTreeMap`/`BTreeSet`/`Vec`, and every transition is a function of
//! the state and the step. So one configuration and one choice log give one journal,
//! byte for byte ([`Journal::encode`]). Cancellation points are exactly the boundaries
//! between steps: no step has a partial effect, and a refused step has none.
//!
//! # Refusal precedence
//!
//! A step is checked in this order, and the first failure is the refusal:
//!
//! 0. a journal already [`MAX_STEPS`] events long: `BoundReached`, so every journal
//!    this handler produces replays under [`run`];
//! 1. a step the profile does not model at all: `Unsupported`;
//! 2. `Send`: an unknown sender, then an unknown receiver (`Malformed`); a payload
//!    over the byte bound, a full in-flight bound, then an exhausted ordinal space
//!    (`BoundReached`);
//! 3. `Deliver`: not in flight; separated by the partition; behind an older envelope
//!    on a FIFO link;
//! 4. `Drop`, `Duplicate`: the fault is not declared; not in flight; and for
//!    `Duplicate` a full in-flight bound;
//! 5. `Delay`: not in flight;
//! 6. `Partition`: a side outside the nodes, then a side that cuts nothing
//!    (`Malformed`); no partition declared, then the budget spent (`NotEnabled`), so a
//!    configuration that already forbids a second partition says so; then a partition
//!    already active (`Unsupported(overlapping-partitions)`);
//! 7. `Heal`: no active partition.
//! 8. after every check above passes, the retained-bytes charge: the event's encoded
//!    size would take the journal past the configuration's budget (`BoundReached`).
//!    The charge is made before anything is cloned or pushed.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use crate::profile::{ADVERSARIAL_V1, Semantic, put_str, u32_len};
use crate::refusal::{Bound, Malformed, NotEnabled, Refusal, RunRefusal};
use crate::step::{EnvelopeId, Event, MAX_STEPS, NetworkConfig, NodeId, NodeSet, Payload, Step};

/// The journal encoding's magic.
const JOURNAL_MAGIC: &[u8] = b"continuum-network-journal\0";

/// The size of the journal encoding's header: magic, profile name and version, the
/// configuration, and the event count. It is the least any journal retains.
#[allow(clippy::cast_possible_truncation, clippy::as_conversions)]
pub const JOURNAL_HEADER_BYTES: u64 =
    (JOURNAL_MAGIC.len() + 4 + crate::profile::PROFILE_NAME.len() + 6 + 14 + 8 + 4) as u64;

/// One sent envelope's fixed facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Sent {
    src: NodeId,
    dst: NodeId,
    /// The index of its `Sent` event in the journal, which holds the payload once.
    event: usize,
}

/// The Lab network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Network {
    config: NetworkConfig,
    sent: Vec<Sent>,
    copies: BTreeMap<EnvelopeId, u32>,
    links: BTreeMap<(NodeId, NodeId), BTreeSet<EnvelopeId>>,
    in_flight: u32,
    partition: Option<NodeSet>,
    partitions_used: u32,
    events: Vec<Event>,
    /// The journal's canonical encoding size so far, header included; never above
    /// the configuration's retained-bytes budget.
    retained: u64,
}

impl Network {
    /// An empty network: nothing sent, nothing in flight, no partition.
    #[must_use]
    pub const fn new(config: NetworkConfig) -> Self {
        Self {
            config,
            sent: Vec::new(),
            copies: BTreeMap::new(),
            links: BTreeMap::new(),
            in_flight: 0,
            partition: None,
            partitions_used: 0,
            events: Vec::new(),
            retained: JOURNAL_HEADER_BYTES,
        }
    }

    /// The configuration.
    #[must_use]
    pub const fn config(&self) -> &NetworkConfig {
        &self.config
    }

    /// The events journalled so far.
    #[must_use]
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    /// The journal so far, as canonical bytes, written once without copying the
    /// history first. Its length is [`Self::retained_bytes`].
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

    /// The journal, consuming the network, without copying the history.
    #[must_use]
    pub fn into_journal(self) -> Journal {
        Journal {
            config: self.config,
            events: self.events,
            retained: self.retained,
        }
    }

    /// How many copies of `envelope` are in flight.
    #[must_use]
    pub fn copies(&self, envelope: EnvelopeId) -> u32 {
        self.copies.get(&envelope).copied().unwrap_or(0)
    }

    /// Every in-flight envelope with its copy count, in ordinal order.
    pub fn in_flight(&self) -> impl Iterator<Item = (EnvelopeId, u32)> + '_ {
        self.copies
            .iter()
            .map(|(envelope, count)| (*envelope, *count))
    }

    /// The total number of in-flight copies.
    #[must_use]
    pub const fn in_flight_total(&self) -> u32 {
        self.in_flight
    }

    /// The active partition's side, normalized to the side that holds node 0.
    #[must_use]
    pub const fn partition(&self) -> Option<NodeSet> {
        self.partition
    }

    /// How many partitions this run has used.
    #[must_use]
    pub const fn partitions_used(&self) -> u32 {
        self.partitions_used
    }

    /// The number of envelopes sent so far.
    #[must_use]
    pub fn sent_count(&self) -> usize {
        self.sent.len()
    }

    /// A sent envelope's sender, receiver and payload.
    #[must_use]
    pub fn envelope(&self, envelope: EnvelopeId) -> Option<(NodeId, NodeId, &Payload)> {
        let sent = self.sent.get(usize::try_from(envelope.0).ok()?)?;
        match self.events.get(sent.event) {
            Some(Event::Sent { payload, .. }) => Some((sent.src, sent.dst, payload)),
            _ => None,
        }
    }

    /// The scheduler's and adversary's enabled choices in this state, in canonical
    /// order: for each in-flight envelope in ordinal order, `Deliver`, `Drop`,
    /// `Duplicate`, `Delay` as each is enabled; then `Heal` if it is enabled.
    ///
    /// `Partition` is not listed, because its argument ranges over node subsets; ask
    /// [`Self::check`] about a particular side. `Send` is the program's, not a choice.
    #[must_use]
    pub fn enabled_choices(&self) -> Vec<Step> {
        let mut out = Vec::new();
        for envelope in self.copies.keys() {
            for step in [
                Step::Deliver(*envelope),
                Step::Drop(*envelope),
                Step::Duplicate(*envelope),
                Step::Delay(*envelope),
            ] {
                if self.check(&step).is_ok() {
                    out.push(step);
                }
            }
        }
        if self.check(&Step::Heal).is_ok() {
            out.push(Step::Heal);
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
        let admitted = self.admit(step)?;
        let cost = admitted.event_bytes();
        let event = match admitted {
            Admitted::Send {
                src,
                dst,
                envelope,
                payload,
            } => {
                self.sent.push(Sent {
                    src,
                    dst,
                    event: self.events.len(),
                });
                self.add_copy(envelope, src, dst);
                Event::Sent {
                    envelope,
                    src,
                    dst,
                    payload: payload.clone(),
                }
            }
            Admitted::Deliver(envelope, src, dst) => {
                self.remove_copy(envelope, src, dst);
                Event::Delivered { envelope, src, dst }
            }
            Admitted::Drop(envelope, src, dst) => {
                self.remove_copy(envelope, src, dst);
                Event::Dropped(envelope)
            }
            Admitted::Duplicate(envelope, src, dst) => {
                self.add_copy(envelope, src, dst);
                Event::Duplicated(envelope)
            }
            Admitted::Delay(envelope) => Event::Delayed(envelope),
            Admitted::Partition(side) => {
                self.partition = Some(side);
                self.partitions_used += 1;
                Event::Partitioned(side)
            }
            Admitted::Heal => {
                self.partition = None;
                Event::Healed
            }
        };
        // `admit` charged this cost against the budget before any state changed; the
        // same number is added here, so `encode().len()` equals `retained`.
        self.retained += cost;
        let index = self.events.len();
        self.events.push(event);
        Ok(&self.events[index])
    }

    fn add_copy(&mut self, envelope: EnvelopeId, src: NodeId, dst: NodeId) {
        *self.copies.entry(envelope).or_insert(0) += 1;
        self.links.entry((src, dst)).or_default().insert(envelope);
        self.in_flight += 1;
    }

    fn remove_copy(&mut self, envelope: EnvelopeId, src: NodeId, dst: NodeId) {
        // `admit` only admits an in-flight envelope, so the entry exists and is positive.
        let Some(count) = self.copies.get_mut(&envelope) else {
            return;
        };
        *count -= 1;
        if *count == 0 {
            self.copies.remove(&envelope);
            if let Some(link) = self.links.get_mut(&(src, dst)) {
                link.remove(&envelope);
                if link.is_empty() {
                    self.links.remove(&(src, dst));
                }
            }
        }
        self.in_flight -= 1;
    }

    fn known(&self, node: NodeId) -> Result<(), Refusal> {
        if node.0 < self.config.nodes() {
            Ok(())
        } else {
            Err(Refusal::Malformed(Malformed::UnknownNode(node)))
        }
    }

    fn in_flight_facts(&self, envelope: EnvelopeId) -> Result<(NodeId, NodeId), Refusal> {
        if self.copies(envelope) == 0 {
            return Err(Refusal::NotEnabled(NotEnabled::NotInFlight(envelope)));
        }
        let index = usize::try_from(envelope.0)
            .map_err(|_| Refusal::NotEnabled(NotEnabled::NotInFlight(envelope)))?;
        let sent = self
            .sent
            .get(index)
            .ok_or(Refusal::NotEnabled(NotEnabled::NotInFlight(envelope)))?;
        Ok((sent.src, sent.dst))
    }

    fn room(&self) -> Result<(), Refusal> {
        if self.in_flight >= self.config.max_in_flight() {
            Err(Refusal::BoundReached(Bound::InFlight {
                max: self.config.max_in_flight(),
            }))
        } else {
            Ok(())
        }
    }

    fn separated(&self, src: NodeId, dst: NodeId) -> bool {
        self.partition
            .is_some_and(|side| side.contains(src) != side.contains(dst))
    }

    /// Admit a step: every check of [`Self::admit_step`], then the retained-bytes
    /// charge, made before `apply` clones or pushes anything.
    fn admit<'s>(&self, step: &'s Step) -> Result<Admitted<'s>, Refusal> {
        if self.events.len() >= MAX_STEPS {
            return Err(Refusal::BoundReached(Bound::Steps { max: MAX_STEPS }));
        }
        let admitted = self.admit_step(step)?;
        let cost = admitted.event_bytes();
        let max = self.config.max_retained_bytes();
        if self
            .retained
            .checked_add(cost)
            .is_none_or(|total| total > max)
        {
            return Err(Refusal::BoundReached(Bound::Retained { max }));
        }
        Ok(admitted)
    }

    fn admit_step<'s>(&self, step: &'s Step) -> Result<Admitted<'s>, Refusal> {
        let faults = self.config.faults();
        match step {
            Step::Send { src, dst, payload } => {
                self.known(*src)?;
                self.known(*dst)?;
                if u32::try_from(payload.0.len())
                    .map_or(true, |len| len > self.config.max_payload_bytes())
                {
                    return Err(Refusal::BoundReached(Bound::Payload {
                        max: self.config.max_payload_bytes(),
                    }));
                }
                self.room()?;
                let envelope = u32::try_from(self.sent.len())
                    .map_err(|_| Refusal::BoundReached(Bound::Envelopes))?;
                Ok(Admitted::Send {
                    src: *src,
                    dst: *dst,
                    envelope: EnvelopeId(envelope),
                    payload,
                })
            }
            Step::Deliver(envelope) => {
                let (src, dst) = self.in_flight_facts(*envelope)?;
                if self.separated(src, dst) {
                    return Err(Refusal::NotEnabled(NotEnabled::Partitioned(*envelope)));
                }
                if !faults.reordering {
                    let ahead = self
                        .links
                        .get(&(src, dst))
                        .and_then(|link| link.first().copied());
                    if let Some(ahead) = ahead.filter(|ahead| ahead != envelope) {
                        return Err(Refusal::NotEnabled(NotEnabled::WouldReorder {
                            envelope: *envelope,
                            ahead,
                        }));
                    }
                }
                Ok(Admitted::Deliver(*envelope, src, dst))
            }
            Step::Drop(envelope) => {
                if !faults.loss {
                    return Err(Refusal::NotEnabled(NotEnabled::FaultNotDeclared(
                        Semantic::Loss,
                    )));
                }
                let (src, dst) = self.in_flight_facts(*envelope)?;
                Ok(Admitted::Drop(*envelope, src, dst))
            }
            Step::Duplicate(envelope) => {
                if !faults.duplication {
                    return Err(Refusal::NotEnabled(NotEnabled::FaultNotDeclared(
                        Semantic::Duplication,
                    )));
                }
                let (src, dst) = self.in_flight_facts(*envelope)?;
                self.room()?;
                Ok(Admitted::Duplicate(*envelope, src, dst))
            }
            Step::Delay(envelope) => {
                self.in_flight_facts(*envelope)?;
                Ok(Admitted::Delay(*envelope))
            }
            Step::Partition(side) => {
                let nodes = self.config.nodes();
                if !side.bits_valid(nodes) {
                    return Err(Refusal::Malformed(Malformed::SideOutsideNodes(*side)));
                }
                let all = NodeSet::all(nodes);
                let normalized = if side.contains(NodeId(0)) {
                    *side
                } else {
                    NodeSet(all.0 & !side.0)
                };
                if normalized.0 == 0 || normalized == all {
                    return Err(Refusal::Malformed(Malformed::TrivialSide(*side)));
                }
                if self.config.max_partitions() == 0 {
                    return Err(Refusal::NotEnabled(NotEnabled::FaultNotDeclared(
                        Semantic::SymmetricPartition,
                    )));
                }
                if self.partitions_used >= self.config.max_partitions() {
                    return Err(Refusal::NotEnabled(NotEnabled::PartitionBudgetSpent {
                        max: self.config.max_partitions(),
                    }));
                }
                if self.partition.is_some() {
                    return Err(Refusal::Unsupported(Semantic::OverlappingPartitions));
                }
                Ok(Admitted::Partition(normalized))
            }
            Step::Heal => {
                if self.partition.is_none() {
                    return Err(Refusal::NotEnabled(NotEnabled::NoPartition));
                }
                Ok(Admitted::Heal)
            }
            Step::OneWayPartition { .. } => {
                Err(Refusal::Unsupported(Semantic::AsymmetricPartition))
            }
            Step::Corrupt(_) => Err(Refusal::Unsupported(Semantic::Corruption)),
            Step::Forge { .. } => Err(Refusal::Unsupported(Semantic::Forgery)),
            Step::ConnectionReset(..) => Err(Refusal::Unsupported(Semantic::ConnectionReset)),
            Step::CrashEndpoint(_) => Err(Refusal::Unsupported(Semantic::EndpointCrash)),
            Step::Recall(_) => Err(Refusal::Unsupported(Semantic::RecallInFlight)),
        }
    }
}

/// A step that passed every check, with the facts `apply` needs.
enum Admitted<'s> {
    Send {
        src: NodeId,
        dst: NodeId,
        envelope: EnvelopeId,
        payload: &'s Payload,
    },
    Deliver(EnvelopeId, NodeId, NodeId),
    Drop(EnvelopeId, NodeId, NodeId),
    Duplicate(EnvelopeId, NodeId, NodeId),
    Delay(EnvelopeId),
    Partition(NodeSet),
    Heal,
}

impl Admitted<'_> {
    /// The encoded size of the event this step will journal.
    fn event_bytes(&self) -> u64 {
        match self {
            Self::Send { payload, .. } => SENT_FIXED_BYTES + len_u64(payload.0.len()),
            Self::Deliver(..) => 7,
            Self::Drop(..) | Self::Duplicate(..) | Self::Delay(_) => 5,
            Self::Partition(_) => 9,
            Self::Heal => 1,
        }
    }
}

/// A `Sent` event's size without its payload: tag, ordinal, two nodes, length.
const SENT_FIXED_BYTES: u64 = 11;

fn len_u64(len: usize) -> u64 {
    u64::try_from(len).unwrap_or(u64::MAX)
}

/// Run a whole choice log from an empty network.
///
/// # Errors
///
/// [`RunRefusal`] naming the first refused step. A log longer than [`MAX_STEPS`] is
/// refused before any step runs.
pub fn run(config: NetworkConfig, steps: &[Step]) -> Result<Journal, RunRefusal> {
    if steps.len() > MAX_STEPS {
        return Err(RunRefusal {
            index: steps.len(),
            refusal: Refusal::BoundReached(Bound::Steps { max: MAX_STEPS }),
        });
    }
    let mut network = Network::new(config);
    for (index, step) in steps.iter().enumerate() {
        network
            .apply(step)
            .map_err(|refusal| RunRefusal { index, refusal })?;
    }
    Ok(network.into_journal())
}

/// A run's journal: its configuration and its events, one per accepted step.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Journal {
    config: NetworkConfig,
    events: Vec<Event>,
    retained: u64,
}

impl Journal {
    /// The configuration the run used.
    #[must_use]
    pub const fn config(&self) -> &NetworkConfig {
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
    /// step is built when it is read, so the history is not copied up front.
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
        let mut network = Network::new(self.config);
        for (index, step) in self.choice_log().enumerate() {
            network
                .apply(&step)
                .map_err(|refusal| RunRefusal { index, refusal })?;
        }
        Ok(network.into_journal())
    }

    /// The canonical bytes: a magic, the profile name and version, the configuration,
    /// and every event. Integers are big-endian; each payload is length-prefixed and
    /// appears once, in its `Sent` event. Written once, into a buffer of exactly
    /// [`Self::retained_bytes`].
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        encode(&self.config, &self.events, self.retained)
    }
}

/// The journal's canonical bytes. The header names the current profile,
/// [`crate::profile::ADVERSARIAL_V1`]. This handler implements the rows every entry of
/// [`crate::profile::PROFILES`] shares, and the profiles differ only in what they
/// declare about the host, so a journal is equally a run under the frozen `-v0`; a
/// consumer that cites `-v0` reads the rows, not the header's name.
fn encode(config: &NetworkConfig, events: &[Event], retained: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(usize::try_from(retained).unwrap_or(0));
    out.extend_from_slice(JOURNAL_MAGIC);
    put_str(&mut out, ADVERSARIAL_V1.name);
    out.extend_from_slice(&ADVERSARIAL_V1.version.major.to_be_bytes());
    out.extend_from_slice(&ADVERSARIAL_V1.version.minor.to_be_bytes());
    out.extend_from_slice(&ADVERSARIAL_V1.version.patch.to_be_bytes());
    let faults = config.faults();
    out.push(config.nodes());
    out.extend_from_slice(&config.max_in_flight().to_be_bytes());
    out.extend_from_slice(&config.max_payload_bytes().to_be_bytes());
    out.push(
        u8::from(faults.loss)
            | (u8::from(faults.duplication) << 1)
            | (u8::from(faults.reordering) << 2),
    );
    out.extend_from_slice(&config.max_partitions().to_be_bytes());
    out.extend_from_slice(&config.max_retained_bytes().to_be_bytes());
    out.extend_from_slice(&u32_len(events.len()).to_be_bytes());
    for event in events {
        match event {
            Event::Sent {
                envelope,
                src,
                dst,
                payload,
            } => {
                out.push(1);
                out.extend_from_slice(&envelope.0.to_be_bytes());
                out.push(src.0);
                out.push(dst.0);
                out.extend_from_slice(&u32_len(payload.0.len()).to_be_bytes());
                out.extend_from_slice(&payload.0);
            }
            Event::Delivered { envelope, src, dst } => {
                out.push(2);
                out.extend_from_slice(&envelope.0.to_be_bytes());
                out.push(src.0);
                out.push(dst.0);
            }
            Event::Dropped(envelope) => {
                out.push(3);
                out.extend_from_slice(&envelope.0.to_be_bytes());
            }
            Event::Duplicated(envelope) => {
                out.push(4);
                out.extend_from_slice(&envelope.0.to_be_bytes());
            }
            Event::Delayed(envelope) => {
                out.push(5);
                out.extend_from_slice(&envelope.0.to_be_bytes());
            }
            Event::Partitioned(side) => {
                out.push(6);
                out.extend_from_slice(&side.0.to_be_bytes());
            }
            Event::Healed => out.push(7),
        }
    }
    out
}
