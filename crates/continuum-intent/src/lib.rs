//! `continuum-intent` — the protected Intent Contract (plan §5, PR 4).
//!
//! # Responsibility
//!
//! Schema and types for properties, assumptions, observers, bounds, faults, fairness,
//! assurance policy, optimization/non-vacuity, field-level change policy, and intent
//! locks.
//!
//! Intent is the question under verification. INV-001: ordinary operations cannot
//! mutate it; a change to intent is a privileged, diffed, and recorded event.
//!
//! # Dependency-boundary contract
//!
//! - Top of the dependency islands, beside `continuum-workspace`.
//! - The Intent Contract parser and policy sit inside the smallest trust base (docs/33
//!   "Trust boundary"), so this crate may not import engines, adapters, `continuum-
//!   forge`, or `continuum-asupersync`.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
