# ADR 0051: Assurance Envelope Instead of Verified Badge

## Status
Accepted.

## Context
A scalar “verified” status hides bounds, faults, fairness, observer, weak-memory, proof, and unknown dimensions.

## Decision
Every verdict carries a structured assurance envelope. UI may summarize but cannot omit material dimensions. Promotion policy specifies required envelope.

## Consequences
- results become comparable without false total ordering;
- CI policies are explicit;
- users must learn a small assurance vocabulary;
- Inconclusive and Unsupported remain visible.

## Evidence required
Schema validation, UI comprehension studies, and policy tests.
