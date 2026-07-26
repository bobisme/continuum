# Causal and Contrastive Counterexample Explanations

## Problem

Model checkers produce witnesses, but a witness is not automatically an explanation. The counterexample-explanation literature reports heavy use of traces, minimization, visualization, localization, and logic-specific techniques, while industrial studies show raw verification output remains difficult to interpret.

References:

- https://arxiv.org/abs/2201.03061
- https://arxiv.org/abs/2304.08950
- https://doi.org/10.1007/s10664-023-10353-4

## Continuum setting

Continuum has richer data than traditional state traces:

- event causality/conflict;
- concrete and abstract states;
- observer/property automata;
- asupersync obligations/cancellation;
- source correspondence;
- fault/durability phases;
- alternate branches.

This permits explanations that distinguish causal mechanism from incidental schedule order.

## Formal explanation object

For execution structure `E`, property `P`, and observer `O`, an explanation candidate `X` includes selected events, conditions, and interventions. Possible guarantees:

```text
Witness(E, P)
Closed(X)
Replay(X, semantics) refutes P
Minimal_k(X)
Contrast(X, safe_execution)
ActualCause_M(X, P)
```

The causality model `M` must be named. Halpern–Pearl-style counterfactual causation, structural causal models, event-structure causality, and minimal correction sets answer different questions.

## Novel proposal: layered causal certificates

A Context Pack can include a certificate chain:

1. selected events form a valid causally closed configuration;
2. replay/partial execution reaches a violating monitor state;
3. omitted events commute or are observer-irrelevant under a checked relation;
4. a counterfactual intervention leads to a safe branch;
5. claimed minimality class checked by deletion/solver proof.

This makes explanation an evidence object rather than narration.

## Novel proposal: explanation lattice

Explanations form a partial order by information and abstraction:

```text
raw execution
  ≥ causal slice
  ≥ abstract transition slice
  ≥ source-local mechanism
  ≥ one-sentence contrast
```

But no single chain fits all users. Define a lattice where joins combine complementary explanations and meets find common causal kernels. Agents can request the least explanation sufficient to choose among available repair actions.

## Novel proposal: causal responsibility over obligations

For cancellation/durability bugs, events alone may mislead. Model linear obligations/resources and compute responsibility for an unresolved or prematurely discharged obligation. A finalizer that publishes a reply can be the responsible transition even if cancellation “triggered” the path.

## Active contrast generation

Find a nearest safe execution under a declared distance:

```text
schedule-choice edits
fault edits
causal graph edits
abstract state edits
owner switches
```

Then compute the minimal relevant difference. Distance sensitivity should be visible; multiple Pareto contrasts may be better than one arbitrary nearest trace.

## Experiments

- durability/cancellation;
- deadlock;
- fair/unfair liveness cycle;
- refinement mismatch;
- weak-memory litmus test;
- insufficient production telemetry.

Compare raw trace, minimized trace, causal core, state delta, and contrastive explanation with humans and agents.

## Success

- explanation preserves/refutes property as claimed;
- improved diagnosis and repair;
- lower false-confidence rate;
- small Context Packs without omitting mechanism;
- explanations stable under irrelevant commuting events.

### Promotion threshold (draft, pending ratification)

Registered in plan §24.5: a replay-preserving causal core ≤10% of trace
length on real (non-synthetic) failures. The registered kill condition
is the existing one below: minimization cost dominates verification.
This is a draft pending lane-owner ratification; an unratified
threshold may not survive Phase A.

The neighboring exploration-reduction threshold (research/01: ≥10×
reduction in explored classes, without regression on dependent
workloads) governs a different lane — DPOR-style exploration reduction,
not causal minimization — and must not be conflated with this one; plan
§24.5 now carries them as separate register rows.

## Kill criteria

- causal claims too sensitive to arbitrary modeling choices;
- minimization cost dominates verification;
- users/agents perform no better than with simple state deltas;
- proof certificates become larger than useful evidence without improving trust.
