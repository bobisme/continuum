# IDE, CLI, DAP, MCP, and SARIF Integration

> **Non-normative projection.** This document is a projection of RFC 0026
> (`continuumd` native protocol), RFC 0027 (agent tool protocol), and plan
> §10.2, and is regenerated from them. On any divergence, the RFCs and the
> plan win.
>
> Regenerated against IDL 3.3 (protocol 3.3, 73 operations) and RFC 0026 as
> of bn-l4kmc.

## One authority, multiple projections

```text
continuumd native protocol
  ├── CLI/Cargo
  ├── LSP
  ├── DAP
  ├── MCP
  ├── SARIF
  └── Workbench UI/TUI
```

Adapters may cache presentation data but not semantic state or evidence authority.

## CLI/Cargo

Primary commands:

```text
continuum init
continuum snapshot
continuum check/explore/prove
continuum explain/debug/replay
continuum review
continuum repair ...
continuum forge ...
continuum evidence ...
continuum task ...
```

`cargo continuum` discovers Rust workspace configuration and forwards explicit snapshot operations.

Machine use:

- `--json` and JSONL streaming;
- stable exit codes;
- no ANSI/progress on non-TTY;
- artifact handles in every result;
- no need to parse explanation prose.

## LSP

Use standard LSP for:

- document synchronization;
- diagnostics;
- completion;
- hover;
- symbols/references/rename;
- code actions/lenses;
- semantic tokens;
- workspace impact.

Custom data carries snapshot/evidence handles. Unsaved documents become explicit overlays. The server never assumes editor content equals disk content.

## DAP

Use standard DAP for common debugger controls. Continuum custom requests include:

```text
continuum/enabledFrontier
continuum/branch
continuum/compareBranches
continuum/stepAbstract
continuum/reverseCausal
continuum/whyEnabled
continuum/whyBlocked
continuum/exportCrashpack
```

`continuum/fairnessLedger` is removed: no declared operation backs it.
`fairness` is one of the nine `AssuranceEnvelope` dimensions (RFC 0026 "The
assurance envelope"), carried on the result of whichever operation produced
it — `debug.state`, `model.check`, `verification.result`, and peers — not a
standalone query surface. A future fairness-ledger view is a new registered
operation (plan §10.2, RFC 0027's table, and the IDL together), not an
adapter-only addition (RFC 0026 correction 29, "an adapter MUST NOT expose
an operation the IDL does not declare").

## MCP

Expose bounded agent tools:

```text
create_workspace
start_verification
compile_context
expand_context
open_debug_branch
begin_repair
apply_repair
evaluate_repair
create_forge
get_evidence
```

Each name is a declared, total rename of a registered `OperationName`
(RFC 0026 correction 23) — a spelling, never a reinterpretation. Three
names in the previous revision had no declared operation behind them and
are corrected here (RFC 0026 correction 29): `get_context_pack` named no
`context.get` operation — Context Packs are produced by `context.compile`
(`compile_context`, initial) and `context.expand` (`expand_context`,
incremental), never fetched by a separate verb; `verify_repair` implied
`evidence.verify`, a different operation with a different authority level —
the gate-evaluation operation is `repair.evaluate` (`evaluate_repair`);
`start_forge` named a verb the registry does not have — the operation is
`forge.create` (`create_forge`).

Tool results contain explicit handles. Immutable artifacts are resources. Adapter descriptions emphasize capability and result class.

## SARIF

Map verification failures to:

- stable rule/property ID;
- primary source location;
- related model/proof/effect locations;
- code flow representing one linearization of causal core;
- properties with evidence/crashpack handles;
- assurance and intent fingerprints;
- safe fix only when a Repair Transaction has produced one.

SARIF consumers may deduplicate and annotate, but the proof artifact lives in Continuum.

## Workbench UI

The optional UI integrates:

- intent/assurance dashboard;
- causal graph and branch comparison;
- state delta explorer;
- evidence graph;
- repair transaction review;
- Forge archive and Pareto map;
- proof goals/receipts;
- corpus parity.

It must remain usable without cloud service.

## Versioning

Track the protocol's own six independently versioned compatibility epochs
plus engine identity (RFC 0026 "Epochs on the wire"), all carried in every
result's `epochs: EpochSet`:

- `protocol` — the negotiated native protocol version; required, never
  null on a served connection;
- `semantic` — the meaning of evaluation (ADR-0018);
- `intent` — the Intent Contract vocabulary and its policy tables;
- `evidence` — the evidence-graph and receipt schema epoch;
- `proof` — the Lean toolchain and theorem-package closure (ADR-0035);
- `corpus` — the pinned corpus revision and oracle toolchain;
- `engine` — engine *identity* (plan §4.7), carried in the same struct
  because continuations and defect reports pin it, but never a seventh
  compatibility epoch and never grouped under "epochs" in prose (RFC 0026
  correction 35).

There is no separate "adapter protocol version": an adapter defines no
semantics of its own (ADR-0036/0038/0042) and so has none to negotiate — it
is wire-compatible with the one native protocol version above, or it is
not. There is no checker epoch either: checker identity is the `proof`
epoch for Lean-backed checkers and engine identity for native checkers
(RFC 0030 correction 3, adopted by RFC 0026 correction 17). `schema_epoch`
is a per-payload header on every `Opaque` instance (`schemas/README.md`),
not a tracked protocol epoch and not a seventh one.

An adapter that cannot render a new semantic artifact MUST NOT fall back to
a generic artifact path: an artifact declaring an epoch the reader does not
implement is rejected with `EpochUnsupported`, never rendered best-effort
(docs/09 T13; RFC 0026 correction 31). Not silently dropping a field is a
separate, still-standing obligation — the two are not the same rule, and
paying one does not excuse the other.
