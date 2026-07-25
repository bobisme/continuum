# ADR-0030: Generalize exploration results with typed semiring analyses

**Status:** Experimental  
**Date:** 2026-07-24

## Context

The same transition structure supports many analyses:

- Boolean reachability;
- shortest counterexample length;
- number of causal classes;
- accumulated cost or latency;
- probability bounds;
- provenance explaining which faults/assumptions contribute;
- reliability or risk scores.

Implementing each as unrelated traversal logic duplicates work and loses algebraic structure. Weighted-automata and provenance-semiring theory suggest a unifying formulation.

## Decision

The semantic graph APIs permit analyses parameterized by a typed algebra when its laws and interpretation are declared:

- Boolean semiring for reachability;
- tropical semiring for shortest/least-cost witnesses;
- natural-number or generating-function semirings for counting;
- probability/expectation structures for probabilistic fragments;
- provenance polynomials over event/fault labels;
- product semirings for simultaneous metrics.

The assurance result states the algebra and required conditions. Quantitative probability is not treated as an ordinary commutative semiring when nondeterminism or scheduler adversaries require MDP/game semantics.

## Consequences

This can unify algorithms and produce richer explanations, but algebraic elegance must not erase semantic distinctions. The feature remains experimental until it produces measurable implementation simplification or novel analysis wins.

## Kill criteria

Reject the abstraction if it introduces dynamic dispatch in hot loops, obscures numerical error, conflates probability with nondeterminism, or fails to simplify at least three concrete analyses.
