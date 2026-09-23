# Incremental Verification and Proof Reuse

**Claim class:** research and implementation program

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

## Ratified sub-file incremental-trust threshold (plan §24.5, FR-06)

This note owns the plan §24.5 row *Sub-file incremental trust (§9)*. The
row names the metric and its source: the clean-mismatch rate at the §9.5
sampled audit rate, where the rate derives from §8.6's confidence
target. This section fixes the number, the denominator, and the rate.
Plan §24.5 quotes the sentence verbatim under the same `quote-id`.

quote-id=subfile-incremental-trust-mismatch-rate "Sub-file dependency capture is trusted for a reuse-edge class other than Experimental only if, in the interactive lane of the Incremental Parity Audit on the G5 reference workload, the class shows zero clean mismatches over all of its sampled comparisons that complete with a typed outcome and at least 4,603 of those comparisons are sub-file reuses, meaning reuses that module-granularity invalidation would have recomputed, which bounds the clean-mismatch rate per sub-file reuse below 1 in 1,000 at one-sided 99 percent confidence by the exact zero-failure binomial bound; a clean mismatch is any equality-auditable disagreement, any certificate-level disagreement of a certificate-auditable query, or any budget-sensitive incremental result whose evidence labels are stronger than the clean run's under equal budget; the audit sampling rate is one rate for every query, selected uniformly by query key hash, and is the larger of the 1-in-64 bootstrap default and 4,603 divided by the smallest expected count of sub-file reuses of any of the three classes in the evaluation window; promotion-lane comparisons and typed-inconclusive comparisons do not count toward the 4,603; a class that cannot reach 4,603 even at a rate of 1 in 1 stays on module-granularity invalidation; and one clean mismatch fails the class, quarantines the implicated RFC 0030 triple, and returns the class to module-granularity invalidation until it again reaches 4,603 sub-file reuses with zero clean mismatches, all counted after the fix."

**Status of the numbers: lead-ratified proposal.** The plan fixes the
form of the target but no number for it. The sentences it rests on are:

- plan §8.6: "The §9.5 sampling rate derives from a declared statistical
  confidence target for mismatch detection per reuse class, reviewed at
  G5."
- plan §22 G5: "incremental results continuously match clean builds
  under the Incremental Parity Audit".
- plan §9.5: "divergence in an equality- or certificate-auditable query
  always does" quarantine.
- RFC 0030 "Sampling policy": "1 in 64, uniformly by query key hash" is
  "the labeled bootstrap default and nothing more", and selection "MUST
  NOT be influenced by reuse class".

No document in the dossier declares the confidence level or the
tolerated rate. The two numbers below are therefore chosen here, as the
most conservative values consistent with that text. G5 review may make
them stricter. A change that makes them weaker is an intent-class
revision of this register row, not a tuning.

- **Observed count: zero.** G5 says "continuously match", and §9.5
  quarantines on every equality- or certificate-auditable divergence.
  Any tolerated count above zero contradicts both sentences. So the
  observed threshold is zero clean mismatches, not a small rate.
- **Confidence: one-sided 99 percent.** Other ratified rows of the
  register use one-sided 95 percent (research/33, research/25). This row
  uses 99 percent, because RFC 0030 names under-invalidation "the only
  unrecoverable error": a stale green result is a release blocker.
- **Tolerated rate: 1 in 1,000 sub-file reuses (p\* = 0.001).** This
  is the judgment call with the least textual support. It is the bound
  that the evidence must *demonstrate*, not a rate of mismatches that is
  permitted: the observed count stays zero.

**Derivation.** With zero mismatches in n independent uniform samples,
the exact one-sided upper confidence bound on the mismatch rate at
confidence 1 − α is 1 − α^(1/n). The bound is below p\* when
(1 − p\*)^n ≤ α, that is, when n ≥ ln α / ln(1 − p\*).

- α = 0.01, p\* = 0.001: ln 0.01 / ln 0.999 = −4.60517 / −0.00100050 =
  4602.87, so n = 4,603.
- Check: 0.999^4603 = 0.0099987 ≤ 0.01, and 0.999^4602 = 0.0100087 >
  0.01. So 4,603 is the smallest n that clears the bound.
- The bound at n = 4,603 is 1 − 0.01^(1/4603) = 0.00099997 < 0.001.

The same number is the detection form that §8.6 asks for. At sampling
rate r, a defect that corrupts a fraction p\* of sub-file reuses escapes
N reuses with probability (1 − r·p\*)^N. At r = 1/64 and
N = 64 × 4,603 = 294,592, that is exp(−4.603) ≈ 0.010. So the audit
detects such a defect with probability at least 99 percent within one
evaluation window.

**Sampling rate.** The rate is the derived quantity, not a free setting.
If E_c is the expected number of sub-file reuses of class c in the G5
evaluation window, the rate is r = max(1/64, max over c of 4,603 / E_c),
capped at 1. One rate applies to every query, because RFC 0030 forbids
selection that depends on reuse class. At the bootstrap rate of 1 in 64,
a class needs at least 294,592 sub-file reuses in the window. If E_c is
below 4,603, no rate can reach the count, and that class stays on the
fallback. A rate below the derived r is a reduced rate under RFC 0030,
and must be recorded in the assurance envelope of every claim it
touches. A reduced rate never counts toward this threshold.

**Denominator.** The unit is the *sub-file reuse*: a sampled interactive
comparison where the incremental lane reused a result that
module-granularity invalidation would have recomputed. Only these
comparisons test the claim of this lane. A reuse that module granularity
also allows says nothing about capture below module granularity.

- The classifier of a comparison as a sub-file reuse depends on the
  reuse decision only. It never depends on the audit outcome. Selection
  stays uniform by key hash, so the filter does not bias the sample.
- The numerator is stricter than the denominator: a clean mismatch in
  *any* sampled comparison of the class fails the class, sub-file or
  not.
- Promotion-lane comparisons are excluded, because every
  promotion-relevant query is recomputed and is not a uniform sample.
  CI full-clean sweeps are excluded for the same reason.
- Typed-inconclusive comparisons (docs/42: "An unrunnable comparison is
  not a passing comparison") are neither passes nor failures. They do
  not count toward 4,603, and their count is reported.
- Budget-sensitive divergence that is attributable only to budget or
  portfolio nondeterminism is not a mismatch (plan §9.5). A
  monotone-honesty violation is a mismatch.
- The Experimental class is out of scope: it cannot drive trusted
  invalidation (RFC 0030 correction 10).

**Quarantine and the class.** RFC 0030 quarantines the triple
`(reuse-edge class, function_id, function_version)`, and this threshold
does not change that rule. The lane adds a stricter consequence at class
scope: after one mismatch, the class loses its sub-file trust claim and
restarts its count from zero after the fix. That consequence changes
granularity only. It does not drop the other definitions of the class to
`Experimental`, so it is not the class-wide quarantine that RFC 0030
rejects.

**Kill and fallback.** The kill stays as written above: dependency
capture cannot be made trustworthy below file/module granularity. The
fallback is module-granularity invalidation, which RFC 0030 requires to
remain a supported configuration. A class that fails the threshold is on
the fallback until it passes again. This threshold adds no condition
that the kill must meet before it fires.

**Scope and dependency.** The query engine, the G5 reference workload,
and the interactive audit lane do not exist today. The threshold binds
when they land, before Phase C opens its G5 evaluation. RFC 0030 states
that it does not ratify this row, and that stays true: this note
ratifies it. Two RFC 0030 sentences call the row draft ("Frontier
dependency" and "Sampling policy"). They are stale after this
ratification, and a revision of RFC 0030 must cite this section. Until
that revision, this section and the plan §24.5 quote decide the number.
