# Claims Matrix

This document is the seed for a machine-readable ledger. Public claims must never outrun the evidence state.

Evidence states:

```text
HYPOTHESIS
TARGET
OBSERVED
ESTABLISHED-BOUNDED
ESTABLISHED
BLOCKED
REFUTED
```

| ID | Claim | Required evidence | Initial state |
|---|---|---|---|
| C001 | Same semantic inputs produce the same Lab CIR | cross-thread-count/cross-run corpus | TARGET |
| C002 | Crashpacks replay exactly within a semantic epoch | retained replay corpus, clean processes | TARGET |
| C003 | CIR preserves relevant partial-order behavior | reference semantics and projection theorems/tests | HYPOTHESIS |
| C004 | Asupersync adapter captures all supported nondeterminism | API inventory, lint/MIR audits, mutation corpus | TARGET |
| C005 | Baseline DPOR preserves finite safety verdicts | exhaustive differential corpus | TARGET |
| C006 | Exact finite explorer has no fingerprint unsoundness | exact equality and collision injection | TARGET |
| C007 | Abstract model Agreement holds in configured scope | checked finite certificate | TARGET |
| C008 | Runtime implementation refines abstract register in configured scope | checked refinement evidence | TARGET |
| C009 | Continuum replaces one project's bespoke DST | migration metrics and bug corpus | TARGET |
| C010 | Continuum replaces practical TLA+ safety use in two projects | standalone models, refinement, certificates | TARGET |
| C011 | Inductive lane proves an unbounded protocol property | independently checked invariant proof | HYPOTHESIS |
| C012 | Liveness lane handles explicit fairness correctly | differential corpus and ranking/SCC certificates | HYPOTHESIS |
| C013 | Production conformance avoids false total orders | partial-order trace experiments | HYPOTHESIS |
| C014 | Observer-sensitive DPOR improves reduction soundly | property-aware differential benchmarks | HYPOTHESIS |
| C015 | Cubical reduction outperforms optimal DPOR on high-width systems | preregistered benchmark | HYPOTHESIS |
| C016 | Sheaf gluing yields useful compositional diagnostics | direct-SAT comparison and theorem | HYPOTHESIS |
| C017 | Topological coverage improves bug yield | held-out mutant campaign | HYPOTHESIS |
| C018 | Proof-carrying results materially shrink the TCB | independent checker audit and mutation | TARGET |
| C019 | Production instrumentation overhead is acceptable | per-tier real workload benchmarks | TARGET |
| C020 | Agent repairs do not weaken properties/assumptions silently | semantic diff and hidden-mutant evaluation | TARGET |

## Documentation rule

A generated table maps claim states to allowed wording. Examples:

| State | Allowed |
|---|---|
| HYPOTHESIS | “we hypothesize,” “research target” |
| TARGET | “designed to,” “planned” |
| OBSERVED | “observed on [scope]” |
| ESTABLISHED-BOUNDED | “established within [exact bounds]” |
| ESTABLISHED | wording matching formal theorem/contract |
| BLOCKED | no positive public claim |
| REFUTED | explicitly document the negative result |

CI should reject bare “verified,” “sound,” “complete,” “deterministic,” or “production-equivalent” unless linked to an adequate claim row.

## Revision-2 claim additions

| ID | Claim | Required evidence | Initial state |
|---|---|---|---|
| C021 | All 80 validated TLA+ example families have native equivalents | corpus dashboard, port manifests, oracle/proof evidence | TARGET |
| C022 | CML is expressive enough to replace ordinary TLA+ usage in target projects | corpus completion plus real-project modeling studies | TARGET |
| C023 | The semantic triptych avoids circular self-validation | independent path audit and mutation tests | TARGET |
| C024 | Lean receipts faithfully justify Continuum claims | verified encoding/checker, axiom manifest, independent replay | TARGET |
| C025 | Cyclic symmetry preserves the dining model | exhaustive spike over 573 states | OBSERVED-BOUNDED |
| C026 | Observer-indexed independence is monotone under observer coarsening | Lean theorem plus engine differential tests | TARGET |
| C027 | Nominal canonicalization removes only identifier renaming | theorem and fresh-name corpus | HYPOTHESIS |
| C028 | Safety games synthesize actionable environment contracts | benchmark against hand-written minimal assumptions | HYPOTHESIS |
| C029 | Checked projection reduces model/code drift | real protocol study and projection theorem | HYPOTHESIS |
| C030 | Weak-memory local receipts compose safely with distributed models | local refinement theorem and integrated example | TARGET |
| C031 | Proof-producing transformations prevent optimizer-induced false proofs | transformation mutation campaign | TARGET |
| C032 | `Inconclusive` monitoring is calibrated under missing evidence | synthetic and production trace campaigns | TARGET |
| C033 | Agents can repair concurrent Rust without semantic weakening | locked-spec hidden-mutant benchmark | TARGET |
| C034 | Semiring-valued traversal provides reusable analysis without claim confusion | specialized-baseline equivalence and performance | HYPOTHESIS |
| C035 | Lean seed files are verified | successful pinned `lake build`, zero `sorry`, axiom reports | BLOCKED |
