//! `continuum-cir` — the Concrete Implementation Representation (docs/01 §4.2, PR 17).
//!
//! # Responsibility
//!
//! Event and configuration validation, causal and conflict edges, and the
//! concrete/abstract state projection used to relate a running implementation to its
//! model.
//!
//! CIR is the wire-stable record of what the implementation actually did; uncovered
//! semantic effects are reported as errors rather than smoothed over.
//!
//! # Dependency-boundary contract
//!
//! - Representation only — classification and refinement judgements belong to
//!   `continuum-refinement`.
//! - No engine, adapter, or Forge dependency.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
