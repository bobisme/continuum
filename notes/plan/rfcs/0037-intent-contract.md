# RFC 0037: Intent Contract Schema and Policy

## Status
Draft for implementation.

**Target gate:** G3 (intent integrity)
**Owners:** intent/workbench leads
**Normative language:** MUST/MUST NOT/SHOULD/SHOULD NOT/MAY per RFC 2119.
**Normative schema:** [`../schemas/intent-contract.schema.json`](../schemas/intent-contract.schema.json) for the contract document and [`../schemas/intent-registry-record.schema.json`](../schemas/intent-registry-record.schema.json) for its registry record. Where this document and a JSON schema disagree, the schema is corrected or this RFC is amended by an explicit revision; neither drifts silently (INV-003: schemas decide artifact shape).
**Normative wire vocabulary:** [`../schemas/continuumd-native-protocol.idl`](../schemas/continuumd-native-protocol.idl) (RFC 0026) — `IntentHandle` (`in_`), `IntentBundleHandle` (`inb_`), `IntentChangeSet`, `AuthorityLevel::revise-intent`, `StructuralOutcome`, `PolicyDecision`, and the error codes `IntentMutationDenied`, `AcceptanceChainInvalid`, `PolicyGateFailed`, `CapabilityDenied`, `MalformedRequest`.
**Companion normative sources:** [RFC 0031](0031-semantic-and-intent-diff.md) (classification lattice, per-field orders, policy verdict, impact set), [RFC 0026](0026-continuumd-native-protocol.md) (protocol, epochs, error taxonomy), [RFC 0027](0027-agent-tool-protocol.md) (authority ladder and capability scope), [RFC 0032](0032-repair-transaction-protocol.md) (gate 3 and promotion-time recomputation), [RFC 0030](0030-incremental-semantic-query-engine.md) (query key and dependency reasons), [RFC 0038](0038-multi-agent-evidence-graph.md) (append-only write model and the `conflict` node), [RFC 0015](0015-temporal-fairness-and-hyperproperties.md) (temporal core, fairness, completion policy), [RFC 0002](0002-controlled-effects-and-domain-packs.md) (fidelity profiles), [`../plan.md`](../plan.md) §4.2/§4.2.1/§5/§25, [ADR-0039](../adr/0039-protected-intent-contract.md), [ADR-0025](../adr/0025-stratified-model-language-fragments.md), [ADR-0013](../adr/0013-exact-state-identity.md), [ADR-0018](../adr/0018-semantic-versioning-and-replay.md), [`../schemas/README.md`](../schemas/README.md).
**Landed vocabulary:** `crates/continuum-value/src/epoch.rs` (`IntentEpoch` — "the Intent Contract vocabulary and its policy tables"; `EpochSet::with_intent`), `crates/continuum-value/src/assurance.rs` (`AssuranceLevel`, `AssuranceRequirement`), and `crates/continuum-intent` (the protected contract's types and policy, inside the smallest trust base). This RFC and those modules MUST be revised together; neither may move alone.

## Summary

The Intent Contract is the protected statement of what a system must mean. It is the root object of plan.md B1/INV-001: ordinary tasks cannot mutate it, repairs are evaluated against it, and every change to it is classified by RFC 0031 before any evidence survives the revision.

This RFC is the normative home of: the contract's field set and field types; the closed policy-verb set and its enforcement; canonical identity and the CPNF-1 property normal form; custody in the daemon's intent registry and the registry status machine; signed intent-bundle distribution, import, acceptance chains, and three-way convergence; and the revision procedure. RFC 0031 is the normative home of the classification lattice and of the per-field orders those verbs are enforced against; this RFC does not restate them and MUST NOT contradict them.

Plan §5.2, §5.4, and the docs restatements are informal; where they disagree with this document, this document governs (plan §25: "Where plan prose and RFC disagree, the RFC is corrected and becomes normative"). Every such correction is recorded below under "Corrections recorded by this RFC".

## Versioning and revision

- The contract document's encoding contract is versioned by `schema_epoch`, declared both in the schema document's `$id` and in every instance's `schema_id`/`schema_epoch` header ([`../schemas/README.md`](../schemas/README.md), plan §25 SD-08). Epoch 1 is the vocabulary specified here. A consumer that does not implement an instance's `schema_epoch` MUST reject the artifact with a typed error; an intent contract is never best-effort decoded across a breaking epoch.
- The following vocabularies are **closed**. Adding, removing, or renaming a member is a breaking change that requires a `schema_epoch` advance *and* an explicit revision of this RFC: the fifteen policy keys; the eight policy verbs; the eleven formula node kinds and five term node kinds; the eight comparison operators; and the `claims[].kind`, `assumptions[].classification`, `fidelity_profile`, `scope.fragments`, `fault_model.enabled`, `completion_policy`, `nondeterminism[].class`, `assurance.minimum`, `accepted_evidence_classes`, and `security_policy.data_classification` enums.
- A reader that encounters an unrecognized token in any of those vocabularies MUST fail closed: it MUST reject the contract, and MUST NOT treat an unknown policy verb as `unlocked`, an unknown node kind as an opaque atom, or an unknown fragment as declared. Forward compatibility is achieved by rejecting, never by ignoring.
- CPNF-1 is versioned by this RFC. A new normal form (`cpnf-2`, or a fragment-specific form) changes the canonical encoding and therefore the `in_*` identity of every stored contract, so it MUST NOT be applied in place: it is an epoch advance under plan §4.6, it MUST publish the typed `Preserved | Revalidate | Incompatible` statement for the intent-contract class before it is applied, and re-normalized contracts are new artifacts linked by `SUPERSEDES` (INV-009). Diffs MUST NOT be reused across a normal-form change (RFC 0031, "Inputs, preconditions, and determinism").
- The **intent epoch** — one of the six independently versioned epochs (`crates/continuum-value/src/epoch.rs`, `IntentEpoch`) — versions the contract vocabulary and the policy tables. It is not `schema_epoch`, which versions one artifact class's encoding and pins no semantics, and it is not the semantic epoch, which versions what an evaluation means. The three MUST NOT be conflated, and an intent-epoch advance MUST NOT be inferred from a `schema_epoch` value or the reverse.
- The `assurance.minimum` enum and its order are shared with RFC 0031 and with `crates/continuum-value/src/assurance.rs`. Reordering or renaming any member is a coordinated breaking change across this RFC, RFC 0031, the contract schema, and that crate.
- Accepted contracts are immutable. A revision never edits a stored contract: it mints a new `in_*` and records a supersession edge. Nothing here permits an in-place edit of an accepted contract, including a "cosmetic" one — a cosmetic change either leaves the identity untouched, because the changed bytes are excluded from the identity preimage, or it is a revision.

## Storage and custody

- Intent Contracts MUST be stored and versioned only in the daemon's intent registry, outside every writable or forkable workspace snapshot (plan §4.2).
- A workspace snapshot carries the intent's content identity (`in_*`) as a reference, never the contract itself (`SnapshotComponents.intent`, RFC 0026). `workspace.fork` MUST preserve the binding by identity and returns it in its response.
- Rebinding a snapshot lineage to a different intent is a privileged operation that MUST produce an intent diff (RFC 0031) and MUST invalidate dependent evidence.
- `workspace.create` accepts an arbitrary `IntentHandle` at `propose` authority. A snapshot whose intent reference differs from its lineage predecessor's is therefore a **rebind**, not an ordinary create: the daemon MUST classify it as one, MUST require `revise-intent`, and MUST NOT permit evidence produced under the new intent to be reported as evidence for the predecessor's intent. Evidence is keyed by the intent identity it was produced under (RFC 0030); a fork that changes the question does not inherit answers. This is the fork-with-modified-intent gaming path, and it is closed by lineage checking, not by handle possession — handles confer no authority (plan §4.4).
- Contract bytes are content-addressed and readable by anyone holding them. Custody is therefore about *authority to accept*, not about secrecy: possession of an `in_*` handle confers nothing, and every mutation path below is gated by the `revise-intent` capability and audit-recorded (RFC 0027).

### Registry record and status

The registry record is a separate artifact ([`../schemas/intent-registry-record.schema.json`](../schemas/intent-registry-record.schema.json)) that names the contract by identity and carries what is *about* the contract rather than *in* it: `record_id`, `intent`, `status`, the `acceptance` block (`accepted_by`, `capability` — pinned to `revise-intent` — `signature`, `audit_record`, `timestamp`), and the `chain` honored on bundle import.

- The record is not part of the contract's identity preimage (see "Canonical identity"). Accepting a contract MUST NOT change its `in_*`; acceptance changes the record's `status`, never the contract.
- Registry status is a three-value machine over the wire literals `proposed`, `accepted`, `superseded`:

  | From | To | Operation | Authority | Notes |
  |---|---|---|---|---|
  | (absent) | `proposed` | `intent.propose_revision`, bundle import | `revise-intent` / import | a proposal is never protected by INV-001 |
  | `proposed` | `accepted` | `intent.accept` | `revise-intent` | the only transition to protected status (plan §5.4); `acceptance` becomes REQUIRED |
  | `proposed` | rejected (terminal) | `intent.reject` | `revise-intent` | typed reason recorded; the record MUST NOT be deleted |
  | `accepted` | `superseded` | acceptance of a successor | `revise-intent` | a `SUPERSEDES` edge to the successor is recorded (plan §11.3) |

- No other transition exists. `accepted` MUST NOT return to `proposed`, `superseded` is terminal, and a status MUST NOT be lowered — the compare-and-set discipline of plan §11.7 and RFC 0038 applies to registry status as it does to claim status.
- `intent.get` returns the canonical contract together with its record, so protection status is never inferred from the contract bytes alone.
- The IDL's `StructuralOutcome` members `accepted`, `rejected`, and `locked` describe what a mutation *did*; the registry `status` describes what a contract *is*. The two vocabularies share tokens and nothing else, exactly as `StructuralOutcome::unchanged` and the relation `unchanged` do (RFC 0031); a daemon MUST NOT derive one from the other.

## Intent bundles and convergence

Absorbed from plan §4.2.1 (SD-05). Registries converge through signed, content-addressed **intent bundles** (`inb_*`).

### Bundle contents and identity

- A bundle exports one or more contracts with their acceptance records and policy tables. Bundles MAY be vendored in the repository or fetched by identity.
- A bundle MUST carry, for every contract it exports: the canonical contract bytes, the registry record including `status` and any `acceptance`, and the signature `chain`. A bundle that exports an `accepted` record without its acceptance block is malformed and MUST be rejected, not imported at a lower status.
- The `inb_*` identity MUST be the digest of the bundle's canonical encoding under the ID5 encoding rules, so two exporters of the same contracts and records produce the same bundle identity.

### Import

- **I1.** Import MUST be idempotent: importing the same bundle twice leaves the registry byte-identical.
- **I2.** Import MUST NOT change protection status upward. An imported `proposed` contract stays `proposed`. An imported `accepted` record is honored only if its signature chain verifies *and* satisfies the local policy; if the chain verifies but a signer is not locally trusted for its `scope`, the contract MUST be imported at `proposed` — never dropped silently and never accepted.
- **I3.** A chain that does not verify, or a referenced bundle that is absent, is `AcceptanceChainInvalid`. CI MUST fail closed on this code when the bundle referenced by the workspace configuration is absent or its acceptance chain does not verify (plan §4.2.1; `workspace.create` and `intent.accept` both carry the error).
- **I4.** Import MUST validate every contract against the schema and against the well-formedness rules W1–W10 before it enters the registry. An unparseable or ill-formed contract is rejected; it is never stored partially.
- **I5.** Import MUST recompute each contract's `in_*` from its bytes and MUST reject a bundle whose declared identity disagrees. A bundle is untrusted data (INV-016): its labels, names, and rationale strings MUST NOT be interpolated into tool descriptions or error text.
- **I6.** Import MUST NOT overwrite a locally accepted contract with a different body under the same identity. Two distinct bodies claiming one `in_*` is a collision and MUST be resolved by canonical comparison (ADR-0013), never by import order.

### Acceptance chain

- **A1.** An acceptance signature MUST cover, at minimum: the accepted contract's `in_*`, the base contract's `in_*` (or an explicit genesis marker), the `revise-intent` capability, the accepting principal, and the timestamp — over the canonical encoding of those fields.
- **A2.** Binding the base identity is what stops **acceptance replay**: a signature that covers only the accepted identity can be lifted from one lineage and replayed onto another whose base was never reviewed. A verifier MUST reject an acceptance whose base does not match the registry's lineage for that contract.
- **A3.** Chain elements are verified in order, and each element's `signer` MUST be permitted by the local policy for that element's `scope`. Local policy — not the bundle — decides which signers count.
- **A4.** Verification failure at any element is `AcceptanceChainInvalid` for the whole record. Partial credit does not exist: a chain is honored or it is not.
- **A5.** Acceptance is not transitive across contracts. Accepting a contract says nothing about the contracts it was merged from, and a merge result MUST be accepted on its own (M7).

### Three-way merge

Divergent branches are reconciled by semantic three-way merge; the procedure is normative.

- **M1.** The common ancestor is the nearest contract reachable from both heads through registry lineage (`SUPERSEDES`). If no common ancestor exists there is no merge: the two lineages are independent and MUST be reconciled by an explicit revision against one of them.
- **M2.** Each head's §5.3 diff (RFC 0031) is taken against the common ancestor.
- **M3.** Two revisions merge automatically only when both diffs are classified **independent within supported fragments**. Independence is: (a) neither diff carries a non-affirmative relation (`unknown`, `unsupported`, `incomparable`) on any field; (b) no classified unit key — claim `id`, assumption `id`, observer `id`, abstraction-map `role`, nondeterminism `site`, fairness (`kind`, `action`) pair, fault class or profile name, optimization or non-vacuity string, redaction class, capability requirement — is touched by both; (c) no whole-field unit (`bounds`, `trust_boundaries`, `completion_policy`, `assurance`, `scope`, `security_policy.data_classification`) is touched by both; and (d) the two policy tables join representably (see "Policy joins").
- **M4.** Otherwise the merge is a `Conflict` node — the evidence-graph node kind `conflict` (plan §11.2; the wire literal is lowercase) — whose resolution requires the `revise-intent` capability, never an automatic pick by write order or timestamp. Resolution by "last writer wins", by timestamp, by branch precedence, or by lexicographic order of `in_*` is forbidden.
- **M5.** An automatic merge applies both change sets to the ancestor, revalidates the result against the schema and W1–W10, and **recomputes** the identity. A merge result never inherits an identity from either head.
- **M6.** The merge MUST be deterministic and order-independent: `merge(A, B)` and `merge(B, A)` MUST produce byte-identical results. Golden convergence vectors are part of the acceptance suite.
- **M7.** A merge result enters the registry at `proposed`, whatever the status of its inputs, and reaches protection only through `intent.accept`. Protection MUST NOT be laundered through a merge.
- CI MUST fail closed (`AcceptanceChainInvalid`) when the bundle referenced by the workspace configuration is absent or its acceptance chain does not verify.
- The intent diff ships with a PR-reviewable projection: a SARIF note plus a rendered semantic diff (plan §17.6).

## Fields

The contract is a JSON object with `additionalProperties: false`. Eighteen keys are REQUIRED; `name` and `policy_reviewers` are OPTIONAL. Fifteen of the groups are protected, and they relate to the fifteen policy keys — and to RFC 0031's fifteen diff `field` members — through the schema's normative `x-policy-field-map`. The *policy* name is the diff name; the contract path is not.

| Contract key | JSON type | Required | Policy key | Classified unit (RFC 0031) |
|---|---|---|---|---|
| `schema_id` | `const` class URI | yes | — | not classified |
| `schema_epoch` | `const` integer | yes | — | not classified |
| `intent_id` | string, `^in_[A-Za-z0-9_-]+$` | yes | — | not classified (the identity itself) |
| `name` | string | no | — | metadata; excluded from identity |
| `claims` | array, ≥ 1 item | yes | `properties` | one claim, keyed by `id` |
| `assumptions` | array (MAY be empty) | yes | `assumptions` | one assumption, keyed by `id` |
| `observers` | array (MAY be empty) | yes | `observers` | one observer, keyed by `id` |
| `abstraction_maps` | array (MAY be empty) | yes | `abstraction_maps` | one binding, keyed by `role` |
| `scope` | object | yes | `scope` | `components`, `fragments`, `abstraction_level` |
| `trust_boundaries` | object | yes | `trust_boundaries` | the pair (`trusted`, `opaque`) |
| `bounds` | object (MAY be empty) | yes | `bounds` | the tuple `(values, nodes, faults, depth)` |
| `fault_model` | object | yes | `faults` | one class or profile |
| `fairness` | array (MAY be empty) | yes | `fairness` | one constraint, keyed by (`kind`, `action`) |
| `completion_policy` | enum scalar | yes | `completion_policy` | the scalar |
| `nondeterminism` | array (MAY be empty) | yes | `nondeterminism` | one entry, keyed by `site` |
| `assurance` | object | yes | `assurance` | the requirement triple plus the accepted-class set |
| `optimization` | object (MAY be empty) | yes | `optimization` **and** `non_vacuity` | one string per set |
| `security_policy` | object | yes | `security_policy` | classification; each redaction class; each capability requirement |
| `policy` | object, all fifteen keys required | yes | — | not classified; it governs |
| `policy_reviewers` | map from policy key to principals | no | — | not classified; binds the `review` verb |

`optimization` is one contract group governed by **two** policy keys: `optimization.hard` and `optimization.soft` under `optimization`, and `optimization.non_vacuity` under `non_vacuity`. The split is required, not cosmetic — `non_vacuity = "no-removal"` (INV-012) is unenforceable if a removed non-vacuity behavior is reported as an `optimization` change (RFC 0031).

### Field types and enums

| Group | Item shape | Required subkeys | Closed enums, types, and notes |
|---|---|---|---|
| `claims[]` | `{id, kind, expression, observer?}` | `id`, `kind`, `expression` | `kind` ∈ {`safety`, `liveness`, `refinement`, `hyperproperty`, `security`, `performance`}; `observer` is a string or `null`; `expression` is a property AST with its normal form specified below |
| `assumptions[]` | `{id, expression, classification?, fidelity_profile?}` | `id`, `expression` | `classification` ∈ {`environment`, `scheduler`, `timing`, `storage`, `network`, `trust`}; `fidelity_profile` ∈ {`ideal`, `contractual`, `platform-qualified`, `adversarial-envelope`} (RFC 0002 — profiles are NOT automatically ordered); timing assumptions are declarative until the ADR-0016 lanes ship |
| `observers[]` | `{id, events, state_projection?, knowledge_projection?, security_projection?}` | `id`, `events` | the four projection kinds of plan §5.2 — `events` (event families), `state_projection`, `knowledge_projection`, `security_projection` — all string sets with `uniqueItems`; the unit reductions are justified against the observer (INV-013) |
| `abstraction_maps[]` | `{role, map_id}` | both | content identities of the §16 correspondence/abstraction maps the claims are stated against; `map_id` matches the §4.4 handle pattern and MUST resolve (RFC 0036); `merged`/`split` changes are privileged (RFC 0031) |
| `scope` | `{components, fragments, abstraction_level?}` | `components`, `fragments` | `fragments` ⊆ {`Finite`, `Symbolic`, `Temporal`, `Probabilistic`, `Theorem`, `Runtime`} (ADR-0025; capitalized on the wire) and bound what the diff can classify |
| `trust_boundaries` | `{trusted, opaque}` | both | two string sets with `uniqueItems`; expansion of the opaque set is a protected change (the "opaque escape", docs/41) |
| `bounds` | `{values?, nodes?, faults?, depth?}` | none | `nodes` integer ≥ 1; `faults` integer ≥ 0; `values` integer ≥ 1 or `null`; `depth` integer ≥ 0 or `null`; componentwise partial order (RFC 0031) |
| `fault_model` | `{enabled, profiles?}` | `enabled` | `enabled` ⊆ {`crash`, `recovery`, `partition`, `loss`, `duplication`, `delay`}; `profiles` are domain-pack profile names (RFC 0002) |
| `fairness[]` | `{kind, action, condition?}` | `kind`, `action` | `kind` ∈ {`weak`, `strong`}; `condition` is a temporal-operator-free property AST or `null`; fairness strengthening is the canonical gaming vector (docs/50) |
| `completion_policy` | scalar | — | {`stutter-forever`, `deadlock-violation`, `finite-trace-only`, `closed`} (RFC 0015); no engine may silently change it |
| `nondeterminism[]` | `{site, class}` | both | `class` ∈ {`demonic`, `angelic`, `scheduler`, `probabilistic`, `timed`, `epistemic`}; the last three are declarative until the ADR-0016 lanes ship |
| `assurance` | `{minimum, independent_checker?, clean_recompute?, accepted_evidence_classes?}` | `minimum` | `minimum` ∈ `observed < sampled < bounded < validated < proved` (total order); the two checker flags are booleans; `accepted_evidence_classes` ⊆ the thirteen evidence kinds of `assurance-result.schema.json` (an **unordered** set) |
| `optimization` | `{hard?, soft?, non_vacuity?}` | none | three string sets with `uniqueItems`; non-vacuity is INV-012's field |
| `security_policy` | `{data_classification, redaction_classes?, capability_requirements?}` | `data_classification` | `data_classification` ∈ `public < internal < confidential < restricted` (total order); first-class per plan §0.1/§5.2, and weakening it is a protected change |
| `policy` | map over the fifteen policy keys | all fifteen | each value is one member of the closed verb set below |

- The five `assurance.minimum` levels and the thirteen `accepted_evidence_classes` are **different vocabularies that share the token `sampled`**. A level is what the intent *demands*; an evidence class is what an artifact *is*. A checker MUST NOT rank the evidence classes or derive one vocabulary from the other (RFC 0010: evidence is a partial order, not a ladder).
- `claims[].observer` is a reference to an `observers[].id`, or `null` for a claim stated without an observer binding.
- `assumptions[].classification` and `assumptions[].fidelity_profile` are optional, and absence means *undeclared*, which is distinct from every declared value: a change from absent to present, or the reverse, is a declaredness change and classifies `incomparable` (RFC 0031's rule for `classification`/`fidelity_profile` edits), never `unchanged`.
- Every array-valued group MAY be empty except `claims`, which MUST carry at least one claim. An empty `assumptions`, `observers`, `fairness`, `nondeterminism`, or `abstraction_maps` array is a *declaration that the set is empty*, and is classified as such; it is not "unspecified".

### Absence, null, and defaults

- In `bounds`, `null` denotes **unbounded** and is the top of that component's order. An absent `values` or `depth` key MUST be read as `null`. `nodes` and `faults` have no null form and no default: a change in the declaredness of either MUST classify `bounds` as `unknown` and fail closed (RFC 0031, "bounds").
- `fairness[].condition: null` means unconditional, which is the weakest condition and therefore the strongest constraint (RFC 0031's antitone rule). An absent `condition` key MUST be read as `null`.
- `scope.abstraction_level: null` means no abstraction level is declared. It is not the empty string, and the two MUST NOT be normalized to each other. An absent `abstraction_level` key MUST be read as `null` for exactly the reason `values`, `depth`, and `condition` above are: "no abstraction level is declared" is what both spellings say, and ID5 forbids "absent" and a declared value for one meaning from yielding two identities. The canonical encoding therefore writes `abstraction_level` explicitly, `null` included, whichever way the author left it.
- An absent optional set (`optimization.hard`, `security_policy.redaction_classes`, `fault_model.profiles`, an observer projection) MUST be read as the empty set for classification, and MUST be encoded explicitly as the empty set when the identity preimage is built (ID5), so that "absent" and "empty" cannot yield two identities for one meaning.
- An absent `policy_reviewers` key MUST be read the same way: as the empty object `{}`, encoded explicitly when the identity preimage is built. `policy_reviewers` is a map, not a set, but ID2 puts it in the preimage regardless, so the same "absent and empty cannot yield two identities" argument applies to it; reversing the reading — treating an absent key as distinct from `{}` — would move the `in_*` identity of every stored contract that ever omitted the key, which is exactly the silent-drift ID5 exists to forbid.
- An assurance checker flag — `assurance.independent_checker` or `assurance.clean_recompute` — has no null form and no default, exactly as `bounds`' `nodes` and `faults` do not: a change in the *declaredness* of either (declared on one side of a revision, undeclared on the other) MUST classify `assurance` as `unknown` and fail closed. RFC 0031's assurance relation has exactly three members (`unchanged`, `upgraded`, `downgraded`; "Assurance movement") and none of them names a declaredness change, so there is no direction to report, and reading the undeclared side as `false` would invent a value the schema left optional — precisely what this section forbids next.
- Beyond the absent-reads-as-null equivalences above (`bounds.values`, `bounds.depth`, `fairness[].condition`, `scope.abstraction_level`) and the absent-optional-set/map-reads-as-empty rule (optional sets and `policy_reviewers`), there are no other defaults. A checker MUST NOT invent a value for a key the schema leaves optional.

### Well-formedness

These rules are checked in addition to schema validation. A contract failing any of them MUST be rejected (`MalformedRequest`) and MUST NOT be stored, imported, or accepted. They are rules about the *document*: each relates two parts of one contract, or one part to a fact the storing lane holds, and each is discharged by editing the contract. What a *lane* must additionally require before it acts on a contract is a different question, and the next section fixes it.

- **W1 — Unique unit keys.** `claims[].id`, `assumptions[].id`, `observers[].id`, `nondeterminism[].site`, and `abstraction_maps[].role` MUST each be unique within their array, and no two `fairness` items may share a (`kind`, `action`) pair. RFC 0031 keys classification by exactly these; duplicates make the diff's unit keying ambiguous, which is a soundness hazard rather than a matter of taste.
- **W2 — Observer references resolve.** A non-null `claims[].observer` MUST name an existing `observers[].id`. A dangling reference is rejected; it MUST NOT be treated as an unbound claim.
- **W3 — Fragments are declared.** `scope.fragments` MUST be non-empty. An expression's `fragment`, when present, MUST be a member of `scope.fragments`. When `scope.fragments` names exactly one fragment, an absent `expression.fragment` denotes that fragment; when it names more than one, `expression.fragment` MUST be present, because "the intent's `scope.fragments` apply" has no single reading otherwise.
- **W4 — Fairness conditions are state formulas.** `fairness[].condition` MUST contain no `always`, `eventually`, or `leads_to` node at any depth. The schema enforces this structurally (`$defs/state_formula` over `$defs/temporal_free`); it MUST be rejected by the schema, not by prose.
- **W5 — Bound variables are bound.** Every `var` node MUST lie within the scope of a binder declaring that name, or the name MUST be a declared free variable of the model. An unbound `var` is rejected.
- **W6 — Verb applicability.** Every `policy` value MUST be well-formed for its field per the applicability matrix below. An inapplicable verb is rejected, never silently treated as `unlocked`.
- **W7 — Reviewers are named where required.** Every key of `policy_reviewers` MUST be one of the fifteen policy keys, and every field whose verb is `review` MUST have a non-empty principal list in `policy_reviewers`. A `review` verb naming no reviewer is an unenforceable block.
- **W8 — Profiles and maps resolve.** `fault_model.profiles` MUST name domain-pack profiles available in the snapshot's domain packs, and every `abstraction_maps[].map_id` MUST resolve in the §16 correspondence graph. An unresolvable reference is rejected at acceptance time; it MUST NOT be deferred into an `unknown` classification later.
- **W9 — Declared identity agrees.** `intent_id` MUST equal the identity computed from the contract's own canonical encoding (ID1–ID5).
- **W10 — Header agrees.** `schema_id` and `schema_epoch` MUST be the class identity and epoch of the schema document the instance was written against ([`../schemas/README.md`](../schemas/README.md)).

### Acceptance obligations that are not well-formedness

W1–W10 decide whether a document can be read as a contract. They do not decide whether a particular lane should act on it, and the two answers MUST stay distinguishable: a contract can be schema-valid, decodable, well formed, and still not something a given lane may use. INV-012's non-vacuity obligation is the first obligation of the second kind, and this section fixes its disposition (correction 17).

- **AO1 — An empty `optimization.non_vacuity` is well formed.** The set MAY be empty, and an empty set is a *declaration that the contract requires no behavior to remain possible* — exactly what every other empty set in the document is ("Absence, null, and defaults"; "Every array-valued group MAY be empty except `claims`"). The schema admits it (no `minItems`, no `required`), so a decoder that rejected it would reject artifacts the shape authority admits (INV-003). No W-rule rejects it, and a checker MUST NOT add one.
- **AO2 — The obligation belongs to the caller with standing, and that caller is named.** INV-012 is scoped to synthesis — "Forge objectives include required progress/availability behaviors and mutation challenges. Safety by disabling the system is rejected" (`../plan.md`, INV-012), "Every synthesis task includes positive behaviors or progress scenarios" (plan §14.5). A lane that assembles a synthesis task against a contract MUST require `optimization.non_vacuity` to be non-empty and MUST refuse the task otherwise, with a typed refusal naming INV-012. A lane that assembles no synthesis task MUST NOT impose it: a contract also governs verification, and one that requires no behavior to remain possible is a legitimate — if weak — statement about a system, not a malformed one.
- **AO3 — A well-formedness verdict is not an acceptance decision and MUST NOT be read as one.** The verdict names the rules it decided; a caller that needs both answers makes both calls. A checker MUST NOT fold AO2's obligation into the verdict behind a per-caller flag: the verdict surface fails closed on every rule it carries and has no "skip this rule" spelling, and a rule some callers switch off is a rule no reader of the verdict can rely on. Nor may the obligation be discharged by a *count*: `optimization.non_vacuity` holds opaque strings classified by membership (RFC 0031), so a non-empty set is evidence that the author declared something, never evidence that what they declared is satisfiable. Judging a declared behavior's content is Forge's (plan §14.5's "property mutation and hidden semantic variants", RFC 0033), not a checker's.
- **AO4 — Neither obligation substitutes for the change policy, and a dormant verb is not an ill-formed one.** `no-removal` on `non_vacuity` governs *revisions*: it denies the `removed` relation on a behavior a stored contract already carries (P3). It says nothing about the set's size, and an empty set does not make it ill-formed — the verb governs whatever the set comes to hold, which is the same dormant shape `no-removal` has on `faults` over an empty `fault_model.enabled` and on `optimization` over empty `hard`/`soft` sets, both of which the corpus carries today. W1–W10, AO1–AO2, and P1–P7 answer three questions about three objects: the document, the task, and the revision.

## Canonical identity

- **ID1.** The `in_*` identity MUST derive from a canonical encoding of the semantic content: sorted keys, property ASTs in the CPNF-1 normal form specified in the next section (alpha-renamed binders, flattened and ordered associative/commutative connectives), and no comment/whitespace/label content.
- **ID2.** The exclusion list is **closed**. Exactly these bytes are excluded from the preimage: `name`; `expression.source`; `expression.normal_form`; every `$comment`; and `intent_id` itself, since a self-reference cannot be part of its own preimage. `name` and human labels are metadata and MUST NOT affect identity. So are `expression.source` and every `$comment`. Nothing else is excluded — in particular the `policy` table, `policy_reviewers`, `schema_id`, and `schema_epoch` are **in** the preimage.
- **ID3.** Because the policy table is in the preimage, `intent.lock` mints a **successor** contract: its response handle is the new identity, the predecessor moves to `superseded`, and the operation stays privileged and audit-recorded (plan §5.4). A policy-only revision classifies all fifteen fields `unchanged`; it is the one revision that moves the identity with no non-`unchanged` record in `intent_changes`. How such a revision's evidence impact is computed belongs to RFC 0030 and RFC 0031, and is raised as flag F6 rather than legislated here.
- **ID4.** Because the registry record is a separate artifact, acceptance, rejection, supersession, and signature chains are **not** in the preimage. `intent.accept` MUST NOT change `in_*`; if it did, the acceptance would name a contract that no longer exists.
- **ID5.** Encoding MUST be byte-deterministic across platforms and releases within a schema version: JSON with object keys sorted ascending by Unicode code point, no insignificant whitespace, UTF-8 output, integers in shortest decimal form without sign or leading zeros, no floating-point values, and explicit encoding of empty sets (see "Absence, null, and defaults"). This is the encoding N8 fixes for property ASTs, applied to the whole document. Golden identity vectors are part of the acceptance suite.
- **ID6.** In certified lanes, identity collisions MUST be resolved by canonical comparison (ADR-0013).
- **ID7.** What changes identity is a checkable list. Renaming the contract, reformatting it, rewriting `source`, adding a `$comment`, or reordering keys MUST NOT change it. Changing any claim, assumption, observer, bound, fault, fairness constraint, completion policy, nondeterminism class, assurance requirement, optimization or non-vacuity string, security-policy element, scope element, abstraction-map binding, policy verb, or reviewer list MUST change it.
- **ID8.** Identity equality is the only admissible basis for the RFC 0031 relation `unchanged` at whole-contract granularity, and per-unit CPNF-1 equality is the only admissible basis per unit (S1). A textual or partial-structural match MUST NOT be used for either.

## Property AST and canonical normal form (CPNF-1)

Absorbed from plan §25 (SD-07). Property expressions are structured ASTs, not strings. A string carries no soundness: under textual comparison a rename or a rewrite is indistinguishable from a weakening, which is precisely the reward-hacking path INV-001 exists to close (RFC 0031 rejects token/set comparison of property strings as the production mechanism). The fields carrying an AST are `claims[].expression`, `assumptions[].expression`, and `fairness[].condition`; the structure is normative in [`../schemas/intent-contract.schema.json`](../schemas/intent-contract.schema.json), the semantics here.

The two carriers differ in shape, and the difference is normative: `claims[].expression` and `assumptions[].expression` are `property_expression` objects — `{ast, source?, fragment?, normal_form?}` — while `fairness[].condition` is a **bare** temporal-operator-free formula with no `source`, no `fragment`, and no `normal_form` declaration. A fairness condition is therefore normalized under the contract's declared fragments (W3) and has no display-only rendering that could disagree with it.

### Node set

The v1 AST is the **Finite-core property fragment**: the `Finite` fragment of ADR-0025 extended with the stuttering-invariant temporal core of RFC 0015. It is deliberately narrower than the CML surface (RFC 0003): the CML parser elaborates a CML property into this AST, and a CML construct with no image in the fragment MUST fail closed rather than be approximated.

Formula nodes:

| `kind` | Fields | Meaning |
|---|---|---|
| `boolean` | `value` | boolean constant |
| `predicate` | `name`, `args?` | state atom: a boolean state predicate or indexed boolean state variable |
| `action` | `name`, `modality` (`occurs` \| `enabled`) | action atom (RFC 0015) |
| `compare` | `op`, `left`, `right` | comparison atom over terms; `op` ∈ {`eq`, `ne`, `lt`, `le`, `gt`, `ge`, `member`, `subset`} |
| `not` | `operand` | negation |
| `and`, `or` | `operands` (≥ 2) | n-ary boolean connective |
| `implies` | `antecedent`, `consequent` | material implication |
| `iff` | `left`, `right` | bi-implication |
| `always`, `eventually` | `operand` | unary temporal operator |
| `leads_to` | `antecedent`, `consequent` | response (RFC 0008) |
| `forall`, `exists` | `binder`, `body` | quantification over a finite domain |

Term nodes: `var` (`name`), `literal` (`value`), `constant` (`name`), `state` (`name`, `indices?`), `apply` (`operator`, `args`).

Every `name`, `operator`, and binder `variable` is an `identifier` matching `^[A-Za-z_][A-Za-z0-9_.]*$`; a `binder` is `{variable, domain}` whose `domain` is a term, normally a declared model constant; a `literal` value is a boolean, integer, string, or `null`.

- `next` MUST NOT appear. The theorem-preserving temporal core is LTL without `next` because stuttering refinement is central (RFC 0015); admitting `next` would make step-count refactorings look like semantic changes and make stuttering-invariant refinements unprovable.
- Recurrence and persistence MUST be spelled `always(eventually φ)` and `eventually(always φ)`. There is no separate node kind, so the two spellings cannot diverge.
- Probabilistic, timed, and epistemic properties are NOT in the v1 AST; they arrive with the ADR-0016 lanes and require a revision of this section.
- Floating-point literals are excluded from v1 so the canonical encoding stays byte-deterministic.
- The term sort is intentionally small. Arithmetic and set operations are expressed as `apply` over declared model operators; no algebraic law is assumed for an operator whose laws are not declared, so CPNF-1 never reorders `args`.

### CPNF-1 normalization

`CPNF-1` (Continuum Property Normal Form, version 1) is the canonical form of a Finite-core AST. `normalize` is a total function on ASTs in the fragment. Rules N1–N7 are applied bottom-up and re-applied to fixpoint; N8 then encodes the result.

- **N1 — Derived-operator elimination.** `implies(p, q)` MUST become `or(not p, q)`; `iff(p, q)` MUST become `and(or(not p, q), or(p, not q))`; `leads_to(p, q)` MUST become `always(or(not p, eventually q))`. A response written as a `leads_to` node and the same response written out by hand therefore normalize identically.
- **N2 — Negation-normal form.** `not` MUST be pushed inward to the atoms: `not not φ` → `φ`; `not and(…)` / `not or(…)` by De Morgan; `not always φ` → `eventually not φ`; `not eventually φ` → `always not φ`; `not forall b. φ` → `exists b. not φ` and dually. After N2, `not` occurs only directly above a `boolean`, `predicate`, `action`, or `compare` atom.
- **N3 — Associativity flattening.** A nested `and` inside an `and` (respectively `or` inside `or`) MUST be spliced into the parent's `operands`, so every maximal same-kind junction is one n-ary node.
- **N4 — Commutative ordering and idempotence.** The `operands` of `and` and `or` MUST be sorted ascending by the byte ordering of their own N8 encodings, and adjacent duplicates MUST be removed. A junction left with a single operand MUST be replaced by that operand.
- **N5 — Binder canonicalization.** Each bound variable MUST be renamed to `v<d>`, where `d` is the number of binders enclosing its own binder (a de Bruijn level), and every `var` reference MUST be updated. Free variables keep their declared names. De Bruijn levels depend only on binder nesting, never on sibling order, so N5 and N4 do not interfere. Alpha-equivalent properties therefore have one normal form, and renaming a bound variable cannot change identity.
- **N6 — Comparison canonicalization.** `gt` and `ge` MUST be rewritten by swapping operands into `lt` and `le`, so only `eq`, `ne`, `lt`, `le`, `member`, `subset` survive. The operands of the commutative `eq` and `ne` MUST be ordered by their N8 encodings. A `not` above an `eq` MUST become `ne` and vice versa. A `not` above `lt`/`le` MAY be absorbed by complementing the operator only where the compared sort is declared totally ordered; otherwise the `not` MUST remain. A `not` above `member`/`subset` MUST remain.
- **N7 — Boolean and temporal simplification.** Unit and absorption laws MUST be applied: `and` containing `false` → `false`; `true` operands dropped from `and`; `or` dually; `always(always φ)` → `always φ`; `eventually(eventually φ)` → `eventually φ`; `eventually(always(eventually φ))` → `always(eventually φ)` and dually; `always(true)` → `true`. Quantifier collapse is one-sided: `forall b. true` → `true` and `exists b. false` → `false` MUST be applied, but `exists b. true` and `forall b. false` MUST NOT be collapsed, because they depend on the domain being non-empty.
- **N8 — Canonical encoding.** The normalized AST MUST be encoded as JSON with object keys sorted ascending by Unicode code point, no insignificant whitespace, UTF-8 output, integers in shortest decimal form without sign or leading zeros, and no floating-point values. `source`, `normal_form`, `$comment`, and every label or comment field MUST be excluded from the encoding. This encoding is what the `in_*` identity above hashes and what N4 and N6 order by.
- **N9 — Idempotence and determinism.** `normalize(normalize(φ))` MUST equal `normalize(φ)` byte for byte, and `normalize` MUST be byte-deterministic across platforms and releases within a schema version. An expression MAY declare `normal_form: "cpnf-1"`; a checker MUST verify the claim by re-normalizing and MUST reject a contract whose declaration does not hold.

### Soundness of the classification built on CPNF-1

- **S1 — Soundness.** Every rule N1–N7 is meaning-preserving, so `normalize(P) = normalize(Q)` implies `P` and `Q` denote the same set of behaviors. RFC 0031 MAY classify a property change `unchanged` on exactly this equality and on nothing weaker. A textual match MUST NOT be used, and neither MUST a partial structural match.
- **S2 — Incompleteness is the safe side.** CPNF-1 is sound but NOT complete: semantically equivalent properties MAY still have distinct normal forms — quantifiers of the same kind are not reordered, `apply` arguments are not reordered, and propositional tautologies beyond N7 are not recognized. Distinct normal forms MUST NOT be read as a direction. The diff MUST discharge the implication obligation for the fragment (in `Finite`, exact language inclusion over the bounded universe; in `Symbolic`, an SMT or Lean obligation) or classify `unknown` and fail closed (RFC 0031). A false `unknown` costs a review; a false `unchanged` costs the invariant.
- **S3 — Fragment scope.** N1–N8 are meaning-preserving in every fragment that interprets these operators standardly, so identity and the `unchanged` classification MAY use CPNF-1 outside `Finite`. The *directional* relations `strengthened` and `weakened` MUST NOT be: they require the fragment's own decision procedure, and outside the intent's declared `scope.fragments` the classification is `unsupported`, never a guessed direction. Normalization rules for the `Symbolic`, `Temporal`, and `Probabilistic` fragments remain open (below).

### Migration from the bare `expression` string

- The retired bare string form is no longer accepted by the schema. Migration MUST parse each string into an AST within the declared fragment and MUST retain the original text in `expression.source`.
- `source` is display-only. It MUST NOT contribute to the `in_*` identity or to any RFC 0031 classification; where `source` and `ast` disagree, `ast` is authoritative and tools SHOULD regenerate `source` from the normalized AST.
- A string that cannot be parsed into the fragment MUST fail closed: the contract is rejected, never stored with a guessed or partial AST.
- Migrating a stored contract changes its canonical encoding and therefore its `in_*` identity. A migrated contract MUST be re-accepted through `intent.accept` and MUST NOT be treated as an ordinary edit. The cross-schema `$id`/schema-epoch convention that carries such a migration uniformly is settled in [`schemas/README.md`](../schemas/README.md) (plan §25, SD-08): a contract declares the `schema_id`/`schema_epoch` header it was written against, and the epoch advance that obsoletes it MUST publish the typed `Preserved | Revalidate | Incompatible` statement for the intent-contract class before it is applied.

### Worked example

```text
source   always (forall e in Events: acknowledged[e] => durable[e])

authored always(forall e in Events. implies(acknowledged(e), durable(e)))
N1       always(forall e in Events. or(not acknowledged(e), durable(e)))
N2       (already in negation-normal form)
N5       always(forall v0 in Events. or(not acknowledged(v0), durable(v0)))
N4       operands of the `or` sorted by their N8 encodings
N8       {"kind":"always","operand":{"binder":{...},"body":{...},"kind":"forall"}}
```

Rewriting the same claim as `not(exists e in Events: acknowledged[e] and not durable[e])`, or renaming `e` to `x`, or swapping the two disjuncts, yields the identical N8 encoding and therefore classifies `unchanged`. Replacing `durable` with a weaker predicate does not, and must then be proved or classified `unknown`.

## Change policy

Each protected field carries one verb:

| Verb | Meaning | Denied relations | Acceptance authority |
|---|---|---|---|
| `unlocked` | ordinary edits allowed | none | ordinary (still `revise-intent` to accept) |
| `proposal-only` | agents may propose; humans accept | none | a human principal MUST perform `intent.accept` |
| `review` | any change requires named-reviewer approval | none | approval by a principal named in `policy_reviewers` for that field |
| `locked` | no change without policy amendment | every non-`unchanged` relation | a prior `intent.lock` amendment |
| `no-decrease` | changes classified as decrease (bounds) are blocked | `contracted` | ordinary |
| `no-removal` | removals (faults, non-vacuity behaviors) are blocked | `removed` | ordinary |
| `no-downgrade` | assurance downgrades are blocked | `downgraded` | ordinary |
| `no-expansion` | growth of the set (opaque boundaries) is blocked | `expanded` | ordinary |

The verb set is closed. A proof-gated acceptance verb (`proof-required`) is a candidate future addition and requires an explicit revision of this RFC (see Open questions); plan §5.4's policy examples use only closed-set verbs.

- The protected set is: properties, assumptions, observers, bounds, faults, **fairness**, **trust boundaries**, **non-vacuity**, **completion policy**, assurance, **security policy**, **optimization**, **scope**, **nondeterminism**, and **abstraction maps** (ADR-0039; all fifteen MUST be lockable — plan §5.4).
- **Protection is structural; the verb is additional.** All fifteen appear in `intent_changes` whatever their verb (RFC 0031: `protected` is `const: true`). `unlocked` does not mean "an ordinary task may edit this field": it means the verb contributes no further block to the verdict. The default agent repair capability profile MUST deny every protected change regardless of verb; a change slipped into a repair reclassifies the transaction as an intent revision (RFC 0032, INV-011).
- Directional verbs (`no-decrease`, `no-removal`, `no-downgrade`, `no-expansion`) are only enforceable where RFC 0031 defines the order for that field. Where the order is undefined or the change is outside declared fragments, the change classifies `unknown` — or `unsupported`, or `incomparable` — and MUST be treated as blocked pending review (fail closed, plan §5.3).

### Verb applicability

A directional verb is well-formed on a field only where the denied relation is admissible for that field *and* denying it is the protective direction. The matrix is closed, and W6 rejects anything outside it.

| Policy key | Admissible verbs |
|---|---|
| `properties`, `assumptions`, `observers`, `abstraction_maps`, `scope`, `fairness`, `completion_policy`, `nondeterminism` | `unlocked`, `proposal-only`, `review`, `locked` |
| `bounds` | the four above plus `no-decrease` |
| `trust_boundaries` | the four above plus `no-expansion` |
| `assurance` | the four above plus `no-downgrade` |
| `faults`, `non_vacuity`, `optimization`, `security_policy` | the four above plus `no-removal` |

- `no-decrease` on `trust_boundaries` would block the *safe* direction — shrinking the opaque and trusted sets — and is ill-formed there, as is `no-downgrade` outside `assurance`, which is the only field that emits `downgraded`.
- The closed set contains **no verb that blocks `added`**. Adding an assumption or a fairness constraint is environment-strengthening and is the canonical gaming vector (plan §5.1, docs/50), so those fields are protected through `review` or `locked`, never through a directional verb. The same holds for observer `coarsened` and property `weakened`. Whether a `no-addition` verb should join the closed set is an open question below.

### Enforcement

Given a proposed revision and its RFC 0031 classification, the verdict is computed as follows. The daemon MUST perform this computation server-side; a client-supplied verdict is never trusted.

- **P1.** Classify all fifteen fields (RFC 0031). A partial classification MUST NOT reach a verdict.
- **P2.** For each record, if the relation is non-affirmative (`unknown`, `unsupported`, `incomparable`), the record contributes at least `review`, and contributes `block` when the field's verb is `locked`. This holds for every field and every verb — the fail-closed rule is not a property of the directional verbs alone.
- **P3.** Otherwise, if the relation is in the verb's denied set, the record contributes `block`.
- **P4.** Otherwise the verb's acceptance authority applies: `unlocked` contributes `allow`; `proposal-only` contributes `allow` only on a path where a human principal performs `intent.accept`, and `review` otherwise; `review` contributes `review` for any non-`unchanged` relation and names the reviewers from `policy_reviewers`; `locked` contributes `block` for any non-`unchanged` relation.
- **P5.** The verdict is the join of the record contributions in the total order `allow < review < block`. One non-affirmative relation on one protected field is sufficient to forbid `allow` (RFC 0031).
- **P6.** `policy.reasons` MUST name the field and the relation of every record that forbade `allow`. Reasons are a rendering of typed records and MUST NOT be the only place a blocking fact appears (INV-003).
- **P7.** The verdict MUST be recomputed at `intent.accept` time against the registry's current state, never reused from the diff produced at `intent.propose_revision` time. A verdict computed against a base that has since been superseded is stale, and accepting on it is a time-of-check/time-of-use hole; `intent.accept` carries `PolicyGateFailed` for exactly this outcome.

### Policy joins

Verbs form a lattice under the product order on (denied relations, acceptance authority), with authority ordered `ordinary < proposal < named-reviewer < policy-amendment`: `v₁ ⊑ v₂` iff `denied(v₁) ⊆ denied(v₂)` and `authority(v₁) ≤ authority(v₂)`. `unlocked` is the bottom and `locked` is the top.

- The closed verb set is **not** closed under join. `no-decrease ⊔ review` is `({contracted}, named-reviewer)`, which no closed-set verb names.
- A three-way merge (M3d) whose per-field policy join is unrepresentable MUST produce a `conflict` node. Rounding up to `locked` is sound but silently changes governance, so it MUST NOT be applied automatically; rounding down to either input is unsound and MUST NOT be applied at all.
- Where the join *is* representable, the merged table takes it. This is not limited to "one side `unlocked` and the other anything, or two identical verbs" — that phrasing is illustrative, not exhaustive, and undersells the product-order definition two paragraphs up. In particular: `locked ⊔ v = locked` for **every** `v`, because `locked` denies every non-`unchanged` relation and holds the top authority, so no verb's denied set or authority can exceed it; and `review ⊔ proposal-only = review`, because both deny nothing and `named-reviewer` is the greater of the two authorities. A merge MUST NOT produce a policy table strictly more permissive than either input on any field.

## Drafting and acceptance

- `cargo continuum init` MAY propose draft contracts from templates, property libraries, and observed effect footprints. Drafts enter the registry at status `proposed` and gain INV-001 protection only on explicit human acceptance (plan §3.1).
- The `proposed` → protected transition is performed only by the `intent.accept` operation (RFC 0027): it requires the `revise-intent` capability, produces an audit record, and is never performed implicitly by `init`, `check`, or any repair operation (plan §5.4). `intent.lock` edits the §5.4 policy table under the same capability.
- Agent-facing installations MUST omit `revise-intent` and `promote` from default capability profiles (RFC 0027). A capability MAY be scoped to named intents (`CapabilityDescriptor.intents`); an acceptance outside that scope fails with `CapabilityDenied` before any semantic work runs.
- Continuum MUST NOT silently promote an inferred intent to protected status, and MUST NOT claim a generated model is the intended abstraction (docs/37).

## Revision procedure

- **R1.** A revision is submitted against a named base contract version (`intent.propose_revision`, request field `base`), carrying an `IntentChangeSet` whose `rationale` is reviewer text and untrusted data (INV-016).
- **R2.** The proposal is materialized as a new contract at status `proposed`. It MUST NOT mutate the base, and it MUST NOT be reachable as the governing intent of any snapshot until it is accepted.
- **R3.** RFC 0031 classifies every field change; undecidable cases yield `unknown`, cases outside declared fragments `unsupported`, and cases whose inclusions are both refuted `incomparable`.
- **R4.** Policy verbs are evaluated against the classification per P1–P6; any blocked or non-affirmative protected change requires the review path.
- **R5.** Concurrency is compare-and-set on the base identity. If the base is no longer the registry head for that lineage, `intent.accept` MUST fail rather than merge implicitly; the proposer rebases onto the new head and reclassifies. Two revisions against one base are a three-way merge (M1–M7), not a race won by arrival order.
- **R6.** On acceptance the verdict is recomputed (P7), the acceptance record and chain are verified (A1–A4), a new `in_*` identity is issued, the predecessor moves to `superseded`, and the supersession edge is recorded in the evidence graph.
- **R7.** Dependent evidence is invalidated per the impact set (RFC 0031, RFC 0030). Invalidation is a statement about evidence, not a downgrade of a claim: a claim is retired by `Superseded` with an explicit edge, never by writing a lower status (RFC 0031, "Evidence-status interaction").
- **R8.** `intent.reject` records a typed reason and leaves the proposal in the registry; a rejected proposal MUST NOT be deleted, because the rejection is the audit trail.
- **R9.** Every step that mutates the registry is audit-recorded with actor, capability, inputs, and decision (plan §18.5). An audit record that cannot be written is a failure of the operation, not a detail to skip.

## Corrections recorded by this RFC

Per plan §25, where plan prose, docs, or a dependent artifact disagrees with this RFC, this RFC governs. The corrections in force:

1. **Case of the non-affirmative relation tokens.** Earlier revisions of this RFC wrote `Unknown` in the change-policy and revision sections. Normative: the relation vocabulary's wire literals are lowercase — `unknown`, `unsupported`, `incomparable` — as the semantic-diff schema's `relation` enum and RFC 0031 record. Direction: this RFC is corrected to agree with RFC 0031 and that schema. The rule is scoped to that vocabulary: `scope.fragments` members (`Finite`, `Symbolic`, `Temporal`, `Probabilistic`, `Theorem`, `Runtime`) and the `Preserved | Revalidate | Incompatible` compatibility statement are capitalized in their own schemas and stay capitalized.
2. **Case of the registry status tokens.** This RFC wrote `Proposed` for registry status. Normative: the wire literals are `proposed`, `accepted`, `superseded`, per `intent-registry-record.schema.json`. Direction: this RFC is corrected to agree with its second normative schema. Prose naming the concept ("a proposal") is not a wire token.
3. **Case of the merge conflict node.** Plan §4.2.1 and this RFC wrote `Conflict` node. Normative: the evidence-graph node kind is the lowercase `conflict` of `evidence-graph-node.schema.json`; plan §11.2's `Conflict` is the concept name. Direction: RFC governs prose; no schema change requested.
4. **Field-group names versus policy and diff names.** This RFC's protected-set list mixed contract-group names (`claims`, `fault_model`, `optimization.non_vacuity`) with policy names. Normative: the fifteen *policy* keys are both the policy-table keys and the diff `field` vocabulary, related to contract paths by the schema's `x-policy-field-map` — `properties ↦ claims`, `faults ↦ fault_model`, `non_vacuity ↦ optimization.non_vacuity`. Direction: RFC corrected to agree with the schema and RFC 0031.
5. **`optimization` is governed by two verbs.** Plan §5.2 lists "optimization: hard constraints, soft objectives, non-vacuity" as one field. Normative: one contract group, two policy keys, two verbs; a non-vacuity removal MUST NOT be reported or governed as an `optimization` change (INV-012). Direction: RFC governs plan prose.
6. **`assurance` carries a required minimum.** Plan §5.2's "assurance policy: accepted evidence classes and required checkers" omits the field's ordered core. Normative: `assurance.minimum` is REQUIRED and totally ordered, and `accepted_evidence_classes` is an unordered set that MUST NOT be ranked. Direction: RFC governs plan prose.
7. **Claims carry a `kind`.** Plan §5.2's "claims: property AST + stable semantic name" omits it. Normative: a claim is `{id, kind, expression, observer?}` with `kind` from the closed six-member enum, and a `kind` change with an unchanged expression classifies `incomparable` (RFC 0031). Direction: RFC governs plan prose.
8. **Fairness conditions are bare state formulas.** This RFC previously listed `fairness[].condition` beside the two `property_expression` fields without distinguishing their shapes. Normative: `condition` is a bare temporal-operator-free formula with no `source`, `fragment`, or `normal_form`, and `null` means unconditional. Direction: RFC corrected to agree with the schema.
9. **`intent.lock` mints a successor identity.** The policy table is inside the contract document and inside the identity preimage (ID2), so a lock cannot leave `in_*` unchanged. Normative: `intent.lock`'s response handle is the successor, and the predecessor is `superseded`. Direction: RFC fixes the reading of an IDL response the IDL does not constrain; no IDL change requested.
10. **Bounds and faults relation names.** Plan §5.3's "bound increase/decrease" and "fault envelope expansion/contraction" are informal; the normative relations are `expanded`/`contracted` for bounds and `added`/`removed` per item for faults. Direction: RFC 0031 governs, this RFC agrees, and the verb name `no-decrease` is an alias for "blocks `contracted`".
11. **Protected does not mean locked.** A reading in which `unlocked` fields sit outside INV-001 is incorrect. Normative: all fifteen are classified, appear in `intent_changes`, and require `revise-intent` to accept; the verb governs only the additional block. Direction: RFC clarifies; no plan contradiction.
12. **The `observers` field table counted five components, not four.** "Event families plus the four projection kinds of plan §5.2" counts `events` twice — once as "event families" and once inside "the four projection kinds" — for a total of five, where plan §5.2, `intent-contract.schema.json`, and RFC 0031's comparison rule ("the four sets `events`, `state_projection`, `knowledge_projection`, `security_projection`") all count four. Normative: an observer carries exactly four string sets, of which `events` is one. Direction: this RFC is corrected to agree with the schema, RFC 0031, and plan §5.2 (INV-003: schema decides shape; found by `crates/continuum-intent/src/observers.rs`).
13. **The "Policy joins" illustration was narrower than its own definition.** "One side `unlocked` and the other anything, or two identical verbs" read as an enumeration of every representable join, and it is not one: `locked ⊔ v = locked` for every `v`, and `review ⊔ proposal-only = review`, both representable under the product-order definition the illustration follows. Direction: this RFC's illustration is extended to match its own definition; `crates/continuum-intent/src/change_policy.rs`'s `PolicyVerb::join` computes both cases already.
14. **An assurance checker flag's declaredness change classifies `assurance` as `unknown`.** Neither this RFC nor RFC 0031 previously stated what a change in the *declaredness* of `independent_checker` or `clean_recompute` classifies, though the analogous rule was already normative for `bounds`' `nodes`/`faults` (a declaredness change classifies `bounds` as `unknown` and fails closed) and for `assumptions[].classification`/`fidelity_profile` (a declaredness change classifies `incomparable`). Normative: a declaredness change on either assurance checker flag classifies `assurance` as `unknown` and fails closed, because RFC 0031's three-member assurance relation names no declaredness case. Direction: this RFC states the rule the implementation (`crates/continuum-intent/src/assurance_policy.rs`, `AssuranceComparisonError::CheckerDeclarednessChanged`) already carries; RFC 0031's "Assurance movement" section gains the same rule as a companion bullet in the same change.
15. **Absence reads as `null` for `scope.abstraction_level`, and this section's "no other defaults" line overclaimed.** The section gave `null` and absence the same meaning for `scope.abstraction_level` without saying that an *absent* key reads as `null` — the same equivalence already stated for `bounds.values`, `bounds.depth`, and `fairness[].condition` — while separately asserting "there are no other defaults", which is only true once that equivalence (and the `policy_reviewers` one, correction 16) is included in "the above". Direction: this RFC is extended to state the equivalence explicitly and to scope its own closing claim correctly; found by `crates/continuum-intent/src/contract.rs` (`Scope::artifact_json`).
16. **An absent `policy_reviewers` key reads as `{}`.** `policy_reviewers` is a map, not a set, so the "absent optional set reads as empty" rule did not literally cover it, though ID2 puts it in the identity preimage on the same footing. Normative: absence and `{}` are one meaning with one encoding, exactly as for the optional sets. Direction: this RFC is extended to state the reading outright, because reversing it would move the `in_*` identity of every contract that ever omitted the key; found by `crates/continuum-intent/src/contract.rs`, which already encodes it this way.
17. **Non-vacuity is absent from the cross-field verdict surface, and that absence is the design.** `crates/continuum-intent/src/optimization.rs` raised a flag against this RFC: a contract whose `policy.non_vacuity` is `no-removal` while `optimization.non_vacuity` is empty "carries a lock over nothing, which is the same unenforceable-block shape W7 rejects for a `review` verb naming no reviewer". This RFC said nothing about either that pair or bare emptiness, and `IntentContract::check` decides five cross-field rules (W2, W3, W7, W8, W9) of which none concerns non-vacuity — so a caller trusting a clean verdict alone accepts a "safety by disabling" contract with no signal (pinned by `crates/continuum-intent/tests/inv012_nonvacuity_evidence.rs`). Normative: the two-call contract is ratified, and "Acceptance obligations that are not well-formedness" (AO1–AO4) states it. The candidate W-rule is declined, and the flag is answered rather than left standing: the W7 analogy does not hold, because a `review` verb naming no reviewer is a block that *fires on the first change and cannot be discharged*, while `no-removal` over an empty set is a block that has *nothing to guard yet* — the identical dormant shape the corpus's own die-hard contract carries twice, on `faults` over an empty `fault_model.enabled` and on `optimization` over empty `hard`/`soft`. Direction: this RFC is extended to state the disposition and the alternatives it declines (below); `src/optimization.rs`'s flag becomes a citation of it (bn-32kpe).

Flags raised against artifacts this RFC does not own (no silent divergence):

- **F1 — Unit keys are not unique by construction.** `intent-contract.schema.json` sets no uniqueness constraint on `claims[].id`, `assumptions[].id`, `observers[].id`, `nondeterminism[].site`, `abstraction_maps[].role`, or fairness (`kind`, `action`) pairs, though RFC 0031 keys classification by exactly those. W1 states the rule; expressing it structurally belongs to the schema sweep.
- **F2 — Referential integrity is not expressible.** `claims[].observer → observers[].id`, `fault_model.profiles →` domain packs, and `abstraction_maps[].map_id →` §16 maps are checker rules (W2, W8) that the schema cannot state.
- **F3 — `policy_reviewers` is unconstrained.** Its keys are `additionalProperties` rather than the fifteen policy keys, and nothing ties a `review` verb to a non-empty principal list (W7).
- **F4 — Any verb validates on any field.** `policy_verb` is one enum applied to all fifteen keys, so the applicability matrix (W6) is prose today; per-field verb enums would make it structural.
- **F5 — `scope.fragments` has no `minItems`.** An empty declaration would make every unit `unsupported` and every promotion blocked — fail-closed, but silently (W3).
- **F6 — Policy-only revisions have no impact-set rule.** A revision that changes only the policy table moves the identity with every field `unchanged`. Whether the dependent evidence is re-addressed through the `SUPERSEDES` edge or conservatively invalidated is RFC 0030 and RFC 0031 business, and is unstated there: RFC 0030 fixes the converse case — a fixed `in_*` read under a changed intent epoch, which enters the query key — but not a moved `in_*` with no changed field.
- **F7 — No intent-bundle schema.** `inb_*` is a registered artifact class (plan §4.4) with an IDL handle, but `schemas/` carries no `intent-bundle.schema.json`; the bundle contents above are prose where the dossier's own rule is that schemas decide shape.
- **F8 — `IntentChangeSet.changes` is `Opaque`.** The wire cannot type which fields a proposal touches; it carries the whole proposed contract form instead. This is the sibling of RFC 0031's F1 (per-unit locators in diff records).
- **F9 — No rebind operation, and `workspace.create` takes an intent at `propose` authority.** The lineage rule in "Storage and custody" is a daemon obligation the IDL cannot express today; RFC 0026 owns that surface.
- **F10 — The registry record carries no lineage.** `intent-registry-record.schema.json` has no predecessor or `superseded_by` field, so the common ancestor the three-way merge requires (M1) is not recoverable from the record alone.
- **F11 — Prose case in the contract schema.** The `scope.fragments` description writes "classification is Unknown"; the wire literal is lowercase (correction 1). No shape impact; raised for the schema sweep.
- **F12 — No landed lane discharges the non-vacuity obligation.** AO2 names the caller with standing — the lane that assembles a synthesis task — and `continuum-forge` is still a PR-1/IMPL-01 scaffold, so nothing outside tests calls `Optimization::require_non_vacuity` today. The obligation is implemented and unreferenced until the Forge lane lands. RFC 0033 owns the surface that must call it and the typed refusal that surface returns; this RFC fixes only that the requirement exists and where it does not belong.

## Rejected alternatives

- **Intent embedded in the workspace snapshot.** Rejected: forkable containers cannot protect their contents (research/33 principle 1; the fork-with-modified-intent gaming path).
- **Free-form policy strings.** Rejected: directional enforcement requires a closed verb set with defined orders.
- **Identity from source text.** Rejected: comment/formatting changes must not invalidate evidence; renames must not evade the diff.
- **Policy table outside the identity preimage.** Rejected: a governance change that left the content identity untouched would let two contracts with different lock tables share one `in_*`, and would make a lock unauditable by content address.
- **Automatic resolution of divergent registries by timestamp, branch precedence, or write order.** Rejected: each resolves a semantic disagreement by a non-semantic tiebreak, which is the merge-shaped form of the reward-hacking path INV-001 closes.
- **Rounding an unrepresentable policy join up to `locked`.** Rejected as an automatic step: it is sound but silently changes governance, and the conflict is the honest result.
- **A merged contract inheriting the accepted status of its heads.** Rejected: acceptance is a statement about one contract identity, and a merge produces a contract nobody accepted.
- **Reusing the propose-time verdict at acceptance.** Rejected: the base may have been superseded in between, so the check would be against a state that no longer exists.
- **A W-rule making a non-empty `optimization.non_vacuity` a well-formedness condition** (correction 17). Rejected on two independent grounds. It imposes a synthesis-scoped obligation — INV-012 speaks of "Forge objectives", plan §14.5 of "Every synthesis task" — on every contract, including the verification-only contracts the obligation does not reach, and whether a contract will govern a synthesis task is not a fact the document carries. And it would put a *presence count* in a surface whose other rules are exact: the set holds opaque strings classified by membership, so `"x"` discharges the rule exactly as a real progress behavior does, and a reader who trusted the verdict would be told "non-vacuous" on the strength of a one-character string. The trap would move rather than close.
- **A W-rule flagging the incoherent pair — `no-removal` on `non_vacuity` over an empty set — rather than bare emptiness.** Rejected: the reasoning does not survive generalization. The identical pair is admissible on every field the applicability matrix admits `no-removal` on (`faults`, `non_vacuity`, `optimization`, `security_policy`), and the corpus's die-hard contract carries two of them as ordinary dormant policy (AO4). A rule that fired on `non_vacuity` alone would be naming INV-012's concern by a governance coincidence rather than by its substance, and it is evadable in precisely the wrong direction: an author who writes `non_vacuity: unlocked` beside an empty set passes it, so the rule would reward removing the lock along with the obligation.
- **A W-rule conditioned on claim kinds — safety-only claims MUST declare a non-vacuity behavior.** Rejected: `claims[].kind` is authored metadata this RFC deliberately does not derive from the expression (correction 7: a `kind` change with an unchanged expression classifies `incomparable`), so a safety claim labelled `liveness` would discharge the rule and the gate would rest on an unverified label. The sound version of the same idea — deciding whether a contract's own claims are satisfiable by a system with no behaviors — needs the fragment's decision procedure, which S2 and S3 keep out of the cross-field checker.
- **A per-caller switch folding the obligation into the verdict when a caller asks for it.** Rejected: the verdict surface fails closed on every rule it carries and has no "skip this rule" spelling (AO3). A rule some callers disable weakens the rules that are unconditional, because the verdict's meaning would then depend on how it was requested.

## Open questions

- Property-AST normalization rules per fragment beyond Finite (owned jointly with RFC 0031). CPNF-1 above fixes the Finite core; `Symbolic`, `Temporal`, and `Probabilistic` normalization is open, and until it is settled those fragments get identity and `unchanged` from CPNF-1 but no directional relation.
- Whether the v1 term sort grows a declared-algebraic-law table, so `apply` arguments over declared-commutative or declared-associative operators can be ordered by CPNF-1 instead of compared positionally.
- Whether `optimization.non_vacuity` behaviors and observer projections, still carried as strings, move to typed forms; RFC 0031 classifies them by set membership only, so no expression order is needed today.
- Whether a proof-gated acceptance verb (`proof-required`) joins the closed verb set; adding it requires a revision of this RFC, and plan §5.4's examples use only closed-set verbs until then.
- Whether a `no-addition` verb is needed for `assumptions` and `fairness`, whose gaming direction is `added` and which today rely on `review`/`locked`. Adding it is a closed-set change with the same revision cost.
- Whether acceptance signatures need revocation and expiry semantics, and how a revoked signer affects contracts already accepted under its key (with RFC 0026's capability model).
- Whether the intent bundle earns its own schema and example instance (F7), and whether the registry record gains lineage (F10); both are schema-sweep decisions rather than RFC decisions.

(Resolved: `security_policy` is a first-class field group, carried by the
schema, lockable per plan §5.4, and diffed per RFC 0031's security-policy
order.)

## Acceptance

- Round-trip schema validation of [`../schemas/examples/intent-contract.example.json`](../schemas/examples/intent-contract.example.json); deterministic identity across platforms (golden vectors).
- CPNF-1 golden vectors: `normalize` is idempotent (N9) and byte-identical across platforms and releases.
- A rewrite corpus — `implies`/`leads_to` expansion, De Morgan pushes, bound-variable renaming, junction reordering and duplication, `gt`/`ge` swaps — MUST normalize to identical CPNF-1 encodings and classify `unchanged`; a paired weakening corpus MUST NOT classify `unchanged`, and MUST classify a proved direction or `unknown`.
- Identity vectors for the ID2 exclusion list: renaming the contract, rewriting `source`, adding `$comment`s, and reordering keys leave `in_*` fixed; changing any policy verb, reviewer list, or protected-field value changes it.
- Fairness conditions carrying a temporal operator MUST be rejected by the schema, not by prose.
- Well-formedness vectors for W1–W10, each rejected with `MalformedRequest`: duplicate claim ids, a dangling `claims[].observer`, an empty `scope.fragments`, an absent `expression.fragment` under two declared fragments, an unbound `var`, an inapplicable verb (`no-decrease` on `properties`), a `review` verb with no reviewers, and a `schema_epoch` the reader does not implement.
- Non-vacuity vectors (AO1–AO3): a contract whose `optimization.non_vacuity` is emptied is schema-valid, decodes, and produces the *same* well-formedness verdict as the contract that declares the behavior — including a clean verdict where the rest of the document is clean; the obligation is a separate call and fails with a typed error naming INV-012.
- All G0-DX-02 gaming mutations classify as protected changes; a source-only guard repair does not.
- Policy-verb enforcement tests for all fifteen protected fields, including `unknown`-fails-closed, the P5 join over mixed records, and the P7 recomputation of a verdict whose base was superseded between propose and accept.
- Registry-status tests: every transition of the status table, and every non-transition rejected — including `accepted → proposed`, any write to a `superseded` record, and a status lowering under concurrency.
- Convergence vectors: independent merges converge automatically and order-independently (M6); overlapping unit keys, non-affirmative relations, and unrepresentable policy joins each produce a `conflict` node requiring `revise-intent`; a merged contract enters at `proposed`.
- Acceptance-chain vectors: a chain that does not verify and an absent referenced bundle both yield `AcceptanceChainInvalid`; a verifying chain with an untrusted signer imports at `proposed`; an acceptance replayed onto a different base is rejected (A2).
- Adversarial suite: renaming, formatting, comment, and label changes MUST NOT change identity; semantic changes MUST. A snapshot created against a different intent than its lineage predecessor MUST be classified as a rebind, and its evidence MUST NOT be reported under the predecessor's intent.
