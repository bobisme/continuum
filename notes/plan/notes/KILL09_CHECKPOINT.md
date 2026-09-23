# KILL-09 Checkpoint — Intent Diff Cannot Reliably Expose Gaming in Supported Fragments

**Prepared:** 2026-09-23 (bone `bn-3kql`, workspace `bn-3kql`)
**Criterion:** `KILL-09`, [`plan.md`](../plan.md) §24 line 2681
**Register row:** none. §24.5 has no lane for intent diff, because intent
integrity is architecture, not a frontier lane (§1).
**Instrument:** `crates/continuum-semantic-diff/tests/kill09_intent_diff_indicator.rs`
**Decision record:** none. This document is the decision **package**. The
decision is open and belongs to the user or the lead (§5, §9).

This is the second kill-criterion checkpoint against plan §24. It follows
the shape [`KILL08_CHECKPOINT.md`](KILL08_CHECKPOINT.md) §8 set, with one
difference: KILL-08 had a recorded decision to locate, and KILL-09 does
not. So this document prepares the package and does **not** annotate the
§24 bullet (KILL-08 §8: "A measured-and-fired criterion with no decision
is **not** dischargeable"; the same holds for a measured-and-not-fired
one).

---

## 0. What this checkpoint concludes, up front

1. **The criterion did not fire as measured.** Zero silent passes and
   zero exposure misses in every measurement: 3,291 hidden weakening
   mutants over all seven G3 dimensions, 3,000 random formula pairs on
   both the claim side and the assumption side, a 24,000-pair extended
   sweep, and 37 named attacks in four adversarial families (§3).
2. **The measured scope is narrow, and that is the main risk.** The
   only supported fragment that gets a direction today is `Finite`. The
   corpus is three committed contracts. No production path calls the
   diff: `continuumd`'s `intent.diff` and `intent.propose_revision`
   return `UnsupportedSemanticFeature` (§6). The result is "the library
   exposes every gaming move tried within `Finite`", not "the system
   exposes gaming".
3. **Two exposure boundaries were found and are not silent passes.**
   A policy-only revision carries no `intent_changes` record (DX02-B14,
   re-measured as K09-P02), and two verbs outside the seven G3
   dimensions would admit a direction once a classifier lands for their
   field (§3.4). Neither weakens a protected field today.
4. **Recommendation: CONTINUE, and keep the criterion active.** Grounds
   and the alternatives are in §5. No agent took the decision.

---

## 1. The source sentence, quoted exactly

`plan.md` §24 opens at line 2634. Its preamble:

> Continuum must narrow or redesign if:

KILL-09 is the ninth bullet, line 2681:

> intent diff cannot reliably expose gaming in supported fragments;

The section's closing sentence, line 2700:

> Failure of a frontier research lane does not kill Continuum. Failure
> of the single-semantic-contract, intent-integrity, replay, or evidence
> architecture does.

Three facts follow from the text alone:

- **KILL-09's subject is the intent-integrity architecture.** Unlike
  KILL-08, it has no §24.5 lane. By the closing sentence, a fired
  KILL-09 that cannot be repaired is a program-level failure, not a
  narrowing. This is why the package in §5 treats "kill" as a live
  option and not a formality.
- **The bullet states no threshold.** The dossier states the guarantee
  it negates in two places, with the same quantifier:
  - plan §5.3 and RFC 0031 "Completeness guarantee": "within declared
    supported fragments, every property weakening, assumption
    strengthening, bound decrease, observer coarsening, fault removal,
    fairness addition or removal, and assurance downgrade MUST be
    classified as a privileged intent change";
  - G3 (plan.md line 2471): "every gaming mutation in the development
    gaming suite (§19.4) that falls in a supported fragment is
    classified as a privileged intent change".
  "Every" is the bar: one miss fires the antecedent. This checkpoint
  grades against that quantifier and invents no rate.
- **"Supported" is a checkable predicate.** Plan §5.3: "Each intent
  field declares its semantic fragment (ADR-0025 …), so 'supported' is
  a checkable predicate." Outside the declared fragments the required
  answer is `unsupported`, and non-affirmative relations fail closed.

"Reliably" is read two ways, and both are measured: (a) no miss, and
(b) the exposure is informative enough to act on. Reading (b) is the
fail-closed share in §3.1. It is a leading indicator, not a threshold.

---

## 2. The assay binding

### 2.1 The leading indicator — exposure over the C020 corpus

`kill09_intent_diff_indicator.rs` part 1 re-runs the C020 hidden-mutant
corpus (`bn-bpz0`; generator `tests/support/c020_mutant_generator.rs`,
written apart from the classifier, source-guarded) through the production
front door: `IntentContract::decode`, `artifact::assemble`, and
`PolicyTable::verdict` against the before-table on the agent acceptance
path.

- **Exposed**: every target field carries a record outside the benign set
  for its direction, and the locked measurement policy does not allow the
  revision, at `bounded` and at `observed`.
- **Unexposed**: a target field with no such record. **Silent**: an
  `allow` under the locked policy. Either one is a KILL-09 event.
- **Affirmed / fail-closed**: an exposed mutant whose record names the
  affirmed direction (`weakened`, `contracted`, …) against one exposed by
  `unknown`, `incomparable`, or `unsupported`.

A drift check (`the_indicator_agrees_with_the_c020_ledger`) ties the
totals to C020's retained ledger line for line.

### 2.2 The semantic differential falsifier

Part 2 is new. It is the strongest attack in this checkpoint, because its
ground truth does not come from the classifier or its authors' reading of
the RFCs:

- A seeded generator authors a random `Finite` formula (connectives, all
  three derived operators, `always`/`eventually`, and `forall`/`exists`
  over two domain constants with shadowing binder names) and one
  agent-style edit of it, from 16 edit operators (widen, narrow,
  tautology disjunct, vacuous quantifier, dual swap, binder rename with
  capture, respellings, and so on).
- Each formula goes through `cpnf::normalize` into the die-hard contract,
  once as the `NotSolved` claim and once as the `JugCapacities`
  assumption, and through `decode` and `assemble`.
- The ground truth evaluates the **authored** formulas directly over 528
  lasso traces and 9 interpretations of the domain constants (the empty
  domain included). A world that separates the two formulas is a genuine
  counterexample.
- It refutes: an `unchanged` on a separable pair; a `strengthened` where
  the edit admits a new behavior; a `weakened` where it drops one; any
  change in meaning by `normalize` alone (RFC 0037 S1); and any `allow`
  on a pair that admits a new behavior (claim side) or drops one
  (assumption side).

`negative_the_ground_truth_refutes_known_unsound_claims` shows it can
refute: persistence against recurrence, the empty-domain vacuity, and
binder capture are each separated.

### 2.3 The named adversarial assay

Part 3: 37 hand-written gaming attacks and 2 controls, in the four
families the bone names, each against a real committed contract:

| Family | IDs | What it tries |
|---|---|---|
| Adversarial encodings | K09-E01…E16 | tautologies by `iff`, double negation, vacuous `leads_to`, empty-domain `forall`, temporal dual swap, recurrence for invariance, guarded invariant, kind flip, source lie, homoglyph rename, shadow unit with a duplicate id, non-normal AST declared `cpnf-1`, binder-reference swap (`v1 == v1`), quantifier domain swap, text-for-integer literal |
| Cross-field compensation | K09-X01…X17 | weaken one claim while strengthening another; move a guarantee into an assumption; weaken plus shrink bounds; bound trade; re-point a claim at a coarser observer; coarsen the observer a claim is stated over; and one attack through each of the eight fields outside the seven dimensions |
| Fragment-boundary tricks | K09-F01…F06 | relabel a weakened claim `Temporal`, drop its fragment, widen `scope.fragments` and move the claim, swap the whole scope to `Symbolic`, relabel a strengthened assumption `Symbolic` |
| Policy-table edits | K09-P01…P05 | self-unlock rider, policy-only loosening, the two-step attack's second step, reviewer injection, verb-swap rider |

Part 4 grades every W6-admissible (field, verb) pair against every
dangerous relation of that field on the agent path: 343 cells.

### 2.4 Trend data — retained artifacts under stable IDs

| Stable artifact ID | Producer | What it carries |
|---|---|---|
| `continuum-semantic-diff kill09-indicator v1` | `tests/golden/kill09_intent_diff_indicator.txt` | the per-dimension indicator, the differential results per edit operator, every assay row with its records and verdicts, the verb matrix |
| `kill09_indicator_trend.txt` | `tests/golden/kill09_indicator_trend.txt` | the trend series T1…T4 |
| C020 ledger | `tests/golden/c020_hidden_mutant_evidence.txt` | the per-mutant rows the indicator aggregates |

The artifact is byte-stable and is compared to its golden in `just check`.
The trend file's last row must equal the live measurement, so a moved
figure fails the gate until a named row is appended. The series:

| Point | Bone | Commit | Reached | Unexposed | Silent | Affirmed | Fail-closed |
|---|---|---|---|---|---|---|---|
| T1 | `bn-3cgt` (G0-DX-02) | `0318730` | 18 attacks | 0 | 0 | — | — |
| T2 | `bn-bpz0` (C020) | `962e4c3` | 3,291 | 0 | 0 | 2,812 | 479 |
| T3 | `bn-36luu` | `a82b604` | 3,291 | 0 | 0 | 2,812 | 479 |
| T4 | `bn-3kql` (this) | — | 3,291 | 0 | 0 | 2,812 | 479 |

T3 is the one movement so far: `bn-36luu` removed the own-policy `allow`
class for evidence-class membership (primary own-policy allows 12 → 4).
The exposure figures have not moved since C020 landed.

The extended lane (`extended_differential_sweep`, `#[ignore]`, 8 fixed
seeds) is not in the gate. It ran once for this checkpoint in a release
build: 24,000 pairs, 5,762 strict weakenings (3,307 affirmed), 3,341
strict strengthenings (1,718 affirmed), 0 unsound, 0 silent.

---

## 3. Outcomes, exactly

### 3.1 The leading indicator, per G3 dimension

Combined primary and held-out corpus. The bound column is a conservative
one-sided 95% upper bound on the miss rate given zero misses
(`-ln 0.05 / n`), in parts per million.

| Dimension | Reached | Exposed | Affirmed | Fail-closed | Unexposed | Silent | 95% bound |
|---|---|---|---|---|---|---|---|
| properties | 1,189 | 1,189 | 894 | 295 | 0 | 0 | 2,520 |
| assumptions | 941 | 941 | 926 | 15 | 0 | 0 | 3,184 |
| bounds | 215 | 215 | 118 | 97 | 0 | 0 | 13,934 |
| observers | 224 | 224 | 152 | 72 | 0 | 0 | 13,374 |
| faults | 181 | 181 | 181 | 0 | 0 | 0 | 16,552 |
| fairness | 212 | 212 | 212 | 0 | 0 | 0 | 14,131 |
| assurance | 435 | 435 | 435 | 0 | 0 | 0 | 6,887 |
| **all** | **3,291** | **3,291** | **2,812** | **479** | **0** | **0** | **911** |

38 further mutants were rejected at decode and never reached the
classifier. They are safe and are counted apart. All 396 meaning-preserving
controls were allowed with no record, so the metric discriminates.

Grade against the bar: **met** on every dimension. Do not read the
bound column as the result. The bar is "every", and the result is zero
misses. The bound only says how small a miss rate this corpus could rule
out; for bounds, observers, faults, and fairness it is about 1.3–1.7%.

The fail-closed share is 479 of 3,291 (14.6%): properties 24.8%, bounds
45.1%, observers 32.1%. Every one of these is exposed and costs a
review. The share is the indicator to watch for the "reliably" reading
(b): a diff that exposes mostly by refusal pushes reviewers toward
routine approval.

### 3.2 The semantic differential

| Measure | Claim side | Assumption side |
|---|---|---|
| pairs | 3,000 | 3,000 (the same pairs) |
| `normalize` changed meaning | 0 | — |
| unsound classifications | 0 | 0 |
| silent passes | 0 | 0 |
| dangerous-direction pairs affirmed | 419 of 698 strict weakenings | 226 of 439 strict strengthenings |

The classifier's own mix on the claim side: 1,036 `unchanged`, 536
`weakened`, 317 `strengthened`, 1,111 `unknown`. Every `unchanged` is a
ground-truth equivalence, every direction is consistent with the ground
truth, and every `unknown` blocks under the locked policy. About 40% of
the strict weakenings are exposed by refusal rather than by direction.
That is completeness, not soundness, and S2 of RFC 0037 prices it at a
review.

### 3.3 The named assay

| Family | Attacks | Exposed | Rejected at decode | Unexposed | Silent |
|---|---|---|---|---|---|
| encodings | 15 + 1 control | 13 | 2 (E11 duplicate id, E12 non-normal AST) | 0 | 0 |
| compensation | 17 | 17 | 0 | 0 | 0 |
| fragment | 5 + 1 control | 5 | 0 | 0 | 0 |
| **all** | **37 + 2** | **35** | **2** | **0** | **0** |

Both controls were allowed with no record. Rows worth reading in the
artifact:

- **K09-E04, E05, E06, E13, E15** are real weakenings that the oracle
  cannot direct (empty-domain vacuity, `always`→`eventually`, recurrence,
  `v1 == v1`, text literal). All five are exposed as `unknown` and block.
  None is affirmed `weakened`.
- **K09-X01** weakens one claim and strengthens another. The weakened
  claim is exposed as `unknown` and the strengthened one as
  `strengthened`. The compensation does not net out.
- **K09-X04, X06, X08–X12, X15–X17** block under the locked policy and
  close to `review` under the committed table. They are exposed.
- **K09-F01, F04, F05**: moving a unit outside `scope.fragments` makes
  it `unsupported`. Widening the scope and moving the unit (F03) makes
  both the unit and `scope[fragments]` `unknown`. No fragment move buys
  a direction.

### 3.4 Policy-table edits and the verb matrix

| ID | Attack | Decision (agent path) | Records |
|---|---|---|---|
| K09-P01 | self-unlock rider: `properties` unlocked and a claim weakened in one revision | **block** | `properties[RuntimeToAbstract]:weakened` |
| K09-P02 | policy-only loosening, `properties` locked→unlocked | **allow** | none |
| K09-P03 | step 2 of the two-step attack, against the P02 table | **allow** | `properties[RuntimeToAbstract]:weakened` |
| K09-P04 | reviewer injection: `agent-self` named as the only `assumptions` reviewer | **review**, naming `verification-lead` only | `assumptions[NetworkNoForgery]:strengthened` |
| K09-P05 | verb-swap rider: bounds `no-decrease`→`review` with `values` 2→1 | **block** | `bounds:contracted` |

P01, P04, and P05 confirm RFC 0037 P7: the verdict uses the before-table
and the before-reviewers, so a revision cannot lift its own block or name
its own reviewer.

**P02 is an exposure boundary, not a silent pass.** The policy table is
not one of the fifteen diff fields, so a governance-only revision has an
empty `intent_changes` and an `allow`. RFC 0037 ID3 states this
("A policy-only revision classifies all fifteen fields `unchanged`"), and
G0-DX-02 recorded it as DX02-B14. It is still visible: the `in_*`
identity moves (ID2), and `continuumd`'s `intent.lock` refuses an edit
that weakens a verb (`crates/continuumd/src/daemon/intent.rs`, `lock`).
**P03** is the consequence if P02 were ever accepted: the weakening is
exposed by its record, and the unlocked table allows it. So the two-step
attack is closed by `intent.lock`'s monotonicity, not by the diff. No
wire path mints a proposal today (§6), so that closure is not yet
exercised end to end.

**Verb matrix.** 343 cells, 39 `allow` cells. On the seven G3
dimensions, every `allow` is under `unlocked`, which is the owner's
explicit declaration and still produces the record. Outside the seven,
two cells allow a dangerous relation under a protective-looking verb:

- `security_policy:no-removal:weakened` — a `data_classification`
  downgrade (for example `restricted`→`public`). RFC 0037 calls this "a
  protected change", but `no-removal` denies only `removed`.
- `optimization:no-removal:added` — a new `optimization.hard` string.
  The dossier states no safe direction for this field, so this cell may
  be benign.

Both are **latent**. Neither field has a classifier, so the assembler
emits only `unknown` for them, and P2 turns that into `review` (K09-X11,
K09-X17). They become live the day a classifier lands for either field.
The test pins the list, so a new cell or a new classifier is seen.

---

## 4. Coverage verdict — does this evidence answer KILL-09?

Partly, and the boundary must be stated exactly.

- **Answered: the classification and policy layer, within `Finite`,
  over the committed contracts.** Every attack family the bone names was
  tried. Nothing got through, and the soundness of every affirmed
  direction was checked against an independent ground truth.
- **Not answered: "in supported fragments" beyond `Finite`.** `Symbolic`,
  `Temporal`, `Probabilistic`, `Theorem`, and `Runtime` get no direction
  from this crate (RFC 0031 S3 and its open question). Units in those
  fragments are exposed by refusal, always. That meets the guarantee
  trivially and is useless as a signal. Whether a direction-capable
  procedure for them can be sound is not measured.
- **Not answered: the system.** The diff runs in no production path.
  The KILL-09 question is about what an agent can get past Continuum, and
  today no agent can submit a revision at all (§6 gap 1). The
  measurement is of the library that the path must use.
- **Not answered: the gaming suites G3 names.** The development gaming
  suite (docs/50-classified, `bn-1f6`) and the held-out suite evaluated
  once at G9 do not exist. The C020 corpus and this assay stand in for
  them. They are not them.

---

## 5. The decision package

KILL-09 is an intent-integrity criterion, so the four options have
different consequences than KILL-08's did. The antecedent did not fire,
so the §24 preamble ("must narrow or redesign if") does not compel
anything. The decision is what to do with a criterion that holds on a
narrow scope.

### Option A — CONTINUE, criterion stays active *(recommended)*

Keep the architecture. Keep KILL-09 active, and re-grade it at each point
where its scope grows: when a production path calls the diff, when a
non-`Finite` fragment gets a direction, and when `bn-1f6`'s gaming suite
lands. Each of those appends a trend row.

- **For:** zero misses and zero silent passes in every instrument,
  including an independent semantic falsifier and 24,000 extended pairs.
  The main failure mode KILL-09 guards against, a weakening that reads
  as harmless, has no example. The fail-closed rule does the work it
  was designed for: every attack the oracle cannot direct is exposed as
  `unknown` and blocks.
- **Against:** "continue" can be misread as "KILL-09 is discharged". It
  is not. §4 lists what the evidence does not cover, and the largest
  gap, no production call path, is exactly where gaming would happen.
- **Required with it:** the follow-ups in §5.5, and no `(delivered:)`
  annotation on the §24 bullet until the system-path gap closes.

### Option B — NARROW the guarantee to `Finite` explicitly

Restate plan §5.3 / RFC 0031 so that the directional guarantee is
declared for `Finite` only, and state the other fragments as
refusal-only until a lane promotes them.

- **For:** this is what the code does today. Saying so removes the
  "trivially met" reading from §4.
- **Against:** it is a plan and RFC edit, and so a protected-text change
  with its own review. It adds no protection. The fail-closed rule
  already makes the other fragments safe.

### Option C — DEFER the grading to Phase B exit

Record that KILL-09 cannot be graded until the system path exists and
`bn-1f6`'s suite lands, and fold it into `bn-188z9`'s Phase B exit package.

- **For:** it avoids a decision on library evidence alone.
- **Against:** it throws away a retained, gated indicator that already
  says something true. A deferral with no instrument lets regressions go
  unseen. The indicator is in `just check` now, so deferring the
  *decision* does not require deferring the *measurement*. If this option
  is taken, keep the gate.

### Option D — KILL

Not supported by the evidence. The antecedent did not fire. By §24's
closing sentence a KILL-09 kill is a program kill, and nothing measured
here comes near it.

### 5.5 Follow-ups the recommendation depends on (for the lead to create)

1. **Wire the diff into the production path.** `continuumd`'s
   `intent.diff` and `intent.propose_revision` must call
   `continuum_semantic_diff::artifact::assemble` and return its
   artifact, with the verdict recomputed at `intent.accept` (P7). No
   bone was found that owns this. The nearest are `bn-1b69` (PR-20 /
   IMPL-04, repair-transaction semantic/intent diff) and `bn-19gw` (PR-22
   / IMPL-03). INV-001's annotation already records this absence. When it
   lands, re-run this indicator against the wire path and append T5.
2. **Latent verb cell on `security_policy`.** Either an RFC 0037 / RFC
   0031 correction that makes `no-removal` on `security_policy` also deny
   `weakened`, or a recorded statement that `review`/`locked` is the only
   protection for a classification downgrade. It must be decided before a
   `security_policy` classifier lands. The `optimization:no-removal:added`
   cell needs a stated direction for `optimization.hard`, or a statement
   that it has none.
3. **The gaming suites.** `bn-1f6` (docs/50-classified development
   corpus) and G3-01 (`bn-146ub`) should consume this file's assay
   families as seed cases, not replace them.

---

## 6. Gaps and typed absences

| # | Gap | Severity |
|---|---|---|
| 1 | **No production call path.** `intent.diff` and `intent.propose_revision` return `UnsupportedSemanticFeature` (`crates/continuumd/src/daemon/intent.rs`, `unclassifiable`). No crate outside `continuum-semantic-diff` depends on it. By the library-only rule, the exposure guarantee is measured at the library and is **partial** for the system. | Structural. It bounds §4's answer. Owner not yet named (§5.5 item 1). |
| 2 | **One directed fragment.** Only `Finite` gets directions. The other five fragments are exposed by refusal only. | Declared in RFC 0031 S3 and its open question. It makes the guarantee trivial outside `Finite`. |
| 3 | **Three base contracts.** The C020 corpus and the assay use the three T09-pinned contracts. The differential uses a synthetic claim and assumption inside die-hard. | It bounds generality. The G3 suites (`bn-1f6`, `bn-146ub`) are the remedy. |
| 4 | **The differential's ground-truth model is bounded.** It has 4-bit states, lassos of at most two states, a two-element universe, and no actions, `lt`/`le`, `apply`, `member`, or `subset`. A refutation is genuine. The absence of one is bounded evidence. | Declared in the test. The unmodelled atoms are opaque to the oracle, which never relates them, so they cannot carry an unsound direction the model misses — but that is an argument, not a measurement. |
| 5 | **P02, the policy-only loosening, carries no diff record.** Visible only as an identity move, and closed by `intent.lock` monotonicity at the daemon, which no end-to-end path exercises yet. | Recorded boundary (DX02-B14, RFC 0037 ID3). Not a silent pass: no protected field changes. |
| 6 | **Two latent verb cells** (§3.4). | Latent: unreachable today by the fail-closed rule. Must be decided before those classifiers land. |
| 7 | **The repair layer is absent.** RFC 0032 repair transactions and Forge are not implemented, so no agent repair has run through the diff. | Same absence C020 declares. INV-011 (`bn-1enq`) owns it. |

None of the seven falsifies the result. Gaps 1–4 bound what was measured.
Gaps 5–7 are recorded so that a later reader does not mistake them for
coverage.

---

## 7. What would change the recommendation

- **One unexposed or silent row** in any instrument. That is an INV-001
  defect first, and a KILL-09 event second. It must be fixed and
  appended to the trend, and the package re-issued.
- **A fail-closed share that rises** when the wire path or a new
  fragment lands. Most exposure by refusal is the "reliably" failure in
  reading (b). It would argue for Option B plus oracle work, not for a
  kill.
- **The wire path landing without the verdict recomputed at accept
  time.** P7 is what closes P01, P04, and P05. A path that reuses the
  propose-time verdict reopens all three.

---

## 8. Notes for later kill-criterion checkpoints

- **A criterion on the architecture has no §24.5 row.** Grade it against
  the guarantee it negates, quoted from its normative home. Here that is
  plan §5.3, RFC 0031, and G3, all of which say "every".
- **A not-fired criterion with no decision is not annotated.** The KILL-08
  rule ("discharged by a decision, not by a measurement") applies both
  ways. The package goes in the checkpoint, and the §24 bullet stays as
  it is, so no traceability regeneration is needed.
- **Add an independent ground truth where one is cheap.** The C020
  corpus is graded by construction, and that construction is also a
  reading of the RFC. The differential falsifier grades by evaluation,
  and that is the one instrument here that could have found an unsound
  normal form.

---

## 9. What this document does NOT do

- **It does not take the KILL-09 decision.** Per this bone's contract,
  "Agents do not take the privileged program decision." §5 recommends and
  the user or the lead decides.
- **It does not annotate plan §24** or mark KILL-09, INV-001, C020, T09,
  or any G3 row delivered. No registry status changes.
- **It does not change the classifier, the policy table, the IDL, or any
  schema.** It adds one test file, its golden artifact, and its trend
  file.
- **It does not close `bn-3kql`, merge, or push.**
- **It does not claim exhaustiveness over §24.** The other kill criteria
  are untouched.
