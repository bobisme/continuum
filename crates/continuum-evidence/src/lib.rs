//! `continuum-evidence` — the multi-agent Evidence Graph (plan §11, PR 7).
//!
//! # Responsibility
//!
//! Node and edge types, the status lattice, immutable versions, provenance, trusted
//! status transitions, and conflict nodes: support, refute, depend, refine, explain,
//! repair, and check edges.
//!
//! Evidence is the shared substrate for humans and agents; coordination is a graph
//! write, not a chat message.
//!
//! # Dependency-boundary contract
//!
//! - INV-004 — no self-certification: a producer may not promote its own claim, and an
//!   untrusted client cannot reach validated or proved.
//! - UI and adapter crates cannot mutate evidence directly (plan §20); writes go
//!   through `continuumd`.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
