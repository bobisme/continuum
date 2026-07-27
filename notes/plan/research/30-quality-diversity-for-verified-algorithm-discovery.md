# Quality-Diversity for Verified Algorithm Discovery

**Claim class:** research hypothesis

## Problem

Optimization usually returns one best candidate under a scalar objective. For algorithm invention, this destroys structural diversity and may hide qualitatively different solutions.

## Proposal

Use quality-diversity (QD) search where archive cells are defined by semantic/architectural descriptors and candidates enter only after required verification evidence.

Possible descriptors:

- message rounds/count;
- quorum intersection geometry;
- stable-write count;
- state bytes;
- concurrency width;
- recovery/cancellation strategy;
- fairness strength;
- failure-detector reliance;
- proof size/quantifiers;
- refinement stuttering depth.

## Verified archive

Each cell stores:

```text
candidate artifact
behavior descriptor evidence
objective vector
assurance envelope
counterexample history
proof/certificate receipts
semantic equivalence links
```

Unverified candidates may inhabit a provisional archive but cannot dominate verified cells in reports.

## Novel proposal: counterexample morphology as diversity

Characterize candidates by the shape of near-failing counterexamples or eliminated interpretation classes. Two correct protocols with similar cost but different vulnerability boundaries may be valuable alternatives.

## Novel proposal: homological/causal descriptors

Experimental descriptors from execution geometry:

- trace-class width;
- cube/higher-dimensional concurrency counts;
- causal bottleneck hyperplanes;
- Betti-like features of independence complexes.

These must prove practical value versus simpler descriptors or be deleted.

## Search

Combine MAP-Elites/novelty search with typed mutation/crossover and LLM transformations. Every mutation is a semantic proposal; invalid syntax/fragment candidates are filtered cheaply, then staged verification.

## Evaluation

- number of behaviorally distinct verified candidates;
- rediscovery of known families;
- Pareto improvements;
- hidden variant robustness;
- human expert rating of conceptual novelty;
- archive stability under descriptor changes;
- proof cost.

## Risks

- descriptor gaming;
- syntactic diversity mislabeled semantic;
- verification budget spread too thin;
- huge archives with no insight;
- novelty mistaken for usefulness.

## Kill criteria

If QD does not discover useful alternatives beyond multi-objective Pareto search on known benchmarks, keep only a lightweight semantic-diversity archive.
