//! `continuum-engine-reference` — the reference exploration path (docs/01 §7.1, PR 8).
//!
//! # Responsibility
//!
//! Deterministic breadth-first exploration with invariant and deadlock checking,
//! shortest witnesses, and finite closure certificates.
//!
//! The reference path is the differential oracle every optimized path is measured
//! against; it is written for obvious correctness, not speed.
//!
//! # Dependency-boundary contract
//!
//! - Engines are untrusted producers: results reach the trust base only as certificates
//!   checked from wire form by `continuum-kernel-*`.
//! - **No `continuum-certificate` or `continuum-kernel-*` crate may depend on this
//!   crate** — experimental engines cannot enter the kernel dependency closure (docs/01
//!   §13).
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
