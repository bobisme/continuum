# RFC 0031: Semantic and Intent Diff

## Status
Draft for implementation.

**Target gate:** G3 (intent integrity)
**Owners:** intent/diff leads
**Normative language:** MUST/MUST NOT/SHOULD/SHOULD NOT/MAY per RFC 2119.
**Normative schema:** [`../schemas/semantic-diff.schema.json`](../schemas/semantic-diff.schema.json). Where this document and the JSON schema disagree, the schema is corrected or this RFC is amended by an explicit revision; neither drifts silently.
**Normative wire vocabulary:** [`../schemas/continuumd-native-protocol.idl`](../schemas/continuumd-native-protocol.idl) (RFC 0026) — `DiffHandle`, `DiffLayer`, `PolicyDecision`, `PolicyVerdictValue`, `StructuralOutcome`.
**Companion normative sources:** [RFC 0037](0037-intent-contract.md) (contract fields, CPNF-1 rules N1–N9, soundness statements S1–S3, the closed policy-verb set), [RFC 0030](0030-incremental-semantic-query-engine.md) (query key, dependency classes and reasons), [RFC 0032](0032-repair-transaction-protocol.md) (gate 3 and promotion-time recomputation), [`../plan.md`](../plan.md) §5.3/§5.4, [`../docs/40_SEMANTIC_DIFF_AND_IMPACT.md`](../docs/40_SEMANTIC_DIFF_AND_IMPACT.md).
**Landed vocabulary:** `crates/continuum-value/src/assurance.rs` (`AssuranceLevel`, `AssuranceRequirement::change_to`, `AssuranceChange`, `AssuranceChange::blocked_by_no_downgrade`) and `crates/continuum-evidence/src/claim_status.rs` (`ClaimStatus`, `StatusTransition`, `StatusTransition::is_regression`) both cite this RFC as the source of the assurance order. This RFC and those modules MUST be revised together; neither may move alone.

## Summary

The diff engine is the anti-reward-hacking core (plan B1, INV-001, INV-011, G0-DX-02). Given two snapshots and two intent versions, it classifies every change, computes the evidence impact set, and produces the policy verdict that gates ordinary versus privileged promotion. The diff artifact this RFC defines carries the `diff_*` handle prefix (plan §4.4 registers it); the normative schema pattern-enforces `^diff_` identities.

This RFC is the normative home of the classification lattice, of the per-field orders the RFC 0037 policy verbs are enforced against, and of the plan §5.3 completeness guarantee. Plan §5.3 and docs/40 are informal restatements; where they disagree with this document, this document governs (plan §25: "Where plan prose and RFC disagree, the RFC is corrected and becomes normative"). Every such correction is recorded below under "Corrections recorded by this RFC".

## Versioning and revision

- The diff artifact carries `schema_version`, which is `"1.0"` for the vocabulary specified here. The artifact version, the relation set, the field set, and the semantic-change axis set version together: they are one closed vocabulary and MUST NOT be extended independently.
- The relation set, the intent `field` set, and the `semantic_changes[].kind` axis set are **closed**. Adding, removing, or renaming a member is a breaking change: it requires a major `schema_version` bump *and* an explicit revision of this RFC. A daemon MUST NOT emit a token outside the closed sets, and MUST NOT invent a free-form relation.
- A consumer that reads an unrecognized `relation`, `field`, or `kind` token MUST fail closed: it MUST treat the record as a protected change with relation `unknown` and MUST NOT treat it as `unchanged` or as an `allow`. Forward compatibility is achieved by blocking, never by ignoring.
- The `AssuranceLevel` order, the `AssuranceChange` relation names, and their machine tokens are shared with `crates/continuum-value/src/assurance.rs` and with `assurance.minimum` in [`../schemas/intent-contract.schema.json`](../schemas/intent-contract.schema.json). Reordering or renaming any of them is a coordinated breaking change across this RFC, RFC 0037, that schema, and that crate.
- The wire enums `DiffLayer` and `PolicyDecision` belong to RFC 0026's IDL. A change to either is a protocol change and MUST follow RFC 0026's N and N−1 protocol-major window; this RFC MUST be revised in the same change.
- CPNF-1 is versioned by RFC 0037. A new normal form (`cpnf-2` or a fragment-specific form) does not silently re-classify history: a diff computed under one normal form MUST NOT be reused across a normal-form change, because the normal-form version is part of the classifier's identity (see "Inputs, preconditions, and determinism").
- Classifications are never retroactively upgraded. A relation recorded as `unknown` under one classifier version MAY be recomputed by a later version, but the later result is a new diff artifact with a new `diff_*` identity; the earlier artifact is not edited (INV-009).

## Inputs, preconditions, and determinism

Inputs: two sealed workspace snapshots, before/after Intent Contract identities, the declared semantic fragments from the intent's `scope.fragments`, and the requested assurance for the classification itself.

- Both snapshots MUST be sealed. An unsealed snapshot is rejected (`StaleSnapshot`, RFC 0026); the diff MUST NOT classify against a mutable input.
- Both intent identities MUST resolve in the daemon's intent registry (RFC 0037). A snapshot carries its intent by reference; the daemon resolves it, so `workspace.diff` need not be given intent handles.
- `requested_assurance` MUST be one of the five `AssuranceLevel` tokens — `observed`, `sampled`, `bounded`, `validated`, `proved` — and bounds the evidence the classifier may rely on to claim a *direction*:

  | Requested level | Directional evidence admissible for expression relations |
  |---|---|
  | `observed`, `sampled` | none. Sampling cannot establish language inclusion; every expression-level direction classifies `unknown` |
  | `bounded` | exact language inclusion over the bounded universe in `Finite`; symbolic-bounded obligations |
  | `validated` | the above plus solver obligations discharged with a `CHECKED_CERTIFICATE` (`ValidationBasis`, plan §11.4) |
  | `proved` | the above plus Lean obligations discharged by the proof service (RFC 0035) |

  A direction the classifier can only support with evidence weaker than the requested level MUST be reported as `unknown`, not as the direction. Relations decidable by canonical encoding alone (`unchanged`, `added`, `removed`, `expanded`, `contracted`, `refined`, `coarsened`, `upgraded`, `downgraded`) do not consume this budget and are admissible at every level.
- **Determinism.** For fixed inputs the diff artifact MUST be byte-identical across platforms and releases within a `schema_version`. Diffs are budget-independent queries under RFC 0030's budget rule, so budget belongs to execution identity, not to output identity.
- The classifier's identity for RFC 0030 caching purposes includes the classifier version, the CPNF version, the `semantic_epoch`, and the `proof_epoch`. A diff MUST NOT be reused across an advance of any of them.
- **`unchanged` shortcut.** If `before_intent` and `after_intent` are the same `in_*` identity, `intent_changes` MUST be empty. Identity is over the canonical encoding of semantic content (RFC 0037), so equal identity entails every field unchanged. The converse does not hold: distinct identities MUST be classified field by field and MUST NOT be reported as a whole-contract change.

## Wire surface and diff layers

Nine IDL sites carry a `DiffHandle`: `workspace.fork` (`pre_diff`, optional), `workspace.diff`, `intent.diff`, `intent.propose_revision`, `model.compare`, `correspondence.drift` (optional), `debug.compare`, `repair.review` (`semantic_diff`, nullable), and `query.explain_invalidation`.

- Every `diff_*` artifact, from every one of those sites, MUST validate against `semantic-diff.schema.json`. There is one diff artifact shape; the intent diff travels as the `intent_changes` set inside it (RFC 0032).
- Every such artifact MUST resolve `before_snapshot`, `after_snapshot`, `before_intent`, and `after_intent`. Where the two sides share an intent — `debug.compare`, `query.explain_invalidation`, and `correspondence.drift` normally do — the two intent identities are equal and `intent_changes` MUST be empty.
- `workspace.diff`, `intent.diff`, and `model.compare` carry `PolicyVerdictValue`, whose `decision` MUST equal the artifact's `policy.decision`. `workspace.fork` and `intent.propose_revision` carry `StructuralVerdictValue`; their policy content lives in the referenced artifact, and a `StructuralOutcome` MUST NOT be read as a policy decision.
- **`StructuralOutcome::unchanged` is not the relation `unchanged`.** The IDL's `StructuralOutcome` (`created, updated, sealed, accepted, rejected, locked, cancelled, unchanged, acknowledged`) describes what a mutation did to an artifact. The relation `unchanged` describes a classified field. The two vocabularies share a token and nothing else; a daemon MUST NOT derive one from the other.
- `repair.review.semantic_diff` is nullable only before evaluation. `repair.promote` MUST recompute the diff and its verdict server-side; a client-supplied or cached verdict is never trusted (RFC 0032).

`DiffLayer` has exactly four members. docs/40's ten analysis layers are groupings *within* them, not additional wire values:

| docs/40 analysis layer | `DiffLayer` | Artifact section |
|---|---|---|
| Text diff | `textual` | none (advisory only) |
| Syntax/AST diff | `structural` | none (advisory only) |
| Type/effect diff | `semantic` | `semantic_changes` (`effects`) |
| Intent diff | `intent` | `intent_changes`, `policy` |
| Transition-system diff | `semantic` | `semantic_changes` |
| Observer/property diff | `semantic` | `semantic_changes` (`observer_publication`) |
| Correspondence/refinement diff | `semantic` | `semantic_changes` (`correspondence`) |
| Proof dependency diff | `semantic` | `impact` |
| Behavior/evidence diff | `semantic` | `impact` |
| Cost/performance diff | `semantic` | none (advisory; see "Program semantic diff") |

- Requesting `intent` MUST produce a complete `intent_changes` classification for all fifteen protected fields; a partial intent layer is not a valid result.
- The `textual` and `structural` layers are advisory. They MAY prioritize review; they MUST NOT contribute to `policy.decision` and MUST NOT authorize promotion (docs/40, "Soundness policy").
- A layer that was not requested MUST leave its artifact section empty and MUST NOT be reported as evidence of absence. `policy.decision` MUST be `review` or `block` whenever the `intent` layer was not requested and the two intent identities differ.

## Classification lattice

Every intent field change is classified with one relation from the closed set:

```text
unchanged      strengthened   weakened
expanded       contracted
refined        coarsened          (observers)
merged         split              (abstraction maps)
upgraded       downgraded         (assurance / proof policy)
added          removed            (assumptions, faults, fairness, non-vacuity)
incomparable   unsupported        unknown
```

The sixteen tokens are the `relation` enum of the normative schema, in that order. They partition into three kinds, and the kind determines the policy treatment:

| Kind | Members | Meaning |
|---|---|---|
| Equality | `unchanged` | the canonical encodings of the classified unit are equal |
| Directional | `strengthened`, `weakened`, `expanded`, `contracted`, `refined`, `coarsened`, `merged`, `split`, `upgraded`, `downgraded`, `added`, `removed` | a direction in the field's own order, established by decidable comparison or by a discharged obligation |
| Non-affirmative | `incomparable`, `unsupported`, `unknown` | no direction was established; these fail closed (see "Fail-closed rule") |

Three rules govern the whole lattice:

- **R1 — Per-field orders only.** A relation names a direction in *one field's* order. There is no global strength order and no global valence: `expanded` on `bounds` is a larger verification envelope and is permitted by `no-decrease`, while `expanded` on `trust_boundaries` is a larger unverified surface and is blocked by `no-expansion`. A consumer MUST resolve valence through the field's policy verb, never through the relation token alone.
- **R2 — `unchanged` on equality and nothing weaker.** `unchanged` MUST be claimed only on equality of the canonical encoding of the classified unit: the CPNF-1 N8 encoding for units carrying a property AST (RFC 0037 S1), and RFC 0037's canonical contract encoding otherwise. Textual match, partial structural match, and label/comment equality MUST NOT be used.
- **R3 — Inequality is a question, not an answer.** Unequal canonical encodings carry no direction by themselves (RFC 0037 S2). A direction is emitted only when the field's order decides it or an obligation is discharged; otherwise the relation is `unknown` or `unsupported`.

Directional relations are defined per field by these orders:

- **Properties (per fragment):** properties are structured ASTs (RFC 0037), and every property relation is computed over their CPNF-1 normal forms, never over surface text or the display-only `source`. `unchanged` is claimed on byte-equality of the CPNF-1 encodings and on nothing weaker; CPNF-1 is sound but incomplete, so unequal normal forms carry no direction by themselves. For a direction: in `Finite`, exact language inclusion over the bounded universe; `P strengthened P'` iff behaviors(P') ⊆ behaviors(P) with a checked witness. In `Symbolic`, an implication obligation is discharged by SMT with a checked certificate or by Lean; otherwise `unknown`.
- **Bounds:** componentwise partial order on `(values, nodes, faults, depth)`; a decrease in any component with no increase elsewhere is `contracted`; mixed changes are `incomparable`. `no-decrease` blocks `contracted` and blocks `incomparable` pending review.
- **Assurance:** total order `observed < sampled < bounded < validated < proved`; movement down is `downgraded` and is what `no-downgrade` blocks. Checker requirements (`independent_checker`, `clean_recompute`) turning off is also `downgraded`.
- **Observers:** `refined` iff the new observer distinguishes at least the old projections/events; dropping an event family or coarsening a projection is `coarsened`.
- **Abstraction maps:** the intent's `abstraction_maps` group binds content identities of the §16 correspondence/abstraction maps the claims are stated against; mapping distinct concrete values to one abstract value is `merged`, the reverse is `split`, and both directions are privileged.
- **Scope:** no directional order is defined; any `scope` change (components, abstraction level, declared fragments) on a protected contract is a semantic change and blocks ordinary promotion pending review.
- **Assumptions/faults/fairness/non-vacuity:** set membership per classified item (`added`/`removed`), with `strengthened`/`weakened` for edits to an item's expression evaluated in its fragment — over the same CPNF-1 normal form for assumption expressions and fairness conditions, which are property ASTs (RFC 0037). Adding an assumption or fairness constraint is environment-strengthening and therefore protected; removing a fault or a non-vacuity behavior is protected.
- **Trust boundaries:** growth of the opaque set is `expanded` and is what `no-expansion` blocks.
- **Security policy:** weakening a data classification, removing a redaction class, or relaxing a capability requirement is `weakened` and protected; the reverse direction is `strengthened`.
- **Completion policy / nondeterminism classes:** any change is a semantic change; cross-policy relations are `incomparable` (there is no soundness order among completion policies).

Naming note: plan §5.3's prose speaks of "bound increase/decrease" and "fault envelope expansion/contraction"; this RFC's relation assignments are normative — bounds classify as `expanded`/`contracted`, faults as `added`/`removed` per classified item. The prose phrases are informal aliases for these relations.

## Field-by-field classification rules

The fifteen diff `field` members are exactly the fifteen policy keys of the Intent Contract, related to the contract's field groups by the schema's normative `x-policy-field-map`. The diff `field` name is the *policy* name, not the contract path: a change under `claims` is reported as `properties`, a change under `fault_model` as `faults`, and a change under `optimization.non_vacuity` as `non_vacuity`.

| Diff `field` | Contract path | Unit of classification | Order | Admissible relations |
|---|---|---|---|---|
| `properties` | `claims[]` | one claim, keyed by `id` | behavior-set inclusion over CPNF-1 | `unchanged`, `strengthened`, `weakened`, `added`, `removed`, `incomparable`, `unsupported`, `unknown` |
| `assumptions` | `assumptions[]` | one assumption, keyed by `id` | behavior-set inclusion over CPNF-1 | as `properties` |
| `observers` | `observers[]` | one observer, keyed by `id` | componentwise set inclusion on four projections | `unchanged`, `refined`, `coarsened`, `added`, `removed`, `incomparable`, `unsupported`, `unknown` |
| `abstraction_maps` | `abstraction_maps[]` | one binding, keyed by `role` | map-content comparison (§16, RFC 0036) | `unchanged`, `merged`, `split`, `added`, `removed`, `incomparable`, `unknown` |
| `scope` | `scope` | each of `components`, `fragments`, `abstraction_level` | sets; no order on `abstraction_level` | `unchanged`, `added`, `removed`, `incomparable`, `unknown` |
| `trust_boundaries` | `trust_boundaries` | the pair (`trusted`, `opaque`) | superset on either set | `unchanged`, `expanded`, `contracted`, `incomparable`, `unknown` |
| `bounds` | `bounds` | the tuple `(values, nodes, faults, depth)` | componentwise partial order | `unchanged`, `expanded`, `contracted`, `incomparable`, `unknown` |
| `faults` | `fault_model` | one member of `enabled` or of `profiles` | set membership | `unchanged`, `added`, `removed` |
| `fairness` | `fairness[]` | one constraint, keyed by (`kind`, `action`) | strength of the constraint (see below) | `unchanged`, `strengthened`, `weakened`, `added`, `removed`, `incomparable`, `unsupported`, `unknown` |
| `completion_policy` | `completion_policy` | the scalar | none | `unchanged`, `incomparable` |
| `nondeterminism` | `nondeterminism[]` | one entry, keyed by `site` | none on `class` | `unchanged`, `added`, `removed`, `incomparable` |
| `assurance` | `assurance` | the requirement triple plus the accepted-class set | total order plus checker flags | `unchanged`, `upgraded`, `downgraded`, `added`, `removed` |
| `optimization` | `optimization.hard`, `optimization.soft` | one constraint or objective | set membership | `unchanged`, `added`, `removed` |
| `non_vacuity` | `optimization.non_vacuity` | one non-vacuity behavior | set membership | `unchanged`, `added`, `removed` |
| `security_policy` | `security_policy` | `data_classification`; each redaction class; each capability requirement | total order on classification; set membership otherwise | `unchanged`, `strengthened`, `weakened`, `added`, `removed` |

- **Non-affirmative relations are admissible on every row, even where the column above omits them.** `faults`, `assurance`, `optimization`, `non_vacuity`, and `security_policy` list only `unchanged` and their directional relations, but the fail-closed rule ("Fail-closed rule"; "Policy verdict" P2) is a property of the enforcement path, not of a field's own order, and it holds "for every field and every verb": a classification that cannot complete is `unknown`, one whose unit falls outside declared fragments is `unsupported`, and one whose comparison refutes both inclusions is `incomparable`. Read every row's admissible set as its listed relations *plus* `incomparable`, `unsupported`, and `unknown`, exactly as the rows that already list them explicitly do. The one field-specific exception is `assurance`: "Assurance movement" states `incomparable` MUST NOT be emitted for that field because the requirement triple's own order makes a true incomparability impossible to produce, so its row's non-affirmative complement is `unsupported` and `unknown` only, not `incomparable`.
- **One record per classified unit.** `intent_changes` is an array and MAY carry several records for one field. The classifier MUST emit one record per classified unit that is not `unchanged`, and MUST NOT collapse two units with different relations into one record. Records whose relation is `unchanged` MAY be omitted.
- **Protection is structural.** Every member of the `field` enum is in RFC 0037's protected set, so `protected` is `const: true` in the schema and an unprotected intent change is unrepresentable. A daemon MUST NOT route a protected-field change around `intent_changes` by reporting it only as a `semantic_changes` record.
- **Fragment attribution.** Each record SHOULD carry the `fragment` its relation was classified in. A unit whose fragment is not in the after-contract's `scope.fragments` MUST classify `unsupported`.

### `properties` (contract `claims`)

Claims are keyed by `id`. A claim present on one side only classifies `added` or `removed`. A claim present on both sides is classified by comparing `expression` under CPNF-1: equal N8 encodings classify `unchanged` (RFC 0037 S1); otherwise the direction is `strengthened` iff behaviors(after) ⊆ behaviors(before), `weakened` iff behaviors(before) ⊆ behaviors(after), `incomparable` iff both inclusions are refuted, and `unknown` otherwise. A change to `kind` or to the bound `observer` with an unchanged expression is a change of what the claim means and MUST classify `incomparable`, never `unchanged`. Renaming an `id` while preserving the expression is `removed` plus `added`, not `unchanged`; the classifier MUST NOT match claims by expression to defeat the rename.

### `assumptions`

Keyed by `id`. The relation is computed on `expression` exactly as for claims — the same order, on the same normal form. Direction and policy valence differ: an assumption `strengthened` admits fewer environments and makes verification easier, and is the gaming move plan §5.1 names ("adding a fairness assumption that schedules away the bug" is its fairness sibling). A change to `classification` or to `fidelity_profile` with an unchanged expression MUST classify `incomparable`: RFC 0002's fidelity profiles are explicitly not ordered, so no direction may be inferred.

### `observers`

Keyed by `id`. Compare the four sets `events`, `state_projection`, `knowledge_projection`, `security_projection` componentwise. All four supersets, at least one strict, is `refined`; all four subsets, at least one strict, is `coarsened`; equal on all four is `unchanged`; any mixed movement is `incomparable` — the componentwise rule admits no exception for this case. Dropping an event family or a projection element, with no other component growing in the same comparison, is the "hide observer events" attack (plan §19.5) and classifies `coarsened` under the rule just stated. If another component grows in the same comparison, the movement is mixed: it classifies `incomparable`, not `coarsened`, and still blocks ordinary promotion under the fail-closed rule, exactly as every other non-affirmative relation does.

### `abstraction_maps`

Keyed by `role`. A role bound to a different `map_id` requires comparing the two maps' contents through the §16 correspondence graph (RFC 0036): a strictly coarser map (distinct concrete values sent to one abstract value) is `merged`; a strictly finer map is `split`; neither is `incomparable`. If either map identity does not resolve, or the comparison is outside the declared fragments, the relation is `unknown` or `unsupported` respectively. Both `merged` and `split` are privileged; `merged` is the abstraction-gaming vector RFC 0032 maps to gate `intent_integrity`.

### `scope`

`components` and `fragments` are classified by set membership (`added`/`removed` per element); `abstraction_level` has no order and any change classifies `incomparable`. The lattice bullet's "no directional order is defined" means no *strength* order: a membership record carries no direction of strength, and every `scope` record blocks ordinary promotion under the `scope` verb whatever its relation. Two second-order rules:

- Removing a fragment from `scope.fragments` MUST force re-classification of every unit stated in that fragment, and those units then classify `unsupported`. Narrowing scope therefore cannot convert a blocked change into an allowed one; it converts it into a fail-closed one.
- Adding a fragment does not retroactively license a direction: relations for units in the newly declared fragment are computed under this RFC's ordinary rules and are `unknown` until an obligation is discharged.

### `trust_boundaries`

The classified unit is the pair (`trusted`, `opaque`). Growth of either set is `expanded`: enlarging `opaque` hides behavior from verification (docs/41's "opaque escape"), and enlarging `trusted` enlarges the TCB. Shrinking both is `contracted`; growing one while shrinking the other is `incomparable`. `no-expansion` blocks `expanded` and, under the fail-closed rule, `incomparable`.

### `bounds`

The tuple is `(values, nodes, faults, depth)` over the schema's types: `nodes` and `faults` are integers; `values` and `depth` are integers or `null`. `null` denotes *unbounded* and is the top of that component's order, so `n → null` contributes an increase and `null → n` contributes a decrease. An absent `values` or `depth` key MUST be read as `null`. An absent `nodes` or `faults` key has no null form and no schema default; a change in the declaredness of either MUST classify `bounds` as `unknown` and fail closed. A decrease in any component with no increase elsewhere is `contracted`; an increase in any component with no decrease elsewhere is `expanded`; mixed movement is `incomparable`.

### `faults` (contract `fault_model`)

`enabled` is a closed set of six classes (`crash`, `recovery`, `partition`, `loss`, `duplication`, `delay`) and `profiles` is a set of pack profile names. Both are classified by membership: one record per class or profile added or removed. `removed` is the "removing crash-after-submit" attack and is what `no-removal` blocks. Plan §5.3's "fault envelope expansion/contraction" is the informal alias for a set of `added`/`removed` records.

### `fairness`

Constraints are keyed by (`kind`, `action`); the schema gives fairness items no `id`. Three movements are classified:

- Membership: a constraint present on one side only is `added` or `removed`. Adding fairness is environment-strengthening and is the canonical gaming vector (docs/50).
- `kind`: `weak → strong` on the same action classifies `strengthened`, because strong fairness implies weak fairness; `strong → weak` classifies `weakened`. (A `kind` change is equivalently expressible as one `removed` plus one `added` record; a classifier MUST choose one encoding and MUST NOT emit both.)
- `condition`: the enabling predicate occupies an **antitone** position. A fairness constraint applies wherever its condition holds, so weakening the condition strengthens the constraint. Where `condition` is a state formula, the relation on the *constraint* is the dual of the relation on the *formula*: formula `weakened` ⇒ constraint `strengthened`, and formula `strengthened` ⇒ constraint `weakened`. `null` means unconditional and is the weakest condition, hence the strongest constraint: `C → null` classifies `strengthened`. Formula relations are computed over CPNF-1 exactly as for claims, and `incomparable`/`unknown` pass through the duality unchanged.

### `completion_policy` and `nondeterminism`

`completion_policy` is a scalar over four unordered values; equal values classify `unchanged` and every other pair classifies `incomparable`. `nondeterminism` entries are keyed by `site`: a site present on one side only is `added` or `removed`; a site whose `class` changed classifies `incomparable`, because the dossier states no soundness order among `demonic`, `angelic`, `scheduler`, `probabilistic`, `timed`, and `epistemic`. `demonic → angelic` is a textbook gaming move and blocks through the fail-closed rule, not through a guessed direction.

### `assurance`

See "Assurance movement" below. `accepted_evidence_classes` is an unordered set (RFC 0010: evidence is a partial order, not a ladder), so it is classified by membership only: one `added`/`removed` record per class. It MUST NOT contribute an `upgraded` or `downgraded` relation, and MUST NOT be ranked.

### `optimization` and `non_vacuity`

`optimization.hard` and `optimization.soft` are classified under field `optimization`; `optimization.non_vacuity` is classified under field `non_vacuity`. Splitting them is required, not cosmetic: the policy table carries separate verbs for the two, and `non_vacuity = "no-removal"` (INV-012) is unenforceable if a removed non-vacuity behavior is reported as an `optimization` change. All three are string sets today and are classified by membership.

### `security_policy`

Three components:

- `data_classification` is a total order `public < internal < confidential < restricted`. Movement up is `strengthened`; movement down is `weakened`.
- `redaction_classes` and `capability_requirements` are sets: one `added`/`removed` record per element. Removing a redaction class or a capability requirement is the weakening direction that plan §18 and the `security_policy` verb protect.
- A change that moves the classification up while removing a redaction class or a capability MUST NOT be reported as `strengthened`. Each component emits its own record, and the blocking record governs the verdict.

## Assurance movement

The `assurance` field is the one totally ordered field, and it is fixed by the "Classification lattice" bullet above. The rules below are the normative statement of what `crates/continuum-value/src/assurance.rs` implements; the crate and this section MUST agree token for token.

- The order is `AssuranceLevel`: `Observed < Sampled < Bounded < Validated < Proved`, with machine tokens `observed`, `sampled`, `bounded`, `validated`, `proved` — the closed `assurance.minimum` enum of the intent-contract schema. `AssuranceLevel::ALL` lists them weakest first.
- The classified unit is the requirement triple (`minimum`, `independent_checker`, `clean_recompute`) — `AssuranceRequirement`. The relation is `AssuranceChange`, whose three members are `unchanged`, `upgraded`, `downgraded`: the schema's sixteen-relation set restricted to this field.
- A change **weakens** iff `minimum` falls, or `independent_checker` goes true→false, or `clean_recompute` goes true→false. A change **strengthens** iff `minimum` rises, or either flag goes false→true.
- **Fail closed in both directions.** A change that both weakens and strengthens MUST classify `downgraded`. A net direction MUST NOT be computed. This is what stops a contract from dropping its independent checker in exchange for a nominally higher `minimum` and passing `no-downgrade`; it is `AssuranceRequirement::change_to`'s `if weakened { Downgraded } else if strengthened { Upgraded } else { Unchanged }` and it is the reason that ordering matters.
- `incomparable` MUST NOT be emitted for `assurance`. The field is totally ordered, unlike `bounds`; the only three outcomes are the three above.
- A change in the *declaredness* of `independent_checker` or `clean_recompute` — declared on one side of a revision and undeclared on the other — has no member among `unchanged`, `upgraded`, `downgraded`: the requirement triple is not defined on the side that leaves a flag undeclared, so there is no direction to compute and reading the undeclared side as `false` would invent a value RFC 0037 forbids inventing. Normative: such a change classifies `assurance` as `unknown` and fails closed — the companion of RFC 0037 correction 14, and the same answer `bounds` gives a declaredness change on `nodes`/`faults`.
- `no-downgrade` blocks exactly `downgraded` — `AssuranceChange::blocked_by_no_downgrade`. It does not block `upgraded`, and it does not by itself block membership changes to `accepted_evidence_classes`; those are governed by the same verb through the `review` path and by the fail-closed rule.
- The five level names coincide with five of the nine claim statuses of plan §11.4, and the coincidence is deliberate but not an identity: an intent *demands* a level, a claim *reaches* a status. `crates/continuum-evidence/src/claim_status.rs` derives the order inside its assurance band from this section, and adds `Proposed`, `Inconclusive`, `Refuted`, and `Superseded`, which are not levels of anything. A diff MUST NOT classify a claim-status movement as an `assurance` relation.

## CPNF-1 interaction

Property, assumption, and fairness-condition expressions are structured ASTs. RFC 0037 fixes their canonical normal form (CPNF-1, rules N1–N9) and states the three soundness facts this RFC is built on (S1–S3). The interaction is exactly:

- **From S1 (soundness):** `normalize(P) = normalize(Q)` implies P and Q denote the same behaviors, so equal CPNF-1 encodings ⇒ `unchanged`, and `unchanged` MUST NOT be claimed on anything weaker. Bound-variable renaming (N5), conjunct reordering and duplication (N4), `implies`/`iff`/`leads_to` expansion (N1), De Morgan pushes (N2), junction flattening (N3), `gt`/`ge` swaps (N6), and the unit/absorption laws (N7) all reach the same encoding, so none of them can masquerade as a semantic edit — and none of them can be *sold* as a refactor while carrying one.
- **From S2 (incompleteness is the safe side):** unequal encodings ⇒ no direction. CPNF-1 does not reorder same-kind quantifiers or `apply` arguments and does not recognize tautologies beyond N7, so a difference in normal form is a question. The diff MUST discharge the fragment's implication obligation or classify `unknown`. A false `unknown` costs a review; a false `unchanged` costs the invariant.
- **From S3 (fragment scope):** identity and `unchanged` MAY use CPNF-1 in every fragment. The directional relations `strengthened` and `weakened` MUST NOT be taken from CPNF-1 in any fragment: they require that fragment's decision procedure, and outside `scope.fragments` the relation is `unsupported`. Until RFC 0037's open question on `Symbolic`/`Temporal`/`Probabilistic` normalization closes, those fragments get `unchanged` and `unsupported`/`unknown` from this RFC and no direction without a discharged obligation.
- **N9 (idempotence and determinism)** is what makes diff determinism achievable. A classifier MUST re-normalize both sides rather than trust a declared `normal_form: "cpnf-1"`, and MUST reject a contract whose declaration does not hold.
- The display-only `source` rendering MUST NOT contribute to identity or to any classification. Where `source` and `ast` disagree, `ast` is authoritative.

## Completeness guarantee

This RFC is the normative home of the plan §5.3 diff guarantee. Within the intent's declared supported fragments, every property weakening, assumption strengthening, bound decrease, observer coarsening, fault removal, fairness addition or removal, and assurance downgrade MUST be classified as a privileged intent change — the seven G3 dimensions. Outside declared fragments the classification is `unsupported`, never a guessed direction.

The seven dimensions are the graded floor, not the protected set. All fifteen fields above are protected (RFC 0037, plan §5.4), all fifteen are lockable, and a change to any of them MUST appear in `intent_changes`. A reading of plan §5.3 in which the eight fields outside the seven dimensions (`scope`, `nondeterminism`, `abstraction_maps`, `trust_boundaries`, `completion_policy`, `security_policy`, `optimization`, `non_vacuity`) are unprotected is incorrect; G3 grades the seven, INV-001 protects the fifteen.

The eight reward-hacking moves of plan §5.1 map onto the lattice as follows, and each maps to the verb that blocks it:

| Gaming move (plan §5.1) | Field | Relation | Blocking verb (plan §5.4) |
|---|---|---|---|
| change `Agreement` to a weaker observer | `observers` | `coarsened` | `review` |
| add a fairness assumption that schedules away the bug | `fairness` | `added` / `strengthened` | `review` |
| reduce node count from five to three | `bounds` | `contracted` | `no-decrease` |
| remove crash-after-submit | `faults` | `removed` | `no-removal` |
| mark a storage effect opaque | `trust_boundaries` | `expanded` | `no-expansion` |
| map two concrete values to one abstract value | `abstraction_maps` | `merged` | `review` |
| lower assurance from exhaustive to sampled | `assurance` | `downgraded` | `no-downgrade` |
| exclude the failing state with a constraint | `properties` | `weakened` | `review` |

## Fail-closed rule

- Where implication is decidable or solver-checkable in the declared fragments, Continuum MUST prove the direction and attach the witness/obligation to the change record.
- Otherwise the relation is `unknown` (undecidable/unattempted) or `unsupported` (outside declared fragments) — these are distinct (INV-008).
- `unknown`, `unsupported`, and `incomparable` on a protected field MUST be treated exactly as a confirmed protected change: ordinary promotion is blocked pending review (plan §5.3) — the `incomparable` rule stated for bounds above holds for every protected field. The diff never guesses an ordering.
- Because `protected` is `const: true`, this rule reaches every `intent_changes` record. The normative schema enforces it structurally: a diff containing a record whose relation is `unknown`, `unsupported`, or `incomparable` MUST carry `policy.decision` of `block` or `review`.
- The wire tokens are lowercase — `unknown`, `unsupported`, `incomparable`. Prose elsewhere in the dossier capitalizes them; the lowercase literals of the relation enum are the values.
- A classifier that cannot complete MUST NOT emit a partial `intent_changes` set with an `allow` decision. Failure to classify is `unknown` on every unclassified protected field, or a typed error (`UnsupportedSemanticFeature`, `BudgetExhausted` with a continuation) — never silence.

## Program semantic diff

Program-side changes are classified along seven closed axes — the `semantic_changes[].kind` enum: `effects` (new/removed/moved effect sites), `atomicity` (split/merged critical regions), `task_ownership`, `cancellation_structure` (checkpoints, obligations), `durability_ordering`, `observer_publication`, and `correspondence` (which links of §16's graph a change touches). Each axis yields typed change records with source locations; these feed the impact set and the repair gates, not the intent policy.

- `semantic_changes` MUST NOT justify an `allow` decision on its own, and a program change MUST NOT be used to explain away an `intent_changes` record.
- A record on the `correspondence` axis MUST place every refinement and correspondence proof depending on the touched link into `invalidated` or `unknown` (RFC 0036, plan §16.3).
- docs/40's program-diff list carries two entries that are not axes here, and this RFC resolves both: an **opaque/FFI boundary change** is an *intent* change on `trust_boundaries` (`expanded`/`contracted`), not a program axis; a **resource-footprint change** belongs to the cost/performance analysis, which is advisory and carries no `semantic_changes` record. Neither may be reported under a program axis token.

## Impact set

The diff MUST emit `invalidated` / `reused` / `unknown` evidence sets computed against the incremental database's dependency classes (RFC 0030). Evidence whose dependency on a changed field is `unknown` goes to `unknown`, never to `reused`.

- The three sets MUST be pairwise disjoint, and their union MUST cover every evidence artifact that names either intent identity or either snapshot. An artifact in none of the three is a completeness failure, not an implicit reuse.
- RFC 0030's tri-state independence rule applies unchanged: `Unknown` is dependent. Heuristic independence is permitted only on `Experimental` edges, and an `Experimental` reuse MUST NOT support promotion.
- Field changes map onto RFC 0030's nine typed dependency reasons as follows; a field with no reason of its own yields `Unknown` and therefore `unknown`, never `reused`:

  | Diff `field` | RFC 0030 dependency reason |
  |---|---|
  | `properties` | query-key change: evidence is bound to the old claim identity and is not addressable under the new one |
  | `assumptions`, `fairness`, `bounds` | `relies-on-assumption-fairness-bound` |
  | `observers` | `observes-event-family` |
  | `abstraction_maps` | `uses-abstraction-component` |
  | `assurance` | `depends-on-lemma-or-checker` |
  | `scope`, and the `correspondence` program axis | `consumes-encoding-epoch-or-correspondence` |
  | `faults`, `completion_policy`, `nondeterminism`, `trust_boundaries`, `security_policy`, `optimization`, `non_vacuity` | no named reason; classify `Unknown` ⇒ `unknown` |

- A property edit changes the contract's canonical encoding and therefore the `in_*` identity and RFC 0030's query key. Evidence keyed to the old identity is not stale — it is unaddressable — so it MUST land in `invalidated` unless a `Validated` edge carries a checker witness licensing reuse.
- Impact output distinguishes definitely invalidated from conservatively rebuilt; a conservatively rebuilt artifact MUST NOT be reported as `reused` without its `Conservative`-class justification.

## Evidence-status interaction

Invalidation is a statement about evidence, not a downgrade of a claim.

- The diff MUST NOT be implemented by writing a lower status onto an existing claim node. `ClaimStatus::transition_to` classifies such a write `StatusTransition::Regression`, and the plan §11.7 compare-and-set rejects it — "racing promotions cannot regress the lattice" (`crates/continuum-evidence/src/claim_status.rs`).
- A claim whose supporting evidence the diff invalidates is retired by `Superseded` with an explicit `SUPERSEDES` edge to its successor under the revised intent (plan §4.6, §11.3), or is re-established under the new identity. Both are permitted transitions; lowering is not.
- Budget exhaustion during classification is never a verdict and never a status: it is `BudgetExhausted` with a continuation (RFC 0026), and the affected evidence stays in `unknown`.

## Policy verdict

The verdict is a `PolicyDecision` — `allow`, `review`, or `block`. It is computed from the per-field relations and the intent's policy verbs, and it MUST name the reasons per field. The promotion endpoint re-computes the verdict at promotion time; cached client verdicts are never trusted (RFC 0032).

- There is no `unknown` decision. An inconclusive classification is expressed as an `unknown`/`unsupported`/`incomparable` *relation*, which fails closed to `review` or `block` at classification time. The three-value enum is shared with the IDL's `PolicyDecision` and with the artifact's `policy.decision`.
- `allow` requires that every protected field classify `unchanged`, or classify a direction the field's verb permits. One non-affirmative relation on one protected field is sufficient to forbid `allow`.
- `policy.reasons` MUST name the field and the relation of every record that forbade `allow`. Reasons are a set of strings on the wire; they are a rendering of typed records and MUST NOT be the only place a blocking fact appears (INV-003).
- Verb enforcement uses RFC 0037's closed verb set (`unlocked`, `proposal-only`, `review`, `locked`, `no-decrease`, `no-removal`, `no-downgrade`, `no-expansion`). A directional verb is enforceable only where this RFC defines the field's order; where it does not, the change classifies `incomparable` or `unknown` and blocks.

## Corrections recorded by this RFC

Per plan §25, where plan prose, docs, or a dependent schema disagrees with this RFC, this RFC governs. The corrections in force:

1. **Policy verdict values.** Earlier prose in this RFC gave the verdict as `allow | review | block | unknown`. Corrected to three values: the normative schema's `policy.decision` and the IDL's `PolicyDecision` both carry exactly `allow`, `review`, `block`. Direction: this RFC is corrected to agree with its own normative schema and with RFC 0026's IDL.
2. **Bounds and faults relation names.** Plan §5.3's "bound increase/decrease" and "fault envelope expansion/contraction" are informal. Normative: bounds classify `expanded`/`contracted`; faults classify `added`/`removed` per item. Direction: RFC governs, plan prose is an alias (retained from the previous revision).
3. **Proof policy.** Plan §5.3's "proof policy upgrade/downgrade" has no field of its own; it is an `assurance` change. Plan §5.3 already records the pointer.
4. **The seven dimensions are a floor.** Plan §5.3's guarantee enumerates seven G3 dimensions; the protected set is fifteen fields. Normative: all fifteen are classified and blockable. Direction: RFC clarifies plan §5.3; no plan contradiction.
5. **docs/40's `changed` relation.** docs/40 lists "fairness added/removed/changed" and "optimization/non-vacuity changed". `changed` is not in the closed relation set. Normative: fairness edits classify `strengthened`/`weakened` (with the antitone rule for `condition`) or `added`/`removed`; optimization and non-vacuity classify `added`/`removed`. Direction: RFC governs docs/40.
6. **docs/40's "no protected change" and "unknown, proof required" categories.** Neither is a relation. Normative: "no protected change" is an `intent_changes` array with no non-`unchanged` record; "unknown, proof required" is relation `unknown` plus an attached obligation. Direction: RFC governs docs/40.
7. **docs/40's program-diff list.** "Changed resource footprints" and "opaque/FFI boundary changes" are not among the seven closed program axes. Normative: see "Program semantic diff". Direction: RFC governs docs/40.
8. **docs/40's ten diff layers versus the IDL's four.** Both are retained with an explicit mapping table; the four `DiffLayer` values are the wire vocabulary and the ten are analyses grouped under them. Direction: RFC reconciles; no IDL change requested.
9. **Case of the non-affirmative tokens.** Plan §5.3 and RFC 0037 write `Unknown` in prose. Normative: the wire literals are lowercase. Direction: RFC governs; RFC 0037's prose is unaffected in meaning.
10. **Assurance mixed movement.** The assurance bullet named only downward movement. Normative: a change that both weakens and strengthens classifies `downgraded`, and `incomparable` is never emitted for `assurance`. Direction: RFC absorbs the rule already implemented in `crates/continuum-value/src/assurance.rs`.
11. **Trust boundaries.** The lattice bullet names growth of the opaque set. Normative: growth of the `trusted` set is also `expanded`; mixed movement is `incomparable`. Direction: RFC extends its own rule; docs/40's undirected "trust boundary expanded/contracted" is made precise.
12. **`optimization` versus `non_vacuity`.** The two are separate diff fields with separate verbs, per the contract schema's `x-policy-field-map`. Normative: a non-vacuity removal MUST NOT be reported under `optimization`. Direction: RFC makes the split explicit.
13. **The per-field admissible-relation table omitted the non-affirmative relations from five rows.** `faults`, `assurance`, `optimization`, `non_vacuity`, and `security_policy` listed only `unchanged` and their directional relations, which reads as forbidding `unknown`/`unsupported`/`incomparable` on exactly the fields whose set-membership or total order gives a classifier the least room to discharge an obligation — the opposite of what the fail-closed rule intends. Normative: the non-affirmative three are admissible on every row (with `incomparable` excluded from `assurance` alone, per its own total-order rule). Direction: this RFC's table is corrected to agree with its own "Fail-closed rule" and "Policy verdict" P2, both already normative; found by `crates/continuum-intent/src/change_policy.rs` (`PolicyField::admits_relation`).
14. **The schema's own worked example violated the `unchanged` shortcut.** `schemas/examples/semantic-diff.example.json` had `before_intent == after_intent` (the same `in_*` identity) with a non-empty `intent_changes` array. The "`unchanged` shortcut" ("Inputs, preconditions, and determinism") requires `intent_changes` to be empty whenever the two intent identities are equal, since equal identity already entails every field unchanged; the example instead spelled that equality out as an explicit per-field `unchanged` record, which the shortcut forbids. Normative: unchanged — the shortcut already said this. Direction: the example is corrected. `after_intent` now names a distinct `in_ack_v2` identity, and `intent_changes` carries a genuine per-field classification (`bounds` `expanded`, permitted under `no-decrease` and so still `allow`) alongside the pre-existing `properties` `unchanged` record for `AckImpliesDurable` — the artifact now demonstrates R2 on both sides of the classification lattice, an actual equality and an actual inequality driving a direction, rather than asserting `unchanged` in the one place the shortcut says nothing may be asserted at all (bn-13nlo).
15. **The `observers` rule's coarsening sentence read as self-contradictory.** "... MUST classify `coarsened` even when other components grow" — read literally as an amendment to the componentwise rule stated one sentence earlier, this contradicts it: growth anywhere rules out "all four subsets", so a mixed case cannot be *both* `coarsened` by the amendment and excluded from `coarsened` by the rule it amends. Normative: the componentwise rule has no exception; a comparison with both growth and shrinkage classifies `incomparable`, never `coarsened`, and blocks under the fail-closed rule exactly as every other non-affirmative relation does — the security property the old sentence was reaching for ("blocks... regardless") holds under this reading too, since `incomparable` is non-affirmative and blocks unconditionally (P2), unlike a bare `coarsened`, which only blocks under a verb that names it. Direction: the "`observers`" section is corrected to state this without the self-contradiction. `crates/continuum-semantic-diff/src/observers.rs` already implements the corrected reading and no longer needs to carry its own disambiguating essay (bn-1sdp implemented the reading; bn-13nlo corrected this RFC's text to state it).

Flags raised against artifacts this RFC does not own (no silent divergence):

- **F1 — `intent_changes` records cannot name the classified unit.** The record shape is `{field, relation, protected, fragment, evidence}` with `additionalProperties: false`, so the wire cannot say *which* claim was weakened. The classifier computes per-unit relations, emits one record per non-`unchanged` unit, and carries the unit identity only in `policy.reasons` prose. A per-unit locator belongs in the schema; raised for the schema sweep (SD-08), not patched here.
- **F2 — `requested_assurance` is an unconstrained string** in the normative schema. This RFC requires one of the five `AssuranceLevel` tokens; the schema does not yet enforce it.
- **F3 — no classifier/CPNF version field.** The artifact records neither the classifier version nor the normal-form version, though both are part of the classification's identity under RFC 0030's query key. Raised for the schema sweep.
- **F4 — `policy.reasons` is free-form strings.** Typed per-field reasons would satisfy INV-003 structurally rather than by convention.
- **F5 — The `faults` per-unit locator does not say which set it names.** `semantic-diff.schema.json`'s `unit` locator for `faults` is "the member of `enabled` or `profiles`" — the bare wire token, with nothing marking which set it came from. A domain-pack profile named `crash` and the fault class `crash` therefore render as the identical locator, though they are different units with different policy consequences: a profile is checked against the snapshot's domain packs (RFC 0037 W8) and a class is not. Accepted as is for this draft epoch rather than fixed: qualifying the locator (e.g. `enabled:crash` / `profiles:crash`) is a wire-shape change to a schema this RFC does not own alone, and `crates/continuum-intent/src/faults.rs` already carries the disambiguation beside the schema's spelling — `FaultUnit::locator()` returns the bare token the schema specifies, and `FaultUnit::field()` returns `fault_model.enabled` or `fault_model.profiles` for any caller that needs to tell the two apart. Raised for the schema sweep (SD-08) rather than patched here.

## Rejected alternatives

- **Token/set comparison of property strings.** Rejected as the production mechanism: defeated by renaming and rewriting (it survives only as the R3 spike baseline).
- **Best-effort ordering for undecidable cases.** Rejected: a guessed "weakened/strengthened" is exactly the false confidence the system exists to prevent.
- **One global strength order across fields.** Rejected: bounds are partial, assurance is total, completion policies are unordered — collapsing them loses the distinctions the locks need.
- **A net direction for the assurance triple.** Rejected: netting a raised `minimum` against a dropped checker requirement is precisely the trade `no-downgrade` exists to refuse.
- **Matching claims by expression instead of by `id`.** Rejected: it would let a rename plus a weakening present as a single `unchanged` pair, and it makes the diff depend on a heuristic match rather than on declared identity.
- **A fourth `unknown` policy decision.** Rejected: a decision value that is neither permissive nor blocking has no defined promotion semantics; inconclusiveness is a property of the relation, and it fails closed into `review` or `block`.

## Open questions

- Property normalization/equivalence beyond Finite (shared with RFC 0037). Until it closes, `Symbolic`, `Temporal`, and `Probabilistic` get `unchanged` and `unsupported`/`unknown` but no direction.
- Which SMT fragments earn `strengthened`/`weakened` with `CHECKED_CERTIFICATE` versus obligation-only.
- Whether `optimization` and `non_vacuity` entries, still strings, gain typed forms; today they are classified by set membership only.
- Whether a fairness `kind` change should be encoded as one `strengthened`/`weakened` record or as a `removed`+`added` pair; this RFC fixes the single-record encoding and the alternative is left open for the acceptance corpus to evaluate.
- Whether abstraction-map comparison can be made decidable enough to avoid `unknown` on realistic §16 maps.

## Acceptance

- Mutation corpus with known strengthened/weakened/incomparable/unknown relations per field, including renames and formula rewrites that MUST NOT classify `unchanged`.
- All G0-DX-02 gaming patches classify as protected; a source-only guard repair classifies as program change only. Every row of the plan §5.1 table above is a required corpus case.
- Fail-closed tests: protected-field `unknown`/`unsupported`/`incomparable` blocks ordinary promotion; a partial classification cannot reach `allow`; an unrecognized relation token blocks.
- Per-field rule tests for all fifteen fields, including the antitone fairness `condition` rule, the `null`-as-unbounded bounds rule, the `weak`→`strong` fairness direction, and the fragment-removal rule (narrowing `scope.fragments` yields `unsupported`, never `allow`).
- Assurance tests mirroring `crates/continuum-value/src/assurance.rs`: all 5×5 minimum movements plus both checker flags, including every mixed case classifying `downgraded`, and `incomparable` never emitted.
- Impact-set soundness: no invalidated-in-truth evidence lands in `reused` on the acceptance corpus; the three sets are disjoint and total.
- Determinism: identical inputs yield byte-identical artifacts across platforms; a diff is not reused across a classifier, CPNF, semantic-epoch, or proof-epoch change.
- Wire conformance: every operation returning a `DiffHandle` produces an artifact validating against the normative schema, and `PolicyVerdictValue.decision` equals the artifact's `policy.decision`.
