# ADR-0010: Make cancellation and obligations part of formal semantics

**Status:** Accepted  
**Date:** 2026-07-24  
**Decision owners:** Continuum architecture group

## Context

Cancellation is ubiquitous in async systems yet often modeled as task disappearance. Asupersync supplies structural lifecycle and obligations that can support stronger reasoning.

## Decision

Define first-class cancellation events, phases, responsiveness assumptions, reserve/commit/abort semantics, obligation ownership/transfer, finalization, and region quiescence. Add safety and liveness property primitives over them.

## Consequences

Continuum gains a unique correctness dimension and can verify shutdown/race behavior. The calculus must remain compositional across adapters and host boundaries.

## Alternatives considered

1. Treat cancellation as crash: semantically wrong.
2. Ignore it in models: leaves a major implementation gap.
3. Model manually per protocol: duplicates subtle semantics.

## Validation and rollback

The calculus is accepted provisionally. It graduates when standard channel/storage/process packs satisfy compositional rules and real bugs are expressed more clearly than with ad hoc state.
