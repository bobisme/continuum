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
//! - [`scenario`] (PR 18, bn-25z9o): the owner, fault and value-domain passes, which
//!   reduce the failing run's configuration rather than its events. Each candidate is a
//!   smaller configuration that the instantiation runs as a real run of the program;
//!   each pass declares what it preserves (INV-013) and refuses rather than change the
//!   checked property; and every attempt is recorded in RFC 0028's minimizer transcript.
//!
//! - [`mechanism`] (PR 18, bn-5kmuf): replay validation. A failure's causal mechanism
//!   (labelled steps, happens-before and order edges, and guards), derived from the
//!   original failure, and the check that a result's own replay embeds it. Every entry
//!   point of [`reduce`] and [`scenario`] goes through it, and returns a core only as a
//!   [`mechanism::Validated`] one.
//!
//! The debugger core itself (PR 19) is not here yet.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.

pub mod mechanism;
pub mod reduce;
pub mod scenario;
