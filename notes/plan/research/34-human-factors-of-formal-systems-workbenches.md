# Human Factors of Formal-Systems Workbenches

**Claim class:** evaluation methodology

## Premise

Formal correctness evidence is useful only when engineers understand what it establishes and act correctly on failures.

## Known adoption barriers

Industry studies and counterexample-explanation surveys identify difficulty with formal notation, incomplete models, refinement inconsistencies, and interpreting checker output. Engineers remain interested when tools reduce manual work and improve safety.

References:

- https://arxiv.org/abs/2304.08950
- https://arxiv.org/abs/2201.03061

## Continuum hypotheses

- causal/state-delta explanations improve diagnosis over raw traces;
- explicit assurance envelopes improve calibration over green/red badges;
- progressive disclosure serves both Rust engineers and formal experts;
- branch comparison makes concurrency mechanisms easier to understand;
- model/program correspondence navigation reduces drift errors;
- excessive formal detail at first contact increases abandonment.

## Study design

### Participants

Rust engineers, distributed-systems experts, formal-methods users, proof experts.

### Tasks

- identify ack-before-durable mechanism;
- distinguish deadlock from slow progress;
- classify fair/unfair liveness cycle;
- review property-weakening patch;
- decide whether production evidence is sufficient;
- repair model/program mismatch.

### Conditions

- raw logs/traces;
- conventional checker output;
- Continuum summary;
- Context Pack + debugger;
- agent-assisted Context Pack.

### Measures

accuracy, time, confidence, confidence calibration, repair quality, retention, transfer, cognitive load, expansion behavior.

## Novel proposal: assurance calibration curves

Treat user confidence as a probabilistic forecast and score calibration/Brier-like measures against task truth. A fast correct answer with wildly overconfident interpretation of bounded evidence is not ideal.

## Novel proposal: semantic wayfinding

Measure whether users can navigate among source, concrete execution, abstract model, property, and proof without losing the conceptual locus. Design correspondence breadcrumbs and persistent intent/assurance anchors.

## Accessibility

- non-color semantics;
- keyboard navigation;
- textual graph alternatives;
- scalable typography;
- no animation required to understand causality;
- screen-reader labels for evidence status and edges.

## Kill criteria

- experts prefer raw output and newcomers gain no accuracy;
- progressive disclosure hides key assumptions;
- causal graph increases confusion;
- assistance raises confidence faster than correctness.
