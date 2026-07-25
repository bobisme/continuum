# ADR 0043: Repair Transactions

## Status
Accepted.

## Context
A patch plus passing tests cannot establish that a concurrency repair preserves intent or generalizes beyond one trace.

## Decision
Repairs are versioned transactions containing base failure, hypothesis, changes, semantic/intent diff, exact replay, neighboring exploration, mutation challenge, proof impact, unknowns, and promotion receipt.

## Consequences
- autonomous repair gains a hard acceptance boundary;
- evaluation can be resumed and reviewed;
- intent revisions are separate transaction class;
- promotion policy becomes auditable.

## Evidence required
Overfit repairs, property-gaming repairs, and genuine repairs in benchmark suite.
