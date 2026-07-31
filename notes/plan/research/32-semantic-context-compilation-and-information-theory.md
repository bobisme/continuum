# Semantic Context Compilation and Information Theory

**Claim class:** research hypothesis

## Lane status (plan §24.5)

This note owns the **general context compilation (§6)** register row jointly
with research/25 (which supplies the ablation design: raw trace vs Context
Pack on the agent benchmark) and research/33 (which supplies the grading
discipline). The rate-distortion framing below is where the numeric win
margin lives, so the threshold is fixed here and plan §24.5 quotes it
verbatim.

**Ratified win margin.**
quote-id=context-compilation-win-margin "The Context Pack condition wins the general context-compilation ablation only if, on the held-out ContinuumBench diagnosis-and-repair split graded by the independent grader of research/33 with model, scaffold, and token budget held equal, it beats the raw-trace baseline by at least 10 percentage points of absolute task success with a 95 percent bootstrap confidence interval on the paired difference excluding zero, while delivering at least a 10x median per-task reduction in context bytes counting every expansion and staying within 2 percentage points of the raw-trace baseline on hidden-variant generalization."

Rationale. The rate-distortion view above makes a reduction figure alone
worthless: rate must be bought with decision quality, so the margin is a
conjunction of a rate term, a distortion term, and a non-inferiority guard.
The 10x rate floor is the only reduction number the program has already
committed to — G0-DX-01 required at least 10x and the finite spike measured
23.62x (17,645 bytes to 747) on a synthetic 200-event durability trace — and
docs/53 is explicit that this validated the artifact shape, not the general
compiler, so 10x is a demonstrated floor rather than an extrapolation, and it
is counted over every expansion so that a small first pack cannot hide its
cost in follow-up queries. The 10 percentage point absolute success margin is
the distortion term: it is large enough that it cannot be produced by grader
noise or a handful of tasks, which the paired bootstrap interval enforces
independently, and equal model, scaffold, and token budget applies research/33
principle 5 (resource-normalized reporting) so the pack condition cannot win
by spending more. The 2 percentage point non-inferiority band on
hidden-variant generalization is aimed directly at this note's last kill
criterion: a pack that induces incorrect repairs despite preservation checks
shows up first as an agent that fits the compiled slice and fails the
semantics-preserving variants of research/33, so the ablation may not be
called a win while that number degrades. The held-out split and isolated
grader follow docs/45's split discipline and plan §19.4.

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
