//! `continuum-kernel-smt` — trusted checking base: SMT certificate checking (plan §20,
//! PR 9).
//!
//! # Responsibility
//!
//! Independent checking of SMT proof certificates over the theories Continuum admits,
//! from wire form only.
//!
//! Foreign solvers remain isolated behind normalized artifacts and never ship in
//! release binaries (ADR-0029).
//!
//! # Dependency-boundary contract
//!
//! - Kernel covenant (plan §20): no shared evaluator code with any engine, no async, no
//!   unsafe, no plugins or dynamic loading.
//! - **May not depend on search**: no `continuum-engine-*`, no `continuum-forge`, and
//!   no `continuum-asupersync` — directly or transitively.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
