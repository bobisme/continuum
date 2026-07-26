# RFC 0027: Agent Tool Protocol

## Status
Draft for implementation.

**Target gate:** G2 (agent-computer interface)
**Owners:** ACI leads
**Normative language:** MUST/SHOULD/MAY per RFC 2119.

## Summary

The typed operation surface agents use, layered over RFC 0026. No agent workflow requires terminal parsing, cursor positions, or session reconstruction (plan B2, INV-003).

## Operation registry

The registry is the plan §10.2 list; each entry carries a minimum authority level. Signatures live in the RFC 0026 IDL.

| Namespace | Operations | Min authority |
|---|---|---|
| `workspace` | create / fork / diff / seal | propose |
| `intent` | get / diff | read |
| `intent` | propose_revision | revise-intent |
| `intent` | accept / reject | revise-intent |
| `intent` | lock | revise-intent |
| `verification` | start / result / await | execute |
| `model` | check / explore / compare | execute |
| `program` | extract / run / replay | execute |
| `refinement` | check / explain | execute |
| `proof` | goal / attempt / check / slice | execute |
| `debug` | open / state / enabled / step_event / step_abstract / reverse_causal / branch / compare / why_enabled / why_blocked / export | execute |
| `context` | compile / expand | read |
| `failure` | explain / minimize / branch | execute |
| `repair` | begin / apply / attach / evaluate / resume / review | propose/execute |
| `repair` | promote / reject | promote |
| `forge` | create / step / archive / materialize | execute |
| `task` | status / cancel / resume / subscribe | read/execute |
| `evidence` | get / query / verify | read |
| `query` | explain_reuse / explain_invalidation / clean_compare | read |
| `benchmark` | run | execute |

## Authority levels and capabilities

Five levels — `read < propose < execute < revise-intent < promote` — map onto the plan §18.2 capability set. A `cap_*` capability names its actor, level, resource scope (snapshots/intents/artifact classes), expiry, and delegation allowance. Agent-facing installations MUST omit `promote` and `revise-intent` by default; `intent.accept` is the only transition from `Proposed` to protected status and, with `intent.lock`, is audit-recorded and never present in default agent capability profiles (plan §5.4); promotion is a service decision gated by RFC 0032, never an agent assertion (INV-015, B12). Calls above the capability's level fail with `CapabilityDenied` before any semantic work runs.

## Context policy

- Requests carry `output_policy` (max bytes/tokens/nodes, audience). Token counts are advisory and tokenizer-relative; **byte budgets are the enforced contract** (token counts differ per model and are recorded with the tokenizer id when reported).
- Default failure result: one Context Pack (RFC 0028) plus `next_operations`. Everything else is reachable by expansion, never by dumping.
- Expansion is relation-based over the evidence/causal/proof/source graphs. The initial relation vocabulary: `causal_predecessors`, `causal_successors`, `conflicts_with`, `same_owner`, `property_automaton_step`, `proof_dependency`, `source_span`, `assumption_uses`, `abstraction_of`, `refinement_of`, `alternate_branch`. Each expansion returns a new immutable pack referencing its parent.

## Handoff

A coordinator hands agents: shared `snapshot`, `intent`, and evidence-graph read scope; isolated `dbg_*`, `ps_*`, and Forge candidate scopes (plan §10.5). Handles thread through tool calls; no session affinity exists — any client holding the handles and a valid capability can continue the work.

## Safety

- Source, logs, model text, and production payloads are data. They MUST never be interpolated into tool descriptions, error text, or `next_operations` (INV-016).
- Result schemas always include omissions and typed uncertainty; hiding uncertainty to save tokens is prohibited (docs/34 anti-goal).
- All privileged calls are audited with actor, capability, inputs, and decision (plan §18.5).

## Evaluation

Baseline ladder (research/33): raw shell agent → typed ACI → ACI + Context Packs → ACI + evidence graph, with identical base models and budgets. Metrics: task success, invalid-operation rate, tokens/bytes, expensive failures, stale-handle recovery. The protocol MUST NOT freeze until ablation shows the typed surface beats disciplined shell use (G0-DX-10); if it does not, the ACI is redesigned, not excused.

## Rejected alternatives

- **Prose-first results with a JSON flag.** Rejected: prose is a projection (INV-003).
- **Session-scoped agent state.** Rejected: breaks resumability and multi-agent handoff (B4).
- **Token-denominated enforcement.** Rejected: token counts are model-relative; bytes are objective.

## Open questions

- Expansion-relation completeness for proof workers (with RFC 0035).
- Capability delegation depth for sub-agent spawning (with RFC 0026).

## Acceptance

Registry/authority table enforced in tests for every operation; prompt-injection corpus cannot trigger privileged operations (docs/52 G2); ablation benchmark report attached to the freeze decision; two subagents sharing snapshot/intent but isolated debugger/proof handles complete PR 27's scenario without session coupling.
