# Candidate Novel Contributions

This distinguishes actual research contributions from competent integration.

## Contribution A: Partial-order semantic substrate spanning model, runtime, and production

Existing systems usually begin with one of:

- abstract transition systems;
- controlled runtime schedules;
- program traces;
- process-mining event logs.

Continuum proposes one typed causal representation that can project into all of them, with checkable projection obligations.

**Novelty risk:** event structures/partial orders are old; the contribution must be the cross-layer contract and evidence architecture, not the data structure alone.

## Contribution B: Cancellation and obligation calculus tied to a production runtime

Structured cancellation, reserve/commit effects, region ownership, and quiescence become formal state and progress obligations.

**Novelty risk:** linear/session/resource logics already exist. The publishable result needs a precise calculus, sound mapping to asupersync, useful automation, and bugs/results beyond existing tools.

## Contribution C: Observer-sensitive partial-order reduction

Dependence is parameterized by property observers, fairness, obligations, and fault semantics rather than only concrete memory/resource conflict.

**Novelty risk:** property-driven POR exists. The advance must be the rich distributed/runtime observer model and certifying implementation.

## Contribution D: Proof-carrying partial-order closure

An unfolding/DPOR exploration emits compact evidence that all relevant causal classes are covered, checked independently.

**Novelty risk:** model-checking certificates and unfolding cutoffs exist. The contribution must improve certificate practicality for runtime-derived systems.

## Contribution E: Multi-grain refinement with executable counterexample transport

Counterexamples move between service, protocol, operational, runtime, and production views; infeasible witnesses drive localized grain refinement.

**Novelty risk:** CEGAR and multi-grain specs exist. The novel part is unified authoring/runtime replay and causal diagnostics.

## Contribution F: Partial-order production conformance plus instrumentation synthesis

Validate incomplete interval/causal traces without fake total ordering; analyze monitorability; synthesize minimal probes needed to decide a property.

**Novelty risk:** trace validation and process conformance are active fields. Instrumentation synthesis and direct Lab reproduction may be differentiators.

## Contribution G: Causal Cubical Reduction

Use higher-dimensional cells to collapse families of commuting distributed effects and preserve observer-relevant directed behavior.

**Novelty risk:** highest. It may fail entirely. It needs a clear preservation theorem and benchmark wins over optimal DPOR/unfoldings.

## Contribution H: Sheaf-based refinement gluing and blame

Local refinement witnesses glue to a global witness; obstruction/minimal ungluable covers diagnose inconsistent composition.

**Novelty risk:** strong mathematics, unclear engineering payoff. Direct SAT is the baseline.

## Contribution I: Evidence-domain algebra

Partial verification artifacts form a composable information order with safe joins, resumable/distributed checking, and no scalar confidence collapse.

**Novelty risk:** related to proof lattices/domain theory and build systems. Value depends on clear semantics and real artifact reuse.

## Contribution J: Proof-gated autonomous repair

Agents consume causal failures and emit proof-carrying repairs subject to neighborhood closure and semantic-diff gates.

**Novelty risk:** integration novelty unless evaluated on difficult real distributed bugs with low false-fix rates.

## Publication sequence

1. **Cancellation + CIR + asupersync mapping.**
2. **Partial-order runtime/model refinement and causal crashpacks.**
3. **Production partial-order conformance and instrumentation synthesis.**
4. **Certifying observer-sensitive DPOR/unfoldings.**
5. **Multi-grain refinement.**
6. **Cubical/sheaf methods only after positive evidence.**

A failed hypothesis is still valuable if the benchmark and negative result are rigorous.

## Revision-2 candidate contributions

### Contribution K: Corpus-governed semantic replacement rather than source compatibility

Use the TLA+ Examples corpus as a behavioral/theorem contract for a different, typed, implementation-linked environment. The contribution is a measurable definition of “replaces TLA+ for this domain.”

### Contribution L: Semantic triptych with independent evidence paths

Model, real program and proof remain first-class, with typed edges and proof receipts. This avoids both drift and circular self-validation.

### Contribution M: Observer lattice as a shared optimization/monitoring object

The same observer refinement structure governs DPOR independence, state slicing, refinement granularity, production observability and cache reuse.

### Contribution N: Assumption synthesis connected to domain packs and production monitors

Game-derived environment contracts become executable fault policies and monitored deployment assumptions rather than isolated synthesis output.

### Contribution O: Nominal causal verification for fresh identifiers

Combine alpha-equivalent fresh-name quotienting with causal event structures, preserving order-sensitive observers and runtime replay.

### Contribution P: Checked choreographic projection to cancel-correct Rust

Project global protocol models into asupersync role interfaces, lifecycle-aware monitors and refinement targets.

### Contribution Q: Proof-receipt supply chain for model checking

Bind semantic closure, reductions, solver certificates, Lean theorem/axiom metadata and independent checker identity into a reusable artifact.

### Contribution R: Property-directed abstraction repair loop for agents

Agents propose views/invariants/ghost state; CEGAR, mutants and proof obligations prevent trace-fitting and semantic weakening.
