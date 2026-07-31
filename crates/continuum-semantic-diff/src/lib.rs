//! `continuum-semantic-diff` — semantic and intent diff (plan §5.3, docs/40, PR 12).
//!
//! # Responsibility
//!
//! Classification of changes — exact equality, property AST edit, assumption
//! add/remove, bound change, observer event change, fault/fairness/assurance change —
//! and the impact analysis derived from them.
//!
//! Solver-based implication is admitted only where it is sound and bounded; everything
//! else is reported as a privileged change.
//!
//! # Dependency-boundary contract
//!
//! - Diffs describe change; they never apply it. Application belongs to `continuum-
//!   repair`.
//! - May not import adapters or `continuum-forge`.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
