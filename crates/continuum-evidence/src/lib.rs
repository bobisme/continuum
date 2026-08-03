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
//! daemon. PR 7 lands the graph's half of that in [`authority`].
//!
//! # What each module owns (PR 7)
//!
//! | Module | Owns | Bullet |
//! |---|---|---|
//! | [`actor`] | who a producer is, and which producers are services | — |
//! | [`provenance`] | who/when/from-what/under-which-epochs | IMPL-02 |
//! | [`identity`] | ADR-0013 content identity and the `ev_` handle that names one | IMPL-01 |
//! | [`node`] | the twenty node kinds, and a node's immutable versions | IMPL-01/02 |
//! | [`edge`] | the thirteen edge kinds as a closed vocabulary, `CHECKED_BY`'s checker | IMPL-01 |
//! | [`authority`] | node-creation capability, the promotion witness, the status ceiling | IMPL-03 |
//! | [`conflict`] | conflicts as nodes, and resolution as a recorded transition | IMPL-04 |
//! | [`graph`] | the append-only store, its refusals, and its queries | all four |
//! | [`claim_status`] | the plan §11.4 lattice and its compare-and-set | PR-1 / IMPL-06 |
//!
//! # This is the library, not the wire
//!
//! RFC 0038 says "This RFC summarizes; the schemas decide", so
//! `notes/plan/schemas/evidence-graph-{node,edge}.schema.json` are what every vocabulary
//! here is transcribed from, token for token. What this crate deliberately does **not** do
//! is add an operation to the native protocol: appending an edge is a 73rd operation and a
//! plan §10.2 + RFC 0027 registry + IDL change together, and RFC 0038's OPEN F14 — which
//! node kind a `CHECKED_BY` edge's to-handle names, how `edge_id` is derived, and who may
//! create an edge — is **bn-3sypm's** decision, not this crate's. Where that decision would
//! otherwise have been pre-empted, the type is parameterized so any of the candidate answers
//! fits: see [`identity::EvidenceNaming`] (`edge_id` derivation) and
//! [`edge::CheckTargetRule`] (the to-handle's node kind).
//!
//! # Dependency-boundary contract
//!
//! - INV-004 — no self-certification: a producer may not promote its own claim, and an
//!   untrusted client cannot reach validated or proved. Both halves are types here, not
//!   checks: see [`authority::Promotion`] and [`graph::EvidenceGraph::promote`].
//! - UI and adapter crates cannot mutate evidence directly (plan §20); writes go
//!   through `continuumd`.
//! - One workspace edge, to the leaf crate `continuum-value` (ADR-0013 identity, the
//!   assurance payloads plan §11.4 attaches to two statuses, and the six epochs plan §4.6
//!   scopes evidence by). The rationale is in `Cargo.toml`.
//! - **Not** `continuumd`. The daemon's `daemon::evidence`/`daemon::observe` pair models
//!   INV-004 with a private-constructor promotion witness; [`authority`] mirrors that
//!   pattern and imports none of it, because plan §20 puts `continuumd` at this crate's own
//!   tier and an evidence crate that depended on the daemon would invert the graph.
//! - No clock, no RNG, no I/O, no async (INV-005, ADR-0003, GOV-1-04): time is a
//!   [`provenance::Timestamp`] the caller supplies.
//!
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.

pub mod actor;
pub mod authority;
pub mod claim_status;
pub mod conflict;
pub mod edge;
pub mod graph;
pub mod identity;
pub mod node;
pub mod provenance;
