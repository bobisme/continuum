# ADR 0033: Checked Projection May Generate Rust Interfaces

## Status

Accepted as a target.

## Context

Model/code drift often begins at messages, roles, operation names, and observability. Pure code generation can constrain architecture without proving behavior.

## Decision

CML global protocol declarations may project to role-local automata, Rust message types, endpoint traits, asupersync skeletons, instrumentation IDs, and monitors. Projection emits a preservation/compatibility receipt. Generated artifacts establish an interface seam, not full implementation correctness.

## Consequences

- global/local choice knowledge must be checked;
- generated APIs must remain idiomatic and overridable behind traits;
- real implementations still require refinement;
- projection failures are model diagnostics, not code-generation errors.
