# ADR-0001: Use asupersync as the execution substrate

**Status:** Accepted  
**Date:** 2026-07-24  
**Decision owners:** Continuum architecture group

## Context

Continuum needs production execution, deterministic Lab execution, structured task ownership, virtual time, explicit capabilities, cancellation semantics, and replay. Implementing another runtime would duplicate asupersync and create two incompatible notions of task, cancellation, and quiescence.

## Decision

Continuum uses asupersync for concrete production and Lab execution. Continuum owns neither a competing executor nor replacement task/region lifecycle. Integration is isolated in `continuum-asupersync`, which translates a stable public semantic-event contract into CIR.

Continuum remains capable of model-only execution without asupersync. The normative abstract semantics are owned by Continuum, so the runtime cannot silently redefine model behavior.

## Consequences

Benefits:

- cancel-correctness and obligations become structural;
- real code can run in deterministic verification mode;
- bespoke DST scheduler/time/replay layers can be deleted;
- the product has an immediate practical wedge.

Costs/risks:

- dependency on a young runtime;
- possible pressure to use private Lab internals;
- shared-runtime/common-mode bugs;
- narrower initial audience than Tokio-based systems.

## Alternatives considered

1. Build on Tokio: larger ecosystem, but cancellation/task ownership remain conventional and Continuum would need to invent the missing semantics.
2. Runtime-neutral `World` trait only: attractive abstraction, but risks lowest-common-denominator semantics and repeated adapters.
3. Build a custom executor: rejected as wasteful and semantically divergent.

## Validation and rollback

G0 requires stable event hooks, replay sufficiency, and no private scheduler dependency. If these cannot be achieved, preserve the abstract/CIR design and replace the concrete adapter rather than forking the runtime indefinitely.
