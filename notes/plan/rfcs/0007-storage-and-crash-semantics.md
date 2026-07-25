# RFC 0007: Storage, Crash, and Recovery Semantics

**Status:** Proposed  
**Target gate:** G1

## Summary

Continuum's storage pack SHALL model durability as a typed transition system, not as a byte map plus random write failure. Crash semantics, atomicity, ordering, persistence barriers, corruption, and recovery are versioned profile data.

## State strata

A minimal storage state distinguishes:

```text
application intent
reserved operation
submitted operation
volatile/cache-visible state
device-accepted state
stable state
recovery-visible state
```

The exact strata depend on the profile. A high-level database pack may instead expose transaction log records, commit indexes, and checkpoints while refining to a lower storage pack.

## Operation phases

Example:

```text
WriteRequested
  → BufferReserved
  → Submitted
  → CompletedVolatile
  → Stable
  → Acknowledged
```

Cancellation is legal only at declared boundaries. An acknowledgement before the durability level promised by the API is a semantic defect.

## Crash

A crash selects a legal projection from pre-crash strata to recovery-visible state according to the profile. It also:

- increments process/node epoch;
- kills or cancels tasks according to crash type;
- invalidates volatile handles;
- preserves or loses in-flight effects according to the profile;
- produces recovery obligations.

The crash choice is explicit and replayable.

## Atomicity and ordering

Profiles declare:

- minimum atomic-write unit;
- torn-write possibilities;
- ordering constraints;
- barrier/fsync/fdatasync semantics;
- metadata and directory persistence;
- rename semantics;
- writeback behavior;
- checksum/corruption detection;
- replication/controller caches where modeled.

A profile may be deliberately adversarial, permitting every state not forbidden by the declared contract.

## Recovery

Recovery is ordinary controlled code executed under a recovery capability. It emits CIR events and can be canceled/crash again. Recovery invariants are first-class:

- idempotence;
- monotonic durable epoch;
- log-prefix validity;
- no acknowledgement resurrection;
- cleanup/quiescence;
- eventual completion under explicit assumptions.

## Layered packs

```text
FilesystemProfile
    refines BlockDeviceProfile

WALProfile
    refines FilesystemProfile

DatabaseTransactionProfile
    refines WALProfile
```

Continuum does not require all layers in every project. Claims identify the layer and refinement evidence used.

## Qualification

Platform-qualified profiles require:

- documentation-derived contract;
- empirical litmus tests;
- fault-injection campaigns;
- cross-version fingerprinting;
- explicit environment (OS, filesystem, mount options, device, cloud service);
- conservative fallback when measurements disagree.

Measurements provide evidence, not universal proof.

## First implementation

The first storage pack uses a small abstract append log:

- writes append records;
- `sync` promotes a prefix to stable;
- crash discards a nondeterministically selected volatile suffix;
- records have checksum validity;
- recovery scans the stable prefix;
- cancellation can occur before reservation, after reservation, after submission, and after stability.

This is enough to expose acknowledgement-before-sync and cleanup defects in the first vertical slice.

## Rejected alternatives

- POSIX as one fixed semantics.
- Treat successful `write` as durable.
- Inject crashes only between test operations.
- Let production and Lab storage adapters use unrelated protocol logic.
