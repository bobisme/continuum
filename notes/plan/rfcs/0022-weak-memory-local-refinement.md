# RFC 0022: Weak-Memory Local Refinement

## Summary

Specify a local component-verification lane based on execution graphs and refinement to atomic contracts.

## MVP semantics

- SC atomics;
- locks/channels as explicit synchronization;
- program order and reads-from;
- operation call/return events;
- linearization witness.

## Extended semantics

- modification order, from-read, synchronizes-with, happens-before;
- Relaxed/Acquire/Release/AcqRel/SeqCst;
- named Rust/compiler/hardware model;
- SAT/SMT candidate execution generation;
- proof-producing solver path.

## Composition

Once verified, the component exports an atomic summary used by distributed models. The receipt records model, bounds, memory semantics, and refinement witness.
