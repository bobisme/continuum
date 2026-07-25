# Dossier Validation Report — Revision 3

**Validation date:** 2026-07-24  
**Validator:** `tools/validate_dossier.py`  
**Overall result:** **PASS within the boundaries stated below**

## Mechanical checks

| Check | Result |
|---|---:|
| JSON parse | PASS — 38 files |
| Draft 2020-12 JSON Schema validation | PASS — 15 schema/example pairs |
| ContinuumBench task validation | PASS — 3 seed tasks |
| TOML parse | PASS — 5 files |
| Python syntax parse | PASS — 18 files |
| Executable spike assertions | PASS — 30 assertions across Revision 2 and Revision 3 |
| Corpus inventory consistency | PASS — 80 validated + 39 extended rows |
| Relative Markdown links | PASS — 169 checked |
| Markdown code-fence parity | PASS — 211 files |
| Empty-file scan | PASS |
| ADR numbering uniqueness | PASS — 52 ADRs |
| RFC numbering uniqueness | PASS — 40 RFCs |
| Bibliography identifier uniqueness | PASS — 159 entries |
| Lean source placeholder/declaration scan | PASS — 17 `.lean` files; no `sorry`, `admit`, or top-level `axiom` declarations |

The machine-readable result is in `validation-results.json`.

## Revision 3 schema pairs validated

In addition to the Revision 2 semantic and certificate formats, the validator checks examples for:

- protected Intent Contracts;
- immutable workspace snapshots;
- resumable verification tasks;
- replay-preserving Context Packs;
- semantic and intent diffs;
- repair transactions;
- synthesis candidates;
- ContinuumBench tasks;
- Evidence Graph nodes.

## Executed Revision 3 experiments

The validator reran `spikes/run_r3_spikes.py` and checked eight experiment groups:

1. **Context Pack slicing:** a 200-event durability failure reduces to a replay-preserving, one-minimal four-event core.
2. **Intent integrity:** property weakening, assumption strengthening, bound contraction, observer coarsening, fault contraction, and assurance downgrade are detected.
3. **Agent protocol:** identical requests are idempotent; stale snapshots and mismatched idempotency reuse are rejected.
4. **Forge CEGIS:** safety plus non-vacuity selects the minimal `synced` acknowledgement guard and rejects vacuous or unsafe alternatives.
5. **Incrementality:** all tested invalidation paths match clean recomputation.
6. **Causal debugger:** one configuration branches into the safe sync path and the failing cancellation/finalizer path.
7. **Proof-oriented lens:** ambiguous abstract-to-concrete durability edits yield an explicit conflict rather than an invented choice.
8. **Multi-agent Evidence Graph:** content deduplication, authority-separated promotion, stale-evidence rejection, intent-gaming rejection, and explicit claim conflicts all hold in the finite spike.

Detailed results:

- `spikes/R3_SPIKE_REPORT.md`
- `spikes/results/r3-spike-results.json`

These are architecture-falsification experiments, not production-scale performance or soundness proofs.

## Corpus validation boundary

`corpus/tla-examples/check_inventory.py` checks the checked-in inventory for exact row counts, IDs, pinned source commit, unique source paths, and category/wave summaries. The artifact environment did not contain a local clone of `tlaplus/Examples`; the ingestion scaffold was therefore not rerun against the upstream tree. Pinned-clone regeneration and differential TLC/Apalache/Continuum execution remain implementation gates.

## Lean validation boundary

The environment contained no `lean`, `lake`, or `elan`, and direct network access from the execution container was unavailable. Therefore:

- Lean sources were inspected for placeholders and explicit top-level axioms;
- the sources were **not parsed, elaborated, or kernel-checked**;
- no theorem in this package should yet be described as machine-checked;
- `lake build`, per-theorem `#print axioms`, proof-receipt generation, and independent release checking remain mandatory gates.

The proof epoch is pinned to `leanprover/lean4:v4.32.1`.

## Rust and CML validation boundary

No Rust toolchain was present. Rust API sketches and proposed CML fixtures were not compiled because the corresponding implementation does not yet exist. The executed results are Python reference experiments over finite models and interaction contracts. DPOR, symbolic/PDR, production conformance, weak memory, scalable synthesis, and proof-carrying optimization remain designs or research programs unless explicitly marked as executed.

## Reproduction

From the dossier root with Python 3.11+ and `jsonschema` installed:

```bash
python3 tools/validate_dossier.py
python3 tools/generate_manifest.py
```

A successful validator run emits JSON with `"status": "pass"`. The manifest generator refreshes `MANIFEST.md` and `SHA256SUMS.txt`.
