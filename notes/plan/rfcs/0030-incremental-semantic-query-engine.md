# RFC 0030: Incremental Semantic Query Engine

## Status
Draft for implementation.

**Target gate:** G5 (incremental trust)
**Owners:** incremental/database leads
**Normative language:** MUST/MUST NOT/SHOULD/SHOULD NOT/MAY per RFC 2119.
**Normative wire vocabulary:** [`../schemas/continuumd-native-protocol.idl`](../schemas/continuumd-native-protocol.idl) (RFC 0026) — `query.explain_reuse`, `query.explain_invalidation`, `query.clean_compare`, `EpochSet`, `Budget`, `Cost`, `TaskRecord`, `ContinuationHandle`, `DefectHandle`, `ErrorCode`. Where this document and the IDL disagree about a wire shape, the IDL decides and this RFC is corrected ([RFC 0026](0026-continuumd-native-protocol.md), "IDL and versioning"); every such correction is recorded below.
**Companion normative sources:** [RFC 0031](0031-semantic-and-intent-diff.md) (the impact set, the dependency-reason mapping, and the `Unknown`-is-dependent rule), [RFC 0032](0032-repair-transaction-protocol.md) (gates `intent_integrity`, `certificate_rebuild`, and `incremental_parity`, and promotion-time recomputation), [RFC 0005](0005-certificates-and-independent-kernel.md) and [RFC 0024](0024-proof-receipt-format.md) (certificate and receipt freshness), [ADR-0044](../adr/0044-incremental-semantic-database.md), [`../plan.md`](../plan.md) §9 (with §4.5, §4.6, §4.7, §8.6, §22 G5), [`../docs/42_INCREMENTAL_VERIFICATION.md`](../docs/42_INCREMENTAL_VERIFICATION.md).
**Landed vocabulary:** `crates/continuum-value/src/epoch.rs` (`EpochKind`, `EpochIdentity`, the five content-identity epoch newtypes, `ProtocolEpoch`, `Compatibility`, `EpochAdvance`, `EpochBinding`, `EpochSet`, `EpochSet::first_mismatch`) is the typed source of the epoch rules this RFC builds continuation resume and query-key identity on. This RFC and that module MUST be revised together; neither may move alone.
**Frontier dependency:** sub-file incremental trust is a **draft** register row (plan §24.5, lane `research/27`). This RFC MUST NOT be read as ratifying it. No numeric mismatch-rate threshold and no ratified sampling rate is stated here; module-granularity invalidation is the named fallback and MUST remain a supported configuration.

## Summary

All derived artifacts are memoized queries over immutable inputs (plan §9). Reuse must be fast enough for interactive work and auditable enough for trust: every reuse edge is classed, every dependency edge carries a typed reason, and the Incremental Parity Audit (renamed from "clean-build Tribunal"; plan §9.5) continuously compares incremental against clean recomputation (INV-010, B9).

This RFC is the normative home of the query key, the closed reuse-edge class set and its downgrade rules, the closed dependency-reason set, the closed auditability-class set, the invalidation-cone contract, the audit's sampling and quarantine rules, continuation-epoch resume, and the crash-safety ordering. Plan §9 and docs/42 are informal restatements; where they disagree with this document, this document governs (plan §25: "Where plan prose and RFC disagree, the RFC is corrected and becomes normative"). Every such correction is recorded below under "Corrections recorded by this RFC".

Two properties are load-bearing and are stated once here so nothing downstream has to re-derive them:

- **Under-invalidation is the only unrecoverable error.** Over-invalidation costs latency; under-invalidation produces a stale green result, which the release-blocker doctrine treats as a blocker, not a bug ticket (docs/42, plan §22).
- **Inconclusiveness is typed, never absent.** A reuse decision the engine cannot justify is `Unknown`, and `Unknown` is dependent (INV-008; RFC 0004's rule generalized).

## Versioning and revision

- **Closed sets.** The reuse-edge class set (four members), the dependency-reason set (nine members), the auditability-class set (three members), the reuse-evidence form set (five members), and the independence tri-state (three members) are **closed**. Adding, removing, or renaming a member is a breaking change requiring an explicit revision of this RFC and, where the token reaches the wire, an RFC 0026 protocol-major change under its N and N−1 window.
- **Fail closed on unrecognized tokens.** A consumer that reads a reuse-edge class, dependency reason, or auditability class it does not recognize MUST treat the edge as `Experimental` with independence `Unknown`, MUST NOT treat it as reusable for promotion, and MUST NOT read the absence of a recognized token as `Exact`. Forward compatibility is achieved by blocking, never by ignoring.
- **Query-definition versioning.** Every query definition carries a `function_version`. It MUST be advanced whenever the definition's output could change for any input — including a change of internal algorithm *believed* to be output-preserving. `function_version` is not a release counter and MUST NOT be derived from the daemon version or from engine identity; engine identity is provenance (plan §4.7), not query identity, and MUST NOT be substituted for it.
- **Auditability class is a property of the definition.** It MUST NOT be declared per run and MUST NOT be reclassified after a mismatch (plan §9.5). Changing a query's auditability class is a revision of the query definition and MUST advance `function_version`.
- **Index-format versioning.** The persistent query index declares a format version. A daemon MUST reject an index whose format version it does not implement rather than best-effort decoding it (docs/09 T13); the index verifier reports the rejection explicitly. An index format change is not an epoch advance: the index is a cache, and discarding it MUST always be a valid recovery.
- **Cache entries are never rewritten.** A committed query result is an immutable artifact under plan §4.4. Re-deriving under a new key produces a new identity linked to its predecessor by a `SUPERSEDES` edge (plan §4.6, INV-009); a later run MUST NOT edit an earlier artifact to reflect a better answer.
- **Epoch advances do not migrate cache entries.** An advance's per-artifact-class compatibility statement (`Compatibility`, plan §4.6) governs the entries of that class: `Preserved` leaves their reuse edges as they are; `Revalidate` demotes every reuse edge into that class from `Exact`/`Validated` to `Conservative` at best; `Incompatible` makes them unreusable and GC-eligible. The statement is published *before* the advance is applied, so the demotion is computable before any query is served under the new epoch.

## Query key

A memoized result is identified by its **query key**:

| Component | Type | Notes |
|---|---|---|
| `function_id` | stable string | the query definition's registry name; see "Query definition registry" |
| `function_version` | stable string | the definition's implementation version, per "Versioning and revision" |
| `canonical_input_identities` | ordered list of content identities | every input the definition names, each a plan §4.4 handle or a content commitment, in the definition's declared order |
| `consumed_epochs` | restricted `EpochSet` | exactly the epochs the definition declares it consumes; see "Which epochs enter the key" |
| `strategy_config` | canonical encoding | the definition's declared configuration: strategy, portfolio selection, solver encoding, normal-form version, reduction settings, determinism knobs |

Rules:

- **A source hash alone is never a sufficient key** (plan §9.2). A key that omits semantic configuration is not a key; it is a collision.
- **The key is canonical.** Two keys are equal iff their canonical encodings are byte-equal (ADR-0013). Field order, list order, and the encoding of absence are fixed by the definition, so one key has exactly one spelling.
- **What is not in the key.** `budget`, wall-clock time, actor identity, capability, `request_id`, `idempotency_key`, tokenizer identity, output policy, page size, engine identity, and the protocol epoch MUST NOT enter the query key. Each belongs to *execution* identity or to the connection. Admitting any of them would fragment the cache without changing meaning; admitting the protocol epoch would make an artifact's identity depend on which client asked for it.
- **Budget rule.** Budget belongs to execution identity, not output identity, for any query whose result is budget-independent when it completes (`parse`, `elaborate`, `property_automaton`, diffs). For budget-truncated searches (`explore`, proof search, synthesis) the committed **frontier** is part of output identity: the committed artifact records the budget actually consumed and its coverage frontier, and two runs of the same key with different budgets produce *comparable, monotone* artifacts related by frontier inclusion, plus a continuation — never two conflicting "results" under one identity (INV-009, B18). The consumed budget is recorded as `Cost` provenance; it is never a key component.
- **Frontier inclusion is checkable.** A budget-sensitive definition MUST carry its frontier in a form for which inclusion between two artifacts of the same key is decidable by the daemon without re-running the engine. A definition that cannot supply one MUST NOT be declared budget-sensitive; its reuse is `Experimental` until it can.

### Which epochs enter the key

The six independently versioned epochs (`crates/continuum-value/src/epoch.rs`, `EpochKind::ALL`) do not all bear on every query. Each definition declares a closed subset, and only the declared subset enters `consumed_epochs`:

| Epoch | Enters the key when | Rationale |
|---|---|---|
| `semantic` | the query interprets a model, property, trace, or program under CML semantics | INV-010 is stated against the semantic epoch; every semantic query consumes it |
| `proof` | the query produces, checks, or reuses a Lean-backed artifact | the pinned toolchain and theorem-package closure change what a proof means (ADR-0035, docs/23) |
| `intent` | the query reads Intent Contract fields or the policy tables interpreting them | the vocabulary version changes what a contract field means, even at a fixed `in_*` identity |
| `evidence` | the query reads or emits evidence-graph nodes, edges, or receipts | an artifact is readable only under its declared schema epoch (RFC 0026, `schemas/README.md`) |
| `corpus` | the query consults the pinned corpus or its oracle toolchain | corpus revision and oracle versions are result-determining (RFC 0019, RFC 0021) |
| `protocol` | **never** | connection-scoped; not part of snapshot or artifact identity (`epoch.rs`, `ProtocolEpoch`) |

Engine identity (plan §4.7) is **not** one of the six and is **not** in the key. It is recorded as provenance on every committed result, is pinned by continuations (see "Continuations and epoch resume"), and is the attribution key for audit mismatches. A definition MUST NOT compensate for an engine change by putting engine identity in the key: if an engine change alters output, that is either a `function_version` advance or an engine defect, and the audit exists to tell them apart.

`consumed_epochs` is an `EpochSet` in which every declared epoch is `Pinned` and every undeclared epoch is explicitly `Unpinned`. `Unpinned` is a *named* absence (`EpochBinding::Unpinned`), never a missing field. A key that cannot pin an epoch its definition declares is not a key: the query MUST fail with `EpochUnsupported` rather than compute under an unnamed epoch.

## Query definition registry

Every memoized computation MUST be a **registered query definition** carrying at least: `function_id`, `function_version`, the ordered input specification, the declared `consumed_epochs` subset, the `strategy_config` shape, the auditability class, and whether it is budget-sensitive. An unregistered computation MUST NOT be memoized.

Plan §9.2 and docs/42 each print an illustrative list of queries, and the two disagree in membership and in spelling. Neither is the registry. Plan §9.2's names (`parse`, `elaborate`, `extract_rust`, `build_model`, `property_automaton`, `abstract_state`, `explore`, `check_certificate`, `compile_context`) fix the canonical spelling convention — lowercase, `snake_case`, verb-first — and docs/42's `CamelCase` list names the same computations plus fragment inference, correspondence building, refinement checking, and semantic diff. All are registrable; none is privileged by appearing in prose.

The registry is machine-readable and part of the daemon's declared surface: `query.explain_reuse` and `query.explain_invalidation` report `function_id` values, so an implementation that cannot enumerate its definitions cannot satisfy the explain contract.

## Reuse-edge classes

Every reuse of a memoized result is licensed by exactly one **reuse-edge class**. The set is closed and has four members. Plan §9.3 calls them "dependency edge classes" and docs/42 calls them "reuse classes"; they are one set, carried on the reuse edge:

| Class | Closed definition | Admissible reuse evidence | Reuse rule |
|---|---|---|---|
| `Exact` | the output is a pure function of the named inputs under the declared `consumed_epochs` and `strategy_config`, and the key matches | exact content-function identity | reuse by content identity, with no check beyond decoder and integrity |
| `Validated` | the output is accompanied by a witness an independent checker accepts *for this input digest* | translation-validation witness; checked certificate | reuse only after the witness is re-checked by the independent checker; a witness that fails to check is not a downgrade, it is a defect |
| `Conservative` | invalidation may over-approximate the dependency cone but MUST NOT under-approximate it under stated assumptions | conservative dependency-theorem instance, with its assumptions enumerated | reuse permitted; the assumptions are recorded on the edge and audited |
| `Experimental` | speed-up only; soundness is not argued | clean-comparison receipt, or none | MAY serve interactive previews and search heuristics; MUST NOT support promotion or any finality claim until cleared by clean validation |

- **Trust order.** `Exact` and `Validated` are the *strong* classes, `Conservative` is *sound but imprecise*, `Experimental` is *unsound-permitted*. The downgrade order is `Exact ≻ Validated ≻ Conservative ≻ Experimental`. It is a downgrade ladder, not a quality ranking: a `Validated` edge is not a weaker `Exact` edge but a different justification, and an implementation MUST NOT "upgrade" a `Validated` edge to `Exact` because its witness checked.
- **Meet rule for composition.** A result derived through several reuse edges carries the **meet** (weakest) of their classes. One `Experimental` edge anywhere in a derivation makes the whole derivation `Experimental`. This is the cache-side form of research/27's "assurance of the consuming result is the meet/composition of dependency assurance", and it is why `query.explain_reuse` MUST report the class of every edge rather than a summary class.
- **No promotion on `Experimental`.** An `Experimental` reuse MUST NOT support promotion, MUST NOT contribute to a `validated` or `proved` claim status, and MUST NOT be reported as `reused` in an RFC 0031 impact set. RFC 0031 states the same rule from the diff side; the two MUST agree.
- **Heuristic independence.** Heuristic (unproven) independence between an edit and a query is permitted **only** on `Experimental` edges. On every other class, independence MUST be established by witness or by the conservative cone.

### Downgrade rules

Downgrades are one-directional and are recorded, never inferred:

1. **Quarantine downgrade.** An audit mismatch attributable to a query drops the implicated `(reuse-edge class, function_id, function_version)` triple to `Experimental` until cleared. See "Quarantine".
2. **Epoch downgrade.** A published advance whose per-class compatibility statement is `Revalidate` demotes every reuse edge into artifacts of that class from `Exact`/`Validated` to `Conservative`; `Incompatible` makes them unreusable. The demotion is applied at the advance, not lazily on first read.
3. **Freshness downgrade.** A reuse edge whose supporting certificate or proof is stale (see "Proof and certificate freshness") MUST NOT be `Exact` or `Validated`. It is `Conservative` where a conservative dependency theorem still covers it and `Experimental` otherwise.
4. **Unknown-dependency non-downgrade.** A reuse whose independence from a change is `Unknown` is not a downgrade at all — it is an invalidation, and the query re-runs. No class licenses reuse across an `Unknown` dependency.
5. **Witness-loss downgrade.** A `Validated` edge whose witness is unavailable — purged, redacted, or garbage-collected — becomes `Experimental`, never `Exact`. A `Redacted(reason, commitment)` witness is reported and downgrades per plan §18.4; it is never read as a checked witness.

Upgrades exist only through recomputation. A quarantined triple is cleared by clean evidence, and an `Experimental` result becomes strong only by being recomputed clean or by acquiring a witness that checks. An implementation MUST NOT raise an edge's class in place.

## Reuse evidence

A cache lookup returns both a value and its **reuse evidence** (research/27, "proof-carrying cache entries"). The set of forms is closed:

| Form | Content | Licenses |
|---|---|---|
| `content-identity` | the key matched byte-for-byte and the content digest verified | `Exact` |
| `translation-validation-witness` | a witness object for this input digest, checkable by an independent checker | `Validated` |
| `checked-certificate` | a certificate proving the result for this input digest, checked from its wire form (INV-004, RFC 0005) | `Validated` |
| `conservative-dependency-theorem` | an instance of a stated closure theorem, with its assumptions enumerated | `Conservative` |
| `clean-comparison-receipt` | a receipt of a clean recomputation that agreed, pinned to a key and an epoch set | clearing a quarantine; never an in-place class upgrade |

- Reuse evidence MUST be retrievable through `query.explain_reuse`. A reuse the daemon cannot explain is not a reuse it may rely on.
- A checker validating a witness MUST be independent of the engine that produced the result (INV-004, plan §20): Forge and the engine crates are never in the checking path.
- Reuse evidence is subject to the retention and redaction policy of plan §4.5 and §18.4. Post-promotion summarization is permitted; the summary MUST preserve the class, the `function_id`, and the `function_version`, because those are what a quarantine is scoped to.

## Semantic dependency reasons

Every recorded dependency edge carries exactly one **typed reason**. The set is closed and has nine members, mirroring plan §9.4 one for one:

| Wire token | plan §9.4 bullet | Meaning |
|---|---|---|
| `reads-type` | reads type | the query consumed a declaration's type, not its value |
| `reads-value` | reads value | the query consumed a definition's value or body |
| `unfolds-definition` | unfolds definition | the query expanded a definition, so its body is semantically inside the result |
| `selects-instance-or-profile` | selects instance/domain profile | the query resolved a domain-pack instance or a fidelity profile (RFC 0002) |
| `observes-event-family` | observes event family | the result depends on which events an observer publishes |
| `relies-on-assumption-fairness-bound` | relies on assumption/fairness/bound | the result is conditional on an assumption, a fairness constraint, or a bound |
| `uses-abstraction-component` | uses abstraction component | the query consumed an abstraction or correspondence map component (§16, RFC 0036) |
| `depends-on-lemma-or-checker` | depends on proof lemma/certificate checker | the result rests on a lemma, a checker identity, or an assurance requirement |
| `consumes-encoding-epoch-or-correspondence` | consumes solver encoding epoch; consumes source correspondence | the result depends on a solver encoding, an epoch-scoped encoding, or a model/program correspondence link |

- The nine reasons drive precise invalidation and are the vocabulary `query.explain_invalidation` reports.
- **Reason granularity is a floor, not a ceiling.** A definition MAY record finer provenance alongside the reason; it MUST NOT record a finer *reason token*, because the closed set is what the invalidation planner and RFC 0031's impact mapping are written against.
- A dependency an engine can observe but cannot classify MUST be recorded with the reason it can justify and with independence `Unknown`. It MUST NOT be dropped.

### Agreement with RFC 0031's impact mapping

RFC 0031 maps diff `field` changes onto these reasons. That mapping is normative there and is reproduced here in the reverse direction so the two RFCs can be checked against each other; a disagreement is a defect in one of them, resolved by revision, never by divergence:

| Dependency reason | RFC 0031 diff `field` |
|---|---|
| `relies-on-assumption-fairness-bound` | `assumptions`, `fairness`, `bounds` |
| `observes-event-family` | `observers` |
| `uses-abstraction-component` | `abstraction_maps` |
| `depends-on-lemma-or-checker` | `assurance` |
| `consumes-encoding-epoch-or-correspondence` | `scope`; the `correspondence` program axis |
| none — a key change | `properties`: a claim edit changes the contract's canonical encoding, hence the `in_*` identity, hence `canonical_input_identities`. Evidence keyed to the old identity is not stale, it is unaddressable, and lands in `invalidated` unless a `Validated` edge carries a checker witness licensing reuse |
| none — `Unknown` | `faults`, `completion_policy`, `nondeterminism`, `trust_boundaries`, `security_policy`, `optimization`, `non_vacuity`: no named reason, so independence is `Unknown`, so the evidence lands in `unknown`, never in `reused` |

The last row is the load-bearing one: a field with no named reason yields `Unknown`, and `Unknown` is dependent. Adding a reason for one of those seven fields is a coordinated revision of this RFC and RFC 0031.

## Independence and the invalidation cone

Independence between an edit and a query is a **tri-state**, generalizing RFC 0004's rule from event independence to query independence:

```text
DefinitelyIndependent(witness)
DefinitelyDependent(reason)
Unknown
```

- **`Unknown` is dependent.** A query whose independence from an edit is `Unknown` MUST be invalidated and re-run. RFC 0031 restates the same rule for the impact set; the two MUST agree.
- **Independence is observer-relative** (RFC 0004): two edits may commute for a safety invariant and not for a fairness or latency property. A `DefinitelyIndependent` witness is scoped to the observing property's footprint and MUST NOT be reused for a query with a different footprint.
- `DefinitelyIndependent` on a strong-class edge MUST carry a witness. Heuristic independence licenses only `Experimental` edges.

The **invalidation cone** of an edit is the set of query keys that must be recomputed. Its contract:

- **Soundness (no under-invalidation).** The computed cone MUST contain the true dependency cone. This is the conservative-closure obligation, and it is the second Lean theorem below.
- **Over-approximation is permitted and measured.** The engine MAY invalidate more than necessary. Precision is a performance property with a kill signal attached: reuse edges collapsing to `Conservative` on the reference workload is docs/08 R21's kill signal and a plan §24 kill criterion.
- **Granularity.** Declaration, action, property, and effect granularity are used where provenance is sound; file granularity is not the design (docs/42). Sub-file granularity is a **draft** frontier row (plan §24.5, research/27) whose kill criterion is "dependency capture cannot be made trustworthy below file/module granularity"; module-granularity invalidation is the named fallback and MUST remain a supported, policy-selectable configuration for as long as the row is unratified.
- **Locality is not a licence.** A small transition change can invalidate reachability globally (docs/42). Reuse MUST be guarded by semantic change classification, never by source locality.
- **Minimality is not claimed.** The engine MUST NOT report a cone as minimal. The invalidation minimizer below computes a minimal *reproducing* delta for a mismatch; that is a different object and MUST NOT be presented as the cone.

## Auditability classes

Absorbed from plan §9.5 (SD-03). Every query definition declares exactly one **auditability class**. The class lives on the query definition, never on the run: it MUST NOT be reclassified per-run, so a mismatch cannot be reclassified away after the fact.

| Class | Meaning | Audit comparison | Divergence outcome |
|---|---|---|---|
| `equality-auditable` | deterministic under the docs/19 §7 determinism matrix | compared bit-for-bit | always quarantines |
| `certificate-auditable` | solver-backed | checked certificates and claim envelopes are compared — never raw solver behavior | a certificate-level disagreement quarantines; a solver-outcome difference with agreeing certificates does not |
| `budget-sensitive` | anytime results | monotone-honesty only: the incremental result's evidence labels MUST be no stronger than a clean run's under equal budget | never quarantines on budget or portfolio grounds; recorded as drift telemetry |

- The quarantine rule is scoped by class: divergence in an equality- or certificate-auditable query always quarantines; divergence attributable solely to budget or portfolio nondeterminism never does.
- **INV-010 applies per auditability class.** "An incremental result claiming exactness must match clean evaluation" binds where the reuse edge is `Exact` or `Validated` on an equality- or certificate-auditable definition, and the comparison basis is the full key (see correction 14). A `budget-sensitive` definition never claims exactness, so INV-010's equality obligation does not reach it; the monotone-honesty obligation does, and it is not weaker, it is different.
- A `budget-sensitive` definition MUST NOT be used to launder a nondeterministic engine. Monotone-honesty is *checked*: an incremental result whose evidence labels are **stronger** than the clean run's is a mismatch of the same severity as a bit-for-bit disagreement, and it quarantines.
- A definition whose determinism is unestablished MUST be declared `budget-sensitive`, never `equality-auditable`. Declaring the stronger class and discovering nondeterminism later produces quarantine storms that hide real defects.

## Persistence, crash safety, and the index

CAS stores outputs; the query index maps keys to content.

- **Commit ordering.** A publishing transaction MUST commit output content before the index entry naming it. A crash therefore leaves unreachable content eligible for GC, never a stale index entry. This is the operational content of INV-017 (plan §4.5) for the query engine.
- **Atomicity.** Disk exhaustion or any other publication failure aborts atomically and returns `PublicationAborted`; nothing is published and nothing truncated (plan §4.5, INV-017). A partially written result MUST NOT become an index entry under any recovery path.
- **Index verifier.** An index verifier (fsck) ships with the daemon and runs on recovery and on verified restore. It MUST detect and report: index entries naming unreachable content; entries whose content digest disagrees with the digest the key records; entries under an index format version it does not implement; and entries whose `consumed_epochs` name an epoch the daemon no longer holds. Its remedy is always to drop the entry, never to repair the content.
- **Recovery of running work.** On restart, `Running` tasks resume from their last committed continuation or transition to `Failed` with a typed reason — never to a silently reconstructed state (plan §4.5).
- **GC interaction.** Cache entries are garbage-collected by reachability from named roots, receipts, and retention policy (plan §4.5). GC MUST NOT collect content an index entry still names; the commit ordering above means an index entry naming collected content is the only direction fsck must handle. Eviction is a performance decision and MUST NOT change a result: evicting an entry costs recomputation, never correctness.
- **Cross-principal sharing.** Content-addressed dedup across principals is an existence oracle (plan §4.5) and is off by default. The query index MUST NOT leak the existence of another principal's key through a reuse report or through timing.

## Incremental Parity Audit

The Incremental Parity Audit is the clean/incremental comparison mechanism of plan §9.5 and ADR-0044. It is **on by default, not opt-in** (docs/42). Two lanes run it:

- **Interactive lane** — maximizes reuse, returns provisional or exact classifications quickly, and recomputes a configurable fraction of interactive queries clean for comparison.
- **Promotion lane** — every promotion-relevant query is recomputed clean or independently validated at promotion time, canonical outputs are compared, and a receipt is emitted (plan §8.2 gate 10, RFC 0032).

CI additionally runs deterministic full-clean sweeps nightly. docs/42 titles these the "dirty/clean dual lane"; the normative names are the interactive lane and the promotion lane, and "dirty" names nothing.

### Sampling policy

- The interactive-lane sampling rate MUST derive from a declared statistical confidence target for mismatch detection **per reuse class** (plan §8.6), reviewed at G5.
- Until that derivation ships, **"1 in 64, uniformly by query key hash" is the labeled bootstrap default and nothing more.** It is not a ratified rate, and a deployment MUST NOT cite it as evidence of a detection probability. The register row that would ratify a mismatch-rate threshold — sub-file incremental trust, plan §24.5, lane research/27 — remains **draft**, and this RFC does not ratify it.
- Selection MUST be uniform by query key hash and MUST NOT be influenced by reuse class, by the engine's confidence, or by observed latency. An audit that skips the queries the engine believes in audits nothing.
- Deployments MAY adjust the rate by policy. An opt-out or a reduced rate MUST be recorded in the assurance envelope of every claim it touches (INV-010, docs/42); an unrecorded reduction is a soundness incident, not a configuration.
- Promotion-relevant queries are **not** sampled: every one is recomputed clean or independently validated at promotion time.

### Compared artifacts

The audit compares, per plan §9.5:

1. verdict;
2. canonical state graph or digest;
3. counterexample class;
4. certificate result;
5. context-slice soundness;
6. semantic diff;
7. proof axiom manifest.

Comparison is over the canonical encoding, under the docs/19 §7 determinism matrix. A difference that is explicitly non-semantic MUST be normalized away by the canonical encoder rather than excused by the comparator.

### On mismatch

A mismatch MUST produce all of the following, in this order:

1. **A mismatch evidence node** published to the evidence graph, and a `defect_*` engine-defect artifact (plan §4.7) pinning all inputs by content identity, the `consumed_epochs`, the checker identity, the engine identity, and an automatically minimized reproduction, under the §18.4 redaction policy.
2. **Class-scoped quarantine**, per "Quarantine" below.
3. **Invalidation minimization**: the minimizer produces the smallest input delta reproducing the divergence, reported as the four-part object of research/27 — smallest edit sequence, smallest missing or incorrect dependency edge, first divergent query, semantic result difference. This treats the incremental engine as another system Continuum can debug.
4. **Surfacing under the release-blocker doctrine**: a stale green result is a blocker, not a bug ticket (docs/42, plan §22).

A mismatch MUST NOT be silently resolved in either direction. The clean result is the reference for *the audit's verdict*; it is not automatically the correct answer, because the defect may lie in either lane, and the defect artifact records both.

### Quarantine

- The quarantine unit is the triple `(reuse-edge class, function_id, function_version)`. Quarantining a class *globally across all queries* is not the rule: one bad definition would disable `Exact` reuse everywhere, which is a denial of service rather than a safety measure.
- A quarantined triple's reuse drops to `Experimental`. Results already committed under it are not rewritten (INV-009); they are re-derived on demand under new identities linked by `SUPERSEDES`.
- Quarantine is cleared only by evidence: a clean-comparison receipt for the implicated triple over a declared key set, or a `function_version` advance that fixes the defect and carries its own clean evidence. Time, restart, and daemon upgrade MUST NOT clear a quarantine.
- Quarantine state is durable and survives restart. It is part of the daemon's declared state and is reported in the assurance envelope of every claim whose derivation touched the triple.
- A `budget-sensitive` divergence records drift telemetry and MUST NOT quarantine, but drift telemetry crossing a declared threshold is a defect report of its own. Silence is not the alternative to quarantine.

### Audit overhead

The audit's own overhead is measured and is part of the docs/34 latency accounting; the Phase C exit budget for it is stated there. "Incremental trust overhead erases interactivity" is a plan §24 kill criterion, so the overhead measurement is gate evidence, not telemetry.

## Proof and certificate freshness

Plan §22 G5 requires that "proof and certificate freshness is tracked". Freshness is defined here:

- A reused **certificate** is *fresh* iff the checker identity and every epoch the certificate pins equal the daemon's current values for the epochs the consuming query declares. A certificate pinning an epoch the daemon no longer holds is not stale — it is unreadable, and reading it is `EpochUnsupported`, never best-effort decoding (docs/09 T13).
- A reused **proof artifact** is *fresh* iff its `proof` epoch equals the daemon's current proof epoch and its axiom manifest is unchanged. A changed elaboration environment invalidates even text-identical proof terms where semantics differ (docs/42); text identity is not freshness.
- A stale certificate or proof MUST NOT support an `Exact` or `Validated` edge (downgrade rule 3). It MAY support a `Conservative` edge where a conservative dependency theorem covers it, and it MAY be shown to a human as historical evidence explicitly labelled with the epoch it was established under — a receipt remains verifiable under its pinned epoch indefinitely (INV-006, INV-014, plan §4.6), which is exactly why it must not be silently read as current.
- Freshness is reported, not inferred: `query.explain_reuse` MUST distinguish "reused with fresh supporting proof" from "reused under a conservative theorem while its proof is stale". Plan §8.2 gate 9 ("invalidated certificates/proofs are rebuilt") is enforced against this predicate.

## Continuations and epoch resume

Plan §9.6: exploration, proof, and synthesis continuations contain committed frontier and search state and the identity of every input; resume rejects mismatched snapshots or epochs; forking a continuation with a new budget is explicit.

### What a continuation pins

A `cont_*` continuation MUST pin:

- the snapshot (`ws_*`) and, where the task is intent-scoped, the intent (`in_*`);
- the committed frontier and search state, in the frontier form the budget rule requires;
- an `EpochSet` naming all six epochs — every epoch the task consumed `Pinned`, every one it did not explicitly `Unpinned`, none omitted;
- **engine identity** (plan §4.7), which RFC 0026 requires continuations to pin alongside the semantic epoch and which the IDL carries as `EpochSet.engine`.

**The pinning obligation is what makes resume safe.** `EpochSet::first_mismatch` treats an epoch the continuation left unpinned as constraining nothing — correctly, because a continuation resumes "only under their pinned epoch" and one that pinned nothing declared nothing to resume under. That rule is permissive by design, so the safety of resume rests entirely on the obligation above: a continuation for a semantic task MUST pin the semantic epoch; a proof- or certificate-bearing continuation MUST pin the proof epoch; a corpus-oracle continuation MUST pin the corpus epoch; an intent-scoped continuation MUST pin the intent epoch. A continuation omitting an epoch its task consumed is malformed and MUST be rejected at creation, not at resume.

### Resume decision

`task.resume` validates continuation identity, snapshot, and epochs. The typed outcomes are distinct (INV-008) and MUST NOT be collapsed:

| Condition | Error |
|---|---|
| the pinned snapshot is no longer the current sealed snapshot | `StaleSnapshot` |
| a pinned epoch disagrees with the daemon's current epoch of that kind | `ContinuationEpochMismatch` |
| a pinned epoch names an identity the daemon no longer holds at all | `EpochUnsupported` |
| the pinned engine identity disagrees with the daemon's engine identity | `ContinuationEpochMismatch` |

The epoch comparison over the six kinds is exactly `EpochSet::first_mismatch(continuation_epochs, current_epochs)`: it returns the first disagreeing kind in `EpochKind::ALL` order (`protocol`, `semantic`, `intent`, `evidence`, `proof`, `corpus`), treats a pinned-but-absent epoch as a mismatch rather than as permission to decode best-effort, and returns none when the resume is admissible. The reported error MUST name that kind, so "why was my resume rejected" is answerable without re-deriving it.

**Engine identity is checked separately.** `crates/continuum-value/src/epoch.rs` scopes itself to the six compatibility epochs and states outright that engine identity is out of scope for that module, while RFC 0026 and the IDL's `EpochSet.engine` require continuations to pin it. The daemon therefore composes two predicates — `first_mismatch` over the six, and an engine-identity equality check — and a disagreement in either yields `ContinuationEpochMismatch`. Neither artifact is wrong; the composition is the daemon's obligation, and it is recorded here so it is not lost between them.

### Resume semantics

- **Resume never silently re-runs.** A mismatch is a typed rejection (RFC 0026); the daemon MUST NOT quietly restart the task under current epochs.
- **Resume is monotone.** Resume MAY add evidence or refine an `Unknown`; it MUST NOT replace prior artifacts under the same identity (INV-009). The frontier after resume MUST include the frontier before it.
- **Continuations are forked across an epoch advance, never migrated in place** (plan §4.6). A fork produces a new continuation with a new identity; the original remains valid for the epochs it pinned.
- **Forking with a new budget is explicit** (plan §9.6). Lowering a budget below committed spend suspends with a continuation rather than truncating the campaign (`task.update_budget`, B18, INV-009); budget exhaustion yields `BudgetExhausted` carrying the continuation and is never a semantic verdict (docs/49).
- **Cancellation** triggers request → drain → finalize and MUST leave either committed partial evidence plus a valid continuation, or nothing published (INV-009, B19). A cancelled task's partial results are cache entries like any other, subject to the same classes and the same audit.

## Wire surface

Three operations in the `query` namespace (plan §10.2, RFC 0026). Their normative shapes are in the IDL; this table summarizes, and the notes are the obligations this RFC adds:

| Operation | IDL annotations | Request | Response |
|---|---|---|---|
| `query.explain_reuse` | `@readonly @paginated`, `authority read` | `derivation: ArtifactHandle` | `reused`, `recomputed` (artifact lists), `reasons` (map) |
| `query.explain_invalidation` | `@readonly @paginated`, `authority read` | `diff: DiffHandle` | `invalidated`, `unknown` (artifact lists), `edges` (list) |
| `query.clean_compare` | `@mutation @task_starting`, `authority read` | `derivation: ArtifactHandle` | `parity` (bool), `defect: DefectHandle` optional; verdict `EvaluationVerdictValue` |

- `query.explain_reuse` answers "why was this result reused": every reuse edge on the derivation, with its class, its dependency reason, its reuse-evidence form, and its freshness. `reused` and `recomputed` MUST be disjoint and MUST together cover every input the derivation consumed; an input in neither is a completeness failure, not an implicit reuse.
- `query.explain_invalidation` answers "why did this re-run" and "which edge caused the rebuild". It is keyed by a `diff_*` handle, so the edit is named by an artifact rather than by an inline delta; this is the site RFC 0031 lists among the nine that carry a `DiffHandle`. Its `invalidated` and `unknown` sets are the sets RFC 0031's impact set defines, computed by this engine and reported by that artifact; the two MUST agree, and an artifact in none of `invalidated`, `unknown`, `reused` is a completeness failure.
- `query.clean_compare` runs the audit on demand. It is a task-starting mutation, not a plain read: it schedules a clean recomputation, may spend budget, and may publish a `defect_*` artifact. It MUST NOT prefer either lane's result; a failed parity is a defect report.
- **Pagination determinism.** All three are subject to the IDL's pagination rule: ordering is deterministic by content identity or a declared sort key, and two identical page requests against the same snapshot and epochs return identical pages.
- **Explain is a read, and reads do not decide.** None of the three may change a reuse class, clear a quarantine, or publish a reuse edge. Only the audit and a `function_version` advance move classes.
- **`BudgetExhausted` is not a parity outcome.** A clean recomputation that runs out of budget returns `BudgetExhausted` with a continuation. It MUST NOT report parity as failed, and it MUST NOT report parity as held.

## Formal model

A simplified Lean model (ADR-0044) MUST state and check, for the finite model of the query graph:

1. **Exact-reuse soundness** — equal keys imply equal results. The existing seed `exactReuse_sound` in `lean/Continuum/Interaction/Incremental.lean` states this shape over an `ExactReuseWitness` of input equality.
2. **Conservative-closure soundness** — the *computed* invalidation cone contains the true dependency cone. The seed defines `ConservativeClosure` as edge-wise containment and proves the single-edge case (`conservative_includes_true_edge`); the computed-cone side — that the transitive closure of the reported relation contains the transitive closure of the true relation, so everything truly stale is invalidated — is **not** yet stated and MUST be added.
3. **Meet composition** — a derivation's class is the meet of its edges' classes, so one `Experimental` edge forces `Experimental` overall.

This is plan §15.3 theorem 5's home ("incremental query equality under valid dependency closure"). The production engine remains more complex than the model; the theorem defines the target contract, not the implementation (research/27). Lean sources carry no `sorry`, no `axiom`, and no `native_decide` (RFC 0012).

## Corrections recorded by this RFC

Per plan §25, where plan prose, docs, a research note, or a dependent artifact disagrees with this RFC, this RFC governs — except for wire shapes, where RFC 0026's IDL governs and this RFC is corrected. The corrections in force:

1. **Budget is not key material.** Plan §9.2 and docs/42 write the signatures `explore(model, intent, strategy, budget)` and `compile_context(evidence_root, query, budget)`, placing budget inside the key. Normative: budget belongs to execution identity, never to output identity; for budget-truncated queries the committed *frontier* is part of output identity and the consumed budget is `Cost` provenance. Direction: RFC corrects plan §9.2 and docs/42.

2. **The query key names more than two epochs.** The previous revision gave the key as `(function_id, function_version, canonical_input_identities, semantic_epoch, proof_epoch, strategy_config)`, which is incomplete for intent-, evidence-, and corpus-consuming queries and over-specified for queries consuming neither of the two it names. Normative: the key carries `consumed_epochs`, a restricted `EpochSet` declared per query definition, from which the protocol epoch is always absent. Direction: this RFC corrects its own earlier key against `crates/continuum-value/src/epoch.rs`.

3. **There is no checker epoch and no solver epoch.** Plan §9.2 writes `check_certificate(certificate, checker_epoch)`, plan §9.4 names "consumes solver encoding epoch", and docs/42 names a "solver epoch". Continuum versions exactly six epochs (docs/12 §7, `epoch.rs`), and none is a checker or solver epoch. Normative: checker identity is the `proof` epoch for Lean-backed checkers and engine identity (plan §4.7) for native checkers; a solver encoding version is part of `strategy_config`. Direction: RFC governs plan §9.2, plan §9.4, and docs/42; `epoch.rs` is unchanged and required no change.

4. **`query.explain_invalidation` is keyed by a diff, not by `(key, edit)`.** The previous revision wrote `query.explain_invalidation(key, edit)`. The IDL declares a `DiffHandle` request and an `invalidated`/`unknown`/`edges` response. Direction: the IDL decides (RFC 0026); this RFC is corrected to the IDL's shape. The same correction applies to `query.explain_reuse(key)` and `query.clean_compare(key)`, both of which take a `derivation` artifact handle.

5. **`query.clean_compare` is not an ordinary read.** The previous revision said all three query operations "are ordinary read operations (plan §10.2)". The IDL declares `clean_compare` `@mutation @task_starting`, with a `BudgetExhausted` error and an `EvaluationVerdictValue` verdict; only `explain_reuse` and `explain_invalidation` are `@readonly`. All three carry `authority read`, which is an authority level, not a read/mutation classification. Direction: the IDL decides; this RFC is corrected.

6. **A conservative disjointness judgement licenses a `Conservative` edge, not a `Validated` one.** Plan §8.6 says gate 5–7 evaluations reuse prior results "as `Validated` edges" when a "Conservative-class §9 dependency query" finds the causal footprint disjoint from the patch delta, and RFC 0032's "Incremental gate campaigns" bullet restates it in the same words while citing this RFC. `Validated` requires a checker-checkable witness for the specific input digest, and a conservative over-approximation is not one; reading a `Conservative` judgement as a `Validated` edge would let the `neighborhood`, `property_mutation`, and `defect_mutants` gates rest on unwitnessed reuse. Normative: a reuse licensed by a conservative disjointness judgement is `Conservative`; it is `Validated` only when the disjointness itself carries an independently checkable witness; an `Unknown` disjointness is dependent and re-runs. Direction: RFC governs plan §8.6, and RFC 0032's bullet carries the same error and MUST be revised to match — this RFC does not edit it.

7. **Quarantine is scoped to a triple.** Plan §9.5 and plan §22 G5 say a disagreement "quarantines the reuse class". Normative: the quarantine unit is `(reuse-edge class, function_id, function_version)`; a global class-wide quarantine is not the rule. Direction: RFC makes plan §9.5 and G5 precise — an abbreviation made exact, not a contradiction.

8. **G5's quarantine bullet is graded by auditability class.** Plan §22 G5 states flatly that "mismatches are minimized and quarantine the class", while plan §9.5 states that budget- or portfolio-attributable divergence never quarantines. Normative: §9.5's class-scoped rule governs and the G5 bullet is its summary. Direction: RFC reconciles §22 G5 with §9.5.

9. **The sampling default is a labeled bootstrap, not a rate.** docs/42 states "default 1 in 64, selected uniformly by query key hash, per RFC 0030" as though this RFC ratified it. Normative: the rate MUST derive from plan §8.6's declared confidence target per reuse class, reviewed at G5; "1 in 64" is the labeled bootstrap default only, and the register row that would ratify a mismatch-rate threshold (plan §24.5, research/27, sub-file incremental trust) is **draft**. Direction: RFC governs docs/42; the register row is untouched by this RFC.

10. **`Complete` is not a reuse-edge class.** research/27 writes "Only Complete/Conservative edge classes may drive trusted invalidation". The closed set is `Exact`, `Validated`, `Conservative`, `Experimental`. Normative: read `Complete` as `Exact`/`Validated`; trusted invalidation is driven by every class except `Experimental`. Direction: RFC governs the research note's exploratory vocabulary.

11. **Eleven edge tokens versus nine reasons.** research/27 lists eleven `SCREAMING_CASE` edge tokens (`READS_TYPE` … `USES_ENCODING_EPOCH`), splitting assumption/fairness/bound into three and omitting source correspondence. Normative: nine reasons in the wire spelling of the table above, mapped one-for-one onto plan §9.4's bullets. Direction: RFC governs; research/27's tokens are the lane's exploratory vocabulary, not a wire set.

12. **One class set, two names.** Plan §9.3 titles the four classes "dependency edge classes", docs/42 titles them "reuse classes", and plan §22 G5 says "reuse edges carry their class". Normative: one closed set, *reuse-edge classes*, carried on the reuse edge and quarantined per triple. Direction: RFC unifies; both prose names are aliases.

13. **"Dirty lane" names nothing.** docs/42's section title "Dirty/clean dual lane" introduces a term the dossier never defines. Normative: the two lanes are the *interactive lane* and the *promotion lane*. Direction: RFC governs docs/42.

14. **INV-010's stated condition is necessary, not sufficient.** INV-010 states the parity obligation "for the same snapshot and semantic epoch". A well-defined comparison also requires the same `function_version`, the same `strategy_config`, and the same remaining `consumed_epochs`. Normative: this RFC states the full comparison basis. Direction: RFC states INV-010's application in full; the invariant is not weakened — the added components make the obligation harder to satisfy, never easier.

Flags raised against artifacts this RFC does not own (no silent divergence):

- **F1 — `query.clean_compare` returns a bare boolean parity.** A boolean cannot distinguish bit-for-bit agreement, certificate-level agreement with a differing solver outcome, monotone-honesty agreement on a `budget-sensitive` definition, and an inconclusive comparison — exactly the distinctions INV-008 requires. A typed parity outcome carrying the auditability class and the comparison basis belongs in the IDL; raised for the protocol sweep, not patched here.
- **F2 — the explain responses are free-form strings.** `reasons` is a string map and `edges` a string list, while the nine dependency reasons and four reuse-edge classes are closed enums. INV-003 forbids prose-only machine interfaces; these should be wire enums with prose as a rendering. Mirrors RFC 0031's F4.
- **F3 — there is no handle for a query key.** `query.explain_reuse` and `query.clean_compare` take an artifact handle for the *derivation*, so a key whose result was never materialized — the interesting case for "why did this re-run" — is unaddressable. Either a key handle is registered in plan §4.4 or the operations gain a key-by-value form.
- **F4 — reuse evidence has no wire shape.** The responses carry handles and free-form reasons but no field for the reuse-evidence form or the witness, though this RFC requires both to be retrievable through `explain_reuse`. Raised for the protocol sweep.
- **F5 — freshness is unrepresented on the wire.** Plan §22 G5 requires proof and certificate freshness to be tracked, and no IDL field or schema carries it. The predicate is defined above; its wire home is not.
- **F6 — `EpochSet::first_mismatch` cannot check engine identity.** `crates/continuum-value/src/epoch.rs` scopes itself to the six compatibility epochs, while RFC 0026 and the IDL's `EpochSet.engine` require continuations to pin engine identity. The daemon must compose two predicates (see "Resume decision"). Neither artifact is wrong; the composition obligation is undeclared in both, is recorded here, and is declared by RFC 0026 as of 2026-07-31 ("The two-predicate obligation", correction 18) token for token with "Resume decision" — the two documents MUST be revised together.
- **F7 — the query-definition registry has no schema.** This RFC requires every memoized computation to be a registered definition with a declared auditability class, `consumed_epochs`, and budget sensitivity, and no schema in `schemas/` describes that record. Raised for the schema sweep.

## Rejected alternatives

- **File-granularity-only invalidation.** Retained as the *fallback*, not the design: research/27's kill criterion drops sub-file granularity if dependency capture proves untrustworthy, and the module-granularity fallback MUST remain a supported configuration.
- **Trusting engine-reported dependencies without audit.** Rejected: B9 — incremental reuse is dangerous exactly when invalidation is silently unsound.
- **Timestamp or mtime invalidation.** Rejected: file timestamps and module dependency alone are too coarse (research/27), and a timestamp is not a content identity (plan §4.4).
- **A single global reuse class per daemon.** Rejected: the meet rule needs per-edge classes and quarantine needs a triple. A daemon-wide "trust level" cannot express "this derivation is `Experimental` because one of its forty edges is".
- **Making engine identity part of the query key.** Rejected: it would invalidate the whole cache on every daemon build and would hide engine defects inside key churn instead of surfacing them through the audit. Engine identity is provenance and a defect-attribution key.
- **Letting a passing audit upgrade a reuse class in place.** Rejected: a clean-comparison receipt clears a quarantine and licenses recomputation; it does not retroactively make an `Experimental` result exact. Upgrades happen by recomputation, and the recomputed artifact gets a new identity (INV-009).
- **A third parity value for "could not decide".** Rejected in favour of the typed-error path: an audit that cannot complete returns `BudgetExhausted` with a continuation or a typed engine error, not a parity value with no defined promotion semantics. (That the current boolean also cannot express the *other* distinctions is F1, a different problem.)
- **Reclassifying a query's auditability class after a mismatch.** Rejected explicitly by plan §9.5 and restated here: it is the exact move that would let a mismatch be excused after the fact.

## Open questions

- Cache eviction and retention interaction with the GC policy of plan §4.5 — in particular whether reuse evidence outlives the result it justifies, and what a summarized campaign (plan §4.5) owes the audit.
- Stable reuse identities for nondeterministic engines (research/27 kill criterion) — candidate answer: canonicalize by committed frontier, not by execution. Whether that survives portfolio solvers is unsettled.
- Whether the invalidation minimizer can run inside the interactive latency budget, or whether it is always a background task producing a defect artifact.
- Whether a `Validated` edge's witness check can be made cheap enough to run on every reuse, or whether witness re-checking is itself sampled — and if sampled, whether that sampling is governed by the same confidence target as the parity audit.
- The build-versus-adopt decision for the memoization substrate (Phase B ADR, plan §9.1, docs/08 R21), and whether an adopted substrate can express the four classes and the nine reasons without a shim that becomes the real engine.
- Which of the seven RFC 0031 fields with no named dependency reason (`faults`, `completion_policy`, `nondeterminism`, `trust_boundaries`, `security_policy`, `optimization`, `non_vacuity`) deserve one, and whether adding one is a precision win or a soundness risk.

## Acceptance

- **Random edit traces** across model, property, correspondence, proof, and context queries yield clean parity; the trace generator covers research/27's nine experiments — property edit, action-guard edit, abstraction-map edit, domain-profile edit, proof-lemma edit, semantics-preserving source refactor, concurrent identical tasks, crash during publication, intentionally missing edge.
- **Intentional dependency bugs** (broken, missing, and mis-reasoned edges) are caught by the audit and quarantine the implicated triple; PR 24's exit.
- **Class-scoped quarantine tests**: an equality-auditable mismatch quarantines; a certificate-auditable mismatch with agreeing certificates does not; a budget-sensitive divergence records drift telemetry without quarantine; a budget-sensitive incremental result whose evidence labels are *stronger* than the clean run's does quarantine.
- **Property-only edits do not rebuild extraction**; model-action edits invalidate reachable graphs and dependent packs (PR 23's exit).
- **Crash-mid-publication** leaves no stale index entries under fault injection; fsck reports unreachable content, digest disagreement, unknown index format version, and unheld epochs, and drops rather than repairs.
- **Clean-versus-incremental conformance vectors** cover every semantic query stage — parse, elaborate, fragment inference, extraction, model build, property automaton, correspondence build, abstraction, exploration, refinement check, certificate check, semantic diff, context compilation — with all seven compared artifacts of plan §9.5 exercised at least once per stage that can produce them.
- **Determinism**: semantic artifacts are byte-identical across the docs/19 §7 determinism matrix (worker counts 1/2/8/32, debug and release, supported targets, hash seeds, snapshot intervals, root-versus-snapshot replay, process restarts); any difference is explicitly non-semantic and normalized by the canonical encoder.
- **Continuation resume**: matched epochs resume; a changed semantic epoch yields `ContinuationEpochMismatch` naming `semantic`; an epoch identity the daemon no longer holds yields `EpochUnsupported`; a changed engine identity yields `ContinuationEpochMismatch`; a stale snapshot yields `StaleSnapshot`; resume never silently re-runs; the post-resume frontier includes the pre-resume frontier.
- **Budget semantics**: two runs of one key under different budgets produce frontier-ordered artifacts, never conflicting results; lowering a budget below committed spend suspends with a continuation; `BudgetExhausted` is never a verdict and never a parity outcome.
- **Explain completeness**: `explain_reuse`'s `reused` and `recomputed` are disjoint and cover every consumed input; `explain_invalidation`'s `invalidated` and `unknown` agree artifact-for-artifact with the RFC 0031 impact set computed over the same `diff_*`; pagination is deterministic across page boundaries.
- **Freshness**: a proof-epoch advance marked `Revalidate` demotes strong edges into the affected class; a stale certificate cannot support `Exact` or `Validated`; plan §8.2 gate 9 is enforced against the freshness predicate.
- **Lean**: `exactReuse_sound` and the extended computed-cone conservative-closure theorem build under the pinned toolchain with no `sorry`, no `axiom`, and no `native_decide`.
- **Overhead**: audit overhead is measured on the reference workload with a saturating background swarm present, and reported against the docs/34 latency accounting for the Phase C exit.
