//! `continuum-engine-dpor` — the partial-order reduction engine (docs/01 §7.2).
//!
//! # Responsibility
//!
//! Dynamic partial-order reduction with an explicit reduction witness for every class
//! of interleavings it declines to explore.
//!
//! Audited by an unreduced differential oracle plus a reduction-witness checker
//! (docs/33).
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
