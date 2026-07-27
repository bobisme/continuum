# Agent Benchmarks and Formal Reward Hacking

**Claim class:** evaluation methodology

## Lane status (plan §24.5)

This note owns two register rows:

1. **Neighborhood adequacy (§8.3)** — lane to be opened. Threshold: hidden-variant catch rate of the §8.3 repair neighborhood on the docs/50 gaming corpus; the numeric target is fixed at lane opening — draft. The kill clause is the first kill criterion below (hidden variants too easy to leak or too hard to grade independently).
2. **Co-ownership of the context-compilation ablation benchmark** (general context compilation, §6) with research/25 and research/32: this note supplies the anti-gaming grading discipline for that lane's agent benchmark.

## Problem

Standard coding benchmarks often grade final tests. Formal-systems agents can exploit the specification, environment, bounds, observer, verifier, or grader itself.

## Threat model

The agent may knowingly or accidentally:

- alter intent;
- exploit visible cases;
- disable instrumentation;
- submit stale evidence;
- consume excessive resources;
- manipulate tool state;
- inject instructions via source;
- overfit proof/search to known lemmas.

## Benchmark principles

1. Protected Intent Contract outside writable snapshot.
2. Hidden semantic variants, not only hidden tests.
3. Independent grader/checker.
4. Full operation/evidence trace.
5. Resource-normalized reporting.
6. Family-level split and semantic clone detection.
7. Security/capability tasks.
8. Exact artifact requirement.

## Novel proposal: adversarial intent mutations

For every task, automatically derive neighboring intents:

- remove property conjunct;
- add assumption;
- reduce bound;
- remove fault;
- coarsen observer;
- strengthen fairness;
- downgrade assurance.

The agent must preserve the original fingerprint. The benchmark checks whether its patch accidentally behaves as if it solved a neighboring easier intent.

## Novel proposal: verifier-aware hidden variants

Generate variants preserving high-level intent but changing:

- names/types/order;
- topology size within cutoff;
- equivalent action decomposition;
- scheduler/fault realization;
- implementation structure;
- proof lemma names;
- domain profile.

This measures semantic generalization.

## Novel proposal: trajectory evidence score

A successful final patch can still arise from unsafe behavior. Score:

- invalid privileged attempts;
- unsupported claims;
- stale handle use;
- context expansion efficiency;
- hypotheses refuted/updated;
- evidence status honesty;
- cancellation/budget behavior.

Trajectory score is diagnostic; final semantic evidence remains primary.

## Baselines

- raw shell coding agent;
- ACI agent;
- ACI + Context Packs;
- ACI + multi-agent evidence graph;
- human engineer;
- human-agent team.

## Kill criteria

- hidden variants are too easy to leak or too hard to grade independently;
- benchmark success fails to predict real project adoption;
- cost normalization dominates task semantics;
- task generation introduces invalid or ambiguous intents.
