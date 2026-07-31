# Causal and Contrastive Counterexample Explanations

**Claim class:** research hypothesis

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

### Ratified threshold (frontier register FR: causal minimization)

quote-id=causal-minimization-core-ratio "On the research/26 experiment corpus of real failures — traces produced by an actual defect, excluding any trace padded with semantically inert events (plan §25) — the replay-preserving causal core must be ≤10% of trace length at the corpus median."

The corpus is the Experiments list above (durability/cancellation,
deadlock, fair/unfair liveness cycle, refinement mismatch, weak-memory
litmus test, insufficient production telemetry), restricted to the
classes in scope when the measurement is taken; each trace enters as the
checker produced it, with no seeded padding. Plan §24.5 quotes this
sentence verbatim under the same `quote-id`; the registered kill
condition is the existing one below — minimization cost dominates
verification.

Rationale: 10% is retained from the draft because the only executed
evidence — the G0-DX-01 spike's 200→4 event reduction (≈2%) — is bounded
by plan §25 to a synthetic trace whose 196 noise events are semantically
inert, so it validates the Context Pack artifact shape, not the context
compiler, and cannot license a tighter number on real failures. Ten
percent is also the weakest ratio at which a causal core is still an
order of magnitude cheaper to read than the trace it replaces, which is
what §12's causal-core level claims. The corpus median rather than a
per-trace bound is used because the experiment classes above differ
widely in trace length and one pathological class must not veto the
lane. If the threshold is missed the registered kill applies —
minimization cost dominates verification — and the lane falls back to
1-minimal delta debugging only, with no causal-core adequacy claim in
the Context Pack.

The neighboring exploration-reduction threshold (research/01: at least
an order-of-magnitude reduction on a non-artificial subset without a
serious regression on dependent workloads) governs a different lane — DPOR-style exploration reduction,
not causal minimization — and must not be conflated with this one; plan
§24.5 now carries them as separate register rows.

## Kill criteria

- causal claims too sensitive to arbitrary modeling choices;
- minimization cost dominates verification;
- users/agents perform no better than with simple state deltas;
- proof certificates become larger than useful evidence without improving trust.
