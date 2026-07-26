# RFC 0026: `continuumd` Native Protocol

## Status
Draft for implementation.

**Target gate:** G1 (workbench identity and lifecycle), G2 (agent-computer interface)
**Owners:** daemon/protocol leads
**Normative language:** MUST/SHOULD/MAY per RFC 2119.

## Summary

The single authoritative request/result protocol. CLI, Cargo, LSP, DAP, MCP, SARIF, TUI, and web clients are projections of this protocol and MUST NOT define semantics of their own (ADR-0036/0038/0042).

## IDL and versioning

- The protocol is defined in one machine-readable IDL file (checked into the repo; generated Rust/TS/JSON-schema clients derive from it). Prose in this RFC summarizes the IDL; the IDL is normative once it exists.
- `protocol_version` (e.g. `"3.0"`) is negotiated at connection open: the client sends its supported range; the daemon selects the highest common version or rejects with `ProtocolVersionUnsupported`. Protocol, semantic, intent, evidence, proof, and corpus epochs are versioned independently (docs/12 §7) and MUST NOT be conflated.
- Within a major protocol version, servers MUST ignore unknown optional request fields and MUST NOT emit fields the negotiated version does not define. Evidence artifacts are never "best-effort decoded" across breaking epochs (docs/09 T13): unknown breaking epoch ⇒ typed rejection.
- Handles remain valid across daemon upgrades within a major version; continuations additionally pin engine and semantic epochs and MUST be rejected with `ContinuationEpochMismatch` on mismatch (never silently re-run).

## Request envelope

| Field | Type | Req | Notes |
|---|---|---|---|
| `protocol_version` | string | yes | negotiated |
| `request_id` | string | yes | client-unique, for tracing |
| `idempotency_key` | string | yes for mutations | see below |
| `actor` | string | yes | e.g. `agent:repairer-1`, `human:bob` |
| `capability` | `cap_*` handle | yes | checked below the adapter, independently of handle possession |
| `operation` | `namespace.verb` | yes | from the operation registry (plan §10.2) |
| `snapshot` | `ws_*` or null | yes | explicit null when unused; the field name is `snapshot` everywhere (schemas included) |
| `intent` | `in_*` or null | yes | explicit null when unused |
| `arguments` | object | yes | per-operation schema from the IDL |
| `budget` | object | for long ops | wall/cpu/memory/states/solver/proof/tokens/candidates |
| `output_policy` | object | optional | max bytes/tokens/nodes, audience |
| `trace` | object | optional | W3C/OTel context propagation |

Idempotency: a mutation replayed with the same `idempotency_key` and byte-identical canonical request MUST return the same task/artifact identity; the same key with a different request MUST be rejected (`IdempotencyKeyReused`). Keys are scoped per actor and MUST be honored for at least the retention window declared in server capabilities (default 24h).

## Result envelope

| Field | Type | Notes |
|---|---|---|
| `request_id` | string | echo |
| `status` | enum | `ok, error, task_started, task_suspended` |
| `verdict` | typed | operation-specific; never bare prose |
| `error` | typed | from the taxonomy below; MAY carry `recovery` = list of allowed operations with pre-filled arguments (never free-form commands) |
| `assurance` | envelope | required on every semantic verdict (plan B11); every dimension names a producer or reads `Unsupported` |
| `artifacts` | handle list | typed prefixes per plan §4.4 |
| `task` / `continuation` | `task_*` / `cont_*` | for long operations |
| `omissions` | manifest | INV-007 |
| `warnings` | typed list | |
| `cost` | object | actual spend per budget dimension |
| `epochs` | object | semantic/engine/proof epochs the result is pinned to |
| `next_operations` | list | allowed operations from this state — the safe recovery/discovery surface (plan §0.2) |

## Task lifecycle

`Created → Running → Suspended | Completed | Failed | Cancelled` (plan §4.1).

- Task-starting operations (`verification.start`, `model.check`, `forge.create`, and peers — there is no generic `task.start`) are idempotent under idempotency keys; `task.status` is monotonic; `task.cancel` triggers request→drain→finalize and MUST leave either committed partial evidence plus a valid continuation, or nothing published (INV-009, B19).
- `task.resume` validates continuation identity, snapshot, and epochs; stale inputs are rejected (`StaleSnapshot`, `ContinuationEpochMismatch`). Resume MAY add evidence; it MUST NOT replace prior artifacts under the same identity.
- `task.subscribe` streams progress events over the same connection; events are hints — committed artifacts and `task.status` are authoritative.

## Pagination

List-returning operations accept `page_size` and an opaque `page_token`, and return `next_page_token`. Ordering MUST be deterministic (content identity or explicitly declared sort key); two identical requests against the same snapshot return identical pages.

## Transport, encoding, authentication

- Local IPC (unix socket / named pipe) first; authenticated HTTP/QUIC for shared/remote modes.
- Canonical JSON for debugging; CBOR with the same canonical field order for performance. One encoding per connection, negotiated.
- Remote mode requires an identity model; capabilities (`cap_*`) are minted, scoped, delegated, and revoked via daemon operations recorded in the audit log (plan §4.5). Possession of an artifact handle never implies authorization (ADR-0037).

## Error taxonomy

Stable codes (plan §10.3): `StaleSnapshot`, `UnsupportedSemanticFeature`, `IntentMutationDenied`, `InsufficientEvidence`, `BudgetExhausted`, `ContinuationEpochMismatch`, `AmbiguousCorrespondence`, `UntrustedDomainBoundary`, `CertificateRejected`, `ReplayDiverged`, `CapabilityDenied`, `PolicyGateFailed`; plus protocol-level `ProtocolVersionUnsupported`, `IdempotencyKeyReused`, `MalformedRequest`. `BudgetExhausted` is never a semantic verdict (docs/49); it carries the continuation when one exists.

## Rejected alternatives

- **Session-scoped implicit state.** Rejected (INV-002): a dropped connection must not change meaning.
- **Adapter-defined semantics.** Rejected (ADR-0042): MCP/LSP/DAP translate; they do not decide.
- **Free-form recovery suggestions in errors.** Rejected: recovery is a list of typed allowed operations to prevent injection through error text.

## Open questions

- IDL technology choice (custom vs. an existing schema language) — decide before PR 5 freezes anything.
- Capability delegation depth and expiry defaults for multi-agent handoff (with RFC 0027).

## Acceptance

Golden request/response traces pinned per protocol version; malformed-input fuzzing on both encodings; idempotency replay tests; restart/resume with epoch mismatches; cancellation at every instrumented phase; deterministic pagination; adapter parity (same operation through CLI/MCP/LSP yields identical artifacts).
