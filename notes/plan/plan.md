# Continuum: Revision 3 Master Implementation Plan

**Working name:** Continuum  
**Document date:** 2026-07-24  
**Status:** implementation architecture and research program  
**Primary implementation language:** Rust  
**Concrete runtime:** asupersync  
**Proof authority:** Lean 4  
**Compatibility tribunal:** TLA+ Examples, TLC, Apalache, Quint, P, Loom/Shuttle, and selected proof systems  
**Product thesis:** verification-native development and invention for concurrent/distributed Rust systems

---

## 0. Declaration of intent

Continuum exists to make formally grounded concurrent and distributed systems engineering the normal way humans and advanced coding agents work—not a specialist ritual performed after design decisions have hardened.

The project is not a Rust rewrite of TLC. It is not a deterministic simulator with formal-methods branding. It is not an LLM wrapper around TLA+, Lean, or a pile of subprocesses. It is not a magical claim that one language or algorithm defeats undecidability.

Continuum is a coherent semantic environment in which:

- an abstract model can be written before production code exists;
- real asupersync Rust code runs under production and controlled execution semantics;
- model and implementation are connected by explicit refinement;
- safety, liveness, fairness, cancellation, durability, faults, time, and weak memory are represented honestly;
- counterexamples are reduced to causal explanations;
- successful results carry independently checkable evidence;
- production observations can be checked without inventing a false total order;
- humans receive progressive, comprehensible tooling;
- agents receive a compact, typed Agent–Computer Interface rather than terminal sludge;
- repairs cannot game properties, assumptions, bounds, observers, or assurance policy;
- protocol and algorithm synthesis happens inside a fixed intent and proof envelope.

The north star is:

> A capable coding agent should be able to invent or repair a concurrent Rust algorithm, receive exact semantic feedback, iterate autonomously, and produce a patch whose behavior, assumptions, coverage, and proof obligations are legible to a human and checkable independently.

That requires more than a strong verifier. It requires an impeccable workbench.

---

## 0.1 Revision 3: the product becomes the proof-guided loop

Revision 2 established the semantic triptych:

```text
Model ↔ Program ↔ Proof
```

Revision 3 protects the loop with **Intent** and makes **Evidence** the sole currency of progress:

```text
                              Intent
                   what the system must mean
                                  │
                   ┌──────────────┼──────────────┐
                   ▼              ▼              ▼
                 Model          Program          Proof
             abstract math   asupersync Rust   Lean/certs
                   └──────────────┼──────────────┘
                                  ▼
                               Evidence
                                  │
                                  ▼
                              Workbench
```

### Intent

Intent includes:

- property ASTs and observer scopes;
- environment assumptions;
- weak/strong fairness declarations;
- model bounds and cutoff claims;
- fault and recovery envelope;
- storage/network/time semantics;
- trusted and opaque boundaries;
- assurance requirements;
- optimization objectives and non-vacuity constraints;
- security and privacy policy.

Intent is content-addressed and immutable by default. Any change receives a semantic diff and cannot be smuggled inside an ordinary repair.

### Evidence

Evidence includes:

- exact counterexamples and replay records;
- causal cores and counterfactual branches;
- finite closure certificates;
- SAT/SMT/CHC/PDR proof artifacts;
- Lean theorem receipts and axiom manifests;
- refinement witnesses;
- coverage and reduction certificates;
- mutation sensitivity;
- production trace verdicts;
- explicit `Inconclusive` explanations.

A claim without evidence is a hypothesis, regardless of whether it came from a human, an agent, or a sophisticated engine.

### Workbench

The workbench is not an interface veneer. It determines whether the semantic system can be used correctly. It owns:

- immutable workspace snapshots;
- incremental query execution;
- typed verification tasks;
- Context Packs;
- the causal debugger;
- semantic and intent diff;
- repair transactions;
- proof and solver isolation;
- multi-agent evidence coordination;
- Continuum Forge;
- evaluation and anti-reward-hacking gates.

---

## 0.2 The product theorem

Continuum should satisfy the following engineering theorem:

> For every accepted operation, the user can determine what snapshot was analyzed, what intent was protected, what semantic engines ran, what was explored or proved, what was omitted, what assumptions were used, what changed, and how to reproduce or independently check the result.

The universal operation shape is:

```text
(snapshot, intent, operation, budget, policy)
  → verdict
  + evidence references
  + new snapshot or continuation
  + explicit omissions/unknowns
  + allowed next operations, warnings, cost, and epoch identities
```

This shape applies equally to:

- `check`;
- `explore`;
- `prove`;
- `explain`;
- `debug`;
- `repair`;
- `synthesize`;
- `observe`;
- `review`.

No semantic state is hidden in a terminal process, editor connection, chat session, or MCP transport.

---

## 0.3 Claim status of this plan

This plan uses the dossier claim lattice (`README.md`). Unless marked
otherwise, sections here are DESIGN (accepted architecture, not evidence).
The following are explicitly weaker:

- HYPOTHESIS: §6 general context compilation, §8.3 neighborhood adequacy,
  §9 sub-file-granularity incremental trust, §10 ACI superiority (B2),
  §11.7 concurrent evidence-graph enforcement, §12 causal explanation
  science, §14 Forge co-synthesis and quality-diversity, §16
  bidirectional lenses, production partial-order conformance (Phase F).
  Each is owned by a research lane with kill criteria (see §24.5).
- Evidence boundary: no Lean source in this dossier has been parsed,
  elaborated, or kernel-checked; no theorem may be described as
  machine-checked (`VALIDATION_REPORT.md`, `docs/18` C035). The Revision 3
  spikes are finite Python reference experiments that validate artifact
  shapes and interaction contracts, not engines, scale, concurrency,
  persistence, or soundness (`docs/53`, "What remains unproven").
- G0 status: DX-01, DX-02, DX-03, and DX-12 carry spike evidence.
  DX-10, DX-13, and DX-14 are open and freeze-blocking (Phase A).
  DX-04, DX-05, DX-06, DX-07, DX-08, DX-09, DX-11, and DX-15 are
  re-homed to the gates owning their machinery (G4, G5, G4, G7, G6,
  G8, G6, G9 respectively — see §22 G0); each re-homing is that item's
  recorded decision, and the four re-homed items with Phase A spike
  results (DX-04, 05, 07, 08) carry them as artifact-shape evidence
  only. Statuses live in `notes/G0_SPIKE_MATRIX.md`, from which these
  counts derive.
- Program status: `READY` for autonomous-agent dispatch. Section §21.1
  defines an elastic swarm execution model with no named-owner or
  headcount prerequisite. `notes/START_HERE_IMPLEMENTATION.md` maps
  phase-opening PRs and dispatch conditions; the Bones dependency graph,
  explicit authority boundaries, and retained acceptance evidence decide
  what can run and what can close. `validation-results.json`
  (`program_status`) validates this execution posture beside the
  mechanical pass/fail. `READY` is not a claim that implementation or
  any release gate is complete.

---

## 1. Twenty-five design bets

### B1 — Intent integrity is a verification property

A tool that proves a weakened property has failed. Continuum versions intent and treats unauthorized changes as violations.

### B2 — Agent interface quality is part of verifier capability

An agent forced to scrape prose, track cursor positions, or reconstruct task state from logs will make more invalid moves and consume more context. Machine users receive concise typed operations and stable handles.

### B3 — `continuumd` is the authority

One daemon owns the semantic query graph, CAS, task lifecycle, evidence ledger, and snapshots. CLI, Cargo, IDE, DAP, MCP, SARIF, TUI, and web clients are projections.

### B4 — Explicit handles dominate implicit sessions

Workspace, proof state, debugger branch, task, continuation, crashpack, and synthesis archive are explicit opaque handles. This enables resumability, sharing, isolation, caching, and deterministic handoff.

### B5 — Context is compiled, not dumped

A Context Pack is a property-directed program slice over causal, proof, source, state, and assumption graphs. It contains an omission manifest and expandable references.

### B6 — Counterexamples are causal objects

The primary failure artifact is a minimal or near-minimal causal explanation with abstract/concrete state deltas, obligation flow, missing order, counterfactual repairs, and replay—not a chronological log.

### B7 — Debugging follows the partial order

The debugger steps among enabled semantic events, not merely source lines. It supports reverse causal stepping, alternate schedule branching, and observer-level state views.

### B8 — Repairs are transactions

A patch proposal, hypothesis, semantic diff, replay, neighborhood campaign, proof impact, and promotion decision form one atomic evidence-bearing workflow.

### B9 — Clean verification audits incremental verification

Incremental reuse is essential for interactive work but dangerous if invalidation is unsound. Every reuse edge has a class and is statistically/deterministically audited against clean recomputation.

### B10 — Model/program synchronization is proof-oriented

Bidirectional assistance can propose edits, but ambiguity produces a conflict and obligations rather than a guessed rewrite. Round-trip laws are not enough; semantic preservation matters.

### B11 — Assurance is an envelope, not a badge

Every result describes dimensions such as bounds, faults, fairness, values, schedules, weak-memory model, observer, proof status, and unknowns. “Verified” alone is prohibited in machine output.

Every envelope dimension names its producing engine or carries a typed
`Unsupported` value; a dimension is never silently omitted. Until the
weak-memory lane ships (ADR-0032), every envelope's memory dimension reads
`Unsupported(sequential-consistency-only)`. Until the timed and
probabilistic extensions ship (ADR-0016), Continuum does not advertise
timed or probabilistic proof support in any machine output, and timing
fields in Intent Contracts are declarative assumptions, not checked
semantics.

### B12 — Agents are untrusted search procedures

Agents may propose models, invariants, proofs, rankings, abstractions, patches, and algorithms. Only engines and independent checkers promote evidence states.

### B13 — Multi-agent coordination uses an evidence graph

Conversation is ephemeral coordination. Claims, dependencies, conflicts, counterexamples, patches, proof goals, and receipts are durable typed nodes.

### B14 — Search must preserve behavioral diversity

Forge retains a quality-diversity archive of semantically distinct correct candidates rather than collapsing immediately to one syntactic optimum.

### B15 — Synthesis is constrained by intent and non-vacuity

A synthesized protocol must satisfy safety, progress, implementability, observer obligations, and explicit optimization objectives. Disabling all behavior is not a solution.

### B16 — Verification and synthesis share counterexamples

CEGIS, invariant inference, ranking synthesis, repair, and protocol synthesis consume the same normalized counterexample and proof-obligation artifacts.

### B17 — Proof context is a product

Lean goals are accompanied by relevant declarations, proof slices, failed attempts, countermodels, candidate lemmas, source correspondence, and version identity. Proof workers need not reconstruct context.

### B18 — Interactive results require budget semantics

Every long task accepts wall, CPU, memory, state, solver, token, and proof budgets and returns a resumable continuation with monotonic evidence.

### B19 — Cancellation correctness applies to verification itself

Search, proof, synthesis, indexing, and debugging tasks run under asupersync regions and publish only committed artifacts. Cancellation cannot leave false finality or orphan work.

### B20 — Security boundaries are semantic boundaries

Agent permissions, opaque effects, foreign solvers, proof services, production traces, and domain packs are capability-scoped and visible in receipts.

### B21 — Humans need progressive disclosure, not simplification by omission

The default view says what failed and why. Every simplification has a direct path to exact state, causal events, formulae, proof obligations, and raw artifacts.

### B22 — Benchmarks must test governance, not just bug fixing

ContinuumBench includes attempts to weaken properties, hide events, shrink bounds, overfit traces, exploit stale snapshots, inject prompts through source, and forge evidence.

### B23 — The TLA+ corpus is both language coverage and interaction coverage

Each port includes authoring, checking, explanation, deliberate failure, proof/refinement where applicable, and an agent task—not merely semantic execution.

### B24 — Formal-methods UX requires empirical science

Human diagnosis accuracy, time, confidence calibration, and error patterns are measured. Agent success, token cost, invalid actions, and expensive failures are measured.

### B25 — The final product is an invention accelerator

Once the trust loop is closed, Continuum should search beyond known implementations: new synchronization schemes, storage protocols, replication algorithms, resource schedulers, and cancellation protocols—while emitting proofs and executable Rust candidates.

---

## 2. Constitutional invariants

### INV-001 — Protected intent

Ordinary tasks may not mutate intent. Intent changes require a new Intent Contract, semantic diff, policy decision, and invalidation of dependent evidence.

### INV-002 — No hidden semantic state

Every stateful workflow uses explicit handles. A dropped connection or restarted client does not change meaning.

### INV-003 — No prose-only machine interfaces

Human text may accompany a result, but agents and integrations consume versioned schemas and enums.

### INV-004 — No self-certification

Search code does not check its own strongest claims. Certificates cross an independent checker; foundational theorems cross Lean.

### INV-005 — No ambient nondeterminism

Controlled code accesses scheduling, time, entropy, I/O, faults, and cancellation through explicit capabilities.

### INV-006 — Replay stability

A failure advertised as replayable must reproduce under its pinned semantic epoch or be downgraded to an engine defect.

### INV-007 — Omission transparency

Any bounded Context Pack, explanation, slice, or visualization names what was omitted and how to retrieve it.

### INV-008 — Typed inconclusiveness

Timeout, unsupported semantics, insufficient telemetry, abstraction ambiguity, and incomplete proof search are distinct outcomes.

### INV-009 — Monotonic task evidence

Resuming a task may add evidence or refine an unknown; it may not silently replace prior artifacts under the same identity.

### INV-010 — Incremental parity

An incremental result claiming exactness must match clean evaluation for the same snapshot and semantic epoch.

### INV-011 — Intent-preserving repair

A repair transaction cannot be promoted if it changes protected intent unless explicitly reclassified as an intent revision.

### INV-012 — Non-vacuous synthesis

Forge objectives include required progress/availability behaviors and mutation challenges. Safety by disabling the system is rejected.

### INV-013 — Property-scoped reduction

Independence, symmetry, abstraction, slicing, and quotienting are justified relative to named observers/properties and fairness obligations.

### INV-014 — Version-explicit proof

Lean version, library closure, theorem hashes, axioms, certificate schema, and checker identity are recorded.

### INV-015 — Agent least authority

Agents cannot alter evidence status, sign receipts, access ungranted production traces, or execute unrestricted host effects.

### INV-016 — Source is untrusted data

Comments, logs, docs, model strings, and production payloads cannot issue instructions to the workbench or proof service.

### INV-017 — Semantic atomicity of publication

Artifacts become visible only after their content, provenance, and references are durably committed.

### INV-018 — Corpus honesty

Compatibility claims include unsupported features, configured bounds, expected verdict, state/trace parity, and proof status.

---

## 3. Product experience

### 3.1 Five-minute human path

A developer adds:

```toml
[dev-dependencies]
continuum = "..."

[package.metadata.continuum]
entry = "crate::protocol"
profile = "dev"
intent = "in_7c2f91"          # registry identity, never a file path
intent_bundle = "inb_09aa41"  # §4.2.1: CI fails closed without it
```

Then:

```bash
cargo continuum init
cargo continuum check
```

This Cargo metadata is the single configuration surface for Rust
projects; model-only projects use the same schema in a standalone
`continuum.toml`. Configuration references intent by registry identity
(`in_*`), never by a workspace file path (§4.2).

`cargo continuum init` also proposes draft Intent Contracts from property
templates, the domain-pack library, and observed effect footprints. Drafts
enter the registry at status `Proposed`; they gain protection (INV-001)
only on explicit acceptance. Continuum never silently promotes an inferred
intent to protected status, and never claims a generated model is the
intended abstraction.

First-run contract: while every governing intent is `Proposed`,
`cargo continuum check` runs the drafts and reports each verdict as
`Hypothesis(unaccepted-intent)` — structurally distinct from FAILURE
and PASS in every surface — alongside intent-free findings that need no
contract at all: effect footprint, uncontrolled-nondeterminism sites
(INV-005), and unmapped opaque effects. This output is the specified
zero-configuration value of the tool, not a degraded mode.

The first result is concise:

```text
FAILURE  AckImpliesDurable          intent in_7c2… (unchanged)
assurance  bounded: ≤3 nodes · ≤2 faults · exhaustive schedules · SC memory

Ack became observable before the corresponding write became durable.
Causal core: 4 events · 2 tasks · 1 cancellation
Abstract mismatch: acked +1, durable unchanged
Unknown: production storage profile assumed (contractual)

replay   crash_7m3...
debug    continuum debug crash_7m3...
explain  continuum explain crash_7m3... --level causal
repair   continuum repair begin crash_7m3...
```

No Java installation, model-config archaeology, or megabytes of state dumps.

### 3.2 Agent path

An agent does not invoke arbitrary shell text. It calls:

```json
{
  "operation": "verification.start",
  "idempotency_key": "b1946ac9…",
  "snapshot": "ws_4f...",
  "intent": "in_91...",
  "budget": {"states": 100000, "wall_ms": 30000},
  "arguments": {
    "target": {"kind": "property", "id": "AckImpliesDurable"}
  },
  "output_policy": {"context_pack": true, "max_tokens": 6000}
}
```

The result contains typed verdict, task/continuation handles, evidence roots, and a Context Pack. The agent may request expansion by graph node, source span, proof goal, alternate branch, or assumption.

### 3.3 Design-before-code path

```bash
continuum model new lease-service
continuum model check lease-service --property no-overlapping-valid-leases
continuum forge lease-service --holes quorum,renewal_guard
```

The model is independent of Rust. Later:

```bash
cargo continuum bind lease-service crate::lease
cargo continuum refine --from program:lease --to model:lease-service
```

### 3.4 Repair path

```bash
continuum repair begin crash_7m3...
continuum repair apply --patch patch.diff --hypothesis "publish only after sync"
continuum repair evaluate rt_2c...
continuum repair promote rt_2c...
```

Promotion returns a receipt only after all policy gates close.

### 3.5 Invention path

```bash
continuum forge synthesize models/broadcast.ctm \
  --holes delivery_rule,ack_rule,state_summary \
  --objective latency,persistent-writes \
  --diversity behavior \
  --assurance bounded-proof
```

Forge returns a Pareto/quality-diversity archive, not a single opaque answer.

---

## 4. Authoritative workbench architecture

### 4.1 `continuumd`

`continuumd` is a long-lived local or remote service composed of:

```text
Protocol gateway
Snapshot/CAS service
Intent registry
Incremental semantic database
Task scheduler and budget manager
Engine orchestrator
Evidence graph and receipt ledger
Context compiler
Debugger service
Repair transaction manager
Forge coordinator
Proof/solver isolation gateways
Audit and policy engine
```

It runs its own work as cancel-correct asupersync regions. A task has explicit lifecycle:

```text
Created → Running → Suspended | Completed | Failed | Cancelled
                       │
                       └── continuation + committed partial evidence
```

The scheduler defines priority classes — interactive > CI >
background/swarm — with preemption of suspendable tasks (safe under B18
continuations). Every capability grant carries concurrency and resource
quotas. The daemon operates under a declared memory budget with
class-aware eviction across the incremental cache, overlay snapshots,
and materialized debugger branches. G5 latency compliance is measured
with a saturating background swarm present, not on an idle daemon.

### 4.2 Workspace snapshots

A workspace snapshot contains content identities for:

- source files;
- CML modules;
- Rust semantic extraction;
- domain-pack manifests;
- dependency lockfiles;
- toolchain and semantic epochs;
- the content identity of the governing Intent Contract (a reference, not
  the contract itself);
- generated correspondence;
- proof environment;
- configuration.

Snapshots form a Merkle DAG. A tool call never means “whatever is currently on disk”; it means a named snapshot. Clients may create a snapshot from a working tree, overlay an in-memory editor buffer, or fork an existing snapshot.

Intent Contracts are stored and versioned only in the intent registry,
outside every writable or forkable snapshot. `workspace.fork` preserves the
intent binding by identity; rebinding a snapshot lineage to a different
intent is a privileged operation that produces a semantic intent diff and
invalidates dependent evidence (INV-001).

#### 4.2.1 Intent distribution and convergence

Registries converge through signed, content-addressed **intent bundles**
(`inb_*`): an export of one or more contracts with their acceptance
records and policy tables. Bundles may be vendored in the repository or
fetched by identity; import is idempotent and never changes protection
status — an imported `Proposed` contract stays `Proposed`, and an
imported acceptance is honored only if its signature chain satisfies the
local policy. Divergent branches are reconciled by semantic three-way
merge: each head's §5.3 diff against the common ancestor; two revisions
merge automatically only when both diffs are classified independent
within supported fragments — otherwise the merge is a `Conflict` node
requiring the `revise-intent` capability. CI fails closed when the
bundle referenced by the workspace configuration is absent or its
acceptance chain does not verify, and §17.6's intent diff ships with a
PR-reviewable projection (SARIF note plus rendered semantic diff).

### 4.3 Native protocol

The native protocol uses strongly typed request/response definitions over local IPC or authenticated HTTP/QUIC. Requirements:

- idempotency keys;
- explicit snapshot and intent handles;
- capability negotiation;
- pagination and continuation;
- deterministic ordering;
- structured error taxonomy;
- asynchronous task handles;
- cancellation and budget updates;
- evidence subscriptions;
- OpenTelemetry context;
- protocol and semantic version separation.

The daemon serves protocol majors N and N−1. Evidence and receipt
schemas are readable by every future verifier for their declared schema
epoch: artifact readability is decoupled from protocol majors and
covered by the kernel crates' reproducible-build covenant (§20).

MCP, LSP, DAP, and SARIF translate to this protocol. None define core semantics.

### 4.4 Content-addressed artifacts

Artifact classes include:

```text
ws_* workspace snapshot
in_* intent contract
inb_* signed intent bundle
model_* elaborated model
cir_* causal execution graph
crash_* crashpack
ctx_* Context Pack
proof_* proof artifact
ps_* proof state
task_* task
ev_* evidence node or edge
dbg_* debugger branch
receipt_* signed/checked receipt
rt_* repair transaction
forge_* synthesis archive
cont_* resumable task continuation
cap_* capability
diff_* semantic/intent diff
defect_* engine-defect report
```

Handles carry a kind prefix but are otherwise structureless. Content-
addressed identities are derivable by anyone holding the content — the
§4.5 existence-oracle rule exists because of this — so handles are
identifiers, not secrets, and confer no authority. Capability tokens
(`cap_*`) are minted randomly and do confer authority. Authorization is
always checked independently of handle possession.

### 4.5 Operational contract of `continuumd`

The daemon is part of the trust spine; its operational behavior is
specified, verified, and gated (G1), not left to implementation:

- **Crash safety.** Publication commits content before index; a crash
  leaves unreachable content eligible for GC, never a stale index entry.
  On restart, `Running` tasks resume from their last committed
  continuation or transition to `Failed` with a typed reason — never to a
  silently reconstructed state. An index verifier (fsck) ships with the
  daemon.
- **Storage lifecycle.** Artifacts are garbage-collected by reachability
  from named roots, receipts, and retention policy. The daemon reports
  storage attribution by artifact class. Disk exhaustion during
  publication aborts atomically (INV-017); it never truncates.
  Campaign-class evidence (neighborhood, mutation) is summarizable:
  after promotion, raw per-class results may be rolled up into a
  coverage certificate attested by a kernel-covenant checker; the
  receipt references the summary, raw classes become GC-eligible under
  retention policy, and later access yields
  `Redacted(summarized, commitment)` with the standard §18.4 downgrade.
  Summarization is the default after promotion and opt-out per
  retention policy.
- **Purge without breaking receipts.** Sensitive artifact classes are
  encrypted at rest per artifact; purge shreds the key and replaces
  content with a typed `Redacted(reason, commitment)` stub. Receipts
  referencing purged content remain structurally verifiable and report
  the redaction; claims requiring the hidden data downgrade per §18.4.
- **Durability and restore.** The CAS, evidence ledger, intent registry,
  and audit log support backup and verified restore; restore runs the
  index verifier, and a receipt whose referenced content was lost
  reports `Redacted(lost, commitment)` rather than disappearing. Audit
  logs are exportable as append-only streams.
- **Multi-user baseline.** Remote mode requires an identity model;
  capabilities are minted, scoped, delegated, and revoked through daemon
  operations recorded in the audit log. Cross-user computation sharing is
  off by default: content-addressed dedup across principals is an
  existence oracle and requires an explicit sharing policy.

### 4.6 Semantic epoch advance

Epochs are content identities; advancing one never mutates existing
artifacts (ADR-0018):

- evidence is epoch-scoped: a receipt remains verifiable under its
  pinned epoch indefinitely (INV-006, INV-014); an epoch advance never
  silently revalidates or invalidates a published receipt;
- each advance publishes a typed per-artifact-class compatibility
  statement — `Preserved | Revalidate | Incompatible` — and an estimated
  invalidation blast radius by artifact class before it is applied;
- the daemon may hold at most two epochs concurrently during migration;
  new work defaults to the newest; continuations resume only under
  their pinned epoch (§9.6) and are forked, never migrated in place;
- re-derived artifacts receive new identities linked to their
  predecessors by `SUPERSEDES` edges; nothing is rewritten in place.

### 4.7 Engine-defect lifecycle

Engine defects are first-class evidence. Any `ReplayDiverged`, parity
mismatch, engine crash, or explanation-validation failure emits a
`defect_*` artifact: all inputs pinned by content identity, semantic and
checker epochs, engine identity, and an automatically minimized
reproduction, governed by the same redaction policy as Context Packs
(§18.4). `continuum doctor` assembles a shareable defect bundle. Daemon
health and performance export as OpenTelemetry alongside — never inside
— semantic evidence.

---

## 5. Intent Contract

### 5.1 Why intent must be explicit

Formal tooling creates unusual reward-hacking opportunities. A patch can make verification green by:

- changing `Agreement` to a weaker observer;
- adding a fairness assumption that schedules away the bug;
- reducing node count from five to three;
- removing crash-after-submit;
- marking a storage effect opaque;
- mapping two concrete values to one abstract value;
- lowering assurance from exhaustive to sampled;
- excluding the failing state with a constraint.

These are potentially valid design changes, but they are not ordinary repairs.

### 5.2 Intent structure

An Intent Contract contains:

```text
claims
  property AST + stable semantic name
assumptions
  environment, scheduler, timing, storage, network, trust
  domain-pack fidelity profile per effect family
    (ideal | contractual | platform-qualified | adversarial-envelope)
fault model
  crash, recovery, partition, loss/duplication/delay envelopes
fairness
  weak/strong fairness declarations per action family
completion policy
  (stutter-forever | deadlock-violation | finite-trace-only | closed)
nondeterminism classes per choice site
  (demonic | angelic | scheduler | probabilistic† | timed† | epistemic†)
  († declarative until ADR-0016 lanes ship — see B11)
observers
  state/event/knowledge/security projections
abstraction maps
  content identities of the §16 correspondence/abstraction maps the
  claims are stated against; merge/split changes are privileged (§5.3)
scope
  model/program components, abstraction level, and semantic fragment
  declarations (ADR-0025)
trust boundaries
  trusted and opaque components and effects
bounds
  values, nodes, faults, depth
assurance policy
  accepted evidence classes and required checkers
optimization
  hard constraints, soft objectives, non-vacuity
security policy
  data classification, redaction classes, and capability requirements
  (first-class per §0.1; resolves RFC 0037's open question)
change policy
  who/what may modify each field
```

### 5.3 Semantic intent diff

A semantic diff is not text diff. It classifies:

- strengthened/weakened property;
- strengthened/weakened assumption;
- observer refinement/coarsening;
- bound increase/decrease;
- fault envelope expansion/contraction;
- fairness addition/removal;
- abstraction merge/split;
- proof policy upgrade/downgrade (represented in the diff lattice as an
  `assurance` change per RFC 0031);
- unsupported semantic change;
- unchanged intent.

Where implication is decidable or solver-checkable, Continuum proves the
direction. Otherwise it emits a proof obligation, `unknown`
(undecidable or unattempted), or `unsupported` (outside declared
fragments) rather than guessing — the two are distinct per INV-008.
All non-affirmative classifications fail closed: an intent change
classified `unknown`, `unsupported`, or `incomparable` on a protected
field blocks ordinary promotion exactly as a confirmed privileged
change does, pending review. The semantic-diff schema enforces this
structurally, not only in prose.

Each intent field declares its semantic fragment (ADR-0025:
`Finite / Symbolic / Temporal / Probabilistic / Theorem / Runtime`), so
"supported" is a checkable predicate. The diff guarantee is: within
declared supported fragments, every property weakening, assumption
strengthening, bound decrease, observer coarsening, fault removal,
fairness addition or removal, and assurance downgrade is classified as
privileged — the seven G3 dimensions; outside them, `unsupported`.

### 5.4 Intent locks

CI policy can lock fields:

```toml
[intent.policy]
properties = "review"
assumptions = "review"
fairness = "review"
trust_boundaries = "no-expansion"
non_vacuity = "no-removal"
completion_policy = "review"
bounds = "no-decrease"
faults = "no-removal"
assurance = "no-downgrade"
observers = "review"   # a proof-gated verb requires an RFC 0037 revision
security_policy = "review"
optimization = "review"
scope = "review"
nondeterminism = "review"
abstraction_maps = "review"
```

Verbs are drawn from RFC 0037's closed set; free-form policy strings are
rejected. `scope`, `nondeterminism`, and `abstraction_maps` are
protected: RFC 0031 classifies any change to them as a semantic change,
so each must be lockable.

Agent repair capabilities exclude intent mutation unless a task explicitly asks for redesign.

`intent.accept` is the only transition from `Proposed` to protected
status; it requires the `revise-intent` capability, produces an audit
record, and is never performed implicitly by `init`, `check`, or any
repair operation. `intent.lock` edits the §5.4 policy table under the
same capability.

---

## 6. Context Packs

### 6.1 Definition

A Context Pack is a bounded, typed, property-directed compilation of the evidence graph for a human or agent task.

It is not a generic summary. It is produced by a query such as:

```text
Why did AckImpliesDurable fail?
What code could affect the missing order?
What proof obligations block promotion?
Which assumptions distinguish this from the safe branch?
```

### 6.2 Contents

A failure Context Pack includes:

- target intent and property;
- exact verdict and assurance envelope;
- causal core and surrounding frontier;
- concrete and abstract state deltas;
- obligation/resource flow;
- missing or reversed order constraints;
- relevant source spans and model actions;
- proof/refinement slice;
- assumptions used and unused;
- counterfactual safe branches;
- candidate repair surfaces, clearly marked heuristic;
- exact replay and debugger handles;
- omitted-node counts and expansion queries;
- schema, semantic epoch, and content hash.

### 6.3 Context compiler

The compiler combines:

- backward causal slicing;
- property automaton relevance;
- dynamic and static dependence;
- proof dependency slicing;
- observer projection;
- abstraction/refinement correspondence;
- minimal unsatisfied core or correction-set analysis;
- information-gain ranking;
- token/byte budget optimization.

The core slice must be replay-preserving when claimed. Additional explanatory items may be heuristic but are labeled.

### 6.4 Views

The same pack can render as:

- concise terminal explanation;
- JSON/CBOR agent artifact;
- IDE diagnostics and code lenses;
- causal graph;
- debugger initial state;
- proof-worker task;
- repair transaction seed.

Presentation never mutates the underlying artifact.

---

## 7. Verification debugger

### 7.1 Debugger model

Traditional debuggers step through one execution. Continuum debugs a space of executions.

The debugger state contains:

```text
selected configuration
causal past
currently enabled event frontier
property-monitor state
abstract views
concrete program state
obligation/region tree
fault and time state
unexplored alternatives
```

State materialization is deterministic re-execution from periodic
committed checkpoints; the checkpoint interval is budget-tunable.
Reverse-step and branch-fork carry docs/34 latency rows. Concurrently
materialized branches per session are capped with LRU spill to the CAS.
Branch comparison operates on projected observer state unless full
concrete state is explicitly expanded.

### 7.2 Operations

- step one semantic event;
- step one abstract transition;
- run until property-relevant event;
- reverse to causal predecessor;
- jump to last change of a value/resource;
- branch on a different enabled event;
- inject/remove a fault at a legal frontier;
- compare two branches;
- inspect why an event is enabled or blocked;
- show fairness debt and liveness rank;
- project to selected observer;
- export branch as a crashpack or regression scenario.

### 7.3 DAP projection

A DAP adapter maps:

- semantic owners/tasks/nodes to threads;
- abstraction/refinement frames to stack frames;
- states, obligations, messages, and enabled events to variables;
- source/model labels to breakpoints;
- causal branches to restart/step targets;
- stable debugger object handles to DAP variable references.

DAP enables broad IDE support, but advanced partial-order operations remain native custom requests.

### 7.4 Why-enabled explanations

Every action should expose a derivation:

```text
ReceiveVote(n2,n1) enabled because:
  message m17 is pending and deliverable
  n1 is running
  current partition permits n2 → n1
  cancellation phase is Running
  guard term(m17) ≥ current_term(n1)
```

Disabled actions expose the smallest blocking conditions where feasible.

---

## 8. Repair transactions

### 8.1 Transaction contents

A Repair Transaction is an immutable proposal plus accumulating evidence:

```text
base snapshot and intent
failure/crashpack
repair hypothesis
patch/model/proof changes
semantic diff
intent diff
exact replay result
neighboring exploration result
mutation challenge result
proof/refinement impact
performance impact
security review
promotion policy verdict
```

Each evaluation produces a new transaction version. The original proposal remains addressable.

### 8.2 Required gates

Default promotion requires:

1. The original failure replays on the base snapshot.
2. The patch applies to the declared snapshot with no hidden edits.
3. Protected intent is unchanged, or the transaction is explicitly reclassified.
4. The exact failure no longer occurs.
5. Neighboring schedules, faults, and values are explored.
6. Property mutations still fail where expected.
7. Known defect mutants remain detected.
8. Refinement coverage is not reduced unexpectedly.
9. Invalidated certificates/proofs are rebuilt.
10. Clean and incremental results agree.
11. Required code tests, static verification, and security gates pass.
12. A promotion receipt is generated.

### 8.3 Neighboring exploration

Overfitting is combated by constructing a semantic neighborhood:

- alternate enabled events at each causal decision;
- fault insertion/removal around the repaired window;
- cancellation at adjacent checkpoints;
- equivalent value/name permutations;
- changed message duplication/loss/delay;
- schedule perturbations in the same trace class boundary;
- generated variants from the abstraction map;
- hidden corpus-style mutations.

The neighborhood is property-directed and budgeted. Coverage appears in the receipt.

### 8.4 Counterfactual repairs

Continuum can evaluate proposed interventions on a failure graph:

```text
add Sync → Ack order
move publication after Commit
abort reply obligation on cancellation
make epoch check atomic with write
```

Counterfactual success does not prove a source patch correct. It prioritizes repair surfaces and produces explicit hypotheses for agents/humans.

### 8.5 Review UX

A reviewer sees:

```text
Intent: unchanged
Original failure: eliminated
Causal neighborhood: 38,412 classes explored
New failures: none in bounded envelope
Proof impact: 2 receipts rebuilt, 1 Lean theorem reused
Coverage: +4.3%
Cost: p99 verification +1.8%
Unresolved: production storage profile remains assumed
```

The exact artifacts are one click/handle away.

### 8.6 Cost governance

Every repair transaction carries a cumulative cost ledger (CPU, wall,
solver, memory, token) across all its versions; the promotion receipt
includes it. Gate 5–7 evaluations are incremental by default:
neighborhood classes and mutants whose causal footprint is disjoint from
the patch delta (a Conservative-class §9 dependency query) reuse prior
results as `Validated` edges; anything else re-runs. The §9.5 sampling
rate derives from a declared statistical confidence target for mismatch
detection per reuse class, reviewed at G5. The daemon enforces
per-principal and per-transaction cost ceilings; exceeding one yields
`BudgetExhausted` with a continuation — never a silently smaller
campaign (the cost-domain form of INV-007).

---

## 9. Incremental semantic database

### 9.1 Need

Impeccable DX requires subsecond feedback for local edits and resumable deeper searches. Re-running every model, extraction, exploration, proof, and explanation from scratch would make Continuum irrelevant in normal development.

The engine itself is engineering risk, not settled design: whether it is
derived from an existing memoization framework or built custom is decided
by a Phase B ADR with spike evidence (§21), and the decision is carried
as docs/08 risk R21, whose failure mode is edge classes collapsing to
Conservative (destroying interactivity), whose controls are that ADR's
spike plus the Incremental Parity Audit, and whose kill signal —
Conservative-collapse on the reference workload — is carried in §24.

### 9.2 Query model

All derived artifacts are memoized queries over immutable inputs:

```text
parse(file)
elaborate(module, imports, semantic_epoch)
extract_rust(crate_snapshot, annotations)
build_model(config, intent)
property_automaton(property)
abstract_state(program_state, map)
explore(model, intent, strategy, budget)
check_certificate(certificate, checker_epoch)
compile_context(evidence_root, query, budget)
```

A query key includes semantic configuration; a source hash alone is insufficient.

### 9.3 Dependency edge classes

- **Exact:** output is a pure function of named inputs; safe to reuse by content identity.
- **Validated:** translation/reduction output carries a checker witness.
- **Conservative:** invalidation may over-approximate dependencies but cannot miss changes under stated assumptions.
- **Experimental:** reuse may improve speed but result cannot support strong finality until clean validation.

### 9.4 Semantic dependencies

Dependencies distinguish:

- reads type vs value;
- unfolds definition;
- selects instance/domain profile;
- observes event family;
- relies on assumption/fairness/bound;
- uses abstraction component;
- depends on proof lemma/certificate checker;
- consumes solver encoding epoch;
- consumes source correspondence.

This enables precise invalidation and useful “why did this re-run?” explanations.

### 9.5 Incremental Parity Audit

(Renamed from “clean-build Tribunal”; **Tribunal** refers exclusively to
the TLA+ corpus oracle harness of ADR-0021.)

CI and sampled local runs compare incremental and clean artifacts:

- verdict;
- canonical state graph or digest;
- counterexample class;
- certificate result;
- context slice soundness;
- semantic diff;
- proof axiom manifest.

Queries carry an auditability class, declared on the query definition:

- **Equality-auditable** — deterministic under the docs/19 matrix;
  compared bit-for-bit. Any disagreement quarantines the reuse class and
  emits a minimal invalidation counterexample.
- **Certificate-auditable** — solver-backed; the audit compares checked
  certificates and claim envelopes, never raw solver behavior. A
  certificate-level disagreement quarantines; a solver-outcome
  difference with agreeing certificates does not.
- **Budget-sensitive** — anytime results; the audit checks only that the
  incremental result's evidence labels are no stronger than a clean
  run's under equal budget (monotone-honesty), and records divergence as
  drift telemetry without quarantine.

Divergence attributable solely to budget or portfolio nondeterminism
never quarantines a reuse class; divergence in an equality- or
certificate-auditable query always does. INV-010's exactness claim
applies per auditability class. The class is declared on the query
definition, not the run, so a mismatch cannot be reclassified away
after the fact.

### 9.6 Long-task continuations

Exploration, proof, and synthesis continuations contain committed frontier/search state and identity of every input. Resume rejects mismatched snapshots or epochs. Forking a continuation with a new budget is explicit.

---

## 10. Agent-native protocol

### 10.1 Design principles

The Agent–Computer Interface follows these rules:

- small orthogonal operations;
- typed arguments and results;
- stable handles instead of cursor positions;
- deterministic ordering;
- concise default payloads;
- explicit expansion;
- error messages with recovery actions;
- idempotency;
- budget and cancellation controls;
- no need to parse terminal prose;
- no implicit authority escalation;
- source text treated as data, not instructions.

### 10.2 Core operations

```text
workspace.create / fork / diff / seal
intent.get / diff / propose_revision / accept / reject / lock
verification.start / result / await
model.check / explore / compare
program.extract / run / replay
refinement.check / explain
proof.goal / attempt / check / slice
correspondence.bind / status / drift
debug.open / state / enabled / step_event / step_abstract / reverse_causal /
  branch / compare / why_enabled / why_blocked / export
context.compile / expand
failure.explain / minimize / branch
repair.begin / apply / attach / evaluate / resume / review / promote / reject
observe.ingest / classify / result
forge.create / step / archive / materialize
benchmark.run
task.status / cancel / resume / subscribe / update_budget
evidence.get / query / verify / subscribe
query.explain_reuse / explain_invalidation / clean_compare
```

### 10.3 Error taxonomy

Errors are actionable and typed:

```text
StaleSnapshot
UnsupportedSemanticFeature
IntentMutationDenied
InsufficientEvidence
BudgetExhausted
ContinuationEpochMismatch
AmbiguousCorrespondence
UntrustedDomainBoundary
CertificateRejected
ReplayDiverged
CapabilityDenied
PolicyGateFailed
AcceptanceChainInvalid
StatusConflict
QuotaExhausted
EpochUnsupported
PublicationAborted
ProtocolVersionUnsupported   (protocol-level, RFC 0026)
IdempotencyKeyReused         (protocol-level, RFC 0026)
MalformedRequest             (protocol-level, RFC 0026)
```

Each error may include safe recovery operations, but never free-form commands with untrusted interpolation.

### 10.4 MCP adapter

MCP exposes a curated subset of native operations. Explicit handles are threaded through tool calls. The adapter:

- validates auth independently of handle possession;
- bounds result sizes;
- exposes schemas and deterministic tool ordering;
- avoids session-scoped semantic state;
- maps resources to immutable artifacts;
- attaches trace context;
- supports client-independent task handoff.

No MCP prompt or resource may mutate evidence status.

### 10.5 Agent handoff

A coordinator can give separate agents:

```text
shared: snapshot, intent, evidence graph
isolated: proof search state, debugger branch, Forge candidate pool
```

This explicit sharing model prevents one giant session boundary from either over-sharing or isolating the wrong state.

---

## 11. Multi-agent Evidence Graph

### 11.1 Why chat is insufficient

Agent conversations are lossy, expensive, difficult to validate, and prone to persuasive but unsupported conclusions. Continuum stores durable work as a graph.

### 11.2 Node types

```text
IntentClaim
Assumption
Property
ModelVersion
ProgramSnapshot
ProofGoal
InvariantCandidate
RankingCandidate
AbstractionMap
SynthesisCandidate
Counterexample
CausalExplanation
RepairHypothesis
Patch
VerificationRun
Certificate
ProofReceipt
BenchmarkResult
Conflict
ReviewDecision
```

### 11.3 Edge types

```text
SUPPORTS
REFUTES
DEPENDS_ON
REFINES
EXPLAINS
REPAIRS
INVALIDATES
GENERALIZES
COUNTEREXAMPLE_TO
CHECKED_BY
DERIVED_FROM
CONFLICTS_WITH
SUPERSEDES
```

Edges name checker/evidence when applicable.

### 11.4 Status lattice

```text
Proposed
Observed
Sampled
Bounded
Validated
Proved
Refuted
Inconclusive
Superseded
```

Only trusted services can promote into `Validated` or `Proved`. Agent votes or confidence cannot.

`Inconclusive` carries a typed reason per INV-008 (`Unsupported`,
`ResourceExhausted`, `EngineError`, `InsufficientTelemetry`,
`AbstractionAmbiguity`, `IncompleteProofSearch`). `Validated` records
whether solver evidence is `CHECKED_CERTIFICATE` or `TRUSTED_SOLVER`; the
two never render identically. Parameterized results carry
`checked(N=k)` / `cutoff_checked(N≤k)` / `proved(∀N)` and are structurally
distinct in every surface. Budget exhaustion is never a verdict.

### 11.5 Whiteboard compiler

For complex invention tasks, agents may use a structured whiteboard:

```text
Goal
Known facts
Candidate invariant
Counterexample
Unresolved obligation
Experiment
Decision
```

The compiler turns whiteboard entries into typed graph proposals, rejecting references to nonexistent artifacts or unsupported status claims.

### 11.6 Swarm roles

Recommended role decomposition:

- architect/modeler;
- implementation analyst;
- counterexample diagnostician;
- invariant/ranking synthesizer;
- Lean proof worker;
- repair worker;
- adversarial intent reviewer;
- benchmark/mutation worker;
- evidence integrator.

Roles are capabilities and task views, not separate truth domains.

### 11.7 Write model

The evidence graph is append-only. Publication is per-artifact atomic
(INV-017) and linearized per claim identity; status promotion is a
compare-and-set against the claim's current status, so racing
promotions cannot regress the lattice. Concurrent contradictory claims
materialize a `Conflict` node rather than resolving by write order.
Idempotency keys make agent retries safe. The concurrency section of
RFC 0038 is specification debt (§25); the evidence-graph spike
validated the authority table, not concurrent enforcement (§25).

---

## 12. Explanation engine

### 12.1 Explanation levels

1. **Outcome:** what property failed and assurance scope.
2. **Causal:** minimal relevant events and order.
3. **Semantic:** abstract/concrete state mismatch and obligations.
4. **Source:** code/model spans and effect boundaries.
5. **Logical:** formula, proof obligation, assumptions, fairness.
6. **Exhaustive:** raw graph/certificate/solver artifacts.

Users can move up or down without rerunning verification.

### 12.2 Explanation objects

An explanation may include:

- actual cause candidates;
- necessary and sufficient causal sets under a stated model;
- minimal unsatisfied cores;
- minimal correction sets;
- proof slices;
- event/state delta slices;
- contrastive explanation: “why failing branch rather than safe branch?”;
- counterfactual interventions;
- uncertainty and incompleteness.

Causal terminology must be precise. “Cause” is not used when only correlation or relevance was computed.

### 12.3 Contrastive example

```text
Why did this run acknowledge before durability while the sibling run did not?

Only differing relevant choice:
  failing: CancelRequested won before SyncCompleted
  safe:    SyncCompleted won before CancelRequested

The finalizer published the reserved reply in both branches.
The property therefore depends on finalizer behavior, not network order.
```

### 12.4 Explanation validation

Explanations are tested with:

- replay preservation;
- removal/addition checks;
- counterfactual execution;
- independent property monitor;
- expert review;
- human task studies;
- agent diagnosis benchmarks.

A concise explanation that omits the actual defect is worse than a long trace.

---

## 13. Human developer experience

### 13.1 Progressive disclosure

Continuum supports four default personas without separate semantics:

- Rust developer: source diagnostics, replay, causal debugger.
- Protocol designer: CML, state graphs, temporal properties.
- Formal-methods expert: formulas, reductions, certificates, Lean.
- Reviewer/operator: intent/assurance diff, production evidence, receipts.

### 13.2 Diagnostics

Diagnostics should answer:

```text
What happened?
Why does it violate intent?
Where is the smallest relevant code/model region?
How certain is the claim?
What was explored or proved?
What remains unknown?
What is the next useful action?
```

### 13.3 IDE

The LSP provides:

- syntax/type/effect/fragment diagnostics;
- semantic hover for actions/properties/assumptions;
- go-to correspondence among model, Rust, proof, and evidence;
- inlay indicators for observer and effect footprints;
- proof/verification code lenses;
- semantic rename and impact preview;
- live bounded checks;
- context-aware completion for model constructs;
- intent-change warnings.

### 13.4 CLI

Human CLI output is stable, terse by default, and expandable:

```bash
continuum check --summary
continuum check --json
continuum explain crash_x --level semantic
continuum evidence show receipt_x
```

Exit codes and JSON schemas are documented. Color is never semantically required.

### 13.5 Learning path

The learning sequence mirrors the TLA+ corpus but uses one environment:

1. finite puzzles and reachability;
2. concurrency/deadlock;
3. invariants and counterexamples;
4. fairness/liveness;
5. symmetry and refinement;
6. real asupersync code;
7. storage/crash/cancellation;
8. production evidence;
9. proof and synthesis.

Every tutorial includes a defect, explanation, repair transaction, and evidence receipt.

### 13.6 Usability metrics

Measure:

- time to first model and first found bug;
- diagnosis accuracy;
- repair correctness;
- assurance calibration;
- number of context expansions;
- abandoned tasks;
- false confidence;
- successful transfer to a new protocol;
- expert vs non-expert divergence.

---

## 14. Continuum Forge

### 14.1 Purpose

Forge is the invention engine. It searches for new algorithms and proof artifacts while Continuum enforces intent and evidence.

Forge can synthesize:

- guards and state updates;
- message/acknowledgement rules;
- quorum systems;
- retry and cancellation protocols;
- recovery transitions;
- auxiliary/ghost state;
- invariants and lemmas;
- ranking functions and fairness obligations;
- abstraction maps;
- domain-pack contracts;
- implementation skeletons;
- optimization policies.

### 14.2 Forge problem

```text
fixed:
  Intent Contract
  semantic fragment
  implementation/effect constraints
  hard safety/liveness/refinement obligations
  assurance requirement
holes:
  typed grammar or sketch locations
objectives:
  latency, messages, durable writes, memory, simplicity, proof size
archive:
  behaviorally distinct correct candidates
```

### 14.3 Search portfolio

- enumerative and constraint-based synthesis;
- CEGIS with exact/generalized counterexamples;
- interpretation reduction;
- SyGuS/SMT/CHC;
- IC3/PDR-guided invariant synthesis;
- game solving and assumption synthesis;
- stochastic/evolutionary search;
- quality-diversity algorithms;
- LLM proposal generation;
- theorem/proof retrieval;
- superoptimization over finite semantic fragments.

No engine is trusted. Candidates flow through the same verifier and proof pipeline.

### 14.4 Co-synthesis

Protocol synthesis often fails if algorithm, invariant, abstraction, and ranking are searched separately. Forge maintains a coupled candidate:

```text
Candidate =
  algorithm
  + inductive invariant
  + refinement map
  + liveness ranking/fairness argument
  + implementation correspondence
  + cost vector
```

Counterexamples are classified against the component that failed:

- unsafe algorithm;
- non-inductive invariant;
- invalid abstraction;
- liveness cycle;
- unrealizable environment;
- implementation mismatch.

### 14.5 Non-vacuity and anti-gaming

Every synthesis task includes positive behaviors or progress scenarios, such as:

```text
A request must be acknowledged in a failure-free fair run.
At least one write must be admitted when capacity exists.
Leadership must remain possible after recovery.
```

Property mutation and hidden semantic variants detect overfitting.

### 14.6 Quality-diversity archive

Candidates are indexed by behavioral descriptors:

- quorum geometry;
- message phase count;
- stable-write count;
- concurrency width;
- recovery mechanism;
- cancellation structure;
- fairness dependence;
- proof complexity.

A diverse archive is useful for discovering qualitatively new algorithms and for agent training.

### 14.7 Materialization

A promoted candidate can emit:

- CML model;
- executable asupersync Rust skeleton;
- correspondence map;
- generated tests and crash campaigns;
- proof obligations and Lean files;
- benchmark configuration;
- evidence receipt.

Generated code is never presented as production-ready without domain-pack and implementation verification.

---

## 15. Proof service and Lean integration

### 15.1 Proof service

Proof work runs in isolated workers keyed by:

- Lean version;
- Mathlib/dependency closure;
- source snapshot;
- options and resource policy;
- target declaration.

The service supports:

- strict proof checking;
- goal extraction;
- declaration/axiom metadata;
- proof-state stepping;
- proof repair attempts;
- lemma extraction and minimization;
- deterministic source transformations;
- proof receipts.

### 15.2 Proof Context Pack

A proof worker receives:

- exact goal and local context;
- relevant definitions and theorem statements;
- proof dependency slice;
- failed attempts and diagnostics;
- finite countermodels where meaningful;
- analogous corpus proofs;
- allowed tactics/axioms;
- version identity and budget.

It does not receive a giant repository dump by default.

### 15.3 Lean theorem program for Revision 3

Formalize:

1. Intent refinement/order for supported property fragments.
2. Soundness of semantic-diff classifications where decidable.
3. Context-slice preservation claims for finite causal graphs.
4. Repair acceptance implies elimination of the named witness under exact replay.
5. Incremental query equality under valid dependency closure.
6. CEGIS candidate acceptance for finite synthesis domains.
7. Composition of evidence/receipts.
8. Lens/conflict laws for proof-oriented correspondence.

Theorems remain small and foundational. High-performance engines emit certificates rather than being verified wholesale first.

### 15.4 Proof repair

Automated repair may use retrieved lemmas, compiler feedback, proof sketches, tactic search, and agent planning. Acceptance is kernel checking under the pinned environment. Changed axioms or theorem statements are intent/semantic changes, not proof repair.

---

## 16. Model/program correspondence

### 16.1 Correspondence graph

Rather than one abstraction function hidden in code, Continuum stores typed links:

```text
Rust type/field/event/effect
  ↔ operational model component
  ↔ abstract model component
  ↔ property observer
  ↔ Lean definition/theorem
```

Links may be generated, inferred, or handwritten, but status and evidence differ.

### 16.2 Proof-oriented lenses

A correspondence has:

- `get`: concrete → abstract projection;
- optional `put`: proposed abstract edit → concrete candidate edits;
- complement/provenance needed for round trip;
- consistency relation;
- ambiguity conditions;
- generated proof obligations;
- effects and unsupported cases.

When multiple concrete repairs satisfy an abstract change, Continuum returns alternatives or a conflict. It does not choose silently.

### 16.3 Drift detection

Changes trigger:

- unmapped source effects;
- changed state projection;
- action split/merge;
- observer loss;
- new stuttering class;
- proof invalidation;
- semantic equivalence check where feasible.

The IDE can show “model correspondence stale” before deep verification.

---

## 17. Protocol adapters and ecosystem

### 17.1 LSP

Use standard LSP for editor-neutral authoring. Custom extensions refer only to stable handles and native operations.

### 17.2 DAP

Use DAP for broad debugger UI compatibility while preserving native partial-order controls.

### 17.3 SARIF

Export source-located, deduplicated verification diagnostics with artifact URIs, stable rule IDs, severity, code flows, fixes where safe, and evidence handles. SARIF is a report projection, not a proof format.

### 17.4 MCP

Expose agent operations and immutable resources. Explicit state handles enable multi-agent sharing and resumability. MCP does not define transaction semantics or authorization.

### 17.5 Cargo and test harness

```bash
cargo continuum check
cargo continuum explore
cargo continuum replay crash_x
cargo continuum review --base main
```

Rust unit/property tests can call an embedded client, but authoritative artifacts still come from the daemon/core library contract.

### 17.6 CI

CI produces:

- intent/semantic diff;
- verification matrix;
- changed evidence graph;
- invalidated/rebuilt receipts;
- benchmark deltas;
- SARIF;
- promotion recommendation with explicit unknowns.

---

## 18. Security architecture

### 18.1 Threats

- prompt injection through source/comments/logs;
- forged handles or evidence;
- unauthorized intent changes;
- solver/proof worker escape;
- denial of service via state explosion;
- secret exfiltration through context packs;
- production trace privacy leakage;
- dependency/toolchain substitution;
- malicious domain packs;
- replay artifact tampering;
- agent collusion/reward hacking.

### 18.2 Capabilities

Agents receive explicit capabilities for:

- reading selected snapshots/artifacts;
- proposing patches;
- starting bounded tasks;
- requesting proof work;
- expanding context;
- creating repair transactions.

They do not receive evidence-signing, intent-policy mutation, arbitrary network, or host execution by default.

### 18.3 Sandboxing

Foreign solvers, Lean workers, generated code, corpus oracles, and agent code run in isolated, resource-bounded environments with pinned images/toolchains and read-only inputs. Output is parsed as untrusted data.

Domain packs are content-addressed, signed, and pinned in the workspace
snapshot; the Intent Contract's fidelity profile binds the exact pack
identity, so pack substitution is a privileged intent diff (G3), not an
environment change. Third-party packs default to `adversarial-envelope`
fidelity until they pass docs/17 conformance qualification, and receipts
render pack provenance (first-party | qualified | unqualified)
distinctly.

### 18.4 Context privacy

Context compilation enforces source/trace field policy before slicing. Redaction is represented in the omission manifest. A redacted pack cannot support claims requiring hidden data unless a trusted checker provides a separate receipt.

Capture-time contract for production traces: payloads are recorded as
salted commitments by default; raw payload capture is per-field opt-in,
tagged with a data classification at ingestion, encrypted per artifact
on entry (enabling §4.5 purge), and subject to a declared retention
clock. Remote workers receive only post-redaction minimized artifacts;
residency constraints are a capability property of the worker pool.

### 18.5 Audit

Every privileged operation records actor, capability, inputs, policy decision, outputs, and evidence identity. Audit logs are append-only and separate from semantic events.

### 18.6 Signing identities

Receipts, intent bundles, and domain packs are signed. The signing
lifecycle is specified in docs/09: identities are minted through audited
daemon operations (the solo-developer default is a local keypair minted
on first use and recorded in the audit log); organizational deployments
pin an allowed-signers set distributed inside the intent bundle;
rotation and revocation are audited operations; a signature that cannot
be verified downgrades the artifact to typed unverified provenance
rather than failing open — except the §4.2.1 CI acceptance check, which
fails closed by policy.

---

## 19. ContinuumBench

### 19.1 Purpose

ContinuumBench measures whether the system actually enables trustworthy agent/human work.

### 19.2 Task families

- translate prose/TLA+ into CML;
- diagnose finite safety failure;
- diagnose liveness/fairness failure;
- repair concurrent Rust;
- repair model/proof correspondence;
- infer invariant;
- synthesize ranking function;
- prove/refine in Lean;
- port a TLA+ example;
- synthesize protocol holes;
- classify insufficient production evidence;
- review semantic/intent diff;
- resist intent gaming and prompt injection;
- manage stale snapshots/continuations;
- optimize correct candidates.

### 19.3 Metrics

```text
correctness / proof status
intent integrity
generalization to hidden variants
counterexample quality
repair minimality and robustness
human diagnosis time/accuracy
agent token and tool-call cost
wall/CPU/memory cost
invalid action rate
expensive failure rate
context compression and expansions
assurance calibration
security-policy compliance
behavioral novelty
```

Grading is ordered per RFC 0034: intent integrity → security → semantic
correctness → evidence validity → hidden-variant generalization → cost →
explanation. A zero in intent integrity caps the total score at failure.
Leaderboards are Pareto fronts; no single ranking hides cost or assurance.

### 19.4 Dataset construction

Sources include:

- 80 validated TLA+ example families;
- deliberate and generated mutations;
- historical concurrency bugs;
- Continuum domain packs;
- asupersync cancellation/obligation cases;
- proof and refinement tasks;
- synthesized hidden variants;
- real project migrations;
- Continuum engine-defect artifacts (§4.7).

Train/dev/test partitions isolate semantic families and source hashes to reduce leakage.

Gaming mutations are split into a development suite — used by G3's
Phase B gate and disclosed to implementers — and a held-out suite,
partitioned by semantic family per this section's rule. A named subset
of corpus families and the held-out gaming suite are excluded from all
development, tuning, and regression use and graded only by the isolated
ContinuumBench grader. G3 cites the development suite; G9's “80
families at declared parity” is measured on the development set plus a
final, single evaluation of the held-out set and held-out suite.

### 19.5 Reward-hacking suite

Every benchmark candidate is tested against attempts to:

- weaken property;
- add assumptions;
- lower bounds;
- remove faults;
- hide observer events;
- lower assurance class;
- substitute a trivially true property (vacuity);
- disable or bypass instrumentation (docs/50 instrumentation attacks);
- exhaust or misdirect budgets (docs/50 resource attacks);
- pass a semantically equivalent distractor patch (grader control);
- return unsupported as pass;
- hard-code known trace;
- exploit stale cache;
- forge receipt/status;
- smuggle instructions through source.

Intent integrity is a prerequisite, not a bonus metric.

---

## 20. Crate and service architecture

```text
continuum-workspace
continuum-intent
continuum-value
continuum-model-core
continuum-cml-syntax
continuum-cml-elab
continuum-cir
continuum-observer
continuum-refinement
continuum-certificate
continuum-kernel-core
continuum-kernel-sat
continuum-kernel-smt
continuum-kernel-temporal
continuum-evidence
continuum-context
continuum-semantic-diff
continuum-repair
continuum-incremental
continuum-task
continuum-debugger
continuum-forge
continuum-benchmark
continuum-security
continuum-asupersync
continuum-effects-{network,storage,time,process}
continuum-engine-reference
continuum-engine-explicit
continuum-engine-dpor
continuum-engine-symbolic
continuum-engine-liveness
continuum-proof-client
continuum-corpus
continuumd
continuum-cli
continuum-lsp
continuum-dap
continuum-mcp
continuum-sarif
```

Dependency rules:

- proof/certificate checker does not depend on search engines;
- the `continuum-kernel-*` crates form the trusted checking base: no shared
  optimized evaluator code with any engine, no async, no unsafe, no plugins
  or dynamic loading, a <15,000 non-test-line covenant (docs/03), and a
  serialization boundary between every engine and the kernel — a
  certificate is checked from its wire form, never from shared memory;
- in certified lanes, content identity is exact canonical identity; hash
  collisions are resolved by canonical comparison; 256-bit-hash identity is
  permitted only in explicitly labeled non-certified modes (ADR-0013);
- model core does not depend on asupersync;
- adapters do not own semantic state;
- Forge depends on verifier interfaces, never vice versa;
- UI crates cannot mutate evidence directly;
- foreign tools remain isolated behind normalized artifacts;
- kernel crates build reproducibly from pinned sources; every receipt
  records the checker's build digest and toolchain identity (INV-014),
  so checker identity is independently re-derivable.

---

## 21. Implementation program

### 21.1 Swarm execution, licensing, and study posture

Phases are gate-driven, not time-driven. Implementation is performed by
an elastic swarm of autonomous agents; no named owner or minimum human
team is a prerequisite for starting a phase or merging its opening PR.
The Bones dependency graph is the scheduling control plane: an agent may
claim dependency-ready work, independent ready bones may execute in
parallel, and conflicting changes serialize at integration. Completion
still requires the bone's specified evidence, independent checking where
required, and explicit exercise of any privileged authority. Swarm scale
never weakens an intent, assurance, security, or release gate.

The phase execution map lives in
`notes/START_HERE_IMPLEMENTATION.md`. It records each opening PR and
dispatch condition, not staffing. A phase opens when its predecessor
exit decision and its explicit Bones prerequisites hold; its opening-PR
designation is an ordering marker rather than a resourcing merge
requirement.
The PR sequence in START_HERE interleaves phases by design: a phase
closes when its assigned gates close, regardless of PR ordinal, and a
PR advancing an earlier phase's gate after that gate has closed is
hardening — it attaches regression evidence to the closed gate without
reopening it. A gate's criteria remain permanent regressions after
closure (§22 release-blocker doctrine).
Phase A additionally resolves: the product license (permissive, compatible
with asupersync and solver adapters); a per-family redistribution audit for
the corpus before any public benchmark release; and the rule that foreign
oracle tooling (TLC, Apalache, solvers) never ships in release binaries
(ADR-0029). The G8 usability gate is defined against a preregistered
minimal study: the three human-executed docs/34 acceptance workflows
(new model; existing Rust system; review), two cohorts (Rust newcomers
completing the deterministic/causal workflow; distributed-systems
experts diagnosing real failures), a raw-trace baseline
comparator, and cohort sizes, instruments, and pass thresholds fixed in
a preregistration document — an expanded docs/48 — published before the
study runs. The fourth docs/34 workflow (agent repair) is covered by the
G2 ACI ablation and ContinuumBench, not the human study. Authoring the
preregistration is a Phase E deliverable; the study itself runs in
Phase F.

### Phase A — Trust spine and ACI kernel

Deliver:

- immutable workspace and intent schemas;
- native daemon protocol;
- task/continuation lifecycle;
- evidence graph;
- Context Pack v0;
- semantic diff v0;
- executable finite reference engine and certificates from Revision 2;
- the four `continuum-kernel-*` crates under the docs/03 covenant
  (<15,000 non-test lines, no async, no unsafe, serialization boundary
  between engines and kernel) — phase-normative here, not only a PR 9
  annotation;
- agent protocol spike parity;
- Lean environment pinned and seed modules kernel-checked; T0/T1 theorems
  (transition-system safety, stuttering simulation, finite-closure
  certificate soundness) compile with no `sorry` and empty axiom manifests,
  per the RFC 0012 theorem ladder and ADR-0035 axiom manifests.

Exit: Die Hard and Dining Philosophers can be checked through native API,
CLI, and an agent client with identical artifacts; the ACI ablation shows
the typed surface beats disciplined shell use on success and cost, or the
protocol is redesigned before freeze (G0-DX-10); the prompt-injection
corpus cannot trigger privileged operations (G2); continuation resume
validates epochs and inputs (G1).

### Phase B — Real-code failure loop

Deliver:

- asupersync semantic adapter (one adapter crate; supported upstream
  window of versions N and N−1, each covered by the adapter conformance
  corpus in CI against upstream pre-releases — an upstream breaking
  release opens a scheduled adapter epoch (§4.6), visible in envelopes;
  explicit semantic-hook contract; the model core never depends on
  asupersync);
- CML core-fragment parser and elaborator (Finite fragment; enough for the
  replicated register and Wave 0 ports; the programmatic model API remains
  supported);
- storage/network/process packs;
- replicated register;
- causal minimizer;
- debugger v0;
- repair transactions;
- exact and neighboring replay;
- an ADR resolving build-vs-adopt for the §9 incremental engine
  (salsa-derived vs custom), backed by a spike implementing the four
  reuse-edge classes and `query.explain_invalidation` over
  parse/elaborate/explore with a measured invalidation-precision
  baseline; Phase C is `BLOCKED` until this ADR exists.

Exit: an agent fixes ack-before-durable without changing intent and
produces a promotion receipt under the Phase B gate profile.

Promotion gate profiles are phase-staged. Each §8.2 gate enters the default
profile when its producing subsystem ships: gates 1–8 and 11–12 in Phase B;
gate 10 (clean/incremental agreement) in Phase C; gate 9 (certificate and
proof rebuild) in Phase D. A receipt must name its profile and list
not-yet-enforced gates as `NotYetEnforced` — never as passed. A Phase B
receipt is therefore structurally distinguishable from a Phase D receipt.

### Phase C — Interactive scale

Deliver:

- incremental semantic database;
- LSP/DAP/SARIF;
- Incremental Parity Audit (§9.5);
- proof Context Pack format and compiler plumbing (the proof-worker
  efficacy criterion is G6's, evaluated in Phase D);
- read-only tokio observation lane: journal observable lifecycle,
  channel, and time events; opaque-effect inventory; envelopes capped at
  `Observed` status with every claim dimension `Unsupported` — the
  adoption top-of-funnel for the Phase F instrumentation lane, making
  no controlled-semantics claim;
- Wave 0/1 corpus interaction tasks (29 families, pinned to
  `tlaplus/Examples@91c22ea…`) at their required parity per
  `corpus/tla-examples/PARITY_LEVELS.md`, capped at P2 (bounded semantic
  parity): 14 Wave 0/1 families are required at P4 and 3 at P3, which
  need the Phase D proof and liveness machinery; Phase C delivers every
  family at min(required, P2);
- CML language stability: normalized semantic AST frozen before surface
  syntax; formatter and migration tool ship before syntax stability
  (docs/11 §14);
- formative usability sessions (think-aloud, ~5 participants per §13.1
  persona) on the edit/check/explain loop, repeated each phase from C
  onward and feeding docs/34 — distinct from and non-binding on the
  preregistered summative G8 study (§21.1).

Exit: the edit/check/explain loop meets the docs/34 latency table (p50/p95
per interaction) on the reference workload — the table is gate-normative
for G5 and carries an explain-interaction row (failure → rendered causal
explanation), both marked in docs/34 itself; incremental results are
trustworthy under differential audit; reduction engines show zero
reachability mismatch against the unreduced reference on the no-reduction
corpus, and certified claims fall back to the unreduced baseline until the
reduction's certificate lane matures (docs/08 R04); semantic artifacts are
byte-identical across the docs/19 determinism matrix (worker counts, build
modes, platforms, hash seeds).

### Phase D — Proof and liveness

Deliver:

- Lean theorem/certificate pipeline;
- fairness/liveness debugger;
- invariant/ranking synthesis;
- proof repair service;
- refinement receipts;
- self-application: a CML model of `continuumd`'s task/continuation
  lifecycle and INV-017 publication protocol, bound to the daemon
  implementation via the §16 correspondence machinery and checked in CI
  from Phase D onward; a daemon concurrency defect found this way is
  recorded as G4-class evidence of product value;
- Wave 0/1 families raised from the Phase C P2 cap to their full
  required parity (P3/P4);
- corpus Waves 2–3 at required parity;
- corpus release checkpoints mapped to phases: 0.1 closes at Phase C
  entry; 0.2 closes in Phase D;
  0.5 (Waves 0–2 at required parity plus ≥5 P5 exemplar families, now
  designated by name in `validated-examples.csv`) closes in Phase E;
  1.0 (all 80) closes in Phase F with G9; 1.x tracks post-G9 upstream
  corpus additions as maintenance, outside the phase ladder.

Exit: one nontrivial corpus protocol has safety and liveness evidence
plus real-code refinement, produced by a proof service that holds G6:
pinned Lean environment, per-request isolation and cancellation, theorem
and axiom manifests on every receipt, and agent proof repair accepted
only by the kernel.

### Phase E — Forge

Deliver:

- typed holes and grammars;
- CEGIS/interpretation reduction;
- quality-diversity archive;
- co-synthesis of algorithm/invariant/ranking/abstraction;
- materialization to asupersync skeleton.

Exit: Forge rediscovers known solutions through typed holes and finite
CEGIS, produces at least one behaviorally novel candidate independently
verified within the declared envelope, materializes model, Rust skeleton,
and proof obligations for a promoted candidate, and reports
unrealizability as distinct from unknown.

### Phase F — Production and ecosystem

Deliver:

- production partial-order evidence;
- foreign-runtime instrumentation lane (tokio), upgrading the Phase C
  read-only lane: instrumentation synthesis, fault-window replay where
  monitorable, and correspondingly bounded assurance envelopes — an
  adoption bridge, not a claim of controlled semantics;
- instrumentation synthesis;
- remote proof/verification workers;
- all 80 corpus families at declared parity;
- ContinuumBench public release;
- the preregistered G8 usability study (per §21.1) executed and analyzed;
- multi-agent workbench.

Exit: Continuum replaces bespoke DST plus separate TLA+ workflow in at
least two materially different real systems. (§22 G10 and docs/52 G10
carry this same two-system criterion; all three statements are
reconciled in one commit per §22's reconciliation rule.)

Scope note: 1.0 verifies systems whose controlled participants live in
one workspace snapshot. Cross-repository federation — one intent
governing independently deployed participants with a version-skew
envelope — is an explicit 1.0 non-goal, recorded here so Phase F
migrations are recruited within scope. A design note registers the
federation-manifest (`fed_*`) direction for post-1.0.

---

## 22. Release gates

The normative gate scheme is `docs/52_RELEASE_GATES_REV3.md` (G0–G10); this
section summarizes it. Legacy gate citations in Revision 2 ADRs (0001–0035),
RFCs (0001–0025), and Revision 2-era docs (`docs/01`–`docs/32`, including
the risk register `docs/08` and the objections in `docs/20`) refer to the
Revision 2 scheme in `docs/26` and may not be cited without translation;
the translation sweep is part of the specification pass in §25 and includes
retiring the `-Corpus`/`-Proof` gate suffixes (RFCs 0011/0012/0019) and
ADR-0022's internal Lean G-ladder. `docs/04` is Rev-2-bannered (done)
and archived, so no live document carries its gate numbering. No
document may introduce a new gate numbering.

`docs/52` and this section are reconciled in both directions in one
commit: every criterion this section adds is folded into `docs/52`, no
`docs/52` criterion is dropped here, and the dossier validator enforces
bullet-for-bullet correspondence between the two.

Every §21 phase-exit criterion likewise either corresponds to a gate
bullet or is explicitly phase-local; gate-normative criteria never live
only in a phase exit. The previously-orphaned Phase C/D exit criteria
(no-reduction parity, determinism-matrix identity, the nontrivial
corpus protocol) are folded into G5 and G6 below.

| Phase (§21) | Gates it must close |
|---|---|
| A | G0 (falsification), G1 (workbench identity and lifecycle), G2 (ACI) |
| B | G3 (intent integrity), G4 (causal debugging and real repair) |
| C | G5 (incremental trust) |
| D | G6 (proof service) |
| E | G7 (Forge) |
| F | G8 (human usability), G9 (corpus parity), G10 (Continuum 1.0) |

### G0 — Load-bearing falsification

All items in [`notes/G0_SPIKE_MATRIX.md`](notes/G0_SPIKE_MATRIX.md) have
evidence or an explicit redesign decision, recorded in the matrix itself.
A failed or unexecuted freeze-blocking item blocks interface freeze
(docs/52 G0).

G0 closes in Phase A for every item whose required experiment runs
against Phase A machinery: DX-01–03, 10, 12, 13, 14. An unexecuted or
failed item in this subset blocks interface freeze. Items whose
experiments require later subsystems are re-homed to the gates that own
them — DX-04 (causal debugger) → G4, DX-05 (incrementality) → G5,
DX-06 (neighborhood/mutation campaign) → G4, DX-07 (Forge
non-vacuity) → G7, DX-08 (lens ambiguity) → G6, DX-09 (human diagnosis
study) → G8, DX-11 (proof-service isolation) → G6, DX-15 (benchmark
leakage) → G9 — with the Phase A spike results for DX-04, 05, 07, and
08 recorded as artifact-shape evidence only, and each re-homing
recorded in the matrix as that item's explicit decision. The Phase A
benchmark subset used for DX-10 and the G2 Context Pack ablation must
itself pass the §19.4 family/source-hash separation check before
either result is accepted; full leakage validation remains DX-15 at
G9. The matrix carries Status, Evidence, and Decision columns; §0.3's
counts are derived from it, not asserted beside it.

### G1 — Workbench identity and lifecycle

- snapshots, intent contracts, handles, and artifacts are immutable and
  content-addressed;
- explicit handles across the native API;
- requests are idempotent under idempotency keys;
- continuation resume validates epochs and inputs before any reuse;
- cancellation closes obligations and publishes no partial finality;
- artifact publication is transactional (INV-017);
- authorization is checked independently of handle possession;
- daemon crash recovery leaves no stale index entries or orphan tasks (§4.5).

### G2 — Agent-computer interface

- no terminal parsing required;
- explicit handles and resumability;
- stale state rejected;
- generated clients and schemas ship for the native protocol;
- Context Packs are bounded, carry omission manifests and expansion
  handles, and improve agent benchmark effectiveness;
- native ACI beats the disciplined shell baseline on success and cost,
  or the protocol is redesigned before freeze (G0-DX-10);
- the prompt-injection corpus cannot trigger privileged operations.

### G3 — Intent integrity

- every gaming mutation in the development gaming suite (§19.4) that
  falls in a supported fragment is classified as a privileged intent
  change, across all seven diff dimensions (property/assumption/bound/
  observer/fault/fairness/assurance); the held-out gaming suite is
  evaluated once, at G9;
- mutations outside supported fragments classify as `Unknown` and block
  ordinary promotion rather than passing silently;
- intent policy locks are enforced; evidence is invalidated on intent
  revision;
- no ordinary repair promotes with a protected-intent change.

### G4 — Causal debugging and real repair

- a real asupersync failure — not only injected mutants — is diagnosed
  and repaired end-to-end, alongside multiple known mutants;
- causal explanation with a replay-preserving core;
- partial-order debugger including alternate-branch exploration;
- exact and neighboring replay;
- mutation challenge;
- promotion receipt;
- human review view over the repair transaction;
- repair transaction closes exact, neighborhood, and mutation gates under
  the Phase B gate profile (§21).

### G5 — Incremental trust

- incremental results continuously match clean builds under the
  Incremental Parity Audit;
- reuse edges carry their class (Exact/Validated/Conservative/
  Experimental) and mismatches are minimized and quarantine the class;
- proof and certificate freshness is tracked;
- interactive latency targets (docs/34) hold on the reference workload,
  measured with a saturating background swarm present (§4.1, docs/34);
- evidence queries and context compilation meet the docs/34 targets at
  ≥10^7 evidence nodes on the reference workload;
- reduction engines show zero reachability mismatch against the
  unreduced reference on the no-reduction corpus, and certified claims
  fall back to the unreduced baseline until the reduction certificate
  lane matures (docs/08 R04);
- semantic artifacts are byte-identical across the docs/19 determinism
  matrix;
- cache and publication are crash-safe.

### G6 — Proof service

- Lean foundations kernel-check (Revision 2 and Revision 3 theorems, no
  placeholders);
- pinned Lean environment with per-request isolation and cancellation;
- every proof receipt carries theorem and axiom manifests;
- agent proof repair is accepted only by the kernel;
- certificate mutations are rejected;
- proof Context Packs improve proof-worker success/cost;
- one nontrivial corpus protocol carries safety and liveness evidence
  plus real-code refinement produced by this service (Phase D exit).

### G7 — Forge

- typed holes and finite CEGIS;
- safety and non-vacuity;
- hidden variant generalization;
- independent candidate verification;
- diversity archive has semantic, not merely syntactic, spread;
- explicit unrealizability/unknown distinction;
- unrealizability produces reusable evidence where supported;
- materialized model/Rust/proof obligations.

### G8 — Human usability

- the preregistered study (§21.1) covers both cohorts — Rust newcomers
  completing the deterministic/causal workflow, and distributed-systems
  experts diagnosing real failures;
- the preregistration (expanded docs/48) is published before the study
  runs; results are graded only against its fixed thresholds;
- explanation beats the raw-trace baseline on diagnosis accuracy and
  time;
- assurance confidence is calibrated, not merely no worse than baseline;
- progressive disclosure reaches exact artifacts;
- accessibility: no critical workflow requires color or a rendered graph;
- no critical workflow requires formal-methods folklore.

### G9 — Corpus parity

- 80 validated TLA+ families at their declared parity level
  (`corpus/tla-examples/PARITY_LEVELS.md`), measured per §19.4's held-out
  discipline;
- every family carries its interaction artifacts: native model, expected
  verdict and state facts, meaningful explanation, failure/mutation task,
  proof/refinement support where applicable, and an agent benchmark
  artifact (docs/52 G9 — this is *interaction* parity, per B23).

### G10 — Continuum 1.0

- two real project migrations;
- two materially different real projects remove bespoke DST
  infrastructure;
- at least two migrated projects stop requiring a separate TLA+
  workflow for normal development;
- production evidence returns valid pass/fail/inconclusive
  classifications (INV-008);
- public ContinuumBench;
- documented assurance envelopes;
- agent-driven repair used on real changes under review;
- operating cost acceptable;
- zero known paths for unprivileged agent to promote false evidence.

### Release blocker doctrine

A missing feature can be documented as unsupported. A misleading assurance
result, replay failure, stale receipt, hidden intent change, or
unauthorized evidence promotion is a release blocker at every gate. A
confirmed false-positive success verdict triggers the soundness incident
policy in `docs/09`: block release, revoke affected claim IDs, publish
affected semantic epochs, ship an artifact scanner, add a permanent
regression, and reevaluate whether the producing engine remains eligible
for certified mode.

---

## 23. Success metrics

### Product

- median time from defect injection to understood causal explanation;
- median time from explanation to promoted repair;
- percentage of concurrent changes carrying evidence receipts;
- reduction in bespoke DST code;
- number of projects no longer maintaining separate TLA+ models;
- human and agent successful transfer to unseen protocols.

### Verification

- counterexample replay rate;
- causal core size and faithfulness;
- state/class reduction with preservation evidence;
- proof/certificate checking time;
- incremental/clean mismatch rate;
- unsupported/inconclusive honesty.

### Agents

- task success under fixed model;
- token/tool/wall cost;
- invalid operation rate;
- stale-handle recovery;
- intent-gaming rejection;
- expensive failure rate;
- patch robustness on hidden variants;
- proof success with/without Context Packs.

### Forge

- solved synthesis tasks;
- proof rate;
- generalization;
- Pareto improvements;
- behavioral diversity;
- rediscovery of known algorithms;
- independently validated novel candidates.

---

## 24. Kill criteria

Continuum must narrow or redesign if:

- real code requires pervasive rewrites solely to become observable;
- correlated wrongness is detected: Continuum, its checker, and its
  adapter agree while a foreign differential oracle (TLC/Apalache over
  the corpus subset run at every semantic epoch advance) disagrees, and
  the discrepancy survives triage — the docs/08 R03 circular-trust
  scenario; this triggers the docs/09 soundness-incident policy, not
  only a narrowing review;
- no real project is willing to migrate its bespoke DST by the end of
  Phase D, two phases before the Phase F migrations are due (docs/08
  R01's kill signal, pulled forward as an adoption checkpoint);
- unreduced liveness checking is intractable on the corpus liveness
  subset and the reduction lane has not promoted (docs/08 R11) — the
  register fallback "unreduced liveness" is not available if this
  bullet fires;
- incremental dependency edges collapse to Conservative on the
  reference workload (docs/08 R21's kill signal);
- model/program correspondence remains mostly manual and fragile;
- Context Packs frequently omit defect causes;
- agent-native API does not beat disciplined CLI use;
- intent diff cannot reliably expose gaming in supported fragments;
- incremental trust overhead erases interactivity;
- Lean/certificate integration makes ordinary checks unusably slow;
- Forge mostly discovers vacuous or overfit protocols;
- all practical power comes from a loose collection of external tools rather than shared semantics;
- a second real project requires engine-specific surgery rather than domain packs;
- users systematically misread bounded evidence as proof despite UX controls;
- asupersync integration requires invasive scheduler forks that cannot be
  stabilized behind the pinned adapter contract (docs/08 R06);
- the foreign-runtime instrumentation lane cannot produce useful envelopes
  for tokio-based systems, leaving no adoption bridge;
- domain packs cannot demonstrate conformance to their fidelity profiles on
  real systems, making applied durability/network claims unearned
  (docs/08 R10);
- a candid comparison shows Quint plus existing DST tooling would be
  cheaper and equally strong for the target users; a “yes” after G4 closes
  is a program-level failure signal (docs/08 states this criterion as
  “after G2” in the Revision 2 scheme; Rev-2 G2 ≈ Rev-3 G4).

Failure of a frontier research lane does not kill Continuum. Failure of the single-semantic-contract, intent-integrity, replay, or evidence architecture does.

## 24.5 Frontier lane register

Every HYPOTHESIS-class capability in this plan is owned by a research lane
with a baseline, a quantitative promotion threshold, a kill criterion, and
a named fallback. The register is authoritative for lane status; no plan
section may claim a lane's output without its status.

A lane is **ratified** when its owner fixes the numeric threshold in the
research note and this register quotes it verbatim; ratified quotes
carry a `quote-id` marker in both files and the dossier validator
compares the marked quotes for identity (a marked row is ratified;
every unmarked row is draft or defer). A row that delegates to a document
("docs/31's stated pair") without quoting is draft by definition.
Until ratified, a row is **draft** and blocks its lane's promotion.

| Capability | Lane | Threshold / kill | Fallback |
|---|---|---|---|
| General context compilation (§6) | research/25, research/32, research/33 | ablation design per research/25 (raw trace vs pack on the agent benchmark); ratified win margin, quote-id=context-compilation-win-margin "The Context Pack condition wins the general context-compilation ablation only if, on the held-out ContinuumBench diagnosis-and-repair split graded by the independent grader of research/33 with model, scaffold, and token budget held equal, it beats the raw-trace baseline by at least 10 percentage points of absolute task success with a 95 percent bootstrap confidence interval on the paired difference excluding zero, while delivering at least a 10x median per-task reduction in context bytes counting every expansion and staying within 2 percentage points of the raw-trace baseline on hidden-variant generalization."; kill (research/32): compact packs repeatedly induce incorrect repairs despite preservation checks | plain causal slice + expansion |
| Exploration reduction (§9, INV-013) | research/01; docs/31 | research/01 (stated there as a kill): at least an order-of-magnitude reduction on a non-artificial subset without a serious regression on dependent workloads (the "explored classes" denominator is a proposed ratification, to be fixed in research/01 before this row leaves draft); observer-indexed (docs/31): median ≥5× on the observer-sensitive class, *checker* overhead <20% (distinct from the certificate-overhead row), zero mutation loss; kill (docs/31, research/13): witness cost dominates, or observer/property changes invalidate cached independence so often that reuse never pays for its witness cost — draft | conservative unreduced exploration |
| Liveness-preserving reduction (§7.2, Phase D) | research/04, research/13 — lane to be opened | soundness gate per research/04: property-directed reduction is proven preserving (for the full property class it is applied to, fair cycles included) or disabled; speedup target and liveness corpus subset fixed at lane opening — draft | unreduced liveness with explicit cost banner; batch expectations stated in the Phase D exit |
| Causal minimization (§6, §12) | research/26 | ratified (research/26) quote-id=causal-minimization-core-ratio "On the research/26 experiment corpus of real failures — traces produced by an actual defect, excluding any trace padded with semantically inert events (plan §25) — the replay-preserving causal core must be ≤10% of trace length at the corpus median."; kill if minimization cost dominates verification | 1-minimal delta debugging only |
| Neighborhood adequacy (§8.3) | research/33 | ratified (research/33) quote-id=neighborhood-adequacy-catch-rate "On the docs/50-classified gaming corpus — at least 20 hidden mutations for each of the five docs/50 attack classes (intent, instrumentation, evidence, overfitting, resource), so at least 100 mutations in total — the §8.3 neighborhood is adequate only if it catches at least 90 percent of the mutations overall with the one-sided 95 percent exact binomial lower bound on that rate above 80 percent and no single attack class below 75 percent, counting a mutation as caught only when property-directed neighboring exploration surfaces a neighbor on which the gaming patch fails and the receipt discloses that neighbor, with every mutation drawn from a family-level held-out split and graded by the independent grader of research/33."; the corpus is a Phase B G3 deliverable and does not exist today, so the ratified threshold binds when it lands; kill (research/33): hidden variants are too easy to leak or too hard to grade independently | fixed strategy-list neighborhood with per-receipt coverage disclosure and no adequacy claim |
| Sub-file incremental trust (§9) | research/27 | clean-mismatch rate at the §9.5 sampled audit rate (rate derives from §8.6's confidence target) — draft; kill if dependency capture cannot be made trustworthy below file/module granularity | module-granularity invalidation |
| Forge co-synthesis + QD (§14) | research/29, research/30 | rediscovery suite (named algorithms and count fixed at ratification — draft); kills (research/29): joint space overwhelms coupled-feedback gains; proof-complexity objective biases toward trivial designs; abstractions overfit finite bounds; agent proposals dominate and cannot be reproduced by structured search; kill (research/30): QD discovers nothing beyond multi-objective Pareto search on known benchmarks | staged synthesis; lightweight semantic-diversity archive (per research/30) |
| Production conformance (Phase F) | research/05, research/36 | draft pending ratification: faithful Lab reproduction of ≥70% of curated known incidents under injected telemetry loss and clock uncertainty; always-on overhead ≤1%; ≥2× reduction in surviving symmetry orbits per synthesized probe set; kill if most target properties are monitorable only as `Inconclusive` on two real systems, or the overhead budgets cannot be met | Lab-replay evidence only |
| Cancellation calculus (§0, B19) | research/09 | draft pending ratification: 10/10 mutation-corpus mutants detected with zero false alarms on the correct implementation; machine-checked drain ranking for the replicated register; kill (research/09): invariant annotations become pervasive in real code, or the calculus cannot express asupersync's actual cancellation semantics | runtime checking only |
| Bidirectional lenses (§16) | research/31 | ambiguity rate := fraction of abstract edits on the drift corpus yielding multiple or no concrete candidates (metric per research/31; the drift corpus is the lane deliverable; the numeric target is fixed in the note at ratification and quoted here) — draft; kill if most real mappings are too ambiguous, or users mistake candidate synchronization for verified preservation (measured under the G8 instruments) | get-only projection + drift detection |
| Proof repair (§15.4) | research/28 | vs source/LSP loop baseline (qualitative; margin fixed at ratification — draft); kill if repair proposes weakening | context packs + human proof work |
| Certificate overhead | research/12; docs/31 | checking ≤10% of search time for large finite proofs, or acceptable asynchronous CI latency (docs/31); research/12 must gain a criteria section before this row leaves draft — draft | per docs/31: the verified native checker remains the routine path; Lean validates the checker and sampled/full release artifacts |
| Nominal / orbit-finite verification | research/14; docs/31 | docs/31's stated promote/kill pair (unbounded session/request protocol with tractable orbit growth and Lean-proved equivariance) | finite-bounds checking only |
| Sheaf-based composition | research/03; docs/31 | docs/31's stated promote/kill pair | monolithic composition proofs |
| Full TLA+ source importer | docs/31; ADR-0029 | docs/31's stated promote/defer pair; not a 1.0 goal (ADR-0029) | manual semantic porting per docs/32 |
| Weak-memory lane (B11) | ADR-0032 — lane to be opened | reproduces the standard litmus corpus under the declared model before any envelope upgrade; until then every memory dimension reads `Unsupported(sequential-consistency-only)`; defer: SC-only is a declared 1.0 non-goal | SC-only, declared as a 1.0 non-goal |
| Timed/probabilistic semantics | ADR-0016 — lane to be opened | per-ADR staging; until shipped, timing fields in Intent Contracts remain declarative assumptions (B11); defer: post-1.0 | declarative assumptions only |
| Agent–computer interface (B2, §10) | research/25 | native ACI vs a disciplined-shell baseline (harness per research/25) on the agent benchmark: success-rate, cost, and invalid-action margins ratified — quote-id=aci-benchmark-margins "On the agent benchmark, with an identical base model, task set, and per-task budget, native ACI must beat the disciplined-shell baseline by at least 10 percentage points of absolute task success, at least 30% fewer interface bytes per solved task (bytes, not tokens, are the graded cost denominator per RFC 0027), and at least a 50% relative reduction in invalid-action rate, with every metric paired per task over at least 3 seeds and the success margin's one-sided 95% lower bound above zero; missing any one of the three margins fails G0-DX-10 and forces protocol redesign before freeze."; kills (research/25): typed surface loses to disciplined shell; schema churn dominates agent cost; handles do not reduce invalid-action rate | protocol redesign before freeze (G0-DX-10); MCP-only surface |
| Explanation science (§12) | research/34 | diagnosis accuracy/time vs raw-trace baseline under the G8 instruments and Phase C formative sessions; thresholds fixed in the docs/48 preregistration — draft; kills (research/34): experts prefer raw output and newcomers gain no accuracy; assistance raises confidence faster than correctness | §12.1 levels 1–2 only (outcome + causal core), no adequacy claim |
| Workbench security (§18) | research/35 | ratified promotion gate (research/35) — quote-id=workbench-security-promotion-gate "Autonomous promotion stays disabled until a single red-team corpus run against the current build and dependency epoch clears every case, where the corpus contains at least three cases for each of the ten red-team classes of research/35, one per prohibited outcome named in its kill criterion (unprivileged intent-status alteration, unprivileged evidence-status alteration, isolation escape), plus one case for each of the seven intent-policy blocks and one escape attempt against each of the eight worker-isolation controls of docs/49, for at least 45 cases in total; the run clears only if every case is refused by a trusted authority check and recorded in the append-only audit log, with zero unprivileged intent-status or evidence-status alterations and zero isolation escapes, and any later case that succeeds re-locks autonomous promotion until the corpus, enlarged with that case and its regression test, clears again in full, leaving human-approved promotion as the standing fallback."; no project kill for an individual vulnerability (research/35): a successful case re-locks autonomous promotion and falls back to human approval | human-approved promotion only |
| Multi-agent evidence graph enforcement (§11.7) | RFC 0038 (§25 debt); docs/53 boundary | concurrent authority enforcement validated beyond the finite spike (docs/53: "authority table only, not enforcement"): linearized promotion, CAS status, conflict materialization under concurrent writers — draft | single-writer evidence graph; agents coordinate through one integrator role |

A register row must be ratified before the phase that first consumes
its lane's output opens: context compilation, ACI, and causal
minimization in Phase A; neighborhood adequacy, workbench security, and
evidence-graph enforcement in Phase B; sub-file incremental trust in
Phase C; liveness reduction, proof repair, and certificate overhead in
Phase D; Forge and explanation science (via the docs/48
preregistration) in Phase E; production conformance in Phase F; rows
not named here ratify at their lane's opening, before any plan section
consumes their output. Rows recorded as `defer` (weak memory,
timed/probabilistic, the TLA+ importer) are exempt while the deferral
stands. Until ratified a row blocks its lane's promotion; a row whose
threshold cannot be ratified by its own deadline is a plan defect, not
a lane defect. docs/31's remaining thresholds (cubical reduction ≥3× on
≥2 real protocol classes with a preservation theorem; semiring within
15% of specialized analyses with three production analyses sharing
code; assumption synthesis on ≥5 liveness cases; and abstraction
discovery, which is qualitative in docs/31 and enters as a draft row
pending a quantitative ratification) are merged into this register
during the §25 pass so the program has exactly one lane authority;
where docs/31 and a research note disagree, the note is corrected and
cited.

---

## 25. Immediate execution

The ordered initial PR sequence (PR 0–PR 30, with inserts 4a/15a/25a) is
in [`notes/START_HERE_IMPLEMENTATION.md`](notes/START_HERE_IMPLEMENTATION.md).
Review-5 inserts carry the phase deliverables previously unowned by any
PR: 15b (CML formatter and migration tool), 22a (incremental-engine ADR
+ reuse-edge spike; blocks Phase C per §21), 26a (read-only tokio
observation lane), 27a (Wave 0/1 corpus at min(required, P2)), and 27b
(docs/48 preregistration and formative-session instruments).

The seven load-bearing Revision 3 RFCs (0026, 0027, 0028, 0030, 0031,
0032, 0037) are expanded to specification form — field types, enums,
classification lattices, versioning, RFC-2119 language — and the
execution layer is reconciled: START_HERE carries PR 0, the Phase A
Lean band (PR 4a; the proof *service* remains PR 28), the four
`continuum-kernel-*` crates at PR 9, the CML parser (15a) and SARIF
(25a) PRs, and per-PR gate annotations; the G0 matrix carries
Status/Evidence/Decision columns, the DX-09 preregistration
requirement, and the §22 re-homing decisions; the dossier validator
enforces gate-citation hygiene, bidirectional plan↔docs/52 bullet
correspondence, and START_HERE gate annotations. Where plan prose and
RFC disagree, the RFC is corrected and becomes normative. The plan is a
map, not the spec.

The validator itself is brought up to these claims and its outputs
regenerated in the same pass: lettered PR headings (4a/15a/25a) are
matched; G0's freeze-blocking subset and the phase↔gate tables are
compared structurally, not as empty bullet lists; bare Rev-2 gate
citations (including `Target gate:` metadata in RFCs 0001–0025) require
a scheme qualifier or banner; §0.3's G0 counts and program status are
derived checks; §24.5's register rows are checked for resolvable lanes
and kill/defer/draft markers, with marked-quote identity enforced once
rows ratify; the specification-debt ledger below is derived by
`check_spec_debt`; and `VALIDATION_REPORT.md` /
`validation-results.json` are regenerated so every advertised check has
a recorded result.

Specification-debt status is validator-derived: each item below carries
a stable ID and a mechanical predicate in `tools/validate_dossier.py`
(`check_spec_debt`); the open set is emitted into
`validation-results.json`, and this list is edited only together with
its predicates — never asserted by hand (the discipline §0.3 already
applies to G0 counts). Pre-freeze items are paid before PR 5 (native
protocol kernel) freezes any interface; PR 0's exit condition is the
pre-freeze open-debt set reading empty. Items previously listed here
that are verified paid (the intent-registry record schema, the
promotion-receipt cost ledger and gate profile, the nine-dimension
assurance envelope with `resource-exhausted` removed, typed `Failed`
reasons, INV-008/`validation_basis`/fail-closed conditionals, RFC
0037's `intent.accept`/`intent.lock`/`revise-intent` and four observer
projections, and the docs/35/40/41/42/45 corrections) are recorded in
`plan.review.5.md` Appendix B, not re-listed as debt.

- SD-01 (open): RFC 0026's normative IDL file (referenced, not
  present);
- SD-07 (open): `schemas/intent-contract.schema.json` — a structured
  property-AST expression form with canonical normalization, replacing
  the bare `expression` string;
- SD-08 (open): one `$id`/versioning convention with a schema-epoch
  field across all schemas (§4.3) — three rival conventions today,
  zero schema-epoch fields;
- SD-09 (open): docs/35, RFC 0026, ADR-0018, and docs/42 absorb the
  §4.5/§4.6/§4.7 operational contract (purge and `Redacted(reason,
  commitment)`, backup and verified restore, the cross-user dedup
  existence-oracle rule, two-epoch migration,
  `Preserved | Revalidate | Incompatible` compatibility statements,
  engine-defect artifacts);
- SD-10 (open): docs/41 regenerated around the 12-gate, phase-profile
  repair design (RFC 0032 is normative in the interim);
- SD-02 (paid, review 5): RFC 0026 absorbs the §4.3 requirements it
  omitted — the N and N−1 protocol-major window, evidence/receipt
  readability decoupling, mid-flight budget updates
  (`task.update_budget`), evidence subscriptions, and the five §10.3
  error codes added in review 5;
- SD-03 (paid, review 5): RFC 0030 absorbs §9.5's auditability classes
  and class-scoped quarantine rule and derives the audit sampling rate
  from §8.6's confidence target (1-in-64 remains the labeled bootstrap
  default);
- SD-04 (paid, review 5): RFC 0032 absorbs §8.6 — the incremental gate
  5–7 reuse rule, cost ceilings, and
  `BudgetExhausted`-with-continuation;
- SD-05 (paid, review 5): RFC 0037 carries the §4.2.1 intent-bundle
  distribution and convergence section;
- SD-06 (paid, review 5): RFC 0038 carries the §11.7 evidence-graph
  write/concurrency model;
- SD-11 (paid, review 5): fail-closed repair of the anti-gaming
  schemas — `semantic-diff` binds `protected` structurally;
  `repair-transaction` drops the undefined `not_applicable` status
  from promotable profiles and gains `phase-c`/`phase-d` conditionals;
  gate lists in `repair-transaction` and `promotion-receipt` enforce
  the twelve gate identities, not a count; `evidence-graph-node`
  requires a service identity on
  `Sampled`/`Bounded`/`Validated`/`Proved` promotion and
  `evidence-graph-edge` requires `checker` on `CHECKED_BY` (INV-004);
- SD-12 (paid, review 5): one assurance envelope — `context-pack`
  mirrors the canonical nine-dimension envelope (identity-checked
  against `assurance-result` by the validator, so the mirrored copies
  cannot drift); one verdict vocabulary (hyphenation unified, INV-008
  reasons conditional everywhere); one gate-status enum; one
  budget/cost dimension list shared by RFC 0026, `verification-task`,
  and the §8.6 cost ledger;
- SD-13 (paid, review 5): `verification-task` requires a continuation
  on `suspended` and on `BudgetExhausted` (typed exception for
  genuinely non-resumable exhaustion); `workspace-snapshot` carries
  all ten §4.2 components and drops the protocol epoch from snapshot
  identity (a connection property; §4.6 forbids identity churn on
  epoch advance);
- SD-14 (paid, review 5): the shared `Redacted(reason, commitment)`
  definition is mirrored verbatim — identity-checked by the validator —
  into every artifact class §4.5 obligates (receipts, transactions,
  envelopes, evidence nodes, tasks), not only the original three.

The first demonstration should be brutally concrete:

```text
1. Agent receives a 200-event failing run through a 4-event Context Pack.
2. It identifies acknowledgement-before-durability.
3. It proposes moving publication after sync.
4. Continuum proves intent unchanged.
5. Exact replay passes after the patch.
6. Neighboring cancellation/crash schedules pass.
7. Property mutations and known mutants remain detectable.
8. Refinement and proof receipts rebuild.
9. A human reviews one compact semantic diff and receipt.
```

Then Forge should solve the corresponding hole from scratch:

```text
ack_guard ∈ grammar
Safety: Ack ⇒ Durable
Progress: failure-free synced request eventually acknowledged
Result: ack_guard = synced
```

This is deliberately small. It closes the complete product loop—from intent through invention and proof—before the project scales outward.

The executed spikes behind this demonstration carry their own boundaries,
which this plan adopts: the 200→4 event reduction was measured on a
synthetic trace whose 196 noise events are semantically inert, so it
validates the Context Pack artifact shape, not the general context
compiler; the synthesis result was selected from a seven-candidate finite
grammar and is not evidence that protocol synthesis will scale; the
evidence-graph spike validates the authority policy table, not
authenticated enforcement, concurrency, or Byzantine agents.

---

## 26. Final doctrine

Continuum will succeed if it makes the correct thing easier than the seductive wrong thing:

- easier to state intent than bury assumptions;
- easier to consume a causal explanation than grep a log;
- easier to repair under a transaction than overfit a test;
- easier to resume by handle than preserve a fragile session;
- easier to inspect an assurance envelope than trust a green badge;
- easier for agents to use typed evidence than hallucinate from prose;
- easier to invent inside proof constraints than bolt verification on afterward.

The long-term ambition is not merely safer concurrency. It is a new mode of systems research:

> humans specify values and constraints; agents explore enormous mathematical and implementation spaces; Continuum turns every proposed leap into replayable evidence, checked refinement, and proof-bearing code.

That is how concurrent and distributed systems stop being artisanal collections of race conditions and become a domain where invention can accelerate without sacrificing truth.
