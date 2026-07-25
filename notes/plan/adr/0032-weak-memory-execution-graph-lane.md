# ADR 0032: Weak Memory Is a Separate Execution-Graph Lane

## Status

Accepted as architecture; non-SC semantics remain gated.

## Context

Loom-style schedule exploration and distributed-system simulation do not fully model Rust/C11 weak memory. Combining every atomic choice with every packet/crash schedule is generally intractable.

## Decision

Continuum models weak memory with a dedicated execution-graph fragment and verifies local components against atomic contracts. Distributed models consume those contracts. The initial supported semantics are SC and explicit synchronization; Rust/C11 axiomatic models are solver-backed extensions.

## Consequences

- assurance states the memory model explicitly;
- local and distributed verification compose hierarchically;
- Loom traces may seed exploration but do not define semantics;
- runtime refinement of lock-free components requires a weak-memory receipt or an explicit SC assumption.
