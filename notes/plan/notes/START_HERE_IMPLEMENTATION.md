# Start Here: Revision 3 Implementation Sequence

## Governing rule

Close the smallest complete trustworthy loop before broadening the verifier:

```text
protected intent
  → immutable snapshot
  → typed verification task
  → exact failure
  → bounded Context Pack
  → causal debugger
  → repair transaction
  → semantic/intent diff
  → neighborhood and mutation challenge
  → independent evidence
  → promotion receipt
```

Do not begin with a polished web UI, a general LLM coordinator, distributed search, or the full CML surface language (the Finite core fragment lands at PR 15a). The first users are the implementation team and coding agents driving the first vertical slice.

## Dependency islands

```text
continuum-workspace / continuum-intent
          │
          ▼
continuum-task / continuum-evidence / continuumd (native protocol)
          │
    ┌─────┼─────────┐
    ▼     ▼         ▼
 model   program    proof
 core    adapter    checker
    └─────┼─────────┘
          ▼
context / debugger / diff / repair
          │
          ▼
       Forge
```

There is no `continuum-protocol` crate; the native protocol lives in `continuumd`. The full Revision 3 crate list is plan §20.

The certificate checker may not depend on search. The model core may not depend on asupersync. Adapters may not own semantic state. Forge may not be imported by the verifier.

## Swarm execution map (plan §21.1)

Continuum is implemented by an elastic swarm of autonomous agents. Bones
is the scheduling control plane: agents claim dependency-ready work,
independent bones may run in parallel, and completion requires the
specified verification and integration evidence. No phase waits for a
named owner or minimum human team.

| Phase | Opening PR | Dispatch condition |
|---|---|---|
| A | PR 0 | The bone is dependency-ready |
| B | PR 14 | Phase A exit accepted and the bone is dependency-ready |
| C | PR 23 | Phase B exit accepted and the bone is dependency-ready |
| D | PR 28 | Phase C exit accepted and the bone is dependency-ready |
| E | PR 29 | Phase D exit accepted and the bone is dependency-ready |
| F | first post-PR-30 PR (unsequenced) | Phase E exit accepted and the bone is dependency-ready |

The map is an ordering and dispatch aid, not a staffing gate. Evidence,
authority, and phase-exit requirements remain fail-closed; a producing
agent may not self-attest evidence that requires independent checking.

Opening-PR designations follow plan §21's phase deliverables (the PR sequence below interleaves phases by design and is not grouped by phase): PR 0 is Phase A's opening PR per §21.1; PR 14 (asupersync semantic journal) opens Phase B; PR 23 (incremental query database) opens Phase C — PR 22a precedes it but delivers the Phase B ADR that unblocks Phase C; PR 28 (Lean proof service) opens Phase D; PR 29 (Forge finite CEGIS) opens Phase E. Every Phase F deliverable is in the deliberately deferred set below, so Phase F's opening PR is the first PR sequenced after PR 30 closes; it must be designated here before Phase F work begins.

Cross-cutting dispatch follows two additional rules. Each active §24.5
frontier lane has its own ratification leaf; Phase C and later deadlines become
dispatchable during the immediately preceding phase and block their first
consumer. Each docs/08 risk or §24 kill-signal assay is assigned to the earliest
phase where its evidence can be complete and blocks that phase's integrated exit
package. Agents prepare those packages, but continue/narrow/defer/kill decisions
and every `goal:manual` phase exit remain privileged human actions. The generated
`notes/PLAN_BONE_TRACEABILITY.md` report applies Bones' blocked-goal ancestor
propagation when reporting the live `bn next` dispatch frontier.

## First pull requests (PR 0 – PR 30)

PR numbers 1–30 are stable; inserted work carries PR 0 or a lettered suffix (4a, 15a, 15b, 22a, 25a, 26a, 27a, 27b). Each heading names the release gate(s) the PR advances, per plan §21's phase deliverables and the G0 staging rule; some PRs carry gates from later phases where §21 explicitly pulls work forward (e.g. PR 4a [G6; Phase A band]).

### PR 0 — Specification pass and program decisions [G1, G2]

Implement (documentation, not code):

- expand RFCs 0026, 0027, 0028, 0030, 0031, 0032, 0037 from summaries to full specifications: field types, enums, classification lattices, IDL, versioning, RFC-2119 language (delivered: bn-3ffu, bn-26fh, bn-4xxw, bn-3hkk, bn-39my, bn-3i38, bn-2s6q);
- reconcile each RFC one-to-one with its plan section; where they disagree, correct the RFC and make it normative (delivered: bn-3ffu, bn-26fh, bn-4xxw, bn-3hkk, bn-39my, bn-3i38, bn-2s6q — each expansion records its corrections under "Corrections recorded by this RFC");
- product license decision (permissive, compatible with asupersync and solver adapters, §21.1) (delivered: bn-2br);
- corpus per-family redistribution audit before any public benchmark release (§21.1) (delivered: bn-2br);
- ADR-0029 rule: foreign oracle tooling (TLC, Apalache, solvers) never ships in release binaries (delivered: bn-2br).

**Exit:** the seven RFCs are normative specifications, and the pre-freeze open-debt set (plan §25, validator `check_spec_debt`) reads empty — not only the seven RFC expansions; PR 5 may not merge before PR 0 closes. PR 0 is Phase A's designated opening PR; it has no staffing merge requirement (plan §21.1). (delivered: bn-39d3)

### PR 1 — Revision 3 constitution and epochs [G1]

Implement:

- Rust workspace and crate boundaries (delivered: bn-147t);
- protocol, semantic, intent, evidence, proof, and corpus epochs (delivered: bn-3oehf);
- assurance and inconclusive enums (delivered: bn-2es0);
- `#![forbid(unsafe_code)]` defaults (delivered: bn-17cw);
- deterministic build and artifact paths (delivered: bn-2b8w);
- claim-status lattice (delivered: bn-11se).

**Exit:** an unsupported empty task returns a valid machine result naming every epoch and no misleading success flag (delivered: bn-1w1y).

### PR 2 — Canonical values and CAS primitives [G0 (DX-13), G1]

Implement:

- exact finite values from Revision 2 (delivered: bn-23m);
- canonical encoding and total order (delivered: bn-23m);
- content identity per ADR-0013: in certified lanes, canonical content identity is primary and hash collisions are resolved by canonical comparison; 256-bit-hash identity only in explicitly labeled non-certified modes (delivered: bn-277);
- collision-injection tests (delivered: bn-277);
- atomic artifact publication (delivered: bn-210);
- authorization separate from handle possession (delivered: bn-210).

**Exit:** concurrent publication of identical artifacts yields one identity; artificial hash collisions are detected and resolved by canonical comparison in certified lanes (delivered: bn-1sh6).

### PR 3 — Workspace snapshots [G1]

Implement:

- Merkle workspace snapshots (delivered: bn-15gj);
- disk import, editor overlay, fork, seal, and diff (delivered: bn-1hrk);
- source/dependency/toolchain/config identities (delivered: bn-2xri);
- stale-snapshot error (delivered: bn-3tda);
- deterministic file ordering (delivered: bn-1qhe).

**Exit:** two clients can fork and analyze independently; an old snapshot remains reproducible after the working tree changes (delivered: bn-3tkw).

### PR 4 — Intent Contract v0 [G1, G3]

Implement schema/types for:

- properties; (delivered: bn-u8z2)
- assumptions; (delivered: bn-136f)
- observers; (delivered: bn-7f23)
- bounds; (delivered: bn-1tgp)
- faults; (delivered: bn-111k)
- fairness; (delivered: bn-14w6)
- assurance policy; (delivered: bn-fp0g)
- optimization/non-vacuity; (delivered: bn-1fc0)
- field-level change policy. (delivered: bn-185wn)

RFC 0037's field table is the authoritative enumeration and lists twenty
top-level keys; the bullet list above names the nine that carried their own
bones. The remaining protected groups — scope, trust boundaries, completion
policy, nondeterminism, abstraction maps, security policy — and the header
keys were delivered with the contract assembly (bn-ces7).

**Exit:** Die Hard and replicated-register intents serialize canonically; ordinary operations cannot mutate them (delivered: bn-ces7).

### PR 4a — Lean environment and seed theorems [G6; Phase A band]

Implement:

- pinned Lean toolchain (`leanprover/lean4:v4.32.1`) (delivered: bn-31mq);
- kernel-check the existing seed modules under `lean/Continuum/` (delivered: bn-fak5);
- T0/T1 theorems: transition-system safety, stuttering simulation, finite-closure certificate soundness (delivered: bn-3qsa).

**Exit:** T0/T1 compile with no `sorry` and empty axiom manifests (RFC 0012 theorem ladder; axiom manifests per ADR-0035). The proof *service* remains PR 28 (delivered: bn-zr81).

### PR 5 — Native protocol kernel [G1, G2]

Implement request/response types and local transport for the plan §10.2 operation names:

- workspace.create / fork / diff / seal (delivered: bn-mtw, bn-3gi, bn-3bhkp);
- intent.get / diff / propose_revision / accept / reject / lock (delivered: bn-mtw, bn-3gi, bn-3bhkp);
- verification.start; task.status / cancel / resume / subscribe (delivered: bn-mtw, bn-18z, bn-3bhkp);
- evidence.get / query / verify (delivered: bn-mtw, bn-24i, bn-3bhkp);
- capability negotiation (delivered: bn-mtw, bn-24i, bn-3bhkp);
- typed errors and idempotency keys (delivered: bn-mtw, bn-3gi, bn-24i, bn-18z, bn-3bhkp).

The *type* layer common to all six bullets landed in `crates/continuumd/src/protocol` (bn-mtw): all 72 §10.2 operations with their request/response/verdict/error declarations, the request and result envelopes, the connection handshake and the protocol-major N/N−1 window, and the complete §10.3 error taxonomy — held to `schemas/continuumd-native-protocol.idl` by a test that parses the IDL and fails closed on any disagreement (`rule conformance.registry_agreement`), with RFC 0027's authority table as an independent third source. The *daemon-behaviour* half landed next, one family bone per bullet (bn-3gi, bn-24i, bn-18z), and each declined its own annotation on the same ground: this section's stem says "request/response types **and local transport**", and the transport was genuinely absent, because the IDL fixed the two encodings but neither their canonical field order nor their union tagging, and its own open item 3 left the envelope's `Opaque` payloads without a declared shape — a codec written then would have been invented wire format.

Those three decisions are now taken, in the IDL where a wire decision belongs (`rule encoding.canonical_form`, `rule encoding.union_tagging`, `rule encoding.opaque_payloads`, protocol 3.2), and the transport half is delivered by bn-3bhkp: a connected pair of endpoints exchanging length-prefixed canonical frames, driving `Daemon::dispatch` end to end, with `ServerReject` reachable before any request is negotiated and `ResultEnvelope.payload` carrying the operation's response struct as real bytes. That is what makes each bullet's annotation honest rather than generous — both halves the stem names now exist, and `crates/continuumd/tests/transport_local.rs` exercises them across a byte boundary.

Two things are *not* claimed by these annotations, and the honest reading needs both. `workspace.diff` and `evidence.subscribe` answer `UnsupportedSemanticFeature`, which is what `rule errors.unsupported_surface` requires of an operation whose producing subsystem has not shipped — a served surface, not a working one. And `canonical_cbor` has no implementation: the IDL fixes two encodings, this transport speaks one, and the CBOR half of every golden vector `rule conformance.golden_traces` requires is owed.

**Exit:** replaying an idempotent request returns the same task/artifact identity (delivered: bn-3bhkp — asserted through the transport for both identity kinds, with byte-identical result frames; `replaying_an_idempotent_request_through_the_transport_returns_a_byte_identical_frame`). May not merge before PR 0 closes.

### PR 6 — Cancel-correct task service [G0 (DX-14), G1]

Use asupersync regions for daemon work:

- task lifecycle (delivered: bn-2gk, bn-12t — the calculus in `crates/continuum-task/src/region/` and the rewiring in `crates/continuumd/src/daemon/region.rs` that puts `continuumd`'s task work inside it. Every unit of task work — the run `verification.start` and `task.resume` drive, and the teardown `task.cancel` drives — is a child region of the daemon's root scope, opened and finalized within the dispatch that runs it. `region::scoped` is the only way to open one: `open` and `settle` are private to that module and `scoped` calls the second on every path out of the first, including the fault path and the path where the work never moved its worker at all, so "no region outlives its dispatch" is a property of what compiles rather than of care taken. A `TaskEntry` carries the `RegionId` and `WorkerId` of the run that produced it, and a run's daemon-side writes are the region's worker steps in the order they happened: `Begin` when it starts, `Reserve` when a campaign comes back, `Commit` when it is written where `verification.result` can read it, then `Complete` for a closed exploration or `Suspend` for a bound that tripped. That order is *checked* rather than intended — the region layer refuses a park that precedes its commit, because cancellation may not truncate a publication in progress, and `TaskRegions::defects` records any step it refused, which `TaskRegions::is_total` then reads. `task.cancel` drives cancel → drain → finalize over the task's residual work and reads its nullable `continuation` off the resulting `CancelOutcome`, so the three arms of `rule task.cancel_correct` are decided by the layer that owns the rule: this daemon states two facts — how much the task published, and whether it holds a continuation — and cannot spell "committed evidence and no continuation" at all, because that arm has no constructor. What the bullet does **not** claim: the families answering synchronous structural questions — `workspace`, `intent`, `evidence`, `observe` — run no worker, own no obligation, and have nothing to drain, and are deliberately not wrapped in regions; "daemon work" is read as the work PR 6 is about. Evidence is `crates/continuumd/tests/daemon_task_regions.rs`, ten tests through `Daemon::dispatch` and nothing else);
- committed partial evidence (delivered: bn-23j7s — `crates/continuumd/src/daemon/budget.rs` and the rewiring it enabled. A `TaskEntry` now holds a `BudgetLedger` instead of a `budget: Budget` and the `bounds: Bounds` derived from it, and the three wire answers are *projections* of that one ledger: `TaskRecord.budget` is its ceilings, `TaskRecord.cost` is its recorded spend, and the engine's `Bounds` is its `states` ceiling. `verification::unenforced` — bn-18z's eight hand-written `budget.<token>` pairs — is **deleted** in favour of `BudgetLedger::omissions` over `MeterSet::STATES_ONLY`, so the manifest is a consequence of what this daemon can measure and a deployment that grows a clock loses an omission by derivation rather than by remembering to delete a line; the subjects, the reason token and their order are unchanged. Spend is charged as the *difference* against what is recorded, because `bfs::explore` has no partial-state entry point and a resumed walk re-explores the parked prefix: a Die Hard campaign that parks at 3 states and closes at 16 cost 16, not 19. `task.update_budget` reads the five-arm legality table per dimension, including the **`Suspend`** arm bn-1gc recorded as uncomputable from the handler — and the bug that arm was hiding is concrete: a lowered ceiling used to become the engine bound directly, so the next `task.resume` came back with a *smaller* campaign and a monotone `cost.states` went down, which is the silent truncation INV-009 prohibits. Now the withdrawn ceiling is recorded (`TaskRecord.budget` reports the budget in force) and is *not* a bound: `bounds_of` floors the engine's state bound at recorded spend, the task stays `Suspended` with its committed partial evidence and its continuation, and a `task.budget_suspended` milestone says so. The fifth arm gained a wire surface too — an operation that accepts a budget owes INV-007's manifest for it, so `task.update_budget` and `task.resume` now name a declared ceiling nothing meters instead of accepting it in silence. **Committed partial evidence gains artifact identity**: a publication is named by the content identity of the campaign that produced it — task, commit ordinal, snapshot, state count, closure, frontier — and `Publications` is a linear typestate in which a staged publication is unobservable, so "uncommitted partials are absent, never half-visible" is a property of what compiles rather than of care taken on the paths out of a run. The names reach a caller as `ResultEnvelope.artifacts` entries, `kind: "task"` with the task handle and the publication's own `commitment`, monotone across a resume: a re-derived artifact receives a new identity and the earlier one is still there. And SD-13 is discharged: `Error.continuation` and `Error.non_resumable_reason` were hard-coded absent in `result::failure`, so every `BudgetExhausted` this daemon raised was the silent dead end that clause forbids; a `Fault` now carries a typed `Resumption` and the two constructors that produce the code cannot produce it without one of the two answers. Evidence is `crates/continuumd/tests/pr6_impl02_budget_evidence.rs`, ten tests through `Daemon::dispatch`. What the bullet does **not** claim: **durability**. Nothing in this workspace writes a task table to disk, and the persistence machinery is bn-3dr's (daemon crash recovery); what this bone owed and delivered is the identity binding and the wire surface, so the honest half of "committed partial evidence survives daemon restart" — that two daemons which ran one campaign name its publications identically, and a name is therefore something a store could be asked to return — is asserted by name, and the storing half is not);
- budget accounting (delivered: bn-1gc — `crates/continuum-task/src/budget/`: a typed ledger over the nine SD-12 budget/cost dimensions, spelled once and held to three independently maintained sources by `tests/budget_dimensions.rs` — the IDL's `struct Budget` in declaration order, RFC 0026's own prose enumeration, and `verification-task.schema.json`'s `additionalProperties: false` property set — with a mutation test that requires each comparison to fail when its source is renamed, dropped from, or widened by RFC 0026's F16 tenth dimension. **Declared and enforced are separated**, which is the INV-007 discipline bn-18z wrote by hand: a `MeterSet` says what the accounting can measure, and the 2×2 of declared × metered is total — a declared ceiling with a meter is enforced, a declared ceiling without one is a typed `DimensionOmission` carrying the same `budget.<token>` subject `continuumd`'s `verification::unenforced` already emits, a charge against an unmetered dimension is refused outright rather than inventing a measurement RFC 0026 requires to be absent, and an undeclared dimension owes nothing. `MeterSet::STATES_ONLY` is the daemon's single enforcement path as a value, and reproduces its eight omissions by derivation rather than by a hand-kept array. Spend is monotone and only *headroom* is refundable — `charge` records what happened and nothing un-records it, while `reserve`/`settle`/`release` hold and hand back budget for a publication in flight, so a settle can never exhaust and a discarded artifact refunds its bytes without refunding the compute that produced it. Exhaustion is a **result**, not an error, carrying the dimension, ceiling, spend and request — the shape `bfs`'s `Exploration::Exhausted` already has — and it names an `ErrorCode` token, never a verdict. `rule task.update_budget` is a five-arm legality table as a value (`raised`, `unchanged`, `tightened`, `suspend`, `unenforced`), walked as data by `tests/budget_update.rs`: **lowering is admitted**, because the rule defines it rather than forbidding it, and lowering below committed spend parks with committed partial evidence plus a continuation (B18) instead of truncating the campaign. Two things the annotation does not carry: this crate ships **no meters** — nothing in it reads a clock, counts a byte or asks an engine anything, and `MeterSet` is the declaration a real service makes about itself — and `continuumd` is **not rewired** onto the ledger, which was outside bn-1gc's fence; the seam is designed and written down in `crates/continuum-task/src/budget.rs` (`TaskEntry` holds a `BudgetLedger`, `bounds_of` becomes a projection of its `states` ceiling, `verification::unenforced` is deleted in favour of `BudgetLedger::omissions`, and `task.update_budget` reads `UpdateOutcome` — including the `Suspend` arm the handler currently has no committed-spend figure to compute));
- suspension/continuation (delivered: bn-12t — a campaign that trips its state bound parks a `cont_*` continuation pinning the snapshot, the intent, the queue-ordered frontier, the five compatibility epochs and engine identity, with no constructor that can omit one, and with the protocol epoch *structurally* absent from `PinnedEpochs` rather than skipped by a rule somebody has to remember. `task.resume` validates RFC 0026's decision table before any reuse — the envelope's snapshot against the pinned one, that snapshot's sealing and lineage currency, then both predicates, P1 over the compatibility epochs and P2 as equality on engine identity — and a refused resume opens no region at all, because validation precedes work. That wire behaviour landed with the task family in bn-18z; what this bone adds is the answer to the question the rewiring forced, and the evidence for it: **a parked continuation is not a live worker.** `RegionTree::finalize` requires terminal, not merely quiescent, so a `Suspended` worker cannot survive a teardown — and RFC 0026 says four times over that a continuation is a durable artifact rather than a scope: handles "remain valid across daemon upgrades within a protocol major"; resume re-validates snapshot and epochs before reuse, which a scope that was never left would not need; continuations are "forked across an epoch advance, never migrated in place", the original remaining valid for the epochs it pinned; and a continuation "MUST leave `protocol` Unpinned" because that epoch is connection-scoped, so a continuation outlives the connection that minted it. The region-layer `Suspended` is therefore transient inside one dispatch: the campaign parks, the scope ends, the parked *execution* is terminated by the scope exit reporting committed partial evidence plus a continuation, and `task.resume` mints a **new** worker in a **new** region — two different `RegionId`s on one task, which is the resolution as a readable fact. Keeping the region open across dispatches so the parked worker stayed alive was rejected on the same evidence: a task parked and never resumed would hold an open region for the daemon's lifetime, which is precisely the leak G0-DX-14 asks about. Evidence: a still-parked task whose region is finalized and whose worker is terminal, a resume whose region and worker ordinals are strictly later than the parked run's, a refused resume that opens no scope, and a valid resume that is byte-identical across two fresh daemons — R3 spike §3's "resumable continuation", at the daemon API);
- drain/finalize behavior (delivered: bn-2gk — `crates/continuum-task/src/region/`: a region tree owning workers and child regions, with the one-way lifecycle `Open → Draining(Closed | Cancelled) → Finalized` and RFC 0026's request → drain → finalize as its three operations. `cancel` is the request: it reaches every non-finalized region in the subtree, upgrades a descendant that was merely closed, and opens RFC 0001's own named `cancellation-finalization` obligation, which stays visible in the ledger until the finalize discharges it. `drain` is total under cancellation — discard the staged publication, compute the typed outcome, transition exactly once, discharge the termination obligation, per worker in identity order, which is docs/35's six-step cancellation list in that order — and *blocking* under a normal close, where a non-terminal worker is a typed `DrainBlocked` naming it, because "wait for owned work" in a calculus with no waiting is a refusal that tells the schedule what it still has to step. `finalize` walks the subtree children-first and requires every owned worker to be **terminal**, not merely quiescent: a parked worker is not running, but a finalized region that still owns a resumable one is a continuation pointing into a scope that no longer exists, and the stronger gate is what makes the post-condition one sentence with no carve-out);
- no orphan workers (delivered: bn-2gk — the property is enforced by construction and instrumented three ways. The only constructor of a worker refuses any region that is not `Open`, so the set a teardown must account for stops growing at the request; the only writer of a worker's state is one private transition function, which discharges its `worker-termination` obligation on and only on the transition into a terminal state; `finalize` refuses while any owned worker is non-terminal. `Finalization::is_total` is the conjunction of three independently computed facts — the orphan set read off worker states, the unresolved-publication set read off evidence ledgers, and the obligation ledger's balance read off both an outstanding *set* and opened/discharged *counters* — so a bug that fooled one accounting still has two to get past. `crates/continuum-task/tests/region_no_orphan.rs` is the instrumented half: 210 interleavings of three worker programs across two regions, each swept with a cancellation *request* at all eight positions and again with a cancellation that *completes* at all eight, 3,570 teardowns in total, every one asserting the post-condition against the tree itself rather than only against the report it returned. The gate is shown to be load-bearing rather than vacuous by `finalize_refuses_when_the_drain_is_skipped`, and the sweep is shown to exercise more than one behaviour by collecting the distinct canonical renderings and requiring more than one).

One thing these annotations deliberately do **not** claim, and two bullets that changed hands between bones; the honest reading needs all three.

This is not an asupersync integration and does not pretend to be one. The crate is on crates.io (`asupersync 0.3.10`, `LicenseRef-MIT-OpenAI-Anthropic-Rider`) and could have been declared, but the dossier splits the work and this bone is on the near side of the split: plan §21 makes "task/continuation lifecycle" a **Phase A** deliverable and "asupersync semantic adapter (one adapter crate …)" a **Phase B** one landing in `continuum-asupersync` (PR 14), plan §20 assigns `continuum-task` the exact words these two bullets use — "drain and finalize behavior, and the guarantee of no orphan workers" — and ADR-0001 requires the separation to be possible at all ("Continuum remains capable of model-only execution without asupersync. The normative abstract semantics are owned by Continuum, so the runtime cannot silently redefine model behavior"). What landed is those normative semantics as a deterministic single-threaded calculus with no thread, clock, executor or ambient anything (INV-005), written so the substrate slots behind the same seam later and is then held to these properties. Declaring the dependency now would have added a full async runtime to a workspace whose one external edge is `blake3`, and would have bought a Phase B deliverable with a Phase A bone.

`committed partial evidence` is annotated now and was not when bn-1gc landed, and the difference is a *store* rather than an accounting. bn-1gc landed the accounting: a `Checkpoint` pairs the region layer's monotone commitment count with the spend recorded when it happened, and `CommittedPartialEvidence` reads four numbers off that pair — what was committed, what that cost, what the campaign cost in total, and the difference, which is real spend against no artifact and is the number neither layer can produce alone. `EvidenceBook::reconcile` then adds a **third independently computed accounting** beside `Finalization::is_total`'s two: every commitment the region layer counted has a price, every price names a commitment, and no budget account still holds headroom for a publication in flight — the budget-side reading of "committed or absent, never dangling", computed from headroom rather than from the region tree, so agreement between the two is evidence and not a tautology. `crates/continuum-task/tests/budget_partial_evidence.rs` sweeps it over 210 interleavings of three worker programs with a cancellation that *completes* at all eight positions, 1,680 runs, each asserting reconciliation, the committed-count agreement between the two layers, and that the continuation the region layer minted and the checkpoint the budget layer took name one frontier. What bn-1gc could not do was **name** any of it: both ledgers count publications rather than identifying them, and `crates/continuum-task/src/lib.rs` records that the crate declares no `continuum-workspace` edge and that plan §20 states none, so there was no artifact identity to persist and no restart to survive. bn-23j7s is that join, on `continuumd`'s side of the plan §20 edge, and it is careful about which half of the criterion it claims: the identity binding and the wire surface are delivered and asserted — a publication is named by the content identity of the campaign that produced it, two daemons agree on that name, a staged publication is unconstructably invisible, and the names ride `ResultEnvelope.artifacts` — while **durability is bn-3dr's** (daemon crash recovery) and is not claimed here. A restart test in this bone would have been a test of the identity function wearing a durability test's clothes, so it is written as what it is.

`task lifecycle` is annotated now and was not when bn-2gk landed, and the difference is the stem: "use asupersync regions for **daemon work**". bn-2gk built the lifecycle and declined the bullet because no daemon work ran in a region — `continuumd`'s `TaskTable` was still the sole owner of a task's status, and the join was a designed seam rather than a done change. bn-12t is that change, and the seam was designed accurately: a `TaskEntry` gained a `RegionId`, and `task.cancel` calls cancel → drain → finalize and reads its nullable `continuation` off the resulting `CancelOutcome`. What is still *not* an asupersync integration is everything above: the regions are the deterministic calculus, and the substrate slots behind the same seam in Phase B. And the scope of the annotation is task work — the four families that answer synchronous structural questions own no worker and are not wrapped, which the bullet says outright rather than leaving to be inferred from a codebase read.

Nothing in the six bullets above was written after the two adversarial campaigns that attacked them, and the block would otherwise read as though nothing had happened since. Three daemon-layer defects were found against this PR's surface, all three real, all three fixed with the campaign's own reproduction flipped into a regression guard that runs un-ignored in the default suite. bn-3p32's 18-attack G0-DX-14 campaign held both named conjuncts under every attack but falsified **record closure**: `task.resume` applied a request budget before it tested terminality, so `cancel` → `resume(continuation, budget)` rewrote a `Cancelled` task's ceilings and could append a `task.budget_suspended` milestone — and its `TaskEvent` — to a closed record. bn-1kp6's 22-attack G0-DX-03 campaign found that same defect independently, beside two more: `workspace.create` named a lineage by the content identity of the snapshot it created and inserted it unconditionally, so an identical create under a fresh idempotency key **rewound the lineage** and could revoke a live continuation; and `daemon::state::ReplayKey` omitted `RequestEnvelope.budget`, which is in the `task_*` preimage, so one key answered two different canonical requests with one campaign instead of `IdempotencyKeyReused`. The repairs are bn-10093 — the terminal check moved above the ledger write, so a resume's budget arm produces `task.update_budget`'s own observable for a terminal task — bn-n1xou — an identical create converges, put-if-absent on the lineage, `sealed` monotone so no create un-seals — and bn-h1zqz — the whole canonical request in the key, with the field-by-field audit of what stays out recorded at the type. Both G0 rows carry complete falsification-and-fix cycles as a result. One sentence above is narrowed by the first repair, and the narrowing is the repair's point rather than a gap: `task.resume` names a declared ceiling nothing meters when it *accepts* one, and a resume of an already-terminal task accepts nothing, records nothing and therefore names nothing, because a terminal task's budget is a historical fact and rewriting it would make its recorded cost unreadable. The other five bullets read true unchanged.

**Exit:** cancellation at every instrumented phase leaves either a valid continuation or no published partial artifact (delivered: bn-1n6r — `crates/continuumd/tests/pr6_exit_evidence.rs`, five tests through `Daemon::dispatch` and nothing else, holding the sentence clause by clause over the **wire-reachable** instrumented phase set: the four G0-DX-14 lanes' ten dispatch phases, the dispatch that faulted, and phase zero, where the cancellation arrives before the task exists and denial precedes semantic work. The disjunction is a *value* rather than a reading. `exit_side` maps the answered status, the wire's nullable `continuation` and `ResultEnvelope.artifacts` onto one of three dispositions — a valid continuation, nothing published, or published-and-complete — and its fourth branch is `None`: artifacts named, no continuation to resume them from, and a campaign that did not close, which is the shape the sentence forbids and the shape `CancelOutcome` has no constructor for. The anti-vacuity test feeds that branch a **real** artifact list taken off a real cancellation, so a green run of this file is a claim about the daemon rather than about a function that cannot say no, and it requires the eleven task-bearing phases to render more than one behaviour class, reaching all three arms and both sides of "did it publish anything". **"A valid continuation" is proved by use**: the `cont_*` a cancel handed back is spent on a second fresh daemon, which minted the byte-identical handle from the same campaign, and it closes that campaign on Die Hard's frozen 16 states, adding a publication rather than replacing one; a third fresh daemon answers the identical resume byte for byte. **"No published partial artifact" is proved on both of its dispositions** — nothing published at all, reached by the synthesis lane and by the faulted dispatch, with SD-13's typed `non_resumable_reason` on the `BudgetExhausted` failure; and published-and-complete, reached by a closed campaign whose artifact is a whole answer and whose arm carries RFC 0026's typed reason rather than degrading to silence — with what was published counted three independent ways on every phase: `ResultEnvelope.artifacts`, `TaskEntry::publications`, and the cancel scope's own `EvidenceLedger::committed`. The arm with no constructor is asserted at the wire on every phase: a continuation is never answered over a task that published nothing, and committed evidence without a continuation is never silent. Post-cancel **closure** — the property bn-3p32's A1 and bn-1kp6's attack 18 falsified independently and bn-10093 repaired — is re-asserted as the exit sentence's own stability: the disjunct a cancellation landed on is recomputed after a resume carrying a budget below recorded spend, and must not move. The exhaustive half is cited rather than re-run — bn-2zy's 5,472-run calculus matrix and its daemon-grain twin, bn-2gk's 3,570 teardowns, bn-1gc's 1,680 reconciliations, bn-3p32's 18 attacks, bn-1kp6's 27 tests — because the exit file's job is to be readable, and one sample per equivalence class is what makes it so. Three limits stated rather than papered over: **durability is bn-3dr's**; a wire-grain **mid-publication** cancellation cannot arise while `Daemon::dispatch` holds `&mut DaemonState` for a whole call, so it is proved at the calculus grain and opens at the wire with the asupersync adapter in Phase B, PR 14; and every daemon here is one thread, one request at a time).

### PR 7 — Evidence Graph v0 [G1]

Implement node/edge/status types, immutable versions, and queries:

- support/refute/depend/refine/explain/repair/check edges (delivered: bn-3cyf — `crates/continuum-evidence/src/edge.rs`, with `src/{node,identity,graph}.rs`: the closed vocabulary is plan §11.3's **thirteen** in the edge schema's own tokens rather than the bullet's seven, and `PR7_BULLET_WORDS` carries the correspondence as data — PR 7's own conflict and status bullets need two of the other six, and an enum missing six members of a closed schema enum cannot round-trip a conforming instance. `CHECKED_BY` names its checker as a *type* rather than as a validation: `EdgeRelation` is twelve unit variants plus `CheckedBy(CheckerBinding)`, the binding carries a `ServiceIdentity` that refuses every actor scheme but `service:`, and there is no `Option` anywhere on the path — two `compile_fail` doc tests, which compile as an external crate, hold both halves, so the schema's one `if`/`then` and SD-11's `evidence-graph-edge` requires `checker` on `CHECKED_BY` are facts about what compiles. Edges are content-identified over what they assert — relation, both typed endpoints, the evidence named, the checker, and the producer — because two agents asserting one relation are two pieces of evidence: collapsing them either loses the credit docs/44 keeps or resolves by the write order plan §11.7 forbids. Endpoints are `NodeRef`s carrying the node's kind, which is what makes any endpoint rule *expressible*; RFC 0038 types none of them, so none is imposed. The graph is append-only put-if-absent keyed by content identity with no removal and no mutable accessor, refuses an edge whose endpoint it does not hold — docs/44 'references must resolve' — refuses a self-edge, and reports contradictions as pairs without ever choosing between them. **Library-level; the wire surface is bn-3sypm's, and RFC 0038's OPEN F14 is left visibly open rather than answered**: the `CHECKED_BY` to-handle's node kind is `CheckTargetRule`, defaulting to `Undecided` and admitting every kind, with each of bn-3sypm's three candidates expressible as `one_of` and asserted to refuse the other two; `edge_id` derivation is the `EvidenceNaming` seam, because ADR-0013 makes the exact identity the canonical preimage and leaves the preimage-to-`ev_` function an index; and edge-creation authority is not modelled at all, since RFC 0038's Authority section does not cover it. Nothing was added to the IDL, the RFC 0027 registry, or `continuumd`, so no operation appends an edge over the wire and `evidence.query` still truthfully answers no edges);
- provenance (delivered: bn-21iq — `crates/continuum-evidence/src/{provenance,actor,identity}.rs`: RFC 0038 has no prose provenance section and delegates — 'This RFC summarizes; the schemas decide' — so the record is transcribed from the object both `evidence-graph-node.schema.json` and `evidence-graph-edge.schema.json` carry: `actor`, `created_at`, `inputs` required and `tool` optional, with `additionalProperties: false`. Three of the four are plain fields of `Provenance`, so a provenance-free node or edge is not something the graph refuses at run time — it is something a caller cannot spell, and `EvidenceNode::propose` and `EvidenceEdge::new` take one by value with no `Option` and no default. `inputs` is a set rather than a list, because one derivation described two ways would have two content identities. Actors are the wire's four schemes with a `ServiceIdentity` narrowing, so 'authority is never a function of who an actor is' and 'promotion is service-restricted' can both hold. **Epochs are carried and never rendered**: plan §4.6 makes evidence epoch-scoped and neither schema has a member for it, so `Provenance` holds an `EpochSet` that `to_record` omits and `to_preimage` includes — the wire record stays exactly the schema's four members and the library record stays able to say what §4.6 requires; which envelope epochs ride in on the wire is RFC 0026's question and is not answered here. Time is an argument, never a reading: nothing in the crate touches a clock, and `Timestamp` transcribes the daemon scalar's one spelling. Identity is ADR-0013's — `EvidenceIdentity` is the canonical encoding and its equality and ordering consult no hasher, while the `ev_` handle the schemas carry is a *name* minted through a labeled non-certified `HashIdentity`, so a handle collision is detectable and never a silent merge);
- trusted status transitions (delivered: bn-2c8l — `crates/continuum-evidence/src/authority.rs` and `src/graph.rs`: `Promotion` has private fields and no public constructor, and the only ways to obtain one are `TrustedService::{promote_to, validate, inconclusive}`, each needing a `&TrustedService`, which needs a `ServiceIdentity`, which refuses every actor scheme but `service:` — so the chain from an `agent:` actor to a promoted status has no first step, and `EvidenceGraph::promote`, the only function in the crate that writes a status, takes a `&Promotion` and nothing else. That is the daemon's `daemon::evidence`/`daemon::observe` device mirrored at the library level with no dependency on `continuumd`, which plan §20 puts at this crate's own tier. Three `compile_fail` doc tests hold the boundary as a client sees it: a `Promotion` cannot be built by hand, `StatusWrite::promoted` is unreachable from outside the crate, and a `TrustedService` cannot be declared over an `ActorId`. docs/44's authority table is `ServiceRole::may_promote_to`, one cell per row, with the two cells that are a *reading* rather than a quotation — `refuted`'s 'valid counterexample/checker' and `inconclusive`'s 'producing service' — marked as such so an auditor knows which two to argue with; `proposed` is authorized for no role at all, because creation is not a promotion. The two payloads plan §11.4 attaches are structural: `validate` is the only path to `validated` and takes a `ValidationBasis`, `inconclusive` is the only path to `inconclusive` and takes a typed INV-008 reason, and `promote_to` refuses both by name; `service_identity` is written only for the four assurance-bearing statuses the node schema requires it on. `promote` keeps three guards in order — the node is held, INV-004's producer-is-not-the-checker check before the lattice as `evidence.verify` orders it, then `claim_status::compare_and_set`, which is this crate's own decision rather than a second copy — so authority and the lattice stay independent and a proof service authorized for `proved` is still refused over a `refuted` claim. Node-creation authority is `CreationCapability`: docs/44's 'actors may append only allowed node types', plus the check the daemon gets from the wire for free, that a node's `provenance.actor` is the capability's own actor. **Library-level**: the daemon's own promotion path is unchanged and no operation was added; the PR-7 exit sentence is held here as a property of the crate's public API, asserted from an integration test that links it as a separate crate);
- conflict nodes (delivered: bn-2acj — `crates/continuum-evidence/src/conflict.rs` and `src/graph.rs`: docs/44's conflict block has four lines and three of them are structure — the parties, the ground on which they cannot both hold, and the experiment or proof that would distinguish them — so `Conflict` carries all three and all three are mandatory; fewer than two *distinct* parties, an empty ground, or an empty required experiment are refusals, because a conflict nobody can act on is how an ambiguity stops being visible. Nothing ranks the parties: there is no confidence field, no join over their statuses, and no automatic resolution, which is docs/44's 'a coordinator cannot resolve conflict by selecting the most confident agent answer' as an API shape rather than a rule. `Resolution` is refused unless it names evidence and unless the node recording it is a `decision`. Resolution is a *transition*: `EvidenceGraph::resolve_conflict` is docs/44's one retirement row — any to superseded, by policy/owner with explicit edge — in two steps, appending the explicit `SUPERSEDES` edge from the decision node to the conflict node and then promoting the conflict through the ordinary `promote` path, so INV-004 and the compare-and-set both still apply and only a service holding `ServiceRole::PolicyOwner` can mint the promotion. Nothing is removed, and the test asserts it by counting nodes and edges before and after, re-reading every held identity, and pinning the surviving `CONFLICTS_WITH` link and the conflict node's own two-entry history. A `ConflictSubject` is a node **or** an edge, because a contradiction is often between two assertions — a `SUPPORTS`/`REFUTES` pair over one claim is `EvidenceGraph::contradictions`' whole output — and materialization draws what the graph's shape allows and names what it cannot: `CONFLICTS_WITH` once per unordered pair of node subjects in canonical identity order, `DERIVED_FROM` from the conflict node to each node subject, and every subject's handle, edges included, in the conflict node's `provenance.inputs`, which is the schema-conforming place to record what a conflict is about when no edge can point at it. **Library-level**: the contradiction table is deliberately two rows read off docs/44 rather than a taxonomy this crate invented, because a wrong contradiction would manufacture conflict nodes nobody can resolve).

**Exit:** an untrusted client cannot promote a proposal to validated/proved.

### PR 8 — Exact finite model service [G1, G2]

Wrap Revision 2 reference semantics behind native protocol:

- programmatic transition model (delivered: bn-2d0e — `crates/continuum-engine-reference/src/{ident,domain,expr,model,diehard}.rs`: named variables over inclusive `i64` domains, named actions as guarded deterministic-or-enumerated simultaneous updates, an explicit initial-state enumeration, and named predicates, all as inert data rather than closures so INV-005 is a property of the types; state vectors, action indices, initial states and successor rows in the certificate wire form's canonical byte order, and the TV-009 Die Hard port asserted against the frozen 16-state/96-transition/depth-6 facts);
- deterministic BFS (delivered: bn-3p8u — `crates/continuum-engine-reference/src/bfs.rs`: an explicit FIFO queue of `(state, depth)` over a `BTreeMap` visited set, so the reachable set comes out of the walk already in the certificate wire form's ascending state-table order with a parallel depth column and no sorting pass; a state is expanded atomically and new targets are admitted in ascending state order, which makes the explored set a prefix of the canonical walk and the frontier a resume point. Three bounds — states, depth and transitions, the first and third taken from the state and transition ceilings `continuum-kernel-core`'s wire form declares, the second the engine's own because no certificate carries a depth — are declared per call with no default, and reaching one returns a typed partial result carrying the explored set, the frontier in queue order and which bound tripped, never a silent truncation (INV-008/INV-009, and the engine-grain shape of RFC 0026's budget-exhausted-carries-a-continuation rule); only the `Complete` arm exposes a closed set, so a finite-closure certificate cannot be built from a truncated one. Die Hard's frozen 16 states / 96 transitions / depth-6 witness are re-derived through the engine and asserted equal, state for state and depth for depth, to the independent hand walk the Die Hard evidence suite already carried);
- invariant/deadlock checking (delivered: bn-3e0m — `crates/continuum-engine-reference/src/checking.rs`: which of a model's declared predicates are invariants, and whether a state with no enabled action is a defect, are the caller's declaration rather than the model's, so one model can be checked under several policies without being re-declared. The deadlock half is the engine-grain projection of the intent contract's completion policy, whose closed four-member vocabulary no engine may quietly reinterpret; the arm that vocabulary's environment-closed member would map to is deliberately absent, because this layer declares no environment and inventing one would be a guess. Each invariant gets a typed outcome from the assurance result's own three-way verdict — established, refuted, inconclusive with a reason — never a bare boolean, and the reasons are spelled exactly as INV-008's closed reason vocabulary spells the two of its six members a finite engine can produce. The asymmetry the exploration bone left here is encoded: a violation found inside a bounded run is a real refutation, because the state was genuinely reached and no larger budget can un-reach it, while *not* finding one is inconclusive naming the tripped bound, the explored count and the frontier — nothing in the module can report an invariant as holding from a truncated run, at any bound. A refutation carries a shortest path when the exploration closed and a typed absence when it did not, because the witness extractor answers only for a closed exploration and this bone does not work around its sibling's refusal. Terminality is recomputed per discovered state rather than read off the walk, so a deadlock sitting unexpanded in a bounded run's frontier is still reported. Die Hard checks as the corpus declares it: type-correctness established over the closed sixteen, the intentionally-false invariant refuted at four gallons and three by the film's six-step solution, and no deadlock under the strictest policy);
- shortest witness (delivered: bn-2pmc — `crates/continuum-engine-reference/src/witness.rs`, on a behaviour-preserving extension of `src/bfs.rs`: every discovered state now records the labelled transition that first reached it, written at the moment its depth is written and under the same canonical admission order, so the chain of those records is itself a shortest path and no second search is needed. Recording it changed nothing else — the newly admitted targets remain a set keyed by state, iterated in ascending state order, of the same size, so the explored states, their depths, the queue-ordered frontier, the counters and the bound checks are the values they were before, asserted against an independent hand walk and against the frozen frontier of a bounded Die Hard run. A target is data — one state, or one of the model's own declared predicates holding or failing — never a caller-supplied closure, because a witness's endpoint has to be something the model can state; the endpoint chosen is the shallowest satisfying state with ties going to the canonically least, so a witness is a function of the model and the target alone. Extraction answers only for a closed exploration, mirroring the closure seam: a path out of a bounded run is not known to be shortest and is refused by name, even when the endpoint was in fact discovered at its true depth (INV-008, INV-009). Die Hard's shortest violation of NotSolved comes back as the film's six-step solution ending at four gallons and three, pinned equal to the path the Lean port proves and replayed one transition at a time through the model layer);
- finite closure certificate (delivered: bn-3e4l — `crates/continuum-engine-reference/src/certificate.rs`: a closed exploration written out as CONTCERT wire bytes, from the grammar documented in `continuum-kernel-core`'s wire module rather than from its types, so producer and checker share no codec — docs/03 §8 lists certificate decoding among the axes independent paths must differ along — and the engine keeps no library dependency on the kernel at all. Emission takes a closed-set witness whose only constructor is the exploration's own closed-set accessor, so a certificate cannot be built from a bounded walk even by mistake; the eight claim-envelope fields of RFC 0005 are the caller's, shape-checked against the kernel's token grammar before a byte is written, because six of them are digests of artifacts this crate cannot see and one is the wire epoch the format fixes — the same trusted-input seam the kernel's own receipt uses, where nothing is invented. Emission is pure: one model, one closed set, one envelope, byte-identical certificate. The fourth frozen TV-009 fact is now asserted rather than deferred — Die Hard is explored through the engine, emitted, and handed to the kernel's checker as bytes, which verifies 16 reachable states, 96 labelled transitions and one initial state under the state-domain property class; every byte of the canonical state table, mutated two ways, is refused, as are a wrong magic, a foreign wire epoch, a foreign certificate family, a trailing byte and every prefix).

**Exit:** Die Hard returns 16 states and depth-6 solution through the daemon API (delivered: bn-1jg0).

### PR 9 — Trusted kernel crates [G1, G6]

Implement the trusted checking base as four crates — `continuum-kernel-core`, `continuum-kernel-sat`, `continuum-kernel-smt`, `continuum-kernel-temporal` (plan §20):

- finite closure/type certificate checking (delivered: bn-2i8);
- no shared evaluator code with any engine; no async, no unsafe, no plugins or dynamic loading (delivered: bn-2i8 — `continuum-kernel-core` depends on no workspace crate and no external crate, so its decoder, arithmetic and state representation are its own per docs/03 §8);
- <15,000 non-test-line covenant across the four crates (docs/03) (delivered: bn-2he — `tools/check_kernel_covenant.py` KCOV-01, wired into `just check` via the `covenant` recipe; the four crates count 10,667 non-test lines against the 15,000 budget, and the counting method — which files, how `#[cfg(test)]` items and test-only modules are excluded, what counts as a line — is documented in the checker's module docstring and recorded in `tools/kernel-covenant/evidence/kernel-covenant.json`; the same checker enforces the no-async/no-unsafe/no-panic lint walls, dependency freedom and the no-plugins rule);
- serialization boundary: certificates are checked from wire form, never from shared memory (delivered: bn-2i8 — the only checking entry point takes `&[u8]`, and `wire::Certificate` has no constructor outside `wire::decode`);
- receipt generation with checker epoch, build digest, and input hashes (delivered: bn-2he — a `receipt` module in each of the four crates emitting `schemas/proof-receipt.schema.json` instances, with the checker's wire contract carried in its *qualified* spelling (`continuum-kernel-core/CONTCERT/1`) because four crates share the ordinal 1, and carried in `checker.version` rather than `epochs` — RFC 0026 correction 17: there is no checker epoch. The build digest, toolchain identity, input certificate digest, observer digest, proof epoch, receipt id and reproduction command are an explicit `Seam` the caller supplies and the receipt names as trusted inputs; the kernel checks their shape and never invents a digest. KCOV-09 validates each crate's golden receipt against the schema);
- mutation tests (delivered: bn-2he — `tools/kernel-covenant/mutation-matrix.toml` is a 23-class × 4-crate matrix mapping every mutation class to the tests that own it, enforced fail-closed by KCOV-08: a named test that does not exist, or a cell with neither an owning test nor an explicit waiver, fails the gate. 90 of 92 cells are owned or permanently not-applicable; the two `uncovered` cells — `envelope.digest-relabelling` in `continuum-kernel-smt` and `continuum-kernel-temporal` — are recorded as gaps and reported by `--strict`).

**Exit:** every single-field certificate mutation in the test suite is rejected, checking wire-form input only (delivered: bn-19bc).

### PR 10 — Agent client and ACI benchmark harness [G0 (DX-10), G2]

Implement a minimal client exposing only typed operations. Compare against a shell-scraping baseline on Die Hard and Dining Philosophers tasks.

Measure:

- valid operation rate (delivered: bn-134i — the instrument, and both of the surfaces it compares. `crates/continuum-mcp/src/{lib,link,answer,register,client}.rs` is the minimal typed client: eight named operations over `continuumd`'s real `LocalPair` wire, every argument a typed struct, every answer a typed `Payload`, every refusal a typed `ErrorCode` with `retryable` and the protocol's own `recovery` list, the INV-007 omission manifest carried inline, and research/25's semantic action grammar shipped as data in `register::OPERATIONS` — eight rows, each an operation and the handle it requires, with preconditions stated as *necessary* conditions so the grammar predicts the daemon rather than restricting the caller, which `the_register_never_refuses_an_operation_the_daemon_then_admits` holds it to. It owns no semantic state: the grammar is evaluated against an `AgentContext` the **caller** passes in per call, which is plan §20's "adapters do not own semantic state" as a property of the struct's fields. `crates/continuum-benchmark/src/shell.rs` is the disciplined baseline — PR 13's CLI does not exist, so the baseline drives the *same daemon over the same wire* through a text-rendering adapter in the harness, and the module states the projection contract it is held to: no invented content, the protocol's own closed-vocabulary tokens rather than synonyms, handles in full with no truncation, every scalar present or explicitly `none`, and lists summarized with an expansion command. Its four disciplines are anchored key lookup, closed-vocabulary parsing through `ProtocolEnum::from_wire`, explicit expansion, and bounded re-read, the last exercised on purpose by an injected flaky renderer so it is not a decoration. The baseline is deliberately **not** charged for the CLI process's own protocol frames, which is a real handicap on the typed arm and is named where it is taken. The metric is `admitted / attempted` per arm per `task × seed × policy` cell, with two anti-cheating rules stated in `run.rs` and asserted in `tests/pr10_impl01_valid_operation_rate.rs`: an operation the client refuses *locally* still counts as an attempt and as an invalid one, and both arms are driven by one shared `Policy`, so the number of mistakes is fixed by construction and only the interface varies. **The honest finding is a tie**: on this instrument the two arms' invalid-action rates are equal, cell for cell, because a disciplined scraper against a well-formed text projection classifies exactly what the typed client classifies. What the typed surface changes is the *cost* of a mistake — the register absorbs one of the two injected mistakes before a frame is written, at zero interface bytes and with no idempotency key spent — and that is reported as `zero_cost_invalid` beside the rate rather than laundered into it. The ratified 50% relative reduction in plan §24.5 is therefore **not met**, and the harness says so; grading that is bn-762i's and bn-2c0a's);
- tokens/bytes (delivered: bn-23gm — `crates/continuum-benchmark/src/run.rs`, IMPL-02, with the counting rule fixed before any number: an interface byte is a byte the agent wrote or read *at its own interface* — the request frame plus the result frame for the typed arm, the command line plus the rendered output for the text arm, with the CLI's own frames uncounted. The graded denominator is interface **bytes per solved task**, per RFC 0027 and plan §24.5's `aci-benchmark-margins`, so an arm cannot win by failing early and cheaply; `tests/pr10_impl02_interface_bytes.rs` asserts that an unsolved run contributes to neither numerator nor denominator by marking one solved run unsolved with a ten-million-byte cost and showing the figure does not move. Tokens are reported because the bullet says "tokens/bytes" and dropping the word would narrow it, but they are `ceil(bytes / 4)` by a **declared arithmetic rule** — no tokenizer, no vocabulary, no model — the rule's name is printed beside every count, and nothing is graded on it, which is RFC 0027's own reason for rejecting token-denominated enforcement. bn-2fj's lead note on the ratified margins asked for a seeds knob and per-task per-seed byte reporting: three declared seeds, every metric present for each, and a per-operation byte breakdown in the artifact. **The honest finding is that the typed surface loses this metric, and not marginally**: 10,859 interface bytes per solved task against the baseline's 4,118, a measured margin of −163% against a ratified floor of +30%. The per-operation table says where it goes and the test asserts the direction so a change is loud: a `ResultEnvelope` carries six epochs, nine cost dimensions, an assurance envelope, an omission manifest and a `next_operations` list on *every* answer, and `workspace.create` is structural rather than incidental — a local CLI names a corpus port and resolves its components from the filesystem while a remote protocol client must transmit `SnapshotComponents` in full. Whether that is worth fixing, and how, is the redesign question this lane exists to raise);
- task completion (delivered: bn-3awq — `tests/pr10_impl03_task_completion.rs`, over a four-task Phase A subset in `src/task.rs` that runs **both** corpus ports plan §21's Phase A exit names. Dining Philosophers had no `Model` anywhere in the workspace — TV-007 is a Wave 1 port and PR 8's scope was TV-009 — so `src/philosophers.rs` declares it the way `continuum-engine-reference::diehard` declares Die Hard: inert data in the engine's own vocabulary, no closures and no new engine, transcribed from `notes/plan/spikes/models.py` and cross-read against the corpus `.ctm`. `tests/philosophers_evidence.rs` checks it against the dossier before any arm runs — **573** reachable states, **2,365** labelled transitions, shortest deadlock depth **10**, and the exact deadlock state `spike-results.json` recorded, every philosopher in `HAS_LEFT` with fork *f* held by philosopher *f* — plus the distinction the corpus entry exists to show: `ForkOwnership` established over all 573 states while the deadlock refutes the campaign. The spike's `type_ok` indexes `phase` by a fork owner read out of the state, which this expression language deliberately cannot do, so it is written as the finite conjunction it denotes, folded *balanced* to stay inside the declared depth bound at every `N` the variable budget admits. Completion is not "the script finished": a run is solved only when the agent read the frozen answer **and it was the dossier's** — settled, verdict read, state count read, ceiling-enforcement known, and equal to `task::Frozen`. Every faithful cell is solved on both arms and the two arms read the same state count and the same verdict, which is the Phase A exit's "identical artifacts" at this harness's grain. Two of the four frozen numbers have no wire home and are corroborated one layer down with the boundary named at the point it is crossed, exactly as `continuumd`'s PR-8 exit evidence does: RFC 0026 F16 gives `Cost` no transition dimension, and nothing in this workspace builds the crashpack a witness would ride in. The grader can say no — a run graded against 17 states, or against `established`, is not solved — and one task records a real asymmetry rather than working around it: `dp-ownership` expects **refuted** because `verification.start` maps every target kind onto `DeadlockPolicy::Defect` and a refutation dominates the fold, so the invariant holds and the campaign is refuted anyway);
- recovery from errors (delivered: bn-1udt — `src/policy.rs`'s fallible policy and `tests/pr10_impl04_error_recovery.rs`. Nothing is mocked and no response is patched: the four injected mistakes are ordinary agent errors that land on error surfaces `continuumd` already has — presenting a `read` capability for an `execute` operation, which is `CapabilityDenied` checked before any semantic work; verifying a snapshot that was never sealed, which is `StaleSnapshot`; declaring a four-state ceiling, which is an *admitted* suspension carrying a continuation rather than a refusal, because RFC 0026 and docs/49 make budget exhaustion resumable; and editing the `.ctm` before asking for a verdict, which is `UnsupportedSemanticFeature` because a changed model is a different model. The last one's recovery is the daemon's own semantics taken seriously rather than worked around: a fork advances the lineage, so neither the pre-fork handle nor a re-create is usable — the agent puts the file back, with a second fork carrying the corpus bytes, and the model resolves again because it is keyed by its source. All twelve faulted cells on **both** arms recover to the frozen answer, and recovery is measured with its cost: every fallible run takes more attempts and more bytes than the faithful run of the same cell. The one place the interfaces genuinely differ on this bullet is the unsealed start — the typed client's register refuses it before the wire, so the typed arm never sees `StaleSnapshot` at all while the text arm spends a round trip to learn it. Two limits are recorded rather than claimed around. `Error.recovery` is RFC 0026's only recovery channel and **this daemon populates none**, so the typed arm's advantage here is the closed error *code* and not a prefilled next step; a test asserts the list is empty so the annotation cannot outrun the surface. And `recovered` is defined as faulted **and solved**, so surviving an error without reaching the dossier's answer is not reported as a recovery);
- deterministic reproduction (delivered: bn-26tb — `tests/pr10_impl05_reproduction.rs`, on the two-fresh-runs device: every claim is made by running a scenario twice against two **independently built** rigs and comparing bytes, so an agreement cannot be explained by a cache, a surviving counter, a map's internal layout or an address — the failure modes a same-daemon replay would miss. Same cell, both arms, both policies, all three seeds: identical transcripts, identical metrics, identical per-operation breakdowns; two full 48-run sweeps render byte-identical report artifacts. The determinism is structural rather than lucky, and each of its four sources is checked rather than assumed: no clock — `rig::NOW` is a constant and two fresh rigs negotiate byte-identical hello and welcome frames and name the staged corpus identically; no entropy — request identifiers and idempotency keys are minted from a monotone call counter on the typed arm and from a digest of the command line on the text arm; no ambient schedule — a "seed" selects one of three *declared* `Schedule`s rather than seeding a generator, and the three are asserted to differ in something that changes a run; and no float, no map order, no path, no locale in the artifact, which `report::Report::render` is documented to and this file holds it to. The comparison is shown to be able to fail — dropping a seed or a policy changes the artifact, and a single appended character changes a transcript — because a byte comparison that could not distinguish two different runs would prove nothing about either. The report itself is a typed value with a total rendering function rather than a printout, and it refuses to be `accepted` unless the plan §19.4 family/source-hash separation check passes over the subset it was computed on: `src/separation.rs` isolates semantic families *and* source hashes as two distinct conditions, refuses a vacuous split where one side is empty, and is falsified by `tests/pr10_family_separation.rs` on each of them — which is the goal bone's criterion 2, the G0 staging rule, held before any number is quotable rather than beside it).

**Exit:** native ACI is measurably more effective or the protocol is redesigned before freeze.

### PR 11 — Context Pack schema and compiler v0 [G2]

Implement:

- target/verdict/assurance;
- state and event slice;
- source/model references (delivered: bn-16ek — `crates/continuum-context/src/{selection,source,model}.rs`: RFC 0028's `selected[].kind` in `source`, `model` row. `SelectionKind` carries the schema's full closed eleven-member `kind` enum — `the_kind_set_is_exactly_the_schema_enum` pins it against a literal transcription of `context-pack.schema.json`'s enum — on the same reasoning `continuum-evidence`'s `EdgeRelation` gave for shipping all thirteen edge kinds against a seven-word bullet: an enum missing members of a closed schema enum cannot round-trip a conforming instance. Only `Source` and `Model` gain typed constructors here; the other nine kinds are wire tokens with no reference type and no way to build an item of that kind, which is IMPL-01/02/04/05/06's scope and not silently claimed. `SourceRef` wraps a `SourceSpan` mirroring the IDL's `SourceSpan` struct field for field, with `file` strengthened from the wire's bare `String` to `continuum_workspace::snapshot::WorkspacePath` — the type `continuum-workspace` already validates repo-relative paths with (no `.`/`..`, no traversal) — rather than an unvalidated string. `ModelActionRef` wraps a `continuum_value::value::Name` action name and an optional `model_*` handle, class-checked against `ArtifactClass::ElaboratedModel` at construction (`with_model` refuses every other class outright) rather than accepted unchecked. INV-016 ("source is untrusted data and MUST NOT be interpolated into any description") is enforced structurally: `SelectedItem::new` is `pub(crate)`, and the only two callers that exist — `SourceRef::into_selected_item`, `ModelActionRef::into_selected_item` — take no string parameter, so a `source` or `model` item's `summary` is synthesized only from the reference's own typed fields, and a `compile_fail` doctest pins that an external crate cannot fabricate one with caller-chosen prose. Canonical encoding reuses `continuum_intent::canonical_json::Json` — this workspace's one JSON-schema-artifact writer (RFC 0037 ID5's discipline, already a cross-crate dependency of `continuum-benchmark`'s) — because `context-pack.schema.json` is JSON-schema-normative exactly as `intent-contract.schema.json` is, and therefore *not* `continuum_value::value::Value`'s CVNF-1, the unrelated exact-finite-value domain's encoding a second writer here would have wrongly reached for. Evidence: `crates/continuum-context/tests/pr11_impl03_source_model_references.rs` (7 tests) plus 22 unit tests plus one `compile_fail` doctest, 30 in total, covering the closed-vocabulary fail-closed discipline, INV-016's structural enforcement, per-kind artifact discipline (a source item never carries one; a model item carries the class-checked `model_*` handle or none), determinism (two independent builds of one reference are byte-identical; distinct references are never byte-identical — both directions asserted, so neither check is vacuous), and `rule ordering.deterministic` (independently built lists of the same items sort to identical canonical bytes regardless of construction order). Three things declined, honestly: per-item content identity under ADR-0013 — the schema gives no `selected[]` item its own `content_hash`, only the whole pack does, and pack assembly is no single IMPL bullet's fence, so exact structural `PartialEq`/`Ord` is delivered in its place; expansion-handle integration (`ExpansionRelation`, `expansions[]`, the omission manifest) is IMPL-04's (bn-28jj), unopened at landing — `SelectedItem::id` is shaped as the anchor a future `expansions[].anchor` would name and nothing more is claimed; and minting a real `model_*` handle is `continuum-cml-elab`'s (an unopened PR-1 scaffold in this workspace), so `ModelActionRef::with_model` accepts one when a caller has one but this bullet cannot produce one itself);
- omissions and expansion handles;
- replay reference;
- byte/token budgets.

Start with graph reachability and invariant failures.

**Exit:** the synthetic 200-event durability case compiles to a replay-preserving core with substantial reduction.

### PR 12 — Intent semantic diff v0 [G3]

Classify supported changes:

- exact equality;
- property AST edit;
- assumption add/remove;
- bound change;
- observer event change (delivered: bn-1sdp — `crates/continuum-semantic-diff/src/observers.rs`: RFC 0031's `observers` field classification over `continuum-intent`'s `Observer`/`ObserverSet`, componentwise across the four projection sets into `unchanged`/`refined`/`coarsened` plus `added`/`removed` membership for units present on one side only, closed over `continuum-intent::change_policy::Relation` rather than a second vocabulary. `unknown`/`unsupported` are never emitted: observer comparison is decidable outright (plain finite set inclusion, no solver), and `observers[]` carries no `fragment` member to fall outside of, pinned against the live schema rather than assumed. Evidence: `tests/pr12_impl05_observer_event_change_evidence.rs` — a real-corpus (`replicated-register-contract.json`) coarsening of the "hide observer events" attack (plan §19.5) closed all the way into `PolicyTable::verdict`'s `review` outcome under the fixture's own policy; an adversarial disguise sweep (same-size event swap, rename, coarsen-then-rename) proving a dropped element never classifies `unchanged`/`refined`/silent `added`; and two anti-vacuity mutants showing the positive assertions are not vacuously true. Library-level classifier only: the wire `intent_changes[]`/`diff_*` artifact assembly, the impact set, and the other five PR-12 bullets — exact equality, property AST edit, assumption add/remove, bound change, fault/fairness/assurance change — remain open, each its own bone);
- fault/fairness/assurance change.

Add solver-based implication only where sound and bounded.

**Exit:** all G0 intent-gaming patches are privileged changes; a source-only guard repair is not.

### PR 13 — Human CLI v0 [G1]

Implement stable commands and output, aligned with plan §13.4/§3.1:

```text
continuum snapshot
continuum check
continuum explain
continuum debug
continuum repair
continuum evidence show
continuum context expand
continuum task status/resume/cancel
```

Support `--json`; prose is a projection.

**Exit:** golden tests pin JSON, exit codes, and concise terminal output.

### PR 14 — Asupersync semantic journal [G4]

Instrument narrow primitives:

- task/region lifecycle;
- reserve/commit/abort;
- cancellation phases;
- obligations;
- virtual time;
- channel communication.

**Exit:** identical controlled choice logs produce canonical identical semantic events.

### PR 15 — Network/process/storage packs [G4]

Implement only the profiles needed for replicated register:

- delivery/drop/duplicate/delay/partition;
- crash/restart/epoch;
- submit/stable/sync/ack;
- declared unsupported cases.

**Exit:** crash windows and cancellation points replay exactly.

### PR 15a — CML core-fragment parser and elaborator [G4]

Implement `continuum-cml-syntax` and `continuum-cml-elab`:

- the Finite fragment only;
- enough surface for the replicated register and the Wave 0 corpus ports;
- the programmatic model API remains supported — CML is a second front end, not a replacement.

**Exit:** the replicated-register model written in CML elaborates to the same semantic model identity as its programmatic equivalent.

### PR 15b — CML formatter and migration tool [G5]

Implement:

- deterministic CML formatter defined over the normalized semantic AST (the AST is frozen before surface syntax, docs/11 §14);
- migration tool rewriting existing CML sources across surface-syntax revisions;
- ships before CML syntax stability is declared (plan §21 Phase C).

**Exit:** formatting is idempotent and semantics-preserving — a formatted or migrated source elaborates to the same semantic model identity — across the replicated-register model and the Wave 0 CML ports.

### PR 16 — Replicated-register model and implementation [G4]

Create:

- abstract atomic register;
- operational durable register;
- asupersync implementation;
- correct version;
- ack-before-sync, lost-abort, stale-epoch, and orphan mutants.

**Exit:** each mutant has an expected intent/property and deterministic campaign.

### PR 17 — CIR and concrete/abstract correspondence [G4]

Implement:

- event/configuration validation;
- causal/conflict edges;
- concrete/abstract state projection;
- stuttering classification;
- uncovered semantic effect errors.

**Exit:** correct implementation satisfies bounded refinement; mutants fail at mapped transitions.

### PR 18 — Causal minimization [G4]

Implement deletion, causal-closure, owner/fault/value reduction, and replay validation.

**Exit:** ack-before-sync failure reduces to a compact core and does not delete the actual causal mechanism.

### PR 19 — Verification debugger core [G4]

Implement:

- selected configuration;
- enabled frontier;
- semantic step/reverse;
- branch on alternate event;
- state/observer/obligation views;
- why-enabled/blocked derivations.

**Exit:** branch before `Sync`/`Ack` shows safe and failing successors from one handle.

### PR 20 — Repair Transaction v0 [G3, G4]

Implement begin/apply/evaluate/promote with:

- hypothesis;
- patch identity;
- exact replay;
- semantic/intent diff;
- evidence accumulation;
- policy verdict.

**Exit:** moving ack after sync is evaluable; a property-weakening patch is reclassified and blocked.

### PR 21 — Neighboring exploration and mutation challenge [G4; closes re-homed G0-DX-06]

Generate semantic neighbors around the causal core and run known property/model mutants.

**Exit:** a hard-coded exact-trace repair fails; the semantic guard repair passes the bounded envelope.

### PR 22 — Promotion receipts [G3, G4]

Compose:

- intent identity;
- before/after snapshots;
- semantic diff;
- replay/neighborhood/mutation results;
- refinement/certificate status;
- unknowns;
- `gate_profile` (the phase-staged profile the receipt was evaluated under, plan §21);
- `NotYetEnforced` list — gates not yet in the profile, never rendered as passed;
- policy decision.

**Exit:** receipt independently verifies references and cannot be forged by the agent client.

### PR 22a — Incremental-engine ADR and reuse-edge spike [G5]

Implement:

- the ADR resolving build-vs-adopt for the incremental engine (salsa-derived vs custom); Phase C is `BLOCKED` until this ADR exists (plan §21, docs/08 R21);
- spike implementing the four reuse-edge classes and `query.explain_invalidation` over parse/elaborate/explore;
- a measured invalidation-precision baseline recorded in the ADR.

**Exit:** the ADR is merged with spike evidence and the invalidation-precision baseline; PR 23 may not merge before it.

### PR 23 — Incremental query database v0 [G5]

Implement content-addressed queries for parsing, model construction, property automata, exploration, context, and diff. Classify edges as exact/validated/conservative/experimental.

**Exit:** property-only edit does not rebuild unrelated extraction; model action edit invalidates reachable graph and dependent context.

### PR 24 — Incremental Parity Audit [G5]

(Renamed from "clean-build Tribunal"; **Tribunal** refers exclusively to the TLA+ corpus oracle harness, plan §9.5.)

Randomly and deterministically compare incremental and clean results. Minimize invalidation mismatches.

**Exit:** intentionally broken dependency edge is caught and quarantined.

### PR 25 — LSP authoring adapter [G5]

Implement CML/source diagnostics, semantic hover, go-to correspondence, code lenses, and intent-change preview over snapshots.

**Exit:** unsaved buffer overlays create explicit snapshots and never mutate daemon state implicitly.

### PR 25a — SARIF exporter [G5]

Implement `continuum-sarif` per plan §17.3:

- source-located, deduplicated diagnostics with artifact URIs;
- stable rule IDs and severity;
- code flows;
- evidence handles;
- SARIF is a report projection, not a proof format.

**Exit:** the replicated-register failure exports schema-valid SARIF with stable rule IDs across reruns; every result carries an evidence handle.

### PR 26 — DAP adapter [G5]

Map Continuum debugger state to DAP threads, frames, scopes, variables, breakpoints, and stepping; expose custom causal operations.

**Exit:** VS Code-compatible client can inspect and branch the replicated-register failure.

### PR 26a — Read-only tokio observation lane [G5]

Implement (Phase C adoption top-of-funnel, plan §21):

- journal observable lifecycle, channel, and time events from an unmodified tokio program;
- opaque-effect inventory for everything not observable;
- assurance envelopes capped at `Observed` status with every claim dimension `Unsupported` — no controlled-semantics claim.

**Exit:** a tokio example produces a semantic journal and an envelope capped at `Observed`; no claim dimension can exceed `Unsupported` through this lane.

### PR 27 — MCP adapter [G2]

Expose curated native operations with explicit handles, deterministic schemas/order, result bounds, and capability checks.

**Exit:** two subagents share one workspace/intent but use isolated debugger/proof handles without session coupling.

### PR 27a — Wave 0/1 corpus at min(required, P2) [G9]

Implement:

- the 29 Wave 0/1 corpus families, pinned to `tlaplus/Examples@91c22ea…` (plan §21 Phase C);
- each family at min(required, P2) parity per `corpus/tla-examples/PARITY_LEVELS.md`;
- the P3/P4 raises for the 17 families requiring proof/liveness machinery are deferred to Phase D.

**Exit:** all 29 families check deterministically in CI at min(required, P2).

### PR 27b — G8 preregistration and formative-session instruments [G8]

Implement:

- formative think-aloud session instruments (~5 participants per persona) for the edit/check/explain loop, run each phase from Phase C onward and feeding docs/34 — non-binding on the summative G8 study;
- authoring of the expanded docs/48 preregistration (workflows, cohorts, baseline comparator, instruments, pass thresholds) — a Phase E deliverable; the summative study itself runs in Phase F (plan §21.1).

**Exit:** formative instruments are in use from Phase C; the preregistration document is published before the summative G8 study runs.

### PR 28 — Lean proof service and Context Pack [G6; closes re-homed G0-DX-11]

Implement the proof *service* over the environment pinned in PR 4a: per-request isolation and cancellation, goals, diagnostics, receipts with theorem/axiom manifests, and relevant-context extraction. (Lean pinning and the foundational T0/T1 theorems are PR 4a, not here.)

**Exit:** concurrent requests across Lean epochs are isolated and cancellable with deterministic results; every proof receipt carries theorem and axiom manifests with zero unapproved axioms.

### PR 29 — Forge finite CEGIS v0 [G7]

Implement typed holes, finite grammar enumeration, exact counterexample feedback, safety plus progress/non-vacuity, and candidate archive.

**Exit:** acknowledgement guard task synthesizes `synced`, rejects never-ack, and independently verifies the result.

### PR 30 — End-to-end agent benchmark [G2, G4]

Give an advanced coding agent only:

- repository snapshot;
- protected intent;
- failing Context Pack;
- native operations;
- bounded budget.

Require it to diagnose, patch, evaluate, and request promotion.

**Exit:** the complete receipt is produced without terminal parsing or human repair guidance; all operations and evidence are replayable.

## Work deliberately deferred

Until PR 30 closes, defer:

- distributed model checking;
- cloud control plane;
- polished web product;
- generalized weak-memory engine;
- CML surface syntax beyond the Finite core fragment (the fragment itself ships in PR 15a);
- all 80 corpus ports;
- unrestricted synthesis grammars;
- learned search in the trusted loop;
- automatic production deployment;
- broad foreign-runtime support.

Not deferred: the CML core fragment (PR 15a) and SARIF export (PR 25a) are scheduled above.

These are multipliers. They must not precede the trustworthy loop they multiply.
