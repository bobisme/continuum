# ADR-0023: Maintain a semantic triptych with independent paths

**Status:** Accepted  
**Date:** 2026-07-24

## Context

“One semantics” is often implemented as one shared library used by model, runtime, and checker. This improves consistency but creates circular assurance: a bug in shared code makes every layer agree.

Conversely, fully independent implementations drift and become unaffordable.

## Decision

Continuum distinguishes three authoritative artifacts:

1. **Model semantics:** mathematical transition/behavior definitions and native CML interpretation.
2. **Program semantics:** asupersync concrete execution and semantic event production.
3. **Proof semantics:** Lean definitions and certificate theorems.

They share versioned data contracts and generated fixtures, but not all evaluation code. Strong claims require at least two independent paths and, for proof-level assurance, a checked certificate under the Lean semantics.

The reference evaluator is simple, deterministic, exact, and optimization-hostile. The production Rust engine is optimized. Solver encodings are a third path. Corpus fixtures and generated bounded models compare all available paths.

## Consequences

The project must budget for differential testing and semantic change management. It cannot “fix” a disagreement by changing all three sides simultaneously without an explicit semantic decision record.

This architecture makes latent ambiguity visible early, which is exactly the point.

## Required independence matrix

| Claim | Minimum independent evidence |
|---|---|
| Counterexample | replay under reference semantics |
| Finite safety | native certificate checker + reference evaluator |
| Proof safety | Lean certificate theorem |
| Concrete refinement | program event trace + independent model/refinement checker |
| Solver proof | proof certificate + verified encoding/checker |
| Reduction soundness | Lean theorem + mutation/differential corpus |

## Rejected alternatives

- one shared evaluator everywhere;
- fully duplicated full-scale engines;
- “N-version agreement” without a theorem or evidence model;
- treating the upstream TLA+ oracle as the permanent semantic authority.
