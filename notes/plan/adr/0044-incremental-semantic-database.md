# ADR 0044: Incremental Semantic Database with Incremental Parity Audit

(The audit was formerly called the clean-build Tribunal; "Tribunal" now refers exclusively to the ADR-0021 corpus oracle harness.)

## Status
Accepted.

## Context
Interactive verification requires reuse; unsound invalidation can produce stale green results.

## Decision
Represent derived computations as content-addressed semantic queries. Dependency/reuse edges are Exact, Validated, Conservative, or Experimental. Clean recomputation continuously audits strong incremental claims and quarantines mismatches.

## Consequences
- provenance-rich query graph;
- some interactive results remain provisional;
- promotion may require clean/validated lane;
- invalidation debugging is user-visible.

## Evidence required
Random edit sequences, corrupted-edge tests, clean differential results, and simplified Lean model.
