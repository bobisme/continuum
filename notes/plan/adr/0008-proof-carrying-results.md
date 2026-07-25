# ADR-0008: Require checkable certificates for strong success claims

**Status:** Accepted  
**Date:** 2026-07-24  
**Decision owners:** Continuum architecture group

## Context

A high-performance verifier will be large, parallel, and heuristic. Trusting its “safe” verdict makes the verifier part of the critical system. Counterexamples are easier to validate than successful exhaustion or induction.

## Decision

Continuum defines certificate families and a small independent `continuum-kernel`. Certified assurance requires the kernel to bind evidence to model, property, scope, assumptions, and semantic epoch.

## Consequences

The TCB shrinks and results become portable. Certificate engineering can be difficult, especially for POR, SMT theories, liveness, and probabilistic claims.

## Alternatives considered

1. Trust the Rust engine: rejected for high assurance.
2. N-version agreement only: useful but not proof.
3. Require a proof assistant for every run: too expensive initially.

## Validation and rollback

Start with counterexample replay and finite-state closure certificates. Solver/liveness/POR certificates remain lower assurance until their checker paths mature.
