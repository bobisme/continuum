//! The shortest labelled path to a chosen state (PR 8, IMPL-04).
//!
//! # What a witness is, and what it is for
//!
//! A depth is a number; a witness is the thing you can hand somebody. docs/03 §3
//! gives an assurance result a `witness: Option<ArtifactRef>` beside its
//! `certificate` (`notes/plan/docs/03_ASSURANCE_AND_TCB.md:28`) for exactly that
//! asymmetry: a certificate is how a *positive* claim survives an independent
//! checker, and a witness is how a *negative* one does. Plan §7 lists "exact
//! counterexamples and replay records" first among the forms evidence takes
//! (`notes/plan/plan.md:89`). This module produces the engine-grain form of one: a
//! sequence of `(action, state)` pairs starting at a declared initial state, every
//! link of which [`Model::successors`] will reproduce.
//!
//! Die Hard is the corpus example and it is not an accident. `NotSolved` is declared
//! "intentionally false" (`DieHard.ctm:31`), so the interesting artifact for that
//! model is not "the invariant holds" but *the shortest way it fails* — which is the
//! film's six-step solution, frozen as PR 8's exit sentence and proved independently
//! in `lean/Continuum/Examples/DieHard.lean:14-26`.
//!
//! # Where the path comes from
//!
//! Nowhere new. [`crate::bfs`] already records, for every discovered state, the
//! labelled transition that first reached it ([`Discovery`]), under the canonical
//! admission order — so the shortest path to a state is read off the recorded chain
//! rather than searched for a second time. This module contributes two things the
//! exploration deliberately does not have: **which** state to walk back from, and the
//! refusal to answer at all when the exploration cannot support the word *shortest*.
//!
//! # Choosing the endpoint
//!
//! A [`Target`] can be satisfied by many discovered states. The one chosen is the
//! shallowest, and among equally shallow ones the ascending-least — which is the
//! first in [`Reachable::states`], so the rule is "scan the canonical table and keep
//! the first strict improvement". Both halves are total orders on the exploration's
//! own data, so the endpoint is a function of the model and the target alone.
//!
//! The target is **data**, never a caller-supplied predicate function. A
//! `Fn(&State) -> bool` would be ambient effect reaching into the search, which is the
//! construct [`crate::expr`] refuses for guards and [`crate::bfs`] refuses for
//! progress callbacks, and it would put a witness's endpoint outside the model that
//! the witness is evidence about. [`Target`] therefore names either one state or one
//! of the model's own declared predicates.
//!
//! # Why a truncated exploration is refused
//!
//! [`shortest`] takes an [`Exploration`] and answers only for
//! [`Exploration::Complete`], mirroring [`Exploration::closed`]. The states in a
//! partial result are genuinely reached and their depths are genuinely shortest — a
//! bounded run is a depth-preserving prefix of the complete one — but the *minimum
//! over the target* is not established: a state that satisfies the target at a
//! shallower depth may simply not have been discovered yet. Returning the shallowest
//! path found so far and calling it shortest is the silent truncation INV-009 exists
//! to prevent, and INV-008 says the honest answer has its own name
//! ([`NoWitness::Truncated`]) rather than being a smaller success
//! (`notes/plan/plan.md:335-337`).
//!
//! One special case is deliberately *not* carved out. Under a pure depth bound every
//! layer up to the bound is complete, so a "shortest within `k` steps, or none within
//! `k`" claim would in fact be sound there. That is a different claim from the one
//! this module makes, it needs its own type to keep the two from being confused at
//! the call site, and inventing it here — where the bound that tripped is knowable
//! but the *reason it tripped* is only sometimes enough — would make the guarantee
//! depend on which bound a caller happened to set. It is left to whoever needs it.
//!
//! # Witnesses do not judge
//!
//! A path to a deadlock, a path to an invariant violation and a path to an ordinary
//! state are the same object here, produced by the same code, and this module says
//! nothing about which of them is a defect. That is the same split [`crate::bfs`]
//! makes when it surfaces an empty successor row without calling it a deadlock:
//! policy belongs to the checking bone (IMPL-03), which decides *what* to ask for a
//! witness to and calls [`shortest`] with the answer.
//!
//! # Determinism (INV-005)
//!
//! There is no input but the model, the exploration and the target, and no iteration
//! order that is not a total order on the data: the endpoint scan runs over the
//! ascending state table, and the walk back is a chain with exactly one recorded
//! predecessor per link. Two extractions on one exploration are *equal*, and their
//! [`Display`](fmt::Display) renderings are byte-identical;
//! `tests/witness_contract.rs` asserts it.

use core::fmt;

use crate::bfs::{Bound, Discovery, Exploration, Reachable};
use crate::ident::Ident;
use crate::model::{Action, EvaluationError, Model, State};

// ---------------------------------------------------------------------------
// what to look for
// ---------------------------------------------------------------------------

/// Which discovered states a witness may end at.
///
/// Data rather than a predicate function, for the reason the module docs give: a
/// witness's endpoint has to be something the model can state, or the witness is
/// evidence about something other than the model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// Exactly one state. The endpoint is that state if it was discovered, and there
    /// is no witness otherwise.
    State(State),
    /// The states where the model's predicate at this index evaluates to `true`.
    Holds(usize),
    /// The states where it evaluates to `false` — the shape an invariant violation
    /// takes. Die Hard's `NotSolved` is the corpus example.
    Fails(usize),
}

// ---------------------------------------------------------------------------
// the witness
// ---------------------------------------------------------------------------

/// One step of a witness: the action that fired, and where it landed.
///
/// The action is carried twice, as the index a certificate's successor row would
/// write (`crates/continuum-kernel-core/src/wire.rs:642-652`) and as the name a
/// reader needs, because a witness is read by people and machines both and neither
/// should have to hold the model to make sense of it.
///
/// The *source* state is not carried: it is the previous step's target, or
/// [`Witness::start`] for the first step, and storing it would let a witness be built
/// that disagrees with itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    action: usize,
    name: Ident,
    target: State,
}

impl Step {
    /// The index of the action that fired, in [`Model::actions`] order.
    #[must_use]
    pub const fn action(&self) -> usize {
        self.action
    }

    /// That action's declared name.
    #[must_use]
    pub const fn name(&self) -> &Ident {
        &self.name
    }

    /// The state the step reached.
    #[must_use]
    pub const fn target(&self) -> &State {
        &self.target
    }
}

impl fmt::Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "--{}--> {}", self.name, self.target)
    }
}

/// A shortest labelled path from an initial state to a state satisfying a [`Target`].
///
/// [`Witness::len`] is the number of steps, and it is equal to
/// [`Reachable::depth_of`] at [`Witness::target`] by construction rather than by a
/// later check: the walk back takes exactly that many hops or the extraction fails.
///
/// "A shortest", not "the shortest": a state can be reached by several paths of the
/// same minimal length, and the one here is the canonical one — the chain
/// [`crate::bfs`] recorded under its admission order. It is a function of the model
/// and the target, not a choice made at extraction time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Witness {
    start: State,
    steps: Vec<Step>,
}

impl Witness {
    /// The initial state the path starts at.
    #[must_use]
    pub const fn start(&self) -> &State {
        &self.start
    }

    /// The steps, in order.
    #[must_use]
    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    /// How many steps the path has, which is the endpoint's breadth-first depth.
    #[must_use]
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Whether the path has no steps — the target was already satisfied at an initial
    /// state, which is a witness of depth 0 and not an absent one.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// The state the path ends at: the last step's target, or [`Witness::start`] when
    /// there are no steps.
    #[must_use]
    pub fn target(&self) -> &State {
        self.steps.last().map_or(&self.start, Step::target)
    }

    /// Every state on the path, starting with [`Witness::start`]. One longer than
    /// [`Witness::len`].
    #[must_use]
    pub fn states(&self) -> Vec<&State> {
        let mut states: Vec<&State> = Vec::with_capacity(self.steps.len().saturating_add(1));
        states.push(&self.start);
        states.extend(self.steps.iter().map(Step::target));
        states
    }
}

impl fmt::Display for Witness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.start)?;
        for step in &self.steps {
            write!(f, " {step}")?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// why there is no witness
// ---------------------------------------------------------------------------

/// Why an exploration yielded no shortest witness.
///
/// Distinct arms rather than one `None`, for INV-008's reason: "no state satisfies the
/// target" is a claim about the model, "the exploration stopped early" is a claim
/// about the budget, and a caller that conflated them would report a property as
/// holding because it ran out of time (`notes/plan/plan.md:335-337`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoWitness {
    /// The exploration stopped at a declared bound, so *shortest* is not a claim it
    /// can support. Raise the bound named here and ask again.
    Truncated {
        /// Which bound stopped it.
        tripped: Bound,
        /// How many states it had discovered.
        explored: usize,
    },
    /// The exploration is closed and no state in it satisfies the target. For a
    /// complete exploration this is a real negative result: no reachable state
    /// satisfies the target, at any depth.
    Unreached {
        /// How many discovered states were examined — the whole reachable set.
        searched: usize,
    },
    /// A [`Target::Holds`] or [`Target::Fails`] predicate could not be evaluated at a
    /// reachable state, or names no declared predicate.
    ///
    /// A defect in the declaration or an index from the wrong model, not a budget
    /// question. Boxed for the reason [`crate::bfs::ExplorationError`] boxes its
    /// source: the success path should not be widened by a failure it does not have.
    Predicate {
        /// The index that was asked for.
        index: usize,
        /// The state it was being evaluated at.
        state: State,
        /// What the model layer reported.
        source: Box<EvaluationError>,
    },
    /// A recorded step names an action index the model does not declare, which means
    /// the exploration is not an exploration of this model.
    ///
    /// The one mismatch that can be caught mechanically. A [`Reachable`] carries no
    /// model identity, so pairing it with the model it came from is the caller's
    /// obligation; this arm catches the subset of violations that would otherwise
    /// have produced a confidently mislabelled path.
    ForeignExploration {
        /// The action index the chain recorded.
        action: usize,
        /// How many actions the model declares.
        declared: usize,
    },
    /// The recorded predecessor chain left the discovered set before reaching an
    /// initial state.
    ///
    /// Structurally unreachable for a [`Reachable`] this crate produced — [`Discovery`]
    /// is written once per state at discovery time, the predecessor is always a state
    /// that was dequeued, and [`Reachable`] has no public constructor — and it is a
    /// value rather than a panic because the alternative in a crate that forbids
    /// panicking is a crash instead of a report.
    Detached {
        /// The state whose discovery could not be read.
        state: State,
    },
}

impl fmt::Display for NoWitness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated { tripped, explored } => write!(
                f,
                "the exploration reached its {tripped} bound after {explored} states, \
                 so no path from it is known to be shortest"
            ),
            Self::Unreached { searched } => write!(
                f,
                "none of the {searched} reachable states satisfies the target"
            ),
            Self::Predicate {
                index,
                state,
                source,
            } => write!(f, "predicate {index} at state {state}: {source}"),
            Self::ForeignExploration { action, declared } => write!(
                f,
                "the exploration records action {action}; the model declares {declared}"
            ),
            Self::Detached { state } => write!(
                f,
                "the discovery chain leaves the reachable set at state {state}"
            ),
        }
    }
}

impl core::error::Error for NoWitness {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Predicate { source, .. } => Some(&**source),
            Self::Truncated { .. }
            | Self::Unreached { .. }
            | Self::ForeignExploration { .. }
            | Self::Detached { .. } => None,
        }
    }
}

// ---------------------------------------------------------------------------
// the extraction
// ---------------------------------------------------------------------------

/// The shortest labelled path from an initial state to a state satisfying `target`.
///
/// Deterministic: the returned value is a function of `model`, `exploration` and
/// `target` and of nothing else.
///
/// `exploration` must be an exploration of `model`. Nothing here can check that in
/// general — a [`Reachable`] carries no model identity — and the mismatch that *can*
/// be caught is [`NoWitness::ForeignExploration`].
///
/// # Errors
///
/// [`NoWitness::Truncated`] when the exploration stopped at a bound,
/// [`NoWitness::Unreached`] when no discovered state satisfies the target,
/// [`NoWitness::Predicate`] when a predicate target cannot be evaluated, and the two
/// mismatch arms.
pub fn shortest(
    model: &Model,
    exploration: &Exploration,
    target: &Target,
) -> Result<Witness, NoWitness> {
    let reachable = match exploration {
        Exploration::Complete(reachable) => reachable,
        Exploration::Exhausted(partial) => {
            return Err(NoWitness::Truncated {
                tripped: partial.tripped(),
                explored: partial.explored().len(),
            });
        }
    };
    let (endpoint, depth) = select(model, reachable, target)?;
    trace(model, reachable, endpoint, depth)
}

/// The shallowest state satisfying `target`, and its depth.
///
/// Ties go to the ascending-least state: the scan runs over the canonical table in
/// order and only a *strictly* shallower state displaces the incumbent.
fn select<'a>(
    model: &Model,
    reachable: &'a Reachable,
    target: &Target,
) -> Result<(&'a State, usize), NoWitness> {
    let mut best: Option<(&'a State, usize)> = None;
    for (state, depth) in reachable.states().iter().zip(reachable.depths().iter()) {
        if !satisfies(model, target, state)? {
            continue;
        }
        let improves = match best {
            Some((_, incumbent)) => *depth < incumbent,
            None => true,
        };
        if improves {
            best = Some((state, *depth));
        }
    }
    best.ok_or(NoWitness::Unreached {
        searched: reachable.len(),
    })
}

/// Whether one state is an admissible endpoint.
fn satisfies(model: &Model, target: &Target, state: &State) -> Result<bool, NoWitness> {
    match target {
        Target::State(wanted) => Ok(state == wanted),
        Target::Holds(index) => evaluate(model, *index, state),
        Target::Fails(index) => evaluate(model, *index, state).map(|holds| !holds),
    }
}

fn evaluate(model: &Model, index: usize, state: &State) -> Result<bool, NoWitness> {
    model
        .evaluate_predicate(index, state)
        .map_err(|source| NoWitness::Predicate {
            index,
            state: state.clone(),
            source: Box::new(source),
        })
}

/// Walk the recorded [`Discovery`] chain back from `endpoint`.
///
/// The loop runs exactly `depth` times, so the returned witness has `depth` steps
/// whatever else happens — the "path length equals the depth map" property is a
/// consequence of the loop bound rather than an assertion after the fact. Reaching a
/// state at depth 0 that is *not* an initial state, or a state that is not in the
/// table at all, means the chain is not the one this crate writes.
fn trace(
    model: &Model,
    reachable: &Reachable,
    endpoint: &State,
    depth: usize,
) -> Result<Witness, NoWitness> {
    let mut steps: Vec<Step> = Vec::with_capacity(depth);
    let mut here: &State = endpoint;
    for _ in 0..depth {
        let Some(Discovery::Step {
            predecessor,
            action,
        }) = reachable.origin_of(here)
        else {
            return Err(NoWitness::Detached {
                state: here.clone(),
            });
        };
        let name = model.actions().get(*action).map(Action::name).ok_or(
            NoWitness::ForeignExploration {
                action: *action,
                declared: model.actions().len(),
            },
        )?;
        steps.push(Step {
            action: *action,
            name: name.clone(),
            target: here.clone(),
        });
        here = predecessor;
    }
    if !matches!(reachable.origin_of(here), Some(Discovery::Initial)) {
        return Err(NoWitness::Detached {
            state: here.clone(),
        });
    }
    steps.reverse();
    Ok(Witness {
        start: here.clone(),
        steps,
    })
}
