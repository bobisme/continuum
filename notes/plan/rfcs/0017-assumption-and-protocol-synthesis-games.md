# RFC 0017: Assumption and Protocol Synthesis as Games

**Status:** Proposed research lane  
**Target gate:** G6

## System/environment partition

Every nondeterministic choice is classified:

- system-controlled;
- implementation/scheduler-controlled;
- environment-controlled;
- adversarial fault;
- stochastic;
- angelic specification choice.

Conflating these choices makes realizability and probability meaningless.

## Game construction

Finite temporal models compile to turn-based or concurrent games with safety, Büchi, generalized Büchi, Streett, or parity objectives.

The engine can return:

- winning system strategy;
- spoiling environment counterstrategy;
- unrealizable core;
- candidate safety restriction;
- candidate fairness assumption;
- strategy implementation sketch.

## Assumption grammar

Synthesis is bounded by a declared grammar:

```text
never(drop class=control forever)
eventually(deliver m) when sender_alive(m)
weak_fair(schedule task) while obligation_pending(task)
at_most(k, crashes, per=epoch)
clock_drift <= epsilon
```

Candidates are ordered by implication/strength within the grammar. Claims of minimality are relative to this order.

## Production realizability

A synthesized assumption must map to:

- domain-pack guarantee;
- deployable mechanism;
- monitorable SLO;
- operator procedure;
- or explicit unverifiable premise.

An assumption that no real network/runtime can guarantee is displayed as a design defect, not a proof success.

## Protocol sketch synthesis

CML permits finite holes in guards, updates, quorum thresholds, retry policy, and phase ordering. Counterexample-guided synthesis explores candidates modulo semantic equivalence. Every synthesized candidate is reverified independently and mutation-tested.

Agents can propose grammars and sketches but cannot bypass finite/symbolic proof.

## Cancellation application

The game view is especially useful for cancel-correctness:

- environment requests cancellation at adversarial points;
- implementation chooses drain/finalize actions;
- budgets/timeouts constrain progress;
- objective requires no leaked obligations and eventual quiescence under responsiveness assumptions.

## Deliverable

A flagship demonstration should start from a liveness-failing replicated service, synthesize the weakest available delivery/scheduling assumption or protocol repair under the chosen grammar, and produce both the counterstrategy and checked repaired model.
