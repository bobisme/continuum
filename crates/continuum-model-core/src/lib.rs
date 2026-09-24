//! `continuum-model-core` — the typed model core (docs/01 §4.1, plan §20).
//!
//! # Responsibility
//!
//! The programmatic transition model: states, actions, enabling conditions, invariants,
//! and the semantic model identity that every front end must agree on.
//!
//! This is the stable semantic boundary the CML front end, the engines, and the
//! correspondence machinery all target.
//!
//! # What is here
//!
//! - the **programmatic transition model** ([`model`], with [`ident`], [`domain`],
//!   [`expr`], and [`fairness`]) — how a finite transition system is declared, what a state is, and how
//!   guards, updates, and named predicates are evaluated. It was written for PR 8 inside
//!   `continuum-engine-reference` and moved here unchanged by bn-ybq (PR 15a), because
//!   PR 15a requires that "the programmatic model API remains supported — CML is a
//!   second front end, not a replacement" (`notes/plan/notes/START_HERE_IMPLEMENTATION.md`,
//!   PR 15a). A second front end can produce the *same* model type only if that type
//!   lives below both front ends and below every engine. `continuum-engine-reference`
//!   re-exports these four modules under their old paths, so no caller changed.
//! - the **definedness convention** ([`definedness`], bn-24a5c) — how a model names an
//!   undefined read: the predicate `X#defined` of an action or invariant `X` (RFC 0003,
//!   "Definedness"), and which declaration each such predicate guards. The elaborator
//!   writes these predicates and every engine reads them through this one module.
//! - the **semantic model identity** ([`identity`]) — the canonical encoding of a
//!   [`Model`], compared exactly (ADR-0013). Two models are the same model exactly when
//!   their identities are equal, whichever front end built them.
//!
//! # Dependency-boundary contract
//!
//! - **The model core does not depend on `continuum-asupersync`** (plan §20, docs/01
//!   §13, `START_HERE_IMPLEMENTATION.md`). The model is a mathematical object; the
//!   concurrency runtime is an observed subject, never a dependency of the thing that
//!   defines meaning.
//! - No engine, adapter, or Forge dependency: search consumes the model core, not the
//!   other way round.
//! - It has no dependency at all, workspace or external.
//!
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
//!
//! # Determinism (INV-005)
//!
//! The model is data: named variables with declared finite domains, named actions whose
//! guards and updates are written in a closed expression language, and an explicit
//! enumeration of initial states. No part of it is a caller-supplied function. Every
//! observable sequence is ordered by a total order on its own contents, and no
//! hash-ordered collection appears in this crate.

#![forbid(unsafe_code)]
// The no-panic covenant the reference engine held this code to when it lived there
// (`crates/continuum-engine-reference/src/lib.rs`): a model layer that panics on a
// malformed declaration cannot report the defect. Malformed input is a value on every
// path.
#![deny(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::arithmetic_side_effects
)]

pub mod definedness;
pub mod domain;
pub mod expr;
pub mod fairness;
pub mod ident;
pub mod identity;
pub mod model;

pub use definedness::{
    DEFINED_SUFFIX, Definedness, Guarded, definedness_base, definedness_subject,
};
pub use domain::{Domain, DomainError, Variable};
pub use expr::{ArithOp, BoolExpr, CmpOp, EvalError, IntExpr};
pub use fairness::{Fairness, Strength};
pub use ident::{Ident, IdentError};
pub use identity::ModelIdentity;
pub use model::{
    Action, ActionDecl, Assignment, EvaluationError, Model, ModelBuilder, ModelError, Outcome,
    Predicate, Site, State, Step, Symbol,
};
