# ADR-0022: Use Lean 4 as metatheory and certificate authority

**Status:** Accepted  
**Date:** 2026-07-24

## Context

Continuum will contain complex, parallel, optimized search engines. Proving the entire Rust implementation correct before it becomes useful is unrealistic; trusting every optimization defeats the assurance goal. Several planned claims—DPOR preservation, symmetry quotienting, stuttering refinement, liveness certificates, cancellation progress, solver encodings—are mathematical theorems rather than testing problems.

Lean 4 provides a small kernel, executable reflection, a strong mathematical ecosystem, and emerging evidence that very large SAT and pseudo-Boolean certificates can be imported through verified checkers without constructing enormous explicit proof terms.

## Decision

Lean 4 is the normative home for Continuum metatheory and high-assurance certificate import.

Lean is used to formalize:

- transition, behavior, observation, and refinement semantics;
- soundness of certificate families;
- preservation theorems for reductions and abstractions;
- cancellation and obligation calculus;
- verified encodings from CIR/model fragments to SAT, PB, SMT, or graph obligations;
- key theorems corresponding to proof-bearing corpus examples.

Lean is **not** placed on the per-transition hot path. Rust engines search aggressively and emit evidence. A small native checker validates evidence for routine use; Lean reflection can validate the same artifact and produce a theorem for high-assurance workflows.

The core metatheory starts on Lean core/Std. Mathlib is a separately versioned dependency for research developments requiring advanced algebra, topology, probability, order theory, or category theory.

## Consequences

Continuum gains a principled trust story without requiring a verified optimizer. It also incurs a serious proof-engineering program and version-management cost.

The Rust and Lean semantics must not be maintained as informal twins. Serialization schemas, executable test vectors, generated lemmas, and differential interpretation are required.

## Rejected alternatives

1. **Trust Rust unit tests.** Insufficient for semantic reduction soundness.
2. **Verify everything in Rust with Verus only.** Valuable for code-level obligations but weaker as a general metatheory and theorem interchange.
3. **Make Lean the implementation language.** Conflicts with the Rust-native runtime/performance/product goal.
4. **Defer formalization until after 1.0.** Allows architecture to ossify around unprovable interfaces.

## Gates

- G0: core transition/refinement/certificate theorems build with no `sorry`.
- G1: one Rust closure certificate is imported and checked in Lean.
- G2: observer-indexed reduction theorem covers the baseline DPOR fragment.
- G3: fairness/lasso certificate theorem supports corpus liveness cases.
- G4: asupersync cancellation primitives have formal contracts linked to emitted events.
