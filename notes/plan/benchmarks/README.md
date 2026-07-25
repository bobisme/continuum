# Continuum Evaluation Corpus

The benchmark suite is evidence infrastructure, not a victory-lap collection. It must contain workloads favorable and hostile to every engine.

## Suites

### `micro/`

Semantic and algorithm litmus tests:

- cancellation reserve/commit;
- task ownership and quiescence;
- message loss/duplication/reordering;
- timer epochs;
- storage sync/crash;
- observer-sensitive independence;
- fairness and pure-await loops;
- exact-state hash collision injection.

### `protocols/`

- two-phase commit and presumed-abort variants;
- Raft/Paxos abstractions;
- leases under partial synchrony;
- primary/backup replication;
- membership/reconfiguration;
- distributed lock service;
- sharded transaction fragment;
- CRDT convergence;
- workflow/saga compensation;
- replicated register vertical slice.

### `concurrency/`

- channels and queues;
- lock-free structures;
- cancellation races;
- actor mailboxes;
- region/task trees;
- work-stealing internals;
- linearizability/strong refinement examples.

### `storage/`

- append log;
- write-ahead log;
- checkpoint/recovery;
- double-write/page repair;
- manifest/rename protocols;
- snapshot install;
- object-store multipart commit.

### `parameterized/`

- mutual exclusion;
- token ring;
- cache coherence abstractions;
- quorum protocols;
- lossy-channel coverability.

### `production-traces/`

- complete traces;
- partial/lossy traces;
- clock uncertainty;
- cross-epoch traces;
- known incidents and generated mutants.

### `negative/`

Cases deliberately hostile to proposed techniques:

- little/no independence;
- pathological symmetry;
- bad decision-diagram variable orders;
- SMT nonlinear arithmetic;
- huge values with tiny schedule space;
- massive schedules with tiny state;
- liveness properties invalidated by safety POR;
- topology/sheaf methods with no advantage.

## Competitors/oracles

Where licensing and automation permit:

- TLC;
- Apalache;
- Quint simulator/Quint Connect;
- Stateright;
- Loom;
- Shuttle;
- Kani;
- LTSmin;
- GenMC/Nidhugg-style DPOR artifacts;
- Ivy/IC3PO-related examples;
- Storm/UPPAAL for extension lanes.

Comparisons report semantic differences and unsupported features, not just runtime.

## Metrics

- time to first counterexample;
- unique causal classes;
- generated and distinct configurations;
- memory per configuration/class;
- replay stability;
- minimized causal-core size;
- certificate size/check time;
- multicore scaling;
- snapshot overhead;
- solver time and proof-check time;
- integration LOC and production-code divergence;
- mutant kill rate;
- false/inconclusive conformance classifications;
- user/agent time to diagnosis.

All benchmark manifests pin semantic, compiler, solver, and hardware envelopes.
