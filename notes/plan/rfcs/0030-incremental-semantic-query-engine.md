# RFC 0030: Incremental Semantic Query Engine

## Status
Draft.

## Query key
Function ID/version, canonical inputs, semantic/proof epochs, strategy configuration. Budget belongs to execution identity unless output semantics depend on it.

## Edge classes
Exact, Validated, Conservative, Experimental. Every output records dependencies and evidence supporting class.

## Persistence
CAS stores outputs; query index maps keys. Transactions commit output then index. Crash leaves unreachable content eligible for GC, not stale index.

## Clean Tribunal
Sampling policy and promotion policy trigger clean recomputation. Mismatch creates evidence node, quarantines edge/query implementation version, and runs minimizer.

## API
`query.explain_reuse`, `query.explain_invalidation`, `query.clean_compare`.

## Acceptance
Random edit traces and intentional dependency bugs across model, property, correspondence, proof, and Context Pack queries.
