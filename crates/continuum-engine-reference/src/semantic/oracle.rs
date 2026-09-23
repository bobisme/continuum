//! The tiny exhaustive oracle: every reachable configuration, every transition, every
//! bounded interleaving, and a canonical artifact that says what they are.
//!
//! The semantics this module implements is stated in full in the parent module's
//! documentation ([`crate::semantic`]); this file is the implementation, and each check
//! below names the clause it implements.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::bfs::{self, Bound, Bounds, Exploration, ExplorationError, MAX_DEPTH};
use crate::model::{EvaluationError, Model, State};
use crate::witness::{self, Target};

use super::system::{Fairness, Footprint, System, encode_vector};

// ---------------------------------------------------------------------------
// limits and refusals
// ---------------------------------------------------------------------------

/// The size limits the oracle enforces. Above any of them it refuses; it never returns
/// a partial answer dressed as a complete one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Limits {
    /// Most state variables accepted.
    pub max_variables: usize,
    /// Most actions accepted.
    pub max_actions: usize,
    /// Most configurations: the model's *declared* state space (the product of its
    /// domains) must fit, so an accepted model is always explored to closure.
    pub max_states: usize,
    /// Interleaving depth: every execution prefix up to this many steps is enumerated.
    pub max_depth: usize,
    /// Most interleavings enumerated.
    pub max_interleavings: u64,
}

impl Limits {
    /// The default tiny limits.
    pub const TINY: Self = Self {
        max_variables: 16,
        max_actions: 32,
        max_states: 4096,
        max_depth: 6,
        max_interleavings: 1 << 18,
    };
}

/// A quantity a [`Limits`] field bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Measure {
    /// [`Limits::max_variables`].
    Variables,
    /// [`Limits::max_actions`].
    Actions,
    /// [`Limits::max_states`], against the declared state space.
    StateSpace,
    /// The transition budget exploration derives from [`Limits`].
    Transitions,
    /// [`Limits::max_interleavings`].
    Interleavings,
}

impl fmt::Display for Measure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match *self {
            Self::Variables => "variables",
            Self::Actions => "actions",
            Self::StateSpace => "state-space",
            Self::Transitions => "transitions",
            Self::Interleavings => "interleavings",
        })
    }
}

/// Why the oracle gave no artifact. Each arm is a distinct kind of inconclusiveness
/// (INV-008): too large is not unsupported, and neither is a model that fails to
/// evaluate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The instance exceeds a limit. `observed` is exact when `at_least` is false, and
    /// a lower bound when the count was abandoned at the limit.
    TooLarge {
        /// Which quantity.
        measure: Measure,
        /// The limit.
        limit: u128,
        /// What was observed.
        observed: u128,
        /// Whether `observed` is only a lower bound.
        at_least: bool,
    },
    /// An action with several outcomes. The interleaving and diamond semantics below
    /// are stated for deterministic actions, so the oracle refuses rather than guess.
    NondeterministicAction {
        /// The action.
        action: String,
    },
    /// Exploration failed on the model itself.
    Exploration(Box<ExplorationError>),
    /// Evaluation failed on a reachable state.
    Evaluation {
        /// The state.
        state: State,
        /// The action.
        action: String,
        /// Why.
        source: Box<EvaluationError>,
    },
    /// An internal consistency check failed: a transition left the closed reachable
    /// set, or a witness could not be read back. An oracle defect, reported as data.
    Inconsistent {
        /// What failed.
        detail: &'static str,
    },
}

fn too_large(measure: Measure, limit: u128, observed: u128, at_least: bool) -> Refusal {
    Refusal::TooLarge {
        measure,
        limit,
        observed,
        at_least,
    }
}

fn wide(value: usize) -> u128 {
    u128::try_from(value).unwrap_or(u128::MAX)
}

// ---------------------------------------------------------------------------
// findings
// ---------------------------------------------------------------------------

/// The four defect classes the oracle tells apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DefectClass {
    /// A declared footprint or independence claim the behaviour contradicts.
    Dependence,
    /// A fair execution that keeps an obligation open forever.
    Fairness,
    /// An obligation still open where the system has stopped or its owner finished.
    Obligation,
    /// A cancellation phase that goes backwards.
    Cancellation,
}

impl DefectClass {
    /// Every class, in canonical order.
    pub const ALL: [Self; 4] = [
        Self::Dependence,
        Self::Fairness,
        Self::Obligation,
        Self::Cancellation,
    ];

    /// The class's token in the canonical encoding.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Dependence => "dependence",
            Self::Fairness => "fairness",
            Self::Obligation => "obligation",
            Self::Cancellation => "cancellation",
        }
    }
}

/// A labelled path: a start state and the `(action, target)` steps taken from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trace {
    /// Where the path starts.
    pub start: State,
    /// The steps.
    pub steps: Vec<(String, State)>,
}

impl Trace {
    /// The state the path ends at.
    #[must_use]
    pub fn end(&self) -> &State {
        self.steps.last().map_or(&self.start, |(_, state)| state)
    }

    fn encode(&self) -> String {
        let mut out = encode_vector(self.start.as_slice());
        for (action, state) in &self.steps {
            out.push_str(&format!(" {action} {}", encode_vector(state.as_slice())));
        }
        out
    }
}

/// How a pair of co-enabled actions failed the diamond (docs/02 §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Breach {
    /// Taking the first disables the second.
    FirstDisablesSecond,
    /// Taking the second disables the first.
    SecondDisablesFirst,
    /// Both orders are defined and end in different states.
    DiamondOpen,
}

impl Breach {
    const fn token(self) -> &'static str {
        match self {
            Self::FirstDisablesSecond => "first-disables-second",
            Self::SecondDisablesFirst => "second-disables-first",
            Self::DiamondOpen => "diamond-open",
        }
    }
}

/// One defect the oracle found. Every witness is a shortest path to where the defect
/// shows, in the canonical order of [`crate::bfs`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    /// An action's syntax reads a variable its footprint does not declare.
    FootprintReadEscape {
        /// The action.
        action: String,
        /// The undeclared variable.
        variable: String,
    },
    /// A reachable transition changes a variable the action's footprint does not
    /// declare written. The witness's last step is that transition.
    FootprintWriteEscape {
        /// The action.
        action: String,
        /// The undeclared variable.
        variable: String,
        /// The path.
        witness: Trace,
    },
    /// Two actions claimed independent fail the diamond at a reachable state where both
    /// are enabled. The witness ends at that state.
    IndependenceViolated {
        /// The smaller action name.
        first: String,
        /// The larger action name.
        second: String,
        /// How.
        breach: Breach,
        /// The path.
        witness: Trace,
    },
    /// A quiescent reachable state (no action enabled) holds an open obligation.
    QuiescenceLeak {
        /// The obligation.
        obligation: String,
        /// The path.
        witness: Trace,
    },
    /// A reachable state where the obligation's owner is in its final phase and the
    /// obligation is still open.
    CompletionLeak {
        /// The obligation.
        obligation: String,
        /// The path.
        witness: Trace,
    },
    /// A weakly fair infinite execution along which the obligation is open forever: a
    /// stem to a state, then a cycle back to it, every cycle state holding the
    /// obligation open.
    OpenForever {
        /// The obligation.
        obligation: String,
        /// The stem.
        stem: Trace,
        /// The cycle, starting from the stem's end and returning to it.
        cycle: Vec<(String, State)>,
    },
    /// A reachable transition lowers a cancellation phase.
    PhaseRegression {
        /// The phase variable.
        variable: String,
        /// The action.
        action: String,
        /// The phase before.
        from: i64,
        /// The phase after.
        to: i64,
        /// The path; its last step is the regression.
        witness: Trace,
    },
}

impl Finding {
    /// The defect class the finding belongs to.
    #[must_use]
    pub const fn class(&self) -> DefectClass {
        match self {
            Self::FootprintReadEscape { .. }
            | Self::FootprintWriteEscape { .. }
            | Self::IndependenceViolated { .. } => DefectClass::Dependence,
            Self::OpenForever { .. } => DefectClass::Fairness,
            Self::QuiescenceLeak { .. } | Self::CompletionLeak { .. } => DefectClass::Obligation,
            Self::PhaseRegression { .. } => DefectClass::Cancellation,
        }
    }

    /// The finding's kind, as its canonical encoding spells it.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::FootprintReadEscape { .. } => "footprint-read-escape",
            Self::FootprintWriteEscape { .. } => "footprint-write-escape",
            Self::IndependenceViolated { .. } => "independence-violated",
            Self::QuiescenceLeak { .. } => "quiescence-leak",
            Self::CompletionLeak { .. } => "completion-leak",
            Self::OpenForever { .. } => "open-forever",
            Self::PhaseRegression { .. } => "phase-regression",
        }
    }

    fn encode(&self) -> String {
        let class = self.class().token();
        match self {
            Self::FootprintReadEscape { action, variable } => {
                format!("finding {class} footprint-read-escape {action} {variable}")
            }
            Self::FootprintWriteEscape {
                action,
                variable,
                witness,
            } => format!(
                "finding {class} footprint-write-escape {action} {variable} witness {}",
                witness.encode()
            ),
            Self::IndependenceViolated {
                first,
                second,
                breach,
                witness,
            } => format!(
                "finding {class} independence-violated {first} {second} {} witness {}",
                breach.token(),
                witness.encode()
            ),
            Self::QuiescenceLeak {
                obligation,
                witness,
            } => format!(
                "finding {class} quiescence-leak {obligation} witness {}",
                witness.encode()
            ),
            Self::CompletionLeak {
                obligation,
                witness,
            } => format!(
                "finding {class} completion-leak {obligation} witness {}",
                witness.encode()
            ),
            Self::OpenForever {
                obligation,
                stem,
                cycle,
            } => {
                let mut text = format!(
                    "finding {class} open-forever {obligation} stem {} cycle",
                    stem.encode()
                );
                for (action, state) in cycle {
                    text.push_str(&format!(" {action} {}", encode_vector(state.as_slice())));
                }
                text
            }
            Self::PhaseRegression {
                variable,
                action,
                from,
                to,
                witness,
            } => format!(
                "finding {class} phase-regression {variable} {action} {from} {to} witness {}",
                witness.encode()
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// the artifact
// ---------------------------------------------------------------------------

/// Bounded interleaving counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Interleavings {
    /// The depth bound they were enumerated to.
    pub depth: usize,
    /// Executions that reach a quiescent state within the bound.
    pub complete: u64,
    /// Executions of exactly `depth` steps that end with an action still enabled.
    pub truncated: u64,
    /// Distinct Mazurkiewicz classes among all of the above, under the semantic
    /// dependence relation, counted per initial state.
    pub classes: u64,
}

/// The oracle's answer, in a form whose canonical bytes are its identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact {
    limits: Limits,
    system: String,
    actions: Vec<String>,
    states: Vec<State>,
    depths: Vec<usize>,
    edges: Vec<(usize, usize, usize)>,
    quiescent: Vec<usize>,
    dependent: BTreeSet<(usize, usize)>,
    interleavings: Interleavings,
    findings: Vec<Finding>,
}

/// Whether the exhaustive check found anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Verdict {
    /// The whole reachable space was checked and no finding exists.
    Clean,
    /// At least one finding exists.
    Defective,
}

impl Artifact {
    /// Every reachable configuration, ascending.
    #[must_use]
    pub fn states(&self) -> &[State] {
        &self.states
    }

    /// Every reachable transition as `(source, action, target)` indices, ascending.
    #[must_use]
    pub fn edges(&self) -> &[(usize, usize, usize)] {
        &self.edges
    }

    /// The indices of the quiescent configurations.
    #[must_use]
    pub fn quiescent(&self) -> &[usize] {
        &self.quiescent
    }

    /// The semantic dependence relation, as `(smaller, larger)` action-name pairs.
    #[must_use]
    pub fn dependent_pairs(&self) -> Vec<(&str, &str)> {
        self.dependent
            .iter()
            .filter_map(|(a, b)| {
                Some((
                    self.actions.get(*a)?.as_str(),
                    self.actions.get(*b)?.as_str(),
                ))
            })
            .collect()
    }

    /// The bounded interleaving counts.
    #[must_use]
    pub const fn interleavings(&self) -> Interleavings {
        self.interleavings
    }

    /// The findings, in the order the checks run.
    #[must_use]
    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }

    /// Whether any finding is of `class`.
    #[must_use]
    pub fn has(&self, class: DefectClass) -> bool {
        self.findings.iter().any(|finding| finding.class() == class)
    }

    /// The verdict.
    #[must_use]
    pub fn verdict(&self) -> Verdict {
        if self.findings.is_empty() {
            Verdict::Clean
        } else {
            Verdict::Defective
        }
    }

    /// The canonical text encoding.
    #[must_use]
    pub fn encode(&self) -> String {
        let limits = &self.limits;
        let mut out = String::from("continuum-semantic-oracle v1\n");
        out.push_str(&format!(
            "limits variables={} actions={} states={} depth={} interleavings={}\n",
            limits.max_variables,
            limits.max_actions,
            limits.max_states,
            limits.max_depth,
            limits.max_interleavings
        ));
        out.push_str(&self.system);
        out.push_str(&format!("states {}\n", self.states.len()));
        for (index, (state, depth)) in self.states.iter().zip(self.depths.iter()).enumerate() {
            out.push_str(&format!(
                "state {index} depth {depth} {}\n",
                encode_vector(state.as_slice())
            ));
        }
        for (source, action, target) in &self.edges {
            let name = self.actions.get(*action).map_or("?", String::as_str);
            out.push_str(&format!("edge {source} {name} {target}\n"));
        }
        for index in &self.quiescent {
            out.push_str(&format!("quiescent {index}\n"));
        }
        for (first, second) in self.dependent_pairs() {
            out.push_str(&format!("dependent {first} {second}\n"));
        }
        let counts = self.interleavings;
        out.push_str(&format!(
            "interleavings depth={} complete={} truncated={} classes={}\n",
            counts.depth, counts.complete, counts.truncated, counts.classes
        ));
        for finding in &self.findings {
            out.push_str(&finding.encode());
            out.push('\n');
        }
        out.push_str(match self.verdict() {
            Verdict::Clean => "verdict clean\n",
            Verdict::Defective => "verdict defective\n",
        });
        out.push_str("end-oracle\n");
        out
    }

    /// The canonical bytes. Per ADR-0013 these bytes *are* the artifact's content
    /// identity: two artifacts are the same artifact iff their bytes are equal, and
    /// any digest a store indexes them by is a lookup key, never the identity.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        self.encode().into_bytes()
    }
}

// ---------------------------------------------------------------------------
// the run
// ---------------------------------------------------------------------------

/// The transition graph of a closed exploration, with deterministic actions.
struct Graph<'a> {
    model: &'a Model,
    exploration: Exploration,
    states: Vec<State>,
    depths: Vec<usize>,
    /// `post[s][a]`: the target of action `a` from state `s`, if enabled.
    post: Vec<Vec<Option<usize>>>,
    /// States by (depth, index): the order every "first" finding is chosen in.
    order: Vec<usize>,
}

impl Graph<'_> {
    fn post(&self, state: usize, action: usize) -> Option<usize> {
        self.post.get(state)?.get(action).copied().flatten()
    }

    fn state(&self, index: usize) -> Result<&State, Refusal> {
        self.states.get(index).ok_or(Refusal::Inconsistent {
            detail: "state index outside the reachable table",
        })
    }

    fn action_name(&self, action: usize) -> String {
        self.model
            .actions()
            .get(action)
            .map_or_else(String::new, |a| a.name().as_str().to_owned())
    }

    fn value(&self, state: usize, variable: usize) -> Option<i64> {
        self.states.get(state)?.as_slice().get(variable).copied()
    }

    fn trace_to(&self, state: usize) -> Result<Trace, Refusal> {
        let target = Target::State(self.state(state)?.clone());
        let path = witness::shortest(self.model, &self.exploration, &target).map_err(|_| {
            Refusal::Inconsistent {
                detail: "no shortest witness to a reachable state",
            }
        })?;
        Ok(Trace {
            start: path.start().clone(),
            steps: path
                .steps()
                .iter()
                .map(|step| (step.name().as_str().to_owned(), step.target().clone()))
                .collect(),
        })
    }

    fn trace_through(&self, state: usize, action: usize, target: usize) -> Result<Trace, Refusal> {
        let mut trace = self.trace_to(state)?;
        trace
            .steps
            .push((self.action_name(action), self.state(target)?.clone()));
        Ok(trace)
    }

    fn actions(&self) -> usize {
        self.model.actions().len()
    }
}

/// Run the oracle over `system` within `limits`.
///
/// # Errors
///
/// A [`Refusal`] when the instance is above a limit, uses unsupported semantics, or
/// fails to evaluate. A refusal is never a partial answer.
pub fn run(system: &System, limits: &Limits) -> Result<Artifact, Refusal> {
    let model = system.model();
    let variables = model.variables().len();
    if variables > limits.max_variables {
        return Err(too_large(
            Measure::Variables,
            wide(limits.max_variables),
            wide(variables),
            false,
        ));
    }
    let action_count = model.actions().len();
    if action_count > limits.max_actions {
        return Err(too_large(
            Measure::Actions,
            wide(limits.max_actions),
            wide(action_count),
            false,
        ));
    }
    let space = model.domain_cardinality();
    if space > wide(limits.max_states) {
        return Err(too_large(
            Measure::StateSpace,
            wide(limits.max_states),
            space,
            false,
        ));
    }
    if let Some(action) = model.actions().iter().find(|a| !a.is_deterministic()) {
        return Err(Refusal::NondeterministicAction {
            action: action.name().as_str().to_owned(),
        });
    }

    let graph = build_graph(model, limits)?;
    let mut findings: Vec<Finding> = Vec::new();
    check_footprints(system, &graph, &mut findings)?;
    let dependent = check_independence(system, &graph, &mut findings)?;
    let quiescent: Vec<usize> = (0..graph.states.len())
        .filter(|state| (0..graph.actions()).all(|action| graph.post(*state, action).is_none()))
        .collect();
    check_obligations(system, &graph, &quiescent, &mut findings)?;
    check_phases(system, &graph, &mut findings)?;
    let interleavings = enumerate_interleavings(&graph, &dependent, limits)?;

    let mut edges: Vec<(usize, usize, usize)> = Vec::new();
    for source in 0..graph.states.len() {
        for action in 0..graph.actions() {
            if let Some(target) = graph.post(source, action) {
                edges.push((source, action, target));
            }
        }
    }
    Ok(Artifact {
        limits: *limits,
        system: system.encode(),
        actions: (0..graph.actions()).map(|a| graph.action_name(a)).collect(),
        states: graph.states.clone(),
        depths: graph.depths.clone(),
        edges,
        quiescent,
        dependent,
        interleavings,
        findings,
    })
}

fn build_graph<'a>(model: &'a Model, limits: &Limits) -> Result<Graph<'a>, Refusal> {
    let transitions = u64::try_from(limits.max_states)
        .unwrap_or(u64::MAX)
        .saturating_mul(u64::try_from(limits.max_actions).unwrap_or(u64::MAX))
        .max(1);
    let bounds = Bounds::new(limits.max_states, MAX_DEPTH, transitions);
    let exploration =
        bfs::explore(model, bounds).map_err(|error| Refusal::Exploration(Box::new(error)))?;
    let reachable = match &exploration {
        Exploration::Complete(reachable) => reachable,
        Exploration::Exhausted(partial) => {
            let measure = match partial.tripped() {
                Bound::Transitions => Measure::Transitions,
                _ => Measure::StateSpace,
            };
            return Err(too_large(
                measure,
                wide(limits.max_states),
                wide(partial.explored().len()),
                true,
            ));
        }
    };
    let states = reachable.states().to_vec();
    let depths = reachable.depths().to_vec();
    let index: BTreeMap<&State, usize> = states.iter().enumerate().map(|(i, s)| (s, i)).collect();
    let mut post: Vec<Vec<Option<usize>>> = Vec::with_capacity(states.len());
    for state in &states {
        let mut row: Vec<Option<usize>> = Vec::with_capacity(model.actions().len());
        for (action, declared) in model.actions().iter().enumerate() {
            let targets =
                model
                    .action_successors(action, state)
                    .map_err(|source| Refusal::Evaluation {
                        state: state.clone(),
                        action: declared.name().as_str().to_owned(),
                        source: Box::new(source),
                    })?;
            match targets.first() {
                None => row.push(None),
                Some(target) => {
                    let target = index.get(target).copied().ok_or(Refusal::Inconsistent {
                        detail: "a transition leaves the closed reachable set",
                    })?;
                    row.push(Some(target));
                }
            }
        }
        post.push(row);
    }
    let mut order: Vec<usize> = (0..states.len()).collect();
    order.sort_by_key(|state| (depths.get(*state).copied().unwrap_or(usize::MAX), *state));
    Ok(Graph {
        model,
        exploration,
        states,
        depths,
        post,
        order,
    })
}

/// Footprint soundness: syntactic reads within the declared reads (static), and every
/// reachable transition changing only declared writes (behavioural).
fn check_footprints(
    system: &System,
    graph: &Graph<'_>,
    findings: &mut Vec<Finding>,
) -> Result<(), Refusal> {
    let model = graph.model;
    for action in model.actions() {
        let name = action.name().as_str();
        let declared = system.meta().get(name).map(|meta| &meta.footprint);
        for variable in Footprint::syntactic(action).reads() {
            if !declared.is_some_and(|footprint| footprint.reads().contains(variable)) {
                findings.push(Finding::FootprintReadEscape {
                    action: name.to_owned(),
                    variable: variable.clone(),
                });
            }
        }
    }
    let mut reported: BTreeSet<(usize, usize)> = BTreeSet::new();
    for &source in &graph.order {
        for action in 0..graph.actions() {
            let Some(target) = graph.post(source, action) else {
                continue;
            };
            let name = graph.action_name(action);
            let writes = system.meta().get(&name).map(|meta| meta.footprint.writes());
            for (position, variable) in model.variables().iter().enumerate() {
                if graph.value(source, position) == graph.value(target, position) {
                    continue;
                }
                let variable = variable.name().as_str();
                if writes.is_some_and(|writes| writes.contains(variable)) {
                    continue;
                }
                if reported.insert((action, position)) {
                    findings.push(Finding::FootprintWriteEscape {
                        action: name.clone(),
                        variable: variable.to_owned(),
                        witness: graph.trace_through(source, action, target)?,
                    });
                }
            }
        }
    }
    Ok(())
}

/// The diamond at every reachable state for every co-enabled pair. Returns the
/// semantic dependence relation and reports every claimed-independent pair that fails.
fn check_independence(
    system: &System,
    graph: &Graph<'_>,
    findings: &mut Vec<Finding>,
) -> Result<BTreeSet<(usize, usize)>, Refusal> {
    let mut dependent: BTreeSet<(usize, usize)> = BTreeSet::new();
    let mut reported: BTreeSet<(usize, usize)> = BTreeSet::new();
    for &state in &graph.order {
        for first in 0..graph.actions() {
            let Some(after_first) = graph.post(state, first) else {
                continue;
            };
            for second in first.saturating_add(1)..graph.actions() {
                let Some(after_second) = graph.post(state, second) else {
                    continue;
                };
                let breach = match (
                    graph.post(after_first, second),
                    graph.post(after_second, first),
                ) {
                    (None, _) => Some(Breach::FirstDisablesSecond),
                    (_, None) => Some(Breach::SecondDisablesFirst),
                    (Some(left), Some(right)) if left != right => Some(Breach::DiamondOpen),
                    _ => None,
                };
                let Some(breach) = breach else {
                    continue;
                };
                dependent.insert((first, second));
                let (first_name, second_name) =
                    (graph.action_name(first), graph.action_name(second));
                if system.claims_independent(&first_name, &second_name)
                    && reported.insert((first, second))
                {
                    findings.push(Finding::IndependenceViolated {
                        first: first_name,
                        second: second_name,
                        breach,
                        witness: graph.trace_to(state)?,
                    });
                }
            }
        }
    }
    Ok(dependent)
}

/// Quiescence leaks, completion leaks, and weakly fair cycles that keep an obligation
/// open.
fn check_obligations(
    system: &System,
    graph: &Graph<'_>,
    quiescent: &[usize],
    findings: &mut Vec<Finding>,
) -> Result<(), Refusal> {
    let model = graph.model;
    let quiescent: BTreeSet<usize> = quiescent.iter().copied().collect();
    let weak: Vec<usize> = model
        .actions()
        .iter()
        .enumerate()
        .filter(|(_, action)| {
            system
                .meta()
                .get(action.name().as_str())
                .is_some_and(|meta| meta.fairness == Fairness::Weak)
        })
        .map(|(index, _)| index)
        .collect();
    for obligation in system.obligations().values() {
        let Some(flag) = model.variable_index(&obligation.variable) else {
            continue;
        };
        let open = |state: usize| graph.value(state, flag).is_some_and(|value| value != 0);
        if let Some(&state) = graph
            .order
            .iter()
            .find(|state| quiescent.contains(state) && open(**state))
        {
            findings.push(Finding::QuiescenceLeak {
                obligation: obligation.name.clone(),
                witness: graph.trace_to(state)?,
            });
        }
        let owner = obligation.owner_phase.as_ref().and_then(|phase| {
            let final_value = system.phases().get(phase)?.final_value;
            Some((model.variable_index(phase)?, final_value))
        });
        if let Some((phase, final_value)) = owner
            && let Some(&state) = graph
                .order
                .iter()
                .find(|state| open(**state) && graph.value(**state, phase) == Some(final_value))
        {
            findings.push(Finding::CompletionLeak {
                obligation: obligation.name.clone(),
                witness: graph.trace_to(state)?,
            });
        }
        let region: BTreeSet<usize> = (0..graph.states.len()).filter(|s| open(*s)).collect();
        if let Some((stem, cycle)) = fair_lasso(graph, &region, &weak)? {
            findings.push(Finding::OpenForever {
                obligation: obligation.name.clone(),
                stem,
                cycle,
            });
        }
    }
    Ok(())
}

/// Monotonicity of every declared cancellation phase along every reachable transition.
fn check_phases(
    system: &System,
    graph: &Graph<'_>,
    findings: &mut Vec<Finding>,
) -> Result<(), Refusal> {
    let mut reported: BTreeSet<(usize, usize)> = BTreeSet::new();
    let phases: Vec<(usize, &str)> = system
        .phases()
        .keys()
        .filter_map(|name| Some((graph.model.variable_index(name)?, name.as_str())))
        .collect();
    for &source in &graph.order {
        for action in 0..graph.actions() {
            let Some(target) = graph.post(source, action) else {
                continue;
            };
            for &(position, name) in &phases {
                let (Some(from), Some(to)) =
                    (graph.value(source, position), graph.value(target, position))
                else {
                    continue;
                };
                if to < from && reported.insert((action, position)) {
                    findings.push(Finding::PhaseRegression {
                        variable: name.to_owned(),
                        action: graph.action_name(action),
                        from,
                        to,
                        witness: graph.trace_through(source, action, target)?,
                    });
                }
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// fair lassos
// ---------------------------------------------------------------------------

/// The strongly connected components of the subgraph induced by `region`, by an
/// iterative Tarjan walk (no recursion, so no stack depth tied to model size).
fn components(graph: &Graph<'_>, region: &BTreeSet<usize>) -> Vec<BTreeSet<usize>> {
    let successors = |state: usize| -> Vec<usize> {
        (0..graph.actions())
            .filter_map(|action| graph.post(state, action))
            .filter(|target| region.contains(target))
            .collect()
    };
    let mut index: BTreeMap<usize, usize> = BTreeMap::new();
    let mut low: BTreeMap<usize, usize> = BTreeMap::new();
    let mut on_stack: BTreeSet<usize> = BTreeSet::new();
    let mut stack: Vec<usize> = Vec::new();
    let mut counter = 0_usize;
    let mut out: Vec<BTreeSet<usize>> = Vec::new();
    for &root in region {
        if index.contains_key(&root) {
            continue;
        }
        let mut calls: Vec<(usize, Vec<usize>, usize)> = Vec::new();
        index.insert(root, counter);
        low.insert(root, counter);
        counter = counter.saturating_add(1);
        stack.push(root);
        on_stack.insert(root);
        calls.push((root, successors(root), 0));
        while let Some((node, children, position)) = calls.pop() {
            if let Some(&child) = children.get(position) {
                calls.push((node, children, position.saturating_add(1)));
                if let Some(&child_index) = index.get(&child) {
                    if on_stack.contains(&child) {
                        let current = low.get(&node).copied().unwrap_or(usize::MAX);
                        low.insert(node, current.min(child_index));
                    }
                } else {
                    index.insert(child, counter);
                    low.insert(child, counter);
                    counter = counter.saturating_add(1);
                    stack.push(child);
                    on_stack.insert(child);
                    calls.push((child, successors(child), 0));
                }
                continue;
            }
            let node_low = low.get(&node).copied().unwrap_or(usize::MAX);
            if Some(&node_low) == index.get(&node) {
                let mut component = BTreeSet::new();
                while let Some(member) = stack.pop() {
                    on_stack.remove(&member);
                    component.insert(member);
                    if member == node {
                        break;
                    }
                }
                out.push(component);
            }
            if let Some((parent, _, _)) = calls.last() {
                let parent_low = low.get(parent).copied().unwrap_or(usize::MAX);
                low.insert(*parent, parent_low.min(node_low));
            }
        }
    }
    out
}

/// A stem and a cycle from its end back to it.
type Lasso = (Trace, Vec<(String, State)>);

/// A weakly fair lasso whose cycle stays inside `region`, or `None`.
///
/// An SCC `C` of the region subgraph with at least one internal edge carries a weakly
/// fair cycle iff every weakly fair action is, somewhere in `C`, either disabled or
/// taken along an internal edge: the cycle that tours all of `C` is then fair, and if
/// some weakly fair action is enabled at every state of `C` and never taken inside it,
/// no cycle in `C` is fair. The SCC chosen is the one whose earliest state (in
/// `(depth, index)` order) comes first.
fn fair_lasso(
    graph: &Graph<'_>,
    region: &BTreeSet<usize>,
    weak: &[usize],
) -> Result<Option<Lasso>, Refusal> {
    let rank: BTreeMap<usize, usize> = graph
        .order
        .iter()
        .enumerate()
        .map(|(rank, state)| (*state, rank))
        .collect();
    let mut best: Option<(usize, BTreeSet<usize>)> = None;
    for component in components(graph, region) {
        let internal = |state: usize, action: usize| {
            graph
                .post(state, action)
                .filter(|target| component.contains(target))
        };
        let has_edge = component
            .iter()
            .any(|state| (0..graph.actions()).any(|action| internal(*state, action).is_some()));
        if !has_edge {
            continue;
        }
        let fair = weak.iter().all(|action| {
            component.iter().any(|state| {
                graph.post(*state, *action).is_none() || internal(*state, *action).is_some()
            })
        });
        if !fair {
            continue;
        }
        let Some(entry) = component
            .iter()
            .min_by_key(|state| rank.get(state).copied().unwrap_or(usize::MAX))
            .copied()
        else {
            continue;
        };
        let entry_rank = rank.get(&entry).copied().unwrap_or(usize::MAX);
        if best.as_ref().is_none_or(|(rank, _)| entry_rank < *rank) {
            best = Some((entry_rank, component));
        }
    }
    let Some((_, component)) = best else {
        return Ok(None);
    };
    let entry = component
        .iter()
        .min_by_key(|state| rank.get(state).copied().unwrap_or(usize::MAX))
        .copied()
        .ok_or(Refusal::Inconsistent {
            detail: "empty component",
        })?;
    let stem = graph.trace_to(entry)?;

    // Tour: one internal edge out of the entry, then each weakly fair action's
    // discharge site (a state where it is disabled, else an internal edge labelled
    // with it), then back to the entry.
    let mut steps: Vec<(usize, usize)> = Vec::new();
    let first = (0..graph.actions())
        .find_map(|action| {
            graph
                .post(entry, action)
                .filter(|target| component.contains(target))
                .map(|target| (action, target))
        })
        .ok_or(Refusal::Inconsistent {
            detail: "an SCC state without an internal edge",
        })?;
    steps.push(first);
    let mut current = first.1;
    for &action in weak {
        let disabled = component
            .iter()
            .filter(|state| graph.post(**state, action).is_none())
            .min_by_key(|state| rank.get(state).copied().unwrap_or(usize::MAX))
            .copied();
        if let Some(site) = disabled {
            steps.extend(path_within(graph, &component, current, site)?);
            current = site;
        } else {
            let taken = component
                .iter()
                .filter_map(|state| {
                    graph
                        .post(*state, action)
                        .filter(|target| component.contains(target))
                        .map(|target| (*state, target))
                })
                .min_by_key(|(state, _)| rank.get(state).copied().unwrap_or(usize::MAX))
                .ok_or(Refusal::Inconsistent {
                    detail: "a fair component without its fairness site",
                })?;
            steps.extend(path_within(graph, &component, current, taken.0)?);
            steps.push((action, taken.1));
            current = taken.1;
        }
    }
    steps.extend(path_within(graph, &component, current, entry)?);
    let mut cycle: Vec<(String, State)> = Vec::with_capacity(steps.len());
    for (action, target) in steps {
        cycle.push((graph.action_name(action), graph.state(target)?.clone()));
    }
    Ok(Some((stem, cycle)))
}

/// A shortest path from `from` to `to` using only edges inside `component`, as
/// `(action, target)` steps; empty when `from == to`.
fn path_within(
    graph: &Graph<'_>,
    component: &BTreeSet<usize>,
    from: usize,
    to: usize,
) -> Result<Vec<(usize, usize)>, Refusal> {
    let mut parent: BTreeMap<usize, (usize, usize)> = BTreeMap::new();
    let mut seen: BTreeSet<usize> = BTreeSet::from([from]);
    let mut queue: VecDeque<usize> = VecDeque::from([from]);
    while let Some(state) = queue.pop_front() {
        if state == to {
            break;
        }
        for action in 0..graph.actions() {
            if let Some(target) = graph.post(state, action)
                && component.contains(&target)
                && seen.insert(target)
            {
                parent.insert(target, (state, action));
                queue.push_back(target);
            }
        }
    }
    let mut path: Vec<(usize, usize)> = Vec::new();
    let mut cursor = to;
    while cursor != from {
        let (previous, action) = parent.get(&cursor).copied().ok_or(Refusal::Inconsistent {
            detail: "no path inside a strongly connected component",
        })?;
        path.push((action, cursor));
        cursor = previous;
    }
    path.reverse();
    Ok(path)
}

// ---------------------------------------------------------------------------
// interleavings
// ---------------------------------------------------------------------------

/// The Foata normal form of `word` under `dependent` (plus identity): each letter's
/// level is one more than the highest level of an earlier letter it depends on, and
/// each level is sorted. Two words are Mazurkiewicz-equivalent iff their Foata forms
/// are equal (Cartier–Foata).
fn foata(word: &[usize], dependent: &BTreeSet<(usize, usize)>) -> Vec<Vec<usize>> {
    let depends = |a: usize, b: usize| a == b || dependent.contains(&(a.min(b), a.max(b)));
    let mut levels: Vec<usize> = Vec::with_capacity(word.len());
    let mut form: Vec<Vec<usize>> = Vec::new();
    for (position, &letter) in word.iter().enumerate() {
        let level = word
            .iter()
            .zip(levels.iter())
            .take(position)
            .filter(|(earlier, _)| depends(**earlier, letter))
            .map(|(_, level)| level.saturating_add(1))
            .max()
            .unwrap_or(0);
        levels.push(level);
        if form.len() <= level {
            form.resize(level.saturating_add(1), Vec::new());
        }
        if let Some(step) = form.get_mut(level) {
            step.push(letter);
        }
    }
    for step in &mut form {
        step.sort_unstable();
    }
    form
}

struct Walk<'g, 'm> {
    graph: &'g Graph<'m>,
    dependent: &'g BTreeSet<(usize, usize)>,
    depth: usize,
    budget: u64,
    complete: u64,
    truncated: u64,
    classes: BTreeSet<(usize, Vec<Vec<usize>>)>,
    word: Vec<usize>,
}

impl Walk<'_, '_> {
    fn record(&mut self, initial: usize, complete: bool) -> Result<(), Refusal> {
        let total = self.complete.saturating_add(self.truncated);
        if total >= self.budget {
            return Err(too_large(
                Measure::Interleavings,
                u128::from(self.budget),
                u128::from(total.saturating_add(1)),
                true,
            ));
        }
        if complete {
            self.complete = self.complete.saturating_add(1);
        } else {
            self.truncated = self.truncated.saturating_add(1);
        }
        self.classes
            .insert((initial, foata(&self.word, self.dependent)));
        Ok(())
    }

    /// Depth-first over every execution prefix; recursion depth is the interleaving
    /// bound, not the model size.
    fn visit(&mut self, initial: usize, state: usize) -> Result<(), Refusal> {
        let enabled: Vec<(usize, usize)> = (0..self.graph.actions())
            .filter_map(|action| Some((action, self.graph.post(state, action)?)))
            .collect();
        if enabled.is_empty() {
            return self.record(initial, true);
        }
        if self.word.len() >= self.depth {
            return self.record(initial, false);
        }
        for (action, target) in enabled {
            self.word.push(action);
            self.visit(initial, target)?;
            self.word.pop();
        }
        Ok(())
    }
}

fn enumerate_interleavings(
    graph: &Graph<'_>,
    dependent: &BTreeSet<(usize, usize)>,
    limits: &Limits,
) -> Result<Interleavings, Refusal> {
    let mut walk = Walk {
        graph,
        dependent,
        depth: limits.max_depth,
        budget: limits.max_interleavings,
        complete: 0,
        truncated: 0,
        classes: BTreeSet::new(),
        word: Vec::new(),
    };
    for (initial, state) in graph.model.initial_states().iter().enumerate() {
        let index = graph
            .states
            .binary_search(state)
            .map_err(|_| Refusal::Inconsistent {
                detail: "an initial state missing from the reachable table",
            })?;
        walk.visit(initial, index)?;
    }
    Ok(Interleavings {
        depth: limits.max_depth,
        complete: walk.complete,
        truncated: walk.truncated,
        classes: u64::try_from(walk.classes.len()).unwrap_or(u64::MAX),
    })
}
