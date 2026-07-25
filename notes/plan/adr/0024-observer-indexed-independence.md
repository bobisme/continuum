# ADR-0024: Index independence by observation and property contracts

**Status:** Accepted for baseline design; theorem work required  
**Date:** 2026-07-24

## Context

Two events can commute in concrete state while differing in audit order, fairness obligations, timing, durability, information flow, or a refinement observer. A global event-kind conflict table is either unsound or needlessly conservative.

Context-sensitive independence research shows that observers can enable exponential reductions. Continuum additionally has explicit views, obligations, and lifecycle events that must participate in the definition.

## Decision

Independence is parameterized by an `ObservationContract` containing:

- state/view abstraction;
- visible event alphabet;
- active safety/temporal/hyperproperties;
- fairness monitors;
- obligation and cancellation observations;
- timing and durability sensitivity;
- fault semantics.

An independence witness for events `e` and `f` at configuration `C` must establish:

1. both orders are enabled or both disabled as required;
2. both orders reach states equivalent under the active view;
3. visible observations and obligation flow agree;
4. neither order changes fairness eligibility incorrectly;
5. future enabledness is preserved for the supported property fragment.

Observer refinement induces a monotonicity law: a finer observer permits no more independence than a coarser observer. This law is formalized in Lean and exploited for cache reuse across property sets.

## Consequences

Reduction becomes property-aware and potentially far stronger. Cache keys must include the observer contract hash. Adding a property can invalidate prior independence witnesses.

The initial implementation remains conservative: static resource conflicts plus checked dynamic witnesses. Research engines may infer stronger relations but cannot raise assurance without certificates.

## Kill criterion

If observer-indexed analysis costs more than the schedules it removes across the corpus, retain only the formal contract and use coarse conservative observers by default.
