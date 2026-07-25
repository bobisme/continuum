# Product Walkthroughs

## Walkthrough A: cancellation corrupts durable acknowledgement

### Intent

```text
Property: every published acknowledgement refers to a durable value.
Faults: cancellation at every checkpoint; crash before/after submit/sync.
Progress: failure-free synced request is eventually acknowledged.
Assurance: bounded exhaustive for 2 replicas, 1 crash, 1 cancellation.
```

### Failure

`cargo continuum check` returns a four-event causal core:

```text
WriteSubmitted(e7)
ReplyReserved(e7)
CancelRequested(task3)
ReplyPublished(e7)
```

The abstract delta is `acknowledged += e7` while `durable` is unchanged. The missing required event is `SyncCompleted(e7)`.

### Debug

The developer opens the branch before cancellation. Frontier:

```text
SyncCompleted(e7)
CancelRequested(task3)
Deliver(other-message)
```

Selecting `SyncCompleted` yields a safe branch. Selecting cancellation reproduces failure. Reverse causal step shows finalizer publication.

### Repair

An agent proposes to resolve the reply obligation only after sync. Intent diff is unchanged. Neighborhood exploration moves cancellation through adjacent phases. Known lost-abort and stale-epoch mutants remain detected. Promotion receipt is generated.

## Walkthrough B: liveness bug hidden by fairness

A model has `Retry` continuously enabled but the scheduler may forever choose `Tick`. The original intent declares weak fairness for `Retry` only under network connectivity.

An agent attempts to add unconditional strong fairness. Semantic diff classifies a stronger environment/scheduler assumption and blocks ordinary repair.

The debugger shows fairness debt and the fair lasso. The valid repair changes the protocol to persist retry intent and couples timer progress to an enabled retry transition.

## Walkthrough C: model/program drift

Rust refactor splits `Commit` into `PrepareCommit` and `PublishCommit`. Correspondence maps both as stuttering, so the abstract model never changes. Continuum reports an uncovered observer publication and an ambiguous lens conflict.

The developer selects a correspondence where `PublishCommit` implements the abstract `Commit`; a refinement obligation checks the intermediate state cannot leak to observers.

## Walkthrough D: production evidence is insufficient

Telemetry records request and acknowledgement but not disk sync completion. Continuum cannot decide `AckImpliesDurable` and returns `Inconclusive` with an instrumentation proposal:

```text
Record StorageStable(entry_id, epoch) before ReplyPublished.
Required correlation: entry_id + node_epoch.
Estimated added event volume: 0.8%.
```

It does not infer durability from timestamps.

## Walkthrough E: Forge invents a protocol variant

The user supplies a broadcast model with holes for acknowledgement and retransmission. Hard constraints require agreement and eventual delivery under eventual connectivity. Objectives minimize messages and stable writes. Forge:

1. retrieves analogous corpus protocols;
2. proposes candidates;
3. uses generalized counterexamples;
4. co-synthesizes an invariant and ranking;
5. archives behaviorally distinct candidates;
6. emits two Pareto candidates with bounded proof receipts;
7. materializes asupersync skeletons.

A human compares quorum geometry, recovery behavior, proof complexity, and benchmark cost before selecting one.

## Walkthrough F: multi-agent proof repair

A Rust change invalidates a Lean refinement theorem. Evidence Graph creates:

- proof goal;
- changed correspondence edge;
- finite countermodel to old induction hypothesis;
- relevant lemma slice.

Planner proposes two lemma decompositions. Proof workers attempt both. Adversarial reviewer checks theorem statement and axioms unchanged. Lean accepts one proof; receipt closes transaction.
