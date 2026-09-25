//! The harness's own closure oracle: a second search over the model core's relation.
//!
//! The reference engine searches breadth-first with a queue, a depth per state, and a
//! discovery chain ([`REFERENCE`](super::engine::REFERENCE)). This oracle shares none
//! of that code. It is a depth-first worklist over an ordered visited map, with one
//! parent pointer per state, written against only [`Model::initial_states`] and
//! [`Model::successors`]. So a defect in the reference's search (a lost successor, a
//! wrong deduplication, a wrong deadlock rule) shows up as a disagreement, and a defect
//! in the shared model evaluator does not: that half is the kernel lane's job, whose
//! evaluator is independent ([`KERNEL`](super::engine::KERNEL)).
//!
//! Its counterexample is a path along parent pointers: valid, not necessarily
//! shortest. The harness compares paths only by replay.
//!
//! Budget: only [`Budget::states`] and [`Budget::transitions`] apply, because this
//! search has no layers. A tripped budget makes every undecided answer
//! `ResourceExhausted`; a violation already found stays a violation, because its state
//! was genuinely reached.
//!
//! Decision records: RFC 0013 (the independent second path a differential needs) and
//! ADR-0003 (no ambient nondeterminism: ordered collections only).

use std::collections::{BTreeMap, BTreeSet};

use continuum_model_core::definedness::{Definedness, Guarded, definedness_base};
use continuum_model_core::model::{Model, State};
use continuum_value::assurance::InconclusiveReason;

use super::engine::{Budget, CLOSURE, Engine, EngineIdentity, Fields};
use super::normal::{
    Inconclusive, InvariantVerdict, Normalized, Projection, Trace, UndefinedKind, UndefinedRead,
};

/// The closure oracle. Stateless.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClosureOracle;

/// Parent of a discovered state: `None` for an initial state.
type Parents = BTreeMap<State, Option<(State, usize)>>;

enum Walk {
    Complete,
    Exhausted(&'static str),
}

fn explore(model: &Model, budget: Budget) -> Result<(Parents, BTreeSet<State>, Walk), String> {
    let mut parents: Parents = BTreeMap::new();
    let mut stack: Vec<State> = Vec::new();
    for initial in model.initial_states() {
        if parents.len() >= budget.states {
            return Ok((parents, BTreeSet::new(), Walk::Exhausted("states")));
        }
        if parents.insert(initial.clone(), None).is_none() {
            stack.push(initial.clone());
        }
    }
    let mut deadlocks: BTreeSet<State> = BTreeSet::new();
    let mut transitions: u64 = 0;
    while let Some(state) = stack.pop() {
        let steps = model
            .successors(&state)
            .map_err(|error| error.to_string())?;
        // Charged before the row is used.
        let row = u64::try_from(steps.len()).unwrap_or(u64::MAX);
        transitions = transitions.saturating_add(row);
        if transitions > budget.transitions {
            return Ok((parents, deadlocks, Walk::Exhausted("transitions")));
        }
        if steps.is_empty() {
            deadlocks.insert(state.clone());
        }
        for step in steps {
            if parents.contains_key(step.target()) {
                continue;
            }
            if parents.len() >= budget.states {
                return Ok((parents, deadlocks, Walk::Exhausted("states")));
            }
            parents.insert(step.target().clone(), Some((state.clone(), step.action())));
            stack.push(step.target().clone());
        }
    }
    Ok((parents, deadlocks, Walk::Complete))
}

/// The path to `end` along parent pointers. `None` if a pointer is missing, which the
/// walk above never produces.
fn path(model: &Model, parents: &Parents, end: &State) -> Option<Trace> {
    let mut steps: Vec<(String, Vec<i64>)> = Vec::new();
    let mut current = end.clone();
    // Each hop moves to a strictly earlier-discovered state, so the walk ends within
    // `parents.len()` hops; the bound makes that a checked fact.
    for _ in 0..=parents.len() {
        match parents.get(&current)? {
            None => {
                steps.reverse();
                return Some(Trace {
                    start: current.as_slice().to_vec(),
                    steps,
                });
            }
            Some((parent, action)) => {
                let name = model.actions().get(*action)?.name().as_str().to_owned();
                steps.push((name, current.as_slice().to_vec()));
                current = parent.clone();
            }
        }
    }
    None
}

impl Engine for ClosureOracle {
    fn identity(&self) -> EngineIdentity {
        EngineIdentity {
            slot: CLOSURE.to_owned(),
            build: concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION")).to_owned(),
        }
    }

    fn fields(&self) -> Fields {
        Fields::ALL
    }

    fn evaluate(&self, model: &Model, budget: Budget) -> Normalized {
        let names = || model.predicates().iter().map(|p| p.name().as_str());
        let (parents, deadlocks, walk) = match explore(model, budget) {
            Ok(found) => found,
            Err(detail) => {
                return Normalized::inconclusive(
                    &Inconclusive::new(InconclusiveReason::EngineError, detail),
                    names(),
                );
            }
        };
        let exhausted = match walk {
            Walk::Complete => None,
            Walk::Exhausted(bound) => Some(Inconclusive::new(
                InconclusiveReason::ResourceExhausted,
                format!(
                    "closure oracle {bound} budget tripped at {} states",
                    parents.len()
                ),
            )),
        };
        // Definedness (RFC 0003), read with the model core's classification of the
        // `X#defined` predicates and nothing of the reference's scan. Every explored
        // state is read, and an evaluation error anywhere wins outright; then an
        // undefined action read anywhere invalidates every verdict; else a false member
        // of an invariant's own chain is an undefined read in it; else its value
        // decides. At a state with an undefined action read nothing else is read. Which
        // reached state is shown is this oracle's choice (the canonically least in its
        // own map); the harness checks it against the model.
        let definedness = Definedness::of(model);
        let read_at = |guard: usize, state: &State| -> UndefinedRead {
            let name = model
                .predicates()
                .get(guard)
                .map(|p| p.name().as_str())
                .unwrap_or_default();
            let kind = match definedness.guards(guard) {
                Some(Guarded::Predicate(_)) => UndefinedKind::Invariant,
                Some(Guarded::Action) | None => UndefinedKind::Action,
            };
            UndefinedRead {
                kind,
                subject: definedness_base(name).unwrap_or(name).to_owned(),
                state: Some(state.as_slice().to_vec()),
                // The path is attached to the kept read only (below).
                path: None,
            }
        };
        // Its own parent pointers: the proof that the kept state is reached.
        let with_path = |mut read: UndefinedRead| {
            if let Some(values) = &read.state {
                if let Ok(state) = model.state(values) {
                    read.path = path(model, &parents, &state);
                }
            }
            read
        };
        // Per state: the first false action guard, if any (deepest first per chain).
        let mut action_at: BTreeMap<&State, UndefinedRead> = BTreeMap::new();
        for state in parents.keys() {
            'chains: for chain in definedness.action_chains() {
                for &guard in chain {
                    match model.evaluate_predicate(guard, state) {
                        Ok(true) => {}
                        Ok(false) => {
                            action_at.insert(state, read_at(guard, state));
                            break 'chains;
                        }
                        Err(error) => {
                            return Normalized::inconclusive(
                                &Inconclusive::new(
                                    InconclusiveReason::EngineError,
                                    error.to_string(),
                                ),
                                names(),
                            );
                        }
                    }
                }
            }
        }
        let undefined_action = action_at.values().next().cloned().map(&with_path);
        let mut invariants = BTreeMap::new();
        for (index, predicate) in model.predicates().iter().enumerate() {
            let verdict = match judge(model, &definedness, &parents, &action_at, index, &read_at) {
                Err(detail) => Some(InvariantVerdict::Inconclusive(Inconclusive::new(
                    InconclusiveReason::EngineError,
                    detail,
                ))),
                Ok(_) if undefined_action.is_some() => {
                    undefined_action.clone().map(InvariantVerdict::Undefined)
                }
                Ok(Some(InvariantVerdict::Undefined(read))) => {
                    Some(InvariantVerdict::Undefined(with_path(read)))
                }
                Ok(found) => found,
            };
            let verdict = verdict.unwrap_or_else(|| match &exhausted {
                None => InvariantVerdict::Holds,
                Some(why) => InvariantVerdict::Inconclusive(why.clone()),
            });
            invariants.insert(predicate.name().as_str().to_owned(), verdict);
        }
        let projection = match exhausted {
            None => Projection::Exact {
                states: parents.keys().map(|s| s.as_slice().to_vec()).collect(),
                deadlocks: deadlocks.iter().map(|s| s.as_slice().to_vec()).collect(),
            },
            Some(why) => Projection::Inconclusive(why),
        };
        Normalized {
            projection,
            invariants,
            undefined_action,
        }
    }
}

/// One invariant over every explored state: an evaluation error anywhere wins; then
/// an undefined read in its own chain (or, for a definedness predicate, itself false);
/// then a violation; else `None`. States with an undefined action read are not read.
fn judge(
    model: &Model,
    definedness: &Definedness,
    parents: &Parents,
    action_at: &BTreeMap<&State, UndefinedRead>,
    index: usize,
    read_at: &dyn Fn(usize, &State) -> UndefinedRead,
) -> Result<Option<InvariantVerdict>, String> {
    let is_guard = definedness.guards(index).is_some();
    let mut undefined: Option<UndefinedRead> = None;
    let mut violated: Option<&State> = None;
    for state in parents.keys() {
        if action_at.contains_key(state) {
            continue;
        }
        let mut own_undefined = None;
        for &guard in definedness.guards_of(index) {
            if !model
                .evaluate_predicate(guard, state)
                .map_err(|e| e.to_string())?
            {
                own_undefined = Some(read_at(guard, state));
                break;
            }
        }
        if let Some(read) = own_undefined {
            undefined.get_or_insert(read);
            continue;
        }
        if !model
            .evaluate_predicate(index, state)
            .map_err(|e| e.to_string())?
        {
            if is_guard {
                // A definedness predicate that is false is an undefined read, never a
                // violation.
                undefined.get_or_insert(read_at(index, state));
            } else {
                violated.get_or_insert(state);
            }
        }
    }
    Ok(match (undefined, violated) {
        (Some(read), _) => Some(InvariantVerdict::Undefined(read)),
        (None, Some(state)) => Some(InvariantVerdict::Violated {
            witness: path(model, parents, state),
        }),
        (None, None) => None,
    })
}
