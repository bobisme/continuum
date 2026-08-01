//! Guards and update expressions: a closed, total expression language over the
//! declared state variables.
//!
//! # Why data and not closures
//!
//! > Controlled code accesses scheduling, time, entropy, I/O, faults, and cancellation
//! > through explicit capabilities.
//! >
//! > — `notes/plan/plan.md:325` (INV-005, "No ambient nondeterminism")
//!
//! The obvious Rust spelling of "a guard" is `Box<dyn Fn(&State) -> bool>`, and this
//! module deliberately does not use it. Three properties are lost the moment a guard
//! becomes an opaque function pointer, and each of them is load-bearing here:
//!
//! 1. **INV-005 stops being enforceable.** A closure may read a clock, draw from a
//!    thread-local generator, or consult a global, and nothing at the model boundary
//!    can see that it did. `just check`'s `GOV-1-04` rule (`no-ambient-time-rng`,
//!    `tools/governance/check_code_policy.py:861`) scans *this crate's source* for
//!    those constructs — it cannot scan a caller's closure. An expression tree is
//!    data whose leaves are constants and declared variable names, so "reads nothing
//!    but the state" is a property of the type rather than a promise of the caller.
//! 2. **The model stops having an identity.** `dyn Fn` has no `PartialEq`, no `Ord`,
//!    no `Debug`, and no encoding. A certificate's envelope binds its claim to a
//!    `model_digest` (`crates/continuum-kernel-core/src/wire.rs:51`; RFC 0005 "Claim
//!    envelope"), and a digest of a model built from closures cannot exist. Keeping
//!    guards and updates as inert, structurally comparable data is what will let the
//!    certificate bone hash a model at all.
//! 3. **PO-MOD-002 stops being checkable.** `notes/plan/docs/16_PROOF_OBLIGATIONS.md:13`
//!    obliges "action guard/postcondition evaluation terminates without semantic
//!    error". This language has no recursion, no loops, no calls, and a declared depth
//!    bound ([`MAX_EXPR_DEPTH`]), so termination is structural and every partial
//!    operation returns a typed [`EvalError`] instead of panicking.
//!
//! The cost is expressiveness, and it is paid deliberately: the language is the
//! smallest one that transcribes the Phase A corpus model
//! (`notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm`) without paraphrase.
//! Growing it is a matter of adding a variant and its evaluation arm, which is a
//! reviewable semantic change; it is not something a downstream caller can do
//! silently.
//!
//! # Two decisions that could have gone either way
//!
//! **Arithmetic is checked, never wrapping.** Every operation goes through
//! `i64::checked_*` and reports [`EvalError::Overflow`]. The workspace already sets
//! `overflow-checks = true` in release (`Cargo.toml:91`) precisely so that
//! `debug`/`release` cannot disagree on a semantic artifact (docs/19 §7); returning a
//! value here rather than panicking extends that from "the two builds agree" to "the
//! failure is reportable evidence".
//!
//! **Boolean connectives do not short-circuit.** `And`, `Or`, and `Implies` evaluate
//! both operands and then combine them. Short-circuiting would make an expression's
//! *error set* depend on operand order — `false && overflow` would be `false` while
//! `overflow && false` would be an error — and docs/16 PO-MOD-004 asks that "pure
//! operator evaluation is independent of implementation iteration order"
//! (`notes/plan/docs/16_PROOF_OBLIGATIONS.md:21`). There is no partial operation in
//! this language (no division, no array indexing) whose guard would need
//! short-circuiting to be well-defined, so nothing is lost. A latent overflow in a
//! branch that happens to be dead is a modelling defect, and a reference oracle
//! should surface it rather than hide it.

use core::fmt;

use crate::domain::Variable;

/// The deepest expression the model layer accepts.
///
/// Evaluation is recursive, so the bound is what makes recursion depth a declared
/// constant rather than a function of caller input — the same reason the certificate
/// wire form has "no recursion for RFC 0005's 'cycle/recursion bounds' to bound"
/// (`crates/continuum-kernel-core/src/wire.rs:76-78`). Both the model builder and
/// [`IntExpr::evaluate`] enforce it, so an expression that never reaches a builder is
/// still evaluated within a bounded stack.
pub const MAX_EXPR_DEPTH: usize = 32;

/// The values of the declared variables in one state.
///
/// Lookup is a linear scan. The reference path "is written for obvious correctness,
/// not speed" (`crates/continuum-engine-reference/src/lib.rs`), a state domain carries
/// at most `MAX_VARIABLES = 64` variables
/// (`crates/continuum-kernel-core/src/wire.rs:130`), and a scan has no precondition to
/// state and no way to silently return the wrong answer on an unsorted slice — which a
/// binary search does. If exploration ever profiles this, the seam is to resolve names
/// to indices once per model; nothing about the language has to change.
#[derive(Debug, Clone, Copy)]
pub struct Environment<'a> {
    variables: &'a [Variable],
    values: &'a [i64],
}

impl<'a> Environment<'a> {
    /// Bind `variables` to `values` positionally.
    ///
    /// Total: a variable with no corresponding value simply has none, and
    /// [`IntExpr::evaluate`] reports that as [`EvalError::Unbound`].
    #[must_use]
    pub const fn new(variables: &'a [Variable], values: &'a [i64]) -> Self {
        Self { variables, values }
    }

    /// The value bound to `name`, or `None` when it is not bound.
    #[must_use]
    pub fn value_of(&self, name: &str) -> Option<i64> {
        let index = self
            .variables
            .iter()
            .position(|variable| variable.name().as_str() == name)?;
        self.values.get(index).copied()
    }
}

/// The binary arithmetic operators, named so an overflow can say which one overflowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ArithOp {
    /// Addition.
    Add,
    /// Subtraction.
    Sub,
    /// Multiplication.
    Mul,
}

impl fmt::Display for ArithOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match *self {
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
        })
    }
}

/// The comparison operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CmpOp {
    /// `==`
    Eq,
    /// `!=`
    Ne,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
}

impl CmpOp {
    /// Apply the comparison. Total: comparison of two `i64`s cannot fail.
    #[must_use]
    pub const fn apply(self, left: i64, right: i64) -> bool {
        match self {
            Self::Eq => left == right,
            Self::Ne => left != right,
            Self::Lt => left < right,
            Self::Le => left <= right,
            Self::Gt => left > right,
            Self::Ge => left >= right,
        }
    }
}

impl fmt::Display for CmpOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match *self {
            Self::Eq => "==",
            Self::Ne => "!=",
            Self::Lt => "<",
            Self::Le => "<=",
            Self::Gt => ">",
            Self::Ge => ">=",
        })
    }
}

/// An integer-valued expression over the declared state variables.
///
/// Negation is deliberately absent: `0 - e` spells it with the same overflow
/// behaviour, and one operator with one meaning is worth more here than a second
/// spelling of it (docs/12 §1, "ambiguous behavior is an error").
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IntExpr {
    /// A literal.
    Const(i64),
    /// The value of a variable in the state being evaluated.
    ///
    /// Carried as a plain [`String`] rather than an [`crate::ident::Ident`] because a
    /// reference is only ever *looked up*, never ordered, and because
    /// [`crate::model::ModelBuilder::build`] has to check that the name is declared
    /// anyway — one validation pass, at the point where "declared" is knowable, rather
    /// than two at different times. Every expression inside a built
    /// [`crate::model::Model`] mentions only declared names.
    Var(String),
    /// A checked binary arithmetic operation.
    Arith(ArithOp, Box<IntExpr>, Box<IntExpr>),
    /// The smaller of two values.
    Min(Box<IntExpr>, Box<IntExpr>),
    /// The larger of two values.
    Max(Box<IntExpr>, Box<IntExpr>),
}

impl IntExpr {
    /// A literal.
    #[must_use]
    pub const fn constant(value: i64) -> Self {
        Self::Const(value)
    }

    /// A reference to a variable. Whether the name is a canonical name, and whether it
    /// is *declared*, are model questions answered by
    /// [`crate::model::ModelBuilder::build`].
    #[must_use]
    pub fn var(name: &str) -> Self {
        Self::Var(name.to_owned())
    }

    /// `left + right`.
    #[must_use]
    pub fn plus(left: Self, right: Self) -> Self {
        Self::Arith(ArithOp::Add, Box::new(left), Box::new(right))
    }

    /// `left - right`.
    #[must_use]
    pub fn minus(left: Self, right: Self) -> Self {
        Self::Arith(ArithOp::Sub, Box::new(left), Box::new(right))
    }

    /// `left * right`.
    #[must_use]
    pub fn times(left: Self, right: Self) -> Self {
        Self::Arith(ArithOp::Mul, Box::new(left), Box::new(right))
    }

    /// `min(left, right)`.
    #[must_use]
    pub fn min(left: Self, right: Self) -> Self {
        Self::Min(Box::new(left), Box::new(right))
    }

    /// `max(left, right)`.
    #[must_use]
    pub fn max(left: Self, right: Self) -> Self {
        Self::Max(Box::new(left), Box::new(right))
    }

    /// Evaluate against `environment`.
    ///
    /// # Errors
    ///
    /// [`EvalError::Unbound`] for a variable the environment does not bind,
    /// [`EvalError::Overflow`] for arithmetic `i64` cannot represent, and
    /// [`EvalError::TooDeep`] beyond [`MAX_EXPR_DEPTH`].
    pub fn evaluate(&self, environment: &Environment<'_>) -> Result<i64, EvalError> {
        self.evaluate_within(environment, MAX_EXPR_DEPTH)
    }

    fn evaluate_within(
        &self,
        environment: &Environment<'_>,
        budget: usize,
    ) -> Result<i64, EvalError> {
        let Some(inner) = budget.checked_sub(1) else {
            return Err(EvalError::TooDeep {
                limit: MAX_EXPR_DEPTH,
            });
        };
        match self {
            Self::Const(value) => Ok(*value),
            Self::Var(name) => environment
                .value_of(name)
                .ok_or_else(|| EvalError::Unbound { name: name.clone() }),
            Self::Arith(op, left, right) => {
                let left = left.evaluate_within(environment, inner)?;
                let right = right.evaluate_within(environment, inner)?;
                let result = match op {
                    ArithOp::Add => left.checked_add(right),
                    ArithOp::Sub => left.checked_sub(right),
                    ArithOp::Mul => left.checked_mul(right),
                };
                result.ok_or(EvalError::Overflow {
                    operator: *op,
                    left,
                    right,
                })
            }
            Self::Min(left, right) => Ok(left
                .evaluate_within(environment, inner)?
                .min(right.evaluate_within(environment, inner)?)),
            Self::Max(left, right) => Ok(left
                .evaluate_within(environment, inner)?
                .max(right.evaluate_within(environment, inner)?)),
        }
    }

    /// The expression's nesting depth; a leaf has depth 1.
    ///
    /// Computed with an explicit worklist rather than by recursion, so measuring an
    /// over-deep expression is not itself a way to overflow the stack.
    #[must_use]
    pub fn depth(&self) -> usize {
        let mut deepest = 0_usize;
        let mut stack: Vec<(&Self, usize)> = vec![(self, 1)];
        while let Some((node, depth)) = stack.pop() {
            deepest = deepest.max(depth);
            let below = depth.saturating_add(1);
            match node {
                Self::Const(_) | Self::Var(_) => {}
                Self::Arith(_, left, right) | Self::Min(left, right) | Self::Max(left, right) => {
                    stack.push((left, below));
                    stack.push((right, below));
                }
            }
        }
        deepest
    }

    /// Append every variable name the expression mentions, in traversal order.
    ///
    /// Names may repeat; the caller decides what to do about that. Iterative for the
    /// same reason [`Self::depth`] is.
    pub fn variables(&self, out: &mut Vec<String>) {
        let mut stack: Vec<&Self> = vec![self];
        while let Some(node) = stack.pop() {
            match node {
                Self::Const(_) => {}
                Self::Var(name) => out.push(name.clone()),
                Self::Arith(_, left, right) | Self::Min(left, right) | Self::Max(left, right) => {
                    stack.push(right);
                    stack.push(left);
                }
            }
        }
    }
}

/// A boolean-valued expression: a guard, an invariant body, or a goal body.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BoolExpr {
    /// A literal. `Const(true)` is the unguarded action.
    Const(bool),
    /// A comparison of two integer expressions.
    Compare {
        /// The operator.
        op: CmpOp,
        /// The left operand.
        left: IntExpr,
        /// The right operand.
        right: IntExpr,
    },
    /// Negation.
    Not(Box<BoolExpr>),
    /// Conjunction. Both operands are evaluated; see the module documentation.
    And(Box<BoolExpr>, Box<BoolExpr>),
    /// Disjunction. Both operands are evaluated.
    Or(Box<BoolExpr>, Box<BoolExpr>),
    /// Implication, `!left || right`. Both operands are evaluated.
    Implies(Box<BoolExpr>, Box<BoolExpr>),
    /// `expr in lo..=hi`, the direct transcription of the corpus `in` form
    /// (`notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm:30`).
    InRange {
        /// The expression under test.
        expr: IntExpr,
        /// Inclusive lower bound.
        lo: i64,
        /// Inclusive upper bound.
        hi: i64,
    },
}

impl BoolExpr {
    /// A literal.
    #[must_use]
    pub const fn constant(value: bool) -> Self {
        Self::Const(value)
    }

    /// `left <op> right`.
    #[must_use]
    pub const fn compare(op: CmpOp, left: IntExpr, right: IntExpr) -> Self {
        Self::Compare { op, left, right }
    }

    /// `!inner`.
    #[must_use]
    pub fn negate(inner: Self) -> Self {
        Self::Not(Box::new(inner))
    }

    /// `left && right`.
    #[must_use]
    pub fn and(left: Self, right: Self) -> Self {
        Self::And(Box::new(left), Box::new(right))
    }

    /// `left || right`.
    #[must_use]
    pub fn or(left: Self, right: Self) -> Self {
        Self::Or(Box::new(left), Box::new(right))
    }

    /// `left => right`.
    #[must_use]
    pub fn implies(left: Self, right: Self) -> Self {
        Self::Implies(Box::new(left), Box::new(right))
    }

    /// `expr in lo..=hi`.
    #[must_use]
    pub const fn in_range(expr: IntExpr, lo: i64, hi: i64) -> Self {
        Self::InRange { expr, lo, hi }
    }

    /// Evaluate against `environment`.
    ///
    /// # Errors
    ///
    /// The [`EvalError`]s of the integer sub-expressions, plus [`EvalError::TooDeep`].
    pub fn evaluate(&self, environment: &Environment<'_>) -> Result<bool, EvalError> {
        self.evaluate_within(environment, MAX_EXPR_DEPTH)
    }

    fn evaluate_within(
        &self,
        environment: &Environment<'_>,
        budget: usize,
    ) -> Result<bool, EvalError> {
        let Some(inner) = budget.checked_sub(1) else {
            return Err(EvalError::TooDeep {
                limit: MAX_EXPR_DEPTH,
            });
        };
        match self {
            Self::Const(value) => Ok(*value),
            Self::Compare { op, left, right } => {
                let left = left.evaluate_within(environment, inner)?;
                let right = right.evaluate_within(environment, inner)?;
                Ok(op.apply(left, right))
            }
            Self::Not(inner_expr) => Ok(!inner_expr.evaluate_within(environment, inner)?),
            Self::And(left, right) => {
                let left = left.evaluate_within(environment, inner)?;
                let right = right.evaluate_within(environment, inner)?;
                Ok(left && right)
            }
            Self::Or(left, right) => {
                let left = left.evaluate_within(environment, inner)?;
                let right = right.evaluate_within(environment, inner)?;
                Ok(left || right)
            }
            Self::Implies(left, right) => {
                let left = left.evaluate_within(environment, inner)?;
                let right = right.evaluate_within(environment, inner)?;
                Ok(!left || right)
            }
            Self::InRange { expr, lo, hi } => {
                let value = expr.evaluate_within(environment, inner)?;
                Ok(*lo <= value && value <= *hi)
            }
        }
    }

    /// The expression's nesting depth; a boolean literal has depth 1.
    #[must_use]
    pub fn depth(&self) -> usize {
        let mut deepest = 0_usize;
        let mut stack: Vec<(&Self, usize)> = vec![(self, 1)];
        while let Some((node, depth)) = stack.pop() {
            deepest = deepest.max(depth);
            let below = depth.saturating_add(1);
            match node {
                Self::Const(_) => {}
                Self::Compare { left, right, .. } => {
                    deepest = deepest.max(below.saturating_add(left.depth().saturating_sub(1)));
                    deepest = deepest.max(below.saturating_add(right.depth().saturating_sub(1)));
                }
                Self::InRange { expr, .. } => {
                    deepest = deepest.max(below.saturating_add(expr.depth().saturating_sub(1)));
                }
                Self::Not(inner) => stack.push((inner, below)),
                Self::And(left, right) | Self::Or(left, right) | Self::Implies(left, right) => {
                    stack.push((left, below));
                    stack.push((right, below));
                }
            }
        }
        deepest
    }

    /// Append every variable name the expression mentions, in traversal order.
    pub fn variables(&self, out: &mut Vec<String>) {
        let mut stack: Vec<&Self> = vec![self];
        while let Some(node) = stack.pop() {
            match node {
                Self::Const(_) => {}
                Self::Compare { left, right, .. } => {
                    left.variables(out);
                    right.variables(out);
                }
                Self::InRange { expr, .. } => expr.variables(out),
                Self::Not(inner) => stack.push(inner),
                Self::And(left, right) | Self::Or(left, right) | Self::Implies(left, right) => {
                    stack.push(right);
                    stack.push(left);
                }
            }
        }
    }
}

/// Why an expression produced no value.
///
/// Every arm is data. Nothing in this module panics, indexes, slices, or unwraps —
/// the crate-level lints in `lib.rs` make a regression a compile error rather than a
/// review question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvalError {
    /// A variable the environment does not bind. Unreachable for an expression that
    /// passed [`crate::model::ModelBuilder::build`], which resolves every name.
    Unbound {
        /// The unbound name.
        name: String,
    },
    /// Arithmetic outside `i64`.
    Overflow {
        /// The operator that overflowed.
        operator: ArithOp,
        /// Its left operand.
        left: i64,
        /// Its right operand.
        right: i64,
    },
    /// Nesting beyond [`MAX_EXPR_DEPTH`].
    TooDeep {
        /// [`MAX_EXPR_DEPTH`].
        limit: usize,
    },
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unbound { name } => write!(f, "variable `{name}` is not bound in this state"),
            Self::Overflow {
                operator,
                left,
                right,
            } => write!(f, "`{left} {operator} {right}` overflows i64"),
            Self::TooDeep { limit } => write!(f, "expression nests deeper than {limit}"),
        }
    }
}

impl core::error::Error for EvalError {}
