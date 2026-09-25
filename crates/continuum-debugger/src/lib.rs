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
//! # What has landed
//!
//! - [`reduce`] (PR 18, bn-3km4z): causal minimization's two structural passes over a
//!   failing trace with a happens-before relation, causal closure and delta-debugging
//!   deletion, each candidate replayed by an oracle and every run charged against a
//!   budget. Plan §20 names no crate for the causal minimizer; it lives here because
//!   the debugger's selected configuration and reverse step are the same objects
//!   (RFC 0029), and it depends on no trace producer.
//!
//! The debugger core itself (PR 19) is not here yet.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.

pub mod reduce;
