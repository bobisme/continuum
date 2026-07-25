# Assurance Model and Trusted Computing Base

## 1. Problem

Verification tools often collapse fundamentally different evidence into “passed.” Continuum forbids that. A result is a typed claim whose meaning is stable across CLI, CI, documentation, and production monitoring.

## 2. Assurance claim schema

Conceptually:

```rust
struct AssuranceClaim {
    subject: SubjectDigest,
    semantic_epoch: SemanticEpoch,
    model_scope: Scope,
    property: PropertyRef,

    semantic_coverage: SemanticCoverage,
    exploration: ExplorationClass,
    proof: ProofClass,
    linkage: ImplementationLinkage,
    observation: ObservationCoverage,

    assumptions: Vec<Assumption>,
    trusted: Vec<TrustedComponent>,
    excluded: Vec<ExcludedBehavior>,

    witness: Option<ArtifactRef>,
    certificate: Option<ArtifactRef>,
    reproduction: Reproduction,
}
```

## 3. Dimensions

### Semantic coverage

| Level | Meaning |
|---|---|
| `MODEL_ONLY` | abstract semantics only |
| `CONTROLLED_CORE` | implementation core uses modeled capabilities |
| `CONTROLLED_CLOSURE` | relevant dependency closure audited |
| `PRODUCTION_OBSERVED` | real execution emitted semantic evidence |
| `HOST_COMPLETE` | host boundary coverage justified for declared property |

### Exploration

| Level | Meaning |
|---|---|
| `ONE_RUN` | one deterministic execution |
| `SAMPLED` | multiple generated executions |
| `BOUNDED_CHOICES` | all executions within schedule/fault bounds |
| `BOUNDED_STATES` | all states within declared model scope |
| `EXHAUSTIVE_FINITE` | complete reachable finite state space |
| `INDUCTIVE` | unbounded executions within a symbolic domain |
| `PARAMETERIZED` | quantified/cutoff argument for instance sizes |
| `STATISTICAL` | confidence/precision statement, not proof of impossibility |

### Proof evidence

| Level | Meaning |
|---|---|
| `ASSERTION_ONLY` | engine reports result |
| `REPLAYED_WITNESS` | counterexample independently replayed |
| `CROSS_ENGINE` | independent engines agree |
| `CHECKED_CERTIFICATE` | small kernel checked evidence |
| `MECHANIZED_KERNEL` | checker correctness linked to proof assistant artifact |

### Implementation linkage

| Level | Meaning |
|---|---|
| `NO_LINK` | abstract model only |
| `TEST_GENERATION` | model traces run against code |
| `TRACE_CONFORMANCE` | code traces accepted by model |
| `STEP_REFINEMENT` | concrete transitions refine abstract transitions |
| `PROGRESSIVE_REFINEMENT` | infinite stuttering excluded |
| `STRONG_OBSERVATIONAL` | selected hyperproperties preserved |

These dimensions form a product lattice; they are not collapsed into a single “level 5.”

## 4. Human-readable verdict examples

```text
SAFE
  property: Agreement
  exploration: exhaustive finite
  scope: nodes=3, values=2, crashes≤1
  evidence: closure certificate checked by continuum-kernel 0.3
  implementation linkage: none
  assumptions: network may drop/reorder/duplicate; no Byzantine behavior
```

```text
NO VIOLATION OBSERVED
  property: NoLeakedObligation
  exploration: 5,000,000 sampled Lab executions
  evidence: deterministic replay for all retained failures
  implementation linkage: controlled core
  warning: not exhaustive
```

```text
REFINES
  concrete: server crate build 98af…
  abstract: LeaseProtocol digest d31c…
  linkage: step refinement over 44,182 bounded configurations
  evidence: checked refinement certificate
  excluded: FFI metrics exporter and allocator behavior
```

## 5. Certificate checker architecture

`continuum-kernel` should be:

- single-threaded;
- deterministic;
- allocation-bounded where practical;
- `#![forbid(unsafe_code)]`;
- independent of search data structures;
- able to stream large certificates;
- fuzzed against malformed/adversarial input;
- specified separately from the emitter.

Suggested initial code-size covenant: fewer than 15,000 non-test lines across decoder, semantics, and certificate checks. The number is a forcing function, not a proof of trustworthiness.

## 6. Certificate formats

### 6.1 Closed finite state space

A certificate includes:

- canonical state table;
- initial-state IDs;
- action schemas;
- for each state/action, exact successor IDs or disabled witness;
- invariant evaluation data or values sufficient to recompute it;
- optional symmetry representative map;
- model and property digests.

Checker:

1. decode and canonicalize;
2. verify all initial states are present;
3. recompute every enabled transition;
4. verify successor closure;
5. evaluate property for every state;
6. verify symmetry map if used.

A smaller certificate can use an independently checkable Merkleized state table, but the checker must still validate closure.

### 6.2 Inductive invariant

Certificate supplies \(Inv\):

- \(Init \Rightarrow Inv\);
- \(Inv \land Step \Rightarrow Inv'\);
- \(Inv \Rightarrow Property\).

Backend proofs can be Alethe/LRAT/solver-specific plus checked theory lemmas. Until theory-proof support is mature, the result is `TRUSTED_SOLVER`, not `CHECKED_CERTIFICATE`.

### 6.3 Refinement

Certificate supplies:

- abstraction function or relation;
- initial mapping;
- visible-step simulation;
- stuttering classification;
- progress measure where required;
- fault/fairness mapping.

The checker can enumerate finite concrete domains or validate symbolic obligations via checked solver proofs.

### 6.4 Partial-order coverage

A certifying POR result needs:

- event schemas;
- explored configurations/prefix;
- dependency relation;
- commutation/diamond witnesses;
- backtracking/source-set coverage evidence;
- property-preservation class.

This is research-grade. Baseline exhaustive claims can fall back to unreduced search until the certificate is trustworthy.

### 6.5 Liveness

Finite graph:

- SCC decomposition;
- reachable-component witnesses;
- fairness acceptance labels;
- absence of accepting SCCs or explicit fair-cycle witness.

Ranking:

- well-founded domain;
- decrease obligations;
- fairness-to-progress linkage;
- safety side conditions.

### 6.6 Probabilistic

- rational/interval value function;
- Bellman inequalities;
- scheduler quantifier;
- residual/error bounds;
- exact arithmetic or checked interval rounding.

## 7. Solver trust classes

```text
SAT:
  model replayed successfully       → witness checked
  model not replayed                → solver-trusted witness

UNSAT:
  Alethe/LRAT proof checked         → certificate checked
  proof unavailable                 → solver-trusted
  multiple solvers agree            → cross-engine, still not proof
```

The CLI makes this distinction prominent.

## 8. Diversity against common-mode bugs

Independent paths should differ in:

- language/runtime;
- state representation;
- traversal;
- arithmetic;
- parser;
- certificate decoding;
- solver.

Practical progression:

1. simple Rust reference evaluator vs optimized Rust engine;
2. foreign oracle (TLC/Quint/Apalache) for compatible corpus;
3. tiny Rust kernel;
4. mechanized semantics/checker in Lean or Rocq;
5. generated conformance vectors between implementations.

## 9. Pack axioms and trust

Some host facts cannot be proven by Continuum. Example: “an acknowledged `fsync` has the documented kernel/filesystem/device semantics.” Packs list:

- modeled guarantees;
- measured assumptions;
- trusted external contracts;
- unsupported hardware behavior;
- observation gaps.

An assurance result includes the exact pack fidelity profile.

## 10. Claims ledger

Each claim record:

```yaml
id: CLM-EXPLICIT-001
text: "Exact explicit exploration is collision-safe."
status: PROVEN_BY_TEST_AND_REVIEW
scope: "continuum-explicit 0.4, canonical state backend"
fresh_until: 2026-10-01
evidence:
  - cargo test -p continuum-explicit collision_corpus
  - artifacts/CLM-EXPLICIT-001/report.json
caveats:
  - "Does not cover experimental GPU backend."
```

Documentation CI rejects stronger wording than the ledger allows.
