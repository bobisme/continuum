# Research 20: Rust Concurrency Verification Frontier, 2026

## Layering principle

No current tool spans abstract distributed protocol semantics, concrete async runtime schedules, unsafe Rust, memory models, liveness and production conformance. Continuum should integrate by proof boundaries, not pretend to dominate every layer.

## Concrete schedule/runtime tools

- Loom: controlled atomics/synchronization and schedule exploration.
- Shuttle: scalable randomized/deterministic schedule testing.
- asupersync Lab: deterministic runtime, cancellation/obligation semantics, virtual time, traces, DPOR-oriented infrastructure.
- Stateright: executable Rust state/actor models and consistency checks.

Continuum's distinctive role is abstract/refinement/proof integration above asupersync.

## Rust program verification

### Verus

Rust-like verification with SMT, linear ghost state and transition-system patterns. Useful for local functional/concurrency contracts and selected implementation components.

https://verus-lang.github.io/verus/

### Kani

Bit-precise bounded model checking for Rust code. Useful for finite unsafe/data-structure obligations and serialization/checker code.

https://model-checking.github.io/kani/

### Thrust

Prophecy-based refinement types for Rust, aimed at modular verification of ownership and mutation patterns.

https://doi.org/10.1145/3729333

### RustMC

Research on model checking compiled Rust concurrency and FFI behavior broadens coverage below source-level abstractions.

https://arxiv.org/abs/2502.06293

## Agentic proof work

Benchmarks such as VeruSAGE show growing interest in agents producing verified Rust proofs. Continuum can provide higher-level protocol goals and concrete counterexamples, while code verifiers discharge local obligations.

## Boundary contract

A Continuum result identifies layers:

```text
Protocol model: proved/checked
Runtime refinement: checked under controlled effects
Local component contract: imported theorem or assumption
Unsafe/FFI/memory model: verified by X or unverified
Production observation: valid/invalid/inconclusive
```

No green badge flattens these distinctions.

## Proposed interoperability

- generate Kani harnesses from finite action/domain constraints;
- generate Verus-style transition invariants for local state machines;
- consume code-verifier theorem receipts as domain-pack assumptions;
- use Loom for primitive implementations not fully controlled by asupersync;
- import Miri/sanitizer/fuzzer evidence as testing evidence, not proof.

## Challenge to the vision

“Same code in model and production” is only true inside controlled boundaries. Dependencies, FFI, allocator behavior, OS semantics, and hardware memory models remain contracts or separate verification tasks. Continuum succeeds by making these boundaries explicit and composable.
