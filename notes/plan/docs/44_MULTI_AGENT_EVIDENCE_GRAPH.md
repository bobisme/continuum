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
| (creation) → proposed | any authorized human/agent — there is no `draft` status (plan §11.4) |
| proposed → observed | execution service |
| proposed → sampled | execution/verification service |
| proposed → bounded | verification service |
| proposed/bounded → validated | independent checker |
| proposed → proved | Lean proof service |
| any → refuted | valid counterexample/checker |
| any → inconclusive | producing service, with a typed INV-008 reason |
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

The note format itself is [`../schemas/whiteboard-note.schema.json`](../schemas/whiteboard-note.schema.json) — plan §11.5's seven headings as a schema, since prose cannot be an input format (INV-003). Which heading proposes which node kind, and how each rule above is met by the shape of the format rather than by a check, is RFC 0038 W1–W9; `crates/continuum-evidence/src/whiteboard.rs` is the library surface. The wire surface is `whiteboard.compile` (plan §10.2, protocol 3.5, `rule whiteboard.compilation`): compilation runs daemon-side, because "references must resolve" is a refusal only the holder of the graph can perform, and the note crosses as an `Opaque` governed by its schema rather than as a declared wire struct. The *view* this section opens with is `crates/continuum-evidence/src/view.rs` (RFC 0038 V1–V6). It is deliberately **not** "rendering graph state back into a note", which is how the previous revision of this sentence described the open work: a note has no status member on purpose — a field a producer can fill is a field a producer can lie in — so a note-shaped view would erase every status the graph holds and would be replayable through `whiteboard.compile` as a fresh proposal set under a new author. The view therefore shares plan §11.5's seven headings with the compiler and nothing else. It places a held node by its kind, which is the compiler's heading-to-kind map inverted; it reports each node's status verbatim and never upgrades one; it counts the fourteen node kinds no heading names rather than dropping them; and `Experiment` is present and empty, because an experiment is a task proposal and never a node. It has no wire verb: nothing declared in the IDL can carry a sectioned, status-bearing report, and the crate-level surface lands first exactly as the compiler's did.

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

The record itself is `provenance` in [`../schemas/evidence-graph-node.schema.json`](../schemas/evidence-graph-node.schema.json), which closes it at four members and is what `crates/continuum-evidence/src/provenance.rs` is transcribed from. Reading it back is two grains and two surfaces: *who produced what* is `EvidenceGraph::nodes_by_producer`, and *derivation* — the "source retrievals" half, the one reproduction needs — is the transitive closure over `provenance.inputs` in `crates/continuum-evidence/src/derivation.rs` (RFC 0038 P1–P7). The closure runs backward only, because "made from" and "used by" are two questions and only the first can state in what sense it is complete. An input that names an artifact the graph holds no node about is reported as a **typed absence** and never dropped: `provenance.inputs` is written for admitted actors but its entries are still names, and a note's own handle is in every node it compiles while never being a node itself, so a graph that names what it does not hold is the ordinary case rather than a malformed one (INV-016, INV-008). It has no wire verb: `evidence.query` answers handle lists, and an artifact the graph does not hold has no handle to answer with.

## Garbage collection

Superseded proposals may be compacted but remain reachable from receipts/audits. Large derivations use content-addressed shared subgraphs.

## Security

- source text cannot instantiate privileged graph edges;
- actors may append only allowed node types;
- promotion endpoints require checker capabilities;
- graph queries enforce trace/source privacy;
- untrusted attachments are sandboxed;
- provenance is signed/hashed where organization policy requires.
