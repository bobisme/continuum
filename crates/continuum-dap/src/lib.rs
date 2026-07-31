//! `continuum-dap` — the DAP debugging adapter (plan §17.2, PR 26).
//!
//! # Responsibility
//!
//! Projection of Continuum debugger state onto DAP threads, frames, scopes, variables,
//! breakpoints, and stepping, plus custom causal operations.
//!
//! DAP is a projection of the causal debugger, not a second debugger.
//!
//! # Dependency-boundary contract
//!
//! - **Adapters do not own semantic state** (`START_HERE_IMPLEMENTATION.md`, plan §20);
//!   debugger state lives behind explicit handles in `continuumd`.
//! - UI crates cannot mutate evidence directly (plan §20).
//! - Adapters are sinks in the workspace graph: no Continuum crate may depend on this
//!   one.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
