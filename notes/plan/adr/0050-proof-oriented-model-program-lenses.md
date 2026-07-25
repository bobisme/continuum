# ADR 0050: Proof-Oriented Model/Program Lenses

## Status
Accepted.

## Context
Automatic synchronization can hide ambiguity and create incorrect abstractions. Manual model/code drift is also costly.

## Decision
Represent correspondence as verified/checked bidirectional transformations with consistency relations, complements/provenance, ambiguity conditions, effects, and proof obligations. Reverse updates return alternatives/conflicts when underdetermined.

## Consequences
- no silent model/code rewrite;
- generated edits are proposals;
- correspondence becomes inspectable/incremental;
- lens laws are necessary but not sufficient; semantic preservation is required.

## Evidence required
Round-trip, ambiguity, drift, and refinement-preservation tests; Lean seed theorems.
