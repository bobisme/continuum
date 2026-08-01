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

**Exit:** the seven RFCs are normative specifications, and the pre-freeze open-debt set (plan §25, validator `check_spec_debt`) reads empty — not only the seven RFC expansions; PR 5 may not merge before PR 0 closes. PR 0 is Phase A's designated opening PR; it has no staffing merge requirement (plan §21.1).

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

- Merkle workspace snapshots;
- disk import, editor overlay, fork, seal, and diff;
- source/dependency/toolchain/config identities;
- stale-snapshot error;
- deterministic file ordering.

**Exit:** two clients can fork and analyze independently; an old snapshot remains reproducible after the working tree changes.

### PR 4 — Intent Contract v0 [G1, G3]

Implement schema/types for:

- properties;
- assumptions;
- observers;
- bounds;
- faults;
- fairness;
- assurance policy;
- optimization/non-vacuity;
- field-level change policy.

**Exit:** Die Hard and replicated-register intents serialize canonically; ordinary operations cannot mutate them.

### PR 4a — Lean environment and seed theorems [G6; Phase A band]

Implement:

- pinned Lean toolchain (`leanprover/lean4:v4.32.1`) (delivered: bn-31mq);
- kernel-check the existing seed modules under `lean/Continuum/` (delivered: bn-fak5);
- T0/T1 theorems: transition-system safety, stuttering simulation, finite-closure certificate soundness (delivered: bn-3qsa).

**Exit:** T0/T1 compile with no `sorry` and empty axiom manifests (RFC 0012 theorem ladder; axiom manifests per ADR-0035). The proof *service* remains PR 28 (delivered: bn-zr81).

### PR 5 — Native protocol kernel [G1, G2]

Implement request/response types and local transport for the plan §10.2 operation names:

- workspace.create / fork / diff / seal;
- intent.get / diff / propose_revision / accept / reject / lock;
- verification.start; task.status / cancel / resume / subscribe;
- evidence.get / query / verify;
- capability negotiation;
- typed errors and idempotency keys.

**Exit:** replaying an idempotent request returns the same task/artifact identity. May not merge before PR 0 closes.

### PR 6 — Cancel-correct task service [G0 (DX-14), G1]

Use asupersync regions for daemon work:

- task lifecycle;
- committed partial evidence;
- budget accounting;
- suspension/continuation;
- drain/finalize behavior;
- no orphan workers.

**Exit:** cancellation at every instrumented phase leaves either a valid continuation or no published partial artifact.

### PR 7 — Evidence Graph v0 [G1]

Implement node/edge/status types, immutable versions, and queries:

- support/refute/depend/refine/explain/repair/check edges;
- provenance;
- trusted status transitions;
- conflict nodes.

**Exit:** an untrusted client cannot promote a proposal to validated/proved.

### PR 8 — Exact finite model service [G1, G2]

Wrap Revision 2 reference semantics behind native protocol:

- programmatic transition model;
- deterministic BFS;
- invariant/deadlock checking;
- shortest witness;
- finite closure certificate.

**Exit:** Die Hard returns 16 states and depth-6 solution through the daemon API.

### PR 9 — Trusted kernel crates [G1, G6]

Implement the trusted checking base as four crates — `continuum-kernel-core`, `continuum-kernel-sat`, `continuum-kernel-smt`, `continuum-kernel-temporal` (plan §20):

- finite closure/type certificate checking;
- no shared evaluator code with any engine; no async, no unsafe, no plugins or dynamic loading;
- <15,000 non-test-line covenant across the four crates (docs/03);
- serialization boundary: certificates are checked from wire form, never from shared memory;
- receipt generation with checker epoch, build digest, and input hashes;
- mutation tests.

**Exit:** every single-field certificate mutation in the test suite is rejected, checking wire-form input only.

### PR 10 — Agent client and ACI benchmark harness [G0 (DX-10), G2]

Implement a minimal client exposing only typed operations. Compare against a shell-scraping baseline on Die Hard and Dining Philosophers tasks.

Measure:

- valid operation rate;
- tokens/bytes;
- task completion;
- recovery from errors;
- deterministic reproduction.

**Exit:** native ACI is measurably more effective or the protocol is redesigned before freeze.

### PR 11 — Context Pack schema and compiler v0 [G2]

Implement:

- target/verdict/assurance;
- state and event slice;
- source/model references;
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
- observer event change;
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
