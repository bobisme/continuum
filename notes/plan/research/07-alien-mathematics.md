# Research Note 07: Alien-Mathematics Portfolio

**Status:** exploratory; none of these ideas are product claims  
**Rule:** mathematics survives only by producing a sound algorithm, smaller evidence, better diagnostics, or measurable verification power.

## 1. Directed topology and concurrency geometry

### Idea

Executions are directed paths through a state/configuration space. Independent operations create higher-dimensional cells, and schedules related by directed homotopy represent the same causal behavior.

### Potential value

- stronger partial-order quotienting;
- liveness classes insensitive to local commutations;
- geometric counterexample normalization;
- concurrency-dimension metrics.

### Required theorem

A quotient/reduction must preserve the selected safety/temporal property and produce a checkable witness.

### Kill condition

No consistent win over optimal DPOR/unfoldings on systems with high causal width.

## 2. Sheaves and cohomological obstruction

### Idea

Local model/refinement witnesses form sections over components or causal regions. Compatibility on overlaps is restriction equality. A global proof is a global section; cohomology can expose obstruction.

### Potential value

- modular conformance from partial telemetry;
- detecting inconsistent local assumptions;
- cross-shard protocol diagnosis;
- local-to-global proof composition.

### Required theorem

For the selected class, global sections correspond to valid global refinement witnesses. Obstruction must be sound, not merely correlated.

### Kill condition

A direct SAT/CSP formulation is simpler, faster, and equally diagnostic.

## 3. Homological schedule coverage

### Idea

Construct a filtration of observed commuting cells; persistent homology tracks stable holes/components as exploration grows.

### Potential value

Prioritize schedules that reveal new concurrency topology and detect missing commutation faces.

### Caveat

Coverage is not correctness. Homology can be an attractive dashboard with no bug-finding value.

### Kill condition

No held-out improvement over pair/event/state coverage.

## 4. Category-theoretic semantics

### Idea

Domain packs are effectful transition systems; composition is monoidal; views are morphisms; refinement certificates compose. String diagrams may expose dataflow and independence.

### Potential value

- principled pack composition;
- reusable proof combinators;
- compositional probabilistic/timed semantics;
- semantic optimizer correctness.

### Discipline

The implementation API uses ordinary Rust types. Category theory belongs in the specification/proofs unless it materially simplifies code.

## 5. Coalgebra and coinduction

### Idea

Potentially infinite behaviors are coalgebras. Bisimulation and coinduction support reactive equivalence, minimization, and liveness/stream reasoning.

### Potential value

- canonical behavior quotients;
- incremental conformance;
- coinductive protocol contracts;
- infinite-state symbolic representations.

### Candidate experiment

Use partition refinement/bisimulation minimization on observer-projected transition systems before liveness checking.

## 6. Domain theory and resumable partial computation

### Idea

Executions under budgets produce increasing approximations. A verification campaign is a monotone computation in an information order, capable of checkpointing and resuming without changing meaning.

### Potential value

- mathematically clean `ResourceExhausted` evidence;
- distributed/resumable search;
- anytime assurance;
- merging partial exploration artifacts.

### Concrete proposal

Define an evidence domain where partial results join only when semantic envelopes match. A completed proof is a maximal element; sampled and exhaustive artifacts remain distinct branches rather than one scalar confidence.

## 7. Linear logic and obligation semantics

### Idea

Runtime obligations are linear resources: they cannot be duplicated or silently discarded. Reserve/commit/abort and cancellation drain have natural session/linear interpretations.

### Potential value

- static/dynamic obligation conservation;
- compositional cancellation proof;
- protocol-capability synthesis;
- failure explanations as unconsumed proof resources.

### Candidate implementation

A lightweight affine/linear ghost calculus embedded in view contracts, with runtime events as evidence of resource transitions.

## 8. Ordinals and termination

### Idea

Distributed recovery and cancellation may need lexicographic/multiset/ordinal rankings beyond a single natural number.

### Potential value

- proof of nested cleanup;
- phase-changing retry protocols;
- parameterized progress.

### Discipline

Use the weakest ranking domain that works. Exotic ordinals are not a badge.

## 9. Game semantics

### Idea

The system, scheduler, network, faults, and environment are players with different powers. Verification asks for winning strategies under adversary classes.

### Potential value

- fault-tolerant synthesis;
- robust refinement;
- explicit scheduler assumptions;
- controller generation.

### Candidate lane

Compile a finite model to a parity/safety game and synthesize a recovery or scheduling policy, then refine it into an asupersync supervisor.

## 10. Information geometry / active experiment design

### Idea

Select faults, schedules, or production probes that maximally distinguish competing semantic hypotheses.

### Potential value

- pack qualification;
- instrumentation synthesis;
- model debugging;
- high-value test generation.

### Soundness boundary

Experiment selection is heuristic. Only resulting checked evidence affects assurance.

## 11. Supervisory control and runtime enforcement

### Idea

Ramadge–Wonham-style supervisory control synthesizes the maximally permissive controller that disables controllable events to maintain safety.

### Potential value

Continuum could generate a guard/supervisor around an implementation when full correctness is not yet proven, while preserving as much behavior as possible.

### Hard question

Which effects are genuinely controllable without violating liveness or changing the contract?

## 12. Choreographies and global types

### Idea

A global protocol can project to node-local implementations. Projection correctness removes classes of communication mismatch before model checking.

### Potential value

- generated typed channels;
- compatibility with zoomable views;
- reduction of exploration space;
- explicit timed/session obligations.

## 13. Proof repair as constrained synthesis

### Idea

A failing refinement or invariant creates a synthesis problem over restricted patches: guards, retry placement, commit ordering, obligation discharge, or invariant strengthening.

### Safety rule

Generated patches are candidates only. They must pass replay, neighboring exploration, regression properties, and proof/certificate gates.

## Research governance

Every lane receives:

- a formal statement;
- baseline algorithms;
- public benchmark corpus;
- predeclared success metric;
- implementation budget;
- falsification/kill criterion;
- result classification (`OBSERVED`, `HYPOTHESIS`, `PROVEN`);
- no effect on stable semantics until an ADR promotes it.

Alien mathematics is valuable precisely when it stops being decorative.
