# TLA+ Examples Compatibility Corpus

This directory turns `tlaplus/Examples` into a versioned, executable acceptance contract for Continuum.

**Pinned upstream commit:** `91c22ea537853196ed1e03e9ad91693ec37642de`  
**Observed on:** 2026-07-24  
**Mandatory CI-validated specifications:** 80  
**Tracked additional examples:** 39

The upstream repository explicitly describes itself both as an example library and as a diverse corpus for development and testing of TLA+ language tools. Continuum uses it in exactly that stronger sense.

## Files

- `validated-examples.csv` — the 80 CI-validated specification families listed by upstream at the pinned commit.
- `other-examples.csv` — in-tree unvalidated, submodule, PDF-only, and external examples tracked for expansion.
- `corpus-summary.json` — generated counts and category summary.
- `PARITY_LEVELS.md` — what “equivalent” means.
- `PORTING_WAVES.md` — dependency-driven migration order.
- `CORPUS_POLICY.md` — pinning, provenance, licensing, and regression policy.
- `REDISTRIBUTION_AUDIT.md` — per-family license and redistribution verdicts at the pinned commit.
- `FEATURE_CENSUS.md` — semantic capabilities forced by the corpus.
- `ports/` — executable/design fixtures for ports already started.

## Contract

A green checkbox does not mean “someone wrote a similar demo.” Each port has a machine-readable `port.json` validated by `schemas/corpus-port.schema.json` recording:

1. pinned source modules and model configurations;
2. native Continuum model and optional procedural surface;
3. state projection and value correspondence;
4. expected success/failure class;
5. reachable-state, depth, and trace-language facts where finite;
6. fairness and liveness assumptions;
7. refinement relation where the example is itself a refinement case study;
8. Lean theorems or imported certificates for proof-bearing examples;
9. unsupported or intentionally divergent semantics;
10. exact reproduction commands.

The corpus is a ratchet. A previously supported parity level cannot silently regress.

## Inventory tooling

- `check_inventory.py` checks IDs, counts, pinned commit, parity/wave domains, and summary recomputation.
- `ingest_local_clone.py` consumes a pinned local upstream clone and emits a strict drift census from manifests and model configuration files. It never overwrites curated port decisions.
