# RFC 0004: Exploration, DPOR, and Unfoldings

**Status:** Proposed  
**Target gates:** G1/G2/G6

## Summary

Continuum SHALL implement exploration as a family of algorithms over one semantic choice interface. The baseline is deterministic replay plus conservative source-DPOR. The frontier path adds optimal/parsimonious DPOR, event-structure unfoldings, symbolic complete prefixes, and observer-sensitive reduction.

No reduction may change the claim type unless its independence and cutoff assumptions are explicit.

## Choice interface

At a configuration \(C\), an engine asks:

```rust
pub trait TransitionOracle {
    fn enabled(&self, c: &Configuration) -> EnabledSet;
    fn execute(&self, c: &Configuration, choice: Choice)
        -> Result<Transition, SemanticError>;
    fn dependence(&self, a: &EnabledEvent, b: &EnabledEvent)
        -> DependenceEvidence;
}
```

`Choice` includes task scheduling, model action parameters, message delivery, timer firing, fault injection, symbolic branch selection, and domain-pack outcomes.

The oracle may be:

- standalone model evaluator;
- asupersync Lab execution;
- replay engine;
- product of a concrete and abstract execution;
- symbolic transition relation.

## Baseline exploration

### Deterministic campaigns

Seeded schedules/faults provide fast bug finding. Results are `sampled`, never exhaustive.

### Source-DPOR

Version 0 uses vector clocks and a conservative conflict relation. It explores one or more representatives per Mazurkiewicz class, depending on sleep-set precision. Correctness is checked against exhaustive tiny instances.

### Stateless versus stateful

Stateless exploration stores schedules/backtracking information and re-executes from the root or snapshot. Stateful exploration stores canonical configurations. Continuum supports both because distributed faults and data nondeterminism may favor different tradeoffs.

## Snapshot strategy

Snapshots are semantic checkpoints, not raw process images. A snapshot binds:

- CIR prefix hash;
- runtime/model state snapshot;
- domain-pack versions;
- choice cursor;
- object identity map;
- pending obligations;
- virtual time;
- semantic version.

Restoration MUST be observationally equivalent to replaying the prefix. Snapshot correctness receives a differential campaign and certificate option.

## Dependence evidence

```rust
enum DependenceEvidence {
    DefinitelyIndependent(ProofRef),
    DefinitelyDependent(Reason),
    Unknown,
}
```

`Unknown` is dependent. Fast lanes MAY use heuristic independence but must downgrade the claim to unsound/experimental and label it accordingly; CI proof lanes cannot.

Independence is observer-relative. Two events may commute for a safety invariant but not for a latency or fairness property. The property compiler emits an observation footprint that participates in dependence.

## Optimal and parsimonious DPOR

The implementation roadmap is:

1. source-DPOR;
2. wakeup-tree optimal DPOR;
3. parsimonious optimal DPOR for polynomial-space exploration where applicable;
4. await/pure-loop-aware reduction;
5. observer-sensitive dependence.

Each step must reproduce exhaustive results on a generated corpus and include adversarial programs constructed to break naïve conflict relations.

## Unfolding lane

The unfolding engine builds an occurrence-net/event-structure prefix:

- conditions represent resource/version facts;
- events represent CIR transitions;
- causality comes from consumed/produced conditions;
- conflict comes from competing causes or semantic exclusions;
- cutoff events identify equivalent future behavior.

A finite complete prefix can compactly represent all reachable configurations for suitable finite systems. High-level/symbolic unfoldings are a research lane for rich values.

## Causal Cubical Reduction hypothesis

Independent \(k\)-event families define \(k\)-dimensional cubes. The hypothesis is that a cubical quotient can preserve property-relevant directed homotopy classes more compactly than pairwise trace equivalence.

The experiment SHALL compare:

- explored representatives;
- memory;
- counterexample preservation;
- certificate size;
- preprocessing overhead

against optimal DPOR and unfoldings. It is killed if it does not produce repeatable wins on systems with genuine high-dimensional concurrency.

## Fairness and liveness interaction

Safety DPOR is not automatically liveness-preserving. Liveness exploration must use cycle provisos, fairness-aware stubborn/ample sets, or a certificate establishing preservation. The engine refuses to reuse a safety reduction for liveness without such evidence.

## Distribution

Distributed exploration partitions by canonical semantic prefix/configuration hash. Work donation includes backtracking obligations, not just frontier states. Deterministic global replay is preserved by stable partition IDs and content-addressed work records.

Distributed checking is postponed until single-node exactness, replay, and certificates are stable.

## Outputs

Every campaign emits:

- explored equivalence classes;
- reduction algorithm/version;
- dependence oracle/version;
- complete/incomplete frontier status;
- bounds and assumptions;
- replayable counterexamples;
- optional closed-set or unfolding-prefix certificate;
- semantic coverage metrics.

## Rejected alternatives

- A single randomized scheduler marketed as a model checker.
- Blindly importing Loom/Shuttle semantics.
- Treating read/write footprints as a proof of commutativity.
- One global concurrent hash set as the only exploration architecture.
