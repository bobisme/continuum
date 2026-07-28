# Research 14: Nominal Sets and Orbit-Finite Verification

## Why revisit nominal mathematics

Modern systems continuously create names: request IDs, sessions, object keys, trace IDs, transaction IDs, leases, generations and nonces. Model checkers usually bound them to a tiny finite set. This creates duplicate states distinguished only by renaming and makes freshness awkward.

Nominal sets treat names as atoms acted on by permutations. Finiteness is replaced by **orbit-finiteness**: finitely many equivalence classes under renaming, each element having finite support.

## Relevant theory

- Nominal automata and coalgebraic semantics:
  https://arxiv.org/abs/2202.06546
- Nominal Büchi automata with name allocation and decidable language inclusion:
  https://doi.org/10.4230/LIPIcs.CONCUR.2021.4
- Alternating nominal automata with name allocation, LICS 2025:
  https://arxiv.org/abs/2408.03658
- Orbit-finite-dimensional vector spaces and weighted register automata:
  https://doi.org/10.46298/theoretics.24.13

## Continuum fragment

```text
type RequestId : atom[equality]
type Node : atom[equality]
fresh rid : RequestId
```

A state such as:

```text
pending = {r1 ↦ Waiting, r2 ↦ Done}
```

is represented by support and equality pattern, not concrete integers. Renaming `r1,r2` yields the same orbit.

## Equivariance obligation

A transition/property is eligible only if it commutes with atom permutations:

```text
step(π·s, π·s') ⇔ step(s,s')
property(π·s) ⇔ property(s)
```

Operations such as hashing raw IDs, comparing allocation order, leaking textual IDs, or using numeric arithmetic violate equality symmetry. Continuum must reject or refine the symmetry.

## Ordered atoms

Epochs and sequence numbers need order, not just equality. Data symmetries can model ordered atoms, but products may have more orbits and complexity rises. The lane should distinguish:

- equality atoms;
- total-order atoms;
- tree/path structured names;
- opaque externally observed names.

## Interaction with runtime

Concrete UUIDs map to abstract atoms through an allocation table in the refinement view. Production traces need only preserve equality/freshness relationships unless the property observes representation.

This could dramatically reduce traces containing thousands of unique request IDs.

## Interaction with parameterized verification

Orbit-finite representation and symmetry-to-quantification are complementary:

- nominal methods quotient names within a behavior;
- quantified invariants prove properties across unbounded populations.

A successful finite orbit model can supply invariant candidates and support patterns to IC3/PDR.

## First experiment

Model an idempotent request service with:

- unbounded fresh request IDs;
- retries and duplicate delivery;
- cancellation;
- deduplication table garbage collection;
- property: one committed result per request atom.

Compare:

1. concrete bounded IDs;
2. finite permutation symmetry;
3. nominal orbit exploration.

Promote only if nominal exploration changes the practical scaling frontier.

## Promotion and kill criteria (draft)

Draft pending lane-owner ratification. The quantitative promote/kill
pair for this lane lives in
[docs/31_FALSIFICATION_AND_KILL_CRITERIA.md](../docs/31_FALSIFICATION_AND_KILL_CRITERIA.md)
("Nominal/orbit-finite lane").

- **Baseline:** concrete bounded IDs and finite permutation symmetry on
  the idempotent-request-service experiment above (comparisons 1 and 2).
- **Promotion criterion:** per docs/31 — a real session/request
  protocol scales unboundedly with tractable orbit growth and
  Lean-proved equivariance; i.e., nominal exploration changes the
  practical scaling frontier rather than matching bounded runs.
- **Kill condition:** per docs/31 — models routinely violate the
  equivariance obligation (hashing raw IDs, allocation-order
  comparisons, arithmetic on names), or orbit explosion matches
  concrete bounding.
