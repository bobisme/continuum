# ADR-0017: Treat solvers as accelerators unless their evidence is checked

**Status:** Accepted  
**Date:** 2026-07-24  
**Decision owners:** Continuum architecture group

## Context

SMT/SAT/CHC solvers are essential but large and fallible. Solver diversity is useful but does not shrink the formal TCB.

## Decision

SAT models are replayed. UNSAT results are `CHECKED_CERTIFICATE` only when proof artifacts are independently checked; otherwise they are `TRUSTED_SOLVER`. Solver invocations are pinned and preserved.

## Consequences

Honest claims may appear weaker than other tools, but users can see the actual trust boundary.

## Alternatives considered

1. Trust solver unconditionally: common but inconsistent with proof-carrying goal.
2. Implement all solvers: infeasible.
3. Avoid solvers: cripples symbolic verification.

## Validation and rollback

Alethe/Carcara and LRAT-style paths are first targets. Unsupported theory proofs remain explicitly solver-trusted.
