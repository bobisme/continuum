# ADR-0007: Use a portfolio of verification engines over one semantics

**Status:** Accepted  
**Date:** 2026-07-24  
**Decision owners:** Continuum architecture group

## Context

Explicit search, DPOR, decision diagrams, BMC, PDR, liveness rankings, timed zones, and probabilistic methods have complementary performance envelopes. Selecting one algorithm would cap the system.

## Decision

Define stable semantic/partitioned-next-state interfaces and let several engines compete or cooperate. A portfolio scheduler can allocate resources, but each engine reports typed evidence and cannot elevate assurance without proof.

## Consequences

The system is more capable and can exploit model structure. Integration and maintenance cost rise; therefore engines are promoted individually behind gates.

## Alternatives considered

1. Build only a fast explicit checker: insufficient for unbounded/liveness work.
2. Shell out to unrelated tools without shared semantics: workflow collage and inconsistent evidence.
3. One universal symbolic IR: risks forcing all models into solver-friendly forms.

## Validation and rollback

Baseline engines are reference explicit, simulation, and DPOR. Later engines must demonstrate unique solved cases or practical wins before becoming core dependencies.
