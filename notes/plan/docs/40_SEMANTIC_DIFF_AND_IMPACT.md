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

`incomparable` on a protected field has the same blocking status as a confirmed privileged change (see Soundness policy).

## Property normalization

Property, assumption, and fairness-condition expressions are structured ASTs, not strings, and every classification above is computed over their canonical normal form (CPNF-1, specified in RFC 0037; carried by `schemas/intent-contract.schema.json`). This is what separates refactoring from weakening:

- equal CPNF-1 encodings ⇒ `unchanged`, and that is the only admissible ground for `unchanged`. Renaming a bound variable, reordering conjuncts, expanding a `leads_to`, or rewriting through De Morgan all reach the same encoding, so none of them can masquerade as a semantic edit;
- unequal encodings ⇒ no direction yet. CPNF-1 is sound but not complete, so a difference in normal form is a question, not an answer: the diff must discharge the implication obligation in the declared fragment or emit `unknown` per the Soundness policy below;
- the display-only `source` rendering of an expression never contributes to identity or classification.

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

A claim such as “property strengthened” requires implication evidence in the supported fragment. Otherwise output `unknown` (undecidable or unattempted), `unsupported` (outside declared fragments), or `incomparable` — `unknown` and `unsupported` are distinct classifications per INV-008.

All non-affirmative classifications fail closed: any intent change classified `unknown`, `unsupported`, or `incomparable` on a protected field blocks ordinary promotion exactly as a confirmed privileged change does, pending review (plan §5.3, RFC 0031). Text heuristics may prioritize review but cannot authorize promotion.

## Use in Forge

Forge candidates are compared semantically to maintain a behaviorally diverse archive and detect duplicate implementations. Syntactic novelty alone is not counted.
