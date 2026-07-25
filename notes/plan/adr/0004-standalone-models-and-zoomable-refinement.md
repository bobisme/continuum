# ADR-0004: Support standalone models and zoomable refinement views

**Status:** Accepted  
**Date:** 2026-07-24  
**Decision owners:** Continuum architecture group

## Context

Executing real code under faults is not a replacement for TLA+ if users cannot model before implementation or abstract away operational detail. Conversely, a separate model without refinement drifts.

## Decision

Continuum supports standalone typed relational models and multiple views from service semantics through protocol, operational model, implementation, and production observation. Adjacent views carry explicit refinement obligations and can be mixed by component.

## Consequences

The language and refinement subsystem become major workstreams. This complexity is necessary to replace abstract modeling rather than only DST infrastructure.

## Alternatives considered

1. Rust-only executable models: too concrete.
2. One abstract model plus code tests: retains the model/code gap.
3. Automatic extraction of the model from arbitrary Rust: unrealistic and tends to reproduce implementation detail.

## Validation and rollback

G2 requires model-before-code plus refinement of a real implementation. If mapping cost remains excessive, prioritize explicit views and multi-grain summaries over speculative automatic extraction.
