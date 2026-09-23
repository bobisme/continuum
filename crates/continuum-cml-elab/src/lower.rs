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
//! - every state variable of an enumeration ranges over its variant indices;
//! - under a run configuration, every state variable of a finite composite type
//!   (`Option`, tuple, `Set`, `Map` over finite types) lowers to the slots of its flat
//!   layout, named by path (`m[k]`, `s{u}`, `o?`, `o!`, `t.0`), and every expression of
//!   such a type to the list of its slot expressions, with static keys, and map reads
//!   recorded as definedness predicates `A#defined` and `I#defined` (RFC 0003
//!   correction 4, "Layout" to "Definedness"; the `collections` module);
//! - every guard, update, and invariant uses only integer arithmetic (`+ - *`, unary
//!   `-`, `min`, `max`), comparisons, `in a..b`, membership in a set literal or a
//!   configuration constant set of scalars, the boolean connectives, and `forall` /
//!   `exists` over a finite scalar domain, expanded (RFC 0003 correction 4, "Quantifier
//!   expansion");
//! - an action's parameters have finite types; it lowers to one action per parameter
//!   tuple, named `A(p=u,…)` (correction 4, "Action schemas");
//! - every expression is planned before it is built: see [`Mode`] and `begin_site`;
//! - a relational action (RFC 0003 "Relational actions") has at most
//!   [`MAX_RELATIONAL_CANDIDATES`] candidate post-states; it lowers to one action per
//!   candidate, named `A[r=c,…]`, whose guard includes the postconditions at that
//!   candidate;
//! - the init predicate is satisfied by at least one state of the (finite) domain;
//! - every behavior is the standard specification `Init && always(step(N) ||
//!   stutter(state))`, where `N` offers every action — which is exactly what the
//!   programmatic model means — and there is no fairness assumption, which the
//!   programmatic model cannot carry.
//!
//! Anything else is a typed [`Unlowerable`] reason, never an approximation: an unbounded
//! `Nat` is not silently bounded, a quantifier is unrolled only over a finite domain it
//! enumerates exactly (never a truncated one), and a fairness assumption is not silently
//! dropped.
//!
//! # Under a run configuration
//!
//! [`lower_configured`] takes a [`RunConfig`] explicitly (RFC 0003 "Run configuration
//! (normative)"). It checks every binding first, then lowers as above with three
//! additions: a state variable of an instantiated sort is an integer in `0..size`, a
//! constant bound to an integer, a Boolean, or a sort element is that literal, and
//! sort values compare as integers. It returns the model with its configuration
//! identity and one [`RunIdentity`] for the pair; [`Model::identity`] itself is left
//! the transition system's alone.
//!
//! A configuration may also bound `Nat` and `Int` explicitly (RFC 0003 correction 4,
//! the schema's `bounds`). A `Nat` state variable then ranges over `0..=max` and an
//! `Int` one over `min..=max`, each intersected with its refinement; an empty
//! intersection is [`Unlowerable::EmptyRefinement`]. Under a configuration, an integer
//! state variable with neither a finite refinement nor a bound is
//! [`Unlowerable::UnboundedType`]; a configuration constant outside the
//! configuration's own bound is [`ConfigRefusalKind::OutsideBound`].
//!
//! # Refinements are domains
//!
//! `big: Nat where big <= 5` becomes the declared domain `0..=5`. An update that leaves
//! the domain is then [`continuum_model_core::EvaluationError::UpdateOutOfDomain`] at
//! exploration time — the refinement type is enforced where the value lands (docs/16
//! PO-MOD-003), and is never clamped.

use std::borrow::Borrow;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

use continuum_cml_syntax::Span;
use continuum_model_core::domain::{Domain, Variable};
use continuum_model_core::expr::{Environment, MAX_EXPR_DEPTH};
use continuum_model_core::ident::MAX_IDENT_BYTES;
use continuum_model_core::model::{MAX_ACTIONS, MAX_INITIAL_STATES, MAX_VARIABLES};
use continuum_model_core::{
    ActionDecl, ArithOp, BoolExpr, CmpOp, EvalError, Ident, IntExpr, Model, ModelBuilder,
    ModelError,
};

use crate::budget::{Budget, Fuel, Limits, Usage, lookup_cost, sort_cost};

mod collections;
mod layout;

use crate::config::{ConfigIdentity, ConfigValue, RunConfig, RunIdentity};
use crate::norm::{BinOp, Builtin, Expr, ExprKind, Next, NormModel, Temporal};
use crate::types::Type;
pub use collections::{DEFINED_SUFFIX, definedness_subject};

/// The largest state domain whose init predicate the lowering enumerates.
///
/// This bounds the *time* of the enumeration: each candidate state is one evaluation
/// over one reused value vector, with no allocation.
pub const MAX_INIT_ENUMERATION: u128 = 1 << 22;

/// The most candidate post-states one relational action may enumerate: the product of
/// its relational variables' domains. Each candidate becomes one programmatic action.
///
/// It equals `continuum_model_core`'s `MAX_ACTIONS`, the most actions a whole model may
/// have, so one action's expansion alone never passes it; the total over all actions
/// is checked separately, before any action is built ([`Unlowerable::TooManyActions`]).
pub const MAX_RELATIONAL_CANDIDATES: u128 = MAX_ACTIONS as u128;

/// The most variable bindings (initial states times state variables) the lowering hands
/// to the builder.
///
/// This bounds the *memory* of the enumeration. Every accepted initial state becomes a
/// list of owned `(name, value)` bindings in [`ModelBuilder`], and the builder refuses a
/// list that is too long only after it holds all of it. The count is checked before each
/// state is added, so a model whose init accepts too many states is refused at a bounded
/// allocation (INV-016: source is untrusted). Each accepted state is also charged to the
/// output budget (a record, and per binding a node and its name) before it is copied, so
/// under the default budget that limit is reached first (cr-3aqchd).
pub const MAX_INIT_BINDINGS: usize = 1 << 20;

// Every accepted initial state holds at least one binding, so the binding bound also
// keeps the initial-state count within the model's own limit.
const _: () = assert!(MAX_INIT_BINDINGS <= MAX_INITIAL_STATES);

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
    /// A relational action whose candidate post-states (the product of its relational
    /// variables' domains) number more than [`MAX_RELATIONAL_CANDIDATES`].
    SuccessorDomainTooLarge,
    /// More state variables than `continuum_model_core`'s `MAX_VARIABLES`. Checked
    /// before any state is enumerated.
    TooManyVariables,
    /// More programmatic actions than `continuum_model_core`'s `MAX_ACTIONS`, counting
    /// one per candidate of every relational action. Checked before any action is built.
    TooManyActions,
    /// A name longer than `continuum_model_core`'s `MAX_IDENT_BYTES`: a declared state
    /// variable, action, or invariant, or the longest name a relational action's
    /// candidates generate (`A[x=-5,y=12]`). Checked before any action is built.
    NameTooLong,
    /// A post-state read (`x'`) outside an action's postconditions, in a hand-built
    /// model. Elaboration never produces one.
    PrimedOutsidePostcondition,
    /// A quantifier over a domain that reads state, whose body might overflow at a
    /// candidate outside the domain (RFC 0003 correction 4). Such an instance is
    /// guarded, and the model core evaluates the body even where the guard is false, so
    /// the lowering admits it only when interval arithmetic shows the body cannot fail
    /// anywhere on the candidates; otherwise it refuses rather than add an error the
    /// model does not have.
    GuardedOverflow,
    /// A refinement no integer satisfies, such as `x > c` at `c = i64::MAX` or bounds
    /// that cross. The domain is empty; it is never widened to make it non-empty.
    /// Under a run configuration this includes a refinement that no value of the
    /// configured `Nat` or `Int` bound satisfies.
    EmptyRefinement,
    /// Under a run configuration, a `Nat` or `Int` that must be finite has neither a
    /// finite refinement nor a configuration bound (RFC 0003 correction 4). The
    /// configuration can bound the type; the lowering never bounds it silently
    /// (ADR-0025). Without a configuration the same state variable is
    /// [`Unlowerable::UnboundedDomain`].
    UnboundedType,
    /// A map key, a set member tested with `in` or written in a literal, or an index
    /// that reads state (RFC 0003 correction 4, "Static keys", bn-23hzh). A static key
    /// selects one slot at lowering time; a key that reads state would select one at
    /// exploration time, which the flat layout cannot express.
    DynamicKey,
    /// A static key, member, or payload whose value is outside the instantiated
    /// universe of its type, or whose evaluation fails (bn-23hzh).
    ValueOutsideBound,
    /// A computed value placed in a slot of a collection (an option payload, a map
    /// value, a tuple component) that the lowering cannot show safe: a Boolean that
    /// reads state, an integer without an interval that fits `i64`, or an option payload
    /// that may lie below its universe, whose code would then collide with `None`
    /// (bn-23hzh). Every slot expression the lowering builds cannot fail, so a slot may
    /// be selected away without dropping an error the model has.
    DynamicValue,
    /// A map literal or map comprehension that gives one key two entries (bn-23hzh).
    DuplicateKey,
    /// A read `m[k]` in the init predicate that is undefined (`k` is not a key of `m`)
    /// at a candidate initial state (bn-23hzh). The programmatic model has no way to
    /// carry an error in its initial states, so the lowering refuses.
    UndefinedRead,
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
            Unlowerable::UnboundedType => "cml.lower.unbounded_type",
            Unlowerable::GuardedOverflow => "cml.lower.guarded_overflow",
            Unlowerable::RecursiveCall => "cml.lower.recursive_call",
            Unlowerable::SuccessorDomainTooLarge => "cml.lower.successor_domain_too_large",
            Unlowerable::TooManyVariables => "cml.lower.too_many_variables",
            Unlowerable::TooManyActions => "cml.lower.too_many_actions",
            Unlowerable::NameTooLong => "cml.lower.name_too_long",
            Unlowerable::PrimedOutsidePostcondition => "cml.lower.primed_outside_postcondition",
            Unlowerable::OutputTooLarge => "cml.lower.output_too_large",
            Unlowerable::WorkLimitExceeded => "cml.lower.work_limit_exceeded",
            Unlowerable::ExpressionTooDeep => "cml.lower.expression_too_deep",
            Unlowerable::DynamicKey => "cml.lower.dynamic_key",
            Unlowerable::ValueOutsideBound => "cml.lower.value_outside_bound",
            Unlowerable::DynamicValue => "cml.lower.dynamic_value",
            Unlowerable::DuplicateKey => "cml.lower.duplicate_key",
            Unlowerable::UndefinedRead => "cml.lower.undefined_read",
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
            Unlowerable::UnboundedType => {
                "an unbounded integer type needs a bound from the run configuration"
            }
            Unlowerable::GuardedOverflow => {
                "a quantifier over a state-dependent domain has a body that may overflow"
            }
            Unlowerable::RecursiveCall => "a recursive call must be unfolded before lowering",
            Unlowerable::SuccessorDomainTooLarge => {
                "a relational action has too many candidate post-states to enumerate"
            }
            Unlowerable::TooManyVariables => "the model has more state variables than a model may",
            Unlowerable::TooManyActions => {
                "the lowered model would have more actions than a model may"
            }
            Unlowerable::NameTooLong => "a lowered name is longer than a model name may be",
            Unlowerable::PrimedOutsidePostcondition => {
                "a post-state read is allowed only in an action's postconditions"
            }
            Unlowerable::OutputTooLarge => "the lowered model exceeds the output budget",
            Unlowerable::WorkLimitExceeded => "lowering exceeds the work bound",
            Unlowerable::ExpressionTooDeep => "the expression nests deeper than the model accepts",
            Unlowerable::DynamicKey => "a map key, set member, or index must not read state",
            Unlowerable::ValueOutsideBound => {
                "a static value is outside the universe of its type, or fails to evaluate"
            }
            Unlowerable::DynamicValue => {
                "a computed value in a collection slot may fail or collide with `None`"
            }
            Unlowerable::DuplicateKey => "a map literal gives one key two entries",
            Unlowerable::UndefinedRead => "the init predicate reads a key that a map lacks",
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
    /// The run configuration does not bind this model (RFC 0003 "Configurations and
    /// bounds"): a missing, extra, or ill-typed binding, or a configuration written for
    /// another model.
    Configuration(ConfigRefusal),
}

/// A binding a run configuration gets wrong, and the name it concerns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigRefusal {
    /// What is wrong.
    pub kind: ConfigRefusalKind,
    /// The sort, constant, or (for [`ConfigRefusalKind::OtherModel`]) model name.
    pub name: String,
}

/// The kinds of [`ConfigRefusal`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConfigRefusalKind {
    /// The configuration names another model.
    OtherModel,
    /// A sort the model declares has no instantiation.
    MissingSort,
    /// An instantiated sort the model does not declare.
    ExtraSort,
    /// A constant the model declares has no value.
    MissingConstant,
    /// A value for a constant the model does not declare.
    ExtraConstant,
    /// A value that does not have its constant's declared type: a wrong tag, an element
    /// of another sort or not in it, an unknown variant, a negative `Nat`, a record with
    /// other fields, or a tuple of another width.
    IllTyped,
    /// A constant of a function type, which the configuration has no value form for.
    UnbindableType,
    /// An integer value (of a constant, or inside one) outside the configuration's own
    /// `Nat` or `Int` bound (RFC 0003 correction 4).
    OutsideBound,
}

impl ConfigRefusalKind {
    /// A stable, machine-readable code.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::OtherModel => "cml.config.other_model",
            Self::MissingSort => "cml.config.missing_sort",
            Self::ExtraSort => "cml.config.extra_sort",
            Self::MissingConstant => "cml.config.missing_constant",
            Self::ExtraConstant => "cml.config.extra_constant",
            Self::IllTyped => "cml.config.ill_typed",
            Self::UnbindableType => "cml.config.unbindable_type",
            Self::OutsideBound => "cml.config.outside_bound",
        }
    }
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
            LowerErrorKind::Configuration(r) => r.kind.code(),
        }
    }
}

impl fmt::Display for LowerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            LowerErrorKind::Unlowerable(u) => write!(f, "{}: {}: {u}", self.span, self.code()),
            LowerErrorKind::Model(e) => write!(f, "{}: {}: {e}", self.span, self.code()),
            LowerErrorKind::Evaluation(e) => write!(f, "{}: {}: {e}", self.span, self.code()),
            LowerErrorKind::Configuration(r) => {
                write!(f, "{}: {}: `{}`", self.span, self.code(), r.name)
            }
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
    /// Whether an action's postconditions are being lowered: only there does a primed
    /// read (`x'`) lower, to the placeholder variable [`primed_placeholder`].
    post: bool,
    /// The slot of each relational variable of the action being lowered, so a primed
    /// read lowers to an indexed placeholder (`'3`), resolved once here and substituted
    /// in constant time per candidate.
    slots: std::collections::BTreeMap<String, usize>,
    /// What a run configuration binds; empty for [`lower`].
    bind: Bindings,
    /// The model's enumerations, by name (built for every lowering).
    enums: BTreeMap<String, EnumTable>,
    /// Each lowered state variable's domain, for the intervals of quantifier domains.
    domains: BTreeMap<String, (i64, i64)>,
    /// Action parameters and quantifier binders in scope: a point while building, an
    /// interval while planning (see [`Mode`]).
    env: Env,
    /// Output nodes the plan of the site being built has already charged.
    prepaid: usize,
    /// Whether a planned site is being built.
    in_site: bool,
    /// Whether that build needed more output than its plan (a planning defect).
    overdrawn: bool,
    /// The work a plan predicts its build will spend.
    predicted: u128,
    /// How many guarded quantifier instances enclose the expression being planned.
    guarded: usize,
    /// How many quantifier binders enclose the expression being lowered.
    binder_depth: usize,
    /// The flat layout of each composite state variable (bn-23hzh), shared so a slot
    /// name is borrowed, not copied, while the meter is charged.
    layout: Rc<collections::StateLayout>,
    /// The definedness conditions of the expression being built (bn-23hzh): one item per
    /// map read `m[k]`, and one per guarded quantifier instance whose body has any.
    defs: Vec<Sized<BoolExpr>>,
    /// Their plan, while planning: an aggregate, multiplied at each fan-out as the
    /// predicted work is.
    defs_plan: collections::DAgg,
    /// While planning: the output nodes the build will charge beyond the measures the
    /// plan returns — static values lowered to be evaluated, and slots selected away —
    /// multiplied at each fan-out as the predicted work is (bn-23hzh).
    scratch: u128,
    /// The work spent when the site being built opened, so its close can check that the
    /// build spent no more than its plan predicted (a planning defect otherwise).
    site_work: u64,
    /// That prediction.
    site_predicted: u128,
}

impl Meter {
    fn new(limits: Limits, model: &NormModel) -> Self {
        Self {
            nodes: Budget::new(limits.nodes),
            fuel: Fuel::new(limits.work),
            vars: model.state.len(),
            post: false,
            slots: BTreeMap::new(),
            bind: Bindings::default(),
            enums: BTreeMap::new(),
            domains: BTreeMap::new(),
            env: Env::default(),
            prepaid: 0,
            in_site: false,
            overdrawn: false,
            predicted: 0,
            guarded: 0,
            binder_depth: 0,
            layout: Rc::default(),
            defs: Vec::new(),
            defs_plan: collections::DAgg::default(),
            scratch: 0,
            site_work: 0,
            site_predicted: 0,
        }
    }
}

/// An enumeration's variants: by name to their index (declaration order), and in
/// order, with the longest name's length (for generated action names).
#[derive(Debug, Default)]
struct EnumTable {
    index: BTreeMap<String, i64>,
    names: Vec<String>,
    widest: usize,
}

/// Action parameters and bound variables in scope, each with an interval (a point
/// while building). A binder number repeats where the same def is inlined inside its
/// own argument; the inner binder then shadows the outer one lexically, so entering a
/// scope saves the entry it shadows and leaving it restores that entry.
#[derive(Debug, Default)]
struct Env {
    params: BTreeMap<String, (i64, i64)>,
    binders: BTreeMap<u32, (i64, i64)>,
}

/// The enumeration tables: every variant charged (a node and its text) and the sort of
/// each table charged before it is built.
fn enum_tables(model: &NormModel, budget: &mut Meter) -> R<()> {
    let whole = whole_span(model);
    for e in &model.enums {
        let text: usize = e.variants.iter().map(String::len).sum();
        let cost = e
            .variants
            .len()
            .saturating_mul(2)
            .saturating_add(crate::budget::text_cost(text).saturating_mul(2))
            .saturating_add(1 + crate::budget::text_cost(e.name.len()));
        charge(budget, cost, whole)?;
        burn(
            budget,
            sort_cost(e.variants.len(), text)
                .saturating_add(lookup_cost(model.enums.len(), e.name.len())),
            whole,
        )?;
        let table = EnumTable {
            index: e
                .variants
                .iter()
                .enumerate()
                .map(|(i, v)| (v.clone(), i as i64))
                .collect(),
            names: e.variants.clone(),
            widest: e.variants.iter().map(|v| escaped_len(v)).max().unwrap_or(0),
        };
        budget.enums.insert(e.name.clone(), table);
    }
    Ok(())
}

/// The bindings of a run configuration, as the lowering reads them: each
/// instantiated sort's size (its elements are the integers `0..size`) and element
/// names, each constant whose value is an integer, a Boolean, a sort element, or a
/// variant (its index), and each constant set of such values (its codes, ascending).
#[derive(Debug, Default)]
struct Bindings {
    configured: bool,
    sorts: BTreeMap<String, i64>,
    /// Each sort's element names, for generated action names, and the longest's length.
    sort_names: BTreeMap<String, (Vec<String>, usize)>,
    ints: BTreeMap<String, i64>,
    bools: BTreeMap<String, bool>,
    /// Shared, never copied: a use takes a reference count, not the members.
    sets: BTreeMap<String, Rc<[i64]>>,
    /// The configuration's explicit `Nat` and `Int` bounds (correction 4).
    bounds: crate::config::Bounds,
    /// Each constant of a finite composite type other than a set whose universe fits
    /// `i64`: the index of its value (bn-23hzh).
    atoms: BTreeMap<String, i64>,
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
    let mut meter = Meter::new(limits, model);
    let result = enum_tables(model, &mut meter).and_then(|()| lower_metered(model, &mut meter));
    let usage = Usage {
        nodes: meter.nodes.used(),
        work: meter.fuel.used(),
    };
    (result, usage)
}

/// A model lowered under a run configuration, with its identities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Configured {
    model: Model,
    run: RunIdentity,
}

impl Configured {
    /// The lowered model. Its [`Model::identity`] is that of the transition system alone,
    /// so it equals a programmatic model built with the same values.
    #[must_use]
    pub fn model(&self) -> &Model {
        &self.model
    }

    /// The configuration's content identity (shared with the [`RunConfig`]).
    #[must_use]
    pub fn config_identity(&self) -> &ConfigIdentity {
        self.run.config()
    }

    /// The one handle for this model under this configuration: two configurations always
    /// give two run identities.
    #[must_use]
    pub fn run_identity(&self) -> &RunIdentity {
        &self.run
    }
}

thread_local! {
    /// How many model identities `lower_configured` has encoded on this thread: the test
    /// seam that shows a refusal came before the encoding.
    static IDENTITY_ENCODINGS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// How many model identities [`lower_configured`] has encoded on the calling thread.
/// A test reads it before and after a refused lowering to show the identity was never
/// built.
#[must_use]
pub fn identity_encodings_on_this_thread() -> u64 {
    IDENTITY_ENCODINGS.with(std::cell::Cell::get)
}

/// Lower `model` under the run configuration `config`, given explicitly (RFC 0003
/// "Configurations and bounds"; it is never read from the environment).
///
/// The bindings are checked first, in a fixed order — the model name; the sorts,
/// missing then extra; the constants, missing then extra; then each constant's value
/// against its declared type, in name order — and the first defect is a typed
/// [`ConfigRefusal`]. Every sort element and every value node is charged to the output
/// and work budgets before its table is built. An instantiated sort lowers a state
/// variable of that sort to the integers `0..size`, with its elements in configuration
/// order; a constant whose value is an integer, a Boolean, or a sort element lowers to
/// that literal. Everything else lowers exactly as [`lower`] does.
pub fn lower_configured(
    model: &NormModel,
    config: &RunConfig,
    limits: Limits,
) -> (Result<Configured, LowerError>, Usage) {
    let mut meter = Meter::new(limits, model);
    let result = enum_tables(model, &mut meter)
        .and_then(|()| bindings(model, config, &mut meter))
        .and_then(|bind| {
            meter.bind = bind;
            let lowered = lower_metered(model, &mut meter)?;
            // The model identity's buffers are the only new allocations here. Before any is
            // made: the walks are charged — `identity_alloc_bound` visits the model twice,
            // and `Model::identity` visits it three times (its two capacity walks and the
            // encoding) — each visit covering every expression node (every one of which the
            // lowering charged) and each initial-state value; then the peak allocation
            // itself (`identity_alloc_bound`: the output buffer, the one reused outcome
            // scratch buffer, and its range list, each allocated once at that capacity) is
            // charged as output and as work. Only then is the identity encoded, and its
            // length never exceeds the bound. The configuration identity is shared, not
            // copied, and the run identity holds the two by value.
            let at = whole_span(model);
            let walk = (meter.nodes.used() as u64)
                .saturating_add(
                    (lowered.initial_states().len() as u64)
                        .saturating_mul(lowered.arity() as u64 + 1),
                )
                .saturating_add(lowered.variables().len() as u64)
                .saturating_add(lowered.actions().len() as u64)
                .saturating_add(lowered.predicates().len() as u64);
            burn(&mut meter, walk.saturating_mul(5), at)?;
            let bound = lowered.identity_alloc_bound();
            charge(&mut meter, crate::budget::text_cost(bound), at)?;
            burn(&mut meter, bound as u64, at)?;
            IDENTITY_ENCODINGS.with(|n| n.set(n.get().saturating_add(1)));
            let identity = lowered.identity();
            debug_assert!(identity.as_bytes().len() <= bound);
            let run = RunIdentity::of(identity, config.shared_identity());
            Ok(Configured {
                model: lowered,
                run,
            })
        });
    let usage = Usage {
        nodes: meter.nodes.used(),
        work: meter.fuel.used(),
    };
    (result, usage)
}

fn whole_span(model: &NormModel) -> Span {
    model.init.as_ref().map_or(
        Span {
            start: 0,
            end: 0,
            line: 1,
            col: 1,
        },
        |i| i.span,
    )
}

fn refuse<T>(kind: ConfigRefusalKind, name: &str, span: Span) -> R<T> {
    Err(LowerError {
        kind: LowerErrorKind::Configuration(ConfigRefusal {
            kind,
            name: name.to_owned(),
        }),
        span,
    })
}

/// Check `config` against `model` and collect the scalar bindings (see
/// [`lower_configured`] for the order).
fn bindings(model: &NormModel, config: &RunConfig, budget: &mut Meter) -> R<Bindings> {
    let whole = whole_span(model);
    if config.model() != model.name {
        return refuse(ConfigRefusalKind::OtherModel, config.model(), whole);
    }
    // The declared-name index is built after its charge (an entry per sort, and a sort
    // over the names' bytes).
    let sort_bytes: usize = model.sorts.iter().map(String::len).sum();
    charge(budget, model.sorts.len(), whole)?;
    burn(budget, sort_cost(model.sorts.len(), sort_bytes), whole)?;
    let declared: std::collections::BTreeSet<&str> =
        model.sorts.iter().map(String::as_str).collect();
    // One lookup per name each way, charged by each name's own length.
    let entries = declared.len().max(config.sorts().len());
    let cost = model
        .sorts
        .iter()
        .chain(config.sorts().keys())
        .fold(0_u64, |acc, n| {
            acc.saturating_add(lookup_cost(entries, n.len()))
        });
    burn(budget, cost, whole)?;
    for s in &model.sorts {
        if !config.sorts().contains_key(s) {
            return refuse(ConfigRefusalKind::MissingSort, s, whole);
        }
    }
    if let Some(extra) = config
        .sorts()
        .keys()
        .find(|s| !declared.contains(s.as_str()))
    {
        return refuse(ConfigRefusalKind::ExtraSort, extra, whole);
    }
    // Every element is charged (a node and its text) before the element tables exist,
    // and building each table — a sort of its names, comparing name bytes — is charged
    // before it is built. Each table maps a name to its index, so a lookup is
    // logarithmic in the sort and linear in the name, never a scan of the elements.
    let mut tables = Tables {
        bounds: *config.bounds(),
        ..Tables::default()
    };
    for (sort, names) in config.sorts() {
        let text: usize = names.iter().map(String::len).sum();
        let cost = names.len().saturating_add(crate::budget::text_cost(text));
        charge(budget, cost, whole)?;
        burn(budget, sort_cost(names.len(), text), whole)?;
        tables.elements.insert(
            sort,
            names
                .iter()
                .enumerate()
                .map(|(i, n)| (n.as_str(), i))
                .collect(),
        );
    }
    for e in &model.enums {
        let text: usize = e.variants.iter().map(String::len).sum();
        let cost = e
            .variants
            .len()
            .saturating_add(crate::budget::text_cost(text));
        charge(budget, cost, whole)?;
        burn(budget, sort_cost(e.variants.len(), text), whole)?;
        tables
            .variants
            .insert(&e.name, e.variants.iter().map(String::as_str).collect());
    }

    let const_bytes: usize = model.constants.iter().map(|c| c.name.len()).sum();
    charge(budget, model.constants.len(), whole)?;
    burn(budget, sort_cost(model.constants.len(), const_bytes), whole)?;
    let constants: std::collections::BTreeMap<&str, &crate::norm::ConstDecl> = model
        .constants
        .iter()
        .map(|c| (c.name.as_str(), c))
        .collect();
    let entries = constants.len().max(config.constants().len());
    let cost = model
        .constants
        .iter()
        .map(|c| &c.name)
        .chain(config.constants().keys())
        .fold(0_u64, |acc, n| {
            acc.saturating_add(lookup_cost(entries, n.len()))
        });
    burn(budget, cost, whole)?;
    for c in &model.constants {
        if !config.constants().contains_key(&c.name) {
            return refuse(ConfigRefusalKind::MissingConstant, &c.name, c.span);
        }
    }
    if let Some(extra) = config
        .constants()
        .keys()
        .find(|c| !constants.contains_key(c.as_str()))
    {
        return refuse(ConfigRefusalKind::ExtraConstant, extra, whole);
    }

    let mut bind = Bindings {
        configured: true,
        bounds: *config.bounds(),
        ..Bindings::default()
    };
    for (sort, names) in config.sorts() {
        // A binding owns its name: charged, with its insertion, before it is made.
        charge(budget, 1 + crate::budget::text_cost(sort.len()), whole)?;
        burn(budget, lookup_cost(bind.sorts.len(), sort.len()), whole)?;
        bind.sorts.insert(sort.clone(), names.len() as i64);
        // The element names, for generated action names (`A(n=a)`): each copied name
        // charged (a node and its text) before the copy, and one pass for the longest.
        let text: usize = names.iter().map(String::len).sum();
        charge(
            budget,
            names
                .len()
                .saturating_add(crate::budget::text_cost(text))
                .saturating_add(1 + crate::budget::text_cost(sort.len())),
            whole,
        )?;
        burn(
            budget,
            (names.len() as u64)
                .saturating_add(crate::budget::text_cost(text) as u64)
                .saturating_add(lookup_cost(bind.sort_names.len(), sort.len())),
            whole,
        )?;
        let widest = names.iter().map(|n| escaped_len(n)).max().unwrap_or(0);
        bind.sort_names
            .insert(sort.clone(), (names.clone(), widest));
    }
    for (name, value) in config.constants() {
        let Some(decl) = constants.get(name.as_str()) else {
            return refuse(ConfigRefusalKind::ExtraConstant, name, whole);
        };
        // The value is charged by its nodes and its text; the check charges each step,
        // each table lookup included, before it takes it.
        burn(
            budget,
            lookup_cost(config.constants().len(), name.len()),
            decl.span,
        )?;
        let size = config.constant_size(name).unwrap_or(usize::MAX);
        charge(budget, size, decl.span)?;
        typecheck(value, &decl.ty, &tables, budget, decl.span)?.map_err(|kind| LowerError {
            kind: LowerErrorKind::Configuration(ConfigRefusal {
                kind,
                name: name.clone(),
            }),
            span: decl.span,
        })?;
        // A constant of a composite type (bn-23hzh): a set of composites by its members'
        // indices, anything else by its value's index, when the universe fits `i64`.
        let composite = match &decl.ty {
            Type::Set(t) => !layout::is_scalar(t),
            t => !layout::is_scalar(t),
        };
        let slot = if composite {
            composite_binding(value, &decl.ty, &tables, &bind, budget, decl.span)?
        } else {
            match value {
                ConfigValue::Int(n) => Some(Slot::Int(*n)),
                ConfigValue::Bool(b) => Some(Slot::Bool(*b)),
                ConfigValue::Elem { sort, name: e } => {
                    let index = tables.element(sort, e, budget, decl.span)?.unwrap_or(0);
                    Some(Slot::Int(index as i64))
                }
                ConfigValue::Variant {
                    enumeration,
                    name: v,
                } => variant_index::<Build>(budget, enumeration, v, decl.span)?.map(Slot::Int),
                // A set of integer-coded values, Booleans as `0`/`1` (correction 4): its
                // codes, ascending. The
                // set was charged by its nodes above; the codes (one each) and their sort are
                // charged before they are stored.
                ConfigValue::Set(members) => {
                    let mut codes = Vec::new();
                    charge(budget, members.len(), decl.span)?;
                    burn(budget, sort_cost(members.len(), 0), decl.span)?;
                    for m in members {
                        let code = match m {
                            ConfigValue::Int(n) => Some(*n),
                            ConfigValue::Bool(b) => Some(i64::from(*b)),
                            ConfigValue::Elem { sort, name: e } => tables
                                .element(sort, e, budget, decl.span)?
                                .map(|i| i as i64),
                            ConfigValue::Variant {
                                enumeration,
                                name: v,
                            } => variant_index::<Build>(budget, enumeration, v, decl.span)?,
                            _ => None,
                        };
                        match code {
                            Some(c) => codes.push(c),
                            None => break,
                        }
                    }
                    if codes.len() == members.len() {
                        codes.sort_unstable();
                        codes.dedup();
                        Some(Slot::Set(codes))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        };
        if let Some(slot) = slot {
            charge(budget, 1 + crate::budget::text_cost(name.len()), decl.span)?;
            burn(
                budget,
                lookup_cost(bind.ints.len().max(bind.bools.len()), name.len()),
                decl.span,
            )?;
            burn(budget, lookup_cost(bind.sets.len(), name.len()), decl.span)?;
            burn(budget, lookup_cost(bind.atoms.len(), name.len()), decl.span)?;
            match slot {
                Slot::Int(v) => bind.ints.insert(name.clone(), v).map(|_| ()),
                Slot::Bool(v) => bind.bools.insert(name.clone(), v).map(|_| ()),
                Slot::Set(v) => bind.sets.insert(name.clone(), Rc::from(v)).map(|_| ()),
                Slot::Atom(v) => bind.atoms.insert(name.clone(), v).map(|_| ()),
            };
        }
    }
    Ok(bind)
}

/// A binding, before it is stored.
enum Slot {
    Int(i64),
    Bool(bool),
    Set(Vec<i64>),
    /// The index of a composite value (bn-23hzh).
    Atom(i64),
}

/// The binding of a constant of a composite type (bn-23hzh): a set by its members'
/// indices (ascending, charged one node each, their sort spent first), any other
/// composite by its value's index. `None` when the universe (of the members, or of the
/// value) does not fit `i64`: the constant then lowers nowhere, and a use of it is
/// refused as a non-integer value. Its type was checked by `typecheck`.
fn composite_binding(
    value: &ConfigValue,
    ty: &Type,
    tables: &Tables<'_>,
    bind: &Bindings,
    budget: &mut Meter,
    at: Span,
) -> R<Option<Slot>> {
    collections::type_cost::<Build>(ty, budget, at)?;
    let fits = |t: &Type, budget: &Meter| {
        matches!(layout::Ty { bind, enums: &budget.enums }.card(t),
            layout::Card::Finite(n) if n <= i64::MAX as u128)
    };
    match (value, ty) {
        (ConfigValue::Set(members), Type::Set(t)) => {
            if !fits(t, budget) {
                return Ok(None);
            }
            charge(budget, members.len(), at)?;
            burn(budget, sort_cost(members.len(), 0), at)?;
            let mut atoms = Vec::with_capacity(members.len());
            for m in members {
                match atom_of(m, t, tables, bind, budget, at)? {
                    Some(i) => atoms.push(i64::try_from(i).unwrap_or(i64::MAX)),
                    None => return Ok(None),
                }
            }
            atoms.sort_unstable();
            atoms.dedup();
            Ok(Some(Slot::Set(atoms)))
        }
        (_, t) => {
            if !fits(t, budget) {
                return Ok(None);
            }
            Ok(atom_of(value, t, tables, bind, budget, at)?
                .and_then(|i| i64::try_from(i).ok())
                .map(Slot::Atom))
        }
    }
}

/// The index in `U(ty)` of a configuration value of type `ty` (RFC 0003 correction 4,
/// "Finite types": the canonical order), or `None` when it is not in the universe or
/// does not fit. One unit of work per value node and each table lookup, spent first.
/// Recursion follows the value, within the configuration's JSON depth bound.
fn atom_of(
    value: &ConfigValue,
    ty: &Type,
    tables: &Tables<'_>,
    bind: &Bindings,
    budget: &mut Meter,
    at: Span,
) -> R<Option<u128>> {
    burn(budget, 1, at)?;
    let t = layout::Ty {
        bind,
        enums: &budget.enums,
    };
    let scalar = |code: i64, t: layout::Ty<'_>| -> Option<u128> {
        let (lo, hi) = t.range(ty).ok()?;
        (lo <= code && code <= hi).then(|| (i128::from(code) - i128::from(lo)) as u128)
    };
    Ok(match (value, ty) {
        (ConfigValue::Int(n), Type::Int | Type::Nat) => scalar(*n, t),
        (ConfigValue::Bool(b), Type::Bool) => Some(u128::from(*b)),
        (ConfigValue::Elem { sort, name }, Type::Sort(_)) => {
            tables.element(sort, name, budget, at)?.map(|i| i as u128)
        }
        (ConfigValue::Variant { enumeration, name }, Type::Enum(_)) => {
            variant_index::<Build>(budget, enumeration, name, at)?.map(|i| i as u128)
        }
        (ConfigValue::None, Type::Option(_)) => Some(0),
        (ConfigValue::Some(x), Type::Option(inner)) => {
            atom_of(x, inner, tables, bind, budget, at)?.and_then(|i| i.checked_add(1))
        }
        (ConfigValue::Tuple(xs), Type::Tuple(ts)) if xs.len() == ts.len() => {
            let mut idx: Option<u128> = Some(0);
            for (x, c) in xs.iter().zip(ts) {
                let count = layout::Ty {
                    bind,
                    enums: &budget.enums,
                }
                .count(c);
                let part = atom_of(x, c, tables, bind, budget, at)?;
                idx = match (idx, part) {
                    (Some(i), Some(p)) => i.checked_mul(count).and_then(|i| i.checked_add(p)),
                    _ => None,
                };
            }
            idx
        }
        (ConfigValue::Set(xs), Type::Set(inner)) => {
            let n = t.count(inner);
            let mut idx: u128 = 0;
            for x in xs {
                let Some(j) = atom_of(x, inner, tables, bind, budget, at)? else {
                    return Ok(None);
                };
                let shift = n.saturating_sub(1).saturating_sub(j);
                if shift >= 128 {
                    return Ok(None);
                }
                idx |= 1 << shift;
            }
            Some(idx)
        }
        (ConfigValue::Map(entries), Type::Map(k, v)) => {
            let n = t.count(k);
            let base = t.count(v).saturating_add(1);
            let mut idx: u128 = 0;
            for (key, val) in entries {
                let Some(j) = atom_of(key, k, tables, bind, budget, at)? else {
                    return Ok(None);
                };
                let Some(d) = atom_of(val, v, tables, bind, budget, at)? else {
                    return Ok(None);
                };
                let place = layout::pow_sat(base, n.saturating_sub(1).saturating_sub(j));
                let Some(add) = d.checked_add(1).and_then(|d| d.checked_mul(place)) else {
                    return Ok(None);
                };
                let Some(sum) = idx.checked_add(add) else {
                    return Ok(None);
                };
                idx = sum;
            }
            Some(idx)
        }
        _ => None,
    })
}

/// The name tables the binding check reads: each sort's elements by name, with their
/// indices, and each enumeration's variants.
#[derive(Default)]
struct Tables<'c> {
    elements: std::collections::BTreeMap<&'c str, std::collections::BTreeMap<&'c str, usize>>,
    variants: std::collections::BTreeMap<&'c str, std::collections::BTreeSet<&'c str>>,
    /// The configuration's `Nat` and `Int` bounds, which its own values must respect.
    bounds: crate::config::Bounds,
}

impl Tables<'_> {
    /// The index of element `name` of `sort`, charging the two lookups (logarithmic in
    /// each table, linear in the name) before they are made.
    fn element(&self, sort: &str, name: &str, budget: &mut Meter, at: Span) -> R<Option<usize>> {
        burn(budget, lookup_cost(self.elements.len(), sort.len()), at)?;
        let Some(names) = self.elements.get(sort) else {
            return Ok(None);
        };
        burn(budget, lookup_cost(names.len(), name.len()), at)?;
        Ok(names.get(name).copied())
    }

    /// Whether `name` is a variant of `enumeration`, charged the same way.
    fn variant(&self, enumeration: &str, name: &str, budget: &mut Meter, at: Span) -> R<bool> {
        burn(
            budget,
            lookup_cost(self.variants.len(), enumeration.len()),
            at,
        )?;
        let Some(names) = self.variants.get(enumeration) else {
            return Ok(false);
        };
        burn(budget, lookup_cost(names.len(), name.len()), at)?;
        Ok(names.contains(name))
    }
}

/// Whether `value` has type `ty`: `Ok(Err(kind))` when it does not. Each step is
/// charged before it is taken: one unit per value node, the text of each name
/// compared, and each table lookup. Recursion is bounded by the configuration's JSON
/// depth bound.
fn typecheck(
    value: &ConfigValue,
    ty: &Type,
    tables: &Tables<'_>,
    budget: &mut Meter,
    at: Span,
) -> R<Result<(), ConfigRefusalKind>> {
    burn(budget, 1, at)?;
    let ill = Ok(Err(ConfigRefusalKind::IllTyped));
    Ok(match (value, ty) {
        (_, Type::Function(..)) => Err(ConfigRefusalKind::UnbindableType),
        // An integer inside the configuration's own bound for its type; a type with no
        // bound admits every value, as before correction 4.
        (ConfigValue::Int(n), Type::Int) => match tables.bounds.int_range() {
            Some((lo, hi)) if *n < lo || *n > hi => Err(ConfigRefusalKind::OutsideBound),
            _ => Ok(()),
        },
        (ConfigValue::Int(n), Type::Nat) if *n >= 0 => match tables.bounds.nat_max() {
            Some(hi) if *n > hi => Err(ConfigRefusalKind::OutsideBound),
            _ => Ok(()),
        },
        (ConfigValue::Bool(_), Type::Bool)
        | (ConfigValue::Str(_), Type::Str)
        | (ConfigValue::None, Type::Option(_)) => Ok(()),
        (ConfigValue::Elem { sort, name }, Type::Sort(s)) => {
            burn(budget, crate::budget::text_cost(s.len()) as u64, at)?;
            if sort != s {
                return ill;
            }
            match tables.element(s, name, budget, at)? {
                Some(_) => Ok(()),
                None => return ill,
            }
        }
        (ConfigValue::Variant { enumeration, name }, Type::Enum(e)) => {
            burn(budget, crate::budget::text_cost(e.len()) as u64, at)?;
            if enumeration != e || !tables.variant(e, name, budget, at)? {
                return ill;
            }
            Ok(())
        }
        (ConfigValue::Tuple(xs), Type::Tuple(ts)) if xs.len() == ts.len() => {
            for (x, t) in xs.iter().zip(ts) {
                if let Err(kind) = typecheck(x, t, tables, budget, at)? {
                    return Ok(Err(kind));
                }
            }
            Ok(())
        }
        (ConfigValue::Record(fields), Type::Record(ts)) => {
            if fields.len() != ts.len() {
                return ill;
            }
            for (name, t) in ts {
                burn(budget, lookup_cost(fields.len(), name.len()), at)?;
                let Some(x) = fields.get(name) else {
                    return ill;
                };
                if let Err(kind) = typecheck(x, t, tables, budget, at)? {
                    return Ok(Err(kind));
                }
            }
            Ok(())
        }
        (ConfigValue::Set(xs), Type::Set(t)) | (ConfigValue::Seq(xs), Type::Seq(t)) => {
            for x in xs {
                if let Err(kind) = typecheck(x, t, tables, budget, at)? {
                    return Ok(Err(kind));
                }
            }
            Ok(())
        }
        (ConfigValue::Map(entries), Type::Map(k, v)) => {
            for (a, b) in entries {
                if let Err(kind) = typecheck(a, k, tables, budget, at)? {
                    return Ok(Err(kind));
                }
                if let Err(kind) = typecheck(b, v, tables, budget, at)? {
                    return Ok(Err(kind));
                }
            }
            Ok(())
        }
        (ConfigValue::Some(x), Type::Option(t)) => return typecheck(x, t, tables, budget, at),
        _ => Err(ConfigRefusalKind::IllTyped),
    })
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

    // State variables and their slots (RFC 0003 correction 4, "Layout", bn-23hzh). The
    // slot count is computed from the types, saturating, and checked against model-core's
    // limit before any slot is built: a scalar is one slot, a finite composite its
    // layout's width, and a composite with no finite universe counts one here and is
    // refused below, in name order, as before.
    let mut slot_count: u128 = 0;
    for v in &model.state {
        let width = if layout::is_scalar(&v.ty) {
            1
        } else {
            collections::type_cost::<Build>(&v.ty, budget, v.span)?;
            let t = collections::ty(budget);
            match t.card(&v.ty) {
                layout::Card::Finite(_) => t.width(&v.ty),
                _ => 1,
            }
        };
        slot_count = slot_count.saturating_add(width);
    }
    if slot_count > MAX_VARIABLES as u128 {
        return no(Unlowerable::TooManyVariables, whole);
    }
    // A variable reference scans the declared slots in the builder and the evaluator.
    budget.vars = usize::try_from(slot_count).unwrap_or(usize::MAX);
    let mut builder = ModelBuilder::new();
    let mut variables: Vec<Variable> = Vec::new();
    let mut laid = collections::StateLayout::default();
    for v in &model.state {
        // Reading the refinement visits each of its nodes a constant number of times.
        let visits = v.refinement.iter().fold(1_u64, |acc, c| {
            acc.saturating_add(crate::elab::measure(c).0 as u64)
        });
        burn(budget, visits.saturating_mul(2), v.span)?;
        // `domain_of` and `constant` recurse over each refinement clause: its depth is
        // checked first, as `shallow` checks every clause the lowering recurses over (a
        // hand-built model is not bounded by elaboration).
        for c in &v.refinement {
            if crate::elab::measure(c).1 > MAX_EXPR_DEPTH.saturating_add(1) {
                return no(Unlowerable::ExpressionTooDeep, c.span);
            }
        }
        if !layout::is_scalar(&v.ty) {
            let var = laid_var(v, budget)?;
            for (name, lo, hi) in &var.slots {
                builder = builder.variable(name, *lo, *hi);
                if let (Ok(name), Ok(domain)) = (Ident::new(name), Domain::new(*lo, *hi)) {
                    variables.push(Variable::new(name, domain));
                }
            }
            burn(budget, lookup_cost(laid.vars.len(), v.name.len()), v.span)?;
            charge(budget, 1 + crate::budget::text_cost(v.name.len()), v.span)?;
            laid.vars.insert(v.name.clone(), var);
            continue;
        }
        if let Type::Sort(s) = &v.ty {
            burn(
                budget,
                lookup_cost(budget.bind.sorts.len(), s.len()).saturating_mul(2),
                v.span,
            )?;
        }
        let (lo, hi) = match &v.ty {
            // An instantiated sort: its elements are the integers `0..size`.
            Type::Sort(s) if budget.bind.sorts.contains_key(s) => {
                if let Some(c) = v.refinement.first() {
                    return no(Unlowerable::NonIntervalRefinement, c.span);
                }
                let size = budget.bind.sorts.get(s).copied().unwrap_or(1);
                (0, size.saturating_sub(1))
            }
            // An enumeration (correction 4): its variant indices.
            Type::Enum(e) => {
                burn(budget, lookup_cost(budget.enums.len(), e.len()), v.span)?;
                let Some(n) = budget.enums.get(e).map(|t| t.names.len() as i64) else {
                    return no(Unlowerable::NonIntegerState, v.span);
                };
                if let Some(c) = v.refinement.first() {
                    return no(Unlowerable::NonIntervalRefinement, c.span);
                }
                if n == 0 {
                    return no(Unlowerable::EmptyRefinement, v.span);
                }
                (0, n - 1)
            }
            _ => domain_of(v.name.as_str(), &v.ty, &v.refinement, v.span, &budget.bind)?,
        };
        // The variable record the builder appends, with its owned name (cr-3aqchd).
        charge(budget, 1 + crate::budget::text_cost(v.name.len()), v.span)?;
        builder = builder.variable(&v.name, lo, hi);
        // The lowering's own copy of the name, for enumeration: at most `MAX_VARIABLES`
        // names of at most `MAX_IDENT_BYTES` bytes (`Ident::new` refuses a longer name
        // before it copies it), a bound of constants.
        if let (Ok(name), Ok(domain)) = (Ident::new(&v.name), Domain::new(lo, hi)) {
            variables.push(Variable::new(name, domain));
        }
    }
    budget.layout = Rc::new(laid);

    // Each slot's domain, for the intervals of quantifier domains: an entry (a node
    // and its name) charged per slot before the table is built.
    let names: usize = variables.iter().map(|v| v.name().as_str().len()).sum();
    charge(
        budget,
        variables
            .len()
            .saturating_add(crate::budget::text_cost(names)),
        whole,
    )?;
    burn(budget, sort_cost(variables.len(), names), whole)?;
    budget.domains = variables
        .iter()
        .map(|v| {
            (
                v.name().as_str().to_owned(),
                (v.domain().lo(), v.domain().hi()),
            )
        })
        .collect();

    // Model-core's own limits, preflighted before anything is enumerated or built, so
    // no expansion ever runs past a bound the builder would only report afterwards.
    let by_name: std::collections::BTreeMap<&str, &Variable> =
        variables.iter().map(|v| (v.name().as_str(), v)).collect();
    preflight(model, &by_name, &mut *budget)?;

    // Initial states: every canonical state of the slot domain the init predicate
    // accepts. A clause's map reads give definedness items (bn-23hzh).
    let Some(init) = &model.init else {
        return no(Unlowerable::NoInit, whole);
    };
    let mut clauses: Vec<Sized<BoolExpr>> = Vec::with_capacity(init.clauses.len());
    let mut defined: Vec<Sized<BoolExpr>> = Vec::new();
    for c in &init.clauses {
        let (clause, items) = top_bool(c, &mut *budget)?;
        clauses.push(clause);
        defined.extend(items);
    }
    let predicate_size = conjoined_size(&clauses);
    let init_pred = conjoin(clauses, &mut *budget, init.span)?;
    let defined_size = if defined.is_empty() {
        0
    } else {
        conjoined_size(&defined)
    };
    let defined = if defined.is_empty() {
        None
    } else {
        Some(conjoin(defined, &mut *budget, init.span)?)
    };
    // The canonicity constraint of every composite variable: only a canonical flat
    // state is a value of the state's type, so no other is a candidate (RFC 0003
    // correction 4, "Layout").
    let layout_now = Rc::clone(&budget.layout);
    let mut canon: Vec<Sized<BoolExpr>> = Vec::new();
    for var in layout_now.vars.values() {
        if let Some(c) = collections::canonicity(var, &mut *budget, init.span)? {
            canon.push(c);
        }
    }
    let canon_size = if canon.is_empty() {
        0
    } else {
        conjoined_size(&canon)
    };
    let canon = if canon.is_empty() {
        None
    } else {
        Some(conjoin(canon, &mut *budget, init.span)?)
    };
    let cardinality = variables.iter().fold(1_u128, |acc, v| {
        acc.saturating_mul(v.domain().cardinality())
    });
    if cardinality > MAX_INIT_ENUMERATION {
        return no(Unlowerable::InitDomainTooLarge, init.span);
    }
    // The whole enumeration is charged before it starts: every candidate evaluates the
    // canonicity constraint, the definedness condition, and the predicate at most once
    // each (each node visited once; a variable read scans the declared slots) and steps
    // the value vector. Too much work is refused here, having enumerated nothing.
    burn(
        budget,
        init_work(
            cardinality,
            predicate_size
                .saturating_add(defined_size)
                .saturating_add(canon_size),
            variables.len(),
        ),
        init.span,
    )?;
    // A variable whose name or domain the builder refuses is missing from `variables`;
    // `build` below reports it, so the enumeration is skipped rather than run over a
    // partial state.
    if variables.len() as u128 == slot_count {
        let mut values: Vec<i64> = variables.iter().map(|v| v.domain().lo()).collect();
        let mut bindings_held: usize = 0;
        // Per accepted state: the names' bytes (for the work of the copy), and the output
        // the builder holds — the state's record, and per variable a binding with its own
        // copy of the variable's name (cr-3aqchd).
        let names: usize = variables.iter().map(|v| v.name().as_str().len()).sum();
        let state_nodes = variables.iter().fold(1_usize, |acc, v| {
            acc.saturating_add(1)
                .saturating_add(crate::budget::text_cost(v.name().as_str().len()))
        });
        let failed = |e: EvalError| LowerError {
            kind: LowerErrorKind::Evaluation(e),
            span: init.span,
        };
        loop {
            let env = Environment::new(&variables, &values);
            // A flat state that is not canonical is no value: skipped before the
            // predicate is evaluated, so the predicate adds no error there. At a
            // canonical state an undefined read in the predicate is an error of the
            // CML model, which initial states cannot carry: a typed refusal.
            let canonical = match &canon {
                Some(c) => c.evaluate(&env).map_err(failed)?,
                None => true,
            };
            if canonical {
                if let Some(d) = &defined
                    && !d.evaluate(&env).map_err(failed)?
                {
                    return no(Unlowerable::UndefinedRead, init.span);
                }
                if init_pred.evaluate(&env).map_err(failed)? {
                    bindings_held = bindings_held.saturating_add(variables.len().max(1));
                    if bindings_held > MAX_INIT_BINDINGS {
                        return no(Unlowerable::TooManyInitialStates, init.span);
                    }
                    // Each accepted state is copied into the builder, then placed and
                    // sorted by `ModelBuilder::build` (a scan of the variables per
                    // binding): its work and its output charged before the copy.
                    let per_state = (variables.len() as u64)
                        .saturating_mul(variables.len() as u64 + 1)
                        .saturating_add(crate::budget::text_cost(names) as u64)
                        .saturating_add(
                            lookup_cost(bindings_held, 8).saturating_mul(variables.len() as u64),
                        );
                    burn(budget, per_state, init.span)?;
                    charge(budget, state_nodes, init.span)?;
                    let bindings: Vec<(&str, i64)> = variables
                        .iter()
                        .zip(values.iter())
                        .map(|(v, x)| (v.name().as_str(), *x))
                        .collect();
                    builder = builder.initial_state(&bindings);
                }
            }
            if !advance(&mut values, &variables) {
                break;
            }
        }
    }

    // Actions: one programmatic action per parameter tuple (an action schema), and per
    // candidate post-state of its relational variables (RFC 0003 "Relational actions"
    // and correction 4).
    let mut lowered_actions = Lowered::default();
    let mut lowered_predicates = Lowered::default();
    for a in &model.actions {
        builder = lower_action(
            builder,
            a,
            &by_name,
            &mut *budget,
            &mut lowered_actions,
            &mut lowered_predicates,
        )?;
    }

    // Invariants become named predicates, and an invariant with map reads also its
    // definedness predicate `I#defined` (bn-23hzh).
    for i in &model.invariants {
        let mut clauses = Vec::with_capacity(i.clauses.len());
        let mut defined: Vec<Sized<BoolExpr>> = Vec::new();
        for c in &i.clauses {
            let (clause, items) = top_bool(c, &mut *budget)?;
            clauses.push(clause);
            defined.extend(items);
        }
        let body = conjoin(clauses, &mut *budget, i.span)?;
        // The predicate record the builder appends, with its owned name (cr-3aqchd).
        charge(budget, 1 + crate::budget::text_cost(i.name.len()), i.span)?;
        builder = builder.predicate(&i.name, body);
        lowered_predicates.add(&i.name);
        if !defined.is_empty() {
            let name = definedness_name(&i.name, &mut *budget, i.span)?;
            let body = conjoin(defined, &mut *budget, i.span)?;
            charge(budget, 1 + crate::budget::text_cost(name.len()), i.span)?;
            builder = builder.predicate(&name, body);
            lowered_predicates.add(&name);
        }
    }

    // `ModelBuilder::build` sorts the variables, actions, and predicates by name; the
    // rest of its validation is linear in what was charged above.
    let slot_bytes: usize = variables.iter().map(|v| v.name().as_str().len()).sum();
    let sorting = sort_cost(variables.len(), slot_bytes)
        .saturating_add(sort_cost(lowered_actions.count, lowered_actions.bytes))
        .saturating_add(sort_cost(
            lowered_predicates.count,
            lowered_predicates.bytes,
        ));
    burn(budget, sorting, whole)?;
    builder.build().map_err(|e| LowerError {
        kind: LowerErrorKind::Model(e),
        span: whole,
    })
}

/// The name of the definedness predicate of `subject`, `subject#defined`: refused past
/// `MAX_IDENT_BYTES` before it is built, and its bytes spent (bn-23hzh).
fn definedness_name(subject: &str, budget: &mut Meter, span: Span) -> R<String> {
    let len = subject.len().saturating_add(DEFINED_SUFFIX.len());
    if len > MAX_IDENT_BYTES {
        return no(Unlowerable::NameTooLong, span);
    }
    burn(budget, crate::budget::text_cost(len) as u64, span)?;
    Ok(format!("{subject}{DEFINED_SUFFIX}"))
}

/// The slot table of a composite state variable (RFC 0003 correction 4, "Layout"): its
/// type must be finite (else `cml.lower.unbounded_type` under a configuration, and
/// `cml.lower.non_integer_state` otherwise) and take no refinement. The table's own
/// copy of the type is charged by its size.
fn laid_var(v: &crate::norm::StateVar, budget: &mut Meter) -> R<collections::LaidVar> {
    let card = collections::ty(budget).card(&v.ty);
    match card {
        layout::Card::Finite(_) => {}
        layout::Card::Unbounded if budget.bind.configured => {
            return no(Unlowerable::UnboundedType, v.span);
        }
        _ => return no(Unlowerable::NonIntegerState, v.span),
    }
    if let Some(c) = v.refinement.first() {
        return no(Unlowerable::NonIntervalRefinement, c.span);
    }
    let slots = collections::slots_of(&v.name, &v.ty, budget, v.span)?;
    let widest = slots.iter().map(|(n, _, _)| n.len()).max().unwrap_or(0);
    let hull = slots.iter().fold((0_i64, 0_i64), |(lo, hi), (_, a, b)| {
        (lo.min(*a), hi.max(*b))
    });
    charge(budget, crate::types::type_measure(&v.ty).0, v.span)?;
    Ok(collections::LaidVar {
        ty: v.ty.clone(),
        slots,
        text: crate::budget::text_cost(widest),
        hull,
    })
}

/// The actions appended to the builder so far, counted for the cost of the sort
/// `ModelBuilder::build` runs over their names: a count and a byte total, so no second
/// copy of any name is held (cr-3aqchd).
#[derive(Default)]
struct Lowered {
    count: usize,
    bytes: usize,
}

impl Lowered {
    fn add(&mut self, name: &str) {
        self.count = self.count.saturating_add(1);
        self.bytes = self.bytes.saturating_add(name.len());
    }
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

/// The variable name a post-state read of the relational variable in `slot` lowers to
/// while its action is enumerated: `'` and the slot number. `'` is not a CML identifier
/// character, so it names no declared variable; every occurrence is replaced by a
/// constant before the builder sees the expression.
fn primed_placeholder(slot: usize) -> String {
    format!("'{slot}")
}

/// The slot a placeholder names, read from its (at most twenty) digits: constant work,
/// whatever the number of relational variables.
fn placeholder_slot(v: &str) -> Option<usize> {
    v.strip_prefix('\'')?.parse().ok()
}

/// The work (and output nodes) of enumerating `candidates` candidate post-states of a
/// relational action whose lowered template (guard with postconditions, updates, and
/// candidate constants) has `template` nodes and whose candidate names total
/// `name_bytes` bytes each: per candidate, one copy of the template with the constants
/// substituted, and its name. Saturates.
pub fn successor_work(candidates: u128, template: usize, name_bytes: usize) -> u64 {
    let per = (template as u128)
        .saturating_add(crate::budget::text_cost(name_bytes) as u128)
        .saturating_add(1);
    u64::try_from(candidates.saturating_mul(per)).unwrap_or(u64::MAX)
}

/// One programmatic action per candidate post-state of the relational variables:
/// guard `G && P[c]`, updates the action's own plus `r := c_r`, named
/// `A[r1=c1,r2=c2]`. Candidates are enumerated with the relational variables in name
/// order and the last one fastest, each over its domain in ascending order.
///
/// The count is bounded by [`MAX_RELATIONAL_CANDIDATES`], and the whole enumeration —
/// candidates × template size, in nodes and in work ([`successor_work`]) — is charged
/// before the first candidate is built, as init enumeration is. Nothing is built for a
/// refused action.
#[allow(clippy::too_many_arguments)]
fn relational_action(
    mut builder: ModelBuilder,
    name: &str,
    guard: (BoolExpr, usize),
    updates: &[(&str, Sized<IntExpr>)],
    relational: &[&Variable],
    budget: &mut Meter,
    span: Span,
    lowered: &mut Lowered,
) -> R<ModelBuilder> {
    let candidates = candidate_count(relational);
    if candidates > MAX_RELATIONAL_CANDIDATES {
        return no(Unlowerable::SuccessorDomainTooLarge, span);
    }
    let name_bytes = widest_label(name.len(), relational);
    let targets = target_names(updates.iter().map(|(v, _)| *v));
    let constants = candidate_constants(relational);
    let template = updates
        .iter()
        .fold(guard.1, |acc, (_, x)| acc.saturating_add(x.1.size))
        .saturating_add(targets)
        .saturating_add(constants);
    // The output is exactly the candidates' copies: charged before the first is built.
    // The work is the same product; it is checked against what the budget has left
    // before the first candidate, then charged as each candidate is built (one unit per
    // node copied, one per placeholder resolved), so the charge is what the
    // substitution actually did and never exceeds the checked prediction.
    let total = successor_work(candidates, template, name_bytes);
    if total > budget.fuel.left() {
        return no(Unlowerable::WorkLimitExceeded, span);
    }
    charge(budget, usize::try_from(total).unwrap_or(usize::MAX), span)?;

    let mut values: Vec<i64> = relational.iter().map(|v| v.domain().lo()).collect();
    loop {
        let label: Vec<String> = relational
            .iter()
            .zip(&values)
            .map(|(v, c)| format!("{}={c}", v.name().as_str()))
            .collect();
        let action = format!("{name}[{}]", label.join(","));
        let mut steps = 0_u64;
        let guard_c = subst_bool(&guard.0, &values, &mut steps);
        let mut assigns: Vec<(&str, IntExpr)> =
            updates.iter().map(|(v, x)| (*v, x.0.clone())).collect();
        let copied = updates
            .iter()
            .fold(0_u64, |acc, (_, x)| acc.saturating_add(x.1.size as u64));
        let per_name = crate::budget::text_cost(action.len()) as u64;
        burn(
            budget,
            steps
                .saturating_add(copied)
                .saturating_add(targets as u64)
                .saturating_add(constants as u64)
                .saturating_add(per_name)
                .saturating_add(1),
            span,
        )?;
        for (v, c) in relational.iter().zip(&values) {
            assigns.push((v.name().as_str(), IntExpr::Const(*c)));
        }
        builder = builder.action(ActionDecl::deterministic(&action, guard_c, assigns));
        lowered.add(&action);
        if !advance(&mut values, relational) {
            break;
        }
    }
    Ok(builder)
}

/// `e` with each placeholder `'k` replaced by `values[k]`. `steps` counts one per node
/// visited; a placeholder is resolved by its slot number, in constant time. Recursion
/// is bounded by the lowered depth ([`MAX_EXPR_DEPTH`]).
fn subst_int(e: &IntExpr, values: &[i64], steps: &mut u64) -> IntExpr {
    *steps = steps.saturating_add(1);
    match e {
        IntExpr::Var(v) => match placeholder_slot(v).and_then(|k| values.get(k)) {
            Some(c) => IntExpr::Const(*c),
            None => e.clone(),
        },
        IntExpr::Const(_) => e.clone(),
        IntExpr::Arith(op, a, b) => IntExpr::Arith(
            *op,
            Box::new(subst_int(a, values, steps)),
            Box::new(subst_int(b, values, steps)),
        ),
        IntExpr::Min(a, b) => IntExpr::Min(
            Box::new(subst_int(a, values, steps)),
            Box::new(subst_int(b, values, steps)),
        ),
        IntExpr::Max(a, b) => IntExpr::Max(
            Box::new(subst_int(a, values, steps)),
            Box::new(subst_int(b, values, steps)),
        ),
    }
}

fn subst_bool(e: &BoolExpr, values: &[i64], steps: &mut u64) -> BoolExpr {
    *steps = steps.saturating_add(1);
    match e {
        BoolExpr::Const(_) => e.clone(),
        BoolExpr::Compare { op, left, right } => BoolExpr::Compare {
            op: *op,
            left: subst_int(left, values, steps),
            right: subst_int(right, values, steps),
        },
        BoolExpr::Not(a) => BoolExpr::Not(Box::new(subst_bool(a, values, steps))),
        BoolExpr::And(x, y) => {
            let x = subst_bool(x, values, steps);
            BoolExpr::And(Box::new(x), Box::new(subst_bool(y, values, steps)))
        }
        BoolExpr::Or(x, y) => {
            let x = subst_bool(x, values, steps);
            BoolExpr::Or(Box::new(x), Box::new(subst_bool(y, values, steps)))
        }
        BoolExpr::Implies(x, y) => {
            let x = subst_bool(x, values, steps);
            BoolExpr::Implies(Box::new(x), Box::new(subst_bool(y, values, steps)))
        }
        BoolExpr::InRange { expr, lo, hi } => BoolExpr::InRange {
            expr: subst_int(expr, values, steps),
            lo: *lo,
            hi: *hi,
        },
    }
}

/// The number of candidate post-states of a relational action: the product of its
/// relational variables' domains. Saturates.
fn candidate_count(relational: &[&Variable]) -> u128 {
    relational.iter().fold(1_u128, |acc, v| {
        acc.saturating_mul(v.domain().cardinality())
    })
}

/// The nodes of one candidate's relational updates `r := c`: per relational variable,
/// the constant and the target name the update owns.
fn candidate_constants(relational: &[&Variable]) -> usize {
    relational.iter().fold(relational.len(), |acc, v| {
        acc.saturating_add(crate::budget::text_cost(v.name().as_str().len()))
    })
}

/// The nodes of the target names an action's updates own: each update copies its
/// variable's name into the action record, charged by its length (cr-3aqchd).
fn target_names<'a>(names: impl Iterator<Item = &'a str>) -> usize {
    names.fold(0, |acc, v| {
        acc.saturating_add(crate::budget::text_cost(v.len()))
    })
}

/// The nodes of one appended action record apart from its guard and update
/// expressions: the record itself, its `name_bytes`-byte name, and its update targets'
/// names (`targets`, from [`target_names`]). A relational candidate's record is counted
/// by [`successor_work`] instead (cr-3aqchd).
fn action_record(name_bytes: usize, targets: usize) -> usize {
    crate::budget::text_cost(name_bytes)
        .saturating_add(targets)
        .saturating_add(1)
}

/// The byte length of the longest name `A[r1=c1,…,rn=cn]` the candidates of action
/// `name` generate. Exact: a value's decimal text is longest at an end of its domain.
fn widest_label(name: usize, relational: &[&Variable]) -> usize {
    let digits = |v: i64| v.to_string().len();
    let parts = relational.iter().fold(0_usize, |acc, v| {
        let widest = digits(v.domain().lo()).max(digits(v.domain().hi()));
        acc.saturating_add(v.name().as_str().len())
            .saturating_add(1)
            .saturating_add(widest)
    });
    let commas = relational.len().saturating_sub(1);
    name.saturating_add(2)
        .saturating_add(parts)
        .saturating_add(commas)
}

/// Model-core's limits on the lowered model, checked before init enumeration and
/// before any action is built: every declared name, the widest generated action name,
/// each relational action's candidate count, and the total number of programmatic
/// actions ([`MAX_ACTIONS`]). The variable count ([`MAX_VARIABLES`]) is checked before
/// the domains are read, and the initial-state count is bounded by
/// [`MAX_INIT_BINDINGS`] `<=` [`MAX_INITIAL_STATES`]. Work: a table lookup per action
/// entry and a pass over the names, charged before the pass.
fn preflight(
    model: &NormModel,
    by_name: &std::collections::BTreeMap<&str, &Variable>,
    budget: &mut Meter,
) -> R<()> {
    let names = model
        .state
        .iter()
        .map(|v| (v.name.len(), v.span))
        .chain(model.actions.iter().map(|a| (a.name.len(), a.span)))
        .chain(model.invariants.iter().map(|i| (i.name.len(), i.span)));
    for (len, span) in names {
        burn(budget, 1, span)?;
        if len > MAX_IDENT_BYTES {
            return no(Unlowerable::NameTooLong, span);
        }
    }
    let mut total: u128 = 0;
    for a in &model.actions {
        burn(
            budget,
            (a.next.len() as u64).saturating_mul(lookup_cost(by_name.len(), 32)),
            a.span,
        )?;
        // A relational variable of a composite type is not in this correction: its
        // candidates would be values of the type, not slot values (bn-23hzh).
        for (v, n) in &a.next {
            if matches!(n, Next::Relational) {
                burn(
                    budget,
                    lookup_cost(budget.layout.vars.len(), v.len()),
                    a.span,
                )?;
                if budget.layout.vars.contains_key(v) {
                    return no(Unlowerable::NonIntegerState, a.span);
                }
            }
        }
        let relational: Vec<&Variable> = a
            .next
            .iter()
            .filter(|(_, n)| matches!(n, Next::Relational))
            .filter_map(|(v, _)| by_name.get(v.as_str()).copied())
            .collect();
        // An action schema: one instance per parameter tuple, named `A(p=u,…)`.
        let space = param_space(a, budget)?;
        let instances = instance_count(&space);
        let width = instance_width(a, &space, budget)?;
        if width > MAX_IDENT_BYTES {
            return no(Unlowerable::NameTooLong, a.span);
        }
        let expanded = if relational.is_empty() {
            1
        } else {
            let candidates = candidate_count(&relational);
            if candidates > MAX_RELATIONAL_CANDIDATES {
                return no(Unlowerable::SuccessorDomainTooLarge, a.span);
            }
            if widest_label(width, &relational) > MAX_IDENT_BYTES {
                return no(Unlowerable::NameTooLong, a.span);
            }
            candidates
        };
        total = total.saturating_add(instances.saturating_mul(expanded));
        if total > MAX_ACTIONS as u128 {
            return no(Unlowerable::TooManyActions, a.span);
        }
    }
    Ok(())
}

/// The universe of each parameter of `a`, in declaration order (RFC 0003 correction
/// 4, "Action schemas"). A parameter of a type with no finite universe is
/// `cml.lower.unbounded_type` under a configuration and `cml.lower.parameterized_action`
/// without one; a parameter of a collection or other non-scalar type is
/// `cml.lower.parameterized_action`.
fn param_space(a: &crate::norm::Action, budget: &mut Meter) -> R<Vec<(i64, i64)>> {
    let mut space = Vec::with_capacity(a.params.len());
    for p in &a.params {
        match universe::<Build>(&p.ty, budget, a.span)? {
            Universe::Finite(lo, hi) => space.push((lo, hi)),
            Universe::Unbounded => {
                return no(unbounded(budget, Unlowerable::ParameterizedAction), a.span);
            }
            Universe::NotScalar => return no(Unlowerable::ParameterizedAction, a.span),
            Universe::TooLarge => return no(Unlowerable::TooManyActions, a.span),
        }
    }
    Ok(space)
}

/// The number of parameter tuples: the product of the universes. Saturates.
fn instance_count(space: &[(i64, i64)]) -> u128 {
    space.iter().fold(1_u128, |acc, (lo, hi)| {
        acc.saturating_mul(span_count(*lo, *hi))
    })
}

/// The byte length of the longest instance name `A(p1=u1,…,pn=un)` of `a`, or of `A`
/// for an action without parameters. Exact: each value's text is longest at an end of
/// an integer universe, or is the longest element or variant name. A table lookup per
/// parameter.
fn instance_width(a: &crate::norm::Action, space: &[(i64, i64)], budget: &mut Meter) -> R<usize> {
    if a.params.is_empty() {
        return Ok(a.name.len());
    }
    let mut width = a.name.len().saturating_add(2);
    for (i, (p, (lo, hi))) in a.params.iter().zip(space).enumerate() {
        let value = match &p.ty {
            Type::Bool => 5,
            Type::Sort(s) => {
                burn(
                    budget,
                    lookup_cost(budget.bind.sort_names.len(), s.len()),
                    a.span,
                )?;
                budget.bind.sort_names.get(s).map_or(0, |(_, w)| *w)
            }
            Type::Enum(e) => {
                burn(budget, lookup_cost(budget.enums.len(), e.len()), a.span)?;
                budget.enums.get(e).map_or(0, |t| t.widest)
            }
            // A composite value's longest canonical text, exact up to the name limit
            // (bn-23hzh).
            t if !layout::is_scalar(t) => {
                collections::widest_text(t, MAX_IDENT_BYTES, budget, a.span)?
            }
            _ => lo.to_string().len().max(hi.to_string().len()),
        };
        width = width
            .saturating_add(p.name.len())
            .saturating_add(1)
            .saturating_add(value)
            .saturating_add(usize::from(i > 0));
    }
    Ok(width)
}

/// The bytes an instance label or a slot name escapes in an element name: every
/// delimiter of a value's canonical text (`,` `(` `)` `{` `}` `[` `]` `:`), of a label
/// (`=`), and the escape itself. A sort element may be any printable ASCII name, so
/// without escaping `A(p=x,q=y)` could be two tuples (cr-3aqchd), and `s{a,b}` two sets
/// (bn-23hzh: `{`, `}`, and `:` delimit set and map texts).
const LABEL_SPECIALS: &[u8] = b"\\,=()[]{}:";

/// `name` as it appears in an instance label: each byte of [`LABEL_SPECIALS`] prefixed
/// with a backslash. Injective, and it keeps the label printable ASCII.
fn escape_label(name: &str) -> String {
    let mut out = String::with_capacity(escaped_len(name));
    for ch in name.chars() {
        if u8::try_from(ch).is_ok_and(|b| LABEL_SPECIALS.contains(&b)) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// The length of [`escape_label`]`(name)`, without building it.
fn escaped_len(name: &str) -> usize {
    name.len()
        .saturating_add(name.bytes().filter(|b| LABEL_SPECIALS.contains(b)).count())
}

/// The work one instance spends on parameter `p`: its scope entry (a lookup by its name)
/// and its value's text (a lookup by its type's name, for a sort or an enumeration; for
/// a composite, its longest text `widest` and one, spent before the text is written).
fn param_work(p: &crate::norm::Param, params: usize, widest: usize, budget: &Meter) -> u64 {
    let text = match &p.ty {
        Type::Sort(s) => lookup_cost(budget.bind.sort_names.len(), s.len()),
        Type::Enum(e) => lookup_cost(budget.enums.len(), e.len()),
        t if !layout::is_scalar(t) => widest as u64 + 1,
        _ => 0,
    };
    lookup_cost(params, p.name.len()).saturating_add(text)
}

/// The canonical text of the atom `v` of a parameter of type `ty`: `false`/`true`, an
/// element name escaped by [`escape_label`], a variant name, a decimal integer, or a
/// composite's canonical text (bn-23hzh; its longest text `widest` and one spent first).
/// One table lookup.
fn value_text(ty: &Type, v: i64, widest: usize, budget: &mut Meter, span: Span) -> R<String> {
    let index = usize::try_from(v).unwrap_or(usize::MAX);
    if !layout::is_scalar(ty) {
        burn(budget, widest as u64 + 1, span)?;
        return Ok(collections::text_of(
            ty,
            u128::try_from(v).unwrap_or(0),
            budget,
        ));
    }
    Ok(match ty {
        Type::Bool => (if v != 0 { "true" } else { "false" }).to_owned(),
        Type::Sort(s) => {
            burn(
                budget,
                lookup_cost(budget.bind.sort_names.len(), s.len()),
                span,
            )?;
            budget
                .bind
                .sort_names
                .get(s)
                .and_then(|(names, _)| names.get(index))
                .map_or_else(String::new, |n| escape_label(n))
        }
        Type::Enum(e) => {
            burn(budget, lookup_cost(budget.enums.len(), e.len()), span)?;
            budget
                .enums
                .get(e)
                .and_then(|t| t.names.get(index))
                .map_or_else(String::new, |n| escape_label(n))
        }
        _ => v.to_string(),
    })
}

/// One update of an action: a scalar variable takes an integer, a composite one its
/// whole layout, slot by slot (RFC 0003 correction 4, "Layout": every lowered write
/// writes a whole canonical layout).
enum Upd<'a> {
    Int(&'a str, &'a Expr),
    Laid(&'a collections::LaidVar, &'a Expr),
}

/// Lower one action: every instance of its parameter tuples, each relational action's
/// candidates within it.
///
/// The whole action is one planned site. The guard clauses, postconditions, and updates
/// are planned once with every parameter at its whole universe (so a quantifier whose
/// domain depends on a parameter is planned at its widest), their measures, predicted
/// work, scratch output, and definedness items multiplied by the instance count, and
/// that product charged as output — and checked against the work left — before the
/// first instance is built. A relational action's candidates are also checked in
/// aggregate (instances times [`successor_work`]) before the first instance, and charged
/// per instance by [`relational_action`] as before.
fn lower_action(
    mut builder: ModelBuilder,
    a: &crate::norm::Action,
    by_name: &BTreeMap<&str, &Variable>,
    budget: &mut Meter,
    lowered: &mut Lowered,
    predicates: &mut Lowered,
) -> R<ModelBuilder> {
    let space = param_space(a, budget)?;
    let instances = instance_count(&space);
    let width = instance_width(a, &space, budget)?;
    // Each relational variable's domain and slot: one table lookup each.
    burn(
        budget,
        (a.next.len() as u64).saturating_mul(lookup_cost(by_name.len(), 32)),
        a.span,
    )?;
    let relational: Vec<&Variable> = a
        .next
        .iter()
        .filter(|(_, n)| matches!(n, Next::Relational))
        .filter_map(|(v, _)| by_name.get(v.as_str()).copied())
        .collect();
    budget.slots = relational
        .iter()
        .enumerate()
        .map(|(i, v)| (v.name().as_str().to_owned(), i))
        .collect();
    let layout = Rc::clone(&budget.layout);
    let mut updates: Vec<Upd<'_>> = Vec::new();
    for (v, n) in &a.next {
        if let Next::Set(e) = n {
            burn(budget, lookup_cost(layout.vars.len(), v.len()), a.span)?;
            updates.push(match layout.vars.get(v) {
                Some(var) => Upd::Laid(var, e),
                None => Upd::Int(v.as_str(), e),
            });
        }
    }
    for c in a.guard.iter().chain(&a.post) {
        shallow(c, budget)?;
    }
    for u in &updates {
        let (Upd::Int(_, e) | Upd::Laid(_, e)) = u;
        shallow(e, budget)?;
    }

    // The parameters in scope: each entry (a node and its name) charged, with its
    // insertion, before it is made.
    for (p, (lo, hi)) in a.params.iter().zip(&space) {
        charge(budget, 1 + crate::budget::text_cost(p.name.len()), a.span)?;
        burn(budget, lookup_cost(a.params.len(), p.name.len()), a.span)?;
        budget.env.params.insert(p.name.clone(), (*lo, *hi));
    }
    let planned = plan_action(a, &updates, budget);
    let result = planned.and_then(|plan| {
        build_action(
            builder,
            a,
            &space,
            instances,
            width,
            plan,
            &updates,
            &relational,
            budget,
            lowered,
            predicates,
        )
    });
    budget.env.params.clear();
    budget.post = false;
    budget.defs.clear();
    end_site(budget);
    builder = result?;
    Ok(builder)
}

/// The plan of one instance of an action, over the parameters' universes: the measure
/// of each guard clause and postcondition, the definedness items of the guard and
/// postconditions (`dg`) and of the updates (`du`), the measure of each update (a
/// composite update's slots together), the predicted work, and the scratch output.
struct ActionPlan {
    guard: Vec<M>,
    post: Vec<M>,
    dg: collections::DAgg,
    du: collections::DAgg,
    updates: Vec<M>,
    work: u128,
    scratch: u128,
}

fn plan_action(a: &crate::norm::Action, updates: &[Upd<'_>], budget: &mut Meter) -> R<ActionPlan> {
    budget.predicted = 0;
    budget.scratch = 0;
    budget.defs_plan = collections::DAgg::default();
    let mut guard = Vec::with_capacity(a.guard.len());
    for c in &a.guard {
        guard.push(bool_expr::<Plan>(c, budget)?.1);
    }
    budget.post = true;
    let mut post = Vec::with_capacity(a.post.len());
    for c in &a.post {
        post.push(bool_expr::<Plan>(c, budget)?.1);
    }
    budget.post = false;
    let dg = std::mem::take(&mut budget.defs_plan);
    let mut sizes = Vec::with_capacity(updates.len());
    for u in updates {
        sizes.push(match u {
            Upd::Int(_, e) => int_expr::<Plan>(e, budget)?.1,
            Upd::Laid(var, e) => {
                let l = collections::lay_at::<Plan>(e, &var.ty, budget)?;
                if <Plan as collections::LayOps>::l_width(&l) != var.slots.len() {
                    return no(Unlowerable::NonIntegerValue, e.span);
                }
                <Plan as collections::LayOps>::l_measure(&l)
            }
        });
    }
    let du = std::mem::take(&mut budget.defs_plan);
    Ok(ActionPlan {
        guard,
        post,
        dg,
        du,
        updates: sizes,
        work: budget.predicted,
        scratch: budget.scratch,
    })
}

/// The per-instance and closing sizes of an action's plan (bn-23hzh), in the order the
/// build spends them: the guard `clauses ++ dg ++ du` conjoined, and for the definedness
/// predicate `A#defined` a charged copy of each `dg` item and, when `du` has items, the
/// item `G => du` over copies of the guard clauses `G` and of `du`.
struct ActionShape {
    /// The conjoined guard: its tree measure, its `conjoined_size`, and its work.
    guard: M,
    guard_cs: usize,
    guard_work: u128,
    /// Per instance: the predicate items, their output, work, and deepest depth.
    pred_items: u128,
    pred_size: usize,
    pred_work: u128,
    pred_depth: usize,
}

fn action_shape(plan: &ActionPlan) -> ActionShape {
    let sum = |ms: &[M]| ms.iter().fold(0_usize, |a, m| a.saturating_add(m.size));
    let deep = |ms: &[M]| ms.iter().map(|m| m.depth).max().unwrap_or(0);
    let (dg, du) = (plan.dg, plan.du);
    let ng = plan.guard.len() as u128;
    let n = ng
        .saturating_add(plan.post.len() as u128)
        .saturating_add(dg.items)
        .saturating_add(du.items);
    let n_size = usize::try_from(n).unwrap_or(usize::MAX);
    let body = sum(&plan.guard)
        .saturating_add(sum(&plan.post))
        .saturating_add(dg.size)
        .saturating_add(du.size);
    let (guard, guard_cs) = if n == 0 {
        (M { size: 1, depth: 1 }, 1)
    } else {
        (
            M {
                size: body.saturating_add(n_size - 1),
                depth: deep(&plan.guard)
                    .max(deep(&plan.post))
                    .max(dg.depth)
                    .max(du.depth)
                    .saturating_add(levels(n)),
            },
            body.saturating_add(n_size),
        )
    };
    let has_du = du.items > 0;
    let (g_size, g_depth) = if ng == 0 {
        (1, 1)
    } else {
        (
            sum(&plan.guard).saturating_add(plan.guard.len() - 1),
            deep(&plan.guard).saturating_add(levels(ng)),
        )
    };
    let du_fold = collections::fold_measure(du);
    let (pred_size, pred_work, pred_depth) = if has_du {
        (
            dg.size
                .saturating_add(g_size)
                .saturating_add(du_fold.size)
                .saturating_add(1),
            (dg.size as u128)
                .saturating_add(sum(&plan.guard) as u128)
                .saturating_add(ng.max(1))
                .saturating_add(du.size as u128)
                .saturating_add(du.items)
                .saturating_add(1),
            dg.depth.max(g_depth.max(du_fold.depth).saturating_add(1)),
        )
    } else {
        (dg.size, dg.size as u128, dg.depth)
    };
    ActionShape {
        guard,
        guard_cs,
        guard_work: n.max(1),
        pred_items: dg.items.saturating_add(u128::from(has_du)),
        pred_size,
        pred_work,
        pred_depth,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_action(
    mut builder: ModelBuilder,
    a: &crate::norm::Action,
    space: &[(i64, i64)],
    instances: u128,
    width: usize,
    plan: ActionPlan,
    updates: &[Upd<'_>],
    relational: &[&Variable],
    budget: &mut Meter,
    lowered: &mut Lowered,
    predicates: &mut Lowered,
) -> R<ModelBuilder> {
    let shape = action_shape(&plan);
    // Per instance: the guard and updates as planned, the action record with its name
    // and its update targets' names (plain actions append it; a relational action's
    // candidates are records charged by `relational_action`), the definedness
    // predicate's items, the scratch output, and a scope lookup per parameter.
    let targets = updates.iter().fold(0_usize, |acc, u| {
        acc.saturating_add(match u {
            Upd::Int(v, _) => crate::budget::text_cost(v.len()),
            Upd::Laid(var, _) => target_names(var.slots.iter().map(|(n, _, _)| n.as_str())),
        })
    });
    let record = if relational.is_empty() {
        action_record(width, targets)
    } else {
        0
    };
    let scratch = usize::try_from(plan.scratch).unwrap_or(usize::MAX);
    let per_size = plan
        .updates
        .iter()
        .fold(shape.guard.size, |acc, m| acc.saturating_add(m.size))
        .saturating_add(record)
        .saturating_add(shape.pred_size)
        .saturating_add(scratch);
    // Each composite parameter's longest text, for its label (bn-23hzh).
    let mut widest = Vec::with_capacity(a.params.len());
    for p in &a.params {
        widest.push(if layout::is_scalar(&p.ty) {
            0
        } else {
            collections::widest_text(&p.ty, MAX_IDENT_BYTES, budget, a.span)?
        });
    }
    // Per parameter: its scope entry and its value's text lookup, each by the name's
    // length, exactly as the build spends them.
    let mut params_work: u128 = 0;
    for (p, w) in a.params.iter().zip(&widest) {
        params_work =
            params_work.saturating_add(u128::from(param_work(p, space.len(), *w, budget)));
    }
    let per_work = plan
        .work
        .saturating_add(shape.guard_work)
        .saturating_add(record as u128)
        .saturating_add(params_work)
        .saturating_add(shape.pred_work)
        .saturating_add(1);
    let depth = plan
        .updates
        .iter()
        .map(|m| m.depth)
        .fold(shape.guard.depth.max(shape.pred_depth), usize::max);
    // The definedness predicate over every instance: its items conjoined, its name's
    // bytes, and its record.
    let pred_total = instances.saturating_mul(shape.pred_items);
    let name_len = a.name.len().saturating_add(DEFINED_SUFFIX.len());
    let (close_size, close_work, close_depth) = if pred_total == 0 {
        (0, 0, 0)
    } else {
        (
            usize::try_from(pred_total - 1)
                .unwrap_or(usize::MAX)
                .saturating_add(1 + crate::budget::text_cost(name_len)),
            pred_total.saturating_add(crate::budget::text_cost(name_len) as u128),
            shape.pred_depth.saturating_add(levels(pred_total)),
        )
    };
    let total_size = usize::try_from(
        instances
            .saturating_mul(per_size as u128)
            .saturating_add(close_size as u128),
    )
    .unwrap_or(usize::MAX);
    budget.predicted = instances
        .saturating_mul(per_work)
        .saturating_add(close_work);
    // The checks in the order the RFC states: depth, then work, then output.
    if depth.max(close_depth) > MAX_EXPR_DEPTH {
        return no(Unlowerable::ExpressionTooDeep, a.span);
    }
    if !relational.is_empty() {
        // The candidates of every instance, checked in aggregate before the first. Each
        // instance's template is measured as `relational_action` measures it
        // ([`conjoined_size`]: the clause count plus sizes, one more than the conjoined
        // tree, or `1` for the `true` of no clause), the updates with their target names, and
        // per relational variable a constant and its target name.
        let candidates = candidate_count(relational);
        let template = plan
            .updates
            .iter()
            .fold(shape.guard_cs, |acc, m| acc.saturating_add(m.size))
            .saturating_add(targets)
            .saturating_add(candidate_constants(relational));
        let each = successor_work(candidates, template, widest_label(width, relational));
        let all = instances.saturating_mul(u128::from(each));
        if all.saturating_add(budget.predicted) > u128::from(budget.fuel.left()) {
            return no(Unlowerable::WorkLimitExceeded, a.span);
        }
        // The candidates' work is spent inside the site: it is part of the prediction.
        budget.predicted = budget.predicted.saturating_add(all);
        if all.saturating_add(total_size as u128) > budget.nodes.left() as u128 {
            return no(Unlowerable::OutputTooLarge, a.span);
        }
    }
    begin_site(budget, total_size, depth, a.span)?;

    let mut values: Vec<i64> = space.iter().map(|(lo, _)| *lo).collect();
    if instances == 0 {
        return Ok(builder);
    }
    let mut defined: Vec<Sized<BoolExpr>> = Vec::new();
    loop {
        // This instance's parameter values, and its name.
        let mut label = a.name.clone();
        if !a.params.is_empty() {
            let mut parts = Vec::with_capacity(a.params.len());
            for ((p, v), w) in a.params.iter().zip(&values).zip(&widest) {
                burn(budget, lookup_cost(space.len(), p.name.len()), a.span)?;
                budget.env.params.insert(p.name.clone(), (*v, *v));
                parts.push(format!(
                    "{}={}",
                    p.name,
                    value_text(&p.ty, *v, *w, budget, a.span)?
                ));
            }
            label = format!("{}({})", a.name, parts.join(","));
        }
        budget.defs.clear();
        let mut guard_clauses = Vec::with_capacity(a.guard.len());
        for c in &a.guard {
            guard_clauses.push(bool_expr::<Build>(c, budget)?);
        }
        budget.post = true;
        let mut posts = Vec::with_capacity(a.post.len());
        for c in &a.post {
            posts.push(bool_expr::<Build>(c, budget)?);
        }
        budget.post = false;
        let dg = std::mem::take(&mut budget.defs);
        let mut assigns: Vec<(&str, Sized<IntExpr>)> = Vec::with_capacity(updates.len());
        for u in updates {
            match u {
                Upd::Int(v, e) => assigns.push((v, int_expr::<Build>(e, budget)?)),
                Upd::Laid(var, e) => {
                    let slots = collections::lay_at::<Build>(e, &var.ty, budget)?;
                    if slots.len() != var.slots.len() {
                        return no(Unlowerable::NonIntegerValue, e.span);
                    }
                    for ((name, _, _), x) in var.slots.iter().zip(slots) {
                        assigns.push((name.as_str(), x));
                    }
                }
            }
        }
        let du = std::mem::take(&mut budget.defs);
        // The definedness predicate's items: where the guard's reads are defined, and
        // where the guard clauses hold, the updates' reads are too (RFC 0003 correction 4,
        // "Definedness"; a relational action's updates are evaluated where its guard
        // clauses hold, since they read no post-state).
        for d in &dg {
            defined.push(copy::<Build, _>(budget, a.span, d)?);
        }
        if !du.is_empty() {
            let mut g = Vec::with_capacity(guard_clauses.len());
            for c in &guard_clauses {
                g.push(copy::<Build, _>(budget, a.span, c)?);
            }
            let g = if g.is_empty() {
                node::<Build, _>(budget, a.span, &[], || BoolExpr::Const(true))?
            } else {
                balanced::<Build>(g, true, budget, a.span)?
            };
            let mut d = Vec::with_capacity(du.len());
            for x in &du {
                d.push(copy::<Build, _>(budget, a.span, x)?);
            }
            let d = balanced::<Build>(d, true, budget, a.span)?;
            defined.push(node::<Build, _>(budget, a.span, &[g.1, d.1], || {
                BoolExpr::implies(g.0, d.0)
            })?);
        }
        // The guard: the clauses, the postconditions, and the definedness conditions,
        // so no successor is computed from an undefined read.
        let mut clauses = guard_clauses;
        clauses.extend(posts);
        clauses.extend(dg);
        clauses.extend(du);
        let guard_size = conjoined_size(&clauses);
        let guard = conjoin(clauses, budget, a.span)?;
        if relational.is_empty() {
            // The record the builder appends: prepaid by the plan (`record`), spent
            // here with the work of copying its names.
            let record = action_record(label.len(), targets);
            burn(budget, record as u64, a.span)?;
            charge(budget, record, a.span)?;
            let assigns = assigns.into_iter().map(|(v, x)| (v, x.0)).collect();
            builder = builder.action(ActionDecl::deterministic(&label, guard, assigns));
            lowered.add(&label);
        } else {
            // The candidates are charged by `relational_action` itself, not from the
            // plan's prepaid output.
            let prepaid = std::mem::take(&mut budget.prepaid);
            let in_site = std::mem::replace(&mut budget.in_site, false);
            let built = relational_action(
                builder,
                &label,
                (guard, guard_size),
                &assigns,
                relational,
                budget,
                a.span,
                lowered,
            );
            budget.prepaid = prepaid;
            budget.in_site = in_site;
            builder = built?;
        }
        if !advance_space(&mut values, space) {
            break;
        }
    }
    if !defined.is_empty() {
        let name = definedness_name(&a.name, budget, a.span)?;
        let body = conjoin(defined, budget, a.span)?;
        charge(budget, 1 + crate::budget::text_cost(name.len()), a.span)?;
        builder = builder.predicate(&name, body);
        predicates.add(&name);
    }
    Ok(builder)
}

/// Step `values` to the next parameter tuple of `space`, last position fastest.
/// Returns `false` after the last tuple.
fn advance_space(values: &mut [i64], space: &[(i64, i64)]) -> bool {
    for (slot, (lo, hi)) in values.iter_mut().zip(space).rev() {
        if *slot < *hi {
            *slot = slot.saturating_add(1);
            return true;
        }
        *slot = *lo;
    }
    false
}

/// Step `values` to the next vector of the domain product, last position fastest.
/// Returns `false` after the last vector.
fn advance<V: std::borrow::Borrow<Variable>>(values: &mut [i64], variables: &[V]) -> bool {
    for (slot, v) in values
        .iter_mut()
        .zip(variables.iter().map(Borrow::borrow))
        .rev()
    {
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

/// The finite domain of an integer state variable: its type's range — `Nat` from `0`,
/// narrowed to a run configuration's explicit bound when it gives one (correction 4) —
/// intersected with every interval clause of its refinement. Constant work per clause.
fn domain_of(
    name: &str,
    ty: &Type,
    refinement: &[Expr],
    span: Span,
    bind: &Bindings,
) -> R<(i64, i64)> {
    let (mut lo, mut hi): (Option<i64>, Option<i64>) = match ty {
        Type::Nat => (Some(0), bind.bounds.nat_max()),
        Type::Int => match bind.bounds.int_range() {
            Some((a, b)) => (Some(a), Some(b)),
            None => (None, None),
        },
        _ => return no(Unlowerable::NonIntegerState, span),
    };
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
        // A configured lowering could have bounded the type: the refusal names it.
        _ if bind.configured => no(Unlowerable::UnboundedType, span),
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

// ---------------------------------------------------------------------------
// expressions: one lowering, run twice — to plan, then to build
// ---------------------------------------------------------------------------

/// What one pass over a normalized expression produces.
///
/// The lowering of an expression is written once, generic over its mode. [`Plan`]
/// builds nothing: it returns the measure ([`M`]) the build will have and adds the work
/// the build will spend to [`Meter::predicted`], without charging the output budget. A
/// quantifier is planned once, with its binder's whole interval, and its measure and
/// work are multiplied by its fan-out. [`Build`] builds the expression and charges as
/// it goes. Because both run the same code, the plan of an expression without a
/// quantifier is exactly its build, and with one it is an upper bound (RFC 0003
/// correction 4, "Resources").
trait Mode: collections::LayOps {
    const BUILD: bool;
    fn i_const(v: i64) -> Self::I;
    /// A state variable (or a post-state placeholder) whose domain is `lo..=hi`.
    fn i_var(name: &str, lo: i64, hi: i64) -> Self::I;
    /// A parameter or bound variable in scope: a point while building, the interval
    /// `lo..=hi` while planning.
    fn i_scoped(lo: i64, hi: i64) -> Self::I;
    /// Whether every value of `i`, and every intermediate result computing it, fits
    /// `i64`: always while building (the build does not decide it), from the interval
    /// while planning.
    fn fits(i: &Self::I) -> bool;
    fn i_arith(op: ArithOp, a: Self::I, b: Self::I) -> Self::I;
    fn i_min(a: Self::I, b: Self::I) -> Self::I;
    fn i_max(a: Self::I, b: Self::I) -> Self::I;
    fn b_const(v: bool) -> Self::B;
    fn b_cmp(op: CmpOp, a: Self::I, b: Self::I) -> Self::B;
    fn b_not(a: Self::B) -> Self::B;
    fn b_and(a: Self::B, b: Self::B) -> Self::B;
    fn b_or(a: Self::B, b: Self::B) -> Self::B;
    fn b_implies(a: Self::B, b: Self::B) -> Self::B;
    fn b_range(x: Self::I, lo: i64, hi: i64) -> Self::B;
}

/// Build the expressions.
enum Build {}

/// Measure them only.
enum Plan {}

impl Mode for Build {
    const BUILD: bool = true;
    fn i_const(v: i64) -> IntExpr {
        IntExpr::Const(v)
    }
    fn i_var(name: &str, _: i64, _: i64) -> IntExpr {
        IntExpr::var(name)
    }
    fn i_scoped(lo: i64, _: i64) -> IntExpr {
        IntExpr::Const(lo)
    }
    fn fits(_: &IntExpr) -> bool {
        true
    }
    fn i_arith(op: ArithOp, a: IntExpr, b: IntExpr) -> IntExpr {
        IntExpr::Arith(op, Box::new(a), Box::new(b))
    }
    fn i_min(a: IntExpr, b: IntExpr) -> IntExpr {
        IntExpr::min(a, b)
    }
    fn i_max(a: IntExpr, b: IntExpr) -> IntExpr {
        IntExpr::max(a, b)
    }
    fn b_const(v: bool) -> BoolExpr {
        BoolExpr::Const(v)
    }
    fn b_cmp(op: CmpOp, a: IntExpr, b: IntExpr) -> BoolExpr {
        BoolExpr::compare(op, a, b)
    }
    fn b_not(a: BoolExpr) -> BoolExpr {
        BoolExpr::negate(a)
    }
    fn b_and(a: BoolExpr, b: BoolExpr) -> BoolExpr {
        BoolExpr::and(a, b)
    }
    fn b_or(a: BoolExpr, b: BoolExpr) -> BoolExpr {
        BoolExpr::or(a, b)
    }
    fn b_implies(a: BoolExpr, b: BoolExpr) -> BoolExpr {
        BoolExpr::implies(a, b)
    }
    fn b_range(x: IntExpr, lo: i64, hi: i64) -> BoolExpr {
        BoolExpr::in_range(x, lo, hi)
    }
}

/// The plan's integer: the interval of values the built expression takes, computed
/// node by node; `None` after an `i128` overflow (which does not fit).
impl Mode for Plan {
    const BUILD: bool = false;
    fn i_const(v: i64) -> Option<Iv> {
        Some(Iv::point(v))
    }
    fn i_var(_: &str, lo: i64, hi: i64) -> Option<Iv> {
        Some(Iv::range(lo, hi))
    }
    fn i_scoped(lo: i64, hi: i64) -> Option<Iv> {
        Some(Iv::range(lo, hi))
    }
    fn fits(i: &Option<Iv>) -> bool {
        i.is_some_and(|iv| iv.fits)
    }
    fn i_arith(op: ArithOp, a: Option<Iv>, b: Option<Iv>) -> Option<Iv> {
        iv_arith(op, a?, b?)
    }
    fn i_min(a: Option<Iv>, b: Option<Iv>) -> Option<Iv> {
        Some(iv_min_max(true, a?, b?))
    }
    fn i_max(a: Option<Iv>, b: Option<Iv>) -> Option<Iv> {
        Some(iv_min_max(false, a?, b?))
    }
    fn b_const(_: bool) {}
    fn b_cmp(_: CmpOp, _: Option<Iv>, _: Option<Iv>) {}
    fn b_not((): ()) {}
    fn b_and((): (), (): ()) {}
    fn b_or((): (), (): ()) {}
    fn b_implies((): (), (): ()) {}
    fn b_range(_: Option<Iv>, _: i64, _: i64) {}
}

/// Work: spent while building; while planning, added to the prediction, and one unit
/// of the plan's own effort spent.
fn work<Md: Mode>(budget: &mut Meter, n: u64, span: Span) -> R<()> {
    if Md::BUILD {
        burn(budget, n, span)
    } else {
        budget.predicted = budget.predicted.saturating_add(u128::from(n));
        predicted_fits(budget, span)?;
        burn(budget, 1, span)
    }
}

/// Refuse as soon as a plan's predicted work passes what the work budget has left: a
/// site's prediction only grows, so the plan stops at the first step that decides the
/// refusal instead of after the whole site (cr-3aqchd).
fn predicted_fits(budget: &Meter, span: Span) -> R<()> {
    if budget.predicted > u128::from(budget.fuel.left()) {
        return no(Unlowerable::WorkLimitExceeded, span);
    }
    Ok(())
}

/// Work that this pass performs itself, in either mode (a sort of a domain's members):
/// spent before it is done, and while planning also predicted for the build, which does
/// it again.
fn effort<Md: Mode>(budget: &mut Meter, n: u64, span: Span) -> R<()> {
    if !Md::BUILD {
        budget.predicted = budget.predicted.saturating_add(u128::from(n));
        predicted_fits(budget, span)?;
    }
    burn(budget, n, span)
}

/// Output: charged while building (from what the site's plan prepaid, see
/// [`begin_site`]); nothing while planning.
fn charge_in<Md: Mode>(budget: &mut Meter, n: usize, span: Span) -> R<()> {
    if Md::BUILD {
        charge(budget, n, span)
    } else {
        Ok(())
    }
}

/// Charge `n` output nodes, or refuse with [`Unlowerable::OutputTooLarge`]. Inside a
/// planned site the nodes come from what the plan prepaid; a build that needed more
/// than its plan would be a planning defect, which is still charged (and caught by a
/// debug assertion).
fn charge(budget: &mut Meter, n: usize, span: Span) -> R<()> {
    let from_plan = n.min(budget.prepaid);
    budget.prepaid -= from_plan;
    let rest = n - from_plan;
    if rest > 0 && budget.in_site {
        budget.overdrawn = true;
    }
    budget.nodes.charge(rest).map_err(|_| LowerError {
        kind: LowerErrorKind::Unlowerable(Unlowerable::OutputTooLarge),
        span,
    })
}

/// Open a planned site: refuse a planned depth past [`MAX_EXPR_DEPTH`], refuse
/// predicted work past what the work budget has left, then charge the planned output
/// in one step. The build that follows draws its nodes from that charge and spends its
/// work as it runs.
fn begin_site(budget: &mut Meter, size: usize, depth: usize, span: Span) -> R<()> {
    if depth > MAX_EXPR_DEPTH {
        return no(Unlowerable::ExpressionTooDeep, span);
    }
    if budget.predicted > u128::from(budget.fuel.left()) {
        return no(Unlowerable::WorkLimitExceeded, span);
    }
    budget.nodes.charge(size).map_err(|_| LowerError {
        kind: LowerErrorKind::Unlowerable(Unlowerable::OutputTooLarge),
        span,
    })?;
    budget.prepaid = size;
    budget.in_site = true;
    budget.site_work = budget.fuel.used();
    budget.site_predicted = budget.predicted;
    Ok(())
}

/// Close a site. What the plan over-estimated stays charged: the plan is an upper
/// bound, and output is never refunded. Work is treated the same way (bn-23hzh): the
/// part of the prediction the build did not spend is spent here, so a site always
/// spends exactly its prediction and a lowering replays under the limits it reported
/// (`begin_site` checked the whole prediction against the work left, so it fits).
fn end_site(budget: &mut Meter) {
    debug_assert!(
        !budget.overdrawn,
        "a build needed more output than its plan"
    );
    if budget.in_site {
        let spent = u128::from(budget.fuel.used().saturating_sub(budget.site_work));
        debug_assert!(
            spent <= budget.site_predicted,
            "a build spent more work than its plan predicted"
        );
        let rest = budget.site_predicted.saturating_sub(spent);
        let rest = u64::try_from(rest)
            .unwrap_or(u64::MAX)
            .min(budget.fuel.left());
        let _ = budget.fuel.burn(rest);
    }
    budget.prepaid = 0;
    budget.in_site = false;
    budget.overdrawn = false;
}

/// Lower one clause (an init or invariant conjunct) as its own planned site, with the
/// definedness items its map reads record (bn-23hzh). The site's planned output is the
/// clause's measure, the scratch output its build charges beyond it, and the items.
fn top_bool(e: &Expr, budget: &mut Meter) -> R<(Sized<BoolExpr>, Vec<Sized<BoolExpr>>)> {
    shallow(e, budget)?;
    budget.predicted = 0;
    budget.scratch = 0;
    budget.defs_plan = collections::DAgg::default();
    let planned = bool_expr::<Plan>(e, budget)?.1;
    let defs = std::mem::take(&mut budget.defs_plan);
    let size = (planned.size as u128)
        .saturating_add(budget.scratch)
        .saturating_add(defs.size as u128);
    let depth = planned.depth.max(defs.depth);
    begin_site(
        budget,
        usize::try_from(size).unwrap_or(usize::MAX),
        depth,
        e.span,
    )?;
    let outer = std::mem::take(&mut budget.defs);
    let built = bool_expr::<Build>(e, budget);
    let items = std::mem::replace(&mut budget.defs, outer);
    end_site(budget);
    Ok((built?, items))
}

/// Conjoin lowered clauses as a *balanced* tree, so `n` clauses add `⌈log₂ n⌉` levels
/// rather than `n - 1`. The result's depth is computed from the clause measures and
/// checked against [`MAX_EXPR_DEPTH`], and the `&&` nodes are charged, before any node
/// is built. Pairing is adjacent and left to right, so the tree is a function of the
/// clause order alone (and for two or three clauses equals the left fold).
fn conjoin(clauses: Vec<Sized<BoolExpr>>, budget: &mut Meter, span: Span) -> R<BoolExpr> {
    if clauses.is_empty() {
        // The implicit `true` is a node like any other: charged (cr-3aqchd).
        return Ok(node::<Build, _>(budget, span, &[], || BoolExpr::Const(true))?.0);
    }
    Ok(balanced::<Build>(clauses, true, budget, span)?.0)
}

/// An upper bound on the nodes of [`conjoin`]`(clauses)`, for the work of evaluating or
/// copying it: the clause count plus their sizes (one more than the tree's `n − 1`
/// combining nodes), and `1` for no clause — the `true` node `conjoin` builds, which is
/// evaluated and copied like any other (cr-3aqchd).
fn conjoined_size(clauses: &[Sized<BoolExpr>]) -> usize {
    clauses
        .iter()
        .fold(clauses.len(), |acc, c| acc.saturating_add(c.1.size))
        .max(1)
}

/// `⌈log₂ n⌉`: the levels a balanced tree over `n` leaves adds.
fn levels(n: u128) -> usize {
    let mut levels = 0_usize;
    let mut width = n;
    while width > 1 {
        width = width.div_ceil(2);
        levels = levels.saturating_add(1);
    }
    levels
}

/// The measure of a balanced tree over `n >= 1` leaves of measure at most `leaf`.
fn balanced_measure(n: u128, leaf: M) -> M {
    let n_size = usize::try_from(n).unwrap_or(usize::MAX);
    M {
        size: n_size
            .saturating_mul(leaf.size)
            .saturating_add(n_size.saturating_sub(1)),
        depth: leaf.depth.saturating_add(levels(n)),
    }
}

/// A balanced `&&` (or `||`) tree over `items` (at least one), adjacent pairs left to
/// right. Its depth is checked, its `n - 1` nodes charged, and `n` units of work spent,
/// before any node is built.
fn balanced<Md: Mode>(
    items: Vec<Sized<Md::B>>,
    and: bool,
    budget: &mut Meter,
    span: Span,
) -> R<Sized<Md::B>> {
    let n = items.len();
    let deepest = items.iter().map(|c| c.1.depth).max().unwrap_or(0);
    let size = items
        .iter()
        .fold(n.saturating_sub(1), |acc, c| acc.saturating_add(c.1.size));
    let depth = deepest.saturating_add(levels(n as u128));
    if depth > MAX_EXPR_DEPTH {
        return no(Unlowerable::ExpressionTooDeep, span);
    }
    charge_in::<Md>(budget, n.saturating_sub(1), span)?;
    work::<Md>(budget, n as u64, span)?;
    let mut round: Vec<Md::B> = items.into_iter().map(|c| c.0).collect();
    while round.len() > 1 {
        let mut next: Vec<Md::B> = Vec::with_capacity(round.len().div_ceil(2));
        let mut items = round.into_iter();
        while let Some(left) = items.next() {
            match items.next() {
                Some(right) => next.push(if and {
                    Md::b_and(left, right)
                } else {
                    Md::b_or(left, right)
                }),
                None => next.push(left),
            }
        }
        round = next;
    }
    match round.pop() {
        Some(b) => Ok((b, M { size, depth })),
        None => node::<Md, _>(budget, span, &[], || Md::b_const(and)),
    }
}

/// Why a non-integer, non-boolean expression does not lower.
fn reason(e: &Expr) -> Unlowerable {
    match &e.kind {
        ExprKind::Const(_) => Unlowerable::Constant,
        ExprKind::Quant(..)
        | ExprKind::SetComp(..)
        | ExprKind::MapComp(..)
        | ExprKind::Bound { .. } => Unlowerable::Quantifier,
        ExprKind::Param(_) => Unlowerable::ParameterizedAction,
        ExprKind::Binary(BinOp::Div | BinOp::Mod, ..) => Unlowerable::DivisionOrModulo,
        ExprKind::Recur { .. } => Unlowerable::RecursiveCall,
        ExprKind::Primed(_) => Unlowerable::PrimedOutsidePostcondition,
        _ => Unlowerable::NonIntegerValue,
    }
}

/// Refuse an expression too deep to lower, before recursing into it.
///
/// Every pass over a normalized expression — the plan, the build, and the interval of a
/// quantifier domain — recurses along the source tree, so this bound is their stack
/// bound. A lowered expression without a quantifier is never shallower than its
/// normalized form less one level (an `in a..b` becomes one range node), so anything
/// deeper than `MAX_EXPR_DEPTH + 1` would be refused by the builder anyway; a quantifier
/// only adds levels, except over an empty domain, whose deep source is refused here too.
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

/// One new node over already-lowered children. Its depth is checked against
/// [`MAX_EXPR_DEPTH`] and the node is charged *before* `build` runs, so no lowered
/// expression deeper than the model admits is ever constructed (and none needs a deep
/// recursive drop).
fn node<Md: Mode, T>(
    budget: &mut Meter,
    span: Span,
    parts: &[M],
    build: impl FnOnce() -> T,
) -> R<Sized<T>> {
    // Building the node, and (for the builder's validation) scanning the declared
    // variables when it names one, is charged as work.
    work::<Md>(budget, 1, span)?;
    let depth = parts
        .iter()
        .map(|m| m.depth)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    if depth > MAX_EXPR_DEPTH {
        return no(Unlowerable::ExpressionTooDeep, span);
    }
    charge_in::<Md>(budget, 1, span)?;
    let size = parts
        .iter()
        .fold(1_usize, |acc, m| acc.saturating_add(m.size));
    Ok((build(), M { size, depth }))
}

/// Whether `ty` lowers to an integer: `Int`, `Nat`, a sort the configuration
/// instantiates, or an enumeration (its variant index).
fn int_like(ty: &Type, budget: &Meter) -> bool {
    match ty {
        Type::Sort(s) => budget.bind.sorts.contains_key(s),
        Type::Enum(e) => budget.enums.contains_key(e),
        other => other.is_integer(),
    }
}

/// [`reason`], except that a constant a run configuration has bound (to a value of a
/// type that does not lower) is a non-integer value, not a missing configuration.
fn reason_in(e: &Expr, budget: &Meter) -> Unlowerable {
    match &e.kind {
        ExprKind::Const(_) if budget.bind.configured => Unlowerable::NonIntegerValue,
        _ => reason(e),
    }
}

/// The value of an action parameter or bound variable in scope: a point while building,
/// the lowest value of its interval while planning (the plan's measure does not depend
/// on it). Charged as one ordered-table lookup.
fn scoped<Md: Mode>(e: &Expr, budget: &mut Meter, span: Span) -> R<Option<(i64, i64)>> {
    Ok(match &e.kind {
        ExprKind::Param(p) => {
            let cost = lookup_cost(budget.env.params.len(), p.len());
            work::<Md>(budget, cost, span)?;
            budget.env.params.get(p).copied()
        }
        ExprKind::Bound { binder, .. } => {
            let cost = lookup_cost(budget.env.binders.len(), 8);
            work::<Md>(budget, cost, span)?;
            budget.env.binders.get(binder).copied()
        }
        _ => None,
    })
}

/// Lower an integer expression. While planning inside a guarded quantifier instance
/// (see [`instance`]), every integer node the build will contain is checked here, as it
/// is planned: one whose interval may leave `i64` — the node itself or any
/// intermediate result below it — is [`Unlowerable::GuardedOverflow`]. This covers
/// everything a guarded instance evaluates, because everything it evaluates is built
/// by this function: its body, the bounds and members in the guards of nested
/// quantifiers, the operands of membership tests, and the operands of comparisons.
fn int_expr<Md: Mode>(e: &Expr, budget: &mut Meter) -> R<Sized<Md::I>> {
    let lowered = int_node::<Md>(e, budget)?;
    if !Md::BUILD && budget.guarded > 0 && !Md::fits(&lowered.0) {
        return no(Unlowerable::GuardedOverflow, e.span);
    }
    Ok(lowered)
}

fn int_node<Md: Mode>(e: &Expr, budget: &mut Meter) -> R<Sized<Md::I>> {
    // A sort- or enumeration-typed node costs a lookup in its table (by the name's
    // length), charged first.
    match &e.ty {
        Type::Sort(s) => {
            let cost = lookup_cost(budget.bind.sorts.len(), s.len());
            work::<Md>(budget, cost, e.span)?;
        }
        Type::Enum(n) => {
            let cost = lookup_cost(budget.enums.len(), n.len());
            work::<Md>(budget, cost, e.span)?;
        }
        _ => {}
    }
    if !int_like(&e.ty, budget) {
        return no(reason_in(e, budget), e.span);
    }
    let sp = e.span;
    match &e.kind {
        ExprKind::Int(n) => node::<Md, _>(budget, sp, &[], || Md::i_const(*n)),
        // A constant the run configuration binds to an integer, a sort element, or a
        // variant.
        // The lookup is charged before it is made, bound or not.
        ExprKind::Const(c) => {
            let cost = lookup_cost(budget.bind.ints.len(), c.len());
            work::<Md>(budget, cost, sp)?;
            match budget.bind.ints.get(c).copied() {
                Some(v) => node::<Md, _>(budget, sp, &[], || Md::i_const(v)),
                None => no(reason_in(e, budget), sp),
            }
        }
        // A variant: its index in declaration order.
        ExprKind::Variant {
            enumeration,
            variant,
        } => {
            let Some(v) = variant_index::<Md>(budget, enumeration, variant, sp)? else {
                return no(Unlowerable::NonIntegerValue, sp);
            };
            node::<Md, _>(budget, sp, &[], || Md::i_const(v))
        }
        // An action parameter or a quantifier's bound variable: its value.
        ExprKind::Param(_) | ExprKind::Bound { .. } => match scoped::<Md>(e, budget, sp)? {
            Some((lo, hi)) => node::<Md, _>(budget, sp, &[], || Md::i_scoped(lo, hi)),
            None => no(reason(e), sp),
        },
        // The name is owned text: its cost travels with the node into every copy.
        ExprKind::State(v) => {
            let text = crate::budget::text_cost(v.len());
            charge_in::<Md>(budget, text, sp)?;
            let scan = budget.vars as u64;
            work::<Md>(budget, scan, sp)?;
            let (lo, hi) = domain_in::<Md>(budget, v, sp)?;
            node::<Md, _>(budget, sp, &[M::text(text)], || Md::i_var(v, lo, hi))
        }
        ExprKind::Neg(a) => {
            let (a, sa) = int_expr::<Md>(a, budget)?;
            node::<Md, _>(budget, sp, &[sa, M { size: 1, depth: 1 }], || {
                Md::i_arith(ArithOp::Sub, Md::i_const(0), a)
            })
        }
        ExprKind::Binary(op @ (BinOp::Add | BinOp::Sub | BinOp::Mul), a, b) => {
            let (a, sa) = int_expr::<Md>(a, budget)?;
            let (b, sb) = int_expr::<Md>(b, budget)?;
            let op = match op {
                BinOp::Add => ArithOp::Add,
                BinOp::Sub => ArithOp::Sub,
                _ => ArithOp::Mul,
            };
            node::<Md, _>(budget, sp, &[sa, sb], || Md::i_arith(op, a, b))
        }
        ExprKind::Builtin(Builtin::Min, args) | ExprKind::Builtin(Builtin::Max, args) => {
            let [a, b] = args.as_slice() else {
                return no(Unlowerable::NonIntegerValue, e.span);
            };
            let (a, sa) = int_expr::<Md>(a, budget)?;
            let (b, sb) = int_expr::<Md>(b, budget)?;
            let min = matches!(e.kind, ExprKind::Builtin(Builtin::Min, _));
            node::<Md, _>(budget, sp, &[sa, sb], || {
                if min {
                    Md::i_min(a, b)
                } else {
                    Md::i_max(a, b)
                }
            })
        }
        // A post-state read, inside a postcondition: the placeholder the candidate
        // enumeration replaces by each candidate value.
        ExprKind::Primed(v) if budget.post => {
            let cost = lookup_cost(budget.slots.len(), v.len());
            work::<Md>(budget, cost, sp)?;
            let Some(slot) = budget.slots.get(v).copied() else {
                return no(Unlowerable::PrimedOutsidePostcondition, sp);
            };
            let name = primed_placeholder(slot);
            let text = crate::budget::text_cost(name.len());
            charge_in::<Md>(budget, text, sp)?;
            let (lo, hi) = domain_in::<Md>(budget, v, sp)?;
            node::<Md, _>(budget, sp, &[M::text(text)], || Md::i_var(&name, lo, hi))
        }
        // `m[k]` of an integer-coded value type (bn-23hzh).
        ExprKind::Index(m, k) if matches!(m.ty, Type::Map(..)) => {
            collections::index_int::<Md>(e, m, k, budget)
        }
        ExprKind::If(..) => no(Unlowerable::ConditionalValue, e.span),
        _ => no(reason_in(e, budget), e.span),
    }
}

/// The declared domain of state variable `v`, for the plan's intervals: one lookup,
/// spent, while planning inside a guarded instance; nothing otherwise. A variable
/// with no domain (its declaration was refused) has the whole `i64` range.
fn domain_in<Md: Mode>(budget: &mut Meter, v: &str, span: Span) -> R<(i64, i64)> {
    // Only the guarded check reads a plan's intervals, so outside a guarded instance the
    // domain is not looked up (and the unread interval is the whole `i64` range).
    if Md::BUILD || budget.guarded == 0 {
        return Ok((i64::MIN, i64::MAX));
    }
    // The plan's own effort: the build does not look the domain up, so the lookup is
    // spent, not predicted.
    let cost = lookup_cost(budget.domains.len(), v.len());
    burn(budget, cost, span)?;
    Ok(budget
        .domains
        .get(v)
        .copied()
        .unwrap_or((i64::MIN, i64::MAX)))
}

/// The index of `variant` in `enumeration`, charging the two lookups first.
fn variant_index<Md: Mode>(
    budget: &mut Meter,
    enumeration: &str,
    variant: &str,
    span: Span,
) -> R<Option<i64>> {
    let cost = lookup_cost(budget.enums.len(), enumeration.len());
    work::<Md>(budget, cost, span)?;
    let Some(table) = budget.enums.get(enumeration) else {
        return Ok(None);
    };
    let cost = lookup_cost(table.index.len(), variant.len());
    work::<Md>(budget, cost, span)?;
    Ok(budget
        .enums
        .get(enumeration)
        .and_then(|t| t.index.get(variant))
        .copied())
}

/// The operand for one use of several: the original, moved, for the `last` use, and a
/// charged [`copy`] otherwise.
fn take_or_copy<Md: Mode, T: Clone>(
    slot: &mut Option<Sized<T>>,
    last: bool,
    budget: &mut Meter,
    span: Span,
) -> R<Sized<T>> {
    if last {
        return match slot.take() {
            Some(x) => Ok(x),
            None => no(Unlowerable::NonIntegerValue, span),
        };
    }
    match slot.as_ref() {
        Some(x) => copy::<Md, _>(budget, span, x),
        None => no(Unlowerable::NonIntegerValue, span),
    }
}

/// Clone a lowered subtree after charging its size: the only way this module copies.
fn copy<Md: Mode, T: Clone>(budget: &mut Meter, span: Span, x: &Sized<T>) -> R<Sized<T>> {
    charge_in::<Md>(budget, x.1.size, span)?;
    work::<Md>(budget, x.1.size as u64, span)?;
    Ok((x.0.clone(), x.1))
}

fn bool_expr<Md: Mode>(e: &Expr, budget: &mut Meter) -> R<Sized<Md::B>> {
    if e.ty != Type::Bool {
        return no(reason(e), e.span);
    }
    let sp = e.span;
    match &e.kind {
        ExprKind::Bool(b) => node::<Md, _>(budget, sp, &[], || Md::b_const(*b)),
        // A constant the run configuration binds to a Boolean.
        ExprKind::Const(c) => {
            let cost = lookup_cost(budget.bind.bools.len(), c.len());
            work::<Md>(budget, cost, sp)?;
            match budget.bind.bools.get(c).copied() {
                Some(v) => node::<Md, _>(budget, sp, &[], || Md::b_const(v)),
                None => no(reason_in(e, budget), sp),
            }
        }
        // A Boolean parameter or bound variable: `0` is `false`, `1` is `true`.
        ExprKind::Param(_) | ExprKind::Bound { .. } => match scoped::<Md>(e, budget, sp)? {
            Some((v, _)) => node::<Md, _>(budget, sp, &[], || Md::b_const(v != 0)),
            None => no(reason(e), sp),
        },
        ExprKind::Quant(q, binders, body) => quantifier::<Md>(*q, binders, body, sp, budget),
        // `m[k]` of a Boolean value type (bn-23hzh).
        ExprKind::Index(m, k) if matches!(m.ty, Type::Map(..)) => {
            collections::index_bool::<Md>(e, m, k, budget)
        }
        ExprKind::Not(a) => {
            let (a, sa) = bool_expr::<Md>(a, budget)?;
            node::<Md, _>(budget, sp, &[sa], || Md::b_not(a))
        }
        ExprKind::If(c, a, b) => {
            // `(c && a) || (!c && b)` mentions `c` twice: the second copy is charged.
            let c = bool_expr::<Md>(c, budget)?;
            let (c2, sc2) = copy::<Md, _>(budget, sp, &c)?;
            let (a, sa) = bool_expr::<Md>(a, budget)?;
            let (b, sb) = bool_expr::<Md>(b, budget)?;
            let (left, sl) = node::<Md, _>(budget, sp, &[c.1, sa], || Md::b_and(c.0, a))?;
            let (nc, snc) = node::<Md, _>(budget, sp, &[sc2], || Md::b_not(c2))?;
            let (right, sr) = node::<Md, _>(budget, sp, &[snc, sb], || Md::b_and(nc, b))?;
            node::<Md, _>(budget, sp, &[sl, sr], || Md::b_or(left, right))
        }
        ExprKind::Binary(op, a, b) => {
            let cmp = |op: CmpOp, budget: &mut Meter| -> R<Sized<Md::B>> {
                let (x, sx) = int_expr::<Md>(a, budget)?;
                let (y, sy) = int_expr::<Md>(b, budget)?;
                node::<Md, _>(budget, sp, &[sx, sy], || Md::b_cmp(op, x, y))
            };
            match op {
                BinOp::And | BinOp::Or | BinOp::Implies => {
                    let (x, sx) = bool_expr::<Md>(a, budget)?;
                    let (y, sy) = bool_expr::<Md>(b, budget)?;
                    node::<Md, _>(budget, sp, &[sx, sy], || match op {
                        BinOp::And => Md::b_and(x, y),
                        BinOp::Or => Md::b_or(x, y),
                        _ => Md::b_implies(x, y),
                    })
                }
                BinOp::Iff => {
                    let x = bool_expr::<Md>(a, budget)?;
                    let y = bool_expr::<Md>(b, budget)?;
                    iff::<Md>(budget, sp, x, y)
                }
                // Composite values compare slot by slot (bn-23hzh).
                BinOp::Eq | BinOp::Ne | BinOp::SubsetEq if collections::is_composite(&a.ty) => {
                    let same = collections::lay_compare::<Md>(a, b, *op, budget, sp)?;
                    if *op == BinOp::Ne {
                        node::<Md, _>(budget, sp, &[same.1], || Md::b_not(same.0))
                    } else {
                        Ok(same)
                    }
                }
                BinOp::Eq | BinOp::Ne if a.ty == Type::Bool => {
                    let x = bool_expr::<Md>(a, budget)?;
                    let y = bool_expr::<Md>(b, budget)?;
                    let same = iff::<Md>(budget, sp, x, y)?;
                    if *op == BinOp::Eq {
                        Ok(same)
                    } else {
                        node::<Md, _>(budget, sp, &[same.1], || Md::b_not(same.0))
                    }
                }
                BinOp::Eq => cmp(CmpOp::Eq, budget),
                BinOp::Ne => cmp(CmpOp::Ne, budget),
                BinOp::Lt => cmp(CmpOp::Lt, budget),
                BinOp::Le => cmp(CmpOp::Le, budget),
                BinOp::Gt => cmp(CmpOp::Gt, budget),
                BinOp::Ge => cmp(CmpOp::Ge, budget),
                BinOp::In | BinOp::NotIn => {
                    let inside = match &b.kind {
                        ExprKind::Binary(BinOp::Range, lo, hi) => {
                            in_range::<Md>(a, lo, hi, sp, budget)?
                        }
                        _ => match collections::membership_of::<Md>(a, b, sp, budget)? {
                            Some(inside) => inside,
                            None => membership::<Md>(a, b, sp, budget)?,
                        },
                    };
                    if *op == BinOp::In {
                        Ok(inside)
                    } else {
                        node::<Md, _>(budget, sp, &[inside.1], || Md::b_not(inside.0))
                    }
                }
                _ => no(reason_in(e, budget), e.span),
            }
        }
        _ => no(reason_in(e, budget), e.span),
    }
}

/// `x in lo..hi`: one range node for constant bounds, else `lo <= x && x <= hi`.
fn in_range<Md: Mode>(
    a: &Expr,
    lo: &Expr,
    hi: &Expr,
    sp: Span,
    budget: &mut Meter,
) -> R<Sized<Md::B>> {
    let x = int_expr::<Md>(a, budget)?;
    match (constant(lo), constant(hi)) {
        (Some(lo), Some(hi)) => node::<Md, _>(budget, sp, &[x.1], || Md::b_range(x.0, lo, hi)),
        _ => {
            // `lo <= x && x <= hi` mentions `x` twice: the copy is charged.
            let (x2, sx2) = copy::<Md, _>(budget, sp, &x)?;
            let (l, sl) = int_expr::<Md>(lo, budget)?;
            let (h, sh) = int_expr::<Md>(hi, budget)?;
            let (ge, sge) = node::<Md, _>(budget, sp, &[x.1, sl], || Md::b_cmp(CmpOp::Ge, x.0, l))?;
            let (le, sle) = node::<Md, _>(budget, sp, &[sx2, sh], || Md::b_cmp(CmpOp::Le, x2, h))?;
            node::<Md, _>(budget, sp, &[sge, sle], || Md::b_and(ge, le))
        }
    }
}

/// The members of a set `x` is tested against.
enum Members<'e> {
    /// A set literal: each member is lowered.
    Exprs(&'e [Expr]),
    /// A configuration constant: its codes, shared (never copied).
    Values(Rc<[i64]>),
}

/// `x in S` for a set of scalars `S` given as a set literal or as a configuration
/// constant (RFC 0003 correction 4): the balanced disjunction of `x == s` over the
/// members — for Booleans `x <=> s`, and against a constant `x` or `!x` — and, for an
/// empty set, `!(x == x)` (Booleans: `x && !x`), which is false but still evaluates
/// `x` (cr-3aqchd). `x` is lowered once and each further use is a charged copy.
fn membership<Md: Mode>(x: &Expr, set: &Expr, sp: Span, budget: &mut Meter) -> R<Sized<Md::B>> {
    let members = match &set.kind {
        ExprKind::SetLit(es) => Members::Exprs(es),
        ExprKind::Const(c) => {
            let cost = lookup_cost(budget.bind.sets.len(), c.len());
            work::<Md>(budget, cost, sp)?;
            match budget.bind.sets.get(c) {
                Some(values) => Members::Values(Rc::clone(values)),
                None => return no(reason_in(set, budget), set.span),
            }
        }
        _ => return no(reason_in(set, budget), set.span),
    };
    let n = match &members {
        Members::Exprs(es) => es.len(),
        Members::Values(vs) => vs.len(),
    };
    if x.ty == Type::Bool {
        return bool_membership::<Md>(x, &members, n, sp, budget);
    }
    // A parameter or binder of a composite type is its index, and a constant set of
    // composites holds its members' indices (bn-23hzh).
    let first = match collections::atom_node::<Md>(x, budget)? {
        Some(atom) => atom,
        None => int_expr::<Md>(x, budget)?,
    };
    if n == 0 {
        // No member, so `x in S` is false — but CML still evaluates `x`, which may fail.
        // `!(x == x)` is false wherever `x` evaluates and fails wherever it fails, so
        // the evaluation, and its errors, are kept (cr-3aqchd); a charged copy of `x`.
        let (again, sa) = copy::<Md, _>(budget, sp, &first)?;
        let (same, ss) = node::<Md, _>(budget, sp, &[first.1, sa], || {
            Md::b_cmp(CmpOp::Eq, first.0, again)
        })?;
        return node::<Md, _>(budget, sp, &[ss], || Md::b_not(same));
    }
    let mut items = Vec::with_capacity(n);
    // Every comparison but the last takes a charged copy of `x`; the last takes `x`
    // itself, moved, not cloned (cr-3aqchd).
    let mut first = Some(first);
    for i in 0..n {
        let (xi, sxi) = take_or_copy::<Md, _>(&mut first, i + 1 == n, budget, sp)?;
        let (m, sm) = match &members {
            Members::Exprs(es) => match es.get(i) {
                Some(e) => int_expr::<Md>(e, budget)?,
                None => return no(Unlowerable::NonIntegerValue, sp),
            },
            Members::Values(vs) => {
                let v = vs.get(i).copied().unwrap_or(0);
                node::<Md, _>(budget, sp, &[], || Md::i_const(v))?
            }
        };
        items.push(node::<Md, _>(budget, sp, &[sxi, sm], || {
            Md::b_cmp(CmpOp::Eq, xi, m)
        })?);
    }
    balanced::<Md>(items, false, budget, sp)
}

/// [`membership`] of a Boolean `x`: against a literal member `x <=> m`, against a
/// constant code `x` (`1`) or `!x` (`0`); the empty set is `x && !x`.
fn bool_membership<Md: Mode>(
    x: &Expr,
    members: &Members<'_>,
    n: usize,
    sp: Span,
    budget: &mut Meter,
) -> R<Sized<Md::B>> {
    let first = bool_expr::<Md>(x, budget)?;
    if n == 0 {
        let (again, sa) = copy::<Md, _>(budget, sp, &first)?;
        let (not, sn) = node::<Md, _>(budget, sp, &[sa], || Md::b_not(again))?;
        return node::<Md, _>(budget, sp, &[first.1, sn], || Md::b_and(first.0, not));
    }
    let mut items = Vec::with_capacity(n);
    let mut first = Some(first);
    for i in 0..n {
        let xi = take_or_copy::<Md, _>(&mut first, i + 1 == n, budget, sp)?;
        let item = match members {
            Members::Exprs(es) => match es.get(i) {
                Some(e) => {
                    let m = bool_expr::<Md>(e, budget)?;
                    iff::<Md>(budget, sp, xi, m)?
                }
                None => return no(Unlowerable::NonIntegerValue, sp),
            },
            Members::Values(vs) => {
                if vs.get(i).copied().unwrap_or(0) != 0 {
                    xi
                } else {
                    node::<Md, _>(budget, sp, &[xi.1], || Md::b_not(xi.0))?
                }
            }
        };
        items.push(item);
    }
    balanced::<Md>(items, false, budget, sp)
}

/// `a <=> b` as `(a => b) && (b => a)`. The expression language has no equivalence
/// and no sharing, so each operand appears twice; the second copy of each is charged
/// to the budget *before* it is made. A nest of `k` equivalences doubles per level, so
/// it is refused as [`Unlowerable::OutputTooLarge`] as soon as the next copy would not
/// fit, having allocated at most the budget.
fn iff<Md: Mode>(
    budget: &mut Meter,
    span: Span,
    a: Sized<Md::B>,
    b: Sized<Md::B>,
) -> R<Sized<Md::B>> {
    let a2 = copy::<Md, _>(budget, span, &a)?;
    let b2 = copy::<Md, _>(budget, span, &b)?;
    let (ab, sab) = node::<Md, _>(budget, span, &[a.1, b.1], || Md::b_implies(a.0, b.0))?;
    let (ba, sba) = node::<Md, _>(budget, span, &[b2.1, a2.1], || Md::b_implies(b2.0, a2.0))?;
    node::<Md, _>(budget, span, &[sab, sba], || Md::b_and(ab, ba))
}

// ---------------------------------------------------------------------------
// finite domains and quantifier expansion (RFC 0003 correction 4, bn-10j7z)
// ---------------------------------------------------------------------------

/// The most quantifier binders in scope at once, across nested quantifiers and the
/// binders of one quantifier: the model's expression depth, which a nest of quantifiers
/// over two or more values each would reach anyway.
pub const MAX_BINDER_NESTING: usize = MAX_EXPR_DEPTH;

/// The universe of a scalar type under this lowering, as the inclusive range of its
/// integer codes (RFC 0003 correction 4, "Finite types").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Universe {
    /// Codes `lo..=hi`; empty when `hi < lo`.
    Finite(i64, i64),
    /// `Int`, `Nat`, or a sort with no bound or instantiation.
    Unbounded,
    /// A type with no finite universe in this correction: a record, string, sequence,
    /// or function (or a collection over one).
    NotScalar,
    /// A finite composite type whose universe does not fit `i64` atoms (bn-23hzh).
    TooLarge,
}

/// The universe of `ty`: `Bool` is `0..=1`; an enumeration its variant indices; a sort
/// its instantiation; `Nat` and `Int` their configuration bounds. One table lookup.
fn universe<Md: Mode>(ty: &Type, budget: &mut Meter, span: Span) -> R<Universe> {
    Ok(match ty {
        Type::Bool => Universe::Finite(0, 1),
        Type::Nat => match budget.bind.bounds.nat_max() {
            Some(max) => Universe::Finite(0, max),
            None => Universe::Unbounded,
        },
        Type::Int => match budget.bind.bounds.int_range() {
            Some((lo, hi)) => Universe::Finite(lo, hi),
            None => Universe::Unbounded,
        },
        Type::Sort(s) => {
            let cost = lookup_cost(budget.bind.sorts.len(), s.len());
            work::<Md>(budget, cost, span)?;
            match budget.bind.sorts.get(s) {
                Some(n) => Universe::Finite(0, n.saturating_sub(1)),
                None => Universe::Unbounded,
            }
        }
        Type::Enum(e) => {
            let cost = lookup_cost(budget.enums.len(), e.len());
            work::<Md>(budget, cost, span)?;
            match budget.enums.get(e) {
                Some(t) => Universe::Finite(0, t.names.len() as i64 - 1),
                None => Universe::NotScalar,
            }
        }
        // A composite type: its values' indices (bn-23hzh).
        _ => match collections::composite_universe::<Md>(ty, budget, span)? {
            Ok(Some((lo, hi))) => Universe::Finite(lo, hi),
            Ok(None) => Universe::TooLarge,
            Err(layout::Card::Unbounded) => Universe::Unbounded,
            Err(_) => Universe::NotScalar,
        },
    })
}

/// The number of codes in `lo..=hi`.
fn span_count(lo: i64, hi: i64) -> u128 {
    if hi < lo {
        0
    } else {
        u128::try_from(i128::from(hi) - i128::from(lo) + 1).unwrap_or(0)
    }
}

/// A refusal for a type with no finite universe: under a configuration the type can
/// be bounded (`cml.lower.unbounded_type`); without one the legacy reason stands.
fn unbounded(budget: &Meter, legacy: Unlowerable) -> Unlowerable {
    if budget.bind.configured {
        Unlowerable::UnboundedType
    } else {
        legacy
    }
}

/// An interval of values an integer expression can take, computed in `i128`; `fits`
/// is false when some intermediate result may leave `i64`, where the model core's
/// checked arithmetic would report an overflow instead of a value.
#[derive(Debug, Clone, Copy)]
struct Iv {
    lo: i128,
    hi: i128,
    fits: bool,
}

impl Iv {
    fn point(v: i64) -> Self {
        Self {
            lo: i128::from(v),
            hi: i128::from(v),
            fits: true,
        }
    }

    fn range(lo: i64, hi: i64) -> Self {
        Self {
            lo: i128::from(lo),
            hi: i128::from(hi),
            fits: true,
        }
    }

    /// The value, when the expression has exactly one and computes it without
    /// overflow: then it is static and folds to a constant.
    fn value(self) -> Option<i64> {
        if self.fits && self.lo == self.hi {
            i64::try_from(self.lo).ok()
        } else {
            None
        }
    }

    fn make(lo: i128, hi: i128, fits: bool) -> Self {
        let inside = lo >= i128::from(i64::MIN) && hi <= i128::from(i64::MAX);
        Self {
            lo,
            hi,
            fits: fits && inside,
        }
    }
}

/// The interval of an integer-coded expression: literals and bound constants are
/// points, parameters and bound variables are their value (building) or interval
/// (planning), and a state variable its declared domain; `+ - *`, negation, `min` and
/// `max` combine intervals. `None` for anything else, or an `i128` overflow. One unit
/// of work per node, charged before the node is read; recursion follows the source
/// tree, whose depth [`shallow`] bounded.
fn interval<Md: Mode>(e: &Expr, budget: &mut Meter) -> R<Option<Iv>> {
    let sp = e.span;
    work::<Md>(budget, 1, sp)?;
    let two = |a: &Expr, b: &Expr, budget: &mut Meter| -> R<Option<(Iv, Iv)>> {
        Ok(
            match (interval::<Md>(a, budget)?, interval::<Md>(b, budget)?) {
                (Some(x), Some(y)) => Some((x, y)),
                _ => None,
            },
        )
    };
    Ok(match &e.kind {
        ExprKind::Int(n) => Some(Iv::point(*n)),
        ExprKind::Const(c) => {
            let cost = lookup_cost(budget.bind.ints.len(), c.len());
            work::<Md>(budget, cost, sp)?;
            budget.bind.ints.get(c).copied().map(Iv::point)
        }
        ExprKind::Variant {
            enumeration,
            variant,
        } => variant_index::<Md>(budget, enumeration, variant, sp)?.map(Iv::point),
        ExprKind::Param(_) | ExprKind::Bound { .. } => {
            scoped::<Md>(e, budget, sp)?.map(|(lo, hi)| Iv::range(lo, hi))
        }
        // A post-state read has an interval only where it lowers: in a postcondition, of a
        // relational variable. Anywhere else it is left to `int_expr`, which refuses it,
        // so no fold can erase an invalid read (cr-3aqchd).
        ExprKind::Primed(v) if !(budget.post && budget.slots.contains_key(v)) => None,
        ExprKind::State(v) | ExprKind::Primed(v) => {
            let cost = lookup_cost(budget.domains.len(), v.len());
            work::<Md>(budget, cost, sp)?;
            budget
                .domains
                .get(v.as_str())
                .map(|(lo, hi)| Iv::range(*lo, *hi))
        }
        ExprKind::Neg(a) => {
            interval::<Md>(a, budget)?.and_then(|x| iv_arith(ArithOp::Sub, Iv::point(0), x))
        }
        ExprKind::Binary(op @ (BinOp::Add | BinOp::Sub | BinOp::Mul), a, b) => {
            let op = match op {
                BinOp::Add => ArithOp::Add,
                BinOp::Sub => ArithOp::Sub,
                _ => ArithOp::Mul,
            };
            two(a, b, budget)?.and_then(|(x, y)| iv_arith(op, x, y))
        }
        ExprKind::Builtin(b @ (Builtin::Min | Builtin::Max), args) => match args.as_slice() {
            [a, c] => two(a, c, budget)?.map(|(x, y)| iv_min_max(*b == Builtin::Min, x, y)),
            _ => None,
        },
        _ => None,
    })
}

/// `x op y` over intervals, in `i128`; `None` on an `i128` overflow. `fits` records
/// whether both operands and the result stay within `i64`, as the model core's checked
/// arithmetic requires of every intermediate result.
fn iv_arith(op: ArithOp, x: Iv, y: Iv) -> Option<Iv> {
    let fits = x.fits && y.fits;
    match op {
        ArithOp::Add => Some(Iv::make(
            x.lo.checked_add(y.lo)?,
            x.hi.checked_add(y.hi)?,
            fits,
        )),
        ArithOp::Sub => Some(Iv::make(
            x.lo.checked_sub(y.hi)?,
            x.hi.checked_sub(y.lo)?,
            fits,
        )),
        ArithOp::Mul => {
            let c = [
                x.lo.checked_mul(y.lo)?,
                x.lo.checked_mul(y.hi)?,
                x.hi.checked_mul(y.lo)?,
                x.hi.checked_mul(y.hi)?,
            ];
            Some(Iv::make(
                c.iter().copied().min()?,
                c.iter().copied().max()?,
                fits,
            ))
        }
    }
}

/// `min` (or `max`) over intervals.
fn iv_min_max(min: bool, x: Iv, y: Iv) -> Iv {
    let fits = x.fits && y.fits;
    if min {
        Iv::make(x.lo.min(y.lo), x.hi.min(y.hi), fits)
    } else {
        Iv::make(x.lo.max(y.lo), x.hi.max(y.hi), fits)
    }
}

/// The values a quantifier's binder takes.
enum Cands<'e> {
    /// Every code of `lo..=hi`, each instance guarded by membership when `guard` is set.
    Span {
        lo: i64,
        hi: i64,
        guard: Option<Guard<'e>>,
        /// While planning, every value a build of this domain may bind: the hull, and
        /// for a range also the lower bound's whole interval (a range empty at build
        /// keeps one candidate at its lower bound). The plan's binder takes this
        /// interval, so it checks every candidate the build can choose (cr-3aqchd).
        reach: (i64, i64),
    },
    /// While planning only: a domain that reads no state and computes without overflow,
    /// but is not yet a fixed set (its bounds or members read parameters or enclosing
    /// binders). The build finds it static — unguarded, at most `count` values, each in
    /// `lo..=hi`.
    Upto { lo: i64, hi: i64, count: u128 },
    /// Exactly these codes: distinct, ascending, unguarded. Shared, not copied.
    List(Rc<[i64]>),
}

/// The membership test of a state-dependent domain.
#[derive(Clone, Copy)]
enum Guard<'e> {
    /// `a..b`: `a <= u && u <= b`.
    Range(&'e Expr, &'e Expr),
    /// `{e1, …}`: `u == e1 || …`.
    Members(&'e [Expr]),
    /// A set-valued expression that reads state: its slot for `u` is `1` (bn-23hzh).
    Slot(&'e Expr),
}

/// The interval of `e`, or the lowering's own refusal of it when it has none (an
/// expression the lowering cannot carry), or `OutputTooLarge` for an `i128` overflow.
fn interval_or_refuse<Md: Mode>(e: &Expr, budget: &mut Meter) -> R<Iv> {
    if let Some(iv) = interval::<Md>(e, budget)? {
        return Ok(iv);
    }
    int_expr::<Plan>(e, budget)?;
    no(Unlowerable::OutputTooLarge, e.span)
}

/// A hull clamped to `i64`: values outside it are never values of an expression.
fn clamp(lo: i128, hi: i128) -> (i64, i64) {
    let c = |v: i128| i64::try_from(v).unwrap_or(if v < 0 { i64::MIN } else { i64::MAX });
    (c(lo), c(hi))
}

/// The candidates of one binder (RFC 0003 correction 4, "Quantifier expansion"):
///
/// - no domain: the universe of its type, which must be finite;
/// - `a..b` with static bounds: exactly `a..=b`; otherwise the interval hull of the
///   bounds, each instance guarded by `a <= u && u <= b`;
/// - a set literal of static members: exactly its distinct members; otherwise the hull
///   of the members, each instance guarded by `u == e1 || …`;
/// - a configuration constant set: exactly its members.
///
/// A `Bool` binder ranges over its type only. Anything else is refused typed.
fn candidates<'e, Md: Mode>(
    binder: &'e crate::norm::Binder,
    span: Span,
    budget: &mut Meter,
) -> R<Cands<'e>> {
    let universe = universe::<Md>(&binder.ty, budget, span)?;
    if universe == Universe::NotScalar {
        return no(Unlowerable::Quantifier, span);
    }
    if universe == Universe::TooLarge {
        return no(Unlowerable::OutputTooLarge, span);
    }
    let Some(domain) = &binder.domain else {
        return match universe {
            Universe::Finite(lo, hi) => Ok(Cands::Span {
                lo,
                hi,
                guard: None,
                reach: (lo, hi),
            }),
            _ => no(unbounded(budget, Unlowerable::Quantifier), span),
        };
    };
    if binder.ty == Type::Bool {
        return no(Unlowerable::NonIntegerValue, domain.span);
    }
    let empty = || Cands::List(Rc::from(Vec::new()));
    match &domain.kind {
        ExprKind::Binary(BinOp::Range, a, b) => {
            let ia = interval_or_refuse::<Md>(a, budget)?;
            let ib = interval_or_refuse::<Md>(b, budget)?;
            if let (Some(lo), Some(hi)) = (ia.value(), ib.value()) {
                return Ok(if hi < lo {
                    empty()
                } else {
                    Cands::Span {
                        lo,
                        hi,
                        guard: None,
                        reach: (lo, hi),
                    }
                });
            }
            // Static once parameters and enclosing binders are fixed: planned unguarded.
            // Both reads are scanned whatever the intervals say, so the plan (where an
            // interval may not fit) predicts the scans the build (where it may) makes.
            let ra = reads_state::<Md>(a, budget)?;
            let rb = reads_state::<Md>(b, budget)?;
            let fixed = ia.fits && ib.fits && !ra && !rb;
            if fixed {
                if ib.hi < ia.lo {
                    return Ok(empty());
                }
                let (lo, hi) = clamp(ia.lo, ib.hi);
                return Ok(Cands::Upto {
                    lo,
                    hi,
                    count: span_count(lo, hi),
                });
            }
            if ib.hi < ia.lo {
                // The range is empty in every state. When both bounds compute without
                // overflow, nothing is evaluated: the constant `true` (`false`). When one
                // may overflow, the CML domain's evaluation may fail, so it must still be
                // evaluated: one guarded candidate, whose guard is false wherever it
                // evaluates, keeps exactly that evaluation (cr-3aqchd).
                if ia.fits && ib.fits {
                    return Ok(empty());
                }
                let (lo, top) = clamp(ia.lo, ia.hi);
                return Ok(Cands::Span {
                    lo,
                    hi: lo,
                    guard: Some(Guard::Range(a, b)),
                    reach: (lo, top),
                });
            }
            let (lo, hi) = clamp(ia.lo, ib.hi);
            let (_, top) = clamp(ia.lo, ib.hi.max(ia.hi));
            Ok(Cands::Span {
                lo,
                hi,
                guard: Some(Guard::Range(a, b)),
                reach: (lo, top),
            })
        }
        // Members of a composite type: static, by index (bn-23hzh).
        ExprKind::SetLit(es) if !layout::is_scalar(&binder.ty) => {
            collections::literal_domain::<Md>(binder, es, span, budget)
        }
        ExprKind::SetLit(es) => {
            let mut ivs = Vec::with_capacity(es.len());
            for e in es {
                ivs.push(interval_or_refuse::<Md>(e, budget)?);
            }
            let values: Option<Vec<i64>> = ivs.iter().map(|iv| iv.value()).collect();
            if let Some(mut values) = values {
                // The sort is done here, in either mode: spent before it runs.
                effort::<Md>(budget, sort_cost(values.len(), 0), span)?;
                values.sort_unstable();
                values.dedup();
                return Ok(Cands::List(Rc::from(values)));
            }
            let lo = ivs.iter().map(|iv| iv.lo).min().unwrap_or(0);
            let hi = ivs.iter().map(|iv| iv.hi).max().unwrap_or(-1);
            let (lo, hi) = clamp(lo, hi);
            let mut fixed = ivs.iter().all(|iv| iv.fits);
            for e in es {
                // Scanned whatever `fixed` is so far, as for a range.
                let reads = reads_state::<Md>(e, budget)?;
                fixed = fixed && !reads;
            }
            if fixed {
                // The build sorts the members it then finds fixed: predicted here.
                work::<Md>(budget, sort_cost(es.len(), 0), span)?;
                return Ok(Cands::Upto {
                    lo,
                    hi,
                    count: span_count(lo, hi).min(es.len() as u128),
                });
            }
            Ok(Cands::Span {
                lo,
                hi,
                guard: Some(Guard::Members(es)),
                reach: (lo, hi),
            })
        }
        ExprKind::Const(c) => {
            // A composite binder takes the constant's member indices, which are indices
            // in the constant's element type: the binder must have exactly that type.
            if !layout::is_scalar(&binder.ty)
                && !matches!(&domain.ty, Type::Set(t) if **t == binder.ty)
            {
                return no(Unlowerable::NonIntegerValue, domain.span);
            }
            let cost = lookup_cost(budget.bind.sets.len(), c.len());
            work::<Md>(budget, cost, span)?;
            // A reference count, not a copy of the members.
            match budget.bind.sets.get(c) {
                Some(values) => Ok(Cands::List(Rc::clone(values))),
                None => no(reason_in(domain, budget), domain.span),
            }
        }
        // Any other set: its members when static, else the universe guarded by the
        // domain's slot (bn-23hzh).
        _ if matches!(domain.ty, Type::Set(_)) => {
            collections::set_domain::<Md>(binder, domain, span, budget)
        }
        _ => no(reason_in(domain, budget), domain.span),
    }
}

/// Whether `e` reads the state (a pre- or post-state variable): syntactic, one unit of
/// work per node, charged as it is visited; recursion follows the source tree, which
/// [`shallow`] bounded.
fn reads_state<Md: Mode>(e: &Expr, budget: &mut Meter) -> R<bool> {
    work::<Md>(budget, 1, e.span)?;
    if matches!(e.kind, ExprKind::State(_) | ExprKind::Primed(_)) {
        return Ok(true);
    }
    for child in crate::elab::children(e) {
        if reads_state::<Md>(child, budget)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `forall` or `exists` over `binders`, expanded (RFC 0003 correction 4): the balanced
/// conjunction (disjunction) of the body at each candidate of the first binder, in
/// ascending order, each instance guarded by membership when its domain reads state;
/// several binders are the nest of single-binder quantifiers, in binder order, so a
/// later domain sees the earlier binders. An empty domain is `true` (`false`).
///
/// Building, the binder takes each candidate in turn. Planning, it takes its whole
/// candidate interval once: the item's measure and predicted work are then multiplied
/// by the fan-out, so nested quantifiers multiply, without building anything.
fn quantifier<Md: Mode>(
    q: crate::norm::Quant,
    binders: &[(u32, crate::norm::Binder)],
    body: &Expr,
    sp: Span,
    budget: &mut Meter,
) -> R<Sized<Md::B>> {
    if binders.is_empty() {
        return bool_expr::<Md>(body, budget);
    }
    // Each binder entered is one level of this recursion (a quantifier with several
    // binders is their nest), and one `Quant` node may hold any number of them, so the
    // source depth does not bound it. The binders entered are counted here, before
    // recursing — by depth, not by the scope table, whose size a repeated binder number
    // does not grow (cr-3aqchd).
    if budget.binder_depth >= MAX_BINDER_NESTING {
        return no(Unlowerable::ExpressionTooDeep, sp);
    }
    budget.binder_depth = budget.binder_depth.saturating_add(1);
    let result = quantifier_level::<Md>(q, binders, body, sp, budget);
    budget.binder_depth = budget.binder_depth.saturating_sub(1);
    result
}

/// Enter binder `id` at `value`, returning the entry it shadows (a binder number repeats
/// where the same def is inlined inside its own argument).
fn enter(budget: &mut Meter, id: u32, value: (i64, i64)) -> Option<(i64, i64)> {
    budget.env.binders.insert(id, value)
}

/// Leave binder `id`, restoring the entry it shadowed.
fn leave(budget: &mut Meter, id: u32, shadowed: Option<(i64, i64)>) {
    match shadowed {
        Some(v) => {
            budget.env.binders.insert(id, v);
        }
        None => {
            budget.env.binders.remove(&id);
        }
    }
}

/// One binder level of [`quantifier`].
fn quantifier_level<Md: Mode>(
    q: crate::norm::Quant,
    binders: &[(u32, crate::norm::Binder)],
    body: &Expr,
    sp: Span,
    budget: &mut Meter,
) -> R<Sized<Md::B>> {
    let Some(((id, binder), rest)) = binders.split_first() else {
        return bool_expr::<Md>(body, budget);
    };
    let forall = q == crate::norm::Quant::Forall;
    let cands = candidates::<Md>(binder, sp, budget)?;
    let (count, reach, guard) = match &cands {
        Cands::Span {
            lo,
            hi,
            guard,
            reach,
        } => (span_count(*lo, *hi), *reach, *guard),
        Cands::Upto { lo, hi, count } => (*count, (*lo, *hi), None),
        Cands::List(vs) => (
            vs.len() as u128,
            (
                vs.first().copied().unwrap_or(0),
                vs.last().copied().unwrap_or(-1),
            ),
            None,
        ),
    };
    if count == 0 {
        return node::<Md, _>(budget, sp, &[], || Md::b_const(forall));
    }
    // Entering and leaving the binder's scope: two ordered-table steps per candidate.
    let cost = lookup_cost(budget.env.binders.len().saturating_add(1), 8).saturating_mul(2);
    if !Md::BUILD {
        // One instance over every value the build may bind; its predicted work (the
        // scope entry included) stands for every candidate's.
        // Everything the instance predicts — work, scratch output, definedness items —
        // stands for every candidate's (bn-23hzh: scratch and items too).
        let before = (budget.predicted, budget.scratch, budget.defs_plan);
        work::<Md>(budget, cost, sp)?;
        let shadowed = enter(budget, *id, reach);
        let item = instance::<Md>(forall, rest, body, guard, reach.0, sp, budget);
        leave(budget, *id, shadowed);
        let item = item?.1;
        let total = balanced_measure(count, item);
        if total.depth > MAX_EXPR_DEPTH {
            return no(Unlowerable::ExpressionTooDeep, sp);
        }
        collections::scale_since(budget, before, count.saturating_sub(1));
        predicted_fits(budget, sp)?;
        work::<Md>(budget, u64::try_from(count).unwrap_or(u64::MAX), sp)?;
        return Ok((Md::b_const(forall), total));
    }
    // Building: the site's plan bounded this fan-out by the output it prepaid (every
    // instance is at least one node); refuse before looping if it did not.
    let left = (budget.prepaid as u128).saturating_add(budget.nodes.left() as u128);
    if count > left {
        return no(Unlowerable::OutputTooLarge, sp);
    }
    let values: Box<dyn Iterator<Item = i64>> = match cands {
        Cands::Span { lo, hi, .. } => Box::new(lo..=hi),
        Cands::List(vs) => Box::new((0..vs.len()).filter_map(move |i| vs.get(i).copied())),
        // Only a plan sees a domain that is not yet fixed.
        Cands::Upto { .. } => return no(Unlowerable::Quantifier, sp),
    };
    let mut items = Vec::new();
    for v in values {
        work::<Md>(budget, cost, sp)?;
        let shadowed = enter(budget, *id, (v, v));
        let item = instance::<Md>(forall, rest, body, guard, v, sp, budget);
        leave(budget, *id, shadowed);
        items.push(item?);
    }
    balanced::<Md>(items, forall, budget, sp)
}

/// One instance of a quantifier at the candidate `v` (the binder is already in scope):
/// the rest of the binders over the body, and, for a guarded domain, `guard => item`
/// (`forall`) or `guard && item` (`exists`).
fn instance<Md: Mode>(
    forall: bool,
    rest: &[(u32, crate::norm::Binder)],
    body: &Expr,
    guard: Option<Guard<'_>>,
    v: i64,
    sp: Span,
    budget: &mut Meter,
) -> R<Sized<Md::B>> {
    // A guarded instance is evaluated at every candidate of the hull, also where its
    // guard is false: the model core's `=>` and `&&` evaluate both operands. That is
    // the CML meaning only if nothing under the guard can fail at a candidate outside
    // the domain, so while planning, everything under it — the rest of the binders
    // with their domains and guards, and the body — is planned in a guarded context,
    // where `int_expr` refuses any integer node that may overflow. The guard itself is
    // not in that context: its bounds and members are evaluated by the CML domain too.
    let guarded = guard.is_some() && !Md::BUILD;
    if guarded {
        budget.guarded = budget.guarded.saturating_add(1);
    }
    // A guarded instance's body is evaluated by CML only where the guard holds, so its
    // definedness items are collected in their own frame and become one item,
    // `guard => items` (bn-23hzh). The guard's own items stay in the enclosing frame.
    let outer = guard.is_some().then(|| Md::f_take(budget));
    let item = quantifier::<Md>(
        if forall {
            crate::norm::Quant::Forall
        } else {
            crate::norm::Quant::Exists
        },
        rest,
        body,
        sp,
        budget,
    );
    if guarded {
        budget.guarded = budget.guarded.saturating_sub(1);
    }
    let inner = outer.map(|o| {
        let inner = Md::f_take(budget);
        Md::f_put(budget, o);
        inner
    });
    let item = item?;
    match guard {
        None => Ok(item),
        Some(guard) => guarded_item::<Md>(forall, guard, v, item, inner, sp, budget),
    }
}

/// A guarded instance, `guard => item` (`forall`) or `guard && item` (`exists`), with
/// the item's definedness frame `inner` recorded as the one item `guard => inner`
/// (bn-23hzh). Kept out of [`instance`], which the binder recursion passes through, so
/// the recursion's frames stay small.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn guarded_item<Md: Mode>(
    forall: bool,
    guard: Guard<'_>,
    v: i64,
    item: Sized<Md::B>,
    inner: Option<Md::F>,
    sp: Span,
    budget: &mut Meter,
) -> R<Sized<Md::B>> {
    let (g, sg) = match guard {
        Guard::Range(a, b) => {
            let (u1, su1) = node::<Md, _>(budget, sp, &[], || Md::i_const(v))?;
            let (la, sa) = int_expr::<Md>(a, budget)?;
            let (ge, sge) = node::<Md, _>(budget, sp, &[su1, sa], || Md::b_cmp(CmpOp::Ge, u1, la))?;
            let (u2, su2) = node::<Md, _>(budget, sp, &[], || Md::i_const(v))?;
            let (lb, sb) = int_expr::<Md>(b, budget)?;
            let (le, sle) = node::<Md, _>(budget, sp, &[su2, sb], || Md::b_cmp(CmpOp::Le, u2, lb))?;
            node::<Md, _>(budget, sp, &[sge, sle], || Md::b_and(ge, le))?
        }
        Guard::Members(es) => {
            let mut eqs = Vec::with_capacity(es.len());
            for e in es {
                let (u, su) = node::<Md, _>(budget, sp, &[], || Md::i_const(v))?;
                let (m, sm) = int_expr::<Md>(e, budget)?;
                eqs.push(node::<Md, _>(budget, sp, &[su, sm], || {
                    Md::b_cmp(CmpOp::Eq, u, m)
                })?);
            }
            balanced::<Md>(eqs, false, budget, sp)?
        }
        Guard::Slot(domain) => collections::slot_guard::<Md>(domain, v, budget, sp)?,
    };
    let (g, sg) = match inner {
        Some(inner) => match Md::f_fold(budget, inner, sp)? {
            Some(d) => {
                let guard = (g, sg);
                let again = copy::<Md, _>(budget, sp, &guard)?;
                let item =
                    node::<Md, _>(budget, sp, &[again.1, d.1], || Md::b_implies(again.0, d.0))?;
                Md::f_push(budget, item);
                guard
            }
            None => (g, sg),
        },
        None => (g, sg),
    };
    node::<Md, _>(budget, sp, &[sg, item.1], || {
        if forall {
            Md::b_implies(g, item.0)
        } else {
            Md::b_and(g, item.0)
        }
    })
}
