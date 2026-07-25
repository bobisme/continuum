# ADR 0037: Explicit Content-Addressed Handles

## Status
Accepted.

## Context
Implicit sessions have ambiguous lifetime and sharing semantics. Multi-agent work often needs some state shared and other state isolated.

## Decision
All durable workflow state—workspace, intent, task, continuation, proof state, debug branch, repair transaction, Forge archive, and evidence—is addressed by explicit opaque handles. Handles bind to immutable content or versioned transactional state. Authorization is checked separately from possession.

## Consequences
- deterministic resume/handoff/caching;
- clients must thread handles explicitly;
- opaque identifiers and lifecycle/retention policy are required;
- stale input becomes a typed error rather than accidental behavior.

## Evidence required
Idempotency, stale-handle, concurrent publication, authorization, and resume tests.
