# Research Note 06: Timed, Probabilistic, and Hyperproperty Extensions

**Claim class:** future extension  
**Relevant sources:** [S50]–[S56], [S76]

## Principle

Qualitative nondeterminism, real time, probability, and adversarial choice are different mathematical structures. Continuum must not combine them in an untyped `choose` operation.

## Typed choice algebra

Proposed effect distinctions:

```text
demonic choose       all outcomes must satisfy
angelic choose        witness exists
probabilistic sample  measure/distribution specified
scheduler choose      governed by scheduler/adversary model
timed delay           constrained by clocks/invariants
epistemic unknown     evidence incomplete, not a runtime choice
```

Mixed systems may form stochastic games or Markov decision processes. Result semantics identify which choices are optimized/adversarial.

## Time

Timed extensions can compile to:

- timed automata/zones;
- difference-bound matrices;
- parametric timed automata;
- bounded SMT constraints;
- virtual-time concrete exploration.

Clock types need explicit semantics:

- monotonic;
- wall;
- logical/vector/hybrid;
- uncertainty interval;
- drift-bounded;
- resettable.

A lease proof must state its clock and synchrony assumptions.

## Probability

Probabilistic packs can model:

- randomized algorithms;
- failure-rate models;
- network loss distributions;
- workload distributions.

Claims include:

- reachability probability;
- expected reward/cost;
- almost-sure safety/progress;
- percentile/quantile under assumptions;
- adversarial scheduler bounds.

Statistical model checking is `sampled statistical evidence`, not exact probabilistic model checking.

Storm/PRISM interoperability is preferable to implementing every numerical backend initially. Certificates for numerical results are a research problem; interval bounds and independently checked linear programs are possible paths.

## Hyperproperties

Many security and robustness properties relate multiple executions:

- noninterference;
- observational determinism;
- opacity;
- differential privacy;
- strong linearizability/refinement;
- scheduler robustness.

Self-composition or HyperLTL-style automata can reduce some finite hyperproperties to ordinary model checking. Observer-indexed CIR and strong refinement contracts are designed to support this.

## Novel proposal: Adversary-Parametric Refinement

Model scheduler, fault injector, and environment as typed adversaries. A refinement claim quantifies over an adversary class:

\[
\forall A\in\mathcal A_C.\;\exists B\in\mathcal A_A.\;
Obs(Exec_C,A)\preceq Obs(Exec_A,B).
\]

Stronger versions require strategy-preserving mappings. This makes hidden scheduler assumptions explicit and connects strong observational refinement to probabilistic games.

## Timed multiparty protocols

Timed multiparty session types can statically validate local communication against global timing protocols. Continuum could import a session/choreography contract as:

- a domain-pack interface;
- an action-enabling constraint;
- a source of monitor events;
- a refinement target.

This offers cheap local guarantees before global state-space exploration.

## Boundaries

Initial Continuum MUST NOT advertise timed/probabilistic proof support. It should only preserve typed events and assumptions needed for future adapters. The G6 research gate requires comparison with UPPAAL, IMITATOR, Storm, and PRISM.

## Experiments

- lease protocol under clock drift and partition;
- randomized leader election;
- retry/backoff with probabilistic loss and adversarial scheduling;
- noninterference of tenant requests;
- strong refinement of a concurrent queue;
- timed session protocol implementation.

## Kill criteria

- One mixed semantics whose results users cannot interpret.
- Numerical answers without error bounds.
- Probability inferred from arbitrary DST schedule sampling.
- Hyperproperty support that destroys replay/explanation quality without compelling use cases.
