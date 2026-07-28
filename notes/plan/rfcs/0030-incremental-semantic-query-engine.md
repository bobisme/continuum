# RFC 0030: Incremental Semantic Query Engine

## Status
Draft for implementation.

**Target gate:** G5 (incremental trust)
**Owners:** incremental/database leads
**Normative language:** MUST/SHOULD/MAY per RFC 2119.

## Summary

All derived artifacts are memoized queries over immutable inputs (plan §9). Reuse must be fast for interactive work and auditable for trust: every reuse edge is classed, and the Incremental Parity Audit (renamed from "clean-build Tribunal"; plan §9.5) continuously compares incremental against clean recomputation (INV-010, B9).

## Query key

`(function_id, function_version, canonical_input_identities, semantic_epoch, proof_epoch, strategy_config)`.

- A source hash alone is never a sufficient key (plan §9.2).
- **Budget rule:** budget belongs to execution identity, not output identity, for any query whose result is budget-independent when it completes (`parse`, `elaborate`, `property_automaton`, diffs). For budget-truncated searches (`explore`, proof search, synthesis), the committed artifact records the budget actually consumed and its coverage frontier; two runs of the same key with different budgets produce *comparable, monotone* artifacts related by frontier inclusion, and a continuation — never two conflicting "results" under one identity (INV-009, B18).

## Edge classes

| Class | Meaning | Reuse rule |
|---|---|---|
| `Exact` | output is a pure function of named inputs | reuse by content identity |
| `Validated` | reuse carries a checker witness (e.g., translation validation) | reuse after witness check |
| `Conservative` | invalidation may over-approximate but MUST NOT miss changes under stated assumptions | reuse; audited |
| `Experimental` | speed-up only | result cannot support strong finality until clean validation |

## Semantic dependency reasons

Every recorded dependency carries one of the nine typed reasons (plan §9.4): `reads-type`, `reads-value`, `unfolds-definition`, `selects-instance-or-profile`, `observes-event-family`, `relies-on-assumption-fairness-bound`, `uses-abstraction-component`, `depends-on-lemma-or-checker`, `consumes-encoding-epoch-or-correspondence`. These reasons drive precise invalidation and power the explain API.

Independence between an edit and a query is a tri-state (`DefinitelyIndependent(witness)`, `DefinitelyDependent(reason)`, `Unknown`), and **`Unknown` is dependent** (RFC 0004's rule generalized). Heuristic independence is permitted only on `Experimental` edges.

## Auditability classes

Absorbed from plan §9.5 (SD-03). Every query definition declares exactly one auditability class. The class lives on the query definition, never on the run: it MUST NOT be reclassified per-run, so a mismatch cannot be reclassified away after the fact.

| Class | Meaning | Audit comparison |
|---|---|---|
| `equality-auditable` | deterministic under the docs/19 matrix | compared bit-for-bit |
| `certificate-auditable` | solver-backed | checked certificates and claim envelopes are compared — never raw solver behavior |
| `budget-sensitive` | anytime results | monotone-honesty only: the incremental result's evidence labels MUST be no stronger than a clean run's under equal budget |

The quarantine rule is scoped by class: divergence in an equality- or certificate-auditable query always quarantines the reuse class; divergence attributable solely to budget or portfolio nondeterminism never does — it is recorded as drift telemetry, not quarantine. A solver-outcome difference with agreeing certificates does not quarantine. INV-010's exactness claim applies per auditability class.

## Persistence and crash safety

CAS stores outputs; the query index maps keys to content. Transactions MUST commit output before index; a crash leaves unreachable content eligible for GC, never a stale index entry (this is the operational content of INV-017; see plan §4.5). The index format is versioned; an index verifier (fsck) ships with the daemon and runs on recovery.

## Incremental Parity Audit

- **Sampling policy:** a configurable fraction of interactive queries plus every promotion-relevant query at promotion time is recomputed clean; CI additionally runs deterministic full-clean sweeps nightly. The sampling rate MUST derive from a declared statistical confidence target for mismatch detection per reuse class (plan §8.6), reviewed at G5; "1 in 64, uniformly by key hash" remains only as the labeled bootstrap default until that derivation ships.
- **Compared artifacts:** verdict, canonical state-graph digest, counterexample class, certificate result, context-slice soundness, semantic diff, proof axiom manifest (plan §9.5).
- **On mismatch:** publish a mismatch evidence node; apply the class-scoped quarantine rule of "Auditability classes" above — an equality- or certificate-auditable mismatch quarantines the implicated edge class + query implementation version (its reuse drops to `Experimental` until cleared), while divergence attributable solely to budget or portfolio nondeterminism is recorded as drift telemetry without quarantine; run the invalidation minimizer to produce the smallest input delta reproducing the divergence; surface per the release-blocker doctrine — a stale green result is a blocker, not a bug ticket (docs/42).
- The audit's own overhead is measured; the Phase C exit budget for it is part of the docs/34 latency accounting.

## Explain API

`query.explain_reuse(key)` — why this result was reused (edges, classes, witnesses). `query.explain_invalidation(key, edit)` — why this re-ran (the dependency reasons hit). `query.clean_compare(key)` — run and diff a clean recomputation now. All three are ordinary read operations (plan §10.2).

## Formal model

A simplified Lean model (per ADR-0044) MUST state and check: exact-reuse soundness (equal keys ⇒ equal results) and conservative-closure soundness (the computed invalidation cone contains the true dependency cone) for the finite model of the query graph. This is plan §15.3 theorem 5's home; the existing seed (`lean/Continuum/Interaction/Incremental.lean`) states the shape but not the computed-cone side and MUST be extended.

## Rejected alternatives

- **File-granularity-only invalidation.** Retained as the *fallback*, not the design: research/27's kill criterion drops sub-file granularity if capture proves untrustworthy, and the fallback MUST remain a supported configuration.
- **Trusting engine-reported dependencies without audit.** Rejected: B9 — incremental reuse is dangerous exactly when invalidation is silently unsound.

## Open questions

- Cache eviction/retention interaction with the GC policy of plan §4.5.
- Stable reuse identities for nondeterministic engines (research/27 kill criterion) — candidate answer: canonicalize by committed frontier, not by execution.

## Acceptance

Random edit traces across model/property/correspondence/proof/context queries with clean parity; intentional dependency bugs (broken edges) are caught and quarantined by the audit (PR 24's exit); property-only edits do not rebuild extraction; model-action edits invalidate reachable graphs and dependent packs (PR 23's exit); crash-mid-publication leaves no stale index entries under fault injection.
