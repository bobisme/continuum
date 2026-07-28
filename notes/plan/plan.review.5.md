# Review 5 — `notes/plan/plan.md` (Revision 3 Master Implementation Plan)

**Review date:** 2026-07-28
**Scope:** `plan.md` plus the accompanying dossier (docs/, rfcs/, adr/, schemas/, research/, notes/, spikes/, tools/), audited against the plan's own claims.
**Method:** four independent audit passes — (1) a dedup catalog of reviews 1–4 so nothing below repeats an already-raised issue; (2) plan §21/§22/§24.5/§25 vs the execution layer (START_HERE, G0 matrix, docs/52, docs/34, docs/04, `tools/validate_dossier.py`); (3) plan §4/§5/§8–§11 vs the seven load-bearing RFCs and all 20 schemas; (4) plan §19/§24/§24.5/§0.3 vs docs/08, docs/31, docs/45, docs/50, docs/53 and the 17 cited research notes. Every finding below carries file:line citations verified in this pass.

---

## Executive Summary

The plan is in the best shape it has ever been. Reviews 1–4 landed: the gate scheme is unified and bidirectionally validated (11 gates, 65 bullets each way), the G0 counts are genuinely derived from the matrix, the corpus arithmetic in Phase C is exactly right (29 families, 14 P4, 3 P3 verified against `validated-examples.csv`), the G10 two-system criterion is consistent in all three places, the intent lifecycle and bundle model are coherent, and several previously-open schema debts (intent-registry record, promotion-receipt cost ledger, nine-dimension assurance envelope, typed `Failed` reasons, fail-closed diff conditional) are actually paid. Appendix B lists everything this pass verified clean, so the next reviewer can skip it.

The remaining problems are of a different kind than in earlier passes. They cluster into five systemic failures:

1. **The honesty ledgers have gone stale again — and this time the staleness proves the process is broken, not the text.** §25's "derived-doc contradictions" paragraph asserts that five documents "each currently state the opposite of the plan"; all five already state exactly what the plan wants (`docs/42:73`, `docs/42:13`, `docs/40:125`, `docs/41:3`, `docs/35:74`, `docs/45:122`). Four §25 schema/RFC debt items are already paid. §22 lists docs/04 bannering as pending; the banner is at `docs/04:3`. This is the third consecutive review (after R3#8's "inverted staleness" and R4#1/#6/#10's "fictional reconciliations") to find the hand-maintained ledger asserting falsehoods about the dossier. The fix is structural: the debt ledger must be validator-derived, like the G0 counts already are.

2. **The anti-gaming machine contracts fail open at exactly the points they exist to close.** `semantic-diff.schema.json` guards its fail-closed rule behind a producer-asserted `protected: true` boolean that nothing binds to the protected field set — a producer emitting `protected: false` on a weakened property passes validation with `decision: "allow"`. `repair-transaction.schema.json` admits a `not_applicable` gate status that RFC 0032 never defines, and its promoted-transaction conditional does not forbid it — a transaction can promote with all twelve gates `not_applicable`. Gate lists enforce `minItems: 12` but not identity, so twelve copies of `base_replay` validate. The evidence-graph schemas enforce neither of the only two anti-forgery rules RFC 0038 states. These are one-boolean-away failures of B1, B12, and INV-004.

3. **The frontier-lane register does not satisfy its own ratification rule.** §24.5 claims the validator "checks register↔note quote identity"; no such check exists (`tools/validate_dossier.py:580-608` checks substring presence and reference resolution only). The one row treated as ratified with a quote misquotes its source ("≥10×" vs research/01's "order-of-magnitude" — and the paraphrase has already laundered back into `research/26:115`). Another row inserts "fair-cycle-" into a soundness gate that research/04 states without qualification. One row's threshold denominator ("the docs/50 gaming corpus") does not exist anywhere in the dossier. One row cannot be ratified until Phase C by the plan's own construction, while §24.5 forbids unratified rows past Phase A. And the plan's single most load-bearing hypothesis — that the typed ACI beats disciplined shell use, which gates the Phase A protocol freeze — has no register row at all.

4. **The kill machinery has holes precisely at the highest-rated risks.** docs/08's R03 (circular trust between asupersync, CIR generation, and the checker — high probability, existential for assurance) has no kill criterion, no register row, and no detection mechanism anywhere in the plan. R21's kill signal ("Conservative-collapse on the reference workload") is dropped, and §9.1 miscites R21's failure mode as its mitigation. R01's adoption kill signal and R11's liveness-intractability risk are likewise unrepresented. Meanwhile every §24 bullet is qualitative ("pervasive", "frequently", "reliably", "mostly") with no owner, instrument, or threshold — and §24's intent-diff bar ("reliably") disagrees with G3's bar (100% of supported-fragment mutations).

5. **The execution layer cannot be executed as sequenced.** All six phase owner rows read `unassigned / unassigned / BLOCKED` (`START_HERE:51-56`), which by the plan's own rule (`plan.md:2012-2014`) makes every phase — including "immediately executable" Phase A — formally BLOCKED, while `VALIDATION_REPORT.md:5` says PASS. Ratification requires lane owners; lane owners come from the empty table; filling the table is a merge requirement of phase-opening PRs that are never designated. The PR sequence inverts the phase order it claims to implement (PR 30, the last PR, advances G2 — a Phase A gate). Four of G0's eleven "runs against Phase A machinery" freeze-blocking items actually require Phase B–E machinery. And three Phase A freeze decisions rest on an agent benchmark whose validity check (DX-15) is deferred to Phase F.

None of these kill the architecture. All of them are the difference between a plan that *describes* honesty machinery and a dossier that *is* honesty machinery. The changes below are ordered by how directly they close that gap.

---

## Proposed Changes

### [CRITICAL] Change #1: Close the fail-open holes in the anti-gaming schemas

**Current State:**
The plan's reward-hacking story rests on three machine contracts, and each fails open:

- `schemas/semantic-diff.schema.json:5-49` — the fail-closed conditional (block on `unknown`/`unsupported`/`incomparable`) fires only when an item carries `protected: true` **and** a non-affirmative relation. `protected` is producer-asserted; nothing binds it to RFC 0037's fifteen-field protected set. `{field: "properties", relation: "unknown", protected: false}` validates with `decision: "allow"`. Since every member of the `field` enum (:120-136) *is* in the protected set, the boolean is pure attack surface.
- `schemas/repair-transaction.schema.json:260-268` — the gate-status enum includes `not_applicable`, which appears nowhere in RFC 0032; the promoted-transaction conditional (:23-69) forbids only `pending/fail/inconclusive`, so a transaction can promote with all twelve gates `not_applicable`. RFC 0032's rejected-alternatives section explicitly rejected exactly this ("a Phase B receipt must be structurally distinguishable from a Phase D receipt").
- Both `repair-transaction.schema.json:246` and `promotion-receipt.schema.json:106` enforce `minItems: 12` with no `uniqueItems` and no per-gate presence check — twelve copies of `base_replay` validate, so RFC 0032's "MUST list all twelve gates (schema-enforced)" is not, in fact, schema-enforced.
- Phase-profile conditionals exist for `phase-b` and `default` only (:70-140); `rfcs/0032:48` says "`phase-c` adds 10; `phase-d`/`default` enforce all twelve", so a `phase-d` transaction may legally mark all twelve gates `not_yet_enforced`.
- `schemas/evidence-graph-node.schema.json` — RFC 0038's only two stated anti-forgery rules are unenforced: no conditional requires a service identity on `Sampled`/`Bounded`/`Proved` promotion (`rfcs/0038:15`), and `provenance.actor` (:124) is a free string equally writable as `agent:x`. `evidence-graph-edge.schema.json:189-193` leaves `checker` optional even on `CHECKED_BY` edges — a `CHECKED_BY` edge naming no checker is precisely the self-certification INV-004 forbids.

**Proposed Change:**
Fix all five schemas (exact edits in Appendix A, items A1–A5) and add a named freeze-blocking item to §25's debt ledger so the closure is checkable. The `protected` field becomes `const: true` (or is removed and derived from `field` membership); `not_applicable` is deleted or confined to non-promoted profiles with an RFC 0032 definition; gate lists gain `uniqueItems` plus a `contains` clause per gate name; `phase-c`/`phase-d` conditionals are added; status-promotion and `CHECKED_BY` conditionals are added to the evidence-graph schemas.

**Rationale:**
B1 says "a tool that proves a weakened property has failed." The current schemas let a producer *assert its way out of* the fail-closed rule. Every other line of anti-gaming machinery (G3's seven dimensions, the hidden mutation suite, intent locks) sits downstream of these contracts; if the wire format validates a gamed artifact, the gates are theater. This is the single highest-leverage fix in the dossier.

**Benefits:**
- Closes the only known one-step bypass of INV-001/INV-011 at the schema layer.
- Makes RFC 0032's "structurally distinguishable" promise true.
- Restores INV-004 enforcement to the evidence graph's wire format.

**Trade-offs:**
- `const: true` on `protected` makes the field redundant; removing it is a breaking schema change. Either is acceptable pre-freeze; after PR 5 it is not — hence freeze-blocking.

**Implementation Notes:**
The per-gate `contains` pattern is verbose in JSON Schema draft 2020-12 (twelve `contains` clauses) but mechanical. Validate all `schemas/examples/` pairs after the change; the `not_applicable` removal may invalidate an existing example.

**Git-Diff:**
```diff
--- a/notes/plan/plan.md
+++ b/notes/plan/plan.md
@@ §25 specification-debt list @@
 - RFC 0038: the evidence-graph write/concurrency model (§11.7);
+- schemas, fail-closed repair (freeze-blocking): `semantic-diff` binds
+  `protected` structurally to the RFC 0037 protected set (no
+  producer-asserted boolean); `repair-transaction` removes the
+  undefined `not_applicable` gate status from promotable profiles and
+  adds the missing `phase-c`/`phase-d` conditionals; gate lists in
+  `repair-transaction` and `promotion-receipt` enforce the twelve gate
+  identities, not a count; `evidence-graph-node` requires a service
+  identity on `Sampled`/`Bounded`/`Validated`/`Proved` promotion and
+  `evidence-graph-edge` requires `checker` on `CHECKED_BY` (INV-004);
```

---

### [CRITICAL] Change #2: Regenerate §25's debt ledger from the validator — and correct its five false claims now

**Current State:**
§25's ledger asserts things that are false about the current dossier:

- `plan.md:2565-2572` claims five derived-doc contradictions "each currently states the opposite of the plan or its normative RFC." All five are already fixed: `docs/42:73` ("It is on by default, not opt-in"), `docs/42:13` (`Elaborate(module, imports, semantic_epoch)`), `docs/40:38,125` (fail-closed rule present, `incomparable` bound), `docs/41:3` (RFC 0032 banner present), `docs/35:74` (cross-user sharing off by default), `docs/45:122` (security second in grading order).
- `plan.md:2538-2540` lists RFC 0037's `intent.accept`/`intent.lock`/`revise-intent` and the knowledge/security projections as debt; `rfcs/0037:74` and `rfcs/0037:29` already carry both, verbatim.
- Of the schema items at `plan.md:2542-2553`, four are paid: `intent-registry-record.schema.json` exists with status and acceptance chains; `promotion-receipt.schema.json` carries `cost_ledger` and `gate_profile`; `assurance-result.schema.json` requires all nine dimensions and `resource-exhausted` is gone from every schema; `verification-task.schema.json` has typed `Failed` reasons.
- Two items are understated: the "shared `Redacted` `$defs` used by every artifact class" is actually *copy-pasted* into exactly three schemas and absent from `promotion-receipt`, `repair-transaction`, `assurance-result`, `evidence-graph-node`, `verification-task`, and `cir` — even though `plan.md:657-660` mandates receipts report `Redacted(lost, commitment)`; and no schema anywhere carries a schema-epoch field despite `plan.md:2552`.
- `plan.md:2213-2215` (§22) lists docs/04 bannering as pending §25 work; the banner is at `docs/04:3`.

An implementer following §25 today would "fix" six documents that are already correct and skip real debt.

**Proposed Change:**
1. Immediately: delete the paid items and the false contradictions paragraph; replace with the genuinely-open remainder (IDL, RFC 0026 §4.3 absorption, RFC 0032 §8.6 absorption, RFC 0038 §11.7, RFC 0037 §4.2.1, property-AST, schema-epoch field, shared-`$ref` Redacted, docs/35+ADR-0018 operational absorption, docs/41 regeneration) plus the new debt found in this review (Changes #1, #4, #11, #15, #16).
2. Structurally: make the ledger validator-derived. Each debt item becomes a machine-checkable predicate (file exists / string present / schema `$ref` resolves), the validator emits the open-debt list into `validation-results.json`, and §25 states the *rule* ("debt is whatever the validator reports open") plus a snapshot table marked as generated — exactly the discipline §0.3 already applies to G0 counts ("derived from the matrix, not asserted beside it").

**Rationale:**
This is the third consecutive review to catch the hand-maintained ledger asserting falsehoods (R3#8, R4#1/#6/#10, now this). A ledger that goes stale within one editing cycle in a dossier *about evidence discipline* is a credibility wound: it teaches readers to distrust exactly the sections that exist to be trusted. The G0 count derivation proved the fix works; apply it to the other ledger.

**Benefits:**
- Stops the stale-ledger recurrence class permanently instead of patching instances.
- Prevents wasted implementation effort on already-paid items.
- Makes "debt paid before PR 5" a checkable gate instead of prose.

**Trade-offs:**
- Some debt items resist mechanical predicates (e.g. "absorb §8.6's semantics"). For those, the predicate is a named marker in the target file (`<!-- absorbs plan §8.6 -->`) plus human review — weaker, but still better than free prose.

**Implementation Notes:**
Regenerate `VALIDATION_REPORT.md` and `validation-results.json` in the same commit so every advertised check has a recorded result (the R4#10 rule).

**Git-Diff:**
```diff
--- a/notes/plan/plan.md
+++ b/notes/plan/plan.md
@@ §25 @@
-- RFC 0037: the `intent.accept` / `intent.lock` operations and
-  `revise-intent` capability (currently only in RFC 0027), and the
-  knowledge/security observer projections dropped from §5.2's four;
 - RFC 0038: the evidence-graph write/concurrency model (§11.7);
-- schemas: an intent-registry record schema carrying `Proposed`/accepted
-  status and §4.2.1 acceptance chains; a shared `Redacted(reason,
-  commitment)` `$defs` used by every artifact class (§4.5, §18.4); a
-  promotion-receipt schema carrying the §8.6 cost ledger and gate
-  profile; a B11-complete assurance envelope (all nine dimensions, each
-  naming its producing engine or a typed `Unsupported(reason)`), with the
-  `resource-exhausted` verdict removed (§11.4: budget exhaustion is never
-  a verdict); conditional enforcement for INV-008 reasons,
-  `validation_basis`, and fail-closed diffs; typed `Failed` reasons on
-  tasks (§4.5); one `$id`/versioning convention with a schema-epoch field
-  (§4.3);
+- schemas: convert the three inlined `Redacted` copies to a shared
+  `$ref` of `redacted.schema.json` and extend it to the artifact
+  classes that lack it entirely (`promotion-receipt`,
+  `repair-transaction`, `assurance-result`, `evidence-graph-node`,
+  `verification-task`, `cir`) — §4.5 requires receipts to report
+  `Redacted(lost, commitment)`; add the schema-epoch field to the one
+  `$id`/versioning convention (currently three rival conventions and
+  zero schema-epoch fields);
@@ §25 @@
-- derived-doc contradictions corrected in the same pass (each currently
-  states the opposite of the plan or its normative RFC): docs/42's
-  opt-in parity audit (§9.5 and RFC 0030 make sampling default-on) and
-  its `Elaborate` query key missing `semantic_epoch` (§9.2); docs/40's
-  missing fail-closed rule and unbound `incomparable` (§5.3); docs/41's
-  missing RFC 0032 banner (added immediately, ahead of regeneration);
-  docs/35's cross-user sharing default (§4.5); docs/45's grading order
-  (security is second per §19.3/RFC 0034).
+This ledger is validator-derived: each item corresponds to a named
+check in `tools/validate_dossier.py`, the open set is emitted into
+`validation-results.json`, and this section's table is regenerated —
+never hand-edited — exactly as §0.3's G0 counts are derived from the
+matrix. A debt item without a corresponding validator check is itself
+a defect.
```

---

### [CRITICAL] Change #3: Make the §24.5 register satisfy its own ratification rule

**Current State:**
`plan.md:2462-2464` defines ratification: the owner fixes the numeric threshold *in the research note* and the register *quotes it verbatim*; "the dossier validator checks register↔note quote identity." Audit findings:

- **The validator check does not exist.** `tools/validate_dossier.py:580-608` checks only that threshold cells contain one of `kill|defer|draft|fallback` and that `research/NN`/`ADR-NNNN` references resolve. `VALIDATION_REPORT.md:33` describes the real (weaker) check, so the plan overclaims its own tooling.
- **The flagship "ratified" row misquotes its source.** `plan.md:2470` quotes research/01 as "≥10× reduction in explored classes on a non-artificial corpus subset"; `research/01:101` says "at least an order-of-magnitude reduction on a non-artificial subset" — no "≥10×", no "explored classes", no "corpus". Worse, `research/26:115-116` now cites "research/01: ≥10× reduction in explored classes" — a note citing a number that exists only in the plan. That is a laundering chain with no primary source.
- **A soundness gate is silently narrowed.** `plan.md:2471` renders research/04's "property-directed POR is proven **preserving** or disabled" (`research/04:124`) as "proven **fair-cycle-preserving** or disabled" — dropping safety-trace preservation from a liveness-reduction soundness gate.
- **The certificate-overhead row drops scope and substitutes a different fallback.** `docs/31:58` scopes ≤10% "for large finite proofs"; the register drops the qualifier while claiming to state "docs/31's full rule". docs/31's prescribed adjustment (verified native checker as routine path, Lean validates sampled artifacts) is replaced by "reduce certified-lane scope" — a materially different remedy with no named approver.
- **A threshold denominator does not exist.** `plan.md:2473` and `research/33:9` key neighborhood adequacy to "the docs/50 gaming corpus"; `docs/50` contains attack *classes* and defenses, no corpus, no size, no construction procedure. The threshold is unmeasurable in principle.
- **Three rows point instead of quote** (`plan.md:2481-2483`: "docs/31's stated promote/kill pair") and are not marked draft — silently exempt from the rule.
- **Four cited research notes have no promotion/kill criteria at all** — research/03, research/04 (for this row's subject), research/12 (the certificate row cites only docs/31 because no owning note has criteria), research/14 — violating `research/README.md:3`.
- **The Forge fallback contradicts its source.** `plan.md:2475` falls back to "Pareto archive only"; `research/30:81` names Pareto search as the *baseline QD must beat* and prescribes keeping a lightweight semantic-diversity archive.
- **The observer-indexed kill half is dropped.** `plan.md:2470` carries only the promote half; `docs/31:10` and `research/13:99-101` state kills (witness cost dominates; cache invalidation too frequent; view-equality diamonds insufficient) that appear nowhere in the register.

**Proposed Change:**
One repair pass over the register, with the rule "the note is corrected and cited" applied in both directions:
1. Fix the three misquotes (restore "order-of-magnitude … non-artificial subset" wording or ratify "≥10× in explored classes" *in research/01 first* and then quote it; restore unqualified "preserving"; restore "for large finite proofs" and docs/31's actual adjustment).
2. Purge the laundering: correct `research/26:115` to cite research/01's actual wording.
3. Replace "docs/50 gaming corpus" with a named deliverable ("the docs/50-classified gaming corpus, constructed as a G3 Phase B deliverable, ≥N mutations per attack class") and add corpus construction to Phase B.
4. Mark the three pointer rows draft, or inline the quoted pair.
5. Add kill halves where dropped (observer-indexed row).
6. Fix the Forge fallback to research/30's prescription ("staged synthesis; semantic-diversity archive").
7. Open criteria sections in research/03, /04, /12, /14 (Appendix A, item A9).
8. Implement the quote-identity validator check the plan already advertises — or delete the advertisement.

**Rationale:**
The register was created (R1#8), repaired (R2#6), extended (R3#15), and audited (R4#3) — and it *still* fails its own rule, because the rule is enforced by prose. A register whose flagship ratified row misquotes its source, and whose misquote is already propagating into research notes, is worse than no register: it manufactures the appearance of quantitative discipline. Either the validator enforces quote identity or the register's authority claim must be withdrawn.

**Benefits:**
- Stops threshold laundering before more notes cite the plan citing the notes.
- Restores the safety half of the liveness soundness gate.
- Makes the neighborhood-adequacy lane measurable at all.

**Trade-offs:**
- Exact quote-identity checking is brittle against harmless rewording; a normalized-substring check with an explicit `quote:` marker in both files is the practical middle ground.

**Implementation Notes:**
The `quote:` marker convention (register cell and research note both carry `> quote-id: <lane>-threshold` fenced text) makes the validator check trivial and the diff reviewable.

**Git-Diff:**
```diff
--- a/notes/plan/plan.md
+++ b/notes/plan/plan.md
@@ §24.5 @@
-| Exploration reduction (§9, INV-013) | research/01; docs/31 | research/01 (stated there as a kill): ≥10× reduction in explored classes on a non-artificial corpus subset without a *serious* regression on dependent workloads; observer-indexed (docs/31): median ≥5× on the observer-sensitive class, *checker* overhead <20% (distinct from the certificate-overhead row), zero mutation loss | conservative unreduced exploration |
+| Exploration reduction (§9, INV-013) | research/01; docs/31 | research/01 (stated there as a kill): at least an order-of-magnitude reduction on a non-artificial subset without a serious regression on dependent workloads (the "explored classes" denominator is a proposed ratification, to be fixed in research/01 before this row leaves draft); observer-indexed (docs/31): median ≥5× on the observer-sensitive class, *checker* overhead <20%, zero mutation loss; kill (docs/31, research/13): witness cost dominates, or observer/property changes invalidate cached independence so often that reuse never pays for its witness cost — draft | conservative unreduced exploration |
@@ §24.5 @@
-| Liveness-preserving reduction (§7.2, Phase D) | research/04, research/13 — lane to be opened | soundness gate per research/04: property-directed reduction is proven fair-cycle-preserving or disabled; speedup target and liveness corpus subset fixed at lane opening — draft | unreduced liveness with explicit cost banner; batch expectations stated in the Phase D exit |
+| Liveness-preserving reduction (§7.2, Phase D) | research/04, research/13 — lane to be opened | soundness gate per research/04: property-directed reduction is proven preserving (for the full property class it is applied to, fair cycles included) or disabled; speedup target and liveness corpus subset fixed at lane opening — draft | unreduced liveness with explicit cost banner; batch expectations stated in the Phase D exit |
@@ §24.5 @@
-| Neighborhood adequacy (§8.3) | research/33 — lane to be opened | hidden-variant catch rate of the §8.3 neighborhood on the docs/50 gaming corpus; target fixed at lane opening — draft; kill (research/33): hidden variants are too easy to leak or too hard to grade independently | fixed strategy-list neighborhood with per-receipt coverage disclosure and no adequacy claim |
+| Neighborhood adequacy (§8.3) | research/33 — lane to be opened | hidden-variant catch rate of the §8.3 neighborhood on the docs/50-classified gaming corpus (the corpus itself is a Phase B G3 deliverable: ≥N mutations per docs/50 attack class, N fixed at lane opening — no such corpus exists today); target fixed at lane opening — draft; kill (research/33): hidden variants are too easy to leak or too hard to grade independently | fixed strategy-list neighborhood with per-receipt coverage disclosure and no adequacy claim |
@@ §24.5 @@
-| Forge co-synthesis + QD (§14) | research/29, research/30 | rediscovery suite (named algorithms and count fixed at ratification — draft); kills (research/29): joint space overwhelms coupled-feedback gains; proof-complexity objective biases toward trivial designs; abstractions overfit finite bounds; agent proposals cannot be reproduced by structured search | staged synthesis; Pareto archive only |
+| Forge co-synthesis + QD (§14) | research/29, research/30 | rediscovery suite (named algorithms and count fixed at ratification — draft); kills (research/29): joint space overwhelms coupled-feedback gains; proof-complexity objective biases toward trivial designs; abstractions overfit finite bounds; agent proposals dominate and cannot be reproduced by structured search; kill (research/30): QD discovers nothing beyond multi-objective Pareto search on known benchmarks | staged synthesis; lightweight semantic-diversity archive (per research/30) |
@@ §24.5 @@
-| Certificate overhead | docs/31 | checking ≤10% of search time, or acceptable asynchronous CI latency (docs/31's full rule); on failure the fallback is applied per lane | reduce certified-lane scope |
+| Certificate overhead | research/12; docs/31 | checking ≤10% of search time for large finite proofs, or acceptable asynchronous CI latency (docs/31); research/12 must gain a criteria section before this row leaves draft — draft | per docs/31: verified native checker remains the routine path; Lean validates the checker and sampled/full release artifacts |
@@ §24.5 rule text @@
-A lane is **ratified** when its owner fixes the numeric threshold in the
-research note and this register quotes it verbatim; the dossier validator
-checks register↔note quote identity. Until then a row is **draft** and
-blocks its lane's promotion.
+A lane is **ratified** when its owner fixes the numeric threshold in the
+research note and this register quotes it verbatim; both files carry a
+`quote-id` marker and the dossier validator compares the marked quotes
+for identity (this check ships with the same commit that introduces the
+markers — as of this revision it does not yet exist). Until then a row
+is **draft** and blocks its lane's promotion. A row that delegates to a
+document ("docs/31's stated pair") without quoting is draft by
+definition.
```

---

### [CRITICAL] Change #4: Resolve the RFC 0030 vs §9.5 contradiction — the RFC currently mandates what the plan forbids

**Current State:**
`plan.md:1134-1153` (§9.5) defines three auditability classes and rules: "Divergence attributable solely to budget or portfolio nondeterminism never quarantines a reuse class; divergence in an equality- or certificate-auditable query always does." `rfcs/0030-incremental-semantic-query-engine.md:44` says flatly: "On mismatch: … quarantine the implicated edge class + query implementation version" — no class distinction, no budget carve-out; RFC 0030 contains no occurrence of "auditability". Separately, `rfcs/0030:42` fixes the audit sampling rate at "default 1 in 64, uniformly by key hash", while `plan.md:1057-1060` (§8.6) requires the rate to "derive from a declared statistical confidence target for mismatch detection per reuse class, reviewed at G5."

Per the plan's own doctrine (`plan.md:2515-2516`: "Where plan prose and RFC disagree, the RFC is corrected and becomes normative"), the normative document currently mandates quarantining solver-portfolio divergence — which would quarantine every SMT-backed reuse class within days of real use — and pins a sampling constant the plan says must be derived. Neither conflict is in §25's debt list.

**Proposed Change:**
Add both to §25's ledger as pre-PR-5 debt: RFC 0030 absorbs the §9.5 auditability-class taxonomy (declared on the query definition, not the run), the class-scoped quarantine rule, and the §8.6-derived sampling rate (1-in-64 may remain as the bootstrap default, labeled as such).

**Rationale:**
This is the exact failure mode R3#4 predicted when it introduced auditability classes: the plan was fixed, the normative RFC wasn't, and the plan's own supremacy rule now points the wrong way. Left as is, an implementer following the RFC builds the quarantine behavior that destroys G5.

**Benefits:**
- Restores the plan→RFC fix propagation for the single most operationally sensitive incremental rule.
- Prevents false quarantines from burying real Exact-class mismatches (the alarm-fatigue path to ignoring the parity audit entirely).

**Trade-offs:**
None; this is pure reconciliation.

**Implementation Notes:**
While editing RFC 0030, also declare where the auditability class lives structurally (on the query definition — `plan.md:1151-1153`) so the RFC captures the no-post-hoc-reclassification rule.

**Git-Diff:**
```diff
--- a/notes/plan/plan.md
+++ b/notes/plan/plan.md
@@ §25 specification-debt list @@
 - RFC 0032: absorb §8.6 — the incremental gate 5–7 reuse rule, cost
   ceilings, and `BudgetExhausted`-with-continuation;
+- RFC 0030: absorb §9.5's auditability classes and class-scoped
+  quarantine rule — the RFC currently mandates unconditional quarantine
+  on any mismatch, which §9.5 forbids for budget/portfolio divergence —
+  and replace the fixed 1-in-64 sampling constant with the §8.6-derived
+  per-class confidence-target rate (1-in-64 may remain the labeled
+  bootstrap default);
```

---

### [CRITICAL] Change #5: Record the program's true BLOCKED status and break the resourcing↔ratification circularity

**Current State:**
`plan.md:2012-2014`: "a phase without both [owner and minimum viable team] is `BLOCKED`, not in progress." All six rows of the owner/team table read `unassigned / unassigned / BLOCKED` (`START_HERE:51-56`). So by the plan's own rule the entire program — including Phase A, which §25 titles "Immediate execution" — is formally BLOCKED. Meanwhile `VALIDATION_REPORT.md:5` reads "PASS", `validation-results.json` reads `"status": "pass"`, and nothing in the dossier surfaces the blocked state. The circularity compounds it: §24.5 rows are ratified by lane owners (`plan.md:2462`); lane owners come from the phase table; filling a row is "a merge requirement of that phase's opening PR" (`plan.md:2015-2016`), but no PR in START_HERE is designated as any phase's opening PR — so the merge requirement is unattachable, ratification is blocked on resourcing, and Phase A cannot exit until ratification completes (`plan.md:2487-2488`).

**Proposed Change:**
1. Add the program-status line to §0.3 (the claim-status section, which exists for exactly this): "Program status: all six phases are `BLOCKED` on the §21.1 owner/team rule as of this revision; Phase A unblocks when its row in `notes/START_HERE_IMPLEMENTATION.md` is filled."
2. Designate the phase-opening PRs in START_HERE (Phase A → PR 0; Phase B → the asupersync adapter PR; etc.) so the merge requirement attaches to something.
3. Break the circularity for Phase A explicitly: the Phase A owner ratifies (or re-drafts with justification) the Phase-A-scoped register rows as part of PR 0, and rows whose ratification structurally requires later-phase machinery are exempted by rule (see Change #8).
4. Add a validator check: if any phase row is `BLOCKED`, `validation-results.json` carries `"program_status": "blocked"` alongside the mechanical `"status"`.

**Rationale:**
R4#13 added the rule; this review finds the rule active, true, and invisible. A dossier whose validator says PASS while its own execution layer is self-certified BLOCKED violates the plan's deepest principle (§0.2: the user can determine what was omitted). The fix is not to soften the rule — it is correct — but to surface its verdict where every other verdict lives.

**Benefits:**
- The single most load-bearing status in the execution layer becomes visible and machine-checked.
- The ratification deadlock gets a defined exit instead of an implicit one.

**Trade-offs:**
- Publishing "BLOCKED" in the headline report is uncomfortable. That is the point; §26 says the correct thing must be easier than the seductive wrong thing.

**Implementation Notes:**
If the reality is a solo developer (the §18.6 "solo-developer default" suggests it), the honest fix may be to fill every row with the same name and a `minimum viable team: 1 (accepted risk)` annotation — the rule's value is forcing that admission into the record, not preventing solo work.

**Git-Diff:**
```diff
--- a/notes/plan/plan.md
+++ b/notes/plan/plan.md
@@ §0.3 @@
 - G0 status: 8 of 15 G0 items have spike evidence. DX-10, DX-13, and
   DX-14 are open and freeze-blocking (Phase A). ...
   Statuses live in `notes/G0_SPIKE_MATRIX.md`, from which these counts
   derive.
+- Program status: by the §21.1 owner rule, every phase whose
+  owner/team row in `notes/START_HERE_IMPLEMENTATION.md` is unfilled is
+  `BLOCKED`. As of this revision all six rows are unfilled; the program
+  is `BLOCKED`, and the dossier validator surfaces this state in
+  `validation-results.json` (`program_status`) beside the mechanical
+  pass/fail. Phase A unblocks by filling its row in PR 0, its
+  designated opening PR.
```

---

### [HIGH] Change #6: Cover the uncovered existential risks — R03 circular trust above all — and fix the R21 miscitation

**Current State:**
Only four of docs/08's 21 risks are cited anywhere in plan.md. Among the uncited:

- **R03 circular trust** (`docs/08:44-57`, high probability, "existential for assurance"): asupersync, CIR generation, and the checker share a defect and agree with each other. No §24 bullet, no §24.5 row, no detection mechanism. INV-004 is a design invariant, not a falsifier — nothing defines how correlated wrongness would ever be *noticed*.
- **R01 scope collapse** (`docs/08:13-28`, high/existential): its kill signal — "no project willing to migrate its DST" — has no plan counterpart; the first adoption checkpoint is the Phase F exit, years of work downstream.
- **R11 liveness intractable** (`docs/08:168-180`, high/severe for the TLA+-replacement thesis): no §24 bullet; the only register row is an unopened draft lane whose *fallback* ("unreduced liveness") presupposes exactly the tractability R11 doubts.
- **R21's kill signal is dropped and its citation inverted**: `plan.md:1073-1076` says the risk is "carried as docs/08 risk R21, with the mitigation 'edge classes collapse to Conservative' named as the failure mode" — but that phrase *is* R21's failure mode (`docs/08:314`); the actual controls are the Phase B ADR/spike and the parity audit (`docs/08:317-319`), and the kill signal "Conservative-collapse on the reference workload" (`docs/08:321`) appears nowhere in §24 or §24.5.

**Proposed Change:**
1. Fix the §9.1 sentence to cite R21 correctly and carry its kill signal into §24.
2. Add three kill-criteria bullets to §24: R03 (with its detection mechanism — see below), R01's adoption clause, R11's intractability clause.
3. Give R03 a real detector: a standing **differential-oracle discipline** — the corpus Tribunal (TLC/Apalache differential runs, already in ADR-0021) is R03's instrument for model semantics; for CIR/adapter semantics, require that every semantic epoch advance re-runs an N-family differential subset against the foreign oracles, and a correlated-agreement failure (Continuum and its own checker agree, foreign oracle disagrees, and the discrepancy survives triage) triggers the docs/09 soundness-incident policy. State this in §22's release-blocker doctrine.

**Rationale:**
The plan's entire assurance story assumes disagreement between independent components is how defects surface. R03 is the scenario where that assumption fails — the components aren't independent — and it is docs/08's highest-rated assurance risk. It deserves at least the sentence of kill machinery that lower-rated risks already have. The R21 miscitation is worse than a typo: it tells the reader the failure mode is the mitigation.

**Benefits:**
- The kill machinery covers all high/existential risks instead of four of eight.
- R03 gets a concrete instrument (the corpus differential runs already exist for other reasons — this is cheap).

**Trade-offs:**
- The foreign-oracle differential subset adds CI cost per epoch advance; bound it (N families, budget-capped) rather than running the full 80.

**Implementation Notes:**
Keep the §24 additions in docs/08's own wording where possible so the citation-sweep validator can match them.

**Git-Diff:**
```diff
--- a/notes/plan/plan.md
+++ b/notes/plan/plan.md
@@ §9.1 @@
-by a Phase B ADR with spike evidence (§21), and the decision is carried
-as docs/08 risk R21, with the mitigation "edge classes collapse to
-Conservative" named as the failure mode that destroys interactivity.
+by a Phase B ADR with spike evidence (§21), and the decision is carried
+as docs/08 risk R21, whose failure mode is edge classes collapsing to
+Conservative (destroying interactivity), whose controls are that ADR's
+spike plus the Incremental Parity Audit, and whose kill signal —
+Conservative-collapse on the reference workload — is carried in §24.
@@ §24 @@
 Continuum must narrow or redesign if:
 
 - real code requires pervasive rewrites solely to become observable;
+- correlated wrongness is detected: Continuum, its checker, and its
+  adapter agree while a foreign differential oracle (TLC/Apalache over
+  the corpus subset run at every semantic epoch advance) disagrees, and
+  the discrepancy survives triage — the docs/08 R03 circular-trust
+  scenario; this triggers the docs/09 soundness-incident policy, not
+  only a narrowing review;
+- no real project is willing to migrate its bespoke DST by the end of
+  Phase D, two phases before the Phase F migrations are due (docs/08
+  R01's kill signal, pulled forward as an adoption checkpoint);
+- unreduced liveness checking is intractable on the corpus liveness
+  subset and the reduction lane has not promoted (docs/08 R11) — the
+  register fallback "unreduced liveness" is not available if this
+  bullet fires;
+- incremental dependency edges collapse to Conservative on the
+  reference workload (docs/08 R21's kill signal);
```

---

### [HIGH] Change #7: Register the plan's most load-bearing unregistered hypotheses — ACI, explanation science, workbench security, multi-agent graph

**Current State:**
`plan.md:2457-2458` claims "Every HYPOTHESIS-class capability in this plan is owned by a research lane…". Four counterexamples:

1. **The ACI hypothesis (B2, §10).** It is a §24 kill (`plan.md:2431`), a G2 freeze gate (`plan.md:2268-2269`), and open freeze-blocking item DX-10 — yet `research/25` (a fully-formed lane with four kill criteria at `research/25:96-101`) appears in the register only as co-owner of the context row. There is no threshold, no definition of the "disciplined shell" baseline, no margin, no fallback. The entire native-protocol freeze turns on an unregistered comparison.
2. **Explanation science (§12).** §0.3 lists it as HYPOTHESIS (`plan.md:161`); the nearest row covers causal *minimization* (a mechanical trace property), not explanation adequacy or calibration. `research/34` carries four real kill criteria (`research/34:71-76`) and is cited nowhere in plan.md.
3. **Workbench security (§18).** `research/35:74-76` states a real gate ("autonomous promotion remains disabled until no known unprivileged path can alter intent/evidence status or escape isolation") — never cited, despite security being second in the grading order.
4. **Multi-agent evidence graph (§11).** `docs/53:141` bounds the spike ("not distributed storage or Byzantine agent resistance"), DX-12's matrix cell says "authority table only, not enforcement" — yet §11 is not in §0.3's HYPOTHESIS list and has no register row, while Phase F ships "multi-agent workbench".

**Proposed Change:**
Add four register rows (ACI with the research/25 kills and a DX-10-aligned threshold; explanation science owned by research/34 with its stated kills, instrumented by the formative sessions and G8; workbench security owned by research/35 with its promotion gate; multi-agent graph concurrency/enforcement owned by RFC 0038's §11.7 work with the docs/53 boundary as its baseline). Add "§11 concurrent enforcement" to §0.3's HYPOTHESIS list.

**Rationale:**
The register exists so that no plan section claims a lane's output without its status (`plan.md:2459-2460`). The four missing rows are not marginal: one gates the Phase A freeze, one gates G8, one is the grading order's second dimension, one ships in Phase F. Their absence makes §24.5's universal claim false in the places it matters most.

**Benefits:**
- The freeze decision (DX-10) inherits a baseline definition, margin, and fallback instead of an unquantified "beats".
- research/34 and research/35 — the only two research notes with real criteria that the plan never cites — enter the governance loop.

**Trade-offs:**
- Four more draft rows worsen the Phase A ratification pressure; Change #8 resolves that rule first.

**Implementation Notes:**
For the ACI row, the baseline must be specified adversarially (a *disciplined* shell harness with the same documentation and retries — research/25 defines this) or the comparison is gameable in the ACI's favor.

**Git-Diff:**
```diff
--- a/notes/plan/plan.md
+++ b/notes/plan/plan.md
@@ §24.5 table @@
 | Timed/probabilistic semantics | ADR-0016 — lane to be opened | per-ADR staging; ... | declarative assumptions only |
+| Agent–computer interface (B2, §10) | research/25 | native ACI vs a disciplined-shell baseline (harness per research/25) on the agent benchmark: success rate and cost margins fixed at ratification — draft; kills (research/25): typed surface loses to disciplined shell; schema churn dominates agent cost; handles do not reduce invalid-action rate | protocol redesign before freeze (G0-DX-10); MCP-only surface |
+| Explanation science (§12) | research/34 | diagnosis accuracy/time vs raw-trace baseline under the G8 instruments and Phase C formative sessions; thresholds fixed in the docs/48 preregistration — draft; kills (research/34): experts prefer raw output and newcomers gain no accuracy; assistance raises confidence faster than correctness | §12.1 levels 1–2 only (outcome + causal core), no adequacy claim |
+| Workbench security (§18) | research/35 | promotion gate per research/35: autonomous promotion remains disabled until no known unprivileged path can alter intent/evidence status or escape isolation; red-team corpus per research/35 | human-approved promotion only |
+| Multi-agent evidence graph enforcement (§11.7) | RFC 0038 (§25 debt); docs/53 boundary | concurrent authority enforcement validated beyond the finite spike (docs/53: "authority table only, not enforcement"): linearized promotion, CAS status, conflict materialization under concurrent writers — draft | single-writer evidence graph; agents coordinate through one integrator role |
@@ §0.3 @@
-- HYPOTHESIS: §6 general context compilation, §8.3 neighborhood adequacy,
-  §9 sub-file-granularity incremental trust, §12 causal explanation science,
-  §14 Forge co-synthesis and quality-diversity, §16 bidirectional lenses,
-  production partial-order conformance (Phase F). Each is owned by a
-  research lane with kill criteria (see §24.5).
+- HYPOTHESIS: §6 general context compilation, §8.3 neighborhood adequacy,
+  §9 sub-file-granularity incremental trust, §10 ACI superiority (B2),
+  §11.7 concurrent evidence-graph enforcement, §12 causal explanation
+  science, §14 Forge co-synthesis and quality-diversity, §16
+  bidirectional lenses, production partial-order conformance (Phase F).
+  Each is owned by a research lane with kill criteria (see §24.5).
```

---

### [HIGH] Change #8: Fix the ratification deadline rule — six rows cannot satisfy it as written

**Current State:**
`plan.md:2487-2488`: "A register row may not carry an unratified or `TBD` threshold past Phase A; such a row blocks its lane's promotion." Six rows are structurally un-ratifiable by Phase A: liveness reduction ("lane to be opened", Phase D machinery), neighborhood adequacy (lane to be opened; corpus is a Phase B deliverable), sub-file incremental trust (its rate "derives from §8.6's confidence target … **reviewed at G5**" — Phase C, two phases past the deadline, a flat internal contradiction), production conformance (the curated incident corpus is a Phase F deliverable), weak memory and timed/probabilistic (explicit post-1.0 deferrals). The rule's two halves also conflict: for permanently-deferred lanes, "blocks its lane's promotion" is the intended and harmless steady state, making the "may not carry past Phase A" prohibition dead letter.

**Proposed Change:**
Replace the blanket deadline with a phase-indexed rule: a row must be ratified before **the phase that first consumes its lane's output** opens (liveness → Phase D; neighborhood adequacy → Phase B; sub-file trust → Phase C; production conformance → Phase F; deferred lanes → exempt while `defer` is recorded). Phase A retains the deadline only for rows whose lanes gate Phase A itself (context compilation, ACI, causal minimization).

**Rationale:**
An unenforceable rule in the register's preamble undermines the register's authority exactly where Change #3 is trying to restore it. The phase-indexed form preserves the rule's intent — no phase consumes an unratified lane — while being satisfiable.

**Benefits:**
- The rule becomes checkable (validator: each row names its consuming phase; ratification status compared at phase-open).
- Removes the false pressure to invent numbers in Phase A for lanes whose instruments don't exist yet — exactly the failure mode that produced the circular ≤10% provenance in the causal-minimization row (`research/26:108-110` cites the plan; the plan cites the note; nobody derived the number).

**Trade-offs:**
- Later ratification means later falsifiability for late-phase lanes. That is honest: a number invented before its instrument exists is not falsifiability, it is decoration.

**Implementation Notes:**
Add a "Ratify by" column to the register table so the validator has a structural hook.

**Git-Diff:**
```diff
--- a/notes/plan/plan.md
+++ b/notes/plan/plan.md
@@ §24.5 @@
-A register row may not carry an unratified or `TBD` threshold past
-Phase A; such a row blocks its lane's promotion.
+A register row must be ratified before the phase that first consumes
+its lane's output opens (recorded per row in a "Ratify by" column);
+until then it blocks its lane's promotion. Rows whose lanes gate
+Phase A itself (context compilation, ACI, causal minimization) must
+ratify in Phase A. Rows recorded as `defer` (weak memory,
+timed/probabilistic) are exempt while the deferral stands. A row whose
+threshold cannot be ratified by its own deadline is a plan defect, not
+a lane defect.
```

---

### [HIGH] Change #9: Repair the G0 freeze-blocking subset — four items do not run against Phase A machinery, and one preregistration contradiction is live

**Current State:**
- `plan.md:2237-2240` justifies the freeze-blocking subset (DX-01–05, 07, 08, 10, 12, 13, 14) as "every item whose required experiment runs against Phase A machinery." But DX-07 requires Forge (Phase E), DX-08 requires lenses (Phase D), DX-04 the causal debugger (Phase B), DX-05 incrementality (Phase C). All four are currently "closed" by finite Python spikes whose own Decision cells read "adopt artifact shape; engine unproven (docs/53)". Combined with G0's pass condition ("evidence **or an explicit redesign decision**"), the freeze gate is satisfiable for these items by a spike plus a note — i.e. not load-bearing.
- `notes/G0_SPIKE_MATRIX.md:5` says DX-09's preregistration covers "the **four** docs/34 acceptance workflows"; `plan.md:2022-2028` and `docs/34:135-139` fix it at **three** human-executed workflows (agent repair routed to G2/ContinuumBench). The matrix header is the normative source for DX-09's requirement, and it states the wrong scope.
- Three Phase A freeze decisions (DX-10; G2's "Context Packs improve agent benchmark effectiveness", `plan.md:2267`; the context-lane draft status) are measured on the agent benchmark — whose validity check (DX-15, benchmark leakage) is re-homed to G9 = Phase F. Nothing acknowledges that Phase A freezes rest on a Phase F-validated instrument.

**Proposed Change:**
1. Fix the matrix header to three workflows.
2. Split the freeze-blocking subset honestly: DX-01–03, 10, 12–14 remain freeze-blocking on Phase A machinery; DX-04, 05, 07, 08 are reclassified as "artifact-shape evidence accepted for freeze; engine evidence re-homed" to G4, G5, G7, and G6/G7 respectively — the same re-homing device the plan already uses for DX-06/09/11/15, recorded as each item's Decision.
3. Add an interim benchmark-validity criterion to G2: the Phase A benchmark subset used for DX-10 and the Context Pack ablation must satisfy a scoped-down leakage check (train/dev separation by semantic family and source hash — the §19.4 rule, which needs no Phase F machinery), so the Phase A freeze does not rest on an unvalidated instrument.

**Rationale:**
G0 is the falsification gate; its credibility is the dossier's credibility. A gate whose justifying sentence ("runs against Phase A machinery") is false for four of eleven items invites exactly the "spike plus note" satisfaction it was designed to prevent. The DX-09 contradiction is small but sits in the one file the plan declares normative for preregistration scope.

**Benefits:**
- G0's freeze-blocking claim becomes true.
- The re-homing device is already established; this extends it consistently.
- Phase A decisions stop depending on Phase F validation.

**Trade-offs:**
- Reclassifying DX-04/05/07/08 weakens the advertised strength of G0 closure. It was already weak; this makes the weakness legible, which is the plan's own standard (INV-007).

**Implementation Notes:**
The matrix carries Status/Evidence/Decision columns; the reclassification is a Decision-cell edit plus a §22 G0 paragraph edit. §0.3's derived counts will change (from "8 evidence" to a split count); the validator derivation handles this automatically.

**Git-Diff:**
```diff
--- a/notes/plan/plan.md
+++ b/notes/plan/plan.md
@@ §22 G0 @@
-G0 closes in Phase A for every item whose required experiment runs
-against Phase A machinery: DX-01–05, 07, 08, 10, 12, 13, 14. An
-unexecuted or failed item in this subset blocks interface freeze. Items
-whose experiments require later subsystems are re-homed to the gates
-that own them — DX-06 (neighborhood/mutation campaign) → G4, DX-11
-(proof-service isolation) → G6, DX-09 (human diagnosis study) → G8,
-DX-15 (benchmark leakage) → G9 — and each re-homing is recorded in the
-matrix as that item's explicit decision.
+G0 closes in Phase A for every item whose required experiment runs
+against Phase A machinery: DX-01–03, 10, 12, 13, 14. An unexecuted or
+failed item in this subset blocks interface freeze. Items whose
+experiments require later subsystems are re-homed to the gates that own
+them — DX-04 (causal debugger) → G4, DX-05 (incrementality) → G5,
+DX-06 (neighborhood/mutation campaign) → G4, DX-07 (Forge
+non-vacuity) → G7, DX-08 (lens ambiguity) → G6, DX-11 (proof-service
+isolation) → G6, DX-09 (human diagnosis study) → G8, DX-15 (benchmark
+leakage) → G9 — with their Phase A spike evidence recorded as
+artifact-shape evidence only, and each re-homing recorded in the matrix
+as that item's explicit decision. The Phase A benchmark subset used for
+DX-10 and the G2 Context Pack ablation must itself pass the §19.4
+family/source-hash separation check before either result is accepted;
+full leakage validation remains DX-15 at G9.
```

---

### [HIGH] Change #10: Reconcile the reward-hacking suite and the held-out discipline with docs/45 and docs/50

**Current State:**
- `plan.md:1927-1936` (§19.5) lists 10 gaming mutations; `docs/45:160-174` lists 13. Missing from the plan: **"trivial true property"** (vacuity — the direct instrument for §14.5, the Forge kill at `plan.md:2435`, and DX-07), **"disabled instrumentation"** (the only representative of docs/50's entire Instrumentation-attacks class, `docs/50:18-24`), and **"semantically equivalent distractor patch"** (the grader-discrimination control). docs/50's Resource-attacks class (`docs/50:42-47`) has zero representation despite `plan.md:1889-1890` making expensive-failure detection first-class, and "lower assurance" is absent despite the spike having detected exactly that mutation (`docs/53:46`).
- `plan.md:1917-1921` fixes the held-out discipline: held out "from all development, tuning, and regression use" with "a final, single evaluation". But **G3** (`plan.md:2275-2277`, Phase B) grades "every gaming mutation in the hidden (held-out) suite" as a pass/fail gate criterion — regression use, two phases before the single final evaluation — and G9 then evaluates it again.
- RFC 0034's grading order covers 7 stages; docs/45's score vector has 10 components and §19.3's metric list has 13. The unordered remainder (repair robustness, invalid actions, expensive failures, context compressions, behavioral novelty, diagnosis time, calibration) has undefined tie-break behavior in a lexicographic-with-cap scheme — a live gaming surface given docs/50's resource attacks target exactly those dimensions.
- `docs/45:43` lists a "weak-memory ordering" diagnosis track while `plan.md:2484` declares every memory dimension `Unsupported(sequential-consistency-only)` and SC-only a 1.0 non-goal — an ungradeable track, and this plan↔docs/45 disagreement is not in the §25 remediation list.

**Proposed Change:**
1. Extend §19.5 to the full docs/45 mutation list plus a resource-attack family and assurance-downgrade (matching the seven G3 dimensions).
2. Split the gaming corpora: a **development gaming suite** (used by G3 in Phase B, disclosed) and the **held-out suite** (single final evaluation at G9). G3's criterion cites the development suite; G9's cites both.
3. Extend RFC 0034's ordering (or explicitly declare the unordered dimensions "reported, never ranked") so tie-break behavior is defined.
4. Mark docs/45's weak-memory track "blocked on ADR-0032 lane" or delete it.

**Rationale:**
B22 says benchmarks must test governance. A suite missing vacuity cannot test the Forge kill; a suite missing instrumentation-disable cannot test INV-005's payoff; a held-out set consumed by a Phase B gate is not held out. These are not doc-drift nits — they are the difference between the benchmark measuring the defenses and measuring around them.

**Benefits:**
- Every docs/50 attack class gains at least one instrument.
- The held-out discipline becomes internally consistent and enforceable.
- Grading becomes deterministic across the full metric vector.

**Trade-offs:**
- Maintaining two gaming suites costs authoring effort; the split is the standard ML-evaluation discipline the plan already endorses for corpus families (§19.4).

**Implementation Notes:**
The development/held-out split should reuse §19.4's partition-by-semantic-family rule so mutations of one family never straddle the split.

**Git-Diff:**
```diff
--- a/notes/plan/plan.md
+++ b/notes/plan/plan.md
@@ §19.5 @@
 Every benchmark candidate is tested against attempts to:
 
 - weaken property;
 - add assumptions;
 - lower bounds;
 - remove faults;
 - hide observer events;
+- lower assurance class;
+- substitute a trivially true property (vacuity);
+- disable or bypass instrumentation (docs/50 instrumentation attacks);
+- exhaust or misdirect budgets (docs/50 resource attacks);
+- pass a semantically equivalent distractor patch (grader control);
 - return unsupported as pass;
 - hard-code known trace;
 - exploit stale cache;
 - forge receipt/status;
 - smuggle instructions through source.
@@ §19.4 @@
-A named subset of corpus families is held out from all development,
-tuning, and regression use and graded only by the isolated ContinuumBench
-grader. G9's "80 families at declared parity" is measured on the
-development set plus a final, single evaluation of the held-out set.
+Gaming mutations are split into a development suite — used by G3's
+Phase B gate and disclosed to implementers — and a held-out suite,
+partitioned by semantic family per this section's rule. A named subset
+of corpus families and the held-out gaming suite are excluded from all
+development, tuning, and regression use and graded only by the isolated
+ContinuumBench grader. G3 cites the development suite; G9's "80
+families at declared parity" is measured on the development set plus a
+final, single evaluation of the held-out set and held-out suite.
```

---

### [HIGH] Change #11: One assurance envelope, one `Redacted`, one verdict vocabulary

**Current State:**
The dossier now contains **two incompatible assurance envelopes**: `assurance-result.schema.json:133-150` (nine required dimensions, each `{engine, summary}` or typed `unsupported`) and `context-pack.schema.json:44-107` (five required dimensions, bare enums, `fairness: string|null`) — the latter's own description claims "Every dimension names its producer," which its structure makes impossible, and it violates RFC 0026's result-envelope rule. Verdict vocabularies drift (`engine_error` vs `engine-error`; context-pack `inconclusive` has no INV-008 reason conditional). `assurance-result` itself double-encodes failure (`verdict: "unsupported"` bypasses the typed-reason conditional that `inconclusive`+`Unsupported` triggers) and its `unsupported` dimension value is a free string, not the "typed `Unsupported(reason)`" `plan.md:2547` demands. Gate statuses spell `passed` in `promotion-receipt` but `pass` in `repair-transaction`, so a promoted transaction cannot round-trip into a valid receipt. Cost-ledger dimensionality is 5 (`promotion-receipt`) vs 8 (RFC 0026 budget) vs 9 (`verification-task`), so "actual spend per budget dimension" cannot round-trip either.

**Proposed Change:**
Add a §25 debt item: a single `assurance-envelope.schema.json` `$def` that every artifact class `$ref`s (context packs included); one verdict enum with hyphenation fixed and the `unsupported`/`engine-error` top-level verdicts removed in favor of `inconclusive` + typed reason (matching §11.4: these are `Inconclusive` reasons, not verdicts); one gate-status enum; one budget/cost dimension list shared by RFC 0026, `verification-task`, and `promotion-receipt`. Appendix A items A6–A8 carry the file-level edits.

**Rationale:**
B11 prohibits "Verified" alone in machine output; a second, weaker envelope in the artifact agents actually consume (the Context Pack) reintroduces exactly the under-specified assurance B11 bans. Spelling drift between mirrored schemas means every consumer needs case-by-case mappings — the "terminal sludge" failure mode in typed clothing.

**Benefits:**
- One envelope definition; conformance becomes a `$ref`, not a copy.
- Receipts, transactions, tasks, and budgets round-trip.
- INV-008's typed reasons apply uniformly.

**Trade-offs:**
- Context Packs get heavier; acceptable — the envelope is the part §3.1's five-minute path already renders.

**Implementation Notes:**
Do this together with the shared `Redacted` `$ref` (Change #2) since both establish the schemas' first cross-file `$ref` convention; decide the convention (relative `$ref` vs bundled `$defs`) once.

**Git-Diff:**
```diff
--- a/notes/plan/plan.md
+++ b/notes/plan/plan.md
@@ §25 specification-debt list @@
+- schemas: one shared assurance-envelope `$def` referenced by every
+  artifact class that renders assurance — `context-pack` currently
+  defines a second, five-dimension envelope that cannot name producers,
+  violating B11 and RFC 0026's result-envelope rule; one verdict enum
+  (hyphenation unified; `unsupported`/`engine-error` demoted from
+  verdicts to INV-008 `Inconclusive` reasons per §11.4); one gate-status
+  enum (`promotion-receipt` says `passed`, `repair-transaction` says
+  `pass` — a promoted transaction cannot round-trip into a valid
+  receipt); one budget/cost dimension list shared by RFC 0026,
+  `verification-task`, and the §8.6 cost ledger (currently 8 vs 9 vs 5);
```

---

### [HIGH] Change #12: Give every phase deliverable an owning PR — starting with the one that blocks Phase C

**Current State:**
Phase deliverables with no PR anywhere in START_HERE's 34 headings:

- The **build-vs-adopt ADR + spike for the §9 incremental engine** (`plan.md:2076-2080`) — the plan says "Phase C is `BLOCKED` until this ADR exists," yet START_HERE goes straight to `PR 23 — Incremental query database v0` with no ADR/spike PR. A hard blocker on the critical path has no owning work item.
- The read-only tokio observation lane (`plan.md:2100-2104`) — the stated "adoption top-of-funnel." No PR.
- Wave 0/1 corpus at min(required, P2) in Phase C (`plan.md:2105-2110`) — no PR; `START_HERE:456` defers all corpus ports past PR 30.
- CML formatter and migration tool, which must ship *before* syntax stability (`plan.md:2111-2113`) — PR 15a delivers only parser + elaborator.
- Formative usability sessions (`plan.md:2114-2118`), the G8 preregistration authoring (`plan.md:2029-2031`) — no PRs; docs/48 is absent from PR 0's expansion list.
- §25's "paid before PR 5" debt covers RFC 0038, all schema items, the property-AST, and the docs/35/ADR-0018/docs/42 absorptions — but PR 0's exit condition (`START_HERE:74`) covers only the seven RFCs plus licensing. PR 0 certifies less than §25 requires before PR 5.
- The four `continuum-kernel-*` crates and the <15,000-line TCB covenant appear only in START_HERE (PR 9), not in §21 Phase A's deliverable list — a gate-grade constraint living only in the PR list.

**Proposed Change:**
1. Insert `PR 22a — incremental engine ADR + reuse-edge spike` (annotated `[G5; blocks Phase C]`) before PR 23.
2. Add PRs (or extend existing ones) for the tokio observation lane, corpus-at-P2 batch, CML formatter/migration, and prereg/formative-session authoring; annotate each with its gate.
3. Extend PR 0's exit condition to the full §25 pre-PR-5 debt set (which Change #2 makes validator-derived, so the exit condition can simply cite the validator's open-debt list being empty for pre-freeze items).
4. Add the kernel crates + TCB covenant to §21 Phase A's deliverables so the covenant is phase-normative, not only PR-annotated.

**Rationale:**
The plan's own §21.1 makes phases gate-driven; a gate whose closing deliverable has no work item is a gate that closes by assertion. The incremental-engine ADR is the worst instance: it is simultaneously "blocks all of Phase C" and unowned.

**Benefits:**
- Every "Phase X is BLOCKED until Y" statement gains an addressable Y.
- PR 0's exit stops under-certifying §25.

**Trade-offs:**
- START_HERE grows. It is already explicitly "initial"; growth toward truth is fine.

**Git-Diff:**
```diff
--- a/notes/plan/plan.md
+++ b/notes/plan/plan.md
@@ §21 Phase A deliverables @@
 - executable finite reference engine and certificates from Revision 2;
+- the four `continuum-kernel-*` crates under the docs/03 covenant
+  (<15,000 non-test lines, no async, no unsafe, serialization boundary)
+  — the covenant is phase-normative here, not only a PR 9 annotation;
 - agent protocol spike parity;
@@ §25 @@
 The ordered initial PR sequence (PR 0–PR 30, with inserts 4a/15a/25a) is
 in [`notes/START_HERE_IMPLEMENTATION.md`](notes/START_HERE_IMPLEMENTATION.md).
+Additional inserts carry the phase deliverables previously unowned by
+any PR: 22a (incremental-engine ADR + reuse-edge spike; blocks Phase C
+per §21), the tokio read-only observation lane, the Wave 0/1
+corpus-at-P2 batch, the CML formatter/migration tool, and the docs/48
+preregistration + formative-session authoring. PR 0's exit condition is
+the validator's pre-freeze open-debt list reading empty, not the seven
+RFC expansions alone.
```

---

### [MEDIUM] Change #13: Fix the PR/gate sequencing inversions and the two cross-phase gate anomalies

**Current State:**
- `START_HERE:62` permits later-phase gates only "where §21 explicitly pulls work forward", yet PRs 23–26/25a advance G5 (Phase C), PR 28 advances G6 (Phase D), PR 29 G7 (Phase E) — and **PR 30, the final PR, advances G2 and G4 (Phases A and B)**. Taken literally, Phase A cannot close until after Phase D and E machinery lands: a dependency cycle between the gate-driven-phases model and the ordered PR sequence.
- "Proof Context Packs" is a Phase C deliverable (`plan.md:2099`) whose only acceptance criterion lives in G6 (Phase D) and whose only PR is PR 28 `[G6]` — a deliverable whose acceptance machinery does not exist in its phase.
- Phase D's self-application defect evidence "is recorded as G4-class evidence" (`plan.md:2141-2144`), but G4 closes in Phase B. Either gates can accrue post-closure evidence or the classification is decorative; the plan never says which.

**Proposed Change:**
1. State the reconciliation rule in §21: the PR sequence is *interleaved across phases by design*; a phase closes when its gates close, regardless of PR ordinal; PR 30's G2/G4 items are Phase A/B *hardening*, and the gates those phases close must be closable without them (or the specific bullets move into the G2/G4 gate text with their PR annotated as pulled-forward).
2. Move "proof Context Packs" to Phase D, or split it (format + compiler plumbing in C, `[G5]`; proof-worker efficacy criterion in D, `[G6]`).
3. Define post-closure evidence: a closed gate's criteria remain permanent regressions (release-blocker doctrine already implies this); new supporting evidence attaches to the gate as regression evidence without reopening it. One sentence in §22.

**Rationale:**
These are the load-bearing sequencing rules an implementer hits in week one. The current text supports two contradictory readings of when Phase A ends, and reviews 1–4 already burned four passes on phase/gate coherence — this is the residue.

**Benefits:**
- One defined answer to "when does Phase A close?"
- The self-application deliverable's evidence class becomes meaningful.

**Trade-offs:**
None material.

**Git-Diff:**
```diff
--- a/notes/plan/plan.md
+++ b/notes/plan/plan.md
@@ §21 preamble @@
 Phases are gate-driven, not time-driven.
+The PR sequence in START_HERE interleaves phases by design: a phase
+closes when its assigned gates close, regardless of PR ordinal, and a
+PR advancing an earlier phase's gate after that gate has closed is
+hardening — it attaches regression evidence to the closed gate without
+reopening it. A gate's criteria remain permanent regressions after
+closure (§22 release-blocker doctrine).
@@ §21 Phase C deliverables @@
--- proof Context Packs;
+- proof Context Pack format and compiler plumbing (the proof-worker
+  efficacy criterion is G6's, evaluated in Phase D);
```

---

### [MEDIUM] Change #14: Bind phase-exit criteria to gates — or they are not release authority

**Current State:**
§22's reconciliation rule binds §22↔docs/52 only; §21's phase *exits* are unbound, and several binding-sounding exit criteria exist in no gate: Phase C's "zero reachability mismatch against the unreduced reference on the no-reduction corpus", the certified-claims fallback (docs/08 R04), and "semantic artifacts are byte-identical across the docs/19 determinism matrix" appear in neither G5 nor docs/52; Phase D's "one nontrivial corpus protocol has safety and liveness evidence plus real-code refinement" is absent from G6. Conversely, G5's latency criterion omits the measurement regime docs/34 declares normative — "measured with a saturating background swarm present" (`docs/34:82-83`, `plan.md:527`) — so the bidirectional §22↔docs/52 validator can never notice the gate being asserted without its measurement condition. And `plan.md:2119-2123` claims the docs/34 explain-row is "marked in docs/34 itself"; the row exists but carries no marking.

**Proposed Change:**
1. Extend the §22 reconciliation rule: every §21 phase-exit criterion either quotes a gate bullet or is explicitly labeled `phase-local (not release authority)`; the validator checks the mapping.
2. Fold the four orphaned exit criteria into G5/G6 (and docs/52, same commit, per the existing rule).
3. Add the saturating-swarm measurement condition to G5's latency bullet in both files.
4. Mark the explain row in docs/34 or drop the "both marked" claim.

**Rationale:**
The plan's central governance claim is that gates are the release authority. Criteria that bind only in §21 either silently bind (two authorities) or silently don't (decorative exits); both readings are wrong.

**Benefits:**
- One authority for release criteria, with the validator able to prove it.
- G5 becomes unpassable on an idle daemon — closing the obvious loophole.

**Trade-offs:**
- More gate bullets to maintain; the validator already counts 65 per direction, so the marginal cost is small.

**Git-Diff:**
```diff
--- a/notes/plan/plan.md
+++ b/notes/plan/plan.md
@@ §22 preamble @@
 `docs/52` and this section are reconciled in both directions in one
 commit: every criterion this section adds is folded into `docs/52`, no
 `docs/52` criterion is dropped here, and the dossier validator enforces
 bullet-for-bullet correspondence between the two.
+Every §21 phase-exit criterion likewise either corresponds to a gate
+bullet or is explicitly labeled phase-local; the validator enforces the
+mapping. A binding release criterion that exists only in a phase exit
+is a defect.
@@ §22 G5 @@
-- interactive latency targets (docs/34) hold on the reference workload;
+- interactive latency targets (docs/34) hold on the reference workload,
+  measured with a saturating background swarm present (§4.1, docs/34);
+- reduction engines show zero reachability mismatch against the
+  unreduced reference on the no-reduction corpus; certified claims fall
+  back to the unreduced baseline until the reduction certificate lane
+  matures (docs/08 R04);
+- semantic artifacts are byte-identical across the docs/19 determinism
+  matrix;
```

---

### [MEDIUM] Change #15: Complete the operation registry and error taxonomy against the behaviors §4 specifies

**Current State:**
- `plan.md:581-582` (§4.3) requires "cancellation and budget updates" and "evidence subscriptions" as protocol requirements, but §10.2 and RFC 0027's registry offer no `task.update_budget` and no `evidence.subscribe` (`task.subscribe` in RFC 0026 is explicitly progress-hints only). §25 books this as RFC 0026 debt; the hole is in the plan's own operation list.
- Specified failure modes with no §10.3 error code: intent-bundle acceptance-chain verification failure (`plan.md:565-567` — CI "fails closed", into what error?), evidence-graph CAS status-promotion conflict (`plan.md:1366-1368`), capability quota exhaustion (`plan.md:521-522` makes quotas distinct from budgets), evidence-epoch rejection, and atomic publication abort on disk exhaustion (`plan.md:645-647`).
- `rfcs/0027:20` claims each registry entry carries "a minimum authority level," but two rows list two (`repair … propose/execute`, `task … read/execute`), leaving `CapabilityDenied` undecidable from the table for eight operations.

**Proposed Change:**
Add `task.update_budget` and `evidence.subscribe` to §10.2; add five error codes (`AcceptanceChainInvalid`, `StatusConflict`, `QuotaExhausted`, `EpochUnsupported`, `PublicationAborted`) to §10.3; split the two ambiguous RFC 0027 rows per-operation (Appendix A item A10).

**Rationale:**
INV-003 and B18 are only as strong as the registry's coverage. Every unrepresentable failure mode becomes a prose error string — the exact thing the taxonomy exists to eliminate.

**Benefits:**
- §4.3's requirements become implementable as typed operations.
- Agents can distinguish quota, budget, and epoch failures — three different recovery strategies.

**Trade-offs:**
None material; enum growth is cheap pre-freeze and expensive after, which is why this is pre-PR-5.

**Git-Diff:**
```diff
--- a/notes/plan/plan.md
+++ b/notes/plan/plan.md
@@ §10.2 @@
-task.status / cancel / resume / subscribe
+task.status / cancel / resume / subscribe / update_budget
-evidence.get / query / verify
+evidence.get / query / verify / subscribe
@@ §10.3 @@
 CertificateRejected
 ReplayDiverged
 CapabilityDenied
 PolicyGateFailed
+AcceptanceChainInvalid
+StatusConflict
+QuotaExhausted
+EpochUnsupported
+PublicationAborted
 ProtocolVersionUnsupported   (protocol-level, RFC 0026)
```

---

### [MEDIUM] Change #16: Close the continuation fail-opens and the snapshot-schema gap

**Current State:**
- `verification-task.schema.json` lists `BudgetExhausted` among `failed_reason` values with `continuation` nullable and no conditional — so "never a silently smaller campaign … with a continuation" (`plan.md:1060-1064`) is unenforced; likewise `status: "suspended"` does not require a continuation despite §4.1 defining Suspended as "continuation + committed partial evidence".
- `workspace-snapshot.schema.json` (with `additionalProperties: false`) carries six of §4.2's ten components — no CML modules, Rust semantic extraction, domain-pack manifests, generated correspondence, proof environment, or configuration — and its `epochs` embeds a **protocol** epoch in snapshot identity, which makes content-addressed snapshots churn on protocol upgrades, contradicting §4.6's "advancing one never mutates existing artifacts". No schema anywhere carries the `intent`, `evidence`, or `corpus` epochs RFC 0026 declares.

**Proposed Change:**
Add conditionals (suspended ⇒ continuation required; `failed_reason: BudgetExhausted` ⇒ continuation required unless a typed `no-continuation` reason is present); extend the snapshot schema to §4.2's component list; move the protocol epoch out of snapshot identity into the connection/result envelope where RFC 0026 already carries it. §25 debt entries; file edits in Appendix A items A11–A12.

**Rationale:**
B18 (budget semantics with resumable continuations) is a headline bet; the schema currently validates the exact behavior B18 prohibits. The snapshot gap means six of the ten §4.2 components cannot appear in the artifact that defines "what was analyzed" — undermining §0.2's product theorem at its root object.

**Benefits:**
- `BudgetExhausted`-without-continuation becomes a validation failure, not a runtime hope.
- Snapshot identity stops depending on a connection property.

**Trade-offs:**
- Extending a `additionalProperties: false` schema is a breaking change; pre-freeze is the time.

**Git-Diff:**
```diff
--- a/notes/plan/plan.md
+++ b/notes/plan/plan.md
@@ §25 specification-debt list @@
+- schemas: `verification-task` requires a continuation on
+  `status: suspended` and on `failed_reason: BudgetExhausted` (typed
+  exception for genuinely non-resumable exhaustion) — B18 is currently
+  unenforced at the wire; `workspace-snapshot` carries all ten §4.2
+  components (six are missing under `additionalProperties: false`) and
+  drops the protocol epoch from snapshot identity (a connection
+  property; §4.6 forbids identity churn on epoch advance);
```

---

### [MEDIUM] Change #17: Small governance reconciliations — corpus checkpoints, merge-list mislabel, docs/04 residual

**Current State:**
- `corpus/tla-examples/PARITY_LEVELS.md:78-84` defines release checkpoints 0.1, 0.2, 0.5, 1.0, and 1.x; §21 maps 0.2/0.5/1.0 to phases (`plan.md:2147-2150`) and leaves 0.1 and 1.x unmapped.
- `plan.md:2488-2492` calls the docs/31 merge set "quantitative thresholds" and includes "abstraction discovery," whose docs/31 criterion (`docs/31:51-52`) is purely qualitative — the merge instruction as written cannot be executed for that item.
- docs/04 is bannered (done) but not archived: a third gate numbering (`G0A–G0D, G1–G6`, `docs/04:11-22`) still lives in the live docs/ tree while `plan.md:2216` says "No document may introduce a new gate numbering," and the hygiene check (`validate_dossier.py:55`) only matches suffixed `G\d+-(Corpus|Proof)` names, so it cannot see docs/04's scheme. §22 also still lists the bannering as pending §25 work.

**Proposed Change:**
Map 0.1 to Phase C entry and 1.x to post-G9 maintenance; reword the merge instruction ("docs/31's remaining thresholds — quantitative where stated, with abstraction discovery entering as a draft qualitative row pending ratification"); move docs/04 to `archive/` (the plan's own preferred disposition) and update §22's sentence to past tense; extend the hygiene check to flag bare gate citations inside `docs/04`-style superseded schemes if the file stays.

**Rationale:**
Three small truths that each mislead an implementer: an unmapped checkpoint has no owner; an unexecutable merge instruction stalls the "one lane authority" goal; a live file with a forbidden gate numbering contradicts a "no document may" rule the validator claims to police.

**Benefits:**
- Corpus release governance covers its whole checkpoint ladder.
- The §25 merge pass becomes executable as written.

**Trade-offs:**
None.

**Git-Diff:**
```diff
--- a/notes/plan/plan.md
+++ b/notes/plan/plan.md
@@ §21 Phase D @@
-- corpus release checkpoints mapped to phases: 0.2 closes in Phase D;
+- corpus release checkpoints mapped to phases: 0.1 closes at Phase C
+  entry; 0.2 closes in Phase D;
   0.5 (Waves 0–2 at required parity plus ≥5 P5 exemplar families, now
   designated by name in `validated-examples.csv`) closes in Phase E;
-  1.0 (all 80) closes in Phase F with G9.
+  1.0 (all 80) closes in Phase F with G9; 1.x tracks post-G9 upstream
+  corpus additions as maintenance, outside the phase ladder.
@@ §22 @@
-the translation sweep is part of the specification pass in §25 and includes
-retiring the `-Corpus`/`-Proof` gate suffixes (RFCs 0011/0012/0019) and
-ADR-0022's internal Lean G-ladder, and archiving or Rev-2-bannering
-`docs/04`, whose gate graph matches neither `docs/26` nor `docs/52`.
+the translation sweep is part of the specification pass in §25 and includes
+retiring the `-Corpus`/`-Proof` gate suffixes (RFCs 0011/0012/0019) and
+ADR-0022's internal Lean G-ladder. `docs/04` is Rev-2-bannered (done)
+and is moved to `archive/` so no live document carries its gate
+numbering.
@@ §24.5 @@
-docs/31's remaining
-quantitative thresholds (cubical reduction ≥3× on ≥2 real protocol
-classes with a preservation theorem; semiring within 15% of specialized
-analyses with three production analyses sharing code; assumption
-synthesis on ≥5 liveness cases; abstraction discovery) are merged into
+docs/31's remaining thresholds (cubical reduction ≥3× on ≥2 real
+protocol classes with a preservation theorem; semiring within 15% of
+specialized analyses with three production analyses sharing code;
+assumption synthesis on ≥5 liveness cases; and abstraction discovery,
+which is qualitative in docs/31 and enters as a draft row pending a
+quantitative ratification) are merged into
```

---

### [LOW] Change #18: Mechanical residue

**Current State (all verified this pass):**
- START_HERE gate annotations: DX-13's closing PR 2 reads `[G1]` and DX-14's PR 6 reads `[G1]` with no `[G0 (DX-13/14)]` marker, though DX-10's PR 10 has one — and the validator only checks that *some* bracket exists.
- §5.3's "proof policy upgrade/downgrade" classification has no `field` enum member in `semantic-diff.schema.json` and no intent field group (RFC 0031 folds it into `assurance`); the plan should say so or the enum should carry it.
- `intent-contract.schema.json` names the same three fields differently in its field groups vs its policy keys (`claims`/`properties`, `fault_model`/`faults`, `optimization.non_vacuity`/`non_vacuity`), defeating mechanical policy↔field joins.
- Handle classes with no schema pattern anywhere: `inb_*` (used in §3.1's example), `cap_*`, `defect_*`, `ps_*`, `dbg_*`, `cont_*`, `model_*`; `cir.schema.json` defines no `cir_` identity field; `proof-receipt.schema.json` uses unpatterned `receipt_id` so §4.4's `proof_*` class has no producer.
- `repair-transaction.schema.json` cannot represent three §8.1 contents (performance impact, security review, promotion policy verdict) that §8.5's review UX renders.
- `semantic-diff.schema.json:203` leaves `semantic_changes[].kind` an open string against RFC 0031's closed axis set; `policy.decision: "unknown"` has no defined blocking semantics.
- `evidence-graph-node` `parameterization` admits `{kind: proved_universal, n: 7}`; `redacted.schema.json` leaves `original_class` optional; `verification-task` lacks priority class, quota, and proof epoch; RFC 0031 names an assurance input and a per-change fragment that `semantic-diff` cannot carry.

**Proposed Change:**
Sweep these in one PR-0-adjacent commit; details routed per file in Appendix A (items A10–A15). Plan-side, only §5.3 needs a line.

**Rationale:** Each is small; collectively they are the gap between "schemas exist" and "schemas enforce."

**Benefits:** Validator-checkable joins; the §4.4 registry becomes fully anchored (the current handle-prefix check passes only because unanchored classes aren't asserted).

**Trade-offs:** None.

**Git-Diff:**
```diff
--- a/notes/plan/plan.md
+++ b/notes/plan/plan.md
@@ §5.3 @@
-- proof policy upgrade/downgrade;
+- proof policy upgrade/downgrade (represented in the diff lattice as an
+  `assurance` change per RFC 0031);
```

---

## Appendix A — Dossier-level fixes routed to files (no plan diff)

- **A1** `schemas/semantic-diff.schema.json`: `protected` → `const: true` (or delete and derive from `field` membership); close `semantic_changes[].kind` to RFC 0031's axis enum; define `policy.decision: "unknown"` as blocking or remove it; add per-change `fragment` and the RFC 0031 assurance input.
- **A2** `schemas/repair-transaction.schema.json`: remove `not_applicable` from promotable profiles (define it in RFC 0032 first if kept anywhere); add `phase-c`/`phase-d` conditionals per `rfcs/0032:48`; `uniqueItems` + twelve per-gate `contains`; add `performance_impact`, `security_review`, `policy_verdict` fields (§8.1); align gate-status spelling with `promotion-receipt` (pick one of `pass`/`passed`).
- **A3** `schemas/promotion-receipt.schema.json`: same gate-list identity enforcement; add before/after snapshots, coverage, unknowns, and policy decision per `rfcs/0032:64`; add proof/checker epochs (INV-014 — `proof-receipt` already does this right); align cost-ledger dimensions with the unified budget list (Change #11).
- **A4** `schemas/evidence-graph-node.schema.json`: conditionals requiring service identity on `sampled`/`bounded`/`validated`/`proved`; add `claim_id`, `previous_status`, `idempotency_key` (§11.7); make `parameterization` fail closed (`proved_universal` ⇒ `n: null`; `checked`/`cutoff_checked` ⇒ integer `n`).
- **A5** `schemas/evidence-graph-edge.schema.json`: require `checker` when `kind: CHECKED_BY`.
- **A6** `schemas/context-pack.schema.json`: replace the local five-dimension envelope with a `$ref` to the shared envelope `$def`; fix `engine_error`→`engine-error`; add the INV-008 reason conditional; add `semantic_epoch`/`content_hash` if the R3#7 fields are still absent after that pass.
- **A7** `schemas/assurance-result.schema.json`: demote `unsupported`/`engine-error` from verdicts to `inconclusive` reasons; make the dimension-level `unsupported` a typed `{reason}` object, not a free string.
- **A8** `schemas/redacted.schema.json` + six schemas lacking it: convert the three inlined copies to `$ref`; add to `promotion-receipt`, `repair-transaction`, `assurance-result`, `evidence-graph-node`, `verification-task`, `cir`; make `original_class` required; confirm the reason enum against docs/49's policy-denial case.
- **A9** `research/01`, `research/03`, `research/04`, `research/12`, `research/14`: add the criteria sections `research/README.md:3` mandates (research/01: ratify or reject the "≥10× explored classes" reading; research/26:115 corrected to quote research/01's actual words); `research/04`: state the preservation obligation for the full applied property class.
- **A10** `rfcs/0027`: split the `repair` and `task` registry rows so every operation has exactly one minimum authority level.
- **A11** `schemas/verification-task.schema.json`: suspended/BudgetExhausted continuation conditionals; add priority class, quota reference, proof epoch.
- **A12** `schemas/workspace-snapshot.schema.json`: extend to §4.2's ten components; remove the protocol epoch from identity.
- **A13** `schemas/intent-contract.schema.json`: unify the three field-naming vocabularies with the policy keys and `semantic-diff`; add `nondeterminism` and `abstraction_maps` to top-level `required` (locked-but-absent is a bypass); give `security_policy` required members; close `fault_model.enabled` to the §5.2 fault vocabulary; bind the `review` verb to a named-reviewer principal; let `assurance` reference the 13 evidence kinds.
- **A14** Handle-prefix registry completion: schema-anchored patterns for `inb_`, `cap_`, `defect_`, `ps_`, `dbg_`, `cont_`, `model_`; a `cir_` identity field; `proof_`-patterned ids in `proof-receipt`.
- **A15** `tools/validate_dossier.py`: implement the register quote-identity check (Change #3), the debt-ledger derivation (Change #2), the `program_status` field (Change #5), the phase-exit↔gate mapping (Change #14), the G0-closing-PR annotation check (DX-13→PR 2, DX-14→PR 6), and per-gate identity checks for the twelve-gate lists; regenerate `VALIDATION_REPORT.md` and `validation-results.json` in the same commit.
- **A16** `notes/G0_SPIKE_MATRIX.md:5`: "the four docs/34 acceptance workflows" → "the three human-executed docs/34 acceptance workflows (the fourth, agent repair, is covered by the G2 ACI ablation and ContinuumBench)".
- **A17** `docs/45`: mark the weak-memory diagnosis track blocked on the ADR-0032 lane (or remove it); add plan §19.4's "Continuum's own engine bugs" source symmetry (docs/45:87 has it; plan §19.4 lacks it — add to plan).
- **A18** `docs/34`: mark the explain-interaction row gate-normative (or the plan drops "both marked"); confirm the saturating-swarm condition is quoted in G5 (Change #14).

## Appendix B — Verified clean this pass (do not re-audit)

- §0.3 G0 counts (8/3/4 of 15) exactly match the matrix; re-homing targets match §22.
- §21 Phase C corpus arithmetic exact against `validated-examples.csv` (29 families; 14 P4; 3 P3).
- Phase B gate-profile split (gates 1–8, 11–12 → B; 10 → C; 9 → D) consistent with §8.2.
- G10 two-system criterion consistent across §21, §22, docs/52.
- §24.5 rows verified verbatim against sources: production conformance (research/05:144-152), cancellation calculus (research/09:152-161), lenses metric (research/31:92-93), context-compilation kill (research/32:100), sub-file kill half (research/27:108), observer-indexed promote half (docs/31:9), Forge kills (research/29:101-104, one cosmetic elision).
- R04, R06, R10, and Quint-question citations of docs/08 accurate (plan adds a kill signal to R10 that docs/08 lacks — an improvement, not drift).
- Grading order identical across plan §19.3, docs/45:122, RFC 0034:13, with the intent-integrity zero-cap present in both.
- Paid §25 items confirmed paid: intent-registry-record schema (status + chains), promotion-receipt cost ledger + gate profile, nine-dimension assurance-result with `resource-exhausted` absent from all schemas, typed `Failed` reasons, fail-closed diff conditional, RFC 0037 `intent.accept`/`intent.lock`/`revise-intent` + four projection kinds, docs/40/41/42/35/45 corrections, docs/04 banner.
- All §-references in §21/§22/§24.5/§25 resolve to real headings (including §4.2.1).
- `docs/53` evidence-boundary characterization in §0.3 accurate; C035 BLOCKED in docs/18 as cited.
