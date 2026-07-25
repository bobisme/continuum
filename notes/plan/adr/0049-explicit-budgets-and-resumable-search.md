# ADR 0049: Explicit Budgets and Resumable Search

## Status
Accepted.

## Context
Verification and synthesis can be unbounded. Silent timeouts and abandoned processes are poor human/agent interfaces.

## Decision
Every long task accepts typed resource budgets and returns terminal evidence or an explicit continuation. Continuations bind to exact inputs and support deterministic resume/fork under declared semantics.

## Consequences
- search engines need serializable committed frontier state;
- budget exhaustion is not pass/fail;
- resource effectiveness can be benchmarked;
- cancellation semantics become foundational.

## Evidence required
Resume equivalence, stale epoch rejection, crash recovery, and budget accounting tests.
