# ADR-0002: Adopt a causal intermediate representation as the semantic interchange

**Status:** Accepted  
**Date:** 2026-07-24  
**Decision owners:** Continuum architecture group

## Context

State vectors and total traces are insufficient as a common format for DPOR, runtime traces, cancellation, refinement, and production conformance. Total orders contain arbitrary scheduling choices; flat states lose causality and lifecycle evidence.

## Decision

Define CIR as a versioned event/configuration representation with causality, conflict, typed footprints, phases, obligations, durability, time constraints, faults, observations, abstract deltas, and provenance.

State graphs and total traces are derived projections. CIR is a semantic format, not merely a logging schema.

## Consequences

Enables one artifact to drive replay, reduction, refinement, conformance, and diagnosis. It also introduces complexity: event-structure validation, canonical encoding, and state projection become foundational.

## Alternatives considered

1. Transition-system IR only: simpler but weak for production partial orders and true-concurrency reduction.
2. OpenTelemetry/JSON traces: interoperable but semantically incomplete.
3. Rust AST/MIR as IR: too implementation-specific and unstable.
4. TLA+ AST as IR: too language-specific.

## Validation and rollback

The CIR reference semantics must reconstruct equivalent states under independent reorders, round-trip canonical encodings, and support the first vertical slice. If true-concurrency fields prove unnecessary, they may remain optional extensions; causality and typed footprints remain mandatory.
