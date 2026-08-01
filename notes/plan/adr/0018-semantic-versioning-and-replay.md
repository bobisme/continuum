# ADR-0018: Version semantics independently from APIs and artifact formats

**Status:** Accepted  
**Date:** 2026-07-24  
**Decision owners:** Continuum architecture group  
**Normative language:** MUST/MUST NOT/SHOULD/MAY per RFC 2119.

> **Scope note:** This ADR absorbs plan §4.6 (semantic epoch advance) — specification
> debt SD-09 in plan §25, now paid. It is normative for what an epoch is, what
> advancing one obliges, and what an advance may never do. Where this ADR and plan §4.6
> disagree, this ADR is corrected and becomes normative (plan §25); the corrections are
> recorded under [Corrections](#corrections). The daemon-side operational duties are
> [docs/35](../docs/35_CONTINUUMD_WORKBENCH_DAEMON.md); the effect on cached and reused
> results is [docs/42](../docs/42_INCREMENTAL_VERIFICATION.md); schema-document identity
> and `schema_epoch` are [`../schemas/README.md`](../schemas/README.md); the wire form is
> RFC 0026 and its IDL.

## Context

Semver of crates does not identify the meaning of an old model or trace. Replay can silently change under new pack/runtime semantics.

The failure this decision prevents is not a build break. It is a published receipt whose meaning changes underneath it — evidence that was true about one semantics being read as evidence about another. Long-lived evidence is the product; a version scheme that cannot say which semantics a receipt was earned under makes that product worthless.

## Decision

**Epochs are content identities, not release numbers.** An epoch names the meaning an artifact was produced under, and it is pinned into the artifact. Advancing an epoch never mutates an existing artifact.

Continuum versions six compatibility epochs independently — **protocol, semantic, intent, evidence, proof, corpus** — and they MUST NOT be conflated with one another (docs/12 §7, RFC 0026 `rule versioning.epoch_independence`). They are distinct types, not a shared integer: `crates/continuum-value/src/epoch.rs` gives each its own newtype with no conversions between them, so passing a proof epoch where a semantic epoch is expected does not compile.

Beside the six, every evidence artifact binds the digests that fix its own shape and inputs: model-language, CIR, certificate, crashpack, and domain-pack digests. Those are content bindings, not epochs; they are what makes a pinned epoch checkable rather than asserted. Breaking semantics cause refusal or explicit migration, never best-effort reinterpretation.

### What an epoch advance obliges

1. **Evidence is epoch-scoped.** A receipt MUST remain verifiable under its pinned epoch indefinitely (INV-006, INV-014). An advance MUST NOT silently revalidate or invalidate a published receipt, and MUST NOT change what a published receipt claims.
2. **A compatibility statement is published before the advance is applied.** Each advance MUST publish a typed per-artifact-class statement — `Preserved | Revalidate | Incompatible` — together with an estimated invalidation blast radius by artifact class. The vocabulary is closed:
   - `Preserved` — artifacts of the class keep their meaning and their evidence standing;
   - `Revalidate` — artifacts remain readable, but their claims MUST be re-established under the new epoch before they are relied on again;
   - `Incompatible` — artifacts of the class do not carry across. Nothing is rewritten in place; re-derivation produces new identities.

   The statement is `EpochAdvanceNotice` on the wire and `EpochAdvance` in `crates/continuum-value/src/epoch.rs`, which refuses an advance whose successor identity equals its predecessor — an advance always produces a new identity.
3. **At most two epochs of a kind are held concurrently** during a migration. New work defaults to the newest. A continuation resumes only under its pinned epoch (plan §9.6) and is forked, never migrated in place; a resume under a different epoch MUST be rejected (`ContinuationEpochMismatch`), and an artifact declaring an epoch the reader does not implement MUST be rejected (`EpochUnsupported`), never best-effort decoded (docs/09 T13).
4. **Re-derivation, never rewriting.** Re-derived artifacts receive new identities linked to their predecessors by `SUPERSEDES` edges.

### Ordering, and why five of the six have none

Succession is a published event, not a computation over identities. A content identity such as a pinned corpus revision or a Lean toolchain image has no successor relation derivable from the identity itself, so `SemanticEpoch`, `IntentEpoch`, `EvidenceEpoch`, `ProofEpoch`, and `CorpusEpoch` carry equality and hashing but deliberately no ordering; `EpochAdvance` is the only successor relation.

The protocol epoch is the exception and is totally ordered on `major.minor`, because the handshake selects the highest common version and the daemon serves majors N and N−1 (RFC 0026). Both statements require a comparison, and neither is a statement about meaning.

### What is not an epoch

- **Engine identity** (plan §4.7) pins which engine build produced a result. It is carried in the result envelope's `epochs` block as `engine` and is pinned by continuations and `defect_*` artifacts, but it is not one of the six independently versioned compatibility epochs.
- **`schema_epoch`** ([`../schemas/README.md`](../schemas/README.md)) versions one artifact class's encoding contract and pins no semantics. It is an epoch advance *in the sense of this ADR* — it publishes the same compatibility statement and obeys the same two-epoch rule — but it is not a seventh member of the six. Advancing the `schema_epoch` of an evidence class is one of the events that MAY advance the evidence epoch; the converse does not hold, and an evidence epoch MUST NOT be inferred from a `schema_epoch` value.
- **The protocol major** is a connection property. A protocol-major bump MUST NOT, by itself, invalidate or reinterpret a published artifact, and retiring a protocol major MUST NOT orphan one; the protocol epoch therefore MUST NOT enter the identity of any artifact, snapshot, or task (plan §25, SD-13).

## Consequences

Long-lived replay and evidence become possible. Version management and migration tooling are mandatory. Three further consequences follow from the clauses above:

- migration tooling MUST be able to compute a blast radius per artifact class before an advance is applied, which makes per-class storage attribution (plan §4.5, docs/35) a prerequisite rather than a nicety;
- the two-epoch window bounds migration cost but also bounds migration *time*: a migration cannot be left half-finished indefinitely, because a third advance cannot begin until the older epoch is drained;
- cached and reused results are epoch-scoped like everything else. A `Revalidate` or `Incompatible` statement is what invalidates reuse across an advance, not a heuristic (docs/42).

## Alternatives considered

1. Use crate semver only: insufficient.
2. Best-effort backward compatibility: dangerous for evidence.
3. Freeze semantics permanently: unrealistic before maturity.
4. One monotonic "workbench epoch" covering all six dimensions: rejected. It forces an advance in one dimension to invalidate artifacts in five others, which is precisely the silent invalidation clause 1 forbids.
5. Migrate artifacts in place on advance: rejected. In-place migration mutates a published artifact, which breaks content addressing and destroys the evidence a receipt was earned against.

## Corrections

Per plan §25 the absorbing document is corrected and becomes normative; the direction of each correction is recorded.

| # | Disagreement | Direction | Resolution |
|---|---|---|---|
| 1 | This ADR's original decision pinned "model-language, semantic epoch, CIR, certificate, crashpack, and pack versions" — a pre-Revision-3 list that neither matches nor maps onto the six independently versioned epochs of docs/12 §7, RFC 0026, and `crates/continuum-value/src/epoch.rs`. | docs/12 §7, RFC 0026, epoch.rs → ADR-0018 (replaced) | Six epochs: protocol, semantic, intent, evidence, proof, corpus. The model-language, CIR, certificate, crashpack, and pack digests remain required bindings on every evidence artifact, but they are content bindings, not epochs. |
| 2 | Plan §4.7 and RFC 0026's result-envelope table speak of an "engine epoch" alongside the six. | plan §4.7 → ADR-0018 (renamed) | Engine identity is not a compatibility epoch. It is pinned in the `epochs` block as `engine` and by `defect_*` artifacts; it is out of scope for the six-epoch independence rule. |
| 3 | Plan §4.6 says an advance "publishes" a compatibility statement without fixing when, leaving room for publishing it with the advance. | plan §4.6 → ADR-0018 (tightened) | The statement and the blast radius MUST be published *before* the advance is applied. A statement published simultaneously with the advance gives no one a chance to act on it. |

## Validation and rollback

G0 schemas include all version fields. Release tests replay a corpus from prior supported epochs. Additionally:

- an advance whose successor identity equals its predecessor is rejected by construction (`EpochAdvanceError::NotAnAdvance`), so "advanced but unchanged" is unrepresentable rather than merely discouraged;
- a `Preserved` statement is falsifiable: replaying the prior corpus under the new epoch MUST reproduce identical verdicts for every class declared `Preserved`, and a mismatch is an engine defect (plan §4.7), not an acceptable drift;
- rollback is an ordinary advance back to the predecessor identity, with its own compatibility statement. There is no in-place undo, because there was no in-place change to undo.
