# RFC 0031: Semantic and Intent Diff

## Status
Draft.

## Inputs
Two workspace snapshots, optional before/after Intent Contracts, requested assurance envelope.

## Outputs
Layered changes, relation classification, invalidated evidence, proof obligations, policy consequences, and Unknown items.

## Relation checking
Finite exact inclusion/equivalence where feasible; SMT/Lean obligations for supported symbolic fragments; otherwise no guessed ordering.

## Protected categories
Property, assumptions, observers, bounds, faults, fairness, opaque/trust boundaries, assurance, optimization/non-vacuity.

## Program changes
Effects, atomicity, task ownership, cancellation, durability, observer publication, and correspondence.

## Acceptance
Mutation corpus with known strengthened/weakened/incomparable relations.
