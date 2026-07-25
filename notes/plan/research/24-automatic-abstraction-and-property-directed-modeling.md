# Automatic Abstraction and Property-Directed Modeling

## Goal

Reduce the cost of creating and maintaining abstraction maps without allowing an agent or heuristic to invent a model that merely agrees with observed tests.

## Inputs

- concrete CIR executions;
- Rust types and effect signatures;
- candidate abstract state fields;
- active properties/observers;
- counterexamples and proof failures;
- existing model/refinement graph;
- corpus-derived modeling patterns.

## Candidate techniques

### Predicate abstraction

Infer predicates from guards, assertions, property atoms, and counterexample interpolants. Use CEGAR to refine when an abstract counterexample is spurious.

### Abstract interpretation

Define a Galois connection between concrete state/configuration domains and abstract domains. Derive sound abstract transformers rather than fitting traces.

### Interpretation reduction

Search for a small interpretation from a concrete transition system into a known abstract protocol template. Validate each candidate by SMT or exhaustive checking.

### Invariant inference

Use PDR/IC3, CHCs, symmetry-to-quantification, and agent-proposed lemmas. Every accepted invariant is checked independently.

### Causal feature selection

Use observer/property dependency and event footprints to identify which concrete facts can affect the property. This yields a conservative initial abstraction.

### Protocol-template mining

Recognize reusable patterns: quorum certificates, epochs, leases, two-phase commit, monotonic logs, ownership tokens, and retry loops. Templates propose—not assert—abstraction fields and invariants.

## Anti-overfitting rules

A candidate abstraction must survive:

- held-out schedules and faults;
- semantic mutants;
- neighboring causal classes;
- stronger bounds;
- differential reference execution;
- proof of simulation or a clearly bounded evidence class.

Agreement on a finite trace set is not refinement.

## Agent workflow

1. propose abstraction variables and mapping;
2. generate proof obligations;
3. classify failed obligations;
4. request missing ghost/history/prophecy state;
5. verify candidate under bounded and symbolic lanes;
6. emit semantic diff for human review;
7. promote only with receipt.

## Product outcome

The ideal UX is not “AI generated your formal model.” It is:

> Continuum found that the property depends only on epoch, durable index, and acknowledgement set; here is the proposed abstraction, the executions it collapses, the proof obligations, and the one obligation still open.

## Primary references

- invariant inference and proof slicing: [S116], [S120];
- protocol synthesis and interpretation reduction: [S117]–[S118];
- symmetry-to-quantification and automatic cutoff discovery: [S119], [S123];
- regular abstractions and ranking-function liveness: [S122], [S124];
- 2026 neuro-symbolic IC3 synthesis, treated as an untrusted proposal engine whose outputs require proof checking: [S125].
