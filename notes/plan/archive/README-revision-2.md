# Continuum Project Dossier — Revision 2

**Document date:** 2026-07-24  
**Status:** corpus-governed architecture, proof program, executable spikes, and implementation plan  
**Primary implementation language:** Rust  
**Concrete execution substrate:** [asupersync](https://github.com/Dicklesworthstone/asupersync)  
**Proof authority:** Lean 4 `v4.32.1`  
**Compatibility corpus:** `tlaplus/Examples@91c22ea537853196ed1e03e9ad91693ec37642de`

Continuum is a verification-native environment for concurrent and distributed Rust systems. Revision 2 is organized around a **semantic triptych**:

```text
                     MODEL
           abstract behavior/properties
              /                 \
             /                   \
      corpus parity           refinement
           /                       \
          /                         \
       PROOF -------------------- PROGRAM
 Lean semantics/certificates   asupersync Rust
```

CIR remains the canonical causal interchange format for concrete executions. It is no longer asked to carry the entire project alone. CML provides standalone mathematical models and temporal semantics; Lean defines strong claims and checks certificates; asupersync supplies cancel-correct real and controlled execution.

The release-level goal is explicit:

> Continuum 1.0 must provide native semantic equivalents for every CI-validated family in the TLA+ Examples repository at its declared parity level.

The dossier inventories **80 validated families** and **39 extended/unvalidated families**. The validated set spans finite search, proof, PlusCal-style algorithms, fairness/liveness, symmetry, refinement, consensus, Byzantine protocols, transactions, storage, TCP, lock-free concurrency, cache coherence, and deliberate failures.

## Start here

1. [`plan.md`](../plan.md) — revision-2 master implementation plan.
2. [`notes/START_HERE_IMPLEMENTATION.md`](../notes/START_HERE_IMPLEMENTATION.md) — first implementation sequence.
3. [`notes/G0_SPIKE_MATRIX.md`](../notes/G0_SPIKE_MATRIX.md) — load-bearing falsification gates.
4. [`docs/24_REVISION_2_ARCHITECTURE.md`](../docs/24_REVISION_2_ARCHITECTURE.md) — semantic triptych.
5. [`docs/22_TLA_EXAMPLES_COMPATIBILITY_PROGRAM.md`](../docs/22_TLA_EXAMPLES_COMPATIBILITY_PROGRAM.md) — corpus contract.
6. [`corpus/tla-examples/README.md`](../corpus/tla-examples/README.md) and [`validated-examples.csv`](../corpus/tla-examples/validated-examples.csv) — complete inventory.
7. [`docs/23_LEAN4_FORMALIZATION_PROGRAM.md`](../docs/23_LEAN4_FORMALIZATION_PROGRAM.md) and [`lean/README.md`](../lean/README.md) — metatheory/certificate program.
8. [`spikes/SPIKE_REPORT.md`](../spikes/SPIKE_REPORT.md) — executable evidence.
9. [`adr/README.md`](../adr/README.md) and [`rfcs/README.md`](../rfcs/README.md) — normative decisions and detailed designs.
10. [`research/README.md`](../research/README.md) — frontier mathematics and computer science.
11. [`docs/31_FALSIFICATION_AND_KILL_CRITERIA.md`](../docs/31_FALSIFICATION_AND_KILL_CRITERIA.md) — anti-bullshit controls.
12. [`VALIDATION_REPORT.md`](../VALIDATION_REPORT.md) — what was and was not mechanically checked.
13. [`REVISION_2_CHANGELOG.md`](REVISION_2_CHANGELOG.md) — exact architectural and research delta.
14. [`tools/validate_dossier.py`](../tools/validate_dossier.py) — repeatable mechanical validation.

## Executable findings in this revision

The included Python reference spikes use exact state identity and deterministic exploration:

- Die Hard: **16 states**, **96 labeled transitions**, shortest solution depth **6**;
- Dining Philosophers(5): **573 states**, **2,365 transitions**, shortest deadlock depth **10**;
- cyclic symmetry quotient: **573 → 117 states**, **4.90×** reduction, exhaustive automorphism check passes;
- reserve/commit/abort register: exhaustive stuttering refinement check passes;
- fair-lasso spike distinguishes unfair stuttering from a genuine liveness failure;
- safety game synthesizes the single forbidden move needed by `Ack ⇒ Durable`: `AckBeforeSync`;
- nominal canonicalization identifies fresh-name renamings without collapsing causal differences;
- semiring-valued traversal jointly computes shortest distance and witness multiplicity.

These are architecture spikes, not production Rust or kernel-checked Lean results.

## Revision-2 repository shape

```text
continuum/
├── crates/
│   ├── continuum-model-syntax / -elab / -core / -reference
│   ├── continuum-cir / -observer / -refinement
│   ├── continuum-asupersync / -effects / domain packs
│   ├── continuum-explicit / -dpor / -unfolding / -symbolic
│   ├── continuum-liveness / -games / -symmetry / -nominal
│   ├── continuum-weak-memory / -hyper / -production
│   ├── continuum-transform / -certificate / -proof-receipt
│   ├── continuum-corpus / -tla-oracle
│   └── continuum-cli / -daemon / -lsp / -agent-protocol
├── lean/
├── corpus/tla-examples/
├── schemas/
├── spikes/
├── benchmarks/
└── docs/
```

## Core terms

- **CML:** Continuum Model Language, the standalone typed model/proof surface.
- **CIR:** Causal Intermediate Representation for concrete and partial-order executions.
- **Semantic triptych:** independent Model, Program, and Proof planes.
- **Observer:** a property-relevant projection of states/executions.
- **Refinement graph:** explicit model/program/view edges across abstraction grains.
- **Domain pack:** versioned semantics for effects such as storage or network.
- **Corpus Tribunal:** differential, mutation, proof, and parity infrastructure.
- **Proof receipt:** content-addressed claim/certificate/Lean/axiom provenance.
- **Crashpack:** deterministic reproduction and causal explanation bundle.

## Claim discipline

Documents use:

- `FACT` — supported by source or executed evidence;
- `DESIGN` — accepted architecture;
- `HYPOTHESIS` — research claim requiring experiment;
- `TARGET` — implementation/release gate;
- `BLOCKED` — missing prerequisite;
- `FALSIFIED` — tested and rejected.

No novel mathematical lane becomes a product guarantee without a preservation theorem, baseline, benchmark win, and certificate story.

See [`VALIDATION_REPORT.md`](../VALIDATION_REPORT.md) for the mechanical checks,
their boundaries, and reproduction instructions. Git history is the archived
dossier's package provenance.
