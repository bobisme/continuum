//! `continuum-sarif` — the SARIF report exporter (plan §17.3, PR 25a).
//!
//! # Responsibility
//!
//! Source-located, deduplicated diagnostics with artifact URIs, stable rule IDs and
//! severity, code flows, and evidence handles.
//!
//! SARIF is a report projection, not a proof format: every result carries an evidence
//! handle pointing back to the authority.
//!
//! # Dependency-boundary contract
//!
//! - **Adapters do not own semantic state** (`START_HERE_IMPLEMENTATION.md`, plan §20).
//! - UI crates cannot mutate evidence directly (plan §20).
//! - Adapters are sinks in the workspace graph: no Continuum crate may depend on this
//!   one.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
