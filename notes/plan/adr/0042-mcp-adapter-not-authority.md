# ADR 0042: MCP Is an Adapter, Not Authority

## Status
Accepted.

## Context
MCP is useful for agent interoperability, but transport/session/tool conventions should not define Continuum semantics or trust.

## Decision
Expose a curated MCP adapter over the native protocol. Stateful workflows use explicit Continuum handles. MCP clients cannot promote evidence, bypass capability checks, or own hidden semantic state.

## Consequences
- broad agent ecosystem access;
- native protocol remains stable across MCP changes;
- tool results must be bounded and deterministic;
- authorization is enforced below the adapter.

## Evidence required
MCP conformance, multi-agent sharing/isolation, prompt-injection, and capability tests.
