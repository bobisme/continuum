# ADR-0011: Represent external systems as versioned semantic domain packs

**Status:** Accepted  
**Date:** 2026-07-24  
**Decision owners:** Continuum architecture group

## Context

Bespoke DSTs repeat network, storage, clock, process, and service fakes. Generic mocks hide fidelity and fault assumptions.

## Decision

A domain pack includes ideal, Lab, and production handlers; operation schemas; event phases; fault algebra; independence; abstraction; fidelity profile; tests; and mutants.

## Consequences

Substantial reuse and explicit assurance become possible. Pack development is expensive and domain expertise is required.

## Alternatives considered

1. Simple traits with mocks: too weak.
2. Monolithic world simulator: poor modularity.
3. Assume host libraries are correct: no fault semantics.

## Validation and rollback

G1 proves the contract by replacing one real DST. A second project must use packs without engine modifications.

(G1 here refers to the Revision 2 gate scheme, docs/26 — not citable without translation to the docs/52 Revision 3 gates per plan §22.)
