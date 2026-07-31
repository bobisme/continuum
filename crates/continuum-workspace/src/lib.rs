//! `continuum-workspace` — immutable workspace snapshots (plan §4.2, PR 3).
//!
//! # Responsibility
//!
//! Merkle snapshots of a workspace: disk import, editor overlay, fork, seal, and diff,
//! with explicit source, dependency, toolchain, and configuration identities and
//! deterministic file ordering.
//!
//! A snapshot is the immutable input every downstream analysis names. An old snapshot
//! stays reproducible after the working tree changes; using a stale one is a typed
//! error, never a silent re-read.
//!
//! # Dependency-boundary contract
//!
//! - Top of the dependency islands (`START_HERE_IMPLEMENTATION.md`): nothing below the
//!   snapshot layer may be imported here.
//! - No engine, adapter, or Forge dependency.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
