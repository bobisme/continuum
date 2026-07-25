# ADR-0026: Require proof-producing reductions for strong assurance

**Status:** Accepted as target architecture  
**Date:** 2026-07-24

## Context

DPOR, symmetry, partial-order unfoldings, abstraction, slicing, and compositional summaries deliberately omit executions or state detail. Their bugs are dangerous because they can turn an unsafe system into a false pass.

## Decision

Every reduction has two modes:

1. **exploratory:** optimized, assurance-capped, may rely on tested algorithms;
2. **certifying:** emits a reduction witness checked independently.

Witness families include:

- commutation diamonds and source/backtrack-set obligations for DPOR;
- orbit representatives plus permutation witnesses for symmetry;
- covering/event-extension obligations for unfolding prefixes;
- Galois connection or simulation obligations for abstraction;
- rely/guarantee interface closure for composition;
- proof-graph dependencies for inductive slicing.

The certificate need not replay the entire search. It must be sufficient to establish the preservation theorem assumed by the result.

## Consequences

Certifying engines will initially be slower and support fewer optimizations. This is acceptable. Fast exploratory lanes find bugs; certifying lanes justify absence claims.

The project must resist “certificate” formats that merely serialize internal data without a small semantic checker.

## Promotion rule

A reduction may become the default for proof-level results only after:

- a Lean preservation theorem exists;
- the certificate checker is independently tested;
- adversarial malformed certificates are rejected;
- the TLA+ corpus and mutation suite show no semantic loss;
- performance beats unreduced or previously certified baselines on a defined benchmark class.
