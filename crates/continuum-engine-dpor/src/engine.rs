//! The reduced search: stateful depth-first exploration with persistent sets, sleep
//! sets, observer visibility, and the cycle proviso.
//!
//! See the crate documentation for the algorithm and its soundness argument. This
//! module is the producer; [`crate::checker`] is the independent consumer of the
//! witness it writes and shares none of its decision logic.

use std::collections::{BTreeMap, BTreeSet};

use continuum_model_core::definedness::Guarded;
use continuum_model_core::expr::Environment;
use continuum_model_core::model::{EvaluationError, Model, State};

use crate::bits::LabelSet;
use crate::budget::{Meter, bits, units};
use crate::footprint::Footprints;
use crate::report::{
    Bound, Bounds, CheckError, Completeness, Deadlock, DeadlockOutcome, DeadlockPolicy,
    EngineFault, InvariantOutcome, InvariantResult, Obligations, Report, Stats, Trace, TraceStep,
    UndefinedRead, Unresolved,
};
use crate::witness::{
    EdgeRecord, Expansion, FullReason, NodeRecord, ReductionWitness, VisitRecord,
};

// ---------------------------------------------------------------------------
// test-only fault seeding
// ---------------------------------------------------------------------------

/// Fault seeding for the C005 mutation campaign.
///
/// In a shipped build this is a zero-sized value whose every question answers the
/// sound way, as a `const fn`: no mutant exists outside `#[cfg(test)]`, so no shipped
/// code path can select one (the bone's "not in shipped code paths").
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Knobs {
    #[cfg(test)]
    pub(crate) mutant: Option<Mutant>,
    /// A charge left out, to show the unpaid-work check catches it (cr-1wuzry).
    #[cfg(test)]
    pub(crate) omit: Option<Omission>,
}

/// One charge-before-work site of cr-1wuzry round 2, which a test build can leave
/// unpaid to show that the meter's unpaid-work check fires there.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Omission {
    /// The per-invariant reserve taken before the obligations are read.
    Obligations,
    /// The state-index search in `enter`.
    IndexSearch,
    /// Collecting the proviso's extra labels.
    ProvisoCollect,
}

/// One seeded fault. Each names a place the sound algorithm inserts something, and
/// removes the insertion.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Mutant {
    /// Independence too coarse: the conflict relation keeps write/write overlap and
    /// drops every write/read dependence edge (a guard or right-hand side that reads
    /// a variable another label writes).
    DropWriteReadDependence,
    /// Skip the backtrack insertion for a disabled stubborn-set member: its necessary
    /// enabling set is not added, so a label that can enable it is left out.
    SkipEnablingInsertion,
    /// Skip the proviso's backtrack insertion: a visit whose edge closes a cycle onto
    /// the search stack is not expanded in full.
    SkipCycleProviso,
    /// Wrong sleep-set wakeup: a sleeping label is not woken when a dependent label
    /// fires, so the child inherits the parent's whole sleep set.
    NoSleepWakeup,
    /// Donate a label to the sleep set even when the region it leads to is still
    /// being explored (its strongly connected component is open): the circular sleep
    /// justification the C005 corpus found in the first version of this engine.
    DonateIntoOpenComponent,
    /// Leave the definedness predicates' reads out of the visible set: a reduction may
    /// then postpone a step that changes one, and skip the interleaving that reaches
    /// an undefined read.
    HideDefinednessReads,
}

#[cfg(test)]
impl Mutant {
    /// Every mutant, in a fixed order.
    pub(crate) const ALL: [Self; 6] = [
        Self::DropWriteReadDependence,
        Self::SkipEnablingInsertion,
        Self::SkipCycleProviso,
        Self::NoSleepWakeup,
        Self::DonateIntoOpenComponent,
        Self::HideDefinednessReads,
    ];

    /// A stable name for evidence records.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::DropWriteReadDependence => "drop-write-read-dependence",
            Self::SkipEnablingInsertion => "skip-enabling-backtrack-insertion",
            Self::SkipCycleProviso => "skip-proviso-backtrack-insertion",
            Self::NoSleepWakeup => "no-sleep-set-wakeup",
            Self::DonateIntoOpenComponent => "donate-into-open-component",
            Self::HideDefinednessReads => "hide-definedness-reads",
        }
    }
}

impl Knobs {
    #[cfg(not(test))]
    const fn write_read_conflicts(self) -> bool {
        true
    }
    #[cfg(not(test))]
    const fn enabling_insertion(self) -> bool {
        true
    }
    #[cfg(not(test))]
    const fn cycle_proviso(self) -> bool {
        true
    }
    #[cfg(not(test))]
    const fn sleep_wakeup(self) -> bool {
        true
    }
    #[cfg(not(test))]
    const fn donation_waits_for_component(self) -> bool {
        true
    }
    #[cfg(not(test))]
    const fn observes_definedness(self) -> bool {
        true
    }
    #[cfg(not(test))]
    const fn pays_obligations(self) -> bool {
        true
    }
    #[cfg(not(test))]
    const fn pays_index_search(self) -> bool {
        true
    }
    #[cfg(not(test))]
    const fn pays_proviso_collect(self) -> bool {
        true
    }

    #[cfg(test)]
    fn write_read_conflicts(self) -> bool {
        self.mutant != Some(Mutant::DropWriteReadDependence)
    }
    #[cfg(test)]
    fn enabling_insertion(self) -> bool {
        self.mutant != Some(Mutant::SkipEnablingInsertion)
    }
    #[cfg(test)]
    fn cycle_proviso(self) -> bool {
        self.mutant != Some(Mutant::SkipCycleProviso)
    }
    #[cfg(test)]
    fn sleep_wakeup(self) -> bool {
        self.mutant != Some(Mutant::NoSleepWakeup)
    }
    #[cfg(test)]
    fn donation_waits_for_component(self) -> bool {
        self.mutant != Some(Mutant::DonateIntoOpenComponent)
    }
    #[cfg(test)]
    fn observes_definedness(self) -> bool {
        self.mutant != Some(Mutant::HideDefinednessReads)
    }
    #[cfg(test)]
    fn pays_obligations(self) -> bool {
        self.omit != Some(Omission::Obligations)
    }
    #[cfg(test)]
    fn pays_index_search(self) -> bool {
        self.omit != Some(Omission::IndexSearch)
    }
    #[cfg(test)]
    fn pays_proviso_collect(self) -> bool {
        self.omit != Some(Omission::ProvisoCollect)
    }
}

// ---------------------------------------------------------------------------
// entry
// ---------------------------------------------------------------------------

/// Run one reduced check.
pub(crate) fn run(
    model: &Model,
    obligations: &Obligations,
    bounds: Bounds,
    knobs: Knobs,
) -> Result<Report, CheckError> {
    // The meter comes first: reading the obligations is proportional to them, and
    // every invariant also needs a result in the report, so both are paid up front.
    let mut meter = Meter::new(bounds.work());
    let count = obligations.invariants().len();
    let reserve = units(count).saturating_mul(crate::OBLIGATION_UNITS);
    if knobs.pays_obligations() && meter.pay(reserve).is_err() {
        return Err(CheckError::WorkBelowObligations {
            invariants: count,
            work: bounds.work(),
        });
    }
    let declared = model.predicates().len();
    let mut invariants: Vec<usize> = Vec::with_capacity(count);
    for index in obligations.invariants() {
        if *index >= declared {
            return Err(CheckError::UnknownPredicate {
                index: *index,
                declared,
            });
        }
        invariants.push(*index);
        meter.did(crate::OBLIGATION_UNITS);
    }
    let footprints = match Footprints::derive(
        model,
        &invariants,
        &mut meter,
        knobs.write_read_conflicts(),
        knobs.observes_definedness(),
    ) {
        Ok(footprints) => footprints,
        Err(bound) => {
            let mut report = refused(model, obligations, &invariants, bound, 0, meter);
            report.bounds = bounds;
            return Ok(report);
        }
    };
    let predicate_cost = footprints.predicate_cost();
    let mut search = Search {
        model,
        fp: footprints,
        bounds,
        meter,
        knobs,
        invariants,
        predicate_cost,
        nodes: Vec::new(),
        index: BTreeMap::new(),
        visits: Vec::new(),
        roots: Vec::new(),
        stack: Vec::new(),
        open: Vec::new(),
        transitions: 0,
        terminals: 0,
        violations: Vec::new(),
        predicate_faults: Vec::new(),
        undefined: Vec::new(),
        action_undefined: None,
        action_fault: None,
    };
    search.violations = vec![None; search.invariants.len()];
    search.predicate_faults = vec![None; search.invariants.len()];
    search.undefined = vec![None; search.invariants.len()];
    let completeness = match search.execute() {
        Ok(()) => Completeness::Complete,
        Err(Halt::Bound(bound)) => Completeness::Exhausted(bound),
        Err(Halt::Fault(fault)) => Completeness::Faulted(fault),
    };
    // Assembling the report — the witness's node and visit records, the projection
    // set, the terminal-state sort, the stats pass — is proportional to the search. It
    // is paid here, before any of it is built; a budget that cannot cover it gives a
    // typed `ResourceExhausted` report, never one assembled past the bound.
    if let Err(bound) = search.pay_finalization() {
        let stored = search.nodes.len();
        let mut report = refused(
            model,
            obligations,
            &search.invariants,
            bound,
            stored,
            search.meter,
        );
        report.stats.states = stored;
        report.stats.visits = search.visits.len();
        report.stats.transitions = search.transitions;
        report.bounds = bounds;
        report.obligations = obligations.clone();
        return Ok(report);
    }
    let mut report = search.finish(completeness, obligations.deadlock());
    report.bounds = bounds;
    report.obligations = obligations.clone();
    Ok(report)
}

/// The report of a check the budget stopped before a report could be assembled: at
/// footprint derivation, or at finalization. Built only from the obligations, whose
/// results the obligation reserve prepaid; no witness and no projections.
fn refused(
    model: &Model,
    obligations: &Obligations,
    invariants: &[usize],
    bound: Bound,
    stored: usize,
    meter: Meter,
) -> Report {
    let unresolved = Unresolved::ResourceExhausted {
        tripped: bound,
        stored,
    };
    let invariants = invariants
        .iter()
        .filter_map(|index| {
            model.predicates().get(*index).map(|predicate| {
                InvariantResult::new(
                    *index,
                    predicate.name().clone(),
                    InvariantOutcome::Inconclusive(unresolved.clone()),
                )
            })
        })
        .collect();
    let deadlock = match obligations.deadlock() {
        DeadlockPolicy::Allowed => DeadlockOutcome::NotJudged { terminal: 0 },
        DeadlockPolicy::Defect => DeadlockOutcome::Inconclusive(unresolved),
    };
    Report {
        bounds: Bounds::new(0, 0, 0, 0),
        obligations: obligations.clone(),
        completeness: Completeness::Exhausted(bound),
        invariants,
        deadlock,
        stats: Stats {
            work: meter.spent(),
            ..Stats::default()
        },
        witness: ReductionWitness {
            labels: Vec::new(),
            visible: 0,
            nodes: Vec::new(),
            visits: Vec::new(),
            roots: Vec::new(),
            complete: false,
        },
        projections: BTreeSet::new(),
    }
}

// ---------------------------------------------------------------------------
// the search
// ---------------------------------------------------------------------------

/// Why the search stopped early.
enum Halt {
    Bound(Bound),
    Fault(EngineFault),
}

impl From<Bound> for Halt {
    fn from(bound: Bound) -> Self {
        Self::Bound(bound)
    }
}

fn internal(what: &'static str) -> Halt {
    Halt::Fault(EngineFault::Internal { what })
}

/// What judging one state found, by obligation slot.
#[derive(Default)]
struct Judgement {
    violated: Vec<usize>,
    faults: Vec<(usize, EngineFault)>,
    undefined: Vec<(usize, usize)>,
    action_undefined: Option<usize>,
    action_fault: Option<EngineFault>,
}

/// One label's standing at one state.
enum Status {
    /// The guard is false.
    Disabled,
    /// The guard holds and firing leaves the state unchanged: a self-loop, which the
    /// progress restriction does not explore.
    Stutter,
    /// The guard holds and firing reaches a different state.
    Enabled(State),
}

/// A stored state, with its label standing and its expansion choice.
struct Node {
    state: State,
    enabled: LabelSet,
    persistent: LabelSet,
    expansion: Expansion,
    /// The visit's node and the label that first reached this state.
    discovery: Option<(usize, usize)>,
    /// The length of the discovery path.
    depth: usize,
    terminal: bool,
    visits: Vec<usize>,
}

/// A visit: a stored state entered with a sleep set — one node of the search graph.
/// Its index is also its Tarjan preorder number: visits are created in depth-first
/// preorder and never re-entered.
struct Visit {
    node: usize,
    sleep: LabelSet,
    proviso: bool,
    edges: Vec<EdgeRecord>,
    explored: LabelSet,
    low: usize,
    /// On the Tarjan stack: its strongly connected component is not yet complete.
    open: bool,
    /// Has an active frame on the search stack.
    on_stack: bool,
}

struct Frame {
    visit: usize,
    todo: Vec<usize>,
    cursor: usize,
    /// The entry sleep set plus every label donated so far.
    sleep: LabelSet,
    /// A tree edge whose child frame is running: `(label, edge position, child)`.
    pending: Option<(usize, usize, usize)>,
}

struct Search<'m> {
    model: &'m Model,
    fp: Footprints<'m>,
    bounds: Bounds,
    meter: Meter,
    knobs: Knobs,
    invariants: Vec<usize>,
    predicate_cost: u64,
    nodes: Vec<Node>,
    index: BTreeMap<State, usize>,
    visits: Vec<Visit>,
    roots: Vec<usize>,
    stack: Vec<Frame>,
    /// Tarjan's stack of visits whose component is still open.
    open: Vec<usize>,
    transitions: u64,
    /// Stored states with no enabled action.
    terminals: usize,
    violations: Vec<Option<usize>>,
    predicate_faults: Vec<Option<EngineFault>>,
    /// Per obligation slot: the first stored state reading a value undefined in the
    /// invariant, and the false definedness predicate.
    undefined: Vec<Option<(usize, usize)>>,
    /// The first stored state with an undefined action read, and the predicate.
    action_undefined: Option<(usize, usize)>,
    /// An action definedness predicate that could not be evaluated.
    action_fault: Option<EngineFault>,
}

impl Search<'_> {
    fn execute(&mut self) -> Result<(), Halt> {
        let width = self.fp.width();
        let model = self.model;
        // Each root is copied before it is entered.
        let arity = units(model.arity());
        for state in self
            .meter
            .iter(model.initial_states(), arity.saturating_add(1))?
        {
            let (visit, created) = self.enter(state.clone(), LabelSet::empty(width), None)?;
            self.roots.push(visit);
            if created {
                self.drain()?;
            }
        }
        Ok(())
    }

    // --- access -------------------------------------------------------------

    fn node(&self, node: usize) -> Result<&Node, Halt> {
        self.nodes.get(node).ok_or_else(|| internal("node index"))
    }

    fn visit(&self, visit: usize) -> Result<&Visit, Halt> {
        self.visits
            .get(visit)
            .ok_or_else(|| internal("visit index"))
    }

    fn visit_mut(&mut self, visit: usize) -> Result<&mut Visit, Halt> {
        self.visits
            .get_mut(visit)
            .ok_or_else(|| internal("visit index"))
    }

    fn frame(&self, top: usize) -> Result<&Frame, Halt> {
        self.stack.get(top).ok_or_else(|| internal("frame"))
    }

    fn frame_mut(&mut self, top: usize) -> Result<&mut Frame, Halt> {
        self.stack.get_mut(top).ok_or_else(|| internal("frame"))
    }

    fn word_units(&self) -> u64 {
        units(self.fp.width().div_ceil(64)).saturating_add(1)
    }
    // --- evaluation -------------------------------------------------------

    fn fault(&self, action: usize, state: &State, source: EvaluationError) -> Halt {
        Halt::Fault(EngineFault::Evaluation {
            state: state.clone(),
            action: self
                .model
                .actions()
                .get(action)
                .map(|declared| declared.name().clone()),
            source: Box::new(source),
        })
    }

    /// One label's standing at `state`. The caller has charged its cost.
    fn status(&self, label: usize, state: &State) -> Result<Status, Halt> {
        let Some(named) = self.fp.label(label) else {
            return Err(internal("label index"));
        };
        let enabled = self
            .model
            .is_enabled(named.action, state)
            .map_err(|source| self.fault(named.action, state, source))?;
        if !enabled {
            return Ok(Status::Disabled);
        }
        let environment = Environment::new(self.model.variables(), state.as_slice());
        let mut values = state.as_slice().to_vec();
        for (variable, expr) in self.fp.updates(label) {
            let value = expr
                .evaluate(&environment)
                .map_err(|source| self.fault(named.action, state, source.into()))?;
            let Some(declared) = self.model.variables().get(*variable) else {
                return Err(internal("assigned variable index"));
            };
            if !declared.domain().admits(value) {
                let action = self
                    .model
                    .actions()
                    .get(named.action)
                    .map(|action| action.name().clone())
                    .ok_or_else(|| internal("action index"))?;
                return Err(self.fault(
                    named.action,
                    state,
                    EvaluationError::UpdateOutOfDomain {
                        action,
                        variable: declared.name().clone(),
                        value,
                        domain: *declared.domain(),
                    },
                ));
            }
            let Some(slot) = values.get_mut(*variable) else {
                return Err(internal("state arity"));
            };
            *slot = value;
        }
        let target = self
            .model
            .state(&values)
            .map_err(|source| self.fault(named.action, state, source))?;
        Ok(if target == *state {
            Status::Stutter
        } else {
            Status::Enabled(target)
        })
    }

    // --- expansion --------------------------------------------------------

    /// The expansion of a new state: a proper stubborn set without an enabled visible
    /// label, the smallest over every invisible enabled seed (ties to the least seed),
    /// or a full expansion when none exists.
    fn expand(&mut self, enabled: &LabelSet) -> Result<(Expansion, LabelSet), Halt> {
        if enabled.is_empty() {
            return Ok((Expansion::Full(FullReason::NoProgress), enabled.clone()));
        }
        let total = enabled.count();
        let mut best: Option<(usize, usize, LabelSet)> = None;
        let words = self.word_units();
        for seed in enabled.iter() {
            if self.fp.is_visible(seed) {
                continue;
            }
            // The seed's own bookkeeping below (two set copies and a count).
            let paid = self.meter.pay(words.saturating_mul(3))?;
            self.meter.did(paid);
            let Some(stubborn) = self.closure(seed, enabled)? else {
                continue;
            };
            let mut persistent = stubborn.clone();
            persistent.intersect_with(enabled);
            let count = persistent.count();
            if count >= total {
                continue;
            }
            if best
                .as_ref()
                .is_none_or(|(incumbent, _, _)| count < *incumbent)
            {
                best = Some((count, seed, stubborn));
            }
            if count <= 1 {
                break;
            }
        }
        Ok(match best {
            Some((_, seed, stubborn)) => {
                let mut persistent = stubborn.clone();
                persistent.intersect_with(enabled);
                // Two copies of the member list: this record, and the witness's.
                let cost = Meter::members_cost(&stubborn, 2);
                self.meter.pay(cost)?;
                let members = stubborn.to_vec();
                self.meter.did(cost);
                (
                    Expansion::Reduced {
                        seed,
                        stubborn: members,
                    },
                    persistent,
                )
            }
            None => (Expansion::Full(FullReason::Exhaustive), enabled.clone()),
        })
    }

    /// The stubborn closure of `seed`: an enabled member brings every label it is
    /// dependent with; a member that is not enabled brings every label that writes a
    /// variable it reads (a necessary enabling set). `None` when an enabled visible
    /// label joins, since such a set cannot be a reduced expansion.
    fn closure(&mut self, seed: usize, enabled: &LabelSet) -> Result<Option<LabelSet>, Halt> {
        let width = self.fp.width();
        let words = self.word_units();
        let mut set = LabelSet::empty(width);
        set.insert(seed);
        let mut work: Vec<usize> = vec![seed];
        while let Some(label) = work.pop() {
            let paid = self.meter.pay(words)?;
            self.meter.did(paid);
            let add = if enabled.contains(label) {
                self.fp.dependents(label)
            } else if self.knobs.enabling_insertion() {
                self.fp.enablers(label)
            } else {
                None
            };
            let Some(add) = add else {
                continue;
            };
            let mut fresh = add.clone();
            fresh.subtract(&set);
            if fresh.is_empty() {
                continue;
            }
            let paid = self.meter.pay(units(fresh.count()))?;
            self.meter.did(paid);
            for member in fresh.iter() {
                if enabled.contains(member) && self.fp.is_visible(member) {
                    return Ok(None);
                }
                work.push(member);
            }
            set.union_with(&fresh);
        }
        Ok(Some(set))
    }

    // --- states -------------------------------------------------------------

    /// Store a new state: evaluate every label at it (charged first), judge the
    /// invariants, and choose its expansion. A state that is terminal or is the
    /// first to falsify an invariant will need its discovery path in the report, so
    /// that path (depth × state size) is charged here, before the state is stored:
    /// the report then builds every path it owes without spending anything.
    fn create_node(
        &mut self,
        state: State,
        discovery: Option<(usize, usize)>,
    ) -> Result<usize, Halt> {
        if self.nodes.len() >= self.bounds.states() {
            return Err(Halt::Bound(Bound::States));
        }
        let width = self.fp.width();
        let arity = units(state.arity());
        // Label evaluation, plus the state's own copies: the index key, the witness
        // record, the projection, and the index insert's logarithmic search.
        let search = units(bits(self.nodes.len()));
        let paid = self.meter.pay(
            self.fp
                .total_cost()
                .saturating_add(units(width))
                .saturating_add(arity.saturating_mul(search.saturating_add(4))),
        )?;
        self.meter.did(paid);
        let mut enabled = LabelSet::empty(width);
        let mut steps = false;
        for label in 0..width {
            match self.status(label, &state)? {
                Status::Disabled => {}
                Status::Stutter => steps = true,
                Status::Enabled(_) => {
                    steps = true;
                    enabled.insert(label);
                }
            }
        }
        let node = self.nodes.len();
        let judged = self.judge(&state)?;
        let depth = match discovery {
            Some((parent, _)) => self.node(parent)?.depth.saturating_add(1),
            None => 0,
        };
        let owes_path = !steps
            || (judged.action_undefined.is_some() && self.action_undefined.is_none())
            || judged
                .undefined
                .iter()
                .any(|(slot, _)| self.undefined.get(*slot).is_some_and(Option::is_none))
            || judged
                .violated
                .iter()
                .any(|slot| self.violations.get(*slot).is_some_and(Option::is_none));
        if !steps {
            // A terminal state is sorted into the deadlock list when the report is
            // built: its share of that sort (a state comparison per level of a sort
            // over at most the state bound) is paid now, while the meter can refuse.
            let paid = self.meter.pay(
                arity
                    .saturating_add(1)
                    .saturating_mul(units(bits(self.bounds.states())).saturating_add(1)),
            )?;
            self.meter.did(paid);
        }
        if owes_path {
            // The path may be copied into every invariant's result and the deadlock
            // outcome (an undefined action read is all of them).
            let copies = units(self.invariants.len()).saturating_add(1);
            let paid = self.meter.pay(
                units(depth)
                    .saturating_add(1)
                    .saturating_mul(arity.saturating_add(1))
                    .saturating_mul(copies),
            )?;
            self.meter.did(paid);
        }
        let (expansion, persistent) = self.expand(&enabled)?;
        if !steps {
            self.terminals = self.terminals.saturating_add(1);
        }
        for slot in judged.violated {
            if let Some(entry) = self.violations.get_mut(slot)
                && entry.is_none()
            {
                *entry = Some(node);
            }
        }
        for (slot, guard) in judged.undefined {
            if let Some(entry) = self.undefined.get_mut(slot)
                && entry.is_none()
            {
                *entry = Some((node, guard));
            }
        }
        if let Some(guard) = judged.action_undefined
            && self.action_undefined.is_none()
        {
            self.action_undefined = Some((node, guard));
        }
        if let Some(fault) = judged.action_fault
            && self.action_fault.is_none()
        {
            self.action_fault = Some(fault);
        }
        for (slot, fault) in judged.faults {
            if let Some(entry) = self.predicate_faults.get_mut(slot)
                && entry.is_none()
            {
                *entry = Some(fault);
            }
        }
        self.index.insert(state.clone(), node);
        self.nodes.push(Node {
            state,
            enabled,
            persistent,
            expansion,
            discovery,
            depth,
            terminal: !steps,
            visits: Vec::new(),
        });
        Ok(node)
    }

    /// Judge a new state, reading the observed predicates in the reference engine's
    /// order (RFC 0003, "Definedness"; `continuum_engine_reference::definedness`):
    /// every action's definedness chain, each deepest first, stopping at the first
    /// false one — an undefined action read, and nothing else is read there; then per
    /// invariant its own chain, deepest first (a false one is an undefined read in the
    /// invariant, and the invariant is not read); then the invariant itself (a false
    /// definedness predicate asked as an invariant is an undefined read in its base,
    /// never a violation). Nothing is recorded here, so a bound that stops the search
    /// after this call leaves no unpaid path owed.
    fn judge(&mut self, state: &State) -> Result<Judgement, Halt> {
        let paid = self.meter.pay(self.predicate_cost)?;
        self.meter.did(paid);
        let mut judged = Judgement::default();
        let fault = |source| EngineFault::Evaluation {
            state: state.clone(),
            action: None,
            source: Box::new(source),
        };
        let definedness = self.fp.definedness();
        'chains: for chain in definedness.action_chains() {
            for &guard in chain {
                match self.model.evaluate_predicate(guard, state) {
                    Ok(true) => {}
                    Ok(false) => {
                        judged.action_undefined = Some(guard);
                        break 'chains;
                    }
                    Err(source) => {
                        judged.action_fault = Some(fault(source));
                        return Ok(judged);
                    }
                }
            }
        }
        if judged.action_undefined.is_some() {
            return Ok(judged);
        }
        for (slot, predicate) in self.invariants.iter().enumerate() {
            let mut undefined = None;
            let mut failed = None;
            for &guard in definedness.guards_of(*predicate) {
                match self.model.evaluate_predicate(guard, state) {
                    Ok(true) => {}
                    Ok(false) => {
                        undefined = Some(guard);
                        break;
                    }
                    Err(source) => {
                        failed = Some(fault(source));
                        break;
                    }
                }
            }
            if let Some(failed) = failed {
                judged.faults.push((slot, failed));
                continue;
            }
            if let Some(guard) = undefined {
                judged.undefined.push((slot, guard));
                continue;
            }
            match self.model.evaluate_predicate(*predicate, state) {
                Ok(true) => {}
                Ok(false) if definedness.guards(*predicate).is_some() => {
                    judged.undefined.push((slot, *predicate));
                }
                Ok(false) => judged.violated.push(slot),
                Err(source) => judged.faults.push((slot, fault(source))),
            }
        }
        Ok(judged)
    }

    // --- visits -------------------------------------------------------------

    /// Enter `state` with sleep set `sleep`: reuse a visit of the state whose sleep set
    /// is a subset of `sleep` (it explores at least what this entry would), or create a
    /// visit and push its frame. Returns the visit and whether it was created.
    fn enter(
        &mut self,
        state: State,
        sleep: LabelSet,
        discovery: Option<(usize, usize)>,
    ) -> Result<(usize, bool), Halt> {
        // The index lookup compares states: pay for the search before making it.
        let search = Meter::search_cost(self.index.len(), state.arity());
        if self.knobs.pays_index_search() {
            self.meter.pay(search)?;
        }
        let found = self.index.get(&state).copied();
        self.meter.did(search);
        let node = match found {
            Some(node) => node,
            None => self.create_node(state, discovery)?,
        };
        let words = self.word_units();
        let known = self.node(node)?.visits.len();
        // Each stored visit's sleep set is compared by words, and the new visit's
        // work list is written label by label.
        let paid = self.meter.pay(
            words
                .saturating_mul(units(known).saturating_add(2))
                .saturating_add(units(self.fp.width())),
        )?;
        self.meter.did(paid);
        for visit in &self.node(node)?.visits {
            if self
                .visits
                .get(*visit)
                .is_some_and(|entry| entry.sleep.is_subset(&sleep))
            {
                return Ok((*visit, false));
            }
        }
        if self.visits.len() >= self.bounds.states() {
            return Err(Halt::Bound(Bound::Visits));
        }
        if self.stack.len() >= self.bounds.depth() {
            return Err(Halt::Bound(Bound::Depth));
        }
        let visit = self.visits.len();
        let mut todo = self.node(node)?.persistent.clone();
        todo.subtract(&sleep);
        self.visits.push(Visit {
            node,
            sleep: sleep.clone(),
            proviso: false,
            edges: Vec::new(),
            explored: LabelSet::empty(self.fp.width()),
            low: visit,
            open: true,
            on_stack: true,
        });
        self.nodes
            .get_mut(node)
            .ok_or_else(|| internal("node index"))?
            .visits
            .push(visit);
        self.open.push(visit);
        self.stack.push(Frame {
            visit,
            todo: todo.to_vec(),
            cursor: 0,
            sleep,
            pending: None,
        });
        Ok((visit, true))
    }

    /// Run the stack to empty.
    fn drain(&mut self) -> Result<(), Halt> {
        while let Some(top) = self.stack.len().checked_sub(1) {
            if let Some((label, position, child)) = self.frame_mut(top)?.pending.take() {
                self.settle(top, label, position, child)?;
                continue;
            }
            let next = {
                let frame = self.frame_mut(top)?;
                let label = frame.todo.get(frame.cursor).copied();
                if label.is_some() {
                    frame.cursor = frame.cursor.saturating_add(1);
                }
                label
            };
            match next {
                Some(label) => self.step(top, label)?,
                None => self.close(top)?,
            }
        }
        Ok(())
    }

    /// The frame at `top` has explored its last label.
    fn close(&mut self, top: usize) -> Result<(), Halt> {
        if top.saturating_add(1) != self.stack.len() {
            return Err(internal("only the top frame closes"));
        }
        let frame = self.stack.pop().ok_or_else(|| internal("frame"))?;
        let visit = frame.visit;
        let entry = self.visit_mut(visit)?;
        entry.on_stack = false;
        if entry.low == visit {
            // The visit roots a strongly connected component: it is complete.
            while let Some(member) = self.open.pop() {
                self.visit_mut(member)?.open = false;
                if member == visit {
                    break;
                }
            }
        }
        Ok(())
    }

    /// A tree edge's child frame has closed: fold its low-link into the parent, and
    /// donate the label to the parent's sleep set once the child's component is
    /// complete.
    fn settle(
        &mut self,
        top: usize,
        label: usize,
        position: usize,
        child: usize,
    ) -> Result<(), Halt> {
        let visit = self.frame(top)?.visit;
        let (child_low, child_open) = {
            let entry = self.visit(child)?;
            (entry.low, entry.open)
        };
        let entry = self.visit_mut(visit)?;
        entry.low = entry.low.min(child_low);
        if !child_open || !self.knobs.donation_waits_for_component() {
            self.donate(top, label, position)?;
        }
        Ok(())
    }

    /// `label`, explored from the frame at `top` along edge `position`, sleeps in the
    /// frame's later edges.
    fn donate(&mut self, top: usize, label: usize, position: usize) -> Result<(), Halt> {
        let visit = self.frame(top)?.visit;
        self.frame_mut(top)?.sleep.insert(label);
        self.visit_mut(visit)?
            .edges
            .get_mut(position)
            .ok_or_else(|| internal("edge position"))?
            .donated = true;
        Ok(())
    }

    /// Explore `label` from the frame at stack position `top`.
    fn step(&mut self, top: usize, label: usize) -> Result<(), Halt> {
        let words = self.word_units();
        let paid = self.meter.pay(words.saturating_mul(2))?;
        self.meter.did(paid);
        let visit = self.frame(top)?.visit;
        let node = self.visit(visit)?.node;
        if self.visit(visit)?.explored.contains(label) {
            return Ok(());
        }
        if self.transitions >= self.bounds.transitions() {
            return Err(Halt::Bound(Bound::Transitions));
        }
        let paid = self.meter.pay(self.fp.cost(label))?;
        self.meter.did(paid);
        let state = self.node(node)?.state.clone();
        let Status::Enabled(target) = self.status(label, &state)? else {
            return Err(internal("an explored label is enabled"));
        };
        let mut child_sleep = self.frame(top)?.sleep.clone();
        if self.knobs.sleep_wakeup()
            && let Some(dependents) = self.fp.dependents(label)
        {
            // Wake every sleeping label the fired one is dependent with.
            child_sleep.subtract(dependents);
        }
        self.visit_mut(visit)?.explored.insert(label);
        self.transitions = self.transitions.saturating_add(1);
        let (child, created) = self.enter(target, child_sleep, Some((node, label)))?;
        let position = {
            let entry = self.visit_mut(visit)?;
            entry.edges.push(EdgeRecord {
                label,
                target: child,
                donated: false,
            });
            entry.edges.len().saturating_sub(1)
        };
        if created {
            self.frame_mut(top)?.pending = Some((label, position, child));
            return Ok(());
        }
        let (on_stack, open) = {
            let entry = self.visit(child)?;
            (entry.on_stack, entry.open)
        };
        if on_stack && self.knobs.cycle_proviso() && !self.visit(visit)?.proviso {
            self.proviso(top, label)?;
        }
        if open {
            let entry = self.visit_mut(visit)?;
            entry.low = entry.low.min(child);
            if !self.knobs.donation_waits_for_component() {
                self.donate(top, label, position)?;
            }
        } else {
            self.donate(top, label, position)?;
        }
        Ok(())
    }

    /// The stack proviso: an edge back onto the search stack closes a cycle of the
    /// search graph, so the visit is expanded in full before the cycle can hide a
    /// transition. Every enabled label not yet explored, queued, or asleep is queued.
    fn proviso(&mut self, top: usize, label: usize) -> Result<(), Halt> {
        let words = self.word_units();
        let paid = self.meter.pay(words.saturating_mul(4))?;
        self.meter.did(paid);
        let visit = self.frame(top)?.visit;
        let node = self.visit(visit)?.node;
        if matches!(self.node(node)?.expansion, Expansion::Full(_)) {
            return Ok(());
        }
        self.visit_mut(visit)?.proviso = true;
        let mut extra = self.node(node)?.enabled.clone();
        extra.subtract(&self.visit(visit)?.explored);
        // The queued scan below walks the frame's remaining work list: pay first.
        let remaining = self.frame(top)?.todo.len();
        self.meter.items(remaining, 1)?;
        let frame = self.frame(top)?;
        extra.subtract(&frame.sleep);
        let mut queued = LabelSet::empty(self.fp.width());
        for pending in frame.todo.iter().skip(frame.cursor) {
            queued.insert(*pending);
        }
        queued.insert(label);
        extra.subtract(&queued);
        // The extra labels are collected and appended to the work list: both copies
        // are paid before the set bits are read.
        let collect = Meter::members_cost(&extra, 2);
        if self.knobs.pays_proviso_collect() {
            self.meter.pay(collect)?;
        }
        let extra = extra.to_vec();
        self.frame_mut(top)?.todo.extend(extra);
        self.meter.did(collect);
        Ok(())
    }

    // --- the report ---------------------------------------------------------

    /// Pay for assembling the report, from counts alone (no pass over the search):
    /// per stored state its witness record, its projection and that projection's
    /// ordered insert; per visit its record (entry sleep set by label, edges by
    /// count); the terminal-state collection and sort; the stats pass over nodes and
    /// visits; the label table. Paths were paid when their states were stored.
    fn pay_finalization(&mut self) -> Result<(), Bound> {
        let nodes = units(self.nodes.len());
        let per_state = units(self.model.arity()).saturating_add(1);
        let search = units(bits(self.nodes.len())).saturating_add(1);
        let width = units(self.fp.width());
        let words = self.word_units();
        let cost = nodes
            .saturating_mul(per_state)
            .saturating_mul(search.saturating_add(2))
            .saturating_add(
                units(self.visits.len())
                    .saturating_mul(width.saturating_add(words.saturating_mul(3))),
            )
            .saturating_add(self.transitions.saturating_mul(2))
            .saturating_add(
                units(self.terminals)
                    .saturating_mul(per_state)
                    .saturating_mul(search.saturating_add(1)),
            )
            .saturating_add(nodes.saturating_mul(words))
            .saturating_add(width);
        let paid = self.meter.pay(cost)?;
        self.meter.did(paid);
        Ok(())
    }

    /// The discovery path of a stored node, from an initial state. Its cost was
    /// charged when the node was stored ([`Search::create_node`]); `Err` only if the
    /// discovery chain is broken, which the search never produces.
    fn trace(&self, node: usize) -> Result<Trace, &'static str> {
        let mut steps: Vec<TraceStep> = Vec::new();
        let mut current = node;
        // Discovery parents are stored before their children, so the chain is
        // strictly decreasing and ends at an initial state.
        loop {
            let entry = self.nodes.get(current).ok_or("discovery chain")?;
            let Some((parent, label)) = entry.discovery else {
                break;
            };
            if parent >= current {
                return Err("discovery chain");
            }
            let named = self.fp.label(label).ok_or("discovery label")?;
            steps.push(TraceStep::new(
                named.action,
                named.outcome,
                entry.state.clone(),
            ));
            current = parent;
        }
        steps.reverse();
        let start = self
            .nodes
            .get(current)
            .ok_or("discovery chain")?
            .state
            .clone();
        Ok(Trace::new(start, steps))
    }

    fn finish(self, completeness: Completeness, policy: DeadlockPolicy) -> Report {
        let stored = self.nodes.len();
        let visible = self.fp.visible();
        let halted = match &completeness {
            Completeness::Complete => None,
            Completeness::Exhausted(bound) => Some(Unresolved::ResourceExhausted {
                tripped: *bound,
                stored,
            }),
            Completeness::Faulted(fault) => Some(Unresolved::EngineError(fault.clone())),
        };
        let broken = |what: &'static str| Unresolved::EngineError(EngineFault::Internal { what });

        let undefined_at = |search: &Self, (node, guard): (usize, usize)| {
            let (Some(entry), Some(named)) =
                (search.nodes.get(node), search.model.predicates().get(guard))
            else {
                return Err("undefined read");
            };
            let read = search
                .fp
                .definedness()
                .guards(guard)
                .unwrap_or(Guarded::Action);
            let trace = search.trace(node)?;
            Ok(Box::new(UndefinedRead::new(
                guard,
                named.name().clone(),
                read,
                entry.state.clone(),
                Some(trace),
            )))
        };
        let action_undefined = self
            .action_undefined
            .map(|found| undefined_at(&self, found));

        let mut invariants: Vec<InvariantResult> = Vec::with_capacity(self.invariants.len());
        let declared = self.invariants.clone();
        for (slot, index) in declared.iter().enumerate() {
            let Some(predicate) = self.model.predicates().get(*index) else {
                continue;
            };
            // Precedence (RFC 0003 "Definedness", as the reference engine reads it): an
            // evaluation error, an undefined action read, an undefined read in the
            // invariant, a violation, then the search's own answer.
            let fault = self
                .action_fault
                .clone()
                .or_else(|| self.predicate_faults.get(slot).cloned().flatten());
            let violation = self.violations.get(slot).copied().flatten();
            let subject = self.undefined.get(slot).copied().flatten();
            let undefined = match (&action_undefined, subject) {
                (Some(found), _) => Some(found.clone()),
                (None, Some(found)) => Some(undefined_at(&self, found)),
                (None, None) => None,
            };
            let outcome = match (fault, violation, &halted) {
                (Some(fault), _, _) => {
                    InvariantOutcome::Inconclusive(Unresolved::EngineError(fault))
                }
                (None, _, _) if undefined.is_some() => match undefined {
                    Some(Ok(read)) => InvariantOutcome::Undefined(read),
                    Some(Err(what)) => InvariantOutcome::Inconclusive(broken(what)),
                    None => InvariantOutcome::Inconclusive(broken("undefined read")),
                },
                (None, Some(node), _) => match (self.trace(node), self.nodes.get(node)) {
                    (Ok(trace), Some(entry)) => InvariantOutcome::Violated {
                        state: entry.state.clone(),
                        trace: Some(trace),
                    },
                    (Err(what), _) => InvariantOutcome::Inconclusive(broken(what)),
                    (Ok(_), None) => InvariantOutcome::Inconclusive(broken("node index")),
                },
                (None, None, Some(reason)) => InvariantOutcome::Inconclusive(reason.clone()),
                (None, None, None) => InvariantOutcome::Holds { stored },
            };
            invariants.push(InvariantResult::new(
                *index,
                predicate.name().clone(),
                outcome,
            ));
        }

        let mut terminal: Vec<(State, usize)> = self
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.terminal)
            .map(|(index, node)| (node.state.clone(), index))
            .collect();
        terminal.sort();
        // A successor row that could not be evaluated ranks first for the deadlock
        // question, as the reference engine's terminal-state scan does.
        let row_fault = match &completeness {
            Completeness::Faulted(fault) => Some(fault),
            _ => None,
        };
        let deadlock = match (
            self.action_fault.as_ref().or(row_fault),
            &action_undefined,
            policy,
        ) {
            (Some(fault), _, _) => {
                DeadlockOutcome::Inconclusive(Unresolved::EngineError(fault.clone()))
            }
            (None, Some(Ok(read)), _) => DeadlockOutcome::Undefined(read.clone()),
            (None, Some(Err(what)), _) => DeadlockOutcome::Inconclusive(broken(what)),
            (None, None, DeadlockPolicy::Allowed) => DeadlockOutcome::NotJudged {
                terminal: terminal.len(),
            },
            (None, None, DeadlockPolicy::Defect) => {
                if terminal.is_empty() {
                    match &halted {
                        Some(reason) => DeadlockOutcome::Inconclusive(reason.clone()),
                        None => DeadlockOutcome::Free { stored },
                    }
                } else {
                    let mut states: Vec<Deadlock> = Vec::with_capacity(terminal.len());
                    let mut defect: Option<&'static str> = None;
                    for (state, node) in terminal {
                        match self.trace(node) {
                            Ok(trace) => states.push(Deadlock::new(state, Some(trace))),
                            Err(what) => defect = Some(what),
                        }
                    }
                    match defect {
                        None => DeadlockOutcome::Deadlocked { states },
                        Some(what) => DeadlockOutcome::Inconclusive(broken(what)),
                    }
                }
            }
        };

        let mut stats = Stats {
            states: stored,
            visits: self.visits.len(),
            transitions: self.transitions,
            labels: self.fp.width(),
            work: self.meter.spent(),
            ..Stats::default()
        };
        let mut projections: BTreeSet<Vec<i64>> = BTreeSet::new();
        let mut records: Vec<NodeRecord> = Vec::with_capacity(stored);
        for node in &self.nodes {
            match &node.expansion {
                Expansion::Reduced { .. } => {
                    stats.reduced_states = stats.reduced_states.saturating_add(1);
                }
                Expansion::Full(FullReason::Exhaustive) => {
                    stats.full_exhaustive = stats.full_exhaustive.saturating_add(1);
                }
                Expansion::Full(_) => {}
            }
            projections.insert(project(&node.state, visible));
            records.push(NodeRecord {
                state: node.state.clone(),
                expansion: node.expansion.clone(),
            });
        }
        let mut visits: Vec<VisitRecord> = Vec::with_capacity(self.visits.len());
        for visit in &self.visits {
            let Some(node) = self.nodes.get(visit.node) else {
                continue;
            };
            let expanded = if visit.proviso {
                stats.full_proviso = stats.full_proviso.saturating_add(1);
                &node.enabled
            } else {
                &node.persistent
            };
            let declined = node.enabled.count().saturating_sub(expanded.count());
            stats.declined_persistent = stats.declined_persistent.saturating_add(units(declined));
            let mut asleep = expanded.clone();
            asleep.intersect_with(&visit.sleep);
            stats.declined_sleep = stats.declined_sleep.saturating_add(units(asleep.count()));
            visits.push(VisitRecord {
                node: visit.node,
                entry_sleep: visit.sleep.to_vec(),
                proviso: visit.proviso,
                edges: visit.edges.clone(),
            });
        }

        Report {
            bounds: self.bounds,
            obligations: Obligations::new(policy),
            witness: ReductionWitness {
                labels: self
                    .fp
                    .labels()
                    .iter()
                    .map(|label| (label.action, label.outcome))
                    .collect(),
                visible,
                nodes: records,
                visits,
                roots: self.roots,
                complete: completeness == Completeness::Complete,
            },
            completeness,
            invariants,
            deadlock,
            stats,
            projections,
        }
    }
}

/// The components of `state` at the visible positions, in canonical order.
pub(crate) fn project(state: &State, visible: u64) -> Vec<i64> {
    state
        .as_slice()
        .iter()
        .enumerate()
        .filter(|(index, _)| {
            u32::try_from(*index)
                .ok()
                .and_then(|shift| 1_u64.checked_shl(shift))
                .is_some_and(|bit| visible & bit != 0)
        })
        .map(|(_, value)| *value)
        .collect()
}
