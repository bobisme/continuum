# Multi-Agent Evidence Graph

## Principle

Agents may converse, but Continuum trusts only typed artifacts and checked edges.

## Graph model

```rust
struct EvidenceNode {
    id: ContentId,
    kind: NodeKind,
    payload: ArtifactHandle,
    status: EvidenceStatus,
    provenance: Provenance,
}

struct EvidenceEdge {
    from: ContentId,
    to: ContentId,
    relation: Relation,
    justification: Option<ArtifactHandle>,
}
```

## Why a graph

Concurrent-systems work is naturally non-linear:

- one counterexample refutes several candidates;
- one invariant supports many properties;
- one patch repairs one failure but invalidates a proof;
- two agents may propose conflicting abstraction maps;
- one theorem generalizes many finite observations.

A chat transcript obscures this structure.

## Status authority

| Status transition | Authority |
|---|---|
| draft → proposed | any authorized human/agent |
| proposed → observed | execution service |
| proposed → bounded | verification service |
| proposed/bounded → validated | independent checker |
| proposed → proved | Lean proof service |
| any → refuted | valid counterexample/checker |
| any → superseded | policy/owner with explicit edge |

## Conflict handling

Conflicts are first-class nodes:

```text
Abstraction A maps reply publication to Ack
Abstraction B maps storage stability to Ack
Conflict: both cannot satisfy observed correspondence
Required experiment/proof: distinguish observer contract
```

A coordinator cannot resolve conflict by selecting the most confident agent answer.

## Task generation

The graph generates bounded tasks:

- close missing proof edge;
- find counterexample to candidate;
- explain conflict;
- synthesize invariant for failed induction;
- repair source span attached to causal core;
- review intent diff;
- compare candidate behavior/cost.

Each task contains only relevant graph slice and handles.

## Whiteboard

A human-friendly whiteboard view permits provisional notes. Compilation rules:

- every claim becomes a proposed node;
- references must resolve;
- status words in prose do not promote status;
- experiments become task proposals;
- conclusions require supporting edges;
- unresolved contradictions remain visible.

## Merging agent work

Agents do not merge mutable branches of “reasoning.” They publish immutable candidates. The integrator computes:

- duplicate semantic candidates;
- supporting/refuting evidence;
- conflicts;
- missing obligations;
- Pareto/dominance relations;
- current frontier of justified options.

## Credit and provenance

Every node records actor/tool/model version, prompts/tool inputs as policy permits, source retrievals, and derivation. This supports scientific credit, debugging, benchmark analysis, and reproduction without granting authority based on identity.

## Garbage collection

Superseded proposals may be compacted but remain reachable from receipts/audits. Large derivations use content-addressed shared subgraphs.

## Security

- source text cannot instantiate privileged graph edges;
- actors may append only allowed node types;
- promotion endpoints require checker capabilities;
- graph queries enforce trace/source privacy;
- untrusted attachments are sandboxed;
- provenance is signed/hashed where organization policy requires.
