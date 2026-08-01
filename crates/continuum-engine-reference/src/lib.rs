//! `continuum-engine-reference` — the reference exploration path (docs/01 §7.1, PR 8).
//!
//! # Responsibility
//!
//! Deterministic breadth-first exploration with invariant and deadlock checking,
//! shortest witnesses, and finite closure certificates.
//!
//! The reference path is the differential oracle every optimized path is measured
//! against; it is written for obvious correctness, not speed.
//!
//! # Dependency-boundary contract
//!
//! - Engines are untrusted producers: results reach the trust base only as certificates
//!   checked from wire form by `continuum-kernel-*`.
//! - **No `continuum-certificate` or `continuum-kernel-*` crate may depend on this
//!   crate** — experimental engines cannot enter the kernel dependency closure (docs/01
//!   §13).
//! - This crate depends on nothing: no external crate and no workspace crate. Plan §20
//!   states dependency *rules*, not an allowlist, and the workspace manifest's standing
//!   instruction is that "a missing edge is cheap to add later; a wrong edge is
//!   architectural debt" (`Cargo.toml:9-11`). `continuum-value` was the one serious
//!   candidate and was declined for a specific, recorded reason — see [`ident`]: its
//!   canonical name order is shortlex, while the certificate wire form this engine must
//!   satisfy orders names by bytes. An *external* crate would additionally need an entry
//!   in `tools/governance/dependency-rationale.toml`, which today records that the
//!   workspace "declares no external dependency at all".
//!
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
//!
//! # What is here, and what is not
//!
//! PR 8 has five deliverables
//! (`notes/plan/notes/START_HERE_IMPLEMENTATION.md:205-215`). Two are here:
//!
//! - the **programmatic transition model** ([`model`], with [`ident`], [`domain`],
//!   [`expr`], [`diehard`]) — how a finite transition system is declared, what a state
//!   is, and how guards, updates, and named predicates are evaluated;
//! - **deterministic breadth-first exploration** ([`bfs`]) — the reachable set of a
//!   declared model, in canonical order, with each state's depth, under explicitly
//!   declared bounds.
//!
//! The other three — invariant/deadlock checking, shortest witness, and the finite
//! closure certificate — are separate bones, and both landed modules are written so
//! each attaches without changing them; see the seam tables in [`model`] and [`bfs`].
//!
//! [`bfs`] searches; [`model`] does not, and the split is load-bearing. Exploration
//! reads the model layer through [`model::Model::initial_states`] and
//! [`model::Model::successors`] and through nothing else, so it has no private path to
//! an answer the model layer cannot also give. `tests/diehard_evidence.rs` walks the
//! model with its own nine-line closure and was written before [`bfs`] existed; it is
//! retained as the corroborating layer, and the two must agree on the frozen Die Hard
//! facts or one of them is wrong.
//!
//! # Determinism (INV-005)
//!
//! > Controlled code accesses scheduling, time, entropy, I/O, faults, and cancellation
//! > through explicit capabilities.
//! >
//! > — `notes/plan/plan.md:323-325`
//!
//! The model is data: named variables with declared finite domains, named actions whose
//! guards and updates are written in a closed expression language, and an explicit
//! enumeration of initial states. No part of it is a caller-supplied function, so "this
//! model reads nothing but its own state" is a property of the types rather than a
//! promise — the argument is spelled out in [`expr`]. Every observable sequence is
//! ordered by a total order on its own contents (`BTreeSet`, sorted vectors; no
//! hash-ordered collection appears in this crate), so two enumerations of one model on
//! one state are byte-identical.

#![forbid(unsafe_code)]
// The same no-panic covenant the trusted checking base holds itself to
// (`crates/continuum-kernel-core/src/lib.rs:96-104`), for a different reason: a
// reference oracle that panics on a malformed model declaration cannot report the
// defect it exists to find. Malformed input has to be a value on every path; test
// modules opt back out locally.
#![deny(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::arithmetic_side_effects
)]

pub mod bfs;
pub mod diehard;
pub mod domain;
pub mod expr;
pub mod ident;
pub mod model;
pub mod witness;

pub use bfs::{
    Bound, Bounds, Discovery, Exploration, ExplorationError, Partial, Reachable, explore,
};
pub use domain::{Domain, DomainError, Variable};
pub use expr::{ArithOp, BoolExpr, CmpOp, EvalError, IntExpr};
pub use ident::{Ident, IdentError};
pub use model::{
    Action, ActionDecl, Assignment, EvaluationError, Model, ModelBuilder, ModelError, Outcome,
    Predicate, Site, State, Step, Symbol,
};
// `witness::Step` is deliberately *not* re-exported: `model::Step` already holds that
// name at the root, and a witness step and a successor-row step are different shapes.
// Name it through its module — `witness::Step` — and the two never collide.
pub use witness::{NoWitness, Target, Witness, shortest};
