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
//! (`notes/plan/notes/START_HERE_IMPLEMENTATION.md:205-215`). This slice is the first:
//! the **programmatic transition model** — how a finite transition system is declared,
//! what a state is, and how guards, updates, and named predicates are evaluated. The
//! other four — deterministic BFS, invariant/deadlock checking, shortest witness, and
//! the finite closure certificate — are separate bones, and [`model`] is written so
//! each attaches without changing it; see the seam table in that module's
//! documentation.
//!
//! Nothing here searches. There is no reachable-set computation, no frontier, and no
//! visited set in this crate's shipped code. The Die Hard evidence tests walk the model
//! with their own closure, deliberately: the exploration bone should land against a
//! model layer that never had a private path to the answer.
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

pub mod diehard;
pub mod domain;
pub mod expr;
pub mod ident;
pub mod model;

pub use domain::{Domain, DomainError, Variable};
pub use expr::{ArithOp, BoolExpr, CmpOp, EvalError, IntExpr};
pub use ident::{Ident, IdentError};
pub use model::{
    Action, ActionDecl, Assignment, EvaluationError, Model, ModelBuilder, ModelError, Outcome,
    Predicate, Site, State, Step, Symbol,
};
