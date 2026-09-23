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

### Run configuration (normative)

The sketch above is illustrative. The normative artifact is the *run configuration*, `notes/plan/schemas/run-config.schema.json` (class `https://continuum.dev/schema/run-config.json`, schema epoch 1; correction 3). Schema epoch 1 carries sorts and constants and, since correction 4, optional `Nat` and `Int` bounds (see "Finite values and quantifiers"); faults and assumptions are deferred to a later schema epoch.

- **Sorts.** Every uninterpreted sort the model declares is instantiated by a non-empty, duplicate-free list of element names (printable ASCII, 1 to 128 bytes, at most 2^16 elements). Order is significant: element `i` is the integer `i` when a state variable of the sort lowers to the programmatic model. There is no default instantiation: an unbounded sort gets an explicit bound from the configuration, never a silent one (ADR-0025).
- **Constants.** Every constant the model declares is bound to one value of its declared type, written in the schema's tagged value union (`int`, `bool`, `str`, `elem`, `variant`, `tuple`, `record`, `set`, `seq`, `map`, `none`, `some`). An element is sort-qualified, `{"elem": {"sort": "Node", "name": "a"}}`, and its sort must be the constant's declared sort; element names need not be unique across sorts. A `set` has no duplicate member and a `map` no duplicate key, by value.
- **Capability.** Lowering takes the configuration as an explicit input (`lower_configured`); it is never read from the environment, a file path, or a default.
- **Refusals.** Each is typed: a configuration for another model (`cml.config.other_model`); a missing or extra sort (`cml.config.missing_sort`, `cml.config.extra_sort`); a missing or extra constant (`cml.config.missing_constant`, `cml.config.extra_constant`); a value of the wrong type, including an element of another sort, an unknown element or variant, and a negative `Nat` (`cml.config.ill_typed`); a constant of a function type (`cml.config.unbindable_type`); an integer outside the configuration's own bound (`cml.config.outside_bound`, correction 4). A document the schema does not admit is refused by the reader (`cml.config.json`, `cml.config.header`, `cml.config.shape`, `cml.config.duplicate`, `cml.config.sort_too_large`, `cml.config.bound_too_large`, `cml.config.too_large`).
- **Resources.** Every sort element and every value node is charged to the lowering's output and work budgets before it is used, and the lowered model is subject to the same model-core limits as any other (variables, actions, name length, expression depth, initial states).
- **Lowering.** A state variable of an instantiated sort lowers to the integers `0..size`; a `Nat` or `Int` state variable lowers to its configured bound intersected with its refinement (correction 4); a constant bound to an integer, a Boolean, or a sort element lowers to that literal; equality of sort values is integer equality. Everything else lowers, or is refused, exactly as without a configuration.
- **Identity.** The configuration's content identity is the canonical ID5 encoding of the document with `set` members and `map` entries sorted by their canonical encoding (ADR-0013). It sits beside the lowered model's identity, not inside it, so a model lowered under a configuration keeps the identity of the equal programmatic model. The *run identity* is one typed content identity: the model identity and the configuration identity, each length-prefixed, behind the tag `continuum-run/1`. Two configurations always give two run identities; equal models under equal configurations give equal ones.

### Finite values and quantifiers (correction 4)

**Status: in force as the design (lead ruling on bn-15zfa); implemented in three parts.** Each paragraph below names the bone that implements it. Until that bone lands, lowering refuses what the paragraph describes exactly as "Relational actions" and "Run configuration (normative)" state (`cml.lower.non_integer_state`, `cml.lower.non_integer_value`, `cml.lower.parameterized_action`, `cml.lower.quantifier`); this text does not claim that the lowering does it yet.

- *Implemented (bn-15zfa):* the configuration bound, the typed refusals `cml.config.outside_bound`, `cml.config.bound_too_large`, and `cml.lower.unbounded_type`, and bounded scalar `Nat` and `Int` state.
- *Implemented (bn-10j7z):* enumeration-typed state, action schemas, membership in a set literal or a constant set, and planned quantifier expansion over scalar finite types, with the typed refusal `cml.lower.guarded_overflow`.
- *bn-23hzh (not yet implemented):* collection layouts, static keys, collection expressions, definedness, and the replicated register.

**Encoding choice (ruled).** A collection value lowers to bounded-integer variables of the programmatic model (a *flat layout*). `continuum-model-core` does not change. The alternative, native collection values in the model core, was rejected for three reasons:

1. *Trusted base.* The finite closure certificate carries the programmatic model on its wire form (bounded integer variables, at most `MAX_VARIABLES = 64`, printable-ASCII names). A flat layout is a model the kernel can already check. Native collections would change model-core evaluation, the model identity, the reference engine's state representation, and the kernel's wire form together (INV-004: the checking base does not grow for a front-end feature).
2. *Identity.* `continuum-model/1` stays the identity of every model, integer-only or not. There is no second encoding to version.
3. *Exactness by construction.* The layout is a bijection between the canonical flat states of a variable and the values of its type (below). Reachable sets, successor sets, and invariant verdicts therefore correspond one to one. Nothing is approximated.

The cost is width. A layout is at most `MAX_VARIABLES` slots over the whole model, and init enumeration is bounded by `MAX_INIT_ENUMERATION`. A configuration past either limit is a typed refusal, never a truncation.

**Finite types (scalars implemented, bn-10j7z; collections bn-23hzh).** Under a run configuration, a type is *finite* when it is `Bool`; an instantiated sort; an enumeration; `Nat` or `Int` with a configuration bound; or `Option[T]`, a tuple, `Set[T]`, or `Map[K, V]` over finite types. Its *universe* `U(T)` has a canonical order: `false < true`; a sort in configuration order; an enumeration in declaration order; integers ascending; `None` first, then `Some(u)` in the order of `U(T)`; tuples lexicographic; a set or map by its characteristic vector over `U(T)` or `U(K)`, compared lexicographically. Sequences, strings, records, and function types are not finite in this correction and keep their present refusals.

**Configuration bound (implemented, bn-15zfa).** The run configuration has one optional property, `bounds`: `{"Nat": {"max": n}}` and `{"Int": {"min": a, "max": b}}`, with `0 <= n`, `a <= b`, and at most `MAX_BOUND_VALUES = 2^16` values in each range (equal to `MAX_SORT_ELEMENTS`). `bounds` is never empty. It was added in place (schemas/README: an optional property keeps every valid instance valid and its meaning). JSON Schema cannot relate `min` to `max`, so the document profile states that part, and `notes/plan/tools/run_config_parity.py` checks the reader against the schema and the profile together. The reader refuses a range of more than `2^16` values as `cml.config.bound_too_large` and any other defect as `cml.config.shape`. When present, `bounds` enters the configuration identity, and so the run identity; a configuration without it keeps the identity it had before this correction.

A bound instantiates the type wherever a value of it must be finite: a scalar state variable (bn-15zfa), a quantifier over the type and an action parameter (bn-10j7z), and a map key, a set member, or a component of a collection (bn-23hzh). A `Nat` state variable ranges over `0..=max` and an `Int` one over `min..=max`, each intersected with the variable's refinement; an empty intersection is `cml.lower.empty_refinement`. There is no default. Under a configuration, a `Nat` or `Int` that must be finite and has neither a finite refinement nor a bound is `cml.lower.unbounded_type` (ADR-0025: never bounded silently); without a configuration the same state variable is still `cml.lower.unbounded_domain`. An integer in a configuration value (at any depth) outside the configuration's own bound for its type is `cml.config.outside_bound`, checked after the value's type (a negative `Nat` stays `cml.config.ill_typed`). An update that leaves a bounded domain is `UpdateOutOfDomain` at exploration, never clamped. The results of a bounded run are about the bounded instance.

**Layout (bn-23hzh).** Each state variable `x: T` becomes an ordered list of slots, each a programmatic variable with a finite integer domain:

- a scalar (`Bool` as `0..=1`, a sort or enumeration as its index, a bounded or refined integer as itself): one slot named `x`;
- `Option[S]` over a scalar `S`: one slot `x`, where `0` is `None` and `i + 1` is `Some` of the `i`-th element of `U(S)`;
- `Option[C]` over a composite `C`: a presence slot `x?` (`0..=1`) and the layout of `C` under `x!`;
- a tuple: the layouts of its components under `x.0`, `x.1`, …;
- `Set[T]`: one `0..=1` slot `x{u}` for each `u` in `U(T)`;
- `Map[K, V]`: for each `k` in `U(K)`, the layout of `Option[V]` under `x[k]`.

An element is written by its canonical text (a sort element's name, a decimal integer, `false`/`true`, a variant name, and `(…)`, `{…}`, `[k:v,…]` for composites, in `U` order). A slot name longer than `MAX_IDENT_BYTES` is `cml.lower.name_too_long`. The slot count is computed from the types, saturating, before any slot is built; more than `MAX_VARIABLES` is `cml.lower.too_many_variables`.

A flat state is *canonical* when every slot under an absent entry (a `0` presence slot, or an `Option` slot at `0`) is at its domain's minimum. The lowering conjoins each composite variable's canonicity constraint to the init predicate, and every lowered write writes a whole canonical layout, so every reachable flat state is canonical. The layout is a bijection between canonical flat states and values of `T`. The layout, slot names included, is part of the lowered model's identity: changing it needs an epoch note (plan §4.6).

**Static keys (bn-23hzh).** A map key, a set member tested with `in`, and an index are *static* when, after action-schema and quantifier expansion, they are closed: literals, configuration constants, expanded parameters and bound variables, and constructors over those. A static key selects one slot at lowering time. A key that is not static (it reads state) is `cml.lower.dynamic_key`. A static value outside the instantiated universe of its type is `cml.lower.value_outside_bound`. A computed update that leaves a slot's domain is `UpdateOutOfDomain` at exploration, as for any variable (never clamped).

**Expressions (bn-23hzh).** A normalized expression of a finite type lowers to the list of scalar expressions of its type's layout. Set literals, `union`, `\`, `intersect`, `in`, `notin`, set and map comprehensions over a finite domain, `Some`, `None`, tuples, `m.get(k)`, `m.put(k, v)`, `m[k := v]`, and `m[k]` lower slot by slot; `==` and `!=` on a composite type are the conjunction (or its negation) over the slots. Everything else keeps its present refusal.

**Definedness (bn-23hzh).** `m[k]` is defined only when `k` is a key of `m`. Evaluation is strict, as in the model core, whose connectives do not short-circuit: an undefined read anywhere in a clause is an error at that state, even in a branch that does not decide the result. For an action, the error set is the undefined reads of its guard, and of its updates where the guard holds, which is the order in which the model core evaluates them. When that condition `D` is not the constant `true`, the lowering conjoins `D` to the action's guard (so no successor is computed from an undefined read) and adds the named predicate `A#defined`, the conjunction of `D` over the action's expanded instances. An invariant `I` with undefined reads gets `I#defined`. `#` is not a CML identifier character, so no declared name collides. A state that violates `X#defined` is the typed outcome "undefined read in `X`", never a verdict of a declared invariant, and the verdict of `I` at such a state is not a CML verdict.

**Scalars (implemented, bn-10j7z).** A value of a scalar finite type lowers to its code: `false`/`true` to `0`/`1` (a Boolean parameter or bound variable becomes a Boolean constant), a sort element to its index, a variant to its index in declaration order, an integer to itself. A state variable of an enumeration ranges over its variant indices; like a sort-typed one it takes no refinement (`cml.lower.non_interval_refinement`). `x in S` and `x notin S`, where `S` is a set literal of scalars or a configuration constant set of them (a constant `Set[Bool]` holds `0`/`1`), lower to the balanced disjunction of `x == s` over the members — for a Boolean `x`, `x <=> s` against a literal member and `x` or `!x` against a constant one — `x` lowered once and copied, each copy charged. With no member (`{}`, or a constant bound to an empty set) the result is `false`, lowered as `!(x == x)` (Booleans: `x && !x`), so `x` is still evaluated and fails where it fails (cr-3aqchd). As a rule, no lowering step folds away the evaluation of a subexpression that can fail: a constant fold is taken only when every value it drops is a literal, a bound constant, a parameter or binder value, or an interval shown to fit `i64`.

**Action schemas (implemented, bn-10j7z).** An action with parameters `p1: T1, …, pn: Tn` of finite scalar types expands to one programmatic action per tuple of `U(T1) × … × U(Tn)`, parameters in declaration order, the last fastest, named `A(p1=u1,…,pn=un)` with each value in canonical text (`false`/`true`, an element or variant name, a decimal integer); an action without parameters keeps its name. A sort element may be any printable ASCII name, so in a label each of its bytes `\ , = ( ) [ ]` is escaped with a preceding `\`, which keeps labels injective: distinct tuples never share a name (cr-3aqchd). A relational action expands its candidates per instance, named `A(…)[r=c,…]`. The count, summed over all actions (instances times relational candidates), is checked against `MAX_ACTIONS` before any action is built (`cml.lower.too_many_actions`), with the widest generated name, computed exactly from the longest value text of each universe (`cml.lower.name_too_long`). A parameter of a type with no finite universe is `cml.lower.unbounded_type` under a configuration and keeps `cml.lower.parameterized_action` without one; a parameter of a collection or other non-scalar type keeps `cml.lower.parameterized_action` (bn-23hzh).

**Quantifier expansion (implemented for scalar binders, bn-10j7z; comprehensions and collection domains, bn-23hzh).** `forall` and `exists` over a finite domain expand to a balanced conjunction or disjunction of the body at each candidate, in ascending code order; an empty domain is `true` or `false`. Several binders are the nest of single-binder quantifiers in binder order, so a later domain may read an earlier binder. At most `MAX_BINDER_NESTING = MAX_EXPR_DEPTH = 32` binders are in scope at once, counted as binders entered across nested quantifiers and the binders of one quantifier (not as distinct binder numbers), and checked before each binder is entered. A binder number repeats where the same def is inlined inside its own argument; the inner binder then shadows the outer one lexically and the outer is restored when it ends. The limit exists because one quantifier may list any number of binders, which the source depth does not bound; more is `cml.lower.expression_too_deep` (cr-3aqchd). The candidates of a binder are:

- no domain: the universe `U(T)` of its type, which must be finite; otherwise `cml.lower.unbounded_type` under a configuration and `cml.lower.quantifier` without one;
- a range `a..b` or a set literal of scalars whose bounds or members are *static* (their value is fixed once parameters and enclosing binders are fixed): exactly its values, unguarded. The plan, which sees parameters and binders as intervals, recognizes such a domain as one that reads no state variable and whose bounds or members compute without overflow over those intervals;
- a range or set literal that reads state: every value of the *interval hull* of its bounds or members — computed by interval arithmetic from literals, constants, parameters, binders, and the declared domains of the state variables it reads — with each instance guarded by membership (`forall`: `(a <= u && u <= b) => body[u]`, or `(u == e1 || …) => body[u]`; `exists`: the guard `&&` the body). Every member of the domain is in the hull, so the expansion is exact. The plan binds the binder to every value the build may choose: the hull, and for a range also the whole interval of its lower bound (a range empty in every state keeps one candidate there, see below);
- a configuration constant set of scalars: exactly its members.

A post-state read in a domain has an interval, and can fold, only in a postcondition and of a relational variable; anywhere else it is refused as `cml.lower.primed_outside_postcondition`, as everywhere (cr-3aqchd). A `Bool` binder takes only its whole type (a domain is `cml.lower.non_integer_value`); a binder of a non-scalar type is `cml.lower.quantifier` (bn-23hzh). The model core evaluates both sides of `=>` and `&&`, so a guarded instance evaluates its body also where the guard is false. That is the CML meaning only when nothing evaluated under the guard can fail there. Everything under a guard — the remaining binders' domain bounds and members, the guards and membership operands of nested quantifiers, and the body — is lowered by the same integer lowering, and the plan computes the interval of every integer node it plans: inside a guarded instance, with every enclosing binder at its whole hull and every parameter at its whole universe, a node whose value or any intermediate result may leave `i64` is refused as `cml.lower.guarded_overflow`, rather than add an evaluation error the model does not have (cr-3aqchd). A quantifier's own guard is not under its guard: its bounds and members are evaluated by the CML domain too. Unguarded instances are evaluated exactly at the members, so their errors are the model's. A range that is empty in every state lowers to the constant `true` (`false`) only when its bounds compute without overflow; otherwise it keeps one guarded candidate, so the bounds are still evaluated and their errors are still the model's. Action schemas and relational postconditions add no evaluation outside the CML meaning: an instance is evaluated at a parameter tuple of the declared universes, and a relational candidate at a value of the variable's domain, each of which the CML relation ranges over; guard clauses and postconditions are evaluated strictly in both. Comprehensions (bn-23hzh) expand in the same way, one entry per candidate.

**Resources (implemented for quantifiers and action schemas, bn-10j7z; layouts, bn-23hzh).** Every expansion is planned before it is built. The lowering of an expression is one code path run in two modes: the *plan* builds nothing and returns the measure (size and depth) the build will have, and predicts the build's work; the *build* constructs the expression. A quantifier is planned once, with its binder at its whole candidate interval, and its measure and predicted work are multiplied by its fan-out (the candidate count; for a guarded domain, the hull size), so nested quantifiers multiply; an action schema multiplies its planned guard, postconditions, updates, and instance name by its instances. Products saturate. A plan is exact for an expression without a quantifier and an upper bound otherwise, because a candidate interval only narrows when parameters and binders become values. Each clause of an init or invariant, each update, and each whole action is one planned *site*: the planned depth, balanced trees included, is checked against `MAX_EXPR_DEPTH` (`cml.lower.expression_too_deep`); the predicted work is checked against the work left (`cml.lower.work_limit_exceeded`); then the planned size is charged to the output budget in one step (`cml.lower.output_too_large`), all before the first node is built. The build draws its nodes from that charge (what the plan over-estimated stays charged, never refunded) and spends its work as it runs, each unit before the step it pays for. The predicted work is also checked against the work left at every step of the plan, so a plan stops at the first step that decides the refusal. Configuration sets are shared by reference, never copied; the sort of a static domain's members is spent before it runs, in the plan as in the build; the implicit `true` guard of an action without guard clauses is a charged node (cr-3aqchd). Each record the lowering appends to the model (a variable, a predicate, an action instance, a relational candidate, or an initial state) is one charged node with its owned name, each update also owns its target's name, and each initial state holds per variable a binding, a node with its own copy of the name, all charged by length before the copy (cr-3aqchd). Under the default output budget an init enumeration therefore reaches `cml.lower.output_too_large` before `MAX_INIT_BINDINGS`, which remains the memory bound under a larger budget. A relational action's candidates are checked in aggregate over all instances before the first instance, and charged per instance as "Relational actions" states. Static values (configuration constants such as `Quorum`) are held as finite member lists, charged by their nodes, and are never widened to a characteristic vector unless a layout needs one.

**Scope.** Fairness still does not lower (`cml.lower.fairness`, bn-1ln12): the programmatic model has no fairness assumption to carry it, so it is not trivial. Relational variables of composite type are not in this correction and keep `cml.lower.non_integer_state`.

**Example (bn-23hzh).** Under the schema example configuration, which carries `"bounds": {"Nat": {"max": 1}}`, the replicated register (without its fairness declaration) has 14 slots — `chosen[0]`, `chosen[1]` (`0..=2`); `stable[a]?` and `stable[a]![0]`, `stable[a]![1]` for each of three nodes; `alive{a}`, `alive{b}`, `alive{c}` — and 50 actions (12 `Stabilize`, 32 `Choose`, 3 `Crash`, 3 `Recover`). Its init domain has 419,904 flat states, within `MAX_INIT_ENUMERATION`. `"max": 2` gives 34,012,224, which is `cml.lower.init_domain_too_large`.

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
3. **Run configuration.** The "Configurations and bounds" sketch showed an informal TOML document and said only that bounds are hashed into every result. Normative: the run configuration is `schemas/run-config.schema.json`, with the rules of "Run configuration (normative)" above (bn-3a9sr). The sketch's `[domains]` is the schema's `sorts`; its `[faults]` and `[assumptions]` are not in schema epoch 1. Direction: the RFC and its schema govern; the sketch is illustrative.
4. **Finite values and quantifiers.** Lowering refused collection-valued state, parameterized actions, and quantifiers, so the replicated register did not lower, and `Nat` had no configuration bound. Normative: "Finite values and quantifiers" above — a flat bounded-integer layout for finite collections (encoding choice ruled by the lead on bn-15zfa), action-schema and quantifier expansion planned and charged before it is built, strict definedness for `m[k]`, and the optional `bounds` property of `schemas/run-config.schema.json`, added in place. Implemented in three parts: the bound and bounded scalar state (bn-15zfa); scalar values, action schemas, and quantifier expansion (bn-10j7z, including the typed refusal `cml.lower.guarded_overflow` for a guarded body that may overflow); collection layouts, definedness, and the register (bn-23hzh). A part is in force as design now and in the lowering when its bone lands; until then its refusals govern. Direction: the RFC completes docs/11 §3 (finite sets, finite maps, `Option`) and ADR-0025 (explicit finite scopes).
