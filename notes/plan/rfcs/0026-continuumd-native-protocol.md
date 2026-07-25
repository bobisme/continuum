# RFC 0026: `continuumd` Native Protocol

## Status
Draft for implementation.

## Summary
Define the authoritative request/result protocol used by all Continuum adapters.

## Request
Required fields: protocol version, request ID, idempotency key, actor/capability, operation, snapshot, intent, arguments, budget, and output policy. Operations that do not use snapshot/intent must state `null` explicitly.

## Result
Required fields: request ID, lifecycle status, typed verdict/error, assurance envelope, artifact handles, task/continuation, omissions, warnings, cost, epochs, and allowed next operations.

## Task semantics
`start` is idempotent. `status` is monotonic. `cancel` requests drain/finalize. `resume` validates task identity. Streaming events are hints; committed artifacts are authoritative.

## Transport
Support local IPC first; authenticated HTTP/QUIC later. Encoding is canonical JSON for debugging and compact CBOR for performance. Generated clients share one IDL.

## Errors
Define stable codes including `StaleSnapshot`, `IntentMutationDenied`, `BudgetExhausted`, `ContinuationEpochMismatch`, `AmbiguousCorrespondence`, `InsufficientEvidence`, and `PolicyGateFailed`.

## Acceptance
Golden traces, malformed-input fuzzing, idempotency, restart/resume, cancellation, deterministic ordering, and adapter parity.
