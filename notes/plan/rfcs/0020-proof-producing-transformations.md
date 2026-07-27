# RFC 0020: Proof-Producing Transformations and Optimization Pipeline

**Status:** Proposed  
**Target gates:** G2–G6 (Revision 2 scheme, docs/26 — not citable without translation to the docs/52 Revision 3 gates per plan §22)

## Motivation

CML models undergo elaboration, desugaring, slicing, symmetry reduction, abstraction, bit-blasting, and solver encoding. A proof about the final formula is useless if an earlier transformation changed meaning.

## Transformation contract

Every transformation implements:

```text
input semantic object
output semantic object
mapping/witness
preservation claim
checker or Lean theorem
source provenance map
```

Claims include:

- equivalence;
- forward simulation;
- backward simulation;
- equisatisfiability;
- safety overapproximation;
- liveness-preserving abstraction;
- property-specific preservation.

“Optimization” is not a semantic category.

## Pipeline example

```text
CML source
  -- elaboration equivalence --> typed core
  -- finite instantiation --> finite relation
  -- slicing --> property cone
  -- symmetry quotient --> orbit graph
  -- transition encoding --> SMT
  -- bit-blast --> CNF
  -- SAT solve --> LRAT
```

The final evidence bundle links every arrow. A failure at any arrow caps the result at exploratory.

## Proof artifact economy

Full proof terms are not always practical. Allowed evidence forms:

- local rewrite traces checked by a small normalizer;
- simulation maps checked by finite traversal;
- hash-consed DAG proofs;
- solver certificates checked by reflection;
- theorem IDs for globally proved compiler passes;
- per-instance side conditions.

## Translation validation

For complex optimizations where a once-and-for-all compiler proof is expensive, Continuum uses translation validation: each output comes with a witness that the checker validates against the input.

## Source mapping

Preservation evidence also carries provenance from transformed variables/actions back to source. Counterexamples and proof failures must remain explainable after aggressive transformations.

## Initial proof-producing passes

1. action desugaring;
2. finite-domain instantiation;
3. property cone slicing;
4. exact state canonicalization;
5. finite symmetry quotient;
6. bounded transition unrolling;
7. CNF/PB encoding.

## Forbidden pattern

A transformation may not be trusted because “the tests match TLC.” Differential tests are necessary evidence, not a semantic proof.
