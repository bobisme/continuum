# Revision-2 Executable Spike Report

**Execution environment:** Python reference implementation, exact state identity, deterministic BFS.  
**Purpose:** falsify architectural assumptions before optimized Rust or Lean implementations exist.  
**Status:** executable spike, not a production implementation or formal proof.

## Die Hard parity seed

- Reachable states: **16**
- Enumerated transitions: **96**
- Shortest state with four gallons in the large jug: depth **6**
- Exact closure/type certificate accepted by independent checker: **True**

The transition relation is a direct semantic port of the six actions in the TLA+ Examples `DieHard.tla`. This establishes the shape of the future corpus harness: a native model, a pinned source model, exact reachable-graph facts, expected failing property, and a replayable shortest witness.

## Dining philosophers

- Reachable states: **573**
- Enumerated transitions: **2365**
- Shortest deadlock depth: **10**
- Exact closure/type certificate accepted: **True**

This confirms that the reference kernel can distinguish ordinary invariant closure from deadlock reachability and produce a shortest concrete schedule.

## Stuttering refinement

A concrete register implements each write as `Reserve(v)` followed by `Commit`, with `Abort` available before commit. The abstraction forgets the pending reservation. Every concrete transition either:

1. preserves the abstract state; or
2. corresponds to an abstract atomic write.

Finite exhaustive check result: **True**.

## Observer-indexed independence

Results:

```json
{
  "events": [
    "write_x",
    "write_y"
  ],
  "commutes_under": {
    "state_xy": true,
    "state_x": true,
    "audit_order": false
  },
  "observer_monotonicity_holds": true,
  "interpretation": "The same events are independent for state observers but dependent for an order-sensitive audit observer. Reduction must therefore be indexed by the active observation/property contract."
}
```

The key finding is intentional: `write_x` and `write_y` commute for state observers but not for an observer that records audit order. Independence cannot be a global property of event kinds; it must be indexed by the property/view/observer contract.

## Additional revision-2 experiments

### Cyclic symmetry

- Raw dining-philosopher states: **573**
- Quotient states under process rotation: **117**
- Reduction factor: **4.90×**
- Exhaustive transition-automorphism check: **True**

This is not yet a proof of symmetry reduction. It establishes the witness shape: a group action, canonical representative, and independently checkable automorphism obligation.

### Fair-lasso semantics

The same infinite `Wait` cycle is a raw liveness counterexample but is rejected under weak fairness of `Complete`; when `Complete` is genuinely unavailable, the cycle becomes a valid fair counterexample. This demonstrates why fairness must be attached to named actions and checked on the cycle rather than treated as a scheduler slogan.

### Environment-assumption synthesis

A tiny turn-based safety game synthesizes the maximal-permissive contract required by `Ack ⇒ Durable`. The only forbidden environment move is `AckBeforeSync`; after removing it, the initial state enters the safety-winning region. This is the seed for deriving domain-pack contracts and production monitors from games.

### Nominal canonicalization

Two traces using unrelated dynamically allocated identifiers canonicalize identically, while a causally different allocation/send order does not. This supports an orbit-finite lane for request IDs and other fresh names without accidentally erasing causal distinctions.

### Semiring-valued search

The Die Hard graph is evaluated over a product of the tropical and natural-number semirings, yielding shortest distance **6** and **1** shortest labeled witnesses in one algebraic traversal.

## What this spike does not establish

- correctness of a Rust implementation;
- correctness of the planned model language;
- soundness or completeness of DPOR;
- temporal/liveness preservation;
- TLA+ semantic parity beyond the hand-ported Die Hard transition system;
- any Lean theorem.

Those become explicit G0/G1 gates rather than prose claims.
