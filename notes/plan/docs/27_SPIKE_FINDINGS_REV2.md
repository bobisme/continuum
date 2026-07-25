# Revision-2 Spike Findings

Executable artifacts live under `spikes/`. They use a deliberately small Python reference kernel because Rust and Lean toolchains were not present in the assembly environment.

## Spike 1 — exact finite reference exploration

The kernel implements:

- deterministic BFS;
- exact hashable state identity;
- labeled transitions;
- shortest witness reconstruction;
- finite closure certificate generation;
- independent certificate checking.

It is intentionally too simple to hide optimization bugs.

## Spike 2 — Die Hard corpus seed

The model directly ports the six actions from the upstream TLA+ example.

Observed:

- 16 reachable states;
- 96 enumerated action edges including self/stuttering-equivalent updates;
- shortest `big = 4` witness at depth 6;
- independently accepted type/closure certificate.

The six action labels and resulting states are stored in `spikes/results/spike-results.json`.

### Architectural consequence

Corpus parity can be built incrementally around exact facts. A P2 port should compare normalized state graph and shortest violation semantics, not just “both tools eventually find a solution.”

## Spike 3 — Dining Philosophers

A five-philosopher left-then-right model produced:

- 573 reachable states;
- 2,365 action edges;
- canonical deadlock with every philosopher holding its left fork;
- shortest deadlock depth 10;
- accepted finite type/closure certificate.

### Architectural consequence

Deadlock is a semantic property distinct from invariant failure. The result also supplies the first procedural/concurrency fixture for DPOR and symmetry spikes.

## Spike 4 — stuttering refinement

A concrete register splits an abstract atomic write into:

```text
Reserve(v) → Commit
          ↘ Abort
```

The abstraction forgets pending reservations. Exhaustive checking confirmed every concrete step either stutters or performs an abstract write.

### Architectural consequence

Reserve/commit/abort and cancellation can be connected to abstract atomic specifications with ordinary stuttering simulation before introducing more exotic true-concurrency refinement.

## Spike 5 — observer-indexed independence

`write_x` and `write_y` commute for observers of final `(x,y)` state and for an `x`-only observer, but not for an order-sensitive audit observer.

### Architectural consequence

Independence must be indexed by the observer/property contract. A universal conflict table would either erase audit bugs or miss reductions available to state-only properties.

## Not established

The spikes do not prove:

- CML semantics;
- DPOR soundness;
- TLA+ source parity beyond the hand port;
- Lean code compilation;
- Rust performance;
- fairness/liveness preservation;
- correctness of asupersync adapters.

Each missing fact is attached to a gate rather than buried in prose.
