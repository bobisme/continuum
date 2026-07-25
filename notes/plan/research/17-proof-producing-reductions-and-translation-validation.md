# Research 17: Proof-Producing Reductions and Translation Validation

## Threat model

An unsound optimizer in a verifier is worse than a crash: it manufactures confidence. High-risk passes include:

- property slicing;
- symmetry quotienting;
- partial-order reduction;
- abstraction;
- module composition;
- finite-domain instantiation;
- bit-blasting and solver encoding.

## Two proof strategies

### Verified transformation

Prove once in Lean that implementation function `transform` preserves a relation.

Advantages: compact per-instance evidence.  
Cost: implementation verification and maintenance.

### Translation validation

For each instance, transformation emits a witness; a small checker verifies preservation.

Advantages: optimized implementation may change.  
Cost: per-instance certificate and checker design.

Continuum should prefer translation validation early and prove stable checkers/semantics.

## Property slicing witness

Given property-variable roots, emit a dependency graph showing every omitted variable/action cannot affect:

- property value;
- enabledness of retained actions;
- fairness monitor;
- observer output;
- hidden action refinement.

A simple syntactic cone is safe but conservative. Semantic slicing needs proof obligations.

## Symmetry witness

For each canonical state:

- representative;
- permutation mapping original to representative;
- proof action relation equivariant;
- property invariant under action.

Group generators can compress evidence.

## DPOR witness

A difficult open engineering problem: produce compact global evidence that source/backtracking choices cover every relevant trace class. Candidate structure:

- exploration prefix tree;
- per-prefix source set;
- dependence relation digest;
- race/backtracking witnesses;
- local commutation diamonds;
- sleep/source-set closure proof.

The certificate checker should not re-explore all schedules, or the reduction buys nothing.

## Solver encoding witness

Each lowering step uses a verified normal form and emits mapping tables. SAT/PB proof import then proves the final formula, while Lean theorems connect formula validity to model safety.

## Incremental certificates

Content-addressed subproofs allow unchanged partitions/actions to be reused after edits. This is essential for agent loops.

## Research metric

Measure:

- certificate/search size ratio;
- check/search time ratio;
- incremental reuse rate;
- malformed-certificate detection;
- semantic mutation detection;
- proof maintenance across optimizer changes.
