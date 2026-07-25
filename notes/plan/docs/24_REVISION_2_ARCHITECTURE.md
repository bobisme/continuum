# Revision-2 Architecture: The Semantic Triptych

## Core shift

Revision 1 centered CIR. Revision 2 keeps CIR but recognizes that a causal interchange format alone cannot replace TLA+ or justify its own optimizations.

The new center is a triangle of independently meaningful artifacts:

```text
                    MODEL
          mathematical transition/behavior
             /                       \
            /                         \
  corpus parity                       refinement
          /                             \
         /                               \
     PROOF ----------------------------- PROGRAM
 Lean semantics/certificates        asupersync Rust
```

CIR is the language spoken on the Program edge and by causal engines. CML typed core is the language spoken on the Model edge. Lean definitions govern theorem meaning on the Proof edge.

## Six planes

### 1. Authoring plane

- CML relational model language;
- procedural algorithm surface;
- Rust model/refinement attributes;
- TLA+/Quint importers;
- generated agent APIs.

### 2. Semantic plane

- typed values and expressions;
- transition systems and infinite behaviors;
- event structures/configurations;
- observers/views;
- temporal/fairness semantics;
- faults, obligations, cancellation and durability.

### 3. Execution plane

- exact reference evaluator;
- asupersync production and Lab adapters;
- domain packs;
- replay and snapshots;
- semantic event journal.

### 4. Verification plane

- simulation/fuzzing;
- source/optimal DPOR;
- unfoldings/HDA experiments;
- explicit and symbolic state engines;
- PDR/CHC/parameterized lanes;
- liveness and games;
- probabilistic/timed lanes.

### 5. Evidence plane

- crashpacks;
- closure/invariant/refinement/reduction certificates;
- proof-producing compiler transformations;
- Lean reflection/import;
- evidence ledger and assurance lattice.

### 6. Corpus/Tribunal plane

- TLA+ Examples parity;
- differential foreign oracles;
- operator-level semantic fuzzing;
- mutation and metamorphic suites;
- performance and proof regressions.

## Semantic fragments

CML does not lie about executability. Every expression/action/property carries fragment requirements:

```text
Finite        exact bounded evaluation
Symbolic      solver-representable mathematics
Temporal      infinite behavior/fairness
Probabilistic distributions/MDP/game semantics
Theorem       Lean-only propositions and proof obligations
Runtime       controlled concrete effects
```

Elaboration computes the least required fragment and reports why a backend applies or does not.

## One model, multiple grains

A project can define:

```text
ServiceSpec
  refines <- ProtocolSpec
  refines <- OperationalSpec
  refines <- AsupersyncProgram
  observed by <- ProductionTrace
```

Each edge carries:

- state relation/view;
- event map;
- hidden/stuttering actions;
- assumptions and fairness;
- proof strategy;
- evidence status.

This graph is a first-class build artifact. “The implementation matches the model” is never an unqualified boolean.

## Independence and observer lattice

Views/properties induce an observer lattice. A fine observer preserves more distinctions; a coarse observer permits more commutations and abstraction.

This lattice drives:

- property-specific DPOR;
- trace canonicalization;
- state slicing;
- refinement granularity;
- production observability requirements;
- cache reuse.

The relationship is formalized so performance optimization remains subordinate to semantics.

## Proof-producing pipeline

Every semantic pass declares its preservation mode:

```text
source
 → elaborated core       equivalence
 → finite instance       instantiation theorem
 → property slice        property preservation
 → symmetry quotient     bisimulation
 → DPOR/unfolding         trace/property preservation
 → solver formula        equisatisfiability
 → proof certificate     checked theorem
```

The user sees the chain attached to the result.

## Why this can replace bespoke DST and TLA+

DST replacement comes from the Program/Execution/CIR side:

- controlled time, scheduling, faults, network, storage, cancellation, replay.

TLA+ replacement comes from the Model/Temporal/Proof side:

- abstract authoring before code, arbitrary mathematical views, exhaustive/symbolic checking, fairness/liveness, refinement, proofs.

The triangle connects them without requiring duplicated hand-maintained models.
