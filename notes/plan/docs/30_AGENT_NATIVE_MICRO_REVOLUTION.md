# Agent-Native Concurrent Systems Engineering

## Premise

Agents will soon generate more concurrent Rust code than humans can manually audit. Current feedback loops are inadequate:

```text
agent writes code → tests happen to pass → merge
```

Continuum changes the loop to:

```text
agent proposes model/code/proof
 → semantics and effects checked
 → schedules/faults explored
 → counterexample minimized
 → repair proposed
 → certificate checked
 → evidence attached to change
```

## Agent-facing primitives

### `continuum model`

Create or update a standalone model from a design, with corpus analogues and unresolved assumptions listed.

### `continuum challenge`

Generate adversarial mutations, missing fairness cases, crash windows, cancellation points, and abstraction counterexamples.

### `continuum explain`

Return causal core, abstract delta, violated lemma, proof slice, assumption involvement, and source locations.

### `continuum repair`

Produce candidate code/model changes but no claim; automatically launches replay and neighboring exploration.

### `continuum prove`

Emit Lean goals, candidate invariant grammar, finite counterexamples to induction, and solver certificate tasks.

### `continuum review`

Compare evidence before/after a PR: weaker properties, stronger assumptions, reduced coverage, new opaque effects, invalidated proof edges.

## Stable machine contracts

Agents consume JSON/CBOR schemas, not human terminal output. Every command supports:

- deterministic IDs;
- bounded resource budget;
- resumable search state;
- evidence references;
- exact semantic epoch;
- structured diagnostics;
- no implicit prompt interpretation in the verifier core.

## Autonomous swarm decomposition

A complex proof/repair can be decomposed into agents for:

- corpus analogy search;
- invariant synthesis;
- liveness ranking;
- abstraction/refinement mapping;
- Rust repair;
- Lean proof;
- adversarial mutation;
- evidence review.

They communicate through the proof graph and claim ledger. A coordinator cannot declare success until machine gates close.

## Anti-reward-hacking design

Agents may be tempted to:

- weaken the property;
- strengthen fairness;
- reduce model bounds;
- hide events;
- change abstraction maps;
- mark external effects opaque;
- overfit the exact counterexample.

Continuum computes semantic diffs on all of these and treats them as first-class review items. Mutation tests and neighboring exploration detect trivial properties and narrow patches.

## Training/evaluation corpus

The 80 TLA+ examples plus generated mutations form a high-quality agent benchmark:

- translate formal models;
- repair semantic defects;
- infer invariants;
- prove theorems;
- classify liveness assumptions;
- connect model to Rust.

Unlike text-only benchmarks, every output has executable or kernel-checked grading.
