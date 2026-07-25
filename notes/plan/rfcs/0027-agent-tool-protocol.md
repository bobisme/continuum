# RFC 0027: Agent Tool Protocol

## Status
Draft.

## Summary
Expose a minimal semantic ACI over the native protocol.

## Tools
Workspace, verification, context, debugger, repair, proof, Forge, task, and evidence tools. Each tool is narrow, typed, and returns handles rather than oversized payloads.

## Context policy
Agents specify maximum bytes/tokens/nodes and desired audience. Default returns one Context Pack plus next operations. Expansion is relation-based.

## Authority
Tool capabilities distinguish read, propose, execute, revise intent, and promote. Agent-facing installations omit promotion by default.

## Safety
Source/log strings remain data. Tool descriptions contain no dynamic untrusted interpolation. Result schemas include omissions and uncertainty.

## Evaluation
Benchmark against shell/CLI interaction with identical base models and budgets. Freeze only after ablation shows advantage.
