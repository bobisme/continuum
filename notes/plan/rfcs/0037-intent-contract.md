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

## Fields

The contract consists of these field groups (types in the schema):

| Group | Content | Notes |
|---|---|---|
| `claims` | property AST + stable semantic id + kind (`safety, liveness, refinement, hyperproperty, security, performance`) + observer | at least one claim required |
| `assumptions` | classified expressions (`environment, scheduler, timing, storage, network, trust`), each MAY declare a domain-pack `fidelity_profile` (`ideal, contractual, platform-qualified, adversarial-envelope`; RFC 0002 — profiles are NOT automatically ordered) | timing assumptions are declarative until ADR-0016 lanes ship |
| `observers` | event families + state projections | the unit reductions are justified against (INV-013) |
| `scope` | components, abstraction level, declared semantic `fragments` (`Finite, Symbolic, Temporal, Probabilistic, Theorem, Runtime`; ADR-0025) | fragments bound what the diff can classify (see RFC 0031) |
| `trust_boundaries` | trusted and opaque components/effects | expansion of the opaque set is a protected change ("opaque escape", docs/41) |
| `bounds` | values, nodes, faults, depth | componentwise partial order; see RFC 0031 |
| `fault_model` | enabled fault classes + pack profiles | |
| `fairness` | weak/strong constraints per action | protected: fairness strengthening is the canonical gaming vector (docs/50) |
| `completion_policy` | `stutter-forever, deadlock-violation, finite-trace-only, closed` (RFC 0015) | no engine may silently change it |
| `nondeterminism` | typed choice classes per site (`demonic, angelic, scheduler, probabilistic, timed, epistemic`) | probabilistic/timed/epistemic declarative until ADR-0016 |
| `assurance` | minimum evidence class + checker requirements | total order: `observed < sampled < bounded < validated < proved` |
| `optimization` | hard constraints, soft objectives, non-vacuity behaviors | INV-012 |
| `policy` | per-field change verb | see below |

## Canonical identity

- The `in_*` identity MUST derive from a canonical encoding of the semantic content: sorted keys, normalized property ASTs (alpha-renamed binders, normalized associativity/commutativity where the fragment defines it), and no comment/whitespace/label content.
- `name` and human labels are metadata and MUST NOT affect identity.
- Encoding MUST be byte-deterministic across platforms and releases within a schema version; golden identity vectors are part of the acceptance suite.
- In certified lanes, identity collisions MUST be resolved by canonical comparison (ADR-0013).

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

- The protected set is: properties, assumptions, observers, bounds, faults, **fairness**, **trust boundaries**, **non-vacuity**, **completion policy**, assurance (ADR-0039; all ten MUST be lockable — plan §5.4).
- Directional verbs (`no-decrease`, `no-downgrade`, `no-expansion`) are only enforceable where RFC 0031 defines the order for that field. Where the order is undefined or the change is outside declared fragments, the change classifies `Unknown` and MUST be treated as blocked pending review (fail closed, plan §5.3).
- The default agent repair capability profile MUST deny every protected change; a change slipped into a repair reclassifies the transaction as an intent revision (RFC 0032, INV-011).

## Drafting and acceptance

- `cargo continuum init` MAY propose draft contracts from templates, property libraries, and observed effect footprints. Drafts enter the registry at evidence status `Proposed` and gain INV-001 protection only on explicit human acceptance (plan §3.1).
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

- Property-AST normalization rules per fragment beyond Finite (owned jointly with RFC 0031).
- Whether `security_policy` becomes a first-class field group or remains under `assumptions(trust)` + `trust_boundaries` (plan §0.1 lists it; the schema currently does not carry it separately).

## Acceptance

- Round-trip schema validation; deterministic identity across platforms (golden vectors).
- All G0-DX-02 gaming mutations classify as protected changes; a source-only guard repair does not.
- Policy-verb enforcement tests for all ten protected fields, including `Unknown`-fails-closed.
- Adversarial suite: renaming, formatting, comment, and label changes MUST NOT change identity; semantic changes MUST.
