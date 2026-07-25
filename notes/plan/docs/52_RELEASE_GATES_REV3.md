# Revision 3 Release Gates

## G0 — Falsification

All load-bearing experiments in [`../notes/G0_SPIKE_MATRIX.md`](../notes/G0_SPIKE_MATRIX.md) are executed or explicitly block interface freeze.

## G1 — Workbench identity and lifecycle

- snapshots and intent are immutable/content-addressed;
- explicit handles across native API;
- idempotent task creation;
- cancellation closes obligations;
- continuation resume validates epochs/inputs;
- artifact publication is transactional;
- authorization independent of handle possession.

## G2 — Agent-computer interface

- generated clients and schemas;
- no terminal parsing in benchmark agents;
- bounded Context Packs with omissions/expansion;
- native ACI beats shell baseline on success/cost or is redesigned;
- prompt injection corpus cannot trigger privileged operations.

## G3 — Intent integrity

- property/assumption/bound/observer/fault/fairness/assurance diffs;
- policy locks;
- hidden gaming suite;
- evidence invalidation on intent revision;
- no ordinary repair promotion with protected change.

## G4 — Causal debugging and repair

- real asupersync failure;
- replay-preserving causal core;
- partial-order debugger and alternate branch;
- exact and neighboring replay;
- mutation challenge;
- promotion receipt;
- human review view.

## G5 — Incremental trust

- query edge classes;
- clean-build differential Tribunal;
- mismatch minimization/quarantine;
- proof/certificate freshness;
- crash-safe cache/publication;
- interactive latency targets on reference workloads.

## G6 — Proof service

- pinned Lean environment;
- request isolation and cancellation;
- theorem and axiom manifests;
- Context Pack proof tasks;
- agent proof repair accepted only by kernel;
- foundational Revision 2/3 theorems compile without placeholders.

## G7 — Forge

- typed holes and finite CEGIS;
- positive/non-vacuity scenarios;
- independent verification;
- behaviorally diverse archive;
- hidden variant generalization;
- explicit unrealizability/unknown distinction;
- materialized model/Rust/proof obligations.

## G8 — Human usability

- Rust newcomers complete deterministic/causal workflow;
- distributed-systems experts correctly diagnose real failures;
- explanation beats raw trace baseline;
- assurance confidence is calibrated;
- exact artifacts reachable from summaries;
- accessibility and non-color CLI/UI semantics.

## G9 — Corpus interaction parity

For every validated TLA+ family at its declared level:

- native model equivalent;
- expected model verdict/state facts;
- meaningful explanation;
- failure/mutation task;
- proof/refinement support where applicable;
- agent benchmark artifact.

## G10 — Real adoption

- two materially different real projects remove bespoke DST infrastructure;
- at least one stops requiring a separate TLA+ workflow for normal development;
- agent-driven repair is used on real changes under review;
- production evidence returns valid pass/fail/inconclusive classifications;
- operating cost is acceptable.

## Release blocker doctrine

A missing feature can be documented as unsupported. A misleading assurance result, replay failure, stale receipt, hidden intent change, or unauthorized promotion is a release blocker.
