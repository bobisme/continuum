# RFC 0028: Context Pack Format and Compiler

## Status
Draft for implementation.

**Target gate:** G2 (agent-computer interface); replay-preservation obligation feeds G6
**Owners:** context/explanation leads
**Normative language:** MUST/SHOULD/MAY per RFC 2119.
**Normative schema:** [`../schemas/context-pack.schema.json`](../schemas/context-pack.schema.json).

## Summary

A Context Pack is a bounded, typed, property-directed compilation of the evidence graph for one question (plan §6). It is the default unit agents and humans receive; expansion, not dumping, reaches everything else (INV-007).

## Artifact

Per the schema: pack identity (`ctx_*`, the plan §4.4 handle prefix), `schema_version`, target question, snapshot + intent identities, pinned semantic epoch, typed verdict, assurance envelope (every B11 dimension present or typed `Unsupported`), selected items (events, state deltas, obligation/resource flow, order constraints, source/model/proof references, assumptions, counterfactuals, heuristic repair surfaces), per-pack guarantee set, omission manifest, expansion queries, evidence references, replay handle (`crash_*`) and optional debugger handle (`dbg_*`), canonical content hash (ADR-0013), and content budget. The semantic-epoch field is what `ReplayPreserving` is pinned to; a pack without it cannot claim that guarantee. Packs are immutable; `context.expand` creates a child pack referencing its parent.

## Guarantee classes

`guarantees` is a closed enum; multiple MAY apply, and every claimed guarantee MUST be checkable:

| Guarantee | Claim | Checked by |
|---|---|---|
| `ReplayPreserving` | the selected core replays to the same verdict under the pinned epoch | replay execution (INV-006) |
| `PropertyPreserving` | the slice preserves the property automaton's verdict-relevant structure | independent monitor |
| `ProofRelevant` | items lie on the proof dependency slice of the named obligations | proof-slice check |
| `HeuristicRelevant` | ranked-relevant only; no semantic claim | none — MUST be structurally distinct, never mixed into a preserved core |
| `OneMinimal` / `CardinalityMinimal` / `CausallyMinimal` / `ValueMinimal` / `OwnerMinimal` / `FaultMinimal` / `ExplanationMinimal` | which minimality was actually achieved (docs/38) | minimizer transcript |
| `CausallyClosed` | selection is downward-closed under the causal order | closure check |
| `CounterfactualUnderNamedModel` | counterfactual items are valid under a named causality model | model named in the item; "cause" language is prohibited otherwise (plan §12.2) |

The pack MUST record which minimality class was achieved rather than implying the strongest; "minimal" without a class is prohibited output.

## Compiler pipeline

Ten stages (plan §6.3 plus root selection), each producing an auditable intermediate:

1. root selection from property/evidence handles;
2. backward causal slicing (downward closure over the CIR);
3. property-automaton relevance filtering;
4. static/dynamic dependence join for source spans;
5. proof-dependency slicing for obligations;
6. observer projection;
7. abstraction/refinement correspondence mapping (selected concrete items link to their abstract counterparts);
8. minimal unsatisfied core / correction-set analysis where a solver artifact exists;
9. heuristic ranking of optional context (information-gain or configured ranker) — outputs are `HeuristicRelevant` only;
10. budget packing.

Stages 1–8 produce guarantee-bearing content; stage 9 never upgrades an item's guarantee. Redaction policy (plan §18.4) is applied **before** slicing; redactions appear in the omission manifest, and a redacted pack cannot support claims requiring hidden data.

## Budget packing

The enforced budget is bytes (tokens are advisory, recorded with tokenizer id). Packing MUST be lexicographic: (1) every item required by a claimed guarantee (if these alone exceed budget, the compiler MUST drop the guarantee or fail — never silently truncate a guaranteed core); (2) highest-utility optional items; (3) omission-manifest completeness is non-negotiable and reserved before optional content.

## Omission manifest

Counts omitted items by kind and relation, with reason (`budget`, `redaction`, `unsupported`, `heuristic-cutoff`, `slice-irrelevant` — the last for items provably outside the property-directed slice), expandability, and the expansion query that retrieves them. An empty manifest asserts completeness and is checkable.

## Validation

- Replay-preservation check on every pack claiming it (this is plan §15.3 theorem 3's finite statement; the Lean model covers finite causal graphs).
- Single-item removal tests on minimality claims.
- Independent property monitor on `PropertyPreserving`.
- Schema validation of every emitted pack in CI — spike and engine outputs included (the R2/R3 spikes' pack dicts MUST be brought under this schema or labeled non-conformant fixtures).

## Rejected alternatives

- **One "relevance score" per item.** Rejected: conflates checked guarantees with heuristics — the precise failure INV-007 exists to prevent.
- **Token-budget enforcement.** Rejected: model-relative (see RFC 0027).
- **Mutable packs updated in place.** Rejected: breaks INV-009 and caching.

## Open questions

- Ranking-function choice and its evaluation (research/32; kill criterion applies).
- Cross-pack deduplication for multi-agent fan-out.

## Acceptance

The synthetic 200-event case plus at least three non-synthetic failures (real exploration output, not hand-built traces) compile to packs whose guarantees all pass their checkers; guarantee-violation mutants (drop a core event, reorder, relabel heuristic as preserved) are rejected; omission manifests reconcile exactly against the unsliced graph.
