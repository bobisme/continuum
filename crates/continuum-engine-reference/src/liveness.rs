//! Liveness under the model's fairness assumptions: a fair-cycle search over a closed
//! exploration (bn-1ln12).
//!
//! # What this module answers
//!
//! RFC 0008's finite-graph lane, at the grain of the reference engine:
//!
//! > Build the product with a Büchi/parity monitor, find accepting SCCs, and account
//! > for fairness. Counterexamples are lassos or partial-order fair cycles.
//! >
//! > — `notes/plan/rfcs/0008-liveness-fairness-and-progress.md`, "Finite graph"
//!
//! For two property shapes over one of the model's own predicates `P` (named by
//! index, as [`crate::checking::Obligations`] names an invariant — data, never a
//! closure):
//!
//! - [`Goal::Eventually`] — `◇ P`: every fair behaviour from an initial state reaches
//!   a state where `P` holds;
//! - [`Goal::Recurrence`] — `□◇ P`: every fair behaviour visits `P` infinitely often.
//!
//! The fairness is exactly the model's own ([`Model::fairness`], weak and strong, each
//! over a set of actions; semantics in [`crate::model`]'s `fairness` module). There is
//! no other source of fairness here and no default: a model that declares none is
//! checked with none (RFC 0008, "no hidden fairness defaults").
//!
//! # The search
//!
//! A counterexample is a fair behaviour that avoids `P` from some point on (`□◇ P`) or
//! from the start (`◇ P`). Over a finite closed graph it exists exactly when some
//! reachable strongly connected set `C` of `¬P` states (for `◇ P`, of states reachable
//! from an initial state through `¬P` states only) carries a fair cycle. The
//! Emerson–Lei decomposition decides that exactly:
//!
//! 1. take the SCCs of the candidate states; an SCC carries a cycle when it has two or
//!    more states or a self-loop;
//! 2. for each assumption with an edge of its scope inside `C`, the cycle through all
//!    of `C` takes it infinitely often: satisfied;
//! 3. otherwise, a **weak** assumption whose scope is enabled at every state of `C`
//!    is violated by every cycle inside `C` (every subset keeps both facts), so `C` is
//!    dropped; one disabled somewhere in `C` is satisfied by a cycle through that state;
//! 4. otherwise, a **strong** assumption whose scope is enabled somewhere in `C` is
//!    violated by any cycle through such a state: those states are removed and the rest
//!    of `C` is decomposed again; one enabled nowhere in `C` is satisfied;
//! 5. an SCC every assumption accepts is fair: a cycle through all of it satisfies
//!    every assumption at once.
//!
//! Each removal strictly shrinks the set, so the decomposition terminates, and it is
//! exact in both directions (a fair cycle lies inside one SCC of each set it survives
//! in; a fair SCC yields the cycle of step 5). `continuum-kernel-temporal` makes the
//! same judgement for weak fairness from wire form (its `is_fair`), with its own code.
//!
//! # Stuttering, and terminal states
//!
//! Whether an execution may stutter — stay at a state without taking any action — is
//! a declared input, [`Stuttering`], never a default, because it decides what a model
//! without fairness means for liveness. A stutter step takes no action, so it is in no
//! scope: a behaviour that stutters forever at a state is fair exactly when every
//! assumption's scope is disabled there.
//!
//! - [`Stuttering::Everywhere`] — a behaviour may stutter at any state. This is CML's
//!   standard specification `Init && always(step(N) || stutter(state))`, so it is the
//!   reading of a lowered CML model: with no fairness, `◇ P` fails at any initial
//!   state where `P` does, since the behaviour may stutter there forever. Only the
//!   declared fairness excludes stuttering. There is no hidden progress assumption.
//! - [`Stuttering::AtTerminal`] — RFC 0015's `stutter-forever` completion: a state with
//!   no enabled action stutters forever, and no other state stutters. Every other
//!   behaviour is an infinite sequence of actions, which is itself a progress
//!   assumption (a global scheduler that always takes some enabled action); it is the
//!   maximal-execution reading `continuum-kernel-temporal` checks, and the caller
//!   chooses it by name.
//! - [`Stuttering::Never`] — RFC 0015's `deadlock-violation`: behaviours are infinite
//!   action sequences, a finite execution is not a behaviour (the deadlock check of
//!   [`crate::checking`] reports it), and terminal states take no part.
//!
//! A stutter loop of a counterexample is [`Cycle::Stutter`].
//!
//! # Outcomes are typed
//!
//! [`LivenessOutcome`] has the three arms [`crate::checking::CheckOutcome`] has: `Holds`
//! only over a closed exploration; `Violated` with a lasso that
//! [`Model::successors`] reproduces link by link; `Inconclusive` with the
//! [`Unresolved`] reason. An exploration that stopped at a bound is `Inconclusive`,
//! never a pass and, here, never searched (a cycle among explored states would be a
//! genuine counterexample, but this module does not search partial graphs).
//!
//! # Determinism and resources (INV-005)
//!
//! Every collection is a sorted vector or a `BTreeMap`/`BTreeSet`; states are found by
//! binary search in the ascending state table, and scope membership by binary search
//! in the scope, never by a scan inside a loop. The SCC search is iterative with its
//! own stack, so a long cycle is heap, not recursion. Memory is linear in the explored
//! graph, which the [`crate::bfs::Bounds`] of the exploration bound; whether a scope is
//! enabled is read from a state's successor row when it is asked. Time is
//! `O((s + 1) · k · E · log)` for `k` assumptions of which `s` are strong and `E`
//! explored transitions: a strong removal disables its scope in everything that
//! remains, so no set is decomposed more than `s + 1` times deep.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::bfs::{Exploration, Reachable};
use crate::checking::Unresolved;
use crate::fairness::Strength;
use crate::model::{EvaluationError, Model, State};
use crate::witness::Step;

/// The liveness property, over one of the model's declared predicates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Goal {
    /// `◇ P`: every fair behaviour reaches a state where the predicate at this index
    /// holds.
    Eventually(usize),
    /// `□◇ P`: every fair behaviour visits such a state infinitely often.
    Recurrence(usize),
}

impl Goal {
    const fn predicate(self) -> usize {
        match self {
            Self::Eventually(index) | Self::Recurrence(index) => index,
        }
    }
}

/// Where a behaviour may stutter (see the module documentation). Declared by the
/// caller; there is no default.
///
/// | Source | Here |
/// |---|---|
/// | CML `Init && always(step(N) \|\| stutter(state))` | [`Stuttering::Everywhere`] |
/// | `completion_policy` `stutter-forever` (RFC 0015), maximal executions | [`Stuttering::AtTerminal`] |
/// | `completion_policy` `deadlock-violation` | [`Stuttering::Never`]: the finite execution is a deadlock defect, reported by [`crate::checking`] |
/// | `finite-trace-only`, `closed` | **no reading here**: a liveness property is over infinite behaviours, and this model declares no environment |
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stuttering {
    /// Any state may stutter forever; only fairness excludes it.
    Everywhere,
    /// Only a state with no enabled action stutters, forever.
    AtTerminal,
    /// No state stutters: finite executions are not behaviours.
    Never,
}

/// The loop of a lasso.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cycle {
    /// The behaviour stutters forever at the stem's end, where every scope is disabled
    /// (a terminal state, or any state under [`Stuttering::Everywhere`]).
    Stutter,
    /// Steps from the stem's end back to it: never empty.
    Steps(Vec<Step>),
}

/// A fair behaviour that refutes the goal: a stem from an initial state and a loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lasso {
    start: State,
    stem: Vec<Step>,
    cycle: Cycle,
}

impl Lasso {
    /// The initial state it starts from.
    #[must_use]
    pub const fn start(&self) -> &State {
        &self.start
    }

    /// The steps from [`Lasso::start`] to the first state of the loop.
    #[must_use]
    pub fn stem(&self) -> &[Step] {
        &self.stem
    }

    /// The loop, repeated forever.
    #[must_use]
    pub const fn cycle(&self) -> &Cycle {
        &self.cycle
    }

    /// The state the loop starts and ends at.
    #[must_use]
    pub fn loop_state(&self) -> &State {
        self.stem.last().map_or(&self.start, Step::target)
    }
}

/// The answer for one goal over one exploration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LivenessOutcome {
    /// No fair behaviour of the closed reachable set refutes the goal.
    Holds {
        /// How many reachable states were checked.
        states: usize,
    },
    /// A fair behaviour refutes it.
    Violated(Box<Lasso>),
    /// Not settled: the exploration stopped at a bound, or the model failed to evaluate.
    Inconclusive(Unresolved),
}

/// Why no liveness check ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LivenessError {
    /// The goal names a predicate the model does not declare.
    UnknownPredicate {
        /// The index offered.
        index: usize,
        /// How many predicates are declared.
        declared: usize,
    },
    /// An internal consistency check failed: a successor outside the closed set, or a
    /// fair set with no path inside it. An engine defect, reported as data.
    Inconsistent {
        /// What failed.
        detail: &'static str,
    },
}

impl fmt::Display for LivenessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownPredicate { index, declared } => {
                write!(f, "predicate index {index}; the model declares {declared}")
            }
            Self::Inconsistent { detail } => write!(f, "liveness engine defect: {detail}"),
        }
    }
}

impl core::error::Error for LivenessError {}

/// Check `goal` against every fair behaviour of `model`'s closed exploration.
///
/// # Errors
///
/// [`LivenessError::UnknownPredicate`] for a goal outside [`Model::predicates`], and
/// [`LivenessError::Inconsistent`] for an engine defect. A model that fails to evaluate
/// at a reachable state is [`LivenessOutcome::Inconclusive`] with
/// [`Unresolved::EngineError`], and a bounded exploration is `Inconclusive` with
/// [`Unresolved::ResourceExhausted`].
pub fn check_liveness(
    model: &Model,
    exploration: &Exploration,
    goal: Goal,
    stuttering: Stuttering,
) -> Result<LivenessOutcome, LivenessError> {
    let index = goal.predicate();
    if index >= model.predicates().len() {
        return Err(LivenessError::UnknownPredicate {
            index,
            declared: model.predicates().len(),
        });
    }
    let reachable = match exploration {
        Exploration::Complete(reachable) => reachable,
        Exploration::Exhausted(partial) => {
            return Ok(LivenessOutcome::Inconclusive(
                Unresolved::ResourceExhausted {
                    tripped: partial.tripped(),
                    explored: partial.explored().len(),
                    frontier: partial.frontier().len(),
                },
            ));
        }
    };
    let graph = match Graph::build(model, reachable, index)? {
        Ok(graph) => graph,
        Err(unresolved) => return Ok(LivenessOutcome::Inconclusive(unresolved)),
    };
    // Stems run through the states reachable from this model's initial states (for
    // `◇ P`, through `¬P` states only), so a table holding other states cannot lead the
    // search to a set no behaviour reaches.
    let reach = graph.reachable_from_initial(|_| true);
    let (candidates, allowed) = match goal {
        Goal::Eventually(_) => {
            let within = graph.reachable_from_initial(|s| !graph.holds(s));
            (within.clone(), within)
        }
        Goal::Recurrence(_) => {
            let not_p: Vec<bool> = (0..graph.len())
                .map(|s| flag(&reach, s) && !graph.holds(s))
                .collect();
            (not_p, reach)
        }
    };
    let Some(found) = graph.fair_set(&candidates, stuttering)? else {
        return Ok(LivenessOutcome::Holds {
            states: graph.len(),
        });
    };
    let lasso = graph.lasso(model, &found, &allowed)?;
    Ok(LivenessOutcome::Violated(Box::new(lasso)))
}

// ---------------------------------------------------------------------------
// the graph
// ---------------------------------------------------------------------------

/// A fair set: an SCC every assumption accepts, or a state that stutters forever with
/// every scope disabled.
enum Found {
    Component(Vec<usize>),
    Stutter(usize),
}

struct Graph<'a> {
    states: &'a [State],
    /// Per state: `(action, target)` in successor order.
    succ: Vec<Vec<(usize, usize)>>,
    /// Per assumption: its strength.
    strengths: Vec<Strength>,
    /// Per state: whether the goal predicate holds.
    goal: Vec<bool>,
    /// Indices of the initial states.
    initial: Vec<usize>,
    /// The fairness scopes, borrowed from the model.
    scopes: Vec<&'a [usize]>,
}

type Built<'a> = Result<Result<Graph<'a>, Unresolved>, LivenessError>;

fn engine_error(state: &State, source: EvaluationError) -> Unresolved {
    Unresolved::EngineError {
        state: state.clone(),
        source: Box::new(source),
    }
}

impl<'a> Graph<'a> {
    fn build(model: &'a Model, reachable: &'a Reachable, predicate: usize) -> Built<'a> {
        let states = reachable.states();
        let fairness = model.fairness();
        let mut succ: Vec<Vec<(usize, usize)>> = Vec::with_capacity(states.len());
        let mut goal: Vec<bool> = Vec::with_capacity(states.len());
        for state in states {
            let steps = match model.successors(state) {
                Ok(steps) => steps,
                Err(source) => return Ok(Err(engine_error(state, source))),
            };
            let mut row: Vec<(usize, usize)> = Vec::with_capacity(steps.len());
            for step in &steps {
                let Ok(target) = states.binary_search(step.target()) else {
                    return Err(LivenessError::Inconsistent {
                        detail: "a successor outside the closed reachable set",
                    });
                };
                row.push((step.action(), target));
            }
            succ.push(row);
            match model.evaluate_predicate(predicate, state) {
                Ok(holds) => goal.push(holds),
                Err(source) => return Ok(Err(engine_error(state, source))),
            }
        }
        let mut initial: Vec<usize> = Vec::with_capacity(model.initial_states().len());
        for state in model.initial_states() {
            let Ok(index) = states.binary_search(state) else {
                return Err(LivenessError::Inconsistent {
                    detail: "an initial state outside the closed reachable set",
                });
            };
            initial.push(index);
        }
        Ok(Ok(Graph {
            states,
            succ,
            strengths: fairness.iter().map(|f| f.strength()).collect(),
            goal,
            initial,
            scopes: fairness.iter().map(|f| f.actions()).collect(),
        }))
    }

    const fn len(&self) -> usize {
        self.states.len()
    }

    fn edges(&self, s: usize) -> &[(usize, usize)] {
        self.succ.get(s).map_or(&[], Vec::as_slice)
    }

    fn holds(&self, s: usize) -> bool {
        self.goal.get(s).copied().unwrap_or(true)
    }

    /// Whether the scope of assumption `f` is enabled at `s`: whether one of its actions
    /// has a successor there, which is exactly where its guard holds (an enabled action
    /// always has a successor, `Model::successors`). Computed from the row on demand, so
    /// the graph holds no table of assumptions × states.
    fn is_enabled(&self, f: usize, s: usize) -> bool {
        self.edges(s).iter().any(|&(a, _)| self.in_scope(f, a))
    }

    fn in_scope(&self, f: usize, action: usize) -> bool {
        self.scopes
            .get(f)
            .is_some_and(|scope| scope.binary_search(&action).is_ok())
    }

    /// The states reachable from an initial state through states satisfying `keep`
    /// (initial states included when they satisfy it).
    fn reachable_from_initial(&self, keep: impl Fn(usize) -> bool) -> Vec<bool> {
        let mut seen = vec![false; self.len()];
        let mut queue: VecDeque<usize> = VecDeque::new();
        for &s in &self.initial {
            if keep(s) && !flag(&seen, s) {
                set(&mut seen, s);
                queue.push_back(s);
            }
        }
        while let Some(s) = queue.pop_front() {
            for &(_, t) in self.edges(s) {
                if keep(t) && !flag(&seen, t) {
                    set(&mut seen, t);
                    queue.push_back(t);
                }
            }
        }
        seen
    }

    /// The first fair set among `candidates`, by the Emerson–Lei decomposition.
    fn fair_set(
        &self,
        candidates: &[bool],
        stuttering: Stuttering,
    ) -> Result<Option<Found>, LivenessError> {
        let all: Vec<usize> = (0..self.len()).filter(|s| flag(candidates, *s)).collect();
        if stuttering == Stuttering::AtTerminal
            && let Some(&t) = all.iter().find(|s| self.edges(**s).is_empty())
        {
            return Ok(Some(Found::Stutter(t)));
        }
        let mut work: Vec<Vec<usize>> = vec![all];
        let mut inside = vec![false; self.len()];
        let mut component = vec![false; self.len()];
        while let Some(members) = work.pop() {
            for &s in &members {
                set(&mut inside, s);
            }
            let sccs = self.sccs(&members, &inside);
            for &s in &members {
                clear(&mut inside, s);
            }
            for scc in sccs {
                // Under `Everywhere` every state carries its stutter loop, which takes no
                // action: the judgement below is unchanged by it (no scope is taken by it,
                // and it disables nothing), so it only makes a single state cyclic.
                let real = self.carries_cycle(&scc);
                if !real && stuttering != Stuttering::Everywhere {
                    continue;
                }
                for &s in &scc {
                    set(&mut component, s);
                }
                let verdict = self.judge(&scc, &component);
                for &s in &scc {
                    clear(&mut component, s);
                }
                match verdict {
                    Judgement::Fair => {
                        return Ok(Some(match scc.as_slice() {
                            [only] if !real => Found::Stutter(*only),
                            _ => Found::Component(scc),
                        }));
                    }
                    Judgement::Unfair => {}
                    Judgement::Shrink(remove) => {
                        let rest: Vec<usize> =
                            scc.into_iter().filter(|s| !remove.contains(s)).collect();
                        if !rest.is_empty() {
                            work.push(rest);
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    fn carries_cycle(&self, scc: &[usize]) -> bool {
        match scc {
            [only] => self.edges(*only).iter().any(|(_, t)| t == only),
            _ => scc.len() >= 2,
        }
    }

    /// Steps 2–5 of the decomposition for one SCC (`component` marks its members).
    fn judge(&self, scc: &[usize], component: &[bool]) -> Judgement {
        let mut remove: BTreeSet<usize> = BTreeSet::new();
        for (f, strength) in self.strengths.iter().enumerate() {
            let taken = scc.iter().any(|&s| {
                self.edges(s)
                    .iter()
                    .any(|&(a, t)| flag(component, t) && self.in_scope(f, a))
            });
            if taken {
                continue;
            }
            let enabled: Vec<usize> = scc
                .iter()
                .copied()
                .filter(|&s| self.is_enabled(f, s))
                .collect();
            match strength {
                Strength::Weak => {
                    if enabled.len() == scc.len() {
                        return Judgement::Unfair;
                    }
                }
                Strength::Strong => remove.extend(enabled),
            }
        }
        if remove.is_empty() {
            Judgement::Fair
        } else {
            Judgement::Shrink(remove)
        }
    }

    /// The SCCs of the subgraph induced by `members` (`inside` marks them), by an
    /// iterative Tarjan: an explicit call stack, so no recursion.
    fn sccs(&self, members: &[usize], inside: &[bool]) -> Vec<Vec<usize>> {
        let mut index: BTreeMap<usize, (usize, usize)> = BTreeMap::new();
        let mut on_stack: BTreeSet<usize> = BTreeSet::new();
        let mut stack: Vec<usize> = Vec::new();
        let mut out: Vec<Vec<usize>> = Vec::new();
        let mut next = 0_usize;
        for &root in members {
            if index.contains_key(&root) {
                continue;
            }
            index.insert(root, (next, next));
            next = next.saturating_add(1);
            stack.push(root);
            on_stack.insert(root);
            let mut calls: Vec<(usize, usize)> = vec![(root, 0)];
            while let Some(&(v, position)) = calls.last() {
                if let Some(&(_, w)) = self.edges(v).get(position) {
                    if let Some(top) = calls.last_mut() {
                        top.1 = position.saturating_add(1);
                    }
                    if !flag(inside, w) {
                        continue;
                    }
                    match index.get(&w).copied() {
                        None => {
                            index.insert(w, (next, next));
                            next = next.saturating_add(1);
                            stack.push(w);
                            on_stack.insert(w);
                            calls.push((w, 0));
                        }
                        Some((w_index, _)) if on_stack.contains(&w) => {
                            if let Some(entry) = index.get_mut(&v) {
                                entry.1 = entry.1.min(w_index);
                            }
                        }
                        Some(_) => {}
                    }
                    continue;
                }
                calls.pop();
                let (v_index, v_low) = index.get(&v).copied().unwrap_or((0, 0));
                if v_low == v_index {
                    let mut scc: Vec<usize> = Vec::new();
                    while let Some(w) = stack.pop() {
                        on_stack.remove(&w);
                        scc.push(w);
                        if w == v {
                            break;
                        }
                    }
                    scc.sort_unstable();
                    out.push(scc);
                }
                if let Some(&(u, _)) = calls.last()
                    && let Some(entry) = index.get_mut(&u)
                {
                    entry.1 = entry.1.min(v_low);
                }
            }
        }
        out
    }

    // -----------------------------------------------------------------------
    // the witness
    // -----------------------------------------------------------------------

    fn lasso(
        &self,
        model: &Model,
        found: &Found,
        allowed: &[bool],
    ) -> Result<Lasso, LivenessError> {
        let inconsistent = |detail| LivenessError::Inconsistent { detail };
        let (targets, component): (Vec<usize>, Vec<bool>) = match found {
            Found::Stutter(t) => (vec![*t], Vec::new()),
            Found::Component(scc) => {
                let mut marks = vec![false; self.len()];
                for &s in scc {
                    set(&mut marks, s);
                }
                (scc.clone(), marks)
            }
        };
        let is_target = |s: usize| targets.binary_search(&s).is_ok();
        let starts: Vec<usize> = self
            .initial
            .iter()
            .copied()
            .filter(|&s| flag(allowed, s))
            .collect();
        let (start, stem) = self
            .path(&starts, is_target, |s| flag(allowed, s))
            .ok_or(inconsistent("no stem to the fair set"))?;
        let entry = stem.last().map_or(start, |(_, s)| *s);
        let cycle = match found {
            Found::Stutter(_) => Cycle::Stutter,
            Found::Component(_) => {
                let within = |s: usize| flag(&component, s);
                let mut steps: Vec<(usize, usize)> = Vec::new();
                let mut here = entry;
                for waypoint in self.waypoints(&component) {
                    let (to, edge) = match waypoint {
                        Waypoint::Edge(u, a, v) => (u, Some((a, v))),
                        Waypoint::State(s) => (s, None),
                    };
                    let (_, leg) = self
                        .path(&[here], |s| s == to, within)
                        .ok_or(inconsistent("no path inside a fair component"))?;
                    steps.extend(leg);
                    here = to;
                    if let Some((a, v)) = edge {
                        steps.push((a, v));
                        here = v;
                    }
                }
                if steps.is_empty() {
                    // No waypoint: leave the entry by an edge inside the component.
                    let &(a, v) = self
                        .edges(entry)
                        .iter()
                        .find(|(_, t)| within(*t))
                        .ok_or(inconsistent("a fair component without an edge"))?;
                    steps.push((a, v));
                    here = v;
                }
                let (_, back) = self
                    .path(&[here], |s| s == entry, within)
                    .ok_or(inconsistent("no way back inside a fair component"))?;
                steps.extend(back);
                Cycle::Steps(self.steps(model, &steps)?)
            }
        };
        Ok(Lasso {
            start: self.state(start)?.clone(),
            stem: self.steps(model, &stem)?,
            cycle,
        })
    }

    /// What the loop must visit so every assumption is satisfied on it: for an
    /// assumption with an edge of its scope inside the component, that edge (the least
    /// by `(source, successor order)`); for a weak one without, a state where its scope
    /// is disabled; a strong one without an edge is disabled everywhere in the
    /// component (that is why it was accepted), so it needs nothing.
    fn waypoints(&self, component: &[bool]) -> Vec<Waypoint> {
        let members: Vec<usize> = (0..self.len()).filter(|s| flag(component, *s)).collect();
        let mut out: Vec<Waypoint> = Vec::new();
        for (f, strength) in self.strengths.iter().enumerate() {
            let edge = members.iter().find_map(|&u| {
                self.edges(u)
                    .iter()
                    .find(|&&(a, v)| flag(component, v) && self.in_scope(f, a))
                    .map(|&(a, v)| Waypoint::Edge(u, a, v))
            });
            if let Some(edge) = edge {
                out.push(edge);
            } else if *strength == Strength::Weak
                && let Some(&s) = members.iter().find(|&&s| !self.is_enabled(f, s))
            {
                out.push(Waypoint::State(s));
            }
        }
        out
    }

    /// A shortest path (breadth-first, successor order) from one of `sources` to a
    /// state satisfying `target`, through states satisfying `allowed`: the source, and
    /// the `(action, state)` links after it. A source that is itself a target gives an
    /// empty path.
    fn path(
        &self,
        sources: &[usize],
        target: impl Fn(usize) -> bool,
        allowed: impl Fn(usize) -> bool,
    ) -> Option<(usize, Vec<(usize, usize)>)> {
        let mut parent: BTreeMap<usize, Option<(usize, usize)>> = BTreeMap::new();
        let mut queue: VecDeque<usize> = VecDeque::new();
        for &s in sources {
            if let std::collections::btree_map::Entry::Vacant(slot) = parent.entry(s) {
                slot.insert(None);
                queue.push_back(s);
            }
        }
        let mut end = None;
        while let Some(s) = queue.pop_front() {
            if target(s) {
                end = Some(s);
                break;
            }
            for &(a, t) in self.edges(s) {
                if allowed(t) && !parent.contains_key(&t) {
                    parent.insert(t, Some((s, a)));
                    queue.push_back(t);
                }
            }
        }
        let mut links: Vec<(usize, usize)> = Vec::new();
        let mut here = end?;
        while let Some(&Some((from, action))) = parent.get(&here) {
            links.push((action, here));
            here = from;
        }
        links.reverse();
        Some((here, links))
    }

    fn state(&self, s: usize) -> Result<&State, LivenessError> {
        self.states.get(s).ok_or(LivenessError::Inconsistent {
            detail: "a state index outside the table",
        })
    }

    fn steps(&self, model: &Model, links: &[(usize, usize)]) -> Result<Vec<Step>, LivenessError> {
        let mut out = Vec::with_capacity(links.len());
        for &(action, s) in links {
            let name = model
                .actions()
                .get(action)
                .ok_or(LivenessError::Inconsistent {
                    detail: "an action index outside the model",
                })?
                .name()
                .clone();
            out.push(Step::new(action, name, self.state(s)?.clone()));
        }
        Ok(out)
    }
}

enum Judgement {
    Fair,
    Unfair,
    Shrink(BTreeSet<usize>),
}

enum Waypoint {
    Edge(usize, usize, usize),
    State(usize),
}

fn flag(marks: &[bool], s: usize) -> bool {
    marks.get(s).copied().unwrap_or(false)
}

fn set(marks: &mut [bool], s: usize) {
    if let Some(slot) = marks.get_mut(s) {
        *slot = true;
    }
}

fn clear(marks: &mut [bool], s: usize) {
    if let Some(slot) = marks.get_mut(s) {
        *slot = false;
    }
}
