# Glossary

**Abstract interpretation** — sound approximation framework relating concrete and abstract domains, often through Galois connections.

**Action** — a relation between pre-state, parameters, and post-state.

**Adequate order** — ordering used to choose cutoffs in unfolding/complete-prefix construction.

**Assurance claim** — typed machine-readable statement of what was established and under which assumptions.

**Causal Intermediate Representation (CIR)** — Continuum's versioned representation of semantic events, causality, conflict, footprints, observations, and evidence.

**Certificate** — independently checkable evidence for a verification result.

**Configuration** — a conflict-free, causally closed set of events.

**Conflict** — relation indicating events cannot coexist in one execution.

**Crashpack** — deterministic reproduction artifact for a violation or notable execution.

**Domain pack** — semantics and handlers for a family of effects such as storage or networking.

**DPOR** — dynamic partial-order reduction.

**Effect phase** — lifecycle stage such as proposed, reserved, committed, or aborted.

**Event structure** — true-concurrency model with events, causality, and conflict.

**Fairness** — assumption restricting infinite executions, such as weak fairness of an action.

**Fidelity profile** — explicit statement relating a domain pack to host behavior and listing exclusions.

**Footprint** — typed semantic resources read, written, transferred, or observed by an event.

**Higher-dimensional automaton (HDA)** — automaton where higher-dimensional cells represent concurrent execution.

**Hyperproperty** — property of sets/tuples of traces, not individual traces.

**Inductive invariant** — predicate true initially, preserved by every step, and implying a safety property.

**Interval pomset** — partially ordered multiset of events with interval-order structure.

**Linearization** — total order consistent with a partial order.

**Model scope** — finite domains and bounds used for a verification run.

**Observation alphabet** — events/state visible at a refinement boundary.

**Obligation** — linear semantic responsibility that must be owned, transferred, or discharged.

**Pack axiom** — external guarantee assumed rather than proved by Continuum.

**PDR/IC3** — property-directed reachability algorithms that infer inductive invariants.

**PINS** — partitioned next-state interface separating language semantics from analysis algorithms.

**Prime event structure** — event structure with causality and hereditary conflict.

**Progressive refinement** — refinement with a well-founded progress condition preventing infinite invisible stuttering.

**Refinement view** — abstraction from a concrete configuration/system to an abstract model.

**Semantic epoch** — identifier for normative semantics used by models, traces, and certificates.

**Stuttering** — concrete step with no abstract-state change.

**Strong observational refinement** — refinement strong enough to preserve selected hyperproperties/scheduler-sensitive behavior.

**Tribunal** — differential, mutation, benchmark, and certificate infrastructure used to substantiate claims.

**True concurrency** — semantics representing concurrent events without choosing arbitrary interleavings.

**WSTS** — well-structured transition system, an infinite-state system ordered by a well-quasi-order enabling decidability of some coverability questions.

**Zoomable refinement** — multiple related abstraction grains used selectively by component or verification goal.
