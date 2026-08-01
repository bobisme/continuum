# RFC 0026: `continuumd` Native Protocol

## Status
Draft for implementation.

**Target gate:** G1 (workbench identity and lifecycle), G2 (agent-computer interface)
**Owners:** daemon/protocol leads
**Normative language:** MUST/SHOULD/MAY per RFC 2119.
**Normative IDL:** [`../schemas/continuumd-native-protocol.idl`](../schemas/continuumd-native-protocol.idl) carries the normative wire definition — every operation of plan §10.2 with its typed request, response, verdict, authority level, and error set; the request/result envelopes; the handshake; the handle registry; and the closed vocabularies. This RFC summarizes; the IDL decides.

## Summary

The single authoritative request/result protocol. CLI, Cargo, LSP, DAP, MCP, SARIF, TUI, and web clients are projections of this protocol and MUST NOT define semantics of their own (ADR-0036/0038/0042).

## IDL and versioning

- The protocol is defined in one machine-readable IDL file, [`schemas/continuumd-native-protocol.idl`](../schemas/continuumd-native-protocol.idl) (checked into the repo; generated Rust/TS/JSON-schema clients derive from it, and MUST be regenerated rather than hand-edited). Prose in this RFC summarizes the IDL; the IDL is normative, and where this RFC and the IDL disagree the IDL decides and this RFC is corrected. The IDL declares its own version (`idl_version`) independently of the `protocol_version` it defines, and states which changes are compatible (minor) and which are breaking (major).
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
| `budget` | object | for long ops | `wall_ms / cpu_ms / memory_bytes / states / solver_ms / proof_ms / tokens / candidates / bytes` (the budget keys of `schemas/verification-task.schema.json`) |
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
| `epochs` | object | all six independently versioned epochs (protocol, semantic, intent, evidence, proof, corpus) plus engine identity; an epoch the result cannot pin is named and reads null — never omitted |
| `next_operations` | list | allowed operations from this state — the safe recovery/discovery surface (plan §0.2) |

## Task lifecycle

`Created → Running → Suspended | Completed | Failed | Cancelled` (plan §4.1).

- Task-starting operations (`verification.start`, `model.check`, `forge.create`, and peers — there is no generic `task.start`) are idempotent under idempotency keys; `task.status` is monotonic; `task.cancel` triggers request→drain→finalize and MUST leave either committed partial evidence plus a valid continuation, or nothing published (INV-009, B19).
- `task.resume` validates continuation identity, snapshot, and epochs; stale inputs are rejected (`StaleSnapshot`, `ContinuationEpochMismatch`). Resume MAY add evidence; it MUST NOT replace prior artifacts under the same identity.
- `task.subscribe` streams progress events over the same connection; events are hints — committed artifacts and `task.status` are authoritative.

## Version window, budget updates, and evidence subscriptions

Absorbed from plan §4.3 (SD-02):

- **Protocol-major window.** The daemon MUST serve protocol majors N and N−1 concurrently; a client on the previous major is rejected only when it falls outside this window (`ProtocolVersionUnsupported`).
- **Artifact readability is decoupled from protocol majors.** Evidence and receipt schemas MUST remain readable by every future verifier for their declared schema epoch — declared by the `schema_id`/`schema_epoch` header whose convention is normative in [`schemas/README.md`](../schemas/README.md) (plan §25, SD-08) — regardless of which protocol majors the daemon still serves; this readability is covered by the kernel crates' reproducible-build covenant (plan §20). Retiring a protocol major never orphans an artifact.
- **Mid-flight budget updates.** `task.update_budget(task_*, budget)` adjusts the budget of a running task. Raising a dimension extends the current run. Lowering below committed spend triggers suspension-with-continuation semantics (B18): the task transitions to `Suspended` with committed partial evidence plus a valid continuation — never silent truncation of the campaign (INV-009).
- **Evidence subscriptions.** `evidence.subscribe` streams typed evidence-graph deltas (node/edge publication and status transitions) for a declared scope. It is distinct from `task.subscribe`: progress events are hints, while evidence deltas reference committed artifacts; the graph itself remains authoritative on reconnect (INV-002 — a dropped subscription changes nothing).

## Operational contract on the wire

Absorbed from plan §4.5 (operational contract), §4.6 (epoch advance) and §4.7 (engine-defect lifecycle) — SD-09. The daemon-side obligations are [docs/35](../docs/35_CONTINUUMD_WORKBENCH_DAEMON.md) and the epoch decision is [ADR-0018](../adr/0018-semantic-versioning-and-replay.md); this section is only what crosses the wire, and where it and plan §4.5–§4.7 disagree this RFC is corrected and becomes normative (plan §25).

### Redacted values

Content can stop being readable in three ways — summarized after promotion, purged by key shred, or lost across a restore — and all three surface identically to a client.

- A value the daemon cannot return in full MUST be returned as the typed `Redacted` stub of the IDL ([`../schemas/redacted.schema.json`](../schemas/redacted.schema.json)): `redacted: true`, `reason`, `commitment`, `original_class`. Omitting the field, returning null, or returning an empty value MUST NOT be used to represent redaction — a client MUST be able to tell "withheld" from "absent" structurally, without inference.
- `reason` is a closed vocabulary: `summarized | purged | lost`. A daemon MUST NOT extend it; a new way to lose content is a protocol change, not a new string.
- The plan writes this value `Redacted(reason, commitment)`. That names the semantic pair; the normative wire and schema form carries four fields, and the two extra fields are load-bearing (`redacted` makes a stub structurally recognizable, `original_class` names what class of evidence is missing without a dereference).
- Every redacted value MUST also appear in the result's `omissions` manifest with reason `redaction` (INV-007), and any claim that required the hidden data MUST downgrade in the `assurance` envelope per plan §18.4. A verdict MUST NOT be reported at full strength over a redacted input on the grounds that the input "would have" supported it.
- `evidence.verify` over a receipt whose referenced content is redacted MUST return the structural verification result *and* the redaction. It MUST NOT return a bare failure (the receipt is intact) and MUST NOT return a bare success (the claim is no longer fully supported).

### Publication, quotas, and storage failure

- A publication that cannot complete atomically MUST fail with `PublicationAborted` (INV-017). Nothing is published and nothing is truncated; the client MAY retry with the same idempotency key, and the retry is a fresh publication, not a resumption of a partial one.
- `QuotaExhausted` is a capability's concurrency or resource quota (the `max_concurrent_tasks` of `ServerLimits` and the quotas a grant carries). `BudgetExhausted` is task-budget spend and carries a continuation. The daemon MUST NOT substitute one for the other, and neither is ever a semantic verdict (docs/49).
- Storage attribution is reported per artifact class (plan §4.5). It is operational telemetry: it MUST NOT appear inside a receipt, an assurance envelope, or an evidence-graph node.

### Existence oracles and cross-principal sharing

Content-addressed identities are derivable by anyone holding the content, so the protocol MUST NOT let identity possession or cache behavior become a channel for learning what another principal holds (plan §4.4, §4.5).

- A read of an artifact the caller is not authorized for MUST return `CapabilityDenied` whether or not the artifact exists. The daemon MUST NOT distinguish "no such artifact" from "not yours" — a distinct not-found *is* the existence oracle.
- With cross-principal sharing off (the default), a mutation whose content already exists under a different principal MUST produce the same result envelope as a first publication, including the `cost` block. A dedup that is invisible in `artifacts` but visible in reported cost is still an oracle. Timing SHOULD be indistinguishable as well; a deployment that cannot bound the timing signal MUST declare the residual channel rather than imply it has closed it.
- Cross-principal sharing is enabled only by an explicit sharing policy naming the sharing scope and the artifact classes in scope, carried as a capability property rather than a daemon-global flag.
- Idempotency keys are scoped per actor. A key MUST NOT be usable to probe or collide with another actor's requests: replaying another actor's key MUST behave as an unused key, never as `IdempotencyKeyReused`.

### Epoch advance

- An epoch advance MUST publish `EpochAdvanceNotice` — the per-artifact-class `Compatibility` map (`Preserved | Revalidate | Incompatible`) and the estimated `blast_radius`, both keyed by plan §4.4 class prefix — **before** the advance is applied.
- The daemon MAY serve at most two epochs of a kind during a migration; new work defaults to the newest. A continuation resumes only under its pinned epoch and MUST be rejected with `ContinuationEpochMismatch` otherwise; a payload declaring an epoch the daemon does not implement MUST be rejected with `EpochUnsupported`, never best-effort decoded (docs/09 T13).
- An advance MUST NOT mutate a published artifact and MUST NOT change what a published receipt claims. Re-derived artifacts get new identities linked by `SUPERSEDES` edges.
- The `epochs` block names all six compatibility epochs plus `engine`. `engine` is engine *identity* (plan §4.7), not a seventh compatibility epoch; it is named here because continuations and defect reports pin it, and ADR-0018 records the correction.

### Engine defects

- A `ReplayDiverged`, a parity mismatch, an engine crash, or an explanation-validation failure MUST cause a `defect_*` artifact to be published, and the result that reports the condition MUST carry that handle in `artifacts`. A client MUST NOT have to reconstruct a defect report from an error string.
- A `defect_*` pins every input by content identity, the semantic and checker epochs, the engine identity, and a minimized reproduction, under the same redaction policy as Context Packs (plan §18.4).
- An engine defect is never a semantic verdict about the client's program. The task reports typed inconclusiveness (INV-008) and the `assurance` envelope names the dimensions it could not establish.

## Pagination

List-returning operations accept `page_size` and an opaque `page_token`, and return `next_page_token`. Ordering MUST be deterministic (content identity or explicitly declared sort key); two identical requests against the same snapshot return identical pages.

## Transport, encoding, authentication

- Local IPC (unix socket / named pipe) first; authenticated HTTP/QUIC for shared/remote modes.
- Canonical JSON for debugging; CBOR with the same canonical field order for performance. One encoding per connection, negotiated.
- Remote mode requires an identity model; capabilities (`cap_*`) are minted, scoped, delegated, and revoked via daemon operations, and every such operation MUST be recorded in the audit log with actor, capability, inputs, policy decision, outputs, and evidence identity (plan §4.5, §18.5). What a capability confers is the IDL's `CapabilityDescriptor`. Possession of an artifact handle never implies authorization (ADR-0037): authorization is checked below the adapter, independently of handle possession.
- `cap_*` is the one plan §4.4 class that is not content-addressed. Capability tokens are minted randomly and do confer authority, so they are secrets: they MUST NOT be logged in request traces, MUST NOT appear in error text or in `next_operations` arguments, and have no content-derived store path (`crates/continuum-workspace/src/artifact_path.rs` refuses to derive one). Revocation, not purge, is how a capability stops being usable.

## Error taxonomy

Stable codes (plan §10.3): `StaleSnapshot`, `UnsupportedSemanticFeature`, `IntentMutationDenied`, `InsufficientEvidence`, `BudgetExhausted`, `ContinuationEpochMismatch`, `AmbiguousCorrespondence`, `UntrustedDomainBoundary`, `CertificateRejected`, `ReplayDiverged`, `CapabilityDenied`, `PolicyGateFailed`, `AcceptanceChainInvalid`, `StatusConflict`, `QuotaExhausted`, `EpochUnsupported`, `PublicationAborted`; plus protocol-level `ProtocolVersionUnsupported`, `IdempotencyKeyReused`, `MalformedRequest`. `BudgetExhausted` is never a semantic verdict (docs/49); it carries the continuation when one exists.

The five codes added in review 5:

- `AcceptanceChainInvalid` — an intent bundle's acceptance signature chain fails verification, or the referenced bundle is absent; CI fails closed on this code (plan §4.2.1, RFC 0037).
- `StatusConflict` — an evidence-graph status promotion lost its compare-and-set against the claim's current status (plan §11.7, RFC 0038); re-read and retry, the lattice never regresses.
- `QuotaExhausted` — a capability's concurrency or resource quota is exhausted (plan §4.5); distinct from `BudgetExhausted`, which is task-budget spend and carries a continuation.
- `EpochUnsupported` — an artifact or continuation declares a schema/semantic epoch unknown or incompatible with this daemon on resume or read; typed rejection, never best-effort decoding (docs/09 T13).
- `PublicationAborted` — an atomic publication aborted (e.g. disk exhaustion, plan §4.5); nothing was published and nothing truncated (INV-017).

## Rejected alternatives

- **Session-scoped implicit state.** Rejected (INV-002): a dropped connection must not change meaning.
- **Adapter-defined semantics.** Rejected (ADR-0042): MCP/LSP/DAP translate; they do not decide.
- **Free-form recovery suggestions in errors.** Rejected: recovery is a list of typed allowed operations to prevent injection through error text.

## Open questions

- ~~IDL technology choice (custom vs. an existing schema language)~~ — decided: a custom textual IDL, because three-valued field presence, per-operation authority levels, task-starting/idempotency annotations, closed per-operation error sets, and citable normative rules have no faithful encoding in JSON Schema, OpenAPI, or Protobuf; JSON Schema is a generated artifact of the IDL, not its source (see the IDL's "Notation" section).
- Capability administration (mint/scope/delegate/revoke, plan §4.5) is named in "Transport, encoding, authentication" above but has no entry in the plan §10.2 registry the IDL implements, so the IDL declares only what such operations would manipulate (`CapabilityDescriptor`; IDL open item 1). Either the registry gains the entries or the surface is declared out-of-band — before PR 5. What is *not* open, and does not wait on that choice: wherever these operations live, each one is privileged and MUST be recorded in the audit log, and the sharing policy that enables cross-principal reuse is a capability property rather than a daemon-global flag (plan §4.5, absorbed above). Declaring the surface out-of-band would move where the operations are invoked, never whether they are audited.
- Capability delegation depth and expiry defaults for multi-agent handoff (with RFC 0027).

## Acceptance

Golden request/response traces pinned per protocol version; malformed-input fuzzing on both encodings; idempotency replay tests; restart/resume with epoch mismatches; cancellation at every instrumented phase; deterministic pagination; adapter parity (same operation through CLI/MCP/LSP yields identical artifacts).

For the operational contract absorbed above:

- redaction round-trips — a summarized, a purged, and a lost artifact each read back as the typed stub with the right `reason`, appear in `omissions`, and downgrade the assurance envelope; `evidence.verify` over the referencing receipt returns structural success plus the redaction;
- existence-oracle tests — an unauthorized read of an artifact that exists and of one that does not produce byte-identical envelopes, and publishing content already held by another principal produces the same envelope and `cost` as a first publication;
- epoch-advance ordering — the notice, its per-class compatibility map, and its blast radius are observable before the advance is applied; a resume across it fails `ContinuationEpochMismatch` and an unknown epoch fails `EpochUnsupported`;
- defect emission — every injected `ReplayDiverged` and parity mismatch yields a result carrying a `defect_*` handle and a typed inconclusive assurance envelope, never a semantic verdict.
