# Model Language and Developer Experience

## 1. Why a standalone language is necessary

A Rust-only model API would improve implementation linkage but weaken abstraction:

- Rust evaluation order becomes visible;
- mathematical sets/relations become library encodings;
- finiteness is unclear;
- nondeterministic relations become awkward iterators;
- compiler restrictions leak into the model;
- modeling before code becomes less approachable.

Continuum therefore has two surfaces over one semantics:

1. `.ctm`, optimized for abstract models;
2. Rust annotations/traits, optimized for implementation and views.

## 2. Language design principles

- relational, not command-sequence-first;
- pure and total by default;
- typed;
- deterministic semantics;
- explicit nondeterminism;
- explicit finite scopes for exhaustive checking;
- arbitrary mathematical abstraction;
- first-class actions, views, fairness, and properties;
- no hidden I/O/time/RNG;
- source maps preserved through normalization;
- simple enough for agents and humans to transform safely.

## 3. Type universe

Initial:

- `Bool`;
- bounded/unbounded mathematical `Int` and `Nat`;
- finite model atoms;
- tuples/records/variants;
- `Option`;
- sequences;
- finite sets;
- finite maps;
- relations;
- functions over finite domains.

Later:

- algebraic data types;
- rational/real symbolic values;
- clocks;
- probabilities;
- opaque user sorts;
- quantified parameterized sorts.

Implementation types are not automatically model types. Conversion is explicit.

## 4. Actions

Actions are relations over pre/post state. Syntactic sugar may look imperative, but lowering is relational.

```rust
action Commit(n: Node, e: Entry) {
    require role[n] == Leader;
    logs' = logs.put(n, logs[n].push(e));
    unchanged(term, role);
}
```

The checker rejects unspecified state changes unless the action explicitly opts into relational postconditions.

## 5. Nondeterminism

```rust
let n = choose Node where alive[n];
let subset = choose Set<Node> where quorum(subset);
```

Choice is semantic, not randomized. Simulation chooses by seed; exhaustive engines enumerate or symbolize.

## 6. Modules and views

```rust
view Service from Protocol {
    state {
        committed: Map<Key, Value>
    }

    map {
        committed = derive_committed(Protocol.logs)
    }

    visible action ClientCommit maps Protocol.ApplyCommitted;
    invisible action Protocol.Replicate;
}
```

View checker emits exact obligations.

## 7. Property syntax

```rust
invariant Agreement { ... }

transition invariant DurableAck {
    event.kind == ReplyCommitted
      => exists w in history:
           w.kind == StorageSynced
           && w.write_id == event.write_id
           && w <= event
}

eventually RequestCompletes {
    assuming EventuallyStableNetwork, WeakFair(HandleRequest);
    forall r: Request:
        accepted(r) => eventually completed(r);
}

hyperproperty ObservationalDeterminism {
    forall traces a, b:
        same_public_inputs(a, b) => same_public_outputs(a, b);
}
```

Syntax remains provisional until semantics are validated.

## 8. Static analyses

- type/effect checking;
- finite-domain analysis;
- totality/termination for model functions;
- action read/write sets;
- cone of influence;
- symmetry candidate detection;
- hidden state update;
- unused fairness;
- vacuity;
- inconsistent assumptions;
- state-space cardinality estimate;
- unsupported symbolic fragment.

## 9. CLI

```bash
continuum check model.ctm
continuum simulate model.ctm --seed 42
continuum explore model.ctm --engine dpor
continuum verify model.ctm --property Agreement
continuum refine model.ctm --crate ./server
continuum replay crashpack/
continuum explain crashpack/
continuum observe trace.cir --against model.ctm
continuum prove model.ctm --property Progress
continuum claims
```

`cargo continuum` resolves crate/build context.

## 10. REPL

Capabilities:

- evaluate expressions;
- inspect initial states;
- list enabled actions;
- step/choose;
- inspect view state;
- ask why action disabled;
- evaluate property;
- fork execution;
- export crashpack.

## 11. LSP/IDE

- semantic highlighting;
- types and cardinality;
- action dependency graph;
- view mapping;
- state explorer;
- counterexample timeline and causal DAG;
- jump from semantic event to Rust source;
- fairness and assumption display;
- one-click replay;
- proof-obligation panels.

## 12. Diagnostics

Bad:

```text
Invariant failed.
```

Required:

```text
CTV2104 Agreement violated
  abstract state: chosen[epoch=7] = {X, Y}

minimal causal explanation:
  e31 A accepted X in epoch 7
  e44 B accepted Y in epoch 7
  missing causal fact: B observed epoch 8 before e44, but action guard ignored it

model: models/consensus.ctm:118
implementation: src/replica.rs:442
replay: continuum replay crashpacks/CTV2104
```

## 13. Agent-facing protocol

Structured operations:

- `model.inspect`;
- `model.successors`;
- `trace.slice`;
- `trace.minimize`;
- `property.check`;
- `invariant.check_inductive`;
- `refinement.check_step`;
- `pack.explain_effect`;
- `replay.run`;
- `mutant.run`.

Responses use stable schemas, not terminal prose.

## 14. Avoiding syntax lock-in

The normalized semantic AST is stable before the surface language. Format and language revisions can lower to the same AST. A formatter and migration tool are mandatory before declaring syntax stable.
