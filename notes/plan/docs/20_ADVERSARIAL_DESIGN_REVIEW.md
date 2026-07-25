# Adversarial Design Review

This document argues against Continuum as currently conceived. Every objection needs evidence, mitigation, or acceptance.

## Objection 1: This is five PhDs and ten products disguised as one repository

Correct. The full vision spans language design, async runtime semantics, model checking, theorem proving, runtime verification, storage modeling, and developer tooling.

**Mitigation:** the product wedge is much smaller:

```text
asupersync Lab → CIR → systematic exploration → causal crashpack
```

Standalone models and refinement follow only after one real DST migration. Frontier engines are separate research programs. The project succeeds commercially/operationally before completing the whole vision if it replaces bespoke DSTs with superior diagnostics.

**Kill signal:** G1 cannot remove substantial bespoke infrastructure from a real project.

## Objection 2: “One semantics” is a category error

Production hardware, Rust abstract machine behavior, asupersync runtime, model language, and SMT encodings cannot literally be one operational semantics.

**Resolution:** one **normative semantic contract** with multiple abstractions and refinements. Implementations remain distinct. The project must never use shared code agreement as proof of correspondence.

## Objection 3: Building on asupersync makes adoption tiny

Asupersync is young and nonstandard. Requiring it may make Continuum irrelevant to Tokio-dominated Rust.

**Response:** asupersync is the high-fidelity native substrate because cancellation/lifecycle semantics are structurally superior. Continuum also defines adapter boundaries and standalone models. Tokio/foreign adapters can exist at lower assurance. Do not weaken the core to chase early ubiquity.

**Kill signal:** required asupersync hooks become invasive or the runtime cannot sustain production adoption.

## Objection 4: A model extracted from code is not abstraction

Correct. Automatic extraction tends to preserve implementation complexity and prove the wrong thing.

**Resolution:** standalone relational models are mandatory. Rust views connect code to abstraction; they do not replace abstraction.

## Objection 5: Partial-order-first semantics complicates everything

Yes. State-based model checking is mature, simple, and often enough.

**Response:** the project needs partial orders for DPOR, production traces, and causal explanation anyway. CIR can project to a state graph. The proof burden is justified only if reduction/diagnostics outperform a state-first baseline.

**Kill signal:** CIR overhead dominates and no meaningful partial-order benefits appear by G2.

## Objection 6: Domain packs become a new universe of fake models

This is likely. Storage/network fidelity is context-dependent and easy to oversell.

**Resolution:** profile classes, qualification evidence, adversarial envelopes, explicit unsupported behavior, and claims tied to exact profile hashes. The best default is conservative nondeterminism, not realism theater.

## Objection 7: Proof certificates are premature architecture astronautics

Counterexample checking and closed-state witnesses can be built early with modest effort. Full SMT/liveness certificates are not.

**Resolution:** start with small witness checking. Expand certificate families only when an engine produces strong positive claims. The typed TCB disclosure exists from day one.

## Objection 8: Rust is not the right model language

Agreed. Rust is the implementation language and one authoring surface. The standalone model language is relational and mathematical.

## Objection 9: State explosion wins

It always can. Continuum does not promise universal verification. It promises a coherent escalation path:

```text
sample → partial-order search → exact finite → symbolic → induction → proof
```

Every lane reports its scope. Some systems remain beyond reach.

## Objection 10: The exotic mathematics is cosplay

It will be unless each idea beats a simpler baseline or produces a theorem/certificate/diagnostic that ordinary techniques cannot.

**Resolution:** experimental governance, preregistration, negative benchmarks, and deletion. No topology/sheaf/category code in the critical path before evidence.

## Objection 11: Runtime trace validation can never confirm correctness

Often true under incomplete asynchronous evidence. Some properties are only refutable, not confirmable.

**Resolution:** monitorability analysis and `Inconclusive`. Production evidence complements, never replaces, design-time proof.

## Objection 12: The project could verify its own mistaken semantics

This is the deepest risk.

**Mitigation:**

- independent reference evaluator;
- differential foreign oracles;
- small kernel;
- proof/counterexample certificates;
- mechanized core semantics eventually;
- semantic mutation testing;
- avoid shared optimized code between producer/checker;
- public adversarial corpus.

## Objection 13: Developer friction will kill adoption

Explicit capabilities, models, views, packs, and properties are work.

**Response:** Continuum must delete more bespoke framework code than it adds. Macros/generation can remove ceremony, but semantic declarations remain. Measure integration LOC and time-to-first-bug.

## Objection 14: The project will optimize benchmarks and miss reality

Use real migrations, retained historical bugs, hidden mutants, negative workloads, and production traces. Freeze benchmark governance before performance work.

## Objection 15: Agentic features create security and epistemic risk

Agents can weaken properties, assumptions, or instrumentation.

**Response:** agents have no authority over claims. Semantic diffs, hidden mutants, replay, certificates, and human review gate changes.

## Decision

Proceed only as a sequence of falsifiable vertical slices. The vision is worth pursuing because the semantic center is coherent; it is not licensed to skip gates because the end state is exciting.

## Revision-2 objections

### Objection 12: The TLA+ Examples corpus will turn Continuum into a compatibility clone

It could. The remedy is semantic parity rather than syntax parity. The corpus controls completeness, not UX or architecture. Features are generalized into reusable semantics and libraries; port-specific operators are rejected.

**Kill signal:** corpus work produces a growing pile of named exceptions instead of shrinking shared infrastructure.

### Objection 13: Lean will consume the project before users get value

This is a serious risk. The formalization boundary is semantic load-bearing walls and certificate checkers, not the optimized runtime or every domain pack implementation.

**Kill signal:** proof work repeatedly blocks executable counterexamples without protecting a real claim. Narrow the theorem surface, use per-instance translation validation, and preserve the proof receipt contract.

### Objection 14: “Independent paths” will duplicate everything

Some duplication is intentional. A tiny reference evaluator and certificate checker are cheaper than discovering that all engines share one semantic bug. Independence is concentrated at trust boundaries, not every utility function.

### Objection 15: 80 examples is a vanity metric

It becomes vanity if ports only parse or reproduce expected answers. Required parity includes behavior, failures, temporal/refinement semantics, mutations and theorem intent. The corpus is necessary but not sufficient; real asupersync refinements remain mandatory.

### Objection 16: Observer-indexed independence is too subtle to trust

Correct. The default remains conservative dependence. Observer-specific reduction is opt-in until preservation theorems, witness checking and differential campaigns mature.

### Objection 17: Nominal sets, cubical complexes and sheaves are academic distraction

They are quarantined research lanes with baselines and kill criteria. First-occurrence renaming, ordinary DPOR and SAT unsat cores are the baselines. No exotic abstraction enters the critical path on aesthetic grounds.

### Objection 18: Generated protocol interfaces will force users into an actor DSL

Projection generates traits/types/monitors, not an entire architecture. If idiomatic Rust cannot implement the generated seam cleanly, narrow or remove generation.

### Objection 19: A proof receipt can create false confidence through provenance theater

A receipt is useful only if its checker recomputes closure hashes, validates transformation evidence and links a Lean theorem to the original semantics. JSON fields and signatures alone prove nothing.

### Objection 20: Weak-memory support explodes the scope again

Weak memory is a separate hierarchical lane. SC is acceptable for abstract corpus ports. Real lock-free refinement requires a local receipt; distributed verification consumes the atomic summary.

### Objection 21: Agents will “fix” bugs by changing the spec

Property, assumption, observer, bound and fault-model changes are privileged. Repair evaluation locks them and emits semantic diffs. Any such change requires explicit review.
