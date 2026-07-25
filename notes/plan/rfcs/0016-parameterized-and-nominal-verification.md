# RFC 0016: Parameterized, Symmetric, and Nominal Verification

**Status:** Proposed research/product boundary  
**Target gate:** G6

## Problem

Checking three nodes is useful bug finding, not a theorem for every cluster size. IDs and fresh names also cause artificial blowups when their concrete identities are irrelevant.

## Portfolio

### Finite symmetry

Declared finite groups act on states/actions. Canonical representatives and orbit sizes are exact. Certificate witnesses include permutations and canonicalization results.

### Symmetry-to-quantification

Finite IC3/PDR clauses are generalized using protocol symmetries into quantified invariants. Cutoff candidates are validated, not assumed.

### Counter abstraction

Processes are grouped by local control/data predicates, producing count variables. Soundness obligations define simulation from concrete populations.

### Regular/array abstraction

Parameterized arrays/rings can be abstracted to regular languages or string-rewriting systems where applicable.

### WSTS

Monotone systems use well-quasi-order coverability and ideals. The engine reports coverability, not exact reachability, unless justified.

### Nominal/orbit-finite lane

Atoms represent identities under a permutation action. States are stored by finite support and orbit shape. Fresh allocation becomes binding rather than selection from a fixed numeric bound.

## Language annotations

```text
type Node : atom[equality]
type Epoch : ordered
symmetry Nodes by all_permutations
fresh RequestId
parameter N : Nat where N >= 1
```

Operations that violate equivariance are rejected or force symmetry refinement.

## Quantified invariant certificate

A certificate records:

- invariant formula and quantifier structure;
- finite instances/cutoff evidence used to derive it;
- initialization proof;
- action-local inductiveness obligations;
- implication to target safety property;
- solver proofs or Lean lemmas.

Finite testing can suggest the formula; only the inductive proof establishes the unbounded claim.

## Small-model discipline

Continuum reports three distinct results:

- `checked(N = 3)`;
- `cutoff_checked(N ≤ k)` with theorem identifying the cutoff rule;
- `proved(∀ N)` via inductive/parameterized certificate.

UI and APIs must make them visually and structurally different.

## Initial targets

- Bakery/mutual exclusion;
- Chang-Roberts ring election;
- German cache coherence;
- quorum protocols/Paxos abstractions;
- sessions with unbounded fresh request IDs;
- EWD self-stabilizing/termination examples.
