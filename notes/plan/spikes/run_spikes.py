from __future__ import annotations

import json
from dataclasses import asdict, is_dataclass
from pathlib import Path
from typing import Any

from models import DieHard, DiningPhilosophers, check_stuttering_refinement
from observer_independence import experiment as observer_experiment
from advanced_spikes import experiment as advanced_experiment
from reference_kernel import check_closure_certificate, explore, make_closure_certificate

ROOT = Path(__file__).resolve().parent
RESULTS = ROOT / "results"
RESULTS.mkdir(exist_ok=True)


def encode(value: Any) -> Any:
    if is_dataclass(value):
        return {k: encode(v) for k, v in asdict(value).items()}
    if isinstance(value, tuple):
        return [encode(v) for v in value]
    if hasattr(value, "name") and hasattr(value, "value"):
        return value.name
    return value


def trace_json(trace):
    return [
        {
            "source": encode(step.source),
            "action": step.action,
            "target": encode(step.target),
        }
        for step in trace
    ]


def main() -> None:
    diehard = DieHard()
    dh = explore(diehard)
    solved = dh.first_state(lambda s: not diehard.not_solved(s))
    assert solved is not None
    dh_cert = make_closure_certificate(diehard, dh)
    dh_cert_ok = check_closure_certificate(diehard, dh_cert, invariant=diehard.type_ok)

    dining = DiningPhilosophers(5)
    dp = explore(dining, max_states=500_000)
    deadlock = dp.first_state(dining.is_deadlock)
    assert deadlock is not None
    dp_cert = make_closure_certificate(dining, dp)
    dp_cert_ok = check_closure_certificate(dining, dp_cert, invariant=dining.type_ok)

    refinement_ok, refinement_errors = check_stuttering_refinement()

    payload = {
        "schema": "continuum-spike-results/v1",
        "diehard": {
            "states": len(dh.states),
            "edges": len(dh.edges),
            "shortest_solution_depth": dh.depth[solved],
            "solution_state": encode(solved),
            "shortest_solution": trace_json(dh.trace_to(solved)),
            "type_closure_certificate_valid": dh_cert_ok.valid,
            "certificate_errors": list(dh_cert_ok.errors),
        },
        "dining_philosophers_5": {
            "states": len(dp.states),
            "edges": len(dp.edges),
            "shortest_deadlock_depth": dp.depth[deadlock],
            "deadlock_state": encode(deadlock),
            "shortest_deadlock": trace_json(dp.trace_to(deadlock)),
            "type_closure_certificate_valid": dp_cert_ok.valid,
            "certificate_errors": list(dp_cert_ok.errors),
        },
        "stuttering_refinement": {
            "valid": refinement_ok,
            "errors": refinement_errors,
            "abstract_model": "atomic register write",
            "concrete_model": "reserve/commit/abort register",
        },
        "observer_indexed_independence": observer_experiment(),
        "advanced": advanced_experiment(),
    }

    (RESULTS / "spike-results.json").write_text(json.dumps(payload, indent=2) + "\n")

    report = f"""# Revision-2 Executable Spike Report

**Execution environment:** Python reference implementation, exact state identity, deterministic BFS.  
**Purpose:** falsify architectural assumptions before optimized Rust or Lean implementations exist.  
**Status:** executable spike, not a production implementation or formal proof.

## Die Hard parity seed

- Reachable states: **{payload['diehard']['states']}**
- Enumerated transitions: **{payload['diehard']['edges']}**
- Shortest state with four gallons in the large jug: depth **{payload['diehard']['shortest_solution_depth']}**
- Exact closure/type certificate accepted by independent checker: **{payload['diehard']['type_closure_certificate_valid']}**

The transition relation is a direct semantic port of the six actions in the TLA+ Examples `DieHard.tla`. This establishes the shape of the future corpus harness: a native model, a pinned source model, exact reachable-graph facts, expected failing property, and a replayable shortest witness.

## Dining philosophers

- Reachable states: **{payload['dining_philosophers_5']['states']}**
- Enumerated transitions: **{payload['dining_philosophers_5']['edges']}**
- Shortest deadlock depth: **{payload['dining_philosophers_5']['shortest_deadlock_depth']}**
- Exact closure/type certificate accepted: **{payload['dining_philosophers_5']['type_closure_certificate_valid']}**

This confirms that the reference kernel can distinguish ordinary invariant closure from deadlock reachability and produce a shortest concrete schedule.

## Stuttering refinement

A concrete register implements each write as `Reserve(v)` followed by `Commit`, with `Abort` available before commit. The abstraction forgets the pending reservation. Every concrete transition either:

1. preserves the abstract state; or
2. corresponds to an abstract atomic write.

Finite exhaustive check result: **{payload['stuttering_refinement']['valid']}**.

## Observer-indexed independence

Results:

```json
{json.dumps(payload['observer_indexed_independence'], indent=2)}
```

The key finding is intentional: `write_x` and `write_y` commute for state observers but not for an observer that records audit order. Independence cannot be a global property of event kinds; it must be indexed by the property/view/observer contract.

## Additional revision-2 experiments

### Cyclic symmetry

- Raw dining-philosopher states: **{payload['advanced']['cyclic_symmetry']['raw_states']}**
- Quotient states under process rotation: **{payload['advanced']['cyclic_symmetry']['quotient_states']}**
- Reduction factor: **{payload['advanced']['cyclic_symmetry']['reduction_factor']:.2f}×**
- Exhaustive transition-automorphism check: **{payload['advanced']['cyclic_symmetry']['automorphism_check_valid']}**

This is not yet a proof of symmetry reduction. It establishes the witness shape: a group action, canonical representative, and independently checkable automorphism obligation.

### Fair-lasso semantics

The same infinite `Wait` cycle is a raw liveness counterexample but is rejected under weak fairness of `Complete`; when `Complete` is genuinely unavailable, the cycle becomes a valid fair counterexample. This demonstrates why fairness must be attached to named actions and checked on the cycle rather than treated as a scheduler slogan.

### Environment-assumption synthesis

A tiny turn-based safety game synthesizes the maximal-permissive contract required by `Ack ⇒ Durable`. The only forbidden environment move is `AckBeforeSync`; after removing it, the initial state enters the safety-winning region. This is the seed for deriving domain-pack contracts and production monitors from games.

### Nominal canonicalization

Two traces using unrelated dynamically allocated identifiers canonicalize identically, while a causally different allocation/send order does not. This supports an orbit-finite lane for request IDs and other fresh names without accidentally erasing causal distinctions.

### Semiring-valued search

The Die Hard graph is evaluated over a product of the tropical and natural-number semirings, yielding shortest distance **{payload['advanced']['semiring_valued_search']['shortest_solution_distance']}** and **{payload['advanced']['semiring_valued_search']['number_of_shortest_labeled_witnesses']}** shortest labeled witnesses in one algebraic traversal.

## What this spike does not establish

- correctness of a Rust implementation;
- correctness of the planned model language;
- soundness or completeness of DPOR;
- temporal/liveness preservation;
- TLA+ semantic parity beyond the hand-ported Die Hard transition system;
- any Lean theorem.

Those become explicit G0/G1 gates rather than prose claims.
"""
    (ROOT / "SPIKE_REPORT.md").write_text(report)


if __name__ == "__main__":
    main()
