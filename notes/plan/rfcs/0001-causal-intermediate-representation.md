# RFC 0001: Causal Intermediate Representation

**Status:** Proposed  
**Owners:** semantics, runtime, verification  
**Target gate:** G0   (Revision 2 scheme, docs/26 — not citable without translation to the docs/52 Revision 3 gates per plan §22)
**Normative vocabulary:** MUST, SHOULD, MAY are interpreted as in RFC 2119.

## Summary

Continuum SHALL use a versioned **Causal Intermediate Representation** (CIR) as its normative semantic interchange format. CIR represents executions as finite event structures rather than as total traces. A run is a causally closed configuration of events equipped with causality, conflict, typed effect footprints, observations, and optional view projections.

CIR is neither a Rust AST nor a serialized scheduler log. It is the mathematical boundary between authoring, concrete execution, model exploration, refinement, replay, production observation, and certificate checking.

## Motivation

Interleaving traces lose the concurrency that reduction and production-trace validation need. Runtime-specific logs contain incidental details that prevent stable replay and independent checking. A TLA+-style state graph is excellent for some algorithms but forces independent events into arbitrary orders. CIR keeps the partial order primary and derives total orders or states only when required.

## Semantic object

A finite prime event-structure fragment is represented by:

\[
\mathcal E = (E,\preceq,\#,\lambda,\rho,\omega)
\]

where:

- \(E\) is a finite set of event identities;
- \(\preceq\) is a well-founded causal partial order;
- \(\#\) is an irreflexive, symmetric, hereditary conflict relation;
- \(\lambda : E \to Label\) assigns semantic labels;
- \(\rho : E \to Footprint\) assigns typed resources and effect phases;
- \(\omega : E \to Observation\) assigns observer-indexed visible facts.

A **configuration** \(C\subseteq E\) is causally closed and conflict-free. Configuration extension \(C \xrightarrow e C\cup\{e\}\) is legal when all causes of \(e\) are in \(C\) and no event in \(C\) conflicts with \(e\).

CIR v0 permits dynamically discovered events and conflict. It does not require materializing the whole event structure before exploration.

## Identity

Event identity MUST be semantic, not pointer- or thread-address-based. The canonical identity is:

```text
EventId = H(
    cir_semantics_version,
    origin,
    actor_epoch,
    local_operation_index,
    normalized_label,
    normalized_inputs,
    canonical_causal_predecessor_ids
)
```

`origin` identifies the model action, Rust source site, foreign frontend node, or production probe. Identity construction MUST be deterministic. Hash collisions MUST be resolved by comparing canonical encodings in proof-bearing lanes.

## Event schema

Each event contains:

```rust
pub struct Event {
    pub id: EventId,
    pub origin: Origin,
    pub owner: OwnerId,
    pub epoch: Epoch,
    pub causes: SmallVec<[EventId; 4]>,
    pub conflicts: ConflictDescriptor,
    pub label: Label,
    pub footprint: Footprint,
    pub effect: EffectTransition,
    pub obligations: ObligationDelta,
    pub time: TimeConstraint,
    pub observation: ObservationSet,
    pub fault: Option<FaultEvent>,
    pub provenance: Provenance,
}
```

### Label

A label is a stable, typed operation descriptor. It contains an operation family and canonical payload. Payloads MUST use the Continuum value algebra, not arbitrary serde blobs.

### Footprint

A footprint is a set of typed access claims:

```rust
enum AccessMode {
    Observe,
    Read,
    Write,
    Consume,
    Produce,
    Reserve,
    Commit,
    Abort,
    Synchronize,
}
struct Access {
    resource: ResourcePath,
    mode: AccessMode,
    logical_range: Option<RangeDescriptor>,
}
```

Footprints are evidence for independence; they are not automatically trusted. Domain packs define conflict rules and must prove or test their soundness.

### Effect transition

Effects use explicit phases:

```text
Requested → Reserved → Committed
                  ↘ Aborted
```

Some packs add domain-specific phases, such as `Submitted`, `Stable`, or `Acknowledged`. Extensions MUST map to the generic phase algebra.

### Obligations

Obligations are linear semantic resources created and discharged by events. Examples include reply obligations, reserved channel capacity, outstanding durable-write acknowledgements, child-region quiescence, and cancellation finalization.

### Time

Time is represented by constraints, not only timestamps:

```text
earliest(e) ≤ occurrence(e) ≤ latest(e)
occurrence(e1) + d ≤ occurrence(e2)
```

A concrete Lab run may instantiate all event times. A production trace may provide intervals. A timed model may retain symbolic bounds.

### Observation

Visibility is indexed by observer:

```text
ObserverId -> finite set of typed observations
```

This supports trace refinement, opacity, noninterference, strong observational refinement, and partial telemetry.

## Conflict and independence

Two events are independent only if swapping them preserves:

1. enabledness;
2. resulting abstract configuration up to canonical equivalence;
3. all relevant observer projections;
4. obligation ownership and discharge;
5. time/fairness feasibility;
6. fault and durability semantics.

The default footprint rule is conservative. Packs MAY supply a stronger semantic independence oracle. Unsound independence is a soundness defect and MUST be subject to mutation tests and differential execution.

## Views

A view is a total or explicitly partial projection:

\[
\alpha_v : Configuration \to AbstractState_v \cup \{\textsf{Unknown}\}
\]

and an observation map:

\[
\pi_v : Event \to AbstractAction_v^*.
\]

`Unknown` cannot be silently treated as success. A view declares supported configurations and abstraction assumptions.

Stuttering refinement permits an event sequence to map to zero abstract steps. Linearization refinement permits a concrete interval to map to one abstract action when a declared witness event or validated prophecy selects the linearization.

## Serialization

CIR SHALL have:

- a canonical binary encoding for hashing, storage, and replay;
- a readable JSON encoding for tooling;
- a deterministic text form for diffs;
- an explicit semantic epoch (`semantics_version`) and an explicit encoding epoch, the latter carried by the `schema_id`/`schema_epoch` header every artifact declares (`schemas/README.md`);
- unknown-field preservation in forward-compatible readers;
- a schema fingerprint embedded in every crashpack.

Canonicalization includes sorted maps/sets, normalized integer representation, canonical NaNs if floats are enabled, normalized paths, and stable symbol interning.

## Validation

The `continuum-cir` validator MUST check:

- unique event identities;
- acyclic causality;
- causal closure of configurations;
- conflict symmetry and heredity where materialized;
- obligation conservation;
- effect-phase legality;
- resource-path type validity;
- epoch and restart constraints;
- observer-schema validity;
- source-provenance integrity;
- version compatibility.

## Performance requirements

CIR is logically rich but hot paths cannot allocate a graph object per event. Implementations SHOULD use interned labels, structure-of-arrays event storage, compressed predecessor sets, persistent configuration bitmaps, and append-only chunked arenas. The wire format is not the in-memory layout.

## Security and trust

Production CIR is untrusted input. Parsers MUST be resource-bounded and reject cyclic, oversized, or adversarial structures. Certificates bind to the canonical CIR hash, semantic version, model hash, and assumptions.

## Rejected alternatives

- **Total trace as primary:** destroys concurrency and creates false ordering.
- **TLA+-style states only:** unsuitable for incomplete production evidence and runtime causality.
- **Rust MIR as canonical semantics:** too concrete, unstable, and language-specific.
- **OpenTelemetry spans as semantics:** useful transport, insufficient formal contract.
- **Petri nets as sole IR:** excellent mathematical lane, but awkward for rich typed values, observations, and arbitrary abstract transitions.

## Open research questions

- Whether stable event structures, occurrence nets, interval pomsets, or a cubical model should become the canonical mathematical presentation after v0.
- Whether conflicts can be represented intensionally without harming certificate checking.
- How much observer-specific independence can be computed compositionally.
- Whether interval events should be first-class or compiled into begin/commit pairs.
