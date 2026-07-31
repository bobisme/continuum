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

## Ratified margins (frontier register: ACI)

The Agent–computer interface (B2, §10) row of plan §24.5 is ratified with these margins:
quote-id=aci-benchmark-margins "On the agent benchmark, with an identical base model, task set, and per-task budget, native ACI must beat the disciplined-shell baseline by at least 10 percentage points of absolute task success, at least 30% fewer interface bytes per solved task (bytes, not tokens, are the graded cost denominator per RFC 0027), and at least a 50% relative reduction in invalid-action rate, with every metric paired per task over at least 3 seeds and the success margin's one-sided 95% lower bound above zero; missing any one of the three margins fails G0-DX-10 and forces protocol redesign before freeze."

Rationale. This note proposes no numbers, so the three margins are derived conservatively from
its own experiment list and metrics (experiment 1, native handles vs shell/CLI on an identical
agent/model; metrics: success, tokens, calls, invalid operations) and from the harness the lane
already commits to. Ten percentage points of absolute success is the smallest margin that
survives seed noise on a corpus of PR 10's planned size while staying demanding against a
*disciplined* shell baseline, which already reads the same typed artifacts through the CLI;
anything smaller would let the typed surface be declared a winner on noise, which is the kill
that the typed surface loses to disciplined shell use. Cost is graded in interface bytes rather
than tokens because RFC 0027 rejects token-denominated enforcement as model-relative, and the
denominator is *per solved task* so a surface cannot win by failing early and cheaply; 30% is
the floor at which the saving exceeds prompt/format variation and therefore pays for the schema
overhead named in the kill that schema churn dominates agent cost. The 50% relative reduction in
invalid-action rate tests the validity criterion and the semantic action grammar directly: if
explicit preconditions and pre-call plan validation do not remove half of the invalid
operations, the grammar is not doing the work claimed for it, which is the kill that handles do
not reduce invalid-action rate. Pairing per task over at least 3 seeds with a one-sided 95%
lower bound above zero on the success margin follows the identical-model/identical-budget
baseline ladder in RFC 0027 and research/33's resource-normalized reporting; all three margins
must hold, so a miss on any one is a fail rather than an average.
