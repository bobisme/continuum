# Agent API Reference Sketch

> **Non-normative projection.** This document is a projection of RFC 0026
> (`continuumd` native protocol), RFC 0027 (agent tool protocol), and plan
> §10.2, and is regenerated from them. On any divergence, the RFCs and the
> plan win.

This is a shape contract, not the final wire IDL.

## Handles

Full prefix set per plan §4.4:

```text
WorkspaceHandle      ws_...
IntentHandle         in_...
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
```

## Workspace

### `workspace.create`

Inputs: root/overlay/dependency/config references.  
Output: immutable workspace handle plus diagnostics.

### `workspace.fork`

Inputs: base handle and patch/overlays.  
Output: new handle and textual/semantic pre-diff availability.

### `workspace.diff`

Inputs: two handles and requested layers.  
Output: diff artifact.

### `workspace.seal`

Seals a snapshot as immutable; sealed snapshots are the only valid semantic inputs.

## Intent

### `intent.get`

Returns canonical Intent Contract.

### `intent.diff`

Returns classified changes, proof obligations, and policy impact.

### `intent.propose_revision`

Creates a proposal only; cannot mutate existing intent.

### `intent.accept`

Privileged. Accepts a proposed intent into the registry; drafts gain INV-001 protection only on explicit acceptance.

### `intent.reject`

Privileged. Rejects a proposed intent revision with a typed reason.

### `intent.lock`

Privileged. Applies an intent lock (plan §5.4); subsequent mutation attempts fail with `IntentMutationDenied`.

## Verification

### `verification.start`

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

Returns typed verdict, assurance, evidence roots, continuation, omissions.

### `verification.await`

Blocks (within budget) on a started verification task and returns its result.

## Model

### `model.check`

Checks a model against a property under intent and budget; returns typed verdict and evidence.

### `model.explore`

Explores model behavior under a strategy and budget; returns explored-region evidence.

### `model.compare`

Compares two models/configurations; returns a semantic diff artifact.

## Program

### `program.extract`

Extracts the semantic model from a Rust crate snapshot and annotations.

### `program.run`

Runs the program under controlled semantics; returns execution artifacts.

### `program.replay`

Replays a recorded execution deterministically; divergence fails with `ReplayDiverged`.

## Refinement

### `refinement.check`

Checks a refinement correspondence between program and model; returns typed verdict.

### `refinement.explain`

Explains a refinement result or failure with a Context Pack.

## Context

### `context.compile`

Inputs: evidence/question/audience/budget.  
Output: Context Pack.

### `context.expand`

Inputs: context, anchor, relation, budget.  
Output: new immutable Context Pack referencing parent.

## Debug

### `debug.open`

Inputs: crashpack/execution/configuration.  
Output: debugger handle and current frontier.

### `debug.state`

Inputs: handle and optional observer projection.  
Output: current concrete/abstract state view.

### `debug.enabled`

Inputs: handle.  
Output: enabled events at the current frontier.

### `debug.step_event`

Inputs: handle, event.  
Output: new handle/state delta (steps one semantic event).

### `debug.step_abstract`

Inputs: handle.  
Output: new handle/state delta (steps one abstract transition).

### `debug.reverse_causal`

Inputs: handle.  
Output: handle at the causal predecessor.

### `debug.branch`

Inputs: handle and alternate enabled event/fault.  
Output: sibling handle.

### `debug.compare`

Inputs: two handles and observer.  
Output: semantic branch diff.

### `debug.why_enabled`

Inputs: handle, event.  
Output: explanation of why the event is enabled.

### `debug.why_blocked`

Inputs: handle, event.  
Output: explanation of why the event is blocked.

### `debug.export`

Inputs: handle.  
Output: branch exported as a crashpack or regression scenario.

## Failure

### `failure.explain`

Returns a causal explanation of a failure at the requested level.

### `failure.minimize`

Returns a minimized counterexample preserving the failure.

### `failure.branch`

Explores an alternate branch from a failure's causal frontier.

## Repair

### `repair.begin`

Inputs: failure, base snapshot/intent, policy.  
Output: transaction handle.

### `repair.apply`

Inputs: transaction, patch/model/proof changes, hypothesis.  
Output: new transaction and candidate snapshot.

### `repair.attach`

Attaches additional evidence/artifacts (e.g. from another agent) to a transaction; produces a new transaction version.

### `repair.evaluate`

Inputs: transaction and optional budget.  
Output: evidence progress/status/continuation.

### `repair.resume`

Resumes a suspended evaluation from its continuation; inputs and epochs are validated.

### `repair.review`

Returns the review view of a transaction: semantic/intent diff, gate status, evidence.

### `repair.promote`

Privileged. Returns receipt or typed policy failure.

### `repair.reject`

Privileged. Rejects a transaction with a typed reason.

## Forge

### `forge.create`

Inputs: snapshot, intent, sketch/holes/objectives/diversity/assurance/budget.

### `forge.step`

Runs one or bounded portfolio iteration; returns archive deltas and counterexamples.

### `forge.archive`

Queries the Pareto/quality-diversity archive by objective/descriptor/evidence status.

### `forge.materialize`

Creates a design transaction with model/Rust/proof artifacts.

## Proof

### `proof.goal`

Returns exact goal, proof Context Pack, and proof-state handle.

### `proof.attempt`

Applies tactic/term to a proof-state handle and returns children/diagnostics.

### `proof.check`

Strict isolated kernel check and receipt.

### `proof.slice`

Returns the proof dependency slice (curated declarations) for a goal.

## Benchmark

### `benchmark.run`

Runs a benchmark task under declared budget and graders; returns evidence, not self-scored results.

## Tasks

### `task.status`

Includes semantic milestones, resource use, committed evidence, and terminal reason.

### `task.cancel`

Requests cancel-correct shutdown.

### `task.resume`

Requires continuation plus matching inputs; optional larger budget.

### `task.subscribe`

Streams progress events over the connection; events are hints — committed artifacts and `task.status` are authoritative.

## Evidence

### `evidence.get`

Fetch typed artifact metadata or bounded content.

### `evidence.query`

Graph query with node/edge/status/property filters.

### `evidence.verify`

Runs relevant independent checker; does not accept client-declared status.

## Query

### `query.explain_reuse`

Explains which memoized results were reused for a derivation and why.

### `query.explain_invalidation`

Explains which results a change invalidated and through which dependency edges.

### `query.clean_compare`

Compares incremental results against a clean recompute (Incremental Parity Audit, plan §9.5).

## Error taxonomy

Stable codes per plan §10.3:

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
ProtocolVersionUnsupported   (protocol-level, RFC 0026)
IdempotencyKeyReused         (protocol-level, RFC 0026)
MalformedRequest             (protocol-level, RFC 0026)
```

Errors may carry `recovery` as a list of allowed operations with pre-filled arguments — never free-form commands. `BudgetExhausted` is never a semantic verdict; it carries the continuation when one exists.

## Common result rules

Every result includes:

- request/task identity;
- snapshot and intent;
- semantic/proof/engine epochs;
- verdict/status;
- artifact handles;
- omissions/unknowns;
- cost/budget;
- valid next operations;
- audit correlation.
