# Agent API Reference Sketch

> **Non-normative projection.** This document is a projection of RFC 0026
> (`continuumd` native protocol), RFC 0027 (agent tool protocol), and plan
> §10.2, and is regenerated from them. On any divergence, the RFCs and the
> plan win.
>
> Regenerated against IDL 3.3 (protocol 3.3, 73 operations in 18
> namespaces) and RFC 0026 / RFC 0027 as of bn-l4kmc. The previous
> revision covered 64 of 72 operations (whole `correspondence` and
> `observe` namespaces, plus `task.update_budget` and `evidence.subscribe`,
> were omitted) and 17 of 19 handle classes (omitting `inb_` and
> `defect_`); both are brought current here (RFC 0026 correction 32).
> Each operation below carries its `authority` level from RFC 0027's
> per-operation registry (RFC 0027 correction 24) — `read`, `propose`,
> `execute`, `revise-intent`, or `promote` — which is a registry fact, not
> itself the authority boundary: the boundary is the registry plus the
> four-test admission predicate.

This is a shape contract, not the final wire IDL.

## Handles

Full prefix set per plan §4.4, in IDL declaration order (19 classes):

```text
WorkspaceHandle      ws_...
IntentHandle         in_...
IntentBundleHandle   inb_...
ModelHandle          model_...
CausalGraphHandle    cir_...
CrashpackHandle      crash_...
ContextHandle        ctx_...
ProofArtifactHandle  proof_...
ProofStateHandle     ps_...
TaskHandle           task_...
EvidenceHandle       ev_...
DebugHandle          dbg_...
ReceiptHandle        receipt_...
RepairHandle         rt_...
ForgeHandle          forge_...
ContinuationHandle   cont_...
CapabilityHandle     cap_...
DiffHandle           diff_...
DefectHandle         defect_...
```

## Workspace

### `workspace.create`

Authority: `propose`.

Inputs: root/overlay/dependency/config references.
Output: immutable workspace handle plus diagnostics.

### `workspace.fork`

Authority: `propose`.

Inputs: base handle and patch/overlays.
Output: new handle and textual/semantic pre-diff availability.

### `workspace.diff`

Authority: `propose`.

Inputs: two handles and requested layers.
Output: diff artifact.

### `workspace.seal`

Authority: `propose`.

Seals a snapshot as immutable; sealed snapshots are the only valid semantic inputs.

## Intent

### `intent.get`

Authority: `read`.

Returns canonical Intent Contract.

### `intent.diff`

Authority: `read`.

Returns classified changes, proof obligations, and policy impact.

### `intent.propose_revision`

Authority: `revise-intent`.

Creates a proposal only; cannot mutate existing intent. The result is a
`Proposed` revision.

### `intent.accept`

Authority: `revise-intent`. Privileged, audit-recorded.

Accepts a proposed intent into the registry; the only transition out of
`Proposed`. There is no `draft` status — `EvidenceStatus` states outright
that none exists, and a proposal gains INV-001 protection only on this
explicit acceptance, never implicitly.

### `intent.reject`

Authority: `revise-intent`. Privileged, audit-recorded.

Rejects a proposed intent revision with a typed reason.

### `intent.lock`

Authority: `revise-intent`. Privileged, audit-recorded.

Edits the plan §5.4 field-to-verb policy table (`policy: map<String,
String>`), not a boolean lock: verbs are drawn from RFC 0037's closed set
and free-form policy strings are rejected. Subsequent mutation attempts
against a locked field fail with `IntentMutationDenied`.

## Verification

### `verification.start`

Authority: `execute`.

Inputs:

```json
{
  "snapshot": "ws_...",
  "intent": "in_...",
  "target": {"kind": "property", "id": "AckImpliesDurable"},
  "portfolio": "interactive|promotion|custom",
  "budget": {},
  "context_policy": {}
}
```

Outputs task handle or cached result.

### `verification.result`

Authority: `execute`.

Returns typed verdict, assurance, evidence roots, continuation, omissions.

### `verification.await`

Authority: `execute`.

Blocks (within budget) on a started verification task and returns its result.

## Model

### `model.check`

Authority: `execute`.

Checks a model against a property under intent and budget; returns typed verdict and evidence.

### `model.explore`

Authority: `execute`.

Explores model behavior under a strategy and budget; returns explored-region evidence.

### `model.compare`

Authority: `execute`.

Compares two models/configurations; returns a semantic diff artifact.

## Program

### `program.extract`

Authority: `execute`.

Extracts the semantic model from a Rust crate snapshot and annotations.

### `program.run`

Authority: `execute`.

Runs the program under controlled semantics; returns execution artifacts.

### `program.replay`

Authority: `execute`.

Replays a recorded execution deterministically; divergence fails with `ReplayDiverged`.

## Refinement

### `refinement.check`

Authority: `execute`.

Checks a refinement correspondence between program and model; returns typed verdict.

### `refinement.explain`

Authority: `execute`.

Explains a refinement result or failure with a Context Pack.

## Correspondence

Plan §16 correspondence links between source spans and semantic elements.
Omitted from the previous revision in its entirety (RFC 0026 correction 32).

### `correspondence.bind`

Authority: `propose`.

Inputs: source span and target semantic element name.
Output: correspondence commitment. Errors include `AmbiguousCorrespondence`
when a source construct maps to more than one semantic element.

### `correspondence.status`

Authority: `read`.

Inputs: optional element filter.
Output: bound/unbound/ambiguous counts for a snapshot's correspondence
coverage.

### `correspondence.drift`

Authority: `read`.

Inputs: two workspace handles (before/after).
Output: drifted element names and an optional diff handle.

## Context

### `context.compile`

Authority: `read`.

Inputs: evidence/question/audience/budget.
Output: Context Pack.

### `context.expand`

Authority: `read`.

Inputs: context, anchor, relation, budget.
Output: new immutable Context Pack referencing parent.

## Debug

### `debug.open`

Authority: `execute`.

Inputs: crashpack/execution/configuration.
Output: debugger handle and current frontier.

### `debug.state`

Authority: `execute`.

Inputs: handle and optional observer projection.
Output: current concrete/abstract state view.

### `debug.enabled`

Authority: `execute`.

Inputs: handle.
Output: enabled events at the current frontier.

### `debug.step_event`

Authority: `execute`.

Inputs: handle, event.
Output: new handle/state delta (steps one semantic event).

### `debug.step_abstract`

Authority: `execute`.

Inputs: handle.
Output: new handle/state delta (steps one abstract transition).

### `debug.reverse_causal`

Authority: `execute`.

Inputs: handle.
Output: handle at the causal predecessor.

### `debug.branch`

Authority: `execute`.

Inputs: handle and alternate enabled event/fault.
Output: sibling handle.

### `debug.compare`

Authority: `execute`.

Inputs: two handles and observer.
Output: semantic branch diff.

### `debug.why_enabled`

Authority: `execute`.

Inputs: handle, event.
Output: explanation of why the event is enabled.

### `debug.why_blocked`

Authority: `execute`.

Inputs: handle, event.
Output: explanation of why the event is blocked.

### `debug.export`

Authority: `execute`.

Inputs: handle.
Output: branch exported as a crashpack or regression scenario.

## Failure

### `failure.explain`

Authority: `execute`.

Returns a causal explanation of a failure at the requested level.

### `failure.minimize`

Authority: `execute`.

Returns a minimized counterexample preserving the failure.

### `failure.branch`

Authority: `execute`.

Explores an alternate branch from a failure's causal frontier.

## Repair

### `repair.begin`

Authority: `propose`.

Inputs: failure, base snapshot/intent, policy.
Output: transaction handle.

### `repair.apply`

Authority: `propose`.

Inputs: transaction, patch/model/proof changes, hypothesis.
Output: new transaction and candidate snapshot.

### `repair.attach`

Authority: `propose`.

Attaches additional evidence/artifacts (e.g. from another agent) to a transaction; produces a new transaction version.

### `repair.evaluate`

Authority: `execute`.

Inputs: transaction and optional budget.
Output: evidence progress/status/continuation.

### `repair.resume`

Authority: `execute`.

Resumes a suspended evaluation from its continuation; inputs and epochs are validated.

### `repair.review`

Authority: `read`.

Returns the review view of a transaction: semantic/intent diff, gate status, evidence.

### `repair.promote`

Authority: `promote`. Privileged, audit-recorded.

Returns receipt or typed policy failure.

### `repair.reject`

Authority: `promote`. Privileged, audit-recorded.

Rejects a transaction with a typed reason.

## Observe

Production-trace conformance (plan §18.2, §18.4). Omitted from the
previous revision in its entirety (RFC 0026 correction 32). Registered
ahead of its producing subsystem: until the Phase C read-only observation
lane ships, all three operations MAY fail with `UnsupportedSemanticFeature`
(`rule errors.unsupported_surface`).

### `observe.ingest`

Authority: `execute`. Audit-recorded. Additionally requires the
production-trace capability (plan §18.2).

Inputs: trace content identity, instrumentation profile.
Output: task handle or evidence handles, once ingestion completes.

### `observe.classify`

Authority: `read`.

Inputs: evidence handle.
Output: classification of an ingested trace against the model's partial
order.

### `observe.result`

Authority: `read`.

Inputs: evidence handle.
Output: conformance result for an ingested trace — typed verdict and
evidence, same shape family as `verification.result`.

## Forge

### `forge.create`

Authority: `execute`.

Inputs: snapshot, intent, sketch/holes/objectives/diversity/assurance/budget.

### `forge.step`

Authority: `execute`.

Runs one or bounded portfolio iteration; returns archive deltas and counterexamples.

### `forge.archive`

Authority: `execute`.

Queries the Pareto/quality-diversity archive by objective/descriptor/evidence status.

### `forge.materialize`

Authority: `execute`.

Creates a design transaction with model/Rust/proof artifacts.

## Proof

### `proof.goal`

Authority: `execute`.

Returns exact goal, proof Context Pack, and proof-state handle.

### `proof.attempt`

Authority: `execute`.

Applies tactic/term to a proof-state handle and returns children/diagnostics.

### `proof.check`

Authority: `execute`.

Strict isolated kernel check and receipt.

### `proof.slice`

Authority: `execute`.

Returns the proof dependency slice (curated declarations) for a goal.

## Benchmark

### `benchmark.run`

Authority: `execute`.

Runs a benchmark task under declared budget and graders; returns evidence, not self-scored results.

## Tasks

### `task.status`

Authority: `read`.

Includes semantic milestones, resource use, committed evidence, and terminal reason.

### `task.cancel`

Authority: `execute`.

Requests cancel-correct shutdown.

### `task.resume`

Authority: `execute`.

Requires continuation plus matching inputs; optional larger budget.

### `task.subscribe`

Authority: `read`. Streaming.

Streams progress events over the connection; events are hints — committed artifacts and `task.status` are authoritative.

### `task.update_budget`

Authority: `execute`.

Inputs: task handle, new budget. Raising a dimension extends the current
run; lowering a dimension below committed spend suspends the task with a
continuation, reported in the response body (not the `task_started`/
`task_suspended` result lane, because this operation is `@mutation` and
not `@task_starting`).
Output: task handle, status, budget, and an optional continuation.

## Evidence

### `evidence.get`

Authority: `read`.

Fetch typed artifact metadata or bounded content.

### `evidence.query`

Authority: `read`.

Graph query with node/edge/status/property filters.

### `evidence.verify`

Authority: `read`.

Runs relevant independent checker; does not accept client-declared status.

### `evidence.subscribe`

Authority: `read`. Streaming.

Inputs: an `EvidenceQuery` scope.
Output: the scope's current frontier at subscription time, then streamed
`EvidenceEvent` deltas referencing committed artifacts — distinct from
`task.subscribe`'s hint-only `TaskEvent` stream (`rule
subscription.hints_only`).

### `evidence.link`

Authority: `execute`. Audit-recorded. The 73rd operation (protocol 3.3,
IDL 1.4) — the only one that appends an evidence-graph edge.

Inputs: the subject evidence node, the checker's proof receipt content
identity, and the checker's tool profile.
Output: the appended `CHECKED_BY` edge, the `receipt` node it points at,
and the checker's service identity. The `checker` is never a request
field: it is read from the admitted capability, which MUST be a
`service:` actor, so an `agent:`/`human:`/`ci:` capability cannot append a
check edge at all (`CapabilityDenied`), and a checker checking its own
production is refused with `InsufficientEvidence` (INV-004's
no-self-certification rule). Writes no status; `evidence.verify` decides
that separately under RFC 0038's compare-and-set.

## Query

### `query.explain_reuse`

Authority: `read`. Paginated.

Explains which memoized results were reused for a derivation and why.

### `query.explain_invalidation`

Authority: `read`. Paginated.

Explains which results a change invalidated and through which dependency edges.

### `query.clean_compare`

Authority: `read`.

Compares incremental results against a clean recompute (Incremental Parity Audit, plan §9.5).

## Error taxonomy

Twenty stable typed `ErrorCode` members (`@open` — a future protocol minor
MAY add more, never remove or repurpose one; RFC 0026 "Error taxonomy"):

```text
StaleSnapshot
UnsupportedSemanticFeature
IntentMutationDenied
InsufficientEvidence
BudgetExhausted
ContinuationEpochMismatch
AmbiguousCorrespondence
UntrustedDomainBoundary
CertificateRejected
ReplayDiverged
CapabilityDenied
PolicyGateFailed
AcceptanceChainInvalid
StatusConflict
QuotaExhausted
EpochUnsupported
PublicationAborted
ProtocolVersionUnsupported   (protocol-level, RFC 0026)
IdempotencyKeyReused         (protocol-level, RFC 0026)
MalformedRequest             (protocol-level, RFC 0026)
```

The five added since the earlier fifteen-code count: `AcceptanceChainInvalid`,
`StatusConflict`, `QuotaExhausted`, `EpochUnsupported`, `PublicationAborted`.

Every error carries `recovery` (**required** — an empty list is the typed
statement that no recovery exists) as a list of allowed operations with
pre-filled arguments, never free-form commands, and `retryable`
(**required**). `BudgetExhausted` is never a semantic verdict; it carries
the continuation when one exists.

## Common result rules

Every result (`ResultEnvelope`) includes:

- request identity (`request_id`), echoed, and — where applicable — task
  identity (`task`);
- a typed `status` (`ok` | `error` | `task_started` | `task_suspended`)
  and, on a `semantic`/`evaluation`/`policy`/`structural` operation, a
  `verdict` as a tagged union, never a bare string;
- artifact handles (`artifacts: list<ArtifactRef>`, each carrying `kind`,
  `handle`, an optional `commitment`, and an optional `Redacted` stub);
- omissions and warnings (`omissions`, `warnings` — the INV-007/INV-016
  manifests, required and never absent; an empty list is a positive
  statement, not a missing one);
- cost (`cost`, the actual spend across the same nine dimensions as
  `Budget`);
- all six compatibility epochs (`protocol`, `semantic`, `intent`,
  `evidence`, `proof`, `corpus`), each pinned or explicitly null, plus
  `engine` — engine *identity*, carried in the same `EpochSet` struct but
  never a seventh compatibility epoch and never grouped under "epochs" in
  prose (RFC 0026 correction 35);
- valid next operations (`next_operations: list<NextOperation>`, structs
  with pre-filled `arguments`, never bare operation names);
- audit correlation (`audit`, 3.1+; **required** on every result of an
  `@audit_recorded` operation and on every `CapabilityDenied`, derived
  from the request identity alone).

The result envelope carries **no** `snapshot`, `intent`, or `budget`
field — those are request-side, on `RequestEnvelope`. A result that needs
to name its inputs does so through the task record or the artifact it
published (RFC 0026 correction 32).
