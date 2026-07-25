# ADR-0020: Gate speculative mathematics behind falsifiable research programs

**Status:** Accepted  
**Date:** 2026-07-24  
**Decision owners:** Continuum architecture group

## Context

Higher-dimensional automata, directed topology, sheaves, cohomology, and persistent homology may unlock real advances—or become decorative complexity.

## Decision

Experimental engines are separately named, benchmarked, and assurance-capped. Each has a hypothesis, baseline, threshold, and kill criterion. No public soundness claim relies on them before independent validation.

## Consequences

Continuum can pursue frontier research without contaminating the product or credibility. Negative results are acceptable outputs.

## Alternatives considered

1. Avoid speculative work: leaves boundary-pushing potential unexplored.
2. Make it core architecture immediately: reckless.
3. Use mathematical vocabulary only for marketing: explicitly rejected.

## Validation and rollback

Research scorecards in `docs/06_RESEARCH_AGENDA.md` and `docs/07_BENCHMARKS_AND_EVALUATION.md` govern promotion.
