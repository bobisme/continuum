# RFC 0004: Exploration, DPOR, and Unfoldings

**Status:** Proposed  
**Target gates:** G1/G2/G6 (Revision 2 scheme, docs/26 — not citable without translation to the docs/52 Revision 3 gates per plan §22)

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

## Corrections recorded by this RFC

### Where the baseline reducer met cyclic finite models (bn-voq4)

1. **The first delivered baseline is the stateful form: persistent sets from the conservative conflict relation, sleep sets, observer visibility, and a cycle proviso.** "Source-DPOR" above explores maximal executions and replays them from the root. The finite models of `continuum-model-core` are routinely cyclic (a register that crashes and recovers), so a maximal execution is often infinite. `continuum-engine-dpor` (`stateful-persistent-sleep-dpor/v0`) is therefore stateful:
    1. **Dependence.** A label is one action outcome. Its read and write footprints are derived from the model's guards and updates, never declared. Two labels are dependent when either writes a variable the other reads or writes. The oracle never answers `Unknown`, and every consumer treats `Unknown` as dependent.
    2. **Source sets.** At each new state the reducer explores the enabled part of the smallest stubborn closure over the invisible enabled seeds: an enabled member brings every label it depends on, and a disabled member brings every label that writes a variable it reads. That set is persistent.
    3. **Visibility.** The variables the obligations read are visible (INV-013), and so are those of every definedness predicate the check consults (RFC 0003: each invariant's chain `I#defined…`, and every action's chain, as `continuum_model_core::definedness` classifies them). A reduced expansion contains no enabled label that writes one, so no reduction can postpone the step that reaches an undefined read; an undefined read is the typed outcome RFC 0003 names, in the reference engine's precedence.
    4. **Sleep sets.** The search graph's nodes are visits: a state with the sleep set it was entered with. A visit whose sleep set is a subset of the new one is reused. A label joins the sleep set of its later siblings only after the strongly connected component it leads to is complete. The C005 corpus found that a label donated along an edge back into an open component makes a sleep justification circular and loses states. This donation rule is the engine's own addition to the persistent-set and sleep-set literature, argued in `crates/continuum-engine-dpor/src/checker.rs` and not machine-checked.
    5. **Proviso.** An edge back onto the search stack expands its visit in full.
    6. **Witness.** Every declined label is justified by persistence or by sleep, and an independent checker re-derives the footprints, re-fires every label, and checks each justification, the donation rule, and the proviso per witness.

    This preserves every reachable deadlock and every reachable valuation of the visible variables, and the existence of a reachable evaluation fault (the search halts at the first fault it meets, as the unreduced reference engine does, so which fault is reported is not fixed): Tier A of RFC 0014 and nothing more. Vector-clock happens-before, stateless replay, and wakeup trees (roadmap step 2) are not implemented. The stateless form remains the roadmap's; this correction records which form the claim C005 evidence covers.

    Evidence: `crates/continuum-engine-dpor/tests/c005_differential.rs` and `crates/continuum-engine-dpor/src/mutation.rs`, with their ledgers under `crates/continuum-engine-dpor/tests/golden/`.

## Rejected alternatives

- A single randomized scheduler marketed as a model checker.
- Blindly importing Loom/Shuttle semantics.
- Treating read/write footprints as a proof of commutativity.
- One global concurrent hash set as the only exploration architecture.
