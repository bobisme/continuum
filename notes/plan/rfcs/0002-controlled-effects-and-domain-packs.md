# RFC 0002: Controlled Effects and Semantic Domain Packs

**Status:** Proposed  
**Target gate:** G0/G1

## Summary

All nondeterministic or externally visible behavior in a Continuum-controlled program MUST cross a typed effect boundary. Effect implementations are grouped into **domain packs** that provide production, deterministic-Lab, abstract, fault, trace, and refinement semantics for one domain.

A pack is not a mock. It is a semantic module with declared fidelity.

## Goals

- Run materially identical protocol logic in production and deterministic exploration.
- Make time, entropy, network, storage, process lifecycle, synchronization, and cancellation explicit.
- Allow packs to be composed without hidden global state.
- Give DPOR and refinement engines typed semantic footprints.
- Prevent the phrase “simulated disk” from concealing an unspecified failure model.

## Pack contract

A domain pack exports:

```rust
pub trait DomainPack {
    type Command;
    type Response;
    type Resource;
    type AbstractState;
    type Fault;
    type EventPayload;

    fn validate_command(...);
    fn production(...);
    fn lab_step(...);
    fn abstract_step(...);
    fn footprint(...);
    fn independence(...);
    fn observe(...);
    fn recover(...);
    fn assumptions(...) -> AssumptionSet;
}
```

The actual Rust API may split this trait to avoid monomorphization and object-safety problems. The semantic obligations remain.

## Required profiles

Every pack names one or more fidelity profiles:

```text
ideal
contractual
platform-qualified
adversarial-envelope
```

- `ideal` is a simple mathematical abstraction.
- `contractual` matches a documented service/API contract.
- `platform-qualified` is backed by measurements and conformance tests for a particular deployment.
- `adversarial-envelope` permits all behavior not forbidden by declared constraints.

A verification result identifies exact profile versions. Profiles are not ordered automatically; a refinement proof or conformance campaign establishes relationships.

## Standard packs

### Network

Must parameterize at least:

- loss, duplication, reordering;
- partition topology;
- bounded/unbounded delay assumptions;
- connection epochs;
- half-open behavior;
- backpressure and capacity;
- corruption/authentication assumptions;
- DNS/service-discovery behavior when relevant.

### Storage

Must distinguish:

- volatile process memory;
- page cache / submitted writes;
- durable stable storage;
- atomicity granularity;
- ordering and barriers;
- torn writes;
- sector/block corruption;
- rename/link/directory durability;
- crash and restart;
- recovery procedure;
- device and filesystem profile.

### Process lifecycle

Must model:

- graceful cancellation;
- panic;
- fail-stop crash;
- power loss;
- restart with a new epoch;
- partial cleanup;
- supervisor decisions;
- leaked external effects.

### Clock

Must distinguish:

- monotonic local time;
- wall time;
- drift and skew;
- discontinuities;
- uncertainty intervals;
- lease assumptions;
- timer coalescing and delayed wakeup.

### Entropy

Must distinguish deterministic pseudo-random choices from cryptographic entropy. Production randomness can be observed as opaque committed values; verification may enumerate a finite abstraction or use symbolic values.

## Asupersync integration

Asupersync's `Cx`, region hierarchy, explicit time/entropy, obligations, reserve/commit primitives, and Lab runtime are the concrete execution foundation. Continuum adapters MUST use public semantic surfaces and MUST NOT duplicate scheduling, cancellation, or quiescence machinery.

A capability call emits a proposed CIR event. The domain pack enriches it with resource paths, effect phases, assumptions, and abstract meaning.

## Ambient nondeterminism enforcement

The project SHALL combine:

- API design: protocol crates receive capabilities, not ambient globals;
- lints: deny `std::time::*::now`, ambient RNG, unmanaged spawn, raw socket/file creation in controlled crates;
- dependency manifests: identify foreign code capable of nondeterminism;
- runtime interception where possible;
- replay checks: detect values not derivable from the choice log;
- optional MIR/LLVM audits for host calls;
- explicit escape hatches with an `UnmodeledEffect` event that downgrades assurance.

An unmodeled effect cannot be ignored. It changes the result from `verified` to a typed incomplete claim.

## Fault algebra

Faults are typed operations over pack state. They support composition:

```text
delay(d) ; duplicate(n) ; crash(epoch)
partition(A,B) || clock_skew(node,+δ)
```

Composition is only commutative when the pack proves independence. Fault campaigns may quantify a budget (e.g., at most two crashes) without implying a real-world probability.

## Refinement obligation

For each profile, the pack defines a relation between concrete and abstract histories. In its simplest form:

\[
R(c,a) \land c \xrightarrow{e} c'
\Rightarrow
\exists a'.\; a \Rightarrow^* a' \land R(c',a').
\]

For partial telemetry, the pack can emit constraints rather than a unique event.

## Pack qualification

A pack is promoted from experimental only after:

- model-based tests against a reference implementation;
- differential testing across production/Lab paths;
- crash-consistency mutation suite;
- independence mutation suite;
- stable replay across supported versions;
- documented unsupported behavior;
- at least one external or independently implemented oracle where feasible.

## Rejected alternatives

- A single universal `World` trait with opaque calls.
- Generic serde payloads and user-written independence functions with no validation.
- Simulators that accidentally promise platform fidelity.
- Build-time feature flags that compile different protocol logic for production and verification.
