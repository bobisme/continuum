//! `continuum-context` — Context Pack schema and compiler (plan §6, PR 11).
//!
//! # Responsibility
//!
//! Target, verdict, and assurance; state and event slices; source and model references;
//! omissions and expansion handles; replay reference; byte and token budgets.
//!
//! Context is compiled against a budget, not dumped. The compiler's output is audited
//! by independent replay and property-preservation checks (docs/33 "Architectural
//! separation").
//!
//! # Dependency-boundary contract
//!
//! - INV-007 — omission transparency: everything dropped is named and expandable.
//! - INV-013 — property-scoped reduction: reduction is justified relative to the
//!   property under check.
//! - Sits below the verification island and above the adapters; may not import adapters
//!   or `continuum-forge`.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
