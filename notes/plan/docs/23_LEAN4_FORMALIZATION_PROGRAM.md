# Lean 4 Formalization Program

## Role

Lean 4 is Continuum's mathematical court of final appeal. It validates semantics, reductions, encodings, and certificates; it does not replace the Rust product or become the normal execution engine.

The dossier pins `v4.32.1`, observed as the current stable patch on 2026-07-24. The pin advances only through a proof epoch that rebuilds all theorem packages and records any changed axioms or performance.

## The metatheory stack

```text
M0 Logic-neutral mathematics
   sets, relations, orders, finite maps, group actions

M1 Transition semantics
   init, step, reachability, behaviors, observations

M2 Temporal semantics
   stuttering, LTL-X, fairness, omega acceptance

M3 Refinement
   simulations, relations, composition, hyperproperties

M4 Causal semantics
   event structures, configurations, conflict, intervals

M5 Effect semantics
   obligations, cancellation, durability, network/storage faults

M6 Algorithms and certificates
   closure, DPOR, symmetry, SCC, ranking, solver encodings

M7 Concrete bridge
   CIR wire format and asupersync event adapter contracts
```

Dependencies point downward. Corpus proofs sit above M1–M7.

## Trusted computing base

The high-assurance claim ultimately trusts:

- Lean kernel and compiler/runtime to the degree stated by Lean's own trust model;
- Continuum's Lean definitions;
- explicit axioms reported by theorem manifests;
- certificate bytes and verified parser/checker/encoding path.

The optimized Rust explorer, SMT/SAT solver, agents, and foreign TLA+ tools are not trusted.

For especially sensitive results, Lean4Lean or another independent checker can validate generated `.olean` environments, adding diversity to the proof-checking path.

## Proof families

### Safety

Core theorem:

```text
Init ⊆ I
Post(I) ⊆ I
I ⊆ Safe
────────────
Reachable ⊆ Safe
```

Certificate instantiations:

- explicit closed set;
- symbolic inductive formula;
- quantified invariant;
- compositional invariant graph.

### Refinement

Initial core:

```text
InitC(c) ⇒ InitA(α c)
StepC(c,c') ⇒ α c = α c' ∨ StepA(α c, α c')
```

Extensions cover relational refinement, event projection, prophecy/history variables, fair refinement, and strong observational refinement for hyperproperties.

### Liveness

Finite certificates:

- absence of fair accepting SCC;
- ranking on SCC condensation or helpful transitions;
- Streett/justice progress obligations.

Symbolic certificates:

- well-founded rank relation;
- fairness premises;
- action-local decrease/nonincrease lemmas;
- proof graph linking progress obligations.

### Reduction

- symmetry quotient preserves init, transitions and property;
- observer-indexed commutation preserves observation language;
- DPOR source sets cover all relevant Mazurkiewicz classes;
- unfolding prefix is complete for target property;
- abstract interpretation is sound via Galois connection/simulation.

### Cancellation

- phase transitions are well-formed;
- every reservation is committed or aborted;
- obligation ownership is linear;
- drain/finalize preserves safety;
- under responsiveness and finite budgets, rank decreases to quiescence;
- cancellation-aware refinement maps partial effects correctly.

## Reflective certificate pipeline

Recent Lean work demonstrates the practicality of verified reflective checkers for LRAT and pseudo-Boolean certificates. Continuum follows this pattern:

1. define a Boolean checker in Lean;
2. prove `check = true → proposition`;
3. compile checker to native code;
4. verify large certificate efficiently;
5. obtain a composable Lean theorem.

The harder and more important task is the verified encoding from Continuum semantics to the solver problem.

## Proof engineering conventions

- no `sorry` in release packages;
- theorem names include semantic epoch where meaning can change;
- `#print axioms` captured in machine-readable manifests;
- proofs are sliced by action/property dependency;
- executable examples accompany abstract definitions;
- definitions are kept reducible enough for reflection but opaque where abstraction stability matters;
- proof performance is benchmarked and regression-gated;
- model-derived finite facts are imported as certificates, not enormous generated source terms.

## Corpus theorem program

The proof-bearing TLA+ examples are divided into patterns:

1. elementary arithmetic and finite combinatorics;
2. loop invariants/program correctness;
3. inductive distributed safety;
4. refinement and auxiliary variables;
5. termination/liveness;
6. graph/topology properties;
7. protocol families with reusable quorum/broadcast libraries.

The goal is not 34 unrelated proof ports. It is a reusable library where later corpus theorems collapse to instantiations of established patterns.

## Initial theorem milestones

1. finite closure safety for DieHard `TypeOK`;
2. shortest path witness validity;
3. deadlock witness validity for Dining Philosophers;
4. reserve/commit register stuttering refinement;
5. mutual exclusion invariant schema;
6. finite symmetry quotient theorem;
7. fair-lasso certificate theorem;
8. quantified quorum intersection library;
9. cancellation obligation conservation;
10. proof-producing DPOR safety theorem.

## Formalization risk controls

Lean can consume the project if every implementation detail is formalized too early. The rule is:

> Formalize semantic load-bearing walls and certificate checkers; validate changing optimizations per instance.

Each proof workstream has a product gate, executable test, theorem statement, and maximum tolerated proof-maintenance cost.
