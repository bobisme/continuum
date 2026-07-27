# Revision 3 Release Gates

> **Note:** Reconciled bullet-for-bullet with `plan.md` §22; the two are updated together.

## Phase↔Gate mapping

| Phase (plan §21) | Gates it must close |
|---|---|
| A | G0 (falsification), G1 (workbench identity and lifecycle), G2 (ACI) |
| B | G3 (intent integrity), G4 (causal debugging and real repair) |
| C | G5 (incremental trust) |
| D | G6 (proof service) |
| E | G7 (Forge) |
| F | G8 (human usability), G9 (corpus parity), G10 (Continuum 1.0) |

## G0 — Falsification

All load-bearing experiments in [`../notes/G0_SPIKE_MATRIX.md`](../notes/G0_SPIKE_MATRIX.md) have evidence or an explicit redesign decision, recorded in the matrix itself. A failed or unexecuted freeze-blocking item blocks interface freeze.

Staging rule: G0 closes in Phase A for every item whose required experiment runs against Phase A machinery — the freeze-blocking subset DX-01–05, 07, 08, 10, 12, 13, 14. An unexecuted or failed item in this subset blocks interface freeze. Items whose experiments require later subsystems are re-homed to the gates that own them — DX-06 (neighborhood/mutation campaign) → G4, DX-11 (proof-service isolation) → G6, DX-09 (human diagnosis study) → G8, DX-15 (benchmark leakage) → G9 — and each re-homing is recorded in the matrix as that item's explicit decision. The matrix carries Status, Evidence, and Decision columns; plan §0.3's counts are derived from it, not asserted beside it.

## G1 — Workbench identity and lifecycle

- snapshots, intent contracts, handles, and artifacts are immutable and content-addressed;
- explicit handles across native API;
- requests are idempotent under idempotency keys;
- continuation resume validates epochs and inputs before any reuse;
- cancellation closes obligations and publishes no partial finality;
- artifact publication is transactional (INV-017);
- authorization is checked independently of handle possession;
- daemon crash recovery leaves no stale index entries or orphan tasks (plan §4.5).

## G2 — Agent-computer interface

- generated clients and schemas ship for the native protocol;
- no terminal parsing required;
- explicit handles and resumability;
- stale state rejected;
- Context Packs are bounded, carry omission manifests and expansion handles, and improve agent benchmark effectiveness;
- native ACI beats the disciplined shell baseline on success and cost, or the protocol is redesigned before freeze (G0-DX-10);
- prompt injection corpus cannot trigger privileged operations.

## G3 — Intent integrity

- every gaming mutation in the hidden (held-out) suite that falls in a supported fragment is classified as a privileged intent change, across all seven diff dimensions (property/assumption/bound/observer/fault/fairness/assurance);
- mutations outside supported fragments classify as `Unknown` and block ordinary promotion rather than passing silently;
- intent policy locks are enforced; evidence is invalidated on intent
  revision;
- no ordinary repair promotes with a protected-intent change.

## G4 — Causal debugging and repair

- a real asupersync failure — not only injected mutants — is diagnosed and repaired end-to-end, alongside multiple known mutants;
- causal explanation with a replay-preserving core;
- partial-order debugger including alternate-branch exploration;
- exact and neighboring replay;
- mutation challenge;
- repair transaction closes exact, neighborhood, and mutation gates under the Phase B gate profile (plan §21);
- promotion receipt;
- human review view over the repair transaction.

## G5 — Incremental trust

- incremental results continuously match clean builds under the Incremental Parity Audit (plan §9.5);
- reuse edges carry their class (Exact/Validated/Conservative/Experimental) and mismatches are minimized and quarantine the class;
- proof and certificate freshness is tracked;
- crash-safe cache/publication;
- evidence queries and context compilation meet the docs/34 targets at
  ≥10^7 evidence nodes on the reference workload;
- interactive latency targets (docs/34) hold on reference workloads.

## G6 — Proof service

- Lean foundations kernel-check (Revision 2 and Revision 3 theorems, no placeholders);
- pinned Lean environment with per-request isolation and cancellation;
- every proof receipt carries theorem and axiom manifests;
- agent proof repair accepted only by kernel;
- certificate mutations are rejected;
- proof Context Packs improve proof-worker success/cost.

## G7 — Forge

- typed holes and finite CEGIS;
- safety and positive/non-vacuity scenarios;
- independent candidate verification;
- diversity archive has semantic, not merely syntactic, spread;
- hidden variant generalization;
- explicit unrealizability/unknown distinction;
- unrealizability produces reusable evidence where supported;
- materialized model/Rust/proof obligations.

## G8 — Human usability

- the preregistered study (plan §21.1) covers both cohorts — Rust newcomers completing the deterministic/causal workflow, and distributed-systems experts correctly diagnosing real failures;
- the preregistration (expanded docs/48) is published before the study
  runs; results are graded only against its fixed thresholds;
- explanation beats raw trace baseline on diagnosis accuracy and time;
- assurance confidence is calibrated, not merely no worse than baseline;
- progressive disclosure reaches exact artifacts;
- accessibility and non-color CLI/UI semantics: no critical workflow requires color or a rendered graph;
- no critical workflow requires formal-methods folklore.

## G9 — Corpus interaction parity

- 80 validated TLA+ families at their declared parity level (`corpus/tla-examples/PARITY_LEVELS.md`), measured per plan §19.4's held-out discipline;
- every family carries its interaction artifacts: native model, expected verdict and state facts, meaningful explanation, failure/mutation task, proof/refinement support where applicable, and an agent benchmark artifact — this is *interaction* parity, per B23, not a family count.

## G10 — Continuum 1.0 (real adoption)

- two real project migrations;
- two materially different real projects remove bespoke DST infrastructure;
- at least two migrated projects stop requiring a separate TLA+
  workflow for normal development;
- agent-driven repair is used on real changes under review;
- production evidence returns valid pass/fail/inconclusive classifications (INV-008);
- public ContinuumBench;
- documented assurance envelopes;
- operating cost is acceptable;
- zero known paths for an unprivileged agent to promote false evidence.

## Release blocker doctrine

A missing feature can be documented as unsupported. A misleading assurance result, replay failure, stale receipt, hidden intent change, or unauthorized promotion is a release blocker at every gate.

A confirmed false-positive success verdict triggers the soundness incident policy in `docs/09`: block release, revoke affected claim IDs, publish affected semantic epochs, ship an artifact scanner, add a permanent regression, and reevaluate whether the producing engine remains eligible for certified mode.
