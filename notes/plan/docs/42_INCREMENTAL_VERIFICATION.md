# Incremental Verification

## Thesis

Continuum should feel interactive without making cached correctness an article of faith.

## Query graph

Derived computations are pure or explicitly effectful queries over immutable artifacts. Examples:

```text
Parse(file)
Elaborate(module, imports)
InferFragment(model)
ExtractRust(crate, annotations)
BuildCorrespondence(model, program)
CompileProperty(property, observer)
Explore(model, config, strategy)
CheckRefinement(concrete, abstract, map)
CheckCertificate(certificate)
CompileContext(evidence, question, budget)
SemanticDiff(before, after, intent)
```

## Granularity

Use declaration/action/property/effect granularity rather than file granularity where provenance is sound. Do not chase maximal granularity before semantic dependencies are trustworthy.

## Reuse classes

### Exact

Content-addressed pure result; no validation beyond decoder/integrity.

### Validated

Result includes a witness checked independently, such as translation validation or certificate.

### Conservative

Dependency tracking may over-invalidate but is proven/argued not to under-invalidate for the supported fragment.

### Experimental

Useful for provisional UI/search. Cannot support final promotion until clean comparison.

## Incremental exploration

Potential reuse:

- unchanged canonical states and successors;
- frontier partitions;
- property monitor results;
- symbolic clauses/lemmas;
- DPOR independence and backtracking metadata;
- proof summaries;
- context slices.

Danger: a small transition change can invalidate reachability globally. Reuse must be guarded by semantic change classification, not source locality.

## Incremental SAT/SMT/PDR

Track assumption frames, formula identities, proof artifacts, and solver epoch. Incremental solving that cannot emit/check suitable evidence remains provisional for strong claims.

## Proof reuse

Lean declaration hashes and dependency closure allow exact theorem reuse. Changed elaboration environment invalidates even text-identical proof terms where semantics differ.

## Dirty/clean dual lane

Interactive lane:

- maximizes reuse;
- returns provisional/exact classifications quickly;
- can schedule background clean validation only when explicitly requested by client policy (not hidden future work).

Promotion lane:

- executes required clean or independently validated computations now;
- compares canonical outputs;
- emits receipt.

## Invalidation debugging

Developers can query:

```text
why was this result reused?
why was this query invalidated?
which edge caused the rebuild?
what clean comparisons support this reuse class?
```

The query graph itself is inspectable evidence.

## Verification of the incremental engine

- property-based edit sequences;
- differential clean builds;
- intentionally corrupted edges;
- crash/recovery/cancellation tests;
- content-hash collision injection;
- cross-thread deterministic scheduling;
- Lean theorem for a simplified dependency-closure model;
- proof-producing incremental solver formats where available.

## Success metric

Not cache hit rate alone. Optimize:

```text
latency × correctness × evidence freshness × recomputation cost
```

A stale green result is infinitely worse than a slow one.
