# ADR-0016: Add timed and probabilistic semantics as typed extensions

**Status:** Accepted in principle; deferred  
**Date:** 2026-07-24  
**Decision owners:** Continuum architecture group

## Context

Time and randomness matter for leases, retries, randomized protocols, and reliability. Mixing them implicitly into the deterministic core risks semantic confusion and unsound POR.

## Decision

Keep deterministic nondeterministic semantics foundational. Add explicit clock/probability types, schedulers, zones, MDPs, statistical claims, and specialized certificates behind separate property/engine classes.

## Consequences

The core remains comprehensible. Specialized tools can be integrated without weakening deterministic evidence.

## Alternatives considered

1. Encode probability as nondeterministic choice: loses quantitative meaning.
2. Put real-valued time everywhere: harms finite exploration.
3. Defer forever: misses important systems.

## Validation and rollback

No implementation before G4 unless a concrete project demands it. Compare against UPPAAL/IMITATOR/Storm/PRISM.
