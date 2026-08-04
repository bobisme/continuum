# Agent Protocol and Tool Contracts

> **Non-normative projection.** This document is a projection of RFC 0026
> (`continuumd` native protocol), RFC 0027 (agent tool protocol), and plan
> §10.2, and is regenerated from them. On any divergence, the RFCs and the
> plan win.
>
> Regenerated against IDL 3.3 (protocol 3.3, 73 operations) and RFC 0026 /
> RFC 0027 as of bn-l4kmc.

## Objective

Give agents the equivalent of a purpose-built verification IDE: compact operations, semantic handles, explicit state, exact feedback, and safe authority boundaries.

## Native request envelope

```json
{
  "protocol_version": "3.3",
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
  "status": "ok",
  "verdict": {"semantic": {"verdict": "refuted", "assurance_class": "validated"}},
  "assurance": {},
  "artifacts": [{"kind": "ctx", "handle": "ctx_..."}],
  "task": "task_...",
  "continuation": null,
  "omissions": [],
  "warnings": [],
  "cost": {"states": 48211, "wall_ms": 9120},
  "epochs": {
    "protocol": "3.3",
    "semantic": "sem_...",
    "intent": "in_epoch_...",
    "evidence": null,
    "proof": null,
    "corpus": null,
    "engine": "eng_..."
  },
  "next_operations": [
    {"operation": "context.expand", "arguments": {"context": "ctx_...", "anchor": "event:e_ack", "relation": "causal_predecessors"}},
    {"operation": "failure.explain", "arguments": {"failure": "crash_..."}},
    {"operation": "repair.begin", "arguments": {"failure": "crash_..."}}
  ]
}
```

Five corrections from the previous revision (RFC 0026 correction 25), against a
completed task-observing result carrying a `semantic` verdict: `status` is a
`ResultStatus` member (`ok`, not the `TaskStatus` member `completed`);
`verdict` is a tagged union, never a bare string (`{"semantic": {...}}`, not
`"refuted"`); `next_operations` is a list of `NextOperation` structs carrying
pre-filled `arguments`, never bare operation names; `artifacts[].kind` is the
plan §4.4 class prefix without its underscore (`"ctx"`, not `"context_pack"`);
and `epochs` names all six compatibility epochs, each pinned or explicitly
null, plus `engine` (identity, not a seventh epoch) — never an empty object.

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
10. Make `inconclusive` (with its typed `inconclusive_reason`) and typed
    omissions easy to represent. There is no `Unknown` verdict, status, or
    reason on this protocol (RFC 0026): `unknown` names a Context Pack
    selection kind (RFC 0028) and a diff relation (RFC 0031), and none of
    the three MUST be conflated with `inconclusive`.

## Example failure workflow

```text
workspace.create → ws_1
verification.start(ws_1, in_1, property) → task_1
verification.await(task_1) → crash_1 + ctx_1
context.expand(ctx_1, relation="source_span") → ctx_2
repair.begin(crash_1) → rt_1
repair.apply(rt_1, patch, hypothesis) → rt_2
repair.evaluate(rt_2) → rt_3
repair.promote(rt_3) → receipt_1 or PolicyGateFailed
```

## Error taxonomy

Errors use the 20 stable typed `ErrorCode` members of RFC 0026 (`@open` — a
future protocol minor MAY add more, never remove or repurpose one):
seventeen non-protocol-level codes (`StaleSnapshot` … `PublicationAborted`)
plus three protocol-level codes (`ProtocolVersionUnsupported`,
`IdempotencyKeyReused`, `MalformedRequest`). Any restatement of the taxonomy
as fifteen codes predates the five codes review 5 added —
`AcceptanceChainInvalid`, `StatusConflict`, `QuotaExhausted`,
`EpochUnsupported`, `PublicationAborted` — and is stale. Every error carries
`recovery` (**required** — an empty list is the typed statement that no
recovery exists) as a list of allowed operations with pre-filled arguments,
never free-form commands, and `retryable` (**required**). `BudgetExhausted`
is never a semantic verdict; it carries the continuation when one exists.

## Context expansion

Expansion is graph-based:

```json
{
  "context": "ctx_1",
  "anchor": "event:e_ack",
  "relation": "causal_predecessors",
  "depth": 2
}
```

A node ceiling is bounded through `output_policy.max_nodes` — an
`OutputPolicy` member of the *envelope* — never through a `budget` argument:
`context.expand` declares no `budget` field of its own, and an unknown key
inside `budget` would be silently ignored under
`rule versioning.compatible_change` rather than enforced.

`ExpansionRelation` is closed at eleven members (RFC 0027); this is the
full closed vocabulary, not an illustrative sample:

- `causal_predecessors`;
- `causal_successors`;
- `conflicts_with`;
- `same_owner`;
- `property_automaton_step`;
- `proof_dependency`;
- `source_span`;
- `assumption_uses`;
- `abstraction_of`;
- `refinement_of`;
- `alternate_branch`.

## Proof operations

Proof state handles are explicit. Agents operate on goals rather than cursor positions:

```text
proof.goal(obligation) → exact goal + proof Context Pack + proof_state
proof.attempt(state, tactic/term) → child states + diagnostics
proof.check(candidate) → kernel receipt
proof.slice(goal) → proof dependency slice (curated declarations)
```

## Repair authority

This section is illustrative, not itself an authority boundary: RFC 0027's
per-operation registry and its four-test admission predicate (T1 level, T2
scope, T3 privilege, T4 actor binding) are what a daemon evaluates. In the
`repair` namespace: `repair.begin`, `repair.apply`, and `repair.attach` are
`propose`; `repair.evaluate` and `repair.resume` are `execute`;
`repair.review` is `read`; `repair.promote` and `repair.reject` are
`promote` (`@privileged`, `@audit_recorded`).

A `propose`/`execute`-level repair agent can typically:

- read context;
- propose patches/models/proofs;
- start bounded evaluation;
- request expansion.

It cannot, at those levels:

- alter locked intent (`IntentMutationDenied` without `revise-intent`);
- promote or reject a transaction — that requires `promote` authority;
- act as the independent checker over evidence it produced itself
  (RFC 0038's no-self-certification rule);
- access unrelated secrets;
- modify benchmark graders.

## MCP mapping

MCP tool calls mirror native operations and carry handles as ordinary typed string fields. Immutable artifacts may be exposed as resources. The adapter remains stateless with respect to semantic work; the client threads handles.

## Protocol conformance

Ship:

- the custom textual IDL (`schemas/continuumd-native-protocol.idl`) as the
  single machine-readable definition — the technology choice is decided,
  not open: three-valued field presence, per-operation authority levels,
  task-starting/idempotency annotations, closed per-operation error sets,
  and citable normative rules have no faithful encoding in JSON Schema,
  OpenAPI, or Protobuf (RFC 0026 "Notation"); JSON Schema is a *generated*
  artifact of the IDL, not its source;
- generated Rust and TypeScript clients;
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
