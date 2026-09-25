//! The journal's declared happens-before relation, and its restriction to a subset of
//! events (PR 18, bn-3km4z).
//!
//! # Why this is here
//!
//! PR 18 reduces a failing trace to a causal core. Its passes are generic over an event
//! trace with a happens-before relation (`continuum-debugger`'s `reduce` module). CIR
//! (RFC 0001, PR 17) will give that relation as each event's `causes`. Until CIR lands,
//! the PR-14 semantic journal is the trace, and this module gives the two things the
//! passes need from it: the relation, and a candidate sub-journal that the lift can
//! replay.
//!
//! # The declared causality semantics
//!
//! RFC 0039 requires causal language to name its causality semantics. This one is
//! Mazurkiewicz dependence over declared footprints:
//!
//! - each event has a **footprint**, a set of `(key, access)` pairs ([`footprint`]):
//!   the task, region, reservation, obligation, timer, channel, message or virtual clock
//!   it reads or writes;
//! - two events are **dependent** when their footprints share a key and at least one of
//!   the two accesses is a write ([`dependent`]);
//! - `a` **happens before** `b` when `a` comes first in the journal and a chain of
//!   dependent events leads from `a` to `b`: the transitive closure of journal order
//!   restricted to dependent pairs.
//!
//! Every event reads every ordinal it names, so it follows that ordinal's introduction
//! (a channel's opening names its receiver, for example, and follows its spawn). A
//! task's own event also reads its region and every ancestor, so it is ordered with
//! those regions' cancellation, close and teardown.
//!
//! The footprints over-approximate the substrate's order. A task's own events all write
//! the task, so program order is kept. A spawn and a child region's opening read their
//! parent region, and every close, cancel, drain, finalize and crash writes its whole
//! subtree, so structure is kept. A channel's queue is written by every send and
//! receive, so FIFO order is kept. The virtual clock is written by each advance and read
//! by each timed event. A drain, finalize or crash also writes every task of its
//! subtree, and a settle writes every obligation its region ever owned.
//!
//! An over-approximation keeps more events in a closure than the substrate needs. An
//! under-approximation would drop one it needs, and the replay check of the reduction
//! would then refuse the candidate. So a closure that does not replay to the failure is
//! a typed outcome of the pass, never a silent loss.
//!
//! # Atoms
//!
//! One substrate operation can emit several events that the lift judges only whole: a
//! region's teardown (request, cleanup, drain, finalize, settle) and a task's own
//! cancellation. [`atoms`] names them, so a reduction keeps or drops each one whole.
//!
//! # Restriction
//!
//! [`restrict`] builds the sub-journal of a set of events, and [`permute`] a journal of
//! events in any order. The journal names regions, tasks, reservations, obligations,
//! timers, channels and messages by dense ordinals in allocation order, and the lift
//! checks that density. A subset leaves gaps, so each kind of ordinal is renamed in the
//! order the new journal first names it, and the root region `r0` stays `r0`.
//! [`Renaming`] records it, so a caller can carry its own tables (for example a
//! projection's role table) across. Sequence numbers are dense again from zero.
//!
//! # What this is not
//!
//! It is not CIR. The relation is derived from a linearization and a footprint table,
//! not declared per event by the producer. When PR 17 lands, CIR events carry their own
//! `causes`, and the reduction reads those instead of this module.

use std::collections::{BTreeMap, BTreeSet};

use crate::family::EventBody;
use crate::family::cancellation::{CancelCause, CancellationEvent};
use crate::family::channel::{ChannelEvent, ChannelOrdinal, MessageOrdinal, MessageSet};
use crate::family::effect::{EffectEvent, ReservationOrdinal};
use crate::family::lifecycle::{LifecycleEvent, RegionOrdinal, TaskOrdinal, TaskSet, TaskStep};
use crate::family::obligation::{ObligationEvent, ObligationOrdinal, ObligationSet};
use crate::family::time::{TimeEvent, TimerOrdinal};
use crate::journal::Journal;

/// What an event touches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Key {
    /// A task, by ordinal.
    Task(u32),
    /// A region, by ordinal.
    Region(u32),
    /// A reservation, by ordinal.
    Reservation(u32),
    /// An obligation, by ordinal.
    Obligation(u32),
    /// A timer, by ordinal.
    Timer(u32),
    /// A channel's queue, by ordinal.
    Channel(u32),
    /// A message, by ordinal.
    Message(u64),
    /// The virtual clock.
    Clock,
    /// A resource an observer declares: state the property reads through an
    /// abstraction and that no journal event names, for example a replica's storage slot
    /// that two incarnations write. Opaque here; its meaning is the observer's
    /// ([`footprints_with`]).
    Observer(u64),
}

/// How an event touches a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Access {
    /// The event depends on the key's state and does not change it.
    Read,
    /// The event changes the key's state.
    Write,
}

/// One event's footprint: `(key, access)` pairs, ascending, one access per key (a write
/// absorbs a read of the same key).
pub type Footprint = Vec<(Key, Access)>;

/// Why a journal's relation could not be derived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CausalError {
    /// The work the derivation needs exceeds the caller's bound: charged before any
    /// footprint is built, from the journal's length and its largest event.
    WorkBound {
        /// The predicted work units.
        needed: u64,
        /// The bound.
        bound: u64,
    },
    /// A kept index is not an event of the journal.
    IndexOutOfRange {
        /// The index.
        index: usize,
        /// The journal's length.
        len: usize,
    },
    /// The kept indices are not strictly ascending.
    NotAscending {
        /// The first index out of order.
        index: usize,
    },
    /// The restricted journal cannot number its events.
    TooManyEvents,
    /// A sequence names one event twice.
    Repeated {
        /// The index.
        index: usize,
    },
    /// An observer footprint list is not one entry per journal event.
    ObserverLength {
        /// Entries given.
        given: usize,
        /// The journal's length.
        len: usize,
    },
}

/// Structural facts the footprints read, collected in journal order.
#[derive(Default)]
struct Facts {
    parent: BTreeMap<u32, u32>,
    children: BTreeMap<u32, Vec<u32>>,
    owner: BTreeMap<u32, u32>,
    reservation_holder: BTreeMap<u32, u32>,
    obligation_holder: BTreeMap<u32, u32>,
    region_obligations: BTreeMap<u32, BTreeSet<u32>>,
    timer_task: BTreeMap<u32, u32>,
    receiver: BTreeMap<u32, u32>,
}

impl Facts {
    /// `region` and every region opened under it so far: a walk over the children map,
    /// with a visited set, so it assumes nothing of ordinal order and a malformed journal
    /// (a region reopened, a cycle) cannot loop.
    fn subtree(&self, region: u32) -> BTreeSet<u32> {
        let mut out = BTreeSet::from([region]);
        let mut stack = vec![region];
        while let Some(r) = stack.pop() {
            if let Some(kids) = self.children.get(&r) {
                for &k in kids {
                    if out.insert(k) {
                        stack.push(k);
                    }
                }
            }
        }
        out
    }

    /// `region` and its ancestors up to the root, bounded by the number of regions so a
    /// malformed parent map cannot loop.
    fn ancestors(&self, region: u32) -> BTreeSet<u32> {
        let mut out = BTreeSet::from([region]);
        let mut at = region;
        for _ in 0..=self.parent.len() {
            match self.parent.get(&at) {
                Some(&p) if out.insert(p) => at = p,
                _ => break,
            }
        }
        out
    }

    fn tasks_in(&self, regions: &BTreeSet<u32>) -> Vec<u32> {
        self.owner
            .iter()
            .filter(|(_, r)| regions.contains(r))
            .map(|(t, _)| *t)
            .collect()
    }

    /// Record what `body` introduces, after its footprint was taken.
    fn learn(&mut self, body: &EventBody) {
        match body {
            EventBody::Lifecycle(LifecycleEvent::RegionOpened { region, parent }) => {
                self.parent.insert(region.0, parent.0);
                self.children.entry(parent.0).or_default().push(region.0);
            }
            EventBody::Lifecycle(LifecycleEvent::TaskSpawned { task, region, .. }) => {
                self.owner.insert(task.0, region.0);
            }
            EventBody::Effect(EffectEvent::Reserved { reservation, task }) => {
                self.reservation_holder.insert(reservation.0, task.0);
            }
            EventBody::Obligation(
                ObligationEvent::Opened {
                    obligation,
                    holder,
                    region,
                    ..
                }
                | ObligationEvent::Transferred {
                    obligation,
                    holder,
                    region,
                },
            ) => {
                self.obligation_holder.insert(obligation.0, holder.0);
                self.region_obligations
                    .entry(region.0)
                    .or_default()
                    .insert(obligation.0);
            }
            EventBody::Time(TimeEvent::Scheduled { timer, task, .. }) => {
                self.timer_task.insert(timer.0, task.0);
            }
            EventBody::Channel(ChannelEvent::Opened {
                channel, receiver, ..
            }) => {
                self.receiver.insert(channel.0, receiver.0);
            }
            _ => {}
        }
    }
}

struct Builder(BTreeMap<Key, Access>);

impl Builder {
    fn new() -> Self {
        Self(BTreeMap::new())
    }
    fn read(&mut self, key: Key) {
        self.0.entry(key).or_insert(Access::Read);
    }
    fn write(&mut self, key: Key) {
        self.0.insert(key, Access::Write);
    }
    fn write_task(&mut self, task: Option<&u32>) {
        if let Some(&t) = task {
            self.write(Key::Task(t));
        }
    }
    fn finish(self) -> Footprint {
        self.0.into_iter().collect()
    }
}

fn footprint_in(body: &EventBody, facts: &Facts) -> Footprint {
    let mut b = Builder::new();
    match body {
        EventBody::Lifecycle(event) => match event {
            LifecycleEvent::RegionOpened { region, parent } => {
                b.write(Key::Region(region.0));
                b.read(Key::Region(parent.0));
            }
            LifecycleEvent::TaskSpawned { task, region, .. } => {
                b.write(Key::Task(task.0));
                b.read(Key::Region(region.0));
            }
            LifecycleEvent::TaskStepped { task, step } => {
                b.write(Key::Task(task.0));
                if matches!(step, TaskStep::CancelRequested | TaskStep::Cancel) {
                    // A task's own cancellation is its deadline's: the clock decides it.
                    b.read(Key::Clock);
                }
            }
            LifecycleEvent::RegionCloseRequested { region }
            | LifecycleEvent::RegionCancelRequested { region } => {
                for r in facts.subtree(region.0) {
                    b.write(Key::Region(r));
                }
            }
            LifecycleEvent::RegionDrained { region, cancelled } => {
                let subtree = facts.subtree(region.0);
                for t in facts.tasks_in(&subtree) {
                    b.write(Key::Task(t));
                }
                for t in cancelled.as_slice() {
                    b.write(Key::Task(t.0));
                }
                for r in subtree {
                    b.write(Key::Region(r));
                }
            }
            LifecycleEvent::RegionFinalized { region } => {
                let subtree = facts.subtree(region.0);
                for t in facts.tasks_in(&subtree) {
                    b.write(Key::Task(t));
                }
                for r in subtree {
                    b.write(Key::Region(r));
                }
            }
            LifecycleEvent::RegionCrashed { region, fenced } => {
                let subtree = facts.subtree(region.0);
                for t in facts.tasks_in(&subtree) {
                    b.write(Key::Task(t));
                }
                for t in fenced.as_slice() {
                    b.write(Key::Task(t.0));
                }
                for r in subtree {
                    b.write(Key::Region(r));
                }
            }
        },
        EventBody::Effect(event) => match event {
            EffectEvent::Reserved { reservation, task } => {
                b.write(Key::Reservation(reservation.0));
                b.write(Key::Task(task.0));
            }
            EffectEvent::Committed { reservation }
            | EffectEvent::Aborted { reservation, .. }
            | EffectEvent::Fenced { reservation } => {
                b.write(Key::Reservation(reservation.0));
                b.write_task(facts.reservation_holder.get(&reservation.0));
            }
        },
        EventBody::Cancellation(event) => match event {
            CancellationEvent::Requested { task, cause } => {
                b.write(Key::Task(task.0));
                if let Some(&owner) = facts.owner.get(&task.0) {
                    for r in facts.ancestors(owner) {
                        b.read(Key::Region(r));
                    }
                }
                if *cause == CancelCause::Deadline {
                    b.read(Key::Clock);
                }
            }
            CancellationEvent::Acknowledged { task }
            | CancellationEvent::Cancelled { task, .. } => {
                b.write(Key::Task(task.0));
            }
        },
        EventBody::Obligation(event) => match event {
            ObligationEvent::Opened {
                obligation,
                holder,
                region,
                ..
            } => {
                b.write(Key::Obligation(obligation.0));
                b.write(Key::Task(holder.0));
                b.read(Key::Region(region.0));
            }
            ObligationEvent::Transferred {
                obligation,
                holder,
                region,
            } => {
                b.write(Key::Obligation(obligation.0));
                b.write_task(facts.obligation_holder.get(&obligation.0));
                b.write(Key::Task(holder.0));
                b.read(Key::Region(region.0));
            }
            ObligationEvent::Discharged { obligation, .. }
            | ObligationEvent::Leaked { obligation }
            | ObligationEvent::Fenced { obligation } => {
                b.write(Key::Obligation(obligation.0));
                b.write_task(facts.obligation_holder.get(&obligation.0));
            }
            ObligationEvent::RegionSettled {
                region,
                open,
                leaked,
                fenced,
            } => {
                b.write(Key::Region(region.0));
                if let Some(owned) = facts.region_obligations.get(&region.0) {
                    for o in owned {
                        b.write(Key::Obligation(*o));
                    }
                }
                for o in open
                    .as_slice()
                    .iter()
                    .chain(leaked.as_slice())
                    .chain(fenced.as_slice())
                {
                    b.write(Key::Obligation(o.0));
                }
            }
        },
        EventBody::Time(event) => match event {
            TimeEvent::Scheduled { timer, task, .. } => {
                b.write(Key::Timer(timer.0));
                b.write(Key::Task(task.0));
                b.read(Key::Clock);
            }
            TimeEvent::Advanced { .. } => b.write(Key::Clock),
            TimeEvent::Fired { timer, .. } | TimeEvent::Cancelled { timer, .. } => {
                b.write(Key::Timer(timer.0));
                b.write_task(facts.timer_task.get(&timer.0));
                b.read(Key::Clock);
            }
            TimeEvent::Deadline { task, .. } => {
                b.write(Key::Task(task.0));
                b.read(Key::Clock);
            }
            TimeEvent::Fenced { timer } => {
                b.write(Key::Timer(timer.0));
                b.write_task(facts.timer_task.get(&timer.0));
            }
        },
        EventBody::Channel(event) => match event {
            ChannelEvent::Opened { channel, .. } | ChannelEvent::SendersClosed { channel } => {
                b.write(Key::Channel(channel.0));
            }
            ChannelEvent::Sent {
                channel,
                message,
                sender,
            }
            | ChannelEvent::SendBlocked {
                channel,
                message,
                sender,
            }
            | ChannelEvent::SendClosed {
                channel,
                message,
                sender,
            }
            | ChannelEvent::SendAbandoned {
                channel,
                message,
                sender,
            } => {
                b.write(Key::Channel(channel.0));
                b.write(Key::Message(message.0));
                b.write(Key::Task(sender.0));
            }
            ChannelEvent::Received { channel, message } => {
                b.write(Key::Channel(channel.0));
                b.write(Key::Message(message.0));
                b.write_task(facts.receiver.get(&channel.0));
            }
            ChannelEvent::RecvBlocked { channel }
            | ChannelEvent::RecvClosed { channel }
            | ChannelEvent::RecvAbandoned { channel } => {
                b.write(Key::Channel(channel.0));
                b.write_task(facts.receiver.get(&channel.0));
            }
            ChannelEvent::ReceiverGone { channel, discarded } => {
                b.write(Key::Channel(channel.0));
                b.write_task(facts.receiver.get(&channel.0));
                for m in discarded.as_slice() {
                    b.write(Key::Message(m.0));
                }
            }
        },
    }
    b.finish()
}

/// The largest number of ordinals one event names: the work unit a footprint is
/// charged at, together with the structure it reads.
fn names_in(body: &EventBody) -> u64 {
    let n = match body {
        EventBody::Lifecycle(
            LifecycleEvent::RegionDrained { cancelled: set, .. }
            | LifecycleEvent::RegionCrashed { fenced: set, .. },
        ) => set.as_slice().len(),
        EventBody::Obligation(ObligationEvent::RegionSettled {
            open,
            leaked,
            fenced,
            ..
        }) => open.as_slice().len() + leaked.as_slice().len() + fenced.as_slice().len(),
        EventBody::Channel(ChannelEvent::ReceiverGone { discarded, .. }) => {
            discarded.as_slice().len()
        }
        _ => 0,
    };
    u64::try_from(n).unwrap_or(u64::MAX).saturating_add(4)
}

/// The work [`footprints`] and [`predecessors`] need for `journal`, in units: per event,
/// the ordinals it names plus the structure a subtree walk can read (every region and
/// task and obligation the journal introduced, bounded by the journal's length). The
/// caller compares it with its bound before either runs.
#[must_use]
pub fn predicted_work(journal: &Journal) -> u64 {
    let len = u64::try_from(journal.len()).unwrap_or(u64::MAX);
    let names: u64 = journal
        .events()
        .iter()
        .map(|e| names_in(e.body()))
        .fold(0_u64, u64::saturating_add);
    // Each subtree or ancestor walk reads at most every event's introduction once.
    len.saturating_mul(len.saturating_add(1))
        .saturating_add(names)
}

/// Every event's footprint, in journal order, under the declared semantics above.
///
/// # Errors
///
/// [`CausalError::WorkBound`] when [`predicted_work`] exceeds `bound`; nothing is built.
pub fn footprints(journal: &Journal, bound: u64) -> Result<Vec<Footprint>, CausalError> {
    let needed = predicted_work(journal);
    if needed > bound {
        return Err(CausalError::WorkBound { needed, bound });
    }
    let mut facts = Facts::default();
    let mut out = Vec::with_capacity(journal.len());
    for event in journal.events() {
        let mut b = Builder(footprint_in(event.body(), &facts).into_iter().collect());
        // Every event reads everything it names: it depends on each one's introduction.
        let _ = rename(event.body(), &mut |kind, v| {
            b.read(key_of(kind, v));
            v
        });
        // A task's own event reads its region and every ancestor: a task acts only as
        // its regions' state allows (a requested cancellation is observed before any
        // other step, RFC 0026 correction 55). Region teardown events already write
        // their whole subtree and are skipped, so this adds at most the depth of the
        // region tree per task the event writes, and at most two tasks are written.
        if !matches!(
            event.body(),
            EventBody::Lifecycle(
                LifecycleEvent::RegionCloseRequested { .. }
                    | LifecycleEvent::RegionCancelRequested { .. }
                    | LifecycleEvent::RegionDrained { .. }
                    | LifecycleEvent::RegionFinalized { .. }
                    | LifecycleEvent::RegionCrashed { .. }
            )
        ) {
            let actors: Vec<u32> =
                b.0.iter()
                    .filter_map(|(k, a)| match (k, a) {
                        (Key::Task(t), Access::Write) => Some(*t),
                        _ => None,
                    })
                    .collect();
            for t in actors {
                if let Some(&owner) = facts.owner.get(&t) {
                    for r in facts.ancestors(owner) {
                        b.read(Key::Region(r));
                    }
                }
            }
        }
        out.push(b.finish());
        facts.learn(event.body());
    }
    Ok(out)
}

/// Every event's footprint with an observer's declared footprint merged in: `observer`
/// is empty (no observer) or has one entry per event. A key both declare is a write when
/// either writes it. RFC 0028 keeps abstraction-relevant hidden events in a causal core
/// (INV-013 scopes reduction to named observers): an observer that reads state no
/// journal event names declares it here, so the relation orders the events that write
/// it.
///
/// # Errors
///
/// As for [`footprints`]; [`CausalError::ObserverLength`] for a list of another length,
/// checked before anything is built. The observer's entries are charged with the rest.
pub fn footprints_with(
    journal: &Journal,
    bound: u64,
    observer: &[Footprint],
) -> Result<Vec<Footprint>, CausalError> {
    if !observer.is_empty() && observer.len() != journal.len() {
        return Err(CausalError::ObserverLength {
            given: observer.len(),
            len: journal.len(),
        });
    }
    let extra = observer
        .iter()
        .map(|f| u64::try_from(f.len()).unwrap_or(u64::MAX))
        .fold(0_u64, u64::saturating_add);
    let needed = predicted_work(journal).saturating_add(extra);
    if needed > bound {
        return Err(CausalError::WorkBound { needed, bound });
    }
    let mut prints = footprints(journal, bound)?;
    for (print, more) in prints.iter_mut().zip(observer) {
        let mut b = Builder(print.iter().copied().collect());
        for &(key, access) in more {
            match access {
                Access::Read => b.read(key),
                Access::Write => b.write(key),
            }
        }
        *print = b.finish();
    }
    Ok(prints)
}

/// Whether two footprints are dependent: they share a key and at least one of the two
/// accesses writes it. Symmetric; it does not look at journal order.
#[must_use]
pub fn dependent(a: &Footprint, b: &Footprint) -> bool {
    // Both are ascending by key: one merge.
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        match a[i].0.cmp(&b[j].0) {
            core::cmp::Ordering::Less => i += 1,
            core::cmp::Ordering::Greater => j += 1,
            core::cmp::Ordering::Equal => {
                if a[i].1 == Access::Write || b[j].1 == Access::Write {
                    return true;
                }
                i += 1;
                j += 1;
            }
        }
    }
    false
}

/// Each event's immediate predecessors under the declared happens-before relation:
/// for every key it touches, the last earlier event that wrote it, and, when it writes
/// the key, every earlier event that read it since that write. The transitive closure
/// of these edges is the transitive closure of journal order restricted to
/// [`dependent`] pairs. Each list is ascending, and every predecessor is earlier than
/// its event.
///
/// # Errors
///
/// As for [`footprints`].
pub fn predecessors(journal: &Journal, bound: u64) -> Result<Vec<Vec<usize>>, CausalError> {
    predecessors_with(journal, bound, &[])
}

/// As [`predecessors`], over [`footprints_with`]'s merged footprints.
///
/// # Errors
///
/// As for [`footprints_with`].
pub fn predecessors_with(
    journal: &Journal,
    bound: u64,
    observer: &[Footprint],
) -> Result<Vec<Vec<usize>>, CausalError> {
    let prints = footprints_with(journal, bound, observer)?;
    let mut last_write: BTreeMap<Key, usize> = BTreeMap::new();
    let mut reads: BTreeMap<Key, Vec<usize>> = BTreeMap::new();
    let mut out = Vec::with_capacity(prints.len());
    for (i, print) in prints.iter().enumerate() {
        let mut preds = BTreeSet::new();
        for &(key, access) in print {
            if let Some(&w) = last_write.get(&key) {
                preds.insert(w);
            }
            if access == Access::Write {
                if let Some(rs) = reads.remove(&key) {
                    preds.extend(rs);
                }
            }
        }
        for &(key, access) in print {
            match access {
                Access::Write => {
                    last_write.insert(key, i);
                }
                Access::Read => reads.entry(key).or_default().push(i),
            }
        }
        out.push(preds.into_iter().collect());
    }
    Ok(out)
}

/// The journal's operation atoms: the runs of events one substrate operation emits and
/// the lift judges only whole ([`crate::lift::LiftStop::Incomplete`] for a journal that
/// ends inside one). Each is an index interval, disjoint and ascending:
///
/// - a region's teardown: from its cancellation request, its crash, or (for a normal
///   close) its drain, through its `RegionFinalized` and the `RegionFinalized` and
///   `RegionSettled` events that follow it in the same batch;
/// - a task's own cancellation: from its `cancel-requested` step through its `cancel`
///   step.
///
/// A teardown or cancellation the journal never completes runs to the journal's end.
/// Overlapping intervals merge, so an atom may hold events of other tasks that the
/// substrate interleaved: an over-approximation, which keeps more, never less.
#[must_use]
pub fn atoms(journal: &Journal) -> Vec<Vec<usize>> {
    let events = journal.events();
    let n = events.len();
    // Open protocols: region → start, task → start.
    let mut regions: BTreeMap<u32, usize> = BTreeMap::new();
    let mut tasks: BTreeMap<u32, usize> = BTreeMap::new();
    let mut spans: Vec<(usize, usize)> = Vec::new();
    for (i, event) in events.iter().enumerate() {
        match event.body() {
            EventBody::Lifecycle(
                LifecycleEvent::RegionCancelRequested { region }
                | LifecycleEvent::RegionCrashed { region, .. }
                | LifecycleEvent::RegionDrained { region, .. },
            ) => {
                regions.entry(region.0).or_insert(i);
            }
            EventBody::Lifecycle(LifecycleEvent::RegionFinalized { region }) => {
                if let Some(start) = regions.remove(&region.0) {
                    let mut end = i;
                    while end + 1 < n
                        && matches!(
                            events[end + 1].body(),
                            EventBody::Lifecycle(LifecycleEvent::RegionFinalized { .. })
                                | EventBody::Obligation(ObligationEvent::RegionSettled { .. })
                        )
                    {
                        end += 1;
                    }
                    spans.push((start, end));
                }
            }
            EventBody::Lifecycle(LifecycleEvent::TaskStepped {
                task,
                step: TaskStep::CancelRequested,
            }) => {
                tasks.entry(task.0).or_insert(i);
            }
            EventBody::Lifecycle(LifecycleEvent::TaskStepped {
                task,
                step: TaskStep::Cancel,
            }) => {
                if let Some(start) = tasks.remove(&task.0) {
                    spans.push((start, i));
                }
            }
            _ => {}
        }
    }
    let last = n.saturating_sub(1);
    spans.extend(regions.values().chain(tasks.values()).map(|&s| (s, last)));
    spans.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (s, e) in spans {
        match merged.last_mut() {
            Some(m) if s <= m.1 => m.1 = m.1.max(e),
            _ => merged.push((s, e)),
        }
    }
    merged
        .into_iter()
        .filter(|(s, e)| e > s)
        .map(|(s, e)| (s..=e).collect())
        .collect()
}

/// One kind of ordinal the journal allocates densely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Region,
    Task,
    Reservation,
    Obligation,
    Timer,
    Channel,
    Message,
}

fn map_task(t: TaskOrdinal, f: &mut impl FnMut(Kind, u64) -> u64) -> TaskOrdinal {
    TaskOrdinal(narrow(f(Kind::Task, u64::from(t.0))))
}

fn map_region(r: RegionOrdinal, f: &mut impl FnMut(Kind, u64) -> u64) -> RegionOrdinal {
    RegionOrdinal(narrow(f(Kind::Region, u64::from(r.0))))
}

fn map_obligation(o: ObligationOrdinal, f: &mut impl FnMut(Kind, u64) -> u64) -> ObligationOrdinal {
    ObligationOrdinal(narrow(f(Kind::Obligation, u64::from(o.0))))
}

fn map_timer(k: TimerOrdinal, f: &mut impl FnMut(Kind, u64) -> u64) -> TimerOrdinal {
    TimerOrdinal(narrow(f(Kind::Timer, u64::from(k.0))))
}

fn map_channel(c: ChannelOrdinal, f: &mut impl FnMut(Kind, u64) -> u64) -> ChannelOrdinal {
    ChannelOrdinal(narrow(f(Kind::Channel, u64::from(c.0))))
}

fn map_message(m: MessageOrdinal, f: &mut impl FnMut(Kind, u64) -> u64) -> MessageOrdinal {
    MessageOrdinal(f(Kind::Message, m.0))
}

fn map_reservation(
    x: ReservationOrdinal,
    f: &mut impl FnMut(Kind, u64) -> u64,
) -> ReservationOrdinal {
    ReservationOrdinal(narrow(f(Kind::Reservation, u64::from(x.0))))
}

/// A `u32` ordinal carried through a `u64` renaming. Every value here is an original
/// `u32` ordinal or a new one, which is below the number of distinct `u32` ordinals of
/// its kind, so this cannot truncate; `u32::MAX` is a defensive saturation.
fn narrow(v: u64) -> u32 {
    u32::try_from(v).unwrap_or(u32::MAX)
}

fn key_of(kind: Kind, v: u64) -> Key {
    let n = narrow(v);
    match kind {
        Kind::Region => Key::Region(n),
        Kind::Task => Key::Task(n),
        Kind::Reservation => Key::Reservation(n),
        Kind::Obligation => Key::Obligation(n),
        Kind::Timer => Key::Timer(n),
        Kind::Channel => Key::Channel(n),
        Kind::Message => Key::Message(v),
    }
}

/// `body` with every ordinal passed through `f`; the event's shape is unchanged.
#[allow(clippy::too_many_lines)]
fn rename(body: &EventBody, f: &mut impl FnMut(Kind, u64) -> u64) -> EventBody {
    match body {
        EventBody::Lifecycle(event) => EventBody::Lifecycle(match event {
            LifecycleEvent::RegionOpened { region, parent } => LifecycleEvent::RegionOpened {
                region: map_region(*region, f),
                parent: map_region(*parent, f),
            },
            LifecycleEvent::TaskSpawned {
                task,
                region,
                resumability,
            } => LifecycleEvent::TaskSpawned {
                task: map_task(*task, f),
                region: map_region(*region, f),
                resumability: resumability.clone(),
            },
            LifecycleEvent::TaskStepped { task, step } => LifecycleEvent::TaskStepped {
                task: map_task(*task, f),
                step: step.clone(),
            },
            LifecycleEvent::RegionCloseRequested { region } => {
                LifecycleEvent::RegionCloseRequested {
                    region: map_region(*region, f),
                }
            }
            LifecycleEvent::RegionCancelRequested { region } => {
                LifecycleEvent::RegionCancelRequested {
                    region: map_region(*region, f),
                }
            }
            LifecycleEvent::RegionDrained { region, cancelled } => LifecycleEvent::RegionDrained {
                region: map_region(*region, f),
                cancelled: TaskSet::new(cancelled.as_slice().iter().map(|t| map_task(*t, f))),
            },
            LifecycleEvent::RegionFinalized { region } => LifecycleEvent::RegionFinalized {
                region: map_region(*region, f),
            },
            LifecycleEvent::RegionCrashed { region, fenced } => LifecycleEvent::RegionCrashed {
                region: map_region(*region, f),
                fenced: TaskSet::new(fenced.as_slice().iter().map(|t| map_task(*t, f))),
            },
        }),
        EventBody::Effect(event) => EventBody::Effect(match event {
            EffectEvent::Reserved { reservation, task } => EffectEvent::Reserved {
                reservation: map_reservation(*reservation, f),
                task: map_task(*task, f),
            },
            EffectEvent::Committed { reservation } => EffectEvent::Committed {
                reservation: map_reservation(*reservation, f),
            },
            EffectEvent::Aborted { reservation, cause } => EffectEvent::Aborted {
                reservation: map_reservation(*reservation, f),
                cause: *cause,
            },
            EffectEvent::Fenced { reservation } => EffectEvent::Fenced {
                reservation: map_reservation(*reservation, f),
            },
        }),
        EventBody::Cancellation(event) => EventBody::Cancellation(match event {
            CancellationEvent::Requested { task, cause } => CancellationEvent::Requested {
                task: map_task(*task, f),
                cause: *cause,
            },
            CancellationEvent::Acknowledged { task } => CancellationEvent::Acknowledged {
                task: map_task(*task, f),
            },
            CancellationEvent::Cancelled { task, cause } => CancellationEvent::Cancelled {
                task: map_task(*task, f),
                cause: *cause,
            },
        }),
        EventBody::Obligation(event) => EventBody::Obligation(match event {
            ObligationEvent::Opened {
                obligation,
                kind,
                holder,
                region,
            } => ObligationEvent::Opened {
                obligation: map_obligation(*obligation, f),
                kind: *kind,
                holder: map_task(*holder, f),
                region: map_region(*region, f),
            },
            ObligationEvent::Discharged { obligation, how } => ObligationEvent::Discharged {
                obligation: map_obligation(*obligation, f),
                how: *how,
            },
            ObligationEvent::Transferred {
                obligation,
                holder,
                region,
            } => ObligationEvent::Transferred {
                obligation: map_obligation(*obligation, f),
                holder: map_task(*holder, f),
                region: map_region(*region, f),
            },
            ObligationEvent::Leaked { obligation } => ObligationEvent::Leaked {
                obligation: map_obligation(*obligation, f),
            },
            ObligationEvent::RegionSettled {
                region,
                open,
                leaked,
                fenced,
            } => {
                let mut set = |s: &ObligationSet| {
                    ObligationSet::new(s.as_slice().iter().map(|o| map_obligation(*o, f)))
                };
                let (open, leaked, fenced) = (set(open), set(leaked), set(fenced));
                ObligationEvent::RegionSettled {
                    region: map_region(*region, f),
                    open,
                    leaked,
                    fenced,
                }
            }
            ObligationEvent::Fenced { obligation } => ObligationEvent::Fenced {
                obligation: map_obligation(*obligation, f),
            },
        }),
        EventBody::Time(event) => EventBody::Time(match event {
            TimeEvent::Scheduled {
                timer,
                task,
                at,
                deadline,
            } => TimeEvent::Scheduled {
                timer: map_timer(*timer, f),
                task: map_task(*task, f),
                at: *at,
                deadline: *deadline,
            },
            TimeEvent::Advanced { from, to } => TimeEvent::Advanced {
                from: *from,
                to: *to,
            },
            TimeEvent::Fired { timer, at } => TimeEvent::Fired {
                timer: map_timer(*timer, f),
                at: *at,
            },
            TimeEvent::Cancelled { timer, at } => TimeEvent::Cancelled {
                timer: map_timer(*timer, f),
                at: *at,
            },
            TimeEvent::Deadline { task, at } => TimeEvent::Deadline {
                task: map_task(*task, f),
                at: *at,
            },
            TimeEvent::Fenced { timer } => TimeEvent::Fenced {
                timer: map_timer(*timer, f),
            },
        }),
        EventBody::Channel(event) => EventBody::Channel(match event {
            ChannelEvent::Opened {
                channel,
                capacity,
                receiver,
            } => ChannelEvent::Opened {
                channel: map_channel(*channel, f),
                capacity: *capacity,
                receiver: map_task(*receiver, f),
            },
            ChannelEvent::Sent {
                channel,
                message,
                sender,
            } => ChannelEvent::Sent {
                channel: map_channel(*channel, f),
                message: map_message(*message, f),
                sender: map_task(*sender, f),
            },
            ChannelEvent::SendBlocked {
                channel,
                message,
                sender,
            } => ChannelEvent::SendBlocked {
                channel: map_channel(*channel, f),
                message: map_message(*message, f),
                sender: map_task(*sender, f),
            },
            ChannelEvent::SendClosed {
                channel,
                message,
                sender,
            } => ChannelEvent::SendClosed {
                channel: map_channel(*channel, f),
                message: map_message(*message, f),
                sender: map_task(*sender, f),
            },
            ChannelEvent::SendAbandoned {
                channel,
                message,
                sender,
            } => ChannelEvent::SendAbandoned {
                channel: map_channel(*channel, f),
                message: map_message(*message, f),
                sender: map_task(*sender, f),
            },
            ChannelEvent::Received { channel, message } => ChannelEvent::Received {
                channel: map_channel(*channel, f),
                message: map_message(*message, f),
            },
            ChannelEvent::RecvBlocked { channel } => ChannelEvent::RecvBlocked {
                channel: map_channel(*channel, f),
            },
            ChannelEvent::RecvClosed { channel } => ChannelEvent::RecvClosed {
                channel: map_channel(*channel, f),
            },
            ChannelEvent::RecvAbandoned { channel } => ChannelEvent::RecvAbandoned {
                channel: map_channel(*channel, f),
            },
            ChannelEvent::SendersClosed { channel } => ChannelEvent::SendersClosed {
                channel: map_channel(*channel, f),
            },
            ChannelEvent::ReceiverGone { channel, discarded } => ChannelEvent::ReceiverGone {
                channel: map_channel(*channel, f),
                discarded: MessageSet::new(discarded.as_slice().iter().map(|m| map_message(*m, f))),
            },
        }),
    }
}

/// How a restriction renamed each kind of ordinal: for each kind, the original ordinal
/// of each new ordinal (index = new ordinal), in order of first appearance.
///
/// A message's ordinal is its payload (`family::channel`): an observer that reads
/// delivered values must map them back through [`Self::messages`] or [`Self::message`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Renaming {
    /// Regions; `regions[0]` is the root, `0`.
    pub regions: Vec<u32>,
    /// Tasks.
    pub tasks: Vec<u32>,
    /// Reservations.
    pub reservations: Vec<u32>,
    /// Obligations.
    pub obligations: Vec<u32>,
    /// Timers.
    pub timers: Vec<u32>,
    /// Channels.
    pub channels: Vec<u32>,
    /// Messages.
    pub messages: Vec<u64>,
    /// Original → new, by kind.
    new_of: BTreeMap<(Kind, u64), u64>,
}

impl Renaming {
    fn table(&mut self, kind: Kind) -> Option<&mut Vec<u32>> {
        match kind {
            Kind::Region => Some(&mut self.regions),
            Kind::Task => Some(&mut self.tasks),
            Kind::Reservation => Some(&mut self.reservations),
            Kind::Obligation => Some(&mut self.obligations),
            Kind::Timer => Some(&mut self.timers),
            Kind::Channel => Some(&mut self.channels),
            Kind::Message => None,
        }
    }

    fn lookup(&self, kind: Kind, old: u64) -> Option<u64> {
        self.new_of.get(&(kind, old)).copied()
    }

    fn lookup32(&self, kind: Kind, old: u32) -> Option<u32> {
        self.lookup(kind, u64::from(old))
            .and_then(|v| u32::try_from(v).ok())
    }

    /// The new ordinal of original region `old`, if the restriction kept it.
    #[must_use]
    pub fn region(&self, old: u32) -> Option<u32> {
        self.lookup32(Kind::Region, old)
    }

    /// The new ordinal of original task `old`, if the restriction kept it.
    #[must_use]
    pub fn task(&self, old: u32) -> Option<u32> {
        self.lookup32(Kind::Task, old)
    }

    /// The new ordinal of original reservation `old`, if the restriction kept it.
    #[must_use]
    pub fn reservation(&self, old: u32) -> Option<u32> {
        self.lookup32(Kind::Reservation, old)
    }

    /// The new ordinal of original obligation `old`, if the restriction kept it.
    #[must_use]
    pub fn obligation(&self, old: u32) -> Option<u32> {
        self.lookup32(Kind::Obligation, old)
    }

    /// The new ordinal of original timer `old`, if the restriction kept it.
    #[must_use]
    pub fn timer(&self, old: u32) -> Option<u32> {
        self.lookup32(Kind::Timer, old)
    }

    /// The new ordinal of original channel `old`, if the restriction kept it.
    #[must_use]
    pub fn channel(&self, old: u32) -> Option<u32> {
        self.lookup32(Kind::Channel, old)
    }

    /// The new ordinal of original message `old`, if the restriction kept it.
    #[must_use]
    pub fn message(&self, old: u64) -> Option<u64> {
        self.lookup(Kind::Message, old)
    }
}

/// A sub-journal and how it renamed the original's ordinals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Restriction {
    /// The kept events, in the order asked, renumbered and renamed.
    pub journal: Journal,
    /// The renaming.
    pub renaming: Renaming,
}

/// The sub-journal of the events at `kept` (strictly ascending indices into `journal`),
/// every ordinal renamed in order of first appearance: [`permute`] in journal order.
///
/// # Errors
///
/// As for [`permute`]; [`CausalError::NotAscending`] for indices out of order.
pub fn restrict(journal: &Journal, kept: &[usize]) -> Result<Restriction, CausalError> {
    for (at, &i) in kept.iter().enumerate() {
        if at > 0 && kept[at - 1] >= i {
            return Err(CausalError::NotAscending { index: i });
        }
    }
    permute(journal, kept)
}

/// The journal of the events at `sequence` (distinct indices into `journal`, in any
/// order), in that order, with every kind of ordinal renamed to the order in which the
/// new journal first names it. The root region stays `r0`. The journal allocates every
/// ordinal densely at its first appearance, so a restriction of a journal the lift
/// accepts renames each kind in the same order as the original. The renaming is
/// injective per kind.
///
/// It judges nothing: a sequence that is not a run (a step of a task whose spawn was
/// dropped, say) is built as asked, and the lift refuses it. Its cost is linear in the
/// sequence with a logarithmic factor; a caller that reduces charges it as part of the
/// replay it prepares.
///
/// # Errors
///
/// [`CausalError::IndexOutOfRange`] or [`CausalError::Repeated`] for a malformed
/// `sequence`; [`CausalError::TooManyEvents`] when the journal cannot number an event.
pub fn permute(journal: &Journal, sequence: &[usize]) -> Result<Restriction, CausalError> {
    let events = journal.events();
    let mut seen = vec![false; events.len()];
    for &i in sequence {
        match seen.get_mut(i) {
            None => {
                return Err(CausalError::IndexOutOfRange {
                    index: i,
                    len: events.len(),
                });
            }
            Some(true) => return Err(CausalError::Repeated { index: i }),
            Some(s) => *s = true,
        }
    }
    let mut renaming = Renaming::default();
    renaming.new_of.insert((Kind::Region, 0), 0);
    renaming.regions.push(0);
    let mut out = Journal::new();
    for &i in sequence {
        let body = rename(events[i].body(), &mut |kind, v| {
            if let Some(&n) = renaming.new_of.get(&(kind, v)) {
                return n;
            }
            // Every kind but messages has a `u32` table; messages keep their `u64`.
            let n = if let Some(table) = renaming.table(kind) {
                table.push(narrow(v));
                table.len() - 1
            } else {
                renaming.messages.push(v);
                renaming.messages.len() - 1
            };
            let n = u64::try_from(n).unwrap_or(u64::MAX);
            renaming.new_of.insert((kind, v), n);
            n
        });
        out.append(body).ok_or(CausalError::TooManyEvents)?;
    }
    Ok(Restriction {
        journal: out,
        renaming,
    })
}
