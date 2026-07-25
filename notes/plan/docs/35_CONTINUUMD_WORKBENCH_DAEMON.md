# `continuumd`: Authoritative Workbench Daemon

## Responsibility

`continuumd` is the sole authority for mutable workbench coordination. Semantic artifacts themselves are immutable.

It owns:

- snapshot creation and overlay resolution;
- Intent Contract registry and policy;
- query dependency graph and cache;
- tasks, budgets, cancellation, and continuations;
- artifact CAS and publication transactions;
- evidence graph/status transitions;
- proof/solver worker dispatch;
- repair and Forge transactions;
- authorization and audit.

It does not own:

- truth of solver results;
- proof acceptance;
- source control;
- agent planning;
- UI presentation semantics.

## Service topology

```text
clients
  │ native protocol
  ▼
protocol gateway
  ├── auth/capability policy
  ├── snapshot service
  ├── query service
  ├── task service
  ├── evidence service
  ├── debugger service
  ├── repair service
  └── Forge service
          │
          ▼
    isolated workers
  reference · explicit · DPOR · SMT · Lean · agents
```

## Snapshot transaction

1. Client submits root paths, overlay buffers, dependency/config references.
2. Daemon normalizes paths and content.
3. Parsers may produce diagnostics but cannot alter content identity.
4. Snapshot manifest is canonicalized and hashed.
5. Intent is attached by identity, not copied implicitly.
6. Snapshot is sealed and immutable.

An editor may create many cheap overlay snapshots. Garbage collection follows reachability from named roots, tasks, receipts, and retention policy.

## Task identity

A semantic task identity includes:

```text
operation
snapshot
intent
semantic/proof/engine epochs
normalized parameters
strategy class
```

Budget may be excluded from semantic identity when continuation semantics are monotonic; it remains in execution identity. Two clients requesting an identical task may share computation if authorization and privacy policy permit.

## Idempotency

Clients supply an idempotency key. The daemon records the canonical request digest. Reusing a key with different content is an error. Reusing it with identical content returns the existing task/result.

## Cancellation

Task cancellation is a request–drain–finalize protocol:

- child workers receive cancellation;
- provisional streams close;
- committed partial artifacts are finalized;
- continuation is emitted if supported;
- task transitions exactly once to terminal/suspended state;
- obligations and resource leases are resolved.

Hard-killed foreign workers produce an explicit worker-failure result, never a logical verdict.

## Continuations

A continuation contains:

- exact semantic task identity;
- frontier/search state;
- committed evidence roots;
- engine version;
- random/choice state where relevant;
- resource accounting;
- integrity checksum.

Resume validates all referenced inputs. Forking with a changed strategy or intent creates a new task, not a resume.

## Evidence publication

Workers return untrusted candidate artifacts. The daemon:

1. validates schema and size;
2. verifies referenced inputs;
3. invokes required independent checker;
4. computes content identity;
5. commits artifact;
6. commits evidence-graph edges/status;
7. emits event/subscription update.

The worker cannot select its own final status.

## Incremental query database

The query engine resembles a self-adjusting computation graph but adds proof-oriented edge classes and evidence provenance. Every cached result records:

- inputs and dependency reasons;
- implementation/query version;
- validation receipt;
- reuse class;
- last clean-comparison outcome.

## Deployment modes

### Embedded/local

A process-local or Unix-domain-socket daemon for `cargo continuum`; no external service required.

### Shared workstation

Multiple IDEs/agents share immutable artifacts and computations under per-user auth.

### Remote organization

CAS and workers scale independently. Intent/evidence policy remains centralized. Sensitive source can use local extraction with remote proof over minimized artifacts.

## Reliability

The daemon is itself verified through:

- asupersync Lab campaigns;
- model of task/publication state machine;
- crash recovery tests;
- linearizability checks for handle publication;
- Loom/weak-memory checks for local concurrent structures where appropriate;
- independent audit-log consistency checks.

## Observability

Operational telemetry is separate from semantic evidence. It includes task latency, cache behavior, worker health, queueing, resource use, and cancellation. A fast worker is not a correct worker; dashboards must not conflate them.
