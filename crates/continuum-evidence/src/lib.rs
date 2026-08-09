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
//! | [`whiteboard`] | plan §11.5's seven sections, and the compiler that turns them into proposals | PHASE-A-DEL-04 |
//! | [`view`] | the other direction: held graph state, rendered into those seven sections | PHASE-A-DEL-04 |
//! | [`derivation`] | RFC 0038's `provenance` query in docs/44's derivation sense: the transitive ancestry over `provenance.inputs` | PHASE-A-DEL-04 |
//!
//! # The whiteboard runs in two directions, and only one of them has a wire verb
//!
//! plan §11.5's whiteboard is an *input* format, and INV-003 says an input format is a
//! schema: `notes/plan/schemas/whiteboard-note.schema.json` is the normative one and
//! [`whiteboard`] is transcribed from it. When that module landed it carried no wire
//! operation and this paragraph said `whiteboard.compile` did not exist; it does now, at
//! protocol 3.5 under `rule whiteboard.compilation` (RFC 0038 W10–W12), and the library
//! keeps its own compiler because the two resolve against two different graphs — the
//! daemon's, keyed by `ev_` handles, and this crate's, keyed by the within-graph content
//! keys RFC 0038 D4 says never reach the wire.
//!
//! [`view`] is the reverse direction, docs/44's "human-friendly whiteboard view", and it has
//! no wire verb: nothing in the IDL can carry a sectioned, status-bearing report without a
//! response member that does not exist, so the crate-level typed surface lands first and the
//! wire spelling lands when its RFC decides it. That is the order `continuum-forge` took and
//! the order the compiler itself took. A view is deliberately **not** a note (RFC 0038 V6):
//! the note format has no status member on purpose, and rendering held state into it would
//! erase every status the graph holds.
//!
//! # Two of RFC 0038's seven queries are answered here
//!
//! [`graph::EvidenceGraph::contradictions`] is "conflicting candidates". [`derivation`] is
//! "provenance", in docs/44's derivation sense — "source retrievals, and derivation" — as
//! the transitive ancestry over `provenance.inputs`, which every node carries and which
//! `observe.ingest`, `evidence.link`, and `whiteboard.compile` all write. It has no wire
//! verb either, and for a sharper reason than the view's: `evidence.query` answers
//! `list<EvidenceHandle>`, and an input naming an artifact the graph holds no node about has
//! no `ev_` handle at all, so the typed absence INV-016 requires is not expressible there —
//! nor is the relation itself, since no `EvidenceQuery` member names an artifact handle.
//! Which of the other five remain blocked, and on what, is [`graph`]'s module documentation
//! and RFC 0038's "Queries".
//!
//! # This is the library, not the wire
//!
//! RFC 0038 says "This RFC summarizes; the schemas decide", so
//! `notes/plan/schemas/evidence-graph-{node,edge}.schema.json` are what every vocabulary
//! here is transcribed from, token for token. What this crate deliberately does **not** do
//! is serve the wire: appending an edge is `evidence.link`, the protocol's 73rd operation
//! (plan §10.2 + RFC 0027's registry + the IDL, at protocol 3.3, bn-3sypm).
//!
//! RFC 0038's F14 was open when this crate landed and the types were parameterized so that
//! no candidate answer was pre-empted. It is now decided, and each answer arrived as a value
//! rather than as a rewrite:
//!
//! | RFC 0038 | Decision | Where it lives here |
//! |---|---|---|
//! | D1 | a `CHECKED_BY` edge's to-handle names a `receipt` | [`edge::CheckTargetRule`]'s [`Default`], enforced by [`graph::EvidenceGraph`] |
//! | D2 | `edge_id` is the content identity, named by the deployment's identity kernel | [`identity::EvidenceNaming`], the seam the wire fills |
//! | D3 | only the checker appends a check edge, only a `service:` actor, never over its own production | the daemon's — this crate still mints no edge-creation capability |
//! | D4 | a node's wire identity is what it is *about*; this crate's is a within-graph content key | [`identity`]'s module docs, `a_records_wire_name_is_a_parameter_not_a_derivation` |
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
pub mod derivation;
pub mod edge;
pub mod graph;
pub mod identity;
pub mod node;
pub mod provenance;
pub mod view;
pub mod whiteboard;
