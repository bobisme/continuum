# Semantic Feature and Backend Matrix

Legend: `N` native baseline, `S` supported by specialized lane, `P` proof-only initially, `R` research, `—` rejected/meaningless.

| Feature | Reference | Explicit | DPOR | SMT/PDR | Lean | Runtime |
|---|---:|---:|---:|---:|---:|---:|
| finite sets/maps/sequences | N | N | N | S | N | N |
| unbounded integers | symbolic | bounded | bounded | N | N | concrete |
| uninterpreted sorts | symbolic | model-bound | model-bound | N | N | mapped |
| higher-order operators | N | elaborated | elaborated | restricted | N | — |
| recursive definitions | guarded | finite | finite | restricted | N | — |
| relational nondeterminism | N | N | N | N | N | controlled |
| stuttering | N | N | N | N | N | event hiding |
| weak/strong fairness | N | N | S | S | N | assumption |
| liveness | N | SCC/lasso | fair DPOR | ranking/PDR | N | monitor/inconclusive |
| symmetry | N | N | N | N | proof | IDs/atoms |
| fresh atoms/names | nominal | R | R | S | N | IDs |
| refinement | N | bounded | causal | symbolic | N | trace/runtime |
| hyperproperties | product | S | restricted | S | N | partial evidence |
| real time | zones | S | interval R | SMT | N | virtual/real |
| probability | distributions | MDP | probabilistic POR R | S | N | sampled |
| adversarial choices | games | game | game POR R | game solving | N | fault model |
| crash/durability | domain pack | N | N | bounded | contracts | N |
| cancellation/obligations | calculus | N | N | bounded | N | asupersync |
| source-level Rust | — | — | concrete SMC | CHC/BMC adapters | contracts | N |

## Capability inference

The elaborator produces a diagnostic such as:

```text
Model requires:
  Finite(Set[Node]) because action Deliver enumerates messages
  Temporal(StrongFairness[Deliver]) because property EventualCommit depends on SF
  Runtime(Storage@v2) because concrete view references sync completion

Applicable engines:
  reference, explicit, fair-DPOR, liveness-SCC, Lean
Not applicable:
  pure IC3: sequence operator unsupported by selected encoding
  probability: model has no probabilistic choices
```

## Definedness

CML separates type correctness from semantic definedness. Division by zero, sequence indexing, recursive nontermination, non-enumerable choice, and opaque foreign operations produce proof obligations or rejection—not arbitrary host behavior.

## TLA+ correspondence

CML equivalents cover TLA+ concepts through semantics rather than glyphs:

| TLA+ idea | CML core |
|---|---|
| `VARIABLES`, priming | state schema and relational next state |
| `UNCHANGED` | frame inference or explicit unchanged set |
| `[A]_v` | action-or-stutter combinator |
| `ENABLED A` | enabledness predicate over relation |
| `WF_v(A)`, `SF_v(A)` | named fairness monitors |
| functions as values | finite/symbolic map/function values |
| `INSTANCE` | parameterized module instantiation |
| `CHOOSE` | theorem witness or explicit executable selection contract |
| PlusCal | procedural surface lowering to labeled actions |
| TLC config | model configuration artifact |
| TLAPS theorem | Lean theorem/verified certificate |
