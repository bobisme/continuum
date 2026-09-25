# ADR-0055: Build the incremental engine; do not adopt salsa

**Status:** Accepted with the reversal criteria below  
**Date:** 2026-09-24  
**Decision owners:** Continuum lead (bn-13sa and bn-3gf6 dispatch); drafted by `continuum-dev`  
**Implements:** plan §9.1 and §21 Phase B ("an ADR resolving build-vs-adopt for the §9 incremental engine"), docs/08 R21, START_HERE PR 22a (PR-22A-IMPL-01 and PR-22A-IMPL-03)  
**Evidence:** `pr22a-impl02-reuse-spike` (bn-31vf) and `pr22a-impl03-precision-baseline` (bn-3gf6), both in `crates/continuum-incremental/tests/golden/`

## Context

Plan §9.1 names the incremental engine as engineering risk, not settled design. A Phase B
ADR decides whether the engine is derived from an existing memoization framework (salsa)
or built custom. Phase C is `BLOCKED` until this ADR exists (plan §21). PR 23 may not
merge before it (START_HERE PR 22a). docs/08 R21 carries the risk. Its failure mode is
that the substrate cannot support the four reuse-edge classes with precise invalidation,
so edge classes collapse to `Conservative`. Its kill signal is "Conservative-collapse on
the reference workload".

RFC 0030 is the normative specification. Its open questions include this decision, and
they state the risk directly: "whether an adopted substrate can express the four classes
and the nine reasons without a shim that becomes the real engine".

Two artifacts give the evidence:

- **The spike (bn-31vf, PR-22A-IMPL-02).** `crates/continuum-incremental` is a custom
  content-addressed engine. It memoizes parse, elaborate, build_model, two projections,
  explore, domain_safety and check_invariant. It has no memoization framework. Its
  cache is a `BTreeMap` from canonical key to entry, and one decision function turns a
  cache hit into a typed outcome. Every reuse is licensed by one RFC 0030 class. An
  independent clean recomputation (`audit::clean`) audits it and shares no decision
  logic with it (INV-010). The security reviews cr-1jv75r and cr-174gtd found
  provenance laundering, definedness gaps, audit-basis forgery and charge-after-work
  defects, and all were fixed.
- **The baseline (bn-3gf6, PR-22A-IMPL-03).** `tests/precision_baseline.rs` measures the
  precision and recall of the spike's invalidation against the clean recomputation, per
  lane and per reuse class. It uses two corpus models and compares the result with
  module-granularity invalidation. The next section records the numbers.

## Measured invalidation-precision baseline

Artifact `pr22a-impl03-precision-baseline`,
`crates/continuum-incremental/tests/golden/pr22a_impl03_precision_baseline.json`, pinned
byte for byte by `the_retained_precision_baseline_is_current_and_sound`.

**Definitions.** The unit is one compared query label (parse, elaborate, explore,
domain_safety, check_invariant) of one audited revision `before → after`. A label is
*changed* when its clean output for `before` is absent or differs from its clean output
for `after`.

- **TP**: recomputed and changed. **FP** (over-invalidation): recomputed, but the clean
  output is present before and equal after.
- **FN** (under-invalidation): reused, and the reused output differs from the clean
  output; or **only in clean**, a label the clean run of the edited source defines
  and the engine did not produce, because a stale reuse upstream stopped the
  pipeline. An only-in-clean FN is attributed to the licence the audit blames for the
  first divergence. **TN**: reused and equal to the clean output.
- **Precision** = TP / (TP + FP). **Recall** = TP / (TP + FN).
- **Module granularity** recomputes every compared label of the edited revision,
  because each corpus model is one file. Its FN is zero by construction.
- **Only in engine**: a label the engine produced that the clean run of the edited
  source does not define. It is a phantom artifact, counted apart and outside TP, FP,
  FN, TN and recall.

The internal queries (build_model and the projections) have no clean counterpart. They
are counted in the artifact and not judged. Recomputed labels are judged as well: a
recompute whose output differs from the clean output is counted and asserted zero. The
test also asserts that its FP and FN sets equal those of `audit::cone`. That check uses
the same clean data, so it checks the test's bookkeeping, not the engine.

**Corpora.**

- **Die Hard**: the TV-009 model, with the 17 spike edits (whitespace, comment, rename,
  property, semantic, two heuristic-trap edits, and one parse error).
- **Finite durable register**: a configuration-free concretization of
  `examples/durable_register.ctm`, with 15 edits of the same classes. It has three
  replicas, one epoch, two values, and majority quorums. Its semantic edits include a
  one-replica quorum, a crash that loses durable bytes, and a replica whose crash
  guard can never hold. The examples' register models cannot be used directly
  (residual 1).

In edits mode, only one heuristic-trap edit per corpus defeats the heuristic
(`trap-join-lines`, through a parse error). The other one is a correct `Experimental`
reuse. In trace mode, `trap-split-expression` follows `trap-join-lines`, and there it
defeats the heuristic too (see the trace-mode interactive lane below).

**Edits mode, promotion lane.** For each edit, a warm engine revises from the base once.
This lane admits no `Experimental` reuse, and it is the baseline of record.

| Corpus | Judged | TP | FP | FN | TN | Precision | Recall | Module precision | Recomputed, spike / module | States explored, spike / clean |
|---|---|---|---|---|---|---|---|---|---|---|
| Die Hard | 92 | 31 | 15 | 0 | 46 | 31/46 (673‰) | 31/31 | 32/92 (347‰) | 46 / 92 | 66 / 226 |
| Finite durable register | 93 | 31 | 17 | 0 | 45 | 31/48 (645‰) | 31/31 | 32/93 (344‰) | 48 / 93 | 1,378 / 3,362 |
| Combined | 185 | 62 | 32 | 0 | 91 | 62/94 (659‰) | 62/62 | 64/185 (345‰) | 94 / 185 | 1,444 / 3,588 |

**Per reuse class, promotion lane, combined.** A reuse is licensed by one class. Each
correct reuse here meets the research/27 definition of a sub-file reuse, because module
granularity would have recomputed it. These are promotion-lane comparisons on a spike
corpus, so none counts toward the research/27 threshold of 4,603.

| Licence | Served | Correct | Stale |
|---|---|---|---|
| `Exact` | 53 | 53 | 0 |
| `Validated` | 6 | 6 | 0 |
| `Conservative` | 32 | 32 | 0 |
| `Experimental` | 0 | 0 | 0 |

**Where the over-invalidation is (combined promotion, 32 FP).** By function:
check_invariant 16, parse 9, explore 3, domain_safety 3, elaborate 1. By cause: 29 are
key misses, where a keyed input identity changed and the output did not. 3 are refused
`Validated` witnesses on domain_safety, where the verdict did not change but the
exploration did. The key misses break down as follows:

- parse, 9: parse runs on every byte change, because an engine must parse to know;
- check_invariant through the whole transition-system projection, 10: the check keys
  that projection (assumption A2's `reads-type` edge), so an action rename or a domain
  widening recomputes every invariant check although the exploration output is
  byte-equal;
- check_invariant after a changed exploration, 5, and after a changed invariant
  projection, 1: the verdict did not change although an input output did;
- explore after a changed transition-system projection, 3, and elaborate after a
  changed parse, 1.

**Interactive lane (edits mode).** This lane admits the spike's `Experimental` textual
heuristic.

| Corpus | TP | FP | Stale strong or conservative | Experimental misses | Only in engine | TN | Precision | Recall |
|---|---|---|---|---|---|---|---|---|
| Die Hard | 30 | 10 | 0 | 1 | 5 | 51 | 30/40 (750‰) | 30/31 (967‰) |
| Finite durable register | 30 | 13 | 0 | 1 | 6 | 49 | 30/43 (697‰) | 30/31 (967‰) |

The heuristic removes 9 parse recomputes over 32 edits. In exchange, 2 heuristic parse
reuses were stale. In each case the edited source does not parse, and the stale parse
let the engine produce 11 phantom artifacts that the clean run does not define. No clean
output was missing in edits mode. In both corpora the audit caught the mismatch,
quarantined `(Experimental, parse, 1)`, and the re-run restored parity.

**Trace mode.** One engine per lane walks every edit in sequence, then returns to the
base (18 and 16 counted revisions). The per-step case lines are in the artifact.

| Corpus, lane | TP | FP | FN, of which only in clean | TN | Precision | Recall | Module precision |
|---|---|---|---|---|---|---|---|
| Die Hard, promotion | 43 | 10 | 0, 0 | 45 | 43/53 (811‰) | 43/43 (1000‰) | 49/98 (500‰) |
| Die Hard, interactive | 37 | 6 | 6, 5 | 49 | 37/43 (860‰) | 37/43 (860‰) | 49/98 (500‰) |
| Finite durable register, promotion | 43 | 11 | 0, 0 | 46 | 43/54 (796‰) | 43/43 (1000‰) | 52/100 (520‰) |
| Finite durable register, interactive | 37 | 8 | 7, 6 | 48 | 37/45 (822‰) | 37/44 (840‰) | 52/100 (520‰) |

In the interactive traces, the heuristic reuses the parse of `trap-join-lines`, which is
a parse error, for `trap-split-expression`, which parses. So the engine stops after
parse, and it does not produce the 5 (Die Hard) and 6 (register) downstream outputs
that the clean run has. With the stale parse, these are 6 and 7 `Experimental` misses,
all attributed to `(Experimental, parse, 1)`. The audit caught the mismatch,
quarantined that triple, and the re-run restored parity.

**FN by lane and mode.** FN is zero in every promotion-lane measurement. In the
interactive lane, every FN is an `Experimental` miss, and no FN goes through an `Exact`,
`Validated` or `Conservative` derivation: edits mode has 1 per corpus, and trace mode
has 6 (Die Hard) and 7 (register).

**Across every corpus, mode and lane,** the test asserts these facts:

- every revision holds parity, except an interactive-lane mismatch whose first
  divergence is blamed on one reuse, and that quarantine re-run holds parity;
- under-invalidation through an `Exact`, `Validated` or `Conservative` derivation or
  blame is zero, only-in-clean outputs included, and no recompute differs from the
  clean output;
- every clean label is judged or counted as only in clean;
- no audit is inconclusive;
- the promotion lane serves no `Experimental` reuse, produces no phantom artifact,
  misses no clean output, and quarantines nothing;
- each corpus exercises correct `Exact`, `Validated` and `Conservative` reuse.

## What each substrate provides against RFC 0030

This table compares the spike, measured, with salsa's documented design. Salsa's design
is: inputs set at a new revision; memoized tracked functions whose dependencies are
recorded automatically as they run; red-green validation of a memo against its
dependencies' change revisions; backdating, so a re-executed query whose value is equal
keeps its old change revision and its dependents validate without running (early
cutoff); durability levels that skip validation; cancellation of in-flight queries by
unwinding; cycle handling; interning; and LRU eviction. No salsa code was built or
added here. A salsa dependency needs a human-recorded audit
(`tools/governance/dependency-audits.toml`), so this comparison rests on the design and
not on a salsa spike. Reversal criterion 4 names the salsa spike that would test it.

| RFC 0030 requirement | Custom spike (measured) | Salsa (documented design) |
|---|---|---|
| `Exact` reuse by content identity, with early cutoff | Yes. The key names each input's output identity, so an equal output is a key hit downstream. 53 correct `Exact` reuses. | Yes. This is salsa's core: red-green validation plus backdating. |
| `Validated` reuse on a witness an independent checker accepts for this input digest | Yes. A finite-closure certificate is checked from wire form by `continuum-certificate` (INV-004) and bound to the engine's model. 6 correct reuses. | Not native. A changed input re-executes the query. The query body must find the prior result and check the witness itself, so the licence logic is user code. |
| `Conservative` reuse across a changed side input under a recorded assumption | Yes. Side edges carry assumptions A1 and A2 on the reuse. 32 correct reuses, 0 stale. | Not expressible. Every tracked read is a dependency, and a changed dependency re-executes. Reuse across it needs an untracked read, which gives up salsa's consistency guarantee, or a restructured decomposition that makes the edge `Exact` and loses the assumption record. |
| `Experimental` reuse, interactive lane only, never promoted | Yes. It is lane-gated and audited. | Not expressible without an untracked side cache. |
| A licence per edge; the meet rule; provenance of every licence triple on a derivation | Yes. `Record::class` is the meet, and `Record::provenance` holds the triples. | No typed edges. A memo stores a value, so provenance must be carried inside the value. |
| Quarantine per `(class, function_id, function_version)`, checked against every producing licence | Yes. Quarantine evicts by provenance, and promotion refuses `Experimental`-origin entries. | None. It would be modelled as inputs that every licence decision reads. |
| `explain_invalidation`: invalidated, unknown, and reused, with a typed causal chain | Yes, one chain per invalidation (residual 7). | Execution events and `tracing` spans tell what ran. Dependency edges carry no reason or class, so the chain needs our own registry in either case. |
| Independent parity audit sharing no decision logic (INV-010) | Yes. `audit.rs` imports only the read-only records, and `tests/independence.rs` pins that. | Neutral. The audit is outside the substrate in either design. |
| Charge before work; a budget-dependent result is returned but not memoized | Yes. Every charge precedes its work, and `Memo::NotMemoized` marks such a result. | Not provided. There is no budget model, and a returned value is memoized. Cancellation is by unwinding, which is not a typed refusal. |
| Typed inconclusiveness (INV-008) | Yes. The parity verdict is Parity, Mismatch or Inconclusive, with typed reasons. | Neutral for values. Cancellation by unwinding must be caught and typed at each boundary. |
| Persistence with commit ordering, index fsck, epoch demotion, `SUPERSEDES` | Not built (residual 9). | Not relied on. RFC 0030's ordering and fsck rules are ours in either design. |

**What salsa would buy:** parallel query evaluation, cycle handling, durability-based
validation skipping, interning, LRU eviction, and maturity at rust-analyzer scale. None
of these is an RFC 0030 obligation that the spike fails.

**What salsa would cost:**

- **The shim becomes the engine.** Salsa supplies exactly one of the four classes,
  `Exact`, and that class is the part the spike implements as a map lookup. The
  `Validated`, `Conservative` and `Experimental` licences, quarantine, provenance, the
  typed explanation, and budgets would be user code around salsa. This is the case the
  RFC 0030 open question warns about.
- **Dependency audit.** Salsa, its procedural-macro crate, and their transitive
  packages need exact-version, exact-checksum full audits under
  `dependency-audits.toml`. That file admits no delta audits, and it needs a fresh audit
  at every version bump. Salsa is pre-1.0 and has been rewritten once. The lock today
  has some of salsa's probable transitive packages (a lock crate, a queue, a hash table,
  `smallvec`, and the `syn` family), but not all. This ADR did not count the new
  packages, because that needs a resolver run on a candidate manifest. A salsa spike
  would count them. Audited dependencies may contain unsafe code, but salsa's
  procedural macros expand into our crate, and there the workspace-wide
  `forbid(unsafe_code)` applies. Whether salsa's generated code compiles under that lint
  is not established here, and an audit must check it.
- **No expected precision gain, a hypothesis.** 29 of the 32 FPs in the baseline are key
  misses that follow from the query decomposition and its keys, and the other 3 follow
  from a changed exploration (see the FP causes above). Salsa's backdating gives the
  same early cutoff that content-addressed keys give. With the same decomposition,
  salsa would recompute at least this set, and also the 32 `Conservative` reuses and,
  unless user code checks the witness, the 6 `Validated` ones. Salsa records
  dependencies automatically at the granularity of what a query reads. That could
  narrow check_invariant's read of the transition-system projection (the 10 FPs of
  residual 2) without a hand-built projection. That possibility is untested, and
  reversal criterion 4 is the test.

## Decision

### D1 — the incremental engine is custom

Phase C builds the §9 engine (PR 23) on the spike's design. The design has a registry of
query definitions, canonical content-addressed keys over output identities, one licence
decision per reuse checked against quarantine and lane, provenance of every licence
triple, and an audit that shares no decision logic with the engine. PR 23 adds no
memoization framework.

### D2 — precision is a property of the decomposition, and it is measured

The engine's precision is tracked against `pr22a-impl03-precision-baseline`, per lane
and per reuse class. PR 23 must keep FN at zero. When PR 23 adds queries or splits keys,
it re-baselines, and over the labels this baseline judges its combined promotion-lane
precision must stay at or above 62/94 (659‰). A lower value needs an ADR revision that
gives the reason.

### D3 — the textual `Experimental` heuristic is not carried forward

The spike's parse heuristic reuses a parse across an edit whose independence it only
guesses from normalized text. RFC 0030 permits heuristic independence on `Experimental`
edges, so this is a decision on the measurement, not a conformance rule. The trade is
poor: over 32 edits it saved 9 parse recomputes, which are the cheapest queries, and it
served 2 stale parses that let the engine produce 11 phantom artifacts. In the traces,
one more stale parse hid 11 clean outputs (5 and 6). PR 23 does not
ship it. `Experimental` stays in the vocabulary for what RFC 0030 admits, and a future
heuristic needs its own measurement.

### D4 — R21's kill signal did not fire on the spike corpora, and it stays live

On both corpora, in the promotion lane, `Exact` and `Validated` license 59 of the 91
correct reuses (648‰). `Conservative` licenses 32. `Validated` is exercised with zero
stale reuses. So the edge classes did not collapse to `Conservative`. The G5 reference
workload does not exist yet. So this is not the R21 evaluation, and the kill signal is
evaluated again at G5 on that workload.

### Reversal criteria

This decision is revisited, by a new ADR that supersedes this one, if any of these hold:

1. **Latency.** The docs/34 latency accounting at the Phase C exit attributes more than
   a quarter of interactive-lane wall time to engine overhead (key hashing, cache
   lookup, and licence decision, but not the queries' own work) on the reference
   workload.
2. **Concurrency.** The lead ratifies a Phase C requirement for concurrent evaluation of
   queries within one revision, and PR 23's engine does not meet it under INV-005.
3. **Precision.** After PR 23, combined promotion-lane precision on this corpus falls
   below 62/94 (D2), and the cause is the memo table or its validation, not the query
   decomposition.
4. **A counter-spike.** A salsa spike under a human-recorded audit implements the four
   classes, per-triple quarantine and typed explanations over the same corpora, with
   zero under-invalidation. It must have less engine code outside salsa than the custom
   engine has in total, and precision no worse than this baseline. No bone funds this
   spike today. The lead decides whether to fund it, for example after criterion 1, 2
   or 3 fires.
5. **Maintenance.** More than one in three PR 23–24 review defects are in the memo
   table and its validation (not in licences, audit or budgets). Those are the defects
   a mature substrate would absorb. The spike's two security reviews found none in
   that category.

R21's kill signal (D4) is not a reversal criterion. A collapse to `Conservative` comes
from dependency capture, and salsa does not supply the `Conservative` class, so a
substrate change would not repair it. The plan §24 kill criterion governs it.

## Consequences

- Phase C is unblocked: PR 23 builds on `crates/continuum-incremental`.
- No new crates.io package is needed for the engine, and the dependency-audit surface
  does not grow.
- The engine team owns the memo table, its validation, and any later concurrency. That
  is code a framework would otherwise supply (reversal criteria 2 and 5).
- The precision baseline becomes a regression reference. Its golden fails when an
  engine change moves a count, so every precision change is reviewed.
- The residuals below are Phase C work, and each is owned by PR 23 or PR 24.

## Residuals and deviations

These are carried forward from the spike's crate documentation, with the ones this
baseline found added:

1. **No run configuration.** The engine lowers with no `RunConfig`, so
   `examples/abstract_register.ctm` and `examples/durable_register.ctm` fail build_model
   with `cml.lower.non_integer_state`. The register corpus here is a hand
   concretization. PR 23 must put the run-configuration identity in the key
   (`strategy_config`).
2. **check_invariant keys the whole transition-system projection.** This causes 10 of
   the 32 promotion-lane FPs. A narrower key needs a definedness projection of the
   actions (assumption A2).
3. **`Validated` saves no work in this pipeline.** Every invariant check demands
   `explore`, which recomputes when the transition-system projection changes. The class
   and its checker path work, and its benefit is not shown.
4. **The promotion lane only refuses `Experimental`.** RFC 0030 also requires it to
   recompute or independently validate every promotion-relevant query.
5. **The budget rule is not modelled.** Explore bounds are `strategy_config`, not
   budget, and there is no frontier inclusion.
6. **A recompute after a downgrade overwrites the entry**, with no `SUPERSEDES` edge.
   Only the semantic epoch is pinned in the key, and the other five are unpinned by
   omission.
7. **The explanation is incomplete.** Each invalidation carries one explanatory chain,
   not the complete `<handle>:<reason>` edge set of `rule query.invalidation_edges`.
   The daemon's `query.explain_invalidation` is not wired, because it takes an RFC 0031
   diff handle that the daemon cannot build yet.
8. **The differential oracle does not check definedness** (bn-1eoco).
9. **Not built:** persistence and crash ordering, the index verifier, `Revalidate`
   demotion, witness-loss downgrade, quarantine clearing, continuations, and the Lean
   computed-cone theorem.
10. **No wall-clock measurement.** The baseline counts recomputes and explored states.
    The docs/34 latency accounting is Phase C.
11. **Every corpus model is one file.** So module granularity means "recompute
    everything", and the sub-file reuse counts (53 `Exact`, 6 `Validated`, 32
    `Conservative`) are far below the 4,603 per class that research/27 requires for
    sub-file trust. This baseline is not evidence for the plan §24.5 row. Module
    granularity stays a supported configuration (RFC 0030).

## Alternatives considered

1. **Adopt salsa as the substrate.** Rejected for Phase C. It supplies `Exact` with
   early cutoff and not the other three classes, quarantine, provenance, typed
   explanations or budgets, so the shim would be the engine. It adds a pre-1.0
   dependency tree that needs full audits. It gives no precision gain on the measured
   decomposition. Reversal criterion 4 keeps this option open.
2. **Fork salsa and extend it with licences.** Rejected. It has the audit cost of
   option 1, and it adds a fork to maintain. The extension is the same code the custom
   engine already has.
3. **Adopt a general incremental-computation framework (adapton-style or
   differential-dataflow-style).** Rejected. Each has the gap of option 1, and a
   dataflow runtime also adds a scheduler that INV-005 would have to control.
4. **Module-granularity invalidation only.** Rejected as the design and kept as the
   fallback (RFC 0030). The baseline measures it at 345‰ precision, against 659‰ for
   the spike, and it explores 3,588 states against 1,444.
5. **Defer the decision to Phase C.** Rejected. Phase C is `BLOCKED` on this ADR, and
   the spike and the baseline give the evidence the plan asks for.

## Compatibility

There is no semantic epoch change, no schema change, no protocol change, and no IDL
change. The crate's public API is unchanged. Only tests, a test fixture, a golden, and
this dossier change. `query.explain_invalidation` stays unwired on the wire.

## Security

The engine processes untrusted CML (INV-016). The spike charges every piece of work
before doing it, refuses atomically, and keys no budget. PR 23 keeps that discipline.
Under the custom design the whole engine is in-house code under the workspace
`forbid(unsafe_code)`, and no framework adds unsafe code to its build graph. The trusted
checking base does not change: a `Validated` witness is checked only by
`continuum-certificate` from wire form, and `continuum-incremental` stays outside the
checking base (INV-004, `tools/check_crate_boundaries.py`). Under-invalidation is the
unrecoverable error. The independent audit is the control, and it measured zero here.
Adopting salsa would also have added an unaudited dependency tree to a crate that
processes untrusted input.

## Performance hypothesis

Engine overhead (key encoding charged by length, `BTreeMap` lookups, licence decisions)
is small beside the queries it saves. In the combined promotion lane the spike explores
1,444 states where a clean run explores 3,588 (402‰). It recomputes 94 of 185 compared
labels, where module granularity recomputes all 185. The hypothesis is falsified by
reversal criterion 1. No wall-clock claim is made until the docs/34 accounting exists.

## Validation plan

- `crates/continuum-incremental/tests/precision_baseline.rs` pins
  `pr22a-impl03-precision-baseline` byte for byte, with the audit verdict on each case
  line. It makes the assertions listed at the end of the baseline section, in every
  corpus, mode and lane. It also asserts that module granularity recomputes at least
  what the spike recomputes, and it checks its own FP and FN sets against
  `audit::cone`.
- `tests/reuse_spike.rs` pins `pr22a-impl02-reuse-spike`, and the crate's other tests
  cover the classes, quarantine, the witness path, independence and the differential
  oracle.
- At the Phase C exit, PR 23 re-runs the baseline and reports it against D2. At G5, the
  R21 measure and reversal criterion 1 are evaluated on the reference workload.

## Rollback

Supersede this ADR with one that adopts a substrate. Nothing is published under the
spike's cache, so no artifact needs migration: the cache is not persisted, and RFC 0030
makes discarding the index a valid recovery. The baseline test and its golden stay as
the comparison reference for the replacement.

## Evidence required

- The two retained artifacts named in the header, current under `just check`.
- For a superseding ADR: a counter-spike per reversal criterion 4, measured with the
  same test over the same corpora, and a recorded audit of every new package.

## Supersession

This ADR is superseded only by an ADR that meets a reversal criterion with measured
evidence. A change of RFC 0030's closed sets does not supersede it, but it re-runs the
baseline.

## References

- plan §9, §21 Phase B, §24, §24.5; docs/08 R21; docs/34
- RFC 0030 (reuse-edge classes, downgrade rules, quarantine, Incremental Parity Audit,
  open questions); RFC 0031; RFC 0026 `query.explain_invalidation`
- research/27 (sub-file incremental trust, `quote-id=subfile-incremental-trust-mismatch-rate`)
- ADR-0044, ADR-0003, ADR-0013, ADR-0035
- START_HERE PR 22a and PR 23; bn-31vf, bn-3gf6, bn-13sa, bn-1eoco
- salsa: https://salsa-rs.github.io/salsa/
