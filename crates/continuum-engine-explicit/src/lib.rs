//! `continuum-engine-explicit` — the optimized explicit-state engine (docs/01 §7.2).
//!
//! # Responsibility
//!
//! Optimized explicit-state exploration: state hashing, frontier management, and
//! parallel search over the model core.
//!
//! Its closure claims are checked by an independent closure/certificate checker
//! (docs/33 "Architectural separation").
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
