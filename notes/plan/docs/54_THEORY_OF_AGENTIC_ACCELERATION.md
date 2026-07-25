# Theory of Agentic Acceleration

## Claim

Advanced coding agents can expand the design space humans explore, but only when the environment supplies dense truthful feedback and prevents objective gaming.

## Current bottleneck

Concurrent/distributed algorithm work has a hostile search landscape:

- failures are sparse and schedule-dependent;
- tests provide weak gradients;
- logs are noisy and linearized;
- specifications drift;
- proofs require specialized context;
- performance objectives conflict with correctness;
- a local fix can create a remote liveness failure.

An agent can generate candidates faster than humans can validate them. Without Continuum, that increases risk rather than progress.

## Acceleration loop

```text
human intent
   ↓
agent proposes model/algorithm/invariant/proof
   ↓
Continuum finds exact counterexample or evidence
   ↓
Context compiler produces dense semantic feedback
   ↓
agent revises the right artifact
   ↓
repair/synthesis transaction resists gaming
   ↓
independent checker/Lean accepts evidence
   ↓
quality-diversity archive preserves discoveries
```

The critical quantity is not raw candidate throughput. It is **validated information gained per unit cost**.

## Feedback density

A raw failing test gives roughly one bit: pass/fail. A Continuum failure can provide:

- causal core;
- violated abstract relation;
- missing order;
- proof obligation;
- safe contrastive branch;
- intent constraints;
- source correspondence;
- counterexample generalization.

This turns search from blind mutation toward structured inference.

## Search decomposition

Agents are strongest when tasks are bounded and contexts are relevant. Evidence Graph decomposes work by missing edge rather than arbitrary role prompts. This supports parallelism without losing coherence.

## Trust amplification

Verification allows the system to use more aggressive candidate generators because correctness authority remains external. An untrusted creative agent may search unusual ideas; a small checker decides promotion.

## Invention modes

### Rediscovery

Reconstruct known algorithms from intent/holes. Validates Forge and provides training trajectories.

### Optimization

Find lower-cost implementations/refinements of a fixed protocol.

### Variant discovery

Explore alternative quorum/recovery/cancellation structures while retaining intent.

### Assumption frontier

Map tradeoffs between achievable guarantees and environment assumptions.

### New protocol discovery

Search broad typed spaces, preserve semantic diversity, and produce proof-bearing artifacts suitable for human mathematical analysis.

## Scientific safeguards

A claimed novel algorithm requires:

- explicit prior-art search;
- precise intent/assumptions;
- reproducible model and implementation;
- evidence/proof receipt;
- comparison baselines;
- adversarial review and hidden variants;
- performance evaluation;
- clear limits.

Continuum accelerates discovery; it does not confer novelty by itself.

## Long-term research opportunity

The combination of:

- true-concurrency semantics;
- property-directed causal abstraction;
- proof-producing reduction;
- program/model/proof co-synthesis;
- agent-native interaction;
- quality-diversity search;
- Lean-checked metatheory;

could support a new experimental mathematics of distributed algorithms. Candidate families become data; counterexamples become lemmas about impossible regions; proofs and implementations co-evolve; humans inspect the resulting structural patterns.

That is the micro-revolution: not agents writing more race-prone code, but agents and humans exploring systems design with a verification substrate strong enough to make radical experimentation responsible.
