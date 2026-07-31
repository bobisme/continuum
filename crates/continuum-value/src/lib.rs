//! `continuum-value` — exact finite values, canonical encoding, and content identity
//! (plan §4.4, PR 2).
//!
//! # Responsibility
//!
//! Exact finite values carried over from Revision 2, their canonical encoding, and
//! their total order.
//!
//! Content identity per ADR-0013: in certified lanes canonical content identity is
//! primary and hash collisions are resolved by canonical comparison; 256-bit-hash
//! identity is permitted only in explicitly labeled non-certified modes.
//!
//! # Dependency-boundary contract
//!
//! - The canonical value decoder is a trust-base component (docs/33): no async, no
//!   search, no adapter, and no Forge dependency.
//! - Leaf crate — it may not import any other Continuum crate.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
