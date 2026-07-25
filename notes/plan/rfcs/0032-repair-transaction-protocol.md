# RFC 0032: Repair Transaction Protocol

## Status
Draft.

## Operations
`begin`, `apply`, `attach`, `evaluate`, `resume`, `review`, `promote`, `reject`.

## Immutability
Each operation returns a new transaction version. Base failure/snapshot/intent cannot change.

## Evaluation stages
Base replay, application/seal, diff, exact regression, neighborhood, mutation, proof/refinement, clean comparison, performance/security, policy.

## Policy
Declarative requirements by property/branch/project. Promotion endpoint checks policy now; it never trusts a cached client verdict.

## Receipt
Canonical signed/checkable structure referencing all evidence and unknowns.

## Acceptance
Genuine, overfit, intent-gaming, instrumentation-gaming, stale-proof, and availability-collapse repairs.
