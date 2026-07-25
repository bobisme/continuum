# Continuum Executive Specification — Revision 2

## Mission

Continuum shall make formal modeling, deterministic simulation, systematic concurrency exploration, implementation refinement, proof, and production conformance coherent execution modes of one concurrent-Rust engineering environment.

It shall replace, for supported projects, the repeated need for:

- a separate TLA+/Quint model;
- bespoke deterministic-simulation infrastructure;
- Loom/Stateright-like one-off harnesses;
- handwritten model-based tests;
- ad hoc production trace mapping;
- unverifiable “the checker said okay” success claims.

## Semantic triptych

```text
MODEL                         PROGRAM
abstract behavior             real asupersync Rust
      \                       /
       \                     /
        \                   /
               PROOF
       Lean semantics/certificates
```

- The **Model** is useful before code and chooses the correct abstraction.
- The **Program** is actual production logic under controlled or real effects.
- The **Proof** independently defines and checks strong claims.

CML is the model language. CIR is the causal interchange for concrete execution. Lean is the metatheory/certificate authority. Asupersync is the native concrete execution substrate.

## Release contract

Continuum 1.0 requires native semantic equivalents for all **80 CI-validated TLA+ Examples families** at their declared P0–P5 parity level. The corpus is pinned by commit and used as:

- language expressiveness test;
- model-checking and liveness test;
- proof-pattern test;
- PlusCal/procedural-lowering test;
- configuration/symmetry/refinement test;
- mutation benchmark;
- performance corpus.

Semantic parity does not require copying TLA+ syntax or TLAPS proof scripts. It requires preserving the intended behavior and claims under an explicit correspondence.

## Constitutional rules

1. **Model, Program, and Proof remain independently meaningful.**
2. **No engine implementation defines semantics accidentally.**
3. **No strongest claim is self-certified.**
4. **Partial orders are primary for concrete concurrency.**
5. **Standalone abstract models are mandatory.**
6. **Fairness and assumptions are explicit and attached to named actions/environment choices.**
7. **No ambient nondeterminism in controlled code.**
8. **Exact equality or collision resolution is mandatory in proof lanes.**
9. **Replay is a compatibility surface.**
10. **Production monitoring may return `Inconclusive`.**
11. **Foreign TLA+/solver tools are evidence producers, not hidden runtime dependencies.**
12. **Proof receipts expose transformations, epochs, checker identity, Lean theorems, and axioms.**
13. **Property/assumption changes are privileged semantic operations.**
14. **Experimental mathematics must beat a simpler baseline and carry a preservation story.**

## Native capabilities

### Abstract design

- mathematical states, sets, maps, relations, sequences, graphs;
- nondeterministic relational actions;
- process/procedural surface lowered to actions;
- invariants, temporal properties, fairness, deadlock policy;
- modules, parameters, refinement, auxiliary/history/prophecy variables.

### Real-code verification

- asupersync tasks, regions, capabilities, obligations and cancellation;
- virtual network, time, storage, process lifecycle and faults;
- deterministic replay and snapshots;
- schedule and causal exploration;
- exact/minimized crashpacks.

### Verification portfolio

- simulation and coverage-guided faults;
- explicit-state search;
- source/optimal/observer-indexed DPOR;
- unfoldings and experimental higher-dimensional reduction;
- bounded symbolic checking and PDR/CHC;
- symmetry, cutoffs, counter abstraction and nominal techniques;
- SCC/lasso/ranking liveness;
- assumption games;
- timed, probabilistic, weak-memory and hyperproperty lanes.

### Proof/evidence

- finite closure and invariant certificates;
- refinement simulations;
- reduction witnesses;
- fair-SCC and ranking certificates;
- SAT/PB/solver proof import through verified encodings;
- Lean theorem receipts and axiom manifests;
- optional independent Lean environment checking.

### Production

- partial-order trace conformance;
- uncertainty-aware verdicts;
- instrumentation synthesis;
- production-to-Lab reproduction.

## Assurance lattice

Continuum never collapses all evidence to “passed.” Representative classes:

```text
example
sampled campaign
bounded schedules
DPOR-complete under declared dependence
finite exact closure
symbolic bounded
inductive/parameterized
liveness proof
refinement proof
Lean theorem receipt
production observation
inconclusive
```

Every result records bounds, assumptions, observers, semantic epochs, pack profiles, trusted components, and limitations.

## Initial wedge

The first end-to-end demonstration is a durable replicated register:

- standalone model;
- real asupersync implementation;
- crash/restart, network, storage, timer and cancellation effects;
- deliberate acknowledgement-before-sync defect;
- observer-indexed causal exploration;
- minimized replayable failure;
- concrete-to-abstract stuttering refinement;
- independently checked finite/Lean receipt.

## Scope boundaries

Continuum does not promise push-button proof of arbitrary Rust, automatic discovery of the correct abstraction, universal liveness automation, exhaustive unbounded checking, or conclusive monitoring from insufficient evidence.

Its promise is narrower:

> one coherent semantic system, explicit abstraction/refinement, aggressive but checkable analysis, honest assurance, and first-class developer/agent workflows.

## Success test

The project has replaced the practical need for TLA+ in its target projects when a user can:

1. model a protocol before implementation;
2. analyze safety and liveness;
3. implement it in asupersync;
4. check the implementation refines the model;
5. reproduce and repair failures causally;
6. obtain independent evidence;
7. validate production traces;
8. port every validated TLA+ example family without semantic special cases.
