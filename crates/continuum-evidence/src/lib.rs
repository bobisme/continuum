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
//! The claim-status lattice is here, in [`claim_status`] (PR-1 / IMPL-06). Plan §11.4
//! defines it as part of the evidence graph and this crate's responsibility statement
//! already names it, so it needs no new crate and no new dependency edge: the lattice is
//! nine statuses, an order, and the non-regression predicate plan §11.7's compare-and-set
//! decides on. Landing it before the graph itself is deliberate — `continuumd` (PR 5) and
//! any independent linearizability checker (plan §24.5, research/08) must share one
//! implementation of "would this write lower the claim?", and a checker that recomputed
//! the order for itself would be checking its own assumption.
//!
//! Owning the *lattice* is not owning the *authority*: which service may write which
//! status (docs/44's authority table, INV-004, INV-015) stays with the graph and the
//! daemon.
//!
//! # Dependency-boundary contract
//!
//! - INV-004 — no self-certification: a producer may not promote its own claim, and an
//!   untrusted client cannot reach validated or proved.
//! - UI and adapter crates cannot mutate evidence directly (plan §20); writes go
//!   through `continuumd`.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The node and edge types, provenance, and conflict nodes land in the PR named
//! above; [`claim_status`] landed with PR-1 / IMPL-06.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.

pub mod claim_status;
