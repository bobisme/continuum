//! Finite types, their universes, canonical value text, and flat layouts (RFC 0003
//! correction 4, "Finite types" and "Layout", bn-23hzh).
//!
//! Everything here is a pure function of a type and the lowering's tables (the
//! instantiated sorts, the configuration bounds, the enumerations). A value of a
//! finite composite type is identified by its *index* in the canonical order of its
//! universe `U(T)`; a value of a scalar type by its *code* (`false`/`true` as `0`/`1`,
//! a sort element or variant by its position, an integer by itself). The *atom* of a
//! value is its code for a scalar and its index for a composite: it is what an action
//! parameter or a quantifier binder holds.
//!
//! Nothing here allocates except the text it is asked to write. Recursion follows the
//! type, whose depth the caller has checked against [`crate::budget::MAX_TYPE_DEPTH`]
//! first. A loop over the members of a universe runs only where the caller has bounded
//! the universe (a slot count, an instance count, a text cap), and every value node the
//! text functions visit calls `step` first, so the caller spends its work as it runs.

use std::collections::BTreeMap;

use super::{Bindings, EnumTable};
use crate::types::Type;

/// The cardinality of a type's universe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Card {
    /// Finite, saturating at `u128::MAX`.
    Finite(u128),
    /// An `Int`, `Nat`, or sort that this lowering has not bounded or instantiated.
    Unbounded,
    /// Not a finite type in this correction: a string, sequence, record, or function.
    NotFinite,
}

/// The tables a type's universe depends on.
#[derive(Clone, Copy)]
pub(super) struct Ty<'m> {
    pub(super) bind: &'m Bindings,
    pub(super) enums: &'m BTreeMap<String, EnumTable>,
}

/// Whether `ty` is a scalar type: one slot, holding its code.
pub(super) fn is_scalar(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Bool | Type::Int | Type::Nat | Type::Sort(_) | Type::Enum(_)
    )
}

/// `base^exp`, saturating. At most 128 multiplications: a base of two or more
/// saturates `u128` by then, and bases `0` and `1` are answered directly.
pub(super) fn pow_sat(base: u128, exp: u128) -> u128 {
    match base {
        _ if exp == 0 => 1,
        0 => 0,
        1 => 1,
        _ => {
            let mut out: u128 = 1;
            let mut i: u128 = 0;
            while i < exp {
                out = out.saturating_mul(base);
                if out == u128::MAX {
                    break;
                }
                i += 1;
            }
            out
        }
    }
}

impl Ty<'_> {
    /// The codes of a scalar type, `lo..=hi` (empty when `hi < lo`), or why it has none.
    pub(super) fn range(&self, ty: &Type) -> Result<(i64, i64), Card> {
        match ty {
            Type::Bool => Ok((0, 1)),
            Type::Nat => self
                .bind
                .bounds
                .nat_max()
                .map(|m| (0, m))
                .ok_or(Card::Unbounded),
            Type::Int => self.bind.bounds.int_range().ok_or(Card::Unbounded),
            Type::Sort(s) => self
                .bind
                .sorts
                .get(s)
                .map(|n| (0, n.saturating_sub(1)))
                .ok_or(Card::Unbounded),
            Type::Enum(e) => self
                .enums
                .get(e)
                .map(|t| (0, t.names.len() as i64 - 1))
                .ok_or(Card::NotFinite),
            _ => Err(Card::NotFinite),
        }
    }

    /// The lowest code of a scalar type: its index `0`.
    pub(super) fn base(&self, ty: &Type) -> i64 {
        self.range(ty).map_or(0, |r| r.0)
    }

    /// The cardinality of `U(ty)`. Unbounded wins over finite, and not-finite over both.
    pub(super) fn card(&self, ty: &Type) -> Card {
        if is_scalar(ty) {
            return match self.range(ty) {
                Ok((lo, hi)) => Card::Finite(super::span_count(lo, hi)),
                Err(c) => c,
            };
        }
        let join = |a: Card, b: Card, f: &dyn Fn(u128, u128) -> u128| match (a, b) {
            (Card::NotFinite, _) | (_, Card::NotFinite) => Card::NotFinite,
            (Card::Unbounded, _) | (_, Card::Unbounded) => Card::Unbounded,
            (Card::Finite(x), Card::Finite(y)) => Card::Finite(f(x, y)),
        };
        match ty {
            Type::Option(t) => join(self.card(t), Card::Finite(1), &|x, y| x.saturating_add(y)),
            Type::Tuple(ts) => ts.iter().fold(Card::Finite(1), |acc, t| {
                join(acc, self.card(t), &|x, y| x.saturating_mul(y))
            }),
            Type::Set(t) => join(self.card(t), Card::Finite(0), &|x, _| pow_sat(2, x)),
            Type::Map(k, v) => join(self.card(k), self.card(v), &|x, y| {
                pow_sat(y.saturating_add(1), x)
            }),
            _ => Card::NotFinite,
        }
    }

    /// The cardinality of a type already known to be finite (`0` otherwise).
    pub(super) fn count(&self, ty: &Type) -> u128 {
        match self.card(ty) {
            Card::Finite(n) => n,
            _ => 0,
        }
    }

    /// The number of slots of a finite type's layout, saturating.
    pub(super) fn width(&self, ty: &Type) -> u128 {
        match ty {
            _ if is_scalar(ty) => 1,
            Type::Option(t) => self.opt_width(t),
            Type::Tuple(ts) => ts
                .iter()
                .fold(0_u128, |a, t| a.saturating_add(self.width(t))),
            Type::Set(t) => self.count(t),
            Type::Map(k, v) => self.count(k).saturating_mul(self.opt_width(v)),
            _ => 0,
        }
    }

    /// The width of `Option[v]`: one slot over a scalar, a presence slot and `v`'s
    /// layout otherwise.
    pub(super) fn opt_width(&self, v: &Type) -> u128 {
        if is_scalar(v) {
            1
        } else {
            self.width(v).saturating_add(1)
        }
    }

    /// The domain `(lo, hi)` of slot `j` of `ty`'s layout; `j` is below the width.
    pub(super) fn slot_domain(&self, ty: &Type, j: u128) -> (i64, i64) {
        match ty {
            _ if is_scalar(ty) => self.range(ty).unwrap_or((0, -1)),
            Type::Option(t) => self.opt_slot_domain(t, j),
            Type::Tuple(ts) => {
                let mut j = j;
                for t in ts {
                    let w = self.width(t);
                    if j < w {
                        return self.slot_domain(t, j);
                    }
                    j -= w;
                }
                (0, -1)
            }
            Type::Set(_) => (0, 1),
            Type::Map(_, v) => self.opt_slot_domain(v, j % self.opt_width(v).max(1)),
            _ => (0, -1),
        }
    }

    /// [`Ty::slot_domain`] of `Option[v]`.
    fn opt_slot_domain(&self, v: &Type, j: u128) -> (i64, i64) {
        if is_scalar(v) {
            (0, i64::try_from(self.count(v)).unwrap_or(i64::MAX))
        } else if j == 0 {
            (0, 1)
        } else {
            self.slot_domain(v, j - 1)
        }
    }

    /// The index of component `i` of the tuple value with index `idx`: the tuple order
    /// is lexicographic, so the last component varies fastest.
    fn tuple_part(&self, ts: &[Type], idx: u128, i: usize) -> u128 {
        let after = ts
            .iter()
            .skip(i.saturating_add(1))
            .fold(1_u128, |a, t| a.saturating_mul(self.count(t)));
        let c = ts.get(i).map_or(1, |t| self.count(t)).max(1);
        (idx / after.max(1)) % c
    }

    /// The code of slot `j` of the layout of the value of `ty` with index `idx`.
    pub(super) fn code_at(&self, ty: &Type, idx: u128, j: u128) -> i64 {
        match ty {
            _ if is_scalar(ty) => {
                let lo = self.base(ty);
                i64::try_from(i128::from(lo).saturating_add(i128::try_from(idx).unwrap_or(0)))
                    .unwrap_or(lo)
            }
            Type::Option(t) => self.opt_code_at(t, idx, j),
            Type::Tuple(ts) => {
                let mut j = j;
                for (i, t) in ts.iter().enumerate() {
                    let w = self.width(t);
                    if j < w {
                        return self.code_at(t, self.tuple_part(ts, idx, i), j);
                    }
                    j -= w;
                }
                0
            }
            Type::Set(t) => {
                let n = self.count(t);
                // The characteristic vector, first member most significant.
                let shift = n.saturating_sub(1).saturating_sub(j);
                i64::from(shift < 128 && (idx >> shift) & 1 == 1)
            }
            Type::Map(k, v) => {
                let n = self.count(k);
                let w = self.opt_width(v).max(1);
                let base = self.count(v).saturating_add(1);
                let key = j / w;
                let place = pow_sat(base, n.saturating_sub(1).saturating_sub(key)).max(1);
                self.opt_code_at(v, (idx / place) % base.max(1), j % w)
            }
            _ => 0,
        }
    }

    /// [`Ty::code_at`] of `Option[v]`: `0` is `None`, `i + 1` is `Some` of the `i`-th.
    fn opt_code_at(&self, v: &Type, idx: u128, j: u128) -> i64 {
        if is_scalar(v) {
            return i64::try_from(idx).unwrap_or(i64::MAX);
        }
        match (idx, j) {
            (0, 0) => 0,
            (0, _) => self.slot_domain(v, j - 1).0,
            (_, 0) => 1,
            (_, _) => self.code_at(v, idx - 1, j - 1),
        }
    }

    /// The index of the value whose layout is `codes`, or `None` when a code is outside
    /// its slot's domain, the layout is not canonical (a slot under an absent entry
    /// that is not at its minimum), or `codes` is not the type's width.
    pub(super) fn index_of(&self, ty: &Type, codes: &[i64]) -> Option<u128> {
        match ty {
            _ if is_scalar(ty) => {
                let (lo, hi) = self.range(ty).ok()?;
                let [c] = codes else { return None };
                (lo <= *c && *c <= hi).then(|| (i128::from(*c) - i128::from(lo)) as u128)
            }
            Type::Option(t) => self.opt_index_of(t, codes),
            Type::Tuple(ts) => {
                let mut at = 0_usize;
                let mut idx: u128 = 0;
                for t in ts {
                    let w = usize::try_from(self.width(t)).ok()?;
                    let part = self.index_of(t, codes.get(at..at.checked_add(w)?)?)?;
                    idx = idx.checked_mul(self.count(t))?.checked_add(part)?;
                    at += w;
                }
                (at == codes.len()).then_some(idx)
            }
            Type::Set(t) => {
                if codes.len() as u128 != self.count(t) {
                    return None;
                }
                let mut idx: u128 = 0;
                for c in codes {
                    if !(0..=1).contains(c) {
                        return None;
                    }
                    idx = idx.checked_mul(2)?.checked_add(*c as u128)?;
                }
                Some(idx)
            }
            Type::Map(k, v) => {
                let n = usize::try_from(self.count(k)).ok()?;
                let w = usize::try_from(self.opt_width(v)).ok()?;
                let base = self.count(v).checked_add(1)?;
                if Some(codes.len()) != n.checked_mul(w) {
                    return None;
                }
                let mut idx: u128 = 0;
                for key in 0..n {
                    let at = key.checked_mul(w)?;
                    let d = self.opt_index_of(v, codes.get(at..at.checked_add(w)?)?)?;
                    idx = idx.checked_mul(base)?.checked_add(d)?;
                }
                Some(idx)
            }
            _ => None,
        }
    }

    /// [`Ty::index_of`] of `Option[v]`.
    fn opt_index_of(&self, v: &Type, codes: &[i64]) -> Option<u128> {
        if is_scalar(v) {
            let [c] = codes else { return None };
            return (*c >= 0 && (*c as u128) <= self.count(v)).then_some(*c as u128);
        }
        let (p, inner) = codes.split_first()?;
        match p {
            0 => {
                if inner.len() as u128 != self.width(v) {
                    return None;
                }
                for (j, c) in inner.iter().enumerate() {
                    if *c != self.slot_domain(v, j as u128).0 {
                        return None;
                    }
                }
                Some(0)
            }
            1 => self.index_of(v, inner).and_then(|i| i.checked_add(1)),
            _ => None,
        }
    }

    /// The canonical text of the value of `ty` with index `idx` (RFC 0003 correction 4,
    /// "Layout"), appended to `out` when it is given; returns its length in bytes. A
    /// sort element is escaped ([`super::escape_label`]), so the text is injective for
    /// its type and self-delimiting inside the brackets of a slot name or a label. Every
    /// value node writes at least one byte and calls `step` first; `None` when `step`
    /// refuses.
    pub(super) fn text(
        &self,
        ty: &Type,
        idx: u128,
        out: &mut Option<&mut String>,
        step: &mut dyn FnMut() -> bool,
    ) -> Option<usize> {
        if !step() {
            return None;
        }
        let put = |s: &str, out: &mut Option<&mut String>| {
            if let Some(o) = out.as_deref_mut() {
                o.push_str(s);
            }
            s.len()
        };
        Some(match ty {
            Type::Bool => put(if idx == 0 { "false" } else { "true" }, out),
            Type::Nat | Type::Int => {
                let v = i128::from(self.base(ty)).saturating_add(i128::try_from(idx).ok()?);
                put(&v.to_string(), out)
            }
            Type::Sort(s) => {
                let name = self
                    .bind
                    .sort_names
                    .get(s)
                    .and_then(|(names, _)| names.get(usize::try_from(idx).ok()?))?;
                if let Some(o) = out.as_deref_mut() {
                    o.push_str(&super::escape_label(name));
                }
                super::escaped_len(name)
            }
            Type::Enum(e) => {
                // Escaped like a sort element: an elaborated variant is an identifier and
                // unchanged, a hand-built one keeps the text injective.
                let name = self
                    .enums
                    .get(e)
                    .and_then(|t| t.names.get(usize::try_from(idx).ok()?))?;
                if let Some(o) = out.as_deref_mut() {
                    o.push_str(&super::escape_label(name));
                }
                super::escaped_len(name)
            }
            Type::Option(t) => {
                if idx == 0 {
                    put("None", out)
                } else {
                    let a = put("Some(", out);
                    let b = self.text(t, idx - 1, out, step)?;
                    a + b + put(")", out)
                }
            }
            Type::Tuple(ts) => {
                // One pass: the place value of component `i` is the product of the
                // counts after it, the whole product divided down component by
                // component (exact: an index is only taken of a type whose universe
                // fits, and every count of such a type is at least one).
                let mut place = ts
                    .iter()
                    .fold(1_u128, |a, t| a.saturating_mul(self.count(t)));
                let mut len = put("(", out);
                for (i, t) in ts.iter().enumerate() {
                    if i > 0 {
                        len += put(",", out);
                    }
                    let c = self.count(t).max(1);
                    place = (place / c).max(1);
                    len += self.text(t, (idx / place) % c, out, step)?;
                }
                len + put(")", out)
            }
            Type::Set(t) => {
                let n = self.count(t);
                // A member's bit is at `n - 1 - j`, so only the last 128 elements of the
                // universe can be members of an index that fits `u128`: the loop starts
                // there, and its length is at most 128 whatever `n` is.
                let first = n.saturating_sub(128);
                let mut len = put("{", out);
                let mut any = false;
                for j in first..n {
                    let shift = n - 1 - j;
                    if (idx >> shift) & 1 == 1 {
                        if any {
                            len += put(",", out);
                        }
                        any = true;
                        len += self.text(t, j, out, step)?;
                    }
                }
                len + put("}", out)
            }
            Type::Map(k, v) => {
                let n = self.count(k);
                let base = self.count(v).saturating_add(1);
                let mut len = put("[", out);
                let mut any = false;
                // With a base of two or more, only the last 128 keys can hold a digit of
                // an index that fits `u128`; with base one every digit is `0`.
                let first = if base >= 2 { n.saturating_sub(128) } else { n };
                for key in first..n {
                    let place = pow_sat(base, n - 1 - key).max(1);
                    let digit = (idx / place) % base.max(1);
                    if digit == 0 {
                        continue;
                    }
                    if any {
                        len += put(",", out);
                    }
                    any = true;
                    len += self.text(k, key, out, step)?;
                    len += put(":", out);
                    len += self.text(v, digit - 1, out, step)?;
                }
                len + put("]", out)
            }
            _ => return None,
        })
    }

    /// The length of the longest canonical text of a value of `ty`, or `cap + 1` when
    /// it is longer than `cap`. Exact up to `cap`: the widest set is the full set, the
    /// widest map has every key at its widest value, the widest option is `Some` of the
    /// widest value, and a tuple's components are independent. A set or map over more
    /// than `cap` elements is longer than `cap` (one separator each), so the members of
    /// a universe are visited only when there are at most `cap` of them. Every node
    /// visited calls `step` first.
    pub(super) fn widest(
        &self,
        ty: &Type,
        cap: usize,
        step: &mut dyn FnMut() -> bool,
    ) -> Option<usize> {
        if !step() {
            return None;
        }
        let over = cap.saturating_add(1);
        let w = match ty {
            Type::Bool => 5,
            Type::Nat | Type::Int => {
                let (lo, hi) = self.range(ty).ok()?;
                lo.to_string().len().max(hi.to_string().len())
            }
            Type::Sort(s) => self.bind.sort_names.get(s).map_or(0, |(_, w)| *w),
            Type::Enum(e) => self.enums.get(e).map_or(0, |t| t.widest),
            Type::Option(t) => 4_usize.max(self.widest(t, cap, step)?.saturating_add(6)),
            Type::Tuple(ts) => {
                let mut len = ts.len().saturating_add(1);
                for t in ts {
                    len = len.saturating_add(self.widest(t, cap, step)?);
                    if len > cap {
                        return Some(over);
                    }
                }
                len
            }
            Type::Set(t) => {
                let n = self.count(t);
                if n > cap as u128 {
                    return Some(over);
                }
                let mut len = 2_usize.saturating_add((n as usize).saturating_sub(1));
                for j in 0..n {
                    len = len.saturating_add(self.text(t, j, &mut None, step)?);
                    if len > cap {
                        return Some(over);
                    }
                }
                len
            }
            Type::Map(k, v) => {
                let n = self.count(k);
                if n > cap as u128 {
                    return Some(over);
                }
                let wv = self.widest(v, cap, step)?;
                let mut len = 2_usize.saturating_add((n as usize).saturating_sub(1));
                for key in 0..n {
                    len = len
                        .saturating_add(self.text(k, key, &mut None, step)?)
                        .saturating_add(1)
                        .saturating_add(wv);
                    if len > cap {
                        return Some(over);
                    }
                }
                len
            }
            _ => return None,
        };
        Some(w.min(over))
    }
}
