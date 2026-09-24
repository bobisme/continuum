//! The exact depth of every tree in a parsed file, measured without recursion.
//!
//! The parser's nesting bound ([`continuum_cml_syntax::MAX_NESTING`]) promises that no
//! tree it returns — expression or type — is deeper than the bound, and every later
//! recursive pass (dump, print, elaboration, `Drop`) relies on it. This walk checks the
//! promise on every accepted source with an explicit stack, so a tree past the bound is a
//! typed finding ([`super::engine::Oracle::TooDeep`]) and not a stack overflow in the
//! measurement.
//!
//! Every declaration and statement kind is matched with all its fields named (no `..`),
//! so a new field that holds a tree is a compile error here until the walk visits it.
//! Declarations and statements themselves are flat lists; the recursive trees are
//! [`Expr`] and [`TypeExpr`].

use continuum_cml_syntax::SourceFile;
use continuum_cml_syntax::ast::{
    ActionBody, Binder, DeclKind, Expr, ExprKind as K, Param, StateField, Stmt, StmtKind, TypeExpr,
    TypeKind,
};

/// The roots of every tree in one file.
#[derive(Default)]
struct Roots<'a> {
    exprs: Vec<&'a Expr>,
    types: Vec<&'a TypeExpr>,
}

impl<'a> Roots<'a> {
    fn stmts(&mut self, body: &'a [Stmt]) {
        for s in body {
            match &s.kind {
                StmtKind::Require(e)
                | StmtKind::Let(_, e)
                | StmtKind::Next(_, e)
                | StmtKind::Expr(e) => self.exprs.push(e),
                StmtKind::Unchanged(_) => {}
            }
        }
    }

    fn params(&mut self, params: &'a [Param]) {
        for Param {
            name: _,
            ty,
            span: _,
        } in params
        {
            self.types.push(ty);
        }
    }
}

/// The depth of the deepest tree, expression or type, in `file`; a leaf is depth 1.
#[must_use]
pub fn file_tree_depth(file: &SourceFile) -> usize {
    let mut r = Roots::default();
    for d in &file.decls {
        match &d.kind {
            DeclKind::Sort { name: _ }
            | DeclKind::Enum {
                name: _,
                variants: _,
            }
            | DeclKind::Fairness {
                strength: _,
                actions: _,
            } => {}
            DeclKind::TypeAlias { name: _, ty } | DeclKind::Const { name: _, ty } => {
                r.types.push(ty);
            }
            DeclKind::State { fields } => {
                for StateField {
                    name: _,
                    ty,
                    refinement,
                    span: _,
                } in fields
                {
                    r.types.push(ty);
                    r.exprs.extend(refinement.iter());
                }
            }
            DeclKind::Init { name: _, body } | DeclKind::Invariant { name: _, body } => {
                r.stmts(body);
            }
            DeclKind::Action {
                name: _,
                params,
                body,
            } => {
                r.params(params);
                match body {
                    ActionBody::Block(body) => r.stmts(body),
                    ActionBody::Choice(_) => {}
                }
            }
            DeclKind::Behavior { name: _, expr } => r.exprs.push(expr),
            DeclKind::Def {
                name: _,
                params,
                ret,
                body,
            } => {
                r.params(params);
                r.types.push(ret);
                r.exprs.push(body);
            }
        }
    }
    let exprs = r.exprs.into_iter().map(expr_depth);
    let types = r.types.into_iter().map(type_depth);
    exprs.chain(types).max().unwrap_or(0)
}

/// The depth of one type tree; a leaf is depth 1.
#[must_use]
pub fn type_depth(root: &TypeExpr) -> usize {
    let mut deepest = 0;
    let mut stack = vec![(root, 1_usize)];
    while let Some((t, d)) = stack.pop() {
        deepest = deepest.max(d);
        match &t.kind {
            TypeKind::Named(_) => {}
            TypeKind::Applied(_, xs) | TypeKind::Tuple(xs) => {
                stack.extend(xs.iter().map(|x| (x, d + 1)));
            }
            TypeKind::Function(a, b) => {
                stack.push((a, d + 1));
                stack.push((b, d + 1));
            }
            TypeKind::Record(fs) => stack.extend(fs.iter().map(|(_, x)| (x, d + 1))),
        }
    }
    deepest
}

fn domains(bs: &[Binder]) -> impl Iterator<Item = &Expr> {
    bs.iter().filter_map(|b| b.domain.as_ref())
}

/// The depth of one expression tree; a leaf is depth 1.
#[must_use]
pub fn expr_depth(root: &Expr) -> usize {
    let mut deepest = 0;
    let mut stack = vec![(root, 1_usize)];
    while let Some((e, d)) = stack.pop() {
        deepest = deepest.max(d);
        let children: Vec<&Expr> = match &e.kind {
            K::Int(_) | K::Bool(_) | K::Str(_) | K::Name(_) | K::WholeState | K::EmptyBraces => {
                vec![]
            }
            K::Prime(a) | K::Unary(_, a) | K::Temporal(_, a) | K::Field(a, _) => vec![a],
            K::Binary(_, a, b) | K::Index(a, b) => vec![a, b],
            K::If(a, b, c) | K::Update(a, b, c) => vec![a, b, c],
            K::Quant(_, bs, body) => domains(bs).chain([&**body]).collect(),
            K::Call(_, xs) | K::Tuple(xs) | K::SetLit(xs) | K::SeqLit(xs) => xs.iter().collect(),
            K::Method(a, _, xs) => std::iter::once(&**a).chain(xs).collect(),
            K::MapLit(kvs) => kvs.iter().flat_map(|(k, v)| [k, v]).collect(),
            K::Record(fs) => fs.iter().map(|(_, v)| v).collect(),
            K::SetComp(a, bs, w) => std::iter::once(&**a)
                .chain(domains(bs))
                .chain(w.as_deref())
                .collect(),
            K::MapComp(a, b, bs, w) => [&**a, &**b]
                .into_iter()
                .chain(domains(bs))
                .chain(w.as_deref())
                .collect(),
        };
        stack.extend(children.into_iter().map(|c| (c, d + 1)));
    }
    deepest
}
