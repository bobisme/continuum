# Frontier Research Agenda

The research agenda separates foundational work from speculative work. Nothing in the speculative lane is required for the first useful product.

## Track A — Foundational semantics

### A1. Causal Intermediate Representation

Question: what is the smallest event/configuration structure that faithfully represents:

- asupersync lifecycle;
- state transitions;
- interval time;
- conflict and independence;
- storage durability;
- faults;
- observations;
- refinement views?

Deliverable: formal semantics, executable reference evaluator, mechanized core definitions, and conformance corpus.

### A2. Cancellation and obligation calculus

Develop a labeled transition system for:

- request/drain/finalize;
- reserve/commit/abort;
- linear obligation ownership;
- region quiescence;
- bounded responsiveness assumptions.

Prove compositional rules such as:

```text
operation cancel-correct
+ finalizer cancel-correct
+ region obligation closure
⇒ no visible half-effect after region close
```

This is a genuinely underdeveloped area compared with safety of message protocols.

### A3. Effect-pack refinement

For each pack, distinguish:

- ideal semantics;
- operational Lab semantics;
- host/production semantics;
- observation relation.

Use refinement transformers inspired by verified distributed-system frameworks: application proofs should survive when a verified pack implementation replaces an ideal pack.

## Track B — Partial-order engines

### B1. Observer-sensitive parsimonious DPOR

Extend dependence with active view/property observers. Internal effects may commute even when their concrete footprints overlap if the abstraction proves commutativity.

Research questions:

- can abstraction-proved commutation safely reduce implementation exploration?
- how are liveness/fairness dependencies retained?
- can independence witnesses be certified compactly?

### B2. Complete finite causal prefixes

Translate bounded CIR systems into occurrence structures and compute adequate-order cutoffs. Explore symbolic high-level events to avoid grounding every data value.

Benchmarks against:

- source DPOR;
- parsimonious optimal DPOR;
- explicit BFS;
- TLC/Stateright where comparable.

### B3. Causal Cubical Reduction

Construct a cubical complex:

- vertices: configurations;
- edges: events;
- \(n\)-cubes: \(n\) mutually commuting events.

Potential uses:

- canonical representatives of schedule families;
- homotopy-class exploration;
- detection of “holes” corresponding to forbidden synchronization patterns;
- compact visualization;
- search guidance.

Required discipline:

- topology is heuristic unless connected to a proven quotient;
- Betti numbers are not correctness evidence;
- retain only if practical reduction or diagnosis improves.

### B4. Event-language minimization

Recent higher-dimensional Myhill–Nerode/Kleene results suggest residual languages over interval pomsets. Explore whether repeated protocol subbehaviors can be minimized directly in a non-interleaving automaton.

This could create reusable causal summaries for components.

## Track C — Compositionality and abstraction

### C1. Multi-grained verification

Automatically compute impact cones from:

- changed code;
- effect footprints;
- view dependencies;
- learned invariants.

Keep affected modules concrete/fine; replace unaffected modules with certified summaries. Validate composition using assume-guarantee obligations.

### C2. Sheaf gluing

Model:

- local component behavior as sections;
- interfaces as restrictions;
- global execution as a compatible gluing.

Research hypotheses:

1. incompatible local abstractions produce a computable obstruction;
2. the obstruction identifies missing interface state/assumptions;
3. local proof certificates can be assembled without constructing the whole state graph.

Comparison baseline: interface automata, assume-guarantee learning, modular unfoldings.

### C3. Abstract-domain synthesis

Candidate methods:

- predicate abstraction;
- CEGAR;
- anti-unification of counterexample states;
- IC3-learned clauses;
- symmetry orbits;
- e-graph equality saturation for expression candidates;
- SyGuS grammar for abstraction functions;
- abstract interpretation with Galois insertions.

All synthesized maps are verified independently.

### C4. Coalgebraic behavioral interfaces

Coalgebra offers a uniform language for state-based systems and bisimulation. Explore a restricted coalgebraic interface for domain packs and views so that behavioral equivalences and compositional operators are not reinvented per engine.

Kill if it adds abstraction vocabulary without simplifying implementations or proofs.

## Track D — Inductive and parameterized proof

### D1. Symmetry-to-quantification

Integrate orbit analysis and IC3PO-style clause generalization:

- infer quantified invariants from finite instances;
- discover candidate cutoffs;
- validate with first-order induction;
- explain which symmetries justify generalization.

### D2. WSTS lane

Detect monotone fragments over:

- counters;
- multisets;
- lossy channels;
- process populations;
- broadcast protocols.

Generate coverability proofs using ideals/backward reachability. Report precisely that only upward-closed safety was proved.

### D3. CHC/PDR portfolio

Lower model/refinement/liveness obligations to CHCs. Run multiple engines and preserve learned invariants. Develop model-specific global guidance using causal/action partitions.

### D4. Proof repair

When an inductive invariant fails:

- classify initiation/consecution/property failure;
- produce minimal counterexample-to-induction;
- mine relevant predicates from CIR footprints and views;
- propose repairs;
- mechanically recheck.

## Track E — Liveness

### E1. Ranking synthesis

Represent eventualities as obligations. Synthesize lexicographic, multiset, ordinal, or relational rankings under fairness.

Use the runtime obligation graph as a candidate ranking vocabulary.

### E2. Fairness diagnostics

Given a liveness counterexample, compute:

- unfair actions;
- permanently enabled actions;
- failure assumptions needed to sustain the cycle;
- weakest fairness strengthening that excludes it;
- whether that strengthening matches production reality.

The tool should not simply print an SCC.

### E3. Cancellation progress

Properties:

- every cancellation request reaches region quiescence under declared responsiveness;
- no obligation is transferred infinitely often without progress;
- race losers drain;
- finalizers terminate or expose a bounded-failure certificate.

### E4. Ordinal progress

Some nested retries/recovery protocols need rankings beyond natural numbers. Explore ordinal templates and well-founded transition invariants, but keep generated proofs explicit and checkable.

## Track F — Production conformance

### F1. Partial-order trace completion

Input:

- event labels;
- causal IDs/vector clocks;
- timeboxes;
- missing-event model;
- abstract semantics.

Output:

- satisfying completion/linearization;
- minimal inconsistency core;
- or explicit insufficiency.

Use SAT/SMT/CP backends, but replay the resulting abstract execution.

### F2. Observability analysis

Compute which properties are monitorable from an instrumentation schema. Suggest the cheapest additional event needed to distinguish valid from invalid executions.

### F3. Trace-to-simulation reconstruction

Infer a constrained Lab scenario from a production trace:

- network envelopes;
- timer order;
- crash epochs;
- storage completions;
- cancellation points.

Then search neighboring schedules to find a smaller or more deterministic reproduction.

### F4. Hyperconformance

For security/randomized systems, ordinary trace inclusion is insufficient. Implement strong observational/progressive refinement for selected finite-state models, then evaluate scalability.

## Track G — Timed and probabilistic verification

### G1. Unified time constraints

Combine virtual time and production intervals through a common difference-constraint layer. Avoid conflating simulated exact times with observed uncertain times.

### G2. Timed partial orders

Use zones over event intervals and causality, reducing permutations that differ only by commuting timed events.

### G3. Rare-event verification

Combine:

- importance splitting;
- cross-entropy tuning;
- semantic distance-to-violation;
- exact replay;
- confidence sequences.

Statistical evidence remains statistical.

### G4. Probabilistic refinement

Define scheduler-sensitive probability-preserving refinement, using strong observational refinement where appropriate. This is a high-risk research area.

## Track H — Agentic verification

### H1. Semantic agent protocol

Expose tools for:

- inspect model state/action;
- request successor slice;
- inspect counterexample core;
- propose invariant;
- check induction;
- propose abstraction;
- run mutant;
- replay exact trace.

Agents never edit evidence artifacts directly.

### H2. Proof portfolio planning

Use agents to choose between explicit, PDR, refinement, and liveness tactics. The policy may be learned; each result remains mechanically gated.

### H3. Spec/code co-repair

Given a conformance failure, classify:

- implementation bug;
- model omission;
- abstraction mismatch;
- instrumentation gap;
- pack fidelity issue.

Require explicit human or policy approval before changing the specification to “fix” a failing implementation.

## Research scorecard

Every experiment records:

| Field | Meaning |
|---|---|
| hypothesis | falsifiable statement |
| baseline | strongest reasonable comparator |
| corpus | public and internal benchmarks |
| metric | soundness, reduction, time, memory, diagnosis |
| threshold | minimum practical win |
| failure mode | what would invalidate result |
| disposition | promote, continue research, kill |

Novelty without a scorecard does not enter the core.

---

## Revision-2 expansion

### Track G — Corpus-complete semantic design

#### G1. Behavioral parity methodology

Develop relational graph/trace comparison that can establish parity without requiring identical state encodings. Produce reusable correspondence proofs for common TLA+ idioms: stuttering closure, module instantiation, `UNCHANGED`, fairness, symmetry, views and auxiliary variables.

#### G2. Corpus-derived language minimization

Determine the smallest typed CML core that can express semantic equivalents of all 80 validated families. Prefer reusable desugarings over one-off operators. Record which source features require theorem-only or symbolic fragments.

#### G3. Mutation adequacy

Design mutants that expose false parity: missing frame conditions, fairness drift, wrong quorum arithmetic, invalid symmetry, altered deadlock policy and weakened properties. Measure whether source and port fail equivalently.

### Track H — Lean and proof-producing verification

#### H1. Reflective closure/refinement checkers

Implement executable Lean checkers with soundness theorems. Compare explicit proof terms, native reflection and external Rust checking plus imported witness.

#### H2. Verified encodings

Prove correspondence from CML finite/symbolic semantics to SAT/PB/SMT instances. This is more important than verifying an already detached solver certificate.

#### H3. Temporal proof library

Bridge Continuum temporal definitions to LeanLTL where compatible; formalize fair lassos, SCC exclusion, rankings and refinement composition.

#### H4. Proof receipts

Formalize enough receipt identity to prevent epoch/model/property substitution. Evaluate independent environment checking with Lean4Lean-class tools.

### Track I — Nominal and parameterized systems

#### I1. First-occurrence canonicalization

Prove alpha-renaming invariance for fresh IDs and quantify reduction on realistic request/transaction workloads.

#### I2. Orbit-finite automata

Evaluate nominal automata and finite-support representations for dynamic membership and allocation. Compare with finite symmetry and bounded-ID abstraction.

#### I3. Cutoff/counterexample lifting

Combine finite corpus models, symmetry-to-quantification, IC3PO-style inference and Lean proof to lift bounded evidence.

### Track J — Games and assumptions

#### J1. Maximal-permissive safety environments

Synthesize environment moves that must be prohibited for a safety property. Minimize and translate them into domain-pack policies.

#### J2. Fairness/progress games

Synthesize scheduler/network/recovery assumptions under which liveness is realizable. Generate monitors that distinguish system failure from violated environment assumptions.

#### J3. Byzantine liveness

Investigate strategy/assumption proof patterns for partial synchrony and adversarial scheduling, informed by current Byzantine-liveness verification work.

### Track K — Weak memory and hierarchical verification

Verify local concurrent components under execution-graph memory semantics and export atomic contracts to distributed models. Avoid the full Cartesian product. Compare against Loom, RustMC and dedicated litmus suites.

### Track L — Checked choreography and generation

Project global CML protocols to role-local automata, Rust endpoint traits and monitors. Prove or translation-validate projection, branch knowledge and communication compatibility.

### Track M — Automatic abstraction and proof-guided agents

Use abstract interpretation, CEGAR, invariant inference, interpretation reduction and corpus templates to propose abstractions. Require held-out schedules, mutants and independent refinement evidence before promotion.
