# ADR 0052: Multi-Agent Evidence Graph

## Status
Accepted.

## Context
Chat-based swarms lose provenance, duplicate work, and can reach consensus without evidence.

## Decision
Durable multi-agent work is stored as typed immutable nodes/edges with status authority. Agents propose; execution/checker services promote. Conflicts and missing obligations are first-class.

## Consequences
- orchestration can be model-agnostic;
- work is resumable and auditable;
- graph/context compilers are required;
- chat remains optional coordination only.

## Evidence required
Swarm benchmark with conflicting candidates, unauthorized promotion attempts, and task handoff.
