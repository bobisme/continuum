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

- typed state variables; (delivered: bn-1fqu — tools/test-policy/evidence/s2_generated_systems.json)
- guarded transitions; (delivered: bn-1fqu — tools/test-policy/evidence/s2_generated_systems.json)
- explicit read/write footprints; (delivered: bn-1fqu — tools/test-policy/evidence/s2_generated_systems.json)
- independent/dependent pairs; (delivered: bn-1fqu — tools/test-policy/evidence/s2_generated_systems.json)
- conflicts; (delivered: bn-1fqu — tools/test-policy/evidence/s2_generated_systems.json)
- obligations; (delivered: bn-1fqu — tools/test-policy/evidence/s2_generated_systems.json)
- cancellation phases; (delivered: bn-31iu — tools/test-policy/evidence/s2_cancellation_fairness.json)
- fairness annotations. (delivered: bn-31iu — tools/test-policy/evidence/s2_cancellation_fairness.json)

For small sizes, enumerate all interleavings and configurations. Compare every optimized engine and reduction against this oracle.

## 3. Metamorphic relations

Expected preservation:

- alpha-renaming; (delivered: bn-2sn5 — tools/test-policy/evidence/s3_metamorphic_relations.json)
- stable reordering of declarations; (delivered: bn-2sn5 — tools/test-policy/evidence/s3_metamorphic_relations.json)
- set/map insertion order; (delivered: bn-2sn5 — tools/test-policy/evidence/s3_metamorphic_relations.json)
- splitting a deterministic action into stuttering substeps with a valid view; (delivered: bn-2sn5 — tools/test-policy/evidence/s3_metamorphic_relations.json)
- joining adjacent internal stutter steps; (delivered: bn-2sn5 — tools/test-policy/evidence/s3_metamorphic_relations.json)
- symmetry renaming; (delivered: bn-2sn5 — tools/test-policy/evidence/s3_metamorphic_relations.json)
- independent-event swap; (delivered: bn-1rt2 — tools/test-policy/evidence/s3_metamorphic_relations_07_10.json)
- equivalent guard normalization;
- serialization round trip; (delivered: bn-1rt2 — tools/test-policy/evidence/s3_metamorphic_relations_07_10.json)
- snapshot restore versus root replay.

Expected non-preservation tests deliberately alter observations, fairness, or effect phases.

## 4. Mutation testing

### Semantic engine

- drop causal edge; (delivered: bn-1ccq — tools/test-policy/evidence/s4_mutation_testing.json)
- declare conflicting events independent; (delivered: bn-1ccq — tools/test-policy/evidence/s4_mutation_testing.json)
- skip obligation discharge; (delivered: bn-1ccq — tools/test-policy/evidence/s4_mutation_testing.json)
- use hash equality only;
- ignore epoch;
- map submitted to stable;
- accept unknown field as old meaning; (delivered: bn-3mo3 — tools/test-policy/evidence/s4_wire_fairness_mutants.json)
- omit fairness edge; (delivered: bn-3mo3 — tools/test-policy/evidence/s4_wire_fairness_mutants.json)
- use timestamp total order.

### Protocol corpus

- all first-demo mutants; (delivered: bn-3mo3 — tools/test-policy/evidence/s4_wire_fairness_mutants.json)
- stale term/epoch;
- double counting; (delivered: bn-3mo3 — tools/test-policy/evidence/s4_wire_fairness_mutants.json)
- non-idempotent retry; (delivered: bn-2dt2 — tools/test-policy/evidence/s4_protocol_corpus.json)
- ack-before-durability; (delivered: bn-2dt2 — tools/test-policy/evidence/s4_protocol_corpus.json)
- forgotten loser drain;
- timeout via silent drop;
- restart timer leak; (delivered: bn-2dt2 — tools/test-policy/evidence/s4_protocol_corpus.json)
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
- external temporal/probabilistic tools for extension lanes. (deferred: plan.md §24.5 register row "Timed/probabilistic semantics" — ADR-0016 lane not opened, defer post-1.0; bn-3iwa enforces the ADR-0029 registration contract and a drift guard in tools/test-policy/sections/s5_extension_lane_oracles.py meanwhile)

Disagreement halts the relevant claim and creates a minimized fixture.

## 6. Fuzzing

- parser and canonical encodings; (delivered: bn-1zb9 — tools/test-policy/evidence/s6_fuzzing_targets.json)
- CIR validator;
- certificate formats; (delivered: bn-1zb9 — tools/test-policy/evidence/s6_fuzzing_targets.json)
- domain-pack commands/faults;
- trace importers;
- solver proof parsers; (delivered: bn-1zb9 — tools/test-policy/evidence/s6_fuzzing_targets.json)
- replay state machine; (delivered: bn-215u — tools/test-policy/evidence/s6_fuzzing_targets_07_08.json)
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
