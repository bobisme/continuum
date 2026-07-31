//! `continuum-debugger` — the verification debugger core (plan §7, PR 19).
//!
//! # Responsibility
//!
//! Selected configuration, enabled frontier, semantic step and reverse, branching on an
//! alternate event, state/observer/obligation views, and why-enabled/why-blocked
//! derivations.
//!
//! Debugging follows the partial order, not a wall-clock trace.
//!
//! # Dependency-boundary contract
//!
//! - The DAP projection lives in `continuum-dap`; this crate holds no protocol surface
//!   and no adapter dependency.
//! - May not import `continuum-forge`.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
