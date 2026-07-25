# Semantic Feature Census Forced by the Corpus

The upstream metadata explicitly identifies PlusCal, proofs, action composition, symbolic/simulation/generation modes, deliberate failures, and TLC configuration features including `SYMMETRY`, `VIEW`, `ALIAS`, `CONSTRAINT`, and `DEADLOCK`. The 80-family inventory additionally forces the following semantic surface.

## Values and operators

- booleans, integers, naturals, bounded ranges, strings, uninterpreted model values;
- finite and symbolic sets, powersets, unions/intersections, comprehensions and filters;
- total functions as values, function constructors, updates, domains, records, tuples and sequences;
- relations, transitive closure, graphs and paths;
- higher-order operators, lambdas, recursive operators, `LET/IN`, conditionals and `CHOOSE`-like witness selection;
- bounded and symbolic quantification;
- cardinality, arithmetic, min/max and sequence operators;
- module extension, instantiation, parameter substitution and local definitions.

## Action semantics

- current/next-state variables;
- priming under nested expressions;
- `UNCHANGED`, except/update forms, action composition and enabledness;
- nondeterministic choice and existentially bound action parameters;
- stuttering closure `[Next]_vars`;
- deadlock policy and terminal-state interpretation;
- procedural programs lowered to labeled atomic actions and program counters.

## Temporal semantics

- invariants and action properties;
- eventually/always, leads-to, response and recurrence;
- weak and strong fairness over subactions;
- liveness counterexamples as fair lassos;
- termination arguments and ranking functions;
- explicit distinction between finite traces, infinite behaviors and machine-closed specifications.

## Model configuration

- constants and bounded model assignments;
- definition overrides;
- state and action constraints;
- symmetry sets and canonicalization;
- views and aliases for state projection/counterexample rendering;
- expected deadlock behavior;
- exhaustive, simulation, trace-generation and symbolic modes;
- expected safety, liveness, assumption and deadlock failures.

## Refinement and proof

- stuttering simulation;
- auxiliary, history and prophecy variables;
- refinement across multiple abstraction grains;
- safety theorems, inductive invariants and mathematical lemmas;
- compositional proofs and action-local proof obligations;
- parameterized claims over process/node counts.

## Domains

- shared memory, locks, atomics and lock-free algorithms;
- asynchronous channels and message bags;
- failure detectors, partitions, omission and Byzantine faults;
- quorums, consensus, broadcast and atomic commitment;
- volatile/durable storage, snapshots, files and B-trees;
- network protocols and cache coherence;
- random/probabilistic choice;
- cyclic, complete and arbitrary graph topologies;
- cyber-physical scheduling and time.

## Resulting architectural consequence

No single backend can honestly implement this surface efficiently. The model language must elaborate to a typed semantic core whose fragments are declared explicitly:

```text
Finite       exact enumeration and certificates
Symbolic     SMT/CHC/PDR
Temporal     Büchi/Streett/parity and ranking
Probabilistic MDP/game analysis
Theorem      Lean statements and proofs
Runtime      asupersync concrete semantics
```

The fragment assignment is part of the checked artifact. Unsupported cross-fragment combinations are rejected rather than approximated silently.
