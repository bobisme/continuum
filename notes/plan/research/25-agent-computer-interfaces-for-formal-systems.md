# Agent–Computer Interfaces for Formal Systems

**Claim class:** design hypothesis

## Question

What interface lets a coding/proof agent use a verification environment reliably and efficiently without granting it authority over truth?

## State of the art

SWE-agent demonstrates that interface design materially changes coding-agent effectiveness. Pantograph argues that human-oriented Lean LSP interaction burdens machine users with cursor/text-state mechanics and instead exposes proof-state operations. LeanDojo, AXLE, OProver, LAMP, and related systems reinforce the value of compiler/kernel feedback, retrieval, explicit proof context, and planner/worker/verifier separation.

Primary references:

- https://arxiv.org/abs/2405.15793
- https://arxiv.org/abs/2410.16429
- https://arxiv.org/abs/2606.26442
- https://arxiv.org/abs/2605.17283
- https://arxiv.org/abs/2606.28841

## Continuum hypothesis

A formal-systems ACI should expose **semantic state**, not filesystem/editor state:

```text
workspace snapshot
intent contract
verification task
proof/debug state
counterexample/evidence graph
repair/synthesis transaction
```

The agent should never need to infer these from process lifetime, cursor position, logs, or chat memory.

## Interface calculus

Treat the ACI as a labeled transition system:

```text
AgentState × ToolOperation → AgentState × Observation
```

We can evaluate:

- validity: operation preconditions explicit;
- sufficiency: all benchmark tasks expressible;
- minimality: redundant operations removed;
- observability: necessary distinctions visible;
- safety: authority not encoded in prose;
- compositionality: handles can be passed among subagents;
- resumability: connection loss does not alter task state.

A possible Lean formalization defines capability-preserving traces and proves that no sequence of unprivileged operations reaches `EvidenceStatus.Proved` without a checker event.

## Novel proposal: semantic action grammar

Ship a machine-readable workflow grammar describing legal operation sequences and state transitions. An agent can validate a plan before calls; orchestrators can synthesize workflows; invalid operations become low-cost local feedback.

Example:

```text
FailureAvailable
  ├─ compile_context → ContextAvailable
  ├─ open_debug → DebugAvailable
  └─ begin_repair → RepairDraft

RepairDraft
  └─ apply_patch → RepairApplied
RepairApplied
  └─ evaluate → RepairReady | RepairBlocked | RepairInconclusive
```

This is not a rigid UI wizard: advanced clients may call any operation whose preconditions hold.

## Novel proposal: observation bisimulation

Two ACI designs are equivalent for a task class if they expose observations sufficient to distinguish all semantic states requiring different correct next actions. This suggests a rigorous way to detect underpowered or excessively verbose interfaces:

- underpowered: semantically distinct states are observation-equivalent but require different actions;
- verbose: observations differ without affecting any valid task policy.

Finite benchmark fragments can compute/approximate a minimal quotient of interface observations.

## Experiments

1. Native handles vs shell/CLI on identical agent/model.
2. Raw trace vs Context Pack.
3. Cursor/source proof API vs proof-state handle API.
4. Implicit session vs explicit workspace/debug/proof handles in multi-agent tasks.
5. Free-form errors vs typed recovery actions.
6. Static one-shot context vs expandable graph context.

Metrics: success, tokens, calls, invalid operations, stale-state errors, time, expensive failures, intent violations.

## Kill criteria

- typed API does not improve effectiveness/cost;
- task grammar constrains legitimate strategies without reducing errors;
- Context Pack expansion still requires repository-scale reconstruction;
- handle threading produces more failures than session-based state under realistic clients.

The result may still justify a thinner API, but “agent-native” cannot remain a slogan.
