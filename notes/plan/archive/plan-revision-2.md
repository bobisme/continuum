# Continuum: Master Implementation Plan

**Document date:** 2026-07-24  
**Status:** revision 2 — corpus-governed execution program  
**Reference corpus:** `tlaplus/Examples@91c22ea537853196ed1e03e9ad91693ec37642de`  
**Proof epoch:** Lean `v4.32.1`  
**Primary implementation language:** Rust  
**Concrete async substrate:** asupersync  
**Evidence vocabulary:** `FACT`, `DESIGN`, `HYPOTHESIS`, `TARGET`, `BLOCKED`, `FALSIFIED`

---

## 0. Declaration of intent

Continuum is a verification-native environment for designing, implementing, testing, proving, operating, and repairing concurrent and distributed Rust systems.

It is not:

- a Rust translation of TLC;
- a model checker hidden behind procedural macros;
- a deterministic simulator with unusually ambitious documentation;
- a theorem prover forced into the critical path of ordinary development;
- a bundle of TLA+, Quint, Loom, Verus, an SMT solver, and a trace viewer;
- a claim that one source program can be both maximally abstract and production-complete without explicit refinement.

Continuum is built around a **semantic triptych**:

```text
                              MODEL
                  mathematical behavior and properties
                    /                           \
                   /                             \
       corpus parity and proof             refinement graph
                 /                                 \
                /                                   \
            PROOF -------------------------------- PROGRAM
     Lean metatheory and certificates       asupersync Rust execution
```

Each corner is independently meaningful:

- **Model** exists before implementation and permits arbitrary useful abstraction.
- **Program** is real Rust code executing under production or controlled semantics.
- **Proof** defines what strong claims mean and independently checks evidence.

The Causal Intermediate Representation (CIR) connects concrete executions, partial-order exploration, replay, and production evidence. The Continuum Model Language (CML) connects human-authored specifications to mathematical transition and behavior semantics. Lean 4 connects semantic definitions, transformations, and solver certificates to machine-checked theorems.

The long-term product promise is deliberately narrower and stronger than “support TLA+ syntax”:

> For a new concurrent or distributed Rust system, there should be no technical reason to maintain a separate TLA+ specification, a bespoke deterministic simulator, a separate model-based test harness, and handwritten production-trace validation glue.

The external proof that this promise is not marketing is the TLA+ Examples corpus. Continuum 1.0 does not ship until it has native semantic equivalents for every CI-validated example family in that corpus at its declared parity level.

---

## 0.1 What revision 2 changes

Revision 1 treated CIR as the project’s stable center. That was necessary but insufficient. A causal execution format can unify runtime evidence and partial-order algorithms, but it cannot by itself provide:

- pre-implementation mathematical modeling;
- infinite-behavior semantics;
- fairness and liveness;
- theorem-level refinement;
- proof-producing reductions;
- an objective completeness test against the expressiveness that makes TLA+ useful.

Revision 2 therefore makes six structural changes.

### R2-1 — The TLA+ Examples corpus becomes a release contract

The pinned corpus contains 80 CI-validated specification families spanning finite puzzles, concurrency, mutual exclusion, refinement, consensus, Byzantine protocols, transactions, storage, TCP, cache coherence, lock-free structures, proofs, liveness, symmetry, PlusCal, and deliberate failures. Its own repository presents the collection as both an example library and a development/testing corpus for language tools.

Continuum ports are semantic equivalents, not source transliterations. Each port has an explicit parity level, oracle evidence, mutations, and—where applicable—Lean proofs and real asupersync refinements.

### R2-2 — Lean begins at the foundation, not as a late audit

Lean owns the metatheory of transition systems, temporal behavior, fairness, refinement, event structures, cancellation, and certificates. Rust owns high-performance search and product integration. Strong results cross a proof-producing boundary.

### R2-3 — No semantic path certifies itself

The reference evaluator, optimized Rust engines, asupersync adapter, foreign TLA+ oracles, and Lean certificate checkers are deliberately separate. Shared implementation code may improve reuse but may not be used as the sole evidence for a strong equivalence claim.

### R2-4 — The model language is stratified by semantic fragment

CML exposes the expressiveness needed for the corpus while reporting which backends apply. Finite, symbolic, temporal, probabilistic, theorem, and runtime fragments are explicit. Unsupported combinations fail loudly rather than receiving a convenient unsound approximation.

### R2-5 — Reductions become proof-producing or translation-validated

Symmetry, slicing, DPOR, unfoldings, finite instantiation, solver lowering, and abstraction are not merely optimizations. Each declares a preservation theorem and emits instance evidence appropriate to the claimed assurance level.

### R2-6 — The research frontier moves closer to the product

Observer-indexed independence, nominal/orbit-finite state spaces, assumption synthesis as games, semiring-valued analysis, higher-dimensional concurrency, choreographic projection, weak-memory execution graphs, and sheaf-like composition receive concrete experiments, promotion gates, and deletion criteria.

---

## 1. The thesis: eighteen bets

### B1 — The semantic triptych

**DESIGN.** Model, Program, and Proof are peers. No corner is a generated afterthought of another.

This resolves the central false dichotomy in formal systems engineering:

- a separate abstract model drifts from code;
- one executable program is too concrete to verify globally.

Continuum instead maintains explicit, checkable refinement edges among multiple grains of model and implementation.

### B2 — Corpus-governed expressiveness

**DESIGN.** The 80 CI-validated TLA+ Examples families are the mandatory compatibility corpus for Continuum 1.0. The 39 currently unvalidated or external examples are tracked as an extended horizon rather than silently counted as complete.

The corpus governs language and engine priorities. A proposed language feature is promoted when it unlocks real corpus families or real Rust systems, not because it is elegant in isolation.

### B3 — Causal IR as concrete semantic interchange

**DESIGN.** Every controlled execution emits a versioned CIR containing:

- events and stable identities;
- causality, conflict, and interval constraints;
- task, node, region, and ownership context;
- reads, writes, resources, messages, and synchronization;
- reserve/commit/abort and durability stages;
- obligation creation, transfer, discharge, and leak;
- faults, cancellation phases, and recovery epochs;
- active observers and source provenance.

CIR is not the sole semantics. It is the canonical interchange for concrete and causal analyses.

### B4 — Asupersync as the concrete execution algebra

**FACT/DESIGN.** Asupersync already makes task ownership, regions, cancellation request → drain → finalize, explicit capabilities, obligations, two-phase effects, virtual time, deterministic Lab execution, replay, chaos, snapshots, and causal traces structural.

Continuum builds above that substrate instead of recreating a second async runtime. The adapter is public, versioned, and isolated so Continuum is not defined by private runtime details.

### B5 — Standalone mathematical models

**DESIGN.** CML models can be written before any network call or Rust task exists. A model may treat a transaction, disk, queue, or concurrent component as one atomic relation. The language must preserve the freedom to choose the right abstraction rather than forcing actor or task structure into every model.

### B6 — Zoomable refinement graphs

**DESIGN.** A project contains a graph, not one model:

```text
Service contract
      ↓
Protocol model
      ↓
Operational model: queues, timers, retries, persistence
      ↓
Asupersync implementation
      ↓
Observed production executions
```

Every edge declares state relation, event projection, hidden/stuttering steps, assumptions, fairness, fault mapping, and evidence status.

### B7 — Observer-indexed true concurrency

**DESIGN/HYPOTHESIS.** Independence is not global. Two events may commute for a state invariant and fail to commute for an audit-order property, liveness monitor, durability observer, or security hyperproperty.

Continuum indexes independence by an observer/property contract and orders observers in a refinement lattice. Coarser observers can justify stronger reduction only through a checked factorization relation.

### B8 — Cancellation and obligation calculus

**DESIGN/HYPOTHESIS.** Cancellation is a protocol, not disappearance:

```text
Running → CancelRequested → Draining → Finalizing → Cancelled
```

Reservations, commits, aborts, replies, permits, and cleanup obligations are linear resources. Continuum models conservation, transfer, and discharge, and proves cancellation-aware refinement rules for covered primitives.

### B9 — Adaptive verification portfolio

**DESIGN.** One semantics feeds multiple algorithms:

1. deterministic simulation and replay;
2. coverage-guided randomized schedules and faults;
3. stateless source/optimal DPOR;
4. stateful POR;
5. unfoldings and complete finite prefixes;
6. explicit-state search;
7. BDD/MDD and antichain methods;
8. SMT bounded model checking;
9. IC3/PDR and CHC solving;
10. counter abstraction and cutoff inference;
11. liveness SCC, ranking, and game solving;
12. timed zones and probabilistic/MDP analysis;
13. interactive or generated Lean proof.

Heuristics and agents allocate work. They do not determine assurance.

### B10 — Proof-carrying results

**DESIGN.** Strong successes carry evidence: closure sets, inductive invariants, simulation relations, source-set coverage witnesses, symmetry automorphisms, liveness rankings, fair-SCC exclusions, SAT/SMT/PB certificates, or theorem receipts.

Counterexamples carry replayable, minimized causal witnesses. “Verified” without a typed evidence class is prohibited.

### B11 — Lean as certificate authority

**DESIGN.** Lean defines the metatheory and imports large certificates by reflection. The model checker may be highly optimized and untrusted; a smaller checker proves that accepted evidence implies the user-facing claim.

The hard trust boundary includes verified encodings from Continuum semantics to solver problems, not merely a SAT certificate about an unrelated CNF.

### B12 — Production conformance without invented total order

**DESIGN.** Production evidence is a partial order with intervals, clocks, causality, and missing observations. Continuum solves for a legal completion against the specified view and returns one of:

- `Valid` within an explicit observation contract;
- `Invalid` with a minimal causal incompatibility;
- `Inconclusive` because evidence cannot decide the property;
- `SemanticsMismatch` because the trace and model epochs differ;
- `ResourceLimit` with retained partial evidence.

### B13 — Nominal and orbit-finite verification

**HYPOTHESIS.** Protocol state often explodes because dynamically allocated request IDs, transaction IDs, and node names are treated as concrete data. Continuum develops a nominal lane where equivariant systems are represented up to finite permutations and fresh-name alpha-equivalence.

The practical first implementation is canonical first-occurrence renaming plus group-action witnesses. Full orbit-finite automata remain gated research.

### B14 — Assumption synthesis as games

**HYPOTHESIS/TARGET.** Fairness, network, crash, and environment assumptions are often the real specification. Continuum treats system and environment choices as games and synthesizes maximal-permissive safety contracts or weakest useful progress assumptions.

The result can become:

- a named model assumption;
- a domain-pack policy;
- a production monitor;
- an explanation of why liveness cannot be guaranteed.

### B15 — Proof-producing transformations

**DESIGN.** Every semantic transformation declares whether it preserves equivalence, simulation, trace language, a property slice, satisfiability, or only counterexamples. The chain is attached to every result:

```text
source model
 → elaborated core
 → finite instance
 → property slice
 → symmetry/nominal quotient
 → POR/unfolding
 → solver encoding
 → certificate
 → Lean theorem receipt
```

### B16 — Algebraic multi-analysis

**HYPOTHESIS.** Many graph analyses share one traversal over different semirings or quantales:

- Boolean reachability;
- tropical shortest counterexamples;
- natural-number witness multiplicity;
- probability and expected cost;
- provenance polynomials;
- min-max game values;
- security or evidence lattices.

Continuum builds a typed algebraic analysis layer where the algebra determines the claim class. It does not blur probabilistic evidence into proof.

### B17 — Model-generated implementation scaffolding

**TARGET.** CML role/action declarations generate Rust message enums, typed endpoint skeletons, instrumentation IDs, trace schemas, and runtime monitors. Projection is checked against the global protocol model, drawing from communicating automata and choreographic programming.

Generation reduces drift; explicit refinement still proves semantic correspondence.

### B18 — Agent-native evidence loops

**DESIGN.** Agents receive structured proof states, causal crashpacks, semantic diffs, unmet obligations, assumption changes, and neighboring unexplored classes. They may propose code, models, invariants, rankings, and proofs.

Acceptance requires machine-checkable evidence. An agent may not silently weaken a property or strengthen an assumption.

---

## 2. Constitutional invariants

These are release-blocking constraints rather than aspirational guidance.

### INV-001 — No ambient nondeterminism in controlled code

Time, entropy, scheduling, network, storage, process lifecycle, cancellation, identity allocation, and external-service behavior must pass through explicit capabilities or named host boundaries.

Enforcement uses:

- API shape;
- rustc/Clippy lints;
- MIR call-graph auditing;
- Lab traps;
- dependency capability manifests;
- production instrumentation coverage.

### INV-002 — No self-certification

A strong claim must cross at least one independent semantic path. Examples:

- optimized Rust explorer → compact certificate → Lean checker;
- asupersync execution → CIR → standalone refinement checker;
- native CML model → bounded graph → differential TLA+ oracle;
- generated solver formula → solver proof → verified encoding theorem.

### INV-003 — Exact state identity in proof lanes

Fingerprints may index states, but a success result cannot depend on assumed collision absence. Exact canonical encodings or collision resolution are mandatory.

### INV-004 — Replay stability is part of correctness

A crashpack identifies semantic epoch, model closure, domain-pack closure, implementation artifact, observer set, scheduler/fault choices, and normalization rules. Replaying under the same closure must reproduce the same semantic verdict.

### INV-005 — Independence is a checked obligation

A false independence relation can erase the only failing schedule. Domain packs supply conservative footprints. Dynamic or observer-specific independence must emit evidence sufficient for an independent checker.

### INV-006 — Fairness is explicit and scoped

Liveness results list scheduler, action, network, process, cancellation, recovery, and environment assumptions. Weak and strong fairness attach to named actions or action sets, not an undifferentiated “fair runtime.”

### INV-007 — Inconclusive is first-class

Missing telemetry, underconstrained clocks, unknown foreign calls, unsupported semantics, or exhausted resources do not become passes.

### INV-008 — The corpus is not gamed

A corpus port may not hard-code expected answers, replace a protocol with a finite lookup table, or weaken the source claim. Ports are reviewed against source intent, differential behavior, mutations, and independent proofs.

### INV-009 — Proof receipts expose axioms and epochs

Every Lean-backed result records theorem names, imported modules, `#print axioms` output, Lean version, Continuum semantic epoch, certificate hash, and encoding hash.

### INV-010 — Foreign tools remain Tribunal-only dependencies

TLC, SANY, PlusCal, Apalache, TLAPS, and foreign proof tools may generate oracle evidence and migration artifacts. No release binary silently invokes them to complete a native Continuum verification claim.

### INV-011 — Experimental mathematics earns its place

Every speculative lane has:

- a baseline;
- a preservation statement;
- benchmark corpus;
- promotion threshold;
- maintenance budget;
- kill criterion.

### INV-012 — Property and assumption changes are privileged

Agents and automatic repair may freely change implementation code in a repair branch. Changes to properties, abstraction maps, fault models, observer sets, or assumptions require explicit semantic diff review.

---

## 3. Product contract and primary workflows

### 3.1 Standalone design

```bash
continuum new-model consensus
continuum check consensus.ctm --property agreement
continuum explore consensus.ctm --nodes 3 --faults omission,crash
continuum prove consensus.ctm --property agreement
```

The model exists with no Rust implementation.

### 3.2 Real-code deterministic verification

```bash
cargo continuum explore --package raft --scenario partition-heal
cargo continuum replay crashpacks/4b2f...
cargo continuum explain crashpacks/4b2f... --property leader-completeness
```

The same protocol logic runs under asupersync production and Lab worlds.

### 3.3 Refinement

```bash
continuum refine protocol.ctm \
  --implementation crate::raft \
  --view crate::verification::abstract_state
```

The result reports checked concrete steps, stuttering classes, uncovered effects, assumptions, and evidence level.

### 3.4 Production conformance

```bash
continuum observe trace.cir \
  --against protocol.ctm \
  --observer client-visible,durability
```

The result never assumes a total order that telemetry did not establish.

### 3.5 Proof and certificates

```bash
continuum verify model.ctm --assurance exhaustive --emit-receipt
continuum proof check result.cproof
continuum proof lean result.cproof --theorem MySystem.agreement
```

### 3.6 Corpus development

```bash
continuum corpus status tla-examples
continuum corpus run TV-009
continuum corpus diff TV-009 --oracle tlc
continuum corpus mutate TV-019
```

### 3.7 Agent repair

```bash
continuum agent bundle crashpacks/4b2f... --output repair-context/
continuum replay repair-context/replay.toml
continuum verify --neighboring-classes repair-context/neighborhood.json
```

---

## 4. The semantic triptych in detail

### 4.1 Model plane

The Model plane describes mathematical behavior. Its authoritative artifacts are:

- CML source;
- elaborated typed core;
- module/constant closure;
- transition and behavior semantics;
- temporal properties and fairness;
- refinement declarations;
- source provenance.

The reference evaluator favors simplicity and determinism over speed. Optimized engines consume the same core but do not define its meaning.

### 4.2 Program plane

The Program plane contains real Rust and effect-pack semantics:

- asupersync tasks, regions, scopes, capabilities, and obligations;
- virtual/production time;
- network and storage packs;
- crash/restart and epoch semantics;
- local synchronization and optional weak-memory events;
- instrumentation and CIR emission.

Production and Lab execution share protocol code. Host boundaries are named and classified.

### 4.3 Proof plane

The Proof plane contains:

- Lean definitions of the semantic core;
- preservation theorems;
- reflective certificate checkers;
- theorem ports for proof-bearing corpus examples;
- proof receipts and axiom manifests;
- optional independent Lean environment checking.

### 4.4 Edges, not slogans

Every connection is a typed edge:

```text
ModelPortParity
ModelRefinement
ProgramRefinement
TraceConformance
CertificateJustification
TransformationPreservation
```

Each edge has a schema, evidence state, assumptions, semantic epoch, and checker.

---

## 5. Continuum Model Language

### 5.1 Design principles

CML must be:

- abstract enough for protocol design before implementation;
- typed enough to prevent accidental nonsense and drive backends;
- relational enough to express nondeterministic next-state actions;
- temporal enough to express fairness and liveness;
- modular enough for refinement and reusable protocol libraries;
- compilable to a small core with a Lean definition;
- agent-legible and formatter-stable;
- capable of expressing semantic equivalents of all validated TLA+ examples.

### 5.2 Authoring surfaces

#### Relational surface

```text
model DieHard {
  state {
    big: Nat where 0 <= big && big <= 5
    small: Nat where 0 <= small && small <= 3
  }

  init { big == 0 && small == 0 }

  action FillBig { big' == 5 && unchanged(small) }
  action FillSmall { small' == 3 && unchanged(big) }
  action SmallToBig {
    let next_big = min(big + small, 5)
    big' == next_big
    small' == small - (next_big - big)
  }
  // ...

  spec { init && always(step(Next) || stutter(state)) }
  invariant TypeOK { big in 0..5 && small in 0..3 }
}
```

#### Procedural surface

A PlusCal-like layer provides processes, labels, atomic blocks, nondeterministic choice, await, and fairness annotations. It lowers to relational actions and an explicit program counter. The lowering emits a translation-validation witness.

#### Rust surface

Attributes and derives connect concrete types and actions to CML/CIR, but Rust is not required to write an abstract model.

### 5.3 Value system

The core supports:

- booleans, bounded and mathematical integers/naturals;
- uninterpreted finite atoms/model values;
- finite sets and symbolic set predicates;
- tuples, records, variants, options, sequences;
- total finite functions and maps;
- relations and graphs;
- bounded quantification and comprehensions;
- higher-order operators in the Theorem or restricted executable fragments;
- recursive definitions with termination/productivity classification;
- deterministic witness selection only where semantics specifies it;
- nondeterministic existential choice as a relation, not an implementation RNG call.

### 5.4 Action semantics

CML actions are relations over current and next states. The core includes:

- primed state references;
- `unchanged` and frame inference;
- existential action parameters;
- enabledness;
- action composition;
- stuttering closure;
- action labels and subaction identity;
- atomicity boundaries;
- observer-visible and internal actions;
- fault and environment ownership.

Frame inference is checked and materialized in the elaborated core. Omitted next-state assignments never mean “whatever the backend finds convenient.”

### 5.5 Temporal semantics

The temporal core separates finite traces from infinite behaviors. It supports:

- invariants and action invariants;
- always/eventually;
- until/release;
- leads-to, response, recurrence, and stability;
- weak and strong fairness over named actions;
- justice/compassion and generalized Büchi/Streett acceptance;
- finite-trace interpretations where explicitly requested;
- stuttering-invariant LTL without next as the default refinement logic;
- HyperLTL-style relational properties in a separate fragment.

LeanLTL is evaluated as a library and interoperability target, but Continuum owns a minimal semantic definition so product meaning is not inherited accidentally from a fast-moving dependency.

### 5.6 Semantic fragments

Elaboration computes required capabilities:

| Fragment | Meaning | Initial backends |
|---|---|---|
| `Finite` | fully enumerable finite state/value semantics | reference BFS, explicit Rust engine, certificates |
| `Symbolic` | solver-representable transition constraints | SMT BMC, CHC, IC3/PDR |
| `Temporal` | infinite behavior, fairness, omega acceptance | SCC/lasso, parity/Streett, Lean proof |
| `Probabilistic` | distributions, schedulers, MDPs/games | statistical lane, exact rational MDP lane |
| `Theorem` | propositions requiring proof rather than execution | Lean |
| `Runtime` | concrete controlled effects and CIR | asupersync adapter, Lab, conformance |
| `WeakMemory` | reads-from/coherence/happens-before execution graphs | SAT/SMT, dedicated checker |
| `Hyper` | sets/relations of executions | self-composition, relational solver, Lean |

Fragments compose only through declared bridge rules.

### 5.7 Modules and refinement

Modules support parameters, imports, instantiation, definition overrides, local definitions, and theorem namespaces. Refinement declarations are first-class and may use:

- functional abstraction maps;
- relations;
- event projections;
- history and auxiliary variables;
- prophecy variables;
- quotient relations;
- compositional assume-guarantee contracts.

### 5.8 Model configuration

The native configuration format includes equivalents for corpus-used TLC concepts:

- constant assignments and finite bounds;
- definition overrides;
- invariant/action/temporal properties;
- state/action constraints;
- symmetry declarations;
- observer views;
- trace aliases/renderers;
- deadlock policy;
- expected result class;
- exhaustive, simulation, generation, and symbolic modes.

The format is explicit, typed, and hashable. UI launch files are migration inputs, not semantic authorities.

---

## 6. Semantic core

### 6.1 Transition systems

At minimum:

```text
System S :=
  State
  Init : State → Prop
  Step : Label → State → State → Prop
  Observe : Observer → execution → observation
  Assumptions : behavior → Prop
```

Reachability is the reflexive-transitive closure of `Step`. Behaviors are infinite state/label streams satisfying initialization and step/stutter constraints.

### 6.2 Event structures and configurations

Concrete executions elaborate to event structures:

```text
EventStructure :=
  E          event identities
  ≤          causality partial order
  #          hereditary conflict
  label      semantic event labels
  footprint  resources/effects
  interval   timing constraints
  owner      task/node/region/environment
```

A configuration is finite, conflict-free, and downward closed. Total schedules are linear extensions, not primary semantics.

### 6.3 Observers

An observer maps executions or configurations to an observation domain. Examples:

- final abstract state;
- client call/return history;
- durability acknowledgements;
- audit event sequence;
- security labels;
- temporal proposition valuation.

Observer refinement is factorization:

```text
O_coarse = h ∘ O_fine
```

This induces a lattice/preorder used by reduction and conformance.

### 6.4 Observer-indexed independence

For enabled events `e` and `f`, independence under observer `O` requires a checked form of:

1. both execution orders exist;
2. both reach equivalent abstract configurations;
3. `O(e;f) = O(f;e)`;
4. relevant enabledness, fairness, obligation, fault, and time conditions are preserved;
5. neither event changes whether the other is classified as visible or required.

Static footprints provide a conservative sufficient condition. Dynamic independence can be stronger but must carry a witness.

### 6.5 Refinement

The initial safety rule is stuttering simulation:

```text
InitC(c) ⇒ InitA(α(c))
StepC(c,c') ⇒ α(c)=α(c') ∨ StepA(α(c),α(c'))
```

The complete program includes:

- relational forward/backward simulation;
- event refinement;
- fair simulation;
- prophecy/history variables;
- trace inclusion;
- observational and hyperproperty-preserving refinement;
- compositional refinement under assumptions.

### 6.6 Cancellation and obligations

The semantic state tracks:

- lifecycle phase;
- region tree;
- owned tasks;
- outstanding obligations;
- reserved effects;
- commit/abort status;
- cleanup budget and responsiveness assumptions.

Core invariants include:

- no orphan task after region quiescence;
- every reservation commits or aborts;
- obligation ownership is unique unless explicitly fractional;
- committed effects are never reported as aborted;
- acknowledgements respect durability stage;
- finalization does not introduce new unbounded obligations.

### 6.7 Time

Time is typed:

- logical time and causal clocks;
- virtual monotonic runtime time;
- dense or discrete model clocks;
- bounded uncertainty intervals;
- real-time constraints.

Timed semantics uses zones/DBMs for appropriate fragments and explicit constraints elsewhere. Wall-clock values never silently become logical ordering.

### 6.8 Probability and nondeterminism

Demonic/angelic/environment choice is distinct from probability. A probabilistic model declares distributions; an MDP declares scheduler ownership. Statistical campaigns yield calibrated evidence, not universal proof. Exact rational analysis is used where feasible.

### 6.9 Weak memory

Local concurrency may require semantics below async scheduling. The WeakMemory fragment represents execution graphs with:

- program order;
- reads-from;
- modification/coherence order;
- from-read;
- synchronizes-with and happens-before;
- atomic ordering constraints;
- compiler/hardware model identity.

The first product lane supports SC atomics and explicit synchronization. C11/Rust axiomatic models are a dedicated solver-backed lane, informed by Loom and RustMC but not conflated with distributed scheduling.

---

## 7. CIR and effect packs

### 7.1 CIR event shape

A CIR event contains stable, canonical fields:

```text
id, semantic_epoch, pack_epoch
node, process, task, region
kind, phase, owner
causes, conflicts, intervals
reads, writes, messages, resources
obligations_created/transferred/resolved
reserve/commit/abort
volatile/submitted/synced durability
fault and recovery context
observer projections
source and implementation provenance
```

### 7.2 Canonicalization

Canonicalization normalizes:

- identifiers under declared symmetry/nominal rules;
- independent event order using Foata-like forms;
- map/set ordering;
- timestamps into constraints rather than raw incidental values;
- source locations and build paths;
- semantic versions and dependency closure.

### 7.3 Domain-pack contract

Every pack defines:

- operations and typed outcomes;
- lifecycle and cancellation behavior;
- event labels;
- fault algebra;
- independence footprints;
- observers;
- abstraction views;
- fidelity matrix;
- conformance corpus;
- proof obligations;
- semantic versioning policy.

Initial packs:

1. virtual network;
2. durable storage/WAL;
3. process crash/restart;
4. virtual clocks/timers;
5. entropy and fresh names;
6. channels and synchronization;
7. database transaction isolation;
8. object store;
9. identity protocols as later real-world packs.

### 7.4 Storage semantics

Storage packs model explicit stages:

```text
application write
 → userspace buffer
 → kernel/device submission
 → media durability
 → recovery visibility
```

Pack profiles declare atomicity, ordering, torn-write, fsync, rename, directory-sync, cache, and crash guarantees. “Disk” is never one universal mock.

---

## 8. Asupersync integration

### 8.1 Adapter boundary

`continuum-asupersync` depends only on a documented semantic surface:

- task/region lifecycle callbacks;
- capability operations;
- scheduler choices;
- virtual time;
- channel/synchronization events;
- obligation and cancellation events;
- snapshot/replay hooks.

Private Lab structures do not leak into Continuum schemas.

### 8.2 Production/Lab equivalence

The same protocol code executes in both modes. Differences are isolated in effect implementations and scheduler choice. The adapter emits a coverage report listing operations not represented in CIR.

### 8.3 Replacing bespoke DST

Continuum plus asupersync replaces common framework code:

- deterministic scheduler;
- virtual time and entropy;
- network fault orchestration;
- crash/restart orchestration;
- snapshots and replay;
- schedule exploration;
- invariant hooks;
- trace capture and minimization;
- scenario format.

Project-specific fake services become domain packs or fixtures, not new simulation frameworks.

### 8.4 Foreign runtime support

Tokio and synchronous adapters may exist for migration and observation, but full assurance is lower unless nondeterminism and lifecycle semantics are covered. Asupersync is the reference concrete substrate.

---

## 9. TLA+ Examples Tribunal

### 9.1 Scope

The mandatory corpus consists of the 80 CI-validated families listed in `corpus/tla-examples/validated-examples.csv`. The extended set contains 39 external or unvalidated families in `other-examples.csv`.

The source commit is immutable within a corpus epoch. Upstream movement creates a new epoch and explicit diff.

### 9.2 Parity levels

- **P0 — Inventoried:** source closure, features, models, expected outcomes known.
- **P1 — Native semantic port:** CML model parses, elaborates, and runs.
- **P2 — Bounded behavioral parity:** selected finite configurations match initial states, transitions/reachability facts, verdicts, and shortest failures modulo declared relation.
- **P3 — Temporal/refinement parity:** fairness, liveness, deadlock, symmetry, views, or refinement claims match.
- **P4 — Theorem parity:** proof-bearing intent is represented and machine-checked in Lean; proof scripts need not be textually translated.
- **P5 — Runtime refinement exemplar:** a real asupersync implementation refines the model under explicit assumptions.

Every validated family has a required minimum in the CSV. Continuum 1.0 requires all 80 at or above that level.

### 9.3 Per-port manifest

Each port records:

- pinned source paths and hashes;
- feature census;
- source models/configurations;
- source properties and expected outcomes;
- CML modules/configurations;
- semantic relation between source and port states/actions;
- foreign oracle commands and artifact hashes;
- exact/bounded graph facts;
- liveness/fairness facts;
- proof theorem names and receipts;
- mutations and expected detection;
- runtime exemplar status;
- known intentional differences.

### 9.4 Differential checks

Where feasible, the Tribunal compares:

- acceptance/rejection and diagnostics;
- initial-state sets;
- successor sets for sampled or exhaustive states;
- reachable state counts;
- total generated transitions;
- depth and shortest counterexample;
- invariant/deadlock/liveness result class;
- lasso/SCC shape;
- symmetry quotient facts;
- refinement verdicts;
- simulation/generation reproducibility.

Comparison is modulo an explicit state/action relation, not raw serialization.

### 9.5 Mutation Tribunal

Every port receives semantic mutants:

- omitted frame condition;
- wrong guard;
- swapped process index;
- missing fairness;
- overstrong fairness;
- premature acknowledgement;
- lost durability edge;
- incorrect quorum threshold;
- duplicated/lost message;
- stale epoch acceptance;
- weakened property;
- strengthened assumption.

The suite must show which engine catches each mutant and whether the corpus expectation remains discriminating.

### 9.6 Porting waves

- **Wave 0:** finite puzzles, arithmetic, proof tutorials, basic concurrency.
- **Wave 1:** mutual exclusion, barriers, readers/writers, auxiliary variables.
- **Wave 2:** termination, reachability, graph algorithms, fairness.
- **Wave 3:** Paxos, transactions, Byzantine protocols, checkpointing, refinement.
- **Wave 4:** storage, TCP, B-trees, lock-free/disruptor, cache coherence, complex operational models.
- **Wave 5:** language-level checking and unusual/meta examples.

Waves are dependency order, not a promise to defer correctness.

### 9.7 Corpus-derived libraries

The goal is not 80 isolated ports. Reusable libraries should emerge:

- finite puzzles and search;
- mutual exclusion and process symmetry;
- graphs/rings/trees;
- quorum systems;
- broadcast and Byzantine thresholds;
- termination/ranking;
- transaction commit;
- storage and snapshots;
- network sequence spaces/TCP;
- cache coherence;
- refinement auxiliaries;
- proof patterns.

### 9.8 Optional TLA+ importer

A future importer may translate a useful TLA+ subset to CML for migration. It is not required for corpus parity and does not define native semantics. SANY/XML/ITF adapters remain Tribunal tools until independently validated.

---

## 10. Verification engines

### 10.1 Reference engine

A deliberately simple exact engine provides:

- deterministic BFS/DFS;
- exact values and state identity;
- shortest traces;
- closure certificates;
- transparent diagnostics;
- differential oracle support.

It is the executable specification of finite semantics, not the performance engine.

### 10.2 Explicit-state engine

Design targets:

- compact canonical state encoding;
- structural sharing and hash-consing where measured;
- delta successor representation;
- exact collision resolution;
- deterministic parallel frontiers;
- in-memory and external-memory modes;
- symmetry/nominal canonicalization;
- certificate emission.

A bulk-synchronous deterministic lane is favored for exhaustive certification; a work-stealing lane is favored for fast bug discovery.

### 10.3 DPOR

The promotion sequence is:

1. conservative source-DPOR;
2. await-aware DPOR for async tasks;
3. optimal/parsimonious DPOR;
4. observer-indexed independence;
5. property-sensitive stateful POR;
6. proof-producing source-set coverage.

Each stage is benchmarked against no reduction and a high-quality baseline. The project does not assume fewer schedules always means faster verification.

### 10.4 Unfoldings and higher-dimensional concurrency

Petri-net/event-structure unfoldings may produce complete finite prefixes. Higher-dimensional automata/cubical structures may quotient jointly independent event families more strongly than pairwise trace equivalence.

These remain research lanes until they provide:

- a formal preservation theorem;
- a checkable completeness witness;
- consistent wins on high-width real protocols;
- manageable certificate size.

### 10.5 Symbolic lane

The symbolic core supports:

- bounded model checking;
- k-induction;
- inductive invariant checking;
- IC3/PDR;
- CHC generation;
- bit-vector and integer theories;
- arrays/maps under explicit encodings;
- proof-producing SAT/PB lanes;
- SMT proof import where reliable, otherwise translation-validated replay.

### 10.6 Parameterized lane

Techniques include:

- finite permutation symmetry;
- symmetry-to-quantification invariant inference;
- counter abstraction;
- cutoff discovery and proof;
- monotonic abstraction;
- well-structured transition systems and coverability;
- regular-language abstractions for arrays/topologies;
- nominal/orbit-finite representations.

Every result names the theorem or cutoff rule that lifted finite evidence to an unbounded claim.

### 10.7 Liveness and games

Finite-state liveness uses:

- nested DFS/SCC decomposition;
- generalized Büchi/Streett/parity acceptance;
- fair-lasso witnesses;
- SCC exclusion certificates;
- ranking/progress measures;
- turn-based and concurrent games for environment assumptions.

Liveness counterexamples distinguish an actual fair cycle from an unfair stuttering artifact.

### 10.8 Timed and probabilistic lanes

Timed automata use DBMs/zones where semantics fits. Probabilistic models use exact rational MDP/game analysis where bounded and statistical campaigns otherwise. Statistical results include confidence/e-process evidence and remain below proof assurance.

### 10.9 Hyperproperty lane

Properties over multiple executions—linearizability, noninterference, observational determinism, consistency—use:

- self-composition/product models;
- relational observers;
- partial-order history completion;
- strong refinement rather than ordinary trace inclusion where required.

---

## 11. Lean 4 formalization and certificate program

### 11.1 Role

Lean is the mathematical court of final appeal, not the normal search engine. The initial pin is `v4.32.1`; upgrades occur through proof epochs.

### 11.2 Layered metatheory

```text
M0  finite mathematics, relations, orders, maps, group actions
M1  transition systems, reachability, invariants
M2  finite/infinite traces, LTL, fairness, omega acceptance
M3  refinement, stuttering, event projection, composition
M4  event structures, configurations, observers, independence
M5  obligations, cancellation, durability, faults
M6  reductions and certificate checkers
M7  CIR/model serialization and semantic bridge theorems
M8  corpus theorem libraries and project-specific proofs
```

Dependencies point downward.

### 11.3 Initial theorem ladder

1. reachable-state induction;
2. finite closure certificate soundness;
3. invariant certificate soundness;
4. stuttering simulation maps reachable states;
5. simulation composition;
6. fair-lasso validation;
7. accepting-SCC exclusion soundness;
8. symmetry automorphism quotient preservation;
9. observer-refinement monotonicity of independence;
10. source-set/DPOR safety preservation;
11. cancellation phase/rank progress;
12. obligation conservation;
13. durability acknowledgement refinement;
14. nominal renaming invariance;
15. solver-encoding bridge theorems.

### 11.4 Reflection

Large certificates are checked by compiled Boolean functions whose soundness is proved in Lean:

```text
check certificate instance = true
────────────────────────────────
theorem about Continuum semantics
```

The architecture follows current successful Lean work on LRAT and pseudo-Boolean certificate import: execution happens in efficient compiled code, while the result becomes a composable Lean theorem.

### 11.5 Proof receipts

A proof receipt includes:

- claim and assurance class;
- model/property/assumption hashes;
- semantic and proof epochs;
- transformation chain;
- certificate format and hash;
- Lean theorem names;
- imported modules;
- axiom manifest;
- checker binary/source hashes;
- optional independent kernel result.

### 11.6 LeanLTL and external libraries

LeanLTL provides useful current work on finite/infinite LTL in Lean. Continuum should interoperate and reuse proven results where semantics align, while retaining a small owned definition and equivalence theorems.

### 11.7 Independent proof checking

Lean4Lean or another external checker may validate theorem environments for high-assurance release artifacts. This adds implementation diversity but does not replace semantic proof obligations.

### 11.8 Rust-to-Lean extraction

Aeneas/Hax/Charon-style extraction is evaluated for pure certificate checkers, encoders, and data-structure kernels. Async runtime code is not assumed extractable. The project prefers small verified checkers over formalizing the entire optimized engine.

### 11.9 Proof engineering rules

- no `sorry` in release packages;
- axioms are machine-reported;
- theorem APIs are semantic-epoch versioned;
- proofs are sliced by action/property dependencies;
- generated proof blueprints expose dependencies to agents;
- proof performance is benchmarked;
- unstable implementation detail stays outside theorem statements;
- per-instance translation validation is favored for rapidly changing optimizers.

---

## 12. Proof-producing transformation chain

### 12.1 Elaboration

The frontend emits typed core plus a source correspondence map. The reference evaluator and Lean semantics consume the same serialized core schema through independent implementations.

### 12.2 Finite instantiation

Unbounded parameters become a finite model only with an explicit instantiation record. If a cutoff theorem exists, the receipt links it; otherwise the result remains bounded.

### 12.3 Property slicing

A dependency slice records retained variables, actions, observers, fairness clauses, and assumptions. Preservation is checked by static theorem or per-instance validation.

### 12.4 Symmetry and nominal quotient

A quotient witness includes:

- group/name action;
- canonicalization function/version;
- proof or exhaustive witness that init, transitions, and properties are equivariant;
- orbit representative mapping.

### 12.5 POR/unfolding

A reduction witness includes dependency relation, backtracking/source sets, explored representatives, and coverage data. Initial certificate checkers target safety. Liveness preservation is a separate theorem and mode.

### 12.6 Solver lowering

Encoding receipts connect core formulas to SAT/PB/SMT instances. A solver proof without an encoding theorem cannot produce the strongest claim class.

### 12.7 Translation validation

For transformations too complex or unstable to verify once and for all, a validator checks this transformation instance and emits a theorem/certificate. This is the default strategy for aggressive optimization.

---

## 13. Novel mathematical programs

### 13.1 Causal abstract interpretation

Define Galois connections between concrete configuration domains and abstract state domains. Use abstract transformers to obtain sound overapproximations, then refine around spurious failures through CEGAR.

Product targets:

- automatic state slicing;
- coarse-to-fine mixed-grain verification;
- explanation of spurious counterexamples;
- inferred abstraction candidates for agents.

### 13.2 Combinatorial geometry of concurrency

Trace monoids, Foata normal forms, median graphs, CAT(0) cube complexes, hyperplanes, and directed homotopy may provide:

- stronger canonicalization;
- geometric schedule distance;
- minimal separators causing failures;
- high-dimensional coverage measures;
- compact counterexample medians.

No topological metric enters assurance without a preservation theorem.

### 13.3 Nominal sets

Fresh identifiers are atoms under finite permutations. Equivariant transitions can be represented by orbits rather than concrete names. First-occurrence canonicalization is the initial practical subset.

### 13.4 Games and assumption synthesis

Safety games synthesize forbidden environment moves. Büchi/Streett/parity games synthesize progress assumptions and strategies. The result can be minimized and translated into model clauses and runtime monitors.

### 13.5 Semiring and quantale analysis

A generic weighted fixed-point engine can instantiate reachability, shortest witnesses, witness counts, probabilities, costs, provenance, and min-max values. Each algebra has a typed interpretation and claim class.

### 13.6 Coalgebra and modal interfaces

Coalgebraic views may unify component behavior, bisimulation, and characteristic modal properties across transition, probabilistic, and timed systems. This is evaluated as a library-design tool before becoming user-visible theory.

### 13.7 Sheaf-like composition

Local models, proofs, and telemetry may overlap on shared interfaces. A global witness exists when local sections glue consistently. Minimal ungluable covers may provide precise cross-component diagnosis.

### 13.8 Choreographic projection

Global protocol actions can project to role-local communicating automata, message types, monitors, and asupersync skeletons. Projection soundness and multiparty compatibility become proof obligations.

---

## 14. Production conformance and observability synthesis

### 14.1 Evidence model

Production events include causal references, node epochs, intervals, logical clocks, operation IDs, durability markers, and observer projections. Missing events are represented as uncertainty, not absence.

### 14.2 Constraint-based alignment

Conformance solves for:

- event-to-model action mapping;
- hidden internal steps;
- legal linearization or partial-order embedding;
- abstraction states;
- clock/interval constraints;
- fault and recovery interpretation.

### 14.3 Decidability boundary

Some runtime-verification problems cannot be conclusively monitored under asynchronous, lossy, or crash-prone observation. The product therefore computes the strongest justified verdict and identifies missing evidence.

### 14.4 Instrumentation synthesis

When a verdict is inconclusive, Continuum computes candidate instrumentation changes:

- add correlation edge;
- expose reserve/commit boundary;
- record durability stage;
- add process epoch;
- tighten clock interval;
- observe an abstraction variable;
- add a client-visible event.

Candidates are ranked by decisiveness, runtime cost, data volume, and privacy exposure.

### 14.5 Production-to-Lab reproduction

A compatible partial-order trace can seed Lab execution. If multiple completions exist, Continuum explores the ambiguity set and returns either a reproducing witness or a proof that the observed failure depends on unobserved behavior.

---

## 15. Agent-native verification

### 15.1 Machine-legible artifacts

```text
crashpack/
  manifest.json
  semantic-closure.json
  causal-trace.cir
  minimized-trace.cir
  property.json
  assumptions.json
  state-diff.json
  proof-obligations.json
  unexplored-neighborhood.json
  replay.toml
  explanation.md
```

### 15.2 Proof blueprints

Lean work is represented as a dependency DAG of definitions, lemmas, open goals, automation attempts, and axiom status. Agents work on independent nodes and the kernel checks the result.

### 15.3 Counterexample-guided repair

The loop is:

1. replay exact failure;
2. extract minimal causal core;
3. generate implementation/model hypotheses;
4. patch implementation;
5. replay original witness;
6. explore neighboring equivalence classes;
7. rerun mutation and corpus regressions;
8. check semantic diff;
9. issue evidence receipt.

### 15.4 Anti-cheating controls

A repair is rejected or escalated when it:

- weakens a property;
- strengthens assumptions;
- removes an observer;
- changes fault semantics;
- reduces bounds;
- marks unsupported behavior atomic;
- suppresses instrumentation;
- changes semantic epoch without migration.

### 15.5 Learned guidance

Learned models may rank actions, invariants, abstractions, source sets, or proof lemmas. A complete fallback and evidence checker remain authoritative. Learned guidance never appears inside the trusted claim.

---

## 16. Repository and crate architecture

```text
continuum/
  crates/
    continuum-cli
    continuum-daemon
    continuum-model-syntax
    continuum-model-elab
    continuum-model-core
    continuum-model-reference
    continuum-cir
    continuum-cir-codec
    continuum-observer
    continuum-refinement
    continuum-effects
    continuum-asupersync
    continuum-domain-network
    continuum-domain-storage
    continuum-domain-process
    continuum-domain-time
    continuum-domain-database
    continuum-explore
    continuum-explicit
    continuum-dpor
    continuum-unfolding
    continuum-symbolic
    continuum-liveness
    continuum-games
    continuum-symmetry
    continuum-nominal
    continuum-weak-memory
    continuum-hyper
    continuum-transform
    continuum-certificate
    continuum-proof-receipt
    continuum-production
    continuum-corpus
    continuum-tla-oracle
    continuum-agent-protocol
    continuum-lsp
  lean/
    Continuum/Semantics
    Continuum/Temporal
    Continuum/Refinement
    Continuum/EventStructure
    Continuum/Cancellation
    Continuum/Symmetry
    Continuum/Certificates
    Continuum/Corpus
  corpus/
    tla-examples/
  schemas/
  benchmarks/
  examples/
  docs/
```

### 16.1 Dependency rules

- model core has no dependency on asupersync;
- reference semantics has no dependency on optimized engines;
- certificate checker has no dependency on search code;
- TLA oracle is never linked into normal release verification;
- production adapter depends on versioned CIR, not explorer internals;
- Lean definitions do not import generated theorem statements without a stable schema;
- experimental crates cannot become transitive dependencies of the kernel by accident.

### 16.2 Unsafe code

Unsafe is prohibited by default. Boundary crates require:

- explicit justification;
- Miri/sanitizer tests;
- local proof or external verification plan;
- no influence on certificate semantics without independent checking.

---

## 17. Executable spikes completed in revision 2

The Python reference spikes are intentionally small and independent of future Rust code.

### 17.1 Die Hard

- 16 reachable states;
- 96 labeled transitions;
- shortest four-gallon witness depth 6;
- exact finite closure/type certificate accepted by an independent checker.

### 17.2 Dining philosophers

- 573 reachable states for five philosophers;
- 2,365 transitions;
- shortest deadlock depth 10;
- exact closure/type certificate accepted.

### 17.3 Stuttering refinement

A reserve/commit/abort register exhaustively refines an atomic register: reserve and abort stutter; commit implements the abstract write.

### 17.4 Observer-indexed independence

Independent writes commute for state observers but not for an order-sensitive audit observer. Observer refinement monotonicity holds in the spike.

### 17.5 Cyclic symmetry

The five-philosopher graph quotients from 573 to 117 states under rotation, a 4.90× reduction. Exhaustive checking confirms rotation preserves the successor relation.

### 17.6 Fair lassos

A pure wait cycle violates eventual completion without fairness, is rejected under weak fairness of a continuously enabled completion action, and becomes a valid fair counterexample when completion is truly unavailable.

### 17.7 Assumption game

Exact subset search synthesizes one forbidden environment move for `Ack ⇒ Durable`: `AckBeforeSync`. Crash-before-ack remains permitted because retry can preserve safety.

### 17.8 Nominal names

Traces differing only by fresh identifier renaming canonicalize identically; causally different allocation/send orders remain distinct.

### 17.9 Semiring traversal

A product of tropical distance and natural-number multiplicity computes the six-step Die Hard solution and its number of shortest labeled witnesses in one traversal.

These are evidence that the architecture is executable. They are not Rust or Lean proofs.

---

## 18. Execution gates

### G0A — Semantic kernel feasibility

Required:

- CML core data model;
- independent reference evaluator;
- exact values/state encoding;
- Die Hard and Dining ports;
- closure and counterexample schemas;
- Rust/Lean semantic statement alignment document;
- no hidden nondeterminism in reference engine.

### G0B — Lean proof foundation

Required:

- Lean build pinned and reproducible;
- transition/reachability/invariant definitions compile;
- finite closure theorem;
- stuttering simulation theorem;
- proof receipt schema;
- zero `sorry`;
- axiom manifest in CI.

### G0C — Asupersync bridge

Required:

- one real protocol codebase runs in production and Lab;
- CIR events cover every relevant effect;
- cancellation and obligations preserved;
- stable crashpack replay;
- adapter uses only public semantic hooks.

### G0D — Corpus Tribunal

Required:

- automated inventory ingestion;
- pinned TLA oracle toolchain;
- port manifest schema;
- exact Die Hard parity;
- one PlusCal/procedural parity example;
- one liveness/fairness example;
- one proof example.

### G1 — First complete vertical slice

A three-node durable replicated register must demonstrate:

- standalone abstract model;
- asupersync implementation;
- network/storage/crash/cancellation packs;
- state exploration and DPOR;
- acknowledgement-before-durability mutant;
- minimized causal crashpack;
- stuttering refinement;
- Lean-checked finite/refinement receipt;
- exact replay.

### G2 — Corpus Wave 0 and language stability

- all Wave 0 ports at required parity;
- language formatter/LSP usable;
- semantic epoch 1 frozen;
- reference/optimized differential fuzzing;
- proof library for elementary invariants and finite search.

### G3 — DST replacement

- one existing project deletes most bespoke scheduler/time/fault/replay framework code;
- all known seeds and scenarios reproduce;
- no engine changes required for a second materially different project;
- only domain packs, models, views, and properties are added.

### G4 — Temporal/refinement maturity

- fair-lasso/SCC engine;
- weak/strong fairness semantics;
- refinement composition;
- auxiliary/history variables;
- corpus Waves 1–2;
- Lean liveness/refinement receipts for selected examples.

### G5 — Symbolic/parameterized and production bridge

- BMC and PDR/CHC lane;
- proof-producing SAT/PB path;
- symmetry/nominal witness checker;
- assumption games;
- production partial-order conformance with `Inconclusive`;
- corpus Waves 3–4 substantially complete.

### G6 — Continuum 1.0

- all 80 validated corpus families at required parity;
- every proof-bearing required port has Lean theorem receipts;
- selected runtime refinements span concurrency, consensus, storage, transaction, network, and cancellation domains;
- no critical semantic path depends on a foreign tool;
- performance and soundness claims have current evidence;
- independent replay/certificate validation works on release artifacts.

---

## 19. Workstreams

### W1 — Model syntax and elaboration

Deliver CML parser, formatter, diagnostics, type/fragment inference, module system, and typed core.

### W2 — Reference semantics

Deliver exact evaluator, state graph, finite behaviors, property evaluation, and differential harness.

### W3 — Lean metatheory

Deliver theorem ladder, reflective checkers, proof receipts, corpus proof library, and CI.

### W4 — CIR and asupersync

Deliver adapter, semantic journaling, replay, snapshots, cancellation/obligation mapping, and coverage.

### W5 — Explicit and causal exploration

Deliver deterministic explicit engine, DPOR, symmetry, nominal canonicalization, minimization, and certificates.

### W6 — Temporal and games

Deliver fairness, lasso/SCC, ranking, environment games, and assumption monitors.

### W7 — Symbolic and parameterized

Deliver SMT/BMC, PDR/CHC, cutoffs/counter abstraction, proof-producing encodings.

### W8 — Corpus Tribunal

Deliver inventory, port manifests, foreign oracles, differential tests, mutation suite, dashboards, and release gates.

### W9 — Domain packs

Deliver network, storage, process, time, synchronization, database, and weak-memory semantics.

### W10 — Production conformance

Deliver trace ingestion, partial-order alignment, observability analysis, and production-to-Lab replay.

### W11 — Developer and agent experience

Deliver CLI, daemon, LSP, visualizer, proof-state protocol, semantic diff, crashpack UX, and generated scaffolding.

### W12 — Evaluation and governance

Deliver claims matrix, benchmarks, threat model, semantic versioning, release evidence, and experimental-lane reviews.

---

## 20. Initial pull-request sequence

The first sequence is intentionally end-to-end rather than subsystem-complete.

1. Workspace constitution, semantic epochs, evidence types.
2. `continuum-model-core`: finite values, states, actions, labels.
3. Reference BFS and exact state codec.
4. Closure/counterexample certificate schema and checker.
5. Native Die Hard port with frozen facts.
6. Lean project compiling reachability and closure theorem.
7. CML parser for state/init/action/invariant subset.
8. Dining philosophers port and deadlock witness.
9. Temporal lasso core with weak fairness.
10. Observer/view definitions and independence contracts.
11. Cyclic symmetry action and witness checker.
12. Asupersync semantic adapter spike.
13. Virtual network and process packs.
14. Durable storage reserve/commit/sync pack.
15. Replicated-register abstract model and real implementation.
16. Crashpack replay with semantic hashes.
17. Concrete-to-abstract stuttering checker.
18. Lean simulation theorem and imported instance receipt.
19. Source-DPOR over asupersync choices.
20. Acknowledgement-before-durability mutant and minimized failure.
21. Corpus port schema and dashboard.
22. PlusCal-like procedural lowering with translation validation.
23. Wave 0 batch ports and differential oracle runner.
24. LSP/formatter/error explanations.
25. Release gate automation and evidence report.

No later PR may replace a checked path with an unverified optimization without differential and evidence coverage.

---

## 21. Benchmark and evaluation program

### 21.1 Dimensions

Measure:

- states/transitions/configurations per second;
- schedules and causal classes explored;
- bytes per exact state;
- peak RSS and external-memory I/O;
- core scaling and determinism;
- certificate generation/check time and size;
- counterexample depth and minimized size;
- mutation kill rate;
- proof maintenance and build time;
- instrumentation overhead;
- production conformance decisiveness;
- agent repair success without specification weakening.

### 21.2 Baselines

Depending on feature:

- TLC;
- Apalache;
- Quint simulator/test workflows;
- Stateright;
- Loom/Shuttle/MadSim/Turmoil;
- P;
- explicit no-reduction engine;
- source/optimal DPOR implementations;
- SAT/SMT solvers with independent certificates;
- Verus/Kani/RustMC for local code obligations.

### 21.3 Corpus metrics

The dashboard tracks:

- parity level by family;
- semantic features unlocked;
- exact graph parity;
- expected failure parity;
- theorem receipt status;
- mutation coverage;
- runtime exemplar status;
- performance versus source tool;
- intentional differences.

### 21.4 Reproducibility

Every published number includes hardware/software closure, command, seed or exhaustive scope, semantic epoch, corpus commit, and artifact hash.

---

## 22. Research promotion and kill criteria

### Observer-indexed DPOR

Promote when it is formally conservative and beats ordinary DPOR materially on audit/state/property-diverse workloads. Kill if observer checking dominates or semantics remains too fragile.

### Higher-dimensional/cubical reduction

Promote when it provides a checkable completeness witness and repeated wins on high-concurrency protocols. Kill if it remains only a visualization or duplicates unfolding performance with greater complexity.

### Nominal/orbit-finite lane

Promote when fresh-name workloads shrink substantially with a Lean-checked equivariance/canonicalization path. Kill full orbit machinery if first-occurrence canonicalization captures practical value.

### Assumption synthesis

Promote when synthesized assumptions are understandable, minimal enough, and monitorable. Kill automatic publication if results are opaque or overfit finite bounds.

### Semiring engine

Promote shared traversal only when it simplifies implementation and matches specialized algorithms. Kill abstraction if genericity obscures evidence or loses performance.

### Sheaf diagnosis

Promote only if it finds smaller, more actionable cross-component inconsistency cores than SAT unsat cores and causal slicing. Otherwise retain as research notes.

### Choreographic projection

Promote when generated Rust interfaces reduce drift and projection soundness is checkable. Kill if generated APIs fight normal Rust architecture.

### Learned guidance

Promote per engine when it improves bug/time curves without reducing completeness or reproducibility. Always keep a nonlearned fallback.

---

## 23. Security and trusted computing base

### 23.1 Threats

- malicious model/certificate causing parser or resource exhaustion;
- unsound domain-pack independence;
- hash collisions or noncanonical serialization;
- forged proof receipts;
- solver certificate/encoding mismatch;
- production trace tampering;
- semantic epoch confusion;
- agent-induced property weakening;
- unsafe/FFI bugs in the checker;
- dependency compromise.

### 23.2 TCB decomposition

Strong claims identify exactly what is trusted:

- Lean kernel and declared axioms;
- Continuum semantic definitions;
- verified decoder/checker and encoding theorem;
- cryptographic hash implementation and artifact closure;
- optional independent checker.

Search engines, foreign oracles, agents, and solvers are evidence producers.

### 23.3 Artifact integrity

Receipts are content-addressed and optionally signed. Production traces use chain/hash/Merkle integrity appropriate to deployment. Integrity does not create missing observability; it only authenticates evidence.

---

## 24. Adoption strategy

### 24.1 Wedge

The first compelling user story is:

> Find a cancellation/durability bug in real asupersync Rust, explain it causally, replay it exactly, and show that the repaired implementation refines a standalone model.

### 24.2 Internal migration

Migrate existing bespoke DST projects incrementally:

1. adopt effect packs and Lab execution;
2. emit CIR and crashpacks;
3. port invariants/scenarios;
4. introduce abstract views;
5. replace framework code;
6. add standalone model and refinement;
7. promote selected claims to Lean receipts.

### 24.3 External credibility

Credibility comes from:

- the public TLA+ corpus dashboard;
- reproducible spikes and benchmarks;
- honest `Inconclusive` results;
- independently checkable receipts;
- bugs found in real systems;
- concise, excellent developer experience.

### 24.4 Compatibility without dependency

Export/import targets include TLA+ behaviors, ITF-style traces, Quint traces, Graphviz, Chrome trace, OpenTelemetry, SAT/PB certificates, and Lean theorem receipts. Interoperability is encouraged; semantic sovereignty remains native.

---

## 25. Definition of success

Continuum succeeds when all of the following are true:

1. A user can design a protocol abstractly before implementation.
2. The abstract model supports safety, liveness, fairness, and refinement.
3. Real asupersync code runs under controlled and production semantics.
4. The implementation’s relationship to the model is explicit and checkable.
5. Counterexamples are causal, minimal, and exactly replayable.
6. Strong successes carry independent evidence.
7. Production traces can be validated without fake total order and can return `Inconclusive` honestly.
8. One framework replaces the repeated DST substrate across materially different projects.
9. Agents can repair failures through a proof-gated loop.
10. Every CI-validated TLA+ Examples family has a native equivalent at its required parity level.

The last criterion is the guardrail against self-congratulation. If Continuum cannot express and validate those examples, it has not replaced TLA+ in the practical sense claimed.

---

## 26. Explicit non-promises

Continuum does not promise:

- automatic proof of arbitrary unbounded distributed systems;
- exhaustive checking of unrestricted production Rust;
- perfect inference of abstractions or invariants;
- conclusive runtime verification from insufficient telemetry;
- one algorithm that dominates all others;
- that Rust alone creates performance;
- that formalizing the checker eliminates errors in the model;
- that a finite corpus proves universal expressiveness;
- that exotic mathematics is valuable without measured results.

It promises a coherent semantics, explicit boundaries, aggressive but checkable algorithms, and evidence strong enough to know what has and has not been established.

---

## 27. Immediate execution directive

Begin with the `START_HERE_IMPLEMENTATION.md` sequence and enforce the G0 gates before freezing broad APIs.

The first public artifact worth showing is not a language screenshot or architecture diagram. It is a complete failure story:

```text
abstract durability property
        ↓
real asupersync protocol
        ↓
controlled crash + cancellation
        ↓
observer-indexed causal exploration
        ↓
ack-before-sync violation
        ↓
9-event minimized crashpack
        ↓
exact replay
        ↓
repair
        ↓
neighboring-class exploration
        ↓
Lean-checked refinement/safety receipt
```

Then make the corpus dashboard move from red to green, one semantic family at a time.

That combination—real-code utility, mathematical abstraction, corpus accountability, and proof-carrying evidence—is the path to the intended micro-revolution.
