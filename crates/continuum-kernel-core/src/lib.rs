//! `continuum-kernel-core` — trusted checking base: core certificate semantics (plan
//! §20, PR 9).
//!
//! # Responsibility
//!
//! Finite-closure and type-certificate checking, plus receipt generation carrying the
//! checker epoch, build digest, and input hashes (INV-014).
//!
//! One of the four `continuum-kernel-*` crates that constitute the trusted checking
//! base. Together they carry a <15,000 non-test-line covenant (docs/03) and must build
//! reproducibly from pinned sources.
//!
//! # Dependency-boundary contract
//!
//! - Kernel covenant (plan §20): no shared optimized evaluator code with any engine, no
//!   async, no unsafe, no plugins or dynamic loading, and a serialization boundary
//!   between every engine and the kernel.
//! - **May not depend on search**: no `continuum-engine-*`, no `continuum-forge`, and
//!   no `continuum-asupersync` — directly or transitively.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
