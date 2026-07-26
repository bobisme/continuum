# Test and Validation Strategy

## 1. Testing layers

```text
unit properties
→ generated semantic litmus tests
→ differential tiny exhaustive tests
→ metamorphic tests
→ mutation campaigns
→ retained crashpack replay
→ cross-engine differential harness
→ performance/evidence gates
```

The key principle is to test **semantic equivalence**, not just API outputs.

## 2. Generated transition systems

A small generator creates finite systems with:

- typed state variables;
- guarded transitions;
- explicit read/write footprints;
- independent/dependent pairs;
- conflicts;
- obligations;
- cancellation phases;
- fairness annotations.

For small sizes, enumerate all interleavings and configurations. Compare every optimized engine and reduction against this oracle.

## 3. Metamorphic relations

Expected preservation:

- alpha-renaming;
- stable reordering of declarations;
- set/map insertion order;
- splitting a deterministic action into stuttering substeps with a valid view;
- joining adjacent internal stutter steps;
- symmetry renaming;
- independent-event swap;
- equivalent guard normalization;
- serialization round trip;
- snapshot restore versus root replay.

Expected non-preservation tests deliberately alter observations, fairness, or effect phases.

## 4. Mutation testing

### Semantic engine

- drop causal edge;
- declare conflicting events independent;
- skip obligation discharge;
- use hash equality only;
- ignore epoch;
- map submitted to stable;
- accept unknown field as old meaning;
- omit fairness edge;
- use timestamp total order.

### Protocol corpus

- all first-demo mutants;
- stale term/epoch;
- double counting;
- non-idempotent retry;
- ack-before-durability;
- forgotten loser drain;
- timeout via silent drop;
- restart timer leak;
- recovery livelock.

A mature suite has kill expectations per engine. Not every engine must kill every mutant, but the assurance result cannot overclaim.

## 5. Differential oracles

Use independently implemented systems where semantics overlap:

- reference model evaluator versus optimized evaluator;
- exhaustive enumerator versus DPOR;
- TLC/Quint/Apalache for model subsets;
- asupersync Lab reports versus Continuum obligation model;
- Kani/Verus/other Rust tools for local components;
- SAT/SMT solvers and proof checkers;
- external temporal/probabilistic tools for extension lanes.

Disagreement halts the relevant claim and creates a minimized fixture.

## 6. Fuzzing

- parser and canonical encodings;
- CIR validator;
- certificate formats;
- domain-pack commands/faults;
- trace importers;
- solver proof parsers;
- replay state machine;
- schema migrations.

Fuzzing is resource-limited and includes malicious cyclic/oversized artifacts.

## 7. Determinism matrix

Test each retained scenario over:

- worker counts 1, 2, 8, 32;
- debug/release;
- supported OS/architectures;
- different hash seeds where internal structures allow;
- snapshot intervals;
- root versus snapshot replay;
- process restarts.

Semantic artifacts must be identical or differences must be explicitly non-semantic and normalized away.

## 8. Performance regression

Benchmarks have confidence intervals, warm/cold distinctions, and hardware manifests. Performance claims require:

- comparison at equal semantics/assurance;
- time to first bug and full closure;
- memory/certificate overhead;
- variance across runs;
- raw data retention.

## 9. Security validation

- dependency and `unsafe` audits;
- malformed untrusted trace/certificate inputs;
- denial-of-service limits;
- path traversal in crashpacks;
- solver sandboxing;
- secret-redaction tests;
- signature/provenance verification.

## 10. Release gates

No release promotes a capability from experimental unless:

- schemas stable for that semantic epoch;
- claims matrix evidence updated;
- all known semantic disagreements resolved/documented;
- replay corpus green;
- mutation thresholds met;
- unsupported cases explicit;
- migration path tested.
