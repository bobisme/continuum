# ADR-0005: Make partial-order executions primary

**Status:** Accepted  
**Date:** 2026-07-24  
**Decision owners:** Continuum architecture group

## Context

Interleaving semantics replicate equivalent schedules and distort production observations. Modern DPOR, unfoldings, interval-pomset theory, and partial-order trace validation all operate on causal structure.

## Decision

CIR executions are partial orders/configurations. Deterministic total linearizations exist for replay and legacy engines. Search engines may use DPOR, unfolding, state graphs, or symbolic encodings while binding results to the same causal semantics.

## Consequences

This creates richer semantics and stronger reduction opportunities. It also raises soundness burden around independence, observers, fairness, and certificate checking.

## Alternatives considered

1. Total traces primary with vector-clock annotations: easier, but reduction and conformance remain add-ons.
2. State graphs only: loses causal explanations.
3. Petri nets as the only core: too restrictive for arbitrary state and effects.

## Validation and rollback

The no-reduction reference must agree with partial-order engines. If an advanced true-concurrency backend loses on benchmarks, it can be killed without changing the CIR contract.
