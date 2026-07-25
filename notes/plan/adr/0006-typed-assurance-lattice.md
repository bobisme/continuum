# ADR-0006: Represent assurance as a multidimensional typed claim

**Status:** Accepted  
**Date:** 2026-07-24  
**Decision owners:** Continuum architecture group

## Context

“Passed” conflates sampling, bounded search, exhaustive finite checking, inductive proof, trace observation, and refinement. A single maturity number hides assumptions and TCB differences.

## Decision

Every result records semantic coverage, exploration class, property class, evidence class, implementation linkage, assumptions, exclusions, and trusted components. Human output is derived from this object.

## Consequences

Claims become honest and composable, but UI and CI must manage more metadata. Marketing simplicity is intentionally sacrificed.

## Alternatives considered

1. Linear assurance levels: too lossy.
2. Engine-specific output: impossible to compare or automate safely.
3. Prose caveats: not machine-checkable.

## Validation and rollback

Schemas and golden verdict examples are part of G0. Claim rendering is tested against the ledger; unsupported combinations are rejected.
