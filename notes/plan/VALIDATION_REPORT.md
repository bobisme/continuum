# Dossier Validation Report — Revision 3

**Validation date:** 2026-07-29
**Validator:** `tools/validate_dossier.py`  
**Overall result:** **PASS within the boundaries stated below**  
**Program status (derived):** **READY FOR SWARM DISPATCH** — plan §21.1
and `notes/START_HERE_IMPLEMENTATION.md` define an autonomous-agent swarm
with no owner or headcount prerequisite. Individual work remains
fail-closed on Bones dependencies, authority boundaries, phase exits, and
retained acceptance evidence. `READY` is an execution-posture result, not
a claim that implementation or any release gate is complete.

## Mechanical checks

| Check | Result |
|---|---:|
| JSON parse | PASS — 47 files |
| Draft 2020-12 JSON Schema validation | PASS — 19 schema/example pairs |
| ContinuumBench task validation | PASS — 3 seed tasks |
| TOML parse | PASS — 5 files |
| Python syntax parse | PASS — 22 files |
| Executable spike assertions | PASS — 30 assertions (count derived from source) across Revision 2 and Revision 3 |
| Corpus inventory consistency | PASS — 80 validated + 39 extended rows |
| Relative Markdown links | PASS — 191 checked |
| Markdown code-fence parity | PASS — 219 files |
| Empty-file scan | PASS — 321 files scanned, 0 empty |
| ADR numbering uniqueness | PASS — 52 ADRs |
| RFC numbering uniqueness | PASS — 40 RFCs |
| Bibliography identifier uniqueness | PASS — 159 entries |
| Lean source placeholder/declaration scan | PASS — 17 `.lean` files; no `sorry`, `admit`, or top-level `axiom` declarations |
| Retired-name scan (Tribunal rename, `cp_` handles) | PASS — 307 files scanned (generated `.typ` renders excluded) |
| Plan §22 ↔ docs/52 gate correspondence (bidirectional) | PASS — 11 gates, 68 bullets each way |
| Phase↔gate tables (plan §22 vs docs/52, structural) | PASS — 6 phases |
| G0 matrix count derivation (plan §0.3 vs matrix) | PASS — 4 evidence, 3 open freeze-blocking, 8 re-homed (DX-04/05/07/08 re-homed with artifact-shape spike evidence per review 5) |
| G10 two-system criterion (plan §21/§22 and docs/52) | PASS |
| Gate-citation hygiene (retired suffixed names) | PASS — 147 files |
| Rev-2 `Target gate` scheme qualifiers (RFCs 0001–0025) | PASS — 20 metadata lines |
| START_HERE PR gate annotations (incl. 4a/15a/15b/22a/25a/26a/27a/27b; open-G0 closing PRs must carry G0) | PASS — 39 PR headings |
| §24.5 register row integrity (lane refs resolve; kill/defer/draft present; marked-quote identity) | PASS — 21 rows, 0 ratified quotes yet |
| Handle-prefix registry (schema patterns ⊆ plan §4.4) | PASS — 19 prefixes, 47 anchored patterns |
| Program status (swarm rule, plan §21.1 vs START_HERE map) | DERIVED — ready, autonomous-agent-swarm, phases A–F |
| Specification-debt ledger (plan §25 vs `check_spec_debt` predicates) | PASS — open: SD-01, SD-07, SD-08, SD-09, SD-10; paid: SD-02–06, SD-11–14 |
| Executable plan↔Bones traceability | PASS — 846 registered requirements, 827 active and covered; 904 active Bones, 796 leaves, 2,089 active blocking edges, 14 layers, 0 cycles |

The machine-readable result is in `validation-results.json`.

## Executable implementation-graph boundary

`notes/PLAN_REQUIREMENTS.json` is generated from the implementation
program, PR sequence, gates, G0 experiments, invariants, proof
obligations, claims, threats, risks, research register, specification
debt, corpus inventory, test strategy, governance rules, metrics, and kill
criteria. Every active requirement has an active leaf-Bone execution home,
and every active Bone traces back through a `req:<ID>` label.

The graph contract additionally verifies:

- no unknown or duplicated generated requirement mappings;
- no untraced, empty, L/XL leaf, or over-bundled work items;
- 37 dispatch-ready leaves, counted with the same blocked/punted-ancestor
  propagation used by `bn next`;
- one `goal:manual` phase goal and one `goal:manual` exit goal for each
  Phase A–F;
- explicit predecessor-phase barriers on every Phase B–F leaf;
- one dependency-closed exit-evidence task per PR;
- integrated phase evidence before each release-gate criterion;
- risk and kill-signal assays assigned to their earliest evidence-complete
  phase and wired into that phase's integrated exit package;
- one leaf ratification task per active frontier lane, staged in the phase
  before its deadline and connected to a consuming Bone;
- separate native-port and interaction-artifact work for every corpus family;
- zero active dependency cycles.

Coverage proves that every registered executable obligation has a graph
home. It does not claim that any open Bone, proof obligation, release gate,
or implementation is complete.

## Revision 3 schema pairs validated

In addition to the Revision 2 semantic and certificate formats, the validator checks examples for:

- protected Intent Contracts;
- immutable workspace snapshots;
- resumable verification tasks;
- replay-preserving Context Packs;
- semantic and intent diffs;
- repair transactions (with cost ledgers and full 12-gate lists);
- synthesis candidates;
- ContinuumBench tasks;
- Evidence Graph nodes and edges;
- intent-registry records (`Proposed`/accepted status and acceptance chains);
- `Redacted(reason, commitment)` stubs (plan §4.5);
- promotion receipts (gate profile, §8.6 cost ledger, INV-014 checker identity).

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
python3 tools/generate_traceability.py
python3 tools/validate_dossier.py
python3 tools/generate_manifest.py
```

A successful validator run emits JSON with `"status": "pass"`. The manifest generator refreshes `MANIFEST.md` and `SHA256SUMS.txt`.
