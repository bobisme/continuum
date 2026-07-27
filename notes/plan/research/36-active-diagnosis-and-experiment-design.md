# Active Diagnosis and Experiment Design

**Claim class:** research hypothesis

## Motivation

A failure may support several hypotheses. More passive logs may not distinguish them. Continuum can deliberately choose the next schedule, fault, observation, or proof query to maximize diagnostic value.

## Formalization

Let `H` be candidate defect hypotheses and `E` available experiments. Each experiment yields outcomes under the model. Choose:

```text
argmax_e  ExpectedInformationGain(H; outcome(e)) - Cost(e)
```

Under nondeterminism/adversarial environments, use minimax or robust information criteria rather than a naive probability distribution.

## Experiment types

- branch on enabled event;
- move cancellation checkpoint;
- insert/remove crash;
- choose message delay/duplication;
- expose one instrumentation field;
- evaluate a derived property;
- increase a model bound;
- request a proof lemma/countermodel;
- switch observer or abstraction for diagnosis only.

Diagnosis-only observer changes must not mutate protected intent.

## Novel proposal: distinguishing causal cuts

Given safe and failing execution families, find a minimal set of frontier choices or observed events separating them. This is related to decision trees over event structures and may yield compact diagnostic experiments.

## Novel proposal: telemetry synthesis as experiment design

For production conformance, choose the least-cost event fields/correlation edges that distinguish legal from illegal model explanations. Output:

- instrumentation requirement;
- expected event volume/privacy cost;
- exact claims it enables;
- remaining ambiguity.

## Novel proposal: proof-search experiment selection

When several lemmas could unblock a proof, estimate which finite countermodel or subgoal most reduces candidate invariant space. Agents receive the chosen discriminating goal.

## Experiments

- ack-before-durable hypotheses: finalizer vs storage profile vs telemetry;
- deadlock: missing wake vs scheduler starvation;
- liveness: protocol cycle vs unfair scheduler;
- refinement: wrong abstraction vs missing event;
- weak memory: compiler/hardware order vs algorithm.

## Success

Fewer runs/tool calls/tokens to correct diagnosis; no loss of soundness; explicit cost and assumptions.

## Kill criteria

- model mismatch makes selected experiments misleading;
- computing information gain costs more than brute-force exploration;
- learned priors dominate and fail on new protocols;
- privacy/production constraints make proposals impractical.
