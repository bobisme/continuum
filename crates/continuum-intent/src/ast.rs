//! The property AST: RFC 0037's Finite-core property fragment, as types (PR-4 / IMPL-01).
//!
//! # Why a property is structure and not a string
//!
//! > Property expressions are structured ASTs, not strings. A string carries no
//! > soundness: under textual comparison a rename or a rewrite is indistinguishable
//! > from a weakening, which is precisely the reward-hacking path INV-001 exists to
//! > close.
//! >
//! > — `notes/plan/rfcs/0037-intent-contract.md`, "Property AST and canonical normal form (CPNF-1)"
//!
//! `docs/50_AGENT_EVALUATION_AND_REWARD_HACKING.md` lists "weaken property" first
//! among the intent attacks, and RFC 0031 classifies a property change `unchanged`
//! only on byte-equality of CPNF-1 encodings. Neither is expressible over a string:
//! `always(p)` and `always (p)` differ textually and not semantically, while
//! `always(p and q)` and `always(p)` differ semantically in a way a token-set
//! comparison can miss. The types below are what makes the difference decidable.
//!
//! # The node set is closed, and this module is where "closed" is enforced
//!
//! RFC 0037 fixes eleven formula node kinds and five term node kinds, and the
//! schema's `$defs/formula` and `$defs/term` enumerate exactly those. Three of the
//! eleven rows carry two wire `kind` tokens each — `and`/`or`, `always`/`eventually`,
//! `forall`/`exists` — so fourteen tokens name eleven kinds. [`Formula`] and
//! [`Term`] are those enumerations and nothing else:
//!
//! - **`next` is absent by construction.** There is no variant to write it in.
//!   "The theorem-preserving temporal core is LTL without `next` because stuttering
//!   refinement is central (RFC 0015); admitting `next` would make step-count
//!   refactorings look like semantic changes and make stuttering-invariant
//!   refinements unprovable."
//! - **Recurrence and persistence have no node.** They are spelled
//!   `always(eventually φ)` and `eventually(always φ)`, "so the two spellings cannot
//!   diverge".
//! - **Floating-point literals are absent.** [`Literal`] has boolean, integer,
//!   string, and null arms, matching `$defs/term_literal`, "so the canonical
//!   encoding stays byte-deterministic".
//! - **Probabilistic, timed, and epistemic properties are absent.** They "arrive
//!   with the ADR-0016 lanes and require a revision of this section".
//!
//! A decoder that met an unrecognized `kind` token would have to invent a variant,
//! and it cannot, which is how RFC 0037's "fail closed […] forward compatibility is
//! achieved by rejecting, never by ignoring" becomes a property of the type system
//! rather than of a `match` arm somebody might add a wildcard to.
//!
//! # Where the invariants live
//!
//! The schema states three shape constraints this module carries:
//!
//! | Constraint | Schema | Here |
//! |---|---|---|
//! | `^[A-Za-z_][A-Za-z0-9_.]*$` for every name, operator, and binder variable | `$defs/identifier` | [`Identifier::new`], the only way to build one |
//! | a junction has at least two operands | `$defs/formula_junction` `minItems: 2` | [`Formula::and`]/[`Formula::or`], and [`Formula::validate`] |
//! | an application has at least one argument | `$defs/term_apply` `minItems: 1` | [`Term::apply`], and [`Formula::validate`] |
//!
//! The variants keep public payloads, matching `continuum_value::value::Value`'s
//! shape, so pattern-matching a property reads like the RFC's table. That means a
//! caller *can* hand-build `Formula::And { operands: Vec::new() }`, so every path
//! that assigns meaning to an AST — [`crate::cpnf::normalize`] and everything built
//! on it — runs [`Formula::validate`] first and returns a typed error rather than
//! encoding a shape no schema admits.
//!
//! # Seams left open on purpose
//!
//! - **No evaluation.** Nothing here decides whether a property *holds*; that is an
//!   engine's job, and this crate sits in the smallest trust base (docs/33).
//! - **No parser for a surface syntax.** RFC 0037 makes the CML elaborator
//!   responsible for producing this AST, and "a CML construct with no image in the
//!   fragment MUST fail closed rather than be approximated". `expression.source` is
//!   display-only and never round-trips through here.
//! - **No sort table.** RFC 0037 N6 permits absorbing a `not` above `lt`/`le` "only
//!   where the compared sort is declared totally ordered". No declaration of sorts
//!   exists yet, so [`crate::cpnf`] never absorbs it. When a model's sort
//!   declarations become available, that is the seam the rule attaches to.

use core::fmt;
use std::collections::BTreeSet;

use crate::canonical_json::{Json, MAX_DEPTH};

/// A semantic fragment (ADR-0025).
///
/// The wire literals are capitalized, and RFC 0037's correction 1 scopes the
/// lowercase rule to the RFC 0031 relation vocabulary: "`scope.fragments` members
/// […] are capitalized in their own schemas and stay capitalized".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Fragment {
    /// Bounded, exhaustively explorable.
    Finite,
    /// SMT-backed.
    Symbolic,
    /// Temporal/liveness.
    Temporal,
    /// Probabilistic. Declarative until the ADR-0016 lanes ship.
    Probabilistic,
    /// Theorem-proving.
    Theorem,
    /// Runtime observation.
    Runtime,
}

impl Fragment {
    /// Every fragment, in wire-enum order.
    pub const ALL: [Self; 6] = [
        Self::Finite,
        Self::Symbolic,
        Self::Temporal,
        Self::Probabilistic,
        Self::Theorem,
        Self::Runtime,
    ];

    /// The wire literal.
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            Self::Finite => "Finite",
            Self::Symbolic => "Symbolic",
            Self::Temporal => "Temporal",
            Self::Probabilistic => "Probabilistic",
            Self::Theorem => "Theorem",
            Self::Runtime => "Runtime",
        }
    }

    /// Recover a fragment from its wire literal.
    ///
    /// Returns `None` for an unrecognized token; the caller turns that into a
    /// rejection, never into a default (RFC 0037, "Versioning and revision").
    #[must_use]
    pub fn from_wire(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.wire() == token)
    }
}

impl fmt::Display for Fragment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.wire())
    }
}

/// A model-level name: state variable, predicate, action schema, constant,
/// operator, or bound variable.
///
/// The only constructor validates `$defs/identifier`'s pattern, so an `Identifier`
/// in hand is an identifier the schema admits. [`Ord`] is byte order over UTF-8,
/// which for this character class is ASCII order.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Identifier(String);

impl Identifier {
    /// Validate and wrap a name.
    ///
    /// # Errors
    ///
    /// [`AstError::Identifier`] when `name` does not match
    /// `^[A-Za-z_][A-Za-z0-9_.]*$`.
    pub fn new(name: &str) -> Result<Self, AstError> {
        let mut chars = name.chars();
        let head_ok = matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_');
        let tail_ok = chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.');
        if head_ok && tail_ok {
            Ok(Self(name.to_owned()))
        } else {
            Err(AstError::Identifier {
                name: name.to_owned(),
            })
        }
    }

    /// The name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Identifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A scalar literal: `$defs/term_literal`'s four admissible JSON types.
///
/// There is no floating-point arm. v1 excludes them "so that the canonical encoding
/// stays byte-deterministic" — a float has more than one shortest decimal spelling
/// and no total order that survives `NaN`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Literal {
    /// A boolean.
    Boolean(bool),
    /// An integer.
    Integer(i64),
    /// A string.
    Text(String),
    /// The `null` literal.
    Null,
}

/// A term: `$defs/term`'s five node kinds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Term {
    /// A reference to a variable bound by an enclosing quantifier.
    Var {
        /// The variable name.
        name: Identifier,
    },
    /// A scalar literal.
    Literal {
        /// The literal value.
        value: Literal,
    },
    /// A reference to a declared model constant: a domain, a fixed set, a named value.
    Constant {
        /// The constant name.
        name: Identifier,
    },
    /// A reference to a state variable, optionally indexed.
    State {
        /// The state variable name.
        name: Identifier,
        /// The index terms. Empty when the variable is unindexed; the empty case is
        /// encoded explicitly rather than omitted (see [`Term::to_json`]).
        indices: Vec<Term>,
    },
    /// An application of a declared model operator.
    ///
    /// CPNF-1 never reorders `args`: "no algebraic law is assumed for an operator
    /// whose laws are not declared".
    Apply {
        /// The operator name.
        operator: Identifier,
        /// The arguments. At least one, per `$defs/term_apply`'s `minItems: 1`.
        args: Vec<Term>,
    },
}

impl Term {
    /// Build an application, rejecting the zero-argument form.
    ///
    /// # Errors
    ///
    /// [`AstError::EmptyApplication`] when `args` is empty.
    pub fn apply(operator: Identifier, args: Vec<Self>) -> Result<Self, AstError> {
        if args.is_empty() {
            return Err(AstError::EmptyApplication {
                operator: operator.to_string(),
            });
        }
        Ok(Self::Apply { operator, args })
    }

    /// The term's wire `kind` token.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Var { .. } => "var",
            Self::Literal { .. } => "literal",
            Self::Constant { .. } => "constant",
            Self::State { .. } => "state",
            Self::Apply { .. } => "apply",
        }
    }

    /// This term as a canonical JSON document.
    ///
    /// `indices` and `args` are always emitted, `indices` even when empty. RFC 0037
    /// requires an absent optional set to be "encoded explicitly as the empty set
    /// when the identity preimage is built (ID5), so that 'absent' and 'empty'
    /// cannot yield two identities for one meaning" — the rule is stated for the
    /// contract's optional set fields, and it is applied here for the same reason:
    /// an unindexed `state` term has one spelling, not two.
    #[must_use]
    pub fn to_json(&self) -> Json {
        let mut fields = Vec::new();
        fields.push(("kind".to_owned(), Json::String(self.kind().to_owned())));
        match self {
            Self::Var { name } | Self::Constant { name } => {
                fields.push(("name".to_owned(), Json::String(name.to_string())));
            }
            Self::Literal { value } => {
                fields.push(("value".to_owned(), literal_to_json(value)));
            }
            Self::State { name, indices } => {
                fields.push((
                    "indices".to_owned(),
                    Json::Array(indices.iter().map(Self::to_json).collect()),
                ));
                fields.push(("name".to_owned(), Json::String(name.to_string())));
            }
            Self::Apply { operator, args } => {
                fields.push((
                    "args".to_owned(),
                    Json::Array(args.iter().map(Self::to_json).collect()),
                ));
                fields.push(("operator".to_owned(), Json::String(operator.to_string())));
            }
        }
        // The keys are distinct by construction, so the object cannot be malformed.
        Json::object(fields).unwrap_or(Json::Null)
    }
}

fn literal_to_json(value: &Literal) -> Json {
    match value {
        Literal::Boolean(v) => Json::Bool(*v),
        Literal::Integer(v) => Json::Integer(*v),
        Literal::Text(v) => Json::String(v.clone()),
        Literal::Null => Json::Null,
    }
}

/// A quantifier binder: one variable ranging over a finite domain term.
///
/// CPNF-1 N5 alpha-renames bound variables, "so binder names never contribute to
/// identity".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binder {
    /// The bound variable.
    pub variable: Identifier,
    /// The finite domain it ranges over, normally a declared model constant.
    pub domain: Term,
}

impl Binder {
    /// Build a binder.
    #[must_use]
    pub const fn new(variable: Identifier, domain: Term) -> Self {
        Self { variable, domain }
    }
}

/// Whether an action atom asserts occurrence or enabledness (RFC 0015).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ActionModality {
    /// The action occurs on this step.
    Occurs,
    /// The action is enabled in this state.
    Enabled,
}

impl ActionModality {
    /// The wire literal.
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            Self::Occurs => "occurs",
            Self::Enabled => "enabled",
        }
    }

    /// Recover a modality from its wire literal, or `None` for an unknown token.
    #[must_use]
    pub const fn from_wire(token: &str) -> Option<Self> {
        match token.as_bytes() {
            b"occurs" => Some(Self::Occurs),
            b"enabled" => Some(Self::Enabled),
            _ => None,
        }
    }
}

/// The eight comparison operators of `$defs/comparison_operator`.
///
/// CPNF-1 N6 rewrites [`Gt`](Self::Gt) and [`Ge`](Self::Ge) away by swapping
/// operands, so only six survive normalization. Both spellings are admitted on
/// input because the schema admits both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ComparisonOperator {
    /// Equality. Commutative; N6 orders its operands.
    Eq,
    /// Disequality. Commutative; N6 orders its operands.
    Ne,
    /// Strictly less than.
    Lt,
    /// Less than or equal.
    Le,
    /// Strictly greater than. Eliminated by N6.
    Gt,
    /// Greater than or equal. Eliminated by N6.
    Ge,
    /// Set membership.
    Member,
    /// Subset.
    Subset,
}

impl ComparisonOperator {
    /// Every operator, in schema-enum order.
    pub const ALL: [Self; 8] = [
        Self::Eq,
        Self::Ne,
        Self::Lt,
        Self::Le,
        Self::Gt,
        Self::Ge,
        Self::Member,
        Self::Subset,
    ];

    /// The wire literal.
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            Self::Eq => "eq",
            Self::Ne => "ne",
            Self::Lt => "lt",
            Self::Le => "le",
            Self::Gt => "gt",
            Self::Ge => "ge",
            Self::Member => "member",
            Self::Subset => "subset",
        }
    }

    /// Recover an operator from its wire literal, or `None` for an unknown token.
    #[must_use]
    pub fn from_wire(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|op| op.wire() == token)
    }

    /// Whether the operator is commutative, and therefore whether N6 may order its
    /// operands.
    ///
    /// Only `eq` and `ne`. `member` and `subset` relate operands of different sorts,
    /// and `lt`/`le` are directional.
    #[must_use]
    pub const fn is_commutative(self) -> bool {
        matches!(self, Self::Eq | Self::Ne)
    }

    /// The operator this one becomes when its operands are swapped, if any.
    ///
    /// N6's rewrite: `gt(a, b)` is `lt(b, a)` and `ge(a, b)` is `le(b, a)`.
    #[must_use]
    pub const fn swapped(self) -> Option<Self> {
        match self {
            Self::Gt => Some(Self::Lt),
            Self::Ge => Some(Self::Le),
            _ => None,
        }
    }

    /// The operator a `not` above this one collapses into, if any.
    ///
    /// N6 gives this for `eq`/`ne` unconditionally. It is deliberately *not* given
    /// for `lt`/`le`: N6 permits absorbing the negation there "only where the
    /// compared sort is declared totally ordered", no sort declarations exist yet,
    /// and a MAY that is exercised without its precondition is an unsound rewrite.
    /// It is never given for `member`/`subset`, where N6 says the `not` "MUST
    /// remain".
    #[must_use]
    pub const fn negated(self) -> Option<Self> {
        match self {
            Self::Eq => Some(Self::Ne),
            Self::Ne => Some(Self::Eq),
            _ => None,
        }
    }
}

/// A formula: `$defs/formula`'s eleven node kinds, fourteen wire tokens.
///
/// [`PartialEq`] is structural. Two formulas that are equal here have the same N8
/// encoding and therefore the same identity; two that are *semantically* equal but
/// structurally different are what [`crate::cpnf::normalize`] is for, and what
/// RFC 0037 S2 warns is only partially achievable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Formula {
    /// A boolean constant.
    Boolean {
        /// The constant.
        value: bool,
    },
    /// A state atom: a boolean state predicate or indexed boolean state variable.
    Predicate {
        /// The predicate name.
        name: Identifier,
        /// The argument terms; may be empty.
        args: Vec<Term>,
    },
    /// An action atom (RFC 0015).
    Action {
        /// The action-schema name.
        name: Identifier,
        /// Occurrence or enabledness.
        modality: ActionModality,
    },
    /// A comparison atom over two terms.
    Compare {
        /// The operator.
        op: ComparisonOperator,
        /// The left operand.
        left: Term,
        /// The right operand.
        right: Term,
    },
    /// Negation. After N2 it stands only directly above an atom.
    Not {
        /// The negated formula.
        operand: Box<Formula>,
    },
    /// N-ary conjunction.
    And {
        /// The conjuncts. At least two, per `$defs/formula_junction`.
        operands: Vec<Formula>,
    },
    /// N-ary disjunction.
    Or {
        /// The disjuncts. At least two, per `$defs/formula_junction`.
        operands: Vec<Formula>,
    },
    /// Material implication. Eliminated by N1.
    Implies {
        /// The antecedent.
        antecedent: Box<Formula>,
        /// The consequent.
        consequent: Box<Formula>,
    },
    /// Bi-implication. Eliminated by N1.
    Iff {
        /// The left side.
        left: Box<Formula>,
        /// The right side.
        right: Box<Formula>,
    },
    /// The `always` temporal operator.
    Always {
        /// The operand.
        operand: Box<Formula>,
    },
    /// The `eventually` temporal operator.
    Eventually {
        /// The operand.
        operand: Box<Formula>,
    },
    /// Response (RFC 0008). Eliminated by N1.
    LeadsTo {
        /// The antecedent.
        antecedent: Box<Formula>,
        /// The consequent.
        consequent: Box<Formula>,
    },
    /// Universal quantification over a finite domain.
    Forall {
        /// The binder.
        binder: Binder,
        /// The body.
        body: Box<Formula>,
    },
    /// Existential quantification over a finite domain.
    Exists {
        /// The binder.
        binder: Binder,
        /// The body.
        body: Box<Formula>,
    },
}

impl Formula {
    /// The formula's wire `kind` token.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Boolean { .. } => "boolean",
            Self::Predicate { .. } => "predicate",
            Self::Action { .. } => "action",
            Self::Compare { .. } => "compare",
            Self::Not { .. } => "not",
            Self::And { .. } => "and",
            Self::Or { .. } => "or",
            Self::Implies { .. } => "implies",
            Self::Iff { .. } => "iff",
            Self::Always { .. } => "always",
            Self::Eventually { .. } => "eventually",
            Self::LeadsTo { .. } => "leads_to",
            Self::Forall { .. } => "forall",
            Self::Exists { .. } => "exists",
        }
    }

    /// The `true` constant.
    #[must_use]
    pub const fn boolean(value: bool) -> Self {
        Self::Boolean { value }
    }

    /// A state atom.
    #[must_use]
    pub const fn predicate(name: Identifier, args: Vec<Term>) -> Self {
        Self::Predicate { name, args }
    }

    /// An action atom.
    #[must_use]
    pub const fn action(name: Identifier, modality: ActionModality) -> Self {
        Self::Action { name, modality }
    }

    /// A comparison atom.
    #[must_use]
    pub const fn compare(op: ComparisonOperator, left: Term, right: Term) -> Self {
        Self::Compare { op, left, right }
    }

    /// A negation.
    ///
    /// Named for the RFC's own connective vocabulary; it is a boxed-AST
    /// constructor, not an operator overload, so `std::ops::Not` (which takes
    /// `self` by value and suggests operator syntax on formulas) is not the
    /// right trait to implement.
    #[must_use]
    #[allow(clippy::should_implement_trait)]
    pub fn not(operand: Self) -> Self {
        Self::Not {
            operand: Box::new(operand),
        }
    }

    /// A conjunction.
    ///
    /// # Errors
    ///
    /// [`AstError::JunctionArity`] when fewer than two operands are given: the
    /// schema's `minItems: 2` is what makes N3's flattening and N4's ordering
    /// expressible over one canonical n-ary shape.
    pub fn and(operands: Vec<Self>) -> Result<Self, AstError> {
        Self::junction("and", operands).map(|operands| Self::And { operands })
    }

    /// A disjunction.
    ///
    /// # Errors
    ///
    /// [`AstError::JunctionArity`] when fewer than two operands are given.
    pub fn or(operands: Vec<Self>) -> Result<Self, AstError> {
        Self::junction("or", operands).map(|operands| Self::Or { operands })
    }

    fn junction(kind: &'static str, operands: Vec<Self>) -> Result<Vec<Self>, AstError> {
        if operands.len() < 2 {
            return Err(AstError::JunctionArity {
                kind,
                found: operands.len(),
            });
        }
        Ok(operands)
    }

    /// A material implication.
    #[must_use]
    pub fn implies(antecedent: Self, consequent: Self) -> Self {
        Self::Implies {
            antecedent: Box::new(antecedent),
            consequent: Box::new(consequent),
        }
    }

    /// A bi-implication.
    #[must_use]
    pub fn iff(left: Self, right: Self) -> Self {
        Self::Iff {
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    /// An `always`.
    #[must_use]
    pub fn always(operand: Self) -> Self {
        Self::Always {
            operand: Box::new(operand),
        }
    }

    /// An `eventually`.
    #[must_use]
    pub fn eventually(operand: Self) -> Self {
        Self::Eventually {
            operand: Box::new(operand),
        }
    }

    /// A response.
    #[must_use]
    pub fn leads_to(antecedent: Self, consequent: Self) -> Self {
        Self::LeadsTo {
            antecedent: Box::new(antecedent),
            consequent: Box::new(consequent),
        }
    }

    /// A universal quantification.
    #[must_use]
    pub fn forall(binder: Binder, body: Self) -> Self {
        Self::Forall {
            binder,
            body: Box::new(body),
        }
    }

    /// An existential quantification.
    #[must_use]
    pub fn exists(binder: Binder, body: Self) -> Self {
        Self::Exists {
            binder,
            body: Box::new(body),
        }
    }

    /// Check the shape constraints the schema states but the type cannot.
    ///
    /// Junction arity, application arity, and nesting depth. Every path that assigns
    /// meaning to a formula runs this first, so a hand-built degenerate node becomes
    /// a typed error rather than an encoding no schema admits.
    ///
    /// # Errors
    ///
    /// [`AstError::JunctionArity`], [`AstError::EmptyApplication`], or
    /// [`AstError::TooDeep`].
    pub fn validate(&self) -> Result<(), AstError> {
        self.validate_at(0)
    }

    fn validate_at(&self, depth: usize) -> Result<(), AstError> {
        if depth >= MAX_DEPTH {
            return Err(AstError::TooDeep { max: MAX_DEPTH });
        }
        match self {
            Self::Boolean { .. } | Self::Action { .. } => Ok(()),
            Self::Predicate { args, .. } => {
                args.iter().try_for_each(|arg| arg.validate_at(depth + 1))
            }
            Self::Compare { left, right, .. } => {
                left.validate_at(depth + 1)?;
                right.validate_at(depth + 1)
            }
            Self::Not { operand } | Self::Always { operand } | Self::Eventually { operand } => {
                operand.validate_at(depth + 1)
            }
            Self::And { operands } | Self::Or { operands } => {
                if operands.len() < 2 {
                    return Err(AstError::JunctionArity {
                        kind: self.kind(),
                        found: operands.len(),
                    });
                }
                operands
                    .iter()
                    .try_for_each(|operand| operand.validate_at(depth + 1))
            }
            Self::Implies {
                antecedent,
                consequent,
            }
            | Self::LeadsTo {
                antecedent,
                consequent,
            } => {
                antecedent.validate_at(depth + 1)?;
                consequent.validate_at(depth + 1)
            }
            Self::Iff { left, right } => {
                left.validate_at(depth + 1)?;
                right.validate_at(depth + 1)
            }
            Self::Forall { binder, body } | Self::Exists { binder, body } => {
                binder.domain.validate_at(depth + 1)?;
                body.validate_at(depth + 1)
            }
        }
    }

    /// Whether the formula carries no temporal operator at any depth.
    ///
    /// This is W4, which `fairness[].condition` must satisfy: "MUST contain no
    /// `always`, `eventually`, or `leads_to` node at any depth". The schema enforces
    /// it structurally through `$defs/state_formula` over `$defs/temporal_free`;
    /// this predicate is the same statement for a caller that already holds an AST.
    ///
    /// Provided here as a seam. The `fairness` group itself is a separate PR-4 bone.
    #[must_use]
    pub fn is_temporal_free(&self) -> bool {
        match self {
            Self::Always { .. } | Self::Eventually { .. } | Self::LeadsTo { .. } => false,
            Self::Boolean { .. }
            | Self::Predicate { .. }
            | Self::Action { .. }
            | Self::Compare { .. } => true,
            Self::Not { operand } => operand.is_temporal_free(),
            Self::And { operands } | Self::Or { operands } => {
                operands.iter().all(Self::is_temporal_free)
            }
            Self::Implies {
                antecedent,
                consequent,
            } => antecedent.is_temporal_free() && consequent.is_temporal_free(),
            Self::Iff { left, right } => left.is_temporal_free() && right.is_temporal_free(),
            Self::Forall { binder: _, body } | Self::Exists { binder: _, body } => {
                body.is_temporal_free()
            }
        }
    }

    /// The names of every `var` node not bound by an enclosing binder.
    ///
    /// W5 requires every `var` to "lie within the scope of a binder declaring that
    /// name, or the name MUST be a declared free variable of the model". Deciding
    /// the second arm needs the model, which this crate does not have and must not
    /// import; this function computes the set a model-aware checker would test.
    ///
    /// [`crate::cpnf::normalize`] takes the stricter line and rejects a free `var`
    /// outright — see its documentation for why alpha-renaming under N5 cannot
    /// safely coexist with free `var` names.
    #[must_use]
    pub fn free_variables(&self) -> BTreeSet<Identifier> {
        let mut found = BTreeSet::new();
        let mut bound = Vec::new();
        self.collect_free_variables(&mut bound, &mut found);
        found
    }

    fn collect_free_variables(
        &self,
        bound: &mut Vec<Identifier>,
        found: &mut BTreeSet<Identifier>,
    ) {
        match self {
            Self::Boolean { .. } | Self::Action { .. } => {}
            Self::Predicate { args, .. } => {
                for arg in args {
                    arg.collect_free_variables(bound, found);
                }
            }
            Self::Compare { left, right, .. } => {
                left.collect_free_variables(bound, found);
                right.collect_free_variables(bound, found);
            }
            Self::Not { operand } | Self::Always { operand } | Self::Eventually { operand } => {
                operand.collect_free_variables(bound, found);
            }
            Self::And { operands } | Self::Or { operands } => {
                for operand in operands {
                    operand.collect_free_variables(bound, found);
                }
            }
            Self::Implies {
                antecedent,
                consequent,
            }
            | Self::LeadsTo {
                antecedent,
                consequent,
            } => {
                antecedent.collect_free_variables(bound, found);
                consequent.collect_free_variables(bound, found);
            }
            Self::Iff { left, right } => {
                left.collect_free_variables(bound, found);
                right.collect_free_variables(bound, found);
            }
            Self::Forall { binder, body } | Self::Exists { binder, body } => {
                // The domain is evaluated outside the binder's own scope.
                binder.domain.collect_free_variables(bound, found);
                bound.push(binder.variable.clone());
                body.collect_free_variables(bound, found);
                bound.pop();
            }
        }
    }

    /// This formula as a canonical JSON document.
    ///
    /// The key order is the container's, so it is ID5's. `args` is always emitted,
    /// empty included, for the reason [`Term::to_json`] gives.
    #[must_use]
    pub fn to_json(&self) -> Json {
        let mut fields = vec![("kind".to_owned(), Json::String(self.kind().to_owned()))];
        match self {
            Self::Boolean { value } => fields.push(("value".to_owned(), Json::Bool(*value))),
            Self::Predicate { name, args } => {
                fields.push((
                    "args".to_owned(),
                    Json::Array(args.iter().map(Term::to_json).collect()),
                ));
                fields.push(("name".to_owned(), Json::String(name.to_string())));
            }
            Self::Action { name, modality } => {
                fields.push((
                    "modality".to_owned(),
                    Json::String(modality.wire().to_owned()),
                ));
                fields.push(("name".to_owned(), Json::String(name.to_string())));
            }
            Self::Compare { op, left, right } => {
                fields.push(("left".to_owned(), left.to_json()));
                fields.push(("op".to_owned(), Json::String(op.wire().to_owned())));
                fields.push(("right".to_owned(), right.to_json()));
            }
            Self::Not { operand } | Self::Always { operand } | Self::Eventually { operand } => {
                fields.push(("operand".to_owned(), operand.to_json()));
            }
            Self::And { operands } | Self::Or { operands } => {
                fields.push((
                    "operands".to_owned(),
                    Json::Array(operands.iter().map(Self::to_json).collect()),
                ));
            }
            Self::Implies {
                antecedent,
                consequent,
            }
            | Self::LeadsTo {
                antecedent,
                consequent,
            } => {
                fields.push(("antecedent".to_owned(), antecedent.to_json()));
                fields.push(("consequent".to_owned(), consequent.to_json()));
            }
            Self::Iff { left, right } => {
                fields.push(("left".to_owned(), left.to_json()));
                fields.push(("right".to_owned(), right.to_json()));
            }
            Self::Forall { binder, body } | Self::Exists { binder, body } => {
                let binder_json = Json::object([
                    ("domain".to_owned(), binder.domain.to_json()),
                    (
                        "variable".to_owned(),
                        Json::String(binder.variable.to_string()),
                    ),
                ])
                .unwrap_or(Json::Null);
                fields.push(("binder".to_owned(), binder_json));
                fields.push(("body".to_owned(), body.to_json()));
            }
        }
        Json::object(fields).unwrap_or(Json::Null)
    }
}

impl Term {
    fn validate_at(&self, depth: usize) -> Result<(), AstError> {
        if depth >= MAX_DEPTH {
            return Err(AstError::TooDeep { max: MAX_DEPTH });
        }
        match self {
            Self::Var { .. } | Self::Literal { .. } | Self::Constant { .. } => Ok(()),
            Self::State { indices, .. } => indices
                .iter()
                .try_for_each(|index| index.validate_at(depth + 1)),
            Self::Apply { operator, args } => {
                if args.is_empty() {
                    return Err(AstError::EmptyApplication {
                        operator: operator.to_string(),
                    });
                }
                args.iter().try_for_each(|arg| arg.validate_at(depth + 1))
            }
        }
    }

    fn collect_free_variables(
        &self,
        bound: &mut Vec<Identifier>,
        found: &mut BTreeSet<Identifier>,
    ) {
        match self {
            Self::Var { name } => {
                if !bound.contains(name) {
                    found.insert(name.clone());
                }
            }
            Self::Literal { .. } | Self::Constant { .. } => {}
            Self::State { indices, .. } => {
                for index in indices {
                    index.collect_free_variables(bound, found);
                }
            }
            Self::Apply { args, .. } => {
                for arg in args {
                    arg.collect_free_variables(bound, found);
                }
            }
        }
    }
}

/// Why an AST is not one the schema admits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AstError {
    /// A name does not match `$defs/identifier`.
    Identifier {
        /// The rejected name.
        name: String,
    },
    /// A junction carries fewer than two operands.
    JunctionArity {
        /// The junction's wire kind, `and` or `or`.
        kind: &'static str,
        /// How many operands were found.
        found: usize,
    },
    /// An `apply` term carries no arguments.
    EmptyApplication {
        /// The operator that was applied to nothing.
        operator: String,
    },
    /// The AST nests past [`crate::canonical_json::MAX_DEPTH`].
    TooDeep {
        /// The bound.
        max: usize,
    },
}

impl fmt::Display for AstError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identifier { name } => write!(
                f,
                "{name:?} is not a property-AST identifier; the schema's pattern is \
                 ^[A-Za-z_][A-Za-z0-9_.]*$"
            ),
            Self::JunctionArity { kind, found } => write!(
                f,
                "an n-ary `{kind}` carries at least two operands, found {found}"
            ),
            Self::EmptyApplication { operator } => write!(
                f,
                "the operator {operator:?} is applied to no arguments; `apply` carries at least one"
            ),
            Self::TooDeep { max } => write!(f, "the property AST nests past the {max} bound"),
        }
    }
}

impl core::error::Error for AstError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn ident(name: &str) -> Identifier {
        Identifier::new(name).expect("a test identifier is well formed")
    }

    fn predicate(name: &str) -> Formula {
        Formula::predicate(ident(name), Vec::new())
    }

    #[test]
    fn the_identifier_pattern_is_the_schemas() {
        for accepted in ["p", "_p", "P1", "a.b.c", "x_1", "_", "A.0"] {
            assert!(
                Identifier::new(accepted).is_ok(),
                "{accepted} should be an identifier"
            );
        }
        for rejected in ["", "1p", ".p", "p-q", "p q", "p:q", "p\u{e9}", "p/q", "-"] {
            assert_eq!(
                Identifier::new(rejected),
                Err(AstError::Identifier {
                    name: rejected.to_owned()
                }),
                "{rejected} should not be an identifier"
            );
        }
    }

    #[test]
    fn a_junction_needs_two_operands_and_an_application_needs_one() {
        assert_eq!(
            Formula::and(vec![predicate("p")]),
            Err(AstError::JunctionArity {
                kind: "and",
                found: 1
            })
        );
        assert_eq!(
            Formula::or(Vec::new()),
            Err(AstError::JunctionArity {
                kind: "or",
                found: 0
            })
        );
        assert!(Formula::and(vec![predicate("p"), predicate("q")]).is_ok());
        assert_eq!(
            Term::apply(ident("f"), Vec::new()),
            Err(AstError::EmptyApplication {
                operator: "f".to_owned()
            })
        );
    }

    #[test]
    fn validate_catches_a_hand_built_degenerate_node() {
        let degenerate = Formula::And {
            operands: Vec::new(),
        };
        assert_eq!(
            degenerate.validate(),
            Err(AstError::JunctionArity {
                kind: "and",
                found: 0
            })
        );
        let nested = Formula::always(Formula::Or {
            operands: vec![predicate("p")],
        });
        assert_eq!(
            nested.validate(),
            Err(AstError::JunctionArity {
                kind: "or",
                found: 1
            })
        );
        let bad_term = Formula::predicate(
            ident("p"),
            vec![Term::Apply {
                operator: ident("f"),
                args: Vec::new(),
            }],
        );
        assert_eq!(
            bad_term.validate(),
            Err(AstError::EmptyApplication {
                operator: "f".to_owned()
            })
        );
    }

    #[test]
    fn deep_nesting_is_a_typed_error_and_not_a_stack_overflow() {
        let mut formula = predicate("p");
        for _ in 0..MAX_DEPTH + 2 {
            formula = Formula::always(formula);
        }
        assert_eq!(
            formula.validate(),
            Err(AstError::TooDeep { max: MAX_DEPTH })
        );
    }

    #[test]
    fn the_json_shape_matches_the_schemas_node_definitions() {
        let formula = Formula::always(Formula::forall(
            Binder::new(
                ident("e"),
                Term::Constant {
                    name: ident("Events"),
                },
            ),
            Formula::implies(
                Formula::predicate(ident("acknowledged"), vec![Term::Var { name: ident("e") }]),
                Formula::predicate(ident("durable"), vec![Term::Var { name: ident("e") }]),
            ),
        ));
        let encoded = String::from_utf8(formula.to_json().to_canonical_bytes()).expect("utf-8");
        assert_eq!(
            encoded,
            concat!(
                r#"{"kind":"always","operand":{"binder":{"domain":{"kind":"constant","name":"Events"},"#,
                r#""variable":"e"},"body":{"antecedent":{"args":[{"kind":"var","name":"e"}],"#,
                r#""kind":"predicate","name":"acknowledged"},"consequent":{"args":[{"kind":"var","#,
                r#""name":"e"}],"kind":"predicate","name":"durable"},"kind":"implies"},"#,
                r#""kind":"forall"}}"#,
            )
        );
    }

    #[test]
    fn an_empty_argument_list_is_encoded_explicitly() {
        // ID5: "absent" and "empty" must not yield two identities for one meaning.
        let zero_arg = predicate("p");
        assert_eq!(
            String::from_utf8(zero_arg.to_json().to_canonical_bytes()).expect("utf-8"),
            r#"{"args":[],"kind":"predicate","name":"p"}"#
        );
        let unindexed = Term::State {
            name: ident("big"),
            indices: Vec::new(),
        };
        assert_eq!(
            String::from_utf8(unindexed.to_json().to_canonical_bytes()).expect("utf-8"),
            r#"{"indices":[],"kind":"state","name":"big"}"#
        );
    }

    #[test]
    fn every_literal_arm_encodes_and_there_is_no_float_arm() {
        for (literal, expected) in [
            (Literal::Boolean(true), "true"),
            (Literal::Integer(-3), "-3"),
            (Literal::Text("x".to_owned()), r#""x""#),
            (Literal::Null, "null"),
        ] {
            let term = Term::Literal { value: literal };
            let encoded = String::from_utf8(term.to_json().to_canonical_bytes()).expect("utf-8");
            assert_eq!(
                encoded,
                format!(r#"{{"kind":"literal","value":{expected}}}"#)
            );
        }
    }

    #[test]
    fn w4_is_a_predicate_over_the_ast_at_every_depth() {
        assert!(predicate("p").is_temporal_free());
        assert!(
            Formula::and(vec![predicate("p"), Formula::not(predicate("q"))])
                .expect("two operands")
                .is_temporal_free()
        );
        assert!(!Formula::always(predicate("p")).is_temporal_free());
        assert!(!Formula::eventually(predicate("p")).is_temporal_free());
        assert!(!Formula::leads_to(predicate("p"), predicate("q")).is_temporal_free());
        // At depth, and under every child position.
        let buried = Formula::forall(
            Binder::new(ident("x"), Term::Constant { name: ident("D") }),
            Formula::iff(predicate("p"), Formula::always(predicate("q"))),
        );
        assert!(!buried.is_temporal_free());
    }

    #[test]
    fn free_variables_are_the_ones_no_binder_declares() {
        let formula = Formula::forall(
            Binder::new(ident("x"), Term::Var { name: ident("d") }),
            Formula::and(vec![
                Formula::predicate(ident("p"), vec![Term::Var { name: ident("x") }]),
                Formula::predicate(ident("q"), vec![Term::Var { name: ident("y") }]),
            ])
            .expect("two operands"),
        );
        let free = formula.free_variables();
        // `x` is bound; `y` is not; `d` is in the binder's *domain*, which is
        // evaluated outside the binder's own scope, so it is free too.
        assert_eq!(
            free,
            BTreeSet::from([ident("y"), ident("d")]),
            "free variables were {free:?}"
        );
    }

    #[test]
    fn the_wire_vocabularies_round_trip_and_reject_unknown_tokens() {
        for fragment in Fragment::ALL {
            assert_eq!(Fragment::from_wire(fragment.wire()), Some(fragment));
        }
        assert_eq!(Fragment::from_wire("finite"), None);
        assert_eq!(Fragment::from_wire("Concurrent"), None);
        for op in ComparisonOperator::ALL {
            assert_eq!(ComparisonOperator::from_wire(op.wire()), Some(op));
        }
        assert_eq!(ComparisonOperator::from_wire("neq"), None);
        for modality in [ActionModality::Occurs, ActionModality::Enabled] {
            assert_eq!(ActionModality::from_wire(modality.wire()), Some(modality));
        }
        assert_eq!(ActionModality::from_wire("fires"), None);
    }

    #[test]
    fn n6s_operator_tables_say_exactly_what_the_rfc_says() {
        assert_eq!(
            ComparisonOperator::Gt.swapped(),
            Some(ComparisonOperator::Lt)
        );
        assert_eq!(
            ComparisonOperator::Ge.swapped(),
            Some(ComparisonOperator::Le)
        );
        assert_eq!(ComparisonOperator::Lt.swapped(), None);
        assert_eq!(
            ComparisonOperator::Eq.negated(),
            Some(ComparisonOperator::Ne)
        );
        assert_eq!(
            ComparisonOperator::Ne.negated(),
            Some(ComparisonOperator::Eq)
        );
        // N6 permits absorbing a `not` above `lt`/`le` only where the sort is
        // declared totally ordered. No sort table exists, so the MAY is not taken.
        assert_eq!(ComparisonOperator::Lt.negated(), None);
        assert_eq!(ComparisonOperator::Le.negated(), None);
        // N6: a `not` above `member`/`subset` MUST remain.
        assert_eq!(ComparisonOperator::Member.negated(), None);
        assert_eq!(ComparisonOperator::Subset.negated(), None);
        assert!(ComparisonOperator::Eq.is_commutative());
        assert!(!ComparisonOperator::Subset.is_commutative());
    }

    #[test]
    fn there_is_no_next_operator_to_write() {
        // Not a runtime assertion so much as a pin on the node set: the fourteen
        // wire tokens below are the whole vocabulary, and `next` is not among them.
        let tokens: BTreeSet<&str> = [
            Formula::boolean(true),
            predicate("p"),
            Formula::action(ident("a"), ActionModality::Occurs),
            Formula::compare(
                ComparisonOperator::Eq,
                Term::Literal {
                    value: Literal::Integer(1),
                },
                Term::Literal {
                    value: Literal::Integer(1),
                },
            ),
            Formula::not(predicate("p")),
            Formula::and(vec![predicate("p"), predicate("q")]).expect("two operands"),
            Formula::or(vec![predicate("p"), predicate("q")]).expect("two operands"),
            Formula::implies(predicate("p"), predicate("q")),
            Formula::iff(predicate("p"), predicate("q")),
            Formula::always(predicate("p")),
            Formula::eventually(predicate("p")),
            Formula::leads_to(predicate("p"), predicate("q")),
            Formula::forall(
                Binder::new(ident("x"), Term::Constant { name: ident("D") }),
                predicate("p"),
            ),
            Formula::exists(
                Binder::new(ident("x"), Term::Constant { name: ident("D") }),
                predicate("p"),
            ),
        ]
        .iter()
        .map(Formula::kind)
        .collect();
        assert_eq!(tokens.len(), 14);
        assert!(!tokens.contains("next"));
    }
}
