# Porting Waves

The order is chosen to retire semantic unknowns, not to maximize screenshot count.

## Wave 0 — semantic kernel

**19 families.** Puzzles, basic algorithms, proof tutorials, relations, and language examples.

Forces:

- finite sets and bounded integers;
- functions, records, tuples, sequences;
- quantifiers and comprehensions;
- primed variables and relational actions;
- stuttering closure;
- shortest counterexamples;
- recursive definitions;
- basic Lean theorem bridge.

Anchor ports: DieHard, N-Queens, Missionaries and Cannibals, Coffee Can, Tower of Hanoi, Majority, `x+x is even`, Transitive Closure, Level Checking.

## Wave 1 — shared-memory and procedural semantics

**10 families.** Mutual exclusion, barriers, readers/writers, lock-free and scheduling examples.

Forces:

- procedural-to-relational lowering;
- process-local variables and program counters;
- atomic labels;
- deadlock and starvation;
- weak/strong fairness;
- symmetry over process identities;
- auxiliary variables and refinement;
- local concurrency mapping to asupersync/Loom-like schedule choices.

Anchor ports: Dining Philosophers, Boulangerie, Dijkstra mutex, Peterson refinement, Barriers, Readers-Writers, Disruptor.

## Wave 2 — asynchronous topology and termination

**13 families.** Rings, graph algorithms, termination detection, allocation, and distributed mutual exclusion.

Forces:

- asynchronous message bags and channels;
- topology-parametric models;
- fairness of delivery and process actions;
- termination/liveness ranking;
- graph symmetry;
- parameterized verification candidates;
- causal traces and observer-sensitive independence.

Anchor ports: EWD687a, EWD840, EWD998, Chang-Roberts, Echo, Lamport mutex, Spanning Tree.

## Wave 3 — consensus, Byzantine behavior, and transactions

**19 families.** Paxos variants, atomic commit, failure detectors, broadcast, self-stabilization.

Forces:

- quorum systems and monotonic state;
- Byzantine/adversarial environment actions;
- refinement chains;
- prophecy/history variables;
- nontrivial liveness assumptions;
- parameterized inductive invariants;
- assumption synthesis and counterstrategies;
- reusable proof slicing.

Anchor ports: Paxos, MultiPaxos, Byzantine Paxos refinement, transaction commit, ACP/NBAC, Bosco, asynchronous Byzantine consensus.

## Wave 4 — storage, probability, protocols, and symbolic structure

**15 families.** Disk Paxos, snapshot KV, B-trees, buffered files, blockchain, probabilistic protocols, cyber-physical scheduling.

Forces:

- crash/durability semantics;
- persistent/volatile state separation;
- model-value overrides and symbolic lanes;
- probabilistic choices and quantitative claims;
- larger state spaces and compositional verification;
- concrete P5 implementations.

Anchor ports: KeyValueStore, Disk Paxos, B-tree, BufferedRandomAccessFile, Slush, NanoBlockchain.

## Wave 5 — stress and metacircular cases

**4 families.** TLC itself, TCP, German cache coherence, and level/semantic stress.

Forces:

- large operator/module closures;
- model-checker self-modeling;
- deep refinement hierarchies;
- symmetry and VIEW/ALIAS/CONSTRAINT behavior;
- industrial-size traces;
- hard performance gates.

These are not deferred because they are unimportant. They are final because they are the most valuable integrated tests of everything below them.
