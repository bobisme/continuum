# Agent API Reference Sketch

This is a shape contract, not the final wire IDL.

## Handles

```text
WorkspaceHandle      ws_...
IntentHandle         in_...
TaskHandle           task_...
ContinuationHandle   cont_...
EvidenceHandle       ev_...
ContextHandle        ctx_...
CrashpackHandle      cp_...
DebugHandle          dbg_...
RepairHandle         rt_...
ForgeHandle          forge_...
ProofStateHandle     ps_...
ReceiptHandle        receipt_...
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

## Intent

### `intent.get`

Returns canonical Intent Contract.

### `intent.diff`

Returns classified changes, proof obligations, and policy impact.

### `intent.propose_revision`

Creates a proposal only; cannot mutate existing intent.

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

### `debug.step`

Inputs: handle, event or stepping policy.  
Output: new handle/state delta.

### `debug.branch`

Inputs: handle and alternate enabled event/fault.  
Output: sibling handle.

### `debug.compare`

Inputs: two handles and observer.  
Output: semantic branch diff.

## Repair

### `repair.begin`

Inputs: failure, base snapshot/intent, policy.  
Output: transaction handle.

### `repair.apply`

Inputs: transaction, patch/model/proof changes, hypothesis.  
Output: new transaction and candidate snapshot.

### `repair.evaluate`

Inputs: transaction and optional budget.  
Output: evidence progress/status/continuation.

### `repair.promote`

Privileged. Returns receipt or typed policy failure.

## Proof

### `proof.open_goal`

Returns exact goal, proof Context Pack, and proof-state handle.

### `proof.apply`

Applies tactic/term to a proof-state handle and returns children/diagnostics.

### `proof.check`

Strict isolated kernel check and receipt.

## Forge

### `forge.create`

Inputs: snapshot, intent, sketch/holes/objectives/diversity/assurance/budget.

### `forge.step`

Runs one or bounded portfolio iteration; returns archive deltas and counterexamples.

### `forge.candidates`

Queries by objective/descriptor/evidence status.

### `forge.materialize`

Creates a design transaction with model/Rust/proof artifacts.

## Tasks

### `task.status`

Includes semantic milestones, resource use, committed evidence, and terminal reason.

### `task.cancel`

Requests cancel-correct shutdown.

### `task.resume`

Requires continuation plus matching inputs; optional larger budget.

## Evidence

### `evidence.get`

Fetch typed artifact metadata or bounded content.

### `evidence.query`

Graph query with node/edge/status/property filters.

### `evidence.verify`

Runs relevant independent checker; does not accept client-declared status.

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
