# Repair Transactions

> **Status:** This document predates the 12-gate, phase-profile repair design and is scheduled for regeneration (plan §25). **RFC 0032 and `schemas/repair-transaction.schema.json` are normative**; where this document's 9-step pipeline disagrees with RFC 0032's twelve gates (including `incremental_parity` as a gate, `gate_profile`, and `not_yet_enforced` statuses), the RFC governs.

## Purpose

Make autonomous repair safe, reproducible, and reviewable by treating repair as an evidence-bearing transaction rather than an edit followed by tests.

## State machine

```text
Draft
  → Applied
  → Evaluating
  → Ready | Blocked | Inconclusive
  → Promoted | Rejected | Superseded
```

Each transition creates a new immutable transaction version.

## Required fields

```rust
struct RepairTransaction {
    base_snapshot: SnapshotHandle,
    base_intent: IntentHandle,
    failure: EvidenceHandle,
    hypothesis: Hypothesis,
    proposed_changes: ChangeSet,
    actor: ActorId,
    evaluation_policy: RepairPolicy,
    evidence: Vec<EvidenceHandle>,
    status: RepairStatus,
}
```

The hypothesis is separate from evidence. Agents can be wrong without corrupting the record.

## Change sets

A transaction may contain:

- Rust patch;
- CML change;
- correspondence change;
- proof change;
- domain-pack/config change;
- explicit Intent Contract revision.

The last category automatically changes transaction class and review policy.

## Evaluation pipeline

### 1. Base validation

Reproduce failure on the exact base snapshot and intent. If replay diverges, block with `ReplayDiverged`.

### 2. Apply and seal

Apply changes in a forked snapshot. Normalize and hash. Reject hidden filesystem edits.

### 3. Semantic/intent diff

Classify changes. Protected intent change blocks ordinary repair policy.

### 4. Exact regression

Replay original execution choices where still meaningful. If the patch eliminates an event, use correspondence to report where replay diverges and continue under a declared policy.

### 5. Causal neighborhood

Explore alternate choices around the failure slice.

### 6. Mutation challenge

Run property/model/code mutants and hidden variants to detect overfitting or accidental verifier disablement.

### 7. Proof/refinement rebuild

Invalidate and rebuild affected evidence. Reject stale receipts.

### 8. Nonfunctional checks

Measure performance/resource/security changes relevant to intent.

### 9. Policy decision

A trusted policy engine evaluates evidence and unknowns.

## Promotion receipt

A receipt contains:

- base and candidate snapshot IDs;
- intent IDs and diff;
- failure identity;
- hypothesis and patch hash;
- exact replay outcome;
- neighborhood definition and coverage;
- mutation results;
- proof/certificate receipts;
- clean/incremental parity;
- performance/security findings;
- unresolved unknowns;
- policy and signer/checker identities.

## Agent swarm

Different agents may attach:

- diagnosis;
- candidate patch;
- invariant/proof repair;
- adversarial review;
- benchmark result.

They do not edit one mutable transaction document. They append typed proposals/evidence nodes. The transaction manager derives status.

## Minimality

Continuum may compute patch subsets and semantic changes to identify unnecessary edits. Minimal textual patch is not always best; the review shows both textual and semantic footprint.

## Failure modes

- **Exact overfit:** original trace fixed, sibling trace fails.
- **Intent gaming:** property/assumption/bound changed.
- **Verifier gaming:** instrumentation or property monitor disabled.
- **Stale proof:** receipt from old snapshot reused.
- **Availability collapse:** safety fixed by suppressing progress.
- **Abstraction gaming:** concrete bad states merged.
- **Opaque escape:** affected effect marked unsupported/opaque.

Each has an explicit gate and benchmark mutation.
