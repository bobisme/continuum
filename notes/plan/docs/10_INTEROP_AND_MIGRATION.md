# Interoperability and Migration Strategy

## 1. Principle

Continuum should win by being useful with existing formal ecosystems before asking users to abandon them. Interoperability is an oracle, migration path, and risk-control mechanism.

## 2. TLA+ interoperability

### Import

Initial path:

```text
TLA+ source
   ↓ SANY or Apalache frontend
resolved semantic representation
   ↓ adapter
Continuum typed model/CIR
```

Do not reimplement SANY before the engine has value.

Compatibility levels:

- parse/import;
- initial-state equivalence;
- successor-set equivalence;
- invariant result equivalence;
- finite liveness equivalence;
- trace round-trip;
- unsupported-feature diagnostics.

### Export

Useful exports:

- TLA+ behavior from CIR;
- model skeleton for external review;
- counterexample trace;
- finite state relation;
- proof obligation comments.

Export is not claimed to preserve every source-level construct.

### Differential Tribunal

For compatible corpus:

- enumerate initial states;
- compare normalized successor sets;
- compare state counts;
- compare invariants;
- compare counterexample existence;
- compare fairness/liveness on restricted cases.

## 3. Quint and Apalache

Priority interoperability:

- ITF trace read/write;
- Quint Connect-compatible state mappings;
- typed IR adapter if stable;
- Apalache JSON-RPC invocation;
- bounded SMT result import;
- model witness replay.

Potential strategy: use Quint as an optional frontend while Continuum's `.ctm` surface matures.

## 4. Stateright

Use cases:

- compare executable Rust models;
- import small protocol corpus;
- benchmark trace quality and state storage;
- validate linearizability-style examples.

No attempt to make Stateright models magically production code.

## 5. P

Potential interchange:

- event/machine models to CIR;
- runtime monitor traces;
- compare inductive verifier on machine-oriented protocols.

P's event queues and monitors can inform domain-pack semantics.

## 6. Rust verifier integration

### Verus/Creusot

Continuum exports local proof obligations:

- pack implementation refines operation contract;
- abstraction function purity;
- canonical encoder injectivity for a type;
- data structure invariants;
- unsafe boundary contracts.

A verified local component can become an atomic/certified summary in higher-level exploration.

### Kani/Crux

Use bounded bit-precise checking for:

- serialization;
- arithmetic;
- unsafe/FFI adapters;
- compact state encoding;
- certificate parser;
- pack micro-semantics.

### GenMC/Loom/Shuttle

Use for local scheduler/memory-model differential tests. Continuum does not claim to subsume weak-memory verification immediately.

### hax/Aeneas

Potentially translate restricted pack/kernel code to Lean/Rocq/F* for independent proofs. Avoid making these translations mandatory for baseline use.

## 7. Solver interoperability

Protocols:

- SMT-LIB 2.x;
- CHC;
- Alethe;
- LRAT/FRAT where bit-blasted;
- solver model format normalized by replay;
- proof artifacts retained.

External process isolation is default. Embedded solver libraries are optional performance features.

## 8. Trace interoperability

CIR adapters:

- OpenTelemetry spans/events;
- asupersync trace;
- ITF;
- TLA+ behavior;
- JSON event logs;
- vector-clock logs;
- linearizability histories.

Adapters must state information loss. OpenTelemetry, for example, does not automatically provide complete semantic causality or effect phases.

## 9. Migrating a bespoke DST

### Inventory

Classify existing code:

- scheduler;
- time;
- RNG;
- network;
- storage;
- process/faults;
- scenarios;
- generators;
- invariants;
- replay;
- trace visualization;
- domain fakes.

### Mapping

```text
scheduler/time/RNG/replay → asupersync Lab
network/storage/process   → standard Continuum packs
scenarios/generators      → Continuum scenario API
invariants                → properties
domain fakes              → project/domain packs
abstract model            → .ctm
```

### Differential migration

Run old and new frameworks with common scenarios:

- compare externally visible outcomes;
- compare known bug seeds;
- compare fault reachability;
- retain old system until parity.

### Deletion gate

Remove old infrastructure only when:

- known scenarios pass;
- known mutants are killed;
- replay parity is established;
- semantic exclusions are documented;
- CI workflow improves.

## 10. Compatibility policy

Every adapter has:

- supported version range;
- semantic fidelity level;
- known loss;
- conformance corpus;
- owner;
- deprecation policy.

No adapter is labeled “compatible” as a single percentage.
