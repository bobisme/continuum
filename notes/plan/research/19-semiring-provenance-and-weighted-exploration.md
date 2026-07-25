# Research 19: Semiring Provenance and Weighted Exploration

## Observation

Many graph/model analyses differ only in how alternative paths and sequential steps combine.

| Analysis | Alternative combine | Sequential combine |
|---|---|---|
| reachability | OR | AND |
| path count | + | × |
| shortest cost | min | + |
| fault provenance | + | × symbolic monomials |
| reliability | probability sum* | multiplication* |

The starred probabilistic case requires care around dependence and adversarial nondeterminism.

## Proposal

Use typed weighted-transition APIs where algebraic laws are explicit and checked. A transition carries labels/weights; analysis evaluates path/configuration expressions in a selected algebra.

## Provenance polynomial

Assign variables to semantic causes:

```text
c_cancel_17
f_disk_loss
m_deliver_42
```

A property violation obtains a polynomial/expression describing combinations sufficient for reachability. Simplification can reveal:

- minimal fault sets;
- common causal factors;
- alternative independent witnesses;
- sensitivity to assumptions.

This is richer than one shortest counterexample.

## Causal-class counting

Trace monoids and Foata normal forms suggest counting equivalence classes rather than linear schedules. Weighted automata over orbit-finite sets suggest a route for names and counts together.

## Quantitative optimization

Tropical or lexicographic weights can minimize:

- semantic events;
- context switches;
- crashed nodes;
- distinct faults;
- elapsed virtual time;
- abstraction-visible steps.

A product algebra yields a canonical “smallest useful counterexample” objective.

## Probability warning

Probabilistic choice, scheduler nondeterminism and adversarial faults require MDP/stochastic-game semantics. A naive probability semiring can double-count dependent paths or choose an unjustified scheduler. Continuum only uses weighted algebra where the semantic conditions are explicit.

## Experiment

Implement one generic acyclic dynamic-programming kernel for:

1. reachability;
2. shortest causal witness;
3. number of equivalence-class representatives;
4. minimal fault-set provenance.

Compare code complexity and performance with specialized implementations. Promote only if the abstraction pays rent.
