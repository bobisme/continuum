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

Staging rule: G0 closes in Phase A for every item whose required experiment runs against Phase A machinery — the freeze-blocking subset DX-01–03, 10, 12, 13, 14. An unexecuted or failed item in this subset blocks interface freeze. Items whose experiments require later subsystems are re-homed to the gates that own them — DX-04 (causal debugger) → G4, DX-05 (incrementality) → G5, DX-06 (neighborhood/mutation campaign) → G4, DX-07 (Forge non-vacuity) → G7, DX-08 (lens ambiguity) → G6, DX-09 (human diagnosis study) → G8, DX-11 (proof-service isolation) → G6, DX-15 (benchmark leakage) → G9 — with the Phase A spike results for DX-04, 05, 07, and 08 recorded as artifact-shape evidence only, and each re-homing recorded in the matrix as that item's explicit decision. The Phase A benchmark subset used for DX-10 and the G2 Context Pack ablation must itself pass the plan §19.4 family/source-hash separation check before either result is accepted; full leakage validation remains DX-15 at G9. The matrix carries Status, Evidence, and Decision columns; plan §0.3's counts are derived from it, not asserted beside it. (delivered: bn-31yxg — independent mechanical audit, tools/check_g0_matrix.py, 0 errors across all 15 rows, every pointer resolved, §0.3 re-derived; two Evidence-cell count defects corrected under bn-3tp78)

## G1 — Workbench identity and lifecycle

- snapshots, intent contracts, handles, and artifacts are immutable and content-addressed; (delivered: bn-1k1s8 — spec-derived second BLAKE3 recomputes every address off the wire; narrowing on the bone: in_* and campaign identities are preimage digests, cap_* by design not content-addressed, 16 of plan §4.4's 19 classes have no minting path)
- explicit handles across native API; (delivered: bn-3j01v — second IDL reader by a different strategy, 75 of 75 operations, all 666 leaves adjudicated, 239 resource-naming leaves all handles or content identities; exact-set narrowing of 6 raw-string leaves tracked bn-ah1k8)
- requests are idempotent under idempotency keys; (delivered: bn-3huh7 — all 18 servable mutations probed with world-fingerprint reads under fresh-request_id retries, 29 unservable proven key-unspent at dispatch; replay request_id and audit-correlation echo defect tracked bn-1ybn3)
- continuation resume validates epochs and inputs before any reuse; (delivered: bn-16v3x — 13 input classes zero-trace-fingerprinted, guarantee measured wider than declared on six epoch axes; model-availability ordering gap and engine error code tracked bn-3oocz; pinned intent never revalidated, recorded on the bone)
- cancellation closes obligations and publishes no partial finality; (delivered: bn-1tkrp — obligation census by kind and subject with wire-derived shadow ledger, store read bit-for-bit across cancels, post-cancel re-drive byte-identical; scope: one of 26 task classes servable in this build, single-threaded dispatch grain)
- artifact publication is transactional (INV-017); (delivered: bn-2vbqm — closed census of all five publisher sites verified load-bearing by injected mutant, full abort taxonomy, 15-phase stroboscopic sweep, no tear found; INV-017 holds per artifact, and bn-12plt corrected the RFC 0026 operation-grain wording to the per-artifact contract)
- authorization is checked independently of handle possession; (delivered: bn-cxd2y — 240 admission-ledger decisions paired across resource-holding and resource-empty states all agree, authority-without-possession admitted, confused-deputy handle channel refused, two admission mutants caught; instance-scope bound of 2 of 19 handle classes tracked bn-28kv4)
- daemon crash recovery leaves no stale index entries or orphan tasks (plan §4.5). (delivered: bn-q80m7 — split verdict: no-stale-index re-derived through the declared identity seam at dropped-value grain; no-orphan-tasks SATISFIED-AT-NARROWER-SCOPE by bn-1z09m's §4.5 startup resolution pass over durable task records, resume branch via bn-20142's continuation record, terminal tasks via bn-2g3ei's terminal record, narrowed at pre-bone Settled stores, a failed resume's budget write and a completed task's older cont_ handles; fsck now audits through the declared seam — bn-k99dt)

## G2 — Agent-computer interface

- generated clients and schemas ship for the native protocol; (delivered: bn-2wypi — shipping inventory plus 11-mutant conformance audit; zero generated artifacts, hand transcription held by checkers one deep at field level; the gate close read checked-hand-written as satisfying ships, since no client is written against prose — user ruling at bn-1grk)
- no terminal parsing required; (delivered: bn-1slz1 — spec-driven walk of the canonical answer document, 27 of 27 workflow steps typed with zero prose-only carriers, 14 free-text fields adjudicated exhaustively, 15 refusals none prose-bound; bare-String artifact-class mis-scope tracked bn-3ncfp)
- explicit handles and resumability; (delivered: bn-24ulk — 12 interruption points amnesia-safe, session rebuilt field-for-field from the wire with zero memory-only tokens; total-amnesia re-entry unsurvivable since no enumeration surface exists in the 75 operations, and safe-resend rests on client deterministic keying — both stated as scope)
- stale state rejected; (delivered: bn-3r10g — six staleness carriers, each refuses with its declared code leaving a byte-identical effect fingerprint, both TOCTOU directions probed; StaleSnapshot refusals name the lineage head as an N2-filtered recovery offer, and workspace.diff consults its sealed-input carriers — bn-27mx7)
- Context Packs are bounded, carry omission manifests and expansion handles, and improve agent benchmark effectiveness; (delivered: bn-1iljt — boundedness and manifest probes exact at both boundaries, handles dereferenced live; context.compile ceilings enforced by bn-2ga1c; the benchmark-effectiveness half is read at the gate close as a Phase B aspiration, not a Phase A conjunct — user ruling at bn-1grk, ablation owned by bn-30bd8)
- native ACI beats the disciplined shell baseline on success and cost, or the protocol is redesigned before freeze (G0-DX-10); (delivered: bn-39uby — satisfied by the second disjunct: first disjunct's failure arithmetic recomputed from pinned data by fresh code and matched, redesign trail verified across four documents, create_by_reference measured on the wire at a 37% request saving; ledger drifts tracked bn-7befs; unreachability conditional on the pinned handshake charge, stated)
- prompt injection corpus cannot trigger privileged operations. (delivered: bn-ugee0 — privileged set and authority ladder re-derived from the IDL independently, all 225 admission decisions predicted before any frame sent and confirmed at the wire, top rung positively admits non-privileged and denies privileged; corpus reaches 3 of 5 privileged operations, tracked bn-1n7hy, and the delivering guard's same-registry blind spot tracked bn-2a7q9)

## G3 — Intent integrity

- every gaming mutation in the development gaming suite (plan §19.4) that falls in a supported fragment is classified as a privileged intent change, across all seven diff dimensions (property/assumption/bound/observer/fault/fairness/assurance); the held-out gaming suite is evaluated once, at G9;
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
- reduction engines show zero reachability mismatch against the unreduced reference on the no-reduction corpus, and certified claims fall back to the unreduced baseline until the reduction certificate lane matures (docs/08 R04);
- semantic artifacts are byte-identical across the docs/19 determinism matrix;
- interactive latency targets (docs/34) hold on the reference workload, measured with a saturating background swarm present (plan §4.1, docs/34).

## G6 — Proof service

- Lean foundations kernel-check (Revision 2 and Revision 3 theorems, no placeholders);
- pinned Lean environment with per-request isolation and cancellation;
- every proof receipt carries theorem and axiom manifests;
- agent proof repair accepted only by kernel;
- certificate mutations are rejected;
- proof Context Packs improve proof-worker success/cost;
- one nontrivial corpus protocol carries safety and liveness evidence plus real-code refinement produced by this service (Phase D exit).

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
