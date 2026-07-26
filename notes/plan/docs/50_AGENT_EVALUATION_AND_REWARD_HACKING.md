# Agent Evaluation and Reward Hacking

## Core risk

An agent optimized for “make verification pass” may discover shortcuts that are formally valid for a modified problem and disastrous for the real one.

## Attack classes

### Intent attacks

- weaken property;
- strengthen assumptions/fairness;
- shrink bounds;
- remove faults;
- coarsen observer;
- lower assurance.

### Instrumentation attacks

- suppress semantic events;
- mark effects opaque;
- alter correspondence;
- bypass controlled runtime;
- fabricate production telemetry.

### Evidence attacks

- reuse stale receipts;
- exploit incremental invalidation bug;
- forge status/handle;
- present sampled result as exhaustive;
- hide unknowns.

### Overfitting attacks

- special-case exact seed/trace/value;
- fix one linearization;
- disable progress;
- exploit visible benchmark cases;
- hard-code corpus names.

### Resource attacks

- consume unlimited search until lucky;
- spawn redundant workers;
- avoid terminal state;
- return huge contexts to obscure failure.

## Defenses

- protected Intent Contract;
- semantic diff;
- hidden variants and mutations;
- neighborhood exploration;
- non-vacuity/positive scenarios;
- Incremental Parity Audit;
- capability security;
- resource-normalized evaluation;
- independent graders/checkers;
- explanation/evidence requirements.

## Evaluation methodology

Compare agent systems under:

- identical model/backend where possible;
- fixed wall/token/tool budgets;
- same native operations;
- public and hidden task variants;
- repeated seeds;
- cost and failure trajectory reporting.

Separate contributions from:

- base model;
- ACI design;
- context compiler;
- orchestration;
- verification engine;
- proof retrieval;
- search budget.

## Expensive failures

Track trajectories that consume high resources without progress. Continuum can terminate or suspend when:

- evidence graph has no novel nodes;
- repeated invalid actions;
- candidate semantics duplicates archive;
- proof state cycles;
- task is proven unrealizable within grammar;
- budget threshold reached.

The final result includes why work stopped and a continuation if useful.

## Agent confidence

Agent confidence is metadata only. Calibration is measured against checked outcomes, but it never changes evidence status.

## Training signals

High-quality trajectories include:

- intent-preserving diagnosis;
- valid tool sequencing;
- targeted context expansion;
- counterexample-guided repair;
- proof feedback and correction;
- explicit inconclusiveness;
- successful promotion receipts.

Negative trajectories include gaming attempts and stale/unsupported claims. This can train agents to interact with verification honestly.

## Benchmark success

A system succeeds only if it produces the required artifact and evidence. Natural-language answers alone cannot solve code/proof/synthesis tasks.
