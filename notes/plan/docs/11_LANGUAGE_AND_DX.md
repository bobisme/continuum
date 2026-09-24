# Model Language and Developer Experience

> **Status:** Design narrative for the CML surface and the developer-experience
> roadmap, written before implementation and not regenerated since (bn-31ie2
> reconciliation, 2026-09).
>
> **Normative source for CML semantics.** [RFC 0003](../rfcs/0003-continuum-model-language.md)
> is normative for the type universe, actions, run configuration, finite values and
> quantifiers, fairness, and model-function termination (corrections 1-5, plan §25).
> Where this document and RFC 0003 disagree, RFC 0003 governs and this document is
> corrected — never the reverse (plan §25: the plan is a map, not the spec).
>
> **Normative source for the wire and CLI surface.** [`schemas/continuumd-native-protocol.idl`](../schemas/continuumd-native-protocol.idl)
> (RFC 0026) is normative for agent-facing operations; `crates/continuum-cli` is what
> ships as the command line. §9 and §13 below predate both and are corrected inline.

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

**Finiteness under a run configuration (RFC 0003 correction 4; implemented bn-15zfa,
bn-10j7z, bn-23hzh).** A type is *finite* — checkable exhaustively under a run
configuration — when it is `Bool`; an instantiated sort; an enumeration; `Nat` or `Int`
with a configuration bound; or `Option[T]`, a tuple, `Set[T]`, or `Map[K, V]` over finite
types. Sequences, strings, records, and function types are not finite under this
correction and keep their present refusals. A finite collection lowers to a flat
bounded-integer layout in `continuum-cml-elab`, never to a native collection value in
`continuum-model-core` (RFC 0003, "Encoding choice"): the trusted base does not grow for
a front-end feature. `Nat`/`Int` bounds are declared in the run configuration
(`schemas/run-config.schema.json`), never inline in source.

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

**Implemented precisely as RFC 0003's "Relational actions" section states** (correction
2, `continuum-cml-elab`, bn-2ouro): a state variable of an action is exactly one of
*updated*, *kept*, or *relational*; a variable is relational when a postcondition primes
it, which is the opt-in this section names; a variable that is none of the three is the
refusal `cml.elab.unspecified_state_change`. RFC 0003 is normative for the frame rule,
the candidate-set expansion, and the resource charging.

## 5. Nondeterminism

```rust
let n = choose Node where alive[n];
let subset = choose Set[Node] where quorum(subset);
```

Choice is semantic, not randomized. Simulation chooses by seed; exhaustive engines enumerate or symbolize.

**Implementation status.** `choose` parses (`continuum-cml-syntax`) but elaboration
refuses it today, `cml.unsupported.choose`: existential/nondeterministic next-value
choice is not yet lowered. Generic type application is written `Name[T, …]`
(`Set[Node]`, `Map[Nat, Value]`), never `Name<T, …>` — `continuum-cml-syntax::TypeKind::Applied`.
The finite-domain sugar that IS lowered today is `forall`/`exists` quantifier expansion
and action-schema parameter expansion over finite scalar and composite types (RFC 0003
correction 4, "Quantifier expansion", "Action schemas").

## 6. Modules and views

```rust
view Service from Protocol {
    state {
        committed: Map[Key, Value]
    }

    map {
        committed = derive_committed(Protocol.logs)
    }

    visible action ClientCommit maps Protocol.ApplyCommitted;
    invisible action Protocol.Replicate;
}
```

View checker emits exact obligations.

**Implementation status.** `view`, `map`, `visible action`, and `invisible action` are
not implemented: `continuum-cml-syntax`'s lexer has no `view` keyword. This section
remains a design sketch, not a built surface.

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

**Implementation status.** `invariant`, `always`, and `eventually` are lexed and parsed
today (`continuum-cml-syntax`). `transition invariant`, `assuming`, `hyperproperty`, and
a named fairness clause inside a property (`WeakFair(...)`) are not implemented; the only
fairness surface that lowers is the top-level model declaration `fairness weak X` /
`fairness strong X` (RFC 0003, "Fairness (correction 5)").

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

**Landed today:** totality/termination for model functions is not an open design
question; it is RFC 0003's normative rule ("Model functions: totality and termination",
bn-36x3b), and elaboration refuses a non-terminating `def` rather than analyzing it
heuristically. The remaining analyses in this list stay roadmap items; their
implementation status is not asserted here.

## 9. CLI

**Implementation status.** The commands below are the original file-first design sketch
and are not what `continuum-cli` implements. The landed CLI is a handle-based client over
the RFC 0026 wire protocol with eight noun groups —
`snapshot`, `check`, `explain`, `context`, `task`, `debug`, `repair`, `evidence` —
for example:

```bash
continuum snapshot create --components <json>
continuum check start <ws_*> --target <id> --target-kind <kind> --portfolio <p> --states <n>
continuum check result <task_*>
continuum repair begin <crash_*> --gate-profile <profile>
continuum repair review <rt_*>
continuum evidence show <ev_*>
continuum debug open <artifact>
continuum explain compile --evidence-root <handle> --question <text> --bytes <n>
```

A command never takes a bare `.ctm` file path; every command takes its handles on the
command line (`crates/continuum-cli/src/cli.rs`), and there is no `--session` or current
state. There is no `cargo continuum` subcommand. Run `continuum --help`, or read
`crates/continuum-cli/src/cli.rs`, for the current surface. The sketch below is retained
as the longer-term ergonomic-CLI direction for authoring a `.ctm` file directly; it does
not describe what ships:

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

Structured operations (illustrative sketch; superseded, see status note below):

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

**Implementation status.** None of the ten operation names above is on the wire. The
normative, landed operation set is [`schemas/continuumd-native-protocol.idl`](../schemas/continuumd-native-protocol.idl)
(RFC 0026), with over 70 operations across 19 namespaces, including `model.check`,
`model.explore`, `model.compare`, `debug.why_blocked`, `debug.why_enabled`,
`debug.reverse_causal`, `debug.step_abstract`, `debug.step_event`, `refinement.check`,
`refinement.explain`, `failure.minimize`, and `program.replay` — the closest landed
analogues, not a one-to-one rename. The stable-schemas, not-terminal-prose principle this
section states is unaffected and holds for every landed operation (INV-003).

## 14. Avoiding syntax lock-in

The normalized semantic AST is stable before the surface language. Format and language revisions can lower to the same AST. A formatter and migration tool are mandatory before declaring syntax stable.
