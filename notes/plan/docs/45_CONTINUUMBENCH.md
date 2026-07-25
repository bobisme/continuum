# ContinuumBench

## Mission

ContinuumBench evaluates whether humans and agents can use Continuum to produce trustworthy concurrent/distributed systems—not merely whether a model emits code that passes visible tests.

## Benchmark object

Each task bundle contains:

```text
public workspace snapshot
protected Intent Contract
allowed capabilities
budget
public task description
hidden semantic variants
hidden mutations
independent grader
expected evidence class
security policy
```

The grader is isolated from the agent and workbench clients.

## Task tracks

### Modeling

- formalize a prose protocol;
- port a TLA+ example;
- identify missing assumptions;
- select correct observers/fairness;
- construct abstraction layers.

### Diagnosis

- safety failure;
- deadlock/futurelock;
- liveness/fairness cycle;
- cancellation/obligation leak;
- durability/recovery;
- weak-memory ordering;
- refinement mismatch;
- insufficient production telemetry.

### Repair

- Rust source;
- model;
- correspondence;
- invariant/proof;
- instrumentation;
- domain-pack profile.

### Proof

- inductive invariant;
- refinement theorem;
- ranking function;
- certificate checker extension;
- Lean proof repair.

### Synthesis

- finite guard/update holes;
- quorum/message rules;
- invariant/ranking co-synthesis;
- unrealizability classification;
- cost optimization with behavioral diversity.

### Governance/security

- detect property weakening;
- reject assumption/bound/fault gaming;
- avoid stale snapshot/receipt;
- resist prompt injection in source/logs;
- respect capabilities and privacy;
- distinguish inconclusive from pass.

## Sources

1. TLA+ Examples corpus, with license/provenance preserved.
2. Generated semantics-preserving and defect mutations.
3. Historical bugs from concurrent Rust/runtime/storage systems.
4. Asupersync Lab/cancellation cases.
5. Continuum's own engine bugs.
6. Real project migrations with sanitized artifacts.
7. Forge-generated hidden variants.

## Split discipline

Avoid benchmark leakage through:

- family-level train/test separation;
- source-hash and semantic-clone detection;
- renamed/restructured variants;
- hidden observers and fault profiles;
- generated values/topologies;
- proof-library version shifts;
- held-out real projects.

## Grading

A task score is a vector:

```text
intent_integrity
semantic_correctness
required_evidence
hidden_variant_generalization
repair robustness
proof/certificate validity
cost and latency
invalid actions
security compliance
explanation quality
```

A zero in intent integrity caps the total at failure.

## Agent effectiveness

Measure both accuracy and resources:

- wall time;
- verifier CPU/memory;
- model tokens;
- tool calls;
- context bytes and expansion count;
- retries/stale operations;
- expensive unsuccessful trajectories.

This prevents a scaffold from appearing superior solely by consuming extreme resources.

## Explanation evaluation

For agents:

- correct defect classification;
- relevant source localization;
- causal mechanism statement;
- appropriate next operation;
- no invented evidence.

For humans:

- diagnosis accuracy/time;
- repair choice;
- assurance comprehension;
- confidence calibration;
- transfer to a new example.

## Mutation challenges

Per task, generate:

- trivial true property;
- property with removed conjunct;
- stronger assumption;
- smaller bound;
- removed fault;
- hidden observer event;
- stale receipt;
- exact-trace hard-code;
- disabled instrumentation;
- semantically equivalent distractor patch;
- source comment containing malicious agent instructions.

## Leaderboards

Report Pareto fronts, not one opaque score:

- strongest evidence under fixed cost;
- lowest cost at fixed success;
- highest intent integrity/generalization;
- human-agent team performance;
- Forge novelty under proof constraints.

## Scientific artifacts

Every benchmark run stores:

- tool/model versions;
- task snapshot and intent hashes;
- operation trace;
- evidence graph;
- final patch/model/proof;
- grader receipt;
- resource metrics.

This makes agent-system research reproducible and exposes where interface design, model capability, and verifier quality contribute.
