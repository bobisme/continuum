//! A fixture: a model held as its declarations, so it can be taken apart.
//!
//! [`Model`] is validated and immutable. Delta debugging needs to delete one
//! declaration at a time and rebuild, so the harness keeps each corpus entry as a
//! [`Fixture`] and builds the model on demand through [`ModelBuilder`], the one place
//! a valid model is defined. [`Fixture::from_model`] reads any model back into this
//! form, so a model from any front end (programmatic, CML, a generator) can enter the
//! corpus.
//!
//! Decision records: RFC 0003 (the model the declarations describe) and ADR-0013 (model
//! identity is the canonical encoding, so a rebuilt fixture is the same model).

use std::collections::BTreeSet;

use continuum_model_core::expr::{BoolExpr, IntExpr};
use continuum_model_core::fairness::Strength;
use continuum_model_core::model::{ActionDecl, Model, ModelBuilder, ModelError};

/// One declared action: a guard and one or more outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureAction {
    /// The action's name.
    pub name: String,
    /// Its guard.
    pub guard: BoolExpr,
    /// Its outcomes; each is a list of simultaneous assignments.
    pub outcomes: Vec<Vec<(String, IntExpr)>>,
}

/// A model as a list of declarations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fixture {
    /// The corpus label, for reports only. It is not part of any identity: the model
    /// identity ([`Model::identity`]) is.
    pub label: String,
    /// Variables as `(name, lo, hi)`.
    pub variables: Vec<(String, i64, i64)>,
    /// Actions.
    pub actions: Vec<FixtureAction>,
    /// Initial states, each a binding for every variable.
    pub initial_states: Vec<Vec<(String, i64)>>,
    /// Named predicates; the harness checks every one as an invariant.
    pub predicates: Vec<(String, BoolExpr)>,
    /// Fairness assumptions as `(strength, action names)`.
    pub fairness: Vec<(Strength, Vec<String>)>,
}

/// One deletable declaration of a [`Fixture`]. The unit of minimization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Element {
    /// Variable `i`. Deleting it also drops its bindings from every initial state;
    /// an expression that still names it makes the rebuilt model invalid.
    Variable(usize),
    /// Action `i`, with all its outcomes.
    Action(usize),
    /// Outcome `j` of action `i`.
    Outcome(usize, usize),
    /// Initial state `i`.
    Initial(usize),
    /// Predicate `i`.
    Predicate(usize),
    /// Fairness assumption `i`.
    Fairness(usize),
}

impl Fixture {
    /// Read a validated model back into declarations. `build` of the result gives a
    /// model with the same identity.
    #[must_use]
    pub fn from_model(label: &str, model: &Model) -> Self {
        let names: Vec<String> = model
            .variables()
            .iter()
            .map(|variable| variable.name().as_str().to_owned())
            .collect();
        let variables = model
            .variables()
            .iter()
            .map(|variable| {
                (
                    variable.name().as_str().to_owned(),
                    variable.domain().lo(),
                    variable.domain().hi(),
                )
            })
            .collect();
        let actions = model
            .actions()
            .iter()
            .map(|action| FixtureAction {
                name: action.name().as_str().to_owned(),
                guard: action.guard().clone(),
                outcomes: action
                    .outcomes()
                    .iter()
                    .map(|outcome| {
                        outcome
                            .assignments()
                            .iter()
                            .map(|assignment| {
                                (
                                    assignment.variable().as_str().to_owned(),
                                    assignment.value().clone(),
                                )
                            })
                            .collect()
                    })
                    .collect(),
            })
            .collect();
        let initial_states = model
            .initial_states()
            .iter()
            .map(|state| {
                names
                    .iter()
                    .cloned()
                    .zip(state.as_slice().iter().copied())
                    .collect()
            })
            .collect();
        let predicates = model
            .predicates()
            .iter()
            .map(|predicate| {
                (
                    predicate.name().as_str().to_owned(),
                    predicate.body().clone(),
                )
            })
            .collect();
        let fairness = model
            .fairness()
            .iter()
            .map(|assumption| {
                (
                    assumption.strength(),
                    assumption
                        .actions()
                        .iter()
                        .filter_map(|index| model.actions().get(*index))
                        .map(|action| action.name().as_str().to_owned())
                        .collect(),
                )
            })
            .collect();
        Self {
            label: label.to_owned(),
            variables,
            actions,
            initial_states,
            predicates,
            fairness,
        }
    }

    /// Build the model through [`ModelBuilder`].
    ///
    /// # Errors
    ///
    /// Whatever [`ModelBuilder::build`] refuses. A minimization candidate that does not
    /// build is not a reproduction.
    pub fn build(&self) -> Result<Model, ModelError> {
        let mut builder = ModelBuilder::new();
        for (name, lo, hi) in &self.variables {
            builder = builder.variable(name, *lo, *hi);
        }
        for action in &self.actions {
            let outcomes: Vec<Vec<(&str, IntExpr)>> = action
                .outcomes
                .iter()
                .map(|outcome| {
                    outcome
                        .iter()
                        .map(|(variable, value)| (variable.as_str(), value.clone()))
                        .collect()
                })
                .collect();
            builder = builder.action(ActionDecl::enumerated(
                &action.name,
                action.guard.clone(),
                outcomes,
            ));
        }
        for initial in &self.initial_states {
            let bindings: Vec<(&str, i64)> = initial
                .iter()
                .map(|(name, value)| (name.as_str(), *value))
                .collect();
            builder = builder.initial_state(&bindings);
        }
        for (name, body) in &self.predicates {
            builder = builder.predicate(name, body.clone());
        }
        for (strength, actions) in &self.fairness {
            builder = builder.fairness(*strength, actions.iter().map(String::as_str));
        }
        builder.build()
    }

    /// Every deletable declaration, in a fixed order.
    #[must_use]
    pub fn elements(&self) -> Vec<Element> {
        let mut out = Vec::new();
        out.extend((0..self.variables.len()).map(Element::Variable));
        for (index, action) in self.actions.iter().enumerate() {
            out.push(Element::Action(index));
            if action.outcomes.len() > 1 {
                out.extend(
                    (0..action.outcomes.len()).map(|outcome| Element::Outcome(index, outcome)),
                );
            }
        }
        out.extend((0..self.initial_states.len()).map(Element::Initial));
        out.extend((0..self.predicates.len()).map(Element::Predicate));
        out.extend((0..self.fairness.len()).map(Element::Fairness));
        out
    }

    /// The fixture restricted to `keep`. An outcome survives only when its action does
    /// and it is itself kept (single-outcome actions have no outcome elements, so their
    /// one outcome follows the action). A deleted variable is projected out of every
    /// initial state.
    #[must_use]
    pub fn restrict(&self, keep: &BTreeSet<Element>) -> Self {
        let kept_variables: BTreeSet<&str> = self
            .variables
            .iter()
            .enumerate()
            .filter(|(index, _)| keep.contains(&Element::Variable(*index)))
            .map(|(_, (name, _, _))| name.as_str())
            .collect();
        let variables = self
            .variables
            .iter()
            .filter(|(name, _, _)| kept_variables.contains(name.as_str()))
            .cloned()
            .collect();
        let actions = self
            .actions
            .iter()
            .enumerate()
            .filter(|(index, _)| keep.contains(&Element::Action(*index)))
            .map(|(index, action)| FixtureAction {
                name: action.name.clone(),
                guard: action.guard.clone(),
                outcomes: if action.outcomes.len() > 1 {
                    action
                        .outcomes
                        .iter()
                        .enumerate()
                        .filter(|(outcome, _)| keep.contains(&Element::Outcome(index, *outcome)))
                        .map(|(_, outcome)| outcome.clone())
                        .collect()
                } else {
                    action.outcomes.clone()
                },
            })
            .collect();
        let initial_states = self
            .initial_states
            .iter()
            .enumerate()
            .filter(|(index, _)| keep.contains(&Element::Initial(*index)))
            .map(|(_, initial)| {
                initial
                    .iter()
                    .filter(|(name, _)| kept_variables.contains(name.as_str()))
                    .cloned()
                    .collect()
            })
            .collect();
        let predicates = self
            .predicates
            .iter()
            .enumerate()
            .filter(|(index, _)| keep.contains(&Element::Predicate(*index)))
            .map(|(_, predicate)| predicate.clone())
            .collect();
        let fairness = self
            .fairness
            .iter()
            .enumerate()
            .filter(|(index, _)| keep.contains(&Element::Fairness(*index)))
            .map(|(_, assumption)| assumption.clone())
            .collect();
        Self {
            label: self.label.clone(),
            variables,
            actions,
            initial_states,
            predicates,
            fairness,
        }
    }

    /// How many deletable declarations the fixture has.
    #[must_use]
    pub fn size(&self) -> usize {
        self.elements().len()
    }
}
