# RFC 0003: Continuum Model Language

**Status:** Proposed  
**Target gate:** G2   (Revision 2 scheme, docs/26 — not citable without translation to the docs/52 Revision 3 gates per plan §22)
**Working extension:** `.ctm`

## Summary

Continuum SHALL provide a standalone, typed, relational model language usable before implementation. Rust attributes/macros are a second authoring surface, not the sole language.

The language is intentionally closer to a transition-system calculus than to executable Rust. It preserves nondeterminism and arbitrary mathematical abstraction.

## Design principles

1. **Relational, not imperative by default.** An action describes a relation between pre- and post-state.
2. **Finite checking without pretending finiteness.** Types can be unbounded; a run configuration selects finite bounds or symbolic domains.
3. **Explicit temporal assumptions.** Fairness, time, and faults are named.
4. **First-class views and refinement.**
5. **No accidental execution order.** Set comprehensions and quantifiers remain mathematical.
6. **Predictable elaboration.** No unrestricted macros or arbitrary host-language execution in trusted elaboration.
7. **Stable semantic IR.** Source syntax can evolve without changing CIR semantics.

## Core syntax sketch

```text
module ReplicatedRegister

type Node
type Value
const Nodes: Set[Node]

state {
  epoch: Node -> Nat
  durable: Node -> Option[(Nat, Value)]
  acked: Set[(Node, Nat, Value)]
  crashed: Set[Node]
}

init {
  forall n in Nodes:
    epoch[n] = 0 &&
    durable[n] = None
  acked = {}
  crashed = {}
}

action Propose(n: Node, v: Value) {
  require n notin crashed
  let e = epoch[n] + 1
  next epoch = epoch[n := e]
  next durable = durable
  next acked = acked
}

action Sync(n: Node, e: Nat, v: Value) {
  require n notin crashed
  next durable = durable[n := Some((e, v))]
  next acked = acked union {(n, e, v)}
  unchanged epoch, crashed
}

invariant UniqueEpochValue {
  forall n1,n2,e,v1,v2:
    (n1,e,v1) in acked &&
    (n2,e,v2) in acked
    => v1 = v2
}

fairness weak Sync
```

Exact syntax remains open. The semantics are not.

## Type system

Initial types:

- `Bool`, mathematical integers/naturals, bounded bitvectors;
- finite and symbolic enums;
- tuples, records, tagged unions;
- sets, maps, sequences, multisets, relations;
- uninterpreted sorts;
- opaque external domains with declared equality/canonicalization;
- time and probability types only under extension modules.

Collections are persistent mathematical values. Finite explicit exploration requires enumerability, but the model itself is not syntactically restricted to finite values.

The checker distinguishes:

```text
Finite[T]
Symbolic[T]
Opaque[T]
Enumerable[T]
Canonical[T]
```

These are semantic capabilities, not necessarily surface-level traits.

## Actions

An action denotes a relation \(A(s,p,s')\). The surface supports:

- guarded relational updates;
- `choose` / existential next values;
- universal branching;
- atomic action composition;
- event emission;
- obligation creation/discharge;
- stuttering;
- action schemas parameterized by values.

Imperative sugar may elaborate to a relation only when deterministic assignment order cannot change meaning.

### Relational actions

This subsection is the Phase B semantics of relational postconditions, implemented by `continuum-cml-elab` (bn-2ouro). Later work MAY widen it (relational variables of non-integer types, symbolic successor sets, action schemas). It MUST NOT be narrowed without an epoch note (plan §4.6).

**Post-state reads.** Inside an action, `x'` is the value of the state variable `x` in the post-state. Only a state variable can be primed. A prime on any other expression (`(x + 1)'`, `x''`, a parameter, a constant) is refused as `cml.elab.unsupported.primed_expression`. A prime outside an action is `cml.elab.prime_outside_action`. Unprimed names always read the pre-state.

**Clauses.** An action's statements elaborate to three things:

1. *updates*: `next x = e` or `x' == e`, where `e` reads no post-state, sets `x` to `e` evaluated in the pre-state. `x' == x` and `unchanged x` keep `x`;
2. *guard clauses*: every other conjunct (of a `require` or a bare Boolean statement) that reads no post-state;
3. *postconditions*: every conjunct that reads a post-state, and every `next x = e` whose `e` reads a post-state (through a `let`), which becomes the postcondition `x' == e`.

**Frame rule.** Each state variable of an action is exactly one of: *updated*, *kept*, or *relational*. A variable is relational when no statement updates or keeps it and a postcondition primes it. A variable that is none of these is refused as `cml.elab.unspecified_state_change` (docs/11 §4: "The checker rejects unspecified state changes unless the action explicitly opts into relational postconditions"; priming the variable in a postcondition is that opt-in). A variable updated or kept twice is `cml.elab.conflicting_update`. There is no implicit `unchanged`.

**Meaning.** In a postcondition, a primed read of an updated variable is its update expression, and of a kept variable is its pre-state value; the normalized AST records these substituted. The action then relates pre-state `s` to post-state `s'` exactly when every guard clause holds at `s`, every updated variable takes its update's value, every kept variable is unchanged, and every relational variable takes a value of its type such that all postconditions hold. The action is enabled at `s` when at least one such `s'` exists.

**Explicit successor sets.** Lowering to the programmatic model (whose variables are bounded integers) enumerates the candidate post-states of the relational variables: the product of their declared domains, with the relational variables in name order, the last one varying fastest, and each domain in ascending order. Each candidate `c` becomes one programmatic action named `A[r1=c1,…,rn=cn]` whose guard is the guard clauses and the postconditions with each `r'` replaced by `c_r`, and whose updates are the action's updates plus `r := c_r`. The union of their transitions is the action's relation. An action with no relational variable keeps its name, and its postconditions join its guard.

**Bounds and refusals.** A relational variable must be an integer with a finite declared domain, as every lowered state variable must be: otherwise the lowering refuses with `cml.lower.non_integer_state` or `cml.lower.unbounded_domain`. The programmatic model's own limits are checked before init enumeration and before any action is built: more than `MAX_RELATIONAL_CANDIDATES = 4096` candidates for one action (equal to the model's `MAX_ACTIONS`) is `cml.lower.successor_domain_too_large`; more than `MAX_ACTIONS = 4096` programmatic actions in total, counting every candidate of every relational action, is `cml.lower.too_many_actions`; a generated candidate name longer than `MAX_IDENT_BYTES = 128` bytes (its longest candidate, computed from the domain ends) is `cml.lower.name_too_long`, as is a declared name past that limit; more than `MAX_VARIABLES = 64` state variables is `cml.lower.too_many_variables`. The whole enumeration (candidates × the lowered size of the guard, postconditions, and updates, plus each candidate's name) is charged to the output budget, and checked against what the work budget has left, before the first candidate is built, as init enumeration is; exceeding either is `cml.lower.output_too_large` or `cml.lower.work_limit_exceeded`. Work is then charged as each candidate is built, one unit per node copied and per post-state read resolved; a read resolves by its variable's slot number, in constant time.

## Properties

### State predicates

- invariants;
- inductive invariants;
- state constraints;
- assertion at action boundaries.

### Trace and temporal predicates

- LTL-like safety/liveness;
- weak and strong fairness;
- leads-to;
- bounded temporal properties;
- observer-indexed hyperproperties under a dedicated module.

### Quantitative predicates

Timed/probabilistic properties are not in the v0 core. They use typed extensions so users cannot accidentally interpret statistical evidence as exhaustive proof.

## Configurations and bounds

A model configuration is separate from source:

```toml
[domains]
Node = ["a", "b", "c"]
Value = [0, 1]

[faults]
max_crashes = 2

[assumptions]
fair = ["Deliver", "Recover"]
```

Bounds and assumptions are hashed into every result.

## Views and refinement

A view declares:

- a source state/configuration;
- a target model;
- an abstraction map or relation;
- visible event mapping;
- stuttering policy;
- prophecy/history variables if needed;
- proof obligations.

```text
view RuntimeToProtocol refines Protocol {
  map_state c => {
    committed: c.storage
      .filter(|x| x.phase == Stable)
      .map(...)
  }

  map_event StorageSync => [Commit]
  map_event Poll | Wake | Reserve => []
}
```

A relation may replace a function when abstraction is nondeterministic or telemetry is incomplete.

## Modules and composition

Modules expose typed state fragments, actions, properties, and assumptions. Composition supports shared resources only through explicit coupling declarations. Name-based global-variable merging is forbidden.

The long-term composition model should align with CIR event interfaces and may use assume-guarantee contracts or interface automata.

## Model functions: totality and termination

A `def` is a model function. Elaboration SHALL either show that it terminates or refuse it with a typed error; it never evaluates a function whose termination it has not shown. This section is the Phase B rule, implemented by `continuum-cml-elab` (bn-36x3b). Later work MAY widen it — measures that are not constant at the call but are bounded by a finite domain, structural recursion (for example on sequences), and a measure declared once and shared by several mutually recursive defs. It MUST NOT be narrowed without an epoch note (plan §4.6), because a narrowing turns accepted models into refused ones.

**Measure.** A `def` that calls itself is admitted when one parameter `k`, of type `Nat` or `Int`, is a measure. The first parameter in declaration order that satisfies all three conditions below is the measure:

1. every recursive call passes `k - c` in `k`'s position, where `c` is a constant expression (integer literals with `-`, `+`, `*`, `min`, `max`) whose value is at least 1;
2. on the path from the body's root to the call, the conditions of the enclosing `if`s that compare `k` with a constant (`k == n`, `k != n`, `k < n`, `k <= n`, `k > n`, `k >= n`, either operand order, under any number of `!`) bound `k` below by `c`. A `Nat` measure starts from the bound `k >= 0`, so the `else` branch of `if k == 0` bounds it by 1. An `Int` measure starts unbounded, so `k != 0` bounds nothing there and a test such as `k <= 0` is needed. A branch that no value of `k` reaches is not checked;
3. no recursive call occurs inside the arguments of another recursive call.

A self-recursive `def` with no measure is refused as `cml.elab.unsupported.no_decreasing_measure`, at its first recursive call, whether or not the def is called. A cycle through two or more defs is refused as `cml.elab.unsupported.mutual_recursion`: the surface has no way to state a measure that several defs share.

**Unfolding.** A call of a recursive def is replaced by its meaning, as for any other def, so the normalized AST and the lowered model contain no recursion. The measure argument of a call SHALL fold to a constant `v` by the same constant expressions as condition 1. Otherwise the call is refused as `cml.elab.unsupported.recursion_bound_not_constant`. A negative `v` for a `Nat` measure is refused as `cml.elab.measure_out_of_domain`. At `v`, every measure test in the body is decided and replaced by the branch it takes, the measure parameter becomes the literal `v`, and each remaining recursive call unfolds again at `v - c`. Each unfolding gives the body's binders fresh identities, so an argument that mentions one of them is never captured.

**Bounds.** Source is untrusted (INV-016). The chain of recursive calls from a call at `v` is at most `v / c_min` long, where `c_min` is the least decrement. That bound SHALL be at most `MAX_RECURSION_DEPTH = 64`, checked from the constant before anything is unfolded. The exact size and depth of the unfolding SHALL be computed and charged to the elaboration's output and work budgets before any of it is built. The output depth is bounded like any inlined def, and the nesting of the unfolding (the nodes it builds, the tests it decides, and the calls it follows, along one path) SHALL be at most `MAX_UNFOLD_NESTING = 512`. Exceeding the chain bound, the nesting bound, the depth, or the output budget is `cml.limit.elaboration_too_large`. Exceeding the work budget is `cml.limit.work_limit_exceeded`.

## Execution semantics

A small reference evaluator SHALL define the core. Optimized evaluators, SMT encoders, and foreign frontends are checked against it through differential and metamorphic tests.

The reference evaluator is deterministic except for explicit choices, which are returned to the caller as a finite/symbolic choice frontier.

## Foreign compatibility

Initial foreign paths:

- Quint-to-CIR adapter;
- TLA+ semantic-AST adapter using SANY as an oracle;
- trace import/export;
- optional Alloy/SMT-related encodings later.

Foreign adapters are untrusted translators. Their outputs can be compared with reference tools on a conformance corpus.

## Non-goals for v0

- Full TLA+ source compatibility.
- General-purpose programming.
- User-defined elaborator macros.
- Dependently typed proofs.
- Silent execution of arbitrary Rust functions inside the model.
- A theorem-prover tactic language.

## Open questions

- Unicode/math-like versus Rust-like surface syntax.
- Whether actions should permit explicit event intervals.
- How to expose finite-domain and symmetry declarations ergonomically.
- Whether the language should use refinement types for model well-formedness.
- Whether a minimal core should be mechanized in Lean/Rocq before freezing v1.

## Corrections recorded by this RFC

Per plan §25, where plan prose or docs disagree with this RFC, this RFC governs. The corrections in force:

1. **Termination of model functions.** docs/11 §8 lists "totality/termination for model functions" as a static analysis but gives no rule. Normative: the rule is "Model functions: totality and termination" above (bn-36x3b). Direction: RFC completes docs/11 §8; no contradiction.
2. **Relational postconditions and the frame rule.** docs/11 §4 says unspecified state changes are rejected "unless the action explicitly opts into relational postconditions" but does not define the opt-in or the frame. Normative: "Relational actions" above (bn-2ouro). Direction: RFC completes docs/11 §4; no contradiction. The candidate bound is per action and equals the model's `MAX_ACTIONS`, and the total over all actions is checked in aggregate against the same limit before any action is built (cr-22lrdf).
