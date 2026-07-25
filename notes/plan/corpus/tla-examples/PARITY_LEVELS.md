# TLA+ Corpus Parity Levels

Continuum is not required to parse every TLA+ source file in its first releases. It is required to reproduce the semantic work performed by every example.

## P0 — inventoried

The source closure, model configurations, required modules, proof files, expected model-checking mode, and observed upstream result are pinned.

P0 makes no compatibility claim.

## P1 — native behavioral port

A native Continuum model expresses the same problem and intended property. It can be evaluated, simulated, and rendered with source correspondence.

Required evidence:

- model loads and type/effect checks;
- at least one expected behavior or counterexample;
- state and action correspondence document;
- no hidden foreign semantic adapter.

## P2 — bounded semantic parity

For every pinned finite model configuration, Continuum and the upstream oracle agree on the observable finite semantics.

Depending on the model, parity is established by one or more of:

- exact normalized reachable-state graph isomorphism;
- strong or stuttering bisimulation under an explicit state projection;
- equal reachable observation sets;
- equal deadlock set;
- equal safety verdict and minimal counterexample depth;
- equal trace-language hash for bounded depth;
- equal symmetry quotient after canonicalization;
- equal model-constraint/view behavior.

Raw state counts are evidence, not the definition: auxiliary variables or different encodings can legitimately change internal graph size.

## P3 — temporal and refinement parity

The native model has equivalent stuttering, fairness, liveness, temporal-property, and refinement behavior.

Required evidence:

- named weak/strong fairness correspondence;
- accepted fair-cycle or ranking certificate;
- leads-to and temporal formula correspondence;
- explicit treatment of finite prefixes versus infinite behaviors;
- refinement map and auxiliary/history/prophecy variable correspondence where applicable;
- model-checking agreement on intentionally failing liveness cases.

## P4 — theorem parity

For examples carrying TLAPS proofs or mathematical proof content, the corresponding key theorem is checked in Lean 4.

P4 does **not** require line-by-line translation of a TLAPS proof. It requires:

- a formally stated theorem over the Continuum semantics;
- a sorry-free Lean proof or a verified certificate imported by a proved checker;
- explicit axioms and classical principles reported by `#print axioms`;
- a bridge theorem connecting executable encodings to the mathematical statement.

## P5 — implementation refinement exemplar

Selected concurrent/distributed examples have a real asupersync implementation and machine-checked evidence that observed concrete steps refine the native model.

Required evidence:

- same protocol logic under Lab and production runtimes;
- semantic event instrumentation coverage;
- stuttering/refinement map;
- deterministic failure replay;
- schedule/fault exploration;
- independent certificate or Lean theorem for the claimed bounded result.

## Release contract

- **0.1:** ten P2 examples spanning values, actions, stuttering, deadlock, and shortest counterexamples.
- **0.2:** twenty-five P2 examples, five P3 examples, three P4 theorems.
- **0.5:** all Wave 0–2 examples at required level; at least five P5 exemplars.
- **1.0:** all 80 CI-validated examples at their `required_parity` level in `validated-examples.csv`.
- **1.x:** all accessible in-tree additional examples, followed by curated external cases.

A release may declare a precisely scoped unsupported feature, but may not count that example as passing.
