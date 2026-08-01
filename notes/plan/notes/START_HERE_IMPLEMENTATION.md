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

- programmatic transition model (delivered: bn-2d0e — `crates/continuum-engine-reference/src/{ident,domain,expr,model,diehard}.rs`: named variables over inclusive `i64` domains, named actions as guarded deterministic-or-enumerated simultaneous updates, an explicit initial-state enumeration, and named predicates, all as inert data rather than closures so INV-005 is a property of the types; state vectors, action indices, initial states and successor rows in the certificate wire form's canonical byte order, and the TV-009 Die Hard port asserted against the frozen 16-state/96-transition/depth-6 facts);
- deterministic BFS (delivered: bn-3p8u — `crates/continuum-engine-reference/src/bfs.rs`: an explicit FIFO queue of `(state, depth)` over a `BTreeMap` visited set, so the reachable set comes out of the walk already in the certificate wire form's ascending state-table order with a parallel depth column and no sorting pass; a state is expanded atomically and new targets are admitted in ascending state order, which makes the explored set a prefix of the canonical walk and the frontier a resume point. Three bounds — states, depth and transitions, the first and third taken from the state and transition ceilings `continuum-kernel-core`'s wire form declares, the second the engine's own because no certificate carries a depth — are declared per call with no default, and reaching one returns a typed partial result carrying the explored set, the frontier in queue order and which bound tripped, never a silent truncation (INV-008/INV-009, and the engine-grain shape of RFC 0026's budget-exhausted-carries-a-continuation rule); only the `Complete` arm exposes a closed set, so a finite-closure certificate cannot be built from a truncated one. Die Hard's frozen 16 states / 96 transitions / depth-6 witness are re-derived through the engine and asserted equal, state for state and depth for depth, to the independent hand walk the Die Hard evidence suite already carried);
- invariant/deadlock checking (delivered: bn-3e0m — `crates/continuum-engine-reference/src/checking.rs`: which of a model's declared predicates are invariants, and whether a state with no enabled action is a defect, are the caller's declaration rather than the model's, so one model can be checked under several policies without being re-declared. The deadlock half is the engine-grain projection of the intent contract's completion policy, whose closed four-member vocabulary no engine may quietly reinterpret; the arm that vocabulary's environment-closed member would map to is deliberately absent, because this layer declares no environment and inventing one would be a guess. Each invariant gets a typed outcome from the assurance result's own three-way verdict — established, refuted, inconclusive with a reason — never a bare boolean, and the reasons are spelled exactly as INV-008's closed reason vocabulary spells the two of its six members a finite engine can produce. The asymmetry the exploration bone left here is encoded: a violation found inside a bounded run is a real refutation, because the state was genuinely reached and no larger budget can un-reach it, while *not* finding one is inconclusive naming the tripped bound, the explored count and the frontier — nothing in the module can report an invariant as holding from a truncated run, at any bound. A refutation carries a shortest path when the exploration closed and a typed absence when it did not, because the witness extractor answers only for a closed exploration and this bone does not work around its sibling's refusal. Terminality is recomputed per discovered state rather than read off the walk, so a deadlock sitting unexpanded in a bounded run's frontier is still reported. Die Hard checks as the corpus declares it: type-correctness established over the closed sixteen, the intentionally-false invariant refuted at four gallons and three by the film's six-step solution, and no deadlock under the strictest policy);
- shortest witness (delivered: bn-2pmc — `crates/continuum-engine-reference/src/witness.rs`, on a behaviour-preserving extension of `src/bfs.rs`: every discovered state now records the labelled transition that first reached it, written at the moment its depth is written and under the same canonical admission order, so the chain of those records is itself a shortest path and no second search is needed. Recording it changed nothing else — the newly admitted targets remain a set keyed by state, iterated in ascending state order, of the same size, so the explored states, their depths, the queue-ordered frontier, the counters and the bound checks are the values they were before, asserted against an independent hand walk and against the frozen frontier of a bounded Die Hard run. A target is data — one state, or one of the model's own declared predicates holding or failing — never a caller-supplied closure, because a witness's endpoint has to be something the model can state; the endpoint chosen is the shallowest satisfying state with ties going to the canonically least, so a witness is a function of the model and the target alone. Extraction answers only for a closed exploration, mirroring the closure seam: a path out of a bounded run is not known to be shortest and is refused by name, even when the endpoint was in fact discovered at its true depth (INV-008, INV-009). Die Hard's shortest violation of NotSolved comes back as the film's six-step solution ending at four gallons and three, pinned equal to the path the Lean port proves and replayed one transition at a time through the model layer);
- finite closure certificate (delivered: bn-3e4l — `crates/continuum-engine-reference/src/certificate.rs`: a closed exploration written out as CONTCERT wire bytes, from the grammar documented in `continuum-kernel-core`'s wire module rather than from its types, so producer and checker share no codec — docs/03 §8 lists certificate decoding among the axes independent paths must differ along — and the engine keeps no library dependency on the kernel at all. Emission takes a closed-set witness whose only constructor is the exploration's own closed-set accessor, so a certificate cannot be built from a bounded walk even by mistake; the eight claim-envelope fields of RFC 0005 are the caller's, shape-checked against the kernel's token grammar before a byte is written, because six of them are digests of artifacts this crate cannot see and one is the wire epoch the format fixes — the same trusted-input seam the kernel's own receipt uses, where nothing is invented. Emission is pure: one model, one closed set, one envelope, byte-identical certificate. The fourth frozen TV-009 fact is now asserted rather than deferred — Die Hard is explored through the engine, emitted, and handed to the kernel's checker as bytes, which verifies 16 reachable states, 96 labelled transitions and one initial state under the state-domain property class; every byte of the canonical state table, mutated two ways, is refused, as are a wrong magic, a foreign wire epoch, a foreign certificate family, a trailing byte and every prefix).

**Exit:** Die Hard returns 16 states and depth-6 solution through the daemon API.

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
