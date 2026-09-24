# RFC 0005: Certificates and the Independent Kernel

**Status:** Proposed  
**Target gates:** G2/G3 (Revision 2 scheme, docs/26 — not citable without translation to the docs/52 Revision 3 gates per plan §22)

## Summary

Strong Continuum claims SHALL be backed by machine-checkable evidence. The high-performance engines are outside the trusted computing base whenever practical. A small independent kernel validates semantic well-formedness and certificate families.

The kernel is deliberately boring. Novel mathematics belongs in certificate producers, not in unchecked trust.

## Claim envelope

A certificate is valid only relative to:

```text
model hash
CIR semantics version
property hash
domain-pack profile hashes
bounds
assumptions
engine version
certificate schema version
```

The kernel checks this envelope before content.

## Certificate families

### Counterexample witness

A counterexample certificate contains a legal initial configuration, legal transitions or causal extensions, and a property violation. Counterexamples are generally easier to check than proofs and are mandatory for reported bugs.

### Closed reachable set

For finite safety:

\[
Init \subseteq S,\quad Post(S)\subseteq S,\quad S\subseteq P.
\]

The certificate may encode states explicitly, with Merkle commitments and transition witnesses, or symbolically with proof-producing solver obligations.

### Inductive invariant

A predicate \(I\) plus certificates for:

\[
Init \Rightarrow I,\quad I\land T\Rightarrow I',\quad I\Rightarrow P.
\]

SMT proof terms SHOULD use Alethe where supported. SAT subproofs MAY use LRAT. Unsupported solver steps are either independently recomputed by a small decision procedure or leave the claim solver-trusted.

### Refinement

For relation \(R(c,a)\):

\[
Init_C(c)\Rightarrow\exists a.Init_A(a)\land R(c,a)
\]

and for every concrete step:

\[
R(c,a)\land T_C(c,c')
\Rightarrow
\exists a'.T_A^*(a,a')\land R(c',a').
\]

Certificates may include stutter witnesses, bounded abstract paths, history/prophecy assignments, and observer equivalence.

### Liveness

Possible forms:

- accepting-cycle witness for failure;
- SCC/emptiness certificate;
- transition-invariant decomposition;
- ranking/lexicographic ranking function;
- fairness discharge map;
- liveness-to-safety reduction plus safety certificate.

### Partial-order closure

A prefix certificate commits to events, cutoffs, and continuation-equivalence obligations. The kernel verifies causality/conflict consistency, cutoff mapping, and property preservation assumptions.

### Production conformance

A SAT/SMT solution maps observed events to model events and ordering constraints. The certificate must distinguish fully matched, existentially completed, ambiguous, and impossible traces.

### Model-bound finite closure (correction 1)

A closed-reachable-set certificate that carries only its own successor relation lets the kernel check closure under that relation, not that the relation is the model's. bn-2npu measured the gap: 7 of 12 producer mutants of the reference engine wrote a closed, in-domain relation that was not the model's, and the kernel verified it. So a certificate that names its model only by digest leaves `certificate-model-correspondence` trusted.

Normative (bn-35y4f). `continuum-kernel-core` wire epoch 2 defines the finite-closure family as model-bound:

```text
body := model_len:u32 model          -- the model's canonical encoding, continuum-model/1
        property                     -- 1 state-domain | 2 predicate:token (invariant)
        state_count:u32 state*       -- the canonical table, strictly ascending
        row * state_count
row  := transition_count:u32 (action:u16 target:u32)*   -- indices, strictly ascending
```

The domain, the initial states and the action names are the carried model's. The kernel decodes the model with its own decoder and evaluates it with its own evaluator (see "Kernel layering"). Then it checks, in order:

1. the evaluation work, charged from decoded sizes before any evaluation;
2. every table state lies in the model's declared domain;
3. every initial state of the model is in the table;
4. at every table state, the kernel evaluates every action of the model and locates each successor in the table, and the carried row must equal that re-derived row entry for entry;
5. for the invariant class, the named predicate of the model holds at every table state.

A fault while evaluating (an `i64` overflow, or an update that leaves its variable's domain) is a rejection: the model has no well-defined relation at that state. Every carried index is range-checked at decode. A model section, an expression depth, a node count or an evaluation work over the kernel's bounds is `Unsupported` with the resource named, never a rejection. A verified wire-epoch-2 claim does not list `certificate-model-correspondence`. It still lists `envelope-digest-binding`: that the carried model is the caller's model. The claim exposes the carried encoding, so a caller binds it by exact byte comparison with its own model's identity (ADR-0013).

Wire epoch 1 stays decodable for the two-epoch window of ADR-0018. A wire-epoch-1 claim keeps `certificate-model-correspondence` in its trusted components, so the assurance difference is visible in every verdict and receipt. Wire epoch 2 defines no state-type family. The invariant class exists only at wire epoch 2.

## Kernel layering

```text
continuum-kernel-core
  canonical values
  CIR validation
  propositional/first-order expression evaluator
  transition relation evaluator
  certificate dispatch

continuum-kernel-sat
  LRAT checker

continuum-kernel-smt
  Alethe checker subset

continuum-kernel-temporal
  graph/SCC/ranking certificates
```

A minimal reference kernel SHOULD avoid async, network, plugins, dynamic loading, and unsafe code. It reads content-addressed artifacts and emits a deterministic verdict.

## Independence

The production engine and kernel MUST NOT share optimized evaluator code. They may share generated schema types, but semantics-sensitive functions need independent implementations or mechanically generated definitions from a small formal semantics.

A long-term goal is a mechanized core semantics and extracted/reference kernel in Lean or Rocq, with a native Rust checker cross-validated against it. This is not a gate for v0.

## Proof-producing solver policy

Preference order:

1. independently checkable certificate;
2. two independent solvers plus replayable query;
3. one pinned solver with explicit solver-trusted claim;
4. heuristic result marked non-proof.

No CLI may print a bare “verified” without exposing the assurance class and trusted components.

## Resource bounds

Certificate checking itself is an attack surface. Formats need:

- size limits;
- streaming validation;
- deterministic resource accounting;
- cycle/recursion bounds;
- hash-agility policy;
- rejection of duplicate/ambiguous encodings.

## Kernel tests

- differential tests against engine evaluators;
- hand-built malformed certificates;
- grammar fuzzing;
- mutation testing of each checker rule;
- proof round trips from multiple producers;
- reproducible cross-platform verdicts;
- `unsafe` audit and minimal dependency closure.

## Corrections recorded by this RFC

Per plan §25, where plan prose or docs disagree with this RFC, this RFC governs. The corrections in force:

1. **Model-bound finite closure.** docs/03 §6.1 lists "recompute every enabled transition" as a checker step, and "Kernel layering" places the expression and transition-relation evaluators in `continuum-kernel-core`, but the wire-epoch-1 certificate carried no model, so the kernel could not perform that step. Normative: "Model-bound finite closure (correction 1)" above (bn-35y4f). Direction: the RFC completes docs/03 §6.1 and its own "Closed reachable set"; no contradiction.

## Rejected alternatives

- Trust the Rust engine because Rust is memory-safe.
- Trust an SMT solver's `unsat` answer without recording the query.
- Produce certificates only after the rest of the system is complete.
- One opaque binary proof format coupled to a specific solver.
