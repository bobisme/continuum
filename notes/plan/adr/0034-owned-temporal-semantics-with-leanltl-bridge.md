# ADR 0034: Own the Temporal Semantics and Bridge to LeanLTL

## Status

Accepted.

## Context

LeanLTL offers valuable current Lean 4 infrastructure for finite and infinite linear temporal logic. Continuum nevertheless needs stable semantics tied to fairness, stuttering, actions, and proof receipts.

## Decision

Continuum defines a minimal temporal semantics in its Lean metatheory and proves bridges to LeanLTL where definitions align. LeanLTL may provide notation, automation, and reusable theorems but does not silently define Continuum product meaning.

## Consequences

- dependency upgrades cannot change verdict semantics without an epoch;
- equivalence theorems make reuse explicit;
- temporal proof work is not duplicated unnecessarily;
- unsupported semantic differences remain visible.
