//! Counterexample validity by replay.
//!
//! Two correct engines may return different counterexamples for one violated
//! invariant, so a path is never compared with another path. It is replayed against
//! the model instead: the start is an initial state, each step is a successor of the
//! state before it under the named action, and the last state falsifies the
//! invariant. The replay uses only `continuum-model-core`'s declared primitives
//! ([`Model::state`], [`Model::action_successors`], [`Model::evaluate_predicate`]), so
//! a path is checked against the semantics, not against the engine that found it.
//!
//! Decision records: RFC 0010 (a negative claim survives as a replayable
//! counterexample) and RFC 0003 (the transition relation replayed against).

use continuum_model_core::model::{Model, State};

use std::collections::BTreeSet;

use continuum_model_core::definedness::{Definedness, Guarded, definedness_base};

use super::normal::{Trace, UndefinedKind, UndefinedRead};

/// The longest path the replay accepts. Checked before any step is replayed, so an
/// oversized path costs one comparison.
pub const MAX_REPLAY_STEPS: usize = 1 << 16;

/// Why a counterexample does not replay.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReplayFault {
    /// The path is longer than [`MAX_REPLAY_STEPS`].
    TooLong {
        /// Its length.
        steps: usize,
    },
    /// The model declares no predicate of that name.
    UnknownInvariant,
    /// A state vector is not a state of the model (arity or domain).
    NotAState {
        /// Step index; `0` is the start.
        at: usize,
    },
    /// The start is not an initial state.
    NotInitial,
    /// The model declares no action of that name.
    UnknownAction {
        /// Step index, from `1`.
        at: usize,
    },
    /// The step's target is not a successor of the previous state under its action.
    NotASuccessor {
        /// Step index, from `1`.
        at: usize,
    },
    /// The model could not evaluate a step or the invariant.
    Evaluation {
        /// Step index; the invariant check is at the path's length.
        at: usize,
    },
    /// The path ends in a state that satisfies the invariant.
    EndSatisfies,
}

impl ReplayFault {
    /// The fault's class, without its position: part of a disagreement's key.
    #[must_use]
    pub const fn class(&self) -> &'static str {
        match self {
            Self::TooLong { .. } => "too-long",
            Self::UnknownInvariant => "unknown-invariant",
            Self::NotAState { .. } => "not-a-state",
            Self::NotInitial => "not-initial",
            Self::UnknownAction { .. } => "unknown-action",
            Self::NotASuccessor { .. } => "not-a-successor",
            Self::Evaluation { .. } => "evaluation",
            Self::EndSatisfies => "end-satisfies",
        }
    }
}

/// Replay `trace` as a counterexample to the invariant `invariant` of `model`.
///
/// # Errors
///
/// The first [`ReplayFault`] found, walking from the start.
pub fn replay(model: &Model, invariant: &str, trace: &Trace) -> Result<(), ReplayFault> {
    let predicate = model
        .predicate_index(invariant)
        .ok_or(ReplayFault::UnknownInvariant)?;
    let current = walk(model, trace)?;
    match model.evaluate_predicate(predicate, &current) {
        Ok(false) => Ok(()),
        Ok(true) => Err(ReplayFault::EndSatisfies),
        Err(_) => Err(ReplayFault::Evaluation {
            at: trace.steps.len(),
        }),
    }
}

/// Walk `trace` from an initial state, each step a successor under its named action,
/// and return the state it ends in: a proof that the end state is reached.
///
/// # Errors
///
/// The first [`ReplayFault`] found, walking from the start.
pub fn walk(model: &Model, trace: &Trace) -> Result<State, ReplayFault> {
    if trace.steps.len() > MAX_REPLAY_STEPS {
        return Err(ReplayFault::TooLong {
            steps: trace.steps.len(),
        });
    }
    let mut current = model
        .state(&trace.start)
        .map_err(|_| ReplayFault::NotAState { at: 0 })?;
    if !model.initial_states().contains(&current) {
        return Err(ReplayFault::NotInitial);
    }
    for (offset, (action, target)) in trace.steps.iter().enumerate() {
        let at = offset.saturating_add(1);
        let index = model
            .action_index(action)
            .ok_or(ReplayFault::UnknownAction { at })?;
        let next = model
            .state(target)
            .map_err(|_| ReplayFault::NotAState { at })?;
        let successors = model
            .action_successors(index, &current)
            .map_err(|_| ReplayFault::Evaluation { at })?;
        if !successors.contains(&next) {
            return Err(ReplayFault::NotASuccessor { at });
        }
        current = next;
    }
    Ok(current)
}

/// Why an undefined-read claim does not hold at its state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UndefinedFault {
    /// The claim names no state, so it cannot be checked.
    NoState,
    /// The state is not a state of the model.
    NotAState,
    /// No definedness chain of that kind has that subject as its base.
    UnknownSubject,
    /// Every member of the subject's chain holds at the state: the read is defined.
    Defined,
    /// The model could not evaluate a member of the chain.
    Evaluation,
    /// The state is not in the claiming side's own reachable set.
    Unreached,
    /// The subject is not the base of the chain of the invariant the claim explains.
    WrongSubject,
    /// The claim's path does not replay from an initial state.
    PathDoesNotReplay,
    /// The claim's path replays but ends in another state than the one claimed.
    PathEndsElsewhere,
    /// A model-level (action) field carries a claim of another kind.
    NotAnActionRead,
    /// Nothing proves the state is reached: the claim carries no path and the side's
    /// reachable set is not exact. Not a fault of the model's answer, but never
    /// agreement: the field is undecided.
    Unproven,
}

impl UndefinedFault {
    /// The fault's class, part of a disagreement's key.
    #[must_use]
    pub const fn class(self) -> &'static str {
        match self {
            Self::NoState => "no-state",
            Self::NotAState => "not-a-state",
            Self::UnknownSubject => "unknown-subject",
            Self::Defined => "defined",
            Self::Evaluation => "evaluation",
            Self::Unreached => "unreached",
            Self::WrongSubject => "wrong-subject",
            Self::PathDoesNotReplay => "path-does-not-replay",
            Self::PathEndsElsewhere => "path-ends-elsewhere",
            Self::NotAnActionRead => "not-an-action-read",
            Self::Unproven => "unproven",
        }
    }
}

/// Check an undefined-read claim against the model, as a counterexample is replayed.
///
/// The claimed state must be a state of the model, and **proven reached**: the claim's
/// own path replays from an initial state to it, or, without a path, it is in
/// `reached` (the claiming side's own exact reachable set). With neither, a claim whose
/// read is undefined at the state is [`UndefinedFault::Unproven`], which the harness
/// records undecided and never counts as agreement. At the state:
///
/// - kind `Action`: some member of an action definedness chain whose base is the
///   claimed subject is false;
/// - kind `Invariant`, for the verdict on `invariant`: the subject is the base of that
///   invariant's chain, and a member of `invariant`'s own guards
///   ([`Definedness::guards_of`]), or `invariant` itself when it is a definedness
///   predicate, is false. So a claim is tied to the verdict it explains;
/// - with no `invariant` (a model-level claim) the kind must be `Action`
///   ([`UndefinedFault::NotAnActionRead`] otherwise).
///
/// The chains are the model core's classification ([`Definedness::of`]).
///
/// # Errors
///
/// The [`UndefinedFault`] that makes the claim not hold.
pub fn check_undefined(
    model: &Model,
    read: &UndefinedRead,
    invariant: Option<&str>,
    reached: Option<&BTreeSet<Vec<i64>>>,
) -> Result<(), UndefinedFault> {
    let values = read.state.as_ref().ok_or(UndefinedFault::NoState)?;
    let state = model.state(values).map_err(|_| UndefinedFault::NotAState)?;
    // Reachability is proven, never assumed: by replaying the claim's own path to the
    // state, or by the side's exact reachable set. With neither the claim is unproven.
    // Both proofs are checked when both exist. A path too long to replay proves
    // nothing, and is not a fault: it falls back to the reachable set.
    let mut proven = false;
    if let Some(path) = &read.path {
        match walk(model, path) {
            Ok(end) if end == state => proven = true,
            Ok(_) => return Err(UndefinedFault::PathEndsElsewhere),
            Err(ReplayFault::TooLong { .. }) => {}
            Err(_) => return Err(UndefinedFault::PathDoesNotReplay),
        }
    }
    if let Some(set) = reached {
        if !set.contains(values) {
            return Err(UndefinedFault::Unreached);
        }
        proven = true;
    }
    // A model-level claim is the undefined *action* read (RFC 0003: it invalidates
    // the model); an invariant read cannot be one.
    if invariant.is_none() && read.kind != UndefinedKind::Action {
        return Err(UndefinedFault::NotAnActionRead);
    }
    let definedness = Definedness::of(model);
    let name_of = |index: usize| model.predicates().get(index).map(|p| p.name().as_str());
    let kind_of = |index: usize| match definedness.guards(index) {
        Some(Guarded::Action) => Some(UndefinedKind::Action),
        Some(Guarded::Predicate(_)) => Some(UndefinedKind::Invariant),
        None => None,
    };
    let members: Vec<usize> = match (read.kind, invariant) {
        (UndefinedKind::Invariant, Some(name)) => {
            let index = model
                .predicate_index(name)
                .ok_or(UndefinedFault::UnknownSubject)?;
            if definedness_base(name).unwrap_or(name) != read.subject {
                return Err(UndefinedFault::WrongSubject);
            }
            let mut own: Vec<usize> = definedness.guards_of(index).to_vec();
            if kind_of(index) == Some(UndefinedKind::Invariant) {
                own.push(index);
            }
            own
        }
        _ => (0..model.predicates().len())
            .filter(|&index| {
                kind_of(index) == Some(read.kind)
                    && name_of(index).and_then(definedness_base) == Some(read.subject.as_str())
            })
            .collect(),
    };
    if members.is_empty() {
        return Err(UndefinedFault::UnknownSubject);
    }
    for index in members {
        match model.evaluate_predicate(index, &state) {
            Ok(false) => {
                // The read is undefined there; whether the state is reached is proven
                // above, or it is not.
                return if proven {
                    Ok(())
                } else {
                    Err(UndefinedFault::Unproven)
                };
            }
            Ok(true) => {}
            Err(_) => return Err(UndefinedFault::Evaluation),
        }
    }
    Err(UndefinedFault::Defined)
}
