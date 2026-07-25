# Proof Engineering Strategy

## Principle: prove interfaces, validate optimizations

A full formal verification of a high-performance parallel Rust model checker is a poor first objective. The better decomposition is:

- prove semantic kernels and certificate checkers once;
- require optimized engines to produce per-instance evidence;
- use translation validation for complex passes;
- verify small concurrency-critical implementation components with code-level tools where useful.

## Proof graph

Every theorem/certificate is a node in a dependency graph:

```text
Safety
 ├─ TypeOK
 ├─ QuorumIntersection
 ├─ LogPrefix
 └─ ActionPreservation
      ├─ ReceiveVote
      ├─ Commit
      └─ Recover
```

This supports inductive proof slicing, incremental rebuild, localized agent goals, and exact blame when a model action changes.

## Four proof modes

### 1. Native Lean proof

Best for foundational theorems, reusable mathematics, and small corpus proofs.

### 2. Reflection

Best for large finite certificates and decision procedures. A proved checker computes.

### 3. External solver certificate

Best for SAT/PB/SMT/PDR obligations. Solver is untrusted; certificate and verified encoding are checked.

### 4. Translation validation

Best for changing compiler/reduction optimizations. Each transformed artifact carries a witness checked against its input.

## Proof object sizing

Evidence must be streamable and content-addressed. Large state closures use:

- sorted chunked state tables;
- Merkle roots;
- partition closure witnesses;
- edge batches;
- local checker summaries;
- a final composition theorem.

Lean reflection verifies chunks and root composition without constructing a term per transition.

## Incrementality

Semantic identities are declaration/content hashes. A model change invalidates only:

- dependent operators/actions;
- proof graph slices;
- affected certificate partitions;
- refinement edges observing changed fields/events.

This is a long-term requirement, not a v0 optimization, because proof rebuild latency determines whether humans and agents keep verification enabled.

## Proof automation

Automation is layered:

1. simplification and decision procedures;
2. action-local VC generation;
3. finite counterexample-to-induction generation;
4. grammar-based lemma/invariant synthesis;
5. proof slicing;
6. solver-backed arithmetic/set reasoning;
7. agent proposal/search;
8. human theorem design for genuinely new abstractions.

No tactic's success is trusted beyond the kernel-checked term it produces.

## Code-level verification

Continuum exports local obligations to complementary tools:

- Kani for bounded bit-precise Rust behaviors;
- Loom/asupersync Lab for concrete schedules;
- Verus/Thrust-style tools for functional/refinement properties;
- Miri/sanitizers/fuzzers for implementation safety;
- Lean for semantic/certificate theorems.

The project does not pretend protocol-level model checking proves unsafe FFI or memory-model correctness.
