# Continuum Lean Metatheory Seed

**Pinned toolchain:** `leanprover/lean4:v4.32.1`  
**Status:** source-level seed; not compiled in the artifact-generation environment because Lean/Lake were not installed.

## Purpose

This directory establishes the proof-plane architecture early enough to constrain the Rust implementation. It is not a complete formalization and must not be cited as a verified result until CI compiles it and records axiom manifests.

## Modules

- `Semantics.lean` — transition systems, reachability and inductive invariants;
- `Temporal.lean` — finite/infinite behavior primitives;
- `Fairness.lean` — named-action weak fairness seed;
- `Refinement.lean` — stuttering simulations, composition, and property transfer;
- `EventStructure.lean` — causal/conflict event-structure seed;
- `Certificate.lean` — semantic contract for finite closure certificates;
- `Independence.lean` — observer refinement and commutation monotonicity;
- `Symmetry.lean` — transition automorphisms preserve reachability;
- `Cancellation.lean` — cancellation lifecycle and decreasing phase rank;
- `ProofReceipt.lean` — proof receipt identity/policy seed;
- `Examples/DieHard.lean` — the concrete shortest solution witness.

## Mandatory CI before theorem claims

```bash
lake build
lake env lean Continuum.lean
rg '\bsorry\b|admit|axiom' Continuum
```

CI must additionally capture `#print axioms` for all public theorems and run an optional independent environment checker for release receipts.

## Formalization order

1. finite closure and invariant certificates;
2. stuttering refinement and composition;
3. finite traces and fair lasso certificates;
4. event configurations and observer-indexed independence;
5. symmetry/nominal quotient preservation;
6. cancellation and obligation conservation;
7. solver encoding and reflective certificate import;
8. corpus theorem libraries.

The source files intentionally avoid `sorry`. Absence of `sorry` does not imply they compile; the validation report distinguishes source inspection from kernel checking.

## Revision 3 interaction metatheory seeds

- `Interaction/Intent.lean` — protected intent equality and admissible repair edges;
- `Interaction/ContextSlice.lean` — replay-preserving context slices;
- `Interaction/Repair.lean` — proof-relevant repair acceptance gates;
- `Interaction/Incremental.lean` — clean/incremental parity contract;
- `Interaction/Synthesis.lean` — safety plus non-vacuity obligations for synthesis.

These files state the intended proof boundary for agent-facing operations. They are source-level seeds until the pinned Lean toolchain compiles them and CI records public-theorem axiom manifests.
