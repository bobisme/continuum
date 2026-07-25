# Incremental Verification and Proof Reuse

## Question

How can Continuum provide interactive feedback while preventing unsound cache reuse from becoming a false proof?

## Relevant foundations

- self-adjusting computation and demand-driven incremental queries;
- Salsa-style red/green query evaluation;
- incremental SAT/SMT and proof certification;
- incremental model checking;
- differential dataflow;
- proof dependency graphs and proof retrieval.

Selected references:

- https://salsa-rs.github.io/salsa/
- https://doi.org/10.29007/pdcc
- https://easychair.org/publications/paper/TbPs
- https://arxiv.org/abs/2401.13244

## Core difficulty

A source edit can be local while reachability impact is global. Conversely, a comment or proof presentation change may be semantically irrelevant. File timestamps and module dependency alone are too coarse.

## Semantic dependency graph

Edges carry a reason:

```text
READS_TYPE
READS_VALUE
UNFOLDS
SELECTS_PROFILE
OBSERVES_EVENT
USES_ASSUMPTION
USES_FAIRNESS
USES_BOUND
USES_ABSTRACTION
USES_LEMMA
USES_ENCODING_EPOCH
```

Only Complete/Conservative edge classes may drive trusted invalidation. Experimental dynamic provenance can accelerate previews but must not certify final results.

## Novel proposal: proof-carrying cache entries

A cache entry includes one of:

- exact content-function identity;
- translation-validation witness;
- certificate proving result for input digest;
- conservative dependency theorem instance;
- clean-comparison receipt.

Cache lookup returns both value and reuse evidence. Assurance of the consuming result is the meet/composition of dependency assurance.

## Novel proposal: invalidation counterexamples

When incremental and clean results differ, minimize the edit/query graph to produce:

```text
smallest edit sequence
smallest missing/incorrect dependency edge
first divergent query
semantic result difference
```

This treats the build/verifier engine as another system Continuum can debug.

## Novel proposal: change-action algebra

Model edits as typed actions with commutation and impact composition. Independent edits may reuse proofs/caches in either order. This connects incremental computation to true-concurrency semantics and may permit partial-order reduction over edit/test campaigns.

## Proof program

Lean model:

- query graph with declared dependencies;
- valid closure condition;
- deterministic query functions;
- theorem: unchanged transitive inputs imply reusable output;
- theorem: conservative over-approximation may over-invalidate but not under-invalidate.

The production engine remains more complex; the theorem defines the target contract.

## Experiments

- property edit;
- action guard edit;
- abstraction-map edit;
- domain-profile edit;
- proof lemma edit;
- source refactor with semantic equivalence;
- concurrent identical tasks;
- crash during publication;
- intentionally missing edge.

## Metrics

latency, recomputation, cache size, clean mismatch, evidence freshness, debugging time, promotion overhead.

## Kill criteria

- dependency capture cannot be made trustworthy below file/module granularity;
- promotion requires nearly full recomputation on every change;
- certificate/provenance overhead dominates useful savings;
- nondeterministic engines prevent stable reuse identities.
