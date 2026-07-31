# RFC 0037: Intent Contract Schema and Policy

## Status
Draft for implementation.

**Target gate:** G3 (intent integrity)
**Owners:** intent/workbench leads
**Normative language:** MUST/SHOULD/MAY per RFC 2119.
**Normative schema:** [`../schemas/intent-contract.schema.json`](../schemas/intent-contract.schema.json). Where this document and the JSON schema disagree, the schema is corrected or this RFC is amended by an explicit revision; neither drifts silently.

## Summary

The Intent Contract is the protected statement of what a system must mean. It is the root object of plan.md B1/INV-001: ordinary tasks cannot mutate it, repairs are evaluated against it, and every change to it is classified by RFC 0031 before any evidence survives the revision.

## Storage and custody

- Intent Contracts MUST be stored and versioned only in the daemon's intent registry, outside every writable or forkable workspace snapshot (plan §4.2).
- A workspace snapshot carries the intent's content identity (`in_*`) as a reference. `workspace.fork` MUST preserve the binding by identity.
- Rebinding a snapshot lineage to a different intent is a privileged operation that MUST produce an intent diff (RFC 0031) and MUST invalidate dependent evidence.

## Intent bundles and convergence

Absorbed from plan §4.2.1 (SD-05). Registries converge through signed, content-addressed **intent bundles** (`inb_*`):

- A bundle exports one or more contracts with their acceptance records and policy tables. Bundles MAY be vendored in the repository or fetched by identity.
- Import is idempotent and MUST NOT change protection status: an imported `Proposed` contract stays `Proposed`. An imported acceptance is honored only if its signature chain satisfies the local policy.
- Divergent branches are reconciled by semantic three-way merge: each head's §5.3 diff (RFC 0031) is taken against the common ancestor; two revisions merge automatically only when both diffs are classified independent within supported fragments. Otherwise the merge is a `Conflict` node whose resolution requires the `revise-intent` capability — never an automatic pick by write order or timestamp.
- CI MUST fail closed (`AcceptanceChainInvalid`) when the bundle referenced by the workspace configuration is absent or its acceptance chain does not verify.
- The intent diff ships with a PR-reviewable projection: a SARIF note plus a rendered semantic diff (plan §17.6).

## Fields

The contract consists of these field groups (types in the schema):

| Group | Content | Notes |
|---|---|---|
| `claims` | structured property AST + stable semantic id + kind (`safety, liveness, refinement, hyperproperty, security, performance`) + observer | at least one claim required; the AST and its normal form are specified below |
| `assumptions` | classified property ASTs (`environment, scheduler, timing, storage, network, trust`), each MAY declare a domain-pack `fidelity_profile` (`ideal, contractual, platform-qualified, adversarial-envelope`; RFC 0002 — profiles are NOT automatically ordered) | timing assumptions are declarative until ADR-0016 lanes ship |
| `observers` | event families + state/knowledge/security projections (the four projection kinds of plan §5.2) | the unit reductions are justified against (INV-013) |
| `abstraction_maps` | content identities of the §16 correspondence/abstraction maps the claims are stated against | `merged`/`split` changes are privileged (RFC 0031) |
| `scope` | components, abstraction level, declared semantic `fragments` (`Finite, Symbolic, Temporal, Probabilistic, Theorem, Runtime`; ADR-0025) | fragments bound what the diff can classify (see RFC 0031) |
| `trust_boundaries` | trusted and opaque components/effects | expansion of the opaque set is a protected change ("opaque escape", docs/41) |
| `bounds` | values, nodes, faults, depth | componentwise partial order; see RFC 0031 |
| `fault_model` | enabled fault classes + pack profiles | |
| `fairness` | weak/strong constraints per action; the enabling `condition` is a state formula (a temporal-operator-free property AST) | protected: fairness strengthening is the canonical gaming vector (docs/50) |
| `completion_policy` | `stutter-forever, deadlock-violation, finite-trace-only, closed` (RFC 0015) | no engine may silently change it |
| `nondeterminism` | typed choice classes per site (`demonic, angelic, scheduler, probabilistic, timed, epistemic`) | probabilistic/timed/epistemic declarative until ADR-0016 |
| `assurance` | minimum evidence class + checker requirements | total order: `observed < sampled < bounded < validated < proved` |
| `optimization` | hard constraints, soft objectives, non-vacuity behaviors | INV-012 |
| `security_policy` | data classification, redaction classes, capability requirements | first-class per plan §0.1/§5.2; weakening is a protected change |
| `policy` | per-field change verb | see below |

## Canonical identity

- The `in_*` identity MUST derive from a canonical encoding of the semantic content: sorted keys, property ASTs in the CPNF-1 normal form specified in the next section (alpha-renamed binders, flattened and ordered associative/commutative connectives), and no comment/whitespace/label content.
- `name` and human labels are metadata and MUST NOT affect identity. So are `expression.source` and every `$comment`.
- Encoding MUST be byte-deterministic across platforms and releases within a schema version; golden identity vectors are part of the acceptance suite.
- In certified lanes, identity collisions MUST be resolved by canonical comparison (ADR-0013).

## Property AST and canonical normal form (CPNF-1)

Absorbed from plan §25 (SD-07). Property expressions are structured ASTs, not strings. A string carries no soundness: under textual comparison a rename or a rewrite is indistinguishable from a weakening, which is precisely the reward-hacking path INV-001 exists to close (RFC 0031 rejects token/set comparison of property strings as the production mechanism). The fields carrying an AST are `claims[].expression`, `assumptions[].expression`, and `fairness[].condition`; the structure is normative in [`../schemas/intent-contract.schema.json`](../schemas/intent-contract.schema.json), the semantics here.

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
- Migrating a stored contract changes its canonical encoding and therefore its `in_*` identity. A migrated contract MUST be re-accepted through `intent.accept` and MUST NOT be treated as an ordinary edit. The cross-schema `$id`/schema-epoch convention that would carry such a migration uniformly is separate specification debt (plan §25, SD-08) and is not settled here.

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

| Verb | Meaning |
|---|---|
| `unlocked` | ordinary edits allowed |
| `proposal-only` | agents may propose; humans accept |
| `review` | any change requires named-reviewer approval |
| `locked` | no change without policy amendment |
| `no-decrease` | changes classified as decrease (bounds) are blocked |
| `no-removal` | removals (faults, non-vacuity behaviors) are blocked |
| `no-downgrade` | assurance downgrades are blocked |
| `no-expansion` | growth of the set (opaque boundaries) is blocked |

The verb set is closed. A proof-gated acceptance verb (`proof-required`) is a candidate future addition and requires an explicit revision of this RFC (see Open questions); plan §5.4's policy examples use only closed-set verbs.

- The protected set is: properties, assumptions, observers, bounds, faults, **fairness**, **trust boundaries**, **non-vacuity**, **completion policy**, assurance, **security policy**, **optimization**, **scope**, **nondeterminism**, and **abstraction maps** (ADR-0039; all fifteen MUST be lockable — plan §5.4).
- Directional verbs (`no-decrease`, `no-downgrade`, `no-expansion`) are only enforceable where RFC 0031 defines the order for that field. Where the order is undefined or the change is outside declared fragments, the change classifies `Unknown` and MUST be treated as blocked pending review (fail closed, plan §5.3).
- The default agent repair capability profile MUST deny every protected change; a change slipped into a repair reclassifies the transaction as an intent revision (RFC 0032, INV-011).

## Drafting and acceptance

- `cargo continuum init` MAY propose draft contracts from templates, property libraries, and observed effect footprints. Drafts enter the registry at evidence status `Proposed` and gain INV-001 protection only on explicit human acceptance (plan §3.1).
- The `Proposed` → protected transition is performed only by the `intent.accept` operation (RFC 0027): it requires the `revise-intent` capability, produces an audit record, and is never performed implicitly by `init`, `check`, or any repair operation (plan §5.4). `intent.lock` edits the §5.4 policy table under the same capability.
- Continuum MUST NOT silently promote an inferred intent to protected status, and MUST NOT claim a generated model is the intended abstraction (docs/37).

## Revision procedure

1. A revision is submitted against a named base contract version.
2. RFC 0031 classifies every field change; undecidable cases yield `Unknown`.
3. Policy verbs are evaluated against the classification; any blocked or `Unknown` protected change requires the review path.
4. On acceptance, dependent evidence is invalidated per the impact set, a new `in_*` identity is issued, and the supersession edge is recorded in the evidence graph.

## Rejected alternatives

- **Intent embedded in the workspace snapshot.** Rejected: forkable containers cannot protect their contents (research/33 principle 1; the fork-with-modified-intent gaming path).
- **Free-form policy strings.** Rejected: directional enforcement requires a closed verb set with defined orders.
- **Identity from source text.** Rejected: comment/formatting changes must not invalidate evidence; renames must not evade the diff.

## Open questions

- Property-AST normalization rules per fragment beyond Finite (owned jointly with RFC 0031). CPNF-1 above fixes the Finite core; `Symbolic`, `Temporal`, and `Probabilistic` normalization is open, and until it is settled those fragments get identity and `unchanged` from CPNF-1 but no directional relation.
- Whether the v1 term sort grows a declared-algebraic-law table, so `apply` arguments over declared-commutative or declared-associative operators can be ordered by CPNF-1 instead of compared positionally.
- Whether `optimization.non_vacuity` behaviors and observer projections, still carried as strings, move to typed forms; RFC 0031 classifies them by set membership only, so no expression order is needed today.
- Whether a proof-gated acceptance verb (`proof-required`) joins the closed verb set; adding it requires a revision of this RFC, and plan §5.4's examples use only closed-set verbs until then.

(Resolved: `security_policy` is a first-class field group, carried by the
schema, lockable per plan §5.4, and diffed per RFC 0031's security-policy
order.)

## Acceptance

- Round-trip schema validation; deterministic identity across platforms (golden vectors).
- CPNF-1 golden vectors: `normalize` is idempotent (N9) and byte-identical across platforms and releases.
- A rewrite corpus — `implies`/`leads_to` expansion, De Morgan pushes, bound-variable renaming, junction reordering and duplication, `gt`/`ge` swaps — MUST normalize to identical CPNF-1 encodings and classify `unchanged`; a paired weakening corpus MUST NOT classify `unchanged`, and MUST classify a proved direction or `unknown`.
- Fairness conditions carrying a temporal operator MUST be rejected by the schema, not by prose.
- All G0-DX-02 gaming mutations classify as protected changes; a source-only guard repair does not.
- Policy-verb enforcement tests for all fifteen protected fields, including `Unknown`-fails-closed.
- Adversarial suite: renaming, formatting, comment, and label changes MUST NOT change identity; semantic changes MUST.
