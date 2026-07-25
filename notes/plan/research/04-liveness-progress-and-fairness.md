# Research Note 04: Liveness, Progress, and Fairness at Scale

**Claim class:** research and implementation program  
**Relevant sources:** [S30], [S31], [S31A], [S55]

## Problem

Safety counterexamples have finite bad prefixes. Liveness failures are infinite behaviors, typically represented by cycles, and their validity depends on fairness and environment assumptions. Distributed systems add crashes, partitions, retries, cancellation, and partial synchrony, making “eventually” dangerously ambiguous.

Continuum must make progress properties explicit enough to prove and diagnose, while avoiding an unreadable temporal-logic research language.

## Semantic decomposition

A progress claim is a tuple:

```text
trigger
goal
environment assumptions
scheduler/action fairness
fault budget or eventual-stability assumption
time model
observer
```

Example:

```text
Every accepted request eventually receives a response
provided:
  a majority remains alive after some finite time,
  network links among that majority eventually deliver,
  stable storage eventually completes,
  enabled request-processing tasks are weakly fair,
  cancellation is not requested.
```

Cancellation requires a separate goal: if requested, the request must drain to a terminal outcome under cooperative-path assumptions.

## Ranking functions

Recent work demonstrates substantial automation for distributed-protocol liveness using ranking functions. Continuum can synthesize:

- natural-number rankings;
- lexicographic tuples;
- multisets;
- ordinal-shaped templates;
- phase-indexed rankings;
- transition invariants.

For asupersync, region/obligation state provides unusually strong ranking features. A drain proof might use a multiset of outstanding obligation depths and cleanup phases rather than elapsed time.

## Novel proposal: Obligation-Flow Ordinals

Associate each obligation with a phase rank and region depth. Define a multiset order:

\[
\mathcal R(C)=
\{\!\{(\text{phase}(o),\text{depth}(o),\text{budget}(o)) \mid o\in O_C\}\!\}.
\]

Cancellation transitions must either decrease this multiset under a well-founded extension or make a fairness-eligible step that will. Spawning new cleanup obligations is allowed only if their ordinal mass is bounded by the consumed parent obligation.

This could turn structured-concurrency lifecycle rules into machine-checkable global progress arguments.

The idea is killed if ordinary lexicographic counts suffice with equal automation and clarity.

## Liveness-to-safety

Several approaches reduce progress checking to repeated safety or induction queries. Continuum should support transformations whose proof artifacts are visible:

- monitor construction;
- loop/fair-cycle detection;
- k-liveness style counters;
- ranking certificates;
- recursive safety chains.

The transformed system is versioned and replayable. Users can inspect which fairness constraints eliminated a cycle.

## Fair partial orders

An interleaving lasso is often a poor explanation. A fair cycle can be represented as a recurring partial-order motif plus a schedule/fairness witness. Research question: can unfoldings or directed topology identify recurrent event-structure components without enumerating all linearizations?

A sound v0 still uses finite graph/SCC algorithms. Partial-order liveness is frontier work.

## Quantitative progress

Latency SLOs are not ordinary liveness. Under probabilistic/timed profiles, Continuum may establish:

- worst-case bound;
- probability of deadline violation;
- expected termination;
- almost-sure termination.

These claim types are incomparable. Statistical simulation does not prove a tail bound unless the sampling/model assumptions justify it.

## Assumption mining

When a counterexample is unfair, the tool can search for a minimal set of fairness/environment assumptions that excludes it. This is diagnostic, not permission to automatically strengthen the specification. Proposed assumptions appear as a diff and require human acceptance.

## Testing the liveness engine

Mutation classes:

- permanently enabled action starved;
- action enabled infinitely often but discontinuously;
- retry resets progress ranking;
- cancellation creates obligation cycles;
- crash/recovery livelock;
- timer continually postponed;
- fairness accidentally applied to a disabled action;
- hidden production effect blocks quiescence.

Differential corpus against TLC and other temporal model checkers is mandatory.

## Exit criteria

Liveness is credible only when:

- assumptions are explicit in source and results;
- fair-cycle witnesses replay;
- rankings are independently checked;
- cancellation progress is covered;
- property-directed POR is proven preserving or disabled;
- time and probability claims cannot be confused with qualitative liveness.
