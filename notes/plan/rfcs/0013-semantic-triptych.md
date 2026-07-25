# RFC 0013: Semantic Triptych and Cross-Path Validation

**Status:** Proposed  
**Target gate:** G0

## Problem

Continuum wants one coherent meaning across abstract models, concrete programs, and proofs. A literal single implementation would create circular validation; three unrelated implementations would drift.

## Surfaces

### Model plane

- CML AST and typed core;
- exact reference evaluator;
- abstract transition/behavior semantics;
- model configurations and finite/symbolic domains.

### Program plane

- asupersync runtime and Lab;
- domain-pack operations;
- semantic event journal;
- concrete snapshots and replay.

### Proof plane

- Lean definitions;
- certificate schemas/checkers;
- theorem manifests;
- solver encoding proofs.

## Shared artifacts

The planes may share declarative artifacts:

- versioned algebraic data schemas;
- operator tables;
- generated serialization fixtures;
- model/value test vectors;
- property IDs and source maps;
- semantic epoch hashes.

They may not all call the same evaluator to justify agreement.

## Cross-path matrix

```text
CML reference ↔ optimized Rust evaluator
CML reference ↔ TLA+ corpus oracle
Rust evaluator ↔ SMT/PDR encodings
CIR journal ↔ model/refinement checker
native certificate checker ↔ Lean reflective checker
asupersync adapter ↔ executable primitive conformance models
```

Each arrow has generated random tests, curated edge cases, and mutation tests.

## Semantic-change protocol

Changing semantics requires:

1. ADR/RFC amendment;
2. Lean definition change or proof that behavior is unchanged;
3. reference evaluator update;
4. optimized engine update;
5. corpus differential report;
6. replay migration decision;
7. semantic epoch increment if old artifacts change meaning.

No pull request may change all expected outputs and call the suite green without an explicit semantics review.

## Generated finite universes

For operator-level differential testing, a generator constructs small closed universes of values:

- booleans and bounded integers;
- small sets/functions/records/sequences;
- model atoms and permutations;
- nested expressions with definedness constraints.

The same expression/transition fixtures are evaluated by every available plane. This is more effective than waiting for full protocols to expose edge cases.

## Undefined and partial behavior

CML does not use host panics to define mathematics. Potentially undefined operations elaborate to:

- a static rejection;
- a proof obligation;
- an explicit `Undefined` semantic result that invalidates the model;
- never an arbitrary value.

The TLA+ oracle bridge records where upstream semantics are intentionally underspecified or implementation-defined.

## Assurance rule

Agreement raises confidence, not theorem level. The proof plane raises theorem level only through checked evidence. Disagreement always lowers confidence until resolved.
