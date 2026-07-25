# ADR 0046: Semantic and Intent Diff Gates

## Status
Accepted.

## Context
Text diff cannot reveal changes to reachable behavior, observers, assumptions, proof dependencies, or assurance.

## Decision
Every review/repair computes layered semantic diff. Protected intent changes, proof-policy downgrades, new opaque effects, and reduced coverage are explicit policy inputs. Unsupported comparisons return Unknown.

## Consequences
- implication/equivalence engines and correspondence provenance are needed;
- review becomes more informative;
- automatic decisions are limited to supported fragments;
- Forge can deduplicate semantically equivalent candidates.

## Evidence required
Known semantic-equivalence/change corpus and adversarial review mutations.
