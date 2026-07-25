# ADR 0047: Forge Sandboxing and Non-Vacuity

## Status
Accepted.

## Context
Protocol synthesis and agents can satisfy safety by disabling behavior or altering assumptions. Generated code is untrusted.

## Decision
Forge runs inside a fixed Intent Contract and typed sketch. Positive/progress scenarios and non-vacuity claims are hard constraints. Candidates are sandboxed and independently verified before archive/promotion.

## Consequences
- synthesis tasks require richer intent;
- candidate generators remain outside TCB;
- unrealizability and unknown are distinguished;
- materialization enters a design/repair transaction.

## Evidence required
Never-ack/trivial-property mutants, hidden variants, and independent candidate checks.
