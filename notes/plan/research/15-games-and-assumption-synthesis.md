# Research 15: Games and Assumption Synthesis

## Problem

A liveness property often fails because the environment can drop every message, crash every leader, never schedule a task, or cancel at pathological points. Engineers add fairness assumptions manually until the property passes. This hides the actual contract.

## Foundational direction

Environment-assumption synthesis formulates the problem as a game and seeks safety/liveness restrictions making a specification realizable. Minimal fairness assumption selection is generally hard, so “weakest” must be scoped to a grammar/order.

Foundational source:
https://arxiv.org/abs/0805.4167

Recent protocol synthesis work shows counterexample-guided sketching can synthesize substantial TLA+ distributed protocols and exploit semantic equivalence reduction:
https://arxiv.org/abs/2405.07807
https://arxiv.org/abs/2501.14585

## Continuum opportunity

Continuum already classifies nondeterminism and owns domain-pack fault semantics. This enables a practical game boundary:

```text
System player: protocol actions/repair choices
Environment player: clients, network, crashes, cancellation timing
Scheduler player: task/action scheduling under declared control
Chance: probabilistic delays/faults where explicitly modeled
```

## Outputs

### Counterstrategy

A finite-state strategy explaining how the environment defeats the property. This is far more actionable than one lasso because it describes a family of failures.

### Assumption candidate

Examples:

- eventually deliver control messages between live nodes;
- do not crash more than `f` nodes per epoch;
- weakly fairly poll a task while it owns an obligation;
- clock uncertainty remains below lease margin;
- cancellation checkpoints are reached within a responsiveness rank.

### Protocol repair

For sketched guards/updates, synthesize an implementation strategy or prove unrealizability under current architecture.

## Novel proposal: operational assumption type

Every assumption has four interpretations:

```text
logical formula
simulator adversary restriction
production monitor/SLO
deployment mechanism or human procedure
```

A proof that relies on an assumption with no operational interpretation is flagged as non-deployable.

## Assumption strength lattice

Within a grammar, implication creates a partial order. Continuum reports Pareto-minimal candidates by:

- logical strength;
- implementation cost;
- observability;
- availability impact;
- security implications.

## Cancellation game

Cancellation is naturally adversarial: it may arrive at any checkpoint. The system must drain and finalize within budgets. A ranking/parity game can synthesize where checkpoints, reservation boundaries or cleanup obligations must exist.

## Risks

- state explosion in parity/Streett games;
- grammars encode the answer;
- synthesized assumptions are too strong or unmonitorable;
- multiple environment players create imperfect-information games;
- chance and adversarial choices must not be conflated.

The first product output should be counterstrategies; assumption synthesis follows once users trust the game semantics.
