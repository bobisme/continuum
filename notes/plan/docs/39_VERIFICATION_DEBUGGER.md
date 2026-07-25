# Verification Debugger

## Problem

A normal debugger answers “what did this execution do?” A verification debugger must answer:

- which semantic events could occur next;
- why each event is enabled or disabled;
- which alternatives were pruned and why;
- where the chosen execution diverged from a safe one;
- how concrete steps map to abstract transitions or stuttering;
- what fairness/progress obligations accumulate;
- which fault or cancellation choices matter.

## Debug state

```rust
struct DebugState {
    snapshot: SnapshotHandle,
    intent: IntentHandle,
    execution: ExecutionHandle,
    configuration: ConfigurationId,
    causal_past: EventSet,
    enabled: Vec<EnabledEvent>,
    observers: Vec<ObserverState>,
    abstractions: Vec<AbstractState>,
    obligations: ObligationTree,
    fairness: FairnessLedger,
    frontier: FrontierSummary,
}
```

Handles are stable within an immutable execution artifact. Large values use lazy child handles.

## Stepping modes

### Semantic event

Execute one chosen event from the enabled frontier.

### Abstract step

Run concrete internal/stuttering events until the selected abstraction changes or a property event occurs.

### Owner turn

Run until task/node/region ownership changes.

### Property step

Run until the monitor changes state.

### Causal reverse

Move to a causally closed predecessor configuration. If multiple maximal events can be removed, return choices.

### Counterfactual branch

Select a different legal event/fault at a prior configuration and create a new branch handle.

## Breakpoints

Breakpoints can target:

- source line/function;
- model action;
- event kind or effect phase;
- property monitor state;
- abstract predicate;
- obligation creation/resolution;
- cancellation phase;
- durability boundary;
- fault injection;
- fairness debt threshold;
- correspondence mismatch.

A breakpoint is compiled into an observer/query and versioned with the debug task.

## Why enabled

The engine returns a proof-like derivation tree:

```text
Deliver(m17, n2)
├─ pending(m17)
├─ destination(m17) = n2
├─ running(n2)
├─ connected(n1,n2)
├─ now ≥ earliest_delivery(m17)
└─ budget allows delivery
```

For disabled actions, return blocking leaves and whether they can become true.

## Branch comparison

Comparing branches produces:

- common causal prefix;
- first conflicting choice;
- event additions/removals;
- state/observer deltas;
- property outcome difference;
- assumption/fairness differences;
- cost difference.

This is a semantic diff between executions, not just a textual trace diff.

## Partial-order visualization

The primary visualization is a layered causal graph:

- horizontal grouping by node/task/region;
- causality arrows;
- conflict/alternative edges;
- intervals for operations;
- resource/obligation flow;
- abstract transition bands;
- property-monitor timeline.

A total-order timeline remains available for replay but is explicitly marked as one linearization.

## DAP adaptation

DAP object references are ephemeral adapter handles backed by stable Continuum handles. Suggested mapping:

| DAP | Continuum |
|---|---|
| thread | node/task/abstract process |
| stack frame | abstraction/source/effect frame |
| scope | concrete state, abstract state, obligations, frontier |
| variable reference | lazy artifact query |
| breakpoint | semantic predicate/event query |
| step in | semantic event or abstraction descent |
| step over | abstract step |
| step back | causal reverse |
| restart frame | branch from configuration |

Custom requests expose causal frontier and branch comparison.

## Correctness

The debugger does not mutate original evidence. Every branch is a new execution artifact. Replay validates selected transitions. The UI cannot manufacture an event not accepted by the semantic engine.

## Agent use

Agents can ask bounded questions:

```text
show the three events enabled before the violation
branch by forcing SyncCompleted first
compare property and abstract state
show the last event that changed reply obligation
```

This is substantially more reliable than asking an agent to infer concurrency from a linear log.
