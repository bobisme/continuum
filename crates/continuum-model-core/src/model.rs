//! The programmatic transition model: declaration, canonical state vectors, and the
//! evaluation primitives exploration is built from (PR 8, IMPL-01).
//!
//! # What a model is
//!
//! > A model \(M\) defines: \(M = (S, I, \mathcal{A}, T, O, F)\) … \(S\): typed
//! > states; \(I \subseteq S\): initial states; \(\mathcal{A}\): action schemas;
//! > \(T_a \subseteq S \times Params_a \times S\): transition relations; \(O\):
//! > observations; \(F\): fairness assumptions.
//! >
//! > — `notes/plan/docs/02_SEMANTICS.md:18-31`
//!
//! This module implements the finite fragment of the first four components — the
//! fragment the closed-finite-state-space certificate can carry (docs/03 §6.1). \(S\)
//! is the product of the declared variable domains, \(I\) is an explicit enumeration,
//! \(\mathcal{A}\) is a set of named actions, and each \(T_a\) is a guard plus a
//! non-empty set of simultaneous updates. Observations and fairness are absent on
//! purpose: nothing in PR 8 consumes them, and a field with no consumer is a field
//! with no defined meaning.
//!
//! # What this module is *not*
//!
//! It declares and evaluates; it does not search. Breadth-first exploration,
//! invariant and deadlock *policy*, shortest witnesses, and certificate emission are
//! the four sibling bones of PR 8 and each attaches here through a stable primitive:
//!
//! | Sibling | Primitive it consumes |
//! |---|---|
//! | deterministic BFS | [`Model::initial_states`], [`Model::successors`] |
//! | invariant/deadlock checking | [`Model::evaluate_predicate`], an empty [`Model::successors`] |
//! | shortest witness | [`Step::action`] and [`Step::target`], in the order `successors` returns them |
//! | finite closure certificate | [`Model::variables`], [`Model::actions`], [`State::as_slice`] |
//!
//! A [`Predicate`] here is a *named boolean evaluation* and nothing more. Whether
//! `TypeOK` is an invariant to be upheld and `NotSolved` a goal whose refutation is
//! the answer is checking policy, and checking policy belongs to the checking bone.
//! This module refuses to encode it, so that one model can be checked under several
//! policies without being re-declared.
//!
//! # Canonical order, and why it is not declaration order
//!
//! Variables, actions, initial states, and successor rows are all held in the order
//! the certificate wire form requires, because the ordering rule is what makes
//! certificate emission a pure function of the model rather than of the order someone
//! typed things in:
//!
//! > Five sequences — domain-pack digests, variable names, state vectors, initial
//! > state vectors, action names, and the transitions within one successor row — must
//! > be *strictly* ascending in byte/lexicographic order.
//! >
//! > — `crates/continuum-kernel-core/src/wire.rs:89-92`
//!
//! Concretely:
//!
//! - **Variables** are sorted by name, byte-lexicographically ascending, and a state
//!   vector's positions mean what that order says they mean. `wire::StateDomain`
//!   spells out why the kernel needs this — "Variable names are strictly ascending,
//!   which fixes the meaning of a state vector's positions without a separate index"
//!   (`crates/continuum-kernel-core/src/wire.rs:496-497`) — and `wire::Token` fixes
//!   the relation as byte order (`.../wire.rs:311-314`). **Declaration order is
//!   discarded**: two models that declare the same variables in different orders are
//!   the same model, with the same state vectors and the same certificate.
//! - **Actions** are sorted by name, and an action's index in [`Model::actions`] is
//!   its index in the certificate's action table (`.../wire.rs:750-763`).
//! - **Initial states** and **successor rows** are strictly ascending state vectors,
//!   which for [`State`] is lexicographic order on the `i64` components — the same
//!   comparison `wire::StateTable` makes (`.../wire.rs:616-620`).
//!
//! # Determinism
//!
//! The crate reads no clock, draws no randomness, opens no socket, and uses no
//! hash-ordered collection (INV-005, `notes/plan/plan.md:323-325`; ADR-0003;
//! `GOV-1-03`/`GOV-1-04` scan this source mechanically). Every sequence a caller can
//! observe is sorted by a total order on its own contents, so two enumerations of one
//! model on one state are equal, not merely equivalent — the property `docs/16`
//! PO-MOD-004 asks for, "pure operator evaluation is independent of implementation
//! iteration order" (`notes/plan/docs/16_PROOF_OBLIGATIONS.md:21`).

use core::fmt;
use std::collections::BTreeSet;

use crate::domain::{Domain, DomainError, Variable};
use crate::expr::{BoolExpr, Environment, EvalError, IntExpr, MAX_EXPR_DEPTH};
use crate::ident::{Ident, IdentError};

/// The largest number of state variables a model may declare.
///
/// `MAX_VARIABLES` from `crates/continuum-kernel-core/src/wire.rs:130`. A model beyond
/// it could be explored but never certified, so it is refused where it is declared.
pub const MAX_VARIABLES: usize = 64;

/// The largest number of actions a model may declare.
///
/// `MAX_ACTIONS` from `crates/continuum-kernel-core/src/wire.rs:136`.
pub const MAX_ACTIONS: usize = 4096;

/// The largest number of initial states a model may enumerate.
///
/// `MAX_STATES` from `crates/continuum-kernel-core/src/wire.rs:133`, which also bounds
/// the initial-state list (`.../wire.rs:729`).
pub const MAX_INITIAL_STATES: usize = 1 << 20;

// ---------------------------------------------------------------------------
// states
// ---------------------------------------------------------------------------

/// A state: one value per declared variable, in canonical variable order.
///
/// There is no public constructor. The only way to obtain a `State` is
/// [`Model::state`], which checks arity and every declared domain, or
/// [`Model::successors`], which produces states from states. That is the same
/// discipline `wire::Certificate` uses — "no constructor … outside `wire::decode`"
/// (`crates/continuum-kernel-core/src/wire.rs:14-16`) — applied on the producer side:
/// a value of this type is a state of *some* model, never an arbitrary vector.
///
/// [`Ord`] is lexicographic on the components, which is the comparison the wire form's
/// canonical state table makes (`crates/continuum-kernel-core/src/wire.rs:616-620`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct State {
    values: Vec<i64>,
}

impl State {
    /// The canonical state vector.
    ///
    /// Position `i` is the value of `model.variables()[i]`. This slice is what the
    /// certificate bone writes as a `state` on the wire
    /// (`crates/continuum-kernel-core/src/wire.rs:64`).
    #[must_use]
    pub fn as_slice(&self) -> &[i64] {
        &self.values
    }

    /// How many components the vector has, which is the model's arity.
    #[must_use]
    pub fn arity(&self) -> usize {
        self.values.len()
    }
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("(")?;
        for (index, value) in self.values.iter().enumerate() {
            if index > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{value}")?;
        }
        f.write_str(")")
    }
}

/// One labelled successor: which action fired, and where it landed.
///
/// Deliberately the shape of `wire::Transition` — an action index plus a target *state
/// vector*, not a table index (`crates/continuum-kernel-core/src/wire.rs:642-652`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Step {
    action: usize,
    target: State,
}

impl Step {
    /// The index of the action that fired, in [`Model::actions`] order.
    #[must_use]
    pub const fn action(&self) -> usize {
        self.action
    }

    /// The successor state.
    #[must_use]
    pub const fn target(&self) -> &State {
        &self.target
    }
}

// ---------------------------------------------------------------------------
// actions and predicates
// ---------------------------------------------------------------------------

/// One variable's new value, as an expression over the *pre-state*.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assignment {
    variable: Ident,
    value: IntExpr,
}

impl Assignment {
    /// The variable being assigned.
    #[must_use]
    pub const fn variable(&self) -> &Ident {
        &self.variable
    }

    /// The expression giving its new value.
    #[must_use]
    pub const fn value(&self) -> &IntExpr {
        &self.value
    }
}

/// One possible result of firing an action: a set of simultaneous assignments.
///
/// # Simultaneity, and the frame rule
///
/// Every right-hand side is evaluated in the **pre-state**, and all assignments then
/// land at once. `big' == big + 1 && small' == big` therefore reads the *old* `big`
/// in both, exactly as RFC 0003's relational reading requires — "An action describes a
/// relation between pre- and post-state"
/// (`notes/plan/rfcs/0003-continuum-model-language.md:15`) — and the result cannot
/// depend on the order the assignments happen to be stored in.
///
/// A variable this outcome does not assign is **unchanged**. That is the corpus's
/// `unchanged` frame (`notes/plan/rfcs/0003-continuum-model-language.md:50-56`) as a
/// default rather than a keyword. It is the one implicit rule in this module, it is
/// stated here, and it is the reason a two-variable model can spell `FillBig` as
/// `big' == 5` alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    assignments: Vec<Assignment>,
}

impl Outcome {
    /// The assignments, ordered by variable name and free of duplicates.
    #[must_use]
    pub fn assignments(&self) -> &[Assignment] {
        &self.assignments
    }
}

/// A named action: a guard, and one or more outcomes.
///
/// One outcome is a deterministic action. Several are the "`choose` / existential next
/// values" of `notes/plan/rfcs/0003-continuum-model-language.md:100-113`, enumerated
/// rather than solved: each outcome contributes one successor, and duplicates among
/// them collapse, because the wire form forbids a successor row from carrying the same
/// `(action, target)` pair twice (`crates/continuum-kernel-core/src/wire.rs:786-794`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    name: Ident,
    guard: BoolExpr,
    outcomes: Vec<Outcome>,
}

impl Action {
    /// The action's canonical name.
    #[must_use]
    pub const fn name(&self) -> &Ident {
        &self.name
    }

    /// The guard. `BoolExpr::Const(true)` is an always-enabled action.
    #[must_use]
    pub const fn guard(&self) -> &BoolExpr {
        &self.guard
    }

    /// The outcomes, in declaration order. Never empty.
    #[must_use]
    pub fn outcomes(&self) -> &[Outcome] {
        &self.outcomes
    }

    /// Whether the action has exactly one outcome.
    #[must_use]
    pub fn is_deterministic(&self) -> bool {
        self.outcomes.len() == 1
    }
}

/// A named boolean evaluation over a state.
///
/// Invariants and goals are both this. See the module documentation for why the
/// distinction is not recorded here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Predicate {
    name: Ident,
    body: BoolExpr,
}

impl Predicate {
    /// The predicate's canonical name.
    #[must_use]
    pub const fn name(&self) -> &Ident {
        &self.name
    }

    /// The predicate's body.
    #[must_use]
    pub const fn body(&self) -> &BoolExpr {
        &self.body
    }
}

// ---------------------------------------------------------------------------
// the model
// ---------------------------------------------------------------------------

/// A finite transition system, declared and validated.
///
/// Immutable once built. Every accessor returns a sequence in canonical order, and
/// every evaluation primitive is total: it returns a value or a typed
/// [`EvaluationError`], never a panic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    variables: Vec<Variable>,
    actions: Vec<Action>,
    initial_states: Vec<State>,
    predicates: Vec<Predicate>,
}

impl Model {
    /// The declared variables, ordered by name.
    ///
    /// Position `i` here is position `i` in every [`State`] vector.
    #[must_use]
    pub fn variables(&self) -> &[Variable] {
        &self.variables
    }

    /// How many components a state vector has.
    #[must_use]
    pub fn arity(&self) -> usize {
        self.variables.len()
    }

    /// The canonical position of a variable, or `None` when it is not declared.
    #[must_use]
    pub fn variable_index(&self, name: &str) -> Option<usize> {
        self.variables
            .iter()
            .position(|variable| variable.name().as_str() == name)
    }

    /// The declared actions, ordered by name. An action's position here is its index
    /// in [`Step::action`] and in the certificate's action table.
    #[must_use]
    pub fn actions(&self) -> &[Action] {
        &self.actions
    }

    /// The index of an action, or `None` when it is not declared.
    #[must_use]
    pub fn action_index(&self, name: &str) -> Option<usize> {
        self.actions
            .iter()
            .position(|action| action.name().as_str() == name)
    }

    /// The enumerated initial states, strictly ascending. Never empty.
    #[must_use]
    pub fn initial_states(&self) -> &[State] {
        &self.initial_states
    }

    /// The declared predicates, ordered by name.
    #[must_use]
    pub fn predicates(&self) -> &[Predicate] {
        &self.predicates
    }

    /// The index of a predicate, or `None` when it is not declared.
    #[must_use]
    pub fn predicate_index(&self, name: &str) -> Option<usize> {
        self.predicates
            .iter()
            .position(|predicate| predicate.name().as_str() == name)
    }

    /// How many states the declared domain admits, whether reachable or not.
    ///
    /// The product of the variable cardinalities, saturating at [`u128::MAX`]. This is
    /// the a-priori bound exploration can compare its own progress against; it is not
    /// a claim that any of those states is reachable.
    #[must_use]
    pub fn domain_cardinality(&self) -> u128 {
        self.variables.iter().fold(1_u128, |acc, variable| {
            acc.saturating_mul(variable.domain().cardinality())
        })
    }

    /// Whether `vector` is a well-typed state of this model.
    #[must_use]
    pub fn admits(&self, vector: &[i64]) -> bool {
        vector.len() == self.arity()
            && self
                .variables
                .iter()
                .zip(vector.iter())
                .all(|(variable, value)| variable.domain().admits(*value))
    }

    /// Build a [`State`] from a canonical state vector.
    ///
    /// # Errors
    ///
    /// [`EvaluationError::StateArity`] when the vector's length is not the model's
    /// arity, and [`EvaluationError::StateOutOfDomain`] for the first component
    /// outside its variable's declared range.
    pub fn state(&self, vector: &[i64]) -> Result<State, EvaluationError> {
        if vector.len() != self.arity() {
            return Err(EvaluationError::StateArity {
                expected: self.arity(),
                found: vector.len(),
            });
        }
        for (variable, value) in self.variables.iter().zip(vector.iter()) {
            if !variable.domain().admits(*value) {
                return Err(EvaluationError::StateOutOfDomain {
                    variable: variable.name().clone(),
                    value: *value,
                    domain: *variable.domain(),
                });
            }
        }
        Ok(State {
            values: vector.to_vec(),
        })
    }

    /// The value a state binds to a named variable, or `None` when the name is not
    /// declared or the state has the wrong arity.
    #[must_use]
    pub fn binding(&self, state: &State, name: &str) -> Option<i64> {
        let index = self.variable_index(name)?;
        state.values.get(index).copied()
    }

    /// Whether `action` is enabled in `state`.
    ///
    /// # Errors
    ///
    /// [`EvaluationError::UnknownAction`] for an index outside [`Model::actions`],
    /// the state-validity errors of [`Model::state`], and
    /// [`EvaluationError::Expression`] when the guard cannot be evaluated.
    pub fn is_enabled(&self, action: usize, state: &State) -> Result<bool, EvaluationError> {
        let declared = self.action_for(action)?;
        self.check_state(state)?;
        let environment = Environment::new(&self.variables, &state.values);
        Ok(declared.guard.evaluate(&environment)?)
    }

    /// The successors of `state` under one action: strictly ascending and duplicate
    /// free, and empty exactly when the action is disabled.
    ///
    /// An empty result for a *disabled* action and an empty result for an enabled
    /// action are not distinguishable here, and cannot be: an enabled action always
    /// has at least one outcome, so an enabled action always has at least one
    /// successor. That is what makes an empty [`Model::successors`] an unambiguous
    /// deadlock — docs/03 §6.1's "disabled witness"
    /// (`crates/continuum-kernel-core/src/wire.rs:673-675`).
    ///
    /// # Errors
    ///
    /// Those of [`Model::is_enabled`], plus [`EvaluationError::UpdateOutOfDomain`]
    /// when an update produces a value outside the target variable's declared range.
    /// That case is docs/16 PO-MOD-003 — "every enabled action produces a state in the
    /// declared state domain" (`notes/plan/docs/16_PROOF_OBLIGATIONS.md:17`) — failing,
    /// and it is reported rather than clamped: clamping would invent a transition the
    /// model does not have, and panicking would turn a model defect into a crash.
    pub fn action_successors(
        &self,
        action: usize,
        state: &State,
    ) -> Result<Vec<State>, EvaluationError> {
        let declared = self.action_for(action)?;
        if !self.is_enabled(action, state)? {
            return Ok(Vec::new());
        }
        let environment = Environment::new(&self.variables, &state.values);
        let mut targets: BTreeSet<State> = BTreeSet::new();
        for outcome in &declared.outcomes {
            let mut values = state.values.clone();
            for assignment in &outcome.assignments {
                let value = assignment.value.evaluate(&environment)?;
                let unbound = || {
                    EvaluationError::Expression(EvalError::Unbound {
                        name: assignment.variable.as_str().to_owned(),
                    })
                };
                let index = self
                    .variable_index(assignment.variable.as_str())
                    .ok_or_else(unbound)?;
                let variable = self.variables.get(index).ok_or_else(unbound)?;
                if !variable.domain().admits(value) {
                    return Err(EvaluationError::UpdateOutOfDomain {
                        action: declared.name.clone(),
                        variable: variable.name().clone(),
                        value,
                        domain: *variable.domain(),
                    });
                }
                let Some(slot) = values.get_mut(index) else {
                    return Err(EvaluationError::StateArity {
                        expected: self.arity(),
                        found: state.arity(),
                    });
                };
                *slot = value;
            }
            targets.insert(State { values });
        }
        Ok(targets.into_iter().collect())
    }

    /// Every labelled successor of `state`, strictly ascending by
    /// `(action index, target vector)`.
    ///
    /// That is the order one successor row must be written in
    /// (`crates/continuum-kernel-core/src/wire.rs:786-794`), so the certificate bone
    /// can emit a row by writing this sequence out, and the exploration bone gets a
    /// frontier whose order is a function of the state alone.
    ///
    /// An empty result is a deadlock: no action is enabled.
    ///
    /// # Errors
    ///
    /// Those of [`Model::action_successors`], for the first action that fails.
    pub fn successors(&self, state: &State) -> Result<Vec<Step>, EvaluationError> {
        self.check_state(state)?;
        let mut steps: Vec<Step> = Vec::new();
        for action in 0..self.actions.len() {
            for target in self.action_successors(action, state)? {
                steps.push(Step { action, target });
            }
        }
        Ok(steps)
    }

    /// Evaluate the predicate at `index` on `state`.
    ///
    /// # Errors
    ///
    /// [`EvaluationError::UnknownPredicate`] for an index outside
    /// [`Model::predicates`], the state-validity errors of [`Model::state`], and
    /// [`EvaluationError::Expression`] when the body cannot be evaluated.
    pub fn evaluate_predicate(&self, index: usize, state: &State) -> Result<bool, EvaluationError> {
        let predicate = self
            .predicates
            .get(index)
            .ok_or(EvaluationError::UnknownPredicate {
                index,
                declared: self.predicates.len(),
            })?;
        self.check_state(state)?;
        let environment = Environment::new(&self.variables, &state.values);
        Ok(predicate.body.evaluate(&environment)?)
    }

    fn action_for(&self, index: usize) -> Result<&Action, EvaluationError> {
        self.actions
            .get(index)
            .ok_or(EvaluationError::UnknownAction {
                index,
                declared: self.actions.len(),
            })
    }

    fn check_state(&self, state: &State) -> Result<(), EvaluationError> {
        if state.arity() != self.arity() {
            return Err(EvaluationError::StateArity {
                expected: self.arity(),
                found: state.arity(),
            });
        }
        for (variable, value) in self.variables.iter().zip(state.values.iter()) {
            if !variable.domain().admits(*value) {
                return Err(EvaluationError::StateOutOfDomain {
                    variable: variable.name().clone(),
                    value: *value,
                    domain: *variable.domain(),
                });
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// declaration
// ---------------------------------------------------------------------------

/// An action as declared, before validation.
#[derive(Debug, Clone)]
pub struct ActionDecl {
    name: String,
    guard: BoolExpr,
    outcomes: Vec<Vec<(String, IntExpr)>>,
}

impl ActionDecl {
    /// An action with exactly one outcome.
    #[must_use]
    pub fn deterministic(name: &str, guard: BoolExpr, updates: Vec<(&str, IntExpr)>) -> Self {
        Self::enumerated(name, guard, vec![updates])
    }

    /// An action whose firing chooses among several outcomes.
    ///
    /// Each inner vector is one complete outcome. An empty outer vector is
    /// [`ModelError::ActionWithoutOutcome`] — an action that can fire but produces no
    /// state is not a relation, it is a hole.
    #[must_use]
    pub fn enumerated(name: &str, guard: BoolExpr, outcomes: Vec<Vec<(&str, IntExpr)>>) -> Self {
        Self {
            name: name.to_owned(),
            guard,
            outcomes: outcomes
                .into_iter()
                .map(|updates| {
                    updates
                        .into_iter()
                        .map(|(variable, value)| (variable.to_owned(), value))
                        .collect()
                })
                .collect(),
        }
    }
}

/// Collects declarations; [`ModelBuilder::build`] validates all of them at once.
///
/// The declaration methods are infallible on purpose. Every rule — name grammar,
/// duplicate detection, domain emptiness, unknown variable references, expression
/// depth, initial-state completeness — lives in [`ModelBuilder::build`], so there is
/// exactly one place to read to know what a valid model is, and a declaration site
/// never has to guess which errors it is responsible for.
#[derive(Debug, Clone, Default)]
pub struct ModelBuilder {
    variables: Vec<(String, i64, i64)>,
    actions: Vec<ActionDecl>,
    initial: Vec<Vec<(String, i64)>>,
    predicates: Vec<(String, BoolExpr)>,
}

impl ModelBuilder {
    /// An empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Declare a variable ranging over the inclusive range `lo..=hi`.
    #[must_use]
    pub fn variable(mut self, name: &str, lo: i64, hi: i64) -> Self {
        self.variables.push((name.to_owned(), lo, hi));
        self
    }

    /// Declare an action.
    #[must_use]
    pub fn action(mut self, action: ActionDecl) -> Self {
        self.actions.push(action);
        self
    }

    /// Enumerate one initial state, as a value for every declared variable.
    ///
    /// Named rather than positional because canonical variable order is decided by
    /// [`ModelBuilder::build`], not by the caller: a positional initial state would
    /// silently change meaning when a variable is renamed.
    #[must_use]
    pub fn initial_state(mut self, bindings: &[(&str, i64)]) -> Self {
        self.initial.push(
            bindings
                .iter()
                .map(|(name, value)| ((*name).to_owned(), *value))
                .collect(),
        );
        self
    }

    /// Declare a named boolean evaluation.
    #[must_use]
    pub fn predicate(mut self, name: &str, body: BoolExpr) -> Self {
        self.predicates.push((name.to_owned(), body));
        self
    }

    /// Validate every declaration and produce a [`Model`].
    ///
    /// Rules are applied in a fixed order — variables, actions, expressions, initial
    /// states, predicates — and the first failure is returned, so the error a given
    /// set of declarations produces is itself deterministic.
    ///
    /// # Errors
    ///
    /// Every arm of [`ModelError`].
    pub fn build(self) -> Result<Model, ModelError> {
        let variables = build_variables(&self.variables)?;
        let actions = build_actions(&self.actions, &variables)?;
        let predicates = build_predicates(&self.predicates, &variables)?;
        let initial_states = build_initial_states(&self.initial, &variables)?;
        Ok(Model {
            variables,
            actions,
            initial_states,
            predicates,
        })
    }
}

fn build_variables(declared: &[(String, i64, i64)]) -> Result<Vec<Variable>, ModelError> {
    if declared.is_empty() {
        return Err(ModelError::NoVariables);
    }
    if declared.len() > MAX_VARIABLES {
        return Err(ModelError::TooMany {
            symbol: Symbol::Variable,
            count: declared.len(),
            max: MAX_VARIABLES,
        });
    }
    let mut variables: Vec<Variable> = Vec::new();
    for (name, lo, hi) in declared {
        let name = ident(Symbol::Variable, name)?;
        let domain = Domain::new(*lo, *hi).map_err(|source| ModelError::InvalidDomain {
            variable: name.clone(),
            source,
        })?;
        variables.push(Variable::new(name, domain));
    }
    variables.sort_by(|left, right| left.name().cmp(right.name()));
    for pair in variables.windows(2) {
        if let [left, right] = pair
            && left.name() == right.name()
        {
            return Err(ModelError::Duplicate {
                symbol: Symbol::Variable,
                name: left.name().clone(),
            });
        }
    }
    Ok(variables)
}

fn build_actions(
    declared: &[ActionDecl],
    variables: &[Variable],
) -> Result<Vec<Action>, ModelError> {
    if declared.is_empty() {
        return Err(ModelError::NoActions);
    }
    if declared.len() > MAX_ACTIONS {
        return Err(ModelError::TooMany {
            symbol: Symbol::Action,
            count: declared.len(),
            max: MAX_ACTIONS,
        });
    }
    let mut actions: Vec<Action> = Vec::new();
    for decl in declared {
        let name = ident(Symbol::Action, &decl.name)?;
        check_bool(&decl.guard, variables, &Site::Guard(name.clone()))?;
        if decl.outcomes.is_empty() {
            return Err(ModelError::ActionWithoutOutcome { action: name });
        }
        let mut outcomes: Vec<Outcome> = Vec::new();
        for updates in &decl.outcomes {
            let mut assignments: Vec<Assignment> = Vec::new();
            for (target, value) in updates {
                let variable = ident(Symbol::Variable, target)?;
                if !declares(variables, &variable) {
                    return Err(ModelError::UnknownVariable {
                        site: Site::Update {
                            action: name.clone(),
                            variable: variable.clone(),
                        },
                        name: variable,
                    });
                }
                check_int(
                    value,
                    variables,
                    &Site::Update {
                        action: name.clone(),
                        variable: variable.clone(),
                    },
                )?;
                assignments.push(Assignment {
                    variable,
                    value: value.clone(),
                });
            }
            assignments.sort_by(|left, right| left.variable.cmp(&right.variable));
            for pair in assignments.windows(2) {
                if let [left, right] = pair
                    && left.variable == right.variable
                {
                    return Err(ModelError::DuplicateAssignment {
                        action: name.clone(),
                        variable: left.variable.clone(),
                    });
                }
            }
            outcomes.push(Outcome { assignments });
        }
        actions.push(Action {
            name,
            guard: decl.guard.clone(),
            outcomes,
        });
    }
    actions.sort_by(|left, right| left.name.cmp(&right.name));
    for pair in actions.windows(2) {
        if let [left, right] = pair
            && left.name == right.name
        {
            return Err(ModelError::Duplicate {
                symbol: Symbol::Action,
                name: left.name.clone(),
            });
        }
    }
    Ok(actions)
}

fn build_predicates(
    declared: &[(String, BoolExpr)],
    variables: &[Variable],
) -> Result<Vec<Predicate>, ModelError> {
    let mut predicates: Vec<Predicate> = Vec::new();
    for (name, body) in declared {
        let name = ident(Symbol::Predicate, name)?;
        check_bool(body, variables, &Site::Predicate(name.clone()))?;
        predicates.push(Predicate {
            name,
            body: body.clone(),
        });
    }
    predicates.sort_by(|left, right| left.name.cmp(&right.name));
    for pair in predicates.windows(2) {
        if let [left, right] = pair
            && left.name == right.name
        {
            return Err(ModelError::Duplicate {
                symbol: Symbol::Predicate,
                name: left.name.clone(),
            });
        }
    }
    Ok(predicates)
}

fn build_initial_states(
    declared: &[Vec<(String, i64)>],
    variables: &[Variable],
) -> Result<Vec<State>, ModelError> {
    if declared.is_empty() {
        return Err(ModelError::NoInitialStates);
    }
    if declared.len() > MAX_INITIAL_STATES {
        return Err(ModelError::TooMany {
            symbol: Symbol::InitialState,
            count: declared.len(),
            max: MAX_INITIAL_STATES,
        });
    }
    let mut states: Vec<State> = Vec::new();
    for (index, bindings) in declared.iter().enumerate() {
        let mut values: Vec<Option<i64>> = vec![None; variables.len()];
        for (name, value) in bindings {
            let name = ident(Symbol::Variable, name)?;
            let unplaceable = || ModelError::UnknownVariable {
                site: Site::InitialState(index),
                name: name.clone(),
            };
            let Some((position, variable)) = variables
                .iter()
                .enumerate()
                .find(|(_, declared)| declared.name() == &name)
            else {
                return Err(unplaceable());
            };
            if !variable.domain().admits(*value) {
                return Err(ModelError::InitialValueOutOfDomain {
                    index,
                    variable: name,
                    value: *value,
                    domain: *variable.domain(),
                });
            }
            // Unreachable: `position` indexes `variables`, and `values` was sized from
            // it. Written as data anyway, because the no-panic covenant is a property
            // of every path, not of the reachable ones.
            let Some(slot) = values.get_mut(position) else {
                return Err(unplaceable());
            };
            if slot.is_some() {
                return Err(ModelError::DuplicateInitialBinding {
                    index,
                    variable: name,
                });
            }
            *slot = Some(*value);
        }
        let mut vector: Vec<i64> = Vec::new();
        for (variable, slot) in variables.iter().zip(values.iter()) {
            let Some(value) = *slot else {
                return Err(ModelError::IncompleteInitialState {
                    index,
                    variable: variable.name().clone(),
                });
            };
            vector.push(value);
        }
        states.push(State { values: vector });
    }
    states.sort();
    for pair in states.windows(2) {
        if let [left, right] = pair
            && left == right
        {
            return Err(ModelError::DuplicateInitialState {
                state: left.clone(),
            });
        }
    }
    Ok(states)
}

fn ident(symbol: Symbol, spelling: &str) -> Result<Ident, ModelError> {
    Ident::new(spelling).map_err(|source| ModelError::InvalidName {
        symbol,
        spelling: spelling.to_owned(),
        source,
    })
}

fn declares(variables: &[Variable], name: &Ident) -> bool {
    variables.iter().any(|variable| variable.name() == name)
}

fn check_int(expr: &IntExpr, variables: &[Variable], site: &Site) -> Result<(), ModelError> {
    let depth = expr.depth();
    if depth > MAX_EXPR_DEPTH {
        return Err(ModelError::ExpressionTooDeep {
            site: site.clone(),
            depth,
            limit: MAX_EXPR_DEPTH,
        });
    }
    let mut mentioned: Vec<String> = Vec::new();
    expr.variables(&mut mentioned);
    check_mentioned(&mentioned, variables, site)
}

fn check_bool(expr: &BoolExpr, variables: &[Variable], site: &Site) -> Result<(), ModelError> {
    let depth = expr.depth();
    if depth > MAX_EXPR_DEPTH {
        return Err(ModelError::ExpressionTooDeep {
            site: site.clone(),
            depth,
            limit: MAX_EXPR_DEPTH,
        });
    }
    let mut mentioned: Vec<String> = Vec::new();
    expr.variables(&mut mentioned);
    check_mentioned(&mentioned, variables, site)
}

/// Resolve every name an expression mentions against the declared variables.
///
/// A spelling outside the canonical grammar is [`ModelError::InvalidName`] rather than
/// [`ModelError::UnknownVariable`]: "you wrote a name no model can declare" and "you
/// wrote a name this model does not declare" are different mistakes and get different
/// answers.
fn check_mentioned(
    mentioned: &[String],
    variables: &[Variable],
    site: &Site,
) -> Result<(), ModelError> {
    for spelling in mentioned {
        let name = ident(Symbol::Variable, spelling)?;
        if !declares(variables, &name) {
            return Err(ModelError::UnknownVariable {
                site: site.clone(),
                name,
            });
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// errors
// ---------------------------------------------------------------------------

/// Which kind of name a declaration error is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Symbol {
    /// A state variable.
    Variable,
    /// An action.
    Action,
    /// A predicate.
    Predicate,
    /// An enumerated initial state.
    InitialState,
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match *self {
            Self::Variable => "variable",
            Self::Action => "action",
            Self::Predicate => "predicate",
            Self::InitialState => "initial state",
        })
    }
}

/// Where in a declaration an expression problem was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Site {
    /// The guard of the named action.
    Guard(Ident),
    /// The expression an action assigns to a variable.
    Update {
        /// The action.
        action: Ident,
        /// The variable being assigned.
        variable: Ident,
    },
    /// The body of the named predicate.
    Predicate(Ident),
    /// The enumerated initial state at this position in declaration order.
    InitialState(usize),
}

impl fmt::Display for Site {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Guard(action) => write!(f, "guard of action `{action}`"),
            Self::Update { action, variable } => {
                write!(f, "update `{variable}` of action `{action}`")
            }
            Self::Predicate(name) => write!(f, "body of predicate `{name}`"),
            Self::InitialState(index) => write!(f, "initial state #{index}"),
        }
    }
}

/// Why a set of declarations is not a model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelError {
    /// A name outside the canonical grammar.
    InvalidName {
        /// What was being named.
        symbol: Symbol,
        /// The spelling that was offered.
        spelling: String,
        /// Why it is not a name.
        source: IdentError,
    },
    /// Two declarations share a name.
    Duplicate {
        /// What was being named.
        symbol: Symbol,
        /// The repeated name.
        name: Ident,
    },
    /// More declarations than the certificate wire form can carry.
    TooMany {
        /// What was being counted.
        symbol: Symbol,
        /// How many were declared.
        count: usize,
        /// The limit.
        max: usize,
    },
    /// A variable's range admits no value.
    InvalidDomain {
        /// The variable.
        variable: Ident,
        /// Why the range is not a domain.
        source: DomainError,
    },
    /// No variables: the state space would have no states to be a space of.
    NoVariables,
    /// No actions: the wire form requires at least one
    /// (`crates/continuum-kernel-core/src/wire.rs:750`).
    NoActions,
    /// No initial states: the wire form requires at least one
    /// (`crates/continuum-kernel-core/src/wire.rs:729-732`).
    NoInitialStates,
    /// An action with no outcome.
    ActionWithoutOutcome {
        /// The action.
        action: Ident,
    },
    /// One outcome assigns the same variable twice, so the post-state would depend on
    /// which assignment was applied last.
    DuplicateAssignment {
        /// The action.
        action: Ident,
        /// The variable assigned twice.
        variable: Ident,
    },
    /// An expression mentions an undeclared variable.
    UnknownVariable {
        /// Where.
        site: Site,
        /// The undeclared name.
        name: Ident,
    },
    /// An expression nests beyond [`crate::expr::MAX_EXPR_DEPTH`].
    ExpressionTooDeep {
        /// Where.
        site: Site,
        /// Its depth.
        depth: usize,
        /// The limit.
        limit: usize,
    },
    /// An enumerated initial state binds a variable twice.
    DuplicateInitialBinding {
        /// Position in declaration order.
        index: usize,
        /// The variable bound twice.
        variable: Ident,
    },
    /// An enumerated initial state leaves a declared variable unbound. Every variable
    /// must be given a value: there is no default, because a default would be an
    /// initial state nobody wrote down.
    IncompleteInitialState {
        /// Position in declaration order.
        index: usize,
        /// The unbound variable.
        variable: Ident,
    },
    /// An enumerated initial state is outside a declared domain.
    InitialValueOutOfDomain {
        /// Position in declaration order.
        index: usize,
        /// The variable.
        variable: Ident,
        /// The value offered.
        value: i64,
        /// The declared domain.
        domain: Domain,
    },
    /// Two enumerated initial states are the same state.
    DuplicateInitialState {
        /// The repeated state.
        state: State,
    },
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName {
                symbol,
                spelling,
                source,
            } => write!(f, "{symbol} name {spelling:?}: {source}"),
            Self::Duplicate { symbol, name } => write!(f, "{symbol} `{name}` is declared twice"),
            Self::TooMany { symbol, count, max } => {
                write!(f, "{count} {symbol} declarations; the limit is {max}")
            }
            Self::InvalidDomain { variable, source } => {
                write!(f, "variable `{variable}`: {source}")
            }
            Self::NoVariables => f.write_str("a model declares at least one variable"),
            Self::NoActions => f.write_str("a model declares at least one action"),
            Self::NoInitialStates => f.write_str("a model enumerates at least one initial state"),
            Self::ActionWithoutOutcome { action } => {
                write!(f, "action `{action}` declares no outcome")
            }
            Self::DuplicateAssignment { action, variable } => write!(
                f,
                "action `{action}` assigns `{variable}` twice in one outcome"
            ),
            Self::UnknownVariable { site, name } => {
                write!(f, "{site} mentions undeclared variable `{name}`")
            }
            Self::ExpressionTooDeep { site, depth, limit } => {
                write!(f, "{site} nests {depth} deep; the limit is {limit}")
            }
            Self::DuplicateInitialBinding { index, variable } => write!(
                f,
                "initial state #{index} binds `{variable}` more than once"
            ),
            Self::IncompleteInitialState { index, variable } => {
                write!(f, "initial state #{index} leaves `{variable}` unbound")
            }
            Self::InitialValueOutOfDomain {
                index,
                variable,
                value,
                domain,
            } => write!(
                f,
                "initial state #{index} binds `{variable}` to {value}, outside {domain}"
            ),
            Self::DuplicateInitialState { state } => {
                write!(f, "initial state {state} is enumerated twice")
            }
        }
    }
}

impl core::error::Error for ModelError {}

/// Why an evaluation over a built model produced no answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvaluationError {
    /// A state vector of the wrong length for this model.
    StateArity {
        /// The model's arity.
        expected: usize,
        /// The vector's length.
        found: usize,
    },
    /// A state component outside its variable's declared domain.
    StateOutOfDomain {
        /// The variable.
        variable: Ident,
        /// The offending value.
        value: i64,
        /// The declared domain.
        domain: Domain,
    },
    /// An enabled action produced a value outside the target variable's domain.
    ///
    /// docs/16 PO-MOD-003 failing, reported as data.
    UpdateOutOfDomain {
        /// The action that fired.
        action: Ident,
        /// The variable it assigned.
        variable: Ident,
        /// The value it produced.
        value: i64,
        /// The declared domain.
        domain: Domain,
    },
    /// An action index outside [`Model::actions`].
    UnknownAction {
        /// The index offered.
        index: usize,
        /// How many actions are declared.
        declared: usize,
    },
    /// A predicate index outside [`Model::predicates`].
    UnknownPredicate {
        /// The index offered.
        index: usize,
        /// How many predicates are declared.
        declared: usize,
    },
    /// An expression could not be evaluated.
    Expression(EvalError),
}

impl From<EvalError> for EvaluationError {
    fn from(source: EvalError) -> Self {
        Self::Expression(source)
    }
}

impl fmt::Display for EvaluationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StateArity { expected, found } => {
                write!(f, "state has {found} components; the model has {expected}")
            }
            Self::StateOutOfDomain {
                variable,
                value,
                domain,
            } => write!(f, "state binds `{variable}` to {value}, outside {domain}"),
            Self::UpdateOutOfDomain {
                action,
                variable,
                value,
                domain,
            } => write!(
                f,
                "action `{action}` assigns `{variable}` the value {value}, outside {domain}"
            ),
            Self::UnknownAction { index, declared } => {
                write!(f, "action index {index}; the model declares {declared}")
            }
            Self::UnknownPredicate { index, declared } => {
                write!(f, "predicate index {index}; the model declares {declared}")
            }
            Self::Expression(source) => write!(f, "{source}"),
        }
    }
}

impl core::error::Error for EvaluationError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Expression(source) => Some(source),
            _ => None,
        }
    }
}
