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
```

Then:

```bash
cargo continuum init
cargo continuum check
```

The first result is concise:

```text
FAIL  AckImpliesDurable

Ack became observable before the corresponding write became durable.
Causal core: 4 events · 2 tasks · 1 cancellation
Abstract mismatch: acked +1, durable unchanged

replay   cp_7m3...
debug    continuum debug cp_7m3...
explain  continuum explain cp_7m3... --level causal
```

No Java installation, model-config archaeology, or megabytes of state dumps.

### 3.2 Agent path

An agent does not invoke arbitrary shell text. It calls:

```json
{
  "operation": "verify.start",
  "snapshot": "ws_4f...",
  "intent": "in_91...",
  "target": "property:AckImpliesDurable",
  "budget": {"states": 100000, "wall_ms": 30000},
  "output": {"context_pack": true, "max_tokens": 6000}
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
continuum repair begin cp_7m3...
continuum repair apply --patch patch.diff --hypothesis "publish only after sync"
continuum repair evaluate rt_2c...
continuum repair promote rt_2c...
```

Promotion returns a receipt only after all policy gates close.

### 3.5 Invention path

```bash
continuum forge synthesize models/broadcast.cml \
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

### 4.2 Workspace snapshots

A workspace snapshot contains content identities for:

- source files;
- CML modules;
- Rust semantic extraction;
- domain-pack manifests;
- dependency lockfiles;
- toolchain and semantic epochs;
- Intent Contract;
- generated correspondence;
- proof environment;
- configuration.

Snapshots form a Merkle DAG. A tool call never means “whatever is currently on disk”; it means a named snapshot. Clients may create a snapshot from a working tree, overlay an in-memory editor buffer, or fork an existing snapshot.

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

MCP, LSP, DAP, and SARIF translate to this protocol. None define core semantics.

### 4.4 Content-addressed artifacts

Artifact classes include:

```text
ws_* workspace snapshot
in_* intent contract
model_* elaborated model
cir_* causal execution graph
cp_* crashpack
ctx_* Context Pack
proof_* proof artifact
receipt_* signed/checked receipt
rt_* repair transaction
forge_* synthesis archive
cont_* resumable task continuation
```

Opaque handles prevent clients from guessing structure. Authorization is checked independently of handle possession.

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
  environment, fairness, timing, failure, trust
observers
  state/event/knowledge/security projections
scope
  model/program components and abstraction level
bounds
  values, processes, faults, schedules, depth
assurance policy
  accepted evidence classes and required checkers
optimization
  hard constraints, soft objectives, non-vacuity
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
- proof policy upgrade/downgrade;
- unsupported semantic change;
- unchanged intent.

Where implication is decidable or solver-checkable, Continuum proves the direction. Otherwise it emits a proof obligation or `Unknown` rather than guessing.

### 5.4 Intent locks

CI policy can lock fields:

```toml
[intent.policy]
properties = "maintainer-review"
assumptions = "maintainer-review"
bounds = "no-decrease"
faults = "no-removal"
assurance = "no-downgrade"
observers = "proof-required"
```

Agent repair capabilities exclude intent mutation unless a task explicitly asks for redesign.

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

---

## 9. Incremental semantic database

### 9.1 Need

Impeccable DX requires subsecond feedback for local edits and resumable deeper searches. Re-running every model, extraction, exploration, proof, and explanation from scratch would make Continuum irrelevant in normal development.

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

### 9.5 Clean-build Tribunal

CI and sampled local runs compare incremental and clean artifacts:

- verdict;
- canonical state graph or digest;
- counterexample class;
- certificate result;
- context slice soundness;
- semantic diff;
- proof axiom manifest.

Any disagreement quarantines the relevant reuse class and emits a minimal invalidation counterexample.

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
intent.get / diff / propose_revision
model.check / explore / compare
program.extract / run / replay
refinement.check / explain
proof.goal / attempt / check / slice
context.compile / expand
failure.explain / minimize / branch
repair.begin / apply / evaluate / promote
forge.create / step / archive / materialize
benchmark.run
task.status / cancel / resume / subscribe
evidence.get / query / verify
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
Bounded
Validated
Proved
Refuted
Inconclusive
Superseded
```

Only trusted services can promote into `Validated` or `Proved`. Agent votes or confidence cannot.

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
continuum explain cp_x --level semantic
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
cargo continuum replay cp_x
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

### 18.4 Context privacy

Context compilation enforces source/trace field policy before slicing. Redaction is represented in the omission manifest. A redacted pack cannot support claims requiring hidden data unless a trusted checker provides a separate receipt.

### 18.5 Audit

Every privileged operation records actor, capability, inputs, policy decision, outputs, and evidence identity. Audit logs are append-only and separate from semantic events.

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

### 19.4 Dataset construction

Sources include:

- 80 validated TLA+ example families;
- deliberate and generated mutations;
- historical concurrency bugs;
- Continuum domain packs;
- asupersync cancellation/obligation cases;
- proof and refinement tasks;
- synthesized hidden variants;
- real project migrations.

Train/dev/test partitions isolate semantic families and source hashes to reduce leakage.

### 19.5 Reward-hacking suite

Every benchmark candidate is tested against attempts to:

- weaken property;
- add assumptions;
- lower bounds;
- remove faults;
- hide observer events;
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
- model core does not depend on asupersync;
- adapters do not own semantic state;
- Forge depends on verifier interfaces, never vice versa;
- UI crates cannot mutate evidence directly;
- foreign tools remain isolated behind normalized artifacts.

---

## 21. Implementation program

### Phase A — Trust spine and ACI kernel

Deliver:

- immutable workspace and intent schemas;
- native daemon protocol;
- task/continuation lifecycle;
- evidence graph;
- Context Pack v0;
- semantic diff v0;
- executable finite reference engine and certificates from Revision 2;
- agent protocol spike parity.

Exit: Die Hard and Dining Philosophers can be checked through native API, CLI, and an agent client with identical artifacts.

### Phase B — Real-code failure loop

Deliver:

- asupersync semantic adapter;
- storage/network/process packs;
- replicated register;
- causal minimizer;
- debugger v0;
- repair transactions;
- exact and neighboring replay.

Exit: an agent fixes ack-before-durable without changing intent and produces a promotion receipt.

### Phase C — Interactive scale

Deliver:

- incremental semantic database;
- LSP/DAP/SARIF;
- clean-build Tribunal;
- proof Context Packs;
- Wave 0/1 corpus interaction tasks.

Exit: normal edit/check/explain loop is subsecond for local changes and trustworthy under differential audit.

### Phase D — Proof and liveness

Deliver:

- Lean theorem/certificate pipeline;
- fairness/liveness debugger;
- invariant/ranking synthesis;
- proof repair service;
- refinement receipts.

Exit: one nontrivial corpus protocol has safety and liveness evidence plus real-code refinement.

### Phase E — Forge

Deliver:

- typed holes and grammars;
- CEGIS/interpretation reduction;
- quality-diversity archive;
- co-synthesis of algorithm/invariant/ranking/abstraction;
- materialization to asupersync skeleton.

Exit: Forge rediscovers known solutions and produces at least one behaviorally novel candidate independently verified within the declared envelope.

### Phase F — Production and ecosystem

Deliver:

- production partial-order evidence;
- instrumentation synthesis;
- remote proof/verification workers;
- all 80 corpus families at declared parity;
- ContinuumBench public release;
- multi-agent workbench.

Exit: Continuum replaces bespoke DST plus separate TLA+ workflow in at least two materially different real systems.

---

## 22. Release gates

### G0 — Load-bearing falsification

All items in [`notes/G0_SPIKE_MATRIX.md`](notes/G0_SPIKE_MATRIX.md) have evidence or explicit redesign.

### G1 — Agent-computer interface

- no terminal parsing required;
- explicit handles and resumability;
- stale state rejected;
- Context Pack improves agent benchmark effectiveness;
- protected intent detects all benchmark gaming mutations.

### G2 — Real repair

- correct asupersync implementation and multiple mutants;
- causal explanation and debugger;
- repair transaction closes exact, neighborhood, mutation, proof, and clean-build gates.

### G3 — Human usability

- task study shows explanation improves diagnosis;
- confidence calibration does not worsen;
- progressive disclosure reaches exact artifacts;
- no critical workflow requires formal-methods folklore.

### G4 — Proof and incrementality

- Lean foundations kernel-check;
- incremental results continuously match clean builds;
- certificate mutations are rejected;
- proof Context Packs improve proof-worker success/cost.

### G5 — Forge

- safety and non-vacuity;
- hidden variant generalization;
- independent candidate verification;
- diversity archive has semantic, not merely syntactic, spread;
- unrealizability produces reusable evidence where supported.

### G6 — Continuum 1.0

- 80 validated TLA+ families at declared parity;
- two real project migrations;
- production evidence path;
- public ContinuumBench;
- documented assurance envelopes;
- zero known paths for unprivileged agent to promote false evidence.

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
- model/program correspondence remains mostly manual and fragile;
- Context Packs frequently omit defect causes;
- agent-native API does not beat disciplined CLI use;
- intent diff cannot reliably expose gaming in supported fragments;
- incremental trust overhead erases interactivity;
- Lean/certificate integration makes ordinary checks unusably slow;
- Forge mostly discovers vacuous or overfit protocols;
- all practical power comes from a loose collection of external tools rather than shared semantics;
- a second real project requires engine-specific surgery rather than domain packs;
- users systematically misread bounded evidence as proof despite UX controls.

Failure of a frontier research lane does not kill Continuum. Failure of the single-semantic-contract, intent-integrity, replay, or evidence architecture does.

---

## 25. Immediate execution

The exact first 30 pull requests are in [`notes/START_HERE_IMPLEMENTATION.md`](notes/START_HERE_IMPLEMENTATION.md).

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
