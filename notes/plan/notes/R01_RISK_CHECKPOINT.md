# R01 Risk Checkpoint — Scope Collapse Under Ambition

**Prepared:** 2026-08-05 (bone `bn-1os6`, workspace epoch `3104b09`)
**Scope:** mechanical evidence for the five controls named against
[`docs/08_RISK_REGISTER.md:13`](../docs/08_RISK_REGISTER.md) R01
("the project simultaneously attempts a language, runtime, TLC
replacement, theorem prover, production monitor, distributed checker,
and IDE; nothing becomes indispensable"):

> Controls: gate-driven delivery; first vertical slice; no frontier
> engine before G2; explicit non-goals; separate core/product/research
> lanes.

This is the **first** risk-register checkpoint executed against the
register (bn-1os6). §1–§3 give the three controls the checkpointing
bone named for deep, cited verification; §4–§5 give the remaining two
register controls a lighter but still-cited pass, so the checkpoint
speaks to every control the row lists rather than a subset. §6 records
gaps as typed absences. §7 sets the house shape for future R0N
checkpoints. §8 states what this document does not do.

`docs/08_RISK_REGISTER.md` carries no per-row status column (verified
by reading all 21 rows: each has only Probability/Impact/Failure
mode/Controls, and some carry a Kill signal — never a Status field).
Per this checkpoint's own brief: with no status column to update, this
document plus the bone comment on `bn-1os6` carry the checkpoint, and
the register row is left byte-for-byte untouched. No plan-visible text
changes as a result of this checkpoint, so no traceability regeneration
is triggered (verified: see §7).

---

## 1. Control — gate-driven delivery

### 1a. The gate scheme: 11 gates, mechanically reconciled

The normative gate scheme is `docs/52_RELEASE_GATES_REV3.md` (G0–G10),
reconciled bullet-for-bullet against `plan.md` §22 by
`tools/validate_dossier.py`'s `check_gate_scheme_correspondence` — a
bidirectional check (a bullet dropped from either document is a
failure) wired into `just check` via the `dossier` recipe. Live run
against this workspace's tree:

```
$ cd notes/plan && uv run --with jsonschema python3 -c "
import sys; sys.path.insert(0,'tools')
import validate_dossier as vd
print(vd.check_gate_scheme_correspondence())"
{'gates': 11, 'plan_bullets': 68, 'docs_bullets': 68}
```

Zero orphans in either direction (the call would raise `AssertionError`
otherwise). G0 itself carries no bullets — its criterion is the
[`G0_SPIKE_MATRIX.md`](G0_SPIKE_MATRIX.md) closing as a whole (docs/52:
"All load-bearing experiments ... have evidence or an explicit redesign
decision") — so the countable bullet criteria are 68 across G1–G10.

**Divergence from the checkpointing brief, recorded honestly:** the
brief cites "69 criteria." The mechanically verified count is **68**
bullet criteria (G1–G10) plus G0's own single holistic criterion, which
totals 69 only if G0 is counted as contributing exactly one criterion
alongside the 68 bullets — a reading this document adopts as the most
likely source of "69" but does not find asserted verbatim anywhere in
the tables. The live, mechanically-checked number quoted above is 68
bullets / 11 gates; both readings agree the scheme is intact and
zero-orphan.

### 1b. PR exit conventions: fourteen of fifteen Phase A PR slots exited

`START_HERE_IMPLEMENTATION.md` names 15 Phase A PR slots (PR 0, 1, 2, 3,
4, 4a, 5, 6, 7, 8, 9, 10, 11, 12, 13 — Phase A closes at PR 13; PR 14
opens Phase B). Each exited slot has a `state: done` exit-evidence bone
with a merged commit; PR 10 alone remains open. Verified via `bn show`
and `git log` against this workspace's history:

| PR | Exit-evidence bone | State | Close commit |
|---|---|---|---|
| 0 | bn-39d3 | done | `6519b7b` |
| 1 | bn-1w1y | done | `7d2021d` |
| 2 | bn-1sh6 | done | `1ceaef9` |
| 3 | bn-3tkw | done | `f4c1f17` |
| 4 | bn-ces7 | done | `58bef24` |
| 4a | bn-zr81 | done | `99ad367` |
| 5 | bn-89km | done | `4f7a7ee` |
| 6 | bn-1n6r | done | `3ec3a90` |
| 7 | bn-wco1 | done | `c8d728d` |
| 8 | bn-1jg0 | done | `49a7d25` |
| 9 | bn-19bc | done | `d90afb4` |
| **10** | bn-762i (exit bone, parent goal bn-38i) | **open** | — |
| 11 | bn-3m65 | done | `b1c3071` |
| 12 | bn-3jtr | done | `8b1d08b` |
| 13 | bn-2hmk | done | `750ffda` |

14 of 15 exited — matching the checkpointing brief's "fourteen PRs
exited" exactly. The one open slot, PR 10 (`bn-38i`, `state: open`),
is the freeze-blocking G0-DX-10 ablation ("native ACI beats the
disciplined shell baseline, or the protocol is redesigned before
freeze"); `plan.md` §0.3 independently states "DX-10 is open and
freeze-blocking (Phase A)." This is not a control gap — it is the gate
scheme doing exactly its job: PR 10's own goal bone cannot close until
DX-10 resolves one of its two valid exits (win or redesign), and no
downstream Phase A/B work is gated on PR 10 having already closed.

### 1c. `just check`'s composition

`Justfile`:

```
check: fmt-check lint test boundaries covenant governance dossier lean
```

Eight gates in one command: Rust formatting, Clippy (`-D warnings`),
the full workspace test suite, the plan §20 crate-boundary/dependency
checker (§3 below), the `<15,000`-line kernel covenant with its
mutation matrix, six governance/policy checkers (docs/12 GOV §1–3 plus
the docs/09 T12 dependency-vet posture), the dossier validator (§1a,
§1d, and 25 other mechanical checks including this checkpoint's own
`plan_bones_traceability` and `risk_checkpoint_failures`), and the Lean
kernel-check gate (RFC 0012 T0/T1, ADR-0035 axiom manifest). This is
the single command this checkpoint itself must exit 0 before finishing
(see the closing protocol on `bn-1os6`).

### 1d. Phase-barrier wiring in the dependency graph

`tools/traceability.py`'s `graph_contract_state` enforces, mechanically
from live Bones state (not asserted prose):

- **`phase_barrier_failures`** — every leaf bone under Phase N's goal
  is blocked by Phase (N-1)'s goal bone, for B←A, C←B, D←C, E←D, F←E.
- **`phase_exit_failures`** — each phase has exactly one `req:phase-X-exit`
  goal bone carrying `goal:manual`.
- **`gate_failures`** — each gate criterion bone (`plan-key:gate-gN`) is
  blocked by its phase's integration goal.
- **`risk_checkpoint_failures`** — every `risk`/`kill-criterion`-labeled
  task bone carries exactly one `phase-X` label, blocks that phase's
  integration bone, and (for phase B–F) has a predecessor-phase barrier.
  **This checkpoint's own bone is a live instance of this rule**:
  `bn-1os6` carries `phase-a` and `risk`, and is wired as a blocker of
  `bn-2ofd` ("Phase A integrated exit evidence and privileged decision
  package") — i.e. Phase A cannot be declared exited without this
  checkpoint resolving first.

All of the above are folded into `validate_checked_traceability`
(`tools/validate_dossier.py`'s `plan_bones_traceability` check), which
also regenerates `PLAN_REQUIREMENTS.json`/`PLAN_BONE_TRACEABILITY.md`
in memory and fails if the checked-in copies disagree byte-for-byte.
Live run against this workspace (immediately after `bn do bn-1os6`,
i.e. with this checkpoint's own bone already transitioned to `doing`):

```json
"plan_bones_traceability": {
  "active_bones": 765,
  "active_edges": 1774,
  "active_requirements": 652,
  "coverage": "complete",
  "dependency_layers": 14,
  "leaf_bones": 672,
  "requirements": 846
},
"status": "pass"
```

No failure keys present (`uncovered`, `risk_checkpoint_failures`,
`phase_barrier_failures`, `gate_failures`, etc. are all absent from the
report, which only lists keys with nonempty content). **Control 1 is
live**, on four independent, mechanically-checked legs.

---

## 2. Control — first vertical slice

Die Hard and Dining Philosophers run end-to-end through
engine → kernel → daemon → CLI → agent client. This is not one test —
it is a fact pattern recorded across the PR-8, PR-10, and PR-13
exit-evidence suites, each hop independently cited below.

### 2a. Engine → kernel → daemon (PR 8, `bn-1jg0`)

`crates/continuum-engine-reference/src/{ident,domain,expr,model,diehard,
bfs,checking,witness,certificate}.rs` explore Die Hard (16 states, 96
transitions, depth-6 solution — TV-009's frozen facts), emit a closed
exploration as CONTCERT wire bytes from `continuum-kernel-core`'s wire
grammar (producer and checker share no codec — "the engine keeps no
library dependency on the kernel at all"), and hand the bytes to the
kernel's checker (`continuum-kernel-core`, PR 9, `bn-19bc`), which
verifies 16 reachable states / 96 labelled transitions / one initial
state from wire form only. `crates/continuumd/tests/pr8_exit_evidence.rs`
carries this through the daemon's native protocol, named in its own
test:

```
fn positive_die_hard_returns_sixteen_states_through_the_wire_and_the_depth_six_solution_one_layer_down()
```

PR-8's own exit sentence: "Die Hard returns 16 states and depth-6
solution through the daemon API" (delivered: `bn-1jg0`, `state: done`).

### 2b. Daemon → CLI (PR 13, `bn-2hmk`)

`crates/continuum-cli/tests/pr13_exit_evidence.rs` plus its golden
fixtures pin Die Hard through the CLI at the wire boundary — golden
artifacts include `pr13-01-snapshot-create--sealed-die-hard-base` and
`pr13-03-check-result--die-hard-refuted` (both `.json.golden` and
`.text.golden`). Exit tests include
`every_pinned_artifact_matches_its_committed_golden_bytes` and
`the_exit_code_matrix_reaches_all_five_classes_on_real_scenarios`.
PR-13 exit: "golden tests pin JSON, exit codes, and concise terminal
output" (delivered: `bn-2hmk`, `state: done`).

### 2c. Daemon → agent client, both corpus members (PR 10, `bn-134i`/`bn-3awq`)

`crates/continuum-benchmark/tests/pr10_impl03_task_completion.rs` runs
the agent client against a four-task Phase A subset over **both** Die
Hard and Dining Philosophers. Dining Philosophers (TV-007, ported at
this bone) is transcribed the way `continuum-engine-reference::diehard`
transcribes Die Hard — inert engine-vocabulary data — and checked
against frozen dossier facts: **573** reachable states, **2,365**
labelled transitions, shortest deadlock depth **10**, exact deadlock
state (every philosopher `HAS_LEFT`, fork *f* held by philosopher *f*).
Test names:

```
fn every_faithful_cell_is_solved_on_both_arms()
fn the_two_arms_read_the_same_state_count_and_the_same_verdict()
fn the_state_count_each_arm_reports_is_the_one_it_read_through_its_own_surface()
fn the_transition_count_and_witness_depth_are_read_one_layer_down_and_said_to_be()
```

"Both arms" is the ACI ablation's own load-bearing comparison (native
protocol vs. disciplined shell baseline) — the vertical slice runs
identically under both, and a run is "solved" only when the agent read
the frozen dossier answer, not merely when the script finished.

### 2d. What is NOT yet closed

PR 10's own umbrella goal (`bn-38i`) remains open — the ablation's
win/redesign verdict is not yet ratified (§1b). The vertical slice
*runs* end to end on both corpus members through all five hops; whether
the agent-client hop *wins* against the shell baseline is a distinct,
still-open question the slice's existence does not answer and this
checkpoint does not decide.

**Control 2 is live**: the five-hop path is exercised, with frozen,
re-derived facts (not merely "it compiled") at every hop, on both named
corpus members.

---

## 3. Control — no frontier engine before G2

### 3a. Crate membership: plan §20's 42-crate list matches `crates/` exactly

`plan.md` §20 lists 39 lines (one, `continuum-effects-{network,storage,
time,process}`, expands to 4 crates) — 42 crate names in total. A
bidirectional diff against this workspace's `crates/` directory:

```
expanded count: 42
actual count: 42
in plan not in crates/: []
in crates/ not in plan: []
```

This is enforced mechanically, not just spot-checked here:
`tools/check_crate_boundaries.py` (self-tested, then run, wired into
`just check` via the `boundaries` recipe) live output against this
workspace:

```json
{
  "members": 42,
  "plan_20_crates": 42,
  "status": "pass",
  "violations": []
}
```

against the rule set `["members-match-plan-20", "no-protocol-crate",
"certificate-checker-not-search", "model-core-not-asupersync",
"kernel-is-synchronous", "forge-not-imported-by-verifier",
"adapters-are-sinks"]`. `forge-not-imported-by-verifier` and
`certificate-checker-not-search` are exactly the edges a smuggled-in
frontier engine would have to cross to reach the trust base or the
product surface; both are checked, not merely documented.

### 3b. Frontier-adjacent crates are doc-comment-only scaffolds, with one audited exception

The research/ directory (36 lanes, `research/01`–`research/36`) names
the frontier work — symbolic and solver-backed exploration
(research/02), liveness/fairness (research/04), incremental
verification (research/27), synthesis/co-design (research/29,
research/30), and more — each owned by `plan.md` §24.5's frontier lane
register with an explicit HYPOTHESIS-class claim status (§0.3), a kill
criterion, and a fallback. The crates those lanes would eventually fill
are present in `crates/` (per §3a's 42-of-42 membership) but hold
**no implementation**, only responsibility/boundary doc comments (`wc
-l` over `src/*.rs`, this workspace):

| Crate | `.rs` files | Lines |
|---|---:|---:|
| `continuum-incremental` | 1 | 21 |
| `continuum-engine-symbolic` | 1 | 21 |
| `continuum-engine-liveness` | 1 | 21 |
| `continuum-engine-dpor` | 1 | 21 |
| `continuum-proof-client` | 1 | 22 |
| `continuum-refinement` | 1 | 21 |
| `continuum-repair` | 1 | 19 |
| `continuum-debugger` | 1 | 19 |
| `continuum-security` | 1 | 19 |
| `continuum-corpus` | 1 | 21 |
| `continuum-cir` | 1 | 20 |
| `continuum-observer` | 1 | 19 |

Every one of these `lib.rs` files is a `//!` module doc declaring
responsibility and dependency boundary and ends with the identical
sentence: "PR-1 / IMPL-01 scaffold: this crate declares its
responsibility and its dependency boundary. The types and behavior
land in the PR named above. `tools/check_crate_boundaries.py` enforces
the forbidden edges mechanically." — a literal, repeated, self-declared
scaffold marker, not an inference from line count alone.

`continuum-incremental` specifically was verified empty by name at
`bn-7ek41` (PR-12's wire-assembler bone, closed 2026-08-04), which had
to restate RFC 0030's dependency-reason/reuse-edge-class vocabulary
locally rather than import it, "because `continuum-incremental` is a
PR-23 scaffold with no types" — independent confirmation from a bone
that needed the real types and recorded their absence as a cost.

**Amendment, 2026-08-09 (bn-1dsih): `continuum-forge` has left this
table, and the reconciliation is recorded here rather than by quietly
restating the control.** The table above carried `continuum-forge` at
1 file / 25 lines; it is now 2 files / 402 lines. What landed is
`src/task.rs`'s task-assembly lane: `assemble` decides whether an
Intent Contract may become a synthesis task, by making the two calls
RFC 0037 corrections 17 and AO2/AO3 require — `IntentContract::check`
for the document's five cross-field rules, and
`Optimization::require_non_vacuity` for INV-012 — and refusing with a
typed `TaskRefusal` when the contract declares no behavior that must
remain possible. RFC 0037 recorded the absence of that caller as flag
F12; the flag now carries its discharge annotation.

Two claims in this section therefore need restating, and they are not
the same claim:

- **Forge is still frontier-adjacent.** §3a above classifies
  `continuum-forge` with the search/synthesis crates by name, and §24.5
  owns the synthesis lanes research/29 and research/30 that would fill
  it. Nothing here reclassifies it, and the forbidden-edge rules
  `forge-not-imported-by-verifier` and `certificate-checker-not-search`
  are untouched — both still pass with zero violations, and the crate's
  single declared dependency is `continuum-intent`, which is
  verifier-side and adds no edge either rule forbids.
- **The sentence "every frontier-adjacent crate is a scaffold" is no
  longer true as written, and is corrected rather than reinterpreted.**
  The table carried thirteen crates when this checkpoint was written.
  Twelve of them are still doc-comment-only scaffolds and keep their
  rows; `continuum-forge` is the one exception and its row is removed,
  because leaving a "1 file / 25 lines" measurement standing beside a
  crate that now has two files would be a false reading of a live
  workspace, not a historical record of one.

Control 3's substance survives that correction under this section's own
test, applied unchanged. R01's failure mode is a frontier engine built
before its gate, and the four ambitions it names are a search or
exploration engine, a theorem prover, a synthesis engine, and the rest
of the §1b list. The lane is none of them: it holds no grammar, no
enumeration, no counterexample loop, no candidate, no archive, and no
engine adapter, and it runs no search at all — it is a contract
*admission* decision, the same category as the intent-diff classifier
this section already declines to count as a frontier engine. PR 29
("Forge finite CEGIS v0") remains unopened and is where the search
itself lands. The mechanical statement of that boundary is
`crates/continuum-intent/tests/inv012_nonvacuity_evidence.rs`'s
`boundary_mutation_challenges_are_forges_own_responsibility_and_not_yet_landed`,
which pins the absence of the search machinery against the lane's real
source, and
`crates/continuumd/tests/inv015_agent_least_authority_evidence.rs`'s
`boundary_forge_grew_a_task_assembly_lane_and_it_reaches_no_host_effect`,
which records the crate's whole public surface and re-derives that it
grants no host authority reachable from a connection. A future
checkpoint should read this amendment as narrowing the *evidence*, not
the control: "no frontier engine before G2" is now carried by two live
tests over a real surface plus twelve self-declared scaffolds, where it
was previously carried by thirteen self-declarations alone.

The twelve rows left in the table are unaffected: each is still a
one-file, 19–22-line doc comment ending in the self-declared scaffold
sentence quoted above.

By contrast, `continuum-kernel-{sat,smt,temporal}` (4,000+ lines each)
and `continuum-semantic-diff` (7,600+ lines) are **not** frontier
engines under R01's failure mode — they are Phase A deliverables
already fully exited (PR 9 §1b; PR 12 §1b): the trusted certificate-
checking base (§20's dependency rule: "the `continuum-kernel-*` crates
form the trusted checking base") and the intent-diff classifier
respectively, neither of which is a search/exploration engine, a
theorem prover, or a synthesis engine — the four ambitions R01's
failure mode names ("a language, runtime, TLC replacement, theorem
prover, production monitor, distributed checker, and IDE") that are
explicitly gated to close no earlier than G2 (ACI, currently the open
gate — §1b) and, for several, no earlier than G4–G7.

**Control 3 is live**: crate membership is exactly the declared 42, the
forbidden-edge checker passes with zero violations, and every
frontier-adjacent crate is independently, mechanically verifiable as
holding no frontier engine — twelve as non-implementing scaffolds
(self-declared in their own doc comments, and externally confirmed at
`bn-7ek41`), and `continuum-forge` by the two live tests named in the
2026-08-09 amendment above, which pin the absence of its search
machinery and the whole of its public surface.

---

## 4. Control — explicit non-goals (supplementary; brief did not request depth here)

`docs/00_EXECUTIVE_SPEC.md` §"Scope boundaries" states explicit
non-goals verbatim: "Continuum does not promise push-button proof of
arbitrary Rust, automatic discovery of the correct abstraction,
universal liveness automation, exhaustive unbounded checking, or
conclusive monitoring from insufficient evidence." `plan.md` separately
records scoped non-goals at the 1.0 boundary — cross-repository
federation is "an explicit 1.0 non-goal" (`plan.md:2342`) and weak
memory is "a declared 1.0 non-goal" (§24.5 register row, `plan.md:2668`,
until ADR-0032's lane opens).

**Gap, typed:** `docs/00_EXECUTIVE_SPEC.md` self-identifies in its own
title as "Revision 2" and is not restated verbatim under a Revision 3
banner (unlike `docs/26`→`docs/52`'s explicit Rev-2→Rev-3 gate-scheme
migration, or `docs/04`, explicitly marked "Rev-2-bannered (done) and
archived"). No mechanical check (comparable to
`check_gate_scheme_correspondence` for gates) reconciles docs/00's
scope-boundaries prose against `plan.md`. This document does not find
evidence the non-goals list is stale — the two scoped 1.0-era
additions above are Revision 3-native and consistent with it — but
also does not find a validator holding the two texts in correspondence
the way it does for gates. **Absence, not failure**: unlike the other
four controls, "explicit non-goals" currently rests on prose citation
alone, with no mechanical reconciliation check.

---

## 5. Control — separate core/product/research lanes (supplementary; brief did not request depth here)

The separation is real but distributed across three mechanisms rather
than named by one:

- **Claim lattice** (`plan.md` §0.3): sections are DESIGN (accepted
  architecture) unless marked HYPOTHESIS, and every HYPOTHESIS-class
  capability is "owned by a research lane with kill criteria" (§24.5).
  Fourteen capabilities are so registered (§3b's examples among them),
  each `draft` or `ratified` — never silently promoted to DESIGN.
- **Directory separation**: `research/` (36 lane documents, prose only,
  no code) is physically separate from `crates/` (product/core code);
  a research lane's promotion to implementation is a named PR, never
  an implicit code change inside `research/`.
- **Crate dependency boundaries** (§3a): `forge-not-imported-by-
  verifier`, `certificate-checker-not-search`, and
  `kernel-is-synchronous` mechanically keep the trust base ("core":
  `continuum-kernel-*`) isolated from search/synthesis crates
  ("frontier"/research-adjacent: `continuum-forge`,
  `continuum-engine-{symbolic,liveness,dpor}`), and "adapters-are-sinks"
  keeps product-surface crates (`continuum-cli`, `continuum-lsp`,
  `continuum-dap`, `continuum-mcp`, `continuum-sarif`) from owning
  semantic state.

**Gap, typed:** there is no single document or checker named
"core/product/research lanes"; this section synthesizes the claim from
three independently-verified mechanisms rather than citing one. That
synthesis is this document's own judgment call, not a quotation — a
future checkpoint agent should treat this as the weakest-cited of the
five controls and consider whether a dedicated crate-tier labeling
(core/product/research, analogous to the existing `plan-key:` label
family) is worth proposing as follow-up, rather than assuming this
document's synthesis is itself normative.

---

## 6. Gaps summary (typed absences, INV-008 spirit)

| # | Control | Gap | Severity |
|---|---|---|---|
| 1 | gate-driven delivery | Brief cites "69 criteria"; mechanically verified count is 68 bullets (+ G0's own holistic criterion). Both readings agree the scheme is zero-orphan. | Cosmetic — documented divergence, not a control failure. |
| 2 | gate-driven delivery | PR 10 (G0-DX-10, the ACI ablation) remains open — 14 of 15 Phase A PR slots exited, not 15 of 15. | Expected — this is the gate scheme operating as designed, not a defect. Tracked at `bn-38i`/`bn-762i`. |
| 3 | first vertical slice | The slice *runs* end to end on both corpus members; whether the agent-client hop *wins* its ablation is undecided (same PR-10 gap as row 2). | Same root cause as row 2. |
| 4 | explicit non-goals | No mechanical text-correspondence check between `docs/00`'s Rev-2-titled scope boundaries and `plan.md`'s Rev-3 non-goal mentions (unlike the gate scheme's `check_gate_scheme_correspondence`). | Low — no evidence of drift found, but no checker guards against future drift either. |
| 5 | core/product/research lanes | No single named enforcement site; this document synthesizes the claim from claim-lattice + directory separation + crate-boundary rules. | Low-medium — real mechanisms exist and are independently verified, but the "lane" framing itself is this document's inference. |

None of the five gaps falsifies its control. Rows 2–3 share one root
cause (PR 10 open) that the register's own kill signal already accounts
for ("after G0 effort, no replayable real-code demo" — not applicable;
a replayable real-code demo exists per §2). Rows 4–5 are honest
scope-narrowing on the two controls the checkpointing brief did not ask
for citation depth on.

---

## 7. Precedent decisions for future R0N checkpoints

This is the first risk-register checkpoint executed. Decisions made
here, for reuse by R02–R21:

- **Artifact home**: `notes/plan/notes/R0N_RISK_CHECKPOINT.md`, sibling
  to `G0_SPIKE_MATRIX.md` and `PR0_EXIT_EVIDENCE.md` — doc-adjacent,
  not `tools/` (no self-testing mechanical checker is being added here;
  this document *cites* existing checkers, it does not add a new one).
  If a future risk's controls are better served by a repeatable
  checker (e.g. a control that needs re-verification every commit
  rather than a point-in-time checkpoint), that checker belongs in
  `tools/` with its own `evidence/` directory, on the
  `check_kernel_covenant.py`/`check_crate_boundaries.py` pattern — this
  document is the lighter-weight shape for a point-in-time register
  checkpoint.
- **Register row**: left untouched when the register carries no status
  column (verified true for all 21 rows in `docs/08_RISK_REGISTER.md`
  today). The bone comment plus this artifact are the checkpoint's
  record. Do not invent a status column.
- **Scope discipline**: when a checkpointing brief names a subset of a
  risk's registered controls for deep citation, still address every
  control the register itself lists for that risk (§4–§5 here), at
  whatever depth the evidence supports, rather than silently dropping
  the untargeted controls. Mark the difference in treatment explicitly.
- **Gaps are typed, not hidden**: a control being "live" does not mean
  zero gaps. Every discrepancy found (§6) is recorded with its own
  severity judgment rather than smoothed into the "live" verdict.
- **No traceability regen was needed**: adding this document did not
  change any text `tools/generate_traceability.py` scans (`plan.md`,
  `START_HERE_IMPLEMENTATION.md`, `.bones/events`) — confirmed by
  re-running `validate_dossier.py`'s `plan_bones_traceability` check
  after this file was added, with an unchanged result (§1d's JSON).
  Future checkpoints should run the same check before assuming a regen
  is or is not needed, rather than assuming either way from this
  precedent alone.

---

## 8. What this document does NOT do

- It does not take the R01 continue/narrow/defer/kill decision. Per the
  checkpointing bone's own contract: "Agents do not take the privileged
  program decision." That decision, if and when the register's kill
  signal fires ("after G0 effort, no replayable real-code demo or no
  project willing to migrate its DST"), belongs to the human lead —
  and this document's §2 finds a replayable real-code demo already
  exists, so the kill signal has not fired.
- It does not modify `docs/08_RISK_REGISTER.md`. The row has no status
  column (§ above); this document and the `bn-1os6` bone comment carry
  the checkpoint instead.
- It does not close `bn-1os6` or any bone. Per the assigned protocol,
  this bone stays open in-workspace; the lead merges, closes, and
  regenerates.
- It does not claim exhaustiveness. §4–§5's lighter treatment of two
  controls is stated as lighter, not as equivalent depth to §1–§3.
- It is evidence, not the decision — a citation-dense restatement of
  facts already recorded in `plan.md`, `docs/52`, `docs/00`,
  `START_HERE_IMPLEMENTATION.md`, `tools/*.py`'s live output, and the
  Bones store, assembled in one place for the human reviewer, asserting
  nothing beyond what those sources and live tool runs already state.
