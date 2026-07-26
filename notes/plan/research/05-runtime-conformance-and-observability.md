# Research Note 05: Runtime Conformance Under Partial Observation

**Claim class:** core research program  
**Relevant sources:** [S12]–[S14], [S68]

## Thesis

Production telemetry should be treated as a constraint on possible executions, not a total trace. Continuum's native partial-order semantics can unify:

- implementation trace validation;
- production runtime verification;
- failure reproduction;
- model coverage feedback;
- instrumentation synthesis.

This is one of the clearest ways Continuum can exceed TLA+ and conventional DST systems.

## State of the art

MODIST showed transparent model checking of unmodified distributed systems. More recent work validates program traces against TLA+ specifications and uses multi-grained specifications to bridge implementation/model gaps. OmniLink explores trace validation for unmodified concurrent systems using semantic meanings and event intervals rather than requiring a single exact order.

These systems validate the need. Continuum's opportunity is to make the semantics and instrumentation native rather than an after-the-fact adapter.

## Core satisfiability problem

Let \(O\) be observed records and \(M\) the model. Build variables for:

- observation-to-model event matches;
- unobserved internal events;
- event ordering;
- abstract state;
- fault/restart epochs;
- time within uncertainty intervals.

Solve:

\[
M(\tau) \land Match(O,\tau) \land Order(O,\tau)
\land Completeness(O)
\]

for a model behavior \(\tau\).

An unsatisfiable result is a violation only when the completeness/ordering assumptions support it. Otherwise the result may be inconclusive.

## Impossibility boundaries

Recent theory of asynchronous fault-tolerant runtime verification shows that some real-time-order-sensitive properties cannot be decisively monitored under weak asynchronous evidence. Continuum should encode a **monitorability analysis**:

```text
property
+ observation model
+ failure model
→ refutable? confirmable? only inconclusive?
```

A monitorability verdict is included before processing production data.

## Instrumentation synthesis

Given a property and current telemetry schema:

1. compute the observer/event dependencies;
2. find ambiguous model behaviors indistinguishable under telemetry;
3. synthesize candidate probes/correlation IDs/fences;
4. rank them by information gain, overhead, and privacy cost;
5. verify that the new observation set distinguishes the target class.

This is an active-learning/test-generation problem over model executions.

## Novel proposal: Causal Information Budget

Define a measure of how much the telemetry reduces the model belief state—not Shannon entropy by default, because no probability distribution may exist. Candidate measures:

- number of surviving symmetry orbits;
- antichain width of possible configurations;
- logical formula size/prime implicants;
- distinguishability partition refinement;
- worst-case adversarial ambiguity.

Use this to choose instrumentation and to report why a trace is inconclusive.

## From production to Lab

For a violating or suspicious trace:

- retain observed order/interval constraints;
- synthesize missing choices/faults;
- minimize the causal explanation;
- generate a deterministic Lab scenario;
- replay against the exact program version;
- if reproduction fails, classify divergence (instrumentation, environment, adapter, model, nondeterminism leak).

Not every production trace is reproducible in a simplified pack profile. The tool must say so.

## Runtime overhead

Semantic instrumentation should support tiers:

- always-on stable IDs and lifecycle;
- sampled payload abstraction;
- incident-mode complete event families;
- hardware/eBPF/adapter-assisted external observation where needed.

Source-generated semantic probes beat generic tracing because they know event families and effect phases. OpenTelemetry compatibility is a transport concern, not the semantic model.

## Security and privacy

The conformance engine handles sensitive operational data. Requirements:

- local abstraction/redaction;
- typed privacy classification;
- cryptographic commitments for selected values;
- access-controlled crashpacks;
- deterministic secret scrubbing;
- no solver query exfiltration;
- eventually, privacy-preserving or zero-knowledge evidence for cross-organization protocol conformance.

## Evaluation

- known production incidents encoded as traces;
- injected telemetry loss and clock uncertainty;
- unmodified versus semantically instrumented variants;
- comparison with timestamp sorting, trace-to-TLA validation, and direct replay;
- overhead and ambiguity reduction;
- quality of synthesized instrumentation;
- success rate producing faithful Lab reproductions.

## Promotion and kill criteria (draft, pending ratification)

Per the research README contract, this lane declares its soundness
boundary, baseline, measurable promotion criterion, and kill condition.

**Soundness boundary.** Verdicts are sound only relative to the declared
observation and failure model; the monitorability analysis above is run
first, and properties classified as only-`Inconclusive` are reported as
such, never as passes or violations.

**Baseline.** The lane's own comparators from the evaluation plan:
timestamp sorting, trace-to-TLA validation, and direct replay.

**Promotion criterion (draft).**

- faithful Lab reproduction of ≥70% of the curated known-incident
  corpus under injected telemetry loss and clock uncertainty;
- always-on (tier-1) instrumentation overhead ≤1%;
- measurable ambiguity reduction per synthesized probe set: ≥2×
  reduction in surviving symmetry orbits, measured with the Causal
  Information Budget measures above.

**Kill condition.** The majority of target properties on two real
systems are monitorable only as `Inconclusive`, or the overhead budgets
cannot be met.

These thresholds are drafts registered in plan §24.5 ("pending
lane-owner ratification"). An unratified threshold may not survive
Phase A; until ratified or revised it blocks this lane's promotion.
