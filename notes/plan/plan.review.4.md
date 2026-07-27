# Review 4 — Continuum Revision 3 Master Implementation Plan

**Reviewed document:** `notes/plan/plan.md` (2512 lines) and the accompanying dossier
(docs/, rfcs/, adr/, schemas/, research/, notes/, tools/, spikes/, examples/)
**Review date:** 2026-07-26
**Method:** full read of plan.md; six targeted cross-audits of the accompanying
artifacts against the plan's checkable claims (execution layer, release gates and
DX docs, the seven load-bearing Rev-3 RFCs, all 18 JSON schemas plus examples,
Rev-3 docs 33–55 contract drift, and the §24.5 research-lane register against its
owning notes); dedup against reviews 1–3, whose changes are verified applied to
plan.md itself.

---

## Executive Summary

Reviews 1–3 did their job on plan.md: the phase/gate structure is coherent, the
intent lifecycle is complete, the operational contract (§4.5–4.7) exists, the
first-run contract and cost governance are specified, and the G0 matrix and
START_HERE reconciliations the plan promised are genuinely done (verified line by
line — see Appendix B). The plan is now a strong document. This review therefore
finds most of its defects not *in* the plan's prose but in the seams between the
plan and the dossier it governs — and in a handful of places where three rounds of
patching left the plan contradicting either itself or the artifacts it declares
normative.

### The five systemic problems of this revision

1. **Parts of the claimed reconciliation are fictional.** The plan asserts, in
   present tense, reconciliations that did not happen: §21 Phase F says "docs/52
   is updated to match" the two-system G10 criterion (it was not — and plan §22's
   own G10 bullet still carries the weaker one-system criterion, so the plan
   contradicts itself); §9.1 says the incremental-engine decision "is carried as
   a docs/08 risk" (no such risk row exists); §22 says the validator enforces
   "bullet-for-bullet correspondence" (it enforces a fuzzy 0.6 token overlap,
   compares G0 as two empty lists, and its PR-heading regex silently skips
   PR 4a/15a/25a — the exact inserts §25 exists to protect). The stale
   `VALIDATION_REPORT.md` predates five of the validator's eighteen checks,
   including all three that §25 advertises.

2. **The plan loses arguments with its own normative layer.** Plan §25 declares:
   where plan prose and RFC disagree, the RFC is corrected and becomes normative.
   By that rule the plan is currently wrong in three places: §5.4's policy verbs
   (`maintainer-review`, `proof-required`) are not members of RFC 0037's closed
   verb set and fail schema validation; §21's `NotYetEnforced` literal disagrees
   with the schema/RFC's `not_yet_enforced`; §5.3's diff guarantee enumerates six
   dimensions where G3 (plan §22 and docs/52) requires seven — fairness, "the
   canonical gaming vector" per RFC 0037, is the one missing.

3. **The §24.5 frontier-lane register is not yet the single lane authority it
   claims to be.** Row-by-row audit against the owning research notes found: two
   rows misquote their sources in ways that silently change the bar (research/01's
   "without a *serious* regression" became zero-regression; docs/31's "*checker*
   overhead" became "certificate overhead", colliding with a different row's
   different budget); research/05's third promotion criterion and second kill
   disjunct are dropped; six of thirteen rows carry no kill clause despite the
   register preamble requiring one; the lenses row omits its own draft marker and
   defers its numeric target to the note while the note defers it back to the
   register; the liveness row's ≥3× figure appears in no cited source; §8.3
   neighborhood adequacy — which §0.3 explicitly promises is "owned by a research
   lane with kill criteria (see §24.5)" — has no register row at all; and three
   docs/31 lanes (sheaf gluing, nominal/orbit-finite, TLA+ importer) with full
   promote/kill pairs are covered nowhere.

4. **The machine contracts cannot represent the plan's headline guarantees.**
   B11's assurance envelope — the plan's single most-repeated promise — is
   unrepresentable in `assurance-result.schema.json` (five of nine dimensions
   missing, no producing-engine field, no typed `Unsupported(reason)`), and that
   schema's `resource-exhausted` *verdict* directly contradicts §11.4's "Budget
   exhaustion is never a verdict." The `Proposed`-vs-accepted intent bit — the
   single most security-relevant state in the system — exists in no schema.
   `Redacted(reason, commitment)` is undefined dossier-wide. The §8.6 cost ledger
   has no field and the gate-12 promotion receipt has no schema. INV-008's typed
   inconclusive reasons are optional properties a bare `inconclusive` node can
   simply omit. The abstraction-map gaming vector (§5.1's "mapping two concrete
   values to one abstract value") has no intent field, no diff field, and no lock
   — the G3 anti-gaming path RFC 0032 names is structurally unrepresentable.

5. **Whole capability families the product story depends on have no operations,
   owner, or lifecycle.** §0.2 declares `observe` a universal operation and
   Phase C/F ship observation lanes, but §10.2/RFC 0027 define no observe
   operations; `cargo continuum bind` and the §16 correspondence graph have no
   correspondence operations; §4.2.1's signature chains and signed receipts have
   no key-management story; the multi-agent evidence graph has no concurrency
   model (the spike explicitly excluded it); §21.1's own rule renders every phase
   `BLOCKED` because no phase names an owner; and the workbench that enforces
   B19's cancellation-correctness bet is itself built on asupersync with no plan
   to ever point Continuum at `continuumd`.

### What the plan does well (verified this pass)

The G0 matrix, START_HERE PR sequence, docs/34 latency table (gate-normative
marking, explain/reverse-step/branch-fork rows), docs/48's preregistration role,
docs/08 R04/R06/R10, docs/09's incident policy, RFC 0027's operation registry
(1:1 with §10.2), the error taxonomy (15/15 across all RFCs, no orphans), RFC
0032's twelve gates, RFC 0034's grading order, docs/33 and docs/44's
reconciliation, the Tribunal rename, and all sixteen schema examples check out
cleanly. Appendix B has the full list.

---

## Proposed Changes

### [CRITICAL] Change #1: Resolve the G10 one-system/two-system contradiction

**Current State:**
Three statements disagree about the 1.0 bar. Plan §21 Phase F exit
(plan.md:2126–2130) requires Continuum to replace bespoke DST plus the separate
TLA+ workflow "in at least two materially different real systems" and claims
"docs/52 is updated to match, per §22's reconciliation rule." But plan §22's own
G10 bullet (plan.md:2295–2296) still reads "at least one migrated project stops
requiring a separate TLA+ workflow", docs/52:110 carries the same weak bullet,
and docs/52:118–119 explicitly *documents the divergence* instead of adopting the
stricter criterion — while mis-citing it as "the second bullet" (it is the
third). The §22 phase↔gate table says Phase F closes G10, so Phase F's exit is
strictly stronger than the gate it closes, inside one document. The
bidirectional validator cannot catch this: §22 and docs/52 agree with each other
and jointly disagree with §21.

**Proposed Change:**
Adopt the stricter two-system criterion in all three places in one commit: plan
§22 G10, docs/52 G10 (and delete docs/52's divergence note), and reword the §21
parenthetical so it no longer asserts a completed update.

**Rationale:**
The plan's release-blocker doctrine makes misleading gate statements a blocker
class. A 1.0 gate that reads differently in the plan's §21, the plan's §22, and
the normative gate document is exactly the ambiguity the reconciliation rule
exists to prevent — and this instance is invisible to the enforcement tooling.

**Benefits:**
- One unambiguous 1.0 bar.
- Removes a false present-tense claim from the plan.
- Fixes the second/third-bullet mis-citation as a side effect.

**Trade-offs:**
- The stricter criterion raises the 1.0 bar; if that is not intended, the same
  edit applied in the other direction (weaken §21) is equally consistent — but
  §21's criterion was chosen deliberately ("deliberately stricter"), so
  strengthening the gate is the faithful resolution.

**Implementation Notes:**
docs/52 edit in Appendix A (A1). The dossier validator upgrade in Change #11
should add the §21-exit-vs-§22-gate cross-check that would have caught this.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ -2124,9 +2124,8 @@
 Exit: Continuum replaces bespoke DST plus separate TLA+ workflow in at
-least two materially different real systems. (This is deliberately
-stricter than docs/52 G10's "at least one stops requiring a separate
-TLA+ workflow"; docs/52 is updated to match, per §22's reconciliation
-rule.)
+least two materially different real systems. (§22 G10 and docs/52 G10
+carry this same two-system criterion; all three statements are
+reconciled in one commit per §22's reconciliation rule.)
@@ -2293,9 +2292,9 @@
 - two real project migrations;
 - two materially different real projects remove bespoke DST
   infrastructure;
-- at least one migrated project stops requiring a separate TLA+ workflow
-  for normal development;
+- at least two migrated projects stop requiring a separate TLA+
+  workflow for normal development;
 - production evidence returns valid pass/fail/inconclusive
   classifications (INV-008);
```

---

### [CRITICAL] Change #2: Rewrite §5.4's policy locks in the closed verb vocabulary — and lock the two unlockable fields

**Current State:**
Plan §5.4's TOML (plan.md:776–790) assigns `maintainer-review` to six fields and
`proof-required` to observers. RFC 0037:53–62 defines a *closed* eight-verb set
(`unlocked | proposal-only | review | locked | no-decrease | no-removal |
no-downgrade | no-expansion`), explicitly rejecting free-form policy strings
("directional enforcement requires a closed verb set"), and
`schemas/intent-contract.schema.json` `$defs/policy_verb` matches the RFC. Seven
of the plan's twelve assignments therefore fail schema validation. Per the
plan's own rule (plan.md:2437: "Where plan prose and RFC disagree, the RFC is
corrected and becomes normative"), the plan's example is wrong. Separately, the
audit found `scope` and `nondeterminism` are diffable fields
(`semantic-diff.schema.json` field enum) that RFC 0031:42 says are "any change
is a semantic change" — yet neither appears in the twelve-field protected set,
so a nondeterminism-class change (`demonic` → `angelic`, a textbook gaming move)
classifies and then attaches to no lock.

**Proposed Change:**
Rewrite the §5.4 TOML using only closed-set verbs; add `scope` and
`nondeterminism` rows; note that if a proof-gated acceptance verb is genuinely
wanted, it must be added to RFC 0037's closed set by RFC revision, not by plan
prose. Extend RFC 0037's protected set and the schema's `policy` object to
fourteen fields (Appendix A2).

**Rationale:**
The policy-lock table is the G3 enforcement surface. An example that cannot
validate teaches integrators the wrong vocabulary, and two semantically
privileged fields with no expressible lock are a promotion loophole shape: the
diff marks the change protected, and no policy can say what to do about it.

**Benefits:**
- The plan's flagship configuration example becomes schema-valid.
- Closes the scope/nondeterminism lock gap the schema audit exposed.
- Forces the proof-required question to be answered where it belongs (RFC).

**Trade-offs:**
- `review` is weaker than a hypothetical `proof-required`; if observer changes
  truly need proof-gated acceptance, the RFC revision must land before G3.

**Implementation Notes:**
`maintainer-review` → `review` throughout; `proof-required` → `review` with an
inline comment referencing the open RFC question. Schema/RFC changes in A2.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ -774,18 +774,22 @@
 ```toml
 [intent.policy]
-properties = "maintainer-review"
-assumptions = "maintainer-review"
-fairness = "maintainer-review"
+properties = "review"
+assumptions = "review"
+fairness = "review"
 trust_boundaries = "no-expansion"
 non_vacuity = "no-removal"
-completion_policy = "maintainer-review"
+completion_policy = "review"
 bounds = "no-decrease"
 faults = "no-removal"
 assurance = "no-downgrade"
-observers = "proof-required"
-security_policy = "maintainer-review"
-optimization = "maintainer-review"
+observers = "review"   # a proof-gated verb requires an RFC 0037 revision
+security_policy = "review"
+optimization = "review"
+scope = "review"
+nondeterminism = "review"
 ```
+
+Verbs are drawn from RFC 0037's closed set; free-form policy strings are
+rejected. `scope` and `nondeterminism` are protected: RFC 0031 classifies
+any change to them as a semantic change, so both must be lockable.
```

---

### [CRITICAL] Change #3: Repair the §24.5 frontier-lane register — restore the misquoted bars, add the missing lanes, and define ratification

**Current State:**
Row-by-row audit against the owning notes found the register systematically
lossy (full detail in the audit summary above): research/01's "without a
*serious* regression" tightened to zero-regression; docs/31's "checker overhead
<20%" renamed "certificate overhead", colliding with the separate
certificate-overhead row's ≤10% budget; research/05's third promotion bullet
(≥2× reduction in surviving symmetry orbits per synthesized probe set) and
"clock uncertainty" condition and second kill disjunct all dropped; research/29's
four kill criteria reduced to a paraphrase of one; research/27's kill narrowed
("file/module" → "module") and its "clean-parity at sampled rate" threshold
appearing in no note; the lenses row missing the draft marker research/31
carries, with the numeric target circularly deferred (note → §24.5 → note); the
liveness row's ≥3× unsourced (research/04's actual gate is
proven-preserving-or-disabled); six of thirteen rows carrying no kill clause
despite the preamble requiring one; §8.3 neighborhood adequacy having no row at
all despite §0.3's promise; three docs/31 lanes (sheaf, nominal/orbit-finite,
TLA+ importer) uncovered; and "draft, pending ratification" being vocabulary no
document defines.

**Proposed Change:**
Replace the register table with a corrected version that (a) quotes each note's
threshold and kill faithfully, (b) marks every unratified number `draft`,
(c) adds rows for neighborhood adequacy (owner: research/33, currently uncited
by the plan), nominal/orbit-finite (research/14), sheaf gluing (research/03),
and the TLA+ importer, (d) defines ratification, and (e) trims the closing
merge-list to the lanes still unmerged, restoring the two preconditions it
currently drops (cubical "with a preservation theorem"; semiring "three
production analyses share code").

**Rationale:**
The register's whole purpose is to be the single authority no plan section may
bypass. A register that silently strengthens one bar, weakens another, drops
kill criteria, and omits a promised lane is worse than no register: it launders
drift into apparent authority. The neighborhood-adequacy gap is the sharpest
instance — §8.3 is the mechanism that makes repair promotion trustworthy, §0.3
labels it HYPOTHESIS with a promised lane, and no lane owns it.

**Benefits:**
- Restores the note↔register quote fidelity the closing paragraph demands of
  docs/31 ("where docs/31 and a research note disagree, the note is corrected
  and cited") — applied to the register itself.
- Every row gains a kill clause; every unratified number is visibly draft.
- §0.3's HYPOTHESIS list becomes fully backed.

**Trade-offs:**
- More rows marked draft makes Phase A's "no unratified threshold past Phase A"
  rule bind harder. That is the rule working as designed.

**Implementation Notes:**
Ratification definition doubles as a validator check (Change #11): the register
row must quote the note verbatim once ratified. Research-note-side fixes
(research/13 has no kill-criteria section at all; register cites notes in
shorthand `/13` that greps cannot resolve) in Appendix A3.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ -2394,17 +2394,34 @@
-| Capability | Lane | Threshold / kill | Fallback |
-|---|---|---|---|
-| General context compilation (§6) | research/25, /32 | pack-ablated agent benchmark win; kill if packs induce wrong repairs | plain causal slice + expansion |
-| Exploration reduction (§9, INV-013) | research/01; docs/31 | ≥10× (order of magnitude) reduction in explored classes on a non-artificial corpus subset **without regression on dependent workloads** (research/01); observer-indexed: median ≥5× on the observer-sensitive class, certificate overhead <20%, zero mutation loss (docs/31) | conservative unreduced exploration |
-| Liveness-preserving reduction (§7.2, Phase D) | research/04, /13 — lane to be opened | fair-cycle-preserving reduction ≥3× on the liveness corpus subset with zero missed accepting cycles vs unreduced SCC baseline | unreduced liveness with explicit cost banner; batch expectations stated in the Phase D exit |
-| Causal minimization (§6, §12) | research/26 | replay-preserving core ≤10% of trace length on real (non-synthetic) failures — draft, pending ratification; kill if minimization cost dominates verification | 1-minimal delta debugging only |
-| Sub-file incremental trust (§9) | research/27 | clean-parity at sampled rate; kill if capture untrustworthy below module granularity | module-granularity invalidation |
-| Forge co-synthesis + QD (§14) | research/29, /30 | rediscovery suite; kill if joint search loses to staged | staged synthesis; Pareto archive only |
-| Production conformance (Phase F) | research/05 | monitorability-gated; draft pending ratification: faithful Lab reproduction of ≥70% of curated known incidents under injected telemetry loss; always-on overhead ≤1%; kill if most target properties are monitorable only as `Inconclusive` on two real systems | Lab-replay evidence only |
-| Cancellation calculus (§0, B19) | research/09 | draft pending ratification: 10/10 mutation-corpus mutants detected with zero false alarms on the correct implementation; machine-checked drain ranking for the replicated register | runtime checking only |
-| Bidirectional lenses (§16) | research/31 | ambiguity rate := fraction of abstract edits on the drift corpus yielding multiple or no concrete candidates (metric defined in research/31; the drift corpus remains the lane deliverable); kill if most real mappings are too ambiguous, or users mistake candidate synchronization for verified preservation | get-only projection + drift detection |
-| Proof repair (§15.4) | research/28 | vs source/LSP loop baseline; kill if repair proposes weakening | context packs + human proof work |
-| Certificate overhead | docs/31 | checking ≤10% of search time, or acceptable asynchronous CI latency (docs/31's full rule) | reduce certified-lane scope |
-| Weak-memory lane (B11) | ADR-0032 — lane to be opened | reproduces the standard litmus corpus under the declared model before any envelope upgrade; until then every memory dimension reads `Unsupported(sequential-consistency-only)` | SC-only, declared as a 1.0 non-goal |
-| Timed/probabilistic semantics | ADR-0016 — lane to be opened | per-ADR staging; until shipped, timing fields in Intent Contracts remain declarative assumptions (B11) | declarative assumptions only |
+A lane is **ratified** when its owner fixes the numeric threshold in the
+research note and this register quotes it verbatim; the dossier validator
+checks register↔note quote identity. Until then a row is **draft** and
+blocks its lane's promotion.
+
+| Capability | Lane | Threshold / kill | Fallback |
+|---|---|---|---|
+| General context compilation (§6) | research/25, /32, /33 | ablation design per research/25 (raw trace vs pack on the agent benchmark); win margin fixed at ratification — draft; kill (research/32): compact packs repeatedly induce incorrect repairs despite preservation checks | plain causal slice + expansion |
+| Exploration reduction (§9, INV-013) | research/01; docs/31 | research/01 (stated there as a kill): ≥10× reduction in explored classes on a non-artificial corpus subset without a *serious* regression on dependent workloads; observer-indexed (docs/31): median ≥5× on the observer-sensitive class, *checker* overhead <20% (distinct from the certificate-overhead row), zero mutation loss | conservative unreduced exploration |
+| Liveness-preserving reduction (§7.2, Phase D) | research/04, /13 — lane to be opened | soundness gate per research/04: property-directed reduction is proven fair-cycle-preserving or disabled; speedup target and liveness corpus subset fixed at lane opening — draft | unreduced liveness with explicit cost banner; batch expectations stated in the Phase D exit |
+| Causal minimization (§6, §12) | research/26 | replay-preserving core ≤10% of trace length on real (non-synthetic) failures — draft, pending ratification; kill if minimization cost dominates verification | 1-minimal delta debugging only |
+| Neighborhood adequacy (§8.3) | research/33 — lane to be opened | hidden-variant catch rate of the §8.3 neighborhood on the docs/50 gaming corpus; target fixed at lane opening — draft; kill (research/33): hidden variants are too easy to leak or too hard to grade independently | fixed strategy-list neighborhood with per-receipt coverage disclosure and no adequacy claim |
+| Sub-file incremental trust (§9) | research/27 | clean-mismatch rate at the §9.5 sampled audit rate (rate derives from §8.6's confidence target) — draft; kill if dependency capture cannot be made trustworthy below file/module granularity | module-granularity invalidation |
+| Forge co-synthesis + QD (§14) | research/29, /30 | rediscovery suite (named algorithms and count fixed at ratification — draft); kills (research/29): joint space overwhelms coupled-feedback gains; proof-complexity objective biases toward trivial designs; abstractions overfit finite bounds; agent proposals cannot be reproduced by structured search | staged synthesis; Pareto archive only |
+| Production conformance (Phase F) | research/05, /36 | draft pending ratification: faithful Lab reproduction of ≥70% of curated known incidents under injected telemetry loss and clock uncertainty; always-on overhead ≤1%; ≥2× reduction in surviving symmetry orbits per synthesized probe set; kill if most target properties are monitorable only as `Inconclusive` on two real systems, or the overhead budgets cannot be met | Lab-replay evidence only |
+| Cancellation calculus (§0, B19) | research/09 | draft pending ratification: 10/10 mutation-corpus mutants detected with zero false alarms on the correct implementation; machine-checked drain ranking for the replicated register; kill (research/09): invariant annotations become pervasive in real code, or the calculus cannot express asupersync's actual cancellation semantics | runtime checking only |
+| Bidirectional lenses (§16) | research/31 | ambiguity rate := fraction of abstract edits on the drift corpus yielding multiple or no concrete candidates (metric per research/31; the drift corpus is the lane deliverable; the numeric target is fixed in the note at ratification and quoted here) — draft; kill if most real mappings are too ambiguous, or users mistake candidate synchronization for verified preservation (measured under the G8 instruments) | get-only projection + drift detection |
+| Proof repair (§15.4) | research/28 | vs source/LSP loop baseline (qualitative; margin fixed at ratification — draft); kill if repair proposes weakening | context packs + human proof work |
+| Certificate overhead | docs/31 | checking ≤10% of search time, or acceptable asynchronous CI latency (docs/31's full rule); on failure the fallback is applied per lane | reduce certified-lane scope |
+| Nominal / orbit-finite verification | research/14; docs/31 | docs/31's stated promote/kill pair (unbounded session/request protocol with tractable orbit growth and Lean-proved equivariance) | finite-bounds checking only |
+| Sheaf-based composition | research/03; docs/31 | docs/31's stated promote/kill pair | monolithic composition proofs |
+| Full TLA+ source importer | docs/31; ADR-0029 | docs/31's stated promote/defer pair; not a 1.0 goal (ADR-0029) | manual semantic porting per docs/32 |
+| Weak-memory lane (B11) | ADR-0032 — lane to be opened | reproduces the standard litmus corpus under the declared model before any envelope upgrade; until then every memory dimension reads `Unsupported(sequential-consistency-only)` | SC-only, declared as a 1.0 non-goal |
+| Timed/probabilistic semantics | ADR-0016 — lane to be opened | per-ADR staging; until shipped, timing fields in Intent Contracts remain declarative assumptions (B11) | declarative assumptions only |
@@ -2412,8 +2429,8 @@
 A register row may not carry an unratified or `TBD` threshold past
 Phase A; such a row blocks its lane's promotion. docs/31's remaining
-quantitative thresholds (cubical reduction ≥3× on ≥2 real protocol
-classes; semiring within 15% of specialized analyses; assumption
-synthesis on ≥5 liveness cases; abstraction discovery) are merged into
+quantitative thresholds (cubical reduction ≥3× on ≥2 real protocol
+classes *with a preservation theorem*; semiring within 15% of
+specialized analyses *with three production analyses sharing code*;
+assumption synthesis on ≥5 liveness cases; abstraction discovery) are
+merged into
 this register during the §25 pass so the program has exactly one lane
 authority; where docs/31 and a research note disagree, the note is
 corrected and cited.
```

---

### [CRITICAL] Change #4: Align §5.3 with the normative diff lattice — seven dimensions, the `unknown`/`unsupported` split, and `incomparable` fail-closed

**Current State:**
Three defects in one section. (a) Plan §5.3's guarantee sentence
(plan.md:767–771) enumerates six privileged-classification dimensions; G3 —
in both plan §22 (plan.md:2213–2216) and docs/52:45 — requires seven, and the
missing one is **fairness**, which RFC 0037:34 calls "the canonical gaming
vector (docs/50)". (b) Plan §5.3 uses a single `Unknown`; RFC 0031:47 and the
schema normatively split `unknown` (undecidable/unattempted) from `unsupported`
(outside declared fragments) — a distinction INV-008 requires — and use
lowercase literals. (c) RFC 0031 and docs/40 introduce `incomparable` as a
classification; RFC 0031:36 blocks it pending review for bounds only, and
docs/40 gives it no blocking status at all — the gap between "not proven
weakened" and "blocked" is a promotion-loophole shape. Additionally, the schema
audit showed the fail-closed rule is documented but structurally unenforced: a
semantic diff with an `unknown` protected change and `decision: "allow"`
validates.

**Proposed Change:**
Rewrite the §5.3 guarantee to name all seven dimensions, adopt the
`unknown`/`unsupported` split with the RFC's literals, and extend the
fail-closed rule to `incomparable` on any protected field. Schema-side `if/then`
enforcement and the docs/40 rewrite in Appendix A4.

**Rationale:**
§5.3 is the sentence G3 is graded against. A guarantee that omits the canonical
gaming vector, and a third classification value with undefined promotion
consequences, are exactly the "formally sophisticated reward hacking" §5.1
catalogues — created by the tool's own spec this time.

**Benefits:**
- The G3 hidden-suite grading and §5.3's guarantee become the same predicate.
- Every non-affirmative classification (`unknown`, `unsupported`,
  `incomparable`) has a defined, closed consequence.
- Literal-level agreement with the schema layer.

**Trade-offs:**
- None of substance; `incomparable`-blocks-pending-review may generate more
  review load, which is the intended safe default.

**Implementation Notes:**
Keep §5.3's prose `Unknown` capitalization only where speaking informally;
literals in code font use the schema's lowercase forms.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ -757,15 +757,19 @@
 Where implication is decidable or solver-checkable, Continuum proves the
-direction. Otherwise it emits a proof obligation or `Unknown` rather than
-guessing. `Unknown` fails closed: an intent change classified `Unknown`
-blocks ordinary promotion exactly as a confirmed privileged change does,
-pending review.
+direction. Otherwise it emits a proof obligation, `unknown`
+(undecidable or unattempted), or `unsupported` (outside declared
+fragments) rather than guessing — the two are distinct per INV-008.
+All non-affirmative classifications fail closed: an intent change
+classified `unknown`, `unsupported`, or `incomparable` on a protected
+field blocks ordinary promotion exactly as a confirmed privileged
+change does, pending review. The semantic-diff schema enforces this
+structurally, not only in prose.
 
 Each intent field declares its semantic fragment (ADR-0025:
 `Finite / Symbolic / Temporal / Probabilistic / Theorem / Runtime`), so
 "supported" is a checkable predicate. The diff guarantee is: within
 declared supported fragments, every property weakening, assumption
-strengthening, bound decrease, observer coarsening, fault removal, and
-assurance downgrade is classified as privileged; outside them, `Unknown`.
+strengthening, bound decrease, observer coarsening, fault removal,
+fairness addition or removal, and assurance downgrade is classified as
+privileged — the seven G3 dimensions; outside them, `unsupported`.
```

---

### [HIGH] Change #5: Give the abstraction map an intent field — the `merged`/`split` gaming vector is currently unrepresentable

**Current State:**
§5.1 names "mapping two concrete values to one abstract value" as a canonical
gaming move (plan.md:698). §5.3 promises to classify "abstraction merge/split"
(plan.md:755). RFC 0031:27 defines `merged | split` "(abstraction maps)", and
RFC 0032:79 makes it load-bearing for G3 ("abstraction gaming … RFC 0031
classifies the abstraction map `merged`"). But §5.2's intent structure
(plan.md:708–742) contains **no abstraction-map field** — only `scope`'s
free-string abstraction level — and consequently neither
`intent-contract.schema.json` nor `semantic-diff.schema.json`'s field enum has
one. The named anti-gaming path has nothing to attach to, at any layer.

**Proposed Change:**
Add an `abstraction maps` group to §5.2's intent structure (referencing the §16
correspondence graph for the maps' content, with the intent contract binding
their identities) and add it to the §5.4 lock table. Schema/RFC propagation in
Appendix A5.

**Rationale:**
This is a plan-side hole, not just a schema bug: the plan promises
classification of changes to an object its own intent structure never declares.
Binding map *identities* in intent (with content in the correspondence graph)
keeps §5.2 lean while making merge/split diffs well-defined.

**Benefits:**
- The §5.1 gaming vector, §5.3 classification, RFC 0032 G3 path, and schema all
  gain a common anchor.
- Abstraction changes become lockable like every other protected field.

**Trade-offs:**
- Intent contracts grow a field whose content lives elsewhere; the
  identity-reference pattern (already used for domain packs) handles this.

**Implementation Notes:**
Mirror the domain-pack pattern: content-addressed identity in intent, content in
its own artifact class.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ -724,6 +724,9 @@
 observers
   state/event/knowledge/security projections
+abstraction maps
+  content identities of the §16 correspondence/abstraction maps the
+  claims are stated against; merge/split changes are privileged (§5.3)
 scope
   model/program components, abstraction level, and semantic fragment
   declarations (ADR-0025)
```
(plus one row `abstraction_maps = "review"` in the Change #2 lock table.)

---

### [HIGH] Change #6: Fix §9.1's phantom docs/08 risk row

**Current State:**
Plan §9.1 (plan.md:1048–1052) states the incremental-engine build-vs-adopt
decision "is carried as a docs/08 risk with the mitigation 'edge classes
collapse to Conservative' named as the failure mode that destroys
interactivity." docs/08 contains exactly R01–R20; none concerns the incremental
engine, and a dossier-wide grep for the quoted mitigation phrase hits only
plan.md itself. The claim is false in present tense.

**Proposed Change:**
Add risk row R21 to docs/08 (Appendix A6) and adjust the plan sentence to cite
it by number, so the claim is checkable.

**Rationale:**
Review 3 mandated this risk row (its Change #3); the plan text was applied but
the docs/08 half was not. The plan's own claim-discipline (§0.3, README claim
lattice) makes an unbacked present-tense citation the kind of drift the
validator should catch — a claim-citation check is added in Change #11.

**Benefits:**
- Restores truthfulness of a load-bearing engineering-risk statement.
- The Phase B ADR gains its recorded risk anchor.

**Trade-offs:** None.

**Implementation Notes:**
R21 content: risk = chosen memoization substrate cannot support the four reuse
classes with precise invalidation; failure mode = edge classes collapse to
Conservative, destroying interactivity (G5); controls = Phase B spike +
ADR, `query.explain_invalidation` precision baseline, §9.5 audit; kill signal =
Conservative-collapse on the reference workload.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ -1048,7 +1048,7 @@
 The engine itself is engineering risk, not settled design: whether it is
 derived from an existing memoization framework or built custom is decided
 by a Phase B ADR with spike evidence (§21), and the decision is carried
-as a docs/08 risk with the mitigation "edge classes collapse to
+as docs/08 risk R21, with the mitigation "edge classes collapse to
 Conservative" named as the failure mode that destroys interactivity.
```

---

### [HIGH] Change #7: Scope the G8 study to workflows humans can execute — and give G5's evidence-query target a real referent

**Current State:**
Two gate-referent defects. (a) Plan §21.1 (plan.md:1969–1976) defines the G8
study over "the four docs/34 acceptance workflows" with two *human* cohorts —
but one of the four (docs/34:139–141, "Agent repair") is explicitly
agent-executed ("An agent can diagnose and propose a repair using only Context
Pack and expansion operations"). A two-human-cohort study cannot cover it as
written. docs/34 also never marks the four workflows as G8-normative the way it
marks the latency table G5-normative. (b) Plan §22 G5 (plan.md:2243–2244) and
docs/52 both require "evidence queries and context compilation meet the docs/34
targets at ≥10^7 evidence nodes" — but docs/34's table contains no
evidence-query row and no scale qualifier anywhere; half the gate bullet points
at a target that does not exist.

**Proposed Change:**
Amend §21.1 to scope the human study to the three human-executed workflows and
route the agent-repair workflow to the G2/ContinuumBench ablations, which
already measure it. Add the evidence-query-at-10^7-nodes row to docs/34 and mark
the four workflows G8-normative (Appendix A7).

**Rationale:**
G8 is a preregistered study; a preregistration whose scope is impossible as
stated will be discovered at the worst time (Phase E authoring). G5's dangling
referent makes half a gate bullet unfalsifiable.

**Benefits:**
- The G8 preregistration becomes executable as specified.
- G5 becomes fully measurable against the normative table.

**Trade-offs:**
- The agent-repair workflow loses "human study" coverage it never really had;
  its coverage now lives where the measurement machinery exists.

**Implementation Notes:**
docs/48's skeleton already matches the two-cohort human design; only the
"four workflows" phrase needs the split.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ -1968,10 +1968,12 @@
 The G8 usability gate is defined against a preregistered
-minimal study: the four docs/34 acceptance workflows, two cohorts (Rust
-newcomers completing the deterministic/causal workflow; distributed-
-systems experts diagnosing real failures), a raw-trace baseline
+minimal study: the three human-executed docs/34 acceptance workflows
+(new model; existing Rust system; review), two cohorts (Rust newcomers
+completing the deterministic/causal workflow; distributed-systems
+experts diagnosing real failures), a raw-trace baseline
 comparator, and cohort sizes, instruments, and pass thresholds fixed in
 a preregistration document — an expanded docs/48 — published before the
-study runs. Authoring that document is a Phase E deliverable; the study
-itself runs in Phase F.
+study runs. The fourth docs/34 workflow (agent repair) is covered by the
+G2 ACI ablation and ContinuumBench, not the human study. Authoring the
+preregistration is a Phase E deliverable; the study itself runs in
+Phase F.
```

---

### [HIGH] Change #8: Register the missing operation families — `observe.*` and `correspondence.*`

**Current State:**
§0.2 lists `observe` among the universal operations (plan.md:146); Phase C
ships a read-only tokio observation lane and Phase F ships production
partial-order evidence; §3.3 shows `cargo continuum bind` and §16 defines the
correspondence graph with generated/inferred/handwritten links and drift
detection. Yet §10.2's operation registry (plan.md:1158–1176) — which RFC 0027
mirrors 1:1 — contains no operation for ingesting or classifying observations
and none for creating, inspecting, or drift-checking correspondence. Two
delivered product surfaces have no protocol surface.

**Proposed Change:**
Add `correspondence.bind / status / drift` and `observe.ingest / classify /
result` to §10.2, and the same rows to RFC 0027's registry with authority
levels (observe ingestion requires the production-trace capability per §18.2;
Appendix A8).

**Rationale:**
INV-002 ("no hidden semantic state") and B3 (all clients are projections of the
daemon) mean a capability without protocol operations can only be delivered as
out-of-band tooling — precisely what the architecture forbids. Registering the
operations now also forces the observation lane's artifact classes and
capability requirements to be designed before Phase C, not discovered during it.

**Benefits:**
- The Phase C read-only lane and Phase F production evidence get a home in the
  ACI from day one.
- `cargo continuum bind` stops being a CLI-only verb with no native operation.

**Trade-offs:**
- Registry grows before the subsystems ship; RFC 0027's authority-level and
  capability columns handle "registered but not yet served" cleanly
  (`UnsupportedSemanticFeature` is already a typed error).

**Implementation Notes:**
Keep observe result artifacts in the existing verdict vocabulary (INV-008 typed
inconclusiveness for insufficient telemetry is already defined).

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ -1166,10 +1166,12 @@
 refinement.check / explain
 proof.goal / attempt / check / slice
+correspondence.bind / status / drift
 debug.open / state / enabled / step_event / step_abstract / reverse_causal /
   branch / compare / why_enabled / why_blocked / export
 context.compile / expand
 failure.explain / minimize / branch
 repair.begin / apply / attach / evaluate / resume / review / promote / reject
+observe.ingest / classify / result
 forge.create / step / archive / materialize
 benchmark.run
```

---

### [HIGH] Change #9: Self-application — Continuum verifies `continuumd`

**Current State:**
`continuumd` "runs its own work as cancel-correct asupersync regions" (§4.1),
G1 demands crash recovery with no stale index entries, INV-017 demands
transactional publication, and B19 declares cancellation correctness of
verification tasks a design bet. The daemon is, structurally, exactly the class
of concurrent system Continuum exists to verify — and the plan never points
Continuum at it. The trust spine's concurrency correctness rests entirely on
conventional testing (docs/35's "Reliability" section is a test-technique
list).

**Proposed Change:**
Add a Phase D deliverable: a CML model of `continuumd`'s task/continuation
lifecycle and INV-017 publication protocol, bound to the implementation through
the standard §16 correspondence machinery, checked in CI from Phase D onward;
a daemon concurrency defect found this way is recorded as G4-class evidence of
product value.

**Rationale:**
Three birds: (1) it directly de-risks G1's hardest criteria (crash recovery,
cancellation, atomic publication) with the strongest tool available; (2) it is
the cheapest possible "real system migration" rehearsal — if Continuum cannot
model its own daemon, the G10 migration criterion is in trouble and the team
learns it years early; (3) it is the most credible demo the product can have.
The plan's own doctrine ("easier to state intent than bury assumptions")
should apply to the workbench first.

**Benefits:**
- Continuous, adversarial dogfooding of the exact features real adopters need.
- An honest early-warning signal for the correspondence-fragility kill
  criterion (§24: "model/program correspondence remains mostly manual and
  fragile").

**Trade-offs:**
- Real engineering cost in Phase D; scoped to the task-lifecycle/publication
  fragment (Finite), not the whole daemon, to keep it tractable.

**Implementation Notes:**
Phase D is the right slot: the asupersync adapter (Phase B), CML (Phase B/C),
and incremental machinery (Phase C) all exist by then. Not a gate criterion —
a deliverable — so a modeling impasse surfaces as a risk, not a release block.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ -2076,6 +2076,11 @@
 - refinement receipts;
+- self-application: a CML model of `continuumd`'s task/continuation
+  lifecycle and INV-017 publication protocol, bound to the daemon
+  implementation via the §16 correspondence machinery and checked in CI
+  from Phase D onward; a daemon concurrency defect found this way is
+  recorded as G4-class evidence of product value;
 - Wave 0/1 families raised from the Phase C P2 cap to their full
   required parity (P3/P4);
```

---

### [HIGH] Change #10: Truth in tooling — upgrade the validator to what §22/§25 claim, and regenerate the stale validation artifacts

**Current State:**
The execution-layer audit found the enforcement §25 advertises materially weaker
than claimed: (a) `check_gate_scheme_correspondence` is a fuzzy ≥0.6
token-overlap match, compares only `- ` bullets (the phase↔gate tables and all
G0 prose go uncompared — G0 is compared as two empty lists), yet plan §22
claims "bullet-for-bullet correspondence"; (b) the PR-heading regex
`^### PR (\d+)\b` silently skips PR 4a/15a/25a — exactly the three inserts §25
names as the point of the reconciliation — and the gate-annotation regex accepts
`[G99]`; (c) `check_gate_citation_hygiene` flags only the retired
`-Corpus`/`-Proof` suffixes, while bare Rev-2 gate citations sit unflagged and
unbannered in `rfcs/0004`, `0006`, `0010`, `0018` (`Target gate:` metadata),
docs/08, and adr/0011; (d) `VALIDATION_REPORT.md`/`validation-results.json`
predate five of the validator's eighteen checks — including all three §25
advertises — so the advertised enforcement has no recorded passing result; file
counts are stale (38→40 JSON, 211→214 MD); the `assertions: 30` figure and
MANIFEST's generation date are hardcoded literals; the `empty_files` metric
reports total files under an "empty" key. (e) §0.3/§22 claim the G0 counts "are
derived from the matrix, not asserted beside it" — no check derives them.

**Proposed Change:**
Add a §25 execution bullet mandating the validator upgrade and regeneration of
the validation artifacts; itemized validator fixes in Appendix A9. Soften no
claims — make them true instead.

**Rationale:**
The dossier's central conceit is that claims carry checkable evidence. Its own
meta-layer currently fails that standard: three advertised checks have never
recorded a pass, and two validator bugs would silently wave through regressions
in the exact places prior reviews fixed. This is the cheapest CRITICAL-adjacent
fix in the review — a few regexes, a parser, and a re-run.

**Benefits:**
- The §22 correspondence claim, §0.3 derivation claim, and §25 enforcement
  claims become mechanically true.
- Bare Rev-2 gate citations become detectable, closing the translation-sweep
  regression hole.

**Trade-offs:**
- Structural (table/prose) comparison is more code than bullet fuzz-matching;
  scoping it to the phase↔gate tables plus exact-quote checks for named
  criteria keeps it small.

**Implementation Notes:**
Also add: §21-exit vs §22-gate consistency (would have caught Change #1), the
register↔note quote-identity check (Change #3), and a claim-citation check for
"docs/08 R\d+" style references (Change #6).

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ -2436,6 +2436,13 @@
 Status/Evidence/Decision columns, the DX-09 preregistration
 requirement, and the §22 re-homing decisions; the dossier validator
 enforces gate-citation hygiene, bidirectional plan↔docs/52 bullet
 correspondence, and START_HERE gate annotations. Where plan prose and
 RFC disagree, the RFC is corrected and becomes normative. The plan is a
 map, not the spec.
+
+The validator itself is brought up to these claims and its outputs
+regenerated in the same pass: lettered PR headings (4a/15a/25a) are
+matched; G0 and the phase↔gate tables are compared structurally, not as
+empty bullet lists; bare Rev-2 gate citations (including `Target gate:`
+metadata in RFCs 0001–0025) require a scheme qualifier or banner; §0.3's
+G0 counts and §24.5's register quotes are derived checks; and
+`VALIDATION_REPORT.md` / `validation-results.json` are regenerated so
+every advertised check has a recorded result.
```

---

### [MEDIUM] Change #11: Extend the §25 specification-debt ledger to the machine-contract gaps

**Current State:**
The schema audit found the plan's headline guarantees unrepresentable in the
artifacts that must carry them, none of it currently booked in §25's debt list:
B11's envelope (five of nine dimensions missing from
`assurance-result.schema.json`, no producing-engine field, no typed
`Unsupported(reason)`, and a `resource-exhausted` **verdict** that directly
contradicts §11.4's "Budget exhaustion is never a verdict"); no schema for the
intent-registry record, so the `Proposed`-vs-accepted bit and §4.2.1 acceptance
chains are unrepresentable; `Redacted(reason, commitment)` undefined
dossier-wide despite being load-bearing in §4.5 and §18.4; no promotion-receipt
schema and no §8.6 cost-ledger field (with `additionalProperties: false`
blocking retrofit); INV-008's inconclusive reasons, `validation_basis`, and
parameterization all optional where §11.4 says "never render identically" /
"structurally distinct"; `proof-receipt` unconditionally requiring a Lean block
even for kernel-checked SAT certificates, and missing the §20 toolchain
identity; task `Failed` with no typed reason against §4.5; two incompatible
`$id`/versioning conventions and no schema-epoch field despite §4.3's
readability promise. Also RFC-side: RFC 0026 omits the N/N−1 window, the
evidence-readability decoupling, mid-flight budget updates, and real evidence
subscriptions (all §4.3 requirements); RFC 0032 omits §8.6's incremental
gate-5–7 rule and cost ceilings; RFC 0037 omits `intent.accept`/`intent.lock`/
`revise-intent` and drops the knowledge/security observer projections.

**Proposed Change:**
Add these as named debt items in §25's pre-PR-5 ledger (each with its target
file, per the ledger's existing style). Artifact-level fixes in Appendix A10.

**Rationale:**
§25's ledger is the mechanism the plan already has for exactly this. The items
above are freeze-blocking by the ledger's own criterion (they touch interfaces
PR 5 freezes) and the `resource-exhausted` verdict is an active soundness
contradiction, not just debt.

**Benefits:**
- The gap between prose guarantees and machine contracts becomes tracked,
  checkable work instead of latent drift.

**Trade-offs:**
- The debt list roughly doubles; that is a measurement, not a cost.

**Implementation Notes:**
Highest-priority single items: the `resource-exhausted` verdict removal, the
`Redacted` shared `$defs`, and the intent-registry record schema.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ -2444,6 +2444,24 @@
 - RFC 0026: the normative IDL file (currently referenced, not present);
+- RFC 0026: absorb the §4.3 requirements it currently omits — the N and
+  N−1 protocol-major window, the evidence/receipt readability decoupling,
+  mid-flight budget updates, and evidence subscriptions;
+- RFC 0032: absorb §8.6 — the incremental gate 5–7 reuse rule, cost
+  ceilings, and `BudgetExhausted`-with-continuation;
+- RFC 0037: the `intent.accept` / `intent.lock` operations and
+  `revise-intent` capability (currently only in RFC 0027), and the
+  knowledge/security observer projections dropped from §5.2's four;
+- RFC 0038: the evidence-graph write/concurrency model (Change §11.7);
+- schemas: an intent-registry record schema carrying `Proposed`/accepted
+  status and §4.2.1 acceptance chains; a shared `Redacted(reason,
+  commitment)` `$defs` used by every artifact class (§4.5, §18.4); a
+  promotion-receipt schema carrying the §8.6 cost ledger and gate
+  profile; a B11-complete assurance envelope (all nine dimensions, each
+  naming its producing engine or a typed `Unsupported(reason)`), with the
+  `resource-exhausted` verdict removed (§11.4: budget exhaustion is never
+  a verdict); conditional enforcement for INV-008 reasons,
+  `validation_basis`, and fail-closed diffs; typed `Failed` reasons on
+  tasks (§4.5); one `$id`/versioning convention with a schema-epoch field
+  (§4.3);
 - RFC 0037: the intent-bundle distribution and convergence section
   (§4.2.1);
```

---

### [MEDIUM] Change #12: Mandate the derived-docs contradiction sweep

**Current State:**
Six places where a Rev-3 doc *contradicts* (not merely omits) the plan or its
normative RFC, ranked by the audit: (1) docs/42:69–75 makes the Incremental
Parity Audit **opt-in** ("only when explicitly requested by client policy")
against §9.5's mandatory CI-plus-sampled-local runs and RFC 0030's default-on
1-in-64 sampling — this is the INV-010 enforcement mechanism; (2) docs/40 never
states that `Unknown` fails closed and introduces `incomparable` with no
blocking status — the INV-001/INV-011 promotion boundary; (3) docs/41 specifies
a 9-step pipeline against RFC 0032's twelve gates, demotes `incremental_parity`
from gate to receipt field, and carries **no** "RFC 0032 is normative" banner
despite §25 explicitly promising one; (4) docs/35:72,137 presents cross-user
computation sharing as permitted/mode-defining against §4.5's off-by-default
existence-oracle rule; (5) docs/45:105–118 orders `security compliance` ninth
(after cost) where §19.3/RFC 0034 put it second; (6) docs/42:13's
`Elaborate(module, imports)` query key drops `semantic_epoch` — the precise
mistake §9.2 warns against.

**Proposed Change:**
Add the six items to §25's reconciliation paragraph as named corrections, with
the docs/41 banner called out as immediate (it is one line and prevents
implementing the wrong pipeline). Full edits in Appendix A11.

**Rationale:**
These are the statements an implementer or agent will actually follow — three
of them sit on soundness-enforcement boundaries (parity audit, fail-closed
diff, gate count). Distinguishing them from the already-booked omission debt
matters because omissions fail loudly and contradictions fail silently.

**Benefits:**
- The three soundness-boundary contradictions stop being implementable.

**Trade-offs:** None.

**Implementation Notes:**
docs/35 is drift-total (predates §4.5–4.7 entirely); regenerate rather than
patch, per the audit's recommendation.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ -2456,6 +2456,13 @@
 - docs/41: regenerate around the 12-gate, phase-profile repair design
   (RFC 0032 is normative in the interim).
+- Derived-doc contradictions corrected in the same pass (each currently
+  states the opposite of the plan or its normative RFC): docs/42's
+  opt-in parity audit (§9.5 and RFC 0030 make sampling default-on) and
+  its `Elaborate` query key missing `semantic_epoch` (§9.2); docs/40's
+  missing fail-closed rule and unbound `incomparable` (§5.3); docs/41's
+  missing RFC 0032 banner (added immediately, ahead of regeneration);
+  docs/35's cross-user sharing default (§4.5); docs/45's grading order
+  (security is second per §19.3/RFC 0034).
```

---

### [MEDIUM] Change #13: Name the phase owners — or the program is `BLOCKED` by its own rule

**Current State:**
§21.1 (plan.md:1962–1964): "Each phase names an owner and a minimum viable
team; a phase without both is `BLOCKED`, not in progress." No owner or team is
named anywhere in plan.md, START_HERE, or any dossier file. Read literally, the
plan declares its own Phase A `BLOCKED`.

**Proposed Change:**
Designate START_HERE as the home of the resourcing table, make filling a
phase's row a merge requirement of that phase's opening PR, and state the
current honest status (unstaffed → `BLOCKED`) so the rule has a recorded
initial value. Table skeleton in Appendix A12.

**Rationale:**
A gate-driven plan with an unfalsifiable staffing rule invites exactly the
ambiguity the rule was written to kill. Recording "currently BLOCKED pending
owners" is not an admission of failure — it is the claim discipline applied to
the plan itself.

**Benefits:**
- The BLOCKED rule becomes checkable (validator: every phase in START_HERE's
  table has owner+team or status `BLOCKED`).

**Trade-offs:** None.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ -1962,7 +1962,10 @@
 Phases are gate-driven, not time-driven. Each phase names an owner and a
 minimum viable team; a phase without both is `BLOCKED`, not in progress.
+The owner/team table lives in `notes/START_HERE_IMPLEMENTATION.md`;
+filling a phase's row is a merge requirement of that phase's opening PR,
+and an unfilled row records the phase as `BLOCKED`.
 Phase A additionally resolves: the product license (permissive, compatible
```

---

### [MEDIUM] Change #14: Specify the signing-identity lifecycle

**Current State:**
The dossier leans on signatures in three load-bearing places — §4.2.1's intent
bundles ("honored only if its signature chain satisfies the local policy"),
§4.4's signed receipts, and §18.3's signed domain packs — and specifies key
management nowhere: no minting, storage, rotation, revocation, loss recovery,
or trust-root distribution, and no answer for the solo developer who *is* the
whole chain. CI "fails closed when … its acceptance chain does not verify",
which without a key story is a bootstrapping wall.

**Proposed Change:**
Add §18.6 defining the signing-identity lifecycle at design level and route the
details to docs/09 (Appendix A13): local keypair minted on first use and
recorded in the audit log (solo default); organizational deployments pin an
allowed-signers set distributed inside the intent bundle; rotation and
revocation are audited daemon operations; a signature that cannot be verified
downgrades provenance (typed) rather than failing open — except the §4.2.1 CI
check, which stays fail-closed by policy.

**Rationale:**
Every fail-closed signature check is only as available as its key
distribution. Without a specified lifecycle, teams will improvise (shared keys
in CI secrets, unrotatable roots), and the security architecture's most
security-relevant transition (intent acceptance) inherits the improvisation.

**Benefits:**
- §4.2.1 becomes implementable end-to-end, including the solo-dev bootstrap.
- Receipt provenance gains a defined degraded state instead of an undefined one.

**Trade-offs:**
- Key management scope-creep risk; capping the plan text at one design
  paragraph and delegating to docs/09 contains it.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ -1802,6 +1802,17 @@
 ### 18.5 Audit
 
 Every privileged operation records actor, capability, inputs, policy decision, outputs, and evidence identity. Audit logs are append-only and separate from semantic events.
+
+### 18.6 Signing identities
+
+Receipts, intent bundles, and domain packs are signed. The signing
+lifecycle is specified in docs/09: identities are minted through audited
+daemon operations (the solo-developer default is a local keypair minted
+on first use and recorded in the audit log); organizational deployments
+pin an allowed-signers set distributed inside the intent bundle;
+rotation and revocation are audited operations; a signature that cannot
+be verified downgrades the artifact to typed unverified provenance
+rather than failing open — except the §4.2.1 CI acceptance check, which
+fails closed by policy.
```

---

### [MEDIUM] Change #15: Give the evidence graph a write and concurrency model

**Current State:**
§11 defines nodes, edges, and the status lattice, and §11.6 defines swarm
roles — but nothing defines what happens when multiple agents write
concurrently: no isolation statement, no ordering of `INVALIDATES` vs
`SUPERSEDES` races, no rule for concurrent contradictory promotions, no lost-
update posture. The plan itself concedes the spike "validates the authority
policy table, not authenticated enforcement, concurrency, or Byzantine agents"
(plan.md:2490–2492), and review 3's scale work (§10^7 nodes) addressed volume,
not races. RFC 0038 is silent too.

**Proposed Change:**
Add §11.7 stating the write model: the graph is append-only; publication is
per-artifact atomic (INV-017) and linearized per claim identity; concurrent
contradictory claims materialize a `Conflict` node rather than last-writer-
wins; idempotency keys make retries safe; and status promotion is a
compare-and-set on the claim's current status so racing promotions cannot
regress the lattice. RFC 0038 section booked as debt (folded into Change #11's
ledger diff).

**Rationale:**
The multi-agent workbench (Phase F) is the product's centerpiece; a shared
mutable truth store without declared concurrency semantics is where "one agent
overwrote the refutation" bugs come from — the exact class of silent evidence
corruption the release-blocker doctrine names.

**Benefits:**
- Multi-agent coordination gets defined semantics before the swarm exists.
- The `Conflict` node type gains its generative rule.

**Trade-offs:**
- Linearization per claim identity constrains implementation; it is the
  weakest guarantee under which the status lattice's monotonicity survives
  races.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ -1332,6 +1332,17 @@
 Roles are capabilities and task views, not separate truth domains.
 
+### 11.7 Write model
+
+The evidence graph is append-only. Publication is per-artifact atomic
+(INV-017) and linearized per claim identity; status promotion is a
+compare-and-set against the claim's current status, so racing
+promotions cannot regress the lattice. Concurrent contradictory claims
+materialize a `Conflict` node rather than resolving by write order.
+Idempotency keys make agent retries safe. The concurrency section of
+RFC 0038 is specification debt (§25); the evidence-graph spike
+validated the authority table, not concurrent enforcement (§25).
+
 ---
```

---

### [MEDIUM] Change #16: Formative usability sessions before the summative G8 study

**Current State:**
The plan's only human-factors instrument is the preregistered G8 study —
authored in Phase E, run in Phase F. Every UX decision in §13 (progressive
disclosure, diagnostics, explanation levels) ships through Phases B–E with no
human evidence at all; the first structured human feedback arrives when the
design is frozen enough to preregister against. B24 ("Formal-methods UX
requires empirical science") deserves better than a single summative endpoint.

**Proposed Change:**
Add a Phase C deliverable: lightweight formative sessions (think-aloud, ~5
participants per persona) on the edit/check/explain loop, repeated each phase
from C onward, feeding docs/34 — explicitly distinct from and non-binding on
the preregistered summative study.

**Rationale:**
Preregistered studies confirm; they do not steer. Cheap formative loops are how
the explanation engine and progressive disclosure get to a state *worth*
preregistering against, and they de-risk G8 (a failed summative study in
Phase F is a program-level schedule event).

**Benefits:**
- Human evidence enters three phases earlier at trivial cost.
- G8 pass probability rises; docs/34's targets get empirical grounding.

**Trade-offs:**
- Small recurring cost; participant recruitment is the real constraint —
  the Phase C corpus tutorials double as session material.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ -2055,6 +2055,10 @@
 - CML language stability: normalized semantic AST frozen before surface
   syntax; formatter and migration tool ship before syntax stability
   (docs/11 §14).
+- formative usability sessions (think-aloud, ~5 participants per §13.1
+  persona) on the edit/check/explain loop, repeated each phase from C
+  onward and feeding docs/34 — distinct from and non-binding on the
+  preregistered summative G8 study (§21.1);
```

---

### [LOW] Change #17: Handles are identifiers, not secrets — fix the "unforgeable" wording

**Current State:**
§4.4 (plan.md:614–616): "Handles carry a kind prefix but are otherwise
structureless and unforgeable; clients cannot derive one handle from another."
But the same section is titled "Content-addressed artifacts" — and a
content-addressed identity is *by definition* derivable by anyone holding the
content. §4.5's cross-user dedup existence-oracle rule exists precisely because
of that derivability. "Unforgeable" is the wrong property; the real invariant
(stated correctly in the next sentence) is that possession confers no
authority.

**Proposed Change:**
Reword to distinguish content identities (derivable, no authority) from
capability tokens (`cap_*`, random, authority-bearing).

**Rationale:**
Security wording that overstates a property invites designs that rely on it
(handle-as-bearer-token), which §4.5 then has to undo.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ -614,5 +614,8 @@
-Handles carry a kind prefix but are otherwise structureless and
-unforgeable; clients cannot derive one handle from another. Authorization
-is checked independently of handle possession.
+Handles carry a kind prefix but are otherwise structureless. Content-
+addressed identities are derivable by anyone holding the content — the
+§4.5 existence-oracle rule exists because of this — so handles are
+identifiers, not secrets, and confer no authority. Capability tokens
+(`cap_*`) are minted randomly and do confer authority. Authorization is
+always checked independently of handle possession.
```

---

### [LOW] Change #18: Bring the §3.1/§3.2 snippets up to their own normative shapes

**Current State:**
(a) §3.1's Cargo snippet (plan.md:374–381) shows only `entry` and `profile` —
no `intent` key — three lines above prose requiring configuration to reference
intent "by registry identity (`in_*`)"; the dossier's own
`examples/continuum.project.toml` gets it right. It also lacks the
intent-bundle reference §4.2.1's CI fail-closed check needs. (b) §3.2's agent
call (plan.md:433–442) uses flat `target: "property:…"` and `output:`; RFC
0027 (normative per §25) uses a structured target, `output_policy`, and a
required `idempotency_key` — and `examples/agent_workflow.md` meanwhile uses
`"workspace"` where plan and schema say `"snapshot"` (example fix in
Appendix A14).

**Proposed Change:**
Update both snippets to the normative shapes.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ -376,6 +376,8 @@
 [package.metadata.continuum]
 entry = "crate::protocol"
 profile = "dev"
+intent = "in_7c2f91"          # registry identity, never a file path
+intent_bundle = "inb_09aa41"  # §4.2.1: CI fails closed without it
@@ -433,10 +435,13 @@
 {
   "operation": "verification.start",
+  "idempotency_key": "b1946ac9…",
   "snapshot": "ws_4f...",
   "intent": "in_91...",
-  "target": "property:AckImpliesDurable",
   "budget": {"states": 100000, "wall_ms": 30000},
-  "output": {"context_pack": true, "max_tokens": 6000}
+  "arguments": {
+    "target": {"kind": "property", "id": "AckImpliesDurable"}
+  },
+  "output_policy": {"context_pack": true, "max_tokens": 6000}
 }
```

---

### [LOW] Change #19: Mechanical residue sweep

**Current State / Proposed Change (one line each; artifact fixes in A15):**
- `REVISION_3_CHANGELOG.md:13,24` and `README.md:87` use pre-rename
  periphrases ("clean-build differential checks"/"clean-build parity") →
  "Incremental Parity Audit".
- `START_HERE:372` rename note says "clean-build **differential** Tribunal";
  every other source says "clean-build Tribunal" — align.
- `docs/48:89` cites "docs/52 G8" for a publish-before-run requirement no G8
  bullet carries → add the bullet to plan §22 G8 and docs/52 G8 (it is the
  preregistration's teeth).
- docs/09's incident-policy item 6 (certified-mode eligibility reevaluation)
  is invisible from both gate documents → add to §22's blocker doctrine.
- RFC 0028 never names `ctx_*`; RFC 0031 never names `diff_*` (its own
  schema pattern-enforces it) → one line each.
- Register/`§0.3` cite research notes as `/13`, `/30`, `/32` — unresolvable by
  grep → full paths.
- `benchmark-task.schema.json` `$id` says `continuumbench-task.json`,
  mismatching its filename.
- `generate_manifest.py:44` hardcodes the Generated date; `validate_dossier.py`
  hardcodes `assertions: 30` and mislabels total files as `empty_files`.

Representative plan diff (G8 preregistration bullet):
```diff
--- plan.md
+++ plan.md
@@ -2270,6 +2270,8 @@
 - the preregistered study (§21.1) covers both cohorts — Rust newcomers
   completing the deterministic/causal workflow, and distributed-systems
   experts diagnosing real failures;
+- the preregistration (expanded docs/48) is published before the study
+  runs; results are graded only against its fixed thresholds;
```

---

## Appendix A — Dossier-level fixes (no plan.md diff; fix in the named artifact)

**A1 (Change #1).** `docs/52_RELEASE_GATES_REV3.md:110` — adopt the two-system
criterion; delete the divergence note at `:118–119` (which also mis-cites "the
second bullet" for the third).

**A2 (Change #2).** `rfcs/0037-intent-contract.md:53–64` and
`schemas/intent-contract.schema.json` — extend the protected set and `policy`
object with `scope`, `nondeterminism`, `abstraction_maps`; decide the
`proof-required` verb question by RFC revision.

**A3 (Change #3).** `research/13` — add the missing kill-criteria section
(README promotion contract item 5). `research/33` — open the neighborhood-
adequacy lane. Add `Claim class:` headers to research/25–36.
`research/31:106–110` — fix the circular threshold deferral (the note owns the
number).

**A4 (Change #4).** `semantic-diff.schema.json` — add `if/then`: relation ∈
{unknown, unsupported, incomparable} ∧ `protected: true` ⇒ `policy.decision` ∈
{block, review} (the conditional technique already exists in
`repair-transaction.schema.json:23–71`). `docs/40:119–121` — state the
fail-closed rule for classifications, not just heuristics; bind `incomparable`.
`rfcs/0031` — state the seven-dimension completeness guarantee (its normative
home), and reconcile `expanded/contracted` vs `added/removed` naming for
bounds/faults with plan §5.3's wording.

**A5 (Change #5).** Add `abstraction_maps` to `intent-contract.schema.json`,
the `field` enum of `semantic-diff.schema.json`, and RFC 0037's field table —
`merged`/`split` finally attach to something.

**A6 (Change #6).** `docs/08_RISK_REGISTER.md` — add R21 (incremental-engine
substrate; failure mode: edge classes collapse to Conservative; controls:
Phase B spike + ADR, invalidation-precision baseline, §9.5 audit).

**A7 (Change #7).** `docs/34` — add the evidence-query row with the ≥10^7-node
scale qualifier to the gate-normative table; mark the four workflow acceptance
tests as the §21.1/G8 referent (the latency table already models the marking).

**A8 (Change #8).** `rfcs/0027-agent-tool-protocol.md` — add
`correspondence.*` and `observe.*` rows with authority levels;
`observe.ingest` requires the production-trace capability (§18.2).

**A9 (Change #10).** `tools/validate_dossier.py` — fix `PR_HEADING_RE` to
`(\d+[a-z]?)`; validate gate annotations against §22's table; compare G0 and
the phase↔gate tables structurally; add scheme-qualifier enforcement for
`Target gate:` lines in RFCs 0001–0025 (`rfcs/0004/0006/0010/0018` currently
carry bare Rev-2 gates; `adr/0011:27` and docs/08's G2 references likewise
need banners or translation); derive §0.3's G0 counts from the matrix; fix the
`empty_files` key and `assertions` literal; regenerate `VALIDATION_REPORT.md`
and `validation-results.json` (currently missing all five newest checks and
stale on file counts); `generate_manifest.py` — emit a real timestamp.

**A10 (Change #11).** Schema fixes, priority order:
1. `assurance-result.schema.json` — remove `resource-exhausted` from the
   verdict enum (§11.4); add the nine-dimension envelope, each dimension an
   object `{engine}` or typed `Unsupported(reason)` (able to express
   `sequential-consistency-only`); require a typed reason with
   `verdict: inconclusive`; unify with `context-pack.schema.json`'s divergent
   embedded envelope via shared `$defs`.
2. New `intent-registry-record.schema.json` — status
   (`Proposed`/accepted), acceptance records, §4.2.1 signature chains.
3. Shared `Redacted(reason, commitment)` `$defs`; reference from
   proof-receipt, context-pack, crashpack.
4. New promotion-receipt schema — §8.6 cost ledger, gate profile,
   `not_yet_enforced` list.
5. `evidence-graph-node.schema.json` — conditionals: `inconclusive` ⇒
   `inconclusive_reason`; `validated` ⇒ `validation_basis`.
6. `proof-receipt.schema.json` — make `lean` conditional on certificate kind
   (kernel-checked SAT/closed-set certificates are Lean-free per §20); add
   toolchain identity (INV-014); structured axiom manifest.
7. `repair-transaction.schema.json` — cost-ledger field; `version`/
   `supersedes` lineage; require all twelve gates listed at every status (the
   example currently lists 4 of 12 at `ready`); bind profile → permissible
   `not_yet_enforced` set.
8. `context-pack.schema.json` — require `replay` and `guarantees`;
   `dependentRequired`: `expandable: true` ⇒ `expansion`.
9. `verification-task.schema.json` — typed `Failed` reason.
10. One `$id` convention; schema-epoch field; a `schemas/README.md` stating
    the versioning policy.

**A11 (Change #12).** `docs/42:69–75` — parity sampling default-on per RFC
0030 (1-in-64), opt-*out* by policy with the opt-out recorded in envelopes;
`docs/42:13` — restore `semantic_epoch` to the `Elaborate` key; adopt the
"Incremental Parity Audit" name and the three auditability classes.
`docs/41` — one-line RFC 0032 banner *now*; regenerate later per §25.
`docs/35` — regenerate post-§4.5–4.7 (drift is total; `:72`/`:137` cross-user
sharing statements actively contradict §4.5). `docs/45:105–118` — reorder the
grading vector (security second); add the named held-out-family discipline and
the two missing §19.5 attacks ("return unsupported as pass", "exploit stale
cache") plus "intent integrity is a prerequisite, not a bonus metric".

**A12 (Change #13).** `notes/START_HERE_IMPLEMENTATION.md` — resourcing table
(Phase | Owner | Minimum team | Status), all rows initially `BLOCKED`; also fix
its `:372` rename-note wording (A15) and note the PR 9/27/30 gate annotations
follow §21 deliverables, not §22's phase table, per the audit's citation-
precision finding.

**A13 (Change #14).** `docs/09_THREAT_MODEL.md` — signing-identity lifecycle
section (minting, storage, rotation, revocation, loss recovery, allowed-signers
distribution via intent bundles, typed unverified-provenance downgrade).

**A14 (Change #18).** `examples/agent_workflow.md` — `workspace` → `snapshot`;
`task` → `task_id`; add `idempotency_key`; align result-object keys with
`verification-task.schema.json`.

**A15 (Change #19).** As listed in the change body.

---

## Appendix B — Verified clean this pass

- **G0 matrix** (`notes/G0_SPIKE_MATRIX.md`): Status/Evidence/Decision columns
  present; all four re-homings recorded with correct targets; DX-09
  preregistration requirement present; 8 + 3 + 4 = 15 reconciles with §0.3;
  freeze-blocking subset character-identical across matrix, plan §22, and
  docs/52; spike-report citations resolve.
- **START_HERE**: PR 0, 4a, 9 (four kernel crates, <15k covenant), 15a, 25a,
  28 all present; every one of 34 PR headings gate-annotated; sequence
  0–30 + inserts monotone with no gaps.
- **docs/34**: latency table marked gate-normative for G5 with the saturating-
  swarm condition; explain, reverse-step, and branch-fork rows all present.
- **docs/48**: owns the preregistration role verbatim (two cohorts, raw-trace
  comparator, Phase E authoring / Phase F execution).
- **docs/08**: R04, R06, R10 exist and say what the plan cites.
- **docs/09**: incident policy carries all five cited measures (plus a sixth
  the gates don't mention — see Change #19).
- **RFC error taxonomy**: all fifteen §10.3 codes present in RFC 0026, zero
  orphan codes anywhere in the seven RFCs.
- **RFC 0027 registry**: 1:1 with §10.2, no drift (before Change #8's
  additions); all handle prefixes used in RFCs are registered in §4.4.
- **RFC 0032**: twelve gates 1:1 with §8.2 in order; all eight §8.3
  neighborhood strategies carried; phase profiles match §21's staging.
- **RFC 0037**: all fourteen §5.2 field groups present; security-policy open
  question genuinely resolved; schema `policy.required` matches §5.4's twelve
  fields.
- **RFC 0031**: sixteen-relation lattice is a well-formed superset of §5.3;
  schema `relation` enum matches byte-for-byte.
- **Schemas**: 16/16 examples structurally valid; `workspace-snapshot` holds
  intent by registry reference exactly per §4.2; `intent-contract` carries all
  four observer projection kinds and the exact fidelity-profile enum.
- **docs/33**: triptych/layer structure reconciled; zero Rev-2 gate leakage.
- **docs/44**: nine-status lattice with correct authority table ("no `draft`
  status" guard included).
- **RFC 0034**: grading order exact per §19.3, including the intent-integrity
  zero-cap.
- **Rename hygiene**: "clean-build Tribunal" has zero live occurrences;
  every surviving "Tribunal" is the ADR-0021 corpus oracle or a rename note;
  docs/04 carries its Rev-2 banner; RFCs 0011/0012/0019 carry translation
  notes; ADR-0022 records the ladder rename.
- **plan.md §22 ↔ docs/52 bullets**: identical bullet sets G1–G10 (wording
  drift only) — the G10 criterion divergence of Change #1 being the exception.

---

## Closing assessment

Reviews 1–3 converged plan.md; this review's finding is that the *dossier* has
not converged on plan.md. The pattern across all six audits is consistent: the
plan states an obligation, marks it done, and the artifact half of the edit is
missing, weaker, or contradictory — and the validator that should catch this
enforces a softer predicate than the plan claims it does. None of this
threatens the architecture; all of it threatens the dossier's core currency,
which is that a stated claim is a checked claim. The four CRITICAL changes
restore self-consistency on gate criteria, lock vocabulary, the lane register,
and the diff guarantee; the HIGH changes make the plan's promises representable
and its tooling honest; the rest close capability families (observation,
correspondence, keys, concurrency, self-verification, staffing) that three
rounds of review left without owners. Apply Changes #1–#4 and #10 before any
interface freeze; they are cheap, and every one of them guards a place where
the plan currently disagrees with itself or with the layer it declared
normative.
