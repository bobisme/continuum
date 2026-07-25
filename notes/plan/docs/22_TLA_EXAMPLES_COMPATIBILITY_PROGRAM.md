# TLA+ Examples Compatibility Program

## Mandate

Continuum 1.0 must provide native semantic equivalents for every CI-validated family in the pinned `tlaplus/Examples` corpus. This is the strongest practical defense against building a beautiful system that handles only Continuum-shaped problems.

The upstream repository states that it is simultaneously:

- an example library;
- a diverse development/testing corpus for TLA+ tools;
- a collection of formal-specification case studies.

Continuum adopts all three roles.

## Snapshot

**Pinned commit:** `91c22ea537853196ed1e03e9ad91693ec37642de`  
**Validated families:** 80  
**Additional tracked examples:** 39  
**README-indicated proof-bearing families:** 34  
**PlusCal or PlusCal-variant families:** 22  
**TLC-model families:** 77  
**Apalache-indicated families:** 28

Counts are generated from `corpus/tla-examples/validated-examples.csv`, transcribed from the pinned README. The Tribunal's first bootstrap task is to regenerate these facts directly from upstream manifests and fail on drift.

## Why this changes the architecture

The corpus proves that “replace TLA+” is not equivalent to “build an explicit-state protocol checker.” It forces:

1. arbitrary mathematical abstraction;
2. procedural and relational authoring;
3. finite and symbolic domains;
4. stuttering and temporal logic;
5. fairness and liveness;
6. model configuration, symmetry, views, aliases, constraints, and deadlock policy;
7. refinement with auxiliary variables;
8. theorem proving;
9. distributed, shared-memory, storage, network, probabilistic, and hardware domains;
10. deliberately failing models.

This motivates the semantic-fragment architecture and the Lean proof plane.

## Port artifact

Every family gets a directory with five independent layers:

```text
upstream source lock
native Continuum model
behavioral correspondence
oracle/evidence artifacts
Lean theorem package (when required)
```

Selected distributed/concurrent cases add an asupersync implementation and P5 refinement evidence.

## Equivalence criteria

### Values

A correspondence maps TLA+ values to canonical Continuum values. It must preserve equality, set/function membership, record/sequence structure, and model-atom identity required by the model.

### States

State correspondence may be:

- bijective;
- projected through auxiliary-variable erasure;
- relational where prophecy/history variables are involved;
- quotient-based under symmetry.

### Actions

Action labels may map one-to-one, many-to-one, or to stuttering. A hidden concrete action is not ignored informally; it is included in the refinement contract.

### Behaviors

For P3, correspondence covers infinite behaviors, stuttering, fairness, and temporal acceptance. Finite trace agreement alone is insufficient.

### Proofs

For P4, theorem intent and dependencies are mapped to Lean. The proof can be structurally different. What matters is the theorem over the corresponding semantics and a checked bridge to the executable model.

## Tribunal architecture

```text
Pinned TLA source/config
        │
        ├─ TLC/SANY oracle
        ├─ Apalache oracle
        └─ TLAPS proof outcome
                 │
                 ▼
         Corpus Oracle IR
                 │
Native CML ─ reference evaluator ─ optimized engines
                 │
                 ▼
     parity relations and minimized diffs
                 │
                 ▼
        evidence ledger + dashboard
```

The oracle IR is not CIR. Corpus Oracle IR describes state/action/value/temporal facts from foreign tools. CIR remains Continuum's native causal execution representation.

## Feature-driven scheduling

Ports are scheduled to retire unknowns:

- Wave 0 freezes values, actions, stuttering, BFS and basic proofs.
- Wave 1 freezes procedural lowering, deadlock, fairness and shared-memory symmetry.
- Wave 2 freezes asynchronous messaging, topology and termination.
- Wave 3 freezes consensus, Byzantine adversaries, transactions and parameterized induction.
- Wave 4 freezes storage, probability and larger composition.
- Wave 5 proves integrated scalability on TCP, German protocol, and TLC's own checker model.

## How ports are validated

Each port must include:

- positive reference run;
- property mutation that must fail;
- transition mutation that changes the expected graph/verdict;
- semantic metamorphisms;
- at least one minimized disagreement fixture created during development;
- deterministic reproduction;
- performance budget and state-space facts.

## What we do not promise

- automatic translation of all TLA+ source in 1.0;
- identical error messages;
- identical search order;
- equal internal state count when auxiliary encodings differ;
- a Lean translation of every TLAPS proof script;
- support for arbitrary Java operator overrides.

We promise equivalent formal work and explicit differences.

## 1.0 acceptance dashboard

The release dashboard must show all 80 rows with:

```text
P0 inventory          80/80
P1 native port         80/80
P2 finite parity       all rows requiring P2+
P3 temporal/refinement all rows requiring P3+
P4 theorem parity      all proof-bearing required rows
P5 runtime exemplars   selected minimum set across domains
unclassified diffs     0
vacuous properties     0
replay failures        0
```
