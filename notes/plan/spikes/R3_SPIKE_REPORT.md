# Revision 3 Executable Spike Report

**Executed with:** Python 3.13.5  
**Result artifact:** [`results/r3-spike-results.json`](results/r3-spike-results.json)  
**Runner:** [`run_r3_spikes.py`](run_r3_spikes.py)

All eight Revision 3 spike groups passed their internal assertions.

Section 9 is a later addition and is not one of those eight: it is a Rust falsification campaign against a landed reference implementation, executed by `cargo test` rather than by the Python runner above, and it records a failure and the fix that closed it.

## 1. Context Pack causal slicing

The synthetic durability trace contained **200 events**: four causal events and 196 observer-independent noise events.

Results:

- replay-preserving core: **4 events**;
- omitted events: **196**;
- raw JSON: **17,645 bytes**;
- Context Pack: **747 bytes**;
- compression ratio: **23.62×**;
- core was 1-minimal under single-event deletion;
- core replay still produced `acked = true ∧ durable = false`.

Core:

```text
WriteSubmitted
  → ReplyReserved
  → CancelRequested
  → ReplyPublished
```

This validates the basic artifact shape, not the general context compiler. Real traces need conflict, abstraction, proof, and domain semantics.

## 2. Semantic intent diff

The spike correctly distinguished an ordinary source guard repair from six formal reward-hacking mutations:

| Mutation | Classification |
|---|---|
| source guard repair | protected intent unchanged |
| remove property term | property weakened |
| add favorable assumption | assumptions strengthened |
| nodes 5 → 3 | bound contracted |
| hide `SyncCompleted` | observer coarsened |
| remove crash fault | fault envelope contracted |
| validated → sampled | assurance downgraded |

All expected protected changes were detected.

## 3. Explicit agent protocol state

The in-memory workbench demonstrated:

- canonical workspace snapshot identities;
- idempotent identical task creation;
- rejection of idempotency-key reuse with different request;
- resumable continuation;
- rejection of continuation under a different workspace snapshot.

This supports the explicit-handle architecture. It does not yet test concurrency, authorization, persistence, or crash recovery.

## 4. Forge finite CEGIS

Candidate grammar:

```text
true
false
reserved
¬reserved
¬synced
reserved ∧ synced
synced
```

Constraints:

- safety: acknowledgement implies synced;
- non-vacuity/progress: a reserved and synced request can acknowledge.

CEGIS iterations:

1. `false` rejected by progress counterexample;
2. `reserved` rejected by safety counterexample;
3. `synced` accepted.

The only valid grammar candidates were `synced` and `reserved ∧ synced`; AST-size objective selected `synced`. Invalid candidates were exhausted in the finite grammar.

This validates coupling safety with positive behavior. It is not evidence that broad protocol synthesis will scale.

## 5. Incremental query invalidation

A ten-query synthetic graph was tested under property, source, model, proof-only, and domain-profile edits.

Findings:

- all incremental outputs matched clean recomputation;
- property edit reused source parsing/extraction and model elaboration;
- proof-only edit invalidated only the proof receipt;
- source edit invalidated extraction, correspondence, refinement, context, and proof receipt;
- domain-profile edit invalidated both model and program semantic paths.

The spike validates the edge taxonomy idea but not production dependency capture.

## 6. Causal debugger

At a configuration with submitted and reserved work, the enabled frontier was:

```text
SyncCompleted
CancelRequested
```

Branches:

- `SyncCompleted → NormalPublishes` produced `acked ∧ durable`;
- `CancelRequested → FinalizerPublishes` produced `acked ∧ ¬durable`.

The spike emitted why-enabled conditions, first conflicting choices, abstract branch difference, and causal reverse choice.

## 7. Proof-oriented lens conflict

The abstract field `durable` could legitimately map to either:

- local storage stability;
- quorum acknowledgement under a stronger domain contract.

The reverse abstract edit therefore had two concrete candidates. The spike returned `AmbiguousCorrespondence` and a distinguishing obligation instead of selecting silently. Direct acknowledgement/publication mapping remained unambiguous.

## 8. Multi-agent Evidence Graph

The immutable graph spike coordinated two independent patch proposals and verifier/kernel evidence. It demonstrated:

- byte-identical proposals deduplicate by content identity;
- an agent cannot publish `validated` or `proved` status;
- stale clean-parity evidence cannot complete a repair envelope;
- the safe acknowledgement patch promotes only after fresh exact replay, neighborhood, mutation-challenge, and clean-parity evidence exist;
- a fault-model contraction is blocked as a protected intent change;
- contradictory claims about the same subject and predicate become an explicit conflict rather than last-writer-wins state.

The run produced ten immutable nodes, seven typed edges, one surfaced claim conflict, and one accepted repair envelope. This validates authority separation and coordination semantics, not distributed storage or Byzantine agent resistance.

## 9. G0-DX-13 falsification campaign

**Executed with:** `cargo test --workspace --locked` (`just check`)  
**Sources:** `crates/continuum-workspace/tests/dx13_falsification.rs`, `crates/continuum-workspace/tests/dx13_mutation_campaign.rs`

Sections 1–8 validate artifact shapes with finite Python spikes. This one is a different kind of evidence and is reported separately for that reason: an adversarial campaign against the *landed reference implementation* of atomic publication (`crates/continuum-workspace/src/publication.rs`), written to make the G0-DX-13 pass condition — "one semantic identity; no lost receipts; deterministic result" — false rather than to confirm it. No `src/` file was modified by the campaign, and every assertion is on an outcome; interleavings are forced through the `StorageFaults` seam where a specific one is wanted, and never asserted.

### Attacks the property survived

| Attack | Scale | Outcome |
|---|---|---|
| identical publication under contention | 32 publishers × 12 rounds | one identity, 32 distinct receipts, schedule-independent store |
| identical and distinct payloads in one store at once | 48 publishers, 6 payloads, 6 rounds | 6 identities, 8 receipts each, content stored once per identity |
| identical bytes across three artifact classes | 24 publishers | 3 identities; per-class attribution intact |
| abandoned stagings interleaved with commits | 48 threads, 4 rounds | 24 receipts, 24 staging/abandoned aborts, no residue |
| crash between the commits, concurrent with success | 48 threads, 4 rounds | residue absorbed by convergence; `fsck` clean; no receipt for a crashed publisher |
| refusal injected at every publication phase | 24 publishers × 3 phases × 3 rounds | 12 successes and 12 aborts per phase; nothing half-published |
| universal refusal at the index commit | 24 publishers | nothing visible; residue classified unreachable content and reclaimed |
| colliding distinct values, four collision families | 32 publishers × 8 rounds | no conflation; losers refused `IdentityCollision`; no receipt names another publisher's bytes |
| total hash collision | 32 publishers × 6 rounds | exactly one artifact; receipts only for the winner's bytes |
| readers and `fsck` against a publishing store | 16 + 8 + 4 threads, 200 probes each | no missing referent, no identity mismatch, no partial read |
| idempotent replay under contention | 16 publishers × 8 replays | 128 receipts, one identity, one stored copy, one reported cost |
| capability mint/revoke racing publication | 24 publishers × 4 rounds | consistent store; a denial is never recorded as an abort |

### The property broke: garbage collection racing an in-flight publication

`ReferenceStore::collect_garbage` derives its root set from the index alone. A publication that has completed its content commit and not yet its index commit is unreachable by that definition, so the collector reclaims content a live publisher has already been told is durable. Three failing tests are retained (marked `#[ignore]` so `just check` stays green; run with `-- --ignored`):

- `a_receipt_must_name_a_readable_artifact_when_gc_runs_concurrently` — the publisher receives a receipt, the artifact becomes visible, and reading it returns `CapabilityDenied`. `fsck` reports a **missing referent**, the defect class the module documents as never producible by this store's own ordering;
- `concurrent_gc_must_not_let_a_receipt_name_another_publishers_bytes` — deleting the in-flight content also deletes the value ADR-0013's exact comparison was about to compare against, so under a colliding identifier a second publisher's byte-different value is stored at the first publisher's identity and the first publisher's receipt names bytes it never published;
- `unassisted_concurrent_gc_and_publication_lose_committed_content` — the same defect from ordinary threads with no fault seam. Corroboration only: it passes vacuously when the scheduler serializes the publications.

Three documented claims are false as a result: INV-017's "visible only after content is durably committed" is violated in the inverted direction (visible, with no content); `collect_garbage`'s "collecting it cannot make a published artifact unavailable"; and docs/35's root set — "named roots, live tasks, receipts, and retention policy" — which is implemented as index entries alone and omits both receipts and in-flight publications. The pass condition's third clause fails outright: the same inputs produce different stores. Diagnosis and repair belong to a follow-up item; the campaign did not patch `src/`.

### Adversarial mutations

The promotion rule's third requirement, and the reason the twelve passes above are worth reading. Three plausible bugs were introduced into a *separate* re-implementation of the publication core and graded by the same three checks as the real store:

| Store under test | convergence and receipts | atomic abort | exact comparison |
|---|---|---|---|
| reference implementation | pass | pass | pass |
| faithful re-implementation (control) | pass | pass | pass |
| index entry written before content | pass | **caught** | pass |
| last-writer-wins receipt ledger | **caught** | pass | **caught** |
| colliding content overwrites instead of aborting | pass | pass | **caught** |

Each catch asserts *which* complaint the check produced, so a coincidental catch cannot be mistaken for a targeted one, and the control shows the checks reject incorrectness rather than unfamiliarity.

### Limits

One process, one mutex, no disk. The campaign says nothing about daemon-scale linearizability, real crash recovery, restore, or a durable store; that evidence is `continuumd`'s, PR 6+ (docs/35). The garbage-collection defect is a finding against the in-memory reference — whether a durable implementation inherits it is a design question the follow-up owns, not one this campaign answered. The three retained failures are excluded from `just check` by `#[ignore]`, which keeps the gate honest about what is green but means the defect will not re-announce itself; it is recorded here and in the G0 matrix instead.

### Resolution — fixed by bn-2siid

The defect is repaired, and the two subsections above are retained as the record of how it was found rather than as a live finding.

`ReferenceStore`'s reachability root set now carries both halves of docs/35's: **named roots** are the index entries as before, and **live tasks** are the publications between their two commits. A publication pins its identity as a root in the same critical section that makes its content durable, and releases it only when its index commit lands or it is abandoned — the pin's lifetime is the lifetime of the `CommittedContent` witness, so no exit from that state can forget it. A collector running concurrently therefore cannot reclaim content a publisher has already been told is durable. Receipts were checked and deliberately *not* added as roots: a receipt is appended in the same critical section that inserts the index entry naming it, an `ArtifactPath` determines that entry's identity, and nothing removes an index entry — so every receipted identity is already a named root, and a receipt root would be dead machinery.

All three reproductions now pass with their assertions intact, their `#[ignore]` attributes are removed, and they run in the default `cargo test`. That pays off the cost recorded under **Limits** above: the suite re-announces this defect if it ever returns. Each still fails against the pre-fix store, which is what makes them worth keeping.

One reproduction changed with the fix, and the change is recorded on bone bn-2siid: `concurrent_gc_must_not_let_a_receipt_name_another_publishers_bytes` asserted that the second, colliding publisher *succeeded* — but that success was itself the defect, since it happened only because collection had deleted the operand ADR-0013's exact comparison was about to use. With the first publisher's content pinned, the second is refused with `IdentityCollision`, and the test asserts that refusal instead. The claim is strictly stronger than the one it replaces.

## What the spikes changed

The experiments support five architecture decisions:

1. bounded Context Packs can be dramatically smaller while preserving a witness;
2. intent integrity needs a dedicated semantic diff, not code review convention;
3. explicit handles provide deterministic agent handoff and stale-state rejection;
4. Forge requires positive/non-vacuity constraints and independent verification.
5. Multi-agent work needs immutable evidence, authority-separated promotion, stale-evidence rejection, and explicit conflicts.

## What remains unproven

- context slicing on real causal/conflict graphs;
- human/agent performance improvements;
- sound semantic implication for rich temporal properties;
- persistent/concurrent daemon behavior;
- incremental correctness under real edits;
- scalable CEGIS/co-synthesis;
- verified lens laws and effectful correspondence;
- Lean kernel checking of Revision 3 seed modules.
