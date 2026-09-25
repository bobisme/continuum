//! What a reduced check is asked, what it may spend, and what it answers.
//!
//! The vocabulary is the assurance result's three-way verdict with a typed reason on
//! the inconclusive arm (RFC 0010 correction 1, INV-008): an outcome is never a
//! `bool`, reaching a declared bound is [`Unresolved::ResourceExhausted`] and never a
//! pass, and an evaluation failure is [`Unresolved::EngineError`] and never a verdict.
//! The shapes restate `continuum-engine-reference`'s `checking` vocabulary on this
//! side of the differential without importing it: docs/33 pairs the DPOR reducer with
//! an *unreduced differential oracle*, and a reducer that linked the oracle's decision
//! code could not be audited by it.

use core::fmt;
use std::collections::BTreeSet;

use continuum_model_core::definedness::{Guarded, definedness_base};
use continuum_model_core::ident::Ident;
use continuum_model_core::model::{EvaluationError, Model, State};

use crate::witness::ReductionWitness;

// ---------------------------------------------------------------------------
// bounds
// ---------------------------------------------------------------------------

/// The declared budget of one reduced check. There is no default: a default would be
/// a cap nobody wrote down (the reference engine's `Bounds` makes the same choice).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    states: usize,
    transitions: u64,
    depth: usize,
    work: u64,
}

impl Bounds {
    /// A budget of `states` stored states (and as many visits — a state entered with a
    /// sleep set), `transitions` explored transitions, a
    /// search stack at most `depth` frames deep, and `work` abstract work units.
    ///
    /// A work unit is one expression node evaluated, one label examined, or one
    /// machine word of a label set combined; every unit is charged before the work it
    /// pays for is done, so an untrusted model cannot make the search do work it has
    /// not paid for (charge-before-work).
    #[must_use]
    pub const fn new(states: usize, transitions: u64, depth: usize, work: u64) -> Self {
        Self {
            states,
            transitions,
            depth,
            work,
        }
    }

    /// The stored-state bound.
    #[must_use]
    pub const fn states(self) -> usize {
        self.states
    }

    /// The explored-transition bound.
    #[must_use]
    pub const fn transitions(self) -> u64 {
        self.transitions
    }

    /// The search-stack bound.
    #[must_use]
    pub const fn depth(self) -> usize {
        self.depth
    }

    /// The work bound.
    #[must_use]
    pub const fn work(self) -> u64 {
        self.work
    }
}

/// Which declared bound stopped a check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Bound {
    /// The stored-state bound.
    States,
    /// The stored-state bound, reached by visits (a state entered with a sleep set)
    /// before it was reached by states.
    Visits,
    /// The explored-transition bound.
    Transitions,
    /// The search-stack bound.
    Depth,
    /// The work bound.
    Work,
}

impl Bound {
    /// The bound's name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::States => "states",
            Self::Visits => "visits",
            Self::Transitions => "transitions",
            Self::Depth => "depth",
            Self::Work => "work",
        }
    }
}

impl fmt::Display for Bound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// obligations
// ---------------------------------------------------------------------------

/// What the caller's completion policy makes of a state with no enabled action. The
/// same two-arm projection of the intent contract's `completion_policy` that the
/// reference engine's `DeadlockPolicy` documents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeadlockPolicy {
    /// A terminal state is a defect (`deadlock-violation`).
    Defect,
    /// A terminal state is legitimate (`stutter-forever`, `finite-trace-only`).
    Allowed,
}

/// The finite safety obligations of one check: which predicates are invariants, and
/// the deadlock policy. These are the observers the reduction is scoped to (INV-013):
/// the variables the invariants read are the *visible* variables, and no transition
/// that writes one is ever left out of a reduced expansion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Obligations {
    invariants: BTreeSet<usize>,
    deadlock: DeadlockPolicy,
}

impl Obligations {
    /// No invariant yet, and `deadlock` as the completion policy.
    #[must_use]
    pub const fn new(deadlock: DeadlockPolicy) -> Self {
        Self {
            invariants: BTreeSet::new(),
            deadlock,
        }
    }

    /// Add the predicate at `index` as an invariant.
    #[must_use]
    pub fn invariant(mut self, index: usize) -> Self {
        self.invariants.insert(index);
        self
    }

    /// Every predicate the model declares, as an invariant.
    #[must_use]
    pub fn every_predicate(model: &Model, deadlock: DeadlockPolicy) -> Self {
        Self {
            invariants: (0..model.predicates().len()).collect(),
            deadlock,
        }
    }

    /// The invariant predicate indices, ascending.
    #[must_use]
    pub const fn invariants(&self) -> &BTreeSet<usize> {
        &self.invariants
    }

    /// The deadlock policy.
    #[must_use]
    pub const fn deadlock(&self) -> DeadlockPolicy {
        self.deadlock
    }
}

/// Why a check produced no report at all: the question is not about this model, or
/// the work bound cannot pay for asking it. The work reserve is checked first, so
/// under a bound below it a malformed obligation is also reported as
/// [`CheckError::WorkBelowObligations`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckError {
    /// The work bound cannot pay even for reading the obligations and holding one
    /// result per invariant ([`crate::OBLIGATION_UNITS`] each), so no report can be
    /// built within it. Refused before any obligation is read.
    WorkBelowObligations {
        /// How many invariants were asked for.
        invariants: usize,
        /// The declared work bound.
        work: u64,
    },
    /// An obligation names a predicate the model does not declare.
    UnknownPredicate {
        /// The index offered.
        index: usize,
        /// How many predicates the model declares.
        declared: usize,
    },
}

impl fmt::Display for CheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WorkBelowObligations { invariants, work } => write!(
                f,
                "a work bound of {work} cannot hold results for {invariants} invariants"
            ),
            Self::UnknownPredicate { index, declared } => write!(
                f,
                "invariant names predicate {index}; the model declares {declared}"
            ),
        }
    }
}

impl core::error::Error for CheckError {}

// ---------------------------------------------------------------------------
// outcomes
// ---------------------------------------------------------------------------

/// The closed three-way verdict (RFC 0010, `assurance-result.schema.json`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Verdict {
    /// The obligation holds on every reachable state.
    Established,
    /// A reachable state falsifies it.
    Refuted,
    /// Neither, with a typed reason.
    Inconclusive,
}

impl Verdict {
    /// The schema spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Established => "established",
            Self::Refuted => "refuted",
            Self::Inconclusive => "inconclusive",
        }
    }
}

/// Why the model layer could not answer at a reachable state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineFault {
    /// A guard or update could not be evaluated at a reachable state.
    Evaluation {
        /// The state.
        state: State,
        /// The action, when the failure belongs to one.
        action: Option<Ident>,
        /// What the model layer reported.
        source: Box<EvaluationError>,
    },
    /// The reducer broke one of its own invariants. Reported as data rather than a
    /// panic (the crate's no-panic covenant); never a verdict.
    Internal {
        /// Which invariant.
        what: &'static str,
    },
}

impl fmt::Display for EngineFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Evaluation {
                state,
                action: Some(action),
                source,
            } => write!(f, "action `{action}` at state {state}: {source}"),
            Self::Evaluation {
                state,
                action: None,
                source,
            } => write!(f, "state {state}: {source}"),
            Self::Internal { what } => write!(f, "reducer invariant broken: {what}"),
        }
    }
}

/// The typed reason on an inconclusive outcome (INV-008).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unresolved {
    /// A declared bound stopped the check before it settled the obligation. Never a
    /// pass: a reduced run that ran out of budget has shown nothing about the states
    /// it did not reach.
    ResourceExhausted {
        /// Which bound.
        tripped: Bound,
        /// How many states had been stored.
        stored: usize,
    },
    /// The model layer, or the reducer itself, could not answer.
    EngineError(EngineFault),
}

impl Unresolved {
    /// The `InconclusiveReason` member this is, spelled as the schemas spell it.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::ResourceExhausted { .. } => "ResourceExhausted",
            Self::EngineError(_) => "EngineError",
        }
    }
}

/// One step of a counterexample: the action that fired and the state it reached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceStep {
    action: usize,
    outcome: usize,
    target: State,
}

impl TraceStep {
    pub(crate) const fn new(action: usize, outcome: usize, target: State) -> Self {
        Self {
            action,
            outcome,
            target,
        }
    }

    /// The action index, in `Model::actions` order — the index `Model::successors`
    /// labels a step with, so a trace replays against any engine's successor rows.
    #[must_use]
    pub const fn action(&self) -> usize {
        self.action
    }

    /// Which of the action's outcomes fired, in declaration order.
    #[must_use]
    pub const fn outcome(&self) -> usize {
        self.outcome
    }

    /// The state reached.
    #[must_use]
    pub const fn target(&self) -> &State {
        &self.target
    }
}

/// A replayable execution from an initial state: the RFC 0004 "replayable
/// counterexample" output. It is the reduced search's own discovery path, so it is a
/// genuine execution but not necessarily a shortest one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trace {
    start: State,
    steps: Vec<TraceStep>,
}

impl Trace {
    pub(crate) const fn new(start: State, steps: Vec<TraceStep>) -> Self {
        Self { start, steps }
    }

    /// The initial state.
    #[must_use]
    pub const fn start(&self) -> &State {
        &self.start
    }

    /// The steps, in execution order.
    #[must_use]
    pub fn steps(&self) -> &[TraceStep] {
        &self.steps
    }

    /// The state the trace ends in.
    #[must_use]
    pub fn end(&self) -> &State {
        self.steps.last().map_or(&self.start, TraceStep::target)
    }
}

/// An undefined read at a reachable state: the typed outcome "undefined read in `X`"
/// of RFC 0003 ("Definedness"), never a verdict of a declared invariant (RFC 0013:
/// an undefined operation invalidates the model). Which predicates are definedness
/// predicates, and what each guards, is `continuum_model_core::definedness`'s
/// reading, used as is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndefinedRead {
    guard: usize,
    name: Ident,
    read: Guarded,
    trace: Option<Trace>,
    state: State,
}

impl UndefinedRead {
    pub(crate) const fn new(
        guard: usize,
        name: Ident,
        read: Guarded,
        state: State,
        trace: Option<Trace>,
    ) -> Self {
        Self {
            guard,
            name,
            read,
            trace,
            state,
        }
    }

    /// The index of the definedness predicate that is false at [`Self::state`].
    #[must_use]
    pub const fn guard(&self) -> usize {
        self.guard
    }

    /// Its declared name, `X#defined` (or a deeper `X#defined#defined`).
    #[must_use]
    pub const fn guard_name(&self) -> &Ident {
        &self.name
    }

    /// The action or predicate `X` whose read is undefined: the base of the chain.
    #[must_use]
    pub fn subject(&self) -> &str {
        definedness_base(self.name.as_str()).unwrap_or(self.name.as_str())
    }

    /// Whether `X` is an action (or a chain read fail-closed as one) or a predicate.
    #[must_use]
    pub const fn read(&self) -> Guarded {
        self.read
    }

    /// The first such state the reduced search stored (not necessarily a shallowest).
    #[must_use]
    pub const fn state(&self) -> &State {
        &self.state
    }

    /// The execution that reaches it, charged when the state was stored.
    #[must_use]
    pub const fn trace(&self) -> Option<&Trace> {
        self.trace.as_ref()
    }
}

/// The outcome for one invariant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvariantOutcome {
    /// No reachable state falsifies it. Only a complete reduced search produces this.
    Holds {
        /// How many states the reduced search stored.
        stored: usize,
    },
    /// A reachable state falsifies it. Genuine even in a bounded run: the state was
    /// reached by an executed path.
    Violated {
        /// The first falsifying state the search stored.
        state: State,
        /// The execution that reaches it. Its cost is charged when the state is
        /// stored, so a violation the search recorded always has its path; the
        /// `Option` is `None` only for a violation reported without a stored path,
        /// which this engine does not produce.
        trace: Option<Trace>,
    },
    /// A reachable state reads a value that is undefined: in some action (which
    /// invalidates every verdict over the model), or in this invariant. Precedence,
    /// the reference engine's and `continuum-incremental`'s: an evaluation error, then
    /// an undefined action read, then an undefined read in the invariant, then a
    /// violation, then the search's answer. Not a verdict: its verdict is
    /// [`Verdict::Inconclusive`].
    Undefined(Box<UndefinedRead>),
    /// Neither.
    Inconclusive(Unresolved),
}

impl InvariantOutcome {
    /// The three-way verdict.
    #[must_use]
    pub const fn verdict(&self) -> Verdict {
        match self {
            Self::Holds { .. } => Verdict::Established,
            Self::Violated { .. } => Verdict::Refuted,
            Self::Undefined(_) | Self::Inconclusive(_) => Verdict::Inconclusive,
        }
    }
}

/// One invariant's result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvariantResult {
    index: usize,
    name: Ident,
    outcome: InvariantOutcome,
}

impl InvariantResult {
    pub(crate) const fn new(index: usize, name: Ident, outcome: InvariantOutcome) -> Self {
        Self {
            index,
            name,
            outcome,
        }
    }

    /// The predicate index.
    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    /// The predicate name.
    #[must_use]
    pub const fn name(&self) -> &Ident {
        &self.name
    }

    /// The outcome.
    #[must_use]
    pub const fn outcome(&self) -> &InvariantOutcome {
        &self.outcome
    }
}

/// A terminal state and the execution that reaches it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deadlock {
    state: State,
    trace: Option<Trace>,
}

impl Deadlock {
    pub(crate) const fn new(state: State, trace: Option<Trace>) -> Self {
        Self { state, trace }
    }

    /// The terminal state.
    #[must_use]
    pub const fn state(&self) -> &State {
        &self.state
    }

    /// The execution that reaches it. A search can store many terminal states with
    /// long paths, so each path is charged when its terminal state is stored, like
    /// any other work; `None` is reserved for a path this engine does not produce.
    #[must_use]
    pub const fn trace(&self) -> Option<&Trace> {
        self.trace.as_ref()
    }
}

/// The deadlock half of a report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeadlockOutcome {
    /// [`DeadlockPolicy::Allowed`]: terminality is not judged.
    NotJudged {
        /// How many terminal states the reduced search stored.
        terminal: usize,
    },
    /// [`DeadlockPolicy::Defect`], and no reachable state is terminal. Only a complete
    /// reduced search produces this.
    Free {
        /// How many states the reduced search stored.
        stored: usize,
    },
    /// [`DeadlockPolicy::Defect`], and these reachable states are terminal, ascending.
    /// In a complete search the list is every reachable terminal state: a persistent
    /// set reduction preserves every deadlock.
    Deadlocked {
        /// The terminal states found.
        states: Vec<Deadlock>,
    },
    /// A reachable state reads a value undefined in some action, under either policy
    /// (an undefined action read invalidates the model, so its terminal states are not
    /// the CML model's).
    Undefined(Box<UndefinedRead>),
    /// Neither.
    Inconclusive(Unresolved),
}

impl DeadlockOutcome {
    /// The three-way verdict; `NotJudged` is established by policy.
    #[must_use]
    pub const fn verdict(&self) -> Verdict {
        match self {
            Self::NotJudged { .. } | Self::Free { .. } => Verdict::Established,
            Self::Deadlocked { .. } => Verdict::Refuted,
            Self::Undefined(_) | Self::Inconclusive(_) => Verdict::Inconclusive,
        }
    }
}

/// Whether the reduced search covered its whole reduced state space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Completeness {
    /// Every stored state was expanded to the reduction's rules, and every cycle of
    /// the reduced graph passes through a fully expanded state.
    Complete,
    /// A declared bound stopped the search.
    Exhausted(Bound),
    /// The model layer or the reducer could not answer.
    Faulted(EngineFault),
}

/// Coverage accounting (RFC 0004 "semantic coverage metrics").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Stats {
    /// States stored.
    pub states: usize,
    /// Transitions explored.
    pub transitions: u64,
    /// Labels in the model's label table (one per action outcome).
    pub labels: usize,
    /// States expanded with a reduced (persistent) set.
    pub reduced_states: usize,
    /// States fully expanded because no proper persistent set without a visible
    /// transition exists.
    pub full_exhaustive: usize,
    /// Visits fully expanded by the stack proviso.
    pub full_proviso: usize,
    /// Enabled labels left out of a persistent set, summed over visits.
    pub declined_persistent: u64,
    /// Enabled persistent-set labels skipped because they were asleep, summed over
    /// visits.
    pub declined_sleep: u64,
    /// Visits: stored states entered with a sleep set, the search graph's nodes. A
    /// state is entered again only with a sleep set no stored visit's set is a
    /// subset of.
    pub visits: usize,
    /// Work units spent.
    pub work: u64,
}

/// The reduced check's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub(crate) bounds: Bounds,
    pub(crate) obligations: Obligations,
    pub(crate) completeness: Completeness,
    pub(crate) invariants: Vec<InvariantResult>,
    pub(crate) deadlock: DeadlockOutcome,
    pub(crate) stats: Stats,
    pub(crate) witness: ReductionWitness,
    pub(crate) projections: BTreeSet<Vec<i64>>,
}

impl Report {
    /// The reduction algorithm and its version (RFC 0004 "Outputs").
    pub const ALGORITHM: &'static str = crate::ALGORITHM;
    /// The dependence oracle and its version (RFC 0004 "Outputs").
    pub const DEPENDENCE_ORACLE: &'static str = crate::DEPENDENCE_ORACLE;

    /// The bounds the check ran under (RFC 0004 "Outputs": bounds and assumptions).
    #[must_use]
    pub const fn bounds(&self) -> Bounds {
        self.bounds
    }

    /// The obligations the check answered; their read footprint is the visible set
    /// the reduction was scoped to (INV-013).
    #[must_use]
    pub const fn obligations(&self) -> &Obligations {
        &self.obligations
    }

    /// Whether the search was complete.
    #[must_use]
    pub const fn completeness(&self) -> &Completeness {
        &self.completeness
    }

    /// One result per invariant, ascending by predicate index.
    #[must_use]
    pub fn invariants(&self) -> &[InvariantResult] {
        &self.invariants
    }

    /// The deadlock outcome.
    #[must_use]
    pub const fn deadlock(&self) -> &DeadlockOutcome {
        &self.deadlock
    }

    /// Coverage accounting.
    #[must_use]
    pub const fn stats(&self) -> Stats {
        self.stats
    }

    /// The reduction witness, for [`crate::check_witness`]. It is an in-process
    /// value: no wire form or schema carries it yet, so the checker runs in the
    /// process that produced it.
    #[must_use]
    pub const fn witness(&self) -> &ReductionWitness {
        &self.witness
    }

    /// Every stored state projected onto the visible variables (those the invariants
    /// read), as vectors in canonical variable order restricted to the visible
    /// positions. A complete reduced search stores a state with every reachable
    /// projection: this is the set the C005 differential compares with the
    /// unreduced oracle's.
    #[must_use]
    pub const fn projections(&self) -> &BTreeSet<Vec<i64>> {
        &self.projections
    }

    /// The overall verdict: refuted if any obligation is refuted, else inconclusive
    /// if any is inconclusive, else established.
    #[must_use]
    pub fn verdict(&self) -> Verdict {
        let mut verdicts: Vec<Verdict> = self
            .invariants
            .iter()
            .map(|result| result.outcome.verdict())
            .collect();
        verdicts.push(self.deadlock.verdict());
        if verdicts.contains(&Verdict::Refuted) {
            Verdict::Refuted
        } else if verdicts.contains(&Verdict::Inconclusive) {
            Verdict::Inconclusive
        } else {
            Verdict::Established
        }
    }
}
