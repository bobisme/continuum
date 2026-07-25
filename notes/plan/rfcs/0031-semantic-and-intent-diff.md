# RFC 0031: Semantic and Intent Diff

## Status
Draft for implementation.

**Target gate:** G3 (intent integrity)
**Owners:** intent/diff leads
**Normative language:** MUST/SHOULD/MAY per RFC 2119.
**Normative schema:** [`../schemas/semantic-diff.schema.json`](../schemas/semantic-diff.schema.json).

## Summary

The diff engine is the anti-reward-hacking core (plan B1, INV-001, INV-011, G0-DX-02). Given two snapshots and two intent versions, it classifies every change, computes the evidence impact set, and produces the policy verdict that gates ordinary versus privileged promotion.

## Inputs

Two workspace snapshots, before/after Intent Contract identities, the declared semantic fragments from the intent's `scope`, and the requested assurance for the classification itself.

## Classification lattice

Every intent field change is classified with one relation from the closed set:

```text
unchanged      strengthened   weakened
expanded       contracted
refined        coarsened          (observers)
merged         split              (abstraction maps)
upgraded       downgraded         (assurance / proof policy)
added          removed            (assumptions, faults, fairness, non-vacuity)
incomparable   unsupported        unknown
```

Directional relations are defined per field by these orders:

- **Properties (per fragment):** in `Finite`, exact language inclusion over the bounded universe; `P strengthened P'` iff behaviors(P') ⊆ behaviors(P) with a checked witness. In `Symbolic`, an implication obligation is discharged by SMT with a checked certificate or by Lean; otherwise `unknown`.
- **Bounds:** componentwise partial order on `(values, nodes, faults, depth)`; a decrease in any component with no increase elsewhere is `contracted`; mixed changes are `incomparable`. `no-decrease` blocks `contracted` and blocks `incomparable` pending review.
- **Assurance:** total order `observed < sampled < bounded < validated < proved`; movement down is `downgraded` and is what `no-downgrade` blocks. Checker requirements (`independent_checker`, `clean_recompute`) turning off is also `downgraded`.
- **Observers:** `refined` iff the new observer distinguishes at least the old projections/events; dropping an event family or coarsening a projection is `coarsened`.
- **Assumptions/faults/fairness/non-vacuity:** set membership per classified item (`added`/`removed`), with `strengthened`/`weakened` for edits to an item's expression evaluated in its fragment. Adding an assumption or fairness constraint is environment-strengthening and therefore protected; removing a fault or a non-vacuity behavior is protected.
- **Trust boundaries:** growth of the opaque set is `expanded` and is what `no-expansion` blocks.
- **Completion policy / nondeterminism classes:** any change is a semantic change; cross-policy relations are `incomparable` (there is no soundness order among completion policies).

## Fail-closed rule

- Where implication is decidable or solver-checkable in the declared fragments, Continuum MUST prove the direction and attach the witness/obligation to the change record.
- Otherwise the relation is `unknown` (undecidable/unattempted) or `unsupported` (outside declared fragments) — these are distinct (INV-008).
- `unknown` and `unsupported` on a protected field MUST be treated exactly as a confirmed protected change: ordinary promotion is blocked pending review (plan §5.3). The diff never guesses an ordering.

## Program semantic diff

Program-side changes are classified along these axes: effects (new/removed/moved effect sites), atomicity (split/merged critical regions), task ownership, cancellation structure (checkpoints, obligations), durability ordering, observer publication points, and correspondence (which links of §16's graph a change touches). Each axis yields typed change records with source locations; these feed the impact set and the repair gates, not the intent policy.

## Impact set

The diff MUST emit `invalidated` / `reused` / `unknown` evidence sets computed against the incremental database's dependency classes (RFC 0030). Evidence whose dependency on a changed field is `unknown` goes to `unknown`, never to `reused`.

## Policy verdict

The verdict (`allow | review | block | unknown`) is computed from the per-field relations and the intent's policy verbs. It MUST name the reasons per field. The promotion endpoint re-computes the verdict at promotion time; cached client verdicts are never trusted (RFC 0032).

## Rejected alternatives

- **Token/set comparison of property strings.** Rejected as the production mechanism: defeated by renaming and rewriting (it survives only as the R3 spike baseline).
- **Best-effort ordering for undecidable cases.** Rejected: a guessed "weakened/strengthened" is exactly the false confidence the system exists to prevent.
- **One global strength order across fields.** Rejected: bounds are partial, assurance is total, completion policies are unordered — collapsing them loses the distinctions the locks need.

## Open questions

- Property normalization/equivalence beyond Finite (shared with RFC 0037).
- Which SMT fragments earn `strengthened`/`weakened` with `CHECKED_CERTIFICATE` versus obligation-only.

## Acceptance

- Mutation corpus with known strengthened/weakened/incomparable/unknown relations per field, including renames and formula rewrites that MUST NOT classify `unchanged`.
- All G0-DX-02 gaming patches classify as protected; a source-only guard repair classifies as program change only.
- Fail-closed tests: protected-field `unknown`/`unsupported` blocks ordinary promotion.
- Impact-set soundness: no invalidated-in-truth evidence lands in `reused` on the acceptance corpus.
