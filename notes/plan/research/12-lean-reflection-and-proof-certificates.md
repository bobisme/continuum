# Research 12: Lean Reflection and Proof Certificates

## Thesis

Continuum should separate finding evidence from trusting evidence. Modern certificate import work in Lean 4 suggests this can scale beyond toy proofs.

## Relevant work

### Lean4Lean

Mario Carneiro's Lean4Lean provides an independent Lean 4 typechecker written in Lean and reports verification of mathlib with performance within tens of percent of the reference checker.

https://arxiv.org/abs/2403.14064

Relevance: proof-checker diversity and a path toward validating Continuum's Lean artifacts with an independent implementation.

### LRAT-Catcher

LRAT-Catcher imports SAT LRAT certificates using Lean's formally verified checker through reflection, including cube-and-conquer composition.

https://arxiv.org/abs/2607.00815

Relevance: huge finite-state/CNF proofs need not become gigantic explicit proof terms.

### PBLean

PBLean imports VeriPB pseudo-Boolean certificates with a fully proved Boolean checker and verified encodings.

https://arxiv.org/abs/2602.08692

Relevance: cardinality, quorum, counting, optimization and finite combinatorics often encode more naturally in PB than CNF.

## Architectural inference

The scalable pattern is:

```text
untrusted search/solver
 → compact certificate
 → proved executable checker
 → theorem
```

But there are two trust gaps:

1. certificate checker soundness;
2. encoding from Continuum model/property to solver formula.

The second is frequently omitted in verification tooling and must be first-class.

## Certificate composition

Large Continuum results should be partitioned:

- state-space shards;
- per-action invariant obligations;
- per-cube SAT results;
- SCC components;
- refinement partitions;
- per-domain-pack assumptions.

A root certificate proves coverage/composition. This aligns with parallel search and incremental proof rebuilding.

## Native checker versus Lean checker

Routine workflow:

```text
Rust explorer → Rust small checker → assurance result
```

High-assurance/release workflow:

```text
same certificate → Lean reflective checker → theorem manifest
```

The Rust checker should eventually be proven equivalent to the Lean checker or generated from a shared verified specification. Until then, cross-checking catches implementation errors.

## What to formalize first

1. finite transition systems and closure;
2. canonical value/state digest correspondence;
3. bounded path witnesses;
4. stuttering simulation;
5. SCC/fair-lasso certificate;
6. CNF/PB bounded-unrolling encoding;
7. finite symmetry quotient.

DPOR and nominal symmetry come later because their preservation theorems are harder and their interfaces should be informed by working baseline engines.

## Performance questions

- Can certificate parsing/checking be streamed?
- Can Merkle-partitioned closure be checked without loading all states?
- What checker-to-search time ratio is acceptable?
- How stable are theorem packages across Lean patch releases?
- Does reflection retain reproducible diagnostics on malformed evidence?

## Novel proposal: theorem receipts

Every `PROVEN` result emits a receipt:

```text
model hash
semantics epoch
property theorem name
assumption theorem names
certificate hash
checker theorem/version
encoding theorem/version
Lean environment hash
axioms report
```

This is a durable, composable proof provenance object rather than a console message.
