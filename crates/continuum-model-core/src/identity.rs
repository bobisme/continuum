//! Semantic model identity: the canonical encoding of a [`Model`], compared exactly.
//!
//! Decision: ADR-0013 (content identity) and PR 15a (RFC 0003 front ends). PR 15a's exit
//! criterion is that a model "written in CML elaborates to the same semantic model
//! identity as its programmatic equivalent"
//! (`notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 15a). This module defines that
//! identity once, on the one model type both front ends produce.
//!
//! # What the identity is
//!
//! ADR-0013 makes canonical content identity primary in certified lanes: two artifacts
//! are the same artifact exactly when their canonical encodings are equal, and a digest
//! may only index. A [`ModelIdentity`] is therefore the canonical encoding itself. It
//! carries no hash, so no hash-vendor decision can change which models are "the same".
//!
//! # Why it is a function of meaning, not of declaration
//!
//! [`crate::model::ModelBuilder::build`] already discards declaration order: variables,
//! actions, predicates, initial states, and the assignments of one outcome are held in
//! byte-lexicographic canonical order. One sequence is still held in declaration order,
//! the outcomes of a nondeterministic action. The encoding sorts and deduplicates the
//! encoded outcomes, so two models that list the same outcomes in different orders have
//! one identity. Everything else is encoded in the order the model already holds.
//!
//! # Encoding
//!
//! ```text
//! identity   := "continuum-model/1" variables actions initials predicates [fairness]
//! variables  := count (token lo:i64 hi:i64)*
//! actions    := count (token bool outcomes)*
//! outcomes   := count (count (token int)*)*      -- sorted by encoding, no duplicates
//! initials   := count (count i64*)*
//! predicates := count (token bool)*
//! fairness   := count (strength count token*)*  -- only when count >= 1; see below
//! strength   := 0x20 (weak) | 0x21 (strong)
//! count      := u64 big-endian     token := count bytes     i64 := big-endian
//! int        := 0x01 i64 | 0x02 token | 0x03 op int int | 0x04 int int | 0x05 int int
//! bool       := 0x10 b | 0x11 cmp int int | 0x12 bool | 0x13 bool bool | 0x14 bool bool
//!             | 0x15 bool bool | 0x16 int lo:i64 hi:i64
//! ```
//!
//! Every field is length-prefixed or fixed-width, so the encoding is injective: two
//! different models cannot share one byte string.
//!
//! # Fairness (bn-1ln12)
//!
//! A model's fairness assumptions are part of what the model *means* (docs/02 §2's
//! \(F\)), so they are part of its identity: RFC 0015, "Adding a fairness assumption
//! changes the claim identity". The section is written after the predicates, and only
//! when the model declares at least one assumption: each assumption in canonical order
//! ([`crate::fairness`]), as its strength byte and its scope's action *names*, in
//! ascending order. A model without fairness therefore keeps, byte for byte, the
//! identity it had before fairness existed (`tests/identity_golden.rs` pins it).
//!
//! Injectivity still holds. The part before the section is self-delimiting (every field
//! is length-prefixed or fixed-width), so a reader knows where it ends; after it comes
//! either nothing (no assumption) or a count of at least one and that many
//! self-delimiting assumptions. Two models that differ in their fairness differ in
//! these bytes; two that differ elsewhere differ before them.

use crate::expr::{ArithOp, BoolExpr, CmpOp, IntExpr};
use crate::fairness::Strength;
use crate::model::Model;

/// The version tag every identity starts with.
pub const IDENTITY_TAG: &[u8] = b"continuum-model/1";

/// The semantic identity of a [`Model`]: its canonical encoding.
///
/// Equality, order, and hashing are those of the encoding bytes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModelIdentity {
    bytes: Vec<u8>,
}

impl ModelIdentity {
    /// The canonical encoding.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl Model {
    /// The semantic identity of this model.
    ///
    /// A pure function of the model's content. A model built through
    /// [`crate::model::ModelBuilder`] and a model elaborated from CML have one identity
    /// exactly when they are the same transition system with the same named predicates.
    #[must_use]
    pub fn identity(&self) -> ModelIdentity {
        // Every buffer is allocated once at its bound, so no buffer grows (and no
        // reallocation copies) while the encoding is written: the peak allocation is
        // exactly `identity_alloc_bound`.
        let (scratch_bytes, max_outcomes) = self.outcome_scratch();
        let mut out: Vec<u8> = Vec::with_capacity(self.identity_len_bound());
        out.extend_from_slice(IDENTITY_TAG);

        count(&mut out, self.variables().len());
        for variable in self.variables() {
            token(&mut out, variable.name().as_str());
            out.extend_from_slice(&variable.domain().lo().to_be_bytes());
            out.extend_from_slice(&variable.domain().hi().to_be_bytes());
        }

        // One action's outcomes are encoded into one reused scratch buffer, each as a
        // byte range, sorted and deduplicated by their bytes, and copied into `out`.
        // No per-outcome allocation: the scratch buffer and the range list are the
        // only allocations besides `out`, and `identity_alloc_bound` counts them.
        let mut scratch: Vec<u8> = Vec::with_capacity(scratch_bytes);
        let mut ranges: Vec<(usize, usize)> = Vec::with_capacity(max_outcomes);
        count(&mut out, self.actions().len());
        for action in self.actions() {
            token(&mut out, action.name().as_str());
            bool_expr(&mut out, action.guard());
            scratch.clear();
            ranges.clear();
            for outcome in action.outcomes() {
                let start = scratch.len();
                count(&mut scratch, outcome.assignments().len());
                for assignment in outcome.assignments() {
                    token(&mut scratch, assignment.variable().as_str());
                    int_expr(&mut scratch, assignment.value());
                }
                ranges.push((start, scratch.len()));
            }
            let bytes = |r: &(usize, usize)| scratch.get(r.0..r.1).unwrap_or_default();
            ranges.sort_by(|a, b| bytes(a).cmp(bytes(b)));
            ranges.dedup_by(|a, b| bytes(a) == bytes(b));
            count(&mut out, ranges.len());
            for r in &ranges {
                out.extend_from_slice(bytes(r));
            }
        }

        count(&mut out, self.initial_states().len());
        for state in self.initial_states() {
            count(&mut out, state.arity());
            for value in state.as_slice() {
                out.extend_from_slice(&value.to_be_bytes());
            }
        }

        count(&mut out, self.predicates().len());
        for predicate in self.predicates() {
            token(&mut out, predicate.name().as_str());
            bool_expr(&mut out, predicate.body());
        }

        if !self.fairness().is_empty() {
            count(&mut out, self.fairness().len());
            for assumption in self.fairness() {
                out.push(strength_byte(assumption.strength()));
                count(&mut out, assumption.actions().len());
                for index in assumption.actions() {
                    let name = self
                        .actions()
                        .get(*index)
                        .map_or("", |action| action.name().as_str());
                    token(&mut out, name);
                }
            }
        }

        ModelIdentity { bytes: out }
    }

    /// An upper bound on every byte [`Model::identity`] allocates: the encoding's
    /// length ([`Model::identity_len_bound`]), plus the scratch buffer one action's
    /// outcomes are encoded into (the largest action's outcomes), plus that action's
    /// range list (16 bytes per outcome). Allocation-free, one visit per node. A caller
    /// that meters allocations charges this before calling [`Model::identity`].
    #[must_use]
    pub fn identity_alloc_bound(&self) -> usize {
        let (scratch, outcomes) = self.outcome_scratch();
        self.identity_len_bound()
            .saturating_add(scratch)
            .saturating_add(outcomes.saturating_mul(16))
    }

    /// The largest action's outcome encodings in bytes, and the most outcomes of one
    /// action: the scratch buffer's and the range list's capacities.
    fn outcome_scratch(&self) -> (usize, usize) {
        let mut bytes = 0_usize;
        let mut outcomes = 0_usize;
        for action in self.actions() {
            let mut block = 0_usize;
            for outcome in action.outcomes() {
                block = block.saturating_add(8);
                for assignment in outcome.assignments() {
                    block = block
                        .saturating_add(8)
                        .saturating_add(assignment.variable().as_str().len())
                        .saturating_add(int_len(assignment.value()));
                }
            }
            bytes = bytes.max(block);
            outcomes = outcomes.max(action.outcomes().len());
        }
        (bytes, outcomes)
    }

    /// An upper bound on the length of [`Model::identity`]'s encoding, computed
    /// without allocating: every field the encoding writes, counted with the same
    /// widths, over every outcome before duplicates collapse (so the bound is exact for
    /// a model with no duplicate outcomes and larger otherwise). A caller that meters
    /// allocations charges this before calling [`Model::identity`].
    ///
    /// It visits each variable, action, outcome, assignment, initial-state value,
    /// predicate, fairness member, and expression node once; recursion is bounded as in
    /// [`Model::identity`]. Saturates.
    #[must_use]
    pub fn identity_len_bound(&self) -> usize {
        const COUNT: usize = 8;
        let token = |t: &str| COUNT.saturating_add(t.len());
        let mut n = IDENTITY_TAG.len().saturating_add(COUNT);
        for variable in self.variables() {
            n = n
                .saturating_add(token(variable.name().as_str()))
                .saturating_add(16);
        }
        n = n.saturating_add(COUNT);
        for action in self.actions() {
            n = n
                .saturating_add(token(action.name().as_str()))
                .saturating_add(bool_len(action.guard()))
                .saturating_add(COUNT);
            for outcome in action.outcomes() {
                n = n.saturating_add(COUNT);
                for assignment in outcome.assignments() {
                    n = n
                        .saturating_add(token(assignment.variable().as_str()))
                        .saturating_add(int_len(assignment.value()));
                }
            }
        }
        n = n.saturating_add(COUNT);
        for state in self.initial_states() {
            n = n
                .saturating_add(COUNT)
                .saturating_add(state.arity().saturating_mul(8));
        }
        n = n.saturating_add(COUNT);
        for predicate in self.predicates() {
            n = n
                .saturating_add(token(predicate.name().as_str()))
                .saturating_add(bool_len(predicate.body()));
        }
        if !self.fairness().is_empty() {
            n = n.saturating_add(COUNT);
            for assumption in self.fairness() {
                n = n.saturating_add(1).saturating_add(COUNT);
                for index in assumption.actions() {
                    let name = self
                        .actions()
                        .get(*index)
                        .map_or("", |action| action.name().as_str());
                    n = n.saturating_add(token(name));
                }
            }
        }
        n
    }
}

/// The byte that opens one fairness assumption's encoding.
const fn strength_byte(strength: Strength) -> u8 {
    match strength {
        Strength::Weak => 0x20,
        Strength::Strong => 0x21,
    }
}

/// The bytes [`int_expr`] writes for `expr`.
fn int_len(expr: &IntExpr) -> usize {
    match expr {
        IntExpr::Const(_) => 9,
        IntExpr::Var(name) => 9_usize.saturating_add(name.len()),
        IntExpr::Arith(_, left, right) => 2_usize
            .saturating_add(int_len(left))
            .saturating_add(int_len(right)),
        IntExpr::Min(left, right) | IntExpr::Max(left, right) => 1_usize
            .saturating_add(int_len(left))
            .saturating_add(int_len(right)),
    }
}

/// The bytes [`bool_expr`] writes for `expr`.
fn bool_len(expr: &BoolExpr) -> usize {
    match expr {
        BoolExpr::Const(_) => 2,
        BoolExpr::Compare { left, right, .. } => 2_usize
            .saturating_add(int_len(left))
            .saturating_add(int_len(right)),
        BoolExpr::Not(inner) => 1_usize.saturating_add(bool_len(inner)),
        BoolExpr::And(left, right) | BoolExpr::Or(left, right) | BoolExpr::Implies(left, right) => {
            1_usize
                .saturating_add(bool_len(left))
                .saturating_add(bool_len(right))
        }
        BoolExpr::InRange { expr, .. } => 17_usize.saturating_add(int_len(expr)),
    }
}

fn count(out: &mut Vec<u8>, n: usize) {
    out.extend_from_slice(&u64::try_from(n).unwrap_or(u64::MAX).to_be_bytes());
}

fn token(out: &mut Vec<u8>, text: &str) {
    count(out, text.len());
    out.extend_from_slice(text.as_bytes());
}

// Recursion depth is bounded: `ModelBuilder::build` refuses any expression deeper than
// `MAX_EXPR_DEPTH`, and a `Model` has no other constructor.
fn int_expr(out: &mut Vec<u8>, expr: &IntExpr) {
    match expr {
        IntExpr::Const(value) => {
            out.push(0x01);
            out.extend_from_slice(&value.to_be_bytes());
        }
        IntExpr::Var(name) => {
            out.push(0x02);
            token(out, name);
        }
        IntExpr::Arith(op, left, right) => {
            out.push(0x03);
            out.push(match op {
                ArithOp::Add => 0,
                ArithOp::Sub => 1,
                ArithOp::Mul => 2,
            });
            int_expr(out, left);
            int_expr(out, right);
        }
        IntExpr::Min(left, right) => {
            out.push(0x04);
            int_expr(out, left);
            int_expr(out, right);
        }
        IntExpr::Max(left, right) => {
            out.push(0x05);
            int_expr(out, left);
            int_expr(out, right);
        }
    }
}

fn bool_expr(out: &mut Vec<u8>, expr: &BoolExpr) {
    match expr {
        BoolExpr::Const(value) => {
            out.push(0x10);
            out.push(u8::from(*value));
        }
        BoolExpr::Compare { op, left, right } => {
            out.push(0x11);
            out.push(match op {
                CmpOp::Eq => 0,
                CmpOp::Ne => 1,
                CmpOp::Lt => 2,
                CmpOp::Le => 3,
                CmpOp::Gt => 4,
                CmpOp::Ge => 5,
            });
            int_expr(out, left);
            int_expr(out, right);
        }
        BoolExpr::Not(inner) => {
            out.push(0x12);
            bool_expr(out, inner);
        }
        BoolExpr::And(left, right) => {
            out.push(0x13);
            bool_expr(out, left);
            bool_expr(out, right);
        }
        BoolExpr::Or(left, right) => {
            out.push(0x14);
            bool_expr(out, left);
            bool_expr(out, right);
        }
        BoolExpr::Implies(left, right) => {
            out.push(0x15);
            bool_expr(out, left);
            bool_expr(out, right);
        }
        BoolExpr::InRange { expr, lo, hi } => {
            out.push(0x16);
            int_expr(out, expr);
            out.extend_from_slice(&lo.to_be_bytes());
            out.extend_from_slice(&hi.to_be_bytes());
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    /// The bound is exact without duplicate outcomes, and an upper bound with them.
    #[test]
    fn the_length_bound_is_exact_or_above() {
        let distinct = two_outcomes(1, 2);
        assert_eq!(
            distinct.identity_len_bound(),
            distinct.identity().as_bytes().len()
        );
        let duplicated = two_outcomes(1, 1);
        assert!(duplicated.identity_len_bound() > duplicated.identity().as_bytes().len());
    }

    use crate::expr::{BoolExpr, IntExpr};
    use crate::fairness::Strength;
    use crate::model::{ActionDecl, ModelBuilder};

    fn two_outcomes(first: i64, second: i64) -> crate::model::Model {
        ModelBuilder::new()
            .variable("x", 0, 3)
            .initial_state(&[("x", 0)])
            .action(ActionDecl::enumerated(
                "Pick",
                BoolExpr::Const(true),
                vec![
                    vec![("x", IntExpr::constant(first))],
                    vec![("x", IntExpr::constant(second))],
                ],
            ))
            .build()
            .unwrap()
    }

    fn two_actions() -> ModelBuilder {
        ModelBuilder::new()
            .variable("x", 0, 1)
            .initial_state(&[("x", 0)])
            .action(ActionDecl::deterministic(
                "A",
                BoolExpr::Const(true),
                vec![("x", IntExpr::constant(1))],
            ))
            .action(ActionDecl::deterministic(
                "B",
                BoolExpr::Const(true),
                vec![("x", IntExpr::constant(0))],
            ))
    }

    /// No fairness keeps the pre-fairness bytes: no trailing section at all.
    #[test]
    fn a_model_without_fairness_has_no_fairness_section() {
        let plain = two_actions().build().unwrap();
        let bytes = plain.identity();
        assert_eq!(plain.identity_len_bound(), bytes.as_bytes().len());
        let fair = two_actions()
            .fairness(Strength::Weak, ["A"])
            .build()
            .unwrap();
        let fair_bytes = fair.identity();
        assert!(fair_bytes.as_bytes().starts_with(bytes.as_bytes()));
        // count(1) strength count(1) token("A")
        assert_eq!(
            fair_bytes.as_bytes().len() - bytes.as_bytes().len(),
            8 + 1 + 8 + 8 + 1
        );
        assert_eq!(fair.identity_len_bound(), fair_bytes.as_bytes().len());
    }

    /// Strength, scope, and the split of a scope into assumptions are all identity.
    #[test]
    fn fairness_is_part_of_identity() {
        let ids = [
            two_actions().build().unwrap().identity(),
            two_actions()
                .fairness(Strength::Weak, ["A"])
                .build()
                .unwrap()
                .identity(),
            two_actions()
                .fairness(Strength::Strong, ["A"])
                .build()
                .unwrap()
                .identity(),
            two_actions()
                .fairness(Strength::Weak, ["B"])
                .build()
                .unwrap()
                .identity(),
            two_actions()
                .fairness(Strength::Weak, ["A", "B"])
                .build()
                .unwrap()
                .identity(),
            two_actions()
                .fairness(Strength::Weak, ["A"])
                .fairness(Strength::Weak, ["B"])
                .build()
                .unwrap()
                .identity(),
        ];
        for (i, a) in ids.iter().enumerate() {
            for b in ids.iter().skip(i + 1) {
                assert_ne!(a, b);
            }
        }
    }

    /// Declaration order, repetition inside a scope, and a repeated assumption are not.
    #[test]
    fn fairness_declaration_order_is_not_part_of_identity() {
        let one = two_actions()
            .fairness(Strength::Strong, ["B"])
            .fairness(Strength::Weak, ["B", "A"])
            .build()
            .unwrap();
        let two = two_actions()
            .fairness(Strength::Weak, ["A", "B", "A"])
            .fairness(Strength::Strong, vec!["B".to_owned()])
            .fairness(Strength::Weak, ["B", "A"])
            .build()
            .unwrap();
        assert_eq!(one, two);
        assert_eq!(one.identity(), two.identity());
        assert_eq!(one.fairness().len(), 2);
        let first = one.fairness().first().unwrap();
        assert_eq!(first.strength(), Strength::Weak);
        assert_eq!(first.actions(), &[0, 1]);
        assert_eq!(one.identity_len_bound(), one.identity().as_bytes().len());
    }

    #[test]
    fn outcome_order_is_not_part_of_identity() {
        assert_eq!(two_outcomes(1, 2).identity(), two_outcomes(2, 1).identity());
    }

    #[test]
    fn a_different_update_is_a_different_identity() {
        assert_ne!(two_outcomes(1, 2).identity(), two_outcomes(1, 3).identity());
    }

    #[test]
    fn declaration_order_is_not_part_of_identity() {
        let one = ModelBuilder::new()
            .variable("a", 0, 1)
            .variable("b", 0, 1)
            .initial_state(&[("a", 0), ("b", 0)])
            .action(ActionDecl::deterministic(
                "A",
                BoolExpr::Const(true),
                vec![("a", IntExpr::constant(1))],
            ))
            .action(ActionDecl::deterministic(
                "B",
                BoolExpr::Const(true),
                vec![("b", IntExpr::constant(1))],
            ))
            .build()
            .unwrap();
        let two = ModelBuilder::new()
            .variable("b", 0, 1)
            .variable("a", 0, 1)
            .initial_state(&[("b", 0), ("a", 0)])
            .action(ActionDecl::deterministic(
                "B",
                BoolExpr::Const(true),
                vec![("b", IntExpr::constant(1))],
            ))
            .action(ActionDecl::deterministic(
                "A",
                BoolExpr::Const(true),
                vec![("a", IntExpr::constant(1))],
            ))
            .build()
            .unwrap();
        assert_eq!(one.identity(), two.identity());
        assert!(one.identity().as_bytes().starts_with(super::IDENTITY_TAG));
    }
}
