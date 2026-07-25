# Research Note 03: Refinement, Composition, and Local-to-Global Reasoning

**Claim class:** core design plus experimental mathematics  
**Relevant sources:** [S14], [S24], [S32]–[S37], [S53], [S54], [S71A]

## Thesis

Continuum's central product claim is not “the simulator found no bugs.” It is that guarantees established at useful abstract views apply to the implementation under explicit refinement relations.

The system needs ordinary engineering paths for stuttering/trace refinement and research lanes for strong observational refinement, multi-grain abstraction, compositional separation logic, and sheaf-style local-to-global consistency.

## Refinement ladder

```text
abstract service
    ↓ data/trace refinement
distributed protocol
    ↓ stuttering/linearization refinement
operational model
    ↓ event/concrete refinement
asupersync execution
    ↓ observational conformance
production evidence
```

Each arrow can fail independently. Diagnostics should locate the highest broken edge.

## Multi-grain specifications

The EuroSys 2025 ZooKeeper work demonstrates the practical value of multiple specification grains for connecting implementation behavior to models. Continuum should make grain boundaries first-class:

- a coarse model for global safety;
- a protocol model for message/state logic;
- an operational model for retries, queues, timers, and storage;
- a runtime view for cancellation and task structure.

A counterexample is replayed downward. If it becomes infeasible, the system learns or requests a refinement predicate at the smallest necessary grain.

## Strong observational refinement

Trace inclusion may be insufficient for randomized clients, adversarial schedulers, security properties, or hyperproperties. Strong observational refinement asks that replacement preserve all client observations under scheduler interaction. Progressive simulations provide proof principles for such properties.

Continuum should not implement this in v0, but its observer-indexed CIR and view contracts must avoid ruling it out.

## History and prophecy

Forward simulation sometimes cannot predict which abstract step a concrete concurrent operation will realize. Controlled history and prophecy variables can make the relation inductive. The system should:

- make prophecy explicit in the view contract;
- constrain prophecy choices;
- include assignments in certificates;
- never execute prophecy in production code;
- minimize prophecy scope in explanations.

## Separation and resources

Aneris, Grove, Perennial, and Trillium show that resource-oriented separation logics can verify realistic distributed, concurrent, and crash-safe systems. Continuum will not recreate Iris initially. It can borrow the engineering principle:

> compositional proof requires ownership of semantic resources, not merely disjoint Rust memory.

CIR resources and obligations can serve as a lightweight dynamic/semantic resource algebra. Long-term adapters may discharge local obligations with Verus or separation-logic proofs and expose summarized contracts to the global model checker.

## Novel proposal: Sheaf Refinement

Suppose each subsystem or observer has a local model and local witness that a trace fragment conforms. Overlaps impose compatibility constraints. Construct a presheaf:

- base space: causal/resource cover of the execution;
- sections: local abstract-state/refinement witnesses;
- restrictions: projection to overlaps.

A global refinement witness is a compatible global section. Failure to glue indicates an inconsistency that no pairwise local checker may expose. Cohomological obstructions may summarize incompatible cycles of assumptions.

### Concrete use cases

- sharded services with cross-shard transactions;
- distributed obligation ownership;
- independent node-local reconstructions of one protocol state;
- trace fragments from partial telemetry;
- composed domain packs whose local abstractions disagree at interfaces.

### Boundary between theorem and metaphor

The sheaf lane becomes real only when:

1. the cover and restriction maps are mechanically defined;
2. global sections correspond to actual refinement witnesses;
3. nonzero obstruction has a sound interpretation;
4. the method outperforms or diagnoses better than a direct CSP/SAT formulation.

Until then, it is experimental.

## Novel proposal: Semantic Blame via Minimal Ungluable Covers

When conformance fails, find a smallest subcover whose local witnesses cannot be glued. Report the corresponding components, overlap resources, and assumptions. This could yield much better diagnostics than one enormous SMT unsat core.

Benchmark against standard minimal unsat cores and causal slicing.

## Assume-guarantee contracts

Components export:

```text
assumptions over imported events/resources
guarantees over owned events/resources
safety invariants
progress guarantees
observation alphabet
fault envelope
```

Composition checks:

- ownership/resource compatibility;
- guarantee implies peer assumption;
- no circular ungrounded liveness assumptions;
- compatible fairness;
- refinement on shared observations.

The default must be conservative. Automated circular assume-guarantee inference is a research lane.

## Proof reuse and semantic diff

Content-address each model declaration, view mapping, property, and pack profile. A change analysis tracks:

- read/type/value dependencies;
- event visibility;
- fairness dependence;
- abstraction mapping;
- resource ownership.

Only claims whose semantic cone changed are invalidated. This mirrors proof/build incrementality without assuming file-level boundaries.

## Deliverables

1. Stuttering refinement checker for finite models.
2. Linearization witness support.
3. Multi-grain CEGAR prototype.
4. Component contract schema.
5. Tiny sheaf-gluing prototype on sharded examples.
6. Cross-validation with a direct SAT encoding.
7. Strong-refinement research prototype for one concurrent object.
