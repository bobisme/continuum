# Repair Transactions

<!-- regenerated: rfc-0032 -->

> **Status:** Non-normative guide, regenerated around the twelve-gate, phase-profile
> repair design (plan §25, specification debt SD-10).
>
> **Normative sources.** [RFC 0032](../rfcs/0032-repair-transaction-protocol.md) is the
> protocol: the transaction object, the nine-status derivation, the eight `repair.*`
> operations, the twelve gates, the closed gate-status set, the phase-staged gate
> profiles, the neighborhood strategies, cost governance, promotion, and the receipt.
> [`repair-transaction.schema.json`](../schemas/repair-transaction.schema.json) and
> [`promotion-receipt.schema.json`](../schemas/promotion-receipt.schema.json) decide
> artifact shape (INV-003: schemas decide, prose does not).
> [`continuumd-native-protocol.idl`](../schemas/continuumd-native-protocol.idl) decides
> wire shapes and error codes (RFC 0026).
>
> **This document defines nothing.** It is the narrative around those sources: what the
> design defends against, why each gate exists, how the parts read end to end, and where
> the sharp edges are. Where it disagrees with RFC 0032, the schemas, or the IDL, they
> govern and this document is corrected — never the reverse (plan §25: the plan is a map,
> not the spec). No field, status, error code, or gate predicate is fixed here. Every
> normative claim below is a restatement with a citation, and the citation is the only
> reason a sentence here should be believed.

## Purpose

Make autonomous repair safe, reproducible, and reviewable by treating a repair as an
evidence-bearing transaction rather than an edit followed by tests.

The adversary is not a careless agent; it is a competent one optimizing for a green
result ([docs/50](50_AGENT_EVALUATION_AND_REWARD_HACKING.md)). Every structural decision
here exists because "the tests pass now" is cheap to manufacture: an exact trace can be
special-cased, a property can be weakened, a monitor can be disabled, a proof can be
reused after the thing it proved changed. A transaction with named gates and a signed
receipt makes each of those moves either impossible or visible.

Three properties carry the design. RFC 0032 states them once so nothing downstream
re-derives them:

- **Promotion is recomputed, never reported.** `repair.promote` recomputes the policy
  verdict server-side from current evidence; no cached, client-supplied, or
  agent-asserted verdict is trusted (INV-015).
- **An absent gate is never a passed gate.** All twelve gates are listed at every status.
  A gate outside the active profile is `not_yet_enforced` — present and visibly
  unenforced — so a Phase B receipt is structurally distinguishable from a Phase D
  receipt (plan §21, INV-007).
- **A hypothesis is not evidence.** The transaction carries the agent's hypothesis beside
  the record, never inside it. An agent can be wrong without corrupting the record, and
  prose never decides a gate (INV-003, INV-016). The same rule governs plan §8.4's
  counterfactual repairs: a counterfactual prioritizes repair surfaces and produces
  explicit hypotheses, and supports no gate.

## The artifact is the schema, not a struct

An earlier revision of this document carried a `RepairTransaction` Rust struct as if it
were the artifact. It was not, and it is gone: RFC 0032 correction 14 records that its
`proposed_changes`, its flat `evidence: Vec<EvidenceHandle>`, and its typed
`evaluation_policy: RepairPolicy` did not match the schema's `changes`, its **per-gate**
`evidence`, and its `evaluation_policy` string. The field map is RFC 0032's table under
"The transaction object"; the shape is
[`repair-transaction.schema.json`](../schemas/repair-transaction.schema.json).

What a reader should carry away, all of it normative elsewhere:

- the **base triple** — base snapshot, base intent, failure — is sealed at
  `repair.begin` and frozen for the lineage. A repair against a different base is a
  different transaction, not a new version of this one;
- `gate_profile` is declared at `begin` and frozen too (correction 4), so a campaign can
  never pick the profile its results happen to satisfy;
- `candidate_snapshot` is null exactly at `draft` and sealed at every later status;
- `gates` is always **twelve entries by identity**, never a count and never a subset;
- `cost_ledger` is cumulative across the lineage and non-decreasing;
- `hypothesis` is a required string of untrusted prose;
- `status` is derived (below), and `version`/`supersedes` record the lineage — `version`
  is transaction lineage, not schema identity (plan §25 SD-08).

`repair.begin` takes a crashpack and a gate profile. It does **not** take a base snapshot
or an intent: the daemon resolves both from the crashpack and from that snapshot's intent
binding (correction 2, the IDL governing).

## Status is derived, never asserted

```text
draft → applied → evaluating → ready | blocked | inconclusive
                                     → promoted | rejected | superseded
```

Each operation commits a new immutable version; earlier versions stay addressable
(plan §8.1). The transaction manager computes `status` from the gates, the policy
verdict, and the lineage. A client never sets it and a daemon never accepts a
client-supplied value — a writable status is a promotion path that bypasses every gate.
RFC 0032's derivation is total and ordered; in narrative form:

1. a recorded terminal outcome (`promoted`, `rejected`) wins;
2. a version that is not the lineage head is `superseded`;
3. no candidate snapshot yet ⇒ `draft`;
4. a running or budget-suspended campaign ⇒ `evaluating`;
5. nothing in the active profile evaluated yet ⇒ `applied`;
6. any in-profile gate `failed`, or a verdict that is not `allow` ⇒ `blocked`;
7. any in-profile gate `inconclusive` ⇒ `inconclusive`;
8. otherwise ⇒ `ready`.

Two consequences are worth stating for reviewers. **Failure dominates
inconclusiveness**: rule 6 precedes rule 7, so a transaction with one failed and one
undecided gate reads `blocked`, and the reviewer sees the refutation first. And **gate 12
is the promotion step, not a precondition of it**: `receipt_generation` is `pending` on a
`ready` transaction, because the receipt does not exist until `promote` composes it, so
rule 8 is evaluated over gates 1–11 of the active profile.

## The twelve gates

Named by their schema identities. The claim column is what a passing gate asserts, the
evidence column is what it must reference, and the last column is the attack it closes.

| # | Gate id | Claim | Passing evidence | Failure mode closed |
|---|---|---|---|---|
| 1 | `base_replay` | the original failure replays on the base snapshot | a replay run reproducing the crashpack's failure identity | evaluating against a base that never failed |
| 2 | `patch_application` | the patch applies cleanly to the declared base | the sealed candidate digest equals the normalized base + changes digest | edits smuggled outside the declared change set |
| 3 | `intent_integrity` | the classification finds no protected change | a `diff_*` artifact whose recomputed decision is `allow` | intent gaming, abstraction gaming, opaque escape |
| 4 | `exact_regression` | the exact failure no longer occurs | a replay run on the candidate under the original choices | — |
| 5 | `neighborhood` | neighboring schedules, faults, and values are explored | per-strategy coverage plus every failing neighbor found | exact overfit |
| 6 | `property_mutation` | property mutations still fail where expected | mutant-by-mutant detection results, including non-vacuity | verifier gaming, availability collapse |
| 7 | `defect_mutants` | known defect mutants remain detected | the mutant-corpus result set | instrumentation gaming |
| 8 | `refinement_coverage` | refinement coverage is not reduced | the computed coverage delta against the base | silently narrowing what is checked |
| 9 | `certificate_rebuild` | invalidated certificates and proofs are rebuilt | rebuilt receipts with fresh checker identity and epochs | stale proof |
| 10 | `incremental_parity` | clean and incremental results agree | an Incremental Parity Audit promotion-lane comparison | exploiting an invalidation bug |
| 11 | `code_and_security` | tests, static verification, and security gates pass | the run results and the security review | — |
| 12 | `receipt_generation` | the receipt is composed and independently verified | the receipt and its verification result | forged or agent-signed receipts |

Reading them correctly — each of these is a place where a plausible-sounding shortcut was
already rejected:

- **Gate 1 separates non-reproduction from engine divergence.** The gate is `failed` when
  the failure does not reproduce on the base snapshot. A `ReplayDiverged` condition is
  **`inconclusive`**, and it must additionally publish a `defect_*` artifact (plan §4.7).
  An earlier revision of this document said "if replay diverges, block with
  `ReplayDiverged`", collapsing the two; RFC 0032 correction 6 governs. An engine defect
  is a statement about the verifier, and rendering it as a gate failure blames the repair
  for it (INV-008).
- **Gate 2 compares digests, not diffs.** The candidate snapshot is sealed and
  normalized, and the gate compares content identities. Any difference is a hidden edit,
  whatever its size.
- **Gate 3 passes on a recomputed verdict, not on the phrase "protected intent is
  unchanged".** It passes only when the [RFC 0031](../rfcs/0031-semantic-and-intent-diff.md)
  classification is complete over all fifteen protected fields and the recomputed
  decision is `allow`; `review` and `block` do not pass. One `unknown`, `unsupported`, or
  `incomparable` relation on a protected field forbids `allow` under the fail-closed
  rule, so the gate blocks without the diff ever guessing a direction (correction 8).
  That is the difference between a gate and a slogan: the earlier phrasing admitted a
  reading in which an unclassified change is not a change.
- **Gate 4 handles eliminated events.** Where the patch removes an event the original
  replay depended on, the gate uses the §16 correspondence to report where replay
  diverges and continues under the transaction's declared evaluation policy. It does not
  pass by declaring the divergence uninteresting.
- **Gate 6 covers non-vacuity.** A property that has become vacuously true is a mutation
  the gate must detect; availability collapse — safety "restored" by suppressing progress
  — surfaces here as a liveness or non-vacuity mutant that no longer fails (INV-012).
- **Gate 8 is a computed delta, not a judgement.** Plan §8.2's "not reduced
  unexpectedly" is not a predicate any daemon can evaluate. The gate passes when coverage
  does not decrease, or when the decrease is declared and permitted by the evaluation
  policy with the delta disclosed in the receipt; an undeclared decrease is `failed` and
  an uncomputable delta is `inconclusive` (correction 7).
- **Gate 9 is enforced against a freshness predicate** from
  [RFC 0030](../rfcs/0030-incremental-semantic-query-engine.md): a certificate or proof is
  fresh only if its checker identity and every epoch it pins match the daemon's current
  values for the epochs the consuming query declares.
- **Gate 10 uses the promotion lane, not the sampled one.** Sampling governs the
  interactive lane; a sampled comparison is not gate-10 evidence.
- **Gate 12 is verified, not asserted.** The receipt is composed and then re-verified by
  reference: `evidence.verify` re-fetches every referenced artifact and checks it,
  accepting no client-declared status. A receipt that cannot be re-verified leaves gate 12
  `failed` and the promotion aborts with nothing published.

## Gate statuses: five, and none of them means "skipped"

| Status | Meaning | May support promotion |
|---|---|---|
| `passed` | the claim holds on the evidence it references | yes |
| `failed` | the claim is refuted | no |
| `pending` | in the active profile, not yet evaluated | no |
| `inconclusive` | evaluated; the evidence cannot decide, with a typed INV-008 reason | no |
| `not_yet_enforced` | outside the active profile (plan §21) | only under a profile that excludes it |

- `inconclusive` is neither `failed` nor `pending`. Timeout, unsupported semantics,
  insufficient telemetry, abstraction ambiguity, and incomplete proof search are distinct
  reasons and the gate carries the one it hit (INV-008).
- **Budget exhaustion is never a gate outcome.** A campaign that runs out of budget leaves
  its gate `pending` and returns `BudgetExhausted` with a continuation. Recording it as
  `inconclusive` would convert a resumable campaign into a decided one; recording it as
  `passed` would credit the smaller campaign that actually ran.
- **There is no `not_applicable`.** A gate that seems inapplicable is still evaluated and
  still reports one of the five (plan §25 SD-11). To a reviewer, "not applicable" is
  indistinguishable from "not checked".
- **A gate cannot pass on absent evidence.** Only `not_yet_enforced` and `pending` gates
  carry an empty evidence list; `passed` with no evidence is prohibited for gates 1–11.
- **Unrecognized tokens fail closed.** A consumer that reads a gate name, status, or
  profile it does not recognize rejects the artifact; it never treats an unknown status as
  `passed` or an unrecognized gate name as an absent gate.

## Phase profiles

Promotion profiles are phase-staged: each gate enters the default profile when the
subsystem that produces its evidence ships (plan §21). A profile is not a dial for
convenience — it is the disclosure that some gates cannot run yet.

| Profile | Enforced gates | May be `not_yet_enforced` |
|---|---|---|
| `phase-b` | 1–8, 11, 12 | `certificate_rebuild` (9), `incremental_parity` (10) |
| `phase-c` | 1–8, 10, 11, 12 | `certificate_rebuild` (9) |
| `phase-d` | all twelve | none |
| `default` | all twelve | none |

Phase C adds gate 10 because the Incremental Parity Audit ships with the incremental
semantic database; Phase D adds gate 9 because certificate and proof rebuild ships with
the proof service. A gate outside the profile is listed `not_yet_enforced`, never omitted
and never `passed`; a gate **inside** the profile may not be listed `not_yet_enforced`.
The schema enforces exactly this, per profile, at every status including `promoted` — so
a promoted `phase-b` transaction carries ten `passed` gates and two `not_yet_enforced`,
and a promoted `phase-d` transaction carries twelve `passed`. The four rows above restate
the schema's conditionals; the schema decides.

Plan §21 and the implementation notes spell the status `NotYetEnforced` in prose; the
value on the wire and in artifacts is `not_yet_enforced` (correction 5).

## Change sets

A transaction's `changes` entries carry a digest and a kind from a closed six-member set:
`rust`, `model` (CML), `proof`, `correspondence`, `domain` (domain-pack or config), and
`intent`.

The last is not an ordinary repair. A `changes` entry of kind `intent` submitted through
ordinary repair authority is refused with `IntentMutationDenied` at `repair.apply`; it is
admissible only on a transaction that has already been reclassified. Reclassification
requires `revise-intent` authority (absent from default agent capability profiles),
routes the delta through [RFC 0037](../rfcs/0037-intent-contract.md)'s revision procedure,
records the decision as audit-recorded evidence, and re-bases onto the accepted revision —
which is a **new** transaction, because the base triple is frozen. Weakening a property,
strengthening an assumption, shrinking a bound, hiding an event, removing a fault, or
downgrading assurance is a privileged intent revision, never a repair (INV-001, INV-011).

## Neighboring exploration (gate 5)

Overfitting is combated by constructing a semantic neighborhood around the failure. The
strategy set is closed and mirrors plan §8.3 one for one:

| Strategy token | What it varies |
|---|---|
| `alternate-enabled-events` | alternate enabled events at each causal decision |
| `fault-window` | fault insertion and removal around the repaired window |
| `cancellation-checkpoints` | cancellation at adjacent checkpoints |
| `value-name-permutation` | equivalent value and name permutations |
| `message-duplication-loss-delay` | changed message duplication, loss, and delay |
| `schedule-perturbation` | schedule perturbations inside the same trace-class boundary |
| `abstraction-map-variants` | generated variants from the abstraction map |
| `hidden-corpus-mutations` | hidden corpus-style mutations |

The neighborhood is property-directed and budgeted, and the disclosure rules are what
make it auditable:

- **coverage is disclosed per strategy** — which strategies ran, their counts, and the
  budget consumed. A single aggregate number ("38,412 classes explored") is a rendering of
  the record, not the record (correction 13);
- **silent caps are prohibited** — a strategy truncated by budget is disclosed as
  truncated with its committed frontier, and its gate stays `pending` with a continuation
  rather than passing;
- **every failing neighbor is disclosed** in the receipt, with its class and a handle to
  its run.

That last rule is not a convenience. The ratified neighborhood-adequacy threshold — FR-05,
the "Neighborhood adequacy (§8.3)" register row in plan §24.5, opened in
[research/33](../research/33-agent-benchmarks-and-reward-hacking.md) and quoted verbatim
in RFC 0032 — counts a mutation as caught **only** when property-directed exploration
surfaces a neighbor on which the gaming patch fails **and** the receipt discloses that
neighbor. The ratified sentence lives in exactly two places, research/33 and the plan
§24.5 register row, under a single marked-quote identity; it is deliberately not
reproduced here, and a reader who needs its numbers should read it there. Its consequence
for a daemon is blunt: finding a failing neighbor and omitting it from the receipt fails
gate 5. An undisclosed catch is not a catch, because a reviewer cannot audit it and it
cannot be told apart from a post-hoc claim.

No adequacy claim is available yet. The docs/50-classified gaming corpus the threshold is
measured on is a Phase B deliverable and does not exist today; docs/50 supplies the
attack-class taxonomy, not the corpus. Until it lands and is graded independently, this
design claims coverage **disclosure** and nothing more, the register's fallback — a fixed
strategy-list neighborhood with per-receipt coverage disclosure and no adequacy claim —
stands, and a receipt does not render coverage as adequacy.

## Mutation challenge (gates 6 and 7)

The mutation challenge is split by target: `property_mutation` mutates what is checked,
`defect_mutants` mutates what is checked *against*.

- a property mutant that no longer fails means the property has stopped doing work: the
  gate is `failed` and the specific mutant is named;
- a defect mutant that is no longer detected means instrumentation or a monitor has been
  disabled — the verifier-gaming vector — and the gate is `failed`;
- a mutant the campaign never ran is evidence of nothing and leaves the gate `pending`;
- mutants are drawn from a held-out corpus. A mutant whose identity leaked into the
  candidate snapshot is excluded and the exclusion disclosed; a hard-coded mutant or
  corpus name is itself a gaming signal.

## Evidence discipline and the agent swarm

Different agents attach diagnosis, candidate patches, invariant or proof repairs,
adversarial review, and benchmark results. They do **not** edit one mutable transaction
document — they append typed nodes, and the evidence graph's rules
([RFC 0038](../rfcs/0038-multi-agent-evidence-graph.md),
[docs/44](44_MULTI_AGENT_EVIDENCE_GRAPH.md)) are not relaxed for repairs:

- every gate evidence entry resolves to an evidence-graph node or a content-addressed
  artifact and is independently re-checkable through `evidence.verify`;
- there is an **evidence floor** per gate: gates 1, 2, 4, and 11 are satisfied at
  `observed`; gates 5–8 require `sampled` or `bounded`, and the receipt discloses which;
  gates 3, 9, 10, and 12 require `validated` or `proved`, because each rests on an
  independent checker (INV-004);
- an agent's `attach` creates nodes at `proposed` and never promotes one. Only trusted
  services promote status, and agent votes and confidence never move it (INV-015);
- repair never lowers a claim: evidence the repair invalidates is retired by `superseded`
  with an explicit edge to its successor, and a regressing write loses its
  compare-and-set;
- contradictory claims materialize a conflict node rather than resolving by write order,
  and no coordinator resolves a conflict by picking the most confident agent.

The gate-status set and the claim-status lattice share the token `inconclusive` and
nothing else. A gate status is a step's outcome; a claim status is a claim's disposition.
Neither is derived from the other.

## Cost, budget, and resumption

Every transaction carries a cumulative cost ledger across all its versions —
`cpu_ms`, `wall_ms`, `solver_ms`, `memory_bytes`, and `tokens` required, with `bytes`,
`candidates`, `proof_ms`, and `states` optional — and the receipt carries the same block.

Gate 5–7 campaigns are incremental by default: a neighborhood class or mutant whose
causal footprint a dependency query judges disjoint from the patch delta may reuse its
prior result. The class of that reuse edge is decided by RFC 0030 — a **conservative**
disjointness judgement licenses a `Conservative` edge, and only a disjointness carrying a
witness an independent checker accepts *for this input digest* licenses a `Validated`
one; an `Unknown` disjointness is dependent and re-runs; an `Experimental` edge never
supports promotion (correction 1, which also corrects plan §8.6's "reuse prior results as
`Validated` edges"). A reused result is reported with its reuse class and the function
identity it was served under, and is never rendered as a fresh campaign.

Two ceilings, two different failures: exceeding a **per-transaction** ceiling is task
budget spend and yields `BudgetExhausted` with a continuation, and `repair.resume`
continues the campaign monotonically — the post-resume frontier includes the pre-resume
one. Exceeding a **per-principal** capability ceiling yields `QuotaExhausted` and carries
no continuation, because the campaign is admissible and the principal is not
(correction 9).

## Promotion and the receipt

`repair.promote` is privileged and audit-recorded, runs only from `ready`, and executes as
one semantically atomic step: verify head and status; recompute the classification and the
policy decision server-side; re-check every gate's evidence for resolvability and
freshness; compose the receipt; verify it by reference; sign it with the daemon's
receipt-service key; publish content before index and advance the base lineage.

The receipt records the promoted transaction and intent identities, the before and after
snapshots, the diff, the gate profile and all twelve gates by identity, coverage
(per-strategy counts, failing neighbors, truncations, reuse classes), the cumulative cost
ledger, every unresolved unknown, the server-recomputed policy decision, the checker
identity, the epochs it is verifiable under, the signature, and any redactions. Only
`passed` and `not_yet_enforced` may appear on it.

Signing binds identity and authorship; it never substitutes for proof checking. Agents
never hold receipt-signing keys, and possession of a well-formed receipt handle authorizes
nothing (ADR-0037): a receipt whose signature does not verify, or that names an artifact
that does not resolve, supports no gate.

Rollback has nothing to unwind, by construction. A failure before publication leaves the
transaction `ready`, publishes nothing, and returns the typed code for the step that
failed; a publication abort publishes nothing and truncates nothing; `reject` is terminal
and never merges the candidate into the base lineage; and a promoted repair is superseded
by a later transaction rather than deleted, with published receipts remaining verifiable
under their pinned epochs indefinitely.

## Worked example: a Phase B repair that promotes

The fixture is
[`schemas/examples/repair-transaction.example.json`](../schemas/examples/repair-transaction.example.json)
— the ack-before-durable repair, hypothesis "publish reply only after storage sync". Its
head reads:

```json
{
  "repair_id": "rt_demo1",
  "base_snapshot": "ws_demo1",
  "base_intent": "in_ack_v1",
  "failure": "crash_demo1",
  "candidate_snapshot": "ws_demo2",
  "changes": [{ "kind": "rust", "digest": "sha256:patch" }],
  "gate_profile": "phase-b",
  "semantic_diff": "diff_demo1",
  "status": "ready",
  "receipt": null
}
```

Its twelve gates: `base_replay`, `patch_application`, `intent_integrity`,
`exact_regression`, `neighborhood`, `property_mutation`, `defect_mutants`,
`refinement_coverage`, and `code_and_security` all `passed`, each naming the evidence it
rests on; `certificate_rebuild` and `incremental_parity` `not_yet_enforced` with no
evidence, because they sit outside the Phase B profile — listed and visibly unenforced;
`receipt_generation` `pending`.

Read the derivation against that list. Nothing terminal is recorded, this version is the
head, the candidate snapshot is non-null, no campaign is running, gates have been
evaluated, and no in-profile gate is `failed` or `inconclusive` — so the transaction is
`ready`, and it is `ready` **with** gate 12 `pending`, because the receipt does not exist
until promotion composes it. A reader who expected twelve `passed` gates on a promotable
transaction has found the one gate that is a step rather than a precondition.

The fixture is a single-version snapshot of a lineage head; in a live campaign `begin`,
`apply`, and `evaluate` each committed a version, and `version`/`supersedes` record that
chain.

Promotion then recomputes the verdict from current evidence, composes
[the receipt](../schemas/examples/promotion-receipt.example.json), verifies it by
reference, signs it, and only then records the outcome: `receipt_generation` flips to
`passed`, `receipt` becomes `receipt_demo1`, `status` becomes `promoted`, and `ws_demo2`
is the receipt's `result_snapshot`. Gates 9 and 10 stay `not_yet_enforced` on the
receipt — that is exactly the structural difference between this Phase B receipt and a
Phase D one, and it is why the profile is named on the receipt itself.

## Worked example: a repair that does not promote

Same base triple, same failure, a different candidate: the patch special-cases the
original interleaving. Replay reproduces the failure on the base (gate 1 `passed`), the
patch applies cleanly (gate 2 `passed`), the classification finds no protected change
(gate 3 `passed`), and the exact failure is gone (gate 4 `passed`) — the shape of every
overfitted repair, and the reason gates 5–8 exist.

Neighboring exploration then perturbs the schedule inside the same trace-class boundary
and finds a sibling interleaving on which the candidate still fails. Gate 5 is `failed`,
that neighbor is disclosed with its class and run handle, and by derivation rule 6 the
transaction is `blocked` — not "ready with a caveat". Promotion from `blocked` is refused
with `PolicyGateFailed` and publishes nothing.

Two variants are worth contrasting, because they are the ones most often confused:

- had the neighborhood campaign instead run out of budget, gate 5 would be **`pending`**
  with a continuation and the transaction `evaluating`; `repair.resume` continues it
  monotonically. Exhaustion is not a verdict;
- had the replay engine diverged on the base snapshot, gate 1 would be **`inconclusive`**
  with a `defect_*` artifact published, and the transaction `inconclusive`. Promotion
  from there refuses with `InsufficientEvidence`, not `PolicyGateFailed`: a gate that
  could not decide is not a gate that failed (INV-008).

## Failure modes and the gate that closes each

| Failure mode | What the agent did | Gate that closes it |
|---|---|---|
| Exact overfit | original trace fixed, sibling trace fails | 5 `neighborhood` |
| Intent gaming | property, assumption, or bound changed | 3 `intent_integrity` |
| Verifier gaming | property monitor disabled or made vacuous | 6 `property_mutation` |
| Stale proof | receipt from an old snapshot reused | 9 `certificate_rebuild` |
| Availability collapse | safety "fixed" by suppressing progress | 6 `property_mutation` (non-vacuity) |
| Abstraction gaming | concrete bad states merged in the abstraction map | 3 `intent_integrity` |
| Opaque escape | affected effect marked unsupported or opaque | 3 `intent_integrity` (trust-boundary expansion) |

Instrumentation gaming — disabling the telemetry a monitor reads rather than the monitor
itself — is gate 7 `defect_mutants`. Each mode is also a corpus mutation: RFC 0032's
acceptance criteria require the mapping above to be demonstrated on the mutation corpus,
not asserted here.

## Minimality and review affordances

Continuum may compute patch subsets and semantic changes to identify unnecessary edits. A
minimal textual patch is not always the best one, and the review surface shows both the
textual and the semantic footprint. This is a review affordance, not a gate: nothing in
the promotion path is decided by patch size. Plan §8.5's reviewer panel is likewise a
projection — the record is per-strategy coverage and every failing neighbor, and the panel
renders it (correction 13).

## Known gaps this guide does not paper over

RFC 0032 raises flags against artifacts it does not own. Two of them have since been paid
by the schema sweep and are recorded here only so that older citations stay readable:
**F8** (coverage had no machine shape) and **F9** (reuse was invisible in the receipt).
`promotion-receipt.schema.json` now types `coverage` as a closed six-group object, with
per-strategy `explored`/`failing`/`truncated` counts over the eight §8.3 strategies, a
committed frontier required whenever a strategy is truncated, and the `failing_neighbors`
array the FR-05 counting rule is stated over — each entry naming the neighbor, its
strategy class, and a handle to its run. Reuse of a gate 5–7 result now carries its
reuse-edge class and the `function_id`/`function_version` it was served under, so a
`Conservative` reuse and a `Validated` one are told apart by a reader rather than by
trust.

The ones still open, and that most change how this document should be read:

- **gate evidence is structurally unconstrained** (F4) — the evidence floor above is
  prose, not schema, and `passed` with an empty evidence array is schema-valid;
- **`policy_verdict` is optional at every status** (F6), and the receipt's
  `policy_decision`, `coverage`, and `unknowns` are optional in the schema while RFC 0032
  requires all three (correction 11). The sweep deferred this half deliberately: the
  receipt-side and transaction-side requiredness are one change and should land together.

One more was observed while regenerating this document and reported rather than fixed
here: the shipped promotion-receipt example named two different diff identities in
`semantic_diff` and `intent_diff`, while correction 10 requires both fields to name the
same `diff_*` artifact. The schema sweep landed that fix — the example now names one
identity, and both fields' descriptions carry the rule. The agreement itself stays
unenforceable in the schema (draft 2020-12 cannot compare two sibling values), which is
the surviving half of F7.

## Superseded nine-step pipeline (historical)

Earlier revisions described evaluation as a nine-step pipeline. It is superseded by the
twelve gates, and is recorded here only so that existing citations of the form "docs/41
step *n*" remain readable:

| Old step | Now |
|---|---|
| 1. Base validation | gate 1 `base_replay`, with the failed/inconclusive split of correction 6 |
| 2. Apply and seal | gate 2 `patch_application` |
| 3. Semantic/intent diff | gate 3 `intent_integrity`, on the recomputed verdict (correction 8) |
| 4. Exact regression | gate 4 `exact_regression` |
| 5. Causal neighborhood | gate 5 `neighborhood` |
| 6. Mutation challenge | gates 6 `property_mutation` and 7 `defect_mutants` |
| 7. Proof/refinement rebuild | gates 8 `refinement_coverage` and 9 `certificate_rebuild` |
| 8. Nonfunctional checks | gate 11 `code_and_security` |
| 9. Policy decision | not a step: the verdict is recomputed at `promote`; gate 10 `incremental_parity` and gate 12 `receipt_generation` had no step at all |

The pipeline had no place for clean/incremental parity, no receipt-verification step, no
profile, and no way to say that a gate had not been run — which is why it was replaced
rather than renumbered.

## What this regeneration changed

Direction is uniform: RFC 0032 and the two schemas govern; this document was corrected.

- the `RepairTransaction` struct is removed as an artifact-shape claim (correction 14);
- the nine-step pipeline is replaced by the twelve named gates, with the phase-staged
  profiles and `not_yet_enforced` semantics that did not exist when the pipeline was
  written;
- "if replay diverges, block with `ReplayDiverged`" is replaced by the gate 1
  failed-versus-inconclusive split and its `defect_*` publication obligation
  (correction 6);
- "protected intent is unchanged" is replaced by gate 3's recomputed policy verdict over
  all fifteen protected fields (correction 8);
- the promotion-receipt contents are stated against the receipt schema and RFC 0032's
  composition table rather than as an independent list;
- status derivation, the evidence floor, the cost ceilings, and the neighborhood
  disclosure rules are stated as restatements with citations, so this document can be
  checked against its sources line by line.
