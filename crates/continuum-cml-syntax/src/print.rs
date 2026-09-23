//! The canonical printer.
//!
//! Decision: RFC 0003 (the CML surface), PR 15a; docs/11 §14 keeps the formatter for PR 15b.
//!
//! Renders a tree back to CML source that parses to the same tree (apart from spans).
//! Parentheses appear only where the grammar needs them, so a print never nests deeper
//! than any source with the same tree (and so never exceeds [`crate::MAX_NESTING`] when
//! the tree came from the parser). Every statement is on one line.
//!
//! This is a round-trip witness for the parser, not the CML formatter: the PR 15b
//! formatter is defined over the normalized semantic AST (docs/11 §14), and comments and
//! layout are not preserved here.

use crate::ast::{
    ActionBody, BinOp, Binder, Decl, DeclKind, Expr, ExprKind, FairnessStrength, HeaderStyle,
    Param, Quantifier, SourceFile, Stmt, StmtKind, TemporalOp, TypeExpr, TypeKind, UnOp,
};

/// Print a source file.
#[must_use]
pub fn print_file(file: &SourceFile) -> String {
    let mut out = String::new();
    match file.header.style {
        HeaderStyle::Module => {
            out.push_str("module ");
            out.push_str(&file.header.name.name);
            out.push('\n');
            for d in &file.decls {
                out.push('\n');
                decl(d, 0, &mut out);
            }
        }
        HeaderStyle::Model => {
            out.push_str("model ");
            out.push_str(&file.header.name.name);
            out.push_str(" {\n");
            for d in &file.decls {
                decl(d, 1, &mut out);
            }
            out.push_str("}\n");
        }
    }
    out
}

/// Print an expression on one line.
#[must_use]
pub fn print_expr(e: &Expr) -> String {
    let mut out = String::new();
    expr(e, &mut out);
    out
}

/// Print a type on one line.
#[must_use]
pub fn print_type(t: &TypeExpr) -> String {
    let mut out = String::new();
    ty(t, &mut out);
    out
}

fn pad(level: usize, out: &mut String) {
    for _ in 0..level {
        out.push_str("  ");
    }
}

fn decl(d: &Decl, level: usize, out: &mut String) {
    pad(level, out);
    match &d.kind {
        DeclKind::Sort { name } => {
            out.push_str("type ");
            out.push_str(&name.name);
        }
        DeclKind::TypeAlias { name, ty: t } => {
            out.push_str("type ");
            out.push_str(&name.name);
            out.push_str(" = ");
            ty(t, out);
        }
        DeclKind::Enum { name, variants } => {
            out.push_str("enum ");
            out.push_str(&name.name);
            out.push_str(" { ");
            join(variants.iter().map(|v| v.name.clone()), ", ", out);
            out.push_str(" }");
        }
        DeclKind::Const { name, ty: t } => {
            out.push_str("const ");
            out.push_str(&name.name);
            out.push_str(": ");
            ty(t, out);
        }
        DeclKind::State { fields } => {
            out.push_str("state {\n");
            for f in fields {
                pad(level + 1, out);
                out.push_str(&f.name.name);
                out.push_str(": ");
                ty(&f.ty, out);
                if let Some(r) = &f.refinement {
                    out.push_str(" where ");
                    expr(r, out);
                }
                out.push('\n');
            }
            pad(level, out);
            out.push('}');
        }
        DeclKind::Init { name, body } => {
            out.push_str("init");
            if let Some(n) = name {
                out.push(' ');
                out.push_str(&n.name);
            }
            block(body, level, out);
        }
        DeclKind::Action { name, params, body } => {
            out.push_str("action ");
            out.push_str(&name.name);
            if !params.is_empty() {
                params_p(params, out);
            }
            match body {
                ActionBody::Block(stmts) => block(stmts, level, out),
                ActionBody::Choice(alts) => {
                    out.push_str(" = ");
                    join(alts.iter().map(|a| a.name.clone()), " | ", out);
                }
            }
        }
        DeclKind::Invariant { name, body } => {
            out.push_str("invariant ");
            out.push_str(&name.name);
            block(body, level, out);
        }
        DeclKind::Fairness { strength, actions } => {
            out.push_str(match strength {
                FairnessStrength::Weak => "fairness weak ",
                FairnessStrength::Strong => "fairness strong ",
            });
            join(actions.iter().map(|a| a.name.clone()), ", ", out);
        }
        DeclKind::Behavior { name, expr: e } => {
            out.push_str("behavior ");
            out.push_str(&name.name);
            out.push_str(" = ");
            expr(e, out);
        }
        DeclKind::Def {
            name,
            params,
            ret,
            body,
        } => {
            out.push_str("def ");
            out.push_str(&name.name);
            params_p(params, out);
            out.push_str(": ");
            ty(ret, out);
            out.push_str(" = ");
            expr(body, out);
        }
    }
    out.push('\n');
}

fn join(items: impl Iterator<Item = String>, sep: &str, out: &mut String) {
    for (i, s) in items.enumerate() {
        if i > 0 {
            out.push_str(sep);
        }
        out.push_str(&s);
    }
}

fn params_p(params: &[Param], out: &mut String) {
    out.push('(');
    for (i, p) in params.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str(&p.name.name);
        out.push_str(": ");
        ty(&p.ty, out);
    }
    out.push(')');
}

fn block(stmts: &[Stmt], level: usize, out: &mut String) {
    out.push_str(" {\n");
    for s in stmts {
        pad(level + 1, out);
        stmt(s, out);
        out.push('\n');
    }
    pad(level, out);
    out.push('}');
}

fn stmt(s: &Stmt, out: &mut String) {
    match &s.kind {
        StmtKind::Require(e) => {
            out.push_str("require ");
            expr(e, out);
        }
        StmtKind::Let(n, e) => {
            out.push_str("let ");
            out.push_str(&n.name);
            out.push_str(" = ");
            expr(e, out);
        }
        StmtKind::Next(n, e) => {
            out.push_str("next ");
            out.push_str(&n.name);
            out.push_str(" = ");
            expr(e, out);
        }
        StmtKind::Unchanged(ns) => {
            out.push_str("unchanged ");
            join(ns.iter().map(|n| n.name.clone()), ", ", out);
        }
        StmtKind::Expr(e) => expr(e, out),
    }
}

fn ty(t: &TypeExpr, out: &mut String) {
    match &t.kind {
        TypeKind::Named(n) => out.push_str(&n.name),
        TypeKind::Applied(n, args) => {
            out.push_str(&n.name);
            out.push('[');
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                ty(a, out);
            }
            out.push(']');
        }
        TypeKind::Tuple(items) => {
            out.push('(');
            for (i, a) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                ty(a, out);
            }
            out.push(')');
        }
        TypeKind::Function(a, b) => {
            if matches!(a.kind, TypeKind::Function(..)) {
                out.push('(');
                ty(a, out);
                out.push(')');
            } else {
                ty(a, out);
            }
            out.push_str(" -> ");
            ty(b, out);
        }
        TypeKind::Record(fields) => {
            out.push('{');
            for (i, (n, t)) in fields.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&n.name);
                out.push_str(": ");
                ty(t, out);
            }
            out.push('}');
        }
    }
}

/// The binding level of the tightest context: postfix operands.
const POSTFIX: u8 = 12;
/// The binding level of prefix operators.
const PREFIX: u8 = 11;
/// The loosest context: a whole expression.
const TOP: u8 = 0;

/// How tightly `e` binds: its operator's level, or [`POSTFIX`] for a self-delimiting
/// form.
fn prec(e: &Expr) -> u8 {
    match &e.kind {
        ExprKind::Binary(op, ..) => binop_level(*op).0,
        ExprKind::Unary(..) => PREFIX,
        _ => POSTFIX,
    }
}

/// Whether `e` extends as far right as possible (a quantifier or `if`), so it may be
/// printed bare only where nothing follows it.
fn is_open(e: &Expr) -> bool {
    matches!(e.kind, ExprKind::Quant(..) | ExprKind::If(..))
}

/// The binding level of `op` (the parser's table), and whether it is right-associative
/// or non-associative.
fn binop_level(op: BinOp) -> (u8, bool, bool) {
    match op {
        BinOp::LeadsTo => (1, false, true),
        BinOp::Iff => (2, false, true),
        BinOp::Implies => (3, true, false),
        BinOp::Or => (4, false, false),
        BinOp::And => (5, false, false),
        BinOp::Eq
        | BinOp::Ne
        | BinOp::Lt
        | BinOp::Le
        | BinOp::Gt
        | BinOp::Ge
        | BinOp::In
        | BinOp::NotIn
        | BinOp::SubsetEq => (6, false, true),
        BinOp::Range => (7, false, true),
        BinOp::Union | BinOp::Intersect | BinOp::Diff => (8, false, false),
        BinOp::Add | BinOp::Sub => (9, false, false),
        BinOp::Mul | BinOp::Div | BinOp::Mod => (10, false, false),
    }
}

/// Print `e` where the context binds at `ctx`, with `tail` true when nothing that could
/// continue an expression follows it. Parentheses are added only where the parser needs
/// them, so the print never nests deeper than any source that yields the same tree.
fn sub(e: &Expr, ctx: u8, tail: bool, out: &mut String) {
    if prec(e) < ctx || (is_open(e) && !tail) {
        out.push('(');
        expr_in(e, true, out);
        out.push(')');
    } else {
        expr_in(e, tail, out);
    }
}

fn list(items: &[Expr], out: &mut String) {
    for (i, e) in items.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        sub(e, TOP, true, out);
    }
}

fn binders(bs: &[Binder], out: &mut String) {
    for (i, b) in bs.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str(&b.name.name);
        if let Some(d) = &b.domain {
            out.push_str(" in ");
            // Binder domains are parsed at the range level.
            sub(d, 7, false, out);
        }
    }
}

fn expr(e: &Expr, out: &mut String) {
    sub(e, TOP, true, out);
}

#[allow(clippy::too_many_lines)]
fn expr_in(e: &Expr, tail: bool, out: &mut String) {
    match &e.kind {
        ExprKind::Int(n) => out.push_str(&n.to_string()),
        ExprKind::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        ExprKind::Str(s) => {
            out.push('"');
            for c in s.chars() {
                match c {
                    '"' => out.push_str("\\\""),
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    c => out.push(c),
                }
            }
            out.push('"');
        }
        ExprKind::Name(n) => out.push_str(&n.name),
        ExprKind::WholeState => out.push_str("state"),
        ExprKind::Prime(x) => {
            sub(x, POSTFIX, false, out);
            out.push('\'');
        }
        ExprKind::Unary(op, x) => {
            out.push(match op {
                UnOp::Not => '!',
                UnOp::Neg => '-',
            });
            sub(x, PREFIX, tail, out);
        }
        ExprKind::Binary(op, a, b) => {
            let (level, right, non) = binop_level(*op);
            let (lctx, rctx) = if right {
                (level + 1, level)
            } else {
                (if non { level + 1 } else { level }, level + 1)
            };
            sub(a, lctx, false, out);
            out.push(' ');
            out.push_str(op.symbol());
            out.push(' ');
            sub(b, rctx, tail, out);
        }
        ExprKind::Quant(q, bs, body) => {
            out.push_str(match q {
                Quantifier::Forall => "forall ",
                Quantifier::Exists => "exists ",
            });
            binders(bs, out);
            out.push_str(": ");
            sub(body, TOP, true, out);
        }
        ExprKind::If(c, a, b) => {
            out.push_str("if ");
            sub(c, TOP, true, out);
            out.push_str(" then ");
            sub(a, TOP, true, out);
            out.push_str(" else ");
            sub(b, TOP, true, out);
        }
        ExprKind::Temporal(op, x) => {
            out.push_str(match op {
                TemporalOp::Always => "always(",
                TemporalOp::Eventually => "eventually(",
            });
            sub(x, TOP, true, out);
            out.push(')');
        }
        ExprKind::Call(f, args) => {
            out.push_str(&f.name);
            out.push('(');
            list(args, out);
            out.push(')');
        }
        ExprKind::Method(r, m, args) => {
            sub(r, POSTFIX, false, out);
            out.push('.');
            out.push_str(&m.name);
            out.push('(');
            list(args, out);
            out.push(')');
        }
        ExprKind::Field(r, f) => {
            sub(r, POSTFIX, false, out);
            out.push('.');
            out.push_str(&f.name);
        }
        ExprKind::Index(b, k) => {
            sub(b, POSTFIX, false, out);
            out.push('[');
            sub(k, TOP, true, out);
            out.push(']');
        }
        ExprKind::Update(b, k, v) => {
            sub(b, POSTFIX, false, out);
            out.push('[');
            sub(k, TOP, true, out);
            out.push_str(" := ");
            sub(v, TOP, true, out);
            out.push(']');
        }
        ExprKind::Tuple(items) => {
            out.push('(');
            list(items, out);
            out.push(')');
        }
        ExprKind::EmptyBraces => out.push_str("{}"),
        ExprKind::SetLit(items) => {
            out.push('{');
            list(items, out);
            out.push('}');
        }
        ExprKind::MapLit(entries) => {
            out.push('{');
            for (i, (k, v)) in entries.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                sub(k, TOP, true, out);
                out.push_str(" -> ");
                sub(v, TOP, true, out);
            }
            out.push('}');
        }
        ExprKind::SetComp(el, bs, filter) => {
            out.push('{');
            sub(el, TOP, true, out);
            out.push_str(" | ");
            binders(bs, out);
            if let Some(f) = filter {
                out.push_str(" where ");
                sub(f, TOP, true, out);
            }
            out.push('}');
        }
        ExprKind::MapComp(k, v, bs, filter) => {
            out.push('{');
            sub(k, TOP, true, out);
            out.push_str(" -> ");
            sub(v, TOP, true, out);
            out.push_str(" | ");
            binders(bs, out);
            if let Some(f) = filter {
                out.push_str(" where ");
                sub(f, TOP, true, out);
            }
            out.push('}');
        }
        ExprKind::Record(fields) => {
            out.push('{');
            for (i, (n, v)) in fields.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&n.name);
                out.push_str(": ");
                sub(v, TOP, true, out);
            }
            out.push('}');
        }
        ExprKind::SeqLit(items) => {
            out.push('[');
            list(items, out);
            out.push(']');
        }
    }
}
