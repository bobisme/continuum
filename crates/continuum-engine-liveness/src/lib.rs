//! `continuum-engine-liveness` — the liveness and fairness engine (docs/01 §7.2, plan
//! §21 Phase D).
//!
//! # Responsibility
//!
//! Liveness and fairness checking: property automata, fair cycle detection, and lasso
//! witness extraction.
//!
//! Its witnesses are checked independently by `continuum-kernel-temporal`.
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
