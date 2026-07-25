# Revision 3 Executable Spike Report

**Executed with:** Python 3.13.5  
**Result artifact:** [`results/r3-spike-results.json`](results/r3-spike-results.json)  
**Runner:** [`run_r3_spikes.py`](run_r3_spikes.py)

All eight Revision 3 spike groups passed their internal assertions.

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
