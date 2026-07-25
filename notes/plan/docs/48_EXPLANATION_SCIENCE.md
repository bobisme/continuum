# Explanation Science Program

## Motivation

Formal verification adoption is often limited not by the absence of counterexamples but by the difficulty of understanding them. Continuum treats explanation quality as an empirical and formal research problem.

## Taxonomy

### Trace explanations

One or more ordered behaviors. Easy to produce, often hard to interpret.

### Slices

Remove events/state/formulae irrelevant to a target property.

### Minimal counterexamples

Minimize length, events, faults, owners, values, or causal configuration.

### Causal explanations

Identify events/interventions satisfying a declared causality definition.

### Contrastive explanations

Explain why failure occurred rather than a nearby safe outcome.

### Logical explanations

Unsat cores, proof slices, failed induction obligations, assumptions, fairness.

### Repair explanations

Minimal correction sets or interventions likely to restore intent.

## Formal claims

Every explanation object declares its guarantee:

```text
replay-preserving
property-preserving
1-minimal
cardinality-minimal
causally closed
counterfactual under model M
heuristic relevance only
```

Do not call a heuristic attention score a cause.

## Multi-objective explanation

Optimal explanation is not simply shortest. Objectives may include:

- faithfulness;
- size;
- semantic abstraction level;
- source locality;
- number of owners/faults;
- cognitive chunk count;
- repair utility;
- uncertainty.

Continuum should expose a small Pareto set where objectives conflict.

## Active diagnosis

When several defect hypotheses explain the current evidence, Continuum can choose the next experiment maximizing expected information gain:

- branch a schedule;
- inject a fault;
- request an event field;
- enable instrumentation;
- check a derived property;
- run a model bound.

The proposed experiment and its assumptions are explicit.

## Human study

Questions:

- Does causal/state-delta presentation improve diagnosis over raw trace?
- Which explanation level best serves experts vs newcomers?
- Do users correctly understand bounds and assumptions?
- Does contrastive branching improve repair quality?
- Does simplification create false confidence?

Metrics:

- accuracy;
- time;
- confidence calibration;
- retained understanding;
- repair correctness;
- ability to explain to another engineer.

## Agent study

Ablate:

- raw logs vs Context Packs;
- source-only vs model/refinement context;
- causal vs chronological traces;
- explicit omissions vs silent truncation;
- native expansion operations vs repository search;
- counterfactual branch availability.

Measure task success, token cost, invalid edits, overfitting, and invented claims.

## Research outputs

- benchmark corpus of explanations and diagnoses;
- causal-core algorithms;
- contrastive branch search;
- explanation certificates;
- adaptive context compiler;
- instrumentation recommendations;
- human/agent design guidelines.
