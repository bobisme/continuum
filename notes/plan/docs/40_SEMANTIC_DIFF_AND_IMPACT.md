# Semantic Diff and Impact Analysis

## Objective

Determine what a change means, what evidence it invalidates, and whether it changes protected intent.

## Diff layers

```text
Text diff
Syntax/AST diff
Type/effect diff
Intent diff
Transition-system diff
Observer/property diff
Correspondence/refinement diff
Proof dependency diff
Behavior/evidence diff
Cost/performance diff
```

A change may be textually large and semantically null, or one token may weaken a critical property.

## Intent diff categories

- property strengthened/weakened/incomparable;
- assumption strengthened/weakened/incomparable;
- observer refined/coarsened/incomparable;
- bounds expanded/contracted;
- fault envelope expanded/contracted;
- fairness added/removed/changed;
- trust boundary expanded/contracted;
- assurance upgraded/downgraded;
- optimization/non-vacuity changed;
- no protected change;
- unknown, proof required.

## Model diff

Compare elaborated relations, not source text alone:

- states/variables/types;
- initial relation;
- action guards/updates/frames;
- stuttering policy;
- temporal formulae;
- symmetry declarations;
- constraints/views;
- refinement edges.

For finite configurations, exact relational inclusion/equivalence may be checked. For symbolic/infinite fragments, generate solver or Lean obligations.

## Program semantic diff

Use extraction and semantic journal metadata to identify:

- new/removed effects;
- changed atomicity boundaries;
- changed resource footprints;
- cancellation checkpoints/finalizers;
- durability phase changes;
- task/region ownership changes;
- observer publication changes;
- opaque/FFI boundary changes.

## Behavioral diff

Within an assurance envelope, compute:

- newly reachable states/configurations;
- removed behavior;
- changed counterexample classes;
- changed liveness SCCs;
- refinement coverage changes;
- trace-language inclusion samples/proofs;
- performance/cost distribution changes.

## Impact graph

Evidence edges classify how changes propagate:

```text
source value read
source type read
definition unfolded
model action used
observer event consumed
assumption/fairness consumed
proof lemma used
certificate checker epoch used
engine encoding used
```

Impact output distinguishes definitely invalidated from conservatively rebuilt.

## Review presentation

```text
Protected intent
  ✓ properties unchanged
  ✓ assumptions unchanged
  ! bound N: 5 → 3 (contraction; blocked)

Program semantics
  reply publication moved after storage sync
  cancellation finalizer no longer publishes reserved reply

Behavior
  failing causal class removed
  no new bounded violations
  availability traces preserved under declared fair profile

Evidence
  2 certificates invalidated/rebuilt
  1 Lean theorem reused
  4 benchmark tasks affected
```

## Soundness policy

A claim such as “property strengthened” requires implication evidence in the supported fragment. Otherwise output `Incomparable` or `Unknown`. Text heuristics may prioritize review but cannot authorize promotion.

## Use in Forge

Forge candidates are compared semantically to maintain a behaviorally diverse archive and detect duplicate implementations. Syntactic novelty alone is not counted.
