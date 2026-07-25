# Research Note 01: True Concurrency as the Native Verification Object

**Claim class:** design hypothesis  
**Relevant sources:** [S15]–[S24], [S17], [S21]

## Thesis

Most model checkers linearize concurrent activity immediately and spend the rest of their lives recovering the independence they discarded. Continuum should invert that architecture: a finite partial-order execution is primary, while an interleaving is one linear extension used for execution or presentation.

This is not merely representational taste. It affects:

- reduction power;
- production-trace ingestion;
- compositionality;
- counterexample stability;
- fairness reasoning;
- certificate size;
- semantic debugging.

## Mathematical candidates

### Mazurkiewicz traces

Given an alphabet \(\Sigma\) and independence relation \(I\), quotient words by adjacent swaps of independent letters. This is the foundation of DPOR and a practical v0 model. Its weakness is that independence can depend on state, observers, faults, and effect phases.

### Prime/stable event structures

Events carry causal predecessors and conflict. Configurations are conflict-free downward-closed sets. They naturally represent branching and concurrency, but rich data may make conflict and enabling intensional.

### Occurrence nets and Petri-net unfoldings

Unfoldings separate causal histories and can produce finite complete prefixes under cutoffs. They offer strong reduction for asynchronous systems and a possible proof-carrying closure artifact.

### Interval pomsets

Operations in real systems extend over time. Interval pomsets can retain overlap without forcing an arbitrary linearization and align with production telemetry. Higher-dimensional automata research increasingly uses interval pomsets as accepted languages.

### Higher-dimensional automata

Independent transitions span cubes: two commuting actions form a square, three form a cube, and so on. Directed paths correspond to executions; directed homotopies identify schedules that differ only by deformation through independent operations. Recent Kleene and Myhill–Nerode results suggest automata/language minimization principles exist beyond interleavings.

## Proposed semantic tower

```text
CIR event structure
   ├── linear extension → executable replay
   ├── configuration graph → explicit-state model checking
   ├── occurrence prefix → unfolding engine
   ├── interval pomset → production conformance
   └── cubical complex → experimental high-dimensional reduction
```

Each projection has a checkable preservation obligation.

## Novel proposal: Causal Cubical Reduction

For a property observer \(O\), define an observer-relative independence relation \(I_O\). Build cubical cells for jointly enabled sets of pairwise independent events, but retain faces that interact with:

- property observations;
- fairness enabling;
- obligation flow;
- cancellation phases;
- time constraints;
- durability barriers.

Compute a directed quotient that preserves \(O\)-relevant reachability and selected cycle classes. The intended win is to collapse entire families of schedules with high concurrency dimension rather than discover pairwise swaps incrementally.

### Why this might work

Distributed runtimes often exhibit bursts of independent activity across nodes, shards, or keys. Pairwise DPOR records many races/backtracking choices even when a larger commuting family exists. A cubical representation makes the \(k\)-way independence explicit.

### Why it might fail

- Building cells may cost more than exploring schedules.
- State-dependent independence can fracture cubes.
- Directed homotopy preservation may be too weak for liveness.
- Rich data and faults can make the complex enormous.
- Certificate checking may become harder than the saved exploration.

### Falsification experiment

Corpus:

- independent per-shard requests;
- actor systems with mailbox-local work;
- replicated protocols with message fan-out;
- cancellation trees;
- key-value transactions with disjoint footprints;
- adversarial cases with mostly dependent events.

Compare against source-DPOR, optimal DPOR, parsimonious ODPOR, and unfolding prefixes. Report:

- maximal executions/relevant classes explored;
- event/cell count;
- wall time;
- peak memory;
- counterexample latency;
- certificate bytes;
- false independence defects found by mutation.

Kill the lane unless it yields at least an order-of-magnitude reduction on a non-artificial subset without a serious regression on dependent workloads.

## Novel proposal: Observer-Sensitive Independence

Traditional dependence asks whether transitions commute in full concrete state. Continuum properties often observe only a projection. Define independence relative to:

\[
\mathsf{Obs}_P = \text{property state} \cup \text{fairness enabling}
\cup \text{assumption monitors}.
\]

Events may be equivalent for one property and dependent for another. This can produce property-directed reduction while remaining sound if the observation abstraction is complete.

The hard part is generating and checking the property footprint. A certificate should include a proof that swapping the events preserves the observer projection and future enabled observer-relevant behavior.

## Counterexample geometry

A causal counterexample should be a minimal bad configuration, not merely a short word. Minimization objectives can include:

- event count;
- number of owners/nodes;
- number of faults;
- causal width;
- obligation-flow complexity;
- observer-visible events;
- semantic edit distance to a passing execution.

Computing a minimal causal core can use delta debugging over downward-closed configurations plus SAT/SMT constraints. Geodesic linearization then produces a readable replay with few owner switches.

## Topological coverage

Betti numbers or persistent homology over a commuting-diamond/cubical complex may indicate unexplored concurrency structure. This is a heuristic, not assurance. It may help schedule selection by prioritizing traces that add new cycles or fill unobserved cells.

The benchmark must compare it against simpler novelty metrics: event-pair coverage, happens-before edge coverage, state novelty, and random/PCT scheduling. If topology does not improve bug yield per CPU, delete it.

## Deliverables

1. Formal CIR-to-event-structure semantics.
2. Reference enumerator for tiny event structures.
3. Source-DPOR over CIR.
4. Unfolding prototype.
5. Cubical prototype behind an experimental feature.
6. Cross-engine counterexample equivalence tests.
7. Certificate format for partial-order closure.
