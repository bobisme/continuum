# ADR-0015: Distinguish ordinary refinement from hyperproperty-preserving refinement

**Status:** Accepted  
**Date:** 2026-07-24  
**Decision owners:** Continuum architecture group

## Context

Trace inclusion/linearizability does not preserve all probability distributions or security hyperproperties. Calling every mapping “refinement” is misleading.

## Decision

Continuum defines trace, stuttering, forward, progressive, and strong observational refinement classes. A property declares which class is sufficient. Hyperproperty claims use dedicated engines and proof obligations.

## Consequences

Results become semantically precise. Stronger refinement can be much harder or impossible for some implementations.

## Alternatives considered

1. Ignore hyperproperties: acceptable only for systems without those claims.
2. Assume linearizability preserves everything: unsound.
3. Always demand strongest refinement: impractical and unnecessarily restrictive.

## Validation and rollback

Initial implementation supports finite trace/forward simulation. Hyperproperty modes remain experimental until validated against AutoHyper-style tools and known examples.
