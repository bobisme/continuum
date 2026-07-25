# Protocol, Invariant, Ranking, and Abstraction Co-Synthesis

## Motivation

Synthesizing a protocol alone often produces candidates that are hard to prove. Synthesizing an invariant for a fixed bad protocol is futile. Liveness may require ranking functions or fairness structure; implementation refinement may require auxiliary state or a better abstraction.

## Candidate tuple

```text
C = (Protocol, Invariant, Abstraction, Ranking, Fairness, AuxiliaryState)
```

Correctness constraints couple components:

```text
Init ⇒ Inv
Inv ∧ Step ⇒ Inv'
Inv ⇒ Safety
ConcreteStep ⇒ AbstractStep ∨ Stutter
Ranking decreases/progress under fairness
```

## Related work

Protocol synthesis systems such as Scythe and interpretation reduction, invariant systems such as IC3PO/Endive/DistAI, CHC/SyGuS synthesis, and ranking-function verification provide specialized pieces.

References:

- https://arxiv.org/abs/2501.14585
- https://arxiv.org/abs/2103.14831
- https://arxiv.org/abs/2108.08796
- https://sygus-org.github.io/

## Novel proposal: coupled counterexample typing

Normalize verifier feedback into:

```text
InitFailure
SafetyTrace
InductionCounterexample
RefinementMismatch
FairCycle
RankingViolation
UnrealizableEnvironment
ImplementationConstraintViolation
```

Each type updates only relevant candidate components while preserving learned constraints for others.

## Novel proposal: proof-complexity objective

Include proof cost as an optimization objective:

- number/size of invariant clauses;
- quantifier alternation;
- auxiliary state;
- ranking dimension;
- Lean proof dependency size;
- certificate checking cost.

A slightly slower protocol with a dramatically simpler proof may be preferable.

## Novel proposal: abstraction as search control

Start with a coarse abstraction. Spurious counterexamples drive refinement. But allow protocol changes that make a simpler abstraction sound. This turns CEGAR into co-design rather than one-way model refinement.

## Search architecture

- typed grammar enumeration/LLM proposals;
- interpretation-class pruning;
- IC3/PDR for invariant hints;
- CHC/ranking synthesis;
- game solver for environment assumptions;
- quality-diversity archive;
- independent verifier/Lean for acceptance.

## Benchmarks

- acknowledgement/durability guard;
- mutual exclusion;
- lock service with cancellation;
- two-phase commit recovery;
- reliable broadcast variants;
- quorum register;
- small leader election;
- dynamic resource allocator.

## Success

- rediscover known algorithms;
- solve tasks that separated synthesis cannot;
- produce smaller proof artifacts;
- generalize to hidden parameter/value variants;
- expose unrealizability rather than time out.

## Kill criteria

- joint space overwhelms gains from coupled feedback;
- proof complexity objective biases toward trivial/slow designs;
- abstractions overfit finite bounds;
- agent proposals dominate and cannot be reproduced by structured search.
