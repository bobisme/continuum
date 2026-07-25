# ADR-0029: Target semantic equivalence, not a TLA+ source clone

**Status:** Accepted  
**Date:** 2026-07-24

## Context

The TLA+ corpus is a critical benchmark, but reproducing every parser, module-system, proof-language, TLC override, and historical edge case would consume the project and constrain better language design.

Users need the ability to express and verify the same systems and properties. They do not necessarily need every file to run unchanged.

## Decision

Continuum 1.0 requires native semantic equivalents for the pinned corpus. Source compatibility is an independent interoperability lane.

The optional TLA frontend proceeds in stages:

1. SANY/TLC oracle export in the Tribunal;
2. import of a normalized semantic IR;
3. native parser/elaborator for the executable subset;
4. diagnostics and source mapping;
5. only then, broader compatibility.

Java or upstream tools are never silently invoked by release binaries.

## Consequences

The model language can be typed, fragment-aware, and designed for refinement into Rust. Corpus parity still prevents semantic retreat.

Users with existing TLA+ assets receive a migration path, but the product's core value does not wait on full language emulation.
