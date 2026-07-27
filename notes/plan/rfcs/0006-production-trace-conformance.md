# RFC 0006: Production Partial-Order Conformance

**Status:** Proposed  
**Target gate:** G5 (Revision 2 scheme, docs/26 — not citable without translation to the docs/52 Revision 3 gates per plan §22)

## Summary

Continuum SHALL validate production evidence as a partial-order constraint problem. It MUST NOT invent a total event order that telemetry does not establish.

The validator asks whether at least one model execution can explain the observed events under declared instrumentation completeness and clock/causality assumptions.

## Observation model

A production record may include:

- stable operation and node/epoch identity;
- parent/span/message correlation;
- start/end or uncertainty interval;
- effect phase;
- request/response payload abstraction;
- durable sequence/fence marker;
- local monotonic sequence;
- vector/hybrid logical clock;
- observer/source;
- sampling and loss metadata.

The import layer translates records to **observation constraints**, not immediately to concrete CIR events.

## Conformance question

Given a model \(M\), observations \(O\), and assumptions \(A\), determine whether:

\[
\exists \tau\in Behaviors(M).\; \tau \models_A O.
\]

The result is one of:

```text
Conforms
Violates(counterexample/core)
Inconclusive(missing evidence)
Unsupported(feature)
ResourceExhausted(partial evidence)
```

`Inconclusive` is not success.

## Matching

Observation-to-model matching may be:

- direct by stable semantic event ID;
- keyed by operation family and payload abstraction;
- inferred through constraints;
- one observation to several internal model events;
- several observations to one abstract event;
- absent for stuttering/internal events.

Matching variables and all inferred orders are included in a certificate or explanation.

## Ordering constraints

Sources include:

- per-thread/process program order;
- message send-before-receive;
- region/task ownership;
- effect reserve-before-commit;
- storage sequence/barrier;
- logical/vector clocks;
- non-overlapping real-time intervals;
- explicit causal links.

Wall-clock timestamps alone are weak evidence and require uncertainty bounds.

## Completeness profiles

Instrumentation declares:

```text
Complete(observer, event family)
LossBounded(observer, family, n)
Sampled(observer, family, p or policy)
BestEffort(observer, family)
Opaque(family)
```

Some safety properties can be refuted with incomplete traces but cannot be validated. The property compiler determines required observation completeness.

## Incremental/streaming validation

The validator maintains a frontier of possible model configurations or a symbolic belief state. It may emit early violations when no completion remains. It must account for out-of-order telemetry and use watermarks/epochs before declaring closure.

Distributed runtime-verification impossibility results mean some properties cannot be decisively monitored under asynchronous faults and incomplete ordering. Continuum must expose those limits rather than paper over them.

## Counterexample explanation

A violation report includes:

- minimal unsatisfiable observation core where possible;
- the property and model assumptions;
- orders forced by evidence;
- alternative matches rejected;
- missing event families that would distinguish ambiguity;
- a Lab scenario/replay template when reconstructable.

## Privacy and security

Trace abstraction occurs at the source where possible. Payload schemas support redaction and cryptographic commitments. Conformance over commitments may prove equality/identity without exporting secrets, but zero-knowledge proof support is future work.

Trace records are adversarial input. Importers are sandboxed/adapters and cannot affect the kernel.

## Production feedback loop

Novel production behaviors that conform but are absent from exploration coverage become seed scenarios. Nonconforming traces become minimized crashpacks. Inconclusive traces drive instrumentation recommendations.

## Rejected alternatives

- Sort by timestamp and replay.
- Require global vector clocks everywhere.
- Declare success because no invariant violation appears in sampled logs.
- Treat observability as independent from property semantics.
