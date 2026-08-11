//! `continuum-security` — capabilities, sandboxing, privacy, and audit (plan §18).
//!
//! # Responsibility
//!
//! Capability model, sandboxing of foreign tools and agent processes, context privacy,
//! audit trail, and signing identities.
//!
//! Security boundaries are semantic boundaries (bet B20): authority is separate from
//! handle possession, and least authority is the default.
//!
//! # Dependency-boundary contract
//!
//! - INV-015 — agent least authority.
//! - Policy and mechanism only: this crate owns no verification semantics and may not
//!   import engines, adapters, or `continuum-forge`.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
//!
//! # What has landed
//!
//! [`injection`] — the prompt-injection / red-team corpus of plan §18.1, docs/49 and
//! research/35, at the shape plan §24.5's ratified promotion gate fixes. It is inert data
//! and nothing else: no dispatch, no policy decision, no expectation about any daemon. The
//! corpus is the question; `crates/continuumd/tests/g2_injection_corpus_evidence.rs` runs
//! every case through the wire boundary and is where the answer is asserted.
//!
//! It lands here rather than beside that test because the corpus is meant to *grow* —
//! research/35's lane enlarges it on every case that ever succeeds — and a corpus that lived
//! inside one test file could only ever be run by that file. Nothing in it imports an
//! engine, an adapter, or `continuum-forge`, so the boundary contract above is unchanged;
//! its one edge is `continuum-workspace`, for the closed plan §4.4
//! [`ArtifactClass`](continuum_workspace::artifact_path::ArtifactClass) the corpus
//! enumerates its surfaces against rather than restating.

pub mod injection;
