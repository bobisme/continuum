# Review of `plan.md` (Revision 3 Master Implementation Plan) and the accompanying dossier

**Review date:** 2026-07-25
**Scope:** `plan.md` plus the full dossier — 56 docs, 52 ADRs, 40 RFCs, 36 research notes, 16 schemas, 14 spike programs, 17 Lean modules, corpus, benchmarks, examples, validation tooling (296 files, ~96k words). All were read for this review; findings below cite specific files.

---

## Executive Summary

### What the plan does exceptionally well

This is one of the most intellectually coherent plans of its kind. Its distinctive strengths deserve stating plainly, because the criticisms below do not diminish them:

1. **Anti-reward-hacking as an architectural first principle.** Protected Intent Contracts, semantic diffs, repair transactions with mutation challenges, and evidence-status authority separation (B1, B12, INV-001, §5, §8) form a genuinely novel and internally consistent answer to a problem most verification tooling ignores entirely.
2. **Falsification discipline.** The G0 spike matrix, kill criteria (§24), claim-status lattice, and the deliberately small first demo (§25) show unusual epistemic honesty about a research-heavy program.
3. **Agent-native interface design.** The typed-operations/explicit-handles/Context-Pack architecture (B2–B5, §10) is ahead of the field and correctly identifies terminal prose as the enemy.
4. **Honest validation reporting.** `VALIDATION_REPORT.md` explicitly states the Lean sources were never kernel-checked, no Rust exists, and the spikes are Python architecture-falsification experiments — a level of candor rare in project dossiers.

### The five systemic problems

The review found ~40 specific defects (detailed as changes below and in Appendix A). They cluster into five systemic problems:

1. **`plan.md` is disconnected from its own dossier.** The plan contains zero references to any doc, ADR, RFC, research note, or schema (its only two outbound links are to `notes/`). The consequences are severe: **four incompatible release-gate numbering schemes** coexist (`plan.md` §22 G0–G6, `docs/52` G0–G10, `docs/26` G0A–G6, `docs/04` G0A–G0D+G1–G6) while 20 RFCs and 12 ADRs cite G-numbers bound to the older schemes; three incompatible crate vocabularies; four incompatible evidence-status enums; two names for the model-language file extension; and the plan's own §3.2 example calls an operation (`verify.start`) that its own §10.2 does not define.
2. **The plan silently overrides its own accepted decisions.** ADR-0022 explicitly *rejects* deferring formalization ("Allows architecture to ossify around unprovable interfaces"), yet §21 defers all Lean work to Phase D. ADR-0021/`docs/26` require the corpus Tribunal at G0; the plan defers it to Phase C/F. The `continuum-kernel` TCB crate — named in the trust boundary of `docs/09`, the assurance schema, and RFC 0005 — is absent from §20's crate list. Intent lives *inside* the forkable workspace snapshot (§4.2), directly contradicting the benchmark-integrity principle in `research/33` ("Protected Intent Contract outside writable snapshot").
3. **The Revision-3 specification layer is inverted.** For every Rev-3 subsystem, the plan's prose is 1.5–3.2× longer than its governing RFC; the seven load-bearing Phase A–C RFCs average 887 bytes (RFC 0037, the Intent Contract — root of the entire product thesis — is 694 bytes). The plan is the de facto spec and the RFCs are stubs, which is backwards for interface-freeze decisions.
4. **The plan overstates its evidence.** The flagship spike results are largely true by construction: the 200→4-event Context Pack reduction uses 196 semantically inert noise events (the replay function has no branch for them); the CEGIS "grammar" is 7 hardcoded lambdas; the incremental-parity spike compares a dependency closure against itself; 18 of 24 Lean theorems are trivial projections and none were ever kernel-checked; spike outputs do not validate against the checked-in schemas (disjoint enums); 7 of 15 G0 gates have no artifact at all — yet §22 G0 claims "All items … have evidence or explicit redesign," and §25 presents the demo numbers without the spike report's own caveats. The plan also never applies the FACT/DESIGN/HYPOTHESIS claim discipline its own README announces.
5. **Load-bearing capabilities have no producer, and open research is scheduled as engineering.** Weak memory is a mandatory assurance-envelope dimension (B11) with no phase, crate, or gate producing it. Timed semantics appear in Phase-A intent fields though ADR-0016 defers them past G4. CML — required by §3.3, §4.2, §13, §14.7, §20 — is delivered by no phase. Production conformance (Phase F, G6) rests on a research note with no kill criteria and no crate. Phase B's own exit gates (§8.2 gates 9–10) require Phase C and D deliverables — an internal scheduling contradiction. Meanwhile 19 of 36 research notes lack the kill criteria their own README mandates, and the plan maps none of its features to their feasibility risk.

Also materially absent (from the plan *and* the dossier): resourcing/timeline of any kind, daemon crash recovery, CAS/evidence-ledger growth/GC/purge policy (which conflicts with append-only receipts when a secret lands in the CAS), multi-user authentication and capability issuance, licensing (one sentence in 296 files, despite "public ContinuumBench" being a 1.0 gate over a corpus with redistribution constraints), and packaging/distribution (the "No Java" pitch coexists with P4 parity requiring Lean, solvers, containers, and TLC oracles).

The proposed changes below are ordered by priority. Diffs are anchored to `plan.md` as it stands; where a change adds a section, the diff shows insertion points by surrounding context.

---

## Proposed Changes

---

### [CRITICAL] Change #1: Adopt a single release-gate scheme and map phases to gates

**Current State:**
`plan.md` §22 defines G0–G6. `docs/52_RELEASE_GATES_REV3.md` defines G0–G10 with different content at every number ≥1. `docs/26` and `docs/04` define two further Rev-2 schemes. All 20 Rev-2 RFCs carry `Target gate:` metadata and 12 ADRs cite G-numbers bound to the old schemes, so "before G2" is now ambiguous across four normative meanings. §21's Phases A–F are a fifth sequencing scheme mapped to no gate. `plan.md` §22 is also *weaker* than `docs/52`: it has no workbench-identity/lifecycle gate (immutability, idempotency, cancellation closes obligations, transactional publication, authorization independent of handle possession — the gate that would verify INV-002/INV-017), no standalone intent-integrity gate, and no release-blocker doctrine (the rule that a misleading assurance result, replay failure, stale receipt, hidden intent change, or unauthorized promotion is a stop-ship, while a missing feature is merely documented).

**Proposed Change:**
Adopt `docs/52`'s G0–G10 as the single scheme in §22, add a Phase↔Gate mapping table, restore the release-blocker doctrine, and add a one-time translation table for legacy ADR/RFC gate citations (with a follow-up sweep task).

**Rationale:**
Gates are the plan's enforcement mechanism. Four numbering schemes mean no ADR or RFC exit condition can be evaluated, and the two most constitutionally load-bearing gates (workbench identity, intent integrity) currently exist only in a doc the plan never cites.

**Benefits:**
- Every "before Gn" citation in 52 ADRs and 40 RFCs becomes evaluable.
- INV-001/002/017 gain their missing enforcement gates.
- The blocker doctrine restores the asymmetry between "unsupported" and "unsound" that §24's kill criteria assume.

**Trade-offs:**
- Renumbering touches many documents; the translation table bounds the cost.

**Implementation Notes:**
`docs/52` should become the normative source; §22 should summarize and link it rather than restate divergently. The legacy sweep can be a validator check (`tools/validate_dossier.py` already checks links; add a gate-citation check).

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ ## 22. Release gates @@
-## 22. Release gates
+## 22. Release gates
+
+The normative gate scheme is `docs/52_RELEASE_GATES_REV3.md` (G0–G10). This
+section summarizes it. Legacy gate citations in ADR-0001..0035 and RFC
+0001..0025 refer to the Revision 2 scheme in `docs/26`; the translation table
+at the end of this section maps them. No document may introduce a new gate
+numbering.
+
+| Phase (§21) | Gates it must close |
+|---|---|
+| A | G0 (falsification), G1 (workbench identity and lifecycle), G2 (ACI) |
+| B | G3 (intent integrity), G4 (causal debugging and repair) |
+| C | G5 (incremental trust) |
+| D | G6 (proof service) |
+| E | G7 (Forge) |
+| F | G8 (human usability), G9 (corpus parity), G10 (real adoption) |
 
 ### G0 — Load-bearing falsification
 
 All items in [`notes/G0_SPIKE_MATRIX.md`](notes/G0_SPIKE_MATRIX.md) have evidence or explicit redesign.
 
-### G1 — Agent-computer interface
+### G1 — Workbench identity and lifecycle
+
+- snapshots, handles, and artifacts are immutable and content-addressed;
+- requests are idempotent under idempotency keys;
+- cancellation closes obligations and publishes no partial finality;
+- artifact publication is transactional (INV-017);
+- authorization is checked independently of handle possession;
+- daemon crash recovery leaves no stale index entries or orphan tasks.
+
+### G2 — Agent-computer interface
 
 - no terminal parsing required;
 - explicit handles and resumability;
 - stale state rejected;
 - Context Pack improves agent benchmark effectiveness;
-- protected intent detects all benchmark gaming mutations.
+
+### G3 — Intent integrity
+
+- every benchmark gaming mutation in supported fragments is classified as a
+  privileged intent change;
+- mutations outside supported fragments classify as `Unknown` and block
+  ordinary promotion rather than passing silently;
+- intent policy locks are enforced; evidence is invalidated on intent revision;
+- no ordinary repair promotes with a protected-intent change.
@@ after the final gate @@
+### Release blocker doctrine
+
+A missing feature can be documented as unsupported. A misleading assurance
+result, replay failure, stale receipt, hidden intent change, or unauthorized
+evidence promotion is a release blocker at every gate. A confirmed
+false-positive success verdict triggers the soundness incident policy in
+`docs/09`: block release, revoke affected claim IDs, publish affected
+semantic epochs, ship an artifact scanner, add a permanent regression.
```

---

### [CRITICAL] Change #2: Fix the Phase B/C/D dependency inversion and restore early Lean seeds

**Current State:**
Phase B's exit ("an agent fixes ack-before-durable … and produces a promotion receipt") requires the §8.2 default promotion gates, but gate 9 ("Invalidated certificates/proofs are rebuilt") requires the Phase D proof pipeline and gate 10 ("Clean and incremental results agree") requires the Phase C incremental database. Separately, §21 places *all* Lean work in Phase D, silently reversing ADR-0022, which lists "Defer formalization until after 1.0" as a rejected alternative ("Allows architecture to ossify around unprovable interfaces") and sets its own G0 as "core transition/refinement/certificate theorems build with no `sorry`." `START_HERE` compounds this by placing the Lean worker at PR 28 of 30. Meanwhile §15.3's theorems 1, 3, and 5 are *about* Phase A–C artifacts (intent order, context slices, incremental equality) — formal models that should exist before those interfaces freeze.

**Proposed Change:**
(a) Introduce explicit, phase-staged promotion-policy profiles: gates enter the default profile as their producing subsystem ships; until then a receipt lists them as `NotYetEnforced` rather than silently passing. (b) Move the T0/T1 Lean seed theorems (transition-system safety, stuttering simulation, finite-closure certificate soundness) plus kernel-checking of the existing seed modules into Phase A, per ADR-0022.

**Rationale:**
As written, Phase B either cannot exit or exits by quietly skipping gates — the exact "green badge with hidden scope" failure the plan's own B11 prohibits. And freezing the protocol, intent schema, Context Pack format, and incremental query identity through Phases A–C with zero formal model is the ossification ADR-0022 was written to prevent.

**Benefits:**
- Phase exits become achievable and honest.
- Receipts gain an explicit enforcement profile, closing a reward-hacking gap (a Phase-B receipt currently looks identical to a Phase-D receipt).
- The 17 existing Lean modules stop being unverifiable decoration (they have never been parsed; see Change #6).

**Trade-offs:**
- Phase A grows by the Lean toolchain setup and ~3 small theorems. This is deliberate and small — the seed modules are ~620 lines total.

**Implementation Notes:**
The `NotYetEnforced` status must be structurally distinct in the receipt schema (not a passing status). Kernel-checking the seeds will likely surface real defects: the artifacts audit found 8 declarations with suspicious leading indentation, deprecated `List.get?`, and `induction … with` case-name patterns that may not elaborate.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ ### Phase A — Trust spine and ACI kernel @@
 - executable finite reference engine and certificates from Revision 2;
-- agent protocol spike parity.
+- agent protocol spike parity;
+- Lean environment pinned and seed modules kernel-checked; T0/T1 theorems
+  (transition-system safety, stuttering simulation, finite-closure
+  certificate soundness) compile with no `sorry` and empty axiom manifests
+  (ADR-0022 G0).
@@ ### Phase B — Real-code failure loop @@
 - exact and neighboring replay.
 
-Exit: an agent fixes ack-before-durable without changing intent and produces a promotion receipt.
+Exit: an agent fixes ack-before-durable without changing intent and produces
+a promotion receipt under the Phase B gate profile.
+
+Promotion gate profiles are phase-staged. Each §8.2 gate enters the default
+profile when its producing subsystem ships: gates 1–8 and 11–12 in Phase B;
+gate 10 (clean/incremental agreement) in Phase C; gate 9 (certificate and
+proof rebuild) in Phase D. A receipt must name its profile and list
+not-yet-enforced gates as `NotYetEnforced` — never as passed. A Phase B
+receipt is therefore structurally distinguishable from a Phase D receipt.
```

---

### [CRITICAL] Change #3: Label the plan's own claims and carry the spike caveats into §25

**Current State:**
`README.md` defines a claim discipline (`FACT/DESIGN/HYPOTHESIS/TARGET/BLOCKED/FALSIFIED`); `plan.md` never uses it (zero occurrences). `docs/18`'s claims matrix has *no claim at OBSERVED or better* — everything is TARGET or HYPOTHESIS — yet the plan reads uniformly as settled design. Concretely: §15.3 presents eight Lean theorems with no statement that **zero Lean code in the dossier has ever been parsed, elaborated, or kernel-checked** (VALIDATION_REPORT is explicit; `docs/18` C035 = BLOCKED). §25 presents the demo numbers without `docs/53`'s own caveats ("validates the basic artifact shape, not the general context compiler"; "not evidence that broad protocol synthesis will scale"), and the 23.62× context reduction was measured on a trace where 196 of 200 events were injected noise with no semantic effect (the spike's replay function has no branch for them). §22 G0 claims all G0 items "have evidence or explicit redesign" while 7 of 15 (DX-09..11, DX-13..15, and DX-06's actual required experiment) have neither.

**Proposed Change:**
Add a §0.3 "Claim status of this plan" that (a) applies the README's labels at section granularity, (b) states the Lean and spike evidence boundaries in the plan itself, and (c) corrects the G0 status line. Add the spike caveats to §25.

**Rationale:**
The plan's entire moral architecture is "a claim without evidence is a hypothesis" (§0.1) and "'Verified' alone is prohibited" (B11). The plan should meet its own bar. A reader of `plan.md` alone — which is how master plans get read — currently concludes the proof plane and the workbench spikes are much further along than they are. That is precisely the epistemic failure INV-014 and `docs/03` exist to prevent.

**Benefits:**
- Restores internal consistency with the README's claim discipline.
- Protects the project's credibility: the honesty is already *in the dossier*; surfacing it in the plan costs nothing and prevents the "formal-methods vaporware" critique from landing later.
- Makes §24's kill criteria evaluable (you cannot falsify claims that aren't labeled).

**Trade-offs:**
- None of substance; the plan reads slightly less triumphant.

**Implementation Notes:**
Section-level labels suffice; per-sentence labeling would be noise. The G0 correction should enumerate the seven unevidenced items.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ after ## 0.2 The product theorem @@
+## 0.3 Claim status of this plan
+
+This plan uses the dossier claim lattice (`README.md`). Unless marked
+otherwise, sections here are DESIGN (accepted architecture, not evidence).
+The following are explicitly weaker:
+
+- HYPOTHESIS: §6 general context compilation, §8.3 neighborhood adequacy,
+  §9 sub-file-granularity incremental trust, §12 causal explanation science,
+  §14 Forge co-synthesis and quality-diversity, §16 bidirectional lenses,
+  production partial-order conformance (Phase F). Each is owned by a
+  research lane with kill criteria (see §24.5).
+- Evidence boundary: no Lean source in this dossier has been parsed,
+  elaborated, or kernel-checked; no theorem may be described as
+  machine-checked (`VALIDATION_REPORT.md`, `docs/18` C035). The Revision 3
+  spikes are finite Python reference experiments that validate artifact
+  shapes and interaction contracts, not engines, scale, concurrency,
+  persistence, or soundness (`docs/53`, "What remains unproven").
+- G0 status: 8 of 15 G0 items have spike evidence; G0-DX-06 (the actual
+  neighborhood/mutation campaign), DX-09, DX-10, DX-11, DX-13, DX-14, and
+  DX-15 currently have neither evidence nor a redesign decision.
@@ ## 25. Immediate execution, after the first demonstration block @@
 This is deliberately small. It closes the complete product loop—from intent
 through invention and proof—before the project scales outward.
+
+The executed spikes behind this demonstration carry their own boundaries,
+which this plan adopts: the 200→4 event reduction was measured on a
+synthetic trace whose 196 noise events are semantically inert, so it
+validates the Context Pack artifact shape, not the general context
+compiler; the synthesis result was selected from a seven-candidate finite
+grammar and is not evidence that protocol synthesis will scale; the
+evidence-graph spike validates the authority policy table, not
+authenticated enforcement, concurrency, or Byzantine agents.
```

---

### [CRITICAL] Change #4: Move the Intent Contract outside the writable workspace snapshot

**Current State:**
§4.2 lists the Intent Contract as one of the contents of a workspace snapshot, and §10.2 exposes `workspace.create / fork`. `research/33` (agent benchmarks and reward hacking) states as benchmark principle 1: "Protected Intent Contract **outside** writable snapshot." `research/35` lists "stale snapshot substitution" among its attack surfaces. As designed, an agent that can fork a snapshot can fork one whose embedded intent differs, then run tasks against it — the exact gaming path INV-001 exists to close, laundered through the snapshot mechanism.

**Proposed Change:**
Snapshots carry a content-identity *reference* to the governing Intent Contract, which is stored and versioned in the intent registry only. `workspace.fork` never forks intent; rebinding a snapshot lineage to a different intent is a privileged operation that produces an intent diff.

**Rationale:**
Intent integrity is the plan's first bet (B1). A protected artifact stored inside a freely forkable container is not protected; the protection must live at the binding, not the copy.

**Benefits:**
- Closes the fork-with-modified-intent path structurally rather than by policy.
- Aligns §4.2 with `research/33`, `research/35`, and the daemon's Intent registry (§4.1), which currently has ambiguous custody.

**Trade-offs:**
- Snapshot self-containedness is slightly reduced; replay tooling must resolve one reference. Content addressing makes this cheap and exact.

**Implementation Notes:**
The verification-task schema already carries separate `workspace` and `intent` handles, so the operation surface needs no change — only the snapshot definition and the fork semantics.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ ### 4.2 Workspace snapshots @@
 - dependency lockfiles;
 - toolchain and semantic epochs;
-- Intent Contract;
+- the content identity of the governing Intent Contract (a reference, not
+  the contract itself);
 - generated correspondence;
 - proof environment;
 - configuration.
 
-Snapshots form a Merkle DAG. A tool call never means "whatever is currently on disk"; it means a named snapshot. Clients may create a snapshot from a working tree, overlay an in-memory editor buffer, or fork an existing snapshot.
+Snapshots form a Merkle DAG. A tool call never means "whatever is currently
+on disk"; it means a named snapshot. Clients may create a snapshot from a
+working tree, overlay an in-memory editor buffer, or fork an existing
+snapshot.
+
+Intent Contracts are stored and versioned only in the intent registry,
+outside every writable or forkable snapshot. `workspace.fork` preserves the
+intent binding by identity; rebinding a snapshot lineage to a different
+intent is a privileged operation that produces a semantic intent diff and
+invalidates dependent evidence (INV-001).
```

---

### [CRITICAL] Change #5: Scope the intent-diff guarantee honestly (G1 overclaim) and align the gaming taxonomy

**Current State:**
§22 G1 requires "protected intent detects **all** benchmark gaming mutations." But §5.3 itself admits undecidable cases ("emits a proof obligation or `Unknown` rather than guessing"), and §24's kill criterion is scoped to "supported fragments." The three statements are mutually inconsistent, and the only executed evidence is 6 handcrafted mutations classified by *set-membership comparison of opaque string tokens* (the spike's "property" is a frozenset of two words; any renaming or genuine formula rewrite defeats it — and the spike's own output enums don't validate against `schemas/semantic-diff.schema.json`).

**Proposed Change:**
Restate the guarantee as: within declared supported fragments, every gaming mutation classifies as a privileged intent change; outside them, the diff returns `Unknown`, and `Unknown` **blocks ordinary promotion**. Add the fragment declaration itself (ADR-0025's `Finite/Symbolic/Temporal/…` strata) to the Intent Contract so "supported" is a checkable predicate, not prose.

**Rationale:**
The current wording promises the undecidable. The safe posture — unknown-implies-privileged-review — is both achievable and *stronger* in practice, because it fails closed. The fragment system (ADR-0025, absent from the plan) is the mechanism that makes the boundary machine-checkable.

**Benefits:**
- G1 becomes passable without weakening the actual security property.
- Fail-closed semantics for the exact case attackers would target.
- Introduces the fragment vocabulary the plan needs in §5.3, §9.2, and §13.2 anyway ("How certain is the claim?").

**Trade-offs:**
- Some legitimate intent edits in unsupported fragments will require human review. That is the correct cost.

**Implementation Notes:**
This is also where RFC 0031 needs real content: a classification lattice and an implication order for bounds and assurance (the `no-decrease`/`no-downgrade` policy verbs in §5.4 require a total order no document defines). See Change #12.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ ### 5.3 Semantic intent diff @@
-Where implication is decidable or solver-checkable, Continuum proves the direction. Otherwise it emits a proof obligation or `Unknown` rather than guessing.
+Where implication is decidable or solver-checkable, Continuum proves the
+direction. Otherwise it emits a proof obligation or `Unknown` rather than
+guessing. `Unknown` fails closed: an intent change classified `Unknown`
+blocks ordinary promotion exactly as a confirmed privileged change does,
+pending review.
+
+Each intent field declares its semantic fragment (ADR-0025:
+`Finite / Symbolic / Temporal / Probabilistic / Theorem / Runtime`), so
+"supported" is a checkable predicate. The diff guarantee is: within
+declared supported fragments, every property weakening, assumption
+strengthening, bound decrease, observer coarsening, fault removal, and
+assurance downgrade is classified as privileged; outside them, `Unknown`.
```

---

### [HIGH] Change #6: Restore the independent certificate kernel to the crate architecture

**Current State:**
§20's crate list has no `continuum-kernel*` crate. Yet ADR-0008 defines "a small independent `continuum-kernel`"; RFC 0005 specifies four crates (`continuum-kernel-core/-sat/-smt/-temporal`) and mandates "the production engine and kernel MUST NOT share optimized evaluator code"; `docs/03` sets a <15,000 non-test-line TCB covenant with no-async/no-unsafe/no-dynamic-loading constraints; `docs/09`'s trust boundary and the assurance schema's checker-identity field both name `continuum-kernel`. §20's dependency rules state only the much weaker "proof/certificate checker does not depend on search engines." The TCB-shrinking story the whole assurance architecture rests on is architecturally unrepresented in the plan. (Related: the existing Python reference checker validates certificates against *the same in-memory model object* the explorer used — no serialization boundary, no independent process — so the independence property has never been exercised even in spike form.)

**Proposed Change:**
Add the four kernel crates to §20, plus the non-sharing rule, the size covenant, and the kernel constraints, and require a serialization boundary between engine and checker from PR 9 onward.

**Rationale:**
INV-004 ("No self-certification") and §22's "zero known paths for unprivileged agent to promote false evidence" are empty without a concretely bounded, independently buildable checker. This is also where ADR-0013's collision-resolution rule (exact canonical comparison in certified lanes; hash identity only in labeled non-certified modes) must live — the plan currently content-addresses everything and never mentions collisions, which `docs/26` lists as a 1.0 blocker class.

**Benefits:**
- Restores the dossier's most important trust-architecture decision to the master plan.
- Gives §22's G6 "zero known paths" claim its enabling mechanism.
- The size covenant creates a measurable guard against R12 (certificate system too large).

**Trade-offs:**
- Four more crates to maintain; that is the price of an auditable TCB and was already accepted in RFC 0005.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ ## 20. Crate and service architecture @@
 continuum-refinement
 continuum-certificate
+continuum-kernel-core
+continuum-kernel-sat
+continuum-kernel-smt
+continuum-kernel-temporal
 continuum-evidence
@@ Dependency rules: @@
 Dependency rules:
 
 - proof/certificate checker does not depend on search engines;
+- the `continuum-kernel-*` crates form the trusted checking base: no shared
+  optimized evaluator code with any engine, no async, no unsafe, no plugins
+  or dynamic loading, a <15,000 non-test-line covenant (docs/03), and a
+  serialization boundary between every engine and the kernel — a
+  certificate is checked from its wire form, never from shared memory;
+- in certified lanes, content identity is exact canonical identity; hash
+  collisions are resolved by canonical comparison; 256-bit-hash identity is
+  permitted only in explicitly labeled non-certified modes (ADR-0013);
 - model core does not depend on asupersync;
```

---

### [HIGH] Change #7: Every advertised capability names its producer or reads `Unsupported`

**Current State:**
§0 declares weak memory and time "represented honestly"; B11 makes "weak-memory model" a required assurance-envelope dimension. But no phase, crate, or gate delivers a weak-memory lane (ADR-0032 defines one; `START_HERE` defers it; the plan mentions weak memory twice, both aspirationally). Timed semantics appear in Phase-A intent fields ("timing" assumptions, "time semantics") though ADR-0016 explicitly defers timed/probabilistic implementation past G4, and `research/06` states the boundary the plan omits: "Initial Continuum MUST NOT advertise timed/probabilistic proof support." Hyperproperties, games, unfoldings, and nominal lanes are Accepted ADRs with zero plan presence. The envelope therefore has mandatory dimensions with no producing subsystem — a structural inducement to the exact "green badge with hidden scope" the plan prohibits.

**Proposed Change:**
Add the rule to B11: every envelope dimension names its producing engine or carries a typed `Unsupported` value; state the ADR-0016 non-advertisement boundary; and add a capability-schedule table (dimension → producing lane → phase or `Unsupported-in-v1`).

**Rationale:**
INV-008 already demands typed inconclusiveness; this extends the same honesty to the envelope's dimensions. It converts a latent false-advertising bug into explicit, checkable scope.

**Benefits:**
- Envelope becomes constructible in Phase A without waiting for (or faking) frontier lanes.
- Readers and agents can distinguish "SC-only, by declaration" from "weak-memory-checked."
- Resolves the C11/C12 contradictions with ADR-0016/0032 without descoping the ADRs.

**Trade-offs:**
- Publicly visible `Unsupported` fields; that is the point.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ ### B11 — Assurance is an envelope, not a badge @@
 Every result describes dimensions such as bounds, faults, fairness, values, schedules, weak-memory model, observer, proof status, and unknowns. "Verified" alone is prohibited in machine output.
+
+Every envelope dimension names its producing engine or carries a typed
+`Unsupported` value; a dimension is never silently omitted. Until the
+weak-memory lane ships (ADR-0032), every envelope's memory dimension reads
+`Unsupported(sequential-consistency-only)`. Until the timed and
+probabilistic extensions ship (ADR-0016), Continuum does not advertise
+timed or probabilistic proof support in any machine output, and timing
+fields in Intent Contracts are declarative assumptions, not checked
+semantics.
```

---

### [HIGH] Change #8: Add a frontier-lane register binding plan capabilities to research maturity and kill criteria

**Current State:**
`plan.md` contains zero references to `research/`. No plan feature carries a maturity label or a link to the lane that owns its feasibility risk. §24 says "Failure of a frontier research lane does not kill Continuum" without naming any lane. Meanwhile: 19 of 36 research notes lack the kill criteria their own README mandates; the two notes carrying the largest plan dependencies — `research/05` (production conformance; Phase F, G6) and `research/09` (cancellation/obligation calculus; the flagship demo and Phase B) — have *no kill criteria, no baselines, and no metrics*; `docs/31`'s nine quantitative promote/kill thresholds (e.g., certificate checking ≤10% of search time; observer independence ≥5× reduction) appear nowhere in the plan; and ADR-0020's per-lane governance regime (hypothesis, baseline, threshold, assurance cap, kill criterion) has no home.

**Proposed Change:**
Add §24.5 "Frontier lane register": a table mapping each HYPOTHESIS-class plan capability to its research note, current status, quantitative threshold, kill criterion, and the plan's fallback if the lane dies. Require the register (not chat, not the plan prose) to be the authority on lane status. Backfill kill criteria for notes 05 and 09 as a named task.

**Rationale:**
This is the missing joint between the plan's engineering half and its research half. Without it, §24's reassurance is unfalsifiable and open problems get scheduled as line items (which is exactly what happened to Phase B/D/F — see Changes #2 and the Phase F note below).

**Benefits:**
- Makes ADR-0020 enforceable and `docs/31`'s thresholds load-bearing.
- Forces fallback design where none exists today (e.g., `research/30`'s own fallback — "keep only a lightweight semantic-diversity archive" — becomes Forge's documented plan B).
- Directly supports Change #3's section labels.

**Trade-offs:**
- Maintenance of one table; trivially validator-checkable.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ ## 24. Kill criteria, after the final paragraph @@
 Failure of a frontier research lane does not kill Continuum. Failure of the single-semantic-contract, intent-integrity, replay, or evidence architecture does.
+
+## 24.5 Frontier lane register
+
+Every HYPOTHESIS-class capability in this plan is owned by a research lane
+with a baseline, a quantitative promotion threshold, a kill criterion, and
+a named fallback. The register is authoritative for lane status; no plan
+section may claim a lane's output without its status.
+
+| Capability | Lane | Threshold / kill | Fallback |
+|---|---|---|---|
+| General context compilation (§6) | research/25, /32 | pack-ablated agent benchmark win; kill if packs induce wrong repairs | plain causal slice + expansion |
+| Causal minimization (§6, §12) | research/01, /26 | ≥10× on non-artificial traces; kill if cost dominates verification | 1-minimal delta debugging only |
+| Sub-file incremental trust (§9) | research/27 | clean-parity at sampled rate; kill if capture untrustworthy below module granularity | module-granularity invalidation |
+| Forge co-synthesis + QD (§14) | research/29, /30 | rediscovery suite; kill if joint search loses to staged | staged synthesis; Pareto archive only |
+| Production conformance (Phase F) | research/05 | monitorability-gated; thresholds TBD (must be added — currently absent) | Lab-replay evidence only |
+| Cancellation calculus (§0, B19) | research/09 | mutation corpus; thresholds TBD (must be added — currently absent) | runtime checking only |
+| Bidirectional lenses (§16) | research/31 | ambiguity rate; kill if most mappings too ambiguous | get-only projection + drift detection |
+| Proof repair (§15.4) | research/28 | vs source/LSP loop baseline; kill if repair proposes weakening | context packs + human proof work |
+| Certificate overhead | docs/31 | checking ≤10% of search time | reduce certified-lane scope |
```

---

### [HIGH] Change #9: Schedule CML delivery, and pin the corpus program to its own vocabulary

**Current State:**
CML is required by §3.3 (`continuum model new`), §4.2 (snapshots contain "CML modules"), §13 (personas, LSP), §14.7 (Forge emits CML), and §20 (two CML crates) — but no phase in §21 delivers a parser, elaborator, or stability milestone. `START_HERE`'s governing rule defers "a complete CML parser" while its PR 25 requires CML diagnostics. Both checked-in `.ctm` fixtures self-describe as "not yet parsed." Meanwhile the corpus program: §21/§22 say "80 families at declared parity" but the plan never defines the P0–P5 parity vocabulary (ADR-0021 calls it normative), never pins the corpus commit, schedules only Waves 0/1 (29 families) in Phase C leaving Waves 2–5 (51 families) unscheduled until Phase F, and §19.4 uses the same 80 families as benchmark data that G6 requires the team to make pass — a leakage conflict with `docs/45`'s family-level train/test separation. Also: `PARITY_LEVELS.md` requires ≥5 P5 exemplars by 0.5 but no CSV row has `required_parity = P5` (three different P5 targets exist across docs).

**Proposed Change:**
(a) Phase B delivers a CML core-fragment parser/elaborator sufficient for the replicated register and Wave 0 (programmatic model API remains the Phase A surface); Phase C delivers language-stability per `docs/11` §14 (normalized AST stabilizes before syntax; formatter + migration tool before syntax stability). (b) §21/§22 adopt the P0–P5 vocabulary and pinned commit; Waves 2–3 land in Phase D/E; §19.4 excludes a named held-out family subset from all development use; the P5 target is set in one place (recommend: the ≥10 from `docs/26` G6, drawn from the 55 `runtime_refinement_target` rows).

**Rationale:**
A model language that is load-bearing for five sections but delivered by none is the single largest unscheduled work item in the plan. The corpus is the plan's compatibility contract; without the parity vocabulary and wave schedule in the plan itself, "declared parity" is unfalsifiable (the `docs/20` "vanity metric" objection).

**Benefits:**
- Eliminates the plan's largest silent scope hole.
- Corpus progress becomes expressible (RFC 0019's 10-value port status taxonomy: only `passing` counts).
- Resolves the benchmark-leakage conflict before it invalidates ContinuumBench.

**Trade-offs:**
- Phase B grows. The alternative — discovering in Phase C that the LSP has no language to serve — is worse.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ ### Phase B — Real-code failure loop @@
 - asupersync semantic adapter;
+- CML core-fragment parser and elaborator (Finite fragment; enough for the
+  replicated register and Wave 0 ports; programmatic model API remains
+  supported);
 - storage/network/process packs;
@@ ### Phase C — Interactive scale @@
 - clean-build Tribunal;
 - proof Context Packs;
-- Wave 0/1 corpus interaction tasks.
+- Wave 0/1 corpus interaction tasks (29 families, pinned to
+  tlaplus/Examples@91c22ea…) at their required parity per
+  `corpus/tla-examples/PARITY_LEVELS.md`;
+- CML language stability: normalized semantic AST frozen before surface
+  syntax; formatter and migration tool ship before syntax stability
+  (docs/11 §14).
@@ ### Phase D — Proof and liveness @@
 - refinement receipts.
+- corpus Waves 2–3 at required parity.
@@ ### 19.4 Dataset construction @@
 Train/dev/test partitions isolate semantic families and source hashes to reduce leakage.
+
+A named subset of corpus families is held out from all development,
+tuning, and regression use and graded only by the isolated ContinuumBench
+grader. G6's "80 families at declared parity" is measured on the
+development set plus a final, single evaluation of the held-out set.
```

---

### [HIGH] Change #10: Add the missing operations chapter (daemon crash recovery, storage lifecycle, purge, multi-user)

**Current State:**
ADR-0036's consequences say "daemon correctness and crash recovery become high-priority verification targets" — and the plan contains zero mentions of crash recovery, backup, index corruption, or daemon upgrade with in-flight continuations. The architecture is append-only and immutable at every layer (snapshots, packs, branches, transaction versions, audit log), and the entire dossier's treatment of growth is four scattered sentences; the plan has none. There is a direct unresolved conflict: receipts must remain reachable (append-only, `docs/44`) but a secret or proprietary source committed into the CAS has no purge path — and every promotion "re-fetches and verifies referenced artifacts" (`docs/49`), so naive deletion breaks receipts. Multi-user mode is asserted ("local or remote", "authenticated HTTP/QUIC") with no identity model, no capability minting/revocation, and an unanalyzed cross-user dedup side channel (`docs/35`: "Two clients requesting an identical task may share computation" — a content-existence oracle).

**Proposed Change:**
Add §4.5 "Operational contract of `continuumd`" covering: (a) crash-recovery invariants (commit-output-then-index per RFC 0030; task-table durability semantics; recovery of in-flight two-phase publications; index fsck); (b) storage lifecycle (reachability GC from named roots + retention policy; sizing telemetry; disk-full behavior under INV-017); (c) a purge design that preserves receipt verifiability — content encryption per artifact class with key shredding, leaving a typed `Redacted(reason, commitment)` stub so receipts verify structurally; (d) multi-user baseline: identity, capability issuance/scoping/revocation as first-class daemon operations, per-actor quotas, and cross-user dedup disabled by default (privacy-preserving sharing requires an explicit policy grant).

**Rationale:**
These aren't enterprise nice-to-haves; they are correctness properties of the trust spine. INV-017's atomic publication is unimplementable without a crash model. INV-015's "agents cannot sign receipts" implies a key-custody design no document provides. The purge conflict is the kind of issue that becomes an emergency the first time a customer's secret lands in a shared CAS.

**Benefits:**
- G1 (workbench identity, per Change #1) gains its crash-safety content.
- The redaction-vs-append-only conflict gets a design instead of a collision.
- The dedup side channel is closed before remote mode exists.

**Trade-offs:**
- Real design work. Scope it to invariants and mechanisms in the plan; full designs belong in an expanded RFC 0026/0030 (Change #12).

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ after ### 4.4 Content-addressed artifacts @@
+### 4.5 Operational contract of `continuumd`
+
+The daemon is part of the trust spine; its operational behavior is
+specified, verified, and gated (G1), not left to implementation:
+
+- **Crash safety.** Publication commits content before index; a crash
+  leaves unreachable content eligible for GC, never a stale index entry.
+  On restart, `Running` tasks resume from their last committed
+  continuation or transition to `Failed` with a typed reason — never to a
+  silently reconstructed state. An index verifier (fsck) ships with the
+  daemon.
+- **Storage lifecycle.** Artifacts are garbage-collected by reachability
+  from named roots, receipts, and retention policy. The daemon reports
+  storage attribution by artifact class. Disk exhaustion during
+  publication aborts atomically (INV-017); it never truncates.
+- **Purge without breaking receipts.** Sensitive artifact classes are
+  encrypted at rest per artifact; purge shreds the key and replaces
+  content with a typed `Redacted(reason, commitment)` stub. Receipts
+  referencing purged content remain structurally verifiable and report
+  the redaction; claims requiring the hidden data downgrade per §18.4.
+- **Multi-user baseline.** Remote mode requires an identity model;
+  capabilities are minted, scoped, delegated, and revoked through daemon
+  operations recorded in the audit log. Cross-user computation sharing is
+  off by default: content-addressed dedup across principals is an
+  existence oracle and requires an explicit sharing policy.
```

---

### [HIGH] Change #11: Carry the asupersync coupling and adoption risk with mitigations

**Current State:**
The plan hard-depends on asupersync (§0, §4.1 daemon regions, §14.7, §20, Phase B, INV-019) with no adapter-pinning strategy, no semantic-hook contract, no compatibility policy, and no kill signal. `docs/08` R06 (asupersync coupling; kill signal: "required hooks force invasive scheduler forks that cannot be stabilized") and `docs/20` Objection 3 (adoption: the async Rust world runs on tokio; kill signal present) are both absent from §24. `research/20` states the qualifier the plan omits: "'Same code in model and production' is only true inside controlled boundaries." The plan's kill criterion "real code requires pervasive rewrites solely to become observable" has no corresponding mitigation lane.

**Proposed Change:**
Add to §21/§24: (a) the adapter discipline (one adapter crate, exact version pin, semantic-hook contract, adapter conformance corpus — the Rev-2 controls); (b) a declared lower-assurance foreign-runtime lane (tokio instrumentation-only mode: journal what is observable, mark the rest opaque, envelope reflects it) as the adoption bridge and the mitigation for the rewrite kill criterion; (c) R06's kill signal and the boundary qualifier from `research/20` stated in §0.

**Rationale:**
A single-substrate bet with no hedge is the plan's largest adoption risk and its most likely kill path. The lower-assurance lane is cheap precisely because the assurance envelope (B11) already knows how to express "partially observed" — the architecture was built for this and the plan just never says so.

**Benefits:**
- Gives tokio-based projects an on-ramp that produces honest envelopes instead of exclusion.
- Converts the §24 rewrite criterion from a pure kill switch into a measured migration funnel.

**Trade-offs:**
- Risk of users camping on the low-assurance lane; the envelope makes the difference visible, and G10 (real adoption) can require at least one full-substrate migration.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ ## 24. Kill criteria list @@
 - real code requires pervasive rewrites solely to become observable;
+- asupersync integration requires invasive scheduler forks that cannot be
+  stabilized behind the pinned adapter contract (docs/08 R06);
+- the foreign-runtime instrumentation lane cannot produce useful envelopes
+  for tokio-based systems, leaving no adoption bridge;
@@ ### Phase B — Real-code failure loop, deliverables @@
 - asupersync semantic adapter;
+  (one adapter crate, exact version pin, explicit semantic-hook contract,
+  adapter conformance corpus; the model core never depends on asupersync);
@@ ### Phase F — Production and ecosystem, deliverables @@
 - production partial-order evidence;
+- foreign-runtime instrumentation lane (tokio): journal observable
+  lifecycle/channel/time events, mark uncontrolled effects opaque, and emit
+  correspondingly bounded assurance envelopes — an adoption bridge, not a
+  claim of controlled semantics;
```

---

### [MEDIUM] Change #12: Declare and pay the Revision-3 specification debt before interface freeze

**Current State:**
The seven RFCs carrying Phases A–C are stubs relative to their role: RFC 0037 (Intent Contract — root of B1, PR 4) is 694 bytes and doesn't cite the 6,939-byte JSON schema that is its real spec; RFC 0032 (repair transactions — Phase B exit) is 783 bytes with 10 stages that don't match §8.2's 12 gates; RFC 0031 (the anti-gaming diff) defines no classification lattice and no implication order for bounds/assurance though §5.4's policy verbs require one; RFC 0026 (the native protocol behind every adapter and 6 of the first 30 PRs) is 1,330 bytes with no IDL, no versioning, no pagination, no subscription semantics; RFC 0027 names zero tool signatures; RFCs 0028/0030 compress nine compiler stages and nine dependency classes into single sentences. The Rev-3 RFC cohort averages 4.7× thinner than Rev-2's while carrying more. None has Rejected Alternatives, Open Questions, target gates, owners, or RFC-2119 language; 16 of 17 Rev-3 ADRs likewise lack rejected alternatives and kill criteria, and 0 of 52 ADRs have the supersession rules their own README requires.

**Proposed Change:**
Add a "PR 0" to §25/`START_HERE`: before any interface freeze, expand the seven critical RFCs to Rev-2 mass (promote `schemas/intent-contract.schema.json` into RFC 0037; define RFC 0031's classification lattice and bounds/assurance orders; reconcile RFC 0032 with §8.2 one-to-one; give RFC 0026 an IDL and versioning rules), and backfill ADR metadata (dates, owners, rejected alternatives, kill criteria, supersession rules) for 0031–0052.

**Rationale:**
The plan freezes protocols in Phase A that exist only as plan prose. Freezing against stub RFCs means the freeze is fictional — the real spec will be whatever the first implementation does, unreviewed.

**Benefits:**
- Interface freeze becomes meaningful; disagreements between plan prose and RFC (currently ~15 documented cases) get resolved once, in the right place.
- ADR precedence becomes resolvable (currently two conflicting ADRs cannot be ordered — no dates).

**Trade-offs:**
- Real writing effort before code. It is cheaper than re-litigating frozen interfaces.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ ## 25. Immediate execution @@
 The exact first 30 pull requests are in [`notes/START_HERE_IMPLEMENTATION.md`](notes/START_HERE_IMPLEMENTATION.md).
+
+Before PR 5 (native protocol kernel) freezes any interface, the seven
+load-bearing Revision 3 RFCs — 0026, 0027, 0028, 0030, 0031, 0032, 0037 —
+are expanded from summaries to specifications: field types, enums,
+classification lattices, IDL, versioning, and RFC-2119 language, each
+reconciled one-to-one with this plan's corresponding section. Where plan
+prose and RFC disagree, the RFC is corrected and becomes normative. The
+plan is a map, not the spec.
```

---

### [MEDIUM] Change #13: Reconcile the operation namespace, status lattice, and result envelope

**Current State:**
Three related surface inconsistencies. (a) §3.2's example calls `verify.start`, which §10.2 never defines; the canonical name everywhere else (`docs/36`, `docs/55`, RFC 0026, `examples/agent_workflow.md`) is `verification.start`. §10.2 also lacks the entire debugger namespace (§7.2 lists 12 operations; RFC 0029 defines 11), lacks `repair.attach/review/reject` (which §8.5's review UX and swarm workflows depend on), and lacks the incremental explain operations §9.4 promises ("why did this re-run?"). (b) §11.4's status lattice omits `Sampled` (present in `docs/33` and the normative schema; required by §5.1's own gaming taxonomy — "lowering assurance from exhaustive to sampled" is inexpressible without it), omits the typed inconclusive reasons INV-008 demands, and cannot express solver-trust (`CHECKED_CERTIFICATE` vs `TRUSTED_SOLVER`, `docs/03`) or parameterized results (`checked(N=3)`/`cutoff_checked(N≤k)`/`proved(∀N)`, RFC 0016). (c) §0.2's universal result shape lacks the RFC 0026 envelope fields (`allowed next operations`, `warnings`, `cost`, `epochs`) — and "allowed next operations" is the mechanism that makes §10.1's "errors with recovery actions" safe.

**Proposed Change:**
Fix the example; complete §10.2; extend §11.4 and §0.2 as below.

**Rationale:**
These are the machine contracts agents code against; every divergence here becomes an integration bug or a spec fight later. All fixes are mechanical.

**Benefits:** Internal consistency; INV-008 becomes satisfiable by the plan's own lattice; §5.1's threat taxonomy becomes expressible in the evidence model.

**Trade-offs:** None.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ §3.2 example @@
-  "operation": "verify.start",
+  "operation": "verification.start",
@@ ### 10.2 Core operations @@
 workspace.create / fork / diff / seal
 intent.get / diff / propose_revision
+verification.start / result / await
 model.check / explore / compare
 program.extract / run / replay
 refinement.check / explain
 proof.goal / attempt / check / slice
+debug.open / state / enabled / step_event / step_abstract / reverse_causal /
+  branch / compare / why_enabled / why_blocked / export
 context.compile / expand
 failure.explain / minimize / branch
-repair.begin / apply / evaluate / promote
+repair.begin / apply / attach / evaluate / resume / review / promote / reject
 forge.create / step / archive / materialize
 benchmark.run
 task.status / cancel / resume / subscribe
 evidence.get / query / verify
+query.explain_reuse / explain_invalidation / clean_compare
@@ ### 11.4 Status lattice @@
 ```text
 Proposed
 Observed
+Sampled
 Bounded
 Validated
 Proved
 Refuted
 Inconclusive
 Superseded
 ```
 
-Only trusted services can promote into `Validated` or `Proved`. Agent votes or confidence cannot.
+Only trusted services can promote into `Validated` or `Proved`. Agent votes
+or confidence cannot.
+
+`Inconclusive` carries a typed reason per INV-008 (`Unsupported`,
+`ResourceExhausted`, `EngineError`, `InsufficientTelemetry`,
+`AbstractionAmbiguity`, `IncompleteProofSearch`). `Validated` records
+whether solver evidence is `CHECKED_CERTIFICATE` or `TRUSTED_SOLVER`; the
+two never render identically. Parameterized results carry
+`checked(N=k)` / `cutoff_checked(N≤k)` / `proved(∀N)` and are structurally
+distinct in every surface. Budget exhaustion is never a verdict.
@@ ### 0.2, the universal operation shape @@
 (snapshot, intent, operation, budget, policy)
   → verdict
   + evidence references
   + new snapshot or continuation
   + explicit omissions/unknowns
+  + allowed next operations, warnings, cost, and epoch identities
```

---

### [MEDIUM] Change #14: Make the five-minute path exemplary of the plan's own rules

**Current State:**
§3.1's flagship output violates B11 and `docs/34`'s DX contract: no intent version, no assurance envelope, no unknowns (5 of the 7 mandatory first-screen elements), and uses `FAIL` where the stable vocabulary reserves "failure" for a checked counterexample. §3.1 also configures via `[package.metadata.continuum]` while `examples/continuum.project.toml` defines an incompatible second surface with different keys — violating `docs/34`'s explicit anti-goal ("forcing users to maintain duplicate configurations"). And the path assumes `AckImpliesDurable` already exists; the plan protects intent everywhere but never says where a newcomer's first intent comes from — the hardest authoring step in the product.

**Proposed Change:**
Fix the sample output; declare one configuration surface (Cargo metadata for Rust projects, generating/overlaying the standalone file for model-only projects, one schema); add an intent-bootstrapping paragraph: `cargo continuum init` proposes draft intents (from templates, property libraries, and observed effects) that enter the registry at status `Proposed` and gain protection only on explicit human acceptance.

**Rationale:**
The first screen is the product's thesis statement; it currently demonstrates the anti-pattern (verdict without envelope) the plan spends 25 bets prohibiting. Intent bootstrapping is a genuine product gap: B10/§16 cover model↔program sync but nothing covers intent elicitation, and "protected but unauthorable" intent maximizes the temptation to game it.

**Benefits:** The showcase teaches the envelope habit from minute one; newcomers get a path to their first protected intent; the `Proposed` status already exists in the lattice, so bootstrapping needs no new machinery.

**Trade-offs:** Slightly busier first screen; `docs/34` already decided that trade correctly.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ ### 3.1 Five-minute human path, the first result block @@
 ```text
-FAIL  AckImpliesDurable
+FAILURE  AckImpliesDurable          intent in_7c2… (unchanged)
+assurance  bounded: ≤3 nodes · ≤2 faults · exhaustive schedules · SC memory
 
 Ack became observable before the corresponding write became durable.
 Causal core: 4 events · 2 tasks · 1 cancellation
 Abstract mismatch: acked +1, durable unchanged
+Unknown: production storage profile assumed (contractual)
 
 replay   cp_7m3...
 debug    continuum debug cp_7m3...
 explain  continuum explain cp_7m3... --level causal
+repair   continuum repair begin cp_7m3...
 ```
@@ after the config block in §3.1 @@
+`cargo continuum init` also proposes draft Intent Contracts from property
+templates, the domain-pack library, and observed effect footprints. Drafts
+enter the registry at status `Proposed`; they gain protection (INV-001)
+only on explicit acceptance. Continuum never silently promotes an inferred
+intent to protected status, and never claims a generated model is the
+intended abstraction.
```

---

### [MEDIUM] Change #15: Complete the Intent Contract's protected surface

**Current State:**
Four definitions of the intent field set disagree (§0.1, §5.2, `docs/33`, RFC 0037), and the normative schema (`additionalProperties: false`) cannot express `scope`, trust/opaque boundaries, or security/privacy policy — fields §0.1 promises. §5.4's lock example locks 6 fields, but ADR-0039 and RFC 0031 name 9 protected categories: as written, **fairness and opaque-boundary expansion are not lockable**, yet fairness-strengthening is the canonical attack in `docs/50`/`docs/51` and "opaque escape" is a named repair failure mode in `docs/41`. The intent also lacks fields other documents require it to have: domain-pack fidelity profile (RFC 0002 — §8.5's own example ends "production storage profile remains assumed" with nowhere to record the assumption), behavior-completion policy (RFC 0015 — a classic silent-divergence axis vs TLC), and typed nondeterminism classes (`research/06` makes untyped `choose` a hard prohibition).

**Proposed Change:**
Reconcile the field set to RFC 0037's 12 groups (schema updated to match); extend §5.4's example to the full protected set; add fidelity-profile, completion-policy, and nondeterminism-class fields.

**Rationale:**
Every unlockable protected category is a free gaming channel. The missing fields are exactly the assumptions that differ silently between model and reality — the plan's own §8.5 example demonstrates the need.

**Benefits:** Closes the two documented attack channels; gives §5.3's classifications a complete domain; makes the "unresolved: storage profile assumed" line expressible as data.

**Trade-offs:** Schema change (pre-1.0, cheap now, breaking later).

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ ### 5.2 Intent structure @@
 assumptions
   environment, fairness, timing, failure, trust
+  domain-pack fidelity profile per effect family
+    (ideal | contractual | platform-qualified | adversarial-envelope)
+  behavior completion policy
+    (stutter-forever | deadlock-violation | finite-trace-only | closed)
+  nondeterminism classes per choice site
+    (demonic | angelic | scheduler | probabilistic† | timed† | epistemic†)
+    († declarative until ADR-0016 lanes ship — see B11)
 observers
   state/event/knowledge/security projections
 scope
-  model/program components and abstraction level
+  model/program components, abstraction level, and semantic fragment
+  declarations (ADR-0025)
+trust boundaries
+  trusted and opaque components and effects
@@ ### 5.4 Intent locks @@
 [intent.policy]
 properties = "maintainer-review"
 assumptions = "maintainer-review"
+fairness = "maintainer-review"
+trust_boundaries = "no-expansion"
+non_vacuity = "no-removal"
+completion_policy = "maintainer-review"
 bounds = "no-decrease"
 faults = "no-removal"
 assurance = "no-downgrade"
 observers = "proof-required"
```

---

### [MEDIUM] Change #16: Restore the dropped risk controls and quantitative targets

**Current State:**
The plan's §23 metrics are entirely qualitative (the only number in the plan is an illustrative "+1.8%"), and §24 dropped multiple existential-class controls the dossier already designed: R04 (unsound POR — controls: conservative default, differential no-reduction oracle, certified claims fall back to baseline; *no Rev-3 gate checks reduction soundness at all*, though `docs/26` G2 and `docs/07`'s "zero reachability mismatch" did); R10 (domain packs lie — no fidelity/conformance language in the plan though the flagship durability demo rests on the storage pack); R19 (determinism under parallelism — `docs/19` §7's worker-count matrix is uncited while §9 introduces parallel evaluation); T13 (semantic downgrade — "no best-effort decode for evidence"); kill question 8 ("Would Quint + existing DST be cheaper and equally strong?" — the most falsifiable business test in the dossier); `docs/34`'s latency table and four ten-minute acceptance tests; `docs/49`'s "budget exhaustion is not a verdict"; and RFC 0034's grader ordering with `docs/45`'s "a zero in intent integrity caps the total at failure."

**Proposed Change:**
Fold the controls in as shown; adopt the latency table and acceptance tests by reference in §13.6/§21 Phase C; adopt the grader ordering in §19.3.

**Rationale:**
Each of these was already argued for and accepted elsewhere in the dossier; the plan dropping them is drift, not decision. The POR gap is the sharpest: INV-013 asserts property-scoped reduction as a constitutional invariant while no gate anywhere verifies any reduction against an unreduced oracle.

**Benefits:** §24 regains its most falsifiable tests; Phase C's "subsecond" exit becomes measurable; the benchmark can't be gamed by trading integrity for cost.

**Trade-offs:** More gates to pass. They were always supposed to be passed.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ ## 22, gate G5/incremental (per Change #1 numbering) or §21 Phase C exit @@
-Exit: normal edit/check/explain loop is subsecond for local changes and trustworthy under differential audit.
+Exit: the edit/check/explain loop meets the docs/34 latency table
+(p50/p95 per interaction) on the reference workload; incremental results
+are trustworthy under differential audit; reduction engines show zero
+reachability mismatch against the unreduced reference on the no-reduction
+corpus, and certified claims fall back to the unreduced baseline until the
+reduction's certificate lane matures (docs/08 R04); semantic artifacts are
+byte-identical across the docs/19 determinism matrix (worker counts,
+build modes, platforms, hash seeds).
@@ ### 19.3 Metrics @@
 assurance calibration
 security-policy compliance
 behavioral novelty
 (end of metrics code block)
+
+Grading is ordered per RFC 0034: intent integrity → security → semantic
+correctness → evidence validity → hidden-variant generalization → cost →
+explanation. A zero in intent integrity caps the total score at failure.
+Leaderboards are Pareto fronts; no single ranking hides cost or assurance.
@@ ## 24. Kill criteria list @@
 - users systematically misread bounded evidence as proof despite UX controls.
+- a candid comparison shows Quint plus existing DST tooling would be
+  cheaper and equally strong for the target users (docs/08 kill question
+  8); a "yes" after the real-repair gate closes is a program-level failure
+  signal;
+- domain packs cannot demonstrate conformance to their fidelity profiles
+  on real systems, making applied durability/network claims unearned
+  (docs/08 R10).
```

---

### [LOW] Change #17: Resolve naming overloads and the remaining mechanical defects

**Current State:**
(a) "Tribunal" names two unrelated subsystems (TLA+ oracle harness, ADR-0021; incremental clean-build audit, §9.5) — both appear in Phase C's deliverable list. (b) `cp_*` = crashpack while "CP" naturally reads Context Pack (`ctx_*`); both schemas exist. (c) §3.5 writes `models/broadcast.cml`; the canonical extension is `.ctm` in 2 ADRs, 4 RFCs, and all four checked-in model files (one more `.cml` leak sits in `schemas/examples/workspace-snapshot.example.json`). (d) §4.4 claims "opaque handles prevent clients from guessing structure" directly above a list of 11 self-describing prefixes, and the prefix taxonomy disagrees with `docs/55`'s (missing `task_`, `ev_`, `dbg_`, `ps_`). (e) §4.4's budget dimensions (7) exceed what the task schema can express (4).

**Proposed Change:** Rename §9.5's mechanism "Incremental Parity Audit"; rename crashpack prefix `crash_*`; fix `.cml`→`.ctm`; reword the opacity claim (handles are *unforgeable and structureless within a kind*, prefixes are kind tags); unify the prefix list with `docs/55`; extend the schema budget object.

**Rationale/Benefits:** Pure friction removal; each overload is a future onboarding bug or spec dispute.

**Trade-offs:** None.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ ### 3.5 Invention path @@
-continuum forge synthesize models/broadcast.cml \
+continuum forge synthesize models/broadcast.ctm \
@@ ### 9.5 @@
-### 9.5 Clean-build Tribunal
+### 9.5 Incremental Parity Audit
+
+(Renamed from "clean-build Tribunal"; **Tribunal** refers exclusively to
+the TLA+ corpus oracle harness of ADR-0021.)
@@ ### 4.4 Content-addressed artifacts @@
 ws_* workspace snapshot
 in_* intent contract
 model_* elaborated model
 cir_* causal execution graph
-cp_* crashpack
+crash_* crashpack
 ctx_* Context Pack
 proof_* proof artifact
+ps_* proof state
+task_* task
+ev_* evidence node
+dbg_* debugger branch
 receipt_* signed/checked receipt
 rt_* repair transaction
 forge_* synthesis archive
 cont_* resumable task continuation
@@ same section @@
-Opaque handles prevent clients from guessing structure. Authorization is checked independently of handle possession.
+Handles carry a kind prefix but are otherwise structureless and
+unforgeable; clients cannot derive one handle from another. Authorization
+is checked independently of handle possession.
```

---

### [LOW] Change #18: State the resourcing posture, licensing decision path, and human-study plan

**Current State:**
Zero content across all 296 files on team size, duration, or sequencing feasibility for a program spanning a language, five engines, a Lean metatheory, a certificate kernel, a daemon with an incremental database, four protocol adapters, a debugger, a synthesis engine, 80 corpus ports, a benchmark with hidden graders, and human-subject studies. Licensing gets one sentence in `docs/12` despite "public ContinuumBench" being a 1.0 gate over a corpus with per-example redistribution constraints, GPL-adjacent oracle tooling (TLC), and academic-restricted comparators. G3/G8 gate release on human task studies with no sample size, recruitment, or consent plan.

**Proposed Change:**
Add a short §21.1: (a) an explicit statement that the plan is gate-driven, not time-driven, plus a minimum-viable-team sketch per phase and the rule that a phase without an owner is `BLOCKED`, not in progress; (b) licensing decision as a Phase A task (product license compatible with asupersync; benchmark redistribution audit per corpus family; oracle tooling kept out of release binaries per ADR-0029); (c) a minimal viable study design for G8 (N, tasks from `docs/34`'s four acceptance workflows, preregistered metrics from `docs/48`) so the gate is passable by a small team.

**Rationale:**
A plan that cannot say what "adequately staffed" means cannot detect that it isn't. The licensing audit is cheap now and blocking at 1.0. The study gate needs a floor, or it silently becomes the reason G8 never closes.

**Benefits:** Feasibility becomes discussable; two known 1.0 landmines get defused early.

**Trade-offs:** Commits the project to statements it has avoided; that avoidance is itself the risk.

**Git-Diff:**
```diff
--- plan.md
+++ plan.md
@@ after ## 21. Implementation program heading @@
+### 21.1 Resourcing, licensing, and study posture
+
+Phases are gate-driven, not time-driven. Each phase names an owner and a
+minimum viable team; a phase without both is `BLOCKED`, not in progress.
+Phase A additionally resolves: the product license (permissive, compatible
+with asupersync and solver adapters); a per-family redistribution audit for
+the corpus before any public benchmark release; and the rule that foreign
+oracle tooling (TLC, Apalache, solvers) never ships in release binaries
+(ADR-0029). The G8 usability gate is defined against a preregistered
+minimal study: the four docs/34 acceptance workflows, cohort sizes and
+metrics fixed per docs/48 before the study runs.
```

---

## Appendix A — Additional defects catalogued (no diff; fix in the named artifact)

**Cross-document consistency (fix during Change #12's RFC/ADR pass):**
- Evidence-graph node kinds: plan §11.2 (19) vs schema (15) — `RepairHypothesis` and `BenchmarkResult` unrepresentable; invariant/ranking/abstraction candidates collapsed into one `candidate`, defeating `docs/43`'s counterexample routing. Edge types: plan 13 vs PR 7's 7 vs RFC 0038's 6.
- Request envelope field: `snapshot` (plan, docs/36/55, RFC 0026) vs `workspace` (required, in `verification-task.schema.json`).
- Repair-transaction schema: free-form gate names, `promoted` with empty gates validates, `receipt` nullable at `promoted` — the schema permits exactly what §8.2 forbids. Make the 12 gates an enum with conditional requirements.
- Context-pack schema: `assurance.envelope` is an untyped object (B11's nine dimensions unschematized); `guarantees` is free-form strings (adopt RFC 0028's enum; reconcile with `docs/38`'s seven minimality classes and `docs/48`'s seven guarantee classes — currently three different lists).
- Handle cross-references in all schemas share one untyped pattern; a `ws_` handle validates where an `in_` is required. Add per-kind patterns via `$defs`.
- Budget schema (4 keys) vs B18 (7 dimensions); no CPU/solver/proof budget expressible.
- `docs/53` is a byte-duplicate of `spikes/R3_SPIKE_REPORT.md`; `schemas/examples/diehard.corpus-port.json` is byte-identical to the only real port manifest — both double-count evidence in the validation tally.
- `docs/13` reuses section letters M, N, O, P; `docs/20` numbers Rev-2 objections 12–21 colliding with Rev-1's 12–15; `docs/18` C025 uses an undeclared state; `docs/01` says M0–M8 where `docs/23` says M0–M7.
- TV-009 `port.json`: placeholder source hash, spike-as-own-oracle evidence links, `expected_labeled_transitions = 96` freezes an encoding artifact (16 states × 6 unconditional actions incl. self-loops) as a parity fact — the thing `PARITY_LEVELS.md` itself warns against. TV-007 has no manifest at all.
- ContinuumBench seed tasks reference nonexistent handles (`ws_demo1`, `in_ack_v1` resolve only to schema fixtures); grading is four booleans with no rubric, no graders, no hidden variants, no baselines.
- `tools/validate_dossier.py` cannot re-run here (`jsonschema` not installed); its Lean check is three regexes; its fence check is line-parity only. Add: spike-output-vs-schema cross-validation, gate-citation checking, and a duplicate-file check.

**Spike-quality items (fix before promoting any spike to "evidence" under Change #3's labels):**
- `agent_protocol_spike`: continuations are replayable indefinitely (tension with INV-009); 80-bit truncated digests (PR 2 demands collision tests); the "stale snapshot" case tested is not the G0-DX-03 case.
- `evidence_graph_spike`: authority is a self-declared field — encode at least a token-based authority check before citing it against G0-DX-12.
- `causal_debugger_spike`: `why_enabled`, `abstract_difference`, and reverse-causal outputs are hardcoded literals.
- Lean: `Fairness.lean`'s sole theorem is vacuous (discards its enabledness premise); `Examples/DieHard.lean` proves a list-index equality with no transition system — yet TV-009's manifest cites it as P4 evidence; `Interaction/*.lean` "theorems" are conjunct projections over uninterpreted `Prop` fields. State these as scaffolding, or replace them, before any "proof seeds" language survives Change #3.

**Research-program hygiene (feeds Change #8):**
- Backfill kill criteria/baselines for notes 03–05, 08–20, 22–24 (19 notes) per the research README's own contract; add `Claim class:` headers to notes 11–36.
- Cross-note arbitration needed: three competing treatments of cancellation-drain progress (04/09/10); sheaf gluing proposed independently three times (03/07/23) with 07's own kill condition predicting the SAT baseline wins; note 01's headline soundness precondition is what note 13 calls "as hard as verification."
- Adopt the concrete mechanisms the notes define but the plan omits, prioritized: monitorability verdict before production data (05); operational assumption type + non-deployable flag (15); counterstrategies before assumption synthesis (15) — currently Forge synthesizes fairness assumptions, the exact artifact §5.1 classifies as gaming, with none of the three controls notes 04/15 require; equivariance obligations (14); evidence noninterference and the three-channel instruction/data/authority architecture (35); trajectory evidence score and baseline ladder (33); accessibility requirements incl. non-visual causal-graph access (34) — currently gated by `docs/52` G8 but dropped from plan G3.

**Interop (feeds Changes #9/#11):** the plan's only ecosystem posture is the oracle tribunal. Adopt `research/20`/`docs/10`'s concrete deliverables: generate Kani harnesses and Verus-style transition invariants from finite fragments; consume verifier receipts as domain-pack assumptions; import Loom/Miri/sanitizer output as testing evidence (never proof); ITF trace import; and the bespoke-DST deletion gate (5 conditions) that Phase F's migration exit implicitly requires.

---

## Appendix B — What was verified clean

- `SHA256SUMS.txt` verifies against the tree.
- The 80-row corpus CSV is internally consistent (IDs, waves, parity distributions, category tallies) and matches `corpus-summary.json` and the checker.
- The reference kernel's closure-certificate checker genuinely recomputes successors rather than trusting the explorer — the right shape for the real kernel.
- The cyclic-symmetry spike does real exhaustive work (573 states × 5 rotations, 4.9× quotient).
- `intent-contract`, `assurance-result`, `cir`, `domain-pack`, and `crashpack` schemas are substantive.
- Plan-internal link targets (`notes/G0_SPIKE_MATRIX.md`, `notes/START_HERE_IMPLEMENTATION.md`) resolve correctly relative to `plan.md`.
- The dossier's self-criticism (`docs/20`, `docs/31`, VALIDATION_REPORT boundaries, spike-report caveats) is of unusually high quality — most of this review's raw material was already in the dossier; the failure mode is that `plan.md` doesn't consume it.
