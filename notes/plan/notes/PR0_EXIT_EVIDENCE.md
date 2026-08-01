# PR 0 Exit Evidence

**Prepared:** 2026-07-31 (bone `bn-39d3`)
**Scope:** mechanical evidence for the PR 0 exit sentence
([`START_HERE_IMPLEMENTATION.md:94`](START_HERE_IMPLEMENTATION.md)):

> the seven RFCs are normative specifications, and the pre-freeze open-debt
> set (plan §25, validator `check_spec_debt`) reads empty — not only the
> seven RFC expansions; PR 5 may not merge before PR 0 closes.

This document assembles the two halves of that sentence — RFC normative
status and the SD-01…SD-14 ledger — plus the negative case ("not only the
seven RFC expansions") and the open follow-up work that does not block the
sentence. It does not decide anything; see §5.

---

## 1. Per-RFC evidence

Each of the seven Revision 3 RFCs declares its own normative status in its
opening block, under the heading it uses to introduce its own
"Corrections recorded by this RFC" section. The quotation below is that
declaration, taken verbatim with its file and line. The correction count is
every numbered entry under that RFC's "Corrections recorded by this RFC"
heading (the "Where the IDL corrected this RFC" / "Where this RFC decides" /
"Where this RFC governs the plan and docs" subsections), counted by reading
the section, not the separately lettered "Flags raised against artifacts
this RFC does not own" (F-numbered) entries, which are forward-work, not
corrections in force.

| RFC | Delivering bone | Lines | Normative-status declaration (verbatim, file:line) | Corrections recorded |
|---|---|---:|---|---:|
| [0026](../rfcs/0026-continuumd-native-protocol.md) — `continuumd` Native Protocol | bn-3ffu | 578 | "The IDL is the wire artifact; **this RFC is the semantic specification around it**. It is the normative home of: the connection lifecycle and what negotiation fixes; operation semantics and what each annotation obliges a daemon to do; the ordering, idempotency, and atomicity contract; task lifecycle and the continuation-resume admissibility predicate; epoch rules and epoch-advance ordering; the *meaning* of the error taxonomy as distinct from its spelling; the operational contract that crosses the wire …; and capability administration." (`rfcs/0026-continuumd-native-protocol.md:17`) | 36 |
| [0027](../rfcs/0027-agent-tool-protocol.md) — Agent Tool Protocol | bn-26fh | 617 | "This RFC is the normative home of: the five-level authority ladder and its order; the per-operation authority registry, one row per registered operation; the admission predicate a daemon evaluates before any semantic work; what a capability confers, how it is scoped, when it expires, how it delegates, and what revocation does; the agent-facing context policy and the closed expansion-relation vocabulary; deterministic ordering and result bounds as an agent contract; the handoff model and its stale-handle behavior; the privileged-operation boundary; and the prompt-injection boundary." (`rfcs/0027-agent-tool-protocol.md:18`) | 26 |
| [0028](../rfcs/0028-context-pack-format.md) — Context Pack Format and Compiler | bn-4xxw | 320 | "This RFC is the normative home of: the pack's field set and field types; the closed guarantee-class set and its composition rules; the selection-kind set; the ten-stage compiler pipeline and what each stage may license; the budget-packing order and the pack's frontier form; the omission manifest and its reconciliation equation; the expansion protocol and its typed outcomes; the redaction interaction; the four pack profiles; and the rendering rule." (`rfcs/0028-context-pack-format.md:19`) | 15 |
| [0030](../rfcs/0030-incremental-semantic-query-engine.md) — Incremental Semantic Query Engine | bn-3hkk | 412 | "This RFC is the normative home of the query key, the closed reuse-edge class set and its downgrade rules, the closed dependency-reason set, the closed auditability-class set, the invalidation-cone contract, the audit's sampling and quarantine rules, continuation-epoch resume, and the crash-safety ordering." (`rfcs/0030-incremental-semantic-query-engine.md:18`) | 14 |
| [0031](../rfcs/0031-semantic-and-intent-diff.md) — Semantic and Intent Diff | bn-39my | 357 | "This RFC is the normative home of the classification lattice, of the per-field orders the RFC 0037 policy verbs are enforced against, and of the plan §5.3 completeness guarantee." (`rfcs/0031-semantic-and-intent-diff.md:18`) | 12 |
| [0032](../rfcs/0032-repair-transaction-protocol.md) — Repair Transaction Protocol | bn-3i38 | 407 | "This RFC is the normative home of: the transaction object and its field types; the nine-status state machine and the derivation that computes it; the eight `repair.*` operations with their totality and idempotency obligations; the twelve gates, the closed gate-status set, and the phase-staged gate profiles; the neighborhood strategy set and its disclosure obligation; the mutation challenge; cost governance; the promotion procedure; and the promotion receipt's composition, signing, and independent verification." (`rfcs/0032-repair-transaction-protocol.md:19`) | 14 |
| [0037](../rfcs/0037-intent-contract.md) — Intent Contract Schema and Policy | bn-2s6q | 408 | "This RFC is the normative home of: the contract's field set and field types; the closed policy-verb set and its enforcement; canonical identity and the CPNF-1 property normal form; custody in the daemon's intent registry and the registry status machine; signed intent-bundle distribution, import, acceptance chains, and three-way convergence; and the revision procedure." (`rfcs/0037-intent-contract.md:18`) | 11 |
| **Total** | | **3,099** | | **128** |

Every one of the seven declarations also states the correction discipline
that makes the normative claim checkable rather than asserted: where plan
prose, docs, a research note, or a dependent artifact disagrees with the
RFC, the RFC governs (plan §25: "Where plan prose and RFC disagree, the RFC
is corrected and becomes normative"), except for wire shapes, where RFC
0026's IDL governs and the RFC is corrected to it, and except for artifact
shape, where the applicable JSON Schema governs (INV-003). All 128
corrections above are recorded with a direction (which artifact governed,
which was corrected) — none is a bare assertion.

## 2. Spec-debt evidence

The pre-freeze open-debt set is plan §25's SD-01…SD-14 ledger, and its
status is validator-derived, not hand-asserted (plan.md:2648-2653): "each
item below carries a stable ID and a mechanical predicate in
`tools/validate_dossier.py` (`check_spec_debt`); the open set is emitted
into `validation-results.json` … never asserted by hand." Run from the
workspace root:

```
cd notes/plan && uv run --with jsonschema python3 tools/validate_dossier.py
```

The `spec_debt` block of the run performed for this document (workspace
`bn-39d3`, commit `c2eb668`, 2026-07-31), pasted verbatim from the
validator's own JSON output (not from `validation-results.json`, which the
validator reads as input and this task does not touch):

```json
"spec_debt": {
  "open": [],
  "paid": [
    "SD-01",
    "SD-02",
    "SD-03",
    "SD-04",
    "SD-05",
    "SD-06",
    "SD-07",
    "SD-08",
    "SD-09",
    "SD-10",
    "SD-11",
    "SD-12",
    "SD-13",
    "SD-14"
  ]
}
```

The same run's top-level result: `"status": "pass"`. `open` is `[]` and
`paid` names all fourteen SD IDs — the pre-freeze open-debt set reads
empty, mechanically, on this workspace's tree.

## 3. Negative/boundary evidence — "not only the seven RFC expansions"

The exit sentence explicitly distinguishes the RFC expansions from the
larger SD-01…SD-14 debt set. Nine of the fourteen items are not RFC
expansions at all — they are docs regenerations/absorptions and one schema
change — and the ledger is the record of what else PR 0 had to pay. One
line per item, from plan §25 (plan.md:2664-2743), with its paying bone
where the ledger names one:

| ID | What it required, beyond the seven expansions | Paying bone (where named) |
|---|---|---|
| SD-01 | RFC 0026's normative IDL file (`schemas/continuumd-native-protocol.idl`) — all 72 §10.2 operations, envelopes, handshake. | — (not named in the §25 ledger; delivered alongside RFC 0026, bn-3ffu) |
| SD-02 | RFC 0026 absorbs §4.3's N/N−1 protocol-major window, evidence/receipt readability decoupling, `task.update_budget`, evidence subscriptions, and five §10.3 error codes. | — |
| SD-03 | RFC 0030 absorbs §9.5's auditability classes, class-scoped quarantine, and the audit sampling rate derived from §8.6's confidence target. | — |
| SD-04 | RFC 0032 absorbs §8.6's incremental gate 5–7 reuse rule, cost ceilings, and `BudgetExhausted`-with-continuation. | — |
| SD-05 | RFC 0037 carries the §4.2.1 intent-bundle distribution and convergence section. | — |
| SD-06 | RFC 0038 carries the §11.7 evidence-graph write/concurrency model. | — |
| SD-07 | `schemas/intent-contract.schema.json` — a structured property-AST expression form with canonical normalization, replacing the bare `expression` string. | — |
| SD-08 | One `$id`/versioning convention with a `schema_epoch` field across all schemas (§4.3), normative in `schemas/README.md`. | — |
| SD-09 | docs/35, RFC 0026, ADR-0018, and docs/42 absorb the §4.5/§4.6/§4.7 operational contract (purge and `Redacted(reason, commitment)`, backup/verified restore, the cross-user dedup existence-oracle rule, two-epoch migration, `Preserved \| Revalidate \| Incompatible` compatibility statements, engine-defect artifacts). | — |
| SD-10 | docs/41 regenerated around the 12-gate, phase-profile repair design as an explicitly non-normative guide; RFC 0032 and the two repair/receipt schemas remain normative. | **bn-7kj**, 2026-07-31 (named in `rfcs/0032-repair-transaction-protocol.md:19` and `:356`) |
| SD-11 | Fail-closed repair of the anti-gaming schemas: `semantic-diff` binds `protected` structurally; `repair-transaction` drops the undefined `not_applicable` status and gains `phase-c`/`phase-d` conditionals; gate lists enforce the twelve gate identities, not a count; `evidence-graph-node`/`-edge` require service identity and `checker` (INV-004). | — |
| SD-12 | One assurance envelope: `context-pack` mirrors the canonical nine-dimension envelope (identity-checked against `assurance-result`); one verdict vocabulary; one gate-status enum; one budget/cost dimension list shared by RFC 0026, `verification-task`, and the §8.6 cost ledger. | — |
| SD-13 | `verification-task` requires a continuation on `suspended` and on `BudgetExhausted`; `workspace-snapshot` carries all ten §4.2 components and drops the protocol epoch from snapshot identity. | — |
| SD-14 | The shared `Redacted(reason, commitment)` definition mirrored verbatim, identity-checked by the validator, into every artifact class §4.5 obligates (receipts, transactions, envelopes, evidence nodes, tasks). | — |

Where a row has no named bone, plan §25 records only "(paid, PR 0)" or
"(paid, review 5)" against it — it attributes the work to the PR or review
pass, not to an individual bone identifier — and this document does not
invent an attribution the ledger does not state. SD-01, SD-07, SD-08,
SD-09, and SD-10 are marked "(paid, PR 0)"; SD-02 through SD-06 and SD-11
through SD-14 are marked "(paid, review 5)", i.e. paid before PR 0 was
opened as a bone-tracked item. All fourteen read `paid` in §2's live run
regardless of which pass paid them.

## 4. Residuals — open, do not block this exit sentence

These bones touch RFC or protocol surfaces and remain open. None is a
member of the SD-01…SD-14 ledger; each is forward-work the RFCs' own
"Flags raised" / "Open questions" sections, or a downstream bookkeeping or
tooling task, recorded on top of a specification already normative.

| Bone | What it is | Why it does not falsify "the seven RFCs are normative specifications" |
|---|---|---|
| bn-3ayom | RFC 0027 flags F1–F8 (IDL/protocol follow-ups) triage + the T4 actor-binding placement question. | Flags are forward-work an RFC raises against artifacts *it does not own* (here, the IDL) — RFC 0027 says so under its own "Flags raised against artifacts this RFC does not own" heading. A flag is a recorded gap in the wire encoding of an already-normative rule, not a defect in the rule; RFC 0027's normative status is a property it already holds over the ladder, registry, admission predicate, etc., independent of whether the IDL later gains a field to make a flag structurally checkable. |
| bn-3l5la | RFC flag bookkeeping after the schema sweep (bn-1lfek): mark RFC 0028/0031/0032/0037 flags paid where the schema sweep already paid their schema-side half, and correct RFC 0028's "sixteen required properties" to seventeen (`content_budget` is now required). | Bookkeeping that brings each RFC's own flag ledger up to date with schema changes that already landed; it does not add, remove, or contradict any of the 128 corrections in force, and the count correction is a documentation catch-up on a schema fact, not a re-opening of RFC 0028's normative content. |
| bn-13nlo | `schemas/examples/semantic-diff.example.json` violates RFC 0031's own MUST that an unchanged intent use the shortcut form. | The fault is in a shipped example instance, not in RFC 0031's specification text — the MUST already exists, is already normative, and is exactly the rule the example needs to be brought into compliance with. Fixing the example does not change what the RFC requires. |
| bn-2ryko | One-line fixes: RFC 0032's Validated-edge sentence and docs/42's quarantine-scoping sentence, both flagged from the RFC 0030 expansion. | Both corrections are already recorded and directioned inside RFC 0030's own "Corrections recorded" list (correction 6) and inside RFC 0032's correction 1, which states outright that RFC 0032's prior sentence "carries the same error and MUST be revised to match" RFC 0030. The normative content is already decided; this bone is the mechanical propagation of a decision already made, not an open question. |
| bn-l4kmc | Regenerate docs/36, docs/46, docs/55 against normative RFC 0026 + the IDL. | These three documents are self-declared non-normative projections (RFC 0026 corrections 24–36 already state, with direction, what governs wherever they disagree with docs/36/46/55). RFC 0026 already governs the disagreement; only the docs' own prose needs to catch up to a normative status the RFC already has. |
| bn-14dqx | Sync `plan.typ` frontier register rows with the ratified `plan.md` rows (FR-01, FR-04, FR-05, FR-18, FR-20, FR-21). | A typesetting-source (`plan.typ`/PDF build) staleness problem against `plan.md`. The dossier validator checks only `plan.md` (bone description: "The dossier validator checks only plan.md"); it does not touch any RFC, and `plan.md` — the source `check_spec_debt` and the register rows are checked against — is already correct. |
| bn-3ehdm | Two-revision governance gate: enforce GOV-1-08/09 (ADR-for-semantic-change, epoch-bump-on-breaking-change) against the merge base rather than single-revision state. | A governance-tooling strengthening (adding delta/diff-based enforcement) unrelated to RFC content. GOV §1 already exists and already holds at its current single-revision precision; this bone raises the *precision* of an existing, passing check, and does not touch any of the seven RFCs, the IDL, or the SD-01…SD-14 ledger. |

## 5. What this document does NOT do

- It does not close PR 0. Closing PR 0 is the goal bone `bn-2hm`'s
  decision, gated on all of PR 0's deliverables, not only the exit
  sentence evidenced here.
- It does not declare Phase A exited. The integrated exit-evidence and
  privileged-decision package is `bn-2ofd` ("Phase A integrated exit
  evidence and privileged decision package"), which depends on this bone
  (`bn-39d3`) among ~130 others and is the package that goes to the
  human for the actual exit decision.
- It does not decide the protocol-minor-bump question. The IDL's own
  revision history records it as open and explicitly not taken silently:
  the IDL 1.1 additive fixes are compatible changes that, under
  `rule versioning.compatible_change`, "MUST raise the minor version," yet
  `version` is left at `"3.0"` in the IDL header, with the comment "the
  bump is flagged for the protocol sweep rather than taken silently"
  (`schemas/continuumd-native-protocol.idl:164-168`). Whether and when to
  take that bump — bound up with bn-3ayom's F1–F8 disposition, which is
  itself scoped to land "paid in the IDL (minor bump per RFC 0026's
  versioning rules) or explicitly deferred" — is routed to `bn-2ofd`, not
  decided here.
- That routing, and any other privileged judgment call PR 0's closure
  requires, belongs to `bn-2ofd`.
- It is evidence, not the decision. Sections 1–4 above are a citation-dense
  restatement of facts already recorded in the seven RFCs, `plan.md` §25,
  and the validator's own output — assembled in one place for the human
  reviewer, asserting nothing beyond what those sources already state.

## 6. Ratification record

Both pending decisions above were taken by the human lead on 2026-07-31,
after this document was assembled:

- **PR 0 closure: ratified.** The goal bone `bn-2hm`'s completion (recorded
  mechanically when `bn-39d3` closed) stands as the exit of record. PR 5's
  "may not merge before PR 0 closes" precondition is therefore discharged;
  PR 5 remains gated by its own dependencies.
- **Protocol-minor bump: deferred and bundled, not skipped.** `version`
  stays `"3.0"` with the IDL's in-file note until `bn-3ayom` (RFC 0027
  F1–F8 disposition) lands its wire changes; that bone then takes one
  minor bump (3.0 → 3.1) covering IDL 1.1's compatible fixes and the
  sweep's additions together. Recorded on `bn-2hm` and `bn-3ayom`.
