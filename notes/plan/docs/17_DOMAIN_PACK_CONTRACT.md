# Domain Pack Contract

## 1. Purpose

A domain pack is a semantic component, not a mock library. It must be precise enough for exploration, refinement, replay, and production conformance.

## 2. Required contents

```text
pack/
├── PACK.md
├── schema/
├── model/
├── lab/
├── production/
├── fidelity/
├── tests/
├── mutants/
└── examples/
```

## 3. PACK.md requirements

- operation list;
- state model;
- event lifecycle;
- fault algebra;
- ordering guarantees;
- cancellation behavior;
- durability/visibility points;
- independence rules;
- fairness assumptions;
- unsupported behaviors;
- host configurations;
- semantic versioning.

## 4. Operation schema

Each operation defines:

```rust
struct OperationContract {
    name: Symbol,
    input_type: Type,
    output_type: Type,
    phases: PhaseMachine,
    resources: FootprintTemplate,
    observations: Vec<ObservationSchema>,
    faults: Vec<FaultSchema>,
    cancel_points: Vec<CancelPoint>,
}
```

## 5. Three implementations

### Ideal

Small mathematical relation used in high-level models.

### Lab

Executable deterministic handler exposing every nondeterministic choice.

### Production

Host implementation plus semantic instrumentation.

Required relations:

```text
Lab refines Pack
Production observations refine Pack under FidelityProfile
Ideal is abstracted/refined by Pack according to declared direction
```

## 6. Fidelity profile example: durable storage

```yaml
profile: linux-ext4-local-ssd-v1
claims:
  - write becomes visible to process after successful syscall
  - sync completion is the modeled durability boundary
modeled_faults:
  - process crash
  - loss of unsynced writes
  - write reordering before sync
excluded:
  - device firmware lies about flush
  - latent sector corruption
  - kernel/filesystem bugs
  - power-loss torn sector below declared atomic unit
assumptions:
  - mount options: [...]
  - device cache configuration: [...]
```

There is no universal `Disk`.

## 7. Network pack baseline

State:

- endpoints;
- message envelopes;
- in-flight multiset;
- partitions;
- connection epochs;
- MTU/frame state;
- delivery history.

Choices:

- deliver;
- delay;
- drop;
- duplicate;
- reorder through choice of deliverable envelope;
- partition/heal;
- connection reset;
- endpoint crash.

Optional profiles:

- reliable FIFO stream;
- QUIC-like stream abstraction;
- datagram;
- adversarial Byzantine.

## 8. Cancellation contract

Every async pack operation declares:

- reservation point;
- commit point;
- abort behavior;
- finalizer obligations;
- whether cancellation can return before host completion;
- late-completion handling;
- idempotence key;
- replay choice.

## 9. Independence contract

A pack supplies a conservative predicate and witness:

```rust
fn independent(a: &Event, b: &Event, observer: &Observer) -> Independence {
    Independence::Proved(witness)
    // or Independence::Unknown
}
```

`Unknown` means dependent.

Typical storage dependence:

- same object/generation: dependent;
- distinct objects may still depend through global flush/barrier;
- crash event depends on all volatile operations;
- sync depends on writes in its durability domain.

## 10. Fault algebra

Faults compose only where semantics says they do. A pack defines constraints such as:

- crash clears volatile state;
- recovery increments epoch;
- delayed completion from old epoch is rejected or explicitly modeled;
- partition affects matching routes;
- clock jump changes wall mapping, not monotonic logical time;
- cancellation is not process crash.

## 11. Pack conformance suite

- model vs Lab successor equality on finite scopes;
- production handler trace validation in controlled integration tests;
- every fault reachable;
- forbidden transitions rejected;
- phase mutants killed;
- independence mutants detected by baseline comparison;
- serialization/replay stable;
- host profile tests.

## 12. Versioning

- patch: implementation/performance, same semantics;
- minor: additive operation/fault, old behavior unchanged;
- major: changed semantics/fidelity;
- semantic epoch bump if core interpretation changes.

Every crashpack pins exact pack digest.

## 13. Third-party packs

Untrusted packs can be used for sampled testing. Certified claims require:

- conformance suite;
- review/signature policy;
- independence audit;
- fidelity statement;
- kernel-recognized schema or checked extension proof.
