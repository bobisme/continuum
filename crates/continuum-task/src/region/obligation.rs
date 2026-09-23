//! The obligation ledger: the linear resources a region owes, and the proof that
//! teardown discharged every one of them.
//!
//! > Obligations are linear semantic resources created and discharged by events.
//! > Examples include reply obligations, reserved channel capacity, outstanding
//! > durable-write acknowledgements, **child-region quiescence**, and **cancellation
//! > finalization**.
//! >
//! > — `notes/plan/rfcs/0001-causal-intermediate-representation.md`, "Obligations"
//!
//! Two of RFC 0001's five named examples are region-lifecycle obligations, and the
//! validator it specifies lists **obligation conservation** among the things
//! `continuum-cir` must check. This module is that accounting, at the region layer and in
//! typed code rather than in a trace: every structural act that creates work opens an
//! obligation, every act that finishes work discharges one, and the ledger is what
//! [`Finalization`](super::Finalization) reports.
//!
//! # Why the no-orphan property is stated here rather than as a counter
//!
//! A pair of counters ("spawned" and "terminated") can agree while the wrong worker
//! terminated twice and another never did. A ledger cannot: an obligation is keyed by its
//! subject, opening one twice is a no-op that the counters refuse to double-count, and
//! discharging one that is not open is refused outright. So "the ledger is empty" is a
//! statement about *which* work finished, not about how much. The counters are kept
//! beside it as a cross-check with an independent failure mode — the shape docs/03 §8
//! asks of independent paths — and [`Ledger::is_balanced`] requires both to agree.
//!
//! This is the machine-checkable half of G0-DX-14's pass condition, whose first conjunct
//! is **"no leaked obligations"** (`notes/plan/notes/G0_SPIKE_MATRIX.md`). The second
//! conjunct — "resumable artifacts either committed or absent" — is
//! [`ObligationKind::ProvisionalPublication`], which is open exactly while a publication
//! is neither.

use core::fmt;
use std::collections::BTreeSet;

use super::RegionId;
use super::worker::{PublicationSlot, ReasonError, WorkerId};

/// What kind of work an obligation stands for.
///
/// Four calculus kinds, each one an act that creates work plus the act that finishes it,
/// and one adapter family. The middle two calculus kinds are RFC 0001's own named
/// examples, quoted in this module's header. The fifth, [`Self::Substrate`], is how an
/// adapter's own obligations (RFC 0001's "reply obligations, reserved channel capacity,
/// outstanding durable-write acknowledgements") enter this ledger: only through
/// [`RegionTree::open_substrate`](super::RegionTree::open_substrate), which checks that the
/// holder has begun and not terminated, so [`Finalization::is_total`](super::Finalization::is_total) covers them too
/// (RFC 0026 correction 51).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ObligationKind {
    /// A spawned worker owes a quiescent state.
    ///
    /// Opened by [`RegionTree::spawn`](super::RegionTree::spawn), discharged when the
    /// worker parks or terminates. An outstanding one *is* an orphan: work that was
    /// admitted and never accounted for.
    WorkerTermination,
    /// An open child region owes quiescence to its parent (RFC 0001's "child-region
    /// quiescence").
    ///
    /// Opened by [`RegionTree::open_child`](super::RegionTree::open_child), discharged
    /// when the child finalizes. This is what makes teardown of a subtree total rather
    /// than shallow: a parent cannot finalize while it holds one.
    ChildRegionQuiescence,
    /// A cancellation request owes a finalization (RFC 0001's "cancellation
    /// finalization").
    ///
    /// Opened by [`RegionTree::cancel`](super::RegionTree::cancel), discharged when the
    /// cancelled region finalizes. A cancellation that is requested and never finalized
    /// is the leak G0-DX-14 is looking for.
    CancellationFinalization,
    /// A staged publication owes a commit or a discard.
    ///
    /// Opened by [`WorkerStep::Reserve`](super::worker::WorkerStep::Reserve) or
    /// [`ReserveSlot`](super::worker::WorkerStep::ReserveSlot), one per staged slot, and
    /// discharged exactly once — by a commit, by an explicit
    /// [`Abort`](super::worker::WorkerStep::Abort), or by the drain or failure that
    /// discards it.
    /// While it is open, the artifact is neither committed nor absent — the state
    /// G0-DX-14's second conjunct forbids at rest.
    ProvisionalPublication,
    /// An adapter's own obligation, of the adapter's own kind, held by one worker.
    ///
    /// Opened by [`RegionTree::open_substrate`](super::RegionTree::open_substrate) and
    /// discharged by [`RegionTree::discharge_substrate`](super::RegionTree::discharge_substrate),
    /// both of which refuse a holder that has not begun or has terminated. No calculus step opens or
    /// discharges one, and no teardown discharges one on the adapter's behalf: a region
    /// that finalizes while one is open reports it as owed, which is a leak.
    Substrate(SubstrateKind),
}

impl ObligationKind {
    /// A stable token for canonical rendering.
    ///
    /// Every substrate kind shares the family token `substrate`; [`fmt::Display`] adds the
    /// adapter's own kind token, so a calculus kind and an adapter kind never render alike.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::WorkerTermination => "worker-termination",
            Self::ChildRegionQuiescence => "child-region-quiescence",
            Self::CancellationFinalization => "cancellation-finalization",
            Self::ProvisionalPublication => "provisional-publication",
            Self::Substrate(_) => "substrate",
        }
    }

    /// The adapter's kind, when this is a substrate obligation.
    #[must_use]
    pub const fn substrate_kind(self) -> Option<SubstrateKind> {
        match self {
            Self::Substrate(kind) => Some(kind),
            _ => None,
        }
    }
}

impl fmt::Display for ObligationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Substrate(kind) => write!(f, "substrate({kind})"),
            other => f.write_str(other.token()),
        }
    }
}

/// An adapter's own obligation kind, as one canonical token (`send-permit`, `lease`, …).
///
/// A carrier, not a vocabulary: the kinds are the adapter's, and this crate declares no
/// edge to any adapter, so it checks only that the token *is* a token — non-empty,
/// printable ASCII — as [`FailureReason`](super::worker::FailureReason) does. The token is
/// `'static` because an adapter's kinds are a closed set it spells in code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SubstrateKind(&'static str);

impl SubstrateKind {
    /// A kind from its token.
    ///
    /// # Errors
    ///
    /// [`ReasonError`] when the token is empty or is not printable ASCII.
    pub fn new(token: &'static str) -> Result<Self, ReasonError> {
        if token.is_empty() {
            return Err(ReasonError::Empty);
        }
        if let Some((index, character)) = token.char_indices().find(|(_, c)| !c.is_ascii_graphic())
        {
            return Err(ReasonError::NonCanonical { index, character });
        }
        Ok(Self(token))
    }

    /// The kind token.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for SubstrateKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// An adapter obligation's identity inside one [`RegionTree`](super::RegionTree).
///
/// Chosen by the adapter — its own obligation ordinal is the natural choice — and used
/// once: a tree refuses to open an identity it has opened before, discharged or not, so a
/// discharged obligation cannot be reopened to move the counters twice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SubstrateId(u64);

impl SubstrateId {
    /// The identity at `ordinal`.
    #[must_use]
    pub const fn at(ordinal: u64) -> Self {
        Self(ordinal)
    }

    /// This identity's ordinal.
    #[must_use]
    pub const fn ordinal(self) -> u64 {
        self.0
    }
}

impl fmt::Display for SubstrateId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "s{}", self.0)
    }
}

/// How an adapter obligation ended, as its discharge reports it.
///
/// docs/02 §7's effect protocol ends a reserved effect in one of two ways, and the
/// distinction matters to the calculus: a task in cancellation only drains and
/// finalizes, so in a cancelling region only [`Self::Aborted`] is admitted
/// (RFC 0026 correction 51).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SubstrateOutcome {
    /// The obligation was met: its effect is visible.
    Committed,
    /// The obligation was dropped: its effect is not visible.
    Aborted,
}

impl SubstrateOutcome {
    /// A stable token for canonical rendering.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Committed => "committed",
            Self::Aborted => "aborted",
        }
    }
}

impl fmt::Display for SubstrateOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.token())
    }
}

/// One adapter obligation: its kind and its identity — the handle the substrate entry
/// points take.
///
/// A discharge names the kind as well as the identity, and a mismatch is refused as "not
/// open": a caller cannot discharge a lease by naming a send permit's identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SubstrateObligation {
    kind: SubstrateKind,
    id: SubstrateId,
}

impl SubstrateObligation {
    /// The obligation of `kind` with identity `id`.
    #[must_use]
    pub const fn new(kind: SubstrateKind, id: SubstrateId) -> Self {
        Self { kind, id }
    }

    /// Its kind.
    #[must_use]
    pub const fn kind(self) -> SubstrateKind {
        self.kind
    }

    /// Its identity.
    #[must_use]
    pub const fn id(self) -> SubstrateId {
        self.id
    }

    /// The ledger entry it stands for.
    #[must_use]
    pub const fn obligation(self) -> Obligation {
        Obligation::new(
            ObligationKind::Substrate(self.kind),
            Subject::Substrate(self.id),
        )
    }
}

impl fmt::Display for SubstrateObligation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.kind, self.id)
    }
}

/// What an obligation is about.
///
/// The subject is part of the obligation's identity, which is what makes the ledger a
/// statement about *which* work finished. Ordering is by discriminant then ordinal, so
/// the outstanding set renders in one canonical order on every platform (INV-005).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subject {
    /// A region: the child that owes quiescence, or the region whose cancellation owes a
    /// finalization.
    Region(RegionId),
    /// A worker: the one that owes a quiescent state, or holds the staged publication in
    /// its [primary slot](PublicationSlot::PRIMARY).
    Worker(WorkerId),
    /// A publication a worker staged in a non-primary slot. The tree never builds this
    /// with the primary slot, whose subject is [`Self::Worker`].
    Publication(WorkerId, PublicationSlot),
    /// An adapter obligation. Its holder is the tree's record, not part of its identity,
    /// so a transfer moves the holder without moving the ledger.
    Substrate(SubstrateId),
}

impl fmt::Display for Subject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Region(region) => write!(f, "{region}"),
            Self::Worker(worker) => write!(f, "{worker}"),
            Self::Publication(worker, slot) => write!(f, "{worker}{slot}"),
            Self::Substrate(id) => write!(f, "{id}"),
        }
    }
}

/// One linear resource: a kind and the thing it is about.
///
/// Linear in RFC 0001's sense — created once, discharged once. [`Ledger`] enforces both
/// halves: opening one that is already open changes nothing, and discharging one that is
/// not open is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Obligation {
    kind: ObligationKind,
    subject: Subject,
}

impl Obligation {
    /// An obligation of `kind` about `subject`.
    #[must_use]
    pub const fn new(kind: ObligationKind, subject: Subject) -> Self {
        Self { kind, subject }
    }

    /// A worker owes a quiescent state.
    #[must_use]
    pub const fn worker_termination(worker: WorkerId) -> Self {
        Self::new(ObligationKind::WorkerTermination, Subject::Worker(worker))
    }

    /// A child region owes quiescence to its parent.
    #[must_use]
    pub const fn child_region_quiescence(child: RegionId) -> Self {
        Self::new(
            ObligationKind::ChildRegionQuiescence,
            Subject::Region(child),
        )
    }

    /// A cancellation request owes a finalization.
    #[must_use]
    pub const fn cancellation_finalization(region: RegionId) -> Self {
        Self::new(
            ObligationKind::CancellationFinalization,
            Subject::Region(region),
        )
    }

    /// A staged publication owes a commit or a discard.
    #[must_use]
    pub const fn provisional_publication(worker: WorkerId) -> Self {
        Self::new(
            ObligationKind::ProvisionalPublication,
            Subject::Worker(worker),
        )
    }

    /// The publication staged in `slot` owes a commit or a discard.
    ///
    /// The primary slot is keyed by the worker alone — it *is*
    /// [`Self::provisional_publication`] — so a worker that stages one publication at a
    /// time owes exactly the obligation it owed before slots existed.
    #[must_use]
    pub const fn staged_publication(worker: WorkerId, slot: PublicationSlot) -> Self {
        if slot.ordinal() == PublicationSlot::PRIMARY.ordinal() {
            Self::provisional_publication(worker)
        } else {
            Self::new(
                ObligationKind::ProvisionalPublication,
                Subject::Publication(worker, slot),
            )
        }
    }

    /// This obligation's kind.
    #[must_use]
    pub const fn kind(&self) -> ObligationKind {
        self.kind
    }

    /// What this obligation is about.
    #[must_use]
    pub const fn subject(&self) -> Subject {
        self.subject
    }
}

impl fmt::Display for Obligation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.kind, self.subject)
    }
}

/// The open obligations of one region tree, plus the two counters that cross-check them.
///
/// Not [`Clone`] on purpose: a ledger is the tree's single accounting, and a copy of it
/// is a second answer to "what is outstanding".
#[derive(Debug, Default)]
pub struct Ledger {
    open: BTreeSet<Obligation>,
    opened: u64,
    discharged: u64,
}

impl Ledger {
    /// An empty ledger.
    #[must_use]
    pub fn new() -> Self {
        Self {
            open: BTreeSet::new(),
            opened: 0,
            discharged: 0,
        }
    }

    /// Record that `obligation` is now owed.
    ///
    /// Returns whether it was newly opened. Opening one that is already open is a no-op
    /// and does not move the counter, which is what keeps "one subject owes one thing
    /// once" true (linearity).
    pub(crate) fn open(&mut self, obligation: Obligation) -> bool {
        let fresh = self.open.insert(obligation);
        if fresh {
            self.opened += 1;
        }
        fresh
    }

    /// Record that `obligation` has been met.
    ///
    /// Returns whether it was outstanding. Discharging one that is not open is refused
    /// and does not move the counter, so the counters cannot be talked into agreeing by
    /// double-discharging.
    pub(crate) fn discharge(&mut self, obligation: Obligation) -> bool {
        let held = self.open.remove(&obligation);
        if held {
            self.discharged += 1;
        }
        held
    }

    /// Whether `obligation` is currently owed.
    #[must_use]
    pub fn holds(&self, obligation: Obligation) -> bool {
        self.open.contains(&obligation)
    }

    /// How many obligations have ever been opened.
    #[must_use]
    pub const fn opened(&self) -> u64 {
        self.opened
    }

    /// How many obligations have been discharged.
    #[must_use]
    pub const fn discharged(&self) -> u64 {
        self.discharged
    }

    /// The obligations still owed, in canonical order.
    #[must_use]
    pub fn outstanding(&self) -> Vec<Obligation> {
        self.open.iter().copied().collect()
    }

    /// Whether nothing is owed **and** the counters agree.
    ///
    /// Both conjuncts are required. The set answers "which work is unaccounted for" and
    /// the counters answer "how much", and they fail differently: a set that empties
    /// while the counters disagree means a discharge was recorded against an obligation
    /// nobody opened, which is a bug in the accounting rather than in the teardown.
    #[must_use]
    pub fn is_balanced(&self) -> bool {
        self.open.is_empty() && self.opened == self.discharged
    }

    /// A snapshot of this ledger, for a [`Finalization`](super::Finalization) report.
    #[must_use]
    pub fn summary(&self) -> LedgerSummary {
        LedgerSummary {
            opened: self.opened,
            discharged: self.discharged,
            outstanding: self.outstanding(),
        }
    }
}

/// What the ledger held at the moment a region finalized.
///
/// Carried by [`Finalization`](super::Finalization) so the no-orphan claim travels with
/// the evidence for it rather than being re-derived by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerSummary {
    opened: u64,
    discharged: u64,
    outstanding: Vec<Obligation>,
}

impl LedgerSummary {
    /// How many obligations had ever been opened.
    #[must_use]
    pub const fn opened(&self) -> u64 {
        self.opened
    }

    /// How many had been discharged.
    #[must_use]
    pub const fn discharged(&self) -> u64 {
        self.discharged
    }

    /// The obligations still owed, in canonical order. Empty is the claim.
    #[must_use]
    pub fn outstanding(&self) -> &[Obligation] {
        &self.outstanding
    }

    /// Whether nothing was owed and the counters agreed.
    #[must_use]
    pub fn is_balanced(&self) -> bool {
        self.outstanding.is_empty() && self.opened == self.discharged
    }
}

impl fmt::Display for LedgerSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "opened={} discharged={} outstanding=[",
            self.opened, self.discharged
        )?;
        for (index, obligation) in self.outstanding.iter().enumerate() {
            if index > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{obligation}")?;
        }
        f.write_str("]")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_the_same_obligation_twice_does_not_double_count_it() {
        let mut ledger = Ledger::new();
        let obligation = Obligation::worker_termination(WorkerId::at(0));
        assert!(ledger.open(obligation));
        assert!(!ledger.open(obligation), "linearity: one subject, one debt");
        assert_eq!(ledger.opened(), 1);
        assert!(ledger.discharge(obligation));
        assert!(ledger.is_balanced());
    }

    #[test]
    fn discharging_an_obligation_nobody_opened_is_refused() {
        let mut ledger = Ledger::new();
        assert!(!ledger.discharge(Obligation::cancellation_finalization(RegionId::at(0))));
        assert_eq!(ledger.discharged(), 0);
        assert!(ledger.is_balanced(), "a refused discharge changes nothing");
    }

    #[test]
    fn a_ledger_holding_one_obligation_is_not_balanced_and_names_it() {
        let mut ledger = Ledger::new();
        ledger.open(Obligation::child_region_quiescence(RegionId::at(2)));
        assert!(!ledger.is_balanced());
        let summary = ledger.summary();
        assert_eq!(summary.outstanding().len(), 1);
        assert_eq!(
            summary.outstanding()[0].kind(),
            ObligationKind::ChildRegionQuiescence
        );
        assert_eq!(
            summary.to_string(),
            "opened=1 discharged=0 outstanding=[child-region-quiescence:r2]"
        );
    }

    #[test]
    fn outstanding_obligations_render_in_one_canonical_order() {
        let mut ledger = Ledger::new();
        for worker in [5_u32, 1, 3] {
            ledger.open(Obligation::worker_termination(WorkerId::at(worker)));
        }
        ledger.open(Obligation::provisional_publication(WorkerId::at(4)));
        let rendered: Vec<String> = ledger
            .outstanding()
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            rendered,
            vec![
                "worker-termination:w1",
                "worker-termination:w3",
                "worker-termination:w5",
                "provisional-publication:w4",
            ]
        );
    }
}
