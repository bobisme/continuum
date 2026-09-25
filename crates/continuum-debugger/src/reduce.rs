//! Causal minimization: deletion and causal-closure reduction of a failing trace (START_HERE
//! PR 18, bn-3km4z).
//!
//! # What it reduces
//!
//! A **trace** is a finite sequence of events, numbered `0..n` in the order they were
//! observed, with a **causal order** ([`CausalOrder`]): each event's immediate
//! predecessors, all earlier than it. Happens-before is the transitive closure. A set of
//! events is a **configuration** when it is downward closed under happens-before (RFC
//! 0001's configuration, without conflict: a trace is one run, so it has no conflicting
//! events). The reduction never judges an event itself. A [`Replay`] oracle replays a
//! configuration under the trace's semantics and checks the failure; its verdict is the
//! only ground a candidate is kept on.
//!
//! The passes are generic. The PR-14 semantic journal instantiates them today
//! (`continuum-asupersync`'s `causal` module gives the order and the sub-journal the
//! lift replays). CIR (RFC 0001, PR 17) instantiates them when it lands: its events carry
//! their `causes`, which are the predecessor lists, and a CIR configuration replay is the
//! oracle.
//!
//! # The two passes
//!
//! 1. **Causal closure** ([`closure_pass`]): replay the whole trace, take the events the
//!    violated property observes (the oracle's **witnesses**), and keep exactly their
//!    happens-before downset. The downset is replayed; the pass keeps it only when the
//!    failure reproduces. By construction the pass never drops an event with a
//!    happens-before path to a witness, and drops every other one.
//! 2. **Deletion** ([`deletion_pass`]): delta debugging (Zeller and Hildebrandt's
//!    `ddmin`, subsets then complements, granularity doubling) over a configuration,
//!    in one of two candidate spaces ([`Deletion`]):
//!    - over **configurations**: a candidate removes a chunk together with its causal
//!      future and its atoms inside the current set, or keeps a chunk together with its
//!      causal past, so every candidate is a configuration again (RFC 0028's minimality
//!      soundness: removal preserves downward closure; RFC 0029's reverse step removes
//!      maximal events). The pass ends when no single event's removal, with its
//!      future, still fails: causally minimal, 1-minimal among configurations;
//!    - over **atoms**: classic `ddmin`, a candidate is any union of atoms and the
//!      replay alone decides. Smaller cores, but only as faithful as the oracle: a
//!      replay that checks the substrate's semantics and not the program's accepts
//!      subsets the program cannot produce, and may drop the mechanism (the register
//!      instantiation shows it). Its cores record no causal closure unless the check
//!      on the final set passes.
//!
//! [`minimize`] runs closure then deletion, the order research/26's pipeline gives
//! (slice, then minimize while preserving failure).
//!
//! # Replay, never assumption
//!
//! Every kept set was replayed and failed. The input is replayed first; an input that
//! does not fail is a typed refusal, not an empty core. A candidate the oracle cannot
//! replay ([`Replayed::NotReplayable`]) is never kept, whatever else holds of it.
//!
//! # Budget, charged before work (INV-008)
//!
//! [`Budget`] bounds replays and work units. Each replay is charged before the oracle
//! runs, and each candidate's construction is charged its predicted cost (trace length
//! plus edges) before it is built. A budget that runs out is the typed
//! [`Reduction::Inconclusive`] with reason [`InconclusiveReason::ResourceExhausted`] and
//! the best replay-validated configuration so far, never a success and never a claim of
//! minimality.
//!
//! # Determinism (INV-006)
//!
//! Nothing here reads a clock, a random source, an address or a hash map's order. The
//! passes are pure functions of the order, the oracle's verdicts and the budget, so
//! identical inputs give identical cores, transcripts and spending.
//!
//! # Guarantees a core records
//!
//! [`Guarantee`] names what was checked, not what was hoped: `ReplayPreserving` (the
//! core itself was replayed and failed), `CausallyClosed` (checked with
//! [`CausalOrder::is_down_closed`] on the final set, when the budget pays for the walk),
//! and one minimality token when the deletion pass finished and every final single-unit
//! removal was replayed, or memoized from an earlier replay, and did not fail:
//! `CausallyMinimal` over configurations (RFC 0028's class), `OneMinimal` over atoms
//! whose kept atoms are single events (docs/38's 1-minimal), `AtomMinimal` over atoms
//! otherwise. A removal the oracle could not decide (an INV-008 inconclusive) is not a
//! removal that did not fail: the pass then ends [`PassEnd::Undecided`] and claims no
//! minimality. An oracle that says a candidate fails but names no witness inside it
//! breaks its contract; the reduction stops, inconclusive with
//! [`InconclusiveReason::EngineError`]. No cardinality minimality, and no explanation
//! minimality (RFC 0028 correction 5), is claimed.

use std::collections::{BTreeMap, BTreeSet};

use continuum_value::assurance::InconclusiveReason;

/// A trace's causal order: each event's immediate predecessors, and its atoms.
///
/// An **atom** is a set of events that stand or fall together: the trace's producer
/// splits one semantic operation into several events, and the semantics judges a
/// configuration that holds only part of it as truncated (the PR-14 journal's region
/// teardown, for example: a cancellation request, its drain and its finalization are one
/// substrate operation). A configuration here is downward closed under happens-before
/// and closed under atom membership. Every event not in a declared atom is an atom of
/// its own. A CIR event is atomic, so a CIR trace declares no atoms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CausalOrder {
    preds: Vec<Vec<usize>>,
    succs: Vec<Vec<usize>>,
    /// Each event's atom, as an index into `atoms`; `usize::MAX` for a singleton.
    atom_of: Vec<usize>,
    atoms: Vec<Vec<usize>>,
    edges: usize,
}

/// Why predecessor lists are not a causal order over a trace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderError {
    /// A predecessor is not earlier than its event: the lists must agree with the trace
    /// order, which also makes the order acyclic.
    PredecessorNotEarlier {
        /// The event.
        event: usize,
        /// The offending predecessor.
        predecessor: usize,
    },
    /// An atom names an event outside the trace.
    AtomOutOfRange {
        /// The event.
        event: usize,
    },
    /// Two atoms share an event.
    AtomsOverlap {
        /// The event.
        event: usize,
    },
}

const SINGLETON: usize = usize::MAX;

impl CausalOrder {
    /// The order whose immediate predecessors are `preds[i]` for event `i`, with no
    /// atoms. Lists are sorted and deduplicated.
    ///
    /// # Errors
    ///
    /// [`OrderError::PredecessorNotEarlier`] for a predecessor at or after its event.
    pub fn from_predecessors(preds: Vec<Vec<usize>>) -> Result<Self, OrderError> {
        Self::with_atoms(preds, Vec::new())
    }

    /// As [`Self::from_predecessors`], with `atoms`: disjoint sets of events that stand
    /// or fall together. An atom of fewer than two events is ignored.
    ///
    /// # Errors
    ///
    /// As for [`Self::from_predecessors`]; [`OrderError::AtomOutOfRange`] and
    /// [`OrderError::AtomsOverlap`] for malformed atoms.
    pub fn with_atoms(preds: Vec<Vec<usize>>, atoms: Vec<Vec<usize>>) -> Result<Self, OrderError> {
        let n = preds.len();
        let mut out = Vec::with_capacity(n);
        let mut succs: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut edges = 0_usize;
        for (event, mut list) in preds.into_iter().enumerate() {
            list.sort_unstable();
            list.dedup();
            if let Some(&predecessor) = list.iter().find(|&&p| p >= event) {
                return Err(OrderError::PredecessorNotEarlier { event, predecessor });
            }
            for &p in &list {
                succs[p].push(event);
            }
            edges = edges.saturating_add(list.len());
            out.push(list);
        }
        let mut atom_of = vec![SINGLETON; n];
        let mut kept_atoms = Vec::new();
        for mut atom in atoms {
            atom.sort_unstable();
            atom.dedup();
            if atom.len() < 2 {
                continue;
            }
            let id = kept_atoms.len();
            for &e in &atom {
                match atom_of.get_mut(e) {
                    None => return Err(OrderError::AtomOutOfRange { event: e }),
                    Some(slot) if *slot != SINGLETON => {
                        return Err(OrderError::AtomsOverlap { event: e });
                    }
                    Some(slot) => *slot = id,
                }
            }
            kept_atoms.push(atom);
        }
        Ok(Self {
            preds: out,
            succs,
            atom_of,
            atoms: kept_atoms,
            edges,
        })
    }

    /// The trace's length.
    #[must_use]
    pub fn len(&self) -> usize {
        self.preds.len()
    }

    /// Whether the trace is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.preds.is_empty()
    }

    /// Event `event`'s immediate predecessors, ascending; empty for an index outside
    /// the trace.
    #[must_use]
    pub fn predecessors(&self, event: usize) -> &[usize] {
        self.preds.get(event).map_or(&[], Vec::as_slice)
    }

    /// The other members of `event`'s atom, ascending with it; `[event]` alone for a
    /// singleton, empty for an index outside the trace.
    #[must_use]
    pub fn atom(&self, event: usize) -> Vec<usize> {
        match self.atom_of.get(event) {
            None => Vec::new(),
            Some(&SINGLETON) => vec![event],
            Some(&id) => self.atoms[id].clone(),
        }
    }

    /// How many predecessor edges the order has.
    #[must_use]
    pub const fn edges(&self) -> usize {
        self.edges
    }

    /// The work one walk over the order costs, in units: its events, its edges in both
    /// directions, and its atoms' members. The charge every candidate construction is
    /// billed before it runs.
    #[must_use]
    pub fn walk_cost(&self) -> u64 {
        let members: usize = self.atoms.iter().map(Vec::len).sum();
        u64::try_from(
            self.preds
                .len()
                .saturating_add(self.edges.saturating_mul(2))
                .saturating_add(members),
        )
        .unwrap_or(u64::MAX)
    }

    /// Mark `start` and everything reachable from it over `next` and atom membership.
    /// Each atom is walked once, so the walk costs [`Self::walk_cost`] at most.
    fn reach(&self, mark: &mut [bool], start: &[usize], next: &[Vec<usize>]) {
        let mut atom_done = vec![false; self.atoms.len()];
        let mut stack: Vec<usize> = Vec::new();
        for &s in start {
            if s < mark.len() && !mark[s] {
                mark[s] = true;
                stack.push(s);
            }
        }
        while let Some(e) = stack.pop() {
            let atom: &[usize] = match self.atom_of[e] {
                SINGLETON => &[],
                id if atom_done[id] => &[],
                id => {
                    atom_done[id] = true;
                    &self.atoms[id]
                }
            };
            for &x in next[e].iter().chain(atom) {
                if !mark[x] {
                    mark[x] = true;
                    stack.push(x);
                }
            }
        }
    }

    /// The smallest configuration holding `seeds`: every seed in the trace, every event
    /// with a happens-before path to one, and every atom any of them belongs to, closed
    /// again. Ascending. Seeds outside the trace are ignored. One walk, so it costs
    /// [`Self::walk_cost`] at most.
    #[must_use]
    pub fn down_closure(&self, seeds: &[usize]) -> Vec<usize> {
        let mut mark = vec![false; self.preds.len()];
        self.reach(&mut mark, seeds, &self.preds);
        (0..self.preds.len()).filter(|&i| mark[i]).collect()
    }

    /// Whether `set` (ascending or not) is a configuration: every member is an event of
    /// the trace, and every predecessor and every atom-mate of every member is a member.
    #[must_use]
    pub fn is_down_closed(&self, set: &[usize]) -> bool {
        let mut member = vec![false; self.preds.len()];
        for &s in set {
            match member.get_mut(s) {
                Some(m) => *m = true,
                None => return false,
            }
        }
        set.iter()
            .all(|&s| self.preds[s].iter().all(|&p| member[p]))
            && self.atoms_whole(&member)
    }

    /// Whether `set` is closed under atom membership alone: every member is an event of
    /// the trace, and every atom-mate of a member is a member. Deletion over atoms
    /// starts from such a set.
    #[must_use]
    pub fn is_atom_closed(&self, set: &[usize]) -> bool {
        let mut member = vec![false; self.preds.len()];
        for &s in set {
            match member.get_mut(s) {
                Some(m) => *m = true,
                None => return false,
            }
        }
        self.atoms_whole(&member)
    }

    /// Whether every atom is wholly in or wholly out of `member`. Each atom is read
    /// once.
    fn atoms_whole(&self, member: &[bool]) -> bool {
        self.atoms.iter().all(|atom| {
            let first = member[atom[0]];
            atom.iter().all(|&m| member[m] == first)
        })
    }

    /// Whether every member of `set` is an atom of its own.
    fn singleton_atoms(&self, set: &[usize]) -> bool {
        set.iter().all(|&e| self.atom_of.get(e) == Some(&SINGLETON))
    }

    /// The work one candidate costs, in units: one walk, plus a logarithmic factor per
    /// event for the ordered sets and the memo the deletion pass keeps.
    #[must_use]
    pub fn candidate_cost(&self) -> u64 {
        let n = u64::try_from(self.preds.len()).unwrap_or(u64::MAX);
        let log = u64::from(u64::BITS - n.leading_zeros()).saturating_add(1);
        self.walk_cost().saturating_add(n.saturating_mul(log))
    }

    /// `set` with every atom of its members, ascending.
    fn with_atoms_of(&self, set: &[usize]) -> Vec<usize> {
        let mut out: BTreeSet<usize> = BTreeSet::new();
        for &e in set {
            match self.atom_of.get(e) {
                Some(&SINGLETON) => {
                    out.insert(e);
                }
                Some(&id) => out.extend(self.atoms[id].iter().copied()),
                None => {}
            }
        }
        out.into_iter().collect()
    }

    /// `current` (a configuration) without `seeds` and without everything a seed
    /// happens before or shares an atom with, closed again: the largest configuration
    /// inside `current` holding no seed. One walk.
    fn without_future(&self, current: &[usize], seeds: &[usize]) -> Vec<usize> {
        let mut gone = vec![false; self.preds.len()];
        self.reach(&mut gone, seeds, &self.succs);
        current.iter().copied().filter(|&i| !gone[i]).collect()
    }
}

/// How the deletion pass forms its candidates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Deletion {
    /// Every candidate is a configuration: a removed chunk takes its causal future and
    /// its atoms with it, a kept chunk its causal past. The core stays causally closed
    /// against the input's order, and is 1-minimal among configurations.
    Configurations,
    /// A candidate is any union of atoms: a removed chunk takes only its atoms with it,
    /// and the replay alone decides. Classic `ddmin` over atoms, research/26's fallback
    /// lane ("1-minimal delta debugging only"). The core is 1-minimal among unions of
    /// atoms; its causal closure against the input's order is checked, not kept, since
    /// the declared order over-approximates what the semantics needs.
    Atoms,
}

/// A replay's refusal: why a configuration could not be judged at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotReplayable {
    /// The semantics refuses the configuration: it is not a run (for example, the lift
    /// finds a nonconforming event). The string is the oracle's rendering.
    Nonconforming(String),
    /// The semantics cannot decide it: an INV-008 reason, with the oracle's rendering.
    Inconclusive(InconclusiveReason, String),
}

/// What replaying a configuration showed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Replayed {
    /// The failure reproduces. `witnesses` are the events the violated property
    /// observes, as trace indices; each is a member of the replayed configuration, and
    /// there is at least one.
    Fails {
        /// The property's observed events.
        witnesses: Vec<usize>,
    },
    /// The configuration is a run and the failure does not reproduce.
    Holds,
    /// The configuration could not be replayed.
    NotReplayable(NotReplayable),
}

/// The oracle the reduction consults. It replays a configuration under the trace's
/// semantics and checks the one failure the reduction preserves.
pub trait Replay {
    /// Replay `kept` (ascending trace indices) and check the failure. Deletion over
    /// configurations passes only configurations of the trace; deletion over atoms
    /// passes any union of atoms, which the oracle judges like any other candidate.
    fn replay(&mut self, kept: &[usize]) -> Replayed;
}

impl<F: FnMut(&[usize]) -> Replayed> Replay for F {
    fn replay(&mut self, kept: &[usize]) -> Replayed {
        self(kept)
    }
}

/// A reduction budget: replays and work units. Charged before the work it pays for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    replays: u64,
    work: u64,
}

/// What a reduction spent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Spent {
    /// Replays run.
    pub replays: u64,
    /// Work units charged.
    pub work: u64,
}

/// A budget ran out: which part.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exhausted {
    /// No replay left.
    Replays,
    /// Not enough work units for the next charge.
    Work,
}

impl Budget {
    /// At most `replays` replays and `work` work units.
    #[must_use]
    pub const fn new(replays: u64, work: u64) -> Self {
        Self { replays, work }
    }

    fn charge_replay(&mut self, spent: &mut Spent) -> Result<(), Exhausted> {
        if self.replays == 0 {
            return Err(Exhausted::Replays);
        }
        self.replays -= 1;
        spent.replays = spent.replays.saturating_add(1);
        Ok(())
    }

    fn charge_work(&mut self, units: u64, spent: &mut Spent) -> Result<(), Exhausted> {
        if self.work < units {
            return Err(Exhausted::Work);
        }
        self.work -= units;
        spent.work = spent.work.saturating_add(units);
        Ok(())
    }
}

/// A property of a core that the reduction checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Guarantee {
    /// The core itself was replayed and the failure reproduced.
    ReplayPreserving,
    /// The core is downward closed under happens-before, checked on the final set.
    CausallyClosed,
    /// Deletion over configurations finished and decided every final removal: no
    /// kept event's removal, with its causal future and atoms, still fails. RFC 0028's
    /// `CausallyMinimal`, a minimal configuration under causal closure (local, not
    /// cardinality).
    CausallyMinimal,
    /// Deletion over atoms finished and decided every final removal, and every kept
    /// atom is a single event: no single kept event's removal still fails. docs/38's
    /// 1-minimal.
    OneMinimal,
    /// Deletion over atoms finished and decided every final removal, and some kept atom
    /// has several events: no single kept atom's removal still fails. 1-minimal over
    /// atoms, weaker than [`Self::OneMinimal`].
    AtomMinimal,
}

/// A replay-validated configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Core {
    /// Its events, ascending trace indices.
    pub events: Vec<usize>,
    /// The witnesses its own replay reported.
    pub witnesses: Vec<usize>,
    /// What was checked of it, ascending.
    pub guarantees: Vec<Guarantee>,
}

/// Which pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Pass {
    /// Causal closure of the witnesses.
    Closure,
    /// Delta-debugging deletion, with its candidate space.
    Deletion(Deletion),
}

/// How one pass ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PassEnd {
    /// It produced a replay-validated configuration no larger than its input.
    Done,
    /// The closure did not reproduce the failure: the pass keeps its input. Evidence
    /// that the declared order under-approximates the semantics, or that the witnesses
    /// miss an observed event; never a silent loss.
    ClosureNotReplayPreserving(Replayed),
    /// The budget ran out inside the pass.
    Exhausted(Exhausted),
    /// Deletion finished, but the oracle could not decide some final single-unit
    /// removals (an INV-008 inconclusive): no minimality is claimed.
    Undecided {
        /// Final removals whose replay was inconclusive.
        removals: u64,
    },
    /// The oracle broke [`Replayed::Fails`]'s contract (no witness, or one outside the
    /// replayed set). The pass stops; the reduction is inconclusive with reason
    /// [`InconclusiveReason::EngineError`].
    OracleContract,
}

/// One pass's record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassRecord {
    /// Which pass.
    pub pass: Pass,
    /// Events in its input.
    pub before: usize,
    /// Events in its result.
    pub after: usize,
    /// Replays it ran.
    pub replays: u64,
    /// Candidates that replayed and did not fail.
    pub held: u64,
    /// Candidates the semantics refused: not a run, so no witness.
    pub nonconforming: u64,
    /// Candidates the oracle could not decide (INV-008 inconclusive).
    pub inconclusive: u64,
    /// How it ended.
    pub end: PassEnd,
}

/// Why a reduction did not start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The trace is empty.
    EmptyTrace,
    /// The starting set is empty.
    EmptyStart,
    /// The starting set is not a configuration of the trace (deletion over atoms: not
    /// closed under atoms).
    NotAConfiguration,
    /// The input replays and does not fail.
    InputHolds,
    /// The input cannot be replayed.
    InputNotReplayable(NotReplayable),
    /// The oracle reported a failure with no witness, or a witness outside the replayed
    /// configuration: it broke [`Replayed::Fails`]'s contract.
    WitnessContract,
}

/// A reduction's result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reduction {
    /// The reduction finished. The core's guarantees say what was checked: a closure
    /// that did not reproduce the failure keeps its input (the transcript says so), and
    /// a deletion with undecided removals claims no minimality.
    Reduced {
        /// The core.
        core: Core,
        /// Each pass, in order.
        transcript: Vec<PassRecord>,
        /// What it spent.
        spent: Spent,
    },
    /// The reduction stopped early. Not a success, and no minimality is claimed.
    Inconclusive {
        /// [`InconclusiveReason::ResourceExhausted`] when the budget ran out;
        /// [`InconclusiveReason::EngineError`] when the oracle broke its contract.
        reason: InconclusiveReason,
        /// The smallest replay-validated configuration reached, if any replay failed.
        best: Option<Core>,
        /// Each pass, in order.
        transcript: Vec<PassRecord>,
        /// What it spent.
        spent: Spent,
    },
    /// The reduction did not start.
    Refused(Refusal),
}

impl Reduction {
    /// The finished core, if the reduction finished.
    #[must_use]
    pub const fn core(&self) -> Option<&Core> {
        match self {
            Self::Reduced { core, .. } => Some(core),
            _ => None,
        }
    }
}

/// The running state of one reduction.
struct Run<'a, R: Replay> {
    order: &'a CausalOrder,
    oracle: &'a mut R,
    budget: Budget,
    spent: Spent,
    transcript: Vec<PassRecord>,
}

/// Why a pass stopped before it finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stop {
    /// The budget ran out.
    Exhausted(Exhausted),
    /// The oracle broke its contract.
    Contract,
}

/// A pass's result: the set and its witnesses, and for deletion whether its final
/// removals were all decided; or why it stopped, with the best set and its witnesses.
type PassResult = Result<(Vec<usize>, Vec<usize>, bool), (Stop, Vec<usize>, Vec<usize>)>;

/// How a candidate that was not kept fared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rejection {
    Holds,
    Nonconforming,
    Inconclusive,
}

enum Step {
    Kept(Vec<usize>),
    Rejected(Rejection, Replayed),
    Contract,
}

fn valid_witnesses(kept: &[usize], witnesses: &[usize]) -> bool {
    !witnesses.is_empty() && witnesses.iter().all(|w| kept.binary_search(w).is_ok())
}

/// The work validating `witnesses` against `kept` costs: a binary search each.
fn witness_cost(kept: &[usize], witnesses: &[usize]) -> u64 {
    let n = u64::try_from(kept.len()).unwrap_or(u64::MAX);
    let log = u64::from(u64::BITS - n.leading_zeros()).saturating_add(1);
    u64::try_from(witnesses.len())
        .unwrap_or(u64::MAX)
        .saturating_mul(log)
}

impl<R: Replay> Run<'_, R> {
    /// Replay `kept`, charged first; the witnesses' validation is charged before it
    /// runs too. A failure whose witnesses break the contract is [`Step::Contract`],
    /// never a rejected candidate: the oracle said it fails.
    fn replay(&mut self, kept: &[usize]) -> Result<Step, Exhausted> {
        self.budget.charge_replay(&mut self.spent)?;
        Ok(match self.oracle.replay(kept) {
            Replayed::Fails { witnesses } => {
                self.budget
                    .charge_work(witness_cost(kept, &witnesses), &mut self.spent)?;
                if valid_witnesses(kept, &witnesses) {
                    let mut w = witnesses;
                    w.sort_unstable();
                    w.dedup();
                    Step::Kept(w)
                } else {
                    Step::Contract
                }
            }
            Replayed::Holds => Step::Rejected(Rejection::Holds, Replayed::Holds),
            Replayed::NotReplayable(why) => {
                let kind = match why {
                    NotReplayable::Nonconforming(_) => Rejection::Nonconforming,
                    NotReplayable::Inconclusive(..) => Rejection::Inconclusive,
                };
                Step::Rejected(kind, Replayed::NotReplayable(why))
            }
        })
    }

    /// The core of `events`, with the guarantees checked on it. Causal closure is
    /// checked only when the budget still pays for the walk; otherwise it is not
    /// claimed. `minimal` names the deletion mode whose final removals were all
    /// decided, if any.
    fn core(
        &mut self,
        events: Vec<usize>,
        witnesses: Vec<usize>,
        minimal: Option<Deletion>,
    ) -> Core {
        let mut guarantees = vec![Guarantee::ReplayPreserving];
        let paid = self
            .budget
            .charge_work(self.order.walk_cost(), &mut self.spent)
            .is_ok();
        if paid && self.order.is_down_closed(&events) {
            guarantees.push(Guarantee::CausallyClosed);
        }
        match minimal {
            Some(Deletion::Configurations) => guarantees.push(Guarantee::CausallyMinimal),
            Some(Deletion::Atoms) if self.order.singleton_atoms(&events) => {
                guarantees.push(Guarantee::OneMinimal);
            }
            Some(Deletion::Atoms) => guarantees.push(Guarantee::AtomMinimal),
            None => {}
        }
        guarantees.sort_unstable();
        Core {
            events,
            witnesses,
            guarantees,
        }
    }

    /// Replay the starting set; its witnesses, or the refusal.
    fn start(&mut self, set: &[usize]) -> Result<Result<Vec<usize>, Refusal>, Exhausted> {
        Ok(match self.replay(set)? {
            Step::Kept(w) => Ok(w),
            Step::Contract => Err(Refusal::WitnessContract),
            Step::Rejected(_, Replayed::NotReplayable(why)) => {
                Err(Refusal::InputNotReplayable(why))
            }
            Step::Rejected(..) => Err(Refusal::InputHolds),
        })
    }

    fn record(
        &mut self,
        pass: Pass,
        before: usize,
        after: usize,
        replays_before: u64,
        counts: [u64; 3],
        end: PassEnd,
    ) {
        self.transcript.push(PassRecord {
            pass,
            before,
            after,
            replays: self.spent.replays - replays_before,
            held: counts[0],
            nonconforming: counts[1],
            inconclusive: counts[2],
            end,
        });
    }

    /// The closure pass over `set` (validated, failing, with `witnesses`).
    fn closure(&mut self, set: Vec<usize>, witnesses: Vec<usize>) -> PassResult {
        let before = set.len();
        let r0 = self.spent.replays;
        if let Err(e) = self
            .budget
            .charge_work(self.order.walk_cost(), &mut self.spent)
        {
            self.record(
                Pass::Closure,
                before,
                before,
                r0,
                [0; 3],
                PassEnd::Exhausted(e),
            );
            return Err((Stop::Exhausted(e), set, witnesses));
        }
        let closure = self.order.down_closure(&witnesses);
        // The witnesses are members of a configuration, so their downset is inside it.
        if closure.len() == set.len() {
            self.record(Pass::Closure, before, before, r0, [0; 3], PassEnd::Done);
            return Ok((set, witnesses, false));
        }
        match self.replay(&closure) {
            Err(e) => {
                self.record(
                    Pass::Closure,
                    before,
                    before,
                    r0,
                    [0; 3],
                    PassEnd::Exhausted(e),
                );
                Err((Stop::Exhausted(e), set, witnesses))
            }
            Ok(Step::Contract) => {
                self.record(
                    Pass::Closure,
                    before,
                    before,
                    r0,
                    [0; 3],
                    PassEnd::OracleContract,
                );
                Err((Stop::Contract, set, witnesses))
            }
            Ok(Step::Kept(w)) => {
                self.record(
                    Pass::Closure,
                    before,
                    closure.len(),
                    r0,
                    [0; 3],
                    PassEnd::Done,
                );
                Ok((closure, w, false))
            }
            Ok(Step::Rejected(kind, why)) => {
                let counts = match kind {
                    Rejection::Holds => [1, 0, 0],
                    Rejection::Nonconforming => [0, 1, 0],
                    Rejection::Inconclusive => [0, 0, 1],
                };
                self.record(
                    Pass::Closure,
                    before,
                    before,
                    r0,
                    counts,
                    PassEnd::ClosureNotReplayPreserving(why),
                );
                Ok((set, witnesses, false))
            }
        }
    }

    /// The deletion pass over `set` (a validated, failing start), with candidates from
    /// `mode`.
    #[allow(clippy::too_many_lines)]
    fn deletion(&mut self, mode: Deletion, set: Vec<usize>, witnesses: Vec<usize>) -> PassResult {
        let before = set.len();
        let r0 = self.spent.replays;
        let mut current = set;
        let mut witnesses = witnesses;
        let mut counts = [0_u64; 3];
        // Candidates already replayed without failing, with how they fared: never
        // replayed twice. At most one entry per replay, each no longer than the trace,
        // and each lookup or insertion is inside the candidate's charge.
        let mut rejected: BTreeMap<Vec<usize>, Rejection> = BTreeMap::new();
        let mut n = 2_usize;
        let outcome: Result<u64, Stop> = loop {
            if current.len() < 2 {
                // One event left: its only removal leaves the empty set, which holds no
                // witness and so cannot fail.
                break Ok(0);
            }
            let chunks = split(&current, n);
            let mut progressed = false;
            let mut stopped = None;
            // Complement removals this round whose replay was inconclusive.
            let mut undecided = 0_u64;
            // Subsets first (keep one chunk with its past), then complements (remove
            // one chunk with its future).
            'phases: for complement in [false, true] {
                for chunk in &chunks {
                    // The candidate's construction and memo lookup: charged first.
                    if let Err(e) = self
                        .budget
                        .charge_work(self.order.candidate_cost(), &mut self.spent)
                    {
                        stopped = Some(Stop::Exhausted(e));
                        break 'phases;
                    }
                    let candidate = match (mode, complement) {
                        (Deletion::Configurations, true) => {
                            self.order.without_future(&current, chunk)
                        }
                        (Deletion::Configurations, false) => self.order.down_closure(chunk),
                        (Deletion::Atoms, true) => {
                            let gone = self.order.with_atoms_of(chunk);
                            current
                                .iter()
                                .copied()
                                .filter(|e| gone.binary_search(e).is_err())
                                .collect()
                        }
                        (Deletion::Atoms, false) => self.order.with_atoms_of(chunk),
                    };
                    // An empty candidate holds no witness and cannot fail.
                    if candidate.is_empty() || candidate.len() >= current.len() {
                        continue;
                    }
                    if let Some(&seen) = rejected.get(&candidate) {
                        if complement && seen == Rejection::Inconclusive {
                            undecided += 1;
                        }
                        continue;
                    }
                    match self.replay(&candidate) {
                        Err(e) => {
                            stopped = Some(Stop::Exhausted(e));
                            break 'phases;
                        }
                        Ok(Step::Contract) => {
                            stopped = Some(Stop::Contract);
                            break 'phases;
                        }
                        Ok(Step::Kept(w)) => {
                            current = candidate;
                            witnesses = w;
                            n = if complement { (n - 1).max(2) } else { 2 };
                            progressed = true;
                            break 'phases;
                        }
                        Ok(Step::Rejected(kind, _)) => {
                            match kind {
                                Rejection::Holds => counts[0] += 1,
                                Rejection::Nonconforming => counts[1] += 1,
                                Rejection::Inconclusive => {
                                    counts[2] += 1;
                                    if complement {
                                        undecided += 1;
                                    }
                                }
                            }
                            rejected.insert(candidate, kind);
                        }
                    }
                }
            }
            if let Some(stop) = stopped {
                break Err(stop);
            }
            if !progressed {
                if n >= current.len() {
                    // The final round: every single unit's removal was tried, fresh or
                    // from the memo. `undecided` counts the inconclusive ones.
                    break Ok(undecided);
                }
                n = n.saturating_mul(2).min(current.len());
            }
        };
        let end = match outcome {
            Ok(0) => PassEnd::Done,
            Ok(removals) => PassEnd::Undecided { removals },
            Err(Stop::Exhausted(e)) => PassEnd::Exhausted(e),
            Err(Stop::Contract) => PassEnd::OracleContract,
        };
        self.record(Pass::Deletion(mode), before, current.len(), r0, counts, end);
        match outcome {
            Ok(undecided) => Ok((current, witnesses, undecided == 0)),
            Err(stop) => Err((stop, current, witnesses)),
        }
    }
}

/// `set` split into `n` contiguous chunks (by position), sizes differing by at most one,
/// the larger ones first. `n` is clamped to `1..=set.len()`.
fn split(set: &[usize], n: usize) -> Vec<Vec<usize>> {
    let n = n.clamp(1, set.len().max(1));
    let base = set.len() / n;
    let extra = set.len() % n;
    let mut out = Vec::with_capacity(n);
    let mut at = 0;
    for k in 0..n {
        let size = base + usize::from(k < extra);
        out.push(set[at..at + size].to_vec());
        at += size;
    }
    out
}

fn stopped<R: Replay>(
    mut run: Run<'_, R>,
    stop: Stop,
    best: Option<(Vec<usize>, Vec<usize>)>,
) -> Reduction {
    let best = best.map(|(set, w)| run.core(set, w, None));
    Reduction::Inconclusive {
        reason: match stop {
            Stop::Exhausted(_) => InconclusiveReason::ResourceExhausted,
            Stop::Contract => InconclusiveReason::EngineError,
        },
        best,
        transcript: run.transcript,
        spent: run.spent,
    }
}

fn whole(order: &CausalOrder) -> Vec<usize> {
    (0..order.len()).collect()
}

/// Which passes a reduction runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Plan {
    Closure,
    Deletion(Deletion),
    Both(Deletion),
}

fn reduce<R: Replay>(
    order: &CausalOrder,
    start: Vec<usize>,
    oracle: &mut R,
    budget: Budget,
    plan: Plan,
) -> Reduction {
    if order.is_empty() {
        return Reduction::Refused(Refusal::EmptyTrace);
    }
    if start.is_empty() {
        return Reduction::Refused(Refusal::EmptyStart);
    }
    let mut run = Run {
        order,
        oracle,
        budget,
        spent: Spent::default(),
        transcript: Vec::new(),
    };
    // Validating the start is a sort and one walk: charged before either runs.
    let len = u64::try_from(start.len()).unwrap_or(u64::MAX);
    let sort = len.saturating_mul(u64::from(u64::BITS - len.leading_zeros()).saturating_add(1));
    if let Err(e) = run
        .budget
        .charge_work(order.walk_cost().saturating_add(sort), &mut run.spent)
    {
        return stopped(run, Stop::Exhausted(e), None);
    }
    let mut start = start;
    start.sort_unstable();
    start.dedup();
    let valid = match plan {
        Plan::Deletion(Deletion::Atoms) => order.is_atom_closed(&start),
        _ => order.is_down_closed(&start),
    };
    if !valid {
        return Reduction::Refused(Refusal::NotAConfiguration);
    }
    let witnesses = match run.start(&start) {
        Err(e) => return stopped(run, Stop::Exhausted(e), None),
        Ok(Err(refusal)) => return Reduction::Refused(refusal),
        Ok(Ok(w)) => w,
    };
    let (set, witnesses) = if matches!(plan, Plan::Deletion(_)) {
        (start, witnesses)
    } else {
        match run.closure(start, witnesses) {
            Ok((set, w, _)) => (set, w),
            Err((stop, set, w)) => return stopped(run, stop, Some((set, w))),
        }
    };
    let mode = match plan {
        Plan::Closure => {
            let core = run.core(set, witnesses, None);
            return Reduction::Reduced {
                core,
                transcript: run.transcript,
                spent: run.spent,
            };
        }
        Plan::Deletion(mode) | Plan::Both(mode) => mode,
    };
    match run.deletion(mode, set, witnesses) {
        Ok((set, w, decided)) => {
            let core = run.core(set, w, decided.then_some(mode));
            Reduction::Reduced {
                core,
                transcript: run.transcript,
                spent: run.spent,
            }
        }
        Err((stop, set, w)) => stopped(run, stop, Some((set, w))),
    }
}

/// The causal-closure pass alone, over the whole trace: replay it, keep the
/// happens-before downset of its witnesses (closed under atoms), and replay that.
#[must_use]
pub fn closure_pass<R: Replay>(order: &CausalOrder, oracle: &mut R, budget: Budget) -> Reduction {
    reduce(order, whole(order), oracle, budget, Plan::Closure)
}

/// The deletion pass alone, over the configuration `start` (the whole trace when it is
/// every index), with candidates from `mode`.
#[must_use]
pub fn deletion_pass<R: Replay>(
    order: &CausalOrder,
    start: Vec<usize>,
    mode: Deletion,
    oracle: &mut R,
    budget: Budget,
) -> Reduction {
    reduce(order, start, oracle, budget, Plan::Deletion(mode))
}

/// Closure, then deletion with candidates from `mode`, over the whole trace.
#[must_use]
pub fn minimize<R: Replay>(
    order: &CausalOrder,
    mode: Deletion,
    oracle: &mut R,
    budget: Budget,
) -> Reduction {
    reduce(order, whole(order), oracle, budget, Plan::Both(mode))
}
