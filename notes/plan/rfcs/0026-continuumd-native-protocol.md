# RFC 0026: `continuumd` Native Protocol

## Status
Draft for implementation.

**Target gate:** G1 (workbench identity and lifecycle), G2 (agent-computer interface)
**Owners:** daemon/protocol leads
**Normative language:** MUST/MUST NOT/SHOULD/SHOULD NOT/MAY per RFC 2119.
**Normative wire artifact:** [`../schemas/continuumd-native-protocol.idl`](../schemas/continuumd-native-protocol.idl) carries the wire definition — every operation of plan §10.2 with its typed request, response, verdict, authority level, annotations, and error set; the request/result envelopes; the handshake; the handle registry; the closed vocabularies; and the `rule` blocks. Where this document and the IDL disagree about a **wire shape** — a field, a presence marker, a type, an enum member, an operation name, an annotation, or an authority level — the IDL decides and this RFC is corrected. Every such correction is recorded below under "Corrections recorded by this RFC".
**Companion normative sources:** [RFC 0027](0027-agent-tool-protocol.md) (the authority ladder and the per-operation authority table each `authority` clause mirrors), [RFC 0028](0028-context-pack-format.md) (Context Pack payloads), [RFC 0030](0030-incremental-semantic-query-engine.md) (query key, reuse, continuation resume, dependency reasons), [RFC 0031](0031-semantic-and-intent-diff.md) (`DiffLayer`, `PolicyDecision`, `StructuralOutcome`, the diff artifact), [RFC 0032](0032-repair-transaction-protocol.md) (gates and promotion), [RFC 0037](0037-intent-contract.md) (contract fields, closed policy verbs, acceptance chain), [RFC 0038](0038-multi-agent-evidence-graph.md) (append-only writes, status compare-and-set), [RFC 0039](0039-explanation-engine.md) (the explanation levels), [`../schemas/README.md`](../schemas/README.md) (the `schema_id`/`schema_epoch` convention every `Opaque` payload declares), [`../plan.md`](../plan.md) §4.1–§4.7, §9.6, §10, §18.5, [`../docs/35_CONTINUUMD_WORKBENCH_DAEMON.md`](../docs/35_CONTINUUMD_WORKBENCH_DAEMON.md) (the daemon-side half of the operational contract), [ADR-0013](../adr/0013-exact-state-identity.md), [ADR-0018](../adr/0018-semantic-versioning-and-replay.md), [ADR-0036](../adr/0036-authoritative-workbench-daemon.md), [ADR-0037](../adr/0037-explicit-content-addressed-handles.md), [ADR-0038](../adr/0038-machine-contracts-not-terminal-prose.md), [ADR-0042](../adr/0042-mcp-adapter-not-authority.md).
**Landed vocabulary:** `crates/continuum-value/src/epoch.rs` (`EpochKind`, `EpochIdentity`, the five content-identity epoch newtypes, `ProtocolEpoch`, `ProtocolRange`, `ProtocolWindow`, `Compatibility`, `EpochAdvance`, `EpochBinding`, `EpochSet`, `EpochSet::first_mismatch`) and `crates/continuum-value/src/assurance.rs` (`AssuranceLevel`, `AssuranceDimension`, `DimensionEvidence`, `UnsupportedReason`, `InconclusiveReason`, `ValidationBasis`) are the typed sources of the epoch, negotiation, and envelope rules this RFC states on the wire. This RFC and those modules MUST be revised together; neither may move alone.

## Summary

The single authoritative request/result protocol. CLI, Cargo, LSP, DAP, MCP, SARIF, TUI, and web clients are projections of this protocol and MUST NOT define semantics of their own (ADR-0036/0038/0042).

The IDL is the wire artifact; **this RFC is the semantic specification around it**. It is the normative home of: the connection lifecycle and what negotiation fixes; operation semantics and what each annotation obliges a daemon to do; the ordering, idempotency, and atomicity contract; task lifecycle and the continuation-resume admissibility predicate; epoch rules and epoch-advance ordering; the *meaning* of the error taxonomy as distinct from its spelling; the operational contract that crosses the wire (redaction, quotas, existence oracles, engine defects); and capability administration. Plan §4.3, §4.5, §10.1–§10.3 and docs/35, docs/36, docs/46, docs/55 are informal restatements; where they disagree with this document, this document governs (plan §25: "Where plan prose and RFC disagree, the RFC is corrected and becomes normative").

Two properties are load-bearing and are stated once here so nothing downstream re-derives them:

- **Nothing is session-scoped.** A dropped connection MUST NOT change the meaning of anything (INV-002). Every recoverable fact is reachable by re-reading a handle under a valid capability.
- **Refusal, exhaustion, and inconclusiveness are typed and distinct.** A budget ceiling, a quota ceiling, a missing capability, an unknown epoch, and an engine defect are five different answers, and none of them is a semantic verdict (INV-008, docs/49).

## Precedence

Four artifact kinds carry protocol truth. The order is fixed:

| Rank | Artifact | Governs |
|---|---|---|
| 1 | `schemas/*.schema.json` | the shape of an artifact instance carried in an `Opaque` field (INV-003, "schemas decide, prose does not") |
| 2 | [`../schemas/continuumd-native-protocol.idl`](../schemas/continuumd-native-protocol.idl) | every wire shape: operations, envelopes, handshake, enums, annotations, authority clauses, per-operation error sets |
| 3 | this RFC | every wire *semantic*: what an operation must do, in what order, under what obligations, and what a code means |
| 4 | `plan.md`, `docs/*` | the map — informal restatements, corrected by ranks 1–3 |

- Rank 1 over rank 2 is [`../schemas/README.md`](../schemas/README.md)'s rule: the IDL's `Opaque` never means "free-form"; the carrying field's doc comment names a governing schema and the payload MUST validate against it.
- Rank 2 over rank 3 is the IDL's own header rule and this document's "Normative wire artifact" line: a disagreement about *shape* is resolved in the IDL's favour and recorded here.
- Rank 3 over rank 2 holds in one direction only: where the IDL declares a shape but states no obligation — presence conditioned on an annotation, ordering across two operations, what a daemon must do before applying a change — this RFC states the obligation and the IDL is not thereby wrong. An obligation the IDL *cannot* express is recorded below as a flag, never resolved by silently contradicting the IDL.
- **Where the IDL contradicts itself**, neither rank settles it and this RFC MUST name the governing reading explicitly. Three such internal contradictions are open at protocol 3.0; each is recorded as a flag naming the minimal additive edit that resolves it. This RFC does not edit the IDL.
- Rank 3 over rank 4 is plan §25.

## Versioning and revision

- **Two versions, never conflated.** `protocol_version` (`"3.0"`) is the wire version this document and the IDL define; `idl_version` (`"1.1"` as of the bn-2wu5j wire fixes) versions the IDL *document* and is bumped on every change to that file, including changes that leave the protocol untouched. A client never negotiates `idl_version`.
- **`protocol_version` is `MAJOR.MINOR`**, decimal, no leading zeros, exactly one `.` — `ProtocolVersion`'s `@pattern`, and the parse/render round-trip of `ProtocolEpoch` in `crates/continuum-value/src/epoch.rs`. It is the one identity of the seven in `EpochSet` that carries a total order, because the daemon "selects the highest common version".
- **Compatible (minor) changes**, per `rule versioning.compatible_change`: adding an operation; adding an `optional` field; adding a member to an `@open` enum; relaxing a server-side constraint; adding an error code. Each MUST raise the minor version.
- **Breaking (major) changes**, per `rule versioning.breaking_change`: removing or renaming any declaration or field; changing a field's type or presence marker; adding a `required` or `nullable` field; adding or removing a member of a closed enum; changing an operation's authority level or verdict type; changing the meaning of an existing error code. Each MUST advance the major *and* MUST publish the typed per-artifact-class compatibility statement of plan §4.6 before it is applied.
- **Field presence is three-valued and wire-visible.** `required` (present, non-null), `nullable` (present, MAY be null — a *named* absence, INV-007), `optional` (MAY be absent). **Absent and null are distinct and MUST NOT be conflated** in either direction, by a daemon, a client, or an adapter. This is the mechanism by which "withheld", "not applicable", and "not asked for" stay distinguishable without inference; emitting `null` for an `optional` field, or omitting a `nullable` one, is malformed.
- **Enums are closed unless `@open`.** A daemon MUST NOT emit a member the negotiated version does not define. A client receiving an unknown member of a *closed* enum MUST treat the message as malformed; of an `@open` enum, it MUST NOT infer semantics, MUST NOT crash, and MUST surface the token verbatim to its caller as unknown. Four enums are `@open` at 3.0: `ErrorCode`, `TargetKind`, `ExplorationStrategy`, `ExplanationLevel`. Every other enum is closed — including `ResultStatus`, `TaskStatus`, `AuthorityLevel`, `Encoding`, `PolicyDecision`, `RedactionReason`, `Compatibility`, `DiffLayer`, `ExpansionRelation`, `StructuralOutcome`, `Portfolio`, `GateProfile`, `Audience`, and the four verdict vocabularies.
- **Unknown optional request fields are ignored**, and a daemon MUST NOT emit a field the negotiated version does not define (`rule envelope.unknown_fields`). This is the only forward-compatibility mechanism the protocol has; there is no best-effort decoding anywhere (docs/09 T13).
- **Epochs are versioned independently.** Protocol, semantic, intent, evidence, proof, and corpus epochs MUST NOT be conflated (docs/12 §7). A protocol-major bump does not by itself invalidate, reinterpret, or orphan any published artifact.
- **Artifact readability is decoupled from protocol majors.** Evidence and receipt schemas MUST remain readable by every future verifier for their declared schema epoch — declared by the `schema_id`/`schema_epoch` instance header whose convention is normative in [`../schemas/README.md`](../schemas/README.md) (plan §25, SD-08) — regardless of which protocol majors the daemon still serves; this readability is covered by the kernel crates' reproducible-build covenant (plan §20). Retiring a protocol major never orphans an artifact.
- **Handles remain valid across daemon upgrades within a protocol major.** Continuations additionally pin engine identity and the epochs their task consumed, and are rejected on mismatch — never silently re-run (see "Continuations and resume admissibility").

### Version window and negotiation

- **Protocol-major window.** The daemon MUST serve protocol majors N and N−1 concurrently — `protocol.majors_served = [3, 2]`, observable per connection as `ServerWelcome.majors_served`. A client on the previous major is rejected only when it falls outside this window, with `ProtocolVersionUnsupported`.
- The client offers an inclusive `VersionRange`; the daemon selects the **highest version common** to that range and its served set, or rejects the connection. The daemon MUST NOT select a version outside the client's range. `ProtocolEpoch::negotiate` is the typed form of this rule and `ProtocolWindow` of the major window; the daemon and that module MUST agree.
- A `RequestEnvelope` naming a different `protocol_version` than the connection negotiated MUST be rejected with `ProtocolVersionUnsupported`. Re-negotiation mid-connection is not a protocol feature; a client that wants a different version opens a new connection.

## Connection lifecycle

The handshake is **connection-level, not an operation**: it precedes any `RequestEnvelope`, is therefore outside the plan §10.2 registry, and has no entry in RFC 0027's authority table. It fixes five things and nothing else.

1. **Version.** `ClientHello.protocol_versions` (a `VersionRange`) against the daemon's served set ⇒ `ServerWelcome.protocol_version`.
2. **Encoding.** `ClientHello.encodings` is ordered most-preferred-first; the daemon selects one — `canonical_json` or `canonical_cbor` — for the life of the connection (`ServerWelcome.encoding`). Both encodings carry the *same* canonical field order; JSON is for debugging and CBOR for performance, and neither is a different protocol.
3. **Authority.** `ClientHello.capability` presents a `cap_*`; `ServerWelcome.grant` returns the `CapabilityDescriptor` the capability *actually* confers after scoping. **A client MUST NOT infer its authority from anything else** — not from a successful call, not from a handle it holds, not from a configuration file. A request MAY present a narrower capability than the connection's; it MUST NOT present a wider one.
4. **Limits.** `ServerWelcome.limits` (`ServerLimits`) declares `idempotency_retention_ms` (default `86_400_000`, i.e. 24h), `max_page_size`, `max_result_bytes`, and `max_concurrent_tasks`. These are the values the `QuotaExhausted` and pagination rules are stated against; a client that never reads them cannot distinguish a refusal it caused from one it did not.
5. **Epoch position.** `ServerWelcome.epochs` is the `EpochSet` the daemon serves *new* work under, and `ServerWelcome.pending_advances` is the list of `EpochAdvanceNotice` values announced but not yet applied. This is where a client reads the plan §4.6 pre-announcement; see "Epoch advance".

Two further rules:

- **`features` is advisory and non-semantic.** `ClientHello.features` lists identifiers the client understands; the daemon MUST ignore unknown ones and returns the intersection in `ServerWelcome.features`. A feature identifier MUST NOT change what an operation means, widen authority, or suppress an omission or an uncertainty. It exists for transport-level and rendering-level behavior only; anything that changes a verdict is a protocol version, not a feature.
- **No session state.** The handshake establishes version, encoding, and authority only. Reconnecting with the same handles and a valid capability continues the work; a dropped connection changes nothing (INV-002). A daemon MUST NOT hold semantic state reachable only through a live connection.

## Request envelope

Every operation is invoked through `RequestEnvelope`. The presence column is the IDL's three-valued marker; the "when" column is the obligation this RFC states on top of it.

| Field | Type | Presence | When |
|---|---|---|---|
| `protocol_version` | `ProtocolVersion` | required | MUST equal the negotiated version |
| `request_id` | `RequestId` (`^req_…`) | required | client-unique; echoed in the result; a tracing identifier, never an artifact identity |
| `idempotency_key` | `String` | optional | **REQUIRED for every `@mutation` operation; absent for every `@readonly` one** |
| `actor` | `ActorId` | required | `agent:` \| `human:` \| `service:` \| `ci:` — a closed four-member scheme |
| `capability` | `CapabilityHandle` | required | checked below the adapter, independently of handle possession, before any semantic work |
| `operation` | `OperationName` | required | one of the 72 registered `namespace.verb` names |
| `snapshot` | `WorkspaceHandle` | nullable | explicit null when the operation takes no snapshot; the field name is `snapshot` everywhere, schemas included |
| `intent` | `IntentHandle` | nullable | explicit null when the operation takes no intent |
| `arguments` | `Opaque` | required | the operation's request struct, exactly |
| `budget` | `Budget` | optional | **REQUIRED for every `@task_starting` operation** |
| `output_policy` | `OutputPolicy` | optional | `max_bytes` (enforced), `max_tokens` (advisory), `max_nodes`, `audience` |
| `trace` | `TraceContext` | optional | W3C Trace Context / OpenTelemetry propagation |
| `page` | `Page` | optional | `page_size` and `page_token`; MUST be honored by every `@paginated` operation |

- **Budget is an envelope field, not an operation argument.** Only `task.resume` and `repair.resume` additionally accept a `budget` inside `arguments`, and there it names the budget the *resumed* run gets, not the budget of the resume request. No operation declares `budget`, `snapshot`, `intent`, or a node ceiling among its own request fields.
- **`output_policy` bounds the payload, never the truth.** `max_bytes` is the enforced contract; token counts are advisory and tokenizer-relative (RFC 0027), and `max_nodes` is an `OutputPolicy` member, not a `Budget` dimension. Trimming to a ceiling MUST produce an `omissions` entry (INV-007) and MUST NOT drop an assurance dimension, a warning, an epoch, or a redaction stub. Hiding uncertainty to save bytes is prohibited.
- **`audience` is a rendering hint.** It never changes a verdict, never widens authority, and never suppresses an omission or an uncertainty.

## Result envelope

| Field | Type | Presence | Notes |
|---|---|---|---|
| `request_id` | `RequestId` | required | echo |
| `status` | `ResultStatus` | required | `ok` \| `error` \| `task_started` \| `task_suspended` |
| `verdict` | `Verdict` | nullable | null when the operation declares no `verdict` clause, and on `status = error` |
| `error` | `Error` | optional | present **exactly** when `status = error` |
| `assurance` | `AssuranceEnvelope` | optional | **REQUIRED whenever `verdict` is `semantic` or `evaluation`** |
| `artifacts` | `list<ArtifactRef>` | required | typed refs (`kind`, `handle`, `commitment?`, `redacted?`), not bare handles |
| `task` | `TaskHandle` | optional | present on `task_started`/`task_suspended` and on task-observing results |
| `continuation` | `ContinuationHandle` | optional | present on `task_suspended` and on a resumable failure |
| `omissions` | `list<Omission>` | required | the INV-007 manifest; an empty list means nothing was omitted, and the field is never absent |
| `warnings` | `list<Warning>` | required | typed `code`/`detail`; never interpolated source, log, or model text (INV-016) |
| `cost` | `Cost` | required | actual spend in the same nine dimensions as `Budget` |
| `epochs` | `EpochSet` | required | see "Epochs on the wire" |
| `next_operations` | `list<NextOperation>` | required | the safe recovery and discovery surface (plan §0.2) |
| `next_page_token` | `PageToken` | optional | present on `@paginated` operations; null on the last page |
| `payload` | `Opaque` | nullable | the operation's response struct; null on `status = error` |

The result envelope carries **no** `snapshot`, `intent`, `budget`, or trace-correlation field. Those are request-side; a result that needs to name its inputs does so through the task record or the artifact it published.

### Verdicts

`Verdict` is a four-member union, and an operation's `verdict` clause names which variant it returns. The distribution across the 72 operations is fixed:

| Variant | Value type | Operations |
|---|---|---|
| `semantic` | `SemanticVerdictValue` — `established` \| `refuted` \| `inconclusive`, plus `assurance_class` | 7 |
| `evaluation` | `EvaluationVerdictValue` — `satisfied` \| `refuted` \| `deadlock` \| `inconclusive`, plus `assurance_class` | 9 |
| `policy` | `PolicyVerdictValue` — a `PolicyDecision` (`allow` \| `review` \| `block`) plus `GateOutcome`s | 7 |
| `structural` | `StructuralVerdictValue` — a `StructuralOutcome` (`created`, `updated`, `sealed`, `accepted`, `rejected`, `locked`, `cancelled`, `unchanged`, `acknowledged`) | 25 |
| — | no `verdict` clause; `verdict` is null | 24 |

- **A verdict is a tagged union value, never a bare scalar.** `refuted` is a member of both `SemanticVerdict` and `EvaluationVerdict`; the union tag is what disambiguates them, and `assurance_class` is required on both. A daemon MUST NOT emit a verdict as a plain string.
- **`inconclusive` MUST carry `inconclusive_reason`** — one of the six `InconclusiveReason` members (`Unsupported`, `ResourceExhausted`, `EngineError`, `InsufficientTelemetry`, `AbstractionAmbiguity`, `IncompleteProofSearch`). A bare `inconclusive` is not a result (INV-008). The six members and their spellings are `crates/continuum-value/src/assurance.rs`'s `InconclusiveReason`; the crate and this RFC MUST agree token for token.
- **There is no `Unknown` verdict, status, or reason on this protocol.** `unknowns` is an assurance *dimension*; `unknown` is a diff *relation* (RFC 0031); `inconclusive` is the verdict token. A daemon MUST NOT emit `Unknown` as any of the three.
- **`StructuralOutcome::unchanged` is not the diff relation `unchanged`** (RFC 0031). `StructuralOutcome` describes what a mutation did to an artifact; the relation describes a classified field. The two vocabularies share a token and nothing else, and a daemon MUST NOT derive one from the other.
- **A verdict is never prose.** A daemon MUST NOT return prose in place of a verdict, a status, or a reason, and MUST NOT interpolate source, logs, model text, or production payloads into `detail`, `rationale`, `next_operations`, or any tool description (INV-003, INV-016). Prose renderings are adapter projections of these fields.
- **`next_operations` entries are structs, not names.** Each is a `NextOperation` carrying `operation` *and* `arguments` pre-filled against that operation's request struct, plus an optional typed `rationale`. A list of bare operation names is not a recovery surface, because a client cannot execute it as given.

### The assurance envelope

`AssuranceEnvelope` carries nine dimensions — `bounds`, `faults`, `fairness`, `values`, `schedules`, `memory_model`, `observer`, `proof_status`, `unknowns` — and every one is `required`. Each is an `EnvelopeDimension`: either `produced` (naming the producing `engine` and a `summary`) or `unsupported` (carrying a typed `reason`). The member list is identical to [`../schemas/assurance-result.schema.json`](../schemas/assurance-result.schema.json) and [`../schemas/context-pack.schema.json`](../schemas/context-pack.schema.json), which the validator holds identical to each other (SD-12), and to `AssuranceDimension::ALL` in `crates/continuum-value/src/assurance.rs`.

- The envelope is REQUIRED on every `semantic` and every `evaluation` verdict (plan B11). It is not required on a `structural` or `policy` verdict, because neither claims anything about program behavior.
- A dimension is **never silently omitted**. "We did not check the memory model" is `unsupported` with a typed reason (`sequential-consistency-only` is the canonical one), not an absent key and not a `produced` entry with an empty summary.
- `assurance_class` on the verdict value and the envelope's contents are different statements: the class is the level the verdict is *supported at* (`observed` \| `sampled` \| `bounded` \| `validated` \| `proved` — the `AssuranceLevel` order of `crates/continuum-value/src/assurance.rs` and of RFC 0031), while the envelope says which dimensions produced anything at all. A daemon MUST NOT report a class the envelope cannot support.

### Cost and omissions

- `Cost` reports the same nine dimensions as `Budget` (`wall_ms`, `cpu_ms`, `memory_bytes`, `states`, `solver_ms`, `proof_ms`, `tokens`, `candidates`, `bytes`) and nothing else (SD-12: one budget/cost dimension list, shared with [`../schemas/verification-task.schema.json`](../schemas/verification-task.schema.json) and the plan §8.6 cost ledger). **A dimension the engine does not measure is absent, never zero** — the presence distinction carries the meaning.
- `tokenizer_id` is REQUIRED whenever `tokens` is reported. A token count without its tokenizer identity is not a measurement.
- `Omission` carries a closed `reason` (`budget`, `redaction`, `unsupported`, `heuristic-cutoff`, `slice-irrelevant`), a typed `subject`, and an optional `recoverable_by` handle. The subject is stated in typed terms, never as free-form prose about the user's source.

## Operation semantics

The IDL declares **72 operations in 18 namespaces** — exactly the plan §10.2 registry, no more and no fewer (`rule conformance.registry_agreement`). Each operation's `authority` clause mirrors RFC 0027's table exactly; a generator or validator MUST fail closed on any disagreement rather than preferring either source. The distribution at protocol 3.0:

| Annotation | Count | Obligation this RFC states |
|---|---|---|
| `@readonly` | 28 | performs no state change; the request MUST NOT carry `idempotency_key` |
| `@mutation` | 44 | changes daemon state, including publishing a new immutable artifact; the request MUST carry `idempotency_key` |
| `@task_starting` | 26 | MAY return `status = task_started` with a task handle instead of a completed result; the request MUST carry `budget` |
| `@paginated` | 6 | MUST honor `page` and MUST return `next_page_token` |
| `@streaming` | 2 | delivers `events` frames on the same connection after the initial response |
| `@privileged` | 5 | audit-recorded, and never present in a default agent capability profile |
| `@audit_recorded` | 6 | every call written to the audit log with actor, capability, inputs, policy decision, outputs, and evidence identity (plan §18.5) |

Authority levels across the 72: `read` 18, `propose` 8, `execute` 40, `revise-intent` 4, `promote` 2. Agent-facing installations MUST omit `promote` and `revise-intent` from default capability profiles (RFC 0027).

Four independence rules govern the annotation set, and each exists because the obvious conflation is wrong:

- **`@mutation` and `authority` are orthogonal.** An operation can publish an immutable artifact at `read` authority when the publication is the daemon's own record of a read: `context.compile`, `context.expand`, `evidence.verify`, and `query.clean_compare` are all `@mutation` at `authority read`. A client MUST NOT infer required authority from the mutation annotation, and a daemon MUST NOT raise the required level because an operation publishes.
- **`@mutation` and `@task_starting` are orthogonal.** Four operations are `@readonly @task_starting` — `workspace.diff`, `verification.await`, `model.compare`, `failure.explain` — and start work without mutating state. They therefore carry `budget` and **do not** carry `idempotency_key`: idempotency is a property of mutations, not of long operations. Twenty-two operations are `@mutation @task_starting` and carry both.
- **`@privileged` implies `@audit_recorded`; the converse does not hold.** `observe.ingest` is audit-recorded without being privileged, because trace ingestion is an ordinary `execute`-authority operation that additionally requires the production-trace capability (plan §18.2) and must leave a record.
- **`@task_starting` is a *may*, not a *must*.** `verification.start` MAY return a cached result instead of a task when one exists for the same snapshot, intent, target, and epochs (RFC 0030's reuse rules decide when). A client MUST handle both shapes; a daemon MUST NOT fabricate a task handle to make the shape uniform.

### Registered ahead of shipping

`rule errors.unsupported_surface`: operations registered in plan §10.2 ahead of their producing subsystem — the `observe` family, and any operation whose lane has not shipped — MUST fail with the typed `UnsupportedSemanticFeature` rather than degrading, guessing, or returning an empty success. An unsupported operation still returns a valid machine result: it names every epoch, carries a typed `omissions` list, and carries no success flag.

### Adapter parity

The same operation invoked through the CLI, MCP, LSP, DAP, or any other adapter MUST yield identical artifacts and identical typed results (`rule conformance.adapter_parity`). Adapters translate; they do not decide (ADR-0042). Three obligations follow, and the third is the one adapters violate in practice:

- An adapter MUST NOT merge two operations into one call, MUST NOT add a semantic argument, and MUST NOT reinterpret a verdict.
- An adapter MUST NOT expose an operation the IDL does not declare. A surface name with no declared operation behind it is a violation whatever it is called.
- An adapter MAY rename an operation to fit its own naming conventions — `create_workspace` for `workspace.create` where dots are unavailable — **only** under a declared, total mapping from adapter name to `OperationName`. The renaming is a spelling; it MUST NOT change the verb's meaning, and a name whose mapping target does not exist is the violation the previous bullet forbids.

## Ordering, idempotency, and atomicity

### Deterministic ordering

Every list a result returns MUST be deterministically ordered by content identity or by an explicitly declared sort key. **Two identical requests against the same snapshot and epochs MUST return identical bytes under the same encoding.** Byte-identity, not set-identity, is the contract: it is what makes golden traces meaningful and what lets a client cache by request.

### Pagination

Pagination is a property of the six `@paginated` operations — `proof.slice`, `debug.enabled`, `forge.archive`, `evidence.query`, `query.explain_reuse`, `query.explain_invalidation` — not of every operation that returns a list. Many operations return unpaginated lists (`workspace.create`'s `diagnostics`, `model.explore`'s `evidence`, `evidence.subscribe`'s `frontier`), and those are bounded by `output_policy`, not by a cursor.

- `@paginated` operations accept `page_size` and an opaque `page_token` in `RequestEnvelope.page` and return `next_page_token`, null on the last page. `page_size` above `ServerLimits.max_page_size` is clamped, and the clamp MUST be visible through `next_page_token` continuing rather than through a silent truncation.
- Paging MUST be stable against the pinned snapshot and epochs: two identical page requests return identical pages, and the concatenation of all pages is the full deterministic ordering with no duplicate and no gap.
- **A `page_token` minted under different epochs MUST be rejected with `EpochUnsupported`.** A cursor is not portable across an epoch advance; re-paging under the new epoch is a new traversal.

### Idempotency

- A mutation replayed with the same `idempotency_key` and a byte-identical canonical request MUST return the same task or artifact identity. The same key with a different request MUST be rejected with `IdempotencyKeyReused`.
- Keys are honored for at least `ServerLimits.idempotency_retention_ms` (default 24h). Outside the window the daemon MAY treat the key as unused; it MUST NOT return a different identity for a replay it still remembers.
- **Keys are scoped per actor**, and the scoping is a security property, not a convenience: replaying *another* actor's key MUST behave as an unused key, never as `IdempotencyKeyReused`. A key that could collide across principals is an existence oracle. The IDL states the scoping but not this observable consequence.
- An idempotency key is not a transaction identifier and MUST NOT be reused to group operations. Each mutation carries its own.

### Atomicity of publication

- A publication that cannot complete atomically MUST fail with `PublicationAborted` (INV-017). Nothing is published and nothing is truncated; the client MAY retry with the same idempotency key, and the retry is a fresh publication, not a resumption of a partial one.
- Publication commits content before index, so a crash leaves unreachable content eligible for GC and never a stale index entry (plan §4.5; docs/35 owns the daemon-side half).
- Cancellation MUST NOT truncate a publication in progress: `task.cancel` finalizes or discards, never both halves.

## Task lifecycle

```text
Created → Running → Suspended | Completed | Failed | Cancelled
                       │
                       └── continuation + committed partial evidence
```

`TaskStatus` (`created`, `running`, `suspended`, `completed`, `failed`, `cancelled`) is the task-lifecycle vocabulary. It is **not** `ResultStatus` (`ok`, `error`, `task_started`, `task_suspended`), and a daemon MUST NOT emit a member of one where the other is declared: `completed` is not a result status, and `ok` is not a task status.

- **There is no generic `task.start`.** Long operations are started by the 26 `@task_starting` operations the IDL declares (`verification.start`, `model.check`, `forge.create`, and peers). Each `@mutation @task_starting` operation is idempotent under `idempotency_key`.
- **`task.status` is monotonic.** Reported milestones and `committed_evidence` only grow, and a terminal status never changes. `TaskRecord.failed_reason` (an `ErrorCode`) is REQUIRED when `status = failed`: **a `Failed` task is never silent** (plan §4.5). `TaskRecord.continuation` is REQUIRED when `status = suspended` — a suspended task is resumable by definition (plan §11.4) — and `non_resumable_reason` is REQUIRED when `failed_reason = BudgetExhausted` and no continuation exists.
- **`task.cancel` triggers request → drain → finalize.** It MUST leave either committed partial evidence plus a valid continuation, or nothing published (INV-009, B19). Its response carries `continuation` as `nullable`, so "cancelled with nothing published" is a representable, named outcome rather than an inference from an absent field.
- **`task.update_budget` adjusts a running task's budget.** Raising a dimension extends the current run. Lowering a dimension below committed spend triggers suspension-with-continuation (B18): the task transitions to `Suspended` with committed partial evidence plus a valid continuation. Silent truncation of a campaign is prohibited (INV-009).
- **`task.subscribe` streams progress events**, and **`evidence.subscribe` streams committed evidence-graph deltas**. The two differ in kind and MUST NOT be merged: `TaskEvent` values (`milestone`, `progress`, `status_change`, `evidence_committed`) are *hints*, while `EvidenceEvent` values (`node_published`, `edge_published`, `status_transition`, `conflict_materialized`) reference *committed* artifacts. Committed artifacts and `task.status` are authoritative for the first; the graph itself is authoritative for the second. A dropped subscription changes nothing (INV-002), and a client MUST be able to recover the same state by re-reading.
- **A subscription is not a lock and not a transaction.** Reconnecting and re-reading MUST yield a superset of what the stream delivered; a daemon MUST NOT rely on a client having observed an event.

## Epochs on the wire

`EpochSet` is the one struct that carries the seven identities a result is pinned to:

| Field | Presence | Meaning |
|---|---|---|
| `protocol` | required | the negotiated protocol version; never null on a served connection |
| `semantic` | nullable | the meaning of evaluation (plan §4.6, ADR-0018) |
| `intent` | nullable | the Intent Contract vocabulary and its policy tables |
| `evidence` | nullable | the evidence-graph and receipt schema epoch an artifact declares itself under |
| `proof` | nullable | the Lean toolchain and theorem-package closure (ADR-0035) |
| `corpus` | nullable | the pinned corpus revision and oracle toolchain |
| `engine` | nullable | **engine identity** (plan §4.7) — provenance, not a seventh compatibility epoch |

- **Every result MUST name all six compatibility epochs.** An epoch the result cannot pin reads null; it is never an absent field. This is `EpochBinding::Unpinned` in `crates/continuum-value/src/epoch.rs`, and it is why an unsupported or empty task still returns a valid machine result naming every epoch and carrying no success flag.
- **`engine` is engine identity, not an epoch.** It is carried in the same struct because continuations pin it and defect reports quote it, and ADR-0018 records the correction. It MUST NOT be added to `EpochKind`, MUST NOT enter a query key (RFC 0030), and MUST NOT be substituted for the semantic epoch. Prose that writes "engine epoch", or that groups `engine` under the word "epochs", is corrected by this rule.
- **There is no checker epoch, no solver epoch, and no schema epoch in this struct.** Checker identity is the `proof` epoch for Lean-backed checkers and engine identity for native checkers (RFC 0030's correction 3, adopted here); a solver encoding version is part of a query definition's `strategy_config`; `schema_epoch` is a per-payload header ([`../schemas/README.md`](../schemas/README.md)), not a tracked epoch and not a seventh one.
- **`protocol` is required, not nullable**, because a served connection always has a negotiated version. It is also the one *connection-scoped* identity: it is not part of snapshot identity (SD-13), never enters a query key (RFC 0030, "protocol | never"), and — see below — never participates in a resume comparison.

### Continuations and resume admissibility

A `cont_*` continuation MUST pin, at creation:

- the snapshot (`ws_*`) and, where the task is intent-scoped, the intent (`in_*`);
- the committed frontier and search state, in the frontier form RFC 0030's budget rule requires;
- an `EpochSet` naming all six compatibility epochs — every epoch the task consumed `Pinned`, every one it did not explicitly `Unpinned`, none omitted;
- **engine identity** (plan §4.7), which the IDL carries as `EpochSet.engine`.

**The pinning obligation is what makes resume safe.** `EpochSet::first_mismatch` treats an epoch the continuation left unpinned as constraining nothing — correctly, because a continuation resumes "only under their pinned epoch" and one that pinned nothing declared nothing to resume under. That rule is permissive by design, so the safety of resume rests on the obligation above: a continuation for a semantic task MUST pin the semantic epoch; a proof- or certificate-bearing continuation MUST pin the proof epoch; a corpus-oracle continuation MUST pin the corpus epoch; an intent-scoped continuation MUST pin the intent epoch. A continuation omitting an epoch its task consumed, or omitting engine identity, is malformed and MUST be rejected at creation, not at resume.

#### The two-predicate obligation

`crates/continuum-value/src/epoch.rs` scopes itself to the six compatibility epochs and states outright that engine identity is out of scope for that module, while this RFC and the IDL's `EpochSet.engine` require continuations to pin it. Both artifacts are right about their own scope, and neither alone decides a resume. **The daemon therefore composes two predicates — `first_mismatch` over the six, and an engine-identity equality check — and a disagreement in either yields `ContinuationEpochMismatch`.** A resume is admissible if and only if both hold:

```text
P1 (compatibility epochs)   EpochSet::first_mismatch(pinned, current) == None
P2 (engine identity)        pinned.engine == current.engine
```

- **Both predicates are required.** Neither implies the other. Satisfying P1 alone permits a resume onto a different engine build, which is exactly the case plan §4.7's defect lifecycle exists to catch; satisfying P2 alone permits a resume across a semantic-epoch advance, which ADR-0018 forbids. A daemon that checks only one has not implemented resume.
- **P1 is exactly `first_mismatch`, not a re-derivation.** It returns the first disagreeing kind in `EpochKind::ALL` order (`protocol`, `semantic`, `intent`, `evidence`, `proof`, `corpus`), treats a pinned-but-absent epoch as a mismatch rather than as permission to decode best-effort (docs/09 T13), and returns none when the resume is admissible. The rejection MUST name the disagreeing kind, so "why was my resume rejected" is answerable without re-deriving it.
- **P2 is equality on `EpochIdentity`, never an ordering.** Engine identities are content identities; there is no "newer engine" relation to compare against. A daemon MUST NOT accept a resume because the current engine is believed to supersede the pinned one.
- **The protocol epoch does not participate.** A continuation MUST leave `protocol` `Unpinned` in its pinned set: the protocol epoch is connection-scoped and is consumed by no task (RFC 0030's "Which epochs enter the key" gives `protocol | never`; SD-13 removed it from snapshot identity for the same reason; the IDL's `rule versioning.handle_stability` names only engine and semantic epochs as continuation-pinned). Protocol compatibility on resume is decided by the handshake's N and N−1 window and reported as `ProtocolVersionUnsupported`. A daemon MUST NOT reject a resume with `ContinuationEpochMismatch` for a protocol-minor difference.
- **`engine` is nullable on the wire but not optional in a continuation.** `EpochSet.engine` is `nullable` because a *result* may legitimately pin no engine — a structural operation runs no engine. A *continuation* whose `engine` is null is malformed. The IDL cannot express a presence marker conditioned on the carrier, so the obligation lives here.

This composition is stated identically in RFC 0030's "Resume decision" and closes that RFC's F6. The two documents MUST be revised together.

#### Resume decision table

`task.resume` and `repair.resume` validate continuation identity, snapshot, and epochs. The typed outcomes are distinct (INV-008) and MUST NOT be collapsed:

| Condition | Error |
|---|---|
| the pinned snapshot is no longer the current sealed snapshot | `StaleSnapshot` |
| a pinned epoch disagrees with the daemon's current epoch of that kind (P1 fails) | `ContinuationEpochMismatch` |
| a pinned epoch names an identity the daemon no longer holds at all | `EpochUnsupported` |
| the pinned engine identity disagrees with the daemon's engine identity (P2 fails) | `ContinuationEpochMismatch` |
| the continuation's protocol major falls outside the served window | `ProtocolVersionUnsupported` |

- **Resume never silently re-runs.** A mismatch is a typed rejection; the daemon MUST NOT quietly restart the task under current epochs.
- **Resume is monotone.** Resume MAY add evidence; it MUST NOT replace prior artifacts under the same identity (INV-009). The frontier after resume MUST include the frontier before it.
- **Continuations are forked across an epoch advance, never migrated in place** (plan §4.6). A fork produces a new continuation with a new identity; the original remains valid for the epochs it pinned.
- `rule task.resume` governs `repair.resume` identically. The two differ only in what they return, not in what they validate. `StaleSnapshot` is admissible for both even though neither operation's `errors` clause lists it and neither takes a non-null envelope `snapshot`; this is an internal IDL contradiction and `rule task.resume` governs.

### Epoch advance

- An epoch advance MUST publish `EpochAdvanceNotice` — the per-artifact-class `Compatibility` map (`Preserved | Revalidate | Incompatible`) and the estimated `blast_radius`, both keyed by plan §4.4 class prefix — **before** the advance is applied. A client reads pending notices from `ServerWelcome.pending_advances`; a notice that first becomes observable after the advance has been applied violates this rule, and is not merely a late notification.
- The daemon MAY serve at most two epochs of a kind during a migration; new work defaults to the newest. A continuation resumes only under its pinned epoch; a payload declaring an epoch the daemon does not implement MUST be rejected with `EpochUnsupported`, never best-effort decoded (docs/09 T13).
- An advance MUST NOT mutate a published artifact and MUST NOT change what a published receipt claims. Re-derived artifacts get new identities linked by `SUPERSEDES` edges.
- `EpochAdvance` and `Compatibility` in `crates/continuum-value/src/epoch.rs` are the typed forms of the notice; an advance that does not change the epoch identity is rejected there and MUST NOT be announced here.

## Error taxonomy

`ErrorCode` is `@open`: adding a code is a *minor* change, because a client is required to treat an unrecognized code as a **non-retryable typed failure** and MUST NOT infer semantics from it. Removing a code, or changing what an existing code means, is breaking. The twenty codes defined at protocol 3.0 are therefore the set as of this version, not a closed set for all time. Any restatement of the taxonomy as fifteen codes predates the five added in review 5 (`AcceptanceChainInvalid`, `StatusConflict`, `QuotaExhausted`, `EpochUnsupported`, `PublicationAborted`) and is stale.

| Code | Class | Means | Identical retry can succeed? |
|---|---|---|---|
| `StaleSnapshot` | input | the named snapshot is not current, or is not sealed where sealing is required | no — re-seal or re-base |
| `UnsupportedSemanticFeature` | engine capability | the request needs a semantic feature this daemon does not implement; also the registered-but-unshipped surface | no |
| `IntentMutationDenied` | authorization | a protected Intent Contract field was mutated without `revise-intent`, or against an intent lock (plan §5.4) | no |
| `InsufficientEvidence` | evidence | the evidence present does not meet the required assurance class | no — produce evidence |
| `BudgetExhausted` | resource | task-budget spend was exhausted; carries `continuation`, or a typed `non_resumable_reason` | no — resume with a larger budget |
| `ContinuationEpochMismatch` | epoch | P1 or P2 of the resume predicate failed | no |
| `AmbiguousCorrespondence` | semantics | a source construct maps to more than one semantic element (plan §16) | no |
| `UntrustedDomainBoundary` | authorization | the request crosses a trust boundary the Intent Contract does not authorize (plan §18) | no |
| `CertificateRejected` | integrity | an independent checker rejected a certificate or proof artifact | no |
| `ReplayDiverged` | integrity | a replay diverged from the recorded execution; emits a `defect_*` | no |
| `CapabilityDenied` | authorization | the presented capability's level or scope does not permit the operation | no |
| `PolicyGateFailed` | policy | a promotion gate or intent policy blocked the operation | no |
| `AcceptanceChainInvalid` | integrity | an intent bundle's acceptance signature chain failed, or the bundle is absent; CI fails closed | no |
| `StatusConflict` | concurrency | an evidence-graph status promotion lost its compare-and-set | **yes** — re-read and retry |
| `QuotaExhausted` | resource | a capability's concurrency or resource quota is exhausted | **yes** — later, or with fewer concurrent tasks |
| `EpochUnsupported` | epoch | an artifact, page token, or continuation declares an epoch this daemon does not implement | no |
| `PublicationAborted` | atomicity | an atomic publication aborted; nothing published, nothing truncated | **yes** — same idempotency key |
| `ProtocolVersionUnsupported` | protocol | no version is common, or the client's major is outside the N / N−1 window | no |
| `IdempotencyKeyReused` | protocol | the same key was replayed with a different canonical request | no |
| `MalformedRequest` | protocol | the request does not parse, does not validate against the IDL, or uses an unknown closed-enum member | no |

The last column is guidance for reading the taxonomy; the authoritative per-occurrence value is the **required** `Error.retryable` field, which a daemon MUST set on every error it returns.

### Error structure

`Error` carries `code` (required), `detail` (required — a stable, non-interpolated explanation of the code in context), `data` (optional — typed, machine-readable specifics whose shape is determined by `code`), `recovery` (**required**), `continuation` (optional), `non_resumable_reason` (optional), and `retryable` (required).

- **`recovery` is required, and an empty list is a statement.** It says "no typed recovery exists from this state", which differs from not having been computed. `recovery` is the *only* recovery channel: a list of allowed operations with pre-filled arguments, never free-form commands, never shell, never interpolated source, log, or model text (INV-016).
- `detail` explains the *code*, not the user's program. A daemon MUST NOT embed source text, log lines, or model output in it.
- `data` is typed by `code`. It is not an escape hatch for prose (INV-003).

### The common error union

`rule errors.common` fixes what an operation's `errors` clause means. Every operation MAY *additionally* return:

- always: `MalformedRequest`, `ProtocolVersionUnsupported`, `CapabilityDenied`, `QuotaExhausted`, `EpochUnsupported`;
- if `@mutation`: `IdempotencyKeyReused`, `PublicationAborted`;
- if it takes a non-null `snapshot`: `StaleSnapshot`.

An operation's `errors` clause lists the codes it may return **beyond** these, and a daemon MUST NOT return a code outside that union for the operation. Three operations — `workspace.seal`, `task.status`, and `task.subscribe` — declare an empty `errors` clause, which means exactly "this operation adds nothing to the common union", not "this operation cannot fail".

### Semantic separations that MUST NOT be blurred

- **`BudgetExhausted` is never a semantic verdict** (docs/49). It is task-budget spend, and it carries the continuation when one exists. A campaign that ran out of budget has not refuted anything.
- **`QuotaExhausted` is not `BudgetExhausted`.** Quota is a capability's concurrency or resource ceiling (`ServerLimits.max_concurrent_tasks` and the quotas a grant carries); budget is task spend. The daemon MUST NOT substitute one for the other, and neither is ever a semantic verdict.
- **`EpochUnsupported` is not `ContinuationEpochMismatch`.** The first says "I cannot read this at all"; the second says "I read it and it does not apply here". Collapsing them destroys the distinction between an unreadable artifact and an inapplicable one.
- **`CapabilityDenied` is not a not-found.** See "Existence oracles and cross-principal sharing".
- **An engine defect is never a semantic verdict.** See "Engine defects".

## Operational contract on the wire

Absorbed from plan §4.5 (operational contract), §4.6 (epoch advance) and §4.7 (engine-defect lifecycle) — SD-09. The daemon-side obligations are [docs/35](../docs/35_CONTINUUMD_WORKBENCH_DAEMON.md) and the epoch decision is [ADR-0018](../adr/0018-semantic-versioning-and-replay.md); this section is only what crosses the wire, and where it and plan §4.5–§4.7 disagree this RFC is corrected and becomes normative (plan §25).

### Redacted values

Content can stop being readable in three ways — summarized after promotion, purged by key shred, or lost across a restore — and all three surface identically to a client.

- A value the daemon cannot return in full MUST be returned as the typed `Redacted` stub of the IDL ([`../schemas/redacted.schema.json`](../schemas/redacted.schema.json)): `redacted: true`, `reason`, `commitment`, `original_class`. Omitting the field, returning null, or returning an empty value MUST NOT be used to represent redaction — a client MUST be able to tell "withheld" from "absent" structurally, without inference.
- `reason` is a closed vocabulary: `summarized | purged | lost`. A daemon MUST NOT extend it; a new way to lose content is a protocol change, not a new string.
- The plan writes this value `Redacted(reason, commitment)`. That names the semantic pair; the normative wire and schema form carries four fields, and the two extra fields are load-bearing (`redacted` makes a stub structurally recognizable, `original_class` names what class of evidence is missing without a dereference).
- The declared wire sites are `ArtifactRef.redacted` in the result envelope's `artifacts` list and `evidence.get`'s `redacted` field. A redacted artifact still appears in `artifacts` with its `kind` and `handle`; it is not removed from the list.
- Every redacted value MUST also appear in the result's `omissions` manifest with reason `redaction` (INV-007), and any claim that required the hidden data MUST downgrade in the `assurance` envelope per plan §18.4. A verdict MUST NOT be reported at full strength over a redacted input on the grounds that the input "would have" supported it.
- `evidence.verify` over a receipt whose referenced content is redacted MUST return the structural verification result *and* the redaction. It MUST NOT return a bare failure (the receipt is intact) and MUST NOT return a bare success (the claim is no longer fully supported). The IDL's `evidence.verify` response declares no field able to carry the redaction; until that is paid this obligation is unimplementable as declared, and the gap is flagged below.

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

- A `ReplayDiverged`, a parity mismatch, an engine crash, or an explanation-validation failure MUST cause a `defect_*` artifact to be published, and the result that reports the condition MUST carry that handle in `artifacts`. A client MUST NOT have to reconstruct a defect report from an error string. The IDL declares `DefectHandle` and surfaces it only on `query.clean_compare`, states no emission rule, and gives `program.replay` no defect field; this obligation governs in the interim, and the gap is flagged below.
- A `defect_*` pins every input by content identity, the semantic and proof epochs, the engine identity, and a minimized reproduction, under the same redaction policy as Context Packs (plan §18.4).
- An engine defect is never a semantic verdict about the client's program. The task reports typed inconclusiveness (INV-008) — `EngineError` where the engine failed, `IncompleteProofSearch` where search did not close — and the `assurance` envelope names the dimensions it could not establish.
- `query.clean_compare` is the on-demand parity audit and returns `defect: DefectHandle` when parity fails. It MUST NOT prefer either lane's result (RFC 0030).

## Transport, encoding, authentication

- Local IPC (unix socket / named pipe) first; authenticated HTTP/QUIC for shared/remote modes.
- Two encodings, `canonical_json` and `canonical_cbor`, with **identical canonical field order**; exactly one is negotiated per connection. JSON is the debugging encoding and CBOR the performance encoding, and a golden trace exists in both (`rule conformance.golden_traces`). An implementation MUST NOT define a third encoding without a protocol-major change.
- Canonical string form is NFC; a daemon MUST reject non-canonical spellings of identity-bearing strings (ADR-0013). `U64` is a JSON number only when exactly representable and a decimal string otherwise; `Bytes` is base64url without padding in JSON and a byte string in CBOR.
- Remote mode requires an identity model. What a capability confers is the IDL's `CapabilityDescriptor`: actor, authority level, snapshot and intent scope, artifact-class scope, expiry, and delegation depth. Possession of an artifact handle never implies authorization (ADR-0037): authorization is checked below the adapter, independently of handle possession, before any semantic work runs.
- `cap_*` is the one plan §4.4 class that is not content-addressed. Capability tokens are minted randomly and do confer authority, so they are secrets: they MUST NOT be logged in request traces, MUST NOT appear in error text or in `next_operations` arguments, and have no content-derived store path (`crates/continuum-workspace/src/artifact_path.rs` refuses to derive one). Revocation, not purge, is how a capability stops being usable.

## Capability administration

Plan §4.5 requires capabilities to be "minted, scoped, delegated, and revoked through daemon operations recorded in the audit log", and plan §10.2's registry — which the IDL implements exactly — contains no such operations. This RFC closes the gap by **declaring capability administration an out-of-band administrative surface for protocol major 3**, and by fixing the obligations that hold wherever it lives.

### The decision, and why

Capability administration is **not** part of the native protocol's operation registry at major 3. Three reasons, in order of weight:

1. **There is no authority level that can hold it.** RFC 0027's ladder is `read < propose < execute < revise-intent < promote`, and each of the 72 operations carries exactly one minimum level so that `CapabilityDenied` is decidable from the table. Minting a grant is not a repair promotion and is not an intent revision; it belongs to neither top level. Adding a sixth level to accommodate one namespace would change `AuthorityLevel`, every `authority` clause, and RFC 0027's table — a breaking protocol change with a far larger blast radius than the gap it closes.
2. **A mint operation on the agent-facing connection is a privilege-escalation surface.** The capability authorizing a mint would itself be presented in `ClientHello.capability`, so a compromised agent connection would be one operation away from a wider grant. Keeping administration out-of-band gives the native protocol a property worth stating outright: **no operation in this protocol can widen the authority of the connection that invokes it** (INV-015, agent least authority).
3. **The descriptor already fixes the vocabulary.** `CapabilityDescriptor` declares what such operations manipulate, and `ServerWelcome.grant` is how a client learns the result. Nothing on the wire is missing; only the administrative verbs are, and they are precisely the verbs that should not be reachable from an agent's connection.

### What is not out of band

The following obligations hold regardless of where the administrative surface lives, and were never conditional on this decision:

- **Every mint, scope, delegate, and revoke MUST be recorded in the audit log** with actor, capability, inputs, policy decision, outputs, and evidence identity (plan §4.5, §18.5). The audit record survives the capability; what was granted, by whom, and under what authority is not itself revocable.
- **The sharing policy that enables cross-principal reuse is a capability property, not a daemon-global flag** — a global flag cannot be scoped, delegated, or revoked.
- **`ServerWelcome.grant` is the only authority channel.** A client MUST NOT infer authority from any other observation.
- **Revocation takes effect on in-flight connections.** A request presenting a revoked capability MUST fail with `CapabilityDenied` on the next request after revocation; a daemon MUST NOT serve a connection from a grant cached at handshake time. Revocation MUST NOT delete or invalidate any published artifact, and MUST NOT retroactively unmake a result the capability lawfully produced (INV-009). A task already running under a revoked capability is cancelled or completed per deployment policy, but its results are readable only under a capability that still authorizes them.
- **Revocation, not purge, ends a capability's life** (docs/35): `cap_*` is not content-addressed, is not GC'd by reachability, and is not purged by key shred.

### Delegation and expiry defaults

The IDL declares `delegation_depth` and `expires_at` and states no defaults. This RFC fixes them in the fail-closed direction:

- **`delegation_depth` defaults to `0`** — no delegation — and a delegated capability's depth MUST be strictly less than its parent's.
- **A delegated capability MUST NOT exceed its parent**: its `level` MUST be at or below the parent's, its `snapshots`, `intents`, and `artifact_classes` scopes MUST be subsets (an empty parent scope means unrestricted, so a non-empty child scope is always a narrowing), and its `expires_at` MUST NOT be later than the parent's.
- **`expires_at: null` means non-expiring** and MUST be confined to deployments where daemon and client share a trust domain (local IPC, a single principal). In remote mode a non-expiring grant MUST NOT be issued; a mint that names no expiry MUST be rejected rather than defaulting to non-expiring.
- **Revoking a capability revokes its delegation subtree.** A child grant MUST NOT outlive its parent.

### If the decision is revisited

Bringing capability administration into the protocol at a future major requires, together and in one change: entries in the plan §10.2 registry; an authority row per operation in RFC 0027's table (which entails deciding point 1 above); IDL operation declarations carrying `@privileged @audit_recorded`; and either a re-statement of point 2's escalation property or an explicit acceptance of its loss. The registry edit is flagged below; this RFC does not edit plan §10.2.

## Conformance and acceptance

### Golden wire vectors

A conforming implementation MUST ship golden request/result traces pinned **per protocol version, in both encodings**, covering: every one of the 72 operations; every error code; idempotency replay (identical and conflicting); restart and resume with matching and mismatched epochs; cancellation at every instrumented phase; deterministic pagination across page boundaries; and an unsupported empty task whose result still names every epoch.

A golden vector is a byte sequence, not a shape: the JSON and CBOR vectors for one exchange MUST decode to the same values, and re-encoding either MUST reproduce it byte for byte. A vector that only round-trips through a permissive parser is not a golden vector.

### Malformed input

Malformed-input fuzzing runs on **both** encodings and MUST produce a typed error, never a panic, never a partial application, and never a success. The required malformed classes and their expected codes:

- a request naming a `protocol_version` other than the negotiated one ⇒ `ProtocolVersionUnsupported`;
- an unknown member of a *closed* enum ⇒ `MalformedRequest` (an unknown member of an `@open` enum is surfaced verbatim, not rejected);
- an `arguments` payload that does not validate against the operation's request struct ⇒ `MalformedRequest`;
- a `required` field absent or null ⇒ `MalformedRequest`; a `nullable` field absent ⇒ `MalformedRequest` (null is required to be *present*); an `optional` field emitted as null ⇒ `MalformedRequest`;
- an unknown `optional` request field ⇒ ignored, request served;
- a `@mutation` request with no `idempotency_key`, and a `@readonly` request carrying one ⇒ `MalformedRequest`;
- a `@task_starting` request with no `budget` ⇒ `MalformedRequest`;
- an `Opaque` payload whose `schema_epoch` the daemon does not implement ⇒ `EpochUnsupported`, never best-effort decoding;
- a `page_token` minted under different epochs ⇒ `EpochUnsupported`;
- a non-NFC identity-bearing string ⇒ `MalformedRequest` (ADR-0013);
- a `U64` beyond exact JSON representation encoded as a number ⇒ `MalformedRequest`.

### Acceptance

Beyond the golden traces and fuzzing above: idempotency replay tests; restart/resume with epoch mismatches; cancellation at every instrumented phase; deterministic pagination; adapter parity (the same operation through CLI/MCP/LSP yields identical artifacts).

For the continuation and capability rules stated here:

- **two-predicate resume** — a matched pair resumes; a changed semantic epoch fails `ContinuationEpochMismatch` naming `semantic`; **a changed engine identity with all six epochs matching fails `ContinuationEpochMismatch`** — a required corpus row, because a one-predicate implementation passes every other resume test; an epoch identity the daemon no longer holds fails `EpochUnsupported`; a protocol-minor difference resumes; a continuation created without engine identity, or without an epoch its task consumed, is rejected at creation;
- **capability administration** — a revoked capability fails `CapabilityDenied` on the next request over an already-open connection; a delegated capability cannot exceed its parent in level, scope, expiry, or depth; revoking a parent revokes its subtree; no operation in the registry widens the authority of the connection invoking it;
- **envelope conformance** — every `@mutation` request without an idempotency key is rejected and every `@readonly` request carrying one is rejected; every `@task_starting` request without a budget is rejected; every `semantic`/`evaluation` verdict carries nine assurance dimensions; every result names six epochs; `absent` and `null` are never interconverted on any round-trip.

For the operational contract absorbed above:

- redaction round-trips — a summarized, a purged, and a lost artifact each read back as the typed stub with the right `reason`, appear in `omissions`, and downgrade the assurance envelope; `evidence.verify` over the referencing receipt returns structural success plus the redaction;
- existence-oracle tests — an unauthorized read of an artifact that exists and of one that does not produce byte-identical envelopes, and publishing content already held by another principal produces the same envelope and `cost` as a first publication;
- epoch-advance ordering — the notice, its per-class compatibility map, and its blast radius are observable in `ServerWelcome.pending_advances` before the advance is applied; a resume across it fails `ContinuationEpochMismatch` and an unknown epoch fails `EpochUnsupported`;
- defect emission — every injected `ReplayDiverged` and parity mismatch yields a result carrying a `defect_*` handle and a typed inconclusive assurance envelope, never a semantic verdict.

## Corrections recorded by this RFC

Per plan §25, where plan prose, docs, or a dependent artifact disagrees with this RFC, this RFC governs — except for wire shapes, where the IDL governs and this RFC is corrected. The corrections in force, each with its direction.

### Where the IDL corrected this RFC

1. **`page_size` and `page_token` are envelope fields, and pagination is `@paginated`-only.** The previous revision wrote "List-returning operations accept `page_size` and an opaque `page_token`", which both misplaces the fields and over-generalizes the rule. Normative: they are the two fields of `Page`, carried in `RequestEnvelope.page`; `next_page_token` is a result-envelope field; and the rule binds the six `@paginated` operations, not every operation returning a list. Direction: the IDL decides; this RFC is corrected.
2. **`budget` is required by an annotation, not by a judgement about length.** The previous revision's envelope table gave `budget` as required "for long ops". Normative: REQUIRED for every `@task_starting` operation, optional otherwise; "long" is not a wire property. Direction: the IDL decides; this RFC is corrected.
3. **Task-starting and mutating are orthogonal, so not every task-starting operation is idempotent.** The previous revision wrote that "task-starting operations … are idempotent under idempotency keys". Four operations — `workspace.diff`, `verification.await`, `model.compare`, `failure.explain` — are `@readonly @task_starting` and carry no idempotency key at all. Normative: `idempotency_key` is required by `@mutation`, and only by `@mutation`. Direction: the IDL decides; this RFC is corrected.
4. **The result envelope has fifteen fields.** The previous revision's table carried twelve rows: it merged `task` and `continuation` into one, omitted `next_page_token` and `payload` entirely, and described `artifacts` as a "handle list". Normative: `artifacts` is `list<ArtifactRef>` carrying `kind`, `handle`, an optional `commitment`, and an optional `Redacted`; `payload` is the operation's response struct and is `nullable`; `next_page_token` is the pagination cursor. Direction: the IDL decides; this RFC is corrected.
5. **`recovery` is required and `retryable` exists.** The previous revision wrote that an error "MAY carry `recovery`". Normative: `Error.recovery` is `required` — an empty list is the typed statement that no recovery exists — and `Error.retryable` is `required` on every error. Direction: the IDL decides; this RFC is corrected.
6. **The error set is open, not closed.** The previous revision presented the codes as "stable codes" with no extensibility statement, which reads as a closed enum. Normative: `ErrorCode` is `@open` per `rule versioning.error_codes`; adding a code is a minor change and a client MUST treat an unrecognized code as a non-retryable typed failure. Direction: the IDL decides; this RFC is corrected.
7. **`epochs.protocol` is `required`, not nullable.** The previous revision said every epoch "is named and reads null" when unpinnable, which is right for five of the six and wrong for `protocol`: a served connection always has a negotiated version. Normative: `protocol` is `required`; `semantic`, `intent`, `evidence`, `proof`, `corpus`, and `engine` are `nullable`. Direction: the IDL decides; this RFC is corrected.
8. **Idempotency retention is `ServerLimits.idempotency_retention_ms`, read from `ServerWelcome`.** The previous revision referred to a retention window "declared in server capabilities", a structure that does not exist. Normative: the field is `ServerLimits.idempotency_retention_ms`, default `86_400_000` ms, delivered once per connection in `ServerWelcome.limits`. Direction: the IDL decides; this RFC is corrected.
9. **The handshake is not an operation and is not in the registry.** The previous revision described negotiation only as behavior at "connection open". Normative: `ClientHello` and `ServerWelcome` are connection-level frames preceding any `RequestEnvelope`; they are outside the plan §10.2 registry and outside RFC 0027's authority table, and they fix version, encoding, grant, limits, and epoch position. Direction: the IDL decides; this RFC absorbs the shape and states the obligations.
10. **Ordering determinism is byte-identity against snapshot *and* epochs.** The previous revision said "two identical requests against the same snapshot return identical pages". Normative: `rule ordering.deterministic` requires identical *bytes* under the same encoding for identical requests against the same snapshot *and* epochs. Direction: the IDL decides; this RFC is corrected and extended.
11. **The actor scheme is closed and has four members.** The previous revision gave `actor` as "e.g. `agent:repairer-1`, `human:bob`". Normative: `ActorId`'s pattern admits exactly `agent:`, `human:`, `service:`, and `ci:`. Direction: the IDL decides; this RFC is corrected.
12. **`request_id` has a shape.** The previous revision gave it as an unstructured "client-unique" string. Normative: `RequestId` matches `^req_[A-Za-z0-9_-]+$`. Direction: the IDL decides; this RFC is corrected.
13. **The encoding tokens are `canonical_json` and `canonical_cbor`.** The previous revision wrote "Canonical JSON … CBOR". Normative: those are the wire members of `Encoding`, negotiated most-preferred-first from `ClientHello.encodings`. Direction: the IDL decides; this RFC is corrected.
14. **`revise-intent` is the wire token; `revise_intent` is the identifier.** The IDL declares `revise_intent = "revise-intent"`. Normative: prose and adapters use `revise-intent`; generated code uses the identifier. An adapter that emits `revise_intent` on the wire is non-conforming. Direction: the IDL decides; this RFC records the mapping.
15. **An empty `errors` clause does not mean "cannot fail".** `workspace.seal`, `task.status`, and `task.subscribe` declare `errors []` (the count was corrected from two to three by the bn-mtw conformance check, 2026-08-01 — the IDL is normative and lists all three). Normative: the clause lists codes *beyond* the common union of `rule errors.common`; both operations can still return the always-admissible five, and `workspace.seal` — a `@mutation` taking a snapshot — can also return `IdempotencyKeyReused`, `PublicationAborted`, and `StaleSnapshot`. Direction: this RFC states what the IDL's clause means.
16. **`inconclusive` carries one of six typed reasons.** The previous revision referred to typed inconclusiveness only by invariant number. Normative: `InconclusiveReason` has exactly `Unsupported`, `ResourceExhausted`, `EngineError`, `InsufficientTelemetry`, `AbstractionAmbiguity`, `IncompleteProofSearch`, identical to `crates/continuum-value/src/assurance.rs`. Direction: the IDL and the crate agree; this RFC absorbs the vocabulary and binds itself to revise with the crate.
17. **There is no checker epoch.** The previous revision's engine-defect bullet said a `defect_*` pins "the semantic and checker epochs", echoing plan §4.7. `EpochSet` has no `checker` member. Normative: a `defect_*` pins the semantic and **proof** epochs and the engine identity; checker identity is the `proof` epoch for Lean-backed checkers and engine identity for native checkers. Direction: RFC 0030's correction 3 governs and this RFC adopts it; plan §4.7's phrasing is an alias.

### Where this RFC decides

18. **The continuation-resume obligation is two predicates, not one.** The previous revision said only that continuations "pin engine and semantic epochs and MUST be rejected with `ContinuationEpochMismatch` on mismatch", which does not say how the check is composed. `crates/continuum-value/src/epoch.rs` decides six compatibility epochs and explicitly excludes engine identity; the IDL's `EpochSet.engine` requires continuations to pin it. Normative: admissibility is `first_mismatch(pinned, current) == None` **and** `pinned.engine == current.engine`, and a disagreement in either yields `ContinuationEpochMismatch`. Direction: this RFC states a composition obligation neither the crate nor the IDL can state alone; it agrees token for token with RFC 0030's "Resume decision" and closes that RFC's F6.
19. **The protocol epoch does not participate in resume.** No previous revision said either way, and the permissive reading — that a continuation pins `protocol` and `first_mismatch` compares it — would reject a resume across a compatible minor bump. Normative: a continuation leaves `protocol` `Unpinned`; protocol compatibility on resume is the handshake's N and N−1 window, reported as `ProtocolVersionUnsupported`. Direction: this RFC decides, consistently with RFC 0030's "protocol | never" key rule, SD-13's removal of the protocol epoch from snapshot identity, and the IDL's `rule versioning.handle_stability`, which names only engine and semantic epochs as continuation-pinned.
20. **Capability administration is out-of-band at major 3.** This RFC's open question 1 and the IDL's open item 1 left the choice between registry entries and an out-of-band declaration. Normative: out-of-band, with the audit, descriptor, revocation, and delegation obligations of "Capability administration" holding regardless. Direction: this RFC decides, closing IDL open item 1.
21. **Delegation and expiry defaults are fail-closed.** The IDL's open item 2 records that `delegation_depth` and `expires_at` have no declared defaults. Normative: `delegation_depth` defaults to `0`; a delegated capability never exceeds its parent in level, scope, expiry, or depth; `expires_at: null` is confined to a shared trust domain and MUST NOT be issued in remote mode. Direction: this RFC decides, closing IDL open item 2 in the restrictive direction.
22. **`rule task.resume` governs the resume error set.** `task.resume` and `repair.resume` declare `errors` clauses omitting `StaleSnapshot`, and neither takes a non-null envelope `snapshot`, so `rule errors.common` does not supply it either — yet `rule task.resume` requires it. Normative: `StaleSnapshot` is admissible for both; the rule governs and the clauses are incomplete. Direction: this RFC names the governing reading of an internal IDL contradiction; the minimal additive fix is flagged below.
23. **An adapter may rename, but only under a declared total mapping.** Neither the IDL nor any prior revision said whether an adapter's own naming convention is a parity violation. Normative: renaming is permitted as a spelling under a declared total mapping to `OperationName`; exposing a name with no declared operation behind it is not. Direction: this RFC decides, refining `rule conformance.adapter_parity`.

### Where this RFC governs the plan and docs

24. **The taxonomy has twenty codes, not fifteen.** docs/36 and docs/55 both state "15 stable typed codes" and enumerate the pre-review-5 set. Normative: twenty, the five additions being `AcceptanceChainInvalid`, `StatusConflict`, `QuotaExhausted`, `EpochUnsupported`, `PublicationAborted`. Direction: RFC governs docs/36 and docs/55, which agree with neither the IDL nor plan §10.3.
25. **`completed` is not a `ResultStatus`, and a verdict is not a scalar.** docs/36's result-envelope example carries `"status": "completed"` (a `TaskStatus` member), `"verdict": "refuted"` as a bare string, `next_operations` as a list of operation names, an `artifacts[].kind` of `"context_pack"` where the value is the plan §4.4 prefix without its underscore (`"ctx"`), and an empty `epochs` object where all six MUST be named. Normative: as stated in "Result envelope". Direction: RFC governs docs/36; the example is not a valid `ResultEnvelope`.
26. **`"source"` is not an `ExpansionRelation`.** docs/36's worked example calls `context.expand` with `relation="source"`; `ExpansionRelation` is closed and the member is `source_span`. Normative: closed-enum members are used verbatim. Direction: RFC governs docs/36.
27. **A node ceiling is an `OutputPolicy` member, and `context.expand` has no `budget` argument.** docs/36's expansion example passes `max_nodes` inside `budget`. Under `rule versioning.compatible_change` an unknown key inside `budget` is ignored, so the example silently loses its bound. Normative: node ceilings are `output_policy.max_nodes`. Direction: RFC governs docs/36.
28. **The IDL technology choice is decided, and the generation targets are Rust and TypeScript.** docs/36's conformance list asks for "JSON Schema/OpenAPI or equivalent IDL" and names Python among the generated clients. Normative: a custom textual IDL, with JSON Schema as a *generated* artifact of it, and Rust and TypeScript clients. Direction: RFC governs docs/36; the question was closed in this RFC's open questions and in the IDL's "Notation".
29. **docs/46's adapter surfaces name operations that do not exist.** The DAP request `continuum/fairnessLedger`, and the MCP tools `get_context_pack` and `verify_repair`, have no declared operation behind them — `fairness` is an assurance dimension, packs come from `context.compile`/`context.expand`, and the gate-evaluation operation is `repair.evaluate` (`evidence.verify` is a different operation). `start_forge` names a verb the registry does not have; the operation is `forge.create`. Normative: correction 23's mapping rule; the four surfaces are non-conforming until mapped or removed. Direction: RFC governs docs/46.
30. **docs/46's versioning list is not the epoch set.** It tracks a native protocol version, an adapter protocol version, a semantic epoch, a schema version, and a fused "proof/checker epoch", omitting `intent`, `evidence`, `corpus`, and engine identity. Normative: six independently versioned epochs plus engine identity; `schema_epoch` is a per-payload header and not a seventh; there is no checker epoch (correction 17); and adapters have no protocol version of their own, because they define no semantics. Direction: RFC governs docs/46.
31. **A "generic artifact path" is not a fallback for an unknown epoch.** docs/46 requires an adapter to expose a generic artifact path rather than silently drop fields. Normative: the not-dropping half is right and the permissive half is wrong — an artifact whose declared epoch the reader does not implement is rejected with `EpochUnsupported`, never rendered generically (docs/09 T13). Direction: RFC governs docs/46.
32. **docs/55 is not a complete projection.** It documents 64 of the 72 operations, omitting the whole `correspondence` and `observe` namespaces plus `task.update_budget` and `evidence.subscribe`; its "full prefix set" lists 17 of the 19 registered handle classes, omitting `inb_` and `defect_`; and its common-result rules name `snapshot`, `intent`, `budget`, and an audit-correlation field, none of which exists in `ResultEnvelope`, while naming three of the seven `EpochSet` members. Normative: 72 operations, 19 handle classes, and the result envelope of this document. Direction: RFC governs docs/55, which is due a regeneration.
33. **`intent.lock` edits a policy table; it does not set a boolean lock.** docs/55 describes it as applying an intent lock. Normative: the request and response both carry the plan §5.4 field-to-verb `policy` map, drawn from RFC 0037's closed verb set. Direction: RFC governs docs/55.
34. **There is no draft status.** docs/55 speaks of drafts gaining protection on acceptance. Normative: the state is `Proposed`; `EvidenceStatus` states outright that there is no `draft` status, and `intent.accept` is the only transition out of `Proposed`. Direction: RFC governs docs/55.
35. **docs/35's engine-identity correction quotes a disagreement that no longer exists, and its own body still commits it.** docs/35's corrections table attributes the phrase "engine epoch" to plan §4.7 and to this RFC's result envelope; neither document contains that token — both say "engine identity". Meanwhile docs/35's task-identity block still groups `engine` with the semantic and proof epochs under the word "epochs", the grouping its own correction forbids, and docs/55 repeats it. Normative: `engine` is engine identity and is never grouped under "epochs". Direction: RFC governs both documents; docs/35's corrections row is right in substance and wrong in its citation.
36. **`continuum doctor` is not a protocol operation.** plan §4.7 and docs/35 name it as the defect-bundle assembler; it is in neither the 72-operation registry nor any adapter mapping. Normative: it is a local tool over published `defect_*` artifacts, not a wire operation, and it MUST NOT acquire semantics the protocol does not declare. Direction: RFC governs; if bundle assembly needs a wire surface it is a registry addition, not an implicit one.

### Flags raised against artifacts this RFC does not own

F4, F5, F6, F7, and F12 were paid by the IDL 1.1 revision (bn-2wu5j,
2026-07-31); their entries are retained as the record of what was flagged
and why.

- **F1 — the IDL cannot express presence conditioned on the carrier.** `EpochSet.engine` must be `nullable` for a result and non-null for a continuation; `idempotency_key` must be present for `@mutation` and absent for `@readonly`; `budget` must be present for `@task_starting`. All three are stated here as obligations because a presence marker is a property of the field, not of the context. A `@required_when(annotation)` form would make them generator-checkable; raised for the protocol sweep.
- **F2 — `intent.lock` types the policy table as `map<String, String>`.** RFC 0037 fixes a closed eight-member verb set (`unlocked`, `proposal-only`, `review`, `locked`, `no-decrease`, `no-removal`, `no-downgrade`, `no-expansion`) and a closed fifteen-member field set, and neither is wire-enforced. INV-003 forbids prose-only machine interfaces. Mirrors RFC 0031's F4 and RFC 0030's F2.
- **F3 — four `Opaque` fields name no schema at all.** The IDL's open item 3 separates three groups; the third — `frontier`, `delta`, `state`, `classification` — has no declared shape in `schemas/` and no struct in the IDL, so `rule envelope.no_prose` is unenforceable for them.
- **F4 — the IDL's open items 1 and 2 are stale as of this revision.** Corrections 20 and 21 close them. The IDL's comment block still records both as open; updating it is a comment-only edit that should advance `idl_version`. This RFC does not edit the IDL.
- **F5 — `task.resume` and `repair.resume` cannot return `StaleSnapshot` under their declared error sets**, though `rule task.resume` requires it (correction 22). The minimal additive fix is adding `StaleSnapshot` to both `errors` clauses — a compatible change under `rule versioning.compatible_change`.
- **F6 — `evidence.verify` has no field able to carry a redaction.** Its response is `evidence`, `status`, `evidence_kind`, `checker`, `validation_basis`; `evidence.get` has `redacted: Redacted optional` and `evidence.verify` does not. The SD-09 obligation that `evidence.verify` return structural success *plus* the redaction is unimplementable as declared. The minimal additive fix is one `optional` field.
- **F7 — defect emission has no `rule`, and `program.replay` has no defect field.** `DefectHandle` is declared and surfaced only on `query.clean_compare`, while `program.replay` lists `ReplayDiverged` among its errors and returns no defect handle. plan §4.7 and this RFC require a `defect_*` on every divergence; the IDL does not.
- **F8 — three obligations of this RFC have no IDL counterpart.** Cross-actor idempotency-key replay behaving as an unused key; `cap_*` tokens never appearing in traces, error text, or `next_operations` arguments; and cost-block indistinguishability under cross-principal dedup. All three are security properties, and none is checkable from the IDL today.
- **F9 — a continuation has no wire struct.** `ContinuationHandle` is a handle; what a continuation pins is prose here and in RFC 0030 and is not machine-checkable. A `Continuation` struct carrying the pinned `EpochSet`, engine identity, snapshot, and frontier commitment would make the two-predicate obligation generator-checkable.
- **F10 — `Warning.code` and `Diagnostic.code` are unconstrained strings.** Both are described as typed and neither draws from a registered code set, so two daemons can spell the same warning differently and a client cannot switch on it.
- **F11 — `Error.retryable` carries no retry-after.** `QuotaExhausted` and `StatusConflict` are the two codes where an identical retry can succeed, and neither can tell a client *when*. A typed backoff hint belongs in `Error.data`; its shape is undeclared.
- **F12 — the IDL misattributes `ExplanationLevel`.** Its common-vocabulary preamble credits docs/55 jointly with RFC 0039 for the enum; docs/55 names neither the enum, the `level` argument, nor any member. The source is RFC 0039 alone, as the `ExplanationLevel` declaration itself correctly says. Comment-only; raised with F4.
- **F13 — a future capability-administration namespace needs a registry edit.** Should the decision of correction 20 be revisited, plan §10.2 and RFC 0027's authority table must gain the entries together with the IDL declarations. Raised for the registry owners; this RFC does not edit plan §10.2.

## Rejected alternatives

- **Session-scoped implicit state.** Rejected (INV-002): a dropped connection must not change meaning.
- **Adapter-defined semantics.** Rejected (ADR-0042): MCP/LSP/DAP translate; they do not decide.
- **Free-form recovery suggestions in errors.** Rejected: recovery is a list of typed allowed operations to prevent injection through error text.
- **A generic `task.start` operation.** Rejected: it would erase which subsystem is doing the work, which is exactly what a task handle has to attribute. Task starting is a property of 26 specific operations.
- **One predicate for resume, by adding `Engine` to `EpochKind`.** Rejected: engine identity is provenance, not a compatibility epoch. Admitting it would put it in `EpochKind::ALL`, therefore into every result's six-epoch obligation, therefore into RFC 0030's query-key surface — which explicitly rejects engine identity as key material because it would invalidate the whole cache on every daemon build and hide engine defects inside key churn. Two predicates are the cost of keeping those two things separate.
- **Comparing engine identities by ordering.** Rejected: content identities have no successor relation (plan §4.6). "Newer engine, so resume is fine" is precisely the reasoning the defect lifecycle exists to refuse.
- **Pinning the protocol epoch in a continuation.** Rejected: it would make a compatible minor upgrade look like a semantic epoch advance, and it would import a connection property into a task's identity — the same error SD-13 removed from snapshot identity.
- **Capability administration as protocol operations at major 3.** Rejected for the three reasons in "Capability administration"; the decisive one is that no operation in this protocol should be able to widen the authority of the connection invoking it.
- **A permissive `expires_at` default.** Rejected: a non-expiring grant issued by omission is the failure mode capability scoping exists to prevent.
- **Best-effort decoding of an unknown epoch or schema epoch.** Rejected (docs/09 T13): a typed rejection is recoverable; a misread artifact is not.
- **A `MalformedResponse` code.** Rejected: a daemon that cannot produce a valid result does not get to report that fact as a result. Malformedness is a client-side statement about a message, and the taxonomy is for failures the daemon diagnoses.

## Open questions

- ~~IDL technology choice (custom vs. an existing schema language)~~ — decided: a custom textual IDL, because three-valued field presence, per-operation authority levels, task-starting/idempotency annotations, closed per-operation error sets, and citable normative rules have no faithful encoding in JSON Schema, OpenAPI, or Protobuf; JSON Schema is a generated artifact of the IDL, not its source (see the IDL's "Notation" section).
- ~~Capability administration (mint/scope/delegate/revoke, plan §4.5) has no entry in the plan §10.2 registry~~ — decided: out-of-band at protocol major 3, with the audit, descriptor, revocation, and delegation obligations of "Capability administration" holding regardless of where the surface lives (correction 20).
- ~~Capability delegation depth and expiry defaults for multi-agent handoff~~ — decided in the fail-closed direction (correction 21). What remains open is the *numeric* expiry a deployment should choose, which is deployment policy rather than protocol, and the multi-agent handoff pattern itself (with RFC 0027).
- Whether a `Continuation` struct belongs on the wire (F9), which would make the two-predicate obligation generator-checkable rather than prose.
- Whether `Warning.code` and `Diagnostic.code` draw from a registered code set (F10), and where that registry lives.
- Whether the four shapeless `Opaque` fields (F3) gain declared schemas before PR 5 or are narrowed to declared structs.
- Whether `Error.data` gains a typed retry-after for the two retryable codes (F11).
