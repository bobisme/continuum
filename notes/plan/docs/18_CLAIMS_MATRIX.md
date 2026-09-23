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
| C020 | Agent repairs do not weaken properties/assumptions silently | semantic diff and hidden-mutant evaluation | OBSERVED |

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

C023 is observed on the workspace dependency graph and on three cross-path arrows:
the reference engine's certificate that the kernel checks from wire bytes, the
Rust reference oracle against the Python oracle, and the asupersync adapter's
journal against an executable primitive conformance model (A7). That is the whole
scope. The evidence is `tools/check_triptych_independence.py` with its retained
record `tools/triptych/evidence/c023.json` (bn-2270, A7 by bn-ujpz0). The audit
reads the full resolved graph, dev edges included, and derives the roles from the
members. Over it, the checking base links only itself, no producer links the
checking base, the model plane and the program plane do not link each other, and
the graph has no cycle. Every public verdict entry of the checking base takes wire
bytes only. A kernel entry that takes a decoded certificate makes the real Rust
test `checking_has_exactly_one_public_entry_point_and_it_takes_wire_form_bytes`
fail. Seventeen circular mutants are rejected by the audit. The boundary gate owns
five of them, and it rejects each. For the other twelve, the fixture states why
the gate has no rule. Three manifest mutants are applied to a real copy of the
workspace, and both tools reject them. The audit found one gap: the gate accepted
`continuum-certificate -> continuum-asupersync`. bn-2270 closed it.

The A7 witness is
`crates/continuum-asupersync/tests/a7_primitive_conformance.rs`. Its model,
`tests/support/primitive_conformance_model.rs`, is written from docs/02 §5 and §7,
docs/01 §6 and plan §4.1. It covers region and task lifecycle, cancellation
phases, reserve/commit/abort, the obligation ledger, virtual time and bounded
channels. It imports `std` only and reads each journal from its canonical bytes
with its own reader, so it shares no type, decoder, lift or scripted source with
the adapter. The audit rule `a7-model-independent` holds that. The corpus is 7513
real `binding::run` journals over eight program sets and three family alphabets,
from a stride through each exhaustive interleaving space and a seeded random
sample. The model accepts each one, and the lift agrees. At 29761 journal
positions, the model's generated next steps include the step the substrate took,
and the model's guard admits each generated step. A run that leaks an obligation
is rejected by the model at its region's close. Twenty-one curated perturbations of
real journals are rejected by the model, each for its own fault, and by the lift.
Over 23587 single-event deletions and adjacent swaps, the model and the lift give
the same verdict in both directions, with one pinned exception. Since bn-j1a50 the
lift runs the substrate ledger inside the region calculus (RFC 0026 correction 51).
In 33 perturbations, an obligation transfer lands between a cancel request and the
holder's acknowledgement. The model admits each one. The calculus refuses each one,
because it decides "cancelling" per region at the request, not per task at the
acknowledgement. This is correction 51's known strictness, and bn-36wy3 owns it. The
lift errs toward nonconformance, never toward a false `is_total`, and no real
journal falls in that window.

That agreement is the result of two repairs. When bn-ujpz0 landed, the lift accepted
666 of 19345 perturbations that the model rejected, in seven classes. Examples are a
cancellation's effect abort before the task's acknowledgement, a task that completes
as cancelled while it holds an obligation or an armed timer, and a finalize with no
drain report. Each is a rule that docs/02 §7 or research/09 states. When bn-1i050
extended the model to channels, it found ten more classes in the channel lift, 475
perturbations. Examples are a send with no committed send permit, a receive by a
parked receiver, and a message ordinal out of allocation order. bn-1i050 made each
class a typed nonconformance in the lift, and every earlier PR-14 test still passes
unchanged. The evidence record retains one program-side mutant: the binding journals
effect aborts before the acknowledgement. The A7 witness fails on it, and since
bn-1i050 the lift-based conformance test of the same family fails on it too.

Four of the seven RFC 0013 arrows have a scaffold side today: CIR journal to
refinement checker, CML reference to optimized evaluator, evaluator to SMT/PDR
encodings, and native checker to Lean reflective checker. On them the refinement
rules hold only because the crates are empty. When a scaffold side lands, the
evidence record goes stale, `just boundaries` fails, and C023 needs new evidence
for that arrow. This first happened when bn-ybq gave `continuum-model-core` real
code, which left A7 with code on both sides and no cross-path test until bn-ujpz0.
The A7 model is a second account of the primitives, not a proof, and the
adapter's lift is still the only check that `binding::run` runs.

C020 is observed on the intent classification and policy layer. That layer is
the diff assembler `continuum_semantic_diff::artifact::assemble` and
`PolicyTable::verdict` in `continuum-intent`, and it is the whole scope. Every
repair must pass through it (INV-011). The repair layer itself is absent: RFC 0032
repair transactions and Forge are not implemented, so no agent repair ran. The
evidence is `crates/continuum-semantic-diff/tests/c020_hidden_mutant_evaluation.rs`
(bn-bpz0). A seeded generator, written apart from the classifier, makes
agent-style weakening edits over the three Intent Contracts that the T09 baseline
pins. The edits cover the seven plan §5.3 dimensions. The disguises are rename,
reorder, double negation, a vacuous conjunct, a prose rewrite, a self-unlock
rider, mixed bound and observer movement, and two-field compounds. Under an
all-`locked` policy on the agent path, 335 of 339 weakening mutants were flagged
on their target field and closed to `block`. The other 4 did not decode. There
were zero silent passes and zero misses. Sixteen held-out seeds with 2990
weakening mutants gave zero misses. All 44 meaning-preserving controls classified
`unchanged` and were allowed. The retained ledger is
`crates/continuum-semantic-diff/tests/golden/c020_hidden_mutant_evidence.txt`. The
property-diff and review-gate controls are the T09 evidence
(`tools/governance/check_t09_evidence.py`). Under each contract's own policy, 4
mutants are allowed by an owner verb: `fairness: unlocked` in Die Hard. The diff
records each of them. The campaign also found 8 mutants that `no-downgrade`
allowed on an added accepted evidence class. RFC 0031 correction 20 (bn-36luu)
now routes each of them to `review`.
C033, agent repair of concurrent Rust, stays TARGET.
