//! The canonical tree dump.
//!
//! Decision: RFC 0003 (the CML surface) and ADR-0025 (the Finite fragment); the dump is
//! the golden form of the PR 15a parse tree.
//!
//! A deterministic S-expression rendering of a tree with spans left out. Two trees
//! dump to the same text exactly when they are equal apart from spans, so the dump is
//! what golden files record and what round-trip tests compare. Names are bare atoms.
//! The only list headed by a name is an applied type such as `(Set Node)`; the other
//! type heads (`tuple-type`, `record-type`, `->`) hold a character no identifier holds,
//! so the forms cannot be confused. Everywhere else a list's position fixes its meaning.
//!
//! Layout: a list that fits in [`WIDTH`] columns is printed on one line; otherwise its
//! head (and the atoms that directly follow the head) stay on the first line and each
//! remaining element goes on its own line, indented two spaces.

use std::fmt::Write as _;

use crate::ast::{
    ActionBody, Binder, Decl, DeclKind, Expr, ExprKind, FairnessStrength, HeaderStyle, Quantifier,
    SourceFile, Stmt, StmtKind, TemporalOp, TypeExpr, TypeKind, UnOp,
};

/// The target line width of the dump.
pub const WIDTH: usize = 88;

enum S {
    Atom(String),
    List(Vec<S>),
}

fn atom(s: &str) -> S {
    S::Atom(s.to_owned())
}

fn list(head: &str, rest: impl IntoIterator<Item = S>) -> S {
    let mut v = vec![atom(head)];
    v.extend(rest);
    S::List(v)
}

impl S {
    fn flat(&self, out: &mut String) {
        match self {
            S::Atom(a) => out.push_str(a),
            S::List(items) => {
                out.push('(');
                for (i, it) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(' ');
                    }
                    it.flat(out);
                }
                out.push(')');
            }
        }
    }

    fn pretty(&self, indent: usize, out: &mut String) {
        let mut flat = String::new();
        self.flat(&mut flat);
        let S::List(items) = self else {
            out.push_str(&flat);
            return;
        };
        if indent + flat.len() <= WIDTH {
            out.push_str(&flat);
            return;
        }
        out.push('(');
        let mut i = 0;
        while i < items.len() {
            match &items[i] {
                S::Atom(a) => {
                    if i > 0 {
                        out.push(' ');
                    }
                    out.push_str(a);
                    i += 1;
                }
                S::List(_) => break,
            }
        }
        for it in &items[i..] {
            out.push('\n');
            out.push_str(&" ".repeat(indent + 2));
            it.pretty(indent + 2, out);
        }
        out.push(')');
    }
}

/// Dump a source file.
#[must_use]
pub fn dump_file(file: &SourceFile) -> String {
    let head = match file.header.style {
        HeaderStyle::Module => "module",
        HeaderStyle::Model => "model",
    };
    let mut items = vec![atom(&file.header.name.name)];
    items.extend(file.decls.iter().map(decl));
    let s = list(head, items);
    let mut out = String::new();
    s.pretty(0, &mut out);
    out.push('\n');
    out
}

/// Dump an expression.
#[must_use]
pub fn dump_expr(e: &Expr) -> String {
    let mut out = String::new();
    expr(e).pretty(0, &mut out);
    out
}

/// Dump a type.
#[must_use]
pub fn dump_type(t: &TypeExpr) -> String {
    let mut out = String::new();
    ty(t).pretty(0, &mut out);
    out
}

fn decl(d: &Decl) -> S {
    match &d.kind {
        DeclKind::Sort { name } => list("sort", [atom(&name.name)]),
        DeclKind::TypeAlias { name, ty: t } => list("type-alias", [atom(&name.name), ty(t)]),
        DeclKind::Enum { name, variants } => list(
            "enum",
            [
                atom(&name.name),
                S::List(variants.iter().map(|v| atom(&v.name)).collect()),
            ],
        ),
        DeclKind::Const { name, ty: t } => list("const", [atom(&name.name), ty(t)]),
        DeclKind::State { fields } => list(
            "state",
            fields.iter().map(|f| {
                let mut v = vec![atom(&f.name.name), ty(&f.ty)];
                if let Some(r) = &f.refinement {
                    v.push(list("where", [expr(r)]));
                }
                list("var", v)
            }),
        ),
        DeclKind::Init { name, body } => {
            let mut v = Vec::new();
            if let Some(n) = name {
                v.push(atom(&n.name));
            }
            v.extend(body.iter().map(stmt));
            list("init", v)
        }
        DeclKind::Action { name, params, body } => {
            let body = match body {
                ActionBody::Block(stmts) => list("block", stmts.iter().map(stmt)),
                ActionBody::Choice(alts) => list("choice", alts.iter().map(|a| atom(&a.name))),
            };
            list("action", [atom(&name.name), params_s(params), body])
        }
        DeclKind::Invariant { name, body } => {
            let mut v = vec![atom(&name.name)];
            v.extend(body.iter().map(stmt));
            list("invariant", v)
        }
        DeclKind::Fairness { strength, actions } => {
            let mut v = vec![atom(match strength {
                FairnessStrength::Weak => "weak",
                FairnessStrength::Strong => "strong",
            })];
            v.extend(actions.iter().map(|a| atom(&a.name)));
            list("fairness", v)
        }
        DeclKind::Behavior { name, expr: e } => list("behavior", [atom(&name.name), expr(e)]),
        DeclKind::Def {
            name,
            params,
            ret,
            body,
        } => list(
            "def",
            [atom(&name.name), params_s(params), ty(ret), expr(body)],
        ),
    }
}

fn params_s(params: &[crate::ast::Param]) -> S {
    list(
        "params",
        params
            .iter()
            .map(|p| S::List(vec![atom(&p.name.name), ty(&p.ty)])),
    )
}

fn stmt(s: &Stmt) -> S {
    match &s.kind {
        StmtKind::Require(e) => list("require", [expr(e)]),
        StmtKind::Let(n, e) => list("let", [atom(&n.name), expr(e)]),
        StmtKind::Next(n, e) => list("next", [atom(&n.name), expr(e)]),
        StmtKind::Unchanged(ns) => list("unchanged", ns.iter().map(|n| atom(&n.name))),
        StmtKind::Expr(e) => list("clause", [expr(e)]),
    }
}

fn ty(t: &TypeExpr) -> S {
    match &t.kind {
        TypeKind::Named(n) => atom(&n.name),
        TypeKind::Applied(n, args) => {
            let mut v = vec![atom(&n.name)];
            v.extend(args.iter().map(ty));
            S::List(v)
        }
        // Type heads contain `-`, which no identifier does, so `(tuple-type A B)` cannot be
        // confused with a sort named `tuple` applied to `A` and `B`.
        TypeKind::Tuple(items) => list("tuple-type", items.iter().map(ty)),
        TypeKind::Function(a, b) => list("->", [ty(a), ty(b)]),
        TypeKind::Record(fields) => list(
            "record-type",
            fields
                .iter()
                .map(|(n, t)| S::List(vec![atom(&n.name), ty(t)])),
        ),
    }
}

fn binders_s(bs: &[Binder]) -> S {
    S::List(
        bs.iter()
            .map(|b| {
                let mut v = vec![atom(&b.name.name)];
                if let Some(d) = &b.domain {
                    v.push(expr(d));
                }
                S::List(v)
            })
            .collect(),
    )
}

fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn expr(e: &Expr) -> S {
    match &e.kind {
        ExprKind::Int(n) => {
            let mut s = String::new();
            let _ = write!(s, "{n}");
            S::Atom(s)
        }
        ExprKind::Bool(b) => atom(if *b { "true" } else { "false" }),
        ExprKind::Str(s) => S::Atom(quote(s)),
        ExprKind::Name(n) => atom(&n.name),
        ExprKind::WholeState => list("whole-state", []),
        ExprKind::Prime(x) => list("prime", [expr(x)]),
        ExprKind::Unary(op, x) => list(
            match op {
                UnOp::Not => "not",
                UnOp::Neg => "neg",
            },
            [expr(x)],
        ),
        ExprKind::Binary(op, a, b) => list(op.name(), [expr(a), expr(b)]),
        ExprKind::Quant(q, bs, body) => list(
            match q {
                Quantifier::Forall => "forall",
                Quantifier::Exists => "exists",
            },
            [binders_s(bs), expr(body)],
        ),
        ExprKind::If(c, a, b) => list("if", [expr(c), expr(a), expr(b)]),
        ExprKind::Temporal(op, x) => list(
            match op {
                TemporalOp::Always => "always",
                TemporalOp::Eventually => "eventually",
            },
            [expr(x)],
        ),
        ExprKind::Call(f, args) => {
            let mut v = vec![atom(&f.name)];
            v.extend(args.iter().map(expr));
            list("call", v)
        }
        ExprKind::Method(r, m, args) => {
            let mut v = vec![expr(r), atom(&m.name)];
            v.extend(args.iter().map(expr));
            list("method", v)
        }
        ExprKind::Field(r, f) => list("field", [expr(r), atom(&f.name)]),
        ExprKind::Index(b, k) => list("index", [expr(b), expr(k)]),
        ExprKind::Update(b, k, v) => list("update", [expr(b), expr(k), expr(v)]),
        ExprKind::Tuple(items) => list("tuple", items.iter().map(expr)),
        ExprKind::EmptyBraces => list("empty-braces", []),
        ExprKind::SetLit(items) => list("set", items.iter().map(expr)),
        ExprKind::MapLit(entries) => list(
            "map",
            entries.iter().map(|(k, v)| S::List(vec![expr(k), expr(v)])),
        ),
        ExprKind::SetComp(el, bs, filter) => {
            let mut v = vec![expr(el), binders_s(bs)];
            if let Some(f) = filter {
                v.push(list("where", [expr(f)]));
            }
            list("set-comp", v)
        }
        ExprKind::MapComp(k, val, bs, filter) => {
            let mut v = vec![expr(k), expr(val), binders_s(bs)];
            if let Some(f) = filter {
                v.push(list("where", [expr(f)]));
            }
            list("map-comp", v)
        }
        ExprKind::Record(fields) => list(
            "record",
            fields
                .iter()
                .map(|(n, e)| S::List(vec![atom(&n.name), expr(e)])),
        ),
        ExprKind::SeqLit(items) => list("seq", items.iter().map(expr)),
    }
}
