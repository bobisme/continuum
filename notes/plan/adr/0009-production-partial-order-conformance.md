# ADR-0009: Validate production traces as partial orders

**Status:** Accepted  
**Date:** 2026-07-24  
**Decision owners:** Continuum architecture group

## Context

Distributed traces are incomplete and only partially ordered. Forcing collector order into a model can create false violations or false validity. Recent trace-validation work shows the value of solving for legal orders.

## Decision

Production semantic events carry causal predecessors, local sequence, and time intervals. Offline checking solves for an execution/completion consistent with the abstract model and reports valid, invalid, insufficient observation, semantic mismatch, or resource inconclusive.

## Consequences

Continuum can close the production feedback loop. Instrumentation, privacy, integrity, and solver scaling become significant work.

## Alternatives considered

1. Total-order logging: expensive and still not necessarily truthful.
2. Linearizability histories only: too narrow for non-atomic protocols.
3. Runtime monitors for local properties only: insufficient for global conformance.

## Validation and rollback

G5 requires real staging/production traces, injected violations, explicit incompleteness, and trace-to-Lab reproduction. Until then the feature is experimental.
