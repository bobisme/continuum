# ADR-0027: Add a nominal/orbit-finite lane for names and symmetry

**Status:** Accepted as research lane; not a baseline dependency  
**Date:** 2026-07-24

## Context

Distributed models contain process IDs, request IDs, session IDs, keys, fresh nonces, and dynamically allocated resources. Bounding each name domain creates artificial state explosion and weakens parameterized claims. Ordinary symmetry reduction handles fixed finite populations but not fresh-name generation elegantly.

Nominal-set and orbit-finite automata theory provides finite representations modulo permutations of atoms, including models with name allocation. It is a mature but underused body of semantics relevant to modern distributed systems.

## Decision

CML introduces an optional `atom` kind with explicit symmetry theory:

- equality atoms;
- ordered atoms where supported;
- fresh allocation and support tracking;
- orbit canonicalization by equality/order pattern;
- alpha-equivalent state identity;
- nominal automata or symbolic transitions for supported fragments.

The lane is used only when the model's operations are equivariant under the declared group action. Non-equivariant observations—hashing raw IDs, stable numeric ordering, external identity—break or refine the symmetry and must be declared.

## Consequences

Continuum may verify classes of dynamic-name systems without arbitrary ID bounds and may synthesize stronger parameterized invariants. The implementation and proof burden is significant.

## Falsification gates

- demonstrate an orbit-finite model with fresh request/session IDs that finite symmetry cannot scale to;
- prove canonicalization/equivariance soundness in Lean;
- compare against standard symmetry plus small-model cutoffs;
- delete or quarantine the lane if it provides no decisive corpus/real-system win.
