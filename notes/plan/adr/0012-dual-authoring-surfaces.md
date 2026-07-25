# ADR-0012: Provide a standalone model language and a Rust integration surface

**Status:** Accepted  
**Date:** 2026-07-24  
**Decision owners:** Continuum architecture group

## Context

A single syntax cannot simultaneously optimize arbitrary mathematics and ordinary implementation integration. Rust-only modeling harms abstraction; a separate language alone harms linkage.

## Decision

`.ctm` is the abstract model surface. Rust macros/traits expose semantic actions and views in implementation code. Both lower to the same typed semantic AST/CIR.

## Consequences

Two frontends add work but avoid a false compromise. Normalized semantics rather than syntax is the compatibility contract.

## Alternatives considered

1. Quint only: viable bootstrap but makes Continuum dependent on an external evolving language and limits custom runtime semantics.
2. Rust DSL only: rejected for abstraction.
3. TLA+ frontend first: too much compatibility burden.

## Validation and rollback

Prototype syntax is explicitly unstable. User tests compare `.ctm` expressiveness and friction with Quint/TLA+ before stabilization.
