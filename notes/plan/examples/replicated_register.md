# First Vertical Slice: Cancel-Correct Replicated Register

## Purpose

This example is intentionally small but semantically dense. It must exercise:

- standalone abstract modeling;
- the same protocol logic under production and Lab asupersync runtimes;
- virtual network, storage, time, crash/restart, and cancellation;
- causal event generation;
- safety checking;
- concrete-to-abstract stuttering refinement;
- DPOR;
- deterministic replay and minimization;
- proof/checking artifacts.

The system is not intended as a production consensus protocol. It is a verification-system acceptance test.

## Service contract

Clients submit `(epoch, value)` proposals. The service may acknowledge at most one value for any epoch. An acknowledgement means the value is stable on a configured quorum.

Abstract state:

```text
chosen: Map[Epoch, Value]
```

Abstract action:

```text
Choose(e, v):
    require e notin chosen || chosen[e] == v
    chosen' = chosen[e := v]
```

Invariant:

```text
forall e, v1, v2:
  Chosen(e, v1) && Chosen(e, v2) => v1 == v2
```

## Operational protocol

Three replicas maintain:

```text
current_epoch
volatile proposal
durable log
acks observed
region/task epoch
```

Messages:

```text
Propose(epoch, value)
Prepare(epoch)
Prepared(epoch, prior)
Commit(epoch, value)
Committed(epoch, value)
```

The deliberately simplified coordinator:

1. collects a quorum of `Prepared`;
2. appends a commit record locally;
3. asks followers to append;
4. waits for a quorum of stable confirmations;
5. acknowledges the client.

## Required mutants

Each mutant must be found and reduced:

| ID | Defect |
|---|---|
| M01 | acknowledge after volatile append but before `sync` |
| M02 | canceled sender loses a reserved message without abort |
| M03 | restart reuses the old process epoch |
| M04 | duplicate `Commit` is not idempotent |
| M05 | coordinator counts one replica twice across restart |
| M06 | loser task from a quorum race is not drained |
| M07 | recovery ignores checksum/truncated-tail state |
| M08 | timer from an old epoch fires in a new process |
| M09 | view maps `Submitted` storage to abstract `Chosen` |
| M10 | independence rule says two writes to same epoch commute |

## Expected causal core for M01

```text
client request
  → quorum prepared
  → local write reserved
  → local write submitted
  → cancellation requested
  → acknowledgement published      [bug]
  → crash
  → volatile write lost
  → new coordinator chooses other value
  → second acknowledgement
```

The minimal counterexample should not contain unrelated task polls, timer checks, or independent messages.

## Acceptance claims

### A. Standalone model

Exact finite exploration over:

```text
Nodes = 3
Values = 2
Epochs = 2
crashes ≤ 2
```

establishes the abstract invariant.

### B. Runtime exploration

DPOR over the correct implementation finds no concrete invariant/refinement violation within declared bounds and emits an exact campaign envelope.

### C. Refinement

Every stable quorum transition maps to zero or one `Choose`; ordinary polls, reservation, delivery, retries, and cancellation cleanup stutter.

### D. Replay

Every mutant counterexample reproduces from its crashpack across:

- clean process invocation;
- supported platforms;
- debug/release profiles where semantic behavior is expected equal;
- snapshot-restored and root-replayed paths.

### E. Certificates

The abstract finite proof emits a reachable-set or inductive-invariant certificate checked by `continuum-kernel`. Mutant traces emit checked counterexample witnesses.
