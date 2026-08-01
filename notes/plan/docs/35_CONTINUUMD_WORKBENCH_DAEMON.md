# `continuumd`: Authoritative Workbench Daemon

> **Status:** Normative for the daemon's operational contract. This document
> absorbs plan §4.5 (crash safety, storage lifecycle, purge, durability and
> restore, the multi-user baseline), §4.6 (semantic epoch advance) and §4.7
> (engine-defect lifecycle), together with the scheduling and memory-budget
> clauses of §4.1 — specification debt SD-09 in plan §25, now paid. Where this
> document and plan §4.5–§4.7 disagree, this document is corrected and becomes
> normative; the plan is a map, not the spec (plan §25). The corrections this
> absorption made are listed under [Corrections](#corrections).
>
> The slices this document does *not* own: the wire surface is RFC 0026 and its
> normative IDL ([`../schemas/continuumd-native-protocol.idl`](../schemas/continuumd-native-protocol.idl));
> epoch-advance semantics are [ADR-0018](../adr/0018-semantic-versioning-and-replay.md);
> the incremental/reuse slice is [docs/42](42_INCREMENTAL_VERIFICATION.md);
> schema identity and `schema_epoch` are [`../schemas/README.md`](../schemas/README.md).
>
> **Normative language:** MUST/MUST NOT/SHOULD/MAY per RFC 2119.

## Responsibility

`continuumd` is the sole authority for mutable workbench coordination. Semantic artifacts themselves are immutable.

It owns:

- snapshot creation and overlay resolution;
- Intent Contract registry and policy;
- query dependency graph and cache;
- tasks, budgets, cancellation, and continuations;
- artifact CAS and publication transactions;
- evidence graph/status transitions;
- proof/solver worker dispatch;
- repair and Forge transactions;
- authorization and audit.

It does not own:

- truth of solver results;
- proof acceptance;
- source control;
- agent planning;
- UI presentation semantics.

## Service topology

```text
clients
  │ native protocol
  ▼
protocol gateway
  ├── auth/capability policy
  ├── snapshot service
  ├── query service
  ├── task service
  ├── evidence service
  ├── debugger service
  ├── repair service
  └── Forge service
          │
          ▼
    isolated workers
  reference · explicit · DPOR · SMT · Lean · agents
```

## Snapshot transaction

1. Client submits root paths, overlay buffers, dependency/config references.
2. Daemon normalizes paths and content.
3. Parsers may produce diagnostics but cannot alter content identity.
4. Snapshot manifest is canonicalized and hashed.
5. Intent is attached by identity, not copied implicitly.
6. Snapshot is sealed and immutable.

An editor may create many cheap overlay snapshots. Garbage collection follows reachability from named roots, tasks, receipts, and retention policy — see [Storage lifecycle, attribution, and summarization](#storage-lifecycle-attribution-and-summarization).

## Task identity

A semantic task identity includes:

```text
operation
snapshot
intent
semantic/proof/engine epochs
normalized parameters
strategy class
```

Budget may be excluded from semantic identity when continuation semantics are monotonic; it remains in execution identity. Two clients of the same principal requesting an identical task share computation freely. Cross-user computation sharing is off by default: content-addressed dedup across principals is an existence oracle and requires an explicit sharing policy — the rule is specified in [Identity, capabilities, and cross-principal sharing](#identity-capabilities-and-cross-principal-sharing).

The protocol epoch is deliberately absent from this list. It is a property of a connection, not of a computation, and MUST NOT enter the identity of any task, snapshot, or artifact (plan §25, SD-13; plan §4.6 forbids identity churn on epoch advance). Adding it would make every task identity change on a protocol upgrade, which is exactly the identity churn §4.6 forbids.

## Idempotency

Clients supply an idempotency key. The daemon records the canonical request digest. Reusing a key with different content is an error. Reusing it with identical content returns the existing task/result.

## Cancellation

Task cancellation is a request–drain–finalize protocol:

- child workers receive cancellation;
- provisional streams close;
- committed partial artifacts are finalized;
- continuation is emitted if supported;
- task transitions exactly once to terminal/suspended state;
- obligations and resource leases are resolved.

Hard-killed foreign workers produce an explicit worker-failure result, never a logical verdict.

## Continuations

A continuation contains:

- exact semantic task identity;
- frontier/search state;
- committed evidence roots;
- engine version;
- random/choice state where relevant;
- resource accounting;
- integrity checksum.

Resume validates all referenced inputs. Forking with a changed strategy or intent creates a new task, not a resume.

## Scheduling, priority classes, and memory budget

Absorbed from plan §4.1.

The scheduler MUST define three priority classes — `interactive` > `ci` > `background` — and this vocabulary is closed: it is the `PriorityClass` enum of the IDL and of [`../schemas/verification-task.schema.json`](../schemas/verification-task.schema.json), and the daemon MUST NOT invent a fourth class or a numeric priority beside it. Higher classes MAY preempt suspendable tasks.

Preemption is the suspend-with-continuation path of B18, not a kill: a preempted task MUST transition to `Suspended` carrying committed partial evidence plus a valid continuation. It MUST NOT transition to `Failed`, and MUST NOT silently truncate a campaign (INV-009). A task that genuinely cannot be resumed states so with a typed non-resumable reason (plan §25, SD-13) rather than losing its evidence quietly.

Every capability grant carries concurrency and resource quotas. Exceeding one is `QuotaExhausted` — a capability-scoped refusal that MUST NOT be spelled `BudgetExhausted` (which is task-budget spend and carries a continuation), and MUST NOT be reported as a semantic verdict (docs/49).

The daemon MUST operate under a declared memory budget with class-aware eviction across the incremental cache, overlay snapshots, and materialized debugger branches. Eviction is confined to results that remain re-derivable from committed artifacts: it MUST NOT be the mechanism by which a published artifact becomes unavailable — that is garbage collection and purge, below — and MUST NOT change any verdict. G5 latency compliance is measured with a saturating background swarm present, not on an idle daemon.

## Evidence publication

Workers return untrusted candidate artifacts. The daemon:

1. validates schema and size;
2. verifies referenced inputs;
3. invokes required independent checker;
4. computes content identity;
5. commits artifact;
6. commits evidence-graph edges/status;
7. emits event/subscription update.

The worker cannot select its own final status.

## Crash safety and the index verifier

Absorbed from plan §4.5.

Publication is ordered: content MUST be committed before the index entry that names it. A crash between the two therefore leaves unreachable content, which is garbage-collectable, and never a stale index entry pointing at content that was never written. The asymmetry is deliberate — wasted bytes are recoverable, a dangling index entry is a lie about what the store holds.

A publication that cannot complete MUST abort atomically and report `PublicationAborted` (INV-017). Disk exhaustion is the motivating case: the daemon MUST NOT truncate an artifact, MUST NOT publish a partial one under a full identity, and MUST leave nothing behind that a reader could mistake for a complete artifact.

On restart the daemon MUST resolve every task it left in `Running` to exactly one of two outcomes:

1. resume from its last committed continuation; or
2. transition to `Failed` with a typed reason (INV-008).

It MUST NOT reconstruct task state by inference and MUST NOT resume from an uncommitted frontier. A silently reconstructed state is indistinguishable from a fabricated one, which is why the choice is binary and typed rather than best-effort.

An index verifier (fsck) ships with the daemon. It MUST be runnable offline against a store, MUST report results per artifact class, and MUST classify every defect it finds as exactly one of:

- **unreachable content** — the ordinary crash residue above; garbage-collectable;
- **missing referent** — a live index entry whose content is absent; re-derived where the inputs survive, otherwise reported as `Redacted(lost, commitment)` (see [Backup and verified restore](#backup-and-verified-restore));
- **identity mismatch** — content that does not hash to the identity it is filed under. This is corruption. The daemon MUST report it and MUST NOT silently repair it, because a store that quietly rewrites content to match an index has abandoned content addressing.

## Storage lifecycle, attribution, and summarization

Absorbed from plan §4.5.

Artifacts are garbage-collected by reachability. The root set is named roots, **live tasks**, receipts, and retention policy. The task roots are load-bearing and are this document's widening of plan §4.5, which omits them (see [Corrections](#corrections)): a `Running` or `Suspended` task's pinned inputs and its continuation MUST be reachable, or the restart contract above cannot be honored.

The store is laid out by artifact class — `<class>/<shard>/<identity>`, implemented in `crates/continuum-workspace/src/artifact_path.rs` — because §4.5's obligations are all per-class: the daemon MUST report storage attribution by artifact class, retention policy is declared per class, and summarization below rolls up whole classes. Placement is a pure function of the handle. It reads no clock, counter, environment, working directory, or random source (INV-005), so the same artifact lands at the same relative path on every machine, and a store listing is a function of content rather than of history.

One plan §4.4 class is outside this section entirely. `cap_*` capability tokens are minted randomly and confer authority; they are not content-addressed, they have no content-derived path, and the artifact-path module refuses to give them one — filing a bearer token under a path derived from itself would publish the secret into the store's directory namespace, where a listing is enough to steal it. Capabilities are therefore governed by mint/scope/delegate/revoke and the audit log, not by GC-by-reachability and not by purge-by-key-shred (see [Corrections](#corrections)).

Campaign-class evidence — neighborhood and mutation results — is summarizable, and summarization is the default after promotion, opt-out per retention policy. The order is normative:

1. a coverage certificate is published and attested by a kernel-covenant checker. The engine that produced the campaign MUST NOT attest its own summary (INV-004);
2. the promotion receipt is amended to reference the summary;
3. only then do the raw per-class results become garbage-collectable under retention policy.

The daemon MUST NOT make raw results collectable before the summary that replaces them is committed and attested; an interruption between steps must leave more evidence than the contract requires, never less. A later read of a summarized artifact MUST yield `Redacted(summarized, commitment)`, MUST be reported in the omission manifest (INV-007), and claims that required the raw results downgrade per §18.4.

## Purge and the `Redacted` stub

Absorbed from plan §4.5.

Sensitive artifact classes are encrypted at rest per artifact. The daemon MUST declare which classes it treats as sensitive; §18.4's capture-time contract already fixes the minimum, since production trace payloads are recorded as salted commitments by default and any raw payload capture is encrypted per artifact on entry — that per-artifact encryption is what makes purge possible at all.

Purge shreds the artifact's key and replaces its content with a typed `Redacted(reason, commitment)` stub ([`../schemas/redacted.schema.json`](../schemas/redacted.schema.json)). Purge is therefore not deletion of a reference: the artifact's identity, its place in the evidence graph, and every edge into it survive. Concretely:

- a receipt referencing purged content MUST remain structurally verifiable and MUST report the redaction. It MUST NOT become unverifiable, and the reference MUST NOT be silently dropped — a receipt that quietly loses a reference is a forged receipt;
- claims that required the hidden data MUST downgrade per §18.4; a redacted input cannot support such a claim unless a trusted checker supplies a separate receipt;
- the redaction MUST appear in the omission manifest with reason `redaction` (INV-007).

`reason` is a closed three-value vocabulary — one value per way content stops being readable — and the daemon MUST NOT introduce a fourth:

| `reason` | Cause | Where specified |
|---|---|---|
| `summarized` | rolled up into an attested coverage certificate after promotion | [Storage lifecycle](#storage-lifecycle-attribution-and-summarization) |
| `purged` | key shredded by an explicit, audited purge operation | this section |
| `lost` | content absent after a restore or an fsck finding | [Backup and verified restore](#backup-and-verified-restore) |

Purge is a privileged operation and MUST be recorded in the audit log with actor, capability, the artifact class and identity purged, and the policy decision (§18.5). The audit record survives the content: what was purged, by whom, and under what authority is not itself purgeable.

The stub's serialized form carries two fields beyond the pair plan §4.5 names — `redacted: true` and `original_class`. Both are required by `redacted.schema.json` and by the IDL's `Redacted` struct, and both are load-bearing: `redacted: true` makes a stub distinguishable from a partially-populated artifact by structure rather than by inference, and `original_class` lets a reader know what class of evidence is missing without dereferencing anything (see [Corrections](#corrections)).

## Backup and verified restore

Absorbed from plan §4.5.

The CAS, the evidence ledger, the intent registry, and the audit log MUST all support backup and verified restore. A backup covering only the CAS is not a backup of the workbench: a receipt without its ledger entry has no status, and an intent registry without its audit log has no accountable history.

"Verified" is the operative word:

- a restore MUST run the index verifier before the restored store serves any request, and MUST report its per-class results;
- content the backup did not carry MUST surface as `Redacted(lost, commitment)` on read. The commitment is retained precisely so a lost artifact is still named and still checkable against a copy recovered later;
- a receipt whose referenced content was lost MUST NOT disappear and MUST NOT report as verifying. It verifies structurally, reports the loss, and its dependent claims downgrade per §18.4.

Audit logs are exportable as append-only streams (§18.5). An export MUST preserve append-only order, and MUST NOT be treated as a rotation mechanism that removes the original — an export is evidence for an external auditor, not a way to shorten the record.

## Incremental query database

The query engine resembles a self-adjusting computation graph but adds proof-oriented edge classes and evidence provenance. Every cached result records:

- inputs and dependency reasons;
- implementation/query version;
- validation receipt;
- reuse class;
- last clean-comparison outcome.

## Identity, capabilities, and cross-principal sharing

Absorbed from plan §4.5.

Remote mode requires an identity model. Capabilities (`cap_*`) are minted, scoped, delegated, and revoked through daemon operations, and every one of those operations MUST be recorded in the audit log with actor, capability, inputs, policy decision, outputs, and evidence identity (§18.5). What a capability confers is the IDL's `CapabilityDescriptor`: actor, authority level, snapshot and intent scope, artifact-class scope, expiry, and delegation depth. The administrative operations themselves are still absent from the plan §10.2 registry — RFC 0026's open questions track closing that before PR 5 — but their audit obligation is not conditional on that closure.

Possession of an artifact handle never implies authorization. Handles are content-derived and therefore derivable by anyone holding the content, so authorization MUST be checked below the adapter, independently of handle possession (plan §4.4, ADR-0037).

Two clients of the same principal requesting an identical task share computation freely. **Cross-principal sharing is off by default**, and the reason is precise: content-addressed dedup across principals is an existence oracle. Because an identity is derivable from content, a principal who can observe that publishing content X was a cache hit learns that some other principal already holds X — a disclosure about another principal's source, model, or counterexample obtained without ever reading it. Therefore:

- cross-principal dedup and cross-principal result reuse MUST be off unless an explicit sharing policy names both the sharing scope and the artifact classes in scope;
- with sharing off, publishing content that already exists under another principal MUST be indistinguishable, in result and in reported cost, from publishing content the store has never seen. The observable behavior is the contract: a dedup that is invisible in the result but visible in the cost ledger is still an oracle. Timing SHOULD be indistinguishable as well; a deployment that cannot bound the timing signal MUST declare that residual channel rather than imply it has closed it;
- a read of an artifact the caller is not authorized for MUST return the same typed refusal (`CapabilityDenied`) whether or not the artifact exists. A distinct not-found is itself the oracle;
- enabling sharing is a privileged, audited operation, and the sharing policy is a capability property — not a daemon-global flag, because a global flag cannot be scoped, delegated, or revoked.

## Deployment modes

### Embedded/local

A process-local or Unix-domain-socket daemon for `cargo continuum`; no external service required.

### Shared workstation

Multiple IDEs/agents run under per-user auth; each principal's sessions share immutable artifacts and computations freely among themselves. Artifacts and computations are shared across principals only under an explicit sharing policy — cross-principal sharing is not a default of this mode, and the existence-oracle rules that make it so are in [Identity, capabilities, and cross-principal sharing](#identity-capabilities-and-cross-principal-sharing).

### Remote organization

CAS and workers scale independently. Intent/evidence policy remains centralized. Sensitive source can use local extraction with remote proof over minimized artifacts. Remote workers receive only post-redaction minimized artifacts, and residency constraints are a capability property of the worker pool (§18.4).

## Epoch advance and two-epoch migration

Absorbed from plan §4.6. The decision and its rationale are [ADR-0018](../adr/0018-semantic-versioning-and-replay.md); this section is the daemon's obligations under it.

- An epoch advance MUST publish a typed per-artifact-class compatibility statement — `Preserved | Revalidate | Incompatible` — together with an estimated invalidation blast radius by class, **before** it is applied. On the wire that is the IDL's `EpochAdvanceNotice`; in the daemon it is `EpochAdvance` in `crates/continuum-value/src/epoch.rs`, which refuses an advance whose successor identity equals its predecessor. Advancing always yields a new identity.
- An advance MUST NOT mutate a published artifact and MUST NOT silently revalidate or invalidate a published receipt. A receipt remains verifiable under its pinned epoch indefinitely (INV-006, INV-014). Re-derived artifacts receive new identities linked to their predecessors by `SUPERSEDES` edges; nothing is rewritten in place.
- The daemon MAY hold at most two epochs of a kind concurrently during a migration. New work defaults to the newest. A continuation resumes only under its pinned epoch (§9.6) and is forked, never migrated in place: a resume under a different epoch MUST be rejected with `ContinuationEpochMismatch`, and an artifact or continuation declaring an epoch this daemon does not implement MUST be rejected with `EpochUnsupported` — never best-effort decoded (docs/09 T13).
- Epoch kinds MUST NOT be conflated. There are six independently versioned compatibility epochs — protocol, semantic, intent, evidence, proof, corpus — and neither the engine identity of §4.7 nor the `schema_epoch` of [`../schemas/README.md`](../schemas/README.md) is a seventh. Five of the six are content identities with no computable successor relation; only the protocol epoch is ordered, because the handshake selects the highest common version and serves majors N and N−1.

## Engine-defect lifecycle

Absorbed from plan §4.7.

Engine defects are first-class evidence, not incidents. The daemon MUST emit a `defect_*` artifact on every `ReplayDiverged`, parity mismatch (including the incremental parity audit of docs/42), engine crash, and explanation-validation failure. A `defect_*` MUST pin:

- every input by content identity;
- the semantic and checker epochs in force;
- the engine identity — plan §4.7 identity, carried in the result envelope's `epochs` block as `engine`, and not one of the six compatibility epochs;
- an automatically minimized reproduction.

A `defect_*` is governed by the same redaction policy as Context Packs (§18.4): minimization runs before export, and a field the policy withholds is reported as an omission, never dropped. `continuum doctor` assembles a shareable defect bundle from these artifacts; the bundle MUST be self-describing — every payload carries its `schema_id`/`schema_epoch` header ([`../schemas/README.md`](../schemas/README.md)) — so a recipient can check it without access to the originating daemon.

An engine defect MUST NOT be reported as a semantic verdict about the user's program. It is a typed statement about the engine; the task in which it arose reports typed inconclusiveness (INV-008), never a verdict the evidence does not support.

## Reliability

The daemon is itself verified through:

- asupersync Lab campaigns;
- model of task/publication state machine;
- crash recovery tests, including a crash injected between the content commit and the index commit of every publication phase, with fsck run on the survivor;
- backup/restore round-trips that drop content deliberately and check that the loss surfaces as `Redacted(lost, commitment)` rather than as a verifying receipt;
- linearizability checks for handle publication;
- Loom/weak-memory checks for local concurrent structures where appropriate;
- independent audit-log consistency checks.

## Observability

Operational telemetry is separate from semantic evidence. It includes task latency, cache behavior, worker health, queueing, resource use, and cancellation. A fast worker is not a correct worker; dashboards must not conflate them.

Daemon health and performance export as OpenTelemetry alongside — never inside — semantic evidence (§4.7). The separation is normative: a telemetry field MUST NOT appear in a receipt, an assurance envelope, or an evidence-graph node, and a semantic claim MUST NOT be derived from a metric. Storage attribution by artifact class (above) is reported on this lane too, and is the one place where operational reporting and the artifact classes of plan §4.4 legitimately meet — it reports how much space a class occupies, never what any artifact in it means.

## Corrections

Absorbing plan §4.5–§4.7 turned up four places where the plan's prose and the artifacts that implement it did not agree. Per plan §25 the absorbing document is corrected and becomes normative; the direction of each correction is recorded here.

| # | Disagreement | Direction | Resolution |
|---|---|---|---|
| 1 | Plan §4.5 gives the GC root set as "named roots, receipts, and retention policy"; this document has always also named live tasks. | plan §4.5 → docs/35 (widened) | The root set includes live tasks. A `Running`/`Suspended` task's pinned inputs and continuation must be reachable, or §4.5's own restart contract cannot be honored. |
| 2 | Plan §4.5's storage lifecycle (GC by reachability, per-artifact encryption, purge by key shred) reads as if it covered every plan §4.4 class, but `cap_*` is not content-addressed and `crates/continuum-workspace/src/artifact_path.rs` refuses to derive a path for it. | plan §4.5 → docs/35 (scoped) | §4.5's storage lifecycle governs the content-addressed classes. Capability tokens are governed by mint/scope/delegate/revoke plus the audit log; they are neither GC'd by reachability nor purged by key shred. |
| 3 | Plan §4.5 writes the stub as `Redacted(reason, commitment)`; `schemas/redacted.schema.json` and the IDL require `redacted: true` and `original_class` as well. | schemas/IDL → docs/35 (adopted) | The four-field form is normative. The plan's two-name spelling is the semantic pair, not the serialized shape; `redacted` makes a stub structurally recognizable and `original_class` names the missing evidence without a dereference. |
| 4 | Plan §4.7 and RFC 0026's result envelope both speak of an "engine epoch" beside the six compatibility epochs. | plan §4.7 → docs/35, ADR-0018 (renamed) | Engine identity is not a seventh compatibility epoch. It is §4.7 identity, carried in the `epochs` block as `engine` and pinned by continuations and `defect_*` artifacts (`crates/continuum-value/src/epoch.rs`, "Scope note"). |
