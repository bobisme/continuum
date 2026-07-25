# ADR-0019: Isolate runtime/compiler/foreign-tool adapters from semantic core

**Status:** Accepted  
**Date:** 2026-07-24  
**Decision owners:** Continuum architecture group

## Context

Asupersync, rustc internals, TLA+/Quint frontends, and solvers evolve. Direct dependencies throughout the workspace would make semantic code unstable and expand the TCB.

## Decision

Each foreign system has one adapter crate/process boundary. Adapters emit normalized, versioned objects and maintain conformance fixtures. The kernel does not depend on them.

## Consequences

Churn and licensing/dependency complexity are contained. Some zero-copy opportunities may be sacrificed.

## Alternatives considered

1. Shared foreign types across crates: convenient initially, expensive later.
2. Dynamic plugins everywhere: unstable and insecure.

## Validation and rollback

Dependency graph CI enforces the boundary. Adapter replacement must not change semantic digests for the conformance corpus.
