# Review of `plan.md` (Revision 3 Master Implementation Plan) — third pass

**Review date:** 2026-07-25
**Scope:** `plan.md` as revised by the review-2 fix wave (commit `0b68967 docs: plan review 2`), verified against the dossier it cites: `docs/52` (normative gates), the seven load-bearing RFCs and 18 schemas, `notes/START_HERE_IMPLEMENTATION.md` and `notes/G0_SPIKE_MATRIX.md`, the corpus parity contract (`corpus/tla-examples/`), the §24.5 research lanes, `tools/validate_dossier.py`, and `SHA256SUMS.txt`. This review does not repeat `plan.review.1.md` or `plan.review.2.md`; it verifies how their changes landed and then reviews dimensions neither pass covered: distribution and team workflows, engineering economics, scale, and the soundness of the plan's own audit machinery.

---

## Executive Summary

### What improved — and what was verified clean this pass

The review-2 uptake crossed the layer boundary that review 2 flagged. Verified correct this pass: the G0 matrix has the Status/Evidence/Decision columns and its counts and re-homings match §0.3 and §22 exactly; all nine mandated START_HERE edits are applied (PR 0, Lean/T0-T1 at PR 4a, the four `continuum-kernel-*` crates at PR 9, CML parser 15a, SARIF 25a, gate annotations throughout); the corpus claims are numerically exact (29 Wave 0/1 families, 14×P4 + 3×P3, all 80 rows pinned to `91c22ea…`, five named P5 exemplars in `validated-examples.csv`); every §24.5 lane threshold now matches its source note verbatim except one stale parenthetical; the 12 repair gates and the phase-staged `not_yet_enforced` profile are fully propagated into RFC 0032 and structurally enforced by the repair-transaction schema; the error taxonomy matches across all three surfaces; docs/04 carries its Rev-2 banner; no stale "Tribunal" usages survive; and all 300 dossier hashes verify.

### The five systemic problems of this revision

1. **§22's reconciliation claim is false in the one direction its own validator cannot see.** The plan states "no `docs/52` criterion is dropped here" and that the validator enforces "bullet-for-bullet correspondence" — but eight docs/52 bullets have no plan counterpart (G1's explicit-handles bullet; G4's exact/neighboring replay and promotion receipt; G7's typed-holes/CEGIS rediscovery, unrealizability-vs-unknown distinction, and materialization; G10's bespoke-DST removal), and `check_gate_scheme_correspondence` only iterates plan bullets looking for docs/52 matches, never the reverse. The validator passes today with the drop in place. The exact mechanism review 2 installed to prevent gate-scheme divergence is silently one-directional.

2. **§25 has inverted its staleness.** The section still presents as *future* work things the fix wave already completed (the RFC expansion — done except the RFC 0026 IDL file; all START_HERE edits; the G0-matrix columns; the validator checks), while the genuinely undone remainder from the same sweep is nowhere listed: the docs/34 explain row and gate-normative marker, the docs/48 preregistration expansion, ADR-0022's still-live internal G-ladder, the RFC 0026 IDL, and the doc-propagation of §4.5/§4.6 (purge/`Redacted`, backup/restore, existence-oracle rule, two-epoch migration, `Preserved|Revalidate|Incompatible` exist **only** in plan.md — docs/35, RFC 0026, ADR-0018, and docs/42 know none of it). A reader of §25 would re-do finished work and never find the open items. One citation is also simply wrong: T0/T1 and axiom manifests are attributed to ADR-0022, which contains neither; they live in RFC 0012 and ADR-0035.

3. **The Intent Contract — the plan's central integrity mechanism — cannot survive contact with a second machine.** Review 1 correctly moved intent out of writable snapshots and into a daemon registry; nothing anywhere (plan, RFC 0037, docs/35) says how `in_*` identities reach a teammate's daemon, a fresh clone, or the CI daemon §17.6 requires. There is no export/import format, no branch-divergence merge semantics, and no PR-review projection of an intent diff. In any multi-machine setting, "protected intent" currently means "whatever the daemon you happen to talk to says" — an intent-integrity hole, not an ergonomic gap.

4. **The plan's largest engineering artifact and its guardian audit are both under-de-risked.** The incremental semantic database (§9) — memoized queries *plus* four trust-classed reuse edges, semantic dependency discrimination, epoch-keyed continuations, and crash-safe publication — has no build-vs-adopt decision, no spike, no G0 item, and no docs/08 risk row; it is implicitly classified as settled DESIGN and delivered wholesale in Phase C. And the Incremental Parity Audit that guards it (§9.5) is ill-posed for the engines it must audit: SMT/PDR/portfolio solvers legitimately diverge between clean and incremental runs under budgets and restarts, so the audit as written must either quarantine reuse classes on noise (destroying G5 interactivity) or be tuned to ignore divergence (destroying the audit).

5. **The economics of the honesty machinery are ungoverned.** Every repair transaction version re-runs a neighborhood campaign (§8.5's example: 38,412 classes) plus two mutation batteries; the parity audit re-runs clean computation at an unspecified sampling rate; receipts pin entire campaigns as GC roots forever (millions of `ev_*` nodes/year with no reclamation path that preserves receipt verifiability); and the daemon has no admission control between an interactive human `check` and a saturating agent swarm — while G5's latency table is gate-normative. The only cost governance in the plan is G10's undefined "operating cost acceptable." Without cost attribution, amortization, and retention rules, the predictable outcome is the one §5.1 warns about — gates quietly weakened under budget pressure, by humans.

Alongside these, the contract layer has specific high-privilege drift: the three most privileged operations (`intent.accept/reject/lock`) are absent from RFC 0027's normative registry; `security_policy` landed in the intent schema but no change-policy verb can lock it; and Context Packs promise "ReplayPreserving under the pinned epoch" while the schema has no epoch or content-hash field to pin.

Changes below are ordered by priority. Diffs are anchored to the current `plan.md` by section context; companion-file diffs name their anchor where exact context is long.

---

## Proposed Changes

---

### [CRITICAL] Change #1: Restore the eight dropped docs/52 bullets and make the gate validator bidirectional

**Current State:**
plan.md §22 (line 2009–2012) claims "no `docs/52` criterion is dropped here" and that `tools/validate_dossier.py` enforces bullet-for-bullet correspondence. Eight docs/52 criteria have no plan counterpart: G1 "explicit handles across native API" (docs/52:25); G4 "exact and neighboring replay" (:56), "mutation challenge" (:57), "promotion receipt" (:59); G7 "typed holes and finite CEGIS" (:81), "explicit unrealizability/unknown distinction" (:86), "materialized model/Rust/proof obligations" (:88); G10 "two materially different real projects remove bespoke DST infrastructure" (:107). `check_gate_scheme_correspondence` (validate_dossier.py:343–377) iterates only plan→docs/52; the validator passes with the drops in place.

**Proposed Change:**
(a) Add the eight bullets to plan §22. (b) Make the validator symmetric: iterate docs/52 bullets against plan §22 with the same ≥0.6-overlap match, failing on orphans in either direction. (c) While in the validator: `check_gate_citation_hygiene` scans only Rev-3 files for retired gate suffixes; extend it to fail on untranslated Rev-2 gate citations in any non-archived file, per §22's translation rule.

**Rationale:**
This is the same defect review 2's Change #1 fixed, recurring one layer down: the correspondence *mechanism* was built with a blind side, so the reconciliation claim is unverifiable exactly where it is currently false. Three of the dropped bullets are soundness-relevant (replay, mutation challenge, receipt at G4) and one changes the meaning of 1.0 (G10's DST-removal requirement is the plan's own §21 Phase F exit — dropping it from §22 G10 recreates the docs/52-vs-plan asymmetry in the opposite direction from review 2).

**Benefits:**
- The reconciliation claim becomes true and machine-checked in both directions.
- G4/G7/G10 summaries can no longer silently weaken the normative gates.

**Trade-offs:**
- Symmetric fuzzy matching will need a few waiver annotations where one plan bullet legitimately covers two docs/52 bullets (G8 today).

**Implementation Notes:**
Run the extended validator before committing; it should fail on exactly the eight orphans, then pass after the plan edit. Keep docs/52 untouched — this direction of drift is plan-side.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ §22 G1 @@
 - continuation resume validates epochs and inputs before any reuse;
+- every stateful workflow is reachable through explicit handles across
+  the native API (no session-implicit state);
 - cancellation closes obligations and publishes no partial finality;
@@ §22 G4 @@
 - partial-order debugger including alternate-branch exploration;
 - human review view over the repair transaction;
+- exact and neighboring replay reproduce the failure and its
+  perturbations; the mutation challenge battery runs on every promotion;
+- promotion emits a receipt naming its gate profile;
 - repair transaction closes exact, neighborhood, and mutation gates under
   the Phase B gate profile (§21).
@@ §22 G7 @@
 ### G7 — Forge

+- known solutions are rediscovered through typed holes and finite CEGIS;
 - safety and non-vacuity;
 - hidden variant generalization;
 - independent candidate verification;
 - diversity archive has semantic, not merely syntactic, spread;
-- unrealizability produces reusable evidence where supported.
+- unrealizability produces reusable evidence where supported, and is
+  reported as distinct from unknown in every surface;
+- a promoted candidate materializes model, Rust skeleton, and proof
+  obligations.
@@ §22 G10 @@
 - two real project migrations;
+- both migrations remove bespoke DST infrastructure in materially
+  different systems;
 - at least one migrated project stops requiring a separate TLA+ workflow
```
Validator (tools/validate_dossier.py, `check_gate_scheme_correspondence`): after the existing plan→docs loop, add the mirrored docs→plan loop over the same parsed bullet sets and report orphans as failures; add an explicit `# waiver` list for known one-covers-two cases.

---

### [CRITICAL] Change #2: Give the Intent Contract a distribution, merge, and CI model

**Current State:**
§4.2 (plan.md:526–531) stores Intent Contracts "only in the intent registry," and §3.1 (:390–393) forbids referencing intent by file path. RFC 0037:17 confirms the registry is daemon-local. Nothing defines: how an accepted `in_*` reaches a second developer's daemon or CI; what happens when two branches each carry an accepted intent revision; or how an intent diff appears in a GitHub PR (docs/37's review workflow assumes Continuum's own review view). §17.6 requires CI to produce intent diffs but CI's daemon has no defined way to obtain the intent.

**Proposed Change:**
Add §4.2.1 "Intent distribution and convergence": signed, content-addressed *intent bundles* (`inb_*`) as the interchange form (vendorable in-repo or fetched by identity); idempotent import; semantic three-way merge over §5.3 diffs with mandatory `Conflict` on non-independent revisions; CI verification of bundle presence and acceptance-chain integrity; a SARIF/PR projection of intent diffs. Register `inb_*` in §4.4 and add the corresponding RFC 0037 section to the §25 debt ledger (Change #8).

**Rationale:**
Intent integrity (B1, INV-001, G3) is defined against a single authoritative registry, but development is multi-machine by default. Without a distribution model, every team deployment either copies intent by hand (unauditable) or points all tools at one shared daemon (unstated, and contradicted by the local-first daemon story in §4.1/docs/35). Branch divergence is the sharpest case: two independently *accepted* revisions are both legitimate; text merge is meaningless for property ASTs; silent last-write-wins would be exactly the smuggled intent change INV-001 exists to prevent. This is not an ergonomic gap — whichever daemon CI consults currently defines what "protected" means.

**Benefits:**
- Team and CI workflows become implementable without weakening INV-001.
- Intent review enters the tool developers already use for review (the PR).
- Acceptance-chain verification makes intent provenance auditable offline.

**Trade-offs:**
- Adds a signing/identity requirement earlier than the §4.5 multi-user baseline planned for; local-only single-user mode can stub the signature but not the bundle format.

**Implementation Notes:**
The bundle is an export of registry state, not a second home: the registry remains the runtime authority; the bundle is how registries converge. Merge must be conservative — independence of two intent diffs is decidable only within supported fragments (§5.3), so anything else is a `Conflict`, consistent with the `Unknown`-fails-closed rule.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ §4.2, after the intent-custody paragraph @@
 Intent Contracts are stored and versioned only in the intent registry,
 outside every writable or forkable snapshot. `workspace.fork` preserves the
 intent binding by identity; rebinding a snapshot lineage to a different
 intent is a privileged operation that produces a semantic intent diff and
 invalidates dependent evidence (INV-001).
+
+#### 4.2.1 Intent distribution and convergence
+
+Registries converge through signed, content-addressed **intent bundles**
+(`inb_*`): an export of one or more contracts with their acceptance
+records and policy tables. Bundles may be vendored in the repository or
+fetched by identity; import is idempotent and never changes protection
+status — an imported `Proposed` contract stays `Proposed`, and an
+imported acceptance is honored only if its signature chain satisfies the
+local policy. Divergent branches are reconciled by semantic three-way
+merge: each head's §5.3 diff against the common ancestor; two revisions
+merge automatically only when both diffs are classified independent
+within supported fragments — otherwise the merge is a `Conflict` node
+requiring the `revise-intent` capability. CI fails closed when the
+bundle referenced by the workspace configuration is absent or its
+acceptance chain does not verify, and §17.6's intent diff ships with a
+PR-reviewable projection (SARIF note plus rendered semantic diff).
@@ §4.4 handle registry @@
 in_* intent contract
+inb_* signed intent bundle
```

---

### [CRITICAL] Change #3: Resolve build-vs-adopt for the incremental semantic database before Phase C, with a spike and a risk row

**Current State:**
§9 specifies the most demanding infrastructure component in the plan — memoized semantic queries with four trust-classed reuse-edge kinds (§9.3), semantic (not syntactic) dependency discrimination (§9.4), epoch-keyed continuations, crash-safe publication, and a statistical self-audit — delivered wholesale as a Phase C deliverable (:1906). The dossier contains no build-vs-adopt decision (salsa/Adapton appear only in the bibliography), no spike, no G0 item, and no docs/08 risk row; §24.5 registers only *sub-file granularity* (research/27) as HYPOTHESIS, implicitly marking the core engine settled.

**Proposed Change:**
Add a Phase B deliverable: an ADR resolving build-vs-adopt (salsa-derived vs custom), backed by a spike implementing the four reuse-edge classes and `query.explain_invalidation` over a bounded query set (parse/elaborate/explore) with a measured invalidation-precision baseline. Phase C is `BLOCKED` without the ADR. Add a docs/08 risk row.

**Rationale:**
rust-analyzer's salsa took years to stabilize with a strictly simpler trust model (one reuse class, syntactic keys, no crash-safety or audit obligations). If Continuum builds custom, this is plausibly the longest engineering pole of Phases C–F; if it adopts, the Validated-witness and edge-class semantics must be layered onto a framework not designed for them, which constrains `continuum-incremental`'s architecture (§20) and the shape of the parity audit. Either answer materially changes Phase C's cost — and the dossier, otherwise obsessed with de-risking (fifteen G0 items, a lane register with kill criteria), schedules zero de-risking for its single biggest artifact.

**Benefits:**
- The G5 critical path gets a decision point before the investment, not during it.
- The spike produces the first real data on invalidation precision — the quantity that decides whether interactive latency and audit trustworthiness can coexist.

**Trade-offs:**
- Adds scope to Phase B. The spike can be small (three query kinds, in-memory) because the decision hinges on the trust-model fit, not throughput.

**Implementation Notes:**
The ADR should explicitly decide where reuse-edge classes live (in the framework's dependency graph vs in a layer above it) — that placement determines whether the §9.5 audit can attribute a mismatch to a reuse class at all.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ §21 Phase B, Deliver list @@
 - repair transactions;
 - exact and neighboring replay.
+- an ADR resolving build-vs-adopt for the §9 incremental engine
+  (salsa-derived vs custom), backed by a spike implementing the four
+  reuse-edge classes and `query.explain_invalidation` over
+  parse/elaborate/explore with a measured invalidation-precision
+  baseline; Phase C is `BLOCKED` until this ADR exists.
@@ §9.1, end of section @@
 Re-running every model, extraction, exploration, proof, and explanation
 from scratch would make Continuum irrelevant in normal development.
+
+The engine itself is engineering risk, not settled design: whether it is
+derived from an existing memoization framework or built custom is decided
+by a Phase B ADR with spike evidence (§21), and the decision is carried
+as a docs/08 risk with the mitigation "edge classes collapse to
+Conservative" named as the failure mode that destroys interactivity.
```

---

### [CRITICAL] Change #4: Re-scope the Incremental Parity Audit for nondeterministic and budget-bounded engines

**Current State:**
§9.5 (plan.md:1003–1018) compares incremental vs clean "verdict … counterexample class; certificate result …" and mandates: "Any disagreement quarantines the relevant reuse class." The Phase C determinism matrix (:1928–1931) covers semantic artifacts only; docs/42:63 concedes incremental solving is provisional. Nothing addresses that SMT/PDR/portfolio engines are not pure functions of their inputs: restarts, wall-clock heuristics, and budget consumption legitimately produce `Inconclusive` vs `Refuted`, or different counterexample classes, between two honest runs.

**Proposed Change:**
Partition queries by auditability class and scope the quarantine rule to the classes where disagreement is actually evidence of a defect.

**Rationale:**
As written, §9.5 self-destructs on first contact with a solver-backed query: either noise quarantines reuse classes (G5's latency gate dies as everything recomputes) or the team tunes the audit to ignore divergence (the audit becomes theater, and INV-010's "exactness" claim silently loses its checker). The plan already solved the analogous problem for replay (INV-006 pins epochs and downgrades honest divergence to engine defect); parity needs the same typed treatment.

**Benefits:**
- The audit's signal stays meaningful: an equality-class mismatch is always a bug.
- Budget-bounded engines can participate in incremental reuse without poisoning the audit.
- INV-010 gets an enforceable meaning per class instead of a global one it can't keep.

**Trade-offs:**
- Certificate-auditable comparison requires both runs to emit checked certificates, which costs checking time — but that cost is already accepted in the docs/31 certificate-overhead lane.

**Implementation Notes:**
The class assignment belongs on the query definition (§9.2), not on the run, so the audit can't be gamed by re-classifying a query after a mismatch. Record budget/portfolio-attributed divergence — it is drift telemetry even when it doesn't quarantine.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ §9.5, after the artifact list @@
 - semantic diff;
 - proof axiom manifest.

-Any disagreement quarantines the relevant reuse class and emits a minimal invalidation counterexample.
+Queries carry an auditability class, declared on the query definition:
+
+- **Equality-auditable** — deterministic under the docs/19 matrix;
+  compared bit-for-bit. Any disagreement quarantines the reuse class and
+  emits a minimal invalidation counterexample.
+- **Certificate-auditable** — solver-backed; the audit compares checked
+  certificates and claim envelopes, never raw solver behavior. A
+  certificate-level disagreement quarantines; a solver-outcome
+  difference with agreeing certificates does not.
+- **Budget-sensitive** — anytime results; the audit checks only that the
+  incremental result's evidence labels are no stronger than a clean
+  run's under equal budget (monotone-honesty), and records divergence as
+  drift telemetry without quarantine.
+
+Divergence attributable solely to budget or portfolio nondeterminism
+never quarantines a reuse class; divergence in an equality- or
+certificate-auditable query always does. INV-010's exactness claim
+applies per auditability class.
```

---

### [HIGH] Change #5: Register the privileged intent operations (and fix `task.start`) in the normative protocol registry

**Current State:**
plan §10.2 (:1049) and §5.4 (:729–733) give `intent.accept / reject / lock` normative semantics ("`intent.accept` is the only transition from `Proposed` to protected status"), and docs/55:65–79 documents them — but RFC 0027's operation registry (rfcs/0027:21–22) has only `intent get/diff/propose_revision`, despite the RFC's own claim that "The registry is the plan §10.2 list." The three most privileged operations in the system have no authority-level assignment in the normative registry. Separately, rfcs/0026:61 says "`task.start` is idempotent," but no `task.start` operation exists in any registry (tasks start via `verification.start`, `forge.create`, etc.).

**Proposed Change:**
Add the three intent operations to RFC 0027's registry with explicit authority levels (`revise-intent` capability; `intent.lock` additionally gated on policy administration), and either note deliberately that agent capability profiles exclude them by default, or split them into a "privileged, human/policy-holder only" registry section. Fix RFC 0026's phrasing to "task-starting operations (`verification.start`, `forge.create`, …) are idempotent under idempotency keys."

**Rationale:**
INV-015 (agent least authority) and G2 depend on the registry being the complete, typed authority map. An operation that can flip a contract from `Proposed` to protected while being absent from the authority lattice is precisely the hole a capability audit would miss: an adapter implementing "the registry" would omit them (fine) or add them ad hoc from docs/55 (catastrophic, because no authority level is specified). The `task.start` slip is minor but sits in the idempotency-critical sentence of the transport RFC.

**Benefits:**
- The privileged surface is enumerated where the security review looks for it.
- MCP/adapter implementers cannot accidentally expose intent acceptance.

**Trade-offs:** None.

**Implementation Notes:**
This is an RFC-side fix (plan and docs/55 already agree); add it to the Change #8 debt ledger so it is tracked to closure.

**Git-Diff:**
```diff
--- rfcs/0027-agent-tool-protocol.md
+++ rfcs/0027-agent-tool-protocol.md
@@ registry table, after the `intent propose_revision` row @@
+| intent | accept | revise-intent + policy-admin | transitions `Proposed` → protected; audit-recorded; never callable through default agent capability profiles |
+| intent | reject | revise-intent | closes a `Proposed` revision with a typed reason |
+| intent | lock | revise-intent + policy-admin | edits the §5.4 policy table; audit-recorded |
```
```diff
--- rfcs/0026-continuumd-native-protocol.md
+++ rfcs/0026-continuumd-native-protocol.md
@@ idempotency section @@
-`task.start` is idempotent
+Task-starting operations (`verification.start`, `model.check`,
+`forge.create`, and peers) are idempotent under idempotency keys
```

---

### [HIGH] Change #6: Make `security_policy` protectable — policy verb, required field, and the stale RFC 0037 question

**Current State:**
Review 2's fix landed `security_policy` as a first-class intent group (intent-contract.schema.json:363–393) and plan §5.2 (:674–676) says this "resolves RFC 0037's open question." But: rfcs/0037:88 still poses the open question and asserts "the schema currently does not carry it separately" (now false on both counts); `security_policy` is absent from the schema's `required` list and from the change-policy verb group (schema:312–358); it is missing from RFC 0037's protected set of ten; and §5.4's lock table (:713–725) has no row for it. Plan §5.2 (:677–678) promises change policy covers "who/what may modify each field."

**Proposed Change:**
Add `security_policy` (and `optimization`, which has the same gap for its non-`non_vacuity` part) to the §5.4 policy table, the schema's policy verbs and `required` list, and RFC 0037's protected set; delete RFC 0037's stale open-question paragraph.

**Rationale:**
A first-class field that no lock verb can protect is an unprotected privileged surface: an "ordinary" edit to redaction classes or capability requirements would not classify as a privileged intent change under §5.3, because the change-policy table doesn't know the field exists. Data-classification weakening is exactly the §18.4 exfiltration path — a repair that reclassifies a field from redacted to visible should be as privileged as one that weakens a property.

**Benefits:**
- Closes the one intent field that could currently be modified without tripping INV-001.
- Removes a normative RFC actively misdescribing its own schema.

**Trade-offs:** None.

**Implementation Notes:**
The semantic-diff classifier needs a corresponding dimension (redaction weakening/strengthening) or the lock is unenforceable in the diff — add it to RFC 0031's lattice alongside the assurance order.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ §5.4 policy table @@
 assurance = "no-downgrade"
 observers = "proof-required"
+security_policy = "maintainer-review"
+optimization = "maintainer-review"
```
Schema: add `security_policy` and `optimization` to `intent-contract.schema.json`'s `required` array and to the `policy` verb group; RFC 0037: add both to the protected set and delete the §"open questions" paragraph about `security_policy`; RFC 0031: add a `redaction weakened/strengthened` classification to the diff lattice.

---

### [HIGH] Change #7: Pin Context Packs — epoch and content-hash fields, debugger handle, and the missing content kinds

**Current State:**
Plan §6.2 (:767–769) requires a pack to carry "exact replay and debugger handles" and "schema, semantic epoch, and content hash." The schema (context-pack.schema.json) has `schema_version` but no semantic-epoch and no content-hash field; only a `replay` handle (`crash_*`), no `dbg_*` field; and its `selected.kind` enum (`event, state_delta, source, model, proof, assumption, counterfactual, unknown`) cannot represent three §6.2 content classes: obligation/resource flow, missing/reversed order constraints, and heuristic repair surfaces. RFC 0028 defines `ReplayPreserving` as "replays to the same verdict **under the pinned epoch**" (rfcs/0028:25) — with no field anywhere to pin the epoch in. Also: rfcs/0028:57 closes omission reasons to four values, but the schema's `omissions[].reason` is a free string, and the RFC's required per-omission expansion query has no schema field (only a boolean `expandable`); RFC 0028's nine-stage pipeline cites "(plan §6.3)" but omits §6.3's abstraction/refinement correspondence stage.

**Proposed Change:**
Schema: add required `semantic_epoch` and `content_hash`; optional `debugger` (`dbg_*`); extend `selected.kind` with `obligation_flow`, `order_constraint`, `repair_surface` (the last constrained `heuristic: true`); close `omissions[].reason` to the RFC enum and replace `expandable: boolean` with an optional `expansion` query reference. RFC 0028: add the correspondence stage or correct the "(plan §6.3)" citation.

**Rationale:**
Context Packs are the agent-facing product (B5) and the artifact G2 measures. `ReplayPreserving` is the pack's strongest guarantee, and it is currently unverifiable: without an epoch field, a checker cannot even state what the claim is relative to; without a content hash, INV-017's provenance chain has a gap at the most-consumed artifact. The missing content kinds mean the plan's flagship failure story — "missing order" (§0's Ack-before-Durable, B6's "missing order" bullet) — has no typed representation in its own delivery format: it must be smuggled through prose in an `event` node, which is exactly the terminal-sludge failure mode B2 exists to prevent.

**Benefits:**
- The pack's headline guarantee becomes independently checkable.
- Agents get typed access to order constraints and obligation flow — the two objects repair hypotheses are made of.
- Omission manifests become machine-verifiable (INV-007) instead of conventionally honest.

**Trade-offs:** Schema-breaking change; costless now (pre-freeze), which is why it should land before PR 5.

**Implementation Notes:**
`content_hash` should cover the canonical pack encoding per ADR-0013's canonical-identity rule (certified lanes), not a raw byte hash of one serialization.

**Git-Diff:**
```diff
--- schemas/context-pack.schema.json (description of edits)
+++ schemas/context-pack.schema.json
@@ top-level properties @@
+  "semantic_epoch": { "type": "string" },          // required
+  "content_hash":  { "type": "string" },           // required; canonical encoding per ADR-0013
+  "debugger":      { "pattern": "^dbg_" },          // optional
@@ selected.kind enum @@
-  "event", "state_delta", "source", "model", "proof", "assumption", "counterfactual", "unknown"
+  "event", "state_delta", "source", "model", "proof", "assumption",
+  "counterfactual", "obligation_flow", "order_constraint",
+  "repair_surface", "unknown"
@@ omissions items @@
-  "reason": { "type": "string" }, "expandable": { "type": "boolean" }
+  "reason": { "enum": ["budget", "redaction", "unsupported", "heuristic-cutoff"] },
+  "expansion": { "$ref": "#/definitions/expansion_query" }   // optional; replaces bare boolean
```

---

### [HIGH] Change #8: Correct §25's inverted staleness — record what is done, ledger what is genuinely open

**Current State:**
§25 (:2269–2293) still mandates, as future work "before PR 5": the RFC expansion (already substantially spec-form — RFC-2119 headers, field tables, closed enums; only the RFC 0026 IDL file is missing), all nine START_HERE edits (all applied), the G0-matrix columns (present), and the three validator checks (present). Meanwhile the same sweep's genuinely undone items appear nowhere: docs/34 lacks the explain row and still reads "product targets, not initial guarantees" with no gate-normative marker (plan :1921–1925 makes it G5-normative); docs/48 contains nothing acknowledging the preregistration requirement (:1848–1849); ADR-0022's internal Lean G-ladder is still live (retirement mandated at :2004–2005); §21 Phase A (:1864–1867) cites "per ADR-0022" for T0/T1 and empty axiom manifests — content that lives in RFC 0012:42–55 and ADR-0035, not ADR-0022; §24.5 (:2248) says research/31's ambiguity metric is "to be added" though it is defined at research/31:84–101; :2267 says "first 30 pull requests" against 31 PR headings; and §4.5/§4.6's operational content (purge/`Redacted`, backup/restore, existence-oracle dedup rule, two-epoch migration, `Preserved|Revalidate|Incompatible`) exists only in plan.md — docs/35, RFC 0026, ADR-0018, and docs/42 were not updated. docs/41 predates the 12-gate design entirely (see Change #16), and RFC 0038 is a 19-line stub while the schemas carry §11.4's normative content.

**Proposed Change:**
Rewrite §25's second and third paragraphs as (a) a completed-state acknowledgment and (b) an explicit open-debt ledger, and fix the two wrong claims in place (ADR-0022 citation; research/31 parenthetical; PR count).

**Rationale:**
§25 is the execution entry point; today it sends an implementer to redo four finished work packages while hiding the eight open ones. Review 2's core complaint was a plan trailing its dossier; the fix overcorrected into a plan that trails its own fix wave. The dossier's whole discipline is that claims match evidence — the plan's claims about its own dossier should meet the same bar, especially since the validator now exists to hold it there.

**Benefits:**
- The next work session starts on real debt instead of archaeology.
- The wrong ADR-0022 citation stops propagating (it already reached START_HERE PR 4a).

**Trade-offs:** None.

**Implementation Notes:**
Add a validator check that every ledger item names its target file, so the ledger can't rot the same way §25 did.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ §21 Phase A, Lean deliverable @@
-  (transition-system safety, stuttering simulation, finite-closure
-  certificate soundness) compile with no `sorry` and empty axiom manifests,
-  per ADR-0022.
+  (transition-system safety, stuttering simulation, finite-closure
+  certificate soundness) compile with no `sorry` and empty axiom manifests,
+  per the RFC 0012 theorem ladder and ADR-0035 axiom manifests.
@@ §24.5 lens row @@
-| Bidirectional lenses (§16) | research/31 | ambiguity rate := fraction of abstract edits on the drift corpus yielding multiple or no concrete candidates (metric and corpus to be added to research/31); kill if ... | get-only projection + drift detection |
+| Bidirectional lenses (§16) | research/31 | ambiguity rate := fraction of abstract edits on the drift corpus yielding multiple or no concrete candidates (metric defined in research/31; the drift corpus remains the lane deliverable); kill if ... | get-only projection + drift detection |
@@ §25 opening @@
-The exact first 30 pull requests are in [`notes/START_HERE_IMPLEMENTATION.md`](notes/START_HERE_IMPLEMENTATION.md).
+The ordered initial PR sequence (PR 0–PR 30, with inserts 4a/15a/25a) is
+in [`notes/START_HERE_IMPLEMENTATION.md`](notes/START_HERE_IMPLEMENTATION.md).
@@ §25 second/third paragraphs @@
-Before PR 5 (native protocol kernel) freezes any interface, the seven
-load-bearing Revision 3 RFCs — 0026, 0027, 0028, 0030, 0031, 0032, 0037 —
-are expanded from summaries to specifications: [...]
-The execution layer is reconciled in the same pass; both files predate
-this revision. `notes/START_HERE_IMPLEMENTATION.md`: add PR 0 [...]
-`notes/G0_SPIKE_MATRIX.md`: add Status / Evidence / Decision columns [...]
+The seven load-bearing Revision 3 RFCs (0026, 0027, 0028, 0030, 0031,
+0032, 0037) are expanded to specification form, and the execution-layer
+reconciliation (START_HERE PR 0/4a/9/15a/25a and gate annotations; the
+G0 matrix's Status/Evidence/Decision columns; the three validator
+checks) is applied. The following specification debt remains open and
+is paid before PR 5 freezes any interface:
+
+- RFC 0026: the normative IDL file (currently referenced, not present);
+- RFC 0027: register `intent.accept/reject/lock` with authority levels;
+- RFC 0037: intent-bundle distribution (§4.2.1) and the stale
+  `security_policy` open question;
+- RFC 0038 and docs/44: absorb §11.4's status lattice, promotion
+  authority (including `sampled`/`inconclusive`), typed Inconclusive
+  reasons, validation basis, and parameterization — and remove
+  docs/44's out-of-lattice `draft` status;
+- docs/34: add the explain-interaction latency row and mark the table
+  gate-normative for G5 (superseding its "product targets" caveat);
+- docs/48: the preregistration expansion (§21.1) — Phase E deliverable,
+  acknowledged in the doc now;
+- docs/35, RFC 0026, ADR-0018, docs/42: absorb §4.5/§4.6 (purge and
+  `Redacted(reason, commitment)`, backup/verified restore, the
+  cross-user dedup existence-oracle rule, two-epoch migration,
+  `Preserved | Revalidate | Incompatible` statements);
+- docs/41: regenerate around the 12-gate/phase-profile design (Change
+  #16 of the third review);
+- ADR-0022: retire the internal Lean G-ladder per §22.
```

---

### [HIGH] Change #9: Define the first-run contract, and pull a read-only tokio lane forward

**Current State:**
§3.1's five-minute path (:404–417) shows a rich FAILURE result that presupposes a bound model and accepted intent. The plan also specifies (:396–401) that `init` produces only `Proposed` drafts, never silently protected. What `cargo continuum check` does *between* those two moments — the actual first run, on a codebase with zero accepted intent — is unspecified: does it check unaccepted hypotheses (labeled how?), refuse, or output nothing? Separately, the only path for existing (tokio) Rust services — approximately all of them — is the Phase F "foreign-runtime instrumentation lane" (:1975–1981), scheduled *after* the G8 usability study and during the same phase as the two real migrations.

**Proposed Change:**
(a) Specify the first-run contract: with only `Proposed` intents, `check` runs them and reports every verdict as `Hypothesis(unaccepted-intent)` — structurally distinct from FAILURE/PASS — plus intent-free findings (effect footprint, uncontrolled nondeterminism, unmapped opaque effects) that deliver value with zero configuration. (b) Split the tokio lane: a read-only observation subset (lifecycle/channel journaling, opaque-effect inventory, `Observed`-status envelopes only) lands in Phase C; controlled-semantics claims stay in Phase F.

**Rationale:**
(a) is a B11/INV-008 issue, not just UX: the moment of maximum trust-formation is the first run, and it is exactly where "verified-looking output on unaccepted intent" could teach users the wrong meaning of the tool's verdicts. The plan's honesty machinery covers every state except the state every new user starts in. (b) is funnel arithmetic: G8 (usability, Phase F) and G10 (two migrations) both require a population of real users, but until Phase F every adopter must first migrate their runtime to asupersync *and then* adopt a verification workbench — two adoptions deep before first value. A read-only tokio lane in Phase C creates the top of the funnel (observe, explain, inventory — no semantics claims) that Phase F's case studies will need to recruit from, and its honest-envelope design is already specified by the Phase F text; only the journaling subset moves.

**Benefits:**
- First-run output is specified, honest, and useful with zero configuration.
- The G8/G10 populations exist by the time those gates need them.
- The kill criterion at :2221–2222 (bridge fails → no adoption path) gets tested three phases earlier, when redesign is cheap.

**Trade-offs:**
- Phase C gains scope; the mitigation is that the read-only lane needs no controlled scheduler, no domain-pack fidelity, and no replay guarantee — journal, project, explain, always `Observed`.

**Implementation Notes:**
The `Hypothesis(unaccepted-intent)` verdict class must be structurally distinct in every surface (§11.4's rule for parameterized results is the pattern to copy).

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ §3.1, after the Proposed-drafts paragraph @@
 intent to protected status, and never claims a generated model is the
 intended abstraction.
+
+First-run contract: while every governing intent is `Proposed`,
+`cargo continuum check` runs the drafts and reports each verdict as
+`Hypothesis(unaccepted-intent)` — structurally distinct from FAILURE
+and PASS in every surface — alongside intent-free findings that need no
+contract at all: effect footprint, uncontrolled-nondeterminism sites
+(INV-005), and unmapped opaque effects. This output is the specified
+zero-configuration value of the tool, not a degraded mode.
@@ §21 Phase C, Deliver list @@
 - proof Context Packs;
+- read-only tokio observation lane: journal observable lifecycle,
+  channel, and time events; opaque-effect inventory; envelopes capped at
+  `Observed` status with every claim dimension `Unsupported` — the
+  adoption top-of-funnel for the Phase F instrumentation lane, making
+  no controlled-semantics claim;
@@ §21 Phase F, first Deliver bullet @@
-- foreign-runtime instrumentation lane (tokio): journal observable
-  lifecycle/channel/time events, mark uncontrolled effects opaque, and emit
-  correspondingly bounded assurance envelopes — an adoption bridge, not a
-  claim of controlled semantics;
+- foreign-runtime instrumentation lane (tokio), upgrading the Phase C
+  read-only lane: instrumentation synthesis, fault-window replay where
+  monitorable, and correspondingly bounded assurance envelopes — an
+  adoption bridge, not a claim of controlled semantics;
```

---

### [HIGH] Change #10: Add cost governance for the verification bill

**Current State:**
B18 gives *per-task* budgets. Nothing aggregates or amortizes: §8.1 re-runs the full neighborhood campaign (§8.5's example: 38,412 classes) and both mutation batteries per transaction version; §9.5's parity audit re-runs clean computation at an unspecified sampling rate ("CI and sampled local runs"); Forge archives imply thousands of full verifications; and the only cost governance anywhere is G10's undefined "operating cost acceptable" (:2145). §19.3 measures cost but nothing governs it.

**Proposed Change:**
Add §8.6 "Cost governance": a cumulative cost ledger (CPU, wall, solver, token) in every repair receipt; incremental gate evaluation by default (neighborhood classes and mutants whose causal footprint is disjoint from the patch delta reuse prior results as `Validated` edges); the parity-audit sampling rate derived from a declared statistical confidence target per reuse class, reviewed at G5; per-principal and per-transaction cost ceilings that yield `BudgetExhausted` with a continuation — never a silently smaller campaign.

**Rationale:**
The gates are only as strong as the willingness to pay for them. Without amortization across transaction versions, an agent iterating on a repair pays the full campaign per attempt — so the practical pressure lands exactly where §5.1 predicts: on shrinking bounds, sampling instead of exhausting, and skipping mutation batteries — performed by budget-pressed humans rather than gaming agents, and invisible because no receipt records what the campaign cost or what a smaller campaign omitted. The "silently smaller campaign" prohibition is the cost-domain twin of INV-007.

**Benefits:**
- Repair iteration becomes affordable without weakening gates (disjoint-footprint reuse is exactly what §9's semantic dependencies are for).
- The parity audit's sampling rate becomes defensible instead of vibes.
- Reviewers see what assurance actually cost — the input G10's "acceptable" judgment needs.

**Trade-offs:**
- Footprint-disjointness computation is itself a §9 dependency query; it must be Conservative-class or the reuse is unsound. Fallback is full re-run, which is today's behavior.

**Implementation Notes:**
Fold "cost ledger present" into §8.2 gate 12's receipt requirements so it is structurally enforced like the gate profile.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ new §8.6 after §8.5 @@
+### 8.6 Cost governance
+
+Every repair transaction carries a cumulative cost ledger (CPU, wall,
+solver, memory, token) across all its versions; the promotion receipt
+includes it. Gate 5–7 evaluations are incremental by default:
+neighborhood classes and mutants whose causal footprint is disjoint from
+the patch delta (a Conservative-class §9 dependency query) reuse prior
+results as `Validated` edges; anything else re-runs. The §9.5 sampling
+rate derives from a declared statistical confidence target for mismatch
+detection per reuse class, reviewed at G5. The daemon enforces
+per-principal and per-transaction cost ceilings; exceeding one yields
+`BudgetExhausted` with a continuation — never a silently smaller
+campaign (the cost-domain form of INV-007).
```

---

### [HIGH] Change #11: Bound the evidence graph — summarizable campaign evidence and scale targets

**Current State:**
Receipts are GC roots (§4.5: reachability "from named roots, receipts, and retention policy"), and every promoted repair pins its full neighborhood campaign and mutation batteries via its receipt; docs/44:105 says superseded proposals "may be compacted but remain reachable from receipts." Net: a busy repository accretes campaign-scale evidence (tens of thousands of class results per repair) with no reclamation path that preserves receipt verifiability. Meanwhile the context compiler, semantic diff, and `evidence.query` traverse this graph interactively, and docs/34's latency table has no graph-scale row — no target says what p95 looks like at 10⁷ nodes.

**Proposed Change:**
Make campaign-class evidence summarizable: after promotion, raw per-class results may be rolled up into a coverage certificate whose checker attests the summary; receipts then reference the summary and raw classes become GC-eligible (`Redacted(summarized, commitment)` on access — reusing the §4.5 purge shape). Add a scale target to G5: evidence queries and context compilation meet the docs/34 table at ≥10⁷ evidence nodes on the reference workload.

**Rationale:**
This is the classic provenance-store-eats-the-product failure, and the plan's architecture makes it worse than usual because *honesty* is implemented as *retention*. The purge machinery §4.5 already built for privacy is the right shape for lifecycle too: a receipt that says "38,412 classes, summarized, commitment `c`" remains structurally verifiable, downgrades gracefully per §18.4 if someone needs the raw classes, and costs constant space. Discovering the retention problem at Phase F — when production evidence starts flowing — is the expensive time.

**Benefits:**
- Storage growth per promoted repair becomes O(1) after summarization instead of O(campaign).
- G5 gains the scale dimension its latency numbers silently assume.

**Trade-offs:**
- A summarization checker joins the trusted set; it belongs in `continuum-kernel-*` under the same covenant.

**Implementation Notes:**
Summarization must be opt-out per retention policy (a regulated deployment may need raw classes); the default is summarize-after-promotion.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ §4.5 Storage lifecycle bullet @@
 - **Storage lifecycle.** Artifacts are garbage-collected by reachability
   from named roots, receipts, and retention policy. The daemon reports
   storage attribution by artifact class. Disk exhaustion during
   publication aborts atomically (INV-017); it never truncates.
+  Campaign-class evidence (neighborhood, mutation) is summarizable:
+  after promotion, raw per-class results may be rolled up into a
+  coverage certificate attested by a kernel-covenant checker; the
+  receipt references the summary, raw classes become GC-eligible under
+  retention policy, and later access yields
+  `Redacted(summarized, commitment)` with the standard §18.4 downgrade.
@@ §22 G5 @@
 - interactive latency targets (docs/34) hold on the reference workload;
+- evidence queries and context compilation meet the docs/34 targets at
+  ≥10⁷ evidence nodes on the reference workload;
 - cache and publication are crash-safe.
```

---

### [MEDIUM] Change #12: Give `continuumd` a capacity model — priority classes, quotas, and load-honest G5 measurement

**Current State:**
§4.1 lists a "task scheduler and budget manager" component; §4.5's multi-user baseline covers identity/capabilities/dedup only. Nothing specifies admission control or priority between an interactive human `check` and a background agent swarm (§11.6 roles), daemon memory budgets (incremental cache residency, overlay snapshots, live `dbg_*` branches), or the load conditions under which G5's latency table is measured.

**Proposed Change:**
Add to §4.1: priority classes (interactive > CI > background/swarm) with preemption of suspendable tasks (B18 continuations make preemption safe); per-capability concurrency and resource quotas; a declared daemon memory budget with class-aware eviction; and G5 latency measured with a saturating background swarm, not on an idle daemon.

**Rationale:**
The workloads the plan celebrates are the ones that saturate the daemon. Per-task budgets bound each task, not interference between tasks — and a latency gate measured idle will pass while the shipped experience fails whenever agents are working, which is the intended steady state.

**Benefits:** G5 becomes honest under the product's own intended load; swarms can't starve humans.

**Trade-offs:** Preemption requires suspend points in engines; B18 already mandates them.

**Implementation Notes:** Quotas hang naturally off capabilities (§18.2), which are already the per-principal object.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ §4.1, after the task lifecycle diagram @@
+The scheduler defines priority classes — interactive > CI >
+background/swarm — with preemption of suspendable tasks (safe under B18
+continuations). Every capability grant carries concurrency and resource
+quotas. The daemon operates under a declared memory budget with
+class-aware eviction across the incremental cache, overlay snapshots,
+and materialized debugger branches. G5 latency compliance is measured
+with a saturating background swarm present.
```

---

### [MEDIUM] Change #13: Engine defects are first-class — the `defect_*` artifact and `continuum doctor`

**Current State:**
INV-006 (:320–322) downgrades a non-reproducing failure "to an engine defect" — and stops. docs/09's soundness-incident policy covers false-positive verdicts; the routine path — `ReplayDiverged`, parity mismatch, engine crash, wrong explanation — produces no defined artifact, no minimized reproduction, and no redaction-governed way to report it (a verification bug report contains the user's model and code).

**Proposed Change:**
Add §4.7: any `ReplayDiverged`, parity mismatch, engine crash, or explanation-validation failure emits a `defect_*` artifact — inputs pinned by content identity, epochs, engine/checker identity, automatically minimized reproduction — under the same redaction policy as Context Packs; `continuum doctor` assembles a shareable defect bundle; daemon health exports as OpenTelemetry alongside, never inside, semantic evidence. Register `defect_*` in §4.4.

**Rationale:**
The system's pitch is evidence discipline, and its own defects are currently the one failure class with no evidence container. Every other subsystem that hits a defect signal (quarantine in §9.5, downgrade in INV-006) needs somewhere typed to put it, or the signal dies in a log — the exact anti-pattern the plan bans for user-facing failures.

**Benefits:** Self-hosting of the evidence doctrine; the vendor gets minimized reproductions; users get a safe reporting path.

**Trade-offs:** None significant; minimization reuses the §12 machinery.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ §4.4 handle registry @@
 diff_* semantic/intent diff
+defect_* engine-defect report
@@ new §4.7 after §4.6 @@
+### 4.7 Engine-defect lifecycle
+
+Engine defects are first-class evidence. Any `ReplayDiverged`, parity
+mismatch, engine crash, or explanation-validation failure emits a
+`defect_*` artifact: all inputs pinned by content identity, semantic and
+checker epochs, engine identity, and an automatically minimized
+reproduction, governed by the same redaction policy as Context Packs
+(§18.4). `continuum doctor` assembles a shareable defect bundle. Daemon
+health and performance export as OpenTelemetry alongside — never inside
+— semantic evidence.
```

---

### [MEDIUM] Change #14: Give the debugger a state-materialization cost model

**Current State:**
§7.1–7.2 promise reverse causal stepping, jump-to-last-change, branch-on-alternate-event, and branch comparison. Neither the plan, docs/39, nor RFC 0029 says how state is materialized; docs/34 has replay/branch *load* rows (250 ms / 1 s) but no reverse-step or branch-fork row. Reverse stepping over a partial order requires per-event snapshots (memory-infeasible on real heaps) or deterministic re-execution from checkpoints (steps cost O(distance-to-checkpoint); comparing two branches costs two re-executions plus two resident states). N live `dbg_*` branches per swarm make daemon memory unbounded.

**Proposed Change:**
Specify deterministic re-execution from periodic committed checkpoints as the materialization model; budget-tunable checkpoint interval; docs/34 rows for reverse-step and branch-fork; a cap on concurrently materialized branches per session with LRU spill to CAS; branch comparison over projected observer state by default, full heaps only on explicit expansion.

**Rationale:**
This is the plan's own "obvious in demo, collapses on real traces" risk class, shipping in Phase B (G4). The fix is cheap now — it's a stated model plus two latency rows — and expensive later, because the DAP projection and the agent protocol both freeze operation semantics around whatever materialization ships.

**Benefits:** G4's debugger has a defensible cost story before its interfaces freeze; swarm debugging can't exhaust the daemon.

**Trade-offs:** Observer-projected comparison hides heap detail by default — consistent with §13.1's progressive disclosure, with expansion as the escape hatch.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ §7.1, after the state-contents block @@
+State materialization is deterministic re-execution from periodic
+committed checkpoints; the checkpoint interval is budget-tunable.
+Reverse-step and branch-fork carry docs/34 latency rows. Concurrently
+materialized branches per session are capped with LRU spill to the CAS.
+Branch comparison operates on projected observer state unless full
+concrete state is explicitly expanded.
```

---

### [MEDIUM] Change #15: Register the liveness-preserving reduction lane

**Current State:**
§24.5's reduction lane thresholds (≥10×; observer-indexed ≥5×) are safety/class-reduction metrics. RFC 0008:80 forbids reusing the safety DPOR configuration for liveness (correctly — the ignoring problem). Phase D promises an interactive fairness/liveness debugger and corpus liveness evidence; §7.2 promises live "fairness debt and liveness rank." No lane covers fair-cycle-preserving reduction, so the honest default is unreduced liveness checking — state explosion exactly where Phase D promises interactivity — and this assumption is stated nowhere.

**Proposed Change:**
Add a §24.5 row: liveness-preserving reduction — lane research/04 + research/13; threshold: fair-cycle-preserving reduction ≥3× on the liveness corpus subset with zero missed accepting cycles vs the unreduced SCC baseline; fallback: unreduced liveness with an explicit cost banner and batch (non-interactive) expectations stated in the Phase D exit.

**Rationale:**
The register's own rule (:2254–2255) is that no plan section may claim a lane's output without its status — §7.2 and Phase D currently consume an unregistered lane. Registering it either produces the research or makes Phase D's exit honest about batch-mode liveness; both outcomes are fine, but the silent third state isn't.

**Benefits:** Phase D's exit criterion stops resting on an unstated assumption; the fallback is priced in advance.

**Trade-offs:** None; this is bookkeeping the register already mandates for every other HYPOTHESIS.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ §24.5 register table, after the Exploration reduction row @@
+| Liveness-preserving reduction (§7.2, Phase D) | research/04, /13 — lane to be opened | fair-cycle-preserving reduction ≥3× on the liveness corpus subset with zero missed accepting cycles vs unreduced SCC baseline | unreduced liveness with explicit cost banner; batch expectations stated in the Phase D exit |
```

---

### [MEDIUM] Change #16: Contract-drift repair sweep — intent vocabulary, repair docs, evidence-graph docs, and the semantic-diff `unknown` set

**Current State:**
Four residual drift clusters, all plan-vs-companion:
1. **Intent vocabulary.** Plan §5.2 (:653) classifies assumptions as "environment, fairness, timing, failure, trust"; RFC 0037:28 and the schema use `environment, scheduler, timing, storage, network, trust`, with fairness and fault model as separate top-level groups — which the plan's field list omits entirely. Plan nests fidelity/completion/nondeterminism under `assumptions`; RFC/schema make them siblings. Plan §5.2 requires property *ASTs* (:651); the schema stores a bare `expression` string.
2. **Repair docs.** docs/41 predates the 12-gate design: 9-step pipeline, no gate ids, no profiles, no `not_yet_enforced`, receipt list missing the gate-profile field RFC 0032:64 requires. docs/41 lists seven failure modes; rfcs/0032:79 maps only six — "abstraction gaming" (concrete bad states merged) is dropped from the RFC's acceptance mapping, though it is a §5.1 canonical gaming vector. The plan's §8.1 "intent diff" is not a schema field (only `semantic_diff`).
3. **Evidence-graph docs.** docs/44:42 uses a `draft` status outside the §11.4 lattice; its authority table omits `sampled` and `inconclusive`; RFC 0038 is a 19-line stub while the schemas carry the normative content.
4. **Semantic diff.** rfcs/0031:55 MUSTs an `unknown` evidence set ("never to `reused`"); the schema requires only `invalidated` and `reused` — the exact hole the MUST closes. The seven G3 diff dimensions are a free-string `field`, unenforceable by schema.

**Proposed Change:**
(1) Align plan §5.2 to the RFC/schema vocabulary (the shipped surface), adding `fault model` and `fairness` as named groups and adopting the six assumption classes; keep the AST requirement and add a structured-expression field to the schema as a ledger item. (2) Regenerate docs/41 around RFC 0032 (ledgered in Change #8); restore "abstraction gaming" to RFC 0032's failure-mode mapping (it maps to gate 3 via the observer/abstraction dimensions of the intent diff); add `intent_diff` to the repair-transaction schema or document that it travels inside `semantic_diff.intent_changes`. (3) Fix docs/44's `draft` and complete its authority table; expand RFC 0038 or formally demote it to a pointer at the schemas. (4) Require the `unknown` evidence set in the semantic-diff schema; close `intent_changes[].field` to the seven-dimension enum.

**Rationale:**
Each cluster is small, but three of the four sit on the anti-gaming spine: a gaming vector missing from the RFC's acceptance mapping, an optional `unknown` set that lets stale evidence pass as `reused`, and free-string diff dimensions under a gate (G3) that quantifies over exactly those dimensions. The plan's precedence rule (§25: RFC corrected, then normative) gives the direction of each fix.

**Benefits:** G3's "across all seven dimensions" becomes schema-checkable; the RFC 0031 MUST becomes enforceable; the abstraction-gaming vector regains a mapped gate.

**Trade-offs:** Plan §5.2's prose gets slightly longer to name the two extra groups.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ §5.2 structure block @@
 assumptions
-  environment, fairness, timing, failure, trust
+  environment, scheduler, timing, storage, network, trust
   domain-pack fidelity profile per effect family
     (ideal | contractual | platform-qualified | adversarial-envelope)
-  behavior completion policy
-    (stutter-forever | deadlock-violation | finite-trace-only | closed)
-  nondeterminism classes per choice site
-    (demonic | angelic | scheduler | probabilistic† | timed† | epistemic†)
-    († declarative until ADR-0016 lanes ship — see B11)
+fault model
+  crash, recovery, partition, loss/duplication/delay envelopes
+fairness
+  weak/strong fairness declarations per action family
+completion policy
+  (stutter-forever | deadlock-violation | finite-trace-only | closed)
+nondeterminism classes per choice site
+  (demonic | angelic | scheduler | probabilistic† | timed† | epistemic†)
+  († declarative until ADR-0016 lanes ship — see B11)
```
Schema edits (semantic-diff.schema.json): add `"unknown"` to the required evidence sets; replace `intent_changes[].field` free string with the enum `["property","assumption","bound","observer","fault","fairness","assurance"]`. RFC 0032: add `abstraction gaming → gate 3 (intent diff, observer/abstraction dimension)` to the failure-mode mapping. repair-transaction.schema.json: add optional `intent_diff` reference or a normative comment that intent changes travel in `semantic_diff.intent_changes`.

---

### [MEDIUM] Change #17: Multi-repo systems — federation manifest, or an explicit 1.0 non-goal

**Current State:**
§4.2 defines the workspace snapshot as one source tree; §16's correspondence graph binds one program to one model; Phase F's exit (:1988–1992) requires migrating "two materially different real systems." Real distributed systems span repositories and deploy with version skew; an intent binding a client in repo A to a server in repo B, cross-repo refinement, and "does the intent hold for A@v3 with B@v2?" have no representation. Every Phase F migration will improvise this per-project — the "engine-specific surgery" kill criterion (:2219) in different clothes.

**Proposed Change:**
Either add a *federation manifest* (`fed_*`): a content-addressed set of workspace snapshots bound to one shared Intent Contract, with per-participant correspondence and an explicit version-skew envelope (which participant-version pairs the intent claims cover), with cross-participant evidence naming the manifest; **or** add "single-workspace scope; multi-repo federation" to an explicit 1.0 non-goals list, so Phase F recruits single-repo systems and the exit is honest.

**Rationale:**
The plan currently promises distributed-system migrations with a single-repo data model and no acknowledgment of the gap. Both resolutions are respectable; the unstated middle is not — it converts Phase F from a gate into a research project at the worst possible time.

**Benefits:** Phase F's recruiting criteria become writable today; version-skew claims (the intent dimension operators actually care about) get a home if option A is taken.

**Trade-offs:** Option A adds a schema and evidence-scoping rules; option B narrows the 1.0 claim. Recommend option B now with option A as a registered post-1.0 design note — skew envelopes are close to research.

**Git-Diff (option B):**
```diff
--- plan.md
+++ plan.md
@@ §21 Phase F, Exit paragraph @@
 Exit: Continuum replaces bespoke DST plus separate TLA+ workflow in at
 least two materially different real systems.
+Scope note: 1.0 verifies systems whose controlled participants live in
+one workspace snapshot. Cross-repository federation — one intent
+governing independently deployed participants with a version-skew
+envelope — is an explicit 1.0 non-goal, recorded here so Phase F
+migrations are recruited within scope. A design note registers the
+federation-manifest (`fed_*`) direction for post-1.0.
```

---

### [LOW] Change #18: Compatibility windows and boundary hardening — protocol N−1, asupersync window, domain-pack pinning, capture-time privacy

**Current State:**
Four adjacent policy gaps: (a) RFC 0026 keeps handles valid "within a major version" and §4.6 promises receipts verify "indefinitely," but no protocol deprecation window or standalone artifact-reader guarantee exists — a 2027 receipt is as verifiable as a 2030 daemon's willingness to parse it. (b) Phase B's asupersync "exact version pin" means one supported upstream version at a time; a mid-phase upstream breaking release strands users or roadmap. (c) §18.1 names "malicious domain packs" as a threat; the only specified mitigation class (sandboxing, §18.3) is irrelevant to the actual attack — a pack that silently narrows the fault envelope attacks semantics, not the host. (d) §18.4 enforces privacy at context *compilation*, but Phase F production payloads enter the immutable, replicated CAS *before* any policy runs; purge-by-key-shredding requires knowing what to purge.

**Proposed Change:**
(a) §4.3: daemon serves protocol majors N and N−1; evidence/receipt schemas are readable by every future verifier for their declared schema epoch — artifact readability decoupled from protocol majors, covered by the kernel crates' reproducible-build covenant. (b) Phase B: adapter declares a supported upstream window (N, N−1), CI runs the conformance corpus against upstream pre-releases, and an upstream break opens a scheduled adapter epoch (§4.6), user-visible in envelopes. (c) §18: packs are content-addressed, signed, and pinned in the snapshot; the intent's fidelity profile binds the exact pack identity so pack substitution is a privileged intent diff; third-party packs default to `adversarial-envelope` fidelity until docs/17 qualification; receipts render pack provenance distinctly. (d) §18.4: capture-time contract — payloads recorded as salted commitments by default; raw capture is per-field opt-in with a data classification at ingestion, per-artifact encryption on entry (enabling §4.5 purge), a declared retention clock; remote workers receive only post-redaction minimized artifacts, residency as a worker-pool capability property.

**Rationale:**
All four are one-paragraph policies now and migration projects later. (c) is the sharpest: pack substitution is the product's supply-chain vector, and binding pack identity into the intent's fidelity profile turns an attack into a G3-classified privileged change — the plan's own best mechanism, applied to its own extension surface. (d) is the difference between a privacy feature and being deployable at the two Phase F reference customers.

**Benefits:** Receipts stay verifiable across daemon generations; upstream breaks become scheduled epochs; pack substitution trips INV-001; production capture becomes legally deployable.

**Trade-offs:** The N−1 window and dual-version adapter CI carry real maintenance cost — the price of "indefinitely."

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ §4.3, end of requirements list @@
 - protocol and semantic version separation.
+
+The daemon serves protocol majors N and N−1. Evidence and receipt
+schemas are readable by every future verifier for their declared schema
+epoch: artifact readability is decoupled from protocol majors and
+covered by the kernel crates' reproducible-build covenant (§20).
@@ §21 Phase B, asupersync deliverable @@
-- asupersync semantic adapter (one adapter crate, exact version pin,
+- asupersync semantic adapter (one adapter crate, supported upstream
+  window of N and N−1 with the conformance corpus run against upstream
+  pre-releases in CI; an upstream breaking release opens a scheduled
+  adapter epoch (§4.6) visible in envelopes,
   explicit semantic-hook contract, adapter conformance corpus; the model
   core never depends on asupersync);
@@ §18.3, after the sandboxing paragraph @@
+Domain packs are content-addressed, signed, and pinned in the workspace
+snapshot; the Intent Contract's fidelity profile binds the exact pack
+identity, so pack substitution is a privileged intent diff (G3), not an
+environment change. Third-party packs default to `adversarial-envelope`
+fidelity until they pass docs/17 conformance qualification, and receipts
+render pack provenance (first-party | qualified | unqualified)
+distinctly.
@@ §18.4, end of section @@
+Capture-time contract for production traces: payloads are recorded as
+salted commitments by default; raw payload capture is per-field opt-in,
+tagged with a data classification at ingestion, encrypted per artifact
+on entry (enabling §4.5 purge), and subject to a declared retention
+clock. Remote workers receive only post-redaction minimized artifacts;
+residency constraints are a capability property of the worker pool.
```

---

### [LOW] Change #19: Mechanical residue

**Current State / Proposed Change (one line each):**
1. `schemas/crashpack.schema.json:24` defines the crashpack's own id as `^crash:[…]` (colon form) while every cross-reference uses `crash_` handles — unify on `^crash_`.
2. `evidence-graph-edge.schema.json:12` uses `^ev_` for *edge* ids while §4.4 registers `ev_*` as "evidence node" — either register `ev_*` as "evidence node or edge" in §4.4 or give edges `eve_*`.
3. Seven registered prefixes (`model_`, `cir_`, `ps_`, `dbg_`, `cap_`, `proof_`, `forge_`) have no `^prefix_` pattern anchor in any schema — add pattern constraints where those handles appear so the registry is machine-enforced.
4. docs/03:125 frames the kernel covenant as "suggested initial"; plan/START_HERE treat it as binding — update docs/03 to match, or the covenant has no normative home.
5. RFC 0026's normative IDL file does not exist ("the IDL is normative once it exists") — tracked in Change #8's ledger; also add a validator check that referenced normative files exist.
6. docs/44's `draft` status and authority-table gaps — covered by Change #16(3), listed here for the ledger.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ §4.4 handle registry @@
-ev_* evidence node
+ev_* evidence node or edge
```
Remaining items are companion-file edits enumerated above; each belongs on the Change #8 ledger so the validator tracks them to closure.

---

## Closing assessment

The dossier is in markedly better shape than at either prior pass: the gate scheme, execution layer, corpus contract, and lane register now largely agree with each other, and the repair-transaction contract — plan, RFC, and schema in lockstep, with `not_yet_enforced` structurally enforced — is the model the rest of the contract surface should converge to. The four critical items this pass are of a different kind than before: not internal inconsistency, but load-bearing absences — a distribution model for the plan's central integrity object (#2), a de-risk decision for its largest engineering artifact (#3), an audit definition that survives nondeterministic engines (#4), and a verification mechanism for the reconciliation claim itself (#1). All four are cheapest now, before PR 5 freezes the interfaces they touch.
