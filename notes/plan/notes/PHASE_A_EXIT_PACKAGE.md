# Phase A Exit Package — Integrated Evidence and Privileged Decision

**Prepared:** 2026-08-11 (bone `bn-2ofd`, workspace `bn-2ofd`)
**Requirement:** `PHASE-A-EXIT`, [`plan.md`](../plan.md) §21 lines 2233–2238
**Phase:** `PHASE-A` — Trust spine and ACI kernel, [`plan.md`](../plan.md) §21 line 2113
**Gates named by the exit goal:** G0 freeze subset, G1, G2
([`docs/52`](../docs/52_RELEASE_GATES_REV3.md), [`plan.md`](../plan.md) §22)
**Decision bone:** `bn-1grk`, labelled `goal:manual` — **open, and left open by this document**
**Source epoch:** `da177ca9b4faadbc056f001143794a887195b8d6`

This is the integrated exit package for Phase A. It assembles the evidence
for each clause of the exit sentence, binds every artifact it produced to
the source, toolchain and semantic epochs, and states every dimension that
is inconclusive, unsupported, or absent.

It **does not close the phase**. `notes/START_HERE_IMPLEMENTATION.md` line
75 is the governing rule:

> Agents prepare those packages, but continue/narrow/defer/kill decisions
> and every `goal:manual` phase exit remain privileged human actions.

§11 says at length what this document does not do.

---

## 0. What this package concludes, up front

1. **All four clauses of the exit sentence are discharged**, but not all
   in the same way. Three are satisfied on their own terms. The second —
   the ACI ablation — **failed as measured** and is discharged by its
   sentence's *second disjunct*, protocol redesign before freeze, on a
   decision the user ratified on 2026-08-11. §4 states this plainly and
   §0.3 of the plan already records it.
2. **The exit sentence is not the whole of the exit goal.** `bn-1grk`
   names three gates — the G0 freeze subset, G1 and G2 — comprising 16
   criteria. The exit sentence covers four of them. The other twelve have
   evidence of varying strength, and two (G2-01, G2-05) have a named half
   that no instrument in this tree measures. §7 grades all sixteen.
3. **Sixteen per-criterion acceptance bones are open and are scheduled
   *after* this package.** `bn-31yxg` (G0-01), `bn-1k1s8`…`bn-q80m7`
   (G1-01…G1-08) and `bn-1slz1`…`bn-ugee0` (G2-01…G2-07) are all children
   of `bn-1grk`, all depend on `bn-2ofd`, and all are open. Their brief is
   "independently rerun this criterion against the integrated phase
   artifact". §10's recommendation turns on this fact more than on any
   other.
4. **One Phase A deliverable is deliberately unannotated.**
   `PHASE-A-DEL-04` — "evidence graph", [`plan.md`](../plan.md) §21 line
   2151 — carries no `(delivered: …)` annotation. Its successor carrier
   `bn-m6qs9` is open and records, query by query, why five of RFC 0038's
   seven queries cannot be served today. It is not an exit clause, and it
   is not nothing.
5. **The demonstration was run from genuinely clean state**, and its
   artifact identities are recorded in §3 and §8 so that a later run is
   comparable to this one by content address rather than by prose.

---

## 1. The exit sentence, quoted exactly

[`plan.md`](../plan.md) §21, lines 2233–2238, closing the Phase A
deliverable list:

> Exit: Die Hard and Dining Philosophers can be checked through native API,
> CLI, and an agent client with identical artifacts; the ACI ablation shows
> the typed surface beats disciplined shell use on success and cost, or the
> protocol is redesigned before freeze (G0-DX-10); the prompt-injection
> corpus cannot trigger privileged operations (G2); continuation resume
> validates epochs and inputs (G1).

Four clauses, separated by semicolons. Two facts follow from the text
alone, before any measurement:

- **The second clause is a disjunction, not a conjunct.** "the ACI
  ablation shows the typed surface beats disciplined shell use on success
  and cost, **or** the protocol is redesigned before freeze". A package
  that reports this clause as "satisfied" without saying which disjunct
  carried it has not read the sentence.
- **Three of the four clauses name a gate in parentheses** — G0-DX-10, G2,
  G1 — and the first names none. The parenthesised gate is the clause's
  home, not its whole content: G1 has eight criteria and the exit sentence
  quotes one of them.

### 1.1 Verdict vocabulary

Four verdicts are used, and they are not interchangeable. INV-007
(omission and counting transparency) and INV-008 (typed inconclusiveness)
both require that "not measured", "measured and failed" and "unsupported
semantics" stay three different facts.

| Verdict | Meaning |
|---|---|
| **satisfied** | the clause's own condition was measured and holds |
| **satisfied-by-disjunct** | the clause's *first* condition was measured and **failed**; a disjunct the sentence itself offers carried it |
| **inconclusive** | an instrument exists, ran, and its result does not decide the clause |
| **failed** | measured, and the measurement is against the clause |

"Unsupported" and "not measured" are not verdicts here — they are
properties of dimensions, and they live in §9.

---

## 2. The clean-state demonstration

### 2.1 What "clean" meant

The `bn-2ofd` workspace had **no `target/` directory** when this run began.
Every binary below was compiled from source in this run; nothing was reused
from a warm build, and no artifact in §3 existed before the run that
produced it. Each of the three deployments the triple-surface check drives
is a separate `Rig::fresh()`, and
`the_three_surfaces_declare_one_budget_and_one_request_shape` asserts a
fresh deployment has published nothing before a surface touches it — so
every content address in §3 was minted by the run it is attributed to.

Every cargo invocation used `--locked`. `Cargo.lock` is byte-identical to
trunk's at `da177ca`; `git diff HEAD -- Cargo.lock` is empty.

### 2.2 What ran, and what it returned

All suites below were run from the cold build, in this workspace, at
`da177ca`. Every one is an ordinary `#[test]` in the default suite — none
is `#[ignore]`d, so `just check` re-runs all of them.

| Clause / gate | Suite | Result |
|---|---|---:|
| 1 | `continuum-benchmark` `phase_a_triple_surface_identity` | 10 / 10 |
| 2 | `continuum-benchmark` `pr10_impl01_valid_operation_rate` | 8 / 8 |
| 2 | `continuum-benchmark` `pr10_impl02_interface_bytes` | 8 / 8 |
| 2 | `continuum-benchmark` `pr10_impl03_task_completion` | 7 / 7 |
| 2 | `continuum-benchmark` `pr10_impl04_error_recovery` | 7 / 7 |
| 2 | `continuum-benchmark` `pr10_impl05_reproduction` | 7 / 7 |
| 2 | `continuum-benchmark` `dx10_falsification` | 17 / 17 |
| 2 | `continuum-benchmark` `pr10_fallible_families` | 15 / 15 |
| 2 | `continuum-benchmark` `pr10_family_separation` | 6 / 6 |
| 2 | `continuum-benchmark` `philosophers_evidence` | 6 / 6 |
| 2 | `continuum-benchmark` `pr10_c4b_port_by_reference` | 7 / 7 |
| 2 | `continuum-benchmark` `pr10_c5_epochs_at_the_handshake` | 8 / 8 |
| 2 | `continuum-benchmark` `pr10_c7_output_policy_ceiling` | 5 / 5 |
| 3 | `continuum-security` (lib) | 15 / 15 |
| 3 | `continuumd` `g2_injection_corpus_evidence` | 20 / 20 |
| 3 | `continuumd` `inv015_agent_least_authority_evidence` | 32 / 32 |
| 3 | `continuumd` `inv016_untrusted_source_evidence` | 22 / 22 |
| 4 | `continuumd` `dx03_falsification` | 27 / 27 |
| 4 | `continuumd` `daemon_task_operations` | 37 / 37 |
| 4 | `continuumd` `pr6_exit_evidence` | 5 / 5 |
| 4 | `continuumd` `pr6_impl02_budget_evidence` | 10 / 10 |
| 4 | `continuumd` `g1_crash_recovery_evidence` | 16 / 16 |
| 4 | `continuumd` `inv002_no_hidden_state_evidence` | 4 / 4 |
| 4 | `continuumd` `inv006_replay_stability_evidence` | 2 / 2 |
| 4 | `continuumd` `dx14_cancellation_matrix` | 7 / 7 |
| 4 | `continuumd` `dx14_falsification` | 14 / 14 |
| 4 | `continuumd` `task_lifecycle_schedule_matrix` | 5 / 5 |
| 4 | `continuumd` `task_lifecycle_mutation_campaign` | 6 / 6 |
| 4 | `continuum-value` `phase_a_epoch_pinning` | 10 / 10 |
| 4 | `continuum-cli` `task_lifecycle` | 5 / 5 |
| G0 | `continuumd` `dx01_falsification` | 15 / 15 |
| G0 | `continuum-semantic-diff` `dx02_falsification` | 28 / 28 |
| G0 | `continuumd` `dx12_falsification` | 16 / 16 |
| G0 | `continuum-workspace` `dx13_falsification` | 17 / 17 |
| G0 | `continuum-workspace` `dx13_mutation_campaign` | 6 / 6 |

Zero failures, zero ignored, zero filtered.

The dossier validator was run separately and read-only:
`cd notes/plan && uv run --with jsonschema python3 tools/validate_dossier.py`
→ `"status": "pass"`, `"program_status": "ready"`, exit 0.
`validation-results.json` was not written by this package.

### 2.3 The one thing this section cannot claim

A green suite is evidence that the checks pass, not that they are
load-bearing. Every campaign cited here carries its own anti-vacuity
apparatus — mutants, controls, withheld mirrors — and those are recorded
per clause below rather than restated as a blanket claim. Where a clause's
load-bearing-ness rests on a mutation that is **not** in the committed
suite (a manual mutant run and reverted), it is said so.

---

## 3. Clause 1 — triple-surface identical artifacts

> Die Hard and Dining Philosophers can be checked through native API, CLI,
> and an agent client with identical artifacts

**Verdict: SATISFIED**, at the scope §3.4 states.

### 3.1 Delivering bones and artifact

| | |
|---|---|
| Delivering bone | `bn-id4` (done; merged at `9ff1eb179678`) |
| Artifact | `crates/continuum-benchmark/tests/phase_a_triple_surface_identity.rs` |
| Tests | 10, all in the default suite |
| Reproduce | `cargo test --locked -p continuum-benchmark --test phase_a_triple_surface_identity` |

The three surfaces, as the file drives them:

| Plan §21 name | Driven through |
|---|---|
| native API | `Daemon::dispatch` — typed request in, typed outcome out, no adapter, no codec |
| CLI | `continuum_cli::cli::run` with real argv through the real `Scan` and real frames, plus `continuum_cli::wire::Connection` |
| agent client | `continuum_mcp::AgentClient`'s named operations over `continuum_mcp::LocalLink` |

The CLI arm is the **real** `continuum-cli` crate, not
`continuum_benchmark::shell` — which the file's own header records is a
text-projection simulacrum built when no CLI existed, and using it would
have compared the daemon to itself. The "agent client" reading is pinned
mechanically by `the_agent_client_reading_is_the_one_its_own_crate_claims`
against three texts (the `continuum-mcp` crate doc, plan §24.5's DX-10
fallback row naming "MCP-only surface", and `continuum-benchmark`'s own
`NativeSurface` disclaimer), so a reworded source fails the test rather
than leaving the file answering a moved sentence. The same discipline
applies to the exit sentence itself:
`the_exit_sentence_this_file_answers_is_still_the_plans` reads
`plan.md` and fails if the sentence in §1 above changes.

### 3.2 Artifact identities produced in this run

Three independently provisioned deployments per fixture; the addresses
below are byte-identical across all three surfaces on every axis
(`ArtifactHandle`, the bytes it names, the publication receipt ledger,
per-class storage attribution, and the canonical wire encoding of each of
the four answered operations).

**Die Hard (`dh-all`, TV-009)** — snapshot
`ws_7a34d51b57cc040143d11f3cffbb4855b2e9d171bb9e3aee5964df1e54df363c`,
campaign
`task_56abbaa5d4801980dd8e98250652f9b1fe1cc61a335e5f0c0df0d16c75648b5c`,
16 reachable states, verdict refuted.

| Artifact class | Content identity | Bytes | Receipt actor |
|---|---|---:|---|
| `Task` | `4ea779a9fb639086f9a24713506dff7fc67b5fe2e6f7bb4b1322034da8299f92` | 210 | `agent:runner` |
| `WorkspaceSnapshot` | `20ed082b4dc60d234a04684543ce8ed61390f2124d260989d2ef06b65db5256f` | 988 | `agent:builder` |
| `WorkspaceSnapshot` | `7a34d51b57cc040143d11f3cffbb4855b2e9d171bb9e3aee5964df1e54df363c` | 167 | `agent:builder` |
| `WorkspaceSnapshot` | `83ea97e550746547465582f02dc2b7fecf4f41410cb23fb4015b4dbc6655ea55` | 278 | `agent:builder` |
| `WorkspaceSnapshot` | `8816e1759f1d669a8cd89ce22b509086b20cbb103f7c551069195ac231aa1579` | 507 | `agent:builder` |
| `WorkspaceSnapshot` | `c44bcc86591846f07288540ae2bd155fd9ded147dbfa6dc662a58a73bb1e0ef1` | 203 | `agent:builder` |

Storage attribution: `WorkspaceSnapshot` 2,143 B, `Task` 210 B.
Canonical answer bytes: `workspace.create` 335, `verification.start` 429,
`task.status` 1,032, `verification.result` 1,344.
`verification.start`'s one `ArtifactRef` names handle
`task_56abbaa5…` under commitment `task_4ea779a9…`.

**Dining Philosophers (`dp-all`, TV-007)** — snapshot
`ws_ea61098f9c93df3192f58ae17d8408fcaacde5a952f421ac69505793b2ea8584`,
campaign
`task_80ebdb28e4f151dca91ebb365c78f73eefcb3e479109bdc66f19a3b9e140f3c1`,
573 reachable states, verdict refuted.

| Artifact class | Content identity | Bytes | Receipt actor |
|---|---|---:|---|
| `Task` | `dedfeade7469edb19d0df62d60e3f656ec3fbc81e484de6f064cc256acb243c7` | 210 | `agent:runner` |
| `WorkspaceSnapshot` | `0a28a81520d78641a6ddd29db3b002649614371114a62659ad7f99f9e9fb5cd9` | 836 | `agent:builder` |
| `WorkspaceSnapshot` | `4c27e6502644421f61243afda7d49bff3acb35e80c71cda0626810b8839b1eee` | 214 | `agent:builder` |
| `WorkspaceSnapshot` | `8d63ed10a652d3d6bb621af5c963142c8c2ecba2e87837e9cef55124200334a6` | 474 | `agent:builder` |
| `WorkspaceSnapshot` | `ea61098f9c93df3192f58ae17d8408fcaacde5a952f421ac69505793b2ea8584` | 91 | `agent:builder` |

Storage attribution: `WorkspaceSnapshot` 1,615 B, `Task` 210 B.
Canonical answer bytes: `workspace.create` 335, `verification.start` 429,
`task.status` 1,035, `verification.result` 1,355.

These identities were read out of each deployment's own `ReferenceStore`
through the operator audit view — the **same function** for all three
surfaces, so no surface describes its own artifacts. They were extracted
for this document by a temporary print inside the two fixture tests, run
under `--nocapture` and reverted; the committed file is byte-identical to
trunk's.

### 3.3 Anti-vacuity

- `every_published_address_is_the_blake3_identity_of_its_own_bytes`
  recomputes every content address in every deployment from the bytes it
  was read with, through the daemon's own `ContentIdentifier`, and checks
  the receipt ledger names only published identities. "The hashes matched"
  cannot therefore be true of two empty or two mislabelled stores.
- `one_changed_input_changes_the_artifacts_so_the_equality_above_is_a_constraint`
  moves the declared ceiling — which `start_handle` folds into the
  campaign identity — and shows artifacts, answers and task handle all
  change while the verdict and the reachable set do not.
- The frozen dossier answers (16 states / 573 states) are asserted as
  **oracles against what the wire returned**, from live `bfs::explore`
  campaigns, never echoed. An identity across three surfaces is therefore
  not an identity across three wrong answers.
- `bn-id4` records two further mutants run manually and reverted, both
  caught: an agent arm given `ceiling + 1` (caught as "native-api and
  agent-client named different campaigns"), and one byte flipped in the
  CLI arm's published artifact (caught twice — "published different
  artifacts" and "holds an artifact whose address does not name its
  bytes"). **These two are not in the committed suite.**

### 3.4 Scope, stated plainly

- **Two artifact classes of plan §4.4's nineteen.** The store's own
  attribution for both fixtures names exactly `WorkspaceSnapshot` and
  `Task`. No `Certificate`, no `ContextPack`, no `Crashpack`, no evidence
  node is compared, because none is published by this four-operation
  script. "Identical artifacts" is demonstrated over the two classes this
  script produces.
- **A four-operation script.** `workspace.create` → `verification.start` →
  `task.status` → `verification.result`. It is four rather than three
  because `verification.result` carries **no** reachable-state count and
  **no** artifact refs (`Cost.states` is `Absent`, `artifacts` empty) — an
  exit check reading only the result operation would have had nothing
  content-addressed to compare. That is `bn-id4`'s finding 1, recorded as
  a property of the wire.
- **One in-process transport.** `LocalLink`/`LocalPair`. `continuum-cli`'s
  own binary answers `LinkError::NoTransportConfigured` at exit 2 because
  `continuumd` ships no socket or process transport. The three surfaces
  are compared across a byte boundary in-process, not across a socket.
- **`request_id` and `idempotency_key` differ across surfaces by
  construction** and are deliberately excluded from the comparison. That
  they reach no artifact identity is precisely what makes "projections of
  one truth" true; holding them equal would have hidden the result.
- **The CLI's `DEFAULT_PROTOCOL_VERSION` is `(3,2)`** while the deployment
  negotiates 3.6, so every CLI invocation passes `--protocol-version`
  explicitly. Tripwired by `bn-id4`, not fixed — real behaviour, out of
  that bone's scope.
- Four typed absences are asserted **still absent** by
  `the_absences_this_check_runs_around_are_still_absent`, each with a
  tripwire that fails when the absence is filled: no Dining Philosophers
  Intent Contract fixture (both ports run under `die-hard-contract.json`);
  no certificate for Dining Philosophers; no crashpack producer anywhere,
  so neither the depth-6 solution nor the depth-10 deadlock is an artifact
  to compare; and no transition count or witness depth on the wire (RFC
  0026 F16).

---

## 4. Clause 2 — the ACI ablation

> the ACI ablation shows the typed surface beats disciplined shell use on
> success and cost, **or** the protocol is redesigned before freeze
> (G0-DX-10)

**Verdict: SATISFIED-BY-DISJUNCT.**
**The first disjunct FAILED as measured.** The clause is discharged by its
second disjunct — protocol redesign before freeze — on a decision the user
ratified on 2026-08-11.

This is the clause most easily mis-reported, so this section states the
outcome before the evidence.

### 4.1 The measured outcome

The ratified numeric definition of "beats" is [`plan.md`](../plan.md) line
2722, `quote-id=aci-benchmark-margins`: at least +10 percentage points of
absolute task success, at least 30% fewer interface bytes per solved task,
and at least a 50% relative reduction in invalid-action rate; **missing any
one of the three fails G0-DX-10 and forces protocol redesign before
freeze.**

Post-redesign, at the protocol-3.6 freeze boundary:

| Ratified margin | Floor | Measured | Verdict |
|---|---|---|---|
| Absolute task success | ≥ +10 pp, one-sided 95% LB above zero | 24/24 native, 24/24 shell — margin 0 pp | **TIE — not a pass** |
| Interface bytes per solved task | ≥ 30% fewer | 10,384 B native vs 4,118 B shell = **−152%** | **FAIL, and unreachable losslessly** |
| Invalid-action rate | ≥ 50% relative reduction | **61%** reduction (0.95 shell-only invalid attempts/run against a 0.60 break-even) | **PASS** |

Two of three margins are missed. Under the composite rule, **G0-DX-10 fails
as measured.** [`G0_SPIKE_MATRIX.md`](G0_SPIKE_MATRIX.md)'s DX-10 Status
column reads, verbatim:

> Closed (failed as measured — consequence discharged by user-ratified
> redesign, bn-762i)

The bytes margin was measured at −163% at first adjudication and improved
to −152% when `workspace.create_by_reference` landed at protocol 3.6
(`bn-3of5h`). Both figures are failures against a +30% floor.

**A tie is not a partial pass.** The success tie is the most robust result
in the lane — it holds under every accounting variant, every leave-one-out
subset and both policies — and breaks only where the *baseline* is
degraded. It is recorded as a tie.

### 4.2 Why the bytes margin is unreachable, not merely unmet

RFC 0027 correction 31's addendum (`bn-zsi1v`) proves the stronger
statement. With `H` the handshake charged per solved task, `n` the calls
per task, `rn` the typed arm's bytes per call and `rs` the text arm's, a
passing row requires `H + n·rn ≤ 0.7·n·rs`. On the measured values
`H` = 875 B, `n` = 7, `rs` = 575 B/call, that requires `rn` ≤ 277 B/call —
request and result together. The typed arm measures 1,551 B/call and the
demonstrated post-redesign floor is 679 B/call.

Without the algebra: the text arm's 575 B/call is a **lossy rendering of an
answer**; the typed arm's is a **lossless self-describing encoding of the
same answer plus the request that asked for it**. The margin asks a
lossless encoding of answer-plus-request to come in under half a lossy
rendering of the answer alone. The +30% bytes margin is therefore
**unreachable under the pinned accounting by any lossless protocol**, and
the addendum is normative that a redesign MUST NOT be commissioned with a
passing bytes row as its goal — a redesign that reached the row would have
become a rendering with extra steps, having discarded the omission
manifests, epoch pinning, assurance envelope and artifact identity that are
the typed surface's reason to exist. The redesign's obligation is **waste
removal, not row-flipping.**

### 4.3 The accounting question, and its name

Whose interface "interface bytes per solved task" counts was the one attack
`bn-2c0a`'s campaign could not decide. **It is not "X1" in this document's
vocabulary**: RFC 0027 uses X1/X2/X3 as its own cross-cutting security
labels, and correction 31 says so explicitly — the `X1` label for the
byte-accounting question is local to `crates/continuum-benchmark`'s suite.
Referred to here as *the byte-accounting question*.

It was decided afterwards, on the governing text, by **RFC 0027 correction
31**: an interface byte is a byte the agent itself wrote or read, because
bytes are the graded denominator *only because token counts are
model-relative*, and that reason reaches only content entering an agent's
context. Process-internal frames are uncounted on both arms alike. The
rejected symmetric reading — under which the margin reads +53% and would
clear — is recorded in the RFC at full strength with its argument, together
with the record that adopting it once the numbers were known is the move
the campaign that raised the question warned against.

### 4.4 The redesign that discharges the second disjunct

Executed inside protocol major 3:

| Item | Bone | Effect | Test |
|---|---|---|---|
| `OutputPolicy.max_bytes` enforced, first time | `bn-6fuu5` | −49 to −255 B/solved | `tests/pr10_c7_output_policy_ceiling.rs` |
| `workspace.create_by_reference` at 3.6 | `bn-3of5h` | −475 B/solved, shell arm byte-for-byte unmoved | `tests/pr10_c4b_port_by_reference.rs` |
| `epochs` pinned at the handshake | `bn-2in7i` | ledger item C5 | `tests/pr10_c5_epochs_at_the_handshake.rs` |

The remaining **3,566 B per solved task — 87.8% of the corrected redesign
set — sits behind protocol major 4.0**, ratified as the standing 4.0 plan
on `bn-3861i` (open) with per-item verdicts, the quoted blocking
declarations, and the plan §4.6 typed-compatibility-statement requirement.
C6 connection-scoped handle aliases are recorded **permanently declined**
at any version — they silent-decode on un-upgraded clients, an INV-002
matter that may not be re-proposed.

**Phase A's protocol freezes at 3.6.**

### 4.5 The privileged decision that carried it

`bn-762i` closed on the exit's second disjunct. Quoted from its
ratification comment, via
[`KILL08_CHECKPOINT.md`](KILL08_CHECKPOINT.md) §5:

> USER RATIFIED OPTION 1 (2026-08-11): the second disjunct is discharged —
> redesign executed to the major-3 constitutional boundary (OutputPolicy
> enforcement + workspace.create_by_reference at 3.6, measured), remainder
> ratified as the standing 4.0 plan on bn-3861i. Phase A's protocol freezes
> at 3.6. C6 recorded permanently declined per the package footnote, no
> objection raised.

The same evidence fired KILL-08 ("agent-native API does not beat
disciplined CLI use"), whose checkpoint is
[`KILL08_CHECKPOINT.md`](KILL08_CHECKPOINT.md) (`bn-3ety`). That
checkpoint's decision — CONTINUE with redesign-as-waste-removal — is the
same user decision of 2026-08-11, not a second one.

### 4.6 Delivering bones and reproduction

| | |
|---|---|
| Instrument | `bn-134i`, `bn-23gm`, `bn-3awq`, `bn-1udt`, `bn-26tb` — `crates/continuum-benchmark/`, `crates/continuum-mcp/` |
| Two-sided falsification | `bn-2c0a` — `tests/dx10_falsification.rs`, 17 tests, 8 attacks landed / 4 held / 1 inconclusive |
| Fallible policy families | `bn-2phq3` — `src/families.rs`, `tests/pr10_fallible_families.rs`, 15 tests |
| Byte ledger | `bn-3bn62`, `bn-zsi1v`, `bn-199nx` — [`DX10_BYTE_LEDGER.md`](DX10_BYTE_LEDGER.md) |
| Adjudication and closure | `bn-762i`; RFC 0027 correction 31 and its addendum |
| Kill-criterion checkpoint | `bn-3ety` — [`KILL08_CHECKPOINT.md`](KILL08_CHECKPOINT.md) |
| Reproduce | `cargo test --locked -p continuum-benchmark` |

Stable artifact IDs, each a byte-stable typed artifact with a versioned
first line: `continuum-benchmark aci-report v1`,
`continuum-benchmark dx10-falsification v1`,
`continuum-benchmark fallible-policy-families v1`. Ratified thresholds live
in code as `report::RATIFIED`, so a drifted threshold is a compile-visible
change.

---

## 5. Clause 3 — the prompt-injection corpus (G2)

> the prompt-injection corpus cannot trigger privileged operations (G2)

**Verdict: SATISFIED**, at the narrower Phase A scope §5.4 states.

### 5.1 Delivering bone and artifacts

| | |
|---|---|
| Delivering bone | `bn-ymw` (done; merged at `c011608`) |
| Corpus | `crates/continuum-security/src/injection.rs` — 45 cases as inert data, 15 in-crate shape tests |
| Evidence | `crates/continuumd/tests/g2_injection_corpus_evidence.rs` — 20 tests |
| Reproduce | `cargo test --locked -p continuum-security --lib` and `cargo test --locked -p continuumd --test g2_injection_corpus_evidence` |

Every case is driven through the **real wire boundary** — a byte frame into
`Server::answer`, a byte frame back. The corpus shape is not invented: plan
§24.5 quotes research/35's ratified promotion gate verbatim, and that
sentence fixes it — three cases for each of ten red-team classes, one case
for each of the seven docs/49 intent-policy blocks, and one escape attempt
against each of its eight worker-isolation controls. All three enumerations
are transcriptions carrying their dossier bullet verbatim, and the floor is
recomputed from the tables rather than written down.

"Across every agent-readable artifact class" is mechanical rather than
asserted: `injection::readability` classifies each variant of the closed
`ArtifactClass`, so a class the plan adds is a non-exhaustive match rather
than a silent gap. Eighteen of plan §4.4's nineteen carry agent-readable
content and each has at least one case; the nineteenth is `cap_*`, excluded
because RFC 0027 S5 keeps a capability out of every trace, error and
rendering.

### 5.2 The three claims it holds

1. **Zero privileged operations triggered**, read off the admission ledger
   (`AdmissionRecord.admitted`, the record RFC 0027 P5 requires), *not* off
   an error code. That distinction is load-bearing: `CapabilityDenied` is
   also what a handler answers for an artifact the daemon does not hold,
   because X2 forbids a distinguishable not-found, so the wire code alone
   cannot separate "refused by admission" from "no such record".
2. **Zero prohibited outcomes** — a before/after comparison of every
   intent's registry status, every evidence claim's status, and the store's
   authorization audit log. That is research/35's first two prohibited
   outcomes literally, rather than an absence of error codes.
3. **Capability checks, not output filtering.** `cap_reviser` sits at the
   steward's own `revise-intent` level with an *empty*
   `privileged_operations` list — docs/49's "revise intent: proposal only"
   cell — so byte-identical request frames are denied for it and admitted
   for the steward, while it is still admitted to
   `intent.propose_revision`. That control is what shows this is the
   privilege bit and not the level ladder.

**The not-a-filter half.**
`inert::an_injected_payload_is_stored_and_returned_verbatim_never_scrubbed`
is the negative of a sanitizer: the steward's privileged `intent.accept`
carrying an injection in the acceptance signature *succeeds*, the status
moves to accepted, and the payload comes back through `intent.get` byte for
byte. This is also the liveness proof for the whole file — the corpus's
denied attempts are attempts at a real operation. If output filtering is
ever added it fails this test, correctly, because a filter here would be a
second, weaker enforcement path standing beside the capability check.

### 5.3 Anti-vacuity, re-proved live

`bn-ymw` mutation-tested each load-bearing negative against current trunk
rather than trusting a first-run green:

- neutering `spec.has(Annotation::Privileged) && !grants_operation(…)` in
  `admission.rs` fails three tests, including
  `no_case_in_the_corpus_triggers_a_privileged_operation` at the
  `!record.admitted` ledger read;
- adding a naive output filter to the acceptance signature in
  `daemon/intent.rs` fails exactly the verbatim-return test;
- `the_corpus_drives_at_every_privileged_operation_this_daemon_serves`
  reads `registry::OPERATIONS` live, filtered on `Annotation::Privileged`;
  retargeting the corpus's three `intent.lock` cases makes it fail with
  "no corpus case drives at the privileged operation intent.lock". The
  privileged set is five: `intent.accept`, `intent.reject`, `intent.lock`,
  `repair.promote`, `repair.reject`.

### 5.4 The two honest edges, carried forward

These are `bn-ymw`'s own words in substance, and they are not softened
here.

1. **Seven corpus operations have no worker in this process.**
   `program.run`, `program.replay`, `proof.attempt`, `proof.check`,
   `forge.step`, `benchmark.run`, `debug.export`. There is no solver
   worker, no Lean worker, no generated-code runner and no benchmark
   grader here, so an escape attempt against one is refused by the codec
   before its payload is read — **an accounting of absence, not a
   defence.** What is proved is that the absence is *total*: an identical
   answer for every payload on the wire, and `decode_arguments` refusing
   by operation name. The corpus carries those cases so the lane that
   lands each worker inherits its escape attempt.
2. **This clears the narrower Phase A exit condition, NOT plan §24.5's
   full ratified promotion gate.** That gate additionally binds a run to a
   build and dependency epoch, requires refusal *and* an append-only audit
   record per case, and re-locks on any later success. This corpus is the
   seed its Phase B lane grows.

A third boundary is recorded by `bn-ymw` and belongs here: `OutputPolicy`
has **no effect** on this evidence. Every corpus frame sets
`output_policy: Absent`, which `Ceiling::of` maps to `Ceiling::UNBOUNDED`,
so no answer is trimmed and no disposition changed.

### 5.5 The tripwire that fired, and how it was repaired

`inv015_agent_least_authority_evidence.rs` asserted `continuum-security`
had zero `pub` items. `bn-ymw` gave that crate its first content, so the
assertion failed. It was **not** dropped and **not** relaxed: it moved to a
four-leg audit re-establishing the property the sweep actually guards — no
crate on that list can execute a host effect on an agent's behalf —
inventoried public surface at 31 items, no host-effect facility named
anywhere in the crate, one model-side dependency, and dev-position-only
reachability from `continuumd`. Leg 2 carries real weight because the
payloads are prose *about* spawning processes and exfiltrating files, so
the predicate must separate talking about an effect from performing one;
`negative_the_security_corpus_audit_detects_its_mutants` holds all four
legs to failure against doctored inputs and asserts the real corpus is
clean under the leg-2 predicate. INV-015 went 30 → 32 tests, all green in
this run.

---

## 6. Clause 4 — continuation resume validates epochs and inputs (G1)

> continuation resume validates epochs and inputs (G1)

**Verdict: SATISFIED**, within one daemon process lifetime. §6.4 states the
boundary, which is material.

### 6.1 The production predicate

`crates/continuumd/src/daemon/task.rs`, `fn resume` — RFC 0026's resume
decision table, in the order the function checks it:

| Condition | Error |
|---|---|
| the continuation is not one this daemon holds | `CapabilityDenied` (X2: no distinguishable not-found) |
| the envelope names a snapshot other than the one the continuation pinned | `StaleSnapshot` |
| the pinned snapshot is not held at all | `CapabilityDenied` |
| the pinned snapshot is held but is not sealed, or has been superseded in its lineage | `StaleSnapshot` |
| a pinned epoch names a kind the daemon pins no identity for | `EpochUnsupported` |
| a pinned epoch disagrees with the daemon's current epoch of that kind (P1) | `ContinuationEpochMismatch` |
| the pinned engine identity disagrees with the daemon's (P2) | `ContinuationEpochMismatch` |
| the model the continuation names is no longer one this daemon can construct | `UnsupportedSemanticFeature` |

Both epoch predicates are checked and **neither implies the other** (RFC
0026's two-predicate obligation): P1 alone would admit a resume onto a
different engine build — the case plan §4.7's defect lifecycle exists to
catch — and P2 alone a resume across a semantic-epoch advance, which
ADR-0018 forbids. **The protocol epoch does not participate**, and that is
a property of the type rather than a step somebody has to remember:
`PinnedEpochs` has no `protocol` field at all, because SD-13 forbids the
protocol epoch from entering the identity of any artifact, snapshot or
task.

The "inputs" half is the snapshot and lineage rows: a superseded, unsealed
or differently-named snapshot is refused before any reuse.

### 6.2 Delivering bones and artifacts

| Sub-claim | Bone | Artifact |
|---|---|---|
| the epoch algebra, both faces | `bn-221j` | `crates/continuum-value/tests/phase_a_epoch_pinning.rs` — 10 tests, property-based over all 64 pin combinations, with two seeded-violation falsification tests and retained shrunk counterexamples |
| the wire predicate, epoch-mismatch vs unheld-epoch | PR 5 / PR 6 | `crates/continuumd/tests/daemon_task_operations.rs::resume_admissibility_separates_an_epoch_disagreement_from_an_unheld_epoch` and `::the_two_predicate_obligation_is_checked_on_both_halves` |
| adversarial campaign, 22 attacks over 4 axes | `bn-1kp6` | `crates/continuumd/tests/dx03_falsification.rs` — 27 tests, including `attack_reuse_a_continuation_is_refused_when_the_daemons_epochs_moved` and `attack_staleness_every_snapshot_consuming_operation_refuses_a_superseded_handle` |
| PR 6 lifecycle exit | `bn-19u` | `crates/continuumd/tests/pr6_exit_evidence.rs`, `pr6_impl02_budget_evidence.rs` |
| cancellation calculus | `bn-2gk`, `bn-1gc`, `bn-12t`, `bn-2zy`, `bn-3p32` | `dx14_cancellation_matrix.rs`, `dx14_falsification.rs` |
| replay stability under a pinned epoch (INV-006) | PR-5 wave (annotated at the INV-006 heading) | `crates/continuumd/tests/inv006_replay_stability_evidence.rs` |
| the protocol-minor rule (a minor difference is **not** a mismatch) | — | `crates/continuumd/tests/inv002_no_hidden_state_evidence.rs` |
| daemon crash recovery (G1's last bullet) | `bn-3dr` | `crates/continuumd/src/daemon/recovery.rs`, `crates/continuumd/tests/g1_crash_recovery_evidence.rs` — 16 tests |
| the CLI surface | `bn-3tz60` | `crates/continuum-cli/src/task.rs`, `crates/continuum-cli/tests/task_lifecycle.rs` |

Reproduce: `cargo test --locked -p continuumd --test dx03_falsification --test daemon_task_operations --test g1_crash_recovery_evidence` and
`cargo test --locked -p continuum-value --test phase_a_epoch_pinning`.

### 6.3 What the campaign proved by falsifying

`bn-1kp6`'s DX-03 campaign held `src/` frozen and **falsified two of the
three conjuncts**. Five attacks landed on three daemon-layer defects:

1. `workspace.create` named a lineage by the content identity of the
   snapshot it created and inserted it unconditionally, so an identical
   create under a fresh idempotency key rewound the `Fork` to its origin —
   the superseded snapshot was served again by `verification.start` and
   `task.resume`. Fixed by `bn-n1xou` (put-if-absent on the lineage,
   `sealed` monotone).
2. `task.resume` applied the request's budget *before* it tested
   `is_terminal`, so it wrote a cancelled task's budget and its documented
   no-op was not one. Fixed by `bn-10093`.
3. `ReplayKey` omitted `RequestEnvelope.budget`, so one key answered two
   different canonical requests. Fixed by `bn-h1zqz`.

All five reproductions run un-ignored in the default suite as regression
guards. That the evidence was produced against a frozen `src/` is what
makes it evidence rather than a fixture shaped around the implementation.

### 6.4 The boundary that matters most

**A continuation does not survive a daemon restart, by design.** `bn-3dr`
declined to build a disk-backed daemon-state store, and recorded the reason
at the type: plan §4.5's restart contract is about the store plus "resume
from the last committed continuation or transition to `Failed` with a typed
reason", and a task table that never reached a disk has no committed
continuation to resume from; inferring one from the store is exactly the
"silently reconstructed state" the same paragraph prohibits.

So thirteen `VolatileFact`s are enumerated with a `Disposition` each —
three `reprovisioned`, ten **declared as lost, never reconstructed** —
including the task table and the continuation table. The suite checks the
declaration behaviourally: after a restart, `task.status` is denied,
`task.resume` on the surviving continuation is denied, and the idempotency
key no longer replays.

The consequence for this clause, stated exactly: **"continuation resume
validates epochs and inputs" is proved for resumes within one daemon
process lifetime.** Across a restart there is no continuation to validate.
That is an honest architecture for a build with no disk-backed daemon
state, and it is a real limit on the sentence's reach.

Three further scope statements, each from the delivering bone rather than
invented here:

- **One thread, one request at a time.** `Daemon::dispatch` takes
  `&mut self`. Daemon-scale linearizability is open and arrives with the
  concurrent daemon (docs/35); the store's own concurrency is covered by
  DX-13's 48-thread campaign.
- **The crash grain is coarse.** The store is in-memory and "survives"
  because the harness moves the value, not because anything was flushed. A
  real crash between a store write and its `fsync` is below this model.
- **`ContinuationEpochMismatch` and `EpochUnsupported` are not
  wire-exercised from the CLI.** `bn-3tz60` records this: the CLI's
  `StaleSnapshot` refusal *is* wire-tested against a real daemon
  (`resume_refuses_a_continuation_whose_pinned_snapshot_the_lineage_superseded`,
  reproducing DX-03's attack-6 recipe), but the two epoch codes share the
  same render path and were not separately driven, because doing so needs
  a second, differently-epoched daemon. They **are** wire-exercised at the
  daemon layer (§6.2 row 2), so the gap is in the CLI projection, not in
  the predicate.

---

## 7. The gates the exit goal names

`bn-1grk` carries `req:g0`, `req:g1`, `req:g2`. Sixteen criteria.

### 7.1 G0 — the freeze-blocking subset

G0-01's own condition is that every load-bearing experiment has **"evidence
or an explicit redesign decision, recorded in the matrix itself"**, and
that no freeze-blocking item is failed or unexecuted without its
consequence in force.

| Item | Matrix Status | This run |
|---|---|---:|
| G0-DX-01 | Evidence (reference implementation) | `dx01_falsification` 15/15 |
| G0-DX-02 | Evidence (reference implementation) | `dx02_falsification` 28/28 |
| G0-DX-03 | Evidence (reference implementation) | `dx03_falsification` 27/27 |
| G0-DX-10 | **Closed (failed as measured — consequence discharged by user-ratified redesign, bn-762i)** | `dx10_falsification` 17/17 |
| G0-DX-12 | Evidence (reference implementation) | `dx12_falsification` 16/16 |
| G0-DX-13 | Evidence (reference implementation) | `dx13_falsification` 17/17, `dx13_mutation_campaign` 6/6 |
| G0-DX-14 | Evidence (reference implementation) | `dx14_cancellation_matrix` 7/7, `dx14_falsification` 14/14 |

**G0-01 verdict: satisfied.** Every freeze-blocking item carries evidence
or an explicit decision recorded in the matrix. DX-10's decision is an
explicit *failure* with the named consequence carried out; the criterion's
text admits exactly that, and plan §0.3's derived counts already say so.
Three of the seven (DX-03, DX-13, DX-14) were **falsified and repaired**
rather than passing first time, and their reproductions are the regression
guards that hold their Status up.

### 7.2 G1 — workbench identity and lifecycle

| # | Criterion | Standing |
|---|---|---|
| G1-01 | snapshots, intent contracts, handles, artifacts immutable and content-addressed | evidenced — `phase_a_epoch_pinning.rs`, `continuum-value/src/identity.rs`, `continuum-workspace/src/publication.rs`, and §3's own address/bytes re-derivation |
| G1-02 | explicit handles across the native API | evidenced — the registry's typed handle vocabulary; `idl_conformance.rs` + `registry_agreement.rs` hold the transcription to the IDL and to RFC 0027's independent table |
| G1-03 | requests are idempotent under idempotency keys | evidenced and **falsified-then-fixed** — `dx03_falsification.rs` idempotence axis; `ReplayKey` defect fixed by `bn-h1zqz` |
| G1-04 | continuation resume validates epochs and inputs before any reuse | evidenced — §6; **bounded to one process lifetime** (§6.4) |
| G1-05 | cancellation closes obligations and publishes no partial finality | evidenced — DX-14 matrix (324 + 108 + 27 + 5,040 runs) and `dx14_falsification.rs`; **three of the four engines do not exist**, so the lanes are lifecycle *profiles* rather than engines |
| G1-06 | artifact publication is transactional (INV-017) | evidenced — DX-13 cycle; the GC/publication race was **falsified** and repaired by `bn-2siid` |
| G1-07 | authorization is checked independently of handle possession | evidenced — `g2_injection_corpus_evidence.rs` capability/level control, `dx12_falsification.rs` authority axis, `inv015` |
| G1-08 | daemon crash recovery leaves no stale index entries or orphan tasks | evidenced — `g1_crash_recovery_evidence.rs`, 16 tests, nine dispatch boundaries composed with the store's own `PublicationPhase` seam; **the task record is declared lost, not recovered** (§6.4) |

**G1 verdict: evidence present for all eight; two carry material bounds
(G1-04, G1-05) and one carries a declared architectural decline (G1-08).**
The criterion-level acceptance re-runs are `bn-1k1s8`, `bn-3j01v`,
`bn-3huh7`, `bn-16v3x`, `bn-1tkrp`, `bn-2vbqm`, `bn-cxd2y`, `bn-q80m7` —
**all open**.

### 7.3 G2 — agent-computer interface

The criterion numbers below follow
[`docs/52`](../docs/52_RELEASE_GATES_REV3.md), which is the registry's
source and the acceptance bones' cited authority.
[`plan.md`](../plan.md) §22 lists the same seven bullets in a **different
order** — "no terminal parsing required" is its first — so a reader
checking §22 should match on text, not on position.

| # | Criterion | Standing |
|---|---|---|
| G2-01 | generated clients and schemas ship for the native protocol | **partial — see below** |
| G2-02 | no terminal parsing required | evidenced — `continuum-mcp/tests/typed_surface.rs`; every argument and every answer, success or refusal, is a typed value; nothing is parsed out of a rendered line |
| G2-03 | explicit handles and resumability | evidenced — §6 and `dx03_falsification.rs` |
| G2-04 | stale state rejected | evidenced — `dx03_falsification.rs`'s stale-snapshot axis over **every** snapshot-consuming operation |
| G2-05 | Context Packs are bounded, carry omission manifests and expansion handles, **and improve agent benchmark effectiveness** | first half evidenced (`pr11_exit_evidence.rs`, `inv007_omission_transparency_evidence.rs`, RFC 0028's compiler); **second half unmeasured — no instrument exists** |
| G2-06 | native ACI beats the disciplined shell baseline …, or the protocol is redesigned before freeze | **satisfied-by-disjunct** — §4 |
| G2-07 | the prompt-injection corpus cannot trigger privileged operations | evidenced — §5 |

**G2-01, stated precisely.** *Schemas* ship: 20 JSON Schema documents under
`notes/plan/schemas/` at `schema_epoch` 1, plus the normative
`continuumd-native-protocol.idl`. *Generated clients* do not: there is no
generator anywhere in the tree — no `build.rs` in any crate, no codegen
tool under `tools/`. `continuumd/src/protocol` is a **hand transcription**
of the IDL, held to it by two independently written checkers
(`idl_conformance.rs` parses the IDL from its own EBNF and shares no code
with the types; `registry_agreement.rs` checks the same types against RFC
0027's independently maintained authority table), and
`continuum_mcp::AgentClient` is **hand-written** with nine named wire
operations of the registry's 75 (its own comment still says "eight",
stale since `workspace.create_by_reference` landed at 3.6). `invoke` is
public and takes the closed `Arguments` enum, so the client is not
*limited* to nine — but nothing is generated. Whether a mechanically
conformance-checked transcription satisfies "generated" is a reading
question that belongs to G2-01's acceptance bone `bn-2wypi`, not to this
package.

**G2-05, stated precisely.** The Phase A benchmark instrument
(`crates/continuum-benchmark/`) does not touch Context Packs at all — no
source or test file in that crate references `ContextPack`,
`context.compile`, or a pack-versus-no-pack arm. The only occurrences of
"Context Pack ablation" in the whole tree are the *quotations of the plan
sentence requiring one*, in `src/separation.rs` and
`tests/pr10_family_separation.rs`. The §19.4 family/source-hash separation
gate that plan §22 requires **before either result is accepted** is built
and passing (`pr10_family_separation` 6/6), so the instrument is validated
— for the DX-10 result. **The G2 Context Pack ablation has not been
built.** "Improve agent benchmark effectiveness" is *not measured*, which
is a different fact from measured-and-failed.

**G2 verdict: five of seven evidenced, one satisfied-by-disjunct, and two
carrying a named unmeasured half (G2-01's "generated", G2-05's "improve").**
The criterion-level acceptance re-runs are `bn-2wypi`, `bn-1slz1`,
`bn-24ulk`, `bn-3r10g`, `bn-1iljt`, `bn-39uby`, `bn-ugee0` — **all open**.

---

## 8. Epoch binding

Per plan §4.6 and ADR-0018: epochs are content identities, advancing one
never mutates an existing artifact, and the six compatibility epochs MUST
NOT be conflated with one another or with anything that is not an epoch.

### 8.1 The three identities, kept separate

ADR-0018 and `notes/plan/schemas/README.md` distinguish three things this
package must not conflate:

| Identity | What it names | Example, for the §3 artifacts |
|---|---|---|
| **Schema-document identity** | one artifact class's encoding contract at one `schema_epoch`; exactly one document per (class, epoch) pair | `https://continuum.dev/schema/v1/workspace-snapshot.json` |
| **Class identity** | the artifact class across every epoch; never changes; what prose, RFCs and the IDL cite; MUST NOT be a `$id` | `https://continuum.dev/schema/workspace-snapshot.json` |
| **Instance identity** | the `schema_id`/`schema_epoch` header an artifact carries, and the content address the store files it under | `ArtifactHandle { class: WorkspaceSnapshot, identity: "7a34d51b…" }` |

`schema_epoch` is **not** a seventh epoch. It is an epoch advance in
ADR-0018's sense — same compatibility statement, same two-epoch rule — but
an evidence epoch MUST NOT be inferred from a `schema_epoch` value.

### 8.2 The binding table

| Epoch / identity | Kind | Value in this run | Where it is written |
|---|---|---|---|
| Source | git commit | `da177ca9b4faadbc056f001143794a887195b8d6` | this repository |
| Toolchain (Rust) | pinned build identity | `1.97.0` — `rustc 1.97.0 (2d8144b78 2026-07-07)` | `rust-toolchain.toml`, exact patch by INV-014/INV-005 |
| Toolchain (Lean) | pinned build identity | `leanprover/lean4:v4.32.1` | `lean/lean-toolchain`, via elan pinned in `mise.toml` |
| Dependency closure | lockfile | `Cargo.lock` byte-identical to trunk at `da177ca`; audited by `tools/governance/dependency-audits.toml` | `--locked` on every invocation |
| **protocol** | compatibility epoch | **3.6** (`PROTOCOL_VERSION`), IDL 1.11, 75 operations — **frozen here for Phase A** | `crates/continuumd/src/protocol/registry.rs` |
| **semantic** | compatibility epoch | `semantic-1` | `continuum_benchmark::rig::epochs()` |
| **intent** | compatibility epoch | `intent-1` | idem |
| **evidence** | compatibility epoch | **`Null` — deliberately unpinned by this deployment** | idem |
| **proof** | compatibility epoch | `proof-1` | idem |
| **corpus** | compatibility epoch | `tla-examples-1` | idem |
| engine identity | provenance, **not** an epoch (plan §4.7) | `engine-reference-1` | idem |
| `schema_epoch` | encoding contract, **not** an epoch of the six | `1` for every one of the 20 schema documents | `notes/plan/schemas/*.schema.json` |

### 8.3 What the binding does and does not establish

- **The protocol epoch is absent from every artifact identity in §3, by
  rule.** ADR-0018 and SD-13: a protocol-major bump MUST NOT invalidate or
  reinterpret a published artifact, so the protocol epoch MUST NOT enter
  the identity of any artifact, snapshot or task. It is a connection
  property. `PinnedEpochs` has no `protocol` field, which makes the rule a
  property of the type.
- **A publication's identity deliberately omits the epochs.** `bn-3dr` and
  `bn-23j7s` recorded and closed this: the store derives identity from
  bytes and nothing else, so the binding needs no epoch-scoped identity,
  and widening it would break
  `two_daemons_name_one_campaigns_publications_identically` whenever two
  daemons' epoch sets differ. **The epochs ride the record, not the
  address.** So the addresses in §3 are reproducible under any epoch set
  that produces the same bytes — which is the point of content addressing,
  and is also why the epoch binding must be stated separately, as it is
  here, rather than read off a handle.
- **The evidence epoch is `Null` in this deployment.** Five of six
  compatibility epochs are pinned; the evidence epoch is explicitly
  unpinned. Under `EpochSet::first_mismatch`'s own rule an unpinned epoch
  constrains nothing, so a continuation minted here pins no evidence epoch
  and a resume cannot be refused on one. This is a declared `Null`, not an
  omission — but it means clause 4's guarantee covers five epochs, not
  six, on this deployment's own configuration.
- **No epoch advance has been exercised.** `EpochAdvance` refuses an
  advance whose successor identity equals its predecessor, and the
  `Preserved | Revalidate | Incompatible` compatibility statement is
  implemented — but no Phase A artifact has survived an actual advance,
  because none has occurred. ADR-0018's "an advance never mutates an
  existing artifact" is upheld by construction and by the property suite,
  not by a migration this package can point at.

---

## 9. Inconclusive, unsupported, and stated absences

This section is the reason the package exists. It separates three facts
INV-007 and INV-008 both require to stay separate: **not measured**,
**measured and failed**, and **unsupported semantics**.

### 9.1 Measured and failed

| # | What | Where |
|---|---|---|
| F1 | **The ACI ablation's interface-bytes margin: −152% against a +30% floor.** Not an encoding artefact — reducing the whole result envelope to its floor and resolving `SnapshotComponents` by reference still leaves −74% at protocol 3.6, and the addendum proves no lossless protocol clears it. | §4; `G0_SPIKE_MATRIX.md` DX-10; RFC 0027 correction 31 |
| F2 | **G0-DX-10's pass condition is not met and the row closes as `failed`.** Success is a measured tie, not a pass. The clause is carried by its second disjunct. | §4.1 |
| F3 | **Three G0 freeze-blocking items were falsified before they were repaired:** DX-03 (three daemon defects), DX-13 (GC-vs-publication race), DX-14 (`task.resume` writing a terminal task's budget). Each fix landed with the campaign's own reproduction as its regression guard. | §7.1 |

### 9.2 Not measured — no instrument exists

| # | What | Owner |
|---|---|---|
| N1 | **The G2 Context Pack ablation.** G2-05's second half — "improve agent benchmark effectiveness" — has no instrument anywhere in the tree. Not failed, not inconclusive: **unbuilt.** | `bn-1iljt` (G2-05 acceptance), open |
| N2 | **Generated clients.** No generator exists; the protocol transcription and the agent client are hand-written and conformance-checked. Whether that satisfies G2-01's "generated" is undecided. | `bn-2wypi` (G2-01 acceptance), open |
| N3 | **Whether a live model would use either surface better.** Every DX-10 variant varies the interface or the accounting; **none varies the agent.** The whole assay is scripted policies over 4 tasks and 2 semantic families. research/25's own mechanism for the invalid-action margin — validating a plan against the register before acting — remains unexercised. | declared by `bn-2c0a` and `bn-2phq3` |
| N4 | **Long-run schema-churn cost.** KILL-08's kill 2 is unmeasured beyond the 3.0 → 3.6 window, over which churn cost was *negative*. A major-version migration's cost is what `bn-3861i`'s 4.0 set will exercise. | `bn-3861i`, open |
| N5 | **Daemon-scale linearizability.** `Daemon::dispatch` takes `&mut self`; there is no concurrent daemon to linearize. Arrives with one, per docs/35. | DX-13's deferral, split and stated |
| N6 | **OS-grain crash safety.** The store is in-memory; a crash between a store write and its `fsync` is below `bn-3dr`'s model. | `bn-3dr` concern 2, ratified as scope |
| N7 | **An actual epoch advance.** No published artifact has survived one. | §8.3 |
| N8 | **CLI-side `ContinuationEpochMismatch` / `EpochUnsupported`.** Render-complete, not wire-exercised — needs a second, differently-epoched daemon. Wire-exercised at the daemon layer. | `bn-3tz60`, recorded |
| N9 | **A live daemon transport.** `continuumd` ships no socket or process transport; `continuum-cli`'s binary answers `NoTransportConfigured` at exit 2. Every surface comparison in §3 is in-process. | `bn-3tz60`, recorded; plan §10's "local IPC or authenticated HTTP/QUIC" half has no carrier bone |

### 9.3 Unsupported semantics — the system refuses rather than answers

| # | What | Shape of the refusal |
|---|---|---|
| U1 | **45 of the registry's 75 operations answer `UnsupportedSemanticFeature`**, pending their producing subsystems. Registration ahead of subsystem is the registry's own discipline (`rule errors.unsupported_surface`), not a protocol gap. | typed refusal |
| U2 | **Seven corpus operations have no worker in this process** (`program.run`, `program.replay`, `proof.attempt`, `proof.check`, `forge.step`, `benchmark.run`, `debug.export`). Their refusal is **an accounting of absence, not a defence.** | codec refusal by operation name, before the payload is read |
| U3 | **Five of RFC 0038's seven evidence-graph queries cannot be served.** Four are content-blocked (missing obligations; proof frontier — `CHECKED_BY` deliberately does not carry the discharge judgement; repair frontier — `REPAIRS`/`INVALIDATES` are appended by nothing; semantic duplicates — structurally impossible for a syntactic identity model). One is blocked on absent edge kinds: of thirteen edge kinds, exactly four are ever appended, so graph-driven task generation would answer *every* candidate or *none*, both of which are the empty-success shape the rules forbid. | `PHASE-A-DEL-04` left **unannotated**; carrier `bn-m6qs9` open |
| U4 | **Eight of `continuum-semantic-diff`'s fifteen protected fields have no production classifier.** An attack through one is caught by RFC 0031's fail-closed rule as `unknown`/`review`, not as the `expanded`/`block` an affirmative classifier would give. A fail-open mutant turns that attack into `allow`, which pins the fail-closed rule as load-bearing until they land. | DX-02 row, recorded boundary 2 |
| U5 | **Three of DX-14's four engines do not exist**, so the cancellation lanes are lifecycle *profiles* rather than engines. A real DPOR/solver/proof/synthesis engine is held to the same table rather than to a new one. | DX-14 scope statement |
| U6 | **RFC 0028 stages 3, 6 and 8 are not configured at all** — a different fact from stages 5 and 7, which are *configured and refuse*. The pack claims exactly `CausallyClosed`; the requested `ReplayPreserving` is carried as rule C1's `unknown`/`unsupported` omission rather than echoed. | DX-01 row |

### 9.4 Stated absences carried forward verbatim in substance

| # | From | Absence |
|---|---|---|
| A1 | `bn-ymw` | The G2 evidence clears the **narrower Phase A exit condition**, NOT plan §24.5's full ratified promotion gate, which additionally binds a run to a build and dependency epoch, requires refusal *and* an append-only audit record per case, and re-locks on any later success. |
| A2 | `bn-ymw` | `OutputPolicy` is dormant on the corpus wire — every frame sets `output_policy: Absent`, so `Ceiling::UNBOUNDED` admits everything and no disposition changed. |
| A3 | `bn-id4` | No Dining Philosophers Intent Contract fixture; both ports run under `die-hard-contract.json`. Tripwired. |
| A4 | `bn-id4` | No certificate for Dining Philosophers; and in the §3 run **no `Certificate`-class artifact is published for either fixture** — the store's own attribution names exactly two classes. Tripwired. |
| A5 | `bn-id4` | No crashpack producer anywhere, so neither the depth-6 solution nor the depth-10 deadlock is an artifact to compare. Tripwired on the CLI golden's `"crashpack":null`. |
| A6 | `bn-id4` | No transition count or witness depth on the wire (RFC 0026 F16); `Cost`'s nine dimensions include neither. Tripwired. |
| A7 | `bn-id4` | `verification.result` carries no reachable-state count and no artifact refs — which is why the exit script is four operations, not three. |
| A8 | `bn-3dr` | Ten of thirteen `VolatileFact`s are **declared lost, never reconstructed**, including the task and continuation tables. A restart resolves no `Running` task to a continuation or to `Failed`, because there is no record to resolve. **Partly repaired by bn-1z09m:** the startup pass now resolves every task that has a durable `task_` record to `Settled` or to a typed `Failed`. The continuation branch waits on a durable continuation record (bn-20142). |
| A9 | `bn-3dr` | `VolatileFact::ALL` is a **hand audit** of `DaemonState`'s fields. A field added without a variant would be a fact a restart loses in silence; the correspondence is documented and behaviourally spot-checked, **not mechanically enforced.** |
| A10 | `bn-3dr` | `advance` can leave the store holding a record the task never committed. That is the recoverable direction, but a long-running daemon accumulates residue only `collect_garbage` reclaims. |
| A11 | `bn-3tz60` | `context expand` has **zero live-success evidence** at that bone's landing — every call returned a real `UnsupportedSemanticFeature`. (`context.compile` has since landed on `bn-1y4qc`; the CLI's live round trip is not re-verified by this package.) |
| A12 | RFC 0027 corr. 31 | `Omission.recoverable_by` is populated on **0 of 324** omission records on the instrument's wire — INV-007's retrieval half is declared and never once exercised. `verification.result` names no artifact on any answer. |
| A13 | RFC 0027 corr. 31 | The `canonical_cbor` question (~816 B per solved task, no version bump) is recorded **open by design** and routed to whoever owns the metric's meaning. Commissioning it after the numbers were known is the move correction 31 refused once already. |
| A14 | `bn-2phq3` | The 61% invalid-action pass depends on the ratified F3 reading of "attempt". Under the alternative reading the margin is **0%**. Adjudicated on pre-registration grounds; both numbers retained in the artifact. |
| A15 | `bn-3ety` | `bn-762i`'s closing evidence trail cites `bn-31cq1`, which **does not resolve in the Bones store**. The edits it stands for landed under `bn-zsi1v` and `bn-199nx`. A citation defect, recorded so a later reader does not hunt for it. |
| A16 | this package | **`PHASE-A-DEL-04` — "evidence graph" — carries no delivered annotation.** It is a Phase A *deliverable*, not an exit clause; the exit sentence does not name it and `bn-1grk`'s checklist does not name it. Its successor carrier `bn-m6qs9` is open. |
| A17 | this package | **Sixteen per-criterion gate acceptance bones are open** and depend on `bn-2ofd`. Their brief — "independently rerun this criterion against the integrated phase artifact" — is not discharged by this package, which re-ran the *delivering* suites, not an independent re-derivation. |

### 9.5 Inconclusive — an instrument ran and did not decide

| # | What | Disposition |
|---|---|---|
| I1 | **The byte-accounting question** (whose interface the metric counts) was `bn-2c0a`'s one undecided attack: −152% agent-interface, +53% fully symmetric, both defensible from the governing text *as written*. | **No longer inconclusive** — decided afterwards by RFC 0027 correction 31 on the metric's own stated reason, with the rejected reading recorded at full strength. Listed here because a package that dropped it would hide that the sign of the headline number turned on a reading. |
| I2 | **Recovery parity in the DX-10 instrument** is conditional on a projection that never drifts and is never stale. The instrument declares both rates to be zero and **cannot measure either**. One stale rendering costs the baseline 4 of 24 tasks. | Inconclusive, declared by the instrument itself. |

---

## 10. Recommendation

**Recommendation: EXIT PHASE A — but not today, and not on this package
alone. Close the sixteen gate-criterion acceptance bones first, then take
the decision, carrying six named items into Phase B as explicit debt.**

The reasoning is set out so it can be disagreed with.

### 10.1 Why the exit sentence itself is discharged

All four clauses hold, and the one that failed as measured was carried by
a disjunct its own sentence offers, on a decision the party that owns it
took and recorded. That is the correct shape: the plan wrote a disjunction
precisely because it anticipated the typed surface might lose, and the
consequence it named — rework the ACI, redesign the protocol before freeze
— was executed to the constitutional boundary the versioning rules draw
and the remainder was ratified as a standing plan with per-item verdicts.
Nothing about that outcome is being laundered: the matrix says `failed`,
plan §0.3 says `failed`, RFC 0027 says `fails as measured`, and this
package says it three times.

The strongest single piece of evidence in Phase A is not that things
passed. It is that **DX-03, DX-13 and DX-14 were falsified against frozen
production code, and their reproductions are now the regression guards.**
Evidence produced with no opportunity to shape the implementation around
it is worth more than evidence that never had the chance to fail.

### 10.2 Why not today

The graph itself says the work is not finished. Sixteen bones —
`bn-31yxg`, `bn-1k1s8`, `bn-3j01v`, `bn-3huh7`, `bn-16v3x`, `bn-1tkrp`,
`bn-2vbqm`, `bn-cxd2y`, `bn-q80m7`, `bn-2wypi`, `bn-1slz1`, `bn-24ulk`,
`bn-3r10g`, `bn-1iljt`, `bn-39uby`, `bn-ugee0` — are children of `bn-1grk`,
depend on `bn-2ofd`, and are open. Their brief is to **independently rerun
each criterion against the integrated phase artifact**, retaining raw
evidence and negative controls, with the standing warning that "a
feature-complete demo is not a pass when this criterion is unsupported,
stale, or inconclusive".

This package did not do that. It re-ran the *delivering* suites from clean
state and read their results. That is a different and weaker thing than an
independent re-derivation, and saying otherwise would be exactly the
self-attestation the phase-exit rules exist to prevent.

Two of those sixteen will land on something real rather than a formality:

- **G2-05** (`bn-1iljt`) must decide what to do about a criterion whose
  second half — "improve agent benchmark effectiveness" — has **no
  instrument at all**. That is a genuine hole, not a scope note. Either an
  ablation is built, or the criterion is explicitly narrowed by the party
  that owns it. It should not close by being read as though its first half
  were the whole of it.
- **G2-01** (`bn-2wypi`) must decide whether "generated clients … ship"
  is satisfied by a hand-written client and a hand transcription held to
  the IDL by two independent conformance checkers. There is a defensible
  yes here — the property the criterion is *for* is that no client is
  written against prose — but it is a reading, and a reading is a decision.

`bn-1grk`'s dependency list contains only the nine delivering bones and
**not** these sixteen children, so the goal reads dependency-ready while
sixteen of its own criteria acceptances are outstanding. That is worth
correcting in the graph regardless of what is decided.

### 10.3 What should be carried into Phase B as named debt

Not blockers. Items that must be *named in the exit decision* so that
nobody later reads Phase A as having settled them.

1. **The evidence graph is a Phase A deliverable that did not land**
   (`PHASE-A-DEL-04`, carrier `bn-m6qs9`). Five of seven RFC 0038 queries
   are unservable, with a written reason each. The exit sentence does not
   name it — which is why this is debt and not a blocker — but Phase A's
   own "Deliver:" list does.
2. **No continuation survives a daemon restart** (§6.4). Clause 4's
   guarantee is scoped to one process lifetime. A disk-backed daemon state
   is not dossier-mandated yet; when it is, `resume`'s predicate has to be
   re-proved across the restart boundary.
3. **The G2 Context Pack ablation is unbuilt** (N1). Its Phase A home was
   G2-05 and it has no owner past `bn-1iljt`.
4. **The prompt-injection corpus clears the narrow condition, not the
   §24.5 promotion gate** (A1), and seven of its operations are answered
   by an absence rather than a defence (U2). Every lane that lands one of
   those workers inherits its escape attempt.
5. **87.8% of the DX-10 redesign set sits behind protocol 4.0**
   (`bn-3861i`, open), and the post-4.0 re-measure is the fallible-families
   baseline. The bytes margin will still not pass — that is proven — so
   the re-measure's purpose is waste accounting, not row-flipping, and it
   should be commissioned as such.
6. **Eight of fifteen semantic-diff fields have no affirmative
   classifier** (U4). Until they land, RFC 0031's fail-closed rule is the
   only thing standing between a `no-expansion` attack and an `allow`.

### 10.4 What would change this recommendation

Stated so the recommendation is falsifiable rather than merely offered.

- If the deciding human reads G2-05's "improve agent benchmark
  effectiveness" as a **conjunct of the gate rather than a Phase-B
  aspiration**, then Phase A should **not** exit until the ablation is
  built. The plan's own §22 G0 text names "the G2 Context Pack ablation"
  as a thing that exists and must pass the §19.4 separation check, which
  reads more like an expectation than an aspiration. I do not think this
  blocks the *exit sentence*, which does not mention it — but it plainly
  bears on the *gate*, and `bn-1grk` closes gates.
- If the sixteen acceptance re-runs surface any criterion as
  **unsupported, stale, or inconclusive** on independent re-derivation,
  that criterion blocks, and this recommendation is void for it.
- If the evidence graph is judged load-bearing for Phase B's opening PR
  rather than deferrable, `PHASE-A-DEL-04` becomes a blocker rather than
  debt.

### 10.5 What I am not recommending

I am not recommending "exit now because the exit sentence is discharged".
The exit sentence is four clauses; the exit *goal* is sixteen criteria and
a deliverable list, and two of the criteria have a named half that nothing
in this tree measures. Reporting the sentence as the whole of the gate
would be the same move as reporting clause 2 as "satisfied" — technically
arrangeable, and false.

---

## 11. What this document does NOT do

- **It does not close the phase exit.** `bn-1grk` is `goal:manual` and is
  left open. Per `notes/START_HERE_IMPLEMENTATION.md` line 75, the
  continue/narrow/defer/kill decision and every `goal:manual` phase exit
  are privileged human actions. This document prepares the package; it
  does not take the decision, and §10 is a recommendation offered for
  disagreement, not a finding.
- **It does not close `bn-2ofd`.** The lead closes it.
- **It does not annotate `PHASE-A-EXIT` as delivered**, and it makes no
  edit to `plan.md`, to `G0_SPIKE_MATRIX.md`, to
  `START_HERE_IMPLEMENTATION.md`, or to any requirement registry. It adds
  no plan requirement, so no traceability regeneration is owed:
  [`KILL08_CHECKPOINT.md`](KILL08_CHECKPOINT.md) needed one because it
  annotated a §24 bullet, and this document annotates nothing. No file
  under `notes/plan/tools/` reads the `notes/` directory as an inventory,
  so a new sibling note needs no wiring.
- **It does not produce a new measurement of any clause.** Every figure
  here is either read off a retained artifact or a committed test named in
  §§3–7, or is the result of re-running those tests from clean state, and
  §2.2 says which. The content addresses in §3.2 are the one thing
  original to this run, and they are reproducible by the command in §3.1.
- **It does not re-adjudicate** the byte-accounting question, F3's reading
  of "attempt", or C6. Each was decided by the party that owned it — the
  first by RFC 0027 correction 31 under the user's explicit delegation,
  the second by the lead on pre-registration grounds, the third by the
  user as permanently declined. This document records the rulings.
- **It does not grade the other kill criteria, risks, or gates.** KILL-08
  has its own checkpoint; the remaining eighteen §24 criteria are
  untouched. G3–G10 are not Phase A's.
- **It does not merge, push, or close any bone.**
