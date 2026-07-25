# Research Note 08: Proof-Gated Agentic Verification

**Claim class:** research program  
**Relevant sources:** [S69], [S70], [S70A]

## Thesis

Continuum should be the verification substrate on which coding agents can safely iterate:

```text
discover counterexample
→ localize causal/refinement failure
→ propose patch or invariant
→ replay exact failure
→ explore neighboring equivalence classes
→ discharge proof obligations
→ independently check certificate
```

The agent is never trusted. It is a search heuristic whose output crosses deterministic evidence gates.

## Machine-legible artifacts

Every failure package contains:

```text
manifest.json
claim.json
causal-trace.cir
minimal-core.cir
linear-replay.cir
state-diff.json
obligation-flow.json
property.json
assumptions.json
replay.toml
explanation.md
```

Stable schemas let an agent reason without scraping terminal prose.

## Causal diagnosis

The explanation engine computes:

- backward causal cone from the violation;
- minimal fault/event core;
- first broken view/refinement edge;
- obligation/resource imbalance;
- effect-phase mismatch;
- competing passing execution;
- source/provenance sites;
- candidate repair templates.

This produces a better repair problem than a 50,000-step interleaving.

## Patch search spaces

Initially restrict synthesis to auditable transformations:

- move acknowledgement after commit/sync;
- add/strengthen guard;
- add epoch validation;
- make operation idempotent;
- add cancellation drain/finalizer;
- reorder independent-looking effects;
- bind task to a region;
- strengthen an abstraction predicate;
- add a missing fairness assumption as a specification proposal, not code.

Unrestricted code generation remains possible but receives no special trust.

## Invariant synthesis

Agents can propose invariants using:

- counterexample states;
- reachable-state samples;
- templates from model types;
- symmetry;
- unsat cores;
- IC3 clauses;
- natural-language design intent.

Every invariant is checked for initiation, consecution, and property implication. “Plausible” invariants do not enter the claims matrix.

## Proof search orchestration

The agent may select engines, decompose goals, request lemmas, and generate Verus/Lean/SMT annotations. Proof objects and deterministic rechecks remain the authority.

Research on AutoVerus and later agent-based Verus systems indicates meaningful automation potential, but evaluations can overfit benchmark styles. Continuum should maintain hidden mutation and semantic-shift sets.

## Anti-reward-hacking controls

- hidden mutants and metamorphic transformations;
- separate model and implementation agents where possible;
- no access to expected counterexample seed in evaluation;
- certificate and replay checks isolated from the agent;
- detect assumption strengthening or property weakening;
- semantic diff of every accepted patch;
- adversarial prompt/data contamination tests;
- bounded tool permissions.

## Novel proposal: Neighborhood Closure Gate

Fixing one schedule can merely move the bug. After replay passes, automatically explore a neighborhood defined by:

- causal swaps around the original core;
- nearby fault placements;
- alternative linearization witnesses;
- small data perturbations;
- one additional cancellation/crash;
- symmetry-renamed instances.

A patch is not accepted until this closure campaign and the original proof obligation pass.

## Novel proposal: Proof-Carrying Repair

A repair submission contains:

```text
patch
failure replay result
semantic diff
new/changed assumptions
certificate(s)
coverage delta
remaining unsupported obligations
```

Review focuses on contract changes and evidence, not agent confidence.

## Evaluation

- known distributed/concurrency bugs;
- hidden mutants;
- novel model/implementation drift;
- cancellation and durability defects;
- invariant synthesis tasks;
- time-to-certificate;
- false-fix rate;
- property weakening rate;
- human review effort.

## Product boundary

Agent features must remain optional. Core Continuum is deterministic and usable without an LLM. No proof claim depends on a model service being available or honest.
