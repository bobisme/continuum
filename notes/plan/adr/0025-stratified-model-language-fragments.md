# ADR-0025: Stratify the model language into declared semantic fragments

**Status:** Accepted  
**Date:** 2026-07-24

## Context

TLA+ permits highly expressive mathematical specifications, while executable model checking requires finiteness or decidable symbolic fragments. A typed Rust-like language can accidentally hide this boundary until an engine fails or silently bounds an unbounded value.

The TLA+ examples corpus spans finite enumeration, symbolic Apalache models, TLAPS proofs, temporal properties, probability, refinement, and procedural PlusCal.

## Decision

CML has one surface syntax and typed semantic core, but every definition is assigned capabilities/effects from explicit fragments:

- `Finite`: exactly enumerable values and transitions;
- `Symbolic`: solver-representable values and relations;
- `Temporal`: infinite-behavior properties and fairness;
- `Probabilistic`: probability distributions, MDPs, games, rewards;
- `Theorem`: propositions delegated to Lean;
- `Runtime`: concrete effects mapped to asupersync/domain packs.

Cross-fragment use is checked. Examples:

- a `Finite` action cannot call an opaque runtime function;
- a symbolic set cannot be enumerated without a finite model assignment;
- probabilistic and adversarial nondeterminism remain distinct;
- theorem-only choice is not given an arbitrary executable witness;
- liveness claims identify the temporal/fairness fragment used.

## Consequences

Users see why a model can be simulated but not exhaustively checked, or proved but not executed. Engines can select only sound encodings. Corpus coverage maps directly to fragment completeness.

The language is slightly more explicit than TLA+, intentionally trading convenience for truthful assurance.

## Rejected alternatives

- unrestricted language with runtime failures;
- silently finite integers/sets;
- separate unrelated languages per backend;
- require every model to be executable Rust.
