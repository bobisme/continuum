# Governance and Engineering Discipline

## 1. Repository constitution

### Code policy

- Rust edition/toolchain pinned.
- `unsafe` forbidden by default.
- deterministic collections in semantic paths.
- no ambient time/RNG in core.
- no platform-dependent hashing in canonical formats.
- no network access in certificate checking.
- dependency additions require rationale and TCB classification.

### Semantic policy

- semantic changes require ADR;
- every breaking change increments semantic epoch;
- every pack operation has a normative contract;
- reference semantics precedes optimization;
- ambiguous behavior is an error, not implementation freedom.

## 2. Decision process

ADRs have statuses:

- proposed;
- accepted;
- superseded;
- rejected;
- experimental.

An ADR must include:

- context;
- decision;
- formal consequences;
- alternatives;
- compatibility;
- security;
- performance hypothesis;
- validation plan;
- rollback.

## 3. Claim governance

Public docs use controlled verbs:

- `observed`;
- `tested`;
- `bounded`;
- `exhaustively checked`;
- `proved under`;
- `certificate checked`;
- `hypothesized`.

CI cross-checks claim IDs.

## 4. Review requirements

### Semantic changes

Require:

- reference tests;
- metamorphic tests;
- differential tests;
- updated schemas;
- migration note;
- security review;
- claim impact.

### POR/reduction changes

Require:

- no-reduction differential campaign;
- mutation of dependence relation;
- liveness/property-class analysis;
- performance report.

### Pack changes

Require:

- fidelity profile;
- host/Lab conformance;
- fault coverage;
- independence review;
- version bump.

### Kernel changes

Require:

- two reviewers;
- fuzz corpus;
- mutation tests;
- code-size report;
- no unchecked optimization.

## 5. Reproducibility

Release assets include:

- source;
- locked dependencies;
- toolchain;
- benchmark manifests;
- claim ledger;
- known unsupported features;
- certificate schemas;
- conformance corpus.

## 6. Experimental feature policy

Research features are behind explicit flags and cannot emit stable certified claims unless promoted.

Example:

```text
--engine cubical-experimental
assurance.proof = CROSS_ENGINE at most
warning = "experimental reduction; certified coverage unavailable"
```

## 7. Compatibility epochs

Separate versions:

- CLI/API semver;
- model-language version;
- semantic epoch;
- CIR format version;
- certificate version;
- pack semantic version;
- crashpack version.

These must not be conflated.

## 8. Community strategy

Early collaborators should be chosen for:

- real distributed Rust systems;
- willingness to expose known bugs/seed corpus;
- formal-methods expertise;
- storage/network semantics;
- independent skepticism.

Avoid optimizing for broad beginner adoption before the semantic product works.

## 9. Publication strategy

Potential research outputs:

- causal IR and cancellation calculus;
- observer-sensitive certifying DPOR;
- proof-carrying partial-order prefixes;
- multi-grained refinement for production Rust;
- production partial-order conformance;
- sheaf/cubical experimental results, including negative findings;
- verified certificate kernel.

Each publication artifact should strengthen the production repository rather than become a disconnected prototype.

## 10. Licensing and ecosystem

The product license is `MIT OR Apache-2.0`, at the recipient's election, decided 2026-07-31 and recorded in [ADR-0053](../adr/0053-product-license-mit-or-apache-2.md) over the analysis in [`notes/LICENSE_DECISION_PACKAGE.md`](../notes/LICENSE_DECISION_PACKAGE.md). `LICENSE-MIT` and `LICENSE-APACHE` at the repository root are the canonical texts and `[workspace.package] license` is the machine-readable form.

That grant covers Continuum's own work only. asupersync, the execution substrate ([ADR-0001](../adr/0001-asupersync-execution-substrate.md)), is distributed under `LicenseRef-MIT-OpenAI-Anthropic-Rider`, whose pass-through clause attaches the rider unmodified to any release artifact that links it; ADR-0053 accepts that exposure and makes disclosing it — in the README, in release notes, and in the shipped license bundle — a release obligation. A model-only build carries no rider. Solver and oracle licenses do not reach release artifacts at all, because [ADR-0029](../adr/0029-semantic-equivalence-not-tla-source-cloning.md) forbids shipping foreign oracle tooling.

Keep certificate formats and CIR openly specified. Domain packs may have separate host-specific dependencies, but the core remains lightweight.

## 11. Release blocker classes

- soundness regression;
- replay instability;
- semantic digest nondeterminism;
- malformed artifact panic in kernel;
- claim-ledger inconsistency;
- undocumented pack behavior change;
- unresolved known false assurance;
- benchmark artifact unreproducible.

Performance regressions are serious but secondary to these.
