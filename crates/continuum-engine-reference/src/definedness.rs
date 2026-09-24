//! Undefined reads as a typed outcome on every checking path (bn-24a5c, INV-008).
//!
//! # What the RFC requires
//!
//! A lowered CML model names each undefined read with a predicate `X#defined` of the
//! action or invariant `X` ([`crate::model`]'s core states the convention in
//! `continuum_model_core::definedness`). RFC 0003, "Definedness":
//!
//! > A state that violates `X#defined` is the typed outcome "undefined read in `X`",
//! > never a verdict of a declared invariant, and the verdict of `I` at such a state
//! > is not a CML verdict.
//!
//! RFC 0013, "Undefined and partial behavior": an undefined operation is "an explicit
//! `Undefined` semantic result that invalidates the model", never an arbitrary value.
//!
//! The lowering conjoins an action's definedness to each instance's guard, so the
//! lowered model computes no successor from an undefined read. That makes the action
//! look disabled at such a state, and it is not: the state is an error of the model.
//! So this engine reads the predicates and reports [`Undefined`] rather than letting
//! the lowered value stand in for a CML one.
//!
//! # Precedence
//!
//! Per explored state, in ascending canonical order, [`scan`] reads:
//!
//! 1. every action's definedness chain, in the order of its shallowest predicate, each
//!    deepest first. A false one marks the state as an *undefined action read*, and
//!    nothing else is read there;
//! 2. the subject predicate's own chain `I#defined` (and any deeper
//!    `I#defined#defined`, cr-pt5h3a), deepest first. A false one marks the state as an
//!    *undefined read in `I`*, and `I` is not read there;
//! 3. the subject predicate itself, when it is asked for. When the subject is itself a
//!    definedness predicate `X#defined`, a false value is an undefined read in `X`,
//!    never a violation.
//!
//! The first evaluation error in that order wins outright
//! ([`crate::checking::Unresolved::EngineError`]). After the scan, the outcome is: an
//! undefined action read, then an undefined read in the subject, then a violation,
//! then the claim's own answer. Each "first" is the least depth, then the canonically
//! least state — the rule [`crate::witness::shortest`] uses for its endpoint.
//!
//! This is the precedence `continuum-incremental` reached independently (bn-31vf,
//! cr-1jv75r): evaluation error, undefined action, undefined invariant, violation,
//! then `Holds` or `Inconclusive`. RFC 0003 and RFC 0013 state no order among these;
//! they state only that an undefined read is never a verdict and invalidates the
//! model. An undefined action read dominates because the reachable set that remains
//! is then not the CML reachable set.
//!
//! An undefined read in a *bounded* exploration is reported as found, as a violation
//! is: the state was genuinely reached, and no larger bound can un-reach it.

use core::fmt;

use continuum_model_core::definedness::{Definedness, Guarded, definedness_base};

use crate::bfs::{Exploration, Reachable};
use crate::checking::{Evidence, Unresolved};
use crate::ident::Ident;
use crate::model::{EvaluationError, Model, State};
use crate::witness::{self, Target};

/// An undefined read at an explored state: the typed outcome "undefined read in `X`"
/// (RFC 0003, "Definedness").
///
/// Not a verdict of any declared invariant, and not a budget question: an error of the
/// model at a state it reaches (RFC 0013, "Undefined and partial behavior").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Undefined {
    guard: usize,
    name: Ident,
    read: Guarded,
    state: State,
    depth: usize,
    states: usize,
    evidence: Evidence,
}

impl Undefined {
    /// The index of the definedness predicate `X#defined` that is false at
    /// [`Self::state`].
    #[must_use]
    pub const fn guard(&self) -> usize {
        self.guard
    }

    /// That predicate's declared name, `X#defined`.
    #[must_use]
    pub const fn guard_name(&self) -> &Ident {
        &self.name
    }

    /// The action or predicate `X` whose read is undefined: the base of the false
    /// predicate's chain (`I` for `I#defined` and for `I#defined#defined`).
    #[must_use]
    pub fn subject(&self) -> &str {
        definedness_base(self.name.as_str()).unwrap_or(self.name.as_str())
    }

    /// Whether `X` is an action or a declared predicate.
    #[must_use]
    pub const fn read(&self) -> Guarded {
        self.read
    }

    /// The shallowest such state; ties go to the canonically least.
    #[must_use]
    pub const fn state(&self) -> &State {
        &self.state
    }

    /// That state's breadth-first depth.
    #[must_use]
    pub const fn depth(&self) -> usize {
        self.depth
    }

    /// How many explored states carry an undefined read of this kind (INV-007: a
    /// report that shows one names how many it omitted).
    #[must_use]
    pub const fn states(&self) -> usize {
        self.states
    }

    /// A shortest path to [`Self::state`], or the typed reason there is none.
    #[must_use]
    pub const fn evidence(&self) -> &Evidence {
        &self.evidence
    }
}

impl fmt::Display for Undefined {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self.read {
            Guarded::Action => "action",
            Guarded::Predicate(_) => "predicate",
        };
        write!(
            f,
            "undefined read in {kind} {} ({}) state={} depth={} states={}",
            self.subject(),
            self.name,
            self.state,
            self.depth,
            self.states
        )
    }
}

/// The first marked state of one kind, and how many states carry that mark.
#[derive(Debug, Clone)]
pub(crate) struct First {
    pub(crate) state: State,
    pub(crate) depth: usize,
    /// The predicate that is false there.
    pub(crate) predicate: usize,
    pub(crate) count: usize,
}

/// Record a marked state: only a strictly shallower one displaces the incumbent, so
/// over an ascending scan ties go to the canonically least.
fn note(slot: &mut Option<First>, state: &State, depth: usize, predicate: usize) {
    match slot {
        Some(first) => {
            first.count = first.count.saturating_add(1);
            if depth < first.depth {
                first.state = state.clone();
                first.depth = depth;
                first.predicate = predicate;
            }
        }
        None => {
            *slot = Some(First {
                state: state.clone(),
                depth,
                predicate,
                count: 1,
            });
        }
    }
}

/// What one [`scan`] found.
#[derive(Debug, Default)]
pub(crate) struct Scan {
    /// The first state where some action's definedness predicate is false.
    pub(crate) action: Option<First>,
    /// The first state (with every action defined) where the subject's read is
    /// undefined.
    pub(crate) subject: Option<First>,
    /// The first state (with every read defined) where the subject is false.
    pub(crate) falsified: Option<First>,
}

impl Scan {
    /// The dominant undefined read, by the module's precedence.
    pub(crate) fn undefined(self) -> Option<First> {
        self.action.or(self.subject)
    }
}

/// A predicate that could not be evaluated during a [`scan`]: the state, and what the
/// model layer reported.
#[derive(Debug)]
pub(crate) struct Failed {
    pub(crate) state: State,
    pub(crate) source: Box<EvaluationError>,
}

impl From<Failed> for Unresolved {
    fn from(failed: Failed) -> Self {
        Self::EngineError {
            state: failed.state,
            source: failed.source,
        }
    }
}

/// Scan every explored state in ascending order, with the module's precedence.
///
/// A chain is followed whole: a definedness predicate's own guard is an obligation of
/// the same base (cr-pt5h3a), so no guard is skipped by its nesting.
///
/// `subject` is the predicate whose own reads are checked, or `None` for the actions
/// alone. `evaluate` also evaluates the subject where its reads are defined, and
/// records where it is false. A subject that is itself a definedness predicate is
/// always evaluated, because its value is a definedness question.
pub(crate) fn scan(
    model: &Model,
    reachable: &Reachable,
    definedness: &Definedness,
    subject: Option<usize>,
    evaluate: bool,
) -> Result<Scan, Failed> {
    let mut found = Scan::default();
    let eval = |index: usize, state: &State| {
        model
            .evaluate_predicate(index, state)
            .map_err(|source| Failed {
                state: state.clone(),
                source: Box::new(source),
            })
    };
    for (state, depth) in reachable.states().iter().zip(reachable.depths().iter()) {
        let mut undefined_action = None;
        'chains: for chain in definedness.action_chains() {
            for &guard in chain {
                if !eval(guard, state)? {
                    undefined_action = Some(guard);
                    break 'chains;
                }
            }
        }
        if let Some(guard) = undefined_action {
            note(&mut found.action, state, *depth, guard);
            continue;
        }
        let Some(index) = subject else {
            continue;
        };
        let mut undefined_subject = None;
        for &guard in definedness.guards_of(index) {
            if !eval(guard, state)? {
                undefined_subject = Some(guard);
                break;
            }
        }
        if let Some(guard) = undefined_subject {
            note(&mut found.subject, state, *depth, guard);
            continue;
        }
        let is_guard = definedness.guards(index).is_some();
        if (evaluate || is_guard) && !eval(index, state)? {
            if is_guard {
                note(&mut found.subject, state, *depth, index);
            } else {
                note(&mut found.falsified, state, *depth, index);
            }
        }
    }
    Ok(found)
}

/// The typed outcome for a marked state, with a shortest path to it when the
/// exploration closed.
///
/// # Errors
///
/// [`Unresolved::EngineError`] when `first` names a predicate the model does not
/// declare, which a [`scan`] of the same model never produces.
pub(crate) fn outcome(
    model: &Model,
    exploration: &Exploration,
    definedness: &Definedness,
    first: First,
) -> Result<Undefined, Unresolved> {
    let Some(predicate) = model.predicates().get(first.predicate) else {
        return Err(Unresolved::EngineError {
            state: first.state,
            source: Box::new(EvaluationError::UnknownPredicate {
                index: first.predicate,
                declared: model.predicates().len(),
            }),
        });
    };
    let read = definedness
        .guards(first.predicate)
        .unwrap_or(Guarded::Action);
    let evidence = Evidence::of(match exploration {
        Exploration::Complete(reachable) => witness::shortest_in(
            model,
            reachable,
            definedness,
            &Target::State(first.state.clone()),
        ),
        Exploration::Exhausted(_) => {
            witness::shortest(model, exploration, &Target::State(first.state.clone()))
        }
    });
    Ok(Undefined {
        guard: first.predicate,
        name: predicate.name().clone(),
        read,
        state: first.state,
        depth: first.depth,
        states: first.count,
        evidence,
    })
}
