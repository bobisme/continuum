# Research 13: Observer-Indexed Independence

## Problem

Classic partial-order reduction defines independence using transition enabledness and commutation on concrete states. Continuum has richer observations:

- abstract views;
- temporal properties;
- fairness eligibility;
- obligation/cancellation lifecycle;
- durability phases;
- security/hyperproperty observers;
- audit order.

Two events can be independent for one claim and dependent for another.

## Prior art

Context-sensitive independence has shown that observers can yield exponentially stronger reductions than context-insensitive relations.

A practical baseline is Parsimonious Optimal DPOR, which explores one Mazurkiewicz class with polynomial worst-case memory in its supported sequentially consistent setting:
https://arxiv.org/abs/2405.11128

Await-aware optimal DPOR avoids executions dominated by pure waiting and can diagnose livelocks:
https://arxiv.org/abs/2208.09259

## Formal proposal

Let `O : Exec → Obs` be an observer. Events `e,f` are independent at configuration `C` when:

1. both orders are executable under the required diamond condition;
2. resulting configurations are equivalent under `O`;
3. emitted observations agree modulo the observer's equivalence;
4. continuation languages are equivalent for the supported property class.

Write `e I_O(C) f`.

### Observer order

`O_fine ⪯ O_coarse` when equality under `O_fine` implies equality under `O_coarse`.

Then the desired monotonicity is:

```text
e I_fine f  ⇒  e I_coarse f
```

This creates a Galois-flavored relationship:

```text
more abstraction ⇔ more possible independence
```

The exact adjunction should be investigated, not asserted prematurely.

## Observer components

An observer is a product of selected components. Product refinement makes the lattice explicit:

```text
StateView × VisibleEvents × Fairness × Obligations × Time × Security
```

Adding a component refines the observer and can only remove independence.

## Certificates

A local commutation witness can be checked by replaying both orders under reference semantics. Global DPOR coverage still needs source/backtracking evidence.

Possible certificate decomposition:

- local diamond witnesses;
- dependence graph;
- source-set coverage per prefix;
- observer lattice hashes;
- reduction theorem ID.

## Risks

- continuation equivalence is as hard as verification;
- local commutation may be insufficient for liveness/fairness;
- dynamic witness cost may exceed savings;
- property changes invalidate caches;
- hyperproperties can observe correlations invisible to single-run state.

## Product strategy

Start conservative:

1. static resource/effect conflicts;
2. view-equality local diamonds for finite safety;
3. observer monotonicity theorem;
4. only then temporal/fairness-aware independence;
5. hyperproperty reductions remain separate until proved.

## Executable spike

`spikes/observer_independence.py` demonstrates the core phenomenon: independent final-state writes become dependent under an audit-order observer.
