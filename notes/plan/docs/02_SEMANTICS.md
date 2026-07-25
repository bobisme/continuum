# Semantic Contract

## 1. Why true concurrency is primary

A total trace chooses an arbitrary order for independent events. It is useful for replay but a poor semantic foundation:

```text
send A→B ; local write C
local write C ; send A→B
```

may represent one causal behavior, not two. Starting from total traces forces every engine to rediscover commutativity and makes production logs appear more ordered than reality.

Continuum instead uses causal configurations and interval pomsets/event structures. Interleavings are linear extensions.

## 2. Core mathematical object

A model \(M\) defines:

\[
M = (S, I, \mathcal{A}, T, O, F)
\]

- \(S\): typed states;
- \(I \subseteq S\): initial states;
- \(\mathcal{A}\): action schemas;
- \(T_a \subseteq S \times Params_a \times S\): transition relations;
- \(O\): observations;
- \(F\): fairness assumptions.

A concrete execution is not merely \(s_0,s_1,\ldots\), but a labeled event structure:

\[
\mathcal{E}=(E,\leq,\#,\lambda,\iota)
\]

- \(E\): events;
- \(\leq\): causality, a partial order;
- \(\#\): conflict, symmetric and hereditary;
- \(\lambda\): semantic labels;
- \(\iota\): interval/resource/obligation metadata.

A finite configuration \(C\) is conflict-free and downward closed. A cut induces state through a deterministic fold where commuting events are required to commute or be marked dependent.

## 3. Determinism condition for state projection

For events \(e\) and \(f\) concurrently enabled in a configuration, if they are declared independent:

\[
apply_f(apply_e(s)) = apply_e(apply_f(s))
\]

and their generated observations are equivalent under the active observer.

This diamond obligation can be:

- proven statically for a pack schema;
- checked dynamically for a specific transition;
- conservatively rejected, making events dependent.

The default is dependence. Performance never overrides soundness.

## 4. Conflict and independence

Dependence sources include:

- overlapping semantic writes;
- read/write conflicts;
- same message/timer/obligation identity;
- lifecycle relation;
- durability ordering;
- explicit synchronization;
- observer-sensitive output;
- fairness/progress interaction;
- pack-defined conflict.

Independence is parameterized:

\[
Indep(e,f \mid View, PropertyClass)
\]

Safety properties often permit more reduction than liveness or hyperproperties. The verifier records which relation was used.

## 5. Time

CIR supports three distinct notions:

1. **model logical time:** abstract counters or clocks;
2. **virtual execution time:** controlled by Lab;
3. **observed wall intervals:** lower/upper bounds from production instrumentation.

An event may have an interval \([start,end]\). Real-time precedence is:

\[
end(e) < start(f) \Rightarrow e \prec f
\]

Overlapping intervals remain unordered unless another causal edge exists.

Timed models add clock valuations and constraints. They are not implicitly inferred from wall timestamps.

## 6. Faults

Faults are events or adversarial choices, never magic mutation. Examples:

- drop/duplicate/delay;
- partition/heal;
- crash/recover;
- volatile-state loss;
- torn or reordered persistence;
- clock jump/skew;
- cancellation;
- budget exhaustion;
- Byzantine corruption, only in packs that explicitly support it.

A fault algebra defines composition and exclusions. For example, a pack may prohibit a disk completion after device destruction unless a recovery model permits it.

## 7. Cancellation calculus

### Lifecycle

```text
Active
  ├─ request(cancel_reason) → Cancelling
  ├─ complete(value)        → Completed
  └─ panic(payload)         → Panicked

Cancelling
  ├─ drain(effect)*
  ├─ finalize(resource)*
  └─ obligations == ∅       → Cancelled
```

### Effect protocol

```text
Idle → Reserved(token)
Reserved(token) → Committed(result)
Reserved(token) → Aborted(reason)
```

A cancellation checkpoint may occur in declared interruptible phases. Committed effects cannot be silently rolled back. Reserved effects must either abort or transfer an obligation to a finalizer.

### Core invariants

- every obligation has one owner or is discharged;
- ownership transfer is causal;
- region close implies no descendant tasks or obligations;
- visible commit implies all required precommit conditions;
- cancellation does not create a visible half-effect;
- finalization order respects durability/resource dependencies.

## 8. Fairness

Fairness objects are typed and scoped:

- `WeakFair(action, scope)`;
- `StrongFair(action, scope)`;
- `EventuallyDelivered(channel predicate)`;
- `EventuallyScheduled(task predicate)`;
- `EventuallyStable(network partition)`;
- `FiniteCrashes(node)`;
- `CancellationResponsive(operation, bound?)`.

No global “fair scheduler” switch exists. Fairness assumptions can be falsified by a concrete execution or remain outside the modeled environment.

## 9. Property languages

### State safety

\[
\forall reachable(s). P(s)
\]

### Transition safety

\[
\forall step(s,s'). Q(s,s')
\]

### Trace/temporal

LTL-like operators, past operators where useful, event quantification, and named fairness.

### Hyperproperties

Properties over sets or tuples of executions:

- noninterference;
- observational determinism;
- probability-distribution preservation;
- differential privacy-style bounds;
- strong linearizability/refinement.

These require dedicated semantics and cannot be inferred from ordinary trace refinement.

### Quantitative

- probability bounds;
- expected cost/time;
- percentile/rare-event targets;
- resource budgets.

## 10. Refinement modes

### Trace inclusion

Every concrete visible trace is accepted by the abstract model.

### Stuttering refinement

Concrete internal steps can leave the abstract state unchanged.

### Forward simulation

A relation \(R(c,a)\) advances with every concrete step.

### Progressive forward simulation

Adds a well-founded progress obligation to prevent infinite concrete stuttering, required for hyperliveness-style claims.

### Strong observational refinement

Preserves selected hyperproperties/scheduler behavior. This is stronger than ordinary linearizability/trace inclusion.

### Approximate refinement

For probabilistic or numeric systems, a metric or error bound relates observations. This is later work and must not share the same verdict as exact refinement.

## 11. Multi-grained composition

Each component exposes:

- interface events;
- assumed environment behavior;
- guaranteed behavior;
- abstract summary;
- hidden internal events;
- refinement certificate.

A mixed-grain system can instantiate a fine model for changed components and summaries for stable components. Compatibility checks ensure interface assumptions compose.

## 12. Semantic normalization

Canonical model form includes:

- alpha-normalized IDs;
- explicit action groups;
- normalized expressions;
- deterministic finite-domain enumeration;
- explicit unchanged variables;
- source-map side tables;
- semantic digest independent of formatting.

This digest anchors replay and certificates.

## 13. Semantic evolution

Changes are classified:

- **representation-only:** same denotation; old artifacts decode;
- **clarification:** previously ambiguous behavior rejected or fixed;
- **semantic extension:** new constructs, old models unchanged;
- **breaking semantic change:** new epoch and migration report.

A model lockfile pins semantic epoch and pack versions.
