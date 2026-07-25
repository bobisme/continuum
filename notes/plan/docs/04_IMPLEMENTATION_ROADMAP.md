# Implementation Roadmap — Revision 2

## Principle

Sequence by **risk retirement and evidence closure**, not by subsystem prestige. The implementation order is gate-driven; it is not a delivery-time promise.

## Gate graph

```text
G0A model/reference semantics ─┐
G0B Lean proof foundation ─────┼→ G1 complete replicated-register slice
G0C asupersync/CIR bridge ─────┤
G0D corpus Tribunal ───────────┘
                                 ↓
G2 Wave-0 language stability → G3 real DST replacement
                                 ↓
G4 temporal/refinement → G5 symbolic/production
                                 ↓
G6 all 80 validated families → 1.0
```

## Workstreams

### W1 — CML and typed core

Parser, formatter, diagnostics, value algebra, actions, modules, temporal properties, fragments and refinement declarations.

### W2 — Reference semantics

Exact deterministic evaluator, graph exploration, shortest traces, finite certificates and semantic fuzzing.

### W3 — Lean metatheory

Reachability, temporal/fairness, refinement, event structures, cancellation, reduction and reflective certificate theorems.

### W4 — CIR/asupersync

Semantic event sink, lifecycle/obligation mapping, replay, snapshots and coverage.

### W5 — Explicit/causal engines

Parallel exact checking, DPOR, unfoldings, symmetry, nominal canonicalization and minimization.

### W6 — Temporal/games

Fair lasso/SCC, ranking, parity/Streett and environment-assumption synthesis.

### W7 — Symbolic/parameterized

BMC, k-induction, PDR/CHC, cutoffs, counter abstraction, regular/nominal lanes and solver evidence.

### W8 — Corpus Tribunal

80-family inventory, foreign oracles, port manifests, graph/trace parity, mutation suite and dashboard.

### W9 — Domain packs

Network, storage, process, time, synchronization, database and weak-memory profiles.

### W10 — Production conformance

Partial-order matching, uncertainty, instrumentation synthesis and Lab reproduction.

### W11 — DX/agents

CLI, daemon, LSP, visualizer, proof blueprints, semantic diff, generated Rust interfaces and repair loop.

## Corpus milestones

- **Wave 0:** finite search, proof basics, elementary concurrency;
- **Wave 1:** mutual exclusion, barriers, readers/writers, auxiliary variables;
- **Wave 2:** fairness, termination, graphs and reachability;
- **Wave 3:** consensus, Byzantine protocols, transactions and refinement;
- **Wave 4:** storage, TCP, cache coherence, lock-free and complex operational models;
- **Wave 5:** language/meta edge cases.

Each wave hardens reusable semantics and libraries. It is not 80 isolated ports.

## Proof milestones

1. closure/invariant theorem;
2. stuttering refinement and composition;
3. fair-lasso/SCC certificate;
4. symmetry/observer preservation;
5. cancellation/obligation conservation;
6. solver encoding and reflective import;
7. parameterized/cutoff theorem patterns;
8. corpus proof families.

## First vertical slice

The replicated-register slice is complete only when abstract model, real code, faults/cancellation, CIR, exploration, minimization, replay, refinement and proof receipt all exist. A partial implementation cannot be labeled G1.

## API freeze rules

Do not freeze:

- higher-order model semantics before corpus pressure;
- observer-indexed DPOR before exhaustive differential evidence;
- proof-receipt binary layout before first Lean import;
- adapter hooks before real asupersync execution;
- production trace schema before uncertainty experiments;
- nominal representation before fresh-name benchmarks.

## 1.0 gate

All 80 validated TLA+ example families meet required parity; proof-bearing claims have Lean receipts; selected runtime refinements cover major domains; critical claims have current reproducible evidence; no hidden foreign fallback exists.
