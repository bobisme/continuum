# Continuum Forge: Verified Algorithm Invention

## Vision

Forge lets humans define what a concurrent/distributed system must accomplish while agents and synthesis engines search the space of algorithms, invariants, abstractions, and implementations. Continuum checks every candidate.

## Forge is not code generation

Code generation maps a known design into syntax. Forge searches behavioral space under formal constraints.

## Problem definition

```rust
struct ForgeProblem {
    snapshot: SnapshotHandle,
    intent: IntentHandle,
    sketch: TypedSketch,
    holes: Vec<TypedHole>,
    hard_constraints: Vec<Claim>,
    positive_scenarios: Vec<Scenario>,
    objectives: Vec<Objective>,
    diversity: Vec<BehaviorDescriptor>,
    assurance: AssuranceRequirement,
    budget: ForgeBudget,
}
```

## Typed holes

Hole categories:

- predicate/guard;
- deterministic or nondeterministic update;
- message rule;
- quorum family;
- retry/finalizer policy;
- state summary;
- invariant/lemma;
- ranking function;
- abstraction map;
- auxiliary state;
- environment assumption;
- implementation primitive.

Assumption holes are dangerous and require explicit synthesis policy. Forge should prefer algorithm repair over environment restriction and report unrealizability separately.

## Candidate lifecycle

```text
Proposed
  → Type/fragment checked
  → Fast finite screen
  → Counterexample-guided refinement
  → Portfolio verification
  → Proof/refinement obligations
  → Cost evaluation
  → Diversity archive
  → Materialization proposal
```

No candidate becomes “correct” because an LLM says so.

## CEGIS loop

1. Generate candidate from grammar/sketch and learned priors.
2. Check positive scenarios/non-vacuity.
3. Check safety/liveness/refinement under current examples.
4. Obtain counterexample or proof evidence.
5. Generalize counterexample to eliminate an interpretation class when sound.
6. Refine candidate/invariant/ranking/abstraction together.
7. Repeat or prove unrealizable within the finite grammar.

## Interpretation reduction

Candidates that are syntactically different but semantically identical over relevant interpretations should share one representative. This can reduce enormous redundant search spaces. The reduction must be scoped to the current synthesis domain and checked/validated.

## Co-synthesis graph

```text
algorithm candidate
  ├── requires invariant candidate
  ├── requires abstraction candidate
  ├── requires liveness ranking/fairness
  ├── induces implementation correspondence
  └── has objective vector
```

A counterexample is routed to the appropriate component rather than forcing blind algorithm mutation.

## Search engines

- grammar enumeration;
- SAT/SMT/SyGuS;
- CHC solving;
- IC3/PDR invariant discovery;
- game solving;
- program synthesis agents;
- evolutionary strategies;
- novelty/quality-diversity search;
- MCTS over typed transformations;
- proof-guided search;
- corpus retrieval and analogy.

The orchestrator uses empirical portfolio selection but preserves reproducibility.

## Quality-diversity

Archive cells are semantic descriptors, for example:

```text
(message rounds, quorum shape, durable writes, state bytes,
 cancellation points, recovery style, fairness strength, proof size)
```

Candidates in one cell compete by objective. Distinct cells preserve alternative ideas and enable human insight.

## Unrealizability

Forge must distinguish:

- no candidate found under budget;
- finite grammar exhausted;
- verified unrealizability under a formal synthesis model;
- intent internally inconsistent;
- implementation/effect constraints too restrictive;
- proof engine inconclusive.

Where possible, return reusable unrealizability cores or environment assumptions required for realizability.

## Materialization to Rust

Generation uses verified/translation-validated templates around asupersync effects. Output contains explicit TODO/proof obligations for unsupported components. Generated code enters a Repair/Design Transaction and is tested against the same model.

## Agent roles

- sketch architect;
- candidate proposer;
- invariant synthesizer;
- liveness analyst;
- proof worker;
- adversarial falsifier;
- performance optimizer;
- novelty curator.

Evidence Graph prevents persuasive coordination from replacing checks.

## Research target

After rediscovering known protocols, test genuinely open design spaces:

- cancellation-safe distributed commit;
- lower-write durable queues;
- dynamic quorum/replication tradeoffs;
- recovery protocols under explicit storage profiles;
- lock-free/async structures with proof-carrying linearization;
- resource schedulers optimizing tail latency under safety constraints.

Novel results must be published with model, code, intent, evidence, and proof artifacts.
