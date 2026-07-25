# ADR-0003: Prohibit ambient nondeterminism in verified cores

**Status:** Accepted  
**Date:** 2026-07-24  
**Decision owners:** Continuum architecture group

## Context

Deterministic testing and exhaustive exploration are unsound when code can bypass controlled time, entropy, scheduling, I/O, or process lifecycle. Discipline alone is insufficient across a dependency closure.

## Decision

Verified cores must obtain nondeterministic or externally observable effects through `Cx` or Continuum capabilities. Continuum adds lint, MIR-audit, runtime-trap, dependency-report, and production-coverage layers. Unsupported effects downgrade assurance or stop analysis.

## Consequences

Signatures become more explicit and migration may be intrusive. The payoff is replay, effect provenance, and a meaningful verification boundary.

## Alternatives considered

1. Best-effort mocking: rejected because hidden effects become silent omissions.
2. LD_PRELOAD/syscall interposition: useful for compatibility experiments, not a primary semantic foundation.
3. Whole-VM determinism: broad but expensive and opaque.

## Validation and rollback

G1 migration reports quantify remaining ambient effects. The rule may be scoped to declared verified cores; application shells can remain unverified with explicit boundaries.
