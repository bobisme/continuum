# Benchmarks and Evaluation

## 1. Evaluation principles

1. Compare semantics, not filenames. A faster engine that checks a weaker model is not a win.
2. Publish bounds and assumptions.
3. Separate startup, exploration, checking, and certificate time.
4. Include hostile cases where each algorithm loses.
5. Reproduce every performance number from a manifest.
6. Measure diagnostic quality, migration cost, and proof burden—not only states/second.

## 2. Benchmark suites

### Suite A — Local concurrency

- bounded MPMC queue;
- lock-free stack;
- once initialization;
- cancellation-safe channel;
- semaphore obligation transfer;
- executor race/loser drain.

Comparators: Loom, Shuttle, GenMC where semantics match.

Metrics:

- schedules/classes explored;
- bug discovery;
- replay;
- memory-model coverage caveats;
- DPOR overhead.

### Suite B — Distributed safety

- two-phase commit;
- replicated register;
- primary/backup;
- Raft election/log safety;
- Paxos/Synod;
- lease/fencing service;
- membership reconfiguration;
- deduplicated request processing.

Comparators: TLC, Quint simulator/Apalache, Stateright, P when encodings are faithful.

### Suite C — Crash consistency

- write-ahead log;
- double-write buffer;
- manifest swap;
- checkpoint/recovery;
- object-store multipart commit;
- queue acknowledgement and persistence.

Metrics include durability-state cardinality and mutation score.

### Suite D — Cancellation

- cancellation during reserve;
- nested region cancellation;
- finalizer failure;
- task race;
- cancellation with storage sync;
- cancellation across request/reply obligation.

This suite is a Continuum differentiator; comparators may not have equivalent semantics.

### Suite E — Parameterized protocols

- mutual exclusion ring;
- cache coherence abstraction;
- broadcast protocols;
- Paxos variants;
- quorum systems;
- token protocols.

Comparators: Ivy, IC3PO, P verifier, CHC solvers.

### Suite F — Liveness

- leader election;
- retry under eventual delivery;
- fair lock;
- two-phase termination;
- cancellation quiescence;
- starvation defect;
- unfair scheduler counterexample.

### Suite G — Timed/probabilistic

- lease expiry with skew;
- timeout/retry race;
- randomized consensus toy models;
- failure probability under replication;
- rare split-brain scenario.

Comparators: UPPAAL/IMITATOR, Storm/PRISM.

### Suite H — Production trace conformance

- concurrent queue traces;
- storage transaction traces;
- staging cluster traces;
- deliberately incomplete traces;
- timestamp uncertainty;
- instrumentation loss.

Comparator: OmniLink-style total-order solving and conventional linearizability checkers where appropriate.

## 3. Core metrics

### Correctness

- known mutants killed;
- false positives;
- differential mismatches;
- certificate rejection/acceptance;
- replay mismatch;
- property-preservation tests.

### Search efficiency

- generated events;
- explored configurations;
- explored interleavings;
- equivalence classes;
- state count;
- reduction ratio;
- time to first violation;
- exhaustive wall time;
- peak RSS;
- bytes per state/configuration;
- parallel scaling.

### Proof efficiency

- invariant size;
- certificate size;
- checker time;
- solver time;
- human annotations;
- failed proof attempts;
- parameterized scope.

### Conformance

- instrumentation overhead;
- observed-event volume;
- valid completion time;
- minimal core size;
- false “inconclusive” rate;
- production-to-Lab reproduction rate.

### Usability

- lines deleted from bespoke DST;
- model LOC;
- abstraction/refinement LOC;
- time to first useful invariant;
- number of semantic concepts users must understand;
- diagnostic task completion;
- CI flake rate.

## 4. Benchmark manifest

```yaml
id: raft-election-3
semantic_epoch: 1
model: models/raft_election.ctm
scope:
  nodes: 3
  terms: 3
faults:
  crashes: 1
  partitions: 1
properties:
  - ElectionSafety
engines:
  - reference-explicit
  - dpor
  - unfold
  - smt-bmc
comparators:
  - tlc
  - apalache
expected:
  mutants_killed: [stale-vote, double-leader]
```

## 5. Fair comparator rules

For each comparator:

- document translation;
- verify initial/successor equivalence on small scopes;
- use equivalent fairness and fault assumptions;
- disclose unsupported constructs;
- run recommended configuration;
- report both raw and normalized results.

No “Rust vs Java” marketing benchmark without data-layout and semantic analysis.

## 6. Performance gates

### Replay

- 100% stable on retained cases;
- deterministic semantic digest across worker counts;
- replay startup under an agreed interactive threshold for small cases.

### DPOR

- zero reachability mismatch versus exhaustive baseline on corpus;
- overhead below 20% on serial/no-concurrency cases or auto-disable;
- meaningful reduction on concurrency-heavy suite.

### Explicit engine

- exact collision handling;
- deterministic trace;
- memory use competitive with TLC/Stateright on at least half the matched corpus;
- no global performance claim.

### Certificate kernel

- checker substantially simpler and faster than search;
- malformed certificates cannot panic or allocate unboundedly;
- independent mutation suite.

### Domain packs

- all declared fault behaviors reached;
- forbidden behaviors rejected;
- fidelity tests on supported host configurations.

## 7. Research kill thresholds

| Research lane | Promote threshold |
|---|---|
| cubical/HDA reduction | ≥2× memory or representative reduction on 3/5 target benchmarks without regressions >25% elsewhere |
| sheaf gluing | solves a modular case conventional assume-guarantee cannot, or cuts proof/exploration cost materially with better diagnostics |
| topological coverage | statistically useful prediction of new bug classes beyond edge/state/trace coverage |
| observer DPOR | soundness corpus clean and net win on view-heavy implementations |
| learned abstraction | checked abstractions reduce total human+compute cost on multiple projects |
| portfolio ML | improves solved instances/time, never changes claim soundness |
| rare-event guidance | orders-of-magnitude improvement for at least one realistic failure with calibrated bounds |

These are this document's benchmark-side statements of the thresholds, not
the lane authority. Plan §24.5's frontier register is authoritative for lane
status, and it records the reconciliation where this table and docs/31
disagree: the cubical row there takes docs/31's ≥3× on at least two real
protocol classes plus a preservation theorem over this table's ≥2× on 3/5
target benchmarks, and keeps this table's "no regressions >25% elsewhere"
clause, which docs/31 lacks; the abstraction row there takes this table's
total human+compute denominator over docs/31's "human work". A lane whose
row in the register is draft is not promotable on this table alone.

## 8. Artifact reproducibility

Every result bundle contains:

- source commit;
- dirty-tree patch;
- toolchain lock;
- CPU/OS metadata;
- model and pack digests;
- command;
- seed/choices;
- raw results;
- processed tables;
- certificate/check log;
- comparator versions.

## 9. Continuous benchmark tiers

- **PR smoke:** seconds/minutes, mutant subset.
- **nightly:** full safety/replay corpus.
- **weekly:** comparator matrix, scalability, symbolic.
- **release:** fresh evidence for all public claims.
- **research:** expensive campaigns, isolated from release claims.

## 10. Evaluation report structure

1. semantic equivalence;
2. correctness/mutation results;
3. bug-finding latency;
4. exhaustive performance;
5. proof/certificate burden;
6. model/implementation linkage;
7. migration cost;
8. failure analysis;
9. threats to validity;
10. exact claims supported.
