# IDE, CLI, DAP, MCP, and SARIF Integration

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
continuum/fairnessLedger
continuum/exportCrashpack
```

## MCP

Expose bounded agent tools:

```text
create_workspace
start_verification
get_context_pack
expand_context
open_debug_branch
begin_repair
apply_repair
verify_repair
start_forge
get_evidence
```

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

Track separately:

- native protocol version;
- adapter protocol version;
- semantic epoch;
- schema version;
- proof/checker epoch.

An adapter can be wire-compatible while unable to render a new semantic artifact; it must expose a generic artifact path rather than silently drop fields.
