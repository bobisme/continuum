# Research 16: Causal Abstract Interpretation

## Motivation

Classical abstract interpretation approximates sets of program states using lattices and sound transfer functions. Distributed executions also have causal structure. Collapsing them immediately to global states can lose independence and create enormous cross-products.

## Proposal

Define abstract domains over **configurations**—causally closed sets of events—rather than only total states.

Let:

- `C` be concrete event configurations;
- `A` be an abstract domain;
- `α : P(C) → A`, `γ : A → P(C)` form a Galois connection or sound approximation;
- transfer adds enabled events modulo conflict/causality.

Abstract elements can summarize:

- which obligations may exist;
- quorum knowledge/support sets;
- durable/volatile phase frontiers;
- causal reachability of acknowledgements;
- message provenance;
- cancellation region topology;
- partial-order width/depth.

## Why causal domains may help

Two systems with the same global variable valuation can differ in causal history relevant to:

- which messages can still arrive;
- whether an acknowledgement depends on sync;
- which obligation owner must finalize;
- whether events are concurrent or ordered;
- production trace conformance.

Causal abstraction can retain exactly those facts without retaining every schedule.

## Product of domains

A Continuum analysis could use a reduced product:

```text
state interval/domain
× happens-before summary
× obligation linearity domain
× durability phase domain
× quorum knowledge domain
```

Reduction operators exchange facts between components.

## Widening and acceleration

Loops/retries generate unbounded event histories. Candidate widenings:

- collapse repeated independent event layers via Foata normal forms;
- summarize monotone message/knowledge accumulation;
- accelerate idempotent retry cycles;
- use well-quasi-order ideals for obligation/message multisets;
- widen causal intervals while preserving forbidden order patterns.

## Relation to partial-order reduction

POR chooses representative concrete executions. Causal abstract interpretation overapproximates families. They can compose:

1. POR reduces redundant orderings;
2. abstraction merges semantically similar configurations;
3. CEGAR refines when a counterexample is spurious.

The preservation story is harder than state-only CEGAR and must be formalized per observer/property.

## Falsifiable experiment

Use a retrying replicated write protocol where message multiplicity and causal provenance dominate. Compare:

- explicit global states;
- DPOR only;
- state abstract interpretation;
- causal abstract interpretation.

Success requires fewer states/configurations and fewer spurious counterexamples, not merely a novel formalism.
