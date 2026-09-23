//! Lowering a normalized model to the programmatic model of `continuum-model-core`.
//!
//! Decision: PR 15a ("the programmatic model API remains supported — CML is a second
//! front end, not a replacement") and ADR-0025 ("silently finite integers/sets" is a
//! rejected alternative).
//!
//! The lowering builds its result through [`continuum_model_core::ModelBuilder`], the
//! same builder a programmatic author calls. There is no second model type: an
//! elaborated model and a hand-built model are both [`Model`], and they are the same
//! model exactly when their [`Model::identity`] values are equal.
//!
//! # What lowers
//!
//! The programmatic model is a finite transition system over bounded integer variables
//! (docs/03 §6.1, the fragment the finite closure certificate carries). A normalized
//! model lowers when:
//!
//! - every state variable is `Int` or `Nat` with a refinement that fixes a finite
//!   interval: `v <= c`, `v < c`, `v >= c`, `v > c`, or `v in a..b`, with constant
//!   bounds (`Nat` supplies the lower bound `0`);
//! - actions take no parameters, and every guard, update, and invariant uses only
//!   integer arithmetic (`+ - *`, unary `-`, `min`, `max`), comparisons, `in a..b`, and
//!   the boolean connectives;
//! - the init predicate is satisfied by at least one state of the (finite) domain;
//! - every behavior is the standard specification `Init && always(step(N) ||
//!   stutter(state))`, where `N` offers every action — which is exactly what the
//!   programmatic model means — and there is no fairness assumption, which the
//!   programmatic model cannot carry.
//!
//! Anything else is a typed [`Unlowerable`] reason, never an approximation: an unbounded
//! `Nat` is not silently bounded, a quantifier is not silently unrolled, and a fairness
//! assumption is not silently dropped.
//!
//! # Refinements are domains
//!
//! `big: Nat where big <= 5` becomes the declared domain `0..=5`. An update that leaves
//! the domain is then [`continuum_model_core::EvaluationError::UpdateOutOfDomain`] at
//! exploration time — the refinement type is enforced where the value lands (docs/16
//! PO-MOD-003), and is never clamped.

use std::fmt;

use continuum_cml_syntax::Span;
use continuum_model_core::domain::{Domain, Variable};
use continuum_model_core::expr::{Environment, MAX_EXPR_DEPTH};
use continuum_model_core::{
    ActionDecl, BoolExpr, CmpOp, EvalError, Ident, IntExpr, Model, ModelBuilder, ModelError,
};

use crate::budget::{Budget, Fuel, Limits, Usage, lookup_cost, sort_cost};
use crate::norm::{BinOp, Builtin, Expr, ExprKind, Next, NormModel, Temporal};
use crate::types::Type;

/// The largest state domain whose init predicate the lowering enumerates.
///
/// This bounds the *time* of the enumeration: each candidate state is one evaluation
/// over one reused value vector, with no allocation.
pub const MAX_INIT_ENUMERATION: u128 = 1 << 22;

/// The most variable bindings (initial states times state variables) the lowering hands
/// to the builder.
///
/// This bounds the *memory* of the enumeration. Every accepted initial state becomes a
/// list of owned `(name, value)` bindings in [`ModelBuilder`], and the builder refuses a
/// list that is too long only after it holds all of it. The count is checked before each
/// state is added, so a model whose init accepts too many states is refused at a bounded
/// allocation (INV-016: source is untrusted). At the limit the bindings take a few tens
/// of MiB.
pub const MAX_INIT_BINDINGS: usize = 1 << 20;

/// Why a well-formed model does not lower to the programmatic model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Unlowerable {
    /// A state variable whose type is not `Int` or `Nat`.
    NonIntegerState,
    /// An integer state variable with no finite interval: finiteness is never invented.
    UnboundedDomain,
    /// A refinement clause that is not a constant interval bound.
    NonIntervalRefinement,
    /// An action with parameters (an action schema).
    ParameterizedAction,
    /// A model constant, which needs a run configuration.
    Constant,
    /// A quantifier or comprehension.
    Quantifier,
    /// `/` or `%`, which the programmatic expression language does not have.
    DivisionOrModulo,
    /// A value of a non-integer, non-boolean type: sets, maps, sequences, options,
    /// tuples, records, strings, sorts, enumerations.
    NonIntegerValue,
    /// An integer-valued `if`.
    ConditionalValue,
    /// A fairness assumption, which the programmatic model cannot carry.
    Fairness,
    /// A behavior other than the standard specification over every action.
    NonStandardBehavior,
    /// No `init` declaration.
    NoInit,
    /// A state domain larger than [`MAX_INIT_ENUMERATION`], so the init predicate is not
    /// enumerated.
    InitDomainTooLarge,
    /// The init predicate accepts more states than [`MAX_INIT_BINDINGS`] allows.
    TooManyInitialStates,
    /// An expression nested deeper than the programmatic model accepts
    /// (`continuum_model_core::expr::MAX_EXPR_DEPTH`). Checked before lowering, without
    /// recursion, so lowering never recurses deeper than that bound.
    ExpressionTooDeep,
    /// Lowering would exceed the work budget ([`crate::budget::MAX_WORK`]); for init
    /// enumeration this is decided from candidates × predicate cost before enumerating.
    WorkLimitExceeded,
    /// The lowered expressions would exceed the output budget
    /// ([`crate::budget::MAX_NODES`] nodes). Charged before each node and each copy.
    OutputTooLarge,
    /// A recursive call left in a hand-built model. Elaboration unfolds every call of a
    /// recursive `def`, so an elaborated model never has one.
    RecursiveCall,
    /// A refinement no integer satisfies, such as `x > c` at `c = i64::MAX` or bounds
    /// that cross. The domain is empty; it is never widened to make it non-empty.
    EmptyRefinement,
}

impl Unlowerable {
    /// A stable, machine-readable code.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Unlowerable::NonIntegerState => "cml.lower.non_integer_state",
            Unlowerable::UnboundedDomain => "cml.lower.unbounded_domain",
            Unlowerable::NonIntervalRefinement => "cml.lower.non_interval_refinement",
            Unlowerable::ParameterizedAction => "cml.lower.parameterized_action",
            Unlowerable::Constant => "cml.lower.constant",
            Unlowerable::Quantifier => "cml.lower.quantifier",
            Unlowerable::DivisionOrModulo => "cml.lower.division_or_modulo",
            Unlowerable::NonIntegerValue => "cml.lower.non_integer_value",
            Unlowerable::ConditionalValue => "cml.lower.conditional_value",
            Unlowerable::Fairness => "cml.lower.fairness",
            Unlowerable::NonStandardBehavior => "cml.lower.non_standard_behavior",
            Unlowerable::NoInit => "cml.lower.no_init",
            Unlowerable::InitDomainTooLarge => "cml.lower.init_domain_too_large",
            Unlowerable::TooManyInitialStates => "cml.lower.too_many_initial_states",
            Unlowerable::EmptyRefinement => "cml.lower.empty_refinement",
            Unlowerable::RecursiveCall => "cml.lower.recursive_call",
            Unlowerable::OutputTooLarge => "cml.lower.output_too_large",
            Unlowerable::WorkLimitExceeded => "cml.lower.work_limit_exceeded",
            Unlowerable::ExpressionTooDeep => "cml.lower.expression_too_deep",
        }
    }
}

impl fmt::Display for Unlowerable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Unlowerable::NonIntegerState => "state variables must be Int or Nat",
            Unlowerable::UnboundedDomain => {
                "an integer state variable needs a constant lower and upper bound"
            }
            Unlowerable::NonIntervalRefinement => {
                "a refinement must be a constant interval bound on its own variable"
            }
            Unlowerable::ParameterizedAction => "actions with parameters do not lower",
            Unlowerable::Constant => "model constants need a run configuration",
            Unlowerable::Quantifier => "quantifiers and comprehensions do not lower",
            Unlowerable::DivisionOrModulo => "`/` and `%` do not lower",
            Unlowerable::NonIntegerValue => "only integer and boolean values lower",
            Unlowerable::ConditionalValue => "an integer-valued `if` does not lower",
            Unlowerable::Fairness => "fairness assumptions do not lower",
            Unlowerable::NonStandardBehavior => {
                "only `Init && always(step(Next) || stutter(state))` over every action lowers"
            }
            Unlowerable::NoInit => "a model without `init` has no initial states",
            Unlowerable::InitDomainTooLarge => "the state domain is too large to enumerate init",
            Unlowerable::TooManyInitialStates => {
                "the init predicate accepts too many states to lower"
            }
            Unlowerable::EmptyRefinement => "the refinement admits no integer value",
            Unlowerable::RecursiveCall => "a recursive call must be unfolded before lowering",
            Unlowerable::OutputTooLarge => "the lowered model exceeds the output budget",
            Unlowerable::WorkLimitExceeded => "lowering exceeds the work bound",
            Unlowerable::ExpressionTooDeep => "the expression nests deeper than the model accepts",
        })
    }
}

/// Why lowering failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LowerErrorKind {
    /// The model uses something the programmatic model cannot carry.
    Unlowerable(Unlowerable),
    /// The programmatic model builder refused the result.
    Model(ModelError),
    /// Evaluating the init predicate failed (for example, overflow).
    Evaluation(EvalError),
}

/// A source-located lowering error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LowerError {
    /// What went wrong.
    pub kind: LowerErrorKind,
    /// Where: the construct that does not lower, or the model declaration.
    pub span: Span,
}

impl LowerError {
    /// A stable, machine-readable code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match &self.kind {
            LowerErrorKind::Unlowerable(u) => u.code(),
            LowerErrorKind::Model(_) => "cml.lower.model_refused",
            LowerErrorKind::Evaluation(_) => "cml.lower.evaluation",
        }
    }
}

impl fmt::Display for LowerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            LowerErrorKind::Unlowerable(u) => write!(f, "{}: {}: {u}", self.span, self.code()),
            LowerErrorKind::Model(e) => write!(f, "{}: {}: {e}", self.span, self.code()),
            LowerErrorKind::Evaluation(e) => write!(f, "{}: {}: {e}", self.span, self.code()),
        }
    }
}

impl std::error::Error for LowerError {}

type R<T> = Result<T, LowerError>;

fn no<T>(u: Unlowerable, span: Span) -> R<T> {
    Err(LowerError {
        kind: LowerErrorKind::Unlowerable(u),
        span,
    })
}

/// Lower a normalized model to the programmatic model.
///
/// # Errors
///
/// A typed [`Unlowerable`] reason for the first construct that does not lower (checked
/// in a fixed order: fairness, behaviors, state, init, actions, invariants), or the
/// builder's own [`ModelError`].
pub fn lower(model: &NormModel) -> Result<Model, LowerError> {
    lower_with(model, Limits::default()).0
}

/// The output and work budgets of one lowering, and the model's variable count (a
/// variable reference costs a scan of the declared variables in the builder and the
/// evaluator, so it is charged that much work).
struct Meter {
    nodes: Budget,
    fuel: Fuel,
    vars: usize,
}

/// Spend `n` units of work, or refuse with [`Unlowerable::WorkLimitExceeded`].
fn burn(meter: &mut Meter, n: u64, span: Span) -> R<()> {
    meter.fuel.burn(n).map_err(|_| LowerError {
        kind: LowerErrorKind::Unlowerable(Unlowerable::WorkLimitExceeded),
        span,
    })
}

/// [`lower`] under explicit resource limits, reporting what it spent (whether or not
/// it succeeds).
pub fn lower_with(model: &NormModel, limits: Limits) -> (Result<Model, LowerError>, Usage) {
    let mut meter = Meter {
        nodes: Budget::new(limits.nodes),
        fuel: Fuel::new(limits.work),
        vars: model.state.len(),
    };
    let result = lower_metered(model, &mut meter);
    let usage = Usage {
        nodes: meter.nodes.used(),
        work: meter.fuel.used(),
    };
    (result, usage)
}

fn lower_metered(model: &NormModel, meter: &mut Meter) -> Result<Model, LowerError> {
    let whole = model.init.as_ref().map_or(
        Span {
            start: 0,
            end: 0,
            line: 1,
            col: 1,
        },
        |i| i.span,
    );

    // Every expression node this lowering builds or copies is charged to `meter` first.
    let budget = meter;

    if let Some(f) = model.fairness.first() {
        return no(Unlowerable::Fairness, f.span);
    }
    // Which choices offer every action: computed once (linear in the choices), so each
    // behavior is then checked with one lookup, not a comparison against every action.
    let every: Vec<&String> = model.actions.iter().map(|a| &a.name).collect();
    let choice_bytes: usize = model.choices.iter().map(|c| c.actions.len()).sum();
    burn(
        budget,
        (choice_bytes as u64).saturating_add(every.len() as u64),
        whole,
    )?;
    let covering: std::collections::BTreeSet<&str> = model
        .choices
        .iter()
        .filter(|c| c.actions.iter().collect::<Vec<_>>() == every)
        .map(|c| c.name.as_str())
        .collect();
    for b in &model.behaviors {
        burn(
            budget,
            lookup_cost(covering.len().max(model.actions.len()), 32).saturating_add(8),
            b.span,
        )?;
        if !standard_behavior(model, &covering, &b.formula) {
            return no(Unlowerable::NonStandardBehavior, b.span);
        }
    }

    // State variables and their domains.
    let mut builder = ModelBuilder::new();
    let mut variables: Vec<Variable> = Vec::new();
    for v in &model.state {
        // Reading the refinement visits each of its nodes a constant number of times.
        let visits = v.refinement.iter().fold(1_u64, |acc, c| {
            acc.saturating_add(crate::elab::measure(c).0 as u64)
        });
        burn(budget, visits.saturating_mul(2), v.span)?;
        let (lo, hi) = domain_of(v.name.as_str(), &v.ty, &v.refinement, v.span)?;
        builder = builder.variable(&v.name, lo, hi);
        if let (Ok(name), Ok(domain)) = (Ident::new(&v.name), Domain::new(lo, hi)) {
            variables.push(Variable::new(name, domain));
        }
    }

    // Initial states: every state of the domain the init predicate accepts.
    let Some(init) = &model.init else {
        return no(Unlowerable::NoInit, whole);
    };
    let clauses: Vec<Sized<BoolExpr>> = init
        .clauses
        .iter()
        .map(|c| top_bool(c, &mut *budget))
        .collect::<R<Vec<_>>>()?;
    let predicate_size = clauses
        .iter()
        .fold(clauses.len(), |acc, c| acc.saturating_add(c.1.size));
    let init_pred = conjoin(clauses, &mut *budget, init.span)?;
    let cardinality = variables.iter().fold(1_u128, |acc, v| {
        acc.saturating_mul(v.domain().cardinality())
    });
    if cardinality > MAX_INIT_ENUMERATION {
        return no(Unlowerable::InitDomainTooLarge, init.span);
    }
    // The whole enumeration is charged before it starts: every candidate evaluates the
    // predicate once (each node visited once; a variable read scans the declared
    // variables) and steps the value vector. Too much work is refused here, having
    // enumerated nothing.
    burn(
        budget,
        init_work(cardinality, predicate_size, variables.len()),
        init.span,
    )?;
    // A variable whose name or domain the builder refuses is missing from `variables`;
    // `build` below reports it, so the enumeration is skipped rather than run over a
    // partial state.
    if variables.len() == model.state.len() {
        let mut values: Vec<i64> = variables.iter().map(|v| v.domain().lo()).collect();
        let mut bindings_held: usize = 0;
        loop {
            let env = Environment::new(&variables, &values);
            let holds = init_pred.evaluate(&env).map_err(|e| LowerError {
                kind: LowerErrorKind::Evaluation(e),
                span: init.span,
            })?;
            if holds {
                bindings_held = bindings_held.saturating_add(variables.len().max(1));
                if bindings_held > MAX_INIT_BINDINGS {
                    return no(Unlowerable::TooManyInitialStates, init.span);
                }
                // Each accepted state is copied into the builder, then placed and sorted by
                // `ModelBuilder::build` (a scan of the variables per binding): charged
                // before the copy.
                let names: usize = variables.iter().map(|v| v.name().as_str().len()).sum();
                let per_state = (variables.len() as u64)
                    .saturating_mul(variables.len() as u64 + 1)
                    .saturating_add(crate::budget::text_cost(names) as u64)
                    .saturating_add(
                        lookup_cost(bindings_held, 8).saturating_mul(variables.len() as u64),
                    );
                burn(budget, per_state, init.span)?;
                let bindings: Vec<(&str, i64)> = variables
                    .iter()
                    .zip(values.iter())
                    .map(|(v, x)| (v.name().as_str(), *x))
                    .collect();
                builder = builder.initial_state(&bindings);
            }
            if !advance(&mut values, &variables) {
                break;
            }
        }
    }

    // Actions.
    for a in &model.actions {
        if !a.params.is_empty() {
            return no(Unlowerable::ParameterizedAction, a.span);
        }
        let guard = a
            .guard
            .iter()
            .map(|c| top_bool(c, &mut *budget))
            .collect::<R<Vec<_>>>()?;
        let guard = conjoin(guard, &mut *budget, a.span)?;
        let mut updates: Vec<(&str, IntExpr)> = Vec::new();
        for (var, next) in &a.next {
            if let Next::Set(e) = next {
                updates.push((var.as_str(), top_int(e, &mut *budget)?));
            }
        }
        builder = builder.action(ActionDecl::deterministic(&a.name, guard, updates));
    }

    // Invariants become named predicates.
    for i in &model.invariants {
        let body = i
            .clauses
            .iter()
            .map(|c| top_bool(c, &mut *budget))
            .collect::<R<Vec<_>>>()?;
        let body = conjoin(body, &mut *budget, i.span)?;
        builder = builder.predicate(&i.name, body);
    }

    // `ModelBuilder::build` sorts the variables, actions, and predicates by name; the
    // rest of its validation is linear in what was charged above.
    let bytes = |names: &mut dyn Iterator<Item = usize>| names.sum::<usize>();
    let sorting = sort_cost(
        model.state.len(),
        bytes(&mut model.state.iter().map(|v| v.name.len())),
    )
    .saturating_add(sort_cost(
        model.actions.len(),
        bytes(&mut model.actions.iter().map(|a| a.name.len())),
    ))
    .saturating_add(sort_cost(
        model.invariants.len(),
        bytes(&mut model.invariants.iter().map(|i| i.name.len())),
    ));
    burn(budget, sorting, whole)?;
    builder.build().map_err(|e| LowerError {
        kind: LowerErrorKind::Model(e),
        span: whole,
    })
}

/// The work of enumerating `candidates` states against a predicate of `size` nodes over
/// `vars` variables: per candidate, one predicate evaluation (each node once, a variable
/// read scanning up to `vars` variables) and one step of the value vector. Saturates.
pub fn init_work(candidates: u128, size: usize, vars: usize) -> u64 {
    let vars = vars as u128;
    let per = (size as u128)
        .saturating_mul(vars.saturating_add(1))
        .saturating_add(vars.saturating_mul(2))
        .saturating_add(1);
    u64::try_from(candidates.saturating_mul(per)).unwrap_or(u64::MAX)
}

/// Step `values` to the next vector of the domain product, last position fastest.
/// Returns `false` after the last vector.
fn advance(values: &mut [i64], variables: &[Variable]) -> bool {
    for (slot, v) in values.iter_mut().zip(variables.iter()).rev() {
        if *slot < v.domain().hi() {
            *slot = slot.saturating_add(1);
            return true;
        }
        *slot = v.domain().lo();
    }
    false
}

/// `Init && always(step(N) || stutter(state))`, where `Init` is the model's init and `N`
/// offers every action.
fn standard_behavior(
    model: &NormModel,
    covering: &std::collections::BTreeSet<&str>,
    formula: &Expr,
) -> bool {
    let ExprKind::Binary(BinOp::And, init, rest) = &formula.kind else {
        return false;
    };
    let ExprKind::InitRef(init_name) = &init.kind else {
        return false;
    };
    if model.init.as_ref().and_then(|i| i.name.as_ref()) != Some(init_name) {
        return false;
    }
    let ExprKind::Temporal(Temporal::Always, body) = &rest.kind else {
        return false;
    };
    let ExprKind::Binary(BinOp::Or, step, stutter) = &body.kind else {
        return false;
    };
    let (ExprKind::Step(next), ExprKind::Stutter) = (&step.kind, &stutter.kind) else {
        return false;
    };
    covering.contains(next.as_str())
        || (model.actions.len() == 1 && model.actions.first().is_some_and(|a| &a.name == next))
}

fn domain_of(name: &str, ty: &Type, refinement: &[Expr], span: Span) -> R<(i64, i64)> {
    let mut lo: Option<i64> = match ty {
        Type::Nat => Some(0),
        Type::Int => None,
        _ => return no(Unlowerable::NonIntegerState, span),
    };
    let mut hi: Option<i64> = None;
    let raise = |slot: &mut Option<i64>, v: i64| *slot = Some(slot.map_or(v, |x| x.max(v)));
    let lower_to = |slot: &mut Option<i64>, v: i64| *slot = Some(slot.map_or(v, |x| x.min(v)));
    for clause in refinement {
        let is_self = |e: &Expr| matches!(&e.kind, ExprKind::State(n) if n == name);
        let ExprKind::Binary(op, l, r) = &clause.kind else {
            return no(Unlowerable::NonIntervalRefinement, clause.span);
        };
        // Normalize to `v op c`.
        let (op, c) = if is_self(l) {
            (*op, constant(r))
        } else if is_self(r) {
            let flipped = match op {
                BinOp::Le => BinOp::Ge,
                BinOp::Lt => BinOp::Gt,
                BinOp::Ge => BinOp::Le,
                BinOp::Gt => BinOp::Lt,
                other => *other,
            };
            if flipped == BinOp::In {
                return no(Unlowerable::NonIntervalRefinement, clause.span);
            }
            (flipped, constant(l))
        } else {
            return no(Unlowerable::NonIntervalRefinement, clause.span);
        };
        match (op, c) {
            (BinOp::Le, Some(c)) => lower_to(&mut hi, c),
            // `v < c` is `v <= c - 1`; below `i64::MIN` no integer is left.
            (BinOp::Lt, Some(c)) => match c.checked_sub(1) {
                Some(b) => lower_to(&mut hi, b),
                None => return no(Unlowerable::EmptyRefinement, clause.span),
            },
            (BinOp::Ge, Some(c)) => raise(&mut lo, c),
            // `v > c` is `v >= c + 1`; above `i64::MAX` no integer is left.
            (BinOp::Gt, Some(c)) => match c.checked_add(1) {
                Some(b) => raise(&mut lo, b),
                None => return no(Unlowerable::EmptyRefinement, clause.span),
            },
            (BinOp::In, None) => {
                let ExprKind::Binary(BinOp::Range, a, b) = &r.kind else {
                    return no(Unlowerable::NonIntervalRefinement, clause.span);
                };
                match (constant(a), constant(b)) {
                    (Some(a), Some(b)) => {
                        raise(&mut lo, a);
                        lower_to(&mut hi, b);
                    }
                    _ => return no(Unlowerable::NonIntervalRefinement, clause.span),
                }
            }
            _ => return no(Unlowerable::NonIntervalRefinement, clause.span),
        }
    }
    match (lo, hi) {
        (Some(lo), Some(hi)) if lo <= hi => Ok((lo, hi)),
        (Some(_), Some(_)) => no(Unlowerable::EmptyRefinement, span),
        _ => no(Unlowerable::UnboundedDomain, span),
    }
}

/// The value of a closed integer expression over literals, if it has one.
fn constant(e: &Expr) -> Option<i64> {
    match &e.kind {
        ExprKind::Int(n) => Some(*n),
        ExprKind::Neg(a) => constant(a)?.checked_neg(),
        ExprKind::Binary(BinOp::Add, a, b) => constant(a)?.checked_add(constant(b)?),
        ExprKind::Binary(BinOp::Sub, a, b) => constant(a)?.checked_sub(constant(b)?),
        ExprKind::Binary(BinOp::Mul, a, b) => constant(a)?.checked_mul(constant(b)?),
        _ => None,
    }
}

/// Conjoin lowered clauses as a *balanced* tree, so `n` clauses add `⌈log₂ n⌉` levels
/// rather than `n - 1`. The result's depth is computed from the clause measures and
/// checked against [`MAX_EXPR_DEPTH`], and the `&&` nodes are charged, before any node
/// is built. Pairing is adjacent and left to right, so the tree is a function of the
/// clause order alone (and for two or three clauses equals the left fold).
fn conjoin(clauses: Vec<Sized<BoolExpr>>, budget: &mut Meter, span: Span) -> R<BoolExpr> {
    let n = clauses.len();
    if n == 0 {
        return Ok(BoolExpr::Const(true));
    }
    let deepest = clauses.iter().map(|c| c.1.depth).max().unwrap_or(0);
    let mut levels = 0_usize;
    let mut width = n;
    while width > 1 {
        width = width.div_ceil(2);
        levels = levels.saturating_add(1);
    }
    if deepest.saturating_add(levels) > MAX_EXPR_DEPTH {
        return no(Unlowerable::ExpressionTooDeep, span);
    }
    charge(budget, n.saturating_sub(1), span)?;
    burn(budget, n as u64, span)?;
    let mut round: Vec<BoolExpr> = clauses.into_iter().map(|c| c.0).collect();
    while round.len() > 1 {
        let mut next: Vec<BoolExpr> = Vec::with_capacity(round.len().div_ceil(2));
        let mut items = round.into_iter();
        while let Some(left) = items.next() {
            match items.next() {
                Some(right) => next.push(BoolExpr::and(left, right)),
                None => next.push(left),
            }
        }
        round = next;
    }
    Ok(round.pop().unwrap_or(BoolExpr::Const(true)))
}

/// Why a non-integer, non-boolean expression does not lower.
fn reason(e: &Expr) -> Unlowerable {
    match &e.kind {
        ExprKind::Const(_) => Unlowerable::Constant,
        ExprKind::Quant(..) | ExprKind::SetComp(..) | ExprKind::MapComp(..) => {
            Unlowerable::Quantifier
        }
        ExprKind::Param(_) => Unlowerable::ParameterizedAction,
        ExprKind::Binary(BinOp::Div | BinOp::Mod, ..) => Unlowerable::DivisionOrModulo,
        ExprKind::Recur { .. } => Unlowerable::RecursiveCall,
        _ => Unlowerable::NonIntegerValue,
    }
}

/// Refuse an expression too deep to lower, before recursing into it.
///
/// A lowered expression is never shallower than its normalized form less one level (an
/// `in a..b` becomes one range node), so anything deeper than `MAX_EXPR_DEPTH + 1`
/// would be refused by the builder anyway.
fn shallow(e: &Expr, budget: &mut Meter) -> R<()> {
    let (size, depth) = crate::elab::measure(e);
    burn(budget, size as u64, e.span)?;
    if depth > MAX_EXPR_DEPTH.saturating_add(1) {
        return no(Unlowerable::ExpressionTooDeep, e.span);
    }
    Ok(())
}

/// The node count and depth of a lowered expression. Depth is counted the way
/// `continuum_model_core`'s `BoolExpr::depth` and `IntExpr::depth` count it, so a
/// lowered expression the checks here admit is one the model builder admits.
#[derive(Debug, Clone, Copy)]
struct M {
    size: usize,
    depth: usize,
}

impl M {
    /// Owned text: it adds size, not depth.
    const fn text(size: usize) -> Self {
        Self { size, depth: 0 }
    }
}

/// A lowered expression and its measure.
type Sized<T> = (T, M);

/// Charge `n` output nodes, or refuse with [`Unlowerable::OutputTooLarge`].
fn charge(budget: &mut Meter, n: usize, span: Span) -> R<()> {
    budget.nodes.charge(n).map_err(|_| LowerError {
        kind: LowerErrorKind::Unlowerable(Unlowerable::OutputTooLarge),
        span,
    })
}

fn top_bool(e: &Expr, budget: &mut Meter) -> R<Sized<BoolExpr>> {
    shallow(e, budget)?;
    bool_expr(e, budget)
}

fn top_int(e: &Expr, budget: &mut Meter) -> R<IntExpr> {
    shallow(e, budget)?;
    Ok(int_expr(e, budget)?.0)
}

/// One new node over already-lowered children. Its depth is checked against
/// [`MAX_EXPR_DEPTH`] and the node is charged *before* `build` runs, so no lowered
/// expression deeper than the model admits is ever constructed (and none needs a deep
/// recursive drop).
fn node<T>(budget: &mut Meter, span: Span, parts: &[M], build: impl FnOnce() -> T) -> R<Sized<T>> {
    // Building the node, and (for the builder's validation) scanning the declared
    // variables when it names one, is charged as work.
    burn(budget, 1, span)?;
    let depth = parts
        .iter()
        .map(|m| m.depth)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    if depth > MAX_EXPR_DEPTH {
        return no(Unlowerable::ExpressionTooDeep, span);
    }
    charge(budget, 1, span)?;
    let size = parts
        .iter()
        .fold(1_usize, |acc, m| acc.saturating_add(m.size));
    Ok((build(), M { size, depth }))
}

fn int_expr(e: &Expr, budget: &mut Meter) -> R<Sized<IntExpr>> {
    if !e.ty.is_integer() {
        return no(reason(e), e.span);
    }
    let sp = e.span;
    match &e.kind {
        ExprKind::Int(n) => node(budget, sp, &[], || IntExpr::Const(*n)),
        // The name is owned text: its cost travels with the node into every copy.
        ExprKind::State(v) => {
            let text = crate::budget::text_cost(v.len());
            charge(budget, text, sp)?;
            let scan = budget.vars as u64;
            burn(budget, scan, sp)?;
            node(budget, sp, &[M::text(text)], || IntExpr::var(v))
        }
        ExprKind::Neg(a) => {
            let (a, sa) = int_expr(a, budget)?;
            node(budget, sp, &[sa, M { size: 1, depth: 1 }], || {
                IntExpr::minus(IntExpr::Const(0), a)
            })
        }
        ExprKind::Binary(op @ (BinOp::Add | BinOp::Sub | BinOp::Mul), a, b) => {
            let (a, sa) = int_expr(a, budget)?;
            let (b, sb) = int_expr(b, budget)?;
            let built = match op {
                BinOp::Add => IntExpr::plus(a, b),
                BinOp::Sub => IntExpr::minus(a, b),
                _ => IntExpr::times(a, b),
            };
            node(budget, sp, &[sa, sb], || built)
        }
        ExprKind::Builtin(Builtin::Min, args) | ExprKind::Builtin(Builtin::Max, args) => {
            let [a, b] = args.as_slice() else {
                return no(Unlowerable::NonIntegerValue, e.span);
            };
            let (a, sa) = int_expr(a, budget)?;
            let (b, sb) = int_expr(b, budget)?;
            let built = if matches!(e.kind, ExprKind::Builtin(Builtin::Min, _)) {
                IntExpr::min(a, b)
            } else {
                IntExpr::max(a, b)
            };
            node(budget, sp, &[sa, sb], || built)
        }
        ExprKind::If(..) => no(Unlowerable::ConditionalValue, e.span),
        _ => no(reason(e), e.span),
    }
}

/// Clone a lowered subtree after charging its size: the only way this module copies.
fn copy<T: Clone>(budget: &mut Meter, span: Span, x: &Sized<T>) -> R<Sized<T>> {
    charge(budget, x.1.size, span)?;
    burn(budget, x.1.size as u64, span)?;
    Ok((x.0.clone(), x.1))
}

fn bool_expr(e: &Expr, budget: &mut Meter) -> R<Sized<BoolExpr>> {
    if e.ty != Type::Bool {
        return no(reason(e), e.span);
    }
    let sp = e.span;
    match &e.kind {
        ExprKind::Bool(b) => node(budget, sp, &[], || BoolExpr::Const(*b)),
        ExprKind::Not(a) => {
            let (a, sa) = bool_expr(a, budget)?;
            node(budget, sp, &[sa], || BoolExpr::negate(a))
        }
        ExprKind::If(c, a, b) => {
            // `(c && a) || (!c && b)` mentions `c` twice: the second copy is charged.
            let c = bool_expr(c, budget)?;
            let (c2, sc2) = copy(budget, sp, &c)?;
            let (a, sa) = bool_expr(a, budget)?;
            let (b, sb) = bool_expr(b, budget)?;
            let (left, sl) = node(budget, sp, &[c.1, sa], || BoolExpr::and(c.0, a))?;
            let (nc, snc) = node(budget, sp, &[sc2], || BoolExpr::negate(c2))?;
            let (right, sr) = node(budget, sp, &[snc, sb], || BoolExpr::and(nc, b))?;
            node(budget, sp, &[sl, sr], || BoolExpr::or(left, right))
        }
        ExprKind::Binary(op, a, b) => {
            let cmp = |op: CmpOp, budget: &mut Meter| -> R<Sized<BoolExpr>> {
                let (x, sx) = int_expr(a, budget)?;
                let (y, sy) = int_expr(b, budget)?;
                node(budget, sp, &[sx, sy], || BoolExpr::compare(op, x, y))
            };
            match op {
                BinOp::And | BinOp::Or | BinOp::Implies => {
                    let (x, sx) = bool_expr(a, budget)?;
                    let (y, sy) = bool_expr(b, budget)?;
                    let built = match op {
                        BinOp::And => BoolExpr::and(x, y),
                        BinOp::Or => BoolExpr::or(x, y),
                        _ => BoolExpr::implies(x, y),
                    };
                    node(budget, sp, &[sx, sy], || built)
                }
                BinOp::Iff => {
                    let x = bool_expr(a, budget)?;
                    let y = bool_expr(b, budget)?;
                    iff(budget, sp, x, y)
                }
                BinOp::Eq | BinOp::Ne if a.ty == Type::Bool => {
                    let x = bool_expr(a, budget)?;
                    let y = bool_expr(b, budget)?;
                    let same = iff(budget, sp, x, y)?;
                    if *op == BinOp::Eq {
                        Ok(same)
                    } else {
                        node(budget, sp, &[same.1], || BoolExpr::negate(same.0))
                    }
                }
                BinOp::Eq => cmp(CmpOp::Eq, budget),
                BinOp::Ne => cmp(CmpOp::Ne, budget),
                BinOp::Lt => cmp(CmpOp::Lt, budget),
                BinOp::Le => cmp(CmpOp::Le, budget),
                BinOp::Gt => cmp(CmpOp::Gt, budget),
                BinOp::Ge => cmp(CmpOp::Ge, budget),
                BinOp::In | BinOp::NotIn => {
                    let ExprKind::Binary(BinOp::Range, lo, hi) = &b.kind else {
                        return no(reason(b), b.span);
                    };
                    let x = int_expr(a, budget)?;
                    let inside = match (constant(lo), constant(hi)) {
                        (Some(lo), Some(hi)) => {
                            node(budget, sp, &[x.1], || BoolExpr::in_range(x.0, lo, hi))?
                        }
                        _ => {
                            // `lo <= x && x <= hi` mentions `x` twice: the copy is charged.
                            let (x2, sx2) = copy(budget, sp, &x)?;
                            let (l, sl) = int_expr(lo, budget)?;
                            let (h, sh) = int_expr(hi, budget)?;
                            let (ge, sge) = node(budget, sp, &[x.1, sl], || {
                                BoolExpr::compare(CmpOp::Ge, x.0, l)
                            })?;
                            let (le, sle) = node(budget, sp, &[sx2, sh], || {
                                BoolExpr::compare(CmpOp::Le, x2, h)
                            })?;
                            node(budget, sp, &[sge, sle], || BoolExpr::and(ge, le))?
                        }
                    };
                    if *op == BinOp::In {
                        Ok(inside)
                    } else {
                        node(budget, sp, &[inside.1], || BoolExpr::negate(inside.0))
                    }
                }
                _ => no(reason(e), e.span),
            }
        }
        _ => no(reason(e), e.span),
    }
}

/// `a <=> b` as `(a => b) && (b => a)`. The expression language has no equivalence
/// and no sharing, so each operand appears twice; the second copy of each is charged
/// to the budget *before* it is made. A nest of `k` equivalences doubles per level, so
/// it is refused as [`Unlowerable::OutputTooLarge`] as soon as the next copy would not
/// fit, having allocated at most the budget.
fn iff(
    budget: &mut Meter,
    span: Span,
    a: Sized<BoolExpr>,
    b: Sized<BoolExpr>,
) -> R<Sized<BoolExpr>> {
    let a2 = copy(budget, span, &a)?;
    let b2 = copy(budget, span, &b)?;
    let (ab, sab) = node(budget, span, &[a.1, b.1], || BoolExpr::implies(a.0, b.0))?;
    let (ba, sba) = node(budget, span, &[b2.1, a2.1], || {
        BoolExpr::implies(b2.0, a2.0)
    })?;
    node(budget, span, &[sab, sba], || BoolExpr::and(ab, ba))
}
