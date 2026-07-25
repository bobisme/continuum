# A Compositional Algebra of Models, Refinements, and Certificates

## Motivation

Large systems cannot be verified as one flat transition graph. Continuum needs a principled way to compose components, assumptions, observers, refinement edges, and evidence without turning the architecture into ad hoc glue.

## Candidate mathematical structure

Treat:

- models/components as objects;
- refinements as vertical morphisms;
- interface compositions or wiring as horizontal morphisms;
- commuting preservation arguments as 2-cells.

This suggests a double-category or bicategorical organization. The user need never see the terminology; the implementation benefits from explicit laws:

- identity refinement;
- refinement composition;
- monotonicity under compatible composition;
- assumption discharge;
- observer projection compatibility;
- certificate composition.

## Practical artifact

A refinement graph edge contains:

```text
source model
 target model
 state/event relation
 assumptions guaranteed/required
 hidden actions
 observer mapping
 certificate type
 semantic epoch
```

A composition checker verifies that adjacent edges agree on interfaces and that assumptions are discharged.

## Assume-guarantee rule

For components `A` and `B`:

```text
Environment(B) ⊨ Assume(A)
Environment(A) ⊨ Assume(B)
A ⊨ Guarantee(A)
B ⊨ Guarantee(B)
────────────────────────────
A || B ⊨ GlobalProperty
```

The exact rule depends on trace, event-structure, or game semantics. Continuum should encode the chosen rule explicitly rather than use “compositional” as an adjective.

## Certificate algebra

Certificates should compose without rebuilding a monolithic proof:

- invariant certificates combine through shared interface invariants;
- refinement receipts compose transitively;
- assumption-game strategies compose with environment monitors;
- solver theorems become lemmas in Lean;
- local production conformance can glue into a global witness when overlaps agree.

## Sheaf connection

The local-to-global gluing problem resembles sheaf consistency: local sections over overlapping subsystems must agree on intersections. A failed gluing attempt may yield a minimal incompatible cover, useful for diagnosis.

## Promotion criterion

Adopt the algebra only where it produces executable validation rules, smaller proofs, or better failure localization. Avoid exposing category-theoretic vocabulary in ordinary UX unless it directly clarifies a problem.

## Primary references

- three-dimensional refinement algebra and proof/compilation composition: [S85];
- local-to-global/sheaf consistency research: [S37]–[S39];
- modular decidable verification and compositional protocol reasoning: [S27], [S31].
