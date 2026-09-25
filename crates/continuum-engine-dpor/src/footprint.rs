//! Labels, their read/write footprints, and the conservative conflict relation.
//!
//! # Labels
//!
//! A *label* is one outcome of one action: `(action, outcome)`. A deterministic
//! action is one label; an action with `n` enumerated outcomes is `n` labels sharing
//! a guard. Each label is a deterministic partial function on states, which is the
//! transition shape the persistent-set and sleep-set theory is stated for.
//!
//! The reducer explores the *progress* restriction of each label: label `l` is
//! enabled at `s` when its guard holds **and** firing it changes the state. A firing
//! that leaves the state unchanged is a self-loop; dropping self-loops removes no
//! reachable state, and it keeps an always-enabled no-op (the register's re-choice of
//! its own value) from closing a one-state cycle at every state.
//!
//! # Footprints, derived and never declared
//!
//! A label's footprint is computed from the model's own expressions:
//!
//! - **writes** `W(l)`: the variables the outcome assigns (an assignment `x := x`
//!   counts: the footprint is syntactic and over-approximates);
//! - **reads** `R(l)`: every variable the guard mentions, every variable any
//!   right-hand side of the outcome mentions, and `W(l)` itself — the progress
//!   condition compares the written variables' old and new values, so it reads them.
//!
//! Masks are `u64`: a model declares at most `MAX_VARIABLES` = 64 variables
//! (`continuum_model_core::model::MAX_VARIABLES`), so one bit per variable is exact.
//!
//! # The conflict relation
//!
//! Two distinct labels are **independent** exactly when neither writes a variable the
//! other reads or writes: `W(a) ∩ R(b) = ∅` and `W(b) ∩ R(a) = ∅` (with `W ⊆ R`, this
//! also excludes write/write overlap). Otherwise they are dependent. The argument that
//! this is sound — that the relation over-approximates semantic dependence and never
//! under-approximates it — is short, because the model language is closed
//! (`continuum_model_core::expr`: constants, variables, checked arithmetic, `min`,
//! `max`, comparisons, connectives; no function calls, no indirection, no ambient
//! input):
//!
//! 1. every value an evaluation of `l` observes is the value of a variable in `R(l)`,
//!    so `l`'s guard, its progress condition, its new values, and any evaluation
//!    error it raises are functions of the `R(l)` projection of the state;
//! 2. firing `b` changes only `W(b)`, which `R(a)` does not meet — so `b` neither
//!    enables, disables, nor alters `a`, nor makes `a` fail, and symmetrically;
//! 3. the two writes land on disjoint variables, so both orders reach one state.
//!
//! That is the textbook independence relation (Godefroid, LNCS 1032, ch. 3)
//! established by construction. RFC 0004 rejects "treating read/write footprints as
//! a proof of commutativity"; here the footprint is not *treated* as one, it *is* one
//! for this closed language. Two limits are stated rather than hidden: the
//! reduction-witness checker ([`crate::checker`]) re-derives every footprint with its
//! own walker from the same written definition, and fires both orders of two labels
//! only at the concrete states where a reduction relied on them directly (a reduced
//! state's persistent set against the labels it leaves out, a sleeping label against
//! the edge that carried it); persistence along the paths that avoid a stubborn set
//! rests on the footprint rule itself. And the model layer's evaluator is shared by
//! the reducer, the checker and the oracle, so a defect there would reach all three
//! alike.
//!
//! [`DependenceEvidence`] is RFC 0004's shape. The derivation here never produces
//! [`DependenceEvidence::Unknown`]; the public [`crate::dependence`] returns it for a
//! label outside the model's table or a work bound too small to derive the
//! footprints, and every consumer treats `Unknown` as dependent ("`Unknown` is
//! dependent", RFC 0004 "Dependence evidence").

use std::collections::{BTreeMap, BTreeSet};

use continuum_model_core::definedness::Definedness;
use continuum_model_core::expr::{BoolExpr, IntExpr};
use continuum_model_core::model::Model;

use crate::bits::LabelSet;
use crate::budget::Meter;
use crate::report::Bound;

/// One action outcome: the unit the reducer schedules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Label {
    /// The action index, in `Model::actions` order.
    pub action: usize,
    /// The outcome index, in the action's declaration order.
    pub outcome: usize,
}

/// A label's derived footprint, one bit per variable in canonical order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Footprint {
    /// Variables an evaluation of the label reads (guard, right-hand sides, and the
    /// written variables, which the progress condition compares).
    pub reads: u64,
    /// Variables the label assigns.
    pub writes: u64,
}

/// Why two labels are dependent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conflict {
    /// A label is dependent with itself.
    SameLabel,
    /// `writer` assigns variable `variable`, which the other label reads or writes.
    Writes {
        /// The label that writes.
        writer: usize,
        /// The lowest variable index in the overlap.
        variable: usize,
    },
}

/// RFC 0004's dependence evidence for one pair of labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependenceEvidence {
    /// The two footprints are disjoint in the sense of the module documentation; the
    /// footprints themselves are the proof reference.
    DefinitelyIndependent {
        /// The first label's footprint.
        first: Footprint,
        /// The second label's footprint.
        second: Footprint,
    },
    /// The footprints overlap.
    DefinitelyDependent(Conflict),
    /// No answer. Treated as dependent everywhere.
    Unknown,
}

impl DependenceEvidence {
    /// Whether a consumer must treat the pair as dependent: `Unknown` is dependent.
    #[must_use]
    pub const fn is_dependent(&self) -> bool {
        !matches!(self, Self::DefinitelyIndependent { .. })
    }
}

/// The label table of one model, with footprints, evaluation costs, the observers'
/// visible variables, and the precomputed conflict and enabling relations.
#[derive(Debug, Clone)]
pub(crate) struct Footprints<'m> {
    labels: Vec<Label>,
    prints: Vec<Footprint>,
    /// Per label: the guard's and the outcome's expression nodes, the cost of one
    /// evaluation.
    cost: Vec<u64>,
    /// Per label: the resolved assignments `(variable index, expression)`.
    updates: Vec<Vec<(usize, &'m IntExpr)>>,
    /// Variables the observed predicates read: the invariants, their definedness
    /// chains, and every action's definedness chain.
    visible: u64,
    /// The observed predicates' expression nodes and state checks: the cost of
    /// evaluating all of them once.
    predicate_cost: u64,
    /// The model's definedness predicates, as `continuum_model_core` classifies them.
    definedness: Definedness,
    /// Per label: every other label it is dependent with.
    dependents: Vec<LabelSet>,
    /// Per label: every label that writes a variable it reads — a necessary enabling
    /// set for it at any state where it is not enabled.
    enablers: Vec<LabelSet>,
}

impl<'m> Footprints<'m> {
    /// Derive every footprint of `model`, the visible variables of `invariants`, and
    /// the two relations. Each expression node is charged before it is visited, and
    /// the relations' `labels × labels` pair tests and bitset words are charged before
    /// either is built.
    pub(crate) fn derive(
        model: &'m Model,
        invariants: &[usize],
        meter: &mut Meter,
        write_read_conflicts: bool,
        observe_definedness: bool,
    ) -> Result<Self, Bound> {
        let mut index = Resolver::new(model);
        let mut labels: Vec<Label> = Vec::new();
        let mut prints: Vec<Footprint> = Vec::new();
        let mut cost: Vec<u64> = Vec::new();
        let mut updates: Vec<Vec<(usize, &'m IntExpr)>> = Vec::new();
        for (action_index, action) in model.actions().iter().enumerate() {
            let mut guard_reads = 0_u64;
            let guard_nodes = walk_bool(action.guard(), &mut index, &mut guard_reads, meter)?;
            for (outcome_index, outcome) in action.outcomes().iter().enumerate() {
                let paid = meter.pay(1)?;
                meter.did(paid);
                let mut reads = guard_reads;
                let mut writes = 0_u64;
                let mut nodes = guard_nodes;
                // The assignment list is sized before it is walked: charge its length
                // before allocating for it.
                let mut resolved: Vec<(usize, &'m IntExpr)> =
                    meter.vec(outcome.assignments().len(), 1)?;
                for assignment in outcome.assignments() {
                    let variable = index.resolve(assignment.variable().as_str(), meter)?;
                    writes |= bit(variable);
                    nodes = nodes.saturating_add(walk_int(
                        assignment.value(),
                        &mut index,
                        &mut reads,
                        meter,
                    )?);
                    resolved.push((variable, assignment.value()));
                }
                labels.push(Label {
                    action: action_index,
                    outcome: outcome_index,
                });
                prints.push(Footprint {
                    reads: reads | writes,
                    writes,
                });
                // One evaluation also checks the state and builds the successor, each a
                // pass over the state vector.
                cost.push(
                    nodes
                        .saturating_add(1)
                        .saturating_add(index.arity().saturating_mul(2)),
                );
                updates.push(resolved);
            }
        }

        // Definedness (RFC 0003): model-core's classification, used as is. Its cost is
        // name work — each predicate and action name, compared by bytes along ordered
        // searches, once per chain level — paid before it runs.
        let names = model
            .predicates()
            .len()
            .saturating_add(model.actions().len());
        let paid = meter.pay(u64::try_from(names).unwrap_or(u64::MAX))?;
        meter.did(paid);
        let levels =
            u64::try_from(crate::budget::bits(names).saturating_add(1)).unwrap_or(u64::MAX);
        let name_work = model
            .predicates()
            .iter()
            .map(|predicate| predicate.name().as_str().len())
            .chain(
                model
                    .actions()
                    .iter()
                    .map(|action| action.name().as_str().len()),
            )
            .map(|len| {
                let len = u64::try_from(len).unwrap_or(u64::MAX);
                len.saturating_add(1)
                    .saturating_mul((len / 8).saturating_add(2))
                    .saturating_mul(levels)
                    // Two ordered searches, a scan for `(`, an insert, and the member
                    // sort per level: a constant factor over one search. The charge is
                    // an asymptotic bound on `Definedness::of`, not an exact count.
                    .saturating_mul(4)
            })
            .fold(0_u64, u64::saturating_add);
        let paid = meter.pay(name_work)?;
        meter.did(paid);
        let definedness = Definedness::of(model);

        // The predicates the check reads at a stored state: every action's
        // definedness chain, and for each invariant its own chain and itself. All of
        // them are observed: their reads are visible, so no reduced expansion hides a
        // step that changes one (a reduction that could postpone such a step could
        // skip the interleaving that reaches an undefined read).
        let mut observed: BTreeSet<usize> = BTreeSet::new();
        for chain in definedness.action_chains() {
            // Each member is inserted into the ordered observed set.
            let paid = meter.pay(
                u64::try_from(chain.len())
                    .unwrap_or(u64::MAX)
                    .saturating_mul(levels),
            )?;
            meter.did(paid);
            observed.extend(chain.iter().copied());
        }
        for predicate in invariants {
            let chain = definedness.guards_of(*predicate);
            let paid = meter.pay(
                u64::try_from(chain.len())
                    .unwrap_or(u64::MAX)
                    .saturating_add(1),
            )?;
            meter.did(paid);
            observed.extend(chain.iter().copied());
            observed.insert(*predicate);
        }
        let mut visible = 0_u64;
        let mut per_predicate: BTreeMap<usize, u64> = BTreeMap::new();
        for predicate in &observed {
            if let Some(declared) = model.predicates().get(*predicate) {
                // `observe_definedness` is `true` in every shipped build; the C005
                // mutation campaign turns it off to seed a hidden-definedness fault.
                let mut reads = 0_u64;
                let nodes = walk_bool(declared.body(), &mut index, &mut reads, meter)?;
                if observe_definedness || invariants.contains(predicate) {
                    visible |= reads;
                }
                // One evaluation: the body's nodes and the state check.
                per_predicate.insert(*predicate, nodes.saturating_add(index.arity()));
            }
        }
        // The cost of judging one state: every action chain once, then per invariant
        // its own chain and itself (a predicate read on two paths is paid twice).
        let cost_of = |predicate: &usize| per_predicate.get(predicate).copied().unwrap_or(0);
        let mut predicate_cost = definedness
            .action_chains()
            .flat_map(<[usize]>::iter)
            .map(cost_of)
            .fold(0_u64, u64::saturating_add);
        for predicate in invariants {
            predicate_cost = definedness
                .guards_of(*predicate)
                .iter()
                .chain(core::iter::once(predicate))
                .map(cost_of)
                .fold(predicate_cost, u64::saturating_add);
        }

        let width = labels.len();
        let words = u64::try_from(width.div_ceil(64)).unwrap_or(u64::MAX);
        let pairs = u64::try_from(width)
            .unwrap_or(u64::MAX)
            .saturating_mul(u64::try_from(width).unwrap_or(u64::MAX));
        // Two relations: the pair tests, plus every set's words.
        let paid = meter.pay(
            pairs
                .saturating_add(words.saturating_mul(u64::try_from(width).unwrap_or(u64::MAX)))
                .saturating_mul(2),
        )?;
        meter.did(paid);
        let mut dependents: Vec<LabelSet> = Vec::with_capacity(width);
        let mut enablers: Vec<LabelSet> = Vec::with_capacity(width);
        for (label, print) in prints.iter().enumerate() {
            let mut dependent = LabelSet::empty(width);
            let mut enabling = LabelSet::empty(width);
            for (other, theirs) in prints.iter().enumerate() {
                if other != label && conflicts(*print, *theirs, write_read_conflicts) {
                    dependent.insert(other);
                }
                if theirs.writes & print.reads != 0 {
                    enabling.insert(other);
                }
            }
            dependents.push(dependent);
            enablers.push(enabling);
        }

        Ok(Self {
            labels,
            prints,
            cost,
            updates,
            visible,
            predicate_cost,
            definedness,
            dependents,
            enablers,
        })
    }

    /// How many labels the model has.
    pub(crate) fn width(&self) -> usize {
        self.labels.len()
    }

    /// The label table, in `(action, outcome)` order.
    pub(crate) fn labels(&self) -> &[Label] {
        &self.labels
    }

    /// One label.
    pub(crate) fn label(&self, label: usize) -> Option<Label> {
        self.labels.get(label).copied()
    }

    /// One label's footprint.
    pub(crate) fn print(&self, label: usize) -> Footprint {
        self.prints.get(label).copied().unwrap_or(Footprint {
            reads: u64::MAX,
            writes: u64::MAX,
        })
    }

    /// The cost of evaluating one label once.
    pub(crate) fn cost(&self, label: usize) -> u64 {
        self.cost.get(label).copied().unwrap_or(u64::MAX)
    }

    /// The cost of evaluating every label once.
    pub(crate) fn total_cost(&self) -> u64 {
        self.cost
            .iter()
            .fold(0_u64, |sum, one| sum.saturating_add(*one))
    }

    /// One label's resolved assignments.
    pub(crate) fn updates(&self, label: usize) -> &[(usize, &'m IntExpr)] {
        self.updates.get(label).map_or(&[], Vec::as_slice)
    }

    /// The model's definedness classification.
    pub(crate) const fn definedness(&self) -> &Definedness {
        &self.definedness
    }

    /// The cost of evaluating every observed predicate once.
    pub(crate) const fn predicate_cost(&self) -> u64 {
        self.predicate_cost
    }

    /// The visible variables.
    pub(crate) const fn visible(&self) -> u64 {
        self.visible
    }

    /// Whether `label` writes a visible variable.
    pub(crate) fn is_visible(&self, label: usize) -> bool {
        self.print(label).writes & self.visible != 0
    }

    /// Every label dependent with `label`, itself excluded. A label outside the table
    /// has no entry; the empty fallback is never reached by the search, which only
    /// holds labels from the table.
    pub(crate) fn dependents(&self, label: usize) -> Option<&LabelSet> {
        self.dependents.get(label)
    }

    /// Every label that writes a variable `label` reads.
    pub(crate) fn enablers(&self, label: usize) -> Option<&LabelSet> {
        self.enablers.get(label)
    }

    /// RFC 0004's `dependence` question for two labels.
    #[must_use]
    pub(crate) fn dependence(&self, first: usize, second: usize) -> DependenceEvidence {
        if first == second {
            return DependenceEvidence::DefinitelyDependent(Conflict::SameLabel);
        }
        let (Some(a), Some(b)) = (self.prints.get(first), self.prints.get(second)) else {
            return DependenceEvidence::Unknown;
        };
        if let Some(variable) = lowest(a.writes & b.reads) {
            return DependenceEvidence::DefinitelyDependent(Conflict::Writes {
                writer: first,
                variable,
            });
        }
        if let Some(variable) = lowest(b.writes & a.reads) {
            return DependenceEvidence::DefinitelyDependent(Conflict::Writes {
                writer: second,
                variable,
            });
        }
        DependenceEvidence::DefinitelyIndependent {
            first: *a,
            second: *b,
        }
    }
}

/// Whether two distinct labels conflict. `write_read_conflicts` is `true` in every
/// shipped build; the C005 mutation campaign turns it off to seed an
/// independence-too-coarse fault (write/write overlap only).
const fn conflicts(a: Footprint, b: Footprint, write_read_conflicts: bool) -> bool {
    if write_read_conflicts {
        a.writes & b.reads != 0 || b.writes & a.reads != 0
    } else {
        a.writes & b.writes != 0
    }
}

fn lowest(mask: u64) -> Option<usize> {
    (mask != 0).then(|| usize::try_from(mask.trailing_zeros()).unwrap_or(0))
}

fn bit(variable: usize) -> u64 {
    u32::try_from(variable)
        .ok()
        .and_then(|shift| 1_u64.checked_shl(shift))
        .unwrap_or(u64::MAX)
}

/// Variable-name resolution. Each lookup is a scan of at most 64 declared variables,
/// charged before it is made.
struct Resolver<'m> {
    model: &'m Model,
}

impl<'m> Resolver<'m> {
    const fn new(model: &'m Model) -> Self {
        Self { model }
    }

    /// The model's arity, as work units.
    fn arity(&self) -> u64 {
        u64::try_from(self.model.arity()).unwrap_or(u64::MAX)
    }

    /// The canonical index of a variable. An unknown name cannot occur in a built
    /// model (`ModelBuilder::build` rejects it); if it did, every variable is read,
    /// which is conservative.
    fn resolve(&mut self, name: &str, meter: &mut Meter) -> Result<usize, Bound> {
        let paid = meter.pay(u64::try_from(self.model.arity()).unwrap_or(u64::MAX))?;
        meter.did(paid);
        Ok(self.model.variable_index(name).unwrap_or(usize::MAX))
    }

    fn mask(&mut self, name: &str, meter: &mut Meter) -> Result<u64, Bound> {
        let index = self.resolve(name, meter)?;
        Ok(if index == usize::MAX {
            u64::MAX
        } else {
            bit(index)
        })
    }
}

/// Collect the variables a Boolean expression reads into `reads`; return its node
/// count. Iterative, one unit charged per node before it is visited.
fn walk_bool(
    expr: &BoolExpr,
    index: &mut Resolver<'_>,
    reads: &mut u64,
    meter: &mut Meter,
) -> Result<u64, Bound> {
    let mut nodes = 0_u64;
    let mut stack: Vec<&BoolExpr> = vec![expr];
    while let Some(node) = stack.pop() {
        let paid = meter.pay(1)?;
        meter.did(paid);
        nodes = nodes.saturating_add(1);
        match node {
            BoolExpr::Const(_) => {}
            BoolExpr::Compare { left, right, .. } => {
                nodes = nodes.saturating_add(walk_int(left, index, reads, meter)?);
                nodes = nodes.saturating_add(walk_int(right, index, reads, meter)?);
            }
            BoolExpr::InRange { expr, .. } => {
                nodes = nodes.saturating_add(walk_int(expr, index, reads, meter)?);
            }
            BoolExpr::Not(inner) => stack.push(inner),
            BoolExpr::And(left, right)
            | BoolExpr::Or(left, right)
            | BoolExpr::Implies(left, right) => {
                stack.push(right);
                stack.push(left);
            }
        }
    }
    Ok(nodes)
}

/// Collect the variables an integer expression reads; return its node count.
fn walk_int(
    expr: &IntExpr,
    index: &mut Resolver<'_>,
    reads: &mut u64,
    meter: &mut Meter,
) -> Result<u64, Bound> {
    let mut nodes = 0_u64;
    let mut stack: Vec<&IntExpr> = vec![expr];
    while let Some(node) = stack.pop() {
        let paid = meter.pay(1)?;
        meter.did(paid);
        nodes = nodes.saturating_add(1);
        match node {
            IntExpr::Const(_) => {}
            IntExpr::Var(name) => {
                // An evaluation looks a variable up by name, a scan of the declared
                // variables: the node costs that scan, not one unit.
                nodes = nodes.saturating_add(index.arity());
                *reads |= index.mask(name, meter)?;
            }
            IntExpr::Arith(_, left, right)
            | IntExpr::Min(left, right)
            | IntExpr::Max(left, right) => {
                stack.push(right);
                stack.push(left);
            }
        }
    }
    Ok(nodes)
}
