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
//! Deterministic artifact paths live here too, in [`artifact_path`] (PR-1 / IMPL-05).
//! Placing an artifact is the same question as ordering a snapshot's files — a stable,
//! content-derived name that no clock, counter, or hash seed participates in — and this
//! crate is the top of the dependency islands, so `continuumd`, the evidence graph, and
//! the task layer can all agree on one store layout without importing each other.
//! Deriving the path is not owning the identity: computing content identities is PR 2's
//! `continuum-value`, and publishing under one is PR 2 and `continuumd`.
//!
//! # Dependency-boundary contract
//!
//! - Top of the dependency islands (`START_HERE_IMPLEMENTATION.md`): nothing below the
//!   snapshot layer may be imported here.
//! - No engine, adapter, or Forge dependency.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The snapshot types and behavior land in the PR named above;
//! [`artifact_path`] landed with PR-1 / IMPL-05.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.

pub mod artifact_path;
