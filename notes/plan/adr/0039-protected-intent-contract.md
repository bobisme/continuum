# ADR 0039: Protected Intent Contract

## Status
Accepted.

## Context
Formal reward hacking can make all checks pass by changing the question: weaken a property, strengthen assumptions, shrink bounds, remove faults, or hide observations.

## Decision
Properties, assumptions, observers, bounds, faults, fairness, trust, assurance, and non-vacuity form a content-addressed Intent Contract. Ordinary repair operations cannot mutate protected fields. Revisions require semantic diff and policy authorization.

## Consequences
- intent becomes an input to all evidence identities;
- legitimate requirement changes are more explicit;
- implication/order checking is needed for supported fragments;
- unknown semantic relation blocks automatic promotion.

## Evidence required
Adversarial intent-gaming suite and independent policy tests.
