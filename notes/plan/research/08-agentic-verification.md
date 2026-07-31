# Research Note 08: Proof-Gated Agentic Verification

**Claim class:** research program  
**Relevant sources:** [S69], [S70], [S70A]

## Thesis

Continuum should be the verification substrate on which coding agents can safely iterate:

```text
discover counterexample
→ localize causal/refinement failure
→ propose patch or invariant
→ replay exact failure
→ explore neighboring equivalence classes
→ discharge proof obligations
→ independently check certificate
```

The agent is never trusted. It is a search heuristic whose output crosses deterministic evidence gates.

## Machine-legible artifacts

Every failure package contains:

```text
manifest.json
claim.json
causal-trace.cir
minimal-core.cir
linear-replay.cir
state-diff.json
obligation-flow.json
property.json
assumptions.json
replay.toml
explanation.md
```

Stable schemas let an agent reason without scraping terminal prose.

## Causal diagnosis

The explanation engine computes:

- backward causal cone from the violation;
- minimal fault/event core;
- first broken view/refinement edge;
- obligation/resource imbalance;
- effect-phase mismatch;
- competing passing execution;
- source/provenance sites;
- candidate repair templates.

This produces a better repair problem than a 50,000-step interleaving.

## Patch search spaces

Initially restrict synthesis to auditable transformations:

- move acknowledgement after commit/sync;
- add/strengthen guard;
- add epoch validation;
- make operation idempotent;
- add cancellation drain/finalizer;
- reorder independent-looking effects;
- bind task to a region;
- strengthen an abstraction predicate;
- add a missing fairness assumption as a specification proposal, not code.

Unrestricted code generation remains possible but receives no special trust.

## Invariant synthesis

Agents can propose invariants using:

- counterexample states;
- reachable-state samples;
- templates from model types;
- symmetry;
- unsat cores;
- IC3 clauses;
- natural-language design intent.

Every invariant is checked for initiation, consecution, and property implication. “Plausible” invariants do not enter the claims matrix.

## Proof search orchestration

The agent may select engines, decompose goals, request lemmas, and generate Verus/Lean/SMT annotations. Proof objects and deterministic rechecks remain the authority.

Research on AutoVerus and later agent-based Verus systems indicates meaningful automation potential, but evaluations can overfit benchmark styles. Continuum should maintain hidden mutation and semantic-shift sets.

## Anti-reward-hacking controls

- hidden mutants and metamorphic transformations;
- separate model and implementation agents where possible;
- no access to expected counterexample seed in evaluation;
- certificate and replay checks isolated from the agent;
- detect assumption strengthening or property weakening;
- semantic diff of every accepted patch;
- adversarial prompt/data contamination tests;
- bounded tool permissions.

## Novel proposal: Neighborhood Closure Gate

Fixing one schedule can merely move the bug. After replay passes, automatically explore a neighborhood defined by:

- causal swaps around the original core;
- nearby fault placements;
- alternative linearization witnesses;
- small data perturbations;
- one additional cancellation/crash;
- symmetry-renamed instances.

A patch is not accepted until this closure campaign and the original proof obligation pass.

## Novel proposal: Proof-Carrying Repair

A repair submission contains:

```text
patch
failure replay result
semantic diff
new/changed assumptions
certificate(s)
coverage delta
remaining unsupported obligations
```

Review focuses on contract changes and evidence, not agent confidence.

## Evaluation

- known distributed/concurrency bugs;
- hidden mutants;
- novel model/implementation drift;
- cancellation and durability defects;
- invariant synthesis tasks;
- time-to-certificate;
- false-fix rate;
- property weakening rate;
- human review effort.

## Lane status (plan §24.5)

This note owns the **multi-agent evidence graph enforcement (§11.7)** register
row. The row is a concurrency-correctness question about the substrate this
note's whole thesis rests on — an agent is a search heuristic whose output must
cross deterministic evidence gates, and that claim is void if two agents writing
at once can move a claim's status by write order. RFC 0038 carries the §11.7
write and concurrency model (SD-06) but an RFC cannot hold a lane marker;
research/35 owns the adjacent *security* row (no unprivileged path may alter
status), so the enforcement row stays here, where the untrusted-agent gate
discipline is stated. docs/53 §8 fixes the boundary being crossed: the finite
spike validated the authority table and coordination semantics on ten immutable
nodes produced by two proposals in one thread, and explicitly not distributed
storage, persistence, or concurrent daemon behaviour.

**Ratified enforcement criteria.**
quote-id=evidence-graph-enforcement "Multi-agent evidence-graph enforcement is validated only when four §11.7 properties — promotion restricted to trusted service identities, per-claim linearized compare-and-set with no lattice regression and no lost promotion, deterministic conflict materialization, and idempotent retry — hold with zero counterexamples under both exhaustive exploration of a bounded model of the §11.7 write protocol at 3 concurrent writers over 2 claim identities and an adversarial campaign against a persistent continuumd of at least 1,000 distinct seeded schedules per configuration across the docs/19 §7 determinism matrix (writer counts 1, 2, 8, and 32 contending on a single claim identity, debug and release, with process restarts), in which every losing promotion returns the typed StatusConflict of plan §10.3 rather than a generic error, every replayed idempotency key returns the original node identity, every injected contradictory claim pair materializes exactly one Conflict node whose content identity is byte-identical across all schedules, seeds, worker counts, and restarts, and every recorded per-claim promotion history is accepted by an independent linearizability checker against the §11.4 lattice, one violation in one trial failing the lane."

Plan §24.5 quotes this sentence verbatim under the same `quote-id`. The
registered kill is that the four properties can be met only by serializing every
writer behind one global lock, or that conflict artifacts cannot be made
deterministic; the fallback is unchanged — a single-writer evidence graph with
agents coordinating through one integrator role (docs/44, *Merging agent work*).

Rationale. The four properties are not new requirements: they are exactly what
RFC 0038 and plan §11.7 already assert (append-only publication, per-artifact
atomicity under INV-017, promotion linearized per claim identity as a
compare-and-set that cannot regress the lattice, a typed `StatusConflict` for the
loser, `Conflict` nodes instead of last-writer-wins, idempotency keys that make a
replayed write return the original node identity). What was missing was a
falsifiable way to say they are *enforced* rather than tabulated, so every number
below is chosen to be the smallest value that separates enforcement from the
spike. Two writers is the spike's own configuration (two independent patch
proposals), so the model floor is 3 concurrent writers — the smallest count at
which a racer can observe a status that a second racer has already replaced, the
interleaving class a pairwise test cannot construct — over 2 claim identities,
because linearization is claimed *per claim* and a single-claim model cannot
falsify cross-claim interference or a global lock masquerading as per-claim
ordering. Exhaustive exploration is required at that bounded configuration
because Continuum's own product claim is that finite exhaustive exploration
beats sampling on exactly this kind of protocol; a lane about concurrency
enforcement that only sampled its own model would be evidence against the
program. The implementation campaign then reuses the docs/19 §7 determinism
matrix verbatim — worker counts 1, 2, 8, and 32, debug and release, process
restarts — rather than inventing a scale, and the restart dimension is what
converts docs/53's open item (persistent and concurrent daemon behaviour) into a
graded one. The 1,000-schedule floor per configuration is derived, not
decorative: with zero observed violations the rule of three bounds the
per-schedule violation rate below 0.3% at 95% confidence, roughly five times
tighter than the 1-in-64 (about 1.6%) sampled-audit default the program already
labels as acceptable for reuse trust in §9.5, which is the right asymmetry
because a lost promotion is an evidence-integrity fault rather than a cache
mismatch. Every outcome bound is zero-tolerance and per-trial for the same
reason: a promotion race that fails one time in ten thousand still produces a
`Validated` claim nobody checked, so a rate target would be a licence to ship the
fault. The typed-error and byte-identical-conflict clauses make two of the
properties observable rather than argued — `StatusConflict` is the only response
that lets a caller re-read and retry as RFC 0038 requires (a generic error is
indistinguishable from a crash), and content-addressed conflict identity that is
stable across schedules is the same standard docs/19 §7 already applies to
semantic artifacts, so a `Conflict` node whose identity depends on arrival order
is a materialized last-writer-wins result wearing a conflict label. The
independent linearizability checker is named separately because the daemon
asserting its own ordering is the failure mode docs/53 warns about; the check is
already in the reliability list of docs/35 for handle publication, and this row
extends it to promotion histories.

## Product boundary

Agent features must remain optional. Core Continuum is deterministic and usable without an LLM. No proof claim depends on a model service being available or honest.
