# Theory of Success

## The product wedge

Continuum succeeds first by replacing bespoke deterministic simulation in real Rust projects:

```text
one dependency
one semantic event model
one replay artifact
one fault/scenario language
one verifier command
```

The initial buyer/user does not need to care about category theory or theorem provers. They need a concurrency bug reduced from 80,000 log lines to nine causal events with a one-command replay.

## The credibility wedge

The TLA+ Examples corpus prevents the product wedge from becoming a glorified test runtime. Lean-checked certificates prevent performance optimizations from becoming a trust exercise.

Together:

```text
immediate utility          long-term assurance
DST replacement      +     corpus/Lean parity
```

Neither is sufficient alone.

## The agent wedge

Agents amplify both sides:

- they can write ports, models, invariants, proof sketches and repairs;
- Continuum turns their output into checked artifacts;
- minimized causal counterexamples reduce context and hallucination surface;
- proof goals and evidence graphs give agents objective progress signals.

The likely immediate-future workflow is not a human hand-writing every invariant. It is an agent generating candidates against a verifier that refuses bullshit.

## Adoption path

### Level 0 — replayable DST

No model language required. Existing Rust tests move onto asupersync/Continuum domain packs.

### Level 1 — semantic events and assertions

Concrete invariants, fault campaigns, causal minimization.

### Level 2 — abstract view

One abstraction map and model-generated scenarios.

### Level 3 — standalone model and refinement

Design-before-code plus bounded proof that implementation events refine it.

### Level 4 — temporal/parameterized proof

Fairness, liveness, unbounded node counts, Lean certificates.

Teams receive value at every level.

## Why comparable systems have not unified this

The pieces historically optimize different workflows:

- theorem provers privilege proof expressiveness;
- model checkers privilege abstract models;
- DST frameworks privilege implementation fidelity;
- concurrency checkers privilege schedule control;
- runtime monitors privilege low-overhead observation.

Asupersync's explicit capabilities, cancellation, obligations, and Lab semantics create an unusually coherent concrete substrate. CIR and CML connect it upward; Lean checks the bridges.

## Key existential risks

1. **Asupersync immaturity.** Mitigate with adapter isolation and semantic conformance.
2. **Language scope explosion.** Let corpus waves drive features; reject unneeded generality.
3. **Proof-program sinkhole.** Formalize kernels and transformations, not every optimizer.
4. **State explosion.** Portfolio plus abstraction; never promise universality.
5. **Rust ecosystem friction.** Offer incremental adoption and Tokio/foreign boundaries outside verified core.
6. **False “same code” claim.** Explicitly track modeled/contracted/unverified dependencies.
7. **Agent-generated noise.** Only evidence changes claim status.

## Metrics that matter

- known bugs reproduced from migrated DSTs;
- median minimized causal-core size;
- exact replay rate;
- integration code removed per project;
- corpus parity coverage;
- mutation kill rate;
- time to first counterexample;
- certificate check/search ratio;
- proof maintenance per semantic change;
- agent repair success under proof gates;
- production traces classified without false validity.

## The north-star demonstration

A production Rust service has a rare cancellation/durability race. Continuum:

1. observes an ambiguous production trace;
2. identifies missing instrumentation rather than guessing;
3. reproduces the permitted causal envelope under Lab;
4. finds the violating schedule;
5. minimizes it;
6. maps it to an abstract model violation;
7. an agent proposes a fix and invariant;
8. exact replay and neighboring exploration pass;
9. a refinement/safety certificate checks in Lean;
10. the same semantic instrumentation guards production.

That is a micro-revolution, not a faster model checker.
