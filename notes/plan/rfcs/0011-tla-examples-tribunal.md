# RFC 0011: TLA+ Examples Compatibility Tribunal

**Status:** Proposed  
**Target gates:** G9 (corpus interaction parity; translated from the Revision 2 corpus gate ladder, docs/26)  
**Normative corpus:** `corpus/tla-examples/validated-examples.csv`

## Summary

Continuum SHALL maintain an automated Tribunal that compares native Continuum ports against a pinned TLA+ Examples epoch. The Tribunal measures semantic parity, not textual similarity.

## Inputs

Each case directory contains:

```text
cases/TV-009-diehard/
  parity.toml
  source.lock
  model.ctm
  model.toml
  correspondence.md
  expected/
    oracle.json
    state-graph.canonical.zst
    shortest-counterexample.json
  lean/
    DieHard.lean
  mutations/
    wrong-capacity.patch
    broken-pour.patch
```

### `source.lock`

Records:

- repository and commit;
- source module paths and SHA-256 hashes;
- configuration files;
- TLA+ tools version and Java runtime;
- optional Apalache/TLAPS versions;
- model execution command;
- upstream expected outcome.

### `parity.toml`

Declares:

```toml
id = "TV-009"
parity = "P2"

[state]
continuum = "State"
tla_vars = ["big", "small"]
projection = "identity"

[actions]
FillBigJug = "FillBigJug"
BigToSmall = "BigToSmall"

[properties.NotSolved]
kind = "invariant"
expected = "violated"
minimal_depth = 6

[comparison]
mode = "projected-bisimulation"
ignore_stuttering_multiplicity = true
```

## Oracle extraction

The Tribunal supports three progressively stronger oracle paths:

1. **Output oracle:** parse TLC/Apalache/TLAPS results and traces.
2. **Graph oracle:** instrument/export finite initial states, successors, action labels, and normalized values.
3. **Semantic-AST oracle:** export SANY-resolved modules into a versioned neutral representation.

The Java tools are quarantined in the Tribunal. Normal Continuum verification never shells out to them.

## Canonical value bridge

TLA+ values are converted to `CorpusValue`:

```text
Bool | Int | String | ModelAtom
Tuple([v]) | Record({field: v})
Set(multiset-free canonical members)
Function(canonical finite graph)
Sequence([v])
```

Canonicalization must preserve distinctions relevant to the model. Uninterpreted model values carry stable atom identities scoped to the model configuration, not guessed textual ordering.

## Comparison modes

### Exact graph isomorphism

Used when state encodings match and auxiliary state is absent.

### Projected graph isomorphism

Applies declared state projections before comparison.

### Bisimulation

Computes a relation between upstream and native states preserving initiality, observations, and labeled transitions. Strong, weak, or stuttering bisimulation is selected explicitly.

### Trace-language comparison

Used when state graph export is impractical. Bounded traces are canonicalized under stuttering and action mapping; hashes are compared and differences minimized.

### Verdict-only comparison

Permitted only at P1. It never qualifies as P2+.

## Temporal parity

For liveness cases, the Tribunal compares:

- fairness clauses after action mapping;
- accepted/rejected fair lassos;
- SCC witnesses;
- minimal lasso stem/loop where deterministic;
- leads-to obligations;
- deliberate liveness failures.

Search-order differences do not fail parity. Semantic fair-cycle differences do.

## Proof parity

A TLAPS proof is not translated line by line. The case identifies theorem statements and semantic dependencies. A P4 case passes when Lean checks the corresponding theorem over the Continuum model and the executable encoding bridge.

## Metamorphic tests

The Tribunal automatically transforms native and, where safe, TLA+ inputs:

- alpha-renaming;
- action reordering;
- definition inlining/factoring;
- equivalent comprehensions and map updates;
- explicit stuttering action insertion;
- permutation of symmetric atoms;
- auxiliary-state instrumentation.

## Failure triage

Differences are minimized by:

1. model-domain reduction;
2. action-set reduction;
3. expression delta-debugging;
4. trace prefix/causal-core reduction;
5. configuration-key reduction.

The resulting fixture is permanently retained.

## CI tiers

- **PR:** 10-second semantic kernel set.
- **Merge:** all Wave 0 plus changed feature families.
- **Nightly:** all 80 validated families at bounded reference configurations.
- **Weekly:** large models, all Lean proofs, mutation suite, multiple worker counts.
- **Epoch qualification:** full upstream update and historical replay.

## Acceptance

The RFC is implemented when DieHard, Dining Philosophers, EWD840, Paxos, KeyValueStore, and TCP demonstrate all comparison modes needed by their families and the harness can classify a deliberately introduced semantic divergence.
