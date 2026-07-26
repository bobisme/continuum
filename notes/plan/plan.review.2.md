# Review of `plan.md` (Revision 3 Master Implementation Plan) — second pass

**Review date:** 2026-07-25
**Scope:** `plan.md` as revised by the review-1 fix wave (commit `e639a52 docs: plan review 1`), checked against the accompanying dossier: `docs/52` (normative gates), `docs/34/36/48/55` (contracts), `notes/START_HERE_IMPLEMENTATION.md` and `notes/G0_SPIKE_MATRIX.md` (execution layer), the Rev-3 RFCs and schemas, the research lanes cited by §24.5, and the corpus parity contract. This review does not repeat `plan.review.1.md`; it verifies how its changes landed and reports what is newly or still broken.

---

## Executive Summary

### What improved

The review-1 uptake was real and unusually disciplined. Every plan-level change landed: §0.3 claim status, §4.5 operational contract, the G0–G10 scheme with a Phase↔Gate table, §24.5 frontier register, §21.1 resourcing/licensing, the kernel crates and TCB covenant, the intent-registry custody rule, phase-staged promotion profiles, and all the mechanical renames (`crash_*`, `.ctm`, `verification.start`, Incremental Parity Audit) are in `plan.md` and clean there. The fix wave also reached the RFC and schema layer: RFC 0027's operation registry matches §10.2 exactly; the repair-transaction schema encodes all 12 §8.2 gates **and** structurally enforces `NotYetEnforced` (a `promoted` transaction must list all 12 gates with none pending/failed) — the cleanest contract in the dossier; the verification-task budget carries all seven B18 dimensions; the intent schema gained scope, trust boundaries, fidelity profiles, completion policy, and nondeterminism classes.

### The five systemic problems of this revision

1. **The fix wave stopped at a layer boundary.** `plan.md`, the seven load-bearing RFCs, and 11 schemas were updated; the execution layer and the reference docs were not. `notes/START_HERE_IMPLEMENTATION.md` and `notes/G0_SPIKE_MATRIX.md` are byte-unchanged since the pre-review commit: START_HERE still schedules Lean at PR 28 (the plan now requires it in Phase A), contains no PR 0 for the RFC expansion §25 mandates "before PR 5," knows nothing of the four `continuum-kernel-*` crates, ships no CML-parser or SARIF PR, and builds receipts without the gate-profile/`NotYetEnforced` fields. `docs/36` and `docs/55` still teach `cp_` handles and three different names for `proof.*` operations. The plan is now *ahead of* its own execution plan and its own agent-facing reference docs — the inverse of review 1's problem, and just as capable of shipping the wrong thing.

2. **§22 has become a second divergent gate scheme.** The plan declares `docs/52` normative and then drops roughly fifteen of its 55 bullet requirements while adding eleven of its own. The dropped bullets include the soundness- and security-critical ones: G2's prompt-injection corpus and shell-baseline ablation (with its redesign consequence), G4's replay-preserving causal core, real-failure requirement, and human review view, G5's reuse-edge classes and quarantine, G6's kernel-only acceptance of agent proof repair and per-request isolation, G8's two named cohorts and raw-trace baseline, G9's six per-family interaction artifacts, and G0's "blocks interface freeze" consequence. A reader implementing from plan §22 alone would build a measurably weaker program than docs/52 specifies — and docs/52 itself still contains the retired "clean-build Tribunal" name, so neither document can currently serve as the single authority.

3. **Scheduling contradictions survived the restage.** Phase C requires the 29 Wave 0/1 corpus families "at their required parity," but 14 of them are required at P4 (sorry-free Lean proofs) and 3 at P3 (liveness certificates) — machinery that is a Phase D deliverable. G0 is assigned to Phase A, but its items map to PR 21 (DX-06), PR 27 (DX-12), PR 28 (DX-11), PR 29 (DX-07), a Phase-F-scale human study (DX-09), and an unscheduled corpus experiment (DX-15) — under the current sequencing Phase A cannot close its own gate. Four phase exits (A, D, E, F) cannot close the gates the §22 table assigns them, and no Phase F deliverable produces the G8 usability study at all.

4. **The new §24.5 register misquotes its own lanes.** The "≥10× on non-artificial traces" threshold attributed to causal minimization actually belongs to research/01's *exploration reduction* (and drops its mandatory "no dependent-workload regression" conjunct); "ambiguity rate" names a metric research/31 never defines; the certificate row truncates docs/31's disjunct; research/05 and /09 still have no thresholds (the "TBD" the plan itself flags); and the register omits the weak-memory lane (ADR-0032), the timed/probabilistic lanes (ADR-0016), and five of docs/31's six numeric thresholds — so the program still has two partially overlapping, mutually unaware lane authorities.

5. **Product-surface residue.** §3.1 requires explicit acceptance of draft intents but §10.2 has no `intent.accept` operation; §0.1 promises a security/privacy policy that §5.2 and the schema no longer carry; §5.2's bounds vocabulary is stale against RFC 0037; two handle prefixes in active RFC use (`cap_*`, `diff_*`) are unregistered in §4.4; there is no semantic-epoch advance policy anywhere despite epochs being pinned in a dozen places; the two-config-surface problem from review 1 was never fixed (`examples/continuum.project.toml` still defines incompatible keys and references intent by file path, contradicting §4.2's registry-only rule); and §4.5 covers crash atomicity but not backup/restore of the trust spine.

Changes below are ordered by priority. Diffs are anchored to the current `plan.md` by section context.

---

## Proposed Changes

---

### [CRITICAL] Change #1: Re-converge §22 with docs/52 — restore the dropped soundness and security criteria

**Current State:**
plan §22 declares docs/52 "the normative gate scheme" and then diverges from it at every gate except G0's title. Dropped from the plan's summaries: G1's continuation-resume validation (docs/52 L13 — the G0-DX-03 pass condition) and the explicit word "intent" in the immutability bullet; G2's prompt-injection exit (docs/52 L23, cited as normative by RFC 0027:76), generated clients/schemas, Context Pack boundedness/omission obligations, and the shell-baseline ablation with its redesign consequence (RFC 0027:61 makes it a freeze blocker); G3's seven named diff dimensions and the *hidden* (held-out) property of the gaming suite; G4's "real asupersync failure" (replaced by "correct implementation and multiple mutants" — synthetic mutants do not satisfy it), replay-preserving causal core, alternate-branch debugging, and human review view; G5's reuse-edge classes, mismatch minimization/quarantine, and proof/certificate freshness; G6's pinned environment, request isolation/cancellation, axiom manifests, and — soundness-critical — "agent proof repair accepted only by kernel"; G8's two cohorts, raw-trace baseline, and absolute calibration ("does not worsen" passes a badly calibrated baseline); G9's six per-family interaction artifacts (a family can now hit parity with zero explanation, mutation task, or benchmark artifact); G10's "at least one project stops requiring a separate TLA+ workflow" and the pass/fail/inconclusive classification requirement. G0 dropped the "blocks interface freeze" consequence. Meanwhile the plan *added* ~11 good criteria docs/52 lacks (crash recovery, `Unknown` fails closed, semantic diversity, folklore-free workflows, the blocker-doctrine escalation), so neither document is a superset of the other.

**Proposed Change:**
(a) Add a bidirectional reconciliation rule: docs/52 and §22 are updated in the same commit, docs/52 absorbs every plan-added criterion, no docs/52 criterion may be dropped from the plan summary, and the dossier validator checks bullet-for-bullet correspondence. (b) Restore the dropped criteria to §22 as diffed below.

**Rationale:**
Gates are the program's enforcement mechanism, and review 1's central finding was that divergent gate schemes make every exit condition unevaluable. The review-1 fix created a *new* fork: this time the normative doc is the stale side. The dropped bullets are not stylistic — kernel-only proof acceptance is the single line preventing an agent from promoting an unchecked proof, and the prompt-injection exit is the only security gate in the scheme.

**Benefits:**
- One gate authority again, checkable by machine.
- INV-004, INV-016, and the B22 governance-benchmark bet regain their gate anchors.
- G9 re-becomes an *interaction* parity gate (B23) instead of a count.

**Trade-offs:**
- §22 grows. It is the constitution; it should.

**Implementation Notes:**
The companion docs/52 edit must also fix its own stale "clean-build differential Tribunal" line (L46) and retitle G9/G10 consistently with the plan's better titles. The validator check is a bullet-set comparison, not prose matching.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ ## 22. Release gates, intro @@
 section summarizes it. Legacy gate citations in Revision 2 ADRs (0001–0035)
 and RFCs (0001–0025) refer to the Revision 2 scheme in `docs/26` and may
 not be cited without translation; the translation sweep is part of the
 specification pass in §25. No document may introduce a new gate numbering.
+
+`docs/52` and this section are reconciled in both directions in one
+commit: every criterion this section adds is folded into `docs/52`, no
+`docs/52` criterion is dropped here, and the dossier validator enforces
+bullet-for-bullet correspondence between the two.
@@ ### G0 — Load-bearing falsification @@
-All items in [`notes/G0_SPIKE_MATRIX.md`](notes/G0_SPIKE_MATRIX.md) have evidence or explicit redesign.
+All items in [`notes/G0_SPIKE_MATRIX.md`](notes/G0_SPIKE_MATRIX.md) have
+evidence or an explicit redesign decision, recorded in the matrix itself.
+A failed or unexecuted freeze-blocking item blocks interface freeze
+(docs/52 G0); see the staging rule below.
@@ ### G1 — Workbench identity and lifecycle @@
-- snapshots, handles, and artifacts are immutable and content-addressed;
+- snapshots, intent contracts, handles, and artifacts are immutable and
+  content-addressed;
 - requests are idempotent under idempotency keys;
+- continuation resume validates epochs and inputs before any reuse;
 - cancellation closes obligations and publishes no partial finality;
@@ ### G2 — Agent-computer interface @@
 - no terminal parsing required;
 - explicit handles and resumability;
 - stale state rejected;
-- Context Pack improves agent benchmark effectiveness.
+- generated clients and schemas ship for the native protocol;
+- Context Packs are bounded, carry omission manifests and expansion
+  handles, and improve agent benchmark effectiveness;
+- native ACI beats the disciplined shell baseline on success and cost,
+  or the protocol is redesigned before freeze (G0-DX-10);
+- the prompt-injection corpus cannot trigger privileged operations.
@@ ### G3 — Intent integrity @@
-- every benchmark gaming mutation in supported fragments is classified as a
-  privileged intent change;
+- every gaming mutation in the hidden (held-out) suite that falls in a
+  supported fragment is classified as a privileged intent change, across
+  all seven diff dimensions (property/assumption/bound/observer/fault/
+  fairness/assurance);
@@ ### G4 — Causal debugging and real repair @@
-- correct asupersync implementation and multiple mutants;
-- causal explanation and debugger;
+- a real asupersync failure — not only injected mutants — is diagnosed
+  and repaired end-to-end, alongside multiple known mutants;
+- causal explanation with a replay-preserving core;
+- partial-order debugger including alternate-branch exploration;
+- human review view over the repair transaction;
 - repair transaction closes exact, neighborhood, and mutation gates under
   the Phase B gate profile (§21).
@@ ### G5 — Incremental trust @@
 - incremental results continuously match clean builds under the
   Incremental Parity Audit;
+- reuse edges carry their class (Exact/Validated/Conservative/
+  Experimental) and mismatches are minimized and quarantine the class;
+- proof and certificate freshness is tracked;
 - interactive latency targets (docs/34) hold on the reference workload;
 - cache and publication are crash-safe.
@@ ### G6 — Proof service @@
 - Lean foundations kernel-check (Revision 2 and Revision 3 theorems, no
   placeholders);
+- pinned Lean environment with per-request isolation and cancellation;
+- every proof receipt carries theorem and axiom manifests;
+- agent proof repair is accepted only by the kernel;
 - certificate mutations are rejected;
 - proof Context Packs improve proof-worker success/cost.
@@ ### G8 — Human usability @@
-- task study shows explanation improves diagnosis (per the preregistered
-  study in §21.1);
-- confidence calibration does not worsen;
+- the preregistered study (§21.1) covers both cohorts — Rust newcomers
+  completing the deterministic/causal workflow, and distributed-systems
+  experts diagnosing real failures — and shows explanation beats the
+  raw-trace baseline on diagnosis accuracy and time;
+- assurance confidence is calibrated, not merely no worse than baseline;
 - progressive disclosure reaches exact artifacts;
@@ ### G9 — Corpus parity @@
 - 80 validated TLA+ families at their declared parity level
   (`corpus/tla-examples/PARITY_LEVELS.md`), measured per §19.4's held-out
-  discipline.
+  discipline;
+- every family carries its interaction artifacts: native model, expected
+  verdict and state facts, meaningful explanation, failure/mutation task,
+  proof/refinement support where applicable, and an agent benchmark
+  artifact (docs/52 G9 — this is *interaction* parity, per B23).
@@ ### G10 — Continuum 1.0 @@
 - two real project migrations;
+- at least one migrated project stops requiring a separate TLA+ workflow
+  for normal development;
-- production evidence path;
+- production evidence returns valid pass/fail/inconclusive
+  classifications (INV-008);
 - public ContinuumBench;
```

---

### [CRITICAL] Change #2: Fix the Phase C corpus-parity inversion (P4 requires Phase D machinery)

**Current State:**
Phase C delivers "Wave 0/1 corpus interaction tasks (29 families …) at their required parity per `corpus/tla-examples/PARITY_LEVELS.md`". The CSV's `required_parity` for Wave 0/1 is: Wave 0 — 9×P2, 1×P3, 9×P4; Wave 1 — 3×P2, 2×P3, 5×P4. So 14 of 29 families are required at P4 (sorry-free Lean 4 proof with axiom manifests and a bridge theorem) and 3 more at P3 (fair-cycle/ranking certificates) — but the Lean theorem/certificate pipeline, fairness/liveness debugger, and ranking synthesis are all **Phase D** deliverables, and G6 closes in Phase D. Phase C's own exit text never mentions Lean, consistent with Phase C having no proof service and therefore inconsistent with its own deliverable. `PARITY_LEVELS.md`'s ramp agrees with Phase D, not C ("0.2: … three P4 theorems" vs the 14 Phase C implies). Separately, the corpus release contract (0.1/0.2/0.5/1.0) maps to no phase, and its "0.5: at least five P5 exemplars" has no machine referent — no CSV row carries `required_parity = P5`.

**Proposed Change:**
Cap Phase C's corpus deliverable at P2; raise Wave 0/1 to full required parity in Phase D; map the release-contract checkpoints to phases; designate the P5 exemplar families in the CSV.

**Rationale:**
As written, Phase C either cannot exit or exits by silently ignoring 17 of its 29 families' declared targets — the "green badge with hidden scope" pattern the plan prohibits. The parity ramp already exists in `PARITY_LEVELS.md`; the plan just has to schedule against it honestly.

**Benefits:**
- Phase C becomes exitable with Phase C machinery.
- The corpus release contract gains an owner per checkpoint.
- The P5 target finally gets a checkable referent (review 1 flagged the three-way P5 disagreement; it is still unresolved).

**Trade-offs:**
- Phase D grows by the Wave 0/1 P3/P4 completion; that work was always Phase D-shaped.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ ### Phase C — Interactive scale, deliverables @@
 - Wave 0/1 corpus interaction tasks (29 families, pinned to
   `tlaplus/Examples@91c22ea…`) at their required parity per
-  `corpus/tla-examples/PARITY_LEVELS.md`;
+  `corpus/tla-examples/PARITY_LEVELS.md`, capped at P2 (bounded semantic
+  parity): 14 Wave 0/1 families are required at P4 and 3 at P3, which
+  need the Phase D proof and liveness machinery; Phase C delivers every
+  family at min(required, P2);
@@ ### Phase D — Proof and liveness, deliverables @@
-- corpus Waves 2–3 at required parity.
+- Wave 0/1 families raised from the Phase C P2 cap to their full
+  required parity (P3/P4);
+- corpus Waves 2–3 at required parity;
+- corpus release checkpoints mapped to phases: 0.2 closes in Phase D;
+  0.5 (Waves 0–2 at required parity plus ≥5 P5 exemplar families, now
+  designated by name in `validated-examples.csv`) closes in Phase E;
+  1.0 (all 80) closes in Phase F with G9.
```

---

### [CRITICAL] Change #3: Make each phase exit able to close its assigned gates

**Current State:**
The §22 table assigns gates to phases, but four exits cannot close them. Phase A (closes G0/G1/G2) exits on "Die Hard and Dining Philosophers … identical artifacts" — nothing about the shell-baseline ablation, the prompt-injection corpus, or continuation-resume validation. Phase D (closes G6) exits on "one nontrivial corpus protocol has safety and liveness evidence plus real-code refinement" — nothing about isolation, axiom manifests, or kernel-only proof acceptance. Phase E (closes G7) exits without typed holes/CEGIS, materialization, or the unrealizability/unknown distinction. Phase F closes G8 but **no Phase F deliverable produces the usability study**; and its exit ("replaces bespoke DST plus separate TLA+ workflow in at least two … systems") is strictly stricter than docs/52 G10 ("two projects remove bespoke DST; **at least one** stops requiring a separate TLA+ workflow") with neither text acknowledging the other.

**Proposed Change:**
Extend the four exit lines so each names its gate-closing evidence; add the study to Phase F deliverables; keep the plan's stricter Phase F exit but reconcile docs/52 G10 to it explicitly (per Change #1's bidirectional rule).

**Rationale:**
A phase whose exit is weaker than its gates either stalls at gate review or promotes with the gate silently open. Both failure modes are the ones §21.1's "gate-driven, not time-driven" doctrine exists to prevent.

**Benefits:**
- Gate review becomes a checklist over the exit evidence, not a negotiation.
- G8 gains a producing work item (currently none exists anywhere in §21).

**Trade-offs:**
- Exits get longer; they were fictional while short.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ ### Phase A exit @@
-Exit: Die Hard and Dining Philosophers can be checked through native API, CLI, and an agent client with identical artifacts.
+Exit: Die Hard and Dining Philosophers can be checked through native API,
+CLI, and an agent client with identical artifacts; the ACI ablation shows
+the typed surface beats disciplined shell use on success and cost, or the
+protocol is redesigned before freeze (G0-DX-10); the prompt-injection
+corpus cannot trigger privileged operations (G2); continuation resume
+validates epochs and inputs (G1).
@@ ### Phase D exit @@
-Exit: one nontrivial corpus protocol has safety and liveness evidence plus real-code refinement.
+Exit: one nontrivial corpus protocol has safety and liveness evidence
+plus real-code refinement, produced by a proof service that holds G6:
+pinned Lean environment, per-request isolation and cancellation, theorem
+and axiom manifests on every receipt, and agent proof repair accepted
+only by the kernel.
@@ ### Phase E exit @@
-Exit: Forge rediscovers known solutions and produces at least one behaviorally novel candidate independently verified within the declared envelope.
+Exit: Forge rediscovers known solutions through typed holes and finite
+CEGIS, produces at least one behaviorally novel candidate independently
+verified within the declared envelope, materializes model, Rust skeleton,
+and proof obligations for a promoted candidate, and reports
+unrealizability as distinct from unknown.
@@ ### Phase F — Production and ecosystem, deliverables @@
 - ContinuumBench public release;
+- the preregistered G8 usability study (per §21.1) executed and analyzed;
 - multi-agent workbench.
@@ ### Phase F exit @@
 Exit: Continuum replaces bespoke DST plus separate TLA+ workflow in at least two materially different real systems.
+(This is deliberately stricter than docs/52 G10's "at least one stops
+requiring a separate TLA+ workflow"; docs/52 is updated to match, per
+§22's reconciliation rule.)
```

---

### [CRITICAL] Change #4: Stage G0 honestly — Phase A cannot close it as sequenced

**Current State:**
The §22 table assigns G0 to Phase A and G0 reads "All items in `notes/G0_SPIKE_MATRIX.md` have evidence or explicit redesign." But the unevidenced items map to machinery far beyond Phase A: DX-06 (neighborhood/mutation campaign) → repair transactions, PR 21/Phase B; DX-11 (proof-service isolation) → PR 28/Phase D; DX-09 (human diagnosis study) → a Phase-F-scale study now governed by G8's preregistration rule; DX-15 (benchmark leakage) → the corpus/benchmark program, no PR at all. Under START_HERE's ordering, G0 cannot close before PR 29 — two phases after the plan says it must. The matrix itself has no status/evidence/decision columns, so the plan's "8 of 15" count (§0.3) has no home in the artifact the gate points at, and the matrix's per-item "Failure consequence" column cannot record whether an item has evidence or a redesign decision.

**Proposed Change:**
Keep G0 a Phase A gate but scope its Phase A closure to the freeze-blocking subset (items whose experiments run against Phase A machinery: DX-01–05, 07, 08, 10, 12, 13, 14); re-home the four later-machinery items to the gates that own their subsystems (DX-06 → G4, DX-11 → G6, DX-09 → G8, DX-15 → G9), with the re-homing recorded in the matrix as the explicit decision. Require the matrix to gain Status/Evidence/Decision columns (executed in Change #5's reconciliation pass).

**Rationale:**
"All 15 in Phase A" is impossible as sequenced, which means either Phase A never exits or G0 closes by hand-waving — and G0 is the falsification gate; it above all must not be closable by assertion. Re-homing is not weakening: each re-homed item still blocks a gate, just the gate whose machinery its experiment needs, and the freeze-blocking subset regains docs/52's "blocks interface freeze" teeth.

**Benefits:**
- Phase A's gate set becomes closable by Phase A work.
- Each falsification item gets exactly one accountable gate.
- §0.3's status line gets a durable home.

**Trade-offs:**
- G0 is no longer a single early checkpoint; it was never actually one.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ ### G0 — Load-bearing falsification (after the Change #1 text) @@
+G0 closes in Phase A for every item whose required experiment runs
+against Phase A machinery: DX-01–05, 07, 08, 10, 12, 13, 14. An
+unexecuted or failed item in this subset blocks interface freeze. Items
+whose experiments require later subsystems are re-homed to the gates
+that own them — DX-06 (neighborhood/mutation campaign) → G4, DX-11
+(proof-service isolation) → G6, DX-09 (human diagnosis study) → G8,
+DX-15 (benchmark leakage) → G9 — and each re-homing is recorded in the
+matrix as that item's explicit decision. The matrix carries Status,
+Evidence, and Decision columns; §0.3's counts are derived from it, not
+asserted beside it.
```

---

### [HIGH] Change #5: Mandate the execution-layer reconciliation (START_HERE and the G0 matrix are pre-review artifacts)

**Current State:**
`notes/START_HERE_IMPLEMENTATION.md` and `notes/G0_SPIKE_MATRIX.md` are unchanged since before review 1, while §25 delegates execution to them. Divergences (all verified): no PR 0 despite §25's "before PR 5" RFC-expansion mandate (START_HERE contains zero occurrences of "RFC" or "ADR"); all Lean work at PR 28 despite Phase A's Lean deliverable; PR 9 builds a singular "certificate service" with no `continuum-kernel-*` split, no wire-form/serialization boundary, no size covenant; PR 2 makes hash identity primary, the opposite of ADR-0013's certified-lane rule now in §20; no CML parser PR in the Phase B band while PR 25 (LSP) depends on CML diagnostics; no SARIF PR while Phase C delivers SARIF; PR 22's receipt has no gate-profile or `NotYetEnforced` fields (a receipt built to its spec violates §21); PR 5's operation names (`workspace get`, `task start`, `artifact get`) and PR 13's CLI commands (no `explain`, `debug`, or `repair` — the three commands §3.1 advertises) contradict §10.2/§13.4; the island diagram names a `continuum-protocol` crate that exists in no §20 list; no PR names a gate; §21.1's Phase A license/redistribution decisions have no PR. The G0 matrix has no status columns and no preregistration note for DX-09.

**Proposed Change:**
Extend §25 with an explicit execution-layer reconciliation mandate enumerating these corrections, and add validator checks (gate-citation translation, plan↔docs/52 bullet correspondence, START_HERE gate annotations).

**Rationale:**
§25 says "the plan is a map, not the spec" — but the *execution* plan is what engineers will follow, and it currently encodes the pre-review architecture. Every divergence listed will otherwise be rediscovered as a mid-PR fight.

**Benefits:**
- The first 30 PRs build the revised architecture, not the superseded one.
- The validator prevents this class of layer-skew from recurring.

**Trade-offs:**
- One more documentation pass before code. It is the same pass §25 already requires for RFCs; this extends it two files further.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ ## 25. Immediate execution, after the RFC-expansion paragraph @@
 plan prose and RFC disagree, the RFC is corrected and becomes normative. The
 plan is a map, not the spec.
+
+The execution layer is reconciled in the same pass; both files predate
+this revision. `notes/START_HERE_IMPLEMENTATION.md`: add PR 0 (the RFC
+expansion above plus the §21.1 license and corpus-redistribution
+decisions), move Lean pinning and the T0/T1 seed theorems into the
+Phase A band (the proof *service* remains PR 28), rewrite PR 9 for the
+four `continuum-kernel-*` crates with wire-form checking and the size
+covenant, align PR 2 with ADR-0013's canonical-identity rule, add a
+Phase B CML core-fragment parser PR and a Phase C SARIF PR, align PR 5
+operation names and PR 13 commands with §10.2/§13.4 (including
+`explain`, `debug`, and `repair`), give PR 22 receipts the gate-profile
+and `NotYetEnforced` fields, remove the unregistered `continuum-protocol`
+crate, and annotate every PR with the gate(s) it advances.
+`notes/G0_SPIKE_MATRIX.md`: add Status / Evidence / Decision columns
+(the home of §0.3's counts), the DX-09 preregistration requirement, and
+the G0 re-homing decisions of §22. `tools/validate_dossier.py` gains
+checks for gate-citation translation, plan↔docs/52 bullet
+correspondence, and START_HERE gate annotations.
```

---

### [HIGH] Change #6: Repair the frontier lane register — misquoted thresholds, missing lanes, unresolved TBDs

**Current State:**
Verified against the lane notes: (a) the causal-minimization row's "≥10× on non-artificial traces" is actually research/01:101's kill rule for *exploration reduction* (DPOR class reduction), rewritten to a different lane, and drops 01's mandatory conjunct "without a serious regression on dependent workloads"; research/26 (the actual minimization lane) has no numeric threshold. (b) "ambiguity rate" (lenses row) names a quantity research/31 never defines — no denominator, no corpus. (c) The certificate row truncates docs/31:58's disjunct ("…or provides acceptable asynchronous CI latency"). (d) research/05 and /09 still have no thresholds — the "TBD" rows are accurate and unresolved, though both notes contain the raw material (05's causal-information-budget measures and overhead tiers; 09's 10-mutant corpus and 8 named invariants). (e) The register omits: the weak-memory lane (ADR-0032 — B11 says "until the lane ships" but nothing ships it), timed/probabilistic (ADR-0016, same situation), and five of docs/31's six numeric thresholds (observer independence ≥5×/<20%/zero mutation loss; cubical ≥3× on ≥2 real protocol classes; semiring within 15%; assumption synthesis ≥5 liveness cases; abstraction discovery). §24.5 and docs/31 cover disjoint lane sets and intersect only at the ≤10% row.

**Proposed Change:**
Split the misattributed row in two; define the ambiguity metric obligation; restore the truncated disjunct; replace the two TBDs with draft thresholds pending lane-owner ratification plus a rule that TBD may not survive Phase A; add the weak-memory and timed rows; merge docs/31's thresholds so the program has one lane authority.

**Rationale:**
The register was created (review 1, Change #8) to be "authoritative for lane status." An authority that misquotes its sources and omits lanes that constitutional bets depend on (B11's envelope dimensions) is worse than none — decisions will cite it.

**Benefits:**
- One lane authority; docs/31 stops being an orphaned second register.
- The weak-memory dimension of every envelope stops being permanently unowned.
- research/05 and /09 get falsifiable targets instead of standing TBDs.

**Trade-offs:**
- The draft numbers below are proposals, not measurements; ratification by lane owners is explicitly required.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ ## 24.5 Frontier lane register, table @@
-| Causal minimization (§6, §12) | research/01, /26 | ≥10× on non-artificial traces; kill if cost dominates verification | 1-minimal delta debugging only |
+| Exploration reduction (§9, INV-013) | research/01; docs/31 | ≥10× (order of magnitude) reduction in explored classes on a non-artificial corpus subset **without regression on dependent workloads** (research/01); observer-indexed: median ≥5× on the observer-sensitive class, certificate overhead <20%, zero mutation loss (docs/31) | conservative unreduced exploration |
+| Causal minimization (§6, §12) | research/26 | replay-preserving core ≤10% of trace length on real (non-synthetic) failures — draft, pending ratification; kill if minimization cost dominates verification | 1-minimal delta debugging only |
@@ same table @@
-| Production conformance (Phase F) | research/05 | monitorability-gated; thresholds TBD (must be added — currently absent) | Lab-replay evidence only |
-| Cancellation calculus (§0, B19) | research/09 | mutation corpus; thresholds TBD (must be added — currently absent) | runtime checking only |
+| Production conformance (Phase F) | research/05 | monitorability-gated; draft pending ratification: faithful Lab reproduction of ≥70% of curated known incidents under injected telemetry loss; always-on overhead ≤1%; kill if most target properties are monitorable only as `Inconclusive` on two real systems | Lab-replay evidence only |
+| Cancellation calculus (§0, B19) | research/09 | draft pending ratification: 10/10 mutation-corpus mutants detected with zero false alarms on the correct implementation; machine-checked drain ranking for the replicated register | runtime checking only |
@@ same table @@
-| Bidirectional lenses (§16) | research/31 | ambiguity rate; kill if most mappings too ambiguous | get-only projection + drift detection |
+| Bidirectional lenses (§16) | research/31 | ambiguity rate := fraction of abstract edits on the drift corpus yielding multiple or no concrete candidates (metric and corpus to be added to research/31); kill if most real mappings are too ambiguous, or users mistake candidate synchronization for verified preservation | get-only projection + drift detection |
@@ same table @@
-| Certificate overhead | docs/31 | checking ≤10% of search time | reduce certified-lane scope |
+| Certificate overhead | docs/31 | checking ≤10% of search time, or acceptable asynchronous CI latency (docs/31's full rule) | reduce certified-lane scope |
+| Weak-memory lane (B11) | ADR-0032 — lane to be opened | reproduces the standard litmus corpus under the declared model before any envelope upgrade; until then every memory dimension reads `Unsupported(sequential-consistency-only)` | SC-only, declared as a 1.0 non-goal |
+| Timed/probabilistic semantics | ADR-0016 — lane to be opened | per-ADR staging; until shipped, timing fields in Intent Contracts remain declarative assumptions (B11) | declarative assumptions only |
@@ after the table @@
+A register row may not carry an unratified or `TBD` threshold past
+Phase A; such a row blocks its lane's promotion. docs/31's remaining
+quantitative thresholds (cubical reduction ≥3× on ≥2 real protocol
+classes; semiring within 15% of specialized analyses; assumption
+synthesis on ≥5 liveness cases; abstraction discovery) are merged into
+this register during the §25 pass so the program has exactly one lane
+authority; where docs/31 and a research note disagree, the note is
+corrected and cited.
```

---

### [HIGH] Change #7: Complete the intent lifecycle — acceptance operations and the §0.1/§5.2 self-conflict

**Current State:**
§3.1 (added in review 1) requires draft intents to "gain protection (INV-001) only on explicit acceptance" — but §10.2's intent namespace is `intent.get / diff / propose_revision`: there is no operation that accepts, rejects, or locks an intent, in the plan or in RFC 0027's registry (which mirrors §10.2 exactly). Separately: §0.1 promises "security and privacy policy" as an Intent component, but §5.2's structure (revised in review 1) does not carry it, and RFC 0037:88 explicitly parks it as an open question — plan §0.1 and §5.2 now disagree with each other. §5.2's bounds line ("values, processes, faults, schedules, depth") is two-against-one stale versus RFC 0037 and the schema ("values, nodes, faults, depth"). §5.2's observers ("state/event/knowledge/security projections") are richer than the schema (events + state projection only).

**Proposed Change:**
Add `accept / reject / lock` to the intent namespace (privileged, capability `revise-intent`); add the security-policy field group to §5.2, resolving RFC 0037's open question in favor of a first-class group; align the bounds vocabulary with RFC 0037; keep the knowledge/security observer projections and add the schema backfill to Appendix A.

**Rationale:**
The `Proposed → protected` transition is the single most security-sensitive state change in the intent lifecycle (it is what INV-001 protects), and it currently has no operation — meaning the first implementation will invent one outside the reviewed surface. The §0.1/§5.2 fork means the plan's declaration of intent and its intent specification disagree about what intent *is*.

**Benefits:**
- The acceptance path becomes a typed, capability-gated, auditable operation.
- §18.4's redaction policy gets the intent-side field it reads from.
- One bounds vocabulary across plan, RFC, and schema.

**Trade-offs:**
- Schema change (pre-1.0, cheap now).

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ ### 10.2 Core operations @@
-intent.get / diff / propose_revision
+intent.get / diff / propose_revision / accept / reject / lock
@@ ### 5.2 Intent structure @@
 bounds
-  values, processes, faults, schedules, depth
+  values, nodes, faults, depth
 assurance policy
   accepted evidence classes and required checkers
 optimization
   hard constraints, soft objectives, non-vacuity
+security policy
+  data classification, redaction classes, and capability requirements
+  (first-class per §0.1; resolves RFC 0037's open question)
 change policy
   who/what may modify each field
@@ ### 5.4 Intent locks, after the policy block @@
 Agent repair capabilities exclude intent mutation unless a task explicitly asks for redesign.
+
+`intent.accept` is the only transition from `Proposed` to protected
+status; it requires the `revise-intent` capability, produces an audit
+record, and is never performed implicitly by `init`, `check`, or any
+repair operation. `intent.lock` edits the §5.4 policy table under the
+same capability.
```

---

### [HIGH] Change #8: Add a semantic-epoch advance policy (§4.6)

**Current State:**
Semantic epochs are load-bearing in at least twelve places (INV-006 replay, INV-010 parity, §4.2 snapshots, §9.2 query keys, §9.6 continuations, §10.3 `ContinuationEpochMismatch`, receipts, the blocker doctrine's "publish affected semantic epochs") — but nothing in the plan says what happens when an epoch *advances*: whether existing receipts stay valid, what fraction of the evidence graph invalidates, whether two epochs can coexist during migration, or how the cost is estimated before committing. ADR-0018 (semantic versioning and replay) exists and is never cited by the plan. §4.5 covers crash, GC, purge, and multi-user, but not upgrade — the one operational event guaranteed to happen at every release.

**Proposed Change:**
Add §4.6 defining epoch-advance semantics: epoch-scoped evidence validity, typed per-class compatibility statements, dual-epoch operation, fork-not-migrate continuations, `SUPERSEDES`-linked re-derivation, and pre-advance blast-radius reporting.

**Rationale:**
Without this policy, the first engine improvement forces an undocumented choice between mass-invalidating all evidence (users lose their receipts) and silently keeping stale results (INV-010 violation). Both are trust-spine failures; the policy is cheap to state now and expensive to retrofit.

**Benefits:**
- Release engineering gets a contract instead of an emergency.
- The blocker doctrine's "publish affected semantic epochs" becomes actionable.
- INV-009's monotonicity extends coherently across upgrades.

**Trade-offs:**
- Dual-epoch support is real daemon complexity; scoping it to two concurrent epochs bounds it.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ after ### 4.5 Operational contract of `continuumd` @@
+### 4.6 Semantic epoch advance
+
+Epochs are content identities; advancing one never mutates existing
+artifacts (ADR-0018):
+
+- evidence is epoch-scoped: a receipt remains verifiable under its
+  pinned epoch indefinitely (INV-006, INV-014); an epoch advance never
+  silently revalidates or invalidates a published receipt;
+- each advance publishes a typed per-artifact-class compatibility
+  statement — `Preserved | Revalidate | Incompatible` — and an estimated
+  invalidation blast radius by artifact class before it is applied;
+- the daemon may hold at most two epochs concurrently during migration;
+  new work defaults to the newest; continuations resume only under
+  their pinned epoch (§9.6) and are forked, never migrated in place;
+- re-derived artifacts receive new identities linked to their
+  predecessors by `SUPERSEDES` edges; nothing is rewritten in place.
```

---

### [MEDIUM] Change #9: Register the missing handle prefixes and protocol error codes

**Current State:**
Two handle prefixes are in active normative use but absent from §4.4's registry: `cap_*` (capability — required in every request envelope per RFC 0026:29, defined in RFC 0027:41's five-level authority order) and `diff_*` (semantic/intent diff — `semantic-diff.schema.json` and the checked-in examples use it). §10.3's error taxonomy (12 codes) matches RFC 0026 verbatim, but the RFC adds three protocol-level codes the plan omits — and one, `IdempotencyKeyReused`, backs a MUST (RFC 0026:38). All cross-kind schema reference fields still use one untyped pattern that accepts any prefix (which is how `cp_` drifted originally); there is no shared `$defs` handle library.

**Proposed Change:**
Add both prefixes to §4.4; add the three protocol codes to §10.3 marked protocol-level; add the shared-`$defs` schema task to Appendix A.

**Rationale:**
§4.4 claims to be the artifact-class registry; an incomplete registry cannot back the per-kind pattern enforcement that would have prevented the `cp_`/`crash_` drift. Mechanical, and cheapest now.

**Benefits:**
- The closed prefix set becomes enforceable in schemas.
- Agents get the full recoverable-error vocabulary.

**Trade-offs:** None.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ ### 4.4 Content-addressed artifacts @@
 forge_* synthesis archive
 cont_* resumable task continuation
+cap_* capability
+diff_* semantic/intent diff
@@ ### 10.3 Error taxonomy @@
 CapabilityDenied
 PolicyGateFailed
+ProtocolVersionUnsupported   (protocol-level, RFC 0026)
+IdempotencyKeyReused         (protocol-level, RFC 0026)
+MalformedRequest             (protocol-level, RFC 0026)
```

---

### [MEDIUM] Change #10: Make the G8 study executable — docs/48 cannot support the preregistration §21.1 promises

**Current State:**
§21.1 gates G8 on "a preregistered minimal study: the four docs/34 acceptance workflows, cohort sizes and metrics fixed per docs/48 before the study runs." Verified: docs/34's four workflows exist, but docs/48 contains metric *names* only — no cohort definitions (the word "preregistration" does not appear in it; the only cohort language is a research question about "experts vs newcomers"), no N, no instruments, no thresholds, no analysis plan. docs/52 G8 names the two required cohorts (Rust newcomers; distributed-systems experts) and the raw-trace baseline — both dropped from plan §22 (restored by Change #1) and never named in §21.1. As written, §21.1 points at a document that cannot fix the study, and no phase produces the preregistration document.

**Proposed Change:**
Name the cohorts and comparator in §21.1 and make authoring the preregistration document (an expanded docs/48) a Phase E deliverable, so it exists before the Phase F study runs.

**Rationale:**
G8 is a release gate; a gate defined against a nonexistent study design silently becomes either unpassable or pro-forma. The two-cohort structure is also what makes G8's claims meaningful — "explanation improves diagnosis" is a different claim for newcomers than for experts.

**Benefits:**
- G8 becomes passable by a small team on a known budget.
- The preregistration exists before anyone has results to fit it to.

**Trade-offs:**
- Commits Phase E to study-design work; that is when it must happen anyway.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ ### 21.1 Resourcing, licensing, and study posture @@
-(ADR-0029). The G8 usability gate is defined against a preregistered
-minimal study: the four docs/34 acceptance workflows, cohort sizes and
-metrics fixed per docs/48 before the study runs.
+(ADR-0029). The G8 usability gate is defined against a preregistered
+minimal study: the four docs/34 acceptance workflows, two cohorts (Rust
+newcomers completing the deterministic/causal workflow; distributed-
+systems experts diagnosing real failures), a raw-trace baseline
+comparator, and cohort sizes, instruments, and pass thresholds fixed in
+a preregistration document — an expanded docs/48 — published before the
+study runs. Authoring that document is a Phase E deliverable; the study
+itself runs in Phase F.
```

---

### [MEDIUM] Change #11: Resolve the latency-table status conflict and its missing "explain" row

**Current State:**
Phase C's exit makes the docs/34 latency table a hard gate ("meets the docs/34 latency table (p50/p95 per interaction)"). docs/34:81 says the opposite: "These are product targets, not initial guarantees." Additionally, the gate names the "edit/check/**explain** loop," but no table row is an explain/diagnose interaction (nearest: "replay/minimized branch load," "Context Pack compilation"), so the gate's referent is under-specified; and docs/52 G5 states the criterion without citing docs/34, so the numbers live in exactly one non-normative place.

**Proposed Change:**
Declare the table gate-normative for G5 (superseding docs/34's caveat, with docs/34 updated per the reconciliation rule) and require an explain-interaction row (failure → rendered causal explanation) to be added to the table before Phase C begins.

**Rationale:**
A gate whose numeric criterion self-describes as "not a guarantee" is not a gate. The explain row matters because explanation latency is the product thesis (§0, B21) — it is the one interaction the table cannot omit.

**Benefits:**
- G5's exit becomes measurable with an agreed instrument.
- The p50/p95 numbers acquire exactly one normative home.

**Trade-offs:** None of substance.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ ### Phase C exit @@
 Exit: the edit/check/explain loop meets the docs/34 latency table (p50/p95
-per interaction) on the reference workload; incremental results are
+per interaction) on the reference workload — the table is gate-normative
+for G5, superseding docs/34's "product targets, not initial guarantees"
+caveat, and gains an explain-interaction row (failure → rendered causal
+explanation) before Phase C begins; incremental results are
 trustworthy under differential audit; reduction engines show zero
```

---

### [MEDIUM] Change #12: Declare one configuration surface (unapplied review-1 remnant)

**Current State:**
Review 1 Change #14 had three parts; the output fix and intent bootstrapping were applied, but the configuration-surface unification was not. §3.1 configures via `[package.metadata.continuum]` with keys `entry`/`profile`; `examples/continuum.project.toml` still defines an incompatible surface (`[continuum]`, `[workspace] rust_entry`, `[profiles.dev]`, `[adapters]`) — and its line 3 references intent by workspace file path (`intent = "intent/ack-durable.json"`), which now contradicts §4.2's rule that intent lives only in the registry, referenced by content identity.

**Proposed Change:**
State the one-surface rule in §3.1; fix the example file to the same schema with a registry-identity intent reference (Appendix A task).

**Rationale:**
docs/34's explicit anti-goal is "forcing users to maintain duplicate configurations." The file-path intent reference is worse than duplication: it is a worked example of the exact intent-custody bug review 1's Change #4 closed.

**Benefits:**
- One schema; the example stops teaching the anti-pattern.

**Trade-offs:** None.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ ### 3.1 Five-minute human path, after the toml block @@
 cargo continuum init
 cargo continuum check
 [bash fence ends]
+
+This Cargo metadata is the single configuration surface for Rust
+projects; model-only projects use the same schema in a standalone
+`continuum.toml`. Configuration references intent by registry identity
+(`in_*`), never by a workspace file path (§4.2).
```

---

### [MEDIUM] Change #13: Trust-spine durability — backup/restore and reproducible checker identity

**Current State:**
§4.5 (added in review 1) covers crash atomicity, GC, purge, and multi-user — but not backup, restore, or loss: "backup," "restore," and "disaster" appear nowhere in the plan or docs/35. An append-only evidence ledger and audit log that exist in one copy on one disk are a single point of total trust loss. Separately, INV-014 records checker *identity*, but nothing requires the kernel crates to build reproducibly, so "checker identity" cannot be independently re-derived from sources — the reproducible-build idea appears once, in docs/09's mitigation list, and never in the plan.

**Proposed Change:**
Add a durability bullet to §4.5 and a reproducible-build rule to §20's dependency rules.

**Rationale:**
The purge design (key-shredding with `Redacted` stubs) already implies the harder half of this machinery; backup/restore is the easy half and the one every real deployment hits first. Reproducible kernel builds are what make "zero known paths to promote false evidence" (G10) auditable by a third party rather than taken on faith.

**Benefits:**
- G1's crash-safety story extends to media loss.
- Receipts become independently re-derivable end to end.

**Trade-offs:**
- Reproducible builds constrain the kernel's build environment; the kernel is small by covenant, which is what makes this feasible.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ ### 4.5 Operational contract of `continuumd`, after the purge bullet @@
+- **Durability and restore.** The CAS, evidence ledger, intent registry,
+  and audit log support backup and verified restore; restore runs the
+  index verifier, and a receipt whose referenced content was lost
+  reports `Redacted(lost, commitment)` rather than disappearing. Audit
+  logs are exportable as append-only streams.
@@ ## 20, dependency rules @@
 - foreign tools remain isolated behind normalized artifacts.
+- kernel crates build reproducibly from pinned sources; every receipt
+  records the checker's build digest and toolchain identity (INV-014),
+  so checker identity is independently re-derivable.
```

---

### [LOW] Change #14: Close the gate-translation holes the §22 clause leaves open

**Current State:**
§22's grandfather clause covers only "Revision 2 ADRs (0001–0035) and RFCs (0001–0025)" — it says nothing about `docs/`, and Rev-2-era docs carry live untranslated citations: docs/08:318 states the Quint kill question as "after **G2**" where plan §24 says "after **G4**" (the same criterion; Rev-2 G2 ≈ Rev-3 G4, stated nowhere), plus docs/08:20,24,37 and docs/20:17,45. `docs/04` is a live, unarchived "Implementation Roadmap" whose gate graph matches *neither* docs/26 nor docs/52 (its G0C/G0D and G1–G3 differ from docs/26 — a third scheme), organizes by workstreams no other document references, and contains a competing 1.0 definition. Three RFCs use `-Corpus`/`-Proof` gate suffixes and ADR-0022 defines an internal G0–G4 Lean ladder — a fourth and fifth reuse of the `G<n>` namespace, which §22's own "no new gate numbering" rule forbids. The promised "translation sweep" is mentioned in §22 but §25's specification pass never enumerates it.

**Proposed Change:**
Extend the grandfather clause to Rev-2-era docs; annotate the §24 Quint criterion; add docs/04's archive-or-banner disposition and the suffix/ladder retirements to the §25 sweep (the sweep itself is Appendix A work).

**Rationale:**
These are exactly the citations a future contributor will resolve wrong silently — docs/04 is the natural first stop for "what do I build, in what order," and it answers with a scheme two revisions stale.

**Benefits:**
- Every legacy gate citation in the dossier becomes classifiable.

**Trade-offs:** None.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ ## 22. Release gates, intro @@
-section summarizes it. Legacy gate citations in Revision 2 ADRs (0001–0035)
-and RFCs (0001–0025) refer to the Revision 2 scheme in `docs/26` and may
-not be cited without translation; the translation sweep is part of the
-specification pass in §25. No document may introduce a new gate numbering.
+section summarizes it. Legacy gate citations in Revision 2 ADRs
+(0001–0035), RFCs (0001–0025), and Revision 2-era docs (`docs/01`–`docs/32`,
+including the risk register `docs/08` and the objections in `docs/20`)
+refer to the Revision 2 scheme in `docs/26` and may not be cited without
+translation; the translation sweep is part of the specification pass in
+§25 and includes retiring the `-Corpus`/`-Proof` gate suffixes (RFCs
+0011/0012/0019) and ADR-0022's internal Lean G-ladder, and archiving or
+Rev-2-bannering `docs/04`, whose gate graph matches neither `docs/26` nor
+`docs/52`. No document may introduce a new gate numbering.
@@ ## 24. Kill criteria, Quint bullet @@
 - a candid comparison shows Quint plus existing DST tooling would be
   cheaper and equally strong for the target users; a “yes” after G4 closes
-  is a program-level failure signal.
+  is a program-level failure signal (docs/08 states this criterion as
+  “after G2” in the Revision 2 scheme; Rev-2 G2 ≈ Rev-3 G4).
```

---

## Appendix A — Dossier-level fixes (no plan.md diff; fix in the named artifact)

**Stale reference docs (highest value — these are what agents and integrators will read):**
- `docs/55`: `CrashpackHandle cp_...` → `crash_...`; add the 4 missing prefixes (`model_`, `cir_`, `proof_`, `crash_`) plus `cap_`/`diff_`; add the 6 missing operation namespaces and 7 missing debug verbs; `proof.open_goal/apply` → `proof.goal/attempt`; `forge.candidates` → `forge.archive`; `debug.step` → `step_event`/`step_abstract`; add `verification.await`. Simplest durable fix: regenerate docs/55 and docs/36 from RFC 0026/0027 and state they are projections of the RFCs.
- `docs/36`: `cp_1` → `crash_1` (lines 60, 62); `proof.open/apply/search` → plan names; add `cost` and `next_operations` to the result envelope; add the error taxonomy.
- `examples/agent_workflow.md:30`: `cp_7m` → `crash_7m`.
- `examples/continuum.project.toml`: converge on the §3.1 schema (Change #12); intent by `in_*` identity, not file path.

**Stale gate/naming sites:**
- `docs/52:46`: "clean-build differential Tribunal" → "Incremental Parity Audit" (the normative gate doc carries the retired name).
- `docs/14_GLOSSARY.md:69`: rewrite **Tribunal** to the ADR-0021 corpus-oracle sense only; add an **Incremental Parity Audit** entry. This glossary entry is the root of the ambiguity.
- `docs/33:95`, `docs/50:56`, ADR-0044 title (propagates to `adr/README.md:55`, `MANIFEST.md:105`), START_HERE PR 24 title: same rename.
- `docs/19:12` "cross-engine Tribunal" is a third sense under the plan's "exclusively" rule — rename (e.g., "cross-engine differential harness"); `docs/10:45` "Differential Tribunal" → "Corpus Differential Tribunal" for disambiguation.
- `schemas/crashpack.schema.json:24`: id pattern `^cp:` → `^crash:` (last artifact still branding itself `cp`).

**Schema layer (contract completeness):**
- Author `evidence-graph-edge.schema.json` — the 13 §11.3 edge types have **no machine encoding anywhere**; RFC 0038 (19 lines, still bare "Draft") names 6 of 13. This is the largest remaining plan-vs-machine gap.
- `evidence-graph-node.schema.json`: add `inconclusive_reason` (INV-008's six values — three of them currently exist only in plan §11.4) and the `CHECKED_CERTIFICATE`/`TRUSTED_SOLVER` discriminator (currently only on context packs, not on the evidence surface where §11.4 places it); encode `checked(N=k)`/`cutoff_checked(N≤k)`/`proved(∀N)`; reconcile the schema's 20th node kind (`candidate`) with §11.2 (recommend: add `SynthesisCandidate` to §11.2 — Forge needs it and `synthesis-candidate.schema.json` exists).
- Finish the `workspace`→`snapshot` field rename (RFC 0026:31 says "everywhere"): `context-pack` (line 265), `benchmark-task` (79), `semantic-diff` (10, 18), `repair-transaction` (80, 84) still use `workspace`-family keys.
- Hoist handle patterns into a shared `$defs` library with per-kind patterns for cross-references (the current untyped catch-all admits any prefix — the mechanism by which `cp_` drifted).
- `intent-contract.schema.json`: add knowledge/security observer projections (§5.2 promises them); add the `security_policy` group per Change #7; note `assumptions` family enum lacks `failure` (lives in `fault_model` — document the mapping).
- Reconcile RFC 0027:45's "byte budgets are the enforced contract" with the task-budget schema, which has no `bytes` key (it lives only in the context-pack `content_budget`).

**Research/benchmark hygiene:**
- research/05 and research/09: backfill kill criteria, baselines, and thresholds per the research README's own 7-point contract (draft numbers in Change #6; both notes already contain the metric raw material).
- research/31: define the ambiguity metric and its drift corpus.
- research/26: adopt its own numeric threshold (Change #6's draft) rather than inheriting research/01's by misquote.
- `validated-examples.csv`: designate the ≥5 P5 exemplar families (Change #2) so `PARITY_LEVELS.md:80` has a machine referent; note the release contract's 0.5 and 1.0 rows currently key off different columns.

**Validator additions (`tools/validate_dossier.py`):**
- plan §22 ↔ docs/52 bullet correspondence; gate-citation translation check (covering `docs/` per Change #14); START_HERE gate-annotation check; handle-prefix closed-set check against §4.4; operation-name check of docs/36/55 against RFC 0027's registry.

## Appendix B — Verified clean / applied correctly

- All 18 review-1 changes are present in `plan.md`; the internal §21↔§8.2 gate-profile mapping is consistent; §22's Phase↔Gate table, §0.3, §4.5, §24.5, and §21.1 all landed as proposed.
- `verify.start` and `.cml` are fully eradicated (only archival hits in `plan.review.1.md`); `crash_*` is consistent within `plan.md` and the schemas.
- RFC 0027's operation registry reproduces §10.2 exactly; RFC 0026 carries the full 12-code error taxonomy; all seven Rev-3 RFCs cite Rev-3 gates correctly with parenthetical titles.
- `repair-transaction.schema.json` is exemplary: 12-gate enum in plan order, `gate_profile`, and structural enforcement that `not_yet_enforced` is the only path to promote with an unshipped gate — exactly §21's rule.
- The verification-task budget carries all seven B18 dimensions (plus a justified `candidates` extension); `sampled` and the 9-value status lattice are in the schemas.
- Corpus numbers check out end to end: 80 validated families, Wave 0/1 = 29, one pinned commit everywhere (`91c22ea…`), CSV/`corpus-summary.json`/checker mutually consistent.
- docs/34 does contain the p50/p95 latency table (7 rows) and the four acceptance workflows §21.1 cites — the citation is real; only its normativity was unresolved (Change #11).
- Rev-3 docs 33–55 and ADRs 0036–0052 contain no stale Rev-2 gate citations; the G0-DX-nn namespace is used consistently.
