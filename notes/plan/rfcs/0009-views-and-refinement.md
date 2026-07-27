# RFC 0009: Zoomable Views and Refinement

**Status:** Proposed  
**Target gates:** G2/G5 (Revision 2 scheme, docs/26 — not citable without translation to the docs/52 Revision 3 gates per plan §22)

## Summary

Continuum SHALL support a chain of explicit semantic views from abstract service behavior to concrete runtime execution. Each edge is a checkable refinement obligation; no tool may infer correctness merely because two views are generated from related source code.

## View graph

Views form a directed acyclic graph, not necessarily a linear stack:

```text
Service semantics
       ↑
Protocol state      Security observer
       ↑                  ↑
Operational runtime ──────┘
       ↑
Concrete CIR execution
```

A view defines state abstraction, event projection, observer alphabet, assumptions, and supported domains.

## Refinement modes

### Trace/stuttering refinement

Each concrete step maps to zero or more abstract steps.

### Linearization refinement

Concrete operation intervals map to atomic abstract actions. Linearization witnesses may be explicit events or certified prophecy choices.

### Forward simulation

A relation is preserved stepwise.

### Backward/progressive simulation

Used where forward simulation is incomplete, especially for strong observational refinement and nondeterministic implementations.

### Data refinement

Concrete representation relation plus operation simulation.

### Hyper-refinement

Relates sets/distributions of executions for noninterference, randomized algorithms, or adversarial schedulers.

## Observation-indexed correctness

Refinement is always relative to an observer set. Hiding an event is not semantically free if it affects timing, fairness, resource exhaustion, or another observer.

```rust
pub struct ViewContract {
    source: SemanticId,
    target: SemanticId,
    observers: ObserverSet,
    relation: Relation,
    event_map: EventMap,
    fairness_map: FairnessMap,
    assumptions: AssumptionSet,
}
```

## Multi-grain checking

A coarse view is cheap but may produce spurious counterexamples. A fine view is expensive. Continuum may move between grains:

1. check coarse model;
2. replay counterexample in next finer view;
3. if infeasible, derive a separating predicate;
4. refine only the affected semantic slice;
5. retain a proof trail.

This is a CEGAR-like process over user-visible views rather than opaque predicates alone.

## Abstraction maps from Rust

Rust-derived views may use:

- pure projection functions over snapshots;
- event-derived abstract state;
- ghost state maintained by checked instrumentation;
- relational constraints when state is incomplete.

Projection code is untrusted until checked. A projection can be evaluated differentially, verified with Verus/Creusot, or covered by a refinement certificate.

## Compositionality

A component view exposes:

- owned resources;
- imported assumptions;
- exported guarantees;
- visible events;
- interference relation;
- progress obligations.

System composition must prove compatibility and discharge assumptions. Sheaf/gluing techniques are an experimental method for diagnosing incompatible local witnesses, not a default soundness mechanism.

## Counterexample lifting and lowering

A violation in an abstract view is replayed in lower views:

```text
abstract witness
  → constraint-guided operational scenario
  → Lab execution
  → production probe recommendation
```

A concrete violation is projected upward to locate the highest view whose property fails. The explanation reports where refinement, rather than the top-level invariant, breaks.

## Versioning

Each view contract is content-addressed. A source or model change invalidates only dependent claims. Semantic diff reports whether changes affect state mapping, event visibility, fairness, or assumptions.

## Rejected alternatives

- A single `fn abstract_state(&Concrete) -> Abstract` with final-state comparison only.
- Generate an abstract model from code and assume it is equivalent.
- One universal refinement relation for every observer and property.
- Hide runtime operations without proving stuttering/observational irrelevance.
