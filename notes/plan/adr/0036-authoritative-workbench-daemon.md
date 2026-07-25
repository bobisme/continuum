# ADR 0036: Authoritative Workbench Daemon

## Status
Accepted.

## Context
Multiple interfaces and agents cannot safely maintain independent notions of workspace, task, cache, proof state, and evidence. Process-local sessions make replay and handoff fragile.

## Decision
`continuumd` is the authority for workspace snapshots, Intent Contracts, incremental queries, task lifecycle, artifact publication, evidence status, repair transactions, and Forge archives. Semantic artifacts remain immutable. CLI, LSP, DAP, MCP, SARIF, TUI, and web surfaces are adapters.

## Consequences
- one state model and audit trail;
- local embedded deployment remains mandatory;
- daemon correctness and crash recovery become high-priority verification targets;
- adapters cannot promote evidence or own hidden semantic state.

## Evidence required
Task/publication model checks, crash/cancellation campaigns, idempotency tests, and adapter conformance.
