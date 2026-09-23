//! Collection-valued state and expressions under the flat layout (RFC 0003 correction
//! 4, "Layout", "Static keys", "Expressions", "Definedness", bn-23hzh).
//!
//! A value of a finite composite type lowers to the list of scalar expressions of its
//! type's layout ([`LayOps::L`]); a scalar value keeps the integer lowering of the
//! parent module. Everything is written once, generic over [`Mode`]: the plan returns
//! an aggregate (the slot count, the largest slot measure, and the hull of the slot
//! values) and predicts the build's work, output, and definedness items; the build
//! constructs the slots. A plan is an upper bound: a slot read of a state variable is
//! charged the text of that variable's longest slot name, so every slot of one
//! variable has one measure and a key the plan does not know (a parameter or a binder)
//! does not change it.
//!
//! # Every slot expression is total
//!
//! A slot is a state read, a constant, `max`, `min`, or a difference of two slots, or a
//! computed payload whose interval fits `i64` ([`Unlowerable::DynamicValue`] otherwise).
//! None of them can fail, so a slot selected away (`m.put(k, v).get(j)`) drops no error
//! the CML model has. The one partial operation, `m[k]`, is total in the lowered model
//! and records its definedness as an item ([`LayOps::f_push`]); items are never
//! dropped.
//!
//! # Definedness
//!
//! An undefined read is an error in CML (strict: in every clause that contains it,
//! even in a branch that does not decide the result). The lowering keeps it as a named
//! predicate rather than a default value: an invariant `I` gets `I#defined`, an action
//! `A` gets `A#defined` and its guard gets the condition too, and a guarded quantifier
//! instance contributes `guard => items` (only where the CML domain evaluates it).

use std::collections::BTreeMap;
use std::rc::Rc;

use continuum_cml_syntax::Span;
use continuum_model_core::expr::{Environment, MAX_EXPR_DEPTH};
use continuum_model_core::ident::MAX_IDENT_BYTES;
use continuum_model_core::{BoolExpr, CmpOp, EvalError, IntExpr};

use super::layout::{Card, Ty, is_scalar};
use super::{
    Build, Guard, Iv, M, Meter, Mode, Plan, R, Sized, Unlowerable, balanced_measure, bool_expr,
    burn, charge_in, copy, effort, int_expr, interval, levels, no, node, predicted_fits,
    reads_state, scoped, work,
};
use crate::budget::{MAX_TYPE_DEPTH, lookup_cost, sort_cost, text_cost};
use crate::norm::{BinOp, Builtin, Expr, ExprKind};
use crate::types::Type;

/// The suffix of a definedness predicate: `A#defined` for an action `A`, `I#defined`
/// for an invariant `I` (RFC 0003 correction 4, "Definedness"). `#` is not a CML
/// identifier character, so no declared name collides with one.
pub const DEFINED_SUFFIX: &str = "#defined";

/// The action or invariant whose definedness predicate `predicate` is, or `None` for
/// any other predicate. A state that violates `X#defined` is the typed outcome
/// "undefined read in `X`": it is not a verdict of a declared invariant, and the
/// verdict of `X` at that state is not a CML verdict.
#[must_use]
pub fn definedness_subject(predicate: &str) -> Option<&str> {
    predicate
        .strip_suffix(DEFINED_SUFFIX)
        .filter(|s| !s.is_empty())
}

// ---------------------------------------------------------------------------
// the state layout
// ---------------------------------------------------------------------------

/// The flat layout of each composite state variable, by name. A scalar state variable
/// is one slot with its own name and is not listed.
#[derive(Debug, Default)]
pub(super) struct StateLayout {
    pub(super) vars: BTreeMap<String, LaidVar>,
}

/// One composite state variable's slots.
#[derive(Debug)]
pub(super) struct LaidVar {
    /// Its type.
    pub(super) ty: Type,
    /// Each slot's name and domain, in layout order.
    pub(super) slots: Vec<(String, i64, i64)>,
    /// The text every read of one of its slots is charged: the longest slot name's.
    pub(super) text: usize,
    /// The hull of its slots' domains.
    pub(super) hull: (i64, i64),
}

/// The tables a type's universe depends on.
pub(super) fn ty(budget: &Meter) -> Ty<'_> {
    Ty {
        bind: &budget.bind,
        enums: &budget.enums,
    }
}

/// Spend the cost of the computations over `ty` one operation makes — at most four
/// passes (shape, cardinality, width, hull), each visiting a node at most depth times
/// and taking at most 128 steps per saturating power: four times its size times its
/// depth plus 128 — refusing a type deeper than [`MAX_TYPE_DEPTH`] first, so no
/// recursion over a hand-built type goes deeper. Returns the cost, which is also what
/// each further per-slot computation over the type (`code_at`, one pass) is charged.
pub(super) fn type_cost<Md: Mode>(ty: &Type, budget: &mut Meter, span: Span) -> R<u64> {
    let (size, depth) = crate::types::type_measure(ty);
    if depth > MAX_TYPE_DEPTH {
        return no(Unlowerable::ExpressionTooDeep, span);
    }
    let cost = (size as u64)
        .saturating_mul(depth as u64 + 128)
        .saturating_mul(4);
    effort::<Md>(budget, cost, span)?;
    Ok(cost)
}

/// The refusal for a composite type with no finite universe: unbounded under a
/// configuration is `cml.lower.unbounded_type`; otherwise `legacy`.
fn infinite(budget: &Meter, card: Card, legacy: Unlowerable) -> Unlowerable {
    match card {
        Card::Unbounded if budget.bind.configured => Unlowerable::UnboundedType,
        _ => legacy,
    }
}

/// The number of slots of the finite type `ty`, which must fit what the output budget
/// has left: every slot is at least one node, so a wider layout is refused before any
/// vector of its width is allocated.
fn width_of(ty: &Type, budget: &Meter, span: Span) -> R<usize> {
    let w = self::ty(budget).width(ty);
    let room = (budget.prepaid as u128).saturating_add(budget.nodes.left() as u128);
    if w > room {
        return no(Unlowerable::OutputTooLarge, span);
    }
    Ok(usize::try_from(w).unwrap_or(usize::MAX))
}

/// The hull of the slot codes of `ty`'s layout.
fn hull(t: Ty<'_>, ty: &Type) -> (i64, i64) {
    match ty {
        _ if is_scalar(ty) => t.range(ty).unwrap_or((0, 0)),
        Type::Option(v) => hull_opt(t, v),
        Type::Tuple(ts) => ts.iter().fold((0, 0), |(lo, hi), c| {
            let (a, b) = hull(t, c);
            (lo.min(a), hi.max(b))
        }),
        Type::Map(_, v) => hull_opt(t, v),
        _ => (0, 1),
    }
}

/// [`hull`] of `Option[v]`.
fn hull_opt(t: Ty<'_>, v: &Type) -> (i64, i64) {
    if is_scalar(v) {
        (0, i64::try_from(t.count(v)).unwrap_or(i64::MAX))
    } else {
        let (lo, hi) = hull(t, v);
        (lo.min(0), hi.max(1))
    }
}

// ---------------------------------------------------------------------------
// the two modes' layouts and definedness frames
// ---------------------------------------------------------------------------

/// The plan of a layout: its slot count, the largest measure of one slot, and the hull
/// of every slot's value (`None` when one may leave `i64`).
#[derive(Debug, Clone, Copy)]
pub(super) struct Agg {
    width: usize,
    slot: M,
    iv: Option<Iv>,
}

/// The plan of a definedness frame: how many items, their total size, and the deepest.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct DAgg {
    pub(super) items: u128,
    pub(super) size: usize,
    pub(super) depth: usize,
}

impl DAgg {
    fn add(&mut self, other: Self) {
        self.items = self.items.saturating_add(other.items);
        self.size = self.size.saturating_add(other.size);
        self.depth = self.depth.max(other.depth);
    }

    /// Everything added since `before`, `k` more times.
    fn scale_since(&mut self, before: Self, k: u128) {
        let items = self.items.saturating_sub(before.items).saturating_mul(k);
        let size = (self.size.saturating_sub(before.size) as u128).saturating_mul(k);
        self.items = self.items.saturating_add(items);
        self.size = self
            .size
            .saturating_add(usize::try_from(size).unwrap_or(usize::MAX));
    }
}

/// The layout and definedness operations of a mode. Each operation's plan predicts
/// exactly the work its build spends per slot, and a measure at least its build's.
pub(super) trait LayOps: std::marker::Sized {
    /// A lowered integer: an expression while building, its interval while planning.
    type I: Clone;
    /// A lowered Boolean: an expression while building, nothing while planning.
    type B: Clone;
    /// A lowered layout.
    type L;
    /// A definedness frame.
    type F;
    /// The value of a lowered integer that reads no state (building only).
    fn eval_i(x: &Self::I) -> Option<Result<i64, EvalError>>;
    /// The value of a lowered Boolean that reads no state (building only).
    fn eval_b(x: &Self::B) -> Option<Result<bool, EvalError>>;
    fn l_width(l: &Self::L) -> usize;
    /// The measure of every slot together: sizes summed, the deepest depth.
    fn l_measure(l: &Self::L) -> M;
    /// `width` constant slots, the `j`-th `code(meter, j)`; `per` units of work each
    /// for computing it, spent before it is computed.
    fn l_consts(
        budget: &mut Meter,
        width: usize,
        hull: (i64, i64),
        per: u64,
        code: &mut dyn FnMut(&Meter, usize) -> i64,
        span: Span,
    ) -> R<Self::L>;
    /// Every slot of a composite state variable.
    fn l_vars(budget: &mut Meter, var: &LaidVar, span: Span) -> R<Self::L>;
    /// The slots, and how many slots each stands for (planning: one for all).
    fn l_items(l: Self::L) -> (Vec<Sized<Self::I>>, usize);
    /// A layout of `items`, each standing for `mult` slots (planning).
    fn l_from(items: Vec<Sized<Self::I>>, mult: usize) -> Self::L;
    fn l_concat(a: Self::L, b: Self::L) -> Self::L;
    /// Slots `off..off + len`; the rest is dropped, and planned as scratch.
    fn l_select(budget: &mut Meter, l: Self::L, off: usize, len: usize) -> Self::L;
    /// The first slot and the rest; the first slot's hull is `(lo, hi)` when planning.
    fn l_split(l: Self::L) -> Option<(Sized<Self::I>, Self::L)>;
    /// Slots `off..off + part width` replaced by `part`; the replaced ones planned as
    /// scratch.
    fn l_splice(budget: &mut Meter, whole: Self::L, off: usize, part: Self::L) -> Self::L;
    /// Planning only: `base` with `part` somewhere inside it (the widest measure of
    /// both, `base`'s width). Building: `base`.
    fn l_overlay(base: Self::L, part: Self::L) -> Self::L;
    /// A charged copy.
    fn l_copy(budget: &mut Meter, l: &Self::L, span: Span) -> R<Self::L>;
    /// Every slot's value, when every slot reads no state (building only).
    fn l_eval(l: &Self::L) -> Option<Result<Vec<i64>, EvalError>>;
    /// The current frame, leaving an empty one.
    fn f_take(budget: &mut Meter) -> Self::F;
    /// Make `f` the current frame.
    fn f_put(budget: &mut Meter, f: Self::F);
    fn f_push(budget: &mut Meter, item: Sized<Self::B>);
    /// The balanced conjunction of a frame's items, `None` for none.
    fn f_fold(budget: &mut Meter, f: Self::F, span: Span) -> R<Option<Sized<Self::B>>>;
    /// The balanced conjunction (`and`) or disjunction of `items`, each standing for
    /// `mult` items (planning); the constant `and` for none.
    fn all(
        budget: &mut Meter,
        items: Vec<Sized<Self::B>>,
        mult: usize,
        and: bool,
        span: Span,
    ) -> R<Sized<Self::B>>;
}

/// The empty environment a static value is evaluated in: it reads no state.
fn nothing() -> Environment<'static> {
    Environment::new(&[], &[])
}

impl LayOps for Build {
    type L = Vec<Sized<IntExpr>>;
    type F = Vec<Sized<BoolExpr>>;
    type I = IntExpr;
    type B = BoolExpr;
    fn eval_i(x: &IntExpr) -> Option<Result<i64, EvalError>> {
        Some(x.evaluate(&nothing()))
    }
    fn eval_b(x: &BoolExpr) -> Option<Result<bool, EvalError>> {
        Some(x.evaluate(&nothing()))
    }
    fn l_width(l: &Self::L) -> usize {
        l.len()
    }
    fn l_measure(l: &Self::L) -> M {
        l.iter().fold(M { size: 0, depth: 0 }, |acc, (_, m)| M {
            size: acc.size.saturating_add(m.size),
            depth: acc.depth.max(m.depth),
        })
    }
    fn l_consts(
        budget: &mut Meter,
        width: usize,
        _: (i64, i64),
        per: u64,
        code: &mut dyn FnMut(&Meter, usize) -> i64,
        span: Span,
    ) -> R<Self::L> {
        let mut out = Vec::with_capacity(width);
        for j in 0..width {
            work::<Build>(budget, per, span)?;
            let c = code(budget, j);
            out.push(node::<Build, _>(budget, span, &[], || IntExpr::Const(c))?);
        }
        Ok(out)
    }
    fn l_vars(budget: &mut Meter, var: &LaidVar, span: Span) -> R<Self::L> {
        let mut out = Vec::with_capacity(var.slots.len());
        for (name, _, _) in &var.slots {
            charge_in::<Build>(budget, var.text, span)?;
            work::<Build>(budget, budget.vars as u64, span)?;
            out.push(node::<Build, _>(
                budget,
                span,
                &[M::text(var.text)],
                || IntExpr::var(name),
            )?);
        }
        Ok(out)
    }
    fn l_items(l: Self::L) -> (Vec<Sized<IntExpr>>, usize) {
        (l, 1)
    }
    fn l_from(items: Vec<Sized<IntExpr>>, _: usize) -> Self::L {
        items
    }
    fn l_concat(mut a: Self::L, b: Self::L) -> Self::L {
        a.extend(b);
        a
    }
    fn l_select(_: &mut Meter, mut l: Self::L, off: usize, len: usize) -> Self::L {
        let end = off.saturating_add(len).min(l.len());
        let start = off.min(end);
        l.drain(start..end).collect()
    }
    fn l_split(mut l: Self::L) -> Option<(Sized<IntExpr>, Self::L)> {
        if l.is_empty() {
            return None;
        }
        let first = l.remove(0);
        Some((first, l))
    }
    fn l_splice(_: &mut Meter, mut whole: Self::L, off: usize, part: Self::L) -> Self::L {
        let end = off.saturating_add(part.len()).min(whole.len());
        let start = off.min(end);
        whole.splice(start..end, part);
        whole
    }
    fn l_overlay(base: Self::L, _: Self::L) -> Self::L {
        base
    }
    fn l_copy(budget: &mut Meter, l: &Self::L, span: Span) -> R<Self::L> {
        let m = Self::l_measure(l);
        charge_in::<Build>(budget, m.size, span)?;
        work::<Build>(budget, m.size as u64, span)?;
        Ok(l.clone())
    }
    fn l_eval(l: &Self::L) -> Option<Result<Vec<i64>, EvalError>> {
        Some(l.iter().map(|(x, _)| x.evaluate(&nothing())).collect())
    }
    fn f_take(budget: &mut Meter) -> Self::F {
        std::mem::take(&mut budget.defs)
    }
    fn f_put(budget: &mut Meter, f: Self::F) {
        budget.defs = f;
    }
    fn f_push(budget: &mut Meter, item: Sized<BoolExpr>) {
        budget.defs.push(item);
    }
    fn f_fold(budget: &mut Meter, f: Self::F, span: Span) -> R<Option<Sized<BoolExpr>>> {
        if f.is_empty() {
            return Ok(None);
        }
        super::balanced::<Build>(f, true, budget, span).map(Some)
    }
    fn all(
        budget: &mut Meter,
        items: Vec<Sized<BoolExpr>>,
        _: usize,
        and: bool,
        span: Span,
    ) -> R<Sized<BoolExpr>> {
        if items.is_empty() {
            return node::<Build, _>(budget, span, &[], || BoolExpr::Const(and));
        }
        super::balanced::<Build>(items, and, budget, span)
    }
}

impl LayOps for Plan {
    type L = Agg;
    type F = DAgg;
    type I = Option<Iv>;
    type B = ();
    fn eval_i(_: &Option<Iv>) -> Option<Result<i64, EvalError>> {
        None
    }
    fn eval_b((): &()) -> Option<Result<bool, EvalError>> {
        None
    }
    fn l_width(l: &Agg) -> usize {
        l.width
    }
    fn l_measure(l: &Agg) -> M {
        if l.width == 0 {
            return M { size: 0, depth: 0 };
        }
        M {
            size: l.width.saturating_mul(l.slot.size),
            depth: l.slot.depth,
        }
    }
    fn l_consts(
        budget: &mut Meter,
        width: usize,
        hull: (i64, i64),
        per: u64,
        _: &mut dyn FnMut(&Meter, usize) -> i64,
        span: Span,
    ) -> R<Agg> {
        work::<Plan>(
            budget,
            (width as u64).saturating_mul(per.saturating_add(1)),
            span,
        )?;
        Ok(Agg {
            width,
            slot: M { size: 1, depth: 1 },
            iv: Some(Iv::range(hull.0, hull.1)),
        })
    }
    fn l_vars(budget: &mut Meter, var: &LaidVar, span: Span) -> R<Agg> {
        let width = var.slots.len();
        let per = (budget.vars as u64).saturating_add(1);
        work::<Plan>(budget, (width as u64).saturating_mul(per), span)?;
        Ok(Agg {
            width,
            slot: M {
                size: 1 + var.text,
                depth: 1,
            },
            iv: Some(Iv::range(var.hull.0, var.hull.1)),
        })
    }
    fn l_items(l: Agg) -> (Vec<Sized<Option<Iv>>>, usize) {
        if l.width == 0 {
            (Vec::new(), 0)
        } else {
            (vec![(l.iv, l.slot)], l.width)
        }
    }
    fn l_from(items: Vec<Sized<Option<Iv>>>, mult: usize) -> Agg {
        match items.first() {
            Some((iv, m)) => Agg {
                width: mult,
                slot: *m,
                iv: *iv,
            },
            None => Agg {
                width: 0,
                slot: M { size: 0, depth: 0 },
                iv: Some(Iv::point(0)),
            },
        }
    }
    fn l_concat(a: Agg, b: Agg) -> Agg {
        if a.width == 0 {
            return b;
        }
        if b.width == 0 {
            return a;
        }
        Agg {
            width: a.width.saturating_add(b.width),
            slot: M {
                size: a.slot.size.max(b.slot.size),
                depth: a.slot.depth.max(b.slot.depth),
            },
            iv: hull_of(a.iv, b.iv),
        }
    }
    fn l_select(budget: &mut Meter, l: Agg, _: usize, len: usize) -> Agg {
        let len = len.min(l.width);
        let dropped = (l.width - len) as u128;
        budget.scratch = budget
            .scratch
            .saturating_add(dropped.saturating_mul(l.slot.size as u128));
        Agg { width: len, ..l }
    }
    fn l_split(l: Agg) -> Option<(Sized<Option<Iv>>, Agg)> {
        if l.width == 0 {
            return None;
        }
        Some((
            (l.iv, l.slot),
            Agg {
                width: l.width - 1,
                ..l
            },
        ))
    }
    fn l_splice(budget: &mut Meter, whole: Agg, _: usize, part: Agg) -> Agg {
        let replaced = part.width.min(whole.width) as u128;
        budget.scratch = budget
            .scratch
            .saturating_add(replaced.saturating_mul(whole.slot.size as u128));
        Self::l_overlay(whole, part)
    }
    fn l_overlay(base: Agg, part: Agg) -> Agg {
        if part.width == 0 {
            return base;
        }
        Agg {
            width: base.width,
            slot: M {
                size: base.slot.size.max(part.slot.size),
                depth: base.slot.depth.max(part.slot.depth),
            },
            iv: hull_of(base.iv, part.iv),
        }
    }
    fn l_copy(budget: &mut Meter, l: &Agg, span: Span) -> R<Agg> {
        work::<Plan>(budget, Self::l_measure(l).size as u64, span)?;
        Ok(*l)
    }
    fn l_eval(_: &Agg) -> Option<Result<Vec<i64>, EvalError>> {
        None
    }
    fn f_take(budget: &mut Meter) -> DAgg {
        std::mem::take(&mut budget.defs_plan)
    }
    fn f_put(budget: &mut Meter, f: DAgg) {
        budget.defs_plan = f;
    }
    fn f_push(budget: &mut Meter, item: Sized<()>) {
        budget.defs_plan.add(DAgg {
            items: 1,
            size: item.1.size,
            depth: item.1.depth,
        });
    }
    fn f_fold(budget: &mut Meter, f: DAgg, span: Span) -> R<Option<Sized<()>>> {
        if f.items == 0 {
            return Ok(None);
        }
        let m = fold_measure(f);
        if m.depth > MAX_EXPR_DEPTH {
            return no(Unlowerable::ExpressionTooDeep, span);
        }
        work::<Plan>(budget, u64::try_from(f.items).unwrap_or(u64::MAX), span)?;
        Ok(Some(((), m)))
    }
    fn all(
        budget: &mut Meter,
        items: Vec<Sized<()>>,
        mult: usize,
        _: bool,
        span: Span,
    ) -> R<Sized<()>> {
        if items.is_empty() {
            return node::<Plan, _>(budget, span, &[], || ());
        }
        let leaf = items.iter().fold(M { size: 0, depth: 0 }, |acc, (_, m)| M {
            size: acc.size.max(m.size),
            depth: acc.depth.max(m.depth),
        });
        let n = (items.len() as u128).saturating_mul(mult as u128);
        let m = balanced_measure(n, leaf);
        if m.depth > MAX_EXPR_DEPTH {
            return no(Unlowerable::ExpressionTooDeep, span);
        }
        work::<Plan>(budget, u64::try_from(n).unwrap_or(u64::MAX), span)?;
        Ok(((), m))
    }
}

/// The measure of the balanced conjunction of a planned frame.
pub(super) fn fold_measure(f: DAgg) -> M {
    M {
        size: f
            .size
            .saturating_add(usize::try_from(f.items.saturating_sub(1)).unwrap_or(usize::MAX)),
        depth: f.depth.saturating_add(levels(f.items)),
    }
}

/// The hull of two plan hulls.
fn hull_of(a: Option<Iv>, b: Option<Iv>) -> Option<Iv> {
    match (a, b) {
        (Some(x), Some(y)) => Some(Iv::make(x.lo.min(y.lo), x.hi.max(y.hi), x.fits && y.fits)),
        _ => None,
    }
}

/// Run `f` once for an item that stands for `mult` items: while planning, everything
/// it predicted (work, scratch output, definedness items) is multiplied by `mult`, as a
/// quantifier's fan-out multiplies its instance. While building, `f` runs per item and
/// `mult` is `1`.
pub(super) fn fan<Md: Mode, T>(
    budget: &mut Meter,
    mult: usize,
    span: Span,
    f: impl FnOnce(&mut Meter) -> R<T>,
) -> R<T> {
    if Md::BUILD || mult <= 1 {
        return f(budget);
    }
    let (p, s, d) = (budget.predicted, budget.scratch, budget.defs_plan);
    let out = f(budget)?;
    scale_since(budget, (p, s, d), (mult - 1) as u128);
    predicted_fits(budget, span)?;
    Ok(out)
}

/// Multiply by `k` everything a plan predicted since `before` (work, scratch, items).
pub(super) fn scale_since(budget: &mut Meter, before: (u128, u128, DAgg), k: u128) {
    let (p, s, d) = before;
    let dp = budget.predicted.saturating_sub(p).saturating_mul(k);
    let ds = budget.scratch.saturating_sub(s).saturating_mul(k);
    budget.predicted = budget.predicted.saturating_add(dp);
    budget.scratch = budget.scratch.saturating_add(ds);
    budget.defs_plan.scale_since(d, k);
}

/// Output the build charges that no returned measure holds: while planning, counted
/// into [`Meter::scratch`].
fn scratch<Md: Mode>(budget: &mut Meter, n: usize) {
    if !Md::BUILD {
        budget.scratch = budget.scratch.saturating_add(n as u128);
    }
}

// ---------------------------------------------------------------------------
// the state layout
// ---------------------------------------------------------------------------

/// The slots of a composite state variable `name: ty`, in layout order, with their
/// names and domains (RFC 0003 correction 4, "Layout"): a scalar is one slot named by
/// its path; `Option[S]` over a scalar one slot; `Option[C]` a presence slot `p?` and
/// `C` under `p!`; a tuple its components under `p.0`, `p.1`, …; `Set[T]` one `0..=1`
/// slot `p{u}` per `u` in `U(T)`; `Map[K, V]` the layout of `Option[V]` under `p[k]` per
/// `k` in `U(K)`. The caller has checked the width against [`MAX_VARIABLES`]. Each name
/// is refused past [`MAX_IDENT_BYTES`] before it is built, and charged (a record and its
/// text, twice: the builder's copy and the table's) before it is copied. Each value
/// text node visited is one unit of work, spent first.
///
/// [`MAX_VARIABLES`]: continuum_model_core::model::MAX_VARIABLES
pub(super) fn slots_of(
    name: &str,
    ty: &Type,
    budget: &mut Meter,
    span: Span,
) -> R<Vec<(String, i64, i64)>> {
    let mut out = Vec::new();
    let mut path = String::with_capacity(MAX_IDENT_BYTES);
    path.push_str(name);
    walk(ty, &mut path, &mut out, budget, span)?;
    Ok(out)
}

/// Append to `path` the text of the value `idx` of `ty` between `open` and `close`,
/// refusing a name past [`MAX_IDENT_BYTES`] before any of it is appended.
fn push_text(
    path: &mut String,
    open: char,
    close: char,
    ty: &Type,
    idx: u128,
    budget: &mut Meter,
    span: Span,
) -> R<()> {
    let len = text_len(ty, idx, budget, span)?;
    if path.len().saturating_add(len).saturating_add(2) > MAX_IDENT_BYTES {
        return no(Unlowerable::NameTooLong, span);
    }
    path.push(open);
    let tables = self::ty(budget);
    tables.text(ty, idx, &mut Some(path), &mut || true);
    path.push(close);
    Ok(())
}

/// The length of the canonical text of the value `idx` of `ty`: one unit of work per
/// value node, spent as it is visited.
pub(super) fn text_len(ty: &Type, idx: u128, budget: &mut Meter, span: Span) -> R<usize> {
    let Meter {
        fuel, bind, enums, ..
    } = budget;
    let tables = Ty { bind, enums };
    let mut step = || fuel.burn(1).is_ok();
    match tables.text(ty, idx, &mut None, &mut step) {
        Some(n) => Ok(n),
        None => no(Unlowerable::WorkLimitExceeded, span),
    }
}

/// The longest canonical text of a value of `ty`, or `cap + 1` past `cap`: one unit of
/// work per node visited, spent as it is visited.
pub(super) fn widest_text(ty: &Type, cap: usize, budget: &mut Meter, span: Span) -> R<usize> {
    let Meter {
        fuel, bind, enums, ..
    } = budget;
    let tables = Ty { bind, enums };
    let mut step = || fuel.burn(1).is_ok();
    match tables.widest(ty, cap, &mut step) {
        Some(n) => Ok(n),
        None => no(Unlowerable::WorkLimitExceeded, span),
    }
}

/// The canonical text of the value `idx` of `ty`. The caller has spent its length.
pub(super) fn text_of(ty: &Type, idx: u128, budget: &Meter) -> String {
    let mut out = String::new();
    self::ty(budget).text(ty, idx, &mut Some(&mut out), &mut || true);
    out
}

fn add_slot(
    path: &str,
    dom: (i64, i64),
    out: &mut Vec<(String, i64, i64)>,
    budget: &mut Meter,
    span: Span,
) -> R<()> {
    if path.len() > MAX_IDENT_BYTES {
        return no(Unlowerable::NameTooLong, span);
    }
    let cost = 1 + text_cost(path.len());
    burn(budget, cost as u64, span)?;
    super::charge(budget, cost.saturating_mul(2), span)?;
    out.push((path.to_owned(), dom.0, dom.1));
    Ok(())
}

fn walk(
    ty: &Type,
    path: &mut String,
    out: &mut Vec<(String, i64, i64)>,
    budget: &mut Meter,
    span: Span,
) -> R<()> {
    let tables = self::ty(budget);
    match ty {
        _ if is_scalar(ty) => {
            let dom = tables.range(ty).unwrap_or((0, -1));
            add_slot(path, dom, out, budget, span)
        }
        Type::Option(v) => walk_opt(v, path, out, budget, span),
        Type::Tuple(ts) => {
            let at = path.len();
            for (i, t) in ts.iter().enumerate() {
                path.push('.');
                path.push_str(&i.to_string());
                if path.len() > MAX_IDENT_BYTES {
                    return no(Unlowerable::NameTooLong, span);
                }
                walk(t, path, out, budget, span)?;
                path.truncate(at);
            }
            Ok(())
        }
        Type::Set(t) => {
            let at = path.len();
            let n = tables.count(t);
            for j in 0..n {
                push_text(path, '{', '}', t, j, budget, span)?;
                add_slot(path, (0, 1), out, budget, span)?;
                path.truncate(at);
            }
            Ok(())
        }
        Type::Map(k, v) => {
            let at = path.len();
            let n = tables.count(k);
            for j in 0..n {
                push_text(path, '[', ']', k, j, budget, span)?;
                walk_opt(v, path, out, budget, span)?;
                path.truncate(at);
            }
            Ok(())
        }
        _ => no(Unlowerable::NonIntegerState, span),
    }
}

/// [`walk`] of `Option[v]`: one slot over a scalar, `p?` and `v` under `p!` otherwise.
fn walk_opt(
    v: &Type,
    path: &mut String,
    out: &mut Vec<(String, i64, i64)>,
    budget: &mut Meter,
    span: Span,
) -> R<()> {
    if is_scalar(v) {
        let n = i64::try_from(ty(budget).count(v)).unwrap_or(i64::MAX);
        return add_slot(path, (0, n), out, budget, span);
    }
    let at = path.len();
    path.push('?');
    add_slot(path, (0, 1), out, budget, span)?;
    path.truncate(at);
    path.push('!');
    walk(v, path, out, budget, span)?;
    path.truncate(at);
    Ok(())
}

/// The canonicity constraint of a composite state variable (RFC 0003 correction 4,
/// "Layout"): for each `Option[C]` over a composite `C` at presence slot `p`, `p == 1 ||
/// every slot of C at its minimum`. `None` when the layout has no such option. Every
/// node is charged as it is built.
pub(super) fn canonicity(
    var: &LaidVar,
    budget: &mut Meter,
    span: Span,
) -> R<Option<Sized<BoolExpr>>> {
    let mut items: Vec<Sized<BoolExpr>> = Vec::new();
    let mut at: u128 = 0;
    canon_walk(&var.ty, var, &mut at, &mut items, budget, span)?;
    if items.is_empty() {
        return Ok(None);
    }
    super::balanced::<Build>(items, true, budget, span).map(Some)
}

fn slot_read(var: &LaidVar, j: u128, budget: &mut Meter, span: Span) -> R<Sized<IntExpr>> {
    let Some((name, _, _)) = usize::try_from(j).ok().and_then(|j| var.slots.get(j)) else {
        return no(Unlowerable::NonIntegerState, span);
    };
    charge_in::<Build>(budget, var.text, span)?;
    work::<Build>(budget, budget.vars as u64, span)?;
    node::<Build, _>(budget, span, &[M::text(var.text)], || IntExpr::var(name))
}

fn cmp_const(
    x: Sized<IntExpr>,
    op: CmpOp,
    c: i64,
    budget: &mut Meter,
    span: Span,
) -> R<Sized<BoolExpr>> {
    let (k, sk) = node::<Build, _>(budget, span, &[], || IntExpr::Const(c))?;
    node::<Build, _>(budget, span, &[x.1, sk], || BoolExpr::compare(op, x.0, k))
}

fn canon_walk(
    ty: &Type,
    var: &LaidVar,
    at: &mut u128,
    items: &mut Vec<Sized<BoolExpr>>,
    budget: &mut Meter,
    span: Span,
) -> R<()> {
    let tables = self::ty(budget);
    match ty {
        _ if is_scalar(ty) => {
            *at += 1;
            Ok(())
        }
        Type::Option(v) => canon_opt(v, var, at, items, budget, span),
        Type::Tuple(ts) => {
            for t in ts {
                canon_walk(t, var, at, items, budget, span)?;
            }
            Ok(())
        }
        Type::Set(t) => {
            *at = at.saturating_add(tables.count(t));
            Ok(())
        }
        Type::Map(k, v) => {
            for _ in 0..tables.count(k) {
                canon_opt(v, var, at, items, budget, span)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// [`canon_walk`] of `Option[v]`.
fn canon_opt(
    v: &Type,
    var: &LaidVar,
    at: &mut u128,
    items: &mut Vec<Sized<BoolExpr>>,
    budget: &mut Meter,
    span: Span,
) -> R<()> {
    if is_scalar(v) {
        *at += 1;
        return Ok(());
    }
    let p = *at;
    let w = ty(budget).width(v);
    if w > 0 {
        let present = slot_read(var, p, budget, span)?;
        let present = cmp_const(present, CmpOp::Eq, 1, budget, span)?;
        let mut mins = Vec::new();
        for j in 0..w {
            let (lo, _) = ty(budget).slot_domain(v, j);
            let s = slot_read(var, p + 1 + j, budget, span)?;
            mins.push(cmp_const(s, CmpOp::Eq, lo, budget, span)?);
        }
        let absent = super::balanced::<Build>(mins, true, budget, span)?;
        items.push(node::<Build, _>(
            budget,
            span,
            &[present.1, absent.1],
            || BoolExpr::or(present.0, absent.0),
        )?);
    }
    *at += 1;
    canon_walk(v, var, at, items, budget, span)
}

// ---------------------------------------------------------------------------
// static values
// ---------------------------------------------------------------------------

/// The code of a static scalar expression (a key, a member, a payload): lowered, then
/// evaluated with no state. The lowered expression is scratch: charged, then dropped.
/// An expression that reads state is [`Unlowerable::DynamicKey`]; one whose evaluation
/// fails is [`Unlowerable::ValueOutsideBound`]. `None` while planning.
#[inline(never)]
fn static_code<Md: Mode>(x: &Expr, budget: &mut Meter) -> R<Option<i64>> {
    if reads_state::<Md>(x, budget)? {
        return no(Unlowerable::DynamicKey, x.span);
    }
    let value = if x.ty == Type::Bool {
        let (b, m) = bool_expr::<Md>(x, budget)?;
        scratch::<Md>(budget, m.size);
        work::<Md>(budget, m.size as u64, x.span)?;
        Md::eval_b(&b).map(|r| r.map(i64::from))
    } else {
        let (i, m) = int_expr::<Md>(x, budget)?;
        scratch::<Md>(budget, m.size);
        work::<Md>(budget, m.size as u64, x.span)?;
        Md::eval_i(&i)
    };
    match value {
        None => Ok(None),
        Some(Ok(c)) => Ok(Some(c)),
        Some(Err(_)) => no(Unlowerable::ValueOutsideBound, x.span),
    }
}

/// The index in `U(t)` of the static expression `x` of type `t` (`None` while
/// planning). A scalar through [`static_code`]; a composite by its layout, evaluated
/// slot by slot. Outside the universe is [`Unlowerable::ValueOutsideBound`].
#[inline(never)]
fn static_index<Md: Mode>(x: &Expr, t: &Type, budget: &mut Meter) -> R<Option<u128>> {
    if is_scalar(t) {
        let Some(c) = static_code::<Md>(x, budget)? else {
            return Ok(None);
        };
        return match ty(budget).range(t) {
            Ok((lo, hi)) if lo <= c && c <= hi => {
                Ok(Some((i128::from(c) - i128::from(lo)) as u128))
            }
            _ => no(Unlowerable::ValueOutsideBound, x.span),
        };
    }
    if reads_state::<Md>(x, budget)? {
        return no(Unlowerable::DynamicKey, x.span);
    }
    let l = lay_at::<Md>(x, t, budget)?;
    let m = Md::l_measure(&l);
    scratch::<Md>(budget, m.size);
    work::<Md>(budget, m.size as u64, x.span)?;
    match Md::l_eval(&l) {
        None => Ok(None),
        Some(Err(_)) => no(Unlowerable::ValueOutsideBound, x.span),
        Some(Ok(codes)) => match ty(budget).index_of(t, &codes) {
            Some(i) => Ok(Some(i)),
            None => no(Unlowerable::ValueOutsideBound, x.span),
        },
    }
}

/// The atom of an action parameter or bound variable of a composite type: its index,
/// as one constant node (for membership in a constant set of composites).
pub(super) fn atom_node<Md: Mode>(x: &Expr, budget: &mut Meter) -> R<Option<Sized<Md::I>>> {
    if is_scalar(&x.ty) || !matches!(x.kind, ExprKind::Param(_) | ExprKind::Bound { .. }) {
        return Ok(None);
    }
    match scoped::<Md>(x, budget, x.span)? {
        Some((lo, hi)) => node::<Md, _>(budget, x.span, &[], || Md::i_scoped(lo, hi)).map(Some),
        None => no(super::reason(x), x.span),
    }
}

// ---------------------------------------------------------------------------
// scalar values placed in slots
// ---------------------------------------------------------------------------

/// A scalar value in a plain scalar slot (a tuple component): a static value is its
/// code, which must be in `t`'s universe; a computed integer must have an interval
/// that fits `i64`, so the slot cannot fail; a computed Boolean has no integer code in
/// the model and is [`Unlowerable::DynamicValue`].
#[inline(never)]
fn scalar_slot<Md: Mode>(x: &Expr, t: &Type, budget: &mut Meter) -> R<Sized<Md::I>> {
    if !reads_state::<Md>(x, budget)? {
        let c = static_code::<Md>(x, budget)?;
        if let Some(c) = c {
            match ty(budget).range(t) {
                Ok((lo, hi)) if lo <= c && c <= hi => {}
                _ => return no(Unlowerable::ValueOutsideBound, x.span),
            }
        }
        return node::<Md, _>(budget, x.span, &[], || Md::i_const(c.unwrap_or(0)));
    }
    if x.ty == Type::Bool {
        return no(Unlowerable::DynamicValue, x.span);
    }
    match interval::<Md>(x, budget)? {
        Some(iv) if iv.fits => int_expr::<Md>(x, budget),
        _ => no(Unlowerable::DynamicValue, x.span),
    }
}

/// The code of `Some(x)` in the one slot of `Option[t]` over a scalar `t`: `i + 1` for
/// the `i`-th value of `U(t)`. A static `x` must be in the universe. A computed integer
/// `x` must have an interval that fits `i64` and lies at or above the universe's
/// minimum `lo`, so its code `x - lo + 1` is at least `1` (never `None`) and fits; above
/// the universe the code is above the slot's domain, which a write reports as
/// `UpdateOutOfDomain` and a comparison finds unequal, as CML does.
#[inline(never)]
fn payload_code<Md: Mode>(x: &Expr, t: &Type, budget: &mut Meter) -> R<Sized<Md::I>> {
    let sp = x.span;
    let lo = ty(budget).base(t);
    if !reads_state::<Md>(x, budget)? {
        let c = static_code::<Md>(x, budget)?;
        let code = match c {
            None => 1,
            Some(c) => match ty(budget).range(t) {
                Ok((lo, hi)) if lo <= c && c <= hi => c - lo + 1,
                _ => return no(Unlowerable::ValueOutsideBound, sp),
            },
        };
        return node::<Md, _>(budget, sp, &[], || Md::i_const(code));
    }
    if x.ty == Type::Bool {
        return no(Unlowerable::DynamicValue, sp);
    }
    let safe = match interval::<Md>(x, budget)? {
        Some(iv) => {
            iv.fits && iv.lo >= i128::from(lo) && iv.hi - i128::from(lo) < i128::from(i64::MAX)
        }
        None => false,
    };
    if !safe {
        return no(Unlowerable::DynamicValue, sp);
    }
    let v = int_expr::<Md>(x, budget)?;
    let shifted = if lo == 0 {
        v
    } else {
        let (c, sc) = node::<Md, _>(budget, sp, &[], || Md::i_const(lo))?;
        node::<Md, _>(budget, sp, &[v.1, sc], || {
            Md::i_arith(continuum_model_core::ArithOp::Sub, v.0, c)
        })?
    };
    let (one, s1) = node::<Md, _>(budget, sp, &[], || Md::i_const(1))?;
    node::<Md, _>(budget, sp, &[shifted.1, s1], || {
        Md::i_arith(continuum_model_core::ArithOp::Add, shifted.0, one)
    })
}

/// The layout of `Some(x)` as a value of `Option[t]`.
#[inline(never)]
fn some_layout<Md: Mode>(x: &Expr, t: &Type, budget: &mut Meter) -> R<Md::L> {
    if is_scalar(t) {
        let code = payload_code::<Md>(x, t, budget)?;
        return Ok(Md::l_from(vec![code], 1));
    }
    let present = Md::l_consts(budget, 1, (0, 1), 0, &mut |_, _| 1, x.span)?;
    let inner = lay_at::<Md>(x, t, budget)?;
    Ok(Md::l_concat(present, inner))
}

// ---------------------------------------------------------------------------
// layouts of expressions
// ---------------------------------------------------------------------------

/// The layout of a composite value `e` at its own type.
pub(super) fn lay_expr<Md: Mode>(e: &Expr, budget: &mut Meter) -> R<Md::L> {
    lay_at::<Md>(e, &e.ty, budget)
}

/// Whether the layout of `e` is fixed by a declared type (a state variable, a
/// constant, a parameter or binder, or a map read or write over one of them), rather
/// than by the type it is lowered at (a literal, `Some`, `None`, a tuple, or a
/// comprehension). Set operations are anchored when an operand is.
pub(super) fn anchored(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::State(_)
        | ExprKind::Const(_)
        | ExprKind::Param(_)
        | ExprKind::Bound { .. }
        | ExprKind::Index(..)
        | ExprKind::Update(..)
        | ExprKind::Builtin(Builtin::MapGet | Builtin::MapPut, _) => true,
        ExprKind::Binary(BinOp::Union | BinOp::Intersect | BinOp::Diff, a, b) => {
            anchored(a) || anchored(b)
        }
        _ => false,
    }
}

/// Whether two types have the same shape, a `Nat` matching an `Int` (the elaborator
/// unifies an integer literal's `Int` with a declared `Nat`). Recursion follows the
/// types, whose depth the caller has checked.
fn compatible(a: &Type, b: &Type) -> bool {
    match (a, b) {
        (Type::Nat | Type::Int, Type::Nat | Type::Int) => true,
        (Type::Option(x), Type::Option(y)) | (Type::Set(x), Type::Set(y)) => compatible(x, y),
        (Type::Map(k1, v1), Type::Map(k2, v2)) => compatible(k1, k2) && compatible(v1, v2),
        (Type::Tuple(xs), Type::Tuple(ys)) => {
            xs.len() == ys.len() && xs.iter().zip(ys).all(|(x, y)| compatible(x, y))
        }
        _ => a == b,
    }
}

/// The layout of a composite value `e` as a value of the type `t` (RFC 0003 correction
/// 4, "Expressions"): every slot of `t`, in layout order. `t` has `e`'s shape up to
/// `Nat` and `Int`, whose layouts differ (`Option[Nat]` codes `v + 1`, `Option[Int]`
/// codes `v - min + 1`): an anchored expression ([`anchored`]) must have exactly the
/// type `t`, and any other one is built at `t`, its static values checked against `t`'s
/// universes. Two layouts are therefore only ever combined slot by slot at one type.
pub(super) fn lay_at<Md: Mode>(e: &Expr, t: &Type, budget: &mut Meter) -> R<Md::L> {
    // A thin dispatcher: the layout recursion follows the source tree through here and
    // the arm it takes, so every arm is its own function and this frame stays small.
    let shape = prologue::<Md>(e, t, budget)?;
    match &e.kind {
        ExprKind::State(v) => lay_state::<Md>(v, shape, budget, e.span),
        ExprKind::Const(c) => {
            constant_layout::<Md>(e, c, shape.width, shape.hull, shape.tc, budget)
        }
        ExprKind::Param(_) | ExprKind::Bound { .. } => lay_atom::<Md>(e, t, shape, budget),
        ExprKind::OptionNone => lay_none::<Md>(t, shape, budget, e.span),
        ExprKind::OptionSome(x) => match t {
            Type::Option(t) => some_layout::<Md>(x, t, budget),
            _ => no(Unlowerable::NonIntegerValue, e.span),
        },
        ExprKind::Tuple(xs) => lay_tuple::<Md>(xs, t, budget, e.span),
        ExprKind::SetLit(xs) => lay_set_lit::<Md>(xs, t, shape, budget, e.span),
        ExprKind::MapLit(pairs) => lay_map_lit::<Md>(pairs, t, shape, budget, e.span),
        ExprKind::SetComp(..) | ExprKind::MapComp(..) => lay_comp::<Md>(e, t, shape, budget),
        ExprKind::Binary(op @ (BinOp::Union | BinOp::Intersect | BinOp::Diff), a, b) => {
            let la = lay_at::<Md>(a, t, budget)?;
            let lb = lay_at::<Md>(b, t, budget)?;
            set_op::<Md>(*op, la, lb, budget, e.span)
        }
        ExprKind::Builtin(Builtin::MapGet, args) => match args.as_slice() {
            [m, key] => entry::<Md>(m, key, budget, e.span),
            _ => no(Unlowerable::NonIntegerValue, e.span),
        },
        ExprKind::Builtin(Builtin::MapPut, args) => match args.as_slice() {
            [m, key, value] => put::<Md>(m, key, value, budget, e.span),
            _ => no(Unlowerable::NonIntegerValue, e.span),
        },
        ExprKind::Update(m, key, value) if matches!(m.ty, Type::Map(..)) => {
            put::<Md>(m, key, value, budget, e.span)
        }
        ExprKind::Index(m, key) if matches!(m.ty, Type::Map(..)) => {
            lay_index::<Md>(m, key, budget, e.span)
        }
        ExprKind::If(..) => no(Unlowerable::ConditionalValue, e.span),
        _ => no(super::reason_in(e, budget), e.span),
    }
}

/// What every layout of type `t` shares: the per-slot type cost, the width, and the
/// hull of the slot codes.
#[derive(Clone, Copy)]
struct Shape {
    tc: u64,
    width: usize,
    hull: (i64, i64),
}

/// The checks every layout makes before any slot: the types' costs and depths, the
/// shape (`t` a finite composite of `e`'s shape; an anchored `e` of exactly `t`), and
/// the width against the output left.
#[inline(never)]
fn prologue<Md: Mode>(e: &Expr, t: &Type, budget: &mut Meter) -> R<Shape> {
    let sp = e.span;
    let tc = type_cost::<Md>(t, budget, sp)?;
    type_cost::<Md>(&e.ty, budget, sp)?;
    let exact = !anchored(e) || e.ty == *t || matches!(e.kind, ExprKind::Binary(..));
    if is_scalar(t) || !compatible(&e.ty, t) || !exact {
        return no(Unlowerable::NonIntegerValue, sp);
    }
    let card = ty(budget).card(t);
    if !matches!(card, Card::Finite(_)) {
        return no(infinite(budget, card, Unlowerable::NonIntegerValue), sp);
    }
    let width = width_of(t, budget, sp)?;
    let hull = hull(ty(budget), t);
    Ok(Shape { tc, width, hull })
}

#[inline(never)]
fn lay_state<Md: Mode>(v: &str, shape: Shape, budget: &mut Meter, sp: Span) -> R<Md::L> {
    let cost = lookup_cost(budget.layout.vars.len(), v.len());
    work::<Md>(budget, cost, sp)?;
    let layout = Rc::clone(&budget.layout);
    match layout.vars.get(v) {
        Some(var) if var.slots.len() == shape.width => Md::l_vars(budget, var, sp),
        _ => no(Unlowerable::NonIntegerValue, sp),
    }
}

#[inline(never)]
fn lay_atom<Md: Mode>(e: &Expr, t: &Type, shape: Shape, budget: &mut Meter) -> R<Md::L> {
    let Some((atom, _)) = scoped::<Md>(e, budget, e.span)? else {
        return no(super::reason(e), e.span);
    };
    let idx = u128::try_from(atom).unwrap_or(0);
    Md::l_consts(
        budget,
        shape.width,
        shape.hull,
        shape.tc,
        &mut |m, j| ty(m).code_at(t, idx, j as u128),
        e.span,
    )
}

#[inline(never)]
fn lay_none<Md: Mode>(t: &Type, shape: Shape, budget: &mut Meter, sp: Span) -> R<Md::L> {
    Md::l_consts(
        budget,
        shape.width,
        shape.hull,
        shape.tc,
        &mut |m, j| ty(m).code_at(t, 0, j as u128),
        sp,
    )
}

#[inline(never)]
fn lay_tuple<Md: Mode>(xs: &[Expr], t: &Type, budget: &mut Meter, sp: Span) -> R<Md::L> {
    let Type::Tuple(ts) = t else {
        return no(Unlowerable::NonIntegerValue, sp);
    };
    if ts.len() != xs.len() {
        return no(Unlowerable::NonIntegerValue, sp);
    }
    let mut out = Md::l_from(Vec::new(), 0);
    for (x, t) in xs.iter().zip(ts) {
        let part = if is_scalar(t) {
            Md::l_from(vec![scalar_slot::<Md>(x, t, budget)?], 1)
        } else {
            lay_at::<Md>(x, t, budget)?
        };
        out = Md::l_concat(out, part);
    }
    Ok(out)
}

#[inline(never)]
fn lay_set_lit<Md: Mode>(
    xs: &[Expr],
    t: &Type,
    shape: Shape,
    budget: &mut Meter,
    sp: Span,
) -> R<Md::L> {
    let Type::Set(t) = t else {
        return no(Unlowerable::NonIntegerValue, sp);
    };
    let mut indices: Vec<u128> = Vec::with_capacity(xs.len());
    for x in xs {
        if let Some(i) = static_index::<Md>(x, t, budget)? {
            indices.push(i);
        }
    }
    effort::<Md>(budget, sort_cost(xs.len(), 0), sp)?;
    indices.sort_unstable();
    indices.dedup();
    members_layout::<Md>(indices, shape.width, budget, sp)
}

#[inline(never)]
fn lay_map_lit<Md: Mode>(
    pairs: &[(Expr, Expr)],
    t: &Type,
    shape: Shape,
    budget: &mut Meter,
    sp: Span,
) -> R<Md::L> {
    let Type::Map(k, v) = t else {
        return no(Unlowerable::NonIntegerValue, sp);
    };
    let mut entries: Vec<(u128, Md::L)> = Vec::with_capacity(pairs.len());
    let mut planned: Vec<Md::L> = Vec::new();
    for (kx, vx) in pairs {
        let key = static_index::<Md>(kx, k, budget)?;
        let value = some_layout::<Md>(vx, v, budget)?;
        match key {
            Some(key) => entries.push((key, value)),
            None => planned.push(value),
        }
    }
    let n = pairs.len();
    map_layout::<Md>(
        entries,
        planned,
        n,
        t,
        shape.width,
        shape.hull,
        shape.tc,
        budget,
        sp,
    )
}

#[inline(never)]
fn lay_comp<Md: Mode>(e: &Expr, t: &Type, shape: Shape, budget: &mut Meter) -> R<Md::L> {
    if reads_state::<Md>(e, budget)? {
        return no(Unlowerable::DynamicKey, e.span);
    }
    comprehension::<Md>(e, t, shape.width, shape.hull, shape.tc, budget)
}

/// `union`, `intersect`, `\` slot by slot over two layouts of one set type.
#[inline(never)]
fn set_op<Md: Mode>(op: BinOp, la: Md::L, lb: Md::L, budget: &mut Meter, sp: Span) -> R<Md::L> {
    map2::<Md>(budget, la, lb, sp, move |budget, x, y| match op {
        BinOp::Union => node::<Md, _>(budget, sp, &[x.1, y.1], || Md::i_max(x.0, y.0)),
        BinOp::Intersect => node::<Md, _>(budget, sp, &[x.1, y.1], || Md::i_min(x.0, y.0)),
        _ => {
            // `max(a - b, 0)`: `a` without `b`, over `0..=1` codes.
            let (d, sd) = node::<Md, _>(budget, sp, &[x.1, y.1], || {
                Md::i_arith(continuum_model_core::ArithOp::Sub, x.0, y.0)
            })?;
            let (z, sz) = node::<Md, _>(budget, sp, &[], || Md::i_const(0))?;
            node::<Md, _>(budget, sp, &[sd, sz], || Md::i_max(d, z))
        }
    })
}

/// `m[k]` of a composite value type: the entry's inner layout, and the definedness
/// item `presence == 1`.
#[inline(never)]
fn lay_index<Md: Mode>(m: &Expr, key: &Expr, budget: &mut Meter, sp: Span) -> R<Md::L> {
    post_read_ok::<Md>(m, budget, sp)?;
    let opt = entry::<Md>(m, key, budget, sp)?;
    let Some((present, inner)) = Md::l_split(opt) else {
        return no(Unlowerable::NonIntegerValue, sp);
    };
    let d = cmp_i::<Md>(present, CmpOp::Eq, 1, budget, sp)?;
    Md::f_push(budget, d);
    Ok(inner)
}

/// `x op c` for a lowered slot `x`.
fn cmp_i<Md: Mode>(
    x: Sized<Md::I>,
    op: CmpOp,
    c: i64,
    budget: &mut Meter,
    sp: Span,
) -> R<Sized<Md::B>> {
    let (k, sk) = node::<Md, _>(budget, sp, &[], || Md::i_const(c))?;
    node::<Md, _>(budget, sp, &[x.1, sk], || Md::b_cmp(op, x.0, k))
}

/// The layout of a constant: a set by its member atoms (slot `j` is `1` exactly when
/// element `j` of the universe is a member, found by one pass over the sorted atoms), any
/// other composite by the index of its value.
fn constant_layout<Md: Mode>(
    e: &Expr,
    c: &str,
    width: usize,
    hull: (i64, i64),
    tc: u64,
    budget: &mut Meter,
) -> R<Md::L> {
    let sp = e.span;
    if let Type::Set(t) = &e.ty {
        work::<Md>(budget, lookup_cost(budget.bind.sets.len(), c.len()), sp)?;
        let Some(atoms) = budget.bind.sets.get(c).map(Rc::clone) else {
            return no(super::reason_in(e, budget), sp);
        };
        let base = if is_scalar(t) { ty(budget).base(t) } else { 0 };
        let mut next = 0_usize;
        return Md::l_consts(
            budget,
            width,
            hull,
            1,
            &mut |_, j| {
                let atom = i128::from(base) + j as i128;
                while atoms.get(next).is_some_and(|a| i128::from(*a) < atom) {
                    next += 1;
                }
                i64::from(atoms.get(next).is_some_and(|a| i128::from(*a) == atom))
            },
            sp,
        );
    }
    work::<Md>(budget, lookup_cost(budget.bind.atoms.len(), c.len()), sp)?;
    let Some(idx) = budget.bind.atoms.get(c).copied() else {
        return no(super::reason_in(e, budget), sp);
    };
    let idx = u128::try_from(idx).unwrap_or(0);
    let t = &e.ty;
    Md::l_consts(
        budget,
        width,
        hull,
        tc,
        &mut |m, j| ty(m).code_at(t, idx, j as u128),
        sp,
    )
}

/// The layout of a set given by its members' indices, ascending: one pass.
fn members_layout<Md: Mode>(
    indices: Vec<u128>,
    width: usize,
    budget: &mut Meter,
    sp: Span,
) -> R<Md::L> {
    let mut next = 0_usize;
    Md::l_consts(
        budget,
        width,
        (0, 1),
        1,
        &mut |_, j| {
            let j = j as u128;
            while indices.get(next).is_some_and(|i| *i < j) {
                next += 1;
            }
            i64::from(indices.get(next).is_some_and(|i| *i == j))
        },
        sp,
    )
}

/// The layout of a map given by its entries (key index, `Some` layout): absent keys
/// are `None`. While building the entries are sorted (spent first) and a repeated key
/// is [`Unlowerable::DuplicateKey`]; while planning every slot is planned as a `None`
/// constant with every entry's layout overlaid.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn map_layout<Md: Mode>(
    mut entries: Vec<(u128, Md::L)>,
    planned: Vec<Md::L>,
    sorted: usize,
    map_ty: &Type,
    width: usize,
    hull: (i64, i64),
    tc: u64,
    budget: &mut Meter,
    sp: Span,
) -> R<Md::L> {
    let Type::Map(k, v) = map_ty else {
        return no(Unlowerable::NonIntegerValue, sp);
    };
    effort::<Md>(budget, sort_cost(sorted, 0), sp)?;
    if !Md::BUILD {
        let t = map_ty;
        let mut out = Md::l_consts(
            budget,
            width,
            hull,
            tc,
            &mut |m, j| ty(m).code_at(t, 0, j as u128),
            sp,
        )?;
        for p in planned {
            out = Md::l_overlay(out, p);
        }
        return Ok(out);
    }
    entries.sort_by_key(|(key, _)| *key);
    if entries.windows(2).any(|w| w[0].0 == w[1].0) {
        return no(Unlowerable::DuplicateKey, sp);
    }
    let _ = k;
    let ow = usize::try_from(ty(budget).opt_width(v)).unwrap_or(usize::MAX);
    let t = map_ty;
    let mut out = Md::l_from(Vec::new(), 0);
    let mut at = 0_usize;
    for (key, value) in entries {
        let start = usize::try_from(key)
            .unwrap_or(usize::MAX)
            .saturating_mul(ow);
        if start > at {
            let gap = start - at;
            let base = at;
            let none = Md::l_consts(
                budget,
                gap,
                hull,
                tc,
                &mut |m, j| ty(m).code_at(t, 0, (base + j) as u128),
                sp,
            )?;
            out = Md::l_concat(out, none);
        }
        out = Md::l_concat(out, value);
        at = start.saturating_add(ow);
    }
    if width > at {
        let base = at;
        let none = Md::l_consts(
            budget,
            width - at,
            hull,
            tc,
            &mut |m, j| ty(m).code_at(t, 0, (base + j) as u128),
            sp,
        )?;
        out = Md::l_concat(out, none);
    }
    Ok(out)
}

/// The `Option[V]` layout of entry `key` of the map `m`. The key must be static.
#[inline(never)]
fn entry<Md: Mode>(m: &Expr, key: &Expr, budget: &mut Meter, sp: Span) -> R<Md::L> {
    let Type::Map(k, v) = &m.ty else {
        return no(Unlowerable::NonIntegerValue, sp);
    };
    type_cost::<Md>(&m.ty, budget, sp)?;
    let idx = static_index::<Md>(key, k, budget)?;
    let whole = lay_expr::<Md>(m, budget)?;
    let ow = usize::try_from(ty(budget).opt_width(v)).unwrap_or(usize::MAX);
    let off = usize::try_from(idx.unwrap_or(0))
        .unwrap_or(usize::MAX)
        .saturating_mul(ow);
    Ok(Md::l_select(budget, whole, off, ow))
}

/// `m.put(key, value)` and `m[key := value]`: `m`'s layout with entry `key` replaced by
/// the layout of `Some(value)`. Every operand is lowered (strict), and the replaced
/// slots, which cannot fail, are dropped.
#[inline(never)]
fn put<Md: Mode>(m: &Expr, key: &Expr, value: &Expr, budget: &mut Meter, sp: Span) -> R<Md::L> {
    let Type::Map(k, v) = &m.ty else {
        return no(Unlowerable::NonIntegerValue, sp);
    };
    type_cost::<Md>(&m.ty, budget, sp)?;
    let whole = lay_expr::<Md>(m, budget)?;
    let idx = static_index::<Md>(key, k, budget)?;
    let part = some_layout::<Md>(value, v, budget)?;
    let ow = usize::try_from(ty(budget).opt_width(v)).unwrap_or(usize::MAX);
    let off = usize::try_from(idx.unwrap_or(0))
        .unwrap_or(usize::MAX)
        .saturating_mul(ow);
    Ok(Md::l_splice(budget, whole, off, part))
}

/// Combine two layouts of one type slot by slot.
#[inline(never)]
fn map2<Md: Mode>(
    budget: &mut Meter,
    a: Md::L,
    b: Md::L,
    sp: Span,
    mut f: impl FnMut(&mut Meter, Sized<Md::I>, Sized<Md::I>) -> R<Sized<Md::I>>,
) -> R<Md::L> {
    let (xs, mult) = Md::l_items(a);
    let (ys, _) = Md::l_items(b);
    if xs.len() != ys.len() {
        return no(Unlowerable::NonIntegerValue, sp);
    }
    let mut out = Vec::with_capacity(xs.len());
    for (x, y) in xs.into_iter().zip(ys) {
        out.push(fan::<Md, _>(budget, mult, sp, |budget| f(budget, x, y))?);
    }
    Ok(Md::l_from(out, mult))
}

/// Compare two layouts of one type slot by slot, and combine the comparisons: `==` is
/// the conjunction of slot equalities (canonical layouts are a bijection), `subseteq`
/// the conjunction of `a <= b` over set slots.
#[inline(never)]
fn compare<Md: Mode>(
    budget: &mut Meter,
    a: Md::L,
    b: Md::L,
    op: CmpOp,
    sp: Span,
) -> R<Sized<Md::B>> {
    let (xs, mult) = Md::l_items(a);
    let (ys, _) = Md::l_items(b);
    if xs.len() != ys.len() {
        return no(Unlowerable::NonIntegerValue, sp);
    }
    let mut items = Vec::with_capacity(xs.len());
    for (x, y) in xs.into_iter().zip(ys) {
        items.push(fan::<Md, _>(budget, mult, sp, |budget| {
            node::<Md, _>(budget, sp, &[x.1, y.1], || Md::b_cmp(op, x.0, y.0))
        })?);
    }
    Md::all(budget, items, mult, true, sp)
}

/// `a == b` (or `a subseteq b`) over a composite type.
#[inline(never)]
pub(super) fn lay_compare<Md: Mode>(
    a: &Expr,
    b: &Expr,
    op: BinOp,
    budget: &mut Meter,
    sp: Span,
) -> R<Sized<Md::B>> {
    // The type both sides are lowered at: an anchored side's own (both anchored must
    // agree, else `lay_at` refuses), else the left side's.
    let t = if anchored(b) && !anchored(a) {
        &b.ty
    } else {
        &a.ty
    };
    let la = lay_at::<Md>(a, t, budget)?;
    let lb = lay_at::<Md>(b, t, budget)?;
    let cmp = if op == BinOp::SubsetEq {
        CmpOp::Le
    } else {
        CmpOp::Eq
    };
    compare::<Md>(budget, la, lb, cmp, sp)
}

/// Whether `ty` is a finite composite type the layout lowering handles.
pub(super) fn is_composite(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Option(_) | Type::Tuple(_) | Type::Set(_) | Type::Map(..)
    )
}

/// `x in set` beyond the scalar paths of the parent module (a range, or a scalar in a
/// set literal or constant): `None` when those apply.
///
/// - a composite `x` in a set literal, or in a constant set when `x` is not a parameter
///   or binder: the balanced disjunction of `x == m` over the members, `x` lowered once
///   and copied per member; no member is `false` (a layout's slots cannot fail, and its
///   definedness items are already recorded);
/// - otherwise `x` must be static: its index selects the slot `set{x}` of the set's
///   layout, and the result is `slot == 1`.
#[inline(never)]
pub(super) fn membership_of<Md: Mode>(
    x: &Expr,
    set: &Expr,
    sp: Span,
    budget: &mut Meter,
) -> R<Option<Sized<Md::B>>> {
    let Type::Set(t) = &set.ty else {
        return Ok(None);
    };
    let literal = matches!(set.kind, ExprKind::SetLit(_) | ExprKind::Const(_));
    if is_scalar(t) && literal {
        return Ok(None);
    }
    // A parameter or binder is its index in its own type, and a constant set of
    // composites holds indices in its element type: compared as integers only when the
    // two types are one.
    if !is_scalar(t)
        && matches!(set.kind, ExprKind::Const(_))
        && matches!(x.kind, ExprKind::Param(_) | ExprKind::Bound { .. })
        && x.ty == **t
    {
        return Ok(None);
    }
    if !is_scalar(t) && literal {
        // The type the members are compared at: `x`'s when it is anchored, else the
        // set's element type; a constant's members are values of its element type.
        let target: &Type = if anchored(x) && !matches!(set.kind, ExprKind::Const(_)) {
            &x.ty
        } else {
            t
        };
        let mut lx = Some(lay_at::<Md>(x, target, budget)?);
        let mut items = Vec::new();
        match &set.kind {
            ExprKind::SetLit(ms) => {
                for (i, m) in ms.iter().enumerate() {
                    // Every use but the last is a charged copy; the last is moved.
                    let xi = match (i + 1 == ms.len(), lx.take()) {
                        (true, Some(l)) => l,
                        (false, Some(l)) => {
                            let c = Md::l_copy(budget, &l, sp)?;
                            lx = Some(l);
                            c
                        }
                        (_, None) => return no(Unlowerable::NonIntegerValue, sp),
                    };
                    let lm = lay_at::<Md>(m, target, budget)?;
                    items.push(compare::<Md>(budget, xi, lm, CmpOp::Eq, sp)?);
                }
            }
            ExprKind::Const(c) => {
                work::<Md>(budget, lookup_cost(budget.bind.sets.len(), c.len()), sp)?;
                let Some(atoms) = budget.bind.sets.get(c).map(Rc::clone) else {
                    return no(super::reason_in(set, budget), set.span);
                };
                let tc = type_cost::<Md>(t, budget, sp)?;
                let width = width_of(t, budget, sp)?;
                let h = hull(ty(budget), t);
                let n = atoms.len();
                for (i, a) in atoms.iter().enumerate() {
                    let xi = match (i + 1 == n, lx.take()) {
                        (true, Some(l)) => l,
                        (false, Some(l)) => {
                            let c = Md::l_copy(budget, &l, sp)?;
                            lx = Some(l);
                            c
                        }
                        (_, None) => return no(Unlowerable::NonIntegerValue, sp),
                    };
                    let idx = u128::try_from(*a).unwrap_or(0);
                    let lm = Md::l_consts(
                        budget,
                        width,
                        h,
                        tc,
                        &mut |m, j| ty(m).code_at(t, idx, j as u128),
                        sp,
                    )?;
                    items.push(compare::<Md>(budget, xi, lm, CmpOp::Eq, sp)?);
                }
            }
            _ => {}
        }
        // No member: `x` was lowered (its items recorded) and is dropped; its slots
        // cannot fail, so the result is `false`, and its output is scratch.
        if let Some(l) = lx.take() {
            scratch::<Md>(budget, Md::l_measure(&l).size);
        }
        return Md::all(budget, items, 1, false, sp).map(Some);
    }
    // A static member selects one slot.
    let idx = static_index::<Md>(x, t, budget)?;
    let whole = lay_expr::<Md>(set, budget)?;
    let off = usize::try_from(idx.unwrap_or(0)).unwrap_or(usize::MAX);
    let one = Md::l_select(budget, whole, off, 1);
    let Some((slot, _)) = Md::l_split(one) else {
        return no(Unlowerable::ValueOutsideBound, x.span);
    };
    cmp_i::<Md>(slot, CmpOp::Eq, 1, budget, sp).map(Some)
}

/// The slot `m[k]` reads, for a scalar value type, with its definedness recorded: the
/// item `slot != 0` (`k` is a key of `m`), on a charged copy of the slot.
#[inline(never)]
fn read_slot<Md: Mode>(m: &Expr, key: &Expr, budget: &mut Meter, sp: Span) -> R<Sized<Md::I>> {
    post_read_ok::<Md>(m, budget, sp)?;
    let opt = entry::<Md>(m, key, budget, sp)?;
    let Some((slot, _)) = Md::l_split(opt) else {
        return no(Unlowerable::NonIntegerValue, sp);
    };
    let again = copy::<Md, _>(budget, sp, &slot)?;
    let d = cmp_i::<Md>(again, CmpOp::Ne, 0, budget, sp)?;
    Md::f_push(budget, d);
    Ok(slot)
}

/// A map read in a postcondition whose map reads the post-state has a definedness
/// that depends on the relational candidate, which one state predicate `A#defined`
/// cannot carry: refused as [`Unlowerable::DynamicValue`] (bn-23hzh, adversarial
/// review). One unit of work per node scanned, in both modes.
fn post_read_ok<Md: Mode>(m: &Expr, budget: &mut Meter, sp: Span) -> R<()> {
    if !budget.post {
        return Ok(());
    }
    let mut stack = vec![m];
    while let Some(e) = stack.pop() {
        effort::<Md>(budget, 1, sp)?;
        if matches!(e.kind, ExprKind::Primed(_)) {
            return no(Unlowerable::DynamicValue, sp);
        }
        stack.extend(crate::elab::children(e));
    }
    Ok(())
}

/// `m[k]` of an integer-coded value type: the slot decoded, `max(s, 1) - 1 + base`,
/// which cannot overflow (it lies in the value type's universe for every slot code, and
/// where the read is undefined the definedness item is false).
#[inline(never)]
pub(super) fn index_int<Md: Mode>(
    e: &Expr,
    m: &Expr,
    key: &Expr,
    budget: &mut Meter,
) -> R<Sized<Md::I>> {
    let sp = e.span;
    let Type::Map(_, v) = &m.ty else {
        return no(Unlowerable::NonIntegerValue, sp);
    };
    let s = read_slot::<Md>(m, key, budget, sp)?;
    let base = ty(budget).base(v);
    let (one, s1) = node::<Md, _>(budget, sp, &[], || Md::i_const(1))?;
    let (mx, smx) = node::<Md, _>(budget, sp, &[s.1, s1], || Md::i_max(s.0, one))?;
    let (one2, s2) = node::<Md, _>(budget, sp, &[], || Md::i_const(1))?;
    let dec = node::<Md, _>(budget, sp, &[smx, s2], || {
        Md::i_arith(continuum_model_core::ArithOp::Sub, mx, one2)
    })?;
    if base == 0 {
        return Ok(dec);
    }
    let (b, sb) = node::<Md, _>(budget, sp, &[], || Md::i_const(base))?;
    node::<Md, _>(budget, sp, &[dec.1, sb], || {
        Md::i_arith(continuum_model_core::ArithOp::Add, dec.0, b)
    })
}

/// `m[k]` of a Boolean value type: `slot == 2` (`Some(true)`).
#[inline(never)]
pub(super) fn index_bool<Md: Mode>(
    e: &Expr,
    m: &Expr,
    key: &Expr,
    budget: &mut Meter,
) -> R<Sized<Md::B>> {
    let s = read_slot::<Md>(m, key, budget, e.span)?;
    cmp_i::<Md>(s, CmpOp::Eq, 2, budget, e.span)
}

// ---------------------------------------------------------------------------
// comprehensions
// ---------------------------------------------------------------------------

/// A static set or map comprehension: its binders expanded over their (static)
/// domains, the filter evaluated at each candidate, and the element (a set member, or a
/// map key and value) taken where it holds. The caller has shown it reads no state.
/// Planned like a quantifier: one instance with each binder at its whole candidate
/// interval, multiplied by the fan-out.
#[inline(never)]
fn comprehension<Md: Mode>(
    e: &Expr,
    t: &Type,
    width: usize,
    hull: (i64, i64),
    tc: u64,
    budget: &mut Meter,
) -> R<Md::L> {
    let sp = e.span;
    let (binders, filter) = match &e.kind {
        ExprKind::SetComp(_, bs, f) | ExprKind::MapComp(_, _, bs, f) => (bs, f.as_deref()),
        _ => return no(Unlowerable::NonIntegerValue, sp),
    };
    let mut sets: Vec<u128> = Vec::new();
    let mut maps: Vec<(u128, Md::L)> = Vec::new();
    let mut planned: Vec<Md::L> = Vec::new();
    // The leaves visited: counted while building, and while planning one per planned
    // leaf multiplied by the fan-out, so the sort below is charged by the same count
    // in both modes (the plan's an upper bound).
    let mut leaves: u128 = 0;
    let mut out = Out {
        sets: &mut sets,
        maps: &mut maps,
        planned: &mut planned,
        leaves: &mut leaves,
    };
    comp_level::<Md>(e, t, binders, filter, &mut out, budget, sp)?;
    let n = usize::try_from(leaves).unwrap_or(usize::MAX);
    match t {
        Type::Set(_) => {
            effort::<Md>(budget, sort_cost(n, 0), sp)?;
            sets.sort_unstable();
            sets.dedup();
            members_layout::<Md>(sets, width, budget, sp)
        }
        _ => map_layout::<Md>(maps, planned, n, t, width, hull, tc, budget, sp),
    }
}

/// What a comprehension collects.
struct Out<'o, L> {
    sets: &'o mut Vec<u128>,
    maps: &'o mut Vec<(u128, L)>,
    planned: &'o mut Vec<L>,
    leaves: &'o mut u128,
}

fn comp_level<Md: Mode>(
    e: &Expr,
    t: &Type,
    binders: &[(u32, crate::norm::Binder)],
    filter: Option<&Expr>,
    out: &mut Out<'_, Md::L>,
    budget: &mut Meter,
    sp: Span,
) -> R<()> {
    let Some(((id, binder), rest)) = binders.split_first() else {
        *out.leaves = out.leaves.saturating_add(1);
        return comp_leaf::<Md>(e, t, filter, out, budget);
    };
    if budget.binder_depth >= super::MAX_BINDER_NESTING {
        return no(Unlowerable::ExpressionTooDeep, sp);
    }
    budget.binder_depth += 1;
    let r = (|| {
        let cands = super::candidates::<Md>(binder, sp, budget)?;
        let (count, reach) = match &cands {
            super::Cands::Span {
                lo,
                hi,
                guard: None,
                reach,
            } => (super::span_count(*lo, *hi), *reach),
            super::Cands::Upto { lo, hi, count } => (*count, (*lo, *hi)),
            super::Cands::List(vs) => (
                vs.len() as u128,
                (
                    vs.first().copied().unwrap_or(0),
                    vs.last().copied().unwrap_or(-1),
                ),
            ),
            super::Cands::Span { .. } => return no(Unlowerable::DynamicKey, sp),
        };
        if count == 0 {
            return Ok(());
        }
        let cost = lookup_cost(budget.env.binders.len().saturating_add(1), 8).saturating_mul(2);
        if !Md::BUILD {
            let before = (budget.predicted, budget.scratch, budget.defs_plan);
            let leaves = *out.leaves;
            work::<Md>(budget, cost, sp)?;
            let shadowed = super::enter(budget, *id, reach);
            let r = comp_level::<Md>(e, t, rest, filter, out, budget, sp);
            super::leave(budget, *id, shadowed);
            r?;
            scale_since(budget, before, count.saturating_sub(1));
            let more = out.leaves.saturating_sub(leaves);
            *out.leaves = out.leaves.saturating_add(more.saturating_mul(count - 1));
            return predicted_fits(budget, sp);
        }
        let values: Box<dyn Iterator<Item = i64>> = match cands {
            super::Cands::Span { lo, hi, .. } => Box::new(lo..=hi),
            super::Cands::List(vs) => {
                Box::new((0..vs.len()).filter_map(move |i| vs.get(i).copied()))
            }
            super::Cands::Upto { .. } => return no(Unlowerable::Quantifier, sp),
        };
        for v in values {
            work::<Md>(budget, cost, sp)?;
            let shadowed = super::enter(budget, *id, (v, v));
            let r = comp_level::<Md>(e, t, rest, filter, out, budget, sp);
            super::leave(budget, *id, shadowed);
            r?;
        }
        Ok(())
    })();
    budget.binder_depth -= 1;
    r
}

fn comp_leaf<Md: Mode>(
    e: &Expr,
    t: &Type,
    filter: Option<&Expr>,
    out: &mut Out<'_, Md::L>,
    budget: &mut Meter,
) -> R<()> {
    let keep = match filter {
        Some(f) => static_code::<Md>(f, budget)?.is_none_or(|v| v != 0),
        None => true,
    };
    match (&e.kind, t) {
        (ExprKind::SetComp(x, ..), Type::Set(t)) => {
            let i = static_index::<Md>(x, t, budget)?;
            if keep && let Some(i) = i {
                out.sets.push(i);
            }
        }
        (ExprKind::MapComp(k, v, ..), Type::Map(kt, vt)) => {
            let key = static_index::<Md>(k, kt, budget)?;
            let value = some_layout::<Md>(v, vt, budget)?;
            match key {
                // Planning: the value may be left out by the filter, dropped after it
                // was built, so its output is also scratch.
                None => {
                    scratch::<Md>(budget, Md::l_measure(&value).size);
                    out.planned.push(value);
                }
                Some(key) if keep => out.maps.push((key, value)),
                // Built, then left out by the filter.
                Some(_) => {}
            }
        }
        _ => return no(Unlowerable::NonIntegerValue, e.span),
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// quantifier domains over sets
// ---------------------------------------------------------------------------

/// The candidates of a binder whose domain is a set-valued expression other than a
/// range, a scalar set literal, or a constant (RFC 0003 correction 4, "collection
/// domains"): a static domain is its members, exactly; a domain that reads state is the
/// whole universe of the element type, each instance guarded by the domain's slot
/// ([`Guard::Slot`]).
#[inline(never)]
pub(super) fn set_domain<'e, Md: Mode>(
    binder: &crate::norm::Binder,
    domain: &'e Expr,
    span: Span,
    budget: &mut Meter,
) -> R<super::Cands<'e>> {
    let Type::Set(t) = &domain.ty else {
        return no(super::reason_in(domain, budget), domain.span);
    };
    // A composite binder's atom is its index in its own type's universe, so the
    // domain's members must be values of exactly that type.
    if !is_scalar(t) && **t != binder.ty {
        return no(Unlowerable::NonIntegerValue, domain.span);
    }
    type_cost::<Md>(&domain.ty, budget, span)?;
    let n = match ty(budget).card(t) {
        Card::Finite(n) => n,
        c => return no(infinite(budget, c, Unlowerable::Quantifier), span),
    };
    let base = if is_scalar(t) { ty(budget).base(t) } else { 0 };
    let top = i128::from(base) + n as i128 - 1;
    let Ok(hi) = i64::try_from(top) else {
        return no(Unlowerable::OutputTooLarge, span);
    };
    if n == 0 {
        return Ok(super::Cands::List(Rc::from(Vec::new())));
    }
    if reads_state::<Md>(domain, budget)? {
        return Ok(super::Cands::Span {
            lo: base,
            hi,
            guard: Some(Guard::Slot(domain)),
            reach: (base, hi),
        });
    }
    let l = lay_expr::<Md>(domain, budget)?;
    let m = Md::l_measure(&l);
    scratch::<Md>(budget, m.size);
    work::<Md>(budget, m.size as u64, span)?;
    match Md::l_eval(&l) {
        None => Ok(super::Cands::Upto {
            lo: base,
            hi,
            count: n,
        }),
        Some(Err(_)) => no(Unlowerable::ValueOutsideBound, span),
        Some(Ok(codes)) => {
            let members: Vec<i64> = codes
                .iter()
                .enumerate()
                .filter(|(_, c)| **c == 1)
                .map(|(j, _)| base.saturating_add(j as i64))
                .collect();
            Ok(super::Cands::List(Rc::from(members)))
        }
    }
}

/// The candidates of a binder of a composite element type over a set literal: its
/// members, each static, by index, ascending and distinct.
#[inline(never)]
pub(super) fn literal_domain<'e, Md: Mode>(
    binder: &'e crate::norm::Binder,
    members: &'e [Expr],
    span: Span,
    budget: &mut Meter,
) -> R<super::Cands<'e>> {
    let mut out: Vec<i64> = Vec::with_capacity(members.len());
    for m in members {
        if let Some(i) = static_index::<Md>(m, &binder.ty, budget)? {
            out.push(i64::try_from(i).unwrap_or(i64::MAX));
        }
    }
    // The sort, spent before it runs in the plan as in the build (one static index per
    // member in both).
    effort::<Md>(budget, sort_cost(members.len(), 0), span)?;
    if !Md::BUILD {
        let n = ty(budget).count(&binder.ty);
        let hi = i64::try_from(n.saturating_sub(1)).unwrap_or(i64::MAX);
        return Ok(super::Cands::Upto {
            lo: 0,
            hi,
            count: (members.len() as u128).min(n),
        });
    }
    out.sort_unstable();
    out.dedup();
    Ok(super::Cands::List(Rc::from(out)))
}

/// The guard `domain{u} == 1` of the instance at atom `v` of a guarded set domain.
#[inline(never)]
pub(super) fn slot_guard<Md: Mode>(
    domain: &Expr,
    v: i64,
    budget: &mut Meter,
    sp: Span,
) -> R<Sized<Md::B>> {
    let Type::Set(t) = &domain.ty else {
        return no(Unlowerable::NonIntegerValue, sp);
    };
    let base = if is_scalar(t) { ty(budget).base(t) } else { 0 };
    let j = usize::try_from(i128::from(v) - i128::from(base)).unwrap_or(0);
    let whole = lay_expr::<Md>(domain, budget)?;
    let one = Md::l_select(budget, whole, j, 1);
    let Some((slot, _)) = Md::l_split(one) else {
        return no(Unlowerable::NonIntegerValue, sp);
    };
    cmp_i::<Md>(slot, CmpOp::Eq, 1, budget, sp)
}

// ---------------------------------------------------------------------------
// universes of composite parameters and binders
// ---------------------------------------------------------------------------

/// The universe of a composite type as atoms `0..card`: `None` for a type with no
/// finite universe (with its card), `Some(None)` for one too large to index in `i64`.
pub(super) fn composite_universe<Md: Mode>(
    t: &Type,
    budget: &mut Meter,
    span: Span,
) -> R<Result<Option<(i64, i64)>, Card>> {
    type_cost::<Md>(t, budget, span)?;
    Ok(match ty(budget).card(t) {
        Card::Finite(0) => Ok(Some((0, -1))),
        Card::Finite(n) => Ok(i64::try_from(n - 1).ok().map(|hi| (0, hi))),
        c => Err(c),
    })
}
