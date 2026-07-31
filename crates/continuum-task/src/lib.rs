//! `continuum-task` — cancel-correct verification tasks (plan §4.5, PR 6).
//!
//! # Responsibility
//!
//! Task lifecycle, committed partial evidence, budget accounting,
//! suspension/continuation, drain and finalize behavior, and the guarantee of no orphan
//! workers.
//!
//! Publication is two-phase: reserve identity, stream provisional evidence, validate
//! references, commit. Cancellation before commit leaves no authoritative artifact.
//!
//! # Dependency-boundary contract
//!
//! - INV-009 — monotonic task evidence.
//! - Bet B18/B19 — interactive results carry budget semantics, and cancellation
//!   correctness applies to verification itself.
//! - Sits in the middle island with `continuum-evidence` and `continuumd`; may not
//!   import engines, adapters, or `continuum-forge`.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
