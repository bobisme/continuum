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
//! `continuum-value`, and durably publishing under one is PR 6+ and `continuumd`.
//!
//! [`publication`] (PR 2 / IMPL-05, IMPL-06) is the next question up from [`artifact_path`]
//! and lands here for the same reason. `artifact_path` says *where* an artifact goes;
//! `publication` says *when* it becomes visible there (INV-017) and *who* may look
//! (plan §4.4: "Authorization is always checked independently of handle possession").
//! Those two rules have three consumers — plan §11.7 makes the evidence graph's write
//! model per-artifact atomic, docs/35 makes publication ordering a daemon obligation, and
//! RFC 0030 repeats atomicity for the incremental query index — and the only crate all
//! three may import is this one. It is a semantic reference implementation, in memory and
//! stdlib-only: identity arrives through a trait (`continuum-value`, ADR-0013) and
//! durability arrives with the daemon, so neither changes this crate's dependency closure.
//!
//! # Dependency-boundary contract
//!
//! - Top of the dependency islands (`START_HERE_IMPLEMENTATION.md`): nothing below the
//!   snapshot layer may be imported here.
//! - No engine, adapter, or Forge dependency.
//! - No Continuum dependency at all, still: [`publication`] takes content identity as a
//!   [`ContentIdentifier`](publication::ContentIdentifier) parameter rather than importing
//!   `continuum-value`, so an edge that would only carry an opaque token is not created.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The snapshot types and behavior land in the PR named above;
//! [`artifact_path`] landed with PR-1 / IMPL-05 and [`publication`] with
//! PR 2 / IMPL-05 and IMPL-06.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.

pub mod artifact_path;
pub mod publication;
