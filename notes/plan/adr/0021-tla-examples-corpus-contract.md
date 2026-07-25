# ADR-0021: Make the TLA+ Examples corpus a release contract

**Status:** Accepted  
**Date:** 2026-07-24  
**Decision owners:** Continuum architecture and semantics groups

## Context

A project claiming to replace the practical need for TLA+ can drift into verifying only the examples its own language makes convenient. Hand-selected demonstrations do not expose semantic blind spots around stuttering, fairness, symmetry, model configuration, refinement, recursive operators, procedural lowering, or deliberately failing models.

The `tlaplus/Examples` repository explicitly serves both as an example library and as a corpus for testing TLA+ tools. At pinned commit `91c22ea537853196ed1e03e9ad91693ec37642de`, its README lists 80 CI-validated specification families and 39 additional in-tree or external examples.

## Decision

The 80 validated families are mandatory compatibility cases for Continuum 1.0. “Compatibility” means semantic equivalence at a declared parity level, not necessarily TLA+ source compatibility.

Every port receives:

- a pinned upstream source closure and toolchain;
- a native Continuum model;
- a state/action/temporal correspondence;
- expected model configurations and verdicts;
- differential and metamorphic tests;
- mutation tests demonstrating non-vacuous properties;
- Lean theorems or certificate imports when theorem parity is required;
- optional asupersync implementation refinement for selected cases.

Parity levels P0–P5 are normative in `corpus/tla-examples/PARITY_LEVELS.md`.

## Consequences

The corpus becomes an external force on language and engine design. Features are not accepted because they work on one flagship protocol; they are accepted when they survive a heterogeneous semantic workload.

This adds substantial scope. It also creates a clear stopping condition and prevents the project from becoming a permanent research prototype.

## Rejected alternatives

1. **Five showcase protocols.** Too easy to overfit.
2. **Only import TLA+ and run TLC.** Does not create native sovereignty or bridge to Rust.
3. **Line-by-line source translation.** Preserves syntax accidents rather than semantic intent.
4. **Raw state-count equality as the sole oracle.** Invalid under legitimate auxiliary-state and encoding differences.

## Evidence and gates

- Corpus inventory is machine-readable and pinned.
- Each passing case publishes its parity artifact and reproduction command.
- Continuum CI runs a fast subset on every commit and the complete Tribunal nightly.
- Upstream disagreements are minimized and classified.
- 1.0 cannot ship while any required row is `unsupported`, `unknown`, or `manual-only`.
