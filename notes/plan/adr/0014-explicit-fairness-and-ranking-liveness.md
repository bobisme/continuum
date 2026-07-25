# ADR-0014: Use explicit fairness plus automata and ranking-function liveness lanes

**Status:** Accepted  
**Date:** 2026-07-24  
**Decision owners:** Continuum architecture group

## Context

TLA+-class replacement requires liveness. Finite SCC checking and symbolic/parameterized liveness need different techniques, and hidden global fairness is dangerous.

## Decision

Fairness is typed and scoped. Finite models use temporal automata/SCC/Emerson-Lei conditions. Symbolic and parameterized models use ranking/progress reductions where applicable. Certificates expose fairness and progress evidence.

## Consequences

Liveness becomes explainable and connected to runtime responsiveness. It remains one of the largest research risks.

## Alternatives considered

1. Safety only: cannot replace TLA+.
2. Finite-run timeouts: testing, not liveness.
3. One global fair scheduler: obscures unrealistic assumptions.

## Validation and rollback

G4 requires known liveness bugs, fair-cycle diagnostics, ranking proofs, and progressive refinement. Unsupported liveness fragments remain inconclusive.
