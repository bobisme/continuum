//! The normalized semantic AST: what a CML model means, with its surface spelling gone.
//!
//! Decision: RFC 0003 ("Stable semantic IR. Source syntax can evolve without changing
//! CIR semantics") and docs/11 §14 ("The normalized semantic AST is stable before the
//! surface language. Format and language revisions can lower to the same AST").
//!
//! # What normalization removes
//!
//! Every surface choice that does not change meaning is resolved here, so two sources
//! that differ only in that choice elaborate to one canonical dump and one identity:
//!
//! | Surface | Normalized |
//! |---|---|
//! | `module M …` or `model M { … }` | the header style is dropped |
//! | `=` or `==` | one equality |
//! | declaration order of top-level items | every list is sorted by name |
//! | `type A = T` | every use of `A` is `T` |
//! | `let x = e` | every use of `x` is `e` |
//! | `def f(x: T): U = e` (not recursive) | every call is the body with the arguments substituted |
//! | `def f(…, k: Nat, …): U = e` (recursive, see [`crate::elab`] "Recursive defs") | every call is its unfolding at the constant value of `k`, with the measure tests decided |
//! | `next x = e` or `x' == e` | one [`Next::Set`] |
//! | `unchanged x`, or `x' == x` | one [`Next::Unchanged`] |
//! | `require a && b`, or `require a` and `require b` | a list of conjuncts |
//! | `{b, a, a}` | the elements sorted by their canonical encoding and deduplicated |
//! | `exists x, y` with no domain | each binder carries its inferred type |
//!
//! Spans are kept on every node for diagnostics, and are ignored by [`NormModel::dump`]
//! and [`NormModel::identity`].
//!
//! # Identity
//!
//! [`NormModel::identity`] is content-addressed from this tree, never from the source
//! text: it is the canonical dump with bound-variable names replaced by their binding
//! depth (so alpha-renaming a quantifier does not change it) and with the model name
//! left out. Following ADR-0013 it is the canonical encoding itself, compared exactly.

use std::fmt::Write as _;

use continuum_cml_syntax::Span;

use crate::types::Type;

/// A normalized CML model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormModel {
    /// The model name from the header. It is not part of the identity.
    pub name: String,
    /// Uninterpreted sorts, sorted by name.
    pub sorts: Vec<String>,
    /// Enumerations, sorted by name.
    pub enums: Vec<EnumDecl>,
    /// Model constants, sorted by name.
    pub constants: Vec<ConstDecl>,
    /// State variables, sorted by name.
    pub state: Vec<StateVar>,
    /// The initial-state predicate, if the model declares one.
    pub init: Option<Init>,
    /// Actions with a body, sorted by name.
    pub actions: Vec<Action>,
    /// Named disjunctions of actions (`action Next = A | B`), sorted by name.
    pub choices: Vec<Choice>,
    /// State invariants, sorted by name.
    pub invariants: Vec<Invariant>,
    /// Fairness assumptions, one per action, sorted by action then strength.
    pub fairness: Vec<Fairness>,
    /// Behavior formulas, sorted by name.
    pub behaviors: Vec<Behavior>,
}

/// An enumeration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumDecl {
    /// The enumeration name.
    pub name: String,
    /// The variants, in declaration order (the order is part of the type).
    pub variants: Vec<String>,
}

/// A model constant, bound by the run configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstDecl {
    /// The constant name.
    pub name: String,
    /// Its type.
    pub ty: Type,
    /// The declaration.
    pub span: Span,
}

/// A state variable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateVar {
    /// The variable name.
    pub name: String,
    /// Its declared type.
    pub ty: Type,
    /// The `where` refinement, as conjuncts.
    pub refinement: Vec<Expr>,
    /// The declaration.
    pub span: Span,
}

/// The initial-state predicate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Init {
    /// The optional name (`init Init { … }`).
    pub name: Option<String>,
    /// The conjuncts, in source order.
    pub clauses: Vec<Expr>,
    /// The declaration.
    pub span: Span,
}

/// An action: a guard and the post-state value of every state variable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    /// The action name.
    pub name: String,
    /// The parameters, in declaration order.
    pub params: Vec<Param>,
    /// The guard conjuncts, in source order. Empty means always enabled.
    pub guard: Vec<Expr>,
    /// One entry per state variable, in state-variable order.
    pub next: Vec<(String, Next)>,
    /// The declaration.
    pub span: Span,
}

/// A typed action parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    /// The parameter name.
    pub name: String,
    /// Its type.
    pub ty: Type,
}

/// The post-state value of one state variable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Next {
    /// The variable keeps its value.
    Unchanged,
    /// The variable takes this value, an expression over the pre-state.
    Set(Expr),
}

/// A named disjunction of actions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    /// The choice name.
    pub name: String,
    /// The actions it offers, sorted and without duplicates.
    pub actions: Vec<String>,
    /// The declaration.
    pub span: Span,
}

/// A state invariant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invariant {
    /// The invariant name.
    pub name: String,
    /// The conjuncts, in source order.
    pub clauses: Vec<Expr>,
    /// The declaration.
    pub span: Span,
}

/// Weak or strong fairness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Strength {
    /// `weak`.
    Weak,
    /// `strong`.
    Strong,
}

/// A fairness assumption on one action (ADR-0025 `Temporal` fragment).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fairness {
    /// The action or choice it applies to.
    pub action: String,
    /// Weak or strong.
    pub strength: Strength,
    /// The declaration.
    pub span: Span,
}

/// A behavior formula (ADR-0025 `Temporal` fragment).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Behavior {
    /// The behavior name.
    pub name: String,
    /// The formula.
    pub formula: Expr,
    /// The declaration.
    pub span: Span,
}

/// A bound variable: a name, its type, and its domain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binder {
    /// The name as written. Not part of the identity.
    pub name: String,
    /// The type, written or inferred.
    pub ty: Type,
    /// The domain set. `None` means the binder ranges over its whole type.
    pub domain: Option<Expr>,
}

/// A typed expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expr {
    /// The expression.
    pub kind: ExprKind,
    /// Its type.
    pub ty: Type,
    /// Where it came from. Not part of the dump or the identity.
    pub span: Span,
}

/// Quantifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Quant {
    /// `forall`.
    Forall,
    /// `exists`.
    Exists,
}

/// Temporal operators, allowed only in a behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Temporal {
    /// `always(e)`.
    Always,
    /// `eventually(e)`.
    Eventually,
}

/// Binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BinOp {
    /// `~>` (behavior only).
    LeadsTo,
    /// `<=>`.
    Iff,
    /// `=>`.
    Implies,
    /// `||`.
    Or,
    /// `&&`.
    And,
    /// `==` / `=`.
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
    /// `..`, the integer range as a set.
    Range,
    /// `union`.
    Union,
    /// `intersect`.
    Intersect,
    /// `\`.
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
    /// The stable dump name.
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

/// Built-in operations, resolved from calls and method calls by type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Builtin {
    /// `min(a, b)` on integers.
    Min,
    /// `max(a, b)` on integers.
    Max,
    /// `m.get(k)` on `Map[K, V]`: `Option[V]`.
    MapGet,
    /// `m.put(k, v)` on `Map[K, V]`.
    MapPut,
    /// `s.head()` on `Seq[T]`.
    SeqHead,
    /// `s.tail()` on `Seq[T]`.
    SeqTail,
    /// `s.append(x)` on `Seq[T]`.
    SeqAppend,
    /// `s.prepend(x)` on `Seq[T]`.
    SeqPrepend,
    /// `s.len()` on `Seq[T]`.
    SeqLen,
}

impl Builtin {
    /// The stable dump name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Builtin::Min => "min",
            Builtin::Max => "max",
            Builtin::MapGet => "map.get",
            Builtin::MapPut => "map.put",
            Builtin::SeqHead => "seq.head",
            Builtin::SeqTail => "seq.tail",
            Builtin::SeqAppend => "seq.append",
            Builtin::SeqPrepend => "seq.prepend",
            Builtin::SeqLen => "seq.len",
        }
    }
}

/// The expression forms of the normalized AST.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExprKind {
    /// A boolean literal.
    Bool(bool),
    /// An integer literal.
    Int(i64),
    /// A string literal.
    Str(String),
    /// A state variable, read in the pre-state.
    State(String),
    /// A model constant.
    Const(String),
    /// An action parameter.
    Param(String),
    /// A bound variable; `binder` is its unique binder number within the model.
    Bound {
        /// The name as written.
        name: String,
        /// The binder it refers to.
        binder: u32,
    },
    /// An enumeration variant.
    Variant {
        /// The enumeration.
        enumeration: String,
        /// The variant.
        variant: String,
    },
    /// `None` of an `Option[T]`.
    OptionNone,
    /// `Some(e)`.
    OptionSome(Box<Expr>),
    /// `!e`.
    Not(Box<Expr>),
    /// `-e`.
    Neg(Box<Expr>),
    /// `a op b`.
    Binary(BinOp, Box<Expr>, Box<Expr>),
    /// `if c then a else b`.
    If(Box<Expr>, Box<Expr>, Box<Expr>),
    /// A quantifier. Each binder carries its unique number.
    Quant(Quant, Vec<(u32, Binder)>, Box<Expr>),
    /// A tuple of two or more components.
    Tuple(Vec<Expr>),
    /// A set literal: sorted by canonical encoding, no duplicates. Empty is `{}`.
    SetLit(Vec<Expr>),
    /// A map literal: sorted by canonical encoding of the pair. Empty is `{}`.
    MapLit(Vec<(Expr, Expr)>),
    /// A sequence literal.
    SeqLit(Vec<Expr>),
    /// `{e | binders where p}`.
    SetComp(Box<Expr>, Vec<(u32, Binder)>, Option<Box<Expr>>),
    /// `{k -> v | binders where p}`.
    MapComp(Box<Expr>, Box<Expr>, Vec<(u32, Binder)>, Option<Box<Expr>>),
    /// A record literal, fields sorted by name.
    Record(Vec<(String, Expr)>),
    /// `e.f` on a record.
    Field(Box<Expr>, String),
    /// `m[k]` on a map, sequence, or function.
    Index(Box<Expr>, Box<Expr>),
    /// `m[k := v]` on a map, sequence, or function.
    Update(Box<Expr>, Box<Expr>, Box<Expr>),
    /// A built-in operation.
    Builtin(Builtin, Vec<Expr>),
    /// `always(e)` or `eventually(e)` (behavior only).
    Temporal(Temporal, Box<Expr>),
    /// `step(A)`: one transition of the named action or choice (behavior only).
    Step(String),
    /// `stutter(state)`: a transition that changes no state variable (behavior only).
    Stutter,
    /// A reference to the init predicate by name (behavior only).
    InitRef(String),
    /// A call of the recursive `def` `function` from inside its own body.
    ///
    /// It exists only in the elaborated body of a recursive def, which is not part of
    /// the model: every call from outside is unfolded (see [`crate::elab`] "Recursive
    /// defs"), so an elaborated model never contains one. Lowering a hand-built model
    /// that does is refused ([`crate::Unlowerable::RecursiveCall`]).
    Recur {
        /// The def.
        function: String,
        /// The arguments, one per parameter.
        args: Vec<Expr>,
    },
}

// ---------------------------------------------------------------------------
// canonical dump and identity
// ---------------------------------------------------------------------------

/// The version tag every normalized identity starts with.
pub const NORM_IDENTITY_TAG: &str = "continuum-cml-norm/1";

/// The identity of a normalized model: its canonical encoding (ADR-0013).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NormIdentity {
    bytes: Vec<u8>,
}

impl NormIdentity {
    /// The canonical encoding.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl NormModel {
    /// The canonical dump: an S-expression with names, without spans.
    ///
    /// Deterministic: a pure function of the tree.
    #[must_use]
    pub fn dump(&self) -> String {
        let mut w = Writer::new(Mode::Named);
        w.model(self, true);
        w.out
    }

    /// The content identity of the normalized model.
    ///
    /// The canonical dump with bound variables written as de Bruijn indices and without
    /// the model name.
    #[must_use]
    pub fn identity(&self) -> NormIdentity {
        let mut w = Writer::new(Mode::Anonymous);
        w.out.push_str(NORM_IDENTITY_TAG);
        w.out.push('\n');
        w.model(self, false);
        NormIdentity {
            bytes: w.out.into_bytes(),
        }
    }
}

impl Expr {
    /// The canonical encoding of one expression. A bound variable bound outside the
    /// expression is written with its binder number, so two different free variables
    /// never encode alike.
    #[must_use]
    pub fn canonical(&self) -> String {
        let mut w = Writer::new(Mode::Anonymous);
        w.expr(self);
        w.out
    }

    /// The canonical encoding of one expression inside the binders `scope` (outermost
    /// first): a variable bound by one of them is written as its de Bruijn depth, as in
    /// [`NormModel::identity`]. This is the key set and map literals are sorted and
    /// deduplicated by, so their order is alpha-stable and does not depend on binder
    /// numbering, and two different bound variables never share a key.
    #[must_use]
    pub fn scoped_key(&self, scope: &[u32]) -> String {
        let mut binders = BinderScope::default();
        for id in scope {
            binders.enter(*id);
        }
        keys_in_scope(&[self], &mut binders)
            .pop()
            .unwrap_or_default()
    }

    /// The canonical dump of one expression, with names.
    #[must_use]
    pub fn dump(&self) -> String {
        let mut w = Writer::new(Mode::Named);
        w.expr(self);
        w.out
    }
}

/// Binder numbers in scope, innermost last, with an index from each number to its
/// positions, so a bound variable's de Bruijn depth is an ordered-table lookup rather
/// than a scan of the scope. Kept incrementally by its owner (the elaborator's finishing
/// pass keeps one across a whole declaration), so rendering a key never rebuilds it.
#[derive(Debug, Default)]
pub struct BinderScope {
    scope: Vec<u32>,
    positions: std::collections::BTreeMap<u32, Vec<usize>>,
}

impl BinderScope {
    /// How many binders are in scope.
    #[must_use]
    pub fn len(&self) -> usize {
        self.scope.len()
    }

    /// Whether no binder is in scope.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.scope.is_empty()
    }

    /// Bring binder `id` into scope, innermost.
    pub fn enter(&mut self, id: u32) {
        self.positions.entry(id).or_default().push(self.scope.len());
        self.scope.push(id);
    }

    /// Leave binders until `len` remain.
    pub fn truncate(&mut self, len: usize) {
        while self.scope.len() > len {
            if let Some(id) = self.scope.pop()
                && let Some(at) = self.positions.get_mut(&id)
            {
                at.pop();
                if at.is_empty() {
                    self.positions.remove(&id);
                }
            }
        }
    }

    /// The de Bruijn depth of binder `id`, innermost `0`, if it is in scope.
    fn depth_of(&self, id: u32) -> Option<usize> {
        let at = *self.positions.get(&id)?.last()?;
        Some(self.scope.len().saturating_sub(1).saturating_sub(at))
    }
}

/// The canonical keys of several expressions inside one binder scope, as
/// [`Expr::scoped_key`] computes them one at a time. The scope is borrowed, not copied
/// or re-indexed, so the cost is the keys alone. `binders` is returned unchanged.
pub fn keys_in_scope(items: &[&Expr], binders: &mut BinderScope) -> Vec<String> {
    let mut w = Writer::new(Mode::Anonymous);
    w.binders = std::mem::take(binders);
    let keys = items
        .iter()
        .map(|x| {
            w.out.clear();
            w.expr(x);
            std::mem::take(&mut w.out)
        })
        .collect();
    *binders = std::mem::take(&mut w.binders);
    keys
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Named,
    Anonymous,
}

struct Writer {
    out: String,
    mode: Mode,
    /// Binders in scope (a number can be bound again inside its own scope by a nested
    /// inlined `def`; the innermost wins).
    binders: BinderScope,
}

impl Writer {
    fn new(mode: Mode) -> Self {
        Self {
            out: String::new(),
            mode,
            binders: BinderScope::default(),
        }
    }

    fn line(&mut self, indent: usize, text: &str) {
        for _ in 0..indent {
            self.out.push_str("  ");
        }
        self.out.push_str(text);
        self.out.push('\n');
    }

    fn model(&mut self, m: &NormModel, with_name: bool) {
        if with_name {
            self.line(0, &format!("(model {}", m.name));
        } else {
            self.line(0, "(model");
        }
        for sort in &m.sorts {
            self.line(1, &format!("(sort {sort})"));
        }
        for e in &m.enums {
            self.line(1, &format!("(enum {} {})", e.name, e.variants.join(" ")));
        }
        for c in &m.constants {
            self.line(1, &format!("(const {} {})", c.name, c.ty));
        }
        for v in &m.state {
            let mut text = format!("(var {} {}", v.name, v.ty);
            for clause in &v.refinement {
                text.push(' ');
                text.push_str(&self.render(clause));
            }
            text.push(')');
            self.line(1, &text);
        }
        if let Some(init) = &m.init {
            match &init.name {
                Some(name) => self.line(1, &format!("(init {name}")),
                None => self.line(1, "(init"),
            }
            for clause in &init.clauses {
                let text = self.render(clause);
                self.line(2, &text);
            }
            self.line(1, ")");
        }
        for a in &m.actions {
            let params: Vec<String> = a
                .params
                .iter()
                .map(|p| format!("({} {})", p.name, p.ty))
                .collect();
            self.line(1, &format!("(action {} ({})", a.name, params.join(" ")));
            for clause in &a.guard {
                let text = format!("(require {})", self.render(clause));
                self.line(2, &text);
            }
            for (var, next) in &a.next {
                let text = match next {
                    Next::Unchanged => format!("(unchanged {var})"),
                    Next::Set(e) => format!("(next {var} {})", self.render(e)),
                };
                self.line(2, &text);
            }
            self.line(1, ")");
        }
        for c in &m.choices {
            self.line(1, &format!("(choice {} {})", c.name, c.actions.join(" ")));
        }
        for i in &m.invariants {
            self.line(1, &format!("(invariant {}", i.name));
            for clause in &i.clauses {
                let text = self.render(clause);
                self.line(2, &text);
            }
            self.line(1, ")");
        }
        for f in &m.fairness {
            let strength = match f.strength {
                Strength::Weak => "weak",
                Strength::Strong => "strong",
            };
            self.line(1, &format!("(fairness {strength} {})", f.action));
        }
        for b in &m.behaviors {
            let text = format!("(behavior {} {})", b.name, self.render(&b.formula));
            self.line(1, &text);
        }
        self.line(0, ")");
    }

    fn render(&mut self, e: &Expr) -> String {
        let saved = std::mem::take(&mut self.out);
        self.expr(e);
        std::mem::replace(&mut self.out, saved)
    }

    fn binders(&mut self, binders: &[(u32, Binder)]) {
        self.out.push('(');
        for (index, (id, b)) in binders.iter().enumerate() {
            if index > 0 {
                self.out.push(' ');
            }
            self.out.push('(');
            match self.mode {
                Mode::Named => self.out.push_str(&b.name),
                Mode::Anonymous => self.out.push('_'),
            }
            let _ = write!(self.out, " {}", b.ty);
            if let Some(d) = &b.domain {
                self.out.push(' ');
                self.expr(d);
            }
            self.out.push(')');
            // A binder's domain is evaluated outside its own scope, and each binder
            // scopes over the ones after it.
            self.binders.enter(*id);
        }
        self.out.push(')');
    }

    fn pop(&mut self, n: usize) {
        let keep = self.binders.len().saturating_sub(n);
        self.binders.truncate(keep);
    }

    fn list(&mut self, head: &str, items: &[Expr]) {
        self.out.push('(');
        self.out.push_str(head);
        for item in items {
            self.out.push(' ');
            self.expr(item);
        }
        self.out.push(')');
    }

    fn expr(&mut self, e: &Expr) {
        match &e.kind {
            ExprKind::Bool(b) => self.out.push_str(if *b { "true" } else { "false" }),
            ExprKind::Int(n) => {
                let _ = write!(self.out, "{n}");
            }
            ExprKind::Str(s) => {
                let _ = write!(self.out, "{s:?}");
            }
            ExprKind::State(name) => {
                let _ = write!(self.out, "(state {name})");
            }
            ExprKind::Const(name) => {
                let _ = write!(self.out, "(const {name})");
            }
            ExprKind::Param(name) => {
                let _ = write!(self.out, "(param {name})");
            }
            ExprKind::Bound { name, binder } => match self.mode {
                Mode::Named => {
                    let _ = write!(self.out, "(bound {name})");
                }
                Mode::Anonymous => {
                    let free = format!("free{binder}");
                    let depth = self
                        .binders
                        .depth_of(*binder)
                        .map_or(free, |d| d.to_string());
                    let _ = write!(self.out, "(bound {depth})");
                }
            },
            ExprKind::Variant {
                enumeration,
                variant,
            } => {
                let _ = write!(self.out, "(variant {enumeration} {variant})");
            }
            ExprKind::OptionNone => {
                let _ = write!(self.out, "(none {})", e.ty);
            }
            ExprKind::OptionSome(inner) => {
                self.out.push_str("(some ");
                self.expr(inner);
                self.out.push(')');
            }
            ExprKind::Not(inner) => {
                self.out.push_str("(not ");
                self.expr(inner);
                self.out.push(')');
            }
            ExprKind::Neg(inner) => {
                self.out.push_str("(neg ");
                self.expr(inner);
                self.out.push(')');
            }
            ExprKind::Binary(op, l, r) => {
                self.out.push('(');
                self.out.push_str(op.name());
                self.out.push(' ');
                self.expr(l);
                self.out.push(' ');
                self.expr(r);
                self.out.push(')');
            }
            ExprKind::If(c, a, b) => {
                self.out.push_str("(if ");
                self.expr(c);
                self.out.push(' ');
                self.expr(a);
                self.out.push(' ');
                self.expr(b);
                self.out.push(')');
            }
            ExprKind::Quant(q, binders, body) => {
                self.out.push_str(match q {
                    Quant::Forall => "(forall ",
                    Quant::Exists => "(exists ",
                });
                self.binders(binders);
                self.out.push(' ');
                self.expr(body);
                self.pop(binders.len());
                self.out.push(')');
            }
            ExprKind::Tuple(items) => self.list("tuple", items),
            ExprKind::SetLit(items) => {
                let head = format!("set {}", e.ty);
                self.list(&head, items);
            }
            ExprKind::MapLit(entries) => {
                let _ = write!(self.out, "(map {}", e.ty);
                for (k, v) in entries {
                    self.out.push_str(" (");
                    self.expr(k);
                    self.out.push(' ');
                    self.expr(v);
                    self.out.push(')');
                }
                self.out.push(')');
            }
            ExprKind::SeqLit(items) => {
                let head = format!("seq {}", e.ty);
                self.list(&head, items);
            }
            ExprKind::SetComp(elem, binders, filter) => {
                self.out.push_str("(set-comp ");
                self.binders(binders);
                self.out.push(' ');
                self.expr(elem);
                if let Some(f) = filter {
                    self.out.push_str(" (where ");
                    self.expr(f);
                    self.out.push(')');
                }
                self.pop(binders.len());
                self.out.push(')');
            }
            ExprKind::MapComp(k, v, binders, filter) => {
                self.out.push_str("(map-comp ");
                self.binders(binders);
                self.out.push(' ');
                self.expr(k);
                self.out.push(' ');
                self.expr(v);
                if let Some(f) = filter {
                    self.out.push_str(" (where ");
                    self.expr(f);
                    self.out.push(')');
                }
                self.pop(binders.len());
                self.out.push(')');
            }
            ExprKind::Record(fields) => {
                self.out.push_str("(record");
                for (name, value) in fields {
                    let _ = write!(self.out, " ({name} ");
                    self.expr(value);
                    self.out.push(')');
                }
                self.out.push(')');
            }
            ExprKind::Field(inner, name) => {
                self.out.push_str("(field ");
                self.expr(inner);
                let _ = write!(self.out, " {name})");
            }
            ExprKind::Index(m, k) => {
                self.out.push_str("(index ");
                self.expr(m);
                self.out.push(' ');
                self.expr(k);
                self.out.push(')');
            }
            ExprKind::Update(m, k, v) => {
                self.out.push_str("(update ");
                self.expr(m);
                self.out.push(' ');
                self.expr(k);
                self.out.push(' ');
                self.expr(v);
                self.out.push(')');
            }
            ExprKind::Builtin(b, args) => self.list(b.name(), args),
            ExprKind::Temporal(op, inner) => {
                self.out.push_str(match op {
                    Temporal::Always => "(always ",
                    Temporal::Eventually => "(eventually ",
                });
                self.expr(inner);
                self.out.push(')');
            }
            ExprKind::Step(name) => {
                let _ = write!(self.out, "(step {name})");
            }
            ExprKind::Stutter => self.out.push_str("(stutter)"),
            ExprKind::InitRef(name) => {
                let _ = write!(self.out, "(init-ref {name})");
            }
            ExprKind::Recur { function, args } => {
                let head = format!("recur {function}");
                self.list(&head, args);
            }
        }
    }
}
