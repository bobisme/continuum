# RFC 0040: LSP, DAP, MCP, SARIF, and CLI Adapters

## Status
Draft.

## Rule
Adapters are stateless projections over explicit handles and native operations. They cannot create semantic truth or hide unsupported fields.

## LSP
Overlays become workspace snapshots; diagnostics carry evidence handles.

## DAP
Common debugger mapping plus namespaced partial-order requests.

## MCP
Curated bounded tools/resources with explicit handles and capability checks.

## SARIF
Stable rule/source/code-flow projection with evidence URIs; not proof format.

## CLI
Stable exit codes, JSON, concise prose, non-TTY determinism.

## Acceptance
Cross-adapter parity: same request inputs resolve to same authoritative artifacts.
