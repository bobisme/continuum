# Continuum Lean Metatheory

**Pinned toolchain:** `leanprover/lean4:v4.32.1`  
**Status:** RFC 0012 rungs **T0** (transition-system safety) and **T1** (stuttering
and refinement) are formalized and kernel-checked (PR-4A / IMPL-03). All 16
modules under `Continuum/` build with `lake build`; the tree contains no
`sorry`, declares no `axiom`, and uses no `native_decide` or `set_option` escape
hatches. The 68 theorems of the T0/T1 ladder depend on **no axioms at all** — see
`artifacts/axiom-manifest-t0-t1.json`. Wiring `lake build` and the manifest check
into `just check` is a separate follow-up and has not been done yet — run them
manually for now.

## Purpose

This directory establishes the proof-plane architecture early enough to constrain
the Rust implementation. It is not a complete formalization and must not be cited
as a verified result until CI compiles it and records axiom manifests.

`notes/plan/lean/` remains the planning dossier's historical record and is not
updated by changes here.

## Modules

- `Semantics.lean` — transition systems, reachability, inductive invariants,
  invariant algebra, finite paths and their endpoints (T0);
- `Temporal.lean` — behaviors, runs and stuttering runs, `Always`/`Eventually`/
  `InfinitelyOften` (T0 behavioral form, T2 seed);
- `Fairness.lean` — named-action weak fairness seed;
- `Refinement.lean` — stuttering and reachable (invariant-relative) simulations,
  composition, safety transfer, observer projection and hidden-event closure,
  history/auxiliary variable erasure, liveness-preservation conditions (T1);
- `EventStructure.lean` — causal/conflict event-structure seed;
- `Certificate.lean` — semantic closure certificates, the axiom-free Boolean
  plumbing (`Continuum.Reflect`), and the reflective finite-closure and
  counterexample checkers with soundness and rejection theorems (T0);
- `Independence.lean` — observer refinement and commutation monotonicity;
- `Symmetry.lean` — transition automorphisms preserve reachability;
- `Cancellation.lean` — cancellation lifecycle and decreasing phase rank;
- `ProofReceipt.lean` — proof receipt identity/policy seed;
- `Examples/DieHard.lean` — the puzzle as an executable `FiniteSystem`, with an
  accepted closure certificate, three rejected malformed certificates, and an
  accepted counterexample path (T0 instantiated).

## The RFC 0012 T0/T1 ladder

**T0 — transition-system safety**

| RFC rung item | key theorems |
| --- | --- |
| reachability induction | `TransitionSystem.reachable_satisfies`, `reachable_isInductive`, `reachable_least`, `reachable_iff_exists_path`, `isStutterRun_reachable` |
| inductive invariant preservation | `safety_iff_exists_inductiveInvariant`, `InductiveInvariant.and/top/mono/holds_on_path`, `always_of_inductiveInvariant` |
| finite closure certificate soundness | `ClosureCertificate.provesSafety`, `ClosureCertificate.exists_iff_safety`, `FiniteSystem.checkClosure_sound`, `checkClosure_provesSafety`, `checkClosure_eq_false_of_unclosed`, `checkClosure_eq_false_of_missing_init` |
| counterexample path validity | `reachable_endpoint`, `exists_path_of_reachable`, `FiniteSystem.checkTrace_sound`, `checkCounterexample_sound`, `no_closure_of_counterexample` |

**T1 — stuttering and refinement**

| RFC rung item | key theorems |
| --- | --- |
| stuttering simulation preserves reachable safety | `StutteringSimulation.maps_reachable`, `transfer_safety`, `ReachableSimulation.maps_reachable`, `ReachableSimulation.of_inductiveInvariant` |
| composition of simulations | `StutteringSimulation.refl`, `StutteringSimulation.compose`, `ReachableSimulation.compose` |
| observer projection and hidden-event closure | `TransitionSystem.observe_simulation`, `observe_step_visible`, `observe_reachable_iff`, `observe_safety_iff` |
| history/auxiliary variable erasure | `HistoryAugmentation.erasure_simulation`, `reachable_lift`, `erases_safety` |
| conditions for liveness preservation | `StutteringSimulation.projects_run`, `projects_run_of_visible`, `transfer_eventually`, `transfer_infinitelyOften` |

Rungs T2–T5 (temporal semantics, partial-order reduction, symmetry, concrete
execution) remain future work; the seeds for them live in `Temporal.lean`,
`Independence.lean`, `Symmetry.lean` and `Cancellation.lean`.

## Axiom manifests (ADR-0035)

```bash
sh scripts/axiom-manifest.sh            # regenerate the manifest artifacts
sh scripts/axiom-manifest.sh --check    # fail if the checked-in artifacts are stale
```

`AxiomManifest.lean` is the generator. It is deliberately outside the `Continuum`
library target (it imports `Lean`), so nothing in the metatheory depends on it.
It collects axioms with the same `Lean.collectAxioms` that `#print axioms` uses,
errors out if a listed theorem no longer exists, and writes:

- `artifacts/axiom-manifest-t0-t1.json` — machine-readable manifest, one row per
  theorem with its rung, rung item and axiom list;
- `artifacts/axiom-manifest-t0-t1.txt` — the same content in the wording
  `#print axioms` prints, for human audit.

**What "empty" means here.** Every one of the 68 T0/T1 theorems reports *does not
depend on any axioms* — not even `propext`, `Quot.sound` or `Classical.choice`.
The reflective checkers avoid `List.all` and `decide (· ∈ l)` because the core
correctness lemmas for those are proved through `propext`; `Continuum.Reflect`
restates them with axiom-free structural proofs.

Two authored theorems elsewhere in the tree still report `propext`, both from
`decide` on statements whose *instances* are proved with it, and neither is part
of the T0/T1 ladder:

- `Continuum.cancel_step_decreases_rank` (`Nat` order-decidability);
- `Continuum.Examples.DieHard.solution_ends_with_four_gallons` (`List` indexing).

Compiler-generated lemmas (`*.mk.injEq`, equation lemmas) also carry `propext`;
they are not authored claims and are not listed in the manifest.

## Mandatory CI before theorem claims

```bash
lake build
lake env lean Continuum.lean
sh scripts/axiom-manifest.sh --check
rg '\bsorry\b|\badmit\b|^ *axiom |native_decide|set_option' Continuum
```

An optional independent environment checker (Lean4Lean) should validate the
generated `.olean` environment for release receipts.

## Formalization order

1. finite closure and invariant certificates — **done (T0)**;
2. stuttering refinement and composition — **done (T1)**;
3. finite traces and fair lasso certificates;
4. event configurations and observer-indexed independence;
5. symmetry/nominal quotient preservation;
6. cancellation and obligation conservation;
7. solver encoding and reflective certificate import;
8. corpus theorem libraries.

## Revision 3 interaction metatheory seeds

- `Interaction/Intent.lean` — protected intent equality and admissible repair edges;
- `Interaction/ContextSlice.lean` — replay-preserving context slices;
- `Interaction/Repair.lean` — proof-relevant repair acceptance gates;
- `Interaction/Incremental.lean` — clean/incremental parity contract;
- `Interaction/Synthesis.lean` — safety plus non-vacuity obligations for synthesis.

These files state the intended proof boundary for agent-facing operations. They
compile under the pinned toolchain; their theorems are shallow projections out of
the acceptance predicates and are not covered by the T0/T1 manifest.
