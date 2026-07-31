//! `continuum-incremental` — the incremental semantic database (plan §9, PR 23).
//!
//! # Responsibility
//!
//! Content-addressed queries for parsing, model construction, property automata,
//! exploration, context, and diff, with dependency edges classified as exact,
//! validated, conservative, or experimental.
//!
//! The build-vs-adopt decision for the engine itself is an ADR (PR 22a) that gates
//! Phase C.
//!
//! # Dependency-boundary contract
//!
//! - INV-010 — incremental parity: the incremental engine is audited by an independent
//!   clean-result comparison (Incremental Parity Audit, plan §9.5), which may not share
//!   decision logic with it.
//! - May not import adapters or `continuum-forge`.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
