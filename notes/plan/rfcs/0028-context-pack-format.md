# RFC 0028: Context Pack Format and Compiler

## Status
Draft.

## Artifact
A Context Pack contains target question, snapshot/intent, verdict/assurance, selected graph nodes/edges, state deltas, source/model/proof references, causal core, assumptions, counterfactuals, omissions, expansion queries, and replay handles.

## Selection
Compiler starts from property/evidence roots, computes causal/proof/source slices, enforces closure for claimed guarantees, ranks optional context by utility, and packs within budget.

## Guarantees
Enum: `ReplayPreserving`, `PropertyPreserving`, `ProofRelevant`, `HeuristicRelevant`. Multiple guarantees may apply. Heuristic context is visually/structurally distinct.

## Omission manifest
Counts omitted nodes by kind/relation, redactions, truncation reasons, and handles/queries for expansion.

## Validation
Replay core, independent monitor, single-node removal tests, checksum, and schema validation.
