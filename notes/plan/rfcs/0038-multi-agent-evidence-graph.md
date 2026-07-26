# RFC 0038: Multi-Agent Evidence Graph

## Status
Draft for implementation.

**Target gate:** G2 (agent-computer interface); promotion authority feeds every gate's evidence discipline
**Owners:** evidence/workbench leads
**Normative language:** MUST/SHOULD/MAY per RFC 2119.
**Normative schemas:** [`../schemas/evidence-graph-node.schema.json`](../schemas/evidence-graph-node.schema.json) and [`../schemas/evidence-graph-edge.schema.json`](../schemas/evidence-graph-edge.schema.json) carry the normative plan §11 content — node kinds, edge types, the §11.4 status lattice, typed `Inconclusive` reasons (INV-008), `validation_basis` (`checked-certificate` vs `trusted-solver`), and parameterization (`checked / cutoff_checked / proved_universal`). This RFC summarizes; the schemas decide.

## Nodes/edges
Typed immutable candidates, claims, failures, proofs, patches, runs, receipts, conflicts, and decisions, connected by the 13 plan §11.3 edge types: `SUPPORTS`, `REFUTES`, `DEPENDS_ON`, `REFINES`, `EXPLAINS`, `REPAIRS`, `INVALIDATES`, `GENERALIZES`, `COUNTEREXAMPLE_TO`, `CHECKED_BY`, `DERIVED_FROM`, `CONFLICTS_WITH`, `SUPERSEDES`. Edges name checker/evidence when applicable; the machine encoding is [`schemas/evidence-graph-edge.schema.json`](../schemas/evidence-graph-edge.schema.json) (nodes: [`schemas/evidence-graph-node.schema.json`](../schemas/evidence-graph-node.schema.json)).

## Authority
Actor capabilities control node creation; status promotion is service-restricted. Confidence is metadata. The status lattice is exactly plan §11.4 (`Proposed, Observed, Sampled, Bounded, Validated, Proved, Refuted, Inconclusive, Superseded` — there is no `draft` status). Only trusted services promote into `Validated` or `Proved`; `Sampled` and `Bounded` promotions name the producing engine's service identity; every `Inconclusive` carries a typed INV-008 reason. Agent votes or confidence never change status.

## Queries
Missing obligations, conflicting candidates, proof frontier, repair frontier, semantic duplicates, provenance, and task generation.

## Whiteboard compiler
Converts structured notes to proposed nodes; rejects nonexistent references and status claims without evidence.

## Acceptance
Parallel swarm with conflicting abstractions and adversarial false consensus.
