//! The reduction-witness checker (docs/33: "DPOR reducer | unreduced differential
//! oracle + reduction witness checker").
//!
//! # Independence from the reducer
//!
//! This module reads a [`ReductionWitness`] — the shared schema — and the model, and
//! nothing else of the reducer. It imports no reducer module: not the footprint
//! derivation, not the label-set type, not the search. `tests/c005_checker_independence.rs`
//! pins that by reading this file's `use` lines. Every fact it relies on it
//! re-derives with its own code:
//!
//! - **footprints**, by its own walk over the closed expression language (an
//!   exhaustive `match`, so a new expression form is a compile error here rather than
//!   a silently unread variable);
//! - **label standing** at every stored state, by its own evaluation of guards and
//!   outcomes, cross-checked against `Model::action_successors` (the model layer's
//!   own successor relation) for every action at every stored state;
//! - **dependence**, from its own footprints;
//! - and, where a declined class rests on independence, **commutation itself**: at
//!   the concrete state where the reducer relied on two labels being independent, it
//!   fires both orders and compares the results.
//!
//! # What it establishes
//!
//! That every enabled label the reducer did not explore at a stored state is
//! declined by a justification that holds locally:
//!
//! - **by persistence** — the state's stubborn set is closed (every label dependent
//!   with an enabled member is a member; every label that writes a variable a
//!   disabled member reads is a member), contains an enabled label, contains no
//!   enabled label that writes a visible variable, and each of its enabled members
//!   commutes with each enabled non-member at that state;
//! - **by sleep** — the label is in the visit's entry sleep set, and every edge into
//!   the visit justifies that set: each member was asleep in the source visit or
//!   donated there by an earlier edge, is independent of the edge's label, and
//!   commutes with it at the source state;
//!
//! and two global conditions over the visit graph: no label is donated along an edge
//! that stays inside a strongly connected component (so no sleep justification is
//! circular), and the explored edges among reduced visits form no cycle (the cycle
//! proviso).
//!
//! # The donation condition, static against dynamic
//!
//! The reducer donates a label when the component its edge leads to is complete at
//! that moment of a depth-first search. The checker sees only the final graph, and
//! checks that the edge's source and target lie in different components of it. The
//! two agree: if the target could reach the source in the final graph, it could
//! reach a visit on the search stack when the edge was explored (every visit on the
//! stack reaches the source), so its component was still open; and edges added
//! later only add reachability, so a pair separated in the final graph was separated
//! then. The condition itself — donation only across components — is this engine's
//! addition to the persistent-set and sleep-set literature, not a result of it: the
//! C005 corpus found the unrestricted state-caching rule unsound here (RFC 0004
//! correction 1). Its support is that argument, the differential corpus, and the
//! seeded mutant that removes it; it is not machine-checked. The global theorem those local
//! conditions discharge (persistent sets with sleep sets and a cycle proviso preserve
//! every reachable deadlock, every reachable valuation of the visible variables, and
//! the existence of a reachable evaluation fault) is the literature's, stated in the crate
//! documentation; the checker's job is to make each premise checkable per instance.

use std::collections::{BTreeMap, BTreeSet};

use continuum_model_core::definedness::Definedness;
use continuum_model_core::expr::{BoolExpr, Environment, IntExpr};
use continuum_model_core::model::{Model, State};

use crate::report::Obligations;
use crate::witness::{Expansion, ReductionWitness};

/// Why a witness was rejected. Every arm names where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WitnessDefect {
    /// The witness is of an incomplete search; it justifies nothing.
    Incomplete,
    /// The checker's own work limit ran out.
    WorkExhausted,
    /// The witness's label table is not this model's.
    LabelTable,
    /// The witness's visible variables are not the obligations' footprint.
    VisibleSet {
        /// The reducer's set.
        claimed: u64,
        /// The checker's.
        derived: u64,
    },
    /// A node index, label index, or frame index points nowhere.
    Dangling {
        /// What dangled.
        what: &'static str,
    },
    /// Two nodes hold one state.
    DuplicateState {
        /// The second node.
        node: usize,
    },
    /// The model cannot evaluate a label at a stored state: the search hid a fault.
    Evaluation {
        /// The node.
        node: usize,
        /// The label.
        label: usize,
    },
    /// The checker's evaluation disagrees with `Model::action_successors`.
    SuccessorMismatch {
        /// The node.
        node: usize,
        /// The action.
        action: usize,
    },
    /// An edge is not a progress transition of its label.
    Edge {
        /// The visit.
        visit: usize,
        /// The label.
        label: usize,
    },
    /// A reduced expansion's seed is not an enabled member of its stubborn set, or
    /// the set has no enabled member.
    Seed {
        /// The node.
        node: usize,
    },
    /// A stubborn set is not closed.
    NotClosed {
        /// The node.
        node: usize,
        /// The member whose obligation is unmet.
        member: usize,
        /// The label that should have been a member.
        missing: usize,
    },
    /// A reduced expansion contains an enabled visible label.
    Visible {
        /// The node.
        node: usize,
        /// The label.
        label: usize,
    },
    /// Two labels the reducer treated as independent do not commute at a state.
    DoesNotCommute {
        /// The node.
        node: usize,
        /// One label.
        first: usize,
        /// The other.
        second: usize,
    },
    /// The roots are not one visit per initial state, in order, each entered with an
    /// empty sleep set.
    Root {
        /// The initial state's position.
        at: usize,
    },
    /// A sleep-set member is not justified by an edge into the visit, or a sleep set
    /// is unsorted or names a label that is not enabled.
    Sleep {
        /// The visit.
        visit: usize,
        /// The member (`usize::MAX` for a malformed set).
        label: usize,
    },
    /// A visit is not reachable from a root.
    Unreached {
        /// The visit.
        visit: usize,
    },
    /// A stored state has no visit.
    NodeWithoutVisit {
        /// The node.
        node: usize,
    },
    /// An enabled label was neither explored nor validly declined.
    Uncovered {
        /// The visit.
        visit: usize,
        /// The label.
        label: usize,
    },
    /// A label was donated to the sleep set along an edge that stays inside one
    /// strongly connected component: its justification would be circular.
    DonatedInsideComponent {
        /// The visit.
        visit: usize,
        /// The label.
        label: usize,
    },
    /// The explored edges among reduced visits form a cycle.
    ReducedCycle,
}

/// The checker's own verdict for one invariant, derived from the witness's states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedInvariant {
    /// No stored state falsifies it.
    Holds,
    /// The first stored state (in node order) that falsifies it.
    Violated(State),
    /// The predicate, or a definedness predicate read before it, could not be
    /// evaluated at a stored state.
    EvaluationFailed,
    /// A stored state reads a value undefined in some action or in this invariant:
    /// the false definedness predicate and the first such state (node order).
    Undefined {
        /// The false definedness predicate.
        guard: usize,
        /// The state.
        state: State,
    },
}

/// A witness the checker accepted, with its own account of what it justifies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedWitness {
    /// Stored states.
    pub nodes: usize,
    /// Explored edges.
    pub edges: usize,
    /// Visits.
    pub visits: usize,
    /// Enabled labels declined by persistence, summed over visits.
    pub declined_by_persistence: usize,
    /// Enabled labels declined by sleep, summed over visits.
    pub declined_by_sleep: usize,
    /// Commutation diamonds fired and compared.
    pub diamonds: usize,
    /// One verdict per invariant, ascending by predicate index.
    pub invariants: Vec<(usize, CheckedInvariant)>,
    /// Every stored state with no enabled action, ascending.
    pub deadlocks: Vec<State>,
    /// Every stored state projected onto the visible variables.
    pub projections: BTreeSet<Vec<i64>>,
    /// The first stored state (node order) with an undefined action read, and the
    /// false definedness predicate: the model is invalid there (RFC 0003, RFC 0013).
    pub undefined_action: Option<(usize, State)>,
    /// An action's definedness predicate could not be evaluated at a stored state.
    pub action_evaluation_failed: bool,
}

/// Check `witness` against `model` and `obligations` within `work` units.
///
/// # Errors
///
/// The first [`WitnessDefect`] found.
pub fn check_witness(
    model: &Model,
    obligations: &Obligations,
    witness: &ReductionWitness,
    work: u64,
) -> Result<CheckedWitness, WitnessDefect> {
    Checker::new(model, work)?.run(obligations, witness)
}

/// [`check_witness`] with one charge site left unpaid (`"stubborn-slots"` or
/// `"donation-search"`), to show the per-site credit check fires there.
#[cfg(test)]
pub(crate) fn check_witness_omitting(
    model: &Model,
    obligations: &Obligations,
    witness: &ReductionWitness,
    work: u64,
    site: &str,
) -> Result<CheckedWitness, WitnessDefect> {
    let mut checker = Checker::new(model, work)?;
    checker.omit = match site {
        "stubborn-slots" => Some(CheckerOmission::StubbornSlots),
        "donation-search" => Some(CheckerOmission::DonationSearch),
        _ => None,
    };
    checker.run(obligations, witness)
}

/// [`check_witness`], also returning the work units the checker's paid sites
/// consumed (every `pay`/`prepay` site, through `used`). Every unit is paid before it
/// is consumed, so the count never exceeds the work limit; it is a guard against a
/// charge that is moved or removed, not an independent measure of work. (An
/// independent allocation count would need a counting global allocator, which is
/// `unsafe` code the workspace forbids.)
#[cfg(test)]
pub(crate) fn check_witness_counted(
    model: &Model,
    obligations: &Obligations,
    witness: &ReductionWitness,
    work: u64,
) -> (Result<CheckedWitness, WitnessDefect>, u64) {
    let mut touched = 0_u64;
    let result = Checker::new_counted(model, work, &mut touched)
        .and_then(|checker| checker.run(obligations, witness));
    (result, touched)
}

// ---------------------------------------------------------------------------
// the checker's own footprints and evaluation
// ---------------------------------------------------------------------------

// Charges multiply `usize` quantities with saturation. On a target narrower than 64
// bits a product would saturate below its true cost and undercharge; refuse to build
// there rather than meter wrongly.
const _: () = assert!(usize::BITS >= 64, "the checker meters in 64-bit usize");

/// The checker's charge sites that cr-1wuzry round 2 found unpaid; a test build can
/// leave one out to show the unpaid-work check fires there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckerOmission {
    /// No site: an ordinary charge.
    None,
    /// The per-node stubborn-set table.
    StubbornSlots,
    /// A search of a visit's growing donation set.
    DonationSearch,
}

#[derive(Debug, Clone, Copy)]
struct Print {
    reads: u64,
    writes: u64,
}

struct Checker<'m> {
    model: &'m Model,
    left: u64,
    /// Work units consumed at paid sites (test builds only; see `check_witness_counted`).
    #[cfg(test)]
    touched: Option<&'m mut u64>,
    /// Units prepaid for a work site and not yet used there (test builds only).
    #[cfg(test)]
    credit: u64,
    /// A charge left out, to show the unpaid-work check catches it (cr-1wuzry).
    #[cfg(test)]
    omit: Option<CheckerOmission>,
    names: BTreeMap<&'m str, usize>,
    /// `(action, outcome)` per label.
    table: Vec<(usize, usize)>,
    prints: Vec<Print>,
    cost: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Standing {
    Off,
    Loop,
    On(State),
}

impl<'m> Checker<'m> {
    fn new(model: &'m Model, work: u64) -> Result<Self, WitnessDefect> {
        Self::build(model, Self::blank(model, work))
    }

    #[cfg(test)]
    fn new_counted(
        model: &'m Model,
        work: u64,
        touched: &'m mut u64,
    ) -> Result<Self, WitnessDefect> {
        let mut blank = Self::blank(model, work);
        blank.touched = Some(touched);
        Self::build(model, blank)
    }

    fn blank(model: &'m Model, work: u64) -> Self {
        Self {
            model,
            left: work,
            #[cfg(test)]
            touched: None,
            #[cfg(test)]
            credit: 0,
            #[cfg(test)]
            omit: None,
            names: BTreeMap::new(),
            table: Vec::new(),
            prints: Vec::new(),
            cost: Vec::new(),
        }
    }

    /// Derive the label table, footprints and costs. Every allocation and every
    /// iteration proportional to the model is charged before it is made: the name
    /// table by its names' bytes, the action scan by the action count, the label
    /// tables by the label count, the assignments by their count and target names'
    /// bytes, and each expression node as it is visited.
    fn build(model: &'m Model, mut checker: Self) -> Result<Self, WitnessDefect> {
        // Variables: at most `MAX_VARIABLES`; each insert compares names by bytes.
        let name_bytes = model
            .variables()
            .iter()
            .map(|variable| variable.name().as_str().len().saturating_add(1))
            .fold(0_usize, usize::saturating_add);
        let paid = checker.pay(name_bytes.saturating_mul(8))?;
        checker.used(paid);
        for (position, variable) in model.variables().iter().enumerate() {
            checker.names.insert(variable.name().as_str(), position);
        }
        // Count the labels (one pass over the actions, paid first), then pay for the
        // three label tables before any of them is allocated.
        let paid = checker.pay(model.actions().len())?;
        checker.used(paid);
        let labels = model
            .actions()
            .iter()
            .map(|action| action.outcomes().len())
            .fold(0_usize, usize::saturating_add);
        let paid = checker.pay(labels.saturating_mul(4))?;
        checker.used(paid);
        checker.table.reserve_exact(labels);
        checker.prints.reserve_exact(labels);
        checker.cost.reserve_exact(labels);
        for (action_index, action) in model.actions().iter().enumerate() {
            let (guard_reads, guard_size) = checker.bool_reads(action.guard())?;
            for (outcome_index, outcome) in action.outcomes().iter().enumerate() {
                // The outcome's assignments: their count first (the byte sum below
                // walks them), then each target name's lookup, which compares bytes.
                let paid = checker.pay(outcome.assignments().len())?;
                checker.used(paid);
                let targets = outcome
                    .assignments()
                    .iter()
                    .map(|a| a.variable().as_str().len().saturating_add(1))
                    .fold(0_usize, usize::saturating_add);
                let paid = checker.pay(targets.saturating_add(1))?;
                checker.used(paid);
                let mut reads = guard_reads;
                let mut writes = 0_u64;
                let mut size =
                    guard_size.saturating_add(u64::try_from(targets).unwrap_or(u64::MAX));
                for assignment in outcome.assignments() {
                    writes |= checker.bit_of(assignment.variable().as_str());
                    let (value_reads, value_size) = checker.int_reads(assignment.value())?;
                    reads |= value_reads;
                    size = size.saturating_add(value_size);
                }
                checker.table.push((action_index, outcome_index));
                checker.prints.push(Print {
                    reads: reads | writes,
                    writes,
                });
                checker.cost.push(
                    size.saturating_add(u64::try_from(model.arity()).unwrap_or(u64::MAX))
                        .saturating_add(1),
                );
            }
        }
        Ok(checker)
    }

    /// Charge `units` for a work site that records them with [`Checker::used`];
    /// `site` names the site so a test build can leave this one charge out.
    #[allow(clippy::panic)]
    fn prepay(&mut self, units: usize, site: CheckerOmission) -> Result<(), WitnessDefect> {
        #[cfg(test)]
        if self.credit != 0 {
            panic!(
                "unpaid checker work: {} units of credit left by the last site",
                self.credit
            );
        }
        #[cfg(test)]
        if self.omit == Some(site) {
            return Ok(());
        }
        let _ = site;
        self.spend(units)?;
        #[cfg(test)]
        {
            self.credit = self
                .credit
                .saturating_add(u64::try_from(units).unwrap_or(u64::MAX));
        }
        Ok(())
    }

    /// Charge `units` for the work that follows, as credit its [`Checker::used`]
    /// consumes. Returns the units, for that call.
    fn pay(&mut self, units: usize) -> Result<usize, WitnessDefect> {
        self.prepay(units, CheckerOmission::None)?;
        Ok(units)
    }

    /// Consume `units` of the credit the site's own [`Checker::pay`] or
    /// [`Checker::prepay`] just provided, before the site's work. Test builds panic
    /// when the credit does not cover them: a site whose charge is removed, or moved
    /// after this call, fails every test that reaches it. `prepay` requires the
    /// credit to be zero on entry, so no site can live on another site's leftover.
    /// What this cannot see is a charge that is present but too small: each site's
    /// amount is argued where it is computed.
    #[cfg(test)]
    #[allow(clippy::panic)]
    fn used(&mut self, units: usize) {
        let units = u64::try_from(units).unwrap_or(u64::MAX);
        if let Some(touched) = self.touched.as_deref_mut() {
            *touched = touched.saturating_add(units);
        }
        match self.credit.checked_sub(units) {
            Some(left) => self.credit = left,
            None => panic!(
                "unpaid checker work: {units} units used, {} prepaid",
                self.credit
            ),
        }
    }

    #[cfg(not(test))]
    #[allow(clippy::unused_self)]
    const fn used(&self, _units: usize) {}

    fn spend(&mut self, units: usize) -> Result<(), WitnessDefect> {
        let units = u64::try_from(units).unwrap_or(u64::MAX);
        self.left = self
            .left
            .checked_sub(units)
            .ok_or(WitnessDefect::WorkExhausted)?;
        Ok(())
    }

    fn spend_units(&mut self, units: u64) -> Result<(), WitnessDefect> {
        self.left = self
            .left
            .checked_sub(units)
            .ok_or(WitnessDefect::WorkExhausted)?;
        Ok(())
    }

    fn bit_of(&self, name: &str) -> u64 {
        self.names
            .get(name)
            .and_then(|position| u32::try_from(*position).ok())
            .and_then(|shift| 1_u64.checked_shl(shift))
            // An unresolvable name reads everything: conservative.
            .unwrap_or(u64::MAX)
    }

    /// The variables an integer expression reads, and its node count, by an explicit
    /// stack. One unit per node, spent before the node is examined.
    fn int_reads(&mut self, root: &IntExpr) -> Result<(u64, u64), WitnessDefect> {
        let mut reads = 0_u64;
        let mut size = 0_u64;
        let mut pending: Vec<&IntExpr> = vec![root];
        while let Some(expr) = pending.pop() {
            self.spend(1)?;
            size = size.saturating_add(1);
            match expr {
                IntExpr::Const(_) => {}
                IntExpr::Var(name) => {
                    // The lookup compares the name's bytes.
                    self.spend(name.len())?;
                    reads |= self.bit_of(name);
                    size =
                        size.saturating_add(u64::try_from(self.model.arity()).unwrap_or(u64::MAX));
                }
                IntExpr::Arith(_, a, b) | IntExpr::Min(a, b) | IntExpr::Max(a, b) => {
                    pending.push(a);
                    pending.push(b);
                }
            }
        }
        Ok((reads, size))
    }

    /// The variables a Boolean expression reads, and its node count.
    fn bool_reads(&mut self, root: &BoolExpr) -> Result<(u64, u64), WitnessDefect> {
        let mut reads = 0_u64;
        let mut size = 0_u64;
        let mut pending: Vec<&BoolExpr> = vec![root];
        while let Some(expr) = pending.pop() {
            self.spend(1)?;
            size = size.saturating_add(1);
            match expr {
                BoolExpr::Const(_) => {}
                BoolExpr::Compare { left, right, .. } => {
                    for side in [left, right] {
                        let (more, nodes) = self.int_reads(side)?;
                        reads |= more;
                        size = size.saturating_add(nodes);
                    }
                }
                BoolExpr::InRange { expr, .. } => {
                    let (more, nodes) = self.int_reads(expr)?;
                    reads |= more;
                    size = size.saturating_add(nodes);
                }
                BoolExpr::Not(inner) => pending.push(inner),
                BoolExpr::And(a, b) | BoolExpr::Or(a, b) | BoolExpr::Implies(a, b) => {
                    pending.push(a);
                    pending.push(b);
                }
            }
        }
        Ok((reads, size))
    }

    fn print(&self, label: usize) -> Print {
        self.prints.get(label).copied().unwrap_or(Print {
            reads: u64::MAX,
            writes: u64::MAX,
        })
    }

    /// Independence from the checker's own footprints.
    fn independent(&self, a: usize, b: usize) -> bool {
        let (x, y) = (self.print(a), self.print(b));
        a != b && x.writes & y.reads == 0 && y.writes & x.reads == 0
    }

    /// Fire `label` at `state`: its standing, or `None` when evaluation fails.
    fn fire(&mut self, label: usize, state: &State) -> Result<Option<Standing>, WitnessDefect> {
        // A label outside the table is a malformed witness, never a budget question:
        // look it up before charging, so no budget turns it into `WorkExhausted`.
        let (Some(&(action_index, outcome_index)), Some(&cost)) =
            (self.table.get(label), self.cost.get(label))
        else {
            return Err(WitnessDefect::Dangling { what: "label" });
        };
        self.spend_units(cost)?;
        let Some(action) = self.model.actions().get(action_index) else {
            return Err(WitnessDefect::Dangling { what: "action" });
        };
        let Some(outcome) = action.outcomes().get(outcome_index) else {
            return Err(WitnessDefect::Dangling { what: "outcome" });
        };
        let scope = Environment::new(self.model.variables(), state.as_slice());
        match action.guard().evaluate(&scope) {
            Ok(false) => return Ok(Some(Standing::Off)),
            Ok(true) => {}
            Err(_) => return Ok(None),
        }
        let mut next = state.as_slice().to_vec();
        for assignment in outcome.assignments() {
            let Ok(value) = assignment.value().evaluate(&scope) else {
                return Ok(None);
            };
            let Some(&position) = self.names.get(assignment.variable().as_str()) else {
                return Ok(None);
            };
            match next.get_mut(position) {
                Some(slot) => *slot = value,
                None => return Ok(None),
            }
        }
        // `Model::state` checks every declared domain: an out-of-domain update is a
        // failed evaluation, as `Model::action_successors` reports it.
        let Ok(target) = self.model.state(&next) else {
            return Ok(None);
        };
        Ok(Some(if target == *state {
            Standing::Loop
        } else {
            Standing::On(target)
        }))
    }

    /// Whether `a` and `b` are both progress transitions at `state` and commute there.
    fn commute(&mut self, state: &State, a: usize, b: usize) -> Result<bool, WitnessDefect> {
        let (Some(Standing::On(after_a)), Some(Standing::On(after_b))) =
            (self.fire(a, state)?, self.fire(b, state)?)
        else {
            return Ok(false);
        };
        let (Some(Standing::On(ab)), Some(Standing::On(ba))) =
            (self.fire(b, &after_a)?, self.fire(a, &after_b)?)
        else {
            return Ok(false);
        };
        Ok(ab == ba)
    }

    // -----------------------------------------------------------------------
    // the check
    // -----------------------------------------------------------------------

    #[allow(clippy::too_many_lines)]
    fn run(
        mut self,
        obligations: &Obligations,
        witness: &ReductionWitness,
    ) -> Result<CheckedWitness, WitnessDefect> {
        if !witness.is_complete() {
            return Err(WitnessDefect::Incomplete);
        }
        let width = self.table.len();
        // The comparison stops at a length mismatch, and otherwise reads each entry.
        let paid = self.pay(width.saturating_add(1))?;
        self.used(paid);
        if witness.labels() != self.table.as_slice() {
            return Err(WitnessDefect::LabelTable);
        }
        let arity = self.model.arity();
        let states_per_node = arity.saturating_add(1);
        // The observed predicates: every action's definedness chain, and each
        // invariant's own chain and itself, as model-core's `Definedness` reads the
        // model (RFC 0003) — the classification is the model's, not a second one.
        // Its name work is paid first.
        let names = self
            .model
            .predicates()
            .len()
            .saturating_add(self.model.actions().len());
        let paid = self.pay(names)?;
        self.used(paid);
        let levels = log2(names).saturating_add(1);
        let name_work = self
            .model
            .predicates()
            .iter()
            .map(|predicate| predicate.name().as_str().len())
            .chain(
                self.model
                    .actions()
                    .iter()
                    .map(|action| action.name().as_str().len()),
            )
            .map(|len| {
                len.saturating_add(1)
                    .saturating_mul((len / 8).saturating_add(2))
                    .saturating_mul(levels)
                    // An asymptotic bound on `Definedness::of` (two searches, a scan,
                    // an insert and a sort per level), with the observed map's inserts.
                    .saturating_mul(4)
            })
            .fold(0_usize, usize::saturating_add);
        let paid = self.pay(name_work)?;
        self.used(paid);
        let definedness = Definedness::of(self.model);
        let action_chains: Vec<Vec<usize>> =
            definedness.action_chains().map(<[usize]>::to_vec).collect();
        let mut observed: BTreeMap<usize, usize> = BTreeMap::new();
        let mut per_state = 0_usize;
        let mut visible = 0_u64;
        let mut size_of = |checker: &mut Self, index: usize| -> Result<usize, WitnessDefect> {
            if let Some(size) = observed.get(&index) {
                return Ok(*size);
            }
            let Some(predicate) = checker.model.predicates().get(index) else {
                return Err(WitnessDefect::Dangling { what: "predicate" });
            };
            let (reads, size) = checker.bool_reads(predicate.body())?;
            visible |= reads;
            let size = usize::try_from(size)
                .unwrap_or(usize::MAX)
                .saturating_add(checker.model.arity());
            observed.insert(index, size);
            Ok(size)
        };
        for chain in &action_chains {
            for &guard in chain {
                per_state = per_state.saturating_add(size_of(&mut self, guard)?);
            }
        }
        for index in obligations.invariants() {
            for &guard in definedness.guards_of(*index) {
                per_state = per_state.saturating_add(size_of(&mut self, guard)?);
            }
            per_state = per_state.saturating_add(size_of(&mut self, *index)?);
        }
        if visible != witness.visible() {
            return Err(WitnessDefect::VisibleSet {
                claimed: witness.visible(),
                derived: visible,
            });
        }

        let nodes = witness.nodes();
        let count = nodes.len();
        // The index: a logarithmic number of state comparisons per insert, each over
        // the state's components (a witness state may be any length: charge its own).
        let depth = log2(count).saturating_add(1);
        let mut at: BTreeMap<&State, usize> = BTreeMap::new();
        for (index, node) in nodes.iter().enumerate() {
            let paid = self.pay(node.state().arity().saturating_add(1).saturating_mul(depth))?;
            self.used(paid);
            if at.insert(node.state(), index).is_some() {
                return Err(WitnessDefect::DuplicateState { node: index });
            }
        }

        // Standing of every label at every stored state; the enabled sets.
        let paid = self.pay(count)?;
        self.used(paid);
        let mut enabled: Vec<Vec<bool>> = Vec::with_capacity(count);
        let mut deadlocks: Vec<State> = Vec::new();
        let labels_depth = log2(width).saturating_add(1);
        for (index, node) in nodes.iter().enumerate() {
            // The flag row, and per label the successor kept for the cross-check (a
            // state copy and its ordered insert), before any of it is built.
            let paid = self.pay(
                width.saturating_mul(
                    states_per_node
                        .saturating_mul(labels_depth)
                        .saturating_add(1),
                ),
            )?;
            self.used(paid);
            let mut on = vec![false; width];
            let mut any = false;
            let mut per_action: BTreeMap<usize, BTreeSet<State>> = BTreeMap::new();
            for (label, slot) in on.iter_mut().enumerate() {
                let Some(standing) = self.fire(label, node.state())? else {
                    return Err(WitnessDefect::Evaluation { node: index, label });
                };
                let action = self.table.get(label).map_or(usize::MAX, |entry| entry.0);
                match standing {
                    Standing::Off => {}
                    Standing::Loop => {
                        any = true;
                        per_action
                            .entry(action)
                            .or_default()
                            .insert(node.state().clone());
                    }
                    Standing::On(target) => {
                        any = true;
                        *slot = true;
                        per_action.entry(action).or_default().insert(target);
                    }
                }
            }
            // Cross-check the checker's evaluation with the model layer's own
            // successor relation, action by action.
            // `Model::action_successors` evaluates each action's guard and every
            // outcome again: the same cost as firing every label once more.
            let again = self
                .cost
                .iter()
                .fold(0_u64, |sum, one| sum.saturating_add(*one));
            let paid = self.pay(usize::try_from(again).unwrap_or(usize::MAX))?;
            self.used(paid);
            for action in 0..self.model.actions().len() {
                self.spend(1)?;
                let Ok(targets) = self.model.action_successors(action, node.state()) else {
                    return Err(WitnessDefect::SuccessorMismatch {
                        node: index,
                        action,
                    });
                };
                let mine: Vec<State> = per_action
                    .get(&action)
                    .map(|set| set.iter().cloned().collect())
                    .unwrap_or_default();
                if mine != targets {
                    return Err(WitnessDefect::SuccessorMismatch {
                        node: index,
                        action,
                    });
                }
            }
            if !any {
                let paid = self.pay(states_per_node)?;
                self.used(paid);
                deadlocks.push(node.state().clone());
            }
            enabled.push(on);
        }
        let paid = self.pay(
            deadlocks
                .len()
                .saturating_mul(states_per_node)
                .saturating_mul(log2(deadlocks.len()).saturating_add(1)),
        )?;
        self.used(paid);
        deadlocks.sort();

        // Expansions: every reduced state's stubborn set is closed, seeded, invisible,
        // and its enabled members commute with the enabled labels it leaves out.
        // One slot per node, full or reduced, and each node's constant pass below,
        // paid before the table is allocated.
        let slots = count.saturating_mul(2);
        self.prepay(slots, CheckerOmission::StubbornSlots)?;
        self.used(slots);
        let mut stubborn_sets: Vec<Option<BTreeSet<usize>>> = Vec::with_capacity(count);
        let mut diamonds = 0_usize;
        for (index, node) in nodes.iter().enumerate() {
            let on = enabled
                .get(index)
                .ok_or(WitnessDefect::Dangling { what: "node" })?;
            let is_on = |label: usize| on.get(label).copied().unwrap_or(false);
            match node.expansion() {
                Expansion::Full(_) => stubborn_sets.push(None),
                Expansion::Reduced { seed, stubborn } => {
                    // The claimed set is witness data of any length: pay for reading
                    // and ordering it before it is read.
                    let paid = self.pay(
                        stubborn
                            .len()
                            .saturating_mul(log2(stubborn.len()).saturating_add(1))
                            .saturating_add(width),
                    )?;
                    self.used(paid);
                    let set: BTreeSet<usize> = stubborn.iter().copied().collect();
                    if set.iter().any(|label| *label >= width) {
                        return Err(WitnessDefect::Dangling {
                            what: "stubborn label",
                        });
                    }
                    if !set.contains(seed) || !is_on(*seed) {
                        return Err(WitnessDefect::Seed { node: index });
                    }
                    // Each member against every label, each test a search of the set.
                    let paid = self.pay(
                        set.len()
                            .saturating_mul(width)
                            .saturating_mul(log2(set.len()).saturating_add(1)),
                    )?;
                    self.used(paid);
                    for &member in &set {
                        let mine = self.print(member);
                        for other in 0..width {
                            if other == member || set.contains(&other) {
                                continue;
                            }
                            let theirs = self.print(other);
                            let needed = if is_on(member) {
                                !self.independent(member, other)
                            } else {
                                theirs.writes & mine.reads != 0
                            };
                            if needed {
                                return Err(WitnessDefect::NotClosed {
                                    node: index,
                                    member,
                                    missing: other,
                                });
                            }
                        }
                        if is_on(member) && mine.writes & visible != 0 {
                            return Err(WitnessDefect::Visible {
                                node: index,
                                label: member,
                            });
                        }
                    }
                    let inside: Vec<usize> = set.iter().copied().filter(|l| is_on(*l)).collect();
                    let outside: Vec<usize> = (0..width)
                        .filter(|l| is_on(*l) && !set.contains(l))
                        .collect();
                    for &first in &inside {
                        for &second in &outside {
                            diamonds = diamonds.saturating_add(1);
                            if !self.commute(node.state(), first, second)? {
                                return Err(WitnessDefect::DoesNotCommute {
                                    node: index,
                                    first,
                                    second,
                                });
                            }
                        }
                    }
                    stubborn_sets.push(Some(set));
                }
            }
        }

        // Visits and their edges.
        let visits = witness.visits();
        let total = visits.len();
        let paid = self.pay(total)?;
        self.used(paid);
        let mut successors: Vec<Vec<usize>> = Vec::with_capacity(total);
        let mut commuted: BTreeSet<(usize, usize, usize)> = BTreeSet::new();
        let mut edges = 0_usize;
        for (index, visit) in visits.iter().enumerate() {
            let state = nodes
                .get(visit.node())
                .ok_or(WitnessDefect::Dangling { what: "visit node" })?
                .state();
            let on = enabled
                .get(visit.node())
                .ok_or(WitnessDefect::Dangling { what: "visit node" })?;
            let sleep = visit.entry_sleep();
            // The sleep set is read, checked and copied into an ordered set; the edge
            // list is sized and walked, each edge comparing a state.
            let paid = self.pay(
                sleep
                    .len()
                    .saturating_mul(log2(sleep.len()).saturating_add(2))
                    .saturating_add(
                        visit
                            .edges()
                            .len()
                            .saturating_mul(states_per_node.saturating_add(2)),
                    )
                    .saturating_add(1),
            )?;
            self.used(paid);
            if !sleep.windows(2).all(|pair| pair.first() < pair.get(1))
                || sleep
                    .iter()
                    .any(|label| !on.get(*label).copied().unwrap_or(false))
            {
                return Err(WitnessDefect::Sleep {
                    visit: index,
                    label: usize::MAX,
                });
            }
            let mut donated: BTreeSet<usize> = sleep.iter().copied().collect();
            let mut targets: Vec<usize> = Vec::with_capacity(visit.edges().len());
            for edge in visit.edges() {
                edges = edges.saturating_add(1);
                let (label, target) = (edge.label(), edge.target());
                if label >= width {
                    return Err(WitnessDefect::Dangling { what: "edge label" });
                }
                let child = visits.get(target).ok_or(WitnessDefect::Dangling {
                    what: "edge target",
                })?;
                let child_state = nodes
                    .get(child.node())
                    .ok_or(WitnessDefect::Dangling { what: "visit node" })?
                    .state();
                if self.fire(label, state)? != Some(Standing::On(child_state.clone())) {
                    return Err(WitnessDefect::Edge {
                        visit: index,
                        label,
                    });
                }
                // The child's sleep set: members asleep here (entry, or donated by an
                // earlier edge), independent of the fired label, and commuting with it.
                let paid = self.pay(child.entry_sleep().len().saturating_mul(
                    log2(sleep.len().saturating_add(visit.edges().len())).saturating_add(2),
                ))?;
                self.used(paid);
                for &member in child.entry_sleep() {
                    if !donated.contains(&member) || !self.independent(member, label) {
                        return Err(WitnessDefect::Sleep {
                            visit: target,
                            label: member,
                        });
                    }
                    // The memo is global: its search costs the log of its own size.
                    let paid = self.pay(log2(commuted.len()).saturating_add(1))?;
                    self.used(paid);
                    if commuted.insert((visit.node(), member, label)) {
                        diamonds = diamonds.saturating_add(1);
                        if !self.commute(state, member, label)? {
                            return Err(WitnessDefect::Sleep {
                                visit: target,
                                label: member,
                            });
                        }
                    }
                }
                if edge.donated() {
                    // The donation set grows with the edges: each insert is a search
                    // of its current size, paid first.
                    let search = log2(donated.len()).saturating_add(1).saturating_mul(2);
                    self.prepay(search, CheckerOmission::DonationSearch)?;
                    self.used(search);
                    donated.insert(label);
                }
                targets.push(target);
            }
            successors.push(targets);
        }

        // Roots: one per initial state, in order, entered with an empty sleep set.
        let initial = self.model.initial_states();
        if witness.roots().len() != initial.len() {
            return Err(WitnessDefect::Root {
                at: witness.roots().len(),
            });
        }
        let paid = self.pay(initial.len().saturating_mul(states_per_node))?;
        self.used(paid);
        for (position, (root, state)) in witness.roots().iter().zip(initial).enumerate() {
            let rooted = visits.get(*root).is_some_and(|visit| {
                visit.entry_sleep().is_empty()
                    && nodes
                        .get(visit.node())
                        .is_some_and(|node| node.state() == state)
            });
            if !rooted {
                return Err(WitnessDefect::Root { at: position });
            }
        }

        // Reachability: every visit from a root, and every node through a visit.
        let paid = self.pay(
            total
                .saturating_mul(2)
                .saturating_add(edges)
                .saturating_add(count),
        )?;
        self.used(paid);
        let mut reached = vec![false; total];
        let mut frontier: Vec<usize> = witness.roots().to_vec();
        while let Some(visit) = frontier.pop() {
            let Some(flag) = reached.get_mut(visit) else {
                return Err(WitnessDefect::Dangling { what: "root" });
            };
            if *flag {
                continue;
            }
            *flag = true;
            if let Some(next) = successors.get(visit) {
                frontier.extend(next.iter().copied());
            }
        }
        if let Some(visit) = reached.iter().position(|flag| !flag) {
            return Err(WitnessDefect::Unreached { visit });
        }
        let mut visited_nodes = vec![false; count];
        for visit in visits {
            if let Some(flag) = visited_nodes.get_mut(visit.node()) {
                *flag = true;
            }
        }
        if let Some(node) = visited_nodes.iter().position(|flag| !flag) {
            return Err(WitnessDefect::NodeWithoutVisit { node });
        }

        // Coverage: every enabled label of every visit is explored, outside the
        // stubborn set of a reduced visit, or asleep on entry.
        let mut by_persistence = 0_usize;
        let mut by_sleep = 0_usize;
        for (index, visit) in visits.iter().enumerate() {
            let on = enabled
                .get(visit.node())
                .ok_or(WitnessDefect::Dangling { what: "visit node" })?;
            let stubborn = stubborn_sets
                .get(visit.node())
                .ok_or(WitnessDefect::Dangling { what: "visit node" })?;
            let paid = self.pay(
                visit
                    .edges()
                    .len()
                    .saturating_mul(log2(visit.edges().len()).saturating_add(1))
                    .saturating_add(
                        width.saturating_mul(
                            // Per label: a search of the explored set, the stubborn set,
                            // and the entry sleep set, whichever is largest.
                            log2(
                                visit
                                    .edges()
                                    .len()
                                    .max(visit.entry_sleep().len())
                                    .max(stubborn.as_ref().map_or(0, BTreeSet::len)),
                            )
                            .saturating_add(1)
                            .saturating_mul(3),
                        ),
                    ),
            )?;
            self.used(paid);
            let explored: BTreeSet<usize> = visit.edges().iter().map(|edge| edge.label()).collect();
            for label in 0..width {
                if !on.get(label).copied().unwrap_or(false) || explored.contains(&label) {
                    continue;
                }
                if !visit.proviso() && stubborn.as_ref().is_some_and(|set| !set.contains(&label)) {
                    by_persistence = by_persistence.saturating_add(1);
                } else if visit.entry_sleep().binary_search(&label).is_ok() {
                    by_sleep = by_sleep.saturating_add(1);
                } else {
                    return Err(WitnessDefect::Uncovered {
                        visit: index,
                        label,
                    });
                }
            }
        }

        // Components of the visit graph (Kosaraju, iterative): no donated edge inside
        // one, and no cycle among reduced visits.
        // Kosaraju (predecessor lists, two passes), the donation scan, and Kahn's
        // pass over reduced visits: each linear in visits and edges.
        let paid = self.pay(total.saturating_add(edges).saturating_mul(8))?;
        self.used(paid);
        let component = components(&successors)?;
        for (index, visit) in visits.iter().enumerate() {
            for edge in visit.edges() {
                if edge.donated() && component.get(index) == component.get(edge.target()) {
                    return Err(WitnessDefect::DonatedInsideComponent {
                        visit: index,
                        label: edge.label(),
                    });
                }
            }
        }
        let reduced: Vec<bool> = visits
            .iter()
            .map(|visit| {
                !visit.proviso() && stubborn_sets.get(visit.node()).is_some_and(Option::is_some)
            })
            .collect();
        let is_reduced = |visit: usize| reduced.get(visit).copied().unwrap_or(false);
        let mut indegree: Vec<usize> = vec![0; total];
        for (index, next) in successors.iter().enumerate() {
            if !is_reduced(index) {
                continue;
            }
            for &target in next {
                if is_reduced(target)
                    && let Some(slot) = indegree.get_mut(target)
                {
                    *slot = slot.saturating_add(1);
                }
            }
        }
        let mut ready: Vec<usize> = (0..total)
            .filter(|visit| is_reduced(*visit) && indegree.get(*visit) == Some(&0))
            .collect();
        let mut removed = 0_usize;
        while let Some(visit) = ready.pop() {
            removed = removed.saturating_add(1);
            for &target in successors.get(visit).map_or(&[][..], Vec::as_slice) {
                if !is_reduced(target) {
                    continue;
                }
                if let Some(slot) = indegree.get_mut(target) {
                    *slot = slot.saturating_sub(1);
                    if *slot == 0 {
                        ready.push(target);
                    }
                }
            }
        }
        if removed != reduced.iter().filter(|flag| **flag).count() {
            return Err(WitnessDefect::ReducedCycle);
        }

        // The checker's own verdicts over the stored states.
        let mut invariants: Vec<(usize, CheckedInvariant)> = Vec::new();
        // Projections: one copy of each state's visible part, ordered into a set.
        let paid = self.pay(
            count
                .saturating_mul(self.model.arity().saturating_add(1))
                .saturating_mul(log2(count).saturating_add(1)),
        )?;
        self.used(paid);
        // One scan over the stored states in node order, in the reference engine's
        // precedence: every action's chain (deepest first, first false wins, nothing
        // else read there), then per invariant its own chain, then the invariant.
        let paid = self.pay(per_state.saturating_mul(count))?;
        self.used(paid);
        let asked: Vec<usize> = obligations.invariants().iter().copied().collect();
        let mut failed = vec![false; asked.len()];
        let mut subject: Vec<Option<(usize, State)>> = vec![None; asked.len()];
        let mut violated: Vec<Option<State>> = vec![None; asked.len()];
        let mut action_undefined: Option<(usize, State)> = None;
        let mut action_failed = false;
        for node in nodes {
            let state = node.state();
            let mut undefined_here = None;
            'chains: for chain in &action_chains {
                for &guard in chain {
                    match self.model.evaluate_predicate(guard, state) {
                        Ok(true) => {}
                        Ok(false) => {
                            undefined_here = Some(guard);
                            break 'chains;
                        }
                        Err(_) => {
                            action_failed = true;
                            break 'chains;
                        }
                    }
                }
            }
            if let Some(guard) = undefined_here {
                if action_undefined.is_none() {
                    action_undefined = Some((guard, state.clone()));
                }
                continue;
            }
            if action_failed {
                // Nothing further is read at a state whose action chain failed.
                continue;
            }
            for (slot, index) in asked.iter().enumerate() {
                let mut chain_false = None;
                let mut chain_failed = false;
                for &guard in definedness.guards_of(*index) {
                    match self.model.evaluate_predicate(guard, state) {
                        Ok(true) => {}
                        Ok(false) => {
                            chain_false = Some(guard);
                            break;
                        }
                        Err(_) => {
                            chain_failed = true;
                            break;
                        }
                    }
                }
                let (Some(fail), Some(undefined), Some(violation)) = (
                    failed.get_mut(slot),
                    subject.get_mut(slot),
                    violated.get_mut(slot),
                ) else {
                    return Err(WitnessDefect::Dangling { what: "obligation" });
                };
                if chain_failed {
                    *fail = true;
                    continue;
                }
                if let Some(guard) = chain_false {
                    if undefined.is_none() {
                        *undefined = Some((guard, state.clone()));
                    }
                    continue;
                }
                match self.model.evaluate_predicate(*index, state) {
                    Ok(true) => {}
                    Ok(false) if definedness.guards(*index).is_some() => {
                        if undefined.is_none() {
                            *undefined = Some((*index, state.clone()));
                        }
                    }
                    Ok(false) => {
                        if violation.is_none() {
                            *violation = Some(state.clone());
                        }
                    }
                    Err(_) => *fail = true,
                }
            }
        }
        for (slot, index) in asked.iter().enumerate() {
            let verdict = if action_failed || failed.get(slot).copied().unwrap_or(true) {
                CheckedInvariant::EvaluationFailed
            } else if let Some((guard, state)) = &action_undefined {
                CheckedInvariant::Undefined {
                    guard: *guard,
                    state: state.clone(),
                }
            } else if let Some(Some((guard, state))) = subject.get(slot) {
                CheckedInvariant::Undefined {
                    guard: *guard,
                    state: state.clone(),
                }
            } else if let Some(Some(state)) = violated.get(slot) {
                CheckedInvariant::Violated(state.clone())
            } else {
                CheckedInvariant::Holds
            };
            invariants.push((*index, verdict));
        }
        let projections = nodes
            .iter()
            .map(|node| {
                node.state()
                    .as_slice()
                    .iter()
                    .enumerate()
                    .filter(|(position, _)| {
                        u32::try_from(*position)
                            .ok()
                            .and_then(|shift| 1_u64.checked_shl(shift))
                            .is_some_and(|bit| bit & visible != 0)
                    })
                    .map(|(_, value)| *value)
                    .collect()
            })
            .collect();

        Ok(CheckedWitness {
            nodes: count,
            visits: total,
            edges,
            declined_by_persistence: by_persistence,
            declined_by_sleep: by_sleep,
            diamonds,
            invariants,
            deadlocks,
            projections,
            undefined_action: if action_failed {
                None
            } else {
                action_undefined
            },
            action_evaluation_failed: action_failed,
        })
    }
}

/// The strongly connected component of every vertex of `successors`, by Kosaraju's
/// two passes with explicit stacks (no recursion, so the graph's depth is not bounded
/// by the thread's stack). Component numbers are arbitrary; only equality matters.
fn components(successors: &[Vec<usize>]) -> Result<Vec<usize>, WitnessDefect> {
    let total = successors.len();
    let mut predecessors: Vec<Vec<usize>> = vec![Vec::new(); total];
    for (source, next) in successors.iter().enumerate() {
        for &target in next {
            predecessors
                .get_mut(target)
                .ok_or(WitnessDefect::Dangling {
                    what: "edge target",
                })?
                .push(source);
        }
    }
    // First pass: finishing order on the graph.
    let mut seen = vec![false; total];
    let mut order: Vec<usize> = Vec::with_capacity(total);
    for start in 0..total {
        if seen.get(start).copied().unwrap_or(true) {
            continue;
        }
        let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
        if let Some(flag) = seen.get_mut(start) {
            *flag = true;
        }
        while let Some((vertex, cursor)) = stack.pop() {
            let next = successors
                .get(vertex)
                .and_then(|list| list.get(cursor))
                .copied();
            match next {
                Some(target) => {
                    stack.push((vertex, cursor.saturating_add(1)));
                    if let Some(flag) = seen.get_mut(target)
                        && !*flag
                    {
                        *flag = true;
                        stack.push((target, 0));
                    }
                }
                None => order.push(vertex),
            }
        }
    }
    // Second pass: reverse finishing order on the transposed graph.
    let mut component = vec![usize::MAX; total];
    let mut number = 0_usize;
    for &start in order.iter().rev() {
        if component.get(start).copied() != Some(usize::MAX) {
            continue;
        }
        let mut stack: Vec<usize> = vec![start];
        if let Some(slot) = component.get_mut(start) {
            *slot = number;
        }
        while let Some(vertex) = stack.pop() {
            for &source in predecessors.get(vertex).map_or(&[][..], Vec::as_slice) {
                if let Some(slot) = component.get_mut(source)
                    && *slot == usize::MAX
                {
                    *slot = number;
                    stack.push(source);
                }
            }
        }
        number = number.saturating_add(1);
    }
    Ok(component)
}

#[cfg(test)]
#[path = "checker_tests.rs"]
mod tests;

/// The bit length of `n` (`⌊log₂ n⌋ + 1`, and 0 for 0), an upper bound on the
/// comparisons of one ordered-collection search, for charging it.
fn log2(n: usize) -> usize {
    usize::try_from(usize::BITS.saturating_sub(n.leading_zeros())).unwrap_or(64)
}
