//! `continuum-kernel-sat` — trusted checking base: SAT certificate checking (plan §20,
//! PR 9).
//!
//! # Responsibility
//!
//! Independent checking of propositional certificates (resolution/DRAT-class evidence)
//! produced by untrusted solvers, from wire form only.
//!
//! The solver is a search procedure and stays outside the trust base; only its
//! certificate crosses the boundary.
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
