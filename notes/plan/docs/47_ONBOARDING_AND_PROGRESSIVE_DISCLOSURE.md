# Onboarding and Progressive Disclosure

## Goal

A Rust engineer should benefit from Continuum before learning temporal logic. An expert should never be trapped behind simplified views.

## Concept ladder

### Level 0 — Deterministic replay

Learn:

- controlled time/entropy/effects;
- one seed/choice log reproduces a failure;
- cancellation and crash windows.

### Level 1 — Invariants and causal failures

Learn:

- state property;
- shortest/minimized counterexample;
- causal vs chronological order;
- Context Pack and debugger.

### Level 2 — Abstract model and refinement

Learn:

- model simpler than code;
- abstraction map;
- stuttering;
- model/program drift.

### Level 3 — Liveness/fairness

Learn:

- infinite behavior;
- fair cycle;
- environment/scheduler assumptions;
- ranking/progress.

### Level 4 — Proof and certificates

Learn:

- inductive invariants;
- independently checked evidence;
- Lean theorem receipts;
- assurance envelope.

### Level 5 — Synthesis and invention

Learn:

- typed holes;
- CEGIS;
- non-vacuity;
- quality-diversity;
- unrealizability.

## `continuum init`

Initialization performs an audit and creates a draft, not fake certainty:

```text
✓ asupersync runtime detected
✓ virtual time path available
! 3 ambient entropy calls
! filesystem adapter has no crash profile
? abstract model not defined

Next: continuum boundary propose
```

The tool explains why each issue matters and links to automatic or manual actions.

## Tutorials

Each tutorial follows:

```text
predict → execute → observe → explain → repair → verify → generalize
```

Examples:

- Die Hard: reachability and shortest witness;
- Dining Philosophers: deadlock and symmetry;
- barrier: fairness and liveness;
- replicated register: durability/cancellation/refinement;
- Paxos family: abstraction and invariant proof;
- storage service: production evidence and insufficient telemetry;
- Forge guard: synthesis/non-vacuity.

## Error messages as teaching

Bad:

```text
Invariant violation at state 3920
```

Good:

```text
AckImpliesDurable failed.
A reply became externally visible while its write was only submitted, not stable.
This is possible because cancellation finalization publishes the reserved reply.
```

Formal details remain expandable.

## Documentation architecture

- task-first guides;
- concept pages;
- exact semantic reference;
- engine/assurance reference;
- examples/corpus gallery;
- troubleshooting by typed error;
- architecture and proof docs;
- agent API cookbook.

Docs are versioned with semantic epochs and generated schema references.

## Agent onboarding

Agents receive a capability/resource manifest and a small “workflow grammar” describing valid operation sequences. They should not need a giant prompt explaining terminal conventions.

## Friction budget

Every mandatory annotation/configuration must justify:

- what semantic ambiguity it resolves;
- what evidence it enables;
- whether it can be inferred and checked;
- whether it remains stable across refactors.

Boilerplate that exists solely for the tool is a design defect.
