//! `continuum-kernel-temporal` — trusted checking base: temporal and liveness
//! certificate checking (plan §20, PR 9).
//!
//! # Responsibility
//!
//! Independent checking of temporal-property and liveness evidence — fairness
//! obligations, lasso witnesses, and their closure conditions — from wire form only.
//!
//! The liveness engine searches; this crate only checks what the search claims.
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
