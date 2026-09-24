//! The pinned init path (bn-2ri63, RFC 0003 correction 6): the initial states of a
//! model whose init predicate pins its variables, found without enumerating the
//! product of the slot domains.
//!
//! # What it enumerates
//!
//! The lowered init predicate is a tree of `&&` over conjuncts. A conjunct *pins* a
//! variable `x` when it is `x == c`, `c == x`, or a `||` tree whose every leaf is such
//! an equality over the same `x`. For each variable, the first conjunct (left to right)
//! that pins it gives its candidate values: that conjunct's constants inside the
//! variable's domain, ascending, without repeats. A variable that no conjunct pins keeps
//! its whole domain. The candidates are the product of these axes, in the order the
//! whole-domain enumeration visits them (the last variable fastest, each axis
//! ascending). Every candidate is then checked exactly as the whole-domain enumeration
//! checks it: the canonicity constraint, then the predicate.
//!
//! # Why it is exact
//!
//! A state the predicate accepts satisfies every conjunct, because `&&` is true only
//! when both operands are true. So it lies on every axis: the candidates include every
//! accepted state, and the accepted states come out in the same order as from the
//! whole domain. The path applies only when no evaluation can fail at any state
//! ([`total`]): no arithmetic (the one operation whose failure depends on the state;
//! the lowering also writes a negative literal as `0 - n`, so an init with one takes
//! the whole domain, which is conservative),
//! every variable read of a declared variable, and every node within
//! `MAX_EXPR_DEPTH`, counted as the evaluator spends it. It also applies only when the
//! predicate has no definedness condition, since the whole-domain enumeration refuses
//! an undefined read at *any* canonical state (`cml.lower.undefined_read`), accepted or
//! not. Under those conditions the whole-domain enumeration raises no evaluation error
//! and no undefined-read refusal, and the builder receives the same initial states in
//! the same order, so `cml.lower.too_many_initial_states` and the builder's own errors
//! fall at the same state. Otherwise the lowering uses the whole-domain enumeration,
//! which is the oracle.
//!
//! # What it charges
//!
//! The analysis is planned before it runs. Its work — the totality scan over the
//! canonicity constraint and the predicate, the pin scan over the predicate, both at a
//! position lookup per node, and the sort of at most one value per predicate node — is
//! spent in one step before the first scan, from sizes the lowering already holds. The
//! position table is charged (an entry per variable, and its sort) before it is built.
//! The axes are charged (per variable a pin entry, an axis, a cursor, and a candidate
//! value, and two copies of each pinning leaf the totality scan counted) before the
//! pin scan allocates any of them.
//! The scan stacks are not charged: each holds at most one pending operand per level,
//! and the lowering builds no expression deeper than `MAX_EXPR_DEPTH`.
//! The enumeration that follows is charged by the caller exactly as the whole-domain
//! one is — [`super::init_work`] over the product of the axes, and the same
//! [`super::MAX_INIT_ENUMERATION`] bound — so no init shape reaches more candidates
//! than that bound.

use std::collections::BTreeMap;

use continuum_cml_syntax::Span;
use continuum_model_core::domain::Variable;
use continuum_model_core::expr::MAX_EXPR_DEPTH;
use continuum_model_core::{BoolExpr, CmpOp, IntExpr};

use super::{Meter, R, burn, charge};
use crate::budget::{lookup_cost, sort_cost};

/// What the last pinned analysis on this thread charged and held: the test seam that
/// shows every allocation stays within its precharge (cr-1s123d).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PinnedTrace {
    /// How many analyses built their axes on this thread.
    pub builds: u64,
    /// The output nodes used right after the axes were charged, before any was
    /// allocated.
    pub nodes_after_charge: usize,
    /// The values charged: two per pinning leaf.
    pub charged_values: usize,
    /// The capacity the scratch list and the axes' value lists held.
    pub held_values: usize,
    /// The per-variable entries charged: four per variable.
    pub charged_entries: usize,
    /// The capacity the pin table (one per variable), the axes, and the cursor held.
    pub held_entries: usize,
}

thread_local! {
    static TRACE: std::cell::Cell<PinnedTrace> = const {
        std::cell::Cell::new(PinnedTrace {
            builds: 0,
            nodes_after_charge: 0,
            charged_values: 0,
            held_values: 0,
            charged_entries: 0,
            held_entries: 0,
        })
    };
}

/// The [`PinnedTrace`] of the calling thread.
#[must_use]
pub fn pinned_trace_on_this_thread() -> PinnedTrace {
    TRACE.with(std::cell::Cell::get)
}

/// One variable's candidate values, in ascending order.
#[derive(Debug)]
enum Axis {
    /// The whole domain `lo..=hi`: no conjunct pins the variable.
    Range(i64, i64),
    /// The pinned values inside the domain, ascending, without repeats (maybe none).
    Values(Vec<i64>),
}

impl Axis {
    fn len(&self) -> u128 {
        match self {
            Self::Range(lo, hi) => (i128::from(*hi) - i128::from(*lo) + 1).max(0) as u128,
            Self::Values(v) => v.len() as u128,
        }
    }

    /// The `i`-th value. Called only for `i < len()`, which fits `usize` because the
    /// product of the lengths is at most `MAX_INIT_ENUMERATION` whenever it is called.
    fn at(&self, i: usize) -> i64 {
        match self {
            Self::Range(lo, _) => lo.saturating_add(i as i64),
            Self::Values(v) => v.get(i).copied().unwrap_or(i64::MAX),
        }
    }
}

/// The candidates of the pinned path: one axis per variable, in declaration order.
#[derive(Debug)]
pub(super) struct Space {
    axes: Vec<Axis>,
    cursor: Vec<usize>,
}

impl Space {
    /// How many candidates the space holds, saturating.
    pub(super) fn cardinality(&self) -> u128 {
        self.axes
            .iter()
            .fold(1_u128, |acc, a| acc.saturating_mul(a.len()))
    }

    /// The first candidate, or `None` when the space is empty.
    pub(super) fn start(&mut self) -> Option<Vec<i64>> {
        if self.cardinality() == 0 {
            return None;
        }
        self.cursor.iter_mut().for_each(|c| *c = 0);
        // One value per axis, allocated at its exact size (charged with the axes).
        let mut values = Vec::with_capacity(self.axes.len());
        values.extend(self.axes.iter().map(|a| a.at(0)));
        Some(values)
    }

    /// Step `values` to the next candidate, the last axis fastest: the order in which
    /// `super::advance` visits the whole domain. Returns `false` after the last one.
    pub(super) fn advance(&mut self, values: &mut [i64]) -> bool {
        for ((slot, axis), cursor) in values
            .iter_mut()
            .zip(&self.axes)
            .zip(self.cursor.iter_mut())
            .rev()
        {
            let next = cursor.saturating_add(1);
            if (next as u128) < axis.len() {
                *cursor = next;
                *slot = axis.at(next);
                return true;
            }
            *cursor = 0;
            *slot = axis.at(0);
        }
        false
    }
}

/// The work of the analysis, spent before it starts: two scans at `visit` units per
/// node — the totality scan over `scanned` nodes (canonicity and predicate), the pin
/// scan over the predicate's `predicate` nodes — and the sort of at most one value per
/// predicate node. Saturates.
pub(super) fn analysis_work(scanned: usize, predicate: usize, visit: u64) -> u64 {
    (scanned as u64)
        .saturating_add(predicate as u64)
        .saturating_mul(visit)
        .saturating_add(sort_cost(predicate, predicate.saturating_mul(8)))
}

/// The pinned space of `predicate` over `variables`, or `None` when the path does not
/// apply ([`total`] fails for the predicate or for the canonicity constraint `canon`),
/// in which case the caller enumerates the whole domain. `predicate_size` and
/// `canon_size` are upper bounds on the node counts of the two expressions. The caller
/// has checked that the predicate has no definedness condition.
#[allow(clippy::too_many_arguments)]
pub(super) fn space(
    predicate: &BoolExpr,
    predicate_size: usize,
    canon: Option<&BoolExpr>,
    canon_size: usize,
    variables: &[Variable],
    budget: &mut Meter,
    span: Span,
) -> R<Option<Space>> {
    // The position table: an entry per variable (a borrowed name, never copied, and an
    // index), its build a sort over the names, charged before it is built.
    let names: usize = variables.iter().map(|v| v.name().as_str().len()).sum();
    let widest = variables
        .iter()
        .map(|v| v.name().as_str().len())
        .max()
        .unwrap_or(0);
    charge(budget, variables.len(), span)?;
    burn(budget, sort_cost(variables.len(), names), span)?;
    // The plan: both scans and the sort, spent before the first scan. A node visit is a
    // stack step, a match, and at most one position lookup.
    let visit = lookup_cost(variables.len(), widest).saturating_add(2);
    let scanned = predicate_size.saturating_add(if canon.is_some() { canon_size } else { 0 });
    burn(budget, analysis_work(scanned, predicate_size, visit), span)?;
    let positions: BTreeMap<&str, usize> = variables
        .iter()
        .enumerate()
        .map(|(i, v)| (v.name().as_str(), i))
        .collect();
    // A repeated name (a hand-built model may declare one) would pin one variable while
    // the evaluator reads another (`Environment::value_of` takes the first): the path
    // does not apply, and the builder reports the repetition on the whole domain.
    if positions.len() != variables.len() {
        return Ok(None);
    }

    // Totality: the canonicity constraint and the predicate raise no evaluation error
    // at any state. The predicate's scan also counts its pinning leaves, which bound
    // the values the pin scan copies.
    if let Some(c) = canon
        && total(c, &positions).is_none()
    {
        return Ok(None);
    }
    let Some(leaves) = total(predicate, &positions) else {
        return Ok(None);
    };

    // The axes: per variable a pin entry, an axis, a cursor, and a candidate value
    // (the enumeration's value vector), and at most `leaves` values held twice (the
    // conjunct's scratch list and the axis it fills), charged before any is allocated.
    charge(
        budget,
        variables
            .len()
            .saturating_mul(4)
            .saturating_add(leaves.saturating_mul(2)),
        span,
    )?;
    let nodes_after_charge = budget.nodes.used();
    // Every vector below is allocated at a capacity fixed before it is filled, never
    // grown: `pinned`, `axes`, and `cursor` at one entry per variable; `scratch` at
    // `leaves`, and a conjunct pushes at most its own leaves into it; each axis's values
    // at its conjunct's scratch length, and the conjuncts' leaves are disjoint, so the
    // axes hold at most `leaves` values together (cr-1s123d).
    let mut pinned: Vec<Option<Vec<i64>>> = Vec::with_capacity(variables.len());
    pinned.resize_with(variables.len(), || None);
    let mut scratch: Vec<i64> = Vec::with_capacity(leaves);
    let mut stack: Vec<&BoolExpr> = vec![predicate];
    while let Some(e) = stack.pop() {
        if let BoolExpr::And(left, right) = e {
            stack.push(right);
            stack.push(left);
            continue;
        }
        scratch.clear();
        let Some(name) = pins(e, &mut scratch) else {
            continue;
        };
        let Some(&at) = positions.get(name) else {
            continue;
        };
        let Some(slot) = pinned.get_mut(at) else {
            continue;
        };
        if slot.is_some() {
            continue;
        }
        let domain = variables[at].domain();
        let mut values: Vec<i64> = Vec::with_capacity(scratch.len());
        values.extend(
            scratch
                .iter()
                .copied()
                .filter(|x| domain.lo() <= *x && *x <= domain.hi()),
        );
        values.sort_unstable();
        values.dedup();
        *slot = Some(values);
    }
    let held_values = scratch.capacity().saturating_add(
        pinned
            .iter()
            .flatten()
            .map(Vec::capacity)
            .fold(0_usize, usize::saturating_add),
    );
    let mut axes: Vec<Axis> = Vec::with_capacity(variables.len());
    axes.extend(variables.iter().zip(pinned).map(|(v, p)| match p {
        Some(values) => Axis::Values(values),
        None => Axis::Range(v.domain().lo(), v.domain().hi()),
    }));
    let cursor = vec![0; axes.len()];
    TRACE.with(|t| {
        let mut now = t.get();
        now.builds = now.builds.saturating_add(1);
        now.nodes_after_charge = nodes_after_charge;
        now.charged_values = leaves.saturating_mul(2);
        now.held_values = held_values;
        now.charged_entries = variables.len().saturating_mul(4);
        now.held_entries = axes
            .capacity()
            .saturating_add(cursor.capacity())
            .saturating_add(variables.len());
        t.set(now);
    });
    Ok(Some(Space { axes, cursor }))
}

/// `x == c` or `c == x`: the variable and the constant.
fn pin_leaf(e: &BoolExpr) -> Option<(&str, i64)> {
    match e {
        BoolExpr::Compare {
            op: CmpOp::Eq,
            left,
            right,
        } => match (left, right) {
            (IntExpr::Var(x), IntExpr::Const(c)) | (IntExpr::Const(c), IntExpr::Var(x)) => {
                Some((x.as_str(), *c))
            }
            _ => None,
        },
        _ => None,
    }
}

/// The variable a conjunct pins, with its constants appended to `out`: a pinning leaf,
/// or a `||` tree whose every leaf is a pinning leaf over one variable. `None` for any
/// other conjunct (whatever `out` then holds is discarded by the caller).
fn pins<'e>(e: &'e BoolExpr, out: &mut Vec<i64>) -> Option<&'e str> {
    let mut name: Option<&str> = None;
    let mut stack: Vec<&BoolExpr> = vec![e];
    while let Some(node) = stack.pop() {
        if let BoolExpr::Or(left, right) = node {
            stack.push(right);
            stack.push(left);
            continue;
        }
        let (x, c) = pin_leaf(node)?;
        match name {
            None => name = Some(x),
            Some(n) if n == x => {}
            Some(_) => return None,
        }
        out.push(c);
    }
    name
}

/// A node of a lowered expression, for the iterative scan.
enum Node<'a> {
    Bool(&'a BoolExpr),
    Int(&'a IntExpr),
}

/// Whether evaluating `e` can fail at no state over the variables of `positions`:
/// `Some(n)`, with `n` the number of pinning leaves ([`pin_leaf`]) in `e`, when there is
/// no arithmetic node, every variable read names a declared variable, and every node
/// lies within `MAX_EXPR_DEPTH` counted as the evaluator counts it (the root at one, and
/// each operand one below its node). Those are the three conditions of the evaluator's
/// three errors: `Overflow`, `Unbound`, and `TooDeep`. Iterative; the stack holds at
/// most one pending operand per level.
fn total(e: &BoolExpr, positions: &BTreeMap<&str, usize>) -> Option<usize> {
    let mut leaves = 0_usize;
    let mut stack: Vec<(Node<'_>, usize)> = vec![(Node::Bool(e), 1)];
    while let Some((node, depth)) = stack.pop() {
        if depth > MAX_EXPR_DEPTH {
            return None;
        }
        let below = depth.saturating_add(1);
        match node {
            Node::Bool(b) => match b {
                BoolExpr::Const(_) => {}
                BoolExpr::Compare { left, right, .. } => {
                    if pin_leaf(b).is_some() {
                        leaves = leaves.saturating_add(1);
                    }
                    stack.push((Node::Int(left), below));
                    stack.push((Node::Int(right), below));
                }
                BoolExpr::Not(inner) => stack.push((Node::Bool(inner), below)),
                BoolExpr::And(left, right)
                | BoolExpr::Or(left, right)
                | BoolExpr::Implies(left, right) => {
                    stack.push((Node::Bool(left), below));
                    stack.push((Node::Bool(right), below));
                }
                BoolExpr::InRange { expr, .. } => stack.push((Node::Int(expr), below)),
            },
            Node::Int(i) => match i {
                IntExpr::Const(_) => {}
                IntExpr::Var(name) => {
                    if !positions.contains_key(name.as_str()) {
                        return None;
                    }
                }
                IntExpr::Arith(..) => return None,
                IntExpr::Min(left, right) | IntExpr::Max(left, right) => {
                    stack.push((Node::Int(left), below));
                    stack.push((Node::Int(right), below));
                }
            },
        }
    }
    Some(leaves)
}

#[cfg(test)]
mod tests {
    use super::*;
    use continuum_model_core::domain::Domain;
    use continuum_model_core::{ArithOp, Ident};

    fn var(name: &str, lo: i64, hi: i64) -> Variable {
        Variable::new(
            Ident::new(name).expect("name"),
            Domain::new(lo, hi).expect("domain"),
        )
    }

    fn eq(x: &str, c: i64) -> BoolExpr {
        BoolExpr::compare(CmpOp::Eq, IntExpr::var(x), IntExpr::Const(c))
    }

    fn positions(vars: &[Variable]) -> BTreeMap<&str, usize> {
        vars.iter()
            .enumerate()
            .map(|(i, v)| (v.name().as_str(), i))
            .collect()
    }

    #[test]
    fn arithmetic_unknown_names_and_depth_are_not_total() {
        let vars = [var("x", 0, 3)];
        let p = positions(&vars);
        assert_eq!(total(&eq("x", 1), &p), Some(1));
        let arith = BoolExpr::compare(
            CmpOp::Eq,
            IntExpr::Arith(
                ArithOp::Add,
                Box::new(IntExpr::var("x")),
                Box::new(IntExpr::Const(1)),
            ),
            IntExpr::Const(2),
        );
        assert_eq!(total(&arith, &p), None);
        assert_eq!(total(&eq("y", 1), &p), None);
        // A chain of `!` one level past what the evaluator admits.
        let mut deep = BoolExpr::Const(true);
        for _ in 0..MAX_EXPR_DEPTH {
            deep = BoolExpr::Not(Box::new(deep));
        }
        assert_eq!(total(&deep, &p), None);
        let mut ok = BoolExpr::Const(true);
        for _ in 1..MAX_EXPR_DEPTH {
            ok = BoolExpr::Not(Box::new(ok));
        }
        assert_eq!(total(&ok, &p), Some(0));
    }

    #[test]
    fn pins_take_one_variable_per_disjunction() {
        let mut out = Vec::new();
        let or = BoolExpr::Or(Box::new(eq("x", 2)), Box::new(eq("x", 0)));
        assert_eq!(pins(&or, &mut out), Some("x"));
        assert_eq!(out, vec![2, 0]);
        out.clear();
        let mixed = BoolExpr::Or(Box::new(eq("x", 2)), Box::new(eq("y", 0)));
        assert_eq!(pins(&mixed, &mut out), None);
        out.clear();
        let ne = BoolExpr::compare(CmpOp::Ne, IntExpr::var("x"), IntExpr::Const(0));
        assert_eq!(pins(&ne, &mut out), None);
    }

    #[test]
    fn the_space_steps_in_whole_domain_order() {
        let mut space = Space {
            axes: vec![Axis::Values(vec![1, 3]), Axis::Range(0, 1)],
            cursor: vec![0, 0],
        };
        assert_eq!(space.cardinality(), 4);
        let mut v = space.start().expect("nonempty");
        let mut seen = vec![v.clone()];
        while space.advance(&mut v) {
            seen.push(v.clone());
        }
        assert_eq!(seen, vec![vec![1, 0], vec![1, 1], vec![3, 0], vec![3, 1]]);
        let mut empty = Space {
            axes: vec![Axis::Values(vec![]), Axis::Range(0, 1)],
            cursor: vec![0, 0],
        };
        assert_eq!(empty.cardinality(), 0);
        assert!(empty.start().is_none());
    }
}
