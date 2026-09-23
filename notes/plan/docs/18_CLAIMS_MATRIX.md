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
| C006 | Exact finite explorer has no fingerprint unsoundness | exact equality and collision injection | OBSERVED |
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
| C023 | The semantic triptych avoids circular self-validation | independent path audit and mutation tests | OBSERVED |
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

## Evidence records

A row leaves `TARGET` only with a record here that names the scope of its evidence.

C006 is observed on the reference explorer, `bfs::explore` in
`continuum-engine-reference`. That explorer is the whole scope. The evidence is
`crates/continuum-engine-reference/tests/bfs_exact_identity.rs` (bn-286s). The
visited map keys on the full state vector with derived equality and order, and a
source guard pins that. Under injected fingerprint collisions (total, pairwise, and
pigeonhole on a 4-bit fingerprint) the explorer returns the exact reachable count.
A fingerprint-only deduplication mutant loses states, and the same exactness check
rejects it. The optimized engine `continuum-engine-explicit` is a scaffold today. When
it lands, C006 needs new evidence for it under ADR-0013.

C023 is observed on the workspace dependency graph and on two cross-path arrows:
the reference engine's certificate that the kernel checks from wire bytes, and the
Rust reference oracle against the Python oracle. That is the whole scope. The
evidence is `tools/check_triptych_independence.py` with its retained record
`tools/triptych/evidence/c023.json` (bn-2270). The audit reads the full resolved
graph, dev edges included, and derives the roles from the members. Over it, the
checking base links only itself, no producer links the checking base, the model
plane and the program plane do not link each other, and the graph has no cycle.
Every public verdict entry of the checking base takes wire bytes only. A kernel
entry that takes a decoded certificate makes the real Rust test
`checking_has_exactly_one_public_entry_point_and_it_takes_wire_form_bytes` fail. Fourteen
circular mutants are rejected by the audit. The boundary gate owns five of them,
and it rejects each. For the other nine, the fixture states why the gate has no
rule. Three manifest mutants are applied to
a real copy of the workspace, and both tools reject them. The audit found one gap:
the gate accepted `continuum-certificate -> continuum-asupersync`. bn-2270 closed
it. Five of the seven RFC 0013 arrows have a scaffold side today: CIR journal to
refinement checker, CML reference to optimized evaluator, evaluator to SMT/PDR
encodings, native checker to Lean reflective checker, and adapter to conformance
models. On them the refinement rules hold only because the crates are empty. When
a scaffold side lands, the evidence record goes stale, `just boundaries` fails,
and C023 needs new evidence for that arrow.
