# Risk Register and Kill Criteria

> **Note:** This file's internal gate citations (e.g. "before G2") use the
> Revision 2 gate scheme of `docs/26_RELEASE_GATES_REV2.md` and may not be
> cited without translation per plan §22 (Rev-2 G2 ≈ Rev-3 G4).

## Risk scale

- Probability: low / medium / high.
- Impact: moderate / severe / existential.
- Evidence state: observed / plausible / speculative.

## R01 — Scope collapse under ambition (delivered: bn-1os6 — Phase A checkpoint; gate-driven delivery, first vertical slice, and no-frontier-engine-before-G2 all verified live with mechanical evidence in notes/plan/notes/R01_RISK_CHECKPOINT.md; fourteen PRs exited, §20's 42-crate list matches crates/ with every frontier-adjacent crate a scaffold)

**Probability:** high  
**Impact:** existential

Failure mode: the project simultaneously attempts a language, runtime, TLC replacement, theorem prover, production monitor, distributed checker, and IDE; nothing becomes indispensable.

Controls:

- gate-driven delivery;
- first vertical slice;
- no frontier engine before G2;
- explicit non-goals;
- separate core/product/research lanes.

Kill signal: after G0 effort, no replayable real-code demo or no project willing to migrate its DST.

## R02 — Excellent DST, failed TLA+ replacement

**Probability:** high  
**Impact:** severe

Failure mode: implementation exploration works, but no standalone abstract modeling, arbitrary abstraction, liveness, or refinement.

Controls:

- abstract model language starts before second project;
- formal-methods gate owner;
- G2 explicitly requires model-before-code;
- liveness roadmap.

## R03 — Circular trust

**Probability:** high  
**Impact:** existential for assurance

Failure mode: asupersync execution, CIR generation, and checker share the same bug and agree.

Controls:

- reference evaluator;
- foreign differential oracles;
- independent kernel;
- certificate evidence;
- semantic mutation tests.

## R04 — Unsound independence

**Probability:** medium  
**Impact:** existential

Failure mode: DPOR/unfolding skips a violating execution because a pack or observer analysis declares dependence incorrectly.

Controls:

- conservative default;
- no-reduction differential corpus;
- pack-level commutation tests;
- checked witnesses;
- certified claims fall back to baseline until POR certificate matures.

## R05 — Hidden nondeterminism

**Probability:** high  
**Impact:** severe

Failure mode: dependencies read ambient time/RNG, spawn foreign threads, or perform untracked I/O; replay and exploration are incomplete.

Controls:

- capability lints;
- MIR audit;
- dependency closure report;
- Lab traps;
- production coverage report;
- assurance downgrade.

## R06 — Asupersync coupling or instability

**Probability:** medium  
**Impact:** severe

Failure mode: Continuum depends on private runtime internals or semantic behavior changes frequently.

Controls:

- one adapter crate;
- exact pin;
- semantic hook contract;
- adapter conformance corpus;
- compatibility branches;
- own CIR and replay format.

Kill signal: required hooks force invasive scheduler forks that cannot be stabilized.

## R07 — State explosion remains dominant

**Probability:** certain  
**Impact:** moderate/severe

Controls:

- partial-order primary;
- multi-grain views;
- symmetry;
- symbolic/PDR;
- compositionality;
- explicit budget/inconclusive results.

The project never claims to solve state explosion universally.

## R08 — Model language becomes a bad Rust clone

**Probability:** medium  
**Impact:** severe

Controls:

- mathematical value semantics;
- relational actions;
- no operational async constructs in abstract layer;
- user studies against Quint/TLA+;
- normalized IR independent of syntax.

## R09 — Model/implementation mapping burden

**Probability:** high  
**Impact:** severe

Controls:

- semantic event generation from capabilities;
- derived views;
- multi-grain mapping;
- mapping diagnostics;
- optional synthesis with independent checking.

Kill signal: mapping code approaches implementation size on multiple projects.

## R10 — Domain packs lie

**Probability:** high  
**Impact:** existential for applied claims

Failure mode: “disk” or “network” abstraction excludes relevant behavior.

Controls:

- fidelity profiles;
- ideal/Lab/host refinement;
- mutation/fault coverage;
- empirical conformance;
- pack version in every claim;
- no generic universal disk semantics.

## R11 — Liveness becomes intractable

**Probability:** high  
**Impact:** severe for TLA+ replacement

Controls:

- finite SCC lane first;
- explicit fairness;
- ranking reduction;
- targeted fragments;
- progress diagnostics;
- accepted inconclusive results.

## R12 — Certificate system too large

**Probability:** medium  
**Impact:** severe

Controls:

- start with finite closure;
- streaming formats;
- code-size covenant;
- proof format reuse;
- certificate benchmarks;
- no certificate for every heuristic until justified.

## R13 — Semantic IR overgeneralization

**Probability:** medium  
**Impact:** severe

Failure mode: CIR tries to encode every model class and becomes unusable.

Controls:

- small mandatory core;
- typed extensions;
- domain packs;
- semantic epochs;
- concrete vertical slices.

## R14 — “Alien math” theater (delivered: bn-33iv — Phase A checkpoint; three controls live with mechanical evidence in notes/plan/notes/R14_ALIEN_MATH_THEATER_EVIDENCE.md; topology-confinement guarantee vacuous until frontier code lands, negative-results policy instance-empty until a §24.5 kill fires)

**Probability:** medium  
**Impact:** reputational/severe

Failure mode: sheaves, homology, HDA, category theory appear in marketing without improving the system.

Controls:

- research labels;
- baseline and threshold;
- no correctness claim from heuristic topology;
- kill criteria;
- publish negative results.

## R15 — Solver proof gap

**Probability:** high  
**Impact:** moderate

Failure mode: SMT solver says UNSAT but proof production/checking lacks theory support.

Controls:

- precise `TRUSTED_SOLVER` label;
- multiple solvers;
- bounded witness replay;
- gradually add Alethe/LRAT/theory checkers;
- never mislabel.

## R16 — Rust compiler churn

**Probability:** high  
**Impact:** moderate/severe

Controls:

- pinned toolchain;
- narrow compiler-internal usage;
- extraction adapters isolated;
- compatibility CI;
- prefer stable metadata over rustc internals where possible.

## R17 — Production tracing overhead/privacy

**Probability:** medium  
**Impact:** severe

Controls:

- semantic sampling;
- payload abstraction/commitments;
- local aggregation;
- configurable event classes;
- overhead budgets;
- explicit monitorability analysis.

## R18 — Agent changes spec to fit bug

**Probability:** high  
**Impact:** existential for trust

Controls:

- spec changes require separate approval;
- agent provenance;
- mutation tests;
- compare property strength;
- no automatic evidence elevation.

## R19 — Performance obsession destroys determinism

**Probability:** medium  
**Impact:** severe

Controls:

- canonical outputs independent of thread schedule;
- deterministic merge;
- thread-count matrix;
- reference mode;
- performance gates do not waive replay.

## R20 — Adoption friction

**Probability:** high  
**Impact:** existential as product

Controls:

- incremental DST-first migration;
- ordinary Rust;
- useful before proof;
- superb counterexamples;
- stable CLI;
- compatibility with Quint/TLA+;
- avoid requiring users to understand every engine.

## R21 — Incremental engine substrate cannot support precise invalidation

**Probability:** medium  
**Impact:** severe

Failure mode: the chosen memoization substrate (salsa-derived or custom; the build-vs-adopt decision is a Phase B ADR per plan §9.1) cannot support the four reuse-edge classes (Exact/Validated/Conservative/Experimental) with precise invalidation; edge classes collapse to Conservative, destroying interactivity (G5).

Controls:

- Phase B build-vs-adopt ADR backed by a spike implementing the four reuse-edge classes and `query.explain_invalidation`, with a measured invalidation-precision baseline;
- Incremental Parity Audit (plan §9.5).

Kill signal: Conservative-collapse on the reference workload.

## Project-level kill questions

At each gate ask:

1. Is Continuum deleting more bespoke infrastructure than it creates?
2. Does the same semantic artifact drive at least three modes?
3. Are claims stronger or merely more convenient?
4. Is replay actually perfect?
5. Can a user model before implementation?
6. Can the independent path catch shared bugs?
7. Is the next frontier feature demanded by evidence?
8. Would using Quint plus existing DST be cheaper and equally strong?

A “yes” to question 8 after G2 is a serious failure signal.
