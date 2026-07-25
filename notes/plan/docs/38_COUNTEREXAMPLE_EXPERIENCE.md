# Counterexample Experience

## Product principle

A counterexample is the beginning of diagnosis, not the end of verification.

## Canonical failure artifact

A crashpack contains:

- snapshot and Intent Contract;
- violated property and monitor state;
- canonical execution/configuration;
- exact choice/fault/replay data;
- concrete and abstract states;
- source/model/proof correspondence;
- causal/conflict graph;
- minimization record;
- assurance envelope;
- engine and semantic epochs.

## Explanation pipeline

```text
raw witness
  → validate/replay
  → normalize causal graph
  → property-directed slice
  → minimize while preserving failure
  → compute state/obligation deltas
  → contrast with nearest safe branch
  → map to source/model/proof
  → compile Context Pack
```

## Causal core

A causal core is closed under the dependencies needed to reproduce the violation. It may include:

- events directly observed by the property;
- causal predecessors;
- conflicts that selected the branch;
- obligation and resource transfers;
- fault preconditions;
- abstraction-relevant hidden events.

A smaller set based only on textual relevance is not accepted as replay-preserving.

## Minimality classes

- **1-minimal:** removing any one selected item destroys the witness.
- **cardinality-minimal:** no smaller selected set witnesses the failure.
- **causally minimal:** minimal configuration under causal closure.
- **value-minimal:** domains/names reduced.
- **owner-minimal:** tasks/nodes reduced.
- **fault-minimal:** no unnecessary injected fault.
- **explanation-minimal:** human/agent study objective, not purely set size.

The artifact records which class was achieved.

## State delta

Instead of dumping whole states:

```text
Concrete:
  disk.pending[e7]   + value X
  replies[e7]        reserved → published
  disk.stable[e7]    unchanged

Abstract:
  acknowledged[e7]   false → true
  durable[e7]        false

Violation:
  acknowledged[e7] ∧ ¬durable[e7]
```

## Missing order

For concurrency bugs, show the order constraint required by intent:

```text
required: SyncCompleted(e7) → ReplyPublished(e7)
observed: ReplyPublished(e7) || SyncCompleted(e7 absent)
```

Use `→`, conflict, concurrency, and absence precisely.

## Contrastive branch

The engine searches for a nearby safe execution and reports the minimal relevant difference. Distance may combine:

- changed scheduler choices;
- fault edit distance;
- causal graph edit distance;
- abstract state distance;
- owner switches.

The distance definition is part of the evidence.

## Source mapping

Each semantic event links to:

- source span;
- macro/generated origin;
- runtime effect primitive;
- model action;
- abstraction edge;
- proof obligation.

If mapping is incomplete, the UI says so and suggests instrumentation/annotation.

## Suggested repairs

Suggestions are hypotheses only. They are ranked by:

- intervention success on the witness;
- locality;
- semantic intent preservation;
- neighboring branch robustness;
- proof impact;
- cost.

They enter a Repair Transaction before any claim.

## Failure of explanation

Continuum must return `ExplanationIncomplete` when it cannot produce a faithful bounded explanation. It may still provide the raw witness. It must not fabricate a tidy narrative.
