//! Name resolution, typing, and elaboration of a parsed model to the normalized AST.
//!
//! Decision: RFC 0003 ("Predictable elaboration", "Type system", "Actions") and
//! ADR-0025 (the Finite fragment), PR 15a.
//!
//! # Passes
//!
//! Elaboration runs in a fixed order, and the first error in that order is returned:
//!
//! 1. collect top-level names (one namespace) and reject duplicates;
//! 2. resolve declared types: aliases (dependencies first), constants, state variables,
//!    parameters;
//! 3. elaborate every `def`, callees before callers, so a def may be called before it
//!    is declared; a def that calls itself is checked for termination (see "Recursive
//!    defs"), and a cycle through two or more defs is typed unsupported;
//! 4. state refinements, `init`, actions, choices, invariants, fairness, behaviors,
//!    each in source order.
//!
//! # Bounded recursion (INV-016)
//!
//! Source is untrusted, so no pass recurses deeper than a declared bound:
//!
//! - alias and def dependency orders are computed by an explicit-stack search
//!   (`dependency_order`), so a chain spread over many declarations is followed
//!   iteratively and one alias or def body is resolved at a time;
//! - expression recursion is bounded by the parser's nesting limit, and inlined `let` and
//!   `def` values (recursive unfoldings included) by [`MAX_INLINE_DEPTH`], so every tree
//!   is at most their sum deep;
//! - the unfolding of a recursive call is planned without recursion and built by a
//!   recursion at most [`MAX_UNFOLD_NESTING`] deep, checked by the plan;
//! - the recursive methods keep their frames small (per-form helpers, one recursive
//!   call site in `finish`), and `tests/resource_bounds.rs` runs the deepest accepted
//!   tree and 5000-link chains through the whole pipeline in a 512 KiB thread.
//!
//! # Recursive defs
//!
//! Decision: RFC 0003 "Model functions: totality and termination" (and its correction
//! 1, which completes docs/11 §8 "totality/termination for model functions"). That
//! section is normative; this is its implementation. The rule is decidable by one pass
//! over the elaborated body.
//!
//! **Termination.** A def that calls itself is admitted when one of its parameters,
//! `k`, of type `Nat` or `Int`, is a *measure*:
//!
//! - every recursive call passes `k - c` in `k`'s position, for a constant `c >= 1`;
//! - on the path from the body's root to the call, the *measure tests* — the conditions
//!   of `if`s that compare `k` with a constant (`k == n`, `n < k`, `!(k > n)`, …) —
//!   bound `k` below by `c`. A `Nat` measure starts from the bound `k >= 0`, so the
//!   `else` of `if k == 0` bounds it by `1`; an `Int` measure starts unbounded, so there
//!   `k != 0` bounds nothing and `k <= 0` is needed;
//! - no recursive call sits inside the arguments of another.
//!
//! The first parameter (in declaration order) that satisfies all three is the measure.
//! Then every chain of recursive calls from a call with measure value `v` is at most
//! `v / c_min` long, and the measure stays non-negative after the first call. A def
//! with no such parameter is [`Unsupported::NoDecreasingMeasure`], at its first
//! recursive call, whether or not it is called. There is no surface syntax for a
//! measure shared by several defs, so a cycle through two or more defs is
//! [`Unsupported::MutualRecursion`].
//!
//! **Unfolding.** A recursive def is normalized like any other def: a call is replaced
//! by its meaning, so the normalized model and the lowering never contain recursion.
//! The measure argument of a call must fold to a constant `v` (literals, `-`, `+`,
//! `*`, `min`, `max`); otherwise the call is [`Unsupported::RecursionBoundNotConstant`],
//! because the depth of the unfolding must be known before it is built. A negative
//! constant for a `Nat` measure is [`crate::ElabErrorKind::MeasureOutOfDomain`]. At
//! `v`, every measure test in the body is decided, so the `if` becomes the branch it
//! takes, the measure parameter is the literal `v`, and each recursive call that
//! remains is unfolded again at `v - c`. Each unfolding gives the body's binders fresh
//! numbers, so an argument that mentions one of them is never captured. Over the Finite
//! fragment this is exactly the def's value at the call; an unfolding that the
//! programmatic model can carry (integers and Booleans) lowers like any other
//! expression.
//!
//! **Resources.** The chain bound `v / c_min <= `[`MAX_RECURSION_DEPTH`] is checked
//! first, from the constant alone. The unfolding is then planned: its exact size and
//! depth are computed by an iterative walk that charges its work as it runs and stops as
//! soon as the size passes what the output budget has left, the depth passes
//! [`MAX_INLINE_DEPTH`], or the nesting of visits passes [`MAX_UNFOLD_NESTING`]. Each
//! argument of each recursive call is charged when it is built, whether or not the body
//! uses it. Only after the plan's size is charged is anything built.
//!
//! # Scoping
//!
//! A local name — an action parameter, a `let`, a binder, a def parameter — may not
//! shadow any name in scope, global or local. A shadowed name reads one way and means
//! another, and the Finite fragment gains nothing from allowing it.
//!
//! # Binders without a domain
//!
//! `forall epoch, v1, v2: …` is admitted. RFC 0003 writes exactly this form
//! (`forall n1,n2,e,v1,v2:` in its "Core syntax sketch"), and its second design
//! principle — "Finite checking without pretending finiteness. Types can be unbounded; a
//! run configuration selects finite bounds" — says what it means: the binder ranges over
//! its whole type. The type is inferred from the uses, and the normalized binder carries
//! it explicitly with no domain. A binder whose uses do not determine its type is
//! [`crate::ElabErrorKind::CannotInferType`]; the elaborator never picks one. Whether a
//! quantifier over a type is *finitely checkable* is not a typing question: it depends on
//! the type's bound in the run configuration, and the lowering to the programmatic model
//! refuses it with a typed reason.
//!
//! In `exists p, q in S`, the domain binds `q` only (the parser records that, see
//! `continuum_cml_syntax::parser`), so `p` ranges over its inferred type.
//!
//! # Actions
//!
//! A clause of an action is a guard when it mentions no post-state, and an update when
//! it is `next x = e` or `x' == e` for a state variable `x`. `unchanged x` and `x' == x`
//! both keep `x`. Every state variable must be specified exactly once: docs/11 §4, "The
//! checker rejects unspecified state changes unless the action explicitly opts into
//! relational postconditions". Relational postconditions — any other use of a prime —
//! are outside the Finite core fragment and are refused as unsupported.

use std::collections::BTreeMap;
use std::rc::Rc;

use continuum_cml_syntax::Span;
use continuum_cml_syntax::ast as syn;

use crate::budget::{Budget, Fuel, Limits, MAX_TYPE_DEPTH, Usage, lookup_cost, sort_cost};
use crate::error::{ElabError, ElabErrorKind, Unsupported};
use crate::norm::{
    Action, Behavior, BinOp, Binder, Builtin, Choice, ConstDecl, EnumDecl, Expr, ExprKind,
    Fairness, Init, Invariant, Next, NormModel, Param, Quant, StateVar, Strength, Temporal,
};
use crate::types::{Type, Unifier, UnifyError, has_var, type_measure};

pub use crate::budget::MAX_NODES;

/// The deepest expression an inlined `let` or `def` may produce.
///
/// Later passes recurse over the tree, so this bound keeps them within a bounded stack:
/// an elaborated tree is at most this plus the parser's `MAX_NESTING` deep. It equals
/// `MAX_NESTING`, so an inlined value is never deeper than one written out.
pub const MAX_INLINE_DEPTH: usize = 64;

/// The longest chain of recursive calls one unfolding may follow: a call of a recursive
/// def whose constant measure `v` and least decrement `step` allow more than
/// `v / step` nested calls beyond this is refused ([`crate::ElabErrorKind::TooLarge`])
/// before anything is planned or built.
pub const MAX_RECURSION_DEPTH: usize = 64;

/// The deepest nesting of visits an unfolding may make: nodes it builds, measure tests
/// it decides, and recursive calls it follows. It bounds the recursion depth of the
/// build, and it is checked while the unfolding is planned, before anything is built.
pub const MAX_UNFOLD_NESTING: usize = 512;

/// Names that the fragment reserves for built-in types and functions.
const RESERVED: &[&str] = &[
    "Bool", "Int", "Nat", "String", "Set", "Map", "Seq", "Option", "Some", "None", "min", "max",
    "step", "stutter",
];

type R<T> = Result<T, ElabError>;

fn err<T>(kind: ElabErrorKind, span: Span) -> R<T> {
    Err(ElabError::new(kind, span))
}

/// What a top-level name denotes.
#[derive(Debug, Clone)]
enum Global {
    Sort,
    Enum,
    Alias,
    Variant(String),
    Const(Type),
    State(Type),
    Def,
    Action,
    Choice,
    Init,
    Invariant,
    Behavior,
}

/// A local name.
#[derive(Debug, Clone)]
enum Local {
    Param(Type),
    Bound(u32, Type),
    /// A `let` value with its measured size and depth, so each use is charged before
    /// the copy.
    Let(Expr, usize, usize),
}

/// Where an expression appears; decides which forms are admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Site {
    /// A state-level predicate or value: init, invariant, refinement, guard, def body.
    State,
    /// Inside an action clause that is not an update (a prime is a relational
    /// postcondition here).
    Action,
    /// A behavior formula.
    Behavior,
}

/// The local names in scope, innermost last, with an ordered index by name.
///
/// Shadowing is refused, so a name occurs at most once and the index maps it to its one
/// position. Lookup, the freshness check, push, and pop cost `O(log n)` comparisons
/// (charged to the work budget by the caller), not a scan of the whole scope.
#[derive(Debug, Default)]
struct Scope {
    entries: Vec<(String, Local)>,
    index: BTreeMap<String, usize>,
}

impl Scope {
    fn len(&self) -> usize {
        self.entries.len()
    }

    fn push(&mut self, name: String, local: Local) {
        self.index.insert(name.clone(), self.entries.len());
        self.entries.push((name, local));
    }

    fn truncate(&mut self, len: usize) {
        while self.entries.len() > len {
            if let Some((name, _)) = self.entries.pop() {
                self.index.remove(&name);
            }
        }
    }

    fn get(&self, name: &str) -> Option<&Local> {
        let at = *self.index.get(name)?;
        self.entries.get(at).map(|(_, l)| l)
    }

    fn contains(&self, name: &str) -> bool {
        self.index.contains_key(name)
    }
}

/// An elaborated `def`.
#[derive(Debug, Clone)]
struct DefBody {
    params: Vec<(u32, Type)>,
    body: Expr,
    /// The termination measure of a recursive def; `None` for a def that does not call
    /// itself.
    rec: Option<Recursion>,
}

/// The termination measure of a recursive def (see "Recursive defs" in the module
/// documentation).
#[derive(Debug, Clone, Copy)]
struct Recursion {
    /// The measure parameter's position.
    index: usize,
    /// Its binder number in the elaborated body.
    binder: u32,
    /// Whether it is declared `Nat`: its value at entry is then non-negative.
    nat: bool,
    /// The least decrement over every recursive call; at least one.
    step: i64,
}

/// The def whose body is being elaborated: a call of it is a [`ExprKind::Recur`].
#[derive(Debug, Clone)]
struct Current {
    name: String,
    params: Vec<(u32, Type)>,
    ret: Type,
}

#[derive(Debug, Clone)]
enum DefState<'a> {
    Pending(&'a syn::Decl),
    InProgress,
    Done(Rc<DefBody>),
}

struct Elaborator<'a> {
    file: &'a syn::SourceFile,
    globals: BTreeMap<String, Global>,
    aliases: BTreeMap<String, &'a syn::TypeExpr>,
    /// Resolved aliases with their structural size and depth, so a use can be charged
    /// before the clone.
    alias_types: BTreeMap<String, (Type, usize, usize)>,
    alias_visiting: Vec<String>,
    /// Binders enclosing the node `finish` is at, innermost last.
    finish_scope: crate::norm::BinderScope,
    defs: BTreeMap<String, DefState<'a>>,
    unifier: Unifier,
    next_binder: u32,
    /// The output budget every amplifying allocation is charged to first.
    budget: Budget,
    /// The work budget every super-linear step is charged to.
    fuel: Fuel,
    /// Resolved sizes of solved inference variables; cleared on every unification.
    resolved_memo: BTreeMap<u32, (usize, usize)>,
    init_name: Option<String>,
    action_names: Vec<String>,
    /// The def being elaborated, so that its calls of itself become
    /// [`ExprKind::Recur`] nodes.
    current: Option<Current>,
}

/// Elaborate a parsed model.
///
/// # Errors
///
/// The first ill-formed or unsupported construct, in pass order (see the module
/// documentation).
pub fn elaborate(file: &syn::SourceFile) -> Result<NormModel, ElabError> {
    elaborate_with(file, Limits::default()).0
}

/// [`elaborate`] under explicit resource limits, reporting what it spent.
///
/// The usage is reported whether or not elaboration succeeds, so a caller (or a test)
/// can observe how the work grows with the input.
pub fn elaborate_with(
    file: &syn::SourceFile,
    limits: Limits,
) -> (Result<NormModel, ElabError>, Usage) {
    let mut e = Elaborator {
        file,
        globals: BTreeMap::new(),
        aliases: BTreeMap::new(),
        alias_types: BTreeMap::new(),
        alias_visiting: Vec::new(),
        finish_scope: crate::norm::BinderScope::default(),
        defs: BTreeMap::new(),
        unifier: Unifier::default(),
        next_binder: 0,
        budget: Budget::new(limits.nodes),
        fuel: Fuel::new(limits.work),
        resolved_memo: BTreeMap::new(),
        init_name: None,
        action_names: Vec::new(),
        current: None,
    };
    let result = e.run().and_then(|m| {
        e.collect_work(file.header.span)?;
        Ok(m)
    });
    let usage = Usage {
        nodes: e.budget.used(),
        work: e.fuel.used().saturating_add(e.unifier.take_work()),
    };
    (result, usage)
}

impl<'a> Elaborator<'a> {
    // -----------------------------------------------------------------------
    // pass 1: names
    // -----------------------------------------------------------------------

    fn declare(&mut self, name: &syn::Ident, what: Global) -> R<()> {
        self.look(&name.name, name.span)?;
        if RESERVED.contains(&name.name.as_str()) || self.globals.contains_key(&name.name) {
            return err(ElabErrorKind::DuplicateName(name.name.clone()), name.span);
        }
        self.globals.insert(name.name.clone(), what);
        Ok(())
    }

    fn collect(&mut self) -> R<()> {
        let file = self.file;
        let mut seen_init = false;
        for decl in &file.decls {
            match &decl.kind {
                syn::DeclKind::Sort { name } => self.declare(name, Global::Sort)?,
                syn::DeclKind::TypeAlias { name, ty } => {
                    self.declare(name, Global::Alias)?;
                    self.aliases.insert(name.name.clone(), ty);
                }
                syn::DeclKind::Enum { name, variants } => {
                    self.declare(name, Global::Enum)?;
                    for v in variants {
                        self.declare(v, Global::Variant(name.name.clone()))?;
                    }
                }
                // Types are resolved in pass 2; the placeholders are replaced there.
                syn::DeclKind::Const { name, .. } => {
                    self.declare(name, Global::Const(Type::Bool))?
                }
                syn::DeclKind::State { fields } => {
                    for f in fields {
                        self.declare(&f.name, Global::State(Type::Bool))?;
                    }
                }
                syn::DeclKind::Init { name, .. } => {
                    if seen_init {
                        return err(ElabErrorKind::DuplicateInit, decl.span);
                    }
                    seen_init = true;
                    if let Some(n) = name {
                        self.declare(n, Global::Init)?;
                        self.init_name = Some(n.name.clone());
                    }
                }
                syn::DeclKind::Action { name, body, .. } => match body {
                    syn::ActionBody::Block(_) => {
                        self.declare(name, Global::Action)?;
                        self.action_names.push(name.name.clone());
                    }
                    syn::ActionBody::Choice(_) => self.declare(name, Global::Choice)?,
                },
                syn::DeclKind::Invariant { name, .. } => self.declare(name, Global::Invariant)?,
                syn::DeclKind::Fairness { .. } => {}
                syn::DeclKind::Behavior { name, .. } => self.declare(name, Global::Behavior)?,
                syn::DeclKind::Def { name, .. } => {
                    self.declare(name, Global::Def)?;
                    self.defs.insert(name.name.clone(), DefState::Pending(decl));
                }
            }
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // pass 2: types
    // -----------------------------------------------------------------------

    /// Resolve a type expression. Its size and depth are computed from the syntax and
    /// the cached alias measures first, checked against [`MAX_TYPE_DEPTH`] and charged
    /// to the budget, and only then is the type built.
    fn resolve_type(&mut self, t: &syn::TypeExpr) -> R<Type> {
        let (size, depth) = self.type_expr_measure(t);
        // Measuring and building both visit the type expression once per node.
        self.burn((size as u64).saturating_mul(2), t.span)?;
        if depth > MAX_TYPE_DEPTH {
            return err(ElabErrorKind::TooLarge, t.span);
        }
        self.charge(size, t.span)?;
        self.build_type(t)
    }

    /// The size and depth [`Self::build_type`] would produce. Recursion is bounded by the
    /// parser's nesting limit; aliases contribute their cached measure.
    fn type_expr_measure(&self, t: &syn::TypeExpr) -> (usize, usize) {
        let combine = |parts: Vec<(usize, usize)>| {
            parts
                .into_iter()
                .fold((1_usize, 1_usize), |(s, d), (ps, pd)| {
                    (s.saturating_add(ps), d.max(pd.saturating_add(1)))
                })
        };
        match &t.kind {
            syn::TypeKind::Named(id) | syn::TypeKind::Applied(id, _) => {
                if let Some((_, size, depth)) = self.alias_types.get(&id.name) {
                    return (*size, *depth);
                }
                let args: &[syn::TypeExpr] = match &t.kind {
                    syn::TypeKind::Applied(_, args) => args,
                    _ => &[],
                };
                combine(args.iter().map(|a| self.type_expr_measure(a)).collect())
            }
            syn::TypeKind::Tuple(items) => {
                combine(items.iter().map(|a| self.type_expr_measure(a)).collect())
            }
            syn::TypeKind::Function(a, b) => {
                combine(vec![self.type_expr_measure(a), self.type_expr_measure(b)])
            }
            syn::TypeKind::Record(fields) => combine(
                fields
                    .iter()
                    .map(|(_, ty)| self.type_expr_measure(ty))
                    .collect(),
            ),
        }
    }

    fn build_type(&mut self, t: &syn::TypeExpr) -> R<Type> {
        match &t.kind {
            syn::TypeKind::Named(id) => self.named_type(id, &[], t.span),
            syn::TypeKind::Applied(id, args) => self.named_type(id, args, t.span),
            syn::TypeKind::Tuple(items) => Ok(Type::Tuple(
                items
                    .iter()
                    .map(|i| self.build_type(i))
                    .collect::<R<Vec<_>>>()?,
            )),
            syn::TypeKind::Function(a, b) => Ok(Type::Function(
                Box::new(self.build_type(a)?),
                Box::new(self.build_type(b)?),
            )),
            syn::TypeKind::Record(fields) => {
                let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
                let mut out: Vec<(String, Type)> = Vec::new();
                let bytes: usize = fields.iter().map(|(n, _)| n.name.len()).sum();
                self.burn(sort_cost(fields.len(), bytes).saturating_mul(2), t.span)?;
                for (name, ty) in fields {
                    if !seen.insert(name.name.as_str()) {
                        return err(ElabErrorKind::DuplicateName(name.name.clone()), name.span);
                    }
                    out.push((name.name.clone(), self.build_type(ty)?));
                }
                out.sort_by(|a, b| a.0.cmp(&b.0));
                Ok(Type::Record(out))
            }
        }
    }

    fn named_type(&mut self, id: &syn::Ident, args: &[syn::TypeExpr], span: Span) -> R<Type> {
        let arity = |expected: usize| -> R<()> {
            if args.len() == expected {
                Ok(())
            } else {
                err(
                    ElabErrorKind::TypeArity {
                        name: id.name.clone(),
                        expected,
                        found: args.len(),
                    },
                    span,
                )
            }
        };
        let arg = |this: &mut Self, i: usize| -> R<Type> {
            match args.get(i) {
                Some(a) => this.build_type(a),
                None => err(ElabErrorKind::UnknownType(id.name.clone()), span),
            }
        };
        self.look(&id.name, id.span)?;
        match id.name.as_str() {
            "Bool" => arity(0).map(|()| Type::Bool),
            "Int" => arity(0).map(|()| Type::Int),
            "Nat" => arity(0).map(|()| Type::Nat),
            "String" => arity(0).map(|()| Type::Str),
            "Set" => {
                arity(1)?;
                Ok(Type::Set(Box::new(arg(self, 0)?)))
            }
            "Seq" => {
                arity(1)?;
                Ok(Type::Seq(Box::new(arg(self, 0)?)))
            }
            "Option" => {
                arity(1)?;
                Ok(Type::Option(Box::new(arg(self, 0)?)))
            }
            "Map" => {
                arity(2)?;
                let k = arg(self, 0)?;
                let v = arg(self, 1)?;
                Ok(Type::Map(Box::new(k), Box::new(v)))
            }
            name => match self.globals.get(name) {
                Some(Global::Sort) => arity(0).map(|()| Type::Sort(name.to_owned())),
                Some(Global::Enum) => arity(0).map(|()| Type::Enum(name.to_owned())),
                Some(Global::Alias) => {
                    arity(0)?;
                    self.alias(id)
                }
                _ => err(ElabErrorKind::UnknownType(name.to_owned()), id.span),
            },
        }
    }

    /// Resolve every alias, dependencies first. The order is computed iteratively over
    /// the syntactic references, so each `alias` call finds its dependencies cached.
    fn resolve_aliases(&mut self) -> R<()> {
        let file = self.file;
        let mut nodes: Vec<(String, Vec<(String, Span)>)> = Vec::new();
        let mut idents: BTreeMap<String, &syn::Ident> = BTreeMap::new();
        for decl in &file.decls {
            if let syn::DeclKind::TypeAlias { name, ty } = &decl.kind {
                let mut refs = Vec::new();
                type_refs(ty, &mut refs);
                let cost = refs.iter().fold(0_u64, |acc, (n, _)| {
                    acc.saturating_add(lookup_cost(self.globals.len(), n.len()))
                });
                self.burn(cost, name.span)?;
                refs.retain(|(n, _)| matches!(self.globals.get(n), Some(Global::Alias)));
                nodes.push((name.name.clone(), refs));
                idents.insert(name.name.clone(), name);
            }
        }
        self.burn(order_cost(&nodes), file.header.span)?;
        let order = dependency_order(&nodes).map_err(|at| {
            let name = nodes
                .iter()
                .flat_map(|(_, refs)| refs.iter())
                .find(|(_, s)| *s == at)
                .map_or_else(String::new, |(n, _)| n.clone());
            ElabError::new(ElabErrorKind::CyclicTypeAlias(name), at)
        })?;
        for (name, _) in order {
            if let Some(id) = idents.get(&name).copied() {
                self.alias(id)?;
            }
        }
        Ok(())
    }

    /// The resolved type of an alias. A cached alias is cloned; the caller has charged
    /// its size already (through [`Self::resolve_type`]'s measure).
    fn alias(&mut self, id: &syn::Ident) -> R<Type> {
        if let Some((t, _, _)) = self.alias_types.get(&id.name) {
            return Ok(t.clone());
        }
        if self.alias_visiting.contains(&id.name) {
            return err(ElabErrorKind::CyclicTypeAlias(id.name.clone()), id.span);
        }
        let Some(target) = self.aliases.get(&id.name).copied() else {
            return err(ElabErrorKind::UnknownType(id.name.clone()), id.span);
        };
        self.alias_visiting.push(id.name.clone());
        let resolved = self.resolve_type(target)?;
        self.alias_visiting.pop();
        let (size, depth) = type_measure(&resolved);
        // The cache holds a second copy: charged like the first.
        self.charge(size, id.span)?;
        self.alias_types
            .insert(id.name.clone(), (resolved.clone(), size, depth));
        Ok(resolved)
    }

    // -----------------------------------------------------------------------
    // the driver
    // -----------------------------------------------------------------------

    fn run(&mut self) -> R<NormModel> {
        self.collect()?;
        let file = self.file;

        let mut model = NormModel {
            name: file.header.name.name.clone(),
            sorts: Vec::new(),
            enums: Vec::new(),
            constants: Vec::new(),
            state: Vec::new(),
            init: None,
            actions: Vec::new(),
            choices: Vec::new(),
            invariants: Vec::new(),
            fairness: Vec::new(),
            behaviors: Vec::new(),
        };

        // Pass 2: declared types. Aliases first, in dependency order computed without
        // recursion, so resolving one alias never recurses into another (an alias chain
        // spread over many declarations is not bounded by the parser's nesting limit).
        self.resolve_aliases()?;
        let mut state_fields: Vec<&syn::StateField> = Vec::new();
        for decl in &file.decls {
            match &decl.kind {
                syn::DeclKind::Sort { name } => model.sorts.push(name.name.clone()),
                // Aliases are resolved below, in dependency order.
                syn::DeclKind::TypeAlias { .. } => {}
                syn::DeclKind::Enum { name, variants } => model.enums.push(EnumDecl {
                    name: name.name.clone(),
                    variants: variants.iter().map(|v| v.name.clone()).collect(),
                }),
                syn::DeclKind::Const { name, ty } => {
                    let ty = self.resolve_type(ty)?;
                    self.globals
                        .insert(name.name.clone(), Global::Const(ty.clone()));
                    model.constants.push(ConstDecl {
                        name: name.name.clone(),
                        ty,
                        span: decl.span,
                    });
                }
                syn::DeclKind::State { fields } => {
                    for f in fields {
                        let ty = self.resolve_type(&f.ty)?;
                        self.globals.insert(f.name.name.clone(), Global::State(ty));
                        state_fields.push(f);
                    }
                }
                _ => {}
            }
        }

        // Pass 3: defs, in call-dependency order computed without recursion, so a def
        // body never elaborates inside another def's elaboration: stack depth is one
        // body's nesting, whatever the length of the call chain. A def's calls of itself
        // are not edges (they are checked for termination instead); a cycle through two
        // or more defs is mutual recursion, typed unsupported at the call that closes it.
        let mut nodes: Vec<(String, Vec<(String, Span)>)> = Vec::new();
        for decl in &file.decls {
            if let syn::DeclKind::Def { name, body, .. } = &decl.kind {
                let mut calls = Vec::new();
                expr_calls(body, &mut calls);
                let cost = calls.iter().fold(0_u64, |acc, (n, _)| {
                    acc.saturating_add(lookup_cost(self.globals.len(), n.len()))
                });
                self.burn(cost, name.span)?;
                calls.retain(|(n, _)| {
                    n != &name.name && matches!(self.globals.get(n), Some(Global::Def))
                });
                nodes.push((name.name.clone(), calls));
            }
        }
        self.burn(order_cost(&nodes), file.header.span)?;
        let order = dependency_order(&nodes).map_err(|at| {
            ElabError::new(ElabErrorKind::Unsupported(Unsupported::MutualRecursion), at)
        })?;
        for (name, span) in order {
            self.def(&name, span)?;
        }

        // Pass 4: the declarations that carry meaning.
        for f in &state_fields {
            self.look(&f.name.name, f.name.span)?;
            let ty = match self.globals.get(&f.name.name) {
                Some(Global::State(t)) => t.clone(),
                _ => return err(ElabErrorKind::UnknownName(f.name.name.clone()), f.name.span),
            };
            let mut refinement = Vec::new();
            if let Some(r) = &f.refinement {
                let mut scope = Scope::default();
                let e = self.expr(r, &mut scope, Site::State)?;
                self.expect(&e, &Type::Bool)?;
                conjuncts(e, &mut refinement);
                self.finish_all(&mut refinement)?;
            }
            model.state.push(StateVar {
                name: f.name.name.clone(),
                ty,
                refinement,
                span: f.span,
            });
        }
        let bytes: usize = model.state.iter().map(|v| v.name.len()).sum();
        self.sorting(model.state.len(), bytes, file.header.span)?;
        model.state.sort_by(|a, b| a.name.cmp(&b.name));
        let state_names: Vec<String> = model.state.iter().map(|v| v.name.clone()).collect();

        for decl in &file.decls {
            match &decl.kind {
                syn::DeclKind::Init { name, body } => {
                    let clauses = self.predicate_block(body)?;
                    model.init = Some(Init {
                        name: name.as_ref().map(|n| n.name.clone()),
                        clauses,
                        span: decl.span,
                    });
                }
                syn::DeclKind::Action {
                    name,
                    params,
                    body: syn::ActionBody::Block(stmts),
                } => {
                    let action = self.action(name, params, stmts, &state_names, decl.span)?;
                    model.actions.push(action);
                }
                syn::DeclKind::Action {
                    name,
                    params,
                    body: syn::ActionBody::Choice(options),
                } => {
                    if let Some(p) = params.first() {
                        return err(ElabErrorKind::NotAnAction(name.name.clone()), p.span);
                    }
                    let mut actions = Vec::new();
                    for o in options {
                        self.look(&o.name, o.span)?;
                        match self.globals.get(&o.name) {
                            Some(Global::Action) => actions.push(o.name.clone()),
                            Some(_) => {
                                return err(ElabErrorKind::NotAnAction(o.name.clone()), o.span);
                            }
                            None => return err(ElabErrorKind::UnknownName(o.name.clone()), o.span),
                        }
                    }
                    let bytes: usize = actions.iter().map(String::len).sum();
                    self.sorting(actions.len(), bytes, decl.span)?;
                    actions.sort();
                    actions.dedup();
                    model.choices.push(Choice {
                        name: name.name.clone(),
                        actions,
                        span: decl.span,
                    });
                }
                syn::DeclKind::Invariant { name, body } => {
                    let clauses = self.predicate_block(body)?;
                    model.invariants.push(Invariant {
                        name: name.name.clone(),
                        clauses,
                        span: decl.span,
                    });
                }
                syn::DeclKind::Fairness { strength, actions } => {
                    for a in actions {
                        self.look(&a.name, a.span)?;
                        match self.globals.get(&a.name) {
                            Some(Global::Action | Global::Choice) => {}
                            Some(_) => {
                                return err(ElabErrorKind::NotAnAction(a.name.clone()), a.span);
                            }
                            None => return err(ElabErrorKind::UnknownName(a.name.clone()), a.span),
                        }
                        model.fairness.push(Fairness {
                            action: a.name.clone(),
                            strength: match strength {
                                syn::FairnessStrength::Weak => Strength::Weak,
                                syn::FairnessStrength::Strong => Strength::Strong,
                            },
                            span: a.span,
                        });
                    }
                }
                syn::DeclKind::Behavior { name, expr } => {
                    let mut scope = Scope::default();
                    let mut formula = self.expr(expr, &mut scope, Site::Behavior)?;
                    self.expect(&formula, &Type::Bool)?;
                    self.finish(&mut formula)?;
                    model.behaviors.push(Behavior {
                        name: name.name.clone(),
                        formula,
                        span: decl.span,
                    });
                }
                _ => {}
            }
        }

        let sorted = [
            (
                model.sorts.len(),
                model.sorts.iter().map(String::len).sum::<usize>(),
            ),
            (
                model.enums.len(),
                model.enums.iter().map(|x| x.name.len()).sum(),
            ),
            (
                model.constants.len(),
                model.constants.iter().map(|x| x.name.len()).sum(),
            ),
            (
                model.actions.len(),
                model.actions.iter().map(|x| x.name.len()).sum(),
            ),
            (
                model.choices.len(),
                model.choices.iter().map(|x| x.name.len()).sum(),
            ),
            (
                model.invariants.len(),
                model.invariants.iter().map(|x| x.name.len()).sum(),
            ),
            (
                model.fairness.len(),
                model.fairness.iter().map(|x| x.action.len()).sum(),
            ),
            (
                model.behaviors.len(),
                model.behaviors.iter().map(|x| x.name.len()).sum(),
            ),
        ];
        for (n, bytes) in sorted {
            self.sorting(n, bytes, file.header.span)?;
        }
        model.sorts.sort();
        model.enums.sort_by(|a, b| a.name.cmp(&b.name));
        model.constants.sort_by(|a, b| a.name.cmp(&b.name));
        model.actions.sort_by(|a, b| a.name.cmp(&b.name));
        model.choices.sort_by(|a, b| a.name.cmp(&b.name));
        model.invariants.sort_by(|a, b| a.name.cmp(&b.name));
        model
            .fairness
            .sort_by(|a, b| (&a.action, a.strength).cmp(&(&b.action, b.strength)));
        model
            .fairness
            .dedup_by(|a, b| a.action == b.action && a.strength == b.strength);
        model.behaviors.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(model)
    }

    /// An `init` or `invariant` block: `let` and predicate clauses.
    fn predicate_block(&mut self, body: &[syn::Stmt]) -> R<Vec<Expr>> {
        let mut scope = Scope::default();
        let mut clauses = Vec::new();
        for s in body {
            match &s.kind {
                syn::StmtKind::Let(name, value) => {
                    let v = self.expr(value, &mut scope, Site::State)?;
                    self.bind_let(name, v, &mut scope)?;
                }
                syn::StmtKind::Expr(e) | syn::StmtKind::Require(e) => {
                    let e = self.expr(e, &mut scope, Site::State)?;
                    self.expect(&e, &Type::Bool)?;
                    conjuncts(e, &mut clauses);
                }
                syn::StmtKind::Next(..) | syn::StmtKind::Unchanged(_) => {
                    return err(ElabErrorKind::PrimeOutsideAction, s.span);
                }
            }
        }
        self.finish_all(&mut clauses)?;
        Ok(clauses)
    }

    fn action(
        &mut self,
        name: &syn::Ident,
        params: &[syn::Param],
        stmts: &[syn::Stmt],
        state_names: &[String],
        span: Span,
    ) -> R<Action> {
        let mut scope = Scope::default();
        let mut norm_params = Vec::new();
        for p in params {
            let ty = self.resolve_type(&p.ty)?;
            self.check_fresh(&p.name, &scope)?;
            scope.push(p.name.name.clone(), Local::Param(ty.clone()));
            norm_params.push(Param {
                name: p.name.name.clone(),
                ty,
            });
        }
        let mut guard: Vec<Expr> = Vec::new();
        let mut updates: BTreeMap<String, (Next, Span)> = BTreeMap::new();
        let action = name.name.clone();
        let mut record = |var: &syn::Ident, next: Next, at: Span| -> R<()> {
            if updates.contains_key(&var.name) {
                return err(
                    ElabErrorKind::ConflictingUpdate {
                        action: action.clone(),
                        variable: var.name.clone(),
                    },
                    at,
                );
            }
            updates.insert(var.name.clone(), (next, at));
            Ok(())
        };
        for s in stmts {
            // A statement may record updates in the per-action table: one lookup each.
            let cost = lookup_cost(state_names.len(), 32);
            self.burn(cost, s.span)?;
            match &s.kind {
                syn::StmtKind::Require(e) => {
                    let e = self.expr(e, &mut scope, Site::Action)?;
                    self.expect(&e, &Type::Bool)?;
                    conjuncts(e, &mut guard);
                }
                syn::StmtKind::Let(n, value) => {
                    let v = self.expr(value, &mut scope, Site::Action)?;
                    self.bind_let(n, v, &mut scope)?;
                }
                syn::StmtKind::Next(var, value) => {
                    let ty = self.state_type(var)?;
                    let v = self.expr(value, &mut scope, Site::Action)?;
                    self.expect(&v, &ty)?;
                    record(var, self.next_of(var, v), s.span)?;
                }
                syn::StmtKind::Unchanged(vars) => {
                    for var in vars {
                        self.state_type(var)?;
                        record(var, Next::Unchanged, var.span)?;
                    }
                }
                syn::StmtKind::Expr(e) => {
                    let mut parts = Vec::new();
                    syntax_conjuncts(e, &mut parts);
                    for part in parts {
                        if let Some((var, rhs)) = primed_update(part) {
                            let ty = self.state_type(var)?;
                            let v = self.expr(rhs, &mut scope, Site::Action)?;
                            self.expect(&v, &ty)?;
                            record(var, self.next_of(var, v), part.span)?;
                        } else {
                            let g = self.expr(part, &mut scope, Site::Action)?;
                            self.expect(&g, &Type::Bool)?;
                            conjuncts(g, &mut guard);
                        }
                    }
                }
            }
        }
        // One entry per state variable per action: output (charged like a node, with its
        // text) and a table lookup each.
        let mut next = Vec::new();
        for var in state_names {
            self.charge(crate::budget::text_cost(var.len()).saturating_add(1), span)?;
            self.burn(lookup_cost(updates.len(), var.len()), span)?;
            match updates.remove(var) {
                Some((mut n, _)) => {
                    if let Next::Set(e) = &mut n {
                        self.finish(e)?;
                    }
                    next.push((var.clone(), n));
                }
                None => {
                    return err(
                        ElabErrorKind::UnspecifiedStateChange {
                            action: name.name.clone(),
                            variable: var.clone(),
                        },
                        span,
                    );
                }
            }
        }
        self.finish_all(&mut guard)?;
        Ok(Action {
            name: name.name.clone(),
            params: norm_params,
            guard,
            next,
            span,
        })
    }

    /// `x' == x` keeps `x`, exactly as `unchanged x` does.
    fn next_of(&self, var: &syn::Ident, value: Expr) -> Next {
        if matches!(&value.kind, ExprKind::State(n) if n == &var.name) {
            Next::Unchanged
        } else {
            Next::Set(value)
        }
    }

    fn state_type(&mut self, var: &syn::Ident) -> R<Type> {
        self.look(&var.name, var.span)?;
        match self.globals.get(&var.name) {
            Some(Global::State(t)) => Ok(t.clone()),
            Some(_) => err(ElabErrorKind::NotAValue(var.name.clone()), var.span),
            None => err(ElabErrorKind::UnknownName(var.name.clone()), var.span),
        }
    }

    // -----------------------------------------------------------------------
    // defs
    // -----------------------------------------------------------------------

    fn def(&mut self, name: &str, at: Span) -> R<Rc<DefBody>> {
        let decl = match self.defs.get(name) {
            Some(DefState::Done(body)) => return Ok(Rc::clone(body)),
            // Calls of the def being elaborated are `Recur` nodes (see `call`), and the
            // dependency order refuses cycles through several defs first, so this is
            // reached only by mutual recursion.
            Some(DefState::InProgress) => {
                return err(ElabErrorKind::Unsupported(Unsupported::MutualRecursion), at);
            }
            Some(DefState::Pending(decl)) => *decl,
            None => return err(ElabErrorKind::UnknownFunction(name.to_owned()), at),
        };
        let syn::DeclKind::Def {
            params, ret, body, ..
        } = &decl.kind
        else {
            return err(ElabErrorKind::UnknownFunction(name.to_owned()), at);
        };
        self.defs.insert(name.to_owned(), DefState::InProgress);
        self.def_body(name, params, ret, body)
    }

    fn def_body(
        &mut self,
        name: &str,
        params: &[syn::Param],
        ret: &syn::TypeExpr,
        body: &syn::Expr,
    ) -> R<Rc<DefBody>> {
        let mut scope = Scope::default();
        let mut ids = Vec::new();
        for p in params {
            let ty = self.resolve_type(&p.ty)?;
            self.check_fresh(&p.name, &scope)?;
            let id = self.fresh_binder();
            scope.push(p.name.name.clone(), Local::Bound(id, ty.clone()));
            ids.push((id, ty));
        }
        let ret = self.resolve_type(ret)?;
        let outer = self.current.replace(Current {
            name: name.to_owned(),
            params: ids.clone(),
            ret: ret.clone(),
        });
        let elaborated = self.expr(body, &mut scope, Site::State);
        self.current = outer;
        let mut e = elaborated?;
        self.expect(&e, &ret)?;
        self.finish(&mut e)?;
        let rec = self.termination(&ids, &e, body.span)?;
        let done = Rc::new(DefBody {
            params: ids,
            body: e,
            rec,
        });
        self.defs
            .insert(name.to_owned(), DefState::Done(Rc::clone(&done)));
        Ok(done)
    }

    // -----------------------------------------------------------------------
    // recursive defs
    // -----------------------------------------------------------------------

    /// A call of the def being elaborated, from inside its own body: a
    /// [`ExprKind::Recur`] node of the declared return type. It is unfolded later, at
    /// each call from outside.
    #[inline(never)]
    fn recur(
        &mut self,
        current: Current,
        args: &[syn::Expr],
        scope: &mut Scope,
        site: Site,
        span: Span,
    ) -> R<Expr> {
        let a = self.args(args, scope, site)?;
        for ((_, ty), x) in current.params.iter().zip(&a) {
            self.unify(&x.ty, ty, x.span)?;
        }
        self.node(
            ExprKind::Recur {
                function: current.name,
                args: a,
            },
            current.ret,
            span,
        )
    }

    /// The termination check of an elaborated def body (see "Recursive defs" in the
    /// module documentation): `None` when the body does not call its def, the measure
    /// when one parameter decreases at every recursive call, and
    /// [`Unsupported::NoDecreasingMeasure`] otherwise.
    ///
    /// The work is predictable and charged before each pass: one scan for recursive
    /// calls, then per candidate parameter one visit of each node plus the constant
    /// folding of the measure tests and decrements, at most three units per node.
    fn termination(
        &mut self,
        params: &[(u32, Type)],
        body: &Expr,
        at: Span,
    ) -> R<Option<Recursion>> {
        let size = measure(body).0 as u64;
        self.burn(size.saturating_mul(2), at)?;
        let Some(first) = first_recur(body) else {
            return Ok(None);
        };
        for (index, (binder, ty)) in params.iter().enumerate() {
            let nat = match ty {
                Type::Nat => true,
                Type::Int => false,
                _ => continue,
            };
            self.burn(size.saturating_mul(3), at)?;
            if let Some(step) = decreases(body, index, *binder, nat) {
                return Ok(Some(Recursion {
                    index,
                    binder: *binder,
                    nat,
                    step,
                }));
            }
        }
        err(
            ElabErrorKind::Unsupported(Unsupported::NoDecreasingMeasure),
            first,
        )
    }

    /// Unfold a call of a recursive def from outside its body.
    ///
    /// The measure argument must fold to a constant `v`; the unfolding chain is then at
    /// most `v / step` calls long, and that bound ([`MAX_RECURSION_DEPTH`]) is checked
    /// first. The exact size and depth of the unfolding are then computed without
    /// building it (`plan_tree`, which charges its work as it runs and stops at the
    /// output budget, at [`MAX_INLINE_DEPTH`], and at [`MAX_UNFOLD_NESTING`]), charged,
    /// and only then is the tree built.
    #[inline(never)]
    fn unfold(
        &mut self,
        name: &str,
        def: &Rc<DefBody>,
        rec: Recursion,
        args: Vec<Expr>,
        span: Span,
    ) -> R<Expr> {
        for ((_, ty), x) in def.params.iter().zip(&args) {
            self.unify(&x.ty, ty, x.span)?;
        }
        let Some(measure_arg) = args.get(rec.index) else {
            return err(ElabErrorKind::UnknownFunction(name.to_owned()), span);
        };
        let mut visits = 0_u64;
        let value = const_int(measure_arg, &mut visits);
        self.burn(visits, measure_arg.span)?;
        let Some(v) = value else {
            return err(
                ElabErrorKind::Unsupported(Unsupported::RecursionBoundNotConstant),
                measure_arg.span,
            );
        };
        if rec.nat && v < 0 {
            return err(
                ElabErrorKind::MeasureOutOfDomain {
                    function: name.to_owned(),
                    value: v,
                },
                measure_arg.span,
            );
        }
        // Every recursive call is reached only with the measure at least `step`, and
        // passes it minus at least `step`: the chain is at most `v / step` long.
        if v.max(0) / rec.step > MAX_RECURSION_DEPTH as i64 {
            return err(ElabErrorKind::TooLarge, span);
        }
        let lit = literal(v, span);
        let lit_measure = measure(&lit);
        let mut measures: BTreeMap<u32, (usize, usize)> = BTreeMap::new();
        for (i, ((id, _), x)) in def.params.iter().zip(&args).enumerate() {
            let m = if i == rec.index {
                lit_measure
            } else {
                measure(x)
            };
            self.burn(m.0 as u64, x.span)?;
            measures.insert(*id, m);
        }
        let frame = Rc::new(PlanFrame {
            v,
            params: measures,
        });
        let (size, depth) = self.plan_tree(&def.body, frame, rec, Some(def), 1, span)?;
        // The literal for the measure is built once, besides the unfolding.
        self.precharge_copy(size.saturating_add(lit_measure.0), depth, span)?;

        let mut subst: BTreeMap<u32, Expr> = BTreeMap::new();
        let mut lit = Some(lit);
        for (i, ((id, _), x)) in def.params.iter().zip(args).enumerate() {
            let value = if i == rec.index {
                lit.take().unwrap_or(x)
            } else {
                x
            };
            subst.insert(*id, value);
        }
        let mut unfolder = Unfolder {
            params: &def.params,
            body: &def.body,
            rec,
            next_binder: self.next_binder,
        };
        let mut frame = BuildFrame {
            v,
            subst,
            rename: BTreeMap::new(),
        };
        let built = unfolder.build(&def.body, &mut frame);
        self.next_binder = unfolder.next_binder;
        Ok(built)
    }

    /// The exact size and depth [`Unfolder::build`] produces from `root` in `frame`,
    /// computed without building anything and without recursion.
    ///
    /// `def` is the recursive def when `root` is its body; it is `None` for the argument
    /// of a recursive call, which has no recursive call of its own (the termination
    /// check refuses one). Work is two units per visited node (the build visits it
    /// again) and the constant folding of each measure test, charged as it runs.
    /// Refused, typed, as soon as the size passes what the output budget has left, the
    /// depth passes [`MAX_INLINE_DEPTH`], or the nesting of visits (which is the build's
    /// recursion depth) passes [`MAX_UNFOLD_NESTING`].
    fn plan_tree(
        &mut self,
        root: &Expr,
        frame: Rc<PlanFrame>,
        rec: Recursion,
        def: Option<&Rc<DefBody>>,
        start: usize,
        at: Span,
    ) -> R<(usize, usize)> {
        let left = self.budget.left();
        let lit_measure = measure(&literal(0, at));
        let mut size = 0_usize;
        let mut deepest = 0_usize;
        let mut stack: Vec<(&Expr, usize, usize, Rc<PlanFrame>)> = vec![(root, 1, start, frame)];
        while let Some((e, depth, visit, frame)) = stack.pop() {
            self.burn(2, e.span)?;
            if visit > MAX_UNFOLD_NESTING {
                return err(ElabErrorKind::TooLarge, at);
            }
            match &e.kind {
                ExprKind::If(c, a, b) => {
                    let mut visits = 0_u64;
                    let test = measure_test(c, rec.binder, &mut visits);
                    self.burn(visits, c.span)?;
                    if let Some((op, n)) = test {
                        let branch = if holds(op, n, frame.v) { a } else { b };
                        stack.push((branch, depth, visit.saturating_add(1), frame));
                        continue;
                    }
                }
                ExprKind::Bound { binder, .. } => {
                    if let Some(&(s, d)) = frame.params.get(binder) {
                        size = size.saturating_add(s);
                        deepest = deepest.max(depth.saturating_sub(1).saturating_add(d));
                        if size > left || deepest > MAX_INLINE_DEPTH {
                            return err(ElabErrorKind::TooLarge, at);
                        }
                        continue;
                    }
                }
                ExprKind::Recur { args, .. } => {
                    let Some(def) = def else {
                        return err(
                            ElabErrorKind::Unsupported(Unsupported::NoDecreasingMeasure),
                            e.span,
                        );
                    };
                    let mut visits = 0_u64;
                    let c = args
                        .get(rec.index)
                        .and_then(|m| decrement(m, rec.binder, &mut visits));
                    self.burn(visits, e.span)?;
                    // The termination check guarantees `step <= c <= frame.v` here.
                    let next = c
                        .and_then(|c| frame.v.checked_sub(c))
                        .filter(|n| (0..frame.v).contains(n));
                    let Some(next) = next else {
                        return err(
                            ElabErrorKind::Unsupported(Unsupported::NoDecreasingMeasure),
                            e.span,
                        );
                    };
                    let mut params: BTreeMap<u32, (usize, usize)> = BTreeMap::new();
                    for (i, ((id, _), x)) in def.params.iter().zip(args).enumerate() {
                        let m = if i == rec.index {
                            lit_measure
                        } else {
                            let inner = visit.saturating_add(1);
                            self.plan_tree(x, Rc::clone(&frame), rec, None, inner, at)?
                        };
                        // Each argument is built once, whether or not the body uses it.
                        size = size.saturating_add(m.0);
                        params.insert(*id, m);
                    }
                    if size > left {
                        return err(ElabErrorKind::TooLarge, at);
                    }
                    let next = Rc::new(PlanFrame { v: next, params });
                    stack.push((&def.body, depth, visit.saturating_add(1), next));
                    continue;
                }
                _ => {}
            }
            size = size
                .saturating_add(own_cost(&e.kind))
                .saturating_add(type_measure(&e.ty).0);
            deepest = deepest.max(depth);
            if size > left || deepest > MAX_INLINE_DEPTH {
                return err(ElabErrorKind::TooLarge, at);
            }
            for child in children(e) {
                stack.push((
                    child,
                    depth.saturating_add(1),
                    visit.saturating_add(1),
                    Rc::clone(&frame),
                ));
            }
        }
        Ok((size, deepest))
    }

    // -----------------------------------------------------------------------
    // scope helpers
    // -----------------------------------------------------------------------

    fn check_fresh(&mut self, name: &syn::Ident, scope: &Scope) -> R<()> {
        // Three ordered-table lookups (the reserved list is a constant).
        let cost = lookup_cost(self.globals.len(), name.name.len())
            .saturating_add(lookup_cost(scope.len(), name.name.len()))
            .saturating_add(lookup_cost(RESERVED.len(), name.name.len()));
        self.burn(cost, name.span)?;
        if RESERVED.contains(&name.name.as_str())
            || self.globals.contains_key(&name.name)
            || scope.contains(&name.name)
        {
            return err(ElabErrorKind::DuplicateName(name.name.clone()), name.span);
        }
        Ok(())
    }

    fn bind_let(&mut self, name: &syn::Ident, value: Expr, scope: &mut Scope) -> R<()> {
        self.check_fresh(name, scope)?;
        let (size, depth) = measure(&value);
        self.burn(size as u64, name.span)?;
        scope.push(name.name.clone(), Local::Let(value, size, depth));
        Ok(())
    }

    fn fresh_binder(&mut self) -> u32 {
        let id = self.next_binder;
        self.next_binder = self.next_binder.saturating_add(1);
        id
    }

    /// Charge one lookup of `name` in the global table.
    fn look(&mut self, name: &str, at: Span) -> R<()> {
        let cost = lookup_cost(self.globals.len(), name.len());
        self.burn(cost, at)
    }

    /// Charge sorting `n` names of `bytes` bytes in total.
    fn sorting(&mut self, n: usize, bytes: usize, at: Span) -> R<()> {
        self.burn(sort_cost(n, bytes), at)
    }

    /// Spend `n` units of work, or refuse with [`ElabErrorKind::WorkLimitExceeded`].
    fn burn(&mut self, n: u64, at: Span) -> R<()> {
        self.fuel
            .burn(n)
            .map_err(|_| ElabError::new(ElabErrorKind::WorkLimitExceeded, at))
    }

    /// Charge the steps the unifier took since the last collection.
    fn collect_work(&mut self, at: Span) -> R<()> {
        let steps = self.unifier.take_work();
        self.burn(steps, at)
    }

    /// Charge `n` nodes to the budget, or refuse with a typed resource error.
    fn charge(&mut self, n: usize, at: Span) -> R<()> {
        self.budget
            .charge(n)
            .map_err(|_| ElabError::new(ElabErrorKind::TooLarge, at))
    }

    /// Build one node. Its children exist and were charged; the node itself and its
    /// type (by structural size) are charged here, and a type deeper than
    /// [`MAX_TYPE_DEPTH`] is refused.
    fn node(&mut self, kind: ExprKind, ty: Type, span: Span) -> R<Expr> {
        self.collect_work(span)?;
        let (ty_size, ty_depth) = type_measure(&ty);
        if ty_depth > MAX_TYPE_DEPTH {
            return err(ElabErrorKind::TooLarge, span);
        }
        self.charge(ty_size.saturating_add(own_cost(&kind)), span)?;
        Ok(Expr { kind, ty, span })
    }

    /// Charge a copy of `size` nodes and bound its depth, before the copy is made.
    fn precharge_copy(&mut self, size: usize, depth: usize, at: Span) -> R<()> {
        if depth > MAX_INLINE_DEPTH {
            return err(ElabErrorKind::TooLarge, at);
        }
        self.charge(size, at)
    }

    /// The head constructor of `t` with solved variables at the top followed. The copy
    /// is of a structure already stored and charged (a node type or a solution); its
    /// components may still be variables, and nothing below the head is resolved.
    fn head_type(&mut self, t: &Type, at: Span) -> R<Type> {
        let head = self.unifier.head(t).clone();
        self.burn(type_measure(&head).0 as u64, at)?;
        self.collect_work(at)?;
        Ok(head)
    }

    /// Resolve `t` completely: its resolved size and depth are computed first (without
    /// building it), the depth is bounded by [`MAX_TYPE_DEPTH`], the size is charged,
    /// and only then is the type materialized. Solutions share structure, so a resolved
    /// type can be exponentially larger than what is stored.
    fn resolve_charged(&mut self, t: &Type, at: Span) -> R<Type> {
        let (size, depth) = self.unifier.measure_resolved(t, &mut self.resolved_memo);
        self.collect_work(at)?;
        if depth > MAX_TYPE_DEPTH {
            return err(ElabErrorKind::TooLarge, at);
        }
        self.charge(size, at)?;
        Ok(self.unifier.resolve(t))
    }

    /// Canonical sort keys for literal elements, in the current binder scope. The keys
    /// are text proportional to each element's measure, which is charged first.
    fn sort_keys(&mut self, items: &[&Expr], at: Span) -> R<Vec<String>> {
        let mut total = 0_usize;
        for x in items {
            total = total.saturating_add(measure(x).0);
        }
        self.charge(total, at)?;
        // Rendering the keys is linear in what was just charged; sorting them costs at
        // most `⌈log₂ n⌉` comparisons per key, charged before the caller sorts.
        self.burn(total as u64, at)?;
        let keys = crate::norm::keys_in_scope(items, &mut self.finish_scope);
        let bytes: usize = keys.iter().map(String::len).sum();
        self.sorting(keys.len(), bytes, at)?;
        Ok(keys)
    }

    /// A fresh inference variable, charged.
    fn fresh(&mut self, at: Span) -> R<Type> {
        self.charge(1, at)?;
        Ok(self.unifier.fresh())
    }

    fn expect(&mut self, e: &Expr, ty: &Type) -> R<()> {
        self.unify(&e.ty, ty, e.span)
    }

    fn unify(&mut self, found: &Type, expected: &Type, at: Span) -> R<()> {
        // Any solution may change: resolved measures cached so far are stale.
        self.resolved_memo.clear();
        let result = self.unifier.unify(expected, found, &mut self.budget);
        self.collect_work(at)?;
        match result {
            Ok(()) => Ok(()),
            Err(UnifyError::Exhausted) => err(ElabErrorKind::TooLarge, at),
            Err(UnifyError::Mismatch) => err(
                ElabErrorKind::TypeMismatch {
                    expected: self.unifier.describe(expected),
                    found: self.unifier.describe(found),
                },
                at,
            ),
        }
    }

    fn integer(&mut self, e: &Expr) -> R<()> {
        self.expect(e, &Type::Int)
    }

    /// A set type `Set[a]`, returning `a`.
    fn element_of(&mut self, e: &Expr) -> R<Type> {
        let a = self.fresh(e.span)?;
        self.expect(e, &Type::Set(Box::new(a.clone())))?;
        Ok(a)
    }

    // -----------------------------------------------------------------------
    // expressions
    // -----------------------------------------------------------------------

    /// Elaborate one expression.
    ///
    /// A thin dispatcher: every recursive form is elaborated in its own
    /// `#[inline(never)]` method, so one level of expression nesting costs this frame
    /// plus that form's frame, not the locals of every form at once. The recursion is
    /// bounded by the parser's `MAX_NESTING`, and `tests/errors.rs` measures the
    /// deepest accepted expression inside a thread with a small stack.
    fn expr(&mut self, e: &syn::Expr, scope: &mut Scope, site: Site) -> R<Expr> {
        let span = e.span;
        match &e.kind {
            syn::ExprKind::Int(n) => match i64::try_from(*n) {
                Ok(v) => self.node(ExprKind::Int(v), Type::Int, span),
                Err(_) => err(
                    ElabErrorKind::Unsupported(Unsupported::IntegerBeyondI64),
                    span,
                ),
            },
            syn::ExprKind::Bool(b) => self.node(ExprKind::Bool(*b), Type::Bool, span),
            syn::ExprKind::Str(s) => self.node(ExprKind::Str(s.clone()), Type::Str, span),
            syn::ExprKind::Name(id) => self.name(id, scope, site),
            syn::ExprKind::WholeState => {
                if site == Site::Behavior {
                    err(ElabErrorKind::NotAValue("state".to_owned()), span)
                } else {
                    err(ElabErrorKind::TemporalOutsideBehavior, span)
                }
            }
            syn::ExprKind::Prime(_) => match site {
                Site::Action => err(
                    ElabErrorKind::Unsupported(Unsupported::RelationalPostcondition),
                    span,
                ),
                _ => err(ElabErrorKind::PrimeOutsideAction, span),
            },
            syn::ExprKind::Unary(op, inner) => self.e_unary(*op, inner, scope, site, span),
            syn::ExprKind::Binary(op, l, r) => self.binary(*op, l, r, scope, site, span),
            syn::ExprKind::Quant(q, binders, body) => {
                self.e_quant(*q, binders, body, scope, site, span)
            }
            syn::ExprKind::If(c, a, b) => self.e_if(c, a, b, scope, site, span),
            syn::ExprKind::Temporal(op, inner) => self.e_temporal(*op, inner, scope, site, span),
            syn::ExprKind::Call(f, args) => self.call(f, args, scope, site, span),
            syn::ExprKind::Method(recv, m, args) => self.e_method(recv, m, args, scope, site, span),
            syn::ExprKind::Field(inner, f) => self.e_field(inner, f, scope, site, span),
            syn::ExprKind::Index(m, k) => self.e_index(m, k, scope, site, span),
            syn::ExprKind::Update(m, k, v) => self.e_update(m, k, v, scope, site, span),
            syn::ExprKind::Tuple(items) => self.e_tuple(items, scope, site, span),
            syn::ExprKind::EmptyBraces => {
                // Set or map: decided from the uses when the declaration is finished.
                let ty = self.fresh(span)?;
                self.node(ExprKind::SetLit(Vec::new()), ty, span)
            }
            syn::ExprKind::SetLit(items) => self.e_collection(items, false, scope, site, span),
            syn::ExprKind::SeqLit(items) => self.e_collection(items, true, scope, site, span),
            syn::ExprKind::MapLit(entries) => self.e_map_lit(entries, scope, site, span),
            syn::ExprKind::SetComp(elem, binders, filter) => {
                self.e_set_comp(elem, binders, filter.as_deref(), scope, site, span)
            }
            syn::ExprKind::MapComp(k, v, binders, filter) => {
                self.e_map_comp(k, v, binders, filter.as_deref(), scope, site, span)
            }
            syn::ExprKind::Record(fields) => self.e_record(fields, scope, site, span),
        }
    }

    #[inline(never)]
    fn e_unary(
        &mut self,
        op: syn::UnOp,
        inner: &syn::Expr,
        scope: &mut Scope,
        site: Site,
        span: Span,
    ) -> R<Expr> {
        let i = self.expr(inner, scope, site)?;
        match op {
            syn::UnOp::Not => {
                self.expect(&i, &Type::Bool)?;
                self.node(ExprKind::Not(Box::new(i)), Type::Bool, span)
            }
            syn::UnOp::Neg => {
                self.integer(&i)?;
                self.node(ExprKind::Neg(Box::new(i)), Type::Int, span)
            }
        }
    }

    #[inline(never)]
    fn e_quant(
        &mut self,
        q: syn::Quantifier,
        binders: &[syn::Binder],
        body: &syn::Expr,
        scope: &mut Scope,
        site: Site,
        span: Span,
    ) -> R<Expr> {
        let depth = scope.len();
        let bs = self.binders(binders, scope, site)?;
        let b = self.expr(body, scope, site)?;
        scope.truncate(depth);
        self.expect(&b, &Type::Bool)?;
        let q = match q {
            syn::Quantifier::Forall => Quant::Forall,
            syn::Quantifier::Exists => Quant::Exists,
        };
        self.node(ExprKind::Quant(q, bs, Box::new(b)), Type::Bool, span)
    }

    #[inline(never)]
    fn e_if(
        &mut self,
        c: &syn::Expr,
        a: &syn::Expr,
        b: &syn::Expr,
        scope: &mut Scope,
        site: Site,
        span: Span,
    ) -> R<Expr> {
        let c = self.expr(c, scope, site)?;
        self.expect(&c, &Type::Bool)?;
        let a = self.expr(a, scope, site)?;
        let b = self.expr(b, scope, site)?;
        self.unify(&b.ty, &a.ty, b.span)?;
        let ty = a.ty.clone();
        self.node(
            ExprKind::If(Box::new(c), Box::new(a), Box::new(b)),
            ty,
            span,
        )
    }

    #[inline(never)]
    fn e_temporal(
        &mut self,
        op: syn::TemporalOp,
        inner: &syn::Expr,
        scope: &mut Scope,
        site: Site,
        span: Span,
    ) -> R<Expr> {
        if site != Site::Behavior {
            return err(ElabErrorKind::TemporalOutsideBehavior, span);
        }
        let i = self.expr(inner, scope, site)?;
        self.expect(&i, &Type::Bool)?;
        let op = match op {
            syn::TemporalOp::Always => Temporal::Always,
            syn::TemporalOp::Eventually => Temporal::Eventually,
        };
        self.node(ExprKind::Temporal(op, Box::new(i)), Type::Bool, span)
    }

    #[inline(never)]
    fn e_method(
        &mut self,
        recv: &syn::Expr,
        m: &syn::Ident,
        args: &[syn::Expr],
        scope: &mut Scope,
        site: Site,
        span: Span,
    ) -> R<Expr> {
        let r = self.expr(recv, scope, site)?;
        let a = self.args(args, scope, site)?;
        self.method(r, m, a, span)
    }

    #[inline(never)]
    fn e_field(
        &mut self,
        inner: &syn::Expr,
        f: &syn::Ident,
        scope: &mut Scope,
        site: Site,
        span: Span,
    ) -> R<Expr> {
        let i = self.expr(inner, scope, site)?;
        match self.head_type(&i.ty, i.span)? {
            Type::Record(fields) => match fields.iter().find(|(n, _)| n == &f.name) {
                Some((_, t)) => {
                    let t = t.clone();
                    self.node(ExprKind::Field(Box::new(i), f.name.clone()), t, span)
                }
                None => err(ElabErrorKind::UnknownField(f.name.clone()), f.span),
            },
            Type::Var(_) => err(
                ElabErrorKind::CannotInferType("the record".to_owned()),
                i.span,
            ),
            _ => err(
                ElabErrorKind::TypeMismatch {
                    expected: "a record".to_owned(),
                    found: self.unifier.describe(&i.ty),
                },
                i.span,
            ),
        }
    }

    #[inline(never)]
    fn e_index(
        &mut self,
        m: &syn::Expr,
        k: &syn::Expr,
        scope: &mut Scope,
        site: Site,
        span: Span,
    ) -> R<Expr> {
        let m = self.expr(m, scope, site)?;
        let k = self.expr(k, scope, site)?;
        let ty = self.indexed(&m, &k)?;
        self.node(ExprKind::Index(Box::new(m), Box::new(k)), ty, span)
    }

    #[inline(never)]
    fn e_update(
        &mut self,
        m: &syn::Expr,
        k: &syn::Expr,
        v: &syn::Expr,
        scope: &mut Scope,
        site: Site,
        span: Span,
    ) -> R<Expr> {
        let m = self.expr(m, scope, site)?;
        let k = self.expr(k, scope, site)?;
        let v = self.expr(v, scope, site)?;
        let vt = self.indexed(&m, &k)?;
        self.unify(&v.ty, &vt, v.span)?;
        let ty = m.ty.clone();
        self.node(
            ExprKind::Update(Box::new(m), Box::new(k), Box::new(v)),
            ty,
            span,
        )
    }

    #[inline(never)]
    fn e_tuple(
        &mut self,
        items: &[syn::Expr],
        scope: &mut Scope,
        site: Site,
        span: Span,
    ) -> R<Expr> {
        let out = self.args(items, scope, site)?;
        let ty = Type::Tuple(out.iter().map(|i| i.ty.clone()).collect());
        self.node(ExprKind::Tuple(out), ty, span)
    }

    /// A set literal (`seq == false`) or a sequence literal.
    #[inline(never)]
    fn e_collection(
        &mut self,
        items: &[syn::Expr],
        seq: bool,
        scope: &mut Scope,
        site: Site,
        span: Span,
    ) -> R<Expr> {
        let elem = self.fresh(span)?;
        let mut out = Vec::new();
        for i in items {
            let x = self.expr(i, scope, site)?;
            self.unify(&x.ty, &elem, x.span)?;
            out.push(x);
        }
        if seq {
            self.node(ExprKind::SeqLit(out), Type::Seq(Box::new(elem)), span)
        } else {
            self.node(ExprKind::SetLit(out), Type::Set(Box::new(elem)), span)
        }
    }

    #[inline(never)]
    fn e_map_lit(
        &mut self,
        entries: &[(syn::Expr, syn::Expr)],
        scope: &mut Scope,
        site: Site,
        span: Span,
    ) -> R<Expr> {
        let kt = self.fresh(span)?;
        let vt = self.fresh(span)?;
        let mut out = Vec::new();
        for (k, v) in entries {
            let k = self.expr(k, scope, site)?;
            self.unify(&k.ty, &kt, k.span)?;
            let v = self.expr(v, scope, site)?;
            self.unify(&v.ty, &vt, v.span)?;
            out.push((k, v));
        }
        self.node(
            ExprKind::MapLit(out),
            Type::Map(Box::new(kt), Box::new(vt)),
            span,
        )
    }

    #[inline(never)]
    fn e_set_comp(
        &mut self,
        elem: &syn::Expr,
        binders: &[syn::Binder],
        filter: Option<&syn::Expr>,
        scope: &mut Scope,
        site: Site,
        span: Span,
    ) -> R<Expr> {
        let depth = scope.len();
        let bs = self.binders(binders, scope, site)?;
        let x = self.expr(elem, scope, site)?;
        let f = self.filter(filter, scope, site)?;
        scope.truncate(depth);
        let ty = Type::Set(Box::new(x.ty.clone()));
        self.node(ExprKind::SetComp(Box::new(x), bs, f), ty, span)
    }

    #[allow(clippy::too_many_arguments)]
    #[inline(never)]
    fn e_map_comp(
        &mut self,
        k: &syn::Expr,
        v: &syn::Expr,
        binders: &[syn::Binder],
        filter: Option<&syn::Expr>,
        scope: &mut Scope,
        site: Site,
        span: Span,
    ) -> R<Expr> {
        let depth = scope.len();
        let bs = self.binders(binders, scope, site)?;
        let k = self.expr(k, scope, site)?;
        let v = self.expr(v, scope, site)?;
        let f = self.filter(filter, scope, site)?;
        scope.truncate(depth);
        let ty = Type::Map(Box::new(k.ty.clone()), Box::new(v.ty.clone()));
        self.node(ExprKind::MapComp(Box::new(k), Box::new(v), bs, f), ty, span)
    }

    #[inline(never)]
    fn e_record(
        &mut self,
        fields: &[(syn::Ident, syn::Expr)],
        scope: &mut Scope,
        site: Site,
        span: Span,
    ) -> R<Expr> {
        let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
        let mut out: Vec<(String, Expr)> = Vec::new();
        let bytes: usize = fields.iter().map(|(n, _)| n.name.len()).sum();
        self.burn(sort_cost(fields.len(), bytes).saturating_mul(2), span)?;
        for (n, v) in fields {
            if !seen.insert(n.name.as_str()) {
                return err(ElabErrorKind::DuplicateName(n.name.clone()), n.span);
            }
            out.push((n.name.clone(), self.expr(v, scope, site)?));
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        let ty = Type::Record(out.iter().map(|(n, v)| (n.clone(), v.ty.clone())).collect());
        self.node(ExprKind::Record(out), ty, span)
    }

    fn filter(
        &mut self,
        f: Option<&syn::Expr>,
        scope: &mut Scope,
        site: Site,
    ) -> R<Option<Box<Expr>>> {
        match f {
            None => Ok(None),
            Some(f) => {
                let f = self.expr(f, scope, site)?;
                self.expect(&f, &Type::Bool)?;
                Ok(Some(Box::new(f)))
            }
        }
    }

    /// Elaborate binders left to right; each binder's domain sees the binders before it.
    /// The binders stay in `scope`; the caller truncates.
    fn binders(
        &mut self,
        binders: &[syn::Binder],
        scope: &mut Scope,
        site: Site,
    ) -> R<Vec<(u32, Binder)>> {
        let mut out = Vec::new();
        for b in binders {
            let (ty, domain) = match &b.domain {
                Some(d) => {
                    let d = self.expr(d, scope, site)?;
                    let t = self.element_of(&d)?;
                    (t, Some(d))
                }
                None => (self.fresh(b.span)?, None),
            };
            self.check_fresh(&b.name, scope)?;
            let id = self.fresh_binder();
            scope.push(b.name.name.clone(), Local::Bound(id, ty.clone()));
            out.push((
                id,
                Binder {
                    name: b.name.name.clone(),
                    ty,
                    domain,
                },
            ));
        }
        Ok(out)
    }

    fn name(&mut self, id: &syn::Ident, scope: &Scope, site: Site) -> R<Expr> {
        let span = id.span;
        let cost = lookup_cost(scope.len(), id.name.len())
            .saturating_add(lookup_cost(self.globals.len(), id.name.len()));
        self.burn(cost, span)?;
        if let Some(local) = scope.get(&id.name) {
            return match local {
                Local::Param(t) => self.node(ExprKind::Param(id.name.clone()), t.clone(), span),
                Local::Bound(b, t) => self.node(
                    ExprKind::Bound {
                        name: id.name.clone(),
                        binder: *b,
                    },
                    t.clone(),
                    span,
                ),
                Local::Let(e, size, depth) => {
                    let (size, depth) = (*size, *depth);
                    self.precharge_copy(size, depth, span)?;
                    Ok(e.clone())
                }
            };
        }
        if id.name == "None" {
            let a = self.fresh(span)?;
            return self.node(ExprKind::OptionNone, Type::Option(Box::new(a)), span);
        }
        match self.globals.get(&id.name).cloned() {
            Some(Global::State(t)) => self.node(ExprKind::State(id.name.clone()), t, span),
            Some(Global::Const(t)) => self.node(ExprKind::Const(id.name.clone()), t, span),
            Some(Global::Variant(en)) => self.node(
                ExprKind::Variant {
                    enumeration: en.clone(),
                    variant: id.name.clone(),
                },
                Type::Enum(en),
                span,
            ),
            Some(Global::Init) => {
                if site == Site::Behavior {
                    self.node(ExprKind::InitRef(id.name.clone()), Type::Bool, span)
                } else {
                    err(ElabErrorKind::TemporalOutsideBehavior, span)
                }
            }
            Some(_) => err(ElabErrorKind::NotAValue(id.name.clone()), span),
            None => err(ElabErrorKind::UnknownName(id.name.clone()), span),
        }
    }

    fn call(
        &mut self,
        f: &syn::Ident,
        args: &[syn::Expr],
        scope: &mut Scope,
        site: Site,
        span: Span,
    ) -> R<Expr> {
        let arity = |expected: usize| -> R<()> {
            if args.len() == expected {
                Ok(())
            } else {
                err(
                    ElabErrorKind::Arity {
                        name: f.name.clone(),
                        expected,
                        found: args.len(),
                    },
                    span,
                )
            }
        };
        match f.name.as_str() {
            "step" | "stutter" if site != Site::Behavior => {
                err(ElabErrorKind::TemporalOutsideBehavior, span)
            }
            "step" => {
                arity(1)?;
                if let Some(syn::ExprKind::Name(a)) = args.first().map(|a| &a.kind) {
                    self.look(&a.name, a.span)?;
                }
                match args.first().map(|a| &a.kind) {
                    Some(syn::ExprKind::Name(a)) => match self.globals.get(&a.name) {
                        Some(Global::Action | Global::Choice) => {
                            self.node(ExprKind::Step(a.name.clone()), Type::Bool, span)
                        }
                        Some(_) => err(ElabErrorKind::NotAnAction(a.name.clone()), a.span),
                        None => err(ElabErrorKind::UnknownName(a.name.clone()), a.span),
                    },
                    _ => err(ElabErrorKind::NotAnAction("step".to_owned()), span),
                }
            }
            "stutter" => {
                arity(1)?;
                match args.first().map(|a| &a.kind) {
                    Some(syn::ExprKind::WholeState) => {
                        self.node(ExprKind::Stutter, Type::Bool, span)
                    }
                    _ => err(
                        ElabErrorKind::TypeMismatch {
                            expected: "`state`".to_owned(),
                            found: "an expression".to_owned(),
                        },
                        span,
                    ),
                }
            }
            "Some" => {
                arity(1)?;
                let mut a = self.args(args, scope, site)?;
                let Some(x) = a.pop() else {
                    return err(ElabErrorKind::UnknownFunction("Some".to_owned()), span);
                };
                let ty = Type::Option(Box::new(x.ty.clone()));
                self.node(ExprKind::OptionSome(Box::new(x)), ty, span)
            }
            "min" | "max" => {
                arity(2)?;
                let a = self.args(args, scope, site)?;
                for x in &a {
                    self.integer(x)?;
                }
                let b = if f.name == "min" {
                    Builtin::Min
                } else {
                    Builtin::Max
                };
                self.node(ExprKind::Builtin(b, a), Type::Int, span)
            }
            name => {
                self.look(name, f.span)?;
                if !matches!(self.globals.get(name), Some(Global::Def)) {
                    return match self.globals.get(name) {
                        Some(_) => err(ElabErrorKind::UnknownFunction(name.to_owned()), f.span),
                        None => err(ElabErrorKind::UnknownName(name.to_owned()), f.span),
                    };
                }
                if self.current.as_ref().is_some_and(|c| c.name == name)
                    && let Some(current) = self.current.clone()
                {
                    arity(current.params.len())?;
                    return self.recur(current, args, scope, site, span);
                }
                let def = self.def(name, f.span)?;
                arity(def.params.len())?;
                let a = self.args(args, scope, site)?;
                if let Some(rec) = def.rec {
                    return self.unfold(name, &def, rec, a, span);
                }
                let mut subst: BTreeMap<u32, Expr> = BTreeMap::new();
                let mut measures: BTreeMap<u32, (usize, usize)> = BTreeMap::new();
                for ((id, ty), x) in def.params.iter().zip(a) {
                    self.unify(&x.ty, ty, x.span)?;
                    let (size, depth) = measure(&x);
                    self.burn(size as u64, x.span)?;
                    measures.insert(*id, (size, depth));
                    subst.insert(*id, x);
                }
                // Measuring and substituting each visit the body once, with one
                // parameter-table lookup per node.
                let body_size = measure(&def.body).0 as u64;
                let per_node = lookup_cost(measures.len(), 4);
                self.burn(body_size.saturating_mul(per_node).saturating_mul(2), span)?;
                // The exact size of the substituted body, before building it: a body
                // that mentions a parameter k times copies its argument k times.
                let (size, depth) = measure_with(&def.body, &measures);
                self.precharge_copy(size, depth, span)?;
                Ok(substitute(&def.body, &subst))
            }
        }
    }

    fn args(&mut self, args: &[syn::Expr], scope: &mut Scope, site: Site) -> R<Vec<Expr>> {
        args.iter().map(|a| self.expr(a, scope, site)).collect()
    }

    fn method(&mut self, recv: Expr, m: &syn::Ident, mut args: Vec<Expr>, span: Span) -> R<Expr> {
        let recv_ty = self.head_type(&recv.ty, recv.span)?;
        let arity = |expected: usize, found: usize| -> R<()> {
            if found == expected {
                Ok(())
            } else {
                err(
                    ElabErrorKind::Arity {
                        name: m.name.clone(),
                        expected,
                        found,
                    },
                    span,
                )
            }
        };
        let (builtin, ty) = match (&recv_ty, m.name.as_str()) {
            (Type::Map(k, v), "get") => {
                arity(1, args.len())?;
                if let Some(a) = args.first() {
                    self.unify(&a.ty, k, a.span)?;
                }
                (Builtin::MapGet, Type::Option(v.clone()))
            }
            (Type::Map(k, v), "put") => {
                arity(2, args.len())?;
                if let [a, b] = args.as_slice() {
                    self.unify(&a.ty, k, a.span)?;
                    self.unify(&b.ty, v, b.span)?;
                }
                (Builtin::MapPut, recv_ty.clone())
            }
            (Type::Seq(t), "head") => {
                arity(0, args.len())?;
                (Builtin::SeqHead, (**t).clone())
            }
            (Type::Seq(_), "tail") => {
                arity(0, args.len())?;
                (Builtin::SeqTail, recv_ty.clone())
            }
            (Type::Seq(_), "len") => {
                arity(0, args.len())?;
                (Builtin::SeqLen, Type::Nat)
            }
            (Type::Seq(t), "append" | "prepend") => {
                arity(1, args.len())?;
                if let Some(a) = args.first() {
                    self.unify(&a.ty, t, a.span)?;
                }
                let b = if m.name == "append" {
                    Builtin::SeqAppend
                } else {
                    Builtin::SeqPrepend
                };
                (b, recv_ty.clone())
            }
            (Type::Var(_), _) => {
                return err(
                    ElabErrorKind::CannotInferType(format!("the receiver of `.{}`", m.name)),
                    recv.span,
                );
            }
            _ => {
                return err(
                    ElabErrorKind::UnknownFunction(format!(
                        "{}.{}",
                        self.unifier.describe(&recv.ty),
                        m.name
                    )),
                    m.span,
                );
            }
        };
        let mut all = vec![recv];
        all.append(&mut args);
        self.node(ExprKind::Builtin(builtin, all), ty, span)
    }

    /// The value type of `m[k]`, after checking `k` against the key type.
    fn indexed(&mut self, m: &Expr, k: &Expr) -> R<Type> {
        match self.head_type(&m.ty, m.span)? {
            Type::Map(kt, vt) | Type::Function(kt, vt) => {
                self.unify(&k.ty, &kt, k.span)?;
                Ok(*vt)
            }
            Type::Seq(t) => {
                self.integer(k)?;
                Ok(*t)
            }
            Type::Var(_) => err(
                ElabErrorKind::CannotInferType("the indexed expression".to_owned()),
                m.span,
            ),
            _ => err(
                ElabErrorKind::TypeMismatch {
                    expected: "a map, sequence, or function".to_owned(),
                    found: self.unifier.describe(&m.ty),
                },
                m.span,
            ),
        }
    }

    #[inline(never)]
    fn binary(
        &mut self,
        op: syn::BinOp,
        l: &syn::Expr,
        r: &syn::Expr,
        scope: &mut Scope,
        site: Site,
        span: Span,
    ) -> R<Expr> {
        if op == syn::BinOp::LeadsTo && site != Site::Behavior {
            return err(ElabErrorKind::TemporalOutsideBehavior, span);
        }
        let l = self.expr(l, scope, site)?;
        let r = self.expr(r, scope, site)?;
        self.binary_typed(op, l, r, span)
    }

    /// Type one binary node whose operands are elaborated. Kept out of [`Self::binary`]
    /// so the recursive frame stays small.
    #[inline(never)]
    fn binary_typed(&mut self, op: syn::BinOp, l: Expr, r: Expr, span: Span) -> R<Expr> {
        let (nop, ty) = match op {
            syn::BinOp::LeadsTo
            | syn::BinOp::Iff
            | syn::BinOp::Implies
            | syn::BinOp::Or
            | syn::BinOp::And => {
                self.expect(&l, &Type::Bool)?;
                self.expect(&r, &Type::Bool)?;
                let nop = match op {
                    syn::BinOp::LeadsTo => BinOp::LeadsTo,
                    syn::BinOp::Iff => BinOp::Iff,
                    syn::BinOp::Implies => BinOp::Implies,
                    syn::BinOp::Or => BinOp::Or,
                    _ => BinOp::And,
                };
                (nop, Type::Bool)
            }
            syn::BinOp::Eq | syn::BinOp::Ne => {
                self.unify(&r.ty, &l.ty, r.span)?;
                let nop = if op == syn::BinOp::Eq {
                    BinOp::Eq
                } else {
                    BinOp::Ne
                };
                (nop, Type::Bool)
            }
            syn::BinOp::Lt | syn::BinOp::Le | syn::BinOp::Gt | syn::BinOp::Ge => {
                self.integer(&l)?;
                self.integer(&r)?;
                let nop = match op {
                    syn::BinOp::Lt => BinOp::Lt,
                    syn::BinOp::Le => BinOp::Le,
                    syn::BinOp::Gt => BinOp::Gt,
                    _ => BinOp::Ge,
                };
                (nop, Type::Bool)
            }
            syn::BinOp::In | syn::BinOp::NotIn => {
                let elem = self.element_of(&r)?;
                self.unify(&l.ty, &elem, l.span)?;
                let nop = if op == syn::BinOp::In {
                    BinOp::In
                } else {
                    BinOp::NotIn
                };
                (nop, Type::Bool)
            }
            syn::BinOp::SubsetEq => {
                self.element_of(&l)?;
                self.unify(&r.ty, &l.ty, r.span)?;
                (BinOp::SubsetEq, Type::Bool)
            }
            syn::BinOp::Range => {
                self.integer(&l)?;
                self.integer(&r)?;
                (BinOp::Range, Type::Set(Box::new(Type::Int)))
            }
            syn::BinOp::Union | syn::BinOp::Intersect | syn::BinOp::Diff => {
                self.element_of(&l)?;
                self.unify(&r.ty, &l.ty, r.span)?;
                let nop = match op {
                    syn::BinOp::Union => BinOp::Union,
                    syn::BinOp::Intersect => BinOp::Intersect,
                    _ => BinOp::Diff,
                };
                (nop, l.ty.clone())
            }
            syn::BinOp::Add
            | syn::BinOp::Sub
            | syn::BinOp::Mul
            | syn::BinOp::Div
            | syn::BinOp::Mod => {
                self.integer(&l)?;
                self.integer(&r)?;
                let nop = match op {
                    syn::BinOp::Add => BinOp::Add,
                    syn::BinOp::Sub => BinOp::Sub,
                    syn::BinOp::Mul => BinOp::Mul,
                    syn::BinOp::Div => BinOp::Div,
                    _ => BinOp::Mod,
                };
                (nop, Type::Int)
            }
        };
        self.node(ExprKind::Binary(nop, Box::new(l), Box::new(r)), ty, span)
    }

    // -----------------------------------------------------------------------
    // finishing a declaration: resolve types, decide `{}`, canonicalize sets
    // -----------------------------------------------------------------------

    #[inline(never)]
    fn finish_all(&mut self, es: &mut [Expr]) -> R<()> {
        for e in es {
            self.finish(e)?;
        }
        Ok(())
    }

    /// Resolve every type in a finished declaration, decide `{}`, and canonicalize set
    /// and map literals. Recursive over the tree; the per-node work lives in
    /// `#[inline(never)]` helpers so the recursive frame stays small.
    fn finish(&mut self, e: &mut Expr) -> R<()> {
        self.burn(1, e.span)?;
        self.finish_type(e)?;
        let depth = self.finish_scope.len();
        let span = e.span;
        let (binders, kids) = kids_mut(&mut e.kind);
        if let Some(bs) = binders {
            self.finish_binders(bs, span)?;
        }
        // One recursive call site keeps this frame small.
        for kid in kids {
            self.finish(kid)?;
        }
        self.finish_scope.truncate(depth);
        self.finish_literal(e)
    }

    /// Canonicalize a set or map literal once its elements are finished.
    #[inline(never)]
    fn finish_literal(&mut self, e: &mut Expr) -> R<()> {
        match &mut e.kind {
            ExprKind::MapLit(entries) => self.finish_map(entries, e.span),
            ExprKind::SetLit(_) => self.finish_set(e),
            _ => Ok(()),
        }
    }

    #[inline(never)]
    fn finish_type(&mut self, e: &mut Expr) -> R<()> {
        let ty = self.resolve_charged(&e.ty.clone(), e.span)?;
        if has_var(&ty) {
            let what = match &e.kind {
                ExprKind::SetLit(items) if items.is_empty() => "`{}`".to_owned(),
                ExprKind::OptionNone => "`None`".to_owned(),
                ExprKind::SeqLit(items) if items.is_empty() => "`[]`".to_owned(),
                _ => "this expression".to_owned(),
            };
            return err(ElabErrorKind::CannotInferType(what), e.span);
        }
        e.ty = ty;
        Ok(())
    }

    #[inline(never)]
    fn finish_map(&mut self, entries: &mut Vec<(Expr, Expr)>, at: Span) -> R<()> {
        let flat: Vec<&Expr> = entries.iter().flat_map(|(k, v)| [k, v]).collect();
        let keys = self.sort_keys(&flat, at)?;
        let mut keyed: Vec<((String, String), (Expr, Expr))> = Vec::new();
        let mut keys = keys.into_iter();
        for entry in entries.drain(..) {
            let k = keys.next().unwrap_or_default();
            let v = keys.next().unwrap_or_default();
            keyed.push(((k, v), entry));
        }
        keyed.sort_by(|a, b| a.0.cmp(&b.0));
        entries.extend(keyed.into_iter().map(|(_, e)| e));
        Ok(())
    }

    /// `{}` is a set or a map, decided by its now-known type; a set literal is sorted
    /// and deduplicated.
    #[inline(never)]
    fn finish_set(&mut self, e: &mut Expr) -> R<()> {
        let span = e.span;
        if let ExprKind::SetLit(items) = &mut e.kind {
            match &e.ty {
                Type::Map(..) if items.is_empty() => e.kind = ExprKind::MapLit(Vec::new()),
                Type::Set(_) => {
                    // Keyed in the enclosing binder scope: a variable bound outside the
                    // literal is written as its de Bruijn depth, so the order depends
                    // neither on binder names nor on binder numbering, and two
                    // different bound variables never share a key.
                    let keys = self.sort_keys(&items.iter().collect::<Vec<_>>(), span)?;
                    let mut keyed: Vec<(String, Expr)> =
                        keys.into_iter().zip(items.drain(..)).collect();
                    keyed.sort_by(|a, b| a.0.cmp(&b.0));
                    keyed.dedup_by(|a, b| a.0 == b.0);
                    items.extend(keyed.into_iter().map(|(_, x)| x));
                }
                other => {
                    return err(
                        ElabErrorKind::TypeMismatch {
                            expected: "a set or a map".to_owned(),
                            found: other.to_string(),
                        },
                        e.span,
                    );
                }
            }
        }
        Ok(())
    }

    /// Resolve binder types and finish domains. Each binder enters the finishing scope
    /// after its own domain, as it does in the canonical encoding; the caller truncates.
    #[inline(never)]
    fn finish_binders(&mut self, bs: &mut [(u32, Binder)], at: Span) -> R<()> {
        for (id, b) in bs.iter_mut() {
            let ty = self.resolve_charged(&b.ty.clone(), at)?;
            if has_var(&ty) {
                return err(
                    ElabErrorKind::CannotInferType(format!("binder `{}`", b.name)),
                    at,
                );
            }
            b.ty = ty;
            if let Some(d) = &mut b.domain {
                self.finish(d)?;
            }
            let cost = lookup_cost(self.finish_scope.len(), 4);
            self.burn(cost, at)?;
            self.finish_scope.enter(*id);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// tree helpers
// ---------------------------------------------------------------------------

/// Split a top-level `&&` chain into conjuncts, in source order.
fn conjuncts(e: Expr, out: &mut Vec<Expr>) {
    match e.kind {
        ExprKind::Binary(BinOp::And, l, r) => {
            conjuncts(*l, out);
            conjuncts(*r, out);
        }
        kind => out.push(Expr { kind, ..e }),
    }
}

/// The same split on the syntax tree, so each conjunct can be read as an update.
fn syntax_conjuncts<'e>(e: &'e syn::Expr, out: &mut Vec<&'e syn::Expr>) {
    match &e.kind {
        syn::ExprKind::Binary(syn::BinOp::And, l, r) => {
            syntax_conjuncts(l, out);
            syntax_conjuncts(r, out);
        }
        _ => out.push(e),
    }
}

/// `x' == e` (or `x' = e`) where `x` is a bare name and `e` mentions no prime.
fn primed_update(e: &syn::Expr) -> Option<(&syn::Ident, &syn::Expr)> {
    let syn::ExprKind::Binary(syn::BinOp::Eq, l, r) = &e.kind else {
        return None;
    };
    let syn::ExprKind::Prime(inner) = &l.kind else {
        return None;
    };
    let syn::ExprKind::Name(x) = &inner.kind else {
        return None;
    };
    if mentions_prime(r) {
        return None;
    }
    Some((x, r))
}

fn mentions_prime(e: &syn::Expr) -> bool {
    use syn::ExprKind as K;
    match &e.kind {
        K::Prime(_) => true,
        K::Int(_) | K::Bool(_) | K::Str(_) | K::Name(_) | K::WholeState | K::EmptyBraces => false,
        K::Unary(_, a) | K::Temporal(_, a) | K::Field(a, _) => mentions_prime(a),
        K::Binary(_, a, b) | K::Index(a, b) => mentions_prime(a) || mentions_prime(b),
        K::If(a, b, c) | K::Update(a, b, c) => {
            mentions_prime(a) || mentions_prime(b) || mentions_prime(c)
        }
        K::Quant(_, bs, body) => {
            bs.iter()
                .any(|b| b.domain.as_ref().is_some_and(mentions_prime))
                || mentions_prime(body)
        }
        K::Call(_, args) | K::Tuple(args) | K::SetLit(args) | K::SeqLit(args) => {
            args.iter().any(mentions_prime)
        }
        K::Method(r, _, args) => mentions_prime(r) || args.iter().any(mentions_prime),
        K::MapLit(entries) => entries
            .iter()
            .any(|(k, v)| mentions_prime(k) || mentions_prime(v)),
        K::SetComp(x, bs, f) => {
            mentions_prime(x)
                || bs
                    .iter()
                    .any(|b| b.domain.as_ref().is_some_and(mentions_prime))
                || f.as_deref().is_some_and(mentions_prime)
        }
        K::MapComp(k, v, bs, f) => {
            mentions_prime(k)
                || mentions_prime(v)
                || bs
                    .iter()
                    .any(|b| b.domain.as_ref().is_some_and(mentions_prime))
                || f.as_deref().is_some_and(mentions_prime)
        }
        K::Record(fields) => fields.iter().any(|(_, v)| mentions_prime(v)),
    }
}

/// Node count and depth of a tree, computed without recursion.
pub(crate) fn measure(e: &Expr) -> (usize, usize) {
    measure_with(e, &BTreeMap::new())
}

/// The size and depth a copy of `e` allocates, with each bound variable in `subst`
/// replaced by a value of the given `(size, depth)` — the exact measure of
/// [`substitute`]'s result, computed without building it. Size counts expression nodes
/// and the structural size of every type they carry (node types and binder types).
/// Iterative; sizes saturate.
pub(crate) fn measure_with(e: &Expr, subst: &BTreeMap<u32, (usize, usize)>) -> (usize, usize) {
    let mut size = 0_usize;
    let mut deepest = 0_usize;
    let mut stack: Vec<(&Expr, usize)> = vec![(e, 1)];
    while let Some((node, depth)) = stack.pop() {
        if let ExprKind::Bound { binder, .. } = &node.kind
            && let Some((s, d)) = subst.get(binder)
        {
            size = size.saturating_add(*s);
            deepest = deepest.max(depth.saturating_sub(1).saturating_add(*d));
            continue;
        }
        size = size
            .saturating_add(own_cost(&node.kind))
            .saturating_add(type_measure(&node.ty).0);
        deepest = deepest.max(depth);
        let below = depth.saturating_add(1);
        for child in children(node) {
            stack.push((child, below));
        }
    }
    (size, deepest)
}

/// What one node costs apart from its type and its children: one, the text it owns
/// (names, string literals), and its binders' names and types.
pub(crate) fn own_cost(kind: &ExprKind) -> usize {
    let text = |t: &str| crate::budget::text_cost(t.len());
    let binders = |bs: &[(u32, Binder)]| {
        bs.iter().fold(0_usize, |acc, (_, b)| {
            acc.saturating_add(text(&b.name))
                .saturating_add(type_measure(&b.ty).0)
        })
    };
    let owned = match kind {
        ExprKind::Str(t)
        | ExprKind::State(t)
        | ExprKind::Const(t)
        | ExprKind::Param(t)
        | ExprKind::Field(_, t)
        | ExprKind::Step(t)
        | ExprKind::InitRef(t)
        | ExprKind::Bound { name: t, .. } => text(t),
        ExprKind::Variant {
            enumeration,
            variant,
        } => text(enumeration).saturating_add(text(variant)),
        ExprKind::Record(fields) => fields
            .iter()
            .fold(0_usize, |acc, (n, _)| acc.saturating_add(text(n))),
        ExprKind::Quant(_, bs, _)
        | ExprKind::SetComp(_, bs, _)
        | ExprKind::MapComp(_, _, bs, _) => binders(bs),
        ExprKind::Recur { function, .. } => text(function),
        _ => 0,
    };
    owned.saturating_add(1)
}

/// The direct subexpressions of a node, binder domains included.
pub(crate) fn children(e: &Expr) -> Vec<&Expr> {
    let mut out: Vec<&Expr> = Vec::new();
    match &e.kind {
        ExprKind::Bool(_)
        | ExprKind::Int(_)
        | ExprKind::Str(_)
        | ExprKind::State(_)
        | ExprKind::Const(_)
        | ExprKind::Param(_)
        | ExprKind::Bound { .. }
        | ExprKind::Variant { .. }
        | ExprKind::OptionNone
        | ExprKind::Step(_)
        | ExprKind::Stutter
        | ExprKind::InitRef(_) => {}
        ExprKind::OptionSome(a)
        | ExprKind::Not(a)
        | ExprKind::Neg(a)
        | ExprKind::Field(a, _)
        | ExprKind::Temporal(_, a) => out.push(a),
        ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) => {
            out.push(a);
            out.push(b);
        }
        ExprKind::If(a, b, c) | ExprKind::Update(a, b, c) => {
            out.push(a);
            out.push(b);
            out.push(c);
        }
        ExprKind::Quant(_, bs, body) => {
            out.extend(bs.iter().filter_map(|(_, b)| b.domain.as_ref()));
            out.push(body);
        }
        ExprKind::Tuple(items)
        | ExprKind::SeqLit(items)
        | ExprKind::SetLit(items)
        | ExprKind::Builtin(_, items)
        | ExprKind::Recur { args: items, .. } => out.extend(items.iter()),
        ExprKind::MapLit(entries) => {
            for (k, v) in entries {
                out.push(k);
                out.push(v);
            }
        }
        ExprKind::SetComp(x, bs, f) => {
            out.extend(bs.iter().filter_map(|(_, b)| b.domain.as_ref()));
            out.push(x);
            if let Some(f) = f {
                out.push(f);
            }
        }
        ExprKind::MapComp(k, v, bs, f) => {
            out.extend(bs.iter().filter_map(|(_, b)| b.domain.as_ref()));
            out.push(k);
            out.push(v);
            if let Some(f) = f {
                out.push(f);
            }
        }
        ExprKind::Record(fields) => out.extend(fields.iter().map(|(_, v)| v)),
    }
    out
}

/// Replace the bound variables named in `subst` by their values.
///
/// Capture cannot occur: every binder in the model has a unique number, and the
/// arguments of a call never mention a binder of the def body they are placed into.
fn substitute(e: &Expr, subst: &BTreeMap<u32, Expr>) -> Expr {
    let s = |x: &Expr| substitute(x, subst);
    let sb = |x: &Expr| Box::new(substitute(x, subst));
    let binders = |bs: &[(u32, Binder)]| -> Vec<(u32, Binder)> {
        bs.iter()
            .map(|(id, b)| {
                (
                    *id,
                    Binder {
                        name: b.name.clone(),
                        ty: b.ty.clone(),
                        domain: b.domain.as_ref().map(|d| substitute(d, subst)),
                    },
                )
            })
            .collect()
    };
    let kind = match &e.kind {
        ExprKind::Bound { binder, .. } => {
            if let Some(v) = subst.get(binder) {
                return v.clone();
            }
            e.kind.clone()
        }
        ExprKind::Bool(_)
        | ExprKind::Int(_)
        | ExprKind::Str(_)
        | ExprKind::State(_)
        | ExprKind::Const(_)
        | ExprKind::Param(_)
        | ExprKind::Variant { .. }
        | ExprKind::OptionNone
        | ExprKind::Step(_)
        | ExprKind::Stutter
        | ExprKind::InitRef(_) => e.kind.clone(),
        ExprKind::OptionSome(a) => ExprKind::OptionSome(sb(a)),
        ExprKind::Not(a) => ExprKind::Not(sb(a)),
        ExprKind::Neg(a) => ExprKind::Neg(sb(a)),
        ExprKind::Field(a, f) => ExprKind::Field(sb(a), f.clone()),
        ExprKind::Temporal(op, a) => ExprKind::Temporal(*op, sb(a)),
        ExprKind::Binary(op, a, b) => ExprKind::Binary(*op, sb(a), sb(b)),
        ExprKind::Index(a, b) => ExprKind::Index(sb(a), sb(b)),
        ExprKind::If(a, b, c) => ExprKind::If(sb(a), sb(b), sb(c)),
        ExprKind::Update(a, b, c) => ExprKind::Update(sb(a), sb(b), sb(c)),
        ExprKind::Quant(q, bs, body) => ExprKind::Quant(*q, binders(bs), sb(body)),
        ExprKind::Tuple(items) => ExprKind::Tuple(items.iter().map(s).collect()),
        ExprKind::SeqLit(items) => ExprKind::SeqLit(items.iter().map(s).collect()),
        ExprKind::SetLit(items) => ExprKind::SetLit(items.iter().map(s).collect()),
        ExprKind::Builtin(b, items) => ExprKind::Builtin(*b, items.iter().map(s).collect()),
        ExprKind::Recur { function, args } => ExprKind::Recur {
            function: function.clone(),
            args: args.iter().map(s).collect(),
        },
        ExprKind::MapLit(entries) => {
            ExprKind::MapLit(entries.iter().map(|(k, v)| (s(k), s(v))).collect())
        }
        ExprKind::SetComp(x, bs, f) => ExprKind::SetComp(sb(x), binders(bs), f.as_deref().map(sb)),
        ExprKind::MapComp(k, v, bs, f) => {
            ExprKind::MapComp(sb(k), sb(v), binders(bs), f.as_deref().map(sb))
        }
        ExprKind::Record(fields) => {
            ExprKind::Record(fields.iter().map(|(n, v)| (n.clone(), s(v))).collect())
        }
    };
    Expr {
        kind,
        ty: e.ty.clone(),
        span: e.span,
    }
}

// ---------------------------------------------------------------------------
// recursive defs: the termination check and the unfolding
// ---------------------------------------------------------------------------

/// An integer literal node, as the unfolding writes a measure value.
fn literal(v: i64, span: Span) -> Expr {
    Expr {
        kind: ExprKind::Int(v),
        ty: Type::Int,
        span,
    }
}

/// The value of a constant integer expression: literals, `-`, `+`, `-`, `*`, `min`,
/// and `max`, with checked arithmetic (an overflow is not a constant). `visits` counts
/// the nodes looked at, for the caller to charge. Recursion is bounded by the depth of
/// the elaborated tree.
fn const_int(e: &Expr, visits: &mut u64) -> Option<i64> {
    *visits = visits.saturating_add(1);
    match &e.kind {
        ExprKind::Int(n) => Some(*n),
        ExprKind::Neg(a) => const_int(a, visits)?.checked_neg(),
        ExprKind::Binary(op @ (BinOp::Add | BinOp::Sub | BinOp::Mul), a, b) => {
            let a = const_int(a, visits)?;
            let b = const_int(b, visits)?;
            match op {
                BinOp::Add => a.checked_add(b),
                BinOp::Sub => a.checked_sub(b),
                _ => a.checked_mul(b),
            }
        }
        ExprKind::Builtin(b @ (Builtin::Min | Builtin::Max), args) => {
            let [a, c] = args.as_slice() else {
                return None;
            };
            let a = const_int(a, visits)?;
            let c = const_int(c, visits)?;
            Some(if *b == Builtin::Min {
                a.min(c)
            } else {
                a.max(c)
            })
        }
        _ => None,
    }
}

fn is_bound(e: &Expr, binder: u32) -> bool {
    matches!(&e.kind, ExprKind::Bound { binder: b, .. } if *b == binder)
}

/// `!op`: the comparison that holds exactly when `op` does not.
fn negate(op: BinOp) -> BinOp {
    match op {
        BinOp::Eq => BinOp::Ne,
        BinOp::Ne => BinOp::Eq,
        BinOp::Lt => BinOp::Ge,
        BinOp::Ge => BinOp::Lt,
        BinOp::Le => BinOp::Gt,
        BinOp::Gt => BinOp::Le,
        other => other,
    }
}

/// The comparison with its operands swapped: `n op k` is `k (flip op) n`.
fn flip(op: BinOp) -> BinOp {
    match op {
        BinOp::Lt => BinOp::Gt,
        BinOp::Gt => BinOp::Lt,
        BinOp::Le => BinOp::Ge,
        BinOp::Ge => BinOp::Le,
        other => other,
    }
}

/// A *measure test* on the parameter `binder`: `k op n` or `n op k` for a comparison
/// `op` and a constant `n`, under any number of `!`. Returned as `k op n`.
fn measure_test(c: &Expr, binder: u32, visits: &mut u64) -> Option<(BinOp, i64)> {
    let mut e = c;
    let mut negated = false;
    while let ExprKind::Not(inner) = &e.kind {
        *visits = visits.saturating_add(1);
        negated = !negated;
        e = inner;
    }
    *visits = visits.saturating_add(1);
    let ExprKind::Binary(op, l, r) = &e.kind else {
        return None;
    };
    if !matches!(
        op,
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge
    ) {
        return None;
    }
    let (op, n) = if is_bound(l, binder) {
        (*op, const_int(r, visits)?)
    } else if is_bound(r, binder) {
        (flip(*op), const_int(l, visits)?)
    } else {
        return None;
    };
    Some((if negated { negate(op) } else { op }, n))
}

/// Whether `v op n`.
fn holds(op: BinOp, n: i64, v: i64) -> bool {
    match op {
        BinOp::Eq => v == n,
        BinOp::Ne => v != n,
        BinOp::Lt => v < n,
        BinOp::Le => v <= n,
        BinOp::Gt => v > n,
        _ => v >= n,
    }
}

/// `k - c` for the parameter `binder` and a constant `c >= 1`: the decrement `c`.
fn decrement(arg: &Expr, binder: u32, visits: &mut u64) -> Option<i64> {
    *visits = visits.saturating_add(1);
    let ExprKind::Binary(BinOp::Sub, l, r) = &arg.kind else {
        return None;
    };
    if !is_bound(l, binder) {
        return None;
    }
    const_int(r, visits).filter(|c| *c >= 1)
}

/// An interval of integers, unbounded where `None`.
type Interval = (Option<i128>, Option<i128>);

/// `iv` narrowed by the fact `k op n`.
fn refine(iv: Interval, op: BinOp, n: i64) -> Interval {
    let n = i128::from(n);
    let (lo, hi) = iv;
    let raise = |b: i128| Some(lo.map_or(b, |lo| lo.max(b)));
    let lower = |b: i128| Some(hi.map_or(b, |hi| hi.min(b)));
    match op {
        BinOp::Eq => (raise(n), lower(n)),
        // `k != n` narrows only at an end of the interval.
        BinOp::Ne => (
            if lo == Some(n) { Some(n + 1) } else { lo },
            if hi == Some(n) { Some(n - 1) } else { hi },
        ),
        BinOp::Lt => (lo, lower(n - 1)),
        BinOp::Le => (lo, lower(n)),
        BinOp::Gt => (raise(n + 1), hi),
        _ => (raise(n), hi),
    }
}

/// The first recursive call in a tree, in visiting order, without recursion.
fn first_recur(e: &Expr) -> Option<Span> {
    let mut stack = vec![e];
    while let Some(node) = stack.pop() {
        if matches!(node.kind, ExprKind::Recur { .. }) {
            return Some(node.span);
        }
        let mut kids = children(node);
        kids.reverse();
        stack.extend(kids);
    }
    None
}

/// The termination rule (see "Recursive defs" in the module documentation) for the
/// parameter at `index`, bound as `binder`: every recursive call passes `k - c` there,
/// with a constant `c >= 1`, at a position where the measure tests on the path to it
/// bound `k` below by `c`; no recursive call sits inside another's arguments. Returns
/// the least `c`. Iterative; each node is visited once.
fn decreases(body: &Expr, index: usize, binder: u32, nat: bool) -> Option<i64> {
    let start: Interval = (if nat { Some(0) } else { None }, None);
    let mut step: Option<i64> = None;
    let mut visits = 0_u64;
    let mut stack: Vec<(&Expr, Interval, bool)> = vec![(body, start, false)];
    while let Some((e, iv, in_args)) = stack.pop() {
        // A branch no value of `k` reaches is never unfolded.
        if let (Some(lo), Some(hi)) = iv
            && lo > hi
        {
            continue;
        }
        match &e.kind {
            ExprKind::If(c, a, b) => {
                if let Some((op, n)) = measure_test(c, binder, &mut visits) {
                    stack.push((a, refine(iv, op, n), in_args));
                    stack.push((b, refine(iv, negate(op), n), in_args));
                    continue;
                }
            }
            ExprKind::Recur { args, .. } => {
                if in_args {
                    return None;
                }
                let c = decrement(args.get(index)?, binder, &mut visits)?;
                if iv.0.is_none_or(|lo| lo < i128::from(c)) {
                    return None;
                }
                step = Some(step.map_or(c, |s| s.min(c)));
                for a in args {
                    stack.push((a, iv, true));
                }
                continue;
            }
            _ => {}
        }
        for child in children(e) {
            stack.push((child, iv, in_args));
        }
    }
    step
}

/// One unfolding plan's frame: the measure value and the `(size, depth)` of the value
/// each parameter is replaced by.
#[derive(Debug)]
struct PlanFrame {
    v: i64,
    params: BTreeMap<u32, (usize, usize)>,
}

/// One unfolding's frame: the measure value, the value each parameter is replaced by,
/// and the fresh number of each of the body's binders in scope.
struct BuildFrame {
    v: i64,
    subst: BTreeMap<u32, Expr>,
    rename: BTreeMap<u32, u32>,
}

/// Builds the unfolding [`Elaborator::plan_tree`] measured and charged. Infallible:
/// everything it allocates was charged, and its recursion depth is the plan's visit
/// nesting, at most [`MAX_UNFOLD_NESTING`].
///
/// Each unfolding gives the body's binders fresh numbers. The arguments of a recursive
/// call may mention the body's binders (`exists m in S: f(m, k - 1)`), so reusing the
/// numbers in the next unfolding would capture them.
struct Unfolder<'d> {
    params: &'d [(u32, Type)],
    body: &'d Expr,
    rec: Recursion,
    next_binder: u32,
}

impl Unfolder<'_> {
    fn build(&mut self, e: &Expr, f: &mut BuildFrame) -> Expr {
        match &e.kind {
            ExprKind::If(c, a, b) => {
                if let Some((op, n)) = measure_test(c, self.rec.binder, &mut 0) {
                    let branch = if holds(op, n, f.v) { a } else { b };
                    return self.build(branch, f);
                }
            }
            ExprKind::Bound { name, binder } => {
                if let Some(x) = f.subst.get(binder) {
                    return x.clone();
                }
                if let Some(fresh) = f.rename.get(binder) {
                    return Expr {
                        kind: ExprKind::Bound {
                            name: name.clone(),
                            binder: *fresh,
                        },
                        ty: e.ty.clone(),
                        span: e.span,
                    };
                }
            }
            ExprKind::Recur { args, .. } => return self.recur(args, e.span, f),
            _ => {}
        }
        self.rebuild(e, f)
    }

    /// The next unfolding: the arguments built in this frame, the body in a new one.
    #[inline(never)]
    fn recur(&mut self, args: &[Expr], span: Span, f: &mut BuildFrame) -> Expr {
        let c = args
            .get(self.rec.index)
            .and_then(|m| decrement(m, self.rec.binder, &mut 0))
            .unwrap_or(1);
        let v = f.v.saturating_sub(c);
        let params = self.params;
        let mut subst: BTreeMap<u32, Expr> = BTreeMap::new();
        for (i, ((id, _), x)) in params.iter().zip(args).enumerate() {
            let value = if i == self.rec.index {
                literal(v, span)
            } else {
                self.build(x, f)
            };
            subst.insert(*id, value);
        }
        let mut next = BuildFrame {
            v,
            subst,
            rename: BTreeMap::new(),
        };
        let body = self.body;
        self.build(body, &mut next)
    }

    /// Fresh numbers for `bs`, each binder's domain built before it enters scope. The
    /// replaced entries are pushed to `saved`, for [`Self::restore`].
    fn binders(
        &mut self,
        bs: &[(u32, Binder)],
        f: &mut BuildFrame,
        saved: &mut Vec<(u32, Option<u32>)>,
    ) -> Vec<(u32, Binder)> {
        let mut out = Vec::new();
        for (id, b) in bs {
            let domain = b.domain.as_ref().map(|d| self.build(d, f));
            let fresh = self.next_binder;
            self.next_binder = self.next_binder.saturating_add(1);
            saved.push((*id, f.rename.insert(*id, fresh)));
            out.push((
                fresh,
                Binder {
                    name: b.name.clone(),
                    ty: b.ty.clone(),
                    domain,
                },
            ));
        }
        out
    }

    fn restore(f: &mut BuildFrame, saved: Vec<(u32, Option<u32>)>) {
        for (id, old) in saved.into_iter().rev() {
            match old {
                Some(o) => {
                    f.rename.insert(id, o);
                }
                None => {
                    f.rename.remove(&id);
                }
            }
        }
    }

    /// Every other node: the same node over built children. The per-form work is in
    /// small `#[inline(never)]` helpers, so each level of the build's recursion costs
    /// small frames (the recursion is up to [`MAX_UNFOLD_NESTING`] deep).
    #[inline(never)]
    fn rebuild(&mut self, e: &Expr, f: &mut BuildFrame) -> Expr {
        let kind = match &e.kind {
            ExprKind::Quant(..) | ExprKind::SetComp(..) | ExprKind::MapComp(..) => {
                self.binding(&e.kind, f)
            }
            _ => self.plain(&e.kind, f),
        };
        Expr {
            kind,
            ty: e.ty.clone(),
            span: e.span,
        }
    }

    /// A node that binds no variable.
    #[inline(never)]
    fn plain(&mut self, kind: &ExprKind, f: &mut BuildFrame) -> ExprKind {
        let mut b = |x: &Expr| Box::new(self.build(x, f));
        match kind {
            ExprKind::OptionSome(a) => ExprKind::OptionSome(b(a)),
            ExprKind::Not(a) => ExprKind::Not(b(a)),
            ExprKind::Neg(a) => ExprKind::Neg(b(a)),
            ExprKind::Field(a, n) => ExprKind::Field(b(a), n.clone()),
            ExprKind::Temporal(op, a) => ExprKind::Temporal(*op, b(a)),
            ExprKind::Binary(op, x, y) => {
                let x = b(x);
                ExprKind::Binary(*op, x, b(y))
            }
            ExprKind::Index(x, y) => {
                let x = b(x);
                ExprKind::Index(x, b(y))
            }
            ExprKind::If(x, y, z) => {
                let x = b(x);
                let y = b(y);
                ExprKind::If(x, y, b(z))
            }
            ExprKind::Update(x, y, z) => {
                let x = b(x);
                let y = b(y);
                ExprKind::Update(x, y, b(z))
            }
            ExprKind::Tuple(items) => ExprKind::Tuple(self.list(items, f)),
            ExprKind::SeqLit(items) => ExprKind::SeqLit(self.list(items, f)),
            ExprKind::SetLit(items) => ExprKind::SetLit(self.list(items, f)),
            ExprKind::Builtin(op, items) => ExprKind::Builtin(*op, self.list(items, f)),
            ExprKind::Recur { function, args } => ExprKind::Recur {
                function: function.clone(),
                args: self.list(args, f),
            },
            ExprKind::MapLit(entries) => {
                let mut out = Vec::new();
                for (k, v) in entries {
                    let k = self.build(k, f);
                    out.push((k, self.build(v, f)));
                }
                ExprKind::MapLit(out)
            }
            ExprKind::Record(fields) => {
                let mut out = Vec::new();
                for (n, v) in fields {
                    out.push((n.clone(), self.build(v, f)));
                }
                ExprKind::Record(out)
            }
            // Leaves, and the binding forms `rebuild` sends to `binding`.
            other => other.clone(),
        }
    }

    /// A quantifier or comprehension: its binders get fresh numbers for the extent of
    /// the node, and the previous numbers are restored after it.
    #[inline(never)]
    fn binding(&mut self, kind: &ExprKind, f: &mut BuildFrame) -> ExprKind {
        let mut saved = Vec::new();
        let kind = match kind {
            ExprKind::Quant(q, bs, body) => {
                let bs = self.binders(bs, f, &mut saved);
                ExprKind::Quant(*q, bs, Box::new(self.build(body, f)))
            }
            ExprKind::SetComp(x, bs, filter) => {
                let bs = self.binders(bs, f, &mut saved);
                let x = self.build(x, f);
                let filter = filter.as_deref().map(|p| Box::new(self.build(p, f)));
                ExprKind::SetComp(Box::new(x), bs, filter)
            }
            ExprKind::MapComp(k, v, bs, filter) => {
                let bs = self.binders(bs, f, &mut saved);
                let k = self.build(k, f);
                let v = self.build(v, f);
                let filter = filter.as_deref().map(|p| Box::new(self.build(p, f)));
                ExprKind::MapComp(Box::new(k), Box::new(v), bs, filter)
            }
            other => other.clone(),
        };
        Self::restore(f, saved);
        kind
    }

    fn list(&mut self, items: &[Expr], f: &mut BuildFrame) -> Vec<Expr> {
        items.iter().map(|x| self.build(x, f)).collect()
    }
}

/// The names a type expression refers to, with their spans. Recursion is bounded by the
/// parser's nesting limit.
fn type_refs(t: &syn::TypeExpr, out: &mut Vec<(String, Span)>) {
    match &t.kind {
        syn::TypeKind::Named(id) => out.push((id.name.clone(), id.span)),
        syn::TypeKind::Applied(id, args) => {
            out.push((id.name.clone(), id.span));
            for a in args {
                type_refs(a, out);
            }
        }
        syn::TypeKind::Tuple(items) => {
            for i in items {
                type_refs(i, out);
            }
        }
        syn::TypeKind::Function(a, b) => {
            type_refs(a, out);
            type_refs(b, out);
        }
        syn::TypeKind::Record(fields) => {
            for (_, ty) in fields {
                type_refs(ty, out);
            }
        }
    }
}

/// The functions an expression calls, with the spans of the callee names. Recursion is
/// bounded by the parser's nesting limit.
fn expr_calls(e: &syn::Expr, out: &mut Vec<(String, Span)>) {
    use syn::ExprKind as K;
    let binders = |bs: &[syn::Binder], out: &mut Vec<(String, Span)>| {
        for b in bs {
            if let Some(d) = &b.domain {
                expr_calls(d, out);
            }
        }
    };
    match &e.kind {
        K::Int(_) | K::Bool(_) | K::Str(_) | K::Name(_) | K::WholeState | K::EmptyBraces => {}
        K::Prime(a) | K::Unary(_, a) | K::Temporal(_, a) | K::Field(a, _) => expr_calls(a, out),
        K::Binary(_, a, b) | K::Index(a, b) => {
            expr_calls(a, out);
            expr_calls(b, out);
        }
        K::If(a, b, c) | K::Update(a, b, c) => {
            expr_calls(a, out);
            expr_calls(b, out);
            expr_calls(c, out);
        }
        K::Quant(_, bs, body) => {
            binders(bs, out);
            expr_calls(body, out);
        }
        K::Call(f, args) => {
            out.push((f.name.clone(), f.span));
            for a in args {
                expr_calls(a, out);
            }
        }
        K::Tuple(args) | K::SetLit(args) | K::SeqLit(args) => {
            for a in args {
                expr_calls(a, out);
            }
        }
        K::Method(r, _, args) => {
            expr_calls(r, out);
            for a in args {
                expr_calls(a, out);
            }
        }
        K::MapLit(entries) => {
            for (k, v) in entries {
                expr_calls(k, out);
                expr_calls(v, out);
            }
        }
        K::SetComp(x, bs, f) => {
            binders(bs, out);
            expr_calls(x, out);
            if let Some(f) = f {
                expr_calls(f, out);
            }
        }
        K::MapComp(k, v, bs, f) => {
            binders(bs, out);
            expr_calls(k, out);
            expr_calls(v, out);
            if let Some(f) = f {
                expr_calls(f, out);
            }
        }
        K::Record(fields) => {
            for (_, v) in fields {
                expr_calls(v, out);
            }
        }
    }
}

/// Order `nodes` so that every node comes after the nodes it depends on, without
/// recursion: an explicit-stack depth-first search, roots in declaration order and
/// edges in source order, so the order is deterministic. Each node is returned with the
/// span of its first reference (or of nothing, for a root: the span of its first edge
/// is irrelevant there). A cycle is `Err` with the span of the edge that closes it.
pub(crate) fn dependency_order(
    nodes: &[(String, Vec<(String, Span)>)],
) -> Result<Vec<(String, Span)>, Span> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Mark {
        New,
        Open,
        Done,
    }
    let index: BTreeMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, (n, _))| (n.as_str(), i))
        .collect();
    let mut mark = vec![Mark::New; nodes.len()];
    let mut order: Vec<(String, Span)> = Vec::new();
    let origin = Span {
        start: 0,
        end: 0,
        line: 1,
        col: 1,
    };
    for root in 0..nodes.len() {
        if mark.get(root) != Some(&Mark::New) {
            continue;
        }
        // (node, next edge to follow, span the node was reached by)
        let mut stack: Vec<(usize, usize, Span)> = vec![(root, 0, origin)];
        if let Some(m) = mark.get_mut(root) {
            *m = Mark::Open;
        }
        while let Some(top) = stack.last_mut() {
            let (node, next, reached) = *top;
            let edges = nodes.get(node).map_or(&[][..], |(_, e)| e.as_slice());
            match edges.get(next) {
                Some((dep, at)) => {
                    top.1 = next.saturating_add(1);
                    let Some(&d) = index.get(dep.as_str()) else {
                        continue;
                    };
                    match mark.get(d).copied() {
                        Some(Mark::Open) => return Err(*at),
                        Some(Mark::New) => {
                            if let Some(m) = mark.get_mut(d) {
                                *m = Mark::Open;
                            }
                            stack.push((d, 0, *at));
                        }
                        _ => {}
                    }
                }
                None => {
                    if let Some(m) = mark.get_mut(node) {
                        *m = Mark::Done;
                    }
                    if let Some((name, _)) = nodes.get(node) {
                        order.push((name.clone(), reached));
                    }
                    stack.pop();
                }
            }
        }
    }
    Ok(order)
}

/// The binders (if any) and the direct subexpressions of a node, mutably. Binder domains
/// are not included: [`Elaborator::finish_binders`] finishes them in binder order.
/// A node's binders (if any) and its direct subexpressions.
type Kids<'a> = (Option<&'a mut Vec<(u32, Binder)>>, Vec<&'a mut Expr>);

fn kids_mut(kind: &mut ExprKind) -> Kids<'_> {
    let mut out: Vec<&mut Expr> = Vec::new();
    let binders = match kind {
        ExprKind::Bool(_)
        | ExprKind::Int(_)
        | ExprKind::Str(_)
        | ExprKind::State(_)
        | ExprKind::Const(_)
        | ExprKind::Param(_)
        | ExprKind::Bound { .. }
        | ExprKind::Variant { .. }
        | ExprKind::OptionNone
        | ExprKind::Step(_)
        | ExprKind::Stutter
        | ExprKind::InitRef(_) => None,
        ExprKind::OptionSome(a)
        | ExprKind::Not(a)
        | ExprKind::Neg(a)
        | ExprKind::Field(a, _)
        | ExprKind::Temporal(_, a) => {
            out.push(a);
            None
        }
        ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) => {
            out.push(a);
            out.push(b);
            None
        }
        ExprKind::If(a, b, c) | ExprKind::Update(a, b, c) => {
            out.push(a);
            out.push(b);
            out.push(c);
            None
        }
        ExprKind::Quant(_, bs, body) => {
            out.push(body);
            Some(bs)
        }
        ExprKind::Tuple(items)
        | ExprKind::SeqLit(items)
        | ExprKind::SetLit(items)
        | ExprKind::Builtin(_, items)
        | ExprKind::Recur { args: items, .. } => {
            out.extend(items.iter_mut());
            None
        }
        ExprKind::MapLit(entries) => {
            for (k, v) in entries.iter_mut() {
                out.push(k);
                out.push(v);
            }
            None
        }
        ExprKind::SetComp(x, bs, f) => {
            out.push(x);
            if let Some(f) = f {
                out.push(f);
            }
            Some(bs)
        }
        ExprKind::MapComp(k, v, bs, f) => {
            out.push(k);
            out.push(v);
            if let Some(f) = f {
                out.push(f);
            }
            Some(bs)
        }
        ExprKind::Record(fields) => {
            out.extend(fields.iter_mut().map(|(_, v)| v));
            None
        }
    };
    (binders, out)
}

/// The work of [`dependency_order`] over `nodes`: one ordered-table lookup per edge,
/// plus building the table.
fn order_cost(nodes: &[(String, Vec<(String, Span)>)]) -> u64 {
    let n = nodes.len();
    let bytes: usize = nodes.iter().map(|(name, _)| name.len()).sum();
    nodes.iter().fold(sort_cost(n, bytes), |acc, (_, edges)| {
        edges.iter().fold(acc, |acc, (dep, _)| {
            acc.saturating_add(lookup_cost(n, dep.len()))
        })
    })
}
