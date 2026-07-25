# Weak Memory as Execution Graphs

## Problem

Distributed protocol verification and local lock-free verification operate at different scales. Treating every local atomic operation as sequentially consistent can miss real Rust executions; exploring the full memory model inside every distributed schedule is intractable.

## Proposed separation

Continuum uses hierarchical verification:

```text
local concurrent component
   weak-memory execution graph
       ↓ refines
atomic component contract
       ↓ used by
protocol/distributed model
```

The local lane establishes that a queue, channel, log buffer, or synchronization primitive refines an atomic contract under a named Rust/compiler/hardware memory model. The distributed lane consumes the contract.

## Execution graph

A candidate execution contains:

- events and thread/task identity;
- program order (`po`);
- reads-from (`rf`);
- modification/coherence order (`mo`);
- from-read (`fr`);
- synchronizes-with (`sw`);
- happens-before (`hb`);
- dependency edges and fences;
- atomic ordering annotations.

A memory model is a set of acyclicity/irreflexivity and consistency constraints over these relations.

## Engine strategy

1. Instrument or lower a restricted Rust concurrency IR.
2. Generate candidate event graphs symbolically.
3. Use SAT/SMT to solve reads-from and coherence choices.
4. Import proof-producing SAT/PB evidence where possible.
5. Map externally visible operations to the atomic component model.
6. Check linearization/refinement.

## Relationship to Loom

Loom remains valuable for controlled schedule exploration and for testing Continuum internals. Continuum’s weak-memory lane should not claim Loom’s approximation is a complete C11/Rust semantics. The lane may consume Loom traces as bug seeds while independently checking the selected axiomatic model.

## State-space control

- verify local components separately;
- collapse verified operations to atomic summaries;
- use observer-indexed event slicing;
- exploit coherence-class symmetry;
- bound data values through abstraction;
- use CEGAR when a weak-memory witness is spurious at the protocol level.

## Corpus pressure

The Disruptor, lock-free, and shared-memory examples can initially be ported under SC semantics. Runtime refinement exemplars must state whether SC, Rust atomics, compiler lowering, or hardware behavior has been justified.

## Promotion criterion

Promote beyond SC when the lane reproduces known litmus tests, finds or excludes mutants in real Rust components, and emits independently checkable solver evidence. Never merge weak-memory and distributed scheduling into one uncontrolled Cartesian product.

## Primary references

- RustMC as a current compiled-Rust/GenMC direction: [S81];
- weak-memory formalism taxonomy and execution-graph vocabulary: [S115];
- Loom remains an engineering baseline, not the normative weak-memory semantics: [S12].
