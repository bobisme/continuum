//! The concrete syntax tree of the CML Finite core fragment.
//!
//! The tree records what the source says, in source order, with a [`Span`] on every
//! node. It assigns no meaning: name resolution, typing, fragment classification of
//! calls, and lowering to relations are the elaborator's work (`continuum-cml-elab`).
//!
//! Two surface spellings reach one node where RFC 0003 and the Revision-2 fixtures
//! disagree only in spelling: `=` and `==` in an expression are both
//! [`BinOp::Eq`], and a `module Name` header and a `model Name { … }` header differ only
//! in [`HeaderStyle`].

use crate::span::Span;

/// A name as written, with its location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ident {
    /// The name.
    pub name: String,
    /// Where it is written.
    pub span: Span,
}

/// A parsed `.ctm` source file: one model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    /// The `module` or `model` header.
    pub header: Header,
    /// The declarations, in source order.
    pub decls: Vec<Decl>,
    /// The whole file.
    pub span: Span,
}

/// The model header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    /// Which header form the source uses.
    pub style: HeaderStyle,
    /// The model name.
    pub name: Ident,
    /// The header keyword and name.
    pub span: Span,
}

/// The two header forms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HeaderStyle {
    /// `module Name` followed by declarations to the end of the file (RFC 0003).
    Module,
    /// `model Name { declarations }` (the Revision-2 corpus fixtures).
    Model,
}

/// A top-level declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decl {
    /// What is declared.
    pub kind: DeclKind,
    /// The whole declaration.
    pub span: Span,
}

/// The declaration forms of the Finite core fragment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeclKind {
    /// `type Name` — an uninterpreted sort, bounded by the run configuration.
    Sort {
        /// The sort name.
        name: Ident,
    },
    /// `type Name = T` — a type alias.
    TypeAlias {
        /// The alias name.
        name: Ident,
        /// The aliased type.
        ty: TypeExpr,
    },
    /// `enum Name { A, B, … }` — a finite enumeration.
    Enum {
        /// The enumeration name.
        name: Ident,
        /// The variants, in source order.
        variants: Vec<Ident>,
    },
    /// `const Name: T` — a model constant, bound by the run configuration.
    Const {
        /// The constant name.
        name: Ident,
        /// Its type.
        ty: TypeExpr,
    },
    /// `state { field: T [where P] … }` — the state variables.
    State {
        /// The fields, in source order.
        fields: Vec<StateField>,
    },
    /// `init [Name] { … }` — the initial-state predicate.
    Init {
        /// The optional name.
        name: Option<Ident>,
        /// The clauses, conjoined.
        body: Vec<Stmt>,
    },
    /// `action Name[(params)] { … }` or `action Name = A | B | …`.
    Action {
        /// The action name.
        name: Ident,
        /// The parameters, in source order.
        params: Vec<Param>,
        /// The body.
        body: ActionBody,
    },
    /// `invariant Name { … }` — a state invariant.
    Invariant {
        /// The invariant name.
        name: Ident,
        /// The clauses, conjoined.
        body: Vec<Stmt>,
    },
    /// `fairness weak|strong A, B, …`.
    Fairness {
        /// Weak or strong fairness.
        strength: FairnessStrength,
        /// The actions it applies to.
        actions: Vec<Ident>,
    },
    /// `behavior Name = e` — a named behavior (specification) formula.
    Behavior {
        /// The behavior name.
        name: Ident,
        /// The formula.
        expr: Expr,
    },
    /// `def name(params): T = e` — a (possibly recursive) model function.
    Def {
        /// The function name.
        name: Ident,
        /// The parameters.
        params: Vec<Param>,
        /// The result type.
        ret: TypeExpr,
        /// The body.
        body: Expr,
    },
}

/// A state variable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateField {
    /// The variable name.
    pub name: Ident,
    /// Its type.
    pub ty: TypeExpr,
    /// The optional `where` refinement predicate.
    pub refinement: Option<Expr>,
    /// The whole field.
    pub span: Span,
}

/// A typed parameter `name: T`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    /// The parameter name.
    pub name: Ident,
    /// Its type.
    pub ty: TypeExpr,
    /// The whole parameter.
    pub span: Span,
}

/// An action body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionBody {
    /// `{ statements }`.
    Block(Vec<Stmt>),
    /// `= A | B | …` — the disjunction of named actions.
    Choice(Vec<Ident>),
}

/// Weak or strong fairness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FairnessStrength {
    /// `weak`.
    Weak,
    /// `strong`.
    Strong,
}

/// A statement in an `init`, `action`, or `invariant` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stmt {
    /// The statement.
    pub kind: StmtKind,
    /// The whole statement.
    pub span: Span,
}

/// The statement forms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StmtKind {
    /// `require e` — an action guard.
    Require(Expr),
    /// `let x = e` — a local definition.
    Let(Ident, Expr),
    /// `next x = e` — the post-state value of a state variable.
    Next(Ident, Expr),
    /// `unchanged a, b, …` — the listed variables keep their values.
    Unchanged(Vec<Ident>),
    /// A bare predicate clause.
    Expr(Expr),
}

/// A type expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeExpr {
    /// The type.
    pub kind: TypeKind,
    /// Where it is written.
    pub span: Span,
}

/// The type forms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeKind {
    /// `Name`.
    Named(Ident),
    /// `Name[T, …]`, such as `Set[Node]` or `Map[Nat, Value]`.
    Applied(Ident, Vec<TypeExpr>),
    /// `(T, U, …)` with two or more components.
    Tuple(Vec<TypeExpr>),
    /// `T -> U`.
    Function(Box<TypeExpr>, Box<TypeExpr>),
    /// `{ f: T, … }`.
    Record(Vec<(Ident, TypeExpr)>),
}

/// An expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expr {
    /// The expression.
    pub kind: ExprKind,
    /// Where it is written.
    pub span: Span,
}

/// The expression forms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExprKind {
    /// A natural-number literal.
    Int(u64),
    /// `true` or `false`.
    Bool(bool),
    /// A string literal (after escape processing).
    Str(String),
    /// A name.
    Name(Ident),
    /// The `state` keyword used as a value: the whole state (as in `stutter(state)`).
    WholeState,
    /// `e'` — the post-state value of `e`.
    Prime(Box<Expr>),
    /// A prefix operator.
    Unary(UnOp, Box<Expr>),
    /// An infix operator.
    Binary(BinOp, Box<Expr>, Box<Expr>),
    /// `forall|exists binders: body`.
    Quant(Quantifier, Vec<Binder>, Box<Expr>),
    /// `if c then a else b`.
    If(Box<Expr>, Box<Expr>, Box<Expr>),
    /// `always(e)` or `eventually(e)`.
    Temporal(TemporalOp, Box<Expr>),
    /// `f(args)`.
    Call(Ident, Vec<Expr>),
    /// `e.m(args)`.
    Method(Box<Expr>, Ident, Vec<Expr>),
    /// `e.f`.
    Field(Box<Expr>, Ident),
    /// `e[i]`.
    Index(Box<Expr>, Box<Expr>),
    /// `e[k := v]` — `e` with the value at `k` replaced by `v`.
    Update(Box<Expr>, Box<Expr>, Box<Expr>),
    /// `(a, b, …)` with two or more components.
    Tuple(Vec<Expr>),
    /// `{}` — the empty set or empty map; the elaborator decides which from the type.
    EmptyBraces,
    /// `{a, b, …}`.
    SetLit(Vec<Expr>),
    /// `{k -> v, …}`.
    MapLit(Vec<(Expr, Expr)>),
    /// `{e | binders [where p]}`.
    SetComp(Box<Expr>, Vec<Binder>, Option<Box<Expr>>),
    /// `{k -> v | binders [where p]}`.
    MapComp(Box<Expr>, Box<Expr>, Vec<Binder>, Option<Box<Expr>>),
    /// `{f: e, …}`.
    Record(Vec<(Ident, Expr)>),
    /// `[a, b, …]` — a sequence literal (possibly empty).
    SeqLit(Vec<Expr>),
}

/// A bound variable with an optional domain: `x` or `x in D`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binder {
    /// The bound name.
    pub name: Ident,
    /// The domain, when written.
    pub domain: Option<Expr>,
    /// The whole binder.
    pub span: Span,
}

/// The quantifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Quantifier {
    /// `forall`.
    Forall,
    /// `exists`.
    Exists,
}

/// The unary temporal operators of the stuttering-invariant core (RFC 0015).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TemporalOp {
    /// `always`.
    Always,
    /// `eventually`.
    Eventually,
}

/// Prefix operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UnOp {
    /// `!`.
    Not,
    /// `-`.
    Neg,
}

/// Infix operators, from loosest to tightest binding level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BinOp {
    /// `~>` (leads-to).
    LeadsTo,
    /// `<=>`.
    Iff,
    /// `=>`.
    Implies,
    /// `||`.
    Or,
    /// `&&`.
    And,
    /// `==` or `=`.
    Eq,
    /// `!=`.
    Ne,
    /// `<`.
    Lt,
    /// `<=`.
    Le,
    /// `>`.
    Gt,
    /// `>=`.
    Ge,
    /// `in`.
    In,
    /// `notin`.
    NotIn,
    /// `subseteq`.
    SubsetEq,
    /// `..` (integer range).
    Range,
    /// `union`.
    Union,
    /// `intersect`.
    Intersect,
    /// `\` (set difference).
    Diff,
    /// `+`.
    Add,
    /// `-`.
    Sub,
    /// `*`.
    Mul,
    /// `/`.
    Div,
    /// `%`.
    Mod,
}

impl BinOp {
    /// The canonical spelling.
    #[must_use]
    pub fn symbol(self) -> &'static str {
        match self {
            BinOp::LeadsTo => "~>",
            BinOp::Iff => "<=>",
            BinOp::Implies => "=>",
            BinOp::Or => "||",
            BinOp::And => "&&",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
            BinOp::In => "in",
            BinOp::NotIn => "notin",
            BinOp::SubsetEq => "subseteq",
            BinOp::Range => "..",
            BinOp::Union => "union",
            BinOp::Intersect => "intersect",
            BinOp::Diff => "\\",
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Mod => "%",
        }
    }

    /// A stable name for tree dumps.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            BinOp::LeadsTo => "leads_to",
            BinOp::Iff => "iff",
            BinOp::Implies => "implies",
            BinOp::Or => "or",
            BinOp::And => "and",
            BinOp::Eq => "eq",
            BinOp::Ne => "ne",
            BinOp::Lt => "lt",
            BinOp::Le => "le",
            BinOp::Gt => "gt",
            BinOp::Ge => "ge",
            BinOp::In => "in",
            BinOp::NotIn => "notin",
            BinOp::SubsetEq => "subseteq",
            BinOp::Range => "range",
            BinOp::Union => "union",
            BinOp::Intersect => "intersect",
            BinOp::Diff => "diff",
            BinOp::Add => "add",
            BinOp::Sub => "sub",
            BinOp::Mul => "mul",
            BinOp::Div => "div",
            BinOp::Mod => "mod",
        }
    }
}
