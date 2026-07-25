# Semantic Context Compilation and Information Theory

## Goal

Select the smallest faithful context that enables a human or agent to choose the correct next action.

## Inputs

- evidence graph;
- task/question;
- actor capabilities and prior context;
- token/byte/node budget;
- required guarantee;
- privacy policy.

## Candidate facts

Events, state deltas, source spans, model actions, assumptions, proof lemmas, counterexamples, branch contrasts, and unknowns.

## Baseline algorithms

- backward causal slice;
- static/dynamic dependence;
- proof dependency slice;
- minimal unsat core/correction set;
- submodular relevance/diversity ranking;
- graph centrality;
- retrieval/reranking.

## Novel proposal: decision-sufficient context

Given a finite set of valid next actions and semantic worlds consistent with evidence, context is sufficient if no two worlds requiring different correct actions remain observationally equivalent. This turns context selection into active feature acquisition.

Approximate objective:

```text
maximize expected reduction in decision regret
subject to faithfulness, privacy, and budget
```

This is better than maximizing generic relevance.

## Novel proposal: rate-distortion for explanations

View compression as rate-distortion:

- rate: tokens/bytes/nodes;
- distortion: loss in diagnosis/repair/proof decision quality;
- hard constraints: causal/property preservation and omission transparency.

Human and agent empirical results estimate task-specific distortion. The compiler can select among representations.

## Novel proposal: semantic entropy budget

Raw event count poorly predicts context difficulty. Estimate uncertainty over:

- defect hypotheses;
- enabled branch choices;
- abstraction correspondence;
- proof lemmas;
- assumptions.

Spend context on high-entropy distinctions rather than repetitive logs.

## Expansion policy

Context Packs expose graph queries. A learned/analytic policy can propose the next expansion maximizing information gain, but clients may choose manually.

## Privacy

Redaction changes the set of distinguishable worlds. The pack must say when privacy filtering makes the task inconclusive or weakens guarantees.

## Experiments

- synthetic noisy causal traces;
- real concurrency bugs;
- Lean proof repair;
- liveness/fairness diagnosis;
- multi-agent handoff;
- private production trace.

Compare random/truncated context, retrieval, causal slice, proof slice, decision-sufficient selection.

## Kill criteria

- information-theoretic estimates do not predict task performance;
- learned ranking overfits benchmark families;
- context compilation cost is too high for interaction;
- compact packs repeatedly induce incorrect repairs despite preservation checks.
