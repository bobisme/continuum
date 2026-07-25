# ADR 0041: Verification Debugger with DAP Projection

## Status
Accepted.

## Context
Linear trace viewers cannot expose alternate schedules, causal predecessors, abstractions, or fairness obligations.

## Decision
Build a native partial-order verification debugger. Provide semantic stepping, causal reverse, branch comparison, why-enabled/blocked, abstract/concrete/obligation views, and fault branching. Project common operations through DAP for IDE reuse.

## Consequences
- debugger works on immutable execution artifacts;
- DAP is insufficient for all semantics, requiring namespaced requests;
- branch handles and lazy values are required;
- debugger results must replay through the semantic engine.

## Evidence required
Branch/replay tests and user/agent diagnosis comparisons.
