# RFC 0032: Repair Transaction Protocol

## Status
Draft for implementation.

**Target gate:** G4 (causal debugging and real repair)
**Owners:** repair/workbench leads
**Normative language:** MUST/MUST NOT/SHOULD/SHOULD NOT/MAY per RFC 2119.
**Normative schemas:** [`../schemas/repair-transaction.schema.json`](../schemas/repair-transaction.schema.json) for the transaction and [`../schemas/promotion-receipt.schema.json`](../schemas/promotion-receipt.schema.json) for the receipt. Where this document and a JSON schema disagree, the schema is corrected or this RFC is amended by an explicit revision; neither drifts silently (INV-003: schemas decide artifact shape).
**Normative wire vocabulary:** [`../schemas/continuumd-native-protocol.idl`](../schemas/continuumd-native-protocol.idl) (RFC 0026) — `RepairHandle` (`rt_`), `ReceiptHandle` (`receipt_`), `ContinuationHandle` (`cont_`), `GateProfile`, `GateName`, `GateStatus`, `GateOutcome`, `PolicyVerdictValue`, `PolicyDecision`, `StructuralVerdictValue`, `EvidenceStatus`, and the eight `repair.*` operations with their closed per-operation error sets. Where this document and the IDL disagree about a wire shape or an error code, the IDL decides and this RFC is corrected (RFC 0026, "IDL and versioning"); every such correction is recorded below.
**Companion normative sources:** [RFC 0031](0031-semantic-and-intent-diff.md) (the classification lattice, the impact set, and the policy verdict gate 3 is decided on), [RFC 0037](0037-intent-contract.md) (the protected field set, the closed policy verbs, and the revision procedure a reclassification routes through), [RFC 0030](0030-incremental-semantic-query-engine.md) (reuse-edge classes, dependency reasons, freshness, and the Incremental Parity Audit behind gate 10), [RFC 0038](0038-multi-agent-evidence-graph.md) with [`../docs/44_MULTI_AGENT_EVIDENCE_GRAPH.md`](../docs/44_MULTI_AGENT_EVIDENCE_GRAPH.md) (the append-only write model and the status authority every gate's evidence obeys), [RFC 0026](0026-continuumd-native-protocol.md) (protocol, epochs, error taxonomy), [RFC 0027](0027-agent-tool-protocol.md) (the authority ladder), [RFC 0005](0005-certificates-and-independent-kernel.md) and [RFC 0024](0024-proof-receipt-format.md) (certificate and receipt checking), [ADR-0035](../adr/0035-proof-receipts-and-axiom-manifests.md), [`../plan.md`](../plan.md) §8 (with §4.4–§4.7, §10.2/§10.3, §11.4/§11.7, §18.5/§18.6, §21, §22 G4), [`../docs/41_REPAIR_TRANSACTIONS.md`](../docs/41_REPAIR_TRANSACTIONS.md).
**Landed vocabulary:** `crates/continuum-evidence/src/claim_status.rs` implements the plan §11.4 claim-status lattice — `ClaimStatus`, `ClaimStatus::COVERS`, `StatusTransition`, `compare_and_set`, `verify_promotion_history`. Promotion and regression semantics are landed code, not a proposal; the evidence rules below are written against that order and MUST NOT be restated in a way that contradicts it. This RFC and that module MUST be revised together.
**Frontier dependency:** neighborhood adequacy (plan §24.5, lane [`research/33`](../research/33-agent-benchmarks-and-reward-hacking.md)) is a **ratified** register row whose corpus does not exist yet — the docs/50-classified gaming corpus is a Phase B G3 deliverable. Until it lands and is graded, this RFC makes **no adequacy claim** for the gate-5 neighborhood; the register fallback — a fixed strategy-list neighborhood with per-receipt coverage disclosure and no adequacy claim — is the standing configuration, and the ratified threshold binds when the corpus lands.

## Summary

A repair is a transaction: an immutable proposal plus accumulating evidence, promoted only when the policy gates close (plan §8, B8, INV-011). Every operation returns a new immutable transaction version; prior versions remain addressable (plan §8.1).

This RFC is the normative home of: the transaction object and its field types; the nine-status state machine and the derivation that computes it; the eight `repair.*` operations with their totality and idempotency obligations; the twelve gates, the closed gate-status set, and the phase-staged gate profiles; the neighborhood strategy set and its disclosure obligation; the mutation challenge; cost governance; the promotion procedure; and the promotion receipt's composition, signing, and independent verification. Plan §8, plan §21's profile paragraph, and docs/41 are informal restatements; where they disagree with this document, this document governs (plan §25: "Where plan prose and RFC disagree, the RFC is corrected and becomes normative"). Every such correction is recorded below under "Corrections recorded by this RFC". docs/41 was regenerated around this design (SD-10 paid, bn-7kj, 2026-07-31) as an explicitly non-normative guide; this RFC and the two schemas remain normative.

Three properties are load-bearing and are stated once here so nothing downstream re-derives them:

- **Promotion is recomputed, never reported.** `repair.promote` re-computes the policy verdict server-side from current evidence. No cached, client-supplied, or agent-asserted verdict is ever trusted (INV-015, B12).
- **An absent gate is never a passed gate.** All twelve gates are listed at every status. A gate outside the active profile is `not_yet_enforced` — present and visibly unenforced — so a Phase B receipt is structurally distinguishable from a Phase D receipt (plan §21, INV-007).
- **A hypothesis is not evidence.** The transaction carries the agent's hypothesis beside the record, never inside it. An agent can be wrong without corrupting the record (docs/41), and prose never decides a gate (INV-003, INV-016).

## Versioning and revision

- **Closed sets.** The twelve gate names, the five gate statuses, the four gate profiles, the nine transaction statuses, the six change kinds (`rust`, `model`, `proof`, `correspondence`, `domain`, `intent`), the three policy decisions (`allow`, `review`, `block`), and the eight neighborhood strategies are **closed**. Adding, removing, or renaming a member is a breaking change requiring an explicit revision of this RFC and a `schema_epoch` advance of the affected schema; for `GateName`, `GateStatus`, `GateProfile`, and `PolicyDecision`, which reach the wire, it is additionally an RFC 0026 protocol-major change under its N and N−1 window.
- **Fail closed on unrecognized tokens.** A consumer that reads a gate name, gate status, gate profile, transaction status, or change kind it does not recognize MUST reject the artifact. It MUST NOT treat an unknown gate status as `passed`, an unknown profile as `default`, or an unrecognized gate name as an absent gate. Forward compatibility is achieved by rejecting, never by ignoring.
- **`not_applicable` does not exist.** The gate-status set has exactly five members. A gate that seems inapplicable to a change is still evaluated and still reports one of the five; "not applicable" is the shape of silent scope-hiding this design exists to refuse (plan §25 SD-11).
- **`version` is transaction lineage, not schema identity.** `repair-transaction.schema.json`'s `version` field is the monotonic version within one transaction's lineage, and this RFC is its documentation. It is not the retired `version`/`schema_version`/`format` convention: identity and encoding versioning are carried by `schema_id` and `schema_epoch` under the one convention of [`../schemas/README.md`](../schemas/README.md) (plan §25 SD-08), which this schema follows like every other. The dossier validator carries the exemption explicitly so that the two cannot be conflated.
- **Version arithmetic.** `version` MUST be `1` exactly when `supersedes` is absent, and MUST be the superseded version's `version` plus one otherwise. A lineage has exactly one head; a write against a non-head version is a lost compare-and-set (see "Concurrency and lineage").
- **Nothing is rewritten.** A committed transaction version and a published receipt are immutable artifacts under plan §4.4. Re-evaluation produces a new version linked by `supersedes`; re-derivation after an epoch advance produces a new identity linked by a `SUPERSEDES` edge (plan §4.6, INV-009). No operation edits an earlier version to reflect a better answer.
- **Epoch advances do not migrate transactions.** An open transaction does not survive an advance of an epoch its evaluation consumed: its continuations are forked, never migrated in place (plan §4.6, RFC 0030), and any gate result carried across an advance is demoted by that advance's per-class `Preserved | Revalidate | Incompatible` statement before it is reused. A published receipt remains verifiable under the epochs it pinned, indefinitely (INV-006, INV-014).

## The transaction object

The normative shape is `repair-transaction.schema.json`. The obligations this RFC adds are in the last column; a dash means the schema is sufficient.

| Field | Type | Required | Obligation added here |
|---|---|---|---|
| `schema_id`, `schema_epoch` | const string, const 1 | yes | the SD-08 instance header |
| `repair_id` | `^rt_` | yes | content identity of this version, excluded from its own preimage |
| `version` | integer ≥ 1 | yes | see "Version arithmetic" |
| `supersedes` | `^rt_` | no | present iff `version > 1`; names the immediate predecessor |
| `base_snapshot` | `^ws_` | yes | sealed at `begin`, fixed for the transaction's lifetime |
| `base_intent` | `^in_` | yes | resolved from the base snapshot's binding (RFC 0037), fixed for the lifetime |
| `failure` | `^crash_` | yes | the crashpack the transaction repairs |
| `hypothesis` | string | yes | untrusted prose; never evidence (see below) |
| `changes` | array of `{kind, digest}` | no | `kind` is the closed six-member set; `intent` is admissible only after reclassification |
| `candidate_snapshot` | `^ws_` or null | yes | null exactly at `draft`; sealed and non-null at every later status |
| `status` | nine-member enum | yes | derived, never asserted by a client (see "Status machine") |
| `gate_profile` | four-member enum | yes | declared at `begin`, fixed for the lifetime |
| `gates` | exactly 12 unique `{name, status, evidence}` | yes | identity, not count; every gate present at every status |
| `cost_ledger` | 5 required + 4 optional dimensions | yes | cumulative and non-decreasing across the lineage |
| `semantic_diff` | handle or null | no | MUST be a `diff_*` identity; non-null from `evaluating` onward |
| `policy_verdict` | `allow`/`review`/`block` | no | MUST be present at `ready`, `blocked`, `promoted`, `rejected`; recomputed at promotion |
| `receipt` | `^receipt_` or null | no | non-null exactly at `promoted` (schema-enforced by the promoted conditional) |
| `evaluation_policy` | string or null | no | names the policy the campaign ran under; see flag F3 |
| `performance_impact`, `security_review` | object or string | no | see flag F3 |
| `actor` | string | no | provenance only; authority is checked from the capability, never from this field (ADR-0037) |
| `redacted_references` | array of `Redacted` | no | the shared four-field stub (plan §4.5, SD-14) |

- **The base triple is frozen.** `base_snapshot`, `base_intent`, and `failure` MUST NOT change across a lineage. A repair against a different base is a different transaction.
- **The hypothesis is not evidence.** `hypothesis` MUST NOT contribute to any gate outcome, to the policy verdict, or to the receipt's decision, and MUST NOT be interpolated into any typed field or into error text (INV-003, INV-016). It is rendered to reviewers and parsed by nothing. The same rule governs plan §8.4's counterfactual repairs: a counterfactual result prioritizes repair surfaces and produces explicit hypotheses, and MUST NOT support any gate.
- **Cost is cumulative.** `cost_ledger` accumulates across every version of the lineage, and each dimension MUST be non-decreasing from a version to its successor. A decrease is a defect, not a correction.
- **Redaction downgrades, it does not hide.** A gate whose supporting artifact reads back as `Redacted(reason, commitment)` reports the redaction and downgrades per plan §18.4. It MUST NOT be reported as `passed` on the strength of a commitment alone.

## Status machine

The nine statuses are exactly `repair-transaction.schema.json`'s `status` enum and exactly docs/41's state machine:

```text
draft → applied → evaluating → ready | blocked | inconclusive
                                     → promoted | rejected | superseded
```

**Status is derived, not asserted.** The transaction manager computes `status` from the gates, the policy verdict, and the lineage (docs/41: "The transaction manager derives status"). A client MUST NOT set it and a daemon MUST NOT accept a client-supplied value. The derivation is total and is evaluated in this order:

1. a recorded terminal outcome (`promoted`, `rejected`) wins;
2. a version that is not the lineage head is `superseded`;
3. `candidate_snapshot` null ⇒ `draft`;
4. a running or budget-suspended gate campaign ⇒ `evaluating`;
5. no gate in the active profile has been evaluated ⇒ `applied`;
6. any in-profile gate `failed`, or `policy_verdict` is not `allow` ⇒ `blocked`;
7. any in-profile gate `inconclusive` ⇒ `inconclusive`;
8. otherwise ⇒ `ready`.

Failure dominates inconclusiveness: rule 6 precedes rule 7, so a transaction with one failed and one inconclusive gate is `blocked` and its reviewer sees the blocking fact first.

**Gate 12 is the promotion step, not a precondition of it.** `receipt_generation` is `pending` on a `ready` transaction — the receipt does not exist until `promote` composes it — so rule 8 is evaluated over gates 1–11 of the active profile. Promotion composes the receipt, verifies it, flips gate 12 to `passed`, and only then records `promoted`. The schema's promoted conditional (no `pending`, `failed`, or `inconclusive` gate) is consistent with that sequencing and is what makes it checkable after the fact.

## Operations

Eight operations (plan §10.2), with the authority levels of RFC 0027 and the closed error sets of the IDL. `repair.begin / apply / attach` are `propose`; `evaluate / resume` are `execute`; `review` is `read`; `promote / reject` are `promote` and are `@privileged @audit_recorded`.

| Operation | Effect | Permitted from | Operation-specific refusals |
|---|---|---|---|
| `begin` | opens `rt_*` v1 at `draft` from a crashpack and a declared `gate_profile` | — | `UnsupportedSemanticFeature`, `PolicyGateFailed` |
| `apply` | seals a candidate snapshot from base + changes; new version at `applied`; gate statuses reset to `pending` or `not_yet_enforced` | `draft`, `applied`, `ready`, `blocked`, `inconclusive` | `IntentMutationDenied`, `UnsupportedSemanticFeature` |
| `attach` | appends typed evidence nodes; new version, same status | every non-terminal status | `InsufficientEvidence`, `StatusConflict` |
| `evaluate` | runs the gate campaign for the active profile; new version carrying gate statuses | `applied`, `evaluating`, `ready`, `blocked`, `inconclusive` | `BudgetExhausted`, `InsufficientEvidence`, `PolicyGateFailed` |
| `resume` | continues a suspended campaign monotonically from `cont_*` | `evaluating` | `ContinuationEpochMismatch`, `BudgetExhausted`, `EpochUnsupported` |
| `review` | the reviewer projection (plan §8.5); records the review decision as evidence | any | `InsufficientEvidence` |
| `promote` | recomputes the verdict, composes and verifies the receipt, advances the lineage | `ready` | `PolicyGateFailed`, `InsufficientEvidence`, `StatusConflict`, `PublicationAborted`, `CertificateRejected` |
| `reject` | terminal refusal with a typed reason | every non-terminal status | `StatusConflict` |

`rule errors.common` adds `MalformedRequest`, `ProtocolVersionUnsupported`, `CapabilityDenied`, `QuotaExhausted`, and `EpochUnsupported` to every operation, and `IdempotencyKeyReused` plus `PublicationAborted` to every `@mutation`. No `repair.*` operation takes a `snapshot` field, so `StaleSnapshot` is **not** in any of their unions (correction 3).

`repair.review` resolves to `read` authority: the review decision it records enters the evidence graph as an audit-recorded append attributed to the named reviewer (RFC 0027), not through agent evidence-write authority.

### Totality

Every operation is **total** over the nine statuses: for each pair the outcome is defined as either a new committed version or a typed refusal that publishes nothing. There is no undefined pair, no silent no-op, and no partial write.

- From a terminal status (`promoted`, `rejected`) and from a non-head (`superseded`) version, every mutation MUST be refused and MUST publish no version. The typed code is `StatusConflict` wherever the operation's error union admits it — `attach`, `promote`, `reject` — and is a recorded protocol gap for `apply` and `evaluate` (flag F1).
- `evaluate` on a `draft` transaction refuses `InsufficientEvidence`: there is no candidate snapshot to evaluate.
- `promote` from any status other than `ready` refuses. `draft`, `applied`, `evaluating`, and `blocked` refuse `PolicyGateFailed`; `inconclusive` refuses `InsufficientEvidence`, because a gate that could not decide is not a gate that failed (INV-008).
- `review` is total by construction: it is `@readonly` and projects whatever exists, including an all-`pending` gate list on a `draft`.
- A refusal MUST NOT advance `version`, MUST NOT change `status`, and MUST NOT spend from `cost_ledger` beyond the admission check.

### Idempotency

Every `@mutation` carries an idempotency key scoped per actor (`rule idempotency.replay`, RFC 0026). Replaying a key with a byte-identical canonical request MUST return the same artifact or task identity; the same key with a different request MUST be rejected with `IdempotencyKeyReused`.

- `begin` replayed returns the original `rt_*` v1. Two distinct `begin` calls against one crashpack are two transactions, not one: deduplication is by key, never by the content of the failure.
- `apply` and `attach` replayed return the version the first call produced. They MUST NOT append a second version with identical content.
- `evaluate` and `resume` are `@task_starting`: a replay returns the same `task_*` and the same continuation, never a second campaign. Re-evaluating with *new* evidence is a new request with a new key and produces a new version — that is not a replay.
- `promote` is idempotent in the strong sense: promoting an already-`promoted` transaction returns the original `receipt_*` identity, and a second receipt for one transaction MUST NOT exist. This is what makes a retried promotion safe across a dropped connection (INV-002).
- `reject` replayed returns the original rejected version.

Idempotency is a protocol property, not a semantic one. It makes retries safe and MUST NOT be used to suppress a genuine second evaluation.

## The twelve gates

The twelve gates of plan §8.2, by schema id. The evidence column names what a passing gate MUST reference; the failure-mode column is the docs/41 attack the gate closes.

| # | Gate id | Claim | Passing evidence | Failure mode closed |
|---|---|---|---|---|
| 1 | `base_replay` | the original failure replays on the base snapshot | a replay run reproducing the crashpack's failure identity | evaluating against a base that never failed |
| 2 | `patch_application` | the patch applies cleanly to the declared base; no hidden edits | the sealed candidate snapshot's digest equals the normalized base+changes digest | smuggling edits outside the declared change set |
| 3 | `intent_integrity` | RFC 0031 classifies no protected change | a `diff_*` artifact whose recomputed `PolicyDecision` is `allow` | intent gaming; abstraction gaming (`merged`); opaque escape (`trust_boundaries` `expanded`) |
| 4 | `exact_regression` | the exact failure no longer occurs | a replay run on the candidate under the original choices | — |
| 5 | `neighborhood` | neighboring schedules, faults, and values are explored | per-strategy coverage plus every failing neighbor found | exact overfit |
| 6 | `property_mutation` | property mutations still fail where expected | mutant-by-mutant detection results, including non-vacuity | verifier gaming; availability collapse |
| 7 | `defect_mutants` | known defect mutants remain detected | the mutant-corpus result set | instrumentation gaming |
| 8 | `refinement_coverage` | refinement coverage is not reduced | the computed coverage delta against the base | silently narrowing what is checked |
| 9 | `certificate_rebuild` | invalidated certificates and proofs are rebuilt | rebuilt receipts with fresh checker identity and epochs | stale proof |
| 10 | `incremental_parity` | clean and incremental results agree | an Incremental Parity Audit promotion-lane comparison | exploiting an invalidation bug |
| 11 | `code_and_security` | tests, static verification, and security gates pass | the run results and the security review | — |
| 12 | `receipt_generation` | the promotion receipt is composed and independently verified | the receipt and its verification result | forged or agent-signed receipts |

Gate-specific normative rules:

- **Gate 1 distinguishes non-reproduction from engine divergence.** The gate is `failed` when the failure does not reproduce on the base snapshot. A `ReplayDiverged` condition is `inconclusive` and MUST additionally publish a `defect_*` artifact (plan §4.7, RFC 0026): an engine defect is not a statement about the patch, and rendering it as a gate failure blames the repair for the verifier.
- **Gate 2 compares digests, not diffs.** The candidate snapshot is sealed and normalized; the gate compares its content identity against the identity of base + declared `changes`. Any difference is a hidden edit and is `failed`, whatever its size.
- **Gate 3 passes on the verdict, not on a guess.** It passes iff the RFC 0031 classification is complete over all fifteen protected fields and the recomputed decision is `allow`. `review` and `block` do not pass. Under RFC 0031's fail-closed rule one `unknown`, `unsupported`, or `incomparable` relation on one protected field is sufficient to forbid `allow`, so the gate blocks without the diff ever guessing a direction.
- **Gate 4 handles eliminated events.** Where the patch removes an event the original replay depended on, the gate uses the §16 correspondence to report where replay diverges and continues under the transaction's declared `evaluation_policy` (docs/41 step 4). It MUST NOT pass by declaring the divergence uninteresting.
- **Gate 6 covers non-vacuity.** A property that has become vacuously true is a mutation the gate MUST detect; availability collapse — safety restored by suppressing progress — surfaces here as a liveness or non-vacuity mutant that no longer fails (INV-012).
- **Gate 8 is a computed delta.** The gate passes iff refinement coverage does not decrease, or the decrease is declared in the transaction and permitted by the evaluation policy with the delta disclosed in the receipt. An undeclared decrease is `failed`; an uncomputable delta is `inconclusive`.
- **Gate 9 is enforced against RFC 0030's freshness predicate.** A certificate or proof is fresh iff the checker identity and every epoch it pins equal the daemon's current values for the epochs the consuming query declares; a stale artifact cannot support an `Exact` or `Validated` reuse edge.
- **Gate 10 uses the promotion lane, not the sampled one.** Every promotion-relevant query is recomputed clean or independently validated at promotion time (RFC 0030). Sampling governs the interactive lane only, and a sampled comparison MUST NOT be presented as gate-10 evidence.
- **Gate 12 is verified, not asserted.** The receipt is composed and then re-verified by reference: `evidence.verify` re-fetches every referenced artifact and checks it, accepting no client-declared status. A receipt that cannot be re-verified leaves gate 12 `failed` and the promotion aborts with nothing published.

## Gate status semantics

`GateStatus` has exactly five members and the enum is closed:

| Status | Meaning | May support promotion |
|---|---|---|
| `passed` | the gate's claim holds on the evidence it references | yes |
| `failed` | the gate's claim is refuted | no |
| `pending` | in the active profile, not yet evaluated | no |
| `inconclusive` | evaluated; the evidence cannot decide, with a typed INV-008 reason | no |
| `not_yet_enforced` | outside the active profile (plan §21) | only under a profile that excludes it |

- **`inconclusive` is neither `failed` nor `pending`** (INV-008). Timeout, unsupported semantics, insufficient telemetry, abstraction ambiguity, and incomplete proof search are distinct reasons, and the gate MUST carry the one it hit.
- **Budget exhaustion is never a gate outcome.** A campaign that runs out of budget leaves its gate `pending` and returns `BudgetExhausted` with a continuation. It MUST NOT be recorded as `inconclusive`, which would convert a resumable campaign into a decided one, and MUST NOT be recorded as `passed` on the strength of the smaller campaign that ran (plan §11.4, INV-007).
- **Statuses do not move twice within one evaluation.** Within one campaign a gate moves from `pending` to exactly one of `passed`/`failed`/`inconclusive`; a revised answer is a new transaction version, not an edit.
- **A gate cannot pass on absent evidence.** `passed` with an empty `evidence` array is well-formed against the schema and is prohibited here for gates 1–11; only `not_yet_enforced` and `pending` gates carry no evidence (flag F4).

### Evidence discipline

Gate evidence lives in the evidence graph, and the graph's rules are not relaxed for repairs (RFC 0038, docs/44, plan §11.7).

- Every `evidence` entry MUST resolve to an evidence-graph node or a content-addressed artifact and MUST be independently re-checkable through `evidence.verify`.
- **The gate-status set and the claim-status lattice are different vocabularies.** They share the token `inconclusive` and nothing else: a gate status is a step's outcome, a claim status is a claim's disposition (`crates/continuum-evidence/src/claim_status.rs`). A daemon MUST NOT derive one from the other, exactly as `StructuralOutcome::unchanged` and the relation `unchanged` share a token and nothing else (RFC 0031).
- **Evidence floor.** A gate MUST NOT pass on evidence whose claim status is `proposed` or `inconclusive` — both sit below `observed` in the implemented lattice — and MUST NOT pass on evidence at `refuted` or `superseded`, which sit above `proved` in the chain without being stronger assurance. The admissible band is `observed` through `proved`. Gates 1, 2, 4, and 11 are satisfied at `observed`; gates 5, 6, 7, and 8 require `sampled` or `bounded`, and the receipt discloses which; gates 3, 9, 10, and 12 require `validated` or `proved`, because each rests on an independent checker (INV-004).
- **Only trusted services promote status.** `sampled` and `bounded` promotions name the producing engine's service identity; `validated` and `proved` are reachable only through an independent checker or the Lean proof service. An agent's `attach` creates nodes at `proposed`; it never promotes one (INV-015). Agent votes and confidence never change status.
- **Repair never lowers a claim.** Evidence the repair invalidates is retired by `superseded` with an explicit `SUPERSEDES` edge to its successor, or re-established under the new identity. Writing a lower status is `StatusTransition::Regression`, and the plan §11.7 compare-and-set rejects it; the loser receives `StatusConflict` and re-reads (RFC 0031, "Evidence-status interaction").
- **Swarm workers append.** Different agents attach diagnosis, candidate patches, invariant or proof repairs, adversarial review, and benchmark results as typed nodes. They do not edit one mutable transaction document (docs/41). Contradictory claims materialize a `conflict` node rather than resolving by write order, and a coordinator MUST NOT resolve a conflict by selecting the most confident agent answer (docs/44).

## Gate profiles

Profiles are phase-staged (plan §21): each gate enters the default profile when its producing subsystem ships.

| Profile | Enforced gates | May be `not_yet_enforced` |
|---|---|---|
| `phase-b` | 1–8, 11, 12 | `certificate_rebuild` (9), `incremental_parity` (10) |
| `phase-c` | 1–8, 10, 11, 12 | `certificate_rebuild` (9) |
| `phase-d` | all twelve | none |
| `default` | all twelve | none |

- The profile is declared at `begin` and is fixed for the transaction's lifetime. A transaction cannot be re-profiled; evaluating under a different profile is a new transaction.
- A gate outside the active profile MUST be listed with status `not_yet_enforced`. It MUST NOT be omitted and MUST NOT be reported as `passed`. This is the profile-side form of INV-007: phase staging discloses reduced scope rather than hiding it.
- A gate **inside** the active profile MUST NOT be listed `not_yet_enforced`. The schema enforces exactly this, per profile, with conditionals that apply at every status including `promoted`.
- The four rows above restate the schema's conditionals; the schema decides. A promoted transaction under `phase-d` or `default` therefore carries twelve `passed` gates, and one under `phase-b` carries ten `passed` and two `not_yet_enforced`.
- The receipt names its profile and lists all twelve gates by identity, not by count. Only `passed` and `not_yet_enforced` may appear on a receipt (`promotion-receipt.schema.json`).

## Neighborhood construction (gate 5)

The neighborhood is property-directed and budgeted, drawn from the eight strategies of plan §8.3. The set is closed and mirrors plan §8.3 one for one:

| Strategy token | plan §8.3 bullet |
|---|---|
| `alternate-enabled-events` | alternate enabled events at each causal decision |
| `fault-window` | fault insertion/removal around the repaired window |
| `cancellation-checkpoints` | cancellation at adjacent checkpoints |
| `value-name-permutation` | equivalent value/name permutations |
| `message-duplication-loss-delay` | changed message duplication/loss/delay |
| `schedule-perturbation` | schedule perturbations in the same trace-class boundary |
| `abstraction-map-variants` | generated variants from the abstraction map |
| `hidden-corpus-mutations` | hidden corpus-style mutations |

- **Coverage is disclosed per strategy.** The receipt records which strategies ran, their coverage counts, and the budget consumed. A single aggregate count is a rendering, not the record.
- **Silent caps are prohibited.** A strategy truncated by budget MUST be disclosed as truncated with its committed frontier; an undisclosed cap is an INV-007 violation, and a truncated campaign leaves gate 5 `pending` with a continuation rather than `passed`.
- **Failing neighbors are disclosed.** Every neighbor on which the candidate fails MUST appear in the receipt, with its class and a handle to its run. This is not a convenience: it is the ratified counting rule of the neighborhood-adequacy lane, quoted verbatim from [`research/33`](../research/33-agent-benchmarks-and-reward-hacking.md), which plan §24.5 quotes under `quote-id=neighborhood-adequacy-catch-rate`:

  > On the docs/50-classified gaming corpus — at least 20 hidden mutations for each of the five docs/50 attack classes (intent, instrumentation, evidence, overfitting, resource), so at least 100 mutations in total — the §8.3 neighborhood is adequate only if it catches at least 90 percent of the mutations overall with the one-sided 95 percent exact binomial lower bound on that rate above 80 percent and no single attack class below 75 percent, counting a mutation as caught only when property-directed neighboring exploration surfaces a neighbor on which the gaming patch fails and the receipt discloses that neighbor, with every mutation drawn from a family-level held-out split and graded by the independent grader of research/33.

  A catch counts only when property-directed neighboring exploration surfaces a failing neighbor **and** the receipt discloses that neighbor. An undisclosed catch is not a catch: a reviewer cannot audit it, and it cannot be distinguished from a post-hoc claim. A daemon that finds a failing neighbor and omits it from the receipt has failed gate 5, not passed it.
- **No adequacy claim until the corpus lands.** The gaming corpus is a Phase B G3 deliverable and does not exist today; docs/50 supplies the attack-class taxonomy, not the corpus. Until the corpus is built to the ratified size and graded, this RFC claims coverage disclosure and nothing more, the register fallback stands, and a receipt MUST NOT render coverage as adequacy.

## Mutation challenge (gates 6 and 7)

Gates 6 and 7 are the docs/41 mutation challenge split by target: `property_mutation` mutates what is checked, `defect_mutants` mutates what is checked *against*.

- A property mutant that no longer fails means the property is no longer doing work: the gate is `failed` and the specific mutant is named. A mutant the campaign never ran is evidence of nothing and leaves the gate `pending`.
- A defect mutant that is no longer detected means instrumentation or a monitor has been disabled — the verifier-gaming vector — and the gate is `failed`.
- Mutants are drawn from a held-out corpus. A mutant whose identity leaked into the candidate snapshot MUST be excluded from the result and the exclusion disclosed; a hard-coded mutant or corpus name is itself a gaming signal (docs/50, overfitting attacks).
- Both gates are evaluated on the candidate; their prior results are reusable only under the incremental rule below.

## Cost governance

Absorbed from plan §8.6 (SD-04).

- **Cumulative ledger.** Every transaction carries a cumulative cost ledger across all its versions. The dimensions are the shared budget vocabulary of `verification-task.schema.json` (SD-12): `cpu_ms`, `wall_ms`, `solver_ms`, `memory_bytes`, and `tokens` are required; `bytes`, `candidates`, `proof_ms`, and `states` are optional. The promotion receipt carries the same block.
- **Incremental gate campaigns.** Gate 5–7 evaluations (`neighborhood`, `property_mutation`, `defect_mutants`) are incremental by default: a neighborhood class or mutant whose causal footprint a §9 dependency query judges disjoint from the patch delta may reuse its prior result; anything else re-runs. The class of that reuse edge is decided by RFC 0030, not by convenience:

  - a **conservative** disjointness judgement licenses a `Conservative` reuse edge;
  - it licenses a `Validated` edge **only** when the disjointness itself carries a witness an independent checker accepts *for this input digest*;
  - an `Unknown` disjointness is dependent: no class licenses reuse across it, and the class or mutant re-runs (RFC 0030, "Independence and the invalidation cone");
  - an `Experimental` edge MUST NOT support promotion, and by the meet rule one `Experimental` edge anywhere in a derivation makes the whole derivation `Experimental` — so a gate resting on one cannot pass.

  The previous revision of this bullet said a conservative disjointness judgement licensed a `Validated` edge. It was wrong and is corrected here (correction 1).
- **Reuse is disclosed.** A gate 5–7 result that was reused rather than re-run MUST be reported with its reuse-edge class and the `function_id`/`function_version` it was served under, and MUST NOT be rendered as a fresh campaign. A quarantined triple drops to `Experimental` and therefore cannot support promotion until it is cleared (RFC 0030, "Quarantine").
- **Ceilings.** The daemon enforces per-transaction and per-principal cost ceilings, and the two are different failures. Exceeding a per-transaction ceiling is task-budget spend: it MUST yield `BudgetExhausted` carrying a continuation, and `repair.resume` continues the campaign monotonically — never a silently smaller campaign (the cost-domain form of INV-007). Exceeding a per-principal capability ceiling yields `QuotaExhausted`, which `rule errors.common` admits on every operation and which carries no continuation, because the campaign is admissible and the principal is not.
- **Resume is monotone.** The post-resume frontier MUST include the pre-resume frontier; resume MAY add evidence or refine an `Unknown` and MUST NOT replace prior artifacts under the same identity (INV-009). Lowering a budget below committed spend suspends with a continuation rather than truncating (`task.update_budget`, B18).
- The §9.5 audit sampling rate derives from a declared statistical confidence target for mismatch detection per reuse class, reviewed at G5. That derivation belongs to RFC 0030; this RFC consumes it and ratifies nothing.

## Intent integrity and reclassification

If gate 3 detects a protected change — including a non-affirmative relation, which fails closed — the transaction is `blocked`. It MAY be explicitly reclassified as an intent revision. Reclassification:

1. requires `revise-intent` authority, which is absent from default agent capability profiles (RFC 0027, plan §5.4);
2. routes the intent delta through RFC 0037's revision procedure — a new `in_*`, the review path, an acceptance record, and evidence invalidation;
3. records the reclassification decision as evidence, audit-recorded (plan §18.5);
4. re-bases the transaction on the accepted revision, which is a new transaction, because the base triple is frozen.

Reclassification is never implicit (INV-011). A `changes` entry of kind `intent` submitted through ordinary repair authority MUST fail `IntentMutationDenied` at `repair.apply`; it is admissible only on a transaction already reclassified. Weakening a property, strengthening an assumption, shrinking a bound, hiding an event, removing a fault, or downgrading assurance is a privileged intent revision with a semantic diff, never a repair (INV-001).

## Concurrency and lineage

Multiple transactions MAY share a base snapshot, and multiple agents MAY append to one transaction. Two serialization rules keep that safe:

- **Per transaction.** `apply`, `attach`, `evaluate`, `promote`, and `reject` are linearized against the lineage head. A write against a non-head version loses its compare-and-set and returns `StatusConflict`; the caller re-reads the head and retries. The loser's version becomes `superseded`, never discarded.
- **Per base lineage.** Promotion is serialized per base lineage. The first promotion advances the lineage; a second promotion against the now-stale base MUST fail `StatusConflict` and MUST be re-based and re-evaluated. Semantic evidence is never auto-merged: two candidate repairs of one failure are two hypotheses, and merging their evidence would produce a receipt describing a campaign no evaluation ever ran.

## Promotion

`repair.promote` is `@privileged @audit_recorded`, requires `promote` authority, and executes as one semantically atomic step (INV-017):

1. verify the transaction is the lineage head and its status is `ready`;
2. re-compute the semantic and intent classification and the `PolicyDecision` server-side from current evidence; a cached or client-supplied verdict is never trusted;
3. re-check every gate's referenced evidence for resolvability and freshness, including certificate and proof freshness (gate 9's predicate);
4. compose the receipt;
5. verify the composed receipt by reference through `evidence.verify`, which re-fetches and re-checks every referenced artifact and accepts no client-declared status;
6. sign the receipt with the daemon's receipt-service key;
7. publish content before index (plan §4.5) and advance the base lineage to `result_snapshot`.

**Signing.** The daemon's receipt service holds the signing keys; agents never do (INV-015, plan §18.6). Signatures bind identity and authorship; they never substitute for proof checking (ADR-0035). `receipt_id` and `signature` are excluded from the receipt's own identity preimage — the signature binds the identity, it is not part of it.

**Rollback.** Promotion has no partial state to unwind, by construction:

- a failure at any step before publication leaves the transaction `ready`, publishes nothing, and returns the typed code for the step that failed — `PolicyGateFailed` for a verdict that is no longer `allow`, `InsufficientEvidence` for an unresolvable reference, `CertificateRejected` for a certificate that fails checking, `StatusConflict` for a lost lineage race;
- a publication failure — disk exhaustion or any other abort — returns `PublicationAborted`: nothing is published and nothing is truncated, and the transaction remains `ready` (plan §4.5, INV-017). This is stronger than the per-artifact floor of [RFC 0026](0026-continuumd-native-protocol.md) "Atomicity of publication" (RFC 0026 correction 48), under which an ordered composite MAY leave earlier records published: promotion is one semantically atomic step, so it MUST NOT leave any of its artifacts published when it aborts;
- rejecting a transaction is the rollback of an applied patch. `reject` is terminal, the candidate snapshot is never merged into the base lineage, and it becomes GC-eligible under the retention policy. No artifact is edited and no identity is reused;
- a promoted repair is not rolled back by deletion. The base lineage is advanced again by a subsequent transaction and the superseding relationship is recorded by a `SUPERSEDES` edge (plan §4.6). Published receipts remain verifiable under their pinned epochs indefinitely.

## The promotion receipt

Composed at `receipt_generation`; the normative shape is `promotion-receipt.schema.json`.

| Content | Field | Notes |
|---|---|---|
| transaction identity | `repair_transaction` | the promoted version |
| intent identity | `intent` | the base intent |
| before/after snapshots | `base_snapshot`, `result_snapshot` | — |
| semantic and intent diff | `semantic_diff`, `intent_diff` | one artifact: the intent diff travels as the `intent_changes` set inside it (RFC 0031). Both fields MUST name the same `diff_*` identity |
| gate profile and all twelve gates | `gate_profile`, `gates` | by identity; only `passed` and `not_yet_enforced` may appear |
| replay, neighborhood, mutation, refinement, certificate, and parity results with coverage | `coverage` | per-strategy counts, failing neighbors, truncations, and reuse classes |
| cumulative cost ledger | `cost_ledger` | plan §8.6; the same block the transaction carries |
| unknowns | `unknowns` | every unresolved unknown, explicitly; an absent array is not an empty one (INV-007) |
| policy decision | `policy_decision` | the server-recomputed `allow` |
| checker identity | `checker` | toolchain and build digest (INV-014, ADR-0035) |
| epochs | `semantic_epoch`, `epochs` | the pins the receipt is verifiable under; the two spellings of the semantic epoch MUST agree |
| signature | `signature` | receipt-service identity (plan §18.6) |
| redactions | `redacted_references` | the shared stub (plan §4.5, §18.4) |

- Receipts are canonically encoded and checkable by reference. `evidence.verify` re-fetches and verifies every referenced artifact and accepts no client-declared status; it returns the checker's service identity and the `validation_basis` (`checked-certificate` or `trusted-solver`), which never render identically.
- A receipt referencing purged or lost content remains structurally verifiable and reports the redaction; claims requiring the hidden data downgrade per plan §18.4 rather than disappearing.
- Campaign-class evidence MAY be summarized after promotion (plan §4.5): the receipt then references a coverage summary attested by a kernel-covenant checker, and later access to a raw class yields `Redacted(summarized, commitment)`. Summarization MUST preserve per-strategy coverage and the failing-neighbor disclosure; a summary that drops them defeats the gate-5 counting rule.
- **Forged and agent-signed receipts are rejected.** A receipt whose signature does not verify against an allowed receipt-service identity reports typed unverified provenance (plan §18.6) and MUST NOT support any gate; a receipt naming an artifact that does not resolve or whose digest disagrees fails verification with `InsufficientEvidence` or `CertificateRejected`. Handle possession confers nothing (ADR-0037): presenting a well-formed `receipt_*` is not evidence that a promotion happened.

## Corrections recorded by this RFC

Per plan §25, where plan prose, docs, or a dependent artifact disagrees with this RFC, this RFC governs — except for wire shapes and error codes, where RFC 0026's IDL governs and this RFC is corrected, and except for artifact shape, where the normative schemas govern (INV-003). The corrections in force:

1. **A conservative disjointness judgement licenses a `Conservative` edge, not a `Validated` one.** The previous revision's cost-governance bullet said gate 5–7 evaluations reuse prior results "as `Validated` edges" when a "`Conservative`-class §9 dependency query" finds the causal footprint disjoint from the patch delta, restating plan §8.6 in its own words. `Validated` requires a witness an independent checker accepts for the specific input digest, and a conservative over-approximation is not one; the sentence would have let `neighborhood`, `property_mutation`, and `defect_mutants` rest on unwitnessed reuse. Normative: a reuse licensed by a conservative disjointness judgement is `Conservative`; it is `Validated` only when the disjointness itself carries an independently checkable witness; an `Unknown` disjointness is dependent and re-runs; an `Experimental` edge never supports promotion. Direction: RFC 0030 is the normative home of the closed reuse-edge class set and already recorded this correction against plan §8.6 and against this RFC (RFC 0030, correction 6, which states that this RFC's bullet "carries the same error and MUST be revised to match"); **this RFC is corrected to agree with RFC 0030**, and plan §8.6 stands corrected by RFC 0030.

2. **`repair.begin` does not take a base snapshot or an intent.** The previous revision wrote `begin(failure: crash_*, base: ws_*, intent: in_*)`. The IDL declares the request as `{failure: CrashpackHandle, gate_profile: GateProfile}`. Normative: the daemon resolves the base snapshot from the crashpack and the base intent from that snapshot's binding (RFC 0037), and records both in the transaction artifact. Direction: the IDL decides; this RFC is corrected to its shape.

3. **A stale-base promotion is `StatusConflict`, not `StaleSnapshot`.** The previous revision said a second promotion against a stale base "fails `StaleSnapshot`". Under `rule errors.common`, `StaleSnapshot` enters an operation's union only when the operation takes a non-null `snapshot`, and no `repair.*` operation does; `repair.promote` declares `StatusConflict`, which is exactly the lost compare-and-set this case is. Normative: the code is `StatusConflict`, and the transaction must be re-based and re-evaluated. Direction: the IDL decides; this RFC is corrected.

4. **`gate_profile` is declared at `begin` and frozen.** The previous revision referred to "the transaction's `gate_profile`" without saying where it comes from, leaving room for a profile chosen at evaluation or promotion time — which would let a campaign pick the profile its results happen to satisfy. Normative: the profile is a required `repair.begin` request field and is fixed for the transaction's lifetime. Direction: this RFC states what the IDL already fixes.

5. **`NotYetEnforced` is not the wire token.** Plan §21 and `notes/START_HERE_IMPLEMENTATION.md` PR 22 write `NotYetEnforced`; the schemas and the IDL carry `not_yet_enforced`. Normative: the lowercase token is the value and the CamelCase spelling is prose. Direction: RFC governs; the prose spelling is unaffected in meaning. (Mirrors RFC 0031's correction 9 for the non-affirmative relation tokens.)

6. **Gate 1 failure and replay divergence are different outcomes.** docs/41 step 1 says "If replay diverges, block with `ReplayDiverged`", collapsing two conditions. Normative: `base_replay` is `failed` when the failure does not reproduce on the base snapshot; a `ReplayDiverged` engine condition is `inconclusive` and MUST publish a `defect_*` artifact (plan §4.7). An engine defect is never a semantic verdict about the client's program. Direction: RFC governs docs/41.

7. **Gate 8's "unexpectedly" is not a predicate.** Plan §8.2 gate 8 reads "refinement coverage is not reduced unexpectedly", which no daemon can evaluate. Normative: the gate passes iff coverage does not decrease, or the decrease is declared in the transaction and permitted by the evaluation policy with the delta disclosed in the receipt; an undeclared decrease is `failed` and an uncomputable delta is `inconclusive`. Direction: RFC makes plan §8.2 machine-checkable.

8. **Gate 3 passes on the policy verdict, not on the phrase "protected intent is unchanged".** Plan §8.2 gate 3 admits a reading in which an unclassified or non-affirmative relation is not a change. Normative: the gate passes iff the RFC 0031 classification is complete over all fifteen protected fields and the recomputed decision is `allow`; `unknown`, `unsupported`, and `incomparable` on a protected field each forbid `allow`. Direction: RFC states plan §8.2 gate 3 in RFC 0031's vocabulary; RFC 0031 governs the classification itself.

9. **Per-transaction and per-principal ceilings are different errors.** Plan §8.6 assigns `BudgetExhausted` to both. RFC 0026 defines `QuotaExhausted` as capability concurrency/resource exhaustion, explicitly "distinct from `BudgetExhausted`, which is task-budget spend and carries a continuation". Normative: a per-transaction ceiling yields `BudgetExhausted` with a continuation; a per-principal capability ceiling yields `QuotaExhausted` with none. Direction: RFC 0026's taxonomy governs; this RFC records the split plan §8.6 collapses.

10. **The intent diff is not a second classification.** `promotion-receipt.schema.json` requires both `semantic_diff` and `intent_diff`, while RFC 0031 states there is one diff artifact shape and the intent diff travels as the `intent_changes` set inside it. Normative: both fields MUST name the same `diff_*` identity for a transaction with one classification; a receipt naming two different diff artifacts is malformed. Direction: the schema decides shape (INV-003); this RFC fixes the semantics, and the redundancy is raised as flag F7. The shipped `promotion-receipt.example.json` was non-conformant with this rule — its `intent_diff` and `semantic_diff` named two different diff identities — and was corrected, **paid by the bn-1lfek schema sweep, 2026-07-31**: both fields now name the same `diff_demo1` identity, and both fields' schema descriptions state the agreement rule. The agreement itself stays unenforceable in the schema (draft 2020-12 has no keyword to compare two sibling values), which is the surviving half of F7.

11. **A receipt MUST carry its policy decision, coverage, and unknowns.** The receipt schema makes `policy_decision`, `coverage`, and `unknowns` optional, so a schema-valid receipt can omit the decision it records, the coverage plan §8.3 requires it to disclose, and the unknowns plan §8.1 lists among the transaction's contents. Normative: all three are required by this RFC, and an absent `unknowns` is not an empty one (INV-007). Direction: this RFC states the obligation; schema permissiveness is not a licence, and the gap is raised as flag F6.

12. **The hypothesis is not evidence, and neither is a counterfactual.** docs/41 states the separation in prose and the schema types `hypothesis` as a required string. Normative: `hypothesis` and plan §8.4's counterfactual results MUST NOT contribute to any gate, to the policy verdict, or to the receipt's decision, and MUST NOT be interpolated into typed fields or error text (INV-003, INV-016). Direction: RFC makes docs/41's sentence normative and extends it to plan §8.4.

13. **Plan §8.5's reviewer panel is a rendering.** "Causal neighborhood: 38,412 classes explored" is one number standing in for eight strategies' coverage. Normative: the record is per-strategy coverage plus every failing neighbor, and the panel is a projection of it. Direction: RFC governs plan §8.5's illustrative block, which is illustrative by its own framing.

14. **docs/41's `RepairTransaction` struct is not the artifact shape.** Its `proposed_changes`, its flat `evidence: Vec<EvidenceHandle>`, and its typed `evaluation_policy: RepairPolicy` do not match the schema's `changes`, its per-gate `evidence`, and its `evaluation_policy` string. Normative: the schema decides (INV-003); the field map is the table under "The transaction object". Direction: RFC governs docs/41; the regeneration (SD-10) was paid by bn-7kj on 2026-07-31, and the struct was removed from docs/41 by it.

Flags raised against artifacts this RFC does not own (no silent divergence):

- **F1 — `repair.apply` and `repair.evaluate` have no typed lifecycle-refusal code.** Both must refuse a write against a terminal or non-head transaction, and `StatusConflict` is in neither operation's `errors` clause nor in `rule errors.common`'s union, while `attach`, `promote`, and `reject` all carry it. Raised for the protocol sweep; this RFC does not edit the IDL.
- **F2 — the transaction's `semantic_diff` pattern is the generic handle pattern.** `repair-transaction.schema.json` accepts any `^[a-z][a-z0-9_]*_…` handle where RFC 0031 requires a `diff_*` identity; the receipt schema correctly pins `^diff_`. The looser pattern is permissive, not licensing.
- **F3 — `performance_impact`, `security_review`, and `evaluation_policy` are untyped.** The first two are `object | string` and the third is `string | null`. A prose security review sitting in a machine field is exactly what INV-003 forbids, and an unstructured policy reference cannot be checked against the campaign that ran. Typed shapes belong in the schema; the policy surface is an open question below.
- **F4 — gate `evidence` entries are unconstrained.** The items carry only the generic handle pattern, with no binding to evidence-graph nodes and no minimum claim status, so the evidence floor above is unenforceable structurally; `passed` with an empty `evidence` array is schema-valid and is prohibited here by prose alone.
- **F5 — `version` and `supersedes` are not tied.** Nothing in the schema forces `version = 1` iff `supersedes` is absent, so a lineage can be forged by hand-writing a v1 that carries a predecessor pointer.
- **F6 — `policy_verdict` is optional at every transaction status,** so a `ready` transaction can omit the verdict promotion is supposed to recompute against. Paired with correction 11 on the receipt side.
- **F7 — the receipt carries two spellings of one epoch and two fields for one diff.** `semantic_epoch` duplicates `epochs.semantic`, and `semantic_diff`/`intent_diff` name one artifact twice. Two spellings of one fact can disagree; a schema that requires both should also require their agreement.
- **F8 — coverage has no machine shape.** `coverage` is an untyped object with `additionalProperties: true`, so the per-strategy counts, the truncation disclosures, and — decisively — the failing-neighbor disclosure the ratified FR-05 counting rule depends on have no field to live in. An adequacy measurement cannot be graded mechanically against an unstructured blob. Raised for the schema sweep as the highest-value gap in this pair of schemas. — **Paid by the bn-1lfek schema sweep, 2026-07-31**: `coverage` is now a closed object over the named groups — `replay`, `neighborhood`, `property_mutation`, `defect_mutants`, `refinement_coverage`, `certificate_rebuild`, `incremental_parity` — with per-strategy `explored`/`failing`/`truncated` counts over the eight §8.3 strategies, a `frontier` required whenever a strategy is truncated, and the `failing_neighbors` array the FR-05 counting rule is stated over.
- **F9 — reuse is invisible in the receipt.** No field records the reuse-edge class, `function_id`, or `function_version` of a gate 5–7 result served from cache under the incremental rule, so a reader cannot distinguish a re-run campaign from a reused one, nor a `Conservative` reuse from a `Validated` one. Correction 1's distinction is unauditable until this lands. — **Paid by the bn-1lfek schema sweep, 2026-07-31**: a new `reuse_provenance` definition — reuse-edge class plus `function_id`/`function_version` — is recordable per neighborhood strategy and per mutation campaign, and is required whenever a strategy reports `reused >= 1`. Correction 1's Conservative-vs-Validated distinction is now auditable from a receipt.

## Rejected alternatives

- **Mutable transaction document.** Rejected: it destroys auditability and the multi-agent append semantics docs/44 depends on.
- **Client-computed promotion verdicts.** Rejected: promotion is the single most attackable decision, so it is recomputed server-side from current evidence.
- **Omitting unenforced gates.** Rejected: a Phase B receipt must be structurally distinguishable from a Phase D receipt, or phase staging becomes silent scope-hiding.
- **A `not_applicable` gate status.** Rejected (SD-11): to a reviewer "not applicable" is indistinguishable from "not checked", and it is the exact shape of a scope claim with no evidence behind it.
- **Deriving gate status from claim status.** Rejected: a gate is a step's outcome and a claim status is a claim's disposition. Collapsing them would make `inconclusive` mean two things and would let a promoted claim carry a gate that never ran.
- **Auto-merging two candidate repairs of one failure.** Rejected: the merged transaction's receipt would describe a campaign no evaluation ever ran.
- **A single aggregate neighborhood count.** Rejected: it cannot distinguish a broad campaign from one strategy run eight times, and it cannot satisfy the ratified counting rule, which is stated over disclosed neighbors.
- **Treating budget exhaustion as an inconclusive gate.** Rejected: it converts a resumable campaign into a decided one and loses the continuation (INV-007, INV-008).
- **Letting a client assert `status`.** Rejected: the status is a function of the gates and the verdict, and a client-writable status is a promotion path that bypasses every gate.

## Open questions

- The policy DSL surface for per-property and per-branch gate requirements — declarative TOML per plan §5.4 versus an embedded policy engine — and what `evaluation_policy` should reference once it has one (F3).
- Merge assistance for re-basing a blocked transaction across an accepted intent revision, given that the base triple is frozen and the re-based transaction is a new one.
- Whether the per-gate evidence floor above should be declared per gate in a schema rather than in this RFC's prose (F4), and whether gates 5–7 should record `sampled` versus `bounded` structurally.
- Whether campaign summarization (plan §4.5) can preserve the failing-neighbor disclosure at acceptable cost, or whether the adequacy corpus must be run with summarization disabled.
- Whether gate 12's self-reference — a receipt asserting its own composition gate — should instead be recorded as a separate verification artifact the receipt references.

## Acceptance

Each item is a required test vector.

- **State-machine totality:** every (operation, status) pair produces either a committed version or a typed refusal that publishes nothing, and the refusal is inside the operation's IDL union. `evaluate` on `draft` yields `InsufficientEvidence`; `promote` on `blocked` yields `PolicyGateFailed`; `promote` on `inconclusive` yields `InsufficientEvidence`; every mutation on `promoted`, on `rejected`, and on a non-head version refuses and publishes nothing.
- **Idempotency:** replaying `begin`, `apply`, `attach`, `promote`, and `reject` with the same key and a byte-identical request returns the original identity; a differing request under the same key yields `IdempotencyKeyReused`; a replayed `promote` returns the original `receipt_*` and no second receipt exists.
- **Status derivation:** the derivation is exercised on all nine statuses, including a transaction with one `failed` and one `inconclusive` gate deriving `blocked`, and a `ready` transaction carrying `receipt_generation` at `pending`.
- **Profiles:** a `phase-b` transaction lists gates 9 and 10 `not_yet_enforced` and everything else evaluated; a `phase-c` transaction lists only gate 9; `phase-d` and `default` list none. A gate inside the active profile carrying `not_yet_enforced` is rejected by the schema. A promoted receipt is structurally distinguishable by profile, and no receipt renders an unenforced gate as `passed`.
- **The seven docs/41 failure modes map to failing gates on the mutation corpus:** exact-overfit → `neighborhood`; intent gaming → `intent_integrity`; verifier and instrumentation gaming → `property_mutation`/`defect_mutants`; stale proof → `certificate_rebuild`; availability collapse → non-vacuity within `property_mutation`; opaque escape → `intent_integrity` (trust-boundary expansion); abstraction gaming → `intent_integrity` (RFC 0031 classifies the abstraction map `merged`).
- **PR 21's exit:** a hard-coded exact-trace repair fails gate 5; the semantic guard repair promotes.
- **PR 22's exit:** forged and agent-signed receipts are rejected by `evidence.verify`; a receipt naming an unresolvable artifact fails with `InsufficientEvidence`; a receipt whose certificate fails checking yields `CertificateRejected`; possession of a well-formed `receipt_*` authorizes nothing.
- **Gate 1 separation:** a base snapshot on which the failure does not reproduce yields `failed`; an injected `ReplayDiverged` yields `inconclusive` plus a `defect_*` handle in the result, never `failed`.
- **Budget:** a campaign exceeding a per-transaction ceiling yields `BudgetExhausted` with a continuation and leaves its gate `pending`; `repair.resume` continues monotonically and the post-resume frontier includes the pre-resume frontier; a per-principal ceiling yields `QuotaExhausted` with no continuation; no path records exhaustion as `passed` or as `inconclusive`.
- **Rollback:** an injected publication failure yields `PublicationAborted`, publishes nothing and truncates nothing, and leaves the transaction `ready`; a promotion whose verdict changed between evaluation and promotion yields `PolicyGateFailed` and publishes nothing; `reject` never merges the candidate snapshot into the base lineage.
- **Concurrency:** two transactions sharing a base both evaluate; the first promotion advances the lineage and the second yields `StatusConflict`; a write against a non-head version yields `StatusConflict` and the loser's version becomes `superseded` rather than being discarded; no agent write promotes a claim status, and a regressing write is rejected by the plan §11.7 compare-and-set.
- **Incremental reuse:** a gate 5–7 result reused under a conservative disjointness judgement is recorded `Conservative`; a reuse carrying a checked witness for the input digest is recorded `Validated`; an `Unknown` disjointness re-runs; a result whose derivation touches an `Experimental` edge cannot pass its gate; a quarantined `(reuse-edge class, function_id, function_version)` triple cannot support promotion.
- **Neighborhood disclosure:** every failing neighbor found is present in the receipt with its class and run handle; a run that finds a failing neighbor and omits it fails gate 5; a budget-truncated strategy is disclosed as truncated with its frontier and leaves the gate `pending`. Until the docs/50-classified gaming corpus lands, no test asserts an adequacy rate; when it lands, the ratified threshold and its counting rule are graded by the independent grader of research/33 on a family-level held-out split.
- **Determinism:** for fixed inputs and epochs the transaction and receipt artifacts are byte-identical across the docs/19 §7 determinism matrix, and a receipt remains verifiable under its pinned epochs after an epoch advance.
