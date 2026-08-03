# RFC 0038: Multi-Agent Evidence Graph

## Status
Draft for implementation.

**Target gate:** G2 (agent-computer interface); promotion authority feeds every gate's evidence discipline
**Owners:** evidence/workbench leads
**Normative language:** MUST/SHOULD/MAY per RFC 2119.
**Normative schemas:** [`../schemas/evidence-graph-node.schema.json`](../schemas/evidence-graph-node.schema.json) and [`../schemas/evidence-graph-edge.schema.json`](../schemas/evidence-graph-edge.schema.json) carry the normative plan §11 content — node kinds, edge types, the §11.4 status lattice, typed `Inconclusive` reasons (INV-008), `validation_basis` (`checked-certificate` vs `trusted-solver`), and parameterization (`checked / cutoff_checked / proved_universal`). This RFC summarizes; the schemas decide.

## Nodes/edges
Typed immutable candidates, claims, failures, proofs, patches, runs, receipts, conflicts, and decisions, connected by the 13 plan §11.3 edge types: `SUPPORTS`, `REFUTES`, `DEPENDS_ON`, `REFINES`, `EXPLAINS`, `REPAIRS`, `INVALIDATES`, `GENERALIZES`, `COUNTEREXAMPLE_TO`, `CHECKED_BY`, `DERIVED_FROM`, `CONFLICTS_WITH`, `SUPERSEDES`. Edges name checker/evidence when applicable; the machine encoding is [`schemas/evidence-graph-edge.schema.json`](../schemas/evidence-graph-edge.schema.json) (nodes: [`schemas/evidence-graph-node.schema.json`](../schemas/evidence-graph-node.schema.json)). The library vocabulary is `crates/continuum-evidence`; the one wire verb that appends an edge is `evidence.link` (plan §10.2, protocol 3.3), and what it may append is decided under "Edges: what a `CHECKED_BY` edge names" below.

## Authority
Actor capabilities control node creation; status promotion is service-restricted. Confidence is metadata. The status lattice is exactly plan §11.4 (`Proposed, Observed, Sampled, Bounded, Validated, Proved, Refuted, Inconclusive, Superseded` — there is no `draft` status). Only trusted services promote into `Validated` or `Proved`; `Sampled` and `Bounded` promotions name the producing engine's service identity; every `Inconclusive` carries a typed INV-008 reason. Agent votes or confidence never change status.

**Edge creation (D3, decided at protocol 3.3 by bn-3sypm; this section previously covered node creation and status promotion only).** Twelve of the thirteen edge kinds are ordinary assertions and are created under the same actor capability that creates a node. `CHECKED_BY` is not: it is the graph's record that an *independent* check happened, so it is the checker's own artifact and no one else's.

- **The checker is the caller.** A `CHECKED_BY` edge MUST be appended by the service that performed the check, and the edge's `checker` MUST be taken from the admitted capability's actor — never from a request field. There is no wire field that names a checker, for the same reason there is no wire field that names a status: a field a caller can fill is a field a caller can lie in. This is the device `observe.ingest` already uses for `provenance.actor`.
- **A checker is a `service:` actor.** An `agent:`, `human:`, or `ci:` capability MUST NOT be able to append a `CHECKED_BY` edge at all. RFC 0027 P3 says an evidence-status write is "performed by a trusted service identity"; the edge that *records* the check is held to the same scheme, and the refusal is `CapabilityDenied` (RFC 0027 X1: a denial is a unit).
- **No self-certification (INV-004).** A `CHECKED_BY` edge whose checker equals the `from` node's `provenance.actor` MUST be refused. This is the edge-level statement of the same rule `EvidenceGraph::promote` and `evidence.verify` already enforce for promotion, and it is checked *before* anything about the checked artifact is read, because a service checking its own production has established nothing whatever the bytes say.
- **An edge is not a promotion.** Appending a `CHECKED_BY` edge writes no status, mints no promotion witness, and cannot reach the compare-and-set. The two are separate operations on purpose: an edge is evidence *that* a check ran, and what that evidence licenses is a later, separate decision. RFC 0027's "no `evidence` operation writes a status" therefore survives the addition unchanged.
- **Not the daemon's own side effect.** `evidence.verify` MUST NOT silently append a `CHECKED_BY` edge as a by-product of a promotion. A daemon that both promoted and recorded the check would make the record indistinguishable from the promotion it justifies.

## Edges: what a `CHECKED_BY` edge names (RFC 0026 F14)
Four questions this RFC left open blocked INV-004's edge dimension — plan §2 SD-11, "`evidence-graph-edge` requires `checker` on `CHECKED_BY`". They are decided here; `crates/continuum-evidence` carries the library half and the native protocol carries the wire half (`evidence.link`, `@since("3.3")`).

**D1 — the `to`-handle names a `receipt`, and only a `receipt`.** RFC 0026 F14 offered three candidates (`run`, `certificate`, `receipt`) and observed that "there is no node kind for a check". There is one: plan §11.2's `ProofReceipt`, whose format is RFC 0024, is precisely the artifact a checker emits to record that it checked something. The other two are rejected on INV-004's own words — "search code does not check its own strongest claims; certificates cross an independent checker":

- a `run` is search's own output, so an edge pointing at one would let the search machinery be its own check;
- a `certificate` is what *crosses* a checker, not the crossing. It belongs on the edge's `from`, and the canonical sentence is `certificate CHECKED_BY receipt`. An artifact offered *toward* a claim is a `SUPPORTS` edge; the vocabulary already has the word.

So the direction is fixed too: `from` is the checked subject, `to` is the receipt that records the check, and `checker` is the service that produced the receipt. `CheckTargetRule` in `crates/continuum-evidence/src/edge.rs` holds the rule as data and its default is now this decision rather than `Undecided`.

**D2 — `edge_id` is the content identity of what the edge asserts, named through one seam.** Identity is the canonical preimage (ADR-0013); the `ev_` token is a *name for* it, minted in an explicitly labeled non-certified mode, which is why naming is separable at all (`identity::EvidenceNaming`). The wire's naming is the deployment's `ContentIdentifier` seam at `ArtifactClass::Evidence` — the same seam and the same class that names nodes, so one identity kernel names every evidence artifact and `evidence.verify`'s re-derivation discipline extends to edges unchanged. The preimage is domain-separated from a node's by a leading `evidence-graph-edge/v1` tag, which is outside the artifact-handle character class and therefore cannot be a commitment, so no node preimage can spell an edge preimage.

**D3 — edge-creation authority** is the "Authority" section above.

**D4 — a node's wire identity is what the node is *about*.** Two derivations existed and this RFC fixed neither: `continuumd`'s `observe.ingest` derives a node handle from (referenced artifact commitment, capture profile), and `continuum-evidence` derives its content key from the whole record minus status and idempotency key — which includes `provenance`, and therefore the producing actor and the wall-clock capture time. **The daemon's is the wire's.** The reason is this RFC's own sentence: "a replayed write returns the original node identity" is a claim *across processes and across producers*, and an identity that contains a clock cannot support it — two ingests of one trace under one profile would mint two nodes, and convergence would depend on when and by whom the write arrived, which is the write-order resolution plan §11.7 forbids. So:

- a node's wire identity is a function of the artifact it is about and the profile it was captured under, and of nothing else; `provenance` is deliberately outside it, as `status` and `idempotency_key` already are;
- an edge's wire identity is likewise a function of what it asserts — kind, both endpoints, and the checker — and not of who asserted it or when, so a checker's retry converges on one edge;
- `continuum-evidence`'s `EvidenceIdentity` is a **within-graph content key**, not a wire name. It never appears on the wire: `EvidenceNode::to_record` and `EvidenceEdge::to_record` take the handle as a *parameter*, so the library already accepts the wire's names as input rather than deriving them. A library graph joined to the wire supplies handles through `EvidenceNaming`; it does not ask the library to name anything.

## Write and concurrency model
Absorbed from plan §11.7 (SD-06). The graph is append-only; nothing is edited in place. Publication is per-artifact atomic (INV-017). Status promotion is linearized per claim identity as a compare-and-set against the claim's current status: racing promotions cannot regress the lattice, and a lost CAS returns the typed `StatusConflict` error (plan §10.3) — the caller re-reads and retries against the current status. Concurrent contradictory claims materialize a `Conflict` node rather than resolving by write order. Idempotency keys (RFC 0026) make agent retries safe: a replayed write returns the original node identity. The node schema MUST carry `claim_id`, `idempotency_key`, and `service_identity` for this purpose.

Enforcement of this model is a frontier lane: the ratified validation criteria for the plan §24.5 row (writer counts, zero-tolerance bounds, and deterministic conflict artifacts) live in [`research/08`](../research/08-agentic-verification.md), which the register quotes verbatim.

## Queries
Missing obligations, conflicting candidates, proof frontier, repair frontier, semantic duplicates, provenance, and task generation.

## Whiteboard compiler
Converts structured notes to proposed nodes; rejects nonexistent references and status claims without evidence.

## Acceptance
Parallel swarm with conflicting abstractions and adversarial false consensus.
