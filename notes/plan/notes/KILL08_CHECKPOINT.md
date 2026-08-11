# KILL-08 Checkpoint — Agent-Native API Does Not Beat Disciplined CLI Use

**Prepared:** 2026-08-11 (bone `bn-3ety`, workspace `bn-3ety`)
**Criterion:** `KILL-08`, [`plan.md`](../plan.md) §24 line 2642
**Register row:** §24.5 "Agent–computer interface (B2, §10)", lane
research/25, [`plan.md`](../plan.md) line 2722
**Instrument:** G0-DX-10, [`G0_SPIKE_MATRIX.md`](G0_SPIKE_MATRIX.md)
**Decision record:** bone `bn-762i`, user-ratified 2026-08-11

This is the **first** kill-criterion checkpoint executed against plan
§24. It binds the criterion to retained evidence, states the per-margin
outcomes exactly, and records the privileged decision that has already
been taken. It does not take that decision, and §9 says so at length.

---

## 0. What this checkpoint concludes, up front

1. **The criterion fired as measured.** The agent-native API does not
   beat disciplined CLI use on the ratified composite. It loses on
   interface bytes, ties on task success, and wins on invalid actions.
   §3 gives the three margins with their figures.
2. **The consequence §24 names — narrow or redesign — was carried
   out.** §24.5's row names the same fallback in the same words
   ("protocol redesign before freeze"). The redesign is executed to the
   protocol major-3 boundary and the remainder is ratified as the
   standing 4.0 plan (§2.5).
3. **The decision is CONTINUE with redesign-as-waste-removal, and it
   was taken by the user on 2026-08-11** as option 1 of `bn-762i`'s
   freeze adjudication package (§5). No agent took it, and this
   document does not take it again.
4. **The ratification answers KILL-08's exact question.** §6 is the
   verify-coverage argument, made against the criterion's own sentence
   and against the three kills its register row names, not against a
   paraphrase.
5. **One margin is unreachable rather than unmet**, and that distinction
   is load-bearing for the decision: RFC 0027 correction 31's addendum
   proves the +30% bytes floor cannot be cleared by any lossless
   protocol under the pinned accounting, so a redesign aimed at flipping
   that row would have to stop being a typed surface (§2.4).

---

## 1. The source sentence, quoted exactly

`plan.md` §24 opens at line 2620. Its preamble, line 2622:

> Continuum must narrow or redesign if:

KILL-08 is the eighth bullet of that list, line 2642, and reads exactly:

> agent-native API does not beat disciplined CLI use;

The section's closing sentence, line 2686, scopes what a fired bullet
may cost:

> Failure of a frontier research lane does not kill Continuum. Failure
> of the single-semantic-contract, intent-integrity, replay, or evidence
> architecture does.

Three facts follow from the text alone, before any measurement:

- **The consequence of a fired KILL-08 is "narrow or redesign", not
  "kill".** The list's preamble is the operative verb phrase; the
  bullets are antecedents.
- **KILL-08's subject is a frontier research lane.** §24.5 assigns
  "Agent–computer interface (B2, §10)" to research/25. By the closing
  sentence, KILL-08 firing cannot kill Continuum.
- **The bullet states no threshold.** "Beat" is defined elsewhere — by
  the §24.5 row's ratified `quote-id=aci-benchmark-margins` sentence,
  which is the only place in the dossier where the comparison is
  numeric. A checkpoint that graded KILL-08 against its own invented
  reading of "beat" would be inventing a threshold on the lane's
  behalf, which §24.5 forbids.

The ratified sentence, quoted from `plan.md` line 2722:

> On the agent benchmark, with an identical base model, task set, and
> per-task budget, native ACI must beat the disciplined-shell baseline
> by at least 10 percentage points of absolute task success, at least
> 30% fewer interface bytes per solved task (bytes, not tokens, are the
> graded cost denominator per RFC 0027), and at least a 50% relative
> reduction in invalid-action rate, with every metric paired per task
> over at least 3 seeds and the success margin's one-sided 95% lower
> bound above zero; missing any one of the three margins fails G0-DX-10
> and forces protocol redesign before freeze.

The same row names three lane kills and one fallback:

> kills (research/25): typed surface loses to disciplined shell; schema
> churn dominates agent cost; handles do not reduce invalid-action rate
> | protocol redesign before freeze (G0-DX-10); MCP-only surface

---

## 2. The assay binding

KILL-08's leading indicator is the G0-DX-10 instrument. This section
names each retained component, its owning bone, and its artifact.

### 2.1 The five-metric instrument — `bn-134i` family

`crates/continuum-benchmark/` and `crates/continuum-mcp/` are the PR-10
instrument. A minimal typed client — eight named operations over
`continuumd`'s real wire, research/25's semantic action grammar shipped
as data in `register::OPERATIONS` — runs against a disciplined
text-projection baseline **over the same daemon and the same wire**,
with one shared `Policy` driving both arms, so the number of mistakes is
fixed by construction and only the interface varies.

Design: 4 tasks × 3 declared schedules × 2 policies × 2 arms = 48 runs,
over the Die Hard and Dining Philosophers corpus ports. The counting
rule was fixed before any number was produced, and plan §19.4's
separation gate runs *inside* `Report::over`, so no leaked subset can
produce an `accepted` report.

| Metric | Bone | Test |
|---|---|---|
| IMPL-01 valid operation rate | `bn-134i` | `tests/pr10_impl01_valid_operation_rate.rs` |
| IMPL-02 interface bytes | `bn-23gm` | `tests/pr10_impl02_interface_bytes.rs` |
| IMPL-03 task completion | `bn-3awq` | `tests/pr10_impl03_task_completion.rs` |
| IMPL-04 error recovery | `bn-1udt` | `tests/pr10_impl04_error_recovery.rs` |
| IMPL-05 deterministic reproduction | `bn-26tb` | `tests/pr10_impl05_reproduction.rs` |

Two independently built rigs render byte-identical 48-run artifacts
(IMPL-05). Family separation has its own regression,
`tests/pr10_family_separation.rs`; spike parity against the frozen
answers has `tests/philosophers_evidence.rs`.

### 2.2 The two-sided falsification — `bn-2c0a`

`crates/continuum-benchmark/tests/dx10_falsification.rs`: 17 tests — 13
attacks and 4 controls — over `src/variants.rs` (the accounting and
instrument variants) and `src/falsification.rs` (the byte-stable verdict
artifact). Every variant is added **beside** the landed metrics, and
`control_every_landed_number_is_unchanged_by_this_campaign` holds all
five landed figures unchanged.

Outcome: **eight attacks landed, four held, one inconclusive.** The
campaign attacked in both directions — attacks that favour the baseline
and attacks that favour the typed arm are both present, which is what
makes the surviving numbers usable by an adjudicator. The single
inconclusive attack was X1, the accounting question (§2.4).

The campaign's own standing warning is recorded here because it
disciplines everything downstream: adopting an accounting reading
*after* the numbers are known is the move it named as illegitimate.

### 2.3 The fallible policy families — `bn-2phq3`

The landed invalid-action tie (69 per mille on both arms) was a **floor
artifact of the shared scripted policy**, not a property of the two
surfaces: a shared policy can only make mistakes both surfaces can
express. `bn-2phq3` replaced it with research/25's fallible-policy
families, `crates/continuum-benchmark/src/families.rs` and
`tests/pr10_fallible_families.rs` (15 tests).

Taxonomy first: a mistake is made in one of five **channels**, and the
five families are exactly those five channels, asserted exhaustive and
non-overlapping.

| Family | Class | Channel | Native | Shell | Reduction |
|---|---|---|---|---|---|
| mf-01 | out-of-grammar | operation-choice | local refusal, 0 bytes | wire | 0% |
| mf-02 | stale-snapshot | handle-value | wire | wire | 0% |
| mf-03 | omitted-argument | argument-presence | unrepresentable | wire | **61%** |
| mf-04 | wrong-capability | capability | wire | wire | 0% |
| mf-05 | misnamed-argument | argument-name | unrepresentable | wire | **61%** |

Anti-gaming, two-sided: a **containment theorem** checked mechanically
— every mistake channel the typed surface has, the text surface also
has, and the text surface has exactly two more, because both arms build
the *same* typed request structs. Every class the typed surface *can*
express is injected on the typed arm too; three of five are, and they
measure exactly zero on every split. An independent grader
(`families::regrade`) recomputes attempted/invalid/prevented from the
operation transcripts alone and is shown able to fail on a corrupted
trace. Determinism is total: no entropy anywhere, seeds are the three
declared schedules, and
`every_family_reproduces_byte_identically_from_two_independent_sweeps`
rebuilds both arms of all five families from scratch and compares whole
`ArmRun` values.

Headline: **0.95 shell-only invalid attempts per run against a 0.60
break-even**, recomputed live by `bn-2c0a`'s own function; the ratified
50% reduction clears at **61%**; both arms remain 24/24; and the landed
matrix is reproduced byte-for-byte as the control.

Two adjudicated findings are retained rather than smoothed. **F3 landed
and is load-bearing**: the margin rests on the pre-registered reading
"an attempt is one operation the policy decided to make"; under the
other reading the two arms tie exactly and the margin is 0%. The lead
ratified reading A on 2026-08-09, on the same pre-registration
discipline that settled the bytes accounting, and both numbers stay in
the artifact. **F4 is declined, not banked**: a CLI's idempotency key is
derived from its command text, so an out-of-grammar call whose command
is byte-identical to the legitimate one is an idempotency replay — worth
a baseline collapse to 4/24 at 931 per mille, which would clear the
ratified margin several times over. It prices *this harness's key
derivation* rather than the interface, so it is measured at zero and
recorded as an attack with its numbers.

### 2.4 The byte ledger and the unreachability result

X1 — whose interface "interface bytes per solved task" counts — was the
one attack `bn-2c0a` could not decide. It was decided afterwards, on the
governing text, by **RFC 0027 correction 31**: an interface byte is a
byte the agent itself wrote or read, because bytes are the graded
denominator *only because token counts are model-relative*, and that
reason reaches only content entering an agent's context. Process-internal
frames are uncounted on both arms alike. The rejected symmetric reading
is recorded at full strength with its argument, and the record states
plainly that adopting it after the numbers were known is the move the
campaign warned against.

[`DX10_BYTE_LEDGER.md`](DX10_BYTE_LEDGER.md) (`bn-3bn62`) took the
shortfall to field grain by re-encoding the landed instrument's own 172
recorded envelopes, field by field, rather than modelling it. Its
correction of the folk explanation is retained: `next_operations` and
the envelope-level `Cost` cost **zero** bytes; the real weight is
`artifacts` 20,732 B, `omissions` 16,656 B, `assurance` 15,720 B,
`epochs` 8,084 B credited, and `SnapshotComponents` 17,208 B on the
request side.

**The unreachability result** (`bn-zsi1v`, RFC 0027 correction 31
addendum). With `H` the handshake charged per solved task, `n` the calls
per task, `rn` the typed arm's bytes per call and `rs` the text arm's, a
passing row is `H + n·rn <= 0.7·n·rs`. On the measured values `H` = 875
B, `n` = 7, `rs` = 575 B per call, that requires `rn` <= 277 B per call,
request and result together. The typed arm measures 1,551 B per call and
the demonstrated post-redesign floor is 679 B per call. Stated without
the algebra: the text arm's 575 B per call is a *lossy rendering of an
answer*, and the typed arm's is a *lossless self-describing encoding of
the same answer plus the request that asked for it*, so the margin asks
a lossless encoding of answer-plus-request to come in under half a lossy
rendering of the answer alone. The +30% bytes margin is therefore
**unreachable under the pinned accounting by any lossless protocol**,
and the addendum is normative that a redesign MUST NOT be commissioned
with a passing bytes row as its goal — a redesign that reached the row
would have become a rendering with extra steps, having thrown away the
omission manifests, epoch pinning, assurance envelope and artifact
identity that are the typed surface's reason to exist. The redesign's
obligation is therefore **waste removal, not row-flipping**.

Four errata to the ledger are retained with the conclusion rather than
folded away: the `epochs` credit is understated 3.4× (8,084 B is the
reduction to protocol-only; full removal is 27,864 B); the
`SnapshotComponents` credit is an upper bound rather than an available
saving; `verification.result` names no artifact on any answer; and
`Omission.recoverable_by` is populated on 0 of 324 omission records. A
fifth erratum (`bn-199nx`) falsified ledger item C7 by measurement and
corrected the set figure from 47.6% to **37.4%** of the typed arm's
total wire spend. One question is recorded **open and deliberately not
decided**: negotiating `canonical_cbor` measures at ~816 B per solved
task with no version bump, and whether that counts as reducing what the
agent *learns* belongs to whoever owns the metric's meaning. Recording
it open rather than commissioning it is the same discipline correction
31 applied to X1.

### 2.5 The redesign executed to the major-3 boundary

The `bn-d2bdi` pre-check classified C1/C2/C3/C4/C6 against
`rule versioning.compatible_change` *before* any dispatch — a closed
list of five permitted change shapes. Six of seven candidates move a
presence marker on a `required` member or rewrite a rule, so they are
major-gated. What landed inside protocol major 3:

| Item | Bone | Effect | Test |
|---|---|---|---|
| `OutputPolicy.max_bytes` enforced, first time | `bn-6fuu5` | −49 to −255 B/solved; dormant on the instrument wire | `tests/pr10_c7_output_policy_ceiling.rs` |
| `workspace.create_by_reference` at 3.6 | `bn-3of5h` | −475 B/solved, 96.2% of projection; shell arm byte-for-byte unmoved | `tests/pr10_c4b_port_by_reference.rs` |
| `epochs` pinned at the handshake | `bn-2in7i` | ledger item C5 | `tests/pr10_c5_epochs_at_the_handshake.rs` |

The remaining **3,566 B per solved task — 87.8% of the corrected set —
sits behind protocol major 4.0**, carried on `bn-3861i` with per-item
verdicts, the quoted blocking declarations, and the plan §4.6
typed-compatibility-statement requirement. C6 connection-scoped handle
aliases are recorded **permanently declined** at any version: they
silent-decode on un-upgraded clients, an INV-002 matter that may not be
re-proposed.

### 2.6 Trend data — retained artifacts under stable IDs

Every number above is produced by a byte-stable typed artifact with a
versioned first line, so a later re-run is comparable to this one
line-for-line rather than by prose.

| Stable artifact ID | Producer | What it carries |
|---|---|---|
| `continuum-benchmark aci-report v1` | `src/report.rs` `Report::render` | the five landed metrics, both arms, the §19.4 separation verdict, per-operation detail |
| `continuum-benchmark dx10-falsification v1` | `src/falsification.rs` | the 13 attacks and 4 controls with each verdict and its figures |
| `continuum-benchmark fallible-policy-families v1` | `src/families.rs` | the five families per-arm and per-split, the containment theorem, the six anti-gaming arms, both F3 readings |

Ratified thresholds live in code as `report::RATIFIED`, not in prose, so
a drifted threshold is a compile-visible change. The artifacts declare
their own scope: `aci-report v1`'s header reads "This artifact reports;
it does not decide."

Documentary trend record: [`DX10_BYTE_LEDGER.md`](DX10_BYTE_LEDGER.md)
§2 (decomposition), §4 and §8.8 (sum-checks), §9 (the landed C4b
measurement against its own projection); the
[`G0_SPIKE_MATRIX.md`](G0_SPIKE_MATRIX.md) DX-10 row, which carries the
full measurement history from landing through adjudication to closure.

---

## 3. Per-margin outcomes, exactly

Post-redesign state, at the protocol-3.6 freeze boundary. Every figure
is per solved task and paired per task over the three declared
schedules.

| Ratified margin | Floor | Measured | Verdict |
|---|---|---|---|
| Absolute task success | ≥ +10 pp, one-sided 95% LB above zero | 24/24 native, 24/24 shell — margin 0 pp | **TIE — not a pass** |
| Interface bytes per solved task | ≥ 30% fewer | 10,384 B native vs 4,118 B shell = **−152%** | **FAIL, and unreachable losslessly** |
| Invalid-action rate | ≥ 50% relative reduction | **61%** reduction, from 0.95 shell-only invalid attempts/run against a 0.60 break-even | **PASS** |

Three readings this table must not be given:

1. **A tie is not a partial pass.** The success tie is the most robust
   result in the lane — it holds under every accounting variant, every
   leave-one-out subset and both policies — and it breaks only where the
   *baseline* is degraded (a raw undisciplined scraper, a drifted
   projection key, a stale read). It is recorded as a tie.
2. **The bytes failure is not an encoding artifact.** Under the pinned
   reading, reducing the whole result envelope to its floor and
   resolving `SnapshotComponents` by reference still leaves −86%. §2.4
   proves the stronger statement: no lossless protocol clears it.
3. **The invalid-action pass is a measurement, not the earlier
   artifact.** The landed 69-per-mille tie was a floor artifact of the
   shared scripted policy and lower-bounded the typed advantage without
   measuring it. `bn-2phq3` measured it. The pass depends on the
   ratified F3 reading (§2.3), and the alternative reading is retained
   in the artifact rather than deleted.

The composite: `aci-benchmark-margins` says "missing any one of the
three margins fails G0-DX-10". Two are missed. **The native ACI does not
beat disciplined CLI use on the ratified composite, so KILL-08's
antecedent is true and the criterion fired.**

---

## 4. The three named lane kills, one by one

§24.5's ACI row names three kills. Grading KILL-08 means grading these,
because they are the only place the dossier says what "does not beat"
means for this lane.

**Kill 1 — "typed surface loses to disciplined shell." FIRED, on one of
three margins.** It loses on interface bytes and does not lose on the
other two. The row's own composite rule makes one missed margin
sufficient, so the kill is fired; the honest form of the finding is that
it is fired *narrowly and structurally*, not broadly. §2.4's
unreachability result is why the narrowness matters: this is not a
surface that lost a contest it could have won.

**Kill 2 — "schema churn dominates agent cost." NOT FIRED, and only
partly measurable today.** What the assay window does show: the protocol
moved 3.0 → 3.6 across the campaign, every step classified against
`rule versioning.compatible_change` before dispatch (`bn-d2bdi`), and
the two churn events that touched the instrument *reduced* agent cost —
`bn-3of5h` by 475 B per solved task and `bn-6fuu5` by 49–255 B — with
the shell arm byte-for-byte unmoved in both. So churn cost is negative,
not dominant, over this window. What the assay does **not** show: a
long-run churn rate, or the cost of a major-version migration, which is
exactly what `bn-3861i`'s 4.0 set will exercise. Recorded as a typed
absence in §7, not as a pass.

**Kill 3 — "handles do not reduce invalid-action rate." FALSIFIED, by
measurement.** The typed surface's handles and named arguments remove
two whole mistake channels — argument-presence and argument-name — which
are *unrepresentable* on it and cost the baseline 0.95 invalid attempts
per run. The 61% reduction is the direct measurement of this kill's
negation, with the containment theorem showing no typed-only channel can
exist and the typed-arm injections of the other three classes measuring
exactly zero. This is the one place the lane's hypothesis was confirmed
against an adversarial instrument.

---

## 5. The decision record

**Decision: CONTINUE, with redesign-as-waste-removal.**
**Taken by: the user.**
**Date: 2026-08-11.**
**Where: bone `bn-762i`, option 1 of its freeze adjudication package of
2026-08-09.**

Grounds, quoted from the ratification comment on `bn-762i`:

> USER RATIFIED OPTION 1 (2026-08-11): the second disjunct is discharged
> — redesign executed to the major-3 constitutional boundary
> (OutputPolicy enforcement + workspace.create_by_reference at 3.6,
> measured), remainder ratified as the standing 4.0 plan on bn-3861i.
> Phase A's protocol freezes at 3.6. C6 recorded permanently declined
> per the package footnote, no objection raised.

The option the user ratified, quoted from the package that offered it:

> (1) LEAD RECOMMENDS: discharge now. Close bn-762i on the second
> disjunct, freeze Phase A's protocol at 3.6, ratify bn-3861i as the
> standing 4.0 plan. Grounds: the row's own failure consequence
> ('simplify/rework ACI') has been executed to the boundary the
> versioning constitution itself draws; landing 4.0 before freeze
> inverts the dependency (a major requires the compatibility statement a
> frozen 3.x line is the anchor for); and the bytes margin cannot pass
> at ANY version losslessly, so waiting buys bytes but not the margin.

The decision-package trail is complete and retained: the first package
(2026-08-02) set out the measurement and offered four options A–D; the
second (2026-08-09) set out where the redesign stood and offered three
options 1–3; the ratification (2026-08-11) chose option 1; the closing
comment (2026-08-11) records the final state. Two options were declined
on the record — holding the freeze until 4.0 lands, and reopening the
metric — and the third, the `canonical_cbor` question, is recorded open
and routed to the metric's owner rather than dropped.

Downstream closure edits already landed under `bn-1ihtt`: the PR-10 exit
line in [`START_HERE_IMPLEMENTATION.md`](START_HERE_IMPLEMENTATION.md),
the [`G0_SPIKE_MATRIX.md`](G0_SPIKE_MATRIX.md) DX-10 row's terminal
state ("Closed — failed as measured, failure consequence discharged by
user-ratified redesign"), and the PHASE-A-DEL-09 annotation in `plan.md`
§21.

---

## 6. Coverage verdict — does the ratification answer KILL-08?

**Yes.** The argument is made term by term against §1's quoted sentence
rather than against a paraphrase, because a checkpoint that annotates a
kill criterion on a decision aimed at a *different* question is the
failure mode this section exists to exclude.

| KILL-08's term | What the ratification decided | Same question? |
|---|---|---|
| "agent-native API" | the native ACI, `crates/continuum-mcp` over `continuumd`'s wire | yes — one artifact, one instrument |
| "disciplined CLI use" | the disciplined text-projection baseline over the **same daemon and the same wire**, not a strawman scraper | yes — and `bn-2c0a` attacked precisely the "was the baseline disciplined enough" direction, and that attack held |
| "does not beat" | the three ratified `aci-benchmark-margins` margins, composite rule "missing any one fails" | yes — §24.5 is the only numeric definition of "beat" for this lane, and it belongs to KILL-08's own register row |
| the consequence — "must narrow or redesign" | protocol redesign executed to the major-3 boundary, remainder ratified as the 4.0 plan, freeze at 3.6 | yes — and §24.5's row names the identical fallback, "protocol redesign before freeze (G0-DX-10)" |
| the decision class — continue / narrow / defer / kill | CONTINUE with redesign; not kill | yes — and §24's closing sentence forbids a lane failure from killing the program |

Three further checks, each of which could have falsified the verdict:

1. **Does the ratification cover the criterion's *whole* scope, or only
   the PR-10 exit?** `bn-762i`'s exit sentence has two disjuncts —
   "native ACI is measurably more effective **or** the protocol is
   redesigned before freeze". The first disjunct is exactly KILL-08's
   question, and the adjudication of 2026-08-09 answered it: it fails as
   measured. The second disjunct is exactly §24's "narrow or redesign"
   consequence, and the ratification of 2026-08-11 discharged it. Both
   halves of KILL-08 — antecedent and consequent — are therefore decided
   by the same record.
2. **Is anything KILL-08 asks left unmeasured?** One thing is: kill 2,
   schema churn over the long run (§4, §7 row 2). It is not fired, it is
   not claimed passed, and it is owned by `bn-3861i`'s 4.0 set rather
   than left to nobody. It does not block this checkpoint, because a
   criterion whose composite has already fired cannot be made to fire
   harder by a second unfired kill.
3. **Is there a dangling successor?** No. Every unfinished thread has a
   named owner: the 4.0 remainder is on `bn-3861i`; the post-4.0
   re-measure re-runs mechanically from the same three artifacts and is
   carried by the same bone; the `canonical_cbor` question is routed to
   the metric's owner and recorded open in RFC 0027; C6 is permanently
   declined and cannot be re-proposed. Nothing waits on an unassigned
   actor. One citation defect is recorded in §7 row 3.

**Because the verdict is YES, this checkpoint annotates the register row
rather than writing a fresh decision package.** Had any row of the table
above read "no", the annotation would have been withheld and a package
prepared instead — that was this bone's standing instruction, and it is
recorded here so the road not taken is visible.

---

## 7. Gaps and typed absences

| # | Gap | Severity |
|---|---|---|
| 1 | The whole assay is **scripted policies over 4 tasks and 2 semantic families**. No variant varies the *agent*, so nothing here answers whether a live model would discover better use of either surface. `bn-2c0a` and `bn-2phq3` both declare this in their own limits sections. | Structural and declared. It bounds what "beat" was tested over; it does not weaken the composite verdict, because the same bound applies to both arms. |
| 2 | Kill 2, "schema churn dominates agent cost", is **not measured over any horizon longer than 3.0 → 3.6**. Over that window churn cost is negative (§4). | Open, owned by `bn-3861i`. Not recorded as a pass. |
| 3 | `bn-762i`'s closing evidence trail cites `bn-31cq1`, which **does not resolve in the Bones store**. The RFC-0027 and matrix edits it stands for did land, under `bn-zsi1v` and `bn-199nx`. | Citation defect, not missing work. Recorded so a later reader does not hunt for a bone that never existed. |
| 4 | The invalid-action pass depends on the ratified F3 reading of "attempt". Under the alternative reading the margin is 0%. | Adjudicated 2026-08-09 on pre-registration grounds, with both numbers retained in the artifact. Recorded, not hidden. |
| 5 | The `canonical_cbor` encoding question (~816 B per solved task, no version bump) is **open by design** and routed to the metric's owner. | Deliberate. Commissioning it after the numbers were known is the move correction 31 refused once already. |
| 6 | `Omission.recoverable_by` is populated on **0 of 324** omission records on this wire, and `verification.result` names no artifact on any answer. | Found by the ledger, recorded rather than acted on. Belongs to INV-007's retrieval half, not to KILL-08. |

None of the six falsifies the checkpoint. Rows 1, 2 and 5 bound what was
measured; rows 3, 4 and 6 are honesty records.

---

## 8. Precedent decisions for future kill-criterion checkpoints

This is the first checkpoint executed against plan §24. Decisions made
here, for reuse by KILL-01…KILL-19:

- **Artifact home**: `notes/plan/notes/KILLNN_CHECKPOINT.md`, sibling to
  `R01_RISK_CHECKPOINT.md` and `G0_SPIKE_MATRIX.md`, on the shape
  R01 §7 set. This document *cites* checkers and artifacts; it adds
  none.
- **Register row**: unlike `docs/08`, plan §24 *is* editable prose and
  the traceability extractor reads it, so a kill criterion **does** close
  by a delivered-annotation on its own bullet — the same
  `(delivered: bn-…)` convention already used for invariants, PR exits,
  phase deliverables, governance-policy bullets, risks and threats.
  `tools/traceability.py`'s kill loop was extended to honour it here, in
  the same shape as `_extract_bullet_policy` and `_extract_headings`.
  R01's "leave the row untouched" precedent does not transfer: it was
  reasoned from `docs/08` having no status column, and §24 has an
  annotation convention instead.
- **No nested parentheses inside the annotation.** The strip regex is
  `\(delivered:[^)]*\)`. A Markdown link, a parenthetical aside or a
  bracketed citation inside the annotation truncates it and corrupts the
  registry summary. Backticked paths, never links.
- **A kill criterion is discharged by a decision, not by a
  measurement.** The three things the annotation must be able to cite
  are: the assay ran, the evidence is retained under stable IDs, and the
  privileged continue/narrow/defer/kill decision was taken by the party
  it belongs to. A measured-and-fired criterion with no decision is
  **not** dischargeable, however complete its evidence.
- **Grade against the register row's own numbers.** §24's bullets state
  no thresholds. Where §24.5 ratifies a `quote-id` sentence for the
  lane, that sentence is the definition of the bullet's verb, and the
  row's named kills are the checklist. Inventing a threshold on a lane's
  behalf is the defect §24.5 exists to prevent.
- **State per-margin outcomes, never a composite alone.** §3's table is
  the required shape: floor, measured value, verdict, plus the readings
  the table must *not* be given. A criterion can fire on one margin
  while its lane's central hypothesis is confirmed on another, and both
  facts must survive into the record.
- **Traceability regen is required**, unlike R01's checkpoint: annotating
  a §24 bullet changes text `generate_traceability.py` scans. Run
  `just traceability` and confirm exactly one status flip.

---

## 9. What this document does NOT do

- **It does not take the KILL-08 decision.** Per this bone's contract,
  "Agents do not take the privileged program decision." The decision was
  taken by the user on 2026-08-11 on `bn-762i` (§5). This document
  *locates* and *quotes* that decision; it does not re-derive it, and it
  would have withheld the annotation and written a package instead had
  §6 come out the other way.
- **It does not re-adjudicate X1, F3, or C6.** Each was decided by the
  party that owned it — X1 by RFC 0027 correction 31 under the user's
  explicit delegation, F3 by the lead on pre-registration grounds, C6 by
  the user as permanently declined. This document records the rulings
  and their grounds.
- **It does not produce a new measurement.** Every figure here is read
  off a retained artifact or a committed test named in §2. No number in
  this document is original to it.
- **It does not claim the ACI is a good surface, or a bad one.** It
  reports that the ratified composite is not met, that one of three
  margins is met, that one is structurally unreachable, and that the
  program's response was to remove demonstrated waste rather than to
  chase an unreachable row.
- **It does not close `bn-3ety` or any bone, and does not merge.** Per
  the assigned protocol the workspace stays unmerged; the lead merges,
  closes, and pushes.
- **It does not claim exhaustiveness over §24.** The other eighteen kill
  criteria are untouched and remain active.
