# RFC 0032: Repair Transaction Protocol

## Status
Draft for implementation.

**Target gate:** G4 (causal debugging and real repair)
**Owners:** repair/workbench leads
**Normative language:** MUST/SHOULD/MAY per RFC 2119.
**Normative schema:** [`../schemas/repair-transaction.schema.json`](../schemas/repair-transaction.schema.json).

## Summary

A repair is a transaction: an immutable proposal plus accumulating evidence, promoted only when the policy gates close (plan §8, B8, INV-011).

## Operations

`repair.begin / apply / attach / evaluate / resume / review / promote / reject` (plan §10.2).

- `begin(failure: crash_*, base: ws_*, intent: in_*)` → `rt_*` v1. The base triple is fixed for the transaction's lifetime.
- `apply(patch, hypothesis)` → new version with sealed candidate workspace. Patches apply to the declared base snapshot only; hidden edits (candidate workspace differing from base+patch) fail gate `patch_application`.
- `attach(evidence)` — agents and services append typed evidence nodes; nothing is edited in place (docs/41: swarm workers append, they do not edit one mutable document).
- `evaluate` — runs the gate campaign for the transaction's `gate_profile`; returns a new version with gate statuses.
- `resume(cont_*)` — continues a budget-suspended evaluation monotonically.
- `review` — produces the reviewer projection (plan §8.5) and records the review decision as evidence.
- `promote` / `reject` — terminal. `promote` re-computes the policy verdict server-side at promotion time from current evidence; it MUST NOT trust any cached client verdict.

Every operation returns a new immutable transaction version; prior versions remain addressable (plan §8.1).

## Gates

The twelve gates (plan §8.2), by schema id:

| # | Gate id | Claim |
|---|---|---|
| 1 | `base_replay` | the original failure replays on the base snapshot |
| 2 | `patch_application` | patch applies cleanly; no hidden edits |
| 3 | `intent_integrity` | RFC 0031 classifies no protected change (`Unknown` blocks) |
| 4 | `exact_regression` | the exact failure no longer occurs |
| 5 | `neighborhood` | neighboring schedules/faults/values explored |
| 6 | `property_mutation` | property mutations still fail where expected |
| 7 | `defect_mutants` | known defect mutants remain detected |
| 8 | `refinement_coverage` | refinement coverage not reduced unexpectedly |
| 9 | `certificate_rebuild` | invalidated certificates/proofs rebuilt |
| 10 | `incremental_parity` | clean and incremental results agree |
| 11 | `code_and_security` | tests, static verification, security gates pass |
| 12 | `receipt_generation` | promotion receipt composed and verified |

Gate status is exactly one of `passed / failed / pending / inconclusive / not_yet_enforced` — this enum is closed and `not_applicable` does not exist.

**Gate profiles** are phase-staged (plan §21): `phase-b` enforces 1–8, 11–12; `phase-c` adds 10; `phase-d`/`default` enforce all twelve. Gates outside the active profile MUST appear with status `not_yet_enforced` — present and visibly unenforced, never omitted or passed. A promoted transaction's receipt MUST list all twelve gates by identity (the gate ids above, not a count), with no `pending`/`failed`/`inconclusive` (schema-enforced).

## Neighborhood construction

Property-directed and budgeted, drawn from the eight strategies of plan §8.3: alternate enabled events at causal decisions; fault insertion/removal around the repaired window; cancellation at adjacent checkpoints; value/name permutations; message duplication/loss/delay changes; schedule perturbations within the trace-class boundary; abstraction-map generated variants; hidden corpus-style mutations. The receipt records which strategies ran and their coverage counts — silent caps are prohibited.

## Cost governance

Absorbed from plan §8.6 (SD-04):

- **Cumulative ledger.** Every transaction carries a cumulative cost ledger (CPU, wall, solver, memory, token) across all its versions; the promotion receipt includes it (see Receipt).
- **Incremental gate campaigns.** Gate 5–7 evaluations (`neighborhood`, `property_mutation`, `defect_mutants`) are incremental by default: neighborhood classes and mutants whose causal footprint is disjoint from the patch delta — a `Conservative`-class §9 dependency query (RFC 0030) — reuse prior results as `Validated` edges; anything else re-runs.
- **Ceilings.** The daemon enforces per-principal and per-transaction cost ceilings. Exceeding a ceiling MUST yield `BudgetExhausted` with a continuation (`repair.resume` continues the campaign monotonically) — never a silently smaller campaign (the cost-domain form of INV-007).

## Reclassification as intent revision

If gate 3 detects a protected (or `Unknown`) intent change, the transaction blocks. It MAY be explicitly reclassified as an intent revision: this routes the intent delta through RFC 0037's revision procedure (review path, new `in_*`, evidence invalidation), records the reclassification decision as evidence, and re-bases the transaction on the accepted revision. Reclassification is never implicit (INV-011).

## Concurrency

Multiple transactions MAY share a base. Promotion is serialized per base lineage: the first promotion advances the lineage; a second promotion against the stale base fails `StaleSnapshot` and MUST be re-based and re-evaluated (no auto-merge of semantic evidence).

## Receipt

Composed at `receipt_generation`: intent identity; before/after snapshots; semantic and intent diffs (the intent diff travels as the `intent_changes` set inside the semantic-diff artifact, RFC 0031 — one artifact, referenced as `semantic_diff` in the schema); replay, neighborhood, mutation, refinement, certificate, and parity results with coverage; cumulative cost ledger (plan §8.6); unknowns; policy decision; gate profile. Receipts are canonically encoded and checkable by reference (`evidence.verify` re-fetches and verifies every referenced artifact and accepts no client-declared status). **Signing:** the daemon's receipt service holds the signing keys; agents never do (INV-015). Signatures bind identity and authorship; they never substitute for proof checking (ADR-0035).

## Rejected alternatives

- **Mutable transaction document.** Rejected: destroys auditability and multi-agent append semantics.
- **Client-computed promotion verdicts.** Rejected: promotion is the single most attackable decision; it is recomputed server-side.
- **Omitting unenforced gates.** Rejected: a Phase B receipt must be structurally distinguishable from a Phase D receipt, or phase staging becomes silent scope-hiding.

## Open questions

- Policy DSL surface for per-property/branch gate requirements (declarative TOML per plan §5.4 vs. embedded policy engine).
- Merge assistance for re-basing a blocked transaction across an accepted intent revision.

## Acceptance

The seven failure modes (docs/41) each map to a failing gate on the mutation corpus: exact-overfit → `neighborhood`; intent gaming → `intent_integrity`; verifier/instrumentation gaming → `property_mutation`/`defect_mutants`; stale proof → `certificate_rebuild`; availability collapse → non-vacuity within `property_mutation`; opaque escape → `intent_integrity` (trust-boundary expansion); abstraction gaming (concrete bad states merged) → `intent_integrity` (RFC 0031 classifies the abstraction map `merged`). A hard-coded exact-trace repair fails; the semantic guard repair promotes (PR 21's exit); forged or agent-signed receipts are rejected by `evidence.verify` (PR 22's exit).
