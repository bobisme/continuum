# Governance and Engineering Discipline

## 1. Repository constitution

### Code policy

- Rust edition/toolchain pinned. (delivered: bn-1j0i)
- `unsafe` forbidden by default. (delivered: bn-1j0i)
- deterministic collections in semantic paths. (delivered: bn-1j0i)
- no ambient time/RNG in core. (delivered: bn-1j0i)
- no platform-dependent hashing in canonical formats. (delivered: bn-1j0i)
- no network access in certificate checking. (delivered: bn-1j0i)
- dependency additions require rationale and TCB classification. (delivered: bn-1j0i, bn-98nd)

### Semantic policy

- semantic changes require ADR; (delivered: bn-1j0i, bn-98nd)
- every breaking change increments semantic epoch; (delivered: bn-1j0i, bn-98nd)
- every pack operation has a normative contract; (delivered: bn-1j0i, bn-98nd)
- reference semantics precedes optimization; (delivered: bn-1j0i, bn-98nd)
- ambiguous behavior is an error, not implementation freedom. (delivered: bn-1j0i, bn-98nd)

These twelve obligations are enforced by `tools/governance/check_code_policy.py`
(self-testing, 56 fixtures; evidence in `tools/governance/evidence/gov-1.json`;
dependency rationale and TCB classes in
`tools/governance/dependency-rationale.toml`). GOV-1-08…12 are enforced at
their checkable core with the proxy boundary stated per entry in the evidence
file; a two-revision gate (diffing against the merge base) would make
GOV-1-08/09 genuinely enforceable and is tracked as follow-up work.

## 2. Decision process

ADRs have statuses:

- proposed; (delivered: bn-29gc)
- accepted; (delivered: bn-29gc)
- superseded; (delivered: bn-29gc)
- rejected; (delivered: bn-29gc)
- experimental. (delivered: bn-29gc)

An ADR must include:

- context; (delivered: bn-29gc)
- decision; (delivered: bn-29gc)
- formal consequences; (delivered: bn-29gc)
- alternatives; (delivered: bn-29gc)
- compatibility; (delivered: bn-29gc)
- security; (delivered: bn-29gc)
- performance hypothesis; (delivered: bn-29gc)
- validation plan; (delivered: bn-29gc, bn-xjz8)
- rollback. (delivered: bn-29gc, bn-xjz8)

These fourteen obligations are enforced by `tools/governance/check_adr_process.py`
(self-testing; evidence in `tools/governance/evidence/gov-2.json`; pre-existing
gaps grandfathered per ADR and per section in
`tools/governance/adr-grandfathered.toml`, 52 of 53 ADRs as of 2026-07-31).

## 3. Claim governance

Public docs use controlled verbs:

- `observed`; (delivered: bn-227c)
- `tested`; (delivered: bn-227c)
- `bounded`; (delivered: bn-227c)
- `exhaustively checked`; (delivered: bn-227c)
- `proved under`; (delivered: bn-227c)
- `certificate checked`; (delivered: bn-227c)
- `hypothesized`. (delivered: bn-227c, bn-18cg)

CI cross-checks claim IDs.

These obligations are enforced by `tools/governance/check_claim_governance.py`
(self-testing; evidence in `tools/governance/evidence/gov-3.json`). The claim
registry is `docs/18_CLAIMS_MATRIX.md` (C001–C035), mirrored by the generated
requirements registry and bound bidirectionally by the check; four pre-existing
instances are grandfathered by exact sentence, named in the evidence file.

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
