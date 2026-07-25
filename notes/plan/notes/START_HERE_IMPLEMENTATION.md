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

Do not begin with a polished web UI, a general LLM coordinator, distributed search, or a complete CML parser. The first users are the implementation team and coding agents driving the first vertical slice.

## Dependency islands

```text
continuum-workspace / continuum-intent
          │
          ▼
continuum-task / continuum-evidence / continuum-protocol
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

The certificate checker may not depend on search. The model core may not depend on asupersync. Adapters may not own semantic state. Forge may not be imported by the verifier.

## First 30 pull requests

### PR 1 — Revision 3 constitution and epochs

Implement:

- Rust workspace and crate boundaries;
- protocol, semantic, intent, evidence, proof, and corpus epochs;
- assurance and inconclusive enums;
- `#![forbid(unsafe_code)]` defaults;
- deterministic build and artifact paths;
- claim-status lattice.

**Exit:** an unsupported empty task returns a valid machine result naming every epoch and no misleading success flag.

### PR 2 — Canonical values and CAS primitives

Implement:

- exact finite values from Revision 2;
- canonical encoding and total order;
- cryptographic content identity;
- collision-injection tests;
- atomic artifact publication;
- authorization separate from handle possession.

**Exit:** concurrent publication of identical artifacts yields one identity; artificial hash collisions are detected/resolved.

### PR 3 — Workspace snapshots

Implement:

- Merkle workspace snapshots;
- disk import, editor overlay, fork, seal, and diff;
- source/dependency/toolchain/config identities;
- stale-snapshot error;
- deterministic file ordering.

**Exit:** two clients can fork and analyze independently; an old snapshot remains reproducible after the working tree changes.

### PR 4 — Intent Contract v0

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

### PR 5 — Native protocol kernel

Implement request/response types and local transport for:

- workspace create/get/fork;
- intent get;
- task start/status/cancel/resume;
- artifact get;
- capability negotiation;
- typed errors and idempotency keys.

**Exit:** replaying an idempotent request returns the same task/artifact identity.

### PR 6 — Cancel-correct task service

Use asupersync regions for daemon work:

- task lifecycle;
- committed partial evidence;
- budget accounting;
- suspension/continuation;
- drain/finalize behavior;
- no orphan workers.

**Exit:** cancellation at every instrumented phase leaves either a valid continuation or no published partial artifact.

### PR 7 — Evidence Graph v0

Implement node/edge/status types, immutable versions, and queries:

- support/refute/depend/refine/explain/repair/check edges;
- provenance;
- trusted status transitions;
- conflict nodes.

**Exit:** an untrusted client cannot promote a proposal to validated/proved.

### PR 8 — Exact finite model service

Wrap Revision 2 reference semantics behind native protocol:

- programmatic transition model;
- deterministic BFS;
- invariant/deadlock checking;
- shortest witness;
- finite closure certificate.

**Exit:** Die Hard returns 16 states and depth-6 solution through the daemon API.

### PR 9 — Independent certificate service

Separate process/crate for:

- finite closure/type certificate checking;
- receipt generation;
- mutation tests;
- checker epoch and input hashes.

**Exit:** every single-field certificate mutation in the test suite is rejected.

### PR 10 — Agent client and ACI benchmark harness

Implement a minimal client exposing only typed operations. Compare against a shell-scraping baseline on Die Hard and Dining Philosophers tasks.

Measure:

- valid operation rate;
- tokens/bytes;
- task completion;
- recovery from errors;
- deterministic reproduction.

**Exit:** native ACI is measurably more effective or the protocol is redesigned before freeze.

### PR 11 — Context Pack schema and compiler v0

Implement:

- target/verdict/assurance;
- state and event slice;
- source/model references;
- omissions and expansion handles;
- replay reference;
- byte/token budgets.

Start with graph reachability and invariant failures.

**Exit:** the synthetic 200-event durability case compiles to a replay-preserving core with substantial reduction.

### PR 12 — Intent semantic diff v0

Classify supported changes:

- exact equality;
- property AST edit;
- assumption add/remove;
- bound change;
- observer event change;
- fault/fairness/assurance change.

Add solver-based implication only where sound and bounded.

**Exit:** all G0 intent-gaming patches are privileged changes; a source-only guard repair is not.

### PR 13 — Human CLI v0

Implement stable commands and output:

```text
continuum snapshot
continuum check
continuum evidence show
continuum context expand
continuum task status/resume/cancel
```

Support `--json`; prose is a projection.

**Exit:** golden tests pin JSON, exit codes, and concise terminal output.

### PR 14 — Asupersync semantic journal

Instrument narrow primitives:

- task/region lifecycle;
- reserve/commit/abort;
- cancellation phases;
- obligations;
- virtual time;
- channel communication.

**Exit:** identical controlled choice logs produce canonical identical semantic events.

### PR 15 — Network/process/storage packs

Implement only the profiles needed for replicated register:

- delivery/drop/duplicate/delay/partition;
- crash/restart/epoch;
- submit/stable/sync/ack;
- declared unsupported cases.

**Exit:** crash windows and cancellation points replay exactly.

### PR 16 — Replicated-register model and implementation

Create:

- abstract atomic register;
- operational durable register;
- asupersync implementation;
- correct version;
- ack-before-sync, lost-abort, stale-epoch, and orphan mutants.

**Exit:** each mutant has an expected intent/property and deterministic campaign.

### PR 17 — CIR and concrete/abstract correspondence

Implement:

- event/configuration validation;
- causal/conflict edges;
- concrete/abstract state projection;
- stuttering classification;
- uncovered semantic effect errors.

**Exit:** correct implementation satisfies bounded refinement; mutants fail at mapped transitions.

### PR 18 — Causal minimization

Implement deletion, causal-closure, owner/fault/value reduction, and replay validation.

**Exit:** ack-before-sync failure reduces to a compact core and does not delete the actual causal mechanism.

### PR 19 — Verification debugger core

Implement:

- selected configuration;
- enabled frontier;
- semantic step/reverse;
- branch on alternate event;
- state/observer/obligation views;
- why-enabled/blocked derivations.

**Exit:** branch before `Sync`/`Ack` shows safe and failing successors from one handle.

### PR 20 — Repair Transaction v0

Implement begin/apply/evaluate/promote with:

- hypothesis;
- patch identity;
- exact replay;
- semantic/intent diff;
- evidence accumulation;
- policy verdict.

**Exit:** moving ack after sync is evaluable; a property-weakening patch is reclassified and blocked.

### PR 21 — Neighboring exploration and mutation challenge

Generate semantic neighbors around the causal core and run known property/model mutants.

**Exit:** a hard-coded exact-trace repair fails; the semantic guard repair passes the bounded envelope.

### PR 22 — Promotion receipts

Compose:

- intent identity;
- before/after snapshots;
- semantic diff;
- replay/neighborhood/mutation results;
- refinement/certificate status;
- unknowns;
- policy decision.

**Exit:** receipt independently verifies references and cannot be forged by the agent client.

### PR 23 — Incremental query database v0

Implement content-addressed queries for parsing, model construction, property automata, exploration, context, and diff. Classify edges as exact/validated/conservative/experimental.

**Exit:** property-only edit does not rebuild unrelated extraction; model action edit invalidates reachable graph and dependent context.

### PR 24 — Clean-build differential Tribunal

Randomly and deterministically compare incremental and clean results. Minimize invalidation mismatches.

**Exit:** intentionally broken dependency edge is caught and quarantined.

### PR 25 — LSP authoring adapter

Implement CML/source diagnostics, semantic hover, go-to correspondence, code lenses, and intent-change preview over snapshots.

**Exit:** unsaved buffer overlays create explicit snapshots and never mutate daemon state implicitly.

### PR 26 — DAP adapter

Map Continuum debugger state to DAP threads, frames, scopes, variables, breakpoints, and stepping; expose custom causal operations.

**Exit:** VS Code-compatible client can inspect and branch the replicated-register failure.

### PR 27 — MCP adapter

Expose curated native operations with explicit handles, deterministic schemas/order, result bounds, and capability checks.

**Exit:** two subagents share one workspace/intent but use isolated debugger/proof handles without session coupling.

### PR 28 — Lean proof worker and Context Pack

Pin Lean environment; implement isolated checking, goals, diagnostics, axiom manifests, and relevant-context extraction.

**Exit:** foundational finite closure/refinement theorems compile and receipts include zero unapproved axioms.

### PR 29 — Forge finite CEGIS v0

Implement typed holes, finite grammar enumeration, exact counterexample feedback, safety plus progress/non-vacuity, and candidate archive.

**Exit:** acknowledgement guard task synthesizes `synced`, rejects never-ack, and independently verifies the result.

### PR 30 — End-to-end agent benchmark

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
- all 80 corpus ports;
- unrestricted synthesis grammars;
- learned search in the trusted loop;
- automatic production deployment;
- broad foreign-runtime support.

These are multipliers. They must not precede the trustworthy loop they multiply.
