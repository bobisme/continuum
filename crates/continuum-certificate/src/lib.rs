//! `continuum-certificate` — certificate formats and their wire representation (plan
//! §20, PR 9).
//!
//! # Responsibility
//!
//! Finite-closure certificates, type certificates, reduction witnesses, and the
//! serialized forms in which an untrusted engine hands its claim to the trusted kernel.
//!
//! A certificate is checked from its wire form, never from shared memory, so this crate
//! defines the boundary format rather than any evaluator.
//!
//! # Dependency-boundary contract
//!
//! - **The certificate checker may not depend on search**
//!   (`START_HERE_IMPLEMENTATION.md`; plan §20 "proof/certificate checker does not
//!   depend on search engines"). No `continuum-engine-*`, no `continuum-forge`,
//!   directly or transitively.
//! - Experimental engines cannot enter the kernel dependency closure (docs/01 §13), and
//!   this crate is inside it.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
