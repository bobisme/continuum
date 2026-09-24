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
| C018 | Proof-carrying results materially shrink the TCB | independent checker audit and mutation | OBSERVED |
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
the graph has no cycle. Every public verdict-returning `fn` of the checking base
takes wire bytes only; trait-associated items and other forms are not checked (see
C018's record). A kernel entry that takes a decoded certificate makes the real Rust
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
the adapter. The audit rule `a7-model-independent` holds that. The corpus is 7873
real `binding::run` journals over nine program sets and four family alphabets,
from a stride through each exhaustive interleaving space and a seeded random
sample. Since bn-36wy3 it includes budget-deadline cancellations, which the model
states from docs/02 §7's per-task `request(cancel_reason)` and docs/02 §5's clock.
The model accepts each one, and the lift agrees. At 31650 journal
positions, the model's generated next steps include the step the substrate took,
and the model's guard admits each generated step. A run that leaks an obligation
is rejected by the model at its region's close. Twenty-seven curated perturbations
of real journals are rejected by the model, each for its own fault, and by the lift.
Over 25277 single-event deletions and adjacent swaps, the model and the lift give
the same verdict in both directions, with no exception. Since bn-j1a50 the lift runs
the substrate ledger inside the region calculus (RFC 0026 correction 51). Until
bn-36wy3, 33 of those perturbations were a pinned exception: an obligation transfer
between a cancel request and the holder's acknowledgement, which the model admitted
and the calculus refused, because the calculus decided "cancelling" per region at
the request. RFC 0026 correction 53 gave the calculus the per-task acknowledgement,
and the pinned count is now zero; the pin stays as the regression guard. RFC
0026 correction 55 (bn-28hup) gave both paths one rule for the window between a
cancellation request and the task's acknowledgement. Before it, the lift refused a
timer there and admitted a reserve, and the model refused both but admitted a
commit or a hand-off. Now neither admits any step of the task there, under every
projection.

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

C018 is observed on finite-closure certificates at `continuum-kernel-core` wire
epoch 2, and only there (bn-35y4f). bn-2npu ran the independent checker audit and
the mutation campaign, and at wire epoch 1 its evidence did not show a material
shrink. bn-35y4f added the model-bound epoch and reran both. The audit is
`tools/check_tcb_audit.py`, with its retained record
`tools/tcb-audit/evidence/c018.json`. It reads the resolved dependency graph by Cargo
package id, dev edges included. Over it, the checking base links only its own
workspace members and no external package. No second package, version or renamed
dependency carries a checking-base name. Each of its five crates has one public
verdict-returning `fn`, and it takes wire bytes only. The Rust entry-point tests are
the authority for that. The audit's lexical scan (TCB-02) is a conservative second
line, and it is not complete over the public surface. It refuses whole syntactic
classes rather than proving them safe, and it covers exactly these: `fn` and method
signatures, aliases and renames, public statics, consts, fields and inherent
associated consts, trait methods, and `#[path]`, `cfg_attr`, `include!` and macro
constructs. It does not cover trait-associated consts or types, or forms it does not
name. One such route is pinned as a known miss: a public trait const holding a
verdict-returning callable, bound in a checking-base impl. Both the scan and the Rust
test accept it, and the evidence records that. A complete check needs a public API
surface derived from rustc. The scan is bound to what rustc compiled: the files in
rustc's dep-info for the five crates must equal the files the scan reads. Twenty-two
compiling fixtures each fail it for their own stated reason. The retained record is
checked whole: its controls, rows, summary and measurements must equal a
recomputation, and its digest covers every file under the checking crates and the
workspace build inputs. The record is replaced only by a campaign that passed. The
checking base is 13156 shipped lines by the KCOV counting method. An edit to it fails
`just covenant` until the lane runs again.

The checker campaign is `crates/continuum-certificate/tests/c018_checker_mutation.rs`.
Its ledger is `tests/golden/c018_checker_ledger.txt`, and its corpus is
`tests/c018-corpus/cases.txt`. It covers the seven classes the kernels check: finite
closure at wire epochs 1 and 2, state type, LRAT, SMT proof, ranking and fair-SCC
exclusion. The kernels accept all eleven green certificates. They reject 37 of 38
corrupted certificates, each for the reason it targets. The exception is a false SMT
theory lemma. The kernel accepts it by design, and the claim names `TRUSTED_SOLVER`
and the theory. Of 11643 single-byte mutants, the kernels accept 2005. At wire epoch
2 the oracle is a third reading of the model grammar, written in the test. For each one, an oracle re-derives the
claim from the mutant by a different method, over the kernels' own decoder. No
accepted claim is false about the structure the mutant carries. Some accepted mutants
carry a different relation, such as a transition moved to another table state. The
kernel cannot tell those from a producer's intent.

Thirty-five source mutants of the checking base ran against scratch copies of its
crates. The campaign kills 33 of them. The second survivor turns off the epoch-2
evaluation precharge: no corpus certificate comes near the budget, and the kernel's
own unit test kills it. The counts that follow are bn-2npu's first 25. Thirteen let a lie through, a false claim or a
broken proof. One overstates the assurance of a lemma proof, 6 reject a green, 2 give
a wrong reason, and 2 change only the ledger. The two ledger-only kills are decoder
canonicity rules. The survivor removes an arity guard that no wire input can reach.

The audit found one checker defect, and Codex review cr-10mg1y found a second in the
first fix. Fair-SCC exclusion accepted `eventually goal` for a system that stops at a
non-goal state before any goal visit. Its reachability also ran through goal states,
so it rejected a fair cycle, and after the first fix a dead end, that comes only after
a goal visit. bn-2npu fixed both. Reachability now stops at goal states, and a dead
end or fair cycle refutes the claim only when an execution reaches it without a goal
visit. `crates/continuumd/tests/c018_fair_scc_dead_end_differential.rs` holds the
kernel to the reference engine's deadlock report over the goal-stopped model, and
`crates/continuumd/tests/daemon_evidence.rs` holds the daemon's answer to the
kernel's.

The shrink was measured on the one class with a real producer: finite closure from
`continuum-engine-reference`. At wire epoch 1, twelve producer mutants ran through
`crates/continuum-engine-reference/tests/c018_producer_corpus.rs`. The kernel caught 4,
missed 7 (4 emitter faults and 3 evaluator faults) and 1 was refused before emission.
Each missed mutant wrote a closed, in-domain relation that was not the model's,
because the kernel saw the model only as an envelope digest.

At wire epoch 2 the certificate carries the model's canonical encoding. The kernel
decodes it with its own decoder and re-derives every successor row with its own
evaluator (RFC 0005 correction 1). The rerun has 13 producer mutants over six probes:
three models, and three model invariants. The kernel caught 8: the 4 emitter faults,
the 3 evaluator faults it missed before, and an encoder operator fault. Three were
refused before the kernel was asked: the evaluator's domain check, and two search
faults that the epoch-2 emitter now detects itself. It missed 2, both in the model's
canonical encoder (`identity.rs`, 395 lines): an encoder that drops an initial state
or widens a domain defines another model, and the certificate is true of that model.
The binding of the carried model to the caller's model is `envelope-digest-binding`,
a trusted component. So for this class the producer files with no observed escape
are `bfs.rs`, `certificate.rs` and `model.rs` (3306 lines), and `identity.rs` stays
trusted. The checker for this class is 4577 lines. A whole-domain differential
(`c018_kernel_evaluator_differential.rs`) holds the kernel's evaluator to the model
core's over 400 random models. The durable register's claim-A certificate, 61,504
states and 597,184 transitions, is 14,697,631 bytes at epoch 2, against 117,371,613 at
epoch 1. The kernel checks it and each of its three invariants.

The scope is exact. A wire-epoch-1 claim still lists
`certificate-model-correspondence` and keeps the old residual. On the system path the
binding is enforced (bn-3hk4v). `evidence.verify` is the one daemon path that accepts a
certificate as evidence. It promotes a verified finite-closure claim to `validated` only
when the claim carries a model and that model's encoding equals, byte for byte, the
model the daemon holds for the sealed snapshot the node derives from
(`provenance.inputs`). A true certificate about another model is refused with
`CertificateRejected`. A claim that trusts anything beyond `envelope-digest-binding` is
refused with `InsufficientEvidence`. That is every wire-epoch-1, state-type and
temporal claim, and every LRAT and SMT claim, which trusts its formula or skeleton
correspondence. So is a node that names no held snapshot or two, or a snapshot that is
unsealed or has no registered model. A caller whose grant does not hold the snapshot is
refused before the kernel runs.
`crates/continuumd/tests/c018_model_binding_system_path.rs` holds each case. The daemon
computes its side with the model core's canonical encoder, the same `identity.rs` the
producer uses, so that encoder stays trusted: a fault in it changes both sides alike.
Three more residuals are named, not closed. The model catalog is registered out of band
and is not checked against the snapshot's `.ctm` bytes, and a re-registration does not
recheck claims already validated. The claim's property (its class, its invariant token
and the envelope's property and scope digests) is not compared with the node's claim or
intent, so a true certificate of a weaker property of the right model still binds. An
idempotent replay re-decides the snapshot against the presenting grant, because the replay
record keeps every derived handle the first call decided (cr-3lrkq3).
No wire operation appends a certificate-class node yet, so the tests file nodes
through daemon state. The SAT, SMT and temporal kernels have no producer in the
workspace, so their shrink is not measured.
