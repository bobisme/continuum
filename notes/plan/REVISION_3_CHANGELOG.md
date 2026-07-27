# Revision 3 Changelog

Revision 3 keeps Revision 2's semantic and proof foundations while redesigning Continuum around trustworthy human and agent interaction.

## Architectural changes

1. **Intent becomes a protected semantic artifact.** Properties, assumptions, fairness, bounds, observers, fault models, and assurance policy receive content identities and privileged change control.
2. **`continuumd` becomes authoritative.** CLI, LSP, DAP, MCP, SARIF, TUI, and web interfaces are adapters over a native typed protocol.
3. **Implicit sessions are removed.** Workspace snapshots, tasks, searches, crashpacks, proof states, and continuations are explicit content-addressed handles.
4. **Context Packs replace terminal scraping.** Agents receive property-directed, bounded causal/proof slices with omission manifests and expansion links.
5. **Repairs become transactions.** A patch cannot be promoted without exact replay, semantic/intent diff, neighboring exploration, mutation challenge, proof impact, and a promotion receipt.
6. **Verification debugging becomes first-class.** Continuum exposes partial-order frontiers, abstract/concrete state, obligations, alternate schedules, and causal reverse stepping through a DAP adapter.
7. **Incrementality receives a trust model.** Reuse is classified as exact, validated, conservative, or experimental; the Incremental Parity Audit (plan §9.5) continuously audits invalidation.
8. **Continuum Forge is added.** Protocol code, invariants, abstractions, rankings, auxiliary state, and assumptions may be co-synthesized under fixed intent and independently checked evidence.
9. **Multi-agent work uses an Evidence Graph.** Chat may coordinate, but claims, counterexamples, patches, proof obligations, and receipts are typed graph artifacts.
10. **ContinuumBench becomes a release contract.** It measures modeling, diagnosis, repair, proof, refinement, synthesis, intent integrity, security, cost, and generalization.

## New executable spikes

- Context Pack causal slicing and replay preservation.
- Intent-gaming semantic diff classification.
- Explicit snapshot/continuation protocol and stale-handle rejection.
- Finite CEGIS synthesis of the minimal safe and live acknowledgement guard.
- Incremental query invalidation and Incremental Parity Audit coverage.
- Partial-order causal debugger branching.
- Proof-oriented model/program synchronization that reports ambiguity rather than inventing edits.

## New proof seeds

Lean source modules now define initial contracts for:

- intent refinement and protected changes;
- context-slice soundness statements;
- repair acceptance;
- incremental result equivalence;
- synthesis-candidate correctness.

These sources contain no placeholders. Kernel checking remains a mandatory implementation gate; no Lean toolchain was available in the artifact-generation environment.

## Scope clarification

Revision 3 does not turn an LLM into part of the trusted computing base. Agents search, propose, translate, explain, and prioritize. Continuum's semantic engines, independent checkers, and Lean kernel decide what evidence supports.
