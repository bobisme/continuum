# Incremental Verification

> **Scope:** normative for the incremental lane. This document absorbs the slices of
> plan §4.5 (retention and redaction as they bear on reuse), §4.6 (epoch advance and
> cache scoping) and §4.7 (parity mismatches as engine defects) that belong to
> incremental verification — specification debt SD-09 in plan §25, now paid. The query
> and parity-audit specification is RFC 0030; the daemon's storage, purge, and epoch
> duties are [docs/35](35_CONTINUUMD_WORKBENCH_DAEMON.md); the epoch decision is
> [ADR-0018](../adr/0018-semantic-versioning-and-replay.md). Where this document and
> plan §4.5–§4.7 disagree, this document is corrected and becomes normative (plan §25);
> the corrections are listed under [Corrections](#corrections).
>
> **Normative language:** MUST/MUST NOT/SHOULD/MAY per RFC 2119.

## Thesis

Continuum should feel interactive without making cached correctness an article of faith.

## Query graph

Derived computations are pure or explicitly effectful queries over immutable artifacts. Examples:

```text
Parse(file)
Elaborate(module, imports, semantic_epoch)
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

A query key includes semantic configuration; a source hash alone is insufficient (plan §9.2).

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

## Dirty/clean dual lane and the Incremental Parity Audit

The clean/dirty comparison mechanism is the **Incremental Parity Audit** (plan §9.5). It is on by default, not opt-in.

Interactive lane:

- maximizes reuse;
- returns provisional/exact classifications quickly;
- a configurable fraction of interactive queries — default 1 in 64, selected uniformly by query key hash, per RFC 0030 — is recomputed clean and compared.

Promotion lane:

- every promotion-relevant query is recomputed clean or independently validated at promotion time;
- compares canonical outputs;
- emits receipt.

Deployments may adjust the sampling rate by policy; an opt-out is recorded in assurance envelopes (INV-010).

### Auditability classes

Queries carry an auditability class, declared on the query definition (plan §9.5):

- **Equality-auditable** — deterministic under the docs/19 matrix; compared bit-for-bit. Any disagreement quarantines the `(reuse-edge class, function_id, function_version)` triple (RFC 0030, "Quarantine") and emits a minimal invalidation counterexample.
- **Certificate-auditable** — solver-backed; the audit compares checked certificates and claim envelopes, never raw solver behavior. A certificate-level disagreement quarantines; a solver-outcome difference with agreeing certificates does not.
- **Budget-sensitive** — anytime results; the audit checks only that the incremental result's evidence labels are no stronger than a clean run's under equal budget (monotone-honesty), and records divergence as drift telemetry without quarantine.

## Epoch advance and reuse

Absorbed from plan §4.6; the decision and its rationale are [ADR-0018](../adr/0018-semantic-versioning-and-replay.md).

A query key already includes semantic configuration, so cached results are epoch-scoped by construction. What an epoch advance adds is a rule for what becomes of them, and that rule is the published per-artifact-class compatibility statement — never a heuristic and never a guess from timestamps:

| Statement | Effect on reuse |
|---|---|
| `Preserved` | Cached results of that artifact class remain reusable at their existing reuse class. |
| `Revalidate` | Results remain readable but MUST NOT be reused in the promotion lane until re-established under the new epoch. Interactive reuse MAY continue and MUST be labeled provisional. |
| `Incompatible` | Reuse MUST stop. Entries are invalidated, not migrated; re-derivation produces new identities linked by `SUPERSEDES` edges. |

The daemon holds at most two epochs of a kind during a migration, and cache entries are keyed per epoch. An entry MUST NOT be shared between the two, and a lookup MUST NOT fall back to the other epoch on a miss: a cross-epoch fallback silently reintroduces exactly the reinterpretation plan §4.6 forbids, and it would do so at the moment the system is least observed.

The blast radius an advance publishes is an estimate over artifact classes; the query graph is where it becomes exact. `query.explain_invalidation` MUST be able to name an epoch advance as an invalidation cause with the same fidelity it names a dependency edge. A migration MUST NOT appear to a developer as an unexplained cold cache.

## Redacted, summarized, and collected inputs

Absorbed from plan §4.5.

Reuse and retention pull against each other: reuse wants inputs kept, retention policy wants them gone. The resolution is that evidence may disappear, but a reuse claim may never quietly outlive the evidence it rests on.

- A cached result whose input has become `Redacted(purged, …)` or `Redacted(lost, …)` MUST NOT be reused in the **Exact** or **Validated** classes. An Exact result is worth only the ability to re-derive it, and a Validated result only the ability to recheck its witness; with the input gone, neither holds. Such an entry is demoted to Experimental or dropped, and the demotion is recorded.
- A result whose supporting campaign evidence became `Redacted(summarized, …)` after promotion MAY still be reused, because the attested coverage certificate is the evidence now. The reuse MUST cite the certificate rather than the raw per-class results it replaced, and the summarization MUST be reported as an omission (INV-007).
- Clean recomputation is the parity audit's only instrument, so garbage collection and retention policy MUST NOT collect an input the promotion lane still requires. Where an input is genuinely gone, the audit reports typed inconclusiveness (INV-008) and MUST NOT record a pass. An unrunnable comparison is not a passing comparison — this is the same fail-closed rule as the gates, applied to the audit itself.
- Cache eviction is not any of this. An evicted entry is re-derivable from committed artifacts and changes nothing (docs/35); a collected or purged *input* changes what can be established at all. A system that reports the two the same way has hidden the one that matters.

## Parity mismatches are engine defects

Absorbed from plan §4.7.

An equality-auditable disagreement quarantines the `(reuse-edge class, function_id, function_version)` triple (RFC 0030, "Quarantine") and emits a minimal invalidation counterexample. That counterexample is not a log line: the disagreement MUST also emit a `defect_*` artifact pinning every input by content identity, the semantic and checker epochs, the engine identity, and the minimized reproduction (plan §4.7, docs/35). The incremental engine is engine code, and a clean/incremental divergence is precisely the class of evidence `defect_*` exists to carry.

Two consequences:

- a quarantine MUST be visible as evidence, not merely as a degraded cache. The quarantine, the counterexample, and the defect report share the artifact identities they were derived from, so "why is this reuse class quarantined" is answerable from the evidence graph rather than from an operator's memory;
- a certificate-level disagreement quarantines and emits. A solver-outcome difference with agreeing certificates does not, and MUST NOT emit a defect report — an engine that files a defect for every benign nondeterminism teaches its readers to ignore defect reports, which is worse than filing none.

A budget-sensitive divergence records drift telemetry without quarantine and without a defect report. Telemetry stays on the operational lane, never inside semantic evidence (plan §4.7).

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
- proof-producing incremental solver formats where available;
- epoch-advance sequences: with a `Revalidate` or `Incompatible` statement in force, no promotion-lane reuse survives and no lookup crosses the two held epochs;
- retention interference: an input collected or purged out from under a promotion-lane query yields typed inconclusiveness, never a pass.

## Success metric

Not cache hit rate alone. Optimize:

```text
latency × correctness × evidence freshness × recomputation cost
```

A stale green result is infinitely worse than a slow one.

## Corrections

Per plan §25 the absorbing document is corrected and becomes normative; the direction of each correction is recorded.

| # | Disagreement | Direction | Resolution |
|---|---|---|---|
| 1 | This document's auditability classes said an equality-auditable disagreement "quarantines the reuse class and emits a minimal invalidation counterexample" and stopped there; plan §4.7 requires a `defect_*` artifact for every parity mismatch. | plan §4.7 → docs/42 (widened) | A parity mismatch emits both. The counterexample is the minimized reproduction the `defect_*` carries; the defect artifact is what makes the mismatch checkable by someone other than the daemon that found it. |
| 2 | Plan §4.5 makes raw campaign results garbage-collectable after promotion, while this document requires every promotion-relevant query to be recomputed clean or independently validated at promotion time. Read together they permit collecting the very evidence the promotion audit needs. | plan §4.5 → docs/42 (ordered) | Retention MUST NOT collect an input the promotion lane still requires; summarization replaces raw results only once the attested coverage certificate is committed. Where an input is gone anyway, the audit reports typed inconclusiveness and MUST NOT record a pass. |
