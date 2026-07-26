# Proof-Oriented Bidirectional Transformations

## Question

Can Continuum reduce model/program drift without silently inventing incorrect synchronization?

## Background

Bidirectional transformations and lenses maintain consistency between views. Verified systems such as KBX and formally verified lens languages provide useful foundations; effectful lenses extend the theory to non-cancellable effects. Software-model synchronization work also emphasizes provenance and conflict.

References:

- https://arxiv.org/abs/2404.18771
- https://doi.org/10.1145/3747523
- https://doi.org/10.1145/2847538.2847544

## Continuum requirements

Model/program correspondence is not ordinary data synchronization:

- one abstract action may represent many concrete steps;
- concrete internal steps may stutter;
- abstraction may deliberately discard detail;
- source edits can add effects or observer leaks;
- reverse changes are underdetermined;
- proof obligations and intent constrain acceptable synchronization.

## Correspondence object

```text
get : Concrete → Abstract
put? : Concrete × AbstractEdit → Set ConcreteEdit
consistent : Concrete × Abstract → Prop
complement : provenance needed for reconstruction
obligations : generated semantic claims
ambiguity : conditions yielding multiple/no edits
effects : synchronization side effects and failures
```

## Novel proposal: proof-oriented lens law

Beyond round-trip laws, require:

```text
if put(c, edit) = c'
and edit preserves protected abstract intent
then generated obligations establish
  consistency(c', edited_abstract)
  and relevant refinement/property preservation.
```

The tool may return a candidate plus obligations instead of claiming the law automatically.

## Novel proposal: conflict as a first-class proof goal

When synchronization is ambiguous, return a compact conflict:

```text
Abstract field durable may map to:
  disk.stable
  journal.committed
  replica.quorum_acked

Distinguishing obligation:
  which event is externally authoritative under observer O?
```

Agents/humans resolve by intent or evidence, not heuristic selection.

## Change provenance

A complement stores why the current correspondence was chosen, source spans, generated code origin, and proof receipts. This supports incremental repair and review.

## Experiments

- field rename/refactor;
- action split/merge;
- internal buffering;
- new observer publication;
- asynchronous effect/cancellation;
- abstract requirement change with many implementation options;
- generated Rust skeleton round trip.

## Ambiguity metric

Plan §24.5's lenses row keys on a quantity this note must define:

```text
ambiguity rate :=
  fraction of abstract edits in the drift corpus for which
  put? yields multiple concrete candidate edits, or none
```

The denominator is a **drift corpus**: each experiment scenario above
(field rename/refactor, action split/merge, internal buffering, new
observer publication, asynchronous effect/cancellation, abstract
requirement change, generated-skeleton round trip) is instantiated as
concrete abstract-edit instances over real model/program pairs, with a
recorded ground-truth set of acceptable concrete edits. Building this
corpus is a lane deliverable; the metric is undefined — and the lane
cannot promote — without it.

Promotion and kill are keyed to this rate together with the existing
kill criteria below:

- **promotion** requires a measured ambiguity rate on the drift corpus
  low enough that candidate-plus-obligation proposals are useful on the
  majority of corpus edits (numeric target fixed at ratification per
  plan §24.5 — draft, pending lane-owner ratification; an unratified
  threshold may not survive Phase A);
- **kill** if the rate shows most real mappings are too ambiguous for
  useful proposals, or if users mistake candidate synchronization for
  verified preservation — both restated from the kill criteria below,
  the first now measurable as this rate.

## Lean program

Formalize a finite pure core:

- consistency relation;
- get/put candidate relation;
- sound conflict reporting;
- composition;
- preservation theorem parameterized by discharged obligations.

Production effects remain translation-validated.

## Kill criteria

- most real mappings are too ambiguous for useful proposals;
- complement/provenance burden exceeds manual model maintenance;
- generated obligations are as hard as writing correspondence;
- users mistake candidate synchronization for verified preservation.
