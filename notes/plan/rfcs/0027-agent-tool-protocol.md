# RFC 0027: Agent Tool Protocol

## Status
Draft for implementation.

**Target gate:** G2 (agent-computer interface); the authority ladder additionally carries the intent-integrity checks of G3 and the plan §24.5 workbench-security row's promotion gate
**Owners:** ACI leads
**Normative language:** MUST/MUST NOT/SHOULD/SHOULD NOT/MAY per RFC 2119.
**Normative wire artifact:** [`../schemas/continuumd-native-protocol.idl`](../schemas/continuumd-native-protocol.idl) carries the wire definition of every operation named here, including its `authority` clause and its annotations. Where this document and the IDL disagree about a **wire shape** — a field, a presence marker, a type, an enum member, an operation name, an annotation, or an authority level — the IDL decides and this RFC is corrected. Every such correction is recorded below under "Corrections recorded by this RFC". The 72 `authority` clauses and this document's registry table are held **token-identical** by `rule conformance.registry_agreement`: a generator or validator MUST fail closed on any disagreement rather than preferring either source.
**Companion normative sources:** [RFC 0026](0026-continuumd-native-protocol.md) (the request/result envelopes, the handshake, epochs, the task lifecycle, the error taxonomy, capability administration, and the delegation and expiry defaults), [RFC 0028](0028-context-pack-format.md) (the Context Pack a bounded result is delivered as), [RFC 0030](0030-incremental-semantic-query-engine.md) (query key and reuse), [RFC 0031](0031-semantic-and-intent-diff.md) (the classification a protected-intent change is decided by), [RFC 0032](0032-repair-transaction-protocol.md) (repair gates and promotion), [RFC 0037](0037-intent-contract.md) (contract fields, the closed policy verbs, the acceptance chain), [RFC 0038](0038-multi-agent-evidence-graph.md) (append-only writes and service-restricted status promotion), [RFC 0039](0039-explanation-engine.md) (explanation levels), [RFC 0029](0029-causal-verification-debugger.md) (`dbg_*` branches), [RFC 0035](0035-isolated-lean-proof-service.md) (`ps_*` proof state), [RFC 0033](0033-continuum-forge.md) (`forge_*` candidate pools), [RFC 0034](0034-continuum-bench.md) (benchmark tasks and graders), [`../plan.md`](../plan.md) §0.2, §4.3–§4.7, §5.4, §10, §11.4/§11.7, §18, §24.5, [`../docs/49_SECURITY_FOR_AUTONOMOUS_AGENTS.md`](../docs/49_SECURITY_FOR_AUTONOMOUS_AGENTS.md) (the capability matrix, the injection boundary, and the isolation controls), [`../docs/36_AGENT_PROTOCOL_AND_TOOL_CONTRACTS.md`](../docs/36_AGENT_PROTOCOL_AND_TOOL_CONTRACTS.md) and [`../docs/55_AGENT_API_REFERENCE_SKETCH.md`](../docs/55_AGENT_API_REFERENCE_SKETCH.md) (non-normative projections), [`../docs/34_DEVELOPER_EXPERIENCE_PRODUCT_CONTRACT.md`](../docs/34_DEVELOPER_EXPERIENCE_PRODUCT_CONTRACT.md) (the anti-goals), [`../docs/50_AGENT_EVALUATION_AND_REWARD_HACKING.md`](../docs/50_AGENT_EVALUATION_AND_REWARD_HACKING.md), [`../docs/09_THREAT_MODEL.md`](../docs/09_THREAT_MODEL.md), [`../schemas/README.md`](../schemas/README.md), [ADR-0036](../adr/0036-authoritative-workbench-daemon.md), [ADR-0037](../adr/0037-explicit-content-addressed-handles.md), [ADR-0038](../adr/0038-machine-contracts-not-terminal-prose.md), [ADR-0042](../adr/0042-mcp-adapter-not-authority.md).
**Landed vocabulary:** `crates/continuum-workspace/src/publication.rs` is the store-side implementation of this document's ladder and admission rules: `AuthorityLevel` (five variants whose declaration order *is* the ladder and whose `Ord` is "at least this level", with `Display` emitting the wire tokens), `ActorId`, `CapabilityToken` (no `Display`, an eliding `Debug`, and no content-derived store path), `CapabilityDescriptor`, `Action` with `Action::minimum_authority`, `AuthorizationPolicy`/`ScopedCapabilityPolicy` (the landed conjunction), `AuthorizationRecord`/`AuditLog`, and `ReferenceStore::{read, publish, mint, revoke}`. This RFC and that module MUST be revised together; neither may move alone. The store-side `CapabilityDescriptor` is a **subset** of the wire one and always has been — it carries the token, actor, level, and class scope that store-side authorization decides on, and not the instance scopes, `expires_at`, `delegation_depth`, or (from 3.1) `profile`, which are decided above it. Field-for-field identity between the two was never the obligation; what MUST hold is that no field the store *does* carry disagrees with the wire, and that a wire field the store starts enforcing arrives here in the same change.
**Frontier dependency:** the agent–computer interface is a **ratified** register row (plan §24.5, `quote-id=aci-benchmark-margins`, owned by [`../research/25-agent-computer-interfaces-for-formal-systems.md`](../research/25-agent-computer-interfaces-for-formal-systems.md), with the grading discipline of [`../research/33-agent-benchmarks-and-reward-hacking.md`](../research/33-agent-benchmarks-and-reward-hacking.md)). Workbench security is a second ratified row (`quote-id=workbench-security-promotion-gate`, owned by [`../research/35-security-of-agentic-verification-workbenches.md`](../research/35-security-of-agentic-verification-workbenches.md)), and this ladder is what that gate's authority checks enforce. The ratified sentences are the sole authority for their numbers; this RFC restates none of them and MUST NOT be read as relaxing any of them.

## Summary

The typed operation surface agents use, layered over RFC 0026. No agent workflow requires terminal parsing, cursor positions, or session reconstruction (plan B2, INV-003, ADR-0038).

This RFC is the normative home of: the five-level authority ladder and its order; the per-operation authority registry, one row per registered operation; the admission predicate a daemon evaluates before any semantic work; what a capability confers, how it is scoped, when it expires, how it delegates, and what revocation does; the agent-facing context policy and the closed expansion-relation vocabulary; deterministic ordering and result bounds as an agent contract; the handoff model and its stale-handle behavior; the privileged-operation boundary; and the prompt-injection boundary. Plan §10, plan §18.2, docs/36, and docs/55 are informal restatements; where they disagree with this document, this document governs (plan §25: "Where plan prose and RFC disagree, the RFC is corrected and becomes normative"). Every such correction is recorded below.

Four properties are load-bearing and are stated once here so nothing downstream re-derives them:

- **Authority is a property of the presented capability and of nothing else.** Not of the handle, not of the `actor` string, not of the tool list a client was shown, not of a call that previously succeeded, not of a configuration file, and never of a model's output. `ServerWelcome.grant` is the only channel through which a client learns what it may do (RFC 0026, ADR-0037).
- **No operation widens the authority of the connection invoking it.** This is the property that keeps capability administration out of the registry (RFC 0026, correction 20), and it extends to every surface in this document: no request, no delegation, no expansion, no recovery list, and no adapter may end with a principal able to do more than it could before.
- **Every admission test can only deny.** The ladder is an upper bound. Scope, privilege, and validity narrow it; nothing widens it. A daemon that cannot decide admission fails closed.
- **A bounded result is bounded in bytes and honest about what it dropped.** Trimming to a ceiling produces a typed omission; it never removes an assurance dimension, a warning, an epoch, a redaction stub, or an uncertainty. Optimizing agent token count by hiding uncertainty is a declared anti-goal (docs/34) and is prohibited here.

## Precedence

Four artifact kinds carry truth for this document, in a fixed order, plus one boundary with RFC 0026.

| Rank | Artifact | Governs |
|---|---|---|
| 1 | `schemas/*.schema.json` | the shape of an artifact instance carried in an `Opaque` field (INV-003) |
| 2 | [`../schemas/continuumd-native-protocol.idl`](../schemas/continuumd-native-protocol.idl) | every wire shape, including each operation's `authority` clause and annotations |
| 3 | this RFC | the meaning of the ladder, the admission predicate, and every obligation on an agent-facing surface |
| 4 | `plan.md`, `docs/*` | the map — informal restatements, corrected by ranks 1–3 |

- Rank 2 over rank 3 is the IDL's own header rule: a disagreement about *shape* — including an authority token — is resolved in the IDL's favour and recorded below. This RFC does not edit the IDL.
- Rank 3 over rank 2 holds in one direction only: where the IDL declares a shape but states no obligation, this RFC states the obligation and the IDL is not thereby wrong. An obligation the IDL *cannot* express is recorded below as a flag, never resolved by silently contradicting the IDL.
- **The RFC 0026 boundary.** RFC 0026 owns the envelopes, the handshake, epochs, ordering and idempotency, the task lifecycle, the error taxonomy, the operational contract, and capability administration. This RFC owns the ladder, the registry, admission, delegation as an agent contract, the context and expansion policy, handoff, and the injection boundary. Where the two overlap — the authority table, the `revise-intent` token, the byte-versus-token rule, the delegation defaults, the closed actor scheme — they MUST agree token for token and MUST be revised together.
- Rank 3 over rank 4 is plan §25.

## Versioning and revision

- **`AuthorityLevel` is a closed enum of exactly five members.** Adding, removing, or renaming a member, and changing any operation's authority level, are **breaking** changes under `rule versioning.breaking_change`: each MUST advance the protocol major *and* publish the typed per-artifact-class compatibility statement of plan §4.6 before it is applied. There is no compatible way to move an operation up or down the ladder.
- **The ladder is not versioned independently of the protocol.** It has no version of its own, no feature flag, and no per-deployment variant. A deployment narrows what it grants; it never redefines what a level means. A feature identifier MUST NOT widen authority (RFC 0026).
- **The registry carries exactly one row per registered operation, and the registry and the plan §10.2 list are the same set.** Adding an operation is a compatible (minor) change and MUST arrive with its authority row, its IDL declaration, and its plan §10.2 entry in one change (`rule conformance.registry_agreement`). A registry with a row the IDL lacks, or an IDL declaration with no row, is a defect in whichever artifact moved alone — never a licence for either to guess.
- **The wire token is `revise-intent`; the IDL identifier is `revise_intent`; the landed Rust variant is `AuthorityLevel::ReviseIntent`.** Prose and adapters use the wire token. An adapter that emits `revise_intent` on the wire is non-conforming.
- **Profile composition is deployment policy, not protocol.** Which privileged operations a capability profile admits, and which artifact classes it scopes to, are chosen per deployment. Changing them never changes `protocol_version`, and a change to an already-issued capability may only narrow it — widening is a new mint. As of protocol 3.1 the *shape* of that policy is protocol — `CapabilityProfile` — while its *contents* remain the deployment's; declaring the shape is what lets a client read what it holds instead of being told.
- **The protocol is at 3.1 as of this revision.** The single minor bump 3.0 → 3.1 (IDL 1.2) covers IDL 1.1's compatible fixes and this document's F1–F8 disposition together: `CapabilityDescriptor.profile`, `CapabilityProfile`, `DataGrant`, `ServerReject`, `ResultEnvelope.audit`, and four new `rule` blocks. Every one is compatible under `rule versioning.compatible_change`. Nothing in the ladder, the registry, or the admission predicate's *structure* moved: the levels are the same five, the rows the same 72, and the tests the same four — T3 gained a wire representation, not a new meaning.
- **`ExpansionRelation` is closed at eleven members.** Adding one is a breaking protocol change and a coordinated revision of this RFC and RFC 0028, whose pack artifact carries the same closed set.
- **Retiring a level is not a way to retire an operation.** An operation removed from the registry is removed from the IDL and from plan §10.2 in the same change; leaving it declared at an unreachable level would make `CapabilityDenied` indistinguishable from "no longer offered".

## The authority ladder

Five levels, totally ordered:

```text
read  <  propose  <  execute  <  revise-intent  <  promote
```

| Level | Wire token | Licenses | plan §18.2 capability | Operations |
|---|---|---|---|---:|
| 1 | `read` | reading published artifacts and compiling bounded views of them | "reading selected snapshots/artifacts"; "expanding context" | 18 |
| 2 | `propose` | publishing candidate artifacts — snapshots, correspondence links, repair transactions and their contents — none of which asserts anything about behavior | "proposing patches"; "creating repair transactions" | 8 |
| 3 | `execute` | starting, steering, resuming, and cancelling bounded semantic work | "starting bounded tasks"; "requesting proof work" | 40 |
| 4 | `revise-intent` | proposing, accepting, rejecting, or locking a change to a protected Intent Contract (INV-001, plan §5.4) | none — explicitly withheld from agents by default | 4 |
| 5 | `promote` | deciding a repair transaction's promotion or rejection (INV-011, INV-015) | none — explicitly withheld | 2 |

Nine rules govern the ladder. Each exists because the obvious conflation is wrong.

- **A1 — the order is total and a registry entry is a *minimum*.** A capability at level *L* satisfies the level test for every operation whose registry minimum is at or below *L*. `AuthorityLevel`'s derived `Ord` in `crates/continuum-workspace/src/publication.rs` is exactly this comparison, so "at least this level" is a comparison and not a table lookup.
- **A2 — every operation carries exactly one minimum level**, so the level component of a denial is decidable from the registry table alone, without reading the operation's implementation and without consulting the store. This is the property RFC 0026's capability-administration decision rests on.
- **A3 — authority and `@mutation` are orthogonal in both directions.** An operation may publish an immutable artifact at `read` authority when the publication is the daemon's own record of a read: `context.compile`, `context.expand`, `evidence.verify`, and `query.clean_compare` are `@mutation` at `authority read`. An operation may require more than `read` while changing nothing: `workspace.diff` is `@readonly` at `authority propose`. A client MUST NOT infer required authority from an annotation, and a daemon MUST NOT raise or lower the required level because of one.
- **A4 — a level is not a scope.** The level says *what kind* of act is permitted; the descriptor's `snapshots`, `intents`, and `artifact_classes` say *over what*. `execute` over no snapshots reaches nothing. Neither substitutes for the other, and a daemon MUST NOT compensate for a missing scope by raising a level, or the reverse.
- **A5 — no level confers what INV-015 withholds.** No capability at any level may alter an evidence-graph claim's status, sign a receipt, read a production trace it was not granted, or execute host effects. `promote` authorizes `repair.promote` and `repair.reject` and nothing else; the evidence-status write those operations cause is performed by a trusted service identity under RFC 0038's compare-and-set, not by the caller. There is no host-execution or network operation anywhere in the 72, so that prohibition is enforced by the registry's *shape*, not by a check.
- **A6 — agent-facing installations MUST omit `promote` and `revise-intent` from default capability profiles.** A task that must propose an intent revision is granted `revise-intent` explicitly and for that task (plan §5.4: agent repair capabilities exclude intent mutation "unless a task explicitly asks for redesign"), and the grant is itself audit-recorded.
- **A7 — no operation widens the authority of the connection invoking it.** No operation in the registry mints, scopes, delegates, or extends a capability; capability administration is out-of-band at protocol major 3 (RFC 0026, correction 20). A daemon MUST NOT let a result — an artifact, a handle, a `next_operations` entry, or a feature identifier — have the effect of widening a grant.
- **A8 — admission is decided below the adapter, before any semantic work.** The presented capability is checked independently of handle possession (ADR-0037, `rule authorization.independent_of_handles`) and before the store's index is consulted, so a denial is a function of the request alone. The landed `ReferenceStore::authorize` implements exactly this order and audits the decision either way.
- **A9 — the ladder is enforced, never advertised.** A client MUST NOT infer its authority from a successful call, a handle it holds, the size of a tool list it was shown, or a configuration file. Filtering a tool list to what a capability admits is good practice and is never the authority check.

## The admission predicate

A daemon admits a request if and only if **all four** of the following hold. Any failure is `CapabilityDenied`, returned before any semantic work runs.

```text
T1 (level)      registry_minimum(operation)  ≤  descriptor.level
T2 (scope)      every snapshot, intent, and artifact class the request names
                lies within descriptor.snapshots / .intents / .artifact_classes
T3 (privilege)  the operation is not @privileged, or the profile explicitly
                grants that operation to this capability
T4 (validity)   the capability is registered, unexpired, unrevoked, its actor
                matches the request's actor, and it is the connection's own
                capability or a descendant of it
```

- **The predicate is a conjunction and it fails closed.** T2, T3, and T4 can only deny; none of them admits an operation T1 rejects. RFC 0026's statement that `CapabilityDenied` is decidable from the authority table is therefore preserved exactly — it is a statement about T1 — and the ladder remains an upper bound on everything a capability can reach.
- **T1 is the registry table below**, read as a comparison on `AuthorityLevel`.
- **T2 is instance-scoped for snapshots and intents and class-scoped for everything else.** An empty list means unrestricted *within the level*, so a non-empty child list is always a narrowing. A request naming an artifact outside scope is denied, and the denial MUST NOT reveal whether the artifact exists.
- **T3 is what makes "proposal only" and "review only" expressible.** The five `@privileged` operations — `intent.accept`, `intent.reject`, `intent.lock`, `repair.promote`, `repair.reject` — are never admitted by level alone; RFC 0026's "never present in a default agent capability profile" is this test. `intent.propose_revision` is *not* `@privileged`, so a capability at `revise-intent` without privileged membership can propose a revision and cannot accept one — which is precisely docs/49's "revise intent: proposal only" cell, and it is unreachable if privilege is read as a level.
- **T3 is read off the wire as of protocol 3.1.** `descriptor.profile.privileged_operations` is the explicit grant, and `descriptor.profile.denied_operations` is the per-operation restriction a total order cannot express (docs/49's Reviewer cell, correction 22). An absent `profile` is the fail-closed reading — no privileged operation is granted — and an operation named in both lists is denied. Until 3.1 this test was prose and two identical descriptors could differ in whether they admitted `intent.accept`; that was F1, and it is paid. `data_grants` carries the requirements that are neither level nor privilege: `observe.ingest` requires `production_trace` in addition to `execute` (R-4), and a capability without it is denied even though its level admits the operation.
- **T4 binds actor to capability.** `RequestEnvelope.actor` MUST equal the `actor` of the `CapabilityDescriptor` for the capability that request presents, and `ClientHello.actor` MUST equal the actor of the connection's capability. A mismatch is `CapabilityDenied`, never `MalformedRequest`: an attempt to act as another principal is an authorization failure and MUST NOT be distinguishable from any other one. Because RFC 0026 scopes idempotency keys per actor as a security property, this binding is what makes that scoping meaningful.
  - **Placement, decided.** This rule's single normative home is here, in the admission predicate, and RFC 0026's "Connection lifecycle" carries an explicit pointer at step 3 (Authority), where the handshake half fires (RFC 0026, correction 41; correction 27 below). The criterion was which document a daemon implementer reads to build the connection state machine: they read RFC 0026, so RFC 0026 must name the check at the step it belongs to — and T4 is one conjunct of a conjunction whose other three conjuncts are this document's, so moving its text there would split T1–T4 across two documents and leave the per-request half, which fires outside the connection state machine entirely, either homeless or duplicated. A pointer satisfies the criterion; a move would not survive the boundary both documents declare in "Precedence".
- **"Narrower" has a definition.** RFC 0026 permits a request to present a narrower capability than the connection's and states no test. Normatively, a presented capability is admissible on a connection if and only if it is the connection's own capability or a descendant of it in the delegation tree, *and* its admission set — the set of (operation, scope) pairs satisfying T1–T3 — is a subset of the connection capability's. A daemon that cannot establish both MUST reject with `CapabilityDenied`.

## Capabilities: scope, expiry, delegation, revocation

A `cap_*` capability is the one plan §4.4 class that is not content-addressed. It is minted randomly, it confers authority, and it is therefore a secret: it MUST NOT be logged in request traces, MUST NOT appear in error text, MUST NOT appear in `next_operations` arguments, and has no content-derived store path (`crates/continuum-workspace/src/artifact_path.rs` refuses to derive one; `CapabilityToken` has no `Display` and an eliding `Debug`).

`CapabilityDescriptor` is what a capability confers. It has eight fields at protocol 3.0 and nine at 3.1:

| Field | Type | Presence | Meaning |
|---|---|---|---|
| `capability` | `CapabilityHandle` | required | the `cap_*` this describes |
| `actor` | `ActorId` | required | the principal every request under it MUST name (T4) |
| `level` | `AuthorityLevel` | required | the ladder position (T1) |
| `snapshots` | `list<WorkspaceHandle>` | required | instance scope; empty means unrestricted within `level` |
| `intents` | `list<IntentHandle>` | required | instance scope; empty means unrestricted within `level` |
| `artifact_classes` | `list<String>` | required | plan §4.4 class prefixes; empty means all |
| `expires_at` | `Timestamp` | nullable | null means non-expiring, and is constrained below |
| `delegation_depth` | `U32` | required | how deeply the holder may delegate; a number, not a flag |
| `profile` | `CapabilityProfile` | optional | 3.1; what the capability grants and withholds beyond level and scope (T3) — absent is the fail-closed reading, not "unrestricted" |

`CapabilityProfile` has four fields, and every one of them narrows:

| Field | Type | Presence | Meaning |
|---|---|---|---|
| `privileged_operations` | `list<OperationName>` | required | the `@privileged` operations this capability may invoke (T3). Absent from the list is denied however high the level; naming a non-`@privileged` operation grants nothing |
| `denied_operations` | `list<OperationName>` | required | operations withheld whatever the level permits — docs/49's Reviewer cell (correction 22). An operation in both lists is denied |
| `data_grants` | `list<DataGrant>` | required | requirements beyond authority: `production_trace` is the plan §18.2 grant `observe.ingest` needs in addition to `execute` (R-4) |
| `cross_principal_sharing` | `Bool` | required | whether this capability's publications may be deduplicated against another principal's content — the sharing policy RFC 0026 requires to be a capability property |

- **The profile can only subtract.** A `privileged_operations` entry admits a privileged operation only where T1 and T2 already admit it, so no entry reaches an operation the level denies; a `denied_operations` entry denies unconditionally; a `data_grants` member satisfies an additional requirement and never substitutes for a level. An absent profile is exactly equivalent to one with empty lists and `cross_principal_sharing` false, and a daemon MUST NOT read absence as "unrestricted" (`rule capability.profile_narrowing`).
- **`DataGrant` is `@open`, and that is the fail-closed direction for a grant.** A client that receives a member it does not recognize MUST NOT infer semantics from it — for a grant, that means it MUST NOT assume it holds one — so the extensibility and the safety point the same way, and adding a grant stays a compatible change (RFC 0026, "Versioning and revision").
- **What the profile does not do.** It is not a per-deployment redefinition of a level (A1, "Profile composition is deployment policy"), it is not a sixth admission test — it is how T3 is read — and it is not a capability-administration surface: a profile is *reported* in `ServerWelcome.grant` and is set out of band like everything else about a `cap_*`.

**Administration is out-of-band at protocol major 3** (RFC 0026, correction 20). Minting, scoping, delegating, and revoking are not operations in the 72, and the reason is A7: a mint reachable from an agent connection is one operation away from a wider grant. The obligations below hold wherever that surface lives. Its landed form is `ReferenceStore::mint` and `ReferenceStore::revoke` under `Action::Administer`, which audits every decision and refuses to mint above the minting capability's own level; `Action::minimum_authority` places `Administer` and `Audit` at `promote`, which is a **store-side floor for an out-of-band action, not a sixth ladder level and not a registry entry**.

### Expiry

- **E1** — expiry is evaluated **per request**, not at handshake. A daemon MUST NOT serve a connection from a grant cached at handshake time; the first request after expiry fails `CapabilityDenied`.
- **E2** — `expires_at: null` means non-expiring and MUST be confined to deployments where daemon and client share a trust domain (local IPC, a single principal). In remote mode a non-expiring grant MUST NOT be issued, and a mint naming no expiry MUST be rejected rather than defaulted to non-expiring.
- **E3** — expiry MUST NOT retroactively invalidate anything. Artifacts the capability lawfully produced remain published and readable under a capability that still authorizes them (INV-009).

### Delegation

Delegation is how a coordinator hands work to sub-agents. It is fail-closed in every dimension.

- **D1** — `delegation_depth` defaults to `0`: no delegation unless a grant says otherwise.
- **D2** — a delegated capability's `delegation_depth` MUST be strictly less than its parent's.
- **D3** — a child's `level` MUST be at or below its parent's.
- **D4** — a child's `snapshots`, `intents`, and `artifact_classes` MUST be subsets of its parent's. An empty parent scope means unrestricted, so a non-empty child scope is always a narrowing.
- **D5** — a child's `expires_at` MUST NOT be later than its parent's, and a non-expiring child of an expiring parent is malformed.
- **D6** — a child's privileged-operation set (T3) MUST be a subset of its parent's. A parent that cannot promote cannot delegate promotion. As of 3.1 this is a comparison of wire fields rather than of deployment folklore: a child's `profile.privileged_operations` and `profile.data_grants` MUST be subsets of its parent's, its `profile.denied_operations` MUST be a superset, and its `cross_principal_sharing` MUST be false wherever the parent's is (`rule capability.profile_narrowing`). A child of a parent with no profile MUST have no profile grants either.
- **D7** — composing D3–D6, **a child's admission set MUST be a subset of its parent's**. This is the checkable form of A7 for handoff, and it is what a conformance suite tests rather than testing the field rules one at a time. Before 3.1 it was checkable for three of the four tests and asserted for the fourth, because T3 had no wire representation to compare; that was F1's practical cost, and paying it is what makes D7 a computation.
- **D8** — revoking a capability revokes its delegation subtree. A child grant MUST NOT outlive its parent.
- **D9** — every mint, scope, delegate, and revoke MUST be recorded in the audit log with actor, capability, inputs, policy decision, outputs, and evidence identity (plan §4.5, §18.5). The audit record survives the capability: what was granted, by whom, and under what authority is not itself revocable.

### Revocation

- **R1** — revocation takes effect on in-flight connections, at the next request (E1's rule).
- **R2** — revocation MUST NOT delete or invalidate any published artifact and MUST NOT retroactively unmake a result the capability lawfully produced (INV-009). A task already running under a revoked capability is cancelled or completed per deployment policy; its results are readable only under a capability that still authorizes them.
- **R3** — revocation, not purge, ends a capability's life (docs/35). `cap_*` is not content-addressed, is not garbage-collected by reachability, and is not purged by key shred. Revoking a token that was never registered MUST succeed silently, so that revocation cannot be used to probe the registry.

## The operation authority registry

The registry is the plan §10.2 list: **72 operations in 18 namespaces**, no more and no fewer. Each row carries exactly one minimum authority level (A2). The `Authority` column is token-identical with the corresponding IDL `authority` clause, allowing for the `revise-intent`/`revise_intent` spelling of one token; the `Annotations` column reproduces the IDL's annotations, which bind the envelope obligations RFC 0026 states. Signatures, request and response structs, and per-operation error sets live in the IDL.

| Operation | Authority | Annotations |
|---|---|---|
| `workspace.create` | propose | `@mutation` |
| `workspace.fork` | propose | `@mutation` |
| `workspace.diff` | propose | `@readonly` `@task_starting` |
| `workspace.seal` | propose | `@mutation` |
| `intent.get` | read | `@readonly` |
| `intent.diff` | read | `@readonly` |
| `intent.propose_revision` | revise-intent | `@mutation` |
| `intent.accept` | revise-intent | `@mutation` `@privileged` `@audit_recorded` |
| `intent.reject` | revise-intent | `@mutation` `@privileged` `@audit_recorded` |
| `intent.lock` | revise-intent | `@mutation` `@privileged` `@audit_recorded` |
| `verification.start` | execute | `@mutation` `@task_starting` |
| `verification.result` | execute | `@readonly` |
| `verification.await` | execute | `@readonly` `@task_starting` |
| `model.check` | execute | `@mutation` `@task_starting` |
| `model.explore` | execute | `@mutation` `@task_starting` |
| `model.compare` | execute | `@readonly` `@task_starting` |
| `program.extract` | execute | `@mutation` `@task_starting` |
| `program.run` | execute | `@mutation` `@task_starting` |
| `program.replay` | execute | `@mutation` `@task_starting` |
| `refinement.check` | execute | `@mutation` `@task_starting` |
| `refinement.explain` | execute | `@readonly` |
| `proof.goal` | execute | `@readonly` |
| `proof.attempt` | execute | `@mutation` `@task_starting` |
| `proof.check` | execute | `@mutation` `@task_starting` |
| `proof.slice` | execute | `@readonly` `@paginated` |
| `correspondence.bind` | propose | `@mutation` |
| `correspondence.status` | read | `@readonly` |
| `correspondence.drift` | read | `@readonly` |
| `debug.open` | execute | `@mutation` |
| `debug.state` | execute | `@readonly` |
| `debug.enabled` | execute | `@readonly` `@paginated` |
| `debug.step_event` | execute | `@mutation` |
| `debug.step_abstract` | execute | `@mutation` |
| `debug.reverse_causal` | execute | `@mutation` |
| `debug.branch` | execute | `@mutation` |
| `debug.compare` | execute | `@readonly` |
| `debug.why_enabled` | execute | `@readonly` |
| `debug.why_blocked` | execute | `@readonly` |
| `debug.export` | execute | `@mutation` |
| `context.compile` | read | `@mutation` `@task_starting` |
| `context.expand` | read | `@mutation` `@task_starting` |
| `failure.explain` | execute | `@readonly` `@task_starting` |
| `failure.minimize` | execute | `@mutation` `@task_starting` |
| `failure.branch` | execute | `@mutation` `@task_starting` |
| `repair.begin` | propose | `@mutation` |
| `repair.apply` | propose | `@mutation` |
| `repair.attach` | propose | `@mutation` |
| `repair.evaluate` | execute | `@mutation` `@task_starting` |
| `repair.resume` | execute | `@mutation` `@task_starting` |
| `repair.review` | read | `@readonly` |
| `repair.promote` | promote | `@mutation` `@privileged` `@audit_recorded` |
| `repair.reject` | promote | `@mutation` `@privileged` `@audit_recorded` |
| `observe.ingest` | execute | `@mutation` `@task_starting` `@audit_recorded` |
| `observe.classify` | read | `@readonly` |
| `observe.result` | read | `@readonly` |
| `forge.create` | execute | `@mutation` `@task_starting` |
| `forge.step` | execute | `@mutation` `@task_starting` |
| `forge.archive` | execute | `@readonly` `@paginated` |
| `forge.materialize` | execute | `@mutation` |
| `benchmark.run` | execute | `@mutation` `@task_starting` |
| `task.status` | read | `@readonly` |
| `task.cancel` | execute | `@mutation` |
| `task.resume` | execute | `@mutation` `@task_starting` |
| `task.subscribe` | read | `@readonly` `@streaming` |
| `task.update_budget` | execute | `@mutation` |
| `evidence.get` | read | `@readonly` |
| `evidence.query` | read | `@readonly` `@paginated` |
| `evidence.verify` | read | `@mutation` `@task_starting` |
| `evidence.subscribe` | read | `@readonly` `@streaming` |
| `query.explain_reuse` | read | `@readonly` `@paginated` |
| `query.explain_invalidation` | read | `@readonly` `@paginated` |
| `query.clean_compare` | read | `@mutation` `@task_starting` |

Distribution, which a conformance test derives from the table rather than asserting beside it:

| Authority | Count | Annotation | Count |
|---|---:|---|---:|
| `read` | 18 | `@readonly` | 28 |
| `propose` | 8 | `@mutation` | 44 |
| `execute` | 40 | `@task_starting` | 26 |
| `revise-intent` | 4 | `@paginated` | 6 |
| `promote` | 2 | `@streaming` | 2 |
| **total** | **72** | `@privileged` | 5 |
| | | `@audit_recorded` | 6 |

Five registry rules:

- **R-1** — a namespace has no authority of its own. `intent` spans two levels and `repair` spans four; the row decides, never the prefix.
- **R-2** — a namespace prefix is not a capability scope. Scope is expressed in artifact classes and instances (T2), which cut across namespaces: a `dbg_` scope bears on eleven `debug` operations and on `context.compile`'s debugger-branch field alike.
- **R-3** — the table is total. There is no default, no fallback level, and no "unlisted operations are `execute`". An operation absent from this table is absent from the protocol.
- **R-4** — `@privileged` implies `@audit_recorded`; the converse does not hold. `observe.ingest` is audit-recorded without being privileged, because trace ingestion is an ordinary `execute` operation that additionally requires the production-trace capability of plan §18.2 (with the capture-time contract of plan §18.4) and must leave a record.
- **R-5** — operations registered ahead of their producing subsystem — the `observe` family, and any operation whose lane has not shipped — MUST fail with the typed `UnsupportedSemanticFeature` (`rule errors.unsupported_surface`). They MUST NOT degrade, guess, or return an empty success, and their authority row is in force from registration: an unshipped operation is denied for want of authority before it is refused for want of an implementation.

### Why each level lands where it does

- **`workspace` is `propose` as a family, including `workspace.diff`.** Creating, forking, and sealing publish candidate snapshots. `workspace.diff` neither mutates nor publishes a snapshot, yet sits at `propose` because it is a snapshot-family verb whose result carries a `PolicyVerdictValue` over two snapshots the caller must already be entitled to propose against. A `read`-only reviewer therefore reaches diffs through `repair.review`, whose response carries `semantic_diff` at `read` (RFC 0031, RFC 0032), and not through `workspace.diff`. Whether the level should fall to `read` at the next protocol major is an open question below; it is a breaking change and is not taken here.
- **`intent` spans `read` and `revise-intent` with nothing between.** `get` and `diff` are inspections. All four mutating verbs are `revise-intent` because each touches protected intent (INV-001); the difference between proposing and accepting is T3, not T1.
- **`context` is `read` although both operations are `@mutation @task_starting`.** Reading is enough to compile a pack, and compiling one MUST NOT become a way to reach data the capability's scope excludes (C1). The published `ctx_*` is the daemon's own record of a read.
- **`evidence` is entirely `read`, including `evidence.verify`.** Verification re-checks a receipt against artifacts; it establishes nothing new and grants nothing. `evidence.subscribe` streams *committed* graph deltas and is therefore also a read. No `evidence` operation writes a status: status promotion is service-restricted (RFC 0038) and has no wire verb at all.
- **`query` is entirely `read`, including `query.clean_compare`.** The parity audit publishes a comparison and, on mismatch, a `defect_*`; both are records of reads, and the audit MUST NOT prefer either lane's result (RFC 0030).
- **`repair` spans four levels because a repair transaction has four distinct acts.** Composing the transaction is `propose`; running its gates is `execute`; reading the reviewer projection is `read`; deciding it is `promote`. `repair.review` resolves to `read` deliberately: the review decision it records enters the evidence graph as an audit-recorded append attributed to the named reviewer (RFC 0032), not through agent evidence-write authority.
- **`task` splits at the boundary between observing and steering.** `status` and `subscribe` are `read`; `cancel`, `resume`, and `update_budget` are `execute` because each changes what a running campaign will do. `task.update_budget` carries plan §4.3's mid-flight budget requirement: lowering a dimension below committed spend suspends with a continuation rather than truncating (INV-009, RFC 0026).
- **`observe` splits between ingestion and inspection**, and `correspondence` splits the same way: `bind` creates or updates a plan §16 correspondence link at patch-proposal authority, while `status` and `drift` are read-only inspections.
- **`benchmark.run` is `execute` and never self-scores.** Graders are named by identity and the daemon MUST reject any grader not registered for the task. Hidden tasks, hidden variants, and graders live outside agent-readable snapshots (research/33), so T2 is what keeps them unreachable; the operation returns evidence, and no capability level lets a caller mark its own run passing.

## Reconciliation with plan §10 and plan §18.2

Each plan bullet has exactly one normative home. A bullet with no home is a defect in one of the two documents, never a licence to invent a rule.

**plan §10.1 — the twelve ACI design principles.**

| plan §10.1 bullet | Normative home |
|---|---|
| small orthogonal operations | the registry: 72 operations in 18 namespaces, one act each; `rule task.no_generic_start` forbids the omnibus verb |
| typed arguments and results | the IDL request/response structs; INV-003; `rule envelope.no_prose` |
| stable handles instead of cursor positions | plan §4.4 handles; ADR-0037; H3 |
| deterministic ordering | B1–B3; `rule ordering.deterministic` |
| concise default payloads | B8: one Context Pack plus `next_operations` |
| explicit expansion | the closed eleven-member relation vocabulary |
| error messages with recovery actions | N1–N3; `Error.recovery` is required and typed (RFC 0026) |
| idempotency | `idempotency_key` on every `@mutation`, per-actor scoping bound to T4 (H4) |
| budget and cancellation controls | `budget` on every `@task_starting`; `task.cancel` and `task.update_budget` at `execute` |
| no need to parse terminal prose | ADR-0038; every agent-visible fact is a typed field |
| **no implicit authority escalation** | A7 and T4; capability administration out-of-band (RFC 0026, correction 20) |
| source text treated as data, not instructions | S1–S7; INV-016 |

**plan §18.2 — the six agent capabilities and the four denials.**

| plan §18.2 item | Ladder position |
|---|---|
| reading selected snapshots/artifacts | `read`, scoped by `snapshots` and `artifact_classes` |
| expanding context | `read` (`context.compile`, `context.expand`) |
| proposing patches | `propose` (`repair.apply`, `repair.attach`, the `workspace` family) |
| creating repair transactions | `propose` (`repair.begin`) |
| starting bounded tasks | `execute` (the 26 `@task_starting` operations, each subject to its own row) |
| requesting proof work | `execute` (`proof.goal`, `proof.attempt`, `proof.check`, `proof.slice`) |
| *denied:* evidence-signing | A5: no level; RFC 0038's service identities |
| *denied:* intent-policy mutation | `revise-intent` plus T3, omitted by default (A6) |
| *denied:* arbitrary network | no such operation exists in the registry |
| *denied:* host execution | no such operation exists in the registry |

**plan §10.4 — the MCP adapter's seven obligations**, which are adapter-independent and bind every adapter (ADR-0042).

| plan §10.4 bullet | Normative home |
|---|---|
| validates auth independently of handle possession | A8; `rule authorization.independent_of_handles` |
| bounds result sizes | B4–B6 |
| exposes schemas and deterministic tool ordering | TS-1 and TS-3 |
| avoids session-scoped semantic state | INV-002; H3 |
| maps resources to immutable artifacts | plan §4.4 content-addressed artifacts; TS-6 |
| attaches trace context | `RequestEnvelope.trace` (W3C Trace Context) |
| supports client-independent task handoff | H1–H6 |
| no prompt or resource may mutate evidence status | A5 — not an adapter rule: no operation in the 72 sets a status |

**docs/49's capability matrix**, made enforceable. The matrix is a role sketch written per verb; the ladder is a total order, so a cell granting a higher-level verb while denying a lower-level one is not expressible as a level and MUST be expressed as scope or as privileged membership.

| docs/49 role | Level | Privileged (T3) | Scope narrowing (T2) |
|---|---|---|---|
| Modeler | `execute`; `revise-intent` only for an explicit redesign task | none | granted snapshots; `model_`, `cir_`, `ctx_`, `ev_` |
| Repairer | `execute` | none | granted snapshots; `rt_`, `ctx_`, `crash_`, `dbg_` |
| Prover | `execute` | none | scoped snapshots; `proof_`, `ps_`, `ctx_` |
| Reviewer | `read`, plus `execute` where the deployment needs review-time evaluation | none | granted snapshots; `rt_`, `diff_`, `receipt_`, `ctx_` |
| Trusted promoter | `promote` | `repair.promote`, `repair.reject`, and — where policy allows — the three privileged `intent` verbs | as deployed |

The Privileged and Scope columns are `profile.privileged_operations` and the descriptor's three scope lists; as of protocol 3.1 the whole triple is a wire value a deployment issues and a client reads back in `ServerWelcome.grant`, rather than a table a deployment is trusted to have implemented.

Two cells did not survive the translation and are recorded as corrections below. "Revise intent: proposal only" is expressible only through T3, never through a level. The Reviewer's "start bounded task yes / propose no" is not expressible as a (level, scope) pair, because `execute` subsumes `propose` and class scope cannot separate reading a class from writing it — but it *is* expressible at 3.1 as `execute` plus `profile.denied_operations = [workspace.create, workspace.fork, workspace.seal, repair.begin, repair.apply, repair.attach, correspondence.bind]`, which is the eight `propose` rows minus `workspace.diff`, the one a reviewer wants. That is what `denied_operations` was added for (F1); the enumeration is a deployment's to make and is written here as the worked example, not as a fixed profile.

## Tool schemas and adapter surfaces

Every agent-visible tool — MCP, CLI, LSP, DAP, or any other — is a **projection of exactly one registry operation** (ADR-0042, `rule conformance.adapter_parity`).

- **TS-1 — a tool's input schema is generated from that operation's request struct, and its output schema from the response struct.** An adapter MUST NOT author a field, MUST NOT merge two operations into one tool, MUST NOT add a semantic argument, and MUST NOT reinterpret a verdict.
- **TS-2 — an adapter MUST NOT expose a tool with no declared operation behind it.** Renaming is permitted as a spelling, only under a **declared, total mapping** from adapter name to `OperationName` (RFC 0026, correction 23). A name whose mapping target does not exist is the violation.
- **TS-3 — the tool list MUST be deterministically ordered, lexicographically by `OperationName`.** Two listings under the same protocol version, adapter mapping, and capability MUST be byte-identical, so a client can cache and diff the surface it is offered. Ordering by "relevance", by recency, or by anything derived from a snapshot's contents is prohibited: it turns the surface into a channel.
- **TS-4 — a tool list MAY be filtered to the presenting capability's admission set, and MUST NOT be the authority check.** Filtering is a usability property; A8's check runs regardless. An unfiltered adapter is not thereby a security defect, while a filter-only adapter is.
- **TS-5 — tool descriptions are static text derived from the IDL's doc comments.** A daemon or adapter MUST NOT interpolate source, comments, logs, model text, production payloads, artifact contents, or any snapshot-derived string into a tool description, a parameter description, or an enum label (INV-016). Tool metadata is part of the trusted instruction channel and is never a projection of user data.
- **TS-6 — resources are immutable artifacts.** An adapter MAY expose a `ctx_*`, `crash_*`, `receipt_*`, or other published artifact as a read-addressed resource; it MUST NOT expose a mutable view, a live cursor, or a session-scoped document, and a resource read is an operation subject to T1–T4 like any other.
- **TS-7 — an adapter has no protocol version of its own**, because it defines no semantics. It reports the negotiated `protocol_version` and its own software identity, and nothing else.

## Deterministic ordering and result bounds

- **B1 — every list a result returns is deterministically ordered** by content identity or by an explicitly declared sort key, and two identical requests against the same snapshot and epochs return **identical bytes** under the same encoding (`rule ordering.deterministic`). Byte-identity, not set-identity, is the contract: it is what lets an agent cache by request and what makes a golden trace meaningful.
- **B2 — ordering is a property of the result, not of the client.** A daemon MUST NOT order by inferred relevance to the caller, by a model-supplied hint, or by anything not derivable from the pinned inputs.
- **B3 — pagination belongs to the six `@paginated` operations only.** Other list-returning operations are bounded by `output_policy`, not by a cursor. A `page_token` minted under different epochs MUST be rejected with `EpochUnsupported`, and the concatenation of all pages is the full deterministic ordering with no duplicate and no gap.
- **B4 — bytes are the enforced contract; tokens are advisory.** `output_policy.max_bytes` is enforced; `max_tokens` is tokenizer-relative and advisory, and a reported token count MUST carry its `tokenizer_id`. A token count without its tokenizer identity is not a measurement.
- **B5 — `output_policy` narrows and never widens.** It is `optional`; when absent, the daemon's own `ServerLimits.max_result_bytes` ceiling applies. A policy naming a ceiling above the server limit is clamped to the limit, and the clamp is visible in the omission manifest rather than silent.
- **B6 — trimming produces a typed omission and never removes the truth.** An `Omission` entry (INV-007) MUST be emitted, and trimming MUST NOT drop an assurance dimension, a warning, an epoch identity, a redaction stub, or an `inconclusive_reason`. Result schemas always carry omissions and typed uncertainty; hiding uncertainty to save bytes or tokens is prohibited (docs/34 anti-goal, `rule envelope.assurance_required`).
- **B7 — `audience` is a rendering hint.** It never changes a verdict, never widens authority, and never suppresses an omission or an uncertainty.
- **B8 — the default failure result is one Context Pack plus `next_operations`.** Everything else is reachable by expansion, never by dumping. A daemon MUST NOT substitute a raw dump for the pack when the budget is small; it compiles a smaller pack with a larger manifest (RFC 0028).

### `next_operations` and recovery are capability-relative

- **N1 — `next_operations` entries are structs, not names**: each carries `operation` *and* `arguments` pre-filled against that operation's request struct, plus an optional typed `rationale`. A list of bare operation names is not a recovery surface, because a client cannot execute it as given.
- **N2 — a daemon MUST NOT offer an operation the presenting capability would deny.** `next_operations` and `Error.recovery` are computed against the request's capability, so a recovery surface never widens authority (A7) and never advertises the existence of a privileged path to a principal that does not hold it. In particular a `@privileged` operation never appears in either list for a connection whose profile does not grant it under T3.
- **N3 — recovery is the only recovery channel.** `Error.recovery` is required, and an empty list is the typed statement that no recovery exists from this state. It is never free-form commands, never shell, and never interpolated source, log, or model text (INV-016).

## Context policy and the expansion vocabulary

Requests carry `output_policy`; results carry Context Packs (RFC 0028). Expansion is relation-based over the evidence, causal, proof, and source graphs, and each expansion returns a **new immutable pack referencing its parent**.

`ExpansionRelation` is a **closed** enum with exactly eleven members. A daemon MUST NOT emit a member outside it; a client reading an unrecognized member MUST treat the message as malformed (`MalformedRequest`) and MUST NOT infer semantics.

| Relation | Traverses from the anchor to |
|---|---|
| `causal_predecessors` | events that must precede it in the causal order (RFC 0001) |
| `causal_successors` | events it must precede |
| `conflicts_with` | the conflicting alternatives that selected this branch |
| `same_owner` | events and obligations of the same task or node |
| `property_automaton_step` | the property-automaton transitions this event drives |
| `proof_dependency` | the proof obligations and declarations this item depends on |
| `source_span` | the source spans corresponding to it |
| `assumption_uses` | the assumptions this item consumes |
| `abstraction_of` | the abstract element this concrete item projects to |
| `refinement_of` | the concrete elements refining this abstract item |
| `alternate_branch` | the sibling safe branches at this choice point |

- **C1 — expansion never widens scope.** A child pack's contents MUST lie within the requesting capability's scope (T2). Compiling or expanding a pack MUST NOT become a path to data the capability excludes; where a relation would cross the scope boundary, the crossing is reported as an omission, not followed.
- **C2 — the question is part of the identity.** For `context.expand` the pack's `question` is the canonical encoding of the triple (relation, anchor, depth), so two expansions selecting the same items are distinct artifacts answering different questions (RFC 0028).
- **C3 — a node ceiling is `output_policy.max_nodes`**, not a `Budget` dimension. `budget` is the envelope field every `@task_starting` operation requires, and both context operations are `@task_starting`; the two are not interchangeable.
- **C4 — expansion is the only completeness mechanism.** A result that cannot fit its ceiling names what it omitted and how to retrieve it. Dumping is not a fallback, and neither is silently returning less.

## Handoff and multi-agent operation

A coordinator hands agents an explicit sharing model (plan §10.5) rather than one session boundary that either over-shares or isolates the wrong state:

| Shared | Isolated |
|---|---|
| the `ws_*` snapshot | `dbg_*` debugger branches |
| the `in_*` Intent Contract | `ps_*` proof search state |
| evidence-graph read scope (`ev_*`) | `forge_*` candidate pools |

- **H1 — sharing and isolation are expressed as capability scope.** Shared state is the `snapshots` and `intents` instance scope plus the `ev_` artifact class; isolated state is the `dbg_`, `ps_`, and `forge_` classes, narrowed per sub-agent. Because `artifact_classes` is class-scoped and not instance-scoped, per-instance isolation between two sub-agents holding the same class is enforced by the daemon from the creating capability's actor until that gap is paid (F2).
- **H2 — each sub-agent gets its own delegated capability**, subject to D1–D9. A coordinator MUST NOT share one capability among sub-agents: a shared bearer token destroys per-actor idempotency scoping, audit attribution, and selective revocation simultaneously.
- **H3 — handles thread through calls and there is no session affinity.** Any client holding the handles and a valid capability can continue the work. A dropped connection changes nothing (INV-002); re-reading a handle recovers every recoverable fact.
- **H4 — idempotency keys are scoped per actor**, and replaying another actor's key MUST behave as an unused key, never as `IdempotencyKeyReused` (RFC 0026). A key that could collide across principals is an existence oracle, and in a swarm it is also a way for one sub-agent to observe another's progress.
- **H5 — sub-agents append; they do not adjudicate.** Every sub-agent may publish candidate nodes and edges into the evidence graph under `propose`; none may promote a claim's status. Racing promotions are linearized by compare-and-set per claim identity, a lost race returns the typed `StatusConflict`, and contradictory claims materialize a `Conflict` node rather than resolving by write order (RFC 0038, plan §11.7). Agent votes and confidence never change status.
- **H6 — a subscription is not a lock and not a transaction.** Reconnecting and re-reading MUST yield a superset of what a stream delivered; a daemon MUST NOT rely on a client having observed an event.

### Stale handles and stale capabilities

Staleness is typed, and the five outcomes MUST NOT be collapsed (INV-008):

| Condition | Result |
|---|---|
| the named snapshot is not current, or is not sealed where sealing is required | `StaleSnapshot` |
| a continuation's pinned epoch or engine identity disagrees with the daemon's | `ContinuationEpochMismatch` |
| an artifact, page token, or continuation declares an epoch the daemon does not implement | `EpochUnsupported` |
| the capability expired, was revoked, or does not scope the named artifact | `CapabilityDenied` |
| the artifact does not exist, and the caller was not authorized for it either way | `CapabilityDenied` |

- **H7 — possession of a stale handle never confers authority**, and losing authority never changes a handle's meaning. The two are independent: a handle names content, a capability authorizes acts (ADR-0037).
- **H8 — a stale-handle failure is recoverable and says so.** Every row above carries a `recovery` list computed under N2, so "re-seal", "re-base", "re-page under the current epoch", or "resume under the pinned epoch" is executable rather than described.

## Privileged operations and the promotion boundary

Five operations are `@privileged` (`intent.accept`, `intent.reject`, `intent.lock`, `repair.promote`, `repair.reject`) and six are `@audit_recorded` (those five plus `observe.ingest`).

- **P1 — a privileged operation is never inferred.** It is admitted only when T1–T4 all hold, and T3 is an explicit grant. A daemon MUST NOT treat a preceding successful call, a returned handle, a gate result, or an evidence state as evidence that the caller may promote.
- **P2 — model output is not an operation.** An adapter MUST NOT synthesize a call from model text, and the daemon decides on the presented capability alone. Text that looks like a tool call, a capability, or an approval is data (INV-016).
- **P3 — promotion is a service decision, not an agent assertion** (INV-015, B12). `repair.promote` re-computes the semantic diff and its policy verdict server-side; a client-supplied or cached verdict is never trusted (RFC 0031, RFC 0032), and the evidence-status write is performed under a trusted service identity (RFC 0038).
- **P4 — `intent.accept` is the only transition from `Proposed` to protected status** (plan §5.4). There is no draft status. `intent.lock` edits the plan §5.4 field-to-verb policy table from RFC 0037's closed verb set; it does not set a boolean.
- **P5 — every privileged call is audited with actor, capability, inputs, policy decision, outputs, and evidence identity** (plan §18.5), and so is every *denial*, including the denial of an unregistered or revoked token — the landed `ReferenceStore::authorize` records the decision before the store's index is consulted, with a null actor when the token is unknown. A block that leaves no audit record is indistinguishable from an attack that was never tried, and fails its case in the ratified workbench-security corpus.
- **P6 — privileged operations are structurally distinct in every rendering.** An adapter MUST make them distinguishable from ordinary ones and MUST NOT present a privileged call as a routine next step; N2 is the checkable form of that rule.

## Safety: untrusted content and the injection boundary

Source code, comments, documentation, logs, model labels, counterexample payloads, and production messages are untrusted data (INV-016, docs/49, docs/09).

- **S1 — untrusted content is never interpolated into an instruction channel.** Tool descriptions, parameter descriptions, enum labels, `Error.detail`, `Warning.detail`, `NextOperation.rationale`, `Omission.subject`, and `recovery` arguments are typed fields authored from the IDL and this dossier, never from a snapshot, a log, a trace, or a model.
- **S2 — data fields are labeled as data.** A pack's `summary` and a diagnostic's message are renderings; they MUST NOT be the only place a machine-consumable fact appears (INV-003), and a renderer MUST escape them rather than concatenate them into anything an agent reads as instruction.
- **S3 — capabilities are enforced independently of model output** (docs/49). No text in any field, from any source, changes T1–T4.
- **S4 — attempted privileged operations are logged** whether or not they succeed (P5).
- **S5 — `cap_*` tokens never appear in a trace, an error, a `next_operations` argument, or a rendering.** The landed `CapabilityToken` makes this structural: no `Display`, an eliding `Debug`, and no store path.
- **S6 — redaction is typed and visible.** A value the daemon cannot return in full is the four-field `Redacted` stub, appears in `omissions` with reason `redaction`, and downgrades the assurance envelope for any claim that needed the hidden data (plan §18.4, RFC 0026). A verdict MUST NOT be reported at full strength over a redacted input on the grounds that the input "would have" supported it.
- **S7 — benchmark integrity is a scope property.** Hidden tasks, hidden variants, and graders lie outside agent-readable snapshots (research/33); no tool output may reveal an expected patch; and `benchmark.run` rejects any grader not registered for its task.

## Denial behavior

- **X1 — `CapabilityDenied` is the single answer to every admission failure.** Level, scope, privilege, expiry, revocation, actor mismatch, and a non-descendant capability all produce it, and a client MUST NOT be able to tell them apart. Distinguishing them tells an attacker which dimension to attack next.
- **X2 — `CapabilityDenied` is not a not-found.** A read of an artifact the caller is not authorized for MUST return `CapabilityDenied` whether or not the artifact exists; a distinct not-found *is* an existence oracle (RFC 0026, plan §4.5). Timing SHOULD be indistinguishable as well; a deployment that cannot bound the timing signal MUST declare the residual channel rather than imply it has closed it.
- **X3 — denial precedes semantic work and precedes the index.** No engine runs, no artifact is dereferenced, and no store lookup happens before admission is decided (A8).
- **X4 — a denial is a complete machine result.** It names every epoch, carries a typed `omissions` list, carries `retryable` (false, for every admission failure), and carries a `recovery` list computed under N2 — which for a denial is usually empty, and an empty list is the typed statement that no recovery exists rather than an unanswered question.
- **X5 — the neighbouring refusals are distinct and MUST NOT be collapsed.**

| Code | Says |
|---|---|
| `CapabilityDenied` | the presented capability does not admit this call |
| `IntentMutationDenied` | a protected Intent Contract field was mutated without `revise-intent`, or against an intent lock |
| `PolicyGateFailed` | a promotion gate or intent policy blocked an otherwise admitted operation |
| `UntrustedDomainBoundary` | the request crosses a trust boundary the Intent Contract does not authorize |
| `QuotaExhausted` | a capability's concurrency or resource quota is exhausted — retryable |
| `BudgetExhausted` | task-budget spend ran out; carries a continuation, and is never a semantic verdict |
| `InsufficientEvidence` | the evidence present does not meet the required assurance class |
| `UnsupportedSemanticFeature` | the operation is registered but its subsystem has not shipped (R-5) |

None of these is a semantic verdict about the caller's program (docs/49, INV-008).

## Isolation controls and the ratified workbench-security gate

The plan §24.5 workbench-security row (`quote-id=workbench-security-promotion-gate`, research/35) keeps autonomous promotion disabled until a red-team corpus clears in full. **The ladder is what that gate's authority checks enforce.** The gate's structure maps onto this document as follows; the ratified sentence is the sole authority for the corpus size and pass bar, and none of its numbers is restated here.

| Gate element | What enforces it |
|---|---|
| no unprivileged intent-status alteration | T1 (`revise-intent`) ∧ T3 (`intent.accept`/`reject`/`lock` are `@privileged`) ∧ A6 ∧ P4 |
| no unprivileged evidence-status alteration | A5 (no level writes a status) ∧ RFC 0038's service-restricted promotion ∧ P3 |
| no isolation escape | T2 scope ∧ the absence of any host-execution or network operation in the registry ∧ docs/49's worker isolation |
| every case refused **and** recorded | X1 ∧ P5 — a refusal with no audit record fails its case |
| the seven intent-policy blocks | plan §5.4's policy table, RFC 0037's closed verbs, and RFC 0031's classification of each as a protected change |
| the eight worker-isolation controls | docs/49; the protocol's share is below |

The protocol's share of docs/49's eight worker-isolation controls:

- *pinned image/toolchain* and *read-only input mount* — snapshots and domain packs are content-addressed and pinned in the snapshot, so substitution is a privileged intent diff rather than an environment change (plan §18.3).
- *no ambient credentials* — a capability is presented per request and nothing is ambient (INV-005); production-trace access is a separate grant (R-4).
- *resource limits* — `Budget` on every `@task_starting` operation, and `ServerLimits`/`QuotaExhausted` for capability quotas.
- *network disabled unless required and allowlisted* — enforced by the registry's shape: there is no network operation to call.
- *output size/schema limits* — B4–B6 and the IDL's typed response structs; a worker's output is parsed as untrusted data.
- *cancellation and hard-kill fallback* — `task.cancel` at `execute`, which finalizes or discards and never truncates a publication in progress.
- *audit trace* — the six `@audit_recorded` operations plus P5's denial records.

Consistent with research/35's kill criteria, no individual vulnerability kills the lane: a case that succeeds re-locks autonomous promotion until the enlarged corpus clears in full, with human-approved promotion as the standing fallback.

## Conformance examples

Four scenarios a conforming implementation MUST demonstrate. Each is stated as typed operations and typed outcomes; no scenario is satisfied by prose.

**1. Single agent, diagnose and repair.** A capability at `execute`, scoped to one snapshot and one intent, with no privileged grant.

```text
workspace.create                              → ws_1              (propose; admitted, execute > propose)
verification.start(ws_1, in_1, target)        → task_1            (execute, @task_starting: budget required)
verification.await(task_1)                    → crash_1 + ctx_1   (execute, @readonly @task_starting)
context.expand(ctx_1, causal_predecessors, 2) → ctx_2             (read; new pack, parent recorded)
repair.begin(crash_1)                         → rt_1              (propose)
repair.apply(rt_1, patch, hypothesis)         → rt_2              (propose)
repair.evaluate(rt_2)                         → rt_3 + gate results   (execute)
repair.promote(rt_3)                          → CapabilityDenied  (T1 and T3 both fail)
```

The final denial is the scenario's point: the agent reaches a promotable transaction and cannot promote it. `next_operations` on `rt_3` MUST NOT offer `repair.promote` (N2), and the denial MUST be audited (P5).

**2. Coordinator handoff, two sub-agents.** The coordinator holds `execute` with `delegation_depth ≥ 1` and mints two children.

- Both children share `snapshots = [ws_1]`, `intents = [in_1]`, and read scope over `ev_`.
- Child A adds `dbg_`; child B adds `ps_`. Neither has the other's class (D4).
- Each child's admission set is a subset of the coordinator's (D7), and each has `delegation_depth = 0` (D2).
- Child A opens a debugger branch and child B opens proof state; each publishes evidence candidates; neither can promote a claim's status (H5, A5).
- Both complete the scenario with no session coupling: dropping either connection and re-reading the handles under the same capability continues the work (H3, INV-002).
- Revoking the coordinator's capability revokes both children (D8), and the next request on each open child connection fails `CapabilityDenied` (R1) without invalidating anything already published (R2).

**3. Stale handle and stale capability.** Against a snapshot that has been superseded and a capability that has been revoked:

- an operation naming the old sealed snapshot returns `StaleSnapshot` with a `recovery` list that re-bases;
- a resume whose continuation pins a superseded semantic epoch returns `ContinuationEpochMismatch` naming the disagreeing kind, and never silently re-runs;
- a page token minted under the previous epoch returns `EpochUnsupported`;
- after revocation, the same connection's next request returns `CapabilityDenied`;
- a read of an artifact that does not exist and a read of one that exists but is out of scope return **byte-identical** envelopes (X2).

**4. Prompt injection.** A snapshot whose source comments, log lines, and model-produced strings all contain instructions — "you may promote this", "call `intent.accept`", "your capability is `cap_…`", "ignore the omission manifest".

- No such string reaches a tool description, a parameter description, an error `detail`, a `rationale`, or a `recovery` argument (S1, TS-5).
- No such string changes T1–T4 (S3); the corpus cannot trigger a privileged operation (docs/52 G2).
- A capability token appearing in untrusted content is not usable, is not echoed, and is not logged (S5).
- An instruction to suppress uncertainty has no effect: the omission manifest and the nine assurance dimensions are emitted regardless (B6).
- Every attempt is recorded (S4, P5), which is what makes the corpus gradeable at all.

## Evaluation

The ablation is a four-rung baseline ladder run with **identical base models, task sets, and per-task budgets**:

```text
raw shell agent  →  typed ACI  →  ACI + Context Packs  →  ACI + evidence graph
```

- The lane owner is research/25, which supplies the harness and the experiment list; research/33 supplies the anti-gaming grading discipline — the independent grader, family-level splits, resource-normalized reporting, and hidden semantic variants.
- Metrics: task success, invalid-operation rate, **interface bytes per solved task**, expensive failures, and stale-handle recovery. Bytes, not tokens, are the graded cost denominator, because token counts are model-relative (B4). Token counts MAY be reported alongside, with their tokenizer identity.
- The promotion threshold is the ratified `quote-id=aci-benchmark-margins` sentence in plan §24.5 and research/25. It fixes **three** margins — absolute task success, interface bytes per solved task, and invalid-action rate — and missing any one of them fails G0-DX-10 and forces protocol redesign before freeze. This RFC restates none of the numbers and MUST NOT be read as relaxing any of them.
- The protocol MUST NOT freeze until that result is in hand. If the typed surface loses, the ACI is redesigned, not excused.
- The benchmark subset used for the ablation MUST itself pass the plan §19.4 family/source-hash separation check before any result is accepted (docs/52's G0 staging rule); full leakage validation remains a later gate.

## Corrections recorded by this RFC

Per plan §25, where plan prose, docs, or a dependent artifact disagrees with this RFC, this RFC governs — except for wire shapes, where the IDL governs and this RFC is corrected. The corrections in force, each with its direction.

### Where the IDL corrected this RFC

1. **The registry is per-operation, not per-namespace group.** The previous revision's table carried 27 grouped rows with slash-separated verb lists, three of which covered the same namespace at the same level. A grouped row cannot be compared token-for-token against 72 `authority` clauses without re-deriving the split, which is the re-derivation `rule conformance.registry_agreement` forbids. Normative: one row per registered operation, 72 rows, with every level token unchanged. Direction: the IDL decides the granularity; this RFC is corrected.
2. **`revise-intent` is the wire token; `revise_intent` is the IDL identifier.** The IDL declares `revise_intent = "revise-intent"`, and the landed variant is `AuthorityLevel::ReviseIntent`. Normative: prose and adapters use `revise-intent`; generated code uses the identifier. Direction: the IDL decides; this RFC records the mapping.
3. **`ExpansionRelation` is closed, not "initial".** The previous revision called the eleven relations "the initial relation vocabulary", which reads as an extension point. Normative: the enum is closed at protocol 3.0, and adding a member is a breaking change under `rule versioning.breaking_change` requiring a coordinated revision of RFC 0028, whose pack artifact carries the same closed set. Direction: the IDL decides; this RFC is corrected.
4. **`CapabilityDescriptor` has eight fields and `delegation_depth` is a number.** The previous revision said a `cap_*` "names its actor, level, resource scope (snapshots/intents/artifact classes), expiry, and delegation allowance". Normative: `capability`, `actor`, `level`, `snapshots`, `intents`, `artifact_classes`, `expires_at` (nullable), and `delegation_depth` (`U32`, required) — a depth, not an allowance flag. Direction: the IDL decides; this RFC is corrected.
5. **`output_policy` is optional, and its absence is not "no ceiling".** The previous revision wrote "Requests carry `output_policy`". Normative: the field is `optional` on `RequestEnvelope`; when absent, `ServerLimits.max_result_bytes` applies, and a policy above that limit is clamped visibly. Direction: the IDL decides; this RFC is corrected.
6. **Authority is orthogonal to the mutation annotation in both directions.** The previous revision described `context` as a read-only pair and said nothing about a `@readonly` operation requiring more than `read`. Normative: `context.compile`, `context.expand`, `evidence.verify`, and `query.clean_compare` are `@mutation` at `read`; `workspace.diff` is `@readonly` at `propose`. Direction: the IDL decides; this RFC absorbs the orthogonality as A3.
7. **The privileged set is five operations and the audit-recorded set is six.** The previous revision named `intent.accept` and `intent.lock` as the audit-recorded intent operations, omitted `intent.reject`, and treated "privileged" as a description rather than an annotation. Normative: `@privileged` is `intent.accept`, `intent.reject`, `intent.lock`, `repair.promote`, `repair.reject`; `@audit_recorded` is those five plus `observe.ingest`. Direction: the IDL decides; this RFC is corrected.

### Where this RFC decides

8. **Admission is a four-test conjunction, not a level comparison.** No previous revision said how level, scope, privilege, and validity compose. Normative: T1 ∧ T2 ∧ T3 ∧ T4, each of which can only deny, so RFC 0026's "decidable from the table" statement is preserved as a statement about T1 and the ladder remains an upper bound. Direction: this RFC states a composition obligation neither the IDL nor RFC 0026 states alone; the landed `ScopedCapabilityPolicy` already implements T1 ∧ T2 in this shape.
9. **`@privileged` is a second admission test, not a level.** This is what makes "proposal only" reachable: `intent.propose_revision` is `revise-intent` and *not* privileged, while the other three intent verbs are both. Read as a level, "proposal only" is unreachable, because any `revise-intent` grant would admit acceptance. Normative: T3. Direction: this RFC decides, consistently with RFC 0026's definition of `@privileged` as "never present in a default agent capability profile".
10. **The request actor is bound to the presented capability.** The IDL declares `actor` on `ClientHello`, on `RequestEnvelope`, and on `CapabilityDescriptor`, and states no equality rule; without one an actor string is a free-form claim, and RFC 0026's per-actor idempotency scoping — a stated security property — is unenforceable. Normative: `RequestEnvelope.actor` MUST equal the descriptor actor of the capability that request presents, `ClientHello.actor` MUST equal the connection capability's, and a mismatch is `CapabilityDenied` rather than `MalformedRequest`. Direction: this RFC decides.
11. **"Narrower capability" is defined by admission-set inclusion and descent.** RFC 0026 permits a request to present a narrower capability and states no test. Normative: the presented capability MUST be the connection's own or a descendant of it, and its admission set MUST be a subset; a daemon that cannot establish both denies. Direction: this RFC decides, supplying the test RFC 0026's rule needs.
12. **`next_operations` and `Error.recovery` are capability-relative.** Neither the IDL nor any previous revision said whether a recovery surface may name an operation the caller cannot execute. Normative: it may not — a recovery surface never widens authority and never advertises a privileged path to a principal that does not hold it. Direction: this RFC decides.
13. **Tool lists are lexicographically ordered by `OperationName`, and capability filtering is never the authority check.** plan §10.4 requires "deterministic tool ordering" and names no order. Normative: TS-3 and TS-4. Direction: this RFC decides.
14. **Handoff isolation is capability scope plus daemon-enforced creator ownership.** plan §10.5 names the isolated sets in prose. Normative: shared state is instance scope over `ws_*`/`in_*` plus the `ev_` class; isolated state is the `dbg_`, `ps_`, and `forge_` classes; per-instance isolation between two sub-agents holding the same class is enforced by the daemon from the creating capability's actor until F2 is paid. Direction: this RFC decides the interim rule and names the gap.
15. **Expansion never widens scope.** Normative: a child pack's contents lie within the requesting capability's scope, and a relation that would cross the boundary is reported as an omission rather than followed. Direction: this RFC decides, closing the escalation path through a `read`-authority operation that publishes.
16. **The store-side administrative floor is not a sixth level.** `Action::Administer` and `Action::Audit` in `crates/continuum-workspace/src/publication.rs` have `minimum_authority() == Promote`. Normative: that is a store-side floor for the out-of-band surface; it is not a ladder member, not a registry entry, and not a wire authority. Direction: this RFC states the reconciliation so the crate and the ladder do not drift; bringing administration onto the wire remains RFC 0026 correction 20's conditional.
17. **Bytes are the graded cost denominator, and the freeze consequence is all three ratified margins.** The previous revision listed "tokens/bytes" among the ablation metrics and stated the freeze condition as a single "beats disciplined shell use", contradicting its own byte rule and understating G0-DX-10. Normative: interface bytes per solved task is the denominator, tokens are reportable with a tokenizer identity, and missing any one of the three ratified margins fails G0-DX-10. Direction: this RFC is corrected to agree with itself and with the ratified register row that cites it.

### Where this RFC governs the plan and docs

18. **The ACI ablation lane is research/25.** The previous revision attributed the baseline ladder to research/33 alone. Normative: plan §24.5's agent–computer-interface row is owned by research/25, which supplies the harness and the ratified margins; research/33 co-owns the context-compilation ablation and supplies the anti-gaming grading discipline. Direction: this RFC is corrected against the register, which governs lane ownership.
19. **docs/36's "useful relations" list is not the vocabulary.** It names nine informal relations, omits `same_owner`, `property_automaton_step`, and `refinement_of`, and adds two that are not relations at all: "state-change provenance" is `causal_predecessors` from a `state_delta` anchor, and "minimal correction candidates" is the `repair_surface` selection kind of RFC 0028. Normative: the closed eleven. Direction: RFC governs docs/36. Its `relation="source"` example and its `budget.max_nodes` example are separately corrected by RFC 0026, corrections 26 and 27.
20. **docs/36's tool-design rule 10 names a token the protocol does not have.** It asks that `Unknown` and `Inconclusive` be easy to represent. Normative: there is no `Unknown` verdict, status, or reason on this protocol (RFC 0026); the verdict token is `inconclusive` with one of six typed reasons, `unknown` is a Context Pack selection kind (RFC 0028) and a diff relation (RFC 0031), and the three MUST NOT be conflated. Direction: RFC governs docs/36, adopting RFC 0026's rule.
21. **docs/36's repair-authority lists confer nothing.** Its "may" list is the `propose` and `execute` rows and its "may not" list is A5, T3, and RFC 0038's service restriction. Normative: the registry and the admission predicate decide; an adapter convention is not an authority boundary. Direction: RFC governs docs/36.
22. **docs/49's capability matrix is a role sketch, and two of its cells are not expressible as levels.** The Reviewer row grants "start bounded task" while denying "propose", which a total order cannot express because `execute` subsumes `propose` and class scope cannot separate reading a class from writing it; and "revise intent: proposal only" is expressible only through T3. Normative: the enforceable form is the (level, privileged, scope) triple tabulated above. The Reviewer restriction needed a per-operation restriction the descriptor could not carry when this correction was written (F1); as of protocol 3.1 it can — `profile.denied_operations` — and the triple is a wire value rather than a deployment's promise. Direction: RFC governs docs/49's matrix as a role sketch; the triple is what a deployment issues.
23. **docs/49's "proposal only" intent cells describe a task grant, not a default profile.** Normative: default agent profiles omit `revise-intent` entirely (A6); a `revise-intent` grant without privileged membership is issued for an explicit redesign task, per plan §5.4, and the grant is itself audit-recorded. Direction: RFC governs docs/49; plan §5.4 already carries the condition.
24. **docs/55 carries no authority annotation at all.** RFC 0026 correction 32 already sends it for regeneration for omitting eight operations and two handle classes; independently of that, nothing in it distinguishes a `read` operation from a `promote` one, which is the single distinction an agent-facing reference exists to make visible. Normative: a regenerated docs/55 MUST carry the per-operation authority level from this registry. Direction: RFC governs docs/55.
25. **plan §10.4's evidence-status rule is not adapter-specific.** "No MCP prompt or resource may mutate evidence status" is true of every adapter and of the native protocol, because no operation in the 72 sets a status. Normative: A5. Direction: RFC generalizes plan §10.4; no contradiction.
26. **plan §10.5's handoff sets are artifact classes.** "Proof search state", "debugger branch", and "Forge candidate pool" are `ps_`, `dbg_`, and `forge_`. Normative: as tabulated in "Handoff". Direction: RFC makes plan §10.5's prose mechanical.

### Where this RFC decides at protocol 3.1

27. **T4's normative home is this document; RFC 0026 names it at the step where it fires.** The rule was stated here and nowhere in RFC 0026, which owns the connection state machine — a new normative surface in a document that does not carry it. Decided against the criterion "which document does a daemon implementer read to build the connection state machine": RFC 0026, so RFC 0026's "Connection lifecycle" step 3 now states the handshake half as an obligation and cites this predicate for its text (RFC 0026, correction 41). Normative: one home, here; one pointer, there. Moving the text was rejected on three grounds — it would split the T1–T4 conjunction across two documents, so a daemon could implement three tests from one and miss the fourth; the per-request half of T4 fires outside the connection state machine entirely and would be left homeless or duplicated; and both documents' "Precedence" sections already assign the frames to RFC 0026 and admission to this one. Direction: decided jointly and recorded in both documents, which MUST be revised together on this rule.
28. **T3 has a wire representation, and the (level, privileged, scope) triple is a wire value.** F1 is paid: `CapabilityDescriptor.profile` carries `privileged_operations`, `denied_operations`, `data_grants`, and `cross_principal_sharing` (protocol 3.1). Normative: T3 is read off `profile`, an absent profile denies every `@privileged` operation and every operation requiring a grant, both lists are narrowing-only, and an operation named in both is denied. This makes docs/49's Reviewer cell expressible (correction 22), makes D7's subset check a computation rather than an assertion, and makes `observe.ingest`'s second requirement (R-4) a field rather than a sentence. Direction: this RFC owns admission and decides how the fields are read; the shapes are the IDL's.
29. **The eight flags this RFC raised are dispositioned, and a deferral is a decision.** Normative: F1, F4, F5, F6, and F7 are paid at protocol 3.1; F2, F3, and F8 are deferred, each with the reason recorded in its entry rather than in a note elsewhere. F3 is additionally reclassified: it is not a wire-shape gap and no additive field can close it, so it is discharged by the Acceptance row that tests it. F8 is not payable at all while capability administration is out of band by decision. Direction: this RFC dispositions its own flags; the entries are retained whole under the house rule that a resolved item keeps its record.
30. **No flag was paid with a new operation.** Normative: the registry is 72 rows, the per-level counts are 18/8/40/4/2, and the annotation counts are unchanged by the sweep. This is stated as a decision rather than an observation because three of the five payments had a tempting registry-shaped alternative — an administrative verb for the profile, a handshake operation for the rejection, an audit-read verb for the correlation identity — and each would have breached A7 or RFC 0026's correction 20. A protocol sweep that grows the registry is a redesign, not a sweep.

### Flags raised against artifacts this RFC does not own

Every flag below was dispositioned by bn-3ayom on 2026-08-01: F1, F4, F5,
F6, and F7 were paid by the IDL 1.2 revision, and F2, F3, and F8 were
deferred with their reasons recorded in place. Entries are retained
whole, as the record of what was flagged and why; each entry's
disposition follows the em dash that ends its original text. Nothing
here added an operation, so the 72-row
registry table above, its five per-level counts, and its seven annotation
counts are untouched by the sweep.

- **F1 — `CapabilityDescriptor` can express only two of the four admission tests.** It carries `level` (T1) and three scope lists (T2), and has no field for T3 (which privileged operations a profile grants), for a per-operation restriction (docs/49's Reviewer cell, correction 22), for the production-trace grant `observe.ingest` additionally requires (plan §18.2, R-4), or for the cross-principal sharing policy RFC 0026 requires to be a capability property. Two capabilities with identical descriptors can therefore differ in whether they admit `intent.accept`. Minimal additive fix: `privileged_operations` and `denied_operations` as `optional list<OperationName>`, both defaulting to empty and both narrowing-only. Raised for the protocol sweep and the IDL fix list. — **PAID at protocol 3.1 (IDL 1.2, bn-3ayom, 2026-08-01), in full rather than minimally.** `CapabilityDescriptor` gains `profile: CapabilityProfile optional`, and `CapabilityProfile` carries all four of the gaps this entry names: `privileged_operations` (T3), `denied_operations` (the Reviewer cell), `data_grants` with the `production_trace` member (F5's requirement), and `cross_principal_sharing` (RFC 0026's capability property). The named minimal fix — two lists on the descriptor — would have paid the first two gaps and left the other two, both of which this entry itself raises; one optional field carrying a named struct pays all four, keeps the descriptor at nine fields, and gives "the default agent capability profile" a citable wire form. `rule capability.profile_narrowing` fixes the narrowing-only reading and the delegation subset rules (D6, D7).
- **F2 — instance-level scope exists only for snapshots and intents.** `artifact_classes` is a list of plan §4.4 prefixes, so two sub-agents that both hold `dbg_` hold the *class*, not disjoint branches. plan §10.5's isolation is therefore not expressible at instance granularity, and H1's interim rule is prose. Minimal additive fix: an instance-scope list per class, or an owner attribution on the isolable classes. — **DEFERRED at 3.1 (bn-3ayom, 2026-08-01).** Three reasons, in order of weight. The shape is undecided between the two candidates this entry names, and the open question below records that the choice depends on which of the debugger, the proof service, and Forge can each *enforce* it — a fact about three unshipped subsystems, not about the wire. A per-class instance map is `map<String, list<ArtifactHandle>>`, which the IDL's own grammar admits but nothing else does: neither the conformance parser nor the type layer expresses a list-valued map today, so paying it means moving the checking machinery in the same change that moves the wire, and the sweep's discipline is that the mirror follows the IDL rather than the reverse. And the interim rule holds and is stated (H1, correction 14): isolation between two holders of one class is enforced from the creating capability's actor. Deferring costs no major — an optional field remains a compatible change after freeze — and the flag stays open against the descriptor.
- **F3 — the capability-relativity of `next_operations` and `Error.recovery` is not expressible.** Both are unconditional lists in the IDL; correction 12's obligation is a presence condition on the *caller*, which a presence marker cannot state. Composes with RFC 0026's F1. — **DEFERRED at 3.1 (bn-3ayom, 2026-08-01), and reclassified.** No additive field pays it, because it is not a shape gap: both lists are `required` and correction 12 constrains their *contents*, which are computed per caller. A wire field could only assert that filtering happened, and an assertion a daemon writes about itself is not a check. The obligation is checkable, and is checked, from the outside: the Acceptance row "no `next_operations` or `recovery` list, on any result or error, names an operation the presenting capability would deny; a promote-capable connection and an agent connection receive different lists for the same result" is the test, and it is stronger than any presence marker. What remains is RFC 0026's F1 (a `@required_when`-style notation), which is a grammar change to the IDL rather than a wire addition.
- **F4 — the handshake has no rejection frame.** `ServerWelcome` is declared as the second frame "sent by the daemon on success", and nothing carries a connection-level `ProtocolVersionUnsupported` or `CapabilityDenied`, though RFC 0026 requires the daemon to reject the connection in both cases. A client cannot distinguish a typed refusal from a transport failure. Minimal additive fix: a `ServerReject` frame carrying an `ErrorCode` and `retryable`. Belongs to RFC 0026's handshake and to the IDL. — **PAID at protocol 3.1 (IDL 1.2, bn-3ayom, 2026-08-01).** `ServerReject` carries `code`, `detail`, `retryable`, and `majors_served`; the fourth field is beyond the minimal fix and is there because a client refused for its version never receives a `ServerWelcome` and has no other channel for the window it must retry inside. `rule handshake.rejection` closes the code set to the two RFC 0026 requires, fixes `retryable` false for both, forbids distinguishing an unregistered token from a revoked one (X1), and gates the frame on the client's *offer* — `high ≥ "3.1"` — because the frame precedes negotiation and one the client cannot parse is not a typed refusal. Two refusals stay outside: a malformed `ClientHello` and an encoding mismatch, for which the protocol fixes no code and inventing one would be inventing wire semantics; both remain transport-level closes and the residue is recorded in that rule.
- **F5 — `observe.ingest`'s second capability requirement is undeclared.** The IDL declares `authority execute`; plan §18.2 additionally requires the production-trace capability, and nothing on the wire says so. Composes with F1. — **PAID at protocol 3.1 (IDL 1.2, bn-3ayom, 2026-08-01), with F1.** `CapabilityProfile.data_grants` carries `DataGrant::production_trace`, and a capability without it is denied `observe.ingest` however high its level. `DataGrant` is `@open` so a second grant is a compatible addition, and because the fail-closed reading of an unknown member — "a grant I cannot name is one I MUST NOT assume I hold" — is already what `rule versioning.enums` requires. The operation remains `authority execute`, unshipped, and subject to `rule errors.unsupported_surface`: this declares the requirement, not the subsystem.
- **F6 — no adapter mapping manifest exists.** RFC 0026 correction 23 permits renaming only under a declared total mapping to `OperationName`, and TS-3 requires a deterministic listing; neither is a machine artifact today, so totality, determinism, and the absence of an undeclared surface are all unchecked. Minimal additive fix: a generated per-adapter mapping manifest derived from the IDL, covered by `rule conformance.generated_artifacts`. — **PAID as an obligation at protocol 3.1 (IDL 1.2, bn-3ayom, 2026-08-01); the artifact is an implementation deliverable.** `rule conformance.adapter_mapping` requires a renaming adapter to ship a generated manifest, total over the names it exposes and ordered lexicographically by `OperationName`, and places it under `rule conformance.generated_artifacts` — regenerated, never hand-edited. No wire surface changed, and none needed to. Stated honestly: this makes the requirement normative and citable at freeze; it does not itself produce a manifest, and no adapter has shipped one, so the four non-conforming surfaces RFC 0026 correction 29 names in docs/46 are now non-conforming against a rule rather than against prose.
- **F7 — a result cannot cite its own audit record.** plan §18.5 requires every privileged call to be recorded with actor, capability, and decision, and `ResultEnvelope` carries neither a capability nor an audit-correlation identity (RFC 0026 correction 32 notes the absent audit-correlation field). A caller therefore cannot name the record its own privileged call produced, which is exactly what the ratified workbench-security corpus grades. Minimal additive fix: an audit-correlation identity on `ResultEnvelope` for `@audit_recorded` operations. — **PAID at protocol 3.1 (IDL 1.2, bn-3ayom, 2026-08-01), with one addition the minimal fix did not name.** `ResultEnvelope.audit: AuditCorrelationId optional` is REQUIRED on every `@audit_recorded` result *and* on every `CapabilityDenied`, because P5 audits denials too and a denial that cannot cite its record fails its case in the ratified corpus exactly as an unrecorded one does. `rule audit.correlation` requires the identity to be a function of the request identity alone: a per-record random value would make two denials that differ only in whether the artifact exists distinguishable, turning the field added to record a denial into the existence oracle X2 forbids. The capability half of this entry is deliberately not paid — a `cap_*` never appears in a result (S5) — and the identity MUST NOT be derived from one.
- **F8 — the store-side and wire-side administrative floors are stated in two places.** `Action::minimum_authority` places administration at `promote` in `crates/continuum-workspace/src/publication.rs`, while the wire has no administrative surface at all (correction 16). The two are consistent today only because the surfaces are disjoint; should RFC 0026 correction 20 ever be revisited, both floors MUST move in the same change. — **DEFERRED at 3.1 (bn-3ayom, 2026-08-01), because there is nothing to pay.** The wire has no administrative surface *by decision* (RFC 0026, correction 20), so the disagreement this entry names is between a store-side floor and an absence; declaring anything on the wire to reconcile them would be the very registry entry that decision refused, and would put a mint one operation away from an agent connection. The co-movement obligation is where an implementer will meet it — RFC 0026's "If the decision is revisited" list — and correction 16 states the reconciliation. This entry stays open as the standing reminder that the two floors are coupled; it is not a gap the IDL can close.

## Rejected alternatives

- **Prose-first results with a JSON flag.** Rejected: prose is a projection (INV-003, ADR-0038).
- **Session-scoped agent state.** Rejected: it breaks resumability and multi-agent handoff (INV-002, B4).
- **Token-denominated enforcement.** Rejected: token counts are model-relative; bytes are objective, and the ratified ACI margin grades bytes for this reason.
- **A sixth authority level for capability administration.** Rejected with RFC 0026's decision: it would change `AuthorityLevel`, every `authority` clause, and this registry — a breaking change with a far larger blast radius than the gap it closes — and it would put a mint one operation away from an agent connection.
- **Per-operation authority as a partial order, or as a per-verb matrix.** Rejected: a partial order makes `CapabilityDenied` undecidable from a table, and a per-verb matrix makes every deployment's boundary different. A total order plus narrowing tests keeps the boundary one comparison and one subset check.
- **Inferring authority from an operation's annotations.** Rejected: authority and `@mutation` are orthogonal in both directions (A3), so a client that infers one from the other will treat a `read`-authority publication as privileged and a `propose`-authority read as free.
- **Granting authority to a handle.** Rejected (ADR-0037): content-addressed identities are derivable by anyone holding the content, so a handle that authorized anything would make possession the security boundary.
- **Deriving authority from the `actor` string.** Rejected: the actor is an attribution and the capability is the authority. The closed four-member scheme exists so attribution is machine-readable, not so a prefix can be trusted.
- **Sharing one capability across sub-agents.** Rejected: it destroys per-actor idempotency scoping, audit attribution, and selective revocation simultaneously.
- **Filtering the tool list as the authority check.** Rejected: an adapter that only hides operations is defeated by a client that calls them anyway; admission is decided below the adapter (A8).
- **A permissive `expires_at` default.** Rejected with RFC 0026: a non-expiring grant issued by omission is the failure mode capability scoping exists to prevent.
- **Offering privileged operations in `next_operations` for discoverability.** Rejected: a pre-filled, executable suggestion to promote is both an escalation invitation and an oracle over another principal's authority.

## Open questions

- ~~Capability delegation depth for sub-agent spawning (with RFC 0026)~~ — decided in the fail-closed direction: depth defaults to `0`, no child exceeds its parent in level, scope, expiry, depth, or privileged set, and revoking a parent revokes its subtree (RFC 0026 correction 21; D1–D9 here). What remains is the numeric expiry a deployment chooses, which is deployment policy rather than protocol.
- Expansion-relation completeness for proof workers (with RFC 0035 and RFC 0028): `proof_dependency` is one relation for what research/28 treats as several distinct edges.
- Whether `workspace.diff` should fall to `read` at the next protocol major. It is the only `@readonly` operation whose authority exceeds `read`, and it is the operation a review-only profile most wants; moving it is a breaking change and is not taken here.
- ~~Whether the descriptor gains F1's two narrowing lists before PR 5 freezes the interface, which would make T3 and docs/49's Reviewer cell machine-checkable instead of prose~~ — decided: it does, at protocol 3.1, and with four fields rather than two (`CapabilityProfile`; correction 28, F1). T3 and the Reviewer cell are machine-checkable before freeze.
- Whether per-instance scope (F2) is expressed as a scope list or as artifact-side ownership, and which of the debugger, the proof service, and Forge can each enforce. **Still open, and deliberately**: F2 was deferred at 3.1 because the answer to the second half decides the first, and it is a fact about three unshipped subsystems. The interim rule (H1, correction 14) holds meanwhile, and the fix stays compatible-additive after freeze.
- Whether a capability profile itself becomes a named, versioned artifact, so that "the default agent profile" is citable rather than deployment folklore. **Half-answered at 3.1**: a profile is now a named wire *struct* a deployment issues and a client reads back in `ServerWelcome.grant`, so it is citable field-by-field. What remains open is whether it also becomes a published, content-addressed artifact with an identity a deployment can name in one token — which would be a plan §4.4 class, and therefore a registry question rather than a protocol one.

## Acceptance

- **Registry agreement** — every one of the 72 operations is enforced against this table in tests, and the table is compared token-for-token against the IDL's `authority` clauses by a check that fails closed on any disagreement rather than preferring either source (`rule conformance.registry_agreement`). The five per-level counts and the seven annotation counts are derived from the table, never asserted beside it.
- **Admission** — for every operation, a capability one level below its row is denied and a capability at its row is admitted; every `@privileged` operation is denied to a capability at or above its level without privileged membership; a request whose actor differs from its capability's descriptor actor is denied; a capability that is neither the connection's nor a descendant of it is denied.
- **Delegation** — a delegated capability cannot exceed its parent in level, scope, expiry, depth, or privileged set; its admission set is a verified subset of its parent's; revoking a parent revokes its subtree; a capability at depth 0 cannot delegate. As of 3.1 the privileged-set half is a field comparison: a child whose `profile.privileged_operations` or `profile.data_grants` exceeds its parent's, whose `profile.denied_operations` is not a superset, or whose `cross_principal_sharing` is true under a parent's false, is rejected at mint.
- **Profile admission** — a capability whose profile omits `intent.accept` is denied it at `revise-intent` and at `promote` alike; a capability with no profile is denied every `@privileged` operation and `observe.ingest`; an operation named in both `privileged_operations` and `denied_operations` is denied; a `denied_operations` entry naming an operation the level already denies changes nothing; `ServerWelcome.grant` reports the profile the connection holds, and a client that infers T3 from anything else is non-conforming.
- **Connection refusal** — a client outside the served window and a client whose capability is inadmissible each receive `ServerReject` rather than a bare close when their offered range reaches 3.1, and no frame when it does not; the two capability refusals are indistinguishable in `code` and `detail` (X1); `retryable` is false; a refused connection carries no request afterwards.
- **Audit correlation** — every `@audit_recorded` result and every `CapabilityDenied` carries `audit`; two denials differing only in whether the named artifact exists carry the same value and remain byte-identical (X2); no `audit` value reveals or is derived from a `cap_*` (S5).
- **Denial** — an unauthorized read of an artifact that exists and of one that does not produce byte-identical envelopes; every admission failure is `CapabilityDenied` with `retryable` false and a typed `recovery`; every denial is audited, including one against an unregistered token.
- **Recovery surfaces** — no `next_operations` or `recovery` list, on any result or error, names an operation the presenting capability would deny; a promote-capable connection and an agent connection receive different lists for the same result.
- **Bounds and ordering** — two identical requests against the same snapshot and epochs return identical bytes; every trimmed result carries an omission and retains all nine assurance dimensions, its warnings, its epochs, and its redaction stubs; a `max_bytes` above the server limit is clamped visibly.
- **Prompt injection** — the corpus of docs/52's G2 exit cannot trigger a privileged operation, cannot place untrusted text in any instruction field, and cannot suppress an omission manifest; every attempt is audited.
- **Handoff** — two sub-agents sharing snapshot and intent but holding isolated debugger and proof handles complete the scenario without session coupling; dropping and reopening either connection changes nothing; neither can promote a claim's status.
- **Ablation** — the four-rung baseline ladder is run under research/25's harness with research/33's grading discipline, on a benchmark subset that has passed the plan §19.4 separation check, and its report is attached to the freeze decision; all three ratified margins hold, or the protocol is redesigned before freeze (G0-DX-10).
