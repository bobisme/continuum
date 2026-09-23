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
//! identity   := "continuum-model/1" variables actions initials predicates
//! variables  := count (token lo:i64 hi:i64)*
//! actions    := count (token bool outcomes)*
//! outcomes   := count (count (token int)*)*      -- sorted by encoding, no duplicates
//! initials   := count (count i64*)*
//! predicates := count (token bool)*
//! count      := u64 big-endian     token := count bytes     i64 := big-endian
//! int        := 0x01 i64 | 0x02 token | 0x03 op int int | 0x04 int int | 0x05 int int
//! bool       := 0x10 b | 0x11 cmp int int | 0x12 bool | 0x13 bool bool | 0x14 bool bool
//!             | 0x15 bool bool | 0x16 int lo:i64 hi:i64
//! ```
//!
//! Every field is length-prefixed or fixed-width, so the encoding is injective: two
//! different models cannot share one byte string.

use crate::expr::{ArithOp, BoolExpr, CmpOp, IntExpr};
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
        let mut out: Vec<u8> = IDENTITY_TAG.to_vec();

        count(&mut out, self.variables().len());
        for variable in self.variables() {
            token(&mut out, variable.name().as_str());
            out.extend_from_slice(&variable.domain().lo().to_be_bytes());
            out.extend_from_slice(&variable.domain().hi().to_be_bytes());
        }

        count(&mut out, self.actions().len());
        for action in self.actions() {
            token(&mut out, action.name().as_str());
            bool_expr(&mut out, action.guard());
            let mut outcomes: Vec<Vec<u8>> = action
                .outcomes()
                .iter()
                .map(|outcome| {
                    let mut encoded = Vec::new();
                    count(&mut encoded, outcome.assignments().len());
                    for assignment in outcome.assignments() {
                        token(&mut encoded, assignment.variable().as_str());
                        int_expr(&mut encoded, assignment.value());
                    }
                    encoded
                })
                .collect();
            outcomes.sort();
            outcomes.dedup();
            count(&mut out, outcomes.len());
            for encoded in outcomes {
                out.extend_from_slice(&encoded);
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

        ModelIdentity { bytes: out }
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
    use crate::expr::{BoolExpr, IntExpr};
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
