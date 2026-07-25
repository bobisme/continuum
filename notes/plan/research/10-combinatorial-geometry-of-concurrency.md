# Research Note 10: Combinatorial Geometry of Concurrency

**Claim class:** experimental algorithm-design program  
**Relevant ideas:** trace monoids, event-structure domains, median graphs, CAT(0) cube complexes, antimatroids/greedoids, distributive lattices

## Thesis

The configuration space of concurrency is often not an arbitrary graph. In important fragments it has rigid combinatorial geometry. Exploiting that geometry may yield faster canonicalization, counterexample minimization, decomposition, exact coverage measures, and incremental verification.

This track asks a narrow question:

> Which real Continuum configuration spaces fall into recognizable geometric/combinatorial classes, and what algorithms become available when they do?

## 1. Configuration ideals and distributive lattices

For a fixed causal poset with no conflict, configurations are order ideals. They form a finite distributive lattice:

```text
meet = intersection
join = union
join-irreducibles = individual causal events
```

Consequences:

- canonical coordinates by event/hyperplane;
- cheap joins/meets for merging partial executions;
- Birkhoff-style representation;
- antichain/frontier representation;
- exact causal slicing.

With conflict, the family is no longer one distributive lattice globally, but may decompose into compatible domains.

### Engineering proposal

Represent a configuration by its maximal-event antichain plus a persistent ideal index when causality is stable. Measure whether this reduces storage versus full bitsets/sets on unfolding workloads.

## 2. Median graphs and partial cubes

Domains of event structures are closely related to median graphs/CAT(0) cube complexes in established concurrency theory. In a median graph, three configurations have a unique median lying on pairwise shortest paths. Hyperplanes/Θ-classes provide coordinates.

Potential Continuum uses:

- median of several failing/passing configurations as a central diagnostic state;
- hyperplane cuts as semantic event dimensions;
- convex/gated subgraphs for component decomposition;
- linear-time or near-linear distance/median algorithms;
- partial-cube embeddings for compact configuration identity;
- separator-guided work partitioning.

### Novel hypothesis: Hyperplane-Blame Decomposition

Map each semantic event/resource dimension to a configuration-space hyperplane. A property violation region and initial region may be separated by a small hyperplane set. Use that set as a candidate minimal semantic blame slice and as a learned abstraction vocabulary.

Compare against ordinary backward slicing, unsat cores, and feature importance from PDR clauses.

## 3. Trace monoids and Cartier–Foata theory

Under a fixed independence alphabet, executions form a trace monoid. Cartier–Foata normal form groups maximally parallel layers. The clique automaton and Möbius polynomial encode growth.

Potential uses:

- stable canonical traces;
- exact counting of equivalence classes by length;
- estimating explosion before exploration;
- schedule-space coverage that counts causal classes rather than interleavings;
- uniform/random trace sampling rather than biased word sampling;
- parallel-depth/width statistics.

Asupersync already uses Foata-style canonicalization concepts. Continuum can make the algebra explicit in CIR tooling.

### Novel proposal: Trace-Growth Forecasting

For a locally stable independence graph, estimate the growth series of trace classes and compare it with observed branching. Use mismatch to detect state-dependent conflicts or missing semantic footprints. Feed the estimate into engine/partition selection.

This is heuristic unless the independence relation is globally fixed.

## 4. Antimatroids and greedoids

Feasible event prefixes in some monotone systems may form accessible set systems, antimatroids, or greedoids. Antimatroids admit greedy constructions and convex-geometry duals.

Potential uses:

- greedy shortest/most-readable legal linearization;
- incremental feasible-prefix maintenance;
- canonical extreme-event sets;
- test-generation order with guaranteed accessibility.

General event structures with conflict will not be antimatroids. The value is in recognizing restricted substructures—cleanup phases, monotone recovery, or one fixed conflict branch.

### Novel proposal: Drain Antimatroids

Hypothesize that valid cancellation-drain prefixes for a well-structured region, after the cancellation choice is fixed, form an antimatroid: prefixes are accessible and unions of compatible drain prefixes remain feasible. If true, greedy algorithms could find canonical cleanup orders and reason compositionally about quiescence.

Falsify with generated cancellation protocols; do not force the property.

## 5. Heaps of pieces

Viennot-style heaps give a geometric representation of trace-monoid elements: dependent pieces stack, independent pieces commute. A CIR trace can be visualized as a heap whose contact graph is dependence.

Potential value:

- compact human explanation;
- canonical layering;
- incremental hash;
- schedule mutation by moving exposed pieces;
- causal minimization preserving feasibility.

This may be a better user-facing representation than raw DAGs for medium traces.

## 6. Möbius inversion and causal attribution

Incidence algebras over posets support Möbius inversion. Given cumulative measurements over configurations, inversion can recover marginal contributions.

Possible applications:

- attribute aggregate latency/resource deltas to causal events;
- separate repeated/overlapping observer effects;
- incremental state reconstruction;
- exact inclusion-exclusion over causal cones.

This is only valid where the measured quantity obeys the required additive relation. It should not become generic “causal inference.”

## 7. Non-positive curvature as a tractability signal

CAT(0)/median geometry informally means independent choices fit together without pathological positive curvature. A tractability detector might measure local cube completion:

```text
if every observed commuting square/cube closes consistently,
the configuration region may admit median/partial-cube methods.
```

Failure to complete a cube is diagnostically useful: it reveals hidden conflict, state-dependent independence, observer sensitivity, or missing causality.

## 8. Experiments

### Recognition

On generated and real CIR prefixes, test:

- distributive-lattice laws;
- median uniqueness;
- partial-cube embedding;
- antimatroid accessibility/union closure;
- fixed versus state-dependent trace independence.

### Algorithms

Compare:

- bitset configuration representation versus antichain/hyperplane coordinates;
- BFS shortest counterexample versus median/convex methods;
- standard causal slicing versus hyperplane blame;
- random interleaving sampling versus trace-class sampling;
- ordinary replay presentation versus heap/Foata presentation.

### Negative corpus

- disabling/conflict that breaks lattice joins;
- dynamic resource aliases;
- fault events whose commutation depends on timing;
- observer-sensitive noncommutation;
- non-monotone recovery.

## 9. Governance

These structures are recognized properties of a semantic fragment, never assumed globally. Recognition either has a proof/certificate or remains an optimization guarded by runtime validation and fallback.

The goal is not to rename model checking with geometric language. The goal is to discover exploitable structure that ordinary graph treatment leaves on the floor.
