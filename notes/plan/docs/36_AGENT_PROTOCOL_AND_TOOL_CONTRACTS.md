# Agent Protocol and Tool Contracts

## Objective

Give agents the equivalent of a purpose-built verification IDE: compact operations, semantic handles, explicit state, exact feedback, and safe authority boundaries.

## Native request envelope

```json
{
  "protocol_version": "3.0",
  "request_id": "req_...",
  "idempotency_key": "...",
  "operation": "verification.start",
  "actor": "agent:repairer-1",
  "capability": "cap_...",
  "snapshot": "ws_...",
  "intent": "in_...",
  "arguments": {},
  "budget": {},
  "output_policy": {}
}
```

## Native result envelope

```json
{
  "request_id": "req_...",
  "status": "completed",
  "verdict": "refuted",
  "assurance": {},
  "artifacts": [{"kind": "context_pack", "handle": "ctx_..."}],
  "task": "task_...",
  "continuation": null,
  "omissions": [],
  "warnings": [],
  "epochs": {}
}
```

## Tool design rules

1. Prefer semantic verbs over filesystem mechanics.
2. Return handles for reusable state.
3. Keep default results small.
4. Allow precise expansion by relation or budget.
5. Include valid next operations.
6. Make destructive/privileged operations visually and structurally distinct.
7. Avoid overloaded “run” tools.
8. Never place untrusted source text inside an instruction field.
9. Separate hypothesis from evidence.
10. Make `Unknown` and `Inconclusive` easy to represent.

## Example failure workflow

```text
workspace.create → ws_1
verification.start(ws_1, in_1, property) → task_1
verification.await(task_1) → cp_1 + ctx_1
context.expand(ctx_1, relation="source") → ctx_2
repair.begin(cp_1) → rt_1
repair.apply(rt_1, patch, hypothesis) → rt_2
repair.evaluate(rt_2) → rt_3
repair.promote(rt_3) → receipt_1 or PolicyGateFailed
```

## Context expansion

Expansion is graph-based:

```json
{
  "context": "ctx_1",
  "anchor": "event:e_ack",
  "relation": "causal_predecessors",
  "depth": 2,
  "budget": {"max_nodes": 50}
}
```

Useful relations include:

- causal predecessors/successors;
- conflicting alternatives;
- source correspondence;
- abstract projection;
- proof dependencies;
- assumptions used;
- state-change provenance;
- minimal correction candidates;
- sibling safe branches.

## Proof operations

Proof state handles are explicit. Agents operate on goals rather than cursor positions:

```text
proof.open(goal) → proof_state
proof.apply(state, tactic/term) → child states + diagnostics
proof.search(state, strategy, budget) → candidates
proof.check(candidate) → kernel receipt
```

## Repair authority

A repair agent may:

- read context;
- propose patches/models/proofs;
- start bounded evaluation;
- request expansion.

It may not:

- alter locked intent;
- declare evidence validated;
- sign promotion receipts;
- access unrelated secrets;
- modify benchmark graders.

## MCP mapping

MCP tool calls mirror native operations and carry handles as ordinary typed string fields. Immutable artifacts may be exposed as resources. The adapter remains stateless with respect to semantic work; the client threads handles.

## Protocol conformance

Ship:

- JSON Schema/OpenAPI or equivalent IDL;
- generated Rust/TypeScript/Python clients;
- golden protocol traces;
- fuzzing for malformed requests/results;
- compatibility matrix;
- deterministic ordering tests;
- authorization tests;
- replay/idempotency tests.

## Token economics

Measure protocol effectiveness rather than optimizing payload size blindly:

```text
useful semantic facts per token
successful task per token/tool call
time to correct diagnosis
expansion precision
invalid action frequency
```

A 500-token pack that induces the wrong repair is worse than a 2,000-token faithful pack.
